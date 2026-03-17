#![cfg(target_os = "windows")]
use std::{fmt::Display, process::Command};

use semver::{Comparator, Version};

use crate::ClangTool;

use super::{PackageManager, PackageManagerError};

/// Supported package managers on Windows.
#[derive(Debug, Clone, Copy)]
pub enum WindowsPackageManager {
    /// Chocolatey
    Chocolatey,
    /// Winget (Windows Package Manager)
    ///
    /// Only available on Windows 10 1809 and later.
    /// Not available on Window Enterprise/Server editions.
    Winget,
    // Scoop,
}
impl Display for WindowsPackageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowsPackageManager::Chocolatey => write!(f, "Chocolatey"),
            WindowsPackageManager::Winget => write!(f, "Winget"),
            // WindowsPackageManager::Scoop => write!(f, "Scoop"),
        }
    }
}
impl WindowsPackageManager {
    fn as_str(&self) -> &'static str {
        match self {
            WindowsPackageManager::Chocolatey => "choco",
            WindowsPackageManager::Winget => "winget",
            // WindowsPackageManager::Scoop => "scoop",
        }
    }
}

impl PackageManager for WindowsPackageManager {
    fn is_installed(&self) -> bool {
        Command::new(self.as_str())
            .arg("--version")
            .output()
            .is_ok()
    }

    fn list_managers() -> Vec<impl PackageManager + Display>
    where
        Self: Sized,
    {
        vec![Self::Chocolatey, Self::Winget]
    }

    fn is_installed_package(&self, package_name: &str, version: Option<&Version>) -> bool {
        let mut cmd = Command::new(self.as_str());
        let ver_cmp = version.map(|v| Comparator {
            op: semver::Op::Caret,
            major: v.major,
            minor: Some(v.minor),
            patch: Some(v.patch),
            pre: v.pre.clone(),
        });
        match self {
            WindowsPackageManager::Chocolatey => {
                let output = cmd.arg("list").arg(package_name).output();
                if let Ok(out) = output
                    && out.status.success()
                {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    // skip line about chocolatey version
                    let lines = stdout.lines().skip(1);
                    for line in lines {
                        // packages are listed as `<name> <version>`
                        let mut l = line.split(' ');
                        if let Some(name) = l.next()
                            && name == package_name
                        {
                            // found the package, check version
                            if let Some(ver_cmp) = ver_cmp.clone() {
                                if let Some(ver_str) = l.next()
                                    && let Ok(ver) = Version::parse(ver_str)
                                {
                                    return ver_cmp.matches(&ver);
                                } else {
                                    // version not found or invalid, treat as not installed
                                    return false;
                                }
                            } else {
                                // version not specified, just check if package is listed
                                return true;
                            }
                        }
                    }
                }
                false
            }
            WindowsPackageManager::Winget => {
                let output = cmd.arg("list").arg("--id").arg(package_name).output();
                if let Ok(out) = output
                    && out.status.success()
                {
                    // skip first 2 lines of table header and divider
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let lines = stdout.lines().skip(2);
                    for line in lines {
                        // packages are listed as `<name> <id> <version> <source>`
                        let mut l = line.split(' ').skip(1);
                        if let Some(name) = l.next()
                            && name == package_name
                        {
                            // found the package, check version
                            if let Some(ver_cmp) = ver_cmp.clone() {
                                // skip id and get version
                                l.next();
                                if let Some(ver_str) = l.next()
                                    && let Ok(ver) = Version::parse(ver_str)
                                {
                                    return ver_cmp.matches(&ver);
                                } else {
                                    // version not found or invalid, treat as not installed
                                    return false;
                                }
                            } else {
                                // version not specified, just check if package is listed
                                return true;
                            }
                        }
                    }
                }
                false
            }
        }
    }

    fn get_package_name(&self, _tool: &ClangTool) -> String {
        match self {
            WindowsPackageManager::Chocolatey => "llvm".to_string(),
            WindowsPackageManager::Winget => "LLVM.LLVM".to_string(),
        }
    }

    fn install_package(
        &self,
        package_name: &str,
        version: Option<&Version>,
    ) -> Result<(), PackageManagerError> {
        let mut cmd = Command::new(self.as_str());
        match self {
            Self::Chocolatey => {
                cmd.arg("install").arg(package_name).arg("-y");
                if let Some(version) = version {
                    cmd.arg("--version").arg(version.to_string());
                }
                let output = cmd.output()?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(PackageManagerError::InstallationError {
                        manager: self.as_str().to_string(),
                        package: package_name.to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    })
                }
            }
            Self::Winget => {
                let mut cmd = Command::new("winget");
                cmd.arg("install").arg("--id").arg(package_name);
                if let Some(version) = version {
                    // winget does not allow installing an older version of a package that
                    // is already installed (with a newer version). So use `--force` to reinstall the specified version.
                    cmd.arg("--version").arg(version.to_string()).arg("--force");
                }
                let output = cmd.output().map_err(PackageManagerError::Io)?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(PackageManagerError::InstallationError {
                        manager: self.as_str().to_string(),
                        package: package_name.to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    })
                }
            }
        }
    }
}
