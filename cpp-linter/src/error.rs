//! This module defines error types for the cpp-linter crate.

use clang_tools_manager::GetToolError;
use git_bot_feedback::RestClientError;

/// Errors related to [`FileObj`](crate::common_fs::FileObj) methods' results.
#[derive(Debug, thiserror::Error)]
pub enum FileObjError {
    /// Error when failing to read a file's contents.
    #[error("Failed to read file contents")]
    ReadFile(std::io::Error),

    /// Error when failing to convert a file's contents to a UTF-8 string.
    #[error("Failed to convert patch buffer to UTF-8 string for file {0}: {1}")]
    FromUtf8Error(String, #[source] std::string::FromUtf8Error),

    /// Error when failing to generate a patch for a file.
    #[error("Failed to print a hunk to a string buffer: {0}")]
    DisplayStringFailed(#[source] std::fmt::Error),
}

/// Errors related to the REST client used for posting feedback and special logging.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Error to propagate client failures.
    #[error(transparent)]
    RestClientError(#[from] RestClientError),

    /// Error when the client cannot detect a supported Git server or CI platform.
    #[error("Unsupported Git server or CI platform")]
    GitServerUnsupported,

    /// Error when the client encounters a poisoned mutex during file processing.
    #[error("Mutex lock poisoned for a source file: {0}")]
    MutexPoisoned(String),

    /// Error to propagate a [`FileObjError`] encountered during file processing.
    #[error(transparent)]
    FileObjError(#[from] FileObjError),
}

/// Errors related to invoking clang tools and processing their output.
#[derive(Debug, thiserror::Error)]
pub enum ClangCaptureError {
    /// Error when failing to acquire a lock on a file's mutex.
    #[error("Failed to acquire a lock on a file's mutex")]
    MutexPoisoned,

    /// Error when invoking a clang tool with no known path to the binary executable.
    #[error("Unknown path to {0} tool; required to invoke it.")]
    ToolPathUnknown(&'static str),

    /// Error when a clang tool fails to be invoked.
    #[error("Failed to {task}: {source}")]
    FailedToRunCommand {
        /// The purpose of running the clang tool.
        ///
        /// May include context about the arguments passed to the clang-tool.
        task: String,

        /// The underlying error from trying to run the clang tool.
        #[source]
        source: std::io::Error,
    },

    /// Error when the output of a clang tool cannot be parsed as a UTF-8 string.
    #[error("{task} output was not valid UTF-8: {source}")]
    NonUtf8Output {
        /// The clang tool that produced the output.
        task: String,

        /// The underlying error from trying to convert the clang tool's output to a UTF-8 string.
        #[source]
        source: std::string::FromUtf8Error,
    },

    /// Error when failing to read a file's contents.
    #[error("Failed to read contents of file '{file_name}': {source}")]
    ReadFileFailed {
        /// The name of the file that failed to be read.
        file_name: String,

        /// The underlying error from trying to read the file's contents.
        #[source]
        source: std::io::Error,
    },

    /// Error when failing to write a file.
    #[error("Failed to write file '{file_name}': {source}")]
    WriteFileFailed {
        /// The name of the file that failed to be written.
        file_name: String,

        /// The underlying error from trying to write the file.
        #[source]
        source: std::io::Error,
    },

    /// Error when failing to compile a regular expression pattern.
    #[error("Failed to compile regular expression: {0}")]
    RegexError(#[from] regex::Error),

    /// Error when failing to determine the current working directory.
    #[error("Failed to determine the current working directory: {0}")]
    UnknownWorkingDirectory(#[source] std::io::Error),

    /// Error when failing to parse an integer from a string.
    #[error("Failed to parse integer from string: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),

    /// Error when failing to determine the parent directory for caching purposes.
    #[error("Failed to determine the parent directory for caching purposes")]
    UnknownCacheParentPath,

    /// Error when failing to create a directory for caching purposes.
    #[error("Failed to create directory for caching patches: {0}")]
    MkDirFailed(#[source] std::io::Error),
}

/// Errors related to orchestrating clang tools in parallel.
#[derive(Debug, thiserror::Error)]
pub enum ClangTaskError {
    /// Error to propagate failures from downloading/installing/finding a clang tool.
    #[error(transparent)]
    GetToolError(#[from] GetToolError),

    /// Error when the tool manager cannot find a suitable version of a clang tool.
    #[error("Failed to find tool {0} or install a suitable version")]
    FindToolError(&'static str),

    /// Error when failing to parse the compilation database.
    ///
    /// This can occur regardless of invoking clang-tidy.
    #[error("Failed to parse compilation database: {0}")]
    ParseJsonError(#[from] serde_json::Error),

    /// Error to propagate task joining failures (from the tokio runtime).
    #[error("Failed to execute task in parallel: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    /// Error to propagate failures from capturing clang tools' output.
    #[error(transparent)]
    ClangCaptureError(#[from] ClangCaptureError),
}
