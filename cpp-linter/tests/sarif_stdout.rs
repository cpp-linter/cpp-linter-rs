#![cfg(feature = "bin")]
//! `--sarif-file -` in a local run: stdout must hold only the SARIF log.

use std::{env, fs, process::Command};

use tempfile::TempDir;

#[test]
fn sarif_to_stdout_is_valid_json() {
    let tmp = TempDir::with_prefix("cpp-linter-test").unwrap();
    for name in ["demo.cpp", "demo.hpp"] {
        fs::copy(format!("tests/demo/{name}"), tmp.path().join(name)).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_cpp-linter"))
        .current_dir(tmp.path())
        // a local run, not a CI run
        .env_remove("GITHUB_ACTIONS")
        .env_remove("GITEA_ACTIONS")
        .env_remove("FORGEJO_ACTIONS")
        .env_remove("GITLAB_CI")
        // checks fail in the demo, so don't exit with an error
        .env_remove("CPP_LINTER_STRICT_EXIT")
        .env_remove("PRE_COMMIT")
        .args([
            format!("-V={}", env::var("CLANG_VERSION").unwrap_or_default()),
            "--lines-changed-only=false".to_string(),
            "--files-changed-only=false".to_string(),
            "--repo-root=.".to_string(),
            "--style=llvm".to_string(),
            "--tidy-checks=-*,readability-braces-around-statements".to_string(),
            "--sarif-file=-".to_string(),
            // the generated patch must not be printed to stdout either
            "--fix".to_string(),
            "demo.cpp".to_string(),
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    let sarif: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!("stdout is not JSON ({e}):\n{stdout}\nstderr:\n{stderr}")
    });
    assert_eq!(sarif["version"], "2.1.0");
    assert!(!sarif["runs"][0]["results"].as_array().unwrap().is_empty());

    // logs, download progress bars (if clang tools were installed),
    // file annotations (incl. clang-format's) and the patch go to stderr
    assert!(stderr.contains("Wrote SARIF log to stdout"), "{stderr}");
    assert!(stderr.contains("does not conform to"), "{stderr}");
    assert!(stderr.contains("Generated patch content:"), "{stderr}");
    assert!(!tmp.path().join("-").exists());
}
