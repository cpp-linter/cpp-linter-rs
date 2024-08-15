//! This module holds functionality specific to using Github's REST API.

use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write};

// non-std crates
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Method;
use serde::Deserialize;
use serde_json;

use crate::clang_tools::clang_format::tally_format_advice;
use crate::clang_tools::clang_tidy::tally_tidy_advice;
// project specific modules/crates
use crate::common_fs::FileObj;
use crate::git::{get_diff, open_repo, parse_diff, parse_diff_from_buf};

use super::{FeedbackInput, RestApiClient, COMMENT_MARKER};

static USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0";

/// A structure to work with Github REST API.
pub struct GithubApiClient {
    /// The HTTP request client to be used for all REST API calls.
    client: Client,

    /// The CI run's event payload from the webhook that triggered the workflow.
    pull_request: Option<i64>,

    /// The name of the event that was triggered when running cpp_linter.
    pub event_name: String,

    /// The value of the `GITHUB_API_URL` environment variable.
    api_url: String,

    /// The value of the `GITHUB_REPOSITORY` environment variable.
    repo: Option<String>,

    /// The value of the `GITHUB_SHA` environment variable.
    sha: Option<String>,

    /// The value of the `ACTIONS_STEP_DEBUG` environment variable.
    pub debug_enabled: bool,
}

impl Default for GithubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApiClient {
    pub fn new() -> Self {
        GithubApiClient {
            client: reqwest::blocking::Client::new(),
            pull_request: {
                if let Ok(event_payload_path) = env::var("GITHUB_EVENT_PATH") {
                    let file_buf = &mut String::new();
                    OpenOptions::new()
                        .read(true)
                        .open(event_payload_path)
                        .unwrap()
                        .read_to_string(file_buf)
                        .unwrap();
                    let json = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                        file_buf.as_str(),
                    )
                    .unwrap();
                    json["number"].as_i64()
                } else {
                    None
                }
            },
            event_name: env::var("GITHUB_EVENT_NAME").unwrap_or(String::from("default")),
            api_url: env::var("GITHUB_API_URL").unwrap_or(String::from("https://api.github.com")),
            repo: if let Ok(val) = env::var("GITHUB_REPOSITORY") {
                Some(val)
            } else {
                None
            },
            sha: if let Ok(val) = env::var("GITHUB_SHA") {
                Some(val)
            } else {
                None
            },
            debug_enabled: match env::var("ACTIONS_STEP_DEBUG") {
                Ok(val) => val == "true",
                Err(_) => false,
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

    fn make_headers(&self, use_diff: Option<bool>) -> HeaderMap<HeaderValue> {
        let mut headers = HeaderMap::new();
        let return_fmt = "application/vnd.github.".to_owned()
            + if use_diff.is_some_and(|val| val) {
                "diff"
            } else {
                "raw+json"
            };
        headers.insert("Accept", return_fmt.parse().unwrap());
        headers.insert("User-Agent", USER_AGENT.parse().unwrap());
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            headers.insert("Authorization", token.parse().unwrap());
        }
        headers
    }

    fn get_list_of_changed_files(
        &self,
        extensions: &[&str],
        ignored: &[String],
        not_ignored: &[String],
    ) -> Vec<FileObj> {
        if env::var("CI").is_ok_and(|val| val.as_str() == "true")
            && self.repo.is_some()
            && self.sha.is_some()
        {
            // get diff from Github REST API
            let url = format!(
                "{}/repos/{}/{}",
                self.api_url,
                self.repo.as_ref().unwrap(),
                if self.event_name == "pull_request" {
                    format!(
                        "pulls/{}",
                        &self.pull_request.expect("Pull request number unknown")
                    )
                } else {
                    format!("commits/{}", self.sha.as_ref().unwrap())
                }
            );
            let response = self
                .client
                .get(url)
                .headers(self.make_headers(Some(true)))
                .send()
                .unwrap()
                .bytes()
                .unwrap();

            parse_diff_from_buf(&response, extensions, ignored, not_ignored)
        } else {
            // get diff from libgit2 API
            let repo = open_repo(".")
                .expect("Please ensure the repository is checked out before running cpp-linter.");
            let list = parse_diff(&get_diff(&repo), extensions, ignored, not_ignored);
            list
        }
    }

    fn post_feedback(&self, files: &[FileObj], user_inputs: FeedbackInput) {
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

        if user_inputs.thread_comments.as_str() != "false" {
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
                let base_url = format!("{}/repos/{}/", &self.api_url, &repo);
                let comments_url = if is_pr {
                    format!(
                        "{base_url}issues/{}",
                        &self.pull_request.expect("Pull request number unknown")
                    )
                } else {
                    format!("{base_url}/commits/{}", &self.sha.as_ref().unwrap())
                };

                // get count of comments
                let request = self
                    .client
                    .get(&comments_url)
                    .headers(self.make_headers(None))
                    .send();
                if let Ok(response) = request {
                    let json = response.json::<serde_json::Value>().unwrap();
                    let count = if is_pr {
                        json["comments"].as_u64().unwrap()
                    } else {
                        json["commit"]["comment_count"].as_u64().unwrap()
                    };
                    self.update_comment(
                        &format!("{}/comments", &comments_url),
                        &comment.unwrap(),
                        count,
                        user_inputs.no_lgtm,
                        format_checks_failed + tidy_checks_failed == 0,
                        user_inputs.thread_comments.as_str() == "update",
                    );
                } else {
                    let error = request.unwrap_err();
                    if let Some(status) = error.status() {
                        log::error!(
                            "Could not get comment count. Got response {:?} from {comments_url}",
                            status
                        );
                    } else {
                        log::error!("attempt GET comment count failed");
                    }
                }
            }
        }
    }
}

