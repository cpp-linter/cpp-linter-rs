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
        let mut cmd = Command::new(cmd_path);
        cmd.current_dir(&cache_path);
        // edit the clang-tody patched file in-place (`-i`)
        cmd.args(["--style", &clang_params.style, "-i"]);
        // if ranges is empty, then we're just formatting the entire file.
        if !ranges.is_empty() {
            let tidy_patch_contents = fs::read_to_string(patched_path).map_err(|e| {
                ClangCaptureError::ReadFileFailed {
                    file_name: patched_path.to_string_lossy().to_string(),
                    source: e,
                }
            })?;
            let (tidy_diff, _) = make_patch(&tidy_patch_contents, &original_contents);
            let joint_ranges = three_way_diff(&ranges, tidy_diff);
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

    use super::{summarize_style, three_way_diff};
    use crate::clang_tools::make_patch;

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
}
