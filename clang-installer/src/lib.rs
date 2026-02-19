#![doc(
    html_logo_url = "https://raw.githubusercontent.com/cpp-linter/cpp-linter-rs/main/docs/docs/images/logo.png"
)]
#![doc(
    html_favicon_url = "https://github.com/cpp-linter/cpp-linter-rs/raw/main/docs/docs/images/favicon.ico"
)]
#![doc = include_str!("../README.md")]
mod downloader;
pub use downloader::{
    DownloadError,
    caching::Cacher,
    pypi::{PyPiDownloadError, PyPiDownloader},
    static_dist::{StaticDistDownloadError, StaticDistDownloader},
};

mod tool;
pub use tool::ClangTool;

pub mod utils;

mod version;
pub use version::RequestedVersion;

mod cli;
pub use cli::CliOptions;

mod progress_bar;
pub use progress_bar::ProgressBar;
