
# cpp-linter

This crate contains the the library used as a backend for the
`cpp-linter` binary executable. The main focus of `cpp-linter` is as follows:

- [x] Lint C/C++ sources using clang-format and clang-tidy.
- [x] Respect file changes when run in a CI workflow on Github.
- [x] Provide feedback via Github's REST API in the any of the following forms:
  - [x] step summary
  - [x] thread comments
  - [x] file annotation
  - [x] pull request review suggestions

Since the [cpp-linter python package][pypi-org] now uses this library
as a binding, the native binary's `main()` behavior is also present in this
library (see [`run::run_main()`](fn@crate::run::run_main)).

See also the [CLI document hosted on github][gh-pages].

[pypi-org]: https://pypi.org/project/cpp-linter
[gh-pages]: https://cpp-linter.github.io/cpp-linter-rs/cli.html
