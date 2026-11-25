# Contributing

This project requires the following tools installed:

- :simple-rust: [rust](https://rustup.rs/)
- :simple-uv: [`uv` (Python Project management tool)](https://docs.astral.sh/uv/)
- :simple-nushell: [nur] for running tasks via [nushell](https://www.nushell.sh/)
- :simple-yarn: [`yarn` (Node.js package management tool)](https://yarnpkg.com/)

## Getting started

After checking out the repo locally, use

```sh
uv sync
```

This creates a venv at ".venv/" in repo root (if it doesn't exist).
It also installs dev dependencies like `pre-commit`, `ruff`, and `mypy`.

See [`uv sync` docs](https://docs.astral.sh/uv/reference/cli/#uv-sync)
for more detailed usage.

> [!TIP]
> To register the pre-commit hooks, use:
>
> ```shell
> uv run pre-commit install
> ```

You can quickly run pre-commit hooks using our [nur] task as well:

```sh
nur pre-commit
```

See `nur pre-commit -h` for more detail.

## Running tests

First ensure that the following cargo-managed binaries are installed:

- [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov)
- [`cargo-nextest`](https://nexte.st/docs/installation/pre-built-binaries/)

Use [nur] to run tests:

```sh
nur test
```

Different test profiles are still defined in .config/nextest.toml.
The above command uses the "default" profile, but to mimic the CI, use:

```sh
nur test --profile ci
```

To generate a coverage report:

```sh
nur test html --open
```

The `--open` flag is optional. It opens the built coverage report in your
default browser.

The uploaded codecov report is generated with

```sh
nur test lcov
```

## Generating docs

To view the docs locally, use our [nur] task:

```sh
nur docs --open
```

The `--open` flag is optional. It opens the built docs in your default browser.

In CI, docs are built using

```sh
nur docs --build
```

## Releasing

Releases are prepared by [release-plz] (see `release-plz.toml`).

1. On every push to `main`, release-plz opens or updates a release PR titled
   "chore: release". It bumps the versions from the conventional commit
   messages (`fix` → patch, `feat` → minor, `!` → major) and adds the
   changelog entries.
2. cpp-linter, cpp-linter-py and cpp-linter-js always get the same version.
   clang-tools-manager has its own version.
3. The release PR is a draft. Mark it ready for review to run the release
   builds (binaries, Python wheels, Node.js packages) on it, then merge it.
4. release-plz then publishes cpp-linter and clang-tools-manager to
   crates.io, pushes a `{package}/v{version}` tag for each released package
   and creates its GitHub release. The tags trigger the builds that upload
   the binaries and publish to PyPI and npm. Pre-releases (`-rc.N`) go to
   TestPyPI and to npm's `next` tag.

If a release stops halfway, release-plz does not retry it: it skips a package
whose tag already exists or whose version is already on crates.io. Create the
missing GitHub release by hand, which also creates the tag if it is missing:

```sh
# add --prerelease for -rc.N versions
gh release create <tag> --target <commit of the merged release PR>
```

A new tag starts the tag-triggered builds; if the tag already existed,
re-run its failed jobs instead.

To choose a version yourself (e.g. to start or end release candidates),
open a PR that sets it (needs [cargo-edit]):

```sh
cargo set-version -p cpp-linter -p cpp-linter-py -p cpp-linter-js 2.0.0
.github/sync-node-version.sh
```

After it is merged, the release PR keeps that version.

[nur]: https://nur-taskrunner.github.io/docs/
[release-plz]: https://release-plz.dev/
[cargo-edit]: https://github.com/killercup/cargo-edit
