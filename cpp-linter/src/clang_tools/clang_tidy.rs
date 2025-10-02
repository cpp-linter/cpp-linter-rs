//! This module holds functionality specific to running clang-tidy and parsing it's
//! output.

use std::{
    env::{consts::OS, current_dir},
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context, Result};
// non-std crates
use regex::Regex;
use serde::Deserialize;

// project-specific modules/crates
use super::MakeSuggestions;
use crate::{
    cli::{ClangParams, LinesChangedOnly},
    common_fs::{normalize_path, FileObj},
};

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

    /// The list of line numbers that had fixes applied via `clang-tidy --fix-error`.
    pub fixed_lines: Vec<u32>,
}

impl TidyNotification {
    pub fn diagnostic_link(&self) -> String {
        if self.diagnostic.starts_with("clang-diagnostic") {
            return self.diagnostic.clone();
        }
        let (category, name) = if self.diagnostic.starts_with("clang-analyzer-") {
            (
                "clang-analyzer",
                self.diagnostic.strip_prefix("clang-analyzer-").unwrap(),
            )
        } else {
            self.diagnostic.split_once('-').unwrap()
        };
        format!(
            "[{}](https://clang.llvm.org/extra/clang-tidy/checks/{category}/{name}.html)",
            self.diagnostic
        )
    }
}

/// A struct to hold notification from clang-tidy about a single file
#[derive(Debug, Clone)]
pub struct TidyAdvice {
    /// A list of notifications parsed from clang-tidy stdout.
    pub notes: Vec<TidyNotification>,
    pub patched: Option<Vec<u8>>,
}

impl MakeSuggestions for TidyAdvice {
    fn get_suggestion_help(&self, start_line: u32, end_line: u32) -> String {
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

    fn get_tool_name(&self) -> String {
        "clang-tidy".to_string()
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
) -> Result<TidyAdvice> {
    let note_header = Regex::new(NOTE_HEADER).unwrap();
    let fixed_note =
        Regex::new(r"^.+:(\d+):\d+:\snote: FIX-IT applied suggested code changes$").unwrap();
    let mut found_fix = false;
    let mut notification = None;
    let mut result = Vec::new();
    let cur_dir = current_dir().unwrap();
    for line in String::from_utf8(tidy_stdout.to_vec()).unwrap().lines() {
        if let Some(captured) = note_header.captures(line) {
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
                    filename = normalize_path(&PathBuf::from_iter([&cur_dir, &filename]));
                }
            } else {
                // still need to normalize the relative path despite missing database info.
                // let's assume the file is relative to current working directory.
                filename = normalize_path(&PathBuf::from_iter([&cur_dir, &filename]));
            }
            assert!(filename.is_absolute());
            if filename.is_absolute() && filename.starts_with(&cur_dir) {
                // if this filename can't be made into a relative path, then it is
                // likely not a member of the project's sources (ie /usr/include/stdio.h)
                filename = filename
                    .strip_prefix(&cur_dir)
                    // we already checked above that filename.starts_with(current_directory)
                    .unwrap()
                    .to_path_buf();
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
        } else if let Some(capture) = fixed_note.captures(line) {
            let fixed_line = capture[1].parse()?;
            if let Some(note) = &mut notification {
                if !note.fixed_lines.contains(&fixed_line) {
                    note.fixed_lines.push(fixed_line);
                }
            }
            // Suspend capturing subsequent lines as suggestions until
            // a new notification is constructed. If we found a note about applied fixes,
            // then the lines of suggestions for that notification have already been parsed.
            found_fix = true;
        } else if !found_fix {
            if let Some(note) = &mut notification {
                // append lines of code that are part of
                // the previous line's notification
                note.suggestion.push(line.to_string());
            }
        }
    }
    if let Some(note) = notification {
        result.push(note);
    }
    Ok(TidyAdvice {
        notes: result,
        patched: None,
    })
}

/// Get a total count of clang-tidy advice from the given list of [FileObj]s.
pub fn tally_tidy_advice(files: &[Arc<Mutex<FileObj>>]) -> u64 {
    let mut total = 0;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(advice) = &file.tidy_advice {
            for tidy_note in &advice.notes {
                let file_path = PathBuf::from(&tidy_note.filename);
                if file_path == file.name {
                    total += 1;
                }
            }
        }
    }
    total
}

