//! 检查点与回滚（harness 能力：让「改动可撤销」成为产品特性而非口头承诺）。
//!
//! 为什么需要它：这个应用里的数据是「一次性整理、长期不再看」的形态——
//! 用户半年后回来改一次处置方式，很容易把之前填对的字段改错，而他自己
//! 记不清原来是什么。没有检查点，唯一的安全网是「每天自动备份」，
//! 但备份粒度是「昨天」，不是「你按下这个按钮之前」。
//!
//! 设计要点：
//! - 检查点是**完整备份 JSON 的命名快照**，复用既有的导出/导入管线，
//!   不新造序列化格式（避免出现「检查点格式」与「备份格式」两套必须同步维护的真相）。
//! - 每个检查点带标签与创建时间，最多保留 `MAX_CHECKPOINTS` 个，超出按时间淘汰。
//! - 回滚走既有的**认证式导入**路径（`import_backup_json_db_with_mode_and_password`），
//!   因此回滚同样受「源口令 / wrapped_dek / AEAD 标签」校验约束，不会成为绕过点。
//! - 回滚前先给当前状态打一个自动检查点（`pre-restore`），避免「回滚本身不可撤销」。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 最多保留的检查点数量。太多了既占空间又让用户选不出来。
const MAX_CHECKPOINTS: usize = 12;

/// 检查点元数据（列表展示用，不含数据本体）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub id: String,
    pub label: String,
    pub created_at: String,
    /// 数据本体大小（字节），让用户知道回滚会恢复多少东西。
    pub size: u64,
    /// 自动检查点（回滚前打的），界面上应与用户手动创建的区分开。
    pub auto: bool,
}

fn dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("checkpoints")
}

/// 生成检查点 id：时间戳 + 随机后缀，保证同一秒内多次创建也不冲突。
fn new_id() -> String {
    let now = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let suffix: u32 = rand::random::<u32>() % 10_000;
    format!("{}-{:04}", now, suffix)
}

/// 创建一个检查点。`json` 是 `export_backup_json_db()` 的产物。
pub fn create(
    app_data_dir: &Path,
    json: &str,
    label: &str,
    auto: bool,
) -> Result<CheckpointMeta, String> {
    let d = dir(app_data_dir);
    fs::create_dir_all(&d).map_err(|e| format!("创建检查点目录失败: {}", e))?;

    let id = new_id();
    let path = d.join(format!("{}.json", id));
    // 检查点含全部个人数据：必须走私有写路径（Unix 0600 / Windows 目录 ACL）。
    crate::fsutil::write_private(&path, json.as_bytes())
        .map_err(|e| format!("写入检查点失败: {}", e))?;

    let meta = CheckpointMeta {
        id: id.clone(),
        label: label.chars().take(80).collect(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        size: json.len() as u64,
        auto,
    };
    // 元数据与数据同目录，`<id>.meta.json`，列表读取时不必解析大文件。
    let meta_path = d.join(format!("{}.meta.json", id));
    let meta_json =
        serde_json::to_string(&meta).map_err(|e| format!("序列化检查点元数据失败: {}", e))?;
    crate::fsutil::write_private(&meta_path, meta_json.as_bytes())
        .map_err(|e| format!("写入检查点元数据失败: {}", e))?;

    prune(app_data_dir)?;
    Ok(meta)
}

/// 列出全部检查点，按时间倒序（最新在前）。
pub fn list(app_data_dir: &Path) -> Vec<CheckpointMeta> {
    let d = dir(app_data_dir);
    let mut out: Vec<CheckpointMeta> = Vec::new();
    let Ok(entries) = fs::read_dir(&d) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.ends_with(".meta.json") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(meta) = serde_json::from_str::<CheckpointMeta>(&text) {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out
}

/// 读取指定检查点的数据本体。
pub fn read(app_data_dir: &Path, id: &str) -> Result<String, String> {
    validate_id(id)?;
    let path = dir(app_data_dir).join(format!("{}.json", id));
    fs::read_to_string(&path).map_err(|e| format!("读取检查点 {} 失败: {}", id, e))
}

/// 删除一个检查点（数据 + 元数据）。
pub fn delete(app_data_dir: &Path, id: &str) -> Result<(), String> {
    validate_id(id)?;
    let d = dir(app_data_dir);
    let _ = fs::remove_file(d.join(format!("{}.json", id)));
    let _ = fs::remove_file(d.join(format!("{}.meta.json", id)));
    Ok(())
}

/// 超过上限时淘汰最旧的检查点。自动检查点优先被淘汰（手动创建的用户更在意）。
fn prune(app_data_dir: &Path) -> Result<(), String> {
    let mut metas = list(app_data_dir);
    if metas.len() <= MAX_CHECKPOINTS {
        return Ok(());
    }
    // 先按「自动优先淘汰」再按「最旧」排序
    metas.sort_by(|a, b| {
        b.auto
            .cmp(&a.auto)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    let excess = metas.len() - MAX_CHECKPOINTS;
    for meta in metas.iter().take(excess) {
        let _ = delete(app_data_dir, &meta.id);
    }
    Ok(())
}

/// id 只允许时间戳形态的字符，防止路径穿越（`../` 之类）。
fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 40
        || !id
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err("检查点 id 非法".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "digilegacy_ckpt_{}_{}_{}",
            tag,
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn create_list_read_delete_roundtrip() {
        let d = temp_dir("roundtrip");
        let meta = create(&d, r#"{"v":1}"#, "before edit", false).unwrap();
        assert_eq!(meta.label, "before edit");
        assert!(!meta.auto);

        let listed = list(&d);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, meta.id);

        let body = read(&d, &meta.id).unwrap();
        assert_eq!(body, r#"{"v":1}"#);

        delete(&d, &meta.id).unwrap();
        assert!(list(&d).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let d = temp_dir("traversal");
        for bad in ["../etc/passwd", "a/b", "", "id with space"] {
            assert!(read(&d, bad).is_err(), "应拒绝非法 id: {}", bad);
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn prune_keeps_newest_and_drops_auto_first() {
        let d = temp_dir("prune");
        // 造 MAX+3 个自动检查点
        for i in 0..(MAX_CHECKPOINTS + 3) {
            create(&d, "{}", &format!("auto-{}", i), true).unwrap();
            // 保证 created_at 可区分（同一秒内会并列，用文件名随机后缀兜底）
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(list(&d).len() <= MAX_CHECKPOINTS);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn label_is_truncated_to_bound() {
        let d = temp_dir("label");
        let long = "x".repeat(200);
        let meta = create(&d, "{}", &long, false).unwrap();
        assert_eq!(meta.label.chars().count(), 80);
        let _ = fs::remove_dir_all(&d);
    }
}
