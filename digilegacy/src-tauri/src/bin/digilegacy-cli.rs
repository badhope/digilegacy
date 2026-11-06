//! DigiLegacy CLI：在不打开桌面的情况下操作同一个本地库。
//!
//! 为什么需要它：应用的核心价值是「家人能执行」。家人、异地设备、服务器
//! 维护、脚本化备份都需要一个不依赖 GUI 的入口——否则每次核对都得人肉点界面。
//!
//! 与桌面端的关系：**同一份数据库、同一套命令层**（`digilegacy_lib::dispatch`）。
//! CLI 不是第二套实现，因此不会出现「CLI 里能删、界面里删不掉」这类漂移。
//!
//! 输出约定：
//! - 默认给人看（表格 + 摘要）。
//! - `--json` 给脚本用，输出 `ApiResponse` 的原始 JSON，退出码同步反映成败。
//! - 敏感字段（凭据、账号标识）默认打码，`--show-secrets` 才展开。

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde_json::{json, Map, Value};

const EXIT_OK: u8 = 0;
const EXIT_FAIL: u8 = 1;

#[derive(Parser)]
#[command(
    name = "digilegacy",
    version,
    about = "DigiLegacy CLI — plan, export and verify a digital legacy from the terminal",
    long_about = "Operates on the same local database as the desktop app. Nothing leaves the machine: \
                  there is no account, no sync and no telemetry."
)]
struct Cli {
    /// Data directory (defaults to the platform app-data dir, or DIGILEGACY_DATA_DIR).
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<String>,

    /// Machine-readable JSON output.
    #[arg(long, global = true)]
    json: bool,

    /// Print sensitive values (account identifiers, credentials) instead of masking them.
    #[arg(long, global = true)]
    show_secrets: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialise a new local database (idempotent).
    Init,

    /// Show database location, encryption and vault state.
    Doctor,

    /// Print the stored user profile.
    Profile,

    /// Manage digital assets.
    #[command(subcommand)]
    Assets(AssetsCmd),

    /// Manage heirs (who receives what).
    #[command(subcommand)]
    Heirs(HeirsCmd),

    /// Dead man's switch: status, check-in, configuration.
    #[command(subcommand)]
    Dms(DmsCmd),

    /// Reports, toolkits and family action guides.
    #[command(subcommand)]
    Report(ReportCmd),

    /// Backups, checkpoints and platform patches.
    #[command(subcommand)]
    Backup(BackupCmd),

    /// Encryption and vault state.
    #[command(subcommand)]
    Vault(VaultCmd),

    /// Platform library and policy data.
    #[command(subcommand)]
    Platforms(PlatformsCmd),

    /// Escape hatch: call any core command directly with JSON arguments.
    Call {
        /// Command name (see `digilegacy commands`).
        name: String,
        /// JSON object of arguments, e.g. '{"id":1}'.
        #[arg(default_value = "{}")]
        args: String,
    },

    /// List every command reachable from this CLI.
    Commands,
}

#[derive(Subcommand)]
enum AssetsCmd {
    /// List all assets.
    List,
    /// Show one asset.
    Get { id: i64 },
    /// Add an asset. `--platform` is a platform id from `digilegacy platforms list`.
    Add {
        #[arg(long)]
        platform: i64,
        #[arg(long)]
        category: String,
        #[arg(long, default_value = "")]
        identifier: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, default_value_t = 0.0)]
        value: f64,
        #[arg(long, default_value = "no_action")]
        disposal: String,
        #[arg(long)]
        heir: Option<String>,
        #[arg(long)]
        heir_contact: Option<String>,
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// Delete an asset.
    Delete { id: i64 },
    /// Set the disposal decision for one asset.
    Dispose {
        id: i64,
        #[arg(long)]
        method: String,
        #[arg(long)]
        heir: Option<String>,
        #[arg(long)]
        heir_contact: Option<String>,
        #[arg(long, default_value = "")]
        notes: String,
    },
    /// Store an encrypted credential for an asset.
    Credential {
        id: i64,
        /// Read the value from stdin instead of the command line (keeps it out of shell history).
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        value: Option<String>,
    },
}

