use std::{fmt::Display, path::PathBuf, str::FromStr};

use clap::ValueEnum;

use super::Cli;
use crate::{clang_tools::clang_tidy::CompilationUnit, common_fs::FileFilter};

/// An enum to describe `--lines-changed-only` CLI option's behavior.
#[derive(PartialEq, Clone, Debug, Default, ValueEnum)]
pub enum LinesChangedOnly {
    /// All lines are scanned
    #[default]
    Off,
    /// Only lines in the diff are scanned
    Diff,
    /// Only lines in the diff with additions are scanned.
    On,
}

impl FromStr for LinesChangedOnly {
    type Err = ();

    fn from_str(val: &str) -> Result<LinesChangedOnly, Self::Err> {
        match val {
            "true" | "on" | "1" => Ok(LinesChangedOnly::On),
            "diff" => Ok(LinesChangedOnly::Diff),
            _ => Ok(LinesChangedOnly::Off),
        }
    }
}

impl LinesChangedOnly {
    pub fn is_change_valid(&self, added_lines: bool, diff_chunks: bool) -> bool {
        match self {
            LinesChangedOnly::Off => true,
            LinesChangedOnly::Diff => diff_chunks,
            LinesChangedOnly::On => added_lines,
        }
    }
}

impl Display for LinesChangedOnly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinesChangedOnly::Off => write!(f, "false"),
            LinesChangedOnly::Diff => write!(f, "diff"),
            LinesChangedOnly::On => write!(f, "true"),
        }
    }
}

/// An enum to describe `--thread-comments` CLI option's behavior.
#[derive(PartialEq, Clone, Debug, ValueEnum)]
pub enum ThreadComments {
    /// Always post a new comment and delete any outdated ones.
    On,
    /// Do not post thread comments.
    Off,
    /// Only update existing thread comments.
    /// If none exist, then post a new one.
    Update,
}

impl FromStr for ThreadComments {
    type Err = ();

    fn from_str(val: &str) -> Result<ThreadComments, Self::Err> {
        match val {
            "true" | "on" | "1" => Ok(ThreadComments::On),
            "update" => Ok(ThreadComments::Update),
            _ => Ok(ThreadComments::Off),
        }
    }
}

impl Display for ThreadComments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadComments::On => write!(f, "true"),
            ThreadComments::Off => write!(f, "false"),
            ThreadComments::Update => write!(f, "update"),
        }
    }
}

/// A data structure to contain CLI options that relate to
/// clang-tidy or clang-format arguments.
#[derive(Debug, Clone, Default)]
pub struct ClangParams {
    pub tidy_checks: String,
    pub lines_changed_only: LinesChangedOnly,
    pub database: Option<PathBuf>,
    pub extra_args: Vec<String>,
    pub database_json: Option<Vec<CompilationUnit>>,
    pub style: String,
    pub clang_tidy_command: Option<PathBuf>,
    pub clang_format_command: Option<PathBuf>,
    pub tidy_filter: Option<FileFilter>,
    pub format_filter: Option<FileFilter>,
    pub tidy_review: bool,
    pub format_review: bool,
}

impl From<&Cli> for ClangParams {
    /// Construct a [`ClangParams`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        ClangParams {
            tidy_checks: args.tidy_options.tidy_checks.clone(),
            lines_changed_only: args.source_options.lines_changed_only.clone(),
            database: args.tidy_options.database.clone(),
            extra_args: args.tidy_options.extra_arg.clone(),
            database_json: None,
            style: args.format_options.style.clone(),
            clang_tidy_command: None,
            clang_format_command: None,
            tidy_filter: args.tidy_options.ignore_tidy.as_ref().map(|ignore_tidy| {
                FileFilter::new(ignore_tidy, args.source_options.extensions.clone())
            }),
            format_filter: args
                .format_options
                .ignore_format
                .as_ref()
                .map(|ignore_format| {
                    FileFilter::new(ignore_format, args.source_options.extensions.clone())
                }),
            tidy_review: args.feedback_options.tidy_review,
            format_review: args.feedback_options.format_review,
        }
    }
}

/// A struct to contain CLI options that relate to
/// [`ResApiClient.post_feedback()`](fn@crate::rest_api::ResApiClient.post_feedback()).
pub struct FeedbackInput {
    pub thread_comments: ThreadComments,
    pub no_lgtm: bool,
    pub step_summary: bool,
    pub file_annotations: bool,
    pub style: String,
    pub tidy_review: bool,
    pub format_review: bool,
    pub passive_reviews: bool,
}

impl From<&Cli> for FeedbackInput {
    /// Construct a [`FeedbackInput`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        FeedbackInput {
            style: args.format_options.style.clone(),
            no_lgtm: args.feedback_options.no_lgtm,
            step_summary: args.feedback_options.step_summary,
            thread_comments: args.feedback_options.thread_comments.clone(),
            file_annotations: args.feedback_options.file_annotations,
            tidy_review: args.feedback_options.tidy_review,
            format_review: args.feedback_options.format_review,
            passive_reviews: args.feedback_options.passive_reviews,
        }
    }
}

impl Default for FeedbackInput {
    /// Construct a [`FeedbackInput`] instance with default values.
    fn default() -> Self {
        FeedbackInput {
            thread_comments: ThreadComments::Off,
            no_lgtm: true,
            step_summary: false,
            file_annotations: true,
            style: "llvm".to_string(),
            tidy_review: false,
            format_review: false,
            passive_reviews: false,
        }
    }
}

#[cfg(test)]
mod test {
    // use crate::cli::get_arg_parser;

    use super::{Cli, LinesChangedOnly, ThreadComments};
    use clap::Parser;
    use std::str::FromStr;

    #[test]
    fn parse_positional() {
        let cli = Cli::parse_from(["cpp-linter", "file1.c", "file2.h"]);
        let not_ignored = cli.not_ignored.expect("failed to parse positional args");
        assert!(!not_ignored.is_empty());
        assert!(not_ignored.contains(&String::from("file1.c")));
        assert!(not_ignored.contains(&String::from("file2.h")));
    }

    #[test]
    fn display_lines_changed_only_enum() {
        let input = "diff".to_string();
        assert_eq!(
            LinesChangedOnly::from_str(&input).unwrap(),
            LinesChangedOnly::Diff
        );
        assert_eq!(format!("{}", LinesChangedOnly::Diff), input);
    }

    #[test]
    fn display_thread_comments_enum() {
        let input = "false".to_string();
        assert_eq!(
            ThreadComments::from_str(&input).unwrap(),
            ThreadComments::Off
        );
        assert_eq!(format!("{}", ThreadComments::Off), input);
    }
}
