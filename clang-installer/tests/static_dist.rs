use std::env;

use clang_installer::{ClangTool, StaticDistDownloader};
use semver::VersionReq;
use tempfile::TempDir;
mod common;

async fn setup(ver_spec: &str) {
    common::initialize_logger();
    log::set_max_level(log::LevelFilter::Debug);

    let tmp_cache_dir = TempDir::new().unwrap();
    // Override cache directory to ensure test isolation and avoid interference with other tests' caches
    unsafe {
        env::set_var("CPP_LINTER_CACHE", tmp_cache_dir.path());
    }

    let tool = ClangTool::ClangFormat;
    let version_req = VersionReq::parse(ver_spec).unwrap();

    let result = StaticDistDownloader::download_tool(&tool, &version_req, None)
        .await
        .unwrap();
    println!(
        "Downloaded clang-format from static distribution to {}",
        result.to_string_lossy()
    );
    let out_ver = tool.capture_version(&result).unwrap();
    log::info!("The downloaded clang-format version is {}", out_ver);
    assert!(
        version_req.matches(&out_ver),
        "The downloaded clang-format version {} does not satisfy the requirement {}",
        out_ver,
        version_req
    );

    // retry using cache
    let cache_result = StaticDistDownloader::download_tool(&tool, &version_req, None)
        .await
        .unwrap();
    assert_eq!(result, cache_result);
}

#[tokio::test]
async fn download_clang_format_17() {
    setup("17").await;
}

#[tokio::test]
async fn download_clang_format_12_0_1() {
    setup("=12.0.1").await;
}
