//! This exposes a function in Python, so an mkdocs plugin can use it to generate the CLI document.
//! For actual library/binary source code look in cpp-linter folder.
use std::collections::HashMap;

use cpp_linter::cli;
use pyo3::{exceptions::PyValueError, prelude::*};

const GROUPS_ORDER: [&str; 5] = [
    "General options",
    "Source options",
    "Clang-format options",
    "Clang-tidy options",
    "Feedback options",
];

#[pyfunction]
fn generate_cli_doc(metadata: HashMap<String, HashMap<String, Py<PyAny>>>) -> PyResult<String> {
    let mut out = String::new();
    let mut command = cli::get_arg_parser();
    out.push_str(
        format!(
            "```text title=\"Usage\"\n{}\n```\n",
            command
                .render_usage()
                .to_string()
                .trim_start_matches("Usage: ")
        )
        .as_str(),
    );

    out.push_str("\n## Commands\n");
    for cmd in command.get_subcommands() {
        out.push_str(format!("\n### `{}`\n\n", cmd.get_name()).as_str());
        out.push_str(
            format!(
                "{}\n",
                &cmd.get_about()
                    .ok_or(PyValueError::new_err(format!(
                        "{} command has no help message",
                        cmd.get_name()
                    )))?
                    .to_string()
                    .trim()
            )
            .as_str(),
        );
    }

    out.push_str("## Arguments\n");
    for arg in command.get_positionals() {
        out.push_str(format!("\n### `{}`\n\n", arg.get_id().as_str()).as_str());
        if let Some(help) = arg.get_help() {
            out.push_str(format!("{}\n", help.to_string().trim()).as_str());
        }
    }

    // reorganize groups according to GROUPS_ORDER
    let mut ordered = Vec::with_capacity(command.get_groups().count());
    for group in GROUPS_ORDER {
        let group_obj = command
            .get_groups()
            .find(|arg_group| arg_group.get_id().as_str() == group)
            .ok_or(PyValueError::new_err(format!(
                "{} not found in command's groups",
                group
            )))?;
        ordered.push(group_obj.to_owned());
    }

    for group in ordered {
        out.push_str(format!("\n## {}\n", group.get_id()).as_str());
        for arg_id in group.get_args() {
            let arg = command
                .get_arguments()
                .find(|a| *a.get_id() == *arg_id)
                .ok_or(PyValueError::new_err(format!(
                    "arg {} in group {} not found in command",
                    arg_id.as_str(),
                    group.get_id().as_str()
                )))?;
            let long_name = arg.get_long().ok_or(PyValueError::new_err(format!(
                "Failed to get long name of argument with id {}",
                arg_id.as_str()
            )))?;
            out.push_str(
                format!(
                    "\n### `-{}, --{}`\n\n",
                    arg.get_short().ok_or(PyValueError::new_err(format!(
                        "Failed to get short name for argument with id {}",
                        arg_id.as_str()
                    )))?,
                    long_name
                )
                .as_str(),
            );
            if let Some(map) = metadata.get(long_name) {
                if let Some(val) = map.get("minimum-version") {
                    out.push_str(format!("<!-- md:version {} -->\n", val).as_str());
                }
                if let Some(val) = map.get("required-permission") {
                    out.push_str(format!("<!-- md:permission {} -->\n", val).as_str());
                }
                if map.contains_key("experimental") {
                    out.push_str("<!-- md:flag experimental -->\n");
                }
            }
            let default = arg.get_default_values();
            if let Some(default_value) = default.first() {
                out.push_str(format!("<!-- md:default {:?} -->\n\n", default_value).as_str());
            } else {
                out.push('\n');
            }
            if let Some(help) = &arg.get_help() {
                out.push_str(format!("{}\n", help.to_string().trim()).as_str());
            }
        }
    }
    Ok(out)
}

#[pymodule]
pub fn cli_gen(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(generate_cli_doc, m)?)?;
    Ok(())
}
