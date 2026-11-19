//! DOM-02 policy sync: remote policies become candidate diffs first.
//!
//! Pulling a remote feed never mutates `policies`. A human must approve a
//! candidate, and approval records the content but deliberately leaves
//! provenance `unverified` so the source check remains a separate action.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use digilegacy_store::Store;

use crate::assets::DomainError;
use crate::policy::{self, Policy, SeedPolicy};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffField {
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub platform_id: String,
    pub name: String,
    pub fetched_at: String,
    pub source: String,
    pub status: String,
    pub hash: String,
    pub changed_fields: Vec<String>,
    pub diff: Vec<DiffField>,
    pub reviewed_at: String,
    pub reviewed_by: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePage {
    pub items: Vec<Candidate>,
    pub next_cursor: Option<String>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct PendingPolicy {
    pub policy: SeedPolicy,
    pub payload_enc: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueStats {
    pub fetched: i64,
    pub changed: i64,
    pub queued: i64,
    pub conflict: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub source: String,
    pub cursor: String,
    pub etag: String,
    pub last_sync_at: String,
    pub last_status: String,
    pub last_error: String,
}

fn value_string(value: impl ToString) -> String {
    value.to_string()
}

fn remote_fields(policy: &SeedPolicy) -> Vec<(&'static str, String)> {
    vec![
        ("name", policy.name.clone()),
        ("name_en", policy.name_en.clone()),
        ("category", policy.category.clone()),
        ("subtype", policy.subtype.clone()),
        ("has_property", value_string(policy.has_property)),
        ("has_personality", value_string(policy.has_personality)),
        ("inheritable", value_string(policy.inheritable)),
        ("required_documents", policy.required_documents.clone()),
        ("process_description", policy.process_description.clone()),
        ("estimated_difficulty", policy.estimated_difficulty.clone()),
        ("official_policy_url", policy.official_policy_url.clone()),
        ("estimated_time_days", policy.estimated_time_days.clone()),
        ("notes", policy.notes.clone()),
        ("data_source", policy.data_source.clone()),
        ("inheritance_form_url", policy.inheritance_form_url.clone()),
        ("monetization_path", policy.monetization_path.clone()),
        ("legal_template_ids", policy.legal_template_ids.clone()),
        ("risk_warnings", policy.risk_warnings.clone()),
        ("tags", policy.tags.clone()),
    ]
}

fn current_fields(policy: &Policy) -> Vec<(&'static str, String)> {
    vec![
        ("name", policy.name.clone()),
        ("name_en", policy.name_en.clone()),
        ("category", policy.category.clone()),
        ("subtype", policy.subtype.clone()),
        ("has_property", value_string(policy.has_property)),
        ("has_personality", value_string(policy.has_personality)),
        ("inheritable", value_string(policy.inheritable)),
        ("required_documents", policy.required_documents.clone()),
        ("process_description", policy.process_description.clone()),
        ("estimated_difficulty", policy.estimated_difficulty.clone()),
        ("official_policy_url", policy.official_policy_url.clone()),
        ("estimated_time_days", policy.estimated_time_days.clone()),
        ("notes", policy.notes.clone()),
        ("data_source", policy.data_source.clone()),
        ("inheritance_form_url", policy.inheritance_form_url.clone()),
        ("monetization_path", policy.monetization_path.clone()),
        ("legal_template_ids", policy.legal_template_ids.clone()),
        ("risk_warnings", policy.risk_warnings.clone()),
        ("tags", policy.tags.clone()),
    ]
}

fn diff_fields(current: Option<&Policy>, remote: &SeedPolicy) -> Vec<DiffField> {
    let next = remote_fields(remote);
    let previous = current.map(current_fields).unwrap_or_default();
    let mut diff = Vec::new();
    for (field, after) in next {
        let before = previous
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        if current.is_none() {
            if after.is_empty() || after == "false" {
                continue;
            }
        } else if before == after {
            continue;
        }
        diff.push(DiffField {
            field: field.to_string(),
            before,
            after,
        });
    }
    diff
}

fn content_hash(policy: &SeedPolicy) -> Result<String, DomainError> {
    let json = serde_json::to_string(policy)
        .map_err(|error| DomainError::Invalid(format!("序列化远程政策失败: {error}")))?;
    let digest = Sha256::digest(json.as_bytes());
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn validate(policy: &SeedPolicy) -> Result<(), DomainError> {
    if policy.id <= 0 {
        return Err(DomainError::Invalid("远程政策 id 必须为正整数".to_string()));
    }
    if policy.name.trim().is_empty() {
        return Err(DomainError::Invalid(format!(
            "远程政策 {} 缺少 name",
            policy.id
        )));
    }
    if policy.category.trim().is_empty() {
        return Err(DomainError::Invalid(format!(
            "远程政策 {} 缺少 category",
            policy.id
        )));
    }
    Ok(())
}

fn candidate_exists(store: &Store, platform_id: i64, hash: &str) -> Result<bool, DomainError> {
    store
        .read(|conn| {
            let found: Option<i64> = conn
                .query_row(
                    "SELECT id FROM policy_candidates
                     WHERE platform_id = ?1 AND instr(diff_json, ?2) > 0
                     ORDER BY id DESC LIMIT 1",
                    params![platform_id, hash],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(found.is_some())
        })
        .map_err(DomainError::from)
}

/// Queue remote policy versions. The caller supplies the encrypted payload
/// because only the application layer owns the connection config key.
pub fn enqueue(
    store: &Store,
    source: &str,
    items: Vec<PendingPolicy>,
) -> Result<EnqueueStats, DomainError> {
    let fetched = items.len() as i64;
    let mut stats = EnqueueStats {
        fetched,
        ..EnqueueStats::default()
    };
    let mut prepared = Vec::new();
    for item in items {
        validate(&item.policy)?;
        let current = policy::find(store, item.policy.id)?;
        let diff = diff_fields(current.as_ref(), &item.policy);
        if diff.is_empty() {
            stats.skipped += 1;
            continue;
        }
        let hash = content_hash(&item.policy)?;
        if candidate_exists(store, item.policy.id, &hash)? {
            stats.skipped += 1;
            continue;
        }
        let conflict = current
            .as_ref()
            .is_some_and(|policy| policy.provenance.status != "unverified");
        let status = if conflict { "conflict" } else { "pending" };
        if conflict {
            stats.conflict += 1;
        }
        stats.changed += 1;
        stats.queued += 1;
        let diff_json = serde_json::json!({
            "platformId": item.policy.id.to_string(),
            "name": item.policy.name,
            "hash": hash,
            "fields": diff,
        })
        .to_string();
        prepared.push((
            item.policy.id,
            item.policy.name,
            source.to_string(),
            item.payload_enc,
            diff_json,
            status.to_string(),
        ));
    }

    if prepared.is_empty() {
        return Ok(stats);
    }
    let fetched_at = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            let tx = conn.transaction()?;
            for (platform_id, _name, source, payload_enc, diff_json, status) in &prepared {
                tx.execute(
                    "INSERT INTO policy_candidates
                     (platform_id, fetched_at, source, payload_enc, diff_json, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        platform_id,
                        fetched_at,
                        source,
                        payload_enc,
                        diff_json,
                        status
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    Ok(stats)
}

fn parse_candidate(row: &rusqlite::Row<'_>) -> rusqlite::Result<Candidate> {
    let diff_json: String = row.get(4)?;
    let root: serde_json::Value = serde_json::from_str(&diff_json).unwrap_or_default();
    let name = root
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let hash = root
        .get("hash")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let diff: Vec<DiffField> = root
        .get("fields")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let changed_fields = diff.iter().map(|item| item.field.clone()).collect();
    Ok(Candidate {
        id: row.get::<_, i64>(0)?.to_string(),
        platform_id: row.get::<_, i64>(1)?.to_string(),
        name,
        fetched_at: row.get(2)?,
        source: row.get(3)?,
        status: row.get(5)?,
        hash,
        changed_fields,
        diff,
        reviewed_at: row.get(6)?,
        reviewed_by: row.get(7)?,
    })
}

pub fn list(store: &Store, input: &ListInput) -> Result<CandidatePage, DomainError> {
    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    let offset = input
        .cursor
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let status = input
        .status
        .as_deref()
        .unwrap_or("pending")
        .trim()
        .to_string();
    let status = if matches!(
        status.as_str(),
        "all" | "pending" | "conflict" | "approved" | "rejected"
    ) {
        status
    } else {
        "pending".to_string()
    };
    store
        .read(|conn| {
            let total: i64 = conn.query_row(
                "SELECT count(*) FROM policy_candidates WHERE (?1 = 'all' OR status = ?1)",
                [&status],
                |row| row.get(0),
            )?;
            let mut stmt = conn.prepare(
                "SELECT id, platform_id, fetched_at, source, diff_json, status, reviewed_at, reviewed_by
                 FROM policy_candidates
                 WHERE (?1 = 'all' OR status = ?1)
                 ORDER BY fetched_at DESC, id DESC LIMIT ?2 OFFSET ?3",
            )?;
            let items = stmt
                .query_map(params![status, limit, offset], parse_candidate)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let next_cursor = if offset + items.len() as i64 >= total {
                None
            } else {
                Some((offset + items.len() as i64).to_string())
            };
            Ok(CandidatePage {
                items,
                next_cursor,
                total,
            })
        })
        .map_err(DomainError::from)
}

pub fn payload_enc(store: &Store, id: &str) -> Result<(String, SeedPolicyHint), DomainError> {
    let id: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("候选 id 不是数字: {id}")))?;
    let row = store
        .read(|conn| {
            conn.query_row(
                "SELECT payload_enc, platform_id, status FROM policy_candidates WHERE id = ?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        SeedPolicyHint {
                            platform_id: row.get(1)?,
                            status: row.get(2)?,
                        },
                    ))
                },
            )
            .optional()
        })
        .map_err(DomainError::from)?;
    row.ok_or_else(|| DomainError::NotFound(format!("政策候选 {id} 不存在")))
}

#[derive(Debug, Clone)]
pub struct SeedPolicyHint {
    pub platform_id: i64,
    pub status: String,
}

fn reviewed_candidate(store: &Store, id: i64) -> Result<Candidate, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, platform_id, fetched_at, source, diff_json, status, reviewed_at, reviewed_by
                 FROM policy_candidates WHERE id = ?1",
                [id],
                parse_candidate,
            )
        })
        .map_err(DomainError::from)
}

