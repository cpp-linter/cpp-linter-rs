# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [clang-tools-manager/v0.3.2] - 2026-10-30

### <!-- 1 --> 🚀 Added

- Print patch to stdout if it exists by @2bndy5 in [#459](https://github.com/cpp-linter/cpp-linter-rs/pull/459)
- Get static binaries from per-version releases by @shenxianpeng in [#510](https://github.com/cpp-linter/cpp-linter-rs/pull/510)

### <!-- 4 --> 🛠️ Fixed

- Updates for latest static binaries by @2bndy5 in [#398](https://github.com/cpp-linter/cpp-linter-rs/pull/398)
- Increase test coverage; fix `--mod-sys`/`--no-mod-sys` flags by @shenxianpeng in [#486](https://github.com/cpp-linter/cpp-linter-rs/pull/486)
- Make `cargo install clang-tools-manager --features bin` work by @shenxianpeng in [#532](https://github.com/cpp-linter/cpp-linter-rs/pull/532)
- Install clang resource headers with downloaded clang-tidy by @shenxianpeng in [#530](https://github.com/cpp-linter/cpp-linter-rs/pull/530)

### <!-- 6 --> 📦 Dependency updates

- Bump the cargo group across 1 directory with 10 updates by @dependabot[bot] in [#443](https://github.com/cpp-linter/cpp-linter-rs/pull/443)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#504](https://github.com/cpp-linter/cpp-linter-rs/pull/504)
- Bump transitive dependencies by @2bndy5 in [`4cb2e2c`](https://github.com/cpp-linter/cpp-linter-rs/commit/4cb2e2c859fe460f0d5a241f5db7f681d5957d7b)

### <!-- 7 -->🚦 Tests

- Cover wheel-tag parsing and clang advice tallying by @shenxianpeng in [#465](https://github.com/cpp-linter/cpp-linter-rs/pull/465)
- Cover platform-specific branches and Windows package managers by @shenxianpeng in [#519](https://github.com/cpp-linter/cpp-linter-rs/pull/519)

[clang-tools-manager/v0.3.2]: https://github.com/cpp-linter/cpp-linter-rs/compare/clang-tools-manager/v0.3.1...clang-tools-manager/v0.3.2

Full commit diff: [`clang-tools-manager/v0.3.1...clang-tools-manager/v0.3.2`][clang-tools-manager/v0.3.2]

## [clang-tools-manager/v0.3.1] - 2026-06-21

### <!-- 6 --> 📦 Dependency updates

- Force use versions.json in docs.rs build by @2bndy5 in [`7d112b9`](https://github.com/cpp-linter/cpp-linter-rs/commit/7d112b9798dbda8e75f4c504915ca31409e308e8)
- Bump version to clang-tools-manager/v0.3.1 by @2bndy5 in [`7cc03b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/7cc03b532394b03ec333de0aed292d72a71d1374)

[clang-tools-manager/v0.3.1]: https://github.com/cpp-linter/cpp-linter-rs/compare/clang-tools-manager/v0.3.0...clang-tools-manager/v0.3.1

Full commit diff: [`clang-tools-manager/v0.3.0...clang-tools-manager/v0.3.1`][clang-tools-manager/v0.3.1]

## [clang-tools-manager/v0.3.0] - 2026-06-21

### <!-- 1 --> 🚀 Added

- Allow explicit use of package managers by @2bndy5 in [#372](https://github.com/cpp-linter/cpp-linter-rs/pull/372)
- Add colors to `--help` output and uniform logging by @2bndy5 in [#374](https://github.com/cpp-linter/cpp-linter-rs/pull/374)

### <!-- 4 --> 🛠️ Fixed

- Refactor for unsupported static binary platforms by @2bndy5 in [#356](https://github.com/cpp-linter/cpp-linter-rs/pull/356)
- Cache the versions.json in build script by @2bndy5 in [#373](https://github.com/cpp-linter/cpp-linter-rs/pull/373)

### <!-- 6 --> 📦 Dependency updates

- Bump version to clang-tools-manager/v0.3.0 by @2bndy5 in [`f7f7b25`](https://github.com/cpp-linter/cpp-linter-rs/commit/f7f7b254f037d5817574494481fd18ca652a42e3)

### <!-- 8 --> 📝 Documentation

- Expand clang-tools-manager/README by @2bndy5 in [#366](https://github.com/cpp-linter/cpp-linter-rs/pull/366)

[clang-tools-manager/v0.3.0]: https://github.com/cpp-linter/cpp-linter-rs/compare/clang-tools-manager/v0.2.1...clang-tools-manager/v0.3.0

Full commit diff: [`clang-tools-manager/v0.2.1...clang-tools-manager/v0.3.0`][clang-tools-manager/v0.3.0]

## [clang-tools-manager/v0.2.1] - 2026-06-10

### <!-- 1 --> 🚀 Added

- Use concrete error types by @2bndy5 in [#343](https://github.com/cpp-linter/cpp-linter-rs/pull/343)

### <!-- 4 --> 🛠️ Fixed

- Do not fail on check for tool presence (via package managers) by @2bndy5 in [#345](https://github.com/cpp-linter/cpp-linter-rs/pull/345)

### <!-- 6 --> 📦 Dependency updates

- Bump version to clang-tools-manager/v0.2.1 by @2bndy5 in [`f4033b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/f4033b5c1807fb3889cbb3c396d0f292bac92d25)

[clang-tools-manager/v0.2.1]: https://github.com/cpp-linter/cpp-linter-rs/compare/clang-tools-manager/v0.2.0...clang-tools-manager/v0.2.1

Full commit diff: [`clang-tools-manager/v0.2.0...clang-tools-manager/v0.2.1`][clang-tools-manager/v0.2.1]

## [clang-tools-manager/v0.2.0] - 2026-06-08

### <!-- 4 --> 🛠️ Fixed

- Rename `clang-installer` to `clang-tools-manager` by @2bndy5 in [#337](https://github.com/cpp-linter/cpp-linter-rs/pull/337)

### <!-- 6 --> 📦 Dependency updates

- Bump version to clang-tools-manager/v0.2.0 by @2bndy5 in [`2505553`](https://github.com/cpp-linter/cpp-linter-rs/commit/25055539eed2ca9f5aca7c65085b826787b52621)

[clang-tools-manager/v0.2.0]: https://github.com/cpp-linter/cpp-linter-rs/compare/clang-tools-manager/v0.1.2...clang-tools-manager/v0.2.0

Full commit diff: [`clang-tools-manager/v0.1.2...clang-tools-manager/v0.2.0`][clang-tools-manager/v0.2.0]

<!-- generated by git-cliff -->
