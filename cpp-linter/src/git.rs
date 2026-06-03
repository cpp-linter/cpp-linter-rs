//! This module is primarily used to parse diff blobs.
//!
//! It can also be used (locally) to get a list of files changes from either the last
//! commit or the next commit's staging area.
//!
//! This also includes a private module that is used as a fallback (brute force)
//! mechanism when parsing diffs fail using libgit2. NOTE: parsing a diff from a buffer
//! (str or bytes) only happens in CI or when libgit2 cannot be used to initialize a
//! repository.

use std::{ops::RangeInclusive, path::PathBuf};

// non-std crates
use anyhow::Result;
use git2::{Diff, Patch};

// project specific modules/crates
use crate::{cli::LinesChangedOnly, common_fs::FileObj};
use git_bot_feedback::{FileFilter, error::DiffError};

/// Parses a patch for a single file in a diff.
///
/// Returns the list of line numbers that have additions and the ranges spanning each
/// chunk present in the `patch`.
fn parse_patch(patch: &Patch) -> (Vec<u32>, Vec<RangeInclusive<u32>>) {
    let mut additions = Vec::new();
    let mut diff_hunks = Vec::new();
    for hunk_idx in 0..patch.num_hunks() {
        let (hunk, line_count) = patch.hunk(hunk_idx).unwrap();
        diff_hunks.push(RangeInclusive::new(
            hunk.new_start(),
            hunk.new_start() + hunk.new_lines(),
        ));
        for line in 0..line_count {
            let diff_line = patch.line_in_hunk(hunk_idx, line).unwrap();
            if diff_line.origin_value() == git2::DiffLineType::Addition {
                additions.push(diff_line.new_lineno().unwrap());
            }
        }
    }
    (additions, diff_hunks)
}

/// Parses a given [`git2::Diff`] and returns a list of [`FileObj`]s.
///
/// The `lines_changed_only` parameter is used to expedite the process and only
/// focus on files that have relevant changes. The `file_filter` parameter applies
/// a filter to only include source files (or ignored files) based on the
/// extensions and ignore patterns specified.
pub fn parse_diff(
    diff: &git2::Diff,
    file_filter: &FileFilter,
    lines_changed_only: &LinesChangedOnly,
) -> Vec<FileObj> {
    let mut files: Vec<FileObj> = Vec::new();
    for file_idx in 0..diff.deltas().count() {
        let diff_delta = diff.get_delta(file_idx).unwrap();
        let file_path = diff_delta.new_file().path().unwrap().to_path_buf();
        if matches!(
            diff_delta.status(),
            git2::Delta::Added | git2::Delta::Modified | git2::Delta::Renamed,
        ) && file_filter.is_qualified(&file_path)
        {
            let (added_lines, diff_chunks) =
                parse_patch(&Patch::from_diff(diff, file_idx).unwrap().unwrap());
            if lines_changed_only.is_change_valid(!added_lines.is_empty(), !diff_chunks.is_empty())
            {
                files.push(FileObj::from(file_path, added_lines, diff_chunks));
            }
        }
    }
    files
}

/// Same as [`parse_diff`] but takes a buffer of bytes instead of a [`git2::Diff`].
///
/// In the case that libgit2 fails to parse the buffer of bytes, a private algorithm is
/// used. In such a case, brute force parsing the diff as a string can be costly. So, a
/// log warning and error are output when this occurs. Please report this instance for
/// troubleshooting/diagnosis as this likely means the diff is malformed or there is a
/// bug in libgit2 source.
pub fn parse_diff_from_buf(
    buff: &[u8],
    file_filter: &FileFilter,
    lines_changed_only: &LinesChangedOnly,
) -> Result<Vec<FileObj>, DiffError> {
    if let Ok(diff_obj) = &Diff::from_buffer(buff) {
        Ok(parse_diff(diff_obj, file_filter, lines_changed_only))
    } else {
        log::warn!("libgit2 failed to parse the diff");
        Ok(git_bot_feedback::parse_diff(
            &String::from_utf8_lossy(buff),
            file_filter,
            &lines_changed_only.clone().into(),
        )?
        .iter()
        .map(|(name, diff_lines)| {
            let diff_chunks = diff_lines
                .diff_hunks
                .iter()
                .map(|hunk| hunk.start..=hunk.end)
                .collect();
            FileObj::from(
                PathBuf::from(&name),
                diff_lines.added_lines.clone(),
                diff_chunks,
            )
        })
        .collect())
    }
}

#[cfg(test)]
mod test {
    use std::{
        env::{self, current_dir, set_current_dir},
        fs,
        process::Command,
    };

    use tempfile::{TempDir, tempdir};

    use crate::{cli::LinesChangedOnly, rest_client::RestClient};
    use git_bot_feedback::FileFilter;

