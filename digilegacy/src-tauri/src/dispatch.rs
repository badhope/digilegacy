//! 无 Tauri 依赖的命令分发层：CLI 与 HTTP 服务共用的唯一入口。
//!
//! 设计动机：桌面端通过 Tauri IPC 直接调用 `#[command]` 函数，而 CLI 与
//! Web 后端拿不到 `AppHandle`。若为它们各写一套，同一条业务规则会在三个
//! 地方各有一份实现，迟早漂移（本项目历史上就出现过「导出比预览少字段」）。
//!
//! 这里用 `serde_json::Value` 做参数与返回值的中转：命令名 + 参数对象 →
//! 具体命令函数 → 统一序列化。分发只覆盖**不依赖 Tauri 运行时**的命令；
//! 需要托盘、窗口、自启的命令（`set_autostart`/`hide_main_window`）明确
//! 返回「该入口不可用」，而不是假装成功。
//!
//! 参数校验与业务规则全部复用命令层，`dispatch` 自身不含业务逻辑。

use serde_json::{Map, Value};

use crate::models::*;

/// 某一入口不支持该命令时的统一错误前缀，便于 CLI/HTTP 各自映射状态码。
pub const ERR_UNSUPPORTED: &str = "UNSUPPORTED";

fn arg_str(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("缺少参数 `{}`（应为字符串）", key))
}

fn arg_opt_str(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn arg_i64(args: &Map<String, Value>, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("缺少参数 `{}`（应为整数）", key))
}

fn arg_u8(args: &Map<String, Value>, key: &str) -> Result<u8, String> {
    let v = arg_i64(args, key)?;
    u8::try_from(v).map_err(|_| format!("参数 `{}` 超出 0-255 范围", key))
}

fn arg_bool(args: &Map<String, Value>, key: &str) -> Result<bool, String> {
    args.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("缺少参数 `{}`（应为布尔）", key))
}

fn to_value<T: serde::Serialize>(resp: ApiResponse<T>) -> Result<Value, String> {
    serde_json::to_value(resp).map_err(|e| format!("序列化响应失败: {}", e))
}

