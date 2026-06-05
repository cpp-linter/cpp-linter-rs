import re
import mkdocs_gen_files
from subprocess import run

FILENAME = "other-licenses.md"

INTRO = """# Third-party Licenses

[MIT]: https://choosealicense.com/licenses/mit
[Apache-2.0]: https://choosealicense.com/licenses/apache-2.0/
[MPL-2.0]: https://choosealicense.com/licenses/mpl-2.0
[GPL-3.0]: https://choosealicense.com/licenses/gpl-3.0/
"""

TABLE_HEADER = "| Dependency | License |\n|:------------|:-------|\n"

CPP_LINTER_DEPS = f"""## cpp-linter's dependencies

{TABLE_HEADER}\
"""

CLANG_INSTALLER_DEPS = f"""## clang-installer's dependencies

{TABLE_HEADER}\
"""

PY_BINDING_HEADER = f"""## Bindings' dependencies

### Python binding

{TABLE_HEADER}"""

JS_BINDING_HEADER = f"""### Node.js binding

{TABLE_HEADER}"""

SELF_DEP = re.compile(
    r"(\| \[(?:cpp-linter|clang-installer) v[0-9.]+[^\s]*)[^\]]+(\]\(.*)$"
)


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
            "--color",
            "never",
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
                .replace(" GPL-3.0", " [GPL-3.0]")
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
    doc = CPP_LINTER_DEPS
    doc += tg.get_output()
    # print(doc)
    print(doc, file=io_doc)
    tg.package("cpp-linter-py")
    doc = tg.get_output()
    print(f"\n{PY_BINDING_HEADER}{doc}", file=io_doc)
    tg.package("cpp-linter-js")
    doc = tg.get_output()
    print(f"\n{JS_BINDING_HEADER}{doc}", file=io_doc)
    tg.package("clang-installer")
    doc = tg.get_output()
    print(f"\n{CLANG_INSTALLER_DEPS}{doc}", file=io_doc)

mkdocs_gen_files.set_edit_path(FILENAME, "license-gen.py")
