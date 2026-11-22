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
pub const ASSET_VALUE_STATUSES: &[&str] = &["market", "appraised", "insured", "unknown"];

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
    pub kind: Option<String>,
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
    pub value_status: String,
    pub note: String,
    pub identifier: String,
    pub ownership_share: String,
    pub beneficiary: String,
    pub beneficiary_contact: String,
    pub credential_location: String,
    pub expires_at: String,
    pub quantity: String,
    pub unit: String,
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
    pub value_status: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub identifier: Option<String>,
    #[serde(default)]
    pub ownership_share: Option<String>,
    #[serde(default)]
    pub beneficiary: Option<String>,
    #[serde(default)]
    pub beneficiary_contact: Option<String>,
    #[serde(default)]
    pub credential_location: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub quantity: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    /// Optional on update. `None` keeps the existing status.
    #[serde(default)]
    pub status: Option<String>,
}

fn row_to_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<Asset> {
    let value: f64 = row.get("estimated_value")?;
    Ok(Asset {
        id: row.get::<_, i64>("id")?.to_string(),
        name: row.get("name")?,
        kind: row.get("kind")?,
        platform: row.get("platform")?,
        status: row.get("status")?,
        value: if value == 0.0 {
            String::new()
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
        },
        value_status: row.get("value_status")?,
        note: row.get("note")?,
        identifier: row.get("identifier")?,
        ownership_share: row.get("ownership_share")?,
        beneficiary: row.get("beneficiary")?,
        beneficiary_contact: row.get("beneficiary_contact")?,
        credential_location: row.get("credential_location")?,
        expires_at: row.get("expires_at")?,
        quantity: row.get("quantity")?,
        unit: row.get("unit")?,
        updated_at: row.get("updated_at")?,
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
    for (label, value, max) in [
        ("机构/平台", input.platform.as_deref(), 80usize),
        ("备注", input.note.as_deref(), 2000),
        ("账号/编号", input.identifier.as_deref(), 120usize),
        ("权属份额", input.ownership_share.as_deref(), 120),
        ("受益人", input.beneficiary.as_deref(), 80),
        ("受益人联系方式", input.beneficiary_contact.as_deref(), 120),
        ("凭证位置", input.credential_location.as_deref(), 120),
        ("数量", input.quantity.as_deref(), 40),
        ("单位", input.unit.as_deref(), 20),
    ] {
        if value.is_some_and(|text| text.chars().count() > max) {
            return Err(DomainError::Invalid(format!("{label}最多 {max} 个字符")));
        }
    }
    if let Some(expires) = input.expires_at.as_deref() {
        if !expires.trim().is_empty()
            && chrono::NaiveDate::parse_from_str(expires, "%Y-%m-%d").is_err()
        {
            return Err(DomainError::Invalid(
                "资产到期日必须为 YYYY-MM-DD".to_string(),
            ));
        }
    }
    if let Some(value) = &input.value {
        if !value.trim().is_empty() {
            let parsed = value
                .parse::<f64>()
                .map_err(|_| DomainError::Invalid("资产估值必须是数字".to_string()))?;
            if !parsed.is_finite() || parsed < 0.0 {
                return Err(DomainError::Invalid(
                    "资产估值必须是非负有限数字".to_string(),
                ));
            }
        }
    }
    if let Some(status) = &input.value_status {
        if !ASSET_VALUE_STATUSES.contains(&status.as_str()) {
            return Err(DomainError::Invalid(format!("未知估值口径: {status}")));
        }
        if status == "unknown"
            && input
                .value
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(DomainError::Invalid(
                "待评估时估值应留空，金额请改选市值或评估价".to_string(),
            ));
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
    if !matches!(status, "all" | "active" | "archived") {
        return Err(DomainError::Invalid(format!("未知资产状态: {status}")));
    }
    let kind = input.kind.as_deref().unwrap_or("").trim();
    if !kind.is_empty() && !ASSET_KINDS.contains(&kind) {
        return Err(DomainError::Invalid(format!("未知资产类型: {kind}")));
    }
    let q = input.q.as_deref().unwrap_or("").trim().to_string();

    store
        .read(|conn| {
            let total: i64 = conn.query_row(
                "SELECT count(*) FROM digital_assets a
                 LEFT JOIN platforms p ON p.id = a.platform_id
                 WHERE (?1 = 'all' OR a.status = ?1)
                   AND (?2 = '' OR coalesce(a.account_description,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.account_identifier,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.notes,'') LIKE '%' || ?2 || '%'
                        OR coalesce(p.name,'') LIKE '%' || ?2 || '%')
                   AND (?3 = '' OR a.category = ?3)",
                params![status, q, kind],
                |r| r.get(0),
            )?;

            let mut stmt = conn.prepare(
                "SELECT a.id AS id,
                        coalesce(nullif(a.account_description,''), nullif(a.account_identifier,''), '未命名资产') AS name,
                        a.category AS kind,
                        coalesce(p.name,'') AS platform,
                        a.status AS status,
                        a.estimated_value AS estimated_value,
                        coalesce(a.notes,'') AS note,
                        a.updated_at AS updated_at,
                        coalesce(a.account_identifier,'') AS identifier,
                        coalesce(a.ownership_share,'') AS ownership_share,
                        coalesce(a.heir_name,'') AS beneficiary,
                        coalesce(a.heir_contact,'') AS beneficiary_contact,
                        coalesce(a.credential_location,'') AS credential_location,
                        coalesce(a.expires_at,'') AS expires_at,
                        coalesce(a.quantity,'') AS quantity,
                        coalesce(a.unit,'') AS unit,
                        coalesce(a.value_status,'market') AS value_status
                 FROM digital_assets a
                 LEFT JOIN platforms p ON p.id = a.platform_id
                 WHERE (?1 = 'all' OR a.status = ?1)
                   AND (?2 = '' OR coalesce(a.account_description,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.account_identifier,'') LIKE '%' || ?2 || '%'
                        OR coalesce(a.notes,'') LIKE '%' || ?2 || '%'
                        OR coalesce(p.name,'') LIKE '%' || ?2 || '%')
                   AND (?3 = '' OR a.category = ?3)
                 ORDER BY a.created_at DESC, a.id DESC
                 LIMIT ?4 OFFSET ?5",
            )?;
            let rows = stmt.query_map(params![status, q, kind, limit, offset], row_to_asset)?;
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
                "SELECT a.id AS id,
                        coalesce(nullif(a.account_description,''), nullif(a.account_identifier,''), '未命名资产') AS name,
                        a.category AS kind,
                        coalesce(p.name,'') AS platform,
                        a.status AS status,
                        a.estimated_value AS estimated_value,
                        coalesce(a.notes,'') AS note,
                        a.updated_at AS updated_at,
                        coalesce(a.account_identifier,'') AS identifier,
                        coalesce(a.ownership_share,'') AS ownership_share,
                        coalesce(a.heir_name,'') AS beneficiary,
                        coalesce(a.heir_contact,'') AS beneficiary_contact,
                        coalesce(a.credential_location,'') AS credential_location,
                        coalesce(a.expires_at,'') AS expires_at,
                        coalesce(a.quantity,'') AS quantity,
                        coalesce(a.unit,'') AS unit,
                        coalesce(a.value_status,'market') AS value_status
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
    let value_status = input.value_status.as_deref().unwrap_or("market");
    let identifier = input.identifier.as_deref().unwrap_or("").trim();
    let ownership_share = input.ownership_share.as_deref().unwrap_or("").trim();
    let beneficiary = input.beneficiary.as_deref().unwrap_or("").trim();
    let beneficiary_contact = input.beneficiary_contact.as_deref().unwrap_or("").trim();
    let credential_location = input.credential_location.as_deref().unwrap_or("").trim();
    let expires_at = input.expires_at.as_deref().unwrap_or("").trim();
    let quantity = input.quantity.as_deref().unwrap_or("").trim();
    let unit = input.unit.as_deref().unwrap_or("").trim();
    let now = Utc::now().to_rfc3339();

    let id = store
        .write(|conn| {
            let tx = conn.transaction()?;
            let platform_id = ensure_platform(&tx, &platform)?;
            tx.execute(
                "INSERT INTO digital_assets
                 (platform_id, account_identifier, account_description, category, estimated_value,
                  value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc,
                  credential_location, ownership_share, expires_at, quantity, unit, value_status,
                  is_confirmed, created_at, updated_at, status, last_verified_at, source, policy_snapshot_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'CNY', 'no_action', ?6, ?7, ?8, '', ?9, ?10, ?11, ?12, ?13,
                         ?14, 0, ?15, ?15, ?16, '', 'manual', '')",
                params![
                    platform_id,
                    identifier,
                    input.name.trim(),
                    input.kind,
                    value,
                    beneficiary,
                    beneficiary_contact,
                    input.note.clone().unwrap_or_default(),
                    credential_location,
                    ownership_share,
                    expires_at,
                    quantity,
                    unit,
                    value_status,
                    now,
                    status
                ],
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
pub struct DeleteInput {
    pub id: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub id: String,
    pub deleted: bool,
}

/// Physically remove an archived asset. Active records must be archived first;
/// the two-step lifecycle keeps a recoverable state before the hard delete.
pub fn delete(store: &Store, id: &str) -> Result<DeleteResult, DomainError> {
    let id_num: i64 = id
        .parse()
        .map_err(|_| DomainError::Invalid(format!("资产 id 不是数字: {id}")))?;
    let asset = get(store, id)?;
    if asset.status != "archived" {
        return Err(DomainError::Conflict(
            "物理删除前请先归档该资产".to_string(),
        ));
    }
    store
        .write(|conn| Ok(conn.execute("DELETE FROM digital_assets WHERE id = ?1", [id_num])?))
        .map_err(DomainError::from)?;
    Ok(DeleteResult {
        id: id_num.to_string(),
        deleted: true,
    })
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
    let existing = get(store, &id.to_string())?;
    let effective_value_status = input
        .value_status
        .as_deref()
        .unwrap_or(&existing.value_status);
    if effective_value_status == "unknown"
        && input
            .value
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(DomainError::Invalid(
            "待评估时估值应留空，金额请改选市值或评估价".to_string(),
        ));
    }
    // Omitted optional fields keep their stored value. Switching to
    // "unknown" deliberately clears the amount so the record cannot claim both.
    let value: Option<f64> = if effective_value_status == "unknown" {
        Some(0.0)
    } else if input.value.is_some() {
        Some(parse_value(input)?)
    } else {
        None
    };
    let now = Utc::now().to_rfc3339();

    let changed = store
        .write(|conn| {
            let tx = conn.transaction()?;
            let platform_id = match input.platform.as_deref() {
                Some(platform) => Some(ensure_platform(&tx, platform)?),
                None => None,
            };
            let changed = tx.execute(
                "UPDATE digital_assets
                 SET account_description=?2,
                     account_identifier=coalesce(?3, account_identifier),
                     category=?4,
                     platform_id=coalesce(?5, platform_id),
                     estimated_value=coalesce(?6, estimated_value),
                     notes=coalesce(?7, notes),
                     status=coalesce(?8, status),
                     heir_name=coalesce(?9, heir_name),
                     heir_contact=coalesce(?10, heir_contact),
                     credential_location=coalesce(?11, credential_location),
                     ownership_share=coalesce(?12, ownership_share),
                     expires_at=coalesce(?13, expires_at),
                     quantity=coalesce(?14, quantity),
                     unit=coalesce(?15, unit),
                     value_status=coalesce(?16, value_status),
                     updated_at=?17
                 WHERE id=?1",
                params![
                    id,
                    input.name.trim(),
                    input.identifier.as_deref().map(str::trim),
                    input.kind,
                    platform_id,
                    value,
                    input.note.as_deref(),
                    input.status.as_deref(),
                    input.beneficiary.as_deref().map(str::trim),
                    input.beneficiary_contact.as_deref().map(str::trim),
                    input.credential_location.as_deref().map(str::trim),
                    input.ownership_share.as_deref().map(str::trim),
                    input.expires_at.as_deref().map(str::trim),
                    input.quantity.as_deref().map(str::trim),
                    input.unit.as_deref().map(str::trim),
                    input.value_status.as_deref(),
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
                    value_status: None,
                    identifier: None,
                    ownership_share: None,
                    beneficiary: None,
                    beneficiary_contact: None,
                    credential_location: None,
                    expires_at: None,
                    quantity: None,
                    unit: None,
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
                        value_status: None,
                        identifier: None,
                        ownership_share: None,
                        beneficiary: None,
                        beneficiary_contact: None,
                        credential_location: None,
                        expires_at: None,
                        quantity: None,
                        unit: None,
                    },
                    Mutation {
                        id: None,
                        name: "备用邮箱".to_string(),
                        kind: "account".to_string(),
                        platform: Some("邮箱".to_string()),
                        value: None,
                        note: None,
                        status: None,
                        value_status: None,
                        identifier: None,
                        ownership_share: None,
                        beneficiary: None,
                        beneficiary_contact: None,
                        credential_location: None,
                        expires_at: None,
                        quantity: None,
                        unit: None,
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
                value_status: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
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
                kind: None,
                q: Some("邮箱".to_string()),
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].platform, "示例邮箱");

        let note_page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: None,
                kind: None,
                q: Some("留给家人".to_string()),
            },
        )
        .unwrap();
        assert_eq!(note_page.total, 1);

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
                value_status: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
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
                kind: None,
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
                kind: None,
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
                value_status: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
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
                    value_status: None,
                    identifier: None,
                    ownership_share: None,
                    beneficiary: None,
                    beneficiary_contact: None,
                    credential_location: None,
                    expires_at: None,
                    quantity: None,
                    unit: None,
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
                    value_status: None,
                    identifier: None,
                    ownership_share: None,
                    beneficiary: None,
                    beneficiary_contact: None,
                    credential_location: None,
                    expires_at: None,
                    quantity: None,
                    unit: None,
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
                    value_status: None,
                    identifier: None,
                    ownership_share: None,
                    beneficiary: None,
                    beneficiary_contact: None,
                    credential_location: None,
                    expires_at: None,
                    quantity: None,
                    unit: None,
                }
            ),
            Err(DomainError::Invalid(_))
        ));
    }
    #[test]
    fn deep_fields_round_trip_filter_and_hard_delete() {
        let store = Store::open_in_memory(None).unwrap();
        let created = create(
            &store,
            &Mutation {
                id: None,
                name: "鼓浪屿老宅".to_string(),
                kind: "real_estate".to_string(),
                platform: Some("不动产登记中心".to_string()),
                value: None,
                value_status: Some("unknown".to_string()),
                note: Some("待评估".to_string()),
                identifier: Some("闽(2026)厦门不动产权第0001号".to_string()),
                ownership_share: Some("50%".to_string()),
                beneficiary: Some("林小雨".to_string()),
                beneficiary_contact: Some("lin@example.com".to_string()),
                credential_location: Some("保险柜 A-03".to_string()),
                expires_at: Some("2056-12-31".to_string()),
                quantity: Some("1".to_string()),
                unit: Some("套".to_string()),
                status: None,
            },
        )
        .unwrap();
        assert_eq!(created.value_status, "unknown");
        assert_eq!(created.identifier, "闽(2026)厦门不动产权第0001号");
        assert_eq!(created.ownership_share, "50%");
        assert_eq!(created.beneficiary, "林小雨");
        assert_eq!(created.beneficiary_contact, "lin@example.com");
        assert_eq!(created.credential_location, "保险柜 A-03");
        assert_eq!(created.expires_at, "2056-12-31");
        assert_eq!(created.quantity, "1");
        assert_eq!(created.unit, "套");

        let page = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: Some("all".to_string()),
                kind: Some("real_estate".to_string()),
                q: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);

        let other_kind = list(
            &store,
            &ListInput {
                cursor: None,
                limit: Some(10),
                status: Some("all".to_string()),
                kind: Some("vehicle".to_string()),
                q: None,
            },
        )
        .unwrap();
        assert_eq!(other_kind.total, 0);

        let archived = update(
            &store,
            &Mutation {
                id: Some(created.id.clone()),
                name: created.name.clone(),
                kind: created.kind.clone(),
                platform: Some(created.platform.clone()),
                value: None,
                value_status: None,
                note: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
                status: Some("archived".to_string()),
            },
        )
        .unwrap();
        assert_eq!(archived.status, "archived");
        assert_eq!(archived.credential_location, "保险柜 A-03");
        assert_eq!(archived.ownership_share, "50%");

        let deleted = delete(&store, &created.id).unwrap();
        assert!(deleted.deleted);
        assert!(matches!(
            get(&store, &created.id),
            Err(DomainError::NotFound(_))
        ));
    }

    #[test]
    fn partial_update_preserves_omitted_fields() {
        let store = Store::open_in_memory(None).unwrap();
        let created = create(
            &store,
            &Mutation {
                id: None,
                name: "招商银行助学金卡".to_string(),
                kind: "financial".to_string(),
                platform: Some("招商银行".to_string()),
                value: Some("3200".to_string()),
                value_status: Some("appraised".to_string()),
                note: Some("每学期学费".to_string()),
                identifier: Some("尾号 8821".to_string()),
                ownership_share: Some("100%".to_string()),
                beneficiary: Some("林小雨".to_string()),
                beneficiary_contact: Some("lin@example.com".to_string()),
                credential_location: Some("书房保险柜".to_string()),
                expires_at: Some("2030-08-31".to_string()),
                quantity: Some("1".to_string()),
                unit: Some("张".to_string()),
                status: None,
            },
        )
        .unwrap();

        let renamed = update(
            &store,
            &Mutation {
                id: Some(created.id.clone()),
                name: "招商银行助学金卡（新）".to_string(),
                kind: "financial".to_string(),
                platform: None,
                value: None,
                value_status: None,
                note: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
                status: None,
            },
        )
        .unwrap();

        assert_eq!(renamed.name, "招商银行助学金卡（新）");
        assert_eq!(renamed.platform, "招商银行");
        assert_eq!(renamed.value, "3200");
        assert_eq!(renamed.value_status, "appraised");
        assert_eq!(renamed.note, "每学期学费");
        assert_eq!(renamed.identifier, "尾号 8821");
        assert_eq!(renamed.ownership_share, "100%");
        assert_eq!(renamed.beneficiary, "林小雨");
        assert_eq!(renamed.beneficiary_contact, "lin@example.com");
        assert_eq!(renamed.credential_location, "书房保险柜");
        assert_eq!(renamed.expires_at, "2030-08-31");
        assert_eq!(renamed.quantity, "1");
        assert_eq!(renamed.unit, "张");
        assert_eq!(renamed.status, "active");
    }

    #[test]
    fn unknown_value_status_clears_amount_and_blocks_new_amount() {
        let store = Store::open_in_memory(None).unwrap();
        let created = create(
            &store,
            &Mutation {
                id: None,
                name: "祖传翡翠手镯".to_string(),
                kind: "physical".to_string(),
                platform: None,
                value: Some("120000".to_string()),
                value_status: Some("appraised".to_string()),
                note: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
                status: None,
            },
        )
        .unwrap();

        let unknown = update(
            &store,
            &Mutation {
                id: Some(created.id.clone()),
                name: created.name.clone(),
                kind: created.kind.clone(),
                platform: None,
                value: None,
                value_status: Some("unknown".to_string()),
                note: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
                status: None,
            },
        )
        .unwrap();
        assert_eq!(unknown.value, "");
        assert_eq!(unknown.value_status, "unknown");

        let blocked = update(
            &store,
            &Mutation {
                id: Some(created.id),
                name: created.name,
                kind: created.kind,
                platform: None,
                value: Some("120000".to_string()),
                value_status: None,
                note: None,
                identifier: None,
                ownership_share: None,
                beneficiary: None,
                beneficiary_contact: None,
                credential_location: None,
                expires_at: None,
                quantity: None,
                unit: None,
                status: None,
            },
        );
        assert!(matches!(blocked, Err(DomainError::Invalid(_))));
    }

    #[test]
    fn negative_or_non_finite_value_is_rejected() {
        let store = Store::open_in_memory(None).unwrap();
        for bad in ["-1", "NaN", "inf"] {
            let result = create(
                &store,
                &Mutation {
                    id: None,
                    name: format!("非法估值 {bad}"),
                    kind: "financial".to_string(),
                    platform: None,
                    value: Some(bad.to_string()),
                    value_status: Some("market".to_string()),
                    note: None,
                    identifier: None,
                    ownership_share: None,
                    beneficiary: None,
                    beneficiary_contact: None,
                    credential_location: None,
                    expires_at: None,
                    quantity: None,
                    unit: None,
                    status: None,
                },
            );
            assert!(matches!(result, Err(DomainError::Invalid(_))));
        }
    }

    #[test]
    fn invalid_deep_field_values_are_rejected() {
        let store = Store::open_in_memory(None).unwrap();
        let base = Mutation {
            id: None,
            name: "期限测试".to_string(),
            kind: "financial".to_string(),
            platform: None,
            value: None,
            value_status: Some("unknown".to_string()),
            note: None,
            identifier: None,
            ownership_share: None,
            beneficiary: None,
            beneficiary_contact: None,
            credential_location: None,
            expires_at: Some("2056/12/31".to_string()),
            quantity: None,
            unit: None,
            status: None,
        };
        assert!(matches!(
            create(&store, &base),
            Err(DomainError::Invalid(_))
        ));
        let bad_status = Mutation {
            expires_at: None,
            value_status: Some("guessed".to_string()),
            ..base
        };
        assert!(matches!(
            create(&store, &bad_status),
            Err(DomainError::Invalid(_))
        ));
    }
}
