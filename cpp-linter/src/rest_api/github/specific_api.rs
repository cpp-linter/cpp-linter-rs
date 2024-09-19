//! This submodule implements functionality exclusively specific to Github's REST API.

use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use reqwest::{Client, Method, Url};

use crate::{
    clang_tools::{clang_format::summarize_style, ReviewComments},
    cli::FeedbackInput,
    common_fs::FileObj,
    rest_api::{RestApiRateLimitHeaders, COMMENT_MARKER},
};

use super::{
    serde_structs::{FullReview, PullRequestInfo, ReviewComment, ReviewDiffComment, ThreadComment},
    GithubApiClient, RestApiClient,
};

impl Default for GithubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApiClient {
    /// Instantiate a [`GithubApiClient`] object.
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

    /// Append step summary to CI workflow's summary page.
    pub fn post_step_summary(&self, comment: &String) {
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

    /// Post file annotations.
    pub fn post_annotations(&self, files: &[Arc<Mutex<FileObj>>], style: &str) {
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

    /// Update existing comment or remove old comment(s) and post a new comment
    pub async fn update_comment(
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

    /// Remove thread comments previously posted by cpp-linter.
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
