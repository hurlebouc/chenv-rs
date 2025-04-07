use std::{
    env::{self, join_paths, split_paths},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use config::Conf;
use init::JavaBuildTool;
mod cli;
mod config;
mod init;
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
    fn get_shell(&self) -> &'static str {
        match self {
            Os::Linux => "bash",
            Os::MacOS => "bash",
            Os::Windows => "cmd",
        }
    }
    fn get_code(&self) -> &'static str {
        match self {
            Os::Linux => "code",
            Os::MacOS => "code",
            Os::Windows => "code.cmd",
        }
    }
    fn get_path(&self) -> &'static str {
        match self {
            Os::Linux => "PATH",
            Os::Windows => "Path",
            Os::MacOS => "PATH",
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
            set_shell(&mut cmd, &conf, path)?;
            cmd.status().expect("shell failed to start");
        }
        cli::Command::Init {
            lang:
                cli::Lang::Java {
                    version,
                    sbt,
                    no_build_tool,
                },
        } => {
            let jbt_opt = match (sbt, no_build_tool) {
                (true, true) => {
                    bail!("Cannot use sbt and no-build-tool at the same time. Please choose one.")
                }
                (true, false) => Some(JavaBuildTool::Sbt),
                (false, true) => None,
                (false, false) => Some(JavaBuildTool::Maven),
            };
            let conf = Conf::init_java(*version, &jbt_opt)?;
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
            set_shell(&mut cmd, &conf, &args.get_repository_path()?)?;
            cmd.status().expect("shell failed to start");
        }
    }
    Ok(())
}

fn set_shell(cmd: &mut Command, conf: &config::Conf, config_parent: &Path) -> Result<()> {
    if let Some(shell) = &conf.shell {
        let interpolation_env = shell.ensure_resources(config_parent)?;
        for (k, v) in shell.get_env(&interpolation_env)? {
            cmd.env(k, v);
        }
        let mut paths = shell
            .get_path(&interpolation_env)?
            .into_iter()
            .map(|s| PathBuf::from(s))
            .collect::<Vec<_>>();
        let mut init_paths = match env::var_os(Os::get().get_path()) {
            Some(path) => split_paths(&path).collect(),
            None => vec![],
        };
        paths.append(&mut init_paths);
        let path_new = join_paths(paths)?;
        cmd.env(Os::get().get_path(), path_new);
    }
    Ok(())
}
