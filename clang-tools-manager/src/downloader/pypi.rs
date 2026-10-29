use super::{
    DownloadError,
    caching::Cacher,
    download,
    hashing::HashAlgorithm,
    resource_headers::{self, ResourceHeadersError},
};
use crate::{ClangTool, progress_bar::ProgressBar, utils::lock_path};

use semver::{Version, VersionReq};
use serde::{Deserialize, de::Visitor};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    num::NonZero,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    time::Duration,
};
use url::Url;
use zip::{ZipArchive, result::ZipError};

/// Errors that occur during PyPI downloads.
#[derive(Debug, thiserror::Error)]
pub enum PyPiDownloadError {
    /// Errors that occur during HTTP requests.
    #[error("HTTP request error: {0}")]
    DownloadCache(#[from] DownloadError),

    /// Errors that occur when parsing version strings.
    #[error("Invalid version string")]
    InvalidVersion,

    /// Error indicating that no suitable version was found on PyPI for the given requirement and system compatibility.
    #[error("No version on PyPI satisfies the given requirement")]
    NoVersionFound,

    /// Errors that occur when deserializing JSON responses.
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),

    /// Errors that occur when parsing wheel filenames.
    #[error("Invalid wheel name: {0}")]
    InvalidWheelName(String),

    /// Errors that occur when parsing URLs.
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// Errors that occur when reading from the cache.
    #[error("Cache read error: {0}")]
    ReadCache(#[from] std::io::Error),

    /// Errors that occur when reading a ZIP archive from the cache.
    #[error("ZIP archive error: {0}")]
    ZipArchive(#[from] ZipError),

    /// Error that indicates the expected executable was not found in the downloaded wheel.
    #[error("Expected executable not found in the downloaded wheel")]
    ExecutableNotFound,

    /// Errors that occur when extracting clang's resource headers from the downloaded wheel.
    #[error("Failed to extract clang resource headers: {0}")]
    ResourceHeaders(#[from] ResourceHeadersError),
}

/// Represents the information of a package on PyPI
#[derive(Debug, Deserialize)]
struct PyPiProjectInfo {
    /// A mapping from version strings to a list of release information for that version.
    releases: HashMap<String, Vec<PyPiReleaseInfo>>,
}

struct HashAlgorithmVisitor;
impl<'de> Visitor<'de> for HashAlgorithmVisitor {
    type Value = Vec<HashAlgorithm>;

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut result = vec![];
        while let Some((key, value)) = map.next_entry::<String, String>()? {
            match key.as_str() {
                "sha256" => result.push(HashAlgorithm::Sha256(value.to_lowercase())),
                "blake2b_256" => result.push(HashAlgorithm::Blake2b256(value.to_lowercase())),
                _ => (),
            }
        }
        Ok(result)
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a map of hash algorithms names to their corresponding checksum values")
    }
}

fn deserialize_digests<'de, D>(digest_map: D) -> Result<Vec<HashAlgorithm>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    digest_map.deserialize_map(HashAlgorithmVisitor)
}

/// Represents the information of a single release of a package on PyPI.
#[derive(Debug, Deserialize, Clone)]
struct PyPiReleaseInfo {
    /// The URL to download the release.
    url: String,

    /// The filename of the release.
    filename: String,

    /// The size of the release in bytes.
    size: u64,

    /// A mapping from digest algorithm names to their corresponding hash values.
    #[serde(deserialize_with = "deserialize_digests")]
    digests: Vec<HashAlgorithm>,

    /// Indicates whether the release has been yanked.
    yanked: bool,
}

/// Represents the C library used by a Linux wheel, which can be either glibc or musl.
#[derive(Debug, PartialEq, Eq)]
enum LinuxLibC {
    Glibc { version: Version },
    Musl { version: Version },
}

impl LinuxLibC {
    fn get_musl_version() -> Option<Version> {
        // This is a simplified check for musl version.
        // In practice, determining the musl version may require more complex logic,
        // such as parsing the output of `ldd --version` or checking for specific symbols in the C library.
        if let Ok(output) = Command::new("ldd").arg("--version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut is_musl = false;
            for line in stdout.lines() {
                let line = line.trim().to_lowercase();
                // Output of `ldd --version` on musl typically looks like:
                // ```shell
                // musl libc (x86_64)
                // Version 1.2.2
                // Dynamic Program Loader
                // ```
                // So, first look for "musl" in the output.
                // Then, look for a version number if "musl" is found.
                if !is_musl && line.contains("musl") {
                    is_musl = true;
                }
                if is_musl
                    && line.contains("version")
                    && let Some(version_str) = line.split_whitespace().last()
                    && let Ok(version) = Version::parse(version_str)
                {
                    return Some(version);
                }
            }
        }
        // If we can't determine the musl version, assume it's compatible with musllinux wheels.
        None
    }

