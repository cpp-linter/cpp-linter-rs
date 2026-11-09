<!-- markdownlint-disable MD041 -->

[file-annotations]: https://cpp-linter.github.io/cpp-linter-rs/cli#-a-file-annotations
[thread-comments]: https://cpp-linter.github.io/cpp-linter-rs/cli#-t-thread-comments
[step-summary]: https://cpp-linter.github.io/cpp-linter-rs/cli#-w-step-summary
[pr-review]: https://cpp-linter.github.io/cpp-linter-rs/cli#-p-pr-review
[other-licenses]: https://cpp-linter.github.io/cpp-linter-rs/other-licenses

[annotations-diff-light]: docs/docs/images/annotations-diff-light.png#gh-light-mode-only
[annotations-diff-dark]: docs/docs/images/annotations-diff-dark.png#gh-dark-mode-only
[annotations-summary-light]: docs/docs/images/annotations-summary-light.png#gh-light-mode-only
[annotations-summary-dark]: docs/docs/images/annotations-summary-dark.png#gh-dark-mode-only
[thread-comment-light]: docs/docs/images/thread-comment-light.png#gh-light-mode-only
[thread-comment-dark]: docs/docs/images/thread-comment-dark.png#gh-dark-mode-only
[pr-review-light]: docs/docs/images/pr-review-light.png#gh-light-mode-only
[pr-review-dark]: docs/docs/images/pr-review-dark.png#gh-dark-mode-only
[step-summary-preview]: docs/docs/images/step-summary.png

[cli-doc]: https://cpp-linter.github.io/cpp-linter-rs/cli

<!-- start -->
# cpp-linter-rs

[![v2 release candidate](https://img.shields.io/badge/v2-release%20candidate-975a16?labelColor=454a63)](https://github.com/cpp-linter/cpp-linter-rs/releases)
[![ci](https://img.shields.io/github/actions/workflow/status/cpp-linter/cpp-linter-rs/run-dev-tests.yml?branch=main&label=ci&labelColor=454a63)](https://github.com/cpp-linter/cpp-linter-rs/actions/workflows/run-dev-tests.yml)
[![coverage](https://img.shields.io/codecov/c/github/cpp-linter/cpp-linter-rs?labelColor=454a63)](https://codecov.io/gh/cpp-linter/cpp-linter-rs)
[![part of cpp-linter](https://img.shields.io/badge/part%20of-cpp--linter-ffc20a?labelColor=454a63)](https://cpp-linter.github.io/)

A package for linting C/C++ code with clang-tidy and/or clang-format to collect
feedback provided in the form of [thread comments](#thread-comment), a
[step summary](#step-summary), [file annotations](#annotations) and
[pull request review](#pull-request-review) suggestions.

[Website](https://cpp-linter.github.io/) |
[Documentation](https://cpp-linter.github.io/cpp-linter-rs/) |
[Get started](https://cpp-linter.github.io/getting-started/#locally-or-in-other-ci) |
[Discussions](https://github.com/orgs/cpp-linter/discussions)

> [!WARNING]
> This project (cpp-linter v2) is in release candidates.
> Please use the [pure python cpp-linter](https://github.com/cpp-linter/cpp-linter)
> package until 2.0 is released.

## Quick start

This package is available in several programming languages (through their
respective package managers).

### Rust

[![Crates.io Version][crates-io-badge]][crates-io-pkg]

Until 2.0 is released, crates.io only has release candidates,
so `--version` is required.

Install from source code hosted at crates.io:

```text
cargo install cpp-linter --version 2.0.0-rc.23 --features bin --locked
```

Install a pre-compiled binary from GitHub releases:

First [install `cargo-binstall`](https://github.com/cargo-bins/cargo-binstall?tab=readme-ov-file#installation).

```text
cargo binstall cpp-linter --version 2.0.0-rc.23
```

### Python

[![testPyPI - Version][test-pypi-badge]][test-pypi-pkg]

Pre-releases are uploaded to test-pypi:

```text
pip install --pre -i https://test.pypi.org/simple/ cpp-linter
```

Until 2.0 is released, `pip install cpp-linter` installs the
[pure python cpp-linter](https://github.com/cpp-linter/cpp-linter)
package from PyPI.

### Node.js

[![NPM Version][npm-badge]][npm-pkg]

Install the Node.js binding:

```text
npm -g install @cpp-linter/cpp-linter@next
```

## Usage

For usage in a CI workflow, see
[the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see
[the CLI documentation][cli-doc].

## Example

### Annotations

Using [`--file-annotations`][file-annotations]:

#### In the pull request's changed files

![clang-tidy annotation][annotations-diff-light]
![clang-tidy annotation][annotations-diff-dark]

#### In the workflow run summary

![annotations in the workflow run summary][annotations-summary-light]
![annotations in the workflow run summary][annotations-summary-dark]

### Thread Comment

Using [`--thread-comments`][thread-comments]:

![sample thread-comment][thread-comment-light]
![sample thread-comment][thread-comment-dark]

### Step Summary

Using [`--step-summary`][step-summary]:

![step summary][step-summary-preview]

### Pull Request Review

Using [`--pr-review`][pr-review]:

![sample pr-review][pr-review-light]
![sample pr-review][pr-review-dark]

## Sponsors

cpp-linter is maintained by two volunteers. [Sponsor the project](https://cpp-linter.github.io/sponsor/)
through [Open Collective](https://opencollective.com/cpp-linter). [Silver and Gold sponsors](https://cpp-linter.github.io/sponsor/#sponsor-tiers)
get their logo here.

## Contributing

To provide feedback (requesting a feature or reporting a bug) please post to
[issues](https://github.com/cpp-linter/cpp-linter-rs/issues).
For development setup, see [CONTRIBUTING.md](https://github.com/cpp-linter/cpp-linter-rs/blob/main/CONTRIBUTING.md).

## License

The scripts and documentation in this project are released under the [MIT] license.

As for dependencies (that are redistributed by us in binary form) and their
licenses, refer to [THIRD-PARTY LICENSES][other-licenses].

[MIT]: https://github.com/cpp-linter/cpp-linter-rs/blob/main/LICENSE
[crates-io-badge]: https://img.shields.io/crates/v/cpp-linter?labelColor=454a63&color=975a16
[crates-io-pkg]: https://crates.io/crates/cpp-linter
[test-pypi-badge]: https://img.shields.io/pypi/v/cpp-linter?pypiBaseUrl=https%3A%2F%2Ftest.pypi.org&label=test-pypi&labelColor=454a63&color=975a16
[test-pypi-pkg]: https://test.pypi.org/project/cpp-linter/
[npm-badge]: https://img.shields.io/npm/v/%40cpp-linter%2Fcpp-linter/next?labelColor=454a63&color=975a16
[npm-pkg]: https://www.npmjs.com/package/@cpp-linter/cpp-linter
