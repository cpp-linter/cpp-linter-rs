use crate::ClangTool;
use reqwest::{ClientBuilder, Url};
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::HashMap;
#[cfg(test)]
use std::env;
use std::{str::FromStr, time::Duration};

#[derive(Debug)]
pub enum DownloaderError {
    /// Errors that occur during HTTP requests.
    Request(reqwest::Error),

    /// Errors that occur when parsing version strings.
    InvalidVersion,

    /// Errors that occur when deserializing JSON responses.
    Deserialization(serde_json::Error),

    /// Errors that occur when parsing wheel filenames.
    InvalidWheelName(String),
}

/// Represents the information of a package on PyPI
#[derive(Debug, Deserialize)]
pub struct PyPiProjectInfo {
    /// A mapping from version strings to a list of release information for that version.
    releases: HashMap<String, Vec<PyPiReleaseInfo>>,
}

/// Represents the information of a single release of a package on PyPI.
#[derive(Debug, Deserialize)]
pub struct PyPiReleaseInfo {
    /// The URL to download the release.
    url: String,

    /// The filename of the release.
    filename: String,

    /// The size of the release in bytes.
    size: u64,

    /// A mapping from digest algorithm names to their corresponding hash values.
    digests: HashMap<String, String>,

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
                    "x86_64" => self.arch == "x86_64",
                    "aarch64" => self.arch == "arm64",
                    _ => false,
                },
                "linux" => self.arch == sys_arch,
                _ => false,
            }
        }
    }
}

impl FromStr for PlatformTag {
    type Err = DownloaderError;

    /// Parses the platform tag from a wheel filename.
    ///
    /// The input string can be the platform tag itself, not the entire wheel filename.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = if s.contains(".manylinux") {
            s.split('.')
                .find(|part| part.starts_with("manylinux_"))
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?
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
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?;
            Ok(Self {
                os: PlatformOs::Windows,
                arch: arch.to_string(),
            })
        } else if s.starts_with("manylinux1") {
            let (_, arch) = s
                .split_once('_')
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?;
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
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?;
            let lib_c_ver = Version::new(
                parts
                    .next()
                    .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?
                    .parse::<u64>()
                    .unwrap_or(1),
                parts
                    .next()
                    .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?
                    .parse::<u64>()
                    .unwrap_or(1),
                0,
            );
            let arch = parts
                .next()
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?;
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
            Err(DownloaderError::InvalidWheelName(s.to_string()))
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
    type Err = DownloaderError;

    /// Parses a wheel filename into its tags.
    ///
    /// Does not support source distribution names.
    /// Only keeps information relevant to system compatibility checks.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.trim_end_matches(".whl").split('-');
        let tags_len = iter.clone().count();
        if tags_len < 5 {
            return Err(DownloaderError::InvalidWheelName(s.to_string()));
        }
        iter.next(); // already know the package name

        // The exact version (PEP 440 compatible) should comply with semver parsing.
        let version = Version::parse(
            iter.next()
                .ok_or(DownloaderError::InvalidWheelName(s.to_string()))?,
        )
        .map_err(|_| DownloaderError::InvalidVersion)?;
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

impl ClangTool {
    /// Returns the best available release info from PyPI for the given tool and minimum version.
    fn parse_info(
        &self,
        body: &str,
        version: &str,
    ) -> Result<(Version, PyPiReleaseInfo), DownloaderError> {
        let pypi_info: PyPiProjectInfo =
            serde_json::from_str(body).map_err(DownloaderError::Deserialization)?;
        let mut result = None;
        let version_req = VersionReq::parse(
            format!("{}{version}", if version.is_empty() { "*" } else { "=" }).as_str(),
        )
        .map_err(|_| DownloaderError::InvalidVersion)?;

        for (ver_str, releases) in pypi_info.releases {
            let ver = match Version::parse(&ver_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if version_req.matches(&ver) {
                for release in releases {
                    if !release.filename.ends_with(".whl") {
                        continue;
                    }
                    let wheel_tags = WheelTags::from_str(&release.filename)?;
                    if !release.yanked && wheel_tags.is_compatible_with_system() {
                        let sha256 = release
                            .digests
                            .get("sha256")
                            .map(|v| v.as_str())
                            .unwrap_or("None");
                        #[cfg(test)]
                        println!(
                            "Found {self} (size: {}, sha256: {sha256}); {wheel_tags:?}",
                            release.size
                        );
                        log::info!(
                            "Found {self} (size: {}, sha256: {sha256}); {wheel_tags:?}",
                            release.size
                        );
                        if result.as_ref().is_none_or(|(v, _)| *v < ver) {
                            result = Some((wheel_tags.version.clone(), release));
                        }
                    }
                }
            }
        }
        result.ok_or(DownloaderError::InvalidVersion)
    }

    pub async fn download(&self, version: &str) -> Result<Option<Url>, DownloaderError> {
        #[cfg(not(test))]
        let pypi_json_api_url = "https://pypi.org";
        #[cfg(test)]
        let pypi_json_api_url = env::var("CPP_LINTER_TEST_PYPI_API_URL")
            .expect("TEST_PYPI_API_URL must be set for tests");

        let api_url = format!("{pypi_json_api_url}/pypi/{self}/");
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(DownloaderError::Request)?;
        let endpoint = Url::parse(&api_url).unwrap().join("json").unwrap();
        let response = client
            .get(endpoint)
            .send()
            .await
            .map_err(DownloaderError::Request)?;
        if let Err(e) = response.error_for_status_ref() {
            let body = response.text().await.unwrap_or_default();
            log::error!("Failed to fetch info from pypi.org: {}", body);
            return Err(DownloaderError::Request(e));
        }
        let body = response.text().await.map_err(DownloaderError::Request)?;
        let (ver, info) = self.parse_info(&body, version)?;
        log::info!("Downloading {self} version {ver} from {}", info.url);
        let reqwest = client
            .get(Url::parse(&info.url).unwrap())
            .send()
            .await
            .map_err(DownloaderError::Request)?;
        if let Err(e) = reqwest.error_for_status_ref() {
            let body = reqwest.text().await.unwrap_or_default();
            log::error!("Failed to download {self} wheel from {}:\n{body}", info.url,);
            return Err(DownloaderError::Request(e));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use std::{fs, path::Path};

    use super::*;

    // #[tokio::test]
    // async fn test_download_clang_format() {
    //     let downloader = ClangToolDownloader::new(
    //         ClangTool::ClangFormat,
    //         Version::parse("15.0.7").unwrap(),
    //     );
    //     let result = downloader.download().await;
    //     assert!(result.is_ok());
    // }

    #[test]
    fn deserialize_pypi_json() {
        let asset = Path::new("tests/pypi_clang-format.json");
        let content = fs::read_to_string(asset).unwrap();
        let version = "21";
        let (ver, pypi_info) = ClangTool::ClangFormat
            .parse_info(&content, version)
            .unwrap();
        println!("{ver}: {:#?}", pypi_info);
    }

    #[test]
    fn deserialize_pypi_json_latest() {
        let asset = Path::new("tests/pypi_clang-format.json");
        let content = fs::read_to_string(asset).unwrap();
        let version = "";
        let (ver, pypi_info) = ClangTool::ClangFormat
            .parse_info(&content, version)
            .unwrap();
        println!("{ver}: {:#?}", pypi_info);
    }
}
