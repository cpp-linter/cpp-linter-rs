use std::env;
use std::error::Error;

use cpp_linter_lib::common_fs::FileFilter;
use cpp_linter_lib::github_api::GithubApiClient;

// needed to use trait implementations (ie `get_list_of_changed_files()`)
use cpp_linter_lib::rest_api::RestApiClient;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("GITHUB_SHA", "950ff0b690e1903797c303c5fc8d9f3b52f1d3c5");
    env::set_var("GITHUB_REPOSITORY", "cpp-linter/cpp-linter");
    let client_controller = GithubApiClient::new();

    let file_filter = FileFilter::new(
        &["target", ".github"],
        vec!["cpp".to_string(), "hpp".to_string()],
    );

    env::set_var("CI", "true"); // needed for get_list_of_changed_files() to use REST API
    let files = client_controller
        .get_list_of_changed_files(&file_filter)
        .await;

    for file in &files {
        println!("{}", file.name.to_string_lossy());
        println!("lines with additions: {:?}", file.added_lines);
        println!("ranges of added lines: {:?}", file.added_ranges);
        println!("ranges of diff hunks: {:?}", file.diff_chunks);
    }
    println!("found {} files in diff", files.len());
    Ok(())
}
