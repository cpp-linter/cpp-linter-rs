//! This module defines the struct that wraps employs a REST API client.

use std::{
    env,
    io::{BufRead, Write, stderr, stdout},
    ops::RangeInclusive,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use colored::Colorize;
use git_bot_feedback::{
    AnnotationLevel, CommentKind, CommentPolicy, FileAnnotation, FileFilter, LinesChangedOnly,
    OutputVariable, RestApiClient, ReviewAction, ReviewOptions, ThreadCommentOptions,
    client::{init_client, nearest_git_root},
};

use crate::{
    clang_tools::{
        ClangVersions, ReviewComments,
        clang_format::{describe_style, tally_format_advice},
        clang_tidy::{TidyNotification, tally_tidy_advice},
    },
    cli::{FeedbackInput, ThreadComments},
    code_quality::CodeQualityReport,
    common_fs::{FileObj, is_stdout},
    error::ClientError,
    markdown::{self, Status, plural},
    sarif::SarifLog,
};

/// The comment marker used to identify bot comments from other comments (from users or other bots).
pub const COMMENT_MARKER: &str = "<!-- cpp linter action -->\n";

/// The UserAgent header value used in HTTP requests.
pub const USER_AGENT: &str = concat!("cpp-linter/", env!("CARGO_PKG_VERSION"),);

/// The user outreach message displayed in bot comments.
pub const USER_OUTREACH: &str = concat!(
    "\n\n---\n<sub>:sparkles: Powered by [**cpp-linter**](https://cpp-linter.github.io)",
    " &bull; [:speech_balloon: Feedback]",
    "(https://github.com/cpp-linter/cpp-linter-action/issues)</sub>"
);

fn repo_root_prefix(repo_root: &Path, workspace_root: Option<PathBuf>) -> PathBuf {
    let repo_root = std::path::absolute(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let workspace_root = workspace_root.unwrap_or_else(|| repo_root.to_path_buf());
    nearest_git_root(&repo_root)
        .or_else(|| nearest_git_root(&workspace_root))
        .and_then(|git_root| repo_root.strip_prefix(git_root).ok().map(Path::to_path_buf))
        .or_else(|| {
            repo_root
                .strip_prefix(&workspace_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or_default()
}

/// GitHub keeps only this many annotations of each level (error, warning, notice) per step.
pub const GITHUB_ANNOTATIONS_PER_STEP_LIMIT: usize = 10;

/// GitHub keeps only this many annotations in total per job.
pub const GITHUB_ANNOTATIONS_PER_JOB_LIMIT: usize = 50;

/// Describe the file annotations that GitHub will not show,
/// because it keeps only the first [`GITHUB_ANNOTATIONS_PER_STEP_LIMIT`] annotations
/// of each level in a step.
///
/// Returns [`None`] if every annotation fits in the step.
/// The job limit ([`GITHUB_ANNOTATIONS_PER_JOB_LIMIT`]) is only mentioned,
/// because annotations from other steps of the job are unknown.
fn annotation_cap_message(
    annotations: &[FileAnnotation],
    feedback_inputs: &FeedbackInput,
) -> Option<String> {
    let mut over_limit = vec![];
    for (level, name) in [
        (AnnotationLevel::Error, "error"),
        (AnnotationLevel::Warning, "warning"),
        (AnnotationLevel::Notice, "notice"),
    ] {
        let count = annotations.iter().filter(|a| a.severity == level).count();
        if count > GITHUB_ANNOTATIONS_PER_STEP_LIMIT {
            over_limit.push(format!(
                "{} of {count} {name} annotations",
                count - GITHUB_ANNOTATIONS_PER_STEP_LIMIT
            ));
        }
    }
    if over_limit.is_empty() {
        return None;
    }
    let full_list = match (
        feedback_inputs.step_summary,
        feedback_inputs.thread_comments != ThreadComments::Off,
    ) {
        (true, true) => "The step summary and the thread comment list every finding.",
        (true, false) => "The step summary lists every finding.",
        (false, true) => "The thread comment lists them, and the workflow log lists every finding.",
        (false, false) => {
            "Use `--step-summary` or `--thread-comments` to get a list of every finding."
        }
    };
    Some(format!(
        "GitHub shows only the first {GITHUB_ANNOTATIONS_PER_STEP_LIMIT} annotations of each level in a step \
        (and {GITHUB_ANNOTATIONS_PER_JOB_LIMIT} in a job), so {} are not shown. {full_list}",
        over_limit.join(" and "),
    ))
}

fn prefix_feedback_path(file_path: &str, repo_root_prefix: &Path) -> String {
    repo_root_prefix
        .join(file_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn normalize_changed_file_name(
    file_name: &str,
    repo_root_prefix: &Path,
    file_filter: &FileFilter,
) -> Option<PathBuf> {
    let file_name = PathBuf::from(file_name);
    let file_name = if repo_root_prefix.as_os_str().is_empty() {
        file_name
    } else {
        file_name.strip_prefix(repo_root_prefix).ok()?.to_path_buf()
    };
    file_filter.is_qualified(&file_name).then_some(file_name)
}

/// A wrapper struct around a REST API client.
///
/// Underneath, the client may support various Git servers and CI platforms.
/// Currently, the client supports GitHub and GitHub Actions.
pub struct RestClient {
    client: Box<dyn RestApiClient + Sync + Send>,
    repo_root_prefix: PathBuf,
}

impl RestClient {
    /// Initializes the REST API client and sets the UserAgent header.
    ///
    /// See [git_bot_feedback::client::init_client] for details on
    /// possible errors during initialization.
    pub fn new(repo_root: &Path) -> Result<Self, ClientError> {
        let mut client = init_client()?;
        client.set_user_agent(USER_AGENT)?;
        let repo_root_prefix = repo_root_prefix(repo_root, client.workspace_root()?);
        Ok(Self {
            client,
            repo_root_prefix,
        })
    }

    /// Returns the kind of client being used.
    ///
    /// Supported options include "local", "github" and "gitea".
    pub fn client_kind(&self) -> String {
        self.client.client_kind()
    }

    /// Is this a pull request event?
    pub fn is_pr(&self) -> bool {
        self.client.is_pr_event()
    }

    /// Gets a list of changed files.
    ///
    /// Use the [`FileFilter`] and [`LinesChangedOnly`] to filter out files
    /// that might be negligible. Returned file paths and filter patterns are relative to
    /// the root supplied to [`RestClient::new`].
    pub async fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
        lines_changed_only: &LinesChangedOnly,
        base_diff: &Option<String>,
        ignore_index: bool,
    ) -> Result<Vec<FileObj>, ClientError> {
        let mut changed_files_filter = file_filter.clone();
        changed_files_filter.ignored.clear();
        changed_files_filter.not_ignored.clear();
        changed_files_filter.not_ignored.insert(String::new());
        let files = self
            .client
            .get_list_of_changed_files(
                &changed_files_filter,
                lines_changed_only,
                base_diff.to_owned(),
                ignore_index,
            )
            .await?;
        Ok(files
            .into_iter()
            .filter_map(|(file_name, diff_lines)| {
                let file_path =
                    normalize_changed_file_name(&file_name, &self.repo_root_prefix, file_filter)?;
                let diff_chunks = diff_lines
                    .diff_hunks
                    .iter()
                    .map(|hunk| hunk.start..=hunk.end)
                    .collect();
                Some(FileObj::from(
                    file_path,
                    diff_lines.added_lines,
                    diff_chunks,
                ))
            })
            .collect())
    }

    /// Should debug logs be enabled?
    pub fn is_debug_enabled(&self) -> bool {
        self.client.is_debug_enabled()
    }

    /// Start a log group with the given `name`.
    pub fn start_log_group(&self, name: &str) {
        self.client.start_log_group(name)
    }

    /// End a log group with the given `name`.
    pub fn end_log_group(&self, name: &str) {
        self.client.end_log_group(name)
    }

    /// Post various forms of feedback.
    ///
    /// Use [`FeedbackInput`] to configure feedback behavior.
    /// The given [`ClangVersions`] is used in thread comments, step summaries, and PR reviews' summary comment.
    pub async fn post_feedback(
        &mut self,
        files: &[Arc<Mutex<FileObj>>],
        feedback_inputs: FeedbackInput,
        clang_versions: ClangVersions,
    ) -> Result<u64, ClientError> {
        let tidy_checks_failed = tally_tidy_advice(files).map_err(ClientError::MutexPoisoned)?;
        let format_checks_failed =
            tally_format_advice(files).map_err(ClientError::MutexPoisoned)?;
        let mut comment = None;

        if feedback_inputs.file_annotations {
            let mut annotations = Self::build_annotations(files, feedback_inputs.format_run())?;
            for annotation in &mut annotations {
                annotation.path = prefix_feedback_path(&annotation.path, &self.repo_root_prefix);
            }
            self.client.write_file_annotations(&annotations)?;
            if self.client.client_kind() == "github"
                && let Some(msg) = annotation_cap_message(&annotations, &feedback_inputs)
            {
                log::warn!("{msg}");
            }
        }
        if let Some(output_file) = &feedback_inputs.sarif_file {
            SarifLog::new(
                files,
                clang_versions.tidy_version.as_ref(),
                &feedback_inputs.repo_root,
            )?
            .write(output_file)?;
            if is_stdout(output_file) {
                log::info!("Wrote SARIF log to stdout");
            } else {
                log::info!("Wrote SARIF log to {}", output_file.to_string_lossy());
            }
        }
        if let Some(output_file) = &feedback_inputs.code_quality_file {
            let report = CodeQualityReport::new(
                files,
                &feedback_inputs.style,
                &feedback_inputs.repo_root,
                &self.repo_root_prefix,
            )?;
            report.write(output_file)?;
            log::info!(
                "Wrote {} finding(s) to the Code Quality report {}",
                report.len(),
                if is_stdout(output_file) {
                    "on stdout".into()
                } else {
                    output_file.to_string_lossy()
                }
            );
        }
        let output_vars = [
            OutputVariable {
                name: "checks-failed".to_string(),
                value: format!("{}", format_checks_failed + tidy_checks_failed),
            },
            OutputVariable {
                name: "clang-format-checks-failed".to_string(),
                value: format_checks_failed.to_string(),
            },
            OutputVariable {
                name: "clang-tidy-checks-failed".to_string(),
                value: tidy_checks_failed.to_string(),
            },
        ];
        self.client.write_output_variables(&output_vars)?;

        if self.client.is_pr_event() && feedback_inputs.pr_review {
            let summary_only = ["true", "on", "1"].contains(
                &env::var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY")
                    .unwrap_or("false".to_string())
                    .to_lowercase()
                    .as_str(),
            );
            let mut review_comments = ReviewComments::default();
            for file in files {
                let file = file
                    .lock()
                    .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
                file.make_suggestions_from_patch(
                    &mut review_comments,
                    summary_only,
                    &feedback_inputs.repo_root,
                    &feedback_inputs.patch_path,
                )?;
            }
            self.show_patch(&feedback_inputs)?;

            if self.client.is_pr_same_remote() {
                let mut options = ReviewOptions {
                    marker: COMMENT_MARKER.to_string(),
                    comments: {
                        let mut comments = vec![];
                        for suggestion in &review_comments.comments {
                            let mut comment = suggestion.as_review_comment();
                            comment.path =
                                prefix_feedback_path(&comment.path, &self.repo_root_prefix);
                            comments.push(comment);
                        }
                        comments
                    },
                    ..Default::default()
                };

                let total_review_comments = options.comments.len() as u32;
                self.client.cull_pr_reviews(&mut options).await?;
                let has_changes = review_comments.tool_total > 0
                    || !review_comments.skipped_fixes.is_empty()
                    || !review_comments.not_posted.is_empty();
                let is_lgtm = options.comments.is_empty() && !has_changes;
                options.action = if feedback_inputs.passive_reviews {
                    ReviewAction::Comment
                } else if is_lgtm {
                    ReviewAction::Approve
                } else {
                    ReviewAction::RequestChanges
                };
                options.summary = review_comments.summarize(
                    &clang_versions,
                    &options.comments,
                    total_review_comments,
                    summary_only,
                );
                if is_lgtm && feedback_inputs.no_lgtm {
                    log::debug!("Not posting an approved review because `no-lgtm` is true");
                } else if options.comments.is_empty() && total_review_comments > 0 {
                    log::info!(
                        "All review comments were already posted in previous reviews; not posting a new review."
                    );
                } else {
                    self.client.post_pr_review(&options).await?;
                }
            } else {
                log::warn!("PR is from a different remote, skipping review.");
            }
        } else if feedback_inputs.gen_patch {
            for file in files {
                let file = file
                    .lock()
                    .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
                file.maybe_append_patch(&feedback_inputs.repo_root, &feedback_inputs.patch_path)?;
            }
            self.show_patch(&feedback_inputs)?;
        }

        // Only point to the patch once this run has written it (`fix-patch-path` names it).
        let patch_exists = (feedback_inputs.gen_patch || feedback_inputs.pr_review)
            && std::fs::metadata(&feedback_inputs.patch_path).is_ok_and(|meta| meta.len() > 0);
        if feedback_inputs.step_summary || feedback_inputs.summary_output_file.is_some() {
            let summary = Self::make_comment(
                files,
                format_checks_failed,
                tidy_checks_failed,
                &clang_versions,
                None,
                patch_exists,
                feedback_inputs.format_run(),
            )?;
            if feedback_inputs.step_summary {
                self.client.append_step_summary(&summary)?;
            }
            if let Some(output_file) = &feedback_inputs.summary_output_file {
                if let Some(parent) = output_file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| ClientError::MkDirFailed {
                        file_path: output_file.clone(),
                        source: e,
                    })?;
                }
                std::fs::write(output_file, &summary).map_err(|e| {
                    ClientError::SummaryOutputFileWriteFailed {
                        file_path: output_file.to_owned(),
                        source: e,
                    }
                })?;
            }
            comment = Some(summary);
        }

        if feedback_inputs.thread_comments != ThreadComments::Off {
            // do not post thread-comments for on PRs from forks
            if self.client.is_pr_event() && !self.client.is_pr_same_remote() {
                log::warn!("PR is from a different remote, skipping thread comments.");
                return Ok(format_checks_failed + tidy_checks_failed);
            }
            // post thread comment for PR or push event
            if comment.as_ref().is_none_or(|c| c.len() > u16::MAX as usize) {
                comment = Some(Self::make_comment(
                    files,
                    format_checks_failed,
                    tidy_checks_failed,
                    &clang_versions,
                    Some(u16::MAX as u64),
                    patch_exists,
                    feedback_inputs.format_run(),
                )?);
            }
            let options = ThreadCommentOptions {
                policy: if feedback_inputs.thread_comments == ThreadComments::Update {
                    CommentPolicy::Update
                } else {
                    // feedback_inputs.thread_comments is not Off and not Update, so it must be just On.
                    CommentPolicy::Anew
                },
                comment: comment.unwrap_or_default(),
                kind: if format_checks_failed == 0 && tidy_checks_failed == 0 {
                    CommentKind::Lgtm
                } else {
                    CommentKind::Concerns
                },
                marker: COMMENT_MARKER.to_string(),
                no_lgtm: feedback_inputs.no_lgtm,
            };
            self.client.post_thread_comment(options).await?;
        }
        Ok(format_checks_failed + tidy_checks_failed)
    }

    /// Post file annotations.
    pub fn make_annotations(
        files: &[Arc<Mutex<FileObj>>],
        format_run: FormatRun<'_>,
    ) -> Result<Vec<FileAnnotation>, ClientError> {
        Self::build_annotations(files, format_run)
    }

    /// Make the file annotations for [`Self::make_annotations`].
    ///
    /// A title is only the name of the check (`clang-format` or the clang-tidy
    /// diagnostic): GitHub (and a local run) already shows the file name and line
    /// next to it.
    fn build_annotations(
        files: &[Arc<Mutex<FileObj>>],
        format_run: FormatRun<'_>,
    ) -> Result<Vec<FileAnnotation>, ClientError> {
        let style_description = describe_style(format_run.style);
        let mut annotations = vec![];

        for file in files {
            let file = file
                .lock()
                .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
            let name = file.name.to_string_lossy().replace('\\', "/");
            if let Some(format_advice) = &file.format_advice {
                if format_advice.failed {
                    annotations.push(FileAnnotation {
                        severity: AnnotationLevel::Warning,
                        path: name.clone(),
                        start_line: None,
                        end_line: None,
                        start_column: None,
                        end_column: None,
                        title: Some("clang-format".to_string()),
                        message: format!(
                            "clang-format exited with an error for {name}, so its formatting was not checked. See the logs for details."
                        ),
                    });
                }
                // post annotation if any applicable lines were formatted
                // Anchor the annotation to the first added line that needs formatting
                // (GitHub shows an annotation in the diff only on a line of the diff),
                // or else to the first line that needs formatting.
                let first_added_line = format_advice
                    .replacements
                    .iter()
                    .flat_map(|r| file.added_lines.iter().filter(|line| r.contains(line)))
                    .min()
                    .copied();
                if let Some(first_line) = first_added_line.or_else(|| {
                    format_advice
                        .replacements
                        .iter()
                        .filter(|r| !r.is_empty())
                        .map(|r| *r.start())
                        .min()
                }) {
                    let lines = markdown::describe_lines(&format_advice.replacements);
                    let mut lines_chars = lines.chars();
                    let lines = lines_chars
                        .next()
                        .map(|first| {
                            first.to_uppercase().collect::<String>() + lines_chars.as_str()
                        })
                        .unwrap_or_default();
                    annotations.push(FileAnnotation {
                        severity: AnnotationLevel::Notice,
                        path: name.clone(),
                        start_line: Some(first_line as usize),
                        end_line: Some(first_line as usize),
                        start_column: None,
                        end_column: None,
                        title: Some("clang-format".to_string()),
                        message: format!(
                            "{lines} {} not match {style_description}. Run: {}",
                            if lines.starts_with("Lines") {
                                "do"
                            } else {
                                "does"
                            },
                            format_run
                                .command([(name.as_str(), format_advice.replacements.as_slice())]),
                        ),
                    });
                }
            } // end format_advice iterations

            // iterate over clang-tidy advice and post annotations
            // The tidy_advice vector is parallel to the files vector; meaning it serves as a file filterer.
            // lines are already filter as specified to clang-tidy CLI.
            if let Some(tidy_advice) = &file.tidy_advice {
                for note in &tidy_advice.notes {
                    if note.filename == name {
                        let annotation = FileAnnotation {
                            severity: match note.severity.as_str() {
                                "warning" => AnnotationLevel::Warning,
                                "error" => AnnotationLevel::Error,
                                _ => AnnotationLevel::Notice, // default to notice for all else
                            },
                            path: name.clone(),
                            // `line` is required to anchor the annotation to a line in the file
                            start_line: Some(note.line as usize),
                            end_line: Some(note.line as usize),
                            start_column: Some(note.cols as usize),
                            end_column: None,
                            title: Some(note.diagnostic.clone()),
                            message: note.rationale.clone(),
                        };
                        annotations.push(annotation);
                    }
                }
            }
        }
        Ok(annotations)
    }

    /// Makes a comment in Markdown syntax about the concerns in `format_advice` and
    /// `tidy_advice` of the given `files`.
    ///
    /// The same comment is used for the step summary and the thread comment:
    /// a heading that states the result, a table with a row per clang tool,
    /// the findings of each tool, and how to fix them.
    /// With `max_len`, findings that do not fit are left out (and counted),
    /// and so is the "How to fix" section.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    #[allow(clippy::too_many_arguments)]
    fn make_comment(
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
        clang_versions: &ClangVersions,
        max_len: Option<u64>,
        patch_exists: bool,
        format_run: FormatRun<'_>,
    ) -> Result<String, ClientError> {
        let findings = Findings::collect(files)?;
        Ok(findings.to_markdown(&CommentContext {
            total: format_checks_failed + tidy_checks_failed,
            files_checked: files.len() as u64,
            clang_versions,
            max_len: max_len
                .and_then(|len| usize::try_from(len).ok())
                .unwrap_or(usize::MAX),
            patch_exists,
            format_run,
        }))
    }

    /// Show the generated patch content in the console output.
    ///
    /// And also write the patch path to the output variable `fix-patch-path` for CI platforms.
    fn show_patch(&self, feedback_inputs: &FeedbackInput) -> Result<(), ClientError> {
        let auto_fix_patch_path = &feedback_inputs.patch_path;
        if let Ok(patch_file) = std::fs::File::open(auto_fix_patch_path) {
            self.client.write_output_variables(&[OutputVariable {
                name: "fix-patch-path".to_string(),
                value: auto_fix_patch_path.to_string_lossy().replace("\\", "/"),
            }])?;

            log::info!("Generated patch content:");
            let mut reader = std::io::BufReader::new(patch_file);
            let mut raw_line = Vec::new();
            // `--sarif-file -` or `--code-quality-file -` writes a report to stdout;
            // keep stdout valid JSON.
            let mut stdout_lock: Box<dyn Write> = if feedback_inputs.report_to_stdout() {
                Box::new(stderr().lock())
            } else {
                Box::new(stdout().lock())
            };
            while let bytes_read = reader
                .read_until(b'\n', &mut raw_line)
                .map_err(ClientError::PatchReadFailed)?
                && bytes_read > 0
            {
                // The patch may hold lines of files that are not UTF-8 (eg. GBK);
                // that is fine for showing it in the console.
                let line_buf = String::from_utf8_lossy(&raw_line);
                if line_buf.starts_with('+') {
                    // Green for additions
                    stdout_lock
                        .write_all(line_buf.green().to_string().as_bytes())
                        .map_err(ClientError::StdoutWriteFailed)?;
                } else if line_buf.starts_with('-') {
                    // Red for deletions
                    stdout_lock
                        .write_all(line_buf.red().to_string().as_bytes())
                        .map_err(ClientError::StdoutWriteFailed)?;
                } else {
                    // Default style for context lines
                    stdout_lock
                        .write_all(line_buf.as_bytes())
                        .map_err(ClientError::StdoutWriteFailed)?;
                }
                raw_line.clear();
            }
            log::info!(
                "Patch file written to {}",
                auto_fix_patch_path.to_string_lossy().replace("\\", "/")
            );
        }
        Ok(())
    }
}

/// A closing tag for details blocks in markdown comments.
const CLOSER: &str = "\n</details>\n";

/// Room kept at the end of a length-limited comment for what is added without
/// a size check: the wrappers of the clang-tidy and clang-format sections and
/// their notes about findings that were left out.
const OMITTED_RESERVE: usize = 600;

/// The most files listed in one `clang-format -i` command of a comment.
const MAX_FILES_IN_COMMAND: usize = 50;

/// How clang-format ran in this check, to tell the user how to run it the same way.
#[derive(Debug, Clone, Copy)]
pub struct FormatRun<'a> {
    /// The `--style` given to clang-format.
    pub style: &'a str,
    /// Whether clang-format only checked some lines (see `--lines-changed-only`).
    ///
    /// Then a command to fix the formatting must not format the whole file;
    /// it passes the lines that need formatting with `--lines`.
    pub lines_only: bool,
}

impl<'a> From<&'a str> for FormatRun<'a> {
    /// clang-format checked whole files with the given `--style`.
    fn from(style: &'a str) -> Self {
        Self {
            style,
            lines_only: false,
        }
    }
}

impl FormatRun<'_> {
    /// The `clang-format` command(s) that fix the formatting of the given files in place:
    /// one command for all files, or one command per file with its `--lines`.
    fn command<'f>(
        &self,
        files: impl IntoIterator<Item = (&'f str, &'f [RangeInclusive<u32>])>,
    ) -> String {
        let style = if self.style.is_empty() {
            "file"
        } else {
            self.style
        };
        let base = format!(
            "clang-format -i {}",
            markdown::shell_quote(&format!("--style={style}"))
        );
        let mut whole_files = base.clone();
        let mut commands = vec![];
        for (file, lines) in files {
            if self.lines_only {
                let mut command = base.clone();
                for (start, end) in markdown::join_ranges(lines) {
                    command.push_str(&format!(" --lines={start}:{end}"));
                }
                command.push(' ');
                command.push_str(&markdown::shell_quote(file));
                commands.push(command);
            } else {
                whole_files.push(' ');
                whole_files.push_str(&markdown::shell_quote(file));
            }
        }
        if self.lines_only {
            commands.join("\n")
        } else {
            whole_files
        }
    }
}

