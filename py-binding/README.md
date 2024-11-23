# cpp-linter
<!-- start -->
The python binding for the [cpp-linter-rs][this] rust project
(built using [pyo3](https://pyo3.rs) and [maturin]).

[this]: https://github.com/cpp-linter/cpp-linter-rs
[maturin]: https://maturin.rs

## Install

Install with `pip`:

```text
pip install cpp-linter
```

Pre-releases are uploaded to [test-pypi](https://test.pypi.org/project/cpp-linter/):

```text
pip install -i https://test.pypi.org/simple/ cpp-linter
```

## Usage

For usage in a CI workflow, see
[the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see
[the CLI documentation](https://cpp-linter.github.io/cpp-linter-rs/cli.html).

## Development

Build the binding with [maturin]:

```text
maturin dev --manifest-path py-binding/Cargo.toml
```

Then invoke the executable script as a normal CLI app:

```text
cpp-linter -help
```

### Folder structure

| Name | Description |
|-----:|:------------|
| `cpp_linter` | The pure python sources that wrap the rust binding. Typing information is located here. |
| `src` | The location for all rust sources related to binding the cpp-linter library. |
| `Cargo.toml` | Metadata about the binding's rust package (which _is not_ intended to be published to crates.io). |
| `pyproject.toml` | Metadata about the python package. |
| `requirements-dev.txt` | The dependencies used in development (not needed for runtime/production). |

Hidden files and folders are not described in the table above.
If they are not ignored by a gitignore specification, then they should be considered
important for maintenance or distribution.