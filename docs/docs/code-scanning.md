# GitHub Code Scanning

[sarif-file]: cli.md#-sarif-file
[lines-changed-only]: cli.md#-l-lines-changed-only
[files-changed-only]: cli.md#-f-files-changed-only
[quick-start]: index.md#quick-start
[upload-sarif]: https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/uploading-a-sarif-file-to-github

[`--sarif-file`][sarif-file] writes the clang-tidy diagnostics as a SARIF file, which
[GitHub code scanning][upload-sarif] can show as alerts. The repository's Security tab then lists
the alerts of each branch, and a pull request gets a `clang-tidy` check with an annotation for each
new alert on the lines it changes. clang-format findings are not part of the SARIF file.

## Scan whole files

The SARIF file follows the source filters, and [`--lines-changed-only`][lines-changed-only]
defaults to `true`. Code scanning, however, treats every upload for a branch as the full state of
that branch. If a push uploads only the diagnostics on the lines it changed, every other alert on
that branch is closed as fixed, and opens again with the next full scan.

So run cpp-linter with these options:

| Event | Options |
|---|---|
| `push` to the default branch | [`--lines-changed-only=false`][lines-changed-only] and [`--files-changed-only=false`][files-changed-only], to scan every file |
| `pull_request` | [`--lines-changed-only=false`][lines-changed-only], to scan every line of the changed files |

For a pull request, code scanning compares the upload with the latest upload for the base branch,
and annotates only the alerts on the lines that the pull request changes. Scanning whole files
keeps the alerts in line with the base branch, and does not add annotations to unchanged lines.

## Example

```yaml title=".github/workflows/cpp-linter.yml"
name: cpp-linter
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  cpp-linter:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: read
      security-events: write
      actions: read # only needed in a private repository
    steps:
      - uses: actions/checkout@v4
      # install cpp-linter here, see "Quick start"
      - name: Run cpp-linter
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          FILES_CHANGED_ONLY: ${{ github.event_name == 'pull_request' }}
        run: >-
          cpp-linter
          --style=file
          --tidy-checks=''
          --lines-changed-only=false
          --files-changed-only="$FILES_CHANGED_ONLY"
          --sarif-file=cpp-linter.sarif
      - name: Upload to code scanning
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: cpp-linter.sarif
          category: cpp-linter
```

See [Quick start][quick-start] for the ways to install cpp-linter.

- The SARIF file is written even when there are no diagnostics, so an upload after the last
  alert is fixed closes it.
- `category` names this analysis. Keep it the same in every run, so that code scanning compares
  each upload with the earlier ones.
