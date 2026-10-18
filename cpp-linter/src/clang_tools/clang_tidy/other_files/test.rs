#![allow(clippy::unwrap_used)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use clang_tools_manager::RequestedVersion;
use git_bot_feedback::FileFilter;

use super::{OtherFileFix, Replacement, TidyFixes, apply_other_file_fixes};
use crate::{
    clang_tools::{ClangTool, analyze_files},
    cli::{ClangParams, LinesChangedOnly},
    common_fs::FileObj,
};

const HEADER: &str = "#pragma once\nvoid set(int *p);\n";
const SOURCE: &str = "#include \"fix.h\"\nvoid set(int *p) {\n  int x = *p;\n  (void)x;\n}\n";

/// The offset of `int *p` in [`HEADER`].
const HEADER_FIX_OFFSET: usize = 22;

fn get_tidy_exe() -> PathBuf {
    ClangTool::ClangTidy
        .get_exe_path(
            &RequestedVersion::from_str(
                env::var("CLANG_VERSION").unwrap_or("".to_string()).as_str(),
            )
            .unwrap(),
        )
        .unwrap()
}

/// Write [`HEADER`] and [`SOURCE`] into a new temporary workspace.
fn workspace() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::with_prefix("cpp-linter-unit-tests_").unwrap();
    fs::write(tmp.path().join("fix.h"), HEADER).unwrap();
    fs::write(tmp.path().join("fix.cpp"), SOURCE).unwrap();
    tmp
}

fn as_files(files: Vec<FileObj>) -> Vec<Arc<Mutex<FileObj>>> {
    files.into_iter().map(|f| Arc::new(Mutex::new(f))).collect()
}

/// A fix that adds `const` to the declaration in the header.
fn header_fix(workspace: &Path) -> OtherFileFix {
    OtherFileFix {
        path: workspace.join("fix.h"),
        diagnostic: "readability-non-const-parameter".to_string(),
        replacement: Replacement {
            offset: HEADER_FIX_OFFSET,
            length: 0,
            text: "const ".to_string(),
        },
    }
}

fn header_fixes(workspace: &Path) -> TidyFixes {
    TidyFixes {
        own: vec![],
        others: vec![header_fix(workspace)],
    }
}

fn patched(file: &Arc<Mutex<FileObj>>) -> Option<String> {
    let file = file.lock().unwrap();
    file.patched_path
        .as_ref()
        .map(|p| fs::read_to_string(p).unwrap())
}

fn messages(logs: &[(log::Level, String)]) -> Vec<&str> {
    logs.iter().map(|(_, msg)| msg.as_str()).collect()
}

/// clang-tidy suggests a fix for the header while analyzing the source.
/// The header is also analyzed, so the fix is applied to the header's cached copy.
#[tokio::test]
async fn fix_analyzed_header() {
    let tmp = workspace();
    let clang_params = Arc::new(ClangParams {
        tidy_checks: "-*,readability-non-const-parameter".to_string(),
        clang_tidy_command: Some(get_tidy_exe()),
        repo_root: tmp.path().to_path_buf(),
        ..Default::default()
    });
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
    ]);
    let logs = analyze_files(&files, clang_params).await.unwrap();

    // worktree is untouched
    assert_eq!(
        fs::read_to_string(tmp.path().join("fix.h")).unwrap(),
        HEADER
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join("fix.cpp")).unwrap(),
        SOURCE
    );

    // both cached copies are fixed
    let fixed = |content: &str| content.replace("set(int *p)", "set(const int *p)");
    assert_eq!(patched(&files[0]).unwrap(), fixed(SOURCE));
    assert_eq!(patched(&files[1]).unwrap(), fixed(HEADER));

    // the fix for the header is logged in the source's group
    assert!(
        messages(&logs[0]).iter().any(|msg| {
            *msg == "Applying the fix for readability-non-const-parameter (reported in fix.cpp) to fix.h"
        }),
        "{logs:#?}"
    );
}

