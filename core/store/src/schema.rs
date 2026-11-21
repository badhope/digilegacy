//! Schema v18: built-in skills and MCP catalog metadata.

use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: i64 = 18;
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

    // Seed only when v18 first runs. A user may edit or delete a built-in skill
    // afterwards; do not resurrect it on every startup.
    let v18_applied: i64 = conn.query_row(
        "SELECT count(*) FROM schema_migrations WHERE version = 18",
        [],
        |row| row.get(0),
    )?;
    if v18_applied == 0 {
        crate::builtin_skills::seed(conn)?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, checksum, applied_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            SCHEMA_VERSION,
            "v18_builtin_resources",
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
    fn schema_is_v18_and_has_32_tables() {
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
        assert_eq!(count, EXPECTED_TABLES, "schema v18 必须正好 32 张表");
    }

    #[test]
    fn migration_ledger_records_v18() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).unwrap();
        let name: String = conn
            .query_row(
                "SELECT name FROM schema_migrations WHERE version = 18",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "v18_builtin_resources");
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
            "v18 seed must not resurrect deleted skills"
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
}
