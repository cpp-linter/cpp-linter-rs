use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use git_bot_feedback::{
    AnnotationLevel, CommentKind, CommentPolicy, FileAnnotation, FileFilter, LinesChangedOnly,
    OutputVariable, RestApiClient, ReviewAction, ReviewOptions, ThreadCommentOptions,
    client::init_client,
};

use crate::{
    clang_tools::{
        ClangVersions, ReviewComments,
        clang_format::{summarize_style, tally_format_advice},
        clang_tidy::tally_tidy_advice,
    },
    cli::{FeedbackInput, ThreadComments},
    common_fs::FileObj,
    error::ClientError,
};

/// The comment marker used to identify bot comments from other comments (from users or other bots).
pub const COMMENT_MARKER: &str = "<!-- cpp linter action -->\n";

/// The UserAgent header value used in HTTP requests.
pub const USER_AGENT: &str = concat!("cpp-linter/", env!("CARGO_PKG_VERSION"),);

/// The user outreach message displayed in bot comments.
pub const USER_OUTREACH: &str = concat!(
    "\n\nHave any feedback or feature suggestions? [Share it here.]",
    "(https://github.com/cpp-linter/cpp-linter-action/issues)"
);

pub struct RestClient {
    client: Box<dyn RestApiClient + Sync + Send>,
}

impl RestClient {
    pub fn new() -> Result<Self, ClientError> {
        let mut client = init_client()?;
        client.set_user_agent(USER_AGENT)?;
        Ok(Self { client })
    }

    pub fn is_pr(&self) -> bool {
        self.client.is_pr_event()
    }

