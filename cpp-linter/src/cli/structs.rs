use std::{fmt::Display, path::PathBuf};

#[cfg(feature = "bin")]
use clap::{ValueEnum, builder::PossibleValue};

#[cfg(feature = "bin")]
use super::{Cli, convert_extra_arg_val};
use crate::clang_tools::clang_tidy::CompilationUnit;

use git_bot_feedback::FileFilter;

/// An enum to describe `--lines-changed-only` CLI option's behavior.
#[derive(PartialEq, Clone, Debug, Default)]
pub enum LinesChangedOnly {
    /// All lines are scanned
    #[default]
    Off,
    /// Only lines in the diff are scanned
    Diff,
    /// Only lines in the diff with additions are scanned.
    On,
}

impl From<LinesChangedOnly> for git_bot_feedback::LinesChangedOnly {
    fn from(val: LinesChangedOnly) -> Self {
        match val {
            LinesChangedOnly::Off => git_bot_feedback::LinesChangedOnly::Off,
            LinesChangedOnly::Diff => git_bot_feedback::LinesChangedOnly::Diff,
            LinesChangedOnly::On => git_bot_feedback::LinesChangedOnly::On,
        }
    }
}

#[cfg(feature = "bin")]
impl ValueEnum for LinesChangedOnly {
    /// Get a list possible value variants for display in `--help` output.
    fn value_variants<'a>() -> &'a [Self] {
        &[
            LinesChangedOnly::Off,
            LinesChangedOnly::Diff,
            LinesChangedOnly::On,
        ]
    }

    /// Get a display value (for `--help` output) of the enum variant.
    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            LinesChangedOnly::Off => Some(
                PossibleValue::new("false")
                    .help("All lines in a file are analyzed.")
                    .aliases(["off", "0"]),
            ),
            LinesChangedOnly::Diff => Some(PossibleValue::new("diff").help(
                "All lines in the diff are analyzed \
                    (including unchanged lines but not subtractions).",
            )),
            LinesChangedOnly::On => Some(
                PossibleValue::new("true")
                    .help("Only lines in the diff that contain additions are analyzed.")
                    .aliases(["on", "1"]),
            ),
        }
    }

    /// Parse a string into a [`LinesChangedOnly`] enum variant.
    fn from_str(val: &str, ignore_case: bool) -> Result<LinesChangedOnly, String> {
        let val = if ignore_case {
            val.to_lowercase()
        } else {
            val.to_string()
        };
        match val.as_str() {
            "true" | "on" | "1" => Ok(LinesChangedOnly::On),
            "diff" => Ok(LinesChangedOnly::Diff),
            _ => Ok(LinesChangedOnly::Off),
        }
    }
}

