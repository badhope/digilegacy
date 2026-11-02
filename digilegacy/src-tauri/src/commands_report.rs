//! 继承报告、ZIP 工具包与家人执行包 IPC 命令（从 commands.rs 拆出，纯移动，行为零变更）。

use super::win_shell;
use crate::db;
use crate::models::*;
use tauri::{command, AppHandle, Manager};

// ========== 继承报告 ==========

#[command]
pub fn get_inheritance_report() -> Result<ApiResponse<InheritanceReport>, String> {
    db::get_inheritance_report_db().map(ApiResponse::ok)
}

/// 渲染报告的核心策略（纯函数，便于脱离 Tauri 运行时回归测试）。
///
/// 顺序是刻意的：先查占位符再渲染。占位符意味着「这些字段根本没被解密」，
/// 渲染出来的报告会给继承人一份写着「需解锁」的废纸，且用户会误以为数据已损毁。
///
/// `locale` 决定文档语言：导出必须与界面语言一致，否则英文用户拿到中文文档。
pub(crate) fn render_report_response(
    report: &crate::models::InheritanceReport,
    locale: &str,
) -> Result<ApiResponse<String>, String> {
    if crate::report::contains_placeholder(report) {
        return Ok(ApiResponse::fail(
            "报告中存在未解密字段，请先解锁后重新生成（本次未生成任何文件）",
        ));
    }
    let html =
        crate::report::render_report_html(report, crate::report::ReportLocale::parse(locale));
    validate_report_html(&html)?;
    Ok(ApiResponse::ok_with_msg(html, "报告已生成"))
}

/// 由后端渲染报告 HTML（单一事实源），前端不再抓 DOM。
///
/// `locale` 由前端传入当前界面语言（`zh-CN` / `en-US`），未知值回退中文。
#[command]
pub fn render_report(
    app_handle: AppHandle,
    locale: Option<String>,
) -> Result<ApiResponse<String>, String> {
    let report = db::get_inheritance_report_db()?;
    let response = render_report_response(&report, locale.as_deref().unwrap_or("zh-CN"))?;
    // app_handle 仅为与其他导出命令保持一致的签名；这里不写盘。
    let _ = &app_handle;
    Ok(response)
}

// ========== 继承报告 / 工具包 ==========

/// 报告 HTML 的 IPC 入口上限（20MB）。
/// 单独抽成纯函数，才能不依赖 Tauri AppHandle 直接回归测试边界。
pub(crate) fn validate_report_html(content: &str) -> Result<(), String> {
    if content.len() > 20 * 1024 * 1024 {
        return Err("报告内容过大（上限 20MB）".to_string());
    }
    Ok(())
}

/// 备份 JSON 的 IPC 入口上限（8MB，与政策补丁同量级）。
pub(crate) fn validate_backup_content(content: &str) -> Result<(), String> {
    if content.len() > 8 * 1024 * 1024 {
        return Err("备份文件过大（上限 8MB）".to_string());
    }
    Ok(())
}