    fn get_glibc_version() -> Option<Version> {
        if let Ok(output) = Command::new("ldd").arg("--version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim().to_lowercase();
                // Output of `ldd --version` on glibc typically looks like:
                // ```shell
                // ldd (Ubuntu GLIBC 2.39-0ubuntu8.7) 2.39
                // Copyright (C) 2024 Free Software Foundation, Inc.
                // ```
                // So, look for the version on the line that contains "glibc" in the output.
                if line.contains("glibc")
                    && let Some(version_str) = line.split_whitespace().last()
                    && let Some((major, minor)) = version_str.split_once('.')
                    && let Ok(major) = major.parse::<u64>()
                    && let Ok(minor) = minor.parse::<u64>()
                {
                    return Some(Version::new(major, minor, 0));
                }
            }
        }
        None
    }

    /// Checks if the [LinuxLibC] is compatible with the current system.
    pub fn is_compatible_with_system(&self) -> bool {
        match self {
            LinuxLibC::Musl {
                version: pkg_musl_version,
            } => Self::get_musl_version()
                .map(|sys_musl_version| sys_musl_version >= *pkg_musl_version)
                .unwrap_or(false),
            LinuxLibC::Glibc {
                version: pkg_glibc_version,
            } => Self::get_glibc_version()
                .map(|sys_glibc_version| sys_glibc_version >= *pkg_glibc_version)
                .unwrap_or(false),
        }
    }
}

/// Represents the operating system of a wheel's target platform.
#[derive(Debug, PartialEq, Eq)]
enum PlatformOs {
    Windows,
    MacOS,
    Linux { lib_c: LinuxLibC },
}

impl PlatformOs {
    /// Checks if the [PlatformOs] is compatible with the current system.
    pub fn is_compatible_with_system(&self) -> bool {
        match self {
            PlatformOs::Windows => cfg!(target_os = "windows"),
            PlatformOs::MacOS => cfg!(target_os = "macos"),
            PlatformOs::Linux { lib_c } => {
                cfg!(target_os = "linux") && lib_c.is_compatible_with_system()
            }
        }
    }
}

/// Represents the platform tag of a Python wheel's filename.
///
/// This is the last segment of the wheel filename,
/// which indicates the target platform for the wheel.
#[derive(Debug)]
struct PlatformTag {
    /// The operating system for which the wheel is built.
    os: PlatformOs,

    /// The machine architecture for which the wheel is built.
    arch: String,
}

impl PlatformTag {
    /// Checks if the platform tag is compatible with the current system.
    pub fn is_compatible_with_system(&self) -> bool {
        self.os.is_compatible_with_system()
            && self.is_arch_compatible(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Checks if the wheel's architecture runs on the given `sys_os` and `sys_arch`
    /// (named as in [`std::env::consts`]).
    fn is_arch_compatible(&self, sys_os: &str, sys_arch: &str) -> bool {
        match sys_os {
            "windows" => match sys_arch {
                "x86_64" => self.arch == "amd64",
                "aarch64" => self.arch == "arm64",
                "x86" => self.arch == "x86",
                _ => false,
            },
            "macos" => match sys_arch {
                "x86_64" => self.arch == "x86_64" || self.arch == "universal2",
                "aarch64" => self.arch == "arm64" || self.arch == "universal2",
                _ => false,
            },
            "linux" => self.arch == sys_arch,
            _ => false,
        }
    }
}

impl FromStr for PlatformTag {
    type Err = PyPiDownloadError;

    /// Parses the platform tag from a wheel filename.
    ///
    /// The input string can be the platform tag itself, not the entire wheel filename.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = if s.contains(".manylinux") {
            s.split('.')
                .find(|part| part.starts_with("manylinux_"))
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?
        } else {
            s
        };
        if s == "win32" {
            Ok(Self {
                os: PlatformOs::Windows,
                arch: "x86".to_string(),
            })
        } else if s.starts_with("win") {
            let (_, arch) = s
                .split_once('_')
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?;
            Ok(Self {
                os: PlatformOs::Windows,
                arch: arch.to_string(),
            })
        } else if s.starts_with("manylinux1") {
            let (_, arch) = s
                .split_once('_')
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?;
            Ok(Self {
                os: PlatformOs::Linux {
                    lib_c: LinuxLibC::Glibc {
                        version: Version::new(2, 5, 0),
                    },
                },
                arch: arch.to_string(),
            })
        } else if s.starts_with("musllinux")
            || s.starts_with("manylinux_")
            || s.starts_with("macosx")
        {
            let mut parts = s.splitn(4, '_');
            let os = parts
                .next()
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?;
            let lib_c_ver = Version::new(
                parts
                    .next()
                    .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?
                    .parse::<u64>()
                    .unwrap_or(1),
                parts
                    .next()
                    .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?
                    .parse::<u64>()
                    .unwrap_or(1),
                0,
            );
            let arch = parts
                .next()
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?;
            if os == "macosx" {
                Ok(Self {
                    os: PlatformOs::MacOS,
                    arch: arch.to_string(),
                })
            } else if os.starts_with("musl") {
                Ok(Self {
                    os: PlatformOs::Linux {
                        lib_c: LinuxLibC::Musl { version: lib_c_ver },
                    },
                    arch: arch.to_string(),
                })
            } else {
                Ok(Self {
                    os: PlatformOs::Linux {
                        lib_c: LinuxLibC::Glibc { version: lib_c_ver },
                    },
                    arch: arch.to_string(),
                })
            }
        } else {
            Err(PyPiDownloadError::InvalidWheelName(s.to_string()))
        }
    }
}

