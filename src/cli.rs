use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to configuration file
    pub conf_path: Option<PathBuf>,
}

pub fn get_cli() -> Cli {
    Cli::parse()
}
