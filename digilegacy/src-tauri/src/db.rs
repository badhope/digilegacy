use chrono::Local;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection, OpenFlags};
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
/// v8：新增 `agent_run_log`（Agent 运行审计摘要，90 天保留）。
/// 这些表都用 `CREATE TABLE IF NOT EXISTS` 建，存量库无需逐列迁移；
/// 版本号仍然递增，让未来任何破坏性迁移能按数字分支而不是靠猜。
const SCHEMA_VERSION: i64 = 8;

// ========== 整库加密（SQLCipher）==========

// SQLCipher 的明文模式 = 不提供 key：无 key 时它以普通 SQLite 语义工作，
// 与未加密文件完全兼容（实测：`PRAGMA key = ''` 在 SQLCipher 4 会报
// "requires a key of one or more characters"，空 key 不是兼容开关）。
// 因此明文路径什么都不做，这条注释就是「故意不发 key」的说明书。

/// 用 32 字节随机 key 打开 SQLCipher 库（raw-key 形式 `x'hex'`）。
/// key 已是 CSPRNG 输出，用 raw 形式绕过 SQLCipher 内置 PBKDF2——
/// 再过一遍 KDF 不增加熵，只会拖慢每次打开。
pub(crate) fn apply_db_key(conn: &Connection, key: &[u8; 32]) -> Result<(), String> {
    let hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
    conn.pragma_update(None, "key", format!("x'{}'", hex))
        .map_err(|e| format!("设置库密钥失败: {}", e))
}

/// key 正确与否的唯一判据：能否读 schema。SQLCipher 对错误 key 的行为是
/// 在第一次真实读时抛 "file is not a database"，打开动作本身不报错——
/// 所以打开后必须立即验证，不能等到第一条业务查询才炸。
pub(crate) fn verify_db_readable(conn: &Connection) -> Result<(), String> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|_| ())
    .map_err(|_| "库密钥不正确或数据库文件已损坏".to_string())
}

/// 初始化数据库
pub fn init_database(app_handle: &AppHandle) -> Result<(), String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {}", e))?;

    std::fs::create_dir_all(&app_dir).map_err(|e| format!("创建应用目录失败: {}", e))?;

    // 整库加密：dbkey.bin 存在 → 用 DPAPI 解出的 key 打开加密库；
    // 不存在 → 明文库（key=''）。key 文件在但解不开是严重错误，
    // 必须显式失败——静默重建明文库等于把加密承诺整个作废。
    match crate::db_crypto::load_db_key(&app_dir)? {
        Some(key) => init_database_at_with_key(app_dir.join("digilegacy.db"), Some(&key)),
        None => init_database_at(app_dir.join("digilegacy.db")),
    }
}

/// 以只读模式打开既有数据库（MCP 服务器 bin 专用）。
///
/// 与 `init_database_at` 的差异：`SQLITE_OPEN_READ_ONLY` 打开、不迁移、不建表、
/// 不同步平台库——即使 MCP 进程被恶意客户端驱动，也没有任何写库路径。
/// 数据库必须已经由主程序建好；WAL 残留（主程序崩溃后未 checkpoint）会让
/// 只读连接打不开，此时报错并提示先跑一次主程序，而不是降级成读写。
pub fn init_database_readonly_at(db_path: PathBuf) -> Result<(), String> {
    init_database_readonly_at_with_key(db_path, None)
}

/// 只读打开（带可选 SQLCipher key）。MCP 服务器专用：
/// 主库启用整库加密时由 DPAPI 解出的 key 在此注入。
pub fn init_database_readonly_at_with_key(
    db_path: PathBuf,
    key: Option<&[u8; 32]>,
) -> Result<(), String> {
    if !db_path.exists() {
        return Err(format!("数据库不存在: {}", db_path.display()));
    }
    {
        let mut path_guard = DB_PATH.lock().unwrap();
        *path_guard = Some(db_path.clone());
    }
    let conn =
        Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
            format!(
                "只读打开数据库失败（若主程序曾异常退出，请先启动一次主程序完成整理）: {}",
                e
            )
        })?;
    if let Some(k) = key {
        apply_db_key(&conn, k)?;
    }
    // key 为 None = 明文模式：不发 key
    // 只读连接也要验证 key：错误 key 会在第一条查询才暴露
    verify_db_readable(&conn)?;
    conn.pragma_update(None, "busy_timeout", "5000")
        .map_err(|e| format!("设置 busy_timeout 失败: {}", e))?;
    {
        let mut guard = CONN.lock().unwrap();
        *guard = Some(conn);
    }
    Ok(())
}

