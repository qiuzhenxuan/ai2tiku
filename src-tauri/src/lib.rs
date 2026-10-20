mod ai;
mod commands;
mod db;
mod models;
mod ocs;
mod presets;
mod search;
mod server;
mod state;

use state::{AppState, SharedState};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let db = db::Db::open(&dir.join("ai2tiku.db"))?;
            let shared: SharedState = Arc::new(AppState::new(db));
            app.manage(shared.clone());

            // 高 DPI 下窗口的物理尺寸会放大，按屏幕可用区域收敛一下，避免超出屏幕
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = win.current_monitor() {
                    let scale = monitor.scale_factor();
                    let area = monitor.work_area();
                    let avail_w = area.size.width as f64 / scale;
                    let avail_h = area.size.height as f64 / scale;
                    let logical_w = win.inner_size()?.width as f64 / scale;
                    let logical_h = win.inner_size()?.height as f64 / scale;
                    if logical_w > avail_w || logical_h > avail_h {
                        let w = logical_w.min(avail_w * 0.94).max(880.0);
                        let h = logical_h.min(avail_h * 0.94).max(560.0);
                        let _ = win.set_size(tauri::LogicalSize::new(w, h));
                    }
                    let _ = win.center();
                }
            }

            // 启动时按配置的端口自动拉起本地题库服务
            let port = shared.db.settings().map(|s| s.port).unwrap_or(3100);
            if let Err(e) = server::start(&shared, port) {
                eprintln!("[ai2tiku] 启动本地题库服务失败：{}", e);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::default_system_prompt,
            commands::save_settings,
            commands::server_status,
            commands::start_server,
            commands::stop_server,
            commands::list_providers,
            commands::save_provider,
            commands::delete_provider,
            commands::test_provider,
            commands::list_models,
            commands::save_model,
            commands::delete_model,
            commands::list_questions,
            commands::save_question,
            commands::delete_question,
            commands::clear_questions,
            commands::list_records,
            commands::clear_records,
            commands::get_stats,
            commands::build_ocs_config,
            commands::list_provider_presets,
            commands::fetch_models,
            commands::sync_provider_models,
            commands::test_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
