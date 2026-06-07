use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use directories::ProjectDirs;

/// A trait for caching downloaded data to avoid unnecessary network requests.
pub trait Cacher {
    /// Returns the path to the cache directory.
    ///
    /// The cache directory can be overridden by setting the `CPP_LINTER_CACHE` environment variable.
    /// If the environment variable is not set, it defaults to a platform-specific cache directory provided by the [`directories`] crate.
    /// If executed on a platform that is not supported by the [`directories`] crate, it falls back to a `.cpp-linter_cache` directory in the working directory.
    fn get_cache_dir() -> PathBuf {
        env::var("CPP_LINTER_CACHE").map(PathBuf::from).unwrap_or(
            ProjectDirs::from("", "cpp-linter", "cpp-linter")
                .map(|d| d.cache_dir().to_path_buf())
                .unwrap_or(PathBuf::from(".cpp-linter_cache")),
        )
    }

    /// Checks if the given `cache_file` is valid based on its "last modified" time and the specified `max_age`.
    ///
    /// If `max_age` is `None` and the `cache_file` exists, then it is considered valid regardless of its age.
    ///
    /// Returns false if the cache does not exist or if it is older than the specified `max_age`.
    fn is_cache_valid(cache_file: &Path, max_age: Option<Duration>) -> bool {
        let now = SystemTime::now();
        cache_file.exists() && {
            cache_file
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map(|modified_time| {
                    max_age.is_none_or(|age| {
                        now.duration_since(modified_time)
                            .is_ok_and(|duration| duration < age)
                    })
                })
                .unwrap_or(false)
        }
    }
}
