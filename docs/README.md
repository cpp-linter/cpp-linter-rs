# Docs

This folder is only for generating the documentation.
Please [visit our website][gh-pages] to see generated documentation.

[gh-pages]: https://cpp-linter.github.io/cpp_linter_rs

To view the documentation locally, some software needs to be installed.

```shell
cargo install --locked cargo-binstall
cargo binstall -y mdbook mdbook-alerts
```

Then use `mdbook` to generate the docs and open them in your browser.

```shell
# in repo root folder
mdbook serve docs --open
```