// ========== 整库加密迁移（sqlcipher_export 官方路径）==========

fn key_hex(key: &[u8; 32]) -> String {
    key.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 把当前打开的明文库升级为 SQLCipher 加密库。
///
/// 流程：WAL checkpoint → 明文库快照（留在应用目录，用户可自行删除）→
/// `sqlcipher_export` 导出到临时加密库（官方迁移路径，复制全部表/索引/触发器；
/// user_version 需手动补）→ 独立连接验证 → 关闭全局连接 → 原子替换 →
/// 用 key 重开。任何一步失败：临时文件与 key 文件清除，原库原样保留。
pub fn encrypt_database_at_runtime(app_dir: &std::path::Path) -> Result<(), String> {
    if crate::db_crypto::load_db_key(app_dir)?.is_some() {
        return Err("数据库已经是整库加密状态".to_string());
    }
    let key = crate::db_crypto::create_db_key(app_dir)?;
    let hex = key_hex(&key);
    let db_path = {
        let guard = DB_PATH.lock().unwrap();
        guard.clone().ok_or_else(|| "数据库未初始化".to_string())?
    };
    let tmp = app_dir.join("digilegacy-enc.tmp");
    let _ = std::fs::remove_file(&tmp);

    let prepare = (|| -> Result<(), String> {
        {
            let guard = get_conn()?;
            let conn = guard.as_ref().unwrap();
            // WAL checkpoint：把 -wal 里的数据全部落进主文件，否则快照丢最新写入
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .map_err(|e| format!("WAL checkpoint 失败: {}", e))?;
            let snap = app_dir.join(format!(
                "digilegacy-plaintext-{}.bak",
                Local::now().format("%Y%m%d-%H%M%S")
            ));
            std::fs::copy(&db_path, &snap).map_err(|e| format!("快照明文库失败: {}", e))?;
            conn.execute_batch(&format!(
                "ATTACH DATABASE '{}' AS enc KEY \"x'{}'\"; SELECT sqlcipher_export('enc'); PRAGMA enc.user_version = {}; DETACH DATABASE enc;",
                tmp.display().to_string().replace('\'', "''"),
                hex,
                SCHEMA_VERSION
            ))
            .map_err(|e| format!("sqlcipher_export 失败: {}", e))?;
        }
        // 用独立连接验证临时加密库（不影响全局连接，失败可直接放弃）
        {
            let vconn = Connection::open(&tmp).map_err(|e| format!("打开临时库失败: {}", e))?;
            apply_db_key(&vconn, &key)?;
            verify_db_readable(&vconn)?;
            let uv: i64 = vconn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .map_err(|e| format!("读取 schema 版本失败: {}", e))?;
            if uv != SCHEMA_VERSION {
                return Err(format!("迁移后 schema 版本不一致: {uv}"));
            }
        }
        Ok(())
    })();

    if let Err(e) = prepare {
        let _ = std::fs::remove_file(&tmp);
        let _ = crate::db_crypto::delete_db_key(app_dir);
        return Err(e);
    }

    // 原子替换：必须先 drop 全局连接释放 Windows 文件锁
    {
        let mut guard = CONN.lock().unwrap();
        *guard = None;
        std::fs::rename(&tmp, &db_path).map_err(|e| format!("替换数据库文件失败: {}", e))?;
        // 明文库时代的 WAL/SHM 残留对加密库是垃圾，清掉
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        let conn = Connection::open(&db_path).map_err(|e| format!("重开加密库失败: {}", e))?;
        apply_db_key(&conn, &key)?;
        verify_db_readable(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("设置 WAL 模式失败: {}", e))?;
        conn.pragma_update(None, "busy_timeout", "5000")
            .map_err(|e| format!("设置 busy_timeout 失败: {}", e))?;
        *guard = Some(conn);
    }
    Ok(())
}

/// 把加密库转回明文库（对称操作，供用户主动停用整库加密）。
pub fn decrypt_database_at_runtime(app_dir: &std::path::Path) -> Result<(), String> {
    let key = crate::db_crypto::load_db_key(app_dir)?
        .ok_or_else(|| "数据库当前未启用整库加密".to_string())?;
    // export 到明文用 KEY ''，key 本身不再参与 SQL——但必须先验证它是有效的
    // （load 成功即证明 DPAPI 解保护成功），key 语义保留为「当前库确实是加密的」。
    let _ = key;
    let db_path = {
        let guard = DB_PATH.lock().unwrap();
        guard.clone().ok_or_else(|| "数据库未初始化".to_string())?
    };
    let tmp = app_dir.join("digilegacy-plain.tmp");
    let _ = std::fs::remove_file(&tmp);
    let snapshot = app_dir.join(format!(
        "digilegacy-encrypted-{}.bak",
        Local::now().format("%Y%m%d-%H%M%S")
    ));

    let prepare = (|| -> Result<(), String> {
        {
            let guard = get_conn()?;
            let conn = guard.as_ref().unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .map_err(|e| format!("WAL checkpoint 失败: {}", e))?;
            std::fs::copy(&db_path, &snapshot).map_err(|e| format!("快照加密库失败: {}", e))?;
            conn.execute_batch(&format!(
                "ATTACH DATABASE '{}' AS plain KEY ''; SELECT sqlcipher_export('plain'); PRAGMA plain.user_version = {}; DETACH DATABASE plain;",
                tmp.display().to_string().replace('\'', "''"),
                SCHEMA_VERSION
            ))
            .map_err(|e| format!("sqlcipher_export 失败: {}", e))?;
        }
        {
            let vconn = Connection::open(&tmp).map_err(|e| format!("打开临时库失败: {}", e))?;
            verify_db_readable(&vconn)?;
        }
        Ok(())
    })();

    if let Err(e) = prepare {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    {
        let mut guard = CONN.lock().unwrap();
        *guard = None;
        std::fs::rename(&tmp, &db_path).map_err(|e| format!("替换数据库文件失败: {}", e))?;
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
        let conn = Connection::open(&db_path).map_err(|e| format!("重开明文库失败: {}", e))?;
        // 明文模式：不发 key（见上方 SQLCipher 明文模式说明）
        verify_db_readable(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("设置 WAL 模式失败: {}", e))?;
        *guard = Some(conn);
    }
    crate::db_crypto::delete_db_key(app_dir)?;
    Ok(())
}

/// 整库加密是否已启用（dbkey.bin 存在）。
pub fn is_db_encryption_enabled(app_dir: &std::path::Path) -> bool {
    crate::db_crypto::load_db_key(app_dir)
        .map(|k| k.is_some())
        .unwrap_or(false)
}

/// 在指定路径初始化明文数据库（供测试与未启用整库加密的启动流程复用）。
pub fn init_database_at(db_path: PathBuf) -> Result<(), String> {
    init_database_at_with_key(db_path, None)
}

/// 带可选 SQLCipher key 的初始化。`Some(key)` 打开/创建加密库，
/// `None` 打开/创建明文库（SQLCipher 构建下显式 `PRAGMA key = ''`）。
/// 所有表结构与迁移逻辑两条路径完全一致——加密的是存储层，不是逻辑层。
pub fn init_database_at_with_key(db_path: PathBuf, key: Option<&[u8; 32]>) -> Result<(), String> {
    {
        let mut path_guard = DB_PATH.lock().unwrap();
        *path_guard = Some(db_path.clone());
    }

    let conn = Connection::open(&db_path).map_err(|e| format!("打开数据库失败: {}", e))?;
    // PRAGMA key 必须是打开后的第一条语句，任何提前的读写都会以
    // "file is not a database" 失败或触发 SQLCipher 的默认 KDF 路径。
    if let Some(k) = key {
        apply_db_key(&conn, k)?;
        // key 模式下先验证可读性：错误 key 在这里给出明确错误，
        // 而不是等到 journal_mode 才报一句含混的 "file is not a database"
        verify_db_readable(&conn)?;
    }
    // key 为 None = 明文模式：不发 key，SQLCipher 以普通 SQLite 语义工作

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
            last_check_in TEXT,
            -- 外出模式：暂停到该日期（YYYY-MM-DD），到期自动恢复；空 = 未暂停
            paused_until TEXT DEFAULT ''
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

        -- Agent 运行审计摘要：只存任务描述、结果摘要与工具名清单。
        -- 工具参数与结果全文不落库——审计要能复盘，但不能变成第二份敏感数据副本。
        -- 保留 90 天，写入时顺手清理过期行（lazy retention，无定时器）。
        CREATE TABLE IF NOT EXISTS agent_run_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            started_at TEXT NOT NULL,
            task TEXT NOT NULL,
            outcome TEXT NOT NULL DEFAULT 'final',
            final_text TEXT DEFAULT '',
            rounds INTEGER DEFAULT 0,
            tools_used TEXT DEFAULT ''
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
    for col in [
        "emergency_contact_email",
        "heir_primary_email",
        "last_reviewed_at",
    ] {
        if !profile_columns.iter().any(|name| name == col) {
            conn.execute_batch(&format!(
                "ALTER TABLE user_profile ADD COLUMN {} TEXT DEFAULT ''",
                col
            ))?;
        }
    }

    // dms_config.paused_until：外出模式暂停。竞品（SafeBeyond、LastSend 等）普遍
    // 支持 check-in 暂停；用日期而不是布尔——忘记恢复也会自动到期，不会一直裸奔。
    let has_paused_until: bool = conn
        .prepare("PRAGMA table_info(dms_config)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name == "paused_until");
    if !has_paused_until {
        conn.execute_batch("ALTER TABLE dms_config ADD COLUMN paused_until TEXT DEFAULT ''")?;
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

/// 当前库的 schema 版本（`PRAGMA user_version`）。
///
/// 暴露给 CLI `doctor` 与 HTTP `/api/health`：升级后如果版本号没跟上，
/// 说明迁移没跑到，这比「看起来能用」更值得报警。
pub fn schema_version_db() -> Result<i64, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| format!("读取 schema 版本失败: {}", e))
}

/// 当前 schema 常量（供自检对照）。
pub fn expected_schema_version() -> i64 {
    SCHEMA_VERSION
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

    // 外出模式：paused_until 是未来日期时，逾期评估整体挂起（notify::stage_snapshot 同步此逻辑）。
    let paused_until_raw: Option<String> = conn
        .query_row("SELECT paused_until FROM dms_config WHERE id=1", [], |r| {
            r.get(0)
        })
        .ok()
        .filter(|s: &String| !s.trim().is_empty());
    let paused = paused_until_raw
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d >= chrono::Local::now().date_naive())
        .unwrap_or(false);

    Ok(CheckInStatus {
        last_check_in,
        interval_days: interval,
        dms_enabled,
        next_check_in_due,
        days_until_due,
        paused_until: if paused { paused_until_raw } else { None },
        paused,
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

// ========== Agent 运行审计 ==========

/// 一次 Agent 运行的审计行（列表展示用，不含结果全文）。
#[derive(Debug, serde::Serialize)]
pub struct AgentRunLogEntry {
    pub id: i64,
    pub started_at: String,
    pub task: String,
    /// final / truncated / error
    pub outcome: String,
    pub rounds: i64,
    pub tools_used: String,
}

/// 写入一次 Agent 运行摘要，并顺手清理 90 天前的旧行。
/// 失败只返回 Err 由调用方决定降级——审计失败不应让任务本身作废。
pub fn log_agent_run_db(
    task: &str,
    outcome: &str,
    final_text: &str,
    rounds: usize,
    tools_used: &str,
) -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO agent_run_log (started_at, task, outcome, final_text, rounds, tools_used) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![now, task, outcome, final_text, rounds as i64, tools_used],
    )
    .map_err(|e| format!("写入 Agent 审计失败: {}", e))?;
    conn.execute(
        "DELETE FROM agent_run_log WHERE started_at < datetime('now', 'localtime', '-90 days')",
        [],
    )
    .map_err(|e| format!("清理过期审计失败: {}", e))?;
    Ok(())
}

/// 最近 N 条 Agent 运行摘要（新的在前）。
pub fn list_agent_run_log_db(limit: usize) -> Result<Vec<AgentRunLogEntry>, String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT id, started_at, task, outcome, rounds, tools_used FROM agent_run_log ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询 Agent 审计失败: {}", e))?;
    let rows = stmt
        .query_map(params![limit.min(100) as i64], |r| {
            Ok(AgentRunLogEntry {
                id: r.get(0)?,
                started_at: r.get(1)?,
                task: r.get(2)?,
                outcome: r.get(3)?,
                rounds: r.get(4)?,
                tools_used: r.get(5)?,
            })
        })
        .map_err(|e| format!("读取 Agent 审计失败: {}", e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集 Agent 审计失败: {}", e))
}

/// 手动清空全部 Agent 审计（用户有权抹掉自己的痕迹）。
pub fn clear_agent_run_log_db() -> Result<(), String> {
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute("DELETE FROM agent_run_log", [])
        .map_err(|e| format!("清空 Agent 审计失败: {}", e))?;
    Ok(())
}

/// 设置/清除外出模式暂停。`until` 空串 = 清除；否则必须是 YYYY-MM-DD 且不早于今天。
/// 日期制而非布尔制：用户忘记恢复也会自动到期，不会让触发链一直裸奔。
pub fn set_dms_pause_db(until: &str) -> Result<(), String> {
    ensure_vault_writable_db()?;
    let until = until.trim();
    let value = if until.is_empty() {
        String::new()
    } else {
        let d = chrono::NaiveDate::parse_from_str(until, "%Y-%m-%d")
            .map_err(|_| "日期格式必须是 YYYY-MM-DD".to_string())?;
        if d < chrono::Local::now().date_naive() {
            return Err("暂停截止日不能早于今天".to_string());
        }
        until.to_string()
    };
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "UPDATE dms_config SET paused_until = ?1 WHERE id = 1",
        params![value],
    )
    .map_err(|e| format!("写入暂停状态失败: {}", e))?;
    Ok(())
}

/// 标记「本次规划已复核」：年度复核提醒的数据锚点。
/// 竞品调研（Switchify/GoodTrust 等的公开指南）都强调年检；但强调的人多，
/// 做进产品提醒链路的少——这里把它做成仪表板可派生的待办而不是日历喊话。
pub fn record_planning_review_db() -> Result<(), String> {
    ensure_vault_writable_db()?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let updated = conn
        .execute(
            "UPDATE user_profile SET last_reviewed_at = ?1 WHERE id = 1",
            params![today],
        )
        .map_err(|e| format!("写入复核时间失败: {}", e))?;
    if updated == 0 {
        conn.execute(
            "INSERT INTO user_profile (id, last_reviewed_at) VALUES (1, ?1)",
            params![today],
        )
        .map_err(|e| format!("写入复核时间失败: {}", e))?;
    }
    Ok(())
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

    // 年度复核提醒：规划是一次承诺，不是一次性设置。从未复核（且已有资产）
    // 或距上次复核超过 365 天时提醒。字段加密与否不影响本查询（普通列）。
    let last_reviewed_at: String = conn
        .query_row(
            "SELECT COALESCE(last_reviewed_at, '') FROM user_profile WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let review_stale = total > 0
        && (last_reviewed_at.trim().is_empty()
            || chrono::NaiveDate::parse_from_str(last_reviewed_at.trim(), "%Y-%m-%d")
                .ok()
                .map(|d| (chrono::Local::now().date_naive() - d).num_days() > 365)
                .unwrap_or(true));
    if review_stale {
        issues.push(crate::models::CheckIssue {
            key: "planning_review_stale".into(),
            severity: "info".into(),
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
        last_reviewed_at,
        review_stale,
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
    /// 外出模式暂停截止日（YYYY-MM-DD）；处于暂停期才有值。
    pub paused_until: Option<String>,
    /// 当前是否处于暂停期（逾期评估被挂起）。
    pub paused: bool,
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

#[path = "db_backup.rs"]
mod backup_store;
pub use backup_store::*;
