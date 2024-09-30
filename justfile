set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]


# run the test suite
[group("code coverage")]
test profile='default':
    cargo llvm-cov --no-report \
    nextest --manifest-path cpp-linter/Cargo.toml \
    --lib --tests --color always --profile {{ profile }}

# Clear previous test build artifacts
[group("code coverage")]
test-clean:
    cargo llvm-cov clean

# generate and open pretty coverage report
[group("code coverage")]
pretty-cov *args='':
    cargo llvm-cov report --json --output-path coverage.json --ignore-filename-regex main
    llvm-cov-pretty coverage.json {{ args }}

# generate and open detailed coverage report
[group("code coverage")]
llvm-cov *args='':
    cargo llvm-cov report --html --ignore-filename-regex main {{ args }}

# This is useful for IDE gutter indicators of line coverage.
# See Coverage Gutters ext in VSCode.
# generate lcov.info
[group("code coverage")]
lcov:
    cargo llvm-cov report --lcov --output-path lcov.info --ignore-filename-regex main

# serve docs
[group("docs")]
docs open='':
    mkdocs serve --config-file docs/mkdocs.yml {{ open }}

# build docs
[group("docs")]
docs-build:
    mkdocs build --config-file docs/mkdocs.yml

# rust docs
[group("docs")]
docs-rs open='':
    cargo doc --no-deps --lib --manifest-path cpp-linter/Cargo.toml {{ open }}

# run cpp-linter native binary
[group("bin")]
run *args:
    cargo run --bin cpp-linter --manifest-path cpp-linter/Cargo.toml -- {{ args }}

# build the native binary
[group("bin")]
build *args='':
    cargo build --bin cpp-linter --manifest-path cpp-linter/Cargo.toml {{ args }}

# run clippy and rustfmt
lint:
    cargo clippy --allow-staged --allow-dirty --fix
    cargo fmt

# bump version in root Cargo.toml
bump component='patch':
    @python .github/workflows/bump_version.py {{ component }}
