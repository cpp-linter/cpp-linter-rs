///! This crate is the binary executable's entrypoint.
use std::env;

use cpp_linter_lib::run::run_main;

/// This function simply forwards CLI args to [`run_main()`].
pub fn main() {
    run_main(env::args().collect::<Vec<String>>());
}
