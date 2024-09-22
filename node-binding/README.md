# cpp-linter

The node.js binding binding for the [cpp_linter_rs][this] rust project
(built using [napi-rs](https://napi.rs) and [yarn](https://yarnpkg.com)).

[this]: https://github.com/cpp-linter/cpp_linter_rs

## Install

Install with `npm`:

```text
npm -g install @cpp-linter/cpp-linter
```

## Usage

For usage in a CI workflow, see
[the cpp-linter/cpp-linter-action repository](https://github.com/cpp-linter/cpp-linter-action).

For the description of supported Command Line Interface options, see
[the CLI documentation](https://cpp-linter.github.io/cpp_linter_rs/cli.html).

## Development

After the native module is built using [`yarn build:debug`](#yarn-builddebug), you can
invoke the executable script as a normal CLI app:

```text
npx cpp-linter --help
```

### Scripts

If an available script is not described below, it should be considered a convenience
tool for the CI/CD workflow.

#### `yarn build`

This script builds the native module for distribution (with release profile optimizations).

##### `yarn build:debug`

Same as `yarn build` but does not use the release profile optimizations.
You should use this script when testing locally.

#### `yarn test`

This script runs a simple test to ensure the native module was built correctly.

### Folder structure

| Name | Description |
|-----:|:------------|
| `__test__` | The location of the unit test(s). |
| `npm` | The required metadata for publishing platform-specific packages to npm. |
| `src` | The location for all rust sources related to binding the cpp-linter library. |
| `build.rs` | The cargo-specific build script used when compiling the binding. |
| `Cargo.toml` | Metadata about the binding's rust package (which _is not_ intended to be published to crates.io). |
| `package.json` | Metadata about the npm package (platform agnostic). |
| `index.d.ts` | The generated TypeScript typing info the describes the exposing functionality in the built native module. |
| `index.js` | The generated script that delegates which platform-specific package to import. |
| `cpp-linter.x-y-z.node` | Then native module built for a specific platform (where `x-y-z` denotes the platform's name using compilation target). |

Hidden files and folders are not described in the table above.
If they are not ignored by a gitignore specification, then they should be considered
important for maintenance or distribution.
