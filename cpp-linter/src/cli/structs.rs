use std::{fmt::Display, path::PathBuf, str::FromStr};

use anyhow::{Error, anyhow};
use clap::{ValueEnum, builder::PossibleValue};
use semver::VersionReq;

use super::Cli;
use crate::{
    clang_tools::clang_tidy::CompilationUnit,
    common_fs::{FileFilter, normalize_path},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequestedVersion {
    /// A specific path to the clang tool binary.
    Path(PathBuf),

    /// Whatever the system default uses (if any).
    #[default]
    SystemDefault,

    /// A specific version requirement for the clang tool binary.
    ///
    /// For example, `=12.0.1`, `>=10.0.0, <13.0.0`.
    Requirement(VersionReq),

    /// A sentinel when no value is given.
    ///
    /// This is used internally to differentiate when the user intended
    /// to invoke the `version` subcommand instead.
    NoValue,
}

impl FromStr for RequestedVersion {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            Ok(Self::SystemDefault)
        } else if input == "CPP-LINTER-VERSION" {
            Ok(Self::NoValue)
        } else if let Ok(req) = VersionReq::parse(input) {
            Ok(Self::Requirement(req))
        } else if let Ok(req) = VersionReq::parse(format!("={input}").as_str()) {
            Ok(Self::Requirement(req))
        } else {
            let path = PathBuf::from(input);
            if !path.exists() {
                return Err(anyhow!(
                    "The specified version is not a proper requirement or a valid path: {}",
                    input
                ));
            }
            let path = if !path.is_dir() {
                path.parent()
                    .ok_or(anyhow!(
                        "Unknown parent directory of the given file path for `--version`: {}",
                        input
                    ))?
                    .to_path_buf()
            } else {
                path
            };
            let path = match path.canonicalize() {
                Ok(p) => Ok(normalize_path(&p)),
                Err(e) => Err(anyhow!("Failed to canonicalize path '{input}': {e:?}")),
            }?;
            Ok(Self::Path(path))
        }
    }
}

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
    pub delete_review_comments: bool,
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
            delete_review_comments: args.feedback_options.delete_review_comments,
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
            delete_review_comments: false,
        }
    }
}

#[cfg(test)]
mod test {
    // use crate::cli::get_arg_parser;

    use std::{path::PathBuf, str::FromStr};

    use crate::{cli::RequestedVersion, common_fs::normalize_path};

    use super::{Cli, LinesChangedOnly, ThreadComments};
    use clap::{Parser, ValueEnum};

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
            LinesChangedOnly::from_str(&input, true).unwrap(),
            LinesChangedOnly::Diff
        );
        assert_eq!(format!("{}", LinesChangedOnly::Diff), input.to_lowercase());

        assert_eq!(
            LinesChangedOnly::from_str(&input, false).unwrap(),
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
    fn validate_version_path() {
        let this_path_str = "src/cli/structs.rs";
        let this_path = PathBuf::from(this_path_str);
        let this_canonical = this_path.canonicalize().unwrap();
        let parent = this_canonical.parent().unwrap();
        let expected = normalize_path(parent);
        let req_ver = RequestedVersion::from_str(this_path_str).unwrap();
        if let RequestedVersion::Path(parsed) = req_ver {
            assert_eq!(&parsed, &expected);
        }

        assert!(RequestedVersion::from_str("file.rs").is_err());
    }
}
