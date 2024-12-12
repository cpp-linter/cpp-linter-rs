#![doc(
    html_logo_url = "https://raw.githubusercontent.com/cpp-linter/cpp-linter-rs/main/docs/docs/images/logo.png"
)]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp-linter-rs/raw/main/docs/docs/images/favicon.ico"
)]
#![doc = include_str!("../README.md")]

// project specific modules/crates
pub mod clang_tools;
pub mod cli;
pub mod common_fs;
pub mod git;
pub mod logger;
pub mod rest_api;
pub mod run;
