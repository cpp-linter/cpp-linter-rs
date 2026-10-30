<!-- markdownlint-disable MD041 -->

[lines-changed-only]: cli.md#-l-lines-changed-only
[files-changed-only]: cli.md#-f-files-changed-only
[version]: cli.md#-v-version
[mod-sys]: cli.md#-mod-sys
[no-mod-sys]: cli.md#-no-mod-sys
[ignore]: cli.md#-i-ignore
[database]: cli.md#-p-database
[repo-root]: cli.md#-r-repo-root
[pr-review]: cli.md#-p-pr-review
[gen-patch]: cli.md#-f-gen-patch
[sarif-file]: cli.md#-sarif-file

# Migrating from v1

This page lists what changes when moving from cpp-linter v1 (the Python package) to v2 (this Rust
project). Most options keep their names, short flags and meaning, so many command lines work
unchanged. The sections below cover the differences.

## Removed options

| v1 option | v2 behavior | What to do |
|---|---|---|
| `-j`, `--jobs` | Files are always analyzed in parallel. | Remove the option. v2 exits with an "unexpected argument" error if it is given. |

## Renamed or merged options

| v1 option | v2 option | Notes |
|---|---|---|
| `-d`, `--tidy-review` | [`-P`, `--pr-review`][pr-review] | v2 posts one review with suggestions from both clang-tidy and clang-format. The old names still work as aliases of `--pr-review`, so `-d true` now also enables clang-format suggestions. |
| `-m`, `--format-review` | [`-P`, `--pr-review`][pr-review] | Same as above. `-m true` now also enables clang-tidy suggestions. |
| `-F`, `--fix` | [`-F`, `--gen-patch`][gen-patch] | See [Fixes are written to a patch](#fixes-are-written-to-a-patch). `--fix` is kept as an alias. |

## Changed defaults

| Option | v1 default | v2 default |
|---|---|---|
| [`-l`, `--lines-changed-only`][lines-changed-only] | `false` | `true` |
| [`-i`, `--ignore`][ignore] | `.github` | `.github` and `target` |

Because `--lines-changed-only` is `true` by default and implies
[`--files-changed-only`][files-changed-only], v2 analyzes only the added lines of changed files
unless told otherwise. To keep the v1 behavior of analyzing every line of every source file, pass:

```shell
cpp-linter --lines-changed-only=false --files-changed-only=false
```

## Fixes are written to a patch

In v1, `--fix` applied clang-format fixes in place, editing the files in the working tree.

In v2, cpp-linter never modifies the working tree. [`--gen-patch`][gen-patch] (alias `--fix`)
writes one patch file that combines the clang-format and clang-tidy fixes. Its path is printed in
the log and, in CI, set as the `fix-patch-path` output. To apply the fixes locally:

```shell
cpp-linter --gen-patch
git apply path/to/the/printed/patch
```

## Clang tools are installed for you

v1 expects clang-format and clang-tidy to be installed already, for example with
[clang-tools-pip](https://github.com/cpp-linter/clang-tools-pip). `--version` only selects which
installed version to use.

In v2, [`-V`, `--version`][version] also accepts a version requirement (for example `16`, `=12.0.1`
or `>=10, <13`). If no matching tools are found on the system, v2 downloads them. It tries
PyPI wheels first, then the system package manager (on by default in CI, see
[`--mod-sys`][mod-sys] and [`--no-mod-sys`][no-mod-sys]), then static binaries built by
cpp-linter. A path to an install location and a blank string (the default installed version) work
as in v1.

Running `-V` without a value prints the cpp-linter version, like the `version` subcommand.

## Paths relative to the repository root

A relative [`--database`][database] path is resolved against [`--repo-root`][repo-root] in v2. In
v1 (1.15 and later) it is resolved against the current working directory. Both give the same result
with the default `--repo-root` of `.`. If you set `--repo-root` to a sub-folder, pass the database
path relative to that folder, or use an absolute path.

## New in v2

- [`--sarif-file`][sarif-file] writes clang-tidy diagnostics as a SARIF 2.1.0 file, for example to
  upload to GitHub code scanning.
- [`--gen-patch`][gen-patch] produces a patch of all fixes, described above.
- [`--mod-sys`][mod-sys] and [`--no-mod-sys`][no-mod-sys] control whether system package managers
  may be used to install the clang tools.