    const TEST_REPO_URL: &str = "https://github.com/cpp-linter/cpp-linter";

    // used to setup a testing stage
    fn clone_repo(sha: Option<&str>, path: &str, patch_path: Option<&str>) {
        // let repo = Repository::clone(TEST_REPO_URL, path).unwrap();
        let ok = Command::new("git")
            .args(["clone", TEST_REPO_URL, path])
            .status()
            .expect("Failed to clone repo");
        if !ok.success() {
            panic!("Failed to clone repo");
        }
        if let Some(sha) = sha {
            let ok = Command::new("git")
                .args(["-c", "advice.detachedHead=false", "checkout", sha])
                .current_dir(path)
                .status()
                .expect("Failed to checkout commit");
            if !ok.success() {
                panic!("Failed to checkout commit");
            }
        }
        if let Some(patch) = patch_path {
            let canonical_path_path = fs::canonicalize(patch).unwrap();
            let ok = Command::new("git")
                .args(["apply", "--index", canonical_path_path.to_str().unwrap()])
                .current_dir(path)
                .status()
                .expect("Failed to apply patch and stage its changes");
            if !ok.success() {
                panic!("Failed to apply patch and stage its changes");
            }
            let ok = Command::new("git")
                .args(["status", "-s"])
                .current_dir(path)
                .status()
                .expect("Failed to get git status after applying patch");
            if !ok.success() {
                panic!("Failed to get git status after applying patch");
            }
        }
    }

    fn get_temp_dir() -> TempDir {
        let tmp = tempdir().unwrap();
        println!("Using temp folder at {:?}", tmp.path());
        tmp
    }

    async fn checkout_cpp_linter_py_repo(
        sha: &str,
        extensions: &[&str],
        tmp: &TempDir,
        patch_path: Option<&str>,
        ignore_staged: bool,
    ) -> Vec<crate::common_fs::FileObj> {
        clone_repo(
            Some(sha),
            tmp.path().as_os_str().to_str().unwrap(),
            patch_path,
        );
        // avoid use of REST API when testing in CI
        unsafe {
            env::set_var("GITHUB_ACTIONS", "false");
            env::set_var("CI", "false");
        }
        let rest_api_client = RestClient::new().unwrap();
        let file_filter = FileFilter::new(&["target"], extensions, None);
        set_current_dir(tmp).unwrap();
        let base_diff = if ignore_staged {
            Some("HEAD".to_string())
        } else {
            None
        };
        rest_api_client
            .get_list_of_changed_files(
                &file_filter,
                &LinesChangedOnly::Off.into(),
                &base_diff,
                ignore_staged,
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn with_no_changed_sources() {
        // commit with no modified C/C++ sources
        let sha = "0c236809891000b16952576dc34de082d7a40bf3";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = ["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(sha, &extensions, &tmp, None, false).await;
        println!("files = {:?}", files);
        assert!(files.is_empty());
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }

    #[tokio::test]
    async fn with_changed_sources() {
        // commit with modified C/C++ sources
        let sha = "950ff0b690e1903797c303c5fc8d9f3b52f1d3c5";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = ["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(sha, &extensions, &tmp, None, false).await;
        println!("files = {:?}", files);
        assert!(files.len() >= 2);
        for file in files {
            assert!(
                extensions.contains(
                    &file
                        .name
                        .extension()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                        .as_str()
                )
            );
        }
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }

    #[tokio::test]
    async fn with_staged_changed_sources() {
        // commit with no modified C/C++ sources
        let sha = "0c236809891000b16952576dc34de082d7a40bf3";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = ["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(
            sha,
            &extensions,
            &tmp,
            Some("tests/git_status_test_assets/cpp-linter/cpp-linter/test_git_lib.patch"),
            false,
        )
        .await;
        println!("files = {:?}", files);
        assert!(!files.is_empty());
        for file in files {
            assert!(
                extensions.contains(
                    &file
                        .name
                        .extension()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                        .as_str()
                )
            );
        }
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }

    #[tokio::test]
    async fn with_ignored_staged_changes() {
        // commit with no modified C/C++ sources
        let sha = "0c236809891000b16952576dc34de082d7a40bf3";
        let cur_dir = current_dir().unwrap();
        let tmp = get_temp_dir();
        let extensions = ["cpp", "hpp"];
        let files = checkout_cpp_linter_py_repo(
            sha,
            &extensions,
            &tmp,
            Some("tests/git_status_test_assets/cpp-linter/cpp-linter/test_git_lib.patch"),
            true,
        )
        .await;
        Command::new("git")
            .args([
                "--no-pager",
                "show",
                "a2875ac00e6f1dc3eb4ac19712c7a241b5a76e83",
                "--format=%b",
            ])
            .status()
            .unwrap();
        eprintln!("files: {files:?}");
        assert!(files.is_empty());
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }
}
