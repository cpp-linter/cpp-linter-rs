//! This module is the home of functionality that uses the REST API of various git-based
//! servers.
//!
//! Currently, only Github is supported.

use std::fmt::Debug;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// non-std crates
use anyhow::{anyhow, Error, Result};
use chrono::DateTime;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, IntoUrl, Method, Request, Response, Url};

// project specific modules
pub mod github;
use crate::clang_tools::ClangVersions;
use crate::cli::{FeedbackInput, LinesChangedOnly};
use crate::common_fs::{FileFilter, FileObj};

pub static COMMENT_MARKER: &str = "<!-- cpp linter action -->\n";
pub static USER_OUTREACH: &str = concat!(
    "\n\nHave any feedback or feature suggestions? [Share it here.]",
    "(https://github.com/cpp-linter/cpp-linter-action/issues)"
);
pub static USER_AGENT: &str = concat!("cpp-linter/", env!("CARGO_PKG_VERSION"));

/// A structure to contain the different forms of headers that
/// describe a REST API's rate limit status.
#[derive(Debug, Clone)]
pub struct RestApiRateLimitHeaders {
    /// The header key of the rate limit's reset time.
    pub reset: String,
    /// The header key of the rate limit's remaining attempts.
    pub remaining: String,
    /// The header key of the rate limit's "backoff" time interval.
    pub retry: String,
}

/// A custom trait that templates necessary functionality with a Git server's REST API.
pub trait RestApiClient {
    /// A way to set output variables specific to cpp_linter executions in CI.
    fn set_exit_code(
        &self,
        checks_failed: u64,
        format_checks_failed: Option<u64>,
        tidy_checks_failed: Option<u64>,
    ) -> u64;

    /// This prints a line to indicate the beginning of a related group of log statements.
    fn start_log_group(&self, name: String);

    /// This prints a line to indicate the ending of a related group of log statements.
    fn end_log_group(&self);

    /// A convenience method to create the headers attached to all REST API calls.
    ///
    /// If an authentication token is provided (via environment variable),
    /// this method shall include the relative information.
    fn make_headers() -> Result<HeaderMap<HeaderValue>>;

    /// Construct a HTTP request to be sent.
    ///
    /// The idea here is that this method is called before [`send_api_request()`].
    /// ```ignore
    /// let request = Self::make_api_request(
    ///     &self.client,
    ///     "https://example.com",
    ///     Method::GET,
    ///     None,
    ///     None
    /// );
    /// let response = send_api_request(&self.client, request, &self.rest_api_headers);
    /// match response.await {
    ///     Ok(res) => {/* handle response */}
    ///     Err(e) => {/* handle failure */}
    /// }
    /// ```
    fn make_api_request(
        client: &Client,
        url: impl IntoUrl,
        method: Method,
        data: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Request> {
        let mut req = client.request(method, url);
        if let Some(h) = headers {
            req = req.headers(h);
        }
        if let Some(d) = data {
            req = req.body(d);
        }
        // RequestBuilder only fails to `build()` if there is a malformed `url`. We
        // should be safe here because of this function's `url` parameter type.
        req.build().map_err(Error::from)
    }

    /// A way to get the list of changed files using REST API calls. It is this method's
    /// job to parse diff blobs and return a list of changed files.
    ///
    /// The context of the file changes are subject to the type of event in which
    /// cpp_linter package is used.
    fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
        lines_changed_only: &LinesChangedOnly,
    ) -> impl Future<Output = Result<Vec<FileObj>>>;

    /// Makes a comment in MarkDown syntax based on the concerns in `format_advice` and
    /// `tidy_advice` about the given set of `files`.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    ///
    /// Returns the markdown comment as a string as well as the total count of
    /// `format_checks_failed` and `tidy_checks_failed` (in respective order).
    fn make_comment(
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
        clang_versions: &ClangVersions,
        max_len: Option<u64>,
    ) -> String {
        let mut comment = format!("{COMMENT_MARKER}# Cpp-Linter Report ");
        let mut remaining_length =
            max_len.unwrap_or(u64::MAX) - comment.len() as u64 - USER_OUTREACH.len() as u64;

        if format_checks_failed > 0 || tidy_checks_failed > 0 {
            let prompt = ":warning:\nSome files did not pass the configured checks!\n";
            remaining_length -= prompt.len() as u64;
            comment.push_str(prompt);
            if format_checks_failed > 0 {
                make_format_comment(
                    files,
                    &mut comment,
                    format_checks_failed,
                    // tidy_version should be `Some()` value at this point.
                    clang_versions.tidy_version.as_ref().unwrap(),
                    &mut remaining_length,
                );
            }
            if tidy_checks_failed > 0 {
                make_tidy_comment(
                    files,
                    &mut comment,
                    tidy_checks_failed,
                    // format_version should be `Some()` value at this point.
                    clang_versions.format_version.as_ref().unwrap(),
                    &mut remaining_length,
                );
            }
        } else {
            comment.push_str(":heavy_check_mark:\nNo problems need attention.");
        }
        comment.push_str(USER_OUTREACH);
        comment
    }

