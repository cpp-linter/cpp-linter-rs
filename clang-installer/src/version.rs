use std::{path::PathBuf, str::FromStr};

use crate::utils::normalize_path;
use semver::VersionReq;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RequestedVersion {
    /// A specific path to the clang tool binary.
    Path(PathBuf),

    /// Whatever the system default uses (if any).
    #[default]
    SystemDefault,

    /// A specific version requirement for the clang tool binary.
    ///
    /// For example, `=12.0.1`, `>=10.0.0, <13.0.0`.
    Requirement(VersionReq),

    /// A sentinel when no value is given.
    ///
    /// This is used internally to differentiate when the user intended
    /// to invoke the `version` subcommand instead.
    NoValue,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestedVersionParsingError {
    #[error("The specified version is not a proper version requirement or a valid path: {0}")]
    InvalidInput(String),
    #[error("Unknown parent directory of the given file path for `--version`: {0}")]
    InvalidPath(String),
    #[error("Failed to canonicalize path '{0}': {1:?}")]
    NonCanonicalPath(String, std::io::Error),
}

impl FromStr for RequestedVersion {
    type Err = RequestedVersionParsingError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            Ok(Self::SystemDefault)
        } else if input == "CPP-LINTER-VERSION" {
            Ok(Self::NoValue)
        } else if let Ok(req) = VersionReq::parse(input) {
            Ok(Self::Requirement(req))
        } else {
            let path = PathBuf::from(input);
            if !path.exists() {
                return Err(RequestedVersionParsingError::InvalidInput(
                    input.to_string(),
                ));
            }
            let path = if !path.is_dir() {
                path.parent()
                    .ok_or(RequestedVersionParsingError::InvalidPath(input.to_string()))?
                    .to_path_buf()
            } else {
                path
            };
            let path = match path.canonicalize() {
                Ok(p) => Ok(normalize_path(&p)),
                Err(e) => Err(RequestedVersionParsingError::NonCanonicalPath(
                    input.to_string(),
                    e,
                )),
            }?;
            Ok(Self::Path(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    use super::RequestedVersion;
    use crate::utils::normalize_path;

    // See also crate::tool::tests module for other `RequestedVersion::from_str()` tests.

    #[test]
    fn validate_version_path() {
        let this_path_str = "src/version.rs";
        let this_path = PathBuf::from(this_path_str);
        let this_canonical = this_path.canonicalize().unwrap();
        let parent = this_canonical.parent().unwrap();
        let expected = normalize_path(parent);
        let req_ver = RequestedVersion::from_str(this_path_str).unwrap();
        if let RequestedVersion::Path(parsed) = req_ver {
            assert_eq!(&parsed, &expected);
        }

        assert!(RequestedVersion::from_str("file.rs").is_err());
    }

    #[test]
    fn validate_version_exact() {
        let req_ver = RequestedVersion::from_str("12").unwrap();
        if let RequestedVersion::Requirement(req) = req_ver {
            assert_eq!(req.to_string(), "^12");
        }
    }
}
