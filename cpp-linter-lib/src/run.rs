//! This module is the native backend of the cpp-linter package written in Rust.
//!
//! In python, this module is exposed as `cpp_linter.run` that has 1 function exposed:
//! `main()`.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// non-std crates
use log::{set_max_level, LevelFilter};
#[cfg(feature = "openssl-vendored")]
use openssl_probe;

// project specific modules/crates
use crate::clang_tools::{capture_clang_tools_output, ClangParams};
use crate::cli::{convert_extra_arg_val, get_arg_parser, LinesChangedOnly};
use crate::common_fs::FileFilter;
use crate::github_api::GithubApiClient;
use crate::logger::{self, end_log_group, start_log_group};
use crate::rest_api::{FeedbackInput, RestApiClient};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "openssl-vendored")]
fn probe_ssl_certs() {
    openssl_probe::init_ssl_cert_env_vars();
}

#[cfg(not(feature = "openssl-vendored"))]
fn probe_ssl_certs() {}

/// This is the backend entry point for console applications.
///
/// The idea here is that all functionality is implemented in Rust. However, passing
/// command line arguments is done differently in Python or Rust.
///
/// - In python, the ``sys.argv`` list is passed from the ``cpp_linter.entry_point.main()``
///   function to rust via the ``cpp_linter.run.main()`` binding (which wraps [`run_main()`]).
/// - In rust, the [`std::env::args`] is passed to [`run_main()`] in the binary
///   source `main.rs`.
///
/// This is done because of the way the python entry point is invoked. If [`std::env::args`]
/// is used instead of python's `sys.argv`, then the list of strings includes the entry point
/// alias ("path/to/cpp-linter.exe"). Thus, the parser in [`crate::cli`] will halt on an error
/// because it is not configured to handle positional arguments.
pub async fn run_main(args: Vec<String>) -> i32 {
    probe_ssl_certs();

    let arg_parser = get_arg_parser();
    let args = arg_parser.get_matches_from(args);

    if args.subcommand_matches("version").is_some() {
        println!("cpp-linter v{}", VERSION);
        return 0;
    }

    logger::init().unwrap();

    let version = args.get_one::<String>("version").unwrap();
    if version == "NO-VERSION" {
        log::error!("The `--version` arg is used to specify which version of clang to use.");
        log::error!("To get the cpp-linter version, use `cpp-linter version` sub-command.");
        return 1;
    }

    let root_path = args.get_one::<String>("repo-root").unwrap();
    if root_path != &String::from(".") {
        env::set_current_dir(Path::new(root_path)).unwrap();
    }

    let database_path = if let Some(database) = args.get_one::<String>("database") {
        if !database.is_empty() {
            Some(PathBuf::from(database).canonicalize().unwrap())
        } else {
            None
        }
    } else {
        None
    };

    let rest_api_client = GithubApiClient::new();
    let verbosity = args.get_one::<String>("verbosity").unwrap().as_str() == "debug";
    set_max_level(if verbosity || rest_api_client.debug_enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
    log::info!("Processing event {}", rest_api_client.event_name);

    let extensions = args
        .get_many::<String>("extensions")
        .unwrap()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let ignore = args
        .get_many::<String>("ignore")
        .unwrap()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let mut file_filter = FileFilter::new(&ignore, extensions.clone());
    file_filter.parse_submodules();

    let lines_changed_only = match args
        .get_one::<String>("lines-changed-only")
        .unwrap()
        .as_str()
    {
        "true" => LinesChangedOnly::On,
        "diff" => LinesChangedOnly::Diff,
        _ => LinesChangedOnly::Off,
    };
    let files_changed_only = args.get_flag("files-changed-only");

    start_log_group(String::from("Get list of specified source files"));
    let files = if lines_changed_only != LinesChangedOnly::Off || files_changed_only {
        // parse_diff(github_rest_api_payload)
        rest_api_client
            .get_list_of_changed_files(&file_filter)
            .await
    } else {
        // walk the folder and look for files with specified extensions according to ignore values.
        file_filter.list_source_files(".")
    };
    let mut arc_files = vec![];
    log::info!("Giving attention to the following files:");
    for file in files {
        log::info!("  ./{}", file.name.to_string_lossy().replace('\\', "/"));
        arc_files.push(Arc::new(Mutex::new(file)));
    }
    end_log_group();

    let user_inputs = FeedbackInput {
        style: args.get_one::<String>("style").unwrap().to_string(),
        no_lgtm: args.get_flag("no-lgtm"),
        step_summary: args.get_flag("step-summary"),
        thread_comments: args
            .get_one::<String>("thread-comments")
            .unwrap()
            .to_string(),
        file_annotations: args.get_flag("file-annotations"),
    };
    let ignore_tidy = args
        .get_many::<String>("ignore-tidy")
        .unwrap()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let ignore_format = args
        .get_many::<String>("ignore-format")
        .unwrap()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    let extra_args = convert_extra_arg_val(&args);
    let mut clang_params = ClangParams {
        tidy_checks: args.get_one::<String>("tidy-checks").unwrap().to_string(),
        lines_changed_only,
        database: database_path,
        extra_args,
        database_json: None,
        style: user_inputs.style.clone(),
        clang_tidy_command: None,
        clang_format_command: None,
        tidy_filter: FileFilter::new(&ignore_tidy, extensions.clone()),
        format_filter: FileFilter::new(&ignore_format, extensions),
    };
    capture_clang_tools_output(&mut arc_files, version, &mut clang_params).await;
    start_log_group(String::from("Posting feedback"));
    rest_api_client.post_feedback(&arc_files, user_inputs).await;
    end_log_group();
    0
}
