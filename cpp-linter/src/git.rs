//! This module was primarily used to parse diff blobs.
//!
//! Since migrating to git-bot-feedback crate, this module is now purely for regression testing.
//! Any logic that parses diffs has been moved to git-bot-feedback.
//! Some diff creation/parsing logic remains in clang_tools/mod.rs module using gix-imara-diff API instead.
#![cfg(test)]

mod test {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

    use std::{
        env::{self, current_dir, set_current_dir},
        process::Command,
    };

    use tempfile::{TempDir, tempdir};

    use crate::{cli::LinesChangedOnly, rest_client::RestClient};
    use git_bot_feedback::FileFilter;

    const TEST_REPO_URL: &str = "https://github.com/cpp-linter/cpp-linter";

    // used to setup a testing stage
    fn clone_repo(sha: Option<&str>, path: &str, patch_path: Option<&str>) {
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
            let canonical_patch_path = crate::common_fs::mk_path_abs(patch).unwrap();
            let patch_path = canonical_patch_path.to_str().unwrap();
            let ok = Command::new("git")
                .args(["apply", "--index", patch_path])
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
        eprintln!("files: {files:?}");
        assert!(files.is_empty());
        set_current_dir(cur_dir).unwrap(); // prep to delete temp_folder
        drop(tmp); // delete temp_folder
    }
}
