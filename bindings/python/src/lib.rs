use pyo3::{
    exceptions::{PyOSError, PySystemExit},
    prelude::*,
};
use std::env;
use tokio::runtime::Builder;

use ::cpp_linter::run::run_main;

/// Run [`run_main()`] with the given CLI args,
/// or the process' args (without the path to the python interpreter) if none are given.
fn run(args: Option<Vec<String>>) -> anyhow::Result<()> {
    // exclude path to python interpreter
    let args = args.unwrap_or(env::args().collect::<Vec<String>>()[1..].to_vec());
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { run_main(args).await })
}

/// A wrapper for the ``::cpp_linter::run::run_main()```
///
/// Errors are raised as an ``OSError``.
#[pyfunction]
#[pyo3(signature = (args = None))]
fn main(args: Option<Vec<String>>) -> PyResult<()> {
    run(args).map_err(|e| PyOSError::new_err(e.to_string()))
}

/// The entrypoint of the ``cpp-linter`` command that the python package installs.
///
/// Like the ``cpp-linter`` executable, this prints an error on a line starting with ``Error:``
/// and exits with code 1, instead of showing a python traceback.
#[pyfunction]
fn cli() -> PyResult<()> {
    run(None).map_err(|e| {
        eprintln!("Error: {e:?}");
        PySystemExit::new_err(1)
    })
}

/// The python binding for the cpp_linter package. It only exposes a ``main()`` function
/// (and the ``cli()`` function used as the entrypoint script).
#[pymodule(gil_used = false)]
fn cpp_linter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(main, m)?)?;
    m.add_function(wrap_pyfunction!(cli, m)?)?;
    Ok(())
}