    /// A way to post feedback in the form of `thread_comments`, `file_annotations`, and
    /// `step_summary`.
    ///
    /// The given `files` should've been gathered from `get_list_of_changed_files()` or
    /// `list_source_files()`.
    ///
    /// The `format_advice` and `tidy_advice` should be a result of parsing output from
    /// clang-format and clang-tidy (see `capture_clang_tools_output()`).
    ///
    /// All other parameters correspond to CLI arguments.
    fn post_feedback(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        user_inputs: FeedbackInput,
        clang_versions: ClangVersions,
    ) -> impl Future<Output = Result<u64>>;

    /// Gets the URL for the next page in a paginated response.
    ///
    /// Returns [`None`] if current response is the last page.
    fn try_next_page(headers: &HeaderMap) -> Option<Url> {
        if let Some(links) = headers.get("link") {
            if let Ok(pg_str) = links.to_str() {
                let pages = pg_str.split(", ");
                for page in pages {
                    if page.ends_with("; rel=\"next\"") {
                        if let Some(link) = page.split_once(">;") {
                            let url = link.0.trim_start_matches("<").to_string();
                            if let Ok(next) = Url::parse(&url) {
                                return Some(next);
                            } else {
                                log::debug!("Failed to parse next page link from response header");
                            }
                        } else {
                            log::debug!("Response header link for pagination is malformed");
                        }
                    }
                }
            }
        }
        None
    }

    fn log_response(response: Response, context: &str) -> impl Future<Output = ()> + Send {
        async move {
            if let Err(e) = response.error_for_status_ref() {
                log::error!("{}: {e:?}", context.to_owned());
                if let Ok(text) = response.text().await {
                    log::error!("{text}");
                }
            }
        }
    }
}

const MAX_RETRIES: u8 = 5;

/// A convenience function to send HTTP requests and respect a REST API rate limits.
///
/// This method respects both primary and secondary rate limits.
/// In the event where  the secondary rate limits is reached,
/// this function will wait for a time interval specified the server and retry afterward.
pub async fn send_api_request(
    client: &Client,
    request: Request,
    rate_limit_headers: &RestApiRateLimitHeaders,
) -> Result<Response> {
    for i in 0..MAX_RETRIES {
        let result = client
            .execute(request.try_clone().ok_or(anyhow!(
                "Failed to clone request object for recursive behavior"
            ))?)
            .await;
        if let Ok(response) = &result {
            if [403u16, 429u16].contains(&response.status().as_u16()) {
                // rate limit may have been exceeded

                // check if primary rate limit was violated; panic if so.
                let mut requests_remaining = None;
                if let Some(remaining) = response.headers().get(&rate_limit_headers.remaining) {
                    if let Ok(count) = remaining.to_str() {
                        if let Ok(value) = count.parse::<i64>() {
                            requests_remaining = Some(value);
                        } else {
                            log::debug!(
                                    "Failed to parse i64 from remaining attempts about rate limit: {count}"
                                );
                        }
                    }
                } else {
                    // NOTE: I guess it is sometimes valid for a request to
                    // not include remaining rate limit attempts
                    log::debug!("Response headers do not include remaining API usage count");
                }
                if requests_remaining.is_some_and(|v| v <= 0) {
                    if let Some(reset_value) = response.headers().get(&rate_limit_headers.reset) {
                        if let Ok(epoch) = reset_value.to_str() {
                            if let Ok(value) = epoch.parse::<i64>() {
                                if let Some(reset) = DateTime::from_timestamp(value, 0) {
                                    return Err(anyhow!(
                                        "REST API rate limit exceeded! Resets at {}",
                                        reset
                                    ));
                                }
                            } else {
                                log::debug!(
                                    "Failed to parse i64 from reset time about rate limit: {epoch}"
                                );
                            }
                        }
                    } else {
                        log::debug!("Response headers does not include a reset timestamp");
                    }
                    return Err(anyhow!("REST API rate limit exceeded!"));
                }

                // check if secondary rate limit is violated; backoff and try again.
                if let Some(retry_value) = response.headers().get(&rate_limit_headers.retry) {
                    if let Ok(retry_str) = retry_value.to_str() {
                        if let Ok(retry) = retry_str.parse::<u64>() {
                            let interval = Duration::from_secs(retry + (i as u64).pow(2));
                            tokio::time::sleep(interval).await;
                        } else {
                            log::debug!(
                                        "Failed to parse u64 from retry interval about rate limit: {retry_str}"
                                    );
                        }
                    }
                    continue;
                }
            }
            return result.map_err(Error::from);
        }
        return result.map_err(Error::from);
    }
    Err(anyhow!(
        "REST API secondary rate limit exceeded after {MAX_RETRIES} retries."
    ))
}

