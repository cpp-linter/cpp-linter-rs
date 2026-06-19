use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use reqwest::blocking::ClientBuilder;

const URL: &str = "https://github.com/cpp-linter/clang-tools-static-binaries/releases/latest/download/versions.json";

#[derive(Debug, serde::Deserialize)]
struct VersionInfo {
    release_tag: String,
    llvm_versions: std::collections::HashMap<u8, String>,
}
fn main() {
    let pre_seed = PathBuf::from("versions.json");
    let version_info_str = if pre_seed.exists()
        && let Ok(metadata) = fs::metadata(&pre_seed)
        && metadata.modified().is_ok_and(|d| {
            SystemTime::now()
                .duration_since(d)
                // repopulate cached file in case of error
                .unwrap_or(Duration::from_hours(25))
                < Duration::from_hours(24)
        }) {
        println!("cargo:warning=Using pre-seeded version info from {pre_seed:?}");
        fs::read_to_string(&pre_seed).unwrap()
    } else {
        let versions = ClientBuilder::new()
            .user_agent("cpp-linter-rs/clang-tools-manager")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap()
            .get(URL)
            .send()
            .unwrap()
            .text()
            .unwrap();
        let mut cached_seed = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pre_seed)
            .unwrap();
        cached_seed.write_all(versions.as_bytes()).unwrap();
        cached_seed.set_modified(SystemTime::now()).unwrap();
        versions
    };
    let version_info: VersionInfo = serde_json::from_str(&version_info_str).unwrap();
    let (min_ver, max_ver) = {
        let (mut min_v, mut max_v) = (None, None);
        for ver in version_info.llvm_versions.keys() {
            if min_v.is_none_or(|v| v > ver) {
                min_v = Some(ver);
            }
            if max_v.is_none_or(|v| v < ver) {
                max_v = Some(ver);
            }
        }
        (min_v.unwrap(), max_v.unwrap())
    };
    println!(
        "cargo:rustc-env=CLANG_TOOLS_TAG={}",
        version_info.release_tag
    );
    println!("cargo:rustc-env=MIN_CLANG_TOOLS_VERSION={min_ver}");
    println!("cargo:rustc-env=MAX_CLANG_TOOLS_VERSION={max_ver}");
}
