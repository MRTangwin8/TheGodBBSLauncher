//! 版本管理 → 管理：单个版本的 Java / 内存 / 参数、存档的备份和恢复、服务器列表、截图。
//! 模组列表直接用资源中心的 local_resources / local_resource_toggle / local_resource_delete。

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::AppState;
use crate::engine::install::GameDir;
use crate::engine::instance::{self, InstanceSettings, ModpackOrigin};
use crate::engine::launch::server::parse_server;
use crate::engine::saves::{self, Backup, World};
use crate::engine::screenshots::{self, Screenshot};
use crate::engine::servers::{self, SavedServer, ServerStatus};
use crate::engine::{Error, Result};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub settings: InstanceSettings,
    pub modpack: Option<ModpackOrigin>,
    /// 这个版本运行时用的游戏目录（存档、模组都在这里）
    pub game_dir: String,
    /// 全局设置里开了版本隔离：那这里的「隔离」开关不起作用
    pub global_isolate: bool,
    pub play_time_secs: u64,
    pub last_played: Option<u64>,
}

/// 确认版本存在，返回 (游戏根目录, 这个版本的游戏目录, 全局隔离)
async fn locate(state: &AppState, version_id: &str) -> Result<(GameDir, PathBuf, bool)> {
    let settings = state.settings.read().await;
    let game = GameDir::new(settings.game_dir.clone());
    let plain = !version_id.is_empty() && !version_id.contains(['/', '\\']) && version_id != "." && version_id != "..";
    if !plain || !game.versions().join(version_id).is_dir() {
        return Err(Error::VersionNotFound(version_id.to_string()));
    }
    let dir = instance::game_dir(&game, version_id, settings.isolate_versions);
    Ok((game, dir, settings.isolate_versions))
}

fn info(game: &GameDir, version_id: &str, global_isolate: bool) -> InstanceInfo {
    let cfg = instance::load(game, version_id);
    InstanceInfo {
        settings: cfg.settings(),
        modpack: cfg.modpack.clone(),
        game_dir: instance::game_dir(game, version_id, global_isolate).display().to_string(),
        global_isolate,
        play_time_secs: cfg.play_time_secs,
        last_played: cfg.last_played,
    }
}

#[tauri::command]
pub async fn instance_info(state: State<'_, AppState>, version_id: String) -> Result<InstanceInfo> {
    let (game, _, global) = locate(&state, &version_id).await?;
    Ok(info(&game, &version_id, global))
}

#[tauri::command]
pub async fn instance_save(state: State<'_, AppState>, version_id: String, settings: InstanceSettings) -> Result<InstanceInfo> {
    let (game, _, global) = locate(&state, &version_id).await?;
    let mut cfg = instance::load(&game, &version_id);
    cfg.apply(settings)?;
    instance::save(&game, &version_id, &cfg)?;
    Ok(info(&game, &version_id, global))
}

#[tauri::command]
pub async fn world_list(state: State<'_, AppState>, version_id: String) -> Result<Vec<World>> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    Ok(tokio::task::spawn_blocking(move || saves::list(&dir)).await.unwrap_or_default())
}

#[tauri::command]
pub async fn world_backups(state: State<'_, AppState>, version_id: String) -> Result<Vec<Backup>> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    Ok(saves::backups(&dir))
}

#[tauri::command]
pub async fn world_backup(state: State<'_, AppState>, version_id: String, world: String) -> Result<Backup> {
    state.ensure_not_migrating()?;
    let (_, dir, _) = locate(&state, &version_id).await?;
    blocking(move || saves::backup(&dir, &world)).await
}

/// 有没有正在运行的游戏用着这个游戏目录
async fn ensure_idle(state: &AppState, game: &GameDir, dir: &std::path::Path, global: bool, msg: &str) -> Result<()> {
    let running: Vec<String> = state.games.lock().await.values().map(|g| g.version_id.clone()).collect();
    if running.iter().any(|v| instance::game_dir(game, v, global) == dir) {
        return Err(Error::Other(msg.into()));
    }
    Ok(())
}

/// 恢复会替换存档文件夹，用到同一个游戏目录的游戏都得先关掉
#[tauri::command]
pub async fn world_restore(state: State<'_, AppState>, version_id: String, file: String) -> Result<Option<Backup>> {
    state.ensure_not_migrating()?;
    let (game, dir, global) = locate(&state, &version_id).await?;
    ensure_idle(&state, &game, &dir, global, "游戏还开着，先退出游戏再恢复存档").await?;
    blocking(move || saves::restore(&dir, &file)).await
}

