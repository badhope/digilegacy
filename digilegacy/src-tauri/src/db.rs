use chrono::Local;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager};

use crate::models::*;
use crate::platform_data::get_all_platforms;

/// 全局数据库连接路径
static DB_PATH: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

/// 全局共享数据库连接（启用 WAL + busy_timeout，避免并发锁死）
static CONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));

/// 当前 schema 版本，写入 `PRAGMA user_version`。
/// 没有版本号时，未来任何一次加列/改表都无法判断「该不该迁移」，
/// 只能靠 PRAGMA table_info 逐列猜；版本号是后续升级的安全阀。
/// v3：新增 `smtp_config` 与 `custody_config`。
/// v4：新增 `heirs` 与 `assistant_config`。
/// v5：新增 `heir_recovery`（Shamir 继承人恢复包元数据）。
/// v6：新增 `imap_config` / `imap_seen`（入站签到）。
/// v7：新增 `ai_chat_log`（AI 助手会话历史）。
/// 这些表都用 `CREATE TABLE IF NOT EXISTS` 建，存量库无需逐列迁移；
/// 版本号仍然递增，让未来任何破坏性迁移能按数字分支而不是靠猜。
const SCHEMA_VERSION: i64 = 7;

/// 初始化数据库
pub fn init_database(app_handle: &AppHandle) -> Result<(), String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;

    std::fs::create_dir_all(&app_dir).map_err(|e| format!("创建应用目录失败: {}", e))?;

    init_database_at(app_dir.join("digilegacy.db"))
}

/// 在指定路径初始化数据库（供启动流程与测试复用）
pub(crate) fn init_database_at(db_path: PathBuf) -> Result<(), String> {
    {
        let mut path_guard = DB_PATH.lock().unwrap();
        *path_guard = Some(db_path.clone());
    }

    let conn = Connection::open(&db_path).map_err(|e| format!("打开数据库失败: {}", e))?;

    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 模式失败: {}", e))?;
    conn.pragma_update(None, "busy_timeout", "5000")
        .map_err(|e| format!("设置 busy_timeout 失败: {}", e))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("启用外键约束失败: {}", e))?;
    // WAL 下 synchronous=NORMAL 已能保证崩溃一致性（只可能丢最后一个事务），
    // 换来数倍写入吞吐；每日快照 + 事务日志覆盖了极端断电场景。
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("设置 synchronous 失败: {}", e))?;
    // 页缓存 64MB：资产/政策查询以内存命中为主，默认 2MB 在 152 平台同步时会反复读盘
    conn.pragma_update(None, "cache_size", "-65536")
        .map_err(|e| format!("设置 cache_size 失败: {}", e))?;
    // 临时表放内存，排序（资产按价值/时间）不再落盘
    conn.pragma_update(None, "temp_store", "MEMORY")
        .map_err(|e| format!("设置 temp_store 失败: {}", e))?;
    // 256MB 内存映射：读取路径绕过 read() 系统调用
    conn.pragma_update(None, "mmap_size", "268435456").ok();
    // 启动自检：损坏的库必须在启动时暴露，而不是等用户点保存时才报错
    conn.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
        .map_err(|e| format!("数据库自检失败: {}", e))?;

    create_tables(&conn).map_err(|e| format!("创建表失败: {}", e))?;
    migrate_schema(&conn).map_err(|e| format!("数据库迁移失败: {}", e))?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| format!("写入 schema 版本失败: {}", e))?;
    sync_platforms(&conn)?;

    conn.execute(
        "INSERT OR IGNORE INTO dms_config (id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in) VALUES (1, 0, 30, 3, 7, 14, NULL)",
        [],
    ).map_err(|e| format!("初始化DMS失败: {}", e))?;

    {
        let mut conn_guard = CONN.lock().unwrap();
        *conn_guard = Some(conn);
    }

    Ok(())
}

