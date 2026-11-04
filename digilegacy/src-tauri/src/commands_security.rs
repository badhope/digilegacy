//! 安全与凭据保管、恢复包（Shamir）、KDBX 归档 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。
//!
//! 本模块属安全边界：改动需 CODEOWNERS 审阅（见 .github/CODEOWNERS）。

use crate::db;
use crate::models::*;
use tauri::{command, AppHandle, Manager};
use zeroize::Zeroize;

use super::report::validate_backup_content;
#[cfg(target_os = "windows")]
use super::win_shell;

// ========== 安全与凭据保管 ==========

#[command]
pub fn security_status() -> Result<ApiResponse<SecurityStatus>, String> {
    let has = db::get_security_config_db()?.is_some();
    // 「解锁」必须是可验证的状态，而不是「内存里有个 key」。
    // 密钥与验证器不匹配时（例如恢复流程中途退出）继续宣称已解锁，
    // 界面就会把占位符当数据展示、并把占位符写回（历史 P0）。
    // 无主密码时如实返回「没有密钥」，是否可用由前端按 has_master_password 决定
    // （security.ts: `unlocked = has ? data.unlocked : true`）。
    let unlocked = if has {
        let guard = db::get_conn()?;
        db::vault_key_matches_config(guard.as_ref().unwrap())
    } else {
        false
    };
    Ok(ApiResponse::ok(SecurityStatus {
        has_master_password: has,
        unlocked,
    }))
}

#[command]
pub fn set_master_password(
    mut password: String,
) -> Result<ApiResponse<MasterPasswordSetup>, String> {
    if password.chars().count() < 8 {
        password.zeroize();
        return Ok(ApiResponse::fail("主密码至少需要 8 个字符"));
    }
    if db::get_security_config_db()?.is_some() {
        password.zeroize();
        return Ok(ApiResponse::fail("已设置主密码，暂不支持修改"));
    }

    // v3 盐（Argon2id，参数随盐串自描述）：新装一律 Argon2id。
    // 保留 PBKDF2 v1/v2 读取路径，旧库升级后仍能解锁。
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt(&password, &salt)?;
    password.zeroize(); // 派生完成后立即擦除明文密码

    // 两层密钥：随机 DEK 才是数据密钥，KEK 只负责包裹它。
    // 改密码从此只需重包裹 DEK（O(1)），且已发出的继承人份额不会失效。
    let dek = crate::vault::generate_data_key();
    let wrapped = crate::vault::wrap_data_key(&dek, &kek)?;
    let verifier = crate::vault::make_verifier(&dek)?;
    let (recovery_code, recovery_seal) = crate::recovery::create_recovery_seal(&dek)?;
    db::set_security_config_with_dek_db(&salt, &verifier, &wrapped)?;
    db::set_recovery_seal_db(&recovery_seal)?;
    crate::vault::set_key(dek);

    let encrypted_rows = db::encrypt_existing_data_db()?;

    Ok(ApiResponse {
        success: true,
        message: format!(
            "加密已启用，存量数据已加密 {} 条。请立即抄写恢复密钥。",
            encrypted_rows
        ),
        data: Some(MasterPasswordSetup {
            status: SecurityStatus {
                has_master_password: true,
                unlocked: true,
            },
            recovery_code,
        }),
    })
}

#[command]
pub fn unlock(mut password: String) -> Result<ApiResponse<SecurityStatus>, String> {
    let Some((salt_b64, verifier)) = db::get_security_config_db()? else {
        password.zeroize();
        return Ok(ApiResponse::fail("尚未设置主密码"));
    };
    let kek = crate::crypto::derive_from_salt(&password, &salt_b64)?;
    password.zeroize(); // 派生完成后立即擦除明文密码

    // 数据密钥的来源：新库从 wrapped_dek 解出；legacy 库密码派生密钥即数据密钥。
    let wrapped = db::get_wrapped_dek_db()?;
    let key = if wrapped.is_empty() {
        kek.clone()
    } else {
        match crate::vault::unwrap_data_key(&wrapped, &kek) {
            Ok(k) => k,
            Err(_) => return Ok(ApiResponse::fail("主密码错误")),
        }
    };

    if crate::vault::verify_key(&verifier, &key) {
        crate::vault::set_key(key);
        Ok(ApiResponse::ok(SecurityStatus {
            has_master_password: true,
            unlocked: true,
        }))
    } else {
        Ok(ApiResponse::fail("主密码错误"))
    }
}

