#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use clang_tools_manager::RequestedVersion;
use regex::Regex;

use crate::{
    clang_tools::{ClangTool, clang_tidy::parse_tidy_output},
    cli::{ClangParams, LinesChangedOnly},
    common_fs::FileObj,
};

use super::{
    CLANG_DIAGNOSTICS_URL, ClangCaptureError, CompilationUnit, ExportedFixes, FixPaths,
    NOTE_HEADER, OtherFileFix, Replacement, TidyNotification, apply_fixes, run_clang_tidy,
};

#[test]
fn clang_diagnostic_link() {
    let note = TidyNotification {
        filename: String::from("some_src.cpp"),
        line: 1504,
        cols: 9,
        rationale: String::from("file not found"),
        severity: String::from("error"),
        diagnostic: String::from("clang-diagnostic-error"),
        suggestion: vec![],
        fixed_lines: vec![],
        skipped_fixes: vec![],
    };
    assert_eq!(note.diagnostic_link(), note.diagnostic);
}

#[test]
fn clang_diagnostic_warning_link() {
    let note = TidyNotification {
        filename: String::from("some_src.cpp"),
        line: 1504,
        cols: 9,
        rationale: String::from("unused variable 'x'"),
        severity: String::from("warning"),
        diagnostic: String::from("clang-diagnostic-unused-variable"),
        suggestion: vec![],
        fixed_lines: vec![],
        skipped_fixes: vec![],
    };
    let expected = format!(
        "[{}]({CLANG_DIAGNOSTICS_URL}#w{})",
        note.diagnostic, "unused-variable",
    );
    assert_eq!(note.diagnostic_link(), expected);
}

#[test]
fn clang_analyzer_link() {
    let note = TidyNotification {
        filename: String::from("some_src.cpp"),
        line: 1504,
        cols: 9,
        rationale: String::from("Dereference of null pointer (loaded from variable 'pipe_num')"),
        severity: String::from("warning"),
        diagnostic: String::from("clang-analyzer-core.NullDereference"),
        suggestion: vec![],
        fixed_lines: vec![],
        skipped_fixes: vec![],
    };
    let expected = format!(
        "[{}](https://clang.llvm.org/extra/clang-tidy/checks/{}/{}.html)",
        note.diagnostic, "clang-analyzer", "core.NullDereference",
    );
    assert_eq!(note.diagnostic_link(), expected);
}

#[test]
fn invalid_diagnostic_link() {
    let expected = "no_diagnostic_name".to_string();
    let note = TidyNotification {
        filename: String::from("some_src.cpp"),
        line: 1504,
        cols: 9,
        rationale: String::from("some rationale"),
        severity: String::from("warning"),
        diagnostic: expected.clone(),
        suggestion: vec![],
        fixed_lines: vec![],
        skipped_fixes: vec![],
    };
    assert_eq!(note.diagnostic_link(), expected);
}

// ***************** test for regex parsing of clang-tidy stdout

#[test]
fn regex_capture() {
    let src = "tests/demo/demo.hpp:11:11: \
    warning: use a trailing return type for this function \
    [modernize-use-trailing-return-type,-warnings-as-errors]";
    let pat = Regex::new(NOTE_HEADER).unwrap();
    let cap = pat.captures(src).unwrap();
    assert_eq!(
        cap.get(0).unwrap().as_str(),
        format!(
            "{}:{}:{}: {}:{}[{},-warnings-as-errors]",
            cap.get(1).unwrap().as_str(),
            cap.get(2).unwrap().as_str(),
            cap.get(3).unwrap().as_str(),
            cap.get(4).unwrap().as_str(),
            cap.get(5).unwrap().as_str(),
            cap.get(6).unwrap().as_str()
        )
        .as_str()
    )
}

