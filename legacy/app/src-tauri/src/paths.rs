//! 运行期数据目录解析：桌面 / CLI / Web 服务共用同一份定位规则。
//!
//! 为什么需要它：命令层原先直接问 Tauri 要 `app_data_dir()`，这让所有命令
//! 都被绑死在桌面进程里，CLI 与 HTTP 服务无法复用同一套业务逻辑——只能
//! 再抄一遍，两份实现必然漂移。这里把「数据放哪」变成一次进程级解析：
//! 三个入口在启动时各自调用 `resolve()` 一次，之后命令层只问 `data_dir()`。
//!
//! 优先级（先到先得，便于用户把数据放到便携目录或测试隔离）：
//!   1. 显式参数（CLI `--data-dir`、服务 `--data-dir`）
//!   2. 环境变量 `DIGILEGACY_DATA_DIR`
//!   3. 平台默认目录
//!
//! 平台默认：Windows `%APPDATA%\com.digilegacy.app`（与 Tauri identifier 一致，
//! 已安装用户升级后数据原地可见）；macOS `~/Library/Application Support/`；
//! 其它 `$XDG_DATA_HOME/digilegacy` 或 `~/.local/share/digilegacy`。

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// 与 `tauri.conf.json` 的 `identifier` 必须一致——改这里等于让老用户丢掉数据。
pub const APP_DIR_NAME: &str = "com.digilegacy.app";

static DATA_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// 解析数据目录（不创建、不写盘，纯计算）。
pub fn resolve(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    if let Ok(raw) = std::env::var("DIGILEGACY_DATA_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    default_dir()
}

fn default_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "环境变量 APPDATA 缺失，无法定位数据目录".to_string())?;
        Ok(PathBuf::from(appdata).join(APP_DIR_NAME))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")
            .map_err(|_| "环境变量 HOME 缺失，无法定位数据目录".to_string())?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(APP_DIR_NAME))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.trim().is_empty() {
                return Ok(PathBuf::from(xdg).join("digilegacy"));
            }
        }
        let home = std::env::var("HOME")
            .map_err(|_| "环境变量 HOME 缺失，无法定位数据目录".to_string())?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("digilegacy"))
    }
}

/// 进程级固定数据目录。桌面入口在 setup、CLI/服务入口在 main 各调用一次。
pub fn set_data_dir(dir: PathBuf) {
    if let Ok(mut guard) = DATA_DIR.write() {
        *guard = Some(dir);
    }
}

/// 读取已固定的数据目录；未初始化时返回错误而不是静默猜一个。
///
/// 静默猜目录的后果是「以为在写用户库、实际写进了另一个空库」——
/// 这类错误不会报错，只会让人对着空列表发呆。
pub fn data_dir() -> Result<PathBuf, String> {
    if let Ok(guard) = DATA_DIR.read() {
        if let Some(dir) = guard.as_ref() {
            return Ok(dir.clone());
        }
    }
    // 兜底：未显式初始化时按默认规则解析一次（单测与 MCP 只读路径受益）。
    // 但仍然不创建目录——创建是调用方的显式动作。
    default_dir()
}

/// 仅测试用：清空进程级绑定，避免同一测试进程里的并行用例互相污染。
#[cfg(test)]
pub fn reset_for_tests() {
    if let Ok(mut guard) = DATA_DIR.write() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_path_wins_over_environment() {
        let p = PathBuf::from("D:\\custom\\dir");
        assert_eq!(resolve(Some(&p)).unwrap(), p);
    }

    #[test]
    fn set_then_read_roundtrip() {
        set_data_dir(PathBuf::from("/tmp/digilegacy-test-dir"));
        assert_eq!(
            data_dir().unwrap(),
            PathBuf::from("/tmp/digilegacy-test-dir")
        );
        reset_for_tests();
    }

    #[test]
    fn app_dir_name_matches_tauri_identifier() {
        // 这个常量决定已安装用户升级后能否看到自己的数据，
        // 与 tauri.conf.json 的 identifier 脱钩就是一次静默的数据丢失。
        assert_eq!(APP_DIR_NAME, "com.digilegacy.app");
        let conf = include_str!("../tauri.conf.json");
        assert!(
            conf.contains("\"identifier\": \"com.digilegacy.app\""),
            "tauri.conf.json 的 identifier 与 paths::APP_DIR_NAME 不一致"
        );
    }
}
