//! AUD-01: read-only audit page over the append-only hash chain.
//!
//! The store owns the chain and verification; this module only shapes the
//! query contract for HTTP/CLI/Agent callers.

use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::DomainError;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditItem {
    pub id: String,
    pub at: String,
    pub kind: String,
    pub subject: String,
    pub outcome: String,
    pub hash: String,
    pub prev_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub items: Vec<AuditItem>,
    pub next_cursor: Option<String>,
    pub total: i64,
}

pub fn page(store: &Store, input: &ListInput) -> Result<AuditPage, DomainError> {
    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let kind = input
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let cursor = match input.cursor.as_deref() {
        Some(value) if !value.trim().is_empty() => Some(
            value
                .parse::<i64>()
                .map_err(|_| DomainError::Invalid(format!("审计游标不是数字: {value}")))?,
        ),
        _ => None,
    };
    let entries = store
        .audit_page(limit, cursor, kind)
        .map_err(DomainError::from)?;
    let total = store.audit_count(kind).map_err(DomainError::from)?;
    let next_cursor = if entries.len() as i64 == limit {
        entries.last().map(|entry| entry.id.to_string())
    } else {
        None
    };
    Ok(AuditPage {
        items: entries
            .into_iter()
            .map(|entry| AuditItem {
                id: entry.id.to_string(),
                at: entry.at,
                kind: entry.kind,
                subject: entry.subject,
                outcome: entry.outcome,
                hash: entry.hash,
                prev_hash: entry.prev_hash,
            })
            .collect(),
        next_cursor,
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_page_filters_by_kind_and_cursor() {
        let store = Store::open_in_memory(None).unwrap();
        store.append_audit("asset.create", "1", "ok", "").unwrap();
        store.append_audit("asset.update", "1", "ok", "").unwrap();
        store
            .append_audit("policy.provenance.set", "1", "ok", "")
            .unwrap();

        let first = page(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(2),
                kind: None,
            },
        )
        .unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.total, 3);
        assert_eq!(first.next_cursor.as_deref(), Some("2"));

        let filtered = page(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(20),
                kind: Some("asset.update".to_string()),
            },
        )
        .unwrap();
        assert_eq!(filtered.items.len(), 1);
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].kind, "asset.update");
    }
}
