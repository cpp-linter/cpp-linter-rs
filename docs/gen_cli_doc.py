from pathlib import Path
from typing import Dict, Any
import yaml
import mkdocs_gen_files
from cli_gen import generate_cli_doc

FILENAME = "cli.md"

VERSION_DISCLAIMER = """
!!! quote "v1.x vs v2.x"

    - v1.x was written in pure python
    - v2.x was written in rust (with python and node.js bindings)

    Version 2.x is intended to be backwards compatible, but a complete
    rewrite in rust prompted a major version bump.

    The minimum versions (badges) specified here hyperlink to different repositories.
    Anything established in v2.x will correspond to the rust project.
    Anything established in v1.4.6 or later (before v2.0.0) will correspond to the pure
    python project. Anything established before v1.4.6 will correspond to pure python
    sources in the cpp-linter-action project.

"""

with mkdocs_gen_files.open(FILENAME, "w") as io_doc:
    options_versions = Path(__file__).parent / "cli.yml"
    versions: Dict[str, Any] = yaml.safe_load(options_versions.read_bytes())

    print("# Command Line Interface\n", file=io_doc)
    print(VERSION_DISCLAIMER, file=io_doc)
    doc = generate_cli_doc(versions["inputs"])
    # print(doc)
    print(doc, file=io_doc)

mkdocs_gen_files.set_edit_path(FILENAME, "gen_cli_doc.py")
