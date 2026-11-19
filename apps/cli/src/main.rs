//! DigiLegacy CLI — M0 skeleton.
//!
//! The CLI is a local entry point, not a second backend. It reads the same
//! generated command manifest and dispatches through the same CommandBus.
//! M0 exposes discovery + generic invoke; nested commands are generated help,
//! then wired one by one in M2/M3.

mod generated;

use clap::{Parser, Subcommand};
use digilegacy_contracts as contracts;
use digilegacy_foundation::{CommandBus, PlatformError};
use serde_json::{json, Value};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "digilegacy-cli",
    version,
    about = "DigiLegacy platform CLI (M0 skeleton)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List commands from the generated contract manifest.
    #[command(name = "commands")]
    Registry {
        #[arg(long)]
        json: bool,
    },
    /// Print a local health envelope.
    Health,
    /// Invoke one command through the shared CommandBus.
    Invoke {
        name: String,
        #[arg(long, default_value = "{}")]
        args: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let command = match cli.command {
        Some(command) => command,
        None => {
            generated::print_help();
            return ExitCode::SUCCESS;
        }
    };
    match command {
        Commands::Registry { json: as_json } => {
            if as_json {
                let items: Vec<Value> = contracts::COMMANDS
                    .iter()
                    .map(|command| {
                        json!({
                            "name": command.name,
                            "domain": command.domain,
                            "permission": command.permission,
                            "http": command.http,
                            "cli": command.name,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "commands": items })).unwrap()
                );
            } else {
                for command in contracts::COMMANDS {
                    println!(
                        "{:<28} {:<10} {}",
                        command.name, command.permission, command.http
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Commands::Health => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "success": true,
                    "code": "OK",
                    "message": "ok",
                    "data": { "status": "ok", "mode": "m0", "contracts": contracts::COMMANDS.len() },
                    "traceId": digilegacy_foundation::trace_id(),
                }))
                .unwrap()
            );
            ExitCode::SUCCESS
        }
        Commands::Invoke { name, args } => {
            let args: Value = match serde_json::from_str(&args) {
                Ok(value) => value,
                Err(error) => {
                    let platform_error =
                        PlatformError::invalid_args(format!("--args 不是合法 JSON: {error}"));
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&json!({
                            "success": false,
                            "code": platform_error.code,
                            "message": platform_error.message,
                            "data": null,
                            "traceId": digilegacy_foundation::trace_id(),
                        }))
                        .unwrap()
                    );
                    return ExitCode::from(2);
                }
            };
            let bus = CommandBus::new();
            let envelope = bus.dispatch(&name, args);
            println!("{}", serde_json::to_string_pretty(&envelope).unwrap());
            if envelope.success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(error_exit_code(&envelope.code))
            }
        }
    }
}

fn error_exit_code(code: &str) -> u8 {
    digilegacy_contracts::find_error(code)
        .map(|spec| spec.cli_exit.clamp(0, 255) as u8)
        .unwrap_or(5)
}
