use std::{
    env::current_dir,
    path::{PathBuf, absolute},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// initializes new configuration
    Init {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
    /// starts VSCode
    Code {
        /// Path to configuration file
        path: PathBuf,
    },
    /// starts new shell
    Shell {
        /// Path to configuration file
        path: Option<PathBuf>,
    },
}

pub fn get_cli() -> Cli {
    Cli::parse()
}

impl Cli {
    pub fn get_repository_path(&self) -> Result<PathBuf> {
        match &self.cmd {
            Command::Init { list } => todo!(),
            Command::Code { path } => Ok(absolute(&path)?),
            Command::Shell { path: Some(path) } => Ok(absolute(&path)?
                .parent()
                .with_context(|| format!("Configuration cannot be found as {:?}", path))?
                .to_owned()),
            Command::Shell { path: None } => Ok(current_dir()?),
        }
    }
}
