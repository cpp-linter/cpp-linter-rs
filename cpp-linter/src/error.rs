use clang_tools_manager::GetToolError;
use git_bot_feedback::RestClientError;

#[derive(Debug, thiserror::Error)]
pub enum SuggestionError {
    #[error("Failed to write hunk of patch into string buffer for {file_name}: {source}")]
    HunkIntoStringFailed {
        file_name: String,
        #[source]
        source: core::fmt::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FileObjError {
    #[error("Failed to read file contents")]
    ReadFile(std::io::Error),
    #[error("Failed to convert patch buffer to UTF-8 string for file {0}: {1}")]
    FromUtf8Error(String, #[source] std::string::FromUtf8Error),
    #[error(transparent)]
    SuggestionError(#[from] SuggestionError),
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    RestClientError(#[from] RestClientError),
    #[error("Unsupported Git server or CI platform")]
    GitServerUnsupported,
    #[error("Mutex lock poisoned for a source file: {0}")]
    MutexPoisoned(String),
    #[error(transparent)]
    FileObjError(#[from] FileObjError),
}

#[derive(Debug, thiserror::Error)]
pub enum ClangCaptureError {
    #[error("Failed to acquire a lock on a file's mutex")]
    MutexPoisoned,
    #[error("Unknown path to {0} tool; required to invoke it.")]
    ToolPathUnknown(&'static str),
    #[error("Failed to {task}: {source}")]
    FailedToRunCommand {
        task: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{task} output was not valid UTF-8: {source}")]
    NonUtf8Output {
        task: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("Failed to parse XML output from clang-format for {file_name}: {source}")]
    XmlParsingFailed {
        file_name: String,
        #[source]
        source: quick_xml::DeError,
    },
    #[error("Failed to read contents of file '{file_name}': {source}")]
    ReadFileFailed {
        file_name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to write file '{file_name}': {source}")]
    WriteFileFailed {
        file_name: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to compile regular expression: {0}")]
    RegexError(#[from] regex::Error),
    #[error("Failed to determine the current working directory: {0}")]
    UnknownWorkingDirectory(#[source] std::io::Error),
    #[error("Failed to parse integer from string: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
}

#[derive(Debug, thiserror::Error)]
pub enum ClangTaskError {
    #[error(transparent)]
    GetToolError(#[from] GetToolError),
    #[error("Failed to find tool {0} or install a suitable version")]
    FindToolError(&'static str),
    #[error("Failed to parse compilation database: {0}")]
    ParseJsonError(#[from] serde_json::Error),
    #[error("Failed to execute task in parallel: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error(transparent)]
    ClangCaptureError(#[from] ClangCaptureError),
}