/// 创建数据库表
fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS platforms (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            name_en TEXT,
            category TEXT NOT NULL,
            subtype TEXT,
            has_property INTEGER DEFAULT 0,
            has_personality INTEGER DEFAULT 0,
            inheritable INTEGER DEFAULT 0,
            required_documents TEXT,
            process_description TEXT,
            estimated_difficulty TEXT,
            official_policy_url TEXT,
            estimated_time_days TEXT,
            notes TEXT
        );

        CREATE TABLE IF NOT EXISTS digital_assets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            platform_id INTEGER NOT NULL,
            account_identifier TEXT,
            account_description TEXT,
            category TEXT NOT NULL,
            estimated_value REAL DEFAULT 0,
            value_currency TEXT DEFAULT 'CNY',
            disposal_method TEXT NOT NULL DEFAULT 'no_action',
            heir_name TEXT,
            heir_contact TEXT,
            notes TEXT DEFAULT '',
            credential_enc TEXT DEFAULT '',
            is_confirmed INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (platform_id) REFERENCES platforms(id)
        );

        CREATE TABLE IF NOT EXISTS security_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            kdf_salt TEXT NOT NULL,
            verifier TEXT NOT NULL,
            recovery_seal TEXT DEFAULT '',
            created_at TEXT NOT NULL,
            -- 两层密钥：随机 DEK 被密码派生 KEK 包裹后存这里。
            -- 空串 = legacy 旧库（密码派生密钥即数据密钥），下次改密时自动迁移。
            wrapped_dek TEXT DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS user_profile (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            full_name TEXT,
            phone TEXT,
            email TEXT,
            id_last4 TEXT,
            emergency_contact_name TEXT,
            emergency_contact_phone TEXT,
            emergency_contact_relation TEXT,
            emergency_contact_email TEXT,
            heir_primary_name TEXT,
            heir_primary_phone TEXT,
            heir_primary_relation TEXT,
            heir_primary_email TEXT,
            check_in_interval_days INTEGER DEFAULT 30,
            last_check_in TEXT,
            dms_enabled INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS check_in_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            checked_at TEXT NOT NULL,
            status TEXT NOT NULL,
            note TEXT
        );

        CREATE TABLE IF NOT EXISTS dms_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            enabled INTEGER DEFAULT 0,
            check_in_interval_days INTEGER DEFAULT 30,
            stage1_wait_days INTEGER DEFAULT 3,
            stage2_wait_days INTEGER DEFAULT 7,
            stage3_wait_days INTEGER DEFAULT 14,
            last_check_in TEXT
        );

        CREATE TABLE IF NOT EXISTS heartbeat_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            enabled INTEGER DEFAULT 0,
            token TEXT DEFAULT '',
            endpoint TEXT DEFAULT ''
        );

        -- SMTP 配置：此前只能通过环境变量设置，普通用户根本配不起来，
        -- 结果「安心传承」的通知永远只落本地发件箱，家人一封邮件也收不到。
        -- 密码走 vault 字段级加密（与凭据同一条路径），不明文落库。
        -- 通用审核留痕：任何场景（政策库、法律文书、家人说明…）共用一张表。
        -- 只追加、不改写：审核历史本身就是证据，修改会毁掉可追溯性。
        CREATE TABLE IF NOT EXISTS review_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id TEXT NOT NULL,
            item_key TEXT NOT NULL,
            fingerprint TEXT NOT NULL,
            ai_verdict TEXT NOT NULL,
            ai_note TEXT DEFAULT '',
            human_verdict TEXT,
            human_note TEXT DEFAULT '',
            decided_by TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_review_item ON review_records(profile_id, item_key, id DESC);

        -- 继承人名单：支持多继承人 + 独立执行人。
        -- 旧模型只有 user_profile 里的一组 heir_primary_* 字段，
        -- 第二个孩子、配偶、律师都没有位置，报告只能写一个人。
        CREATE TABLE IF NOT EXISTS heirs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            relation TEXT DEFAULT '',
            phone TEXT DEFAULT '',
            email TEXT DEFAULT '',
            share_note TEXT DEFAULT '',
            is_executor INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- AI 助手配置：**不内置任何端点**。用户自己有 OpenAI 兼容服务时
        -- 填地址与 Key；没有就保持关闭。Key 走字段级加密，永不明文落库。
        CREATE TABLE IF NOT EXISTS assistant_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            enabled INTEGER DEFAULT 0,
            endpoint TEXT DEFAULT '',
            model TEXT DEFAULT '',
            api_key_enc TEXT DEFAULT '',
            updated_at TEXT NOT NULL
        );

        -- 保管位置：备份文件放在哪、恢复码抄在哪。
        -- 这两个信息决定了「家人能不能真正拿到凭据」，此前完全没有字段可填。
        -- 属于敏感元数据（泄露即等于指出攻击目标），因此同样走字段级加密。
        CREATE TABLE IF NOT EXISTS custody_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            backup_location_enc TEXT DEFAULT '',
            recovery_code_location_enc TEXT DEFAULT '',
            updated_at TEXT NOT NULL
        );

        -- 继承人恢复包元数据（Shamir 份额本身绝不落库，只存门限/份数/指纹/盐）。
        -- 主密码轮换时必须清空：旧份额解不开新数据密钥。
        -- AI 助手会话历史：本地留痕，用户可见可清除。
        -- 问题原文由用户输入（不含凭据——出境白名单在 ai_assist 保证），
        -- 回答是模型输出，两者都只是文本；不存任何敏感数据快照。
        CREATE TABLE IF NOT EXISTS ai_chat_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            question TEXT NOT NULL,
            answer TEXT NOT NULL,
            model TEXT DEFAULT '',
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ai_chat_created ON ai_chat_log(created_at DESC);

        -- IMAP 入站签到：回一封邮件 = 我还安好。密码走 vault 加密，不明文落库。
        CREATE TABLE IF NOT EXISTS imap_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            enabled INTEGER DEFAULT 0,
            host TEXT DEFAULT '',
            port INTEGER DEFAULT 993,
            username TEXT DEFAULT '',
            password_enc TEXT DEFAULT '',
            allowed_senders TEXT DEFAULT '',
            lookback_days INTEGER DEFAULT 14,
            updated_at TEXT
        );
        -- 去重记录：同一条邮件只结算一次
        CREATE TABLE IF NOT EXISTS imap_seen (
            uid INTEGER PRIMARY KEY,
            sender TEXT DEFAULT '',
            seen_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS heir_recovery (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            threshold INTEGER NOT NULL,
            total INTEGER NOT NULL,
            fingerprint TEXT NOT NULL,
            created_at TEXT NOT NULL,
            passphrase_protected INTEGER DEFAULT 0,
            kdf_salt TEXT DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS smtp_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            host TEXT DEFAULT '',
            port INTEGER DEFAULT 587,
            from_addr TEXT DEFAULT '',
            username TEXT DEFAULT '',
            password_enc TEXT DEFAULT '',
            allow_insecure INTEGER DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        -- 政策库补丁：内置平台数据每次启动都会被 sync_platforms 覆盖，
        -- 用户导入的更新必须单独存这里，并在读取时叠加到内置数据之上。
        CREATE TABLE IF NOT EXISTS platform_overrides (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            name_en TEXT DEFAULT '',
            category TEXT NOT NULL,
            subtype TEXT DEFAULT '',
            has_property INTEGER DEFAULT 0,
            has_personality INTEGER DEFAULT 0,
            inheritable INTEGER DEFAULT 0,
            required_documents TEXT DEFAULT '',
            process_description TEXT DEFAULT '',
            estimated_difficulty TEXT DEFAULT '',
            official_policy_url TEXT DEFAULT '',
            estimated_time_days TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_assets_category ON digital_assets(category);
        CREATE INDEX IF NOT EXISTS idx_assets_platform ON digital_assets(platform_id);
        CREATE INDEX IF NOT EXISTS idx_assets_disposal ON digital_assets(disposal_method);
        -- 资产列表默认按 created_at DESC 排序，无索引时全表扫描 + 临时 B 树排序
        CREATE INDEX IF NOT EXISTS idx_assets_created ON digital_assets(created_at DESC);
        -- 仪表板按「未解密占位符」筛账号标识，前缀匹配可用索引
        CREATE INDEX IF NOT EXISTS idx_assets_identifier ON digital_assets(account_identifier);
        CREATE INDEX IF NOT EXISTS idx_checkin_checked_at ON check_in_log(checked_at);
        CREATE INDEX IF NOT EXISTS idx_heirs_order ON heirs(sort_order, id);
    ",
    )
}

/// 增量迁移：为既有数据库补齐新列（幂等）
fn migrate_schema(conn: &Connection) -> rusqlite::Result<()> {
    let has_credential: bool = conn
        .prepare("PRAGMA table_info(digital_assets)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "credential_enc");

    if !has_credential {
        conn.execute_batch("ALTER TABLE digital_assets ADD COLUMN credential_enc TEXT DEFAULT ''")?;
    }

    let has_recovery: bool = conn
        .prepare("PRAGMA table_info(security_config)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "recovery_seal");

    if !has_recovery {
        conn.execute_batch("ALTER TABLE security_config ADD COLUMN recovery_seal TEXT DEFAULT ''")?;
    }

    let has_wrapped_dek = conn
        .prepare("PRAGMA table_info(security_config)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "wrapped_dek");
    if !has_wrapped_dek {
        conn.execute_batch("ALTER TABLE security_config ADD COLUMN wrapped_dek TEXT DEFAULT ''")?;
    }

    let profile_columns: Vec<String> = conn
        .prepare("PRAGMA table_info(user_profile)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    for col in ["emergency_contact_email", "heir_primary_email"] {
        if !profile_columns.iter().any(|name| name == col) {
            conn.execute_batch(&format!(
                "ALTER TABLE user_profile ADD COLUMN {} TEXT DEFAULT ''",
                col
            ))?;
        }
    }
    Ok(())
}

/// 幂等同步平台数据：每次启动 upsert 全部平台（含政策与官方链接），
/// 让存量数据库跟随平台库更新，用户自建资产数据不受影响。
pub(super) fn sync_platforms(conn: &Connection) -> Result<(), String> {
    let platforms = get_all_platforms();

    // 单事务 + prepare 复用：逐条 autocommit 会让 152 个平台各走一次磁盘落盘，
    // 产生可感知的冷启动耗时方差；包进单事务并复用预编译语句消除之。
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("开启平台同步事务失败: {}", e))?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO platforms (id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, name_en=excluded.name_en, category=excluded.category,
                subtype=excluded.subtype, has_property=excluded.has_property,
                has_personality=excluded.has_personality, inheritable=excluded.inheritable,
                required_documents=excluded.required_documents,
                process_description=excluded.process_description,
                estimated_difficulty=excluded.estimated_difficulty,
                official_policy_url=excluded.official_policy_url,
                estimated_time_days=excluded.estimated_time_days,
                notes=excluded.notes",
        ).map_err(|e| format!("准备平台同步语句失败: {}", e))?;
        for platform in platforms {
            stmt.execute(params![
                platform.id,
                platform.name,
                platform.name_en,
                platform.category,
                platform.subtype,
                if platform.has_property { 1 } else { 0 },
                if platform.has_personality { 1 } else { 0 },
                if platform.inheritable { 1 } else { 0 },
                platform.required_documents,
                platform.process_description,
                platform.estimated_difficulty,
                platform.official_policy_url,
                platform.estimated_time_days,
                platform.notes,
            ])
            .map_err(|e| format!("同步平台数据失败: {}", e))?;
        }
    }
    tx.commit()
        .map_err(|e| format!("提交平台同步事务失败: {}", e))?;

    Ok(())
}

/// 获取数据库连接（共享连接，避免并发锁死）
pub(crate) fn get_conn() -> Result<MutexGuard<'static, Option<Connection>>, String> {
    let guard = CONN.lock().map_err(|_| "数据库锁中毒".to_string())?;
    if guard.is_none() {
        return Err("数据库未初始化".to_string());
    }
    Ok(guard)
}

/// 辅助函数：解析 rusqlite 行到 Platform
fn row_to_platform(row: &rusqlite::Row) -> rusqlite::Result<Platform> {
    Ok(Platform {
        id: row.get(0)?,
        name: row.get(1)?,
        category: row.get(3)?,
        subtype: row.get(4)?,
        has_property: row.get::<_, i64>(5)? != 0,
        has_personality: row.get::<_, i64>(6)? != 0,
        inheritable: row.get::<_, i64>(7)? != 0,
        required_documents: row.get(8)?,
        process_description: row.get(9)?,
        estimated_difficulty: row.get(10)?,
        official_policy_url: row.get(11)?,
        estimated_time_days: row.get(12)?,
        notes: row.get(13)?,
    })
}

/// 辅助函数：解析 rusqlite 行到 DigitalAsset（敏感字段自动解密）
pub(crate) fn row_to_asset(row: &rusqlite::Row) -> rusqlite::Result<DigitalAsset> {
    let dec = |idx: usize| -> rusqlite::Result<String> {
        let raw: String = row.get::<_, Option<String>>(idx)?.unwrap_or_default();
        Ok(crate::vault::decrypt_or_placeholder(&raw))
    };
    Ok(DigitalAsset {
        id: row.get(0)?,
        platform_id: row.get(1)?,
        platform_name: row.get(2)?,
        account_identifier: dec(3)?,
        account_description: dec(4)?,
        category: row.get(5)?,
        estimated_value: row.get(6)?,
        value_currency: row.get(7)?,
        disposal_method: row.get(8)?,
        heir_name: {
            let v = dec(9)?;
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        },
        heir_contact: {
            let v = dec(10)?;
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        },
        notes: dec(11)?,
        has_credential: row.get::<_, i64>(15).unwrap_or(0) != 0,
        is_confirmed: row.get::<_, i64>(12)? != 0,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

// ========== 平台查询 ==========

pub fn get_all_platforms_db() -> Result<Vec<Platform>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn.prepare("SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes FROM platforms ORDER BY category, name")
        .map_err(|e| format!("查询平台失败: {}", e))?;

    let mut platforms = stmt
        .query_map([], row_to_platform)
        .map_err(|e| format!("平台数据解析失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("平台数据收集失败: {}", e))?;

    platforms::apply_platform_overrides(conn, &mut platforms)?;

    Ok(platforms)
}

/// 报告用：把资产实际引用到的平台政策收成一份索引。
///
/// 只收 `assets` 用到的平台，避免报告里塞进 152 条无关政策。
/// 扩展字段（变现路径/风险提示/标签）来自内置表；政策补丁覆盖过的平台
/// 以 `platforms` 表为准（补丁镜像后的结果），保证导出与界面读同一份数据。
pub fn get_policy_briefs_db(platform_ids: &[i64]) -> Result<Vec<PlatformPolicyBrief>, String> {
    use std::collections::HashSet;
    let wanted: HashSet<i64> = platform_ids.iter().copied().collect();
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let platforms = get_all_platforms_db()?;
    let meta_by_id: std::collections::HashMap<i64, crate::platform_data::PlatformEntry> =
        crate::platform_data::get_all_platforms()
            .into_iter()
            .map(|m| (m.id, m))
            .collect();
    let mut out: Vec<PlatformPolicyBrief> = platforms
        .into_iter()
        .filter(|p| wanted.contains(&p.id))
        .map(|p| {
            let meta = meta_by_id.get(&p.id);
            PlatformPolicyBrief {
                platform_id: p.id,
                name: p.name,
                required_documents: p.required_documents,
                process_description: p.process_description,
                estimated_difficulty: p.estimated_difficulty,
                estimated_time_days: p.estimated_time_days,
                official_policy_url: p.official_policy_url,
                monetization_path: meta
                    .map(|m| m.monetization_path.clone())
                    .unwrap_or_default(),
                risk_warnings: meta.map(|m| m.risk_warnings.clone()).unwrap_or_default(),
                tags: meta.map(|m| m.tags.clone()).unwrap_or_default(),
                has_source: meta
                    .map(|m| !m.data_source.trim().is_empty() && !m.data_source.contains("未核实"))
                    .unwrap_or(false),
                last_reviewed: meta.map(|m| m.last_reviewed.clone()).unwrap_or_default(),
                data_source: meta.map(|m| m.data_source.clone()).unwrap_or_default(),
            }
        })
        .collect();
    out.sort_by_key(|p| p.platform_id);
    Ok(out)
}

// ========== 数字资产 CRUD ==========

/// 写敏感字段前的前置校验：已设置主密码但处于锁定状态时，禁止写入。
/// 此时 vault::encrypt_if_needed 会静默明文直通，用户以为已加密实则明文落库——
/// 这是历史缺陷，必须在此处拦截。尚未启用加密（无 security_config）时放行：
/// 存量明文模式由 encrypt_existing_data_db 在启用加密后统一收编。
/// 供其他 db 子模块复用的写入门禁（锁定态一律拒绝）。
pub(crate) fn ensure_vault_writable_pub(conn: &Connection) -> Result<(), String> {
    ensure_vault_writable(conn)
}

pub(crate) fn ensure_vault_readable_pub(conn: &Connection) -> Result<(), String> {
    ensure_vault_readable(conn)
}

pub(crate) fn ensure_vault_readable_db() -> Result<(), String> {
    let guard = get_conn()?;
    ensure_vault_readable(guard.as_ref().unwrap())
}

pub(crate) fn ensure_vault_writable_db() -> Result<(), String> {
    let guard = get_conn()?;
    ensure_vault_writable(guard.as_ref().unwrap())
}

fn ensure_vault_readable(conn: &Connection) -> Result<(), String> {
    let configured: i64 = conn
        .query_row("SELECT COUNT(*) FROM security_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("查询保险库配置失败: {}", e))?;
    if configured == 0 {
        return Ok(());
    }
    if vault_key_matches_config(conn) {
        Ok(())
    } else {
        Err("主密码已设置但处于锁定状态，请先解锁后再读取敏感历史".to_string())
    }
}

pub(crate) fn ensure_vault_writable(conn: &Connection) -> Result<(), String> {
    let configured: i64 = conn
        .query_row("SELECT COUNT(*) FROM security_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("查询保险库配置失败: {}", e))?;
    if configured == 0 {
        // 尚未启用主密码：明文模式，放行
        return Ok(());
    }
    if !crate::vault::is_unlocked() {
        return Err("主密码已设置但处于锁定状态，请先解锁后再保存".to_string());
    }
    // 内存里有密钥 ≠ 密钥正确。恢复流程用旧封印解出的密钥可能与当前验证器不匹配，
    // 那种状态下读取全是占位符、写入会把占位符当明文写回（数据丢失）。
    // 因此写路径必须验证密钥与库里验证器一致，而不只看「有没有 key」。
    if !vault_key_matches_config(conn) {
        return Err("当前会话密钥与主密码不匹配，请重新锁定后再解锁".to_string());
    }
    Ok(())
}

/// 内存中的密钥是否能通过库内验证器校验。
pub fn vault_key_matches_config(conn: &Connection) -> bool {
    let Ok(Some((_salt, verifier))) = read_security_config(conn) else {
        return false;
    };
    match crate::vault::get_key() {
        Some(key) => crate::vault::verify_key(&verifier, &key),
        None => false,
    }
}

/// 读取安全配置（不依赖全局连接，供内部守卫复用）。
fn read_security_config(conn: &Connection) -> Result<Option<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT kdf_salt, verifier FROM security_config WHERE id = 1")
        .map_err(|e| format!("查询安全配置失败: {}", e))?;
    let mut rows = stmt.query([]).map_err(|e| format!("查询执行失败: {}", e))?;
    if let Some(row) = rows.next().map_err(|e| format!("读取失败: {}", e))? {
        Ok(Some((
            row.get(0).map_err(|e| format!("{}", e))?,
            row.get(1).map_err(|e| format!("{}", e))?,
        )))
    } else {
        Ok(None)
    }
}

/// 写路径的统一守卫：拒绝把「解密失败的占位符」当作真实内容写回。
///
/// 背景：`decrypt_or_placeholder` 在锁定/错钥匙时返回占位符，界面会把它渲染到表单里；
/// 用户点保存时整张表单原样提交，占位符就被当成明文重新加密，原始数据永久丢失。
/// 只拦截占位符本身，不影响用户真的想把字段清空（空串）或手动输入同款文本之外的内容。
pub(crate) fn reject_placeholder(field: &str, value: &str) -> Result<(), String> {
    if crate::vault::is_placeholder(value) {
        return Err(format!(
            "{}当前处于加密锁定状态，无法保存。请先解锁后重试（本次未写入任何数据）",
            field
        ));
    }
    Ok(())
}

// ========== 安全配置与凭据保管 ==========

/// 读取安全配置：(kdf_salt_base64, verifier)
pub fn get_security_config_db() -> Result<Option<(String, String)>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare("SELECT kdf_salt, verifier FROM security_config WHERE id = 1")
        .map_err(|e| format!("查询安全配置失败: {}", e))?;
    let mut rows = stmt.query([]).map_err(|e| format!("查询执行失败: {}", e))?;
    if let Some(row) = rows.next().map_err(|e| format!("读取失败: {}", e))? {
        let salt: String = row.get(0).map_err(|e| format!("{}", e))?;
        let verifier: String = row.get(1).map_err(|e| format!("{}", e))?;
        Ok(Some((salt, verifier)))
    } else {
        Ok(None)
    }
}

/// 读取恢复封印（主密钥被恢复码加密后的密文；空表示未启用恢复）
pub fn get_recovery_seal_db() -> Result<String, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let seal: String = conn
        .query_row(
            "SELECT COALESCE(recovery_seal, '') FROM security_config WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    Ok(seal)
}

pub fn set_recovery_seal_db(seal: &str) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "UPDATE security_config SET recovery_seal=?1 WHERE id=1",
        params![seal],
    )
    .map_err(|e| format!("保存恢复密钥失败: {}", e))?;
    Ok(())
}

/// 心跳配置（启用开关 / 令牌 / 端点）存本地库，替代只认环境变量的旧实现：
/// 环境变量对普通用户不可达，等于该功能在发布版里无法开启。
pub fn get_heartbeat_config_db() -> Result<(bool, String, String), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let row = conn.query_row(
        "SELECT enabled, COALESCE(token,''), COALESCE(endpoint,'') FROM heartbeat_config WHERE id=1",
        [],
        |r| Ok((r.get::<_, i64>(0)? != 0, r.get::<_, String>(1)?, r.get::<_, String>(2)?)),
    );
    let (enabled, raw_token, endpoint) = match row {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Ok((false, String::new(), String::new()))
        }
        Err(e) => return Err(format!("读取心跳配置失败: {}", e)),
    };
    if raw_token.is_empty() {
        return Ok((enabled, String::new(), endpoint));
    }
    if crate::vault::is_encrypted(&raw_token) {
        let token = crate::vault::decrypt_if_needed(&raw_token)
            .map_err(|_| "心跳令牌已加密，请先解锁后重试".to_string())?;
        return Ok((enabled, token, endpoint));
    }
    if read_security_config(conn)?.is_none() {
        return Err("心跳令牌必须保存在已启用主密码的保险库中".to_string());
    }
    ensure_vault_writable(conn)?;
    let encrypted = crate::vault::encrypt_field_v2(
        &raw_token,
        &*crate::vault::get_key().ok_or("请先解锁后迁移心跳令牌")?,
    )?;
    conn.execute(
        "UPDATE heartbeat_config SET token=?1 WHERE id=1",
        params![encrypted],
    )
    .map_err(|e| format!("迁移心跳令牌失败: {}", e))?;
    Ok((enabled, raw_token, endpoint))
}

pub fn set_heartbeat_config_db(enabled: bool, token: &str, endpoint: &str) -> Result<(), String> {
    if !endpoint.trim().is_empty() {
        crate::heartbeat::validate_heartbeat_endpoint(endpoint)?;
    }
    ensure_vault_writable_db()?;
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    if !token.is_empty() && read_security_config(conn)?.is_none() {
        return Err("心跳令牌必须保存在已启用主密码的保险库中".to_string());
    }
    let stored_token = if token.is_empty() {
        String::new()
    } else {
        let key = crate::vault::get_key().ok_or("请先解锁后保存心跳令牌")?;
        crate::vault::encrypt_field_v2(token, &key)?
    };
    conn.execute(
        "INSERT INTO heartbeat_config (id, enabled, token, endpoint) VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET enabled=?1, token=?2, endpoint=?3",
        params![enabled as i64, stored_token, endpoint.trim()],
    )
    .map_err(|e| format!("保存心跳配置失败: {}", e))?;
    Ok(())
}

/// SMTP 配置（不含密码明文）。
///
/// `has_password` 只表示库里有密文，不返回明文——界面只需要知道「已保存」。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SmtpConfig {
    pub host: String,
    pub port: i64,
    pub from_addr: String,
    pub username: String,
    pub has_password: bool,
    pub allow_insecure: bool,
}

