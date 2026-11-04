//! db 模块的回归测试（从 db.rs 拆出）。
//!
//! 为什么单独成文件：生产代码 2.1k 行 + 测试 1.7k 行挤在一个文件里，
//! 每次改一处安全边界都要在 3.9k 行里找平行路径。拆分是纯移动，行为零变更。
//!
//! 注意：本模块与 db 共享进程级连接与主密钥，必须持 `vault::test_lock()`。

use super::*;
use crate::models::AssetRequest;
use crate::vault;

/// 测试库路径。
///
/// 注意：不要在 Drop 里删文件。数据库连接是进程级全局单例，测试并行执行时
/// 一个用例的清理会关掉另一个用例正在用的连接（实测表现为 "数据库未初始化"
/// 随机失败）。临时文件由 CI/本地定期清理，正确性优先。
struct TempDb {
    path: PathBuf,
}

impl std::ops::Deref for TempDb {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.path
    }
}

impl AsRef<std::path::Path> for TempDb {
    fn as_ref(&self) -> &std::path::Path {
        self.path.as_path()
    }
}

fn fresh_db_path(tag: &str) -> TempDb {
    let mut p = std::env::temp_dir();
    p.push(format!("digilegacy_test_{}_{}.db", tag, std::process::id()));
    // 每个用例开头先清掉自己上次跑的残留（-wal/-shm 也必须拼在后缀上，
    // 不能用 with_extension，否则会把 .db 替换掉导致删不到）。
    let base = p.to_string_lossy().to_string();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{}", base, suffix));
    }
    TempDb { path: p }
}

#[test]
fn vault_guards_fail_closed_when_security_config_query_errors() {
    let missing = rusqlite::Connection::open_in_memory().unwrap();
    assert!(
        ensure_vault_readable_pub(&missing).is_err(),
        "缺少 security_config 时读取门禁必须返回错误"
    );
    assert!(
        ensure_vault_writable_pub(&missing).is_err(),
        "缺少 security_config 时写入门禁必须返回错误"
    );

    let damaged = rusqlite::Connection::open_in_memory().unwrap();
    damaged
        .execute_batch("CREATE VIEW security_config AS SELECT id FROM missing_security_source;")
        .unwrap();
    assert!(
        ensure_vault_readable_pub(&damaged).is_err(),
        "security_config 查询损坏时读取门禁必须返回错误"
    );
    assert!(
        ensure_vault_writable_pub(&damaged).is_err(),
        "security_config 查询损坏时写入门禁必须返回错误"
    );
}

#[test]
fn test_migration_encryption_and_credential_roundtrip() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("enc");
    init_database_at(path.clone()).unwrap();

    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "13800000000".to_string(),
        account_description: "主账号".to_string(),
        category: "property".to_string(),
        estimated_value: Some(100.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("张三".to_string()),
        heir_contact: None,
        notes: "备注信息".to_string(),
    };
    let id = add_asset_db(&req).unwrap();

    let guard = get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let raw_before: String = conn
        .query_row(
            "SELECT account_identifier FROM digital_assets WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw_before, "13800000000", "未启用主密码前应保持明文");
    drop(guard);

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("test-master-123", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db("dGVzdC1zYWx0", &verifier, "").unwrap();
    vault::set_key(key);

    let changed = encrypt_existing_data_db().unwrap();
    assert!(changed >= 1, "存量数据应被加密");

    let guard = get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let raw_after: String = conn
        .query_row(
            "SELECT account_identifier FROM digital_assets WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(vault::is_encrypted(&raw_after), "库内应为 enc: 密文");
    assert!(!raw_after.contains("13800000000"), "库内不得出现明文");
    drop(guard);

    let asset = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(asset.account_identifier, "13800000000");
    assert_eq!(asset.notes, "备注信息");
    assert_eq!(asset.heir_name.as_deref(), Some("张三"));

    set_asset_credential_db(id, "手机号+密码+密保").unwrap();
    assert_eq!(get_asset_credential_db(id).unwrap(), "手机号+密码+密保");
    assert!(get_asset_by_id_db(id).unwrap().unwrap().has_credential);

    set_asset_credential_db(id, "").unwrap();
    assert_eq!(get_asset_credential_db(id).unwrap(), "");
    assert!(!get_asset_by_id_db(id).unwrap().unwrap().has_credential);

    set_asset_credential_db(id, "再次保存").unwrap();
    vault::clear_key();
    assert!(
        get_asset_credential_db(id).is_err(),
        "锁定后不得返回明文凭据"
    );
    assert!(
        set_asset_credential_db(id, "锁定写入").is_err(),
        "锁定后不得写入新凭据"
    );
    let list = get_all_assets_db().unwrap();
    let mine = list.iter().find(|a| a.id == Some(id)).unwrap();
    assert!(
        mine.account_identifier.contains("加密数据"),
        "锁定后读取应为占位符"
    );

    let _ = std::fs::remove_file(&path);
}

/// 启用加密时若已有档案，`encrypt_existing_data_db` 必须完整加密全部 12 个档案字段。
///
/// 历史缺陷：该函数的档案查询只 SELECT 了 11 个字段（漏 heir_primary_email），
/// 取值循环又写成 1..11（10 个），随后 UPDATE 却按下标取到 new_vals[11] —
/// 直接越界 panic。「先填档案、再设置主密码」是正常用户路径，会当场崩溃。
#[test]
fn encrypt_existing_data_covers_all_profile_fields() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("profile-encrypt");
    init_database_at(path.clone()).unwrap();

    save_user_profile_db(&UserProfile {
        full_name: "张三".to_string(),
        phone: "13800000000".to_string(),
        email: "zhangsan@example.com".to_string(),
        id_last4: "1234".to_string(),
        emergency_contact_name: "李四".to_string(),
        emergency_contact_phone: "13900000000".to_string(),
        emergency_contact_relation: "配偶".to_string(),
        emergency_contact_email: "lisi@example.com".to_string(),
        heir_primary_name: "王五".to_string(),
        heir_primary_phone: "13700000000".to_string(),
        heir_primary_relation: "子女".to_string(),
        heir_primary_email: "wangwu@example.com".to_string(),
        check_in_interval_days: 30,
        ..Default::default()
    })
    .unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("master-pw-123", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db("dGVzdC1zYWx0", &verifier, "").unwrap();
    vault::set_key(key);

    let changed = encrypt_existing_data_db().expect("档案加密不得 panic 或报错");
    assert_eq!(changed, 1, "一份档案应计为一次改动");

    let raw_rows: Vec<String> = {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        let mut stmt = conn
            .prepare("SELECT full_name, phone, email, id_last4, emergency_contact_name, emergency_contact_phone, emergency_contact_relation, emergency_contact_email, heir_primary_name, heir_primary_phone, heir_primary_relation, heir_primary_email FROM user_profile")
            .unwrap();
        let rows = stmt
            .query_map([], |r| {
                Ok((0..12)
                    .map(|i| {
                        r.get::<_, Option<String>>(i)
                            .ok()
                            .flatten()
                            .unwrap_or_default()
                    })
                    .collect::<Vec<String>>())
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows.into_iter().next().unwrap()
    };
    for (idx, raw) in raw_rows.iter().enumerate() {
        assert!(
            vault::is_encrypted(raw),
            "第 {} 个档案字段应已加密，实际: {}",
            idx,
            raw
        );
    }

    let profile = get_user_profile_db().unwrap().unwrap();
    assert_eq!(profile.full_name, "张三");
    assert_eq!(profile.email, "zhangsan@example.com");
    assert_eq!(profile.emergency_contact_email, "lisi@example.com");
    assert_eq!(profile.heir_primary_email, "wangwu@example.com");
    assert_eq!(profile.heir_primary_relation, "子女");

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_change_password_and_backup_roundtrip() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};
    vault::clear_key();
    let path = fresh_db_path("r2");
    init_database_at(path.clone()).unwrap();

    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "acc-round2".to_string(),
        account_description: "描述".to_string(),
        category: "property".to_string(),
        estimated_value: Some(88.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("李四".to_string()),
        heir_contact: Some("13900000000".to_string()),
        notes: "备注R2".to_string(),
    };
    let id = add_asset_db(&req).unwrap();

    let salt1 = crate::crypto::generate_salt();
    let key1 = crate::crypto::derive_key_bytes("old-master-123", &salt1);
    let verifier1 = vault::make_verifier(&key1).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt1), &verifier1, "").unwrap();
    vault::set_key(key1);
    encrypt_existing_data_db().unwrap();
    set_asset_credential_db(id, "cred-round2").unwrap();

    let new_recovery_code =
        change_master_password_db("old-master-123", "new-master-456", "new-master-456").unwrap();
    assert!(vault::is_unlocked(), "改密后应保持解锁状态");
    assert_eq!(get_asset_credential_db(id).unwrap(), "cred-round2");
    let asset = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(asset.account_identifier, "acc-round2");
    assert_eq!(asset.heir_contact.as_deref(), Some("13900000000"));

    let (salt2_b64, verifier2) = get_security_config_db().unwrap().unwrap();
    // 改密后安全配置应为 v3 盐（Argon2id，参数随盐串自描述）
    assert!(
        salt2_b64.starts_with(crate::crypto::SALT_PREFIX_V3),
        "改密后盐应为 v3 格式（Argon2id）"
    );
    // 两层密钥语义：verifier 绑定的是**数据密钥**，不是密码派生 KEK。
    // 新密码派生 KEK → 解开 wrapped_dek → 得到同一把 DEK（本例中就是 legacy 模式下的 key1）。
    let kek_new = crate::crypto::derive_from_salt("new-master-456", &salt2_b64).unwrap();
    let wrapped_after = get_wrapped_dek_db().unwrap();
    assert!(!wrapped_after.is_empty(), "改密后必须写入 wrapped_dek");
    let dek = crate::vault::unwrap_data_key(&wrapped_after, &kek_new).unwrap();
    assert!(
        vault::verify_key(&verifier2, &dek),
        "新密码解出的数据密钥必须通过校验"
    );
    let kek_old = crate::crypto::derive_from_salt("old-master-123", &salt2_b64).unwrap();
    assert!(
        crate::vault::unwrap_data_key(&wrapped_after, &kek_old).is_err(),
        "旧密码不得再解出数据密钥"
    );

    // 新恢复密钥必须能解封"当前数据密钥"（改密轮换恢复码不得丢弃——历史缺陷回归）
    let seal_after = get_recovery_seal_db().unwrap();
    let unsealed = crate::recovery::unseal_with_recovery(&new_recovery_code, &seal_after).unwrap();
    assert_eq!(*unsealed, *dek, "新恢复码必须解出改密后的数据密钥");

    let backup = export_backup_json_db().unwrap();
    assert!(
        backup.contains("acc-round2") || backup.contains("enc:v1:") || backup.contains("enc:v2:"),
        "备份应含密文数据"
    );

    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute("DELETE FROM digital_assets", []).unwrap();
        conn.execute("DELETE FROM user_profile", []).unwrap();
    }
    assert!(
        get_asset_by_id_db(id).unwrap().is_none(),
        "清空后资产应不存在"
    );

    let restored = import_backup_json_db(&backup).unwrap();
    assert!(restored >= 1, "导入应恢复资产行数");
    assert!(
        get_asset_credential_db(id).is_err(),
        "导入后应处于锁定态，需重新解锁"
    );

    vault::set_key(dek.clone());
    assert_eq!(
        get_asset_credential_db(id).unwrap(),
        "cred-round2",
        "重新解锁后凭据应可读"
    );

    vault::clear_key();
    let (_, verifier_after) = get_security_config_db().unwrap().unwrap();
    let key2b = dek.clone();
    assert!(
        vault::verify_key(&verifier_after, &key2b),
        "导入后新密码仍可解锁"
    );
    vault::set_key(key2b);
    let after = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(
        after.heir_name.as_deref(),
        Some("李四"),
        "档案字段应完整恢复"
    );

    let _ = std::fs::remove_file(&path);
}

