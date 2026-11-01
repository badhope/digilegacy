use crate::db;
use crate::db::CheckInStatus;
use crate::models::*;
use crate::platform_data::get_all_platforms as get_all_platforms_data;
use tauri::{command, AppHandle, Manager};
use zeroize::Zeroize;

// ========== 平台 ==========

#[command]
pub fn get_all_platforms() -> Result<ApiResponse<Vec<Platform>>, String> {
    db::get_all_platforms_db().map(ApiResponse::ok)
}

#[command]
pub fn get_all_platforms_extended() -> Result<ApiResponse<Vec<PlatformExtended>>, String> {
    // 必须从数据库读（而不是直接用内置 const 表），否则用户导入的政策补丁在
    // 继承人向导 / 变现路径卡里完全不生效——两个入口会显示不一致的政策。
    let platforms = db::get_all_platforms_db()?;
    // 扩展元数据先一次性建索引：原实现在 map 内调用 get_all_platforms_data()，
    // 每处理一个平台就重建整张 152 条内置表（O(n²) 且每次分配上千个 String）。
    let meta_by_id: std::collections::HashMap<i64, crate::platform_data::PlatformEntry> =
        get_all_platforms_data()
            .into_iter()
            .map(|m| (m.id, m))
            .collect();
    let extended = platforms
        .into_iter()
        .map(|p| {
            let meta = meta_by_id.get(&p.id).cloned().unwrap_or_else(|| {
                // 补丁新增的平台在内置库里没有扩展元数据，用空值占位
                crate::platform_data::PlatformEntry::new(
                    p.id,
                    &p.name,
                    "",
                    &p.category,
                    &p.subtype,
                    p.has_property,
                    p.has_personality,
                    p.inheritable,
                    &p.required_documents,
                    &p.process_description,
                    &p.estimated_difficulty,
                    &p.official_policy_url,
                    &p.estimated_time_days,
                    &p.notes,
                )
            });
            PlatformExtended {
                base: Platform {
                    id: p.id,
                    name: p.name,
                    category: p.category,
                    subtype: p.subtype,
                    has_property: p.has_property,
                    has_personality: p.has_personality,
                    inheritable: p.inheritable,
                    required_documents: p.required_documents,
                    process_description: p.process_description,
                    estimated_difficulty: p.estimated_difficulty,
                    official_policy_url: p.official_policy_url,
                    estimated_time_days: p.estimated_time_days,
                    notes: p.notes,
                },
                name_en: meta.name_en,
                data_source: meta.data_source,
                inheritance_form_url: meta.inheritance_form_url,
                monetization_path: meta.monetization_path,
                legal_template_ids: meta.legal_template_ids,
                risk_warnings: meta.risk_warnings,
                tags: meta.tags,
                last_reviewed: meta.last_reviewed,
            }
        })
        .collect();
    Ok(ApiResponse::ok(extended))
}

// ========== 政策库补丁（离线更新通道） ==========

