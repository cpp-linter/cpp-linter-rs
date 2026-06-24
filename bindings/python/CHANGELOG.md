# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [cpp-linter-py/v2.0.0-rc.20] - 2026-06-24

### <!-- 4 --> 🛠️ Fixed

- Pass `--extra-arg`s to clang-tidy properly by @2bndy5 in [#386](https://github.com/cpp-linter/cpp-linter-rs/pull/386)
- Make repo-root path absolute by @2bndy5 in [#387](https://github.com/cpp-linter/cpp-linter-rs/pull/387)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter-js/v2.0.0-rc.19 by @2bndy5 in [`7949a3c`](https://github.com/cpp-linter/cpp-linter-rs/commit/7949a3c62fce83be53721bc507a5ce630afdcb1d)
- Bump the cargo group with 3 updates by @dependabot[bot] in [#381](https://github.com/cpp-linter/cpp-linter-rs/pull/381)
- Bump version to cpp-linter/v2.0.0-rc.21 by @2bndy5 in [`9a9fe54`](https://github.com/cpp-linter/cpp-linter-rs/commit/9a9fe5432bdc279c0cd9f8ccce390caafcb27a34)

[cpp-linter-py/v2.0.0-rc.20]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-py/v2.0.0-rc.19...cpp-linter-py/v2.0.0-rc.20

Full commit diff: [`cpp-linter-py/v2.0.0-rc.19...cpp-linter-py/v2.0.0-rc.20`][cpp-linter-py/v2.0.0-rc.20]

## [cpp-linter-py/v2.0.0-rc.19] - 2026-06-21

### <!-- 1 --> 🚀 Added

- Use diff instead of clang-format XML output by @2bndy5 in [#347](https://github.com/cpp-linter/cpp-linter-rs/pull/347)
- Use stricter linting rules by @2bndy5 in [#353](https://github.com/cpp-linter/cpp-linter-rs/pull/353)
- Get clang-tidy fixes regardless of tidy-review value by @2bndy5 in [#354](https://github.com/cpp-linter/cpp-linter-rs/pull/354)
- Merge review suggestions from both clang tools by @2bndy5 in [#358](https://github.com/cpp-linter/cpp-linter-rs/pull/358)
- Add logs about summarizing PR reviews by @2bndy5 in [#371](https://github.com/cpp-linter/cpp-linter-rs/pull/371)
- Allow explicit use of package managers by @2bndy5 in [#372](https://github.com/cpp-linter/cpp-linter-rs/pull/372)
- Add colors to `--help` output and uniform logging by @2bndy5 in [#374](https://github.com/cpp-linter/cpp-linter-rs/pull/374)
- Add `fix-patch-path` output variable by @2bndy5 in [#378](https://github.com/cpp-linter/cpp-linter-rs/pull/378)

### <!-- 2 --> 🚫 Deprecated

- Replace `--tidy-review`/`--format-review` with `--pr-review` by @2bndy5 in [#377](https://github.com/cpp-linter/cpp-linter-rs/pull/377)

### <!-- 4 --> 🛠️ Fixed

- Respect verbosity per GitHub Actions env var by @2bndy5 in [#346](https://github.com/cpp-linter/cpp-linter-rs/pull/346)
- Do not change directory to `repo-root` by @2bndy5 in [#349](https://github.com/cpp-linter/cpp-linter-rs/pull/349)
- Refactor for unsupported static binary platforms by @2bndy5 in [#356](https://github.com/cpp-linter/cpp-linter-rs/pull/356)
- Bump git-bot-feedback to v0.6.1 by @2bndy5 in [#363](https://github.com/cpp-linter/cpp-linter-rs/pull/363)
- Prevent PR review summary exceeding 65535 len by @2bndy5 in [#370](https://github.com/cpp-linter/cpp-linter-rs/pull/370)
- Cache the versions.json in build script by @2bndy5 in [#373](https://github.com/cpp-linter/cpp-linter-rs/pull/373)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter-js/v2.0.0-rc.18 by @2bndy5 in [`45cfec0`](https://github.com/cpp-linter/cpp-linter-rs/commit/45cfec016eae56ae5b0442f5cfcef43ce1657247)
- Bump urllib3 from 2.6.3 to 2.7.0 by @dependabot[bot] in [#352](https://github.com/cpp-linter/cpp-linter-rs/pull/352)
- Bump pyo3 from 0.28.3 to 0.29.0 by @dependabot[bot] in [#355](https://github.com/cpp-linter/cpp-linter-rs/pull/355)
- Bump the uv-pip group across 1 directory with 4 updates by @dependabot[bot] in [#357](https://github.com/cpp-linter/cpp-linter-rs/pull/357)
- Bump the cargo group with 2 updates by @dependabot[bot] in [#360](https://github.com/cpp-linter/cpp-linter-rs/pull/360)
- Bump version to clang-tools-manager/v0.3.0 by @2bndy5 in [`f7f7b25`](https://github.com/cpp-linter/cpp-linter-rs/commit/f7f7b254f037d5817574494481fd18ca652a42e3)
- Bump version to cpp-linter/v2.0.0-rc.19 by @2bndy5 in [`8f49650`](https://github.com/cpp-linter/cpp-linter-rs/commit/8f49650b709e0b6dbc347c30d79c1c224aebc798)
- Force use versions.json in docs.rs build by @2bndy5 in [`7d112b9`](https://github.com/cpp-linter/cpp-linter-rs/commit/7d112b9798dbda8e75f4c504915ca31409e308e8)
- Bump version to clang-tools-manager/v0.3.1 by @2bndy5 in [`7cc03b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/7cc03b532394b03ec333de0aed292d72a71d1374)
- Bump version to cpp-linter/v2.0.0-rc.20 by @2bndy5 in [`9eb5f6c`](https://github.com/cpp-linter/cpp-linter-rs/commit/9eb5f6cf686d4278f56fb14a8ee4e4cf3a407164)
- Bump version to cpp-linter-py/v2.0.0-rc.19 by @2bndy5 in [`178b7f4`](https://github.com/cpp-linter/cpp-linter-rs/commit/178b7f425fba50c40221cbeed11f86fcb5233135)

### <!-- 8 --> 📝 Documentation

- Expand clang-tools-manager/README by @2bndy5 in [#366](https://github.com/cpp-linter/cpp-linter-rs/pull/366)

[cpp-linter-py/v2.0.0-rc.19]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-py/v2.0.0-rc.18...cpp-linter-py/v2.0.0-rc.19

Full commit diff: [`cpp-linter-py/v2.0.0-rc.18...cpp-linter-py/v2.0.0-rc.19`][cpp-linter-py/v2.0.0-rc.19]

## [cpp-linter-py/v2.0.0-rc.18] - 2026-06-10

### <!-- 1 --> 🚀 Added

- Use concrete error types by @2bndy5 in [#343](https://github.com/cpp-linter/cpp-linter-rs/pull/343)
- Make diff without libgit2 by @2bndy5 in [#344](https://github.com/cpp-linter/cpp-linter-rs/pull/344)

### <!-- 4 --> 🛠️ Fixed

- Do not fail on check for tool presence (via package managers) by @2bndy5 in [#345](https://github.com/cpp-linter/cpp-linter-rs/pull/345)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter-js/v2.0.0-rc.17 by @2bndy5 in [`1529d72`](https://github.com/cpp-linter/cpp-linter-rs/commit/1529d7214611e45a6876ef20580565b1af3ca491)
- Bump version to clang-tools-manager/v0.2.1 by @2bndy5 in [`f4033b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/f4033b5c1807fb3889cbb3c396d0f292bac92d25)
- Bump version to cpp-linter/v2.0.0-rc.18 by @2bndy5 in [`83c1b4a`](https://github.com/cpp-linter/cpp-linter-rs/commit/83c1b4aa29c8e388372351724c395a6a3bad7769)
- Bump version to cpp-linter-py/v2.0.0-rc.18 by @2bndy5 in [`43220ef`](https://github.com/cpp-linter/cpp-linter-rs/commit/43220ef57786a7ee16e5284cbef95894e7e4b457)

[cpp-linter-py/v2.0.0-rc.18]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-py/v2.0.0-rc.17...cpp-linter-py/v2.0.0-rc.18

Full commit diff: [`cpp-linter-py/v2.0.0-rc.17...cpp-linter-py/v2.0.0-rc.18`][cpp-linter-py/v2.0.0-rc.18]

## [cpp-linter-py/v2.0.0-rc.17] - 2026-06-08

### <!-- 1 --> 🚀 Added

- Set clang version min and max at compile time by @2bndy5 in [#333](https://github.com/cpp-linter/cpp-linter-rs/pull/333)

### <!-- 4 --> 🛠️ Fixed

- Rename temp downloaded file to given cache path by @2bndy5 in [#332](https://github.com/cpp-linter/cpp-linter-rs/pull/332)
- Show used clang tools' version in logs by @2bndy5 in [#336](https://github.com/cpp-linter/cpp-linter-rs/pull/336)
- Rename `clang-installer` to `clang-tools-manager` by @2bndy5 in [#337](https://github.com/cpp-linter/cpp-linter-rs/pull/337)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter-js/v2.0.0-rc.16 by @2bndy5 in [`0ca69c4`](https://github.com/cpp-linter/cpp-linter-rs/commit/0ca69c4aa1031bb079d3ba64ab53ebf056482f6f)
- Bump version to clang-tools-manager/v0.2.0 by @2bndy5 in [`2505553`](https://github.com/cpp-linter/cpp-linter-rs/commit/25055539eed2ca9f5aca7c65085b826787b52621)
- Bump version to cpp-linter/v2.0.0-rc.17 by @2bndy5 in [`9d83a7f`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d83a7fdbf2ecde4bcc324c921a2d2ffa233e520)
- Bump version to cpp-linter-py/v2.0.0-rc.17 by @2bndy5 in [`58640c5`](https://github.com/cpp-linter/cpp-linter-rs/commit/58640c5516fcc462b686d94b90e2cde0f958d0d9)

### <!-- 8 --> 📝 Documentation

- Revise third-party license tables in docs by @2bndy5 in [#329](https://github.com/cpp-linter/cpp-linter-rs/pull/329)

[cpp-linter-py/v2.0.0-rc.17]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-py/v2.0.0-rc.16...cpp-linter-py/v2.0.0-rc.17

Full commit diff: [`cpp-linter-py/v2.0.0-rc.16...cpp-linter-py/v2.0.0-rc.17`][cpp-linter-py/v2.0.0-rc.17]

## [cpp-linter-py/v2.0.0-rc.16] - 2026-06-05

### <!-- 4 --> 🛠️ Fixed

- Restore cargo binstall support by @2bndy5 in [#328](https://github.com/cpp-linter/cpp-linter-rs/pull/328)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter-js/v2.0.0-rc.1 by @2bndy5 in [`86cc773`](https://github.com/cpp-linter/cpp-linter-rs/commit/86cc7739b865fd0bce86d0f265a0e0935ef13437)
- Bump deps in the uv-pip group and drop python v3.9 support by @dependabot[bot] in [#316](https://github.com/cpp-linter/cpp-linter-rs/pull/316)
- Bump version to clang-installer/v0.1.2 by @2bndy5 in [`2aea596`](https://github.com/cpp-linter/cpp-linter-rs/commit/2aea596824ca58387860b392e67e180630db258b)
- Bump version to cpp-linter/v2.0.0-rc.16 by @2bndy5 in [`85d6de5`](https://github.com/cpp-linter/cpp-linter-rs/commit/85d6de5fb3073ae66333d48c3d3275ac07baf02f)
- Bump version to cpp-linter-py/v2.0.0-rc.16 by @2bndy5 in [`8dbd515`](https://github.com/cpp-linter/cpp-linter-rs/commit/8dbd515697b89696e922dc1a65c324a22e6624bd)

### <!-- 9 --> 🗨️ Changed

- Realign release candidate numbers by @2bndy5 in [`22dd0bb`](https://github.com/cpp-linter/cpp-linter-rs/commit/22dd0bb6bf0493d88ee20fed5acc8ffbf1d32f62)

[cpp-linter-py/v2.0.0-rc.16]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-py/v2.0.0-rc.1...cpp-linter-py/v2.0.0-rc.16

Full commit diff: [`cpp-linter-py/v2.0.0-rc.1...cpp-linter-py/v2.0.0-rc.16`][cpp-linter-py/v2.0.0-rc.16]

## [cpp-linter-py/v2.0.0-rc.1] - 2026-06-04

### <!-- 1 --> 🚀 Added

- Support glob patterns by @2bndy5 in [#25](https://github.com/cpp-linter/cpp-linter-rs/pull/25)
- Resort to paginated requests for changed files by @2bndy5 in [#37](https://github.com/cpp-linter/cpp-linter-rs/pull/37)
- Use napi-rs by @2bndy5 in [#39](https://github.com/cpp-linter/cpp-linter-rs/pull/39)
- Add changelog and automate version bump and release workflows by @2bndy5 in [#42](https://github.com/cpp-linter/cpp-linter-rs/pull/42)
- Add optional colored log output by @2bndy5 in [#52](https://github.com/cpp-linter/cpp-linter-rs/pull/52)
- Capture and output clang tool's version number by @2bndy5 in [#54](https://github.com/cpp-linter/cpp-linter-rs/pull/54)
- Prefix review comments with marker by @2bndy5 in [`9d2a9a3`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d2a9a3e4c4f91ab778959396e4e153d7cbb6d56)
- Rationale to diagnostic comments in PR reviews by @2bndy5 in [`0923c6a`](https://github.com/cpp-linter/cpp-linter-rs/commit/0923c6a1c61617a09fd914a702959204fc41c26d)
- Merge pull request #90 from cpp-linter/patch-2 by @shenxianpeng in [#90](https://github.com/cpp-linter/cpp-linter-rs/pull/90)
- Switch to `quick_xml` library by @2bndy5 in [#101](https://github.com/cpp-linter/cpp-linter-rs/pull/101)
- Distribute future-compatible python wheels by @2bndy5 in [#178](https://github.com/cpp-linter/cpp-linter-rs/pull/178)
- Delegate vendoring of OpenSSL to git2 dependency tree by @2bndy5 in [#200](https://github.com/cpp-linter/cpp-linter-rs/pull/200)
- Improve CLI value parsing/docs by @2bndy5 in [#208](https://github.com/cpp-linter/cpp-linter-rs/pull/208)
- Upgrade to rust edition 2024 by @2bndy5 in [#228](https://github.com/cpp-linter/cpp-linter-rs/pull/228)
- Optimize use of `ClangParams` struct by @2bndy5 in [#231](https://github.com/cpp-linter/cpp-linter-rs/pull/231)
- Start phasing out `.unwrap()` calls by @2bndy5 in [#242](https://github.com/cpp-linter/cpp-linter-rs/pull/242)
- Allow specifying the base commit for local (non-CI) diffs by @2bndy5 in [#260](https://github.com/cpp-linter/cpp-linter-rs/pull/260)
- Install clang tools on demand by @2bndy5 in [#279](https://github.com/cpp-linter/cpp-linter-rs/pull/279)
- Migrate to git-bot-feedback lib by @2bndy5 in [#304](https://github.com/cpp-linter/cpp-linter-rs/pull/304)

### <!-- 4 --> 🛠️ Fixed

- Fix links to clang-analyzer diagnostic's help site by @2bndy5 in [#36](https://github.com/cpp-linter/cpp-linter-rs/pull/36)
- Propagate errors by @2bndy5 in [#47](https://github.com/cpp-linter/cpp-linter-rs/pull/47)
- Regenerate TS type definitions by @2bndy5 in [`023c170`](https://github.com/cpp-linter/cpp-linter-rs/commit/023c1705a078b9a7542022deefa228a567d68b67)
- Include type stubs in python source distribution by @2bndy5 in [`7dfcce7`](https://github.com/cpp-linter/cpp-linter-rs/commit/7dfcce72f39412f45208e5586d0a1ef2d0a40207)
- Clang tools' version output string in PR review summary by @2bndy5 in [`3333796`](https://github.com/cpp-linter/cpp-linter-rs/commit/33337965a240ff791d39d4e4cd6339855ea42fd8)
- Clang-tidy diagnostic comments in PR review by @2bndy5 in [#77](https://github.com/cpp-linter/cpp-linter-rs/pull/77)
- Fix generated doc about licenses by @2bndy5 in [#159](https://github.com/cpp-linter/cpp-linter-rs/pull/159)
- Parse clang-tidy output when `WarningsAsErrors` is asserted by @2bndy5 in [#190](https://github.com/cpp-linter/cpp-linter-rs/pull/190)
- Properly parse xml with no replacements by @2bndy5 in [#230](https://github.com/cpp-linter/cpp-linter-rs/pull/230)
- Use diagnostic name by default by @2bndy5 in [#236](https://github.com/cpp-linter/cpp-linter-rs/pull/236)
- Skip parsing clang-tidy diagnostic rationale by @2bndy5 in [#237](https://github.com/cpp-linter/cpp-linter-rs/pull/237)

### <!-- 6 --> 📦 Dependency updates

- Bump openssl from 0.10.62 to 0.10.66 by @dependabot[bot] in [#6](https://github.com/cpp-linter/cpp-linter-rs/pull/6)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#7](https://github.com/cpp-linter/cpp-linter-rs/pull/7)
- Bump the cargo group with 3 updates by @dependabot[bot] in [#15](https://github.com/cpp-linter/cpp-linter-rs/pull/15)
- Bump serde_json from 1.0.125 to 1.0.127 in the cargo group by @dependabot[bot] in [#19](https://github.com/cpp-linter/cpp-linter-rs/pull/19)
- Bump serde from 1.0.208 to 1.0.209 in the cargo group by @dependabot[bot] in [#23](https://github.com/cpp-linter/cpp-linter-rs/pull/23)
- Bump tempfile from 3.9.0 to 3.12.0 in the cargo group by @dependabot[bot] in [#26](https://github.com/cpp-linter/cpp-linter-rs/pull/26)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#34](https://github.com/cpp-linter/cpp-linter-rs/pull/34)
- Bump the npm group with 2 updates by @dependabot[bot] in [#43](https://github.com/cpp-linter/cpp-linter-rs/pull/43)
- Bump reqwest from 0.12.7 to 0.12.8 in the cargo group by @dependabot[bot] in [#51](https://github.com/cpp-linter/cpp-linter-rs/pull/51)
- Bump the cargo group across 1 directory with 4 updates by @dependabot[bot] in [#58](https://github.com/cpp-linter/cpp-linter-rs/pull/58)
- Bump the npm group across 1 directory with 4 updates by @dependabot[bot] in [#71](https://github.com/cpp-linter/cpp-linter-rs/pull/71)
- Bump the cargo group across 1 directory with 13 updates by @dependabot[bot] in [#73](https://github.com/cpp-linter/cpp-linter-rs/pull/73)
- Bump pyo3 from 0.23.1 to 0.23.2 in the cargo group by @dependabot[bot] in [#76](https://github.com/cpp-linter/cpp-linter-rs/pull/76)
- Bump pyo3 from 0.23.2 to 0.23.3 by @dependabot[bot] in [#79](https://github.com/cpp-linter/cpp-linter-rs/pull/79)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#82](https://github.com/cpp-linter/cpp-linter-rs/pull/82)
- Bump the cargo group across 1 directory with 16 updates by @dependabot[bot] in [#98](https://github.com/cpp-linter/cpp-linter-rs/pull/98)
- Bump openssl from 0.10.68 to 0.10.70 by @dependabot[bot] in [#105](https://github.com/cpp-linter/cpp-linter-rs/pull/105)
- Bump the cargo group across 1 directory with 14 updates by @dependabot[bot] in [#116](https://github.com/cpp-linter/cpp-linter-rs/pull/116)
- Bump ring from 0.17.8 to 0.17.13 by @dependabot[bot] in [#119](https://github.com/cpp-linter/cpp-linter-rs/pull/119)
- Bump the cargo group with 7 updates by @dependabot[bot] in [#120](https://github.com/cpp-linter/cpp-linter-rs/pull/120)
- Bump pyo3 from 0.24.0 to 0.24.1 by @dependabot[bot] in [#125](https://github.com/cpp-linter/cpp-linter-rs/pull/125)
- Bump openssl from 0.10.71 to 0.10.72 by @dependabot[bot] in [#127](https://github.com/cpp-linter/cpp-linter-rs/pull/127)
- Bump tokio from 1.44.0 to 1.44.2 by @dependabot[bot] in [#128](https://github.com/cpp-linter/cpp-linter-rs/pull/128)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#129](https://github.com/cpp-linter/cpp-linter-rs/pull/129)
- Bump the cargo group across 1 directory with 9 updates by @dependabot[bot] in [#139](https://github.com/cpp-linter/cpp-linter-rs/pull/139)
- Switch to uv and nox by @2bndy5 in [#145](https://github.com/cpp-linter/cpp-linter-rs/pull/145)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#152](https://github.com/cpp-linter/cpp-linter-rs/pull/152)
- Migrate to napi-rs v3 by @2bndy5 in [#164](https://github.com/cpp-linter/cpp-linter-rs/pull/164)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#165](https://github.com/cpp-linter/cpp-linter-rs/pull/165)
- Update cargo dependencies by @2bndy5 in [`9663a04`](https://github.com/cpp-linter/cpp-linter-rs/commit/9663a04ffcc83c70d890c052ba7a4176ed3e69f1)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#176](https://github.com/cpp-linter/cpp-linter-rs/pull/176)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#180](https://github.com/cpp-linter/cpp-linter-rs/pull/180)
- Bump the cargo group with 4 updates by @dependabot[bot] in [#185](https://github.com/cpp-linter/cpp-linter-rs/pull/185)
- Bump the cargo group across 1 directory with 11 updates by @dependabot[bot] in [#202](https://github.com/cpp-linter/cpp-linter-rs/pull/202)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#209](https://github.com/cpp-linter/cpp-linter-rs/pull/209)
- Bump yarn to v4.11.0 by @2bndy5 in [#213](https://github.com/cpp-linter/cpp-linter-rs/pull/213)
- Bump python dependencies by @2bndy5 in [#214](https://github.com/cpp-linter/cpp-linter-rs/pull/214)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#222](https://github.com/cpp-linter/cpp-linter-rs/pull/222)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#223](https://github.com/cpp-linter/cpp-linter-rs/pull/223)
- Bump the cargo group with 8 updates by @dependabot[bot] in [#240](https://github.com/cpp-linter/cpp-linter-rs/pull/240)
- Update python dependencies by @2bndy5 in [#246](https://github.com/cpp-linter/cpp-linter-rs/pull/246)
- Bump bytes from 1.11.0 to 1.11.1 by @dependabot[bot] in [#257](https://github.com/cpp-linter/cpp-linter-rs/pull/257)
- Bump git2 from 0.20.3 to 0.20.4 by @dependabot[bot] in [#258](https://github.com/cpp-linter/cpp-linter-rs/pull/258)
- Bump the cargo group across 1 directory with 12 updates by @dependabot[bot] in [#265](https://github.com/cpp-linter/cpp-linter-rs/pull/265)
- Bump pyo3 from 0.28.1 to 0.28.2 by @dependabot[bot] in [#267](https://github.com/cpp-linter/cpp-linter-rs/pull/267)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#276](https://github.com/cpp-linter/cpp-linter-rs/pull/276)
- Bump rustls-webpki from 0.103.8 to 0.103.10 by @dependabot[bot] in [#283](https://github.com/cpp-linter/cpp-linter-rs/pull/283)
- Bump pygments from 2.19.2 to 2.20.0 by @dependabot[bot] in [#289](https://github.com/cpp-linter/cpp-linter-rs/pull/289)
- Bump meson from 1.9.1 to 1.10.2 in the uv-pip group across 1 directory by @dependabot[bot] in [#292](https://github.com/cpp-linter/cpp-linter-rs/pull/292)
- Bump quinn-proto from 0.11.13 to 0.11.14 by @dependabot[bot] in [#299](https://github.com/cpp-linter/cpp-linter-rs/pull/299)
- Bump rand from 0.9.2 to 0.9.4 by @dependabot[bot] in [#298](https://github.com/cpp-linter/cpp-linter-rs/pull/298)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#301](https://github.com/cpp-linter/cpp-linter-rs/pull/301)
- Bump openssl from 0.10.75 to 0.10.80 by @dependabot[bot] in [#320](https://github.com/cpp-linter/cpp-linter-rs/pull/320)
- Bump pymdown-extensions from 10.20 to 10.21.3 by @dependabot[bot] in [#321](https://github.com/cpp-linter/cpp-linter-rs/pull/321)
- Bump idna from 3.11 to 3.15 by @dependabot[bot] in [#322](https://github.com/cpp-linter/cpp-linter-rs/pull/322)
- Bump version to clang-installer/v0.1.1 by @2bndy5 in [`24ed0a4`](https://github.com/cpp-linter/cpp-linter-rs/commit/24ed0a4556bf5169b8da5c06eef29f5694caa2cd)
- Bump version to cpp-linter/v2.0.0-rc.1 by @2bndy5 in [`2c65f26`](https://github.com/cpp-linter/cpp-linter-rs/commit/2c65f26bc24060e5f80271ef315f54dced30b9cd)
- Bump version to cpp-linter-py/v2.0.0-rc.1 by @2bndy5 in [`e88c348`](https://github.com/cpp-linter/cpp-linter-rs/commit/e88c348424db535e7ced5f81300df74aed0885f4)

### <!-- 8 --> 📝 Documentation

- Switch to mdbook for docs by @2bndy5 in [#13](https://github.com/cpp-linter/cpp-linter-rs/pull/13)
- Release trial follow up by @2bndy5 in [#41](https://github.com/cpp-linter/cpp-linter-rs/pull/41)
- Move logic for release notes generation from Python script to Jinja template (release CI) by @2bndy5 in [#44](https://github.com/cpp-linter/cpp-linter-rs/pull/44)
- Add ReadTheDocs config by @2bndy5 in [#45](https://github.com/cpp-linter/cpp-linter-rs/pull/45)
- [rust API] update logo, favicon, and some links by @2bndy5 in [`31b7add`](https://github.com/cpp-linter/cpp-linter-rs/commit/31b7add5ea8b1938ea4f816f27a732f3ec8d5227)
- Reorganize LICENSE info by @shenxianpeng in [#89](https://github.com/cpp-linter/cpp-linter-rs/pull/89)
- Fix typo in doc string comment by @2bndy5 in [`9463247`](https://github.com/cpp-linter/cpp-linter-rs/commit/9463247d5fc127a765243893f54c3745f940094d)
- Update contributing guide to reflect changes in workflows by @shenxianpeng in [`38ab160`](https://github.com/cpp-linter/cpp-linter-rs/commit/38ab160f28eca2bd4291b2227ba62afc8c6829b1)

### <!-- 9 --> 🗨️ Changed

- Better Benchmark by @2bndy5 in [#92](https://github.com/cpp-linter/cpp-linter-rs/pull/92)
- Use `Client` instance by reference by @2bndy5 in [#141](https://github.com/cpp-linter/cpp-linter-rs/pull/141)
- Update locked transitive dependencies by @2bndy5 in [`aad12c6`](https://github.com/cpp-linter/cpp-linter-rs/commit/aad12c633a0589f36f8cd5e462ad89e359c24d9c)
- Review CI by @2bndy5 in [#195](https://github.com/cpp-linter/cpp-linter-rs/pull/195)
- Use clap derive feature by @2bndy5 in [#204](https://github.com/cpp-linter/cpp-linter-rs/pull/204)
- Include LICENSE file in source distribution for PyPI by @2bndy5 in [`32e20b3`](https://github.com/cpp-linter/cpp-linter-rs/commit/32e20b39d43908e254534f4a6bf2a67560a315ba)
- Adjust benchmark parameters by @2bndy5 in [#229](https://github.com/cpp-linter/cpp-linter-rs/pull/229)
- Adhere to new clippy lint warning by @2bndy5 in [#255](https://github.com/cpp-linter/cpp-linter-rs/pull/255)
- Prepare to release individual packages by @2bndy5 in [#302](https://github.com/cpp-linter/cpp-linter-rs/pull/302)

[cpp-linter-py/v2.0.0-rc.1]: https://github.com/cpp-linter/cpp-linter-rs/compare/2e25fec0a447df24d0bcc1b80f6624040bab755e...cpp-linter-py/v2.0.0-rc.1

Full commit diff: [`2e25fec...cpp-linter-py/v2.0.0-rc.1`][cpp-linter-py/v2.0.0-rc.1]

<!-- generated by git-cliff -->