/// What [`Findings::to_markdown`] needs to know besides the findings.
struct CommentContext<'a> {
    /// The count of failed checks (files not formatted plus clang-tidy notes).
    total: u64,
    /// The count of files that were checked.
    files_checked: u64,
    clang_versions: &'a ClangVersions,
    /// The longest comment allowed (in bytes).
    max_len: usize,
    /// Whether a patch file of fixes was written (see `fix-patch-path`).
    patch_exists: bool,
    /// How clang-format ran.
    format_run: FormatRun<'a>,
}

/// The findings about one file, for a thread comment or step summary.
struct FileFindings {
    name: String,
    /// clang-format failed to run on this file.
    format_failed: bool,
    /// The lines that clang-format would change.
    format_lines: Vec<RangeInclusive<u32>>,
    /// The clang-tidy notes about this file.
    tidy_notes: Vec<TidyNotification>,
}

impl FileFindings {
    /// The file name for a list item or a table: in a code span, if it can be.
    fn display_name(&self) -> String {
        if self.name.contains('`') {
            markdown::escape(&self.name)
        } else {
            format!("`{}`", self.name)
        }
    }
}

/// The findings about all files that have some.
struct Findings(Vec<FileFindings>);

impl Findings {
    fn collect(files: &[Arc<Mutex<FileObj>>]) -> Result<Self, ClientError> {
        let mut findings = vec![];
        for file in files {
            let file = file
                .lock()
                .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
            let format_advice = file
                .format_advice
                .as_ref()
                .filter(|advice| advice.is_concern());
            let tidy_notes = file
                .tidy_advice
                .as_ref()
                .map(|advice| {
                    advice
                        .notes
                        .iter()
                        .filter(|note| Path::new(&note.filename) == file.name.as_path())
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if format_advice.is_none() && tidy_notes.is_empty() {
                continue;
            }
            findings.push(FileFindings {
                name: file.name.to_string_lossy().replace('\\', "/"),
                format_failed: format_advice.is_some_and(|advice| advice.failed),
                format_lines: format_advice
                    .map(|advice| advice.replacements.clone())
                    .unwrap_or_default(),
                tidy_notes,
            });
        }
        Ok(Self(findings))
    }

    fn tidy_notes(&self) -> impl Iterator<Item = (&FileFindings, &TidyNotification)> {
        self.0
            .iter()
            .flat_map(|file| file.tidy_notes.iter().map(move |note| (file, note)))
    }

    /// Describe the clang-tidy notes in a few words: `1 error, 2 warnings · 1 fixable`.
    ///
    /// Also returns the status of the clang-tidy check.
    fn tidy_result(&self) -> (Status, String) {
        let (mut errors, mut warnings, mut others, mut fixable) = (0, 0, 0, 0);
        for (_, note) in self.tidy_notes() {
            match note.severity.as_str() {
                "error" => errors += 1,
                "warning" => warnings += 1,
                _ => others += 1,
            }
            if !note.fixed_lines.is_empty() {
                fixable += 1;
            }
        }
        let counts = [(errors, "error"), (warnings, "warning"), (others, "note")]
            .into_iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, noun)| plural(count, noun))
            .collect::<Vec<_>>();
        if counts.is_empty() {
            return (Status::Passed, String::from("no findings"));
        }
        let mut result = counts.join(", ");
        if fixable > 0 {
            result.push_str(&format!(" · {fixable} fixable"));
        }
        let status = if errors > 0 {
            Status::Failed
        } else {
            Status::Warned
        };
        (status, result)
    }

