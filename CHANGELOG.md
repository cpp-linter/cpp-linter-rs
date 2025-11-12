# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [2.0.0-rc14] - 2025-11-12

### <!-- 6 --> 📦 Dependency updates

- Bump python dependencies by @2bndy5 in [#214](https://github.com/cpp-linter/cpp-linter-rs/pull/214)
- Bump oxlint from 1.24.0 to 1.28.0 in the npm group by @dependabot[bot] in [#215](https://github.com/cpp-linter/cpp-linter-rs/pull/215)

### <!-- 9 --> 🗨️ Changed

- Use trusted publishing for npm deployments by @2bndy5 in [#212](https://github.com/cpp-linter/cpp-linter-rs/pull/212)

[2.0.0-rc14]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc13...v2.0.0-rc14

Full commit diff: [`v2.0.0-rc13...v2.0.0-rc14`][2.0.0-rc14]

## [2.0.0-rc13] - 2025-11-12

### <!-- 1 --> 🚀 Added

- Merge pull request #90 from cpp-linter/patch-2 by @shenxianpeng in [#90](https://github.com/cpp-linter/cpp-linter-rs/pull/90)
- Switch to `quick_xml` library by @2bndy5 in [#101](https://github.com/cpp-linter/cpp-linter-rs/pull/101)
- Distribute future-compatible python wheels by @2bndy5 in [#178](https://github.com/cpp-linter/cpp-linter-rs/pull/178)
- Delegate vendoring of OpenSSL to git2 dependency tree by @2bndy5 in [#200](https://github.com/cpp-linter/cpp-linter-rs/pull/200)
- Improve CLI value parsing/docs by @2bndy5 in [#208](https://github.com/cpp-linter/cpp-linter-rs/pull/208)

### <!-- 4 --> 🛠️ Fixed

- Fix generated doc about licenses by @2bndy5 in [#159](https://github.com/cpp-linter/cpp-linter-rs/pull/159)
- Parse clang-tidy output when `WarningsAsErrors` is asserted by @2bndy5 in [#190](https://github.com/cpp-linter/cpp-linter-rs/pull/190)

### <!-- 6 --> 📦 Dependency updates

- Bump the npm group across 1 directory with 3 updates by @dependabot[bot] in [#96](https://github.com/cpp-linter/cpp-linter-rs/pull/96)
- Bump mkdocs-material from 9.5.49 to 9.5.50 in the pip group by @dependabot[bot] in [#99](https://github.com/cpp-linter/cpp-linter-rs/pull/99)
- Bump the cargo group across 1 directory with 16 updates by @dependabot[bot] in [#98](https://github.com/cpp-linter/cpp-linter-rs/pull/98)
- Bump openssl from 0.10.68 to 0.10.70 by @dependabot[bot] in [#105](https://github.com/cpp-linter/cpp-linter-rs/pull/105)
- Bump the cargo group across 1 directory with 14 updates by @dependabot[bot] in [#116](https://github.com/cpp-linter/cpp-linter-rs/pull/116)
- Bump ring from 0.17.8 to 0.17.13 by @dependabot[bot] in [#119](https://github.com/cpp-linter/cpp-linter-rs/pull/119)
- Bump the cargo group with 7 updates by @dependabot[bot] in [#120](https://github.com/cpp-linter/cpp-linter-rs/pull/120)
- Bump the pip group across 1 directory with 2 updates by @dependabot[bot] in [#117](https://github.com/cpp-linter/cpp-linter-rs/pull/117)
- Bump pyo3 from 0.24.0 to 0.24.1 by @dependabot[bot] in [#125](https://github.com/cpp-linter/cpp-linter-rs/pull/125)
- Bump openssl from 0.10.71 to 0.10.72 by @dependabot[bot] in [#127](https://github.com/cpp-linter/cpp-linter-rs/pull/127)
- Bump pypa/gh-action-pypi-publish in the actions group by @dependabot[bot] in [#103](https://github.com/cpp-linter/cpp-linter-rs/pull/103)
- Bump the npm group across 1 directory with 3 updates by @dependabot[bot] in [#118](https://github.com/cpp-linter/cpp-linter-rs/pull/118)
- Bump mkdocs-material from 9.6.9 to 9.6.10 in the pip group by @dependabot[bot] in [#123](https://github.com/cpp-linter/cpp-linter-rs/pull/123)
- Bump tokio from 1.44.0 to 1.44.2 by @dependabot[bot] in [#128](https://github.com/cpp-linter/cpp-linter-rs/pull/128)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#129](https://github.com/cpp-linter/cpp-linter-rs/pull/129)
- Bump the cargo group across 1 directory with 9 updates by @dependabot[bot] in [#139](https://github.com/cpp-linter/cpp-linter-rs/pull/139)
- Bump the npm group across 1 directory with 4 updates by @dependabot[bot] in [#137](https://github.com/cpp-linter/cpp-linter-rs/pull/137)
- Bump mkdocs-material from 9.6.11 to 9.6.12 in the pip group by @dependabot[bot] in [#132](https://github.com/cpp-linter/cpp-linter-rs/pull/132)
- Switch to uv and nox by @2bndy5 in [#145](https://github.com/cpp-linter/cpp-linter-rs/pull/145)
- Bump astral-sh/setup-uv from 5 to 6 in the actions group by @dependabot[bot] in [#150](https://github.com/cpp-linter/cpp-linter-rs/pull/150)
- Bump @eslint/js in the npm group across 1 directory by @dependabot[bot] in [#151](https://github.com/cpp-linter/cpp-linter-rs/pull/151)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#152](https://github.com/cpp-linter/cpp-linter-rs/pull/152)
- Bump @eslint/plugin-kit from 0.3.2 to 0.3.3 by @dependabot[bot] in [#162](https://github.com/cpp-linter/cpp-linter-rs/pull/162)
- Migrate to napi-rs v3 by @2bndy5 in [#164](https://github.com/cpp-linter/cpp-linter-rs/pull/164)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#166](https://github.com/cpp-linter/cpp-linter-rs/pull/166)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#165](https://github.com/cpp-linter/cpp-linter-rs/pull/165)
- Bump the actions group across 1 directory with 3 updates by @dependabot[bot] in [#171](https://github.com/cpp-linter/cpp-linter-rs/pull/171)
- Update cargo dependencies by @2bndy5 in [`9663a04`](https://github.com/cpp-linter/cpp-linter-rs/commit/9663a04ffcc83c70d890c052ba7a4176ed3e69f1)
- Bump slab from 0.4.10 to 0.4.11 by @dependabot[bot] in [#173](https://github.com/cpp-linter/cpp-linter-rs/pull/173)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#176](https://github.com/cpp-linter/cpp-linter-rs/pull/176)
- Bump the npm group with 2 updates by @dependabot[bot] in [#179](https://github.com/cpp-linter/cpp-linter-rs/pull/179)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#180](https://github.com/cpp-linter/cpp-linter-rs/pull/180)
- Bump the actions group with 4 updates by @dependabot[bot] in [#181](https://github.com/cpp-linter/cpp-linter-rs/pull/181)
- Bump the npm group with 2 updates by @dependabot[bot] in [#186](https://github.com/cpp-linter/cpp-linter-rs/pull/186)
- Bump the cargo group with 4 updates by @dependabot[bot] in [#185](https://github.com/cpp-linter/cpp-linter-rs/pull/185)
- Bump the actions group across 1 directory with 4 updates by @dependabot[bot] in [#197](https://github.com/cpp-linter/cpp-linter-rs/pull/197)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#191](https://github.com/cpp-linter/cpp-linter-rs/pull/191)
- Bump the actions group across 1 directory with 6 updates by @dependabot[bot] in [#203](https://github.com/cpp-linter/cpp-linter-rs/pull/203)
- Bump the cargo group across 1 directory with 11 updates by @dependabot[bot] in [#202](https://github.com/cpp-linter/cpp-linter-rs/pull/202)
- Bump the actions group across 1 directory with 5 updates by @dependabot[bot] in [#210](https://github.com/cpp-linter/cpp-linter-rs/pull/210)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#209](https://github.com/cpp-linter/cpp-linter-rs/pull/209)
- Bump yarn to v4.11.0 by @2bndy5 in [#213](https://github.com/cpp-linter/cpp-linter-rs/pull/213)

### <!-- 8 --> 📝 Documentation

- Reorganize LICENSE info by @shenxianpeng in [#89](https://github.com/cpp-linter/cpp-linter-rs/pull/89)
- Render licenses of deps in tables by @2bndy5 in [#91](https://github.com/cpp-linter/cpp-linter-rs/pull/91)

### <!-- 9 --> 🗨️ Changed

- Trim trailing whitespace by @2bndy5 in [`5757648`](https://github.com/cpp-linter/cpp-linter-rs/commit/57576486ccc3228a28b6463c04845eb960b8076b)
- Better Benchmark by @2bndy5 in [#92](https://github.com/cpp-linter/cpp-linter-rs/pull/92)
- Bump pre-commit hooks by @2bndy5 in [`4b1b5bd`](https://github.com/cpp-linter/cpp-linter-rs/commit/4b1b5bdff3b9ab454c4e62309e1f0a432b3f03ed)
- Use `Client` instance by reference by @2bndy5 in [#141](https://github.com/cpp-linter/cpp-linter-rs/pull/141)
- Update locked transitive dependencies by @2bndy5 in [`aad12c6`](https://github.com/cpp-linter/cpp-linter-rs/commit/aad12c633a0589f36f8cd5e462ad89e359c24d9c)
- Add permissions to stale checking CI by @2bndy5 in [`a9ddd15`](https://github.com/cpp-linter/cpp-linter-rs/commit/a9ddd158b5d4c362200ce15cd084a2f63aba7ea2)
- Review CI by @2bndy5 in [#195](https://github.com/cpp-linter/cpp-linter-rs/pull/195)
- Use clap derive feature by @2bndy5 in [#204](https://github.com/cpp-linter/cpp-linter-rs/pull/204)
- Fix benchmark CI about py-binding artifact by @2bndy5 in [#211](https://github.com/cpp-linter/cpp-linter-rs/pull/211)
- Persist git credentials for bump-n-release workflow by @2bndy5 in [`3b7d3e2`](https://github.com/cpp-linter/cpp-linter-rs/commit/3b7d3e26264ba24d0bbe593ba102d53af3a55686)

[2.0.0-rc13]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc12...v2.0.0-rc13

Full commit diff: [`v2.0.0-rc12...v2.0.0-rc13`][2.0.0-rc13]

## New Contributors

- @shenxianpeng made their first contribution in [#89](https://github.com/cpp-linter/cpp-linter-rs/pull/89)

## [2.0.0-rc12] - 2024-12-12

### <!-- 1 --> 🚀 Added

- Rationale to diagnostic comments in PR reviews by @2bndy5 in [`0923c6a`](https://github.com/cpp-linter/cpp-linter-rs/commit/0923c6a1c61617a09fd914a702959204fc41c26d)

### <!-- 8 --> 📝 Documentation

- [rust API] update logo, favicon, and some links by @2bndy5 in [`31b7add`](https://github.com/cpp-linter/cpp-linter-rs/commit/31b7add5ea8b1938ea4f816f27a732f3ec8d5227)

[2.0.0-rc12]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc11...v2.0.0-rc12

Full commit diff: [`v2.0.0-rc11...v2.0.0-rc12`][2.0.0-rc12]

## [2.0.0-rc11] - 2024-12-11

### <!-- 6 --> 📦 Dependency updates

- Bump pypa/gh-action-pypi-publish in the actions group by @dependabot[bot] in [#81](https://github.com/cpp-linter/cpp-linter-rs/pull/81)
- Bump the npm group with 3 updates by @dependabot[bot] in [#78](https://github.com/cpp-linter/cpp-linter-rs/pull/78)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#82](https://github.com/cpp-linter/cpp-linter-rs/pull/82)

[2.0.0-rc11]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc10...v2.0.0-rc11

Full commit diff: [`v2.0.0-rc10...v2.0.0-rc11`][2.0.0-rc11]

## [2.0.0-rc10] - 2024-12-11

### <!-- 1 --> 🚀 Added

- Prefix review comments with marker by @2bndy5 in [`9d2a9a3`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d2a9a3e4c4f91ab778959396e4e153d7cbb6d56)

### <!-- 6 --> 📦 Dependency updates

- Bump pyo3 from 0.23.2 to 0.23.3 by @dependabot[bot] in [#79](https://github.com/cpp-linter/cpp-linter-rs/pull/79)

[2.0.0-rc10]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc9...v2.0.0-rc10

Full commit diff: [`v2.0.0-rc9...v2.0.0-rc10`][2.0.0-rc10]

## [2.0.0-rc9] - 2024-11-27

### <!-- 4 --> 🛠️ Fixed

- Clang-tidy diagnostic comments in PR review by @2bndy5 in [#77](https://github.com/cpp-linter/cpp-linter-rs/pull/77)

### <!-- 6 --> 📦 Dependency updates

- Bump pyo3 from 0.23.1 to 0.23.2 in the cargo group by @dependabot[bot] in [#76](https://github.com/cpp-linter/cpp-linter-rs/pull/76)

[2.0.0-rc9]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc8...v2.0.0-rc9

Full commit diff: [`v2.0.0-rc8...v2.0.0-rc9`][2.0.0-rc9]

## [2.0.0-rc8] - 2024-11-24

### <!-- 4 --> 🛠️ Fixed

- Include type stubs in python source distribution by @2bndy5 in [`7dfcce7`](https://github.com/cpp-linter/cpp-linter-rs/commit/7dfcce72f39412f45208e5586d0a1ef2d0a40207)
- Clang tools' version output string in PR review summary by @2bndy5 in [`3333796`](https://github.com/cpp-linter/cpp-linter-rs/commit/33337965a240ff791d39d4e4cd6339855ea42fd8)

### <!-- 8 --> 📝 Documentation

- Minor update node binding README by @2bndy5 in [`fc2244f`](https://github.com/cpp-linter/cpp-linter-rs/commit/fc2244f81903c719a5268f18e595f422d1a958e1)

[2.0.0-rc8]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc7...v2.0.0-rc8

Full commit diff: [`v2.0.0-rc7...v2.0.0-rc8`][2.0.0-rc8]

## [2.0.0-rc6] - 2024-11-23

### <!-- 1 --> 🚀 Added

- Add optional colored log output by @2bndy5 in [#52](https://github.com/cpp-linter/cpp-linter-rs/pull/52)
- Add CI to detect performance regressions by @2bndy5 in [#53](https://github.com/cpp-linter/cpp-linter-rs/pull/53)
- Capture and output clang tool's version number by @2bndy5 in [#54](https://github.com/cpp-linter/cpp-linter-rs/pull/54)

### <!-- 4 --> 🛠️ Fixed

- Let public forks' PRs run git cliff in CI by @2bndy5 in [`0857121`](https://github.com/cpp-linter/cpp-linter-rs/commit/0857121826e2ae8ebdd896ba6033eb495b614613)
- Regenerate TS type definitions by @2bndy5 in [`023c170`](https://github.com/cpp-linter/cpp-linter-rs/commit/023c1705a078b9a7542022deefa228a567d68b67)

### <!-- 6 --> 📦 Dependency updates

- Bump reqwest from 0.12.7 to 0.12.8 in the cargo group by @dependabot[bot] in [#51](https://github.com/cpp-linter/cpp-linter-rs/pull/51)
- Bump the cargo group across 1 directory with 4 updates by @dependabot[bot] in [#58](https://github.com/cpp-linter/cpp-linter-rs/pull/58)
- Bump @eslint/plugin-kit from 0.2.0 to 0.2.3 by @dependabot[bot] in [#69](https://github.com/cpp-linter/cpp-linter-rs/pull/69)
- Bump cross-spawn from 7.0.3 to 7.0.5 by @dependabot[bot] in [#70](https://github.com/cpp-linter/cpp-linter-rs/pull/70)
- Bump the actions group across 1 directory with 2 updates by @dependabot[bot] in [#72](https://github.com/cpp-linter/cpp-linter-rs/pull/72)
- Bump the npm group across 1 directory with 4 updates by @dependabot[bot] in [#71](https://github.com/cpp-linter/cpp-linter-rs/pull/71)
- Bump the cargo group across 1 directory with 13 updates by @dependabot[bot] in [#73](https://github.com/cpp-linter/cpp-linter-rs/pull/73)

[2.0.0-rc6]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc5...v2.0.0-rc6

Full commit diff: [`v2.0.0-rc5...v2.0.0-rc6`][2.0.0-rc6]

## [2.0.0-rc5] - 2024-09-29

### <!-- 1 --> 🚀 Added

- Add changelog and automate version bump and release workflows by @2bndy5 in [#42](https://github.com/cpp-linter/cpp-linter-rs/pull/42)

### <!-- 4 --> 🛠️ Fixed

- Propagate errors by @2bndy5 in [#47](https://github.com/cpp-linter/cpp-linter-rs/pull/47)

### <!-- 6 --> 📦 Dependency updates

- Bump the npm group with 2 updates by @dependabot[bot] in [#43](https://github.com/cpp-linter/cpp-linter-rs/pull/43)

### <!-- 8 --> 📝 Documentation

- Release trial follow up by @2bndy5 in [#41](https://github.com/cpp-linter/cpp-linter-rs/pull/41)
- Move logic for release notes generation from Python script to Jinja template (release CI) by @2bndy5 in [#44](https://github.com/cpp-linter/cpp-linter-rs/pull/44)
- Add ReadTheDocs config by @2bndy5 in [#45](https://github.com/cpp-linter/cpp-linter-rs/pull/45)

[2.0.0-rc5]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc4...v2.0.0-rc5

Full commit diff: [`v2.0.0-rc4...v2.0.0-rc5`][2.0.0-rc5]

## [2.0.0-rc2] - 2024-09-21

### <!-- 1 --> 🚀 Added

- Use napi-rs by @2bndy5 in [#39](https://github.com/cpp-linter/cpp-linter-rs/pull/39)

### <!-- 6 --> 📦 Dependency updates

- Bump pypa/gh-action-pypi-publish in the actions group by @dependabot[bot] in [#40](https://github.com/cpp-linter/cpp-linter-rs/pull/40)

[2.0.0-rc2]: https://github.com/cpp-linter/cpp-linter-rs/compare/v2.0.0-rc1...v2.0.0-rc2

Full commit diff: [`v2.0.0-rc1...v2.0.0-rc2`][2.0.0-rc2]

## [2.0.0-rc1] - 2024-09-19

### <!-- 1 --> 🚀 Added

- Support glob patterns by @2bndy5 in [#25](https://github.com/cpp-linter/cpp-linter-rs/pull/25)
- Resort to paginated requests for changed files by @2bndy5 in [#37](https://github.com/cpp-linter/cpp-linter-rs/pull/37)

### <!-- 4 --> 🛠️ Fixed

- Fix links to clang-analyzer diagnostic's help site by @2bndy5 in [#36](https://github.com/cpp-linter/cpp-linter-rs/pull/36)

### <!-- 6 --> 📦 Dependency updates

- Bump openssl from 0.10.62 to 0.10.66 by @dependabot[bot] in [#6](https://github.com/cpp-linter/cpp-linter-rs/pull/6)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#7](https://github.com/cpp-linter/cpp-linter-rs/pull/7)
- Bump the cargo group with 3 updates by @dependabot[bot] in [#15](https://github.com/cpp-linter/cpp-linter-rs/pull/15)
- Bump serde_json from 1.0.125 to 1.0.127 in the cargo group by @dependabot[bot] in [#19](https://github.com/cpp-linter/cpp-linter-rs/pull/19)
- Bump serde from 1.0.208 to 1.0.209 in the cargo group by @dependabot[bot] in [#23](https://github.com/cpp-linter/cpp-linter-rs/pull/23)
- Bump tempfile from 3.9.0 to 3.12.0 in the cargo group by @dependabot[bot] in [#26](https://github.com/cpp-linter/cpp-linter-rs/pull/26)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#34](https://github.com/cpp-linter/cpp-linter-rs/pull/34)

### <!-- 8 --> 📝 Documentation

- Switch to mdbook for docs by @2bndy5 in [#13](https://github.com/cpp-linter/cpp-linter-rs/pull/13)
- Begin documenting permissions by @2bndy5 in [#22](https://github.com/cpp-linter/cpp-linter-rs/pull/22)

[2.0.0-rc1]: https://github.com/cpp-linter/cpp-linter-rs/compare/2e25fec0a447df24d0bcc1b80f6624040bab755e...v2.0.0-rc1

Full commit diff: [`2e25fec...v2.0.0-rc1`][2.0.0-rc1]

## New Contributors

- @2bndy5 made their first contribution
- @dependabot[bot] made their first contribution in [#34](https://github.com/cpp-linter/cpp-linter-rs/pull/34)

<!-- generated by git-cliff -->