#[tauri::command]
pub async fn world_backup_delete(state: State<'_, AppState>, version_id: String, file: String) -> Result<()> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    saves::delete_backup(&dir, &file)
}

// ---- 服务器列表（servers.dat） ----

fn servers_file(dir: &std::path::Path) -> PathBuf {
    dir.join("servers.dat")
}

fn checked_address(address: &str) -> Result<String> {
    parse_server(address).ok_or_else(|| Error::Other(format!("服务器地址不合法：{}（应为 主机名 或 主机名:端口）", address.trim())))?;
    Ok(address.trim().to_string())
}

#[tauri::command]
pub async fn server_list(state: State<'_, AppState>, version_id: String) -> Result<Vec<SavedServer>> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    blocking(move || servers::list(&servers_file(&dir))).await
}

/// 改列表前先确认游戏没开：游戏退出时会把它内存里的列表写回 servers.dat，改了也会被盖掉
async fn editable(state: &AppState, version_id: &str) -> Result<PathBuf> {
    state.ensure_not_migrating()?;
    let (game, dir, global) = locate(state, version_id).await?;
    ensure_idle(state, &game, &dir, global, "游戏还开着，退出游戏后再改服务器列表（不然游戏退出时会把改动盖掉）").await?;
    Ok(servers_file(&dir))
}

#[tauri::command]
pub async fn server_add(state: State<'_, AppState>, version_id: String, name: String, address: String) -> Result<Vec<SavedServer>> {
    let ip = checked_address(&address)?;
    let file = editable(&state, &version_id).await?;
    blocking(move || {
        servers::add(&file, &name, &ip)?;
        servers::list(&file)
    })
    .await
}

#[tauri::command]
pub async fn server_update(state: State<'_, AppState>, version_id: String, index: usize, name: String, address: String) -> Result<Vec<SavedServer>> {
    let ip = checked_address(&address)?;
    let file = editable(&state, &version_id).await?;
    blocking(move || {
        servers::update(&file, index, &name, &ip)?;
        servers::list(&file)
    })
    .await
}

#[tauri::command]
pub async fn server_remove(state: State<'_, AppState>, version_id: String, index: usize) -> Result<Vec<SavedServer>> {
    let file = editable(&state, &version_id).await?;
    blocking(move || {
        servers::remove(&file, index)?;
        servers::list(&file)
    })
    .await
}

#[tauri::command]
pub async fn server_move(state: State<'_, AppState>, version_id: String, from: usize, to: usize) -> Result<Vec<SavedServer>> {
    let file = editable(&state, &version_id).await?;
    blocking(move || {
        servers::reorder(&file, from, to)?;
        servers::list(&file)
    })
    .await
}

/// 查询服务器状态（延迟、在线人数、MOTD）
#[tauri::command]
pub async fn server_ping(address: String) -> Result<ServerStatus> {
    let (host, port) = parse_server(&address).ok_or_else(|| Error::Other(format!("服务器地址不合法：{}", address.trim())))?;
    servers::ping(&host, port).await
}

// ---- 截图 ----

#[tauri::command]
pub async fn screenshot_list(state: State<'_, AppState>, version_id: String) -> Result<Vec<Screenshot>> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    Ok(tokio::task::spawn_blocking(move || screenshots::list(&dir)).await.unwrap_or_default())
}

/// 图片原始字节，界面里转成 blob 显示（不开放本地文件协议，只能读 screenshots 里的图片）
#[tauri::command]
pub async fn screenshot_data(state: State<'_, AppState>, version_id: String, name: String) -> Result<tauri::ipc::Response> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    let path = screenshots::path(&dir, &name)?;
    Ok(tauri::ipc::Response::new(tokio::fs::read(path).await?))
}

/// 用系统的看图程序打开
#[tauri::command]
pub async fn screenshot_open(app: AppHandle, state: State<'_, AppState>, version_id: String, name: String) -> Result<()> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    let path = screenshots::path(&dir, &name)?;
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(|e| Error::Other(format!("打不开图片：{e}")))
}

/// 在文件管理器里选中这张图
#[tauri::command]
pub async fn screenshot_reveal(app: AppHandle, state: State<'_, AppState>, version_id: String, name: String) -> Result<()> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    let path = screenshots::path(&dir, &name)?;
    app.opener().reveal_item_in_dir(path).map_err(|e| Error::Other(format!("打不开文件夹：{e}")))
}

#[tauri::command]
pub async fn screenshot_delete(state: State<'_, AppState>, version_id: String, name: String) -> Result<()> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    screenshots::delete(&dir, &name)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Other(format!("任务意外中断：{e}")))?
}
