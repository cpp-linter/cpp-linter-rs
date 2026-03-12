//! A module to download static binaries from cpp-linter/clang-tools-static-binaries.

use std::{
    env::consts,
    fs,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use semver::{Version, VersionReq};
use url::Url;

use crate::{
    Cacher, ClangTool, DownloadError,
    downloader::{download, hashing::HashAlgorithm},
};

/// An error that can occur while downloading a static binary.
#[derive(Debug, thiserror::Error)]
pub enum StaticDistDownloadError {
    /// An error that occurred while downloading the binary.
    #[error("Failed to download static binary: {0}")]
    DownloadError(#[from] DownloadError),

    /// The requested version does not match any available versions.
    #[error("The requested version does not match any available versions")]
    UnsupportedVersion,

    /// The static binaries are only built for x86_64 (amd64) architecture.
    #[error("The static binaries are only built for x86_64 (amd64) architecture")]
    UnsupportedArchitecture,

    /// Failed to parse a URL.
    #[error("Failed to parse the URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    /// Failed to read or write a cache file.
    #[error("Failed to read or write cache file: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse the SHA512 sum file.
    #[error("Failed to parse the SHA512 sum file")]
    Sha512Corruption,
}

/// A downloader that uses statically linked binary distribution files
/// provided by the cpp-linter team.
pub struct StaticDistDownloader;

impl Cacher for StaticDistDownloader {}

impl StaticDistDownloader {
    /// Finds a suitable version from `req_ver` within the range of available clang tools versions.
    ///
    /// The available versions are determined by the `MIN_CLANG_TOOLS_VERSION` and
    /// `MAX_CLANG_TOOLS_VERSION` environment variables (inclusive) at compile time.
    fn find_suitable_version(req_ver: &VersionReq) -> Option<Version> {
        let min_clang_tools_version: u8 = option_env!("MIN_CLANG_TOOLS_VERSION")
            .unwrap_or("9")
            .parse()
            .expect("Invalid MIN_CLANG_TOOLS_VERSION env var value");
        let max_clang_tools_version: u8 = option_env!("MAX_CLANG_TOOLS_VERSION")
            .unwrap_or("21")
            .parse()
            .expect("Invalid MAX_CLANG_TOOLS_VERSION env var value");
        let clang_tools_versions: RangeInclusive<u8> =
            min_clang_tools_version..=max_clang_tools_version;
        let outlier = Version::new(12, 0, 1);
        for ver in clang_tools_versions
            .map(|v| Version::new(v as u64, 0, 0))
            .rev()
        {
            if ver.major == 12 && req_ver.matches(&outlier) {
                return Some(outlier);
            } else if req_ver.matches(&ver) {
                return Some(ver);
            }
        }
        None
    }

    /// Verifies the SHA512 checksum of the downloaded file.
    ///
    /// The expected checksum is extracted from another downloaded `*.sha512sum` file
    /// (pointed to by `sha512_path`).
    fn verify_sha512(file_path: &Path, sha512_path: &Path) -> Result<(), StaticDistDownloadError> {
        let checksum_file_content = fs::read_to_string(sha512_path)?;
        let expected = checksum_file_content
            .split(' ')
            .next()
            .ok_or(StaticDistDownloadError::Sha512Corruption)?;
        HashAlgorithm::Sha512(expected.to_string()).verify(file_path)?;
        Ok(())
    }

    /// Downloads the `requested_version` of the specified `tool` from a distribution of statically linked binaries.
    ///
    /// The distribution is maintained at <https://github.com/cpp-linter/clang-tools-static-binaries>.
    /// Supported platforms includes Windows, Linux, and MacOS.
    /// Supported architectures is limited to `x86_64` (`amd64`).
    pub async fn download_tool(
        tool: &ClangTool,
        requested_version: &VersionReq,
    ) -> Result<PathBuf, StaticDistDownloadError> {
        if consts::ARCH != "x86_64" {
            return Err(StaticDistDownloadError::UnsupportedArchitecture);
        }
        let ver = Self::find_suitable_version(requested_version)
            .ok_or(StaticDistDownloadError::UnsupportedVersion)?;
        let ver_str = if ver.minor == 0 && ver.patch == 0 {
            ver.major.to_string()
        } else {
            ver.to_string()
        };
        let suffix = if cfg!(target_os = "windows") {
            ".exe"
        } else {
            ""
        };
        let clang_tools_repo: &str = option_env!("CLANG_TOOLS_REPO")
            .unwrap_or("https://github.com/cpp-linter/clang-tools-static-binaries");
        let clang_tools_tag: &str = option_env!("CLANG_TOOLS_TAG").unwrap_or("master-6e612956");

        let base_url = format!(
            "{clang_tools_repo}/releases/download/{clang_tools_tag}/{tool}-{ver_str}_{}-amd64",
            if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "macos") {
                "macos"
            } else {
                "linux"
            },
        );
        let url = Url::parse(format!("{base_url}{suffix}").as_str())?;
        let cache_path = Self::get_cache_dir();
        let download_path = cache_path
            .join("bin")
            .join(format!("{tool}-{ver_str}{suffix}").as_str());
        if download_path.exists() {
            log::info!(
                "Using cached static binary for {tool} version {ver_str} from {:?}",
                download_path.to_string_lossy()
            );
        } else {
            log::info!("Downloading static binary for {tool} version {ver_str} from {url}");
            download(&url, &download_path, 60 * 2).await?;
            #[cfg(unix)]
            {
                // Make the extracted binary executable on Unix-like systems.
                use std::os::unix::fs::PermissionsExt;
                let out = fs::OpenOptions::new().write(true).open(&download_path)?;
                let mut perms = out.metadata()?.permissions();
                perms.set_mode(0o755);
                out.set_permissions(perms)?;
            }
        }
        let sha512_cache_path = cache_path
            .join("static_dist")
            .join(format!("{tool}-{ver_str}.sha512").as_str());
        if sha512_cache_path.exists() {
            log::info!(
                "Using cached SHA512 checksum for {tool} version {ver_str} from {:?}",
                sha512_cache_path.to_string_lossy()
            );
        } else {
            let sha512_url = Url::parse(format!("{base_url}.sha512sum").as_str())?;
            log::info!(
                "Downloading SHA512 checksum for {tool} version {ver_str} from {sha512_url}"
            );
            download(&sha512_url, &sha512_cache_path, 10).await?;
        }
        Self::verify_sha512(&download_path, &sha512_cache_path)?;
        Ok(download_path)
    }
}

#[cfg(test)]
mod tests {
    use super::StaticDistDownloader;
    use semver::VersionReq;

    #[test]
    fn find_none() {
        let req_ver = VersionReq::parse("=8").unwrap();
        let suitable_version = StaticDistDownloader::find_suitable_version(&req_ver);
        assert_eq!(suitable_version, None);
    }
}
