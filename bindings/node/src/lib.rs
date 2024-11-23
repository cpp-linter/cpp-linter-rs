#[macro_use]
extern crate napi_derive;
use ::cpp_linter::run::run_main;
use napi::bindgen_prelude::*;

#[napi]
pub async fn main(args: Vec<String>) -> Result<()> {
    run_main(args)
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))
}