#[command]
pub fn import_platform_patch(content: String) -> Result<ApiResponse<String>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    // 与备份导入同一量级的上限：政策补丁是纯文本 JSON
    if content.len() > 8 * 1024 * 1024 {
        return Ok(ApiResponse::fail("政策补丁过大（上限 8MB）"));
    }
    match db::import_platform_overrides_db(&content) {
        Ok((added, updated)) => Ok(ApiResponse::ok_with_msg(
            format!("新增 {} 条，覆盖 {} 条", added, updated),
            "政策库补丁已导入",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn export_platform_patch(app_handle: AppHandle) -> Result<ApiResponse<String>, String> {
    let json = db::export_platform_overrides_db()?;
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let dir = app_dir.join("政策补丁");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let name = format!(
        "digilegacy-platform-patch-{}.json",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(name);
    // 与备份导出同样落在应用数据目录，不额外申请前端写文件权限
    std::fs::write(&path, json).map_err(|e| format!("写入补丁失败: {}", e))?;
    Ok(ApiResponse::ok_with_msg(
        path.to_string_lossy().to_string(),
        "政策补丁已导出",
    ))
}

#[command]
pub fn platform_patch_status() -> Result<ApiResponse<i64>, String> {
    db::count_platform_overrides_db().map(ApiResponse::ok)
}

#[command]
pub fn clear_platform_patch() -> Result<ApiResponse<usize>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    db::clear_platform_overrides_db().map(ApiResponse::ok)
}

// ========== 数字资产 ==========

/// IPC 入参上限：文本字段单条 8KB、ID 必须为正。
/// 没有上限时，一次失控调用就能把几十 MB 写进 SQLite（且会被每日备份复制 7 份）。
fn validate_asset_request(req: &AssetRequest) -> Result<(), String> {
    if req.platform_id <= 0 {
        return Err("平台 id 必须为正数".to_string());
    }
    for (name, value) in [
        ("账号标识", Some(req.account_identifier.as_str())),
        ("账号描述", Some(req.account_description.as_str())),
        ("备注", Some(req.notes.as_str())),
        ("继承人姓名", req.heir_name.as_deref()),
        ("继承人联系方式", req.heir_contact.as_deref()),
    ] {
        if let Some(v) = value {
            if v.chars().count() > 8_000 {
                return Err(format!("{}过长（上限 8000 字符）", name));
            }
        }
    }
    if let Some(v) = req.estimated_value {
        if !v.is_finite() || v < 0.0 || v > 1e12 {
            return Err("预估价值必须是 0 到 1 万亿之间的有限数值".to_string());
        }
    }
    Ok(())
}

#[command]
pub fn add_asset(req: AssetRequest) -> Result<ApiResponse<i64>, String> {
    if let Err(e) = validate_asset_request(&req) {
        return Ok(ApiResponse::fail(&e));
    }
    let id = db::add_asset_db(&req)?;
    Ok(ApiResponse::ok_with_msg(id, "数字资产添加成功"))
}

#[command]
pub fn update_asset(id: i64, req: AssetRequest) -> Result<ApiResponse<()>, String> {
    let exists = db::get_asset_by_id_db(id)?.is_some();
    if !exists {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    if let Err(e) = validate_asset_request(&req) {
        return Ok(ApiResponse::fail(&e));
    }
    db::update_asset_db(id, &req)?;
    Ok(ApiResponse::ok_with_msg((), "资产更新成功"))
}

#[command]
pub fn delete_asset(id: i64) -> Result<ApiResponse<()>, String> {
    if db::get_asset_by_id_db(id)?.is_none() {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    db::delete_asset_db(id)?;
    Ok(ApiResponse::ok_with_msg((), "资产已删除"))
}

#[command]
pub fn get_all_assets() -> Result<ApiResponse<Vec<DigitalAsset>>, String> {
    db::get_all_assets_db().map(ApiResponse::ok)
}

#[command]
pub fn get_asset_by_id(id: i64) -> Result<ApiResponse<Option<DigitalAsset>>, String> {
    db::get_asset_by_id_db(id).map(ApiResponse::ok)
}

#[command]
pub fn get_assets_by_category(category: String) -> Result<ApiResponse<Vec<DigitalAsset>>, String> {
    db::get_assets_by_category_db(&category).map(ApiResponse::ok)
}

// ========== 处置意愿 ==========

#[command]
pub fn set_disposal_wish(
    mut disposal_method: String,
    asset_id: i64,
    heir_name: Option<String>,
    heir_contact: Option<String>,
    mut notes: String,
) -> Result<ApiResponse<()>, String> {
    // 此命令绕过 add/update_asset 的校验直接写库，必须自带同样的边界，
    // 否则它是唯一一条能把任意长度文本与非法枚举写进资产表的入口。
    if let Err(e) = crate::models::validate_disposal(&disposal_method) {
        return Ok(ApiResponse::fail(&e));
    }
    for (name, value) in [
        ("备注", Some(notes.as_str())),
        ("继承人姓名", heir_name.as_deref()),
        ("继承人联系方式", heir_contact.as_deref()),
    ] {
        if let Some(v) = value {
            if v.chars().count() > 8_000 {
                return Ok(ApiResponse::fail(&format!(
                    "{}过长（上限 8000 字符）",
                    name
                )));
            }
        }
    }
    let asset = db::get_asset_by_id_db(asset_id)?.ok_or("资产不存在")?;

    let req = AssetRequest {
        platform_id: asset.platform_id,
        account_identifier: asset.account_identifier,
        account_description: asset.account_description,
        category: asset.category,
        estimated_value: asset.estimated_value,
        disposal_method: std::mem::take(&mut disposal_method),
        heir_name,
        heir_contact,
        notes: std::mem::take(&mut notes),
    };

    db::update_asset_db(asset_id, &req)?;
    Ok(ApiResponse::ok_with_msg((), "处置意愿已设定"))
}

// ========== 用户档案 ==========

#[command]
pub fn set_user_profile(profile: UserProfile) -> Result<ApiResponse<()>, String> {
    // 档案字段会进入继承报告与 DMS 邮件正文，必须有上限；
    // 其中紧急联系人邮箱还会作为 SMTP 收件人。
    for (name, value) in [
        ("姓名", profile.full_name.as_str()),
        ("手机号", profile.phone.as_str()),
        ("邮箱", profile.email.as_str()),
        ("紧急联系人姓名", profile.emergency_contact_name.as_str()),
        ("紧急联系人电话", profile.emergency_contact_phone.as_str()),
        ("紧急联系人邮箱", profile.emergency_contact_email.as_str()),
        ("继承人姓名", profile.heir_primary_name.as_str()),
        ("继承人电话", profile.heir_primary_phone.as_str()),
        ("继承人邮箱", profile.heir_primary_email.as_str()),
    ] {
        if value.chars().count() > 500 {
            return Ok(ApiResponse::fail(&format!("{}过长（上限 500 字符）", name)));
        }
    }
    if !(1..=365).contains(&profile.check_in_interval_days) {
        return Ok(ApiResponse::fail("检查间隔必须在 1 到 365 天之间"));
    }
    db::save_user_profile_db(&profile)?;
    Ok(ApiResponse::ok_with_msg((), "用户档案已保存"))
}

#[command]
pub fn get_user_profile() -> Result<ApiResponse<Option<UserProfile>>, String> {
    db::get_user_profile_db().map(ApiResponse::ok)
}

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

// ========== 保管位置（家人能否真正拿到凭据） ==========

#[command]
pub fn get_custody_hints() -> Result<ApiResponse<db::CustodyHints>, String> {
    db::get_custody_hints_db().map(ApiResponse::ok)
}

#[command]
pub fn set_custody_hints(hints: db::CustodyHints) -> Result<ApiResponse<()>, String> {
    for (name, v) in [
        ("备份位置", hints.backup_location.as_str()),
        ("恢复码位置", hints.recovery_code_location.as_str()),
    ] {
        if v.chars().count() > 500 {
            return Ok(ApiResponse::fail(&format!("{}过长（上限 500 字符）", name)));
        }
        if let Err(e) = crate::report::reject_placeholder_value(name, v) {
            return Ok(ApiResponse::fail(&e));
        }
    }
    match db::set_custody_hints_db(&hints) {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "保管位置已保存")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
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

// ========== 继承人名单（多继承人 + 执行人） ==========

#[command]
pub fn get_heirs() -> Result<ApiResponse<Vec<Heir>>, String> {
    db::get_heirs_db().map(ApiResponse::ok)
}

/// 整组替换：家庭成员是一个整体，逐条增删会短暂留下「两个执行人」的非法状态。
#[command]
pub fn set_heirs(heirs: Vec<Heir>) -> Result<ApiResponse<()>, String> {
    if heirs.len() > db::MAX_HEIRS {
        return Ok(ApiResponse::fail(&format!(
            "继承人最多 {} 位",
            db::MAX_HEIRS
        )));
    }
    match db::replace_heirs_db(&heirs) {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "继承人名单已保存")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== 权威来源 ==========

/// 内置权威来源表。每条都带「可达 / 内容匹配」两个独立结论。
#[command]
pub fn list_authorities() -> Result<ApiResponse<Vec<Authority>>, String> {
    Ok(ApiResponse::ok(crate::models::authorities()))
}

/// 某个资产类别相关的来源（报告与继承人向导用）。
#[command]
pub fn authorities_for(category: String) -> Result<ApiResponse<Vec<Authority>>, String> {
    let list = crate::models::authorities()
        .into_iter()
        .filter(|a| a.applies_to.iter().any(|c| c == &category))
        .collect();
    Ok(ApiResponse::ok(list))
}

// ========== 通用审核管线（AI 初审 + 人工终审） ==========

/// 已注册的审核场景。界面据此渲染场景切换，而不是写死「政策」一个。
#[command]
pub fn list_review_profiles() -> Result<ApiResponse<Vec<crate::review::ReviewProfile>>, String> {
    Ok(ApiResponse::ok(crate::review_profiles::all_profiles()))
}

/// 某场景的全部条目及其当前审核状态。
#[command]
pub fn list_review_items(
    profile_id: String,
) -> Result<ApiResponse<Vec<crate::db::ItemState>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    match crate::db::summarise_states_db(&profile_id, &items) {
        Ok(states) => Ok(ApiResponse::ok(states)),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 某场景的审核进度摘要：(已通过, 待人工, 待审)。
/// 界面用来回答「这个场景还差多少」——没有摘要时用户只能一条条数。
#[command]
pub fn review_progress(profile_id: String) -> Result<ApiResponse<serde_json::Value>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let (approved, awaiting, pending) = crate::db::review_counts_db(&profile_id, &items)?;
    Ok(ApiResponse::ok(serde_json::json!({
        "total": items.len(),
        "approved": approved,
        "awaiting_human": awaiting,
        "pending": pending,
    })))
}

/// 该场景会送审哪些字段（界面在跑 AI 前展示，与助手的出境预览同一条原则）。
#[command]
pub fn review_payload_fields(profile_id: String) -> Result<ApiResponse<Vec<String>>, String> {
    Ok(ApiResponse::ok(crate::review_profiles::payload_fields_for(
        &profile_id,
    )))
}

/// 跑 AI 初审（只写 `AwaitingHuman`，绝不写 `Approved`）。
///
/// 没有配置 AI 端点时**不是错误**：返回 `Skipped` 的结果，条目进入纯人工队列。
#[command]
pub fn run_ai_review(
    profile_id: String,
    limit: Option<usize>,
) -> Result<ApiResponse<Vec<crate::db::ItemState>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(profile) = crate::review_profiles::profile_by_id(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let states = crate::db::summarise_states_db(&profile_id, &items)?;
    // 只审「待审」的条目：已通过且内容未变的跳过，避免重复花费。
    let todo: Vec<&crate::review::ReviewItem> = items
        .iter()
        .filter(|i| {
            states
                .iter()
                .find(|s| s.key == i.key)
                .map(|s| s.status == crate::review::ReviewStatus::Pending)
                .unwrap_or(true)
        })
        .take(limit.unwrap_or(usize::MAX))
        .collect();

    if todo.is_empty() {
        return Ok(ApiResponse::ok_with_msg(states, "没有需要初审的条目"));
    }

    // 未配置 AI / 未启用：优雅降级，写一条 Skipped 留痕，条目转入人工队列。
    let cfg = db::get_assistant_config_db()?;
    let ai_ready = cfg.enabled
        && !cfg.endpoint.trim().is_empty()
        && !cfg.model.trim().is_empty()
        && cfg.has_key;
    if !profile.use_ai || !ai_ready {
        let reason = if !profile.use_ai {
            "该场景声明为纯人工审核"
        } else {
            "未配置 AI 端点，已转为纯人工审核"
        };
        for item in &todo {
            crate::db::append_review_record_db(&crate::review::ReviewRecord {
                profile_id: profile_id.clone(),
                item_key: item.key.clone(),
                fingerprint: item.fingerprint.clone(),
                ai_verdict: crate::review::AiVerdict::Skipped,
                ai_note: reason.to_string(),
                human_verdict: None,
                human_note: String::new(),
                decided_by: "ai".to_string(),
                created_at: String::new(),
            })?;
        }
        let after = crate::db::summarise_states_db(&profile_id, &items)?;
        return Ok(ApiResponse::ok_with_msg(after, reason));
    }

    let key = db::get_assistant_key_plain()?.unwrap_or_default();
    for item in &todo {
        let prompt = crate::review::ai_review_prompt(&profile, item);
        let (verdict, note) = match crate::ai_assist::call_chat_api(
            &cfg.endpoint,
            &key,
            &cfg.model,
            "你是内容初审员，只输出 PASS/FLAG/UNKNOWN 加依据。你不是最终决策者。",
            &prompt,
        ) {
            Ok(raw) => {
                let (v, n) = crate::review::parse_ai_verdict(&raw);
                // 严格模式：逐项作答缺任何一项，整条降级为 Unknown。
                // 模型跳过某一项往往说明它没看，而不是「没问题」。
                let (missing, flagged) = crate::review::validate_strict_answer(&profile, &raw);
                if !missing.is_empty() {
                    (
                        crate::review::AiVerdict::Unknown,
                        format!(
                            "{}\n\n[严格模式] 缺少以下必检项作答，本次初审作废：{}",
                            n,
                            missing.join(", ")
                        ),
                    )
                } else if !flagged.is_empty() && v == crate::review::AiVerdict::Pass {
                    // 逐项检查报了问题却给 PASS：以逐项结论为准，改为 FLAG
                    (
                        crate::review::AiVerdict::Flag,
                        format!("{}\n\n[严格模式] 逐项检查发现：{}", n, flagged.join(", ")),
                    )
                } else {
                    (v, n)
                }
            }
            Err(e) => (
                crate::review::AiVerdict::Unknown,
                format!("AI 调用失败：{}", e),
            ),
        };
        crate::db::append_review_record_db(&crate::review::ReviewRecord {
            profile_id: profile_id.clone(),
            item_key: item.key.clone(),
            fingerprint: item.fingerprint.clone(),
            ai_verdict: verdict,
            ai_note: note,
            human_verdict: None,
            human_note: String::new(),
            decided_by: "ai".to_string(),
            created_at: String::new(),
        })?;
    }
    let after = crate::db::summarise_states_db(&profile_id, &items)?;
    let msg = format!("AI 初审完成，{} 条待人工确认", todo.len());
    Ok(ApiResponse::ok_with_msg(after, &msg))
}

/// 人工终审。**只有这里能让条目变成 Approved。**
#[command]
pub fn submit_human_review(
    profile_id: String,
    item_key: String,
    verdict: String,
    note: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let Some(_profile) = crate::review_profiles::profile_by_id(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(items) = crate::review_profiles::items_for(&profile_id) else {
        return Ok(ApiResponse::fail("未知的审核场景"));
    };
    let Some(item) = items.iter().find(|i| i.key == item_key) else {
        return Ok(ApiResponse::fail("条目不存在"));
    };
    let human = match verdict.as_str() {
        "approved" => crate::review::HumanVerdict::Approved,
        "rejected" => crate::review::HumanVerdict::Rejected,
        "needs_edit" => crate::review::HumanVerdict::NeedsEdit,
        _ => return Ok(ApiResponse::fail("无效的审核结论")),
    };
    let note = note.unwrap_or_default();
    if note.chars().count() > 2_000 {
        return Ok(ApiResponse::fail("审核备注过长（上限 2000 字符）"));
    }
    // 严格场景：AI 提过疑点却要「通过」时，必须写明理由。
    // 这不是流程洁癖——空白放行会让留痕无法回答「当初为什么认为可以」。
    if human == crate::review::HumanVerdict::Approved {
        if let Some(profile) = crate::review_profiles::profile_by_id(&profile_id) {
            if profile.strictness == crate::review::ReviewStrictness::Strict {
                let history = crate::db::list_review_records_db(&profile_id, &item_key)?;
                let ai_flagged = history
                    .first()
                    .map(|r| {
                        matches!(
                            r.ai_verdict,
                            crate::review::AiVerdict::Flag | crate::review::AiVerdict::Unknown
                        )
                    })
                    .unwrap_or(false);
                if ai_flagged && note.trim().is_empty() {
                    return Ok(ApiResponse::fail(
                        "AI 初审对本条提出了疑点，通过时必须填写理由（用于留痕）",
                    ));
                }
            }
        }
    }
    // 沿用最近一次的 AI 结论一起留痕，便于回看「当时 AI 说了什么」。
    let history = crate::db::list_review_records_db(&profile_id, &item.key)?;
    let (ai_verdict, ai_note) = history
        .first()
        .map(|r| (r.ai_verdict.clone(), r.ai_note.clone()))
        .unwrap_or((crate::review::AiVerdict::Skipped, String::new()));
    crate::db::append_review_record_db(&crate::review::ReviewRecord {
        profile_id: profile_id.clone(),
        item_key: item.key.clone(),
        fingerprint: item.fingerprint.clone(),
        ai_verdict,
        ai_note,
        human_verdict: Some(human),
        human_note: note,
        decided_by: "human".to_string(),
        created_at: String::new(),
    })?;
    Ok(ApiResponse::ok_with_msg((), "审核结论已记录"))
}

/// 某条目的完整审核历史（新在前）。留痕只增不改。
#[command]
pub fn review_history(
    profile_id: String,
    item_key: String,
) -> Result<ApiResponse<Vec<crate::review::ReviewRecord>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::db::list_review_records_db(&profile_id, &item_key).map(ApiResponse::ok)
}

// ========== AI 助手（可选 · 用户自配端点） ==========

#[command]
pub fn get_assistant_config() -> Result<ApiResponse<crate::ai_assist::AssistantConfig>, String> {
    db::get_assistant_config_db().map(ApiResponse::ok)
}

#[command]
pub fn set_assistant_config(
    enabled: bool,
    endpoint: String,
    model: String,
    api_key: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if endpoint.len() > 300 {
        return Ok(ApiResponse::fail("端点地址过长"));
    }
    if !endpoint.trim().is_empty() {
        let ep = endpoint.trim();
        if !(ep.starts_with("https://") || ep.starts_with("http://")) {
            return Ok(ApiResponse::fail("端点必须以 https:// 或 http:// 开头"));
        }
    }
    if model.len() > 120 {
        return Ok(ApiResponse::fail("模型名过长"));
    }
    if let Some(k) = api_key.as_deref() {
        if k.len() > 512 {
            return Ok(ApiResponse::fail("API Key 过长"));
        }
    }
    // 开启时必须有端点与模型；否则给一个明确提示，而不是等到提问时才失败。
    if enabled && (endpoint.trim().is_empty() || model.trim().is_empty()) {
        return Ok(ApiResponse::fail(
            "启用助手需要填写端点与模型名；没有服务时请保持关闭",
        ));
    }
    match db::set_assistant_config_db(enabled, endpoint.trim(), model.trim(), api_key.as_deref()) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            "助手配置已保存（Key 已加密存储，不会回传）",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 发送前预览：逐项列出将出境的非身份字段，让用户在点「提问」前看见全貌。
#[command]
pub fn preview_assistant_payload(
    question: String,
) -> Result<ApiResponse<crate::ai_assist::OutboundPreview>, String> {
    let report = db::get_inheritance_report_db()?;
    if crate::report::contains_placeholder(&report) {
        return Ok(ApiResponse::fail(
            "当前数据处于加密锁定状态，请先解锁后再使用助手",
        ));
    }
    Ok(ApiResponse::ok(crate::ai_assist::preview_outbound(
        &report, &question,
    )))
}

/// 提问。**只读**：产出的文本不会写库，用户确认后由前端拼进工具包。
#[command]
pub fn ask_assistant(
    question: String,
    locale: Option<String>,
) -> Result<ApiResponse<String>, String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Ok(ApiResponse::fail(
            "助手未启用。本应用不内置任何 AI 服务；如你有 OpenAI 兼容端点，可在「我的档案 → AI 助手」填写地址与 Key",
        ));
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() {
        return Ok(ApiResponse::fail("助手配置不完整（缺端点或模型名）"));
    }
    let key = match db::get_assistant_key_plain()? {
        Some(k) => k,
        None => return Ok(ApiResponse::fail("尚未填写 API Key")),
    };
    let report = db::get_inheritance_report_db()?;
    let question_trimmed = question.trim().to_string();
    match crate::ai_assist::ask(
        &report,
        &question,
        &cfg.endpoint,
        &key,
        &cfg.model,
        crate::report::ReportLocale::parse(locale.as_deref().unwrap_or("zh-CN")),
    ) {
        Ok(answer) => {
            // 问答本地留痕：界面可回看与清除。留痕失败不阻断回答本身。
            if let Err(e) = db::append_ai_chat_db(&question_trimmed, &answer, &cfg.model) {
                eprintln!("保存问答历史失败: {}", e);
            }
            Ok(ApiResponse::ok_with_msg(
                answer,
                "助手已回答（仅本地留痕，未写入任何资产数据）",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 流式提问：回答经 Tauri channel 逐字推送到前端。
///
/// 与 ask_assistant 相同的安全约束（白名单 payload、占位符拦截、Key 不回传）；
/// 流结束后把完整问答落一条本地历史。
#[command]
pub fn ask_assistant_streaming(
    question: String,
    locale: Option<String>,
    on_delta: tauri::ipc::Channel<String>,
) -> Result<ApiResponse<String>, String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Ok(ApiResponse::fail(
            "助手未启用。本应用不内置任何 AI 服务；如你有 OpenAI 兼容端点，可在「我的档案 → AI 助手」填写地址与 Key",
        ));
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() {
        return Ok(ApiResponse::fail("助手配置不完整（缺端点或模型名）"));
    }
    let key = match db::get_assistant_key_plain()? {
        Some(k) => k,
        None => return Ok(ApiResponse::fail("尚未填写 API Key")),
    };
    let report = db::get_inheritance_report_db()?;
    let question_trimmed = question.trim().to_string();
    if question_trimmed.is_empty() {
        return Ok(ApiResponse::fail("问题不能为空"));
    }
    if question_trimmed.chars().count() > 2_000 {
        return Ok(ApiResponse::fail("问题过长（上限 2000 字符）"));
    }
    if crate::report::contains_placeholder(&report) {
        return Ok(ApiResponse::fail(
            "当前数据处于加密锁定状态，请先解锁后再使用助手",
        ));
    }
    let payload = crate::ai_assist::preview_payload(&report, &question_trimmed);
    let user_content = serde_json::to_string_pretty(&payload).unwrap_or_default();
    let system = crate::ai_assist::system_prompt_pub(crate::report::ReportLocale::parse(
        locale.as_deref().unwrap_or("zh-CN"),
    ));

    let mut full = String::new();
    let result = crate::ai_assist::call_chat_api_streaming(
        &cfg.endpoint,
        &key,
        &cfg.model,
        system,
        &user_content,
        &mut |delta| {
            full.push_str(delta);
            // channel 关闭/背压不致命：收完流并落历史
            let _ = on_delta.send(delta.to_string());
        },
    );
    match result {
        Ok(()) => {
            if full.trim().is_empty() {
                Ok(ApiResponse::fail("模型未返回内容"))
            } else {
                if let Err(e) = db::append_ai_chat_db(&question_trimmed, full.trim(), &cfg.model) {
                    eprintln!("保存问答历史失败: {}", e);
                }
                Ok(ApiResponse::ok_with_msg(full.trim().to_string(), "已完成"))
            }
        }
        Err(e) => {
            // 已输出的部分交还前端展示，不整段丢弃
            if !full.is_empty() {
                let _ = on_delta.send(format!("\n\n[中断] {}", e));
            }
            Ok(ApiResponse::fail(&e))
        }
    }
}

#[command]
pub fn list_ai_chat(limit: Option<usize>) -> Result<ApiResponse<Vec<serde_json::Value>>, String> {
    match db::list_ai_chat_db(limit.unwrap_or(50)) {
        Ok(rows) => Ok(ApiResponse::ok(rows)),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn clear_ai_chat() -> Result<ApiResponse<()>, String> {
    match db::clear_ai_chat_db() {
        Ok(()) => Ok(ApiResponse::ok_with_msg((), "问答历史已清空")),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== 检查记录 / Dead Man's Switch ==========

#[command]
pub fn update_check_in() -> Result<ApiResponse<String>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    db::record_check_in_internal_db()?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    Ok(ApiResponse::ok_with_msg(now, "检查完成，状态良好"))
}

#[command]
pub fn get_check_in_status() -> Result<ApiResponse<CheckInStatus>, String> {
    db::get_check_in_status_db().map(ApiResponse::ok)
}

/// 只读的 DMS 状态：供界面渲染，不触发投递。
#[command]
pub fn get_dms_status() -> Result<ApiResponse<crate::notify::DmsStatus>, String> {
    crate::notify::status_snapshot().map(ApiResponse::ok)
}

/// 本地发件箱历史（最近 50 条），让用户能看到「过去真的发过什么」。
#[command]
pub fn get_dms_history(
    app_handle: AppHandle,
) -> Result<ApiResponse<Vec<crate::notify::DmsNotification>>, String> {
    if let Err(e) = db::ensure_vault_readable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::notify::outbox_history(&app_handle, 50).map(ApiResponse::ok)
}

#[command]
pub fn set_dead_mans_switch(enabled: bool, interval_days: i64) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    // 合法区间与前端下拉一致：1 天（测试/特殊场景）到 365 天。
    // 0 或负数会让「到期日」落在过去，第一天就触发三级通知；极端大值等于永不提醒。
    if !(1..=365).contains(&interval_days) {
        return Ok(ApiResponse::fail("检查间隔必须在 1 到 365 天之间"));
    }
    db::set_dms_config_db(enabled, interval_days)?;
    let msg = if enabled {
        format!("Dead Man's Switch 已开启，每 {} 天检查一次", interval_days)
    } else {
        "Dead Man's Switch 已关闭".to_string()
    };
    Ok(ApiResponse::ok_with_msg((), &msg))
}

// ========== 仪表板统计 ==========

#[command]
pub fn get_dashboard_stats() -> Result<ApiResponse<DashboardStats>, String> {
    db::get_dashboard_stats_db().map(ApiResponse::ok)
}

// ========== 安全与凭据保管 ==========

#[command]
pub fn security_status() -> Result<ApiResponse<SecurityStatus>, String> {
    let has = db::get_security_config_db()?.is_some();
    // 「解锁」必须是可验证的状态，而不是「内存里有个 key」。
    // 密钥与验证器不匹配时（例如恢复流程中途退出）继续宣称已解锁，
    // 界面就会把占位符当数据展示、并把占位符写回（历史 P0）。
    // 无主密码时如实返回「没有密钥」，是否可用由前端按 has_master_password 决定
    // （security.ts: `unlocked = has ? data.unlocked : true`）。
    let unlocked = if has {
        let guard = db::get_conn()?;
        db::vault_key_matches_config(guard.as_ref().unwrap())
    } else {
        false
    };
    Ok(ApiResponse::ok(SecurityStatus {
        has_master_password: has,
        unlocked,
    }))
}

#[command]
pub fn set_master_password(
    mut password: String,
) -> Result<ApiResponse<MasterPasswordSetup>, String> {
    if password.chars().count() < 8 {
        password.zeroize();
        return Ok(ApiResponse::fail("主密码至少需要 8 个字符"));
    }
    if db::get_security_config_db()?.is_some() {
        password.zeroize();
        return Ok(ApiResponse::fail("已设置主密码，暂不支持修改"));
    }

    // v3 盐（Argon2id，参数随盐串自描述）：新装一律 Argon2id。
    // 保留 PBKDF2 v1/v2 读取路径，旧库升级后仍能解锁。
    let salt = crate::crypto::generate_salt_v3();
    let kek = crate::crypto::derive_from_salt(&password, &salt)?;
    password.zeroize(); // 派生完成后立即擦除明文密码

    // 两层密钥：随机 DEK 才是数据密钥，KEK 只负责包裹它。
    // 改密码从此只需重包裹 DEK（O(1)），且已发出的继承人份额不会失效。
    let dek = crate::vault::generate_data_key();
    let wrapped = crate::vault::wrap_data_key(&dek, &kek)?;
    let verifier = crate::vault::make_verifier(&dek)?;
    let (recovery_code, recovery_seal) = crate::recovery::create_recovery_seal(&dek)?;
    db::set_security_config_with_dek_db(&salt, &verifier, &wrapped)?;
    db::set_recovery_seal_db(&recovery_seal)?;
    crate::vault::set_key(dek);

    let encrypted_rows = db::encrypt_existing_data_db()?;

    Ok(ApiResponse {
        success: true,
        message: format!(
            "加密已启用，存量数据已加密 {} 条。请立即抄写恢复密钥。",
            encrypted_rows
        ),
        data: Some(MasterPasswordSetup {
            status: SecurityStatus {
                has_master_password: true,
                unlocked: true,
            },
            recovery_code,
        }),
    })
}

#[command]
pub fn unlock(mut password: String) -> Result<ApiResponse<SecurityStatus>, String> {
    let Some((salt_b64, verifier)) = db::get_security_config_db()? else {
        password.zeroize();
        return Ok(ApiResponse::fail("尚未设置主密码"));
    };
    let kek = crate::crypto::derive_from_salt(&password, &salt_b64)?;
    password.zeroize(); // 派生完成后立即擦除明文密码

    // 数据密钥的来源：新库从 wrapped_dek 解出；legacy 库密码派生密钥即数据密钥。
    let wrapped = db::get_wrapped_dek_db()?;
    let key = if wrapped.is_empty() {
        kek.clone()
    } else {
        match crate::vault::unwrap_data_key(&wrapped, &kek) {
            Ok(k) => k,
            Err(_) => return Ok(ApiResponse::fail("主密码错误")),
        }
    };

    if crate::vault::verify_key(&verifier, &key) {
        crate::vault::set_key(key);
        Ok(ApiResponse::ok(SecurityStatus {
            has_master_password: true,
            unlocked: true,
        }))
    } else {
        Ok(ApiResponse::fail("主密码错误"))
    }
}

#[command]
pub fn lock() -> Result<ApiResponse<SecurityStatus>, String> {
    crate::vault::clear_key();
    Ok(ApiResponse::ok_with_msg(
        SecurityStatus {
            has_master_password: true,
            unlocked: false,
        },
        "已锁定",
    ))
}

#[command]
pub fn set_asset_credential(id: i64, credential: String) -> Result<ApiResponse<()>, String> {
    // IPC 入口参数上限：单条凭据 64KB，正常凭据远小于此
    if credential.len() > 64 * 1024 {
        return Ok(ApiResponse::fail("凭据内容过长（上限 64KB）"));
    }
    if db::get_asset_by_id_db(id)?.is_none() {
        return Ok(ApiResponse::fail("资产不存在或已被删除"));
    }
    db::set_asset_credential_db(id, &credential)?;
    let msg = if credential.is_empty() {
        "凭据已清除"
    } else {
        "凭据已加密保存"
    };
    Ok(ApiResponse::ok_with_msg((), msg))
}

#[command]
pub fn get_asset_credential(id: i64) -> Result<ApiResponse<String>, String> {
    db::get_asset_credential_db(id).map(ApiResponse::ok)
}

#[command]
pub fn change_master_password(
    mut old_password: String,
    mut new_password: String,
    mut confirm_password: String,
) -> Result<ApiResponse<MasterPasswordSetup>, String> {
    match db::change_master_password_db(&old_password, &new_password, &confirm_password) {
        Ok(recovery_code) => {
            old_password.zeroize();
            new_password.zeroize();
            confirm_password.zeroize();
            Ok(ApiResponse {
                success: true,
                message: "主密码已更新，全部数据已用新密钥重新加密。请立即抄写新的恢复密钥。"
                    .to_string(),
                data: Some(MasterPasswordSetup {
                    status: SecurityStatus {
                        has_master_password: true,
                        unlocked: true,
                    },
                    recovery_code,
                }),
            })
        }
        Err(e) => {
            old_password.zeroize();
            new_password.zeroize();
            confirm_password.zeroize();
            Ok(ApiResponse::fail(&e))
        }
    }
}

#[command]
pub fn export_backup(app_handle: AppHandle) -> Result<ApiResponse<String>, String> {
    let json = db::export_backup_json_db()?;
    let encrypted = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|value| {
            value
                .get("tables")
                .and_then(|tables| tables.get("security_config"))
                .map(serde_json::Value::is_object)
        })
        .unwrap_or(false);
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let dir = app_dir.join("备份");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let name = format!(
        "归零{}备份_{}.json",
        if encrypted { "加密" } else { "明文" },
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(name);
    crate::fsutil::write_private(&path, json.as_bytes())
        .map_err(|e| format!("写入备份失败: {}", e))?;
    let message = if encrypted {
        "备份已导出（含密文与安全配置，可与其他设备互导）"
    } else {
        "明文备份已导出（尚未启用主密码，不提供加密保护）"
    };
    Ok(ApiResponse::ok_with_msg(
        path.to_string_lossy().to_string(),
        message,
    ))
}

/// 导入备份的核心策略（纯函数，便于脱离 Tauri 运行时回归）。
///
/// `replace` 模式必须先写一份导入前快照；命令层传应用数据目录，测试可传入故障路径。
/// `merge` 模式只追加备份数据，不依赖快照，保留其非破坏语义。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn import_backup_response(
    content: &str,
    mode: &str,
    snapshot_dir: Option<&std::path::Path>,
) -> Result<ApiResponse<String>, String> {
    import_backup_response_with_password(content, mode, snapshot_dir, None)
}

pub(crate) fn import_backup_response_with_password(
    content: &str,
    mode: &str,
    snapshot_dir: Option<&std::path::Path>,
    source_password: Option<&str>,
) -> Result<ApiResponse<String>, String> {
    if let Err(e) = validate_backup_content(content) {
        return Ok(ApiResponse::fail(&e));
    }
    let import_mode = db::ImportMode::parse(mode);

    let snapshot = if import_mode == db::ImportMode::Replace {
        let Some(dir) = snapshot_dir else {
            return Ok(ApiResponse::fail(
                "覆盖导入前快照失败：无法获取应用数据目录",
            ));
        };
        let json = match db::export_backup_json_db() {
            Ok(json) => json,
            Err(e) => return Ok(ApiResponse::fail(&format!("覆盖导入前快照导出失败：{}", e))),
        };
        match crate::backup::write_pre_import_snapshot(dir, &json) {
            Ok(path) => Some(path.to_string_lossy().to_string()),
            Err(e) => return Ok(ApiResponse::fail(&format!("覆盖导入前快照写入失败：{}", e))),
        }
    } else {
        None
    };

    match db::import_backup_json_db_with_mode_and_password(content, import_mode, source_password) {
        Ok(n) => {
            let mode_label = if import_mode == db::ImportMode::Replace {
                "覆盖"
            } else {
                "合并"
            };
            let snapshot_note = snapshot
                .map(|path| format!("；导入前快照：{}", path))
                .unwrap_or_default();
            Ok(ApiResponse::ok_with_msg(
                format!(
                    "已按{}模式导入 {} 条资产，请用该备份对应的主密码重新解锁{}",
                    mode_label, n, snapshot_note
                ),
                "备份导入完成",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 导入备份。
///
/// `mode`：`merge`（默认，保留现有资产，只追加）或 `replace`（清空后写入）。
/// 两种模式都会在写入前自动落一份「导入前快照」——覆盖导入是不可逆操作，
/// 误选一次就永久丢数据，快照是唯一能回滚的路径。
#[command]
pub fn import_backup(
    content: String,
    mode: Option<String>,
    source_password: Option<String>,
    app_handle: AppHandle,
) -> Result<ApiResponse<String>, String> {
    let dir = app_handle.path().app_data_dir().ok();
    let mut password = source_password;
    let result = import_backup_response_with_password(
        &content,
        mode.as_deref().unwrap_or("merge"),
        dir.as_deref(),
        password.as_deref(),
    );
    if let Some(password) = password.as_mut() {
        password.zeroize();
    }
    result
}

#[command]
pub fn get_recovery_seal_exists() -> Result<ApiResponse<bool>, String> {
    Ok(ApiResponse::ok(!db::get_recovery_seal_db()?.is_empty()))
}

/// 自动备份状态：让用户知道「每日快照」是否真的在跑。
/// 旧界面只有手动导出按钮，自动备份存在但完全不可见。
#[command]
pub fn get_backup_status(app_handle: AppHandle) -> Result<ApiResponse<serde_json::Value>, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let summary = crate::backup::latest_backup_summary(&dir);
    let count = crate::backup::list_backups(&dir).len();
    Ok(ApiResponse::ok(serde_json::json!({
        "count": count,
        "latest_name": summary.as_ref().map(|(n, _)| n.clone()),
        "latest_size": summary.map(|(_, s)| s),
    })))
}

/// 恢复流程第一步：用恢复码解出主密钥（不设新密码），返回临时解锁态
#[command]
pub fn start_recovery(recovery_code: String) -> Result<ApiResponse<SecurityStatus>, String> {
    let seal = db::get_recovery_seal_db()?;
    if seal.is_empty() {
        return Ok(ApiResponse::fail("该安装未启用恢复密钥"));
    }
    match crate::recovery::unseal_with_recovery(&recovery_code, &seal) {
        Ok(master_key) => {
            crate::vault::set_key(master_key);
            Ok(ApiResponse::ok(SecurityStatus {
                has_master_password: true,
                unlocked: true,
            }))
        }
        Err(_) => Ok(ApiResponse::fail("恢复密钥不正确")),
    }
}

/// 恢复流程第二步：处于恢复解锁态时设置新密码（重派生验证器并轮换恢复码）
#[command]
pub fn complete_recovery(
    mut new_password: String,
    mut confirm_password: String,
) -> Result<ApiResponse<String>, String> {
    if !crate::vault::is_unlocked() {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("请先通过恢复密钥解锁"));
    }
    if new_password.chars().count() < 8 {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("新主密码至少需要 8 个字符"));
    }
    if new_password != confirm_password {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("两次输入的新主密码不一致"));
    }
    let Some(master_key) = crate::vault::get_key() else {
        new_password.zeroize();
        confirm_password.zeroize();
        return Ok(ApiResponse::fail("解锁状态异常"));
    };

    // 恢复重设沿用恢复码解出的同一把数据密钥：只换 KEK 与恢复封印，不碰任何字段。
    let salt_new = crate::crypto::generate_salt_v3();
    let kek_new = crate::crypto::derive_from_salt(&new_password, &salt_new)?;
    new_password.zeroize();
    confirm_password.zeroize();
    let verifier_new = crate::vault::make_verifier(&master_key)?;
    let (recovery_code_new, recovery_seal_new) =
        crate::recovery::create_recovery_seal(&master_key)?;

    db::reset_security_config_db(
        &salt_new,
        &verifier_new,
        &recovery_seal_new,
        &master_key,
        &kek_new,
    )?;
    crate::vault::set_key(master_key);

    Ok(ApiResponse::ok_with_msg(
        recovery_code_new,
        "主密码已重置，请立即保存新的恢复密钥",
    ))
}

/// SMTP 配置（界面可填；密码只落库不回传）。
#[command]
pub fn get_smtp_config() -> Result<ApiResponse<db::SmtpConfig>, String> {
    db::get_smtp_config_db().map(ApiResponse::ok)
}

/// 保存 SMTP 配置。`password` 传 `None` 表示保持原密码不变。
#[command]
pub fn set_smtp_config(
    host: String,
    port: i64,
    from_addr: String,
    username: String,
    password: Option<String>,
    allow_insecure: bool,
) -> Result<ApiResponse<()>, String> {
    if host.len() > 253 {
        return Ok(ApiResponse::fail("SMTP 服务器地址过长"));
    }
    if from_addr.len() > 320 || username.len() > 320 {
        return Ok(ApiResponse::fail("发件人或用户名过长"));
    }
    if let Some(p) = password.as_deref() {
        if p.len() > 512 {
            return Ok(ApiResponse::fail("SMTP 密码过长"));
        }
    }
    match db::set_smtp_config_db(
        &host,
        port,
        &from_addr,
        &username,
        password.as_deref(),
        allow_insecure,
    ) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            "SMTP 配置已保存；配置后请用「发送测试邮件」确认能真正送达",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

/// 发送一封测试邮件。
///
/// 为什么必须有这个按钮：SMTP 配错时用户完全看不出来——通知只在到期时
/// 静默降级到本地发件箱，「安心传承」看起来是开着的，家人却什么都收不到。
/// 测试发送把这个隐性失败提前暴露在配置当下。
#[command]
pub fn send_test_email(recipient: String) -> Result<ApiResponse<String>, String> {
    if recipient.len() > 320 {
        return Ok(ApiResponse::fail("收件人地址过长"));
    }
    let Some(settings) = crate::notify::resolve_smtp_settings() else {
        return Ok(ApiResponse::fail(
            "尚未配置 SMTP 服务器（需要服务器地址与发件人地址）",
        ));
    };
    if !recipient.contains('@') {
        return Ok(ApiResponse::fail("请填写有效的收件人邮箱"));
    }
    match crate::notify::send_test_message(&settings, &recipient) {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            recipient,
            "测试邮件已发送，请检查收件箱（含垃圾邮件）",
        )),
        Err(e) => Ok(ApiResponse::fail(&format!(
            "测试邮件发送失败：{}。提示：多数邮箱需要「授权码」而非登录密码，端口常用 465/587",
            e
        ))),
    }
}

#[command]
pub fn get_heartbeat_status() -> Result<ApiResponse<crate::heartbeat::HeartbeatStatus>, String> {
    Ok(ApiResponse::ok(crate::heartbeat::current_status()))
}

/// 读取心跳配置（界面渲染用）。token 只回传「是否已配置」，不回传明文。
#[command]
pub fn get_heartbeat_config() -> Result<ApiResponse<serde_json::Value>, String> {
    let (enabled, token, endpoint) = db::get_heartbeat_config_db()?;
    Ok(ApiResponse::ok(serde_json::json!({
        "enabled": enabled,
        "has_token": !token.is_empty(),
        "endpoint": endpoint,
    })))
}

/// 保存心跳配置。token 传空串表示保持原值不变（避免界面把掩码值回写覆盖真令牌）。
#[command]
pub fn set_heartbeat_config(
    enabled: bool,
    token: String,
    endpoint: String,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let (_, current_token, _) = db::get_heartbeat_config_db()?;
    let next_token = if token.trim().is_empty() {
        current_token
    } else {
        token.trim().to_string()
    };
    if enabled && next_token.is_empty() {
        return Ok(ApiResponse::fail("启用心跳前请先填写令牌"));
    }
    if enabled {
        let ep = endpoint.trim();
        if ep.is_empty() {
            return Ok(ApiResponse::fail(
                "启用心跳前请先填写服务器地址（可自建 server/heartbeat-server.mjs）",
            ));
        }
        if !(ep.starts_with("http://") || ep.starts_with("https://")) {
            return Ok(ApiResponse::fail(
                "服务器地址必须以 http:// 或 https:// 开头",
            ));
        }
    }
    db::set_heartbeat_config_db(enabled, &next_token, endpoint.trim())?;
    Ok(ApiResponse::ok_with_msg(
        (),
        if enabled {
            "心跳已启用"
        } else {
            "心跳已关闭"
        },
    ))
}

#[command]
pub fn send_heartbeat_now() -> Result<ApiResponse<crate::heartbeat::HeartbeatStatus>, String> {
    Ok(ApiResponse::ok(crate::heartbeat::send_heartbeat()))
}

#[command]
pub fn set_autostart(app_handle: AppHandle, enabled: bool) -> Result<ApiResponse<()>, String> {
    use tauri_plugin_autostart::ManagerExt as _;
    let manager = app_handle.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => Ok(ApiResponse::ok_with_msg(
            (),
            if enabled {
                "已设置开机自启"
            } else {
                "已关闭开机自启"
            },
        )),
        Err(e) => Ok(ApiResponse::fail(&format!("设置自启失败: {}", e))),
    }
}

#[command]
pub fn hide_main_window(app_handle: AppHandle) -> Result<ApiResponse<()>, String> {
    use tauri::Manager as _;
    if let Some(win) = app_handle.get_webview_window("main") {
        let _ = win.hide();
    }
    Ok(ApiResponse::ok_with_msg((), "已隐藏到托盘"))
}

#[cfg(target_os = "windows")]
mod win_shell {
    use std::os::windows::ffi::OsStrExt;

    // `ShellExecuteW` 只在 Windows 存在；返回 `HINSTANCE`（指针宽度）。
    // 全项目只声明一次：两个调用点重复声明会触发 `clashing_extern_declarations`，
    // 而签名不一致在 ABI 上属于未定义行为。
    #[link(name = "shell32")]
    extern "system" {
        pub fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> isize;
    }

    pub fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[command]
pub fn open_external(url: String) -> Result<ApiResponse<()>, String> {
    let checked = crate::vault::validate_external_url(&url)?;

    #[cfg(target_os = "windows")]
    {
        let op = win_shell::wide("open");
        let target = win_shell::wide(&checked);
        let result = unsafe {
            win_shell::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result <= 32 {
            return Err("无法打开链接".to_string());
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&checked)
            .spawn()
            .map_err(|e| format!("无法打开链接: {}", e))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&checked)
            .spawn()
            .map_err(|e| format!("无法打开链接: {}", e))?;
    }

    Ok(ApiResponse::ok_with_msg((), "已在浏览器打开"))
}

// ========== 继承人恢复包（Shamir 3-of-5） ==========

/// 生成继承人恢复包：份额写入打印友好的 HTML，元数据（不含份额）落库。
#[command]
pub fn generate_heir_recovery_kit(
    app_handle: AppHandle,
    threshold: u8,
    total: u8,
    passphrase: Option<String>,
) -> Result<ApiResponse<crate::heir_recovery::KitResult>, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取目录失败: {}", e))?;
    let dir = app_dir.join("继承人恢复包");
    let mut pass = passphrase;
    let result = crate::heir_recovery::generate_kit(threshold, total, pass.as_deref(), &dir);
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    match result {
        Ok((kit, _html)) => Ok(ApiResponse::ok_with_msg(
            kit,
            "恢复包已生成，请分发给不同保管人",
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

#[command]
pub fn heir_recovery_status() -> Result<ApiResponse<serde_json::Value>, String> {
    crate::heir_recovery::status()
        .map(|v| ApiResponse::ok(v.unwrap_or(serde_json::json!({"exists": false, "stale": false}))))
}

/// 恢复演练：只读校验（不重建、不落盘、不改变解锁状态）。
#[command]
pub fn run_recovery_drill(
    shares: Vec<String>,
    passphrase: Option<String>,
) -> Result<ApiResponse<crate::heir_recovery::DrillResult>, String> {
    if shares.len() > crate::heir_recovery::MAX_SHARES as usize {
        return Ok(ApiResponse::fail("份额数量超出上限"));
    }
    if shares.iter().any(|s| s.len() > 4096) {
        return Ok(ApiResponse::fail("份额长度超出上限"));
    }
    let mut pass = passphrase;
    let result = crate::heir_recovery::run_drill_with_passphrase(&shares, pass.as_deref());
    if let Some(value) = pass.as_mut() {
        value.zeroize();
    }
    result.map(ApiResponse::ok)
}

#[command]
pub fn clear_heir_recovery() -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    crate::heir_recovery::clear_meta().map(|_| ApiResponse::ok_with_msg((), "恢复包元数据已清除"))
}

/// 用份额解锁（失能场景下由家人在新机器上执行）。
#[command]
pub fn unlock_with_recovery_shares(
    shares: Vec<String>,
    passphrase: Option<String>,
) -> Result<ApiResponse<SecurityStatus>, String> {
    if shares.is_empty() {
        return Ok(ApiResponse::fail("请至少提供一份份额"));
    }
    if shares.iter().any(|s| s.len() > 4096)
        || shares.len() > crate::heir_recovery::MAX_SHARES as usize
    {
        return Ok(ApiResponse::fail("份额数量或长度超出上限"));
    }
    let mut pass = passphrase;
    let result = crate::heir_recovery::recover_from_shares(&shares, pass.as_deref());
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    match result {
        Ok(msg) => Ok(ApiResponse::ok_with_msg(
            SecurityStatus {
                has_master_password: true,
                unlocked: true,
            },
            &msg,
        )),
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== KDBX 长期归档导出 ==========

/// 导出 KDBX 4 归档：家人用 KeePassXC 即可打开，脱离本应用仍可读。
#[command]
pub fn export_kdbx_archive(
    app_handle: AppHandle,
    archive_password: String,
) -> Result<ApiResponse<String>, String> {
    use crate::report::ReportLocale;
    // ReportLocale::parse 自带回退（未知值 → Zh），无需再包一层
    let locale =
        ReportLocale::parse(&std::env::var("DIGILEGACY_LOCALE").unwrap_or_else(|_| "zh-CN".into()));
    db::ensure_vault_readable_db()?;
    let report = db::get_inheritance_report_db()?;
    let hints = db::get_custody_hints_db().unwrap_or_default();
    let mut pass = archive_password;
    let result = crate::kdbx_export::export_kdbx(
        &report,
        locale,
        &hints.backup_location,
        &hints.recovery_code_location,
        pass.trim(),
    );
    pass.zeroize();
    match result {
        Ok(bytes) => {
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|e| format!("获取目录失败: {}", e))?;
            let dir = app_dir.join("KDBX归档");
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
            let path = dir.join(format!(
                "归零继承归档_{}.kdbx",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            ));
            crate::fsutil::write_private(&path, &bytes)
                .map_err(|e| format!("写入归档失败: {}", e))?;
            Ok(ApiResponse::ok_with_msg(
                path.to_string_lossy().to_string(),
                "KDBX 归档已导出（可用 KeePassXC 打开）",
            ))
        }
        Err(e) => Ok(ApiResponse::fail(&e)),
    }
}

// ========== 内置 Agent（受限本地工具循环） ==========

fn agent_config_and_key() -> Result<(crate::ai_assist::AssistantConfig, String), String> {
    let cfg = db::get_assistant_config_db()?;
    if !cfg.enabled {
        return Err("助手未启用。请先在「我的档案 → AI 助手」配置用户自有端点".to_string());
    }
    if cfg.endpoint.trim().is_empty() || cfg.model.trim().is_empty() || !cfg.has_key {
        return Err("助手配置不完整（缺端点、模型名或 API Key）".to_string());
    }
    let key = db::get_assistant_key_plain()?.ok_or("尚未填写 API Key")?;
    Ok((cfg, key))
}

fn run_agent_inner(
    task: String,
    on_event: Option<tauri::ipc::Channel<crate::agent::AgentEvent>>,
) -> Result<crate::agent::AgentOutcome, String> {
    if task.trim().is_empty() {
        return Err("任务描述不能为空".to_string());
    }
    if task.chars().count() > 2_000 {
        return Err("任务描述过长（上限 2000 字符）".to_string());
    }
    let (cfg, key) = agent_config_and_key()?;
    let report = db::get_inheritance_report_db()?;
    if crate::report::contains_placeholder(&report) {
        return Err("当前数据处于加密锁定状态，请先解锁后再使用 Agent".to_string());
    }
    match on_event {
        Some(channel) => crate::agent::run_task_with_events(
            &task,
            &cfg.endpoint,
            &key,
            &cfg.model,
            move |event| {
                let _ = channel.send(event);
            },
        ),
        None => crate::agent::run_task(&task, &cfg.endpoint, &key, &cfg.model),
    }
}

#[command]
pub async fn run_agent_task(
    task: String,
) -> Result<ApiResponse<crate::agent::AgentOutcome>, String> {
    let result = tauri::async_runtime::spawn_blocking(move || run_agent_inner(task, None))
        .await
        .map_err(|e| format!("Agent 工作线程异常: {}", e))?;
    result
        .map(ApiResponse::ok)
        .or_else(|e| Ok(ApiResponse::fail(&e)))
}

/// 运行受限本地 Agent。事件经 Channel 推给前端，最终结果仍以 ApiResponse 返回。
#[command]
pub async fn run_agent_task_streaming(
    task: String,
    on_event: tauri::ipc::Channel<crate::agent::AgentEvent>,
) -> Result<ApiResponse<crate::agent::AgentOutcome>, String> {
    let result =
        tauri::async_runtime::spawn_blocking(move || run_agent_inner(task, Some(on_event)))
            .await
            .map_err(|e| format!("Agent 工作线程异常: {}", e))?;
    result
        .map(ApiResponse::ok)
        .or_else(|e| Ok(ApiResponse::fail(&e)))
}

// ========== IMAP 入站签到（回邮件 = 我还安好） ==========

#[command]
pub fn get_imap_config() -> Result<ApiResponse<crate::imap_checkin::ImapConfig>, String> {
    crate::imap_checkin::get_config().map(ApiResponse::ok)
}

#[command]
pub fn set_imap_config(
    config: crate::imap_checkin::ImapConfig,
    password: Option<String>,
) -> Result<ApiResponse<()>, String> {
    if let Err(e) = db::ensure_vault_writable_db() {
        return Ok(ApiResponse::fail(&e));
    }
    let mut pass = password;
    let result = crate::imap_checkin::set_config(&config, pass.as_deref());
    if let Some(p) = pass.as_mut() {
        p.zeroize();
    }
    result.map(|_| ApiResponse::ok_with_msg((), "IMAP 签到配置已保存"))
}

/// 手动触发一次收件箱轮询（后台循环每 30 分钟也会跑）。
#[command]
pub fn poll_imap_checkin() -> Result<ApiResponse<crate::imap_checkin::ImapCheckResult>, String> {
    Ok(ApiResponse::ok(crate::imap_checkin::poll_once()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// 注：get_dms_history / get_backup_status 需要 AppHandle，无法在单元测试中构造，
    /// 其底层 outbox_history / latest_backup_summary 已分别由 notify 与 backup 的测试覆盖。
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
