use pyo3::{exceptions::PyOSError, prelude::*};
use std::env;
use tokio::runtime::Builder;

use ::cpp_linter::run::run_main;

/// A wrapper for the ``::cpp_linter::run::run_main()```
#[pyfunction]
#[pyo3(signature = (args = None))]
fn main(args: Option<Vec<String>>) -> PyResult<()> {
    // exclude path to python interpreter
    let args = args.unwrap_or(env::args().collect::<Vec<String>>()[1..].to_vec());
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run_main(args).await })
        .map_err(|e| PyOSError::new_err(e.to_string()))
}

/// The python binding for the cpp_linter package. It only exposes a ``main()`` function
/// that is used as the entrypoint script.
#[pymodule]
fn cpp_linter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(main, m)?)?;
    Ok(())
}
