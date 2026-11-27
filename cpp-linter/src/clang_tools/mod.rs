//! This module holds the functionality related to running clang-format and/or
//! clang-tidy.

use std::{
    fs,
    path::{Path, PathBuf},
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
    markdown::{Status, plural},
    rest_client::{RestClient, USER_OUTREACH},
};
pub mod clang_format;
use clang_format::run_clang_format;
pub mod clang_tidy;
use clang_tidy::{
    other_files::{TidyFixes, apply_other_file_fixes},
    run_clang_tidy,
};

/// The path to give a clang tool for the source file at `name`.
///
/// clang-format and clang-tidy read an argument that starts with `-` as an option,
/// so a relative path like `-file.cpp` is given as `./-file.cpp`.
/// (`--` can't be used instead: clang-tidy reads the arguments after it as compiler options.)
pub(crate) fn tool_file_arg(name: &Path) -> PathBuf {
    if name.as_os_str().as_encoded_bytes().starts_with(b"-") {
        Path::new(".").join(name)
    } else {
        name.to_path_buf()
    }
}

/// A collection of cached logs: a [`Vec`] of tuples that hold
/// - log level
/// - messages
type Logs = Vec<(log::Level, String)>;

/// Run clang-tidy on a single file.
///
/// Returns the cached logs and the fixes that clang-tidy suggested (if it was run).
fn analyze_tidy(
    file: Arc<Mutex<FileObj>>,
    clang_params: Arc<ClangParams>,
) -> Result<(Logs, Option<TidyFixes>), ClangCaptureError> {
    let mut file = file.lock().map_err(|_| ClangCaptureError::MutexPoisoned)?;
    if clang_params.clang_tidy_command.is_none() {
        return Ok((vec![], None));
    }
    if clang_params
        .tidy_filter
        .as_ref()
        .is_some_and(|f| !f.is_qualified(file.name.as_path()))
    {
        let msg = format!(
            "{} not scanned by clang-tidy due to `--ignore-tidy`",
            file.name.as_os_str().to_string_lossy()
        );
        return Ok((vec![(log::Level::Info, msg)], None));
    }
    let (logs, fixes) = run_clang_tidy(&mut file, &clang_params)?;
    Ok((logs, Some(fixes)))
}

/// Run clang-format on a single file.
///
/// This must happen after all clang-tidy fixes for the `file` are cached
/// (see [`apply_other_file_fixes()`]), because clang-format also formats those fixes.
fn analyze_format(
    file: Arc<Mutex<FileObj>>,
    clang_params: Arc<ClangParams>,
) -> Result<Logs, ClangCaptureError> {
    let mut file = file.lock().map_err(|_| ClangCaptureError::MutexPoisoned)?;
    if clang_params.clang_format_command.is_none() {
        return Ok(vec![]);
    }
    if clang_params
        .format_filter
        .as_ref()
        .is_some_and(|f| !f.is_qualified(file.name.as_path()))
    {
        let msg = format!(
            "{} not scanned by clang-format due to `--ignore-format`",
            file.name.as_os_str().to_string_lossy()
        );
        return Ok(vec![(log::Level::Info, msg)]);
    }
    run_clang_format(&mut file, &clang_params)
}

