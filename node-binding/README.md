# cpp-linter node binding

The node.js binding generated from rust source code.

This project must use yarn because [napi-rs] uses [yarn] to get platform-specific information.

[yarn]: https://yarnpkg.com/
[napi-rs]: https://napi.rs/

This repo also doubles as a yarn workspace. So, the lock file exists in repo root folder.
Furthermore, most scripts that can be executed in this project are available to run from repo root folder (using the same script name).

## Usage

After the native module is built using [`yarn-build`](#yarn-build), you can import the binding via the generated `index.js` file (using the `node` console):

```sh
cppLinter = require('./index.js')
await cppLinter.main(['cpp-linter', '--help'])
```

All CLI arguments are passed as a array of strings to the binding's `main()` function.
Notice the name of the CLI app (`'cpp-linter'`) is required because the rust argument parsing
mechanism (`clap` crate) needs to know the name of the program invoked.

## Scripts

> [!note]
> If an available script is not described below, it should be considered a convenience tool for the CI workflow.

### `yarn build`

This script builds the native module for distribution.

#### `yarn build:debug`

Same as `yarn build` but does not use the release optimizations.
You should use this script when testing locally.

### `yarn test`

This script runs a simple test to ensure the native module was built correctly.

## Folder structure

| Name | Description |
|-----:|:------------|
| `__test__` | The location of the unit test(s). |
| `npm` | The required metadata for publishing platform-specific packages to npm. |
| `src` | The location for all rust sources related to binding the cpp-linter library. |
| `build.rs` | The cargo-specific build script used when compiling the binding. |
| `Cargo.toml` | Metadata about the rust package (which _is not_ intended to be published to crates.io). |
| `package.json` | Metadata about the node.js binding (which _is_ meant to be published to npm). |
| `index.d.ts` | The generated TypeScript typing info the describes the exposing functionality in the built native module. |
| `index.js` | The generated script that delegates which platform-specific package to import. |
| `cpp-linter.x-y-z.node` | Then native module built for a specific platform (where `x-y-z` denotes the platform's name using compilation target). |

Hidden files and folders are not described in the table above.
If they are not ignored by a gitignore specification, then they should be considered
important for maintenance or distribution.
