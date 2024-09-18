//! This crate is the home of functionality that uses the REST API of various git-based
//! servers.
//!
//! Currently, only Github is supported.

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// non-std crates
use chrono::DateTime;
use futures::future::{BoxFuture, FutureExt};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, IntoUrl, Method, Request, Response, StatusCode, Url};

// project specific modules/crates
pub mod github_api;
use crate::cli::FeedbackInput;
use crate::common_fs::{FileFilter, FileObj};

pub static COMMENT_MARKER: &str = "<!-- cpp linter action -->\n";
pub static USER_OUTREACH: &str = "\n\nHave any feedback or feature suggestions? [Share it here.](https://github.com/cpp-linter/cpp-linter-action/issues)";

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

pub struct CachedResponse {
    pub text: String,
    pub headers: HeaderMap,
    pub status: StatusCode,
}

impl CachedResponse {
    async fn from(response: Response) -> Self {
        let headers = response.headers().to_owned();
        let status = response.status();
        let text = response
            .text()
            .await
            .expect("Failed to decode response body to text.");
        Self {
            text,
            headers,
            status,
        }
    }
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

    /// A convenience method to create the headers attached to all REST API calls.
    ///
    /// If an authentication token is provided (via environment variable),
    /// this method shall include the relative information.
    fn make_headers() -> HeaderMap<HeaderValue>;

    /// Construct a HTTP request to be sent.
    ///
    /// The idea here is that this method is called before [`RestApiClient::send_api_request()`].
    /// ```ignore
    /// let request = Self::make_api_request(
    ///     &self.client,
    ///     "https://example.com",
    ///     Method::GET,
    ///     None,
    ///     None
    /// );
    /// let response = Self::send_api_request(
    ///     self.client.clone(),
    ///     request,
    ///     false, // false means don't panic
    ///     0, // start recursion count at 0
    /// );
    /// match response.await {
    ///     Some(res) => {/* handle response */}
    ///     None => {/* handle failure */}
    /// }
    /// ```
    fn make_api_request(
        client: &Client,
        url: impl IntoUrl,
        method: Method,
        data: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Request {
        let mut req = client.request(method, url);
        if let Some(h) = headers {
            req = req.headers(h);
        }
        if let Some(d) = data {
            req = req.body(d);
        }
        req.build().expect("Failed to create a HTTP request")
    }

    /// A convenience function to send HTTP requests and respect a REST API rate limits.
    ///
    /// This method must own all the data passed to it because asynchronous recursion is used.
    /// Recursion is needed when a secondary rate limit is hit. The server tells the client that
    /// it should back off and retry after a specified time interval.
    ///
    /// Setting the `strict` parameter to true will panic when the HTTP request fails to send or
    /// the HTTP response's status code does not describe success. This should only be used for
    /// requests that are vital to the app operation.
    /// With `strict` as true, the returned type is guaranteed to be a [`Some`] value.
    /// If `strict` is false, then a failure to send the request is returned as a [`None`] value,
    /// and a [`Some`] value could also indicate if the server failed to process the request.
    fn send_api_request(
        client: Client,
        request: Request,
        strict: bool,
        rate_limit_headers: RestApiRateLimitHeaders,
        retries: u64,
    ) -> BoxFuture<'static, Option<CachedResponse>> {
        async move {
            match client
                .execute(
                    request
                        .try_clone()
                        .expect("Could not clone HTTP request object"),
                )
                .await
            {
                Ok(response) => {
                    if [403u16, 429u16].contains(&response.status().as_u16()) {
                        // rate limit exceeded

                        // check if primary rate limit was violated; panic if so.
                        let remaining = response
                            .headers()
                            .get(&rate_limit_headers.remaining)
                            .expect("Response headers do not include remaining API usage count")
                            .to_str()
                            .expect("Failed to extract remaining attempts about rate limit")
                            .parse::<i64>()
                            .expect("Failed to parse i64 from remaining attempts about rate limit");
                        if remaining <= 0 {
                            let reset = DateTime::from_timestamp(
                                response
                                    .headers()
                                    .get(&rate_limit_headers.reset)
                                    .expect("response headers does not include a reset timestamp")
                                    .to_str()
                                    .expect("Failed to extract reset info about rate limit")
                                    .parse::<i64>()
                                    .expect("Failed to parse i64 from reset time about rate limit"),
                                0,
                            )
                            .expect("rate limit reset UTC timestamp is an invalid");
                            panic!("REST API rate limit exceeded! Resets at {}", reset);
                        }

                        // check if secondary rate limit is violated; backoff and try again.
                        if retries > 4 {
                            panic!("REST API secondary rate limit exceeded");
                        }
                        if let Some(retry) = response.headers().get(&rate_limit_headers.retry) {
                            let interval = Duration::from_secs(
                                retry
                                    .to_str()
                                    .expect("Failed to extract retry interval about rate limit")
                                    .parse::<u64>()
                                    .expect(
                                        "Failed to parse u64 from retry interval about rate limit",
                                    )
                                    + retries.pow(2),
                            );
                            tokio::time::sleep(interval).await;
                            return Self::send_api_request(
                                client,
                                request,
                                strict,
                                rate_limit_headers,
                                retries + 1,
                            )
                            .await;
                        }
                    }
                    let cached_response = CachedResponse::from(response).await;
                    if !cached_response.status.is_success() {
                        let summary = format!(
                            "Got {} response from {} request to {}:\n{}",
                            cached_response.status.as_u16(),
                            request.method().as_str(),
                            request.url().as_str(),
                            cached_response.text,
                        );
                        if strict {
                            panic!("{summary}");
                        } else {
                            log::error!("{summary}");
                        }
                    }
                    Some(cached_response)
                }
                Err(e) => {
                    if strict {
                        panic!("Failed to complete the HTTP request.\n{}", e);
                    } else {
                        None
                    }
                }
            }
        }
        .boxed()
    }

