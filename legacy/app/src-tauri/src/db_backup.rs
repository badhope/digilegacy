//! 备份导出 / 导入 / 存量加密（从 db.rs 拆出，纯移动，行为零变更）。
//!
//! 拆分理由：备份是 db.rs 里最长且最独立的一段（约 1500 行）——它有自己的
//! 输入校验层（行形状、加密字段、外连配置绑定、认证密文）、自己的事务边界，
//! 与日常 CRUD 几乎不共享状态。单独成文件后，改备份格式不会和 schema、
//! 资产查询混在同一个 diff 里，安全审阅也有了清晰的范围。
//!
//! 依赖方向：只依赖 db.rs 的连接获取与锁定态门禁（`super::*`，向上不依赖
//! 命令层）；加密仍统一走 `vault` / `crypto`，本模块不自管密钥。
//!
//! 安全约束（改动前必读）：
//! - 导入是**认证式**的：源口令、wrapped_dek、verifier、AEAD 标签必须全部
//!   校验通过，缺一不可；伪密文、空 wrapped、被篡改的外连配置一律拒绝。
//! - 导入默认 merge（非破坏性），replace 必须显式选择，且先写导入前快照。

use chrono::Local;
use rusqlite::{params, Connection};

use crate::models::*;

use super::heirs::get_heirs_db;
use super::profile_store::{select_profile_sensitive, update_profile_sensitive};
use super::{ensure_vault_readable, ensure_vault_readable_db, get_conn, get_security_config_db};

const HEARTBEAT_BINDING_FIELDS: &[&str] = &["enabled", "token", "endpoint"];
const SMTP_BINDING_FIELDS: &[&str] = &[
    "host",
    "port",
    "from_addr",
    "username",
    "password_enc",
    "allow_insecure",
];
const ASSISTANT_BINDING_FIELDS: &[&str] = &["enabled", "endpoint", "model", "api_key_enc"];

fn canonical_json(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let mut output = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key)
                        .map_err(|e| format!("序列化绑定字段失败: {}", e))?,
                );
                output.push(':');
                output.push_str(&canonical_json(&object[*key])?);
            }
            output.push('}');
            Ok(output)
        }
        serde_json::Value::Array(items) => {
            let values = items
                .iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", values.join(",")))
        }
        _ => serde_json::to_string(value).map_err(|e| format!("序列化绑定值失败: {}", e)),
    }
}

fn binding_object(tables: &serde_json::Value, table: &str, fields: &[&str]) -> serde_json::Value {
    let Some(value) = tables.get(table) else {
        return serde_json::Value::Null;
    };
    let Some(object) = value.as_object() else {
        return serde_json::Value::Null;
    };
    let mut selected = serde_json::Map::new();
    for field in fields {
        selected.insert(
            (*field).to_string(),
            object
                .get(*field)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(selected)
}

fn external_config_binding_payload(tables: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "heartbeat_config": binding_object(tables, "heartbeat_config", HEARTBEAT_BINDING_FIELDS),
        "smtp_config": binding_object(tables, "smtp_config", SMTP_BINDING_FIELDS),
        "assistant_config": binding_object(tables, "assistant_config", ASSISTANT_BINDING_FIELDS),
    })
}

