//! This module holds functionality to export clang-format and clang-tidy findings as a
//! [GitLab Code Quality report](https://docs.gitlab.com/ci/testing/code_quality/#code-quality-report-format).
//!
//! GitLab reads the report from a `codequality` job artifact and shows the findings in
//! merge requests. The format is a JSON array of issues; it is a subset of the
//! [Code Climate](https://github.com/codeclimate/platform/blob/master/spec/analyzers/SPEC.md#data-types)
//! issue format.

use std::{
    collections::HashMap,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

// non-std crates
use serde::Serialize;
use sha2::{Digest, Sha256};

// project-specific modules/crates
use crate::{
    clang_tools::clang_format::describe_style,
    common_fs::{FileObj, is_stdout},
    error::ClientError,
};

/// The `check_name` (and tool name in fingerprints) used for clang-format findings.
const CLANG_FORMAT: &str = "clang-format";

/// The tool name used in fingerprints of clang-tidy findings.
const CLANG_TIDY: &str = "clang-tidy";

/// A GitLab Code Quality report about the clang-format and clang-tidy findings.
///
/// Use [`CodeQualityReport::new()`] to create one, and [`CodeQualityReport::write()`] to save it.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct CodeQualityReport {
    issues: Vec<Issue>,
}

#[derive(Debug, Serialize)]
struct Issue {
    description: String,
    check_name: String,
    fingerprint: String,
    severity: Severity,
    location: Location,
}

#[derive(Debug, Serialize)]
struct Location {
    path: String,
    lines: Lines,
}

#[derive(Debug, Serialize)]
struct Lines {
    begin: u32,
    end: u32,
}

/// The severities that GitLab accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Info,
    Minor,
    Major,
    Critical,
}

impl Severity {
    /// Map a clang-tidy severity to a GitLab severity.
    fn from_tidy(severity: &str) -> Self {
        match severity {
            "error" => Severity::Critical,
            "warning" => Severity::Major,
            // clang-tidy may also report "note" or "remark" severities
            _ => Severity::Info,
        }
    }
}

