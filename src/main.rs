mod app;
mod cgroup;
mod cli;
mod config;
mod display_isolation;
mod gnome;
mod ipc;
mod logging;
mod video;
mod wayland;
mod wine;
mod wm_visibility;
mod x11;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, ControlAction};
use ipc::ControlCommand;

fn main() -> Result<()> {
    logging::init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run { config } => app::run(config.as_deref()),
        Command::Switch { config } => ipc::send_switch_config(&config),
        Command::Doctor => app::doctor(),
        Command::PrintConfig { config } => {
            let cfg = config::Config::load(config.as_deref())?;
            println!("{}", cfg.to_toml_pretty()?);
            Ok(())
        }
        Command::Ctl { action } => match action {
            ControlAction::Stop => ipc::send_command(ControlCommand::Stop),
            ControlAction::Pause => ipc::send_command(ControlCommand::Pause),
            ControlAction::Resume => ipc::send_command(ControlCommand::Resume),
            ControlAction::Reload => ipc::send_command(ControlCommand::Reload),
            ControlAction::HideWindow => ipc::send_command(ControlCommand::HideWindow),
            ControlAction::ShowWindow => ipc::send_command(ControlCommand::ShowWindow),
            ControlAction::Status => {
                println!("{}", ipc::request_running_config()?);
                Ok(())
            }
        },
    }
}
