//! The file annotations for clang-format and clang-tidy findings.

use std::sync::{Arc, Mutex};

use git_bot_feedback::{AnnotationLevel, FileAnnotation};

use super::{FormatRun, RestClient};
use crate::{
    clang_tools::clang_format::describe_style,
    cli::{FeedbackInput, ThreadComments},
    common_fs::FileObj,
    error::ClientError,
    markdown,
};

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
pub(super) fn annotation_cap_message(
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

impl RestClient {
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
    pub(super) fn build_annotations(
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
                            format_run.command([(
                                format_run.file_name(&name).as_str(),
                                format_advice.replacements.as_slice()
                            )]),
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
}
