use std::{
    io::Write,
    path::Path,
    process::Command,
};

use anyhow::Result;
use config::Conf;
mod cli;
mod config;
mod interpol;
mod resources;
#[derive(Debug, Clone, Copy)]
enum Os {
    Linux,
    Windows,
    MacOS,
}

impl Os {
    fn get() -> Self {
        let os = std::env::consts::OS;
        match os {
            "linux" => Os::Linux,
            "windows" => Os::Windows,
            "macos" => Os::MacOS,
            _ => panic!("Unsupported OS {}", os),
        }
    }
    fn get_shell(&self) -> String {
        match self {
            Os::Linux => "bash".to_string(),
            Os::MacOS => "bash".to_string(),
            Os::Windows => "cmd".to_string(),
        }
    }
    fn get_code(&self) -> String {
        match self {
            Os::Linux => "code".to_string(),
            Os::MacOS => "code".to_string(),
            Os::Windows => "code.cmd".to_string(),
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let os = Os::get();
    let args = cli::get_cli();
    match &args.cmd {
        cli::Command::Code { path } => {
            let mut cmd = Command::new(os.get_code());
            cmd.arg("-n").arg("--wait").arg(path);
            let conf = config::read_config_in_repo(path)?;
            set_command(&mut cmd, &conf, path)?;
            cmd.status().expect("shell failed to start");
        }
        cli::Command::Init {
            lang: cli::Lang::Java { version },
        } => {
            let conf = Conf::init_java(&os, *version)?;
            let yaml = serde_yaml::to_string(&conf)?;
            let mut file = std::fs::File::create("chenv.yaml")?;
            file.write_all(yaml.as_bytes())?;
        }
        cli::Command::Shell { path } => {
            let conf = match path {
                Some(path) => config::read_config(path)?,
                None => config::read_config_in_repo(&args.get_repository_path()?)?,
            };
            let mut cmd = Command::new(os.get_shell());
            set_command(&mut cmd, &conf, &args.get_repository_path()?)?;
            cmd.status().expect("shell failed to start");
        }
    }
    Ok(())
}

fn set_command(cmd: &mut Command, conf: &config::Conf, config_parent: &Path) -> Result<()> {
    if let Some(shell) = &conf.shell {
        for (k, v) in shell.get_env(&shell.ensure_resources(config_parent)?)? {
            cmd.env(k, v);
        }
    }
    Ok(())
}
