//! Applying clang-tidy fixes to files other than the analyzed one.
//!
//! A fix for a diagnostic in one file can also change another file
//! (eg. `readability-non-const-parameter` adds `const` to the definition in a source
//! file and to the declaration in a header). clang-tidy analyzes one file at a time,
//! so these fixes are collected while files are analyzed in parallel, and applied
//! once all files are analyzed. This way, no two threads write the same cached file.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clang_tools_manager::utils::normalize_path;

use super::{
    ExportedReplacement, TidyAdvice, TidyNotification, offset_to_line, patch_content, write_cache,
};
use crate::{cli::ClangParams, common_fs::FileObj, error::ClangCaptureError};

/// A text replacement in a file's original content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Replacement {
    /// The byte offset where the replaced text starts.
    pub offset: usize,
    /// The length (in bytes) of the replaced text.
    pub length: usize,
    /// The replacement text.
    pub text: String,
}

impl From<&ExportedReplacement> for Replacement {
    fn from(value: &ExportedReplacement) -> Self {
        Self {
            offset: value.offset,
            length: value.length,
            text: value.replacement_text.clone(),
        }
    }
}

/// A clang-tidy fix for a file other than the analyzed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherFileFix {
    /// The absolute, normalized path to the other file.
    pub path: PathBuf,
    /// The name of the diagnostic that suggested the fix.
    pub diagnostic: String,
    /// The replacement in the other file's original content.
    pub replacement: Replacement,
    /// The notification (parsed from clang-tidy's stdout) about the diagnostic,
    /// if one was found.
    ///
    /// Once the fix is applied, a copy (with the fixed line in the other file) is
    /// added to the other file's [`TidyAdvice`], so suggestions in a PR review can
    /// explain the fix.
    pub note: Option<TidyNotification>,
}

/// The fixes that clang-tidy suggested while analyzing a file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TidyFixes {
    /// The replacements in the analyzed file.
    pub own: Vec<Replacement>,
    /// The replacements in other files.
    pub others: Vec<OtherFileFix>,
}

/// Why a [`OtherFileFix`] was not applied.
enum Rejection {
    NotAnalyzed,
    IgnoredByTidy,
    /// The fix's byte offsets do not fit in the file.
    OutOfBounds {
        /// The byte offset where the fix starts.
        start: usize,
        /// The byte offset where the fix ends (exclusive).
        end: usize,
        /// The file's size (in bytes).
        size: usize,
    },
    OutsideChangedLines,
}

impl Rejection {
    fn reason(&self) -> String {
        match self {
            Self::NotAnalyzed => "it is not one of the analyzed files".to_string(),
            Self::IgnoredByTidy => "it is ignored by `--ignore-tidy`".to_string(),
            Self::OutOfBounds { start, end, size } => format!(
                "the fix seems malformed and is out of the file's bounds \
                (bytes {start} to {end} of a {size} byte file)"
            ),
            Self::OutsideChangedLines => {
                "the fix clashes with the `--lines-changed-only` constraint".to_string()
            }
        }
    }
}

/// The analyzed files, looked up by their absolute path.
struct AnalyzedFiles<'a> {
    files: &'a [Arc<Mutex<FileObj>>],
    by_path: HashMap<PathBuf, usize>,
    contents: HashMap<usize, Vec<u8>>,
}

impl<'a> AnalyzedFiles<'a> {
    fn new(
        files: &'a [Arc<Mutex<FileObj>>],
        working_dir: &Path,
    ) -> Result<Self, ClangCaptureError> {
        let mut by_path = HashMap::new();
        for (index, file) in files.iter().enumerate() {
            let file = file.lock().map_err(|_| ClangCaptureError::MutexPoisoned)?;
            let path = normalize_path(&working_dir.join(&file.name));
            // also match paths that clang-tidy reports through symbolic links
            if let Ok(canonical) = fs::canonicalize(&path) {
                by_path.insert(canonical, index);
            }
            by_path.insert(path, index);
        }
        Ok(Self {
            files,
            by_path,
            contents: HashMap::new(),
        })
    }