#[test]
fn not_analyzed() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        ..Default::default()
    };
    let files = as_files(vec![FileObj::new(PathBuf::from("fix.cpp"))]);
    let mut logs = vec![vec![]];
    apply_other_file_fixes(
        &files,
        &[(0, header_fixes(tmp.path()))],
        &clang_params,
        &mut logs,
    )
    .unwrap();
    assert!(patched(&files[0]).is_none());
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].starts_with("Not applying the fix for readability-non-const-parameter"));
    assert!(messages[0].ends_with("to fix.h because it is not one of the analyzed files"));
}

#[test]
fn ignored_by_tidy() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        tidy_filter: Some(FileFilter::new(&["fix.h"], &["cpp", "h"], None)),
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
    ]);
    let mut logs = vec![vec![], vec![]];
    apply_other_file_fixes(
        &files,
        &[(0, header_fixes(tmp.path()))],
        &clang_params,
        &mut logs,
    )
    .unwrap();
    assert!(patched(&files[1]).is_none());
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].ends_with("to fix.h because it is ignored by `--ignore-tidy`"));
    assert!(logs[1].is_empty());
}

#[test]
#[allow(clippy::single_range_in_vec_init)]
fn outside_changed_lines() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        lines_changed_only: LinesChangedOnly::On,
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::from(PathBuf::from("fix.cpp"), vec![2], vec![2..=2]),
        // only the first line of the header was changed
        FileObj::from(PathBuf::from("fix.h"), vec![1], vec![1..=1]),
    ]);
    let mut logs = vec![vec![], vec![]];
    apply_other_file_fixes(
        &files,
        &[(0, header_fixes(tmp.path()))],
        &clang_params,
        &mut logs,
    )
    .unwrap();
    assert!(patched(&files[1]).is_none());
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].ends_with("to fix.h because the fix is outside the file's changed lines"));
}

#[test]
fn out_of_bounds() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
    ]);
    let mut fix = header_fix(tmp.path());
    fix.replacement.offset = HEADER.len();
    fix.replacement.length = 1;
    let fixes = TidyFixes {
        own: vec![],
        others: vec![fix],
    };
    let mut logs = vec![vec![], vec![]];
    apply_other_file_fixes(&files, &[(0, fixes)], &clang_params, &mut logs).unwrap();
    assert!(patched(&files[1]).is_none());
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].ends_with("to fix.h because the fix is out of the file's bounds"));
}

/// The same fix for the header is suggested by two analyzed sources,
/// and the header has a fix of its own.
#[test]
fn merge_with_own_fixes() {
    let tmp = workspace();
    fs::write(tmp.path().join("other.cpp"), SOURCE).unwrap();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
        FileObj::new(PathBuf::from("other.cpp")),
    ]);
    let own = TidyFixes {
        own: vec![Replacement {
            offset: 0,
            length: 0,
            text: "// fixed\n".to_string(),
        }],
        others: vec![],
    };
    let tidy_fixes = [
        (0, header_fixes(tmp.path())),
        (1, own),
        (2, header_fixes(tmp.path())),
    ];
    let mut logs = vec![vec![], vec![], vec![]];
    apply_other_file_fixes(&files, &tidy_fixes, &clang_params, &mut logs).unwrap();
    assert_eq!(
        patched(&files[1]).unwrap(),
        "// fixed\n#pragma once\nvoid set(const int *p);\n"
    );
    for (source, name) in [(0, "fix.cpp"), (2, "other.cpp")] {
        let messages = messages(&logs[source]);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains(&format!("(reported in {name}) to ")));
        assert!(messages[0].starts_with("Applying the fix"));
    }
    // duplicate fixes do not cause warnings
    assert!(logs[1].is_empty(), "{:#?}", logs[1]);
}

#[test]
fn no_other_fixes() {
    let clang_params = ClangParams {
        // an unknown directory is never read
        repo_root: PathBuf::from("does-not-exist"),
        ..Default::default()
    };
    let files = as_files(vec![FileObj::new(PathBuf::from("fix.cpp"))]);
    let mut logs = vec![vec![]];
    apply_other_file_fixes(
        &files,
        &[(0, TidyFixes::default())],
        &clang_params,
        &mut logs,
    )
    .unwrap();
    assert!(logs[0].is_empty());
    assert!(patched(&files[0]).is_none());
}
