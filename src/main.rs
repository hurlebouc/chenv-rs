use std::process::Command;
mod cli;
mod config;

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
    let conf = config::read_config();
    let mut cmd = Command::new(get_shell());
    for (k, v) in &conf.env {
        cmd.env(k, v);
    }
    cmd.status().expect("ls command failed to start");
}
