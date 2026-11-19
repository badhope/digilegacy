//! Schema v15: notification outbox, additive migrations and ledger.

use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: i64 = 15;
pub const SCHEMA_SQL: &str = include_str!("schema.sql");
pub const EXPECTED_TABLES: i64 = 30;

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

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, checksum, applied_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            SCHEMA_VERSION,
            "v15_notification_outbox",
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
    fn schema_is_v15_and_has_30_tables() {
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
        assert_eq!(count, EXPECTED_TABLES, "schema v15 必须正好 30 张表");
    }

    #[test]
    fn migration_ledger_records_v15() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        let name: String = conn
            .query_row(
                "SELECT name FROM schema_migrations WHERE version = 15",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "v15_notification_outbox");
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
}