    /// Describe the clang-format findings in a few words: `1 file needs formatting`.
    ///
    /// Also returns the status of the clang-format check.
    fn format_result(&self) -> (Status, String) {
        let unformatted = self
            .0
            .iter()
            .filter(|file| !file.format_lines.is_empty())
            .count() as u64;
        let failed = self.0.iter().filter(|file| file.format_failed).count() as u64;
        let mut parts = vec![];
        if unformatted > 0 {
            parts.push(format!(
                "{} formatting",
                if unformatted == 1 {
                    String::from("1 file needs")
                } else {
                    format!("{unformatted} files need")
                }
            ));
        }
        if failed > 0 {
            parts.push(format!("clang-format failed on {}", plural(failed, "file")));
        }
        let status = if failed > 0 {
            Status::Failed
        } else if unformatted > 0 {
            Status::Warned
        } else {
            return (Status::Passed, String::from("all files formatted"));
        };
        (status, parts.join(", "))
    }

    fn to_markdown(&self, ctx: &CommentContext) -> String {
        let fits =
            |len: usize| len.saturating_add(USER_OUTREACH.len() + OMITTED_RESERVE) <= ctx.max_len;
        let mut comment = String::from(COMMENT_MARKER);
        let versions = [
            ("clang-format", ctx.clang_versions.format_version.as_ref()),
            ("clang-tidy", ctx.clang_versions.tidy_version.as_ref()),
        ];

        if ctx.total == 0 {
            comment.push_str(&format!(
                "## {} cpp-linter passed\n\n",
                Status::Passed.mark()
            ));
            let mut details = versions
                .iter()
                .filter_map(|(tool, version)| version.map(|ver| format!("{tool} {ver}")))
                .collect::<Vec<_>>();
            details.push(format!("{} checked", plural(ctx.files_checked, "file")));
            comment.push_str(&details.join(" · "));
            comment.push_str(USER_OUTREACH);
            return comment;
        }

        let (format_status, format_result) = self.format_result();
        let (tidy_status, tidy_result) = self.tidy_result();
        let status = if format_status == Status::Failed || tidy_status == Status::Failed {
            Status::Failed
        } else {
            Status::Warned
        };
        comment.push_str(&format!(
            "## {} cpp-linter found {} in {}\n\n| | Check | Result |\n|:-:|:--|:--|\n",
            status.mark(),
            plural(ctx.total, "issue"),
            plural(self.0.len() as u64, "file"),
        ));
        for ((tool, version), (row_status, result)) in versions.iter().zip([
            (format_status, format_result.as_str()),
            (tidy_status, tidy_result.as_str()),
        ]) {
            if let Some(version) = version {
                comment.push_str(&format!(
                    "| {} | **{tool}** {version} | {result} |\n",
                    row_status.small_mark()
                ));
            }
        }

        // The reserve covers each section's wrapper and its "not shown" note. Below that
        // (a limit barely larger than the heading), a section is left out entirely.
        let within = |len: usize| len.saturating_add(USER_OUTREACH.len()) <= ctx.max_len;
        let before = comment.len();
        self.push_tidy_section(&mut comment, &tidy_result, fits);
        if !within(comment.len()) {
            comment.truncate(before);
        }
        let before = comment.len();
        self.push_format_section(&mut comment, fits);
        if !within(comment.len()) {
            comment.truncate(before);
        }
        let how_to_fix = self.how_to_fix(ctx);
        if !how_to_fix.is_empty() && fits(comment.len() + how_to_fix.len()) {
            comment.push_str(&how_to_fix);
        }
        comment.push_str(USER_OUTREACH);
        comment
    }

