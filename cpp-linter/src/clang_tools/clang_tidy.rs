//! This module holds functionality specific to running clang-tidy and parsing it's
//! output.

use std::{
    env::consts::OS,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

// non-std crates
use clang_tools_manager::utils::normalize_path;
use regex::Regex;
use serde::Deserialize;

// project-specific modules/crates
use crate::{cli::ClangParams, common_fs::FileObj, error::ClangCaptureError};

/// Used to deserialize a json compilation database's translation unit.
///
/// The only purpose this serves is to normalize relative paths for build systems that
/// use/need relative paths (ie ninja).
#[derive(Deserialize, Debug, Clone)]
pub struct CompilationUnit {
    /// The directory of the build environment
    directory: String,

    /// The file path of the translation unit.
    ///
    /// Sometimes, this is relative to the build [`CompilationUnit::directory`].
    ///
    /// This is typically the path that clang-tidy uses in its stdout (for a dry run).
    /// So, having this information helps with matching clang-tidy's stdout with the
    /// repository files.
    file: String,
}

/// A structure that represents a single notification parsed from clang-tidy's stdout.
#[derive(Debug, Clone)]
pub struct TidyNotification {
    /// The file's path and name (supposedly relative to the repository root folder).
    pub filename: String,

    /// The line number from which the notification originated.
    pub line: u32,

    /// The column offset on the line from which the notification originated.
    pub cols: u32,

    /// The severity (ie error/warning/note) of the [`TidyNotification::diagnostic`]
    /// that caused the notification.
    pub severity: String,

    /// A helpful message explaining why the notification exists.
    pub rationale: String,

    /// The diagnostic name as used when configuring clang-tidy.
    pub diagnostic: String,

    /// A code block that points directly to the origin of the notification.
    ///
    /// Sometimes, this code block doesn't exist. Sometimes, it contains suggested
    /// fixes/advice. This information is purely superfluous.
    pub suggestion: Vec<String>,

    /// The list of line numbers (in the analyzed file) that clang-tidy suggested fixes for.
    pub fixed_lines: Vec<u32>,
}

/// The URL to clang-tidy's documentation.
pub const CLANG_TIDY_URL: &str = "https://clang.llvm.org/extra/clang-tidy/";

/// The URL to clang's reference of compiler diagnostics (warning flags).
pub const CLANG_DIAGNOSTICS_URL: &str = "https://clang.llvm.org/docs/DiagnosticsReference.html";

impl TidyNotification {
    /// Get the URL of the documentation page for [`Self::diagnostic`].
    ///
    /// For clang-tidy checks, this is the check's page in [clang-tidy's documentation](CLANG_TIDY_URL).
    /// For compiler diagnostics (`clang-diagnostic-*`), this is the warning flag's entry in
    /// [clang's diagnostics reference](CLANG_DIAGNOSTICS_URL).
    ///
    /// Returns [`None`] for compiler errors (`clang-diagnostic-error`) because they are not
    /// associated with a warning flag.
    pub fn diagnostic_url(&self) -> Option<String> {
        if let Some(flag) = self.diagnostic.strip_prefix("clang-diagnostic-") {
            return if flag == "error" || flag.is_empty() {
                None
            } else {
                Some(format!("{CLANG_DIAGNOSTICS_URL}#w{flag}"))
            };
        }
        let (category, name) = if self.diagnostic.starts_with("clang-analyzer-") {
            self.diagnostic
                .strip_prefix("clang-analyzer-")
                .map(|n| ("clang-analyzer", n))
        } else {
            self.diagnostic.split_once('-')
        }?;
        // In production, both category and name should be non-empty strings.
        // Clang does not actually have a diagnostic name whose category or name is empty.
        debug_assert!(!category.is_empty() && !name.is_empty());
        Some(format!("{CLANG_TIDY_URL}checks/{category}/{name}.html"))
    }

    /// Get a markdown-formatted link to the documentation page for [`Self::diagnostic`].
    pub fn diagnostic_link(&self) -> String {
        match self.diagnostic_url() {
            Some(url) => format!("[{}]({url})", self.diagnostic),
            None => self.diagnostic.clone(),
        }
    }
}

/// A struct to hold notification from clang-tidy about a single file
#[derive(Debug, Clone)]
pub struct TidyAdvice {
    /// A list of notifications parsed from clang-tidy stdout.
    pub notes: Vec<TidyNotification>,
}

impl TidyAdvice {
    pub(crate) fn get_suggestion_help(&self, start_line: u32, end_line: u32) -> String {
        let mut diagnostics = vec![];
        for note in &self.notes {
            for fixed_line in &note.fixed_lines {
                if (start_line..=end_line).contains(fixed_line) {
                    diagnostics.push(format!(
                        "- {} [{}]\n",
                        note.rationale,
                        note.diagnostic_link()
                    ));
                }
            }
        }
        format!(
            "### clang-tidy {}\n{}",
            if diagnostics.is_empty() {
                "suggestion"
            } else {
                "diagnostic(s)"
            },
            diagnostics.join("")
        )
    }
}

/// A regex pattern to capture the clang-tidy note header.
const NOTE_HEADER: &str = r"^(.+):(\d+):(\d+):\s(\w+):(.*)\[([a-zA-Z\d\-\.]+),?[^\]]*\]$";

/// Parses clang-tidy stdout.
///
/// Here it helps to have the JSON database deserialized for normalizing paths present
/// in the notifications.
fn parse_tidy_output(
    tidy_stdout: &[u8],
    database_json: &Option<Vec<CompilationUnit>>,
    repo_root: &Path,
) -> Result<Vec<TidyNotification>, ClangCaptureError> {
    let note_header = Regex::new(NOTE_HEADER)?;
    let fixed_note = Regex::new(r"^.+:(\d+):\d+:\snote: FIX-IT applied suggested code changes$")?;
    let mut found_fix = false;
    let mut notification = None;
    let mut result = Vec::new();
    for line in String::from_utf8(tidy_stdout.to_vec())
        .map_err(|e| ClangCaptureError::NonUtf8Output {
            task: "convert clang-tidy stdout".to_string(),
            source: e,
        })?
        .lines()
    {
        if let Some(captured) = note_header.captures(line) {
            // First check that the diagnostic name is a actual diagnostic name.
            // Sometimes clang-tidy uses square brackets to enclose additional context
            // about the diagnostic rationale. For example: '[with auto = typename ...]'
            // We need to ignore such cases as they do not start a diagnostic report.
            if captured
                .get(6)
                .is_some_and(|diag| !diag.as_str().contains(' ') && diag.as_str().contains('-'))
            {
                // starting a new TidyNotification; store the previous one (if any) to results
                if let Some(note) = notification {
                    result.push(note);
                }

                // normalize the filename path and try to make it relative to the repo root
                let mut filename = PathBuf::from(&captured[1]);
                // if database was given try to use that first
                if let Some(db_json) = &database_json {
                    let mut found_unit = false;
                    for unit in db_json {
                        let unit_path =
                            PathBuf::from_iter([unit.directory.as_str(), unit.file.as_str()]);
                        if unit_path == filename {
                            filename =
                                normalize_path(&PathBuf::from_iter([&unit.directory, &unit.file]));
                            found_unit = true;
                            break;
                        }
                    }
                    if !found_unit {
                        // file was not a named unit in the database;
                        // try to normalize path as if relative to working directory.
                        // NOTE: This shouldn't happen with a properly formed JSON database
                        filename = normalize_path(&PathBuf::from_iter([repo_root, &filename]));
                    }
                } else {
                    // still need to normalize the relative path despite missing database info.
                    // let's assume the file is relative to current working directory.
                    filename = normalize_path(&PathBuf::from_iter([repo_root, &filename]));
                }

                if filename.is_absolute()
                    && let Ok(file_n) = filename.strip_prefix(repo_root)
                {
                    // if this filename can't be made into a relative path, then it is
                    // likely not a member of the project's sources (ie /usr/include/stdio.h)
                    filename = file_n.to_path_buf();
                }

                notification = Some(TidyNotification {
                    filename: filename.to_string_lossy().to_string().replace('\\', "/"),
                    line: captured[2].parse()?,
                    cols: captured[3].parse()?,
                    severity: String::from(&captured[4]),
                    rationale: String::from(&captured[5]).trim().to_string(),
                    diagnostic: String::from(&captured[6]),
                    suggestion: Vec::new(),
                    fixed_lines: Vec::new(),
                });
                // begin capturing subsequent lines as suggestions
                found_fix = false;
            }
        } else if let Some(capture) = fixed_note.captures(line) {
            let fixed_line = capture[1].parse()?;
            if let Some(note) = &mut notification
                && !note.fixed_lines.contains(&fixed_line)
            {
                note.fixed_lines.push(fixed_line);
            }
            // Suspend capturing subsequent lines as suggestions until
            // a new notification is constructed. If we found a note about applied fixes,
            // then the lines of suggestions for that notification have already been parsed.
            found_fix = true;
        } else if !found_fix && let Some(note) = &mut notification {
            // append lines of code that are part of
            // the previous line's notification
            note.suggestion.push(line.to_string());
        }
    }
    if let Some(note) = notification {
        result.push(note);
    }
    Ok(result)
}

/// Get a total count of clang-tidy advice from the given list of [FileObj]s.
pub fn tally_tidy_advice(files: &[Arc<Mutex<FileObj>>]) -> Result<u64, String> {
    let mut total = 0;
    for file in files {
        let file = file.lock().map_err(|e| e.to_string())?;
        if let Some(advice) = &file.tidy_advice {
            for tidy_note in &advice.notes {
                let file_path = PathBuf::from(&tidy_note.filename);
                if file_path == file.name {
                    total += 1;
                }
            }
        }
    }
    Ok(total)
}

/// The YAML document that clang-tidy writes with `--export-fixes`.
///
/// Only the fields needed to apply fixes are deserialized.
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
struct ExportedFixes {
    #[serde(default)]
    diagnostics: Vec<ExportedDiagnostic>,
}

/// A single diagnostic in [`ExportedFixes`].
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ExportedDiagnostic {
    diagnostic_name: String,
    diagnostic_message: ExportedMessage,
    #[serde(default)]
    build_directory: String,
}

/// The location and fixes of an [`ExportedDiagnostic`].
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "PascalCase")]
struct ExportedMessage {
    #[serde(default)]
    file_path: String,
    #[serde(default)]
    file_offset: usize,
    #[serde(default)]
    replacements: Vec<ExportedReplacement>,
}

