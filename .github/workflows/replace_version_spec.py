import argparse
from pathlib import Path
import sys


class Args(argparse.Namespace):
    new_version: str = "2.0.0"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-n", "--new-version", required=True)
    args = parser.parse_args(namespace=Args())
    cargo_path = Path("Cargo.toml")
    if not cargo_path.exists():
        print("workspace Cargo.toml not in working directory")
        return 1
    doc = cargo_path.read_text(encoding="utf-8")
    version_pattern = 'version = "%s" # auto'
    old_version = version_pattern % "2.0.0"
    if old_version not in doc:
        print("Could not find version in Cargo.toml:\n", doc)
        return 1
    doc = doc.replace(old_version, version_pattern % args.new_version)
    cargo_path.write_text(doc, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
