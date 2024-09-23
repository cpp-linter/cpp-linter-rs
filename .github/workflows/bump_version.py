"""This script automated the release process for all of the packages in this repository.
In order, this script does the following:

1. Bump version number in manifest files according to given required arg (see `--help`).
   This alters the Cargo.toml in repo root and the package.json files in node-binding.

   Requires `yarn` (see https://yarnpkg.com) and `napi` (see https://napi.rs) installed
   to bump node-binding versions.

2. Updates the CHANGELOG.md

   Requires `git-cliff` installed (see https://git-cliff.org)
   to regenerate the change logs from git history.

   NOTE: `git cliff` uses GITHUB_TOKEN env var to access GitHub's REST API for
   fetching certain data (like PR labels and commit author's username).

3. Pushes the changes from above 2 steps to remote

4. Creates a GitHub Release and uses the section from the CHANGELOG about the new tag
   as a release description.

   Requires `gh-cli` installed (see https://cli.github.com) to create the release and
   push the tag.

   NOTE: This step also tags the commit from step 3.
   When a tag is pushed to the remote, the CI builds are triggered and

   - release assets are uploaded to the Github Release corresponding to the new tag
   - packages are published for npm, pip and cargo

   NOTE: In a CI run, the GH_TOKEN env var to authenticate access.
   Locally, you can use `gh login` to interactively authenticate the user account.
"""

import argparse
from pathlib import Path
import subprocess
import re

VER_PATTERN = re.compile(
    r'^version = "(\d+)\.(\d+)\.(\d+)(?:\-rc)?(\d*)[^"]*" # auto', re.MULTILINE
)
VER_REPLACE = 'version = "%d.%d.%d%s" # auto'
COMPONENTS = ("major", "minor", "patch", "rc")
VERSION_LOG = re.compile(rb"^## \[\d+\.\d+\.\d+(?:\-rc)?\d*\]")


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


def get_release_notes(tag: str = Updater.new_version):
    title, buf = ("", b"")
    log_file = Path(__file__).parent.parent.parent / "CHANGELOG.md"
    tag_buf = f"[{tag}]".encode(encoding="utf-8")
    with open(str(log_file), "rb") as log:
        found_notes = False
        for line in log.readlines():
            matched = VERSION_LOG.match(line)
            if matched is not None:
                if tag_buf in matched.group(0):
                    title = tag + line[matched.end() :].decode(encoding="utf-8")
                    found_notes = True
                else:
                    found_notes = False
            elif line.startswith(b"[unreleased]: ") and found_notes:
                found_notes = False
            elif found_notes:
                buf += line
            elif line.startswith(tag_buf + b": "):
                buf += line.replace(tag_buf, b"Full commit diff", 1)
    return title.rstrip(), buf.strip()


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

    subprocess.run(["git", "cliff", "--tag", Updater.new_version], check=True)
    print("Updated CHANGELOG.md")

    subprocess.run(["git", "add", "--all"], check=True)
    tag = "v" + Updater.new_version
    subprocess.run(["git", "commit", "-m", f"Bump version to {tag}"], check=True)
    try:
        subprocess.run(["git", "push"], check=True)
    except subprocess.CalledProcessError as exc:
        raise RuntimeError("Failed to push commit for version bump") from exc

    title, notes = get_release_notes()
    print("Pushed commit to 'Bump version to", tag, "'")
    gh_cmd = [
        "gh",
        "release",
        "create",
        tag,
        "--title",
        title,
        "--notes",
        notes.decode("utf-8"),
    ]
    if Updater.component == "rc":
        gh_cmd.append("--prerelease")
    subprocess.run(gh_cmd, check=True)
    print("Created tag", tag, "and corresponding GitHub Release")


if __name__ == "__main__":
    main()
