//! A module to hold all common file system functionality.

use std::fmt::Debug;
use std::fs;
use std::path::{Component, Path};
use std::{ops::RangeInclusive, path::PathBuf};

use anyhow::{Context, Result};

use crate::clang_tools::clang_format::FormatAdvice;
use crate::clang_tools::clang_tidy::TidyAdvice;
use crate::clang_tools::{make_patch, MakeSuggestions, ReviewComments, Suggestion};
use crate::cli::LinesChangedOnly;
mod file_filter;
pub use file_filter::FileFilter;
use git2::DiffHunk;

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
        }
    }

    /// Instantiate an object with file name and changed lines information.
    pub fn from(
        name: PathBuf,
        added_lines: Vec<u32>,
        diff_chunks: Vec<RangeInclusive<u32>>,
    ) -> Self {
        let added_ranges = FileObj::consolidate_numbers_to_ranges(&added_lines);
        FileObj {
            name,
            added_lines,
            added_ranges,
            diff_chunks,
            format_advice: None,
            tidy_advice: None,
        }
    }

    /// A helper function to consolidate a [Vec<u32>] of line numbers into a
    /// [Vec<RangeInclusive<u32>>] in which each range describes the beginning and
    /// ending of a group of consecutive line numbers.
    fn consolidate_numbers_to_ranges(lines: &[u32]) -> Vec<RangeInclusive<u32>> {
        let mut range_start = None;
        let mut ranges: Vec<RangeInclusive<u32>> = Vec::new();
        for (index, number) in lines.iter().enumerate() {
            if index == 0 {
                range_start = Some(*number);
            } else if number - 1 != lines[index - 1] {
                ranges.push(RangeInclusive::new(range_start.unwrap(), lines[index - 1]));
                range_start = Some(*number);
            }
            if index == lines.len() - 1 {
                ranges.push(RangeInclusive::new(range_start.unwrap(), *number));
            }
        }
        ranges
    }

    pub fn get_ranges(&self, lines_changed_only: &LinesChangedOnly) -> Vec<RangeInclusive<u32>> {
        match lines_changed_only {
            LinesChangedOnly::Diff => self.diff_chunks.to_vec(),
            LinesChangedOnly::On => self.added_ranges.to_vec(),
            _ => Vec::new(),
        }
    }

    /// Is the range from `start_line` to `end_line` contained in a single item of
    /// [`FileObj::diff_chunks`]?
    pub fn is_hunk_in_diff(&self, hunk: &DiffHunk) -> Option<(u32, u32)> {
        let (start_line, end_line) = if hunk.old_lines() > 0 {
            // if old hunk's total lines is > 0
            let start = hunk.old_start();
            (start, start + hunk.old_lines() - 1)
        } else {
            // old hunk's total lines is 0, meaning changes were only added
            let start = hunk.new_start();
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
    /// generated [`Patch`](struct@git2::Patch) and store them in the given
    /// [`ReviewComments`](struct@crate::clang_tools::ReviewComments).
    ///
    /// The suggestions will also include diagnostics from clang-tidy that
    /// did not have a fix applied in the patch.
    pub fn make_suggestions_from_patch(
        &self,
        review_comments: &mut ReviewComments,
        summary_only: bool,
    ) -> Result<()> {
        let original_content =
            fs::read(&self.name).with_context(|| "Failed to read original contents of file")?;
        let file_name = self.name.to_str().unwrap_or_default().replace("\\", "/");
        let file_path = Path::new(&file_name);
        if let Some(advice) = &self.format_advice {
            if let Some(patched) = &advice.patched {
                let mut patch = make_patch(file_path, patched, &original_content)?;
                advice.get_suggestions(review_comments, self, &mut patch, summary_only)?;
            }
        }

        if let Some(advice) = &self.tidy_advice {
            if let Some(patched) = &advice.patched {
                let mut patch = make_patch(file_path, patched, &original_content)?;
                advice.get_suggestions(review_comments, self, &mut patch, summary_only)?;
            }

            if summary_only {
                return Ok(());
            }

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
                    total += 1;
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
            review_comments.tool_total[1] =
                Some(review_comments.tool_total[1].unwrap_or_default() + total);
        }
        Ok(())
    }
}

/// Gets the line number for a given `offset` (of bytes) from the given
/// buffer `contents`.
///
/// The `offset` given to this function is expected to originate from
/// diagnostic information provided by clang-format. Any `offset` out of
/// bounds is clamped to the given `contents` buffer's length.
pub fn get_line_count_from_offset(contents: &[u8], offset: u32) -> u32 {
    let offset = (offset as usize).min(contents.len());
    let lines = contents[0..offset].split(|byte| byte == &b'\n');
    lines.count() as u32
}

/// This was copied from [cargo source code](https://github.com/rust-lang/cargo/blob/fede83ccf973457de319ba6fa0e36ead454d2e20/src/cargo/util/paths.rs#L61).
///
/// NOTE: Rust [std::path] crate has no native functionality equivalent to this.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::{env::current_dir, fs};

    use super::{get_line_count_from_offset, normalize_path, FileObj};
    use crate::cli::LinesChangedOnly;

    // *********************** tests for normalized paths

    #[test]
    fn normalize_redirects() {
        let mut src = current_dir().unwrap();
        src.push("..");
        src.push(
            current_dir()
                .unwrap()
                .strip_prefix(current_dir().unwrap().parent().unwrap())
                .unwrap(),
        );
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), current_dir().unwrap());
    }

    #[test]
    fn normalize_no_root() {
        let src = PathBuf::from("../cpp-linter");
        let mut cur_dir = current_dir().unwrap();
        cur_dir = cur_dir
            .strip_prefix(current_dir().unwrap().parent().unwrap())
            .unwrap()
            .to_path_buf();
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), cur_dir);
    }

    #[test]
    fn normalize_current_redirect() {
        let src = PathBuf::from("tests/./ignored_paths");
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), PathBuf::from("tests/ignored_paths"));
    }

    // *********************** tests for translating byte offset into line/column

    #[test]
    fn translate_byte_offset() {
        let contents = fs::read(PathBuf::from("tests/demo/demo.cpp")).unwrap();
        let lines = get_line_count_from_offset(&contents, 144);
        assert_eq!(lines, 13);
    }

    #[test]
    fn get_line_count_edge_cases() {
        // Empty content
        assert_eq!(get_line_count_from_offset(&[], 0), 1);

        // No newlines
        assert_eq!(get_line_count_from_offset(b"abc", 3), 1);

        // Consecutive newlines
        assert_eq!(get_line_count_from_offset(b"a\n\nb", 3), 3);

        // Offset beyond content length
        assert_eq!(get_line_count_from_offset(b"a\nb\n", 10), 3);
    }
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
    fn line_not_in_diff() {
        let file_obj = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        assert!(!file_obj.is_line_in_diff(&42));
    }
}
