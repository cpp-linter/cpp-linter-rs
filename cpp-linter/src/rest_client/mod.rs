//! A REST API client that posts cpp-linter's feedback.
//!
//! The [`RestClient`] is defined here; its feedback is built in sub-modules:
//!
//! - `feedback`: [`RestClient::post_feedback`] and the patch it shows.
//! - `comment`: the thread comment and step summary.
//! - `annotations`: the file annotations.
//! - `format_run`: how clang-format ran, to repeat it in commands.
//! - `paths`: file names relative to the repository root.

use std::path::{Path, PathBuf};

use git_bot_feedback::{FileFilter, LinesChangedOnly, RestApiClient, client::init_client};

use crate::{common_fs::FileObj, error::ClientError};
use paths::{normalize_changed_file_name, repo_root_prefix};

mod annotations;
mod comment;
mod feedback;
mod format_run;
mod paths;
#[cfg(all(test, feature = "bin"))]
mod test;

pub use annotations::{GITHUB_ANNOTATIONS_PER_JOB_LIMIT, GITHUB_ANNOTATIONS_PER_STEP_LIMIT};
pub use format_run::FormatRun;

/// The comment marker used to identify bot comments from other comments (from users or other bots).
pub const COMMENT_MARKER: &str = "<!-- cpp linter action -->\n";

/// The UserAgent header value used in HTTP requests.
pub const USER_AGENT: &str = concat!("cpp-linter/", env!("CARGO_PKG_VERSION"),);

/// The user outreach message displayed in bot comments.
pub const USER_OUTREACH: &str = concat!(
    "\n\n<sub>Powered by [**cpp-linter**](https://cpp-linter.github.io)",
    " | [Feedback]",
    "(https://github.com/cpp-linter/cpp-linter-action/issues)</sub>"
);

/// A wrapper struct around a REST API client.
///
/// Underneath, the client may support various Git servers and CI platforms.
/// Currently, the client supports GitHub and GitHub Actions.
pub struct RestClient {
    client: Box<dyn RestApiClient + Sync + Send>,
    repo_root_prefix: PathBuf,
}

impl RestClient {
    /// Initializes the REST API client and sets the UserAgent header.
    ///
    /// See [git_bot_feedback::client::init_client] for details on
    /// possible errors during initialization.
    pub fn new(repo_root: &Path) -> Result<Self, ClientError> {
        let mut client = init_client()?;
        client.set_user_agent(USER_AGENT)?;
        let repo_root_prefix = repo_root_prefix(repo_root, client.workspace_root()?);
        Ok(Self {
            client,
            repo_root_prefix,
        })
    }

    /// Returns the kind of client being used.
    ///
    /// Supported options include "local", "github" and "gitea".
    pub fn client_kind(&self) -> String {
        self.client.client_kind()
    }

    /// Is this a pull request event?
    pub fn is_pr(&self) -> bool {
        self.client.is_pr_event()
    }

    /// Gets a list of changed files.
    ///
    /// Use the [`FileFilter`] and [`LinesChangedOnly`] to filter out files
    /// that might be negligible. Returned file paths and filter patterns are relative to
    /// the root supplied to [`RestClient::new`].
    pub async fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
        lines_changed_only: &LinesChangedOnly,
        base_diff: &Option<String>,
        ignore_index: bool,
    ) -> Result<Vec<FileObj>, ClientError> {
        let mut changed_files_filter = file_filter.clone();
        changed_files_filter.ignored.clear();
        changed_files_filter.not_ignored.clear();
        changed_files_filter.not_ignored.insert(String::new());
        let files = self
            .client
            .get_list_of_changed_files(
                &changed_files_filter,
                lines_changed_only,
                base_diff.to_owned(),
                ignore_index,
            )
            .await?;
        Ok(files
            .into_iter()
            .filter_map(|(file_name, diff_lines)| {
                let file_path =
                    normalize_changed_file_name(&file_name, &self.repo_root_prefix, file_filter)?;
                let diff_chunks = diff_lines
                    .diff_hunks
                    .iter()
                    .map(|hunk| hunk.start..=hunk.end)
                    .collect();
                Some(FileObj::from(
                    file_path,
                    diff_lines.added_lines,
                    diff_chunks,
                ))
            })
            .collect())
    }

    /// Should debug logs be enabled?
    pub fn is_debug_enabled(&self) -> bool {
        self.client.is_debug_enabled()
    }

    /// Start a log group with the given `name`.
    pub fn start_log_group(&self, name: &str) {
        self.client.start_log_group(name)
    }

    /// End a log group with the given `name`.
    pub fn end_log_group(&self, name: &str) {
        self.client.end_log_group(name)
    }
}
