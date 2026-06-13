//! A module to download static binaries from cpp-linter/clang-tools-static-binaries.

use std::{
    env::consts::{ARCH, OS},
    ops::RangeInclusive,
};

use crate::{Cacher, DownloadError};

/// An error that can occur while downloading a static binary.
#[derive(Debug, thiserror::Error)]
pub enum StaticDistDownloadError {
    /// An error that occurred while downloading the binary.
    #[error("Failed to download static binary: {0}")]
    DownloadError(#[from] DownloadError),

    /// The requested version does not match any available versions.
    #[error("The requested version does not match any available versions")]
    UnsupportedVersion,

    /// The static binaries are only built for
    /// x86_64 and aarch64 architecture on Linux MacOS, and
    /// Windows (not aarch64 for Windows currently).
    #[error("The static binaries are not built for {OS} {ARCH} architecture")]
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

const MIN_CLANG_TOOLS_VERSION: &str = env!("MIN_CLANG_TOOLS_VERSION");
pub(crate) const MAX_CLANG_TOOLS_VERSION: &str = env!("MAX_CLANG_TOOLS_VERSION");

/// A downloader that uses statically linked binary distribution files
/// provided by the cpp-linter team.
pub struct StaticDistDownloader;

impl Cacher for StaticDistDownloader {}

impl StaticDistDownloader {
    pub fn get_major_version_range() -> RangeInclusive<u8> {
        MIN_CLANG_TOOLS_VERSION.parse().unwrap_or(11)
            ..=MAX_CLANG_TOOLS_VERSION.parse().unwrap_or(22)
    }
}

#[cfg(any(
    // Windows support is only for x86_64 architecture (for now)
    all(target_os = "windows", not(target_arch = "x86_64")),
    // Linux and macOS support only x86_64 and aarch64 architectures
    all(
        any(target_os = "linux", target_os = "macos"),
        not(any(target_arch = "x86_64", target_arch = "aarch64"))
    ),
    // Any OS other than Windows, Linux, or macOS is unsupported
    not(any(target_os = "windows", target_os = "linux", target_os = "macos")),
))]
mod unsupported_platform {
    use super::{StaticDistDownloadError, StaticDistDownloader};
    use crate::ClangTool;
    use semver::VersionReq;
    use std::path::PathBuf;

    impl StaticDistDownloader {
        pub async fn download_tool(
            _tool: &ClangTool,
            _requested_version: &VersionReq,
            _directory: Option<&PathBuf>,
        ) -> Result<PathBuf, StaticDistDownloadError> {
            Err(StaticDistDownloadError::UnsupportedArchitecture)
        }
    }
}

#[cfg(any(
    // Windows support is only for x86_64 architecture (for now)
    all(target_os = "windows", target_arch = "x86_64"),
    // Linux and macOS support only x86_64 and aarch64 architectures
    all(
        any(target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
))]
mod supported_platform {
    use std::{
        fs,
        ops::RangeInclusive,
        path::{Path, PathBuf},
    };

    use semver::{Version, VersionReq};
    use url::Url;

    use super::{StaticDistDownloadError, StaticDistDownloader};
    use crate::{
        Cacher, ClangTool,
        downloader::{download, hashing::HashAlgorithm},
        utils::lock_path,
    };

    const CLANG_TOOLS_REPO: &str = "https://github.com/cpp-linter/clang-tools-static-binaries";
    const CLANG_TOOLS_TAG: &str = env!("CLANG_TOOLS_TAG");

    impl StaticDistDownloader {
        /// Finds a suitable version from `req_ver` within the range of available clang tools versions.
        ///
        /// The available versions are determined by the `MIN_CLANG_TOOLS_VERSION` and
        /// `MAX_CLANG_TOOLS_VERSION` environment variables (inclusive) at compile time.
        fn find_suitable_version(req_ver: &VersionReq) -> Option<Version> {
            let clang_tools_versions: RangeInclusive<u8> = Self::get_major_version_range();
            clang_tools_versions
                .map(|v| Version::new(v as u64, 0, 0))
                .rev()
                .find(|ver| req_ver.matches(ver))
        }

        /// Verifies the SHA512 checksum of the downloaded file.
        ///
        /// The expected checksum is extracted from another downloaded `*.sha512sum` file
        /// (pointed to by `sha512_path`).
        fn verify_sha512(
            file_path: &Path,
            sha512_path: &Path,
        ) -> Result<(), StaticDistDownloadError> {
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
        /// Supported architectures is limited to `x86_64` (`amd64`) and `aarch64` (`arm64`);
        /// Windows only supports x86_64 architecture (for now).
        pub async fn download_tool(
            tool: &ClangTool,
            requested_version: &VersionReq,
            directory: Option<&PathBuf>,
        ) -> Result<PathBuf, StaticDistDownloadError> {
            let ver = Self::find_suitable_version(requested_version)
                .ok_or(StaticDistDownloadError::UnsupportedVersion)?;
            let ver_str = ver.major.to_string();
            // we already gated unsupported architectures above,
            // so we can assume it's either x86_64 or aarch64 here
            let arch = if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "amd64"
            };
            let platform = if cfg!(target_os = "windows") {
                "windows"
            } else if cfg!(target_os = "macos") {
                "macos"
            } else {
                "linux"
            };

            let base_url = format!(
                "{CLANG_TOOLS_REPO}/releases/download/{CLANG_TOOLS_TAG}/{tool}-{ver_str}_{platform}-{arch}",
            );
            let suffix = if cfg!(target_os = "windows") {
                ".exe"
            } else {
                ""
            };
            let url = Url::parse(format!("{base_url}{suffix}").as_str())?;
            let cache_path = Self::get_cache_dir();
            let bin_name = format!("{tool}-{ver_str}{suffix}");
            let download_path = match directory {
                None => cache_path.join("bin").join(&bin_name),
                Some(dir) => dir.join(&bin_name),
            };
            let file_lock = lock_path(&download_path)?;
            if download_path.exists() {
                log::info!(
                    "Using cached static binary for {tool} version {ver_str} from {:?}",
                    download_path.to_string_lossy()
                );
            } else {
                log::info!("Downloading static binary for {tool} version {ver_str} from {url}");
                download(&url, &download_path, 60 * 2).await?;
                #[cfg(unix)]
                super::super::chmod_file(&download_path, None)?;
            }
            let sha512_cache_path = cache_path
                .join("static_dist")
                .join(format!("{tool}-{ver_str}.sha512"));
            if sha512_cache_path.exists() {
                log::info!(
                    "Using cached SHA512 checksum for {tool} version {ver_str} from {:?}",
                    sha512_cache_path.to_string_lossy()
                );
            } else {
                let sha512_url = Url::parse(format!("{base_url}{suffix}.sha512sum").as_str())?;
                log::info!(
                    "Downloading SHA512 checksum for {tool} version {ver_str} from {sha512_url}"
                );
                download(&sha512_url, &sha512_cache_path, 10).await?;
            }
            Self::verify_sha512(&download_path, &sha512_cache_path)?;
            file_lock.unlock()?;
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
}
