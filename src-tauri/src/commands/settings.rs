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

/// 自定义背景图，以 data URL 返回（不用开放 asset 协议）；没设置或读不到返回 None
#[tauri::command]
pub async fn background_image(state: State<'_, AppState>) -> Result<Option<String>> {
    use base64::Engine;
    let Some(path) = state.settings.read().await.background_image.clone() else { return Ok(None) };
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => return Ok(None),
    };
    let Ok(bytes) = tokio::fs::read(&path).await else { return Ok(None) };
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(Error::Other("背景图太大了（超过 20MB）".into()));
    }
    Ok(Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
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
