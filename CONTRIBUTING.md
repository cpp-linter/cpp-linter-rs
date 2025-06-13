# Contributing

This project requires the following tools installed:

- :simple-rust: [rust](https://rustup.rs/)
- :simple-uv: [`uv` (Python Project management tool)](https://docs.astral.sh/uv/)

## Getting started

After checking out the repo locally, use

```sh
uv sync
```

This creates a venv at ".venv/" in repo root (if it doesn't exist).
It also installs dev dependencies like `pre-commit`, `nox`, `ruff`, and `mypy`.

See [`uv sync` docs](https://docs.astral.sh/uv/reference/cli/#uv-sync)
for more detailed usage.

> [!TIP]
> To register the pre-commit hooks, use:
>
> ```shell
> uv run pre-commit install
> ```

## Running tests

First ensure that the following cargo-managed binaries are installed:

- [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov)
- [`cargo-nextest`](https://nexte.st/docs/installation/pre-built-binaries/)
- [`llvm-cov-pretty`](https://crates.io/crates/llvm-cov-pretty) is used only for the optional `nox -s pretty-cov` task. Without this installed, `nox -s llvm-cov` will suffice.

Use nox to run tests:

```sh
uv run nox -s test
```

Different test profiles are still defined in .config/nextest.toml.
The above command uses the "default" profile, but to mimic the CI, use:

```sh
uv run nox -s test -- --profile ci
```

To generate a coverage report:

```sh
uv run nox -s llvm-cov -- --open
```

The `-- --open` part is optional. It opens the built coverage report in your default browser.

The uploaded codecov report is generated with

```sh
uv run nox -s lcov
```

## Generating docs

To view the docs locally, use

```sh
uv run nox -s docs -- --open
```

The `-- --open` part is optional. It opens the built docs in your default browser.

In CI, docs are built using

```sh
uv run nox -s docs-build
```
