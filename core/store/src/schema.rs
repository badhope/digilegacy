//! Schema v19: asset depth fields (ownership, beneficiary, credential location, expiry, quantity, value status).

use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: i64 = 19;
pub const SCHEMA_SQL: &str = include_str!("schema.sql");
pub const EXPECTED_TABLES: i64 = 32;

fn checksum() -> String {
    let digest = Sha256::digest(SCHEMA_SQL.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn add_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    if has_column(conn, table, column)? {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
    // 表名/列名均来自本文件常量，不含外部输入。
    conn.execute_batch(&sql)
}

/// 建表 + 补列 + 写 migration ledger + 默认工作区。幂等。
pub fn apply_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;

    // v8 -> v9 additive columns. 旧库不重建，只补列。
    add_column(
        conn,
        "digital_assets",
        "status",
        "TEXT NOT NULL DEFAULT 'active'",
    )?;
    add_column(
        conn,
        "digital_assets",
        "last_verified_at",
        "TEXT DEFAULT ''",
    )?;
    add_column(conn, "digital_assets", "source", "TEXT DEFAULT 'manual'")?;
    add_column(
        conn,
        "digital_assets",
        "policy_snapshot_hash",
        "TEXT DEFAULT ''",
    )?;
    add_column(conn, "digital_assets", "ownership_share", "TEXT DEFAULT ''")?;
    add_column(
        conn,
        "digital_assets",
        "credential_location",
        "TEXT DEFAULT ''",
    )?;
    add_column(conn, "digital_assets", "expires_at", "TEXT DEFAULT ''")?;
    add_column(conn, "digital_assets", "quantity", "TEXT DEFAULT ''")?;
    add_column(conn, "digital_assets", "unit", "TEXT DEFAULT ''")?;
    add_column(
        conn,
        "digital_assets",
        "value_status",
        "TEXT NOT NULL DEFAULT 'market'",
    )?;
    add_column(
        conn,
        "user_profile",
        "emergency_contact_email",
        "TEXT DEFAULT ''",
    )?;
    add_column(
        conn,
        "user_profile",
        "heir_primary_email",
        "TEXT DEFAULT ''",
    )?;
    add_column(conn, "user_profile", "last_reviewed_at", "TEXT DEFAULT ''")?;
    add_column(conn, "dms_config", "paused_until", "TEXT DEFAULT ''")?;
    add_column(conn, "security_config", "recovery_seal", "TEXT DEFAULT ''")?;
    add_column(conn, "security_config", "wrapped_dek", "TEXT DEFAULT ''")?;
    add_column(conn, "agent_run_log", "steps_json", "TEXT DEFAULT '[]'")?;
    add_column(conn, "agent_run_log", "duration_ms", "INTEGER DEFAULT 0")?;
    add_column(conn, "agent_run_log", "tokens_in", "INTEGER DEFAULT 0")?;
    add_column(conn, "agent_run_log", "tokens_out", "INTEGER DEFAULT 0")?;
    add_column(conn, "agent_run_log", "cost_micros", "INTEGER DEFAULT 0")?;
    add_column(conn, "agent_run_log", "cost_priced", "INTEGER DEFAULT 0")?;
    add_column(conn, "agent_run_log", "plan_id", "TEXT DEFAULT ''")?;
    add_column(
        conn,
        "agent_run_log",
        "cancel_requested",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column(conn, "dms_config", "triggered_at", "TEXT DEFAULT ''")?;
    add_column(conn, "dms_config", "trigger_stage", "INTEGER DEFAULT 0")?;
    add_column(
        conn,
        "agent_skills",
        "builtin",
        "INTEGER NOT NULL DEFAULT 0",
    )?;

    // Seed only on the first schema application. A user may edit or delete a
    // built-in skill afterwards; do not resurrect it on every startup.
    let migrations_applied: i64 =
        conn.query_row("SELECT count(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    if migrations_applied == 0 {
        crate::builtin_skills::seed(conn)?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, checksum, applied_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            SCHEMA_VERSION,
            "v19_asset_details",
            checksum(),
            Utc::now().to_rfc3339()
        ],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO workspaces (id, name, created_at, settings_json)
         VALUES ('ws_default', '默认工作区', ?1, '{}')",
        params![Utc::now().to_rfc3339()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO dms_config
         (id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in, paused_until, triggered_at, trigger_stage)
         VALUES (1, 0, 30, 3, 7, 14, NULL, '', '', 0)",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn schema_is_v19_and_has_32_tables() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, EXPECTED_TABLES, "schema v19 必须正好 32 张表");
    }

    #[test]
    fn migration_ledger_records_v19() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        let name: String = conn
            .query_row(
                "SELECT name FROM schema_migrations WHERE version = 19",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "v19_asset_details");
    }

    #[test]
    fn seeds_builtin_skills_once() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        let builtin: i64 = conn
            .query_row(
                "SELECT count(*) FROM agent_skills WHERE builtin=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(builtin, 8);
        let enabled: i64 = conn
            .query_row(
                "SELECT count(*) FROM agent_skills WHERE builtin=1 AND enabled=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(enabled, 3);

        // Deleting a user-visible built-in skill must survive the next launch.
        conn.execute("DELETE FROM agent_skills WHERE name='年度复核与演练'", [])
            .unwrap();
        apply_schema(&conn).unwrap();
        let builtin_after: i64 = conn
            .query_row(
                "SELECT count(*) FROM agent_skills WHERE builtin=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            builtin_after, 7,
            "v19 seed must not resurrect deleted skills"
        );
    }

    #[test]
    fn apply_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        apply_schema(&conn).unwrap();
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
    #[test]
    fn v18_database_gains_asset_depth_columns_without_losing_rows() {
        let conn = Connection::open_in_memory().unwrap();
        let v18 = SCHEMA_SQL
            .replace("    ownership_share TEXT DEFAULT '',\n", "")
            .replace("    credential_location TEXT DEFAULT '',\n", "")
            .replace("    expires_at TEXT DEFAULT '',\n", "")
            .replace("    quantity TEXT DEFAULT '',\n", "")
            .replace("    unit TEXT DEFAULT '',\n", "")
            .replace("    value_status TEXT NOT NULL DEFAULT 'market',\n", "");
        conn.execute_batch(&v18).unwrap();
        conn.execute(
            "INSERT INTO platforms (name, category) VALUES ('示例银行', 'finance')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO digital_assets
             (platform_id, account_description, category, estimated_value, created_at, updated_at, status)
             VALUES (1, '旧版助学金卡', 'financial', 3200, '2026-11-01T00:00:00+08:00', '2026-11-01T00:00:00+08:00', 'active')",
            [],
        )
        .unwrap();

        apply_schema(&conn).unwrap();
        let row: (i64, String, String, String, String, String, String) = conn
            .query_row(
                "SELECT count(*), max(account_description), max(ownership_share), max(credential_location),
                        max(expires_at), max(quantity), max(value_status)
                 FROM digital_assets",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .unwrap();
        assert_eq!(row.0, 1);
        assert_eq!(row.1, "旧版助学金卡");
        assert_eq!(row.2, "");
        assert_eq!(row.3, "");
        assert_eq!(row.4, "");
        assert_eq!(row.5, "");
        assert_eq!(row.6, "market");
    }
}