/// AI 助手配置（不含 Key 明文）。默认关闭、无内置端点。
pub fn get_assistant_config_db() -> Result<crate::ai_assist::AssistantConfig, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let row = conn.query_row(
        "SELECT enabled, COALESCE(endpoint,''), COALESCE(model,''), COALESCE(api_key_enc,'') FROM assistant_config WHERE id=1",
        [],
        |r| {
            Ok(crate::ai_assist::AssistantConfig {
                enabled: r.get::<_, i64>(0)? != 0,
                endpoint: r.get::<_, String>(1)?,
                model: r.get::<_, String>(2)?,
                has_key: !r.get::<_, String>(3)?.is_empty(),
            })
        },
    );
    match row {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Default::default()),
        Err(e) => Err(format!("读取助手配置失败: {}", e)),
    }
}

/// 保存助手配置。`api_key` 为 `None` 表示保持现有 Key 不变。
pub fn set_assistant_config_db(
    enabled: bool,
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_writable(conn)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let key_enc = match api_key {
        Some(k) if !k.is_empty() => crate::vault::encrypt_if_needed(k)?,
        Some(_) => String::new(),
        None => conn
            .query_row(
                "SELECT COALESCE(api_key_enc,'') FROM assistant_config WHERE id=1",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default(),
    };
    conn.execute(
        "INSERT INTO assistant_config (id, enabled, endpoint, model, api_key_enc, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET enabled=?1, endpoint=?2, model=?3, api_key_enc=?4, updated_at=?5",
        params![enabled as i64, endpoint, model, key_enc, now],
    )
    .map_err(|e| format!("保存助手配置失败: {}", e))?;
    Ok(())
}

/// 取出解密后的 API Key（仅供请求使用，绝不回传前端）。
pub fn get_assistant_key_plain() -> Result<Option<String>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let enc: String = conn
        .query_row(
            "SELECT COALESCE(api_key_enc,'') FROM assistant_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if enc.is_empty() {
        return Ok(None);
    }
    if crate::vault::is_encrypted(&enc) && !crate::vault::is_unlocked() {
        return Err("API Key 已加密，请先解锁后重试".to_string());
    }
    crate::vault::decrypt_if_needed(&enc).map(Some)
}

pub fn get_smtp_config_db() -> Result<SmtpConfig, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let row = conn.query_row(
        "SELECT COALESCE(host,''), port, COALESCE(from_addr,''), COALESCE(username,''), COALESCE(password_enc,''), allow_insecure FROM smtp_config WHERE id=1",
        [],
        |r| {
            Ok(SmtpConfig {
                host: r.get::<_, String>(0)?,
                port: r.get::<_, i64>(1)?,
                from_addr: r.get::<_, String>(2)?,
                username: r.get::<_, String>(3)?,
                has_password: !r.get::<_, String>(4)?.is_empty(),
                allow_insecure: r.get::<_, i64>(5)? != 0,
            })
        },
    );
    match row {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(SmtpConfig {
            port: 587,
            ..Default::default()
        }),
        Err(e) => Err(format!("读取 SMTP 配置失败: {}", e)),
    }
}

/// 保存 SMTP 配置。`password` 为 `None` 表示「保持现有密码不变」——
/// 界面只回显 `has_password`，用户不重填时不能把已保存的密码清空。
pub fn set_smtp_config_db(
    host: &str,
    port: i64,
    from_addr: &str,
    username: &str,
    password: Option<&str>,
    allow_insecure: bool,
) -> Result<(), String> {
    if !(1..=65535).contains(&port) {
        return Err("SMTP 端口必须在 1 到 65535 之间".to_string());
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_writable(conn)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let password_enc = match password {
        Some(p) if !p.is_empty() => crate::vault::encrypt_if_needed(p)?,
        Some(_) => String::new(),
        None => conn
            .query_row(
                "SELECT COALESCE(password_enc,'') FROM smtp_config WHERE id=1",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_default(),
    };
    conn.execute(
        "INSERT INTO smtp_config (id, host, port, from_addr, username, password_enc, allow_insecure, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET host=?1, port=?2, from_addr=?3, username=?4, password_enc=?5, allow_insecure=?6, updated_at=?7",
        params![host, port, from_addr, username, password_enc, allow_insecure as i64, now],
    )
    .map_err(|e| format!("保存 SMTP 配置失败: {}", e))?;
    Ok(())
}

/// 取出解密后的 SMTP 明文密码（仅供投递使用，绝不回传前端）。
pub fn get_smtp_password_plain() -> Result<Option<String>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let enc: String = conn
        .query_row(
            "SELECT COALESCE(password_enc,'') FROM smtp_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    if enc.is_empty() {
        return Ok(None);
    }
    if crate::vault::is_encrypted(&enc) && !crate::vault::is_unlocked() {
        return Err("SMTP 密码已加密，请先解锁后重试".to_string());
    }
    crate::vault::decrypt_if_needed(&enc).map(Some)
}

/// 保存安全配置（含被包裹的数据密钥）。
pub fn set_security_config_with_dek_db(
    kdf_salt: &str,
    verifier: &str,
    wrapped_dek: &str,
) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO security_config (id, kdf_salt, verifier, wrapped_dek, created_at) VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET kdf_salt=?1, verifier=?2, wrapped_dek=?3",
        params![kdf_salt, verifier, wrapped_dek, now],
    )
    .map_err(|e| format!("保存安全配置失败: {}", e))?;
    Ok(())
}

/// 读取被包裹的数据密钥（空串 = legacy 库）。
pub fn get_wrapped_dek_db() -> Result<String, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let value: String = conn
        .query_row(
            "SELECT COALESCE(wrapped_dek, '') FROM security_config WHERE id=1",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("读取数据密钥失败: {}", e))?;
    Ok(value)
}

/// 保存资产凭据（非空时需解锁；空字符串表示清除）
pub fn set_asset_credential_db(id: i64, credential: &str) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let stored = if credential.is_empty() {
        String::new()
    } else {
        if !crate::vault::is_unlocked() {
            return Err("请先解锁后再保存凭据".to_string());
        }
        crate::vault::encrypt_if_needed(credential)?
    };
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE digital_assets SET credential_enc=?1, updated_at=?2 WHERE id=?3",
        params![stored, now, id],
    )
    .map_err(|e| format!("保存凭据失败: {}", e))?;
    Ok(())
}

/// 读取资产凭据（需解锁；未保存返回空字符串）
pub fn get_asset_credential_db(id: i64) -> Result<String, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let raw: String = conn
        .query_row(
            "SELECT COALESCE(credential_enc, '') FROM digital_assets WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| format!("查询凭据失败: {}", e))?;
    if raw.is_empty() {
        return Ok(String::new());
    }
    crate::vault::decrypt_if_needed(&raw)
}

/// 修改主密码：校验旧密码后全库重加密并更新安全配置（单事务，失败自动回滚）。
/// 返回轮换后的新恢复密钥——改密即轮换恢复码，UI 必须展示给用户抄写，
/// 否则旧恢复码随旧封印一起失效、恢复链路断裂（历史缺陷）。
pub fn change_master_password_db(
    old_password: &str,
    new_password: &str,
    confirm_password: &str,
) -> Result<String, String> {
    if new_password.chars().count() < 8 {
        return Err("新主密码至少需要 8 个字符".to_string());
    }
    if new_password != confirm_password {
        return Err("两次输入的新主密码不一致".to_string());
    }

    let Some((salt_b64, verifier)) = get_security_config_db()? else {
        return Err("尚未设置主密码".to_string());
    };
    // 兼容 v1/v2/v3 盐前缀（v3 为 Argon2id，参数随盐串存储）
    let kek_old = crate::crypto::derive_from_salt(old_password, &salt_b64)?;
    let wrapped_old = get_wrapped_dek_db()?;

    // 数据密钥的来源有两种：
    // - 新库：wrapped_dek 非空 → 用旧 KEK 解出 DEK；
    // - legacy 库：wrapped_dek 为空 → 密码派生密钥本身就是数据密钥。
    // 两种情况都正确，且都在下方统一迁到「DEK + wrapped_dek」结构。
    let (dek, migrated_from_legacy) = if wrapped_old.is_empty() {
        if !crate::vault::verify_key(&verifier, &kek_old) {
            return Err("当前主密码错误".to_string());
        }
        (
            crate::vault::get_key().ok_or("请先解锁后再修改主密码")?,
            true,
        )
    } else {
        let dek = crate::vault::unwrap_data_key(&wrapped_old, &kek_old)
            .map_err(|_| "当前主密码错误".to_string())?;
        if !crate::vault::verify_key(&verifier, &dek) {
            return Err("当前主密码错误".to_string());
        }
        (dek, false)
    };

    // 新 KEK + 新恢复封印 + 用新 KEK 重新包裹同一把 DEK。
    let salt_new = crate::crypto::generate_salt_v3();
    let kek_new = crate::crypto::derive_from_salt(new_password, &salt_new)?;
    let verifier_new = crate::vault::make_verifier(&dek)?;
    let wrapped_new = crate::vault::wrap_data_key(&dek, &kek_new)?;
    let (recovery_code_new, recovery_seal_new) = crate::recovery::create_recovery_seal(&dek)?;

    {
        let guard = get_conn()?;
        let conn = guard.as_ref().unwrap();
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| format!("开启事务失败: {}", e))?;
        let result = (|| -> Result<(), String> {
            // legacy 库：字段是 v1 格式（裸密钥），一次性升级到 v2（HKDF 域分离）。
            // 新库已经全部是 v2，这里不做任何逐字段操作 —— 改密是 O(1)。
            if migrated_from_legacy {
                crate::db::reencrypt_all_to_v2(conn, &dek)?;
            }
            conn.execute(
                "INSERT INTO security_config (id, kdf_salt, verifier, recovery_seal, wrapped_dek, created_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET kdf_salt=?1, verifier=?2, recovery_seal=?3, wrapped_dek=?4",
                params![
                    salt_new,
                    verifier_new,
                    recovery_seal_new,
                    wrapped_new,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                ],
            )
            .map_err(|e| format!("更新安全配置失败: {}", e))?;
            // DEK 未变 → 已发出的继承人份额继续有效，不得作废元数据。
            Ok(())
        })();
        match result {
            Ok(()) => conn
                .execute_batch("COMMIT")
                .map_err(|e| format!("提交事务失败: {}", e))?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }

    crate::vault::set_key(dek);
    Ok(recovery_code_new)
}

/// 把全库 v1 字段一次性升级为 v2（HKDF 域分离）。仅 legacy 库改密时调用一次。
fn reencrypt_all_to_v2(conn: &Connection, dek: &[u8; 32]) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, account_identifier, account_description, heir_name, heir_contact, notes, credential_enc FROM digital_assets",
        )
        .map_err(|e| format!("查询资产失败: {}", e))?;
    let rows: Vec<(i64, String, String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("读取资产失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集资产失败: {}", e))?;
    let upgrade = |v: &str| -> Result<String, String> {
        if v.is_empty() {
            return Ok(String::new());
        }
        let plain = crate::vault::decrypt_field_any(v, dek)?;
        crate::vault::encrypt_field_v2(&plain, dek)
    };
    for (id, ai, ad, hn, hc, nt, cr) in rows {
        conn.execute(
            "UPDATE digital_assets SET account_identifier=?1, account_description=?2, heir_name=?3, heir_contact=?4, notes=?5, credential_enc=?6 WHERE id=?7",
            params![upgrade(&ai)?, upgrade(&ad)?, upgrade(&hn)?, upgrade(&hc)?, upgrade(&nt)?, upgrade(&cr)?, id],
        )
        .map_err(|e| format!("升级资产格式失败: {}", e))?;
    }

    if let Some((pid, vals)) = select_profile_sensitive(conn)? {
        let mut new_vals = Vec::with_capacity(vals.len());
        for v in &vals {
            new_vals.push(upgrade(v)?);
        }
        update_profile_sensitive(conn, pid, &new_vals)?;
    }

    let heirs: Vec<(i64, String, String, String, String, String)> = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,'') FROM heirs",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集继承人失败: {}", e))?;
    for (id, name, relation, phone, email, share_note) in heirs {
        conn.execute(
            "UPDATE heirs SET name=?1, relation=?2, phone=?3, email=?4, share_note=?5 WHERE id=?6",
            params![
                upgrade(&name)?,
                upgrade(&relation)?,
                upgrade(&phone)?,
                upgrade(&email)?,
                upgrade(&share_note)?,
                id
            ],
        )
        .map_err(|e| format!("升级继承人格式失败: {}", e))?;
    }
    Ok(())
}

