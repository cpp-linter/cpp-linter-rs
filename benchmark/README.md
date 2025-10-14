# Benchmark

This folder is the space where benchmarks are preformed.
Our primary concern is elapsed time.
Thus, we only measure the time it takes for various forms of
cpp-linter to execute the same set of operations.

## Measurement

We use libgit2 (with a generated compilation database) and cpp-linter defaults.
Elapsed time is measured with [hyperfine].
See [benchmark.nu](./benchmark.nu) for more details.

[hyperfine]: https://github.com/sharkdp/hyperfine
