mod common;
use chrono::Utc;
use common::{create_test_space, mock_server};
use mockito::Matcher;
use tempfile::{NamedTempFile, TempDir};

use cpp_linter::{
    cli::LinesChangedOnly,
    common_fs::FileFilter,
    logger,
    rest_api::{github::GithubApiClient, RestApiClient},
};
use std::{env, io::Write, path::Path};

#[derive(PartialEq, Default)]
enum EventType {
    #[default]
    Push,
    PullRequest,
}

#[derive(Default)]
struct TestParams {
    event_t: EventType,
    fail_serde_diff: bool,
    fail_serde_event_payload: bool,
    no_event_payload: bool,
}

const REPO: &str = "cpp-linter/test-cpp-linter-action";
const SHA: &str = "DEADBEEF";
const PR: u8 = 42;
const TOKEN: &str = "123456";
const EVENT_PAYLOAD: &str = r#"{"number": 42}"#;
const RESET_RATE_LIMIT_HEADER: &str = "x-ratelimit-reset";
const REMAINING_RATE_LIMIT_HEADER: &str = "x-ratelimit-remaining";
const MALFORMED_RESPONSE_PAYLOAD: &str = "{\"message\":\"Resource not accessible by integration\"}";

async fn get_paginated_changes(lib_root: &Path, test_params: &TestParams) {
    env::set_var("GITHUB_REPOSITORY", REPO);
    env::set_var("GITHUB_SHA", SHA);
    env::set_var("GITHUB_TOKEN", TOKEN);
    env::set_var("CI", "true");
    env::set_var(
        "GITHUB_EVENT_NAME",
        if test_params.event_t == EventType::Push {
            "push"
        } else {
            "pull_request"
        },
    );
    let tmp = TempDir::new().expect("Failed to create a temp dir for test");
    let mut event_payload = NamedTempFile::new_in(tmp.path())
        .expect("Failed to spawn a tmp file for test event payload");
    env::set_var(
        "GITHUB_EVENT_PATH",
        if test_params.no_event_payload {
            Path::new("no a file.txt")
        } else {
            event_payload.path()
        },
    );
    if EventType::PullRequest == test_params.event_t
        && !test_params.fail_serde_event_payload
        && !test_params.no_event_payload
    {
        event_payload
            .write_all(EVENT_PAYLOAD.as_bytes())
            .expect("Failed to write data to test event payload file")
    }

    let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
    let asset_path = format!("{}/tests/paginated_changes", lib_root.to_str().unwrap());

    let mut server = mock_server().await;
    env::set_var("GITHUB_API_URL", server.url());
    env::set_current_dir(tmp.path()).unwrap();
    logger::try_init();
    log::set_max_level(log::LevelFilter::Debug);
    let gh_client = GithubApiClient::new();
    if test_params.fail_serde_event_payload || test_params.no_event_payload {
        assert!(gh_client.is_err());
        return;
    }
    let client = gh_client.unwrap();

    let mut mocks = vec![];
    let diff_end_point = format!(
        "/repos/{REPO}/{}",
        if EventType::PullRequest == test_params.event_t {
            format!("pulls/{PR}")
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
    let pg_end_point = if test_params.event_t == EventType::Push {
        diff_end_point.clone()
    } else {
        format!("{diff_end_point}/files")
    };
    let pg_count = if test_params.fail_serde_diff { 1 } else { 2 };
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
        if test_params.fail_serde_diff {
            mock = mock.with_body(MALFORMED_RESPONSE_PAYLOAD);
        } else {
            mock = mock.with_body_from_file(format!(
                "{asset_path}/{}_files_pg{pg}.json",
                if test_params.event_t == EventType::Push {
                    "push"
                } else {
                    "pull_request"
                }
            ));
        }
        mocks.push(mock.create());
    }

    let file_filter = FileFilter::new(&[], vec!["cpp".to_string(), "hpp".to_string()]);
    let files = client
        .get_list_of_changed_files(&file_filter, &LinesChangedOnly::Off)
        .await;
    match files {
        Err(e) => {
            if !test_params.fail_serde_diff {
                panic!("Failed to get changed files: {e:?}");
            }
        }
        Ok(files) => {
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
    }
    for mock in mocks {
        mock.assert();
    }
}

async fn test_get_changes(test_params: &TestParams) {
    let tmp_dir = create_test_space(false);
    let lib_root = env::current_dir().unwrap();
    env::set_current_dir(tmp_dir.path()).unwrap();
    get_paginated_changes(&lib_root, test_params).await;
    env::set_current_dir(lib_root.as_path()).unwrap();
    drop(tmp_dir);
}

#[tokio::test]
async fn get_push_files_paginated() {
    test_get_changes(&TestParams::default()).await
}

#[tokio::test]
async fn get_pr_files_paginated() {
    test_get_changes(&TestParams {
        event_t: EventType::PullRequest,
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn fail_push_files_paginated() {
    test_get_changes(&TestParams {
        fail_serde_diff: true,
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn fail_pr_files_paginated() {
    test_get_changes(&TestParams {
        event_t: EventType::PullRequest,
        fail_serde_diff: true,
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn fail_event_payload() {
    test_get_changes(&TestParams {
        event_t: EventType::PullRequest,
        fail_serde_event_payload: true,
        ..Default::default()
    })
    .await
}

#[tokio::test]
async fn no_event_payload() {
    test_get_changes(&TestParams {
        event_t: EventType::PullRequest,
        no_event_payload: true,
        ..Default::default()
    })
    .await
}
