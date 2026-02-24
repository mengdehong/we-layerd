mod app;
mod cli;
mod config;
mod logging;
mod wayland;
mod wine;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    logging::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config } => app::run(config.as_deref()),
        Command::Doctor => {
            app::doctor();
            Ok(())
        }
        Command::PrintConfig { config } => {
            let cfg = config::Config::load(config.as_deref())?;
            println!("{}", cfg.to_toml_pretty()?);
            Ok(())
        }
    }
}