/// 把 `ApiResponse<T>` 转成裸 JSON；CLI 打印 `data`，HTTP 原样返回。
pub fn call(cmd: &str, args: Map<String, Value>) -> Result<Value, String> {
    match cmd {
        // ---------- 平台与政策库 ----------
        "get_all_platforms" => to_value(crate::commands::get_all_platforms()?),
        "get_all_platforms_extended" => to_value(crate::commands::get_all_platforms_extended()?),
        "import_platform_patch" => {
            let content = arg_str(&args, "content")?;
            to_value(crate::commands::import_platform_patch(content)?)
        }
        "export_platform_patch" => to_value(crate::commands::export_platform_patch()?),
        "platform_patch_status" => to_value(crate::commands::platform_patch_status()?),
        "clear_platform_patch" => to_value(crate::commands::clear_platform_patch()?),
        "list_authorities" => to_value(crate::commands::list_authorities()?),
        "authorities_for" => {
            let category = arg_str(&args, "category")?;
            to_value(crate::commands::authorities_for(category)?)
        }

        // ---------- 资产 ----------
        "add_asset" => {
            let req: AssetRequest =
                serde_json::from_value(args.get("req").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("资产参数不合法: {}", e))?;
            to_value(crate::commands::add_asset(req)?)
        }
        "update_asset" => {
            let id = arg_i64(&args, "id")?;
            let req: AssetRequest =
                serde_json::from_value(args.get("req").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("资产参数不合法: {}", e))?;
            to_value(crate::commands::update_asset(id, req)?)
        }
        "delete_asset" => {
            let id = arg_i64(&args, "id")?;
            to_value(crate::commands::delete_asset(id)?)
        }
        "get_all_assets" => to_value(crate::commands::get_all_assets()?),
        "get_asset_by_id" => {
            let id = arg_i64(&args, "id")?;
            to_value(crate::commands::get_asset_by_id(id)?)
        }
        "get_assets_by_category" => {
            let category = arg_str(&args, "category")?;
            to_value(crate::commands::get_assets_by_category(category)?)
        }
        "set_disposal_wish" => {
            let asset_id = arg_i64(&args, "asset_id")?;
            let disposal_method = arg_str(&args, "disposal_method")?;
            let heir_name = arg_opt_str(&args, "heir_name");
            let heir_contact = arg_opt_str(&args, "heir_contact");
            let notes = arg_opt_str(&args, "notes").unwrap_or_default();
            to_value(crate::commands::set_disposal_wish(
                disposal_method,
                asset_id,
                heir_name,
                heir_contact,
                notes,
            )?)
        }
        "get_heirs" => to_value(crate::commands::get_heirs()?),
        "set_heirs" => {
            let heirs: Vec<Heir> =
                serde_json::from_value(args.get("heirs").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("继承人参数不合法: {}", e))?;
            to_value(crate::commands::set_heirs(heirs)?)
        }

        // ---------- 档案、DMS、仪表板 ----------
        "get_user_profile" => to_value(crate::commands::get_user_profile()?),
        "set_user_profile" => {
            let profile: UserProfile =
                serde_json::from_value(args.get("profile").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("档案参数不合法: {}", e))?;
            to_value(crate::commands::set_user_profile(profile)?)
        }
        "get_inheritance_report" => to_value(crate::commands::get_inheritance_report()?),
        "get_dashboard_stats" => to_value(crate::commands::get_dashboard_stats()?),
        "update_check_in" => to_value(crate::commands::update_check_in()?),
        "get_check_in_status" => to_value(crate::commands::get_check_in_status()?),
        "get_dms_status" => to_value(crate::commands::get_dms_status()?),
        "get_dms_history" => to_value(crate::commands::get_dms_history()?),
        "set_dead_mans_switch" => {
            let enabled = arg_bool(&args, "enabled")?;
            let interval_days = arg_i64(&args, "interval_days")?;
            to_value(crate::commands::set_dead_mans_switch(
                enabled,
                interval_days,
            )?)
        }
        "set_dms_pause" => {
            let until = arg_str(&args, "until")?;
            to_value(crate::commands::set_dms_pause(until)?)
        }
        "record_planning_review" => to_value(crate::commands::record_planning_review()?),
        "get_custody_hints" => to_value(crate::commands::get_custody_hints()?),
        "set_custody_hints" => {
            let hints: crate::db::CustodyHints =
                serde_json::from_value(args.get("hints").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("保管位置参数不合法: {}", e))?;
            to_value(crate::commands::set_custody_hints(hints)?)
        }

        // ---------- 报告与工具包 ----------
        "render_report" => {
            let locale = arg_opt_str(&args, "locale");
            to_value(crate::commands::render_report(locale)?)
        }
        "export_report_to_file" => {
            let content = arg_str(&args, "content")?;
            to_value(crate::commands::export_report_to_file(content)?)
        }
        "export_zip_to_file" => {
            let content = arg_str(&args, "content")?;
            to_value(crate::commands::export_zip_to_file(content)?)
        }
        "render_action_guide" => {
            let locale = arg_opt_str(&args, "locale");
            to_value(crate::commands::render_action_guide(locale)?)
        }
        "run_family_drill" => to_value(crate::commands::run_family_drill()?),
        "export_kdbx_archive" => {
            let archive_password = arg_str(&args, "archive_password")?;
            to_value(crate::commands::export_kdbx_archive(archive_password)?)
        }

        // ---------- 安全与备份 ----------
        "security_status" => to_value(crate::commands::security_status()?),
        "set_master_password" => {
            let password = arg_str(&args, "password")?;
            to_value(crate::commands::set_master_password(password)?)
        }
        "unlock" => {
            let password = arg_str(&args, "password")?;
            to_value(crate::commands::unlock(password)?)
        }
        "lock" => to_value(crate::commands::lock()?),
        "set_asset_credential" => {
            let id = arg_i64(&args, "id")?;
            let credential = arg_str(&args, "credential")?;
            to_value(crate::commands::set_asset_credential(id, credential)?)
        }
        "get_asset_credential" => {
            let id = arg_i64(&args, "id")?;
            to_value(crate::commands::get_asset_credential(id)?)
        }
        "change_master_password" => {
            let old_password = arg_str(&args, "old_password")?;
            let new_password = arg_str(&args, "new_password")?;
            let confirm_password = arg_str(&args, "confirm_password")?;
            to_value(crate::commands::change_master_password(
                old_password,
                new_password,
                confirm_password,
            )?)
        }
        "export_backup" => to_value(crate::commands::export_backup()?),
        "import_backup" => {
            let content = arg_str(&args, "content")?;
            let mode = arg_opt_str(&args, "mode");
            let source_password = arg_opt_str(&args, "source_password");
            to_value(crate::commands::import_backup(
                content,
                mode,
                source_password,
            )?)
        }
        "get_backup_status" => to_value(crate::commands::get_backup_status()?),
        "get_db_encryption_status" => to_value(crate::commands::get_db_encryption_status()?),
        "enable_db_encryption" => to_value(crate::commands::enable_db_encryption()?),
        "disable_db_encryption" => to_value(crate::commands::disable_db_encryption()?),
        "get_recovery_seal_exists" => to_value(crate::commands::get_recovery_seal_exists()?),
        "start_recovery" => {
            let recovery_code = arg_str(&args, "recovery_code")?;
            to_value(crate::commands::start_recovery(recovery_code)?)
        }
        "complete_recovery" => {
            let new_password = arg_str(&args, "new_password")?;
            let confirm_password = arg_str(&args, "confirm_password")?;
            to_value(crate::commands::complete_recovery(
                new_password,
                confirm_password,
            )?)
        }

        // ---------- 检查点 ----------
        "create_checkpoint" => {
            let label = arg_str(&args, "label")?;
            to_value(crate::commands::create_checkpoint(label)?)
        }
        "list_checkpoints" => to_value(crate::commands::list_checkpoints()?),
        "delete_checkpoint" => {
            let id = arg_str(&args, "id")?;
            to_value(crate::commands::delete_checkpoint(id)?)
        }
        "restore_checkpoint" => {
            let id = arg_str(&args, "id")?;
            let source_password = arg_opt_str(&args, "source_password");
            to_value(crate::commands::restore_checkpoint(id, source_password)?)
        }

        // ---------- 继承人恢复包 ----------
        "generate_heir_recovery_kit" => {
            let threshold = arg_u8(&args, "threshold")?;
            let total = arg_u8(&args, "total")?;
            let passphrase = arg_opt_str(&args, "passphrase");
            to_value(crate::commands::generate_heir_recovery_kit(
                threshold, total, passphrase,
            )?)
        }
        "heir_recovery_status" => to_value(crate::commands::heir_recovery_status()?),
        "clear_heir_recovery" => to_value(crate::commands::clear_heir_recovery()?),
        "run_recovery_drill" => {
            let shares: Vec<String> =
                serde_json::from_value(args.get("shares").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("份额参数不合法: {}", e))?;
            let passphrase = arg_opt_str(&args, "passphrase");
            to_value(crate::commands::run_recovery_drill(shares, passphrase)?)
        }
        "unlock_with_recovery_shares" => {
            let shares: Vec<String> =
                serde_json::from_value(args.get("shares").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("份额参数不合法: {}", e))?;
            let passphrase = arg_opt_str(&args, "passphrase");
            to_value(crate::commands::unlock_with_recovery_shares(
                shares, passphrase,
            )?)
        }

        // ---------- 通知通道 ----------
        "get_smtp_config" => to_value(crate::commands::get_smtp_config()?),
        "set_smtp_config" => {
            let host = arg_str(&args, "host")?;
            let port = arg_i64(&args, "port")?;
            let from_addr = arg_str(&args, "from_addr")?;
            let username = arg_str(&args, "username")?;
            let password = arg_opt_str(&args, "password");
            let allow_insecure = args
                .get("allow_insecure")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            to_value(crate::commands::set_smtp_config(
                host,
                port,
                from_addr,
                username,
                password,
                allow_insecure,
            )?)
        }
        "send_test_email" => {
            let recipient = arg_str(&args, "recipient")?;
            to_value(crate::commands::send_test_email(recipient)?)
        }
        "get_heartbeat_config" => to_value(crate::commands::get_heartbeat_config()?),
        "set_heartbeat_config" => {
            let enabled = arg_bool(&args, "enabled")?;
            let token = arg_str(&args, "token")?;
            let endpoint = arg_str(&args, "endpoint")?;
            to_value(crate::commands::set_heartbeat_config(
                enabled, token, endpoint,
            )?)
        }
        "get_heartbeat_status" => to_value(crate::commands::get_heartbeat_status()?),
        "send_heartbeat_now" => to_value(crate::commands::send_heartbeat_now()?),
        "get_imap_config" => to_value(crate::commands::get_imap_config()?),
        "set_imap_config" => {
            let config: crate::imap_checkin::ImapConfig =
                serde_json::from_value(args.get("config").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("IMAP 参数不合法: {}", e))?;
            let password = arg_opt_str(&args, "password");
            to_value(crate::commands::set_imap_config(config, password)?)
        }
        "poll_imap_checkin" => to_value(crate::commands::poll_imap_checkin()?),

        // ---------- AI 助手与 Agent ----------
        "get_assistant_config" => to_value(crate::commands::get_assistant_config()?),
        "set_assistant_config" => {
            let enabled = arg_bool(&args, "enabled")?;
            let endpoint = arg_str(&args, "endpoint")?;
            let model = arg_str(&args, "model")?;
            let api_key = arg_opt_str(&args, "api_key");
            to_value(crate::commands::set_assistant_config(
                enabled, endpoint, model, api_key,
            )?)
        }
        "preview_assistant_payload" => {
            let question = arg_str(&args, "question")?;
            to_value(crate::commands::preview_assistant_payload(question)?)
        }
        "ask_assistant" => {
            let question = arg_str(&args, "question")?;
            let locale = arg_opt_str(&args, "locale");
            to_value(crate::commands::ask_assistant(question, locale)?)
        }
        "list_ai_chat" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            to_value(crate::commands::list_ai_chat(limit)?)
        }
        "clear_ai_chat" => to_value(crate::commands::clear_ai_chat()?),
        "list_agent_run_log" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            to_value(crate::commands::list_agent_run_log(limit)?)
        }
        "clear_agent_run_log" => to_value(crate::commands::clear_agent_run_log()?),
        "cancel_agent_task" => to_value(crate::commands::cancel_agent_task()?),
        "run_agent_task" => {
            // Agent 任务在 Tauri 侧是 async（内部走 spawn_blocking）。这里的
            // 分发器刻意保持同步：CLI/HTTP 调用方各自持有运行时，由它们
            // 决定阻塞策略，分发器只负责路由。
            let task = arg_str(&args, "task")?;
            let outcome = tauri::async_runtime::block_on(crate::commands::run_agent_task(task))?;
            to_value(outcome)
        }

        // ---------- 审核管线 ----------
        "list_review_profiles" => to_value(crate::commands::list_review_profiles()?),
        "list_review_items" => {
            let profile_id = arg_str(&args, "profile_id")?;
            to_value(crate::commands::list_review_items(profile_id)?)
        }
        "review_progress" => {
            let profile_id = arg_str(&args, "profile_id")?;
            to_value(crate::commands::review_progress(profile_id)?)
        }
        "review_payload_fields" => {
            let profile_id = arg_str(&args, "profile_id")?;
            to_value(crate::commands::review_payload_fields(profile_id)?)
        }
        "run_ai_review" => {
            let profile_id = arg_str(&args, "profile_id")?;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            to_value(crate::commands::run_ai_review(profile_id, limit)?)
        }
        "submit_human_review" => {
            let profile_id = arg_str(&args, "profile_id")?;
            let item_key = arg_str(&args, "item_key")?;
            let verdict = arg_str(&args, "verdict")?;
            let note = arg_opt_str(&args, "note");
            to_value(crate::commands::submit_human_review(
                profile_id, item_key, verdict, note,
            )?)
        }
        "review_history" => {
            let profile_id = arg_str(&args, "profile_id")?;
            let item_key = arg_str(&args, "item_key")?;
            to_value(crate::commands::review_history(profile_id, item_key)?)
        }

        // ---------- 桌面专属：明确不支持而不是假装成功 ----------
        "set_autostart" | "hide_main_window" => Err(format!(
            "{}: `{}` 只在桌面应用内可用（依赖系统托盘与窗口管理）",
            ERR_UNSUPPORTED, cmd
        )),

        other => Err(format!("未知命令: {}", other)),
    }
}

