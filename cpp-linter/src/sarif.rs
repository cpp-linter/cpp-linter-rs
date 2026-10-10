//! This module holds functionality to export clang-tidy diagnostics as a
//! [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html) log.
//!
//! Only the subset of the SARIF specification that is needed to describe
//! clang-tidy diagnostics is implemented here.

use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::{Arc, Mutex},
};

// non-std crates
use semver::Version;
use serde::Serialize;

// project-specific modules/crates
use crate::{clang_tools::clang_tidy::TidyNotification, common_fs::FileObj, error::ClientError};

/// The URI of the JSON schema for SARIF 2.1.0.
const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The version of the SARIF specification that is implemented.
const SARIF_VERSION: &str = "2.1.0";

/// The symbolic name used for the repository root in artifact locations.
const SRC_ROOT: &str = "%SRCROOT%";

/// The URL to clang-tidy's documentation.
const CLANG_TIDY_URL: &str = "https://clang.llvm.org/extra/clang-tidy/";

/// The URL to clang's reference of compiler diagnostics (warning flags).
const CLANG_DIAGNOSTICS_URL: &str = "https://clang.llvm.org/docs/DiagnosticsReference.html";

/// A SARIF log that describes the diagnostics reported by clang-tidy.
///
/// Use [`SarifLog::new()`] to create one, and [`SarifLog::write()`] to save it.
#[derive(Debug, Serialize)]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<Run>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    tool: Tool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    original_uri_base_ids: BTreeMap<&'static str, ArtifactLocation>,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
struct Tool {
    driver: ToolComponent,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolComponent {
    name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic_version: Option<String>,
    information_uri: &'static str,
    rules: Vec<ReportingDescriptor>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportingDescriptor {
    id: String,
    name: String,
    short_description: Message,
    full_description: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    help_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<MultiformatMessage>,
    default_configuration: ReportingConfiguration,
}

#[derive(Debug, Serialize)]
struct ReportingConfiguration {
    level: Level,
}

#[derive(Debug, Serialize)]
struct Message {
    text: String,
}

#[derive(Debug, Serialize)]
struct MultiformatMessage {
    text: String,
    markdown: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    rule_index: usize,
    level: Level,
    message: Message,
    locations: Vec<Location>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    physical_location: PhysicalLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalLocation {
    artifact_location: ArtifactLocation,
    region: Region,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactLocation {
    uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    uri_base_id: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Error,
    Warning,
    Note,
}

impl From<&str> for Level {
    fn from(severity: &str) -> Self {
        match severity {
            "error" => Level::Error,
            "warning" => Level::Warning,
            // clang-tidy may also report "note" or "remark" severities
            _ => Level::Note,
        }
    }
}

/// Percent-encode a path so it can be used as (part of) a URI.
///
/// All characters except the unreserved characters (RFC 3986) and path
/// separators (`/`) are encoded. If `keep_colon` is true, then `:` is also kept
/// as is (used for Windows drive letters in `file:` URIs).
fn encode_uri_path(path: &str, keep_colon: bool) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/')
            || (keep_colon && byte == b':')
        {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// Convert an absolute path of the repository root into a `file:` URI that ends with a `/`.
fn repo_root_uri(repo_root: &Path) -> String {
    let mut path = repo_root.to_string_lossy().replace('\\', "/");
    if !path.starts_with('/') {
        // Windows paths start with a drive letter (eg `C:/`)
        path.insert(0, '/');
    }
    if !path.ends_with('/') {
        path.push('/');
    }
    format!("file://{}", encode_uri_path(&path, true))
}

/// Get a URL to documentation about the given clang-tidy notification's diagnostic.
fn help_uri(note: &TidyNotification) -> Option<String> {
    if let Some(flag) = note.diagnostic.strip_prefix("clang-diagnostic-") {
        // compiler errors (`clang-diagnostic-error`) are not warning flags
        return if flag == "error" {
            None
        } else {
            Some(format!("{CLANG_DIAGNOSTICS_URL}#w{flag}"))
        };
    }
    note.diagnostic_url()
}

/// Create a SARIF rule (reporting descriptor) for the given clang-tidy notification's diagnostic.
fn make_rule(note: &TidyNotification) -> ReportingDescriptor {
    let id = note.diagnostic.clone();
    let help_uri = help_uri(note);
    let description = if note.diagnostic.starts_with("clang-diagnostic-") {
        format!("Clang compiler diagnostic {id}")
    } else {
        format!("clang-tidy check {id}")
    };
    let help = help_uri.as_ref().map(|url| MultiformatMessage {
        text: format!("{description}. See {url} for details."),
        markdown: format!("{description}. See the [documentation]({url}) for details."),
    });
    ReportingDescriptor {
        name: id.clone(),
        id,
        short_description: Message {
            text: description.clone(),
        },
        full_description: Message {
            text: match &help_uri {
                Some(url) => format!("{description}. See {url} for details."),
                None => format!("{description}."),
            },
        },
        help_uri,
        help,
        default_configuration: ReportingConfiguration {
            level: Level::from(note.severity.as_str()),
        },
    }
}

impl SarifLog {
    /// Create a SARIF log from the clang-tidy advice about the given `files`.
    ///
    /// Like other forms of feedback, only the diagnostics about the given `files` are
    /// included. So, any filtering (eg. `--lines-changed-only`, `--ignore-tidy`) is
    /// respected.
    ///
    /// All file paths are relative to the given `repo_root` path.
    pub fn new(
        files: &[Arc<Mutex<FileObj>>],
        tidy_version: Option<&Version>,
        repo_root: &Path,
    ) -> Result<Self, ClientError> {
        let mut rules = vec![];
        let mut rule_indices: HashMap<String, usize> = HashMap::new();
        let mut results = vec![];
        for file in files {
            let file = file
                .lock()
                .map_err(|e| ClientError::MutexPoisoned(e.to_string()))?;
            let Some(tidy_advice) = &file.tidy_advice else {
                continue;
            };
            for note in &tidy_advice.notes {
                if Path::new(&note.filename) != file.name.as_path() {
                    // skip notes about other files (eg. included headers)
                    continue;
                }
                let rule_index =
                    *rule_indices
                        .entry(note.diagnostic.clone())
                        .or_insert_with(|| {
                            rules.push(make_rule(note));
                            rules.len() - 1
                        });
                results.push(SarifResult {
                    rule_id: note.diagnostic.clone(),
                    rule_index,
                    level: Level::from(note.severity.as_str()),
                    message: Message {
                        text: note.rationale.clone(),
                    },
                    locations: vec![Location {
                        physical_location: PhysicalLocation {
                            artifact_location: ArtifactLocation {
                                uri: encode_uri_path(&note.filename, false),
                                uri_base_id: Some(SRC_ROOT),
                            },
                            region: Region {
                                // SARIF line and column numbers start at 1
                                start_line: note.line.max(1),
                                start_column: if note.cols > 0 { Some(note.cols) } else { None },
                            },
                        },
                    }],
                });
            }
        }

        let mut original_uri_base_ids = BTreeMap::new();
        if repo_root.is_absolute() {
            original_uri_base_ids.insert(
                SRC_ROOT,
                ArtifactLocation {
                    uri: repo_root_uri(repo_root),
                    uri_base_id: None,
                },
            );
        }

        Ok(Self {
            schema: SARIF_SCHEMA,
            version: SARIF_VERSION,
            runs: vec![Run {
                tool: Tool {
                    driver: ToolComponent {
                        name: "clang-tidy",
                        version: tidy_version.map(|v| v.to_string()),
                        semantic_version: tidy_version.map(|v| v.to_string()),
                        information_uri: CLANG_TIDY_URL,
                        rules,
                    },
                },
                original_uri_base_ids,
                results,
            }],
        })
    }

    /// Write the SARIF log (as JSON) to the given `output_file` path.
    ///
    /// Any missing parent directories are created.
    pub fn write(&self, output_file: &Path) -> Result<(), ClientError> {
        if let Some(parent) = output_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ClientError::MkDirFailed {
                file_path: output_file.to_path_buf(),
                source: e,
            })?;
        }
        let json = serde_json::to_string_pretty(self).map_err(ClientError::SarifSerialize)?;
        std::fs::write(output_file, json).map_err(|e| ClientError::SarifFileWriteFailed {
            file_path: output_file.to_path_buf(),
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

    use semver::Version;
    use serde_json::Value;

    use super::{SarifLog, encode_uri_path, repo_root_uri};
    use crate::{
        clang_tools::clang_tidy::{TidyAdvice, TidyNotification},
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
        }
    }

    fn make_files() -> Vec<Arc<Mutex<FileObj>>> {
        let mut src = FileObj::new(PathBuf::from("src/some source.cpp"));
        src.tidy_advice = Some(TidyAdvice {
            notes: vec![
                note(
                    "src/some source.cpp",
                    3,
                    "warning",
                    "modernize-use-trailing-return-type",
                ),
                note("src/some source.cpp", 7, "error", "clang-diagnostic-error"),
                // a note about an included header is not about the analyzed file
                note(
                    "src/demo.hpp",
                    1,
                    "warning",
                    "readability-identifier-length",
                ),
                note(
                    "src/some source.cpp",
                    9,
                    "warning",
                    "modernize-use-trailing-return-type",
                ),
            ],
        });
        let mut other = FileObj::new(PathBuf::from("src/other.c"));
        other.tidy_advice = Some(TidyAdvice {
            notes: vec![note(
                "src/other.c",
                0,
                "remark",
                "clang-diagnostic-unused-variable",
            )],
        });
        // a file that clang-tidy did not analyze
        let skipped = FileObj::new(PathBuf::from("src/skipped.cpp"));
        vec![
            Arc::new(Mutex::new(src)),
            Arc::new(Mutex::new(other)),
            Arc::new(Mutex::new(skipped)),
        ]
    }

    fn to_json(log: &SarifLog) -> Value {
        serde_json::from_str(&serde_json::to_string(log).unwrap()).unwrap()
    }

    #[test]
    fn uri_encoding() {
        assert_eq!(
            encode_uri_path("src/some source#1.cpp", false),
            "src/some%20source%231.cpp"
        );
        assert_eq!(encode_uri_path("C:/a b", false), "C%3A/a%20b");
        assert_eq!(encode_uri_path("C:/a b", true), "C:/a%20b");
        assert_eq!(
            repo_root_uri(Path::new("/home/user/my repo")),
            "file:///home/user/my%20repo/"
        );
        assert_eq!(
            repo_root_uri(Path::new(r"C:\Users\me\repo")),
            "file:///C:/Users/me/repo/"
        );
    }

    #[test]
    fn sarif_log() {
        let files = make_files();
        let version = Version::new(18, 1, 3);
        let repo_root = std::env::current_dir().unwrap();
        let log = to_json(&SarifLog::new(&files, Some(&version), &repo_root).unwrap());

        assert_eq!(log["version"], "2.1.0");
        assert_eq!(
            log["$schema"],
            "https://json.schemastore.org/sarif-2.1.0.json"
        );
        let runs = log["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(
            run["originalUriBaseIds"]["%SRCROOT%"]["uri"],
            repo_root_uri(&repo_root)
        );

        let driver = &run["tool"]["driver"];
        assert_eq!(driver["name"], "clang-tidy");
        assert_eq!(driver["version"], "18.1.3");
        let rules = driver["rules"].as_array().unwrap();
        let rule_ids = rules
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            rule_ids,
            [
                "modernize-use-trailing-return-type",
                "clang-diagnostic-error",
                "clang-diagnostic-unused-variable"
            ]
        );
        for rule in rules {
            // fields required by some consumers (eg. SonarQube)
            for field in ["id", "name", "shortDescription", "fullDescription"] {
                assert!(rule.get(field).is_some(), "rule is missing {field}");
            }
            assert!(rule["defaultConfiguration"]["level"].is_string());
        }
        assert_eq!(
            rules[0]["helpUri"],
            "https://clang.llvm.org/extra/clang-tidy/checks/modernize/use-trailing-return-type.html"
        );
        assert!(rules[1].get("helpUri").is_none());
        assert_eq!(
            rules[2]["helpUri"],
            "https://clang.llvm.org/docs/DiagnosticsReference.html#wunused-variable"
        );

        let results = run["results"].as_array().unwrap();
        // the note about the header (not an analyzed file) is excluded
        assert_eq!(results.len(), 4);
        let first = &results[0];
        assert_eq!(first["ruleId"], "modernize-use-trailing-return-type");
        assert_eq!(first["ruleIndex"], 0);
        assert_eq!(first["level"], "warning");
        assert_eq!(
            first["message"]["text"],
            "rationale for modernize-use-trailing-return-type"
        );
        let location = &first["locations"][0]["physicalLocation"];
        assert_eq!(location["artifactLocation"]["uri"], "src/some%20source.cpp");
        assert_eq!(location["artifactLocation"]["uriBaseId"], "%SRCROOT%");
        assert_eq!(location["region"]["startLine"], 3);
        assert_eq!(location["region"]["startColumn"], 5);

        assert_eq!(results[1]["level"], "error");
        assert_eq!(results[1]["ruleIndex"], 1);
        // repeated diagnostics reuse the same rule
        assert_eq!(results[2]["ruleIndex"], 0);
        assert_eq!(
            results[2]["locations"][0]["physicalLocation"]["region"]["startLine"],
            9
        );
        // unknown severities are reported as notes, and line numbers start at 1
        assert_eq!(results[3]["level"], "note");
        assert_eq!(
            results[3]["locations"][0]["physicalLocation"]["region"]["startLine"],
            1
        );
    }

    #[test]
    fn empty_sarif_log() {
        let log = to_json(&SarifLog::new(&[], None, Path::new("relative/path")).unwrap());
        let run = &log["runs"][0];
        assert!(run["results"].as_array().unwrap().is_empty());
        assert!(
            run["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(run["tool"]["driver"].get("version").is_none());
        assert!(run.get("originalUriBaseIds").is_none());
    }

    #[test]
    fn write_sarif_file() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let out_path = tmp_dir.path().join("sub/dir/clang-tidy.sarif");
        let log = SarifLog::new(&make_files(), None, tmp_dir.path()).unwrap();
        log.write(&out_path).unwrap();
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&out_path).unwrap()).unwrap();
        assert_eq!(written, to_json(&log));
    }
}