/// 恢复流程落库：重写安全配置（验证器+恢复封印）并把全库字段重加密到主密钥对应的新密码密钥
pub fn reset_security_config_db(
    kdf_salt: &str,
    verifier: &str,
    recovery_seal: &str,
    data_key: &[u8; 32],
    kek: &[u8; 32],
) -> Result<(), String> {
    // 恢复重设沿用同一把数据密钥：只需要用新 KEK 重新包裹它。
    // 旧实现会逐字段重加密全库，资产多时是一次可感知的卡顿，且中途失败会留下
    // 「部分字段用新钥、部分用旧钥」的半完成状态。现在整件事是一条 UPDATE。
    let wrapped = crate::vault::wrap_data_key(data_key, kek)?;
    {
        let guard = get_conn()?;
        let conn = guard.as_ref().unwrap();
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| format!("开启事务失败: {}", e))?;
        let result = conn.execute(
            "UPDATE security_config SET kdf_salt=?1, verifier=?2, recovery_seal=?3, wrapped_dek=?4 WHERE id=1",
            params![kdf_salt, verifier, recovery_seal, wrapped],
        );
        match result {
            Ok(_) => conn
                .execute_batch("COMMIT")
                .map_err(|e| format!("提交事务失败: {}", e))?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(format!("更新安全配置失败: {}", e));
            }
        }
    }
    crate::vault::set_key(zeroize::Zeroizing::new(*data_key));
    Ok(())
}

const HEARTBEAT_BINDING_FIELDS: &[&str] = &["enabled", "token", "endpoint"];
const SMTP_BINDING_FIELDS: &[&str] = &[
    "host",
    "port",
    "from_addr",
    "username",
    "password_enc",
    "allow_insecure",
];
const ASSISTANT_BINDING_FIELDS: &[&str] = &["enabled", "endpoint", "model", "api_key_enc"];

fn canonical_json(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let mut output = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key)
                        .map_err(|e| format!("序列化绑定字段失败: {}", e))?,
                );
                output.push(':');
                output.push_str(&canonical_json(&object[*key])?);
            }
            output.push('}');
            Ok(output)
        }
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", values.join(",")))
        }
        _ => serde_json::to_string(value).map_err(|e| format!("序列化绑定值失败: {}", e)),
    }
}

fn binding_object(tables: &serde_json::Value, table: &str, fields: &[&str]) -> serde_json::Value {
    let Some(value) = tables.get(table) else {
        return serde_json::Value::Null;
    };
    let Some(object) = value.as_object() else {
        return serde_json::Value::Null;
    };
    let mut selected = serde_json::Map::new();
    for field in fields {
        selected.insert(
            (*field).to_string(),
            object
                .get(*field)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(selected)
}

fn external_config_binding_payload(tables: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "heartbeat_config": binding_object(tables, "heartbeat_config", HEARTBEAT_BINDING_FIELDS),
        "smtp_config": binding_object(tables, "smtp_config", SMTP_BINDING_FIELDS),
        "assistant_config": binding_object(tables, "assistant_config", ASSISTANT_BINDING_FIELDS),
    })
}

fn make_external_config_binding(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<String, String> {
    let payload = external_config_binding_payload(tables);
    let canonical = canonical_json(&payload)?;
    crate::vault::encrypt_field_v2(&canonical, dek)
}

fn validate_external_config_binding(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<bool, String> {
    let Some(security) = tables.get("security_config") else {
        return Ok(false);
    };
    let Some(object) = security.as_object() else {
        return Ok(false);
    };
    let Some(raw) = object.get("external_config_binding") else {
        return Ok(false);
    };
    let binding = raw
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("备份 external_config_binding 必须是非空密文")?;
    let decrypted = crate::vault::decrypt_field_any(binding, dek)
        .map_err(|_| "备份外连配置完整性认证失败".to_string())?;
    let expected = canonical_json(&external_config_binding_payload(tables))?;
    if decrypted != expected {
        return Err("备份外连配置完整性认证失败".to_string());
    }
    Ok(true)
}

fn clear_external_delivery_config(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE heartbeat_config SET enabled=0, token='', endpoint='' WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空心跳配置失败: {}", e))?;
    conn.execute(
        "UPDATE smtp_config SET host='', port=587, from_addr='', username='', password_enc='', allow_insecure=0 WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空 SMTP 配置失败: {}", e))?;
    conn.execute(
        "UPDATE assistant_config SET enabled=0, endpoint='', model='', api_key_enc='' WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空助手配置失败: {}", e))?;
    Ok(())
}

fn validate_external_delivery_config(tables: &serde_json::Value) -> Result<(), String> {
    if let Some(heartbeat) = tables.get("heartbeat_config").filter(|v| v.is_object()) {
        if let Some(endpoint) = heartbeat
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
        {
            if !endpoint.trim().is_empty() {
                crate::heartbeat::validate_heartbeat_endpoint(endpoint)?;
            }
        }
    }
    if let Some(assistant) = tables.get("assistant_config").filter(|v| v.is_object()) {
        if let Some(endpoint) = assistant
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
        {
            if !endpoint.trim().is_empty() {
                crate::vault::validate_external_url(endpoint)?;
            }
        }
    }
    if let Some(smtp) = tables.get("smtp_config").filter(|v| v.is_object()) {
        if let Some(port) = smtp.get("port").and_then(serde_json::Value::as_i64) {
            if !(1..=65535).contains(&port) {
                return Err("备份 SMTP 端口必须在 1 到 65535 之间".to_string());
            }
        }
    }
    Ok(())
}

/// 导出备份 JSON：密文原样 + 盐/验证器，可在同密码下恢复到其他设备
pub fn export_backup_json_db() -> Result<String, String> {
    use serde_json::{json, Value};

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_readable(conn)?;

    let mut assets: Vec<Value> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, platform_id, account_identifier, account_description, category, estimated_value, value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc, is_confirmed, created_at, updated_at FROM digital_assets",
        ).map_err(|e| format!("查询资产失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "platform_id": row.get::<_, i64>(1)?,
                    "account_identifier": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    "account_description": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    "category": row.get::<_, String>(4)?,
                    "estimated_value": row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                    "value_currency": row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "CNY".to_string()),
                    "disposal_method": row.get::<_, String>(7)?,
                    "heir_name": row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    "heir_contact": row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    "notes": row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                    "credential_enc": row.get::<_, Option<String>>(11)?.unwrap_or_default(),
                    "is_confirmed": row.get::<_, i64>(12)?,
                    "created_at": row.get::<_, String>(13)?,
                    "updated_at": row.get::<_, String>(14)?,
                }))
            })
            .map_err(|e| format!("读取资产失败: {}", e))?;
        for r in rows {
            assets.push(r.map_err(|e| format!("资产解析失败: {}", e))?);
        }
    }

    let mut profiles: Vec<Value> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in, dms_enabled, created_at, updated_at FROM user_profile",
        ).map_err(|e| format!("查询档案失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "full_name": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    "phone": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    "email": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    "id_last4": row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    "emergency_contact_name": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    "emergency_contact_phone": row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    "emergency_contact_relation": row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    "emergency_contact_email": row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    "heir_primary_name": row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    "heir_primary_phone": row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                    "heir_primary_relation": row.get::<_, Option<String>>(11)?.unwrap_or_default(),
                    "heir_primary_email": row.get::<_, Option<String>>(12)?.unwrap_or_default(),
                    "check_in_interval_days": row.get::<_, i64>(13)?,
                    "last_check_in": row.get::<_, Option<String>>(14)?,
                    "dms_enabled": row.get::<_, i64>(15)?,
                    "created_at": row.get::<_, String>(16)?,
                    "updated_at": row.get::<_, String>(17)?,
                }))
            })
            .map_err(|e| format!("读取档案失败: {}", e))?;
        for r in rows {
            profiles.push(r.map_err(|e| format!("档案解析失败: {}", e))?);
        }
    }

    let mut check_ins: Vec<Value> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, checked_at, status, note FROM check_in_log ORDER BY id")
            .map_err(|e| format!("查询记录失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "checked_at": row.get::<_, String>(1)?,
                    "status": row.get::<_, String>(2)?,
                    "note": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                }))
            })
            .map_err(|e| format!("读取记录失败: {}", e))?;
        for r in rows {
            check_ins.push(r.map_err(|e| format!("记录解析失败: {}", e))?);
        }
    }

    let dms: Option<Value> = conn
        .query_row(
            "SELECT id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in FROM dms_config WHERE id=1",
            [],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "enabled": row.get::<_, i64>(1)?,
                    "check_in_interval_days": row.get::<_, i64>(2)?,
                    "stage1_wait_days": row.get::<_, i64>(3)?,
                    "stage2_wait_days": row.get::<_, i64>(4)?,
                    // 保留字段仅为兼容旧备份格式；应用内已不使用（见 notify.rs 注释）
                    "stage3_wait_days": row.get::<_, i64>(5)?,
                    "last_check_in": row.get::<_, Option<String>>(6)?,
                }))
            },
        )
        .ok();

    let security: Option<Value> = conn
        .query_row(
            "SELECT kdf_salt, verifier, COALESCE(recovery_seal, ''), COALESCE(wrapped_dek, '') FROM security_config WHERE id = 1",
            [],
            |row| {
                Ok(json!({
                    "kdf_salt": row.get::<_, String>(0)?,
                    "verifier": row.get::<_, String>(1)?,
                    // v3：带上恢复封印。旧版备份不含封印，换机恢复后即使记得恢复码
                    // 也无法再解出主密钥——恢复链路在「换电脑」这一最常见场景下断掉。
                    "recovery_seal": row.get::<_, String>(2)?,
                    // 两层密钥：换机后靠 wrapped_dek 才能解出数据密钥。
                    // 它本身是 KEK 加密的密文，随备份走不降低安全性。
                    "wrapped_dek": row.get::<_, String>(3)?,
                }))
            },
        )
        .ok()
        .map(|mut value| {
            if let Some(object) = value.as_object_mut() {
                if object
                    .get("recovery_seal")
                    .and_then(Value::as_str)
                    .is_some_and(str::is_empty)
                {
                    object.remove("recovery_seal");
                }
                if object
                    .get("wrapped_dek")
                    .and_then(Value::as_str)
                    .is_some_and(str::is_empty)
                {
                    object.remove("wrapped_dek");
                }
            }
            value
        });

    // 心跳配置与政策补丁也必须进备份：
    // 旧版只备份 5 张表，换机恢复后用户会发现「心跳令牌没了、自己导入的政策补丁也没了」，
    // 而这两项都是用户手工配置、无法从内置库重建的数据。
    let heartbeat: Option<Value> = conn
        .query_row(
            "SELECT enabled, COALESCE(token,''), COALESCE(endpoint,'') FROM heartbeat_config WHERE id=1",
            [],
            |row| {
                Ok(json!({
                    "enabled": row.get::<_, i64>(0)?,
                    "token": row.get::<_, String>(1)?,
                    "endpoint": row.get::<_, String>(2)?,
                }))
            },
        )
        .ok();

    let mut platform_overrides: Vec<Value> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes FROM platform_overrides ORDER BY id")
            .map_err(|e| format!("查询政策补丁失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "name_en": row.get::<_, String>(2)?,
                    "category": row.get::<_, String>(3)?,
                    "subtype": row.get::<_, String>(4)?,
                    "has_property": row.get::<_, i64>(5)? != 0,
                    "has_personality": row.get::<_, i64>(6)? != 0,
                    "inheritable": row.get::<_, i64>(7)? != 0,
                    "required_documents": row.get::<_, String>(8)?,
                    "process_description": row.get::<_, String>(9)?,
                    "estimated_difficulty": row.get::<_, String>(10)?,
                    "official_policy_url": row.get::<_, String>(11)?,
                    "estimated_time_days": row.get::<_, String>(12)?,
                    "notes": row.get::<_, String>(13)?,
                }))
            })
            .map_err(|e| format!("读取政策补丁失败: {}", e))?;
        for r in rows {
            platform_overrides.push(r.map_err(|e| format!("政策补丁解析失败: {}", e))?);
        }
    }

    // v4：新增继承人名单、SMTP、保管位置、助手配置。
    // 都是「换机后必须带走」的配置，否则用户在新机器上要重新配一遍。
    let heirs = export_heirs_json(conn)?;
    // 审核留痕也是「换机后必须带走」的证据：只留本机就等于换台电脑审核作废。
    let review_records = export_review_records_json(conn)?;
    let smtp = export_smtp_json(conn)?;
    let custody = export_custody_json(conn)?;
    let assistant = export_assistant_json(conn)?;
    let recovery = crate::heir_recovery::export_meta_json(conn)?;
    let backup_version = security
        .as_ref()
        .and_then(|value| value.get("wrapped_dek"))
        .and_then(Value::as_str)
        .filter(|wrapped| !wrapped.is_empty())
        .map(|_| 5)
        .unwrap_or_else(|| if security.is_some() { 4 } else { 5 });

    let mut backup = json!({
        "app": "digilegacy",
        "version": backup_version,
        "created_at": Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "tables": {
            "digital_assets": assets,
            "user_profile": profiles,
            "heirs": heirs,
            "review_records": review_records,
            "check_in_log": check_ins,
            "dms_config": dms,
            "security_config": security,
            "heartbeat_config": heartbeat,
            "platform_overrides": platform_overrides,
            "smtp_config": smtp,
            "custody_config": custody,
            "assistant_config": assistant,
            "heir_recovery": recovery,
        }
    });
    if backup["tables"]["security_config"].is_object() {
        let dek = crate::vault::get_key().ok_or("当前保险库密钥不可用，拒绝导出")?;
        validate_encrypted_backup_fields(&backup["tables"], &dek)?;
        let binding = make_external_config_binding(&backup["tables"], &dek)?;
        backup["tables"]["security_config"]["external_config_binding"] = Value::String(binding);
    }

    serde_json::to_string_pretty(&backup).map_err(|e| format!("生成备份失败: {}", e))
}

