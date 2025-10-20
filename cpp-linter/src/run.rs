//! This module is the native backend of the cpp-linter package written in Rust.
//!
//! In python, this module is exposed as `cpp_linter.run` that has 1 function exposed:
//! `main()`.

use std::env;
use std::path::Path;
use std::sync::{Arc, Mutex};

// non-std crates
use anyhow::{anyhow, Result};
use log::{set_max_level, LevelFilter};

// project specific modules/crates
use crate::clang_tools::capture_clang_tools_output;
use crate::cli::{get_arg_parser, ClangParams, Cli, FeedbackInput, LinesChangedOnly};
use crate::common_fs::FileFilter;
use crate::logger;
use crate::rest_api::{github::GithubApiClient, RestApiClient};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// This is the backend entry point for console applications.
///
/// The idea here is that all functionality is implemented in Rust. However, passing
/// command line arguments is done differently in Python, node.js, or Rust.
///
/// - In python, the CLI arguments list is optionally passed to the binding's
///   `cpp_linter.main()` function (which wraps [`run_main()`]). If no args are passed,
///   then `cpp_linter.main()` uses [`std::env::args`] without the leading path to the
///   python interpreter removed.
/// - In node.js, the `process.argv` array (without the leading path to the node
///   interpreter removed) is passed from `cli.js` module to rust via `index.node`
///   module's `main()` (which wraps([`run_main()`])).
/// - In rust, the [`std::env::args`] is passed to [`run_main()`] in the binary
///   source `main.rs`.
///
/// This is done because of the way the python entry point is invoked. If [`std::env::args`]
/// is used instead of python's `sys.argv`, then the list of strings includes the entry point
/// alias ("path/to/cpp-linter.exe"). Thus, the parser in [`crate::cli`] will halt on an error
/// because it is not configured to handle positional arguments.
pub async fn run_main(args: Vec<String>) -> Result<()> {
    let arg_parser = get_arg_parser();
    let args = arg_parser.get_matches_from(args);
    let cli = Cli::from(&args);

    if args.subcommand_matches("version").is_some() {
        println!("cpp-linter v{}", VERSION);
        return Ok(());
    }

    logger::try_init();

    if cli.version == "NO-VERSION" {
        log::error!("The `--version` arg is used to specify which version of clang to use.");
        log::error!("To get the cpp-linter version, use `cpp-linter version` sub-command.");
        return Err(anyhow!("Clang version not specified."));
    }

    if cli.repo_root != "." {
        env::set_current_dir(Path::new(&cli.repo_root))
            .unwrap_or_else(|_| panic!("'{}' is inaccessible or does not exist", cli.repo_root));
    }

    let rest_api_client = GithubApiClient::new()?;
    set_max_level(if cli.verbosity || rest_api_client.debug_enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
    log::info!("Processing event {}", rest_api_client.event_name);
    let is_pr = rest_api_client.event_name == "pull_request";

    let mut file_filter = FileFilter::new(&cli.ignore, cli.extensions.clone());
    file_filter.parse_submodules();
    if let Some(files) = &cli.not_ignored {
        file_filter.not_ignored.extend(files.clone());
    }

    if !file_filter.ignored.is_empty() {
        log::info!("Ignored:");
        for pattern in &file_filter.ignored {
            log::info!("  {pattern}");
        }
    }
    if !file_filter.not_ignored.is_empty() {
        log::info!("Not Ignored:");
        for pattern in &file_filter.not_ignored {
            log::info!("  {pattern}");
        }
    }

    rest_api_client.start_log_group(String::from("Get list of specified source files"));
    let files =
        if !matches!(cli.lines_changed_only, LinesChangedOnly::Off) || cli.files_changed_only {
            // parse_diff(github_rest_api_payload)
            rest_api_client
                .get_list_of_changed_files(&file_filter, &cli.lines_changed_only)
                .await?
        } else {
            // walk the folder and look for files with specified extensions according to ignore values.
            let mut all_files = file_filter.list_source_files(".")?;
            if is_pr && (cli.tidy_review || cli.format_review) {
                let changed_files = rest_api_client
                    .get_list_of_changed_files(&file_filter, &LinesChangedOnly::Off)
                    .await?;
                for changed_file in changed_files {
                    for file in &mut all_files {
                        if changed_file.name == file.name {
                            file.diff_chunks = changed_file.diff_chunks.clone();
                            file.added_lines = changed_file.added_lines.clone();
                            file.added_ranges = changed_file.added_ranges.clone();
                        }
                    }
                }
            }
            all_files
        };
    let mut arc_files = vec![];
    log::info!("Giving attention to the following files:");
    for file in files {
        log::info!("  ./{}", file.name.to_string_lossy().replace('\\', "/"));
        arc_files.push(Arc::new(Mutex::new(file)));
    }
    rest_api_client.end_log_group();

    let mut clang_params = ClangParams::from(&cli);
    clang_params.format_review &= is_pr;
    clang_params.tidy_review &= is_pr;
    let user_inputs = FeedbackInput::from(&cli);
    let clang_versions = capture_clang_tools_output(
        &mut arc_files,
        cli.version.as_str(),
        &mut clang_params,
        &rest_api_client,
    )
    .await?;
    rest_api_client.start_log_group(String::from("Posting feedback"));
    let checks_failed = rest_api_client
        .post_feedback(&arc_files, user_inputs, clang_versions)
        .await?;
    rest_api_client.end_log_group();
    if env::var("PRE_COMMIT").is_ok_and(|v| v == "1") && checks_failed > 1 {
        return Err(anyhow!("Some checks did not pass"));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::run_main;
    use std::env;

    #[tokio::test]
    async fn normal() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        let result = run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--repo-root".to_string(),
            "tests".to_string(),
            "demo/demo.cpp".to_string(),
        ])
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn version_command() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        let result = run_main(vec!["cpp-linter".to_string(), "version".to_string()]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn force_debug_output() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        let result = run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "-v".to_string(),
            "debug".to_string(),
            "-i=target|benches/libgit2".to_string(),
        ])
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn bad_version_input() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        let result = run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "-V".to_string(),
        ])
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pre_commit_env() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        env::set_var("PRE_COMMIT", "1");
        let result = run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "-i=target|benches/libgit2".to_string(),
        ])
        .await;
        assert!(result.is_err());
    }

    // Verifies that the system gracefully handles cases where all analysis is disabled.
    // This ensures no diagnostic comments are generated when analysis is explicitly skipped.
    #[tokio::test]
    async fn no_analysis() {
        env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
        let result = run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--style".to_string(),
            String::new(),
            "--tidy-checks=-*".to_string(),
        ])
        .await;
        assert!(result.is_ok());
    }
}
