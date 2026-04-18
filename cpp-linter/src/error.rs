use git_bot_feedback::RestClientError;

#[derive(Debug, thiserror::Error)]
pub enum SuggestionError {
    #[error("Failed to convert patch for '{file_name}' into bytes: {source}")]
    PatchIntoBytesFailed {
        file_name: String,
        #[source]
        source: git2::Error,
    },
    #[error("Failed to convert patch for file '{file_name}' into string: {source}")]
    PatchIntoStringFailed {
        file_name: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("Failed to get hunk {hunk_id} from patch for {file_name}: {source}")]
    GetHunkFailed {
        hunk_id: usize,
        file_name: String,
        #[source]
        source: git2::Error,
    },
    #[error(
        "Failed to get line {line_index} in a hunk {hunk_id} of patch for {file_name}: {source}"
    )]
    GetHunkLineFailed {
        line_index: usize,
        hunk_id: usize,
        file_name: String,
        #[source]
        source: git2::Error,
    },
    #[error(
        "Failed to convert line {line_index} buffer to string in hunk {hunk_id} of patch for {file_name}: {source}"
    )]
    HunkLineIntoStringFailed {
        line_index: usize,
        hunk_id: usize,
        file_name: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FileObjError {
    #[error("Failed to read file contents")]
    ReadFile(std::io::Error),
    #[error("Failed to create patch for file {0:?}: {1}")]
    MakePatchFailed(String, #[source] git2::Error),
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