/// 导出辅助：继承人名单（含密文原样导出，导入方用同一主密码即可解密）。
fn export_heirs_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,''), is_executor, sort_order FROM heirs ORDER BY sort_order, id",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "relation": r.get::<_, String>(2)?,
                "phone": r.get::<_, String>(3)?,
                "email": r.get::<_, String>(4)?,
                "share_note": r.get::<_, String>(5)?,
                "is_executor": r.get::<_, i64>(6)?,
                "sort_order": r.get::<_, i64>(7)?,
            }))
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?;
    for r in rows {
        out.push(r.map_err(|e| format!("继承人解析失败: {}", e))?);
    }
    Ok(serde_json::Value::Array(out))
}

/// 导出审核留痕（含 AI 结论与人工结论）。备份是证据链的一部分，不能只留本机。
fn export_review_records_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT profile_id, item_key, fingerprint, ai_verdict, ai_note,
                    human_verdict, human_note, decided_by, created_at
             FROM review_records ORDER BY id",
        )
        .map_err(|e| format!("查询审核记录失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "profile_id": r.get::<_, String>(0)?,
                "item_key": r.get::<_, String>(1)?,
                "fingerprint": r.get::<_, String>(2)?,
                "ai_verdict": r.get::<_, String>(3)?,
                "ai_note": r.get::<_, String>(4)?,
                "human_verdict": r.get::<_, Option<String>>(5)?,
                "human_note": r.get::<_, String>(6)?,
                "decided_by": r.get::<_, String>(7)?,
                "created_at": r.get::<_, String>(8)?,
            }))
        })
        .map_err(|e| format!("读取审核记录失败: {}", e))?;
    for r in rows {
        out.push(r.map_err(|e| format!("审核记录解析失败: {}", e))?);
    }
    Ok(serde_json::Value::Array(out))
}

fn export_smtp_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT COALESCE(host,''), port, COALESCE(from_addr,''), COALESCE(username,''), COALESCE(password_enc,''), allow_insecure FROM smtp_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "host": r.get::<_, String>(0)?,
                "port": r.get::<_, i64>(1)?,
                "from_addr": r.get::<_, String>(2)?,
                "username": r.get::<_, String>(3)?,
                "password_enc": r.get::<_, String>(4)?,
                "allow_insecure": r.get::<_, i64>(5)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取 SMTP 配置失败: {}", other)),
    })
}

fn export_custody_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT COALESCE(backup_location_enc,''), COALESCE(recovery_code_location_enc,'') FROM custody_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "backup_location_enc": r.get::<_, String>(0)?,
                "recovery_code_location_enc": r.get::<_, String>(1)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取保管位置失败: {}", other)),
    })
}

fn export_assistant_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT enabled, COALESCE(endpoint,''), COALESCE(model,''), COALESCE(api_key_enc,'') FROM assistant_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "enabled": r.get::<_, i64>(0)?,
                "endpoint": r.get::<_, String>(1)?,
                "model": r.get::<_, String>(2)?,
                "api_key_enc": r.get::<_, String>(3)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取助手配置失败: {}", other)),
    })
}

/// 备份导入模式。
///
/// 覆盖是历史行为，会先清空资产/档案/签到记录；导入前没有快照时，
/// 一次误操作就永久丢数据。默认改为合并，覆盖必须显式选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// 保留现有数据，只追加备份里的资产（id 冲突时分配新 id）。
    Merge,
    /// 清空资产/档案/签到后写入备份内容（原行为）。
    Replace,
}

impl ImportMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "replace" => Self::Replace,
            _ => Self::Merge,
        }
    }
}

fn validate_backup_row_shapes(tables: &serde_json::Value) -> Result<(), String> {
    const ARRAY_TABLES: &[&str] = &[
        "digital_assets",
        "user_profile",
        "heirs",
        "review_records",
        "check_in_log",
        "platform_overrides",
    ];
    for table in ARRAY_TABLES {
        if let Some(value) = tables.get(*table) {
            if !value.is_array() && !value.is_null() {
                return Err(format!("备份字段 {} 必须是数组", table));
            }
            if let Some(rows) = value.as_array() {
                if rows.iter().any(|row| !row.is_object()) {
                    return Err(format!("备份字段 {} 的行必须是对象", table));
                }
            }
        }
    }
    for table in [
        "dms_config",
        "heartbeat_config",
        "smtp_config",
        "custody_config",
        "assistant_config",
        "heir_recovery",
    ] {
        if let Some(value) = tables.get(table) {
            if !value.is_object() && !value.is_null() {
                return Err(format!("备份字段 {} 必须是对象或 null", table));
            }
        }
    }
    Ok(())
}

