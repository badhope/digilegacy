//! Connection registry and health state (CON-01..04).
//!
//! Connection rows are metadata + health only. Credential material stays in
//! `config_enc` and is never returned by the read model. Environment-managed
//! channels are marked with `config_enc = 'env'`.

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use digilegacy_store::Store;

use crate::assets::DomainError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub enabled: bool,
    pub last_check_at: String,
    pub last_status: String,
    pub last_error: String,
    pub updated_at: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<Connection>,
    pub total: i64,
}

fn redact_error(value: String) -> String {
    if value.contains("http://") || value.contains("https://") || value.contains("://") {
        "连接错误（地址已隐藏）".to_string()
    } else {
        value
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Connection> {
    let config_enc: String = row.get(3)?;
    let last_error: String = row.get(7)?;
    Ok(Connection {
        id: row.get::<_, i64>(0)?.to_string(),
        kind: row.get(1)?,
        label: row.get(2)?,
        enabled: row.get::<_, i64>(4)? != 0,
        last_check_at: row.get(5)?,
        last_status: row.get(6)?,
        last_error: redact_error(last_error),
        updated_at: row.get(8)?,
        source: if config_enc == "env" {
            "env".to_string()
        } else {
            "stored".to_string()
        },
    })
}

pub fn list(store: &Store) -> Result<Page, DomainError> {
    store
        .read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, kind, label, config_enc, enabled, last_check_at,
                        last_status, last_error, updated_at
                 FROM connections ORDER BY kind, label, id",
            )?;
            let items = stmt
                .query_map([], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let total = items.len() as i64;
            Ok(Page { items, total })
        })
        .map_err(DomainError::from)
}

pub fn record_check(
    store: &Store,
    id: &str,
    status: &str,
    error: &str,
) -> Result<Connection, DomainError> {
    if !matches!(
        status,
        "unconfigured" | "ok" | "degraded" | "error" | "disabled"
    ) {
        return Err(DomainError::Invalid(
            "连接状态必须是 unconfigured/ok/degraded/error/disabled".to_string(),
        ));
    }
    let result = store.write(|conn| {
        let now = Utc::now().to_rfc3339();
        let changed = conn.execute(
            "UPDATE connections
             SET last_check_at=?2, last_status=?3, last_error=?4, updated_at=?2
             WHERE id=?1",
            params![id, now, status, error],
        )?;
        if changed == 0 {
            return Err(digilegacy_store::StoreError::State(
                "连接不存在".to_string(),
            ));
        }
        Ok(())
    });
    if let Err(digilegacy_store::StoreError::State(message)) = &result {
        if message == "连接不存在" {
            return Err(DomainError::NotFound(format!("连接 {id} 不存在")));
        }
    }
    result.map_err(DomainError::from)?;
    get(store, id)
}

pub const KINDS: [&str; 9] = [
    "llm",
    "policy",
    "smtp",
    "imap",
    "heartbeat",
    "webhook",
    "feishu",
    "wecom",
    "mcp",
];

pub fn is_kind(kind: &str) -> bool {
    KINDS.contains(&kind)
}

pub fn config_enc(store: &Store, id: &str) -> Result<String, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT config_enc FROM connections WHERE id=?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
        })
        .map_err(|error| match error {
            digilegacy_store::StoreError::Sql(rusqlite::Error::QueryReturnedNoRows) => {
                DomainError::NotFound(format!("连接 {id} 不存在"))
            }
            other => DomainError::from(other),
        })
}

