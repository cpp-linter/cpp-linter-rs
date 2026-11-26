#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    default::Default,
    env,
    io::Read,
    ops::RangeInclusive,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use regex::Regex;
use semver::Version;
use tempfile::{NamedTempFile, tempdir};

use git_bot_feedback::{AnnotationLevel, FileAnnotation, FileFilter};

use super::{
    FormatRun, RestClient, USER_OUTREACH,
    annotations::annotation_cap_message,
    comment::{FileFindings, Findings, OMITTED_RESERVE},
    paths::{normalize_changed_file_name, prefix_feedback_path, repo_root_prefix},
};
use crate::{
    clang_tools::{
        ClangVersions,
        clang_format::FormatAdvice,
        clang_tidy::{SkippedFix, TidyAdvice, TidyNotification},
    },
    cli::{FeedbackInput, ThreadComments},
    common_fs::FileObj,
    error::ClientError,
    markdown::Status,
};
use clang_tools_manager::logger::try_init_logger;

// ************************* tests for step-summary and output variables

#[test]
fn normalize_changed_files_for_nested_repo_root() {
    let tmp_dir = tempdir().unwrap();
    let git_root = tmp_dir.path().join("checkout");
    let repo_root = git_root.join("member");
    std::fs::create_dir_all(&repo_root).unwrap();
    std::fs::write(git_root.join(".git"), "").unwrap();

    let prefix = repo_root_prefix(&repo_root, Some(tmp_dir.path().to_path_buf()));
    assert_eq!(prefix, PathBuf::from("member"));
    assert_eq!(
        prefix_feedback_path("src/a.cpp", &prefix),
        "member/src/a.cpp"
    );

    let file_filter = FileFilter::new(&["build"], &["cpp"], None);
    assert_eq!(
        normalize_changed_file_name("member/src/a.cpp", &prefix, &file_filter),
        Some(PathBuf::from("src/a.cpp"))
    );
    assert_eq!(
        normalize_changed_file_name("member/build/a.cpp", &prefix, &file_filter),
        None
    );
    assert_eq!(
        normalize_changed_file_name("other/src/a.cpp", &prefix, &file_filter),
        None
    );
}

async fn create_comment(is_lgtm: bool) -> (String, String) {
    let tmp_dir = tempdir().unwrap();
    let repo_root = env::current_dir().unwrap();
    unsafe {
        // ensure we are mimicking a CI platform
        env::set_var("GITHUB_ACTIONS", "true");
        env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter-rs");
        env::set_var("GITHUB_SHA", "deadbeef123");
        env::set_var("GITHUB_WORKSPACE", &repo_root);
    }
    let mut rest_api_client = RestClient::new(&repo_root).unwrap();
    #[cfg(feature = "bin")]
    try_init_logger();
    if env::var("ACTIONS_STEP_DEBUG").is_ok_and(|var| var == "true") {
        // assert!(rest_api_client.debug_enabled);
        log::set_max_level(log::LevelFilter::Debug);
    }
    let mut files = vec![];
    if !is_lgtm {
        for _i in 0..u16::MAX {
            let filename = String::from("tests/demo/demo.cpp");
            let mut file = FileObj::new(PathBuf::from(&filename));
            let notes = vec![TidyNotification {
                filename,
                line: 0,
                cols: 0,
                severity: String::from("note"),
                rationale: String::from("A test dummy rationale"),
                diagnostic: String::from("clang-diagnostic-warning"),
                suggestion: vec![],
                fixed_lines: vec![],
                skipped_fixes: vec![],
            }];
            file.tidy_advice = Some(TidyAdvice { notes });
            file.format_advice = Some(FormatAdvice {
                #[allow(clippy::single_range_in_vec_init)]
                replacements: vec![1..=2],
                ..Default::default()
            });
            files.push(Arc::new(Mutex::new(file)));
        }
    }
    let feedback_inputs = FeedbackInput {
        style: if is_lgtm {
            String::new()
        } else {
            String::from("file")
        },
        step_summary: true,
        file_annotations: false,
        ..Default::default()
    };
    let mut step_summary_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
    let mut gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
    unsafe {
        env::set_var("GITHUB_STEP_SUMMARY", step_summary_path.path());
        env::set_var("GITHUB_OUTPUT", gh_out_path.path());
    }
    let clang_versions = ClangVersions {
        format_version: Some(Version::new(1, 2, 3)),
        tidy_version: Some(Version::new(1, 2, 3)),
    };
    rest_api_client
        .post_feedback(&files, feedback_inputs, clang_versions)
        .await
        .unwrap();
    let mut step_summary_content = String::new();
    step_summary_path
        .read_to_string(&mut step_summary_content)
        .unwrap();
    assert!(&step_summary_content.contains(USER_OUTREACH));
    let mut gh_out_content = String::new();
    gh_out_path.read_to_string(&mut gh_out_content).unwrap();
    assert!(gh_out_content.starts_with("checks-failed="));
    (step_summary_content, gh_out_content)
}

