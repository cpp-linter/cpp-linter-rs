use chrono::Utc;
use cpp_linter::{
    cli::LinesChangedOnly,
    rest_api::{COMMENT_MARKER, USER_OUTREACH},
    run::run_main,
};
use mockito::Matcher;
use serde_json::json;
use std::{env, io::Write, path::Path};
use tempfile::NamedTempFile;

mod common;
use common::{create_test_space, mock_server};

const SHA: &str = "8d68756375e0483c7ac2b4d6bbbece420dbbb495";
const REPO: &str = "cpp-linter/test-cpp-linter-action";
const PR: i64 = 27;
const TOKEN: &str = "123456";
const MOCK_ASSETS_PATH: &str = "tests/reviews_test_assets/";
const EVENT_PAYLOAD: &str = "{\"number\": 27}";

const RESET_RATE_LIMIT_HEADER: &str = "x-ratelimit-reset";
const REMAINING_RATE_LIMIT_HEADER: &str = "x-ratelimit-remaining";

struct TestParams {
    pub lines_changed_only: LinesChangedOnly,
    pub tidy_review: bool,
    pub format_review: bool,
    pub passive_reviews: bool,
    pub no_lgtm: bool,
    pub force_lgtm: bool,
    pub summary_only: bool,
    pub pr_state: String,
    pub pr_draft: bool,
    pub fail_dismissal: bool,
    pub fail_get_existing_reviews: bool,
    pub fail_posting: bool,
    pub bad_pr_info: bool,
    pub bad_existing_reviews: bool,
}

impl Default for TestParams {
    fn default() -> Self {
        Self {
            lines_changed_only: LinesChangedOnly::On,
            tidy_review: true,
            format_review: true,
            passive_reviews: false,
            no_lgtm: true,
            force_lgtm: false,
            summary_only: false,
            pr_state: "open".to_string(),
            pr_draft: false,
            fail_dismissal: false,
            fail_get_existing_reviews: false,
            fail_posting: false,
            bad_pr_info: false,
            bad_existing_reviews: false,
        }
    }
}

fn generate_tool_summary(review_enabled: bool, force_lgtm: bool, tool_name: &str) -> String {
    if !review_enabled {
        return String::new();
    }
    if force_lgtm {
        format!("No concerns reported by {}. Great job! :tada:", tool_name)
    } else {
        format!("Click here for the full {} patch", tool_name)
    }
}

