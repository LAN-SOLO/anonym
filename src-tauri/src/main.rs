// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod settings;
mod state;
mod store;

use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = settings::config_dir(&handle);
            let s = settings::load(&config_dir);
            let rules = store::load_rules(&config_dir);
            app.manage(Arc::new(AppState {
                settings: Mutex::new(s),
                rules: Mutex::new(rules),
                dicts: anonym_core::Dictionaries::builtin(),
                config_dir,
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::set_settings,
            commands::data_path,
            commands::get_rules,
            commands::set_rules,
            commands::default_rules,
            commands::import_rules,
            commands::export_rules,
            commands::list_worlds,
            commands::analyze_file,
            commands::apply_file,
            commands::suggest_output,
            commands::store_info,
            commands::store_clear,
            commands::store_export,
            commands::store_import,
            commands::path_exists,
            commands::open_path,
            commands::check_update,
            commands::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running anonym");
}