/// Run clang-tidy, then parse and return it's output.
pub fn run_clang_tidy(
    file: &mut MutexGuard<FileObj>,
    clang_params: &ClangParams,
) -> Result<Vec<(log::Level, std::string::String)>> {
    let mut cmd = Command::new(clang_params.clang_tidy_command.as_ref().unwrap());
    let mut logs = vec![];
    if !clang_params.tidy_checks.is_empty() {
        cmd.args(["-checks", &clang_params.tidy_checks]);
    }
    if let Some(db) = &clang_params.database {
        cmd.args(["-p", &db.to_string_lossy()]);
    }
    for arg in &clang_params.extra_args {
        cmd.args(["--extra-arg", format!("\"{}\"", arg).as_str()]);
    }
    let file_name = file.name.to_string_lossy().to_string();
    if clang_params.lines_changed_only != LinesChangedOnly::Off {
        let ranges = file.get_ranges(&clang_params.lines_changed_only);
        if !ranges.is_empty() {
            let filter = format!(
                "[{{\"name\":{:?},\"lines\":{:?}}}]",
                &file_name.replace('/', if OS == "windows" { "\\" } else { "/" }),
                ranges
                    .iter()
                    .map(|r| [r.start(), r.end()])
                    .collect::<Vec<_>>()
            );
            cmd.args(["--line-filter", filter.as_str()]);
        }
    }
    let original_content = if !clang_params.tidy_review {
        None
    } else {
        cmd.arg("--fix-errors");
        Some(fs::read_to_string(&file.name).with_context(|| {
            format!(
                "Failed to cache file's original content before applying clang-tidy changes: {}",
                file_name.clone()
            )
        })?)
    };
    if !clang_params.style.is_empty() {
        cmd.args(["--format-style", clang_params.style.as_str()]);
    }
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
    let output = cmd.output().unwrap();
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
    file.tidy_advice = Some(parse_tidy_output(
        &output.stdout,
        &clang_params.database_json,
    )?);
    if clang_params.tidy_review {
        if let Some(tidy_advice) = &mut file.tidy_advice {
            // cache file changes in a buffer and restore the original contents for further analysis
            tidy_advice.patched =
                Some(fs::read(&file_name).with_context(|| {
                    format!("Failed to read changes from clang-tidy: {file_name}")
                })?);
        }
        // original_content is guaranteed to be Some() value at this point
        fs::write(&file_name, original_content.unwrap())
            .with_context(|| format!("Failed to restore file's original content: {file_name}"))?;
    }
    Ok(logs)
}

#[cfg(test)]
mod test {
    use std::{
        env,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use regex::Regex;

    use crate::{
        clang_tools::get_clang_tool_exe,
        cli::{ClangParams, LinesChangedOnly},
        common_fs::FileObj,
    };

    use super::{run_clang_tidy, TidyNotification, NOTE_HEADER};

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

    // ***************** test for regex parsing of clang-tidy stdout

    #[test]
    fn test_capture() {
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
        let exe_path = get_clang_tool_exe(
            "clang-tidy",
            env::var("CLANG_VERSION").unwrap_or("".to_string()).as_str(),
        )
        .unwrap();
        let file = FileObj::new(PathBuf::from("tests/demo/demo.cpp"));
        let arc_ref = Arc::new(Mutex::new(file));
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
            tidy_review: false,
            format_review: false,
            clang_tidy_command: Some(exe_path),
            clang_format_command: None,
        };
        let mut file_lock = arc_ref.lock().unwrap();
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
        for arg in &extra_args {
            let extra_arg = format!("\"{arg}\"");
            assert!(args.contains(&extra_arg.as_str()));
        }
    }
}
