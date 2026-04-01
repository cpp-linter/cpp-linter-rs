use super::DownloadError;

use std::{fs, io::Read, path::Path};

/// Represents the supported hash algorithms for file integrity checking.
///
/// Each variant holds the expected checksum value as a string,
/// which is used for verification.
///
/// Note, MD5 is intentionally excluded due to security weaknesses.
/// As such, MD5 is not recommended for file integrity checks.
#[derive(Debug, Clone)]
pub enum HashAlgorithm {
    /// SHA-256 hash algorithm with the expected checksum value.
    Sha256(String),
    /// BLAKE2b-256 hash algorithm with the expected checksum value.
    Blake2b256(String),
    /// SHA-512 hash algorithm with the expected checksum value.
    Sha512(String),
}

impl HashAlgorithm {
    /// Verify a given file (located at `file_path`) against the expected checksum value.
    ///
    /// This method reads the file in chunks (of 1024 bytes) to compute the hash,
    /// thus no extraneous memory is allocated when reading the file's entire contents.
    ///
    /// Note, a progress bar is displayed to stdout.
    pub fn verify(&self, file_path: &Path) -> Result<(), DownloadError> {
        let mut file_reader = fs::OpenOptions::new().read(true).open(file_path)?;
        let mut buf = [0u8; 1024];
        match self {
            HashAlgorithm::Sha256(expected) => {
                use sha2::{Digest, Sha256};

                let mut hasher = Sha256::new();
                loop {
                    let bytes_read = file_reader.read(&mut buf)?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buf[..bytes_read]);
                }
                let actual = format!("{:x}", hasher.finalize());
                if actual == *expected {
                    Ok(())
                } else {
                    Err(DownloadError::HashMismatch {
                        expected: expected.clone(),
                        actual,
                    })
                }
            }
            HashAlgorithm::Blake2b256(expected) => {
                use blake2::{Blake2b, Digest, digest::consts::U32};

                let mut hasher = Blake2b::<U32>::new();
                loop {
                    let bytes_read = file_reader.read(&mut buf)?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buf[..bytes_read]);
                }
                let actual = format!("{:x}", hasher.finalize());
                if actual == *expected {
                    Ok(())
                } else {
                    Err(DownloadError::HashMismatch {
                        expected: expected.clone(),
                        actual,
                    })
                }
            }
            HashAlgorithm::Sha512(expected) => {
                use sha2::{Digest, Sha512};

                let mut hasher = Sha512::new();

                loop {
                    let bytes_read = file_reader.read(&mut buf)?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.update(&buf[..bytes_read]);
                }
                let actual = format!("{:x}", hasher.finalize());
                if actual == *expected {
                    Ok(())
                } else {
                    Err(DownloadError::HashMismatch {
                        expected: expected.clone(),
                        actual,
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use super::HashAlgorithm;
    use tempfile::NamedTempFile;

    const CONTENT: &[u8] = b"hello world";
    const INVALID_CHECKSUM: &str = "deadbeef";

    #[test]
    fn sha256() {
        use sha2::Digest;

        let mut temp_file = NamedTempFile::new().unwrap();
        fs::write(&mut temp_file, CONTENT).unwrap();
        let hasher = sha2::Sha256::new().chain_update(CONTENT);
        let expected = format!("{:x}", hasher.finalize());
        let hash_algorithm = HashAlgorithm::Sha256(expected);
        assert!(hash_algorithm.verify(temp_file.path()).is_ok());
        assert!(
            HashAlgorithm::Sha256(INVALID_CHECKSUM.to_string())
                .verify(temp_file.path())
                .is_err()
        );
    }

    #[test]
    fn sha512() {
        use sha2::Digest;

        let mut temp_file = NamedTempFile::new().unwrap();
        fs::write(&mut temp_file, CONTENT).unwrap();
        let hasher = sha2::Sha512::new().chain_update(CONTENT);
        let expected = format!("{:x}", hasher.finalize());
        let hash_algorithm = HashAlgorithm::Sha512(expected);
        assert!(hash_algorithm.verify(temp_file.path()).is_ok());
        assert!(
            HashAlgorithm::Sha512(INVALID_CHECKSUM.to_string())
                .verify(temp_file.path())
                .is_err()
        );
    }

    #[test]
    fn blake2b256() {
        use blake2::{Blake2b, Digest, digest::consts::U32};

        let mut temp_file = NamedTempFile::new().unwrap();
        fs::write(&mut temp_file, CONTENT).unwrap();
        let hasher = Blake2b::<U32>::new().chain_update(CONTENT);
        let expected = format!("{:x}", hasher.finalize());
        let hash_algorithm = HashAlgorithm::Blake2b256(expected);
        assert!(hash_algorithm.verify(temp_file.path()).is_ok());
        assert!(
            HashAlgorithm::Blake2b256(INVALID_CHECKSUM.to_string())
                .verify(temp_file.path())
                .is_err()
        );
    }
}