/// 供 CLI `--help` 与 HTTP `GET /api/commands` 共用：可在此入口执行的命令清单。
///
/// 手写而不是反射：Rust 没有运行时反射，硬编码的清单若与 `call` 漂移，
/// 下面的测试会用「清单里每条命令都能被路由（不会命中 unknown）」兜住。
pub fn command_names() -> Vec<&'static str> {
    vec![
        "get_all_platforms",
        "get_all_platforms_extended",
        "import_platform_patch",
        "export_platform_patch",
        "platform_patch_status",
        "clear_platform_patch",
        "list_authorities",
        "authorities_for",
        "add_asset",
        "update_asset",
        "delete_asset",
        "get_all_assets",
        "get_asset_by_id",
        "get_assets_by_category",
        "set_disposal_wish",
        "get_heirs",
        "set_heirs",
        "get_user_profile",
        "set_user_profile",
        "get_inheritance_report",
        "get_dashboard_stats",
        "update_check_in",
        "get_check_in_status",
        "get_dms_status",
        "get_dms_history",
        "set_dead_mans_switch",
        "set_dms_pause",
        "record_planning_review",
        "get_custody_hints",
        "set_custody_hints",
        "render_report",
        "export_report_to_file",
        "export_zip_to_file",
        "render_action_guide",
        "run_family_drill",
        "export_kdbx_archive",
        "security_status",
        "set_master_password",
        "unlock",
        "lock",
        "set_asset_credential",
        "get_asset_credential",
        "change_master_password",
        "export_backup",
        "import_backup",
        "get_backup_status",
        "get_db_encryption_status",
        "enable_db_encryption",
        "disable_db_encryption",
        "get_recovery_seal_exists",
        "start_recovery",
        "complete_recovery",
        "create_checkpoint",
        "list_checkpoints",
        "delete_checkpoint",
        "restore_checkpoint",
        "generate_heir_recovery_kit",
        "heir_recovery_status",
        "clear_heir_recovery",
        "run_recovery_drill",
        "unlock_with_recovery_shares",
        "get_smtp_config",
        "set_smtp_config",
        "send_test_email",
        "get_heartbeat_config",
        "set_heartbeat_config",
        "get_heartbeat_status",
        "send_heartbeat_now",
        "get_imap_config",
        "set_imap_config",
        "poll_imap_checkin",
        "get_assistant_config",
        "set_assistant_config",
        "preview_assistant_payload",
        "ask_assistant",
        "list_ai_chat",
        "clear_ai_chat",
        "list_agent_run_log",
        "clear_agent_run_log",
        "cancel_agent_task",
        "run_agent_task",
        "list_review_profiles",
        "list_review_items",
        "review_progress",
        "review_payload_fields",
        "run_ai_review",
        "submit_human_review",
        "review_history",
    ]
}

