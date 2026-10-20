#![allow(clippy::unwrap_used)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use clang_tools_manager::RequestedVersion;
use git_bot_feedback::FileFilter;

use super::{OtherFileFix, Pending, Replacement, TidyFixes, apply_other_file_fixes, merge_note};
use crate::{
    clang_tools::{
        ClangTool, analyze_files,
        clang_tidy::{SkippedFix, TidyAdvice, TidyNotification, tally_tidy_advice},
    },
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
        note: Some(source_note()),
    }
}

/// The note about the diagnostic (in [`SOURCE`]) that suggests [`header_fix()`].
fn source_note() -> TidyNotification {
    TidyNotification {
        filename: "fix.cpp".to_string(),
        line: 2,
        cols: 10,
        severity: "warning".to_string(),
        rationale: "pointer parameter 'p' can be pointer to const".to_string(),
        diagnostic: "readability-non-const-parameter".to_string(),
        suggestion: vec![],
        fixed_lines: vec![],
        skipped_fixes: vec![],
    }
}

/// The notes (about other files) in the header's tidy advice.
fn header_notes(file: &Arc<Mutex<FileObj>>) -> Vec<TidyNotification> {
    let file = file.lock().unwrap();
    file.tidy_advice
        .as_ref()
        .map(|advice| advice.notes.clone())
        .unwrap_or_default()
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
        gen_patch: true,
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

    // the header's advice explains the fix, without counting it as the header's diagnostic
    let notes = header_notes(&files[1]);
    let note = notes
        .iter()
        .find(|n| n.diagnostic == "readability-non-const-parameter")
        .unwrap();
    assert_eq!(note.filename, "fix.cpp");
    assert_eq!(note.fixed_lines, vec![2]);
    let help = files[1]
        .lock()
        .unwrap()
        .tidy_advice
        .as_ref()
        .unwrap()
        .get_suggestion_help(2, 2);
    assert!(help.starts_with("### clang-tidy diagnostic(s)"), "{help}");
    assert!(help.contains(&note.rationale), "{help}");
    assert_eq!(tally_tidy_advice(&files).unwrap(), 1);
}

#[test]
fn not_analyzed() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
        ..Default::default()
    };
    let mut source = FileObj::new(PathBuf::from("fix.cpp"));
    source.tidy_advice = Some(TidyAdvice {
        notes: vec![source_note()],
    });
    let files = as_files(vec![source]);
    let mut without_note = header_fix(tmp.path());
    without_note.note = None;
    let fixes = TidyFixes {
        own: vec![],
        // the same fix twice, and a fix whose diagnostic has no note
        others: vec![header_fix(tmp.path()), header_fix(tmp.path()), without_note],
    };
    let mut logs = vec![vec![]];
    apply_other_file_fixes(&files, &[(0, fixes)], &clang_params, &mut logs).unwrap();
    assert!(patched(&files[0]).is_none());
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].starts_with("Not applying the fix for readability-non-const-parameter"));
    assert!(messages[0].ends_with("to fix.h because it is not one of the analyzed files"));
    // the skipped fix is recorded once on the diagnostic's note
    let file = files[0].lock().unwrap();
    let note = &file.tidy_advice.as_ref().unwrap().notes[0];
    assert_eq!(
        note.skipped_fixes,
        vec![SkippedFix {
            file: "fix.h".to_string(),
            reason: "it is not one of the analyzed files".to_string(),
        }]
    );
    assert_eq!(
        note.skipped_fixes[0].describe(),
        "Part of this fix was not applied to `fix.h` because it is not one of the analyzed files."
    );
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
        gen_patch: true,
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
    assert!(
        messages[0].ends_with(
            "to fix.h because the fix clashes with the `--lines-changed-only` constraint"
        )
    );
}

#[test]
fn out_of_bounds() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
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
    assert!(messages[0].ends_with(&format!(
        "to fix.h because the fix seems malformed and is out of the file's bounds \
        (bytes {} to {} of a {} byte file)",
        HEADER.len(),
        HEADER.len() + 1,
        HEADER.len(),
    )));
}

/// The same fix for the header is suggested by two analyzed sources,
/// and the header has a fix of its own.
#[test]
fn merge_with_own_fixes() {
    let tmp = workspace();
    fs::write(tmp.path().join("other.cpp"), SOURCE).unwrap();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
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
    // the same note is recorded once, with the fixed line in the header
    let notes = header_notes(&files[1]);
    assert_eq!(notes.len(), 1, "{notes:#?}");
    assert_eq!(notes[0].fixed_lines, vec![2]);
}

/// A fix that removes a whole changed line (including its line break)
/// does not reach into the next line.
#[test]
#[allow(clippy::single_range_in_vec_init)]
fn remove_changed_line() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        lines_changed_only: LinesChangedOnly::On,
        gen_patch: true,
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::from(PathBuf::from("fix.cpp"), vec![2], vec![2..=2]),
        // only the first line of the header was changed
        FileObj::from(PathBuf::from("fix.h"), vec![1], vec![1..=1]),
    ]);
    let mut fix = header_fix(tmp.path());
    fix.replacement = Replacement {
        offset: 0,
        length: "#pragma once\n".len(),
        text: String::new(),
    };
    let fixes = TidyFixes {
        own: vec![],
        others: vec![fix],
    };
    let mut logs = vec![vec![], vec![]];
    apply_other_file_fixes(&files, &[(0, fixes)], &clang_params, &mut logs).unwrap();
    assert_eq!(patched(&files[1]).unwrap(), "void set(int *p);\n");
    assert_eq!(header_notes(&files[1])[0].fixed_lines, vec![1]);
}

