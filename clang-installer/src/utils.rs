use std::path::{Component, Path, PathBuf};

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
