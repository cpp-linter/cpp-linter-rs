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
use serde::{Deserialize, Serialize};
use serde_json;

// project specific modules/crates
use crate::clang_tools::clang_format::{summarize_style, tally_format_advice};
use crate::clang_tools::clang_tidy::tally_tidy_advice;
use crate::clang_tools::{ReviewComments, Suggestion};
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
            for (prompt, value) in [
                ("checks-failed", Some(checks_failed)),
                ("format-checks-failed", format_checks_failed),
                ("tidy-checks-failed", tidy_checks_failed),
            ] {
                if let Err(e) = writeln!(gh_out_file, "{prompt}={}", value.unwrap_or(0),) {
                    log::error!("Could not write to GITHUB_OUTPUT file: {}", e);
                    break;
                }
            }
            gh_out_file
                .flush()
                .expect("Failed to flush buffer to GITHUB_OUTPUT file");
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
        headers.insert(
            "Accept",
            HeaderValue::from_str("application/vnd.github.raw+json")
                .expect("Failed to create a header value for the API return data type"),
        );
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
            let request = Self::make_api_request(
                &self.client,
                url.as_str(),
                Method::GET,
                None,
                Some(diff_header),
            );
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                false,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await;
            match response {
                Some(response) => {
                    if response.status.is_success() {
                        return parse_diff_from_buf(response.text.as_bytes(), file_filter);
                    } else {
                        let endpoint = if is_pr {
                            Url::parse(format!("{}/files", url.as_str()).as_str())
                                .expect("failed to parse URL endpoint")
                        } else {
                            url
                        };
                        self.get_changed_files_paginated(endpoint, file_filter)
                            .await
                    }
                }
                None => panic!("Failed to get list of changed files."),
            }
        } else {
            // get diff from libgit2 API
            let repo = open_repo(".")
                .expect("Please ensure the repository is checked out before running cpp-linter.");
            let list = parse_diff(&get_diff(&repo), file_filter);
            list
        }
    }

    async fn get_changed_files_paginated(
        &self,
        url: Url,
        file_filter: &FileFilter,
    ) -> Vec<FileObj> {
        let mut url = Some(Url::parse_with_params(url.as_str(), &[("page", "1")]).unwrap());
        let mut files = vec![];
        while let Some(ref endpoint) = url {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None);
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                true,
                self.rate_limit_headers.clone(),
                0,
            )
            .await;
            if let Some(response) = response {
                url = Self::try_next_page(&response.headers);
                let files_list = if self.event_name != "pull_request" {
                    let json_value: PushEventFiles = serde_json::from_str(&response.text)
                        .expect("Failed to deserialize list of changed files from json response");
                    json_value.files
                } else {
                    serde_json::from_str::<Vec<GithubChangedFile>>(&response.text).expect(
                        "Failed to deserialize list of file changes from Pull Request event.",
                    )
                };
                for file in files_list {
                    if let Some(patch) = file.patch {
                        let diff = format!(
                            "diff --git a/{old} b/{new}\n--- a/{old}\n+++ b/{new}\n{patch}",
                            old = file.previous_filename.unwrap_or(file.filename.clone()),
                            new = file.filename,
                        );
                        if let Some(file_obj) =
                            parse_diff_from_buf(diff.as_bytes(), file_filter).first()
                        {
                            files.push(file_obj.to_owned());
                        }
                    }
                }
            }
        }
        files
    }

    async fn post_feedback(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        feedback_inputs: FeedbackInput,
    ) -> u64 {
        let tidy_checks_failed = tally_tidy_advice(files);
        let format_checks_failed = tally_format_advice(files);
        let mut comment = None;

        if feedback_inputs.file_annotations {
            self.post_annotations(files, feedback_inputs.style.as_str());
        }
        if feedback_inputs.step_summary {
            comment =
                Some(self.make_comment(files, format_checks_failed, tidy_checks_failed, None));
            self.post_step_summary(comment.as_ref().unwrap());
        }
        self.set_exit_code(
            format_checks_failed + tidy_checks_failed,
            Some(format_checks_failed),
            Some(tidy_checks_failed),
        );

        if feedback_inputs.thread_comments != ThreadComments::Off {
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
                    feedback_inputs.no_lgtm,
                    format_checks_failed + tidy_checks_failed == 0,
                    feedback_inputs.thread_comments == ThreadComments::Update,
                )
                .await;
            }
        }
        if self.event_name == "pull_request"
            && (feedback_inputs.tidy_review || feedback_inputs.format_review)
        {
            self.post_review(files, &feedback_inputs).await;
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
                log::error!("Could not write to GITHUB_STEP_SUMMARY file: {}", e);
            }
        }
    }

    fn post_annotations(&self, files: &[Arc<Mutex<FileObj>>], style: &str) {
        let style_guide = summarize_style(style);

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
            // log::debug!("payload body:\n{:?}", payload);
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
                Some(
                    serde_json::to_string(&payload)
                        .expect("Failed to serialize thread comment to json string"),
                ),
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
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                false,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await;
            if response.is_none() || response.as_ref().is_some_and(|r| !r.status.is_success()) {
                log::error!("Failed to get list of existing comments from {}", endpoint);
                return comment_url;
            }
            comments_url = Self::try_next_page(&response.as_ref().unwrap().headers);
            let payload: Vec<ThreadComment> = serde_json::from_str(&response.unwrap().text)
                .expect("Failed to serialize response's text");
            for comment in payload {
                if comment.body.starts_with(COMMENT_MARKER) {
                    log::debug!(
                        "comment id {} from user {} ({})",
                        comment.id,
                        comment.user.login,
                        comment.user.id,
                    );
                    #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
                    if delete || (!delete && comment_url.is_some()) {
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
                        Self::send_api_request(
                            self.client.clone(),
                            req,
                            false,
                            self.rate_limit_headers.to_owned(),
                            0,
                        )
                        .await;
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
        comment_url
    }

    /// Post a PR review with code suggestions.
    ///
    /// Note: `--no-lgtm` is applied when nothing is suggested.
    pub async fn post_review(&self, files: &[Arc<Mutex<FileObj>>], feedback_input: &FeedbackInput) {
        let url = self
            .api_url
            .join("repos/")
            .unwrap()
            .join(format!("{}/", self.repo.as_ref().expect("Repo name unknown")).as_str())
            .unwrap()
            .join("pulls/")
            .unwrap()
            .join(
                self.pull_request
                    .expect("pull request number unknown")
                    .to_string()
                    .as_str(),
            )
            .unwrap();
        let request = Self::make_api_request(&self.client, url.as_str(), Method::GET, None, None);
        let response = Self::send_api_request(
            self.client.clone(),
            request,
            true,
            self.rate_limit_headers.clone(),
            0,
        )
        .await;
        let pr_info: PullRequestInfo =
            serde_json::from_str(&response.expect("Failed to get PR info").text)
                .expect("Failed to deserialize PR info");

        let url = Url::parse(format!("{}/", url.as_str()).as_str())
            .unwrap()
            .join("reviews")
            .expect("Failed to parse URL endpoint for PR reviews");
        let dismissal = self.dismiss_outdated_reviews(&url);

        if pr_info.draft || pr_info.state != "open" {
            dismissal.await;
            return;
        }

        let summary_only =
            env::var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY").unwrap_or("false".to_string()) == "true";

        let mut review_comments = ReviewComments::default();
        for file in files {
            let file = file.lock().unwrap();
            file.make_suggestions_from_patch(&mut review_comments, summary_only);
        }
        let has_no_changes =
            review_comments.full_patch[0].is_empty() && review_comments.full_patch[1].is_empty();
        if has_no_changes && feedback_input.no_lgtm {
            log::debug!("Not posting an approved review because `no-lgtm` is true");
            dismissal.await;
            return;
        }
        let mut payload = FullReview {
            event: if feedback_input.passive_reviews {
                String::from("COMMENT")
            } else if has_no_changes {
                String::from("APPROVE")
            } else {
                String::from("REQUEST_CHANGES")
            },
            body: String::new(),
            comments: vec![],
        };
        payload.body = review_comments.summarize();
        if !summary_only {
            payload.comments = {
                let mut comments = vec![];
                for comment in review_comments.comments {
                    comments.push(ReviewDiffComment::from(comment));
                }
                comments
            };
        }
        dismissal.await; // free up the `url` variable
        let request = Self::make_api_request(
            &self.client,
            url,
            Method::POST,
            Some(
                serde_json::to_string(&payload)
                    .expect("Failed to serialize PR review to json string"),
            ),
            None,
        );
        let response = Self::send_api_request(
            self.client.clone(),
            request,
            false,
            self.rate_limit_headers.clone(),
            0,
        )
        .await;
        if response.is_none() || response.is_some_and(|r| !r.status.is_success()) {
            log::error!("Failed to post a new PR review");
        }
    }

    /// Dismiss any outdated reviews generated by cpp-linter.
    async fn dismiss_outdated_reviews(&self, url: &Url) {
        let mut url_ = Some(
            Url::parse_with_params(url.as_str(), [("page", "1")])
                .expect("Failed to parse endpoint for getting existing PR reviews"),
        );
        while let Some(ref endpoint) = url_ {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None);
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                false,
                self.rate_limit_headers.clone(),
                0,
            )
            .await;
            if response.is_none() || response.as_ref().is_some_and(|r| !r.status.is_success()) {
                log::error!("Failed to get a list of existing PR reviews");
                return;
            }
            let response = response.unwrap();
            url_ = Self::try_next_page(&response.headers);
            let payload: Vec<ReviewComment> = serde_json::from_str(&response.text)
                .expect("Unable to deserialize malformed JSON about review comments");
            for review in payload {
                if let Some(body) = &review.body {
                    if body.starts_with(COMMENT_MARKER)
                        && !(["PENDING", "DISMISSED"].contains(&review.state.as_str()))
                    {
                        // dismiss outdated review
                        let req = Self::make_api_request(
                            &self.client,
                            url.join("reviews/")
                                .unwrap()
                                .join(review.id.to_string().as_str())
                                .expect("Failed to parse URL for dismissing outdated review."),
                            Method::PUT,
                            Some(
                                serde_json::json!(
                                    {
                                        "message": "outdated suggestion",
                                        "event": "DISMISS"
                                    }
                                )
                                .to_string(),
                            ),
                            None,
                        );
                        let result = Self::send_api_request(
                            self.client.clone(),
                            req,
                            false,
                            self.rate_limit_headers.clone(),
                            0,
                        )
                        .await;
                        if result.is_none() || result.is_some_and(|r| !r.status.is_success()) {
                            log::error!("Failed to dismiss outdated review");
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct FullReview {
    pub event: String,
    pub body: String,
    pub comments: Vec<ReviewDiffComment>,
}

#[derive(Debug, Serialize)]
struct ReviewDiffComment {
    pub body: String,
    pub line: i64,
    pub start_line: Option<i64>,
    pub path: String,
}

impl From<Suggestion> for ReviewDiffComment {
    fn from(value: Suggestion) -> Self {
        Self {
            body: value.suggestion,
            line: value.line_end as i64,
            start_line: if value.line_end != value.line_start {
                Some(value.line_start as i64)
            } else {
                None
            },
            path: value.path,
        }
    }
}

/// A structure for deserializing a single changed file in a CI event.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct GithubChangedFile {
    /// The file's name (including relative path to repo root)
    pub filename: String,
    /// If renamed, this will be the file's old name as a [`Some`], otherwise [`None`].
    pub previous_filename: Option<String>,
    /// The individual patch that describes the file's changes.
    pub patch: Option<String>,
}

/// A structure for deserializing a Push event's changed files.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct PushEventFiles {
    /// The list of changed files.
    pub files: Vec<GithubChangedFile>,
}

/// A structure for deserializing a comment from a response's json.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct PullRequestInfo {
    /// Is this PR a draft?
    pub draft: bool,
    /// What is current state of this PR?
    ///
    /// Here we only care if it is `"open"`.
    pub state: String,
}

/// A structure for deserializing a comment from a response's json.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct ReviewComment {
    /// The content of the review's summary comment.
    pub body: Option<String>,
    /// The review's ID.
    pub id: i64,
    /// The state of the review in question.
    ///
    /// This could be "PENDING", "DISMISSED", "APPROVED", or "COMMENT".
    pub state: String,
}

/// A structure for deserializing a comment from a response's json.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct ThreadComment {
    /// The comment's ID number.
    pub id: i64,
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
        default::Default,
        env,
        io::Read,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use chrono::Utc;
    use mockito::{Matcher, Server};
    use regex::Regex;
    use reqwest::{Method, Url};
    use tempfile::{tempdir, NamedTempFile};

    use super::GithubApiClient;
    use crate::{
        clang_tools::capture_clang_tools_output,
        cli::{ClangParams, FeedbackInput, LinesChangedOnly},
        common_fs::FileObj,
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
            style: style.to_string(),
            ..Default::default()
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

    async fn simulate_rate_limit(secondary: bool) {
        let mut server = Server::new_async().await;
        let url = Url::parse(server.url().as_str()).unwrap();
        env::set_var("GITHUB_API_URL", server.url());
        let client = GithubApiClient::default();
        let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
        let mock = server
            .mock("GET", "/")
            .match_body(Matcher::Any)
            .expect_at_least(1)
            .expect_at_most(5)
            .with_status(429)
            .with_header(
                &client.rate_limit_headers.remaining,
                if secondary { "1" } else { "0" },
            )
            .with_header(&client.rate_limit_headers.reset, &reset_timestamp);
        if secondary {
            mock.with_header(&client.rate_limit_headers.retry, "0")
                .create();
        } else {
            mock.create();
        }
        let request =
            GithubApiClient::make_api_request(&client.client, url, Method::GET, None, None);
        GithubApiClient::send_api_request(
            client.client.clone(),
            request,
            true,
            client.rate_limit_headers.clone(),
            0,
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    #[should_panic(expected = "REST API secondary rate limit exceeded")]
    async fn secondary_rate_limit() {
        simulate_rate_limit(true).await;
    }

    #[tokio::test]
    #[ignore]
    #[should_panic(expected = "REST API rate limit exceeded!")]
    async fn primary_rate_limit() {
        simulate_rate_limit(false).await;
    }
}