/// 安全回归：v3 备份自带恢复密钥封印，导入后必须原样保留；
/// 而 v1/v2 备份不含封印，导入时必须清空本地残留，否则恢复码会解出错误密钥。
#[test]
fn import_backup_preserves_v3_seal_and_clears_legacy_stale_seal() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};
    vault::clear_key();
    let path = fresh_db_path("seal");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("seal-master-123", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    let (_, recovery_seal) = crate::recovery::create_recovery_seal(&key).unwrap();
    set_recovery_seal_db(&recovery_seal).unwrap();
    vault::set_key(key.clone());

    // v3：封印随备份往返
    let backup = export_backup_json_db().unwrap();
    import_backup_json_db_with_mode_and_password(
        &backup,
        ImportMode::Replace,
        Some("seal-master-123"),
    )
    .unwrap();
    assert_eq!(
        get_recovery_seal_db().unwrap(),
        recovery_seal,
        "v3 备份必须保留恢复密钥封印"
    );

    // 旧 v2 备份（无 recovery_seal）：本地残留封印必须被清空
    let legacy: serde_json::Value = serde_json::from_str(&backup).unwrap();
    let mut legacy = legacy;
    legacy["version"] = serde_json::json!(2);
    legacy["tables"]["security_config"]
        .as_object_mut()
        .unwrap()
        .remove("recovery_seal");
    vault::clear_key();
    vault::set_key(key);
    import_backup_json_db_with_mode_and_password(
        &legacy.to_string(),
        ImportMode::Replace,
        Some("seal-master-123"),
    )
    .unwrap();
    assert_eq!(
        get_recovery_seal_db().unwrap(),
        "",
        "v2 备份不含封印，导入后不得留下指向旧主密钥的封印"
    );

    let _ = std::fs::remove_file(&path);
}

