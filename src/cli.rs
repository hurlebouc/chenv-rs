use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Optional name to operate on
    pub name: Option<String>,
}

pub fn get_cli() -> Cli {
    Cli::parse()
}
