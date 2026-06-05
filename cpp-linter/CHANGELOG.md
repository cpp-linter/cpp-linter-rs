# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [cpp-linter/v2.0.0-rc.16] - 2026-06-05

### <!-- 4 --> 🛠️ Fixed

- Restore cargo binstall support by @2bndy5 in [#328](https://github.com/cpp-linter/cpp-linter-rs/pull/328)

### <!-- 6 --> 📦 Dependency updates

- Bump version to clang-installer/v0.1.2 by @2bndy5 in [`2aea596`](https://github.com/cpp-linter/cpp-linter-rs/commit/2aea596824ca58387860b392e67e180630db258b)

### <!-- 9 --> 🗨️ Changed

- Realign release candidate numbers by @2bndy5 in [`22dd0bb`](https://github.com/cpp-linter/cpp-linter-rs/commit/22dd0bb6bf0493d88ee20fed5acc8ffbf1d32f62)

[cpp-linter/v2.0.0-rc.16]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter/v2.0.0-rc.1...cpp-linter/v2.0.0-rc.16

Full commit diff: [`cpp-linter/v2.0.0-rc.1...cpp-linter/v2.0.0-rc.16`][cpp-linter/v2.0.0-rc.16]

## [cpp-linter/v2.0.0-rc.1] - 2026-06-04

### <!-- 1 --> 🚀 Added

- Add optional colored log output by @2bndy5 in [#52](https://github.com/cpp-linter/cpp-linter-rs/pull/52)
- Capture and output clang tool's version number by @2bndy5 in [#54](https://github.com/cpp-linter/cpp-linter-rs/pull/54)
- Prefix review comments with marker by @2bndy5 in [`9d2a9a3`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d2a9a3e4c4f91ab778959396e4e153d7cbb6d56)
- Rationale to diagnostic comments in PR reviews by @2bndy5 in [`0923c6a`](https://github.com/cpp-linter/cpp-linter-rs/commit/0923c6a1c61617a09fd914a702959204fc41c26d)
- Switch to `quick_xml` library by @2bndy5 in [#101](https://github.com/cpp-linter/cpp-linter-rs/pull/101)
- Delegate vendoring of OpenSSL to git2 dependency tree by @2bndy5 in [#200](https://github.com/cpp-linter/cpp-linter-rs/pull/200)
- Improve CLI value parsing/docs by @2bndy5 in [#208](https://github.com/cpp-linter/cpp-linter-rs/pull/208)
- Upgrade to rust edition 2024 by @2bndy5 in [#228](https://github.com/cpp-linter/cpp-linter-rs/pull/228)
- Optimize use of `ClangParams` struct by @2bndy5 in [#231](https://github.com/cpp-linter/cpp-linter-rs/pull/231)
- Start phasing out `.unwrap()` calls by @2bndy5 in [#242](https://github.com/cpp-linter/cpp-linter-rs/pull/242)
- Allow specifying the base commit for local (non-CI) diffs by @2bndy5 in [#260](https://github.com/cpp-linter/cpp-linter-rs/pull/260)
- Install clang tools on demand by @2bndy5 in [#279](https://github.com/cpp-linter/cpp-linter-rs/pull/279)
- Migrate to git-bot-feedback lib by @2bndy5 in [#304](https://github.com/cpp-linter/cpp-linter-rs/pull/304)

### <!-- 4 --> 🛠️ Fixed

- Propagate errors by @2bndy5 in [#47](https://github.com/cpp-linter/cpp-linter-rs/pull/47)
- Clang tools' version output string in PR review summary by @2bndy5 in [`3333796`](https://github.com/cpp-linter/cpp-linter-rs/commit/33337965a240ff791d39d4e4cd6339855ea42fd8)
- Clang-tidy diagnostic comments in PR review by @2bndy5 in [#77](https://github.com/cpp-linter/cpp-linter-rs/pull/77)
- Parse clang-tidy output when `WarningsAsErrors` is asserted by @2bndy5 in [#190](https://github.com/cpp-linter/cpp-linter-rs/pull/190)
- Properly parse xml with no replacements by @2bndy5 in [#230](https://github.com/cpp-linter/cpp-linter-rs/pull/230)
- Use diagnostic name by default by @2bndy5 in [#236](https://github.com/cpp-linter/cpp-linter-rs/pull/236)
- Skip parsing clang-tidy diagnostic rationale by @2bndy5 in [#237](https://github.com/cpp-linter/cpp-linter-rs/pull/237)

### <!-- 6 --> 📦 Dependency updates

- Bump reqwest from 0.12.7 to 0.12.8 in the cargo group by @dependabot[bot] in [#51](https://github.com/cpp-linter/cpp-linter-rs/pull/51)
- Bump the cargo group across 1 directory with 4 updates by @dependabot[bot] in [#58](https://github.com/cpp-linter/cpp-linter-rs/pull/58)
- Bump the cargo group across 1 directory with 13 updates by @dependabot[bot] in [#73](https://github.com/cpp-linter/cpp-linter-rs/pull/73)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#82](https://github.com/cpp-linter/cpp-linter-rs/pull/82)
- Bump the cargo group across 1 directory with 16 updates by @dependabot[bot] in [#98](https://github.com/cpp-linter/cpp-linter-rs/pull/98)
- Bump the cargo group across 1 directory with 14 updates by @dependabot[bot] in [#116](https://github.com/cpp-linter/cpp-linter-rs/pull/116)
- Bump the cargo group with 7 updates by @dependabot[bot] in [#120](https://github.com/cpp-linter/cpp-linter-rs/pull/120)
- Bump tokio from 1.44.0 to 1.44.2 by @dependabot[bot] in [#128](https://github.com/cpp-linter/cpp-linter-rs/pull/128)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#129](https://github.com/cpp-linter/cpp-linter-rs/pull/129)
- Bump the cargo group across 1 directory with 9 updates by @dependabot[bot] in [#139](https://github.com/cpp-linter/cpp-linter-rs/pull/139)
- Switch to uv and nox by @2bndy5 in [#145](https://github.com/cpp-linter/cpp-linter-rs/pull/145)
- Migrate to napi-rs v3 by @2bndy5 in [#164](https://github.com/cpp-linter/cpp-linter-rs/pull/164)
- Update cargo dependencies by @2bndy5 in [`9663a04`](https://github.com/cpp-linter/cpp-linter-rs/commit/9663a04ffcc83c70d890c052ba7a4176ed3e69f1)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#176](https://github.com/cpp-linter/cpp-linter-rs/pull/176)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#180](https://github.com/cpp-linter/cpp-linter-rs/pull/180)
- Bump the cargo group with 4 updates by @dependabot[bot] in [#185](https://github.com/cpp-linter/cpp-linter-rs/pull/185)
- Bump the cargo group across 1 directory with 11 updates by @dependabot[bot] in [#202](https://github.com/cpp-linter/cpp-linter-rs/pull/202)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#209](https://github.com/cpp-linter/cpp-linter-rs/pull/209)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#223](https://github.com/cpp-linter/cpp-linter-rs/pull/223)
- Bump the cargo group with 8 updates by @dependabot[bot] in [#240](https://github.com/cpp-linter/cpp-linter-rs/pull/240)
- Bump git2 from 0.20.3 to 0.20.4 by @dependabot[bot] in [#258](https://github.com/cpp-linter/cpp-linter-rs/pull/258)
- Bump the cargo group across 1 directory with 12 updates by @dependabot[bot] in [#265](https://github.com/cpp-linter/cpp-linter-rs/pull/265)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#276](https://github.com/cpp-linter/cpp-linter-rs/pull/276)
- Bump version to clang-installer/v0.1.1 by @2bndy5 in [`24ed0a4`](https://github.com/cpp-linter/cpp-linter-rs/commit/24ed0a4556bf5169b8da5c06eef29f5694caa2cd)
- Bump version to cpp-linter/v2.0.0-rc.1 by @2bndy5 in [`2c65f26`](https://github.com/cpp-linter/cpp-linter-rs/commit/2c65f26bc24060e5f80271ef315f54dced30b9cd)

### <!-- 8 --> 📝 Documentation

- [rust API] update logo, favicon, and some links by @2bndy5 in [`31b7add`](https://github.com/cpp-linter/cpp-linter-rs/commit/31b7add5ea8b1938ea4f816f27a732f3ec8d5227)
- Fix typo in doc string comment by @2bndy5 in [`9463247`](https://github.com/cpp-linter/cpp-linter-rs/commit/9463247d5fc127a765243893f54c3745f940094d)

### <!-- 9 --> 🗨️ Changed

- Better Benchmark by @2bndy5 in [#92](https://github.com/cpp-linter/cpp-linter-rs/pull/92)
- Use `Client` instance by reference by @2bndy5 in [#141](https://github.com/cpp-linter/cpp-linter-rs/pull/141)
- Review CI by @2bndy5 in [#195](https://github.com/cpp-linter/cpp-linter-rs/pull/195)
- Use clap derive feature by @2bndy5 in [#204](https://github.com/cpp-linter/cpp-linter-rs/pull/204)
- Adhere to new clippy lint warning by @2bndy5 in [#255](https://github.com/cpp-linter/cpp-linter-rs/pull/255)
- Prepare to release individual packages by @2bndy5 in [#302](https://github.com/cpp-linter/cpp-linter-rs/pull/302)

[cpp-linter/v2.0.0-rc.1]: https://github.com/cpp-linter/cpp-linter-rs/compare/2e25fec0a447df24d0bcc1b80f6624040bab755e...cpp-linter/v2.0.0-rc.1

Full commit diff: [`2e25fec...cpp-linter/v2.0.0-rc.1`][cpp-linter/v2.0.0-rc.1]

<!-- generated by git-cliff -->
