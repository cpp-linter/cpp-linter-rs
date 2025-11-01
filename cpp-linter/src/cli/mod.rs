//! This module holds the Command Line Interface design.
use std::{path::PathBuf, str::FromStr};

// non-std crates
use clap::{
    builder::{
        ArgPredicate, FalseyValueParser, NonEmptyStringValueParser, PossibleValuesParser,
        TypedValueParser,
    },
    value_parser, ArgAction, Args, Parser, Subcommand, ValueEnum,
};

mod structs;
pub use structs::{ClangParams, FeedbackInput, LinesChangedOnly, ThreadComments};

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum Verbosity {
    Info,
    Debug,
}

impl Verbosity {
    pub fn is_debug(&self) -> bool {
        matches!(self, Verbosity::Debug)
    }
}

/// A structure to contain parsed CLI options.
#[derive(Debug, Clone, Parser)]
#[command(author, about)]
pub struct Cli {
    #[command(flatten)]
    pub general_options: GeneralOptions,

    #[command(flatten)]
    pub source_options: SourceOptions,

    #[command(flatten)]
    pub format_options: FormatOptions,

    #[command(flatten)]
    pub tidy_options: TidyOptions,

    #[command(flatten)]
    pub feedback_options: FeedbackOptions,

    /// An explicit path to a file.
    ///
    /// This can be specified zero or more times, resulting in a list of files.
    /// The list of files is appended to the internal list of 'not ignored' files.
    /// Further filtering can still be applied (see [Source options](#source-options)).
    #[arg(
        name = "files",
        value_name = "FILE",
        action = ArgAction::Append,
        verbatim_doc_comment,
    )]
    pub not_ignored: Option<Vec<String>>,

    #[command(subcommand)]
    pub commands: Option<SubCommand>,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SubCommand {
    /// Display the version of cpp-linter and exit.
    Version,
}

#[derive(Debug, Clone, Args)]
#[group(id = "General options", multiple = true, required = false)]
pub struct GeneralOptions {
    /// The desired version of the clang tools to use.
    ///
    /// Accepted options are:
    ///
    /// - A semantic major version number from `10` to `21`.
    /// - A blank string (`''`) to use the platform's default
    ///   installed version.
    /// - A path to where the clang tools are
    ///   installed (if using a custom install location).
    ///   All paths specified here are converted to absolute.
    #[arg(
        short = 'V',
        long,
        default_missing_value = "NO-VERSION",
        default_value = "",
        num_args = 0..=1,
        require_equals = true,
        help_heading = "General options",
        verbatim_doc_comment,
    )]
    pub version: String,

    /// This controls the action's verbosity in the workflow's logs.
    ///
    /// This option does not affect the verbosity of resulting
    /// thread comments or file annotations.
    #[arg(
        short = 'v',
        long,
        default_value = "info",
        help_heading = "General options"
    )]
    pub verbosity: Verbosity,
}

#[derive(Debug, Clone, Args)]
#[group(id = "Source options", multiple = true, required = false)]
pub struct SourceOptions {
    /// A comma-separated list of file extensions to analyze.
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_value = "c,h,C,H,cpp,hpp,cc,hh,c++,h++,cxx,hxx",
        value_parser = NonEmptyStringValueParser::new(),
        help_heading = "Source options"
    )]
    pub extensions: Vec<String>,

    /// The relative path to the repository root directory.
    ///
    /// This path is relative to the runner's `GITHUB_WORKSPACE`
    /// environment variable (or the current working directory if
    /// not using a CI runner).
    #[arg(short, long, default_value = ".", help_heading = "Source options")]
    pub repo_root: String,

    /// This controls what part of the files are analyzed.
    ///
    /// The following values are accepted:
    ///
    /// - `false`: All lines in a file are analyzed.
    /// - `true`: Only lines in the diff that contain additions are analyzed.
    /// - `diff`: All lines in the diff are analyzed (including unchanged
    ///   lines but not subtractions).
    #[arg(
        short,
        long,
        default_value = "true",
        value_parser = PossibleValuesParser::new(
            ["true", "on", "1", "false", "off", "0", "diff"],
        ).map(|s| <LinesChangedOnly as FromStr>::from_str(&s).unwrap()),
        help_heading = "Source options",
        verbatim_doc_comment,
    )]
    pub lines_changed_only: LinesChangedOnly,

    /// Set this option to false to analyze any source files in the repo.
    ///
    /// This is automatically enabled if
    /// [`--lines-changed-only`](#-l-lines-changed-only) is enabled.
    ///
    /// > [!NOTE]
    /// > The `GITHUB_TOKEN` should be supplied when running on a
    /// > private repository with this option enabled, otherwise the runner
    /// > does not not have the privilege to list the changed files for an event.
    /// >
    /// > See [Authenticating with the `GITHUB_TOKEN`](
    /// > https://docs.github.com/en/actions/reference/authentication-in-a-workflow).
    #[arg(
        short,
        long,
        default_value = "false",
        default_value_if("lines-changed-only", ArgPredicate::Equals("true".into()), "true"),
        value_parser = FalseyValueParser::new(),
        help_heading = "Source options",
        verbatim_doc_comment,
    )]
    pub files_changed_only: bool,

    /// Set this option with path(s) to ignore (or not ignore).
    ///
    /// - In the case of multiple paths, you can use `|` to separate each path.
    /// - There is no need to use `./` for each entry; a blank string (`''`)
    ///   represents the repo-root path.
    /// - This can also have files, but the file's path (relative to
    ///   the [`--repo-root`](#-r-repo-root)) has to be specified with the filename.
    /// - Submodules are automatically ignored. Hidden directories (beginning
    ///   with a `.`) are also ignored automatically.
    /// - Prefix a path with `!` to explicitly not ignore it. This can be
    ///   applied to a submodule's path (if desired) but not hidden directories.
    /// - Glob patterns are supported here. Path separators in glob patterns should
    ///   use `/` because `\` represents an escaped literal.
    #[arg(
        short,
        long,
        value_delimiter = '|',
        default_value = ".github|target",
        help_heading = "Source options",
        verbatim_doc_comment
    )]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Args)]
