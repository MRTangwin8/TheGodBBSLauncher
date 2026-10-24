mod commands;
mod config;
pub mod engine;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("settings.json");
            let settings = config::load(&config_path);
            app.manage(AppState::new(settings, config_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::probe_mirrors,
            commands::versions::get_version_manifest,
            commands::versions::list_installed_versions,
            commands::versions::install_version,
        ])
        .run(tauri::generate_context!())
        .expect("神权论坛启动器启动失败");
}