fn make_format_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    format_checks_failed: u64,
    version_used: &String,
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-format (v{version_used}) reports: <strong>{format_checks_failed} file(s) not formatted</strong></summary>\n\n",
    );
    let closer = String::from("\n</details>");
    let mut format_comment = String::new();
    *remaining_length -= opener.len() as u64 + closer.len() as u64;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(format_advice) = &file.format_advice {
            if !format_advice.replacements.is_empty() && *remaining_length > 0 {
                let note = format!("- {}\n", file.name.to_string_lossy().replace('\\', "/"));
                if (note.len() as u64) < *remaining_length {
                    format_comment.push_str(&note.to_string());
                    *remaining_length -= note.len() as u64;
                }
            }
        }
    }
    comment.push_str(&opener);
    comment.push_str(&format_comment);
    comment.push_str(&closer);
}

fn make_tidy_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    tidy_checks_failed: u64,
    version_used: &String,
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-tidy (v{version_used}) reports: {tidy_checks_failed}<strong> concern(s)</strong></summary>\n\n"
    );
    let closer = String::from("\n</details>");
    let mut tidy_comment = String::new();
    *remaining_length -= opener.len() as u64 + closer.len() as u64;
    for file in files {
        let file = file.lock().unwrap();
        if let Some(tidy_advice) = &file.tidy_advice {
            for tidy_note in &tidy_advice.notes {
                let file_path = PathBuf::from(&tidy_note.filename);
                if file_path == file.name {
                    let mut tmp_note = format!("- {}\n\n", tidy_note.filename);
                    tmp_note.push_str(&format!(
                        "   <strong>{filename}:{line}:{cols}:</strong> {severity}: [{diagnostic}]\n   > {rationale}\n{concerned_code}",
                        filename = tidy_note.filename,
                        line = tidy_note.line,
                        cols = tidy_note.cols,
                        severity = tidy_note.severity,
                        diagnostic = tidy_note.diagnostic_link(),
                        rationale = tidy_note.rationale,
                        concerned_code = if tidy_note.suggestion.is_empty() {String::from("")} else {
                            format!("\n   ```{ext}\n   {suggestion}\n   ```\n",
                                ext = file_path.extension().unwrap_or_default().to_string_lossy(),
                                suggestion = tidy_note.suggestion.join("\n   "),
                            ).to_string()
                        },
                    ).to_string());

                    if (tmp_note.len() as u64) < *remaining_length {
                        tidy_comment.push_str(&tmp_note);
                        *remaining_length -= tmp_note.len() as u64;
                    }
                }
            }
        }
    }
    comment.push_str(&opener);
    comment.push_str(&tidy_comment);
    comment.push_str(&closer);
}

