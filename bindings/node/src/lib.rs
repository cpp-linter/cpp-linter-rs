#[macro_use]
extern crate napi_derive;
use ::cpp_linter::run::run_main;
use napi::{bindgen_prelude::*, tokio};

#[napi]
pub fn main(args: Vec<String>) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_main(args))
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}
