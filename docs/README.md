# Docs

This folder is only for generating the documentation about CLI and runtime permissions.
Please [visit our website][gh-pages] to see generated documentation.

[gh-pages]: https://cpp-linter.github.io/cpp-linter-rs
[uv]: https://docs.astral.sh/uv/

## Build and inspect locally

To view the documentation locally, some software needs to be installed.
This project's dependencies are managed with a tool called [`uv`][uv].
So, [`uv`][uv] is the only software that needs to be manually installed beforehand.

After [`uv`][uv] is installed, building (and viewing) the docs is as simple as

```shell
uvx nox -s docs -- --open
```
