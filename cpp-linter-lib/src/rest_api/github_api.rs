//! This module holds functionality specific to using Github's REST API.

use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

// non-std crates
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Method;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json;

// project specific modules/crates
use crate::clang_tools::clang_format::tally_format_advice;
use crate::clang_tools::clang_tidy::tally_tidy_advice;
use crate::cli::{FeedbackInput, ThreadComments};
use crate::common_fs::{FileFilter, FileObj};
use crate::git::{get_diff, open_repo, parse_diff, parse_diff_from_buf};

use super::{RestApiClient, RestApiRateLimitHeaders, COMMENT_MARKER};

/// A structure to work with Github REST API.
pub struct GithubApiClient {
    /// The HTTP request client to be used for all REST API calls.
    client: Client,

    /// The CI run's event payload from the webhook that triggered the workflow.
    pull_request: Option<i64>,

    /// The name of the event that was triggered when running cpp_linter.
    pub event_name: String,

    /// The value of the `GITHUB_API_URL` environment variable.
    api_url: Url,

    /// The value of the `GITHUB_REPOSITORY` environment variable.
    repo: Option<String>,

    /// The value of the `GITHUB_SHA` environment variable.
    sha: Option<String>,

    /// The value of the `ACTIONS_STEP_DEBUG` environment variable.
    pub debug_enabled: bool,

    rate_limit_headers: RestApiRateLimitHeaders,
}

impl Default for GithubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApiClient {
    pub fn new() -> Self {
        let event_name = env::var("GITHUB_EVENT_NAME").unwrap_or(String::from("unknown"));
        let pull_request = {
            match event_name.as_str() {
                "pull_request" => {
                    let event_payload_path = env::var("GITHUB_EVENT_PATH")
                        .expect("GITHUB_EVENT_NAME is set to 'pull_request', but GITHUB_EVENT_PATH is not set");
                    let file_buf = &mut String::new();
                    OpenOptions::new()
                        .read(true)
                        .open(event_payload_path)
                        .unwrap()
                        .read_to_string(file_buf)
                        .unwrap();
                    let json = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                        file_buf,
                    )
                    .unwrap();
                    json["number"].as_i64()
                }
                _ => None,
            }
        };
        let api_url = Url::parse(
            env::var("GITHUB_API_URL")
                .unwrap_or("https://api.github.com".to_string())
                .as_str(),
        )
        .expect("Failed to parse URL from GITHUB_API_URL");

        GithubApiClient {
            client: Client::builder()
                .default_headers(Self::make_headers())
                .build()
                .expect("Failed to create a session client for REST API calls"),
            pull_request,
            event_name,
            api_url,
            repo: match env::var("GITHUB_REPOSITORY") {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            sha: match env::var("GITHUB_SHA") {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            debug_enabled: match env::var("ACTIONS_STEP_DEBUG") {
                Ok(val) => val == "true",
                Err(_) => false,
            },
            rate_limit_headers: RestApiRateLimitHeaders {
                reset: "x-ratelimit-reset".to_string(),
                remaining: "x-ratelimit-remaining".to_string(),
                retry: "retry-after".to_string(),
            },
        }
    }
}

// implement the RestApiClient trait for the GithubApiClient
impl RestApiClient for GithubApiClient {
    fn set_exit_code(
        &self,
        checks_failed: u64,
        format_checks_failed: Option<u64>,
        tidy_checks_failed: Option<u64>,
    ) -> u64 {
        if let Ok(gh_out) = env::var("GITHUB_OUTPUT") {
            let mut gh_out_file = OpenOptions::new()
                .append(true)
                .open(gh_out)
                .expect("GITHUB_OUTPUT file could not be opened");
            if let Err(e) = writeln!(
                gh_out_file,
                "checks-failed={}\nformat-checks-failed={}\ntidy-checks-failed={}",
                checks_failed,
                format_checks_failed.unwrap_or(0),
                tidy_checks_failed.unwrap_or(0),
            ) {
                panic!("Could not write to GITHUB_OUTPUT file: {}", e);
            }
        }
        log::info!(
            "{} clang-format-checks-failed",
            format_checks_failed.unwrap_or(0)
        );
        log::info!(
            "{} clang-tidy-checks-failed",
            tidy_checks_failed.unwrap_or(0)
        );
        log::info!("{checks_failed} checks-failed");
        checks_failed
    }

    fn make_headers() -> HeaderMap<HeaderValue> {
        let mut headers = HeaderMap::new();
        let return_fmt = "application/vnd.github.raw+json".to_owned();
        headers.insert("Accept", return_fmt.parse().unwrap());
        // headers.insert("User-Agent", USER_AGENT.parse().unwrap());
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            let mut val = HeaderValue::from_str(token.as_str())
                .expect("Failed to create a secure header value for the API token.");
            val.set_sensitive(true);
            headers.insert(AUTHORIZATION, val);
        }
        headers
    }

