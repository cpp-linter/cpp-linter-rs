#![deny(clippy::unwrap_used)]
//! This module holds the functionality related to running clang-format and/or
//! clang-tidy.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

// non-std crates
use clang_tools_manager::{ClangTool, RequestedVersion};
use git_bot_feedback::ReviewComment;
use gix_imara_diff::{BasicLineDiffPrinter, Diff, InternedInput, UnifiedDiffConfig};
use semver::Version;
use tokio::task::JoinSet;

// project-specific modules/crates
use super::common_fs::FileObj;
use crate::error::{ClangCaptureError, ClangTaskError};
use crate::{
    cli::ClangParams,
    rest_client::{RestClient, USER_OUTREACH},
};
pub mod clang_format;
use clang_format::run_clang_format;
pub mod clang_tidy;
use clang_tidy::{CompilationUnit, run_clang_tidy};

/// This creates a task to run clang-tidy and clang-format on a single file.
///
/// Returns a Future that infallibly resolves to a 2-tuple that contains
///
/// 1. The file's path.
/// 2. A collections of cached logs. A [`Vec`] of tuples that hold
///    - log level
///    - messages
fn analyze_single_file(
    file: Arc<Mutex<FileObj>>,
    clang_params: Arc<ClangParams>,
) -> Result<(PathBuf, Vec<(log::Level, String)>), ClangCaptureError> {
    let mut file = file.lock().map_err(|_| ClangCaptureError::MutexPoisoned)?;
    let mut logs = vec![];
    if clang_params.clang_format_command.is_some() {
        if clang_params
            .format_filter
            .as_ref()
            .is_some_and(|f| f.is_qualified(file.name.as_path()))
            || clang_params.format_filter.is_none()
        {
            let format_result = run_clang_format(&mut file, &clang_params)?;
            logs.extend(format_result);
        } else {
            logs.push((
                log::Level::Info,
                format!(
                    "{} not scanned by clang-format due to `--ignore-format`",
                    file.name.as_os_str().to_string_lossy()
                ),
            ));
        }
    }
    if clang_params.clang_tidy_command.is_some() {
        if clang_params
            .tidy_filter
            .as_ref()
            .is_some_and(|f| f.is_qualified(file.name.as_path()))
            || clang_params.tidy_filter.is_none()
        {
            let tidy_result = run_clang_tidy(&mut file, &clang_params)?;
            logs.extend(tidy_result);
        } else {
            logs.push((
                log::Level::Info,
                format!(
                    "{} not scanned by clang-tidy due to `--ignore-tidy`",
                    file.name.as_os_str().to_string_lossy()
                ),
            ));
        }
    }
    Ok((file.name.clone(), logs))
}

/// A struct to contain the version numbers of the clang-tools used
#[derive(Debug, Default)]
pub struct ClangVersions {
    /// The clang-format version used.
    pub format_version: Option<Version>,

    /// The clang-tidy version used.
    pub tidy_version: Option<Version>,
}