#[test]
fn use_extra_args() {
    let exe_path = ClangTool::ClangTidy
        .get_exe_path(
            &RequestedVersion::from_str(
                env::var("CLANG_VERSION").unwrap_or("".to_string()).as_str(),
            )
            .unwrap(),
        )
        .unwrap();
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let file = FileObj::new(PathBuf::from("demo/demo.cpp"));
    let arc_file = Arc::new(Mutex::new(file));
    let extra_args = vec!["-std=c++17".to_string(), "-Wall".to_string()];
    let clang_params = ClangParams {
        style: "".to_string(),
        tidy_checks: "".to_string(), // use .clang-tidy config file
        lines_changed_only: LinesChangedOnly::Off,
        database: None,
        extra_args: extra_args.clone(), // <---- the reason for this test
        database_json: None,
        format_filter: None,
        tidy_filter: None,
        clang_tidy_command: Some(exe_path),
        clang_format_command: None,
        repo_root: tmp_workspace.path().to_path_buf(),
    };
    let mut file_lock = arc_file.lock().unwrap();
    fs::create_dir_all(clang_params.get_cache_path()).unwrap();
    let logs = run_clang_tidy(&mut file_lock, &clang_params)
        .unwrap()
        .0
        .into_iter()
        .filter_map(|(_lvl, msg)| {
            if msg.contains("Running ") {
                Some(msg)
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let args = &logs
        .first()
        .expect("expected a log message about invoked clang-tidy command")
        .split(' ')
        .collect::<Vec<&str>>();
    println!("args: {:?}", args);
    for arg in &extra_args {
        let extra_arg = format!("--extra-arg={arg}");
        assert!(args.contains(&extra_arg.as_str()));
    }
}

#[test]
fn skip_parse_tidy_diagnostic_rationale() {
    let tidy_out = r#"
TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:46:19: error: use of undeclared identifier 'readFreeImageTexture' [clang-diagnostic-error]
   46 |            return readFreeImageTexture(reader);
      |                   ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:659:32: note: in instantiation of function template specialization 'tb::mdl::(anonymous namespace)::loadTexture(const std::string &)::(anonymous class)::operator()<std::shared_ptr<tb::fs::File>>' requested here
  659 |     using Fn_Result = decltype(f(std::declval<Value&&>()));
      |                                ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:2949:29: note: in instantiation of function template specialization 'kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>::and_then<(lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
 2949 |   return std::forward<R>(r).and_then(t.and_then);
      |                             ^
TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:32: note: in instantiation of function template specialization 'kdl::detail::operator|<kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>, (lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
   44 |   return diskFS.openFile(name) | kdl::and_then([](const auto& file) {
      |                                ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:661:19: error: static assertion failed due to requirement 'is_result_v<int>': Function must return a result type [clang-diagnostic-error]
  661 |     static_assert(is_result_v<Fn_Result>, "Function must return a result type");
      |                   ^~~~~~~~~~~~~~~~~~~~~~
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:2949:29: note: in instantiation of function template specialization 'kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>::and_then<(lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
 2949 |   return std::forward<R>(r).and_then(t.and_then);
      |                             ^
TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:32: note: in instantiation of function template specialization 'kdl::detail::operator|<kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>, (lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
   44 |   return diskFS.openFile(name) | kdl::and_then([](const auto& file) {
      |                                ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:663:77: error: no type named 'type' in 'kdl::detail::chain_results<kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>, int>' [clang-diagnostic-error]
  663 |     using Cm_Result = typename detail::chain_results<My_Result, Fn_Result>::type;
      |                       ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~^~~~
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:667:48: error: no matching function for call to object of type 'const (lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)' [clang-diagnostic-error]
  667 |         [&](value_type&& v) { return Cm_Result{f(std::move(v))}; },
      |                                                ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:667:29: note: while substituting into a lambda expression here
  667 |         [&](value_type&& v) { return Cm_Result{f(std::move(v))}; },
      |                             ^
TrenchBroom/TrenchBroom/lib/KdLib/include/kd/result.h:2949:29: note: in instantiation of function template specialization 'kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>::and_then<(lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
 2949 |   return std::forward<R>(r).and_then(t.and_then);
      |                             ^
TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:32: note: in instantiation of function template specialization 'kdl::detail::operator|<kdl::result<std::shared_ptr<tb::fs::File>, kdl::result_error>, (lambda at TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48)>' requested here
   44 |   return diskFS.openFile(name) | kdl::and_then([](const auto& file) {
      |                                ^
TrenchBroom/TrenchBroom/common/test/src/mdl/tst_ReadFreeImageTexture.cpp:44:48: note: candidate template ignored: substitution failure [with file:auto = typename std::remove_reference<shared_ptr<File> &>::type]
   44 |   return diskFS.openFile(name) | kdl::and_then([](const auto& file) {
      |                                                ^
"#;
    let advice = parse_tidy_output(tidy_out.as_bytes(), &None, &PathBuf::from(".")).unwrap();
    assert_eq!(advice.len(), 4);
    for note in advice {
        assert!(note.diagnostic.contains('-'));
        assert!(!note.diagnostic.contains(' '));
    }
}

fn dummy_note(filename: &str, fixed_lines: Vec<u32>) -> TidyNotification {
    TidyNotification {
        filename: filename.to_string(),
        line: 5,
        cols: 1,
        severity: String::from("warning"),
        rationale: String::from("a dummy rationale"),
        diagnostic: String::from("modernize-use-auto"),
        suggestion: vec![],
        fixed_lines,
        skipped_fixes: vec![],
    }
}

#[test]
fn tally_counts_matching_notes() {
    use super::{TidyAdvice, tally_tidy_advice};

    let mut file = FileObj::new(PathBuf::from("demo.cpp"));
    file.tidy_advice = Some(TidyAdvice {
        notes: vec![
            // matches the file name -> counted
            dummy_note("demo.cpp", vec![]),
            // does not match the file name -> not counted
            dummy_note("other.cpp", vec![]),
        ],
    });
    let files = vec![Arc::new(Mutex::new(file))];
    assert_eq!(tally_tidy_advice(&files).unwrap(), 1);
}

#[test]
fn suggestion_help_with_and_without_fixes() {
    use super::TidyAdvice;

    let advice = TidyAdvice {
        notes: vec![dummy_note("demo.cpp", vec![5])],
    };
    // A fixed line within the range yields a "diagnostic(s)" header plus the rationale.
    let help = advice.get_suggestion_help(1, 10);
    assert!(help.contains("clang-tidy diagnostic(s)"));
    assert!(help.contains("a dummy rationale"));
    // No fixed line within the range falls back to a plain "suggestion" header.
    let help = advice.get_suggestion_help(20, 30);
    assert!(help.contains("clang-tidy suggestion"));
}

#[test]
fn suggestion_help_with_skipped_fixes() {
    use super::{SkippedFix, TidyAdvice};

    let mut note = dummy_note("demo.cpp", vec![5]);
    note.skipped_fixes.push(SkippedFix {
        file: "demo.h".to_string(),
        reason: "it is not one of the analyzed files".to_string(),
    });
    let advice = TidyAdvice { notes: vec![note] };
    let help = advice.get_suggestion_help(5, 5);
    assert!(help.contains(
        "\n  - :warning: Part of this fix was not applied to `demo.h` \
        because it is not one of the analyzed files.\n"
    ));
}

#[test]
fn parse_non_utf8_output() {
    let err = parse_tidy_output(&[0xFF, 0xFE], &None, &PathBuf::from(".")).unwrap_err();
    assert!(matches!(err, ClangCaptureError::NonUtf8Output { .. }));
}

#[test]
fn parse_with_database_units() {
    let build_dir = env::current_dir().unwrap().join("build");
    let database_json = Some(vec![
        CompilationUnit {
            directory: build_dir.to_string_lossy().to_string(),
            file: "../other.cpp".to_string(),
        },
        CompilationUnit {
            directory: build_dir.to_string_lossy().to_string(),
            file: "../demo.cpp".to_string(),
        },
    ]);
    let tidy_out = format!(
        "{}:1:1: warning: some rationale [modernize-use-auto]\n\
        {}:2:1: warning: other rationale [modernize-use-auto]\n",
        build_dir.join("../demo.cpp").to_string_lossy(),
        build_dir.join("../unknown.cpp").to_string_lossy(),
    );
    let notes = parse_tidy_output(
        tidy_out.as_bytes(),
        &database_json,
        &env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(notes.len(), 2);
    // path of a file in the database is resolved and made relative to the repo root
    assert_eq!(notes[0].filename, "demo.cpp");
    // path of a file not in the database is also made relative to the repo root
    assert_eq!(notes[1].filename, "unknown.cpp");

    // clang-tidy can also report the path exactly as written in the database (eg. meson)
    let notes = parse_tidy_output(
        b"../demo.cpp:1:1: warning: some rationale [modernize-use-auto]\n",
        &database_json,
        &env::current_dir().unwrap(),
    )
    .unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].filename, "demo.cpp");
}

fn make_clang_params(
    repo_root: &std::path::Path,
    clang_tidy_command: Option<PathBuf>,
) -> ClangParams {
    ClangParams {
        tidy_checks: "-*,modernize-use-auto".to_string(),
        clang_tidy_command,
        repo_root: repo_root.to_path_buf(),
        ..Default::default()
    }
}

#[test]
fn tidy_missing_source_file() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let clang_params = make_clang_params(tmp_workspace.path(), Some(PathBuf::from("clang-tidy")));
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/missing.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::ReadFileFailed { .. }));
}

#[test]
fn tidy_command_not_found() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let clang_params = make_clang_params(
        tmp_workspace.path(),
        Some(tmp_workspace.path().join("non-existent-clang-tidy")),
    );
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::FailedToRunCommand { .. }));
    // the source file is left untouched
    assert_eq!(
        fs::read_to_string(tmp_workspace.path().join("demo/demo.cpp")).unwrap(),
        fs::read_to_string("tests/demo/demo.cpp").unwrap()
    );
}

#[cfg(unix)]
#[test]
fn tidy_cache_not_writable() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let tools_dir = tempfile::tempdir().unwrap();
    let fake_tidy = crate::test_common::make_fake_tool(tools_dir.path(), "clang-tidy", "");
    let clang_params = make_clang_params(tmp_workspace.path(), Some(fake_tidy));
    // a directory where the patched file should be cached
    fs::create_dir_all(clang_params.get_cache_path().join("demo/demo.cpp")).unwrap();
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::WriteFileFailed { .. }));
}

#[cfg(unix)]
#[test]
fn tidy_non_utf8_output() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let tools_dir = tempfile::tempdir().unwrap();
    let fake_tidy = crate::test_common::make_fake_tool(
        tools_dir.path(),
        "clang-tidy",
        r"printf '\377\376'; printf 'some summary' >&2",
    );
    let clang_params = make_clang_params(tmp_workspace.path(), Some(fake_tidy));
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::NonUtf8Output { .. }));
}

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