/// Run clang-tidy and then clang-format on all `files`.
///
/// Files are analyzed in parallel. clang-format only runs after clang-tidy analyzed
/// all files, so that clang-tidy fixes for other analyzed files (eg. a header) are
/// cached first; see [`apply_other_file_fixes()`].
///
/// Returns the cached logs for each file (in the same order as `files`).
async fn analyze_files(
    files: &[Arc<Mutex<FileObj>>],
    clang_params: Arc<ClangParams>,
) -> Result<Vec<Logs>, ClangTaskError> {
    let mut logs = vec![Logs::new(); files.len()];

    let mut executors = JoinSet::new();
    for (index, file) in files.iter().enumerate() {
        let (file, params) = (file.clone(), clang_params.clone());
        executors.spawn(async move { (index, analyze_tidy(file, params)) });
    }
    let mut tidy_fixes = vec![];
    while let Some(output) = executors.join_next().await {
        // output? acts as a fast-fail for any `spawn()` error,
        // and result? does the same for any `analyze_tidy()` error.
        // Any unresolved tasks are aborted and dropped when an error is returned here.
        let (index, result) = output?;
        let (file_logs, fixes) = result?;
        logs[index].extend(file_logs);
        if let Some(fixes) = fixes {
            tidy_fixes.push((index, fixes));
        }
    }
    apply_other_file_fixes(files, &tidy_fixes, &clang_params, &mut logs)?;

    let mut executors = JoinSet::new();
    for (index, file) in files.iter().enumerate() {
        let (file, params) = (file.clone(), clang_params.clone());
        executors.spawn(async move { (index, analyze_format(file, params)) });
    }
    while let Some(output) = executors.join_next().await {
        let (index, result) = output?;
        logs[index].extend(result?);
    }
    Ok(logs)
}

