//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::{
    fs,
    ops::RangeInclusive,
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use log::Level;

// project-specific crates/modules
use crate::{
    clang_tools::make_patch, cli::ClangParams, common_fs::FileObj, error::ClangCaptureError,
};

/// A struct to hold clang-format advice for a single file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatAdvice {
    /// A list of line ranges that clang-format wants to replace.
    pub replacements: Vec<RangeInclusive<u32>>,
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

/// Get a total count of clang-format advice from the given list of [FileObj]s.
pub fn tally_format_advice(files: &[Arc<Mutex<FileObj>>]) -> Result<u64, String> {
    let mut total = 0;
    for file in files {
        let file = file.lock().map_err(|e| e.to_string())?;
        if let Some(advice) = &file.format_advice
            && !advice.replacements.is_empty()
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
    cmd.arg(file.name.to_path_buf().as_os_str());
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

    if !output.stderr.is_empty() || !output.status.success() {
        logs.push((
            log::Level::Debug,
            format!(
                "clang-format raised the follow errors:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    // use a diff between patched and original contents to get format results
    let original_contents =
        fs::read_to_string(clang_params.repo_root.join(&file.name)).map_err(|e| {
            ClangCaptureError::ReadFileFailed {
                file_name: file_name.clone(),
                source: e,
            }
        })?;
    let patched_contents = String::from_utf8(output.stdout.to_vec()).map_err(|e| {
        ClangCaptureError::NonUtf8Output {
            task: "clang-format".to_string(),
            source: e,
        }
    })?;
    let (diff, _) = make_patch(&patched_contents, &original_contents);
    let format_advice = FormatAdvice {
        replacements: diff
            .hunks()
            .filter_map(|hunk| {
                let replacement = if hunk.is_pure_insertion() {
                    RangeInclusive::new(hunk.after.start, hunk.after.start)
                } else {
                    RangeInclusive::new(hunk.before.start, hunk.before.end.saturating_sub(1))
                };
                if ranges.is_empty() {
                    Some(replacement)
                } else {
                    // only include replacements that fall within the specified line ranges
                    if ranges.iter().any(|range| {
                        range.contains(replacement.start()) && range.contains(replacement.end())
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
        let tidy_patch_contents =
            fs::read_to_string(patched_path).map_err(|e| ClangCaptureError::ReadFileFailed {
                file_name: patched_path.to_string_lossy().to_string(),
                source: e,
            })?;
        let (tidy_diff, _) = make_patch(&tidy_patch_contents, &original_contents);
        let mut cmd = Command::new(cmd_path);
        cmd.current_dir(&cache_path);
        // edit the clang-tody patched file in-place (`-i`)
        cmd.args(["--style", &clang_params.style, "-i"]);
        // if ranges is empty, then we're just formatting the entire file.
        if !ranges.is_empty() {
            // We're concerned about formatting what clang-tidy changed (tidy_diff.hunks().before),
            // but we also want to include any clang-format changes that do not overlap clang-tidy fixes.
            let mut joint_ranges = tidy_diff
                .hunks()
                // hunk is partially inclusive: [start, end),
                // but clang-format expects fully inclusive line ranges.
                // subtract 1 from hunk.before.end
                .map(|hunk| {
                    RangeInclusive::new(hunk.before.start, hunk.before.end.saturating_sub(1))
                })
                .collect::<Vec<_>>();
            for range in &ranges {
                let mut contained = false;
                for hunk in tidy_diff.hunks() {
                    if hunk.before.contains(range.start()) && hunk.before.contains(range.end()) {
                        contained = true;
                        break;
                    }
                }
                if !contained {
                    joint_ranges.push(range.clone());
                }
            }
            for range in &joint_ranges {
                cmd.arg(format!("--lines={}:{}", range.start(), range.end()).as_str());
            }
        }
        cmd.arg(&file_name);
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

    file.format_advice = Some(format_advice);
    Ok(logs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{
        env, fs,
        path::PathBuf,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use clang_tools_manager::RequestedVersion;

    use super::{run_clang_format, summarize_style};
    use crate::{
        clang_tools::ClangTool,
        cli::{ClangParams, LinesChangedOnly},
        common_fs::FileObj,
    };

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

    fn get_clang_format_exe() -> PathBuf {
        ClangTool::ClangFormat
            .get_exe_path(
                &RequestedVersion::from_str(
                    env::var("CLANG_VERSION").unwrap_or_default().as_str(),
                )
                .unwrap(),
            )
            .unwrap()
    }

    /// Case 1: Only clang-format runs (no prior clang-tidy patch).
    ///
    /// Verifies that `run_clang_format` caches the format fixes and sets
    /// `file.patched_path` when clang-tidy was not run on the file.
    #[test]
    fn format_only_sets_patched_path() {
        let exe_path = get_clang_format_exe();
        let tmp_workspace = crate::test_common::setup_tmp_workspace();
        let file = FileObj::new(PathBuf::from("demo/demo.cpp"));
        let arc_file = Arc::new(Mutex::new(file));
        let clang_params = ClangParams {
            // Use LLVM style directly (no .clang-format lookup needed)
            style: "llvm".to_string(),
            lines_changed_only: LinesChangedOnly::Off,
            clang_format_command: Some(exe_path),
            repo_root: tmp_workspace.path().to_path_buf(),
            ..Default::default()
        };
        fs::create_dir_all(clang_params.get_cache_path()).unwrap();
        let mut file_lock = arc_file.lock().unwrap();

        // patched_path is not set before running clang-format
        assert!(file_lock.patched_path.is_none());

        run_clang_format(&mut file_lock, &clang_params).unwrap();

        // demo/demo.cpp has formatting issues, so clang-format should produce fixes
        let advice = file_lock.format_advice.as_ref().unwrap();
        assert!(
            !advice.replacements.is_empty(),
            "expected clang-format to report replacements for demo.cpp"
        );
        // patched_path should now be set to the cached format output
        let patched_path = file_lock
            .patched_path
            .as_ref()
            .expect("expected patched_path to be set after format-only run");
        assert!(
            patched_path.exists(),
            "expected cached format file to exist at {patched_path:?}"
        );
        let expected_cache = clang_params
            .get_cache_path()
            .join("demo/demo.cpp");
        assert_eq!(patched_path, &expected_cache);
    }

    /// Case 2: clang-format runs after clang-tidy (patched_path already exists).
    ///
    /// Verifies that when a clang-tidy patch is already cached, `run_clang_format`
    /// applies formatting in-place to the cached tidy-patched file rather than
    /// creating a new cache entry.
    #[test]
    fn format_after_tidy_formats_tidy_patch_in_place() {
        let exe_path = get_clang_format_exe();
        let tmp_workspace = crate::test_common::setup_tmp_workspace();
        // Simulate clang-tidy having run: copy demo.cpp (unformatted) into cache
        // as if tidy patched it but left it still needing clang-format fixes.
        let cache_path = tmp_workspace
            .path()
            .join(ClangParams::CACHE_DIR)
            .join("patched");
        let tidy_cached_file = cache_path.join("demo/demo.cpp");
        fs::create_dir_all(tidy_cached_file.parent().unwrap()).unwrap();
        // Use the original (unformatted) file content to simulate a tidy patch
        // that hasn't been formatted yet.
        let original =
            fs::read_to_string(tmp_workspace.path().join("demo/demo.cpp")).unwrap();
        fs::write(&tidy_cached_file, &original).unwrap();

        let file = FileObj {
            patched_path: Some(tidy_cached_file.clone()),
            ..FileObj::new(PathBuf::from("demo/demo.cpp"))
        };
        let arc_file = Arc::new(Mutex::new(file));
        let clang_params = ClangParams {
            // Use LLVM style directly (no .clang-format lookup needed)
            style: "llvm".to_string(),
            lines_changed_only: LinesChangedOnly::Off,
            clang_format_command: Some(exe_path),
            repo_root: tmp_workspace.path().to_path_buf(),
            ..Default::default()
        };

        let mut file_lock = arc_file.lock().unwrap();
        run_clang_format(&mut file_lock, &clang_params).unwrap();

        // patched_path should remain pointing to the same tidy cache file
        let patched_path = file_lock
            .patched_path
            .as_ref()
            .expect("expected patched_path to remain set");
        assert_eq!(
            patched_path, &tidy_cached_file,
            "patched_path should still point to the tidy cache file"
        );
        // The tidy-cached file should have been formatted in-place and now
        // differ from the original unformatted content.
        let formatted = fs::read_to_string(&tidy_cached_file).unwrap();
        assert_ne!(
            formatted, original,
            "expected clang-format to modify the tidy-cached file in-place"
        );
    }

    /// Case 3: Only clang-tidy runs (no clang-format step).
    ///
    /// Verifies that `run_clang_tidy` sets `file.patched_path` to the cached tidy
    /// output and that the cache file exists, ready for an optional clang-format pass.
    #[test]
    fn tidy_only_sets_patched_path() {
        use crate::clang_tools::clang_tidy::run_clang_tidy;

        let exe_path = ClangTool::ClangTidy
            .get_exe_path(
                &RequestedVersion::from_str(
                    env::var("CLANG_VERSION").unwrap_or_default().as_str(),
                )
                .unwrap(),
            )
            .unwrap();
        let tmp_workspace = crate::test_common::setup_tmp_workspace();
        let file = FileObj::new(PathBuf::from("demo/demo.cpp"));
        let arc_file = Arc::new(Mutex::new(file));
        let clang_params = ClangParams {
            style: "llvm".to_string(),
            tidy_checks: "".to_string(), // use .clang-tidy config file
            lines_changed_only: LinesChangedOnly::Off,
            clang_tidy_command: Some(exe_path),
            repo_root: tmp_workspace.path().to_path_buf(),
            ..Default::default()
        };
        fs::create_dir_all(clang_params.get_cache_path()).unwrap();
        let mut file_lock = arc_file.lock().unwrap();

        // patched_path is not set before running clang-tidy
        assert!(file_lock.patched_path.is_none());

        run_clang_tidy(&mut file_lock, &clang_params).unwrap();

        // After running clang-tidy, patched_path should be set
        let patched_path = file_lock
            .patched_path
            .as_ref()
            .expect("expected patched_path to be set after tidy-only run");
        assert!(
            patched_path.exists(),
            "expected cached tidy file to exist at {patched_path:?}"
        );
        let expected_cache = clang_params.get_cache_path().join("demo/demo.cpp");
        assert_eq!(patched_path, &expected_cache);
    }
}
