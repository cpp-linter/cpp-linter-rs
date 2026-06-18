//! A module to hold all common file system functionality.

use std::{
    fmt::Debug,
    fs,
    num::NonZeroU32,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use gix_imara_diff::{
    BasicLineDiffPrinter, Diff, Hunk, InternedInput, UnifiedDiffConfig, UnifiedDiffPrinter,
};

use crate::{
    clang_tools::{
        ReviewComments, Suggestion, clang_format::FormatAdvice, clang_tidy::TidyAdvice, make_patch,
    },
    cli::LinesChangedOnly,
    error::FileObjError,
};

/// A structure to represent a file's path and line changes.
#[derive(Debug, Clone)]
pub struct FileObj {
    /// The path to the file.
    pub name: PathBuf,

    /// The list of lines with additions.
    pub added_lines: Vec<u32>,

    /// The list of ranges that span only lines with additions.
    pub added_ranges: Vec<RangeInclusive<u32>>,

    /// The list of ranges that span the lines present in diff chunks.
    pub diff_chunks: Vec<RangeInclusive<u32>>,

    /// The collection of clang-format advice for this file.
    pub format_advice: Option<FormatAdvice>,

    /// The collection of clang-format advice for this file.
    pub tidy_advice: Option<TidyAdvice>,

    /// A path to a cached file with all/any patches applied.
    pub(crate) patched_path: Option<PathBuf>,
}

impl FileObj {
    /// Instantiate a rudimentary object with only file name information.
    ///
    /// To instantiate an object with line information, use [`FileObj::from`].
    pub fn new(name: PathBuf) -> Self {
        FileObj {
            name,
            added_lines: Vec::<u32>::new(),
            added_ranges: Vec::<RangeInclusive<u32>>::new(),
            diff_chunks: Vec::<RangeInclusive<u32>>::new(),
            format_advice: None,
            tidy_advice: None,
            patched_path: None,
        }
    }

    /// Instantiate an object with file name and changed lines information.
    pub fn from(
        name: PathBuf,
        added_lines: Vec<u32>,
        diff_chunks: Vec<RangeInclusive<u32>>,
    ) -> Self {
        // filter out any line numbers that are 0 since line numbers are always 1-indexed in diffs
        let added_lines: Vec<NonZeroU32> = added_lines
            .into_iter()
            .filter_map(NonZeroU32::new)
            .collect();
        let added_ranges = FileObj::consolidate_numbers_to_ranges(&added_lines);
        FileObj {
            name,
            added_lines: added_lines.into_iter().map(|v| v.get()).collect(),
            added_ranges,
            diff_chunks,
            format_advice: None,
            tidy_advice: None,
            patched_path: None,
        }
    }

    /// A helper function to consolidate a [Vec<u32>] of line numbers into a
    /// [Vec<RangeInclusive<u32>>] in which each range describes the beginning and
    /// ending of a group of consecutive line numbers.
    fn consolidate_numbers_to_ranges(lines: &[NonZeroU32]) -> Vec<RangeInclusive<u32>> {
        let mut ranges: Vec<RangeInclusive<u32>> = Vec::new();
        let mut line_iter = lines.iter().enumerate();
        let mut range_start = match line_iter.next() {
            Some((_, number)) => number.get(),
            None => return ranges, // return empty vector if no lines
        };
        // lines.len() cannot be 0 at this point
        let last_index = lines.len() - 1;
        if last_index == 0 {
            // Single element case: push range and return
            ranges.push(RangeInclusive::new(range_start, range_start));
            return ranges;
        }
        for (index, number) in line_iter {
            // use let chain to avoid repeated lookup of lines[index - 1].
            // should always yield some value since we entered the for loop at index 1.
            if let Some(prev_line) = lines.get(index - 1)
                && number.get() - 1 != prev_line.get()
            {
                ranges.push(RangeInclusive::new(range_start, prev_line.get()));
                range_start = number.get();
            }
            if index == last_index {
                ranges.push(RangeInclusive::new(range_start, number.get()));
            }
        }
        ranges
    }

    /// Get the list of line ranges to consider based on the given
    /// [`LinesChangedOnly`] configuration.
    pub fn get_ranges(&self, lines_changed_only: &LinesChangedOnly) -> Vec<RangeInclusive<u32>> {
        match lines_changed_only {
            LinesChangedOnly::Diff => self.diff_chunks.to_vec(),
            LinesChangedOnly::On => self.added_ranges.to_vec(),
            _ => Vec::new(),
        }
    }

    /// Is the range from `start_line` to `end_line` contained in a single item of
    /// [`FileObj::diff_chunks`]?
    pub fn is_hunk_in_diff(&self, hunk: &Hunk) -> Option<(u32, u32)> {
        let (start_line, end_line) = if !hunk.before.is_empty() {
            // if old hunk's total lines is > 0
            let start = hunk.before.start;
            (start, start + hunk.before.len() as u32 - 1)
        } else {
            // old hunk's total lines is 0, meaning changes were only added
            let start = hunk.after.start;
            // make old hunk's range span 1 line
            (start, start)
        };
        for range in &self.diff_chunks {
            if range.contains(&start_line) && range.contains(&end_line) {
                return Some((start_line, end_line));
            }
        }
        None
    }

    /// Similar to [`FileObj::is_hunk_in_diff()`] but looks for a single line instead of
    /// an entire [`DiffHunk`].
    ///
    /// This is a private function because it is only used in
    /// [`FileObj::make_suggestions_from_patch()`].
    fn is_line_in_diff(&self, line: &u32) -> bool {
        for range in &self.diff_chunks {
            if range.contains(line) {
                return true;
            }
        }
        false
    }

    /// Create a list of [`Suggestion`](struct@crate::clang_tools::Suggestion) from a
    /// generated diff and store them in the given
    /// [`ReviewComments`](struct@crate::clang_tools::ReviewComments).
    ///
    /// The suggestions will also include diagnostics from clang-tidy that
    /// did not have a fix applied in the patch.
    pub fn make_suggestions_from_patch(
        &self,
        review_comments: &mut ReviewComments,
        summary_only: bool,
        repo_root: &Path,
    ) -> Result<(), FileObjError> {
        let patched = match &self.patched_path {
            Some(patched_path) if patched_path.exists() => {
                fs::read_to_string(patched_path).map_err(FileObjError::ReadFile)?
            }
            _ => return Ok(()),
        };
        let original_content =
            fs::read_to_string(repo_root.join(&self.name)).map_err(FileObjError::ReadFile)?;
        let (diff, input) = make_patch(patched.as_str(), &original_content);
        let file_name = self.name.to_str().unwrap_or_default().replace("\\", "/");

        self.get_suggestions(review_comments, &diff, &input, summary_only)
            .map_err(FileObjError::DisplayStringFailed)?;
        if let Some(advice) = &self.tidy_advice {
            // now check for clang-tidy warnings with no fixes applied
            let file_ext = self
                .name
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            // Count of clang-tidy diagnostics that had no fixes applied
            let mut total = 0;
            for note in &advice.notes {
                if note.fixed_lines.is_empty() && self.is_line_in_diff(&note.line) {
                    // notification had no suggestion applied in `patched`
                    total += 1;
                    if summary_only {
                        continue;
                    }
                    let mut suggestion = format!(
                        "### clang-tidy diagnostic\n**{file_name}:{}:{}** {}: [{}]\n\n> {}\n",
                        &note.line,
                        &note.cols,
                        &note.severity,
                        note.diagnostic_link(),
                        &note.rationale
                    );
                    if !note.suggestion.is_empty() {
                        suggestion.push_str(
                            format!("\n```{file_ext}\n{}\n```\n", &note.suggestion.join("\n"))
                                .as_str(),
                        );
                    }
                    let mut is_merged = false;
                    for s in &mut review_comments.comments {
                        if s.path == file_name
                            && s.line_end >= note.line
                            && s.line_start <= note.line
                        {
                            s.suggestion.push_str(suggestion.as_str());
                            is_merged = true;
                            break;
                        }
                    }
                    if !is_merged {
                        review_comments.comments.push(Suggestion {
                            line_start: note.line,
                            line_end: note.line,
                            suggestion,
                            path: file_name.to_owned(),
                        });
                    }
                }
            }
            review_comments.tool_total += total;
        }
        Ok(())
    }

    /// Create a bunch of suggestions from a [`FileObj`]'s advice's generated `patched` buffer.
    fn get_suggestions(
        &self,
        review_comments: &mut ReviewComments,
        diff: &Diff,
        input: &InternedInput<&str>,
        summary_only: bool,
    ) -> Result<(), std::fmt::Error> {
        let file_name = self
            .name
            .to_string_lossy()
            .replace("\\", "/")
            .trim_start_matches("./")
            .to_owned();
        let mut config = UnifiedDiffConfig::default();
        config.context_len(0);
        let printer = BasicLineDiffPrinter(&input.interner);
        let mut patch_buff = String::new();
        let mut hunks_in_patch = 0u32;
        for hunk in diff.hunks() {
            hunks_in_patch += 1;
            let hunk_range = self.is_hunk_in_diff(&hunk);
            match hunk_range {
                Some((start_line, end_line)) if !summary_only => {
                    let mut suggestion = String::new();
                    let suggestion_help = self
                        .tidy_advice
                        .as_ref()
                        .map(|t| t.get_suggestion_help(start_line, end_line))
                        .unwrap_or_default();
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
                _ => {
                    printer.display_header(
                        &mut patch_buff,
                        hunk.before.start,
                        hunk.after.start,
                        hunk.before.len() as u32,
                        hunk.after.len() as u32,
                    )?;
                    printer.display_hunk(
                        &mut patch_buff,
                        &input.before[hunk.before.start as usize..hunk.before.end as usize],
                        &input.after[hunk.after.start as usize..hunk.after.end as usize],
                    )?;
                }
            }
        }
        if !patch_buff.is_empty() {
            let patch_buf = format!("--- a/{file_name}\n+++ b/{file_name}\n{patch_buff}");
            review_comments.full_patch.push_str(patch_buf.as_str());
        }
        review_comments.tool_total += hunks_in_patch;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::FileObj;
    use crate::cli::LinesChangedOnly;

    // *********************** tests for FileObj::get_ranges()

    #[test]
    fn get_ranges_none() {
        let file_obj = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        let ranges = file_obj.get_ranges(&LinesChangedOnly::Off);
        assert!(ranges.is_empty());
    }

    #[test]
    fn get_ranges_diff() {
        let diff_chunks = vec![1..=10];
        let added_lines = vec![4, 5, 9];
        let file_obj = FileObj::from(
            PathBuf::from("tests/demo/demo.cpp"),
            added_lines,
            diff_chunks.clone(),
        );
        let ranges = file_obj.get_ranges(&LinesChangedOnly::Diff);
        assert_eq!(ranges, diff_chunks);
    }

    #[test]
    fn get_ranges_added() {
        let diff_chunks = vec![1..=10];
        let added_lines = vec![4, 5, 9];
        let file_obj = FileObj::from(
            PathBuf::from("tests/demo/demo.cpp"),
            added_lines,
            diff_chunks,
        );
        let ranges = file_obj.get_ranges(&LinesChangedOnly::On);
        assert_eq!(ranges, vec![4..=5, 9..=9]);
    }

    #[test]
    fn get_ranges_single_added_line() {
        let added_lines = vec![5];
        let file_obj = FileObj::from(PathBuf::from("tests/demo/demo.cpp"), added_lines, vec![]);
        let ranges = file_obj.get_ranges(&LinesChangedOnly::On);
        assert_eq!(ranges, vec![5..=5]);
    }

    #[test]
    fn line_not_in_diff() {
        let file_obj = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        assert!(!file_obj.is_line_in_diff(&42));
    }
}