    async fn get_list_of_changed_files(&self, file_filter: &FileFilter) -> Vec<FileObj> {
        if env::var("CI").is_ok_and(|val| val.as_str() == "true")
            && self.repo.is_some()
            && self.sha.is_some()
        {
            // get diff from Github REST API
            let is_pr = self.event_name == "pull_request";
            let pr = self.pull_request.unwrap_or(-1).to_string();
            let sha = self.sha.clone().unwrap();
            let url = self
                .api_url
                .join("repos/")
                .unwrap()
                .join(format!("{}/", self.repo.as_ref().unwrap()).as_str())
                .unwrap()
                .join(if is_pr { "pulls/" } else { "commits/" })
                .unwrap()
                .join(if is_pr { pr.as_str() } else { sha.as_str() })
                .unwrap();
            let mut diff_header = HeaderMap::new();
            diff_header.insert("Accept", "application/vnd.github.diff".parse().unwrap());
            let request =
                Self::make_api_request(&self.client, url, Method::GET, None, Some(diff_header));
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                true,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();

            parse_diff_from_buf(&response, file_filter)
        } else {
            // get diff from libgit2 API
            let repo = open_repo(".")
                .expect("Please ensure the repository is checked out before running cpp-linter.");
            let list = parse_diff(&get_diff(&repo), file_filter);
            list
        }
    }

    async fn post_feedback(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        user_inputs: FeedbackInput,
    ) -> u64 {
        let format_checks_failed = tally_format_advice(files);
        let tidy_checks_failed = tally_tidy_advice(files);
        let mut comment = None;

        if user_inputs.file_annotations {
            self.post_annotations(files, user_inputs.style.as_str());
        }
        if user_inputs.step_summary {
            comment =
                Some(self.make_comment(files, format_checks_failed, tidy_checks_failed, None));
            self.post_step_summary(comment.as_ref().unwrap());
        }
        self.set_exit_code(
            format_checks_failed + tidy_checks_failed,
            Some(format_checks_failed),
            Some(tidy_checks_failed),
        );

        if user_inputs.thread_comments != ThreadComments::Off {
            // post thread comment for PR or push event
            if comment.as_ref().is_some_and(|c| c.len() > 65535) || comment.is_none() {
                comment = Some(self.make_comment(
                    files,
                    format_checks_failed,
                    tidy_checks_failed,
                    Some(65535),
                ));
            }
            if let Some(repo) = &self.repo {
                let is_pr = self.event_name == "pull_request";
                let pr = self.pull_request.unwrap_or(-1).to_string() + "/";
                let sha = self.sha.clone().unwrap() + "/";
                let comments_url = self
                    .api_url
                    .join("repos/")
                    .unwrap()
                    .join(format!("{}/", repo).as_str())
                    .unwrap()
                    .join(if is_pr { "issues/" } else { "commits/" })
                    .unwrap()
                    .join(if is_pr { pr.as_str() } else { sha.as_str() })
                    .unwrap()
                    .join("comments/")
                    .unwrap();

                self.update_comment(
                    comments_url,
                    &comment.unwrap(),
                    user_inputs.no_lgtm,
                    format_checks_failed + tidy_checks_failed == 0,
                    user_inputs.thread_comments == ThreadComments::Update,
                )
                .await;
            }
        }
        format_checks_failed + tidy_checks_failed
    }
}

impl GithubApiClient {
    fn post_step_summary(&self, comment: &String) {
        if let Ok(gh_out) = env::var("GITHUB_STEP_SUMMARY") {
            let mut gh_out_file = OpenOptions::new()
                .append(true)
                .open(gh_out)
                .expect("GITHUB_STEP_SUMMARY file could not be opened");
            if let Err(e) = writeln!(gh_out_file, "\n{}\n", comment) {
                panic!("Could not write to GITHUB_STEP_SUMMARY file: {}", e);
            }
        }
    }

    fn post_annotations(&self, files: &[Arc<Mutex<FileObj>>], style: &str) {
        // formalize the style guide name
        let style_guide =
            if ["google", "chromium", "microsoft", "mozilla", "webkit"].contains(&style) {
                // capitalize the first letter
                let mut char_iter = style.chars();
                match char_iter.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + char_iter.as_str(),
                }
            } else if style == "llvm" || style == "gnu" {
                style.to_ascii_uppercase()
            } else {
                String::from("Custom")
            };