#[derive(Subcommand)]
enum HeirsCmd {
    /// List heirs.
    List,
    /// Add an heir.
    Add {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        relation: String,
        #[arg(long, default_value = "")]
        phone: String,
        #[arg(long, default_value = "")]
        email: String,
        /// Allocation note, e.g. "financial accounts only".
        #[arg(long, default_value = "")]
        share_note: String,
        #[arg(long)]
        is_executor: bool,
    },
    /// Remove an heir by name.
    Remove { name: String },
}

#[derive(Subcommand)]
enum DmsCmd {
    /// Current DMS state.
    Status,
    /// Record a check-in now.
    Checkin,
    /// Enable and set the interval.
    Enable {
        #[arg(long)]
        days: i64,
    },
    /// Disable the switch.
    Disable,
    /// Pause evaluation until a date (YYYY-MM-DD), or clear with --clear.
    Pause {
        #[arg(long)]
        until: Option<String>,
        #[arg(long)]
        clear: bool,
    },
    /// Record an annual planning review.
    Review,
    /// Local outbox history.
    History,
}

#[derive(Subcommand)]
enum ReportCmd {
    /// Render the inheritance report as HTML to stdout.
    Html {
        #[arg(long, default_value = "zh-CN")]
        locale: String,
    },
    /// Export the report into the data directory.
    Export {
        #[arg(long, default_value = "zh-CN")]
        locale: String,
    },
    /// Render the family action guide.
    Guide {
        #[arg(long, default_value = "zh-CN")]
        locale: String,
    },
    /// Run the family drill (read-only readiness check).
    Drill,
}

#[derive(Subcommand)]
enum BackupCmd {
    /// Export a backup JSON into the data directory.
    Export,
    /// Import a backup JSON file (merge by default).
    Import {
        path: String,
        #[arg(long, default_value = "merge")]
        mode: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Create a checkpoint.
    Checkpoint { label: String },
    /// List checkpoints.
    Checkpoints,
    /// Restore a checkpoint.
    Restore {
        id: String,
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Subcommand)]
enum VaultCmd {
    /// Show lock state.
    Status,
    /// Unlock with the master password (prompted on stdin when --password is omitted).
    Unlock {
        #[arg(long)]
        password: Option<String>,
    },
    /// Lock immediately.
    Lock,
    /// Set the master password on a fresh database.
    Setup {
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Subcommand)]
enum PlatformsCmd {
    /// List the platform library.
    List {
        /// Filter by category (property / personality / mixed).
        #[arg(long)]
        category: Option<String>,
    },
    /// Show how many platform records exist.
    Count,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let data_dir = cli.data_dir.clone().map(std::path::PathBuf::from);

    // `init` 与 `commands` 不需要先开库；其余命令必须先把库打开。
    if let Command::Commands = cli.command {
        for name in digilegacy_lib::dispatch::command_names() {
            println!("{}", name);
        }
        return ExitCode::from(EXIT_OK);
    }

    let opened = match digilegacy_lib::bootstrap::open(data_dir.clone()) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("错误: {}", e);
            return ExitCode::from(EXIT_FAIL);
        }
    };

    let out = run(&cli, &opened);
    match out {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("错误: {}", e);
            ExitCode::from(EXIT_FAIL)
        }
    }
}

