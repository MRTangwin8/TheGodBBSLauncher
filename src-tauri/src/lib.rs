mod commands;
mod config;
pub mod engine;

use tauri::Manager;

use commands::AppState;
use engine::auth::store::AccountStore;
use engine::auth::vault::Vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let settings = config::load(&config_dir.join("settings.json"));
            let vault = Vault::open(&config_dir.join("account.key"));
            let accounts = AccountStore::load(config_dir.join("accounts.json"), vault);
            app.manage(AppState::new(settings, config_dir.join("settings.json"), data_dir, accounts));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::probe_mirrors,
            commands::versions::get_version_manifest,
            commands::versions::list_installed_versions,
            commands::versions::install_version,
            commands::accounts::list_accounts,
            commands::accounts::add_offline_account,
            commands::accounts::microsoft_login_start,
            commands::accounts::microsoft_login_finish,
            commands::accounts::microsoft_login_cancel,
            commands::accounts::add_yggdrasil_account,
            commands::accounts::remove_account,
            commands::accounts::select_account,
            commands::loaders::list_loader_versions,
            commands::loaders::install_loader,
            commands::java::list_java,
            commands::java::memory_info,
            commands::game::launch_game,
            commands::game::kill_game,
            commands::game::list_running,
            commands::crash::analyze_log_text,
            commands::crash::crash_rules_info,
            commands::crash::update_crash_rules,
            commands::crash::reset_crash_rules,
        ])
        .run(tauri::generate_context!())
        .expect("神权论坛启动器启动失败");
}
