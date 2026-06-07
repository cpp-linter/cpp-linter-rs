//! A utility module for path normalization.
use std::{
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
};

/// This was copied from [cargo source code](https://github.com/rust-lang/cargo/blob/8cc0cb136772b8f54eafe0d163fcb7226a06af0c/crates/cargo-util/src/paths.rs#L84).
///
/// NOTE: Rust [std::path] crate has no native functionality equivalent to this.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

/// Creates (and returns) a lock [`File`] for the given `path` and locks it.
///
/// This function will create a lock file with `.lock` appended to the
/// given `path`'s extension. It will then acquire an exclusive lock on
/// the file to prevent concurrent access.
///
/// Note, This will block until a lock is obtained.
///
/// The caller is responsible for cleanup, which includes
///
/// 1. unlocking the returned file and
/// 2. deleting the file when done (if desired).
pub fn lock_path(path: &Path) -> Result<File, std::io::Error> {
    let lock_path = path.with_added_extension("lock");
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)?;
    file_lock.lock()?;
    Ok(file_lock)
}

#[cfg(test)]
mod tests {
    use std::{env::current_dir, path::PathBuf};

    use super::normalize_path;

    #[test]
    fn normalize_redirects() {
        let mut src = current_dir().unwrap();
        src.push("..");
        src.push(
            current_dir()
                .unwrap()
                .strip_prefix(current_dir().unwrap().parent().unwrap())
                .unwrap(),
        );
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), current_dir().unwrap());
    }

    #[test]
    fn normalize_no_root() {
        let src = PathBuf::from(concat!("../", env!("CARGO_PKG_NAME")));
        let mut cur_dir = current_dir().unwrap();
        cur_dir = cur_dir
            .strip_prefix(current_dir().unwrap().parent().unwrap())
            .unwrap()
            .to_path_buf();
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), cur_dir);
    }

    #[test]
    fn normalize_current_redirect() {
        let src = PathBuf::from("tests/./ignored_paths");
        println!("relative path = {}", src.to_str().unwrap());
        assert_eq!(normalize_path(&src), PathBuf::from("tests/ignored_paths"));
    }
}
