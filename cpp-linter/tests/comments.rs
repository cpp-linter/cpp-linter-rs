use chrono::Utc;
use cpp_linter::cli::{LinesChangedOnly, ThreadComments};
use cpp_linter::run::run_main;
use mockito::Matcher;
use std::{env, fmt::Display, io::Write, path::Path};
use tempfile::NamedTempFile;

mod common;
use common::{create_test_space, mock_server};

const SHA: &str = "8d68756375e0483c7ac2b4d6bbbece420dbbb495";
const REPO: &str = "cpp-linter/test-cpp-linter-action";
const PR: i64 = 22;
const TOKEN: &str = "123456";
const MOCK_ASSETS_PATH: &str = "tests/comment_test_assets/";
const EVENT_PAYLOAD: &str = "{\"number\": 22}";

const RESET_RATE_LIMIT_HEADER: &str = "x-ratelimit-reset";
const REMAINING_RATE_LIMIT_HEADER: &str = "x-ratelimit-remaining";

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

struct TestParams {
    pub event_t: EventType,
    pub lines_changed_only: LinesChangedOnly,
    pub thread_comments: ThreadComments,
    pub no_lgtm: bool,
    pub force_lgtm: bool,
    pub fail_get_existing_comments: bool,
    pub fail_dismissal: bool,
    pub fail_posting: bool,
    pub bad_existing_comments: bool,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            event_t: EventType::Push,
            lines_changed_only: LinesChangedOnly::On,
            thread_comments: ThreadComments::On,
            no_lgtm: true,
            force_lgtm: false,
            fail_get_existing_comments: false,
            fail_dismissal: false,
            fail_posting: false,
            bad_existing_comments: false,
        }
    }
}

async fn setup(lib_root: &Path, test_params: &TestParams) {
    env::set_var(
        "GITHUB_EVENT_NAME",
        test_params.event_t.to_string().as_str(),
    );
    env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
    env::set_var("GITHUB_REPOSITORY", REPO);
    env::set_var("GITHUB_SHA", SHA);
    env::set_var("GITHUB_TOKEN", TOKEN);
    env::set_var("CI", "true");
    let mut event_payload_path = NamedTempFile::new_in("./").unwrap();
    if test_params.event_t == EventType::PullRequest {
        event_payload_path
            .write_all(EVENT_PAYLOAD.as_bytes())
            .expect("Failed to create mock event payload.");
        env::set_var("GITHUB_EVENT_PATH", event_payload_path.path());
    }

    let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
    let asset_path = format!("{}/{MOCK_ASSETS_PATH}", lib_root.to_str().unwrap());

    let mut server = mock_server().await;
    env::set_var("GITHUB_API_URL", server.url());
    let mut mocks = vec![];

    if test_params.lines_changed_only != LinesChangedOnly::Off {
        let diff_end_point = if test_params.event_t == EventType::PullRequest {
            format!("pulls/{PR}")
        } else {
            format!("commits/{SHA}")
        };
        mocks.push(
            server
                .mock("GET", format!("/repos/{REPO}/{diff_end_point}").as_str())
                .match_header("Accept", "application/vnd.github.diff")
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_body_from_file(format!("{asset_path}patch.diff"))
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .create(),
        );
    }
    if test_params.event_t == EventType::Push {
        let mut mock = server
            .mock(
                "GET",
                format!("/repos/{REPO}/commits/{SHA}/comments").as_str(),
            )
            .match_header("Accept", "application/vnd.github.raw+json")
            .match_header("Authorization", format!("token {TOKEN}").as_str())
            .match_body(Matcher::Any)
            .match_query(Matcher::UrlEncoded("page".to_string(), "1".to_string()))
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .with_status(if test_params.fail_get_existing_comments {
                403
            } else {
                200
            });
        if test_params.bad_existing_comments {
            mock = mock.with_body(String::new());
        } else {
            mock = mock.with_body_from_file(format!("{asset_path}push_comments_{SHA}.json"));
        }
        mock = mock.create();
        mocks.push(mock);
    } else {
        let pr_endpoint = format!("/repos/{REPO}/issues/{PR}/comments");
        for pg in ["1", "2"] {
            let link = if pg == "1" {
                format!("<{}{pr_endpoint}?page=2>; rel=\"next\"", server.url())
            } else {
                "".to_string()
            };
            mocks.push(
                server
                    .mock("GET", pr_endpoint.as_str())
                    .match_header("Accept", "application/vnd.github.raw+json")
                    .match_header("Authorization", format!("token {TOKEN}").as_str())
                    .match_body(Matcher::Any)
                    .match_query(Matcher::UrlEncoded("page".to_string(), pg.to_string()))
                    .with_body_from_file(format!("{asset_path}pr_comments_pg{pg}.json"))
                    .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                    .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                    .with_header("link", link.as_str())
                    .with_status(if test_params.fail_dismissal { 403 } else { 200 })
                    .create(),
            );
        }
    }
    let comment_url = format!(
        "/repos/{REPO}{}/comments/76453652",
        if test_params.event_t == EventType::PullRequest {
            "/issues"
        } else {
            ""
        }
    );

    if !test_params.fail_get_existing_comments && !test_params.bad_existing_comments {
        mocks.push(
            server
                .mock("DELETE", comment_url.as_str())
                .match_body(Matcher::Any)
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_status(if test_params.fail_dismissal { 403 } else { 200 })
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .expect_at_least(1)
                .create(),
        );
    }

    let new_comment_match = Matcher::Regex(format!(
        "# Cpp-Linter Report :{}:",
        if test_params.force_lgtm {
            "heavy_check_mark"
        } else {
            "warning"
        }
    ));

    let lgtm_allowed = !test_params.force_lgtm || !test_params.no_lgtm;

    if test_params.thread_comments == ThreadComments::Update && lgtm_allowed {
        mocks.push(
            server
                .mock("PATCH", comment_url.as_str())
                .match_body(new_comment_match.clone())
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_status(if test_params.fail_posting { 403 } else { 200 })
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .create(),
        );
    }

    if test_params.thread_comments == ThreadComments::On
        && lgtm_allowed
        && !test_params.bad_existing_comments
    {
        mocks.push(
            server
                .mock(
                    "POST",
                    format!(
                        "/repos/{REPO}/{}/comments",
                        if test_params.event_t == EventType::PullRequest {
                            format!("issues/{PR}")
                        } else {
                            format!("commits/{SHA}")
                        }
                    )
                    .as_str(),
                )
                .match_body(new_comment_match)
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .with_status(if test_params.fail_posting { 403 } else { 200 })
                .create(),
        );
    }

    let mut args = vec![
        "cpp-linter".to_string(),
        "-v=debug".to_string(),
        format!("-V={}", env::var("CLANG_VERSION").unwrap_or("".to_string())),
        format!("-l={}", test_params.lines_changed_only),
        "--ignore-tidy=src/some source.c".to_string(),
        "--ignore-format=src/some source.c".to_string(),
        format!("--thread-comments={}", test_params.thread_comments),
        format!("--no-lgtm={}", test_params.no_lgtm),
        "-p=build".to_string(),
        "-i=build".to_string(),
    ];
    if test_params.force_lgtm {
        args.push("-e=c".to_string());
    }
    let result = run_main(args).await;
    assert!(result.is_ok());
    for mock in mocks {
        mock.assert();
    }
}

