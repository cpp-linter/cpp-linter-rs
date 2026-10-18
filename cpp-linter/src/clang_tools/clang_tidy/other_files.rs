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

use super::{ExportedReplacement, offset_to_line, patch_content, write_cache};
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
    OutOfBounds,
    OutsideChangedLines,
}

impl Rejection {
    fn reason(&self) -> &'static str {
        match self {
            Self::NotAnalyzed => "it is not one of the analyzed files",
            Self::IgnoredByTidy => "it is ignored by `--ignore-tidy`",
            Self::OutOfBounds => "the fix is out of the file's bounds",
            Self::OutsideChangedLines => "the fix is outside the file's changed lines",
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
    fn check(
        &mut self,
        index: usize,
        fix: &OtherFileFix,
        clang_params: &ClangParams,
    ) -> Result<Option<Rejection>, ClangCaptureError> {
        let content_len = self.content(index, clang_params)?.len();
        let replacement = &fix.replacement;
        let Some(end) = replacement
            .offset
            .checked_add(replacement.length)
            .filter(|end| *end <= content_len)
        else {
            return Ok(Some(Rejection::OutOfBounds));
        };
        let content = self.content(index, clang_params)?;
        let (start_line, end_line) = (
            offset_to_line(content, replacement.offset),
            offset_to_line(content, end),
        );
        let file = self.files[index]
            .lock()
            .map_err(|_| ClangCaptureError::MutexPoisoned)?;
        if clang_params
            .tidy_filter
            .as_ref()
            .is_some_and(|f| !f.is_qualified(&file.name))
        {
            return Ok(Some(Rejection::IgnoredByTidy));
        }
        let ranges = file.get_ranges(&clang_params.lines_changed_only);
        if !ranges.is_empty()
            && !ranges
                .iter()
                .any(|r| r.contains(&start_line) && r.contains(&end_line))
        {
            return Ok(Some(Rejection::OutsideChangedLines));
        }
        Ok(None)
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
    let mut pending: BTreeMap<usize, Vec<Replacement>> = BTreeMap::new();
    for (source, fixes) in tidy_fixes {
        let source_name = file_name(&files[*source])?;
        for fix in &fixes.others {
            // show the path relative to the repository root, if possible
            let other = fix
                .path
                .strip_prefix(&working_dir)
                .unwrap_or(&fix.path)
                .to_string_lossy();
            let rejection = match analyzed.find(&fix.path) {
                None => Some(Rejection::NotAnalyzed),
                Some(index) => {
                    let rejection = analyzed.check(index, fix, clang_params)?;
                    if rejection.is_none() {
                        pending
                            .entry(index)
                            .or_default()
                            .push(fix.replacement.clone());
                    }
                    rejection
                }
            };
            logs[*source].push(match rejection {
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
    for (index, mut replacements) in pending {
        if let Some((_, fixes)) = tidy_fixes.iter().find(|(i, _)| *i == index) {
            replacements.extend(fixes.own.iter().cloned());
        }
        let content = analyzed.content(index, clang_params)?;
        let mut file = files[index]
            .lock()
            .map_err(|_| ClangCaptureError::MutexPoisoned)?;
        let patched = patch_content(content, &replacements, &file.name, &mut logs[index]);
        file.patched_path = Some(write_cache(clang_params, &file.name, &patched)?);
    }
    Ok(())
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
