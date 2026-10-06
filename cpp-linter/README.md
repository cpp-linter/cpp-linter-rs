
# cpp-linter

This crate contains the library used as a backend for the
`cpp-linter` binary executable. The main focus of `cpp-linter` is as follows:

- [x] Lint C/C++ sources using clang-format and clang-tidy.
- [x] Respect file changes when run in a CI workflow on GitHub.
- [x] Provide feedback via GitHub's REST API in any of the following forms:
  - [x] step summary
  - [x] thread comments
  - [x] file annotation
  - [x] pull request review suggestions

[Website](https://cpp-linter.github.io/) |
[Documentation](https://cpp-linter.github.io/cpp-linter-rs/) |
[Get started](https://cpp-linter.github.io/getting-started/#locally-or-in-other-ci) |
[Discussions](https://github.com/orgs/cpp-linter/discussions)

See also the [CLI document hosted on GitHub][cli-doc].

[cli-doc]: https://cpp-linter.github.io/cpp-linter-rs/cli/
