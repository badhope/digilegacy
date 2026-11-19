//! Legacy v5 backup import (STO-02 / WP-12).
//!
//! The old desktop app exported a JSON envelope (`app`, `version`, `tables`).
//! Import verifies the source password against the backup's own security
//! config before writing anything. New tables are mapped into v9 shape; the
//! source backup format is never replayed as raw SQL.

use serde_json::{json, Value};
use zeroize::Zeroizing;

use digilegacy_security::crypto::derive_from_salt;
use digilegacy_security::vault::{encrypt_field_v2, make_verifier, unwrap_data_key, verify_key};

use crate::{Result, Store, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub assets: usize,
    pub profiles: usize,
    pub heirs: usize,
    pub check_ins: usize,
    pub connections: usize,
    pub overrides: usize,
    pub reviews: usize,
    pub security_config: bool,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn integer(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn boolean_int(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_bool).unwrap_or(false) as i64
}

fn array(v: &Value, key: &str) -> Vec<Value> {
    v.get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn security_identity(sec: &Value) -> String {
    format!(
        "{}\n{}\n{}",
        text(sec, "kdf_salt"),
        text(sec, "verifier"),
        text(sec, "wrapped_dek")
    )
}

/// Verify the backup's password without writing. Returns the data key if the
/// backup carries security material, otherwise None.
fn unlock_backup(
    tables: &Value,
    password: Option<&str>,
) -> std::result::Result<Option<Zeroizing<[u8; 32]>>, String> {
    let Some(sec) = tables.get("security_config").filter(|v| v.is_object()) else {
        return Ok(None);
    };
    let salt = text(sec, "kdf_salt");
    let verifier = text(sec, "verifier");
    let wrapped = text(sec, "wrapped_dek");
    if salt.is_empty() || verifier.is_empty() {
        return Err("备份安全配置缺少盐或验证器".to_string());
    }
    let password = password
        .filter(|p| !p.is_empty())
        .ok_or("该备份包含加密数据，请输入源主密码".to_string())?;
    let kek = derive_from_salt(password, &salt).map_err(|e| e.to_string())?;
    let key = if wrapped.is_empty() {
        // v1 backup: password-derived key is the data key.
        Zeroizing::new(*kek)
    } else {
        unwrap_data_key(&wrapped, &kek).map_err(|_| "备份主密码不正确".to_string())?
    };
    if !verify_key(&verifier, &key) {
        return Err("备份主密码不正确".to_string());
    }
    Ok(Some(key))
}

fn encrypt_config(
    config: &Value,
    key: Option<&Zeroizing<[u8; 32]>>,
) -> std::result::Result<String, String> {
    let plain = serde_json::to_string(config).map_err(|e| format!("序列化连接配置失败: {e}"))?;
    match key {
        Some(k) => encrypt_field_v2(&plain, k),
        None => Err("备份含外连配置，但缺少可用的数据密钥".to_string()),
    }
}

fn ensure_platform(tx: &rusqlite::Transaction<'_>, id: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO platforms (id, name, category) VALUES (?1, ?2, 'unknown')",
        rusqlite::params![id, format!("平台 {id}")],
    )?;
    Ok(())
}

fn insert_asset(
    tx: &rusqlite::Transaction<'_>,
    a: &Value,
    mode: ImportMode,
) -> rusqlite::Result<()> {
    let platform_id = integer(a, "platform_id");
    if platform_id > 0 {
        ensure_platform(tx, platform_id)?;
    }
    let sql = if mode == ImportMode::Replace {
        "INSERT OR REPLACE INTO digital_assets
         (id, platform_id, account_identifier, account_description, category, estimated_value,
          value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc,
          is_confirmed, created_at, updated_at, status, last_verified_at, source, policy_snapshot_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,'active','','import','')"
    } else {
        "INSERT OR IGNORE INTO digital_assets
         (id, platform_id, account_identifier, account_description, category, estimated_value,
          value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc,
          is_confirmed, created_at, updated_at, status, last_verified_at, source, policy_snapshot_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,'active','','import','')"
    };
    tx.execute(
        sql,
        rusqlite::params![
            integer(a, "id"),
            platform_id,
            text(a, "account_identifier"),
            text(a, "account_description"),
            text(a, "category"),
            a.get("estimated_value")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            if text(a, "value_currency").is_empty() {
                "CNY".to_string()
            } else {
                text(a, "value_currency")
            },
            if text(a, "disposal_method").is_empty() {
                "no_action".to_string()
            } else {
                text(a, "disposal_method")
            },
            text(a, "heir_name"),
            text(a, "heir_contact"),
            text(a, "notes"),
            text(a, "credential_enc"),
            integer(a, "is_confirmed"),
            text(a, "created_at"),
            text(a, "updated_at"),
        ],
    )?;
    Ok(())
}

/// Import a legacy JSON backup. Authentication happens before any write.
pub fn import_legacy_backup(
    store: &Store,
    json_text: &str,
    password: Option<&str>,
    mode: ImportMode,
) -> Result<ImportReport> {
    let root: Value = serde_json::from_str(json_text)
        .map_err(|e| StoreError::State(format!("备份 JSON 无效: {e}")))?;
    if root.get("app").and_then(Value::as_str) != Some("digilegacy") {
        return Err(StoreError::State("不是 DigiLegacy 备份文件".to_string()));
    }
    let version = root.get("version").and_then(Value::as_i64).unwrap_or(0);
    if !(4..=5).contains(&version) {
        return Err(StoreError::State(format!("不支持的旧备份版本: {version}")));
    }
    let tables = root
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| StoreError::State("备份缺少 tables".to_string()))?;
    let tables_value = Value::Object(tables.clone());
    let key = unlock_backup(&tables_value, password).map_err(StoreError::State)?;

    // Reject security-config overwrite in merge mode if different.
    let incoming_sec = tables.get("security_config").filter(|v| v.is_object());
    if mode == ImportMode::Merge {
        if let Some(sec) = incoming_sec {
            let existing = store
                .read(|conn| {
                    Ok(conn
                        .query_row(
                            "SELECT kdf_salt, verifier, COALESCE(wrapped_dek,'') FROM security_config WHERE id=1",
                            [],
                            |r| {
                                Ok(json!({
                                    "kdf_salt": r.get::<_, String>(0)?,
                                    "verifier": r.get::<_, String>(1)?,
                                    "wrapped_dek": r.get::<_, String>(2)?,
                                }))
                            },
                        )
                        .ok())
                })?;
            if let Some(existing) = existing {
                if security_identity(&existing) != security_identity(sec) {
                    return Err(StoreError::State(
                        "目标库已有不同安全配置；覆盖导入请使用 replace".to_string(),
                    ));
                }
            }
        }
    }

    store.write(|conn| {
        let tx = conn.transaction()?;
        let mut report = ImportReport::default();

        if mode == ImportMode::Replace {
            for table in [
                "digital_assets",
                "user_profile",
                "heirs",
                "check_in_log",
                "review_records",
                "connections",
                "policy_overrides",
                "custody_config",
                "heir_recovery",
                "dms_config",
            ] {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO dms_config
                 (id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in, paused_until)
                 VALUES (1,0,30,3,7,14,NULL,'')",
                [],
            )?;
        }

        for a in array(&tables_value, "digital_assets") {
            insert_asset(&tx, &a, mode)?;
            report.assets += 1;
        }

        for p in array(&tables_value, "user_profile") {
            tx.execute(
                "INSERT OR REPLACE INTO user_profile
                 (id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone,
                  emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone,
                  heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in,
                  dms_enabled, created_at, updated_at, last_reviewed_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,'')",
                rusqlite::params![
                    integer(&p, "id"),
                    text(&p, "full_name"),
                    text(&p, "phone"),
                    text(&p, "email"),
                    text(&p, "id_last4"),
                    text(&p, "emergency_contact_name"),
                    text(&p, "emergency_contact_phone"),
                    text(&p, "emergency_contact_relation"),
                    text(&p, "emergency_contact_email"),
                    text(&p, "heir_primary_name"),
                    text(&p, "heir_primary_phone"),
                    text(&p, "heir_primary_relation"),
                    text(&p, "heir_primary_email"),
                    integer(&p, "check_in_interval_days"),
                    Option::<String>::None,
                    integer(&p, "dms_enabled"),
                    text(&p, "created_at"),
                    text(&p, "updated_at"),
                ],
            )?;
            report.profiles += 1;
        }

        for h in array(&tables_value, "heirs") {
            tx.execute(
                "INSERT OR REPLACE INTO heirs
                 (id, name, relation, phone, email, share_note, is_executor, sort_order, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    integer(&h, "id"),
                    text(&h, "name"),
                    text(&h, "relation"),
                    text(&h, "phone"),
                    text(&h, "email"),
                    text(&h, "share_note"),
                    integer(&h, "is_executor"),
                    integer(&h, "sort_order"),
                    text(&h, "created_at"),
                    text(&h, "updated_at"),
                ],
            )?;
            report.heirs += 1;
        }

        for c in array(&tables_value, "check_in_log") {
            tx.execute(
                "INSERT OR REPLACE INTO check_in_log (id, checked_at, status, note) VALUES (?1,?2,?3,?4)",
                rusqlite::params![
                    integer(&c, "id"),
                    text(&c, "checked_at"),
                    text(&c, "status"),
                    text(&c, "note"),
                ],
            )?;
            report.check_ins += 1;
        }

        if let Some(dms) = tables.get("dms_config").filter(|v| v.is_object()) {
            tx.execute(
                "INSERT OR REPLACE INTO dms_config
                 (id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in, paused_until)
                 VALUES (1,?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    integer(dms, "enabled"),
                    integer(dms, "check_in_interval_days"),
                    integer(dms, "stage1_wait_days"),
                    integer(dms, "stage2_wait_days"),
                    integer(dms, "stage3_wait_days"),
                    Option::<String>::None,
                    "",
                ],
            )?;
        }

        for o in array(&tables_value, "platform_overrides") {
            tx.execute(
                "INSERT OR REPLACE INTO policy_overrides
                 (id, name, name_en, category, subtype, has_property, has_personality, inheritable,
                  required_documents, process_description, estimated_difficulty, official_policy_url,
                  estimated_time_days, notes, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                rusqlite::params![
                    integer(&o, "id"),
                    text(&o, "name"),
                    text(&o, "name_en"),
                    text(&o, "category"),
                    text(&o, "subtype"),
                    boolean_int(&o, "has_property"),
                    boolean_int(&o, "has_personality"),
                    boolean_int(&o, "inheritable"),
                    text(&o, "required_documents"),
                    text(&o, "process_description"),
                    text(&o, "estimated_difficulty"),
                    text(&o, "official_policy_url"),
                    text(&o, "estimated_time_days"),
                    text(&o, "notes"),
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
            report.overrides += 1;
        }

        for r in array(&tables_value, "review_records") {
            tx.execute(
                "INSERT INTO review_records
                 (profile_id, item_key, fingerprint, ai_verdict, ai_note, human_verdict,
                  human_note, decided_by, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    text(&r, "profile_id"),
                    text(&r, "item_key"),
                    text(&r, "fingerprint"),
                    text(&r, "ai_verdict"),
                    text(&r, "ai_note"),
                    Option::<String>::None,
                    text(&r, "human_note"),
                    text(&r, "decided_by"),
                    text(&r, "created_at"),
                ],
            )?;
            report.reviews += 1;
        }

        // Old discrete config rows become unified `connections`.
        let mut add_connection = |kind: &str, cfg: Value| -> std::result::Result<(), StoreError> {
            let config_enc = encrypt_config(&cfg, key.as_ref())
                .map_err(StoreError::State)?;
            tx.execute(
                "INSERT OR REPLACE INTO connections
                 (kind, label, config_enc, enabled, last_check_at, last_status, last_error, created_at, updated_at)
                 VALUES (?1,'default',?2,?3,'','unconfigured','',?4,?4)",
                rusqlite::params![
                    kind,
                    config_enc,
                    integer(&cfg, "enabled"),
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
            report.connections += 1;
            Ok(())
        };

        if let Some(h) = tables.get("heartbeat_config").filter(|v| v.is_object()) {
            add_connection("heartbeat", h.clone())?;
        }
        if let Some(s) = tables.get("smtp_config").filter(|v| v.is_object()) {
            add_connection("smtp", s.clone())?;
        }
        if let Some(a) = tables.get("assistant_config").filter(|v| v.is_object()) {
            add_connection("llm", a.clone())?;
        }

        if let Some(c) = tables.get("custody_config").filter(|v| v.is_object()) {
            tx.execute(
                "INSERT OR REPLACE INTO custody_config
                 (id, backup_location_enc, recovery_code_location_enc, updated_at)
                 VALUES (1,?1,?2,?3)",
                rusqlite::params![
                    text(c, "backup_location_enc"),
                    text(c, "recovery_code_location_enc"),
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
        }

        if let Some(r) = tables.get("heir_recovery").filter(|v| v.is_object()) {
            tx.execute(
                "INSERT OR REPLACE INTO heir_recovery
                 (id, threshold, total, fingerprint, created_at, passphrase_protected, kdf_salt)
                 VALUES (1,?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    integer(r, "threshold"),
                    integer(r, "total"),
                    text(r, "fingerprint"),
                    text(r, "created_at"),
                    integer(r, "passphrase_protected"),
                    text(r, "kdf_salt"),
                ],
            )?;
        }

        if let Some(sec) = incoming_sec {
            let exists = tx
                .query_row("SELECT count(*) FROM security_config WHERE id=1", [], |r| {
                    r.get::<_, i64>(0)
                })?
                > 0;
            if !exists {
                tx.execute(
                    "INSERT INTO security_config (id, kdf_salt, verifier, recovery_seal, created_at, wrapped_dek)
                     VALUES (1,?1,?2,?3,?4,?5)",
                    rusqlite::params![
                        text(sec, "kdf_salt"),
                        text(sec, "verifier"),
                        text(sec, "recovery_seal"),
                        chrono::Utc::now().to_rfc3339(),
                        text(sec, "wrapped_dek"),
                    ],
                )?;
                report.security_config = true;
            }
        }

        // Defensive: a backup must not smuggle in a verifier that disagrees.
        if let Some(k) = &key {
            let verifier = text(incoming_sec.unwrap_or(&Value::Null), "verifier");
            if !verifier.is_empty() {
                let fresh = make_verifier(k).map_err(StoreError::State)?;
                let _ = fresh;
                if !verify_key(&verifier, k) {
                    return Err(StoreError::State("备份验证器与数据密钥不匹配".to_string()));
                }
            }
        }

        tx.commit()?;
        Ok(report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use digilegacy_security::crypto::{derive_from_salt, generate_salt_v3};
    use digilegacy_security::vault::{generate_data_key, make_verifier, wrap_data_key};

    fn fixture(password: &str) -> String {
        let dek = generate_data_key();
        let salt = generate_salt_v3();
        let kek = derive_from_salt(password, &salt).unwrap();
        let wrapped = wrap_data_key(&dek, &kek).unwrap();
        let verifier = make_verifier(&dek).unwrap();
        json!({
            "app": "digilegacy",
            "version": 5,
            "created_at": "2026-01-01 00:00:00",
            "tables": {
                "security_config": {
                    "kdf_salt": salt,
                    "verifier": verifier,
                    "wrapped_dek": wrapped,
                },
                "digital_assets": [{
                    "id": 7,
                    "platform_id": 16,
                    "account_identifier": "user@example.com",
                    "category": "社交",
                    "estimated_value": 120.0,
                    "value_currency": "CNY",
                    "disposal_method": "delete",
                    "credential_enc": "enc:v2:old",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }],
                "user_profile": [{
                    "id": 1,
                    "full_name": "张三",
                    "check_in_interval_days": 30,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }],
                "heirs": [{
                    "id": 1,
                    "name": "李四",
                    "relation": "配偶",
                    "is_executor": 1,
                    "sort_order": 1,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }],
                "check_in_log": [{
                    "id": 1,
                    "checked_at": "2026-01-01T00:00:00Z",
                    "status": "ok"
                }],
                "platform_overrides": [{
                    "id": 16,
                    "name": "B站",
                    "category": "音视频",
                    "has_property": true,
                    "inheritable": true
                }]
            }
        })
        .to_string()
    }

    #[test]
    fn import_requires_source_password() {
        let store = Store::open_in_memory(None).unwrap();
        let json = fixture("correct-password");
        assert!(import_legacy_backup(&store, &json, None, ImportMode::Merge).is_err());
        assert!(import_legacy_backup(&store, &json, Some("wrong"), ImportMode::Merge).is_err());
    }

    #[test]
    fn import_maps_v5_tables() {
        let store = Store::open_in_memory(None).unwrap();
        let json = fixture("correct-password");
        let report =
            import_legacy_backup(&store, &json, Some("correct-password"), ImportMode::Replace)
                .unwrap();
        assert_eq!(report.assets, 1);
        assert!(report.security_config);
        let asset_count: i64 = store
            .read(|conn| conn.query_row("SELECT count(*) FROM digital_assets", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(asset_count, 1);
        let source: String = store
            .read(|conn| {
                conn.query_row("SELECT source FROM digital_assets WHERE id=7", [], |r| {
                    r.get(0)
                })
            })
            .unwrap();
        assert_eq!(source, "import");
        assert_eq!(store.integrity_check().unwrap(), "ok");
    }

    #[test]
    fn import_is_idempotent_in_merge_mode() {
        let store = Store::open_in_memory(None).unwrap();
        let json = fixture("correct-password");
        import_legacy_backup(&store, &json, Some("correct-password"), ImportMode::Merge).unwrap();
        import_legacy_backup(&store, &json, Some("correct-password"), ImportMode::Merge).unwrap();
        let count: i64 = store
            .read(|conn| conn.query_row("SELECT count(*) FROM digital_assets", [], |r| r.get(0)))
            .unwrap();
        assert_eq!(count, 1);
    }
}
