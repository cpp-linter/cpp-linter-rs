use crate::{ClangTool, RequestedVersion};
use std::path::PathBuf;
#[cfg(feature = "clap")]
use std::str::FromStr;

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Debug)]
pub struct CliOptions {
    /// The desired version of clang to install.
    #[cfg_attr(
        feature = "clap",
        arg(
            short,
            long,
            default_missing_value = "CPP-LINTER-VERSION",
            num_args = 0..=1,
            value_parser = RequestedVersion::from_str,
            default_value = "",
        )
    )]
    pub version: Option<RequestedVersion>,
    /// The clang tool to install.
    #[cfg_attr(
        feature = "clap",
        arg(
            short,
            long,
            value_delimiter = ' ',
            default_value = "clang-format clang-tidy",
        )
    )]
    pub tool: Option<Vec<ClangTool>>,
    /// The directory where the clang tools should be installed.
    #[cfg_attr(feature = "clap", arg(short, long))]
    pub directory: Option<PathBuf>,
    /// Force overwriting symlink to the installed binary.
    ///
    /// This will only overwrite an existing symlink.
    #[cfg_attr(feature = "clap", arg(short, long))]
    pub force: bool,
}
