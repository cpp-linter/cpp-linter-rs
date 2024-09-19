import argparse
from pathlib import Path
import sys
import re

VER_PATTERN = re.compile(
    r'^version = "(\d+)\.(\d+)\.(\d+)(?:\-rc)?(\d*)[^"]*" # auto', re.MULTILINE
)
VER_REPLACE = 'version = "%d.%d.%d%s" # auto'
COMPONENTS = ("major", "minor", "patch", "rc")


class Updater:
    component: str = "patch"

    @staticmethod
    def replace(match: re.Match[str]) -> str:
        ver = []
        for v in match.groups():
            try:
                ver.append(int(v))
            except ValueError:
                ver.append(0)
        old_version = ".".join([str(x) for x in ver[:3]])
        rc_str = ""
        if ver[3] > 0:
            rc_str = f"-rc{ver[3]}"
        old_version += rc_str
        print("old version:", old_version)
        index = COMPONENTS.index(Updater.component)
        ver[index] += 1
        for i in range(index + 1, len(COMPONENTS)):
            ver[i] = 0
        new_version = ".".join([str(x) for x in ver[:3]])
        rc_str = f"-rc{ver[3]}" if ver[3] > 0 else ""
        new_version += rc_str
        print("new version:", new_version)
        return VER_REPLACE % (tuple(ver[:3]) + (rc_str,))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("component", default="patch", choices=COMPONENTS)
    parser.parse_args(namespace=Updater)
    cargo_path = Path("Cargo.toml")
    doc = cargo_path.read_text(encoding="utf-8")
    doc = VER_PATTERN.sub(Updater.replace, doc)
    cargo_path.write_text(doc, encoding="utf-8", newline="\n")
    print("Updated version in Cargo.toml")
    return 0


if __name__ == "__main__":
    sys.exit(main())
