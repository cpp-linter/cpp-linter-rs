use std::fmt::Display;

use semver::{Version, VersionReq};

use crate::{
    ClangTool, RequestedVersion,
    version::{ClangVersion, GetToolError},
};

mod unix;
mod windows;

#[derive(Debug, thiserror::Error)]
pub enum PackageManagerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{manager} failed to install {package} package: {stderr}")]
    InstallationError {
        manager: String,
        package: String,
        stderr: String,
    },
    #[cfg(target_os = "linux")]
    #[error("Failed to add LLVM PPA repository (for `apt`): {0}")]
    LlvmPpaError(String),
    #[error("Failed parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error(transparent)]
    DownloadError(#[from] crate::downloader::DownloadError),
}

pub trait PackageManager {
    /// Checks if the package manager is installed on the system.
    fn is_installed(&self) -> bool;

    /// Returns the name of the package for the specified tool.
    ///
    /// Because different package managers handle package version differently,
    /// this only returns the package's base name. Instead, the version is
    /// handled by the [`Self::is_installed_package`] and [`Self::install_package()`].
    fn get_package_name(&self, tool: &ClangTool) -> String;

    /// Lists the supported package managers on the system.
    fn list_managers() -> Vec<impl PackageManager + Display>
    where
        Self: Sized;

    /// Checks if the specified package is installed using the package manager.
    fn is_installed_package(&self, package_name: &str, version: Option<&Version>) -> bool;

    /// Installs the specified package using the package manager.
    fn install_package(
        &self,
        package_name: &str,
        version: Option<&Version>,
    ) -> impl Future<Output = Result<(), PackageManagerError>>;
}

pub fn get_available_package_managers() -> Vec<impl PackageManager + Display> {
    let mut managers = Vec::new();
    #[cfg(target_os = "windows")]
    let possibles = windows::WindowsPackageManager::list_managers();
    #[cfg(unix)]
    let possibles = unix::UnixPackageManager::list_managers();
    for manager in possibles {
        if manager.is_installed() {
            managers.push(manager);
        }
    }
    managers
}

pub async fn try_install_package(
    tool: &ClangTool,
    version_req: &VersionReq,
    min_version: &Version,
) -> Result<Option<ClangVersion>, GetToolError> {
    let os_pkg_managers = get_available_package_managers();
    if os_pkg_managers.is_empty() {
        log::error!("No supported package managers found on the system.");
        return Ok(None);
    } else {
        for mgr in os_pkg_managers {
            log::info!("Trying to install {tool} v{min_version} using {mgr} package manager.");
            let pkg_name = mgr.get_package_name(tool);
            if mgr.is_installed_package(&pkg_name, Some(min_version))
                && let Ok(path) =
                    tool.get_exe_path(&RequestedVersion::Requirement(version_req.clone()))
                && let Ok(version) = tool.capture_version(&path)
                && version_req.matches(&version)
            {
                log::info!(
                    "Found {tool} version matching {version_req} installed via {mgr} package manager."
                );
                return Ok(Some(ClangVersion { version, path }));
            } else {
                log::info!(
                    "{mgr} package manager does not have a version of {tool} matching {version_req} installed."
                );
                match mgr.install_package(&pkg_name, Some(min_version)).await {
                    Ok(()) => {
                        log::info!(
                            "Successfully installed {tool} v{min_version} using {mgr} package manager."
                        );
                        let path =
                            tool.get_exe_path(&RequestedVersion::Requirement(version_req.clone()))?;
                        let version = tool.capture_version(&path)?;
                        if version_req.matches(&version) {
                            log::info!(
                                "Installed {tool} version {version} matches the requirement {version_req}."
                            );
                            return Ok(Some(ClangVersion { version, path }));
                        } else {
                            log::error!(
                                "Installed {tool} version {version} does not match the requirement {version_req}."
                            );
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to install {tool} v{min_version} using {mgr} package manager: {e}"
                        );
                    }
                }
            }
        }
    }
    Ok(None)
}

#[cfg(all(test, unix))]
mod tests {
    //! These tests use a fake `brew` (and fake clang tools) in an isolated `PATH`.
    //! Thus, no real package manager is ever invoked.

    use std::path::Path;

    use semver::{Version, VersionReq};
    use tempfile::TempDir;

    use super::{get_available_package_managers, try_install_package};
    use crate::{
        ClangTool,
        test_utils_unix::{enable_logs, fake_clang_tool, fake_exe, isolate_path},
        version::GetToolError,
    };

    const TOOL: ClangTool = ClangTool::ClangFormat;

    /// A fake `brew` where `brew list` exits with `list_status`,
    /// and `brew install` exits with `install_status`.
    fn fake_brew(dir: &Path, list_status: u8, install_status: u8) {
        let script = format!(
            "if [ \"$1\" = \"list\" ]; then exit {list_status}; fi\n\
            exit {install_status}"
        );
        fake_exe(dir, "brew", &script);
    }

    async fn run() -> Result<Option<crate::ClangVersion>, GetToolError> {
        let req = VersionReq::parse("17").unwrap();
        try_install_package(&TOOL, &req, &Version::new(17, 0, 0)).await
    }

    fn setup() -> TempDir {
        enable_logs();
        let tmp = TempDir::new().unwrap();
        isolate_path(tmp.path());
        tmp
    }

    #[tokio::test]
    async fn no_package_managers() {
        let _tmp = setup();
        assert!(get_available_package_managers().is_empty());
        assert!(run().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn already_installed() {
        let tmp = setup();
        fake_brew(tmp.path(), 0, 1);
        assert_eq!(get_available_package_managers().len(), 1);
        fake_clang_tool(tmp.path(), "clang-format-17", "17.0.6");
        let result = run().await.unwrap().unwrap();
        assert_eq!(result.version, Version::new(17, 0, 6));
        assert_eq!(result.path, tmp.path().join("clang-format-17"));
    }

    #[tokio::test]
    async fn install_success() {
        let tmp = setup();
        fake_brew(tmp.path(), 1, 0);
        fake_clang_tool(tmp.path(), "clang-format-17", "17.0.6");
        let result = run().await.unwrap().unwrap();
        assert_eq!(result.version, Version::new(17, 0, 6));
    }

    #[tokio::test]
    async fn install_version_mismatch() {
        let tmp = setup();
        fake_brew(tmp.path(), 0, 0);
        // the found binary does not satisfy the version requirement
        fake_clang_tool(tmp.path(), "clang-format", "16.0.0");
        assert!(run().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn install_failure() {
        let tmp = setup();
        fake_brew(tmp.path(), 1, 1);
        assert!(run().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn installed_tool_not_found() {
        let tmp = setup();
        fake_brew(tmp.path(), 1, 0);
        let err = run().await.unwrap_err();
        assert!(matches!(err, GetToolError::GetClangPathError(_)));
    }

    #[tokio::test]
    async fn installed_tool_no_version() {
        let tmp = setup();
        fake_brew(tmp.path(), 1, 0);
        fake_exe(tmp.path(), "clang-format-17", "echo \"no version info\"");
        let err = run().await.unwrap_err();
        assert!(matches!(err, GetToolError::GetClangVersion(_)));
    }
}
