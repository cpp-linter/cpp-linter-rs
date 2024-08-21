//! This crate is the home of functionality that uses the REST API of various git-based
//! servers.
//!
//! Currently, only Github is supported.

use std::sync::{Arc, Mutex};
use std::{future::Future, path::PathBuf};

// non-std crates
use reqwest::header::{HeaderMap, HeaderValue};

// project specific modules/crates
pub mod github_api;
use crate::common_fs::{FileFilter, FileObj};

pub static COMMENT_MARKER: &str = "<!-- cpp linter action -->";
pub static USER_OUTREACH: &str = "\n\nHave any feedback or feature suggestions? [Share it here.](https://github.com/cpp-linter/cpp-linter-action/issues)";

/// A struct to hold a collection of user inputs related to [`ResApiClient.post_feedback()`].
pub struct FeedbackInput {
    pub thread_comments: String,
    pub no_lgtm: bool,
    pub step_summary: bool,
    pub file_annotations: bool,
    pub style: String,
}

impl Default for FeedbackInput {
    /// Construct a [`FeedbackInput`] instance with default values.
    fn default() -> Self {
        FeedbackInput {
            thread_comments: "false".to_string(),
            no_lgtm: true,
            step_summary: false,
            file_annotations: true,
            style: "llvm".to_string(),
        }
    }
}

/// A custom trait that templates necessary functionality with a Git server's REST API.
pub trait RestApiClient {
    /// A way to set output variables specific to cpp_linter executions in CI.
    fn set_exit_code(
        &self,
        checks_failed: u64,
        format_checks_failed: Option<u64>,
        tidy_checks_failed: Option<u64>,
    ) -> u64;

    /// A convenience method to create the headers attached to all REST API calls.
    ///
    /// If an authentication token is provided, this method shall include the relative
    /// information in the returned [HeaderMap].
    fn make_headers(&self, use_diff: Option<bool>) -> HeaderMap<HeaderValue>;

    /// A way to get the list of changed files using REST API calls. It is this method's
    /// job to parse diff blobs and return a list of changed files.
    ///
    /// The context of the file changes are subject to the type of event in which
    /// cpp_linter package is used.
    fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
    ) -> impl Future<Output = Vec<FileObj>>;

    /// Makes a comment in MarkDown syntax based on the concerns in `format_advice` and
    /// `tidy_advice` about the given set of `files`.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    ///
    /// Returns the markdown comment as a string as well as the total count of
    /// `format_checks_failed` and `tidy_checks_failed` (in respective order).
    fn make_comment(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
        max_len: Option<u64>,
    ) -> String {
        let mut comment = format!("{COMMENT_MARKER}\n# Cpp-Linter Report ");
        let mut remaining_length =
            max_len.unwrap_or(u64::MAX) - comment.len() as u64 - USER_OUTREACH.len() as u64;

        if format_checks_failed > 0 || tidy_checks_failed > 0 {
            let prompt = ":warning:\nSome files did not pass the configured checks!\n";
            remaining_length -= prompt.len() as u64;
            comment.push_str(prompt);
            if format_checks_failed > 0 {
                make_format_comment(
                    files,
                    &mut comment,
                    format_checks_failed,
                    &mut remaining_length,
                );
            }
            if tidy_checks_failed > 0 {
                make_tidy_comment(
                    files,
                    &mut comment,
                    tidy_checks_failed,
                    &mut remaining_length,
                );
            }
        } else {
            comment.push_str(":heavy_check_mark:\nNo problems need attention.");
        }
        comment.push_str(USER_OUTREACH);
        comment
    }

    /// A way to post feedback in the form of `thread_comments`, `file_annotations`, and
    /// `step_summary`.
    ///
    /// The given `files` should've been gathered from `get_list_of_changed_files()` or
    /// `list_source_files()`.
    ///
    /// The `format_advice` and `tidy_advice` should be a result of parsing output from
    /// clang-format and clang-tidy (see `capture_clang_tools_output()`).
    ///
    /// All other parameters correspond to CLI arguments.
    fn post_feedback(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        user_inputs: FeedbackInput,
    ) -> impl Future<Output = u64>;
}

fn make_format_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    format_checks_failed: u64,
    remaining_length: &mut u64,
) {
    let opener = format!("\n<details><summary>clang-format reports: <strong>{} file(s) not formatted</strong></summary>\n\n", format_checks_failed);
    let closer = String::from("\n</details>");
    let mut format_comment = String::new();
    *remaining_length -= opener.len() as u64 + closer.len() as u64;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(format_advice) = &file.format_advice {
            if !format_advice.replacements.is_empty() && *remaining_length > 0 {
                let note = format!("- {}\n", file.name.to_string_lossy().replace('\\', "/"));
                if (note.len() as u64) < *remaining_length {
                    format_comment.push_str(&note.to_string());
                    *remaining_length -= note.len() as u64;
                }
            }
        }
    }
    comment.push_str(&opener);
    comment.push_str(&format_comment);
    comment.push_str(&closer);
}

fn make_tidy_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    tidy_checks_failed: u64,
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-tidy reports: <strong>{} concern(s)</strong></summary>\n\n",
        tidy_checks_failed
    );
    let closer = String::from("\n</details>");
    let mut tidy_comment = String::new();
    *remaining_length -= opener.len() as u64 + closer.len() as u64;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(tidy_advice) = &file.tidy_advice {
            for tidy_note in &tidy_advice.notes {
                let file_path = PathBuf::from(&tidy_note.filename);
                if file_path == file.name {
                    let mut tmp_note = format!("- {}\n\n", tidy_note.filename);
                    tmp_note.push_str(&format!(
                        "   <strong>{filename}:{line}:{cols}:</strong> {severity}: [{diagnostic}]\n   > {rationale}\n{concerned_code}",
                        filename = tidy_note.filename,
                        line = tidy_note.line,
                        cols = tidy_note.cols,
                        severity = tidy_note.severity,
                        diagnostic = tidy_note.diagnostic_link(),
                        rationale = tidy_note.rationale,
                        concerned_code = if tidy_note.suggestion.is_empty() {String::from("")} else {
                            format!("\n   ```{ext}\n   {suggestion}\n   ```\n",
                                ext = file_path.extension().expect("file extension was not determined").to_string_lossy(),
                                suggestion = tidy_note.suggestion.join("\n    "),
                            ).to_string()
                        },
                    ).to_string());

                    if (tmp_note.len() as u64) < *remaining_length {
                        tidy_comment.push_str(&tmp_note);
                        *remaining_length -= tmp_note.len() as u64;
                    }
                }
            }
        }
    }
    comment.push_str(&opener);
    comment.push_str(&tidy_comment);
    comment.push_str(&closer);
}
