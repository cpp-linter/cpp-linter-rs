use chrono::Utc;
use cpp_linter_lib::run::run_main;
use mockito::{Matcher, Server, ServerGuard};
use std::{env, fmt::Display, io::Write, path::Path};
use tempfile::NamedTempFile;

mod common;
use common::create_test_space;

const SHA: &str = "8d68756375e0483c7ac2b4d6bbbece420dbbb495";
const REPO: &str = "cpp-linter/test-cpp-linter-action";
const PR: i64 = 22;
const TOKEN: &str = "123456";
const MOCK_ASSETS_PATH: &str = "tests/comment_test_assets/";
const EVENT_PAYLOAD: &str = "{\"number\": 22}";

const RESET_RATE_LIMIT_HEADER: &str = "x-ratelimit-reset";
const REMAINING_RATE_LIMIT_HEADER: &str = "x-ratelimit-remaining";

async fn mock_server() -> ServerGuard {
    Server::new_async().await
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum EventType {
    Push,
    PullRequest,
}

impl Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Push => write!(f, "push"),
            Self::PullRequest => write!(f, "pull_request"),
        }
    }
}

async fn setup(
    event_t: EventType,
    lib_root: &Path,
    lines_changed_only: &str,
    thread_comments: &str,
    no_lgtm: bool,
) {
    env::set_var("GITHUB_EVENT_NAME", event_t.to_string().as_str());
    env::set_var("GITHUB_REPOSITORY", REPO);
    env::set_var("GITHUB_SHA", SHA);
    env::set_var("GITHUB_TOKEN", TOKEN);
    env::set_var("CI", "true");
    let mut event_payload_path = NamedTempFile::new().unwrap();
    if event_t == EventType::PullRequest {
        event_payload_path
            .write_all(EVENT_PAYLOAD.as_bytes())
            .expect("Failed to create mock event payload.");
        env::set_var("GITHUB_EVENT_PATH", event_payload_path.path());
    }
    let diff_end_point = if event_t == EventType::PullRequest {
        format!("pulls/{PR}")
    } else {
        format!("commits/{SHA}")
    };
    let diff_file = if event_t == EventType::PullRequest {
        format!("pr_{PR}")
    } else {
        format!("push_{SHA}")
    };
    let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
    let is_lgtm =
        no_lgtm || (("false", "update", false) == (lines_changed_only, thread_comments, no_lgtm));

    let asset_path = format!("{}/{MOCK_ASSETS_PATH}", lib_root.to_str().unwrap());
    let mut server = mock_server().await;
    env::set_var("GITHUB_API_URL", server.url());
    server
        .mock("GET", format!("/repos/{REPO}/{diff_end_point}").as_str())
        .match_header("Accept", "application/vnd.github.diff")
        .match_header("Authorization", TOKEN)
        .with_body_from_file(format!("{asset_path}{diff_file}.diff"))
        .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
        .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
        .create();
    if event_t == EventType::Push {
        server
            .mock(
                "GET",
                format!("/repos/{REPO}/commits/{SHA}/comments/").as_str(),
            )
            .match_header("Accept", "application/vnd.github.raw+json")
            .match_header("Authorization", TOKEN)
            .match_body(Matcher::Any)
            .match_query(Matcher::UrlEncoded("page".to_string(), "1".to_string()))
            .with_body_from_file(format!("{asset_path}push_comments_{SHA}.json"))
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .create();
    } else {
        let pr_endpoint = format!("/repos/{REPO}/issues/{PR}/comments/");
        for pg in ["1", "2"] {
            let link = if pg == "1" {
                format!("<{}{pr_endpoint}?page=2>; rel=\"next\"", server.url())
            } else {
                "".to_string()
            };
            server
                .mock("GET", pr_endpoint.as_str())
                .match_header("Accept", "application/vnd.github.raw+json")
                .match_header("Authorization", TOKEN)
                .match_body(Matcher::Any)
                .match_query(Matcher::UrlEncoded("page".to_string(), pg.to_string()))
                .with_body_from_file(format!("{asset_path}pr_comments_pg{pg}.json"))
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .with_header("link", link.as_str())
                .create();
        }
    }
    let comment_url = format!("/repos/{REPO}/comments/76453652");
    for method in ["DELETE", "PATCH"] {
        server
            .mock(method, comment_url.as_str())
            .match_body(Matcher::Regex(format!(
                "# Cpp-Linter Report :{}:",
                if is_lgtm {
                    "heavy_check_mark"
                } else {
                    "warning"
                }
            )))
            .create();
    }
    for method in ["PATCH", "POST"] {
        server
            .mock(
                method,
                format!(
                    "/repos/{REPO}/{}/comments/",
                    if event_t == EventType::PullRequest {
                        format!("issues/{PR}")
                    } else {
                        format!("commits/{SHA}")
                    }
                )
                .as_str(),
            )
            .match_query(Matcher::Any)
            .match_body(Matcher::Any)
            .create();
    }

    let mut args = vec![
        "cpp-linter".to_string(),
        "-v=debug".to_string(),
        format!("-V={}", env::var("CLANG_VERSION").unwrap_or("".to_string())),
        format!("-l={lines_changed_only}"),
        "--ignore-tidy=src/some source.c".to_string(),
        "--ignore-format=src/some source.c".to_string(),
        format!("--thread-comments={thread_comments}"),
        format!("--no-lgtm={no_lgtm}"),
        "-p=build".to_string(),
        "-i=build".to_string(),
    ];
    if is_lgtm {
        args.push("-e=c".to_string());
    }
    let result = run_main(args).await;
    assert_eq!(result, 0);
}

