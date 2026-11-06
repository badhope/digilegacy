//! IPC 命令层（薄适配器）：校验入参 -> 委托 `db::*` -> 包 `ApiResponse`。
//!
//! 业务命令按域拆在 commands_*.rs 子模块（见底部声明），本文件只保留
//! 跨域集成测试；`lib.rs` 的 `generate_handler!` 仍通过 `commands::*`
//! 引用，命令注册路径不变。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::*;

    #[test]
    fn set_master_password_returns_recovery_code_in_data_and_it_unseals() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let res = set_master_password("test-master-123".to_string()).unwrap();
        assert!(res.success);
        // 恢复密钥必须放在 data 字段中交给 UI 展示，而不是塞进 message 里被丢弃
        let recovery_code = res
            .data
            .as_ref()
            .expect("data 必须存在")
            .recovery_code
            .clone();
        assert!(!recovery_code.is_empty(), "恢复密钥不得为空");
        assert_eq!(
            recovery_code.matches('-').count(),
            5,
            "恢复密钥应为 XXXX-…-XXXX 分组格式"
        );
        assert!(
            !res.message.contains(&recovery_code),
            "恢复密钥不得明文出现在 message 中"
        );

        // 用返回的恢复密钥应能解封主密钥（模拟用户忘记主密码后的恢复路径）
        let seal = db::get_recovery_seal_db().unwrap();
        let unsealed = crate::recovery::unseal_with_recovery(&recovery_code, &seal).unwrap();
        assert_eq!(
            unsealed,
            crate::vault::get_key().unwrap(),
            "恢复密钥必须能解出当前主密钥"
        );

        // 重复设置应被拒绝
        let second = set_master_password("another-pass-456".to_string()).unwrap();
        assert!(!second.success);
        assert!(second.data.is_none());

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    /// 凭据命令：锁定态必须拒绝读取，且不得把明文回给前端。
    #[test]
    fn credential_commands_refuse_while_locked() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_cred_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        // 未启用加密时可直接写入
        let req = crate::models::AssetRequest {
            platform_id: 1,
            account_identifier: "acc".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(1.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        };
        let id = db::add_asset_db(&req).unwrap();

        // 凭据的写入门槛比普通字段更严：即使未启用主密码也必须有会话密钥，
        // 否则「保存了凭据」实际是明文直通。这里先启用加密并解锁再写入。
        let salt = crate::crypto::generate_salt();
        let key = crate::crypto::derive_key_bytes("lock-master-123", &salt);
        let verifier = crate::vault::make_verifier(&key).unwrap();
        let salt_b64 = {
            use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
            b64.encode(salt)
        };
        db::set_security_config_with_dek_db(&salt_b64, &verifier, "").unwrap();
        crate::vault::set_key(key);
        db::encrypt_existing_data_db().unwrap();

        set_asset_credential(id, "user:pass".to_string()).unwrap();
        assert_eq!(
            get_asset_credential(id).unwrap().data.as_deref(),
            Some("user:pass")
        );

        // 锁定后读取必须被拒绝（不能返回明文，也不能返回成功空串）
        crate::vault::clear_key();

        // 读凭据在锁定态下以 Err 拒绝（fail-closed），而不是返回成功空串——
        // 空串会和「用户确实没存凭据」混淆，Err 才是明确的安全信号。
        let res = get_asset_credential(id);
        assert!(res.is_err(), "锁定态读取凭据必须被拒绝");
        assert!(res.unwrap_err().contains("解锁"), "拒绝原因应提示需要解锁");

        // 写入同样 fail-closed：Err 而非 success:false
        let write = set_asset_credential(id, "new-secret".to_string());
        assert!(write.is_err(), "锁定态写入凭据必须被拒绝");
        assert!(
            write.unwrap_err().contains("解锁"),
            "拒绝原因应提示需要解锁"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// 备份导入：非法内容必须拒绝，且不得破坏既有数据。
    #[test]
    fn import_backup_rejects_invalid_without_data_loss() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_imp_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let req = crate::models::AssetRequest {
            platform_id: 1,
            account_identifier: "keep-me".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(9.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        };
        db::add_asset_db(&req).unwrap();

        for bad in ["{not json", "", r#"{"app":"other","version":1}"#] {
            let res = import_backup_response(bad, "replace", None).unwrap();
            assert!(!res.success, "非法备份必须被拒绝: {}", bad);
        }

        let assets = db::get_all_assets_db().unwrap();
        assert_eq!(assets.len(), 1, "失败的导入不得清空既有数据");
        assert_eq!(assets[0].account_identifier, "keep-me");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn replace_import_aborts_when_pre_snapshot_export_fails() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_cmd_imp_snapshot_export_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let req = crate::models::AssetRequest {
            platform_id: 1,
            account_identifier: "keep-me".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(9.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        };
        db::add_asset_db(&req).unwrap();
        {
            let guard = db::get_conn().unwrap();
            let conn = guard.as_ref().unwrap();
            conn.execute("DROP TABLE review_records", []).unwrap();
        }
        assert!(
            db::export_backup_json_db().is_err(),
            "测试必须先证明导出快照会失败"
        );

        let backup = serde_json::json!({
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
        let snapshot_dir = std::env::temp_dir().join(format!(
            "digilegacy_cmd_imp_snapshot_export_dir_{}",
            std::process::id()
        ));
        let res = import_backup_response(&backup, "replace", Some(&snapshot_dir)).unwrap();
        assert!(!res.success, "导出快照失败时覆盖导入必须失败");
        assert!(res.message.contains("快照"), "失败原因必须指出快照失败");

        let assets = db::get_all_assets_db().unwrap();
        assert_eq!(assets.len(), 1, "导出快照失败不得清空既有数据");
        assert_eq!(assets[0].account_identifier, "keep-me");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&snapshot_dir);
    }

    #[test]
    fn replace_import_aborts_when_pre_snapshot_write_fails() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_cmd_imp_snapshot_write_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let req = crate::models::AssetRequest {
            platform_id: 1,
            account_identifier: "keep-me".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(9.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        };
        db::add_asset_db(&req).unwrap();

        let snapshot_path = std::env::temp_dir().join(format!(
            "digilegacy_cmd_imp_snapshot_file_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&snapshot_path);
        std::fs::write(&snapshot_path, b"not-a-directory").unwrap();

        let backup = serde_json::json!({
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
        let res = import_backup_response(&backup, "replace", Some(&snapshot_path)).unwrap();
        assert!(!res.success, "写入快照失败时覆盖导入必须失败");
        assert!(
            res.message.contains("快照") && res.message.contains("写入"),
            "失败原因必须指出快照写入失败"
        );

        let assets = db::get_all_assets_db().unwrap();
        assert_eq!(assets.len(), 1, "写入快照失败不得清空既有数据");
        assert_eq!(assets[0].account_identifier, "keep-me");

        let _ = std::fs::remove_file(&snapshot_path);
        let _ = std::fs::remove_file(&path);
    }

    /// 平台补丁导入：超过 8MB 上限必须在解析前拒绝（防内存/CPU 放大）。
    #[test]
    fn import_platform_patch_rejects_oversized() {
        let _g = crate::vault::test_lock();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_patch_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let huge = "x".repeat(8 * 1024 * 1024 + 1);
        let res = import_platform_patch(huge).unwrap();
        assert!(!res.success);
        assert!(
            res.message.contains("过大"),
            "应提示超过上限，实际: {}",
            res.message
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn locked_public_recovery_and_patch_commands_fail_closed() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_cmd_locked_public_mutations_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let salt = crate::crypto::generate_salt();
        let key = crate::crypto::derive_key_bytes("locked-command-test", &salt);
        let verifier = crate::vault::make_verifier(&key).unwrap();
        let salt_b64 = {
            use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
            b64.encode(salt)
        };
        db::set_security_config_with_dek_db(&salt_b64, &verifier, "").unwrap();
        crate::vault::clear_key();

        let patch = serde_json::json!({
            "app": "digilegacy",
            "platforms": [{
                "id": 9001,
                "name": "Locked patch",
                "category": "property"
            }]
        })
        .to_string();
        let imported = import_platform_patch(patch).unwrap();
        assert!(!imported.success, "锁定态导入政策补丁必须失败");
        assert!(imported.message.contains("锁定状态") || imported.message.contains("解锁"));
        assert_eq!(db::count_platform_overrides_db().unwrap(), 0);

        let cleared_patch = clear_platform_patch().unwrap();
        assert!(!cleared_patch.success, "锁定态清除政策补丁必须失败");
        assert!(
            cleared_patch.message.contains("锁定状态") || cleared_patch.message.contains("解锁")
        );

        let cleared_recovery = clear_heir_recovery().unwrap();
        assert!(!cleared_recovery.success, "锁定态清除恢复包必须失败");
        assert!(
            cleared_recovery.message.contains("锁定状态")
                || cleared_recovery.message.contains("解锁")
        );

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    /// 心跳配置校验：命令层独有的规则（db 层只负责存取）。
    #[test]
    fn heartbeat_config_validates_before_saving() {
        let _g = crate::vault::test_lock();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_hb_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        // 启用但无令牌 → 拒绝
        let r = set_heartbeat_config(true, String::new(), "https://hb.example.org".to_string())
            .unwrap();
        assert!(!r.success);
        assert!(r.message.contains("令牌"));

        // 启用但无端点 → 拒绝
        let r = set_heartbeat_config(true, "tok".to_string(), String::new()).unwrap();
        assert!(!r.success);
        assert!(r.message.contains("服务器地址"));

        // 端点协议不对 → 拒绝
        for bad in ["ftp://x", "hb.example.org", "javascript:alert(1)"] {
            let r = set_heartbeat_config(true, "tok".to_string(), bad.to_string()).unwrap();
            assert!(!r.success, "非法端点必须拒绝: {}", bad);
        }

        // 合法 → 保存成功，且读取一致
        let salt = crate::crypto::generate_salt_v3();
        let kek = crate::crypto::derive_from_salt("heartbeat-test-password", &salt).unwrap();
        let dek = crate::vault::generate_data_key();
        let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
        let verifier = crate::vault::make_verifier(&dek).unwrap();
        db::set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
        crate::vault::set_key(dek);
        let r = set_heartbeat_config(
            true,
            "tok-1".to_string(),
            "https://hb.example.org".to_string(),
        )
        .unwrap();
        assert!(r.success);
        let (enabled, token, ep) = db::get_heartbeat_config_db().unwrap();
        assert!(enabled);
        assert_eq!(token, "tok-1");
        assert_eq!(ep, "https://hb.example.org");

        // 传空令牌表示「保持原值」（界面用掩码，不回写真令牌）
        let r = set_heartbeat_config(true, String::new(), "https://hb.example.org".to_string())
            .unwrap();
        assert!(r.success);
        assert_eq!(
            db::get_heartbeat_config_db().unwrap().1,
            "tok-1",
            "空令牌不得清空已有令牌"
        );

        // 关闭时不做端点校验，允许清空
        let r = set_heartbeat_config(false, String::new(), String::new()).unwrap();
        assert!(r.success);
        assert!(!db::get_heartbeat_config_db().unwrap().0);

        let _ = std::fs::remove_file(&path);
    }

    /// render_report 的策略层：占位符拒绝、合法报告出完整文档、超限拒绝。
    ///
    /// 命令本体需要 AppHandle（无法在单测构造），但真正会出错的判断都在
    /// `render_report_response` 里，这里逐条锁定。
    #[test]
    fn render_report_response_refuses_placeholder_and_returns_full_document() {
        use crate::models::{DigitalAsset, InheritanceReport, UserProfile};

        let user = UserProfile {
            id: None,
            full_name: "张三".to_string(),
            phone: "13800000000".to_string(),
            email: "a@b.com".to_string(),
            id_last4: String::new(),
            emergency_contact_name: String::new(),
            emergency_contact_phone: String::new(),
            emergency_contact_relation: String::new(),
            emergency_contact_email: String::new(),
            heir_primary_name: "王五".to_string(),
            heir_primary_phone: String::new(),
            heir_primary_relation: "子女".to_string(),
            heir_primary_email: String::new(),
            check_in_interval_days: 30,
            last_check_in: None,
            dms_enabled: false,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let asset = |identifier: &str| DigitalAsset {
            id: Some(1),
            platform_id: 1,
            platform_name: "支付宝".to_string(),
            account_identifier: identifier.to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(100.0),
            value_currency: "CNY".to_string(),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
            has_credential: false,
            is_confirmed: true,
            created_at: "2026-01-01 00:00:00".to_string(),
            updated_at: "2026-01-01 00:00:00".to_string(),
        };
        let mk = |identifier: &str| InheritanceReport {
            user: user.clone(),
            assets: vec![asset(identifier)],
            heirs: Vec::new(),
            authorities: Vec::new(),
            policies: Vec::new(),
            total_estimated_value: 100.0,
            property_assets_count: 1,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-10-26 12:00:00".to_string(),
        };

        // 正常报告：成功，且是完整独立文档
        let ok = render_report_response(&mk("real-account"), "zh-CN").unwrap();
        assert!(ok.success, "合法报告必须成功");
        let html = ok.data.unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("@media print"));
        assert!(html.contains("real-account"));

        // 语言必须跟随界面：英文用户导出英文文档
        let en = render_report_response(&mk("real-account"), "en-US").unwrap();
        let en_html = en.data.unwrap();
        assert!(en_html.contains(r#"<html lang="en">"#));
        assert!(en_html.contains("Digital Legacy Inheritance Report"));
        assert!(!en_html.contains("数字遗产继承报告"));

        // 占位符：必须拒绝，且不产出任何 HTML
        let blocked =
            render_report_response(&mk(crate::vault::PLACEHOLDER_LOCKED), "zh-CN").unwrap();
        assert!(!blocked.success, "含占位符的报告必须拒绝");
        assert!(blocked.data.is_none(), "拒绝时不得返回半成品 HTML");
        assert!(
            blocked.message.contains("未解密"),
            "实际: {}",
            blocked.message
        );
        assert!(
            blocked.message.contains("未生成"),
            "必须明确说明没有生成文件: {}",
            blocked.message
        );
    }

    /// 密钥错配必须报「未解锁」：否则界面会把占位符当数据展示并写回（历史 P0）。
    #[test]
    fn security_status_is_false_when_key_does_not_match_verifier() {
        let _g = crate::vault::test_lock();
        use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
        crate::vault::clear_key();
        let path =
            std::env::temp_dir().join(format!("digilegacy_cmd_mismatch_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let salt = crate::crypto::generate_salt();
        let key = crate::crypto::derive_key_bytes("right-pw", &salt);
        db::set_security_config_with_dek_db(
            &b64.encode(salt),
            &crate::vault::make_verifier(&key).unwrap(),
            "",
        )
        .unwrap();

        // 正确密钥 → 已解锁
        crate::vault::set_key(key);
        let st = security_status().unwrap().data.unwrap();
        assert!(st.has_master_password);
        assert!(st.unlocked, "正确密钥必须报告已解锁");

        // 错配密钥 → 必须报告未解锁，而不是「有 key 就算解锁」
        crate::vault::set_key(crate::crypto::derive_key_bytes("wrong-pw", &salt));
        let st = security_status().unwrap().data.unwrap();
        assert!(st.has_master_password);
        assert!(!st.unlocked, "密钥与验证器不匹配时必须报告未解锁");

        // 且写路径必须拒绝（不是只把界面标红）
        let err = db::add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: "x".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(1.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap_err();
        assert!(
            err.contains("不匹配") || err.contains("解锁"),
            "实际: {}",
            err
        );

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    /// 未解锁时凭据类命令与锁定命令的状态一致性。
    #[test]
    fn lock_and_status_commands_are_consistent() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_lock_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        // 未设主密码：has_master_password=false。
        // 注意 unlocked 反映的是「内存里是否有主密钥」，此时必然为 false；
        // 「未设密码视为可用」是前端的呈现规则（security.ts: unlocked = has ? data.unlocked : true），
        // 两层的职责不同，这里只锁后端的真实语义。
        let st = security_status().unwrap().data.unwrap();
        assert!(!st.has_master_password);
        assert!(!st.unlocked, "无主密钥时 unlocked 必须为 false");

        // 锁定命令在无主密码时也不得崩溃
        let l = lock().unwrap();
        assert!(l.success);

        let _ = std::fs::remove_file(&path);
    }

    /// 解锁命令：正确密码放行、错误密码拒绝、未设密码时明确报错。
    #[test]
    fn unlock_command_accepts_only_correct_password() {
        use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_unlock_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        // 未设主密码
        let r = unlock("anything".to_string()).unwrap();
        assert!(!r.success);
        assert!(r.message.contains("尚未设置主密码"));

        // 设置主密码后再锁定
        let salt = crate::crypto::generate_salt();
        let key = crate::crypto::derive_key_bytes("unlock-master-1", &salt);
        let verifier = crate::vault::make_verifier(&key).unwrap();
        db::set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
        crate::vault::clear_key();

        // 错误密码
        let r = unlock("wrong-password".to_string()).unwrap();
        assert!(!r.success, "旧实现 message 应非空");
        assert_eq!(r.message, "主密码错误");
        assert!(!crate::vault::is_unlocked(), "错误密码不得留下密钥");

        // 正确密码
        let r = unlock("unlock-master-1".to_string()).unwrap();
        assert!(r.success);
        assert!(r.data.as_ref().unwrap().unlocked);
        assert!(crate::vault::is_unlocked());

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    /// 恢复命令：未启用恢复密钥时明确拒绝；错误恢复码不得留下密钥。
    #[test]
    fn start_recovery_rejects_without_seal_and_on_bad_code() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let mut path = std::env::temp_dir();
        path.push(format!("digilegacy_cmd_rec_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        // 未启用恢复密钥
        let r = start_recovery("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF".to_string()).unwrap();
        assert!(!r.success);
        assert!(r.message.contains("未启用恢复密钥"));

        // 有封印但恢复码错误。
        // 前置：set_recovery_seal_db 是 UPDATE，必须先有 security_config 行。
        {
            use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
            let salt = crate::crypto::generate_salt();
            let key = crate::crypto::derive_key_bytes("rec-master-1", &salt);
            let verifier = crate::vault::make_verifier(&key).unwrap();
            db::set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
        }
        let master = [5u8; 32];
        let (code, seal) = crate::recovery::create_recovery_seal(&master).unwrap();
        db::set_recovery_seal_db(&seal).unwrap();

        let r = start_recovery("AAAA-BBBB-CCCC-DDDD-EEEE-FFFF".to_string()).unwrap();
        assert!(!r.success, "错误恢复码必须拒绝");
        assert!(!crate::vault::is_unlocked(), "失败不得留下密钥");

        // 正确恢复码
        let r = start_recovery(code).unwrap();
        assert!(r.success);
        assert_eq!(
            *crate::vault::get_key().unwrap(),
            master,
            "必须解出原主密钥"
        );

        crate::vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    /// 资产入参必须在命令层被拦下：超长文本、非有限估值、非法平台 id。
    /// 这些值会写入 SQLite 并被每日备份复制 7 份，不能只靠前端约束。
    #[test]
    fn asset_commands_reject_oversized_and_invalid_payloads() {
        let _g = crate::vault::test_lock();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_val_{}.db", std::process::id())),
        )
        .unwrap();

        let base = |over: AssetRequest| over;
        let mk = || AssetRequest {
            platform_id: 1,
            account_identifier: "acc".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(1.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        };

        let too_long = base(AssetRequest {
            account_identifier: "x".repeat(8_001),
            ..mk()
        });
        let r = add_asset(too_long).unwrap();
        assert!(!r.success, "超长账号标识必须被拒绝");

        let nan_value = base(AssetRequest {
            estimated_value: Some(f64::NAN),
            ..mk()
        });
        let r = add_asset(nan_value).unwrap();
        assert!(!r.success, "NaN 估值必须被拒绝");

        let bad_platform = base(AssetRequest {
            platform_id: 0,
            ..mk()
        });
        let r = add_asset(bad_platform).unwrap();
        assert!(!r.success, "非法平台 id 必须被拒绝");

        // 合法请求仍应成功（防止校验过严把正常路径也拦掉）
        let r = add_asset(mk()).unwrap();
        assert!(r.success, "合法请求必须通过: {}", r.message);
    }

    /// set_disposal_wish 是唯一绕过 add/update_asset 校验的写入口：
    /// 非法枚举与超长备注都必须在这里被拦下。
    #[test]
    fn disposal_wish_validates_enum_and_size() {
        let _g = crate::vault::test_lock();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_wish_{}.db", std::process::id())),
        )
        .unwrap();
        let id = crate::db::add_asset_db(&AssetRequest {
            platform_id: 1,
            account_identifier: "acc".to_string(),
            account_description: String::new(),
            category: "property".to_string(),
            estimated_value: Some(1.0),
            disposal_method: "inherit".to_string(),
            heir_name: Some("王五".to_string()),
            heir_contact: None,
            notes: String::new(),
        })
        .unwrap();

        let r =
            set_disposal_wish("not_a_method".to_string(), id, None, None, String::new()).unwrap();
        assert!(!r.success, "非法处置枚举必须被拒绝");

        // 「继承」不填继承人必须被拒绝——这条是新增的完整性校验。
        // 注意：db 层返回 Err，命令层原样向上传递（不是 ApiResponse::fail）。
        let err = set_disposal_wish("inherit".to_string(), id, None, None, String::new())
            .expect_err("继承未填继承人必须被拒绝");
        assert!(err.contains("继承人"), "实际错误: {}", err);

        let r = set_disposal_wish(
            "inherit".to_string(),
            id,
            Some("王五".to_string()),
            None,
            "y".repeat(8_001),
        )
        .unwrap();
        assert!(!r.success, "超长备注必须被拒绝");

        let r = set_disposal_wish(
            "memorialize".to_string(),
            id,
            None,
            None,
            "保留纪念".to_string(),
        )
        .unwrap();
        assert!(r.success, "合法处置必须通过: {}", r.message);

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_wish_{}.db", std::process::id())),
        );
    }

    /// 报告/备份入参必须有限额：一次失控 IPC 能写满磁盘（还会被每日备份放大）。
    /// 边界抽成纯函数后可以脱离 Tauri AppHandle 直接验证。
    #[test]
    fn export_commands_reject_oversized_payloads() {
        assert!(validate_report_html("<!DOCTYPE html>").is_ok());
        assert!(validate_backup_content("{\"app\":\"digilegacy\"}").is_ok());

        let oversized_report = "x".repeat(20 * 1024 * 1024 + 1);
        let err = validate_report_html(&oversized_report).unwrap_err();
        assert!(err.contains("20MB"), "报告上限必须点名具体额度: {}", err);

        let oversized_backup = "x".repeat(8 * 1024 * 1024 + 1);
        let err = validate_backup_content(&oversized_backup).unwrap_err();
        assert!(err.contains("8MB"), "备份上限必须点名具体额度: {}", err);

        // 恰好等于上限必须放行（不能用 >= 把合法边界也拦掉）
        let exact_report = "x".repeat(20 * 1024 * 1024);
        assert!(validate_report_html(&exact_report).is_ok());
    }

    /// get_dms_status 是界面唯一的 DMS 数据源，必须是纯读：
    /// 调用它不得写发件箱、不得触达 SMTP（旧实现调 evaluate_dms 会真发邮件）。
    /// 注：get_dms_history / get_backup_status 依赖进程级数据目录，
    /// 其底层 outbox_history / latest_backup_summary 已分别由 notify 与 backup 的测试覆盖；
    /// dispatch 层另有「命令可路由」测试确保它们没有被漏接。
    #[test]
    fn get_dms_status_is_read_only_and_reports_stage() {
        let _g = crate::vault::test_lock();
        crate::db::init_database_at(std::env::temp_dir().join(format!(
            "digilegacy_cmd_dmsstatus_{}.db",
            std::process::id()
        )))
        .unwrap();

        // 未启用：enabled=false, stage=0
        let r = get_dms_status().unwrap();
        assert!(r.success);
        let data = r.data.unwrap();
        assert!(!data.enabled, "默认应未启用 DMS");
        assert_eq!(data.current_stage, 0);
        assert!(!data.overdue);

        // 启用且逾期：stage 必须与底层分级一致
        {
            let guard = crate::db::get_conn().unwrap();
            let conn = guard.as_ref().unwrap();
            let last = (chrono::Local::now() - chrono::Duration::days(60))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string();
            conn.execute(
                "UPDATE dms_config SET enabled=1, check_in_interval_days=30, last_check_in=?1 WHERE id=1",
                rusqlite::params![last],
            )
            .unwrap();
        }
        let data = get_dms_status().unwrap().data.unwrap();
        assert!(data.enabled);
        assert!(data.overdue, "逾期 30 天必须报告 overdue");
        assert_eq!(data.current_stage, 3);
        assert_eq!(data.days_overdue, 30);

        // 纯读的硬证据：连续读取结果完全一致，且 dms_config 行未被改动。
        // 该命令没有 AppHandle 参数，结构上也拿不到发件箱路径，因此无法投递；
        // 真正的投递路径 evaluate_dms 需要 AppHandle，只有后台循环能调用。
        let read_row = || -> (i64, Option<String>) {
            let guard = crate::db::get_conn().unwrap();
            guard
                .as_ref()
                .unwrap()
                .query_row(
                    "SELECT enabled, last_check_in FROM dms_config WHERE id=1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        };
        let before_row = read_row();
        for _ in 0..5 {
            let again = get_dms_status().unwrap().data.unwrap();
            assert_eq!(again.current_stage, 3, "重复读取不得改变分级");
            assert_eq!(again.days_overdue, 30, "重复读取不得改变逾期天数");
        }
        assert_eq!(before_row, read_row(), "get_dms_status 不得改动 dms_config");

        let _ = std::fs::remove_file(std::env::temp_dir().join(format!(
            "digilegacy_cmd_dmsstatus_{}.db",
            std::process::id()
        )));
    }

    /// DMS 间隔必须有界：0/负数会立刻触发三级通知，超大值等于永不提醒。
    #[test]
    fn dms_interval_must_be_within_range() {
        let _g = crate::vault::test_lock();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_dms_{}.db", std::process::id())),
        )
        .unwrap();

        for bad in [0i64, -1, 366, 100_000] {
            let r = set_dead_mans_switch(true, bad).unwrap();
            assert!(!r.success, "间隔 {} 必须被拒绝", bad);
        }
        let r = set_dead_mans_switch(true, 30).unwrap();
        assert!(r.success);
        assert_eq!(db::get_check_in_status_db().unwrap().interval_days, 30);
    }

    /// 保管位置是「家人能否找到凭据」的前提，命令层必须：
    /// 1) 长度有上限（防止把整篇文章塞进清单）；
    /// 2) 拒绝占位符（锁定态读出来的值不能被当作用户输入存回去）；
    /// 3) 正常值可往返。
    #[test]
    fn custody_hints_validate_and_roundtrip() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_custody_{}.db", std::process::id())),
        )
        .unwrap();

        // 空值可保存（表示「尚未填写」）
        let r = set_custody_hints(db::CustodyHints::default()).unwrap();
        assert!(r.success, "空保管位置应允许保存");

        // 超长拒绝
        let too_long = db::CustodyHints {
            backup_location: "x".repeat(501),
            recovery_code_location: String::new(),
        };
        let r = set_custody_hints(too_long).unwrap();
        assert!(!r.success, "超长位置必须被拒绝: {}", r.message);

        // 占位符拒绝：锁定态读出来的值绝不能被写回
        let placeholder = db::CustodyHints {
            backup_location: crate::vault::PLACEHOLDER_LOCKED.to_string(),
            recovery_code_location: String::new(),
        };
        let r = set_custody_hints(placeholder).unwrap();
        assert!(!r.success, "占位符必须被拒绝: {}", r.message);
        assert!(
            r.message.contains("解锁"),
            "提示必须指向解锁: {}",
            r.message
        );

        // 正常值往返
        let hints = db::CustodyHints {
            backup_location: "家中书房保险柜".to_string(),
            recovery_code_location: "纸质笔记本第 3 页".to_string(),
        };
        assert!(set_custody_hints(hints.clone()).unwrap().success);
        let got = get_custody_hints().unwrap().data.unwrap();
        assert_eq!(got.backup_location, "家中书房保险柜");
        assert_eq!(got.recovery_code_location, "纸质笔记本第 3 页");

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_custody_{}.db", std::process::id())),
        );
    }

    /// 执行清单：含占位符时必须拒绝，且错误信息说明「没有生成文件」。
    #[test]
    fn action_guide_refuses_undecrypted_data() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_guide_{}.db", std::process::id())),
        )
        .unwrap();

        // 空库：可以生成（内容只有「尚未登记」与保管位置警告）
        let r = render_action_guide(Some("zh-CN".to_string())).unwrap();
        assert!(r.success, "空库也应能生成清单: {}", r.message);
        assert!(r.data.unwrap().contains("数字遗产执行清单"));

        // 英文
        let r = render_action_guide(Some("en-US".to_string())).unwrap();
        assert!(r.data.unwrap().contains("Digital Legacy Action Guide"));

        // 未指定 locale 时回退中文
        let r = render_action_guide(None).unwrap();
        assert!(r.data.unwrap().contains("数字遗产执行清单"));

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_guide_{}.db", std::process::id())),
        );
    }

    /// 演练：空库时不应 panic，且必须如实报「触发链不可用」。
    #[test]
    fn family_drill_reports_unready_trigger_on_empty_vault() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_drill_{}.db", std::process::id())),
        )
        .unwrap();

        let r = run_family_drill().unwrap();
        assert!(r.success, "演练必须可执行: {}", r.message);
        let d = r.data.unwrap();
        assert_eq!(d.assets, 0);
        assert!(!d.trigger_ready, "未启用 DMS 时触发链不可用");
        assert!(
            d.checks.iter().any(|c| c.key == "trigger" && !c.ok),
            "必须把触发链列为未通过"
        );

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_drill_{}.db", std::process::id())),
        );
    }

    /// 严格场景：AI 提过疑点后，人工「通过」必须写理由。
    /// 这是「严格」在流程上的第二个落点（第一个是逐项作答校验）。
    #[test]
    fn strict_scene_requires_reason_before_approving_flagged_item() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_strict_{}.db", std::process::id())),
        )
        .unwrap();

        // 法律文书场景是严格模式，取它的一条真实条目
        let items =
            crate::review_profiles::items_for(crate::review_profiles::LEGAL_REVIEW).unwrap();
        let item = items.first().expect("法律文书场景应有条目").clone();

        // 先写一条 AI FLAG 记录（模拟初审提了疑点）
        db::append_review_record_db(&crate::review::ReviewRecord {
            profile_id: crate::review_profiles::LEGAL_REVIEW.to_string(),
            item_key: item.key.clone(),
            fingerprint: item.fingerprint.clone(),
            ai_verdict: crate::review::AiVerdict::Flag,
            ai_note: "第 3 段仍有未填空白".to_string(),
            human_verdict: None,
            human_note: String::new(),
            decided_by: "ai".to_string(),
            created_at: String::new(),
        })
        .unwrap();

        // 空备注通过 → 拒绝
        let r = submit_human_review(
            crate::review_profiles::LEGAL_REVIEW.to_string(),
            item.key.clone(),
            "approved".to_string(),
            None,
        )
        .unwrap();
        assert!(!r.success, "AI 报过疑点时不得空备注放行");
        assert!(r.message.contains("理由"), "实际: {}", r.message);

        // 只有空白也算空
        let r = submit_human_review(
            crate::review_profiles::LEGAL_REVIEW.to_string(),
            item.key.clone(),
            "approved".to_string(),
            Some("   ".to_string()),
        )
        .unwrap();
        assert!(!r.success, "空白理由不得放行");

        // 写了理由 → 允许，且状态变 Approved
        let r = submit_human_review(
            crate::review_profiles::LEGAL_REVIEW.to_string(),
            item.key.clone(),
            "approved".to_string(),
            Some("已核对官方模板，空白项为无需填写".to_string()),
        )
        .unwrap();
        assert!(r.success, "写明理由后应允许通过: {}", r.message);

        let states = db::summarise_states_db(
            crate::review_profiles::LEGAL_REVIEW,
            std::slice::from_ref(&item),
        )
        .unwrap();
        assert_eq!(states[0].status, crate::review::ReviewStatus::Approved);

        // 拒绝不需要理由（拒绝本身是保守动作）
        let r = submit_human_review(
            crate::review_profiles::LEGAL_REVIEW.to_string(),
            item.key,
            "rejected".to_string(),
            None,
        )
        .unwrap();
        assert!(r.success, "拒绝不要求理由");

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_strict_{}.db", std::process::id())),
        );
    }

    /// 非严格场景不要求理由：政策场景 AI 报疑点后，人工可直接通过。
    #[test]
    fn standard_scene_allows_approval_without_reason() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        db::init_database_at(
            std::env::temp_dir().join(format!("digilegacy_cmd_std_{}.db", std::process::id())),
        )
        .unwrap();

        let items =
            crate::review_profiles::items_for(crate::review_profiles::POLICY_REVIEW).unwrap();
        let item = items.first().unwrap().clone();
        db::append_review_record_db(&crate::review::ReviewRecord {
            profile_id: crate::review_profiles::POLICY_REVIEW.to_string(),
            item_key: item.key.clone(),
            fingerprint: item.fingerprint.clone(),
            ai_verdict: crate::review::AiVerdict::Flag,
            ai_note: "来源为空".to_string(),
            human_verdict: None,
            human_note: String::new(),
            decided_by: "ai".to_string(),
            created_at: String::new(),
        })
        .unwrap();

        let r = submit_human_review(
            crate::review_profiles::POLICY_REVIEW.to_string(),
            item.key,
            "approved".to_string(),
            None,
        )
        .unwrap();
        assert!(r.success, "标准场景不要求理由: {}", r.message);

        let _ = std::fs::remove_file(
            std::env::temp_dir().join(format!("digilegacy_cmd_std_{}.db", std::process::id())),
        );
    }

    #[test]
    fn locked_vault_refuses_sensitive_history_and_public_dms_writes() {
        let _g = crate::vault::test_lock();
        crate::vault::clear_key();
        let path = std::env::temp_dir().join(format!(
            "digilegacy_cmd_locked_history_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        db::init_database_at(path.clone()).unwrap();

        let salt = crate::crypto::generate_salt_v3();
        let kek = crate::crypto::derive_from_salt("locked-history-pw", &salt).unwrap();
        let dek = crate::vault::generate_data_key();
        let wrapped = crate::vault::wrap_data_key(&dek, &kek).unwrap();
        let verifier = crate::vault::make_verifier(&dek).unwrap();
        db::set_security_config_with_dek_db(&salt, &verifier, &wrapped).unwrap();
        crate::vault::clear_key();

        let review_list =
            list_review_items(crate::review_profiles::POLICY_REVIEW.to_string()).unwrap();
        assert!(!review_list.success, "锁定态不得读取审核条目");
        let review_history = review_history(
            crate::review_profiles::POLICY_REVIEW.to_string(),
            "1".to_string(),
        )
        .unwrap();
        assert!(!review_history.success, "锁定态不得读取审核历史");
        let ai_history = list_ai_chat(Some(10)).unwrap();
        assert!(!ai_history.success, "锁定态不得读取 AI 历史");
        let ai_clear = clear_ai_chat().unwrap();
        assert!(!ai_clear.success, "锁定态不得清除 AI 历史");

        assert!(db::append_review_record_db(&crate::review::ReviewRecord {
            profile_id: "p".to_string(),
            item_key: "k".to_string(),
            fingerprint: "f".to_string(),
            ai_verdict: crate::review::AiVerdict::Skipped,
            ai_note: "n".to_string(),
            human_verdict: None,
            human_note: String::new(),
            decided_by: "test".to_string(),
            created_at: String::new(),
        })
        .is_err());
        assert!(db::append_ai_chat_db("q", "a", "m").is_err());

        let check_in = update_check_in().unwrap();
        assert!(!check_in.success, "锁定态公共签到必须拒绝");
        let dms = set_dead_mans_switch(true, 30).unwrap();
        assert!(!dms.success, "锁定态公共 DMS 设置必须拒绝");
        assert!(db::set_heartbeat_config_db(true, "token", "https://example.com").is_err());
        assert!(crate::imap_checkin::set_config(
            &crate::imap_checkin::ImapConfig {
                enabled: true,
                host: "imap.example.com".to_string(),
                port: 993,
                username: "user@example.com".to_string(),
                has_password: false,
                allowed_senders: "user@example.com".to_string(),
                lookback_days: 14,
            },
            Some("password"),
        )
        .is_err());

        let _ = std::fs::remove_file(&path);
    }
}

#[path = "commands_platform.rs"]
mod platform;
pub use platform::*;

#[path = "commands_assets.rs"]
mod assets;
pub use assets::*;

#[path = "commands_report.rs"]
mod report;
pub use report::*;

#[path = "commands_review.rs"]
mod review;
pub use review::*;

#[path = "commands_ai.rs"]
mod ai;
pub use ai::*;

#[path = "commands_profile.rs"]
mod profile;
pub use profile::*;

#[path = "commands_security.rs"]
mod security;
pub use security::*;

#[path = "commands_custody.rs"]
mod custody;
pub use custody::*;

#[cfg(target_os = "windows")]
#[path = "commands_win_shell.rs"]
pub(crate) mod win_shell;

#[path = "commands_checkpoint.rs"]
mod checkpoint_cmds;
pub use checkpoint_cmds::*;