#[group(id = "Clang-format options", multiple = true, required = false)]
pub struct FormatOptions {
    /// The style rules to use.
    ///
    /// - Set this to `file` to have clang-format use the closest relative
    ///   .clang-format file.
    /// - Set this to a blank string (`''`) to disable using clang-format
    ///   entirely.
    ///
    /// > [!NOTE]
    /// > If this is not a blank string, then it is also passed to clang-tidy
    /// > (if [`--tidy_checks`](#-c-tidy-checks) is not `-*`).
    /// > This is done to ensure suggestions from both clang-tidy and
    /// > clang-format are consistent.
    #[arg(
        short,
        long,
        default_value = "llvm",
        help_heading = "Clang-format options",
        verbatim_doc_comment
    )]
    pub style: String,

    /// Similar to [`--ignore`](#-i-ignore) but applied
    /// exclusively to files analyzed by clang-format.
    #[arg(
        short = 'M',
        long,
        value_delimiter = '|',
        help_heading = "Clang-format options"
    )]
    pub ignore_format: Option<Vec<String>>,
}

#[derive(Debug, Clone, Args)]
#[group(id = "Clang-tidy options", multiple = true, required = false)]
pub struct TidyOptions {
    /// Similar to [`--ignore`](#-i-ignore) but applied
    /// exclusively to files analyzed by clang-tidy.
    #[arg(
        short = 'D',
        long,
        value_delimiter = '|',
        help_heading = "Clang-tidy options"
    )]
    pub ignore_tidy: Option<Vec<String>>,

    /// A comma-separated list of globs with optional `-` prefix.
    ///
    /// Globs are processed in order of appearance in the list.
    /// Globs without `-` prefix add checks with matching names to the set,
    /// globs with the `-` prefix remove checks with matching names from the set of
    /// enabled checks. This option's value is appended to the value of the 'Checks'
    /// option in a .clang-tidy file (if any).
    ///
    /// - It is possible to disable clang-tidy entirely by setting this option to
    ///   `'-*'`.
    /// - It is also possible to rely solely on a .clang-tidy config file by
    ///   specifying this option as a blank string (`''`).
    ///
    /// See also clang-tidy docs for more info.
    #[arg(
        short = 'c',
        long,
        default_value = "boost-*,bugprone-*,performance-*,readability-*,portability-*,modernize-*,clang-analyzer-*,cppcoreguidelines-*",
        help_heading = "Clang-tidy options",
        verbatim_doc_comment
    )]
    pub tidy_checks: String,

    /// The path that is used to read a compile command database.
    ///
    /// For example, it can be a CMake build directory in which a file named
    /// compile_commands.json exists (set `CMAKE_EXPORT_COMPILE_COMMANDS` to `ON`).
    /// When no build path is specified, a search for compile_commands.json will be
    /// attempted through all parent paths of the first input file. See [LLVM docs about
    /// setup tooling](https://clang.llvm.org/docs/HowToSetupToolingForLLVM.html)
    /// for an example of setting up Clang Tooling on a source tree.
    #[arg(
        short = 'p',
        long,
        value_name = "PATH",
        value_parser = value_parser!(PathBuf),
        help_heading = "Clang-tidy options",
    )]
    pub database: Option<PathBuf>,

    /// A string of extra arguments passed to clang-tidy for use as compiler arguments.
    ///
    /// This can be specified more than once for each
    /// additional argument. Recommend using quotes around the value and
    /// avoid using spaces between name and value (use `=` instead):
    ///
    /// ```shell
    /// cpp-linter --extra-arg="-std=c++17" --extra-arg="-Wall"
    /// ```
    #[arg(
        short = 'x',
        long,
        action = ArgAction::Append,
        help_heading = "Clang-tidy options",
        verbatim_doc_comment
    )]
    pub extra_arg: Vec<String>,
}

