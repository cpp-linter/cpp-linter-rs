<!-- markdownlint-disable MD041 -->

[file-annotations]: https://cpp-linter.github.io/cpp-linter-rs/cli#-a-file-annotations
[thread-comments]: https://cpp-linter.github.io/cpp-linter-rs/cli#-g-thread-comments
[step-summary]: https://cpp-linter.github.io/cpp-linter-rs/cli#-w-step-summary
[tidy-review]: https://cpp-linter.github.io/cpp-linter-rs/cli#-d-tidy-review
[format-review]: https://cpp-linter.github.io/cpp-linter-rs/cli#-m-format-review
[other-licenses]: https://cpp-linter.github.io/cpp-linter-rs/other-licenses

[format-annotations-preview]: docs/docs/images/annotations-clang-format.png
[tidy-annotations-preview]: docs/docs/images/annotations-clang-tidy.png
[step-summary-preview]: docs/docs/images/step-summary.png
[thread-comment-preview]: docs/docs/images/comment.png
[tidy-review-preview]: docs/docs/images/tidy-review.png
[format-review-preview]: docs/docs/images/format-review.png
[format-suggestion-preview]: docs/docs/images/format-suggestion.png

[cli-doc]: https://cpp-linter.github.io/cpp-linter-rs/cli

<!-- start -->
[![Python packaging][py-build-badge]][py-build-ci]
[![Binary executable builds][bin-build-badge]][bin-build-ci]
[![node-js builds][node-ci-badge]][node-ci]
[![Test CI][test-ci-badge]][test-ci]
[![Docs][docs-ci-badge]][docs-site]
[![Pre-commit-ci][pre-commit-badge]][pre-commit-ci]
[![codecov-status][codecov-badge]][codecov-project]

[py-build-ci]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/python-packaging.yml
[py-build-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/python-packaging.yml/badge.svg
[bin-build-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/binary-builds.yml/badge.svg
[bin-build-ci]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/binary-builds.yml
[node-ci-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/node-js-packaging.yml/badge.svg
[node-ci]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/node-js-packaging.yml
[test-ci-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/run-dev-tests.yml/badge.svg
[test-ci]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/run-dev-tests.yml
[docs-ci-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/build-docs.yml/badge.svg
[docs-site]: https://cpp-linter.github.io/cpp-linter-rs
[pre-commit-badge]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/pre-commit-hooks.yml/badge.svg
[pre-commit-ci]: https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/pre-commit-hooks.yml
[codecov-badge]: https://codecov.io/gh/cpp-linter/cpp-linter-rs/graph/badge.svg?token=7ibzERx2AD
[codecov-project]: https://codecov.io/gh/cpp-linter/cpp-linter-rs
[docs-rs-badge]: https://img.shields.io/docsrs/cpp-linter?label=docs.rs
[docs-rs]: https://docs.rs/cpp-linter
[pypi-badge]: https://img.shields.io/pypi/v/cpp-linter
[pypi-pkg]: https://pypi.org/project/cpp-linter/
[test-pypi-badge]: https://img.shields.io/pypi/v/cpp-linter?pypiBaseUrl=https%3A%2F%2Ftest.pypi.org&label=test-pypi
[test-pypi-pkg]: https://test.pypi.org/project/cpp-linter/
[crates-io-badge]: https://img.shields.io/crates/v/cpp-linter
[crates-io-pkg]: https://crates.io/crates/cpp-linter
[npm-badge]: https://img.shields.io/npm/v/%40cpp-linter%2Fcpp-linter
[npm-pkg]: https://www.npmjs.com/package/@cpp-linter/cpp-linter

# C/C++ Linting Package

A package for linting C/C++ code with clang-tidy and/or clang-format to collect feedback provided in the form of

- [x] [thread-comments](#thread-comment)
- [x] [step-summary](#step-summary)
- [x] [file-annotations](#annotations)
- [x] [Pull Request Review](#pull-request-review) suggestions

> [!WARNING]
> This project is still experimental and subject to drastic changes.
> Please use the [pure python cpp-linter](https://github.com/cpp-linter/cpp-linter)
> package until this project is ready for deployment.

## Install

This package is available in several programming languages (through their respective package managers).

### Rust

[![Crates.io Version][crates-io-badge]][crates-io-pkg]
[![docs.rs][docs-rs-badge]][docs-rs]

Install from source code hosted at crates.io:

```text
cargo install cpp-linter
```

Install a pre-compiled binary from GitHub releases:

First [install `cargo-binstall`](https://github.com/cargo-bins/cargo-binstall?tab=readme-ov-file#installation).

```text
cargo binstall cpp-linter
```

### Python

[![PyPI - Version][pypi-badge]][pypi-pkg]

Install the python package:

```text
pip install cpp-linter
```

[![testPyPI - Version][test-pypi-badge]][test-pypi-pkg]

Pre-releases are uploaded to test-pypi:

```text
pip install -i https://test.pypi.org/simple/ cpp-linter
```

### Node.js

[![NPM Version][npm-badge]][npm-pkg]

Install the Node.js binding:

```text
npm -g install @cpp-linter/cpp-linter
```

## Usage

For usage in a CI workflow, see
[the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see
[the CLI documentation][cli-doc].

## Example

### Annotations

Using [`--file-annotations`][file-annotations]:

#### clang-format annotations

![clang-format annotations][format-annotations-preview]

#### clang-tidy annotations

![clang-tidy annotations][tidy-annotations-preview]

### Thread Comment

Using [`--thread-comments`][thread-comments]:

![sample thread-comment][thread-comment-preview]

### Step Summary

Using [`--step-summary`][step-summary]:

![step summary][step-summary-preview]

### Pull Request Review

#### Only clang-tidy

Using [`--tidy-review`][tidy-review]:

![sample tidy-review][tidy-review-preview]

#### Only clang-format

Using [`--format-review`][format-review]:

![sample format-review][format-review-preview]

![sample format-suggestion][format-suggestion-preview]

## Have question or feedback?

To provide feedback (requesting a feature or reporting a bug) please post to
[issues](https://github.com/cpp-linter/cpp-linter-rs/issues).

## License

The scripts and documentation in this project are released under the [MIT] license.

As for dependencies (that are redistributed by us in binary form) and their licenses, refer to [THIRD-PARTY LICENSES][other-licenses].

[MIT]: https://choosealicense.com/licenses/mit
