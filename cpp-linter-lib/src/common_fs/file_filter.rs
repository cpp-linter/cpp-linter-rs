use std::{
    fs,
    path::{Path, PathBuf},
};

use super::FileObj;

#[derive(Debug, Clone)]
pub struct FileFilter {
    pub ignored: Vec<String>,
    pub not_ignored: Vec<String>,
    pub extensions: Vec<String>,
}
impl FileFilter {
    pub fn new(ignore: &[String], extensions: Vec<String>) -> Self {
        let (ignored, not_ignored) = Self::parse_ignore(ignore);
        Self {
            ignored,
            not_ignored,
            extensions,
        }
    }

    /// This will parse the list of paths specified from the CLI using the `--ignore`
    /// argument.
    ///
    /// It returns 2 lists (in order):
    ///
    /// - `ignored` paths
    /// - `not_ignored` paths
    fn parse_ignore(ignore: &[String]) -> (Vec<String>, Vec<String>) {
        let mut ignored = vec![];
        let mut not_ignored = vec![];
        for pattern in ignore {
            let as_posix = pattern.replace('\\', "/");
            let mut pat = as_posix.as_str().trim();
            let is_ignored = !pat.starts_with('!');
            if !is_ignored {
                pat = pat[1..].trim_start();
            }
            if pat.starts_with("./") {
                pat = &pat[2..];
            }
            let is_hidden = pat.starts_with('.');
            if is_hidden || is_ignored {
                ignored.push(format!("./{pat}"));
            } else {
                not_ignored.push(format!("./{pat}"));
            }
        }
        (ignored, not_ignored)
    }

    /// This function will also read a .gitmodules file located in the working directory.
    /// The named submodules' paths will be automatically added to the ignored list,
    /// unless the submodule's path is already specified in the not_ignored list.
    pub fn parse_submodules(&mut self) {
        if let Ok(read_buf) = fs::read_to_string(".gitmodules") {
            for line in read_buf.split('\n') {
                if line.trim_start().starts_with("path") {
                    assert!(line.find('=').unwrap() > 0);
                    let submodule = String::from("./") + line.split('=').last().unwrap().trim();
                    log::debug!("Found submodule: {submodule}");
                    let mut is_ignored = true;
                    for pat in &self.not_ignored {
                        if pat == &submodule {
                            is_ignored = false;
                            break;
                        }
                    }
                    if is_ignored && !self.ignored.contains(&submodule) {
                        self.ignored.push(submodule);
                    }
                }
            }
        }
    }

    /// Describes if a specified `file_name` is contained within the given `set` of paths.
    ///
    /// The `is_ignored` flag describes which list of paths is used as domains.
    /// The specified `file_name` can be a direct or distant descendant of any paths in
    /// the list.
    ///
    /// Returns a [`Some`] value of the the path/pattern that matches the given `file_name`.
    /// If given `file_name` is not in the specified list, then [`None`] is returned.
    pub fn is_file_in_list(&self, file_name: &Path, is_ignored: bool) -> Option<String> {
        let file_name = PathBuf::from(format!(
            "./{}",
            file_name
                .as_os_str()
                .to_string_lossy()
                .to_string()
                .replace("\\", "/")
        ));
        let set = if is_ignored {
            &self.ignored
        } else {
            &self.not_ignored
        };
        for pattern in set {
            let pat = PathBuf::from(&pattern);
            if (pat.is_file() && file_name == pat) || (pat.is_dir() && file_name.starts_with(pat)) {
                return Some(pattern.to_owned());
            }
        }
        None
    }

    /// A helper function that checks if `entry` satisfies the following conditions (in
    /// ordered priority):
    ///
    /// - Does `entry`'s path use at least 1 of the listed file `extensions`? (takes
    ///   precedence)
    /// - Is `entry` *not* specified in list of `ignored` paths?
    /// - Is `entry` specified in the list of explicitly `not_ignored` paths? (supersedes
    ///   specified `ignored` paths)
    pub fn is_source_or_ignored(&self, entry: &Path) -> bool {
        let extension = entry.extension();
        if extension.is_none() {
            return false;
        }
        let mut is_ignored = true;
        for ext in &self.extensions {
            if ext == &extension.unwrap().to_os_string().into_string().unwrap() {
                is_ignored = false;
                break;
            }
        }
        if !is_ignored {
            let is_in_ignored = self.is_file_in_list(entry, true);
            let is_in_not_ignored = self.is_file_in_list(entry, false);
            if is_in_not_ignored.is_some() || is_in_ignored.is_none() {
                return true;
            }
        }
        false
    }