#[tokio::test]
async fn check_comment_concerns() {
    let (comment, gh_out) = create_comment(false).await;
    assert!(comment.contains(" cpp-linter found "), "{comment}");
    assert!(comment.contains("<b>How to fix</b>"), "{comment}");
    let fmt_pattern = Regex::new(r"clang-format-checks-failed=(\d+)\n").unwrap();
    let tidy_pattern = Regex::new(r"clang-tidy-checks-failed=(\d+)\n").unwrap();
    for pattern in [fmt_pattern, tidy_pattern] {
        let number = pattern
            .captures(&gh_out)
            .expect("found no number of checks-failed")
            .get(1)
            .unwrap()
            .as_str()
            .parse::<u64>()
            .unwrap();
        assert!(number > 0);
    }
}

#[tokio::test]
async fn check_comment_lgtm() {
    unsafe {
        env::set_var("ACTIONS_STEP_DEBUG", "true");
    }
    let (comment, gh_out) = create_comment(true).await;
    assert!(comment.contains(" cpp-linter passed\n"), "{comment}");
    assert!(comment.contains(" files checked"), "{comment}");
    assert_eq!(
        gh_out,
        "checks-failed=0\nclang-format-checks-failed=0\nclang-tidy-checks-failed=0\n"
    );
}

// ************************* tests for other feedback paths

fn make_dummy_files(severity: &str) -> Vec<Arc<Mutex<FileObj>>> {
    let filename = String::from("tests/demo/demo.cpp");
    let mut file = FileObj::new(PathBuf::from(&filename));
    file.tidy_advice = Some(TidyAdvice {
        notes: vec![TidyNotification {
            filename,
            line: 1,
            cols: 1,
            severity: severity.to_string(),
            rationale: String::from("A test dummy rationale"),
            diagnostic: String::from("clang-diagnostic-error"),
            suggestion: vec![String::from("int main() {}")],
            fixed_lines: vec![1],
            skipped_fixes: vec![],
        }],
    });
    file.format_advice = Some(FormatAdvice {
        #[allow(clippy::single_range_in_vec_init)]
        replacements: vec![1..=2],
        ..Default::default()
    });
    vec![Arc::new(Mutex::new(file))]
}

/// With `--repo-root` in a subdirectory, file names and commands are relative
/// to the repository root, where the comment says to run them.
#[test]
fn comment_names_files_from_the_repository_root() {
    let files = make_dummy_files("warning");
    let format_run = FormatRun {
        style: "file",
        lines_only: false,
        path_prefix: Path::new("member"),
    };
    let comment =
        RestClient::make_comment(&files, 1, 1, &dummy_versions(), None, false, format_run).unwrap();
    assert!(
        comment.contains("**`member/tests/demo/demo.cpp`**"),
        "{comment}"
    );
    assert!(
        comment.contains("- `member/tests/demo/demo.cpp`, lines 1-2"),
        "{comment}"
    );
    assert!(
        comment.contains("clang-format -i --style=file member/tests/demo/demo.cpp\n"),
        "{comment}"
    );
    assert!(
        comment.contains("- member/tests/demo/demo.cpp:1:1: warning:"),
        "{comment}"
    );
    assert!(!comment.contains(" tests/demo/demo.cpp"), "{comment}");

    // a style file given relative to `--repo-root`
    let lines = [RangeInclusive::new(1, 2)];
    let style_file = FormatRun {
        style: "file:config/.clang-format",
        ..format_run
    };
    assert_eq!(
        style_file.command([("a.cpp", &lines[..])]),
        "clang-format -i --style=file:member/config/.clang-format a.cpp"
    );
    let absolute = FormatRun {
        style: "file:/etc/.clang-format",
        ..format_run
    };
    assert_eq!(
        absolute.command([("a.cpp", &lines[..])]),
        "clang-format -i --style=file:/etc/.clang-format a.cpp"
    );

    let annotations = RestClient::build_annotations(&files, format_run).unwrap();
    let format_annotation = annotations
        .iter()
        .find(|a| a.title.as_deref() == Some("clang-format"))
        .unwrap();
    assert!(
        format_annotation
            .message
            .ends_with("Run: clang-format -i --style=file member/tests/demo/demo.cpp"),
        "{}",
        format_annotation.message
    );
}