    /// Get the index of the analyzed file at `path`.
    fn find(&self, path: &Path) -> Option<usize> {
        self.by_path.get(path).copied().or_else(|| {
            fs::canonicalize(path)
                .ok()
                .and_then(|canonical| self.by_path.get(&canonical).copied())
        })
    }

    /// Get (and cache) the original content of the analyzed file at `index`.
    fn content(
        &mut self,
        index: usize,
        clang_params: &ClangParams,
    ) -> Result<&[u8], ClangCaptureError> {
        if !self.contents.contains_key(&index) {
            let name = self.files[index]
                .lock()
                .map_err(|_| ClangCaptureError::MutexPoisoned)?
                .name
                .clone();
            let content = fs::read(clang_params.repo_root.join(&name)).map_err(|e| {
                ClangCaptureError::ReadFileFailed {
                    file_name: name.to_string_lossy().to_string(),
                    source: e,
                }
            })?;
            self.contents.insert(index, content);
        }
        Ok(self.contents[&index].as_slice())
    }

    /// Decide whether the `fix` may be applied to the analyzed file at `index`.
    ///
    /// Returns the line where the fix starts if it may be applied.
    fn check(
        &mut self,
        index: usize,
        fix: &OtherFileFix,
        clang_params: &ClangParams,
    ) -> Result<Result<u32, Rejection>, ClangCaptureError> {
        let content_len = self.content(index, clang_params)?.len();
        let replacement = &fix.replacement;
        let Some(end) = replacement
            .offset
            .checked_add(replacement.length)
            .filter(|end| *end <= content_len)
        else {
            return Ok(Err(Rejection::OutOfBounds {
                start: replacement.offset,
                end: replacement.offset.saturating_add(replacement.length),
                size: content_len,
            }));
        };
        let content = self.content(index, clang_params)?;
        // `end` is exclusive, so use the last replaced byte (if any) for the end line
        let last = if replacement.length > 0 { end - 1 } else { end };
        let (start_line, end_line) = (
            offset_to_line(content, replacement.offset),
            offset_to_line(content, last),
        );
        let file = self.files[index]
            .lock()
            .map_err(|_| ClangCaptureError::MutexPoisoned)?;
        if clang_params
            .tidy_filter
            .as_ref()
            .is_some_and(|f| !f.is_qualified(&file.name))
        {
            return Ok(Err(Rejection::IgnoredByTidy));
        }
        let ranges = file.get_ranges(&clang_params.lines_changed_only);
        if !ranges.is_empty()
            && !ranges
                .iter()
                .any(|r| r.contains(&start_line) && r.contains(&end_line))
        {
            return Ok(Err(Rejection::OutsideChangedLines));
        }
        Ok(Ok(start_line))
    }
}

