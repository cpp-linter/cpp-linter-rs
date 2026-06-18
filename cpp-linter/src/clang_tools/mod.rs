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
use gix_imara_diff::{Diff, InternedInput};
use semver::Version;
use tokio::task::JoinSet;

// project-specific modules/crates
use super::common_fs::FileObj;
use crate::{
    clang_tools::clang_tidy::CompilationUnit,
    cli::ClangParams,
    error::{ClangCaptureError, ClangTaskError},
    rest_client::{RestClient, USER_OUTREACH},
};
pub mod clang_format;
use clang_format::run_clang_format;
pub mod clang_tidy;
use clang_tidy::run_clang_tidy;

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

/// Runs clang-tidy and/or clang-format and returns the version used for each.
///
/// If [`ClangParams::tidy_checks`] is `"-*"` then clang-tidy is not executed.
/// If [`ClangParams::style`] is a blank string (`""`), then clang-format is not executed.
///
/// The `modify_system` parameter controls whether or not to use a systems' available
/// package managers when installing the specified `version` of clang tools.
///
/// The provided `rest_api_client` is only used for consistent logging messages.
pub async fn capture_clang_tools_output(
    files: &[Arc<Mutex<FileObj>>],
    version: &RequestedVersion,
    mut clang_params: ClangParams,
    rest_api_client: &RestClient,
    modify_system: bool,
) -> Result<ClangVersions, ClangTaskError> {
    let mut clang_versions = ClangVersions::default();
    // find the executable paths for clang-tidy and/or clang-format and show version
    // info as debugging output.
    if clang_params.tidy_checks != "-*" {
        let tool = ClangTool::ClangTidy;
        let tool_info = version
            .eval_tool(&tool, false, None, modify_system)
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
            .eval_tool(&tool, false, None, modify_system)
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
    if let Some(db_path) = &clang_params.database {
        let db_path = db_path.join("compile_commands.json");
        match fs::read_to_string(&db_path) {
            Ok(db_str) => match serde_json::from_str::<Vec<CompilationUnit>>(&db_str) {
                Ok(db_json) => {
                    clang_params.database_json = Some(db_json);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to parse compilation database JSON at {}: {e:?}",
                        db_path.to_string_lossy()
                    );
                }
            },
            Err(e) => {
                log::warn!(
                    "Failed to read compilation database file at {}: {e:?}",
                    db_path.to_string_lossy()
                );
            }
        }
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
            line_start: if self.line_start == self.line_end {
                None
            } else {
                Some(self.line_start)
            },
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
    pub tool_total: u32,
    /// A list of comment suggestions to be posted.
    ///
    /// These suggestions are guaranteed to fit in the file's diff.
    pub comments: Vec<Suggestion>,
    /// The complete patch of changes to all files scanned.
    ///
    /// This includes changes from both clang-tidy and clang-format
    /// (assembled in that order).
    pub full_patch: String,
}

impl ReviewComments {
    /// Get a markdown-formatted string that summarizes the given [`ReviewComment`]s.
    ///
    /// The total_review_comments parameter describes the number of comments before
    /// removing duplicates found in previous reviews.
    pub fn summarize(
        &self,
        clang_versions: &ClangVersions,
        comments: &[ReviewComment],
        total_review_comments: u32,
        summary_only: bool,
    ) -> String {
        let mut body = String::from("## Cpp-linter Review\n");
        let versions = [
            (
                ClangTool::ClangFormat,
                clang_versions.format_version.as_ref(),
            ),
            (ClangTool::ClangTidy, clang_versions.tidy_version.as_ref()),
        ];
        for (tool_name, tool_version) in versions {
            if let Some(ver) = tool_version {
                // If a tool was used, then we know it's version at this point.
                body.push_str(format!("### Used {tool_name} v{ver}\n").as_str());
            }
        }

        let total = comments.len() as u32;
        if summary_only && self.tool_total > 0 {
            body.push_str(
                format!(
                    "\nFound {} areas of concern according to clang tools output.\n",
                    self.tool_total
                )
                .as_str(),
            );
        }
        if !summary_only && total_review_comments != self.tool_total {
            log::info!(
                "Only {total_review_comments} out of {} concerns fit within this pull request's diff.",
                self.tool_total
            );
            body.push_str(
                format!(
                    "\nOnly {total_review_comments} out of {} concerns fit within this pull request's diff.\n",
                    self.tool_total,
                )
                .as_str(),
            );
        }
        // total number of comments can only go down after culling comments found in previous reviews.
        if total_review_comments > total {
            let dupes = total_review_comments - total;
            log::info!(
                "Found and removed {dupes} concerns that were duplicates of previous reviews."
            );
            body.push_str(
                format!("\n{dupes} suggestions were duplicates of previous reviews.\n").as_str(),
            );
        }
        // The `full_patch` includes all suggestions that didn't fit in the diff.
        // It can also contain suggestions that were duplicates of previous reviews.
        if !self.full_patch.is_empty() {
            let current_len = body.len() + USER_OUTREACH.len();
            let mut patch_prefix = "\n<details><summary>Click here for ".to_string();
            if summary_only {
                patch_prefix.push_str("the full patch of fixes");
            } else {
                patch_prefix.push_str("a patch of fixes outside the diff");
            }
            patch_prefix.push_str("</summary><p>\n\n```diff\n");
            let patch_suffix = "```\n\n</p></details>\n";

            if (current_len + patch_prefix.len() + self.full_patch.len() + patch_suffix.len())
                > u16::MAX as usize
            {
                log::warn!(
                    "The full patch of fixes is too large to include in the review summary."
                );
                body.push_str(
                    "\nThe full patch of fixes is too large to include in this summary.\n",
                );
            } else {
                body.push_str(&patch_prefix);
                body.push_str(self.full_patch.as_str());
                body.push_str(patch_suffix);
            }
        } else if total_review_comments == 0 {
            // Only congratulate if there was no reused comments
            log::info!("No concerns to report: LGTM");
            body.push_str("\nNo concerns to report. Great job! :tada:\n");
        }
        body.push_str(USER_OUTREACH);
        body
    }

    /// Check if a given comment's [`Suggestion`] is already contained within the existing [`Self::comments`].
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

/// A helper function to create a [`Diff`] and its associated [`InternedInput`] from
/// a `patched` buffer and the `original_content`` of the file.
pub fn make_patch<'buffer>(
    patched: &'buffer str,
    original_content: &'buffer str,
) -> (Diff, InternedInput<&'buffer str>) {
    let input = InternedInput::new(original_content, patched);
    let mut diff = Diff::compute(gix_imara_diff::Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);
    (diff, input)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::{env, fs, path::Path};

    use git_bot_feedback::ReviewComment;

    use super::*;
    #[cfg(feature = "bin")]
    use crate::logger::try_init;

    async fn test_db_parse<P: AsRef<Path>>(path: P) -> Result<ClangVersions, ClangTaskError> {
        let clang_params = ClangParams {
            database: Some(path.as_ref().to_path_buf()),
            repo_root: PathBuf::from("."),
            ..Default::default()
        };
        let version = RequestedVersion::default();
        // We don't need to use any specific git REST API client for this.
        unsafe {
            env::remove_var("GITHUB_ACTIONS");
        }
        let rest_client = RestClient::new().unwrap();
        #[cfg(feature = "bin")]
        try_init();
        capture_clang_tools_output(&[], &version, clang_params, &rest_client, false).await
    }

    #[tokio::test]
    async fn bad_db_path() {
        test_db_parse("nonexistent/path").await.unwrap();
    }

    #[tokio::test]
    async fn bad_db_json() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let db_path = tmp_dir.path().join("compile_commands.json");
        fs::write(&db_path, "not a valid json").unwrap();
        test_db_parse(tmp_dir.path()).await.unwrap();
    }

    const PSEUDO_VERSION: Version = Version::new(15, 0, 0);

    /// This test simulates removed suggestions that were reused in other PR reviews.
    ///
    /// We do this as a arbitrary unit test because different clang tools versions
    /// produce different suggestions, which makes any attempt in integrations tests
    /// rather non-deterministic.
    #[test]
    fn summarize_reused_reviews() {
        let comments = vec![ReviewComment {
            line_start: Some(1),
            line_end: 1,
            comment: "First comment".to_string(),
            path: "src/demo.cpp".to_string(),
        }];
        let clang_versions = ClangVersions {
            format_version: Some(PSEUDO_VERSION.clone()),
            tidy_version: Some(PSEUDO_VERSION),
        };
        let total_review_comments = 2;
        let summary_only = false;
        #[cfg(feature = "bin")]
        {
            crate::logger::try_init();
            log::set_max_level(log::LevelFilter::Info);
        }
        let review_summary = ReviewComments::default().summarize(
            &clang_versions,
            &comments,
            total_review_comments,
            summary_only,
        );
        assert!(review_summary.contains("suggestions were duplicates of previous reviews"));
    }

    #[test]
    fn summary_len_truncated() {
        let comments = vec![ReviewComment {
            line_start: Some(1),
            line_end: 1,
            comment: "First comment".to_string(),
            path: "src/demo.cpp".to_string(),
        }];
        let clang_versions = ClangVersions {
            format_version: Some(PSEUDO_VERSION.clone()),
            tidy_version: Some(PSEUDO_VERSION),
        };
        let total_review_comments = 2;
        let summary_only = false;
        let long_patch = "a".repeat(u16::MAX as usize);
        let review_summary = ReviewComments {
            full_patch: long_patch,
            ..Default::default()
        }
        .summarize(
            &clang_versions,
            &comments,
            total_review_comments,
            summary_only,
        );
        assert!(
            review_summary
                .contains("The full patch of fixes is too large to include in this summary.")
        );
    }
}
