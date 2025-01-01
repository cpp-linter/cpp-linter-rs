import re
import mkdocs_gen_files
from subprocess import run

FILENAME = "other-licenses.md"

INTRO = """# Third-party Licenses

[MIT]: https://choosealicense.com/licenses/mit
[Apache-2.0]: https://choosealicense.com/licenses/apache-2.0/
[MPL-2.0]: https://choosealicense.com/licenses/mpl-2.0
"""

TABLE_HEADER = "| Dependency | License |\n|:------------|:-------|\n"

OPTIONAL_DEPS = f"""## Optional dependencies

The following are conditionally included in binaries (using the `openssl-vendored`
feature on a case-by-case basis) because it is a dependency of
[git2](https://crates.io/crates/git2):

{TABLE_HEADER}\
| [openssl](https://crates.io/crates/openssl) | [Apache-2.0] |
| [openssl-probe](https://crates.io/crates/openssl-probe) | [MIT] OR [Apache-2.0] |
"""

PY_BINDING_HEADER = f"""## Bindings' dependencies

### Python binding

{TABLE_HEADER}"""

JS_BINDING_HEADER = f"""### Node.js binding

{TABLE_HEADER}"""

SELF_DEP = re.compile(r"(\| \[cpp-linter v[0-9.]+[^\s]*)[^\]]+(\]\(.*)$")


class TreeGetter:
    def __init__(self):
        self.args = [
            "cargo",
            "tree",
            "-f",
            r"| [{p}]({r}) | {l} |",
            "-e",
            "normal",
            "-p",
            "cpp-linter",
            "--depth",
            "1",
        ]

    def package(self, value: str) -> None:
        self.args[7] = value

    def get_output(self) -> str:
        output = run(
            self.args,
            capture_output=True,
            check=True,
        )
        result = []
        for line in output.stdout.decode(encoding="utf-8").splitlines()[1:]:
            dep = (
                line[3:]
                .replace(" MIT", " [MIT]")
                .replace(" Apache-2.0", " [Apache-2.0]")
                .replace(" MPL-2.0", " [MPL-2.0]")
                .strip()
            )
            self_match = SELF_DEP.match(dep)
            if self_match is not None:
                dep = SELF_DEP.sub(r"\1\2", dep)
            result.append(dep)
        return "\n".join(result)


with mkdocs_gen_files.open(FILENAME, "w") as io_doc:
    tg = TreeGetter()
    print(INTRO, file=io_doc)
    doc = TABLE_HEADER
    doc += tg.get_output()
    # print(doc)
    print(doc, file=io_doc)
    print(f"\n{OPTIONAL_DEPS}\n", file=io_doc)
    tg.package("cpp-linter-py")
    doc = tg.get_output()
    print(f"\n{PY_BINDING_HEADER}{doc}", file=io_doc)
    tg.package("cpp-linter-js")
    doc = tg.get_output()
    print(f"\n{JS_BINDING_HEADER}{doc}", file=io_doc)

mkdocs_gen_files.set_edit_path(FILENAME, "license-gen.py")