/// Apply the clang-tidy fixes for other files (see [`TidyFixes::others`]) to the
/// cached copies of those files.
///
/// `tidy_fixes` holds the fixes found while analyzing the file at the paired index of
/// `files`. A fix for another file is only applied if that file is also analyzed,
/// is not ignored by `--ignore-tidy`, and the fix is within the file's changed lines
/// (according to `--lines-changed-only`).
///
/// Messages about each fix are added to the `logs` of the analyzed file whose
/// diagnostic suggested it (at the paired index of `files`).
///
/// This must be called after all files are analyzed by clang-tidy and before any
/// file is analyzed by clang-format, because clang-format formats the fixes in the
/// cached copy.
pub fn apply_other_file_fixes(
    files: &[Arc<Mutex<FileObj>>],
    tidy_fixes: &[(usize, TidyFixes)],
    clang_params: &ClangParams,
    logs: &mut [Vec<(log::Level, String)>],
) -> Result<(), ClangCaptureError> {
    if tidy_fixes.iter().all(|(_, fixes)| fixes.others.is_empty()) {
        return Ok(());
    }
    let working_dir = std::path::absolute(&clang_params.repo_root)
        .map_err(ClangCaptureError::UnknownWorkingDirectory)?;
    let mut analyzed = AnalyzedFiles::new(files, &working_dir)?;
    // the replacements to apply, keyed by the index of the file they apply to
    let mut pending: BTreeMap<usize, Pending> = BTreeMap::new();
    for (source_index, fixes) in tidy_fixes {
        let source_name = file_name(&files[*source_index])?;
        for fix in &fixes.others {
            // show the path relative to the repository root, if possible
            let other = fix
                .path
                .strip_prefix(&working_dir)
                .unwrap_or(&fix.path)
                .to_string_lossy();
            let verdict = match analyzed.find(&fix.path) {
                None => Err(Rejection::NotAnalyzed),
                Some(index) => {
                    let verdict = analyzed.check(index, fix, clang_params)?;
                    if let Ok(line) = verdict {
                        pending.entry(index).or_default().add(fix, line);
                    }
                    verdict
                }
            };
            logs[*source_index].push(match verdict.err() {
                None => (
                    log::Level::Info,
                    format!(
                        "Applying the fix for {} (reported in {source_name}) to {other}",
                        fix.diagnostic,
                    ),
                ),
                Some(rejection) => (
                    log::Level::Info,
                    format!(
                        "Not applying the fix for {} (reported in {source_name}) to {other} because {}",
                        fix.diagnostic,
                        rejection.reason(),
                    ),
                ),
            });
        }
    }
    for (
        index,
        Pending {
            mut replacements,
            notes,
        },
    ) in pending
    {
        if let Some((_, fixes)) = tidy_fixes.iter().find(|(i, _)| *i == index) {
            replacements.extend(fixes.own.iter().cloned());
        }
        let content = analyzed.content(index, clang_params)?;
        let mut file = files[index]
            .lock()
            .map_err(|_| ClangCaptureError::MutexPoisoned)?;
        let patched = patch_content(content, &replacements, &file.name, &mut logs[index]);
        file.patched_path = Some(write_cache(clang_params, &file.name, &patched)?);
        let name = file.name.to_string_lossy().replace('\\', "/");
        let advice = file
            .tidy_advice
            .get_or_insert_with(|| TidyAdvice { notes: vec![] });
        for note in notes {
            merge_note(&mut advice.notes, note, &name);
        }
    }
    Ok(())
}

/// The accepted fixes for one analyzed file.
#[derive(Default)]
struct Pending {
    replacements: Vec<Replacement>,
    /// Copies of the notes about the diagnostics that suggested the fixes,
    /// with the fixed lines in the analyzed file.
    notes: Vec<TidyNotification>,
}

impl Pending {
    fn add(&mut self, fix: &OtherFileFix, line: u32) {
        self.replacements.push(fix.replacement.clone());
        let Some(note) = &fix.note else {
            return;
        };
        let note = TidyNotification {
            fixed_lines: vec![line],
            ..note.clone()
        };
        if let Some(existing) = self.notes.iter_mut().find(|n| is_same_note(n, &note)) {
            if !existing.fixed_lines.contains(&line) {
                existing.fixed_lines.push(line);
            }
        } else {
            self.notes.push(note);
        }
    }
}

fn is_same_note(a: &TidyNotification, b: &TidyNotification) -> bool {
    a.filename == b.filename && a.line == b.line && a.diagnostic == b.diagnostic
}

/// Add the fixed lines of a `note` to the `notes` of the analyzed file named `file_name`.
///
/// A note about another file (eg. the source file that suggested the fix) is added,
/// so suggestions in a PR review can explain the fix; notes about other files are not
/// counted as this file's diagnostics. A note about this file is only merged into the
/// same note found by analyzing this file, so it is not counted twice.
fn merge_note(notes: &mut Vec<TidyNotification>, note: TidyNotification, file_name: &str) {
    if let Some(existing) = notes.iter_mut().find(|n| is_same_note(n, &note)) {
        for line in note.fixed_lines {
            if !existing.fixed_lines.contains(&line) {
                existing.fixed_lines.push(line);
            }
        }
    } else if note.filename != file_name {
        notes.push(note);
    }
}

fn file_name(file: &Arc<Mutex<FileObj>>) -> Result<String, ClangCaptureError> {
    Ok(file
        .lock()
        .map_err(|_| ClangCaptureError::MutexPoisoned)?
        .name
        .to_string_lossy()
        .to_string())
}

#[cfg(test)]
mod test;
