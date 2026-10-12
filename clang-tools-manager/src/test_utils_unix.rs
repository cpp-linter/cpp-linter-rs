//! Helpers shared by unit tests.
//!
//! These helpers mutate process-wide state (like the `PATH` environment variable).
//! This is safe because `cargo nextest` runs each test in a separate process.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// Writes an executable `#!/bin/sh` script named `name` (with the given `body`) into `dir`.
///
/// The `body` should only use shell builtins because the `PATH` is
/// expected to be isolated with [`isolate_path()`].
pub fn fake_exe(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// Writes a fake clang tool executable (named `name`) into `dir`
/// that reports the given `version` when invoked.
pub fn fake_clang_tool(dir: &Path, name: &str, version: &str) -> PathBuf {
    fake_exe(dir, name, &format!("echo \"{name} version {version}\""))
}

/// Sets the `PATH` environment variable to only the given `dir`.
///
/// This ensures no real executable (like `sudo` or a package manager)
/// can be invoked by the code under test.
pub fn isolate_path(dir: &Path) {
    unsafe {
        std::env::set_var("PATH", dir);
    }
}

/// Sets the `CPP_LINTER_CACHE` environment variable to the given `dir`.
pub fn set_cache_dir(dir: &Path) {
    unsafe {
        std::env::set_var("CPP_LINTER_CACHE", dir);
    }
}

/// Enables all log levels, so arguments passed to `log` macros are evaluated.
pub fn enable_logs() {
    log::set_max_level(log::LevelFilter::Trace);
}
