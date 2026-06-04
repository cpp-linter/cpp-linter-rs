use anyhow::Result;
use clang_installer::{ClangTool, RequestedVersion};
use clap::Parser;

use std::{collections::HashMap, path::PathBuf, str::FromStr};
mod logging {
    use colored::{Colorize, control::set_override};
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use std::{
        env,
        io::{Write, stdout},
    };

    struct SimpleLogger;

    impl SimpleLogger {
        fn level_color(level: &Level) -> String {
            let name = format!("{:>5}", level.as_str().to_uppercase());
            match level {
                Level::Error => name.red().bold().to_string(),
                Level::Warn => name.yellow().bold().to_string(),
                Level::Info => name.green().bold().to_string(),
                Level::Debug => name.blue().bold().to_string(),
                Level::Trace => name.magenta().bold().to_string(),
            }
        }
    }

    impl Log for SimpleLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= log::max_level()
        }

        fn log(&self, record: &Record) {
            let mut stdout = stdout().lock();
            if record.target() == "CI_LOG_GROUPING" {
                // this log is meant to manipulate a CI workflow's log grouping
                writeln!(stdout, "{}", record.args())
                    .expect("Failed to write log command to stdout");
                stdout
                    .flush()
                    .expect("Failed to flush log command in stdout");
            } else if self.enabled(record.metadata()) {
                let module = record.module_path();
                if module.is_none_or(|v| {
                    v.starts_with("clang_installer") || v.starts_with("clang_tools")
                }) {
                    writeln!(
                        stdout,
                        "[{}]: {}",
                        Self::level_color(&record.level()),
                        record.args()
                    )
                    .expect("Failed to write log message to stdout");
                } else if let Some(module) = module {
                    writeln!(
                        stdout,
                        "[{}]{{{}:{}}}: {}",
                        Self::level_color(&record.level()),
                        module,
                        record.line().unwrap_or_default(),
                        record.args()
                    )
                    .expect("Failed to write detailed log message to stdout");
                }
                stdout
                    .flush()
                    .expect("Failed to flush log message in stdout");
            }
        }

        fn flush(&self) {}
    }

    /// A function to initialize the private `LOGGER`.
    ///
    /// The logging level defaults to [`LevelFilter::Info`].
    /// This logs a debug message about [`SetLoggerError`](struct@log::SetLoggerError)
    /// if the `LOGGER` is already initialized.
    pub fn initialize_logger() {
        let logger: SimpleLogger = SimpleLogger;
        if env::var("CPP_LINTER_COLOR")
            .as_deref()
            .is_ok_and(|v| matches!(v, "on" | "1" | "true"))
        {
            set_override(true);
        }
        if let Err(e) =
            log::set_boxed_logger(Box::new(logger)).map(|()| log::set_max_level(LevelFilter::Info))
        {
            // logger singleton already instantiated.
            // we'll just use whatever the current config is.
            log::debug!("{e:?}");
        }
    }
}

#[derive(clap::Parser, Debug)]
pub struct CliOptions {
    /// The desired version of clang to install.
    #[arg(
            short,
            long,
            default_missing_value = "CPP-LINTER-VERSION",
            num_args = 0..=1,
            value_parser = RequestedVersion::from_str,
            default_value = "",
    )]
    pub version: Option<RequestedVersion>,

    /// The clang tool to install.
    #[arg(
        short,
        long,
        value_delimiter = ' ',
        default_value = "clang-format clang-tidy"
    )]
    pub tool: Option<Vec<ClangTool>>,

    /// The directory where the clang tools should be installed.
    #[arg(short, long)]
    pub directory: Option<PathBuf>,

    /// Force overwriting symlink to the installed binary.
    ///
    /// This will only overwrite an existing symlink.
    #[arg(short, long)]
    pub force: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    logging::initialize_logger();
    let options = CliOptions::parse();

    let tool = options
        .tool
        .expect("--tool should have a default value: [clang-format, clang-tidy]");
    match options.version.unwrap_or(RequestedVersion::default()) {
        RequestedVersion::NoValue => {
            log::info!(
                "clang-tools(-installer) version: {}",
                env!("CARGO_PKG_VERSION")
            );
        }
        req_ver => {
            let mut map_tools = HashMap::new();
            for t in tool {
                if let Some(version) = req_ver
                    .eval_tool(&t, options.force, options.directory.as_ref())
                    .await?
                {
                    map_tools.entry(t).or_insert(version);
                }
            }
            log::info!("Finished! \n{map_tools:#?}");
        }
    }
    Ok(())
}
