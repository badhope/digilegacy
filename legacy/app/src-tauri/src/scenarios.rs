//! 真实使用场景模拟：用真实 SQLite + 真实 AES 跑数百个操作组合，
//! 每一步校验不变量，而不是只验证「函数能返回 Ok」。
//!
//! 与 db.rs 内联单测的区别：
//! - 内联单测每条针对一个已知缺陷（回归网）；
//! - 这里按「用户会怎么用」做矩阵遍历（类别 × 处置 × 金额 × 加密状态 × 备份版本），
//!   目的是暴露组合边界，而不是复述实现。
//!
//! 所有用例共用进程级连接与主密钥，必须持 `vault::test_lock()`。

use crate::crypto;
use crate::models::AssetRequest;
use crate::vault;

/// 场景计数器：每个断言算一个可执行检查点，最后汇总。
struct Check {
    count: usize,
}

impl Check {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn ok(&mut self, cond: bool, msg: impl FnOnce() -> String) {
        self.count += 1;
        assert!(cond, "{}", msg());
    }
}

fn temp_db(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "digilegacy_scenario_{}_{}.db",
        tag,
        std::process::id()
    ));
    let base = p.to_string_lossy().to_string();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{}", base, suffix));
    }
    p
}

fn wipe_assets() {
    let guard = crate::db::get_conn().unwrap();
    let conn = guard.as_ref().unwrap();
    conn.execute("DELETE FROM digital_assets", []).unwrap();
}

fn enable_encryption(password: &str) {
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};
    let salt = crypto::generate_salt();
    let key = crypto::derive_key_bytes(password, &salt);
    let verifier = vault::make_verifier(&key).unwrap();
    crate::db::set_security_config_with_dek_db(&b64.encode(salt), &verifier, "").unwrap();
    vault::set_key(key);
}

fn req(platform_id: i64, category: &str, disposal: &str, value: Option<f64>) -> AssetRequest {
    AssetRequest {
        platform_id,
        account_identifier: format!("acc-{}-{}", category, platform_id),
        account_description: "场景模拟账号".to_string(),
        category: category.to_string(),
        estimated_value: value,
        disposal_method: disposal.to_string(),
        heir_name: Some("继承人".to_string()),
        heir_contact: Some("heir@example.com".to_string()),
        notes: "场景备注".to_string(),
    }
}