const FIX_HEADER: &str = "#pragma once\nvoid set(int *p);\n";
const FIX_SOURCE: &str = "#include \"fix.h\"\nvoid set(int *p) {\n  int x = *p;\n  (void)x;\n}\n";

/// A fix for a diagnostic in the analyzed source file can also change a header
/// (see cpp-linter/cpp-linter-rs#475).
/// Neither file in the worktree should be modified,
/// but the fix for the analyzed file should be cached.
#[test]
fn fixes_do_not_modify_worktree() {
    let tmp_workspace = tempfile::TempDir::with_prefix("cpp-linter-unit-tests_").unwrap();
    fs::write(tmp_workspace.path().join("fix.h"), FIX_HEADER).unwrap();
    fs::write(tmp_workspace.path().join("fix.cpp"), FIX_SOURCE).unwrap();
    let clang_params = ClangParams {
        tidy_checks: "-*,readability-non-const-parameter".to_string(),
        clang_tidy_command: Some(get_tidy_exe()),
        repo_root: tmp_workspace.path().to_path_buf(),
        ..Default::default()
    };
    let file = Mutex::new(FileObj::new(PathBuf::from("fix.cpp")));
    let mut file = file.lock().unwrap();
    let (_logs, fixes) = run_clang_tidy(&mut file, &clang_params).unwrap();

    // worktree is untouched
    assert_eq!(
        fs::read_to_string(tmp_workspace.path().join("fix.h")).unwrap(),
        FIX_HEADER
    );
    assert_eq!(
        fs::read_to_string(tmp_workspace.path().join("fix.cpp")).unwrap(),
        FIX_SOURCE
    );

    // the fix for the analyzed file is cached
    let patched = fs::read_to_string(file.patched_path.as_ref().unwrap()).unwrap();
    assert_eq!(
        patched,
        FIX_SOURCE.replace("set(int *p)", "set(const int *p)")
    );
    // the exported fixes are cleaned up
    assert!(
        !fs::read_dir(clang_params.get_cache_path())
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".yml"))
    );

    let notes = &file.tidy_advice.as_ref().unwrap().notes;
    let note = notes
        .iter()
        .find(|n| n.diagnostic == "readability-non-const-parameter")
        .unwrap();
    assert_eq!(note.fixed_lines, vec![2]);

    // the fix for the header is returned (to be applied later, if allowed)
    assert_eq!(fixes.own.len(), 1);
    assert_eq!(fixes.others.len(), 1);
    let header_fix = &fixes.others[0];
    assert_eq!(header_fix.diagnostic, "readability-non-const-parameter");
    assert!(header_fix.path.ends_with("fix.h"));
}

