# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [Unreleased]

### <!-- 1 --> 🚀 Added

- Add changelog and automate version bump and release workflows by @2bndy5 in [#42](https://github.com/cpp-linter/cpp_linter_rs/pull/42)

### <!-- 4 --> 🛠️ Fixed

- Fix typo in node-binding/README by @2bndy5 in [`7732676`](https://github.com/cpp-linter/cpp_linter_rs/commit/7732676e03941a37a4fb5b474d319c640689985a)

### <!-- 8 --> 📝 Documentation

- Release trial follow up by @2bndy5 in [#41](https://github.com/cpp-linter/cpp_linter_rs/pull/41)

### <!-- 9 --> 🗨️ Changed

- Gimme them badges by @2bndy5 in [`c0f1ea5`](https://github.com/cpp-linter/cpp_linter_rs/commit/c0f1ea516ee6efdf1137884cbc2e99e4ce1d4a11)

[Unreleased]: https://github.com/cpp-linter/cpp_linter_rs/compare/v2.0.0-rc4...HEAD

Full commit diff: [`v2.0.0-rc4...HEAD`][Unreleased]

## [2.0.0-rc4] - 2024-09-21

### <!-- 4 --> 🛠️ Fixed

- Fix metadata in platform-specific node pkgs by @2bndy5 in [`1dbb1de`](https://github.com/cpp-linter/cpp_linter_rs/commit/1dbb1de3abdb231646a33ac2721e6a8778ca4ece)

### <!-- 9 --> 🗨️ Changed

- Bump version to v2.0.0-rc4 by @2bndy5 in [`3e98e20`](https://github.com/cpp-linter/cpp_linter_rs/commit/3e98e20d2405b909b038ff87911dc0d5457613cc)

[2.0.0-rc4]: https://github.com/cpp-linter/cpp_linter_rs/compare/v2.0.0-rc3...v2.0.0-rc4

Full commit diff: [`v2.0.0-rc3...v2.0.0-rc4`][2.0.0-rc4]

## [2.0.0-rc3] - 2024-09-21

### <!-- 9 --> 🗨️ Changed

- [node] add life cycle script prepublishOnly by @2bndy5 in [`55650ea`](https://github.com/cpp-linter/cpp_linter_rs/commit/55650ea96aac628023acb120525d674bcf17a529)
- Bump version to v2.0.0-rc3 by @2bndy5 in [`070c5f7`](https://github.com/cpp-linter/cpp_linter_rs/commit/070c5f75f15d0190ee0204992165673c8f16c47d)

[2.0.0-rc3]: https://github.com/cpp-linter/cpp_linter_rs/compare/v2.0.0-rc2...v2.0.0-rc3

Full commit diff: [`v2.0.0-rc2...v2.0.0-rc3`][2.0.0-rc3]

## [2.0.0-rc2] - 2024-09-21

### <!-- 1 --> 🚀 Added

- Use napi-rs by @2bndy5 in [#39](https://github.com/cpp-linter/cpp_linter_rs/pull/39)
- Add `napi version` cmd to `just bump` script by @2bndy5 in [`a6a8bf2`](https://github.com/cpp-linter/cpp_linter_rs/commit/a6a8bf2f8f02c8d1a7b4047dae7bb13b537c370a)

### <!-- 6 --> 📦 Dependency updates

- Bump pypa/gh-action-pypi-publish in the actions group by @dependabot[bot] in [#40](https://github.com/cpp-linter/cpp_linter_rs/pull/40)

### <!-- 9 --> 🗨️ Changed

- Update READMEs by @2bndy5 in [`3e9c128`](https://github.com/cpp-linter/cpp_linter_rs/commit/3e9c12846c0eb96f8cdd68fc7435bd8965e7ce6a)
- Some cleanup from release trials by @2bndy5 in [`25c3951`](https://github.com/cpp-linter/cpp_linter_rs/commit/25c3951b0ecef9e078ea71932c9401ad8abc2a28)
- Bump version to v2.0.0-rc2 by @2bndy5 in [`ebcb6c4`](https://github.com/cpp-linter/cpp_linter_rs/commit/ebcb6c4941fbaa8147c768252d6d7d9adcfa3bb3)

[2.0.0-rc2]: https://github.com/cpp-linter/cpp_linter_rs/compare/v2.0.0-rc1...v2.0.0-rc2

Full commit diff: [`v2.0.0-rc1...v2.0.0-rc2`][2.0.0-rc2]

## [2.0.0-rc1] - 2024-09-19

### <!-- 1 --> 🚀 Added

- Add more testing and various improvements by @2bndy5 in [#4](https://github.com/cpp-linter/cpp_linter_rs/pull/4)
- Support file paths in CLI positional argument by @2bndy5 in [#16](https://github.com/cpp-linter/cpp_linter_rs/pull/16)
- Support glob patterns by @2bndy5 in [#25](https://github.com/cpp-linter/cpp_linter_rs/pull/25)
- Resort to paginated requests for changed files by @2bndy5 in [#37](https://github.com/cpp-linter/cpp_linter_rs/pull/37)

### <!-- 4 --> 🛠️ Fixed

- Fix parsing of `--extra-arg` by @2bndy5 in [`03f3de5`](https://github.com/cpp-linter/cpp_linter_rs/commit/03f3de5232e29446d57de00d8ac6deb2fc17d9a5)
- Fix CI docs workflow by @2bndy5 in [`ae33a6d`](https://github.com/cpp-linter/cpp_linter_rs/commit/ae33a6d81da82d8f6c1b2b438e748dd276e4f61f)
- Fix GithubApiClient init for non-PR events by @2bndy5 in [`5b60ab8`](https://github.com/cpp-linter/cpp_linter_rs/commit/5b60ab8af020f81fc986cdf86568263b5e5f8e50)
- Fix typo in README by @2bndy5 in [`afa1312`](https://github.com/cpp-linter/cpp_linter_rs/commit/afa1312af05f3920e9750dd1371fcad09643bc3f)
- Fix dependabot config by @2bndy5 in [`3957be2`](https://github.com/cpp-linter/cpp_linter_rs/commit/3957be228662faa3ab0c7241a88ac3b9d3bd09f8)
- Fix links to clang-analyzer diagnostic's help site by @2bndy5 in [#36](https://github.com/cpp-linter/cpp_linter_rs/pull/36)
- Fix CI workflows for publishing releases by @2bndy5 in [`4f9b912`](https://github.com/cpp-linter/cpp_linter_rs/commit/4f9b91234bf05fd14afc60d7c87768d7ca0d7bb0)
- Fix release CI by @2bndy5 in [`49b3487`](https://github.com/cpp-linter/cpp_linter_rs/commit/49b3487c6d0804c075c7e8863be921c8ba3fdaea)
- Fix release CI steps by @2bndy5 in [`23efee5`](https://github.com/cpp-linter/cpp_linter_rs/commit/23efee50413ae6b6d1b51d147dcdc832d213de94)
- Fix metadata and switch to pypa-publish action by @2bndy5 in [`092e0c2`](https://github.com/cpp-linter/cpp_linter_rs/commit/092e0c20cf66747b59bab4bdf60be29f7f02dcc6)

### <!-- 6 --> 📦 Dependency updates

- Bump openssl from 0.10.62 to 0.10.66 by @dependabot[bot] in [#6](https://github.com/cpp-linter/cpp_linter_rs/pull/6)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#7](https://github.com/cpp-linter/cpp_linter_rs/pull/7)
- Bump the cargo group with 3 updates by @dependabot[bot] in [#15](https://github.com/cpp-linter/cpp_linter_rs/pull/15)
- Bump serde_json from 1.0.125 to 1.0.127 in the cargo group by @dependabot[bot] in [#19](https://github.com/cpp-linter/cpp_linter_rs/pull/19)
- Bump serde from 1.0.208 to 1.0.209 in the cargo group by @dependabot[bot] in [#23](https://github.com/cpp-linter/cpp_linter_rs/pull/23)
- Bump tempfile from 3.9.0 to 3.12.0 in the cargo group by @dependabot[bot] in [#26](https://github.com/cpp-linter/cpp_linter_rs/pull/26)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#34](https://github.com/cpp-linter/cpp_linter_rs/pull/34)

### <!-- 7 -->🚦 Tests

- Refactor line filters; minor metadata updates by @2bndy5 in [`19d5517`](https://github.com/cpp-linter/cpp_linter_rs/commit/19d5517dc1c95c8269c0beb583387df6197b1ec7)
- Mock REST API calls in tests by @2bndy5 in [#21](https://github.com/cpp-linter/cpp_linter_rs/pull/21)
- PR review suggestions by @2bndy5 in [`bd049d0`](https://github.com/cpp-linter/cpp_linter_rs/commit/bd049d06c48b4dc40da958a478873ac30183ee46)

### <!-- 8 --> 📝 Documentation

- Switch to mdbook for docs by @2bndy5 in [#13](https://github.com/cpp-linter/cpp_linter_rs/pull/13)
- Begin documenting permissions by @2bndy5 in [#22](https://github.com/cpp-linter/cpp_linter_rs/pull/22)

### <!-- 9 --> 🗨️ Changed

- Init commit by @2bndy5 in [`2e25fec`](https://github.com/cpp-linter/cpp_linter_rs/commit/2e25fec0a447df24d0bcc1b80f6624040bab755e)
- Use separate crates for different entry points by @2bndy5 in [#2](https://github.com/cpp-linter/cpp_linter_rs/pull/2)
- Update README.md by @2bndy5 in [`ff4a735`](https://github.com/cpp-linter/cpp_linter_rs/commit/ff4a735ec5a74cc9a2e835e58dc76696233ad688)
- Some updates from py codebase by @2bndy5 in [#5](https://github.com/cpp-linter/cpp_linter_rs/pull/5)
- Rename test CI; add badges to README by @2bndy5 in [`b77058f`](https://github.com/cpp-linter/cpp_linter_rs/commit/b77058f166f1062abe8193ab6fc4bc671793a7c8)
- Use task runner `just` (or VSCode "tasks") by @2bndy5 in [#14](https://github.com/cpp-linter/cpp_linter_rs/pull/14)
- Update README by @2bndy5 in [`215485c`](https://github.com/cpp-linter/cpp_linter_rs/commit/215485c3f5032b7253e2d13f6726e3bfe70a16d0)
- Prepare for v2.0.0-rc1 by @2bndy5 in [`9189e86`](https://github.com/cpp-linter/cpp_linter_rs/commit/9189e86da499606439f6b65b62df5603f57d9da7)
- Refactor files by @2bndy5 in [#38](https://github.com/cpp-linter/cpp_linter_rs/pull/38)
- Metadata changes by @2bndy5 in [`f4237ae`](https://github.com/cpp-linter/cpp_linter_rs/commit/f4237ae593e468eca0e63169c9360e97bd6e1f26)

[2.0.0-rc1]: https://github.com/cpp-linter/cpp_linter_rs/compare/2e25fec0a447df24d0bcc1b80f6624040bab755e...v2.0.0-rc1

Full commit diff: [`2e25fec...v2.0.0-rc1`][2.0.0-rc1]

<!-- generated by git-cliff -->
