
# cpp-linter

[![crates.io][crates-io-badge]][crates-io-link]
[![docs.rs][docs-badge]][docs-link]
[![CHANGELOG][changelog-badge]][changelog-link]

This crate contains the the library used as a backend for the
`cpp-linter` binary executable. The main focus of `cpp-linter` is as follows:

- [x] Lint C/C++ sources using clang-format and clang-tidy.
- [x] Respect file changes when run in a CI workflow on Github.
- [x] Provide feedback via Github's REST API in the any of the following forms:
  - [x] step summary
  - [x] thread comments
  - [x] file annotation
  - [x] pull request review suggestions

See also the [CLI document hosted on github][gh-pages].

[gh-pages]: https://cpp-linter.github.io/cpp-linter-rs/cli.html
[crates-io-badge]: https://img.shields.io/crates/v/cpp-linter
[crates-io-link]: https://crates.io/crates/cpp-linter
[docs-badge]: https://img.shields.io/docsrs/cpp-linter
[docs-link]: https://docs.rs/cpp-linter
[changelog-badge]: https://img.shields.io/badge/keep_a_change_log-v1.1.0-ffec3d
[changelog-link]: https://github.com/cpp-linter/cpp-linter-rs/blob/main/cpp-linter/CHANGELOG.md
