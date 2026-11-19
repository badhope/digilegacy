//! Append-only audit hash chain (FND-05 / WP-14).
//!
//! Each row hashes the previous hash plus the public fields. Tampering with a
//! historical row breaks every later hash, so `verify_chain` can pinpoint the
//! first bad id without re-signing anything.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{Result, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: i64,
    pub at: String,
    pub kind: String,
    pub subject: String,
    pub outcome: String,
    pub detail_enc: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditVerification {
    pub valid: bool,
    pub checked: i64,
    pub broken_at: Option<i64>,
}

fn digest(prev: &str, at: &str, kind: &str, subject: &str, outcome: &str, detail: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev.as_bytes());
    h.update(b"\n");
    h.update(at.as_bytes());
    h.update(b"\n");
    h.update(kind.as_bytes());
    h.update(b"\n");
    h.update(subject.as_bytes());
    h.update(b"\n");
    h.update(outcome.as_bytes());
    h.update(b"\n");
    h.update(detail.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

impl Store {
    /// Append one audit row. `detail_enc` may be empty or a vault-encrypted value.
    pub fn append_audit(
        &self,
        kind: &str,
        subject: &str,
        outcome: &str,
        detail_enc: &str,
    ) -> Result<AuditEntry> {
        let at = Utc::now().to_rfc3339();
        self.write(|conn| {
            let prev: String = conn
                .query_row(
                    "SELECT hash FROM audit_log ORDER BY id DESC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let hash = digest(&prev, &at, kind, subject, outcome, detail_enc);
            conn.execute(
                "INSERT INTO audit_log (at, kind, subject, outcome, detail_enc, prev_hash, hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![at, kind, subject, outcome, detail_enc, prev, hash],
            )?;
            let id = conn.last_insert_rowid();
            Ok(AuditEntry {
                id,
                at,
                kind: kind.to_string(),
                subject: subject.to_string(),
                outcome: outcome.to_string(),
                detail_enc: detail_enc.to_string(),
                prev_hash: prev,
                hash,
            })
        })
    }

    /// Recompute the chain from id=1 upward. Read-only and non-destructive.
    pub fn verify_audit_chain(&self) -> Result<AuditVerification> {
        self.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, at, kind, subject, outcome, detail_enc, prev_hash, hash
                 FROM audit_log ORDER BY id ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(AuditEntry {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    kind: r.get(2)?,
                    subject: r.get(3)?,
                    outcome: r.get(4)?,
                    detail_enc: r.get(5)?,
                    prev_hash: r.get(6)?,
                    hash: r.get(7)?,
                })
            })?;
            let mut prev = String::new();
            let mut checked = 0i64;
            let mut broken_at = None;
            for row in rows {
                let entry = row?;
                checked += 1;
                let expected = digest(
                    &prev,
                    &entry.at,
                    &entry.kind,
                    &entry.subject,
                    &entry.outcome,
                    &entry.detail_enc,
                );
                if entry.prev_hash != prev || entry.hash != expected {
                    broken_at = Some(entry.id);
                    break;
                }
                prev = entry.hash;
            }
            Ok(AuditVerification {
                valid: broken_at.is_none(),
                checked,
                broken_at,
            })
        })
    }

    /// Read one audit page, newest first, optionally filtered by kind.
    pub fn audit_page(
        &self,
        limit: i64,
        cursor: Option<i64>,
        kind: Option<&str>,
    ) -> Result<Vec<AuditEntry>> {
        let limit = limit.clamp(1, 100);
        self.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, at, kind, subject, outcome, detail_enc, prev_hash, hash
                 FROM audit_log
                 WHERE (?1 IS NULL OR id < ?1)
                   AND (?2 IS NULL OR kind = ?2)
                 ORDER BY id DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![cursor, kind, limit], |r| {
                Ok(AuditEntry {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    kind: r.get(2)?,
                    subject: r.get(3)?,
                    outcome: r.get(4)?,
                    detail_enc: r.get(5)?,
                    prev_hash: r.get(6)?,
                    hash: r.get(7)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn audit_count(&self, kind: Option<&str>) -> Result<i64> {
        self.read(|conn| {
            let count = conn.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE (?1 IS NULL OR kind = ?1)",
                rusqlite::params![kind],
                |r| r.get(0),
            )?;
            Ok(count)
        })
    }

    pub fn audit_entries(&self, limit: i64) -> Result<Vec<AuditEntry>> {
        self.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, at, kind, subject, outcome, detail_enc, prev_hash, hash
                 FROM audit_log ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], |r| {
                Ok(AuditEntry {
                    id: r.get(0)?,
                    at: r.get(1)?,
                    kind: r.get(2)?,
                    subject: r.get(3)?,
                    outcome: r.get(4)?,
                    detail_enc: r.get(5)?,
                    prev_hash: r.get(6)?,
                    hash: r.get(7)?,
                })
            })?;
            rows.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_valid_after_appends() {
        let store = Store::open_in_memory(None).unwrap();
        let a = store
            .append_audit("vault.unlock", "ws_default", "ok", "")
            .unwrap();
        let b = store
            .append_audit("asset.create", "asset:1", "ok", "")
            .unwrap();
        let c = store
            .append_audit("asset.update", "asset:1", "ok", "enc:v2:x")
            .unwrap();
        assert_eq!(a.prev_hash, "");
        assert_eq!(b.prev_hash, a.hash);
        assert_eq!(c.prev_hash, b.hash);
        let v = store.verify_audit_chain().unwrap();
        assert!(v.valid);
        assert_eq!(v.checked, 3);
        assert_eq!(v.broken_at, None);
        assert_eq!(store.audit_entries(2).unwrap().len(), 2);

        let page = store.audit_page(2, None, None).unwrap();
        assert_eq!(
            page.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![3, 2]
        );
        let older = store.audit_page(2, Some(2), None).unwrap();
        assert_eq!(
            older.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![1]
        );
        let assets = store.audit_page(10, None, Some("asset.create")).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(store.audit_count(Some("asset.update")).unwrap(), 1);
    }

    #[test]
    fn tampering_is_detected_at_first_bad_row() {
        let store = Store::open_in_memory(None).unwrap();
        store.append_audit("a", "s", "ok", "").unwrap();
        store.append_audit("b", "s", "ok", "").unwrap();
        store.append_audit("c", "s", "ok", "").unwrap();
        store
            .write(|conn| {
                conn.execute("UPDATE audit_log SET subject='tampered' WHERE id=2", [])?;
                Ok(())
            })
            .unwrap();
        let v = store.verify_audit_chain().unwrap();
        assert!(!v.valid);
        assert_eq!(v.broken_at, Some(2));
        assert_eq!(v.checked, 2);
    }
}