    pub async fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
        lines_changed_only: &LinesChangedOnly,
        base_diff: &Option<String>,
        ignore_index: bool,
    ) -> Result<Vec<FileObj>, ClientError> {
        let files = self
            .client
            .get_list_of_changed_files(
                file_filter,
                lines_changed_only,
                base_diff.to_owned(),
                ignore_index,
            )
            .await?;
        Ok(files
            .iter()
            .map(|(file_name, diff_lines)| {
                let diff_chunks = diff_lines
                    .diff_hunks
                    .iter()
                    .map(|hunk| hunk.start..=hunk.end)
                    .collect();
                FileObj::from(
                    PathBuf::from(&file_name),
                    diff_lines.added_lines.clone(),
                    diff_chunks,
                )
            })
            .collect())
    }

    pub fn start_log_group(&self, name: &str) {
        self.client.start_log_group(name)
    }

    pub fn end_log_group(&self, name: &str) {
        self.client.end_log_group(name)
    }

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
            let annotations = Self::make_annotations(files, &feedback_inputs.style)?;
            self.client.write_file_annotations(&annotations)?;
        }
        if feedback_inputs.step_summary {
            comment = Some(Self::make_comment(
                files,
                format_checks_failed,
                tidy_checks_failed,
                &clang_versions,
                None,
            ));
            self.client.append_step_summary(comment.as_ref().unwrap())?;
        }
        let output_vars = [
            OutputVariable {
                name: "checks-failed".to_string(),
                value: format!("{}", format_checks_failed + tidy_checks_failed),
            },
            OutputVariable {
                name: "format-checks-failed".to_string(),
                value: format_checks_failed.to_string(),
            },
            OutputVariable {
                name: "tidy-checks-failed".to_string(),
                value: tidy_checks_failed.to_string(),
            },
        ];
        self.client.write_output_variables(&output_vars)?;

        if feedback_inputs.thread_comments != ThreadComments::Off {
            // post thread comment for PR or push event
            if comment.as_ref().is_none_or(|c| c.len() > 65535) {
                comment = Some(Self::make_comment(
                    files,
                    format_checks_failed,
                    tidy_checks_failed,
                    &clang_versions,
                    Some(65535),
                ));
            }
            let options = ThreadCommentOptions {
                policy: match feedback_inputs.thread_comments {
                    ThreadComments::Update => CommentPolicy::Update,
                    ThreadComments::On => CommentPolicy::Anew,
                    ThreadComments::Off => unreachable!(),
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
        if self.client.is_pr_event()
            && (feedback_inputs.tidy_review || feedback_inputs.format_review)
        {
            let summary_only = ["true", "on", "1"].contains(
                &env::var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY")
                    .unwrap_or("false".to_string())
                    .as_str(),
            );
            let mut review_comments = ReviewComments::default();
            for file in files {
                let file = file
                    .lock()
                    .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
                file.make_suggestions_from_patch(&mut review_comments, summary_only)?;
            }

            let mut options = ReviewOptions {
                marker: COMMENT_MARKER.to_string(),
                comments: {
                    let mut comments = vec![];
                    for suggestion in &review_comments.comments {
                        comments.push(suggestion.as_review_comment());
                    }
                    comments
                },
                ..Default::default()
            };

            self.client.cull_pr_reviews(&mut options).await?;
            let has_changes = review_comments.full_patch.iter().any(|p| !p.is_empty());
            options.action = if feedback_inputs.passive_reviews {
                ReviewAction::Comment
            } else {
                if options.comments.is_empty() && !has_changes {
                    ReviewAction::Approve
                } else {
                    ReviewAction::RequestChanges
                }
            };
            options.summary = review_comments.summarize(&clang_versions, &options.comments);
            self.client.post_pr_review(&options).await?;
        }
        Ok(format_checks_failed + tidy_checks_failed)
    }

    /// Post file annotations.
    pub fn make_annotations(
        files: &[Arc<Mutex<FileObj>>],
        style: &str,
    ) -> Result<Vec<FileAnnotation>, ClientError> {
        let style_guide = summarize_style(style);
        let mut annotations = vec![];

        // iterate over clang-format advice and post annotations
        for file in files {
            let file = file
                .lock()
                .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
            if let Some(format_advice) = &file.format_advice {
                // assemble a list of line numbers
                let mut lines = Vec::new();
                for replacement in &format_advice.replacements {
                    if !lines.contains(&replacement.line) {
                        lines.push(replacement.line);
                    }
                }
                // post annotation if any applicable lines were formatted
                if !lines.is_empty() {
                    let name = file.name.to_string_lossy().replace('\\', "/");
                    let title = format!("Run clang-format on {name}");
                    let message = format!(
                        "File {name} does not conform to {style_guide} style guidelines. (lines {line_set})",
                        line_set = lines
                            .iter()
                            .map(|val| val.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                    let annotation = FileAnnotation {
                        severity: AnnotationLevel::Notice,
                        path: name,
                        start_line: None,
                        end_line: None,
                        start_column: None,
                        end_column: None,
                        title: Some(title),
                        message,
                    };
                    annotations.push(annotation);
                }
            } // end format_advice iterations

            // iterate over clang-tidy advice and post annotations
            // The tidy_advice vector is parallel to the files vector; meaning it serves as a file filterer.
            // lines are already filter as specified to clang-tidy CLI.
            if let Some(tidy_advice) = &file.tidy_advice {
                for note in &tidy_advice.notes {
                    let path = file.name.to_string_lossy().replace('\\', "/");
                    if note.filename == path {
                        let title = format!("{}:{}:{}", note.filename, note.line, note.cols);
                        let annotation = FileAnnotation {
                            severity: match note.severity.as_str() {
                                "note" => AnnotationLevel::Notice,
                                "warning" => AnnotationLevel::Warning,
                                "error" => AnnotationLevel::Error,
                                _ => AnnotationLevel::Notice, // default to notice if severity is unrecognized
                            },
                            path,
                            start_line: None,
                            end_line: Some(note.line as usize),
                            start_column: None,
                            end_column: Some(note.cols as usize),
                            title: Some(title),
                            message: note.rationale.clone(),
                        };
                        annotations.push(annotation);
                    }
                }
            }
        }
        Ok(annotations)
    }

    /// Makes a comment in MarkDown syntax based on the concerns in `format_advice` and
    /// `tidy_advice` about the given set of `files`.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    ///
    /// Returns the markdown comment as a string as well as the total count of
    /// `format_checks_failed` and `tidy_checks_failed` (in respective order).
    fn make_comment(
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
        clang_versions: &ClangVersions,
        max_len: Option<u64>,
    ) -> String {
        let mut comment = format!("{COMMENT_MARKER}# Cpp-Linter Report ");
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
                    // tidy_version should be `Some()` value at this point.
                    &clang_versions.tidy_version.as_ref().unwrap().to_string(),
                    &mut remaining_length,
                );
            }
            if tidy_checks_failed > 0 {
                make_tidy_comment(
                    files,
                    &mut comment,
                    tidy_checks_failed,
                    // format_version should be `Some()` value at this point.
                    &clang_versions.format_version.as_ref().unwrap().to_string(),
                    &mut remaining_length,
                );
            }
        } else {
            comment.push_str(":heavy_check_mark:\nNo problems need attention.");
        }
        comment.push_str(USER_OUTREACH);
        comment
    }
}

