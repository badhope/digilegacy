//! AGT-03: human approval queue for Agent writes.
//!
//! The table is authoritative; this module owns state transitions and never
//! executes the approved tool itself. Execution stays in the Agent runtime.

use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: String,
    pub run_id: String,
    pub tool: String,
    pub object_key: String,
    pub diff: String,
    pub status: String,
    pub reason: String,
    pub created_at: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPage {
    pub items: Vec<Approval>,
    pub pending: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecideInput {
    pub id: String,
    pub decision: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub background: bool,
}

fn row_to_approval(row: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    Ok(Approval {
        id: row.get(0)?,
        run_id: row.get(1)?,
        tool: row.get(2)?,
        object_key: row.get(3)?,
        diff: row.get(4)?,
        status: row.get(5)?,
        reason: row.get(6)?,
        created_at: row.get(7)?,
        decided_at: row.get(8)?,
    })
}

pub fn get(store: &Store, id: &str) -> Result<Approval, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, run_id, tool, object_key, diff_json, status, reason, created_at, decided_at
                 FROM agent_approvals WHERE id=?1",
                [id],
                row_to_approval,
            )
            .optional()
        })
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::NotFound(format!("审批记录不存在: {id}")))
}

pub fn list(store: &Store, input: &ListInput) -> Result<ApprovalPage, DomainError> {
    let status = input.status.as_deref().unwrap_or("pending").trim();
    let status = if status.is_empty() { "pending" } else { status };
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, run_id, tool, object_key, diff_json, status, reason, created_at, decided_at
                 FROM agent_approvals
                 WHERE (?1 = 'all' OR status = ?1)
                 ORDER BY created_at DESC, id DESC",
            )?;
            let items = stmt
                .query_map([status], row_to_approval)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let pending: i64 = conn.query_row(
                "SELECT count(*) FROM agent_approvals WHERE status='pending'",
                [],
                |r| r.get(0),
            )?;
            Ok(ApprovalPage { items, pending })
        })
        .map_err(DomainError::from)
}

pub fn decide(store: &Store, input: &DecideInput) -> Result<Approval, DomainError> {
    let status = match input.decision.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        other => return Err(DomainError::Invalid(format!("未知审批决定: {other}"))),
    };
    let reason = input.reason.trim().to_string();
    if reason.chars().count() > 500 {
        return Err(DomainError::Invalid("审批理由不能超过 500 字".to_string()));
    }
    let existing = store
        .read(|conn| {
            conn.query_row(
                "SELECT id, run_id, tool, object_key, diff_json, status, reason, created_at, decided_at
                 FROM agent_approvals WHERE id=?1",
                [&input.id],
                row_to_approval,
            )
            .optional()
        })
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::NotFound(format!("审批记录不存在: {}", input.id)))?;
    if existing.status != "pending" {
        return Err(DomainError::Conflict(format!(
            "审批记录已是 {} 状态",
            existing.status
        )));
    }
    let decided_at = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "UPDATE agent_approvals
                 SET status=?2, reason=?3, decided_at=?4
                 WHERE id=?1 AND status='pending'",
                rusqlite::params![input.id, status, reason, decided_at],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, run_id, tool, object_key, diff_json, status, reason, created_at, decided_at
                 FROM agent_approvals WHERE id=?1",
                [&input.id],
                row_to_approval,
            )
        })
        .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(store: &Store, id: &str, status: &str) {
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO agent_approvals (id, run_id, tool, object_key, diff_json, status, created_at)
                     VALUES (?1, 'run_1', 'assets.update', '3', '{\"value\":\"4200\"}', ?2, 't1')",
                    rusqlite::params![id, status],
                )?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn approves_pending_and_rejects_second_decision() {
        let store = Store::open_in_memory(None).unwrap();
        seed(&store, "appr_1", "pending");
        seed(&store, "appr_2", "approved");
        let page = list(
            &store,
            &ListInput {
                status: Some("pending".to_string()),
            },
        )
        .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.pending, 1);

        let approved = decide(
            &store,
            &DecideInput {
                id: "appr_1".to_string(),
                decision: "approve".to_string(),
                reason: "核对无误".to_string(),
                background: false,
            },
        )
        .unwrap();
        assert_eq!(approved.status, "approved");
        assert_eq!(approved.reason, "核对无误");
        assert!(!approved.decided_at.is_empty());

        assert!(matches!(
            decide(
                &store,
                &DecideInput {
                    id: "appr_1".to_string(),
                    decision: "reject".to_string(),
                    reason: String::new(),
                    background: false,
                }
            ),
            Err(DomainError::Conflict(_))
        ));
    }
}