#[command]
pub fn lock() -> Result<ApiResponse<SecurityStatus>, String> {
    crate::vault::clear_key();
    Ok(ApiResponse::ok_with_msg(
        SecurityStatus {
            has_master_password: true,
            unlocked: false,
        },
        "已锁定",
    ))
}

#[command]
pub fn set_asset_credential(id: i64, credential: String) -> Result<ApiResponse<()>, String> {
    // IPC 入口参数上限：单条凭据 64KB，正常凭据远小于此
    if credential.len() > 64 * 1024 {
        return Ok(ApiResponse::fail("凭据内容过长（上限 64KB）"));
    }
    if db::get_asset_by_id_db(id)?.is_none() {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    db::set_asset_credential_db(id, &credential)?;
    let msg = if credential.is_empty() {
        "凭据已清除"
    } else {
        "凭据已加密保存"
    };
    Ok(ApiResponse::ok_with_msg((), msg))
}

#[command]
pub fn get_asset_credential(id: i64) -> Result<ApiResponse<String>, String> {
    db::get_asset_credential_db(id).map(ApiResponse::ok)
}

#[command]
pub fn change_master_password(
    mut old_password: String,
    mut new_password: String,
    mut confirm_password: String,
) -> Result<ApiResponse<MasterPasswordSetup>, String> {
    match db::change_master_password_db(&old_password, &new_password, &confirm_password) {
        Ok(recovery_code) => {
            old_password.zeroize();
            new_password.zeroize();
            confirm_password.zeroize();
            Ok(ApiResponse {
                success: true,
                message: "主密码已更新，全部数据已用新密钥重新加密。请立即抄写新的恢复密钥。"
                    .to_string(),
                data: Some(MasterPasswordSetup {
                    status: SecurityStatus {
                        has_master_password: true,
                        unlocked: true,
                    },
                    recovery_code,
                }),
            })
        }
        Err(e) => {
            old_password.zeroize();
            new_password.zeroize();
            confirm_password.zeroize();
            Ok(ApiResponse::fail(&e))
        }
    }
}

#[command]
pub fn export_backup(app_handle: AppHandle) -> Result<ApiResponse<String>, String> {
    let json = db::export_backup_json_db()?;
    let encrypted = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|value| {
            value
                .get("tables")
                .and_then(|tables| tables.get("security_config"))
                .map(serde_json::Value::is_object)
        })
        .unwrap_or(false);
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let dir = app_dir.join("备份");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let name = format!(
        "归零{}备份_{}.json",
        if encrypted { "加密" } else { "明文" },
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(name);
    crate::fsutil::write_private(&path, json.as_bytes())
        .map_err(|e| format!("写入备份失败: {}", e))?;
    let message = if encrypted {
        "备份已导出（含密文与安全配置，可与其他设备互导）"
    } else {
        "明文备份已导出（尚未启用主密码，不提供加密保护）"
    };
    Ok(ApiResponse::ok_with_msg(
        path.to_string_lossy().to_string(),
        message,
    ))
}

// ========== 整库加密（SQLCipher）==========

/// 查询整库加密状态。
#[command]
pub fn get_db_encryption_status(
    app_handle: AppHandle,
) -> Result<ApiResponse<crate::db_crypto::DbEncryptionStatus>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;
    Ok(ApiResponse::ok(crate::db_crypto::DbEncryptionStatus {
        enabled: crate::db::is_db_encryption_enabled(&app_dir),
    }))
}

/// 启用整库加密：明文库 → SQLCipher（带快照，失败自动回滚）。
#[command]
pub fn enable_db_encryption(app_handle: AppHandle) -> Result<ApiResponse<()>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;
    crate::db::encrypt_database_at_runtime(&app_dir)?;
    Ok(ApiResponse::ok_with_msg(
        (),
        "整库加密已开启：数据库文件此后以 SQLCipher（AES-256）存储，明文快照保留在应用目录",
    ))
}