fn make_external_config_binding(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<String, String> {
    let payload = external_config_binding_payload(tables);
    let canonical = canonical_json(&payload)?;
    crate::vault::encrypt_field_v2(&canonical, dek)
}

fn validate_external_config_binding(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<bool, String> {
    let Some(security) = tables.get("security_config") else {
        return Ok(false);
    };
    let Some(object) = security.as_object() else {
        return Ok(false);
    };
    let Some(raw) = object.get("external_config_binding") else {
        return Ok(false);
    };
    let binding = raw
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("备份 external_config_binding 必须是非空密文")?;
    let decrypted = crate::vault::decrypt_field_any(binding, dek)
        .map_err(|_| "备份外连配置完整性认证失败".to_string())?;
    let expected = canonical_json(&external_config_binding_payload(tables))?;
    if decrypted != expected {
        return Err("备份外连配置完整性认证失败".to_string());
    }
    Ok(true)
}

fn clear_external_delivery_config(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE heartbeat_config SET enabled=0, token='', endpoint='' WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空心跳配置失败: {}", e))?;
    conn.execute(
        "UPDATE smtp_config SET host='', port=587, from_addr='', username='', password_enc='', allow_insecure=0 WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空 SMTP 配置失败: {}", e))?;
    conn.execute(
        "UPDATE assistant_config SET enabled=0, endpoint='', model='', api_key_enc='' WHERE id=1",
        [],
    )
    .map_err(|e| format!("清空助手配置失败: {}", e))?;
    Ok(())
}

fn validate_external_delivery_config(tables: &serde_json::Value) -> Result<(), String> {
    if let Some(heartbeat) = tables.get("heartbeat_config").filter(|v| v.is_object()) {
        if let Some(endpoint) = heartbeat
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
        {
            if !endpoint.trim().is_empty() {
                crate::heartbeat::validate_heartbeat_endpoint(endpoint)?;
            }
        }
    }
    if let Some(assistant) = tables.get("assistant_config").filter(|v| v.is_object()) {
        if let Some(endpoint) = assistant
            .get("endpoint")
            .and_then(serde_json::Value::as_str)
        {
            if !endpoint.trim().is_empty() {
                crate::vault::validate_external_url(endpoint)?;
            }
        }
    }
    if let Some(smtp) = tables.get("smtp_config").filter(|v| v.is_object()) {
        if let Some(port) = smtp.get("port").and_then(serde_json::Value::as_i64) {
            if !(1..=65535).contains(&port) {
                return Err("备份 SMTP 端口必须在 1 到 65535 之间".to_string());
            }
        }
    }
    Ok(())
}

/// 导出备份 JSON：密文原样 + 盐/验证器，可在同密码下恢复到其他设备
pub fn export_backup_json_db() -> Result<String, String> {
    use serde_json::{json, Value};

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    ensure_vault_readable(conn)?;

    let mut assets: Vec<Value> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, platform_id, account_identifier, account_description, category, estimated_value, value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc, is_confirmed, created_at, updated_at FROM digital_assets",
        ).map_err(|e| format!("查询资产失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "platform_id": row.get::<_, i64>(1)?,
                    "account_identifier": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    "account_description": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    "category": row.get::<_, String>(4)?,
                    "estimated_value": row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                    "value_currency": row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "CNY".to_string()),
                    "disposal_method": row.get::<_, String>(7)?,
                    "heir_name": row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    "heir_contact": row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    "notes": row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                    "credential_enc": row.get::<_, Option<String>>(11)?.unwrap_or_default(),
                    "is_confirmed": row.get::<_, i64>(12)?,
                    "created_at": row.get::<_, String>(13)?,
                    "updated_at": row.get::<_, String>(14)?,
                }))
            })
            .map_err(|e| format!("读取资产失败: {}", e))?;
        for r in rows {
            assets.push(r.map_err(|e| format!("资产解析失败: {}", e))?);
        }
    }

    let mut profiles: Vec<Value> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in, dms_enabled, created_at, updated_at FROM user_profile",
        ).map_err(|e| format!("查询档案失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "full_name": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    "phone": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    "email": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    "id_last4": row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    "emergency_contact_name": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    "emergency_contact_phone": row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                    "emergency_contact_relation": row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    "emergency_contact_email": row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    "heir_primary_name": row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    "heir_primary_phone": row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                    "heir_primary_relation": row.get::<_, Option<String>>(11)?.unwrap_or_default(),
                    "heir_primary_email": row.get::<_, Option<String>>(12)?.unwrap_or_default(),
                    "check_in_interval_days": row.get::<_, i64>(13)?,
                    "last_check_in": row.get::<_, Option<String>>(14)?,
                    "dms_enabled": row.get::<_, i64>(15)?,
                    "created_at": row.get::<_, String>(16)?,
                    "updated_at": row.get::<_, String>(17)?,
                }))
            })
            .map_err(|e| format!("读取档案失败: {}", e))?;
        for r in rows {
            profiles.push(r.map_err(|e| format!("档案解析失败: {}", e))?);
        }
    }

    let mut check_ins: Vec<Value> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, checked_at, status, note FROM check_in_log ORDER BY id")
            .map_err(|e| format!("查询记录失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "checked_at": row.get::<_, String>(1)?,
                    "status": row.get::<_, String>(2)?,
                    "note": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                }))
            })
            .map_err(|e| format!("读取记录失败: {}", e))?;
        for r in rows {
            check_ins.push(r.map_err(|e| format!("记录解析失败: {}", e))?);
        }
    }

    let dms: Option<Value> = conn
        .query_row(
            "SELECT id, enabled, check_in_interval_days, stage1_wait_days, stage2_wait_days, stage3_wait_days, last_check_in FROM dms_config WHERE id=1",
            [],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "enabled": row.get::<_, i64>(1)?,
                    "check_in_interval_days": row.get::<_, i64>(2)?,
                    "stage1_wait_days": row.get::<_, i64>(3)?,
                    "stage2_wait_days": row.get::<_, i64>(4)?,
                    // 保留字段仅为兼容旧备份格式；应用内已不使用（见 notify.rs 注释）
                    "stage3_wait_days": row.get::<_, i64>(5)?,
                    "last_check_in": row.get::<_, Option<String>>(6)?,
                }))
            },
        )
        .ok();

    let security: Option<Value> = conn
        .query_row(
            "SELECT kdf_salt, verifier, COALESCE(recovery_seal, ''), COALESCE(wrapped_dek, '') FROM security_config WHERE id = 1",
            [],
            |row| {
                Ok(json!({
                    "kdf_salt": row.get::<_, String>(0)?,
                    "verifier": row.get::<_, String>(1)?,
                    // v3：带上恢复封印。旧版备份不含封印，换机恢复后即使记得恢复码
                    // 也无法再解出主密钥——恢复链路在「换电脑」这一最常见场景下断掉。
                    "recovery_seal": row.get::<_, String>(2)?,
                    // 两层密钥：换机后靠 wrapped_dek 才能解出数据密钥。
                    // 它本身是 KEK 加密的密文，随备份走不降低安全性。
                    "wrapped_dek": row.get::<_, String>(3)?,
                }))
            },
        )
        .ok()
        .map(|mut value| {
            if let Some(object) = value.as_object_mut() {
                if object
                    .get("recovery_seal")
                    .and_then(Value::as_str)
                    .is_some_and(str::is_empty)
                {
                    object.remove("recovery_seal");
                }
                if object
                    .get("wrapped_dek")
                    .and_then(Value::as_str)
                    .is_some_and(str::is_empty)
                {
                    object.remove("wrapped_dek");
                }
            }
            value
        });

    // 心跳配置与政策补丁也必须进备份：
    // 旧版只备份 5 张表，换机恢复后用户会发现「心跳令牌没了、自己导入的政策补丁也没了」，
    // 而这两项都是用户手工配置、无法从内置库重建的数据。
    let heartbeat: Option<Value> = conn
        .query_row(
            "SELECT enabled, COALESCE(token,''), COALESCE(endpoint,'') FROM heartbeat_config WHERE id=1",
            [],
            |row| {
                Ok(json!({
                    "enabled": row.get::<_, i64>(0)?,
                    "token": row.get::<_, String>(1)?,
                    "endpoint": row.get::<_, String>(2)?,
                }))
            },
        )
        .ok();

    let mut platform_overrides: Vec<Value> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes FROM platform_overrides ORDER BY id")
            .map_err(|e| format!("查询政策补丁失败: {}", e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "name_en": row.get::<_, String>(2)?,
                    "category": row.get::<_, String>(3)?,
                    "subtype": row.get::<_, String>(4)?,
                    "has_property": row.get::<_, i64>(5)? != 0,
                    "has_personality": row.get::<_, i64>(6)? != 0,
                    "inheritable": row.get::<_, i64>(7)? != 0,
                    "required_documents": row.get::<_, String>(8)?,
                    "process_description": row.get::<_, String>(9)?,
                    "estimated_difficulty": row.get::<_, String>(10)?,
                    "official_policy_url": row.get::<_, String>(11)?,
                    "estimated_time_days": row.get::<_, String>(12)?,
                    "notes": row.get::<_, String>(13)?,
                }))
            })
            .map_err(|e| format!("读取政策补丁失败: {}", e))?;
        for r in rows {
            platform_overrides.push(r.map_err(|e| format!("政策补丁解析失败: {}", e))?);
        }
    }

    // v4：新增继承人名单、SMTP、保管位置、助手配置。
    // 都是「换机后必须带走」的配置，否则用户在新机器上要重新配一遍。
    let heirs = export_heirs_json(conn)?;
    // 审核留痕也是「换机后必须带走」的证据：只留本机就等于换台电脑审核作废。
    let review_records = export_review_records_json(conn)?;
    let smtp = export_smtp_json(conn)?;
    let custody = export_custody_json(conn)?;
    let assistant = export_assistant_json(conn)?;
    let recovery = crate::heir_recovery::export_meta_json(conn)?;
    let backup_version = security
        .as_ref()
        .and_then(|value| value.get("wrapped_dek"))
        .and_then(Value::as_str)
        .filter(|wrapped| !wrapped.is_empty())
        .map(|_| 5)
        .unwrap_or_else(|| if security.is_some() { 4 } else { 5 });

    let mut backup = json!({
        "app": "digilegacy",
        "version": backup_version,
        "created_at": Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "tables": {
            "digital_assets": assets,
            "user_profile": profiles,
            "heirs": heirs,
            "review_records": review_records,
            "check_in_log": check_ins,
            "dms_config": dms,
            "security_config": security,
            "heartbeat_config": heartbeat,
            "platform_overrides": platform_overrides,
            "smtp_config": smtp,
            "custody_config": custody,
            "assistant_config": assistant,
            "heir_recovery": recovery,
        }
    });
    if backup["tables"]["security_config"].is_object() {
        let dek = crate::vault::get_key().ok_or("当前保险库密钥不可用，拒绝导出")?;
        validate_encrypted_backup_fields(&backup["tables"], &dek)?;
        let binding = make_external_config_binding(&backup["tables"], &dek)?;
        backup["tables"]["security_config"]["external_config_binding"] = Value::String(binding);
    }

    serde_json::to_string_pretty(&backup).map_err(|e| format!("生成备份失败: {}", e))
}