impl LinesChangedOnly {
    /// Is the instance valid for under the given conditions/flags?
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
#[derive(PartialEq, Clone, Debug)]
pub enum ThreadComments {
    /// Always post a new comment and delete any outdated ones.
    On,
    /// Do not post thread comments.
    Off,
    /// Only update existing thread comments.
    /// If none exist, then post a new one.
    Update,
}

#[cfg(feature = "bin")]
impl ValueEnum for ThreadComments {
    /// Get a list possible value variants for display in `--help` output.
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::On, Self::Off, Self::Update]
    }

    /// Get a display value (for `--help` output) of the enum variant.
    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            ThreadComments::On => Some(
                PossibleValue::new("true")
                    .help("Always post a new comment and delete any outdated ones.")
                    .aliases(["on", "1"]),
            ),
            ThreadComments::Off => Some(
                PossibleValue::new("false")
                    .help("Do not post thread comments.")
                    .aliases(["off", "0"]),
            ),
            ThreadComments::Update => {
                Some(PossibleValue::new("update").help(
                    "Only update existing thread comments. If none exist, then post a new one.",
                ))
            }
        }
    }

    /// Parse a string into a [`ThreadComments`] enum variant.
    fn from_str(val: &str, ignore_case: bool) -> Result<ThreadComments, String> {
        let val = if ignore_case {
            val.to_lowercase()
        } else {
            val.to_string()
        };
        match val.as_str() {
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
///
/// This struct is designed to be a thread-safe vehicle for common clang arguments and configurations.
#[derive(Debug, Clone, Default)]
pub struct ClangParams {
    /// The clang-tidy checks to run.
    ///
    /// Format of this string follows the `-checks` argument of clang-tidy.
    pub tidy_checks: String,

    /// Focus on changed lines or entire files.
    pub lines_changed_only: LinesChangedOnly,

    /// An optional path to a compilation database, used for clang-tidy.
    pub database: Option<PathBuf>,

    /// Extra arguments to pass to clang-tidy.
    ///
    /// Format of these strings follows the `-extra-arg` argument of clang-tidy.
    pub extra_args: Vec<String>,

    /// An optional list of compilation units, used for clang-tidy.
    ///
    /// This can be set to None initially, but it will be populated by
    /// [`capture_clang_tools_output()`](crate::clang_tools::capture_clang_tools_output),
    /// if the [`Self::database`] is given [`Some`] valid value (and the
    /// compile_commands.json file is parsed successfully).
    pub database_json: Option<Vec<CompilationUnit>>,

    /// The clang-format style to use.
    ///
    /// Format of this string follows the `-style` argument of clang-format.
    pub style: String,

    /// An optional path to the clang-tidy executable.
    ///
    /// If [`Self::tidy_checks`] is not `-*`, then this will be populated by
    /// [`capture_clang_tools_output()`](crate::clang_tools::capture_clang_tools_output),
    /// regardless if this is given [`Some`] value.
    pub clang_tidy_command: Option<PathBuf>,

    /// An optional path to the clang-format executable.
    ///
    /// If [`Self::style`] is not an empty string, then this will be populated by
    /// [`capture_clang_tools_output()`](crate::clang_tools::capture_clang_tools_output),
    /// regardless if this is given [`Some`] value.
    pub clang_format_command: Option<PathBuf>,

    /// An optional [`FileFilter`] to exclude files only from clang-tidy analysis.
    pub tidy_filter: Option<FileFilter>,

    /// An optional [`FileFilter`] to exclude files only from clang-format analysis.
    pub format_filter: Option<FileFilter>,

    /// The root of the repository, used to locate relative file paths in processing.
    ///
    /// A project-specific cache folder is created in this path.
    pub repo_root: PathBuf,
}

impl ClangParams {
    /// The directory name to use for caching clang-tidy and clang-format results.
    pub(crate) const CACHE_DIR: &str = ".cpp-linter-cache";

    /// The file name for aggregating auto-fixes into a unified patch.
    pub(crate) const AUTO_FIX_PATCH: &str = "auto-fix.patch";

    pub(crate) fn get_cache_path(&self) -> PathBuf {
        self.repo_root.join(Self::CACHE_DIR).join("patched")
    }
}

#[cfg(feature = "bin")]
impl From<&Cli> for ClangParams {
    /// Construct a [`ClangParams`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        let extensions: Vec<&str> = args
            .source_options
            .extensions
            .iter()
            .map(|ext| ext.as_str())
            .collect();
        let tidy_filter = args.tidy_options.ignore_tidy.as_ref().map(|ignore_tidy| {
            let ignore_tidy: Vec<&str> = ignore_tidy.iter().map(|s| s.as_str()).collect();
            FileFilter::new(&ignore_tidy, &extensions.clone(), Some("clang-tidy"))
        });
        let format_filter = args
            .format_options
            .ignore_format
            .as_ref()
            .map(|ignore_format| {
                let ignore_format: Vec<&str> = ignore_format.iter().map(|s| s.as_str()).collect();
                FileFilter::new(&ignore_format, &extensions, Some("clang-format"))
            });
        let repo_root = args.source_options.repo_root.clone();
        let database = args
            .tidy_options
            .database
            .as_ref()
            .map(PathBuf::from)
            .map(|db| {
                if db.is_relative() {
                    repo_root.join(db)
                } else {
                    db
                }
            });
        ClangParams {
            tidy_checks: args.tidy_options.tidy_checks.clone(),
            lines_changed_only: args.source_options.lines_changed_only.clone(),
            database,
            extra_args: convert_extra_arg_val(&args.tidy_options.extra_arg),
            database_json: None,
            style: args.format_options.style.clone(),
            clang_tidy_command: None,
            clang_format_command: None,
            tidy_filter,
            format_filter,
            repo_root,
        }
    }
}

/// A struct to contain CLI options that relate to
/// [`RestClient.post_feedback()`](fn@crate::rest_api::RestClient.post_feedback()).
pub struct FeedbackInput {
    /// How thread comments are created or updated.
    pub thread_comments: ThreadComments,

    /// Whether to omit a "LGTM" type message.
    pub no_lgtm: bool,

    /// Whether to post a step summary comment.
    pub step_summary: bool,

    /// An optional file path to which a summary comment is written.
    pub summary_output_file: Option<PathBuf>,

    /// Whether to post file annotations.
    pub file_annotations: bool,

    /// The clang-format style to show in file annotations.
    pub style: String,

    /// Whether to post a PR review.
    pub pr_review: bool,

    /// Should PR reviews be commentary?
    ///
    /// If false, reviews will approve or request changes.
    pub passive_reviews: bool,

    /// The root of the repository, used to locate relative file paths in processing.
    pub repo_root: PathBuf,
}

#[cfg(feature = "bin")]
impl From<&Cli> for FeedbackInput {
    /// Construct a [`FeedbackInput`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        FeedbackInput {
            style: args.format_options.style.clone(),
            no_lgtm: args.feedback_options.no_lgtm,
            step_summary: args.feedback_options.step_summary,
            thread_comments: args.feedback_options.thread_comments.clone(),
            file_annotations: args.feedback_options.file_annotations,
            pr_review: args.feedback_options.pr_review,
            passive_reviews: args.feedback_options.passive_reviews,
            repo_root: args.source_options.repo_root.clone(),
            summary_output_file: args.feedback_options.summary_output_file.clone(),
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
            pr_review: false,
            passive_reviews: false,
            repo_root: PathBuf::from("."),
            summary_output_file: None,
        }
    }
}

#[cfg(all(test, feature = "bin"))]
mod test {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use clap::{Parser, ValueEnum};

    use super::{ClangParams, Cli, LinesChangedOnly, ThreadComments};

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
        let input = "Diff";
        assert_eq!(
            LinesChangedOnly::from_str(input, true).unwrap(),
            LinesChangedOnly::Diff
        );
        assert_eq!(format!("{}", LinesChangedOnly::Diff), input.to_lowercase());

        assert_eq!(
            LinesChangedOnly::from_str(input, false).unwrap(),
            LinesChangedOnly::Off
        );
    }

    #[test]
    fn display_thread_comments_enum() {
        let input = "Update";
        assert_eq!(
            ThreadComments::from_str(input, true).unwrap(),
            ThreadComments::Update
        );
        assert_eq!(format!("{}", ThreadComments::Update), input.to_lowercase());
        assert_eq!(
            ThreadComments::from_str(input, false).unwrap(),
            ThreadComments::Off
        );
    }

    #[test]
    fn absolute_db_path() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let cli = Cli::parse_from(["cpp-linter", "--database", tmp_dir.path().to_str().unwrap()]);
        let clang_params = ClangParams::from(&cli);
        assert_eq!(clang_params.database, Some(tmp_dir.path().to_path_buf()));
    }
}