#[test]
fn missing_includes_get_a_hint() {
    let files = make_dummy_files("error");
    let comment =
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "file".into())
            .unwrap();
    assert!(!comment.contains("compilation database"), "{comment}");

    files[0].lock().unwrap().tidy_advice.as_mut().unwrap().notes[0].rationale =
        String::from("'demo.hpp' file not found");
    let comment =
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "file".into())
            .unwrap();
    assert!(
        comment.contains("clang-tidy could not find 1 included file, so"),
        "{comment}"
    );
    // the hint comes before the findings
    let hint = comment.find("Generate a compilation database").unwrap();
    assert!(hint < comment.find("demo.cpp").unwrap(), "{comment}");
}

#[test]
fn skipped_fix_with_pipe_stays_in_its_cell() {
    let files = make_dummy_files("warning");
    files[0].lock().unwrap().tidy_advice.as_mut().unwrap().notes[0]
        .skipped_fixes
        .push(SkippedFix {
            file: "a|b.h".to_string(),
            reason: "it is not one of the analyzed files".to_string(),
        });
    let comment =
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "file".into())
            .unwrap();
    assert!(comment.contains("`a\\|b.h`"), "{comment}");
}

#[test]
fn tidy_comment_with_skipped_fixes() {
    let files = make_dummy_files("warning");
    {
        let mut file = files[0].lock().unwrap();
        let note = &mut file.tidy_advice.as_mut().unwrap().notes[0];
        note.skipped_fixes.push(SkippedFix {
            file: "tests/demo/demo.hpp".to_string(),
            reason: "it is not one of the analyzed files".to_string(),
        });
    }
    let comment =
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, true, "file".into())
            .unwrap();
    assert!(
        comment.contains(
            " (fixable)<br>clang-diagnostic-error<br><img alt=\"⚠\" width=\"20\" align=\"absmiddle\" \
            src=\"https://cpp-linter.github.io/m/w.png\"> \
            Part of this fix was not \
            applied to `tests/demo/demo.hpp` because it is not one of the analyzed files. |\n"
        ),
        "{comment}"
    );
    assert!(
        comment.contains("Or apply every fix at once: this step's `fix-patch-path`"),
        "{comment}"
    );
    // The prompt is for the clang-tidy findings, not another way to do the formatting.
    assert!(
        comment.contains("\nFor the clang-tidy findings, give this to your coding agent:"),
        "{comment}"
    );
}

fn dummy_versions() -> ClangVersions {
    ClangVersions {
        format_version: Some(Version::new(1, 2, 3)),
        tidy_version: Some(Version::new(1, 2, 3)),
    }
}

#[tokio::test]
async fn annotations_over_cap_on_github() {
    let tmp_dir = tempdir().unwrap();
    let repo_root = env::current_dir().unwrap();
    let gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
    unsafe {
        env::set_var("GITHUB_ACTIONS", "true");
        env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter-rs");
        env::set_var("GITHUB_SHA", "deadbeef123");
        env::set_var("GITHUB_WORKSPACE", &repo_root);
        env::set_var("GITHUB_OUTPUT", gh_out_path.path());
    }
    let mut rest_api_client = RestClient::new(&repo_root).unwrap();
    // 11 warnings: one more than GitHub shows in a step
    let files = (0..11)
        .flat_map(|_| make_dummy_files("warning"))
        .collect::<Vec<_>>();
    let feedback_inputs = FeedbackInput {
        file_annotations: true,
        ..Default::default()
    };
    let checks_failed = rest_api_client
        .post_feedback(&files, feedback_inputs, dummy_versions())
        .await
        .unwrap();
    assert_eq!(checks_failed, 22);
}