/// 停用整库加密：SQLCipher → 明文库（同样带快照）。
#[command]
pub fn disable_db_encryption(app_handle: AppHandle) -> Result<ApiResponse<()>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;
    crate::db::decrypt_database_at_runtime(&app_dir)?;
    Ok(ApiResponse::ok_with_msg(
        (),
        "整库加密已停用，数据库转回明文存储",
    ))
}

/// 导入备份的核心策略（纯函数，便于脱离 Tauri 运行时回归）。
///
/// `replace` 模式必须先写一份导入前快照；命令层传应用数据目录，测试可传入故障路径。
/// `merge` 模式只追加备份数据，不依赖快照，保留其非破坏语义。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn import_backup_response(
    content: &str,
    mode: &str,
    snapshot_dir: Option<&std::path::Path>,
) -> Result<ApiResponse<String>, String> {
    import_backup_response_with_password(content, mode, snapshot_dir, None)
}

pub(crate) fn import_backup_response_with_password(
    content: &str,
    mode: &str,
    snapshot_dir: Option<&std::path::Path>,
    source_password: Option<&str>,
) -> Result<ApiResponse<String>, String> {
    if let Err(e) = validate_backup_content(content) {
        return Ok(ApiResponse::fail(&e));
    }
    let import_mode = db::ImportMode::parse(mode);

    let snapshot = if import_mode == db::ImportMode::Replace {
        let Some(dir) = snapshot_dir else {
            return Ok(ApiResponse::fail(
                "覆盖导入前快照失败：无法获取应用数据目录",
            ));
        };
        let json = match db::export_backup_json_db() {
            Ok(json) => json,
            Err(e) => return Ok(ApiResponse::fail(&format!("覆盖导入前快照导出失败：{}", e))),
        };
        match crate::backup::write_pre_import_snapshot(dir, &json) {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(e) => return Ok(ApiResponse::fail(&format!("覆盖导入前快照写入失败：{}", e))),
        }
    } else {
        None
    };

    match db::import_backup_json_db_with_mode_and_password(content, import_mode, source_password) {
        Ok(n) => {
            let mode_label = if import_mode == db::ImportMode::Replace {
                "覆盖"
            } else {
                "合并"
            };
            let snapshot_note = snapshot
                .map(|path| format!("；导入前快照：{}", path))
                .unwrap_or_default();
            Ok(ApiResponse::ok_with_msg(
                format!(
                    "已按{}模式导入 {} 条资产，请用该备份对应的主密码重新解锁{}",
                    mode_label, n, snapshot_note
                ),
                "备份导入完成",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 导入备份。
///
/// `mode`：`merge`（默认，保留现有资产，只追加）或 `replace`（清空后写入）。
/// 两种模式都会在写入前自动落一份「导入前快照」——覆盖导入是不可逆操作，
/// 误选一次就永久丢数据，快照是唯一能回滚的路径。
#[command]
pub fn import_backup(
    content: String,
    mode: Option<String>,
    source_password: Option<String>,
    app_handle: AppHandle,
) -> Result<ApiResponse<String>, String> {
    let dir = app_handle.path().app_data_dir().ok();
    let mut password = source_password;
    let result = import_backup_response_with_password(
        &content,
        mode.as_deref().unwrap_or("merge"),
        dir.as_deref(),
        password.as_deref(),
    );
    if let Some(password) = password.as_mut() {
        password.zeroize();
    }
    result
}

#[command]
pub fn get_recovery_seal_exists() -> Result<ApiResponse<bool>, String> {
    Ok(ApiResponse::ok(!db::get_recovery_seal_db()?.is_empty()))
}

/// 自动备份状态：让用户知道「每日快照」是否真的在跑。
/// 旧界面只有手动导出按钮，自动备份存在但完全不可见。
#[command]
pub fn get_backup_status(app_handle: AppHandle) -> Result<ApiResponse<serde_json::Value>, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let summary = crate::backup::latest_backup_summary(&dir);
    let count = crate::backup::list_backups(&dir).len();
    Ok(ApiResponse::ok(serde_json::json!({
        "count": count,
        "latest_name": summary.as_ref().map(|(n, _)| n.clone()),
        "latest_size": summary.map(|(_, s)| s),
    })))
}

/// 恢复流程第一步：用恢复码解出主密钥（不设新密码），返回临时解锁态
#[command]
pub fn start_recovery(recovery_code: String) -> Result<ApiResponse<SecurityStatus>, String> {
    let seal = db::get_recovery_seal_db()?;
    if seal.is_empty() {
        return Ok(ApiResponse::fail("该安装未启用恢复密钥"));
    }
    match crate::recovery::unseal_with_recovery(&recovery_code, &seal) {
        Ok(master_key) => {
            crate::vault::set_key(master_key);
            Ok(ApiResponse::ok(SecurityStatus {
                has_master_password: true,
                unlocked: true,
            }))
        }
        Err(_) => Ok(ApiResponse::fail("恢复密钥不正确")),
    }
}

/// 恢复流程第二步：处于恢复解锁态时设置新密码（重派生验证器并轮换恢复码）
#[command]
pub fn complete_recovery(
    mut new_password: String,
    mut confirm_password: String,
) -> Result<ApiResponse<String>, String> {
    if !crate::vault::is_unlocked() {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("请先通过恢复密钥解锁"));
    }
    if new_password.chars().count() < 8 {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("新主密码至少需要 8 个字符"));
    }
    if new_password != confirm_password {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("两次输入的新主密码不一致"));
    }
    let Some(master_key) = crate::vault::get_key() else {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("解锁状态异常"));
    };

    // 恢复重设沿用恢复码解出的同一把数据密钥：只换 KEK 与恢复封印，不碰任何字段。
    let salt_new = crate::crypto::generate_salt_v3();
    let kek_new = crate::crypto::derive_from_salt(&new_password, &salt_new)?;
    new_password.zeroize();
    confirm_password.zeroize();
    let verifier_new = crate::vault::make_verifier(&master_key)?;
    let (recovery_code_new, recovery_seal_new) =
        crate::recovery::create_recovery_seal(&master_key)?;

    db::reset_security_config_db(
        &salt_new,
        &verifier_new,
        &recovery_seal_new,
        &master_key,
        &kek_new,
    )?;
    crate::vault::set_key(master_key);

    Ok(ApiResponse::ok_with_msg(
        recovery_code_new,
        "主密码已重置，请立即保存新的恢复密钥",
    ))
}