fn validate_encrypted_backup_fields(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<(), String> {
    const FIELDS: &[(&str, &str)] = &[
        ("digital_assets", "account_identifier"),
        ("digital_assets", "account_description"),
        ("digital_assets", "heir_name"),
        ("digital_assets", "heir_contact"),
        ("digital_assets", "notes"),
        ("digital_assets", "credential_enc"),
        ("user_profile", "full_name"),
        ("user_profile", "phone"),
        ("user_profile", "email"),
        ("user_profile", "id_last4"),
        ("user_profile", "emergency_contact_name"),
        ("user_profile", "emergency_contact_phone"),
        ("user_profile", "emergency_contact_relation"),
        ("user_profile", "emergency_contact_email"),
        ("user_profile", "heir_primary_name"),
        ("user_profile", "heir_primary_phone"),
        ("user_profile", "heir_primary_relation"),
        ("user_profile", "heir_primary_email"),
        ("heirs", "name"),
        ("heirs", "relation"),
        ("heirs", "phone"),
        ("heirs", "email"),
        ("heirs", "share_note"),
        ("smtp_config", "password_enc"),
        ("custody_config", "backup_location_enc"),
        ("custody_config", "recovery_code_location_enc"),
        ("assistant_config", "api_key_enc"),
        ("heartbeat_config", "token"),
        ("security_config", "external_config_binding"),
    ];

    for &(table, field) in FIELDS {
        let Some(table_value) = tables.get(table) else {
            continue;
        };
        let rows: &[serde_json::Value] = match table_value {
            serde_json::Value::Array(rows) => rows.as_slice(),
            serde_json::Value::Object(_) => std::slice::from_ref(table_value),
            serde_json::Value::Null => continue,
            _ => return Err(format!("备份字段 {} 结构无效", table)),
        };
        for (index, row) in rows.iter().enumerate() {
            let Some(value) = row.get(field) else {
                continue;
            };
            let Some(raw) = value.as_str() else {
                if value.is_null() {
                    continue;
                }
                return Err(format!(
                    "备份字段 {}[{}].{} 必须是加密字符串",
                    table, index, field
                ));
            };
            if raw.is_empty() {
                continue;
            }
            if !crate::vault::is_encrypted(raw) {
                return Err(format!(
                    "备份字段 {}[{}].{} 必须是带 enc 前缀的密文",
                    table, index, field
                ));
            }
            crate::vault::decrypt_field_any(raw, dek)
                .map_err(|_| format!("备份字段 {}[{}].{} 密文认证失败", table, index, field))?;
        }
    }
    Ok(())
}

fn reject_unauthenticated_ciphertext_fields(tables: &serde_json::Value) -> Result<(), String> {
    const FIELDS: &[(&str, &str)] = &[
        ("digital_assets", "account_identifier"),
        ("digital_assets", "account_description"),
        ("digital_assets", "heir_name"),
        ("digital_assets", "heir_contact"),
        ("digital_assets", "notes"),
        ("digital_assets", "credential_enc"),
        ("user_profile", "full_name"),
        ("user_profile", "phone"),
        ("user_profile", "email"),
        ("user_profile", "id_last4"),
        ("user_profile", "emergency_contact_name"),
        ("user_profile", "emergency_contact_phone"),
        ("user_profile", "emergency_contact_relation"),
        ("user_profile", "emergency_contact_email"),
        ("user_profile", "heir_primary_name"),
        ("user_profile", "heir_primary_phone"),
        ("user_profile", "heir_primary_relation"),
        ("user_profile", "heir_primary_email"),
        ("heirs", "name"),
        ("heirs", "relation"),
        ("heirs", "phone"),
        ("heirs", "email"),
        ("heirs", "share_note"),
        ("smtp_config", "password_enc"),
        ("custody_config", "backup_location_enc"),
        ("custody_config", "recovery_code_location_enc"),
        ("assistant_config", "api_key_enc"),
        ("heartbeat_config", "token"),
        ("security_config", "external_config_binding"),
    ];
    for &(table, field) in FIELDS {
        let Some(table_value) = tables.get(table) else {
            continue;
        };
        let rows: &[serde_json::Value] = match table_value {
            serde_json::Value::Array(rows) => rows.as_slice(),
            serde_json::Value::Object(_) => std::slice::from_ref(table_value),
            serde_json::Value::Null => continue,
            _ => return Err(format!("备份字段 {} 结构无效", table)),
        };
        for (index, row) in rows.iter().enumerate() {
            let Some(raw) = row.get(field).and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !raw.is_empty() && crate::vault::is_encrypted(raw) {
                return Err(format!(
                    "无 security_config 的备份字段 {}[{}].{} 不得带未认证密文",
                    table, index, field
                ));
            }
        }
    }
    Ok(())
}

fn authenticate_backup_security(
    version: i64,
    tables: &serde_json::Value,
    source_password: Option<&str>,
    allow_current_key: bool,
    allow_empty_recovery_seal: bool,
) -> Result<Option<zeroize::Zeroizing<[u8; 32]>>, String> {
    let Some(security) = tables.get("security_config") else {
        return Ok(None);
    };
    let Some(sec) = security.as_object() else {
        if security.is_null() {
            return Ok(None);
        }
        return Err("备份 security_config 必须是对象或 null".to_string());
    };
    let password = source_password.filter(|password| !password.is_empty());
    let salt = sec
        .get("kdf_salt")
        .and_then(serde_json::Value::as_str)
        .filter(|salt| !salt.trim().is_empty())
        .ok_or("备份 security_config 缺少有效的 kdf_salt")?;
    crate::crypto::parse_kdf_params(salt)
        .map_err(|_| "备份 security_config 的 kdf_salt 无效".to_string())?;
    let derived_kek = match password {
        Some(password) => Some(crate::crypto::derive_from_salt(password, salt)?),
        None => None,
    };
    let verifier = sec
        .get("verifier")
        .and_then(serde_json::Value::as_str)
        .filter(|verifier| !verifier.is_empty())
        .ok_or("备份 security_config 缺少有效的 verifier")?;
    if !crate::vault::is_encrypted(verifier) {
        return Err("备份 security_config 的 verifier 必须是密文".to_string());
    }
    let fallback_key = || {
        if allow_current_key {
            crate::vault::get_key()
        } else {
            None
        }
    };
    let dek = if allow_current_key && derived_kek.is_none() {
        fallback_key().ok_or("带 security_config 的备份必须提供源主密码")?
    } else {
        match sec.get("wrapped_dek") {
            Some(value) => {
                let wrapped = value
                    .as_str()
                    .ok_or("备份 security_config 的 wrapped_dek 必须是字符串")?;
                if wrapped.is_empty() {
                    return Err("备份 security_config 的 wrapped_dek 不能为空".to_string());
                }
                let kek = derived_kek
                    .clone()
                    .or_else(fallback_key)
                    .ok_or("带 security_config 的备份必须提供源主密码")?;
                crate::vault::unwrap_data_key(wrapped, &kek)
                    .map_err(|_| "备份 security_config 的 wrapped_dek 解包失败".to_string())?
            }
            None if version < 5 => derived_kek
                .or_else(fallback_key)
                .ok_or("带 security_config 的备份必须提供源主密码")?,
            None => return Err("备份 security_config 缺少 wrapped_dek".to_string()),
        }
    };
    if !crate::vault::verify_key(verifier, &dek) {
        return Err("备份 security_config 的 verifier 认证失败".to_string());
    }
    if let Some(seal) = sec.get("recovery_seal") {
        let seal = seal
            .as_str()
            .ok_or("备份 security_config 的 recovery_seal 必须是字符串")?;
        if seal.is_empty() && !allow_empty_recovery_seal {
            return Err("备份 security_config 的 recovery_seal 不能为空".to_string());
        }
        if !seal.is_empty() {
            crate::recovery::validate_recovery_seal_structure(seal)
                .map_err(|_| "备份 security_config 的 recovery_seal 无效".to_string())?;
        }
    }
    validate_encrypted_backup_fields(tables, &dek)?;
    let _ = version;
    Ok(Some(dek))
}

/// 导入备份：覆盖模式（历史行为，保留给既有调用方与回归测试）。
///
/// 新代码应使用 `import_backup_json_db_with_mode`，由命令层按用户选择决定模式。
/// 生产路径已全部切到带模式的版本，这里只被测试与场景矩阵调用。
#[cfg(test)]
#[allow(dead_code)]
pub fn import_backup_json_db(content: &str) -> Result<i64, String> {
    import_backup_json_db_with_mode(content, ImportMode::Replace)
}

/// 导入备份：按 `mode` 合并或覆盖用户数据与安全配置；导入后强制重新解锁。
///
/// 含 `security_config` 的生产导入必须通过带密码的 helper。测试保留旧 wrapper，
/// 仅用于既有 legacy round-trip 场景，不由 Tauri command 调用。
#[cfg(test)]
pub fn import_backup_json_db_with_mode(content: &str, mode: ImportMode) -> Result<i64, String> {
    import_backup_json_db_internal(content, mode, None, cfg!(test), cfg!(test))
}

pub fn import_backup_json_db_with_mode_and_password(
    content: &str,
    mode: ImportMode,
    source_password: Option<&str>,
) -> Result<i64, String> {
    import_backup_json_db_internal(content, mode, source_password, false, false)
}

fn import_backup_json_db_internal(
    content: &str,
    mode: ImportMode,
    source_password: Option<&str>,
    allow_current_key: bool,
    allow_empty_recovery_seal: bool,
) -> Result<i64, String> {
    use serde_json::Value;

    let v: Value = serde_json::from_str(content).map_err(|e| format!("备份文件解析失败: {}", e))?;
    if v.get("app").and_then(|x| x.as_str()) != Some("digilegacy") {
        return Err("不是有效的归零备份文件".to_string());
    }
    // v1 备份仍然可导入（向后兼容）：新表缺失时保持本地现状，
    // 旧备份里没有的 heartbeat/platform_overrides 按「未提供」处理。
    let version = v.get("version").and_then(|x| x.as_i64()).unwrap_or(0);
    if !(1..=5).contains(&version) {
        return Err("备份版本不受支持".to_string());
    }
    let tables = v.get("tables").ok_or("备份缺少数据表")?;
    validate_backup_row_shapes(tables)?;

    let backup_dek = authenticate_backup_security(
        version,
        tables,
        source_password,
        allow_current_key,
        allow_empty_recovery_seal,
    )?;
    if backup_dek.is_none() {
        reject_unauthenticated_ciphertext_fields(tables)?;
    }
    let backup_has_security = backup_dek.is_some();
    let external_config_bound = match backup_dek.as_ref() {
        Some(dek) => validate_external_config_binding(tables, dek)?,
        None => false,
    };
    validate_external_delivery_config(tables)?;
    let local_security = get_security_config_db()?;
    let local_has_vault = local_security.is_some();
    if local_has_vault {
        ensure_vault_readable_db()?;
    }

    let mut import_security_config = backup_has_security;
    if mode == ImportMode::Merge && backup_has_security {
        let Some(local_security) = local_security.as_ref() else {
            return Err(
                "含 security_config 的备份不能合并到未配置保险库，请改用覆盖导入".to_string(),
            );
        };
        let local_key = crate::vault::get_key().ok_or("当前保险库未解锁")?;
        let backup_key = backup_dek.as_ref().ok_or("备份数据密钥缺失")?;
        if *local_key != **backup_key {
            return Err("合并导入的数据密钥与当前库不同，已拒绝修改安全配置".to_string());
        }
        let security = tables
            .get("security_config")
            .and_then(|value| value.as_object())
            .ok_or("备份 security_config 必须是对象")?;
        let backup_salt = security
            .get("kdf_salt")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let backup_verifier = security
            .get("verifier")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if backup_salt != local_security.0 || backup_verifier != local_security.1 {
            return Err(
                "合并导入的 security_config 与当前库不同，已拒绝以保护本地密钥".to_string(),
            );
        }
        import_security_config = false;
    }
    let encrypt_plaintext_on_import = local_has_vault && !backup_has_security;
    if encrypt_plaintext_on_import && !crate::vault::is_unlocked() {
        return Err(
            "该备份不含加密配置，而当前库已加密且处于锁定状态。请先解锁再导入，否则明文数据会绕过加密"
                .to_string(),
        );
    }
    let protect = |value: String| -> Result<String, String> {
        if !encrypt_plaintext_on_import || value.is_empty() {
            return Ok(value);
        }
        crate::vault::encrypt_if_needed(&value)
    };

    let sval = |val: &Value, k: &str| -> String {
        val.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    let ival = |val: &Value, k: &str| -> i64 { val.get(k).and_then(|x| x.as_i64()).unwrap_or(0) };
    let fval = |val: &Value, k: &str| -> f64 { val.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) };
    let oval = |val: &Value, k: &str| -> Option<String> {
        val.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
    };

    let assets = tables
        .get("digital_assets")
        .and_then(|x| x.as_array())
        .ok_or("备份缺少资产数据")?;
    // 缺失 = 旧格式，允许为空；**存在但不是数组 = 损坏，必须拒绝**。
    // 旧写法用 `and_then(as_array).unwrap_or_default()`，一个手改过的
    // `"user_profile": "x"` 会被静默当成空数组，导入"成功"却把档案清空了。
    let array_or_missing = |key: &str| -> Result<Vec<Value>, String> {
        match tables.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => Ok(items.clone()),
            Some(_) => Err(format!("备份字段 {} 必须是数组", key)),
        }
    };
    let profiles = array_or_missing("user_profile")?;
    let check_ins = array_or_missing("check_in_log")?;

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();

    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("开启事务失败: {}", e))?;
    let result = (|| -> Result<i64, String> {
        if mode == ImportMode::Replace {
            conn.execute("DELETE FROM digital_assets", [])
                .map_err(|e| format!("清空资产失败: {}", e))?;
            conn.execute("DELETE FROM user_profile", [])
                .map_err(|e| format!("清空档案失败: {}", e))?;
            conn.execute("DELETE FROM check_in_log", [])
                .map_err(|e| format!("清空记录失败: {}", e))?;
        }

        let mut count = 0i64;
        for (idx, a) in assets.iter().enumerate() {
            // 备份文件是外部输入：类别/处置/金额必须与命令层同样校验，
            // 否则一个手改过的 JSON 就能把非法枚举和负金额写进统计与报告。
            let category = sval(a, "category");
            validate_category(&category)
                .map_err(|e| format!("备份第 {} 条资产类别非法: {}", idx + 1, e))?;
            let disposal = sval(a, "disposal_method");
            validate_disposal(&disposal)
                .map_err(|e| format!("备份第 {} 条资产处置非法: {}", idx + 1, e))?;
            let value = fval(a, "estimated_value");
            if !value.is_finite() || value < 0.0 || value > 1e12 {
                return Err(format!(
                    "备份第 {} 条资产估值非法（要求 0..1e12 的有限数值，实际 {}）",
                    idx + 1,
                    value
                ));
            }
            // 合并模式不能复用备份里的 id：本地可能已有同 id 资产，
            // 直接 INSERT 会撞主键并把整个导入回滚。让 SQLite 分配新 id。
            let backup_id = ival(a, "id");
            let target_id = if mode == ImportMode::Replace {
                backup_id
            } else {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM digital_assets WHERE id=?1",
                        params![backup_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken == 0 && backup_id > 0 {
                    backup_id
                } else {
                    0
                }
            };
            conn.execute(
                "INSERT INTO digital_assets (id, platform_id, account_identifier, account_description, category, estimated_value, value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc, is_confirmed, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    if target_id > 0 { Some(target_id) } else { None },
                    ival(a, "platform_id"),
                    protect(sval(a, "account_identifier"))?,
                    protect(sval(a, "account_description"))?,
                    category,
                    value,
                    { let c = sval(a, "value_currency"); if c.is_empty() { "CNY".to_string() } else { c } },
                    disposal,
                    protect(sval(a, "heir_name"))?,
                    protect(sval(a, "heir_contact"))?,
                    protect(sval(a, "notes"))?,
                    protect(sval(a, "credential_enc"))?,
                    ival(a, "is_confirmed"),
                    sval(a, "created_at"),
                    sval(a, "updated_at"),
                ],
            ).map_err(|e| format!("写入资产失败: {}", e))?;
            count += 1;
        }

        // 合并模式不清空档案/签到：只补「本地还没有」的档案行，
        // 已有档案保持现状（否则一次合并导入会把当前联系人信息换掉）。
        let profile_id = |p: &Value| ival(p, "id");
        for p in &profiles {
            let pid = profile_id(p);
            if mode == ImportMode::Merge {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM user_profile WHERE id=?1",
                        params![pid],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken > 0 {
                    continue;
                }
            }
            conn.execute(
                "INSERT INTO user_profile (id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in, dms_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    pid,
                    protect(sval(p, "full_name"))?,
                    protect(sval(p, "phone"))?,
                    protect(sval(p, "email"))?,
                    protect(sval(p, "id_last4"))?,
                    protect(sval(p, "emergency_contact_name"))?,
                    protect(sval(p, "emergency_contact_phone"))?,
                    protect(sval(p, "emergency_contact_relation"))?,
                    protect(sval(p, "emergency_contact_email"))?,
                    protect(sval(p, "heir_primary_name"))?,
                    protect(sval(p, "heir_primary_phone"))?,
                    protect(sval(p, "heir_primary_relation"))?,
                    protect(sval(p, "heir_primary_email"))?,
                    ival(p, "check_in_interval_days"),
                    oval(p, "last_check_in"),
                    ival(p, "dms_enabled"),
                    sval(p, "created_at"),
                    sval(p, "updated_at"),
                ],
            ).map_err(|e| format!("写入档案失败: {}", e))?;
        }

        // 合并模式只追加备份里比本地更新的确认记录（id 冲突跳过），
        // 覆盖模式沿用原行为整表写入。
        for c in &check_ins {
            let cid = ival(c, "id");
            if mode == ImportMode::Merge {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM check_in_log WHERE id=?1",
                        params![cid],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken > 0 {
                    continue;
                }
            }
            conn.execute(
                "INSERT INTO check_in_log (id, checked_at, status, note) VALUES (?1, ?2, ?3, ?4)",
                params![
                    cid,
                    sval(c, "checked_at"),
                    sval(c, "status"),
                    sval(c, "note")
                ],
            )
            .map_err(|e| format!("写入记录失败: {}", e))?;
        }

        if let Some(d) = tables.get("dms_config").filter(|x| x.is_object()) {
            conn.execute(
                "UPDATE dms_config SET enabled=?1, check_in_interval_days=?2, stage1_wait_days=?3, stage2_wait_days=?4, stage3_wait_days=?5, last_check_in=?6 WHERE id=1",
                params![
                    ival(d, "enabled"),
                    ival(d, "check_in_interval_days"),
                    ival(d, "stage1_wait_days"),
                    ival(d, "stage2_wait_days"),
                    // 接收旧备份里的该值但不再使用，保证导入不因缺字段失败
                    ival(d, "stage3_wait_days"),
                    oval(d, "last_check_in"),
                ],
            ).map_err(|e| format!("写入DMS配置失败: {}", e))?;
        }

        if import_security_config {
            if let Some(sec) = tables.get("security_config").filter(|x| x.is_object()) {
                // v3 备份带恢复封印：与新主密钥（备份里的 kdf_salt/verifier）成套，
                // 一起恢复才能保留「记得恢复码就能找回」的能力。
                // v1/v2 备份不含封印：必须显式清空，否则本地旧封印指向旧设备的主密钥，
                // 会让恢复码解出错误密钥，恢复链路静默断裂。
                let seal = sval(sec, "recovery_seal");
                conn.execute(
                    "INSERT INTO security_config (id, kdf_salt, verifier, created_at, recovery_seal, wrapped_dek) VALUES (1, ?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET kdf_salt=?1, verifier=?2, recovery_seal=?4, wrapped_dek=?5",
                    params![
                        sval(sec, "kdf_salt"),
                        sval(sec, "verifier"),
                        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        seal,
                        sval(sec, "wrapped_dek"),
                    ],
                ).map_err(|e| format!("写入安全配置失败: {}", e))?;
            }
        }

        // 只有通过 DEK 认证绑定的备份才允许恢复外连配置；旧备份在覆盖模式下清空，
        // 合并模式则保留本机配置并要求用户重新确认。未认证配置绝不能继续外送。
        if mode == ImportMode::Replace && !external_config_bound {
            clear_external_delivery_config(conn)?;
        }
        if external_config_bound {
            if let Some(hb) = tables.get("heartbeat_config").filter(|x| x.is_object()) {
                let endpoint = sval(hb, "endpoint");
                if !endpoint.trim().is_empty() {
                    crate::heartbeat::validate_heartbeat_endpoint(&endpoint)?;
                }
                conn.execute(
                "INSERT INTO heartbeat_config (id, enabled, token, endpoint) VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET enabled=?1, token=?2, endpoint=?3",
                params![
                    ival(hb, "enabled"),
                    protect(sval(hb, "token"))?,
                    sval(hb, "endpoint")
                ],
            )
                .map_err(|e| format!("写入心跳配置失败: {}", e))?;
            }
        }

        // v4：继承人名单。Merge 模式下只追加「本地还没有的」那几条
        // （按 name+relation 判重），Replace 模式整表替换。
        if let Some(heirs) = tables.get("heirs") {
            if !heirs.is_array() && !heirs.is_null() {
                return Err("备份字段 heirs 必须是数组".to_string());
            }
            if let Some(list) = heirs.as_array() {
                if mode == ImportMode::Replace {
                    conn.execute("DELETE FROM heirs", [])
                        .map_err(|e| format!("清空继承人失败: {}", e))?;
                }
                for (idx, h) in list.iter().enumerate() {
                    let name = protect(sval(h, "name"))?;
                    if name.trim().is_empty() {
                        continue;
                    }
                    if mode == ImportMode::Merge {
                        // 简单判重：同名同关系视为同一个人（密文比较不可用，
                        // 因此比较解密后的明文；锁定态下 protect 已拒绝明文备份）
                        let existing = get_heirs_db()?;
                        if existing.iter().any(|e| e.name == name) {
                            continue;
                        }
                    }
                    conn.execute(
                        "INSERT INTO heirs (name, relation, phone, email, share_note, is_executor, sort_order, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                        params![
                            name,
                            protect(sval(h, "relation"))?,
                            protect(sval(h, "phone"))?,
                            protect(sval(h, "email"))?,
                            protect(sval(h, "share_note"))?,
                            ival(h, "is_executor"),
                            idx as i64,
                            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        ],
                    )
                    .map_err(|e| format!("写入继承人失败: {}", e))?;
                }
            }
        }

        // v4：审核留痕。Merge 模式跳过已存在的同指纹记录，避免重复堆积。
        if let Some(records) = tables.get("review_records") {
            if !records.is_array() && !records.is_null() {
                return Err("备份字段 review_records 必须是数组".to_string());
            }
            if let Some(list) = records.as_array() {
                if mode == ImportMode::Replace {
                    conn.execute("DELETE FROM review_records", [])
                        .map_err(|e| format!("清空审核记录失败: {}", e))?;
                }
                for r in list {
                    let profile_id = sval(r, "profile_id");
                    let item_key = sval(r, "item_key");
                    let fingerprint = sval(r, "fingerprint");
                    if profile_id.is_empty() || item_key.is_empty() {
                        continue;
                    }
                    if mode == ImportMode::Merge {
                        // 同一条目同一指纹的记录视为已经存在（留痕只增，不重复）
                        let exists: i64 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM review_records
                                 WHERE profile_id=?1 AND item_key=?2 AND fingerprint=?3
                                   AND ai_verdict=?4 AND COALESCE(human_verdict,'')=?5",
                                params![
                                    profile_id,
                                    item_key,
                                    fingerprint,
                                    sval(r, "ai_verdict"),
                                    sval(r, "human_verdict"),
                                ],
                                |x| x.get(0),
                            )
                            .unwrap_or(0);
                        if exists > 0 {
                            continue;
                        }
                    }
                    conn.execute(
                        "INSERT INTO review_records (profile_id, item_key, fingerprint, ai_verdict, ai_note, human_verdict, human_note, decided_by, created_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        params![
                            profile_id,
                            item_key,
                            fingerprint,
                            sval(r, "ai_verdict"),
                            sval(r, "ai_note"),
                            { let h = sval(r, "human_verdict"); if h.is_empty() { None } else { Some(h) } },
                            sval(r, "human_note"),
                            sval(r, "decided_by"),
                            sval(r, "created_at"),
                        ],
                    )
                    .map_err(|e| format!("写入审核记录失败: {}", e))?;
                }
            }
        }

        // v4：SMTP / 保管位置 / 助手配置。旧备份没有这些块 → 保持本地现状。
        if external_config_bound {
            if let Some(smtp) = tables.get("smtp_config").filter(|x| x.is_object()) {
                conn.execute(
                "INSERT INTO smtp_config (id, host, port, from_addr, username, password_enc, allow_insecure, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET host=?1, port=?2, from_addr=?3, username=?4, password_enc=?5, allow_insecure=?6, updated_at=?7",
                params![
                    sval(smtp, "host"),
                    ival(smtp, "port"),
                    sval(smtp, "from_addr"),
                    sval(smtp, "username"),
                    protect(sval(smtp, "password_enc"))?,
                    ival(smtp, "allow_insecure"),
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
            )
            .map_err(|e| format!("写入 SMTP 配置失败: {}", e))?;
            }
        }

        if let Some(c) = tables.get("custody_config").filter(|x| x.is_object()) {
            conn.execute(
                "INSERT INTO custody_config (id, backup_location_enc, recovery_code_location_enc, updated_at)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET backup_location_enc=?1, recovery_code_location_enc=?2, updated_at=?3",
                params![
                    protect(sval(c, "backup_location_enc"))?,
                    protect(sval(c, "recovery_code_location_enc"))?,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
            )
            .map_err(|e| format!("写入保管位置失败: {}", e))?;
        }

        // 恢复包元数据随备份往返：否则换机后家人手里的份额永远配不上新库
        // （份额本身不落库，但门限/指纹/KDF 盐必须跟着走，否则无法判断批次与口令模式）。
        crate::heir_recovery::import_meta_json(conn, tables.get("heir_recovery"))?;

        if external_config_bound {
            if let Some(a) = tables.get("assistant_config").filter(|x| x.is_object()) {
                conn.execute(
                "INSERT INTO assistant_config (id, enabled, endpoint, model, api_key_enc, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET enabled=?1, endpoint=?2, model=?3, api_key_enc=?4, updated_at=?5",
                params![
                    ival(a, "enabled"),
                    sval(a, "endpoint"),
                    sval(a, "model"),
                    protect(sval(a, "api_key_enc"))?,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
                )
                .map_err(|e| format!("写入助手配置失败: {}", e))?;
            }
        }

        if let Some(overrides) = tables.get("platform_overrides").and_then(|x| x.as_array()) {
            conn.execute("DELETE FROM platform_overrides", [])
                .map_err(|e| format!("清空政策补丁失败: {}", e))?;
            for o in overrides {
                conn.execute(
                    "INSERT INTO platform_overrides (id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                    params![
                        ival(o, "id"),
                        sval(o, "name"),
                        sval(o, "name_en"),
                        sval(o, "category"),
                        sval(o, "subtype"),
                        ival(o, "has_property"),
                        ival(o, "has_personality"),
                        ival(o, "inheritable"),
                        sval(o, "required_documents"),
                        sval(o, "process_description"),
                        sval(o, "estimated_difficulty"),
                        sval(o, "official_policy_url"),
                        sval(o, "estimated_time_days"),
                        sval(o, "notes"),
                        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    ],
                ).map_err(|e| format!("写入政策补丁失败: {}", e))?;
            }
        }

        Ok(count)
    })();

    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| format!("提交事务失败: {}", e))?;
            crate::vault::clear_key();
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 将存量明文敏感字段加密（幂等；返回改动的资产行数）
pub fn encrypt_existing_data_db() -> Result<i64, String> {
    if !crate::vault::is_unlocked() {
        return Err("请先解锁".to_string());
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut changed: i64 = 0;

    let mut stmt = conn.prepare(
        "SELECT id, account_identifier, account_description, heir_name, heir_contact, notes, credential_enc FROM digital_assets",
    ).map_err(|e| format!("查询资产失败: {}", e))?;
    let rows: Vec<(i64, String, String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("读取资产失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集资产失败: {}", e))?;

    for (id, ai, ad, hn, hc, nt, cr) in rows {
        let nai = crate::vault::encrypt_if_needed(&ai)?;
        let nad = crate::vault::encrypt_if_needed(&ad)?;
        let nhn = crate::vault::encrypt_if_needed(&hn)?;
        let nhc = crate::vault::encrypt_if_needed(&hc)?;
        let nnt = crate::vault::encrypt_if_needed(&nt)?;
        let ncr = crate::vault::encrypt_if_needed(&cr)?;
        if nai != ai || nad != ad || nhn != hn || nhc != hc || nnt != nt || ncr != cr {
            conn.execute(
                "UPDATE digital_assets SET account_identifier=?1, account_description=?2, heir_name=?3, heir_contact=?4, notes=?5, credential_enc=?6 WHERE id=?7",
                params![nai, nad, nhn, nhc, nnt, ncr, id],
            ).map_err(|e| format!("加密存量资产失败: {}", e))?;
            changed += 1;
        }
    }

    if let Some((pid, vals)) = select_profile_sensitive(conn)? {
        let mut new_vals = Vec::with_capacity(vals.len());
        let mut dirty = false;
        for v in &vals {
            let nv = crate::vault::encrypt_if_needed(v)?;
            if nv != *v {
                dirty = true;
            }
            new_vals.push(nv);
        }
        if dirty {
            update_profile_sensitive(conn, pid, &new_vals)
                .map_err(|e| format!("加密存量档案失败: {}", e))?;
            changed += 1;
        }
    }

    // 继承人名单同样必须收编：否则开启加密后它仍是明文落库。
    if encrypt_existing_heirs(conn)? {
        changed += 1;
    }

    Ok(changed)
}

/// 把 heirs 表的明文值就地加密（启用加密时的存量收编）。
fn encrypt_existing_heirs(conn: &Connection) -> Result<bool, String> {
    let rows: Vec<(i64, String, String, String, String, String)> = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,'') FROM heirs",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集继承人失败: {}", e))?;
    let mut dirty = false;
    for (id, name, relation, phone, email, share_note) in rows {
        let n_name = crate::vault::encrypt_if_needed(&name)?;
        let n_rel = crate::vault::encrypt_if_needed(&relation)?;
        let n_phone = crate::vault::encrypt_if_needed(&phone)?;
        let n_email = crate::vault::encrypt_if_needed(&email)?;
        let n_share = crate::vault::encrypt_if_needed(&share_note)?;
        if n_name != name
            || n_rel != relation
            || n_phone != phone
            || n_email != email
            || n_share != share_note
        {
            conn.execute(
                "UPDATE heirs SET name=?1, relation=?2, phone=?3, email=?4, share_note=?5 WHERE id=?6",
                params![n_name, n_rel, n_phone, n_email, n_share, id],
            )
            .map_err(|e| format!("加密存量继承人失败: {}", e))?;
            dirty = true;
        }
    }
    Ok(dirty)
}

