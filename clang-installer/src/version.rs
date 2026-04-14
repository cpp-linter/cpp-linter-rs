use std::{path::PathBuf, str::FromStr};

use crate::{
    Cacher, ClangTool, PyPiDownloadError, PyPiDownloader,
    downloader::{native_packages::try_install_package, static_dist::StaticDistDownloader},
    tool::{GetClangPathError, GetClangVersionError},
    utils::normalize_path,
};
use semver::{Version, VersionReq};

#[derive(Debug, Clone)]
pub struct ClangVersion {
    pub version: Version,
    pub path: PathBuf,
}

/// An enumeration of the possible requested versions of the clang tool binary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequestedVersion {
    /// A specific path to the clang tool binary.
    Path(PathBuf),

    /// Whatever the system default uses (if any).
    #[default]
    SystemDefault,

    /// A specific version requirement for the clang tool binary.
    ///
    /// For example, `=12.0.1`, `>=10.0.0, <13.0.0`.
    Requirement(VersionReq),

    /// A sentinel when no value is given.
    ///
    /// This is used internally to differentiate when the user intended
    /// to invoke the `version` CLI subcommand instead.
    NoValue,
}

/// Errors that occur when getting the clang tool binary.
#[derive(Debug, thiserror::Error)]
pub enum GetToolError {
    /// No executable found for the specified version requirement.
    #[error("No executable found for the specified version requirement")]
    NotFound,

    /// Failed to parse the version string.
    #[error("Failed to parse version: {0}")]
    VersionParseError(String),

    /// The version requirement does satisfy any known/supported clang version
    #[error("The version requirement does satisfy any known/supported clang version")]
    UnsupportedVersion,

    /// Binary executable in cache has no parent directory.
    #[error("Binary executable in cache has no parent directory")]
    ExecutablePathNoParent,