/// A single text replacement in an [`ExportedMessage`].
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
struct ExportedReplacement {
    file_path: String,
    offset: usize,
    length: usize,
    #[serde(default)]
    replacement_text: String,
}

/// Is `path` (as reported by clang-tidy) the same file as `target`?
///
/// `target` is expected to be absolute and normalized.
/// Relative paths are resolved from the diagnostic's `build_dir`.
fn is_same_file(path: &str, build_dir: &str, target: &Path) -> bool {
    if path.is_empty() {
        return false;
    }
    let path = normalize_path(&Path::new(build_dir).join(path));
    path == target
        || fs::canonicalize(&path)
            .ok()
            .zip(fs::canonicalize(target).ok())
            .is_some_and(|(a, b)| a == b)
}

/// Convert a byte `offset` in `content` into a 1-based line number.
fn offset_to_line(content: &[u8], offset: usize) -> u32 {
    let end = offset.min(content.len());
    content[..end].iter().filter(|b| **b == b'\n').count() as u32 + 1
}

/// Apply the clang-tidy `fixes` that pertain to the `target` file onto its `content`.
///
/// Fixes for any other file (eg. a header declaring a function defined in `target`)
/// are ignored, so nothing outside the analyzed file is ever changed.
///
/// Returns the patched content.
/// Each diagnostic's `fixed_lines` (in the original `content`) are recorded in the
/// matching `notes`.
fn apply_fixes(
    fixes: &ExportedFixes,
    target: &Path,
    content: &[u8],
    notes: &mut [TidyNotification],
) -> Vec<u8> {
    let mut replacements = vec![];
    for diag in &fixes.diagnostics {
        let msg = &diag.diagnostic_message;
        let mut fixed_lines = vec![];
        for replacement in &msg.replacements {
            if replacement
                .offset
                .checked_add(replacement.length)
                .is_some_and(|end| end <= content.len())
                && is_same_file(&replacement.file_path, &diag.build_directory, target)
            {
                let line = offset_to_line(content, replacement.offset);
                if !fixed_lines.contains(&line) {
                    fixed_lines.push(line);
                }
                replacements.push(replacement);
            }
        }
        if fixed_lines.is_empty() {
            continue;
        }
        // Find the notification (parsed from stdout) for this diagnostic.
        let diag_path = normalize_path(&Path::new(&diag.build_directory).join(&msg.file_path));
        let diag_line = if is_same_file(&msg.file_path, &diag.build_directory, target) {
            Some(offset_to_line(content, msg.file_offset))
        } else {
            fs::read(&diag_path)
                .ok()
                .map(|c| offset_to_line(&c, msg.file_offset))
        };
        if let Some(note) = notes.iter_mut().find(|note| {
            note.diagnostic == diag.diagnostic_name
                && Some(note.line) == diag_line
                && diag_path.ends_with(&note.filename)
        }) {
            for line in fixed_lines {
                if !note.fixed_lines.contains(&line) {
                    note.fixed_lines.push(line);
                }
            }
        }
    }

    // clang-tidy already discards conflicting fixes before exporting them,
    // but be defensive about duplicates and overlaps anyway.
    replacements.sort_by_key(|r| (r.offset, r.length));
    replacements.dedup();
    let mut patched = Vec::with_capacity(content.len());
    let mut cursor = 0;
    for replacement in replacements {
        let end = replacement.offset + replacement.length;
        if replacement.offset < cursor {
            continue;
        }
        patched.extend_from_slice(&content[cursor..replacement.offset]);
        patched.extend_from_slice(replacement.replacement_text.as_bytes());
        cursor = end;
    }
    patched.extend_from_slice(&content[cursor..]);
    patched
}