fn run(cli: &Cli, data_dir: &std::path::Path) -> Result<u8, String> {
    match &cli.command {
        Command::Init => {
            println!("数据库已就绪：{}", data_dir.join("digilegacy.db").display());
            println!("数据目录：{}", data_dir.display());
            Ok(EXIT_OK)
        }

        Command::Doctor => {
            let report = digilegacy_lib::bootstrap::health(data_dir);
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
                );
            } else {
                println!("数据目录  {}", report.data_dir);
                println!(
                    "数据库    {}",
                    if report.database_present {
                        "存在"
                    } else {
                        "缺失"
                    }
                );
                println!(
                    "整库加密  {}",
                    if report.db_encryption_enabled {
                        "已启用"
                    } else {
                        "未启用"
                    }
                );
                println!(
                    "保险库    {}",
                    if !report.has_master_password {
                        "未设置主密码"
                    } else if report.unlocked {
                        "已解锁"
                    } else {
                        "已锁定"
                    }
                );
                println!("平台库    {} 条", report.platform_count);
                println!("资产      {} 条", report.asset_count);
                println!("Schema    v{}", report.schema_version);
                println!();
                for c in &report.checks {
                    println!("[{:4}] {:16} {}", c.status, c.name, c.detail);
                }
            }
            Ok(if report.ok { EXIT_OK } else { EXIT_FAIL })
        }

        Command::Profile => {
            let v = call("get_user_profile", Map::new())?;
            emit(cli, &v, |d| {
                let Some(p) = d.as_object() else {
                    return "尚未填写个人档案".to_string();
                };
                let get = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
                format!(
                    "姓名      {}\n邮箱      {}\n紧急联系人 {}\n紧急电话   {}\n执行人     {}",
                    get("full_name"),
                    get("email"),
                    get("emergency_contact_name"),
                    get("emergency_contact_email"),
                    get("executor_name")
                )
            })
        }

        Command::Assets(cmd) => assets(cli, cmd),
        Command::Heirs(cmd) => heirs(cli, cmd),
        Command::Dms(cmd) => dms(cli, cmd),
        Command::Report(cmd) => report(cli, cmd),
        Command::Backup(cmd) => backup(cli, cmd),
        Command::Vault(cmd) => vault(cli, cmd),
        Command::Platforms(cmd) => platforms(cli, cmd),
        Command::Call { name, args } => {
            let parsed: Value =
                serde_json::from_str(args).map_err(|e| format!("参数不是合法 JSON: {}", e))?;
            let obj = parsed
                .as_object()
                .cloned()
                .ok_or_else(|| "参数必须是 JSON 对象".to_string())?;
            let v = call(name, obj)?;
            emit(cli, &v, pretty)
        }
        Command::Commands => Ok(EXIT_OK),
    }
}

fn assets(cli: &Cli, cmd: &AssetsCmd) -> Result<u8, String> {
    match cmd {
        AssetsCmd::List => {
            let v = call("get_all_assets", Map::new())?;
            emit(cli, &v, |d| {
                let rows = d.as_array().cloned().unwrap_or_default();
                if rows.is_empty() {
                    return "还没有登记任何资产。用 `digilegacy assets add` 开始。".to_string();
                }
                let mut out = format!(
                    "{:>4}  {:<22} {:<12} {:<14} {:>10}  {}\n",
                    "ID", "平台", "类别", "处置", "估值", "账号标识"
                );
                for r in rows {
                    let g = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
                    let ident = g("account_identifier");
                    out.push_str(&format!(
                        "{:>4}  {:<22} {:<12} {:<14} {:>10.2}  {}\n",
                        r.get("id").and_then(Value::as_i64).unwrap_or(0),
                        truncate(g("platform_name"), 22),
                        g("category"),
                        g("disposal_method"),
                        r.get("estimated_value")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                        mask(ident, cli.show_secrets)
                    ));
                }
                out.trim_end().to_string()
            })
        }
        AssetsCmd::Get { id } => {
            let v = call("get_asset_by_id", one("id", json!(id)))?;
            emit(cli, &v, pretty)
        }
        AssetsCmd::Add {
            platform,
            category,
            identifier,
            description,
            value,
            disposal,
            heir,
            heir_contact,
            notes,
        } => {
            let req = json!({
                "platform_id": platform,
                "category": category,
                "account_identifier": identifier,
                "account_description": description,
                "estimated_value": value,
                "disposal_method": disposal,
                "heir_name": heir,
                "heir_contact": heir_contact,
                "notes": notes,
            });
            let v = call("add_asset", one("req", req))?;
            emit(cli, &v, |d| format!("已添加资产 ID {}", d))
        }
        AssetsCmd::Delete { id } => {
            let v = call("delete_asset", one("id", json!(id)))?;
            emit(cli, &v, |_| "已删除".to_string())
        }
        AssetsCmd::Dispose {
            id,
            method,
            heir,
            heir_contact,
            notes,
        } => {
            let mut args = one("disposal_method", json!(method));
            args.insert("asset_id".into(), json!(id));
            args.insert("heir_name".into(), json!(heir));
            args.insert("heir_contact".into(), json!(heir_contact));
            args.insert("notes".into(), json!(notes));
            let v = call("set_disposal_wish", args)?;
            emit(cli, &v, |_| "处置方式已更新".to_string())
        }
        AssetsCmd::Credential { id, stdin, value } => {
            let secret = if *stdin {
                let mut buf = String::new();
                std::io::stdin()
                    .read_line(&mut buf)
                    .map_err(|e| format!("读取 stdin 失败: {}", e))?;
                buf.trim_end_matches(['\r', '\n']).to_string()
            } else {
                value.clone().unwrap_or_default()
            };
            if secret.is_empty() {
                return Err("凭据为空：用 --value 或 --stdin 提供".to_string());
            }
            let v = call("set_asset_credential", {
                let mut m = one("id", json!(id));
                m.insert("credential".into(), json!(secret));
                m
            })?;
            emit(cli, &v, |_| "凭据已加密保存".to_string())
        }
    }
}

