//! Umbra's entry point: parse the command line and hand off to a service.

mod cli;
mod models;
mod repositories;
mod services;
mod utils;

use clap::Parser;

use cli::{Cli, Command};
use services::{inspect_service, run_service};

/// Runs the chosen subcommand and exits with its status code.
fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        None => run_service::run_task(&cli.run),
        Some(Command::Run(args)) => run_service::run_task(&args),
        Some(Command::Skills(args)) => inspect_service::list_skills(&args).map(|_| 0),
        Some(Command::Instructions(args)) => inspect_service::list_instructions(&args).map(|_| 0),
        Some(Command::Commands(args)) => inspect_service::list_commands(&args).map(|_| 0),
        Some(Command::Mcp(args)) => inspect_service::list_mcp(&args).map(|_| 0),
        Some(Command::Config(args)) => inspect_service::show_config(&args).map(|_| 0),
        Some(Command::Prompt(args)) => run_service::print_prompt(&args).map(|_| 0),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("umbra: {error:#}");
            std::process::exit(2);
        }
    }
}
