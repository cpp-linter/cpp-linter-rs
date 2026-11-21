//! File names in feedback, relative to the root of the git repository.

use std::path::{Path, PathBuf};

use git_bot_feedback::{FileFilter, client::nearest_git_root};

pub(super) fn repo_root_prefix(repo_root: &Path, workspace_root: Option<PathBuf>) -> PathBuf {
    let repo_root = std::path::absolute(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let workspace_root = workspace_root.unwrap_or_else(|| repo_root.to_path_buf());
    nearest_git_root(&repo_root)
        .or_else(|| nearest_git_root(&workspace_root))
        .and_then(|git_root| repo_root.strip_prefix(git_root).ok().map(Path::to_path_buf))
        .or_else(|| {
            repo_root
                .strip_prefix(&workspace_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or_default()
}

pub(super) fn prefix_feedback_path(file_path: &str, repo_root_prefix: &Path) -> String {
    repo_root_prefix
        .join(file_path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn normalize_changed_file_name(
    file_name: &str,
    repo_root_prefix: &Path,
    file_filter: &FileFilter,
) -> Option<PathBuf> {
    let file_name = PathBuf::from(file_name);
    let file_name = if repo_root_prefix.as_os_str().is_empty() {
        file_name
    } else {
        file_name.strip_prefix(repo_root_prefix).ok()?.to_path_buf()
    };
    file_filter.is_qualified(&file_name).then_some(file_name)
}