impl GithubApiClient {
    fn post_step_summary(&self, comment: &String) {
        if let Ok(gh_out) = env::var("GITHUB_STEP_SUMMARY") {
            let mut gh_out_file = OpenOptions::new()
                .append(true)
                .open(gh_out)
                .expect("GITHUB_STEP_SUMMARY file could not be opened");
            if let Err(e) = writeln!(gh_out_file, "\n{}\n", comment,) {
                panic!("Could not write to GITHUB_STEP_SUMMARY file: {}", e);
            }
        }
    }

    fn post_annotations(&self, files: &[FileObj], style: &str) {
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
    #[allow(clippy::too_many_arguments)]
    fn update_comment(
        &self,
        url: &String,
        comment: &String,
        count: u64,
        no_lgtm: bool,
        is_lgtm: bool,
        update_only: bool,
    ) {
        let comment_url =
            self.remove_bot_comments(url, count, !update_only || (is_lgtm && no_lgtm));
        #[allow(clippy::nonminimal_bool)] // an inaccurate assessment
        if (is_lgtm && !no_lgtm) || !is_lgtm {
            let payload = HashMap::from([("body", comment)]);
            log::debug!("payload body:\n{:?}", payload);
            let req_meth = if comment_url.is_some() {
                Method::PATCH
            } else {
                Method::POST
            };
            if let Ok(response) = self
                .client
                .request(
                    req_meth.clone(),
                    if let Some(_url) = comment_url {
                        _url
                    } else {
                        url.to_string()
                    },
                )
                .headers(self.make_headers(None))
                .json(&payload)
                .send()
            {
                log::info!(
                    "Got {} response from {:?}ing comment",
                    response.status(),
                    req_meth,
                );
            }
        }
    }

    fn remove_bot_comments(&self, url: &String, count: u64, delete: bool) -> Option<String> {
        let mut page = 1;
        let mut comment_url = None;
        let mut total = count;
        while total > 0 {
            let request = self.client.get(format!("{url}/?page={page}")).send();
            if request.is_err() {
                log::error!("Failed to get list of existing comments");
                return None;
            } else if let Ok(response) = request {
                let payload: JsonCommentsPayload = response.json().unwrap();
                let mut comment_count = 0;
                for comment in payload.comments {
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
                                &comment.url
                            };
                            if let Ok(response) = self
                                .client
                                .delete(del_url)
                                .headers(self.make_headers(None))
                                .send()
                            {
                                log::info!(
                                    "Got {} from DELETE {}",
                                    response.status(),
                                    del_url.strip_prefix(&self.api_url).unwrap(),
                                )
                            } else {
                                log::error!("Unable to remove old bot comment");
                                return None; // exit early as this is most likely due to rate limit.
                            }
                        }
                        if !delete {
                            comment_url = Some(comment.url)
                        }
                    }
                    comment_count += 1;
                }
                total -= comment_count;
                page += 1;
            }
        }
        comment_url
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct JsonCommentsPayload {
    comments: Vec<Comment>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Comment {
    pub id: i64,
    pub url: String,
    pub body: String,
    pub user: User,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct User {
    pub login: String,
    pub id: u64,
}

#[cfg(test)]
mod test {
    use std::{env, io::Read, path::PathBuf};

    use tempfile::{tempdir, NamedTempFile};

    use super::{GithubApiClient, USER_AGENT};
    use crate::{
        clang_tools::{
            capture_clang_tools_output, clang_format::tally_format_advice,
            clang_tidy::tally_tidy_advice,
        },
        cli::LinesChangedOnly,
        common_fs::FileObj,
        rest_api::RestApiClient,
    };

    // ************************** tests for GithubApiClient::make_headers()
    fn assert_header(use_diff: bool, auth: Option<&str>) {
        let rest_api_client = GithubApiClient::new();
        if let Some(token) = auth {
            env::set_var("GITHUB_TOKEN", token);
        }
        let headers = rest_api_client.make_headers(Some(use_diff));
        assert!(headers.contains_key("User-Agent"));
        assert_eq!(headers.get("User-Agent").unwrap(), USER_AGENT);
        assert!(headers.contains_key("Accept"));
        assert!(headers
            .get("Accept")
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(if use_diff { "diff" } else { "raw+json" }));
        if let Some(token) = auth {
            assert!(headers.contains_key("Authorization"));
            assert_eq!(headers.get("Authorization").unwrap(), token);
        }
    }

    #[test]
    fn get_headers_json_token() {
        assert_header(false, Some("123456"));
    }

    #[test]
    fn get_headers_diff() {
        assert_header(true, None);
    }

    // ************************** tests for GithubApiClient::set_exit_code()

    #[test]
    fn set_exit_code() {
        let rest_api_client = GithubApiClient::new();
        let checks_failed = 3;
        let format_checks_failed = 2;
        let tidy_checks_failed = 1;
        let tmp_dir = tempdir().unwrap();
        let mut tmp_file = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        env::set_var("GITHUB_OUTPUT", tmp_file.path());
        assert_eq!(
            checks_failed,
            rest_api_client.set_exit_code(
                checks_failed,
                Some(format_checks_failed),
                Some(tidy_checks_failed)
            )
        );
        let mut output_file_content = String::new();
        tmp_file.read_to_string(&mut output_file_content).unwrap();
        assert!(output_file_content.contains(
            format!(
                "checks-failed={}\nformat-checks-failed={}\ntidy-checks-failed={}\n",
                3, 2, 1
            )
            .as_str()
        ));
        println!("temp file used: {:?}", tmp_file.path());
        drop(tmp_file);
        drop(tmp_dir);
    }

    // ************************* tests for comment output

    #[test]
    fn check_comment_concerns() {
        let tmp_dir = tempdir().unwrap();
        let mut tmp_file = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        let rest_api_client = GithubApiClient::new();
        let mut files = vec![FileObj::new(PathBuf::from("tests/demo/demo.cpp"))];
        capture_clang_tools_output(
            &mut files,
            env::var("CLANG-VERSION").unwrap_or("".to_string()).as_str(),
            "readability-*",
            "file",
            &LinesChangedOnly::Off,
            None,
            None,
        );
        let format_checks_failed = tally_format_advice(&files);
        let tidy_checks_failed = tally_tidy_advice(&files);
        let comment =
            rest_api_client.make_comment(&files, format_checks_failed, tidy_checks_failed, None);
        assert!(format_checks_failed > 0);
        assert!(tidy_checks_failed > 0);
        env::set_var("GITHUB_STEP_SUMMARY", tmp_file.path());
        rest_api_client.post_step_summary(&comment);
        let mut output_file_content = String::new();
        tmp_file.read_to_string(&mut output_file_content).unwrap();
        assert_eq!(format!("\n{comment}\n\n"), output_file_content);
    }

    #[test]
    fn check_comment_lgtm() {
        let tmp_dir = tempdir().unwrap();
        let mut tmp_file = NamedTempFile::new_in(tmp_dir.path()).unwrap();
        let rest_api_client = GithubApiClient::new();
        let mut files = vec![FileObj::new(PathBuf::from("tests/demo/demo.cpp"))];
        capture_clang_tools_output(
            &mut files,
            env::var("CLANG-VERSION").unwrap_or("".to_string()).as_str(),
            "-*",
            "",
            &LinesChangedOnly::Off,
            None,
            None,
        );
        let format_checks_failed = tally_format_advice(&files);
        let tidy_checks_failed = tally_tidy_advice(&files);
        let comment =
            rest_api_client.make_comment(&files, format_checks_failed, tidy_checks_failed, None);
        assert_eq!(format_checks_failed, 0);
        assert_eq!(tidy_checks_failed, 0);
        env::set_var("GITHUB_STEP_SUMMARY", tmp_file.path());
        rest_api_client.post_step_summary(&comment);
        let mut output_file_content = String::new();
        tmp_file.read_to_string(&mut output_file_content).unwrap();
        assert_eq!(format!("\n{comment}\n\n"), output_file_content);
    }
}
