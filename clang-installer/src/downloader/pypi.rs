use super::{DownloadError, caching::Cacher, download, hashing::HashAlgorithm};
use crate::{ClangTool, progress_bar::ProgressBar};

use semver::{Version, VersionReq};
use serde::{Deserialize, de::Visitor};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    num::NonZero,
    path::PathBuf,
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
    /// Checks if the [LinuxLibC] is compatible with the current system.
    pub fn is_compatible_with_system(&self) -> bool {
        match self {
            #[cfg(target_env = "musl")]
            LinuxLibC::Musl { .. } => true,
            #[cfg(not(target_env = "musl"))]
            LinuxLibC::Glibc { .. } => true,
            _ => false,
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
            PlatformOs::Windows => std::env::consts::OS == "windows",
            PlatformOs::MacOS => std::env::consts::OS == "macos",
            PlatformOs::Linux { lib_c } => {
                std::env::consts::OS == "linux" && lib_c.is_compatible_with_system()
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
        self.os.is_compatible_with_system() && {
            let sys_arch = std::env::consts::ARCH;
            match std::env::consts::OS {
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
    /// The version of the package for which the wheel is built.
    ///
    /// For clang-format and clang-tidy wheels, this corresponds to the version of the clang tool.
    version: Version,

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

        // The exact version (PEP 440 compatible) should comply with semver parsing.
        let version = Version::parse(
            iter.next()
                .ok_or(PyPiDownloadError::InvalidWheelName(s.to_string()))?,
        )
        .map_err(|_| PyPiDownloadError::InvalidVersion)?;
        let platform = PlatformTag::from_str(iter.next_back().unwrap())?;

        // The remaining tags are not used for compatibility checks.
        // These binary wheels come with the executable within, so
        // we don't need to validate python version nor abi tags here.
        // optional build tag is not used in clang_format or clang_tidy wheel deployments

        Ok(Self { version, platform })
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
    ) -> Result<(Version, PyPiReleaseInfo), PyPiDownloadError> {
        let mut result = None;

        for (ver_str, releases) in &pypi_info.releases {
            let ver = match Version::parse(ver_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if version.matches(&ver) {
                for release in releases {
                    if !release.filename.ends_with(".whl") {
                        continue;
                    }
                    let wheel_tags = WheelTags::from_str(&release.filename)?;
                    if !release.yanked && wheel_tags.is_compatible_with_system() {
                        log::debug!(
                            "Found {clang_tool} (size: {}, digest: {:?}); {wheel_tags:?}",
                            release.size,
                            release.digests
                        );
                        if result.as_ref().is_none_or(|(v, _)| *v < ver) {
                            result = Some((wheel_tags.version.clone(), release));
                        }
                    }
                }
            }
        }
        result
            .map(|(a, b)| (a, b.to_owned()))
            .ok_or(PyPiDownloadError::NoVersionFound)
    }

    async fn get_pypi_release_info(
        clang_tool: &ClangTool,
    ) -> Result<PyPiProjectInfo, PyPiDownloadError> {
        let cache_file = Self::get_cache_dir()
            .join("pypi")
            .join(format!("{clang_tool}_pypi.json"));

        // PyPI package info cache should not be refreshed unless it is more than 10 minutes old.
        // This is behavior recommended by PyPI response header `Cache-Control: max-age=600`.
        // Instead of caching the `Cache-Control` header, we'll just check the cached file's "last modified" time.
        let cache_valid = Self::is_cache_valid(&cache_file, Some(Duration::from_mins(10)));
        let body = if cache_valid {
            log::info!(
                "Using cached PyPI info for {clang_tool} from {}",
                cache_file.to_string_lossy()
            );
            std::fs::read_to_string(cache_file)?
        } else {
            let api_url = format!("{PYPI_JSON_API_URL}/pypi/{clang_tool}/");
            let endpoint = Url::parse(&api_url)?.join("json")?;
            log::info!("Fetching PyPI info for {clang_tool} from {endpoint}");
            download(&endpoint, &cache_file, 10).await?;
            std::fs::read_to_string(cache_file)?
        };
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
        let (ver, info) = Self::get_best_pypi_release(clang_tool, &info, version)?;
        let cached_filename = format!("{clang_tool}_{ver}.whl");
        let cached_dir = Self::get_cache_dir();
        let cached_wheel = cached_dir.join("pypi").join(&cached_filename);
        if Self::is_cache_valid(&cached_wheel, None) {
            log::info!(
                "Using cached wheel for {clang_tool} version {ver} from {}",
                cached_wheel.to_string_lossy()
            );
        } else {
            log::info!("Downloading {clang_tool} version {ver} from {}", info.url);
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
            if file.name() == expected_zip_path {
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
    use std::str::FromStr;

    use super::{PlatformTag, PyPiReleaseInfo, WheelTags};

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
        println!("{}", result.to_string());
    }

    #[test]
    fn manylinux1_tag() {
        let tag = "manylinux1_x86_64";
        let platform_tag = PlatformTag::from_str(tag).unwrap();
        assert_eq!(platform_tag.arch.as_str(), "x86_64");

        let bad_tag = "manylinux1-x86-64";
        let err = PlatformTag::from_str(bad_tag).unwrap_err();
        println!("{}", err.to_string());
    }

    #[test]
    fn unknown_platform_tag() {
        let bad_tag = "unknown_platform";
        let err = PlatformTag::from_str(bad_tag).unwrap_err();
        println!("{}", err.to_string());
    }

    #[test]
    fn bad_wheel_tags() {
        // should have at least 5 hyphenated segments.
        let bad_wheel_name = "clang_format-17.0.0-py3-none_manylinux_2_17_x86_64.whl";
        let err = WheelTags::from_str(bad_wheel_name).unwrap_err();
        println!("{}", err.to_string());
    }
}
