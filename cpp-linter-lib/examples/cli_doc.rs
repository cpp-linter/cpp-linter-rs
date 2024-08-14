use std::{fs::OpenOptions, io::Write};

use cpp_linter_lib::cli;

pub fn main() -> std::io::Result<()> {
    let command = cli::get_arg_parser();
    let doc_file = OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open("cpp-linter-py/docs/cli_args.rst")?;
    let title = "Command Line Interface".to_string();
    writeln!(&doc_file, "\n{}", &title)?;
    for _ in title.chars() {
        write!(&doc_file, "=")?;
    }
    write!(&doc_file, "\n\n")?;
    writeln!(&doc_file, "Commands\n--------\n")?;
    for cmd in command.get_subcommands() {
        writeln!(&doc_file, ".. std:option:: {}\n", cmd.get_name())?;
        for line in cmd.get_about().unwrap().to_string().split('\n') {
            writeln!(&doc_file, "    {}", &line)?;
        }
        writeln!(&doc_file)?;
    }
    for group in command.get_groups() {
        writeln!(&doc_file, "\n{}", group.get_id().to_string())?;
        for _ in group.get_id().to_string().chars() {
            write!(&doc_file, "-")?;
        }
        write!(&doc_file, "\n\n")?;
        for arg_id in group.get_args() {
            let mut arg_match = command.get_arguments().filter(|a| *a.get_id() == *arg_id);
            let arg = arg_match.next().expect(
                format!(
                    "arg {} expected in group {}",
                    arg_id.as_str(),
                    group.get_id().as_str()
                )
                .as_str(),
            );
            writeln!(
                &doc_file,
                ".. std:option:: -{}, --{}\n",
                &arg.get_short().unwrap(),
                &arg.get_long().unwrap()
            )?;
            for line in arg.get_long_help().unwrap().to_string().split('\n') {
                writeln!(&doc_file, "    {}", &line)?;
            }
            writeln!(&doc_file)?;
            let default = arg.get_default_values();
            if !default.is_empty() {
                writeln!(&doc_file, "    :Default:")?;
                if default.len() < 2 {
                    writeln!(&doc_file, "        ``{:?}``", default.first().unwrap())?;
                } else {
                    for val in default {
                        writeln!(&doc_file, "        - ``{:?}``", val)?;
                    }
                }
            }
        }
    }
    println!("cpp-linter-py/docs/cli_args.rst generated!");
    Ok(())
}