/// SMTP 配置（界面可填；密码只落库不回传）。
#[command]
pub fn get_smtp_config() -> Result<ApiResponse<db::SmtpConfig>, String> {
    db::get_smtp_config_db().map(ApiResponse::ok)
}

/// 保存 SMTP 配置。`password` 传 `None` 表示保持原密码不变。
#[command]
pub fn set_smtp_config(
    host: String,
    port: i64,
    from_addr: String,
    username: String,
    password: Option<String>,
    allow_insecure: bool,
) -> Result<ApiResponse<()>, String> {
    if host.len() > 253 {
        return Ok(ApiResponse::fail("SMTP 服务器地址过长"));
    }
    if from_addr.len() > 320 || username.len() > 320 {
        return Ok(ApiResponse::fail("发件人或用户名过长"));
    }
    if let Some(p) = password.as_deref() {
        if p.len() > 512 {
            return Ok(ApiResponse::fail("SMTP 密码过长"));
        }
    }
    match db::set_smtp_config_db(
        &host,
        port,
        &from_addr,
        &username,
        password.as_deref(),
        allow_insecure,
    ) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            "SMTP 配置已保存；配置后请用「发送测试邮件」确认能真正送达",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 发送一封测试邮件。
///
/// 为什么必须有这个按钮：SMTP 配错时用户完全看不出来——通知只在到期时
/// 静默降级到本地发件箱，「安心传承」看起来是开着的，家人却什么都收不到。
/// 测试发送把这个隐性失败提前暴露在配置当下。
#[command]
pub fn send_test_email(recipient: String) -> Result<ApiResponse<String>, String> {
    if recipient.len() > 320 {
        return Ok(ApiResponse::fail("收件人地址过长"));
    }
    let Some(settings) = crate::notify::resolve_smtp_settings() else {
        return Ok(ApiResponse::fail(
            "尚未配置 SMTP 服务器（需要服务器地址与发件人地址）",
        ));
    };
    if !recipient.contains('@') {
        return Ok(ApiResponse::fail("请填写有效的收件人邮箱"));
    }
    match crate::notify::send_test_message(&settings, &recipient) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            recipient,
            "测试邮件已发送，请检查收件箱（含垃圾邮件）",
        )),
        Err(e) => Ok(ApiResponse::fail(&format!(
            "测试邮件发送失败：{}。提示：多数邮箱需要「授权码」而非登录密码，端口常用 465/587",
            e
        ))),
    }
}