/// Create a stored connection, or replace one by id. The caller supplies an
/// already encrypted config; this module never sees plaintext credentials.
pub fn upsert(
    store: &Store,
    id: Option<&str>,
    kind: &str,
    label: &str,
    config_enc: &str,
    enabled: bool,
) -> Result<Connection, DomainError> {
    if !is_kind(kind) {
        return Err(DomainError::Invalid(format!("未知连接类型: {kind}")));
    }
    let label = label.trim();
    if label.is_empty() || label.len() > 80 {
        return Err(DomainError::Invalid(
            "连接标签不能为空且不能超过 80 字符".to_string(),
        ));
    }
    if config_enc == "env" || !config_enc.starts_with("enc:") {
        return Err(DomainError::Invalid("连接配置未加密".to_string()));
    }
    let target_id = if let Some(id) = id {
        let existing = get(store, id)?;
        if existing.source == "env" {
            return Err(DomainError::Conflict(
                "环境变量管理的连接不能在平台内修改".to_string(),
            ));
        }
        if existing.kind != kind {
            return Err(DomainError::Invalid(
                "已保存连接不能改变类型，请新建连接".to_string(),
            ));
        }
        check_label_collision(store, kind, label, Some(id))?;
        Some(id.to_string())
    } else {
        check_label_collision(store, kind, label, None)?;
        None
    };

    store
        .write(|conn| {
            let now = Utc::now().to_rfc3339();
            if let Some(id) = target_id.as_deref() {
                let changed = conn.execute(
                    "UPDATE connections
                     SET kind=?2, label=?3, config_enc=?4, enabled=?5,
                         last_check_at='', last_status='unconfigured', last_error='', updated_at=?6
                     WHERE id=?1",
                    params![id, kind, label, config_enc, if enabled { 1 } else { 0 }, now],
                )?;
                if changed == 0 {
                    return Err(digilegacy_store::StoreError::State(
                        "连接不存在".to_string(),
                    ));
                }
            } else {
                conn.execute(
                    "INSERT INTO connections
                     (kind,label,config_enc,enabled,last_check_at,last_status,last_error,created_at,updated_at)
                     VALUES (?1,?2,?3,?4,'','unconfigured','',?5,?5)",
                    params![kind, label, config_enc, if enabled { 1 } else { 0 }, now],
                )?;
            }
            Ok(())
        })
        .map_err(DomainError::from)?;
    if let Some(id) = target_id {
        get(store, &id)
    } else {
        let page = list(store)?;
        page.items
            .into_iter()
            .find(|item| item.kind == kind && item.label == label)
            .ok_or_else(|| DomainError::NotFound("连接写入后未找到".to_string()))
    }
}

fn check_label_collision(
    store: &Store,
    kind: &str,
    label: &str,
    except_id: Option<&str>,
) -> Result<(), DomainError> {
    let id: Option<i64> = store
        .read(|conn| {
            conn.query_row(
                "SELECT id FROM connections WHERE kind=?1 AND label=?2 AND (?3 IS NULL OR id<>?3)",
                params![kind, label, except_id],
                |row| row.get(0),
            )
            .optional()
        })
        .map_err(DomainError::from)?;
    if id.is_some() {
        return Err(DomainError::Conflict(format!("连接 {kind}/{label} 已存在")));
    }
    Ok(())
}

pub fn delete(store: &Store, id: &str) -> Result<(String, String), DomainError> {
    let (kind, label, marker) = store
        .read(|conn| {
            conn.query_row(
                "SELECT kind, label, config_enc FROM connections WHERE id=?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
        })
        .map_err(|error| match error {
            digilegacy_store::StoreError::Sql(rusqlite::Error::QueryReturnedNoRows) => {
                DomainError::NotFound(format!("连接 {id} 不存在"))
            }
            other => DomainError::from(other),
        })?;
    if marker == "env" {
        return Err(DomainError::Conflict(
            "环境变量管理的连接不能在平台内删除".to_string(),
        ));
    }
    store
        .write(|conn| {
            conn.execute("DELETE FROM connections WHERE id=?1", params![id])?;
            Ok(())
        })
        .map_err(DomainError::from)?;
    Ok((kind, label))
}

pub fn get(store: &Store, id: &str) -> Result<Connection, DomainError> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT id, kind, label, config_enc, enabled, last_check_at,
                        last_status, last_error, updated_at
                 FROM connections WHERE id=?1",
                params![id],
                map_row,
            )
        })
        .map_err(|error| match error {
            digilegacy_store::StoreError::Sql(rusqlite::Error::QueryReturnedNoRows) => {
                DomainError::NotFound(format!("连接 {id} 不存在"))
            }
            other => DomainError::from(other),
        })
}

