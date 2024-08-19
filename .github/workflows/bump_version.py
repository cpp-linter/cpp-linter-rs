import argparse
from pathlib import Path
import sys
import re

VER_PATTERN = re.compile(r'^version = "(\d+)\.(\d+)\.(\d+)[^"]*" # auto', re.MULTILINE)
VER_REPLACE = 'version = "%d.%d.%d" # auto'
COMPONENTS = ["major", "minor", "patch"]


class Args(argparse.Namespace):
    component: str = "patch"


def replace(match: re.Match | None) -> str:
    assert match is not None, "Could not find version in Cargo.toml"
    ver = [int(x) for x in match.groups()[:3]]
    assert len(ver) == 3
    print("old version:", ".".join([str(x) for x in ver]))
    index = COMPONENTS.index(Args.component)
    ver[index] += 1
    for i in range(index + 1, 3):
        ver[i] = 0
    print("new version:", ".".join([str(x) for x in ver]))
    return VER_REPLACE % tuple(ver)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("component", default="patch", choices=COMPONENTS)
    parser.parse_args(namespace=Args)
    cargo_path = Path("Cargo.toml")
    doc = cargo_path.read_text(encoding="utf-8")
    doc = VER_PATTERN.sub(replace, doc)
    cargo_path.write_text(doc, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
