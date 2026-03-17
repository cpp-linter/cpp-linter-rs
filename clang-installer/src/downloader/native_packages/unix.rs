#![cfg(unix)]
use std::{fmt::Display, process::Command};

use semver::Version;

use crate::ClangTool;

use super::{PackageManager, PackageManagerError};

/// Supported package managers on Linux and MacOS.
pub enum UnixPackageManager {
    /// Debian-based distributions (Ubuntu, etc.)
    #[cfg(target_os = "linux")]
    Apt,
    /// RedHat-based distributions (Fedora, etc.)
    #[cfg(target_os = "linux")]
    Dnf,
    /// Arch-based distributions (Arch, Manjaro, etc.)
    #[cfg(target_os = "linux")]
    PacMan,
    /// Homebrew (Linux or MacOS)
    Homebrew,
}

impl Display for UnixPackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt => write!(f, "Apt"),
            #[cfg(target_os = "linux")]
            UnixPackageManager::Dnf => write!(f, "Dnf"),
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => write!(f, "PacMan"),
            UnixPackageManager::Homebrew => write!(f, "Homebrew"),
        }
    }
}

impl UnixPackageManager {
    fn as_str(&self) -> &'static str {
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt => "apt",
            #[cfg(target_os = "linux")]
            UnixPackageManager::Dnf => "dnf",
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => "pacman",
            UnixPackageManager::Homebrew => "brew",
        }
    }

    fn pkg_name_with_version(&self, package_name: &str, version: Option<&Version>) -> String {
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt | UnixPackageManager::Dnf => {
                if let Some(ver) = version {
                    format!("{package_name}-{}", ver.major)
                } else {
                    package_name.to_string()
                }
            }
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => {
                if let Some(ver) = version {
                    format!("{package_name}{}", ver.major)
                } else {
                    package_name.to_string()
                }
            }
            UnixPackageManager::Homebrew => {
                if let Some(ver) = version {
                    format!("{package_name}@{}", ver.major)
                } else {
                    package_name.to_string()
                }
            }
        }
    }
}

impl PackageManager for UnixPackageManager {
    fn is_installed(&self) -> bool {
        which::which(self.as_str()).is_ok()
    }

    fn list_managers() -> Vec<impl PackageManager + Display>
    where
        Self: Sized,
    {
        #[cfg(target_os = "linux")]
        {
            vec![Self::Apt, Self::Dnf, Self::PacMan, Self::Homebrew]
        }
        #[cfg(not(target_os = "linux"))]
        {
            vec![Self::Homebrew]
        }
    }

    #[cfg_attr(
        not(target_os = "linux"),
        allow(
            unused_variables,
            reason = "`tool` param only used for linux package managers"
        )
    )]
    fn get_package_name(&self, tool: &ClangTool) -> String {
        match self {
            #[cfg(target_os = "linux")]
            Self::Apt => tool.to_string(),
            #[cfg(target_os = "linux")]
            Self::Dnf | Self::PacMan => "clang".to_string(),
            Self::Homebrew => "llvm".to_string(),
        }
    }

    fn install_package(
        &self,
        package_name: &str,
        version: Option<&Version>,
    ) -> Result<(), PackageManagerError> {
        let mut args = vec![];
        let package_id = self.pkg_name_with_version(package_name, version);
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt | UnixPackageManager::Dnf => {
                args.extend(["install", "-y"]);
            }
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => {
                args.extend(["-S", "-y"]);
            }
            UnixPackageManager::Homebrew => {
                args.push("install");
            }
        }
        let output = Command::new(self.as_str())
            .args(args)
            .arg(package_id.as_str())
            .output()?;
        if output.status.success() {
            Ok(())
        } else {
            Err(PackageManagerError::InstallationError {
                manager: self.as_str().to_string(),
                package: package_id,
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }

    fn is_installed_package(&self, package_name: &str, version: Option<&Version>) -> bool {
        let mut cmd = Command::new(self.as_str());
        let package_id = self.pkg_name_with_version(package_name, version);
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt => {
                let output = cmd
                    .args(["list", "--installed", package_id.as_str()])
                    .output();
                output.is_ok_and(|out| out.status.success())
            }
            #[cfg(target_os = "linux")]
            UnixPackageManager::Dnf => {
                let output = cmd.arg("list").arg(package_id.as_str()).output();
                output.is_ok_and(|out| out.status.success())
            }
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => {
                let output = cmd.arg("-Qs").arg(package_id.as_str()).output();
                output.is_ok_and(|out| out.status.success())
            }
            UnixPackageManager::Homebrew => {
                let output = cmd
                    .arg("list")
                    .arg("--versions")
                    .arg(package_id.as_str())
                    .output();
                output.is_ok_and(|out| out.status.success())
            }
        }
    }
}