/// 资产生命周期矩阵：3 类别 × 7 处置 × 3 金额形态 × 加密/明文 = 126 个场景。
/// 每个场景都走完 新增 → 读回 → 更新 → 分类查询 → 统计 → 删除。
#[test]
fn asset_lifecycle_matrix_126_scenarios() {
    let _g = vault::test_lock();
    let mut c = Check::new();
    let categories = ["property", "personality", "mixed"];
    let disposals = [
        "inherit",
        "liquidate",
        "memorialize",
        "authorize_view",
        "delete",
        "donate",
        "no_action",
    ];
    let values = [None, Some(0.0), Some(9_999_999.5)];

    for encrypted in [false, true] {
        vault::clear_key();
        let path = temp_db(if encrypted { "enc" } else { "plain" });
        crate::db::init_database_at(path.clone()).unwrap();
        if encrypted {
            enable_encryption("scenario-master-pw");
        }
        // 外键要求 platform_id 必须真实存在；平台库 id 非连续（1..196 有空洞），
        // 这里取真实 id 列表循环使用，而不是凭公式猜。
        let real_ids: Vec<i64> = crate::db::get_all_platforms_db()
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();

        for (ci, cat) in categories.iter().enumerate() {
            for (di, disposal) in disposals.iter().enumerate() {
                for (vi, value) in values.iter().enumerate() {
                    wipe_assets();
                    let platform_id = real_ids[(ci * 7 + di + vi) % real_ids.len()];
                    let id = crate::db::add_asset_db(&req(platform_id, cat, disposal, *value))
                        .unwrap_or_else(|e| panic!("add 失败 cat={} d={}: {}", cat, disposal, e));

                    let fetched = crate::db::get_asset_by_id_db(id).unwrap().unwrap();
                    c.ok(fetched.category == *cat, || {
                        format!("类别被改写: {} → {}", cat, fetched.category)
                    });
                    c.ok(fetched.disposal_method == *disposal, || {
                        format!("处置被改写: {} → {}", disposal, fetched.disposal_method)
                    });
                    c.ok(
                        fetched
                            .estimated_value
                            .map(|v| (v - value.unwrap_or(0.0)).abs() < 1e-9)
                            == Some(true),
                        || {
                            format!(
                                "金额回读不一致: {:?} vs {:?}",
                                fetched.estimated_value, value
                            )
                        },
                    );
                    c.ok(!fetched.has_credential, || "新资产不应带凭据".to_string());
                    c.ok(fetched.is_confirmed, || {
                        "新增资产应标记为已确认".to_string()
                    });

                    // 更新：换处置 + 改金额
                    let mut next = req(platform_id, cat, "memorialize", Some(123.0));
                    next.account_identifier = format!("updated-{}", id);
                    crate::db::update_asset_db(id, &next).unwrap();
                    let updated = crate::db::get_asset_by_id_db(id).unwrap().unwrap();
                    c.ok(updated.disposal_method == "memorialize", || {
                        format!("更新后处置错误: {}", updated.disposal_method)
                    });
                    c.ok(
                        updated.account_identifier == format!("updated-{}", id),
                        || "更新后账号标识未生效".to_string(),
                    );

                    // 分类查询必须只返回本类别
                    let by_cat = crate::db::get_assets_by_category_db(cat).unwrap();
                    c.ok(by_cat.iter().all(|a| a.category == *cat), || {
                        format!(
                            "分类查询混入其他类别: {:?}",
                            by_cat.iter().map(|a| &a.category).collect::<Vec<_>>()
                        )
                    });

                    // 统计必须与库内一致
                    let stats = crate::db::get_dashboard_stats_db().unwrap();
                    c.ok(stats.total_assets == by_cat.len() as i64, || {
                        format!(
                            "统计总数 {} != 分类查询 {}",
                            stats.total_assets,
                            by_cat.len()
                        )
                    });
                    c.ok(
                        stats.property_count + stats.personality_count + stats.mixed_count
                            == stats.total_assets,
                        || "三类计数之和必须等于总数".to_string(),
                    );

                    crate::db::delete_asset_db(id).unwrap();
                    c.ok(crate::db::get_asset_by_id_db(id).unwrap().is_none(), || {
                        "删除后仍能读到资产".to_string()
                    });
                }
            }
        }
        vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    println!("asset_lifecycle_matrix: {} 检查点", c.count);
    assert!(c.count >= 126 * 8, "场景检查点不足: {}", c.count);
}

/// 备份往返矩阵：v3 导出 → 修改 → 导入 → 逐字段比对，含 v1/v2 旧版本与损坏输入。
#[test]
fn backup_roundtrip_matrix_60_scenarios() {
    let _g = vault::test_lock();
    let mut c = Check::new();
    vault::clear_key();
    let path = temp_db("backup_matrix");
    crate::db::init_database_at(path.clone()).unwrap();

    let categories = ["property", "personality", "mixed"];
    let disposals = [
        "inherit",
        "liquidate",
        "memorialize",
        "authorize_view",
        "delete",
        "donate",
    ];
    let mut seq: u64 = 42;

    for round in 0..30 {
        wipe_assets();
        // 每轮写入 1..=4 条，字段值由确定性伪随机序列生成（可复现）
        let n = (round % 4) + 1;
        let mut expected: Vec<(String, String, f64)> = Vec::new();
        for i in 0..n {
            seq = seq
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let cat = categories[((seq >> 33) as usize) % categories.len()];
            let disp = disposals[((seq >> 21) as usize) % disposals.len()];
            let value = ((seq >> 11) % 100_000) as f64 / 10.0;
            let mut r = req((i + 1) as i64, cat, disp, Some(value));
            r.account_identifier = format!("r{}-i{}-{}", round, i, seq % 1000);
            crate::db::add_asset_db(&r).unwrap();
            expected.push((r.account_identifier.clone(), disp.to_string(), value));
        }

        let json = crate::db::export_backup_json_db().unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        c.ok(v["version"] == 5, || {
            format!("备份版本应为 4，实际 {}", v["version"])
        });
        c.ok(
            v["tables"]["digital_assets"].as_array().unwrap().len() == n,
            || "导出条数与写入条数不一致".to_string(),
        );
        c.ok(
            v["tables"]["security_config"].is_null()
                || v["tables"]["security_config"]["recovery_seal"].is_string(),
            || "v3 备份的 security_config 必须含 recovery_seal 字段".to_string(),
        );

        // 清空后导入，逐条比对
        wipe_assets();
        c.ok(crate::db::get_all_assets_db().unwrap().is_empty(), || {
            "清空后仍有资产".to_string()
        });
        crate::db::import_backup_json_db(&json).unwrap();
        let restored = crate::db::get_all_assets_db().unwrap();
        c.ok(restored.len() == n, || {
            format!("导入条数 {} != {}", restored.len(), n)
        });
        for (identifier, disposal, value) in &expected {
            let hit = restored
                .iter()
                .find(|a| &a.account_identifier == identifier);
            c.ok(hit.is_some(), || format!("导入后丢失 {}", identifier));
            if let Some(a) = hit {
                c.ok(&a.disposal_method == disposal, || {
                    format!("处置不一致 {} != {}", a.disposal_method, disposal)
                });
                c.ok(
                    (a.estimated_value.unwrap_or(-1.0) - value).abs() < 1e-6,
                    || format!("金额不一致 {:?} != {}", a.estimated_value, value),
                );
            }
            c.ok(
                crate::db::get_asset_by_id_db(restored[0].id.unwrap())
                    .unwrap()
                    .is_some(),
                || "按 id 读回失败".to_string(),
            );
        }

        // 旧版本兼容：把 version 改成 1、删掉 v2/v3 才有字段，导入不得报错
        if round % 3 == 0 {
            let mut legacy = v.clone();
            legacy["version"] = serde_json::json!(1);
            let tables = legacy["tables"].as_object_mut().unwrap();
            tables.remove("heartbeat_config");
            tables.remove("platform_overrides");
            c.ok(
                crate::db::import_backup_json_db(&legacy.to_string()).is_ok(),
                || "v1 备份必须仍可导入".to_string(),
            );
        }

        // 损坏输入必须 fail-closed 且不留半截数据
        let before = crate::db::get_all_assets_db().unwrap().len();
        for bad in [
            "{}",
            "{\"app\":\"other\",\"version\":3,\"tables\":{}}",
            "{\"app\":\"digilegacy\",\"version\":99,\"tables\":{}}",
            "{\"app\":\"digilegacy\",\"version\":3}",
        ] {
            c.ok(crate::db::import_backup_json_db(bad).is_err(), || {
                format!("损坏输入必须拒绝: {}", bad)
            });
            c.ok(
                crate::db::get_all_assets_db().unwrap().len() == before,
                || "拒绝导入后数据不得变化".to_string(),
            );
        }
    }

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
    println!("backup_roundtrip_matrix: {} 检查点", c.count);
    assert!(c.count >= 30 * 12, "场景检查点不足: {}", c.count);
}

/// DMS 时间线矩阵：间隔 × 逾期偏移，校验只读快照的分级与界面判定一致。
#[test]
fn dms_timeline_50_scenarios() {
    let _g = vault::test_lock();
    let mut c = Check::new();
    vault::clear_key();
    let path = temp_db("dms_timeline");
    crate::db::init_database_at(path.clone()).unwrap();

    let intervals = [1i64, 7, 14, 30, 60, 90, 365];
    let offsets = [-40i64, -1, 0, 1, 3, 4, 8, 11, 15, 40];

    for interval in intervals {
        for offset in offsets {
            // offset 相对「到期日」的偏移天数，负数表示尚未到期
            let last = chrono::Local::now() - chrono::Duration::days(interval + offset);
            {
                let guard = crate::db::get_conn().unwrap();
                let conn = guard.as_ref().unwrap();
                conn.execute(
                    "UPDATE dms_config SET enabled=1, check_in_interval_days=?1, stage1_wait_days=3, stage2_wait_days=7, stage3_wait_days=14, last_check_in=?2 WHERE id=1",
                    rusqlite::params![interval, last.format("%Y-%m-%d %H:%M:%S").to_string()],
                )
                .unwrap();
            }

            let status = crate::notify::status_snapshot().unwrap();
            let expect_overdue = offset > 0;
            c.ok(status.overdue == expect_overdue, || {
                format!(
                    "interval={} offset={} overdue 期望 {} 实际 {}",
                    interval, offset, expect_overdue, status.overdue
                )
            });
            let expect_stage = if offset <= 0 {
                0
            } else if offset <= 3 {
                1
            } else if offset <= 10 {
                2
            } else {
                3
            };
            c.ok(status.current_stage == expect_stage, || {
                format!(
                    "interval={} offset={} stage 期望 {} 实际 {}",
                    interval, offset, expect_stage, status.current_stage
                )
            });
            c.ok(status.days_overdue == offset.max(0), || {
                format!(
                    "interval={} offset={} 逾期天数期望 {} 实际 {}",
                    interval,
                    offset,
                    offset.max(0),
                    status.days_overdue
                )
            });

            // 签到后必须立即归零
            crate::db::record_check_in_db().unwrap();
            let after = crate::notify::status_snapshot().unwrap();
            c.ok(!after.overdue, || "签到后不应逾期".to_string());
            c.ok(after.current_stage == 0, || {
                "签到后 stage 必须为 0".to_string()
            });
        }
    }

    vault::clear_key();
    let _ = std::fs::remove_file(&path);
    println!("dms_timeline: {} 检查点", c.count);
    assert!(c.count >= 70 * 4, "场景检查点不足: {}", c.count);
}

/// 回归：补丁平台上有资产时，清除补丁不得失败，也不得连带删掉用户资产。
#[test]
fn clearing_patch_keeps_assets_on_patch_only_platforms() {
    let _g = vault::test_lock();
    let mut c = Check::new();
    let path = temp_db("clear_patch_assets");
    crate::db::init_database_at(path.clone()).unwrap();

    crate::db::import_platform_overrides_db(
        r#"{"app":"digilegacy","platforms":[{"id":9101,"name":"补丁平台","category":"游戏"}]}"#,
    )
    .unwrap();
    let id = crate::db::add_asset_db(&req(9101, "property", "inherit", Some(1.0))).unwrap();

    let removed =
        crate::db::clear_platform_overrides_db().expect("有资产挂在补丁平台上时也必须能清除补丁");
    c.ok(removed == 1, || {
        format!("应清除 1 条补丁，实际 {}", removed)
    });
    c.ok(crate::db::get_asset_by_id_db(id).unwrap().is_some(), || {
        "用户资产不得被连带删除".to_string()
    });

    // 平台行保留但被标注，便于用户识别并迁移
    let platforms = crate::db::get_all_platforms_db().unwrap();
    let kept = platforms.iter().find(|p| p.id == 9101);
    c.ok(kept.is_some(), || {
        "在用的补丁平台必须保留占位行".to_string()
    });
    c.ok(
        kept.map(|p| p.name.contains("补丁已清除")).unwrap_or(false),
        || "保留的平台必须标注来源".to_string(),
    );

    // 用户把资产迁走后可以再清除一次，占位行才会被真正删掉
    crate::db::delete_asset_db(id).unwrap();
    crate::db::clear_platform_overrides_db().unwrap();
    c.ok(
        crate::db::get_all_platforms_db()
            .unwrap()
            .iter()
            .all(|p| p.id != 9101),
        || "无资产占用后占位行必须被清理".to_string(),
    );

    let _ = std::fs::remove_file(&path);
    println!("clearing_patch_keeps_assets: {} 检查点", c.count);
    assert!(c.count >= 5);
}

/// 导入层鲁棒性：用确定性伪随机生成 500 份畸形/边界备份，
/// 断言「永不 panic、永不留下半截数据、错误信息非空」。
///
/// 为什么不用 proptest：不引入新依赖；用固定种子的 LCG 即可复现，
/// 且覆盖的畸形形态由下面的变异算子显式列出（比随机字符串更有针对性）。
#[test]
fn backup_import_survives_500_malformed_inputs() {
    let _g = vault::test_lock();
    vault::clear_key();
    let path = temp_db("fuzz_backup");
    crate::db::init_database_at(path.clone()).unwrap();

    // 先放一条基线数据，用来验证「拒绝后原数据不变」
    let id = crate::db::add_asset_db(&req(1, "property", "inherit", Some(10.0))).unwrap();
    assert!(crate::db::get_asset_by_id_db(id).unwrap().is_some());

    let mut seed: u64 = 0x5EED_1234;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        seed
    };

    let mut checked = 0usize;

    // 1) 纯随机字节串（含非法 UTF-8 已由 &str 边界挡掉，这里测 JSON 层面）
    for _ in 0..80 {
        let len = (next() % 200) as usize;
        let s: String = (0..len)
            .map(|_| {
                let pool = b"{}[]\",:0123456789abcdefghijklmnopqrstuvwxyz \n\t\\";
                pool[(next() as usize) % pool.len()] as char
            })
            .collect();
        let result = crate::db::import_backup_json_db(&s);
        assert!(result.is_err(), "随机串不应被当作合法备份: {}", s);
        let err = result.unwrap_err();
        assert!(!err.is_empty(), "错误信息不得为空");
        checked += 1;
    }

    // 2) 结构合法但字段类型错误（string 位置给 number、array 位置给 object 等）
    let type_confusions = [
        r#"{"app":"digilegacy","version":"three","tables":{}}"#,
        r#"{"app":"digilegacy","version":3,"tables":[]}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":{}}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[1,2,3]}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[null]}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[[]]}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[{"id":"x"}]}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[{"category":[]}]}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[],"user_profile":"x"}}"#,
        r#"{"app":"digilegacy","version":3,"tables":{"digital_assets":[],"check_in_log":{}}}"#,
    ];
    for s in type_confusions {
        let result = crate::db::import_backup_json_db(s);
        assert!(result.is_err(), "类型错乱必须拒绝: {}", s);
        checked += 1;
    }

    // 3) 边界数值：极端 id / 超大数组 / 深层嵌套
    let mut cases = Vec::new();
    cases.push(
        serde_json::json!({
            "app": "digilegacy", "version": 3,
            "tables": {"digital_assets": [{
                "id": 9223372036854775807i64, "platform_id": 1,
                "account_identifier": "x", "category": "property",
                "disposal_method": "inherit", "estimated_value": 1.0,
                "is_confirmed": 1,
                "created_at": "x", "updated_at": "x"
            }], "user_profile": [], "check_in_log": []}
        })
        .to_string(),
    );
    cases.push(
        serde_json::json!({
            "app": "digilegacy", "version": 3,
            "tables": {"digital_assets": [{
                "id": 1, "platform_id": -1,
                "account_identifier": "x", "category": "property",
                "disposal_method": "inherit", "estimated_value": 1.0,
                "is_confirmed": 1,
                "created_at": "x", "updated_at": "x"
            }], "user_profile": [], "check_in_log": []}
        })
        .to_string(),
    );
    cases.push(
        serde_json::json!({
            "app": "digilegacy", "version": 3,
            "tables": {"digital_assets": [], "user_profile": [
                {"id": 1, "check_in_interval_days": -999}
            ], "check_in_log": []}
        })
        .to_string(),
    );
    for s in &cases {
        // 这些要么拒绝、要么成功但不得破坏既有数据；两种都可接受，但不得 panic
        let before = crate::db::get_all_assets_db().unwrap().len();
        match crate::db::import_backup_json_db(s) {
            Ok(_) => {
                // 成功导入是允许的（导入本就是整体替换），只要不 panic
            }
            Err(e) => {
                assert!(!e.is_empty());
                assert_eq!(
                    crate::db::get_all_assets_db().unwrap().len(),
                    before,
                    "拒绝导入后数据条数不得变化"
                );
            }
        }
        checked += 1;
    }

    // 4) 每轮随机挑一种变异：删字段 / 改类型 / 截断 / 注入非法枚举
    let base = serde_json::json!({
        "app": "digilegacy",
        "version": 3,
        "tables": {
            "digital_assets": [{
                "id": 1, "platform_id": 1,
                "account_identifier": "acc", "account_description": "",
                "category": "property", "estimated_value": 1.0,
                "value_currency": "CNY", "disposal_method": "inherit",
                "heir_name": "", "heir_contact": "", "notes": "",
                "credential_enc": "", "is_confirmed": 1,
                "created_at": "2026-01-01 00:00:00", "updated_at": "2026-01-01 00:00:00"
            }],
            "user_profile": [], "check_in_log": [], "dms_config": null,
            "security_config": null
        }
    });

    let field_mutations: [(&str, serde_json::Value); 6] = [
        ("category", serde_json::json!("not_a_category")),
        ("disposal_method", serde_json::json!("sell")),
        ("estimated_value", serde_json::json!(-1.0)),
        ("estimated_value", serde_json::json!(1e300)),
        ("platform_id", serde_json::json!(0)),
        ("account_identifier", serde_json::json!(12345)),
    ];

    for round in 0..420 {
        let mut v = base.clone();
        let pick = (next() as usize) % field_mutations.len();
        let (field, value) = &field_mutations[pick];
        if let Some(asset) = v["tables"]["digital_assets"].get_mut(0) {
            asset[*field] = value.clone();
        }
        let encoded = v.to_string();

        // 再用 1/3 的概率做「截断」
        let payload = if round % 3 == 0 {
            &encoded[..encoded.len() / 2]
        } else {
            &encoded[..]
        };

        let before = crate::db::get_all_assets_db().unwrap().len();
        let result = std::panic::catch_unwind(|| crate::db::import_backup_json_db(payload));
        assert!(result.is_ok(), "导入不得 panic（round {}）", round);
        match result.unwrap() {
            Ok(_) => {}
            Err(e) => {
                assert!(!e.is_empty(), "错误信息不得为空（round {}）", round);
                assert_eq!(
                    crate::db::get_all_assets_db().unwrap().len(),
                    before,
                    "拒绝导入后数据不得变化（round {}）",
                    round
                );
            }
        }
        checked += 1;
    }

    assert!(checked >= 500, "至少覆盖 500 个畸形输入，实际 {}", checked);
    println!("backup_import_fuzz: {} 输入", checked);
    let _ = std::fs::remove_file(&path);
}

