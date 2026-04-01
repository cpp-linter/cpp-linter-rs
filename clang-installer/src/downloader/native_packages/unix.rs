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

    fn has_sudo() -> bool {
        which::which("sudo").is_ok()
    }

    fn pkg_name_with_version(&self, package_name: &str, version: Option<&Version>) -> String {
        match self {
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt | UnixPackageManager::Dnf => {
                version.map(|ver| format!("{package_name}-{}", ver.major))
            }
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan => version.map(|ver| format!("{package_name}{}", ver.major)),
            UnixPackageManager::Homebrew => {
                version.map(|ver| format!("{package_name}@{}", ver.major))
            }
        }
        .unwrap_or(package_name.to_string())
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

    async fn install_package(
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
                // spell-checker: disable-next-line
                args.extend(["-S", "--noconfirm"]);
            }
            UnixPackageManager::Homebrew => {
                args.push("install");
            }
        }
        let output = if Self::has_sudo() && !matches!(self, UnixPackageManager::Homebrew) {
            Command::new("sudo")
                .arg(self.as_str())
                .args(args)
                .arg(package_id.as_str())
                .output()?
        } else {
            Command::new(self.as_str())
                .args(args)
                .arg(package_id.as_str())
                .output()?
        };
        if output.status.success() {
            Ok(())
        } else {
            #[cfg(target_os = "linux")]
            if matches!(self, UnixPackageManager::Apt)
                && let Some(version) = version
            {
                log::info!(
                    "trying to install from official LLVM PPA repository (for Debian-based `apt` package manager)"
                );
                return llvm_apt_install::install_llvm_via_apt(
                    version.major.to_string(),
                    package_id.as_str(),
                )
                .await;
            }
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

#[cfg(target_os = "linux")]
mod llvm_apt_install {
    use crate::downloader::{
        caching::Cacher,
        chmod_file, download,
        native_packages::{PackageManagerError, unix::UnixPackageManager},
    };
    use std::{process::Command, time::Duration};
    use url::Url;

    impl Cacher for UnixPackageManager {}

    const LLVM_INSTALL_SCRIPT_URL: &str = "https://apt.llvm.org/llvm.sh";

    /// Installs the official LLVM APT repository and its GPG key.
    ///
    /// This is required to install specific versions of clang tools on Debian-based distributions using `apt`.}
    pub async fn install_llvm_via_apt(
        ver_major: String,
        package_name: &str,
    ) -> Result<(), PackageManagerError> {
        let download_path = UnixPackageManager::get_cache_dir().join("llvm_apt_install.sh");
        if !download_path.exists()
            || !UnixPackageManager::is_cache_valid(&download_path, Some(Duration::from_hours(24)))
        {
            log::info!(
                "Downloading LLVM APT repository installation script from {LLVM_INSTALL_SCRIPT_URL}"
            );
            download(
                &Url::parse(LLVM_INSTALL_SCRIPT_URL)?,
                &download_path,
                60 * 2,
            )
            .await?;
            chmod_file(&download_path, Some(0o111))?;
        }
        let has_sudo = UnixPackageManager::has_sudo();

        let output = if has_sudo {
            Command::new("sudo")
                .arg("bash")
                .arg(download_path.as_os_str())
                .arg(ver_major)
                .output()?
        } else {
            Command::new("bash")
                .arg(download_path.as_os_str())
                .arg(ver_major)
                .output()?
        };
        if !output.status.success() {
            return Err(PackageManagerError::LlvmPpaError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        let output = if has_sudo {
            Command::new("sudo")
                .arg("apt")
                .args(["install", "-y", package_name])
                .output()
        } else {
            Command::new("apt")
                .args(["install", "-y", package_name])
                .output()
        }?;
        if !output.status.success() {
            return Err(PackageManagerError::InstallationError {
                manager: "apt (with LLVM PPA)".to_string(),
                package: package_name.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        Ok(())
    }
}