/// Runs clang-tidy and/or clang-format and returns the parsed output from each.
///
/// If `tidy_checks` is `"-*"` then clang-tidy is not executed.
/// If `style` is a blank string (`""`), then clang-format is not executed.
pub async fn capture_clang_tools_output(
    files: &[Arc<Mutex<FileObj>>],
    version: &RequestedVersion,
    mut clang_params: ClangParams,
    rest_api_client: &RestClient,
) -> Result<ClangVersions, ClangTaskError> {
    let mut clang_versions = ClangVersions::default();
    // find the executable paths for clang-tidy and/or clang-format and show version
    // info as debugging output.
    if clang_params.tidy_checks != "-*" {
        let tool = ClangTool::ClangTidy;
        let tool_info = version
            .eval_tool(&tool, false, None)
            .await?
            .ok_or(ClangTaskError::FindToolError(tool.as_str()))?;
        log::info!(
            "Using {tool} version {}.{}.{}",
            tool_info.version.major,
            tool_info.version.minor,
            tool_info.version.patch,
        );
        clang_versions.tidy_version = Some(tool_info.version);
        clang_params.clang_tidy_command = Some(tool_info.path);
    }
    if !clang_params.style.is_empty() {
        let tool = ClangTool::ClangFormat;
        let tool_info = version
            .eval_tool(&tool, false, None)
            .await?
            .ok_or(ClangTaskError::FindToolError(tool.as_str()))?;
        log::info!(
            "Using {tool} version {}.{}.{}",
            tool_info.version.major,
            tool_info.version.minor,
            tool_info.version.patch,
        );
        clang_versions.format_version = Some(tool_info.version);
        clang_params.clang_format_command = Some(tool_info.path);
    }

    // parse database (if provided) to match filenames when parsing clang-tidy's stdout
    if let Some(db_path) = &clang_params.database
        && let Ok(db_str) = fs::read(db_path.join("compile_commands.json"))
    {
        clang_params.database_json = Some(
            // A compilation database should be UTF-8 encoded, but file paths are not; use lossy conversion.
            serde_json::from_str::<Vec<CompilationUnit>>(&String::from_utf8_lossy(&db_str))?,
        )
    };

    let mut executors = JoinSet::new();
    let arc_params = Arc::new(clang_params);
    // iterate over the discovered files and run the clang tools
    for file in files {
        let arc_file = file.clone();
        let arc_params = arc_params.clone();
        executors.spawn(async move { analyze_single_file(arc_file, arc_params) });
    }

    while let Some(output) = executors.join_next().await {
        // output?? acts as a fast-fail for any error encountered.
        // This includes any `spawn()` error and any `analyze_single_file()` error.
        // Any unresolved tasks are aborted and dropped when an error is returned here.
        let (file_name, logs) = output??;
        let log_group_name = format!("Analyzing {}", file_name.to_string_lossy());
        rest_api_client.start_log_group(&log_group_name);
        for (level, msg) in logs {
            log::log!(level, "{}", msg);
        }
        rest_api_client.end_log_group(&log_group_name);
    }
    Ok(clang_versions)
}

/// A struct to describe a single suggestion in a pull_request review.
pub struct Suggestion {
    /// The file's line number in the diff that begins the suggestion.
    pub line_start: u32,
    /// The file's line number in the diff that ends the suggestion.
    pub line_end: u32,
    /// The actual suggestion.
    pub suggestion: String,
    /// The file that this suggestion pertains to.
    pub path: String,
}

impl Suggestion {
    pub(crate) fn as_review_comment(&self) -> ReviewComment {
        ReviewComment {
            line_start: Some(self.line_start),
            line_end: self.line_end,
            comment: self.suggestion.clone(),
            path: self.path.clone(),
        }
    }
}

/// A struct to describe the Pull Request review suggestions.
#[derive(Default)]
pub struct ReviewComments {
    /// The total count of suggestions from clang-tidy and clang-format.
    ///
    /// This differs from `comments.len()` because some suggestions may
    /// not fit within the file's diff.
    pub tool_total: [Option<u32>; 2],
    /// A list of comment suggestions to be posted.
    ///
    /// These suggestions are guaranteed to fit in the file's diff.
    pub comments: Vec<Suggestion>,
    /// The complete patch of changes to all files scanned.
    ///
    /// This includes changes from both clang-tidy and clang-format
    /// (assembled in that order).
    pub full_patch: [String; 2],
}

impl ReviewComments {
    pub fn summarize(
        &self,
        clang_versions: &ClangVersions,
        comments: &Vec<ReviewComment>,
    ) -> String {
        let mut body = String::from("## Cpp-linter Review\n");
        for t in 0_usize..=1 {
            let mut total = 0;
            let (tool_name, tool_version) = if t == 0 {
                ("clang-format", clang_versions.format_version.as_ref())
            } else {
                ("clang-tidy", clang_versions.tidy_version.as_ref())
            };
            if tool_version.is_none() {
                // this tool was not used at all
                continue;
            }
            let tool_total = self.tool_total[t].unwrap_or_default();

            // If the tool's version is unknown, then we don't need to output this line.
            // NOTE: If the tool was invoked at all, then the tool's version shall be known.
            if let Some(ver_str) = tool_version {
                body.push_str(format!("\n### Used {tool_name} v{ver_str}\n").as_str());
            }
            for comment in comments {
                if comment
                    .comment
                    .contains(format!("### {tool_name}").as_str())
                {
                    total += 1;
                }
            }

            if total != tool_total {
                body.push_str(
                    format!(
                        "\nOnly {total} out of {tool_total} {tool_name} concerns fit within this pull request's diff.\n",
                    )
                    .as_str(),
                );
            }
            if !self.full_patch[t].is_empty() {
                body.push_str(
                    format!(
                        "\n<details><summary>Click here for the full {tool_name} patch</summary>\n\n```diff\n{}```\n\n</details>\n",
                        self.full_patch[t]
                    ).as_str()
                );
            } else {
                body.push_str(
                    format!(
                        "\nNo concerns reported by {}. Great job! :tada:\n",
                        tool_name
                    )
                    .as_str(),
                )
            }
        }
        body.push_str(USER_OUTREACH);
        body
    }