    /// Append the clang-tidy notes to the `comment`: a table of them per file.
    fn push_tidy_section(&self, comment: &mut String, summary: &str, fits: impl Fn(usize) -> bool) {
        let count = self.tidy_notes().count();
        if count == 0 {
            return;
        }
        // A short list is open, so nobody has to click to see it.
        let open = if count <= 10 { " open" } else { "" };
        let mut section =
            format!("\n<details{open}><summary><b>clang-tidy</b> · {summary}</summary>\n");
        let mut omitted = 0;
        for file in &self.0 {
            let header = format!(
                "\n**{}**\n\n| Line | Finding |\n|--:|:--|\n",
                file.display_name()
            );
            let mut rows = String::new();
            for note in &file.tidy_notes {
                let severity = match note.severity.as_str() {
                    "warning" => String::new(),
                    other => format!("**{other}:** "),
                };
                let fixable = if note.fixed_lines.is_empty() {
                    ""
                } else {
                    " · ⚡ fixable"
                };
                let skipped = note
                    .skipped_fixes
                    .iter()
                    .map(|skipped| {
                        format!(
                            "<br>{} {}",
                            Status::Warned.small_mark(),
                            // a `|` ends the table cell, even in a code span
                            skipped.describe().replace('|', "\\|")
                        )
                    })
                    .collect::<String>();
                let row = format!(
                    "| {} | {severity}{}{fixable}<br>{}{skipped} |\n",
                    note.line,
                    markdown::escape(&note.rationale),
                    note.diagnostic_link(),
                );
                if omitted == 0
                    && fits(
                        comment.len()
                            + section.len()
                            + header.len()
                            + rows.len()
                            + row.len()
                            + CLOSER.len(),
                    )
                {
                    rows.push_str(&row);
                } else {
                    omitted += 1;
                }
            }
            if !rows.is_empty() {
                section.push_str(&header);
                section.push_str(&rows);
            }
        }
        if omitted > 0 {
            section.push_str(&Self::omitted_note(omitted));
        }
        section.push_str(CLOSER);
        comment.push_str(&section);
    }

