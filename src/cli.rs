// Clap definitions in derive style

use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Cli {
    /// Set the level of verbosity
    /// -v for info, -vv for debug, -vvv for trace
    #[clap(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Music library path
    pub library_path: PathBuf,
}
