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
    ) -> Result<(), PackageManagerError>;
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

pub fn try_install_package(
    tool: &ClangTool,
    version_req: &VersionReq,
) -> Result<Option<ClangVersion>, GetToolError> {
    let os_pkg_managers = get_available_package_managers();
    if os_pkg_managers.is_empty() {
        log::error!("No supported package managers found on the system.");
        return Ok(None);
    } else {
        let min_version = get_min_ver(version_req).ok_or(GetToolError::VersionMajorRequired)?;
        for mgr in os_pkg_managers {
            if !mgr.is_installed() {
                log::debug!("Skipping {mgr} package manager as it is not installed.");
                continue;
            }
            log::info!("Trying to install {tool} v{min_version} using {mgr} package manager.");
            let pkg_name = mgr.get_package_name(tool);
            if mgr.is_installed_package(&pkg_name, Some(&min_version)) {
                let path =
                    tool.get_exe_path(&RequestedVersion::Requirement(version_req.clone()))?;
                let version = tool.capture_version(&path)?;
                log::info!(
                    "Found {tool} version matching {version_req} installed via {mgr} package manager."
                );
                return Ok(Some(ClangVersion { version, path }));
            } else {
                log::info!(
                    "{mgr} package manager does not have a version of {tool} matching {version_req} installed."
                );
                match mgr.install_package(&pkg_name, Some(&min_version)) {
                    Ok(()) => {
                        log::info!(
                            "Successfully installed {tool} v{min_version} using {mgr} package manager."
                        );
                        let path = tool.get_exe_path(&RequestedVersion::SystemDefault)?;
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

fn get_min_ver(version_req: &VersionReq) -> Option<Version> {
    let mut result = None;
    for cmp in &version_req.comparators {
        if matches!(cmp.op, semver::Op::Exact | semver::Op::Caret) {
            let ver = Version {
                major: cmp.major,
                minor: cmp.minor.unwrap_or(0),
                patch: cmp.patch.unwrap_or(0),
                pre: cmp.pre.clone(),
                build: Default::default(),
            };
            if result.as_ref().is_none_or(|r| ver < *r) {
                result = Some(ver);
            }
        }
    }
    result
}
