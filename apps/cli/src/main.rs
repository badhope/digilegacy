//! DigiLegacy CLI — local entry point over the shared application layer.
//!
//! The CLI is not a second backend: every subcommand calls the same
//! `Application` use case as HTTP. Writes require an explicit `--allow-write`
//! because the CLI is the local privileged surface; HTTP writes still need a
//! scoped token.

mod generated;

use clap::{Parser, Subcommand};
use digilegacy_application::Application;
use digilegacy_contracts as contracts;
use digilegacy_foundation::{Envelope, PlatformError};
use serde_json::{json, Value};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "digilegacy-cli", version, about = "DigiLegacy platform CLI")]
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
    /// Print store and schema health.
    Health {
        #[arg(long)]
        json: bool,
    },
    /// Read the platform overview.
    Overview {
        #[arg(long)]
        json: bool,
    },
    /// Asset registry.
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    /// Holder, heirs and trigger chain.
    Plan {
        #[arg(long)]
        json: bool,
    },
    /// Generate a deliverable export.
    Deliver {
        #[command(subcommand)]
        command: DeliverCommand,
    },
    /// Policy library.
    Policies {
        #[command(subcommand)]
        command: PoliciesCommand,
    },
    /// Audit hash-chain records.
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    /// Security status summary.
    Security {
        #[command(subcommand)]
        command: SecurityCommand,
    },
    /// Agent run history.
    Runs {
        #[arg(long, default_value = "all")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// Invoke one command through the shared application layer.
    Invoke {
        name: String,
        #[arg(long, default_value = "{}")]
        args: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AssetsCommand {
    /// List assets with server-side search and pagination.
    List {
        #[arg(long, default_value = "active")]
        status: String,
        #[arg(long, default_value = "")]
        q: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read one asset.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Create one asset (local privileged surface).
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long, default_value = "")]
        platform: String,
        #[arg(long, default_value = "")]
        value: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// Update one asset (local privileged surface).
    Update {
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long, default_value = "")]
        platform: String,
        #[arg(long, default_value = "")]
        value: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DeliverCommand {
    /// Export report/checklist/legal/toolkit as HTML.
    Export {
        #[arg(long)]
        kind: String,
        #[arg(long, default_value = "zh-CN")]
        locale: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PoliciesCommand {
    /// Search policies by category and keyword.
    List {
        #[arg(long, default_value = "")]
        category: String,
        #[arg(long, default_value = "")]
        q: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Read one policy and its provenance.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Record a policy provenance check (metadata only).
    Provenance {
        id: String,
        #[arg(long, value_parser = ["unverified", "source_cited", "verified", "disputed"])]
        status: String,
        #[arg(long, default_value = "")]
        source_url: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AuditCommand {
    /// List audit records (newest first).
    List {
        #[arg(long, default_value = "")]
        kind: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SecurityCommand {
    /// Read lock/recovery/key-source summary.
    Status {
        #[arg(long)]
        json: bool,
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
    let app = match Application::from_env() {
        Ok(app) => app,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(5);
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
        Commands::Health { json: as_json } => {
            if as_json {
                print_ok(app.health(), "ok");
            } else {
                println!(
                    "ok · schema {} · {} tables",
                    app.store().schema_version().unwrap_or(-1),
                    app.store().table_count().unwrap_or(-1)
                );
            }
            ExitCode::SUCCESS
        }
        Commands::Overview { json: as_json } => {
            run(&app, "app.overview", json!({}), as_json, false)
        }
        Commands::Assets { command } => match command {
            AssetsCommand::List {
                status,
                q,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "assets.list",
                json!({ "status": status, "q": q, "limit": limit, "cursor": cursor }),
                as_json,
                false,
            ),
            AssetsCommand::Get { id, json: as_json } => {
                run(&app, "assets.get", json!({ "id": id }), as_json, false)
            }
            AssetsCommand::Create {
                name,
                kind,
                platform,
                value,
                note,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "assets.create",
                json!({
                    "name": name, "kind": kind, "platform": platform,
                    "value": value, "note": note
                }),
                as_json,
                allow_write,
            ),
            AssetsCommand::Update {
                id,
                name,
                kind,
                platform,
                value,
                note,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "assets.update",
                json!({
                    "id": id, "name": name, "kind": kind, "platform": platform,
                    "value": value, "note": note
                }),
                as_json,
                allow_write,
            ),
        },
        Commands::Plan { json: as_json } => run(&app, "plan.get", json!({}), as_json, false),
        Commands::Deliver { command } => match command {
            DeliverCommand::Export {
                kind,
                locale,
                json: as_json,
            } => run(
                &app,
                "deliver.export",
                json!({ "kind": kind, "locale": locale }),
                as_json,
                false,
            ),
        },
        Commands::Policies { command } => match command {
            PoliciesCommand::List {
                category,
                q,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "policies.list",
                json!({ "category": category, "q": q, "limit": limit, "cursor": cursor }),
                as_json,
                false,
            ),
            PoliciesCommand::Get { id, json: as_json } => {
                run(&app, "policies.get", json!({ "id": id }), as_json, false)
            }
            PoliciesCommand::Provenance {
                id,
                status,
                source_url,
                note,
                json: as_json,
            } => run(
                &app,
                "policies.provenance.set",
                json!({ "id": id, "status": status, "sourceUrl": source_url, "note": note }),
                as_json,
                false,
            ),
        },
        Commands::Audit { command } => match command {
            AuditCommand::List {
                kind,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "audit.list",
                json!({ "kind": kind, "limit": limit, "cursor": cursor }),
                as_json,
                false,
            ),
        },
        Commands::Security { command } => match command {
            SecurityCommand::Status { json: as_json } => {
                run(&app, "security.status", json!({}), as_json, false)
            }
        },
        Commands::Runs {
            status,
            limit,
            json: as_json,
        } => run(
            &app,
            "agent.runs.list",
            json!({ "status": status, "limit": limit }),
            as_json,
            false,
        ),
        Commands::Invoke {
            name,
            args,
            allow_write,
            json: as_json,
        } => {
            let args: Value = match serde_json::from_str(&args) {
                Ok(value) => value,
                Err(error) => {
                    return fail(PlatformError::invalid_args(format!(
                        "--args 不是合法 JSON: {error}"
                    )));
                }
            };
            run(&app, &name, args, as_json, allow_write)
        }
    }
}

fn run(app: &Application, name: &str, args: Value, as_json: bool, allow_write: bool) -> ExitCode {
    let spec = match contracts::find_command(name) {
        Some(spec) => spec,
        None => return fail(PlatformError::not_found(&format!("命令 {name}"))),
    };
    let permission = digilegacy_foundation::Permission::parse(spec.permission);
    let needs_write = permission == Some(digilegacy_foundation::Permission::Write);
    if needs_write && !allow_write {
        return fail(PlatformError::new(
            "FORBIDDEN",
            "写命令需要 --allow-write；这是本地特权入口，HTTP 写入另需 token",
        ));
    }
    match app.dispatch(name, args) {
        Ok(data) => {
            if as_json {
                print_ok(data, "ok");
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_default()
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => fail(error),
    }
}

fn print_ok(data: Value, message: &str) {
    let envelope = Envelope::ok(data);
    let mut value = serde_json::to_value(envelope).unwrap_or_else(|_| json!({}));
    value["message"] = json!(message);
    println!("{}", serde_json::to_string_pretty(&value).unwrap());
}

fn fail(error: PlatformError) -> ExitCode {
    let envelope = Envelope::<Value>::err(error.clone());
    println!("{}", serde_json::to_string_pretty(&envelope).unwrap());
    ExitCode::from(error.cli_exit_code().clamp(0, 255) as u8)
}
