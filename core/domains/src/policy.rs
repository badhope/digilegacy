//! DOM-02 policy library: 152 platform inheritance policies with provenance.
//!
//! The library is platform-native (`policies` table, schema v10). Old
//! `platforms`/`policy_overrides` remain for asset FK compatibility, but policy
//! reads never depend on the old desktop shape. Provenance is explicit: seeded
//! rows are `unverified` until a human records a source check.

use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use digilegacy_store::Store;

use crate::assets::DomainError;

const SEED_JSON: &str = include_str!("../../../content/policies/platforms.json");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub status: String,
    pub source_url: String,
    pub verified_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub name_en: String,
    pub category: String,
    pub subtype: String,
    pub has_property: bool,
    pub has_personality: bool,
    pub inheritable: bool,
    pub required_documents: String,
    pub process_description: String,
    pub estimated_difficulty: String,
    pub official_policy_url: String,
    pub estimated_time_days: String,
    pub notes: String,
    pub data_source: String,
    pub inheritance_form_url: String,
    pub monetization_path: String,
    pub legal_template_ids: String,
    pub risk_warnings: String,
    pub tags: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeedPolicy {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub name_en: String,
    pub category: String,
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub has_property: bool,
    #[serde(default)]
    pub has_personality: bool,
    #[serde(default)]
    pub inheritable: bool,
    #[serde(default)]
    pub required_documents: String,
    #[serde(default)]
    pub process_description: String,
    #[serde(default)]
    pub estimated_difficulty: String,
    #[serde(default)]
    pub official_policy_url: String,
    #[serde(default)]
    pub estimated_time_days: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub data_source: String,
    #[serde(default)]
    pub inheritance_form_url: String,
    #[serde(default)]
    pub monetization_path: String,
    #[serde(default)]
    pub legal_template_ids: String,
    #[serde(default)]
    pub risk_warnings: String,
    #[serde(default)]
    pub tags: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPage {
    pub items: Vec<Policy>,
    pub next_cursor: Option<String>,
    pub total: i64,
    pub categories: Vec<String>,
}

fn row_to_policy(row: &rusqlite::Row<'_>) -> rusqlite::Result<Policy> {
    Ok(Policy {
        id: row.get::<_, i64>(0)?.to_string(),
        name: row.get(1)?,
        name_en: row.get(2)?,
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
        data_source: row.get(14)?,
        inheritance_form_url: row.get(15)?,
        monetization_path: row.get(16)?,
        legal_template_ids: row.get(17)?,
        risk_warnings: row.get(18)?,
        tags: row.get(19)?,
        provenance: Provenance {
            status: row.get(20)?,
            source_url: row.get(21)?,
            verified_at: row.get(22)?,
            note: row.get(23)?,
        },
    })
}

pub fn seed_if_empty(store: &Store) -> Result<usize, DomainError> {
    let count: i64 = store
        .read(|conn| conn.query_row("SELECT count(*) FROM policies", [], |r| r.get(0)))
        .map_err(DomainError::from)?;
    if count > 0 {
        return Ok(0);
    }
    let entries: Vec<SeedPolicy> = serde_json::from_str(SEED_JSON)
        .map_err(|e| DomainError::Invalid(format!("政策内容解析失败: {e}")))?;
    let now = Utc::now().to_rfc3339();
    store
        .write(|conn| {
            let tx = conn.transaction()?;
            for item in &entries {
                tx.execute(
                    "INSERT OR REPLACE INTO policies
                     (id, name, name_en, category, subtype, has_property, has_personality, inheritable,
                      required_documents, process_description, estimated_difficulty, official_policy_url,
                      estimated_time_days, notes, data_source, inheritance_form_url, monetization_path,
                      legal_template_ids, risk_warnings, tags, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?21)",
                    rusqlite::params![
                        item.id,
                        item.name,
                        item.name_en,
                        item.category,
                        item.subtype,
                        i64::from(item.has_property),
                        i64::from(item.has_personality),
                        i64::from(item.inheritable),
                        item.required_documents,
                        item.process_description,
                        item.estimated_difficulty,
                        item.official_policy_url,
                        item.estimated_time_days,
                        item.notes,
                        item.data_source,
                        item.inheritance_form_url,
                        item.monetization_path,
                        item.legal_template_ids,
                        item.risk_warnings,
                        item.tags,
                        now
                    ],
                )?;
                tx.execute(
                    "INSERT OR REPLACE INTO policy_provenance
                     (platform_id, status, source_url, verified_at, verified_by, evidence_hash, note)
                     VALUES (?1, 'unverified', ?2, '', '', '', ?3)",
                    rusqlite::params![item.id, item.official_policy_url, item.data_source],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    Ok(entries.len())
}

pub fn list(store: &Store, input: &ListInput) -> Result<PolicyPage, DomainError> {
    let limit = input.limit.unwrap_or(50).clamp(1, 100);
    let offset = input
        .cursor
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let category = input.category.as_deref().unwrap_or("").trim().to_string();
    let q = input.q.as_deref().unwrap_or("").trim().to_string();
    store
        .read(|conn| {
            let total: i64 = conn.query_row(
                "SELECT count(*) FROM policies
                 WHERE (?1 = '' OR category = ?1)
                   AND (?2 = '' OR name LIKE '%'||?2||'%' OR name_en LIKE '%'||?2||'%'
                        OR category LIKE '%'||?2||'%' OR tags LIKE '%'||?2||'%'
                        OR notes LIKE '%'||?2||'%')",
                rusqlite::params![category, q],
                |r| r.get(0),
            )?;
            let mut stmt = conn.prepare(
                "SELECT p.id,p.name,p.name_en,p.category,p.subtype,p.has_property,p.has_personality,p.inheritable,
                        p.required_documents,p.process_description,p.estimated_difficulty,p.official_policy_url,
                        p.estimated_time_days,p.notes,p.data_source,p.inheritance_form_url,p.monetization_path,
                        p.legal_template_ids,p.risk_warnings,p.tags,
                        coalesce(v.status,'unverified'),coalesce(v.source_url,'' ),
                        coalesce(v.verified_at,''),coalesce(v.note,'')
                 FROM policies p LEFT JOIN policy_provenance v ON v.platform_id = p.id
                 WHERE (?1 = '' OR p.category = ?1)
                   AND (?2 = '' OR p.name LIKE '%'||?2||'%' OR p.name_en LIKE '%'||?2||'%'
                        OR p.category LIKE '%'||?2||'%' OR p.tags LIKE '%'||?2||'%'
                        OR p.notes LIKE '%'||?2||'%')
                 ORDER BY p.category, p.id LIMIT ?3 OFFSET ?4",
            )?;
            let items = stmt
                .query_map(rusqlite::params![category, q, limit, offset], row_to_policy)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let next_cursor = if offset + items.len() as i64 >= total {
                None
            } else {
                Some((offset + items.len() as i64).to_string())
            };
            let mut cat_stmt = conn.prepare("SELECT DISTINCT category FROM policies ORDER BY category")?;
            let categories = cat_stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(PolicyPage {
                items,
                next_cursor,
                total,
                categories,
            })
        })
        .map_err(DomainError::from)
}

pub fn all(store: &Store) -> Result<Vec<Policy>, DomainError> {
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT p.id,p.name,p.name_en,p.category,p.subtype,p.has_property,p.has_personality,p.inheritable,
                        p.required_documents,p.process_description,p.estimated_difficulty,p.official_policy_url,
                        p.estimated_time_days,p.notes,p.data_source,p.inheritance_form_url,p.monetization_path,
                        p.legal_template_ids,p.risk_warnings,p.tags,
                        coalesce(v.status,'unverified'),coalesce(v.source_url,''),
                        coalesce(v.verified_at,''),coalesce(v.note,'')
                 FROM policies p LEFT JOIN policy_provenance v ON v.platform_id = p.id
                 ORDER BY p.category, p.id",
            )?;
            let rows = stmt
                .query_map([], row_to_policy)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .map_err(DomainError::from)
}

pub fn find_by_name(store: &Store, name: &str) -> Result<Option<Policy>, DomainError> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    store
        .read(|conn| {
            conn.query_row(
                "SELECT p.id,p.name,p.name_en,p.category,p.subtype,p.has_property,p.has_personality,p.inheritable,
                        p.required_documents,p.process_description,p.estimated_difficulty,p.official_policy_url,
                        p.estimated_time_days,p.notes,p.data_source,p.inheritance_form_url,p.monetization_path,
                        p.legal_template_ids,p.risk_warnings,p.tags,
                        coalesce(v.status,'unverified'),coalesce(v.source_url,''),
                        coalesce(v.verified_at,''),coalesce(v.note,'')
                 FROM policies p LEFT JOIN policy_provenance v ON v.platform_id = p.id
                 WHERE lower(p.name)=lower(?1) OR lower(p.name_en)=lower(?1) LIMIT 1",
                [name],
                row_to_policy,
            )
            .optional()
        })
        .map_err(DomainError::from)
}

pub fn get(store: &Store, id: &str) -> Result<Policy, DomainError> {
    let id: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("政策 id 不是数字: {id}")))?;
    store
        .read(|conn| {
            conn.query_row(
                "SELECT p.id,p.name,p.name_en,p.category,p.subtype,p.has_property,p.has_personality,p.inheritable,
                        p.required_documents,p.process_description,p.estimated_difficulty,p.official_policy_url,
                        p.estimated_time_days,p.notes,p.data_source,p.inheritance_form_url,p.monetization_path,
                        p.legal_template_ids,p.risk_warnings,p.tags,
                        coalesce(v.status,'unverified'),coalesce(v.source_url,''),
                        coalesce(v.verified_at,''),coalesce(v.note,'')
                 FROM policies p LEFT JOIN policy_provenance v ON v.platform_id = p.id
                 WHERE p.id = ?1",
                [id],
                row_to_policy,
            )
            .optional()
        })
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::NotFound(format!("政策 {id} 不存在")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_152_policies_and_keeps_unverified_provenance() {
        let store = Store::open_in_memory(None).unwrap();
        let count = seed_if_empty(&store).unwrap();
        assert_eq!(count, 152);
        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                category: Some("支付金融".to_string()),
                q: Some("支付宝".to_string()),
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "支付宝");
        assert_eq!(page.items[0].provenance.status, "unverified");
        assert_eq!(
            page.categories
                .iter()
                .filter(|c| c.as_str() == "支付金融")
                .count(),
            1
        );
    }

    #[test]
    fn get_unknown_is_not_found() {
        let store = Store::open_in_memory(None).unwrap();
        assert!(matches!(get(&store, "9999"), Err(DomainError::NotFound(_))));
    }
}
