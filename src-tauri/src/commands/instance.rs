//! 版本管理 → 管理：单个版本的 Java / 内存 / 参数，以及存档的备份和恢复。
//! 模组列表直接用资源中心的 local_resources / local_resource_toggle / local_resource_delete。

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use super::AppState;
use crate::engine::install::GameDir;
use crate::engine::instance::{self, InstanceSettings, ModpackOrigin};
use crate::engine::saves::{self, Backup, World};
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

/// 恢复会替换存档文件夹，用到同一个游戏目录的游戏都得先关掉
#[tauri::command]
pub async fn world_restore(state: State<'_, AppState>, version_id: String, file: String) -> Result<Option<Backup>> {
    state.ensure_not_migrating()?;
    let (game, dir, global) = locate(&state, &version_id).await?;
    let running: Vec<String> = state.games.lock().await.values().map(|g| g.version_id.clone()).collect();
    if running.iter().any(|v| instance::game_dir(&game, v, global) == dir) {
        return Err(Error::Other("游戏还开着，先退出游戏再恢复存档".into()));
    }
    blocking(move || saves::restore(&dir, &file)).await
}

#[tauri::command]
pub async fn world_backup_delete(state: State<'_, AppState>, version_id: String, file: String) -> Result<()> {
    let (_, dir, _) = locate(&state, &version_id).await?;
    saves::delete_backup(&dir, &file)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Other(format!("任务意外中断：{e}")))?
}