#[derive(Debug, Clone, Args)]
#[group(id = "Feedback options", multiple = true, required = false)]
pub struct FeedbackOptions {
    /// Set this option to true to enable the use of thread comments as feedback.
    ///
    /// Set this to `update` to update an existing comment if one exists;
    /// the value 'true' will always delete an old comment and post a new one
    /// if necessary.
    ///
    /// > [!NOTE]
    /// > To use thread comments, the `GITHUB_TOKEN` (provided by
    /// > Github to each repository) must be declared as an environment
    /// > variable.
    /// >
    /// > See [Authenticating with the `GITHUB_TOKEN`](
    /// > https://docs.github.com/en/actions/reference/authentication-in-a-workflow).
    #[arg(
        short = 'g',
        long,
        default_value = "false",
        value_parser = PossibleValuesParser::new(
            ["true", "on", "1", "false", "off", "0", "update"],
        ).map(|s| <ThreadComments as FromStr>::from_str(&s).unwrap()),
        help_heading = "Feedback options",
        verbatim_doc_comment,
    )]
    pub thread_comments: ThreadComments,

    /// Set this option to true or false to enable or disable the use of a
    /// thread comment that basically says 'Looks Good To Me' (when all checks pass).
    ///
    /// > [!IMPORTANT]
    /// > The [`--thread-comments`](#-g-thread-comments)
    /// > option also notes further implications.
    #[arg(
        short = 't',
        long,
        default_value_t = true,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
        verbatim_doc_comment,
    )]
    pub no_lgtm: bool,

    /// Set this option to true or false to enable or disable the use of
    /// a workflow step summary when the run has concluded.
    #[arg(
        short = 'w',
        long,
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
    )]
    pub step_summary: bool,

    /// Set this option to false to disable the use of
    /// file annotations as feedback.
    #[arg(
        short = 'a',
        long,
        default_value_t = true,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
    )]
    pub file_annotations: bool,

    /// Set to `true` to enable Pull Request reviews from clang-tidy.
    #[arg(
        short = 'd',
        long,
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
    )]
    pub tidy_review: bool,

    /// Set to `true` to enable Pull Request reviews from clang-format.
    #[arg(
        short = 'm',
        long,
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
    )]
    pub format_review: bool,

    /// Set to `true` to prevent Pull Request reviews from
    /// approving or requesting changes.
    #[arg(
        short = 'R',
        long,
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = FalseyValueParser::new(),
        help_heading = "Feedback options",
    )]
    pub passive_reviews: bool,
}

/// Converts the parsed value of the `--extra-arg` option into an optional vector of strings.
///
/// This is for adapting to 2 scenarios where `--extra-arg` is either
///
/// - specified multiple times
///     - each val is appended to a [`Vec`] (by clap crate)
/// - specified once with multiple space-separated values
///     - resulting [`Vec`] is made from splitting at the spaces between
/// - not specified at all (returns empty [`Vec`])
///
/// It is preferred that the values specified in either situation do not contain spaces and are
/// quoted:
///
/// ```shell
/// --extra-arg="-std=c++17" --extra-arg="-Wall"
/// # or equivalently
/// --extra-arg="-std=c++17 -Wall"
/// ```
///
/// The cpp-linter-action (for Github CI workflows) can only use 1 `extra-arg` input option, so
/// the value will be split at spaces.
pub fn convert_extra_arg_val(args: &[String]) -> Vec<String> {
    let mut val = args.iter();
    if val.len() == 1 {
        // specified once; split and return result
        val.next()
            .unwrap()
            .trim_matches('\'')
            .trim_matches('"')
            .split(' ')
            .map(|i| i.to_string())
            .collect()
    } else {
        // specified multiple times; just return
        val.map(|i| i.to_string()).collect()
    }
}

#[cfg(test)]
mod test {
    use super::{convert_extra_arg_val, Cli};
    use clap::Parser;

    #[test]
    fn error_on_blank_extensions() {
        let cli = Cli::try_parse_from(vec!["cpp-linter", "-e", "c,,h"]);
        assert!(cli.is_err());
        println!("{}", cli.unwrap_err());
    }

    #[test]
    fn extra_arg_0() {
        let args = Cli::parse_from(vec!["cpp-linter"]);
        let extras = convert_extra_arg_val(&args.tidy_options.extra_arg);
        assert!(extras.is_empty());
    }

    #[test]
    fn extra_arg_1() {
        let args = Cli::parse_from(vec!["cpp-linter", "--extra-arg='-std=c++17 -Wall'"]);
        let extra_args = convert_extra_arg_val(&args.tidy_options.extra_arg);
        assert_eq!(extra_args.len(), 2);
        assert_eq!(extra_args, ["-std=c++17", "-Wall"])
    }

    #[test]
    fn extra_arg_2() {
        let args = Cli::parse_from(vec![
            "cpp-linter",
            "--extra-arg=-std=c++17",
            "--extra-arg=-Wall",
        ]);
        let extra_args = convert_extra_arg_val(&args.tidy_options.extra_arg);
        assert_eq!(extra_args.len(), 2);
        assert_eq!(extra_args, ["-std=c++17", "-Wall"])
    }
}