/// Warn that clang-tidy has to guess how each file is compiled when `--database`
/// is not given.
fn warn_without_database(clang_params: &ClangParams) {
    if clang_params.database.is_none() {
        log::warn!(
            "--database is not set, so clang-tidy will guess how each file is compiled \
            and may report \"file not found\" errors that are not in the code. \
            Pass the folder of a compile_commands.json with --database."
        );
    }
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
    if clang_params.tidy_checks != "-*" {
        warn_without_database(&clang_params);
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

    let logs = analyze_files(files, Arc::new(clang_params)).await?;
    for (file, logs) in files.iter().zip(logs) {
        let file_name = file
            .lock()
            .map_err(|_| ClangCaptureError::MutexPoisoned)?
            .name
            .clone();
        let log_group_name = format!("Analyzing {}", file_name.to_string_lossy());
        rest_api_client.start_log_group(&log_group_name);
        for (level, msg) in logs {
            log::log!(level, "{}", msg);
        }
        rest_api_client.end_log_group(&log_group_name);
    }
    check_format_failures(files)?;
    Ok(clang_versions)
}

/// Fail if clang-format failed on every file it was run on.
///
/// When clang-format fails for only some files (eg. an unusual file), those files
/// are counted as failed checks (see [`FormatAdvice::failed`](clang_format::FormatAdvice::failed)).
/// But when it fails for all of them, the cause is most likely the configuration
/// (eg. a bad `--style` value or a `.clang-format` key that the installed version
/// does not know), so there are no results worth reporting.
fn check_format_failures(files: &[Arc<Mutex<FileObj>>]) -> Result<(), ClangTaskError> {
    let (mut attempted, mut failed) = (0, 0);
    for file in files {
        let file = file.lock().map_err(|_| ClangCaptureError::MutexPoisoned)?;
        if let Some(advice) = &file.format_advice {
            attempted += 1;
            if advice.failed {
                failed += 1;
            }
        }
    }
    if attempted > 0 && failed == attempted {
        return Err(ClangTaskError::FormatFailedOnAllFiles(failed));
    }
    Ok(())
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
    /// The count of suggestions (included in `tool_total`) that fit within the file's diff.
    ///
    /// This differs from `comments.len()` because suggestions on the same lines
    /// are merged into one comment.
    pub in_diff: u32,
    /// A list of comment suggestions to be posted.
    ///
    /// These suggestions are guaranteed to fit in the file's diff.
    pub comments: Vec<Suggestion>,
    /// The complete patch of changes to all files scanned.
    ///
    /// This includes changes from both clang-tidy and clang-format
    /// (assembled in that order).
    pub full_patch: String,
    /// Markdown list items about the parts of clang-tidy fixes that were not applied
    /// to other files (see [`clang_tidy::TidyNotification::skipped_fixes`]).
    pub skipped_fixes: Vec<String>,
    /// Markdown list items about concerns that cannot be posted as suggestions:
    /// a file that clang-format failed to run on, or fixes for a file that is
    /// not UTF-8 encoded.
    pub not_posted: Vec<String>,
    /// Whether a finding in the diff is an error (from clang-tidy),
    /// or clang-format failed to run on a file.
    pub has_errors: bool,
}

/// The notice added to a review summary instead of a patch that does not fit.
const PATCH_TOO_LARGE: &str =
    "\nThe full patch of fixes is too large to include in this summary.\n";

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
        let total = comments.len() as u32;
        // Only congratulate if there were no findings, no reused comments,
        // no fix parts left for the user to apply, and no concerns that could not be posted.
        let is_lgtm = self.tool_total == 0
            && !self.has_errors
            && self.full_patch.is_empty()
            && total_review_comments == 0
            && self.skipped_fixes.is_empty()
            && self.not_posted.is_empty();
        let status = if is_lgtm {
            Status::Passed
        } else if self.has_errors {
            Status::Failed
        } else {
            Status::Warned
        };
        let mut body = format!("## {} cpp-linter review\n\n", status.mark());
        let headline = if summary_only && self.tool_total > 0 {
            format!("**Found {}**", plural(self.tool_total.into(), "issue"))
        } else if total > 0 {
            format!("**{}**", plural(total.into(), "comment"))
        } else if is_lgtm {
            log::info!("No concerns to report: LGTM");
            String::from("No issues in this diff")
        } else {
            String::from("No new comments in this diff")
        };
        let versions = [
            (
                ClangTool::ClangFormat,
                clang_versions.format_version.as_ref(),
            ),
            (ClangTool::ClangTidy, clang_versions.tidy_version.as_ref()),
        ]
        .into_iter()
        // If a tool was used, then we know its version at this point.
        .filter_map(|(tool_name, version)| version.map(|ver| format!("{tool_name} {ver}")))
        .collect::<Vec<_>>();
        body.push_str(&headline);
        if !versions.is_empty() {
            body.push_str(&format!(" ({})", versions.join(", ")));
        }
        body.push('\n');

        if !summary_only && self.in_diff != self.tool_total {
            log::info!(
                "Only {} out of {} concerns fit within this pull request's diff.",
                self.in_diff,
                self.tool_total
            );
            body.push_str(
                format!(
                    "\nOnly {} of {} findings are in this pull request's diff.\n",
                    self.in_diff, self.tool_total,
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
            body.push_str(&format!(
                "\n{} already posted in a previous review.\n",
                if dupes == 1 {
                    String::from("1 comment was")
                } else {
                    format!("{dupes} comments were")
                }
            ));
        }
        self.summarize_skipped_fixes(&mut body);
        self.summarize_not_posted(&mut body);
        // The `full_patch` includes all suggestions that didn't fit in the diff.
        // It can also contain suggestions that were duplicates of previous reviews.
        if !self.full_patch.is_empty() {
            let current_len = body.len() + USER_OUTREACH.len();
            let mut patch_prefix = "\n<details><summary>".to_string();
            if summary_only {
                patch_prefix.push_str("The full patch of fixes");
            } else {
                patch_prefix.push_str("A patch of fixes outside the diff");
            }
            patch_prefix.push_str("</summary>\n\n```diff\n");
            let patch_suffix = "```\n\n</details>\n";

            if (current_len + patch_prefix.len() + self.full_patch.len() + patch_suffix.len())
                > u16::MAX as usize
            {
                log::warn!(
                    "The full patch of fixes is too large to include in the review summary."
                );
                body.push_str(PATCH_TOO_LARGE);
            } else {
                body.push_str(&patch_prefix);
                body.push_str(self.full_patch.as_str());
                body.push_str(patch_suffix);
            }
        }
        body.push_str(USER_OUTREACH);
        body
    }

    /// Append the list of [`Self::skipped_fixes`] to the review summary `body`.
    ///
    /// Only the list items that fit within the maximum length of a GitHub comment
    /// (leaving room for [`USER_OUTREACH`]) are added.
    /// The number of items left out is mentioned instead.
    fn summarize_skipped_fixes(&self, body: &mut String) {
        const HEADING: &str = "\nSome clang-tidy fixes also change files that cpp-linter did not change. \
            Please apply these parts yourself:\n\n";
        Self::summarize_list(body, HEADING, &self.skipped_fixes, "skipped fixes");
    }

    /// Append the list of [`Self::not_posted`] concerns to the review summary `body`.
    ///
    /// The same length limit applies as in [`Self::summarize_skipped_fixes()`].
    fn summarize_not_posted(&self, body: &mut String) {
        const HEADING: &str = "\nSome findings could not be posted as suggestions:\n\n";
        Self::summarize_list(
            body,
            HEADING,
            &self.not_posted,
            "concerns that could not be posted",
        );
    }

    /// Append a `heading` and the Markdown list `items` to the review summary `body`.
    ///
    /// `description` names the items in the logs when they do not fit.
    fn summarize_list(body: &mut String, heading: &str, items: &[String], description: &str) {
        if items.is_empty() {
            return;
        }
        // Room kept for the line about omitted items.
        const OMITTED_RESERVE: usize = 100;
        // Also keep room for the notice that replaces a patch that does not fit.
        let max_len = (u16::MAX as usize)
            .saturating_sub(USER_OUTREACH.len() + OMITTED_RESERVE + PATCH_TOO_LARGE.len());
        if body.len() + heading.len() > max_len {
            log::warn!("The list of {description} is too large to include in the review summary.");
            return;
        }
        body.push_str(heading);
        let mut omitted = 0;
        for item in items {
            if omitted == 0 && body.len() + item.len() < max_len {
                body.push_str(item);
                body.push('\n');
            } else {
                omitted += 1;
            }
        }
        if omitted > 0 {
            log::warn!("{omitted} {description} did not fit in the review summary.");
            body.push_str(&format!(
                "\n{omitted} more were not listed because this summary is too long.\n"
            ));
        }
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

/// A file's original and patched contents as text that can be diffed with [`make_patch()`].
///
/// Source files are not always UTF-8 (eg. a file saved as GBK or Latin-1). When either
/// buffer is not valid UTF-8, both buffers are decoded byte for byte as Latin-1 (each
/// byte becomes the `char` of the same value). That keeps line numbers and line contents
/// exact, so a patch written from the diff applies to the original bytes after
/// [`DiffContents::encode()`] turns it back into bytes.
pub struct DiffContents {
    /// The original contents of the file.
    pub original: String,

    /// The patched contents of the file.
    pub patched: String,

    /// Is `false` if either buffer was not valid UTF-8 and both were decoded as Latin-1.
    pub is_utf8: bool,
}

impl DiffContents {
    /// Prepare the `original` and `patched` bytes of a file for diffing.
    pub fn new(original: &[u8], patched: &[u8]) -> Self {
        match (str::from_utf8(original), str::from_utf8(patched)) {
            (Ok(original), Ok(patched)) => Self {
                original: original.to_string(),
                patched: patched.to_string(),
                is_utf8: true,
            },
            _ => Self {
                original: original.iter().copied().map(char::from).collect(),
                patched: patched.iter().copied().map(char::from).collect(),
                is_utf8: false,
            },
        }
    }

    /// Create a [`Diff`] (and its [`InternedInput`]) of the patched contents.
    pub fn make_patch(&self) -> (Diff, InternedInput<&str>) {
        make_patch(&self.patched, &self.original)
    }

    /// Convert `text` that was derived from these contents (eg. a unified diff)
    /// back to the bytes it represents.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        if self.is_utf8 {
            text.as_bytes().to_vec()
        } else {
            // Every char here came from a single byte (or is ASCII, like the
            // markers in a unified diff), so it always fits in a `u8`.
            text.chars()
                .map(|c| u8::try_from(c).unwrap_or(b'?'))
                .collect()
        }
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

    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    use clang_tools_manager::logger::try_init_logger;
    use git_bot_feedback::ReviewComment;

    use super::*;

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
        let rest_client = RestClient::new(&PathBuf::from(".")).unwrap();
        try_init_logger();
        capture_clang_tools_output(&[], &version, clang_params, &rest_client, false).await
    }

    #[test]
    fn warn_only_without_database() {
        try_init_logger();
        warn_without_database(&ClangParams::default());
        warn_without_database(&ClangParams {
            database: Some(PathBuf::from("build")),
            ..Default::default()
        });
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
        try_init_logger();
        log::set_max_level(log::LevelFilter::Info);
        let review_summary = ReviewComments::default().summarize(
            &clang_versions,
            &comments,
            total_review_comments,
            summary_only,
        );
        assert!(review_summary.contains("1 comment was already posted in a previous review."));
    }

    /// A summary-only review of a finding with no fix shows no pass mark.
    #[test]
    fn summary_only_review_with_findings_is_not_passed() {
        let clang_versions = ClangVersions {
            format_version: None,
            tidy_version: Some(PSEUDO_VERSION),
        };
        let review = ReviewComments {
            tool_total: 1,
            in_diff: 1,
            ..Default::default()
        };
        let summary = review.summarize(&clang_versions, &[], 0, true);
        assert!(summary.contains("/m/hw.png"), "{summary}");
        assert!(summary.contains("**Found 1 issue**"), "{summary}");
        let clean = ReviewComments::default().summarize(&clang_versions, &[], 0, true);
        assert!(clean.contains("/m/hp.png"), "{clean}");
    }

    /// An error in the diff makes the review's mark red, like the thread comment's.
    #[test]
    fn review_with_errors_is_failed() {
        let clang_versions = ClangVersions {
            format_version: None,
            tidy_version: Some(PSEUDO_VERSION),
        };
        let review = ReviewComments {
            tool_total: 1,
            in_diff: 1,
            has_errors: true,
            ..Default::default()
        };
        let summary = review.summarize(&clang_versions, &[], 0, true);
        assert!(summary.contains("/m/hf.png"), "{summary}");
    }

    #[test]
    fn summarize_concerns_in_diff() {
        let clang_versions = ClangVersions {
            format_version: None,
            tidy_version: Some(PSEUDO_VERSION),
        };
        let summarize = |in_diff| {
            ReviewComments {
                tool_total: 6,
                in_diff,
                ..Default::default()
            }
            .summarize(&clang_versions, &[], 2, false)
        };
        // concerns merged into fewer comments still fit within the diff
        assert!(!summarize(6).contains("findings are in this"));
        assert!(summarize(2).contains("\nOnly 2 of 6 findings are in this pull request's diff."));
    }

    #[test]
    fn summarize_skipped_fixes() {
        let clang_versions = ClangVersions {
            format_version: None,
            tidy_version: Some(PSEUDO_VERSION),
        };
        let review_summary = ReviewComments {
            skipped_fixes: vec!["- first".to_string(), "- second".to_string()],
            ..Default::default()
        }
        .summarize(&clang_versions, &[], 0, true);
        assert!(
            review_summary.contains("Please apply these parts yourself:\n\n- first\n- second\n")
        );
        assert!(!review_summary.contains("No issues in this diff"));
    }

    #[test]
    fn summarize_not_posted_concerns() {
        let clang_versions = ClangVersions {
            format_version: Some(PSEUDO_VERSION),
            tidy_version: None,
        };
        let review_summary = ReviewComments {
            not_posted: vec!["- **demo.cpp**: clang-format failed".to_string()],
            ..Default::default()
        }
        .summarize(&clang_versions, &[], 0, false);
        assert!(review_summary.contains(
            "Some findings could not be posted as suggestions:\n\n- **demo.cpp**: clang-format failed\n"
        ));
        assert!(!review_summary.contains("No issues in this diff"));
    }

    #[test]
    fn summarize_skipped_fixes_truncated() {
        let clang_versions = ClangVersions {
            format_version: None,
            tidy_version: Some(PSEUDO_VERSION),
        };
        let item = format!("- {}", "a".repeat(1000));
        let review_summary = ReviewComments {
            skipped_fixes: vec![item; 100],
            full_patch: "a".repeat(1000),
            ..Default::default()
        }
        .summarize(&clang_versions, &[], 0, true);
        assert!(review_summary.len() <= u16::MAX as usize);
        assert!(review_summary.contains(PATCH_TOO_LARGE));
        assert!(review_summary.contains("more were not listed because this summary is too long."));
        assert!(review_summary.ends_with(USER_OUTREACH));
    }

    #[test]
    fn summarize_skipped_fixes_no_room() {
        let mut body = "a".repeat(u16::MAX as usize);
        ReviewComments {
            skipped_fixes: vec!["- first".to_string()],
            ..Default::default()
        }
        .summarize_skipped_fixes(&mut body);
        assert_eq!(body.len(), u16::MAX as usize);
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

    #[test]
    fn diff_contents_utf8() {
        let contents = DiffContents::new("// 中文\n".as_bytes(), "// 中文\n\n".as_bytes());
        assert!(contents.is_utf8);
        assert_eq!(contents.original, "// 中文\n");
        assert_eq!(contents.encode("// 中文"), "// 中文".as_bytes());
    }

    #[test]
    fn diff_contents_not_utf8() {
        // "// 中文" in GBK
        let gbk: &[u8] = b"// \xd6\xd0\xce\xc4\n";
        // one buffer that is not UTF-8 is enough to treat both as bytes
        let contents = DiffContents::new(gbk, "// 中文\n".as_bytes());
        assert!(!contents.is_utf8);
        assert_eq!(contents.original.chars().count(), gbk.len());
        assert_eq!(contents.encode(&contents.original), gbk);
        assert_eq!(contents.encode(&contents.patched), "// 中文\n".as_bytes());
        let (diff, _) = contents.make_patch();
        assert_eq!(diff.hunks().count(), 1);
    }

    fn files_with_format_results(results: &[Option<bool>]) -> Vec<Arc<Mutex<FileObj>>> {
        results
            .iter()
            .enumerate()
            .map(|(i, failed)| {
                let mut file = FileObj::new(PathBuf::from(format!("file{i}.cpp")));
                file.format_advice = failed.map(|failed| clang_format::FormatAdvice {
                    failed,
                    ..Default::default()
                });
                Arc::new(Mutex::new(file))
            })
            .collect()
    }

    #[test]
    fn format_failed_on_all_files() {
        // `None` = clang-format was not run on the file (eg. `--ignore-format`)
        let files = files_with_format_results(&[Some(true), None, Some(true)]);
        let err = check_format_failures(&files).unwrap_err();
        assert!(matches!(err, ClangTaskError::FormatFailedOnAllFiles(2)));
    }

    #[test]
    fn format_failed_on_some_files() {
        let files = files_with_format_results(&[Some(true), Some(false)]);
        check_format_failures(&files).unwrap();
        // clang-format was not run at all
        let files = files_with_format_results(&[None, None]);
        check_format_failures(&files).unwrap();
        check_format_failures(&[]).unwrap();
    }

    #[test]
    fn tool_file_arg_for_dash_names() {
        assert_eq!(
            tool_file_arg(Path::new("-dash.cpp")),
            Path::new(".").join("-dash.cpp")
        );
        assert_eq!(
            tool_file_arg(Path::new("src/-dash.cpp")),
            PathBuf::from("src/-dash.cpp")
        );
        assert_eq!(
            tool_file_arg(Path::new("a-b.cpp")),
            PathBuf::from("a-b.cpp")
        );
    }
}
