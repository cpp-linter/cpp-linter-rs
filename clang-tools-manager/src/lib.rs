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
    resource_headers::ResourceHeadersError,
    static_dist::{StaticDistDownloadError, StaticDistDownloader},
};

mod tool;
pub use tool::ClangTool;

pub mod utils;

mod version;
pub use version::{ClangVersion, GetToolError, RequestedVersion, RequestedVersionParsingError};

mod progress_bar;
pub use progress_bar::ProgressBar;

/// Should log messages and progress bars be written to stderr instead of stdout?
///
/// This is set with `logger::set_log_to_stderr()` (requires the `bin` feature).
pub(crate) static LOG_TO_STDERR: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub mod logger;

#[cfg(all(test, unix))]
mod test_utils_unix;
