mod common;
use chrono::Utc;
use common::{create_test_space, mock_server};
use mockito::Matcher;
use tempfile::{NamedTempFile, TempDir};

use cpp_linter::{
    common_fs::FileFilter,
    logger,
    rest_api::{github::GithubApiClient, RestApiClient},
};
use std::{env, io::Write, path::Path};

#[derive(PartialEq)]
enum EventType {
    Push,
    PullRequest(u64),
}

const REPO: &str = "cpp-linter/test-cpp-linter-action";
const SHA: &str = "DEADBEEF";
const TOKEN: &str = "123456";
const RESET_RATE_LIMIT_HEADER: &str = "x-ratelimit-reset";
const REMAINING_RATE_LIMIT_HEADER: &str = "x-ratelimit-remaining";
const MALFORMED_RESPONSE_PAYLOAD: &str = "{\"message\":\"Resource not accessible by integration\"}";

async fn get_paginated_changes(lib_root: &Path, event_type: EventType, fail_serialization: bool) {
    env::set_var("GITHUB_REPOSITORY", REPO);
    env::set_var("GITHUB_SHA", SHA);
    env::set_var("GITHUB_TOKEN", TOKEN);
    env::set_var("CI", "true");
    env::set_var(
        "GITHUB_EVENT_NAME",
        if event_type == EventType::Push {
            "push"
        } else {
            "pull_request"
        },
    );
    let tmp = TempDir::new().expect("Failed to create a temp dir for test");
    let mut event_payload = NamedTempFile::new_in(tmp.path())
        .expect("Failed to spawn a tmp file for test event payload");
    env::set_var("GITHUB_EVENT_PATH", event_payload.path());
    if let EventType::PullRequest(pr_number) = event_type {
        event_payload
            .write_all(
                serde_json::json!({"number": pr_number})
                    .to_string()
                    .as_bytes(),
            )
            .expect("Failed to write data to test event payload file")
    }

    let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
    let asset_path = format!("{}/tests/paginated_changes", lib_root.to_str().unwrap());

    let mut server = mock_server().await;
    env::set_var("GITHUB_API_URL", server.url());
    env::set_current_dir(tmp.path()).unwrap();
    let gh_client = GithubApiClient::new().unwrap();
    logger::init().unwrap();
    log::set_max_level(log::LevelFilter::Debug);

    let mut mocks = vec![];
    let diff_end_point = format!(
        "/repos/{REPO}/{}",
        if let EventType::PullRequest(pr) = event_type {
            format!("pulls/{pr}")
        } else {
            format!("commits/{SHA}")
        }
    );
    mocks.push(
        server
            .mock("GET", diff_end_point.as_str())
            .match_header("Accept", "application/vnd.github.diff")
            .match_header("Authorization", format!("token {TOKEN}").as_str())
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .with_status(403)
            .create(),
    );
    let pg_end_point = if event_type == EventType::Push {
        diff_end_point.clone()
    } else {
        format!("{diff_end_point}/files")
    };
    let pg_count = if fail_serialization { 1 } else { 2 };
    for pg in 1..=pg_count {
        let link = if pg == 1 {
            format!("<{}{pg_end_point}?page=2>; rel=\"next\"", server.url())
        } else {
            "".to_string()
        };
        let mut mock = server
            .mock("GET", pg_end_point.as_str())
            .match_header("Accept", "application/vnd.github.raw+json")
            .match_header("Authorization", format!("token {TOKEN}").as_str())
            .match_query(Matcher::UrlEncoded("page".to_string(), pg.to_string()))
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .with_header("link", link.as_str());
        if fail_serialization {
            mock = mock.with_body(MALFORMED_RESPONSE_PAYLOAD);
        } else {
            mock = mock.with_body_from_file(format!(
                "{asset_path}/{}_files_pg{pg}.json",
                if event_type == EventType::Push {
                    "push"
                } else {
                    "pull_request"
                }
            ));
        }
        mocks.push(mock.create());
    }

    let file_filter = FileFilter::new(&[], vec!["cpp".to_string(), "hpp".to_string()]);
    let files = gh_client.get_list_of_changed_files(&file_filter).await;
    if let Ok(files) = files {
        // if !fail_serialization
        assert_eq!(files.len(), 2);
        for file in files {
            assert!(["src/demo.cpp", "src/demo.hpp"].contains(
                &file
                    .name
                    .as_path()
                    .to_str()
                    .expect("Failed to get file name from path")
            ));
        }
    }
    for mock in mocks {
        mock.assert();
    }
}

async fn test_get_changes(event_type: EventType, fail_serialization: bool) {
    let tmp_dir = create_test_space(false);
    let lib_root = env::current_dir().unwrap();
    env::set_current_dir(tmp_dir.path()).unwrap();
    get_paginated_changes(&lib_root, event_type, fail_serialization).await;
    env::set_current_dir(lib_root.as_path()).unwrap();
    drop(tmp_dir);
}

#[tokio::test]
async fn get_push_files_paginated() {
    test_get_changes(EventType::Push, false).await
}

#[tokio::test]
async fn get_pr_files_paginated() {
    test_get_changes(EventType::PullRequest(42), false).await
}

#[tokio::test]
async fn fail_push_files_paginated() {
    test_get_changes(EventType::Push, true).await
}

#[tokio::test]
async fn fail_pr_files_paginated() {
    test_get_changes(EventType::PullRequest(42), true).await
}