/// Make a stable fingerprint of a finding.
///
/// The fingerprint does not depend on line numbers, so a finding keeps its
/// fingerprint when code above it is added or removed. Instead it uses the
/// `tool`, the `check`, the file's `path` (relative to the repository root),
/// the (trimmed) text of the source line(s), and the `occurrence` index of
/// findings that are otherwise the same.
pub(crate) fn fingerprint(
    tool: &str,
    check: &str,
    path: &str,
    line_text: &str,
    occurrence: usize,
) -> String {
    let mut hasher = Sha256::new();
    for part in [tool, check, path, line_text, &occurrence.to_string()] {
        hasher.update(part.as_bytes());
        // separate the parts, so ("ab", "c") and ("a", "bc") differ
        hasher.update([0]);
    }
    // 128 bits (32 hex digits) are plenty to tell findings apart
    hasher.finalize()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The path of `file` relative to the git repository, with `/` separators and no `./` prefix.
///
/// `file` is relative to `repo_root` (the `--repo-root`), which is `path_prefix`
/// in the git repository (empty if `--repo-root` is the repository's root).
fn relative_path(file: &Path, repo_root: &Path, path_prefix: &Path) -> String {
    let path = path_prefix.join(file.strip_prefix(repo_root).unwrap_or(file));
    path.components()
        // drop `.` components (eg. a `./` prefix)
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        // a Windows-style path given on another platform
        .replace('\\', "/")
}

/// The source lines of a file, used to make fingerprints.
struct SourceLines(Vec<String>);

impl SourceLines {
    /// Read the lines of `file`. A file that cannot be read has no lines.
    fn read(file: &Path) -> Self {
        let content = std::fs::read(file).unwrap_or_default();
        Self(
            String::from_utf8_lossy(&content)
                .lines()
                .map(|line| line.trim().to_string())
                .collect(),
        )
    }

    /// The trimmed text of the given 1-based line numbers, joined with `\n`.
    fn text(&self, begin: u32, end: u32) -> String {
        (begin..=end)
            .filter_map(|line| {
                (line as usize)
                    .checked_sub(1)
                    .and_then(|index| self.0.get(index))
            })
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Collects issues and counts the occurrences of identical findings.
#[derive(Default)]
struct IssueBuilder {
    issues: Vec<Issue>,
    occurrences: HashMap<String, usize>,
}

impl IssueBuilder {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        tool: &str,
        check_name: &str,
        description: String,
        severity: Severity,
        path: &str,
        lines: Lines,
        source: &SourceLines,
    ) {
        let line_text = source.text(lines.begin, lines.end);
        let key = fingerprint(tool, check_name, path, &line_text, 0);
        let occurrence = self.occurrences.entry(key).or_default();
        let fingerprint = fingerprint(tool, check_name, path, &line_text, *occurrence);
        *occurrence += 1;
        self.issues.push(Issue {
            description,
            check_name: check_name.to_string(),
            fingerprint,
            severity,
            location: Location {
                path: path.to_string(),
                lines,
            },
        });
    }
}

impl CodeQualityReport {
    /// Create a Code Quality report from the clang-format and clang-tidy advice
    /// about the given `files`.
    ///
    /// Like other forms of feedback, only the findings about the given `files` are
    /// included. So, any filtering (eg. `--lines-changed-only`, `--ignore-tidy`) is
    /// respected. clang-tidy diagnostics about other files (eg. included headers) are
    /// skipped.
    ///
    /// The `style` is the `--style` value, used to describe clang-format findings.
    /// The source files are read from the given `repo_root` path to make fingerprints.
    ///
    /// GitLab needs paths relative to the git repository. So, if `repo_root` is a
    /// sub-directory of the git repository, then `path_prefix` is that sub-directory
    /// (relative to the git repository's root), and it is prepended to the file paths.
    pub fn new(
        files: &[Arc<Mutex<FileObj>>],
        style: &str,
        repo_root: &Path,
        path_prefix: &Path,
    ) -> Result<Self, ClientError> {
        let style_description = describe_style(style);
        let mut builder = IssueBuilder::default();
        for file in files {
            let file = file
                .lock()
                .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
            let format_advice = file.format_advice.as_ref().filter(|a| a.is_concern());
            let tidy_notes = file
                .tidy_advice
                .iter()
                .flat_map(|advice| &advice.notes)
                .filter(|note| Path::new(&note.filename) == file.name.as_path())
                .collect::<Vec<_>>();
            if format_advice.is_none() && tidy_notes.is_empty() {
                continue;
            }
            let path = relative_path(&file.name, repo_root, path_prefix);
            let source = SourceLines::read(&repo_root.join(&file.name));

            if let Some(format_advice) = format_advice {
                if format_advice.failed {
                    builder.push(
                        CLANG_FORMAT,
                        CLANG_FORMAT,
                        "clang-format exited with an error for this file, so its formatting was not checked. See the job's log for details.".to_string(),
                        Severity::Major,
                        &path,
                        Lines { begin: 1, end: 1 },
                        &SourceLines(vec![]),
                    );
                }
                for range in &format_advice.replacements {
                    let begin = (*range.start()).max(1);
                    let end = (*range.end()).max(begin);
                    builder.push(
                        CLANG_FORMAT,
                        CLANG_FORMAT,
                        format!(
                            "Code does not match {style_description}. Run clang-format to fix it."
                        ),
                        Severity::Minor,
                        &path,
                        Lines { begin, end },
                        &source,
                    );
                }
            }

            for note in tidy_notes {
                // GitLab line numbers start at 1
                let line = note.line.max(1);
                builder.push(
                    CLANG_TIDY,
                    &note.diagnostic,
                    format!("{} [{}]", note.rationale, note.diagnostic),
                    Severity::from_tidy(&note.severity),
                    &path,
                    Lines {
                        begin: line,
                        end: line,
                    },
                    &source,
                );
            }
        }
        Ok(Self {
            issues: builder.issues,
        })
    }

    /// Is the report empty (no findings)?
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }

    /// The number of findings in the report.
    pub fn len(&self) -> usize {
        self.issues.len()
    }

    /// Write the report (as JSON) to the given `output_file` path.
    ///
    /// Any missing parent directories are created.
    /// If `output_file` is [`STDOUT_PATH`](crate::common_fs::STDOUT_PATH) (`-`),
    /// the report is written to stdout.
    /// The report is written even if there are no findings (an empty array),
    /// and it never starts with a byte order mark (GitLab rejects one).
    pub fn write(&self, output_file: &Path) -> Result<(), ClientError> {
        let json = serde_json::to_string_pretty(self).map_err(ClientError::CodeQualitySerialize)?;
        let written = if is_stdout(output_file) {
            let mut out = std::io::stdout().lock();
            writeln!(out, "{json}").and_then(|()| out.flush())
        } else {
            if let Some(parent) = output_file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ClientError::MkDirFailed {
                    file_path: output_file.to_path_buf(),
                    source: e,
                })?;
            }
            std::fs::write(output_file, json)
        };
        written.map_err(|e| ClientError::CodeQualityFileWriteFailed {
            file_path: PathBuf::from(output_file),
            source: e,
        })
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::unwrap_used)]

    use std::{
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    use serde_json::Value;

    use super::{CodeQualityReport, SourceLines, fingerprint, relative_path};
    use crate::{
        clang_tools::{
            clang_format::FormatAdvice,
            clang_tidy::{TidyAdvice, TidyNotification},
        },
        common_fs::FileObj,
    };

    fn note(filename: &str, line: u32, severity: &str, diagnostic: &str) -> TidyNotification {
        TidyNotification {
            filename: filename.to_string(),
            line,
            cols: 5,
            severity: severity.to_string(),
            rationale: format!("rationale for {diagnostic}"),
            diagnostic: diagnostic.to_string(),
            suggestion: vec![],
            fixed_lines: vec![],
            skipped_fixes: vec![],
        }
    }

    const SOURCE: &str = "int main() {\n  int x = 1;\n  int x = 1;\n  return 0;\n}\n";

    /// Make the files in `repo_root`, and the [`FileObj`]s about them.
    fn make_files(repo_root: &Path) -> Vec<Arc<Mutex<FileObj>>> {
        std::fs::create_dir_all(repo_root.join("src")).unwrap();
        std::fs::write(repo_root.join("src/some source.cpp"), SOURCE).unwrap();
        std::fs::write(repo_root.join("src/other.c"), SOURCE).unwrap();

        let mut src = FileObj::new(PathBuf::from("src/some source.cpp"));
        src.format_advice = Some(FormatAdvice {
            replacements: vec![2..=3, 0..=0],
            failed: false,
        });
        src.tidy_advice = Some(TidyAdvice {
            notes: vec![
                // the same finding on two lines with the same text
                note("src/some source.cpp", 2, "warning", "readability-x"),
                note("src/some source.cpp", 3, "warning", "readability-x"),
                note("src/some source.cpp", 4, "error", "clang-diagnostic-error"),
                // a note about an included header is not about the analyzed file
                note("src/demo.hpp", 1, "warning", "readability-x"),
                note(
                    "src/some source.cpp",
                    0,
                    "remark",
                    "clang-diagnostic-unused",
                ),
            ],
        });
        let mut other = FileObj::new(PathBuf::from("./src/other.c"));
        other.format_advice = Some(FormatAdvice {
            replacements: vec![],
            failed: true,
        });
        // a file with only clang-tidy findings
        let mut tidy_only = FileObj::new(PathBuf::from("src/other.c"));
        tidy_only.tidy_advice = Some(TidyAdvice {
            notes: vec![note("src/other.c", 4, "warning", "readability-y")],
        });
        // a file without findings
        let mut clean = FileObj::new(PathBuf::from("src/clean.cpp"));
        clean.format_advice = Some(FormatAdvice::default());
        clean.tidy_advice = Some(TidyAdvice { notes: vec![] });
        // a file that the clang tools did not analyze
        let skipped = FileObj::new(PathBuf::from("src/skipped.cpp"));
        vec![
            Arc::new(Mutex::new(src)),
            Arc::new(Mutex::new(other)),
            Arc::new(Mutex::new(tidy_only)),
            Arc::new(Mutex::new(clean)),
            Arc::new(Mutex::new(skipped)),
        ]
    }

    fn to_json(report: &CodeQualityReport) -> Value {
        serde_json::from_str(&serde_json::to_string(report).unwrap()).unwrap()
    }

    #[test]
    fn paths() {
        let root = Path::new("/repo");
        let none = Path::new("");
        assert_eq!(relative_path(Path::new("./src/a.c"), root, none), "src/a.c");
        assert_eq!(
            relative_path(Path::new("/repo/src/a.c"), root, none),
            "src/a.c"
        );
        assert_eq!(relative_path(Path::new(r"src\a.c"), root, none), "src/a.c");
        // `--repo-root` is a sub-directory of the git repository
        let prefix = Path::new("native");
        assert_eq!(
            relative_path(Path::new("./src/a.c"), root, prefix),
            "native/src/a.c"
        );
        assert_eq!(
            relative_path(Path::new("/repo/src/a.c"), root, prefix),
            "native/src/a.c"
        );
    }

    #[test]
    fn fingerprints() {
        let fp = fingerprint("clang-tidy", "check", "a.c", "int x;", 0);
        assert_eq!(fp.len(), 32);
        assert!(fp.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        // stable across runs, platforms and versions (a changed fingerprint
        // would show every finding as new in GitLab)
        assert_eq!(fp, "96bec3ae0b8117bbfb74ceaa957cbedc");
        assert_ne!(fp, fingerprint("clang-tidy", "check", "a.c", "int x;", 1));
        assert_ne!(fp, fingerprint("clang-tidy", "check", "b.c", "int x;", 0));
        // parts are separated
        assert_ne!(
            fingerprint("ab", "c", "", "", 0),
            fingerprint("a", "bc", "", "", 0)
        );
    }

    #[test]
    fn source_lines() {
        let source = SourceLines(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(source.text(1, 2), "a\nb");
        // out of range lines are ignored
        assert_eq!(source.text(0, 0), "");
        assert_eq!(source.text(2, 5), "b");
        let missing = SourceLines::read(Path::new("does/not/exist.cpp"));
        assert!(missing.0.is_empty());
    }

    #[test]
    fn code_quality_report() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let files = make_files(tmp_dir.path());
        let report = CodeQualityReport::new(&files, "llvm", tmp_dir.path(), Path::new("")).unwrap();
        assert_eq!(report.len(), 8);
        assert!(!report.is_empty());
        let json = to_json(&report);
        let issues = json.as_array().unwrap();
        assert_eq!(issues.len(), 8);
        for issue in issues {
            // fields required by GitLab
            for field in ["description", "check_name", "fingerprint", "severity"] {
                assert!(issue[field].is_string(), "issue is missing {field}");
            }
            assert!(issue["location"]["path"].is_string());
            assert!(issue["location"]["lines"]["begin"].as_u64().unwrap() >= 1);
            let path = issue["location"]["path"].as_str().unwrap();
            assert!(!path.starts_with("./"));
            assert_ne!(path, "src/demo.hpp");
        }

        // clang-format ranges come first
        assert_eq!(issues[0]["check_name"], "clang-format");
        assert_eq!(issues[0]["severity"], "minor");
        assert_eq!(issues[0]["location"]["path"], "src/some source.cpp");
        assert_eq!(issues[0]["location"]["lines"]["begin"], 2);
        assert_eq!(issues[0]["location"]["lines"]["end"], 3);
        assert_eq!(
            issues[0]["description"],
            "Code does not match the LLVM style. Run clang-format to fix it."
        );
        // line numbers start at 1
        assert_eq!(issues[1]["location"]["lines"]["begin"], 1);
        assert_eq!(issues[1]["location"]["lines"]["end"], 1);

        // clang-tidy notes
        assert_eq!(issues[2]["check_name"], "readability-x");
        assert_eq!(issues[2]["severity"], "major");
        assert_eq!(
            issues[2]["description"],
            "rationale for readability-x [readability-x]"
        );
        assert_eq!(issues[2]["location"]["lines"]["begin"], 2);
        assert_eq!(issues[2]["location"]["lines"]["end"], 2);
        // the same finding on a line with the same text gets its own fingerprint
        assert_eq!(issues[3]["location"]["lines"]["begin"], 3);
        assert_ne!(issues[2]["fingerprint"], issues[3]["fingerprint"]);
        assert_eq!(issues[4]["severity"], "critical");
        assert_eq!(issues[5]["severity"], "info");
        assert_eq!(issues[5]["location"]["lines"]["begin"], 1);

        // clang-format failed on a file
        assert_eq!(issues[6]["check_name"], "clang-format");
        assert_eq!(issues[6]["severity"], "major");
        assert_eq!(issues[6]["location"]["path"], "src/other.c");

        // a file with only clang-tidy findings
        assert_eq!(issues[7]["check_name"], "readability-y");
        assert_eq!(issues[7]["location"]["path"], "src/other.c");

        // all fingerprints are unique
        let mut fingerprints = issues
            .iter()
            .map(|i| i["fingerprint"].as_str().unwrap())
            .collect::<Vec<_>>();
        fingerprints.sort_unstable();
        fingerprints.dedup();
        assert_eq!(fingerprints.len(), issues.len());
    }

    #[test]
    fn fingerprint_ignores_line_numbers() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let files = make_files(tmp_dir.path());
        let before = to_json(
            &CodeQualityReport::new(&files, "file", tmp_dir.path(), Path::new("")).unwrap(),
        );

        // add a line above the findings, and move them down by one line
        std::fs::write(
            tmp_dir.path().join("src/some source.cpp"),
            format!("// new line\n{SOURCE}"),
        )
        .unwrap();
        {
            let mut src = files[0].lock().unwrap();
            for note in &mut src.tidy_advice.as_mut().unwrap().notes {
                if note.line > 0 {
                    note.line += 1;
                }
            }
        }
        let after = to_json(
            &CodeQualityReport::new(&files, "file", tmp_dir.path(), Path::new("")).unwrap(),
        );
        for index in [2, 3, 4] {
            assert_ne!(
                before[index]["location"]["lines"]["begin"],
                after[index]["location"]["lines"]["begin"]
            );
            assert_eq!(before[index]["fingerprint"], after[index]["fingerprint"]);
        }
        assert_eq!(
            after[0]["description"],
            "Code does not match the style in .clang-format. Run clang-format to fix it."
        );
    }

    #[test]
    fn empty_report() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let report = CodeQualityReport::new(&[], "llvm", tmp_dir.path(), Path::new("")).unwrap();
        assert!(report.is_empty());
        let out_path = tmp_dir.path().join("gl-code-quality.json");
        report.write(&out_path).unwrap();
        // an empty array, so the artifact always exists
        assert_eq!(std::fs::read_to_string(&out_path).unwrap(), "[]");
    }

    #[test]
    fn write_report() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let out_path = tmp_dir.path().join("sub/dir/gl-code-quality.json");
        let report = CodeQualityReport::new(
            &make_files(tmp_dir.path()),
            "llvm",
            tmp_dir.path(),
            Path::new(""),
        )
        .unwrap();
        report.write(&out_path).unwrap();
        let bytes = std::fs::read(&out_path).unwrap();
        // GitLab rejects a byte order mark
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let written: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(written, to_json(&report));
    }

    #[test]
    fn write_report_to_stdout() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let report = CodeQualityReport::new(&[], "llvm", tmp_dir.path(), Path::new("")).unwrap();
        // start without a file named `-` in the working directory
        let _ = std::fs::remove_file("-");
        report
            .write(Path::new(crate::common_fs::STDOUT_PATH))
            .unwrap();
        // no file named `-` is created
        assert!(!Path::new("-").exists());
    }

    #[test]
    fn write_report_errors() {
        use crate::error::ClientError;

        let tmp_dir = tempfile::tempdir().unwrap();
        let report = CodeQualityReport::new(&[], "llvm", tmp_dir.path(), Path::new("")).unwrap();

        // the parent "directory" is actually a file
        let not_a_dir = tmp_dir.path().join("not-a-dir");
        std::fs::write(&not_a_dir, "").unwrap();
        let err = report.write(&not_a_dir.join("report.json")).unwrap_err();
        assert!(matches!(err, ClientError::MkDirFailed { .. }));

        // the output "file" is actually a directory
        let err = report.write(tmp_dir.path()).unwrap_err();
        assert!(matches!(
            err,
            ClientError::CodeQualityFileWriteFailed { .. }
        ));

        // the output path has no parent (and is not a writable file)
        #[cfg(unix)]
        {
            let err = report.write(Path::new("/")).unwrap_err();
            assert!(matches!(
                err,
                ClientError::CodeQualityFileWriteFailed { .. }
            ));
        }
    }

    #[test]
    fn poisoned_mutex() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let files = make_files(tmp_dir.path());
        crate::test_common::poison_mutex(&files[0]);
        assert!(CodeQualityReport::new(&files, "llvm", tmp_dir.path(), Path::new("")).is_err());
    }
}