#[command]
pub fn export_report_to_file(
    app_handle: AppHandle,
    content: String,
) -> Result<ApiResponse<String>, String> {
    if let Err(e) = validate_report_html(&content) {
        return Ok(ApiResponse::fail(&e));
    }

    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;

    let dir = app_dir.join("报告");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let file_path = dir.join(format!(
        "归零继承报告_{}.html",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    crate::fsutil::write_private(&file_path, content.as_bytes())
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(ApiResponse::ok_with_msg(
        file_path.to_string_lossy().to_string(),
        "报告已导出到应用数据目录",
    ))
}

// ========== ZIP 工具包导出 ==========

#[command]
pub fn export_zip_to_file(
    content: String,
    app_handle: AppHandle,
) -> Result<ApiResponse<String>, String> {
    use base64::{engine::general_purpose::STANDARD as base64, Engine};
    // IPC 入口参数上限：解码后 200MB（对应 base64 约 267MB）。
    // 无上限时恶意/失控前端可凭一次 IPC 请求拖垮进程内存。
    if content.len() > 270_000_000 {
        return Ok(ApiResponse::fail("工具包体积超出上限（200MB）"));
    }
    let decoded = base64
        .decode(&content)
        .map_err(|e| format!("Base64解码失败: {}", e))?;
    if decoded.len() > 200 * 1024 * 1024 {
        return Ok(ApiResponse::fail("工具包体积超出上限（200MB）"));
    }
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;

    std::fs::create_dir_all(&app_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let zip_path = app_dir.join(format!(
        "归零继承工具包_{}.zip",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    // 工具包内含 `02-资产清单.json`（全部账号标识），必须与报告/备份同权限。
    // 旧实现用裸 std::fs::write，Unix 下按 umask 落成 0644，同机其他用户可读。
    crate::fsutil::write_private(&zip_path, &decoded).map_err(|e| format!("写入ZIP失败: {}", e))?;

    Ok(ApiResponse::ok_with_msg(
        zip_path.to_string_lossy().to_string(),
        "继承工具包已导出",
    ))
}

/// 在系统文件管理器中定位刚导出的文件。
///
/// 导出后只显示一行路径，普通用户往往找不到（尤其应用数据目录是隐藏的
/// `%APPDATA%`）。这里只接受「本应用数据目录内的已存在文件」，
/// 不接受任意路径——否则等于把 `open_external` 扩成任意路径执行器。
#[command]
pub fn reveal_exported_file(
    path: String,
    app_handle: AppHandle,
) -> Result<ApiResponse<()>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let target = std::path::PathBuf::from(&path);
    if path.chars().any(|c| c == '\0' || c.is_control()) {
        return Ok(ApiResponse::fail("路径包含非法字符"));
    }
    // 规范化后必须仍在应用数据目录内：防 `..` 穿越到任意位置。
    let canon_target = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(ApiResponse::fail("文件不存在或已被移动")),
    };
    let canon_root = app_dir
        .canonicalize()
        .map_err(|e| format!("解析应用目录失败: {}", e))?;
    if !canon_target.starts_with(&canon_root) {
        return Ok(ApiResponse::fail("只允许定位本应用导出的文件"));
    }

    #[cfg(target_os = "windows")]
    {
        let dir = canon_target
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| canon_root.to_string_lossy().to_string());
        // explorer /select,<file> 会打开父目录并选中该文件
        let params = format!("/select,\"{}\"", canon_target.display());
        let op = win_shell::wide("open");
        let file = win_shell::wide("explorer.exe");
        let params_w = win_shell::wide(&params);
        let dir_w = win_shell::wide(&dir);
        let rc = unsafe {
            win_shell::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                params_w.as_ptr(),
                dir_w.as_ptr(),
                1,
            )
        };
        if rc <= 32 {
            return Ok(ApiResponse::fail("无法打开文件管理器"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&canon_target)
            .spawn()
            .map_err(|e| format!("打开访达失败: {}", e))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = canon_target
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| canon_root.clone());
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {}", e))?;
    }

    Ok(ApiResponse::ok_with_msg((), "已定位文件"))
}

// ========== 家人执行包 / 演练 ==========

/// 生成家人执行清单（自包含 HTML，可打印，按平台分组）。
#[command]
pub fn render_action_guide(locale: Option<String>) -> Result<ApiResponse<String>, String> {
    let report = db::get_inheritance_report_db()?;
    let hints = db::get_custody_hints_db()?;
    match crate::toolkit::build_action_guide(
        &report,
        crate::report::ReportLocale::parse(locale.as_deref().unwrap_or("zh-CN")),
        &hints.backup_location,
        &hints.recovery_code_location,
    ) {
        Ok(guide) => Ok(ApiResponse::ok_with_msg(guide.html, "执行清单已生成")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 家人演练：只用元数据检查「家人拿到后会卡在哪」，不读取任何凭据。
#[command]
pub fn run_family_drill() -> Result<ApiResponse<crate::toolkit::DrillReport>, String> {
    let report = db::get_inheritance_report_db()?;
    let stats = db::get_dashboard_stats_db()?;
    Ok(ApiResponse::ok(crate::toolkit::evaluate_drill(
        &report,
        stats.trigger_ready,
        &stats.trigger_channel,
    )))
}
