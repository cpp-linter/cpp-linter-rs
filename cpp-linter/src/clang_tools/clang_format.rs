//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::{
    fs,
    ops::RangeInclusive,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use gix_imara_diff::{Diff, InternedInput};
use log::Level;

// project-specific crates/modules
use super::{CACHE_DIR, MakeSuggestions};
use crate::{cli::ClangParams, common_fs::FileObj, error::ClangCaptureError};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatAdvice {
    /// A list of [`Replacement`]s that clang-tidy wants to make.
    pub replacements: Vec<RangeInclusive<u32>>,

    pub patched: PathBuf,
}

impl MakeSuggestions for FormatAdvice {
    fn get_suggestion_help(&self, _start_line: u32, _end_line: u32) -> String {
        String::from("### clang-format suggestions\n")
    }

    fn get_tool_name(&self) -> String {
        "clang-format".to_string()
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
    let cache_path = clang_params.repo_root.join(CACHE_DIR).join("patches");
    let cache_format_fixes = cache_path.join(file.name.with_added_extension("format"));
    fs::create_dir_all(
        cache_format_fixes
            .parent()
            .ok_or(ClangCaptureError::UnknownCacheParentPath)?,
    )
    .map_err(ClangCaptureError::MkDirFailed)?;
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
    fs::write(&cache_format_fixes, &output.stdout).map_err(|e| {
        ClangCaptureError::WriteFileFailed {
            file_name: cache_format_fixes.to_string_lossy().to_string(),
            source: e,
        }
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
    let input = InternedInput::new(original_contents.as_str(), patched_contents.as_str());
    let mut diff = Diff::compute(gix_imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    let format_advice = FormatAdvice {
        replacements: diff
            .hunks()
            .filter_map(|hunk| {
                let replacement = if hunk.is_pure_insertion() {
                    RangeInclusive::new(hunk.after.start, hunk.after.end.saturating_sub(1))
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
        patched: cache_format_fixes,
    };
    file.format_advice = Some(format_advice);
    Ok(logs)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::summarize_style;

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
}
