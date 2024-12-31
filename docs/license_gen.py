import mkdocs_gen_files
from subprocess import run

FILENAME = "other-licenses.md"

INTRO = """# Third-party Licenses

[MIT]: https://choosealicense.com/licenses/mit
[Apache-2.0]: https://choosealicense.com/licenses/apache-2.0/
[MPL-2.0]: https://choosealicense.com/licenses/mpl-2.0
"""

OPTIONAL_DEPS = """## Optional dependencies

The following are conditionally included in binaries (using the `openssl-vendored`
feature on a case-by-case basis) because it is a dependency of
[git2](https://crates.io/crates/git2):

- [openssl](https://crates.io/crates/openssl): Licensed under [Apache-2.0].
- [openssl-probe](https://crates.io/crates/openssl-probe):
  Dual-licensed under [Apache-2.0] or [MIT].
"""

BINDING_DEPS = """## Bindings' dependencies

The python binding uses

- [pyo3](https://crates.io/crates/pyo3):
  Dual-licensed under [Apache-2.0] or [MIT].

The node binding uses

- [napi](https://crates.io/crates/napi): Licensed under [MIT]
- [napi-derive](https://crates.io/crates/napi-derive): Licensed under [MIT]
"""

with mkdocs_gen_files.open(FILENAME, "w") as io_doc:
    print(INTRO, file=io_doc)
    output = run(
        [
            "cargo",
            "tree",
            "-f",
            r"[{p}]({r}): Licensed under {l}",
            "-e",
            "normal",
            "-p",
            "cpp-linter",
            "--depth",
            "1",
        ],
        capture_output=True,
        check=True,
    )
    doc = "\n".join(
        [
            "- "
            + line[3:]
            .replace(" MIT", " [MIT]")
            .replace(" Apache-2.0", " [Apache-2.0]")
            .replace(" MPL-2.0", " [MPL-2.0]")
            for line in output.stdout.decode(encoding="utf-8").splitlines()[1:]
        ]
    )
    # print(doc)
    print(doc, file=io_doc)
    print(f"\n{OPTIONAL_DEPS}\n", file=io_doc)
    print(f"\n{BINDING_DEPS}\n", file=io_doc)

mkdocs_gen_files.set_edit_path(FILENAME, "license-gen.py")