    /// A way to get the list of changed files using REST API calls. It is this method's
    /// job to parse diff blobs and return a list of changed files.
    ///
    /// The context of the file changes are subject to the type of event in which
    /// cpp_linter package is used.
    fn get_list_of_changed_files(
        &self,
        file_filter: &FileFilter,
    ) -> impl Future<Output = Vec<FileObj>>;

    /// Makes a comment in MarkDown syntax based on the concerns in `format_advice` and
    /// `tidy_advice` about the given set of `files`.
    ///
    /// This method has a default definition and should not need to be redefined by
    /// implementors.
    ///
    /// Returns the markdown comment as a string as well as the total count of
    /// `format_checks_failed` and `tidy_checks_failed` (in respective order).
    fn make_comment(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        format_checks_failed: u64,
        tidy_checks_failed: u64,
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
                    &mut remaining_length,
                );
            }
            if tidy_checks_failed > 0 {
                make_tidy_comment(
                    files,
                    &mut comment,
                    tidy_checks_failed,
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
    ) -> impl Future<Output = u64>;

    /// Gets the URL for the next page in a paginated response.
    ///
    /// Returns [`None`] if current response is the last page.
    fn try_next_page(headers: &HeaderMap) -> Option<Url> {
        if headers.contains_key("link") {
            let pages = headers["link"]
                .to_str()
                .expect("Failed to convert header value of links to a str")
                .split(", ");
            for page in pages {
                if page.ends_with("; rel=\"next\"") {
                    let url = page
                        .split_once(">;")
                        .expect("header link for pagination is malformed")
                        .0
                        .trim_start_matches("<")
                        .to_string();
                    return Some(
                        Url::parse(&url)
                            .expect("Failed to parse next page link from response header"),
                    );
                }
            }
        }
        None
    }
}

fn make_format_comment(
    files: &[Arc<Mutex<FileObj>>],
    comment: &mut String,
    format_checks_failed: u64,
    remaining_length: &mut u64,
) {
    let opener = format!("\n<details><summary>clang-format reports: <strong>{} file(s) not formatted</strong></summary>\n\n", format_checks_failed);
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
    remaining_length: &mut u64,
) {
    let opener = format!(
        "\n<details><summary>clang-tidy reports: <strong>{} concern(s)</strong></summary>\n\n",
        tidy_checks_failed
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
                                ext = file_path.extension().expect("file extension was not determined").to_string_lossy(),
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
