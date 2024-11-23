#!/usr/bin/env node
const cppLinter = require('./index.js')

process.argv.shift(); // pop the path to `node` interpreter

cppLinter.main(process.argv);