fn make_format_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    format_checks_failed: u64,
    version_used: &String,
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-format (v{version_used}) reports: <strong>{format_checks_failed} file(s) not formatted</strong></summary>\n\n",
    );
    let closer = String::from("\n</details>");
    let mut format_comment = String::new();
    *remaining_length -= opener.len() as u64 + closer.len() as u64;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(format_advice) = &file.format_advice
            && !format_advice.replacements.is_empty()
            && *remaining_length > 0
        {
            let note = format!("- {}\n", file.name.to_string_lossy().replace('\\', "/"));
            if (note.len() as u64) < *remaining_length {
                format_comment.push_str(&note.to_string());
                *remaining_length -= note.len() as u64;
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
    version_used: &String,
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-tidy (v{version_used}) reports: {tidy_checks_failed}<strong> concern(s)</strong></summary>\n\n"
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
                                ext = file_path.extension().unwrap_or_default().to_string_lossy(),
                                suggestion = tidy_note.suggestion.join("\n   "),
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

#[cfg(all(test, feature = "bin"))]
mod test {
    use std::{
        default::Default,
        env,
        io::Read,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    use regex::Regex;
    use semver::Version;
    use tempfile::{NamedTempFile, tempdir};

    use super::{RestClient, USER_OUTREACH};
    use crate::{
        clang_tools::{
            ClangVersions,
            clang_format::{FormatAdvice, Replacement},
            clang_tidy::{TidyAdvice, TidyNotification},
        },
        cli::FeedbackInput,
        common_fs::FileObj,
        logger,
    };

    // ************************* tests for step-summary and output variables

    async fn create_comment(
        is_lgtm: bool,
        fail_gh_out: bool,
        fail_summary: bool,
    ) -> (String, String) {
        let tmp_dir = tempdir().unwrap();
        unsafe {
            // ensure we are mimicking a CI platform
            env::set_var("GITHUB_ACTIONS", "true");
            env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter-rs");
            env::set_var("GITHUB_SHA", "deadbeef123");
        }
        let mut rest_api_client = RestClient::new().unwrap();
        logger::try_init();
        if env::var("ACTIONS_STEP_DEBUG").is_ok_and(|var| var == "true") {
            // assert!(rest_api_client.debug_enabled);
            log::set_max_level(log::LevelFilter::Debug);
        }
        let mut files = vec![];
        if !is_lgtm {
            for _i in 0..65535 {
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
                }];
                file.tidy_advice = Some(TidyAdvice {
                    notes,
                    patched: None,
                });
                file.format_advice = Some(FormatAdvice {
                    replacements: vec![Replacement { offset: 0, line: 1 }],
                    patched: None,
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
            ..Default::default()
        };
        let mut step_summary_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        let mut gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        unsafe {
            env::set_var(
                "GITHUB_STEP_SUMMARY",
                if fail_summary {
                    Path::new("not-a-file.txt")
                } else {
                    step_summary_path.path()
                },
            );
            env::set_var(
                "GITHUB_OUTPUT",
                if fail_gh_out {
                    Path::new("not-a-file.txt")
                } else {
                    gh_out_path.path()
                },
            );
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
        if !fail_summary {
            assert!(&step_summary_content.contains(USER_OUTREACH));
        }
        let mut gh_out_content = String::new();
        gh_out_path.read_to_string(&mut gh_out_content).unwrap();
        if !fail_gh_out {
            assert!(gh_out_content.starts_with("checks-failed="));
        }
        (step_summary_content, gh_out_content)
    }

    #[tokio::test]
    async fn check_comment_concerns() {
        let (comment, gh_out) = create_comment(false, false, false).await;
        assert!(&comment.contains(":warning:\nSome files did not pass the configured checks!\n"));
        let fmt_pattern = Regex::new(r"format-checks-failed=(\d+)\n").unwrap();
        let tidy_pattern = Regex::new(r"tidy-checks-failed=(\d+)\n").unwrap();
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
        let (comment, gh_out) = create_comment(true, false, false).await;
        assert!(comment.contains(":heavy_check_mark:\nNo problems need attention."));
        assert_eq!(
            gh_out,
            "checks-failed=0\nformat-checks-failed=0\ntidy-checks-failed=0\n"
        );
    }
}