/// Represents the tags of a Python wheel's filename.
///
/// See [PyPA docs](https://packaging.python.org/en/latest/specifications/binary-distribution-format/#file-format)
/// for more details.
///
/// ```txt
/// {distribution}-{version}(-{build tag})?-{python tag}-{abi tag}-{platform tag}.whl
/// ```
#[derive(Debug)]
struct WheelTags {
    /// The platform tag indicates the wheel's target platform.
    platform: PlatformTag,
}

impl FromStr for WheelTags {
    type Err = PyPiDownloadError;

    /// Parses a wheel filename into its tags.
    ///
    /// Does not support source distribution names.
    /// Only keeps information relevant to system compatibility checks.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.trim_end_matches(".whl").split('-');
        let tags_len = iter.clone().count();
        if tags_len < 5 {
            return Err(PyPiDownloadError::InvalidWheelName(s.to_string()));
        }
        iter.next(); // already know the package name

        let platform = PlatformTag::from_str(iter.next_back().unwrap())?;

        // The remaining tags are not used for compatibility checks.
        // These binary wheels come with the executable within, so
        // we don't need to validate python version nor abi tags here.
        // optional build tag is not used in clang_format or clang_tidy wheel deployments

        Ok(Self { platform })
    }
}

impl WheelTags {
    /// Checks if the wheel tags indicate compatibility with the current system.
    pub fn is_compatible_with_system(&self) -> bool {
        self.platform.is_compatible_with_system()
    }
}

/// A downloader for PyPI releases.
pub struct PyPiDownloader;

impl Cacher for PyPiDownloader {}

const PYPI_JSON_API_URL: &str = "https://pypi.org";

impl PyPiDownloader {
    /// Returns the best available release info from PyPI for the given tool and minimum version.
    fn get_best_pypi_release(
        clang_tool: &ClangTool,
        pypi_info: &PyPiProjectInfo,
        version: &VersionReq,
    ) -> Result<(Version, PyPiReleaseInfo, String), PyPiDownloadError> {
        let mut result = None;

        for (ver_str, releases) in &pypi_info.releases {
            let ver = match Version::parse(ver_str) {
                Ok(v) => Version {
                    // append a pre-release number to weight it properly against a build number (translated to prerelease number below)
                    pre: semver::Prerelease::from_str("0").unwrap_or_default(),
                    ..v
                },
                Err(_) => {
                    let mut components = ver_str.split('.');
                    let count = components.clone().count();
                    if count <= 3 {
                        // should've parsed this normally, log warning and skip this version
                        log::warn!("Skipping malformed version {ver_str} for {clang_tool} on PyPI");
                        continue;
                    } else {
                        // take the first four components and treat them like a `major.minor.patch.build-number`
                        let major: u64 = components.next().unwrap_or("0").parse().unwrap_or(0);
                        let minor: u64 = components.next().unwrap_or("0").parse().unwrap_or(0);
                        let patch: u64 = components.next().unwrap_or("0").parse().unwrap_or(0);
                        let build: &str = components.next().unwrap_or("0");
                        Version {
                            major,
                            minor,
                            patch,
                            pre: semver::Prerelease::from_str(build).unwrap_or_default(),
                            build: semver::BuildMetadata::EMPTY,
                        }
                    }
                }
            };
            // do not compare pre-release numbers since these are actually build numbers
            if version.matches(&Version {
                major: ver.major,
                minor: ver.minor,
                patch: ver.patch,
                pre: semver::Prerelease::default(),
                build: semver::BuildMetadata::EMPTY,
            }) {
                for release in releases {
                    if !release.filename.ends_with(".whl") {
                        continue;
                    }
                    let wheel_tags = WheelTags::from_str(&release.filename)?;
                    if !release.yanked && wheel_tags.is_compatible_with_system() {
                        log::debug!(
                            "Found {clang_tool} v{ver_str} (size: {}, digest: {:?}); {wheel_tags:?}",
                            release.size,
                            release.digests
                        );
                        if result.as_ref().is_none_or(|(v, _, _)| *v < ver) {
                            result = Some((ver.clone(), release, ver_str.to_owned()));
                        }
                    }
                }
            }
        }
        result
            .map(|(a, b, c)| (Version::new(a.major, a.minor, a.patch), b.to_owned(), c))
            .ok_or(PyPiDownloadError::NoVersionFound)
    }