/// 导出辅助：继承人名单（含密文原样导出，导入方用同一主密码即可解密）。
fn export_heirs_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,''), is_executor, sort_order FROM heirs ORDER BY sort_order, id",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "relation": r.get::<_, String>(2)?,
                "phone": r.get::<_, String>(3)?,
                "email": r.get::<_, String>(4)?,
                "share_note": r.get::<_, String>(5)?,
                "is_executor": r.get::<_, i64>(6)?,
                "sort_order": r.get::<_, i64>(7)?,
            }))
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?;
    for r in rows {
        out.push(r.map_err(|e| format!("继承人解析失败: {}", e))?);
    }
    Ok(serde_json::Value::Array(out))
}

/// 导出审核留痕（含 AI 结论与人工结论）。备份是证据链的一部分，不能只留本机。
fn export_review_records_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT profile_id, item_key, fingerprint, ai_verdict, ai_note,
                    human_verdict, human_note, decided_by, created_at
             FROM review_records ORDER BY id",
        )
        .map_err(|e| format!("查询审核记录失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "profile_id": r.get::<_, String>(0)?,
                "item_key": r.get::<_, String>(1)?,
                "fingerprint": r.get::<_, String>(2)?,
                "ai_verdict": r.get::<_, String>(3)?,
                "ai_note": r.get::<_, String>(4)?,
                "human_verdict": r.get::<_, Option<String>>(5)?,
                "human_note": r.get::<_, String>(6)?,
                "decided_by": r.get::<_, String>(7)?,
                "created_at": r.get::<_, String>(8)?,
            }))
        })
        .map_err(|e| format!("读取审核记录失败: {}", e))?;
    for r in rows {
        out.push(r.map_err(|e| format!("审核记录解析失败: {}", e))?);
    }
    Ok(serde_json::Value::Array(out))
}

fn export_smtp_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT COALESCE(host,''), port, COALESCE(from_addr,''), COALESCE(username,''), COALESCE(password_enc,''), allow_insecure FROM smtp_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "host": r.get::<_, String>(0)?,
                "port": r.get::<_, i64>(1)?,
                "from_addr": r.get::<_, String>(2)?,
                "username": r.get::<_, String>(3)?,
                "password_enc": r.get::<_, String>(4)?,
                "allow_insecure": r.get::<_, i64>(5)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取 SMTP 配置失败: {}", other)),
    })
}

fn export_custody_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT COALESCE(backup_location_enc,''), COALESCE(recovery_code_location_enc,'') FROM custody_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "backup_location_enc": r.get::<_, String>(0)?,
                "recovery_code_location_enc": r.get::<_, String>(1)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取保管位置失败: {}", other)),
    })
}

fn export_assistant_json(conn: &Connection) -> Result<serde_json::Value, String> {
    use serde_json::json;
    conn.query_row(
        "SELECT enabled, COALESCE(endpoint,''), COALESCE(model,''), COALESCE(api_key_enc,'') FROM assistant_config WHERE id=1",
        [],
        |r| {
            Ok(json!({
                "enabled": r.get::<_, i64>(0)?,
                "endpoint": r.get::<_, String>(1)?,
                "model": r.get::<_, String>(2)?,
                "api_key_enc": r.get::<_, String>(3)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(serde_json::Value::Null),
        other => Err(format!("读取助手配置失败: {}", other)),
    })
}

/// 备份导入模式。
///
/// 覆盖是历史行为，会先清空资产/档案/签到记录；导入前没有快照时，
/// 一次误操作就永久丢数据。默认改为合并，覆盖必须显式选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// 保留现有数据，只追加备份里的资产（id 冲突时分配新 id）。
    Merge,
    /// 清空资产/档案/签到后写入备份内容（原行为）。
    Replace,
}

impl ImportMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "replace" => Self::Replace,
            _ => Self::Merge,
        }
    }
}

fn validate_backup_row_shapes(tables: &serde_json::Value) -> Result<(), String> {
    const ARRAY_TABLES: &[&str] = &[
        "digital_assets",
        "user_profile",
        "heirs",
        "review_records",
        "check_in_log",
        "platform_overrides",
    ];
    for table in ARRAY_TABLES {
        if let Some(value) = tables.get(*table) {
            if !value.is_array() && !value.is_null() {
                return Err(format!("备份字段 {} 必须是数组", table));
            }
            if let Some(rows) = value.as_array() {
                if rows.iter().any(|row| !row.is_object()) {
                    return Err(format!("备份字段 {} 的行必须是对象", table));
                }
            }
        }
    }
    for table in [
        "dms_config",
        "heartbeat_config",
        "smtp_config",
        "custody_config",
        "assistant_config",
        "heir_recovery",
    ] {
        if let Some(value) = tables.get(table) {
            if !value.is_object() && !value.is_null() {
                return Err(format!("备份字段 {} 必须是对象或 null", table));
            }
        }
    }
    Ok(())
}

