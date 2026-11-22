//! How clang-format ran in a check.

use std::{ops::RangeInclusive, path::Path};

use super::paths::prefix_feedback_path;
use crate::markdown;

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
    /// Where `--repo-root` is in the git repository (empty when it is the repository root).
    ///
    /// File names in a report are relative to the repository root, so that the commands
    /// in it can be run there.
    pub path_prefix: &'a Path,
}

impl<'a> From<&'a str> for FormatRun<'a> {
    /// clang-format checked whole files with the given `--style`.
    fn from(style: &'a str) -> Self {
        Self {
            style,
            lines_only: false,
            path_prefix: Path::new(""),
        }
    }
}

impl FormatRun<'_> {
    /// The name of a file as a report shows it: relative to the repository root.
    pub(super) fn file_name(&self, name: &str) -> String {
        prefix_feedback_path(name, self.path_prefix)
    }
}

impl FormatRun<'_> {
    /// The `clang-format` command(s) that fix the formatting of the given files in place:
    /// one command for all files, or one command per file with its `--lines`.
    pub(super) fn command<'f>(
        &self,
        files: impl IntoIterator<Item = (&'f str, &'f [RangeInclusive<u32>])>,
    ) -> String {
        let style = match self.style {
            "" => String::from("file"),
            // a relative path to a style file is relative to `--repo-root`
            style => match style.strip_prefix("file:") {
                Some(path) if Path::new(path).is_relative() => {
                    format!("file:{}", self.file_name(path))
                }
                _ => style.to_string(),
            },
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
                command.push_str(&file_arg(file));
                commands.push(command);
            } else {
                whole_files.push(' ');
                whole_files.push_str(&file_arg(file));
            }
        }
        if self.lines_only {
            commands.join("\n")
        } else {
            whole_files
        }
    }
}

/// A file name as a shell word for clang-format.
///
/// clang-format reads a word that starts with `-` as an option,
/// so a name like `-file.cpp` is written as `./-file.cpp`.
fn file_arg(file: &str) -> String {
    if file.starts_with('-') {
        markdown::shell_quote(&format!("./{file}"))
    } else {
        markdown::shell_quote(file)
    }
}