    async fn get_pypi_release_info(
        clang_tool: &ClangTool,
    ) -> Result<PyPiProjectInfo, PyPiDownloadError> {
        let cache_file = Self::get_cache_dir()
            .join("pypi")
            .join(format!("{clang_tool}_pypi.json"));
        let file_lock = lock_path(&cache_file)?;
        // PyPI package info cache should not be refreshed unless it is more than 10 minutes old.
        // This is behavior recommended by PyPI response header `Cache-Control: max-age=600`.
        // Instead of caching the `Cache-Control` header, we'll just check the cached file's "last modified" time.
        let cache_valid = Self::is_cache_valid(&cache_file, Some(Duration::from_mins(10)));
        let body = if cache_valid {
            log::info!(
                "Using cached PyPI info for {clang_tool} from {}",
                cache_file.to_string_lossy()
            );
            std::fs::read_to_string(&cache_file)?
        } else {
            let api_url = format!("{PYPI_JSON_API_URL}/pypi/{clang_tool}/");
            let endpoint = Url::parse(&api_url)?.join("json")?;
            log::info!("Fetching PyPI info for {clang_tool} from {endpoint}");
            download(&endpoint, &cache_file, 10).await?;
            std::fs::read_to_string(&cache_file)?
        };
        file_lock.unlock()?;
        Ok(serde_json::from_str(body.as_str())?)
    }

    /// Downloads the specified `clang_tool` and `version` from PyPI.
    ///
    /// Determines the best available release based on the version requirement and system compatibility,
    /// then downloads the wheel file and caches it locally.
    pub async fn download_tool(
        clang_tool: &ClangTool,
        version: &VersionReq,
        directory: Option<&PathBuf>,
    ) -> Result<PathBuf, PyPiDownloadError> {
        let info = Self::get_pypi_release_info(clang_tool).await?;
        let (ver, info, ver_str) = Self::get_best_pypi_release(clang_tool, &info, version)?;
        let cached_filename = format!("{clang_tool}_{ver_str}.whl",);
        let cached_dir = Self::get_cache_dir();
        let cached_wheel = cached_dir.join("pypi").join(&cached_filename);
        let file_lock = lock_path(&cached_wheel)?;
        if Self::is_cache_valid(&cached_wheel, None) {
            log::info!(
                "Using cached wheel for {clang_tool} version {ver_str} from {}",
                cached_wheel.to_string_lossy()
            );
        } else {
            log::info!(
                "Downloading {clang_tool} version {ver_str} from {}",
                info.url
            );
            download(&Url::parse(&info.url)?, &cached_wheel, 60).await?;
        }
        if let Some(digest) = info.digests.first() {
            log::info!("Verifying wheel file integrity with digest: {digest:?}");
            digest.verify(&cached_wheel)?;
        }
        let bin_name = format!(
            "{clang_tool}-{}{}",
            ver.major,
            if cfg!(windows) { ".exe" } else { "" }
        );
        let extracted_bin = match directory {
            None => cached_dir.join(format!("bin/{bin_name}",)),
            Some(dir) => dir.join(&bin_name),
        };
        Self::extract_bin(clang_tool, &cached_wheel, &extracted_bin)?;
        if clang_tool.needs_resource_headers()
            && !resource_headers::has_resource_headers(&extracted_bin, ver.major)
        {
            log::info!("Extracting clang resource headers from the {clang_tool} wheel");
            let package = clang_tool.as_str().replace('-', "_");
            match resource_headers::extract_from_wheel(&cached_wheel, &package, &extracted_bin) {
                Err(ResourceHeadersError::NotFound(_)) => log::warn!(
                    "The {clang_tool} v{ver_str} wheel has no clang resource headers (like stddef.h). \
                    {clang_tool} may fail to parse sources that include standard headers."
                ),
                result => result?,
            }
        }
        file_lock.unlock()?;
        Ok(extracted_bin)
    }

