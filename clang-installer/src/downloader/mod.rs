use reqwest::ClientBuilder;
use std::{
    fs,
    io::Write,
    num::NonZero,
    path::Path,
    time::{Duration, SystemTime},
};
use url::Url;

use crate::progress_bar::ProgressBar;

pub mod caching;
pub mod hashing;
pub mod native_packages;
pub mod pypi;
pub mod static_dist;

/// An enumeration of possible errors during download operations.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// An error that occurred while making a request or handling the response.
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    /// An error that occurred during interaction with the file system.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// An error that describes the mismatch between the expected and actual hash of the downloaded file.
    #[error("Hash mismatch for downloaded file. Expected: {expected}, Actual: {actual}")]
    HashMismatch { expected: String, actual: String },

    /// An error that occurred while moving/persisting a temporary file into a cache path.
    #[error("Error persisting temporary file: {0}")]
    TempFilePersistence(#[from] tempfile::PersistError),
}

/// Downloads data from the specified URL and returns the response.
///
/// If the response's status code indicates an error, then the error will be returned.
/// If the erroneous response's body is UTF-8 text, then it will be included in the logged error.
async fn download(url: &Url, cache_path: &Path, timeout: u64) -> Result<(), DownloadError> {
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .build()?;
    if let Some(cache_parent) = cache_path.parent() {
        fs::create_dir_all(cache_parent)?;
    }
    let mut response = client.get(url.clone()).send().await?;
    if let Err(e) = response.error_for_status_ref() {
        if let Ok(body) = response.text().await
            && !body.is_empty()
        {
            log::error!("Failed to download data from {url}:\n{body}");
        } else {
            log::error!("Failed to download data from {url}");
        }
        return Err(e.into());
    }
    let mut tmp_file = tempfile::NamedTempFile::new()?;
    let content_len = response.content_length().and_then(NonZero::new);
    let mut progress_bar = ProgressBar::new(
        content_len,
        format!(
            "Downloading {}",
            cache_path
                .file_name()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default()
        )
        .as_str(),
    );
    progress_bar.render()?;
    while let Some(chunk) = response.chunk().await? {
        let chunk_len = chunk.len() as u64;
        progress_bar.inc(chunk_len)?;
        tmp_file.write_all(&chunk)?;
    }
    progress_bar.finish()?;
    tmp_file.flush()?;
    tmp_file.as_file_mut().set_modified(SystemTime::now())?;
    tmp_file.persist(cache_path)?;
    Ok(())
}

/// On Unix-like systems, this changes the permissions of the file at `path`.
///
/// If `mode` is `Some`, then the permissions will be set to the bitwise OR of
/// the existing permissions and given `mode`.
/// If `mode` is `None`, then the permissions will be set to `0o755`.
#[cfg(unix)]
fn chmod_file(path: &Path, mode: Option<u32>) -> std::io::Result<()> {
    // Make the extracted binary executable on Unix-like systems.
    use std::os::unix::fs::PermissionsExt;
    let out = fs::OpenOptions::new().write(true).open(path)?;
    let mut perms = out.metadata()?.permissions();
    match mode {
        Some(mode) => {
            let prev = perms.mode();
            perms.set_mode(prev | mode);
        }
        None => perms.set_mode(0o755),
    }
    out.set_permissions(perms)
}

#[cfg(test)]
mod tests {
    use crate::DownloadError;

    use super::download;
    use mockito::Server;
    use tempfile::NamedTempFile;
    use url::Url;
    struct TestLogger;

    impl log::Log for TestLogger {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            println!("[{}] - {}", record.level(), record.args());
        }

        fn flush(&self) {}
    }

    fn initialize_logger() {
        let logger: TestLogger = TestLogger;
        if let Err(e) = log::set_boxed_logger(Box::new(logger))
            .map(|()| log::set_max_level(log::LevelFilter::Info))
        {
            // logger singleton already instantiated.
            // we'll just use whatever the current config is.
            log::debug!("{e:?}");
        }
    }

    #[tokio::test]
    async fn fail_download() {
        initialize_logger();
        assert!(log::log_enabled!(log::Level::Info));
        let mut server = Server::new_async().await;
        let url_path = "/test";
        let url = Url::parse(server.url().as_str())
            .unwrap()
            .join(url_path)
            .unwrap();
        let mock = server
            .mock("GET", url_path)
            .with_status(500)
            .with_body("Intentionally failed request")
            .create();
        let tmp_file = NamedTempFile::new().unwrap();
        let err = download(&url, tmp_file.path(), 1).await.unwrap_err();
        println!("{}", err);
        assert!(matches!(err, DownloadError::RequestError(_)));
        mock.assert();
        log::logger().flush();
    }

    #[tokio::test]
    async fn fail_download_no_body() {
        initialize_logger();
        let mut server = Server::new_async().await;
        let url_path = "/test";
        let url = Url::parse(server.url().as_str())
            .unwrap()
            .join(url_path)
            .unwrap();
        // to trigger the abridged log::error!() call, the body must be non-UTF-8.
        let mock = server
            .mock("GET", url_path)
            .with_status(500)
            // .with_body(b"\xC0")
            .create();
        let tmp_file = NamedTempFile::new().unwrap();
        let err = download(&url, tmp_file.path(), 1).await.unwrap_err();
        println!("{}", err);
        assert!(matches!(err, DownloadError::RequestError(_)));
        mock.assert();
    }
}
