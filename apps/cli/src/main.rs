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
use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;
use zeroize::Zeroizing;

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
        #[command(subcommand)]
        command: Option<PlanCommand>,
        #[arg(long, global = true)]
        json: bool,
    },
    /// Trigger chain and check-in.
    Trigger {
        #[command(subcommand)]
        command: TriggerCommand,
    },
    /// Trigger final notification outbox.
    Notifications {
        #[command(subcommand)]
        command: NotificationsCommand,
    },
    /// External connections and health probes.
    Connections {
        #[command(subcommand)]
        command: ConnectionsCommand,
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
    /// Agent approvals.
    Approvals {
        #[command(subcommand)]
        command: ApprovalsCommand,
    },
    /// Background jobs.
    Jobs {
        #[command(subcommand)]
        command: JobsCommand,
    },
    /// Sanitized structural diagnostics and audit-chain state.
    Diagnostics {
        #[command(subcommand)]
        command: DiagnosticsCommand,
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
    /// Agent usage rollup.
    Usage {
        #[command(subcommand)]
        command: UsageCommand,
    },
    /// Agent run history and runtime.
    Runs {
        #[command(subcommand)]
        command: RunsCommand,
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
enum NotificationsCommand {
    /// List outbox entries.
    List {
        #[arg(long, default_value = "all")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Flush pending entries through the configured notification channel.
    Send {
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        retry_failed: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ConnectionsCommand {
    /// List connection metadata and last health.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Encrypt and save a connection config.
    Save {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        security: Option<String>,
        #[arg(long)]
        disabled: bool,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// Delete a saved connection; environment channels are immutable.
    Delete {
        id: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// Probe a connection and persist its health.
    Test {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        id: Option<i64>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum JobsCommand {
    /// Read one job status, progress and result.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Cancel a queued job.
    Cancel {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum UsageCommand {
    /// Summarize plan tokens, cost and run count.
    Summary {
        #[arg(long, default_value_t = 30)]
        days: i64,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RunsCommand {
    /// List runs (default all).
    List {
        #[arg(long, default_value = "all")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// Read one run with its step timeline.
    Get {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Plan a task with the configured OpenAI-compatible model.
    Plan {
        #[arg(long)]
        task: String,
        #[arg(long, default_value_t = 20)]
        max_steps: usize,
        #[arg(long)]
        json: bool,
    },
    /// Start a deterministic run. steps is a JSON array.
    Start {
        #[arg(long)]
        task: String,
        #[arg(long)]
        steps: String,
        #[arg(long, default_value_t = 20)]
        max_steps: usize,
        #[arg(long, default_value_t = 5)]
        max_write_steps: usize,
        #[arg(long)]
        json: bool,
    },
    /// Cancel a non-terminal run and expire its pending approvals.
    Cancel {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Retry a failed/rejected/cancelled run as a new run.
    Retry {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    /// Read the current plan.
    Get,
    /// Update the holder display name.
    Profile {
        #[arg(long)]
        name: String,
        #[arg(long)]
        allow_write: bool,
    },
    /// Replace the heir list from a JSON file containing {"heirs":[...]}.
    Heirs {
        #[arg(long)]
        file: String,
        #[arg(long)]
        allow_write: bool,
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
        /// Optional asset kind, e.g. account or real_estate.
        #[arg(long, default_value = "")]
        kind: String,
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
        #[arg(long)]
        platform: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        value_status: Option<String>,
        #[arg(long)]
        identifier: Option<String>,
        #[arg(long)]
        ownership_share: Option<String>,
        #[arg(long)]
        beneficiary: Option<String>,
        #[arg(long)]
        beneficiary_contact: Option<String>,
        #[arg(long)]
        credential_location: Option<String>,
        #[arg(long)]
        expires_at: Option<String>,
        #[arg(long)]
        quantity: Option<String>,
        #[arg(long)]
        unit: Option<String>,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// Update one asset (local privileged surface; omitted fields are kept).
    Update {
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        value_status: Option<String>,
        #[arg(long)]
        identifier: Option<String>,
        #[arg(long)]
        ownership_share: Option<String>,
        #[arg(long)]
        beneficiary: Option<String>,
        #[arg(long)]
        beneficiary_contact: Option<String>,
        #[arg(long)]
        credential_location: Option<String>,
        #[arg(long)]
        expires_at: Option<String>,
        #[arg(long)]
        quantity: Option<String>,
        #[arg(long)]
        unit: Option<String>,
        /// active | archived; empty keeps the current status.
        #[arg(long, default_value = "")]
        status: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// List curated asset suggestions, filtered by category or query.
    Discover {
        #[arg(long, default_value = "")]
        category: String,
        #[arg(long, default_value = "")]
        q: String,
        #[arg(long)]
        json: bool,
    },
    /// Permanently delete an archived asset. Requires --confirm.
    Delete {
        id: String,
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// Create multiple assets from a JSON file containing {"items":[...]}.
    BulkCreate {
        #[arg(long)]
        file: String,
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
        /// Enqueue the export and return a job id instead of waiting.
        #[arg(long)]
        background: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum TriggerCommand {
    /// Record a safety check-in and reset the trigger timer.
    Checkin {
        #[arg(long)]
        json: bool,
    },
    /// Enable/disable DMS or change the check-in interval.
    Configure {
        #[arg(long)]
        enabled: bool,
        #[arg(long, conflicts_with = "enabled")]
        disabled: bool,
        #[arg(long)]
        days: Option<i64>,
        #[arg(long)]
        pause_days: Option<i64>,
        #[arg(long)]
        json: bool,
    },
    /// Evaluate the trigger chain and advance waiting stages.
    Evaluate {
        #[arg(long)]
        json: bool,
    },
    /// List trigger events.
    Events {
        #[arg(long, default_value = "")]
        cursor: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
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
    /// Pull a remote policy feed into candidates without changing the library.
    Sync {
        #[arg(long, default_value = "")]
        since: String,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
    /// List policy sync candidates and their field diffs.
    Candidates {
        #[arg(long, default_value = "pending")]
        status: String,
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Approve or reject one policy sync candidate.
    Decide {
        id: String,
        #[arg(long, value_parser = ["approve", "reject"])]
        action: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        allow_write: bool,
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
        allow_write: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ApprovalsCommand {
    /// List approvals (default pending).
    List {
        #[arg(long, default_value = "pending")]
        status: String,
        #[arg(long)]
        json: bool,
    },
    /// Approve or reject one pending Agent write.
    Decide {
        id: String,
        #[arg(long, value_parser = ["approve", "reject"])]
        decision: String,
        #[arg(long, default_value = "")]
        reason: String,
        /// Queue the resume as a durable background job instead of waiting.
        #[arg(long)]
        background: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum DiagnosticsCommand {
    /// Export a sanitized diagnostics document.
    Export {
        #[arg(long)]
        allow_write: bool,
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
    /// Set the first master password. Reads it from stdin.
    Init {
        #[arg(long)]
        json: bool,
    },
    /// Unlock this process with the master password. Reads it from stdin.
    Unlock {
        #[arg(long)]
        json: bool,
    },
    /// Unlock this process with the one-time recovery code. Reads it from stdin.
    Recover {
        #[arg(long)]
        json: bool,
    },
    /// Clear the in-memory data key for this process.
    Lock {
        #[arg(long)]
        json: bool,
    },
    /// Create an authenticated checkpoint beside the live database.
    Checkpoint {
        #[arg(long, default_value = "manual")]
        label: String,
        #[arg(long)]
        json: bool,
    },
    /// List checkpoints with their paths and timestamps.
    Checkpoints {
        #[arg(long)]
        json: bool,
    },
    /// Verify one checkpoint without writing.
    Verify {
        id: String,
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
    if !security_passthrough(&command) {
        if let Err(error) = unlock_cli_vault(&app) {
            eprintln!("{}: {}", error.code, error.message);
            return ExitCode::from(5);
        }
    }

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
                kind,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "assets.list",
                json!({ "status": status, "q": q, "kind": if kind.is_empty() { None } else { Some(kind) }, "limit": limit, "cursor": cursor }),
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
                value_status,
                identifier,
                ownership_share,
                beneficiary,
                beneficiary_contact,
                credential_location,
                expires_at,
                quantity,
                unit,
                allow_write,
                json: as_json,
            } => {
                let mut args = json!({ "name": name, "kind": kind });
                for (key, value) in [
                    ("platform", platform),
                    ("value", value),
                    ("note", note),
                    ("valueStatus", value_status),
                    ("identifier", identifier),
                    ("ownershipShare", ownership_share),
                    ("beneficiary", beneficiary),
                    ("beneficiaryContact", beneficiary_contact),
                    ("credentialLocation", credential_location),
                    ("expiresAt", expires_at),
                    ("quantity", quantity),
                    ("unit", unit),
                ] {
                    if let Some(value) = value {
                        args[key] = json!(value);
                    }
                }
                run(&app, "assets.create", args, as_json, allow_write)
            }
            AssetsCommand::Update {
                id,
                name,
                kind,
                platform,
                value,
                note,
                value_status,
                identifier,
                ownership_share,
                beneficiary,
                beneficiary_contact,
                credential_location,
                expires_at,
                quantity,
                unit,
                status,
                allow_write,
                json: as_json,
            } => {
                let mut args = json!({
                    "id": id, "name": name, "kind": kind,
                    "status": if status.is_empty() { None } else { Some(status) }
                });
                for (key, value) in [
                    ("platform", platform),
                    ("value", value),
                    ("note", note),
                    ("valueStatus", value_status),
                    ("identifier", identifier),
                    ("ownershipShare", ownership_share),
                    ("beneficiary", beneficiary),
                    ("beneficiaryContact", beneficiary_contact),
                    ("credentialLocation", credential_location),
                    ("expiresAt", expires_at),
                    ("quantity", quantity),
                    ("unit", unit),
                ] {
                    if let Some(value) = value {
                        args[key] = json!(value);
                    }
                }
                run(&app, "assets.update", args, as_json, allow_write)
            }
            AssetsCommand::Delete {
                id,
                confirm,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "assets.delete",
                json!({ "id": id, "confirm": confirm }),
                as_json,
                allow_write,
            ),
            AssetsCommand::Discover {
                category,
                q,
                json: as_json,
            } => run(
                &app,
                "assets.discover",
                json!({ "category": category, "q": q }),
                as_json,
                false,
            ),
            AssetsCommand::BulkCreate {
                file,
                allow_write,
                json: as_json,
            } => {
                let raw = match std::fs::read_to_string(&file) {
                    Ok(raw) => raw,
                    Err(error) => {
                        return fail(PlatformError::new(
                            "INVALID_ARGS",
                            format!("读取批量文件失败: {error}"),
                        ))
                    }
                };
                let args: Value = match serde_json::from_str(&raw) {
                    Ok(args) => args,
                    Err(error) => {
                        return fail(PlatformError::new(
                            "INVALID_ARGS",
                            format!("批量文件不是合法 JSON: {error}"),
                        ))
                    }
                };
                run(&app, "assets.bulkCreate", args, as_json, allow_write)
            }
        },
        Commands::Plan {
            command,
            json: as_json,
        } => match command {
            None | Some(PlanCommand::Get) => run(&app, "plan.get", json!({}), as_json, false),
            Some(PlanCommand::Profile { name, allow_write }) => run(
                &app,
                "plan.profile.set",
                json!({ "name": name }),
                as_json,
                allow_write,
            ),
            Some(PlanCommand::Heirs { file, allow_write }) => {
                let raw = match std::fs::read_to_string(&file) {
                    Ok(raw) => raw,
                    Err(error) => {
                        return fail(PlatformError::new(
                            "INVALID_ARGS",
                            format!("读取继承人文件失败: {error}"),
                        ))
                    }
                };
                let args: Value = match serde_json::from_str(&raw) {
                    Ok(args) => args,
                    Err(error) => {
                        return fail(PlatformError::new(
                            "INVALID_ARGS",
                            format!("继承人文件不是合法 JSON: {error}"),
                        ))
                    }
                };
                run(&app, "plan.heirs.set", args, as_json, allow_write)
            }
        },
        Commands::Trigger { command } => match command {
            TriggerCommand::Checkin { json: as_json } => {
                run(&app, "trigger.checkin", json!({}), as_json, true)
            }
            TriggerCommand::Configure {
                enabled,
                disabled,
                days,
                pause_days,
                json: as_json,
            } => {
                let mut args = json!({});
                if enabled {
                    args["enabled"] = json!(true);
                } else if disabled {
                    args["enabled"] = json!(false);
                }
                if let Some(days) = days {
                    args["days"] = json!(days);
                }
                if let Some(pause_days) = pause_days {
                    args["pauseDays"] = json!(pause_days);
                }
                run(&app, "trigger.configure", args, as_json, true)
            }
            TriggerCommand::Evaluate { json: as_json } => {
                run(&app, "trigger.evaluate", json!({}), as_json, true)
            }
            TriggerCommand::Events {
                cursor,
                limit,
                json: as_json,
            } => run(
                &app,
                "trigger.events.list",
                json!({ "cursor": if cursor.is_empty() { None } else { Some(cursor) }, "limit": limit }),
                as_json,
                false,
            ),
        },
        Commands::Connections { command } => match command {
            ConnectionsCommand::List { json: as_json } => {
                run(&app, "connections.list", json!({}), as_json, false)
            }
            ConnectionsCommand::Save {
                kind,
                endpoint,
                id,
                label,
                token,
                username,
                from,
                security,
                disabled,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "connections.upsert",
                json!({
                    "kind": kind,
                    "endpoint": endpoint,
                    "id": id,
                    "label": label,
                    "token": token,
                    "username": username,
                    "from": from,
                    "security": security,
                    "enabled": !disabled
                }),
                as_json,
                allow_write,
            ),
            ConnectionsCommand::Delete {
                id,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "connections.delete",
                json!({ "id": id }),
                as_json,
                allow_write,
            ),
            ConnectionsCommand::Test {
                kind,
                id,
                label,
                endpoint,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "connections.test",
                json!({
                    "kind": kind,
                    "id": id,
                    "label": label,
                    "endpoint": endpoint
                }),
                as_json,
                allow_write,
            ),
        },
        Commands::Notifications { command } => match command {
            NotificationsCommand::List {
                status,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "notifications.list",
                json!({ "status": status, "limit": limit, "cursor": cursor }),
                as_json,
                false,
            ),
            NotificationsCommand::Send {
                allow_write,
                retry_failed,
                json: as_json,
            } => run(
                &app,
                "notifications.send",
                json!({ "retryFailed": retry_failed }),
                as_json,
                allow_write,
            ),
        },
        Commands::Deliver { command } => match command {
            DeliverCommand::Export {
                kind,
                locale,
                background,
                json: as_json,
            } => run(
                &app,
                "deliver.export",
                json!({ "kind": kind, "locale": locale, "wait": !background }),
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
            PoliciesCommand::Sync {
                since,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "policies.sync",
                json!({ "since": since }),
                as_json,
                allow_write,
            ),
            PoliciesCommand::Candidates {
                status,
                limit,
                cursor,
                json: as_json,
            } => run(
                &app,
                "policies.candidates.list",
                json!({ "status": status, "limit": limit, "cursor": cursor }),
                as_json,
                false,
            ),
            PoliciesCommand::Decide {
                id,
                action,
                note,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "policies.candidate.decide",
                json!({ "id": id, "action": action, "note": note }),
                as_json,
                allow_write,
            ),
            PoliciesCommand::Provenance {
                id,
                status,
                source_url,
                note,
                allow_write,
                json: as_json,
            } => run(
                &app,
                "policies.provenance.set",
                json!({ "id": id, "status": status, "sourceUrl": source_url, "note": note }),
                as_json,
                allow_write,
            ),
        },
        Commands::Approvals { command } => match command {
            ApprovalsCommand::List {
                status,
                json: as_json,
            } => run(
                &app,
                "agent.approvals.list",
                json!({ "status": status }),
                as_json,
                false,
            ),
            ApprovalsCommand::Decide {
                id,
                decision,
                reason,
                background,
                json: as_json,
            } => run(
                &app,
                "agent.approvals.decide",
                json!({ "id": id, "decision": decision, "reason": reason, "background": background }),
                as_json,
                true,
            ),
        },
        Commands::Jobs { command } => match command {
            JobsCommand::Get { id, json: as_json } => {
                run(&app, "jobs.get", json!({ "id": id }), as_json, false)
            }
            JobsCommand::Cancel { id, json: as_json } => {
                run(&app, "jobs.cancel", json!({ "id": id }), as_json, false)
            }
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
        Commands::Diagnostics { command } => match command {
            DiagnosticsCommand::Export {
                allow_write,
                json: as_json,
            } => {
                if !allow_write {
                    return fail(PlatformError::new(
                        "FORBIDDEN",
                        "诊断导出会写入审计记录；请加 --allow-write",
                    ));
                }
                local(app.diagnostics_export(), as_json)
            }
        },
        Commands::Security { command } => match command {
            SecurityCommand::Status { json: as_json } => {
                run(&app, "security.status", json!({}), as_json, false)
            }
            SecurityCommand::Init { json: as_json } => match read_password() {
                Ok(password) => local(app.security_init(&password), as_json),
                Err(error) => fail(error),
            },
            SecurityCommand::Unlock { json: as_json } => match read_password() {
                Ok(password) => run(
                    &app,
                    "security.unlock",
                    json!({ "password": password.as_str() }),
                    as_json,
                    false,
                ),
                Err(error) => fail(error),
            },
            SecurityCommand::Recover { json: as_json } => match read_recovery_code() {
                Ok(code) => run(
                    &app,
                    "security.recover",
                    json!({ "code": code.as_str() }),
                    as_json,
                    false,
                ),
                Err(error) => fail(error),
            },
            SecurityCommand::Lock { json: as_json } => {
                run(&app, "security.lock", json!({}), as_json, false)
            }
            SecurityCommand::Checkpoint {
                label,
                json: as_json,
            } => local(app.security_checkpoint(&label), as_json),
            SecurityCommand::Checkpoints { json: as_json } => {
                local(app.security_checkpoints(), as_json)
            }
            SecurityCommand::Verify { id, json: as_json } => {
                local(app.security_verify_checkpoint(&id), as_json)
            }
        },
        Commands::Usage { command } => match command {
            UsageCommand::Summary {
                days,
                json: as_json,
            } => run(
                &app,
                "agent.usage.summary",
                json!({ "days": days }),
                as_json,
                false,
            ),
        },
        Commands::Runs { command } => match command {
            RunsCommand::List {
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
            RunsCommand::Get { id, json: as_json } => {
                run(&app, "agent.runs.get", json!({ "id": id }), as_json, false)
            }
            RunsCommand::Plan {
                task,
                max_steps,
                json: as_json,
            } => run(
                &app,
                "agent.runs.plan",
                json!({ "task": task, "maxSteps": max_steps }),
                as_json,
                false,
            ),
            RunsCommand::Start {
                task,
                steps,
                max_steps,
                max_write_steps,
                json: as_json,
            } => {
                let steps: Value = match serde_json::from_str(&steps) {
                    Ok(value) => value,
                    Err(error) => {
                        return fail(PlatformError::invalid_args(format!(
                            "--steps 不是合法 JSON: {error}"
                        )));
                    }
                };
                run(
                    &app,
                    "agent.runs.start",
                    json!({
                        "task": task,
                        "steps": steps,
                        "maxSteps": max_steps,
                        "maxWriteSteps": max_write_steps
                    }),
                    as_json,
                    true,
                )
            }
            RunsCommand::Cancel { id, json: as_json } => run(
                &app,
                "agent.runs.cancel",
                json!({ "id": id }),
                as_json,
                true,
            ),
            RunsCommand::Retry { id, json: as_json } => {
                run(&app, "agent.runs.retry", json!({ "id": id }), as_json, true)
            }
        },
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

fn security_passthrough(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Security {
            command: SecurityCommand::Init { .. }
                | SecurityCommand::Unlock { .. }
                | SecurityCommand::Recover { .. }
                | SecurityCommand::Status { .. }
                | SecurityCommand::Lock { .. }
        }
    )
}

/// A configured vault follows the process, so a standalone `security unlock`
/// cannot help the next CLI invocation. Read the master password for this
/// process from `DIGILEGACY_VAULT_PASSWORD`, or prompt when stdin is a terminal.
fn unlock_cli_vault(app: &Application) -> Result<(), PlatformError> {
    let status = app.security_status()?;
    let configured = status["configured"].as_bool().unwrap_or(false);
    let locked = status["locked"].as_bool().unwrap_or(false);
    if !configured || !locked {
        return Ok(());
    }
    let password = match std::env::var("DIGILEGACY_VAULT_PASSWORD") {
        Ok(value) if !value.is_empty() => Zeroizing::new(value),
        _ if std::io::stdin().is_terminal() => {
            eprintln!("保险库已锁定，请输入主密码：");
            read_password()?
        }
        _ => {
            return Err(PlatformError::new(
                "VAULT_LOCKED",
                "保险库已锁定；请在终端输入主密码，或设置 DIGILEGACY_VAULT_PASSWORD",
            ));
        }
    };
    app.security_unlock(json!({ "password": password.as_str() }))?;
    Ok(())
}

fn read_password() -> Result<Zeroizing<String>, PlatformError> {
    read_secret("主密码")
}

fn read_recovery_code() -> Result<Zeroizing<String>, PlatformError> {
    read_secret("恢复码")
}

fn read_secret(label: &str) -> Result<Zeroizing<String>, PlatformError> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| PlatformError::new("IO", format!("读取{label}失败：{e}")))?;
    let value = line.trim_end_matches(['\r', '\n']).to_string();
    if value.is_empty() {
        return Err(PlatformError::new(
            "INVALID_ARGS",
            format!("{label}不能为空"),
        ));
    }
    Ok(Zeroizing::new(value))
}

fn local(result: Result<Value, PlatformError>, as_json: bool) -> ExitCode {
    match result {
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