    fn extract_bin(
        clang_tool: &ClangTool,
        wheel_path: &PathBuf,
        extracted_bin: &PathBuf,
    ) -> Result<(), PyPiDownloadError> {
        let mut archive = fs::File::open(wheel_path)
            .map_err(ZipError::from)
            .and_then(ZipArchive::new)?;
        let expected_zip_path = format!(
            "{}/data/bin/{}{}",
            clang_tool.as_str().replace('-', "_"),
            clang_tool.as_str(),
            if cfg!(windows) { ".exe" } else { "" }
        );
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file
                .enclosed_name()
                .is_some_and(|n| n == Path::new(&expected_zip_path))
            {
                if extracted_bin.exists() {
                    let meta = fs::metadata(extracted_bin)?;
                    if meta.len() == file.size() {
                        return Ok(());
                    }
                }
                if let Some(parent) = extracted_bin.parent() {
                    fs::create_dir_all(parent)?;
                }
                let file_size = NonZero::new(file.size());
                let mut out = fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(extracted_bin)?;
                let mut buffer = [0; EXTRACTED_CHUNK_SIZE as usize];
                let mut total_extracted = 0;
                let mut progress_bar = ProgressBar::new(file_size, "Extracting binary from wheel");
                progress_bar.render()?;
                loop {
                    let bytes_read = file.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    total_extracted += bytes_read as u64;
                    out.write_all(&buffer[..bytes_read])?;
                    progress_bar.inc(bytes_read as u64)?;
                    if let Some(total_size) = file_size
                        && total_extracted >= total_size.get()
                    {
                        break;
                    }
                }
                progress_bar.finish()?;
                #[cfg(unix)]
                {
                    // Make the extracted binary executable on Unix-like systems.
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = out.metadata()?.permissions();
                    perms.set_mode(0o755);
                    out.set_permissions(perms)?;
                }
                return Ok(());
            }
        }
        log::error!("Failed to find expected binary in the wheel: {expected_zip_path}");
        Err(PyPiDownloadError::ExecutableNotFound)
    }
}

const EXTRACTED_CHUNK_SIZE: u64 = 1024;

#[cfg(test)]
mod test {
    use std::{fs, io::Write, path::Path, str::FromStr};

    use semver::VersionReq;
    use tempfile::TempDir;

    use super::{
        LinuxLibC, PlatformOs, PlatformTag, PyPiDownloadError, PyPiDownloader, PyPiProjectInfo,
        PyPiReleaseInfo, WheelTags,
    };
    use crate::ClangTool;

    #[test]
    fn bad_json_digest() {
        let json = r#"
        {
            "url": "https://files.pythonhosted.org/packages/xx/yy/clang_format-17.0.0-py3-none-manylinux_2_17_x86_64.whl",
            "filename": "clang_format-17.0.0-py3-none-manylinux_2_17_x86_64.whl",
            "size": 12345678,
            "digests": ["sha256"],
            "yanked": false
        }
        "#;
        let result = serde_json::from_str::<PyPiReleaseInfo>(json).unwrap_err();
        println!("{}", result);
    }

    #[test]
    fn manylinux1_tag() {
        let tag = "manylinux1_x86_64";
        let platform_tag = PlatformTag::from_str(tag).unwrap();
        assert_eq!(platform_tag.arch.as_str(), "x86_64");

        let bad_tag = "manylinux1-x86-64";
        let err = PlatformTag::from_str(bad_tag).unwrap_err();
        println!("{}", err);
    }

    #[test]
    fn unknown_platform_tag() {
        let bad_tag = "unknown_platform";
        let err = PlatformTag::from_str(bad_tag).unwrap_err();
        println!("{}", err);
    }

    #[test]
    fn bad_wheel_tags() {
        // should have at least 5 hyphenated segments.
        let bad_wheel_name = "clang_format-17.0.0-py3-none_manylinux_2_17_x86_64.whl";
        let err = WheelTags::from_str(bad_wheel_name).unwrap_err();
        println!("{}", err);
    }

