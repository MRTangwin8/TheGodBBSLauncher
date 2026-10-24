use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::AppState;
use crate::engine::download::{DownloadReport, Downloader, Progress};
use crate::engine::install::{self, GameDir, InstalledVersion, Stage};
use crate::engine::meta::{Platform, VersionManifest};
use crate::engine::{Error, Result};

pub const INSTALL_PROGRESS_EVENT: &str = "install-progress";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    version_id: String,
    stage: Stage,
    progress: Progress,
}

#[tauri::command]
pub async fn get_version_manifest(state: State<'_, AppState>, force: bool) -> Result<VersionManifest> {
    if force {
        *state.manifest.lock().await = None;
    }
    cached_manifest(&state).await
}

#[tauri::command]
pub async fn list_installed_versions(state: State<'_, AppState>) -> Result<Vec<InstalledVersion>> {
    let game = GameDir::new(state.settings.read().await.game_dir.clone());
    install::list_installed(&game).await
}

#[tauri::command]
pub async fn install_version(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<DownloadReport> {
    if !state.installing.lock().await.insert(id.clone()) {
        return Err(Error::Other(format!("{id} 已经在安装了，别急")));
    }
    let result = do_install(&app, &state, &id).await;
    state.installing.lock().await.remove(&id);
    result
}

async fn do_install(app: &AppHandle, state: &AppState, id: &str) -> Result<DownloadReport> {
    let manifest = cached_manifest(state).await?;
    let entry = manifest.find(id).ok_or_else(|| Error::VersionNotFound(id.to_string()))?.clone();
    let (game, concurrency) = {
        let s = state.settings.read().await;
        (GameDir::new(s.game_dir.clone()), s.concurrency)
    };
    let downloader = Downloader::new(state.client.clone(), state.source().await, concurrency);

    let (app, version_id) = (app.clone(), id.to_string());
    let report = Arc::new(move |stage: Stage, progress: Progress| {
        let _ = app.emit(INSTALL_PROGRESS_EVENT, InstallProgress { version_id: version_id.clone(), stage, progress });
    });
    install::install_vanilla(&downloader, &game, &entry, &Platform::current(), report).await
}

async fn cached_manifest(state: &AppState) -> Result<VersionManifest> {
    if let Some(m) = state.manifest.lock().await.clone() {
        return Ok(m);
    }
    let m = install::fetch_manifest(&state.client, state.source().await).await?;
    *state.manifest.lock().await = Some(m.clone());
    Ok(m)
}
