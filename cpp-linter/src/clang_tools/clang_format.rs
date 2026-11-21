//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::{
    fs,
    ops::RangeInclusive,
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use gix_imara_diff::Diff;
use log::Level;

// project-specific crates/modules
use crate::{
    clang_tools::{DiffContents, tool_file_arg},
    cli::ClangParams,
    common_fs::FileObj,
    error::ClangCaptureError,
};

/// A struct to hold clang-format advice for a single file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatAdvice {
    /// A list of line ranges that clang-format wants to replace.
    pub replacements: Vec<RangeInclusive<u32>>,

    /// Is `true` if clang-format failed to run on the file (it exited with an error).
    ///
    /// Such a file has no [`FormatAdvice::replacements`], but it still counts as a
    /// failed clang-format check, so a broken `--style` or `.clang-format` does not
    /// pass silently.
    pub failed: bool,
}

impl FormatAdvice {
    /// Does this advice count as a failed clang-format check?
    pub fn is_concern(&self) -> bool {
        self.failed || !self.replacements.is_empty()
    }
}

/// Get a string that summarizes the given `--style`
pub fn summarize_style(style: &str) -> String {
    let mut char_iter = style.chars();
    if ["google", "chromium", "microsoft", "mozilla", "webkit"].contains(&style)
        && let Some(first_char) = char_iter.next()
    {
        // capitalize the first letter
        first_char.to_ascii_uppercase().to_string() + char_iter.as_str()
    } else if style == "llvm" || style == "gnu" {
        style.to_ascii_uppercase()
    } else {
        String::from("Custom")
    }
}

/// Describe the given `--style` for a sentence such as "Line 4 does not match {description}."
///
/// - `file` (or no value): the style in `.clang-format`
/// - `file:<path>`: the style in that file
/// - a predefined style (eg. `llvm`): the LLVM style
/// - anything else (eg. `{BasedOnStyle: llvm}`): the configured style
pub fn describe_style(style: &str) -> String {
    if style.is_empty() || style == "file" {
        String::from("the style in .clang-format")
    } else if let Some(path) = style.strip_prefix("file:") {
        format!("the style in {path}")
    } else if [
        "google",
        "chromium",
        "microsoft",
        "mozilla",
        "webkit",
        "llvm",
        "gnu",
    ]
    .contains(&style)
    {
        format!("the {} style", summarize_style(style))
    } else {
        String::from("the configured style")
    }
}

/// Get a total count of clang-format advice from the given list of [FileObj]s.
pub fn tally_format_advice(files: &[Arc<Mutex<FileObj>>]) -> Result<u64, String> {
    let mut total = 0;
    for file in files {
        let file = file.lock().map_err(|e| e.to_string())?;
        if file
            .format_advice
            .as_ref()
            .is_some_and(FormatAdvice::is_concern)
        {
            total += 1;
        }
    }
    Ok(total)
}