pub fn approve(
    store: &Store,
    id: &str,
    reviewer: &str,
    remote: &SeedPolicy,
) -> Result<Candidate, DomainError> {
    let id: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("候选 id 不是数字: {id}")))?;
    let (_payload, hint) = payload_enc(store, &id.to_string())?;
    if !matches!(hint.status.as_str(), "pending" | "conflict") {
        return Err(DomainError::Conflict(format!(
            "候选 {id} 已是 {}，不能重复审核",
            hint.status
        )));
    }
    validate(remote)?;
    if remote.id != hint.platform_id {
        return Err(DomainError::Invalid(format!(
            "候选平台 {} 与载荷平台 {} 不一致",
            hint.platform_id, remote.id
        )));
    }
    let now = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            let tx = conn.transaction()?;
            let created_at: Option<String> = tx
                .query_row("SELECT created_at FROM policies WHERE id = ?1", [remote.id], |row| {
                    row.get(0)
                })
                .optional()?;
            let created_at = created_at.unwrap_or_else(|| now.clone());
            tx.execute(
                "INSERT INTO policies
                 (id, name, name_en, category, subtype, has_property, has_personality, inheritable,
                  required_documents, process_description, estimated_difficulty, official_policy_url,
                  estimated_time_days, notes, data_source, inheritance_form_url, monetization_path,
                  legal_template_ids, risk_warnings, tags, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
                 ON CONFLICT(id) DO UPDATE SET
                   name=excluded.name, name_en=excluded.name_en, category=excluded.category,
                   subtype=excluded.subtype, has_property=excluded.has_property,
                   has_personality=excluded.has_personality, inheritable=excluded.inheritable,
                   required_documents=excluded.required_documents,
                   process_description=excluded.process_description,
                   estimated_difficulty=excluded.estimated_difficulty,
                   official_policy_url=excluded.official_policy_url,
                   estimated_time_days=excluded.estimated_time_days,
                   notes=excluded.notes, data_source=excluded.data_source,
                   inheritance_form_url=excluded.inheritance_form_url,
                   monetization_path=excluded.monetization_path,
                   legal_template_ids=excluded.legal_template_ids,
                   risk_warnings=excluded.risk_warnings, tags=excluded.tags,
                   updated_at=excluded.updated_at",
                params![
                    remote.id,
                    remote.name,
                    remote.name_en,
                    remote.category,
                    remote.subtype,
                    i64::from(remote.has_property),
                    i64::from(remote.has_personality),
                    i64::from(remote.inheritable),
                    remote.required_documents,
                    remote.process_description,
                    remote.estimated_difficulty,
                    remote.official_policy_url,
                    remote.estimated_time_days,
                    remote.notes,
                    remote.data_source,
                    remote.inheritance_form_url,
                    remote.monetization_path,
                    remote.legal_template_ids,
                    remote.risk_warnings,
                    remote.tags,
                    created_at,
                    now,
                ],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO policy_provenance
                 (platform_id, status, source_url, verified_at, verified_by, evidence_hash, note)
                 VALUES (?1, 'unverified', ?2, '', '', '', ?3)",
                params![
                    remote.id,
                    remote.official_policy_url,
                    "同步候选已批准；内容已更新，来源需重新核对"
                ],
            )?;
            tx.execute(
                "UPDATE policy_candidates SET status = 'approved', reviewed_at = ?1, reviewed_by = ?2
                 WHERE id = ?3",
                params![now, reviewer, id],
            )?;
            tx.commit()?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    reviewed_candidate(store, id)
}

