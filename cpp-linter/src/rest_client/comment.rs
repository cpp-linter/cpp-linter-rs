//! The comment for a thread and a step summary.

use std::{
    ops::RangeInclusive,
    path::Path,
    sync::{Arc, Mutex},
};

use super::{COMMENT_MARKER, FormatRun, RestClient, USER_OUTREACH, lock_file};
use crate::{
    clang_tools::{ClangVersions, clang_tidy::TidyNotification},
    common_fs::FileObj,
    error::ClientError,
    markdown::{self, Status, plural},
};

/// A closing tag for details blocks in markdown comments.
const CLOSER: &str = "\n</details>\n";

/// Room kept at the end of a length-limited comment for what is added without
/// a size check: the wrappers of the clang-tidy and clang-format sections and
/// their notes about findings that were left out.
pub(super) const OMITTED_RESERVE: usize = 600;

/// The most files listed in one `clang-format -i` command of a comment.
const MAX_FILES_IN_COMMAND: usize = 50;

/// The most clang-tidy findings listed in the prompt for an AI coding agent.
const MAX_NOTES_IN_PROMPT: usize = 50;

impl RestClient {
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
    pub(super) fn make_comment(
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
        clang_versions: &ClangVersions,
        max_len: Option<u64>,
        patch_exists: bool,
        format_run: FormatRun<'_>,
    ) -> Result<String, ClientError> {
        let findings = Findings::collect(files, format_run)?;
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
pub(super) struct FileFindings {
    pub(super) name: String,
    /// clang-format failed to run on this file.
    pub(super) format_failed: bool,
    /// The lines that clang-format would change.
    pub(super) format_lines: Vec<RangeInclusive<u32>>,
    /// The clang-tidy notes about this file.
    pub(super) tidy_notes: Vec<TidyNotification>,
}

impl FileFindings {
    /// The file name for a list item or a table: in a code span, if it can be.
    pub(super) fn display_name(&self) -> String {
        if self.name.contains('`') {
            markdown::escape(&self.name)
        } else {
            format!("`{}`", self.name)
        }
    }
}

/// The findings about all files that have some.
pub(super) struct Findings(pub(super) Vec<FileFindings>);

impl Findings {
    fn collect(files: &[Arc<Mutex<FileObj>>], format_run: FormatRun) -> Result<Self, ClientError> {
        let mut findings = vec![];
        for file in files {
            let file = lock_file(file)?;
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
                name: format_run.file_name(&file.name.to_string_lossy()),
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

    /// Describe the clang-tidy notes in a few words: `1 error, 2 warnings, 1 fixable`.
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
            result.push_str(&format!(", {fixable} fixable"));
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
    pub(super) fn format_result(&self) -> (Status, String) {
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
            comment.push_str(&details.join(", "));
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
            "## {} cpp-linter found {} in {}\n\n| Tool | Result |\n|:--|:--|\n",
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
                    "| {} **{tool}** {version} | {result} |\n",
                    row_status.small_mark()
                ));
            }
        }

        // "How to fix" matters most when there are many findings, so room for it is kept
        // before the findings are listed (if it fits at all).
        let how_to_fix = self.how_to_fix(ctx);
        let how_to_fix = if fits(comment.len() + how_to_fix.len()) {
            how_to_fix
        } else {
            String::new()
        };
        let fits_findings = |len: usize| fits(len + how_to_fix.len());
        // The reserve covers each section's wrapper and its "not shown" note. Below that
        // (a limit barely larger than the heading), a section is left out entirely.
        let within =
            |len: usize| len.saturating_add(how_to_fix.len() + USER_OUTREACH.len()) <= ctx.max_len;
        let before = comment.len();
        self.push_tidy_section(&mut comment, &tidy_result, fits_findings);
        if !within(comment.len()) {
            comment.truncate(before);
        }
        let before = comment.len();
        self.push_format_section(&mut comment, fits_findings);
        if !within(comment.len()) {
            comment.truncate(before);
        }
        comment.push_str(&how_to_fix);
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
            format!("\n<details{open}><summary><b>clang-tidy</b>: {summary}</summary>\n");
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
                    " (fixable)"
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
    pub(super) fn push_format_section(&self, comment: &mut String, fits: impl Fn(usize) -> bool) {
        let files = self
            .0
            .iter()
            .filter(|file| file.format_failed || !file.format_lines.is_empty())
            .collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }
        let mut section = format!(
            "\n<details><summary><b>clang-format</b>: {}</summary>\n\n",
            plural(files.len() as u64, "file")
        );
        let mut omitted = 0;
        for file in files {
            let item = if file.format_lines.is_empty() {
                format!(
                    "- {}: clang-format failed to run; see the logs\n",
                    file.display_name()
                )
            } else {
                format!(
                    "- {}, {}\n",
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
                "the listed lines"
            } else {
                "these files"
            };
            let mut part = format!(
                "From the repository root, format {scope}{with_version}:\n\n{}",
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
            // The patch also has the formatting, so it is another way to do the step above.
            let lead = if parts.is_empty() {
                "Apply"
            } else {
                "Or apply"
            };
            parts.push(format!(
                "{lead} every fix at once: this step's `fix-patch-path` output is a patch for `git apply`.\n",
            ));
        }
        let notes = self.tidy_notes().collect::<Vec<_>>();
        if !notes.is_empty() {
            let mut prompt = String::from("Fix these clang-tidy findings:\n");
            for (file, note) in notes.iter().take(MAX_NOTES_IN_PROMPT) {
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
            let mut part = format!(
                "For the clang-tidy findings, give this to your coding agent:\n\n{}",
                markdown::fenced("text", &prompt)
            );
            if notes.len() > MAX_NOTES_IN_PROMPT {
                part.push_str(&format!(
                    "\nThis covers the first {MAX_NOTES_IN_PROMPT} of {} findings. \
                    The workflow log lists every finding.\n",
                    notes.len()
                ));
            }
            parts.push(part);
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
