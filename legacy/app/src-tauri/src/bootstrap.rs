//! 非桌面入口的启动引导：CLI 与 HTTP 服务共用。
//!
//! 桌面端由 Tauri 的 `setup` 完成这些事（解析目录 → 开库 → 建托盘 → 起后台循环）。
//! CLI 与服务端没有 `AppHandle`，但需要同一套「目录定位 + 加密库打开 + 迁移」
//! 语义，否则同一个数据库在三个入口下会表现出不同行为。
//!
//! 这里刻意**不做**的事：
//! - 不建托盘、不弹窗、不启动 DMS 后台循环（那是常驻进程的职责，
//!   CLI 跑完就退出，服务端的循环由服务端自己决定是否开启）。
//! - 不在库不存在时静默造一个空库再报「一切正常」：首次使用必须显式 `init`。

use std::path::PathBuf;

use crate::db;

/// 打开（或创建）指定数据目录下的数据库，并把目录固定到进程级。
///
/// `explicit` 为空时按 `paths::resolve` 的优先级解析（环境变量 → 平台默认）。
/// 返回实际使用的目录，便于 CLI 打印「你正在操作哪个库」。
pub fn open(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    let dir = crate::paths::resolve(explicit.as_deref())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {}", e))?;
    crate::paths::set_data_dir(dir.clone());

    let db_path = dir.join("digilegacy.db");
    // 与桌面端完全一致的分支：有 dbkey.bin 就按加密库打开；
    // key 文件存在但解不开必须显式失败，不能回退成明文库。
    match crate::db_crypto::load_db_key(&dir)? {
        Some(key) => db::init_database_at_with_key(db_path, Some(&key)),
        None => db::init_database_at(db_path),
    }?;
    Ok(dir)
}

/// 数据库文件是否已存在（用于区分「首次初始化」与「打开既有库」）。
pub fn database_exists(explicit: Option<PathBuf>) -> Result<bool, String> {
    let dir = crate::paths::resolve(explicit.as_deref())?;
    Ok(dir.join("digilegacy.db").is_file())
}

/// 跑一次每日快照（与桌面启动时的行为一致，失败不阻断）。
pub fn run_startup_backup(dir: &std::path::Path) {
    match db::export_backup_json_db() {
        Ok(json) => {
            if let Err(e) = crate::backup::run_daily_backup(dir, &json) {
                eprintln!("自动备份失败: {}", e);
            }
        }
        Err(e) => eprintln!("自动备份跳过（导出失败）: {}", e),
    }
}

/// 运行时自检：给 `doctor` 子命令与 `GET /api/health` 共用。
///
/// 只报告**可判定**的事实，不做「应该没问题」式的猜测。
#[derive(Debug, serde::Serialize)]
pub struct HealthReport {
    pub ok: bool,
    pub data_dir: String,
    pub database_present: bool,
    pub db_encryption_enabled: bool,
    pub has_master_password: bool,
    pub unlocked: bool,
    pub platform_count: i64,
    pub asset_count: i64,
    pub schema_version: i64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, serde::Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

pub fn health(dir: &std::path::Path) -> HealthReport {
    let mut checks = Vec::new();

    let db_present = dir.join("digilegacy.db").is_file();
    checks.push(HealthCheck {
        name: "database".into(),
        status: if db_present { "ok" } else { "fail" }.into(),
        detail: if db_present {
            "数据库文件存在".into()
        } else {
            "数据库尚未初始化，请先运行 init".into()
        },
    });

    let encrypted = db::is_db_encryption_enabled(dir);
    checks.push(HealthCheck {
        name: "db_encryption".into(),
        status: "info".into(),
        detail: if encrypted {
            "整库加密已启用（SQLCipher）".into()
        } else {
            "整库加密未启用（仅字段级加密）".into()
        },
    });

    let (platform_count, asset_count, schema_version) = if db_present {
        match db::get_all_platforms_db() {
            Ok(v) => {
                let assets = db::get_all_assets_db().map(|a| a.len() as i64).unwrap_or(0);
                (
                    v.len() as i64,
                    assets,
                    db::schema_version_db().unwrap_or(-1),
                )
            }
            Err(e) => {
                checks.push(HealthCheck {
                    name: "query".into(),
                    status: "fail".into(),
                    detail: format!("读取平台库失败: {}", e),
                });
                (0, 0, -1)
            }
        }
    } else {
        (0, 0, -1)
    };

    if db_present && platform_count > 0 {
        checks.push(HealthCheck {
            name: "platform_library".into(),
            status: "ok".into(),
            detail: format!("平台库 {} 条", platform_count),
        });
    }

    let has_password = matches!(db::get_security_config_db(), Ok(Some(_)));
    let unlocked = has_password && crate::vault::is_unlocked();
    let vault_status: String = if !has_password {
        "info".into()
    } else if unlocked {
        "ok".into()
    } else {
        "locked".into()
    };
    checks.push(HealthCheck {
        name: "vault".into(),
        status: vault_status,
        detail: if !has_password {
            "未设置主密码（无字段级加密）".into()
        } else if unlocked {
            "已解锁".into()
        } else {
            "已锁定；敏感字段以占位符返回".into()
        },
    });

    let ok = checks.iter().all(|c| c.status != "fail");
    HealthReport {
        ok,
        data_dir: dir.to_string_lossy().to_string(),
        database_present: db_present,
        db_encryption_enabled: encrypted,
        has_master_password: has_password,
        unlocked,
        platform_count,
        asset_count,
        schema_version,
        checks,
    }
}