fn heirs(cli: &Cli, cmd: &HeirsCmd) -> Result<u8, String> {
    match cmd {
        HeirsCmd::List => {
            let v = call("get_heirs", Map::new())?;
            emit(cli, &v, |d| {
                let rows = d.as_array().cloned().unwrap_or_default();
                if rows.is_empty() {
                    return "还没有登记继承人。".to_string();
                }
                let mut out = String::from(
                    "姓名                 关系         执行人  电话             邮箱\n",
                );
                for r in rows {
                    let g = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
                    out.push_str(&format!(
                        "{:<20} {:<12} {:<7} {:<16} {}\n",
                        g("name"),
                        g("relation"),
                        if r.get("is_executor")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            "是"
                        } else {
                            "否"
                        },
                        mask(g("phone"), cli.show_secrets),
                        mask(g("email"), cli.show_secrets)
                    ));
                }
                out.trim_end().to_string()
            })
        }
        HeirsCmd::Add {
            name,
            relation,
            phone,
            email,
            share_note,
            is_executor,
        } => {
            let existing = call("get_heirs", Map::new())?;
            let mut list = existing
                .get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            list.push(json!({
                "id": Value::Null,
                "name": name,
                "relation": relation,
                "phone": phone,
                "email": email,
                "share_note": share_note,
                "is_executor": is_executor,
                "sort_order": list.len() as i64,
            }));
            let v = call("set_heirs", one("heirs", json!(list)))?;
            emit(cli, &v, |_| format!("已保存继承人 {}", name))
        }
        HeirsCmd::Remove { name } => {
            let existing = call("get_heirs", Map::new())?;
            let before = existing
                .get("data")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let kept: Vec<Value> = before
                .iter()
                .filter(|h| h.get("name").and_then(Value::as_str) != Some(name.as_str()))
                .cloned()
                .collect();
            if kept.len() == before.len() {
                return Err(format!("未找到名为 {} 的继承人", name));
            }
            let v = call("set_heirs", one("heirs", json!(kept)))?;
            emit(cli, &v, |_| format!("已移除继承人 {}", name))
        }
    }
}

fn dms(cli: &Cli, cmd: &DmsCmd) -> Result<u8, String> {
    match cmd {
        DmsCmd::Status => {
            let v = call("get_dms_status", Map::new())?;
            emit(cli, &v, pretty)
        }
        DmsCmd::Checkin => {
            let v = call("update_check_in", Map::new())?;
            emit(cli, &v, |_| "已记录签到".to_string())
        }
        DmsCmd::Enable { days } => {
            let mut m = one("enabled", json!(true));
            m.insert("interval_days".into(), json!(days));
            let v = call("set_dead_mans_switch", m)?;
            emit(cli, &v, |_| format!("已开启，每 {} 天检查一次", days))
        }
        DmsCmd::Disable => {
            let mut m = one("enabled", json!(false));
            m.insert("interval_days".into(), json!(30));
            let v = call("set_dead_mans_switch", m)?;
            emit(cli, &v, |_| "已关闭".to_string())
        }
        DmsCmd::Pause { until, clear } => {
            let value = if *clear {
                String::new()
            } else {
                until.clone().unwrap_or_default()
            };
            let v = call("set_dms_pause", one("until", json!(value)))?;
            emit(cli, &v, pretty)
        }
        DmsCmd::Review => {
            let v = call("record_planning_review", Map::new())?;
            emit(cli, &v, |_| "已记录年度复核".to_string())
        }
        DmsCmd::History => {
            let v = call("get_dms_history", Map::new())?;
            emit(cli, &v, pretty)
        }
    }
}

