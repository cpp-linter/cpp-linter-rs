//! A module to install clang's resource headers next to a downloaded clang-tidy binary.
//!
//! clang-tidy needs clang's builtin headers (like `stddef.h`) to parse C/C++ sources.
//! It looks for them in `<bin_dir>/../lib/clang/<version>/include`, where `<bin_dir>`
//! is the folder that holds the clang-tidy executable.
//! Without these headers, any source that includes a standard header fails to compile.

use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use flate2::read::GzDecoder;
use zip::{ZipArchive, result::ZipError};

use crate::utils::normalize_path;

/// Errors that occur while installing clang's resource headers.
#[derive(Debug, thiserror::Error)]
pub enum ResourceHeadersError {
    /// Failed to read the archive or write the extracted headers.
    #[error("Failed to read or write clang resource headers: {0}")]
    Io(#[from] io::Error),

    /// Failed to read the wheel (a ZIP archive) that holds the headers.
    #[error("Failed to read clang resource headers from wheel: {0}")]
    ZipArchive(#[from] ZipError),

    /// The archive has an entry that would be written outside the `lib/clang` folder.
    #[error("Refusing to extract an unsafe path from archive: {0}")]
    UnsafePath(PathBuf),

    /// The archive has no clang resource headers.
    #[error("No clang resource headers found in {0}")]
    NotFound(PathBuf),
}

/// The `lib/clang` folder in which the executable at `bin` looks for its resource headers.
pub fn lib_clang_dir(bin: &Path) -> io::Result<PathBuf> {
    let bin = std::path::absolute(bin)?;
    let bin_dir = bin.parent().unwrap_or(&bin);
    Ok(normalize_path(
        &bin_dir.join("..").join("lib").join("clang"),
    ))
}

/// Are the resource headers for LLVM's `major` version installed for the executable at `bin`?
///
/// LLVM 15 and older name the headers' folder with the full version (`lib/clang/15.0.7`).
/// Newer versions use only the major version (`lib/clang/16`).
pub fn has_resource_headers(bin: &Path, major: u64) -> bool {
    let Ok(entries) = lib_clang_dir(bin).and_then(fs::read_dir) else {
        return false;
    };
    let major = major.to_string();
    let prefix = format!("{major}.");
    entries.flatten().any(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        (name == major || name.starts_with(&prefix))
            && entry.path().join("include").join("stddef.h").is_file()
    })
}

/// Joins the `relative` path of an archive entry to the `dir` folder.
///
/// Fails if the `relative` path could point outside of the `dir` folder.
fn dest_path(dir: &Path, relative: &Path) -> Result<PathBuf, ResourceHeadersError> {
    if relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        Ok(dir.join(relative))
    } else {
        Err(ResourceHeadersError::UnsafePath(relative.to_path_buf()))
    }
}

/// Installs resource headers into the `lib_clang` folder, all or nothing.
///
/// `extract` writes the headers' version folders (like `21/include/stddef.h`) into
/// the given staging folder, and returns `false` if the `archive` has no headers.
/// Each extracted version folder then replaces the one in `lib_clang`.
/// So a failed extraction never leaves a partial folder that [`has_resource_headers()`]
/// would accept.
fn install_staged(
    lib_clang: &Path,
    archive: &Path,
    extract: impl FnOnce(&Path) -> Result<bool, ResourceHeadersError>,
) -> Result<(), ResourceHeadersError> {
    let staging = lib_clang.with_file_name("clang.partial");
    if staging.exists() {
        // left over from an extraction that was interrupted
        fs::remove_dir_all(&staging)?;
    }
    let found = extract(&staging).inspect_err(|_| {
        let _ = fs::remove_dir_all(&staging);
    })?;
    if !found {
        return Err(ResourceHeadersError::NotFound(archive.to_path_buf()));
    }
    fs::create_dir_all(lib_clang)?;
    for entry in fs::read_dir(&staging)? {
        let entry = entry?;
        let dest = lib_clang.join(entry.file_name());
        if dest.is_dir() {
            fs::remove_dir_all(&dest)?;
        }
        fs::rename(entry.path(), dest)?;
    }
    fs::remove_dir(&staging)?;
    Ok(())
}

