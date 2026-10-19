use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use reqwest::blocking::ClientBuilder;

/// The index of all static binaries, one release per LLVM version.
const URL: &str = "https://cpp-linter.github.io/clang-tools-static-binaries/index.json";

/// The only index format this build script understands.
const INDEX_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, serde::Deserialize)]
struct StaticBinariesIndex {
    schema_version: u8,
    versions: BTreeMap<u8, VersionEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct VersionEntry {
    /// The release tag that holds this LLVM version's binaries.
    tag: String,
}

fn main() {
    let pre_seed = PathBuf::from("static-binaries-index.json");
    let index_str = if pre_seed.exists()
        && let Ok(metadata) = fs::metadata(&pre_seed)
        // if this is a docs.rs build, then use the index present in the package.
        && (env::var_os("DOCS_RS").is_some()
            // otherwise, check if the file is not too old (24 hours)
            || metadata.modified().is_ok_and(|d| {
                SystemTime::now()
                    .duration_since(d)
                    // repopulate cached file in case of error
                    .unwrap_or(Duration::from_hours(25))
                    < Duration::from_hours(24)
            })) {
        println!("cargo:warning=Using pre-seeded static binaries index from {pre_seed:?}");
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
    let index: StaticBinariesIndex = serde_json::from_str(&index_str).unwrap();
    assert_eq!(
        index.schema_version, INDEX_SCHEMA_VERSION,
        "Unsupported schema version of {URL}"
    );
    let min_ver = index
        .versions
        .keys()
        .next()
        .expect("index lists no versions");
    let max_ver = index.versions.keys().next_back().unwrap();
    println!("cargo:rustc-env=MIN_CLANG_TOOLS_VERSION={min_ver}");
    println!("cargo:rustc-env=MAX_CLANG_TOOLS_VERSION={max_ver}");

    // Each LLVM version is published in its own release.
    let release_tags = index
        .versions
        .iter()
        .map(|(major, entry)| format!("    ({major}, {:?}),\n", entry.tag))
        .collect::<String>();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(
        out_dir.join("release_tags.rs"),
        format!("&[\n{release_tags}]\n"),
    )
    .unwrap();
}
