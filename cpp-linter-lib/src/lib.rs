#![doc(html_logo_url = "https://github.com/cpp-linter/cpp-linter/raw/main/docs/_static/logo.png")]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp-linter/raw/main/docs/_static/favicon.ico"
)]
#![doc = include_str!("../README.md")]

// project specific modules/crates
pub mod clang_tools;
pub mod cli;
pub mod common_fs;
pub mod git;
pub mod rest_api;
pub use rest_api::github_api;
pub mod logger;
pub mod run;