    #[test]
    fn win32_tag() {
        let tag = PlatformTag::from_str("win32").unwrap();
        assert!(matches!(tag.os, PlatformOs::Windows));
        assert_eq!(tag.arch.as_str(), "x86");
    }

    #[test]
    fn win_amd64_tag() {
        let tag = PlatformTag::from_str("win_amd64").unwrap();
        assert!(matches!(tag.os, PlatformOs::Windows));
        assert_eq!(tag.arch.as_str(), "amd64");
    }

    #[test]
    fn arch_compatibility() {
        let compatible = |tag: &str, sys_os: &str, sys_arch: &str| {
            PlatformTag::from_str(tag)
                .unwrap()
                .is_arch_compatible(sys_os, sys_arch)
        };
        for (tag, sys_os, sys_arch, expected) in [
            ("win_amd64", "windows", "x86_64", true),
            ("win_arm64", "windows", "aarch64", true),
            ("win32", "windows", "x86", true),
            ("win32", "windows", "x86_64", false),
            ("win_amd64", "windows", "aarch64", false),
            ("win_amd64", "windows", "riscv64", false),
            ("macosx_10_9_x86_64", "macos", "x86_64", true),
            ("macosx_11_0_arm64", "macos", "aarch64", true),
            ("macosx_10_9_universal2", "macos", "x86_64", true),
            ("macosx_10_9_universal2", "macos", "aarch64", true),
            ("macosx_11_0_arm64", "macos", "x86_64", false),
            ("macosx_10_9_x86_64", "macos", "aarch64", false),
            ("macosx_10_9_universal2", "macos", "powerpc", false),
            ("manylinux_2_17_x86_64", "linux", "x86_64", true),
            ("manylinux_2_17_aarch64", "linux", "aarch64", true),
            ("manylinux_2_17_x86_64", "linux", "aarch64", false),
            ("manylinux_2_17_x86_64", "freebsd", "x86_64", false),
        ] {
            assert_eq!(
                compatible(tag, sys_os, sys_arch),
                expected,
                "{tag} on {sys_os} {sys_arch}"
            );
        }
    }

    #[test]
    fn win_missing_arch_tag() {
        // "win" prefix without an "_<arch>" suffix is malformed.
        let err = PlatformTag::from_str("win").unwrap_err();
        println!("{}", err);
    }

    #[test]
    fn macosx_tag() {
        let tag = PlatformTag::from_str("macosx_11_0_arm64").unwrap();
        assert!(matches!(tag.os, PlatformOs::MacOS));
        assert_eq!(tag.arch.as_str(), "arm64");
    }

    #[test]
    fn musllinux_tag() {
        let tag = PlatformTag::from_str("musllinux_1_2_x86_64").unwrap();
        assert!(matches!(
            tag.os,
            PlatformOs::Linux {
                lib_c: LinuxLibC::Musl { .. }
            }
        ));
        assert_eq!(tag.arch.as_str(), "x86_64");
    }

    #[test]
    fn manylinux2_tag() {
        let tag = PlatformTag::from_str("manylinux_2_17_x86_64").unwrap();
        assert!(matches!(
            tag.os,
            PlatformOs::Linux {
                lib_c: LinuxLibC::Glibc { .. }
            }
        ));
        assert_eq!(tag.arch.as_str(), "x86_64");
    }

    #[test]
    fn manylinux_dotted_tag() {
        // A combined tag like "manylinux_2_17_x86_64.manylinux2014_x86_64" should
        // select the "manylinux_" variant from the dotted alternatives.
        let tag = PlatformTag::from_str("manylinux_2_17_x86_64.manylinux2014_x86_64").unwrap();
        assert!(matches!(
            tag.os,
            PlatformOs::Linux {
                lib_c: LinuxLibC::Glibc { .. }
            }
        ));
        assert_eq!(tag.arch.as_str(), "x86_64");
    }

    #[test]
    fn valid_wheel_tags() {
        let wheel_name = "clang_format-17.0.0-py3-none-manylinux_2_17_x86_64.whl";
        let tags = WheelTags::from_str(wheel_name).unwrap();
        assert_eq!(tags.platform.arch.as_str(), "x86_64");
    }

