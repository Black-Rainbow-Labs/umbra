//! Umbra's entry point: parse the command line and hand off to a service.

mod cli;
mod models;
mod repositories;
mod services;
mod ui;
mod utils;

use clap::Parser;

use cli::{Cli, Command, McpArgs};
use services::{acp, cli_service, inspect_service, run_service, setup_service, update_service};

/// Runs the chosen subcommand and exits with its status code.
fn main() {
    let cli = Cli::parse();
    update_service::remove_old_binary();
    let result = match cli.command {
        None => run_service::run_task(&cli.run),
        Some(Command::Run(args)) => run_service::run_task(&args),
        Some(Command::Skills(args)) => inspect_service::list_skills(&args).map(|_| 0),
        Some(Command::Instructions(args)) => inspect_service::list_instructions(&args).map(|_| 0),
        Some(Command::Commands(args)) => inspect_service::list_commands(&args).map(|_| 0),
        Some(Command::Mcp(McpArgs { action: Some(action), .. })) => cli_service::mcp_command(&action),
        Some(Command::Mcp(args)) => inspect_service::list_mcp(&args).map(|_| 0),
        Some(Command::Config(args)) => cli_service::config_command(&args),
        Some(Command::Models(args)) => cli_service::list_models(&args),
        Some(Command::Session(args)) => cli_service::session_command(&args),
        Some(Command::Update(args)) => cli_service::update_command(&args),
        Some(Command::Prompt(args)) => run_service::print_prompt(&args).map(|_| 0),
        Some(Command::Sessions(args)) => inspect_service::list_sessions(&args).map(|_| 0),
        Some(Command::Resume(args)) => run_service::resume_command(&args),
        Some(Command::Acp(args)) if args.setup => setup_service::run_setup(&cli::SetupArgs::default()),
        Some(Command::Acp(args)) => acp::serve(&args),
        Some(Command::Setup(args)) => setup_service::run_setup(&args),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("umbra: {error:#}");
            std::process::exit(2);
        }
    }
}
