//! DOM-01 assets: list/get/create/update over the v9 `digital_assets` table.
//!
//! The old desktop schema stores account descriptions as free text; the
//! platform contract uses `name`/`kind`. This module owns that translation so
//! HTTP/CLI/Agent callers never hand-write SQL or guess legacy columns.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use digilegacy_store::{Store, StoreError};

pub const ASSET_KINDS: &[&str] = &[
    "account",
    "file",
    "device",
    "domain",
    "subscription",
    "physical",
    "financial",
    "real_estate",
    "vehicle",
    "intellectual_property",
    "business",
    "virtual_item",
    "membership",
    "identity",
    "other",
];

pub const ASSET_STATUSES: &[&str] = &["active", "archived"];

#[derive(Debug)]
pub enum DomainError {
    NotFound(String),
    Invalid(String),
    Conflict(String),
    Store(StoreError),
}

impl From<StoreError> for DomainError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(s) | Self::Invalid(s) | Self::Conflict(s) => f.write_str(s),
            Self::Store(e) => f.write_str(&e.to_string()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInput {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub platform: String,
    pub status: String,
    pub value: String,
    pub note: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPage {
    pub items: Vec<Asset>,
    pub next_cursor: Option<String>,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mutation {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// Optional on update. `None` keeps the existing status.
    #[serde(default)]
    pub status: Option<String>,
}

fn row_to_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<Asset> {
    let value: f64 = row.get(5)?;
    Ok(Asset {
        id: row.get::<_, i64>(0)?.to_string(),
        name: row.get(1)?,
        kind: row.get(2)?,
        platform: row.get(3)?,
        status: row.get(4)?,
        value: if value == 0.0 {
            String::new()
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
        },
        note: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn validate_mutation(input: &Mutation) -> Result<(), DomainError> {
    if input.name.trim().is_empty() || input.name.chars().count() > 120 {
        return Err(DomainError::Invalid(
            "资产名称必须为 1..120 个字符".to_string(),
        ));
    }
    if !ASSET_KINDS.contains(&input.kind.as_str()) {
        return Err(DomainError::Invalid(format!(
            "未知资产类型: {}",
            input.kind
        )));
    }
    if let Some(value) = &input.value {
        if !value.trim().is_empty() && value.parse::<f64>().is_err() {
            return Err(DomainError::Invalid("资产估值必须是数字".to_string()));
        }
    }
    if let Some(status) = &input.status {
        if !ASSET_STATUSES.contains(&status.as_str()) {
            return Err(DomainError::Invalid(format!("未知资产状态: {status}")));
        }
    }
    Ok(())
}

fn parse_value(input: &Mutation) -> Result<f64, DomainError> {
    match input
        .value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(s) => s
            .parse::<f64>()
            .map_err(|_| DomainError::Invalid("资产估值必须是数字".to_string())),
        None => Ok(0.0),
    }
}

fn ensure_platform(tx: &rusqlite::Transaction<'_>, name: &str) -> rusqlite::Result<i64> {
    if name.trim().is_empty() {
        tx.execute(
            "INSERT OR IGNORE INTO platforms (id, name, category) VALUES (0, '未分类', 'unknown')",
            [],
        )?;
        return Ok(0);
    }
    if let Some(id) = tx
        .query_row("SELECT id FROM platforms WHERE name = ?1", [name], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
    {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO platforms (name, category) VALUES (?1, 'unknown')",
        [name],
    )?;
    Ok(tx.last_insert_rowid())
}

pub fn list(store: &Store, input: &ListInput) -> Result<AssetPage, DomainError> {
    let limit = input.limit.unwrap_or(50).clamp(1, 200);
    let offset = input
        .cursor
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let status = input.status.as_deref().unwrap_or("active");
    let q = input.q.as_deref().unwrap_or("").trim().to_string();

    store
        .read(|conn| {
            let total: i64 = conn.query_row(
                "SELECT count(*) FROM digital_assets a
                 LEFT JOIN platforms p ON p.id = a.platform_id
                 WHERE (?1 = 'all' OR a.status = ?1)
                   AND (?2 = '' OR coalesce(a.account_description,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.account_identifier,'') LIKE '%' || ?2 || '%'
                        OR coalesce(p.name,'') LIKE '%' || ?2 || '%')",
                params![status, q],
                |r| r.get(0),
            )?;

            let mut stmt = conn.prepare(
                "SELECT a.id,
                        coalesce(nullif(a.account_description,''), nullif(a.account_identifier,''), '未命名资产'),
                        a.category,
                        coalesce(p.name,''),
                        a.status,
                        a.estimated_value,
                        coalesce(a.notes,''),
                        a.updated_at
                 FROM digital_assets a
                 LEFT JOIN platforms p ON p.id = a.platform_id
                 WHERE (?1 = 'all' OR a.status = ?1)
                   AND (?2 = '' OR coalesce(a.account_description,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.account_identifier,'') LIKE '%' || ?2 || '%'
                        OR coalesce(p.name,'') LIKE '%' || ?2 || '%')
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT ?3 OFFSET ?4",
            )?;
            let rows = stmt.query_map(params![status, q, limit, offset], row_to_asset)?;
            let items: Vec<Asset> = rows.collect::<rusqlite::Result<_>>()?;
            let next = if offset + items.len() as i64 >= total {
                None
            } else {
                Some((offset + items.len() as i64).to_string())
            };
            Ok(AssetPage {
                items,
                next_cursor: next,
                total,
            })
        })
        .map_err(DomainError::from)
}

pub fn get(store: &Store, id: &str) -> Result<Asset, DomainError> {
    let id: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("资产 id 不是数字: {id}")))?;
    store
        .read(|conn| {
            conn.query_row(
                "SELECT a.id,
                        coalesce(nullif(a.account_description,''), nullif(a.account_identifier,''), '未命名资产'),
                        a.category,
                        coalesce(p.name,''),
                        a.status,
                        a.estimated_value,
                        coalesce(a.notes,''),
                        a.updated_at
                 FROM digital_assets a
                 LEFT JOIN platforms p ON p.id = a.platform_id
                 WHERE a.id = ?1",
                [id],
                row_to_asset,
            )
            .optional()
        })
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::NotFound(format!("资产 {id} 不存在")))
}

pub fn create(store: &Store, input: &Mutation) -> Result<Asset, DomainError> {
    validate_mutation(input)?;
    let value = parse_value(input)?;
    let platform = input.platform.clone().unwrap_or_default();
    let status = input.status.as_deref().unwrap_or("active");
    let now = Utc::now().to_rfc3339();

    let id = store
        .write(|conn| {
            let tx = conn.transaction()?;
            let platform_id = ensure_platform(&tx, &platform)?;
            tx.execute(
                "INSERT INTO digital_assets
                 (platform_id, account_identifier, account_description, category, estimated_value,
                  value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc,
                  is_confirmed, created_at, updated_at, status, last_verified_at, source, policy_snapshot_hash)
                 VALUES (?1, '', ?2, ?3, ?4, 'CNY', 'no_action', '', '', ?5, '', 0, ?6, ?6,
                         ?7, '', 'manual', '')",
                params![platform_id, input.name.trim(), input.kind, value, input.note.clone().unwrap_or_default(), now, status],
            )?;
            let id = tx.last_insert_rowid();
            tx.commit()?;
            Ok(id)
        })
        .map_err(DomainError::from)?;
    get(store, &id.to_string())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateInput {
    pub items: Vec<Mutation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCreateResult {
    pub created: i64,
    pub skipped: i64,
    pub items: Vec<Asset>,
}

/// Create selected discovery suggestions as assets. Existing name/platform pairs are
/// skipped so the wizard is safe to rerun after a partial failure.
pub fn bulk_create(
    store: &Store,
    input: &BulkCreateInput,
) -> Result<BulkCreateResult, DomainError> {
    if input.items.is_empty() {
        return Err(DomainError::Invalid("至少选择一项资产".to_string()));
    }
    if input.items.len() > 50 {
        return Err(DomainError::Invalid("一次最多导入 50 项资产".to_string()));
    }
    for item in &input.items {
        validate_mutation(item)?;
    }

    let mut created_items = Vec::with_capacity(input.items.len());
    let mut skipped = 0i64;
    for item in &input.items {
        let name = item.name.trim();
        let platform = item.platform.as_deref().unwrap_or("").trim();
        let already_exists = store
            .read(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT count(*) FROM digital_assets a
                     LEFT JOIN platforms p ON p.id=a.platform_id
                     WHERE coalesce(a.account_description,'')=?1
                       AND coalesce(p.name,'')=?2",
                    rusqlite::params![name, platform],
                    |row| row.get(0),
                )?;
                Ok(count > 0)
            })
            .map_err(DomainError::from)?;
        if already_exists {
            skipped += 1;
            continue;
        }
        created_items.push(create(store, item)?);
    }
    let created = created_items.len() as i64;
    Ok(BulkCreateResult {
        created,
        skipped,
        items: created_items,
    })
}

pub fn update(store: &Store, input: &Mutation) -> Result<Asset, DomainError> {
    validate_mutation(input)?;
    let id: i64 = input
        .id
        .as_deref()
        .ok_or_else(|| DomainError::Invalid("更新资产必须提供 id".to_string()))?
        .parse()
        .map_err(|_| DomainError::Invalid("资产 id 不是数字".to_string()))?;
    let value = parse_value(input)?;
    let platform = input.platform.clone().unwrap_or_default();
    let now = Utc::now().to_rfc3339();

    let changed = store
        .write(|conn| {
            let tx = conn.transaction()?;
            let platform_id = ensure_platform(&tx, &platform)?;
            let changed = tx.execute(
                "UPDATE digital_assets
                 SET account_description=?2, category=?3, platform_id=?4,
                     estimated_value=?5, notes=?6, status=coalesce(?7, status), updated_at=?8
                 WHERE id=?1",
                params![
                    id,
                    input.name.trim(),
                    input.kind,
                    platform_id,
                    value,
                    input.note.clone().unwrap_or_default(),
                    input.status.as_deref(),
                    now
                ],
            )?;
            tx.commit()?;
            Ok(changed)
        })
        .map_err(DomainError::from)?;
    if changed == 0 {
        return Err(DomainError::NotFound(format!("资产 {id} 不存在")));
    }
    get(store, &id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_create_skips_existing_pairs_and_rejects_empty_batches() {
        let store = Store::open_in_memory(None).unwrap();
        let first = bulk_create(
            &store,
            &BulkCreateInput {
                items: vec![Mutation {
                    id: None,
                    name: "主邮箱".to_string(),
                    kind: "account".to_string(),
                    platform: Some("邮箱".to_string()),
                    value: None,
                    note: None,
                    status: None,
                }],
            },
        )
        .unwrap();
        assert_eq!(first.created, 1);
        assert_eq!(first.skipped, 0);

        let second = bulk_create(
            &store,
            &BulkCreateInput {
                items: vec![
                    Mutation {
                        id: None,
                        name: "主邮箱".to_string(),
                        kind: "account".to_string(),
                        platform: Some("邮箱".to_string()),
                        value: None,
                        note: None,
                        status: None,
                    },
                    Mutation {
                        id: None,
                        name: "备用邮箱".to_string(),
                        kind: "account".to_string(),
                        platform: Some("邮箱".to_string()),
                        value: None,
                        note: None,
                        status: None,
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(second.created, 1);
        assert_eq!(second.skipped, 1);

        let empty = bulk_create(&store, &BulkCreateInput { items: vec![] });
        assert!(matches!(empty, Err(DomainError::Invalid(_))));
    }

    #[test]
    fn create_list_get_update_roundtrip() {
        let store = Store::open_in_memory(None).unwrap();
        let created = create(
            &store,
            &Mutation {
                id: None,
                name: "主邮箱".to_string(),
                kind: "account".to_string(),
                platform: Some("示例邮箱".to_string()),
                value: Some("120".to_string()),
                note: Some("留给家人".to_string()),
                status: None,
            },
        )
        .unwrap();
        assert_eq!(created.name, "主邮箱");
        assert_eq!(created.value, "120");
        assert_eq!(created.status, "active");

        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: None,
                q: Some("邮箱".to_string()),
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].platform, "示例邮箱");

        let fetched = get(&store, &created.id).unwrap();
        assert_eq!(fetched.note, "留给家人");

        let updated = update(
            &store,
            &Mutation {
                id: Some(created.id.clone()),
                name: "主邮箱（已更新）".to_string(),
                kind: "account".to_string(),
                platform: Some("示例邮箱".to_string()),
                value: Some("150".to_string()),
                note: Some("新备注".to_string()),
                status: Some("archived".to_string()),
            },
        )
        .unwrap();
        assert_eq!(updated.name, "主邮箱（已更新）");
        assert_eq!(updated.value, "150");
        assert_eq!(updated.status, "archived");

        let active_page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: Some("active".to_string()),
                q: Some("邮箱".to_string()),
            },
        )
        .unwrap();
        assert_eq!(active_page.total, 0);

        let archived_page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: Some("archived".to_string()),
                q: Some("邮箱".to_string()),
            },
        )
        .unwrap();
        assert_eq!(archived_page.total, 1);

        let restored = update(
            &store,
            &Mutation {
                id: Some(created.id.clone()),
                name: "主邮箱（已更新）".to_string(),
                kind: "account".to_string(),
                platform: Some("示例邮箱".to_string()),
                value: Some("150".to_string()),
                note: Some("新备注".to_string()),
                status: Some("active".to_string()),
            },
        )
        .unwrap();
        assert_eq!(restored.status, "active");
    }

    #[test]
    fn every_declared_asset_kind_is_accepted() {
        let store = Store::open_in_memory(None).unwrap();
        for kind in ASSET_KINDS {
            let created = create(
                &store,
                &Mutation {
                    id: None,
                    name: format!("分类测试-{kind}"),
                    kind: (*kind).to_string(),
                    platform: None,
                    value: None,
                    note: None,
                    status: None,
                },
            )
            .unwrap();
            assert_eq!(created.kind, *kind);
        }
    }

    #[test]
    fn missing_asset_is_not_found() {
        let store = Store::open_in_memory(None).unwrap();
        assert!(matches!(get(&store, "999"), Err(DomainError::NotFound(_))));
    }

    #[test]
    fn invalid_mutation_is_rejected() {
        let store = Store::open_in_memory(None).unwrap();
        assert!(matches!(
            create(
                &store,
                &Mutation {
                    id: None,
                    name: "".to_string(),
                    kind: "nope".to_string(),
                    platform: None,
                    value: None,
                    note: None,
                    status: None,
                }
            ),
            Err(DomainError::Invalid(_))
        ));
        assert!(matches!(
            create(
                &store,
                &Mutation {
                    id: None,
                    name: "合法名称".to_string(),
                    kind: "account".to_string(),
                    platform: None,
                    value: None,
                    note: None,
                    status: Some("deleted".to_string()),
                }
            ),
            Err(DomainError::Invalid(_))
        ));
    }
}