pub fn reject(store: &Store, id: &str, reviewer: &str) -> Result<Candidate, DomainError> {
    let id: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("候选 id 不是数字: {id}")))?;
    let (_, hint) = payload_enc(store, &id.to_string())?;
    if !matches!(hint.status.as_str(), "pending" | "conflict") {
        return Err(DomainError::Conflict(format!(
            "候选 {id} 已是 {}，不能重复审核",
            hint.status
        )));
    }
    let now = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "UPDATE policy_candidates SET status = 'rejected', reviewed_at = ?1, reviewed_by = ?2
                 WHERE id = ?3",
                params![now, reviewer, id],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    reviewed_candidate(store, id)
}

pub fn state(store: &Store) -> Result<SyncState, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT source, cursor, etag, last_sync_at, last_status, last_error
                 FROM policy_sync_state WHERE id = 1",
                [],
                |row| {
                    Ok(SyncState {
                        source: row.get(0)?,
                        cursor: row.get(1)?,
                        etag: row.get(2)?,
                        last_sync_at: row.get(3)?,
                        last_status: row.get(4)?,
                        last_error: row.get(5)?,
                    })
                },
            )
            .optional()?
            .map(Ok)
            .unwrap_or_else(|| Ok(SyncState::default()))
        })
        .map_err(DomainError::from)
}

