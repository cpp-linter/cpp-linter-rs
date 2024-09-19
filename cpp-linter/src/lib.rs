#![doc(
    html_logo_url = "https://github.com/cpp-linter/cpp_linter_rs/raw/main/docs/theme/favicon.png"
)]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp_linter_rs/raw/main/docs/theme/favicon.png"
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