#[tokio::test]
async fn thread_comment_without_summary() {
    unsafe {
        env::remove_var("GITHUB_ACTIONS");
    }
    let tmp_dir = tempdir().unwrap();
    let mut rest_api_client = RestClient::new(tmp_dir.path()).unwrap();
    for thread_comments in [ThreadComments::On, ThreadComments::Update] {
        let feedback_inputs = FeedbackInput {
            thread_comments,
            repo_root: tmp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let checks_failed = rest_api_client
            .post_feedback(
                &make_dummy_files("error"),
                feedback_inputs,
                dummy_versions(),
            )
            .await
            .unwrap();
        assert_eq!(checks_failed, 2);
    }
}

#[test]
fn annotation_severity() {
    let annotations =
        RestClient::make_annotations(&make_dummy_files("error"), "llvm".into()).unwrap();
    assert!(
        annotations
            .iter()
            .any(|a| matches!(a.severity, AnnotationLevel::Error))
    );
    let annotations =
        RestClient::make_annotations(&make_dummy_files("note"), "llvm".into()).unwrap();
    // both the clang-format and clang-tidy annotations are notices
    assert_eq!(annotations.len(), 2);
    assert!(
        annotations
            .iter()
            .all(|a| matches!(a.severity, AnnotationLevel::Notice))
    );
}

#[test]
fn tidy_annotation_points_to_line() {
    let annotations =
        RestClient::make_annotations(&make_dummy_files("warning"), "llvm".into()).unwrap();
    let tidy = annotations
        .iter()
        .find(|a| matches!(a.severity, AnnotationLevel::Warning))
        .unwrap();
    assert_eq!(tidy.start_line, Some(1));
    assert_eq!(tidy.start_column, Some(1));
    assert_eq!(tidy.title.as_deref(), Some("clang-diagnostic-error"));
    assert!(
        tidy.fmt_github()
            .starts_with("::warning file=tests/demo/demo.cpp,line=1,col=1,")
    );
}

#[test]
fn short_annotation_titles() {
    let files = make_dummy_files("warning");
    let annotations = RestClient::build_annotations(&files, "llvm".into()).unwrap();
    let titles = annotations
        .iter()
        .map(|a| a.title.as_deref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(titles, vec!["clang-format", "clang-diagnostic-error"]);

    let file = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
    let file = Arc::new(Mutex::new(file));
    file.lock().unwrap().format_advice = Some(FormatAdvice {
        failed: true,
        ..Default::default()
    });
    let annotations = RestClient::build_annotations(&[file], "llvm".into()).unwrap();
    assert_eq!(annotations[0].title.as_deref(), Some("clang-format"));
}

#[test]
fn format_annotation_points_to_first_line() {
    let file = FileObj::new(PathBuf::from("src/demo.cpp"));
    let file = Arc::new(Mutex::new(file));
    file.lock().unwrap().format_advice = Some(FormatAdvice {
        replacements: vec![20..=20, 3..=5, 6..=11],
        ..Default::default()
    });
    let annotations =
        RestClient::build_annotations(std::slice::from_ref(&file), "file".into()).unwrap();
    assert_eq!(annotations[0].start_line, Some(3));
    assert_eq!(
        annotations[0].message,
        "Lines 3-11, 20 do not match the style in .clang-format. \
        Run: clang-format -i --style=file src/demo.cpp"
    );
    file.lock().unwrap().format_advice = Some(FormatAdvice {
        replacements: vec![RangeInclusive::new(4, 4)],
        ..Default::default()
    });
    let annotations =
        RestClient::build_annotations(&[file], "{BasedOnStyle: llvm}".into()).unwrap();
    assert_eq!(
        annotations[0].message,
        "Line 4 does not match the configured style. \
        Run: clang-format -i '--style={BasedOnStyle: llvm}' src/demo.cpp"
    );
}

/// When clang-format only checked the changed lines, the annotation sits on the
/// first added line that needs formatting, and its command formats only those lines.
#[test]
fn format_annotation_for_changed_lines() {
    let mut file = FileObj::new(PathBuf::from("src/demo.cpp"));
    file.added_lines = vec![12, 13];
    file.format_advice = Some(FormatAdvice {
        replacements: vec![10..=12, 20..=21],
        ..Default::default()
    });
    let format_run = FormatRun {
        style: "file",
        lines_only: true,
        path_prefix: Path::new(""),
    };
    let annotations =
        RestClient::build_annotations(&[Arc::new(Mutex::new(file))], format_run).unwrap();
    assert_eq!(annotations[0].start_line, Some(12));
    assert_eq!(
        annotations[0].message,
        "Lines 10-12, 20-21 do not match the style in .clang-format. \
        Run: clang-format -i --style=file --lines=10:12 --lines=20:21 src/demo.cpp"
    );
}

#[test]
fn format_commands() {
    let lines = [1..=2, 3..=3];
    let files = [("a.cpp", &lines[..]), ("b c.cpp", &lines[..1])];
    let whole = FormatRun::from("llvm");
    assert_eq!(
        whole.command(files),
        "clang-format -i --style=llvm a.cpp 'b c.cpp'"
    );
    let scoped = FormatRun {
        style: "llvm",
        lines_only: true,
        path_prefix: Path::new(""),
    };
    assert_eq!(
        scoped.command(files),
        "clang-format -i --style=llvm --lines=1:3 a.cpp\n\
        clang-format -i --style=llvm --lines=1:2 'b c.cpp'"
    );
    // no style given: clang-format looks for a .clang-format file
    assert_eq!(
        FormatRun::from("").command([("a.cpp", &lines[..])]),
        "clang-format -i --style=file a.cpp"
    );
    // a name that starts with `-` would be read as an option
    let dash = [("-a.cpp", &lines[..1]), ("-b c.cpp", &lines[..1])];
    assert_eq!(
        whole.command(dash),
        "clang-format -i --style=llvm ./-a.cpp './-b c.cpp'"
    );
    assert_eq!(
        scoped.command(dash),
        "clang-format -i --style=llvm --lines=1:2 ./-a.cpp\n\
        clang-format -i --style=llvm --lines=1:2 './-b c.cpp'"
    );
}

#[test]
fn findings_without_formatting() {
    let findings = Findings(vec![FileFindings {
        name: String::from("odd`name.cpp"),
        format_failed: false,
        format_lines: vec![],
        tidy_notes: vec![],
    }]);
    // a backtick can't go in a code span, so the name is escaped instead
    assert_eq!(findings.0[0].display_name(), "odd\\`name.cpp");
    assert_eq!(
        findings.format_result(),
        (Status::Passed, String::from("all files formatted"))
    );
    let mut comment = String::from("heading\n");
    findings.push_format_section(&mut comment, |_| true);
    assert_eq!(comment, "heading\n");
}

#[test]
fn format_failure_feedback() {
    let file = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
    let file = Arc::new(Mutex::new(file));
    file.lock().unwrap().format_advice = Some(FormatAdvice {
        failed: true,
        ..Default::default()
    });
    let files = vec![file];
    let annotations = RestClient::make_annotations(&files, "llvm".into()).unwrap();
    assert_eq!(annotations.len(), 1);
    assert!(matches!(annotations[0].severity, AnnotationLevel::Warning));
    assert_eq!(annotations[0].title.as_deref(), Some("clang-format"));
    let comment =
        RestClient::make_comment(&files, 1, 0, &dummy_versions(), None, false, "llvm".into())
            .unwrap();
    assert!(
        comment.contains("- `tests/demo/demo.cpp`: clang-format failed to run; see the logs"),
        "{comment}"
    );
    assert!(comment.contains("/m/hf.png"), "{comment}");
    assert!(
        comment.contains("/m/f.png\"> **clang-format** 1.2.3 | clang-format failed on 1 file |"),
        "{comment}"
    );
    // nothing to format, so no "How to fix"
    assert!(!comment.contains("How to fix"), "{comment}");
}

#[test]
fn comment_without_tidy_concerns() {
    let files = make_dummy_files("warning");
    files[0].lock().unwrap().tidy_advice = None;
    let comment =
        RestClient::make_comment(&files, 1, 0, &dummy_versions(), None, false, "llvm".into())
            .unwrap();
    assert!(
        comment.contains(" cpp-linter found 1 issue in 1 file\n"),
        "{comment}"
    );
    assert!(
        comment.contains("/m/w.png\"> **clang-format** 1.2.3 | 1 file needs formatting |"),
        "{comment}"
    );
    assert!(
        comment.contains("/m/p.png\"> **clang-tidy** 1.2.3 | no findings |"),
        "{comment}"
    );
    assert!(
        comment.contains("- `tests/demo/demo.cpp`, lines 1-2\n"),
        "{comment}"
    );
    assert!(!comment.contains("<b>clang-tidy</b>"), "{comment}");
    assert!(
        comment.contains("```bash\nclang-format -i --style=llvm tests/demo/demo.cpp\n```"),
        "{comment}"
    );
    assert!(!comment.contains("AI coding agent"), "{comment}");
}

#[tokio::test]
async fn summary_output_file_errors() {
    unsafe {
        env::remove_var("GITHUB_ACTIONS");
    }
    let tmp_dir = tempdir().unwrap();
    let mut rest_api_client = RestClient::new(tmp_dir.path()).unwrap();

    // the parent "directory" is actually a file
    let not_a_dir = tmp_dir.path().join("not-a-dir");
    std::fs::write(&not_a_dir, "").unwrap();
    let feedback_inputs = FeedbackInput {
        summary_output_file: Some(not_a_dir.join("summary.md")),
        repo_root: tmp_dir.path().to_path_buf(),
        ..Default::default()
    };
    let err = rest_api_client
        .post_feedback(
            &make_dummy_files("warning"),
            feedback_inputs,
            dummy_versions(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::MkDirFailed { .. }));

    // the output "file" is actually a directory
    let feedback_inputs = FeedbackInput {
        summary_output_file: Some(tmp_dir.path().to_path_buf()),
        repo_root: tmp_dir.path().to_path_buf(),
        ..Default::default()
    };
    let err = rest_api_client
        .post_feedback(
            &make_dummy_files("warning"),
            feedback_inputs,
            dummy_versions(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ClientError::SummaryOutputFileWriteFailed { .. }
    ));

    // the output path has no parent (and is not a writable file)
    #[cfg(unix)]
    {
        let feedback_inputs = FeedbackInput {
            summary_output_file: Some(PathBuf::from("/")),
            repo_root: tmp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let err = rest_api_client
            .post_feedback(
                &make_dummy_files("warning"),
                feedback_inputs,
                dummy_versions(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            ClientError::SummaryOutputFileWriteFailed { .. }
        ));
    }
}

#[test]
fn comment_with_poisoned_mutex() {
    let files = make_dummy_files("warning");
    crate::test_common::poison_mutex(&files[0]);
    // poisoned while making the clang-format comment
    assert!(
        RestClient::make_comment(&files, 1, 1, &dummy_versions(), None, false, "llvm".into())
            .is_err()
    );
    // poisoned while making the clang-tidy comment
    assert!(
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, false, "llvm".into())
            .is_err()
    );
}

#[test]
fn annotations_with_poisoned_mutex() {
    let files = make_dummy_files("warning");
    crate::test_common::poison_mutex(&files[0]);
    assert!(matches!(
        RestClient::make_annotations(&files, "llvm".into()),
        Err(ClientError::MutexPoisoned(_))
    ));
}

/// A clang tool that did not run has no version, so it gets no row in the table.
#[test]
fn comment_without_a_tool_version() {
    let files = make_dummy_files("warning");
    let versions = ClangVersions {
        format_version: Some(Version::new(1, 2, 3)),
        tidy_version: None,
    };
    let comment =
        RestClient::make_comment(&files, 1, 0, &versions, None, false, "file".into()).unwrap();
    assert!(comment.contains("**clang-format** 1.2.3 |"), "{comment}");
    assert!(!comment.contains("**clang-tidy**"), "{comment}");
}

/// Without files to format, the patch is the first way to fix, not an alternative.
#[test]
fn patch_is_the_first_fix() {
    let files = make_dummy_files("warning");
    files[0].lock().unwrap().format_advice = None;
    let comment =
        RestClient::make_comment(&files, 0, 1, &dummy_versions(), None, true, "file".into())
            .unwrap();
    assert!(
        comment.contains("\nApply every fix at once: this step's `fix-patch-path`"),
        "{comment}"
    );
    assert!(!comment.contains("Or apply"), "{comment}");
}

#[test]
fn comment_too_long_for_tidy_note() {
    let files = make_dummy_files("error");
    let unlimited =
        RestClient::make_comment(&files, 1, 1, &dummy_versions(), None, false, "llvm".into())
            .unwrap();
    assert!(unlimited.contains("| 1 | **error:** A test dummy rationale"));
    assert!(unlimited.contains("/m/hf.png"), "{unlimited}");
    assert!(unlimited.contains("How to fix"));
    // No room for the clang-tidy note and the clang-format list.
    let head = unlimited.find("\n<details").unwrap();
    let limited = RestClient::make_comment(
        &files,
        1,
        1,
        &dummy_versions(),
        Some((head + USER_OUTREACH.len() + OMITTED_RESERVE + 100) as u64),
        false,
        "llvm".into(),
    )
    .unwrap();
    assert!(!limited.contains("A test dummy rationale"), "{limited}");
    assert!(
        limited.contains("1 more is not shown here because the comment would be too long."),
        "{limited}"
    );
    assert!(limited.ends_with(USER_OUTREACH));
}

/// With many findings and too little room, "How to fix" is kept and findings are left out.
#[test]
fn how_to_fix_outlasts_the_findings() {
    let files = make_dummy_files("warning");
    {
        let mut file = files[0].lock().unwrap();
        let notes = &mut file.tidy_advice.as_mut().unwrap().notes;
        let note = notes[0].clone();
        notes.extend(std::iter::repeat_n(note, 20));
    }
    let unlimited =
        RestClient::make_comment(&files, 1, 21, &dummy_versions(), None, false, "llvm".into())
            .unwrap();
    let head = unlimited.find("\n<details").unwrap();
    let how_to_fix_len = unlimited.len()
        - unlimited.find("\n<details><summary><b>How to fix").unwrap()
        - USER_OUTREACH.len();
    let max_len = head + how_to_fix_len + USER_OUTREACH.len() + OMITTED_RESERVE + 500;
    assert!(max_len < unlimited.len());
    let limited = RestClient::make_comment(
        &files,
        1,
        21,
        &dummy_versions(),
        Some(max_len as u64),
        false,
        "llvm".into(),
    )
    .unwrap();
    assert!(limited.len() <= max_len);
    assert!(limited.contains("<b>How to fix</b>"), "{limited}");
    assert!(limited.contains("more are not shown here"), "{limited}");
}

/// The prompt for an AI coding agent lists at most 50 findings.
#[test]
fn ai_prompt_is_capped() {
    let files = make_dummy_files("warning");
    {
        let mut file = files[0].lock().unwrap();
        let notes = &mut file.tidy_advice.as_mut().unwrap().notes;
        let note = notes[0].clone();
        notes.extend(std::iter::repeat_n(note, 59));
    }
    let comment =
        RestClient::make_comment(&files, 1, 60, &dummy_versions(), None, false, "llvm".into())
            .unwrap();
    let prompt = &comment[comment.find("Fix these clang-tidy findings").unwrap()..];
    assert_eq!(prompt.matches("A test dummy rationale").count(), 50);
    assert!(
        prompt.contains("This covers the first 50 of 60 findings."),
        "{prompt}"
    );
}

/// A length-limited comment never grows past its limit, even when both
/// sections have to leave out their findings.
#[test]
fn comment_stays_within_its_limit() {
    let files = make_dummy_files("warning");
    {
        let mut file = files[0].lock().unwrap();
        let notes = &mut file.tidy_advice.as_mut().unwrap().notes;
        let note = notes[0].clone();
        notes.extend(std::iter::repeat_n(note, 50));
    }
    let unlimited =
        RestClient::make_comment(&files, 1, 51, &dummy_versions(), None, true, "llvm".into())
            .unwrap();
    let head = unlimited.find("\n<details").unwrap();
    // From a limit that only fits the heading, the table and the footer.
    for max_len in (head + USER_OUTREACH.len())..unlimited.len() {
        let limited = RestClient::make_comment(
            &files,
            1,
            51,
            &dummy_versions(),
            Some(max_len as u64),
            true,
            "llvm".into(),
        )
        .unwrap();
        assert!(limited.len() <= max_len, "{} > {max_len}", limited.len());
    }
}

// ************************* tests for GitHub's annotation limits

fn annotations(warnings: usize, notices: usize) -> Vec<FileAnnotation> {
    let make = |severity| FileAnnotation {
        severity,
        path: "demo.cpp".to_string(),
        message: "a dummy message".to_string(),
        ..Default::default()
    };
    let mut result = vec![make(AnnotationLevel::Warning); warnings];
    result.extend(vec![make(AnnotationLevel::Notice); notices]);
    result
}

#[test]
fn annotations_within_limits() {
    let inputs = FeedbackInput::default();
    assert!(annotation_cap_message(&[], &inputs).is_none());
    assert!(annotation_cap_message(&annotations(10, 10), &inputs).is_none());
    let msg = annotation_cap_message(&annotations(11, 0), &inputs).unwrap();
    assert!(msg.ends_with("to get a list of every finding."), "{msg}");
}

#[test]
fn annotations_over_step_limit() {
    let inputs = FeedbackInput {
        step_summary: true,
        thread_comments: ThreadComments::Off,
        ..Default::default()
    };
    let msg = annotation_cap_message(&annotations(20, 1), &inputs).unwrap();
    assert!(
        msg.contains("so 10 of 20 warning annotations are not shown."),
        "{msg}"
    );
    assert!(
        msg.ends_with("The step summary lists every finding."),
        "{msg}"
    );

    let inputs = FeedbackInput {
        step_summary: false,
        thread_comments: ThreadComments::Update,
        ..Default::default()
    };
    let msg = annotation_cap_message(&annotations(11, 12), &inputs).unwrap();
    assert!(
        msg.contains(
            "so 1 of 11 warning annotations and 2 of 12 notice annotations are not shown."
        ),
        "{msg}"
    );
    assert!(
        msg.ends_with("The thread comment lists them, and the workflow log lists every finding."),
        "{msg}"
    );

    let inputs = FeedbackInput {
        step_summary: true,
        thread_comments: ThreadComments::On,
        ..Default::default()
    };
    let msg = annotation_cap_message(&annotations(0, 11), &inputs).unwrap();
    assert!(
        msg.ends_with("The step summary and the thread comment list every finding."),
        "{msg}"
    );
}

// ************************* tests for listing changed files

/// An ignored untracked file is not read, so one that can't be read doesn't stop the run.
#[cfg(unix)]
#[tokio::test]
async fn ignored_untracked_file_is_not_read() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};

    let tmp = tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(&root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.name=test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "init",
    ]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/ok.cpp"), "int main() {}\n").unwrap();
    fs::create_dir_all(root.join("vendor")).unwrap();
    let unreadable = root.join("vendor/broken.cpp");
    fs::write(&unreadable, "int x;\n").unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    // a local run, not a CI run
    unsafe {
        for var in [
            "GITHUB_ACTIONS",
            "GITEA_ACTIONS",
            "FORGEJO_ACTIONS",
            "GITLAB_CI",
        ] {
            env::remove_var(var);
        }
    }
    let cur_dir = env::current_dir().unwrap();
    env::set_current_dir(&root).unwrap();
    let client = RestClient::new(&root).unwrap();
    assert_eq!(client.client_kind(), "local");
    let file_filter = FileFilter::new(&["vendor"], &["cpp"], None);
    let files = client
        .get_list_of_changed_files(
            &file_filter,
            &git_bot_feedback::LinesChangedOnly::Off,
            &None,
            false,
        )
        .await;
    env::set_current_dir(cur_dir).unwrap();

    let names: Vec<PathBuf> = files.unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec![PathBuf::from("src/ok.cpp")]);
}
