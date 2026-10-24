use tauri::State;

use super::AppState;
use crate::config::{self, Settings};
use crate::engine::mirror::{self, ProbeResult};
use crate::engine::{Error, Result};

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    config::save(&state.config_path, &settings)?;
    *state.settings.write().await = settings;
    *state.auto_source.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn probe_mirrors(state: State<'_, AppState>) -> Result<Vec<ProbeResult>> {
    let results = mirror::probe(&state.client).await;
    if results.iter().all(|r| r.millis.is_none()) {
        return Err(Error::Other("两个下载源都连不上，检查一下网络？".into()));
    }
    *state.auto_source.lock().await = Some(mirror::fastest(&results));
    Ok(results)
}
