//! This module is the native backend of the cpp-linter package written in Rust.
//!
//! In python, this module is exposed as `cpp_linter.run` that has 1 function exposed:
//! `main()`.
#![cfg(feature = "bin")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

// non-std crates
use anyhow::{Context, Result, anyhow};
use clang_tools_manager::{
    RequestedVersion,
    logger::{set_log_to_stderr, try_init_logger},
};
use clap::Parser;
use log::{LevelFilter, set_max_level};

// project specific modules/crates
use crate::{
    clang_tools::capture_clang_tools_output,
    cli::{ClangParams, Cli, CliCommand, FeedbackInput, LinesChangedOnly, ignore_patterns},
    common_fs::{FileObj, mk_path_abs},
    rest_client::RestClient,
    sarif::SarifLog,
};
use git_bot_feedback::FileFilter;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Can the given `file` (relative to `repo_root`) be opened and read as a regular file?
///
/// If not, a warning is logged that says why the file is skipped.
fn is_readable_file(repo_root: &Path, file: &FileObj) -> bool {
    let path = repo_root.join(&file.name);
    let reason = match fs::File::open(&path).and_then(|f| f.metadata()) {
        Ok(meta) if meta.is_file() => return true,
        Ok(_) => "it is not a regular file".to_string(),
        Err(e) => e.to_string(),
    };
    log::warn!(
        "Skipping {}: {reason}",
        file.name.to_string_lossy().replace('\\', "/")
    );
    false
}

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

    let sarif_to_stdout = cli
        .feedback_options
        .sarif_file
        .as_deref()
        .is_some_and(SarifLog::is_stdout);
    // Until the client is known, keep log messages off stdout if it is meant for the SARIF log.
    // This also resets the choice made by a previous call (from the Python or Node.js bindings).
    set_log_to_stderr(sarif_to_stdout);
    try_init_logger();

    let repo_root_abs = mk_path_abs(&cli.source_options.repo_root).with_context(|| {
        format!(
            "Failed to canonicalize the repo root path: {}",
            cli.source_options.repo_root.to_string_lossy()
        )
    })?;
    let mut rest_api_client = RestClient::new(&repo_root_abs)?;
    // `--sarif-file -` writes the SARIF log to stdout, so in a local run everything else
    // goes to stderr to keep stdout valid JSON. In CI, logs stay on stdout.
    set_log_to_stderr(sarif_to_stdout && rest_api_client.client_kind() == "local");
    set_max_level(
        if cli.general_options.verbosity.is_debug() || rest_api_client.is_debug_enabled() {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        },
    );
    if cli.general_options.jobs.is_some() {
        log::warn!(
            "The `--jobs` option is deprecated and ignored. Files are always analyzed in parallel."
        );
    }
    let is_pr = rest_api_client.is_pr();
    if !is_pr && cli.feedback_options.is_pr_review() && rest_api_client.client_kind() == "local" {
        log::warn!("Pull request reviews are only posted when running in a supported CI platform.");
    }

    let mut file_filter = FileFilter::new(
        &ignore_patterns(&cli.source_options.ignore),
        &cli.source_options
            .extensions
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
        None,
    );
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
    let mut files = if !matches!(cli.source_options.lines_changed_only, LinesChangedOnly::Off)
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
        if is_pr && cli.feedback_options.is_pr_review() {
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
    // The file lists from git and the directory walk are unordered.
    // Sort them, so the order of feedback (comments, annotations, etc) is deterministic.
    files.sort_by(|a, b| a.name.cmp(&b.name));
    // A listed file can be impossible to read: a dangling symlink (eg. into a submodule
    // that is not checked out), a file removed by an earlier CI step, or a directory.
    // Skip it with a warning instead of stopping the whole run.
    files.retain(|file| is_readable_file(&cli.source_options.repo_root, file));
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
    let user_inputs = FeedbackInput::from(&cli);
    // delete old patch file
    if user_inputs.patch_path.exists() {
        std::fs::remove_file(&user_inputs.patch_path)
            .with_context(|| "Failed to remove old patch file from previous runs.")?;
    }
    // add gitignore file in project cache dir
    std::fs::write(
        cache_dir.join(".gitignore"),
        "# Automatically created by cpp-linter\n*\n",
    )
    .with_context(|| "Failed to write .cpp-linter-cache/.gitignore file")?;
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
    if (cli.general_options.strict || env::var("PRE_COMMIT").is_ok_and(|v| v == "1"))
        && checks_failed > 0
    {
        return Err(anyhow!("Some checks did not pass"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test {
    #![allow(clippy::unwrap_used)]

    use super::run_main;
    use crate::{cli::ClangParams, test_common::setup_tmp_workspace};
    use clang_tools_manager::logger::is_log_to_stderr;
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

    // The patch is written to `--gen-patch-path`, and a stale patch there is replaced.
    #[tokio::test]
    async fn custom_patch_path() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        let patch_path = tmp_workspace.path().join("out").join("fixes.patch");
        std::fs::create_dir_all(patch_path.parent().unwrap()).unwrap();
        std::fs::write(&patch_path, "stale").unwrap();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--tidy-checks=-*".to_string(),
            "--gen-patch-path".to_string(),
            patch_path.to_str().unwrap().to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
            "demo/demo.cpp".to_string(),
        ])
        .await
        .unwrap();
        let patch = std::fs::read_to_string(&patch_path).unwrap();
        assert!(patch.starts_with("--- a/demo/demo.cpp\n"), "{patch}");
        let default_patch = ClangParams::default_patch_path(tmp_workspace.path());
        assert!(!default_patch.exists());
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn pr_review_outside_ci() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        unsafe {
            env::set_var("GITHUB_ACTIONS", "false");
        }
        // a PR review is requested, but only a warning is logged outside CI
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--pr-review".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
            "demo/demo.cpp".to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn no_mod_sys() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--no-mod-sys".to_string(),
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

    #[tokio::test]
    async fn strict_exit() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        unsafe {
            env::remove_var("PRE_COMMIT");
            env::remove_var("CPP_LINTER_STRICT_EXIT");
        }
        let tmp_workspace = setup_tmp_workspace();
        let args = |strict: Option<&str>| {
            let mut args = vec![
                "cpp-linter".to_string(),
                "--lines-changed-only".to_string(),
                "false".to_string(),
                "--repo-root".to_string(),
                tmp_workspace.path().to_str().unwrap().to_string(),
            ];
            if let Some(value) = strict {
                args.push(value.to_string());
            }
            args
        };
        // checks fail on the demo files, but the default exit code is still 0
        run_main(args(None)).await.unwrap();
        run_main(args(Some("--strict"))).await.unwrap_err();
        run_main(args(Some("--strict=false"))).await.unwrap();
        unsafe {
            env::set_var("CPP_LINTER_STRICT_EXIT", "true");
        }
        run_main(args(None)).await.unwrap_err();
        drop(tmp_gh_out);
    }

    #[test]
    fn unreadable_files_are_skipped() {
        use super::is_readable_file;
        use crate::common_fs::FileObj;
        use std::path::PathBuf;

        let tmp_workspace = setup_tmp_workspace();
        let root = tmp_workspace.path();
        assert!(is_readable_file(
            root,
            &FileObj::new(PathBuf::from("demo/demo.cpp"))
        ));
        // a missing file (eg. removed by an earlier CI step)
        assert!(!is_readable_file(
            root,
            &FileObj::new(PathBuf::from("demo/missing.cpp"))
        ));
        // a directory with a source file extension
        std::fs::create_dir(root.join("folder.cpp")).unwrap();
        assert!(!is_readable_file(
            root,
            &FileObj::new(PathBuf::from("folder.cpp"))
        ));
    }

    /// A dangling symlink (eg. into a submodule that is not checked out) does not stop the run.
    #[cfg(unix)]
    #[tokio::test]
    async fn dangling_symlink() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        std::os::unix::fs::symlink(
            "not-checked-out/missing.cpp",
            tmp_workspace.path().join("demo/dangling.cpp"),
        )
        .unwrap();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l=false".to_string(),
            "--style=llvm".to_string(),
            "--tidy-checks=-*".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn sarif_to_stdout() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "--lines-changed-only".to_string(),
            "false".to_string(),
            "--style".to_string(),
            String::new(),
            "--sarif-file".to_string(),
            "-".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
        ])
        .await
        .unwrap();
        // no file named `-` is created
        assert!(!std::path::Path::new("-").exists());
        assert!(!tmp_workspace.path().join("-").exists());
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn sarif_to_stdout_then_not() {
        unsafe {
            // a local run
            env::remove_var("GITHUB_ACTIONS");
        }
        let tmp_workspace = setup_tmp_workspace();
        let args = |sarif_file: &str| {
            vec![
                "cpp-linter".to_string(),
                "-l=false".to_string(),
                "-f=false".to_string(),
                "--style=".to_string(),
                "--tidy-checks=-*".to_string(),
                format!("--sarif-file={sarif_file}"),
                "--repo-root".to_string(),
                tmp_workspace.path().to_str().unwrap().to_string(),
                "demo/demo.cpp".to_string(),
            ]
        };
        run_main(args("-")).await.unwrap();
        assert!(is_log_to_stderr());
        // a later call (like from the Python or Node.js bindings) without `--sarif-file -`
        // writes log messages to stdout again
        let sarif_path = tmp_workspace.path().join("cpp-linter.sarif");
        run_main(args(sarif_path.to_str().unwrap())).await.unwrap();
        assert!(!is_log_to_stderr());
        assert!(sarif_path.exists());
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

    // v1's `--jobs` is accepted and ignored (with a warning) instead of failing to parse.
    #[tokio::test]
    async fn deprecated_jobs() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            "--style".to_string(),
            String::new(),
            "--tidy-checks=-*".to_string(),
            "-j".to_string(),
            "2".to_string(),
        ])
        .await
        .unwrap();
        drop(tmp_gh_out);
    }

    #[tokio::test]
    async fn only_not_ignored() {
        let tmp_gh_out = setup_tmp_gh_out_path();
        let tmp_workspace = setup_tmp_workspace();
        run_main(vec![
            "cpp-linter".to_string(),
            "-l".to_string(),
            "false".to_string(),
            // only specify a not-ignored pattern, so the ignored set is empty
            "-i=!demo".to_string(),
            "--repo-root".to_string(),
            tmp_workspace.path().to_str().unwrap().to_string(),
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