/// Seed or update environment-managed connection metadata.
/// `config_enc` stays a marker, never a secret.
pub fn ensure_env(
    store: &Store,
    kind: &str,
    label: &str,
    enabled: bool,
) -> Result<(), DomainError> {
    store
        .write(|conn| {
            let now = Utc::now().to_rfc3339();
            let existing: Option<String> = conn
                .query_row(
                    "SELECT config_enc FROM connections WHERE kind=?1 AND label=?2",
                    params![kind, label],
                    |r| r.get(0),
                )
                .optional()?;
            match existing.as_deref() {
                Some("env") => {
                    conn.execute(
                        "UPDATE connections SET enabled=?3, updated_at=?4 WHERE kind=?1 AND label=?2",
                        params![kind, label, if enabled { 1 } else { 0 }, now],
                    )?;
                }
                Some(_) => {}
                None if enabled => {
                    conn.execute(
                        "INSERT INTO connections
                         (kind,label,config_enc,enabled,last_check_at,last_status,last_error,created_at,updated_at)
                         VALUES (?1,?2,'env',1,'','unconfigured','',?3,?3)",
                        params![kind, label, now],
                    )?;
                }
                None => {}
            }
            Ok(())
        })
        .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory(None).unwrap()
    }

    #[test]
    fn list_hides_config_and_exposes_source() {
        let store = store();
        ensure_env(&store, "llm", "env", true).unwrap();
        store
            .write(|conn| {
                conn.execute(
                    "INSERT INTO connections (kind,label,config_enc,enabled,last_check_at,last_status,last_error,created_at,updated_at)
                     VALUES ('smtp','legacy','enc:v1:secret',1,'','unconfigured','','2026-01-01','2026-01-01')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let page = list(&store).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].source, "env");
        assert_eq!(page.items[1].source, "stored");
        assert!(!page
            .items
            .iter()
            .any(|item| item.last_error.contains("secret")));
    }

    #[test]
    fn record_check_updates_health_and_rejects_unknown_id() {
        let store = store();
        ensure_env(&store, "llm", "env", true).unwrap();
        let id = list(&store).unwrap().items[0].id.clone();
        let checked = record_check(&store, &id, "ok", "").unwrap();
        assert_eq!(checked.last_status, "ok");
        assert!(!checked.last_check_at.is_empty());
        assert!(matches!(
            record_check(&store, "999", "ok", ""),
            Err(DomainError::Store(_)) | Err(DomainError::NotFound(_))
        ));
    }

    #[test]
    fn stored_connection_crud_resets_health_and_protects_env_rows() {
        let store = store();
        ensure_env(&store, "webhook", "env", true).unwrap();
        assert!(matches!(
            upsert(&store, None, "webhook", "env", "enc:v1:new", true),
            Err(DomainError::Conflict(_))
        ));
        let saved = upsert(&store, None, "webhook", "family", "enc:v1:secret", true).unwrap();
        assert_eq!(saved.source, "stored");
        assert_eq!(saved.kind, "webhook");
        assert_eq!(saved.label, "family");
        assert!(matches!(
            upsert(&store, None, "webhook", "family", "enc:v1:other", true),
            Err(DomainError::Conflict(_))
        ));
        let id = saved.id.clone();
        let updated = upsert(
            &store,
            Some(&id),
            "webhook",
            "family-2",
            "enc:v1:next",
            false,
        )
        .unwrap();
        assert!(!updated.enabled);
        assert_eq!(updated.label, "family-2");
        assert_eq!(updated.last_status, "unconfigured");
        let (kind, label) = delete(&store, &id).unwrap();
        assert_eq!(kind, "webhook");
        assert_eq!(label, "family-2");
        assert!(matches!(delete(&store, &id), Err(DomainError::NotFound(_))));
    }

    #[test]
    fn environment_channels_can_be_disabled_without_touching_imported_rows() {
        let store = store();
        ensure_env(&store, "webhook", "env", true).unwrap();
        ensure_env(&store, "webhook", "env", false).unwrap();
        assert!(!list(&store).unwrap().items[0].enabled);
    }
}