// ========== Check-in / Dead Man's Switch ==========

pub fn record_check_in_db() -> Result<(), String> {
    ensure_vault_writable_db()?;
    record_check_in_internal_db()
}

pub(crate) fn record_check_in_internal_db() -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO check_in_log (checked_at, status, note) VALUES (?1, 'ok', '用户手动检查')",
        params![now],
    )
    .map_err(|e| format!("记录检查失败: {}", e))?;

    conn.execute(
        "UPDATE dms_config SET last_check_in=?1 WHERE id=1",
        params![now],
    )
    .map_err(|e| format!("更新DMS状态失败: {}", e))?;

    conn.execute(
        "UPDATE user_profile SET last_check_in=?1 WHERE id=(SELECT MAX(id) FROM user_profile)",
        params![now],
    )
    .map_err(|e| format!("更新用户签到时间失败: {}", e))?;

    Ok(())
}

pub fn get_check_in_status_db() -> Result<CheckInStatus, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let last_check_in: Option<String> = conn
        .query_row(
            "SELECT last_check_in FROM dms_config WHERE id=1",
            [],
            |row| row.get(0),
        )
        .ok();

    let interval: i64 = conn
        .query_row(
            "SELECT check_in_interval_days FROM dms_config WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(30);

    let dms_enabled: bool = conn
        .query_row("SELECT enabled FROM dms_config WHERE id=1", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v != 0)
        .unwrap_or(false);

    let next_check_in_due = if let Some(ref last) = last_check_in {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last, "%Y-%m-%d %H:%M:%S") {
            let next = dt + chrono::Duration::days(interval);
            Some(next.format("%Y-%m-%d").to_string())
        } else {
            None
        }
    } else {
        None
    };

    let days_until_due = if let Some(ref last) = last_check_in {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last, "%Y-%m-%d %H:%M:%S") {
            let next = dt + chrono::Duration::days(interval);
            let now = chrono::Local::now().naive_local();
            let duration = next.signed_duration_since(now);
            // 向上取整：`num_days()` 截断会把「还剩 20 小时」算成 0 天，
            // 界面据此判定逾期，用户会在到期前被误报（含系统通知）。
            Some((duration.num_seconds() as f64 / 86_400.0).ceil() as i64)
        } else {
            None
        }
    } else {
        None
    };

    Ok(CheckInStatus {
        last_check_in,
        interval_days: interval,
        dms_enabled,
        next_check_in_due,
        days_until_due,
        recent_check_ins: recent_check_ins_db(conn, 5).unwrap_or_default(),
    })
}

/// 最近若干次签到时间（新的在前）。读失败返回 Err，由调用方决定是否降级为空。
fn recent_check_ins_db(conn: &Connection, limit: usize) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT checked_at FROM check_in_log ORDER BY id DESC LIMIT ?1")
        .map_err(|e| format!("查询签到记录失败: {}", e))?;
    let rows = stmt
        .query_map(params![limit as i64], |r| r.get::<_, String>(0))
        .map_err(|e| format!("读取签到记录失败: {}", e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集签到记录失败: {}", e))
}

pub fn set_dms_config_db(enabled: bool, interval_days: i64) -> Result<(), String> {
    ensure_vault_writable_db()?;
    set_dms_config_internal_db(enabled, interval_days)
}

pub(crate) fn set_dms_config_internal_db(enabled: bool, interval_days: i64) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();

    conn.execute(
        "INSERT OR IGNORE INTO dms_config (id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in) VALUES (1, 0, 30, 3, 7, 14, NULL)",
        [],
    ).map_err(|e| format!("初始化DMS配置失败: {}", e))?;

    // 首次启用时写入起始锚点：没有锚点，逾期天数永远算不出来，
    // 用户会以为「已开启」而实际永远不会触发任何提醒（已由
    // enabled_dms_anchors_on_first_enable_instead_of_never_firing 锁定）。
    // 已有锚点则保持不变——改间隔不应重置倒计时。
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE dms_config SET enabled=?1, check_in_interval_days=?2,          last_check_in = CASE WHEN ?1 = 1 AND last_check_in IS NULL THEN ?3 ELSE last_check_in END          WHERE id=1",
        params![if enabled { 1 } else { 0 }, interval_days, now],
    )
    .map_err(|e| format!("设置DMS失败: {}", e))?;

    Ok(())
}

/// SMTP 是否已配置到「可以真正发送」的程度（host 与 from 都非空）。
/// 直接查库：调用方可能已持有连接锁，不能再走 `get_conn()`。
fn smtp_configured_in(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COALESCE(host,''), COALESCE(from_addr,'') FROM smtp_config WHERE id=1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map(|(host, from)| !host.trim().is_empty() && !from.trim().is_empty())
    .unwrap_or(false)
}

