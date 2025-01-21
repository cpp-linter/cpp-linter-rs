//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::{
    fs,
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context, Result};
use log::Level;
use serde::Deserialize;

// project-specific crates/modules
use super::MakeSuggestions;
use crate::{
    cli::ClangParams,
    common_fs::{get_line_count_from_offset, FileObj},
};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct FormatAdvice {
    /// A list of [`Replacement`]s that clang-tidy wants to make.
    #[serde(rename(deserialize = "replacement"))]
    pub replacements: Vec<Replacement>,

    pub patched: Option<Vec<u8>>,
}

impl MakeSuggestions for FormatAdvice {
    fn get_suggestion_help(&self, _start_line: u32, _end_line: u32) -> String {
        String::from("### clang-format suggestions\n")
    }

    fn get_tool_name(&self) -> String {
        "clang-format".to_string()
    }
}

/// A single replacement that clang-format wants to make.
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy, Deserialize)]
pub struct Replacement {
    /// The byte offset where the replacement will start.
    #[serde(rename = "@offset")]
    pub offset: u32,

    /// The line number described by the [`Replacement::offset`].
    ///
    /// This value is not provided by the XML output, but we calculate it after
    /// deserialization.
    #[serde(default)]
    pub line: u32,
}

/// Get a string that summarizes the given `--style`
pub fn summarize_style(style: &str) -> String {
    if ["google", "chromium", "microsoft", "mozilla", "webkit"].contains(&style) {
        // capitalize the first letter
        let mut char_iter = style.chars();
        let first_char = char_iter.next().unwrap();
        first_char.to_uppercase().collect::<String>() + char_iter.as_str()
    } else if style == "llvm" || style == "gnu" {
        style.to_ascii_uppercase()
    } else {
        String::from("Custom")
    }
}

/// Get a total count of clang-format advice from the given list of [FileObj]s.
pub fn tally_format_advice(files: &[Arc<Mutex<FileObj>>]) -> u64 {
    let mut total = 0;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(advice) = &file.format_advice {
            if !advice.replacements.is_empty() {
                total += 1;
            }
        }
    }
    total
}

/// Run clang-tidy for a specific `file`, then parse and return it's XML output.
pub fn run_clang_format(
    file: &mut MutexGuard<FileObj>,
    clang_params: &ClangParams,
) -> Result<Vec<(log::Level, String)>> {
    let mut cmd = Command::new(clang_params.clang_format_command.as_ref().unwrap());
    let mut logs = vec![];
    cmd.args(["--style", &clang_params.style]);
    let ranges = file.get_ranges(&clang_params.lines_changed_only);
    for range in &ranges {
        cmd.arg(format!("--lines={}:{}", range.start(), range.end()));
    }
    let file_name = file.name.to_string_lossy().to_string();
    cmd.arg(file.name.to_path_buf().as_os_str());
    let patched = if !clang_params.format_review {
        None
    } else {
        logs.push((
            Level::Info,
            format!(
                "Getting format fixes with \"{} {}\"",
                clang_params
                    .clang_format_command
                    .as_ref()
                    .unwrap()
                    .to_str()
                    .unwrap_or_default(),
                cmd.get_args()
                    .map(|a| a.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ));
        Some(
            cmd.output()
                .with_context(|| format!("Failed to get fixes from clang-format: {file_name}"))?
                .stdout,
        )
    };
    cmd.arg("--output-replacements-xml");
    logs.push((
        log::Level::Info,
        format!(
            "Running \"{} {}\"",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|x| x.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    ));
    let output = cmd
        .output()
        .with_context(|| format!("Failed to get replacements from clang-format: {file_name}"))?;
    if !output.stderr.is_empty() || !output.status.success() {
        logs.push((
            log::Level::Debug,
            format!(
                "clang-format raised the follow errors:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    let mut format_advice = if !output.stdout.is_empty() {
        let xml = String::from_utf8(output.stdout).with_context(|| {
            format!("XML output from clang-format was not UTF-8 encoded: {file_name}")
        })?;
        quick_xml::de::from_str::<FormatAdvice>(&xml).with_context(|| {
            format!("Failed to parse XML output from clang-format for {file_name}")
        })?
    } else {
        FormatAdvice {
            replacements: vec![],
            patched: None,
        }
    };
    format_advice.patched = patched;
    if !format_advice.replacements.is_empty() {
        let original_contents = fs::read(&file.name).with_context(|| {
            format!(
                "Failed to read file's original content before translating byte offsets: {file_name}",
            )
        })?;
        // get line and column numbers from format_advice.offset
        let mut filtered_replacements = Vec::new();
        for replacement in &mut format_advice.replacements {
            let line_number = get_line_count_from_offset(&original_contents, replacement.offset);
            replacement.line = line_number;
            for range in &ranges {
                if range.contains(&line_number) {
                    filtered_replacements.push(*replacement);
                    break;
                }
            }
            if ranges.is_empty() {
                // lines_changed_only is disabled
                filtered_replacements.push(*replacement);
            }
        }
        format_advice.replacements = filtered_replacements;
    }
    file.format_advice = Some(format_advice);
    Ok(logs)
}

#[cfg(test)]
mod tests {
    use super::{summarize_style, FormatAdvice, Replacement};

    #[test]
    fn parse_blank_xml() {
        let xml = String::new();
        let result = quick_xml::de::from_str::<FormatAdvice>(&xml);
        assert!(result.is_err());
    }

    #[test]
    fn parse_xml() {
        let xml_raw = r#"<?xml version='1.0'?>
<replacements xml:space='preserve' incomplete_format='false'>
<replacement offset='113' length='5'>&#10;      </replacement>
<replacement offset='147' length='0'> </replacement>
<replacement offset='161' length='0'></replacement>
<replacement offset='165' length='19'>&#10;&#10;</replacement>
</replacements>"#
            .as_bytes()
            .to_vec();

        let expected = FormatAdvice {
            replacements: [113, 147, 161, 165]
                .iter()
                .map(|offset| Replacement {
                    offset: *offset,
                    ..Default::default()
                })
                .collect(),
            patched: None,
        };

        let xml = String::from_utf8(xml_raw).unwrap();

        let document = quick_xml::de::from_str::<FormatAdvice>(&xml).unwrap();
        assert_eq!(expected, document);
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
}