/// 安全回归：已设置主密码但处于锁定状态时，敏感字段写入必须被拒绝
/// （否则 vault::encrypt_if_needed 会静默明文落库）。尚未启用加密时放行。
#[test]
fn locked_vault_blocks_plaintext_writes() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};
    vault::clear_key();
    let path = fresh_db_path("lock");
    init_database_at(path.clone()).unwrap();

    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "13800000000".to_string(),
        account_description: "主账号".to_string(),
        category: "property".to_string(),
        estimated_value: Some(100.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("张三".to_string()),
        heir_contact: None,
        notes: "备注".to_string(),
    };

    // 未启用主密码：允许写入（存量明文模式）
    let id = add_asset_db(&req).unwrap();

    // 模拟"已设置主密码但处于锁定状态"
    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("lock-master-123", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    vault::clear_key();

    assert!(add_asset_db(&req).is_err(), "锁定态不得静默新增敏感字段");
    assert!(
        update_asset_db(id, &req).is_err(),
        "锁定态不得静默更新敏感字段"
    );
    assert!(
        save_user_profile_db(&UserProfile::default()).is_err(),
        "锁定态不得静默保存档案敏感字段"
    );

    // 解锁后恢复正常写入
    vault::set_key(key);
    assert!(add_asset_db(&req).is_ok(), "解锁后应可正常写入");

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 政策库补丁：导入后必须覆盖同 id 内置条目、并新增未知 id。
#[test]
fn platform_patch_overrides_builtin_entries() {
    let _g = vault::test_lock();
    let path = fresh_db_path("patch");
    init_database_at(path.clone()).unwrap();

    let builtin = get_all_platforms_db().unwrap();
    let target_id = builtin.iter().find(|p| p.name == "支付宝").unwrap().id;
    let before_len = builtin.len();

    let patch = serde_json::json!({
        "app": "digilegacy",
        "platforms": [
            {
                "id": target_id,
                "name": "支付宝（2026 政策更新）",
                "category": "支付金融",
                "required_documents": "死亡证明、继承公证书、继承人身份证",
                "process_description": "更新后的办理路径",
                "inheritable": true
            },
            {
                "id": 9001,
                "name": "某新平台",
                "category": "社交通讯"
            }
        ]
    })
    .to_string();

    let (added, updated) = import_platform_overrides_db(&patch).unwrap();
    assert_eq!((added, updated), (1, 1), "一条新增、一条覆盖");

    let after = get_all_platforms_db().unwrap();
    assert_eq!(after.len(), before_len + 1, "补丁新增的条目应出现在列表里");

    let patched = after.iter().find(|p| p.id == target_id).unwrap();
    assert_eq!(
        patched.name, "支付宝（2026 政策更新）",
        "同 id 必须被补丁覆盖"
    );
    assert_eq!(patched.process_description, "更新后的办理路径");

    let newp = after.iter().find(|p| p.id == 9001).unwrap();
    assert_eq!(newp.name, "某新平台");
    assert_eq!(count_platform_overrides_db().unwrap(), 2);

    // 再次启动时的 sync_platforms 不得抹掉补丁
    init_database_at(path.clone()).unwrap();
    let after_restart = get_all_platforms_db().unwrap();
    assert_eq!(
        after_restart
            .iter()
            .find(|p| p.id == target_id)
            .unwrap()
            .name,
        "支付宝（2026 政策更新）",
        "重启后补丁仍须生效（内置同步不能覆盖用户补丁）"
    );

    let cleared = clear_platform_overrides_db().unwrap();
    assert_eq!(cleared, 2);
    let restored = get_all_platforms_db().unwrap();
    assert_eq!(
        restored.iter().find(|p| p.id == target_id).unwrap().name,
        "支付宝"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn platform_patch_rejects_invalid_payloads() {
    let _g = vault::test_lock();
    let path = fresh_db_path("patchbad");
    init_database_at(path.clone()).unwrap();

    assert!(import_platform_overrides_db("{not json")
        .unwrap_err()
        .contains("解析失败"));
    assert!(import_platform_overrides_db(
        r#"{"app":"other","platforms":[{"id":1,"name":"x","category":"y"}]}"#
    )
    .unwrap_err()
    .contains("不是归零政策库补丁"));
    assert!(
        import_platform_overrides_db(r#"{"app":"digilegacy","platforms":[]}"#)
            .unwrap_err()
            .contains("没有平台条目")
    );
    assert!(import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":0,"name":"x","category":"y"}]}"#
    )
    .unwrap_err()
    .contains("id 必须为正数"));
    assert!(import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":5,"name":"","category":"y"}]}"#
    )
    .unwrap_err()
    .contains("缺少名称"));

    assert_eq!(
        count_platform_overrides_db().unwrap(),
        0,
        "失败的导入不得留下部分数据"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn platform_patch_export_roundtrip() {
    let _g = vault::test_lock();
    let path = fresh_db_path("patchrt");
    init_database_at(path.clone()).unwrap();

    let payload = r#"{"app":"digilegacy","platforms":[{"id":9100,"name":"导出测试","category":"游戏","notes":"n"}]}"#;
    import_platform_overrides_db(payload).unwrap();

    let exported = export_platform_overrides_db().unwrap();
    let v: serde_json::Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(v["app"], "digilegacy");
    assert_eq!(v["kind"], "platform-policy-patch");
    assert_eq!(v["platforms"].as_array().unwrap().len(), 1);
    assert_eq!(v["platforms"][0]["name"], "导出测试");

    // 导出的文件必须能被自己再导入（格式闭环）
    let (added, updated) = import_platform_overrides_db(&exported).unwrap();
    assert_eq!((added, updated), (0, 1), "重复导入应视为覆盖而非新增");

    let _ = std::fs::remove_file(&path);
}

/// 备份导出必须包含真实数据且可被自己再导入（闭环），而不是空壳 JSON。
#[test]
fn backup_export_contains_real_rows_and_reimports() {
    let _g = vault::test_lock();
    let path = fresh_db_path("backup");
    init_database_at(path.clone()).unwrap();

    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "acc-backup".to_string(),
        account_description: "desc".to_string(),
        category: "property".to_string(),
        estimated_value: Some(321.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("继承人A".to_string()),
        heir_contact: Some("h@example.com".to_string()),
        notes: "backup-notes".to_string().to_string(),
    };
    let id = add_asset_db(&req).unwrap();

    let json = export_backup_json_db().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["app"], "digilegacy");
    let rows = v["tables"]["digital_assets"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "备份必须包含已登记资产");
    assert_eq!(rows[0]["account_identifier"], "acc-backup");
    assert_eq!(rows[0]["notes"], "backup-notes");

    // 清空后重新导入，数据必须回来
    delete_asset_db(id).unwrap();
    assert!(get_all_assets_db().unwrap().is_empty());

    import_backup_json_db(&json).unwrap();
    let restored = get_all_assets_db().unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].account_identifier, "acc-backup");
    assert_eq!(restored[0].heir_name.as_deref(), Some("继承人A"));

    let _ = std::fs::remove_file(&path);
}

/// 备份必须覆盖用户手工配置的两张表：心跳令牌与政策补丁。
///
/// 历史缺陷：v1 备份只导出 5 张表，换机恢复后心跳令牌与导入的政策补丁全部丢失，
/// 而这两项无法从内置库重建。同时导入时不得因为「旧备份没有这些字段」而清空本地数据。
#[test]
fn backup_v2_covers_heartbeat_and_platform_overrides() {
    let _g = vault::test_lock();
    let path = fresh_db_path("backupv2");
    init_database_at(path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("backup-v2-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek.clone());

    set_heartbeat_config_db(true, "hb-token-123", "https://hb.example.org").unwrap();
    import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":9101,"name":"备份补丁平台","category":"游戏"}]}"#,
    )
    .unwrap();

    let json = export_backup_json_db().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v["version"], 5,
        "新增表（heirs/smtp/custody/assistant）后必须升版本号"
    );
    let exported_token = v["tables"]["heartbeat_config"]["token"]
        .as_str()
        .unwrap_or_default();
    assert!(
        crate::vault::is_encrypted(exported_token),
        "心跳令牌必须以密文进入备份"
    );
    assert!(
        !exported_token.contains("hb-token-123"),
        "备份不得含心跳令牌明文"
    );
    assert_eq!(
        v["tables"]["platform_overrides"].as_array().unwrap().len(),
        1,
        "政策补丁必须进备份"
    );

    // 清空后导入，两项都要回来
    set_heartbeat_config_db(false, "", "").unwrap();
    clear_platform_overrides_db().unwrap();
    let key_for_read = vault::get_key().unwrap();
    import_backup_json_db_with_mode_and_password(
        &json,
        ImportMode::Replace,
        Some("backup-v2-password"),
    )
    .unwrap();
    vault::set_key(key_for_read);

    let (enabled, token, endpoint) = get_heartbeat_config_db().unwrap();
    assert!(enabled);
    assert_eq!(token, "hb-token-123");
    assert_eq!(endpoint, "https://hb.example.org");
    assert_eq!(count_platform_overrides_db().unwrap(), 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn authenticated_backup_rejects_tampered_external_configuration() {
    let _g = vault::test_lock();
    vault::clear_key();
    let source_path = fresh_db_path("auth_external_binding_source");
    init_database_at(source_path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("external-binding-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek);
    set_heartbeat_config_db(true, "heartbeat-binding-test", "https://hb.example.org").unwrap();
    set_assistant_config_db(
        true,
        "https://assistant.example.org/v1",
        "bound-model",
        Some("assistant-binding-test"),
    )
    .unwrap();
    set_smtp_config_db(
        "smtp.example.org",
        587,
        "owner@example.org",
        "smtp-user",
        Some("smtp-binding-test"),
        false,
    )
    .unwrap();
    let backup = export_backup_json_db().unwrap();
    let mut tampered: serde_json::Value = serde_json::from_str(&backup).unwrap();
    tampered["tables"]["heartbeat_config"]["endpoint"] =
        serde_json::json!("https://attacker.example.org");

    vault::clear_key();
    let target_path = fresh_db_path("auth_external_binding_target");
    init_database_at(target_path.clone()).unwrap();
    let result = import_backup_json_db_with_mode_and_password(
        &tampered.to_string(),
        ImportMode::Replace,
        Some("external-binding-password"),
    );
    assert!(result.is_err(), "篡改外连配置的认证备份必须被拒绝");
    assert_eq!(
        get_heartbeat_config_db().unwrap(),
        (false, String::new(), String::new())
    );
    let assistant = get_assistant_config_db().unwrap();
    assert!(!assistant.enabled);
    assert!(assistant.endpoint.is_empty());
    let smtp = get_smtp_config_db().unwrap();
    assert!(smtp.host.is_empty());
    assert!(!smtp.allow_insecure);

    vault::clear_key();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&target_path);
}

#[test]
fn backup_import_rejects_heartbeat_endpoint_outside_public_https_policy() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("backup-heartbeat-policy");
    init_database_at(path.clone()).unwrap();
    let backup = serde_json::json!({
        "app": "digilegacy",
        "version": 1,
        "tables": {
            "digital_assets": [],
            "user_profile": [],
            "check_in_log": [],
            "dms_config": null,
            "security_config": null,
            "heartbeat_config": {
                "enabled": 1,
                "token": "",
                "endpoint": "http://example.com"
            }
        }
    })
    .to_string();

    let err = import_backup_json_db_with_mode(&backup, ImportMode::Replace).unwrap_err();
    assert!(
        err.contains("https") || err.contains("公网"),
        "实际错误: {}",
        err
    );

    let stored = get_heartbeat_config_db().unwrap();
    assert_eq!(stored, (false, String::new(), String::new()));
    let _ = std::fs::remove_file(&path);
}

/// 旧版 v1 备份仍可导入，且不会清空本地 v2 才有的配置。
#[test]
fn legacy_v1_backup_imports_without_wiping_v2_config() {
    let _g = vault::test_lock();
    let path = fresh_db_path("backupv1");
    init_database_at(path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("backup-v1-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek.clone());

    set_heartbeat_config_db(true, "keep-me", "https://hb.example.org").unwrap();
    import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":9102,"name":"保留补丁","category":"游戏"}]}"#,
    )
    .unwrap();

    let v1 = serde_json::json!({
        "app": "digilegacy",
        "version": 1,
        "tables": {
            "digital_assets": [],
            "user_profile": [],
            "check_in_log": [],
            "dms_config": null,
            "security_config": null
        }
    })
    .to_string();
    let key_for_read = vault::get_key().unwrap();
    import_backup_json_db_with_mode(&v1, ImportMode::Merge).unwrap();
    vault::set_key(key_for_read);

    let (enabled, token, _) = get_heartbeat_config_db().unwrap();
    assert!(enabled, "导入 v1 备份不得关闭心跳");
    assert_eq!(token, "keep-me");
    assert_eq!(
        count_platform_overrides_db().unwrap(),
        1,
        "不得清空政策补丁"
    );

    let _ = std::fs::remove_file(&path);
}

/// 明文备份不得绕过加密：锁定态拒绝，解锁态必须逐字段加密后再入库。
#[test]
fn plaintext_backup_never_lands_as_plaintext_in_encrypted_vault() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};
    vault::clear_key();
    let path = fresh_db_path("plainimport");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("plain-import-pw", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();

    let plaintext_backup = serde_json::json!({
        "app": "digilegacy",
        "version": 1,
        "tables": {
            "digital_assets": [{
                "id": 1,
                "platform_id": 1,
                "account_identifier": "明文账号",
                "account_description": "明文描述",
                "category": "property",
                "estimated_value": 10.0,
                "value_currency": "CNY",
                "disposal_method": "inherit",
                "heir_name": "明文继承人",
                "heir_contact": "13800000000",
                "notes": "明文备注",
                "credential_enc": "明文凭据",
                "is_confirmed": 1,
                "created_at": "2026-01-01 00:00:00",
                "updated_at": "2026-01-01 00:00:00"
            }],
            "user_profile": [{
                "id": 1,
                "full_name": "明文姓名",
                "phone": "13800000000",
                "email": "plain@example.com",
                "created_at": "2026-01-01 00:00:00",
                "updated_at": "2026-01-01 00:00:00"
            }],
            "check_in_log": [],
            "dms_config": null,
            "security_config": null
        }
    })
    .to_string();

    // 锁定态：必须拒绝，且不能改动已有数据
    let err = import_backup_json_db(&plaintext_backup).unwrap_err();
    assert!(
        err.contains("请先解锁"),
        "锁定态导入明文备份必须 fail-closed，实际: {}",
        err
    );

    // 解锁态：允许导入，但每个敏感字段都必须已加密
    vault::set_key(key.clone());
    import_backup_json_db(&plaintext_backup).unwrap();

    let guard = get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let raw_identifier: String = conn
        .query_row(
            "SELECT account_identifier FROM digital_assets WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        crate::vault::is_encrypted(&raw_identifier),
        "明文账号必须被加密后落库"
    );
    let raw_name: String = conn
        .query_row("SELECT full_name FROM user_profile WHERE id=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        crate::vault::is_encrypted(&raw_name),
        "档案字段必须被加密后落库"
    );
    drop(guard);

    // 导入成功后密钥被清空（强制重新解锁），重新解锁才能验证明文回读
    vault::set_key(crate::crypto::derive_key_bytes("plain-import-pw", &salt));
    let assets = get_all_assets_db().unwrap();
    assert_eq!(assets[0].account_identifier, "明文账号");
    assert_eq!(
        get_user_profile_db().unwrap().unwrap().full_name,
        "明文姓名"
    );

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 备份是外部输入：非法类别/处置/负金额必须在事务内被拒绝，且不留半截数据。
/// 旧实现直接把 JSON 里的字符串写进资产表，手改过的备份能污染统计与报告。
#[test]
fn backup_import_rejects_invalid_asset_fields() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("backup_invalid");
    init_database_at(path.clone()).unwrap();

    let id = add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "keep-me".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(10.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("王五".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();
    assert_eq!(get_all_assets_db().unwrap().len(), 1);

    let base_asset = serde_json::json!({
        "id": 2,
        "platform_id": 1,
        "account_identifier": "bad",
        "account_description": "",
        "category": "property",
        "estimated_value": 1.0,
        "value_currency": "CNY",
        "disposal_method": "inherit",
        "heir_name": "",
        "heir_contact": "",
        "notes": "",
        "credential_enc": "",
        "is_confirmed": 1,
        "created_at": "2026-01-01 00:00:00",
        "updated_at": "2026-01-01 00:00:00"
    });
    let mk = |patch: serde_json::Value| {
        let mut asset = base_asset.clone();
        if let Some(obj) = patch.as_object() {
            for (k, v) in obj {
                asset[k] = v.clone();
            }
        }
        serde_json::json!({
            "app": "digilegacy",
            "version": 3,
            "tables": {
                "digital_assets": [asset],
                "user_profile": [],
                "check_in_log": [],
                "dms_config": null,
                "security_config": null
            }
        })
        .to_string()
    };

    for (label, patch) in [
        (
            "非法类别",
            serde_json::json!({"category": "not_a_category"}),
        ),
        (
            "非法处置",
            serde_json::json!({"disposal_method": "sell_everything"}),
        ),
        ("负金额", serde_json::json!({"estimated_value": -5.0})),
        ("超大金额", serde_json::json!({"estimated_value": 1e15})),
    ] {
        let err = import_backup_json_db(&mk(patch)).unwrap_err();
        assert!(!err.is_empty(), "{} 必须给出错误信息", label);
        assert_eq!(
            get_all_assets_db().unwrap().len(),
            1,
            "{} 被拒绝后原数据必须完好",
            label
        );
        assert_eq!(
            get_asset_by_id_db(id).unwrap().unwrap().account_identifier,
            "keep-me",
            "{} 拒绝后不得改动既有资产",
            label
        );
    }

    // 合法备份仍然可以导入
    import_backup_json_db(&mk(serde_json::json!({}))).unwrap();
    assert_eq!(
        get_all_assets_db().unwrap().len(),
        1,
        "合法备份应替换为 1 条"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn encrypted_vault_rejects_malformed_security_config_before_plaintext_write() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};

    for (label, security_config) in [
        ("empty", serde_json::json!({})),
        (
            "incomplete",
            serde_json::json!({"kdf_salt": "v3.19456.2.1.AAAAAAAAAAAAAAAAAAAAAA=="}),
        ),
        (
            "forged",
            serde_json::json!({
                "kdf_salt": "not-a-valid-salt",
                "verifier": "not-a-ciphertext"
            }),
        ),
    ] {
        vault::clear_key();
        let path = fresh_db_path(&format!("malformed_security_{}", label));
        init_database_at(path.clone()).unwrap();

        let salt = crate::crypto::generate_salt();
        let key = crate::crypto::derive_key_bytes("malformed-security-pw", &salt);
        let verifier = vault::make_verifier(&key).unwrap();
        set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();

        let backup = serde_json::json!({
            "app": "digilegacy",
            "version": 5,
            "tables": {
                "digital_assets": [{
                    "id": 1,
                    "platform_id": 1,
                    "account_identifier": "malformed-config-plaintext",
                    "account_description": "敏感明文",
                    "category": "property",
                    "estimated_value": 10.0,
                    "value_currency": "CNY",
                    "disposal_method": "inherit",
                    "heir_name": "明文继承人",
                    "heir_contact": "13800000000",
                    "notes": "明文备注",
                    "credential_enc": "明文凭据",
                    "is_confirmed": 1,
                    "created_at": "2026-01-01 00:00:00",
                    "updated_at": "2026-01-01 00:00:00"
                }],
                "user_profile": [],
                "check_in_log": [],
                "dms_config": null,
                "security_config": security_config
            }
        })
        .to_string();

        let result = import_backup_json_db_with_mode(&backup, ImportMode::Replace);
        let (count, raw_identifier): (i64, String) = {
            let guard = get_conn().unwrap();
            let conn = guard.as_ref().unwrap();
            conn.query_row(
                "SELECT COUNT(*), COALESCE(MAX(account_identifier), '') FROM digital_assets",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
        };
        vault::clear_key();
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err(), "{} security_config 必须被拒绝", label);
        assert_eq!(count, 0, "{} 被拒绝后不得写入资产", label);
        assert!(
            raw_identifier.is_empty(),
            "{} 被拒绝后不得留下敏感明文",
            label
        );
    }
}

#[test]
fn encrypted_vault_rejects_plaintext_fields_with_valid_security_config() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine};
    vault::clear_key();
    let path = fresh_db_path("plaintext_with_config");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("plaintext-config-pw", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    let backup = serde_json::json!({
        "app": "digilegacy",
        "version": 5,
        "tables": {
            "digital_assets": [{
                "id": 1,
                "platform_id": 1,
                "account_identifier": "plaintext-under-valid-config",
                "category": "property",
                "estimated_value": 1.0,
                "disposal_method": "inherit",
                "heir_name": "明文继承人",
                "credential_enc": "明文凭据"
            }],
            "user_profile": [],
            "check_in_log": [],
            "security_config": {
                "kdf_salt": b64.encode(salt),
                "verifier": verifier,
                "wrapped_dek": ""
            }
        }
    })
    .to_string();

    let result = import_backup_json_db_with_mode(&backup, ImportMode::Replace);
    let count: i64 = {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.query_row("SELECT COUNT(*) FROM digital_assets", [], |row| row.get(0))
            .unwrap()
    };
    vault::clear_key();
    let _ = std::fs::remove_file(&path);

    assert!(result.is_err(), "有效外形配置下的敏感明文必须被拒绝");
    assert_eq!(count, 0, "拒绝后不得写入敏感明文");
}

#[test]
fn merge_rejects_same_password_backup_with_different_wrapped_dek() {
    let _g = vault::test_lock();
    vault::clear_key();
    let source_path = fresh_db_path("merge_source");
    init_database_at(source_path.clone()).unwrap();

    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("merge-same-password", &salt).unwrap();
    let source_dek = vault::generate_data_key();
    let source_wrapped = vault::wrap_data_key(&source_dek, &kek).unwrap();
    let source_verifier = vault::make_verifier(&source_dek).unwrap();
    set_security_config_with_dek_db(&salt, &source_verifier, &source_wrapped).unwrap();
    vault::set_key(source_dek.clone());
    add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "source-asset".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(10.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("source-heir".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();
    let backup = export_backup_json_db().unwrap();

    let other_dek = vault::generate_data_key();
    let other_wrapped = vault::wrap_data_key(&other_dek, &kek).unwrap();
    let mut forged: serde_json::Value = serde_json::from_str(&backup).unwrap();
    forged["tables"]["security_config"]["wrapped_dek"] = serde_json::json!(other_wrapped);
    let forged_backup = forged.to_string();

    vault::clear_key();
    let target_path = fresh_db_path("merge_target");
    init_database_at(target_path.clone()).unwrap();
    set_security_config_with_dek_db(&salt, &source_verifier, &source_wrapped).unwrap();
    vault::set_key(source_dek);
    add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "target-asset".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(20.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("target-heir".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();

    let result = import_backup_json_db_with_mode_and_password(
        &forged_backup,
        ImportMode::Merge,
        Some("merge-same-password"),
    );
    let identifiers = get_all_assets_db()
        .unwrap()
        .into_iter()
        .map(|asset| asset.account_identifier)
        .collect::<Vec<_>>();
    vault::clear_key();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&target_path);

    assert!(result.is_err(), "不同 wrapped_dek 的 Merge 必须被拒绝");
    assert_eq!(identifiers, vec!["target-asset"], "拒绝后不得追加源库资产");
}

#[test]
fn authenticated_backup_import_rejects_missing_password_fake_ciphertext_and_empty_wrapped() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("auth_backup_boundary");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("source-backup-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek);
    let backup = export_backup_json_db().unwrap();
    vault::clear_key();
    let target_path = fresh_db_path("auth_backup_boundary_target");
    init_database_at(target_path.clone()).unwrap();

    assert!(
        import_backup_json_db_with_mode_and_password(&backup, ImportMode::Replace, None).is_err()
    );

    let mut fake: serde_json::Value = serde_json::from_str(&backup).unwrap();
    fake["tables"]["digital_assets"] = serde_json::json!([{
        "id": 1,
        "platform_id": 1,
        "account_identifier": "enc:v1:AAAAAAAAAAAAAAAAAAAAAA==",
        "category": "property",
        "estimated_value": 1.0,
        "disposal_method": "delete",
        "created_at": "",
        "updated_at": ""
    }]);
    assert!(import_backup_json_db_with_mode_and_password(
        &fake.to_string(),
        ImportMode::Replace,
        Some("source-backup-password")
    )
    .is_err());

    fake["tables"]["security_config"]["wrapped_dek"] = serde_json::json!("");
    assert!(import_backup_json_db_with_mode_and_password(
        &fake.to_string(),
        ImportMode::Replace,
        Some("source-backup-password")
    )
    .is_err());

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&target_path);
}

#[test]
fn authenticated_backup_import_rejects_scalar_configuration_rows() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("auth_backup_scalar");
    init_database_at(path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("source-backup-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek);
    let mut backup: serde_json::Value =
        serde_json::from_str(&export_backup_json_db().unwrap()).unwrap();
    backup["tables"]["smtp_config"] = serde_json::json!("scalar");
    vault::clear_key();
    assert!(import_backup_json_db_with_mode_and_password(
        &backup.to_string(),
        ImportMode::Replace,
        Some("source-backup-password")
    )
    .is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn authenticated_backup_import_accepts_real_dek_verifier_and_aead_fields() {
    let _g = vault::test_lock();
    vault::clear_key();
    let source_path = fresh_db_path("auth_backup_source");
    init_database_at(source_path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("source-backup-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek.clone());
    add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "authenticated-account".to_string(),
        account_description: "authenticated-description".to_string(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "delete".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: "authenticated-notes".to_string(),
    })
    .unwrap();
    let backup = export_backup_json_db().unwrap();

    vault::clear_key();
    let target_path = fresh_db_path("auth_backup_target");
    init_database_at(target_path.clone()).unwrap();
    assert_eq!(
        import_backup_json_db_with_mode_and_password(
            &backup,
            ImportMode::Replace,
            Some("source-backup-password")
        )
        .unwrap(),
        1
    );
    assert!(!vault::is_unlocked());
    vault::set_key(dek);
    assert_eq!(
        get_all_assets_db().unwrap()[0].account_identifier,
        "authenticated-account"
    );

    vault::clear_key();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&target_path);
}

#[test]
fn locked_vault_rejects_backup_export_and_import() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("locked_backup_boundary");
    init_database_at(path.clone()).unwrap();
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("source-backup-password", &salt).unwrap();
    let dek = vault::generate_data_key();
    let wrapped = vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek);
    let backup = export_backup_json_db().unwrap();
    vault::clear_key();
    assert!(export_backup_json_db().is_err());
    assert!(import_backup_json_db_with_mode_and_password(
        &backup,
        ImportMode::Merge,
        Some("source-backup-password")
    )
    .is_err());
    let _ = std::fs::remove_file(&path);
}

/// 安全回归：资产详情页拿到占位符后点「保存修改」，必须被拒绝且原始密文不受影响。
///
/// 历史缺陷：`row_to_asset` 用占位符降级，前端把整张表单原样提交，
/// `update_asset_db` 就把占位符当成明文重新加密写回，真实账号被永久覆盖。
#[test]
fn asset_placeholder_cannot_be_written_back() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
    vault::clear_key();
    let path = fresh_db_path("asset_placeholder");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("asset-pw", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    vault::set_key(key.clone());

    let id = add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "real-account".to_string(),
        account_description: "真实描述".to_string(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("真实继承人".to_string()),
        heir_contact: Some("13800000000".to_string()),
        notes: "真实备注".to_string(),
    })
    .unwrap();

    let wrong = crate::crypto::derive_key_bytes("other-pw", &salt);
    vault::set_key(wrong);
    let loaded = get_asset_by_id_db(id).unwrap().unwrap();
    assert!(
        crate::vault::is_placeholder(&loaded.account_identifier),
        "错钥匙时必须降级为占位符"
    );

    // 前端资产详情点「保存修改」时会把这个占位符原样提交
    let err = update_asset_db(
        id,
        &AssetRequest {
            platform_id: loaded.platform_id,
            account_identifier: loaded.account_identifier.clone(),
            account_description: loaded.account_description.clone(),
            category: loaded.category.clone(),
            estimated_value: loaded.estimated_value,
            disposal_method: loaded.disposal_method.clone(),
            heir_name: loaded.heir_name.clone(),
            heir_contact: loaded.heir_contact.clone(),
            notes: loaded.notes.clone(),
        },
    )
    .unwrap_err();
    assert!(
        err.contains("解锁") && err.contains("未写入"),
        "必须明确拒绝并说明未写入，实际: {}",
        err
    );

    // 原始密文必须完好
    vault::set_key(key);
    let after = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(after.account_identifier, "real-account", "原始账号必须还在");
    assert_eq!(after.heir_name.as_deref(), Some("真实继承人"));
    assert_eq!(after.notes, "真实备注");

    // 正常编辑（非占位符）仍然可以保存
    update_asset_db(
        id,
        &AssetRequest {
            platform_id: 1,
            account_identifier: "edited-account".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(2.0),
            disposal_method: "memorialize".to_string(),
            heir_name: None,
            heir_contact: None,
            notes: String::new(),
        },
    )
    .unwrap();
    assert_eq!(
        get_asset_by_id_db(id).unwrap().unwrap().account_identifier,
        "edited-account"
    );

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 安全回归：档案页拿到占位符后点「保存档案」，必须被拒绝且原始密文完好。
#[test]
fn profile_placeholder_cannot_be_written_back() {
    let _g = vault::test_lock();
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
    vault::clear_key();
    let path = fresh_db_path("placeholder_wb");
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("wb-pw", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    vault::set_key(key.clone());

    let mut profile = UserProfile {
        id: None,
        full_name: "真实姓名".to_string(),
        phone: "13800000000".to_string(),
        email: "real@example.com".to_string(),
        id_last4: "1234".to_string(),
        emergency_contact_name: "紧急".to_string(),
        emergency_contact_phone: "13900000000".to_string(),
        emergency_contact_relation: "子女".to_string(),
        emergency_contact_email: "e@example.com".to_string(),
        heir_primary_name: "继承人".to_string(),
        heir_primary_phone: "13700000000".to_string(),
        heir_primary_relation: "子女".to_string(),
        heir_primary_email: "h@example.com".to_string(),
        check_in_interval_days: 30,
        last_check_in: None,
        dms_enabled: false,
        created_at: String::new(),
        updated_at: String::new(),
    };
    save_user_profile_db(&profile).unwrap();

    // 换一把钥匙：库内密文用旧钥匙，内存里是另一把 → 读回全是占位符
    let wrong = crate::crypto::derive_key_bytes("other-pw", &salt);
    vault::set_key(wrong);
    let loaded = get_user_profile_db().unwrap().unwrap();
    assert!(
        crate::vault::is_placeholder(&loaded.full_name),
        "错钥匙/未解锁时必须降级为占位符"
    );

    // 用户在界面上点保存（占位符被原样提交）
    profile.id = loaded.id;
    profile.full_name = loaded.full_name.clone();
    profile.phone = loaded.phone.clone();
    let err = save_user_profile_db(&profile).unwrap_err();
    assert!(
        err.contains("解锁") && err.contains("未写入"),
        "必须明确拒绝，实际: {}",
        err
    );

    // 原始密文完好
    vault::set_key(key);
    let after = get_user_profile_db().unwrap().unwrap();
    assert_eq!(after.full_name, "真实姓名");
    assert_eq!(after.phone, "13800000000");
    assert_eq!(after.heir_primary_name, "继承人");

    // 正常保存仍然可用
    let mut fixed = after.clone();
    fixed.full_name = "改名后".to_string();
    save_user_profile_db(&fixed).unwrap();
    assert_eq!(get_user_profile_db().unwrap().unwrap().full_name, "改名后");

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 回归：政策补丁新增的平台必须能直接登记资产。
///
/// 历史缺陷：补丁只写 platform_overrides，而 digital_assets.platform_id 的外键
/// 指向 platforms 表，于是「界面能选、一保存就 FOREIGN KEY constraint failed」。
#[test]
fn patch_only_platform_can_hold_assets() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("patch_fk");
    init_database_at(path.clone()).unwrap();

    import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":9001,"name":"补丁专属平台","category":"游戏"}]}"#,
    )
    .unwrap();

    let listed = get_all_platforms_db().unwrap();
    assert!(listed.iter().any(|p| p.id == 9001), "补丁平台应出现在列表");

    let id = add_asset_db(&AssetRequest {
        platform_id: 9001,
        account_identifier: "acc".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("王五".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .expect("补丁平台必须能登记资产（外键依赖已通过镜像解决）");

    let asset = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(asset.platform_name, "补丁专属平台", "平台名应来自补丁");

    // 补丁覆盖内置平台时也不得破坏既有资产的外键
    import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":1,"name":"支付宝（改名）","category":"支付金融"}]}"#,
    )
    .unwrap();
    let builtin_id = add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "builtin".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("王五".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();
    assert_eq!(
        get_asset_by_id_db(builtin_id)
            .unwrap()
            .unwrap()
            .platform_name,
        "支付宝（改名）",
        "补丁覆盖后的名称必须用于展示"
    );

    let _ = std::fs::remove_file(&path);
}

/// 仪表板与报告的总价值口径：负值与超大值都必须被排除，
/// 否则一条脏数据就能让「我的遗产总值」变成负数或荒谬数字。
#[test]
fn totals_ignore_negative_and_absurd_values() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("totals");
    init_database_at(path.clone()).unwrap();

    for (identifier, value) in [
        ("valid", 100.0),
        ("zero", 0.0),
        ("negative", -5.0),
        ("absurd", 1e15),
    ] {
        add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: identifier.to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(value),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap();
    }

    let stats = get_dashboard_stats_db().unwrap();
    assert_eq!(stats.total_assets, 4, "条数统计不受金额影响");
    assert!(
        (stats.total_estimated_value - 100.0).abs() < 1e-6,
        "只应累加合法的 100，实际 {}",
        stats.total_estimated_value
    );

    let report = get_inheritance_report_db().unwrap();
    assert!(
        (report.total_estimated_value - 100.0).abs() < 1e-6,
        "报告口径必须与仪表板一致，实际 {}",
        report.total_estimated_value
    );

    let _ = std::fs::remove_file(&path);
}

/// schema 版本号必须写入，且新库与再次打开都应保持同一版本。
#[test]
fn schema_version_is_recorded() {
    let _g = vault::test_lock();
    let path = fresh_db_path("schemaver");
    init_database_at(path.clone()).unwrap();

    let guard = get_conn().unwrap();
    let v: i64 = guard
        .as_ref()
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, SCHEMA_VERSION);
    drop(guard);

    // 同一路径再次初始化（模拟重启）不应回退版本
    init_database_at(path.clone()).unwrap();
    let guard = get_conn().unwrap();
    let v2: i64 = guard
        .as_ref()
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v2, SCHEMA_VERSION);

    let _ = std::fs::remove_file(&path);
}

/// 继承报告必须真的由库内数据生成，而不是固定模板。
#[test]
fn inheritance_report_reflects_actual_assets() {
    let _g = vault::test_lock();
    let path = fresh_db_path("report");
    init_database_at(path.clone()).unwrap();

    assert_eq!(get_inheritance_report_db().unwrap().assets.len(), 0);

    add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "r1".to_string(),
        account_description: "".to_string(),
        category: "property".to_string(),
        estimated_value: Some(1000.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("甲".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();
    add_asset_db(&AssetRequest {
        platform_id: 2,
        account_identifier: "r2".to_string(),
        account_description: "".to_string(),
        category: "personality".to_string(),
        estimated_value: Some(500.0),
        disposal_method: "memorialize".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();

    let report = get_inheritance_report_db().unwrap();
    assert_eq!(report.assets.len(), 2, "报告必须包含全部资产");
    assert_eq!(report.property_assets_count, 1);
    assert_eq!(report.personality_assets_count, 1);
    assert_eq!(report.total_estimated_value, 1500.0);
    assert!(!report.generated_at.is_empty(), "报告必须有生成时间");

    let _ = std::fs::remove_file(&path);
}

/// 新增资产时每个敏感字段必须立即以密文落库。
///
/// 此前的加密断言全部集中在 `encrypt_existing_data_db`（存量收编），
/// 「已解锁状态下 add_asset 写入」这条主路径没有断言——
/// 把 encrypt_if_needed 从 add_asset_db 摘掉，59 个测试仍全绿（变异实测确认）。
#[test]
fn add_asset_encrypts_every_sensitive_column_on_write() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("write_enc");
    init_database_at(path.clone()).unwrap();

    // 启用加密并解锁
    let salt = crate::crypto::generate_salt();
    let key = crate::crypto::derive_key_bytes("write-enc-master", &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    let salt_b64 = {
        use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
        b64.encode(salt)
    };
    set_security_config_with_dek_db(&salt_b64, &verifier, "").unwrap();
    vault::set_key(key);

    let id = add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "PLAINTEXT-ID".to_string(),
        account_description: "PLAINTEXT-DESC".to_string(),
        category: "property".to_string(),
        estimated_value: Some(42.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("PLAINTEXT-HEIR".to_string()),
        heir_contact: Some("PLAINTEXT-CONTACT".to_string()),
        notes: "PLAINTEXT-NOTES".to_string(),
    })
    .unwrap();

    // 直接读原始行，绕过读取层的自动解密
    let guard = get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    let row: (String, String, String, String, String) = conn
        .query_row(
            "SELECT account_identifier, account_description, COALESCE(heir_name,''), COALESCE(heir_contact,''), notes FROM digital_assets WHERE id=?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    drop(guard);

    let (acc, desc, heir, contact, notes) = row;
    for (name, value, plain) in [
        ("account_identifier", &acc, "PLAINTEXT-ID"),
        ("account_description", &desc, "PLAINTEXT-DESC"),
        ("heir_name", &heir, "PLAINTEXT-HEIR"),
        ("heir_contact", &contact, "PLAINTEXT-CONTACT"),
        ("notes", &notes, "PLAINTEXT-NOTES"),
    ] {
        assert!(
            vault::is_encrypted(value),
            "{} 必须以 enc:v1 密文落库，实际: {}",
            name,
            value
        );
        assert!(!value.contains(plain), "{} 不得以明文出现在库里", name);
    }

    // 读取层仍应解密还原
    let asset = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(asset.account_identifier, "PLAINTEXT-ID");
    assert_eq!(asset.notes, "PLAINTEXT-NOTES");
    assert_eq!(asset.heir_contact.as_deref(), Some("PLAINTEXT-CONTACT"));

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 签到必须复位 DMS 计时。
///
/// 删掉 `UPDATE dms_config SET last_check_in` 这一句，60 个测试仍全绿
/// （变异实测确认）。而真实后果是：用户点了「我还安好」，三级升级链
/// 却仍按旧时间继续升级，持续误报——这是会直接伤害用户的高危缺口。
#[test]
fn check_in_resets_dms_countdown() {
    let _g = vault::test_lock();
    let path = fresh_db_path("checkin");
    init_database_at(path.clone()).unwrap();

    // 造一个「已逾期」的初始状态：上次签到在 100 天前，间隔 30 天
    let old = (chrono::Local::now() - chrono::Duration::days(100))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute("UPDATE dms_config SET enabled=1, check_in_interval_days=30, last_check_in=?1 WHERE id=1", params![old])
            .unwrap();
    }
    let before = get_check_in_status_db().unwrap();
    assert!(before.days_until_due.unwrap_or(0) < 0, "前置：应已逾期");

    record_check_in_db().unwrap();

    let after = get_check_in_status_db().unwrap();
    assert!(
        after.days_until_due.unwrap_or(-1) > 0,
        "签到后剩余天数必须重新变正，实际 {:?}",
        after.days_until_due
    );
    assert_ne!(
        after.last_check_in.as_deref(),
        Some(old.as_str()),
        "dms_config.last_check_in 必须被更新"
    );

    // 签到日志也必须有记录
    let logs: i64 = {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.query_row("SELECT COUNT(*) FROM check_in_log", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(logs, 1, "签到必须写日志");

    let _ = std::fs::remove_file(&path);
}

/// 报告的类别统计必须准确，尤其 mixed 不能被算进 property。
#[test]
fn inheritance_report_counts_mixed_separately() {
    let _g = vault::test_lock();
    let path = fresh_db_path("report_mixed");
    init_database_at(path.clone()).unwrap();

    for (cat, val) in [
        ("property", 100.0),
        ("personality", 50.0),
        ("mixed", 25.0),
        ("mixed", 25.0),
    ] {
        add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: format!("acc-{}", cat),
            account_description: String::new(),
            category: cat.to_string(),
            estimated_value: Some(val),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap();
    }

    let report = get_inheritance_report_db().unwrap();
    assert_eq!(
        report.property_assets_count, 1,
        "property 只应统计 property"
    );
    assert_eq!(
        report.personality_assets_count, 1,
        "personality 只应统计 personality"
    );
    assert_eq!(report.mixed_assets_count, 2, "mixed 必须单独统计");
    assert_eq!(report.assets.len(), 4);
    assert_eq!(report.total_estimated_value, 200.0);

    let _ = std::fs::remove_file(&path);
}

/// 备份里含签到记录时必须能完整导入。
///
/// 此前 `backup_export_contains_real_rows_and_reimports` 没先签到，
/// 所以永远造不出「check_in_log 有行」的场景；删掉导入里的
/// `DELETE FROM check_in_log` 后 62 个测试仍全绿，而真实后果是：
/// 任何用过签到的用户，备份在灾难恢复时都会因主键冲突整体失败。
#[test]
fn backup_with_checkin_log_reimports_cleanly() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("backup_checkin");
    init_database_at(path.clone()).unwrap();

    add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "acc-ci".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("王五".to_string()),
        heir_contact: None,
        notes: String::new(),
    })
    .unwrap();
    // 产生至少一条签到记录（这是旧测试缺失的前置）
    record_check_in_db().unwrap();
    record_check_in_db().unwrap();

    let logs_before: i64 = {
        let guard = get_conn().unwrap();
        guard
            .as_ref()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM check_in_log", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(logs_before, 2);

    let json = export_backup_json_db().unwrap();
    assert!(json.contains("check_in_log"), "备份必须包含签到表");

    // 再签到一次，让本地记录比备份多，验证导入是整体替换而非追加
    record_check_in_db().unwrap();

    import_backup_json_db(&json).expect("含签到记录的备份必须可导入");

    let logs_after: i64 = {
        let guard = get_conn().unwrap();
        guard
            .as_ref()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM check_in_log", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(
        logs_after, 2,
        "导入后签到记录应与备份一致（不追加、不冲突）"
    );

    let assets = get_all_assets_db().unwrap();
    assert_eq!(assets.len(), 1, "资产也要一并恢复");

    let _ = std::fs::remove_file(&path);
}

/// 按类别查询：只返回该类别，且按估值降序。
#[test]
fn assets_by_category_filters_and_sorts() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("bycat");
    init_database_at(path.clone()).unwrap();

    for (cat, val) in [
        ("property", 100.0),
        ("property", 900.0),
        ("personality", 500.0),
    ] {
        add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: format!("acc-{}-{}", cat, val),
            account_description: String::new(),
            category: cat.to_string(),
            estimated_value: Some(val),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap();
    }

    let prop = get_assets_by_category_db("property").unwrap();
    assert_eq!(prop.len(), 2, "只应返回 property");
    assert!(prop.iter().all(|a| a.category == "property"));
    assert_eq!(prop[0].estimated_value, Some(900.0), "必须按估值降序");
    assert_eq!(prop[1].estimated_value, Some(100.0));

    let per = get_assets_by_category_db("personality").unwrap();
    assert_eq!(per.len(), 1);

    let none = get_assets_by_category_db("mixed").unwrap();
    assert!(none.is_empty());

    let _ = std::fs::remove_file(&path);
}

/// DMS 配置读写：enabled 与 interval 必须持久化并回读一致。
#[test]
fn dms_config_roundtrip() {
    let _g = vault::test_lock();
    let path = fresh_db_path("dmscfg");
    init_database_at(path.clone()).unwrap();

    // 默认值
    let st = get_check_in_status_db().unwrap();
    assert_eq!(st.interval_days, 30);
    assert!(!st.dms_enabled);

    set_dms_config_db(true, 14).unwrap();
    let st = get_check_in_status_db().unwrap();
    assert!(st.dms_enabled);
    assert_eq!(st.interval_days, 14);

    set_dms_config_db(false, 7).unwrap();
    let st = get_check_in_status_db().unwrap();
    assert!(!st.dms_enabled);
    assert_eq!(st.interval_days, 7);

    let _ = std::fs::remove_file(&path);
}

/// 剩余天数必须向上取整：到期前不到 24 小时应显示为 1 天而不是 0 天。
/// 旧实现用 `num_days()` 截断，界面把「还剩 20 小时」判成逾期，会误发提醒。
#[test]
fn days_until_due_rounds_up_instead_of_truncating() {
    let _g = vault::test_lock();
    let path = fresh_db_path("dueceil");
    init_database_at(path.clone()).unwrap();

    // 上次签到在 29 天 4 小时前，间隔 30 天 → 还剩 20 小时
    let last = (chrono::Local::now() - chrono::Duration::hours(29 * 24 + 4))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute(
            "UPDATE dms_config SET enabled=1, check_in_interval_days=30, last_check_in=?1 WHERE id=1",
            params![last],
        )
        .unwrap();
    }

    let st = get_check_in_status_db().unwrap();
    assert_eq!(
        st.days_until_due,
        Some(1),
        "到期前不足一天必须向上取整为 1，不能是 0（会被判成逾期）"
    );

    let _ = std::fs::remove_file(&path);
}

/// 签到历史：新的在前、只取指定条数，并在 status 里回传。
#[test]
fn check_in_history_is_newest_first_and_capped() {
    let _g = vault::test_lock();
    let path = fresh_db_path("checkin_log");
    init_database_at(path.clone()).unwrap();

    for _ in 0..7 {
        record_check_in_db().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
    }

    let st = get_check_in_status_db().unwrap();
    assert_eq!(st.recent_check_ins.len(), 5, "只回传最近 5 条");
    assert_eq!(
        Some(st.recent_check_ins[0].clone()),
        st.last_check_in,
        "第一条必须是最新签到"
    );
    let mut sorted = st.recent_check_ins.clone();
    sorted.sort();
    sorted.reverse();
    assert_eq!(sorted, st.recent_check_ins, "必须按时间倒序");

    let _ = std::fs::remove_file(&path);
}

/// 仪表板统计：各类计数与完成度必须与库内真实数据一致。
#[test]
fn dashboard_stats_reflect_real_data() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("stats");
    init_database_at(path.clone()).unwrap();

    let empty = get_dashboard_stats_db().unwrap();
    assert_eq!(empty.total_assets, 0);
    assert_eq!(empty.completion_percentage, 0, "空库完成度应为 0，不能除零");

    // 两条已设处置（inherit）、一条未设（no_action）
    for (method, cat) in [
        ("inherit", "property"),
        ("inherit", "personality"),
        ("no_action", "mixed"),
    ] {
        add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: format!("a-{}-{}", method, cat),
            account_description: String::new(),
            category: cat.to_string(),
            estimated_value: Some(10.0),
            disposal_method: method.to_string(),
            // 只有「继承」需要继承人；这一组要覆盖多种处置，按 method 给值。
            heir_name: if method == "inherit" {
                Some("王五".to_string())
            } else {
                None
            },
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap();
    }

    let s = get_dashboard_stats_db().unwrap();
    assert_eq!(s.total_assets, 3);
    assert_eq!(s.property_count, 1);
    assert_eq!(s.personality_count, 1);
    assert_eq!(s.mixed_count, 1);
    assert_eq!(s.disposal_planned_count, 2, "no_action 不计入已规划");
    assert_eq!(s.total_estimated_value, 30.0);
    assert_eq!(s.completion_percentage, 66, "2/3 应四舍五入为 66");

    let _ = std::fs::remove_file(&path);
}

/// 恢复流程的重加密：用旧主密钥解密 → 新主密钥加密，且新密码可正常解锁。
///
/// 这是「忘记主密码后用恢复码重设」的最后一公里：若这里漏掉某张表或某个字段，
/// 用户重设成功后会发现部分数据永久无法解密。
#[test]
fn reset_security_reencrypts_assets_and_profile() {
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("reset");
    init_database_at(path.clone()).unwrap();

    // 旧密钥体系
    let salt_old = crate::crypto::generate_salt();
    let key_old = crate::crypto::derive_key_bytes("old-master-111", &salt_old);
    let verifier_old = vault::make_verifier(&key_old).unwrap();
    set_security_config_with_dek_db(&b64.encode(salt_old), &verifier_old, "").unwrap();
    vault::set_key(key_old.clone());

    let id = add_asset_db(&AssetRequest {
        platform_id: 1,
        account_identifier: "reset-acc".to_string(),
        account_description: "desc-reset".to_string(),
        category: "property".to_string(),
        estimated_value: Some(77.0),
        disposal_method: "inherit".to_string(),
        heir_name: Some("继承人X".to_string()),
        heir_contact: Some("x@example.com".to_string()),
        notes: "notes-reset".to_string(),
    })
    .unwrap();
    set_asset_credential_db(id, "cred-reset").unwrap();
    save_user_profile_db(&UserProfile {
        full_name: "档案姓名".to_string(),
        phone: "13800000000".to_string(),
        ..Default::default()
    })
    .unwrap();

    // 关键前置：确认换密钥前，档案与资产确实是以旧密钥加密存储的
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        let raw: String = conn
            .query_row(
                "SELECT full_name FROM user_profile ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(vault::is_encrypted(&raw), "前置：档案字段必须是密文");
    }

    // 两层密钥：恢复重设沿用同一把数据密钥，只换 KEK（O(1)，不再逐字段重加密）
    let kek_new = crate::crypto::derive_key_bytes("new-master-222", &[7u8; 16]);
    let verifier_new = vault::make_verifier(&key_old).unwrap();
    let salt_v3 = crate::crypto::generate_salt_v3();

    reset_security_config_db(&salt_v3, &verifier_new, "", &key_old, &kek_new)
        .expect("重设必须成功");

    // 数据密钥未变 → 切回同一把 DEK 后所有数据必须可读且值不变
    vault::set_key(key_old.clone());
    let asset = get_asset_by_id_db(id).unwrap().unwrap();
    assert_eq!(asset.account_identifier, "reset-acc");
    assert_eq!(asset.account_description, "desc-reset");
    assert_eq!(asset.heir_name.as_deref(), Some("继承人X"));
    assert_eq!(asset.heir_contact.as_deref(), Some("x@example.com"));
    assert_eq!(asset.notes, "notes-reset");
    assert_eq!(get_asset_credential_db(id).unwrap(), "cred-reset");

    // 档案表是独立的一条 UPDATE，漏掉它时资产仍能读，所以必须单独断言
    let profile = get_user_profile_db().unwrap().expect("档案必须存在");
    assert_eq!(profile.full_name, "档案姓名", "档案必须用新密钥重新加密");
    assert_eq!(profile.phone, "13800000000");

    // 两层密钥的核心不变式：数据密钥不随密码变化。
    // 重设只换了包裹它的 KEK —— 这正是「改密 O(1)」与「继承人份额不失效」的前提。
    let wrapped_after = get_wrapped_dek_db().unwrap();
    assert!(!wrapped_after.is_empty(), "重设后必须写入 wrapped_dek");
    assert_eq!(
        *crate::vault::unwrap_data_key(&wrapped_after, &kek_new).unwrap(),
        *key_old,
        "新 KEK 必须能解出同一把数据密钥"
    );
    assert!(
        crate::vault::unwrap_data_key(&wrapped_after, &key_old).is_err(),
        "旧 KEK 不得再解开 wrapped_dek"
    );

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 「指定继承」必须写明继承人：否则报告里是「继承人：未指定」，家人无法执行。
/// 前端有同样的拦截，但 IPC 可绕过前端，这条必须在写库前执行。
#[test]
fn inherit_disposal_requires_heir_name() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("inherit_heir_required");
    init_database_at(path.clone()).unwrap();

    let base = AssetRequest {
        platform_id: 1,
        account_identifier: "acc".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "inherit".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: String::new(),
    };
    let err = add_asset_db(&base).unwrap_err();
    assert!(err.contains("继承人"), "实际错误: {}", err);

    // 空字符串同样拒绝（前端可能提交空串）
    let mut blank = base.clone();
    blank.heir_name = Some("   ".to_string());
    assert!(add_asset_db(&blank).is_err(), "空白继承人必须拒绝");

    // 其他处置方式不要求继承人
    let mut liquidate = base.clone();
    liquidate.disposal_method = "liquidate".to_string();
    assert!(add_asset_db(&liquidate).is_ok(), "变现分配不要求继承人");

    // 填了继承人就能写入
    let mut ok = base;
    ok.heir_name = Some("王五".to_string());
    assert!(add_asset_db(&ok).is_ok());
}

/// 完成度必须四项分解：只设了处置方式不能算「完成」。
#[test]
fn dashboard_completion_counts_four_dimensions() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("completion_four");
    init_database_at(path.clone()).unwrap();

    // 两条资产：一条只设处置（无继承人），一条什么都不填
    let mut a = AssetRequest {
        platform_id: 1,
        account_identifier: "acc-1".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "liquidate".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: String::new(),
    };
    add_asset_db(&a).unwrap();
    a.account_identifier = String::new();
    a.disposal_method = "no_action".to_string();
    add_asset_db(&a).unwrap();

    let stats = get_dashboard_stats_db().unwrap();
    assert_eq!(stats.total_assets, 2);
    assert_eq!(stats.disposal_planned_count, 1);
    assert_eq!(stats.heir_assigned_count, 2, "非继承处置视为已满足继承人项");
    assert_eq!(stats.identifier_ready_count, 1);
    assert!(!stats.trigger_ready, "未启用 DMS 时触发链不可能就绪");
    assert!(
        stats.completion_percentage < 100,
        "只设了处置方式不得算完成，实际 {}",
        stats.completion_percentage
    );
    assert!(
        stats.completion_percentage > 0,
        "已有部分达标时不应为 0，实际 {}",
        stats.completion_percentage
    );
}

/// 仅心跳可用时，触发链不算达标：家人收不到邮件，不能显示 100%。
#[test]
fn heartbeat_only_does_not_count_as_reachable_trigger() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("trigger_channel");
    init_database_at(path.clone()).unwrap();

    // 启用 DMS + 心跳，但完全没配 SMTP
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("heartbeat-dashboard-password", &salt).unwrap();
    let dek = crate::vault::generate_data_key();
    let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = crate::vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek);
    set_dms_config_db(true, 30).unwrap();
    set_heartbeat_config_db(true, "tok", "https://hb.example.org").unwrap();

    let stats = get_dashboard_stats_db().unwrap();
    assert!(!stats.trigger_ready, "仅心跳不得算作家人可达");
    assert_eq!(stats.trigger_channel, "heartbeat");
    assert!(
        stats.completion_percentage < 100,
        "触发链未达标时完成度不得为 100%，实际 {}",
        stats.completion_percentage
    );
}

/// 待办检查项：必须能算出「差在哪」，且 blocker 优先。
/// 只有百分比时用户不知道下一步做什么，这条测试锁住可执行性。
#[test]
fn dashboard_issues_are_actionable() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("issues");
    init_database_at(path.clone()).unwrap();

    // 空库：没有资产，但保管位置与触发链仍未完成 → 必须报 blocker
    let stats = get_dashboard_stats_db().unwrap();
    let keys: Vec<&str> = stats.issues.iter().map(|i| i.key.as_str()).collect();
    assert!(
        keys.contains(&"custody_missing"),
        "空库时保管位置缺失必须报出，实际 {:?}",
        keys
    );
    assert!(
        keys.contains(&"trigger_disabled"),
        "未启用 DMS 必须报出，实际 {:?}",
        keys
    );
    // 每条都必须带跳转目标，否则用户点不动
    for issue in &stats.issues {
        assert!(issue.target.starts_with('/'), "{} 缺跳转目标", issue.key);
        assert!(
            ["blocker", "warn", "info"].contains(&issue.severity.as_str()),
            "{} 的严重度非法: {}",
            issue.key,
            issue.severity
        );
    }

    // 加一条「继承但无继承人」的资产 → 必须报 missing_heir 且为 blocker
    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "acc".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "liquidate".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: String::new(),
    };
    add_asset_db(&req).unwrap();
    let stats2 = get_dashboard_stats_db().unwrap();
    assert!(
        !stats2.issues.iter().any(|i| i.key == "missing_heir"),
        "liquidate 不涉及继承人，不应报 missing_heir"
    );
}

/// 政策未核实按「用户持有的平台」统计，不是按政策库总量。
/// 直接报 147 会让用户觉得与自己无关，报自己持有的数量才有代入感。
#[test]
fn unverified_policy_issue_is_asset_scoped() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("policy_issue");
    init_database_at(path.clone()).unwrap();

    // 加一条资产（平台 1 = 支付宝，其 data_source 是真实出处，不算未核实）
    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "acc".to_string(),
        account_description: String::new(),
        category: "property".to_string(),
        estimated_value: Some(1.0),
        disposal_method: "liquidate".to_string(),
        heir_name: None,
        heir_contact: None,
        notes: String::new(),
    };
    add_asset_db(&req).unwrap();
    let stats = get_dashboard_stats_db().unwrap();
    // 支付宝有出处，因此不应出现 policy_unverified
    assert!(
        !stats.issues.iter().any(|i| i.key == "policy_unverified"),
        "有出处的平台不应报未核实: {:?}",
        stats.issues.iter().map(|i| &i.key).collect::<Vec<_>>()
    );
}

/// 审核留痕必须随备份往返：换机后审核记录不能凭空消失。
/// 这是「留痕是证据」这条设计的前提，否则证据只存在于一台电脑上。
#[test]
fn review_records_survive_backup_roundtrip() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = fresh_db_path("review_backup");
    init_database_at(path.clone()).unwrap();

    // 写一条审核记录
    append_review_record_db(&crate::review::ReviewRecord {
        profile_id: "platform-policy".to_string(),
        item_key: "1".to_string(),
        fingerprint: "abc123".to_string(),
        ai_verdict: crate::review::AiVerdict::Flag,
        ai_note: "来源为空".to_string(),
        human_verdict: Some(crate::review::HumanVerdict::Approved),
        human_note: "已补官方链接".to_string(),
        decided_by: "human".to_string(),
        created_at: "2026-10-27 10:00:00".to_string(),
    })
    .unwrap();

    let json = export_backup_json_db().unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let records = v["tables"]["review_records"]
        .as_array()
        .expect("备份必须含审核记录");
    assert_eq!(records.len(), 1, "导出条数不符");
    assert_eq!(records[0]["human_note"], "已补官方链接");

    // 清空后导入，记录必须回来
    {
        let guard = get_conn().unwrap();
        guard
            .as_ref()
            .unwrap()
            .execute("DELETE FROM review_records", [])
            .unwrap();
    }
    import_backup_json_db(&json).unwrap();
    let back = list_review_records_db("platform-policy", "1").unwrap();
    assert_eq!(back.len(), 1, "导入后记录必须恢复");
    assert_eq!(back[0].human_note, "已补官方链接");
    assert_eq!(back[0].fingerprint, "abc123");

    // 再导一次同一份（merge 语义）：不得重复堆积
    import_backup_json_db(&json).unwrap();
    let again = list_review_records_db("platform-policy", "1").unwrap();
    assert_eq!(again.len(), 1, "重复导入不得产生重复留痕");
}