    pub fn is_comment_in_suggestions(&mut self, comment: &Suggestion) -> bool {
        for s in &mut self.comments {
            if s.path == comment.path
                && s.line_end == comment.line_end
                && s.line_start == comment.line_start
            {
                s.suggestion.push('\n');
                s.suggestion.push_str(comment.suggestion.as_str());
                return true;
            }
        }
        false
    }
}

pub fn make_patch<'buffer>(
    patched: &'buffer str,
    original_content: &'buffer str,
) -> (Diff, InternedInput<&'buffer str>) {
    let input = InternedInput::new(original_content, patched);
    let mut diff = Diff::compute(gix_imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    (diff, input)
}

/// A trait for generating suggestions from a [`FileObj`]'s advice's generated `patched` buffer.
pub trait MakeSuggestions {
    /// Create some user-facing helpful info about what the suggestion aims to resolve.
    fn get_suggestion_help(&self, start_line: u32, end_line: u32) -> String;

    /// Get the tool's name which generated the advice.
    fn get_tool_name(&self) -> String;

    /// Create a bunch of suggestions from a [`FileObj`]'s advice's generated `patched` buffer.
    fn get_suggestions(
        &self,
        review_comments: &mut ReviewComments,
        file_obj: &FileObj,
        diff: &Diff,
        input: &InternedInput<&str>,
        summary_only: bool,
    ) {
        let is_tidy_tool = (&self.get_tool_name() == "clang-tidy") as usize;
        let file_name = file_obj
            .name
            .to_string_lossy()
            .replace("\\", "/")
            .trim_start_matches("./")
            .to_owned();
        let mut config = UnifiedDiffConfig::default();
        config.context_len(0);
        let printer = BasicLineDiffPrinter(&input.interner);
        let unified_diff = diff.unified_diff(&printer, config, input).to_string();
        if !unified_diff.is_empty() {
            let patch_buf = format!("--- a/{file_name}\n+++ b/{file_name}\n{unified_diff}");
            review_comments.full_patch[is_tidy_tool].push_str(patch_buf.as_str());
        }
        if summary_only {
            review_comments.tool_total[is_tidy_tool].get_or_insert(0);
            return;
        }
        let mut hunks_in_patch = 0u32;
        for hunk in diff.hunks() {
            hunks_in_patch += 1;
            let hunk_range = file_obj.is_hunk_in_diff(&hunk);
            match hunk_range {
                None => continue,
                Some((start_line, end_line)) => {
                    let mut suggestion = String::new();
                    let suggestion_help = self.get_suggestion_help(start_line, end_line);
                    if hunk.is_pure_removal() {
                        suggestion.push_str(
                            format!(
                                "Please remove the line(s)\n- {}",
                                hunk.before
                                    .map(|l| l.to_string())
                                    .collect::<Vec<String>>()
                                    .join("\n- ")
                            )
                            .as_str(),
                        );
                    } else {
                        suggestion.push_str("```suggestion\n");
                        for token in
                            &input.after[hunk.after.start as usize..hunk.after.end as usize]
                        {
                            let line = &input.interner[*token];
                            suggestion.push_str(line);
                        }
                        suggestion.push_str("```\n");
                    }
                    let comment = Suggestion {
                        line_start: start_line,
                        line_end: end_line,
                        suggestion: format!("{suggestion_help}\n{suggestion}"),
                        path: file_name.clone(),
                    };
                    if !review_comments.is_comment_in_suggestions(&comment) {
                        review_comments.comments.push(comment);
                    }
                }
            }
        }
        let tool_total = review_comments.tool_total[is_tidy_tool].get_or_insert(0);
        *tool_total += hunks_in_patch;
    }
}