        // iterate over clang-format advice and post annotations
        for file in files {
            let file = file.lock().unwrap();
            if let Some(format_advice) = &file.format_advice {
                // assemble a list of line numbers
                let mut lines: Vec<usize> = Vec::new();
                for replacement in &format_advice.replacements {
                    if let Some(line_int) = replacement.line {
                        if !lines.contains(&line_int) {
                            lines.push(line_int);
                        }
                    }
                }
                // post annotation if any applicable lines were formatted
                if !lines.is_empty() {
                    println!(
                            "::notice file={name},title=Run clang-format on {name}::File {name} does not conform to {style_guide} style guidelines. (lines {line_set})",
                            name = &file.name.to_string_lossy().replace('\\', "/"),
                            line_set = lines.iter().map(|val| val.to_string()).collect::<Vec<_>>().join(","),
                        );
                }
            } // end format_advice iterations

            // iterate over clang-tidy advice and post annotations
            // The tidy_advice vector is parallel to the files vector; meaning it serves as a file filterer.
            // lines are already filter as specified to clang-tidy CLI.
            if let Some(tidy_advice) = &file.tidy_advice {
                for note in &tidy_advice.notes {
                    if note.filename == file.name.to_string_lossy().replace('\\', "/") {
                        println!(
                            "::{severity} file={file},line={line},title={file}:{line}:{cols} [{diag}]::{info}",
                            severity = if note.severity == *"note" { "notice".to_string() } else {note.severity.clone()},
                            file = note.filename,
                            line = note.line,
                            cols = note.cols,
                            diag = note.diagnostic,
                            info = note.rationale,
                        );
                    }
                }
            }
        }
    }

    /// update existing comment or remove old comment(s) and post a new comment
    async fn update_comment(
        &self,
        url: Url,
        comment: &String,
        no_lgtm: bool,
        is_lgtm: bool,
        update_only: bool,
    ) {
        let comment_url = self
            .remove_bot_comments(&url, !update_only || (is_lgtm && no_lgtm))
            .await;
        #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
        if (is_lgtm && !no_lgtm) || !is_lgtm {
            let payload = HashMap::from([("body", comment.to_owned())]);
            #[cfg(not(test))]
            log::debug!("payload body:\n{:?}", payload);
            let req_meth = if comment_url.is_some() {
                Method::PATCH
            } else {
                Method::POST
            };
            let request = Self::make_api_request(
                &self.client,
                if let Some(url_) = comment_url {
                    url_
                } else {
                    url
                },
                req_meth,
                Some(payload),
                None,
            );
            Self::send_api_request(
                self.client.clone(),
                request,
                false,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await;
        }
    }

    async fn remove_bot_comments(&self, url: &Url, delete: bool) -> Option<Url> {
        let mut comment_url = None;
        let mut comments_url = Some(
            Url::parse_with_params(url.as_str(), &[("page", "1")])
                .expect("Failed to parse invalid URL string"),
        );
        let repo = format!(
            "repos/{}/comments/",
            self.repo.as_ref().expect("Repo name unknown.")
        );
        let base_comment_url = self.api_url.join(&repo).unwrap();
        while let Some(ref endpoint) = comments_url {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None);
            match Self::send_api_request(
                self.client.clone(),
                request,
                false,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await
            {
                None => {
                    log::error!("Failed to get list of existing comments from {}", endpoint);
                    return comment_url;
                }
                Some(response) => {
                    if !response.status().is_success() {
                        log::error!("Failed to get list of existing comments from {}", endpoint);
                        return comment_url;
                    }
                    comments_url = Self::try_next_page(response.headers());
                    let payload: Vec<Comment> = response
                        .json()
                        .await
                        .expect("Unable to deserialize malformed JSON about comments");
                    for comment in payload {
                        if comment.body.starts_with(COMMENT_MARKER) {
                            log::debug!(
                                "comment id {} from user {} ({})",
                                comment.id,
                                comment.user.login,
                                comment.user.id,
                            );
                            #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
                            if delete || (!delete && comment_url.is_none()) {
                                // if not updating: remove all outdated comments
                                // if updating: remove all outdated comments except the last one

                                // use last saved comment_url (if not None) or current comment url
                                let del_url = if let Some(last_url) = &comment_url {
                                    last_url
                                } else {
                                    let comment_id = comment.id.to_string();
                                    &base_comment_url
                                        .join(&comment_id)
                                        .expect("Failed to parse URL from JSON comment.url")
                                };
                                let req = Self::make_api_request(
                                    &self.client,
                                    del_url.clone(),
                                    Method::DELETE,
                                    None,
                                    None,
                                );
                                match Self::send_api_request(
                                    self.client.clone(),
                                    req,
                                    false,
                                    self.rate_limit_headers.to_owned(),
                                    0,
                                )
                                .await
                                {
                                    Some(res) => {
                                        log::info!(
                                            "Got {} from DELETE {}",
                                            res.status(),
                                            del_url.path(),
                                        )
                                    }
                                    None => {
                                        log::error!("Unable to remove old bot comment");
                                        // exit early as this is most likely due to rate limit.
                                        return comment_url;
                                    }
                                }
                            }
                            if !delete {
                                let comment_id = comment.id.to_string();
                                comment_url = Some(
                                    base_comment_url
                                        .join(&comment_id)
                                        .expect("Failed to parse URL from JSON comment.url"),
                                )
                            }
                        }
                    }
                }
            }
        }
        comment_url
    }
}