    /// Walks a given `root_path` recursively and returns a [`Vec<FileObj>`] that
    ///
    /// - uses at least 1 of the given `extensions`
    /// - is not specified in the internal list of `ignored` paths
    /// - is specified in the internal list `not_ignored` paths (which supersedes `ignored` paths)
    pub fn list_source_files(&self, root_path: &str) -> Vec<FileObj> {
        let mut files: Vec<FileObj> = Vec::new();
        let entries = fs::read_dir(root_path).expect("repo root-path should exist");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let mut is_hidden = false;
                let parent = path.components().last().expect("parent not known");
                if parent.as_os_str().to_str().unwrap().starts_with('.') {
                    is_hidden = true;
                }
                if !is_hidden {
                    files.extend(
                        self.list_source_files(&path.into_os_string().into_string().unwrap()),
                    );
                }
            } else {
                let is_valid_src = self.is_source_or_ignored(&path);
                if is_valid_src {
                    files.push(FileObj::new(
                        path.clone().strip_prefix("./").unwrap().to_path_buf(),
                    ));
                }
            }
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::FileFilter;
    use crate::cli::get_arg_parser;
    use std::{env::set_current_dir, path::PathBuf};

    // ************* tests for ignored paths

    fn setup_ignore(input: &str, extension: Vec<String>) -> FileFilter {
        let arg_parser = get_arg_parser();
        let args = arg_parser.get_matches_from(vec!["cpp-linter", "-i", input]);
        let ignore_arg = args
            .get_many::<String>("ignore")
            .unwrap()
            .map(|s| s.to_owned())
            .collect::<Vec<_>>();
        let file_filter = FileFilter::new(&ignore_arg, extension);
        println!("ignored = {:?}", file_filter.ignored);
        println!("not ignored = {:?}", file_filter.not_ignored);
        file_filter
    }

    #[test]
    fn ignore_src() {
        let file_filter = setup_ignore("src", vec![]);
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./src/lib.rs"), true)
            .is_some());
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./src/lib.rs"), false)
            .is_none());
    }

    #[test]
    fn ignore_root() {
        let file_filter = setup_ignore("!src/lib.rs|./", vec![]);
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./cargo.toml"), true)
            .is_some());
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./src/lib.rs"), false)
            .is_some());
    }

    #[test]
    fn ignore_root_implicit() {
        let file_filter = setup_ignore("!src|", vec![]);
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./cargo.toml"), true)
            .is_some());
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./src/lib.rs"), false)
            .is_some());
    }

    #[test]
    fn ignore_submodules() {
        set_current_dir("tests/ignored_paths").unwrap();
        let mut file_filter = setup_ignore("!pybind11", vec![]);
        file_filter.parse_submodules();

        // using Vec::contains() because these files don't actually exist in project files
        for ignored_submodule in ["./RF24", "./RF24Network", "./RF24Mesh"] {
            assert!(file_filter.ignored.contains(&ignored_submodule.to_string()));
            assert!(file_filter
                .is_file_in_list(
                    &PathBuf::from(ignored_submodule.to_string() + "/some_src.cpp"),
                    true
                )
                .is_none());
        }
        assert!(file_filter.not_ignored.contains(&"./pybind11".to_string()));
        assert!(file_filter
            .is_file_in_list(&PathBuf::from("./pybind11/some_src.cpp"), false)
            .is_none());
    }

    // *********************** tests for recursive path search

    #[test]
    fn walk_dir_recursively() {
        let extensions = vec!["cpp".to_string(), "hpp".to_string()];
        let file_filter = setup_ignore("target", extensions.clone());
        let files = file_filter.list_source_files(".");
        assert!(!files.is_empty());
        for file in files {
            assert!(extensions.contains(
                &file
                    .name
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            ));
        }
    }
}