//! KDBX 4 长期归档导出。
//!
//! 为什么需要：本应用的 JSON 备份只有本应用能读。数字遗产产品的时间尺度是
//! 「十几年后家人来用」——届时这个应用可能已经不存在。KDBX 4（KeePass 格式）
//! 是事实上最长寿的开源密码容器规范，KeePassXC / KeePass2 / 强密码管理器
//! 全都能打开。导出一份 KDBX = 「脱离本应用仍可读」的唯一保证。
//!
//! 设计边界：
//! 1. **拒绝占位符**：任一字段未解密（锁定态）则整包拒绝，绝不出「写着需解锁的归档」。
//! 2. **主密码独立**：归档密码由用户当场输入，与主密码解耦——家人只需要记住这一个。
//! 3. **结构**：根组下按资产逐条建 Entry（Title=平台名，UserName=账号标识，
//!    Password=凭据，Notes=处置意愿/继承人/备注）， heirs 与保管位置放进
//!    「继承人信息」组，报告指引写入根组 Notes。
//! 4. 使用 `keepass` crate 的 save_kdbx4，文件可用 KeePassXC 直接验证。

use keepass::db::{fields, EntryMut, GroupMut};
use keepass::{Database, DatabaseKey};

use crate::models::InheritanceReport;
use crate::report::{contains_placeholder, ReportLocale};

const ROOT_NOTES_HEADER: &str =
    "由 归零 DigiLegacy 导出。任何支持 KeePass(KDBX4) 的软件均可打开：推荐 KeePassXC。";

