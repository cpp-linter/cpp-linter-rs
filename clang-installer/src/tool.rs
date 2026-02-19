use std::{
    env::current_dir,
    fmt::{self, Display, Formatter},
    path::PathBuf,
    process::Command,
};

use regex::Regex;
use semver::Version;
use which::{which, which_in};

use crate::RequestedVersion;

/// Error that occur when trying to get the path to a clang tool executable.
#[derive(Debug, thiserror::Error)]
pub enum GetClangPathError {
    /// Failed to access current working directory.
    #[error("Failed to access current working directory: {0}")]
    InvalidCurrentDirectory(#[from] std::io::Error),

    /// Failed to find the clang tool binary by searching for the provided name.
    #[error("Failed to find the {0} binary by searching for the provided name: {1}")]
    NotFoundByName(ClangTool, which::Error),

    /// Failed to find the clang tool binary by searching for the provided version requirement.
    #[error("Failed to find the {0} binary by searching for the provided version requirement: {1}")]
    NotFoundByVersion(ClangTool, which::Error),

    /// Failed to find the clang tool binary by searching for the provided path.
    #[error("Failed to find the {0} binary by searching for the provided path: {1}")]
    NotFoundByPath(ClangTool, which::Error),
}

/// Error that occur when trying to get the version number of a clang tool executable's output.
#[derive(Debug, thiserror::Error)]
pub enum GetClangVersionError {
    /// Failed to run the clang tool executable with `--version` flag.
    #[error("Failed to run `{0} --version` flag: {1}")]
    Command(PathBuf, std::io::Error),

    /// Regex pattern failed to compile.
    #[error("Regex pattern failed to compile: {0}")]
    RegexCompile(#[from] regex::Error),

    /// Failed to parse the version number from the output of `clang-tool --version`.
    #[error("Failed to parse the version number from the `--version` output")]
    VersionParse,
}

#[derive(Debug, Clone, Copy)]
pub enum ClangTool {
    ClangTidy,
    ClangFormat,
}

impl Display for ClangTool {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl ClangTool {
    /// Get the string representation of the clang tool's name.
    pub const fn as_str(&self) -> &'static str {
        match self {
            ClangTool::ClangTidy => "clang-tidy",
            ClangTool::ClangFormat => "clang-format",
        }
    }

    /// Fetch the path to an executable clang tool for the specified `version`.
    ///
    /// If the executable is not found using the specified `version`, then the tool is
    /// sought only by it's name ([`Self::as_str()`]).
    ///
    /// The only reason this function would return an error is if the specified tool is not
    /// installed or present on the system (nor in the `PATH` environment variable).
    pub fn get_exe_path(&self, version: &RequestedVersion) -> Result<PathBuf, GetClangPathError> {
        let name = self.as_str();
        match version {
            RequestedVersion::Path(path_buf) => which_in(
                name,
                Some(path_buf),
                current_dir().map_err(GetClangPathError::InvalidCurrentDirectory)?,
            )
            .map_err(|e| GetClangPathError::NotFoundByPath(*self, e)),
            // Thus, we should use whatever is installed and added to $PATH.
            RequestedVersion::SystemDefault | RequestedVersion::NoValue => {
                which(name).map_err(|e| GetClangPathError::NotFoundByName(*self, e))
            }
            RequestedVersion::Requirement(req) => {
                // `req.comparators` has at least a major version number for each comparator.
                // We need to start with the highest major version number first, then
                // decrement to the lowest that satisfies the requirement.

                // find the highest major version from requirement's boundaries.
                let mut it = req.comparators.iter();
                let mut highest_major = it.next().map(|v| v.major).unwrap_or_default() + 1;
                for n in it {
                    if n.major > highest_major {
                        // +1 because we aren't checking the comparator's operator here.
                        highest_major = n.major + 1;
                    }
                }

                // aggregate by decrementing through major versions that satisfy the requirement.
                let mut majors = vec![];
                while highest_major > 0 {
                    // check if the current major version satisfies the requirement.
                    if req.matches(&Version::new(highest_major, 0, 0)) {
                        majors.push(highest_major);
                    }
                    highest_major -= 1;
                }

                // now we're ready to search for the binary exe with the major version suffixed.
                for major in majors {
                    if let Ok(cmd) = which(format!("{self}-{major}")) {
                        return Ok(cmd);
                    }
                }
                // failed to find a binary where the major version number is suffixed to the tool name.

                // This line essentially ignores the version specified as a fail-safe.
                //
                // On Windows, the version's major number is typically not appended to the name of
                // the executable (or symlink for executable), so this is useful in that scenario.
                //
                // On Unix systems, this line is not likely reached. Typically, installing clang
                // will produce a symlink to the executable with the major version appended to the
                // name.
                which(name).map_err(|e| GetClangPathError::NotFoundByVersion(*self, e))
            }
        }
    }

    /// Run `clang-tool --version`, then extract and return the version number.
    pub fn capture_version(clang_tool: &PathBuf) -> Result<String, GetClangVersionError> {
        let output = Command::new(clang_tool)
            .arg("--version")
            .output()
            .map_err(|e| GetClangVersionError::Command(clang_tool.clone(), e))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version_pattern =
            Regex::new(r"(?i)version[^\d]*([\d.]+)").map_err(GetClangVersionError::RegexCompile)?;
        let captures = version_pattern
            .captures(&stdout)
            .ok_or(GetClangVersionError::VersionParse)?;
        Ok(captures
            .get(1)
            .ok_or(GetClangVersionError::VersionParse)?
            .as_str()
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use which::which;

    use super::ClangTool;
    use crate::RequestedVersion;

    const CLANG_FORMAT: ClangTool = ClangTool::ClangFormat;

    #[test]
    fn get_exe_by_version() {
        let requirement = ">=9, <22";
        let req_version = RequestedVersion::from_str(requirement).unwrap();
        let tool_exe = CLANG_FORMAT.get_exe_path(&req_version);
        println!("tool_exe: {:?}", tool_exe);
        assert!(tool_exe.is_ok_and(|val| {
            val.file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .contains(CLANG_FORMAT.as_str())
        }));
    }

    #[test]
    fn get_exe_by_default() {
        let tool_exe = CLANG_FORMAT.get_exe_path(&RequestedVersion::from_str("").unwrap());
        println!("tool_exe: {:?}", tool_exe);
        assert!(tool_exe.is_ok_and(|val| {
            val.file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .contains(CLANG_FORMAT.as_str())
        }));
    }

    #[test]
    fn get_exe_by_path() {
        static TOOL_NAME: &'static str = CLANG_FORMAT.as_str();
        let clang_version = which(TOOL_NAME).unwrap();
        let bin_path = clang_version.parent().unwrap().to_str().unwrap();
        println!("binary exe path: {bin_path}");
        let tool_exe = CLANG_FORMAT
            .get_exe_path(&RequestedVersion::from_str(bin_path).unwrap())
            .unwrap();
        println!("tool_exe: {:?}", tool_exe);
        assert!(
            tool_exe
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
                .contains(TOOL_NAME)
        );
    }

    #[test]
    fn get_exe_by_invalid_path() {
        let tool_exe =
            CLANG_FORMAT.get_exe_path(&RequestedVersion::Path(PathBuf::from("non-existent-path")));
        assert!(tool_exe.is_err());
    }
}