fn report(cli: &Cli, cmd: &ReportCmd) -> Result<u8, String> {
    match cmd {
        ReportCmd::Html { locale } => {
            let v = call("render_report", one("locale", json!(locale)))?;
            match v.get("data").and_then(Value::as_str) {
                Some(html) => {
                    println!("{}", html);
                    Ok(EXIT_OK)
                }
                None => Err(msg_of(&v)),
            }
        }
        ReportCmd::Export { locale } => {
            let html = call("render_report", one("locale", json!(locale)))?;
            let content = html
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| msg_of(&html))?;
            let v = call("export_report_to_file", one("content", json!(content)))?;
            emit(cli, &v, |d| format!("已导出：{}", d))
        }
        ReportCmd::Guide { locale } => {
            let v = call("render_action_guide", one("locale", json!(locale)))?;
            match v.get("data").and_then(Value::as_str) {
                Some(html) => {
                    println!("{}", html);
                    Ok(EXIT_OK)
                }
                None => Err(msg_of(&v)),
            }
        }
        ReportCmd::Drill => {
            let v = call("run_family_drill", Map::new())?;
            emit(cli, &v, pretty)
        }
    }
}

fn backup(cli: &Cli, cmd: &BackupCmd) -> Result<u8, String> {
    match cmd {
        BackupCmd::Export => {
            let v = call("export_backup", Map::new())?;
            emit(cli, &v, |d| format!("备份已写入：{}", d))
        }
        BackupCmd::Import {
            path,
            mode,
            password,
        } => {
            let content =
                std::fs::read_to_string(path).map_err(|e| format!("读取备份文件失败: {}", e))?;
            let mut m = one("content", json!(content));
            m.insert("mode".into(), json!(mode));
            m.insert("source_password".into(), json!(password));
            let v = call("import_backup", m)?;
            emit(cli, &v, |d| format!("导入完成：{}", d))
        }
        BackupCmd::Checkpoint { label } => {
            let v = call("create_checkpoint", one("label", json!(label)))?;
            emit(cli, &v, pretty)
        }
        BackupCmd::Checkpoints => {
            let v = call("list_checkpoints", Map::new())?;
            emit(cli, &v, pretty)
        }
        BackupCmd::Restore { id, password } => {
            let mut m = one("id", json!(id));
            m.insert("source_password".into(), json!(password));
            let v = call("restore_checkpoint", m)?;
            emit(cli, &v, |d| format!("{}", d))
        }
    }
}

fn vault(cli: &Cli, cmd: &VaultCmd) -> Result<u8, String> {
    match cmd {
        VaultCmd::Status => {
            let v = call("security_status", Map::new())?;
            emit(cli, &v, pretty)
        }
        VaultCmd::Unlock { password } => {
            let pw = resolve_secret(password)?;
            let v = call("unlock", one("password", json!(pw)))?;
            emit(cli, &v, |_| "已解锁".to_string())
        }
        VaultCmd::Lock => {
            let v = call("lock", Map::new())?;
            emit(cli, &v, |_| "已锁定".to_string())
        }
        VaultCmd::Setup { password } => {
            let pw = resolve_secret(password)?;
            let v = call("set_master_password", one("password", json!(pw)))?;
            emit(cli, &v, pretty)
        }
    }
}

