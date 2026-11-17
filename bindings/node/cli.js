#!/usr/bin/env node
const cppLinter = require('./index.js')

process.argv.shift(); // pop the path to `node` interpreter

try {
  cppLinter.main(process.argv);
} catch (error) {
  // Print the error like the `cpp-linter` executable does, instead of a stack trace.
  console.error(`Error: ${error.message}`);
  process.exitCode = 1;
}