/// 仪表板统计。
///
/// **不要合并成单条聚合查询**（已实测否决）：12 条独立 COUNT 中多数能命中
/// `idx_assets_category` / `idx_assets_disposal` 覆盖索引，只读索引不读表；
/// 而单条 SUM(CASE ...) 必须整表扫描并对每行求值 12 个分支。
/// 实测（Python sqlite3，WAL + synchronous=NORMAL，300 次取均值）：
///   20 行: 0.077ms vs 0.030ms（合并更快，但绝对值可忽略）
///   500 行: 0.378ms vs 0.403ms
///   2000 行: 1.365ms vs 1.663ms（合并慢 22%）
///   20000 行: 28.6ms vs 18.5ms（只有到万级才反超）
/// 个人数字资产量级（几十到几百条）下合并无收益且更慢，故保持逐项查询。
pub fn get_dashboard_stats_db() -> Result<DashboardStats, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM digital_assets", [], |row| row.get(0))
        .map_err(|e| format!("统计失败: {}", e))?;

    let property_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets WHERE category='property'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let personality_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets WHERE category='personality'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let mixed_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets WHERE category='mixed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    // 注意：这里不再统计 `is_confirmed`。该列在插入/更新时恒写 1，
    // 从未有真实的「未确认」状态，`unconfirmed_count` 永远为 0——
    // 一个永远显示「全部已确认」的指标比没有指标更糟，故整列从统计中移除
    // （列本身保留，避免破坏既有备份与迁移路径）。
    // 只累加非负有限值：数据可能来自手改备份或外部导入，
    // 负估值会把「总价值」算成负数（前端 totalValue 已有同样口径）。
    let total_value: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(CASE WHEN estimated_value > 0 AND estimated_value < 1e12 THEN estimated_value ELSE 0 END), 0.0) FROM digital_assets",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);
    let disposal_planned: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets WHERE disposal_method != 'no_action'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    // 继承人：只有「选继承却没填名字」才是不合格，其他处置方式不涉及继承人。
    let heir_assigned: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets
             WHERE disposal_method != 'inherit'
                OR (heir_name IS NOT NULL AND TRIM(heir_name) != '')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    // 账号标识：占位符表示未解密，不算已填——否则锁定态下指标会虚高。
    let identifier_ready: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets
             WHERE account_identifier IS NOT NULL
               AND TRIM(account_identifier) != ''
               AND account_identifier NOT LIKE '%需解锁%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let last_check_in: Option<String> = conn
        .query_row("SELECT MAX(checked_at) FROM check_in_log", [], |row| {
            row.get(0)
        })
        .ok();
    let dms_enabled: bool = conn
        .query_row("SELECT enabled FROM dms_config WHERE id=1", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|v| v != 0)
        .unwrap_or(false);

    // 触发链可用性：只认真正能送达的通道，不看「开关有没有打开」。
    // 旧界面把「DMS 已启用」等同于「家人会收到通知」，实际没配 SMTP 时
    // 通知只进本地发件箱，家人什么都收不到。
    //
    // 注意：这里必须直接查库，不能调用 `notify::resolve_smtp_settings()`——
    // 那个函数内部会 `get_conn()`，而本函数已持有连接锁，二次加锁会死锁
    // （std Mutex 不可重入，实测整个测试套挂死）。
    let smtp_ready = smtp_configured_in(conn);
    let heartbeat_ready = conn
        .query_row(
            "SELECT enabled, COALESCE(endpoint,'') FROM heartbeat_config WHERE id=1",
            [],
            |r| Ok((r.get::<_, i64>(0)? != 0, r.get::<_, String>(1)?)),
        )
        .map(|(enabled, endpoint)| enabled && !endpoint.trim().is_empty())
        .unwrap_or(false);
    let trigger_channel = if smtp_ready {
        "smtp".to_string()
    } else if heartbeat_ready {
        "heartbeat".to_string()
    } else {
        "none".to_string()
    };
    // 「触发链就绪」= DMS 启用 **且** 家人真的能收到（当前只有 SMTP 能做到）。
    // 心跳只证明「设备还在联网」，它不通知任何家人，因此不能算就绪。
    let trigger_ready = dms_enabled && trigger_channel == "smtp";

    // 四项各 25%。任何一项为 0，整体就不会虚高到「完成」。
    let ratio = |n: i64| -> i64 {
        if total > 0 {
            n * 100 / total
        } else {
            0
        }
    };
    // 只有「家人真的能收到」才算这一项达标。
    // 仅心跳可用时，家人收不到邮件——这不算触发链可用，否则完成度会虚高到 100%
    // 同时界面提示「家人收不到邮件」，自相矛盾。
    // ── 待办检查项：把「还差什么」算出来，而不是只给一个百分比 ──
    // 每条都必须可执行（带跳转目标），否则用户看完仍不知道下一步。
    let missing_disposal: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets WHERE disposal_method = 'no_action'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let missing_heir: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets
             WHERE disposal_method = 'inherit' AND (heir_name IS NULL OR TRIM(heir_name) = '')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let missing_identifier: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets
             WHERE account_identifier IS NULL
                OR TRIM(account_identifier) = ''
                OR account_identifier LIKE '%需解锁%'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let no_credential: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM digital_assets
             WHERE (credential_enc IS NULL OR credential_enc = '')
               AND disposal_method IN ('inherit', 'liquidate', 'authorize_view')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    // 保管位置：家人能否找到备份与恢复码
    // 用 EXISTS 判断「是否已填写」，而不是 COUNT。
    // 空库时 custody_config 没有行，COUNT(*) 返回 0，会被误判成「已填写」，
    // 于是这条最关键的 blocker 永远不出现（实测就是这么被抓出来的）。
    let custody_filled: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custody_config
                WHERE id=1
                  AND COALESCE(backup_location_enc,'') != ''
                  AND COALESCE(recovery_code_location_enc,'') != ''
             )",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(false);
    let custody_missing = if custody_filled { 0 } else { 1 };

    // 继承人恢复包：主密码启用后，没有恢复包意味着「只有这台机器的主人能解密」。
    // 这是本项目最核心的能力缺口，必须作为 blocker 出现在待办里。
    let recovery_kit_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM heir_recovery WHERE id=1)",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(false);
    // 必须直接查库：本函数已持有连接锁，调用 get_security_config_db() 会二次加锁死锁
    // （std Mutex 不可重入，历史上同类写法让整个测试套挂死）。
    let vault_configured: bool = conn
        .query_row("SELECT COUNT(*) FROM security_config WHERE id=1", [], |r| {
            r.get::<_, i64>(0)
        })
        .map(|n| n > 0)
        .unwrap_or(false);

    let mut issues: Vec<crate::models::CheckIssue> = Vec::new();
    if total > 0 && missing_disposal > 0 {
        issues.push(crate::models::CheckIssue {
            key: "missing_disposal".into(),
            severity: "warn".into(),
            count: missing_disposal,
            target: "/assets".into(),
        });
    }
    if missing_heir > 0 {
        issues.push(crate::models::CheckIssue {
            key: "missing_heir".into(),
            severity: "blocker".into(),
            count: missing_heir,
            target: "/assets".into(),
        });
    }
    if missing_identifier > 0 {
        issues.push(crate::models::CheckIssue {
            key: "missing_identifier".into(),
            severity: "warn".into(),
            count: missing_identifier,
            target: "/assets".into(),
        });
    }
    if no_credential > 0 {
        issues.push(crate::models::CheckIssue {
            key: "no_credential".into(),
            severity: "info".into(),
            count: no_credential,
            target: "/assets".into(),
        });
    }
    if custody_missing > 0 {
        issues.push(crate::models::CheckIssue {
            key: "custody_missing".into(),
            severity: "blocker".into(),
            count: 0,
            target: "/profile".into(),
        });
    }
    // 只有启用了主密码才需要恢复包；明文模式下没有可拆分的密钥。
    if vault_configured && !recovery_kit_exists {
        issues.push(crate::models::CheckIssue {
            key: "recovery_kit_missing".into(),
            severity: "blocker".into(),
            count: 0,
            target: "/profile".into(),
        });
    }
    // 政策未核实：按「用户实际持有的资产」统计，而不是按政策库总量。
    // 政策库 152 条里有 147 条未核实，如果直接报 147，用户会觉得与自己无关；
    // 报「你持有的 N 项资产对应的政策尚未核实」才是他能感知的风险。
    let unverified_platforms: std::collections::HashSet<i64> =
        crate::platform_data::get_all_platforms()
            .into_iter()
            .filter(|p| p.data_source.contains("未核实"))
            .map(|p| p.id)
            .collect();
    let asset_platforms: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT platform_id FROM digital_assets")
            .map_err(|e| format!("查询资产平台失败: {}", e))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, i64>(0))
            .map_err(|e| format!("读取资产平台失败: {}", e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("收集资产平台失败: {}", e))?
    };
    let unverified_hit = asset_platforms
        .iter()
        .filter(|id| unverified_platforms.contains(id))
        .count() as i64;
    if unverified_hit > 0 {
        issues.push(crate::models::CheckIssue {
            key: "policy_unverified".into(),
            severity: "info".into(),
            count: unverified_hit,
            target: "/review".into(),
        });
    }

    if !trigger_ready {
        issues.push(crate::models::CheckIssue {
            key: if dms_enabled {
                "trigger_no_channel".into()
            } else {
                "trigger_disabled".into()
            },
            severity: "blocker".into(),
            count: 0,
            target: "/profile".into(),
        });
    }

    let trigger_score = if trigger_ready { 100 } else { 0 };
    let completion = if total > 0 {
        (ratio(disposal_planned) + ratio(heir_assigned) + ratio(identifier_ready) + trigger_score)
            / 4
    } else {
        0
    };

    Ok(DashboardStats {
        total_assets: total,
        property_count,
        personality_count,
        mixed_count,
        disposal_planned_count: disposal_planned,
        heir_assigned_count: heir_assigned,
        identifier_ready_count: identifier_ready,
        trigger_ready,
        trigger_channel,
        total_estimated_value: total_value,
        last_check_in,
        dms_enabled,
        completion_percentage: completion,
        issues,
    })
}

/// 检查状态
#[derive(Debug, serde::Serialize)]
pub struct CheckInStatus {
    pub last_check_in: Option<String>,
    pub interval_days: i64,
    pub dms_enabled: bool,
    pub next_check_in_due: Option<String>,
    pub days_until_due: Option<i64>,
    /// 最近 5 次确认时间（新的在前）。签到日志此前只写不读，
    /// 用户无法判断「我真的点过吗、上次是什么时候」。
    pub recent_check_ins: Vec<String>,
}

// ========== 继承报告 ==========

/// 报告里出现的资产类别（去重）。
fn report_categories(assets: &[DigitalAsset]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in assets {
        if !out.contains(&a.category) {
            out.push(a.category.clone());
        }
    }
    out
}

pub fn get_inheritance_report_db() -> Result<InheritanceReport, String> {
    let user = get_user_profile_db()?.unwrap_or(UserProfile {
        id: None,
        full_name: "未设置".to_string(),
        phone: String::new(),
        email: String::new(),
        id_last4: String::new(),
        emergency_contact_name: String::new(),
        emergency_contact_phone: String::new(),
        emergency_contact_relation: String::new(),
        emergency_contact_email: String::new(),
        heir_primary_name: String::new(),
        heir_primary_phone: String::new(),
        heir_primary_relation: String::new(),
        heir_primary_email: String::new(),
        check_in_interval_days: 30,
        last_check_in: None,
        dms_enabled: false,
        created_at: String::new(),
        updated_at: String::new(),
    });

    let assets = get_all_assets_db()?;
    // 与仪表板同口径：只累加有限非负值，避免脏数据让报告金额为负。
    let total_value: f64 = assets
        .iter()
        .filter_map(|a| a.estimated_value)
        .filter(|v| v.is_finite() && *v > 0.0 && *v < 1e12)
        .sum();

    let property_count = assets.iter().filter(|a| a.category == "property").count() as i64;
    let personality_count = assets
        .iter()
        .filter(|a| a.category == "personality")
        .count() as i64;
    let mixed_count = assets.iter().filter(|a| a.category == "mixed").count() as i64;

    let platform_ids: Vec<i64> = assets.iter().map(|a| a.platform_id).collect();
    let policies = get_policy_briefs_db(&platform_ids)?;
    // 多继承人名单；为空时报告与执行清单回退到 user.heir_primary_*。
    let heirs = get_heirs_db()?;
    // 只挑与本次资产类别相关的来源：全部塞进去会让报告变成法规清单。
    let categories: Vec<String> = report_categories(&assets);
    let authorities = crate::models::authorities()
        .into_iter()
        .filter(|a| a.applies_to.iter().any(|c| categories.contains(c)))
        .collect();

    Ok(InheritanceReport {
        user,
        assets,
        heirs,
        policies,
        authorities,
        total_estimated_value: total_value,
        property_assets_count: property_count,
        personality_assets_count: personality_count,
        mixed_assets_count: mixed_count,
        generated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

// ========== AI 助手会话历史 ==========

/// 追加一条问答记录。
pub fn append_ai_chat_db(question: &str, answer: &str, model: &str) -> Result<(), String> {
    ensure_vault_writable_db()?;
    if question.chars().count() > 2_000 || answer.chars().count() > 20_000 {
        return Err("问答内容超出长度上限".to_string());
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "INSERT INTO ai_chat_log (question, answer, model, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            question,
            answer,
            model,
            Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
        ],
    )
    .map_err(|e| format!("保存问答记录失败: {}", e))?;
    Ok(())
}

/// 最近 N 条问答（新的在前）。
pub fn list_ai_chat_db(limit: usize) -> Result<Vec<serde_json::Value>, String> {
    ensure_vault_readable_db()?;
    let limit = limit.clamp(1, 200) as i64;
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, question, answer, model, created_at FROM ai_chat_log ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询问答历史失败: {}", e))?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?,
                "question": r.get::<_, String>(1)?,
                "answer": r.get::<_, String>(2)?,
                "model": r.get::<_, String>(3)?,
                "created_at": r.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| format!("读取问答历史失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集问答历史失败: {}", e))?;
    Ok(rows)
}

/// 清空问答历史。
pub fn clear_ai_chat_db() -> Result<(), String> {
    ensure_vault_writable_db()?;
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute("DELETE FROM ai_chat_log", [])
        .map_err(|e| format!("清空问答历史失败: {}", e))?;
    Ok(())
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;

#[path = "db_platforms.rs"]
mod platforms;
pub use platforms::*;

#[path = "db_custody.rs"]
mod custody;
pub use custody::*;

#[path = "db_heirs.rs"]
mod heirs;
pub use heirs::*;

#[path = "db_review.rs"]
mod review_store;
pub use review_store::*;

#[path = "db_assets.rs"]
mod assets_store;
pub use assets_store::*;

#[path = "db_profile.rs"]
mod profile_store;
pub use profile_store::*;
