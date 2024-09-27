//! This module holds functionality specific to running clang-format and parsing it's
//! output.

use std::{
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context, Result};
use log::Level;
// non-std crates
use serde::Deserialize;
use serde_xml_rs::de::Deserializer;

// project-specific crates/modules
use super::MakeSuggestions;
use crate::{
    cli::ClangParams,
    common_fs::{get_line_cols_from_offset, FileObj},
};

/// A Structure used to deserialize clang-format's XML output.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename = "replacements")]
pub struct FormatAdvice {
    /// A list of [`Replacement`]s that clang-tidy wants to make.
    #[serde(rename = "$value")]
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
#[derive(Debug, Deserialize, PartialEq)]
pub struct Replacement {
    /// The byte offset where the replacement will start.
    pub offset: usize,

    /// The amount of bytes that will be removed.
    pub length: usize,

    /// The bytes (UTF-8 encoded) that will be added at the [`Replacement::offset`] position.
    #[serde(rename = "$value")]
    pub value: Option<String>,

    /// The line number described by the [`Replacement::offset`].
    ///
    /// This value is not provided by the XML output, but we calculate it after
    /// deserialization.
    pub line: Option<usize>,

    /// The column number on the line described by the [`Replacement::offset`].
    ///
    /// This value is not provided by the XML output, but we calculate it after
    /// deserialization.
    pub cols: Option<usize>,
}

impl Clone for Replacement {
    fn clone(&self) -> Self {
        Replacement {
            offset: self.offset,
            length: self.length,
            value: self.value.clone(),
            line: self.line,
            cols: self.cols,
        }
    }
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
    let mut patched = None;
    if clang_params.format_review {
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
                    .map(|a| a.to_str().unwrap())
                    .collect::<Vec<&str>>()
                    .join(" ")
            ),
        ));
        patched = Some(
            cmd.output()
                .with_context(|| format!("Failed to get fixes from clang-format: {file_name}"))?
                .stdout,
        );
    }
    cmd.arg("--output-replacements-xml");
    logs.push((
        log::Level::Info,
        format!(
            "Running \"{} {}\"",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|x| x.to_str().unwrap())
                .collect::<Vec<&str>>()
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
    if output.stdout.is_empty() {
        return Ok(logs);
    }
    let xml = String::from_utf8(output.stdout)
        .with_context(|| format!("stdout from clang-format was not UTF-8 encoded: {file_name}"))?
        .lines()
        .collect::<Vec<&str>>()
        .join("");
    let config = serde_xml_rs::ParserConfig::new()
        .trim_whitespace(false)
        .whitespace_to_characters(true)
        .ignore_root_level_whitespace(true);
    let event_reader = serde_xml_rs::EventReader::new_with_config(xml.as_bytes(), config);
    let mut format_advice = FormatAdvice::deserialize(&mut Deserializer::new(event_reader))
        .unwrap_or(FormatAdvice {
            replacements: vec![],
            patched: None,
        });
    format_advice.patched = patched;
    if !format_advice.replacements.is_empty() {
        let mut filtered_replacements = Vec::new();
        for replacement in &mut format_advice.replacements {
            let (line_number, columns) = get_line_cols_from_offset(&file.name, replacement.offset);
            replacement.line = Some(line_number);
            replacement.cols = Some(columns);
            for range in &ranges {
                if range.contains(&line_number.try_into().unwrap_or(0)) {
                    filtered_replacements.push(replacement.clone());
                    break;
                }
            }
            if ranges.is_empty() {
                // lines_changed_only is disabled
                filtered_replacements.push(replacement.clone());
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
    use serde::Deserialize;

    #[test]
    fn parse_xml() {
        let xml_raw = r#"<?xml version='1.0'?>
<replacements xml:space='preserve' incomplete_format='false'>
<replacement offset='113' length='5'>&#10;      </replacement>
<replacement offset='147' length='0'> </replacement>
<replacement offset='161' length='0'></replacement>
<replacement offset='165' length='19'>&#10;&#10;</replacement>
</replacements>"#;
        //since whitespace is part of the elements' body, we need to remove the LFs first
        let xml = xml_raw.lines().collect::<Vec<&str>>().join("");

        let expected = FormatAdvice {
            replacements: vec![
                Replacement {
                    offset: 113,
                    length: 5,
                    value: Some(String::from("\n      ")),
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 147,
                    length: 0,
                    value: Some(String::from(" ")),
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 161,
                    length: 0,
                    value: None,
                    line: None,
                    cols: None,
                },
                Replacement {
                    offset: 165,
                    length: 19,
                    value: Some(String::from("\n\n")),
                    line: None,
                    cols: None,
                },
            ],
            patched: None,
        };
        let config = serde_xml_rs::ParserConfig::new()
            .trim_whitespace(false)
            .whitespace_to_characters(true)
            .ignore_root_level_whitespace(true);
        let event_reader = serde_xml_rs::EventReader::new_with_config(xml.as_bytes(), config);
        let document =
            FormatAdvice::deserialize(&mut serde_xml_rs::de::Deserializer::new(event_reader))
                .unwrap();
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
