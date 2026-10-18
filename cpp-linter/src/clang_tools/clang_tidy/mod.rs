//! This module holds functionality specific to running clang-tidy and parsing it's
//! output.

use std::{
    collections::HashMap,
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

pub mod other_files;
use other_files::{OtherFileFix, Replacement, TidyFixes};

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
                        // clang-tidy may report the path as it is written in the database
                        // (relative to the unit's directory) or joined with the directory.
                        if unit_path == filename || Path::new(&unit.file) == filename {
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
            }
        } else if let Some(note) = &mut notification {
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

    /// The directory that relative paths in this diagnostic are relative to.
    ///
    /// clang-tidy analyzes one translation unit per invocation,
    /// so this is the same for every diagnostic in one [`ExportedFixes`].
    /// If missing, paths are resolved from clang-tidy's working directory.
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

/// Resolves the paths in [`ExportedFixes`] and compares them to the analyzed file.
struct FixPaths<'a> {
    /// The absolute, normalized path to the analyzed file.
    target: &'a Path,
    /// The canonical path to the analyzed file, used when a lexical comparison fails
    /// (eg. because of symbolic links).
    canonical_target: Option<PathBuf>,
    /// clang-tidy's working directory (absolute).
    working_dir: &'a Path,
    /// Results of [`Self::other_file()`], keyed by the paths given to it.
    cache: HashMap<(&'a str, &'a str), Option<PathBuf>>,
}

impl<'a> FixPaths<'a> {
    fn new(target: &'a Path, working_dir: &'a Path) -> Self {
        Self {
            target,
            canonical_target: fs::canonicalize(target).ok(),
            working_dir,
            cache: HashMap::new(),
        }
    }

    /// Get the absolute, normalized path of a `path` reported by clang-tidy.
    fn resolve(&self, build_dir: &str, path: &str) -> PathBuf {
        let base = if build_dir.is_empty() {
            self.working_dir
        } else {
            Path::new(build_dir)
        };
        normalize_path(&base.join(path))
    }

    /// Get the resolved `path` (reported by clang-tidy) if it is not the analyzed file.
    ///
    /// Returns [`None`] if the `path` is the analyzed file.
    fn other_file(&mut self, build_dir: &'a str, path: &'a str) -> Option<PathBuf> {
        if let Some(result) = self.cache.get(&(build_dir, path)) {
            return result.clone();
        }
        let resolved = self.resolve(build_dir, path);
        let is_target = resolved == self.target
            || self
                .canonical_target
                .as_ref()
                .is_some_and(|target| fs::canonicalize(&resolved).is_ok_and(|p| p == *target));
        let result = (!is_target).then_some(resolved);
        self.cache.insert((build_dir, path), result.clone());
        result
    }
}

/// Convert a byte `offset` in `content` into a 1-based line number.
fn offset_to_line(content: &[u8], offset: usize) -> u32 {
    let end = offset.min(content.len());
    content[..end].iter().filter(|b| **b == b'\n').count() as u32 + 1
}

/// Apply the clang-tidy `fixes` that pertain to the analyzed file onto its `content`.
///
/// Fixes for any other file (eg. a header declaring a function defined in the analyzed
/// file) are not applied here, so nothing outside the analyzed file is ever changed.
/// They are returned in [`TidyFixes::others`] instead, to be applied (if allowed)
/// after all files are analyzed; see [`other_files::apply_other_file_fixes()`].
///
/// Each diagnostic's `fixed_lines` (in the original `content`) are recorded in the
/// matching `notes`.
fn apply_fixes<'a>(
    fixes: &'a ExportedFixes,
    paths: &mut FixPaths<'a>,
    content: &[u8],
    notes: &mut [TidyNotification],
    logs: &mut Vec<(log::Level, String)>,
) -> (Vec<u8>, TidyFixes) {
    let mut tidy_fixes = TidyFixes::default();
    for diag in &fixes.diagnostics {
        let fixed_lines = collect_diagnostic_fixes(diag, paths, content, &mut tidy_fixes, logs);
        if !fixed_lines.is_empty() {
            record_fixed_lines(diag, paths, content, notes, fixed_lines);
        }
    }
    let patched = patch_content(content, &tidy_fixes.own, paths.target, logs);
    (patched, tidy_fixes)
}

