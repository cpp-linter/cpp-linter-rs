//! This submodule implements functionality exclusively specific to Github's REST API.

use std::{
    collections::HashMap,
    env,
    fs::OpenOptions,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use reqwest::{Client, Method, Url};

use crate::{
    clang_tools::{clang_format::summarize_style, ReviewComments},
    cli::FeedbackInput,
    common_fs::FileObj,
    rest_api::{RestApiRateLimitHeaders, COMMENT_MARKER, USER_AGENT},
};

use super::{
    serde_structs::{FullReview, PullRequestInfo, ReviewComment, ReviewDiffComment, ThreadComment},
    GithubApiClient, RestApiClient,
};

impl GithubApiClient {
    /// Instantiate a [`GithubApiClient`] object.
    pub fn new() -> Result<Self> {
        let event_name = env::var("GITHUB_EVENT_NAME").unwrap_or(String::from("unknown"));
        let pull_request = {
            match event_name.as_str() {
                "pull_request" => {
                    let event_payload_path = env::var("GITHUB_EVENT_PATH").with_context(|| {
                        "GITHUB_EVENT_NAME is set to 'pull_request', but GITHUB_EVENT_PATH is not set"
                    })?;
                    let file_buf = &mut String::new();
                    OpenOptions::new()
                        .read(true)
                        .open(event_payload_path.clone())?
                        .read_to_string(file_buf)
                        .with_context(|| {
                            format!("Failed to read event payload at {event_payload_path}")
                        })?;
                    let payload =
                        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                            file_buf,
                        )
                        .with_context(|| "Failed to deserialize event payload")?;
                    payload["number"].as_i64()
                }
                _ => None,
            }
        };
        let gh_api_url = env::var("GITHUB_API_URL").unwrap_or("https://api.github.com".to_string());
        let api_url = Url::parse(gh_api_url.clone().as_str())
            .with_context(|| format!("Failed to parse URL from GITHUB_API_URL: {}", gh_api_url))?;

        Ok(GithubApiClient {
            client: Client::builder()
                .default_headers(Self::make_headers()?)
                .user_agent(USER_AGENT)
                .build()
                .with_context(|| "Failed to create a session client for REST API calls")?,
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
        })
    }

    /// Append step summary to CI workflow's summary page.
    pub fn post_step_summary(&self, comment: &String) {
        if let Ok(gh_out) = env::var("GITHUB_STEP_SUMMARY") {
            if let Ok(mut gh_out_file) = OpenOptions::new().append(true).open(gh_out) {
                if let Err(e) = writeln!(gh_out_file, "\n{}\n", comment) {
                    log::error!("Could not write to GITHUB_STEP_SUMMARY file: {}", e);
                }
            }
            log::error!("GITHUB_STEP_SUMMARY file could not be opened");
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
    ) -> Result<()> {
        let comment_url = self
            .remove_bot_comments(&url, !update_only || (is_lgtm && no_lgtm))
            .await?;
        if !is_lgtm || !no_lgtm {
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
                        .with_context(|| "Failed to serialize thread comment to json string")?,
                ),
                None,
            )?;
            match Self::send_api_request(
                self.client.clone(),
                request,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await
            {
                Ok(response) => {
                    Self::log_response(response, "Failed to post thread comment").await;
                }
                Err(e) => {
                    log::error!("Failed to post thread comment: {e:?}");
                }
            }
        }
        Ok(())
    }

    /// Remove thread comments previously posted by cpp-linter.
    async fn remove_bot_comments(&self, url: &Url, delete: bool) -> Result<Option<Url>> {
        let mut comment_url = None;
        let mut comments_url = Some(
            Url::parse_with_params(url.clone().as_str(), &[("page", "1")]).with_context(|| {
                format!("Failed to parse URL to fetch existing comments: {url}")
            })?,
        );
        let repo = format!(
            "repos/{}/comments",
            // if we got here, then we know it is on a CI runner as self.repo should be known
            self.repo.as_ref().expect("Repo name unknown.")
        );
        let base_comment_url = self.api_url.join(&repo).unwrap();
        while let Some(ref endpoint) = comments_url {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None)?;
            match Self::send_api_request(
                self.client.clone(),
                request,
                self.rate_limit_headers.to_owned(),
                0,
            )
            .await
            {
                Ok(response) => {
                    if !response.status().is_success() {
                        Self::log_response(
                            response,
                            "Failed to get list of existing thread comments",
                        )
                        .await;
                        return Ok(comment_url);
                    }
                    comments_url = Self::try_next_page(response.headers());
                    let payload: Vec<ThreadComment> = serde_json::from_str(&response.text().await?)
                        .with_context(|| {
                            "Failed to deserialize list of existing thread comments"
                        })?;
                    for comment in payload {
                        if comment.body.starts_with(COMMENT_MARKER) {
                            log::debug!(
                                "comment id {} from user {} ({})",
                                comment.id,
                                comment.user.login,
                                comment.user.id,
                            );
                            let this_comment_url =
                                Url::parse(format!("{base_comment_url}/{}", &comment.id).as_str())
                                    .with_context(|| {
                                        format!(
                                            "Failed to parse URL for JSON comment.id: {}",
                                            comment.id
                                        )
                                    })?;
                            if delete || comment_url.is_some() {
                                // if not updating: remove all outdated comments
                                // if updating: remove all outdated comments except the last one

                                // use last saved comment_url (if not None) or current comment url
                                let del_url = if let Some(last_url) = &comment_url {
                                    last_url
                                } else {
                                    &this_comment_url
                                };
                                let req = Self::make_api_request(
                                    &self.client,
                                    del_url.as_str(),
                                    Method::DELETE,
                                    None,
                                    None,
                                )?;
                                match Self::send_api_request(
                                    self.client.clone(),
                                    req,
                                    self.rate_limit_headers.to_owned(),
                                    0,
                                )
                                .await
                                {
                                    Ok(result) => {
                                        if !result.status().is_success() {
                                            Self::log_response(
                                                result,
                                                "Failed to delete old thread comment",
                                            )
                                            .await;
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to delete old thread comment: {e:?}")
                                    }
                                }
                            }
                            if !delete {
                                comment_url = Some(this_comment_url)
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to get list of existing thread comments: {e:?}");
                    return Ok(comment_url);
                }
            }
        }
        Ok(comment_url)
    }

    /// Post a PR review with code suggestions.
    ///
    /// Note: `--no-lgtm` is applied when nothing is suggested.
    pub async fn post_review(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        feedback_input: &FeedbackInput,
    ) -> Result<()> {
        let url = self
            .api_url
            .join("repos/")?
            // if we got here, then we know it is on a CI runner and self.repo should be known
            .join(format!("{}/", self.repo.as_ref().expect("Repo name unknown")).as_str())?
            .join("pulls/")?
            .join(
                self.pull_request
                    // if we got here, then we know that it is a pull_request event
                    .expect("pull request number unknown")
                    .to_string()
                    .as_str(),
            )?;
        let request = Self::make_api_request(&self.client, url.as_str(), Method::GET, None, None)?;
        let response = Self::send_api_request(
            self.client.clone(),
            request,
            self.rate_limit_headers.clone(),
            0,
        )
        .await
        .with_context(|| format!("Failed to get PR info from {}", url.as_str()))?;
        let pr_info: PullRequestInfo = serde_json::from_str(&response.text().await?)
            .with_context(|| "Failed to deserialize PR info")?;

        let url = Url::parse(format!("{}/", url.as_str()).as_str())?
            .join("reviews")
            .with_context(|| "Failed to parse URL endpoint for PR reviews")?;
        let dismissal = self.dismiss_outdated_reviews(&url);

        if pr_info.draft || pr_info.state != "open" {
            return dismissal.await;
        }

        let summary_only = ["true", "on", "1"].contains(
            &env::var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY")
                .unwrap_or("false".to_string())
                .as_str(),
        );

        let mut review_comments = ReviewComments::default();
        for file in files {
            let file = file.lock().unwrap();
            file.make_suggestions_from_patch(&mut review_comments, summary_only)?;
        }
        let has_no_changes =
            review_comments.full_patch[0].is_empty() && review_comments.full_patch[1].is_empty();
        if has_no_changes && feedback_input.no_lgtm {
            log::debug!("Not posting an approved review because `no-lgtm` is true");
            return dismissal.await;
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
        dismissal.await?; // free up the `url` variable
        let request = Self::make_api_request(
            &self.client,
            url.clone(),
            Method::POST,
            Some(
                serde_json::to_string(&payload)
                    .with_context(|| "Failed to serialize PR review to json string")?,
            ),
            None,
        )?;
        match Self::send_api_request(
            self.client.clone(),
            request,
            self.rate_limit_headers.clone(),
            0,
        )
        .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    Self::log_response(response, "Failed to post a new PR review").await;
                }
            }
            Err(e) => {
                log::error!("Failed to post a new PR review: {e:?}");
            }
        }
        Ok(())
    }

    /// Dismiss any outdated reviews generated by cpp-linter.
    async fn dismiss_outdated_reviews(&self, url: &Url) -> Result<()> {
        let mut url_ = Some(
            Url::parse_with_params(url.as_str(), [("page", "1")]).with_context(|| {
                format!(
                    "Failed to parse endpoint for getting existing PR reviews: {}",
                    url.as_str()
                )
            })?,
        );
        while let Some(ref endpoint) = url_ {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None)?;
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                self.rate_limit_headers.clone(),
                0,
            )
            .await;
            if response.is_err() || response.as_ref().is_ok_and(|r| !r.status().is_success()) {
                log::error!(
                    "Failed to get a list of existing PR reviews: {}",
                    endpoint.as_str()
                );
                return Ok(());
            }
            let response = response.unwrap();
            url_ = Self::try_next_page(response.headers());
            match serde_json::from_str::<Vec<ReviewComment>>(&response.text().await?) {
                Ok(payload) => {
                    for review in payload {
                        if let Some(body) = &review.body {
                            if body.starts_with(COMMENT_MARKER)
                                && !(["PENDING", "DISMISSED"].contains(&review.state.as_str()))
                            {
                                // dismiss outdated review
                                match url.join("reviews/")?.join(review.id.to_string().as_str()) {
                                    Ok(dismiss_url) => {
                                        if let Ok(req) = Self::make_api_request(
                                            &self.client,
                                            dismiss_url,
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
                                        ) {
                                            match Self::send_api_request(
                                                self.client.clone(),
                                                req,
                                                self.rate_limit_headers.clone(),
                                                0,
                                            )
                                            .await
                                            {
                                                Ok(result) => {
                                                    if !result.status().is_success() {
                                                        Self::log_response(
                                                            result,
                                                            "Failed to dismiss outdated review",
                                                        )
                                                        .await;
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!(
                                                        "Failed to dismiss outdated review: {e:}"
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse URL for dismissing outdated review: {e:?}");
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Unable to deserialize malformed JSON about review comments: {e:?}")
                }
            }
        }
        Ok(())
    }
}