/// 构建一个参数对象的小工具（CLI 侧用，避免到处手写 Map 样板代码）。
pub fn args_from(pairs: &[(&str, Value)]) -> Map<String, Value> {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_command_reports_clearly() {
        let err = call("no_such_command", Map::new()).unwrap_err();
        assert!(err.contains("未知命令"), "实际错误: {}", err);
    }

    #[test]
    fn desktop_only_commands_are_rejected_not_faked() {
        for cmd in ["set_autostart", "hide_main_window"] {
            let err = call(cmd, Map::new()).unwrap_err();
            assert!(
                err.starts_with(ERR_UNSUPPORTED),
                "{} 应明确标记为不支持，实际: {}",
                cmd,
                err
            );
        }
    }

    #[test]
    fn missing_argument_names_the_parameter() {
        let err = call("delete_asset", Map::new()).unwrap_err();
        assert!(err.contains("id"), "错误信息应指出缺哪个参数: {}", err);
    }

    /// 清单与分发器的漂移是本文件最容易犯的错：漏一条命令，用户会看到
    /// 「未知命令」而不是该命令的真实错误。
    ///
    /// 这里**静态**核对而不是逐条调用：真实调用会执行命令（`lock` 会清空
    /// 全局保险库 key、`export_backup` 会写盘），在并行测试进程里会污染
    /// 其他用例——本项目已经因此踩过一次「外键莫名失败」的假阴性。
    #[test]
    fn every_advertised_command_has_a_match_arm() {
        let source = include_str!("dispatch.rs");
        // 只看 match 区域，避免命令名出现在测试或注释里也算命中
        let start = source.find("pub fn call(").expect("call 函数必须存在");
        let end = source[start..]
            .find("pub fn command_names")
            .map(|i| start + i)
            .expect("command_names 必须紧随其后");
        let body = &source[start..end];

        for name in command_names() {
            let arm = format!("\"{}\" =>", name);
            assert!(
                body.contains(&arm),
                "命令清单包含 `{}`，但 call() 里没有对应的 match 分支",
                name
            );
        }
    }

    #[test]
    fn no_duplicate_entries_in_command_list() {
        let names = command_names();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "命令清单存在重复项");
    }

    #[test]
    fn args_from_builds_object() {
        let m = args_from(&[("id", serde_json::json!(7))]);
        assert_eq!(m.get("id").and_then(Value::as_i64), Some(7));
    }
}