/// Sort the replacements of one `diag` into [`TidyFixes::own`] or [`TidyFixes::others`].
///
/// Returns the lines (in the analyzed file's `content`) that this diagnostic fixes.
fn collect_diagnostic_fixes<'a>(
    diag: &'a ExportedDiagnostic,
    paths: &mut FixPaths<'a>,
    content: &[u8],
    tidy_fixes: &mut TidyFixes,
    logs: &mut Vec<(log::Level, String)>,
) -> Vec<u32> {
    let build_dir = diag.build_directory.as_str();
    let mut fixed_lines = vec![];
    for replacement in &diag.diagnostic_message.replacements {
        if let Some(other) = paths.other_file(build_dir, &replacement.file_path) {
            tidy_fixes.others.push(OtherFileFix {
                path: other,
                diagnostic: diag.diagnostic_name.clone(),
                replacement: Replacement::from(replacement),
            });
        } else if replacement
            .offset
            .checked_add(replacement.length)
            .is_some_and(|end| end <= content.len())
        {
            let line = offset_to_line(content, replacement.offset);
            if !fixed_lines.contains(&line) {
                fixed_lines.push(line);
            }
            tidy_fixes.own.push(Replacement::from(replacement));
        } else {
            logs.push((
                log::Level::Warn,
                format!(
                    "Skipping a fix for {} that is out of bounds of {}: {replacement:?}",
                    diag.diagnostic_name,
                    paths.target.to_string_lossy(),
                ),
            ));
        }
    }
    fixed_lines
}

/// Record the `fixed_lines` of a `diag` in the notification (parsed from stdout)
/// that matches it.
fn record_fixed_lines<'a>(
    diag: &'a ExportedDiagnostic,
    paths: &mut FixPaths<'a>,
    content: &[u8],
    notes: &mut [TidyNotification],
    fixed_lines: Vec<u32>,
) {
    let msg = &diag.diagnostic_message;
    let (diag_path, diag_line) = match paths.other_file(&diag.build_directory, &msg.file_path) {
        None => (
            paths.target.to_path_buf(),
            Some(offset_to_line(content, msg.file_offset)),
        ),
        Some(other) => {
            let line = fs::read(&other)
                .ok()
                .map(|c| offset_to_line(&c, msg.file_offset));
            (other, line)
        }
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

/// Apply the given `replacements` onto a file's original `content`.
///
/// The `file` is only used in log messages.
pub(super) fn patch_content(
    content: &[u8],
    replacements: &[Replacement],
    file: &Path,
    logs: &mut Vec<(log::Level, String)>,
) -> Vec<u8> {
    // clang-tidy already discards conflicting fixes before exporting them,
    // but be defensive about duplicates and overlaps anyway.
    // Duplicates are expected when several translation units fix the same header.
    let mut replacements = replacements.iter().collect::<Vec<_>>();
    replacements.sort();
    replacements.dedup();
    let mut patched = Vec::with_capacity(content.len());
    let mut cursor = 0;
    for replacement in replacements {
        if replacement.offset < cursor {
            logs.push((
                log::Level::Warn,
                format!(
                    "Skipping a fix that overlaps another fix in {}: {replacement:?}",
                    file.to_string_lossy(),
                ),
            ));
            continue;
        }
        patched.extend_from_slice(&content[cursor..replacement.offset]);
        patched.extend_from_slice(replacement.text.as_bytes());
        cursor = replacement.offset + replacement.length;
    }
    patched.extend_from_slice(&content[cursor..]);
    patched
}

/// Write the `patched` content of a `file` to the cache.
///
/// Returns the path to the cached file.
pub(super) fn write_cache(
    clang_params: &ClangParams,
    file: &Path,
    patched: &[u8],
) -> Result<PathBuf, ClangCaptureError> {
    let cache_patch_path = clang_params.get_cache_path().join(file);
    fs::create_dir_all(
        cache_patch_path
            .parent()
            .ok_or(ClangCaptureError::UnknownCacheParentPath)?,
    )
    .map_err(ClangCaptureError::MkDirFailed)?;
    fs::write(&cache_patch_path, patched).map_err(|e| ClangCaptureError::WriteFileFailed {
        file_name: cache_patch_path.to_string_lossy().to_string(),
        source: e,
    })?;
    Ok(cache_patch_path)
}

/// Build the clang-tidy command that analyzes the given `file`.
///
/// clang-tidy exports its fixes to `fixes_path` instead of applying them.
fn build_tidy_command(
    file: &FileObj,
    clang_params: &ClangParams,
    fixes_path: &Path,
) -> Result<Command, ClangCaptureError> {
    let cmd_path = clang_params
        .clang_tidy_command
        .as_ref()
        .ok_or(ClangCaptureError::ToolPathUnknown("clang-tidy"))?;
    let mut cmd = Command::new(cmd_path);
    cmd.current_dir(&clang_params.repo_root);
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
    let file_name = file.name.to_string_lossy();
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
    cmd.arg("--export-fixes");
    cmd.arg(fixes_path);
    cmd.arg(file_name.as_ref());
    Ok(cmd)
}

/// Remove the file at `path`, if it exists.
fn remove_file_if_exists(path: &Path) -> Result<(), ClangCaptureError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(ClangCaptureError::RemoveFileFailed {
                file_name: path.to_string_lossy().to_string(),
                source: e,
            })
        }
        _ => Ok(()),
    }
}

