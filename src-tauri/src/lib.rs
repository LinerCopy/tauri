//! Точка входа Tauri 2 (поддерживает desktop + mobile через `mobile_entry_point`).

pub mod commands;
pub mod dto;
pub mod ffi;
#[cfg(feature = "rust-core")]
pub mod rust_core;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_site,
            commands::core_version,
            commands::save_report,
            commands::trust_store_info,
            commands::check_trust_store_updates,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_title("GosCertInspector (dev)");
                }
            }
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
