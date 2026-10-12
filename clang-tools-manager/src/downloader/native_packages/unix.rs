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
            log::debug!(
                "Running `sudo {} {} {package_id}`",
                self.as_str(),
                args.join(" ")
            );
            Command::new("sudo")
                .arg(self.as_str())
                .args(args)
                .arg(package_id.as_str())
                .output()?
        } else {
            log::debug!(
                "Running `{} {} {package_id}`",
                self.as_str(),
                args.join(" ")
            );
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
                log::error!(
                    "Failed to install {package_id} via apt: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
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
                let output = cmd
                    .args(["list", "installed"])
                    .arg(package_id.as_str())
                    .output();
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
        install_llvm_via_apt_from(LLVM_INSTALL_SCRIPT_URL, ver_major, package_name).await
    }

    /// The implementation of [`install_llvm_via_apt()`] with
    /// a configurable `script_url` (to allow mocking in tests).
    pub(super) async fn install_llvm_via_apt_from(
        script_url: &str,
        ver_major: String,
        package_name: &str,
    ) -> Result<(), PackageManagerError> {
        let download_path = UnixPackageManager::get_cache_dir().join("llvm_apt_install.sh");
        if !download_path.exists()
            || !UnixPackageManager::is_cache_valid(&download_path, Some(Duration::from_hours(24)))
        {
            log::info!("Downloading LLVM APT repository installation script from {script_url}");
            download(&Url::parse(script_url)?, &download_path, 60 * 2).await?;
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

#[cfg(test)]
mod tests {
    //! These tests use fake executables (shell scripts) in an isolated `PATH`.
    //! Thus, no real package manager (nor `sudo`) is ever invoked.

    use semver::Version;
    use tempfile::TempDir;

    use super::{PackageManager, PackageManagerError, UnixPackageManager};
    use crate::{
        ClangTool,
        test_utils_unix::{enable_logs, fake_exe, isolate_path},
    };

    /// A fake `sudo` that just runs the given command.
    const SUDO: &str = "exec \"$@\"";
    const SUCCESS: &str = "exit 0";
    const FAILURE: &str = "echo \"intentional failure\" >&2\nexit 1";

    fn all_managers() -> Vec<UnixPackageManager> {
        vec![
            #[cfg(target_os = "linux")]
            UnixPackageManager::Apt,
            #[cfg(target_os = "linux")]
            UnixPackageManager::Dnf,
            #[cfg(target_os = "linux")]
            UnixPackageManager::PacMan,
            UnixPackageManager::Homebrew,
        ]
    }

    /// Creates a temp directory and isolates the `PATH` to it.
    fn setup() -> TempDir {
        enable_logs();
        let tmp = TempDir::new().unwrap();
        isolate_path(tmp.path());
        tmp
    }

    async fn install(
        mgr: &UnixPackageManager,
        version: Option<&Version>,
    ) -> Result<(), PackageManagerError> {
        mgr.install_package("clang", version).await
    }

    fn assert_install_error(result: Result<(), PackageManagerError>, expected_mgr: &str) {
        let err = result.unwrap_err();
        println!("{err}");
        assert!(matches!(
            err,
            PackageManagerError::InstallationError { manager, stderr, .. }
                if manager == expected_mgr && stderr.contains("intentional failure")
        ));
    }

    #[test]
    fn names() {
        let ver = Version::new(17, 0, 0);
        for mgr in all_managers() {
            assert_eq!(
                mgr.to_string().to_lowercase().replace("homebrew", "brew"),
                mgr.as_str()
            );
            let pkg = mgr.get_package_name(&ClangTool::ClangFormat);
            let with_ver = mgr.pkg_name_with_version(&pkg, Some(&ver));
            assert!(with_ver.starts_with(&pkg));
            assert!(with_ver.ends_with("17"));
            assert_eq!(mgr.pkg_name_with_version(&pkg, None), pkg);
        }
        let listed = UnixPackageManager::list_managers()
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>();
        let expected = all_managers()
            .iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>();
        assert_eq!(listed, expected);
    }

    #[test]
    fn is_installed() {
        let tmp = setup();
        for mgr in all_managers() {
            assert!(!mgr.is_installed());
            fake_exe(tmp.path(), mgr.as_str(), SUCCESS);
            assert!(mgr.is_installed());
        }
    }

    #[test]
    fn is_installed_package() {
        let tmp = setup();
        let ver = Version::new(17, 0, 0);
        for mgr in all_managers() {
            // package manager not found
            assert!(!mgr.is_installed_package("clang", Some(&ver)));
            fake_exe(tmp.path(), mgr.as_str(), SUCCESS);
            assert!(mgr.is_installed_package("clang", Some(&ver)));
            fake_exe(tmp.path(), mgr.as_str(), FAILURE);
            assert!(!mgr.is_installed_package("clang", None));
        }
    }

    #[tokio::test]
    async fn install_with_sudo() {
        let tmp = setup();
        fake_exe(tmp.path(), "sudo", SUDO);
        for mgr in all_managers() {
            fake_exe(tmp.path(), mgr.as_str(), SUCCESS);
            install(&mgr, Some(&Version::new(17, 0, 0))).await.unwrap();
        }
    }

    #[tokio::test]
    async fn install_without_sudo() {
        let tmp = setup();
        for mgr in all_managers() {
            fake_exe(tmp.path(), mgr.as_str(), SUCCESS);
            install(&mgr, None).await.unwrap();
        }
    }

    #[tokio::test]
    async fn install_failure() {
        let tmp = setup();
        for mgr in all_managers() {
            fake_exe(tmp.path(), mgr.as_str(), FAILURE);
            // `apt` falls back to LLVM's PPA only when a version is specified.
            assert_install_error(install(&mgr, None).await, mgr.as_str());
        }
    }

    #[tokio::test]
    async fn install_missing_manager() {
        let _tmp = setup();
        for mgr in all_managers() {
            let err = install(&mgr, None).await.unwrap_err();
            assert!(matches!(err, PackageManagerError::Io(_)));
        }
    }

    #[cfg(target_os = "linux")]
    mod llvm_ppa {
        use std::{fs, path::Path};

        use mockito::Server;
        use semver::Version;
        use tempfile::TempDir;

        use super::{FAILURE, SUCCESS, SUDO, assert_install_error, fake_exe, install, setup};
        use crate::{
            downloader::native_packages::{
                PackageManagerError,
                unix::{UnixPackageManager, llvm_apt_install::install_llvm_via_apt_from},
            },
            test_utils_unix::set_cache_dir,
        };

        const MARKER: &str = "ppa_added";

        /// Creates a fake `bash` that (if `succeed`) adds a marker file to signify the PPA was added,
        /// and a fake `apt` that only succeeds if the PPA marker file exists.
        fn fake_ppa(dir: &Path, succeed: bool) {
            let marker = dir.join(MARKER);
            let marker = marker.to_string_lossy();
            let bash = if succeed {
                format!(": > \"{marker}\"")
            } else {
                FAILURE.to_string()
            };
            fake_exe(dir, "bash", &bash);
            let apt = format!("if [ -f \"{marker}\" ]; then exit 0; fi\n{FAILURE}");
            fake_exe(dir, "apt", &apt);
        }

        /// Sets the cache dir to a new temp dir, and (optionally) seeds it with
        /// a fresh copy of the LLVM install script (to avoid downloading it).
        fn seed_cache(seed: bool) -> TempDir {
            let cache = TempDir::new().unwrap();
            set_cache_dir(cache.path());
            if seed {
                fs::write(cache.path().join("llvm_apt_install.sh"), "exit 0").unwrap();
            }
            cache
        }

        async fn apt_install() -> Result<(), PackageManagerError> {
            install(&UnixPackageManager::Apt, Some(&Version::new(17, 0, 0))).await
        }

        #[tokio::test]
        async fn with_sudo() {
            let tmp = setup();
            let _cache = seed_cache(true);
            fake_exe(tmp.path(), "sudo", SUDO);
            fake_ppa(tmp.path(), true);
            apt_install().await.unwrap();
            assert!(tmp.path().join(MARKER).exists());
        }

        #[tokio::test]
        async fn without_sudo() {
            let tmp = setup();
            let _cache = seed_cache(true);
            fake_ppa(tmp.path(), true);
            apt_install().await.unwrap();
            assert!(tmp.path().join(MARKER).exists());
        }

        #[tokio::test]
        async fn script_failure() {
            for sudo in [true, false] {
                let tmp = setup();
                let _cache = seed_cache(true);
                if sudo {
                    fake_exe(tmp.path(), "sudo", SUDO);
                }
                fake_ppa(tmp.path(), false);
                let err = apt_install().await.unwrap_err();
                println!("{err}");
                assert!(matches!(
                    err,
                    PackageManagerError::LlvmPpaError(msg) if msg.contains("intentional failure")
                ));
            }
        }

        #[tokio::test]
        async fn apt_failure_after_ppa() {
            for sudo in [true, false] {
                let tmp = setup();
                let _cache = seed_cache(true);
                if sudo {
                    fake_exe(tmp.path(), "sudo", SUDO);
                }
                fake_exe(tmp.path(), "bash", SUCCESS);
                fake_exe(tmp.path(), "apt", FAILURE);
                assert_install_error(apt_install().await, "apt (with LLVM PPA)");
            }
        }

        #[tokio::test]
        async fn apt_missing_after_ppa() {
            let tmp = setup();
            let _cache = seed_cache(true);
            fake_exe(tmp.path(), "bash", SUCCESS);
            // the script URL is not used because the script is already cached.
            let err = install_llvm_via_apt_from("not used", "17".to_string(), "clang-format-17")
                .await
                .unwrap_err();
            assert!(matches!(err, PackageManagerError::Io(_)));
        }

        #[tokio::test]
        async fn download_script() {
            let tmp = setup();
            let cache = seed_cache(false);
            fake_ppa(tmp.path(), true);
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/llvm.sh")
                .with_status(200)
                .with_body("exit 0")
                .create();
            let url = format!("{}/llvm.sh", server.url());
            install_llvm_via_apt_from(&url, "17".to_string(), "clang-format-17")
                .await
                .unwrap();
            mock.assert();
            assert!(cache.path().join("llvm_apt_install.sh").exists());
        }

        #[tokio::test]
        async fn bad_script_url() {
            let _tmp = setup();
            let _cache = seed_cache(false);
            let err = install_llvm_via_apt_from("not a url", "17".to_string(), "clang-format-17")
                .await
                .unwrap_err();
            assert!(matches!(err, PackageManagerError::UrlParseError(_)));
        }

        #[tokio::test]
        async fn failed_script_download() {
            let _tmp = setup();
            let _cache = seed_cache(false);
            let mut server = Server::new_async().await;
            let mock = server.mock("GET", "/llvm.sh").with_status(404).create();
            let url = format!("{}/llvm.sh", server.url());
            let err = install_llvm_via_apt_from(&url, "17".to_string(), "clang-format-17")
                .await
                .unwrap_err();
            mock.assert();
            assert!(matches!(err, PackageManagerError::DownloadError(_)));
        }
    }
}