fn dummy_fixes(target: &Path) -> String {
    let target = target.to_string_lossy();
    format!(
        r#"---
MainSourceFile:  '{target}'
Diagnostics:
  - DiagnosticName:  modernize-use-auto
    DiagnosticMessage:
      Message:         'a ''quoted'' rationale'
      FilePath:        '{target}'
      FileOffset:      7
      Replacements:
        - FilePath:        'fix.h'
          Offset:          0
          Length:          4
          ReplacementText: 'nope'
        - FilePath:        '{target}'
          Offset:          7
          Length:          3
          ReplacementText: "auto\nx"
        # a duplicate is ignored
        - FilePath:        '{target}'
          Offset:          7
          Length:          3
          ReplacementText: "auto\nx"
        # an overlapping fix is ignored
        - FilePath:        '{target}'
          Offset:          8
          Length:          1
          ReplacementText: 'overlap'
        # out-of-bounds fix is ignored
        - FilePath:        '{target}'
          Offset:          100
          Length:          1
          ReplacementText: 'oob'
    Level:           Warning
    BuildDirectory:  '{build_dir}'
...
"#,
        build_dir = target
            .trim_end_matches("demo.cpp")
            .trim_end_matches(['/', '\\']),
    )
}

#[test]
fn apply_exported_fixes() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("demo.cpp");
    let content = b"line\n  int y;\n";
    let fixes: ExportedFixes = serde_saphyr::from_str(&dummy_fixes(&target)).unwrap();
    assert_eq!(fixes.diagnostics.len(), 1);
    let mut notes = vec![dummy_note("demo.cpp", vec![])];
    notes[0].line = 2;
    let mut logs = vec![];
    let mut paths = FixPaths::new(&target, tmp.path());
    let (patched, tidy_fixes) = apply_fixes(&fixes, &mut paths, content, &mut notes, &mut logs);
    assert_eq!(String::from_utf8(patched).unwrap(), "line\n  auto\nx y;\n");
    assert_eq!(notes[0].fixed_lines, vec![2]);
    let messages = logs.into_iter().map(|(_, msg)| msg).collect::<Vec<_>>();
    assert_eq!(messages.len(), 2, "{messages:#?}");
    assert!(messages[0].contains("out of bounds"));
    assert!(messages[1].contains("overlaps"));
    // the fix for another file is not applied, but returned
    assert_eq!(
        tidy_fixes.others,
        vec![OtherFileFix {
            path: tmp.path().join("fix.h"),
            diagnostic: "modernize-use-auto".to_string(),
            replacement: Replacement {
                offset: 0,
                length: 4,
                text: "nope".to_string(),
            },
            // the matching note, to explain the fix once it is applied
            note: Some(TidyNotification {
                fixed_lines: vec![],
                skipped_fixes: vec![],
                ..notes[0].clone()
            }),
        }]
    );
}

