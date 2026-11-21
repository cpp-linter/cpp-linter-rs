//! Posting the feedback of a check.

use std::{
    env,
    io::{BufRead, Write, stderr, stdout},
    sync::{Arc, Mutex},
};

use colored::Colorize;
use git_bot_feedback::{
    CommentKind, CommentPolicy, OutputVariable, ReviewAction, ReviewOptions, ThreadCommentOptions,
};

use super::{
    COMMENT_MARKER, FormatRun, RestClient, annotations::annotation_cap_message, lock_file,
    paths::prefix_feedback_path,
};
use crate::{
    clang_tools::{
        ClangVersions, ReviewComments, clang_format::tally_format_advice,
        clang_tidy::tally_tidy_advice,
    },
    cli::{FeedbackInput, ThreadComments},
    code_quality::CodeQualityReport,
    common_fs::{FileObj, is_stdout},
    error::ClientError,
    sarif::SarifLog,
};

impl RestClient {
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
        let format_run = FormatRun {
            path_prefix: &self.repo_root_prefix,
            ..feedback_inputs.format_run()
        };
        let format_checks_failed =
            tally_format_advice(files).map_err(ClientError::MutexPoisoned)?;
        let mut comment = None;

        if feedback_inputs.file_annotations {
            let mut annotations = Self::build_annotations(files, format_run)?;
            for annotation in &mut annotations {
                annotation.path = prefix_feedback_path(&annotation.path, &self.repo_root_prefix);
            }
            if self.client.client_kind() == "local" && feedback_inputs.report_to_stdout() {
                // The local client prints annotations to stdout,
                // which must hold only the report.
                let mut err = stderr().lock();
                for annotation in &annotations {
                    writeln!(err, "{}", annotation.fmt_local())
                        .map_err(ClientError::StdoutWriteFailed)?;
                }
            } else {
                self.client.write_file_annotations(&annotations)?;
            }
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

        // Write the patch first, so the reports can point to it.
        let mut review = None;
        if self.client.is_pr_event() && feedback_inputs.pr_review {
            let summary_only = ["true", "on", "1"].contains(
                &env::var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY")
                    .unwrap_or("false".to_string())
                    .to_lowercase()
                    .as_str(),
            );
            let mut review_comments = ReviewComments::default();
            for file in files {
                let file = lock_file(file)?;
                file.make_suggestions_from_patch(
                    &mut review_comments,
                    summary_only,
                    &feedback_inputs.repo_root,
                    &feedback_inputs.patch_path,
                )?;
            }
            self.show_patch(&feedback_inputs)?;
            review = Some((review_comments, summary_only));
        } else if feedback_inputs.gen_patch {
            for file in files {
                let file = lock_file(file)?;
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
                format_run,
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

        // Post the review after the summary, so a failed review does not lose the summary.
        if let Some((review_comments, summary_only)) = review {
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
                    || review_comments.has_errors
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
                    format_run,
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