async fn test_comment(test_params: &TestParams) {
    let tmp_dir = create_test_space(true);
    let lib_root = env::current_dir().unwrap();
    env::set_current_dir(tmp_dir.path()).unwrap();
    setup(&lib_root, test_params).await;
    env::set_current_dir(lib_root.as_path()).unwrap();
    drop(tmp_dir);
}

#[tokio::test]
async fn new_push_all_lines() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn new_push_changed_lines() {
    test_comment(&TestParams {
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn new_pr_all_lines() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        lines_changed_only: LinesChangedOnly::Off,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn new_pr_changed_lines() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_push_all_lines() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        thread_comments: ThreadComments::Update,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_push_changed_lines() {
    test_comment(&TestParams {
        thread_comments: ThreadComments::Update,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_pr_all_lines() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        lines_changed_only: LinesChangedOnly::Off,
        thread_comments: ThreadComments::Update,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_pr_changed_lines() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        thread_comments: ThreadComments::Update,
        no_lgtm: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn new_push_no_lgtm() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_push_no_lgtm() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        thread_comments: ThreadComments::Update,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn new_pr_no_lgtm() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        lines_changed_only: LinesChangedOnly::Off,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn update_pr_no_lgtm() {
    test_comment(&TestParams {
        event_t: EventType::PullRequest,
        lines_changed_only: LinesChangedOnly::Off,
        thread_comments: ThreadComments::Update,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_get_existing_comments() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Diff,
        fail_get_existing_comments: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_dismissal() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Diff,
        fail_dismissal: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_posting() {
    test_comment(&TestParams {
        lines_changed_only: LinesChangedOnly::Diff,
        fail_posting: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn bad_existing_comments() {
    test_comment(&TestParams {
        bad_existing_comments: true,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}