    #[test]
    fn skip_malformed_version() {
        log::set_max_level(log::LevelFilter::Trace);
        let json = serde_json::json!({ "releases": { "1.0": [], "not-a-version": [] } });
        let info = serde_json::from_value::<PyPiProjectInfo>(json).unwrap();
        let err = PyPiDownloader::get_best_pypi_release(
            &ClangTool::ClangFormat,
            &info,
            &VersionReq::STAR,
        )
        .unwrap_err();
        assert!(matches!(err, PyPiDownloadError::NoVersionFound));
    }

    const BIN_PATH_IN_WHEEL: &str = if cfg!(windows) {
        "clang_format/data/bin/clang-format.exe"
    } else {
        "clang_format/data/bin/clang-format"
    };

    /// Creates a (uncompressed) zip file at `path` with the given `entries` (name, content).
    fn make_wheel(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, content) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn extract_bin() {
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("test.whl");
        // larger than the chunk size used for extraction
        let content = vec![b'a'; 3000];
        make_wheel(
            &wheel,
            &[("other.txt", b"ignored"), (BIN_PATH_IN_WHEEL, &content)],
        );
        let extracted = tmp.path().join("bin").join("clang-format-17");
        PyPiDownloader::extract_bin(&ClangTool::ClangFormat, &wheel, &extracted).unwrap();
        assert_eq!(fs::read(&extracted).unwrap(), content);
        // extracting again is skipped because the existing file is the same size.
        PyPiDownloader::extract_bin(&ClangTool::ClangFormat, &wheel, &extracted).unwrap();
        assert_eq!(fs::read(&extracted).unwrap(), content);
    }

    #[test]
    fn extract_bin_overwrite() {
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("test.whl");
        make_wheel(&wheel, &[(BIN_PATH_IN_WHEEL, b"new content")]);
        let extracted = tmp.path().join("clang-format-17");
        // an existing file of a different size is overwritten
        fs::write(&extracted, b"old").unwrap();
        PyPiDownloader::extract_bin(&ClangTool::ClangFormat, &wheel, &extracted).unwrap();
        assert_eq!(fs::read(&extracted).unwrap(), b"new content");
    }

    #[test]
    fn extract_empty_bin() {
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("test.whl");
        make_wheel(&wheel, &[(BIN_PATH_IN_WHEEL, b"")]);
        let extracted = tmp.path().join("clang-format-17");
        PyPiDownloader::extract_bin(&ClangTool::ClangFormat, &wheel, &extracted).unwrap();
        assert!(fs::read(&extracted).unwrap().is_empty());
    }

    #[test]
    fn extract_missing_bin() {
        log::set_max_level(log::LevelFilter::Trace);
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("test.whl");
        make_wheel(
            &wheel,
            &[("clang_format/data/bin/clang-tidy", b"wrong tool")],
        );
        let extracted = tmp.path().join("clang-format-17");
        let err =
            PyPiDownloader::extract_bin(&ClangTool::ClangFormat, &wheel, &extracted).unwrap_err();
        assert!(matches!(err, PyPiDownloadError::ExecutableNotFound));
        assert!(!extracted.exists());
    }

    /// These tests use a fake `ldd` executable in an isolated `PATH`.
    #[cfg(target_os = "linux")]
    mod fake_ldd {
        use std::fs;

        use semver::{Version, VersionReq};
        use tempfile::TempDir;

        use super::{BIN_PATH_IN_WHEEL, make_wheel};
        use crate::{
            Cacher, ClangTool, PyPiDownloader,
            downloader::{pypi::LinuxLibC, resource_headers},
            test_utils_unix::{enable_logs, fake_exe, isolate_path, set_cache_dir},
        };

        fn setup(ldd_output: Option<&str>) -> TempDir {
            enable_logs();
            let tmp = TempDir::new().unwrap();
            isolate_path(tmp.path());
            if let Some(out) = ldd_output {
                fake_exe(tmp.path(), "ldd", &format!("echo \"{out}\""));
            }
            tmp
        }

        #[test]
        fn musl() {
            let _tmp = setup(Some(
                "musl libc (x86_64)\nVersion 1.2.2\nDynamic Program Loader",
            ));
            assert_eq!(LinuxLibC::get_musl_version(), Some(Version::new(1, 2, 2)));
            assert_eq!(LinuxLibC::get_glibc_version(), None);
            let musl = |minor| LinuxLibC::Musl {
                version: Version::new(1, minor, 0),
            };
            assert!(musl(2).is_compatible_with_system());
            assert!(!musl(3).is_compatible_with_system());
            let glibc = LinuxLibC::Glibc {
                version: Version::new(2, 17, 0),
            };
            assert!(!glibc.is_compatible_with_system());
        }

