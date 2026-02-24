use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "we-layerd", version, about = "Wallpaper Engine layer daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run daemon with a configuration file
    Run {
        /// Path to TOML config file
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Print environment diagnostics
    Doctor,
    /// Print the effective config as TOML
    PrintConfig {
        /// Path to TOML config file
        #[arg(long)]
        config: Option<PathBuf>,
    },
}