/// A note about the analyzed file itself is only merged into the file's own note,
/// so it is never counted twice.
#[test]
fn merge_note_about_same_file() {
    let mut note = source_note();
    note.fixed_lines = vec![3];
    let mut notes = vec![];
    merge_note(&mut notes, note.clone(), "fix.cpp");
    assert!(notes.is_empty());

    let own = TidyNotification {
        fixed_lines: vec![2],
        ..source_note()
    };
    let mut notes = vec![own];
    merge_note(&mut notes, note.clone(), "fix.cpp");
    merge_note(&mut notes, note, "fix.cpp");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].fixed_lines, vec![2, 3]);
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

/// An analyzed file that was removed after it was analyzed cannot be fixed.
#[test]
fn analyzed_file_missing() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
    ]);
    // keep the header's path known to `AnalyzedFiles`, but make it unreadable
    let fix = header_fix(tmp.path());
    fs::remove_file(tmp.path().join("fix.h")).unwrap();
    fs::create_dir(tmp.path().join("fix.h")).unwrap();
    let fixes = TidyFixes {
        own: vec![],
        others: vec![fix],
    };
    let mut logs = vec![vec![], vec![]];
    let err = apply_other_file_fixes(&files, &[(0, fixes)], &clang_params, &mut logs).unwrap_err();
    assert!(
        matches!(err, crate::error::ClangCaptureError::ReadFileFailed { ref file_name, .. } if file_name == "fix.h"),
        "{err:?}"
    );
}

/// A fix whose diagnostic has no matching note is applied without a note.
#[test]
fn pending_fix_without_note() {
    let tmp = workspace();
    let mut fix = header_fix(tmp.path());
    fix.note = None;
    let mut pending = Pending::new(&[]);
    assert!(pending.add(&fix, 2).is_ok());
    assert_eq!(pending.replacements, vec![fix.replacement]);
    assert!(pending.notes.is_empty());

    // fixes for the same diagnostic on different lines share a note
    let fix = header_fix(tmp.path());
    assert!(pending.add(&fix, 2).is_ok());
    assert!(pending.add(&fix, 3).is_ok());
    assert_eq!(pending.notes.len(), 1);
    assert_eq!(pending.notes[0].fixed_lines, vec![2, 3]);
}

/// A fix that replaces `int` in the header's declaration.
fn replace_int(workspace: &Path, text: &str) -> OtherFileFix {
    OtherFileFix {
        replacement: Replacement {
            offset: HEADER_FIX_OFFSET,
            length: 3,
            text: text.to_string(),
        },
        ..header_fix(workspace)
    }
}

/// Of two different fixes for the same text in another file, only the first is applied.
#[test]
fn overlapping_fixes() {
    let tmp = workspace();
    fs::write(tmp.path().join("other.cpp"), SOURCE).unwrap();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
        FileObj::new(PathBuf::from("other.cpp")),
    ]);
    let fixes_of = |text: &str| TidyFixes {
        own: vec![],
        others: vec![replace_int(tmp.path(), text)],
    };
    let tidy_fixes = [(0, fixes_of("long")), (2, fixes_of("char"))];
    let mut logs = vec![vec![], vec![], vec![]];
    apply_other_file_fixes(&files, &tidy_fixes, &clang_params, &mut logs).unwrap();
    assert_eq!(
        patched(&files[1]).unwrap(),
        "#pragma once\nvoid set(long *p);\n"
    );
    assert!(messages(&logs[0])[0].starts_with("Applying the fix"));
    let messages = messages(&logs[2]);
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].ends_with("to fix.h because the fix overlaps another fix for the same file")
    );
    // the overlapping fix does not touch the patch, so it gets no fixed line
    let notes = header_notes(&files[1]);
    assert_eq!(notes.len(), 1, "{notes:#?}");
    assert_eq!(notes[0].fixed_lines, vec![2]);
    // the patch is not changed behind the scenes either
    assert!(logs[1].is_empty(), "{:#?}", logs[1]);
}

/// A fix from another file that overlaps the file's own fix is not applied.
#[test]
fn overlaps_own_fix() {
    let tmp = workspace();
    let clang_params = ClangParams {
        repo_root: tmp.path().to_path_buf(),
        gen_patch: true,
        ..Default::default()
    };
    let files = as_files(vec![
        FileObj::new(PathBuf::from("fix.cpp")),
        FileObj::new(PathBuf::from("fix.h")),
    ]);
    let own = TidyFixes {
        own: vec![replace_int(tmp.path(), "long").replacement],
        others: vec![],
    };
    let mut fix = header_fix(tmp.path());
    // insert inside the replaced `int`
    fix.replacement.offset += 1;
    let others = TidyFixes {
        own: vec![],
        others: vec![fix],
    };
    let mut logs = vec![vec![], vec![]];
    apply_other_file_fixes(&files, &[(0, others), (1, own)], &clang_params, &mut logs).unwrap();
    // only the header's own fix is applied
    assert_eq!(
        patched(&files[1]).unwrap(),
        "#pragma once\nvoid set(long *p);\n"
    );
    let messages = messages(&logs[0]);
    assert_eq!(messages.len(), 1);
    assert!(
        messages[0].ends_with("to fix.h because the fix overlaps another fix for the same file")
    );
    assert!(header_notes(&files[1]).is_empty());
}
