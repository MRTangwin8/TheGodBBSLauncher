mod commands;
mod config;
mod edition;
pub mod engine;

use tauri::{Emitter, Manager};

use commands::AppState;
use engine::auth::store::AccountStore;
use engine::auth::vault::Vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 必须第一个注册：已经开着启动器时，第二次打开（比如桌面快捷方式）只把参数交给这个窗口
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
            if let Some(id) = engine::shortcut::launch_arg(&argv) {
                let _ = app.emit("launch-request", id);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            // 标识符从 me.mcbbs.godbbs 改成了 co.mcbbs.godbbs：把旧目录搬过来（Windows / macOS 上两者是同一个目录）
            let moved: Vec<_> = [&config_dir, &data_dir]
                .into_iter()
                .filter_map(|dir| config::migrate_legacy_dir(dir).map(|old| (old, dir.clone())))
                .collect();
            let mut settings = config::load(&config_dir.join("settings.json"));
            if moved.iter().fold(false, |changed, (old, new)| settings.rebase_paths(old, new) | changed) {
                let _ = config::save(&config_dir.join("settings.json"), &settings);
            }
            let vault = Vault::open(&config_dir.join("account.key"));
            let accounts = AccountStore::load(config_dir.join("accounts.json"), vault);
            let state = AppState::new(settings, config_dir.join("settings.json"), data_dir, accounts);
            *state.pending_launch.lock().unwrap_or_else(|e| e.into_inner()) = engine::shortcut::launch_arg(&std::env::args().collect::<Vec<_>>());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::announce::get_announcements,
            commands::buttons::quick_open_folder,
            commands::gamedir::game_dir_info,
            commands::gamedir::dir_stats,
            commands::gamedir::game_dir_migrate,
            commands::gamedir::game_dir_migrate_cancel,
            commands::buttons::quick_run_program,
            commands::settings::save_settings,
            commands::settings::probe_mirrors,
            commands::settings::background_image,
            commands::settings::theme_export,
            commands::settings::theme_preview,
            commands::settings::theme_import,
            commands::settings::check_update,
            commands::settings::install_update,
            commands::versions::get_version_manifest,
            commands::versions::list_installed_versions,
            commands::versions::install_version,
            commands::versions::cancel_job,
            commands::versions::rename_version,
            commands::versions::copy_version,
            commands::accounts::list_accounts,
            commands::accounts::add_offline_account,
            commands::accounts::rename_offline_account,
            commands::accounts::microsoft_login_start,
            commands::accounts::microsoft_login_finish,
            commands::accounts::microsoft_login_cancel,
            commands::accounts::add_yggdrasil_account,
            commands::accounts::remove_account,
            commands::accounts::account_skin,
            commands::accounts::select_account,
            commands::loaders::list_loader_versions,
            commands::loaders::install_loader,
            commands::loaders::list_optifine,
            commands::loaders::install_optifine,
            commands::java::list_java,
            commands::java::memory_info,
            commands::game::launch_game,
            commands::game::kill_game,
            commands::game::list_running,
            commands::game::take_pending_launch,
            commands::game::create_shortcut,
            commands::resources::resource_target,
            commands::resources::resource_search,
            commands::resources::klpbbs_search,
            commands::resources::resource_versions,
            commands::resources::resource_plan,
            commands::resources::resource_install,
            commands::resources::local_resources,
            commands::resources::local_resource_toggle,
            commands::resources::local_resource_delete,
            commands::resources::mod_updates,
            commands::instance::instance_info,
            commands::instance::instance_save,
            commands::instance::world_list,
            commands::instance::world_backups,
            commands::instance::world_backup,
            commands::instance::world_restore,
            commands::instance::world_backup_delete,
            commands::instance::server_list,
            commands::instance::server_add,
            commands::instance::server_update,
            commands::instance::server_remove,
            commands::instance::server_move,
            commands::instance::server_ping,
            commands::instance::screenshot_list,
            commands::instance::screenshot_data,
            commands::instance::screenshot_open,
            commands::instance::screenshot_reveal,
            commands::instance::screenshot_delete,
            commands::resources::local_compat,
            commands::modpack::modpack_preview,
            commands::modpack::modpack_import,
            commands::modpack::modpack_versions,
            commands::modpack::modpack_download,
            commands::modpack::export_candidates,
            commands::modpack::modpack_export,
            commands::crash::analyze_log_text,
            commands::crash::crash_rules_info,
            commands::crash::update_crash_rules,
            commands::crash::reset_crash_rules,
            commands::crash::share_log,
            commands::multiplayer::mp_status,
            commands::multiplayer::mp_install,
            commands::multiplayer::mp_host,
            commands::multiplayer::mp_join,
            commands::multiplayer::mp_leave,
        ])
        .build(tauri::generate_context!())
        .expect("神权论坛启动器启动失败")
        .run(|app, event| {
            // 关掉我们拉起的陶瓦联机进程（别人在用的不关）
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(commands::multiplayer::shutdown(&app.state::<AppState>()));
            }
        });
}
