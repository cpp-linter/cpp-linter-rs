//! This module is the native backend of the cpp-linter package written in Rust.
//!
//! In python, this module is exposed as `cpp_linter.run` that has 1 function exposed:
//! `main()`.
#![cfg(feature = "bin")]

use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};

// non-std crates
use anyhow::{Context, Result, anyhow};
use clang_tools_manager::{RequestedVersion, logger::try_init_logger};
use clap::Parser;
use log::{LevelFilter, set_max_level};

// project specific modules/crates
use crate::{
    clang_tools::capture_clang_tools_output,
    cli::{ClangParams, Cli, CliCommand, FeedbackInput, LinesChangedOnly},
    common_fs::{FileObj, mk_path_abs},
    rest_client::RestClient,
};
use git_bot_feedback::FileFilter;

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
    let mut cli = Cli::parse_from(args);

    if matches!(cli.commands, Some(CliCommand::Version))
        || cli.general_options.version == RequestedVersion::NoValue
    {
        println!("cpp-linter v{}", VERSION);
        return Ok(());
    }

    try_init_logger();

    let mut rest_api_client = RestClient::new()?;
    set_max_level(
        if cli.general_options.verbosity.is_debug() || rest_api_client.is_debug_enabled() {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        },
    );
    let is_pr = rest_api_client.is_pr();

    let mut file_filter = FileFilter::new(
        &cli.source_options
            .ignore
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
        &cli.source_options
            .extensions
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
        None,
    );
    let repo_root_abs = mk_path_abs(&cli.source_options.repo_root).with_context(|| {
        format!(
            "Failed to canonicalize the repo root path: {}",
            cli.source_options.repo_root.to_string_lossy()
        )
    })?;
    let gitmodules = repo_root_abs.join(".gitmodules");
    cli.source_options.repo_root = repo_root_abs;
    file_filter.parse_submodules(Some(gitmodules.as_path()));
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

    rest_api_client.start_log_group("Get list of specified source files");
    let files = if !matches!(cli.source_options.lines_changed_only, LinesChangedOnly::Off)
        || cli.source_options.files_changed_only
    {
        // parse_diff(github_rest_api_payload)
        rest_api_client
            .get_list_of_changed_files(
                &file_filter,
                &cli.source_options.lines_changed_only.clone().into(),
                &cli.source_options.diff_base,
                cli.source_options.ignore_index,
            )
            .await?
    } else {
        // walk the folder and look for files with specified extensions according to ignore values.
        let mut all_files: Vec<FileObj> = file_filter
            .walk_dir(&cli.source_options.repo_root)?
            .into_iter()
            .map(|file_name| {
                let file_path = PathBuf::from(&file_name);
                FileObj::new(file_path)
            })
            .collect();
        if is_pr && cli.feedback_options.pr_review {
            let changed_files = rest_api_client
                .get_list_of_changed_files(
                    &file_filter,
                    &LinesChangedOnly::Off.into(),
                    &cli.source_options.diff_base,
                    cli.source_options.ignore_index,
                )
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
    rest_api_client.end_log_group("Get list of specified source files");

    let clang_params = ClangParams::from(&cli);
    // mkdir -p .cpp-linter-cache/
    let cache_dir = clang_params.repo_root.join(ClangParams::CACHE_DIR);
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| "Failed to create a local cache directory.")?;
    // delete old patch file
    let patch_file = cache_dir.join(ClangParams::AUTO_FIX_PATCH);
    if patch_file.exists() {
        std::fs::remove_file(&patch_file)
            .with_context(|| "Failed to remove old patch file from previous runs.")?;
    }
    // add gitignore file in project cache dir
    std::fs::write(
        cache_dir.join(".gitignore"),
        "# Automatically created by cpp-linter\n*\n",
    )
    .with_context(|| "Failed to write .cpp-linter-cache/.gitignore file")?;
    let user_inputs = FeedbackInput::from(&cli);
    let clang_versions = capture_clang_tools_output(
        &arc_files,
        &cli.general_options.version,
        clang_params,
        &rest_api_client,
        if cli.general_options.no_mod_sys {
            false // explicitly false
        } else {
            cli.general_options.mod_sys // explicitly true
                || env::var("CI").is_ok_and(|v| ["true", "on", "1"].contains(&v.to_lowercase().as_str())) // implicitly true in CI environments
        },
    )
    .await?;
    rest_api_client.start_log_group("Posting feedback");
    let checks_failed = rest_api_client
        .post_feedback(&arc_files, user_inputs, clang_versions)
        .await?;
    rest_api_client.end_log_group("Posting feedback");
    if env::var("PRE_COMMIT").is_ok_and(|v| v == "1") && checks_failed > 1 {
        return Err(anyhow!("Some checks did not pass"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test {
    #![allow(clippy::unwrap_used)]

    use super::run_main;
    use crate::{cli::ClangParams, test_common::setup_tmp_workspace};
    use std::env;

    /// helper to avoid writing to the same GITHUB_OUTPUT file in parallel-running tests.
    fn setup_tmp_gh_out_path() -> tempfile::NamedTempFile {
        let gh_out_path = tempfile::NamedTempFile::new().unwrap();
        unsafe {
            env::set_var(
                "GITHUB_OUTPUT",
                gh_out_path.path().to_string_lossy().to_string(),
            );
        }
        gh_out_path
    }

    #[tokio::test]
    async fn normal() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
            "demo/demo.cpp".to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn version_command() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        run_main(vec!["cpp-linter".to_string(), "version".to_string()])
            .await
            .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn force_debug_output() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();

        // create a dummy patch file to ensure it is deleted (in code coverage).
        let cache_dir = tmp_workspace.path().join(ClangParams::CACHE_DIR);
        std::fs::create_dir_all(&cache_dir).unwrap();
        let patch_path = cache_dir.join(ClangParams::AUTO_FIX_PATCH);
        std::fs::write(&patch_path, "").unwrap();

        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "-v".to_string(),
            "-i=target|benches/libgit2".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn no_version_input() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "-V".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn pre_commit_env() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        unsafe {
            env::set_var("PRE_COMMIT", "1");
        }
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "--lines-changed-only".to_string(),
            "false".to_string(),
            "-v".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
        ])
        .await
        .unwrap_err();
        drop(tmp_gh_out);
    }

    // Verifies that the system gracefully handles cases where all analysis is disabled.
    // This ensures no diagnostic comments are generated when analysis is explicitly skipped.
    #[tokio::test]
    async fn no_analysis() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--style".to_string(),
            String::new(),
            "--tidy-checks=-*".to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn bad_repo_root() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        run_main(vec![
            "cpp-linter".to_string(),
            "--repo-root".to_string(),
            "non-existent_path".to_string(),
        ])
        .await
        .unwrap_err();
        drop(tmp_gh_out);
    }
}
