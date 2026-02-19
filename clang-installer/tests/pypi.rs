use std::{env, process::Command};

use semver::VersionReq;

use clang_installer::{ClangTool, PyPiDownloader};
use tempfile::TempDir;
mod common;

async fn setup() {
    common::initialize_logger();
    log::set_max_level(log::LevelFilter::Debug);

    let tmp_cache_dir = TempDir::new().unwrap();
    // Override cache directory to ensure test isolation and avoid interference with other tests' caches
    unsafe {
        env::set_var("CPP_LINTER_CACHE", tmp_cache_dir.path());
    }

    let tool = ClangTool::ClangFormat;
    let version_req = VersionReq::parse("17").unwrap();

    let result = PyPiDownloader::download_tool(&tool, &version_req)
        .await
        .unwrap();
    println!(
        "Downloaded clang-format from PyPI at: {:?}",
        result.to_string_lossy()
    );
    assert!(
        Command::new(&result)
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );

    // retry using cache
    let cache_result = PyPiDownloader::download_tool(&tool, &version_req)
        .await
        .unwrap();
    assert_eq!(result, cache_result);
}

#[tokio::test]
async fn download_clang_format() {
    setup().await;
}
