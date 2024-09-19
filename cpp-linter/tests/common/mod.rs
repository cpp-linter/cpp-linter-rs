//! This module contains common function(s) for running integration tests (tests/*.rs files).

use std::{
    fs,
    io::{self, BufRead},
    process::Command,
};

use mockito::{Server, ServerGuard};
use tempfile::TempDir;

/// Create a temporary folder to run tests.
///
/// The temporary folder will contain assets used to run the tests.
/// The source files used for these tests are copied from tests/demo.
///
/// The meson build system generator and the ninja build system are
/// third-party dependencies of these tests. They are used to generate
/// a compilation database for clang-tidy (and cpp-linter) to utilize.
///
/// The returned directory object will automatically delete the
/// temporary folder when it is dropped out of scope.
pub fn create_test_space(setup_meson: bool) -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("src")).unwrap();
    let src = fs::read_dir("tests/demo").unwrap();
    for file in src {
        let file = file.unwrap();
        if file.path().is_file() {
            let new_file = tmp.path().join("src").join(file.file_name());
            fs::copy(file.path(), new_file.to_str().unwrap()).unwrap();
        }
    }

    if !setup_meson {
        return tmp;
    }

    // generate compilation database with meson (& ninja)
    let test_dir = tmp.path().join("src");
    let mut cmd = Command::new("meson");
    cmd.args([
        "init",
        "-C",
        test_dir.to_str().unwrap(),
        "--name",
        "demo",
        "demo.cpp",
        "demo.hpp",
    ]);
    let output = cmd.output().expect("Failed to run 'meson init'");
    println!(
        "meson init stdout:\n{}",
        String::from_utf8(output.stdout.to_vec()).unwrap()
    );
    let meson_build_dir = tmp.path().join("build");
    let mut cmd = Command::new("meson");
    cmd.args([
        "setup",
        "--backend=ninja",
        meson_build_dir.as_path().to_str().unwrap(),
        test_dir.to_str().unwrap(),
    ]);
    let output = cmd
        .output()
        .expect("Failed to generate build assets with 'meson setup'");
    println!(
        "meson setup stdout:\n{}",
        String::from_utf8(output.stdout.to_vec()).unwrap()
    );
    let db = fs::File::open(meson_build_dir.join("compile_commands.json"))
        .expect("Failed to open compilation database");
    for line in io::BufReader::new(db).lines().map_while(Result::ok) {
        println!("{line}");
    }
    tmp
}

/// Creates a mock server for use in tests.
///
/// It is the test's responsibility to create the mock responses.
pub async fn mock_server() -> ServerGuard {
    Server::new_async().await
}
