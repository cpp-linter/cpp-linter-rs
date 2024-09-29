# cpp-linter/cpp-linter-rs/install-clang-action

This is action is not meant to be published in the GitHub marketplace.
It intended to be a repo-specific action for installing multiple versions of
clang-format and clang-tidy in a single workflow run.

This is used in the cpp-linter rust tests only.

## Example

```yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - name: Install clang tools v16
        uses: cpp-linter/cpp-linter-rs/install-clang-action@main
        with:
          version: 16
      - name: test with clang tools v16
        env:
          CLANG_VERSION: 16
        run: just test
      - name: Install clang tools v17
        uses: cpp-linter/cpp-linter-rs/install-clang-action@main
        with:
          version: 17
      - name: test with clang tools v17
        env:
          CLANG_VERSION: 17
        run: just test
```