/// A structure for deserializing a comment from a response's json.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Comment {
    /// The comment's ID number.
    pub id: i64,
    /// The comment's url number.
    pub url: String,
    /// The comment's body number.
    pub body: String,
    /// The comment's user number.
    ///
    /// This is only used for debug output.
    pub user: User,
}

/// A structure for deserializing a comment's author from a response's json.
///
/// This is only used for debug output.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct User {
    pub login: String,
    pub id: u64,
}

#[cfg(test)]
mod test {
    use std::{
        env,
        io::Read,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use regex::Regex;
    use tempfile::{tempdir, NamedTempFile};

    use super::GithubApiClient;
    use crate::{
        clang_tools::capture_clang_tools_output,
        cli::{ClangParams, FeedbackInput, LinesChangedOnly},
        common_fs::{FileFilter, FileObj},
        rest_api::{RestApiClient, USER_OUTREACH},
    };

    // ************************* tests for step-summary and output variables

    async fn create_comment(tidy_checks: &str, style: &str) -> (String, String) {
        let tmp_dir = tempdir().unwrap();
        let rest_api_client = GithubApiClient::default();
        if env::var("ACTIONS_STEP_DEBUG").is_ok_and(|var| var == "true") {
            assert!(rest_api_client.debug_enabled);
        }
        let mut files = vec![Arc::new(Mutex::new(FileObj::new(PathBuf::from(
            "tests/demo/demo.cpp",
        ))))];
        let mut clang_params = ClangParams {
            tidy_checks: tidy_checks.to_string(),
            lines_changed_only: LinesChangedOnly::Off,
            database: None,
            extra_args: None,
            database_json: None,
            style: style.to_string(),
            clang_tidy_command: None,
            clang_format_command: None,
            tidy_filter: FileFilter::new(&[], vec!["cpp".to_string(), "hpp".to_string()]),
            format_filter: FileFilter::new(&[], vec!["cpp".to_string(), "hpp".to_string()]),
        };
        capture_clang_tools_output(
            &mut files,
            env::var("CLANG-VERSION").unwrap_or("".to_string()).as_str(),
            &mut clang_params,
        )
        .await;
        let feedback_inputs = FeedbackInput {
            style: style.to_string(),
            step_summary: true,
            ..Default::default()
        };
        let mut step_summary_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        env::set_var("GITHUB_STEP_SUMMARY", step_summary_path.path());
        let mut gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        env::set_var("GITHUB_OUTPUT", gh_out_path.path());
        rest_api_client.post_feedback(&files, feedback_inputs).await;
        let mut step_summary_content = String::new();
        step_summary_path
            .read_to_string(&mut step_summary_content)
            .unwrap();
        assert!(&step_summary_content.contains(USER_OUTREACH));
        let mut gh_out_content = String::new();
        gh_out_path.read_to_string(&mut gh_out_content).unwrap();
        assert!(gh_out_content.starts_with("checks-failed="));
        (step_summary_content, gh_out_content)
    }

    #[tokio::test]
    async fn check_comment_concerns() {
        let (comment, gh_out) = create_comment("readability-*", "file").await;
        assert!(&comment.contains(":warning:\nSome files did not pass the configured checks!\n"));
        let fmt_pattern = Regex::new(r"format-checks-failed=(\d+)\n").unwrap();
        let tidy_pattern = Regex::new(r"tidy-checks-failed=(\d+)\n").unwrap();
        for pattern in [fmt_pattern, tidy_pattern] {
            let number = pattern
                .captures(&gh_out)
                .expect("found no number of checks-failed")
                .get(1)
                .unwrap()
                .as_str()
                .parse::<u64>()
                .unwrap();
            assert!(number > 0);
        }
    }

    #[tokio::test]
    async fn check_comment_lgtm() {
        env::set_var("ACTIONS_STEP_DEBUG", "true");
        let (comment, gh_out) = create_comment("-*", "").await;
        assert!(&comment.contains(":heavy_check_mark:\nNo problems need attention."));
        assert_eq!(
            &gh_out,
            "checks-failed=0\nformat-checks-failed=0\ntidy-checks-failed=0\n"
        );
    }
}
