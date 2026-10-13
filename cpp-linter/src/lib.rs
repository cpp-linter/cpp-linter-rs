#![doc(
    html_logo_url = "https://raw.githubusercontent.com/cpp-linter/cpp-linter-rs/main/docs/docs/images/logo.png"
)]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp-linter-rs/raw/main/docs/docs/images/favicon.ico"
)]
#![doc = include_str!("../README.md")]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    missing_docs
)]

// project specific modules/crates
pub mod clang_tools;
pub mod cli;
pub mod common_fs;
pub mod error;
mod git;
pub mod rest_client;
pub mod run;
pub mod sarif;

#[cfg(test)]
pub(crate) mod test_common {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use std::{fs, path::PathBuf};

    /// helper to avoid concurrent writes by executing (& processing the output of)
    /// clang tools on the same test assets.
    pub fn setup_tmp_workspace() -> tempfile::TempDir {
        let tmp_workspace = tempfile::TempDir::with_prefix("cpp-linter-unit-tests_").unwrap();
        let demo_path = tmp_workspace.path().join("demo");
        fs::create_dir(&demo_path).unwrap();
        for asset in ["demo.cpp", "demo.hpp"] {
            fs::copy(
                PathBuf::from("tests/demo").join(asset),
                demo_path.join(asset),
            )
            .unwrap();
        }
        tmp_workspace
    }

    /// Poison the given `mutex` by panicking in a thread that holds its lock.
    pub fn poison_mutex<T: Send + 'static>(mutex: &std::sync::Arc<std::sync::Mutex<T>>) {
        let mutex = mutex.clone();
        std::thread::spawn(move || {
            let _lock = mutex.lock().unwrap();
            panic!("intentionally poison the mutex");
        })
        .join()
        .unwrap_err();
    }

    /// Create an executable shell script (in the given `dir`) that can be used in
    /// place of a clang tool. The script's `body` is executed by `/bin/sh`.
    ///
    /// This is useful to simulate unusual behavior of clang tools.
    #[cfg(unix)]
    pub fn make_fake_tool(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}