async fn test_comment(
    event_t: EventType,
    lines_changed_only: &str,
    thread_comments: &str,
    no_lgtm: bool,
) {
    let tmp_dir = create_test_space();
    let lib_root = env::current_dir().unwrap();
    env::set_current_dir(tmp_dir.path()).unwrap();
    setup(
        event_t,
        &lib_root,
        lines_changed_only,
        thread_comments,
        no_lgtm,
    )
    .await;
    env::set_current_dir(lib_root.as_path()).unwrap();
    drop(tmp_dir);
}

#[tokio::test]
async fn new_push_all_lines() {
    test_comment(
        EventType::Push, // event_t
        "false",         // lines_changed_only
        "true",          // thread_comments
        false,           // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn new_push_changed_lines() {
    test_comment(
        EventType::Push, // event_t
        "true",          // lines_changed_only
        "true",          // thread_comments
        false,           // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn new_pr_all_lines() {
    test_comment(
        EventType::PullRequest, // event_t
        "false",                // lines_changed_only
        "true",                 // thread_comments
        false,                  // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn new_pr_changed_lines() {
    test_comment(
        EventType::PullRequest, // event_t
        "true",                 // lines_changed_only
        "true",                 // thread_comments
        false,                  // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_push_all_lines() {
    test_comment(
        EventType::Push, // event_t
        "false",         // lines_changed_only
        "update",        // thread_comments
        false,           // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_push_changed_lines() {
    test_comment(
        EventType::Push, // event_t
        "true",          // lines_changed_only
        "update",        // thread_comments
        false,           // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_pr_all_lines() {
    test_comment(
        EventType::PullRequest, // event_t
        "false",                // lines_changed_only
        "update",               // thread_comments
        false,                  // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_pr_changed_lines() {
    test_comment(
        EventType::PullRequest, // event_t
        "true",                 // lines_changed_only
        "update",               // thread_comments
        false,                  // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn new_push_no_lgtm() {
    test_comment(
        EventType::Push, // event_t
        "false",         // lines_changed_only
        "true",          // thread_comments
        true,            // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_push_no_lgtm() {
    test_comment(
        EventType::Push, // event_t
        "false",         // lines_changed_only
        "update",        // thread_comments
        true,            // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn new_pr_no_lgtm() {
    test_comment(
        EventType::PullRequest, // event_t
        "false",                // lines_changed_only
        "true",                 // thread_comments
        true,                   // no_lgtm
    )
    .await;
}

#[tokio::test]
async fn update_pr_no_lgtm() {
    test_comment(
        EventType::PullRequest, // event_t
        "false",                // lines_changed_only
        "update",               // thread_comments
        true,                   // no_lgtm
    )
    .await;
}