/// Run clang-tidy, then parse and return it's output.
///
/// clang-tidy is not allowed to modify any files.
/// Instead, its fixes are exported (`--export-fixes`), and only the fixes for the
/// analyzed file are applied to a copy of that file in the cache.
pub fn run_clang_tidy(
    file: &mut MutexGuard<FileObj>,
    clang_params: &ClangParams,
) -> Result<Vec<(log::Level, String)>, ClangCaptureError> {
    let cmd_path = clang_params
        .clang_tidy_command
        .as_ref()
        .ok_or(ClangCaptureError::ToolPathUnknown("clang-tidy"))?;
    let mut cmd = Command::new(cmd_path);
    cmd.current_dir(&clang_params.repo_root);
    let mut logs = vec![];
    if !clang_params.tidy_checks.is_empty() {
        cmd.args(["-checks", &clang_params.tidy_checks]);
    }
    if let Some(db) = &clang_params.database {
        cmd.args(["-p", &db.to_string_lossy()]);
    }
    for arg in &clang_params.extra_args {
        // Use `--extra-arg=...` syntax to prevent values getting
        // interpreted as a clang-tidy options.
        // At this point, values should already be split by
        // whitespace in `ClangParams::from(&cli)` via
        // `crate::cli::convert_extra_arg_val()`.
        cmd.arg(format!("--extra-arg={arg}").as_str());
    }
    let file_name = file.name.to_string_lossy().to_string();
    let ranges = file.get_ranges(&clang_params.lines_changed_only);
    if !ranges.is_empty() {
        let filter = format!(
            "[{{\"name\":{:?},\"lines\":{:?}}}]",
            file_name.replace('/', if OS == "windows" { "\\" } else { "/" }),
            ranges
                .iter()
                .map(|r| [r.start(), r.end()])
                .collect::<Vec<_>>()
        );
        cmd.args(["--line-filter", filter.as_str()]);
    }
    let repo_file_path = clang_params.repo_root.join(&file.name);
    let original_content =
        fs::read(&repo_file_path).map_err(|e| ClangCaptureError::ReadFileFailed {
            file_name: file_name.clone(),
            source: e,
        })?;
    let cache_path = clang_params.get_cache_path();
    let cache_patch_path = cache_path.join(&file.name);
    fs::create_dir_all(
        cache_patch_path
            .parent()
            .ok_or(ClangCaptureError::UnknownCacheParentPath)?,
    )
    .map_err(ClangCaptureError::MkDirFailed)?;
    // clang-tidy runs from the repo root, so give it an absolute path.
    let mut fixes_path = std::path::absolute(&cache_patch_path)
        .map_err(ClangCaptureError::UnknownWorkingDirectory)?
        .into_os_string();
    fixes_path.push(".fixes.yml");
    let fixes_path = PathBuf::from(fixes_path);
    // clang-tidy does not write this file if there is nothing to report,
    // so don't let a stale one from a previous run get used.
    let _ = fs::remove_file(&fixes_path);
    cmd.arg("--export-fixes");
    cmd.arg(&fixes_path);
    cmd.arg(file.name.to_string_lossy().as_ref());
    logs.push((
        log::Level::Info,
        format!(
            "Running \"{} {}\"",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|x| x.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    ));
    let output = cmd
        .output()
        .map_err(|e| ClangCaptureError::FailedToRunCommand {
            task: format!("execute clang-tidy on file {file_name}"),
            source: e,
        })?;

    logs.push((
        log::Level::Debug,
        format!(
            "Output from clang-tidy:\n{}",
            String::from_utf8_lossy(&output.stdout)
        ),
    ));
    if !output.stderr.is_empty() {
        logs.push((
            log::Level::Debug,
            format!(
                "clang-tidy made the following summary:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }

    let mut notes = parse_tidy_output(
        &output.stdout,
        &clang_params.database_json,
        &clang_params.repo_root,
    )?;

    let fixes = match fs::read_to_string(&fixes_path) {
        Ok(yml) => {
            let _ = fs::remove_file(&fixes_path);
            serde_saphyr::from_str::<ExportedFixes>(&yml).unwrap_or_else(|e| {
                logs.push((
                    log::Level::Error,
                    format!("Failed to parse fixes exported by clang-tidy for {file_name}: {e}"),
                ));
                ExportedFixes::default()
            })
        }
        // no file means no diagnostics
        Err(_) => ExportedFixes::default(),
    };
    let target = normalize_path(
        &std::path::absolute(&repo_file_path)
            .map_err(ClangCaptureError::UnknownWorkingDirectory)?,
    );
    let patched = apply_fixes(&fixes, &target, &original_content, &mut notes);
    fs::write(&cache_patch_path, patched).map_err(|e| ClangCaptureError::WriteFileFailed {
        file_name: cache_patch_path.to_string_lossy().to_string(),
        source: e,
    })?;

    let tidy_advice = TidyAdvice { notes };
    file.patched_path = Some(cache_patch_path.to_path_buf());
    file.tidy_advice = Some(tidy_advice);
    Ok(logs)
}

#[cfg(test)]
mod test {
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
        CLANG_DIAGNOSTICS_URL, ClangCaptureError, CompilationUnit, ExportedFixes, NOTE_HEADER,
        TidyNotification, apply_fixes, run_clang_tidy,
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
            rationale: String::from(
                "Dereference of null pointer (loaded from variable 'pipe_num')",
            ),
            severity: String::from("warning"),
            diagnostic: String::from("clang-analyzer-core.NullDereference"),
            suggestion: vec![],
            fixed_lines: vec![],
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
        let clang_params =
            make_clang_params(tmp_workspace.path(), Some(PathBuf::from("clang-tidy")));
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
    const FIX_SOURCE: &str =
        "#include \"fix.h\"\nvoid set(int *p) {\n  int x = *p;\n  (void)x;\n}\n";

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
        run_clang_tidy(&mut file, &clang_params).unwrap();

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
        let patched = apply_fixes(&fixes, &target, content, &mut notes);
        assert_eq!(String::from_utf8(patched).unwrap(), "line\n  auto\nx y;\n");
        assert_eq!(notes[0].fixed_lines, vec![2]);
    }

    #[cfg(unix)]
    #[test]
    fn tidy_bad_exported_fixes() {
        let tmp_workspace = crate::test_common::setup_tmp_workspace();
        let tools_dir = tempfile::tempdir().unwrap();
        // write invalid YAML to the path given to `--export-fixes`
        let fake_tidy = crate::test_common::make_fake_tool(
            tools_dir.path(),
            "clang-tidy",
            r#"while [ "$#" -gt 0 ]; do
  if [ "$1" = "--export-fixes" ]; then printf 'Diagnostics: [' > "$2"; fi
  shift
done"#,
        );
        let clang_params = make_clang_params(tmp_workspace.path(), Some(fake_tidy));
        let file = Mutex::new(FileObj::new(PathBuf::from("demo/demo.cpp")));
        let mut file = file.lock().unwrap();
        let logs = run_clang_tidy(&mut file, &clang_params).unwrap();
        assert!(
            logs.iter()
                .any(|(lvl, msg)| *lvl == log::Level::Error && msg.contains("Failed to parse"))
        );
        // the cached file is the original content
        assert_eq!(
            fs::read_to_string(file.patched_path.as_ref().unwrap()).unwrap(),
            fs::read_to_string("tests/demo/demo.cpp").unwrap()
        );
    }
}