#[test]
fn fix_paths_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("src/demo.cpp");
    let build_dir = tmp.path().join("build");
    let build_dir = build_dir.to_string_lossy();
    let mut paths = FixPaths::new(&target, tmp.path());
    // relative paths are resolved from the build directory
    assert!(paths.other_file(&build_dir, "../src/demo.cpp").is_none());
    // or from the working directory if no build directory is given
    assert!(paths.other_file("", "src/demo.cpp").is_none());
    assert_eq!(
        paths.other_file("", "src/demo.hpp"),
        Some(tmp.path().join("src/demo.hpp"))
    );
    // cached results are reused
    assert!(paths.other_file(&build_dir, "../src/demo.cpp").is_none());
}

/// Make a fake clang-tidy that runs `cmd` with the path given to `--export-fixes` as `$2`.
#[cfg(unix)]
fn make_fake_exporting_tidy_unix(dir: &Path, cmd: &str) -> PathBuf {
    crate::test_common::make_fake_tool(
        dir,
        "clang-tidy",
        &format!(
            r#"while [ "$#" -gt 0 ]; do
  if [ "$1" = "--export-fixes" ]; then {cmd}; fi
  shift
done"#
        ),
    )
}

#[cfg(unix)]
#[test]
fn tidy_bad_exported_fixes() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let tools_dir = tempfile::tempdir().unwrap();
    let fake_tidy =
        make_fake_exporting_tidy_unix(tools_dir.path(), r#"printf 'Diagnostics: [' > "$2""#);
    let clang_params = make_clang_params(tmp_workspace.path(), Some(fake_tidy));
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::ParseFixesFailed { .. }));
}

