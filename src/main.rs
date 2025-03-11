use std::process::Command;
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

fn main() {
    let args = cli::get_cli();
    let conf = config::read_config(&args.conf_path);
    let mut cmd = Command::new(get_shell());
    if let Some(shell) = &conf.shell {
        if let Some(e) = &shell.env {
            for (k, v) in e {
                cmd.env(k, v);
            }
        }
    }
    cmd.status().expect("ls command failed to start");
}
