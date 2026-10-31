use std::{path::PathBuf, str::FromStr};

use crate::{
    Cacher, ClangTool, PyPiDownloadError, PyPiDownloader,
    downloader::{
        native_packages::try_install_package, resource_headers, static_dist::StaticDistDownloader,
    },
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

    /// The version requirement does not satisfy any known/supported clang version
    #[error("The version requirement does not satisfy any known/supported clang version")]
    UnsupportedVersion,

    /// Binary executable in cache has no parent directory.
    #[error("Binary executable in cache has no parent directory")]
    ExecutablePathNoParent,

    /// Failed to capture the clang version from `--version` output.
    #[error(transparent)]
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
        allow_system_package_manager: bool,
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
                // Downloaded binaries are named with only the major version.
                let cached_bin = StaticDistDownloader::get_cache_dir()
                    .join("bin")
                    .join(format!("{tool}-{}{bin_ext}", min_ver.major));
                if cached_bin.exists() {
                    let version = tool.capture_version(&cached_bin)?;
                    // A cached clang-tidy without resource headers is downloaded again,
                    // so the headers get installed.
                    if version_req.matches(&version)
                        && (!tool.needs_resource_headers()
                            || resource_headers::has_resource_headers(&cached_bin, &version))
                    {
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

                // try a system package manager first (if allowed)
                if allow_system_package_manager {
                    match try_install_package(tool, version_req, &min_ver).await {
                        Ok(Some(result)) => return Ok(Some(result)),
                        Ok(None) => {}
                        Err(e) => log::error!(
                            "Failed to install {tool} {version_req} using a system package manager: {e}"
                        ),
                    }
                }

                // then try to download a suitable version from PyPI or the static binaries
                let bin = match PyPiDownloader::download_tool(tool, version_req, directory).await {
                    Ok(bin) => bin,
                    Err(e) => {
                        log::error!("Failed to download {tool} {version_req} from PyPi: {e}");
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
    use std::{env, path::PathBuf, str::FromStr};

    use semver::VersionReq;
    use tempfile::TempDir;

    use super::{RequestedVersion, RequestedVersionParsingError};
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
        assert_eq!(req_ver, RequestedVersion::Path(expected));

        assert!(matches!(
            RequestedVersion::from_str("file.rs").unwrap_err(),
            RequestedVersionParsingError::InvalidInput(_)
        ));
    }

    #[test]
    fn validate_version_dir() {
        let req_ver = RequestedVersion::from_str("src").unwrap();
        let expected = normalize_path(&PathBuf::from("src").canonicalize().unwrap());
        assert_eq!(req_ver, RequestedVersion::Path(expected));
    }

    #[test]
    fn validate_version_non_canonical_path() {
        // A file in the working directory has an empty parent path, which cannot be canonicalized.
        let err = RequestedVersion::from_str("Cargo.toml").unwrap_err();
        println!("{err}");
        assert!(matches!(
            err,
            RequestedVersionParsingError::NonCanonicalPath(..)
        ));
    }

    #[test]
    fn validate_version_no_value() {
        assert_eq!(
            RequestedVersion::from_str("CPP-LINTER-VERSION").unwrap(),
            RequestedVersion::NoValue
        );
    }

    #[test]
    fn validate_version_exact() {
        let req_ver = RequestedVersion::from_str("12").unwrap();
        assert_eq!(
            req_ver,
            RequestedVersion::Requirement(VersionReq::parse("^12").unwrap())
        );
    }

    #[tokio::test]
    async fn eval_no_value() {
        log::set_max_level(log::LevelFilter::Trace);
        let result = RequestedVersion::NoValue
            .eval_tool(&ClangTool::ClangFormat, false, None, false)
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
            VersionReq::parse(option_env!("MIN_CLANG_TOOLS_VERSION").unwrap_or("16")).unwrap();
        let downloaded_clang = RequestedVersion::Requirement(version_req.clone())
            .eval_tool(
                &tool,
                false,
                Some(&PathBuf::from(tmp_cache_dir.path())),
                false,
            )
            .await
            .unwrap()
            .unwrap();
        println!("Downloaded clang-format: {downloaded_clang:?}");
        let req_ver = RequestedVersion::Path(downloaded_clang.path.parent().unwrap().to_owned());
        let result = req_ver
            .eval_tool(&tool, false, None, false)
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
        let clang_version = option_env!("CLANG_VERSION").unwrap_or("16");
        for tool in [ClangTool::ClangFormat, ClangTool::ClangTidy] {
            let version_req = VersionReq::parse(clang_version).unwrap();
            println!("Installing {tool} with version requirement: {version_req}");
            let clang_path = RequestedVersion::Requirement(version_req.clone())
                .eval_tool(
                    &tool,
                    false,
                    None,
                    env::var("CI")
                        .is_ok_and(|v| ["true", "on", "1"].contains(&v.to_lowercase().as_str())),
                )
                .await
                .unwrap()
                .unwrap();
            eprintln!("Using {clang_path:?}");
            // assert!(version_req.matches(&clang_path.version));
        }
    }

    /// These tests use fake clang tools (and package managers) in an isolated `PATH`
    /// and an isolated (pre-seeded) cache. Thus, no network access is needed.
    #[cfg(unix)]
    mod fake_tools {
        use std::{fs, path::Path};

        use semver::{Version, VersionReq};
        use tempfile::TempDir;

        use crate::{
            ClangTool, GetToolError, RequestedVersion, StaticDistDownloader,
            test_utils_unix::{
                enable_logs, fake_clang_tool, fake_exe, isolate_path, set_cache_dir,
            },
        };

        const TOOL: ClangTool = ClangTool::ClangFormat;

        struct TestEnv {
            /// The only directory in `PATH`.
            bin: TempDir,
            /// The cache directory.
            cache: TempDir,
            /// The directory to "download" binaries into.
            out: TempDir,
        }

        impl TestEnv {
            fn new() -> Self {
                enable_logs();
                let env = Self {
                    bin: TempDir::new().unwrap(),
                    cache: TempDir::new().unwrap(),
                    out: TempDir::new().unwrap(),
                };
                isolate_path(env.bin.path());
                set_cache_dir(env.cache.path());
                env
            }

            /// Seed the PyPI cache with info that has no releases (so download from PyPI fails).
            fn seed_empty_pypi(&self) {
                let pypi = self.cache.path().join("pypi");
                fs::create_dir_all(&pypi).unwrap();
                fs::write(
                    pypi.join("clang-format_pypi.json"),
                    serde_json::json!({"releases": {}}).to_string(),
                )
                .unwrap();
            }

            /// Seed the static binary (in the [`Self::out`] dir) and the cached SHA512SUMS file.
            ///
            /// If `valid` is false, then the checksum will not match the binary.
            fn seed_static_dist(&self, valid: bool) {
                use sha2::{Digest, Sha512};

                let bin = fake_clang_tool(self.out.path(), "clang-format-17", "17.0.6");
                let hash = if valid {
                    Sha512::digest(fs::read(bin).unwrap())
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                } else {
                    "deadbeef".to_string()
                };
                // list checksums for all supported (unix) platforms
                let mut checksums = String::new();
                for platform in ["linux", "macos"] {
                    for arch in ["amd64", "arm64"] {
                        checksums.push_str(&format!("{hash}  clang-format-17_{platform}-{arch}\n"));
                    }
                }
                let (_, release_tag) =
                    StaticDistDownloader::find_suitable_version(&VersionReq::parse("17").unwrap())
                        .unwrap();
                let sums = StaticDistDownloader::sha512_cache_path(release_tag);
                fs::create_dir_all(sums.parent().unwrap()).unwrap();
                fs::write(sums, checksums).unwrap();
            }

            async fn eval(
                &self,
                requested: RequestedVersion,
                allow_pkg_mgr: bool,
            ) -> Result<Option<crate::ClangVersion>, GetToolError> {
                let out = self.out.path().to_path_buf();
                requested
                    .eval_tool(&TOOL, false, Some(&out), allow_pkg_mgr)
                    .await
            }
        }

        fn req(spec: &str) -> RequestedVersion {
            RequestedVersion::Requirement(VersionReq::parse(spec).unwrap())
        }

        fn assert_version(path: &Path, result: crate::ClangVersion, version: &str) {
            assert_eq!(result.path, path);
            assert_eq!(result.version, Version::parse(version).unwrap());
        }

        #[tokio::test]
        async fn eval_path() {
            let env = TestEnv::new();
            let exe = fake_clang_tool(env.bin.path(), "clang-format", "17.0.6");
            let result = env
                .eval(RequestedVersion::Path(env.bin.path().to_path_buf()), false)
                .await
                .unwrap()
                .unwrap();
            assert_version(&exe, result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_system_default() {
            let env = TestEnv::new();
            let exe = fake_clang_tool(env.bin.path(), "clang-format", "17.0.6");
            let result = env
                .eval(RequestedVersion::SystemDefault, false)
                .await
                .unwrap()
                .unwrap();
            assert_version(&exe, result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_installed_requirement() {
            let env = TestEnv::new();
            let exe = fake_clang_tool(env.bin.path(), "clang-format-17", "17.0.6");
            let result = env.eval(req("17"), false).await.unwrap().unwrap();
            assert_version(&exe, result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_unsupported_requirement() {
            let env = TestEnv::new();
            let err = env.eval(req("=8"), false).await.unwrap_err();
            assert!(matches!(err, GetToolError::UnsupportedVersion));
        }

        #[tokio::test]
        async fn eval_cached_bin() {
            let env = TestEnv::new();
            // installed version does not satisfy the requirement
            fake_clang_tool(env.bin.path(), "clang-format", "16.0.0");
            let cached_bin_dir = env.cache.path().join("bin");
            fs::create_dir_all(&cached_bin_dir).unwrap();
            let cached = fake_clang_tool(&cached_bin_dir, "clang-format-17", "17.0.6");
            let result = env.eval(req("17"), false).await.unwrap().unwrap();
            assert_version(&cached, result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_cached_tidy_needs_resource_headers() {
            let env = TestEnv::new();
            let cached_bin_dir = env.cache.path().join("bin");
            fs::create_dir_all(&cached_bin_dir).unwrap();
            let cached = fake_clang_tool(&cached_bin_dir, "clang-tidy-17", "17.0.6");
            let requested = req("17");
            let eval = || requested.eval_tool(&ClangTool::ClangTidy, false, None, false);
            // A cached clang-tidy without resource headers is not used.
            // Instead, it is downloaded again, which fails here because
            // PyPI has no releases and the static binary's checksum does not match.
            let pypi = env.cache.path().join("pypi");
            fs::create_dir_all(&pypi).unwrap();
            let no_releases = serde_json::json!({"releases": {}}).to_string();
            fs::write(pypi.join("clang-tidy_pypi.json"), no_releases).unwrap();
            let (_, release_tag) =
                StaticDistDownloader::find_suitable_version(&VersionReq::parse("17").unwrap())
                    .unwrap();
            let sums = StaticDistDownloader::sha512_cache_path(release_tag);
            fs::create_dir_all(sums.parent().unwrap()).unwrap();
            fs::write(sums, "deadbeef  clang-tidy-17_linux-amd64\n").unwrap();
            let err = eval().await.unwrap_err();
            assert!(matches!(err, GetToolError::NotFound));

            let include = env.cache.path().join("lib/clang/17/include");
            fs::create_dir_all(&include).unwrap();
            fs::write(include.join("stddef.h"), "").unwrap();
            let result = eval().await.unwrap().unwrap();
            assert_version(&cached, result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_static_dist_fallback() {
            let env = TestEnv::new();
            // cached version does not satisfy the requirement
            let cached_bin_dir = env.cache.path().join("bin");
            fs::create_dir_all(&cached_bin_dir).unwrap();
            fake_clang_tool(&cached_bin_dir, "clang-format-17", "16.0.0");
            env.seed_empty_pypi();
            env.seed_static_dist(true);
            // no package managers are available.
            let result = env.eval(req("17"), true).await.unwrap().unwrap();
            assert_version(&env.out.path().join("clang-format-17"), result, "17.0.6");
            let symlink = env.out.path().join("clang-format");
            assert_eq!(
                fs::read_link(symlink).unwrap(),
                env.out.path().join("clang-format-17")
            );
        }

        #[tokio::test]
        async fn eval_static_dist_failure() {
            let env = TestEnv::new();
            env.seed_empty_pypi();
            env.seed_static_dist(false);
            let err = env.eval(req("17"), false).await.unwrap_err();
            assert!(matches!(err, GetToolError::NotFound));
        }

        #[tokio::test]
        async fn eval_package_manager_error_falls_back() {
            let env = TestEnv::new();
            env.seed_empty_pypi();
            env.seed_static_dist(true);
            // A fake `brew` that "installs" clang-format, but no clang-format executable appears.
            fake_exe(
                env.bin.path(),
                "brew",
                "if [ \"$1\" = \"list\" ]; then exit 1; fi",
            );
            let result = env.eval(req("17"), true).await.unwrap().unwrap();
            assert_version(&env.out.path().join("clang-format-17"), result, "17.0.6");
        }

        #[tokio::test]
        async fn eval_package_manager() {
            let env = TestEnv::new();
            // PyPI is not seeded: the package manager is tried first,
            // so nothing is downloaded from PyPI.
            let marker = env.bin.path().join("installed");
            let marker = marker.to_string_lossy();
            // A fake `brew` that "installs" clang-format v17 (by creating a marker file).
            fake_exe(
                env.bin.path(),
                "brew",
                &format!("if [ \"$1\" = \"list\" ]; then exit 1; fi\n: > \"{marker}\""),
            );
            // A fake clang-format that reports v17 only after being "installed".
            let exe = fake_exe(
                env.bin.path(),
                "clang-format-17",
                &format!(
                    "if [ -f \"{marker}\" ]; then echo \"version 17.0.6\"; \
                    else echo \"version 16.0.0\"; fi"
                ),
            );
            let result = env.eval(req("17"), true).await.unwrap().unwrap();
            assert_version(&exe, result, "17.0.6");
        }
    }
}
