//! A module to hold all common file system functionality.

use std::fs;
use std::io::Read;
use std::path::{Component, Path};
use std::{ops::RangeInclusive, path::PathBuf};

use crate::clang_tools::clang_format::FormatAdvice;
use crate::clang_tools::clang_tidy::TidyAdvice;
use crate::cli::LinesChangedOnly;
mod file_filter;
pub use file_filter::FileFilter;

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
    /// To instantiate an object with line information, use [FileObj::from].
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
}

/// Gets the line and column number from a given `offset` (of bytes) for given
/// `file_path`.
///
/// This computes the line and column numbers from a buffer of bytes read from the
/// `file_path`. In non-UTF-8 encoded files, this does not guarantee that a word
/// boundary exists at the returned column number. However, the `offset` given to this
/// function is expected to originate from diagnostic information provided by
/// clang-format or clang-tidy.
pub fn get_line_cols_from_offset(file_path: &PathBuf, offset: usize) -> (usize, usize) {
    let mut file_buf = vec![0; offset];
    fs::File::open(file_path)
        .unwrap()
        .read_exact(&mut file_buf)
        .unwrap();
    let lines = file_buf.split(|byte| byte == &b'\n');
    let line_count = lines.clone().count();
    let column_count = lines.last().unwrap_or(&[]).len() + 1; // +1 because not a 0 based count
    (line_count, column_count)
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
    use std::env::current_dir;
    use std::path::PathBuf;

    use super::{get_line_cols_from_offset, normalize_path, FileObj};
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
        let src = PathBuf::from("../cpp-linter-lib");
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
        let (lines, cols) = get_line_cols_from_offset(&PathBuf::from("tests/demo/demo.cpp"), 144);
        println!("lines: {lines}, cols: {cols}");
        assert_eq!(lines, 13);
        assert_eq!(cols, 5);
    }

    // *********************** tests for FileObj::get_ranges()

    #[test]
    fn get_ranges_0() {
        let file_obj = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        let ranges = file_obj.get_ranges(&LinesChangedOnly::Off);
        assert!(ranges.is_empty());
    }

    #[test]
    fn get_ranges_2() {
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
    fn get_ranges_1() {
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
}
