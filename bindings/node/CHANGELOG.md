# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
<!-- markdownlint-disable MD024 -->

## [cpp-linter-js/v2.0.0-rc.22] - 2026-10-30

### <!-- 1 --> 🚀 Added

- Print patch to stdout if it exists by @2bndy5 in [#459](https://github.com/cpp-linter/cpp-linter-rs/pull/459)
- Redesign the comment footer with website, star and sponsor links by @shenxianpeng in [#473](https://github.com/cpp-linter/cpp-linter-rs/pull/473)
- Write clang-tidy diagnostics to a SARIF 2.1.0 file by @shenxianpeng in [#479](https://github.com/cpp-linter/cpp-linter-rs/pull/479)
- Apply clang-tidy fixes to other analyzed files by @2bndy5 in [`cc29a7e`](https://github.com/cpp-linter/cpp-linter-rs/commit/cc29a7e585c575ee12907fce3fc6529d824304dc)
- Get static binaries from per-version releases by @shenxianpeng in [#510](https://github.com/cpp-linter/cpp-linter-rs/pull/510)
- Report clang-tidy fixes that could not be applied to other files by @shenxianpeng in [#508](https://github.com/cpp-linter/cpp-linter-rs/pull/508)
- Add `--fix` as alias to new `--gen-patch/-F` arg by @2bndy5 in [#511](https://github.com/cpp-linter/cpp-linter-rs/pull/511)

### <!-- 4 --> 🛠️ Fixed

- Updates for latest static binaries by @2bndy5 in [#398](https://github.com/cpp-linter/cpp-linter-rs/pull/398)
- Increase test coverage; fix `--mod-sys`/`--no-mod-sys` flags by @shenxianpeng in [#486](https://github.com/cpp-linter/cpp-linter-rs/pull/486)
- Never let clang-tidy modify the worktree by @shenxianpeng in [#488](https://github.com/cpp-linter/cpp-linter-rs/pull/488)
- Shift changed lines past clang-tidy fixes above them by @shenxianpeng in [#491](https://github.com/cpp-linter/cpp-linter-rs/pull/491)
- Close behavior gaps with cpp-linter v1 by @shenxianpeng in [#492](https://github.com/cpp-linter/cpp-linter-rs/pull/492)
- Restore v1's `-t`/`-g` short flags by @2bndy5 in [`d8c28eb`](https://github.com/cpp-linter/cpp-linter-rs/commit/d8c28eb698d6275618ca58f5b21e70afd8aeb0ff)
- Bump git-bot-feedback and resolve file paths correctly by @2bndy5 in [#525](https://github.com/cpp-linter/cpp-linter-rs/pull/525)
- Make `cargo install clang-tools-manager --features bin` work by @shenxianpeng in [#532](https://github.com/cpp-linter/cpp-linter-rs/pull/532)
- Install clang resource headers with downloaded clang-tidy by @shenxianpeng in [#530](https://github.com/cpp-linter/cpp-linter-rs/pull/530)
- Count review suggestions merged into one comment as within the diff by @shenxianpeng in [#531](https://github.com/cpp-linter/cpp-linter-rs/pull/531)

### <!-- 6 --> 📦 Dependency updates

- Bump js-yaml from 3.14.2 to 3.15.0 by @dependabot[bot] in [#397](https://github.com/cpp-linter/cpp-linter-rs/pull/397)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#399](https://github.com/cpp-linter/cpp-linter-rs/pull/399)
- Bump ruff from 0.15.16 to 0.15.20 in the uv-pip group across 1 directory by @dependabot[bot] in [#383](https://github.com/cpp-linter/cpp-linter-rs/pull/383)
- Bump oxlint from 1.69.0 to 1.72.0 in the npm group across 1 directory by @dependabot[bot] in [#391](https://github.com/cpp-linter/cpp-linter-rs/pull/391)
- Bump version to cpp-linter/v2.0.0-rc.23 by @2bndy5 in [`fbdcfa4`](https://github.com/cpp-linter/cpp-linter-rs/commit/fbdcfa468b2705d090b43e9198c4c928d8fcbe2d)
- Bump version to cpp-linter-py/v2.0.0-rc.22 by @2bndy5 in [`9b1adcc`](https://github.com/cpp-linter/cpp-linter-rs/commit/9b1adcc3a429e699f352d09d2a92de08e09a17e3)
- Bump brace-expansion from 5.0.6 to 5.0.7 by @dependabot[bot] in [#408](https://github.com/cpp-linter/cpp-linter-rs/pull/408)
- Bump tar from 7.5.16 to 7.5.20 by @dependabot[bot] in [#409](https://github.com/cpp-linter/cpp-linter-rs/pull/409)
- Bump tar from 7.5.20 to 7.5.22 by @dependabot[bot] in [#415](https://github.com/cpp-linter/cpp-linter-rs/pull/415)
- Bump pymdown-extensions from 10.21.3 to 11.0 by @dependabot[bot] in [#414](https://github.com/cpp-linter/cpp-linter-rs/pull/414)
- Bump brace-expansion from 5.0.7 to 5.0.8 by @dependabot[bot] in [#413](https://github.com/cpp-linter/cpp-linter-rs/pull/413)
- Bump quinn-proto from 0.11.14 to 0.11.16 by @dependabot[bot] in [#412](https://github.com/cpp-linter/cpp-linter-rs/pull/412)
- Bump the uv-pip group across 1 directory with 2 updates by @dependabot[bot] in [#404](https://github.com/cpp-linter/cpp-linter-rs/pull/404)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#403](https://github.com/cpp-linter/cpp-linter-rs/pull/403)
- Bump the cargo group across 1 directory with 11 updates by @dependabot[bot] in [#410](https://github.com/cpp-linter/cpp-linter-rs/pull/410)
- Bump the uv-pip group with 2 updates by @dependabot[bot] in [#419](https://github.com/cpp-linter/cpp-linter-rs/pull/419)
- Bump the npm group with 2 updates by @dependabot[bot] in [#417](https://github.com/cpp-linter/cpp-linter-rs/pull/417)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#418](https://github.com/cpp-linter/cpp-linter-rs/pull/418)
- Bump brace-expansion from 5.0.8 to 5.0.9 by @dependabot[bot] in [#422](https://github.com/cpp-linter/cpp-linter-rs/pull/422)
- Bump pymdown-extensions from 11.0 to 11.0.1 by @dependabot[bot] in [#423](https://github.com/cpp-linter/cpp-linter-rs/pull/423)
- Bump js-yaml from 3.15.0 to 3.15.1 by @dependabot[bot] in [#424](https://github.com/cpp-linter/cpp-linter-rs/pull/424)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#425](https://github.com/cpp-linter/cpp-linter-rs/pull/425)
- Bump ruff from 0.16.0 to 0.16.1 in the uv-pip group by @dependabot[bot] in [#427](https://github.com/cpp-linter/cpp-linter-rs/pull/427)
- Bump the cargo group with 2 updates by @dependabot[bot] in [#430](https://github.com/cpp-linter/cpp-linter-rs/pull/430)
- Bump the cargo group across 1 directory with 10 updates by @dependabot[bot] in [#443](https://github.com/cpp-linter/cpp-linter-rs/pull/443)
- Bump the uv-pip group across 1 directory with 3 updates by @dependabot[bot] in [#435](https://github.com/cpp-linter/cpp-linter-rs/pull/435)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#426](https://github.com/cpp-linter/cpp-linter-rs/pull/426)
- Bump js-yaml from 3.15.1 to 3.15.2 by @dependabot[bot] in [#447](https://github.com/cpp-linter/cpp-linter-rs/pull/447)
- Bump the npm group with 2 updates by @dependabot[bot] in [#448](https://github.com/cpp-linter/cpp-linter-rs/pull/448)
- Bump ruff from 0.16.1 to 0.16.6 in the uv-pip group by @dependabot[bot] in [#449](https://github.com/cpp-linter/cpp-linter-rs/pull/449)
- Bump the cargo group across 1 directory with 5 updates by @dependabot[bot] in [#462](https://github.com/cpp-linter/cpp-linter-rs/pull/462)
- Bump the cargo group with 6 updates by @dependabot[bot] in [#466](https://github.com/cpp-linter/cpp-linter-rs/pull/466)
- Bump brace-expansion from 5.0.9 to 5.0.12 by @dependabot[bot] in [#471](https://github.com/cpp-linter/cpp-linter-rs/pull/471)
- Bump virtualenv from 21.5.0 to 21.7.13 by @dependabot[bot] in [#470](https://github.com/cpp-linter/cpp-linter-rs/pull/470)
- Bump urllib3 from 2.7.0 to 2.8.0 by @dependabot[bot] in [#469](https://github.com/cpp-linter/cpp-linter-rs/pull/469)
- Bump the uv-pip group across 1 directory with 2 updates by @dependabot[bot] in [#467](https://github.com/cpp-linter/cpp-linter-rs/pull/467)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#454](https://github.com/cpp-linter/cpp-linter-rs/pull/454)
- Bump the cargo group with 6 updates by @dependabot[bot] in [#482](https://github.com/cpp-linter/cpp-linter-rs/pull/482)
- Bump mypy from 2.3.1 to 2.4.0 in the uv-pip group by @dependabot[bot] in [#485](https://github.com/cpp-linter/cpp-linter-rs/pull/485)
- Bump the npm group with 2 updates by @dependabot[bot] in [#483](https://github.com/cpp-linter/cpp-linter-rs/pull/483)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#504](https://github.com/cpp-linter/cpp-linter-rs/pull/504)
- Bump ruff from 0.16.9 to 0.17.0 in the uv-pip group by @dependabot[bot] in [#506](https://github.com/cpp-linter/cpp-linter-rs/pull/506)
- Bump the npm group with 2 updates by @dependabot[bot] in [#505](https://github.com/cpp-linter/cpp-linter-rs/pull/505)
- Bump transitive dependencies by @2bndy5 in [`4cb2e2c`](https://github.com/cpp-linter/cpp-linter-rs/commit/4cb2e2c859fe460f0d5a241f5db7f681d5957d7b)
- Bump git-bot-feedback from v0.8.6 to v0.8.8 by @2bndy5 in [#515](https://github.com/cpp-linter/cpp-linter-rs/pull/515)
- Bump certifi from 2026.5.20 to 2026.10.19 by @dependabot[bot] in [#516](https://github.com/cpp-linter/cpp-linter-rs/pull/516)
- Bump virtualenv from 21.7.13 to 21.14.13 by @dependabot[bot] in [#520](https://github.com/cpp-linter/cpp-linter-rs/pull/520)
- Bump ruff from 0.17.0 to 0.17.1 in the uv-pip group by @dependabot[bot] in [#523](https://github.com/cpp-linter/cpp-linter-rs/pull/523)
- Bump the npm group with 2 updates by @dependabot[bot] in [#522](https://github.com/cpp-linter/cpp-linter-rs/pull/522)
- Bump jsonschema from 0.58.6 to 0.59.0 in the cargo group by @dependabot[bot] in [#521](https://github.com/cpp-linter/cpp-linter-rs/pull/521)
- Bump oxlint from 1.88.0 to 1.89.0 in the npm group by @dependabot[bot] in [#527](https://github.com/cpp-linter/cpp-linter-rs/pull/527)
- Bump mkdocs-gen-files from 0.6.0 to 0.6.1 in the uv-pip group by @dependabot[bot] in [#528](https://github.com/cpp-linter/cpp-linter-rs/pull/528)
- Bump version to clang-tools-manager/v0.3.2 by @2bndy5 in [`bd385e5`](https://github.com/cpp-linter/cpp-linter-rs/commit/bd385e5b4d918631fb7d740c600b7884aadc0144)
- Bump version to cpp-linter/v2.0.0-rc.24 by @2bndy5 in [`3ac3fac`](https://github.com/cpp-linter/cpp-linter-rs/commit/3ac3fac75fbe9d0761d0360dc6bb52e566199507)
- Bump version to cpp-linter-py/v2.0.0-rc.23 by @2bndy5 in [`2504be7`](https://github.com/cpp-linter/cpp-linter-rs/commit/2504be7d43b9fcee32521ca881461193a90606ec)

### <!-- 7 -->🚦 Tests

- Cover wheel-tag parsing and clang advice tallying by @shenxianpeng in [#465](https://github.com/cpp-linter/cpp-linter-rs/pull/465)
- Cover platform-specific branches and Windows package managers by @shenxianpeng in [#519](https://github.com/cpp-linter/cpp-linter-rs/pull/519)

### <!-- 8 --> 📝 Documentation

- Rework the READMEs to the org layout and fix the install commands by @shenxianpeng in [#464](https://github.com/cpp-linter/cpp-linter-rs/pull/464)

### <!-- 9 --> 🗨️ Changed

- Reword 'fix available' by @2bndy5 in [#458](https://github.com/cpp-linter/cpp-linter-rs/pull/458)
- Update benchmark script for hyperfine v2 JSON by @Copilot in [#480](https://github.com/cpp-linter/cpp-linter-rs/pull/480)
- Move module imports for unix only tests by @2bndy5 in [#487](https://github.com/cpp-linter/cpp-linter-rs/pull/487)

[cpp-linter-js/v2.0.0-rc.22]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.21...cpp-linter-js/v2.0.0-rc.22

Full commit diff: [`cpp-linter-js/v2.0.0-rc.21...cpp-linter-js/v2.0.0-rc.22`][cpp-linter-js/v2.0.0-rc.22]

## New Contributors

- @Copilot made their first contribution in [#480](https://github.com/cpp-linter/cpp-linter-rs/pull/480)

## [cpp-linter-js/v2.0.0-rc.21] - 2026-06-28

### <!-- 1 --> 🚀 Added

- Add option for custom path to summary comment output by @2bndy5 in [#388](https://github.com/cpp-linter/cpp-linter-rs/pull/388)

### <!-- 4 --> 🛠️ Fixed

- Use correct output variable names by @2bndy5 in [`7ad731f`](https://github.com/cpp-linter/cpp-linter-rs/commit/7ad731fb5e81fbcd455713c9d2b41de7e78e32e8)
- Use diff line index as 1-based line numbers by @2bndy5 in [#389](https://github.com/cpp-linter/cpp-linter-rs/pull/389)
- Add context lines to generated patch by @2bndy5 in [#390](https://github.com/cpp-linter/cpp-linter-rs/pull/390)

### <!-- 6 --> 📦 Dependency updates

- Bump version to cpp-linter/v2.0.0-rc.22 by @2bndy5 in [`3f3647c`](https://github.com/cpp-linter/cpp-linter-rs/commit/3f3647c8860946dfc4e5a1cdf112822692136e7e)
- Bump version to cpp-linter-py/v2.0.0-rc.21 by @2bndy5 in [`1808e4a`](https://github.com/cpp-linter/cpp-linter-rs/commit/1808e4a67b9463eb3b338da7fd2906197ae72c2c)
- Bump version to cpp-linter-js/v2.0.0-rc.21 by @2bndy5 in [`115cdcf`](https://github.com/cpp-linter/cpp-linter-rs/commit/115cdcf560be209f8f3130720f15915852c9477d)

[cpp-linter-js/v2.0.0-rc.21]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.20...cpp-linter-js/v2.0.0-rc.21

Full commit diff: [`cpp-linter-js/v2.0.0-rc.20...cpp-linter-js/v2.0.0-rc.21`][cpp-linter-js/v2.0.0-rc.21]

## [cpp-linter-js/v2.0.0-rc.20] - 2026-06-24

### <!-- 4 --> 🛠️ Fixed

- Pass `--extra-arg`s to clang-tidy properly by @2bndy5 in [#386](https://github.com/cpp-linter/cpp-linter-rs/pull/386)
- Make repo-root path absolute by @2bndy5 in [#387](https://github.com/cpp-linter/cpp-linter-rs/pull/387)

### <!-- 6 --> 📦 Dependency updates

- Bump the cargo group with 3 updates by @dependabot[bot] in [#381](https://github.com/cpp-linter/cpp-linter-rs/pull/381)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#382](https://github.com/cpp-linter/cpp-linter-rs/pull/382)
- Bump version to cpp-linter/v2.0.0-rc.21 by @2bndy5 in [`9a9fe54`](https://github.com/cpp-linter/cpp-linter-rs/commit/9a9fe5432bdc279c0cd9f8ccce390caafcb27a34)
- Bump version to cpp-linter-py/v2.0.0-rc.20 by @2bndy5 in [`bac1838`](https://github.com/cpp-linter/cpp-linter-rs/commit/bac1838924bb614bf2d02d27792d1d976942dd3f)
- Bump version to cpp-linter-js/v2.0.0-rc.20 by @2bndy5 in [`2489719`](https://github.com/cpp-linter/cpp-linter-rs/commit/2489719d03db7c4cbf2100cc6962e5fb9b2b639c)

[cpp-linter-js/v2.0.0-rc.20]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.19...cpp-linter-js/v2.0.0-rc.20

Full commit diff: [`cpp-linter-js/v2.0.0-rc.19...cpp-linter-js/v2.0.0-rc.20`][cpp-linter-js/v2.0.0-rc.20]

## [cpp-linter-js/v2.0.0-rc.19] - 2026-06-21

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

- Bump urllib3 from 2.6.3 to 2.7.0 by @dependabot[bot] in [#352](https://github.com/cpp-linter/cpp-linter-rs/pull/352)
- Bump pyo3 from 0.28.3 to 0.29.0 by @dependabot[bot] in [#355](https://github.com/cpp-linter/cpp-linter-rs/pull/355)
- Bump the uv-pip group across 1 directory with 4 updates by @dependabot[bot] in [#357](https://github.com/cpp-linter/cpp-linter-rs/pull/357)
- Bump the cargo group with 2 updates by @dependabot[bot] in [#360](https://github.com/cpp-linter/cpp-linter-rs/pull/360)
- Bump tar from 7.5.11 to 7.5.16 by @dependabot[bot] in [#369](https://github.com/cpp-linter/cpp-linter-rs/pull/369)
- Bump version to clang-tools-manager/v0.3.0 by @2bndy5 in [`f7f7b25`](https://github.com/cpp-linter/cpp-linter-rs/commit/f7f7b254f037d5817574494481fd18ca652a42e3)
- Bump version to cpp-linter/v2.0.0-rc.19 by @2bndy5 in [`8f49650`](https://github.com/cpp-linter/cpp-linter-rs/commit/8f49650b709e0b6dbc347c30d79c1c224aebc798)
- Force use versions.json in docs.rs build by @2bndy5 in [`7d112b9`](https://github.com/cpp-linter/cpp-linter-rs/commit/7d112b9798dbda8e75f4c504915ca31409e308e8)
- Bump version to clang-tools-manager/v0.3.1 by @2bndy5 in [`7cc03b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/7cc03b532394b03ec333de0aed292d72a71d1374)
- Bump version to cpp-linter/v2.0.0-rc.20 by @2bndy5 in [`9eb5f6c`](https://github.com/cpp-linter/cpp-linter-rs/commit/9eb5f6cf686d4278f56fb14a8ee4e4cf3a407164)
- Bump version to cpp-linter-py/v2.0.0-rc.19 by @2bndy5 in [`178b7f4`](https://github.com/cpp-linter/cpp-linter-rs/commit/178b7f425fba50c40221cbeed11f86fcb5233135)
- Bump version to cpp-linter-js/v2.0.0-rc.19 by @2bndy5 in [`7949a3c`](https://github.com/cpp-linter/cpp-linter-rs/commit/7949a3c62fce83be53721bc507a5ce630afdcb1d)

### <!-- 8 --> 📝 Documentation

- Expand clang-tools-manager/README by @2bndy5 in [#366](https://github.com/cpp-linter/cpp-linter-rs/pull/366)

[cpp-linter-js/v2.0.0-rc.19]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.18...cpp-linter-js/v2.0.0-rc.19

Full commit diff: [`cpp-linter-js/v2.0.0-rc.18...cpp-linter-js/v2.0.0-rc.19`][cpp-linter-js/v2.0.0-rc.19]

## [cpp-linter-js/v2.0.0-rc.18] - 2026-06-10

### <!-- 1 --> 🚀 Added

- Use concrete error types by @2bndy5 in [#343](https://github.com/cpp-linter/cpp-linter-rs/pull/343)
- Make diff without libgit2 by @2bndy5 in [#344](https://github.com/cpp-linter/cpp-linter-rs/pull/344)

### <!-- 4 --> 🛠️ Fixed

- Do not fail on check for tool presence (via package managers) by @2bndy5 in [#345](https://github.com/cpp-linter/cpp-linter-rs/pull/345)

### <!-- 6 --> 📦 Dependency updates

- Bump version to clang-tools-manager/v0.2.1 by @2bndy5 in [`f4033b5`](https://github.com/cpp-linter/cpp-linter-rs/commit/f4033b5c1807fb3889cbb3c396d0f292bac92d25)
- Bump version to cpp-linter/v2.0.0-rc.18 by @2bndy5 in [`83c1b4a`](https://github.com/cpp-linter/cpp-linter-rs/commit/83c1b4aa29c8e388372351724c395a6a3bad7769)
- Bump version to cpp-linter-py/v2.0.0-rc.18 by @2bndy5 in [`43220ef`](https://github.com/cpp-linter/cpp-linter-rs/commit/43220ef57786a7ee16e5284cbef95894e7e4b457)
- Bump version to cpp-linter-js/v2.0.0-rc.18 by @2bndy5 in [`45cfec0`](https://github.com/cpp-linter/cpp-linter-rs/commit/45cfec016eae56ae5b0442f5cfcef43ce1657247)

[cpp-linter-js/v2.0.0-rc.18]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.17...cpp-linter-js/v2.0.0-rc.18

Full commit diff: [`cpp-linter-js/v2.0.0-rc.17...cpp-linter-js/v2.0.0-rc.18`][cpp-linter-js/v2.0.0-rc.18]

## [cpp-linter-js/v2.0.0-rc.17] - 2026-06-08

### <!-- 1 --> 🚀 Added

- Set clang version min and max at compile time by @2bndy5 in [#333](https://github.com/cpp-linter/cpp-linter-rs/pull/333)

### <!-- 4 --> 🛠️ Fixed

- Rename temp downloaded file to given cache path by @2bndy5 in [#332](https://github.com/cpp-linter/cpp-linter-rs/pull/332)
- Show used clang tools' version in logs by @2bndy5 in [#336](https://github.com/cpp-linter/cpp-linter-rs/pull/336)
- Rename `clang-installer` to `clang-tools-manager` by @2bndy5 in [#337](https://github.com/cpp-linter/cpp-linter-rs/pull/337)

### <!-- 6 --> 📦 Dependency updates

- Bump the npm group with 2 updates by @dependabot[bot] in [#338](https://github.com/cpp-linter/cpp-linter-rs/pull/338)
- Bump version to clang-tools-manager/v0.2.0 by @2bndy5 in [`2505553`](https://github.com/cpp-linter/cpp-linter-rs/commit/25055539eed2ca9f5aca7c65085b826787b52621)
- Bump version to cpp-linter/v2.0.0-rc.17 by @2bndy5 in [`9d83a7f`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d83a7fdbf2ecde4bcc324c921a2d2ffa233e520)
- Bump version to cpp-linter-py/v2.0.0-rc.17 by @2bndy5 in [`58640c5`](https://github.com/cpp-linter/cpp-linter-rs/commit/58640c5516fcc462b686d94b90e2cde0f958d0d9)
- Bump version to cpp-linter-js/v2.0.0-rc.17 by @2bndy5 in [`1529d72`](https://github.com/cpp-linter/cpp-linter-rs/commit/1529d7214611e45a6876ef20580565b1af3ca491)

### <!-- 8 --> 📝 Documentation

- Revise third-party license tables in docs by @2bndy5 in [#329](https://github.com/cpp-linter/cpp-linter-rs/pull/329)

[cpp-linter-js/v2.0.0-rc.17]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.16...cpp-linter-js/v2.0.0-rc.17

Full commit diff: [`cpp-linter-js/v2.0.0-rc.16...cpp-linter-js/v2.0.0-rc.17`][cpp-linter-js/v2.0.0-rc.17]

## [cpp-linter-js/v2.0.0-rc.16] - 2026-06-05

### <!-- 4 --> 🛠️ Fixed

- Restore cargo binstall support by @2bndy5 in [#328](https://github.com/cpp-linter/cpp-linter-rs/pull/328)

### <!-- 6 --> 📦 Dependency updates

- Bump deps in the uv-pip group and drop python v3.9 support by @dependabot[bot] in [#316](https://github.com/cpp-linter/cpp-linter-rs/pull/316)
- Bump the npm group across 1 directory with 3 updates by @dependabot[bot] in [#315](https://github.com/cpp-linter/cpp-linter-rs/pull/315)
- Bump version to clang-installer/v0.1.2 by @2bndy5 in [`2aea596`](https://github.com/cpp-linter/cpp-linter-rs/commit/2aea596824ca58387860b392e67e180630db258b)
- Bump version to cpp-linter/v2.0.0-rc.16 by @2bndy5 in [`85d6de5`](https://github.com/cpp-linter/cpp-linter-rs/commit/85d6de5fb3073ae66333d48c3d3275ac07baf02f)
- Bump version to cpp-linter-py/v2.0.0-rc.16 by @2bndy5 in [`8dbd515`](https://github.com/cpp-linter/cpp-linter-rs/commit/8dbd515697b89696e922dc1a65c324a22e6624bd)
- Bump version to cpp-linter-js/v2.0.0-rc.16 by @2bndy5 in [`0ca69c4`](https://github.com/cpp-linter/cpp-linter-rs/commit/0ca69c4aa1031bb079d3ba64ab53ebf056482f6f)

### <!-- 9 --> 🗨️ Changed

- Realign release candidate numbers by @2bndy5 in [`22dd0bb`](https://github.com/cpp-linter/cpp-linter-rs/commit/22dd0bb6bf0493d88ee20fed5acc8ffbf1d32f62)

[cpp-linter-js/v2.0.0-rc.16]: https://github.com/cpp-linter/cpp-linter-rs/compare/cpp-linter-js/v2.0.0-rc.1...cpp-linter-js/v2.0.0-rc.16

Full commit diff: [`cpp-linter-js/v2.0.0-rc.1...cpp-linter-js/v2.0.0-rc.16`][cpp-linter-js/v2.0.0-rc.16]

## [cpp-linter-js/v2.0.0-rc.1] - 2026-06-04

### <!-- 1 --> 🚀 Added

- Add more testing and various improvements by @2bndy5 in [#4](https://github.com/cpp-linter/cpp-linter-rs/pull/4)
- Support glob patterns by @2bndy5 in [#25](https://github.com/cpp-linter/cpp-linter-rs/pull/25)
- Resort to paginated requests for changed files by @2bndy5 in [#37](https://github.com/cpp-linter/cpp-linter-rs/pull/37)
- Use napi-rs by @2bndy5 in [#39](https://github.com/cpp-linter/cpp-linter-rs/pull/39)
- Add changelog and automate version bump and release workflows by @2bndy5 in [#42](https://github.com/cpp-linter/cpp-linter-rs/pull/42)
- Add optional colored log output by @2bndy5 in [#52](https://github.com/cpp-linter/cpp-linter-rs/pull/52)
- Capture and output clang tool's version number by @2bndy5 in [#54](https://github.com/cpp-linter/cpp-linter-rs/pull/54)
- Prefix review comments with marker by @2bndy5 in [`9d2a9a3`](https://github.com/cpp-linter/cpp-linter-rs/commit/9d2a9a3e4c4f91ab778959396e4e153d7cbb6d56)
- Rationale to diagnostic comments in PR reviews by @2bndy5 in [`0923c6a`](https://github.com/cpp-linter/cpp-linter-rs/commit/0923c6a1c61617a09fd914a702959204fc41c26d)
- Add gitlens to vscode and update repo link by @shenxianpeng in [`26c3b2d`](https://github.com/cpp-linter/cpp-linter-rs/commit/26c3b2d6d06e111449c1ac7e8bd2d26675709098)
- Merge pull request #90 from cpp-linter/patch-2 by @shenxianpeng in [#90](https://github.com/cpp-linter/cpp-linter-rs/pull/90)
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

- Fix parsing of `--extra-arg` by @2bndy5 in [`03f3de5`](https://github.com/cpp-linter/cpp-linter-rs/commit/03f3de5232e29446d57de00d8ac6deb2fc17d9a5)
- Fix GithubApiClient init for non-PR events by @2bndy5 in [`5b60ab8`](https://github.com/cpp-linter/cpp-linter-rs/commit/5b60ab8af020f81fc986cdf86568263b5e5f8e50)
- Fix links to clang-analyzer diagnostic's help site by @2bndy5 in [#36](https://github.com/cpp-linter/cpp-linter-rs/pull/36)
- Fix metadata and switch to pypa-publish action by @2bndy5 in [`092e0c2`](https://github.com/cpp-linter/cpp-linter-rs/commit/092e0c20cf66747b59bab4bdf60be29f7f02dcc6)
- Fix metadata in platform-specific node pkgs by @2bndy5 in [`1dbb1de`](https://github.com/cpp-linter/cpp-linter-rs/commit/1dbb1de3abdb231646a33ac2721e6a8778ca4ece)
- Fix typo in node-binding/README by @2bndy5 in [`7732676`](https://github.com/cpp-linter/cpp-linter-rs/commit/7732676e03941a37a4fb5b474d319c640689985a)
- Propagate errors by @2bndy5 in [#47](https://github.com/cpp-linter/cpp-linter-rs/pull/47)
- Regenerate TS type definitions by @2bndy5 in [`023c170`](https://github.com/cpp-linter/cpp-linter-rs/commit/023c1705a078b9a7542022deefa228a567d68b67)
- Fix python binding's entry point by @2bndy5 in [`bba2e68`](https://github.com/cpp-linter/cpp-linter-rs/commit/bba2e68f43fa2ee652cc67a18ea2cd1cfbfe537a)
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
- Bump @eslint/plugin-kit from 0.2.0 to 0.2.3 by @dependabot[bot] in [#69](https://github.com/cpp-linter/cpp-linter-rs/pull/69)
- Bump cross-spawn from 7.0.3 to 7.0.5 by @dependabot[bot] in [#70](https://github.com/cpp-linter/cpp-linter-rs/pull/70)
- Bump the npm group across 1 directory with 4 updates by @dependabot[bot] in [#71](https://github.com/cpp-linter/cpp-linter-rs/pull/71)
- Bump the cargo group across 1 directory with 13 updates by @dependabot[bot] in [#73](https://github.com/cpp-linter/cpp-linter-rs/pull/73)
- Bump pyo3 from 0.23.1 to 0.23.2 in the cargo group by @dependabot[bot] in [#76](https://github.com/cpp-linter/cpp-linter-rs/pull/76)
- Bump pyo3 from 0.23.2 to 0.23.3 by @dependabot[bot] in [#79](https://github.com/cpp-linter/cpp-linter-rs/pull/79)
- Bump the npm group with 3 updates by @dependabot[bot] in [#78](https://github.com/cpp-linter/cpp-linter-rs/pull/78)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#82](https://github.com/cpp-linter/cpp-linter-rs/pull/82)
- Bump the npm group across 1 directory with 3 updates by @dependabot[bot] in [#96](https://github.com/cpp-linter/cpp-linter-rs/pull/96)
- Bump the cargo group across 1 directory with 16 updates by @dependabot[bot] in [#98](https://github.com/cpp-linter/cpp-linter-rs/pull/98)
- Bump openssl from 0.10.68 to 0.10.70 by @dependabot[bot] in [#105](https://github.com/cpp-linter/cpp-linter-rs/pull/105)
- Bump the cargo group across 1 directory with 14 updates by @dependabot[bot] in [#116](https://github.com/cpp-linter/cpp-linter-rs/pull/116)
- Bump ring from 0.17.8 to 0.17.13 by @dependabot[bot] in [#119](https://github.com/cpp-linter/cpp-linter-rs/pull/119)
- Bump the cargo group with 7 updates by @dependabot[bot] in [#120](https://github.com/cpp-linter/cpp-linter-rs/pull/120)
- Bump pyo3 from 0.24.0 to 0.24.1 by @dependabot[bot] in [#125](https://github.com/cpp-linter/cpp-linter-rs/pull/125)
- Bump openssl from 0.10.71 to 0.10.72 by @dependabot[bot] in [#127](https://github.com/cpp-linter/cpp-linter-rs/pull/127)
- Bump the npm group across 1 directory with 3 updates by @dependabot[bot] in [#118](https://github.com/cpp-linter/cpp-linter-rs/pull/118)
- Bump tokio from 1.44.0 to 1.44.2 by @dependabot[bot] in [#128](https://github.com/cpp-linter/cpp-linter-rs/pull/128)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#129](https://github.com/cpp-linter/cpp-linter-rs/pull/129)
- Bump the cargo group across 1 directory with 9 updates by @dependabot[bot] in [#139](https://github.com/cpp-linter/cpp-linter-rs/pull/139)
- Bump the npm group across 1 directory with 4 updates by @dependabot[bot] in [#137](https://github.com/cpp-linter/cpp-linter-rs/pull/137)
- Switch to uv and nox by @2bndy5 in [#145](https://github.com/cpp-linter/cpp-linter-rs/pull/145)
- Bump @eslint/js in the npm group across 1 directory by @dependabot[bot] in [#151](https://github.com/cpp-linter/cpp-linter-rs/pull/151)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#152](https://github.com/cpp-linter/cpp-linter-rs/pull/152)
- Bump @eslint/plugin-kit from 0.3.2 to 0.3.3 by @dependabot[bot] in [#162](https://github.com/cpp-linter/cpp-linter-rs/pull/162)
- Migrate to napi-rs v3 by @2bndy5 in [#164](https://github.com/cpp-linter/cpp-linter-rs/pull/164)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#166](https://github.com/cpp-linter/cpp-linter-rs/pull/166)
- Bump the cargo group across 1 directory with 2 updates by @dependabot[bot] in [#165](https://github.com/cpp-linter/cpp-linter-rs/pull/165)
- Update cargo dependencies by @2bndy5 in [`9663a04`](https://github.com/cpp-linter/cpp-linter-rs/commit/9663a04ffcc83c70d890c052ba7a4176ed3e69f1)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#176](https://github.com/cpp-linter/cpp-linter-rs/pull/176)
- Bump the npm group with 2 updates by @dependabot[bot] in [#179](https://github.com/cpp-linter/cpp-linter-rs/pull/179)
- Bump the cargo group with 5 updates by @dependabot[bot] in [#180](https://github.com/cpp-linter/cpp-linter-rs/pull/180)
- Bump the npm group with 2 updates by @dependabot[bot] in [#186](https://github.com/cpp-linter/cpp-linter-rs/pull/186)
- Bump the cargo group with 4 updates by @dependabot[bot] in [#185](https://github.com/cpp-linter/cpp-linter-rs/pull/185)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#191](https://github.com/cpp-linter/cpp-linter-rs/pull/191)
- Bump the cargo group across 1 directory with 11 updates by @dependabot[bot] in [#202](https://github.com/cpp-linter/cpp-linter-rs/pull/202)
- Bump the cargo group across 1 directory with 6 updates by @dependabot[bot] in [#209](https://github.com/cpp-linter/cpp-linter-rs/pull/209)
- Bump yarn to v4.11.0 by @2bndy5 in [#213](https://github.com/cpp-linter/cpp-linter-rs/pull/213)
- Bump python dependencies by @2bndy5 in [#214](https://github.com/cpp-linter/cpp-linter-rs/pull/214)
- Bump oxlint from 1.24.0 to 1.28.0 in the npm group by @dependabot[bot] in [#215](https://github.com/cpp-linter/cpp-linter-rs/pull/215)
- Bump js-yaml from 3.14.1 to 3.14.2 by @dependabot[bot] in [#219](https://github.com/cpp-linter/cpp-linter-rs/pull/219)
- Bump glob from 10.4.5 to 10.5.0 by @dependabot[bot] in [#220](https://github.com/cpp-linter/cpp-linter-rs/pull/220)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#222](https://github.com/cpp-linter/cpp-linter-rs/pull/222)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#223](https://github.com/cpp-linter/cpp-linter-rs/pull/223)
- Bump the cargo group with 8 updates by @dependabot[bot] in [#240](https://github.com/cpp-linter/cpp-linter-rs/pull/240)
- Bump tar from 7.4.3 to 7.5.3 by @dependabot[bot] in [#245](https://github.com/cpp-linter/cpp-linter-rs/pull/245)
- Update python dependencies by @2bndy5 in [#246](https://github.com/cpp-linter/cpp-linter-rs/pull/246)
- Bump tar from 7.5.3 to 7.5.6 by @dependabot[bot] in [#250](https://github.com/cpp-linter/cpp-linter-rs/pull/250)
- Bump tar from 7.5.6 to 7.5.7 by @dependabot[bot] in [#254](https://github.com/cpp-linter/cpp-linter-rs/pull/254)
- Bump lodash from 4.17.21 to 4.17.23 by @dependabot[bot] in [#251](https://github.com/cpp-linter/cpp-linter-rs/pull/251)
- Bump bytes from 1.11.0 to 1.11.1 by @dependabot[bot] in [#257](https://github.com/cpp-linter/cpp-linter-rs/pull/257)
- Bump git2 from 0.20.3 to 0.20.4 by @dependabot[bot] in [#258](https://github.com/cpp-linter/cpp-linter-rs/pull/258)
- Bump the cargo group across 1 directory with 12 updates by @dependabot[bot] in [#265](https://github.com/cpp-linter/cpp-linter-rs/pull/265)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#241](https://github.com/cpp-linter/cpp-linter-rs/pull/241)
- Bump tar from 7.5.7 to 7.5.9 by @dependabot[bot] in [#266](https://github.com/cpp-linter/cpp-linter-rs/pull/266)
- Bump pyo3 from 0.28.1 to 0.28.2 by @dependabot[bot] in [#267](https://github.com/cpp-linter/cpp-linter-rs/pull/267)
- Bump minimatch from 9.0.5 to 9.0.9 by @dependabot[bot] in [#272](https://github.com/cpp-linter/cpp-linter-rs/pull/272)
- Bump the npm group across 1 directory with 2 updates by @dependabot[bot] in [#275](https://github.com/cpp-linter/cpp-linter-rs/pull/275)
- Bump tar from 7.5.9 to 7.5.10 by @dependabot[bot] in [#274](https://github.com/cpp-linter/cpp-linter-rs/pull/274)
- Bump the cargo group across 1 directory with 7 updates by @dependabot[bot] in [#276](https://github.com/cpp-linter/cpp-linter-rs/pull/276)
- Bump tar from 7.5.10 to 7.5.11 by @dependabot[bot] in [#280](https://github.com/cpp-linter/cpp-linter-rs/pull/280)
- Bump rustls-webpki from 0.103.8 to 0.103.10 by @dependabot[bot] in [#283](https://github.com/cpp-linter/cpp-linter-rs/pull/283)
- Bump picomatch from 2.3.1 to 2.3.2 by @dependabot[bot] in [#286](https://github.com/cpp-linter/cpp-linter-rs/pull/286)
- Bump brace-expansion from 5.0.4 to 5.0.5 by @dependabot[bot] in [#287](https://github.com/cpp-linter/cpp-linter-rs/pull/287)
- Bump pygments from 2.19.2 to 2.20.0 by @dependabot[bot] in [#289](https://github.com/cpp-linter/cpp-linter-rs/pull/289)
- Bump lodash from 4.17.23 to 4.18.1 by @dependabot[bot] in [#295](https://github.com/cpp-linter/cpp-linter-rs/pull/295)
- Bump meson from 1.9.1 to 1.10.2 in the uv-pip group across 1 directory by @dependabot[bot] in [#292](https://github.com/cpp-linter/cpp-linter-rs/pull/292)
- Bump quinn-proto from 0.11.13 to 0.11.14 by @dependabot[bot] in [#299](https://github.com/cpp-linter/cpp-linter-rs/pull/299)
- Bump rand from 0.9.2 to 0.9.4 by @dependabot[bot] in [#298](https://github.com/cpp-linter/cpp-linter-rs/pull/298)
- Bump the cargo group across 1 directory with 8 updates by @dependabot[bot] in [#301](https://github.com/cpp-linter/cpp-linter-rs/pull/301)
- Bump brace-expansion from 5.0.5 to 5.0.6 by @dependabot[bot] in [#319](https://github.com/cpp-linter/cpp-linter-rs/pull/319)
- Bump openssl from 0.10.75 to 0.10.80 by @dependabot[bot] in [#320](https://github.com/cpp-linter/cpp-linter-rs/pull/320)
- Bump pymdown-extensions from 10.20 to 10.21.3 by @dependabot[bot] in [#321](https://github.com/cpp-linter/cpp-linter-rs/pull/321)
- Bump idna from 3.11 to 3.15 by @dependabot[bot] in [#322](https://github.com/cpp-linter/cpp-linter-rs/pull/322)
- Bump version to clang-installer/v0.1.1 by @2bndy5 in [`24ed0a4`](https://github.com/cpp-linter/cpp-linter-rs/commit/24ed0a4556bf5169b8da5c06eef29f5694caa2cd)
- Bump version to cpp-linter/v2.0.0-rc.1 by @2bndy5 in [`2c65f26`](https://github.com/cpp-linter/cpp-linter-rs/commit/2c65f26bc24060e5f80271ef315f54dced30b9cd)
- Bump version to cpp-linter-py/v2.0.0-rc.1 by @2bndy5 in [`e88c348`](https://github.com/cpp-linter/cpp-linter-rs/commit/e88c348424db535e7ced5f81300df74aed0885f4)
- Bump version to cpp-linter-js/v2.0.0-rc.1 by @2bndy5 in [`86cc773`](https://github.com/cpp-linter/cpp-linter-rs/commit/86cc7739b865fd0bce86d0f265a0e0935ef13437)

### <!-- 8 --> 📝 Documentation

- Switch to mdbook for docs by @2bndy5 in [#13](https://github.com/cpp-linter/cpp-linter-rs/pull/13)
- Mock REST API calls in tests by @2bndy5 in [#21](https://github.com/cpp-linter/cpp-linter-rs/pull/21)
- Release trial follow up by @2bndy5 in [#41](https://github.com/cpp-linter/cpp-linter-rs/pull/41)
- Move logic for release notes generation from Python script to Jinja template (release CI) by @2bndy5 in [#44](https://github.com/cpp-linter/cpp-linter-rs/pull/44)
- Add ReadTheDocs config by @2bndy5 in [#45](https://github.com/cpp-linter/cpp-linter-rs/pull/45)
- Switch to mkdocs by @2bndy5 in [`da09e54`](https://github.com/cpp-linter/cpp-linter-rs/commit/da09e542488ace2c11b60d83963912ef57b4d754)
- Minor update node binding README by @2bndy5 in [`fc2244f`](https://github.com/cpp-linter/cpp-linter-rs/commit/fc2244f81903c719a5268f18e595f422d1a958e1)
- [rust API] update logo, favicon, and some links by @2bndy5 in [`31b7add`](https://github.com/cpp-linter/cpp-linter-rs/commit/31b7add5ea8b1938ea4f816f27a732f3ec8d5227)
- Reorganize LICENSE info by @shenxianpeng in [#89](https://github.com/cpp-linter/cpp-linter-rs/pull/89)
- Render licenses of deps in tables by @2bndy5 in [#91](https://github.com/cpp-linter/cpp-linter-rs/pull/91)
- Fix typo in doc string comment by @2bndy5 in [`9463247`](https://github.com/cpp-linter/cpp-linter-rs/commit/9463247d5fc127a765243893f54c3745f940094d)
- Update contributing guide to reflect changes in workflows by @shenxianpeng in [`38ab160`](https://github.com/cpp-linter/cpp-linter-rs/commit/38ab160f28eca2bd4291b2227ba62afc8c6829b1)

### <!-- 9 --> 🗨️ Changed

- Init commit by @2bndy5 in [`2e25fec`](https://github.com/cpp-linter/cpp-linter-rs/commit/2e25fec0a447df24d0bcc1b80f6624040bab755e)
- Use separate crates for different entry points by @2bndy5 in [#2](https://github.com/cpp-linter/cpp-linter-rs/pull/2)
- Refactor line filters; minor metadata updates by @2bndy5 in [`19d5517`](https://github.com/cpp-linter/cpp-linter-rs/commit/19d5517dc1c95c8269c0beb583387df6197b1ec7)
- Some updates from py codebase by @2bndy5 in [#5](https://github.com/cpp-linter/cpp-linter-rs/pull/5)
- Use task runner `just` (or VSCode "tasks") by @2bndy5 in [#14](https://github.com/cpp-linter/cpp-linter-rs/pull/14)
- Support file paths in CLI positional argument by @2bndy5 in [#16](https://github.com/cpp-linter/cpp-linter-rs/pull/16)
- PR review suggestions by @2bndy5 in [`bd049d0`](https://github.com/cpp-linter/cpp-linter-rs/commit/bd049d06c48b4dc40da958a478873ac30183ee46)
- Prepare for v2.0.0-rc1 by @2bndy5 in [`9189e86`](https://github.com/cpp-linter/cpp-linter-rs/commit/9189e86da499606439f6b65b62df5603f57d9da7)
- Refactor files by @2bndy5 in [#38](https://github.com/cpp-linter/cpp-linter-rs/pull/38)
- Metadata changes by @2bndy5 in [`f4237ae`](https://github.com/cpp-linter/cpp-linter-rs/commit/f4237ae593e468eca0e63169c9360e97bd6e1f26)
- Update READMEs by @2bndy5 in [`3e9c128`](https://github.com/cpp-linter/cpp-linter-rs/commit/3e9c12846c0eb96f8cdd68fc7435bd8965e7ce6a)
- Some cleanup from release trials by @2bndy5 in [`25c3951`](https://github.com/cpp-linter/cpp-linter-rs/commit/25c3951b0ecef9e078ea71932c9401ad8abc2a28)
- Bump version to v2.0.0-rc2 by @2bndy5 in [`ebcb6c4`](https://github.com/cpp-linter/cpp-linter-rs/commit/ebcb6c4941fbaa8147c768252d6d7d9adcfa3bb3)
- [node] add life cycle script prepublishOnly by @2bndy5 in [`55650ea`](https://github.com/cpp-linter/cpp-linter-rs/commit/55650ea96aac628023acb120525d674bcf17a529)
- Bump version to v2.0.0-rc3 by @2bndy5 in [`070c5f7`](https://github.com/cpp-linter/cpp-linter-rs/commit/070c5f75f15d0190ee0204992165673c8f16c47d)
- Bump version to v2.0.0-rc4 by @2bndy5 in [`3e98e20`](https://github.com/cpp-linter/cpp-linter-rs/commit/3e98e20d2405b909b038ff87911dc0d5457613cc)
- Rename repository by @2bndy5 in [`5e3d93f`](https://github.com/cpp-linter/cpp-linter-rs/commit/5e3d93f53e98bbfed56057d2f3984f65c7835550)
- Bump version to v2.0.0-rc5 by @2bndy5 in [`12ecf9f`](https://github.com/cpp-linter/cpp-linter-rs/commit/12ecf9f702fcd1735d16079567e730f0be4a9a35)
- Reorganize repo folders by @2bndy5 in [`d30125b`](https://github.com/cpp-linter/cpp-linter-rs/commit/d30125bbe9015d7ea070a3b8c20fcc9acc56dbc0)
- Bump version to v2.0.0-rc6 by @2bndy5 in [`12729b8`](https://github.com/cpp-linter/cpp-linter-rs/commit/12729b86ec9047f550c842c7cbbd412a232b8470)
- Bump version to v2.0.0-rc7 by @2bndy5 in [`a2001f3`](https://github.com/cpp-linter/cpp-linter-rs/commit/a2001f395a6bb078ed43d4ef5014fa3304ca9323)
- Bump version to v2.0.0-rc8 by @2bndy5 in [`205bd2d`](https://github.com/cpp-linter/cpp-linter-rs/commit/205bd2d492ee45e199ca23d1ac9f118c03a2c4a2)
- Bump version to v2.0.0-rc9 by @2bndy5 in [`9ccbc14`](https://github.com/cpp-linter/cpp-linter-rs/commit/9ccbc14c575ddceec5827f18c9c0cf69d8296927)
- Bump version to v2.0.0-rc10 by @2bndy5 in [`3915e2b`](https://github.com/cpp-linter/cpp-linter-rs/commit/3915e2b5a09fd9b69a0233da6b8f64a2e34ce553)
- Bump version to v2.0.0-rc11 by @2bndy5 in [`d73c4f6`](https://github.com/cpp-linter/cpp-linter-rs/commit/d73c4f6c7d80bc55e8204b3b9a75879c5914f75a)
- Bump version to v2.0.0-rc12 by @2bndy5 in [`aa03ebb`](https://github.com/cpp-linter/cpp-linter-rs/commit/aa03ebb48a5bff90ffa0bcf2e1adb4dc64932c57)
- Thanks for having me and updated to my chinese name by @shenxianpeng in [`2e1aea8`](https://github.com/cpp-linter/cpp-linter-rs/commit/2e1aea831851be2e17d1292743b3fc40f060f108)
- Better Benchmark by @2bndy5 in [#92](https://github.com/cpp-linter/cpp-linter-rs/pull/92)
- Use `Client` instance by reference by @2bndy5 in [#141](https://github.com/cpp-linter/cpp-linter-rs/pull/141)
- Update locked transitive dependencies by @2bndy5 in [`aad12c6`](https://github.com/cpp-linter/cpp-linter-rs/commit/aad12c633a0589f36f8cd5e462ad89e359c24d9c)
- Review CI by @2bndy5 in [#195](https://github.com/cpp-linter/cpp-linter-rs/pull/195)
- Use clap derive feature by @2bndy5 in [#204](https://github.com/cpp-linter/cpp-linter-rs/pull/204)
- Bump version to v2.0.0-rc13 by @2bndy5 in [`26e5b8a`](https://github.com/cpp-linter/cpp-linter-rs/commit/26e5b8ace3ce5d682406246bc92a631fd4ebb026)
- Bump version to v2.0.0-rc14 by @2bndy5 in [`3272095`](https://github.com/cpp-linter/cpp-linter-rs/commit/3272095e6d93d78aeb5d3add84398309407027b5)
- Include LICENSE file in source distribution for PyPI by @2bndy5 in [`32e20b3`](https://github.com/cpp-linter/cpp-linter-rs/commit/32e20b39d43908e254534f4a6bf2a67560a315ba)
- Bump version to v2.0.0-rc15 by @2bndy5 in [`c5b90f4`](https://github.com/cpp-linter/cpp-linter-rs/commit/c5b90f407cd55dd8b8b96b13e079acce2c6b5a8c)
- Adjust benchmark parameters by @2bndy5 in [#229](https://github.com/cpp-linter/cpp-linter-rs/pull/229)
- Adhere to new clippy lint warning by @2bndy5 in [#255](https://github.com/cpp-linter/cpp-linter-rs/pull/255)
- Prepare to release individual packages by @2bndy5 in [#302](https://github.com/cpp-linter/cpp-linter-rs/pull/302)

[cpp-linter-js/v2.0.0-rc.1]: https://github.com/cpp-linter/cpp-linter-rs/compare/2e25fec0a447df24d0bcc1b80f6624040bab755e...cpp-linter-js/v2.0.0-rc.1

Full commit diff: [`2e25fec...cpp-linter-js/v2.0.0-rc.1`][cpp-linter-js/v2.0.0-rc.1]

<!-- generated by git-cliff -->
