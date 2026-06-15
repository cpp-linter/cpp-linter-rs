# clang-tools-manager

[![crates.io][crates-io-badge]][crates-io-link]
[![CHANGELOG][changelog-badge]][changelog-link]

A Rust crate to ensure clang-format and/or clang-tidy are installed.

This is a utility for [cpp-linter] CLI,
thus its API is considered internal to [cpp-linter] crate.

## Binary executable

This crate comes with a `clang-tools` executable binary.
It can be installed using

```sh
cargo install clang-tools-manager --locked --features bin
```

Or using [cargo-binstall] to download pre-built binary executable:

```sh
cargo binstall clang-tools-manager
```

To run the executable from a checked out `git clone`:

```text
cargo run --bin clang-tools --features bin -- --help
```

[cpp-linter]: https://crates.io/crates/cpp-linter
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall

### CLI

The `clang-tools` binary's Command Line Interface (CLI) is rather simple.

<!-- markdownlint-disable MD033 -->

<details><summary><code>clang-tools --help</code></summary>
<p>

```sh
Usage: clang-tools [OPTIONS]

Options:
  -v, --version [<VERSION>]
          The desired version of clang to install

          [default: ""]

  -V, --verbose
          Enable verbose logging for debugging purposes.

          This will include more DEBUG level log messages.
          Without it, log level is set to INFO by default.

  -t, --tool <TOOL>
          The clang tool to install

          [default: "clang-format clang-tidy"]
          [possible values: clang-tidy, clang-format]

  -d, --directory <DIRECTORY>
          The directory where the clang tools should be installed

  -f, --force
          Force overwriting symlink to the installed binary.

          This will only overwrite an existing symlink.

  -h, --help
          Print help (see a summary with '-h')
```

</p>
</details>

For example, to install version 21 of clang-format and clang-tidy:

```sh
clang-tools --version 21
```

[crates-io-badge]: https://img.shields.io/crates/v/clang-tools-manager
[crates-io-link]: https://crates.io/crates/clang-tools-manager
[changelog-badge]: https://img.shields.io/badge/keep_a_change_log-v1.1.0-ffec3d
[changelog-link]: https://github.com/cpp-linter/cpp-linter-rs/tree/main/clang-tools-manager/CHANGELOG.md