    /// Append the files that clang-format would change to the `comment`.
    fn push_format_section(&self, comment: &mut String, fits: impl Fn(usize) -> bool) {
        let files = self
            .0
            .iter()
            .filter(|file| file.format_failed || !file.format_lines.is_empty())
            .collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }
        let mut section = format!(
            "\n<details><summary><b>clang-format</b> · {}</summary>\n\n",
            plural(files.len() as u64, "file")
        );
        let mut omitted = 0;
        for file in files {
            let item = if file.format_lines.is_empty() {
                format!(
                    "- {} · clang-format failed to run; see the logs\n",
                    file.display_name()
                )
            } else {
                format!(
                    "- {} · {}\n",
                    file.display_name(),
                    markdown::describe_lines(&file.format_lines)
                )
            };
            if omitted == 0 && fits(comment.len() + section.len() + item.len() + CLOSER.len()) {
                section.push_str(&item);
            } else {
                omitted += 1;
            }
        }
        if omitted > 0 {
            section.push_str(&Self::omitted_note(omitted));
        }
        section.push_str(CLOSER);
        comment.push_str(&section);
    }

    /// The sentence about findings that a length-limited comment left out.
    fn omitted_note(omitted: u64) -> String {
        format!(
            "\n{} not shown here because the comment would be too long. The workflow log lists every finding.\n",
            if omitted == 1 {
                String::from("1 more is")
            } else {
                format!("{omitted} more are")
            }
        )
    }

    /// The "How to fix" section: a command for clang-format, the patch of all fixes,
    /// and a prompt for an AI coding agent about the clang-tidy notes.
    ///
    /// Returns an empty string if there is nothing to say.
    fn how_to_fix(&self, ctx: &CommentContext) -> String {
        let mut parts = vec![];
        let unformatted = self
            .0
            .iter()
            .filter(|file| !file.format_lines.is_empty())
            .map(|file| (file.name.as_str(), file.format_lines.as_slice()))
            .collect::<Vec<_>>();
        if !unformatted.is_empty() {
            let with_version = ctx
                .clang_versions
                .format_version
                .as_ref()
                .map(|version| format!(" with clang-format {}", version.major))
                .unwrap_or_default();
            let scope = if ctx.format_run.lines_only {
                " It formats only the lines listed above."
            } else {
                ""
            };
            let mut part = format!(
                "**Formatting.** Run this in the repository root{with_version}.{scope}\n\n{}",
                markdown::fenced(
                    "bash",
                    &ctx.format_run
                        .command(unformatted.iter().take(MAX_FILES_IN_COMMAND).copied())
                )
            );
            if unformatted.len() > MAX_FILES_IN_COMMAND {
                part.push_str(&format!(
                    "\nThis covers the first {MAX_FILES_IN_COMMAND} of {} files.\n",
                    unformatted.len()
                ));
            }
            parts.push(part);
        }
        if ctx.patch_exists {
            parts.push(String::from(
                "**All fixes as one patch.** The `fix-patch-path` output of this step names a patch file \
                with every fix from clang-format and clang-tidy. Apply it with `git apply`.\n",
            ));
        }
        let notes = self.tidy_notes().collect::<Vec<_>>();
        if !notes.is_empty() {
            let mut prompt = String::from("Fix these clang-tidy findings:\n");
            for (file, note) in notes {
                prompt.push_str(&format!(
                    "- {}:{}:{}: {}: {} [{}]\n",
                    file.name,
                    note.line,
                    note.cols,
                    note.severity,
                    note.rationale.replace(['\r', '\n'], " "),
                    note.diagnostic
                ));
            }
            parts.push(format!(
                "**With an AI coding agent.** Paste this prompt:\n\n{}",
                markdown::fenced("text", &prompt)
            ));
        }
        if parts.is_empty() {
            return String::new();
        }
        let mut section = String::from("\n<details><summary><b>How to fix</b></summary>\n");
        for part in parts {
            section.push('\n');
            section.push_str(&part);
        }
        section.push_str(CLOSER);
        section
    }
}

#[cfg(all(test, feature = "bin"))]
mod test {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::{
        default::Default,
        env,
        io::Read,
        ops::RangeInclusive,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use regex::Regex;
    use semver::Version;
    use tempfile::{NamedTempFile, tempdir};

    use git_bot_feedback::{AnnotationLevel, FileAnnotation, FileFilter};

    use super::{
        FormatRun, RestClient, USER_OUTREACH, annotation_cap_message, normalize_changed_file_name,
        prefix_feedback_path, repo_root_prefix,
    };
    use crate::{
        clang_tools::{
            ClangVersions,
            clang_format::FormatAdvice,
            clang_tidy::{SkippedFix, TidyAdvice, TidyNotification},
        },
        cli::{FeedbackInput, ThreadComments},
        common_fs::FileObj,
        error::ClientError,
    };
    use clang_tools_manager::logger::try_init_logger;

    // ************************* tests for step-summary and output variables

