//! A module to initialize and customize the logger object used in (most) stdout.

use std::{
    env,
    io::{stdout, Write},
};

use colored::{control::set_override, Colorize};
use log::{Level, LevelFilter, Log, Metadata, Record};

#[derive(Default)]
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
            writeln!(stdout, "{}", record.args()).expect("Failed to write log command to stdout");
            stdout
                .flush()
                .expect("Failed to flush log command to stdout");
        } else if self.enabled(record.metadata()) {
            let module = record.module_path();
            if module.is_none_or(|v| v.starts_with("cpp_linter")) {
                writeln!(
                    stdout,
                    "[{}]: {}",
                    Self::level_color(&record.level()),
                    record.args()
                )
                .expect("Failed to write log message to stdout");
            } else {
                writeln!(
                    stdout,
                    "[{}]{{{}:{}}}: {}",
                    Self::level_color(&record.level()),
                    module.unwrap(), // safe to unwrap here because the None case is caught above
                    record.line().unwrap_or_default(),
                    record.args()
                )
                .expect("Failed to write detailed log message to stdout");
            }
            stdout
                .flush()
                .expect("Failed to flush log message to stdout");
        }
    }

    fn flush(&self) {}
}

/// A function to initialize the private `LOGGER`.
///
/// The logging level defaults to [`LevelFilter::Info`].
/// This logs a debug message about [`SetLoggerError`](struct@log::SetLoggerError)
/// if the `LOGGER` is already initialized.
pub fn try_init() {
    let logger: SimpleLogger = SimpleLogger;
    if matches!(
        env::var("CPP_LINTER_COLOR").as_deref(),
        Ok("on" | "1" | "true")
    ) {
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

#[cfg(test)]
mod test {
    use std::env;

    use super::{try_init, SimpleLogger};

    #[test]
    fn trace_log() {
        env::set_var("CPP_LINTER_COLOR", "true");
        try_init();
        assert!(SimpleLogger::level_color(&log::Level::Trace).contains("TRACE"));
        log::set_max_level(log::LevelFilter::Trace);
        log::trace!("A dummy log statement for code coverage");
    }
}
