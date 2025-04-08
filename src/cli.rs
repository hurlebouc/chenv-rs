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
        #[command(subcommand)]
        lang: Lang,
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

#[derive(Subcommand)]
pub enum Lang {
    /// init java
    Java {
        /// Use sbt
        #[arg(long, default_value_t = false)]
        sbt: bool,

        /// disable build tool installation
        #[arg(long, default_value_t = false)]
        no_build_tool: bool,

        /// Java version
        version: u8,
    },

    Go,
}

pub fn get_cli() -> Cli {
    Cli::parse()
}

impl Cli {
    pub fn get_repository_path(&self) -> Result<PathBuf> {
        match &self.cmd {
            Command::Init { lang: _ } => Ok(current_dir()?),
            Command::Code { path } => Ok(absolute(path)?),
            Command::Shell { path: Some(path) } => Ok(absolute(path)?
                .parent()
                .with_context(|| format!("Configuration cannot be found as {:?}", path))?
                .to_owned()),
            Command::Shell { path: None } => Ok(current_dir()?),
        }
    }
}