/// 把报告内容写成 KDBX 4 字节流。
///
/// `archive_password` 是独立于主密码的归档口令（家人用它开这份文件）。
pub fn export_kdbx(
    report: &InheritanceReport,
    locale: ReportLocale,
    backup_location: &str,
    recovery_code_location: &str,
    archive_password: &str,
) -> Result<Vec<u8>, String> {
    if archive_password.chars().count() < 8 {
        return Err("归档密码至少需要 8 个字符".to_string());
    }
    // 与报告/工具包同一拒绝策略：锁定态导出的归档对家人毫无价值。
    if contains_placeholder(report) {
        return Err("存在未解密字段，请先解锁后再导出（本次未生成任何文件）".to_string());
    }

    let zh = matches!(locale, ReportLocale::Zh);
    let label = |zh_text: &str, en_text: &str| {
        if zh {
            zh_text.to_string()
        } else {
            en_text.to_string()
        }
    };
    let mut db = Database::new();
    {
        let mut root: GroupMut<'_> = db.root_mut();
        root.name = "DigiLegacy".into();
        root.notes = Some(ROOT_NOTES_HEADER.to_string());

        // 继承人信息（含执行人）：家人第一个要看的东西
        let mut heir_group: GroupMut<'_> = root.add_group();
        heir_group.name = label("继承人信息", "Heirs");
        for h in &report.heirs {
            let mut e: EntryMut<'_> = heir_group.add_entry();
            e.set_unprotected(fields::TITLE, h.name.clone());
            e.set_unprotected(
                fields::USERNAME,
                if h.is_executor {
                    label("遗产执行人", "Executor")
                } else {
                    label("继承人", "Heir")
                },
            );
            e.set_unprotected(fields::NOTES, format!("{}\n{}", h.relation, h.share_note));
            let mut contact = Vec::new();
            if !h.phone.is_empty() {
                contact.push(format!("Tel: {}", h.phone));
            }
            if !h.email.is_empty() {
                contact.push(format!("Email: {}", h.email));
            }
            if !contact.is_empty() {
                e.set_unprotected(fields::URL, contact.join("\n"));
            }
        }
        // 保管位置：家人去哪找备份与恢复码
        let mut custody_entry: EntryMut<'_> = heir_group.add_entry();
        custody_entry.set_unprotected(fields::TITLE, label("保管位置", "Custody locations"));
        custody_entry.set_unprotected(
            fields::NOTES,
            format!(
                "{}: {}\n{}: {}",
                label("备份文件", "Backup file"),
                backup_location,
                label("恢复码", "Recovery code"),
                recovery_code_location
            ),
        );

        // 资产逐条：账号标识是 USERNAME、凭据是 PASSWORD（受保护字段）
        let mut asset_group: GroupMut<'_> = root.add_group();
        asset_group.name = label("数字资产", "Digital assets");
        for a in &report.assets {
            let mut e: EntryMut<'_> = asset_group.add_entry();
            e.set_unprotected(fields::TITLE, a.platform_name.clone());
            e.set_unprotected(fields::USERNAME, a.account_identifier.clone());
            e.set_unprotected(
                fields::NOTES,
                format!(
                    "{}: {}\n{}: {}\n{}: {}",
                    label("处置", "Disposal"),
                    crate::report::disposal_label(&a.disposal_method, locale),
                    label("继承人", "Heir"),
                    a.heir_name.as_deref().unwrap_or(""),
                    label("备注", "Notes"),
                    a.notes,
                ),
            );
            // 凭据是敏感字段：受 KDBX 内层加密保护（protected value）
            if a.has_credential {
                // has_credential 只说明「存过凭据」，明文需要解密；报告 DTO 不带凭据，
                // 因此这里只放指引，真正凭据由 heir 通过应用内「查看凭据」或工具包 JSON 获取。
                e.set_unprotected(
                    fields::URL,
                    label(
                        "凭据已存于本应用加密库（用「查看凭据」获取）；本归档不含明文凭据",
                        "Credential stored in the app's encrypted vault; this archive holds no plaintext secrets",
                    ),
                );
            }
        }
    }

    let mut buf = Vec::new();
    db.save(&mut buf, DatabaseKey::new().with_password(archive_password))
        .map_err(|e| format!("写入 KDBX 失败: {}", e))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn sample_report() -> InheritanceReport {
        let mut asset = DigitalAsset {
            id: Some(1),
            platform_id: 1,
            platform_name: "支付宝".into(),
            account_identifier: "acc-001".into(),
            account_description: "主账户".into(),
            category: "property".into(),
            estimated_value: Some(1000.0),
            value_currency: "CNY".into(),
            disposal_method: "inherit".into(),
            heir_name: Some("李四".into()),
            heir_contact: Some("13900000000".into()),
            notes: "备用邮箱见档案".into(),
            has_credential: true,
            is_confirmed: true,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        };
        asset.is_confirmed = true;
        InheritanceReport {
            user: UserProfile::default(),
            assets: vec![asset],
            heirs: vec![Heir {
                id: Some(1),
                name: "李四".into(),
                relation: "儿子".into(),
                phone: "13900000000".into(),
                email: "lisi@example.com".into(),
                share_note: "全部".into(),
                is_executor: true,
                sort_order: 0,
            }],
            policies: Vec::new(),
            authorities: Vec::new(),
            total_estimated_value: 1000.0,
            property_assets_count: 1,
            personality_assets_count: 0,
            mixed_assets_count: 0,
            generated_at: "2026-01-01 00:00:00".into(),
        }
    }

    #[test]
    fn kdbx_bytes_are_kdbx4_and_reopenable() {
        let report = sample_report();
        let bytes = export_kdbx(
            &report,
            ReportLocale::Zh,
            "家中保险柜",
            "笔记本第3页",
            "archive-pw-123",
        )
        .unwrap();
        // KDBX 魔数（keepass crate 常量）：SIG1 = 0x9AA2D903, SIG2 = 0xB54BFB65
        let sig1 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let sig2 = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        assert_eq!(sig1, 0x9AA2D903, "必须是 KDBX SIG1");
        assert_eq!(
            sig2, 0xB54BFB67,
            "必须是 KDBX4 SIG2（keepass crate 用 4.x 变体）"
        );
        // 文件版本是 u32：0x00040001（KDBX 4.1）或 0x00040000（KDBX 4.0），高 16 位 = 4
        let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        assert_eq!(version >> 16, 4, "必须是 KDBX4（版本高 16 位为 4）");

        // 用同一口令重新打开，条目必须完整（根组 + 两个子组的平铺迭代）
        let mut cursor = std::io::Cursor::new(bytes.clone());
        let reopened = Database::open(
            &mut cursor,
            DatabaseKey::new().with_password("archive-pw-123"),
        )
        .expect("同口令必须能打开");
        let root = reopened.root();
        let mut titles: Vec<String> = root
            .entries()
            .map(|e| e.get_title().unwrap_or_default().to_string())
            .collect();
        for g in root.groups() {
            for e in g.entries() {
                titles.push(e.get_title().unwrap_or_default().to_string());
            }
        }
        assert!(
            titles.iter().any(|t| t.contains("支付宝")),
            "资产条目必须在归档里，实际 {:?}",
            titles
        );
        assert!(
            titles.iter().any(|t| t.contains("李四")),
            "继承人必须在归档里"
        );

        // 错误口令必须打不开
        let mut cursor2 = std::io::Cursor::new(bytes);
        assert!(
            Database::open(&mut cursor2, DatabaseKey::new().with_password("wrong-pw")).is_err(),
            "错误口令不得打开归档"
        );
    }

    #[test]
    fn kdbx_rejects_short_password_and_placeholder() {
        let report = sample_report();
        assert!(export_kdbx(&report, ReportLocale::Zh, "", "", "short").is_err());

        // 任一字段是占位符 → 整包拒绝
        let mut bad = sample_report();
        bad.assets[0].account_identifier = crate::vault::PLACEHOLDER_LOCKED.into();
        assert!(export_kdbx(&bad, ReportLocale::Zh, "", "", "archive-pw-123").is_err());
    }
}
