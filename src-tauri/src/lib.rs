mod ai;
#[cfg(not(any(feature = "native", feature = "webview")))]
compile_error!("请启用 webview 或 native 功能，默认使用 WebView 界面");
#[cfg(all(
    feature = "webview",
    not(debug_assertions),
    not(feature = "custom-protocol")
))]
compile_error!("WebView Release 需要 --features custom-protocol，以嵌入生产界面资源");
mod answers;
mod browser;
#[cfg(feature = "webview")]
mod commands;
mod db;
mod diagnostics;
mod models;
#[cfg(feature = "native")]
mod native;
mod ocs;
mod presets;
mod search;
mod server;
mod services;
mod state;
#[cfg(test)]
mod test_support;
mod transfer;

#[cfg(feature = "webview")]
use state::{AppState, SharedState};
#[cfg(feature = "webview")]
use std::sync::Arc;
#[cfg(feature = "webview")]
use tauri::Manager;

#[cfg(not(feature = "webview"))]
pub fn run() {
    #[cfg(feature = "native")]
    if let Err(error) = native::run() {
        eprintln!("启动失败：{error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "webview")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let args: Vec<_> = std::env::args_os().collect();
            let dir = args
                .windows(2)
                .find(|pair| pair[0] == "--data-dir")
                .map(|pair| std::path::PathBuf::from(&pair[1]))
                .map(Ok)
                .unwrap_or_else(|| app.path().app_data_dir())?;
            let db = db::Db::open(&dir.join("ai2tiku.db"))?;
            let shared: SharedState = Arc::new(AppState::new(db));
            app.manage(shared.clone());

            // 高 DPI 下窗口的物理尺寸会放大，按屏幕可用区域收敛一下，避免超出屏幕
            if let Some(win) = app.get_webview_window("main") {
                // Disable the embedded browser menu at its source. The Vue shell also
                // cancels contextmenu events; text editing shortcuts stay available.
                #[cfg(windows)]
                if let Err(error) = win.with_webview(|webview| unsafe {
                    if let Err(error) = webview
                        .controller()
                        .CoreWebView2()
                        .and_then(|core| core.Settings())
                        .and_then(|settings| settings.SetAreDefaultContextMenusEnabled(false))
                    {
                        eprintln!("[ai2tiku] 关闭 WebView2 默认右键菜单失败：{error}");
                    }
                }) {
                    eprintln!("[ai2tiku] 配置 WebView2 失败：{error}");
                }
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
            commands::get_ui_pref,
            commands::set_ui_pref,
            commands::latest_ocs_release,
            commands::browser_info,
            commands::open_in_edge,
            commands::deployment_status,
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
            commands::data_revision,
            commands::export_question_file,
            commands::export_questions_json,
            commands::pick_question_file,
            commands::preview_question_import,
            commands::apply_question_import,
            commands::run_diagnostics,
            commands::build_ocs_config,
            commands::list_provider_presets,
            commands::fetch_models,
            commands::sync_provider_models,
            commands::test_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
