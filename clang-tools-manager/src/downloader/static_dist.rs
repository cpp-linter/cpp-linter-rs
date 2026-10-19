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
    /// x86_64 and aarch64 architecture on Linux, MacOS, and Windows.
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
    // Windows, Linux, and MacOS support only x86_64 and aarch64 architectures
    all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
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

#[cfg(
    // Windows, Linux, and MacOS support only x86_64 and aarch64 architectures
    all(
        any(target_os = "windows", target_os = "linux", target_os = "macos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
)]
mod supported_platform {
    use std::{
        fs,
        io::{BufRead, BufReader},
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

    /// Each available LLVM major version and the release tag that holds its binaries.
    const RELEASE_TAGS: &[(u8, &str)] = include!(concat!(env!("OUT_DIR"), "/release_tags.rs"));

    impl StaticDistDownloader {
        /// Finds the newest available version that satisfies `req_ver`,
        /// along with the release tag that holds its binaries.
        ///
        /// The available versions (and their release tags) are read from the
        /// static binaries' `index.json` at compile time.
        pub(crate) fn find_suitable_version(
            req_ver: &VersionReq,
        ) -> Option<(Version, &'static str)> {
            RELEASE_TAGS
                .iter()
                .rev()
                .map(|(major, tag)| (Version::new(*major as u64, 0, 0), *tag))
                .find(|(ver, _)| req_ver.matches(ver))
        }

        /// Verifies the SHA512 checksum of the downloaded file.
        ///
        /// The expected checksum is extracted from another downloaded `SHA512SUMS` file
        /// (pointed to by `sha512_path`).
        fn verify_sha512(
            file_path: &Path,
            tool: &str,
            sha512_path: &Path,
        ) -> Result<(), StaticDistDownloadError> {
            let expected = {
                let checksum_file_handle = fs::File::open(sha512_path)?;
                let checksum_file_content = BufReader::new(checksum_file_handle);
                let mut found = None;
                for line in checksum_file_content.lines() {
                    let line = line?;
                    if line.ends_with(tool) {
                        found = line.split(' ').next().map(|s| s.trim().to_string());
                        break;
                    }
                }
                found
            }
            .ok_or(StaticDistDownloadError::Sha512Corruption)?;
            HashAlgorithm::Sha512(expected).verify(file_path)?;
            Ok(())
        }

        /// The cached `SHA512SUMS` file of the release tagged `release_tag`.
        pub(crate) fn sha512_cache_path(release_tag: &str) -> PathBuf {
            Self::get_cache_dir()
                .join("static_dist")
                .join(release_tag)
                .join("SHA512SUMS")
        }

        /// Downloads the `requested_version` of the specified `tool` from a distribution of statically linked binaries.
        ///
        /// The distribution is maintained at <https://github.com/cpp-linter/clang-tools-static-binaries>.
        /// Supported platforms includes Windows, Linux, and MacOS.
        /// Supported architectures is limited to `x86_64` (`amd64`) and `aarch64` (`arm64`).
        pub async fn download_tool(
            tool: &ClangTool,
            requested_version: &VersionReq,
            directory: Option<&PathBuf>,
        ) -> Result<PathBuf, StaticDistDownloadError> {
            let (ver, release_tag) = Self::find_suitable_version(requested_version)
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

            let base_url = format!("{CLANG_TOOLS_REPO}/releases/download/{release_tag}/");
            let suffix = if cfg!(target_os = "windows") {
                ".exe"
            } else {
                ""
            };
            let tool_url_path = format!("{tool}-{ver_str}_{platform}-{arch}{suffix}");
            let url = Url::parse(format!("{base_url}{tool_url_path}").as_str())?;
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
            let sha512_cache_path = Self::sha512_cache_path(release_tag);
            if sha512_cache_path.exists() {
                log::info!(
                    "Using cached SHA512 checksums for static binaries from {:?}",
                    sha512_cache_path.to_string_lossy()
                );
            } else {
                let sha512_url = Url::parse(format!("{base_url}SHA512SUMS").as_str())?;
                log::info!("Downloading SHA512 checksum for static binaries from {sha512_url}");
                download(&sha512_url, &sha512_cache_path, 10).await?;
            }
            Self::verify_sha512(&download_path, &tool_url_path, &sha512_cache_path)?;
            file_lock.unlock()?;
            Ok(download_path)
        }
    }

    #[cfg(test)]
    mod tests {
        use std::fs;

        use super::{StaticDistDownloadError, StaticDistDownloader};
        use crate::ClangTool;
        use semver::VersionReq;
        use tempfile::TempDir;

        #[test]
        fn find_newest_with_tag() {
            let req_ver = VersionReq::parse(">=12").unwrap();
            let (ver, tag) = StaticDistDownloader::find_suitable_version(&req_ver).unwrap();
            assert_eq!(ver.major.to_string(), super::super::MAX_CLANG_TOOLS_VERSION);
            assert!(tag.starts_with(&format!("llvm-{}.", ver.major)));
        }

        #[test]
        fn find_none() {
            let req_ver = VersionReq::parse("=8").unwrap();
            let suitable_version = StaticDistDownloader::find_suitable_version(&req_ver);
            assert_eq!(suitable_version, None);
        }

        /// Uses a pre-seeded cache (binary and SHA512SUMS), so no network access is needed.
        #[tokio::test]
        async fn cached_in_directory() {
            use sha2::{Digest, Sha512};

            log::set_max_level(log::LevelFilter::Trace);
            let cache = TempDir::new().unwrap();
            unsafe {
                std::env::set_var("CPP_LINTER_CACHE", cache.path());
            }
            let out_dir = TempDir::new().unwrap();
            let suffix = std::env::consts::EXE_SUFFIX;
            let bin = out_dir.path().join(format!("clang-format-17{suffix}"));
            let content = b"not really clang-format";
            fs::write(&bin, content).unwrap();
            let hash = Sha512::digest(content)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            // list checksums for all supported platforms
            let mut sums = String::new();
            for platform in ["linux", "macos", "windows"] {
                for arch in ["amd64", "arm64"] {
                    sums.push_str(&format!(
                        "{hash}  clang-format-17_{platform}-{arch}{suffix}\n"
                    ));
                }
            }
            let sha512_sums = StaticDistDownloader::sha512_cache_path(
                StaticDistDownloader::find_suitable_version(&VersionReq::parse("17").unwrap())
                    .unwrap()
                    .1,
            );
            fs::create_dir_all(sha512_sums.parent().unwrap()).unwrap();
            fs::write(&sha512_sums, sums).unwrap();

            let req = VersionReq::parse("17").unwrap();
            let tool = ClangTool::ClangFormat;
            let out_dir_path = out_dir.path().to_path_buf();
            let result = StaticDistDownloader::download_tool(&tool, &req, Some(&out_dir_path))
                .await
                .unwrap();
            assert_eq!(result, bin);

            // a corrupt SHA512SUMS file (missing the tool's entry)
            fs::write(&sha512_sums, format!("{hash}  some-other-file\n")).unwrap();
            let err = StaticDistDownloader::download_tool(&tool, &req, Some(&out_dir_path))
                .await
                .unwrap_err();
            assert!(matches!(err, StaticDistDownloadError::Sha512Corruption));
        }
    }
}
