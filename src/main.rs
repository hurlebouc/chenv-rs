use std::{
    env::current_dir,
    path::{Path, PathBuf, absolute},
    process::Command,
};

use anyhow::Result;
mod cli;
mod config;
mod interpol;
mod resources;

fn get_shell() -> String {
    let os = std::env::consts::OS;
    match os {
        "linux" => "bash".to_string(),
        "macos" => "bash".to_string(),
        "windows" => "cmd".to_string(),
        _ => "bash".to_string(),
    }
}

fn get_code() -> String {
    let os = std::env::consts::OS;
    match os {
        "linux" => "code".to_string(),
        "macos" => "code".to_string(),
        "windows" => "code.cmd".to_string(),
        _ => "code".to_string(),
    }
}

fn main() -> Result<()> {
    let args = cli::get_cli();
    match &args.cmd {
        cli::Command::Code { path } => {
            let mut cmd = Command::new(get_code());
            cmd.arg("-n").arg("--wait").arg(&path);
            let conf = config::read_config_in_repo(&path)?;
            set_command(&mut cmd, &conf, &path)?;
            cmd.status().expect("shell failed to start");
        }
        cli::Command::Init { list } => todo!("Not yet implemented!"),
        cli::Command::Shell { path } => {
            let conf = match path {
                Some(path) => config::read_config(&path)?,
                None => config::read_config_in_repo(&args.get_repository_path()?)?,
            };
            let mut cmd = Command::new(get_shell());
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