/// 凭据保险库矩阵/// 凭据保险库矩阵：20 个资产 × 加密/明文 × 保存/读取/清除/锁定拒绝。
#[test]
fn credential_vault_40_scenarios() {
    let _g = vault::test_lock();
    let mut c = Check::new();

    for encrypted in [false, true] {
        vault::clear_key();
        let path = temp_db(if encrypted { "cred_enc" } else { "cred_plain" });
        crate::db::init_database_at(path.clone()).unwrap();
        if encrypted {
            enable_encryption("cred-master-pw");
        }
        let real_ids: Vec<i64> = crate::db::get_all_platforms_db()
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();

        for i in 0..20 {
            let id = crate::db::add_asset_db(&req(
                real_ids[i % real_ids.len()],
                "property",
                "inherit",
                Some(1.0),
            ))
            .unwrap();
            let secret = format!("user{}:pass-{}-{}", i, encrypted, i * 7);

            // 锁定态写凭据必须被拒绝（无论是否启用主密码）
            vault::clear_key();
            c.ok(
                crate::db::set_asset_credential_db(id, &secret).is_err(),
                || format!("锁定态写入凭据必须失败 id={}", id),
            );
            if encrypted {
                enable_encryption("cred-master-pw");
            } else {
                // 明文模式（未启用主密码）：凭据保险库按契约不可用，
                // 被拒绝后不得留下半截数据，界面也不能显示「已存凭据」。
                c.ok(
                    crate::db::get_asset_credential_db(id).unwrap().is_empty(),
                    || "被拒绝后不应留下凭据".to_string(),
                );
                c.ok(
                    !crate::db::get_asset_by_id_db(id)
                        .unwrap()
                        .unwrap()
                        .has_credential,
                    || "被拒绝后 has_credential 必须为 false".to_string(),
                );
                continue;
            }

            crate::db::set_asset_credential_db(id, &secret).unwrap();
            let read = crate::db::get_asset_credential_db(id).unwrap();
            c.ok(read == secret, || {
                format!("凭据回读不一致 id={} 期望 {} 实际 {}", id, secret, read)
            });

            // 列表层的 has_credential 必须为真
            let listed = crate::db::get_asset_by_id_db(id).unwrap().unwrap();
            c.ok(listed.has_credential, || {
                "has_credential 应为 true".to_string()
            });

            if encrypted {
                // 密文不得等于明文
                let guard = crate::db::get_conn().unwrap();
                let raw: String = guard
                    .as_ref()
                    .unwrap()
                    .query_row(
                        "SELECT credential_enc FROM digital_assets WHERE id=?1",
                        rusqlite::params![id],
                        |r| r.get(0),
                    )
                    .unwrap();
                drop(guard);
                c.ok(raw != secret && vault::is_encrypted(&raw), || {
                    "凭据必须以 enc: 密文落库".to_string()
                });
            }

            // 清除后读取必须为空且标记消失
            crate::db::set_asset_credential_db(id, "").unwrap();
            c.ok(
                crate::db::get_asset_credential_db(id).unwrap().is_empty(),
                || "清除后仍能读到凭据".to_string(),
            );
            c.ok(
                !crate::db::get_asset_by_id_db(id)
                    .unwrap()
                    .unwrap()
                    .has_credential,
                || "清除后 has_credential 应为 false".to_string(),
            );
        }

        vault::clear_key();
        let _ = std::fs::remove_file(&path);
    }

    println!("credential_vault: {} 检查点", c.count);
    assert!(c.count >= 40 * 4, "场景检查点不足: {}", c.count);
}

