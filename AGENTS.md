# Agent Guide

## Project Layout

- `cpp-linter/` contains the main Rust library, CLI, and integration tests.
- `clang-tools-manager/` manages clang-format and clang-tidy installations.
- `bindings/python/` and `bindings/node/` contain the Python and Node.js bindings.
- `docs/` contains the MkDocs site and Rust documentation support.
- The workspace is defined in the root `Cargo.toml`.

Keep changes scoped to the owning package and update binding or documentation surfaces when a public API change requires it. Follow the relevant package's existing conventions.

## Setup

Install Rust, `uv`, Nushell and `nur`, and Yarn. For the Python tooling environment, run:

```sh
uv sync
```

Install the Rust test tools `cargo-nextest` and `cargo-llvm-cov` if they are not already available. Register pre-commit hooks with `uv run pre-commit install` when desired.

## Testing

**Do not run `cargo test`.** Tests use environment variables and can collide when Cargo runs them in parallel without isolating each test's environment. Use nextest through the repository task instead:

```sh
nur test
```

This runs the Rust library and integration tests in `cpp-linter` and `clang-tools-manager` through `cargo llvm-cov ... nextest`. Available profiles are defined in `.config/nextest.toml`:

- `nur test` uses `default`, which excludes selected slower tests.
- `nur test --profile ci` runs the clang-tool-focused subset used to keep normal CI runs short.
- `nur test --profile all` includes the full test suite except the `eval_version` warmup test.
- `nur test --clean` clears the coverage data before testing.

The CI workflow first runs `warmup` to populate the clang-tools cache. It then runs `all` for the minimum clang version and `ci` for other clang versions found in the static-binaries index, on both Windows and Ubuntu. When changing Rust tests, use nextest profiles and environment configuration; do not replace nextest with `cargo test`.

Node binding checks use:

```sh
yarn lint
yarn test
```

These run Oxlint and AVA from `bindings/node/`. CI also builds/tests the bindings across multiple operating systems, architectures, and Node versions. Python packaging CI builds wheels and an sdist; use the Python tooling and package-specific checks relevant to the change.

## Lint And Formatting

Run the CI-equivalent Rust checks with:

```sh
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --check
```

Or run the repository task in check mode:

```sh
nur lint --check
```

`nur lint` without `--check` applies Clippy fixes and formats files. Pre-commit hooks include whitespace/end-of-file/YAML/TOML checks, Ruff lint/format, and cspell; run them with `nur pre-commit` (or `nur pre-commit --all` for all files).

## Documentation And Build Notes

- `nur docs --build` builds the MkDocs site; `nur docs` serves it locally. The task installs or refreshes the `cli-gen` package unless `--dirty` can reuse an existing install.
- `nur docs rs` builds Rust API docs.
- CI pre-seeds `clang-tools-manager/static-binaries-index.json` so build scripts and docs builds do not need to fetch the index themselves. If a local build fails because this index is missing, `nur seed versions` fetches it. Treat the generated index as build support data, not a routine source change.
- Use the root `package.json` scripts for Node work; use `uv` and the root `pyproject.toml` for Python development tooling.

## Repository Checks

Before finishing, run the focused tests for the changed package and relevant lint/format checks. For Rust tests, the required runner is nextest as described above. Keep generated build artifacts, coverage output, and local environment files out of source changes unless the task specifically requires them.