async fn setup(lib_root: &Path, test_params: &TestParams) {
    env::remove_var("GITHUB_OUTPUT"); // avoid writing to GH_OUT in parallel-running tests
    env::set_var("GITHUB_EVENT_NAME", "pull_request");
    env::set_var("GITHUB_REPOSITORY", REPO);
    env::set_var("GITHUB_SHA", SHA);
    env::set_var("GITHUB_TOKEN", TOKEN);
    env::set_var("CI", "true");
    if test_params.summary_only {
        env::set_var("CPP_LINTER_PR_REVIEW_SUMMARY_ONLY", "true");
    }
    let mut event_payload_path = NamedTempFile::new_in("./").unwrap();
    event_payload_path
        .write_all(EVENT_PAYLOAD.as_bytes())
        .expect("Failed to create mock event payload.");
    env::set_var("GITHUB_EVENT_PATH", event_payload_path.path());
    let clang_version = env::var("CLANG_VERSION").unwrap_or("".to_string());
    let reset_timestamp = (Utc::now().timestamp() + 60).to_string();
    let asset_path = format!("{}/{MOCK_ASSETS_PATH}", lib_root.to_str().unwrap());

    let mut server = mock_server().await;
    env::set_var("GITHUB_API_URL", server.url());
    let mut mocks = vec![];

    let pr_endpoint = format!("/repos/{REPO}/pulls/{PR}");
    mocks.push(
        server
            .mock("GET", pr_endpoint.as_str())
            .match_header("Accept", "application/vnd.github.diff")
            .match_header("Authorization", format!("token {TOKEN}").as_str())
            .with_body_from_file(format!("{asset_path}pr_{PR}.diff"))
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .create(),
    );
    mocks.push(
        server
            .mock("GET", pr_endpoint.as_str())
            .match_header("Accept", "application/vnd.github.raw+json")
            .match_header("Authorization", format!("token {TOKEN}").as_str())
            .with_body(if test_params.bad_pr_info {
                String::new()
            } else {
                json!({"state": test_params.pr_state, "draft": test_params.pr_draft}).to_string()
            })
            .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
            .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
            .create(),
    );

    let reviews_endpoint = format!("/repos/{REPO}/pulls/{PR}/reviews");

    let mut mock = server
        .mock("GET", reviews_endpoint.as_str())
        .match_header("Accept", "application/vnd.github.raw+json")
        .match_header("Authorization", format!("token {TOKEN}").as_str())
        .match_body(Matcher::Any)
        .match_query(Matcher::UrlEncoded("page".to_string(), "1".to_string()))
        .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
        .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
        .with_status(if test_params.fail_get_existing_reviews {
            403
        } else {
            200
        });
    if test_params.bad_existing_reviews {
        mock = mock.with_body(String::new()).create();
    } else {
        mock = mock
            .with_body_from_file(format!("{asset_path}pr_reviews.json"))
            .create()
    }
    mocks.push(mock);
    if !test_params.fail_get_existing_reviews && !test_params.bad_existing_reviews {
        mocks.push(
            server
                .mock(
                    "PUT",
                    format!("{reviews_endpoint}/1807607546/dismissals").as_str(),
                )
                .match_body(r#"{"event":"DISMISS","message":"outdated suggestion"}"#)
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .with_status(if test_params.fail_dismissal { 403 } else { 200 })
                .create(),
        );
    }

    let lgtm_allowed = !test_params.force_lgtm || !test_params.no_lgtm;
    if lgtm_allowed
        && !test_params.pr_draft
        && test_params.pr_state == "open"
        && !test_params.bad_pr_info
    {
        let review_reaction = if test_params.passive_reviews {
            "COMMENT"
        } else if test_params.force_lgtm {
            "APPROVE"
        } else {
            "REQUEST_CHANGES"
        };
        let tidy_summary = generate_tool_summary(
            test_params.tidy_review,
            test_params.force_lgtm,
            "clang-tidy",
        );
        let format_summary = generate_tool_summary(
            test_params.format_review,
            test_params.force_lgtm,
            "clang-format",
        );
        let review_summary = format!(
            "{}## Cpp-linter Review.*{format_summary}.*{tidy_summary}.*{}",
            regex::escape(format!("{}", COMMENT_MARKER.escape_default()).as_str()),
            regex::escape(format!("{}", USER_OUTREACH.escape_default()).as_str()),
        );
        let expected_review_payload = format!(
            "\\{{\"event\":\"{review_reaction}\",\"body\":\"{review_summary}\",\"comments\":\\[{}\\]}}",
            if test_params.force_lgtm || test_params.summary_only {
                ""
            } else {
                ".+"
            }
        );
        mocks.push(
            server
                .mock("POST", reviews_endpoint.as_str())
                .match_body(Matcher::Regex(expected_review_payload))
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_header(REMAINING_RATE_LIMIT_HEADER, "50")
                .with_header(RESET_RATE_LIMIT_HEADER, reset_timestamp.as_str())
                .with_status(if test_params.fail_posting { 403 } else { 200 })
                .create(),
        );
    }

    let mut tool_ignore = "**/*.c".to_string();
    if test_params.force_lgtm {
        tool_ignore.push_str("|**/*.cpp|**/*.h");
    }
    let mut args = vec![
        "cpp-linter".to_string(),
        "-v=debug".to_string(),
        format!("-V={}", clang_version),
        format!("-l={}", test_params.lines_changed_only),
        format!("--ignore-tidy={}", tool_ignore),
        format!("--ignore-format={}", tool_ignore),
        format!("--tidy-review={}", test_params.tidy_review),
        format!("--format-review={}", test_params.format_review),
        format!("--passive-reviews={}", test_params.passive_reviews),
        format!("--no-lgtm={}", test_params.no_lgtm),
        "-p=build".to_string(),
        "-i=build".to_string(),
    ];
    if test_params.force_lgtm {
        if test_params.tidy_review {
            // only use a check that doesn't trigger concern on test assets
            args.push("--tidy-checks=-*,bugprone-infinite-loop".to_string());
        }
        if test_params.format_review {
            // explicitly disable formatting using `DisableFormat: true`
            args.push("--style={DisableFormat: true}".to_string());
        }
    } else {
        args.push("--style=file".to_string()); // use .clang-format file
    }
    let result = run_main(args).await;
    assert!(result.is_ok());
    for mock in mocks {
        mock.assert();
    }
}

async fn test_review(test_params: &TestParams) {
    let tmp_dir = create_test_space(true);
    let lib_root = env::current_dir().unwrap();
    env::set_current_dir(tmp_dir.path()).unwrap();
    setup(&lib_root, test_params).await;
    env::set_current_dir(lib_root.as_path()).unwrap();
    drop(tmp_dir);
}

#[tokio::test]
async fn all_lines() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn all_lines_tidy_only() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        format_review: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn all_lines_format_only() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        tidy_review: false,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn changed_lines() {
    test_review(&TestParams::default()).await;
}

#[tokio::test]
async fn all_lines_passive() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        passive_reviews: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn changed_lines_passive() {
    test_review(&TestParams {
        passive_reviews: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn summary_only() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        summary_only: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn lgtm() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        no_lgtm: false,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn lgtm_passive() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        no_lgtm: false,
        force_lgtm: true,
        passive_reviews: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn no_lgtm() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        force_lgtm: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn is_draft() {
    test_review(&TestParams {
        pr_draft: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn is_closed() {
    test_review(&TestParams {
        pr_state: "closed".to_string(),
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_posting() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        fail_posting: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_dismissal() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        fail_dismissal: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn fail_get_existing_reviews() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        fail_get_existing_reviews: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn bad_existing_reviews() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        force_lgtm: true,
        bad_existing_reviews: true,
        ..Default::default()
    })
    .await;
}

#[tokio::test]
async fn bad_pr_info() {
    test_review(&TestParams {
        lines_changed_only: LinesChangedOnly::Off,
        force_lgtm: true,
        bad_pr_info: true,
        ..Default::default()
    })
    .await;
}
