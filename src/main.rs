mod app;
mod cli;
mod config;
mod logging;
mod video;
mod wayland;
mod wine;
mod x11;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    logging::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config } => app::run(config.as_deref()),
        Command::Doctor => app::doctor(),
        Command::PrintConfig { config } => {
            let cfg = config::Config::load(config.as_deref())?;
            println!("{}", cfg.to_toml_pretty()?);
            Ok(())
        }
    }
}
