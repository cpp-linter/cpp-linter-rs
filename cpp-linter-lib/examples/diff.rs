use cpp_linter_lib::{
    common_fs::FileFilter,
    git::{get_diff, open_repo, parse_diff},
};
use std::error::Error;

/// An example to show the file names of the diff for either
///
/// - only last commit
/// - only staged files
pub fn main() -> Result<(), Box<dyn Error>> {
    let repo = open_repo(".")?;
    let diff = get_diff(&repo);

    let extensions = vec!["cpp".to_string(), "hpp".to_string(), "rs".to_string()];
    let file_filter = FileFilter::new(&["target", ".github"], extensions);
    let files = parse_diff(&diff, &file_filter);

    for file in &files {
        println!("{}", file.name.to_string_lossy());
        println!("lines with additions: {:?}", file.added_lines);
        println!("ranges of added lines: {:?}", file.added_ranges);
        println!("ranges of diff hunks: {:?}", file.diff_chunks);
    }
    println!("found {} files in diff", files.len());
    Ok(())
}
