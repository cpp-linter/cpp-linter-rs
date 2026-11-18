#![cfg(feature = "bin")]
//! `--code-quality-file -` in a local run: stdout must hold only the Code Quality report.

use std::{env, fs, process::Command};

use tempfile::TempDir;

#[test]
fn code_quality_to_stdout_is_valid_json() {
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
            "--code-quality-file=-".to_string(),
            // the generated patch must not be printed to stdout either
            "--fix".to_string(),
            "demo.cpp".to_string(),
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!("stdout is not JSON ({e}):\n{stdout}\nstderr:\n{stderr}")
    });
    let issues = report.as_array().unwrap();
    // both clang-tidy and clang-format findings are in the report
    assert!(issues.iter().any(|i| i["check_name"] == "clang-format"));
    assert!(
        issues
            .iter()
            .any(|i| i["check_name"] == "readability-braces-around-statements")
    );

    // logs, download progress bars (if clang tools were installed),
    // file annotations and the patch go to stderr
    assert!(stderr.contains("Code Quality report on stdout"), "{stderr}");
    assert!(stderr.contains("does not conform to"), "{stderr}");
    assert!(stderr.contains("Generated patch content:"), "{stderr}");
    assert!(!tmp.path().join("-").exists());
}
