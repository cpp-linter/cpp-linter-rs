import argparse
from pathlib import Path
import subprocess
import re

VER_PATTERN = re.compile(
    r'^version = "(\d+)\.(\d+)\.(\d+)(?:\-rc)?(\d*)[^"]*" # auto', re.MULTILINE
)
VER_REPLACE = 'version = "%d.%d.%d%s" # auto'
COMPONENTS = ("major", "minor", "patch", "rc")


class Updater:
    component: str = "patch"
    new_version: str = "0.0.0"

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
        Updater.new_version = new_version
        return VER_REPLACE % (tuple(ver[:3]) + (rc_str,))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("component", default="patch", choices=COMPONENTS)
    parser.parse_args(namespace=Updater)
    cargo_path = Path("Cargo.toml")
    doc = cargo_path.read_text(encoding="utf-8")
    doc = VER_PATTERN.sub(Updater.replace, doc)
    cargo_path.write_text(doc, encoding="utf-8", newline="\n")
    subprocess.run(["cargo", "update", "--workspace"], check=True)
    print("Updated version in Cargo.toml")
    subprocess.run(
        [
            "yarn",
            "version",
            "--new-version",
            Updater.new_version,
            "--no-git-tag-version",
        ],
        cwd="node-binding",
        check=True,
    )
    subprocess.run(["napi", "version"], cwd="node-binding", check=True)
    print("Updated version in node-binding/**package.json")
    tag = "v" + Updater.new_version
    subprocess.run(["git", "add", "--all"], check=True)
    subprocess.run(["git", "commit", "-m", f"bump version to {tag}"], check=True)
    try:
        subprocess.run(["git", "push"], check=True)
    except subprocess.CalledProcessError as exc:
        raise RuntimeError("Failed to push commit for version bump") from exc
    print("Pushed commit to 'bump version to", tag, "'")
    try:
        subprocess.run(["git", "tag", tag], check=True)
    except subprocess.CalledProcessError as exc:
        raise RuntimeError("Failed to create tag for commit") from exc
    print("Created tag", tag)
    print(f"Use 'git push origin refs/tags/{tag}' to publish a release")


if __name__ == "__main__":
    main()