    #[test]
    fn normalize_changed_files_for_nested_repo_root() {
        let tmp_dir = tempdir().unwrap();
        let git_root = tmp_dir.path().join("checkout");
        let repo_root = git_root.join("member");
        std::fs::create_dir_all(&repo_root).unwrap();
        std::fs::write(git_root.join(".git"), "").unwrap();

        let prefix = repo_root_prefix(&repo_root, Some(tmp_dir.path().to_path_buf()));
        assert_eq!(prefix, PathBuf::from("member"));
        assert_eq!(
            prefix_feedback_path("src/a.cpp", &prefix),
            "member/src/a.cpp"
        );

        let file_filter = FileFilter::new(&["build"], &["cpp"], None);
        assert_eq!(
            normalize_changed_file_name("member/src/a.cpp", &prefix, &file_filter),
            Some(PathBuf::from("src/a.cpp"))
        );
        assert_eq!(
            normalize_changed_file_name("member/build/a.cpp", &prefix, &file_filter),
            None
        );
        assert_eq!(
            normalize_changed_file_name("other/src/a.cpp", &prefix, &file_filter),
            None
        );
    }

    async fn create_comment(is_lgtm: bool) -> (String, String) {
        let tmp_dir = tempdir().unwrap();
        let repo_root = env::current_dir().unwrap();
        unsafe {
            // ensure we are mimicking a CI platform
            env::set_var("GITHUB_ACTIONS", "true");
            env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter-rs");
            env::set_var("GITHUB_SHA", "deadbeef123");
            env::set_var("GITHUB_WORKSPACE", &repo_root);
        }
        let mut rest_api_client = RestClient::new(&repo_root).unwrap();
        #[cfg(feature = "bin")]
        try_init_logger();
        if env::var("ACTIONS_STEP_DEBUG").is_ok_and(|var| var == "true") {
            // assert!(rest_api_client.debug_enabled);
            log::set_max_level(log::LevelFilter::Debug);
        }
        let mut files = vec![];
        if !is_lgtm {
            for _i in 0..u16::MAX {
                let filename = String::from("tests/demo/demo.cpp");
                let mut file = FileObj::new(PathBuf::from(&filename));
                let notes = vec![TidyNotification {
                    filename,
                    line: 0,
                    cols: 0,
                    severity: String::from("note"),
                    rationale: String::from("A test dummy rationale"),
                    diagnostic: String::from("clang-diagnostic-warning"),
                    suggestion: vec![],
                    fixed_lines: vec![],
                    skipped_fixes: vec![],
                }];
                file.tidy_advice = Some(TidyAdvice { notes });
                file.format_advice = Some(FormatAdvice {
                    #[allow(clippy::single_range_in_vec_init)]
                    replacements: vec![1..=2],
                    ..Default::default()
                });
                files.push(Arc::new(Mutex::new(file)));
            }
        }
        let feedback_inputs = FeedbackInput {
            style: if is_lgtm {
                String::new()
            } else {
                String::from("file")
            },
            step_summary: true,
            file_annotations: false,
            ..Default::default()
        };
        let mut step_summary_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        let mut gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        unsafe {
            env::set_var("GITHUB_STEP_SUMMARY", step_summary_path.path());
            env::set_var("GITHUB_OUTPUT", gh_out_path.path());
        }
        let clang_versions = ClangVersions {
            format_version: Some(Version::new(1, 2, 3)),
            tidy_version: Some(Version::new(1, 2, 3)),
        };
        rest_api_client
            .post_feedback(&files, feedback_inputs, clang_versions)
            .await
            .unwrap();
        let mut step_summary_content = String::new();
        step_summary_path
            .read_to_string(&mut step_summary_content)
            .unwrap();
        assert!(&step_summary_content.contains(USER_OUTREACH));
        let mut gh_out_content = String::new();
        gh_out_path.read_to_string(&mut gh_out_content).unwrap();
        assert!(gh_out_content.starts_with("checks-failed="));
        (step_summary_content, gh_out_content)
    }

    #[tokio::test]
    async fn check_comment_concerns() {
        let (comment, gh_out) = create_comment(false).await;
        assert!(comment.contains(" cpp-linter found "), "{comment}");
        assert!(comment.contains("<b>How to fix</b>"), "{comment}");
        let fmt_pattern = Regex::new(r"clang-format-checks-failed=(\d+)\n").unwrap();
        let tidy_pattern = Regex::new(r"clang-tidy-checks-failed=(\d+)\n").unwrap();
        for pattern in [fmt_pattern, tidy_pattern] {
            let number = pattern
                .captures(&gh_out)
                .expect("found no number of checks-failed")
                .get(1)
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();
            assert!(number > 0);
        }
    }

    #[tokio::test]
    async fn check_comment_lgtm() {
        unsafe {
            env::set_var("ACTIONS_STEP_DEBUG", "true");
        }
        let (comment, gh_out) = create_comment(true).await;
        assert!(comment.contains(" cpp-linter passed\n"), "{comment}");
        assert!(comment.contains(" files checked"), "{comment}");
        assert_eq!(
            gh_out,
            "checks-failed=0\nclang-format-checks-failed=0\nclang-tidy-checks-failed=0\n"
        );
    }

    // ************************* tests for other feedback paths

    fn make_dummy_files(severity: &str) -> Vec<Arc<Mutex<FileObj>>> {
        let filename = String::from("tests/demo/demo.cpp");
        let mut file = FileObj::new(PathBuf::from(&filename));
        file.tidy_advice = Some(TidyAdvice {
            notes: vec![TidyNotification {
                filename,
                line: 1,
                cols: 1,
                severity: severity.to_string(),
                rationale: String::from("A test dummy rationale"),
                diagnostic: String::from("clang-diagnostic-error"),
                suggestion: vec![String::from("int main() {}")],
                fixed_lines: vec![1],
                skipped_fixes: vec![],
            }],
        });
        file.format_advice = Some(FormatAdvice {
            #[allow(clippy::single_range_in_vec_init)]
            replacements: vec![1..=2],
            ..Default::default()
        });
        vec![Arc::new(Mutex::new(file))]
    }

    #[test]
    fn skipped_fix_with_pipe_stays_in_its_cell() {
        let files = make_dummy_files("warning");
        files[0].lock().unwrap().tidy_advice.as_mut().unwrap().notes[0]
            .skipped_fixes
            .push(SkippedFix {
                file: "a|b.h".to_string(),
                reason: "it is not one of the analyzed files".to_string(),
            });
        let comment =
            RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "file".into())
                .unwrap();
        assert!(comment.contains("`a\\|b.h`"), "{comment}");
    }

