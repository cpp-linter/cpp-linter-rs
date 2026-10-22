#![cfg(target_os = "windows")]
use std::{
    fmt::Display,
    process::{Command, Output},
};

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

    /// Is the `package_name` listed in the `stdout` of this package manager's `list` command?
    ///
    /// If a `version` is given, then the listed version must be compatible with it
    /// (as in `^version`). A missing or invalid listed version is treated as not installed.
    fn is_listed(&self, stdout: &str, package_name: &str, version: Option<&Version>) -> bool {
        let (header_lines, name_column) = match self {
            // `choco list` starts with a line about chocolatey's version,
            // then packages are listed as `<name> <version>`
            WindowsPackageManager::Chocolatey => (1, 0),
            // `winget list` starts with a table header and divider,
            // then packages are listed as `<name> <id> <version> <source>`
            WindowsPackageManager::Winget => (2, 1),
        };
        for line in stdout.lines().skip(header_lines) {
            let mut columns = line.split_ascii_whitespace().skip(name_column);
            if columns.next() != Some(package_name) {
                continue;
            }
            // found the package, check version
            let Some(version) = version else {
                // version not specified, just check if package is listed
                return true;
            };
            let ver_cmp = Comparator {
                op: semver::Op::Caret,
                major: version.major,
                minor: Some(version.minor),
                patch: Some(version.patch),
                pre: version.pre.clone(),
            };
            return columns
                .next()
                .and_then(|ver_str| Version::parse(ver_str).ok())
                .is_some_and(|ver| ver_cmp.matches(&ver));
        }
        false
    }

    /// Converts the `output` of this package manager's `install` command into a [`Result`].
    fn check_install(
        &self,
        package_name: &str,
        output: &Output,
    ) -> Result<(), PackageManagerError> {
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

impl PackageManager for WindowsPackageManager {
    fn is_installed(&self) -> bool {
        Command::new(self.as_str())
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn list_managers() -> Vec<impl PackageManager + Display>
    where
        Self: Sized,
    {
        vec![Self::Chocolatey, Self::Winget]
    }

    fn is_installed_package(&self, package_name: &str, version: Option<&Version>) -> bool {
        let mut cmd = Command::new(self.as_str());
        match self {
            WindowsPackageManager::Chocolatey => cmd.arg("list").arg(package_name),
            WindowsPackageManager::Winget => cmd.arg("list").arg("--id").arg(package_name),
        };
        match cmd.output() {
            Ok(out) if out.status.success() => {
                self.is_listed(&String::from_utf8_lossy(&out.stdout), package_name, version)
            }
            _ => false,
        }
    }

    fn get_package_name(&self, _tool: &ClangTool) -> String {
        match self {
            WindowsPackageManager::Chocolatey => "llvm".to_string(),
            WindowsPackageManager::Winget => "LLVM.LLVM".to_string(),
        }
    }

    async fn install_package(
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
            }
            Self::Winget => {
                cmd.arg("install").arg("--id").arg(package_name);
                if let Some(version) = version {
                    // winget does not allow installing an older version of a package that
                    // is already installed (with a newer version). So use `--force` to reinstall the specified version.
                    cmd.arg("--version").arg(version.to_string()).arg("--force");
                }
            }
        }
        let output = cmd.output()?;
        self.check_install(package_name, &output)
    }
}

#[cfg(test)]
mod tests {
    use std::{os::windows::process::ExitStatusExt, process::ExitStatus};

    use semver::Version;

    use super::{Output, PackageManagerError, WindowsPackageManager};

    const CHOCO_LIST: &str = "Chocolatey v2.4.3\n\
        llvm 18.1.8\n\
        llvm-nightly not-a-version\n\
        2 packages installed.\n";

    const WINGET_LIST: &str = "Name Id        Version Source\n\
        -----------------------------\n\
        LLVM LLVM.LLVM 18.1.8  winget\n\
        Broken LLVM.Broken\n";

    #[test]
    fn choco_listed() {
        let choco = WindowsPackageManager::Chocolatey;
        assert!(choco.is_listed(CHOCO_LIST, "llvm", None));
        assert!(choco.is_listed(CHOCO_LIST, "llvm", Some(&Version::new(18, 1, 0))));
        assert!(!choco.is_listed(CHOCO_LIST, "llvm", Some(&Version::new(19, 1, 0))));
        // the listed version is not valid semver
        assert!(choco.is_listed(CHOCO_LIST, "llvm-nightly", None));
        assert!(!choco.is_listed(CHOCO_LIST, "llvm-nightly", Some(&Version::new(18, 1, 0))));
        // the first line is about chocolatey itself, not a package
        assert!(!choco.is_listed(CHOCO_LIST, "Chocolatey", None));
        assert!(!choco.is_listed(CHOCO_LIST, "cmake", None));
    }

    #[test]
    fn winget_listed() {
        let winget = WindowsPackageManager::Winget;
        assert!(winget.is_listed(WINGET_LIST, "LLVM.LLVM", None));
        assert!(winget.is_listed(WINGET_LIST, "LLVM.LLVM", Some(&Version::new(18, 1, 0))));
        assert!(!winget.is_listed(WINGET_LIST, "LLVM.LLVM", Some(&Version::new(17, 0, 6))));
        // no version is listed
        assert!(winget.is_listed(WINGET_LIST, "LLVM.Broken", None));
        assert!(!winget.is_listed(WINGET_LIST, "LLVM.Broken", Some(&Version::new(18, 1, 0))));
        // the table header and divider are not packages
        assert!(!winget.is_listed(WINGET_LIST, "Id", None));
        // the name column is not the ID
        assert!(!winget.is_listed(WINGET_LIST, "LLVM", None));
    }

    fn output(code: u32, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code),
            stdout: vec![],
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn install_result() {
        for mgr in [
            WindowsPackageManager::Chocolatey,
            WindowsPackageManager::Winget,
        ] {
            assert!(mgr.check_install("llvm", &output(0, "")).is_ok());
            let err = mgr
                .check_install("llvm", &output(1, "no such package"))
                .unwrap_err();
            let PackageManagerError::InstallationError {
                manager,
                package,
                stderr,
            } = err
            else {
                panic!("unexpected error: {err}");
            };
            assert_eq!(manager, mgr.as_str());
            assert_eq!(package, "llvm");
            assert_eq!(stderr, "no such package");
        }
    }
}