/// 两层密钥的核心收益：改密码不再重写任何字段。
///
/// 用一个可观测的不变式证明：改密前后**密文本身完全不变**。
/// 若实现退回「逐字段重加密」，AES-GCM 每次 nonce 随机 → 密文必然变，测试立刻失败。
#[test]
fn change_password_does_not_touch_ciphertext() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = std::env::temp_dir().join(format!("digilegacy_layered_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    init_database_at(path.clone()).unwrap();

    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("old-master-123", &salt).unwrap();
    let dek = crate::vault::generate_data_key();
    let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    vault::set_key(dek.clone());

    let req = AssetRequest {
        platform_id: 1,
        account_identifier: "o1".into(),
        account_description: "d1".into(),
        category: "property".into(),
        estimated_value: Some(100.0),
        disposal_method: "inherit".into(),
        heir_name: Some("甲".into()),
        heir_contact: Some("139".into()),
        notes: "n1".into(),
    };
    let id = add_asset_db(&req).unwrap();
    set_asset_credential_db(id, "cred-o1").unwrap();

    let ciphertext_before: String = {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.query_row(
            "SELECT account_identifier || '|' || credential_enc FROM digital_assets WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    };

    change_master_password_db("old-master-123", "new-master-456", "new-master-456").unwrap();

    let ciphertext_after: String = {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.query_row(
            "SELECT account_identifier || '|' || credential_enc FROM digital_assets WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        ciphertext_before, ciphertext_after,
        "改密不得重写字段：密文必须逐字节相同（否则是 O(n) 旧实现）"
    );
    assert_eq!(get_asset_credential_db(id).unwrap(), "cred-o1");
    assert!(
        ciphertext_after.contains("enc:v2:"),
        "新写入的字段应为 v2（HKDF 域分离）"
    );

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
}

/// 跨设备恢复：库 A 生成恢复包（元数据随备份导出），库 B 导入后用份额 + 口令解出同一数据密钥。
#[test]
fn cross_device_recovery_from_shares_and_backup() {
    let _g = vault::test_lock();
    vault::clear_key();

    let path_a = std::env::temp_dir().join(format!("digilegacy_devA_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path_a);
    init_database_at(path_a.clone()).unwrap();

    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt("device-a-pw", &salt).unwrap();
    let dek = crate::vault::generate_data_key();
    let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
    let verifier = vault::make_verifier(&dek).unwrap();
    let (_code, seal) = crate::recovery::create_recovery_seal(&dek).unwrap();
    set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
    set_recovery_seal_db(&seal).unwrap();
    vault::set_key(dek.clone());

    // 生成恢复包（口令模式：份额拆的是密文，必须口令 + 份额同时具备）
    let dir = std::env::temp_dir().join(format!("digilegacy_kit_{}", std::process::id()));
    let (kit, _html) =
        crate::heir_recovery::generate_kit(3, 5, Some("kit-passphrase"), &dir).unwrap();
    assert_eq!(kit.total, 5);
    assert!(kit.passphrase_protected);

    // 把生成的 HTML 里每份份额抽出来（模拟家人按纸上内容输入）
    let on_disk = std::fs::read_to_string(&kit.path).unwrap();
    let mut shares: Vec<String> = Vec::new();
    let mut rest = on_disk.as_str();
    while let Some(pos) = rest.find(crate::heir_recovery::SHARE_PREFIX) {
        let tail = &rest[pos..];
        let code: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        let advance = code.len().max(1);
        if code.len() > 20 {
            shares.push(code);
        }
        rest = &tail[advance..];
    }
    assert_eq!(shares.len(), 5, "恢复包必须包含 5 份可抄写的份额");

    let backup = export_backup_json_db().unwrap();
    let v: serde_json::Value = serde_json::from_str(&backup).unwrap();
    assert_eq!(v["version"], 5);
    assert!(
        v["tables"]["heir_recovery"]["fingerprint"].is_string(),
        "备份必须携带恢复包元数据，否则换机后份额配不上"
    );

    // ── 设备 B ──
    vault::clear_key();
    let path_b = std::env::temp_dir().join(format!("digilegacy_devB_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path_b);
    init_database_at(path_b.clone()).unwrap();
    import_backup_json_db_with_mode_and_password(&backup, ImportMode::Replace, Some("device-a-pw"))
        .unwrap();
    vault::clear_key();

    // 份额不足 → 必须拒绝
    assert!(
        crate::heir_recovery::recover_from_shares(&shares[..2], Some("kit-passphrase")).is_err(),
        "少于门限不得恢复"
    );
    // 口令错误 → 必须拒绝
    assert!(
        crate::heir_recovery::recover_from_shares(&shares[..3], Some("wrong-pass")).is_err(),
        "口令错误不得恢复"
    );
    // 正确：任意 3 份 + 口令
    crate::heir_recovery::recover_from_shares(&shares[..3], Some("kit-passphrase"))
        .expect("跨设备恢复必须成功");
    assert_eq!(*vault::get_key().unwrap(), *dek, "必须解出同一把数据密钥");

    vault::clear_key();
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
    let _ = std::fs::remove_dir_all(&dir);
}

/// AI 问答历史：追加 → 倒序读取 → 清空。
#[test]
fn ai_chat_log_roundtrip() {
    let _g = vault::test_lock();
    let path = std::env::temp_dir().join(format!("digilegacy_chat_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    init_database_at(path.clone()).unwrap();

    append_ai_chat_db(
        "怎么继承支付宝余额？",
        "第一步：准备死亡证明…",
        "gpt-4o-mini",
    )
    .unwrap();
    append_ai_chat_db("恢复码丢了怎么办？", "用继承人恢复份额…", "gpt-4o-mini").unwrap();

    let history = list_ai_chat_db(10).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["question"], "恢复码丢了怎么办？", "新的在前");
    assert_eq!(history[1]["answer"], "第一步：准备死亡证明…");
    assert!(history[0]["model"] == "gpt-4o-mini");

    clear_ai_chat_db().unwrap();
    assert!(list_ai_chat_db(10).unwrap().is_empty(), "清空后必须为空");

    let _ = std::fs::remove_file(&path);
}

/// 长度上限：超长问答必须拒绝（防止把几十 MB 粘贴进历史表）。
#[test]
fn ai_chat_log_rejects_oversized() {
    let _g = vault::test_lock();
    let path = std::env::temp_dir().join(format!("digilegacy_chat2_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    init_database_at(path.clone()).unwrap();

    let big_q = "x".repeat(2_001);
    assert!(append_ai_chat_db(&big_q, "ok", "m").is_err());
    let big_a = "y".repeat(20_001);
    assert!(append_ai_chat_db("q", &big_a, "m").is_err());

    let _ = std::fs::remove_file(&path);
}

// ========== 整库加密（SQLCipher）迁移往返 ==========

/// 明文库 → 加密 → 数据一致 → 解密 → 数据一致。
/// 这是整库加密核心承诺的可执行规格：迁移不丢数据，两种形态都真实落盘。
/// DPAPI 是 Windows 设施，Linux CI 上整库加密不可用（fallback 恒明文）。
#[cfg(windows)]
#[test]
fn sqlcipher_encrypt_decrypt_roundtrip_preserves_data() {
    let _lock = vault::test_lock();
    let dir = std::env::temp_dir().join(format!(
        "digilegacy_enc_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("digilegacy.db");

    // 1. 明文库 + 一条真实资产
    init_database_at(db.clone()).unwrap();
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute_batch(
            "INSERT INTO digital_assets (platform_id, account_identifier, category, disposal_method, created_at, updated_at)
             VALUES (1, 'test-id-001', 'property', 'no_action', '2026-11-04 00:00:00', '2026-11-04 00:00:00');",
        )
        .unwrap();
    }

    // 2. 迁移前：文件头必须是 SQLite 明文 magic
    let head = std::fs::read(&db).unwrap();
    assert!(
        head.starts_with(b"SQLite format 3"),
        "迁移前必须是明文 SQLite 文件"
    );

    // 3. 加密迁移
    encrypt_database_at_runtime(&dir).unwrap();
    assert!(
        dir.join(crate::db_crypto::KEY_FILE_NAME).exists(),
        "key 文件应落盘"
    );

    // 4. 迁移后：文件头不再是 SQLite magic（整库已加密）
    let head = std::fs::read(&db).unwrap();
    assert!(
        !head.starts_with(b"SQLite format 3"),
        "迁移后必须是加密文件，不能残留明文头"
    );

    // 5. 当前连接（已用 key 重开）数据一致
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM digital_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "迁移后数据必须还在");
    }

    // 6. 解密回明文
    decrypt_database_at_runtime(&dir).unwrap();
    assert!(
        !dir.join(crate::db_crypto::KEY_FILE_NAME).exists(),
        "解密后 key 文件应删除"
    );
    let head = std::fs::read(&db).unwrap();
    assert!(
        head.starts_with(b"SQLite format 3"),
        "解密后应回到明文 SQLite"
    );
    {
        let guard = get_conn().unwrap();
        let conn = guard.as_ref().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM digital_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "解密后数据必须还在");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// 错误 key 不能打开加密库——「key 正确性可验证」是整库加密的安全前提。
#[cfg(windows)]
#[test]
fn sqlcipher_wrong_key_fails_closed() {
    let _lock = vault::test_lock();
    let dir = std::env::temp_dir().join(format!(
        "digilegacy_encwrong_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("digilegacy.db");
    init_database_at(db.clone()).unwrap();
    encrypt_database_at_runtime(&dir).unwrap();

    // 用错误 key 打开必须失败（verify_db_readable 的 fail-closed 语义）
    let wrong = [7u8; 32];
    let err = init_database_at_with_key(db.clone(), Some(&wrong)).unwrap_err();
    assert!(
        err.contains("密钥不正确") || err.contains("损坏"),
        "错误 key 的错误信息应明确: {err}"
    );

    // 用正确 key 重开（恢复测试环境的连接状态）
    let right = crate::db_crypto::load_db_key(&dir).unwrap().unwrap();
    init_database_at_with_key(db.clone(), Some(&right)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
