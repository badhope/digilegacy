//! Diagnostics snapshot (WP-14).
//!
//! Only structural metadata: schema, table counts, integrity, audit-chain state.
//! Never includes row values, credentials or profile data.

use crate::schema::EXPECTED_TABLES;
use chrono::Utc;
use serde_json::json;

use crate::{Result, Store};

pub const DIAGNOSTIC_VERSION: u32 = 1;

impl Store {
    pub fn diagnostics_json(&self) -> Result<String> {
        let schema_version = self.schema_version()?;
        let integrity = self.integrity_check()?;
        let audit = self.verify_audit_chain()?;
        let table_count = self.table_count()?;

        let counts = self.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )?;
            let names: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            let mut out = serde_json::Map::new();
            for name in names {
                // 表名来自 sqlite_master 自身，不是外部输入。
                let sql = format!("SELECT count(*) FROM \"{}\"", name.replace('"', ""));
                let n: i64 = conn.query_row(&sql, [], |r| r.get(0))?;
                out.insert(name, json!(n));
            }
            Ok(out)
        })?;

        let doc = json!({
            "version": DIAGNOSTIC_VERSION,
            "generated_at": Utc::now().to_rfc3339(),
            "store": {
                "path": self.path().to_string_lossy(),
                "state": format!("{:?}", self.state()),
                "schema_version": schema_version,
                "expected_tables": EXPECTED_TABLES,
                "table_count": table_count,
            },
            "integrity": integrity,
            "audit": {
                "valid": audit.valid,
                "checked": audit.checked,
                "broken_at": audit.broken_at,
            },
            "rows": counts,
        });
        serde_json::to_string_pretty(&doc)
            .map_err(|e| crate::StoreError::State(format!("诊断序列化失败: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_contains_structure_not_values() {
        let store = Store::open_in_memory(None).unwrap();
        store.append_audit("test", "ws_default", "ok", "").unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO app_settings (key, value, updated_at) VALUES ('secret_key','secret_value','now')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let json = store.diagnostics_json().unwrap();
        assert!(json.contains(&format!("\"schema_version\": {}", crate::SCHEMA_VERSION)));
        assert!(json.contains("\"valid\": true"));
        assert!(json.contains("\"table_count\": 28"));
        assert!(!json.contains("secret_value"), "诊断包不得包含数据值");
        assert!(!json.contains("secret_key"), "诊断包不得包含数据键");
    }
}