/// Extracts the resource headers from a clang-tidy `wheel` (`<package>/data/lib/clang/**`)
/// into the [`lib_clang_dir()`] of the executable at `bin`.
///
/// Wheels of clang-tidy v15 and older on PyPI do not have the headers.
/// In that case, a [`ResourceHeadersError::NotFound`] error is returned.
pub fn extract_from_wheel(
    wheel: &Path,
    package: &str,
    bin: &Path,
) -> Result<(), ResourceHeadersError> {
    let prefix = Path::new(package).join("data").join("lib").join("clang");
    install_staged(&lib_clang_dir(bin)?, wheel, |staging| {
        let mut archive = ZipArchive::new(fs::File::open(wheel)?)?;
        let mut found = false;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if !file.is_file() {
                continue;
            }
            let Some(relative) = file
                .enclosed_name()
                .and_then(|name| name.strip_prefix(&prefix).ok().map(Path::to_path_buf))
            else {
                continue;
            };
            let dest = dest_path(staging, &relative)?;
            // `dest` is always inside `staging`, so it always has a parent.
            fs::create_dir_all(dest.parent().unwrap_or(staging))?;
            io::copy(&mut file, &mut fs::File::create(&dest)?)?;
            found = true;
        }
        Ok(found)
    })
}

/// Extracts a `clang-resource-headers-<major>.tar.gz` `archive` (holding `lib/clang/**`)
/// into the [`lib_clang_dir()`] of the executable at `bin`.
///
/// These archives are published with the static binaries since LLVM v12.
pub fn extract_from_tar_gz(archive: &Path, bin: &Path) -> Result<(), ResourceHeadersError> {
    install_staged(&lib_clang_dir(bin)?, archive, |staging| {
        let mut tar = tar::Archive::new(GzDecoder::new(fs::File::open(archive)?));
        let mut found = false;
        for entry in tar.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry
                .path()?
                .components()
                .filter(|c| !matches!(c, Component::CurDir))
                .collect::<PathBuf>();
            let Ok(relative) = path.strip_prefix(Path::new("lib").join("clang")) else {
                continue;
            };
            let dest = dest_path(staging, relative)?;
            // `dest` is always inside `staging`, so it always has a parent.
            fs::create_dir_all(dest.parent().unwrap_or(staging))?;
            entry.unpack(&dest)?;
            found = true;
        }
        Ok(found)
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write, path::Path};

    use flate2::{Compression, write::GzEncoder};
    use tempfile::TempDir;

    use super::{
        ResourceHeadersError, extract_from_tar_gz, extract_from_wheel, has_resource_headers,
        lib_clang_dir,
    };

    const STDDEF: &[u8] = b"/* stddef.h */";

    /// Creates a gzipped tar file at `path` with the given `entries` (name, content).
    fn make_tar_gz(path: &Path, entries: &[(&str, &[u8])]) {
        let mut tar = tar::Builder::new(GzEncoder::new(
            fs::File::create(path).unwrap(),
            Compression::fast(),
        ));
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            // `append_data()` would reject unsafe paths, so the name is set directly.
            header.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name.as_bytes());
            header.set_cksum(); // spell-checker: disable-line
            tar.append(&header, *content).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

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
    fn lib_clang_next_to_bin_dir() {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        assert_eq!(
            lib_clang_dir(&bin).unwrap(),
            tmp.path().join("lib").join("clang")
        );
    }

    #[test]
    fn headers_by_version() {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        assert!(!has_resource_headers(&bin, 21));
        let lib_clang = tmp.path().join("lib").join("clang");

        // a folder without `stddef.h` does not count
        fs::create_dir_all(lib_clang.join("21").join("include")).unwrap();
        assert!(!has_resource_headers(&bin, 21));

        // LLVM 16+ use only the major version
        fs::write(lib_clang.join("21/include/stddef.h"), STDDEF).unwrap();
        assert!(has_resource_headers(&bin, 21));
        assert!(!has_resource_headers(&bin, 2));

        // LLVM 15 and older use the full version
        fs::create_dir_all(lib_clang.join("15.0.7").join("include")).unwrap();
        fs::write(lib_clang.join("15.0.7/include/stddef.h"), STDDEF).unwrap();
        assert!(has_resource_headers(&bin, 15));
        assert!(!has_resource_headers(&bin, 1));
    }

    #[test]
    fn from_wheel() {
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("clang_tidy.whl");
        make_wheel(
            &wheel,
            &[
                ("clang_tidy/data/bin/clang-tidy", b"binary"),
                ("clang_tidy/data/lib/clang/21/include/stddef.h", STDDEF),
                ("clang_tidy/__init__.py", b"ignored"),
            ],
        );
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        extract_from_wheel(&wheel, "clang_tidy", &bin).unwrap();
        assert!(has_resource_headers(&bin, 21));
        assert_eq!(
            fs::read(tmp.path().join("lib/clang/21/include/stddef.h")).unwrap(),
            STDDEF
        );
        assert!(!tmp.path().join("lib/clang/__init__.py").exists());
    }

    #[test]
    fn from_wheel_without_headers() {
        let tmp = TempDir::new().unwrap();
        let wheel = tmp.path().join("clang_tidy.whl");
        make_wheel(&wheel, &[("clang_tidy/data/bin/clang-tidy", b"binary")]);
        let bin = tmp.path().join("bin").join("clang-tidy-14");
        let err = extract_from_wheel(&wheel, "clang_tidy", &bin).unwrap_err();
        assert!(matches!(err, ResourceHeadersError::NotFound(_)));
        assert!(!has_resource_headers(&bin, 14));
    }

    #[test]
    fn from_tar_gz() {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("clang-resource-headers-15.tar.gz");
        make_tar_gz(
            &archive,
            &[
                ("./lib/clang/15.0.7/include/stddef.h", STDDEF),
                ("LICENSE.TXT", b"ignored"),
            ],
        );
        let bin = tmp.path().join("bin").join("clang-tidy-15");
        extract_from_tar_gz(&archive, &bin).unwrap();
        assert!(has_resource_headers(&bin, 15));
        assert!(!tmp.path().join("lib/clang/LICENSE.TXT").exists());
    }

    #[test]
    fn from_tar_gz_without_headers() {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("empty.tar.gz");
        make_tar_gz(&archive, &[("LICENSE.TXT", b"ignored")]);
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        let err = extract_from_tar_gz(&archive, &bin).unwrap_err();
        assert!(matches!(err, ResourceHeadersError::NotFound(_)));
    }

    #[test]
    fn from_tar_gz_unsafe_path() {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("evil.tar.gz");
        make_tar_gz(&archive, &[("lib/clang/../../../evil.h", b"evil")]);
        let bin = tmp.path().join("out").join("bin").join("clang-tidy-21");
        let err = extract_from_tar_gz(&archive, &bin).unwrap_err();
        assert!(matches!(err, ResourceHeadersError::UnsafePath(_)));
        assert!(!tmp.path().join("evil.h").exists());
    }

    #[test]
    fn failed_extraction_leaves_no_partial_headers() {
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("bad.tar.gz");
        make_tar_gz(
            &archive,
            &[
                ("lib/clang/21/include/stddef.h", STDDEF),
                ("lib/clang/../../evil.h", b"evil"),
            ],
        );
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        let err = extract_from_tar_gz(&archive, &bin).unwrap_err();
        assert!(matches!(err, ResourceHeadersError::UnsafePath(_)));
        assert!(!has_resource_headers(&bin, 21));
        assert!(!tmp.path().join("lib/clang.partial").exists());
    }

    #[test]
    fn extraction_replaces_old_headers() {
        let tmp = TempDir::new().unwrap();
        let lib = tmp.path().join("lib");
        // a partial install from an older version of this crate
        fs::create_dir_all(lib.join("clang/21/include")).unwrap();
        fs::write(lib.join("clang/21/include/stale.h"), b"stale").unwrap();
        // a staging folder left over from an interrupted extraction
        fs::create_dir_all(lib.join("clang.partial/21")).unwrap();

        let archive = tmp.path().join("clang-resource-headers-21.tar.gz");
        make_tar_gz(&archive, &[("lib/clang/21/include/stddef.h", STDDEF)]);
        let bin = tmp.path().join("bin").join("clang-tidy-21");
        extract_from_tar_gz(&archive, &bin).unwrap();
        assert!(has_resource_headers(&bin, 21));
        assert!(!lib.join("clang/21/include/stale.h").exists());
        assert!(!lib.join("clang.partial").exists());
    }
}
