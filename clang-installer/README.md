# clang-installer

A Rust crate to ensure clang-format and/or clang-tidy are installed.

This is a utility for cpp-linter CLI,
thus its API is considered internal to cpp-linter crate.

To run the binary for this crate (from git source):

```text
cargo run --bin clang-tools --features bin -- --help
```