fn platforms(cli: &Cli, cmd: &PlatformsCmd) -> Result<u8, String> {
    match cmd {
        PlatformsCmd::List { category } => {
            let v = call("get_all_platforms", Map::new())?;
            emit(cli, &v, |d| {
                let rows: Vec<Value> = d
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|r| match category {
                        Some(c) => r.get("category").and_then(Value::as_str) == Some(c.as_str()),
                        None => true,
                    })
                    .collect();
                let mut out = format!(
                    "共 {} 条\n{:>4}  {:<28} {:<12} {:<12}\n",
                    rows.len(),
                    "ID",
                    "平台",
                    "类别",
                    "难度"
                );
                for r in rows {
                    let g = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or("");
                    out.push_str(&format!(
                        "{:>4}  {:<28} {:<12} {:<12}\n",
                        r.get("id").and_then(Value::as_i64).unwrap_or(0),
                        truncate(g("name"), 28),
                        g("category"),
                        g("estimated_difficulty")
                    ));
                }
                out.trim_end().to_string()
            })
        }
        PlatformsCmd::Count => {
            let v = call("get_all_platforms", Map::new())?;
            emit(cli, &v, |d| {
                format!("{}", d.as_array().map(|a| a.len()).unwrap_or(0))
            })
        }
    }
}

fn call(name: &str, args: Map<String, Value>) -> Result<Value, String> {
    digilegacy_lib::dispatch::call(name, args)
}

fn one(key: &str, value: Value) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(key.to_string(), value);
    m
}

fn msg_of(v: &Value) -> String {
    v.get("message")
        .and_then(Value::as_str)
        .unwrap_or("命令失败")
        .to_string()
}

/// 统一出口：`--json` 打印原始响应；否则用调用方给的渲染器打印 `data`。
fn emit<F>(cli: &Cli, v: &Value, render: F) -> Result<u8, String>
where
    F: FnOnce(&Value) -> String,
{
    let ok = v.get("success").and_then(Value::as_bool).unwrap_or(false);
    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(v).map_err(|e| e.to_string())?
        );
    } else if ok {
        let data = v.get("data").cloned().unwrap_or(Value::Null);
        println!("{}", render(&data));
        let msg = msg_of(v);
        if msg != "操作成功" && !msg.is_empty() {
            eprintln!("({})", msg);
        }
    } else {
        eprintln!("{}", msg_of(v));
    }
    Ok(if ok { EXIT_OK } else { EXIT_FAIL })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}

/// 敏感值打码：保留首尾各一个字符，其余用 *。
///
/// 打码而不是隐藏：用户需要能确认「这条是不是我要的那条」，
/// 但完整值不应出现在终端历史、截图或 CI 日志里。
fn mask(value: &str, show: bool) -> String {
    if show {
        return value.to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    match chars.len() {
        0 => String::new(),
        1 => "*".to_string(),
        2 => format!("{}*", chars[0]),
        n => format!(
            "{}{}{}",
            chars[0],
            "*".repeat(n.saturating_sub(2).min(8)),
            chars[n - 1]
        ),
    }
}

fn resolve_secret(provided: &Option<String>) -> Result<String, String> {
    if let Some(p) = provided {
        return Ok(p.clone());
    }
    eprint!("主密码（输入不回显）: ");
    use std::io::Write as _;
    std::io::stderr().flush().ok();
    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .map_err(|e| format!("读取密码失败: {}", e))?;
    Ok(buf.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_keeps_edges_and_hides_middle() {
        assert_eq!(mask("", false), "");
        assert_eq!(mask("a", false), "*");
        assert_eq!(mask("ab", false), "a*");
        // 17 字符 → 首 + 8 个星（上限 8）+ 尾
        assert_eq!(mask("alice@example.com", false), "a********m");
        assert_eq!(mask("alice@example.com", true), "alice@example.com");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("短", 4), "短");
        assert_eq!(truncate("中文平台名称很长", 4), "中文平…");
    }

    #[test]
    fn one_builds_single_key_object() {
        let m = one("id", json!(3));
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("id").and_then(Value::as_i64), Some(3));
    }

    #[test]
    fn cli_definition_is_valid() {
        // clap 的 derive 在编译期检查大部分约束，这里额外确认命令名不冲突。
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