/// This module tests the silent errors' debug logs
/// from `try_next_page()` and `send_api_request()` functions.
#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use anyhow::{anyhow, Result};
    use chrono::Utc;
    use mockito::{Matcher, Server};
    use reqwest::Method;
    use reqwest::{
        header::{HeaderMap, HeaderValue},
        Client,
    };

    use crate::cli::LinesChangedOnly;
    use crate::{
        clang_tools::ClangVersions,
        cli::FeedbackInput,
        common_fs::{FileFilter, FileObj},
        logger,
    };

    use super::{send_api_request, RestApiClient, RestApiRateLimitHeaders};

    /// A dummy struct to impl RestApiClient
    #[derive(Default)]
    struct TestClient {}

    impl RestApiClient for TestClient {
        fn set_exit_code(
            &self,
            _checks_failed: u64,
            _format_checks_failed: Option<u64>,
            _tidy_checks_failed: Option<u64>,
        ) -> u64 {
            0
        }

        fn make_headers() -> Result<HeaderMap<HeaderValue>> {
            Err(anyhow!("Not implemented"))
        }

        async fn get_list_of_changed_files(
            &self,
            _file_filter: &FileFilter,
            _lines_changed_only: &LinesChangedOnly,
        ) -> Result<Vec<FileObj>> {
            Err(anyhow!("Not implemented"))
        }

        async fn post_feedback(
            &self,
            _files: &[Arc<Mutex<FileObj>>],
            _user_inputs: FeedbackInput,
            _clang_versions: ClangVersions,
        ) -> Result<u64> {
            Err(anyhow!("Not implemented"))
        }

        fn start_log_group(&self, name: String) {
            log::info!(target: "CI_LOG_GROUPING", "start_log_group: {name}");
        }

        fn end_log_group(&self) {
            log::info!(target: "CI_LOG_GROUPING", "end_log_group");
        }
    }

    #[tokio::test]
    async fn dummy_coverage() {
        assert!(TestClient::make_headers().is_err());
        let dummy = TestClient::default();
        dummy.start_log_group("Dummy test".to_string());
        assert_eq!(dummy.set_exit_code(1, None, None), 0);
        assert!(dummy
            .get_list_of_changed_files(&FileFilter::new(&[], vec![]), &LinesChangedOnly::Off)
            .await
            .is_err());
        assert!(dummy
            .post_feedback(&[], FeedbackInput::default(), ClangVersions::default())
            .await
            .is_err());
        dummy.end_log_group();
    }

    // ************************************************* try_next_page() tests

    #[test]
    fn bad_link_header() {
        let mut headers = HeaderMap::with_capacity(1);
        assert!(headers
            .insert("link", HeaderValue::from_str("; rel=\"next\"").unwrap())
            .is_none());
        logger::try_init();
        log::set_max_level(log::LevelFilter::Debug);
        let result = TestClient::try_next_page(&headers);
        assert!(result.is_none());
    }

    #[test]
    fn bad_link_domain() {
        let mut headers = HeaderMap::with_capacity(1);
        assert!(headers
            .insert(
                "link",
                HeaderValue::from_str("<not a domain>; rel=\"next\"").unwrap()
            )
            .is_none());
        logger::try_init();
        log::set_max_level(log::LevelFilter::Debug);
        let result = TestClient::try_next_page(&headers);
        assert!(result.is_none());
    }

    // ************************************************* Rate Limit Tests

    #[derive(Default)]
    struct RateLimitTestParams {
        secondary: bool,
        has_remaining_count: bool,
        bad_remaining_count: bool,
        has_reset_timestamp: bool,
        bad_reset_timestamp: bool,
        has_retry_interval: bool,
        bad_retry_interval: bool,
    }

    async fn simulate_rate_limit(test_params: &RateLimitTestParams) {
        let rate_limit_headers = RestApiRateLimitHeaders {
            reset: "reset".to_string(),
            remaining: "remaining".to_string(),
            retry: "retry".to_string(),
        };
        logger::try_init();
        log::set_max_level(log::LevelFilter::Debug);

        let mut server = Server::new_async().await;
        let client = Client::new();
        let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
        let mut mock = server
            .mock("GET", "/")
            .match_body(Matcher::Any)
            .expect_at_least(1)
            .expect_at_most(5)
            .with_status(429);
        if test_params.has_remaining_count {
            mock = mock.with_header(
                &rate_limit_headers.remaining,
                if test_params.secondary {
                    "1"
                } else if test_params.bad_remaining_count {
                    "X"
                } else {
                    "0"
                },
            );
        }
        if test_params.has_reset_timestamp {
            mock = mock.with_header(
                &rate_limit_headers.reset,
                if test_params.bad_reset_timestamp {
                    "X"
                } else {
                    &reset_timestamp
                },
            );
        }
        if test_params.secondary && test_params.has_retry_interval {
            mock.with_header(
                &rate_limit_headers.retry,
                if test_params.bad_retry_interval {
                    "X"
                } else {
                    "0"
                },
            )
            .create();
        } else {
            mock.create();
        }
        let request =
            TestClient::make_api_request(&client, server.url(), Method::GET, None, None).unwrap();
        send_api_request(&client, request, &rate_limit_headers)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic(expected = "REST API secondary rate limit exceeded")]
    async fn rate_limit_secondary() {
        simulate_rate_limit(&RateLimitTestParams {
            secondary: true,
            has_retry_interval: true,
            ..Default::default()
        })
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "REST API secondary rate limit exceeded")]
    async fn rate_limit_bad_retry() {
        simulate_rate_limit(&RateLimitTestParams {
            secondary: true,
            has_retry_interval: true,
            bad_retry_interval: true,
            ..Default::default()
        })
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "REST API rate limit exceeded!")]
    async fn rate_limit_primary() {
        simulate_rate_limit(&RateLimitTestParams {
            has_remaining_count: true,
            has_reset_timestamp: true,
            ..Default::default()
        })
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "REST API rate limit exceeded!")]
    async fn rate_limit_no_reset() {
        simulate_rate_limit(&RateLimitTestParams {
            has_remaining_count: true,
            ..Default::default()
        })
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "REST API rate limit exceeded!")]
    async fn rate_limit_bad_reset() {
        simulate_rate_limit(&RateLimitTestParams {
            has_remaining_count: true,
            has_reset_timestamp: true,
            bad_reset_timestamp: true,
            ..Default::default()
        })
        .await;
    }

    #[tokio::test]
    async fn rate_limit_bad_count() {
        simulate_rate_limit(&RateLimitTestParams {
            has_remaining_count: true,
            bad_remaining_count: true,
            ..Default::default()
        })
        .await;
    }
}
