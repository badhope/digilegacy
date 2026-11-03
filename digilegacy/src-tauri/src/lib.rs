mod action_guide;
mod agent;
mod agent_tools;
mod ai_assist;
mod backup;
mod checkpoint;
mod commands;
mod crypto;
mod db;
mod family_faq;
mod fsutil;
mod heartbeat;
mod heir_recovery;
mod imap_checkin;
mod kdbx_export;
mod models;
mod notify;
mod platform_data;
mod recovery;
mod report;
mod review;
mod review_heir;
mod review_profiles;
#[cfg(test)]
mod scenarios;
mod toolkit;
mod vault;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

/// 后台推进线程：每 30 分钟评估 DMS（写发件箱）并尝试云端心跳。
///
/// 关键：整轮工作包在 `catch_unwind` 里。DMS 评估与心跳都会触达全局状态
/// （DB 连接、LAST_STATUS、文件系统），任一处的 panic 会直接杀死这个线程——
/// 而线程一旦死了，用户会「以为」安心传承还在值守，实际提醒永久停摆。
/// 崩溃后记录并继续下一轮，比静默停摆安全得多。
fn spawn_background_loop(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        let handle = app_handle.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Err(e) = notify::evaluate_dms(handle.clone()) {
                eprintln!("后台 DMS 评估失败: {}", e);
            }
            heartbeat::send_heartbeat();
            // IMAP 入站签到：回信 = 我还安好。失败只记录，不影响其他轮次。
            let r = imap_checkin::poll_once();
            if r.enabled && !r.connected {
                eprintln!("后台 IMAP 签到轮询失败: {}", r.message);
            }
        }));
        if result.is_err() {
            // 单轮崩溃不应终止守护：记录后继续下一轮
            eprintln!(
                "[{}] 后台任务发生 panic，已隔离并将在下一轮继续",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(30 * 60));
    });
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let checkin = MenuItem::with_id(app, "checkin", "我还安好（安全确认）", true, None::<&str>)?;
    let lock_item = MenuItem::with_id(app, "lock", "锁定应用", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &checkin, &lock_item, &quit])?;

    let mut tray = TrayIconBuilder::with_id("digilegacy-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("归零 · 数字遗产管理系统")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
            }
            "checkin" => {
                let _ = db::record_check_in_db();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.emit("dms-checked-in", ());
                }
            }
            "lock" => {
                crate::vault::clear_key();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.emit("app-locked", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(win) = tray.app_handle().get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例：双开会让两个进程同时跑后台 DMS 循环，
        // 同一封「到期提醒」被投递两次，家人收到重复邮件；SQLite 也会争锁。
        // 第二个实例把已有主窗口拉到前台后退出。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            db::init_database(app.handle())?;

            // 每日自动备份：失败不阻断启动（磁盘满/权限问题不应让应用不可用）
            if let Ok(dir) = app.path().app_data_dir() {
                match db::export_backup_json_db() {
                    Ok(json) => {
                        if let Err(e) = backup::run_daily_backup(&dir, &json) {
                            eprintln!("自动备份失败: {}", e);
                        }
                    }
                    Err(e) => eprintln!("自动备份跳过（导出失败）: {}", e),
                }
            }

            build_tray(app)?;
            spawn_background_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_all_platforms,
            commands::get_all_platforms_extended,
            commands::import_platform_patch,
            commands::export_platform_patch,
            commands::platform_patch_status,
            commands::clear_platform_patch,
            commands::add_asset,
            commands::update_asset,
            commands::delete_asset,
            commands::get_all_assets,
            commands::get_asset_by_id,
            commands::get_assets_by_category,
            commands::set_disposal_wish,
            commands::set_user_profile,
            commands::get_user_profile,
            commands::get_inheritance_report,
            commands::render_report,
            commands::export_report_to_file,
            commands::export_zip_to_file,
            commands::reveal_exported_file,
            commands::get_custody_hints,
            commands::set_custody_hints,
            commands::render_action_guide,
            commands::run_family_drill,
            commands::list_authorities,
            commands::authorities_for,
            commands::list_review_profiles,
            commands::list_review_items,
            commands::review_payload_fields,
            commands::review_progress,
            commands::run_ai_review,
            commands::submit_human_review,
            commands::review_history,
            commands::get_heirs,
            commands::set_heirs,
            commands::get_assistant_config,
            commands::set_assistant_config,
            commands::preview_assistant_payload,
            commands::ask_assistant,
            commands::update_check_in,
            commands::get_check_in_status,
            commands::get_dms_status,
            commands::get_dms_history,
            commands::set_dead_mans_switch,
            commands::get_dashboard_stats,
            commands::security_status,
            commands::set_master_password,
            commands::unlock,
            commands::lock,
            commands::set_asset_credential,
            commands::get_asset_credential,
            commands::change_master_password,
            commands::export_backup,
            commands::import_backup,
            commands::open_external,
            commands::start_recovery,
            commands::complete_recovery,
            commands::get_recovery_seal_exists,
            commands::get_backup_status,
            commands::create_checkpoint,
            commands::list_checkpoints,
            commands::restore_checkpoint,
            commands::delete_checkpoint,
            commands::get_heartbeat_status,
            commands::get_smtp_config,
            commands::set_smtp_config,
            commands::send_test_email,
            commands::get_heartbeat_config,
            commands::set_heartbeat_config,
            commands::send_heartbeat_now,
            commands::generate_heir_recovery_kit,
            commands::heir_recovery_status,
            commands::clear_heir_recovery,
            commands::run_recovery_drill,
            commands::unlock_with_recovery_shares,
            commands::get_imap_config,
            commands::set_imap_config,
            commands::poll_imap_checkin,
            commands::run_agent_task,
            commands::run_agent_task_streaming,
            commands::export_kdbx_archive,
            commands::ask_assistant_streaming,
            commands::list_ai_chat,
            commands::clear_ai_chat,
            commands::set_autostart,
            commands::hide_main_window,
            notify::evaluate_dms,
        ])
        .on_window_event(|window, event| {
            // 关闭窗口 = 隐藏到托盘（驻留后台推进提醒与心跳）
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// 后台循环的核心契约：单轮 panic 必须被隔离，不能终止后续轮次。
    /// 这里用与 `spawn_background_loop` 相同的结构做等价验证
    /// （真实函数会开线程并 sleep 30 分钟，不适合直接测试）。
    #[test]
    fn panic_in_one_round_does_not_stop_the_loop() {
        let rounds = Arc::new(AtomicUsize::new(0));
        let r = rounds.clone();

        // 模拟 3 轮：第 2 轮 panic
        for i in 0..3 {
            let r2 = r.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if i == 1 {
                    panic!("模拟后台任务崩溃");
                }
                r2.fetch_add(1, Ordering::SeqCst);
            }));
            // 关键：panic 被抓住，循环继续
            assert_eq!(result.is_err(), i == 1, "第 {} 轮的 panic 必须被隔离", i);
        }

        assert_eq!(
            rounds.load(Ordering::SeqCst),
            2,
            "除崩溃轮外，其余两轮必须正常执行"
        );
    }

    /// 使用 AssertUnwindSafe 后仍能捕获 panic（确认没有 unwind 泄漏）。
    #[test]
    fn catch_unwind_captures_panic_without_aborting() {
        let result = std::panic::catch_unwind(|| {
            panic!("boom");
        });
        assert!(result.is_err());
        // 走到这里说明进程没有 abort
    }
}