/// Read (and then remove) the fixes that clang-tidy exported to `fixes_path`.
///
/// clang-tidy does not write the file if there was nothing to report.
fn read_exported_fixes(fixes_path: &Path) -> Result<ExportedFixes, ClangCaptureError> {
    let file_name = fixes_path.to_string_lossy().to_string();
    let yml = match fs::read_to_string(fixes_path) {
        Ok(yml) => yml,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExportedFixes::default());
        }
        Err(e) => {
            return Err(ClangCaptureError::ReadFileFailed {
                file_name,
                source: e,
            });
        }
    };
    remove_file_if_exists(fixes_path)?;
    serde_saphyr::from_str(&yml).map_err(|e| ClangCaptureError::ParseFixesFailed {
        file_name,
        source: Box::new(e),
    })
}

/// Run clang-tidy, then parse and return it's output.
///
/// clang-tidy is not allowed to modify any files.
/// Instead, its fixes are exported (`--export-fixes`), and only the fixes for the
/// analyzed file are applied to a copy of that file in the cache.
/// The fixes for other files are returned in [`TidyFixes::others`].
pub fn run_clang_tidy(
    file: &mut MutexGuard<FileObj>,
    clang_params: &ClangParams,
) -> Result<(Vec<(log::Level, String)>, TidyFixes), ClangCaptureError> {
    let file_name = file.name.to_string_lossy().to_string();
    let repo_file_path = clang_params.repo_root.join(&file.name);
    let original_content =
        fs::read(&repo_file_path).map_err(|e| ClangCaptureError::ReadFileFailed {
            file_name: file_name.clone(),
            source: e,
        })?;
    let cache_patch_path = clang_params.get_cache_path().join(&file.name);
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
    // Don't let a stale file from a previous run get used.
    remove_file_if_exists(&fixes_path)?;

    let mut cmd = build_tidy_command(file, clang_params, &fixes_path)?;
    let mut logs = vec![(
        log::Level::Info,
        format!(
            "Running \"{} {}\"",
            cmd.get_program().to_string_lossy(),
            cmd.get_args()
                .map(|x| x.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    )];
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

    let fixes = read_exported_fixes(&fixes_path)?;
    let working_dir = std::path::absolute(&clang_params.repo_root)
        .map_err(ClangCaptureError::UnknownWorkingDirectory)?;
    let target = normalize_path(&working_dir.join(&file.name));
    let mut paths = FixPaths::new(&target, &working_dir);
    let (patched, tidy_fixes) =
        apply_fixes(&fixes, &mut paths, &original_content, &mut notes, &mut logs);
    file.patched_path = Some(write_cache(clang_params, &file.name, &patched)?);
    file.tidy_advice = Some(TidyAdvice { notes });
    Ok((logs, tidy_fixes))
}

#[cfg(test)]
mod test;