    /// Failed to capture the clang version from `--version` output.
    #[error("Failed to capture the clang version from `--version` output: {0}")]
    GetClangVersion(#[from] GetClangVersionError),

    /// Failed to get the clang executable path.
    #[error("Failed to get the clang executable path: {0}")]
    GetClangPathError(#[from] GetClangPathError),

    /// Failed to create symlink for the downloaded binary.
    #[error("Failed to create symlink for the downloaded binary: {0}")]
    SymlinkError(std::io::Error),

    /// Failed to download tool from PyPi.
    #[error("Failed to download tool from PyPi: {0}")]
    PyPiDownloadError(#[from] PyPiDownloadError),
}

impl RequestedVersion {
    pub async fn eval_tool(
        &self,
        tool: &ClangTool,
        overwrite_symlink: bool,
        directory: Option<&PathBuf>,
    ) -> Result<Option<ClangVersion>, GetToolError> {
        match self {
            RequestedVersion::Path(_) => {
                let exec_path = tool.get_exe_path(self)?;
                let version = tool.capture_version(&exec_path)?;
                log::info!(
                    "Found {tool} version {version} at path: {:?}",
                    exec_path.to_string_lossy()
                );
                Ok(Some(ClangVersion {
                    version,
                    path: exec_path,
                }))
            }
            RequestedVersion::SystemDefault => {
                let path = tool.get_exe_path(&Self::SystemDefault)?;
                let version = tool.capture_version(&path)?;
                log::info!(
                    "Found {tool} version {version} at path: {:?}",
                    path.to_string_lossy()
                );
                Ok(Some(ClangVersion { version, path }))
            }
            RequestedVersion::Requirement(version_req) => {
                // check default available version first (if any)
                if let Ok(path) = tool.get_exe_path(&Self::Requirement(version_req.clone())) {
                    let version = tool.capture_version(&path)?;
                    if version_req.matches(&version) {
                        log::info!(
                            "Found {tool} version {version} at path: {:?}",
                            path.to_string_lossy()
                        );
                        return Ok(Some(ClangVersion { version, path }));
                    }
                }

                // check if cache has a suitable version
                let bin_ext = if cfg!(windows) { ".exe" } else { "" };
                let min_ver = get_min_ver(version_req).ok_or(GetToolError::UnsupportedVersion)?;
                let cached_bin = StaticDistDownloader::get_cache_dir()
                    .join("bin")
                    .join(format!("{tool}-{min_ver}{bin_ext}"));
                if cached_bin.exists() {
                    let version = tool.capture_version(&cached_bin)?;
                    if version_req.matches(&version) {
                        log::info!(
                            "Found {tool} version {version} in cache at path: {:?}",
                            cached_bin.to_string_lossy()
                        );
                        return Ok(Some(ClangVersion {
                            version,
                            path: cached_bin,
                        }));
                    }
                }

                // try to download a suitable version
                let bin = match PyPiDownloader::download_tool(tool, version_req, directory).await {
                    Ok(bin) => bin,
                    Err(e) => {
                        log::error!("Failed to download {tool} {version_req} from PyPi: {e}");
                        if let Some(result) =
                            try_install_package(tool, version_req, &min_ver).await?
                        {
                            return Ok(Some(result));
                        }
                        log::info!("Falling back to downloading {tool} static binaries.");
                        match StaticDistDownloader::download_tool(tool, version_req, directory)
                            .await
                        {
                            Ok(bin) => bin,
                            Err(e) => {
                                log::error!(
                                    "Failed to download {tool} from static distribution: {e}"
                                );
                                return Err(GetToolError::NotFound);
                            }
                        }
                    }
                };

                // create a symlink
                let bin_dir = bin.parent().ok_or(GetToolError::ExecutablePathNoParent)?;
                let symlink_path = bin_dir.join(format!("{tool}{bin_ext}"));
                tool.symlink_bin(&bin, &symlink_path, overwrite_symlink)
                    .map_err(GetToolError::SymlinkError)?;
                let version = tool.capture_version(&bin)?;
                Ok(Some(ClangVersion { version, path: bin }))
            }
            RequestedVersion::NoValue => {
                log::info!(
                    "{} version: {}",
                    option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
                    env!("CARGO_PKG_VERSION")
                );
                Ok(None)
            }
        }
    }
}

pub fn get_min_ver(version_req: &VersionReq) -> Option<Version> {
    let mut result = None;
    let supported_version_range = StaticDistDownloader::get_major_version_range();
    for major in supported_version_range.rev() {
        let ver = Version::new(major as u64, 0, 0);
        if version_req.matches(&ver) {
            result = Some(ver);
        }
    }
    result
}

/// Represents an error that occurred while parsing a requested version.
#[derive(Debug, thiserror::Error)]
pub enum RequestedVersionParsingError {
    /// The specified version is not a proper version requirement or a valid path.
    #[error("The specified version is not a proper version requirement or a valid path: {0}")]
    InvalidInput(String),

    /// Unknown parent directory of the given file path for `--version`.
    #[error("Unknown parent directory of the given file path for `--version`: {0}")]
    InvalidPath(String),

    /// Failed to canonicalize path '{0}'.
    #[error("Failed to canonicalize path '{0}': {1:?}")]
    NonCanonicalPath(String, std::io::Error),
}

impl FromStr for RequestedVersion {
    type Err = RequestedVersionParsingError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            Ok(Self::SystemDefault)
        } else if input == "CPP-LINTER-VERSION" {
            Ok(Self::NoValue)
        } else if let Ok(req) = VersionReq::parse(input) {
            Ok(Self::Requirement(req))
        } else {
            let path = PathBuf::from(input);
            if !path.exists() {
                return Err(RequestedVersionParsingError::InvalidInput(
                    input.to_string(),
                ));
            }
            let path = if !path.is_dir() {
                path.parent()
                    .ok_or(RequestedVersionParsingError::InvalidPath(input.to_string()))?
                    .to_path_buf()
            } else {
                path
            };
            let path = match path.canonicalize() {
                Ok(p) => Ok(normalize_path(&p)),
                Err(e) => Err(RequestedVersionParsingError::NonCanonicalPath(
                    input.to_string(),
                    e,
                )),
            }?;
            Ok(Self::Path(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use semver::VersionReq;
    use tempfile::TempDir;

    use super::RequestedVersion;
    use crate::{ClangTool, utils::normalize_path};

    // See also crate::tool::tests module for other `RequestedVersion::from_str()` tests.

    #[test]
    fn validate_version_path() {
        let this_path_str = "src/version.rs";
        let this_path = PathBuf::from(this_path_str);
        let this_canonical = this_path.canonicalize().unwrap();
        let parent = this_canonical.parent().unwrap();
        let expected = normalize_path(parent);
        let req_ver = RequestedVersion::from_str(this_path_str).unwrap();
        if let RequestedVersion::Path(parsed) = req_ver {
            assert_eq!(&parsed, &expected);
        }

        assert!(RequestedVersion::from_str("file.rs").is_err());
    }

    #[test]
    fn validate_version_exact() {
        let req_ver = RequestedVersion::from_str("12").unwrap();
        if let RequestedVersion::Requirement(req) = req_ver {
            assert_eq!(req.to_string(), "^12");
        }
    }

    #[tokio::test]
    async fn eval_no_value() {
        let result = RequestedVersion::NoValue
            .eval_tool(&ClangTool::ClangFormat, false, None)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    /// The idea for this test is to make sure the desired clang-tool is downloaded and
    /// the download path can be used to identify the location of the clang tool.
    #[tokio::test]
    async fn eval_download_path() {
        let tmp_cache_dir = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("CPP_LINTER_CACHE", tmp_cache_dir.path());
        }
        let tool = ClangTool::ClangFormat;
        // for this test we should use the oldest supported clang version
        // because that would be most likely to require downloading.
        let version_req =
            VersionReq::parse(option_env!("MIN_CLANG_TOOLS_VERSION").unwrap_or("11")).unwrap();
        let downloaded_clang = RequestedVersion::Requirement(version_req.clone())
            .eval_tool(&tool, false, Some(&PathBuf::from(tmp_cache_dir.path())))
            .await
            .unwrap()
            .unwrap();
        println!("Downloaded clang-format: {downloaded_clang:?}");
        let req_ver = RequestedVersion::Path(downloaded_clang.path.parent().unwrap().to_owned());
        let result = req_ver
            .eval_tool(&tool, false, None)
            .await
            .unwrap()
            .unwrap();
        println!("Evaluated clang-format from path: {result:?}");
        assert!(
            version_req.matches(&result.version),
            "Expected {downloaded_clang:?} does not match {result:?}",
        );
        assert_eq!(result.version, downloaded_clang.version);
        assert_eq!(result.path.parent(), downloaded_clang.path.parent());
    }

    /// WARNING: This test should only run in CI.
    /// It is designed to use the system's package manager to install clang-tidy.
    /// If successful, clang-tidy will be installed globally, which may be undesirable.
    #[tokio::test]
    async fn eval_version() {
        let clang_version = option_env!("CLANG_VERSION").unwrap_or("12.0.1");
        for tool in [ClangTool::ClangFormat, ClangTool::ClangTidy] {
            let version_req = VersionReq::parse(clang_version).unwrap();
            println!("Installing {tool} with version requirement: {version_req}");
            let clang_path = RequestedVersion::Requirement(version_req.clone())
                .eval_tool(&tool, false, None)
                .await
                .unwrap()
                .unwrap();
            eprintln!("Using {clang_path:?}");
            // assert!(version_req.matches(&clang_path.version));
        }
    }
}
