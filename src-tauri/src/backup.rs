//! 自动备份：应用启动时检查当日是否已有快照，没有则写一份并清理旧备份。
//!
//! 为什么需要：本地 SQLite 是唯一数据源，磁盘损坏 / 误删 / 误导入都会永久丢数据。
//! 手动备份依赖用户记得点，实测很少有人做——所以做成静默的每日快照。
//!
//! 设计取舍：
//! - **只保留 7 份**：够覆盖「发现数据不对」的常见窗口，又不会无限膨胀。
//! - **不加密**：备份内容本身已是字段级密文（enc:v1），再包一层没有额外收益，
//!   反而会让「主密码丢失时无法读取备份元信息」更糟。
//! - **失败不阻断启动**：备份是附加保障，磁盘满或权限问题不能阻止应用可用。

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

/// 保留的快照数量（含当日）。
const KEEP: usize = 7;

/// 备份目录名（位于应用数据目录下）。
const DIR_NAME: &str = "自动备份";

fn backup_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(DIR_NAME)
}

fn today_stamp() -> String {
    Local::now().format("%Y%m%d").to_string()
}

/// 执行一次自动备份检查。
///
/// 返回 `Ok(Some(path))` 表示新建了快照，`Ok(None)` 表示当日已存在（跳过）。
pub fn run_daily_backup(app_data_dir: &Path, json: &str) -> Result<Option<PathBuf>, String> {
    let dir = backup_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {}", e))?;

    let stamp = today_stamp();
    let target = dir.join(format!("digilegacy-{}.json", stamp));
    if target.exists() {
        return Ok(None);
    }

    // 先写临时文件再原子改名：避免写入中断留下半个损坏的备份
    let tmp = dir.join(format!(".digilegacy-{}.json.tmp", stamp));
    crate::fsutil::write_private(&tmp, json.as_bytes())
        .map_err(|e| format!("写入备份失败: {}", e))?;
    fs::rename(&tmp, &target).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("保存备份失败: {}", e)
    })?;

    prune(&dir);
    Ok(Some(target))
}

/// 只保留最近的 `KEEP` 份快照（按文件名日期倒序）。
fn prune(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|s| s.to_str()) == Some("json")
                && p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with("digilegacy-"))
                    .unwrap_or(false)
        })
        .collect();

    if files.len() <= KEEP {
        return;
    }
    // 文件名里带 yyyymmdd，字典序即时间序
    files.sort();
    let remove_count = files.len() - KEEP;
    for p in files.into_iter().take(remove_count) {
        let _ = fs::remove_file(p);
    }
}

/// 列出已有快照（供测试与「备份状态」界面使用）。
pub fn list_backups(app_data_dir: &Path) -> Vec<PathBuf> {
    let dir = backup_dir(app_data_dir);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    files.sort();
    files
}

/// 导入前快照：写在自动备份目录，命名带秒级时间戳（同日多次导入不会互相覆盖）。
///
/// 与每日快照的区别：每日快照一天只留一份（当天已有就跳过），
/// 而「导入前」这一刻的状态必须被无条件保存——覆盖导入是不可逆操作。
/// 该文件同样计入 `KEEP` 裁剪，避免无限增长。
pub fn write_pre_import_snapshot(app_data_dir: &Path, json: &str) -> Result<PathBuf, String> {
    let dir = backup_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {}", e))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut target = dir.join(format!("digilegacy-preimport-{}.json", stamp));
    // 同一秒内连续导入两次：加序号，绝不覆盖前一份快照。
    let mut n = 1;
    while target.exists() {
        target = dir.join(format!("digilegacy-preimport-{}-{}.json", stamp, n));
        n += 1;
    }

    let tmp = dir.join(format!(".digilegacy-preimport-{}.json.tmp", stamp));
    crate::fsutil::write_private(&tmp, json.as_bytes())
        .map_err(|e| format!("写入导入前快照失败: {}", e))?;
    fs::rename(&tmp, &target).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("保存导入前快照失败: {}", e)
    })?;

    prune(&dir);
    Ok(target)
}