fn validate_encrypted_backup_fields(
    tables: &serde_json::Value,
    dek: &[u8; 32],
) -> Result<(), String> {
    const FIELDS: &[(&str, &str)] = &[
        ("digital_assets", "account_identifier"),
        ("digital_assets", "account_description"),
        ("digital_assets", "heir_name"),
        ("digital_assets", "heir_contact"),
        ("digital_assets", "notes"),
        ("digital_assets", "credential_enc"),
        ("user_profile", "full_name"),
        ("user_profile", "phone"),
        ("user_profile", "email"),
        ("user_profile", "id_last4"),
        ("user_profile", "emergency_contact_name"),
        ("user_profile", "emergency_contact_phone"),
        ("user_profile", "emergency_contact_relation"),
        ("user_profile", "emergency_contact_email"),
        ("user_profile", "heir_primary_name"),
        ("user_profile", "heir_primary_phone"),
        ("user_profile", "heir_primary_relation"),
        ("user_profile", "heir_primary_email"),
        ("heirs", "name"),
        ("heirs", "relation"),
        ("heirs", "phone"),
        ("heirs", "email"),
        ("heirs", "share_note"),
        ("smtp_config", "password_enc"),
        ("custody_config", "backup_location_enc"),
        ("custody_config", "recovery_code_location_enc"),
        ("assistant_config", "api_key_enc"),
        ("heartbeat_config", "token"),
        ("security_config", "external_config_binding"),
    ];

    for &(table, field) in FIELDS {
        let Some(table_value) = tables.get(table) else {
            continue;
        };
        let rows: &[serde_json::Value] = match table_value {
            serde_json::Value::Array(rows) => rows.as_slice(),
            serde_json::Value::Object(_) => std::slice::from_ref(table_value),
            serde_json::Value::Null => continue,
            _ => return Err(format!("备份字段 {} 结构无效", table)),
        };
        for (index, row) in rows.iter().enumerate() {
            let Some(value) = row.get(field) else {
                continue;
            };
            let Some(raw) = value.as_str() else {
                if value.is_null() {
                    continue;
                }
                return Err(format!(
                    "备份字段 {}[{}].{} 必须是加密字符串",
                    table, index, field
                ));
            };
            if raw.is_empty() {
                continue;
            }
            if !crate::vault::is_encrypted(raw) {
                return Err(format!(
                    "备份字段 {}[{}].{} 必须是带 enc 前缀的密文",
                    table, index, field
                ));
            }
            crate::vault::decrypt_field_any(raw, dek)
                .map_err(|_| format!("备份字段 {}[{}].{} 密文认证失败", table, index, field))?;
        }
    }
    Ok(())
}

fn reject_unauthenticated_ciphertext_fields(tables: &serde_json::Value) -> Result<(), String> {
    const FIELDS: &[(&str, &str)] = &[
        ("digital_assets", "account_identifier"),
        ("digital_assets", "account_description"),
        ("digital_assets", "heir_name"),
        ("digital_assets", "heir_contact"),
        ("digital_assets", "notes"),
        ("digital_assets", "credential_enc"),
        ("user_profile", "full_name"),
        ("user_profile", "phone"),
        ("user_profile", "email"),
        ("user_profile", "id_last4"),
        ("user_profile", "emergency_contact_name"),
        ("user_profile", "emergency_contact_phone"),
        ("user_profile", "emergency_contact_relation"),
        ("user_profile", "emergency_contact_email"),
        ("user_profile", "heir_primary_name"),
        ("user_profile", "heir_primary_phone"),
        ("user_profile", "heir_primary_relation"),
        ("user_profile", "heir_primary_email"),
        ("heirs", "name"),
        ("heirs", "relation"),
        ("heirs", "phone"),
        ("heirs", "email"),
        ("heirs", "share_note"),
        ("smtp_config", "password_enc"),
        ("custody_config", "backup_location_enc"),
        ("custody_config", "recovery_code_location_enc"),
        ("assistant_config", "api_key_enc"),
        ("heartbeat_config", "token"),
        ("security_config", "external_config_binding"),
    ];
    for &(table, field) in FIELDS {
        let Some(table_value) = tables.get(table) else {
            continue;
        };
        let rows: &[serde_json::Value] = match table_value {
            serde_json::Value::Array(rows) => rows.as_slice(),
            serde_json::Value::Object(_) => std::slice::from_ref(table_value),
            serde_json::Value::Null => continue,
            _ => return Err(format!("备份字段 {} 结构无效", table)),
        };
        for (index, row) in rows.iter().enumerate() {
            let Some(raw) = row.get(field).and_then(serde_json::Value::as_str) else {
                continue;
            };
            if !raw.is_empty() && crate::vault::is_encrypted(raw) {
                return Err(format!(
                    "无 security_config 的备份字段 {}[{}].{} 不得带未认证密文",
                    table, index, field
                ));
            }
        }
    }
    Ok(())
}

fn authenticate_backup_security(
    version: i64,
    tables: &serde_json::Value,
    source_password: Option<&str>,
    allow_current_key: bool,
    allow_empty_recovery_seal: bool,
) -> Result<Option<zeroize::Zeroizing<[u8; 32]>>, String> {
    let Some(security) = tables.get("security_config") else {
        return Ok(None);
    };
    let Some(sec) = security.as_object() else {
        if security.is_null() {
            return Ok(None);
        }
        return Err("备份 security_config 必须是对象或 null".to_string());
    };
    let password = source_password.filter(|password| !password.is_empty());
    let salt = sec
        .get("kdf_salt")
        .and_then(serde_json::Value::as_str)
        .filter(|salt| !salt.trim().is_empty())
        .ok_or("备份 security_config 缺少有效的 kdf_salt")?;
    crate::crypto::parse_kdf_params(salt)
        .map_err(|_| "备份 security_config 的 kdf_salt 无效".to_string())?;
    let derived_kek = match password {
        Some(password) => Some(crate::crypto::derive_from_salt(password, salt)?),
        None => None,
    };
    let verifier = sec
        .get("verifier")
        .and_then(serde_json::Value::as_str)
        .filter(|verifier| !verifier.is_empty())
        .ok_or("备份 security_config 缺少有效的 verifier")?;
    if !crate::vault::is_encrypted(verifier) {
        return Err("备份 security_config 的 verifier 必须是密文".to_string());
    }
    let fallback_key = || {
        if allow_current_key {
            crate::vault::get_key()
        } else {
            None
        }
    };
    let dek = if allow_current_key && derived_kek.is_none() {
        fallback_key().ok_or("带 security_config 的备份必须提供源主密码")?
    } else {
        match sec.get("wrapped_dek") {
            Some(value) => {
                let wrapped = value
                    .as_str()
                    .ok_or("备份 security_config 的 wrapped_dek 必须是字符串")?;
                if wrapped.is_empty() {
                    return Err("备份 security_config 的 wrapped_dek 不能为空".to_string());
                }
                let kek = derived_kek
                    .clone()
                    .or_else(fallback_key)
                    .ok_or("带 security_config 的备份必须提供源主密码")?;
                crate::vault::unwrap_data_key(wrapped, &kek)
                    .map_err(|_| "备份 security_config 的 wrapped_dek 解包失败".to_string())?
            }
            None if version < 5 => derived_kek
                .or_else(fallback_key)
                .ok_or("带 security_config 的备份必须提供源主密码")?,
            None => return Err("备份 security_config 缺少 wrapped_dek".to_string()),
        }
    };
    if !crate::vault::verify_key(verifier, &dek) {
        return Err("备份 security_config 的 verifier 认证失败".to_string());
    }
    if let Some(seal) = sec.get("recovery_seal") {
        let seal = seal
            .as_str()
            .ok_or("备份 security_config 的 recovery_seal 必须是字符串")?;
        if seal.is_empty() && !allow_empty_recovery_seal {
            return Err("备份 security_config 的 recovery_seal 不能为空".to_string());
        }
        if !seal.is_empty() {
            crate::recovery::validate_recovery_seal_structure(seal)
                .map_err(|_| "备份 security_config 的 recovery_seal 无效".to_string())?;
        }
    }
    validate_encrypted_backup_fields(tables, &dek)?;
    let _ = version;
    Ok(Some(dek))
}

