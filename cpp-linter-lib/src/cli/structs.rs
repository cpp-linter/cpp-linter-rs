use std::path::PathBuf;

use clap::ArgMatches;

use super::convert_extra_arg_val;
use crate::{clang_tools::clang_tidy::CompilationDatabase, common_fs::FileFilter};

/// An enum to describe `--lines-changed-only` CLI option's behavior.
#[derive(PartialEq, Clone, Debug)]
pub enum LinesChangedOnly {
    /// All lines are scanned
    Off,
    /// Only lines in the diff are scanned
    Diff,
    /// Only lines in the diff with additions are scanned.
    On,
}

impl LinesChangedOnly {
    fn from_string(val: &str) -> LinesChangedOnly {
        match val {
            "true" | "on" | "1" => LinesChangedOnly::On,
            "diff" => LinesChangedOnly::Diff,
            _ => LinesChangedOnly::Off,
        }
    }
}

/// A structure to contain parsed CLI options.
pub struct Cli {
    pub version: String,
    pub verbosity: bool,
    pub extensions: Vec<String>,
    pub repo_root: String,
    pub lines_changed_only: LinesChangedOnly,
    pub files_changed_only: bool,
    pub ignore: Vec<String>,
    pub style: String,
    pub ignore_format: Vec<String>,
    pub ignore_tidy: Vec<String>,
    pub tidy_checks: String,
    pub database: Option<PathBuf>,
    pub extra_arg: Option<Vec<String>>,
    pub thread_comments: ThreadComments,
    pub no_lgtm: bool,
    pub step_summary: bool,
    pub file_annotations: bool,
    pub not_ignored: Option<Vec<String>>,
}

impl From<&ArgMatches> for Cli {
    /// Construct a [`Cli`] instance from a [`ArgMatches`] instance (after options are parsed from CLI).
    fn from(args: &ArgMatches) -> Self {
        let ignore = args
            .get_many::<String>("ignore")
            .unwrap()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        let ignore_tidy = if let Some(val) = args.get_many::<String>("ignore-tidy") {
            val.map(|s| s.to_owned()).collect::<Vec<_>>()
        } else {
            vec![]
        };
        let ignore_format = if let Some(val) = args.get_many::<String>("ignore-format") {
            val.map(|s| s.to_owned()).collect::<Vec<_>>()
        } else {
            vec![]
        };
        let extra_arg = convert_extra_arg_val(args);

        let lines_changed_only = LinesChangedOnly::from_string(
            args.get_one::<String>("lines-changed-only")
                .unwrap()
                .as_str(),
        );

        let thread_comments = ThreadComments::from_string(
            args.get_one::<String>("thread-comments").unwrap().as_str(),
        );

        let extensions = args
            .get_many::<String>("extensions")
            .unwrap()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        Self {
            version: args.get_one::<String>("version").unwrap().to_owned(),
            verbosity: args.get_one::<String>("verbosity").unwrap().as_str() == "debug",
            extensions,
            repo_root: args.get_one::<String>("repo-root").unwrap().to_owned(),
            lines_changed_only,
            files_changed_only: args.get_flag("files-changed-only"),
            ignore,
            style: args.get_one::<String>("style").unwrap().to_owned(),
            ignore_format,
            ignore_tidy,
            tidy_checks: args.get_one::<String>("tidy-checks").unwrap().to_owned(),
            database: args.get_one::<PathBuf>("database").map(|v| v.to_owned()),
            extra_arg,
            no_lgtm: args.get_flag("no-lgtm"),
            step_summary: args.get_flag("step-summary"),
            thread_comments,
            file_annotations: args.get_flag("file-annotations"),
            not_ignored: args
                .get_many::<String>("files")
                .map(|files| Vec::from_iter(files.map(|v| v.to_owned()))),
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

impl ThreadComments {
    fn from_string(val: &str) -> ThreadComments {
        match val {
            "true" | "on" | "1" => ThreadComments::On,
            "update" => ThreadComments::Update,
            _ => ThreadComments::Off,
        }
    }
}

/// A data structure to contain CLI options that relate to
/// clang-tidy or clang-format arguments.
#[derive(Debug, Clone)]
pub struct ClangParams {
    pub tidy_checks: String,
    pub lines_changed_only: LinesChangedOnly,
    pub database: Option<PathBuf>,
    pub extra_args: Option<Vec<String>>,
    pub database_json: Option<CompilationDatabase>,
    pub style: String,
    pub clang_tidy_command: Option<PathBuf>,
    pub clang_format_command: Option<PathBuf>,
    pub tidy_filter: FileFilter,
    pub format_filter: FileFilter,
}

impl From<&Cli> for ClangParams {
    /// Construct a [`ClangParams`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        ClangParams {
            tidy_checks: args.tidy_checks.clone(),
            lines_changed_only: args.lines_changed_only.clone(),
            database: args.database.clone(),
            extra_args: args.extra_arg.clone(),
            database_json: None,
            style: args.style.clone(),
            clang_tidy_command: None,
            clang_format_command: None,
            tidy_filter: FileFilter::new(&args.ignore_tidy, args.extensions.clone()),
            format_filter: FileFilter::new(&args.ignore_format, args.extensions.clone()),
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
}

impl From<&Cli> for FeedbackInput {
    /// Construct a [`FeedbackInput`] instance from a [`Cli`] instance.
    fn from(args: &Cli) -> Self {
        FeedbackInput {
            style: args.style.clone(),
            no_lgtm: args.no_lgtm,
            step_summary: args.step_summary,
            thread_comments: args.thread_comments.clone(),
            file_annotations: args.file_annotations,
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
        }
    }
}

#[cfg(test)]
mod test {
    use crate::cli::get_arg_parser;

    use super::Cli;

    #[test]
    fn parse_positional() {
        let parser = get_arg_parser();
        let args = parser.get_matches_from(["cpp-linter", "file1.c", "file2.h"]);
        let cli = Cli::from(&args);
        let not_ignored = cli.not_ignored.expect("failed to parse positional args");
        assert!(!not_ignored.is_empty());
        assert!(not_ignored.contains(&String::from("file1.c")));
        assert!(not_ignored.contains(&String::from("file2.h")));
    }
}