/// Run clang-format for a specific `file`, then parse and return it's XML output.
pub fn run_clang_format(
    file: &mut MutexGuard<FileObj>,
    clang_params: &ClangParams,
) -> Result<Vec<(log::Level, String)>, ClangCaptureError> {
    let cmd_path = clang_params
        .clang_format_command
        .as_ref()
        .ok_or(ClangCaptureError::ToolPathUnknown("clang-format"))?;
    let mut cmd = Command::new(cmd_path);
    cmd.current_dir(&clang_params.repo_root);
    let mut logs = vec![];
    cmd.args(["--style", &clang_params.style]);
    let ranges = file.get_ranges(&clang_params.lines_changed_only);
    for range in &ranges {
        cmd.arg(format!("--lines={}:{}", range.start(), range.end()));
    }
    let cache_path = clang_params.get_cache_path();
    let file_name = file.name.to_string_lossy().to_string();
    cmd.arg(tool_file_arg(&file.name));
    logs.push((
        Level::Info,
        format!(
            "Getting format fixes with \"{} {}\"",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    ));
    let output = cmd
        .output()
        .map_err(|e| ClangCaptureError::FailedToRunCommand {
            task: format!("get fixes from clang-format {file_name}"),
            source: e,
        })?;

    // When clang-format exits non-zero (eg. an invalid `--style` value or a
    // `.clang-format` config that the installed clang-format version rejects), its
    // stdout is empty. Diffing that empty output against the file's contents would
    // wrongly report the entire file as needing reformatting (and, with
    // `--fix`/`--gen-patch-path`, emit a patch that deletes the whole file). So make
    // no format suggestions for this file, but count it as a failed check.
    if !output.status.success() {
        logs.push((
            log::Level::Warn,
            format!(
                "clang-format failed to run on {file_name}; counting it as not formatted:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
        file.format_advice = Some(FormatAdvice {
            failed: true,
            ..Default::default()
        });
        return Ok(logs);
    }
    if !output.stderr.is_empty() {
        logs.push((
            log::Level::Debug,
            format!(
                "clang-format raised the follow errors:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    // use a diff between patched and original contents to get format results.
    // The file is read as bytes because it may not be UTF-8 (eg. GBK or Latin-1).
    let original_contents = fs::read(clang_params.repo_root.join(&file.name)).map_err(|e| {
        ClangCaptureError::ReadFileFailed {
            file_name: file_name.clone(),
            source: e,
        }
    })?;
    let format_contents = DiffContents::new(&original_contents, &output.stdout);
    let (diff, _) = format_contents.make_patch();
    let format_advice = FormatAdvice {
        failed: false,
        replacements: diff
            .hunks()
            .filter_map(|hunk| {
                let altered_start = hunk.after.start.saturating_add(1); // convert to 1-based line numbers
                let og_start = hunk.before.start.saturating_add(1); // convert to 1-based line numbers
                let og_end = hunk.before.end; // exclusive end is inclusive for 1-based line numbers
                let replacement = if hunk.is_pure_insertion() {
                    RangeInclusive::new(altered_start, altered_start)
                } else {
                    RangeInclusive::new(og_start, og_end)
                };
                if ranges.is_empty() {
                    Some(replacement)
                } else {
                    // Only include replacements that touch the specified line ranges.
                    // clang-format was limited to these ranges with `--lines`, but it
                    // formats whole statements, so a fix for a changed line can also
                    // change an unchanged line next to it (eg. re-wrapping the arguments
                    // of a call). Such a fix still belongs to the changed lines.
                    if ranges.iter().any(|range| {
                        range.start() <= replacement.end() && replacement.start() <= range.end()
                    }) {
                        Some(replacement)
                    } else {
                        None
                    }
                }
            })
            .collect(),
    };

    // if a clang-tidy patched file exists in cache,
    // get the diff between it and the original file,
    // then format both clang-tidy fixes and any other changes by clang-format fixes.
    if let Some(patched_path) = &file.patched_path
        && patched_path.exists()
    {
        let mut cmd = Command::new(cmd_path);
        cmd.current_dir(&cache_path);
        // edit the clang-tody patched file in-place (`-i`)
        cmd.args(["--style", &clang_params.style, "-i"]);
        // if ranges is empty, then we're just formatting the entire file.
        if !ranges.is_empty() {
            let tidy_patch_contents =
                fs::read(patched_path).map_err(|e| ClangCaptureError::ReadFileFailed {
                    file_name: patched_path.to_string_lossy().to_string(),
                    source: e,
                })?;
            let tidy_contents = DiffContents::new(&original_contents, &tidy_patch_contents);
            let (tidy_diff, _) = tidy_contents.make_patch();
            let joint_ranges = three_way_diff(&ranges, tidy_diff);
            for range in &joint_ranges {
                cmd.arg(format!("--lines={}:{}", range.start(), range.end()).as_str());
            }
        }
        cmd.arg(tool_file_arg(&file.name));
        let output = cmd
            .output()
            .map_err(|e| ClangCaptureError::FailedToRunCommand {
                task: format!("apply clang-format to clang-tidy fixes ({file_name})"),
                source: e,
            })?;
        if !output.stderr.is_empty() || !output.status.success() {
            logs.push((
                log::Level::Debug,
                format!(
                    "clang-format raised the follow errors about clang-tidy fixes:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }
    } else {
        // clang-tidy was not run on this file,
        // so just use the clang-format fixes as the patched content.
        if clang_params.gen_patch {
            let cache_format_fixes = cache_path.join(&file.name);
            fs::create_dir_all(
                cache_format_fixes
                    .parent()
                    .ok_or(ClangCaptureError::UnknownCacheParentPath)?,
            )
            .map_err(ClangCaptureError::MkDirFailed)?;
            fs::write(&cache_format_fixes, &output.stdout).map_err(|e| {
                ClangCaptureError::WriteFileFailed {
                    file_name: cache_format_fixes.to_string_lossy().to_string(),
                    source: e,
                }
            })?;
            file.patched_path = Some(cache_format_fixes);
        }
    }

    file.format_advice = Some(format_advice);
    Ok(logs)
}

/// Essentially does a three way diff without the original source that generated the given `ranges` (simplified hunks).
///
/// The returned list of ranges are lines that need formatting in the clang-tidy patched file,
/// provided by the `tidy_diff`. The given `ranges` are the line numbers in the original file
/// that clang-tidy patched.
fn three_way_diff(ranges: &[RangeInclusive<u32>], tidy_diff: Diff) -> Vec<RangeInclusive<u32>> {
    // We're concerned about the formatting cases:
    //
    // 1. changes that clang-tidy made: `tidy_diff.hunks().after`
    // 2. changes in the current CI event's diff (`ranges`)
    //    that clang-tidy did not touch (`tidy_diff.hunks().before`)
    // 3. changes that do not overlap clang-tidy fixes: `ranges` - `tidy_diff.hunks().before`
    // 4. changes that overlap with clang-tidy fixes. This one is complex because
    //    - tidy fixes can prefix an og range
    //    - tidy fixes can suffix an og range
    //    - tidy fixes can be contained within an og range
    //    - multiple tidy fixes can (in order) suffix, be contained within, and prefix an og range
    let mut joint_ranges = vec![];
    let mut tidy_iter = tidy_diff.hunks().peekable();
    let mut line_shift = 0i32;

    /// Prevent pure removals from causing invalid inclusive ranges.
    fn maybe_push_range(joint_ranges: &mut Vec<RangeInclusive<u32>>, start: u32, end: u32) {
        if start <= end {
            joint_ranges.push(RangeInclusive::new(start, end));
        }
    }

    for og_range in ranges {
        let og_start = *og_range.start();
        let og_end = *og_range.end();

        // track the start and end of a merged range that gets pushed into joint_ranges.
        let mut merged_start = (og_start as i32 + line_shift) as u32;
        let mut merged_end = (og_end as i32 + line_shift) as u32;

        while let Some(tidy_hunk) = tidy_iter.peek() {
            // alias for readability and prevent some repeated calculations
            let before_start = tidy_hunk.before.start.saturating_add(1); // convert to 1-based line numbers
            let before_end = tidy_hunk.before.end; // exclusive end is inclusive for 1-based line numbers
            let after_start = tidy_hunk.after.start.saturating_add(1); // convert to 1-based line numbers
            let after_end = tidy_hunk.after.end; // exclusive end is inclusive for 1-based line numbers
            let delta = tidy_hunk.after.len() as i32 - tidy_hunk.before.len() as i32;

            // The tidy hunk is a pure removal that encompasses the og range.
            if tidy_hunk.is_pure_removal() && before_start <= og_start && before_end >= og_end {
                // Skip the og range and tidy hunk entirely.
                // The line shift must still be adjusted for the pure removal though
                line_shift += delta;
                merged_end = 0; // causes invalid inclusive range which does not get pushed.
                tidy_iter.next(); // skip this tidy hunk
                break; // skip og range and iterate to the next one.
            }

            // tidy hunk is before the og range.
            if before_end < og_start {
                maybe_push_range(&mut joint_ranges, after_start, after_end);
                line_shift += delta;
                // shift the og range by the lines this tidy hunk added or removed
                merged_start = (og_start as i32 + line_shift) as u32;
                merged_end = (og_end as i32 + line_shift) as u32;
                tidy_iter.next();
                continue;
            }

            // tidy hunk is after the og range.
            if before_start > og_end {
                // handle the og range before iterating the next tidy hunk
                break;
            }

            // tidy hunk overlaps with the og range in some way (case 4).
            if (before_start..=before_end).contains(&og_start) {
                merged_start = after_start;
            }

            // commit the line shift now that the tidy hunk start is checked.
            line_shift += delta;

            // tidy hunk suffixes the og range.
            if (before_start..=before_end).contains(&og_end) {
                merged_end = after_end;
                tidy_iter.next(); // this tidy hunk is handled.
                break; // break from loop to push the merged range into joint_ranges.
            }

            // tidy hunk is contained within the og range.
            // so adjust the og range end accordingly and continue iterating tidy hunks
            merged_end = (og_end as i32 + line_shift) as u32;
            tidy_iter.next();
        }

        maybe_push_range(&mut joint_ranges, merged_start, merged_end);
    }

    // handle any remaining tidy hunks that are after all og ranges.
    for tidy_hunk in tidy_iter {
        maybe_push_range(
            &mut joint_ranges,
            tidy_hunk.after.start.saturating_add(1), // convert to 1-based line numbers
            tidy_hunk.after.end, // exclusive end is inclusive for 1-based line numbers
        );
    }

    joint_ranges
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::ops::RangeInclusive;

    use std::{
        path::{Path, PathBuf},
        sync::Mutex,
    };

    use super::{run_clang_format, summarize_style, three_way_diff};
    use crate::{
        clang_tools::make_patch, cli::ClangParams, common_fs::FileObj, error::ClangCaptureError,
        test_common::setup_tmp_workspace,
    };

    fn make_clang_params(repo_root: &Path, clang_format_command: PathBuf) -> ClangParams {
        ClangParams {
            style: "llvm".to_string(),
            clang_format_command: Some(clang_format_command),
            repo_root: repo_root.to_path_buf(),
            ..Default::default()
        }
    }

    #[test]
    fn format_command_not_found() {
        let tmp_workspace = setup_tmp_workspace();
        let clang_params = make_clang_params(
            tmp_workspace.path(),
            tmp_workspace.path().join("non-existent-clang-format"),
        );
        let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
        let err = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap_err();
        assert!(matches!(err, ClangCaptureError::FailedToRunCommand { .. }));
    }

    #[cfg(unix)]
    mod fake_tool {
        use super::*;
        use crate::test_common::make_fake_tool;
        use std::fs;

        #[test]
        fn missing_source_file() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format = make_fake_tool(tools_dir.path(), "clang-format", "");
            let clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/missing.cpp")));
            let err = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap_err();
            assert!(matches!(err, ClangCaptureError::ReadFileFailed { .. }));
        }

        #[test]
        fn non_utf8_output() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format =
                make_fake_tool(tools_dir.path(), "clang-format", r"printf '\377\376'");
            let clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
            let mut file = file.lock().unwrap();
            run_clang_format(&mut file, &clang_params).unwrap();
            // the output replaced every line of the file
            let advice = file.format_advice.as_ref().unwrap();
            assert!(!advice.failed);
            assert!(!advice.replacements.is_empty());
        }

        /// A fix that starts on an unchanged line but also changes a changed line is
        /// reported. A fix that only changes unchanged lines is not.
        #[test]
        fn fix_overlapping_changed_lines() {
            use crate::cli::LinesChangedOnly;

            let tmp_workspace = setup_tmp_workspace();
            fs::write(
                tmp_workspace.path().join("demo/wrap.cpp"),
                "int x = f(1,\n          2,\n          3);\nint y;\nint  z;\n",
            )
            .unwrap();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format = make_fake_tool(
                tools_dir.path(),
                "clang-format",
                r"printf 'int x = f(1, 2, 3);\nint y;\nint z;\n'",
            );
            let mut clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            clang_params.lines_changed_only = LinesChangedOnly::On;
            // only lines 2 and 3 were added; line 1 is unchanged
            #[allow(clippy::single_range_in_vec_init)]
            let file = Mutex::new(FileObj::from(
                PathBuf::from("demo/wrap.cpp"),
                vec![2, 3],
                vec![1..=3],
            ));
            let mut file = file.lock().unwrap();
            run_clang_format(&mut file, &clang_params).unwrap();
            let advice = file.format_advice.as_ref().unwrap();
            // the fix for lines 1-3 is kept, the fix for line 5 (not changed) is not
            assert_eq!(advice.replacements, vec![RangeInclusive::new(1, 3)]);
        }

        /// A source file that is not UTF-8 (eg. GBK) gets format advice like any other.
        #[test]
        fn non_utf8_source_file() {
            let tmp_workspace = setup_tmp_workspace();
            fs::write(
                tmp_workspace.path().join("demo/gbk.cpp"),
                b"// \xd6\xd0\xce\xc4\nint  a;\nint b;\n",
            )
            .unwrap();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format = make_fake_tool(
                tools_dir.path(),
                "clang-format",
                r"printf '// \326\320\316\304\nint a;\nint b;\n'",
            );
            let mut clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            clang_params.gen_patch = true;
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/gbk.cpp")));
            let mut file = file.lock().unwrap();
            run_clang_format(&mut file, &clang_params).unwrap();
            let advice = file.format_advice.as_ref().unwrap();
            assert!(!advice.failed);
            assert_eq!(advice.replacements, vec![RangeInclusive::new(2, 2)]);
            assert!(file.patched_path.as_ref().unwrap().is_file());
        }

        // A clang-format that exits non-zero (eg. a bad `--style` or an unsupported
        // `.clang-format` key) must not be treated as "the whole file should become
        // empty". Instead the file is skipped with no format advice and a warning.
        #[test]
        fn nonzero_exit_skips_file_with_warning() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            // mimic clang-format rejecting the style: an error on stderr, empty stdout,
            // and a non-zero exit code.
            let fake_format = make_fake_tool(
                tools_dir.path(),
                "clang-format",
                "echo 'Invalid value for -style' >&2; exit 1",
            );
            let clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
            let logs = {
                let mut locked = file.lock().unwrap();
                run_clang_format(&mut locked, &clang_params).unwrap()
            };
            // the failure is surfaced at warning level, carrying clang-format's stderr
            let warnings = logs
                .iter()
                .filter(|(lvl, msg)| {
                    *lvl == log::Level::Warn && msg.contains("Invalid value for -style")
                })
                .count();
            assert_eq!(warnings, 1);
            // the file has no format suggestions (NOT every line), so no patch that
            // deletes the file is ever produced. But it counts as a failed check.
            let advice = file.lock().unwrap().format_advice.clone().unwrap();
            assert!(advice.replacements.is_empty());
            assert!(advice.failed);
            assert!(advice.is_concern());
        }

        #[test]
        fn errors_reported_in_stderr() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            // a successful run that still writes to stderr (eg. a warning)
            let fake_format = make_fake_tool(
                tools_dir.path(),
                "clang-format",
                "echo 'some error' >&2; for a in \"$@\"; do f=$a; done; cat \"$f\"",
            );
            let clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            // simulate a file already patched by clang-tidy
            let patched_path = clang_params.get_cache_path().join("demo/demo.cpp");
            fs::create_dir_all(patched_path.parent().unwrap()).unwrap();
            fs::copy("tests/demo/demo.cpp", &patched_path).unwrap();
            let mut file = FileObj::new(PathBuf::from("demo/demo.cpp"));
            file.patched_path = Some(patched_path);
            let file = Mutex::new(file);
            let logs = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap();
            let errors = logs
                .iter()
                .filter(|(lvl, msg)| *lvl == log::Level::Debug && msg.contains("some error"))
                .count();
            assert_eq!(errors, 2);
        }

        #[test]
        fn tidy_patch_not_readable() {
            use crate::cli::LinesChangedOnly;

            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format = make_fake_tool(tools_dir.path(), "clang-format", "");
            let mut clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            clang_params.lines_changed_only = LinesChangedOnly::Diff;
            // the patched path exists, but it is a directory
            let patched_path = clang_params.get_cache_path().join("demo/demo.cpp");
            fs::create_dir_all(&patched_path).unwrap();
            #[allow(clippy::single_range_in_vec_init)]
            let mut file = FileObj::from(PathBuf::from("demo/demo.cpp"), vec![2], vec![1..=5]);
            file.patched_path = Some(patched_path);
            let file = Mutex::new(file);
            let err = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap_err();
            assert!(matches!(err, ClangCaptureError::ReadFileFailed { .. }));
        }

        #[test]
        fn format_tidy_patch_fails() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            // the tool removes itself, so it cannot be invoked a second time
            let fake_format = make_fake_tool(tools_dir.path(), "clang-format", r#"rm -f "$0""#);
            let clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            let patched_path = clang_params.get_cache_path().join("demo/demo.cpp");
            fs::create_dir_all(patched_path.parent().unwrap()).unwrap();
            fs::copy("tests/demo/demo.cpp", &patched_path).unwrap();
            let mut file = FileObj::new(PathBuf::from("demo/demo.cpp"));
            file.patched_path = Some(patched_path);
            let file = Mutex::new(file);
            let err = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap_err();
            assert!(matches!(err, ClangCaptureError::FailedToRunCommand { .. }));
        }

        #[test]
        fn cache_not_writable() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format = make_fake_tool(tools_dir.path(), "clang-format", "");
            let mut clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            clang_params.gen_patch = true;
            // a directory where the patched file should be cached
            fs::create_dir_all(clang_params.get_cache_path().join("demo/demo.cpp")).unwrap();
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
            let err = run_clang_format(&mut file.lock().unwrap(), &clang_params).unwrap_err();
            assert!(matches!(err, ClangCaptureError::WriteFileFailed { .. }));
        }

        #[test]
        fn no_gen_patch() {
            let tmp_workspace = setup_tmp_workspace();
            let tools_dir = tempfile::tempdir().unwrap();
            let fake_format =
                make_fake_tool(tools_dir.path(), "clang-format", "echo 'void main() {}'");
            let mut clang_params = make_clang_params(tmp_workspace.path(), fake_format);
            clang_params.gen_patch = false;
            let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
            let mut file = file.lock().unwrap();
            let _ = run_clang_format(&mut file, &clang_params).unwrap();
            assert!(file.patched_path.is_none());
            assert!(file.format_advice.is_some());
            assert!(!file.format_advice.as_ref().unwrap().replacements.is_empty());
        }
    }

    fn formalize_style(style: &str, expected: &str) {
        assert_eq!(summarize_style(style), expected);
    }

    #[test]
    fn formalize_llvm_style() {
        formalize_style("llvm", "LLVM");
    }

    #[test]
    fn formalize_google_style() {
        formalize_style("google", "Google");
    }

    #[test]
    fn formalize_custom_style() {
        formalize_style("file", "Custom");
    }

    #[test]
    fn tally_counts_concerns() {
        use std::sync::{Arc, Mutex};

        use super::{FormatAdvice, tally_format_advice};
        use crate::common_fs::FileObj;

        let mut with_advice = FileObj::new(std::path::PathBuf::from("demo.cpp"));
        #[allow(clippy::single_range_in_vec_init)]
        {
            with_advice.format_advice = Some(FormatAdvice {
                replacements: vec![1..=2],
                ..Default::default()
            });
        }
        let mut empty_advice = FileObj::new(std::path::PathBuf::from("empty.cpp"));
        empty_advice.format_advice = Some(FormatAdvice::default());
        // a third file with no advice at all is also ignored
        let no_advice = FileObj::new(std::path::PathBuf::from("none.cpp"));
        // a file that clang-format failed to run on is counted
        let mut failed = FileObj::new(std::path::PathBuf::from("failed.cpp"));
        failed.format_advice = Some(FormatAdvice {
            failed: true,
            ..Default::default()
        });
        let files = vec![
            Arc::new(Mutex::new(with_advice)),
            Arc::new(Mutex::new(empty_advice)),
            Arc::new(Mutex::new(no_advice)),
            Arc::new(Mutex::new(failed)),
        ];
        assert_eq!(tally_format_advice(&files).unwrap(), 2);
    }

    #[test]
    fn three_way_diff_mixed() {
        const OG_SRC: &str =
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12";
        // The first hunk line2-3->StringA-B (hunk 2..4 prefixes og range[3..=5]).
        // The second hunk line8-11->StringC-D\nline11\n (hunk 8..12 suffixes og range[7..=9]).
        const TIDY_SRC: &str =
            "line1\nStringA\nStringB\nline4\nline5\nline6\nline7\nline8\nStringC\nStringD\nline11";
        let (tidy_diff, _input) = make_patch(TIDY_SRC, OG_SRC);
        #[cfg(feature = "bin")]
        print_diff(OG_SRC, TIDY_SRC, &tidy_diff, &_input);
        let ranges = vec![RangeInclusive::new(3, 5), RangeInclusive::new(7, 9)];
        println!("tidy diff: {tidy_diff:#?}\ncompared to og ranges: {ranges:?}");
        let joint_ranges = three_way_diff(&ranges, tidy_diff);
        println!("joint ranges: {joint_ranges:#?}");
        assert_eq!(joint_ranges, vec![2..=5, 7..=11]);
    }

    #[test]
    fn three_way_diff_separated() {
        const OG_SRC: &str =
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11";
        // TIDY_SRC removes "line3" which decrements offsets in ranges[5,8] and removes ranges[3,3].
        // TIDY_SRC appends StringE, which handles remaining tidy hunks after done iterating ranges
        const TIDY_SRC: &str =
            "line1\nline2\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nStringE";
        let (tidy_diff, _input) = make_patch(TIDY_SRC, OG_SRC);
        #[cfg(feature = "bin")]
        print_diff(OG_SRC, TIDY_SRC, &tidy_diff, &_input);
        let ranges = vec![3..=3, 5..=8];
        println!("tidy diff: {tidy_diff:#?}\ncompared to og ranges: {ranges:?}");
        let joint_ranges = three_way_diff(&ranges, tidy_diff);
        println!("joint ranges: {joint_ranges:#?}");
        assert_eq!(joint_ranges, vec![4..=7, 10..=11]);
    }

    #[test]
    fn three_way_diff_tidy_hunk_before_range() {
        const OG_SRC: &str = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8";
        // TIDY_SRC replaces "line2" with 2 lines, which is entirely before ranges[5..=6].
        // So the tidy hunk is kept as is and the og range is shifted down by 1 line.
        const TIDY_SRC: &str = "line1\nStringA\nStringB\nline3\nline4\nline5\nline6\nline7\nline8";
        let (tidy_diff, _input) = make_patch(TIDY_SRC, OG_SRC);
        #[cfg(feature = "bin")]
        print_diff(OG_SRC, TIDY_SRC, &tidy_diff, &_input);
        let ranges = vec![RangeInclusive::new(5, 6)];
        println!("tidy diff: {tidy_diff:#?}\ncompared to og ranges: {ranges:?}");
        let joint_ranges = three_way_diff(&ranges, tidy_diff);
        println!("joint ranges: {joint_ranges:#?}");
        assert_eq!(joint_ranges, vec![2..=3, 6..=7]);
    }

    #[cfg(feature = "bin")]
    fn print_diff(
        og: &str,
        altered: &str,
        diff: &gix_imara_diff::Diff,
        input: &gix_imara_diff::InternedInput<&str>,
    ) {
        use clap::builder::styling::{AnsiColor, Color, Style};
        use gix_imara_diff::{BasicLineDiffPrinter, UnifiedDiffConfig};

        println!("---\nOG SRC:");
        for (i, l) in og.lines().enumerate() {
            println!("{i:>2}|{l}");
        }
        println!("---\nALTERED SRC:");
        for (i, l) in altered.lines().enumerate() {
            println!("{i:>2}|{l}");
        }
        let printer = BasicLineDiffPrinter(&input.interner);
        let mut config = UnifiedDiffConfig::default();
        config.context_len(0);
        let unified = diff.unified_diff(&printer, config, input).to_string();
        for l in unified.lines() {
            let style = if l.starts_with('+') {
                Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)))
            } else if l.starts_with('-') {
                Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)))
            } else {
                Style::new()
            };
            println!("{style}{l}{style:#}");
        }
    }

    #[test]
    fn style_descriptions() {
        use super::describe_style;
        assert_eq!(describe_style("file"), "the style in .clang-format");
        assert_eq!(describe_style(""), "the style in .clang-format");
        assert_eq!(
            describe_style("file:../.clang-format"),
            "the style in ../.clang-format"
        );
        assert_eq!(describe_style("llvm"), "the LLVM style");
        assert_eq!(describe_style("google"), "the Google style");
        assert_eq!(
            describe_style("{BasedOnStyle: llvm}"),
            "the configured style"
        );
    }

    /// A file name that starts with `-` is not read as an option by clang-format,
    /// both when getting the format advice and when formatting clang-tidy's fixes.
    #[test]
    fn file_name_starts_with_dash() {
        use clang_tools_manager::{ClangTool, RequestedVersion};
        use std::{fs, str::FromStr};

        let clang_format = ClangTool::ClangFormat
            .get_exe_path(
                &RequestedVersion::from_str(
                    std::env::var("CLANG_VERSION").unwrap_or_default().as_str(),
                )
                .unwrap(),
            )
            .unwrap();
        let tmp_workspace = tempfile::TempDir::with_prefix("cpp-linter-unit-tests_").unwrap();
        fs::write(tmp_workspace.path().join("-dash.cpp"), "int  a;\nint b;\n").unwrap();
        let clang_params = make_clang_params(tmp_workspace.path(), clang_format);
        // simulate a file already patched by clang-tidy
        let patched_path = clang_params.get_cache_path().join("-dash.cpp");
        fs::create_dir_all(patched_path.parent().unwrap()).unwrap();
        fs::write(&patched_path, "int  a;\nint  b;\n").unwrap();
        let mut file = FileObj::new(PathBuf::from("-dash.cpp"));
        file.patched_path = Some(patched_path.clone());
        let file = Mutex::new(file);
        let mut file = file.lock().unwrap();
        run_clang_format(&mut file, &clang_params).unwrap();
        let advice = file.format_advice.as_ref().unwrap();
        assert!(!advice.failed);
        assert_eq!(advice.replacements, vec![RangeInclusive::new(1, 1)]);
        assert_eq!(
            fs::read_to_string(&patched_path).unwrap(),
            "int a;\nint b;\n"
        );
    }
}