pub fn record_state(
    store: &Store,
    source: &str,
    cursor: &str,
    etag: &str,
    status: &str,
    error: &str,
) -> Result<SyncState, DomainError> {
    let now = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            conn.execute(
                "INSERT INTO policy_sync_state (id, source, cursor, etag, last_sync_at, last_status, last_error)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   source=excluded.source, cursor=excluded.cursor, etag=excluded.etag,
                   last_sync_at=excluded.last_sync_at, last_status=excluded.last_status,
                   last_error=excluded.last_error",
                params![source, cursor, etag, now, status, error],
            )?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    state(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ProvenanceInput;

    fn store() -> Store {
        let store = Store::open_in_memory(None).unwrap();
        policy::seed_if_empty(&store).unwrap();
        store
    }

    fn remote(name: &str) -> SeedPolicy {
        SeedPolicy {
            id: 1,
            name: name.to_string(),
            name_en: "Alipay".to_string(),
            category: "支付金融".to_string(),
            subtype: "移动支付".to_string(),
            has_property: true,
            has_personality: false,
            inheritable: true,
            required_documents: "死亡证明".to_string(),
            process_description: "远程新流程".to_string(),
            estimated_difficulty: "中等".to_string(),
            official_policy_url: "https://example.com/alipay".to_string(),
            estimated_time_days: "30 天".to_string(),
            notes: "远程更新".to_string(),
            data_source: "远程来源".to_string(),
            inheritance_form_url: String::new(),
            monetization_path: "远程变现".to_string(),
            legal_template_ids: "a,b".to_string(),
            risk_warnings: "远程风险".to_string(),
            tags: "需公证".to_string(),
        }
    }

    #[test]
    fn enqueue_diff_is_idempotent_and_does_not_mutate_policy() {
        let store = store();
        let stats = enqueue(
            &store,
            "https://example.com/v1/policies",
            vec![PendingPolicy {
                policy: remote("支付宝"),
                payload_enc: "plain:{}".to_string(),
            }],
        )
        .unwrap();
        assert_eq!(stats.fetched, 1);
        assert_eq!(stats.queued, 1);
        assert_eq!(stats.changed, 1);
        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: None,
                status: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].platform_id, "1");
        assert!(page.items[0].changed_fields.contains(&"notes".to_string()));
        let current = policy::get(&store, "1").unwrap();
        assert_eq!(current.notes, "5万以下走简易继承程序（2025银行新规）");
        let again = enqueue(
            &store,
            "https://example.com/v1/policies",
            vec![PendingPolicy {
                policy: remote("支付宝"),
                payload_enc: "plain:{}".to_string(),
            }],
        )
        .unwrap();
        assert_eq!(again.queued, 0);
        assert_eq!(again.skipped, 1);
    }

    #[test]
    fn approve_applies_content_but_keeps_provenance_unverified() {
        let store = store();
        enqueue(
            &store,
            "https://example.com/v1/policies",
            vec![PendingPolicy {
                policy: remote("支付宝"),
                payload_enc: "plain:{}".to_string(),
            }],
        )
        .unwrap();
        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: None,
                status: None,
            },
        )
        .unwrap();
        let id = page.items[0].id.clone();
        let approved = approve(&store, &id, "local-user", &remote("支付宝")).unwrap();
        assert_eq!(approved.status, "approved");
        let updated = policy::get(&store, "1").unwrap();
        assert_eq!(updated.notes, "远程更新");
        assert_eq!(updated.provenance.status, "unverified");
        assert_eq!(
            reject(&store, &id, "local-user").unwrap_err().to_string(),
            "候选 1 已是 approved，不能重复审核"
        );
    }

    #[test]
    fn verified_local_policy_becomes_conflict() {
        let store = store();
        policy::set_provenance(
            &store,
            &ProvenanceInput {
                id: "1".to_string(),
                status: "verified".to_string(),
                source_url: "https://example.com".to_string(),
                note: "人工核对".to_string(),
            },
        )
        .unwrap();
        let stats = enqueue(
            &store,
            "https://example.com/v1/policies",
            vec![PendingPolicy {
                policy: remote("支付宝"),
                payload_enc: "plain:{}".to_string(),
            }],
        )
        .unwrap();
        assert_eq!(stats.conflict, 1);
        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: None,
                status: Some("conflict".to_string()),
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].status, "conflict");
    }

    #[test]
    fn sync_state_round_trips() {
        let store = store();
        let saved = record_state(&store, "https://example.com", "12", "etag-1", "ok", "").unwrap();
        assert_eq!(saved.cursor, "12");
        assert_eq!(saved.etag, "etag-1");
        assert_eq!(saved.last_status, "ok");
        let same = state(&store).unwrap();
        assert_eq!(same.source, "https://example.com");
    }
}
