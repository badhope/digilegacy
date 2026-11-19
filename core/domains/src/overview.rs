//! DOM-00 overview: a read-only projection assembled from the store.
//!
//! Counts are computed in SQL; the projection never decrypts credentials or
//! profile fields. Add more modules here as their domains land.

use rusqlite::OptionalExtension;
use serde::Serialize;

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub id: String,
    pub label: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub at: String,
    pub title: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub workspace: String,
    pub completion: i64,
    pub assets_total: i64,
    pub heirs_total: i64,
    pub approvals_pending: i64,
    pub alerts: i64,
    pub modules: Vec<ModuleStatus>,
    pub activity: Vec<ActivityItem>,
}

fn count(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |r| r.get(0))
}

pub fn get(store: &Store) -> Result<Overview, DomainError> {
    let mut overview = store
        .read(|conn| {
            let assets_total = count(
                conn,
                "SELECT count(*) FROM digital_assets WHERE status='active'",
            )?;
            let heirs_total = count(conn, "SELECT count(*) FROM heirs")?;
            let approvals_pending = count(
                conn,
                "SELECT count(*) FROM agent_approvals WHERE status='pending'",
            )?;
            let profile_exists = conn
                .query_row(
                    "SELECT count(*) FROM user_profile WHERE id=1 AND coalesce(full_name,'') != ''",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
                .unwrap_or(0)
                > 0;
            let trigger_enabled = conn
                .query_row("SELECT enabled FROM dms_config WHERE id=1", [], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?
                .unwrap_or(0)
                > 0;

            let mut done = 0;
            if profile_exists {
                done += 25;
            }
            if heirs_total > 0 {
                done += 25;
            }
            if assets_total > 0 {
                done += 25;
            }
            if trigger_enabled {
                done += 25;
            }
            let alerts =
                approvals_pending + i64::from(!profile_exists) + i64::from(heirs_total == 0);

            Ok(Overview {
                workspace: "默认工作区".to_string(),
                completion: done,
                assets_total,
                heirs_total,
                approvals_pending,
                alerts,
                modules: vec![
                    ModuleStatus {
                        id: "assets".to_string(),
                        label: "资产".to_string(),
                        status: "ready".to_string(),
                    },
                    ModuleStatus {
                        id: "plan".to_string(),
                        label: "规划".to_string(),
                        status: "ready".to_string(),
                    },
                    ModuleStatus {
                        id: "trigger".to_string(),
                        label: "触发".to_string(),
                        status: "ready".to_string(),
                    },
                    ModuleStatus {
                        id: "policy".to_string(),
                        label: "政策".to_string(),
                        status: "ready".to_string(),
                    },
                    ModuleStatus {
                        id: "deliver".to_string(),
                        label: "交付".to_string(),
                        status: "ready".to_string(),
                    },
                    ModuleStatus {
                        id: "agent".to_string(),
                        label: "Agent".to_string(),
                        status: "planned".to_string(),
                    },
                ],
                activity: Vec::new(),
            })
        })
        .map_err(DomainError::from)?;

    if let Ok(entries) = store.audit_entries(8) {
        overview.activity = entries
            .into_iter()
            .map(|e| ActivityItem {
                at: e.at,
                title: if e.subject.is_empty() {
                    e.kind.clone()
                } else {
                    format!("{} · {}", e.kind, e.subject)
                },
                kind: e.kind,
            })
            .collect();
    }
    Ok(overview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_counts_and_activity() {
        let store = Store::open_in_memory(None).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO platforms (id,name,category) VALUES (0,'未分类','unknown')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO digital_assets (platform_id, category, created_at, updated_at) VALUES (0,'account','now','now')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO heirs (name, is_executor, created_at, updated_at) VALUES ('李四',0,'now','now')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        store
            .append_audit("asset.create", "asset:1", "ok", "")
            .unwrap();
        let overview = get(&store).unwrap();
        assert_eq!(overview.assets_total, 1);
        assert_eq!(overview.heirs_total, 1);
        assert_eq!(overview.completion, 50);
        assert_eq!(overview.activity.len(), 1);
    }
}