#[cfg(unix)]
#[test]
fn tidy_unreadable_exported_fixes() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let tools_dir = tempfile::tempdir().unwrap();
    // a directory where the exported fixes should be
    let fake_tidy = make_fake_exporting_tidy_unix(tools_dir.path(), r#"mkdir "$2""#);
    let clang_params = make_clang_params(tmp_workspace.path(), Some(fake_tidy));
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::ReadFileFailed { .. }));
}

#[test]
fn tidy_stale_fixes_not_removable() {
    let tmp_workspace = crate::test_common::setup_tmp_workspace();
    let clang_params = make_clang_params(tmp_workspace.path(), Some(PathBuf::from("clang-tidy")));
    // a directory where a stale file of exported fixes would be
    fs::create_dir_all(
        clang_params
            .get_cache_path()
            .join("demo/demo.cpp.fixes.yml"),
    )
    .unwrap();
    let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
    let err = run_clang_tidy(&mut file.lock().unwrap(), &clang_params).unwrap_err();
    assert!(matches!(err, ClangCaptureError::RemoveFileFailed { .. }));
}

/// A diagnostic reported in another file (eg. a header) can suggest a fix
/// for the analyzed file. The fixed lines are recorded in the header's note.
#[test]
fn fixed_lines_of_diagnostic_in_other_file() {
    let tmp = tempfile::tempdir().unwrap();
    let header = tmp.path().join("fix.h");
    fs::write(&header, FIX_HEADER).unwrap();
    let target = tmp.path().join("demo.cpp");
    let (target_str, header_str) = (target.to_string_lossy(), header.to_string_lossy());
    let yml = format!(
        r#"---
MainSourceFile:  '{target_str}'
Diagnostics:
  - DiagnosticName:  modernize-use-auto
    DiagnosticMessage:
      Message:         'a rationale'
      FilePath:        '{header_str}'
      FileOffset:      22
      Replacements:
        - FilePath:        '{target_str}'
          Offset:          7
          Length:          3
          ReplacementText: 'auto'
    Level:           Warning
    BuildDirectory:  '{build_dir}'
...
"#,
        build_dir = tmp.path().to_string_lossy(),
    );
    let fixes: ExportedFixes = serde_saphyr::from_str(&yml).unwrap();
    let mut notes = vec![dummy_note("fix.h", vec![])];
    notes[0].line = 2;
    let mut logs = vec![];
    let mut paths = FixPaths::new(&target, tmp.path());
    let (patched, tidy_fixes) = apply_fixes(
        &fixes,
        &mut paths,
        b"line\n  int y;\n",
        &mut notes,
        &mut logs,
    );
    assert_eq!(String::from_utf8(patched).unwrap(), "line\n  auto y;\n");
    assert!(logs.is_empty(), "{logs:#?}");
    assert!(tidy_fixes.others.is_empty());
    assert_eq!(notes[0].fixed_lines, vec![2]);
}
