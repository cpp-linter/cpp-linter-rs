#[macro_use]
extern crate napi_derive;
use ::cpp_linter::run::run_main;

#[napi]
pub async fn main(args: Vec<String>) -> i32 {
    run_main(args).await
}