        #[test]
        fn glibc() {
            let _tmp = setup(Some(
                "ldd (Ubuntu GLIBC 2.39-0ubuntu8.7) 2.39\nCopyright (C) 2024",
            ));
            assert_eq!(LinuxLibC::get_musl_version(), None);
            assert_eq!(LinuxLibC::get_glibc_version(), Some(Version::new(2, 39, 0)));
        }

        #[test]
        fn no_ldd() {
            let _tmp = setup(None);
            assert_eq!(LinuxLibC::get_musl_version(), None);
            assert_eq!(LinuxLibC::get_glibc_version(), None);
        }

        /// Seeds the cache with PyPI info about `tool` v17.0.6 (without digest)
        /// and its wheel (with the given `entries`), so no network access is needed.
        fn seed_cached_wheel(tool: &ClangTool, entries: &[(&str, &[u8])]) {
            let pypi_cache = PyPiDownloader::get_cache_dir().join("pypi");
            fs::create_dir_all(&pypi_cache).unwrap();
            let package = tool.as_str().replace('-', "_");
            let arch = std::env::consts::ARCH;
            let filename = format!("{package}-17.0.6-py2.py3-none-manylinux_2_17_{arch}.whl");
            let json = serde_json::json!({
                "releases": {
                    "17.0.6": [{
                        "url": format!("https://example.invalid/{filename}"),
                        "filename": filename,
                        "size": 1,
                        "digests": {},
                        "yanked": false
                    }]
                }
            });
            fs::write(
                pypi_cache.join(format!("{tool}_pypi.json")),
                json.to_string(),
            )
            .unwrap();
            make_wheel(&pypi_cache.join(format!("{tool}_17.0.6.whl")), entries);
        }

        #[tokio::test]
        async fn download_cached_wheel_without_digest() {
            let tmp = setup(Some("ldd (Ubuntu GLIBC 2.39-0ubuntu8.7) 2.39"));
            let cache = TempDir::new().unwrap();
            set_cache_dir(cache.path());
            let content = b"#!/bin/sh\necho \"clang-format version 17.0.6\"\n";
            seed_cached_wheel(&ClangTool::ClangFormat, &[(BIN_PATH_IN_WHEEL, content)]);
            let out_dir = tmp.path().join("out");
            let req = VersionReq::parse("17").unwrap();
            for _ in 0..2 {
                let bin =
                    PyPiDownloader::download_tool(&ClangTool::ClangFormat, &req, Some(&out_dir))
                        .await
                        .unwrap();
                assert_eq!(bin, out_dir.join("clang-format-17"));
                assert_eq!(fs::read(&bin).unwrap(), content);
            }
            // clang-format does not need the resource headers
            assert!(!tmp.path().join("lib").exists());
        }

        #[tokio::test]
        async fn download_tidy_with_resource_headers() {
            let tmp = setup(Some("ldd (Ubuntu GLIBC 2.39-0ubuntu8.7) 2.39"));
            let cache = TempDir::new().unwrap();
            set_cache_dir(cache.path());
            let header = "clang_tidy/data/lib/clang/17/include/stddef.h";
            seed_cached_wheel(
                &ClangTool::ClangTidy,
                &[("clang_tidy/data/bin/clang-tidy", b"bin"), (header, b"h")],
            );
            let bin_dir = tmp.path().join("bin");
            let req = VersionReq::parse("17").unwrap();
            let bin = PyPiDownloader::download_tool(&ClangTool::ClangTidy, &req, Some(&bin_dir))
                .await
                .unwrap();
            assert!(resource_headers::has_resource_headers(&bin, 17));
            assert!(tmp.path().join("lib/clang/17/include/stddef.h").is_file());
        }

        #[tokio::test]
        async fn download_tidy_without_resource_headers() {
            let tmp = setup(Some("ldd (Ubuntu GLIBC 2.39-0ubuntu8.7) 2.39"));
            let cache = TempDir::new().unwrap();
            set_cache_dir(cache.path());
            seed_cached_wheel(
                &ClangTool::ClangTidy,
                &[("clang_tidy/data/bin/clang-tidy", b"bin")],
            );
            let bin_dir = tmp.path().join("bin");
            let req = VersionReq::parse("17").unwrap();
            // only a warning is logged about the missing headers
            let bin = PyPiDownloader::download_tool(&ClangTool::ClangTidy, &req, Some(&bin_dir))
                .await
                .unwrap();
            assert_eq!(fs::read(&bin).unwrap(), b"bin");
            assert!(!resource_headers::has_resource_headers(&bin, 17));
        }
    }
}