/// 导入备份：覆盖模式（历史行为，保留给既有调用方与回归测试）。
///
/// 新代码应使用 `import_backup_json_db_with_mode`，由命令层按用户选择决定模式。
/// 生产路径已全部切到带模式的版本，这里只被测试与场景矩阵调用。
#[cfg(test)]
pub fn import_backup_json_db(content: &str) -> Result<i64, String> {
    import_backup_json_db_with_mode(content, ImportMode::Replace)
}

/// 导入备份：按 `mode` 合并或覆盖用户数据与安全配置；导入后强制重新解锁。
///
/// 含 `security_config` 的生产导入必须通过带密码的 helper。测试保留旧 wrapper，
/// 仅用于既有 legacy round-trip 场景，不由 Tauri command 调用。
#[cfg(test)]
pub fn import_backup_json_db_with_mode(content: &str, mode: ImportMode) -> Result<i64, String> {
    import_backup_json_db_internal(content, mode, None, cfg!(test), cfg!(test))
}

pub fn import_backup_json_db_with_mode_and_password(
    content: &str,
    mode: ImportMode,
    source_password: Option<&str>,
) -> Result<i64, String> {
    import_backup_json_db_internal(content, mode, source_password, false, false)
}

fn import_backup_json_db_internal(
    content: &str,
    mode: ImportMode,
    source_password: Option<&str>,
    allow_current_key: bool,
    allow_empty_recovery_seal: bool,
) -> Result<i64, String> {
    use serde_json::Value;

    let v: Value = serde_json::from_str(content).map_err(|e| format!("备份文件解析失败: {}", e))?;
    if v.get("app").and_then(|x| x.as_str()) != Some("digilegacy") {
        return Err("不是有效的归零备份文件".to_string());
    }
    // v1 备份仍然可导入（向后兼容）：新表缺失时保持本地现状，
    // 旧备份里没有的 heartbeat/platform_overrides 按「未提供」处理。
    let version = v.get("version").and_then(|x| x.as_i64()).unwrap_or(0);
    if !(1..=5).contains(&version) {
        return Err("备份版本不受支持".to_string());
    }
    let tables = v.get("tables").ok_or("备份缺少数据表")?;
    validate_backup_row_shapes(tables)?;

    let backup_dek = authenticate_backup_security(
        version,
        tables,
        source_password,
        allow_current_key,
        allow_empty_recovery_seal,
    )?;
    if backup_dek.is_none() {
        reject_unauthenticated_ciphertext_fields(tables)?;
    }
    let backup_has_security = backup_dek.is_some();
    let external_config_bound = match backup_dek.as_ref() {
        Some(dek) => validate_external_config_binding(tables, dek)?,
        None => false,
    };
    validate_external_delivery_config(tables)?;
    let local_security = get_security_config_db()?;
    let local_has_vault = local_security.is_some();
    if local_has_vault {
        ensure_vault_readable_db()?;
    }

    let mut import_security_config = backup_has_security;
    if mode == ImportMode::Merge && backup_has_security {
        let Some(local_security) = local_security.as_ref() else {
            return Err(
                "含 security_config 的备份不能合并到未配置保险库，请改用覆盖导入".to_string(),
            );
        };
        let local_key = crate::vault::get_key().ok_or("当前保险库未解锁")?;
        let backup_key = backup_dek.as_ref().ok_or("备份数据密钥缺失")?;
        if *local_key != **backup_key {
            return Err("合并导入的数据密钥与当前库不同，已拒绝修改安全配置".to_string());
        }
        let security = tables
            .get("security_config")
            .and_then(|value| value.as_object())
            .ok_or("备份 security_config 必须是对象")?;
        let backup_salt = security
            .get("kdf_salt")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let backup_verifier = security
            .get("verifier")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if backup_salt != local_security.0 || backup_verifier != local_security.1 {
            return Err(
                "合并导入的 security_config 与当前库不同，已拒绝以保护本地密钥".to_string(),
            );
        }
        import_security_config = false;
    }
    let encrypt_plaintext_on_import = local_has_vault && !backup_has_security;
    if encrypt_plaintext_on_import && !crate::vault::is_unlocked() {
        return Err(
            "该备份不含加密配置，而当前库已加密且处于锁定状态。请先解锁再导入，否则明文数据会绕过加密"
                .to_string(),
        );
    }
    let protect = |value: String| -> Result<String, String> {
        if !encrypt_plaintext_on_import || value.is_empty() {
            return Ok(value);
        }
        crate::vault::encrypt_if_needed(&value)
    };

    let sval = |val: &Value, k: &str| -> String {
        val.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    let ival = |val: &Value, k: &str| -> i64 { val.get(k).and_then(|x| x.as_i64()).unwrap_or(0) };
    let fval = |val: &Value, k: &str| -> f64 { val.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) };
    let oval = |val: &Value, k: &str| -> Option<String> {
        val.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
    };

    let assets = tables
        .get("digital_assets")
        .and_then(|x| x.as_array())
        .ok_or("备份缺少资产数据")?;
    // 缺失 = 旧格式，允许为空；**存在但不是数组 = 损坏，必须拒绝**。
    // 旧写法用 `and_then(as_array).unwrap_or_default()`，一个手改过的
    // `"user_profile": "x"` 会被静默当成空数组，导入"成功"却把档案清空了。
    let array_or_missing = |key: &str| -> Result<Vec<Value>, String> {
        match tables.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => Ok(items.clone()),
            Some(_) => Err(format!("备份字段 {} 必须是数组", key)),
        }
    };
    let profiles = array_or_missing("user_profile")?;
    let check_ins = array_or_missing("check_in_log")?;

    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();

    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("开启事务失败: {}", e))?;
    let result = (|| -> Result<i64, String> {
        if mode == ImportMode::Replace {
            conn.execute("DELETE FROM digital_assets", [])
                .map_err(|e| format!("清空资产失败: {}", e))?;
            conn.execute("DELETE FROM user_profile", [])
                .map_err(|e| format!("清空档案失败: {}", e))?;
            conn.execute("DELETE FROM check_in_log", [])
                .map_err(|e| format!("清空记录失败: {}", e))?;
        }

        let mut count = 0i64;
        for (idx, a) in assets.iter().enumerate() {
            // 备份文件是外部输入：类别/处置/金额必须与命令层同样校验，
            // 否则一个手改过的 JSON 就能把非法枚举和负金额写进统计与报告。
            let category = sval(a, "category");
            validate_category(&category)
                .map_err(|e| format!("备份第 {} 条资产类别非法: {}", idx + 1, e))?;
            let disposal = sval(a, "disposal_method");
            validate_disposal(&disposal)
                .map_err(|e| format!("备份第 {} 条资产处置非法: {}", idx + 1, e))?;
            let value = fval(a, "estimated_value");
            if !value.is_finite() || value < 0.0 || value > 1e12 {
                return Err(format!(
                    "备份第 {} 条资产估值非法（要求 0..1e12 的有限数值，实际 {}）",
                    idx + 1,
                    value
                ));
            }
            // 合并模式不能复用备份里的 id：本地可能已有同 id 资产，
            // 直接 INSERT 会撞主键并把整个导入回滚。让 SQLite 分配新 id。
            let backup_id = ival(a, "id");
            let target_id = if mode == ImportMode::Replace {
                backup_id
            } else {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM digital_assets WHERE id=?1",
                        params![backup_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken == 0 && backup_id > 0 {
                    backup_id
                } else {
                    0
                }
            };
            conn.execute(
                "INSERT INTO digital_assets (id, platform_id, account_identifier, account_description, category, estimated_value, value_currency, disposal_method, heir_name, heir_contact, notes, credential_enc, is_confirmed, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    if target_id > 0 { Some(target_id) } else { None },
                    ival(a, "platform_id"),
                    protect(sval(a, "account_identifier"))?,
                    protect(sval(a, "account_description"))?,
                    category,
                    value,
                    { let c = sval(a, "value_currency"); if c.is_empty() { "CNY".to_string() } else { c } },
                    disposal,
                    protect(sval(a, "heir_name"))?,
                    protect(sval(a, "heir_contact"))?,
                    protect(sval(a, "notes"))?,
                    protect(sval(a, "credential_enc"))?,
                    ival(a, "is_confirmed"),
                    sval(a, "created_at"),
                    sval(a, "updated_at"),
                ],
            ).map_err(|e| format!("写入资产失败: {}", e))?;
            count += 1;
        }

        // 合并模式不清空档案/签到：只补「本地还没有」的档案行，
        // 已有档案保持现状（否则一次合并导入会把当前联系人信息换掉）。
        let profile_id = |p: &Value| ival(p, "id");
        for p in &profiles {
            let pid = profile_id(p);
            if mode == ImportMode::Merge {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM user_profile WHERE id=?1",
                        params![pid],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken > 0 {
                    continue;
                }
            }
            conn.execute(
                "INSERT INTO user_profile (id, full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email, check_in_interval_days, last_check_in, dms_enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    pid,
                    protect(sval(p, "full_name"))?,
                    protect(sval(p, "phone"))?,
                    protect(sval(p, "email"))?,
                    protect(sval(p, "id_last4"))?,
                    protect(sval(p, "emergency_contact_name"))?,
                    protect(sval(p, "emergency_contact_phone"))?,
                    protect(sval(p, "emergency_contact_relation"))?,
                    protect(sval(p, "emergency_contact_email"))?,
                    protect(sval(p, "heir_primary_name"))?,
                    protect(sval(p, "heir_primary_phone"))?,
                    protect(sval(p, "heir_primary_relation"))?,
                    protect(sval(p, "heir_primary_email"))?,
                    ival(p, "check_in_interval_days"),
                    oval(p, "last_check_in"),
                    ival(p, "dms_enabled"),
                    sval(p, "created_at"),
                    sval(p, "updated_at"),
                ],
            ).map_err(|e| format!("写入档案失败: {}", e))?;
        }

        // 合并模式只追加备份里比本地更新的确认记录（id 冲突跳过），
        // 覆盖模式沿用原行为整表写入。
        for c in &check_ins {
            let cid = ival(c, "id");
            if mode == ImportMode::Merge {
                let taken: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM check_in_log WHERE id=?1",
                        params![cid],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if taken > 0 {
                    continue;
                }
            }
            conn.execute(
                "INSERT INTO check_in_log (id, checked_at, status, note) VALUES (?1, ?2, ?3, ?4)",
                params![
                    cid,
                    sval(c, "checked_at"),
                    sval(c, "status"),
                    sval(c, "note")
                ],
            )
            .map_err(|e| format!("写入记录失败: {}", e))?;
        }

        if let Some(d) = tables.get("dms_config").filter(|x| x.is_object()) {
            conn.execute(
                "UPDATE dms_config SET enabled=?1, check_in_interval_days=?2, stage1_wait_days=?3, stage2_wait_days=?4, stage3_wait_days=?5, last_check_in=?6 WHERE id=1",
                params![
                    ival(d, "enabled"),
                    ival(d, "check_in_interval_days"),
                    ival(d, "stage1_wait_days"),
                    ival(d, "stage2_wait_days"),
                    // 接收旧备份里的该值但不再使用，保证导入不因缺字段失败
                    ival(d, "stage3_wait_days"),
                    oval(d, "last_check_in"),
                ],
            ).map_err(|e| format!("写入DMS配置失败: {}", e))?;
        }

        if import_security_config {
            if let Some(sec) = tables.get("security_config").filter(|x| x.is_object()) {
                // v3 备份带恢复封印：与新主密钥（备份里的 kdf_salt/verifier）成套，
                // 一起恢复才能保留「记得恢复码就能找回」的能力。
                // v1/v2 备份不含封印：必须显式清空，否则本地旧封印指向旧设备的主密钥，
                // 会让恢复码解出错误密钥，恢复链路静默断裂。
                let seal = sval(sec, "recovery_seal");
                conn.execute(
                    "INSERT INTO security_config (id, kdf_salt, verifier, created_at, recovery_seal, wrapped_dek) VALUES (1, ?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET kdf_salt=?1, verifier=?2, recovery_seal=?4, wrapped_dek=?5",
                    params![
                        sval(sec, "kdf_salt"),
                        sval(sec, "verifier"),
                        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        seal,
                        sval(sec, "wrapped_dek"),
                    ],
                ).map_err(|e| format!("写入安全配置失败: {}", e))?;
            }
        }

        // 只有通过 DEK 认证绑定的备份才允许恢复外连配置；旧备份在覆盖模式下清空，
        // 合并模式则保留本机配置并要求用户重新确认。未认证配置绝不能继续外送。
        if mode == ImportMode::Replace && !external_config_bound {
            clear_external_delivery_config(conn)?;
        }
        if external_config_bound {
            if let Some(hb) = tables.get("heartbeat_config").filter(|x| x.is_object()) {
                let endpoint = sval(hb, "endpoint");
                if !endpoint.trim().is_empty() {
                    crate::heartbeat::validate_heartbeat_endpoint(&endpoint)?;
                }
                conn.execute(
                "INSERT INTO heartbeat_config (id, enabled, token, endpoint) VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET enabled=?1, token=?2, endpoint=?3",
                params![
                    ival(hb, "enabled"),
                    protect(sval(hb, "token"))?,
                    sval(hb, "endpoint")
                ],
            )
                .map_err(|e| format!("写入心跳配置失败: {}", e))?;
            }
        }

        // v4：继承人名单。Merge 模式下只追加「本地还没有的」那几条
        // （按 name+relation 判重），Replace 模式整表替换。
        if let Some(heirs) = tables.get("heirs") {
            if !heirs.is_array() && !heirs.is_null() {
                return Err("备份字段 heirs 必须是数组".to_string());
            }
            if let Some(list) = heirs.as_array() {
                if mode == ImportMode::Replace {
                    conn.execute("DELETE FROM heirs", [])
                        .map_err(|e| format!("清空继承人失败: {}", e))?;
                }
                for (idx, h) in list.iter().enumerate() {
                    let name = protect(sval(h, "name"))?;
                    if name.trim().is_empty() {
                        continue;
                    }
                    if mode == ImportMode::Merge {
                        // 简单判重：同名同关系视为同一个人（密文比较不可用，
                        // 因此比较解密后的明文；锁定态下 protect 已拒绝明文备份）
                        let existing = get_heirs_db()?;
                        if existing.iter().any(|e| e.name == name) {
                            continue;
                        }
                    }
                    conn.execute(
                        "INSERT INTO heirs (name, relation, phone, email, share_note, is_executor, sort_order, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                        params![
                            name,
                            protect(sval(h, "relation"))?,
                            protect(sval(h, "phone"))?,
                            protect(sval(h, "email"))?,
                            protect(sval(h, "share_note"))?,
                            ival(h, "is_executor"),
                            idx as i64,
                            Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                        ],
                    )
                    .map_err(|e| format!("写入继承人失败: {}", e))?;
                }
            }
        }

        // v4：审核留痕。Merge 模式跳过已存在的同指纹记录，避免重复堆积。
        if let Some(records) = tables.get("review_records") {
            if !records.is_array() && !records.is_null() {
                return Err("备份字段 review_records 必须是数组".to_string());
            }
            if let Some(list) = records.as_array() {
                if mode == ImportMode::Replace {
                    conn.execute("DELETE FROM review_records", [])
                        .map_err(|e| format!("清空审核记录失败: {}", e))?;
                }
                for r in list {
                    let profile_id = sval(r, "profile_id");
                    let item_key = sval(r, "item_key");
                    let fingerprint = sval(r, "fingerprint");
                    if profile_id.is_empty() || item_key.is_empty() {
                        continue;
                    }
                    if mode == ImportMode::Merge {
                        // 同一条目同一指纹的记录视为已经存在（留痕只增，不重复）
                        let exists: i64 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM review_records
                                 WHERE profile_id=?1 AND item_key=?2 AND fingerprint=?3
                                   AND ai_verdict=?4 AND COALESCE(human_verdict,'')=?5",
                                params![
                                    profile_id,
                                    item_key,
                                    fingerprint,
                                    sval(r, "ai_verdict"),
                                    sval(r, "human_verdict"),
                                ],
                                |x| x.get(0),
                            )
                            .unwrap_or(0);
                        if exists > 0 {
                            continue;
                        }
                    }
                    conn.execute(
                        "INSERT INTO review_records (profile_id, item_key, fingerprint, ai_verdict, ai_note, human_verdict, human_note, decided_by, created_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        params![
                            profile_id,
                            item_key,
                            fingerprint,
                            sval(r, "ai_verdict"),
                            sval(r, "ai_note"),
                            { let h = sval(r, "human_verdict"); if h.is_empty() { None } else { Some(h) } },
                            sval(r, "human_note"),
                            sval(r, "decided_by"),
                            sval(r, "created_at"),
                        ],
                    )
                    .map_err(|e| format!("写入审核记录失败: {}", e))?;
                }
            }
        }

        // v4：SMTP / 保管位置 / 助手配置。旧备份没有这些块 → 保持本地现状。
        if external_config_bound {
            if let Some(smtp) = tables.get("smtp_config").filter(|x| x.is_object()) {
                conn.execute(
                "INSERT INTO smtp_config (id, host, port, from_addr, username, password_enc, allow_insecure, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET host=?1, port=?2, from_addr=?3, username=?4, password_enc=?5, allow_insecure=?6, updated_at=?7",
                params![
                    sval(smtp, "host"),
                    ival(smtp, "port"),
                    sval(smtp, "from_addr"),
                    sval(smtp, "username"),
                    protect(sval(smtp, "password_enc"))?,
                    ival(smtp, "allow_insecure"),
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
            )
            .map_err(|e| format!("写入 SMTP 配置失败: {}", e))?;
            }
        }

        if let Some(c) = tables.get("custody_config").filter(|x| x.is_object()) {
            conn.execute(
                "INSERT INTO custody_config (id, backup_location_enc, recovery_code_location_enc, updated_at)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET backup_location_enc=?1, recovery_code_location_enc=?2, updated_at=?3",
                params![
                    protect(sval(c, "backup_location_enc"))?,
                    protect(sval(c, "recovery_code_location_enc"))?,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
            )
            .map_err(|e| format!("写入保管位置失败: {}", e))?;
        }

        // 恢复包元数据随备份往返：否则换机后家人手里的份额永远配不上新库
        // （份额本身不落库，但门限/指纹/KDF 盐必须跟着走，否则无法判断批次与口令模式）。
        crate::heir_recovery::import_meta_json(conn, tables.get("heir_recovery"))?;

        if external_config_bound {
            if let Some(a) = tables.get("assistant_config").filter(|x| x.is_object()) {
                conn.execute(
                "INSERT INTO assistant_config (id, enabled, endpoint, model, api_key_enc, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET enabled=?1, endpoint=?2, model=?3, api_key_enc=?4, updated_at=?5",
                params![
                    ival(a, "enabled"),
                    sval(a, "endpoint"),
                    sval(a, "model"),
                    protect(sval(a, "api_key_enc"))?,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ],
                )
                .map_err(|e| format!("写入助手配置失败: {}", e))?;
            }
        }

        if let Some(overrides) = tables.get("platform_overrides").and_then(|x| x.as_array()) {
            conn.execute("DELETE FROM platform_overrides", [])
                .map_err(|e| format!("清空政策补丁失败: {}", e))?;
            for o in overrides {
                conn.execute(
                    "INSERT INTO platform_overrides (id, name, name_en, category, subtype, has_property, has_personality, inheritable, required_documents, process_description, estimated_difficulty, official_policy_url, estimated_time_days, notes, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                    params![
                        ival(o, "id"),
                        sval(o, "name"),
                        sval(o, "name_en"),
                        sval(o, "category"),
                        sval(o, "subtype"),
                        ival(o, "has_property"),
                        ival(o, "has_personality"),
                        ival(o, "inheritable"),
                        sval(o, "required_documents"),
                        sval(o, "process_description"),
                        sval(o, "estimated_difficulty"),
                        sval(o, "official_policy_url"),
                        sval(o, "estimated_time_days"),
                        sval(o, "notes"),
                        Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    ],
                ).map_err(|e| format!("写入政策补丁失败: {}", e))?;
            }
        }

        Ok(count)
    })();

    match result {
        Ok(n) => {
            conn.execute_batch("COMMIT")
                .map_err(|e| format!("提交事务失败: {}", e))?;
            crate::vault::clear_key();
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 将存量明文敏感字段加密（幂等；返回改动的资产行数）
pub fn encrypt_existing_data_db() -> Result<i64, String> {
    if !crate::vault::is_unlocked() {
        return Err("请先解锁".to_string());
    }
    let guard = get_conn()?;
    let conn = guard.as_ref().unwrap();
    let mut changed: i64 = 0;

    let mut stmt = conn.prepare(
        "SELECT id, account_identifier, account_description, heir_name, heir_contact, notes, credential_enc FROM digital_assets",
    ).map_err(|e| format!("查询资产失败: {}", e))?;
    let rows: Vec<(i64, String, String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, Option<String>>(6)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| format!("读取资产失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集资产失败: {}", e))?;

    for (id, ai, ad, hn, hc, nt, cr) in rows {
        let nai = crate::vault::encrypt_if_needed(&ai)?;
        let nad = crate::vault::encrypt_if_needed(&ad)?;
        let nhn = crate::vault::encrypt_if_needed(&hn)?;
        let nhc = crate::vault::encrypt_if_needed(&hc)?;
        let nnt = crate::vault::encrypt_if_needed(&nt)?;
        let ncr = crate::vault::encrypt_if_needed(&cr)?;
        if nai != ai || nad != ad || nhn != hn || nhc != hc || nnt != nt || ncr != cr {
            conn.execute(
                "UPDATE digital_assets SET account_identifier=?1, account_description=?2, heir_name=?3, heir_contact=?4, notes=?5, credential_enc=?6 WHERE id=?7",
                params![nai, nad, nhn, nhc, nnt, ncr, id],
            ).map_err(|e| format!("加密存量资产失败: {}", e))?;
            changed += 1;
        }
    }

    if let Some((pid, vals)) = select_profile_sensitive(conn)? {
        let mut new_vals = Vec::with_capacity(vals.len());
        let mut dirty = false;
        for v in &vals {
            let nv = crate::vault::encrypt_if_needed(v)?;
            if nv != *v {
                dirty = true;
            }
            new_vals.push(nv);
        }
        if dirty {
            update_profile_sensitive(conn, pid, &new_vals)
                .map_err(|e| format!("加密存量档案失败: {}", e))?;
            changed += 1;
        }
    }

    // 继承人名单同样必须收编：否则开启加密后它仍是明文落库。
    if encrypt_existing_heirs(conn)? {
        changed += 1;
    }

    Ok(changed)
}

/// 把 heirs 表的明文值就地加密（启用加密时的存量收编）。
fn encrypt_existing_heirs(conn: &Connection) -> Result<bool, String> {
    let rows: Vec<(i64, String, String, String, String, String)> = conn
        .prepare(
            "SELECT id, COALESCE(name,''), COALESCE(relation,''), COALESCE(phone,''),
                    COALESCE(email,''), COALESCE(share_note,'') FROM heirs",
        )
        .map_err(|e| format!("查询继承人失败: {}", e))?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .map_err(|e| format!("读取继承人失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("收集继承人失败: {}", e))?;
    let mut dirty = false;
    for (id, name, relation, phone, email, share_note) in rows {
        let n_name = crate::vault::encrypt_if_needed(&name)?;
        let n_rel = crate::vault::encrypt_if_needed(&relation)?;
        let n_phone = crate::vault::encrypt_if_needed(&phone)?;
        let n_email = crate::vault::encrypt_if_needed(&email)?;
        let n_share = crate::vault::encrypt_if_needed(&share_note)?;
        if n_name != name
            || n_rel != relation
            || n_phone != phone
            || n_email != email
            || n_share != share_note
        {
            conn.execute(
                "UPDATE heirs SET name=?1, relation=?2, phone=?3, email=?4, share_note=?5 WHERE id=?6",
                params![n_name, n_rel, n_phone, n_email, n_share, id],
            )
            .map_err(|e| format!("加密存量继承人失败: {}", e))?;
            dirty = true;
        }
    }
    Ok(dirty)
}