#[command]
pub fn get_heartbeat_status() -> Result<ApiResponse<crate::heartbeat::HeartbeatStatus>, String> {
    Ok(ApiResponse::ok(crate::heartbeat::current_status()))
}

/// 读取心跳配置（界面渲染用）。token 只回传「是否已配置」，不回传明文。
#[command]
pub fn get_heartbeat_config() -> Result<ApiResponse<serde_json::Value>, String> {
    let (enabled, token, endpoint) = db::get_heartbeat_config_db()?;
    Ok(ApiResponse::ok(serde_json::json!({
        "enabled": enabled,
        "has_token": !token.is_empty(),
        "endpoint": endpoint,
    })))
}

/// 保存心跳配置。token 传空串表示保持原值不变（避免界面把掩码值回写覆盖真令牌）。
#[command]
pub fn set_heartbeat_config(
    enabled: bool,
    token: String,
    endpoint: String,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let (_, current_token, _) = db::get_heartbeat_config_db()?;
    let next_token = if token.trim().is_empty() {
        current_token
    } else {
        token.trim().to_string()
    };
    if enabled && next_token.is_empty() {
        return Ok(ApiResponse::fail("启用心跳前请先填写令牌"));
    }
    if enabled {
        let ep = endpoint.trim();
        if ep.is_empty() {
            return Ok(ApiResponse::fail(
                "启用心跳前请先填写服务器地址（可自建 server/heartbeat-server.mjs）",
            ));
        }
        if !(ep.starts_with("http://") || ep.starts_with("https://")) {
            return Ok(ApiResponse::fail(
                "服务器地址必须以 http:// 或 https:// 开头",
            ));
        }
    }
    db::set_heartbeat_config_db(enabled, &next_token, endpoint.trim())?;
    Ok(ApiResponse::ok_with_msg(
        (),
        if enabled {
            "心跳已启用"
        } else {
            "心跳已关闭"
        },
    ))
}

#[command]
pub fn send_heartbeat_now() -> Result<ApiResponse<crate::heartbeat::HeartbeatStatus>, String> {
    Ok(ApiResponse::ok(crate::heartbeat::send_heartbeat()))
}

#[command]
pub fn set_autostart(app_handle: AppHandle, enabled: bool) -> Result<ApiResponse<()>, String> {
    use tauri_plugin_autostart::ManagerExt as _;
    let manager = app_handle.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            if enabled {
                "已设置开机自启"
            } else {
                "已关闭开机自启"
            },
        )),
        Err(e) => Ok(ApiResponse::fail(&format!("设置自启失败: {}", e))),
    }
}

#[command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<ApiResponse<()>, String> {
    use tauri::Manager as _;
    if let Some(win) = app_handle.get_webview_window("main") {
        let _ = win.hide();
    }
    Ok(ApiResponse::ok_with_msg((), "已隐藏到托盘"))
}

// 不能把整个命令 cfg 在 windows 上：lib.rs 的 generate_handler 无条件引用
// `commands::open_external`，非 Windows 编译会直接找不到符号（CI ubuntu 上爆过）。
// 函数体内的 Windows 分支才走 win_shell，其余平台用 open/xdg-open 降级。
#[command]
pub fn open_external(url: String) -> Result<ApiResponse<()>, String> {
    let checked = crate::vault::validate_external_url(&url)?;

    #[cfg(target_os = "windows")]
    {
        let op = win_shell::wide("open");
        let target = win_shell::wide(&checked);
        let result = unsafe {
            win_shell::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result <= 32 {
            return Err("无法打开链接".to_string());
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&checked)
            .spawn()
            .map_err(|e| format!("无法打开链接: {}", e))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&checked)
            .spawn()
            .map_err(|e| format!("无法打开链接: {}", e))?;
    }

    Ok(ApiResponse::ok_with_msg((), "已在浏览器打开"))
}

// ========== 继承人恢复包（Shamir 3-of-5） ==========

