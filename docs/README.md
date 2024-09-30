# Docs

This folder is only for generating the documentation.
Please [visit our website][gh-pages] to see generated documentation.

[gh-pages]: https://cpp-linter.github.io/cpp-linter-rs

To view the documentation locally, some software needs to be installed.

```shell
pip install maturin
cd docs
maturin dev
pip install -r docs/requirements.txt
```

Then use `mkdocs` to generate the docs and open them in your browser.

```shell
mkdocs serve --open
```
