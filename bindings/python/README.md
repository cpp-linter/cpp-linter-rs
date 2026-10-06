# cpp-linter
<!-- start -->
The python binding for the [cpp-linter-rs][this] rust project
(built using [pyo3](https://pyo3.rs) and [maturin]).

[Website](https://cpp-linter.github.io/) |
[Documentation](https://cpp-linter.github.io/cpp-linter-rs/) |
[Get started](https://cpp-linter.github.io/getting-started/#locally-or-in-other-ci) |
[Discussions](https://github.com/orgs/cpp-linter/discussions)

[this]: https://github.com/cpp-linter/cpp-linter-rs
[maturin]: https://maturin.rs

## Quick start

Pre-releases are uploaded to [test-pypi](https://test.pypi.org/project/cpp-linter/):

```text
pip install --pre -i https://test.pypi.org/simple/ cpp-linter
```

Until 2.0 is released, `pip install cpp-linter` installs the
[pure python cpp-linter (v1.x)](https://github.com/cpp-linter/cpp-linter)
package from PyPI.

## Usage

For usage in a CI workflow, see
[the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see
[the CLI documentation](https://cpp-linter.github.io/cpp-linter-rs/cli/).

## Development

Build the binding with [maturin] (from repository root folder):

```text
maturin dev
```

Then invoke the executable script as a normal CLI app:

```text
cpp-linter --help
```

### Folder structure

| Name | Description |
|-----:|:------------|
| `src` | The location for all rust sources related to binding the cpp-linter library. |
| `Cargo.toml` | Metadata about the binding's rust package (which _is not_ intended to be published to crates.io). |
| `../../cpp_linter.pyi` | The typing stubs for the package (located in repo root). |
| `../../pyproject.toml` | Metadata about the python package (located in repo root). |

Hidden files and folders are not described in the table above.
If they are not ignored by a gitignore specification, then they should be considered
important only for maintenance or distribution.
