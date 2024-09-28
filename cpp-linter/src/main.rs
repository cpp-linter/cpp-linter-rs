#![cfg(not(test))]
/// This crate is the binary executable's entrypoint.
use std::env;

use ::cpp_linter::run::run_main;
use anyhow::Result;

/// This function simply forwards CLI args to [`run_main()`].
#[tokio::main]
pub async fn main() -> Result<()> {
    run_main(env::args().collect::<Vec<String>>()).await
}
