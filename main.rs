//! This is a preprocessor for mdbook that generates the CLI document.
//! For actual library/binary source code look in cpp-linter folder.

extern crate clap;
use clap::{Arg, ArgMatches, Command};
extern crate mdbook;
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
extern crate semver;
extern crate serde_json;
use semver::{Version, VersionReq};
use std::io;
use std::process;

extern crate cpp_linter;

use cli_gen_lib::CliGen;

pub fn make_app() -> Command {
    Command::new("cli-gen")
        .about("A mdbook preprocessor which generates a doc of CLI options")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args
        .get_one::<String>("renderer")
        .expect("Required argument");
    let supported = pre.supports_renderer(renderer);

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

fn main() {
    let matches = make_app().get_matches();

    // Users will want to construct their own preprocessor here
    let preprocessor = CliGen {};

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(&preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(&preprocessor) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

mod cli_gen_lib {
    use std::path::PathBuf;

    use mdbook::book::Book;
    use mdbook::preprocess::{Preprocessor, PreprocessorContext};

    use cpp_linter::cli;

    pub struct CliGen;

    impl CliGen {
        fn generate_cli(groups_order: &Option<Vec<String>>) -> String {
            let mut out = String::new();
            let command = cli::get_arg_parser();
            out.push_str(format!("\n{}\n\n", "# Command Line Interface").as_str());
            out.push_str("<!-- markdownlint-disable MD033 MD028 -->\n");
            out.push_str("\n## Commands\n");
            for cmd in command.get_subcommands() {
                out.push_str(format!("\n### `{}`\n\n", cmd.get_name()).as_str());
                out.push_str(
                    format!("{}\n", &cmd.get_about().unwrap().to_string().trim()).as_str(),
                );
            }
            out.push_str("## Arguments\n");
            for arg in command.get_positionals() {
                out.push_str(format!("\n### `{}`\n\n", arg.get_id().as_str()).as_str());
                if let Some(help) = arg.get_help() {
                    out.push_str(format!("{}\n", help.to_string().trim()).as_str());
                }
            }
            let arg_groups = if let Some(groups) = groups_order {
                eprintln!("ordering groups into {:?}", groups);
                let mut ordered = Vec::with_capacity(command.get_groups().count());
                for group in groups {
                    let mut group_obj = None;
                    for arg_group in command.get_groups() {
                        if arg_group.get_id().as_str() == group.as_str() {
                            group_obj = Some(arg_group.clone());
                        }
                    }
                    ordered.push(
                        group_obj
                            .unwrap_or_else(|| panic!("{} not found in command's groups", group)),
                    );
                }
                ordered
            } else {
                command.get_groups().map(|g| g.to_owned()).collect()
            };
            for group in arg_groups {
                out.push_str(format!("\n## {}\n", group.get_id()).as_str());
                for arg_id in group.get_args() {
                    let mut arg_match = command.get_arguments().filter(|a| *a.get_id() == *arg_id);
                    let arg = arg_match.next().unwrap_or_else(|| {
                        panic!(
                            "arg {} expected in group {}",
                            arg_id.as_str(),
                            group.get_id().as_str()
                        )
                    });
                    out.push_str(
                        format!(
                            "\n### `-{}, --{}`\n\n",
                            &arg.get_short().unwrap(),
                            &arg.get_long().unwrap()
                        )
                        .as_str(),
                    );
                    let default = arg.get_default_values();
                    if !default.is_empty() {
                        out.push_str("<dt>Default</dt><dd>");
                        assert_eq!(default.len(), 1);
                        out.push_str(
                            format!("<code>{:?}</code></dd>\n\n", default.first().unwrap())
                                .as_str(),
                        );
                    }
                    if let Some(help) = &arg.get_help() {
                        out.push_str(format!("{}\n", help.to_string().trim()).as_str());
                    }
                }
            }
            out
        }
    }

    impl Preprocessor for CliGen {
        fn name(&self) -> &str {
            "cli-gen"
        }

        fn run(&self, ctx: &PreprocessorContext, book: Book) -> mdbook::errors::Result<Book> {
            let mut altered = book.clone();
            let groups_order = match ctx
                .config
                .get_preprocessor("cli-gen")
                .unwrap()
                .get("groups-order")
            {
                Some(val) => val.clone().as_array_mut().map(|v| {
                    v.iter_mut()
                        .map(|o| o.to_string().trim_matches('"').to_string())
                        .collect()
                }),
                None => None,
            };
            altered.for_each_mut(|item| {
                if let mdbook::BookItem::Chapter(chap) = item {
                    if chap
                        .path
                        .clone()
                        .is_some_and(|p| p == PathBuf::from("cli.md"))
                    {
                        chap.content = CliGen::generate_cli(&groups_order);
                    }
                }
            });
            Ok(altered)
        }

        fn supports_renderer(&self, renderer: &str) -> bool {
            matches!(renderer, "html" | "markdown")
        }
    }
}
