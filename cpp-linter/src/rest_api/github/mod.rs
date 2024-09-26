//! This module holds functionality specific to using Github's REST API.
//!
//! In the root module, we just implement the RestApiClient trait.
//! In other (private) submodules we implement behavior specific to Github's REST API.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

// non-std crates
use anyhow::{anyhow, Context, Result};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client, Method, Url,
};
use serde_json;

// project specific modules/crates
use super::{RestApiClient, RestApiRateLimitHeaders};
use crate::clang_tools::clang_format::tally_format_advice;
use crate::clang_tools::clang_tidy::tally_tidy_advice;
use crate::cli::{FeedbackInput, ThreadComments};
use crate::common_fs::{FileFilter, FileObj};
use crate::git::{get_diff, open_repo, parse_diff, parse_diff_from_buf};

// private submodules.
mod serde_structs;
mod specific_api;
use serde_structs::{GithubChangedFile, PushEventFiles};

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

// implement the RestApiClient trait for the GithubApiClient
impl RestApiClient for GithubApiClient {
    fn set_exit_code(
        &self,
        checks_failed: u64,
        format_checks_failed: Option<u64>,
        tidy_checks_failed: Option<u64>,
    ) -> u64 {
        if let Ok(gh_out) = env::var("GITHUB_OUTPUT") {
            if let Ok(mut gh_out_file) = OpenOptions::new().append(true).open(gh_out) {
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
                if let Err(e) = gh_out_file.flush() {
                    log::debug!("Failed to flush buffer to GITHUB_OUTPUT file: {e:?}");
                }
            } else {
                log::debug!("GITHUB_OUTPUT file could not be opened");
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

    fn make_headers() -> Result<HeaderMap<HeaderValue>> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Accept",
            HeaderValue::from_str("application/vnd.github.raw+json")?,
        );
        // headers.insert("User-Agent", USER_AGENT.parse().unwrap());
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            let mut val = HeaderValue::from_str(token.as_str())?;
            val.set_sensitive(true);
            headers.insert(AUTHORIZATION, val);
        }
        Ok(headers)
    }

    async fn get_list_of_changed_files(&self, file_filter: &FileFilter) -> Result<Vec<FileObj>> {
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
                .join("repos/")?
                .join(format!("{}/", self.repo.as_ref().unwrap()).as_str())?
                .join(if is_pr { "pulls/" } else { "commits/" })?
                .join(if is_pr { pr.as_str() } else { sha.as_str() })?;
            let mut diff_header = HeaderMap::new();
            diff_header.insert("Accept", "application/vnd.github.diff".parse()?);
            let request = Self::make_api_request(
                &self.client,
                url.as_str(),
                Method::GET,
                None,
                Some(diff_header),
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
                    if response.status().is_success() {
                        Ok(parse_diff_from_buf(&response.bytes().await?, file_filter))
                    } else {
                        let endpoint = if is_pr {
                            Url::parse(format!("{}/files", url.as_str()).as_str())?
                        } else {
                            url
                        };
                        self.get_changed_files_paginated(endpoint, file_filter)
                            .await
                    }
                }
                Err(e) => Err(anyhow!(
                    "Failed to connect with GitHub server to get list of changed files."
                )
                .context(e)),
            }
        } else {
            // get diff from libgit2 API
            let repo = open_repo(".").with_context(|| {
                "Please ensure the repository is checked out before running cpp-linter."
            })?;
            let list = parse_diff(&get_diff(&repo)?, file_filter);
            Ok(list)
        }
    }

    async fn get_changed_files_paginated(
        &self,
        url: Url,
        file_filter: &FileFilter,
    ) -> Result<Vec<FileObj>> {
        let mut url = Some(Url::parse_with_params(url.as_str(), &[("page", "1")])?);
        let mut files = vec![];
        while let Some(ref endpoint) = url {
            let request =
                Self::make_api_request(&self.client, endpoint.as_str(), Method::GET, None, None)?;
            let response = Self::send_api_request(
                self.client.clone(),
                request,
                self.rate_limit_headers.clone(),
                0,
            )
            .await;
            if let Ok(response) = response {
                url = Self::try_next_page(response.headers());
                let files_list = if self.event_name != "pull_request" {
                    let json_value: PushEventFiles = serde_json::from_str(&response.text().await?)
                        .with_context(|| {
                            "Failed to deserialize list of changed files from json response"
                        })?;
                    json_value.files
                } else {
                    serde_json::from_str::<Vec<GithubChangedFile>>(&response.text().await?)
                        .with_context(|| {
                            "Failed to deserialize list of file changes from Pull Request event."
                        })?
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
        Ok(files)
    }

    async fn post_feedback(
        &self,
        files: &[Arc<Mutex<FileObj>>],
        feedback_inputs: FeedbackInput,
    ) -> Result<u64> {
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
                .await?;
            }
        }
        if self.event_name == "pull_request"
            && (feedback_inputs.tidy_review || feedback_inputs.format_review)
        {
            self.post_review(files, &feedback_inputs).await?;
        }
        Ok(format_checks_failed + tidy_checks_failed)
    }
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
        let rest_api_client = GithubApiClient::new().unwrap();
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
        .await
        .unwrap();
        let feedback_inputs = FeedbackInput {
            style: style.to_string(),
            step_summary: true,
            ..Default::default()
        };
        let mut step_summary_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        env::set_var("GITHUB_STEP_SUMMARY", step_summary_path.path());
        let mut gh_out_path = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        env::set_var("GITHUB_OUTPUT", gh_out_path.path());
        rest_api_client
            .post_feedback(&files, feedback_inputs)
            .await
            .unwrap();
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
        let client = GithubApiClient::new().unwrap();
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
            GithubApiClient::make_api_request(&client.client, url, Method::GET, None, None)
                .unwrap();
        GithubApiClient::send_api_request(
            client.client.clone(),
            request,
            client.rate_limit_headers.clone(),
            0,
        )
        .await
        .unwrap();
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
