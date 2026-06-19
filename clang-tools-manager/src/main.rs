use anyhow::Result;
use clang_tools_manager::{
    ClangTool, RequestedVersion,
    logger::{CLI_HELP_STYLE, try_init_logger},
};
use clap::Parser;

use std::{collections::HashMap, path::PathBuf, str::FromStr};

#[derive(clap::Parser, Debug)]
#[command(styles(CLI_HELP_STYLE))]
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

    /// Enable verbose logging for debugging purposes.
    ///
    /// This will include more DEBUG level log messages.
    /// Without it, log level is set to INFO by default.
    #[arg(
        short = 'V',
        long,
        default_value_t = false,
        action = clap::ArgAction::SetTrue,
    )]
    pub verbose: bool,

    /// The clang tool to install.
    #[arg(
        num_args = 0..,
        default_values_t = vec![ClangTool::ClangFormat, ClangTool::ClangTidy],
    )]
    pub tool: Vec<ClangTool>,

    /// The directory where the clang tools should be installed.
    #[arg(short, long)]
    pub directory: Option<PathBuf>,

    /// Force overwriting symlink to the installed binary.
    ///
    /// This will only overwrite an existing symlink.
    #[arg(short, long)]
    pub force: bool,

    /// Whether to use the system's available package managers.
    ///
    /// By default, this matches the value of a CI environment variable.
    /// For non-CI contexts, this allows users to opt-in to using
    /// system package managers as a fallback in case PyPI offerings
    /// are unsatisfactory.
    ///
    /// If system package managers are not allowed or fail, then
    /// static binaries built by cpp-linter are sought (for
    /// compatible platforms).
    #[arg(long, action = clap::ArgAction::SetTrue, conflicts_with = "no_mod_sys")]
    pub mod_sys: bool,

    /// Strictly disallow using system package managers.
    ///
    /// This can be used to override the default behavior of `--mod-sys`,
    /// useful in sensitive CI environments, like self-hosted runners.
    #[arg(long, action = clap::ArgAction::SetTrue, conflicts_with = "mod_sys")]
    pub no_mod_sys: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    try_init_logger();
    let options = CliOptions::parse();
    if options.verbose {
        log::set_max_level(log::LevelFilter::Debug);
    }
    match options.version.unwrap_or(RequestedVersion::default()) {
        RequestedVersion::NoValue => {
            log::info!(
                "clang-tools(-installer) version: {}",
                env!("CARGO_PKG_VERSION")
            );
        }
        req_ver => {
            let mut map_tools = HashMap::new();
            for t in options.tool {
                if let Some(version) = req_ver
                    .eval_tool(
                        &t,
                        options.force,
                        options.directory.as_ref(),
                        if options.no_mod_sys {
                            false // explicitly false
                        } else {
                            options.mod_sys // explicitly true
                                || std::env::var("CI").is_ok_and(|v| {
                                    ["true", "on", "1"].contains(&v.to_lowercase().as_str())
                                }) // implicitly true in CI environments
                        },
                    )
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