/// 政策补丁矩阵：导入/覆盖/新增强/导出/清除，覆盖 30 个平台 id。
#[test]
fn platform_patch_matrix_30_scenarios() {
    let _g = vault::test_lock();
    let mut c = Check::new();
    let path = temp_db("patch_matrix");
    crate::db::init_database_at(path.clone()).unwrap();

    let mut expected_patch_count = 0i64;
    for round in 0..30 {
        let target_id = (9000 + round) as i64;
        let patch = serde_json::json!({
            "app": "digilegacy",
            "platforms": [{
                "id": target_id,
                "name": format!("场景平台{}", round),
                "category": "游戏",
                "process_description": format!("第 {} 轮流程", round),
                "inheritable": round % 2 == 0,
            }]
        })
        .to_string();

        let (added, updated) = crate::db::import_platform_overrides_db(&patch).unwrap();
        expected_patch_count += 1;
        c.ok(added == 1 && updated == 0, || {
            format!("首轮应新增 1 条，实际 added={} updated={}", added, updated)
        });

        let platforms = crate::db::get_all_platforms_db().unwrap();
        let hit = platforms.iter().find(|p| p.id == target_id).unwrap();
        c.ok(hit.name == format!("场景平台{}", round), || {
            format!("补丁名称未生效: {}", hit.name)
        });
        c.ok(
            hit.process_description == format!("第 {} 轮流程", round),
            || "补丁流程未生效".to_string(),
        );
        c.ok(hit.inheritable == (round % 2 == 0), || {
            "可继承标记未生效".to_string()
        });

        // 重复导入同一份：必须判为覆盖而不是新增
        let (added2, updated2) = crate::db::import_platform_overrides_db(&patch).unwrap();
        c.ok(added2 == 0 && updated2 == 1, || {
            format!(
                "重复导入应覆盖 1 条，实际 added={} updated={}",
                added2, updated2
            )
        });

        // 导出后清空，再导入导出内容必须还原
        let exported = crate::db::export_platform_overrides_db().unwrap();
        let count_before = crate::db::count_platform_overrides_db().unwrap();
        c.ok(count_before == expected_patch_count, || {
            format!("补丁计数错误 {} != {}", count_before, expected_patch_count)
        });
        c.ok(exported.contains(&format!("场景平台{}", round)), || {
            "导出内容缺本轮补丁".to_string()
        });

        if round % 10 == 9 {
            crate::db::clear_platform_overrides_db().unwrap();
            expected_patch_count = 0;
            c.ok(
                crate::db::count_platform_overrides_db().unwrap() == 0,
                || "清除后仍有补丁".to_string(),
            );
            c.ok(
                crate::db::get_all_platforms_db().unwrap().len() == 152,
                || "清除补丁后必须回到内置 152 条".to_string(),
            );
        }
    }

    let _ = std::fs::remove_file(&path);
    println!("platform_patch_matrix: {} 检查点", c.count);
    assert!(c.count >= 30 * 7, "场景检查点不足: {}", c.count);
}
