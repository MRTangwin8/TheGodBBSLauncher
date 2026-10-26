use std::path::{Path, PathBuf};

use tauri::State;

use super::AppState;
use crate::config::{self, Settings};
use crate::engine::mirror::{self, ProbeResult};
use crate::engine::theme::{self, ThemePreview};
use crate::engine::update::{self, UpdateInfo};
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

/// 把当前外观导出成 .godtheme
#[tauri::command]
pub async fn theme_export(state: State<'_, AppState>, path: PathBuf, name: String, author: String) -> Result<()> {
    let settings = state.settings.read().await.clone();
    let bytes = tokio::task::spawn_blocking(move || theme::export(&name, &author, &settings))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    tokio::fs::write(&path, bytes).await?;
    Ok(())
}

async fn read_theme(path: &Path) -> Result<theme::ThemeFile> {
    let meta = tokio::fs::metadata(path).await?;
    // base64 后会比原图大三分之一，再留点 JSON 的余量
    if meta.len() as usize > theme::MAX_IMAGE / 3 * 4 + 64 * 1024 {
        return Err(Error::Other("主题文件太大了".into()));
    }
    theme::parse(&tokio::fs::read(path).await?)
}

/// 导入前预览
#[tauri::command]
pub async fn theme_preview(path: PathBuf) -> Result<ThemePreview> {
    Ok(theme::preview(&read_theme(&path).await?))
}

/// 导入并立即应用，返回新的设置
#[tauri::command]
pub async fn theme_import(state: State<'_, AppState>, path: PathBuf) -> Result<Settings> {
    let file = read_theme(&path).await?;
    let mut settings = state.settings.read().await.clone();
    theme::apply(file, &mut settings, &state.data_dir.join("themes"))?;
    config::save(&state.config_path, &settings)?;
    *state.settings.write().await = settings.clone();
    Ok(settings)
}

/// 检查 GitHub 上有没有新版本。只做查询，不下载也不安装；是否提示由前端按设置决定
#[tauri::command]
pub async fn check_update(state: State<'_, AppState>) -> Result<UpdateInfo> {
    update::check(&state.client, env!("CARGO_PKG_VERSION")).await
}