/// 最近一次自动备份的文件名与大小（界面展示用；无快照返回 None）。
pub fn latest_backup_summary(app_data_dir: &Path) -> Option<(String, u64)> {
    let latest = list_backups(app_data_dir).pop()?;
    let name = latest.file_name()?.to_string_lossy().to_string();
    let size = fs::metadata(&latest).ok()?.len();
    Some((name, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "digilegacy_backup_test_{}_{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn creates_one_snapshot_per_day_and_skips_second_call() {
        let base = temp_dir("daily");
        let first = run_daily_backup(&base, "{\"v\":1}").unwrap();
        assert!(first.is_some(), "首次调用应创建快照");

        let path = first.unwrap();
        assert!(path.exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"v\":1}");

        // 同日第二次调用必须跳过（幂等）
        let second = run_daily_backup(&base, "{\"v\":2}").unwrap();
        assert!(second.is_none(), "同一天不应重复备份");
        // 内容保持首次写入的版本（未被覆盖）
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"v\":1}");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn prunes_to_keep_limit() {
        let base = temp_dir("prune");
        let dir = backup_dir(&base);
        fs::create_dir_all(&dir).unwrap();

        // 造 10 份历史快照
        for d in 1..=10 {
            fs::write(dir.join(format!("digilegacy-202601{:02}.json", d)), "{}").unwrap();
        }
        assert_eq!(list_backups(&base).len(), 10);

        // 再触发一次备份 → 应触发 prune
        run_daily_backup(&base, "{}").unwrap();

        let remaining = list_backups(&base).len();
        assert!(
            remaining <= KEEP,
            "备份数量应被裁剪到 {} 以内，实际 {}",
            KEEP,
            remaining
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn pruned_oldest_is_removed_first() {
        let base = temp_dir("order");
        let dir = backup_dir(&base);
        fs::create_dir_all(&dir).unwrap();
        for d in 1..=9 {
            fs::write(dir.join(format!("digilegacy-202601{:02}.json", d)), "{}").unwrap();
        }

        run_daily_backup(&base, "{}").unwrap();
        let names: Vec<String> = list_backups(&base)
            .iter()
            .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
            .collect();

        assert!(
            !names.iter().any(|n| n.contains("20260101")),
            "最旧的快照应被删除，实际: {:?}",
            names
        );
        assert!(
            names.iter().any(|n| n.contains("20260109")),
            "较新的快照应保留"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn no_temp_files_left_behind() {
        let base = temp_dir("tmp");
        run_daily_backup(&base, "{}").unwrap();
        let leftovers: Vec<_> = fs::read_dir(backup_dir(&base))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "不应残留临时文件");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn latest_summary_reports_newest_file_and_size() {
        let base = temp_dir("latest");
        run_daily_backup(&base, "{}").unwrap();
        let (name, size) = latest_backup_summary(&base).expect("应有最近备份");
        assert!(name.starts_with("digilegacy-"));
        assert!(name.ends_with(".json"));
        assert_eq!(size, 2);
        assert!(latest_backup_summary(&temp_dir("latest_empty")).is_none());
        let _ = fs::remove_dir_all(&base);
    }

    /// 导入前快照：同日多次导入必须每次都留一份，绝不覆盖。
    #[test]
    fn pre_import_snapshot_is_never_overwritten() {
        let base = temp_dir("preimport");
        let first = write_pre_import_snapshot(&base, "{\"v\":1}").unwrap();
        let second = write_pre_import_snapshot(&base, "{\"v\":2}").unwrap();
        assert_ne!(first, second, "两次快照必须是不同文件");
        assert_eq!(fs::read_to_string(&first).unwrap(), "{\"v\":1}");
        assert_eq!(fs::read_to_string(&second).unwrap(), "{\"v\":2}");
        let _ = fs::remove_dir_all(&base);
    }

    /// 快照必须是仅本人可读（Windows 继承用户目录 ACL；Unix 断言 0600）。
    #[test]
    fn pre_import_snapshot_is_written_privately() {
        let base = temp_dir("preimport_perm");
        let path = write_pre_import_snapshot(&base, "{}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "导入前快照必须仅本人可读，实际 {:o}", mode);
        }
        #[cfg(not(unix))]
        {
            assert!(path.exists());
        }
        let _ = fs::remove_dir_all(&base);
    }
}