/// 生成继承人恢复包：份额写入打印友好的 HTML，元数据（不含份额）落库。
#[command]
pub fn generate_heir_recovery_kit(
    app_handle: AppHandle,
    threshold: u8,
    total: u8,
    passphrase: Option<String>,
) -> Result<ApiResponse<crate::heir_recovery::KitResult>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let dir = app_dir.join("继承人恢复包");
    let mut pass = passphrase;
    let result = crate::heir_recovery::generate_kit(threshold, total, pass.as_deref(), &dir);
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    match result {
        Ok((kit, _html)) => Ok(ApiResponse::ok_with_msg(
            kit,
            "恢复包已生成，请分发给不同保管人",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn heir_recovery_status() -> Result<ApiResponse<serde_json::Value>, String> {
    crate::heir_recovery::status()
        .map(|v| ApiResponse::ok(v.unwrap_or(serde_json::json!({"exists": false, "stale": false}))))
}

/// 恢复演练：只读校验（不重建、不落盘、不改变解锁状态）。
#[command]
pub fn run_recovery_drill(
    shares: Vec<String>,
    passphrase: Option<String>,
) -> Result<ApiResponse<crate::heir_recovery::DrillResult>, String> {
    if shares.len() > crate::heir_recovery::MAX_SHARES as usize {
        return Ok(ApiResponse::fail("份额数量超出上限"));
    }
    if shares.iter().any(|s| s.len() > 4096) {
        return Ok(ApiResponse::fail("份额长度超出上限"));
    }
    let mut pass = passphrase;
    let result = crate::heir_recovery::run_drill_with_passphrase(&shares, pass.as_deref());
    if let Some(value) = pass.as_mut() {
        value.zeroize();
    }
    result.map(ApiResponse::ok)
}

#[command]
pub fn clear_heir_recovery() -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::heir_recovery::clear_meta().map(|_| ApiResponse::ok_with_msg((), "恢复包元数据已清除"))
}

/// 用份额解锁（失能场景下由家人在新机器上执行）。
#[command]
pub fn unlock_with_recovery_shares(
    shares: Vec<String>,
    passphrase: Option<String>,
) -> Result<ApiResponse<SecurityStatus>, String> {
    if shares.is_empty() {
        return Ok(ApiResponse::fail("请至少提供一份份额"));
    }
    if shares.iter().any(|s| s.len() > 4096)
        || shares.len() > crate::heir_recovery::MAX_SHARES as usize
    {
        return Ok(ApiResponse::fail("份额数量或长度超出上限"));
    }
    let mut pass = passphrase;
    let result = crate::heir_recovery::recover_from_shares(&shares, pass.as_deref());
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    match result {
        Ok(msg) => Ok(ApiResponse::ok_with_msg(
            SecurityStatus {
                has_master_password: true,
                unlocked: true,
            },
            &msg,
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== KDBX 长期归档导出 ==========

/// 导出 KDBX 4 归档：家人用 KeePassXC 即可打开，脱离本应用仍可读。
#[command]
pub fn export_kdbx_archive(
    app_handle: AppHandle,
    archive_password: String,
) -> Result<ApiResponse<String>, String> {
    use crate::report::ReportLocale;
    // ReportLocale::parse 自带回退（未知值 → Zh），无需再包一层
    let locale =
        ReportLocale::parse(&std::env::var("DIGILEGACY_LOCALE").unwrap_or_else(|_| "zh-CN".into()));
    db::ensure_vault_readable_db()?;
    let report = db::get_inheritance_report_db()?;
    let hints = db::get_custody_hints_db().unwrap_or_default();
    let mut pass = archive_password;
    let result = crate::kdbx_export::export_kdbx(
        &report,
        locale,
        &hints.backup_location,
        &hints.recovery_code_location,
        pass.trim(),
    );
    pass.zeroize();
    match result {
        Ok(bytes) => {
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|e| format!("获取目录失败: {}", e))?;
            let dir = app_dir.join("KDBX归档");
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
            let path = dir.join(format!(
                "归零继承归档_{}.kdbx",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            ));
            crate::fsutil::write_private(&path, &bytes)
                .map_err(|e| format!("写入归档失败: {}", e))?;
            Ok(ApiResponse::ok_with_msg(
                path.to_string_lossy().to_string(),
                "KDBX 归档已导出（可用 KeePassXC 打开）",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}