    #[test]
    fn tidy_comment_with_skipped_fixes() {
        let files = make_dummy_files("warning");
        {
            let mut file = files[0].lock().unwrap();
            let note = &mut file.tidy_advice.as_mut().unwrap().notes[0];
            note.skipped_fixes.push(SkippedFix {
                file: "tests/demo/demo.hpp".to_string(),
                reason: "it is not one of the analyzed files".to_string(),
            });
        }
        let comment =
            RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, true, "file".into())
                .unwrap();
        assert!(
            comment.contains(
                " · ⚡ fixable<br>clang-diagnostic-error<br><img alt=\"⚠\" width=\"16\" height=\"16\" \
                src=\"https://raw.githubusercontent.com/cpp-linter/.github/main/branding/status/warned.png\"> \
                Part of this fix was not \
                applied to `tests/demo/demo.hpp` because it is not one of the analyzed files. |\n"
            ),
            "{comment}"
        );
        assert!(comment.contains("**All fixes as one patch.**"), "{comment}");
    }

    fn dummy_versions() -> ClangVersions {
        ClangVersions {
            format_version: Some(Version::new(1, 2, 3)),
            tidy_version: Some(Version::new(1, 2, 3)),
        }
    }

    #[tokio::test]
    async fn annotations_over_cap_on_github() {
        let tmp_dir = tempdir().unwrap();
        let repo_root = env::current_dir().unwrap();
        let gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        unsafe {
            env::set_var("GITHUB_ACTIONS", "true");
            env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter-rs");
            env::set_var("GITHUB_SHA", "deadbeef123");
            env::set_var("GITHUB_WORKSPACE", &repo_root);
            env::set_var("GITHUB_OUTPUT", gh_out_path.path());
        }
        let mut rest_api_client = RestClient::new(&repo_root).unwrap();
        // 11 warnings: one more than GitHub shows in a step
        let files = (0..11)
            .flat_map(|_| make_dummy_files("warning"))
            .collect::<Vec<_>>();
        let feedback_inputs = FeedbackInput {
            file_annotations: true,
            ..Default::default()
        };
        let checks_failed = rest_api_client
            .post_feedback(&files, feedback_inputs, dummy_versions())
            .await
            .unwrap();
        assert_eq!(checks_failed, 22);
    }

    #[tokio::test]
    async fn thread_comment_without_summary() {
        unsafe {
            env::remove_var("GITHUB_ACTIONS");
        }
        let tmp_dir = tempdir().unwrap();
        let mut rest_api_client = RestClient::new(tmp_dir.path()).unwrap();
        for thread_comments in [ThreadComments::On, ThreadComments::Update] {
            let feedback_inputs = FeedbackInput {
                thread_comments,
                repo_root: tmp_dir.path().to_path_buf(),
                ..Default::default()
            };
            let checks_failed = rest_api_client
                .post_feedback(
                    &make_dummy_files("error"),
                    feedback_inputs,
                    dummy_versions(),
                )
                .await
                .unwrap();
            assert_eq!(checks_failed, 2);
        }
    }

    #[test]
    fn annotation_severity() {
        let annotations =
            RestClient::make_annotations(&make_dummy_files("error"), "llvm".into()).unwrap();
        assert!(
            annotations
                .iter()
                .any(|a| matches!(a.severity, AnnotationLevel::Error))
        );
        let annotations =
            RestClient::make_annotations(&make_dummy_files("note"), "llvm".into()).unwrap();
        // both the clang-format and clang-tidy annotations are notices
        assert_eq!(annotations.len(), 2);
        assert!(
            annotations
                .iter()
                .all(|a| matches!(a.severity, AnnotationLevel::Notice))
        );
    }

    #[test]
    fn tidy_annotation_points_to_line() {
        let annotations =
            RestClient::make_annotations(&make_dummy_files("warning"), "llvm".into()).unwrap();
        let tidy = annotations
            .iter()
            .find(|a| matches!(a.severity, AnnotationLevel::Warning))
            .unwrap();
        assert_eq!(tidy.start_line, Some(1));
        assert_eq!(tidy.start_column, Some(1));
        assert_eq!(tidy.title.as_deref(), Some("clang-diagnostic-error"));
        assert!(
            tidy.fmt_github()
                .starts_with("::warning file=tests/demo/demo.cpp,line=1,col=1,")
        );
    }

    #[test]
    fn short_annotation_titles() {
        let files = make_dummy_files("warning");
        let annotations = RestClient::build_annotations(&files, "llvm".into()).unwrap();
        let titles = annotations
            .iter()
            .map(|a| a.title.as_deref().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["clang-format", "clang-diagnostic-error"]);

        let file = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        let file = Arc::new(Mutex::new(file));
        file.lock().unwrap().format_advice = Some(FormatAdvice {
            failed: true,
            ..Default::default()
        });
        let annotations = RestClient::build_annotations(&[file], "llvm".into()).unwrap();
        assert_eq!(annotations[0].title.as_deref(), Some("clang-format"));
    }

    #[test]
    fn format_annotation_points_to_first_line() {
        let file = FileObj::new(PathBuf::from("src/demo.cpp"));
        let file = Arc::new(Mutex::new(file));
        file.lock().unwrap().format_advice = Some(FormatAdvice {
            replacements: vec![20..=20, 3..=5, 6..=11],
            ..Default::default()
        });
        let annotations =
            RestClient::build_annotations(std::slice::from_ref(&file), "file".into()).unwrap();
        assert_eq!(annotations[0].start_line, Some(3));
        assert_eq!(
            annotations[0].message,
            "Lines 3-11, 20 do not match the style in .clang-format. \
            Run: clang-format -i --style=file src/demo.cpp"
        );
        file.lock().unwrap().format_advice = Some(FormatAdvice {
            replacements: vec![RangeInclusive::new(4, 4)],
            ..Default::default()
        });
        let annotations =
            RestClient::build_annotations(&[file], "{BasedOnStyle: llvm}".into()).unwrap();
        assert_eq!(
            annotations[0].message,
            "Line 4 does not match the configured style. \
            Run: clang-format -i '--style={BasedOnStyle: llvm}' src/demo.cpp"
        );
    }

    /// When clang-format only checked the changed lines, the annotation sits on the
    /// first added line that needs formatting, and its command formats only those lines.
    #[test]
    fn format_annotation_for_changed_lines() {
        let mut file = FileObj::new(PathBuf::from("src/demo.cpp"));
        file.added_lines = vec![12, 13];
        file.format_advice = Some(FormatAdvice {
            replacements: vec![10..=12, 20..=21],
            ..Default::default()
        });
        let format_run = FormatRun {
            style: "file",
            lines_only: true,
        };
        let annotations =
            RestClient::build_annotations(&[Arc::new(Mutex::new(file))], format_run).unwrap();
        assert_eq!(annotations[0].start_line, Some(12));
        assert_eq!(
            annotations[0].message,
            "Lines 10-12, 20-21 do not match the style in .clang-format. \
            Run: clang-format -i --style=file --lines=10:12 --lines=20:21 src/demo.cpp"
        );
    }

    #[test]
    fn format_commands() {
        let lines = [1..=2, 3..=3];
        let files = [("a.cpp", &lines[..]), ("b c.cpp", &lines[..1])];
        let whole = FormatRun::from("llvm");
        assert_eq!(
            whole.command(files),
            "clang-format -i --style=llvm a.cpp 'b c.cpp'"
        );
        let scoped = FormatRun {
            style: "llvm",
            lines_only: true,
        };
        assert_eq!(
            scoped.command(files),
            "clang-format -i --style=llvm --lines=1:3 a.cpp\n\
            clang-format -i --style=llvm --lines=1:2 'b c.cpp'"
        );
    }

    #[test]
    fn format_failure_feedback() {
        let file = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        let file = Arc::new(Mutex::new(file));
        file.lock().unwrap().format_advice = Some(FormatAdvice {
            failed: true,
            ..Default::default()
        });
        let files = vec![file];
        let annotations = RestClient::make_annotations(&files, "llvm".into()).unwrap();
        assert_eq!(annotations.len(), 1);
        assert!(matches!(annotations[0].severity, AnnotationLevel::Warning));
        assert_eq!(annotations[0].title.as_deref(), Some("clang-format"));
        let comment =
            RestClient::make_comment(&files, 1, 0, &dummy_versions(), None, false, "llvm".into())
                .unwrap();
        assert!(
            comment.contains("- `tests/demo/demo.cpp` · clang-format failed to run; see the logs"),
            "{comment}"
        );
        assert!(comment.contains("/failed.png"), "{comment}");
        assert!(
            comment.contains(
                "/failed.png\"> | **clang-format** 1.2.3 | clang-format failed on 1 file |"
            ),
            "{comment}"
        );
        // nothing to format, so no "How to fix"
        assert!(!comment.contains("How to fix"), "{comment}");
    }

    #[test]
    fn comment_without_tidy_concerns() {
        let files = make_dummy_files("warning");
        files[0].lock().unwrap().tidy_advice = None;
        let comment =
            RestClient::make_comment(&files, 1, 0, &dummy_versions(), None, false, "llvm".into())
                .unwrap();
        assert!(
            comment.contains(" cpp-linter found 1 issue in 1 file\n"),
            "{comment}"
        );
        assert!(
            comment.contains("/warned.png\"> | **clang-format** 1.2.3 | 1 file needs formatting |"),
            "{comment}"
        );
        assert!(
            comment.contains("/passed.png\"> | **clang-tidy** 1.2.3 | no findings |"),
            "{comment}"
        );
        assert!(
            comment.contains("- `tests/demo/demo.cpp` · lines 1-2\n"),
            "{comment}"
        );
        assert!(!comment.contains("<b>clang-tidy</b>"), "{comment}");
        assert!(
            comment.contains("```bash\nclang-format -i --style=llvm tests/demo/demo.cpp\n```"),
            "{comment}"
        );
        assert!(!comment.contains("AI coding agent"), "{comment}");
    }

    #[tokio::test]
    async fn summary_output_file_errors() {
        unsafe {
            env::remove_var("GITHUB_ACTIONS");
        }
        let tmp_dir = tempdir().unwrap();
        let mut rest_api_client = RestClient::new(tmp_dir.path()).unwrap();

        // the parent "directory" is actually a file
        let not_a_dir = tmp_dir.path().join("not-a-dir");
        std::fs::write(&not_a_dir, "").unwrap();
        let feedback_inputs = FeedbackInput {
            summary_output_file: Some(not_a_dir.join("summary.md")),
            repo_root: tmp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let err = rest_api_client
            .post_feedback(
                &make_dummy_files("warning"),
                feedback_inputs,
                dummy_versions(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ClientError::MkDirFailed { .. }));

        // the output "file" is actually a directory
        let feedback_inputs = FeedbackInput {
            summary_output_file: Some(tmp_dir.path().to_path_buf()),
            repo_root: tmp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let err = rest_api_client
            .post_feedback(
                &make_dummy_files("warning"),
                feedback_inputs,
                dummy_versions(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ClientError::SummaryOutputFileWriteFailed { .. }
        ));

        // the output path has no parent (and is not a writable file)
        #[cfg(unix)]
        {
            let feedback_inputs = FeedbackInput {
                summary_output_file: Some(PathBuf::from("/")),
                repo_root: tmp_dir.path().to_path_buf(),
                ..Default::default()
            };
            let err = rest_api_client
                .post_feedback(
                    &make_dummy_files("warning"),
                    feedback_inputs,
                    dummy_versions(),
                )
                .await
                .unwrap_err();
            assert!(matches!(
                err,
                ClientError::SummaryOutputFileWriteFailed { .. }
            ));
        }
    }

    #[test]
    fn comment_with_poisoned_mutex() {
        let files = make_dummy_files("warning");
        crate::test_common::poison_mutex(&files[0]);
        // poisoned while making the clang-format comment
        assert!(
            RestClient::make_comment(&files, 1, 1, &dummy_versions(), None, false, "llvm".into())
                .is_err()
        );
        // poisoned while making the clang-tidy comment
        assert!(
            RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "llvm".into())
                .is_err()
        );
    }

    #[test]
    fn comment_too_long_for_tidy_note() {
        let files = make_dummy_files("error");
        let unlimited =
            RestClient::make_comment(&files, 1, 1, &dummy_versions(), None, false, "llvm".into())
                .unwrap();
        assert!(unlimited.contains("| 1 | **error:** A test dummy rationale"));
        assert!(unlimited.contains("/failed.png"), "{unlimited}");
        assert!(unlimited.contains("How to fix"));
        // No room for "How to fix", but the clang-tidy note still fits.
        let how_to_fix = unlimited.find("\n<details><summary><b>How to fix").unwrap();
        let limited = RestClient::make_comment(
            &files,
            1,
            1,
            &dummy_versions(),
            Some((how_to_fix + USER_OUTREACH.len() + super::OMITTED_RESERVE) as u64),
            false,
            "llvm".into(),
        )
        .unwrap();
        assert!(limited.contains("A test dummy rationale"));
        assert!(!limited.contains("How to fix"));
        // No room for the clang-tidy note and the clang-format list.
        let head = unlimited.find("\n<details").unwrap();
        let limited = RestClient::make_comment(
            &files,
            1,
            1,
            &dummy_versions(),
            Some((head + USER_OUTREACH.len() + super::OMITTED_RESERVE + 100) as u64),
            false,
            "llvm".into(),
        )
        .unwrap();
        assert!(!limited.contains("A test dummy rationale"), "{limited}");
        assert!(
            limited.contains("1 more is not shown here because the comment would be too long."),
            "{limited}"
        );
        assert!(limited.ends_with(USER_OUTREACH));
    }

    /// A length-limited comment never grows past its limit, even when both
    /// sections have to leave out their findings.
    #[test]
    fn comment_stays_within_its_limit() {
        let files = make_dummy_files("warning");
        {
            let mut file = files[0].lock().unwrap();
            let notes = &mut file.tidy_advice.as_mut().unwrap().notes;
            let note = notes[0].clone();
            notes.extend(std::iter::repeat_n(note, 50));
        }
        let unlimited =
            RestClient::make_comment(&files, 1, 51, &dummy_versions(), None, true, "llvm".into())
                .unwrap();
        let head = unlimited.find("\n<details").unwrap();
        // From a limit that only fits the heading, the table and the footer.
        for max_len in (head + USER_OUTREACH.len())..unlimited.len() {
            let limited = RestClient::make_comment(
                &files,
                1,
                51,
                &dummy_versions(),
                Some(max_len as u64),
                true,
                "llvm".into(),
            )
            .unwrap();
            assert!(limited.len() <= max_len, "{} > {max_len}", limited.len());
        }
    }

    // ************************* tests for GitHub's annotation limits

    fn annotations(warnings: usize, notices: usize) -> Vec<FileAnnotation> {
        let make = |severity| FileAnnotation {
            severity,
            path: "demo.cpp".to_string(),
            message: "a dummy message".to_string(),
            ..Default::default()
        };
        let mut result = vec![make(AnnotationLevel::Warning); warnings];
        result.extend(vec![make(AnnotationLevel::Notice); notices]);
        result
    }

    #[test]
    fn annotations_within_limits() {
        let inputs = FeedbackInput::default();
        assert!(annotation_cap_message(&[], &inputs).is_none());
        assert!(annotation_cap_message(&annotations(10, 10), &inputs).is_none());
        let msg = annotation_cap_message(&annotations(11, 0), &inputs).unwrap();
        assert!(msg.ends_with("to get a list of every finding."), "{msg}");
    }

    #[test]
    fn annotations_over_step_limit() {
        let inputs = FeedbackInput {
            step_summary: true,
            thread_comments: ThreadComments::Off,
            ..Default::default()
        };
        let msg = annotation_cap_message(&annotations(20, 1), &inputs).unwrap();
        assert!(
            msg.contains("so 10 of 20 warning annotations are not shown."),
            "{msg}"
        );
        assert!(
            msg.ends_with("The step summary lists every finding."),
            "{msg}"
        );

        let inputs = FeedbackInput {
            step_summary: false,
            thread_comments: ThreadComments::Update,
            ..Default::default()
        };
        let msg = annotation_cap_message(&annotations(11, 12), &inputs).unwrap();
        assert!(
            msg.contains(
                "so 1 of 11 warning annotations and 2 of 12 notice annotations are not shown."
            ),
            "{msg}"
        );
        assert!(
            msg.ends_with(
                "The thread comment lists them, and the workflow log lists every finding."
            ),
            "{msg}"
        );

        let inputs = FeedbackInput {
            step_summary: true,
            thread_comments: ThreadComments::On,
            ..Default::default()
        };
        let msg = annotation_cap_message(&annotations(0, 11), &inputs).unwrap();
        assert!(
            msg.ends_with("The step summary and the thread comment list every finding."),
            "{msg}"
        );
    }
}
