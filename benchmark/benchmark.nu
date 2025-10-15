# Run benchmarks for cpp-linter.
#
# This script assumes it is being run from the directory in which it resides (benchmark/).
# It also assumes that the cpp-linter repository is located in the parent directory (../).
# The libgit2 repository will be cloned into the benchmark/ directory if it does not exist.
# The benchmark results will be saved to benchmark/benchmark.json.
#
# To use this script locally, run:
#   nu benchmark/benchmark.nu
# To use this script in CI, see .github/workflows/benchmark.yml.

use "../nurfile" run-cmd

# Build release version of cpp-linter (rust) binary
#
# Returns the path to the built binary.
export def build-release [] {
    run-cmd cargo build --release --bin cpp-linter
    let install_path = glob "../target/release/cpp-linter*"
    print $install_path
    (
        $install_path
        | where {
            let ext = ($in | path parse) | get extension
            ($ext | is-empty) or ($ext == "exe")
        }
        | first
        | str replace --all "\\" "/"
    )
}

# Find the cpp-linter binary installed in the virtual environment
#
# Assumes the venv is located in current directory.
#
# Returns the path to the binary.
def find-bin-from-venv [] {
    let list_env_bin = glob ".venv/*/cpp-linter*"
    print $list_env_bin
    let install_path = (
        $list_env_bin
        | where {($in | path basename) | str starts-with "cpp-linter"}
        | first
        | str replace --all "\\" "/"
    )
    $install_path
}

# Install the new version (v2.x) of cpp-linter with python bindings in a virtual environment
#
# Returns the path to the installed binary.
export def install-py-binding [
    wheel_path?: string, # The path to a pre-built wheel. If not provided, the wheel will be built from source.
] {
    print "Installing new cpp-linter version (2.x) via python binding..."
    cd ..
    if not (".venv" | path exists) {
        run-cmd uv venv
    }
    let path_to_dist = if ($wheel_path | is-not-empty) {
        $wheel_path
    } else {
        "./"
    }
    run-cmd uv pip install $path_to_dist
    let install_path = find-bin-from-venv
    cd benchmark
    $install_path
}

# Install the old version (v1.x) of cpp-linter in a virtual environment
#
# Returns the path to the installed binary.
export def install-old [] {
    print "Installing old cpp-linter version (1.x)..."
    run-cmd uv venv --clear
    run-cmd uv pip install -r requirements-old.txt
    let install_path = find-bin-from-venv
    $install_path
}

# Clone the libgit2 repository and check out the specified `tag`.
#
# If the libgit2 repository already exists, it will not be cloned again.
#
# If the libgit2/build directory does not exist, it will be created via CMake.
export def checkout-libgit2 [
    tag: string = "v1.8.1", # The libgit2 tag to checkout
] {
    if not ("libgit2" | path exists) {
        run-cmd git clone https://github.com/libgit2/libgit2
        cd libgit2
        run-cmd git checkout $tag
        cd ..
    }
    if not ("libgit2/build" | path exists) {
        run-cmd cmake ...[
            -B libgit2/build
            -S libgit2
            -D CMAKE_EXPORT_COMPILE_COMMANDS=ON
            -D BUILD_TESTS=OFF
            -D BUILD_CLI=OFF
        ]
    }
}

# Run benchmarks using hyperfine.
#
# Assumes this is executed in the same directory as this script (benchmark/).
#
# Saves results to benchmark.json
export def run-hyperfine [
    old_path: string, # path to old version of cpp-linter (v1.x)
    new_path: string, # path to new version of cpp-linter (v2.x)
    rust_bin: string, # path to rust binary of cpp-linter (v2.x)
    prev_rust_bin?: string, # path to previous commit's release build of rust binary (v2.x)
] {
    mut hyperfine_args = [
        --shell none
        --output pipe
        --export-json ../benchmark.json
        --export-markdown ../benchmark.md
        --warmup 1
        --style color
        --runs 3
    ]
    let common_args = "-l 0 -a 0 -i=|!src/libgit2 -p build -e c"
    if ($old_path | is-not-empty) {
        $hyperfine_args = $hyperfine_args | append [--command-name python-pure $"($old_path) ($common_args) -j 0"]
    }
    if ($new_path | is-not-empty) {
        $hyperfine_args = $hyperfine_args | append [--command-name python-rust $"($new_path) ($common_args)"]
    }
    if ($rust_bin | is-not-empty) {
        $hyperfine_args = $hyperfine_args | append [--command-name rust $"($rust_bin) ($common_args)"]
    }
    if ($prev_rust_bin | is-not-empty) {
        $hyperfine_args = $hyperfine_args | append [--command-name rust-previous $"($prev_rust_bin) ($common_args)"]
    }
    cd libgit2
    run-cmd hyperfine ...$hyperfine_args
    cd ..
}

export def summarize [] {
    let data = open benchmark.json
    let results = $data | get results | reject "times" "exit_codes"
    let summary = open benchmark.md
    let summary_file = if ($env | get --optional GITHUB_STEP_SUMMARY | is-not-empty) {
        $"\n# Results\n\n($summary)" | save --append $env.GITHUB_STEP_SUMMARY
    } else {
        $"# Results\n\n($summary)" | save --force benchmark.md
    }
    print $results
}


# Run benchmark.
#
# Assumes this is executed in the same directory as this script (benchmark/).
#
# 1. Builds the Rust binary of cpp-linter.
# 2. Installs the old version (v1.x) of cpp-linter in a virtual environment.
# 3. Installs the new version (v2.x) of cpp-linter in a virtual environment.
# 4. Clones the libgit2 repository if it does not exist.
# 5. Runs benchmarks using hyperfine and saves the results to benchmark.json.
export def main [
    --new-py: string, # path to wheel of new version of cpp-linter (v2.x)
    --rust-bin: string, # path to rust binary of cpp-linter (v2.x)
    --prev-rust-bin: string, # path to previous commit's release build of rust binary (v2.x)
] {
    let is_on_win = sys host | get name | str starts-with "Windows"
    let rust_bin = if ($rust_bin | is-not-empty) {
        if not $is_on_win {
            run-cmd chmod +x $rust_bin
        }
        $rust_bin
    } else {
        build-release
    }
    if not $is_on_win and ($prev_rust_bin | is-not-empty) {
        run-cmd chmod +x $prev_rust_bin
    }
    let old_path = install-old
    let new_path = install-py-binding $new_py
    checkout-libgit2
    run-hyperfine $old_path $new_path $rust_bin $prev_rust_bin
    summarize
}
