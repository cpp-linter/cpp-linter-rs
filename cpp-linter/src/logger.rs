//! A module to initialize and customize the logger object used in (most) stdout.

use std::env;

use anyhow::{Error, Result};
use colored::{control::set_override, Colorize};
use log::{Level, LevelFilter, Metadata, Record};

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

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if record.target() == "CI_LOG_GROUPING" {
            // this log is meant to manipulate a CI workflow's log grouping
            println!("{}", record.args());
        } else if self.enabled(record.metadata()) {
            println!(
                "[{}]: {}",
                Self::level_color(&record.level()),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// A function to initialize the private `LOGGER`.
///
/// The logging level defaults to [`LevelFilter::Info`].
/// Returns a [`SetLoggerError`] if the `LOGGER` is already initialized.
pub fn init() -> Result<()> {
    let logger: SimpleLogger = SimpleLogger;
    if env::var("CPP_LINTER_COLOR").is_ok_and(|v| ["on", "1", "true"].contains(&v.as_str())) {
        set_override(true);
    }
    log::set_boxed_logger(Box::new(logger))
        .map(|()| log::set_max_level(LevelFilter::Info))
        .map_err(Error::from)
}

#[cfg(test)]
mod test {
    use std::env;

    use super::{init, SimpleLogger};

    #[test]
    fn trace_log() {
        env::set_var("CPP_LINTER_COLOR", "true");
        init().unwrap_or(());
        assert!(SimpleLogger::level_color(&log::Level::Trace).contains("TRACE"));
        log::set_max_level(log::LevelFilter::Trace);
        log::trace!("A dummy log statement for code coverage");
    }
}
