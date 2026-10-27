use std::path::{Path, PathBuf};

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;

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
    let settings = settings.sanitized();
    config::save(&state.config_path, &settings)?;
    *state.settings.write().await = settings;
    *state.auto_source.lock().await = None;
    Ok(())
}

/// 自定义背景图，以 data URL 返回（不用开放 asset 协议）；没设置或读不到返回 None。
/// `path`：前端刚选的图片。不传就读已保存的设置——刚选完图时设置可能还没写完，读到的是旧值，所以前端会把路径带过来
#[tauri::command]
pub async fn background_image(state: State<'_, AppState>, path: Option<PathBuf>) -> Result<Option<String>> {
    use base64::Engine;
    let Some(path) = (match path {
        Some(p) => Some(p),
        None => state.settings.read().await.background_image.clone(),
    }) else {
        return Ok(None);
    };
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
pub async fn check_update(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateInfo> {
    let mut info = update::check(&state.client, env!("CARGO_PKG_VERSION")).await?;
    info.can_install = updater_enabled(&app);
    Ok(info)
}

/// 打包时有没有写进更新签名公钥。没有就不能在应用内更新：装的东西没法验证来源
fn updater_enabled(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|c| c.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

pub const UPDATE_PROGRESS_EVENT: &str = "update-progress";

#[derive(Clone, serde::Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

static INSTALLING: AtomicBool = AtomicBool::new(false);

/// 下载并安装用户确认过的版本，装完重启。
/// 更新清单只从本仓库对应标签的 Release 里取，安装包必须通过内置公钥的签名校验，校验不过不会安装。
#[tauri::command]
pub async fn install_update(app: AppHandle, version: String) -> Result<()> {
    if !updater_enabled(&app) {
        return Err(Error::Other("这个版本没有内置更新签名密钥，不能在应用内更新，请前往发布页下载安装".into()));
    }
    let tag = update::release_tag(&version)?;
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(Error::Other("更新正在进行中".into()));
    }
    let result = do_install(&app, &tag, version.trim().trim_start_matches(['v', 'V'])).await;
    INSTALLING.store(false, Ordering::SeqCst);
    result?;
    app.restart()
}

async fn do_install(app: &AppHandle, tag: &str, version: &str) -> Result<()> {
    let fail = |what: &str, e: &dyn std::fmt::Display| Error::Other(format!("{what}：{e}"));
    let manifest = format!("https://github.com/{}/releases/download/{tag}/latest.json", update::REPO);
    let url = manifest.parse().map_err(|e| fail("更新地址不合法", &e))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| fail("更新地址不合法", &e))?
        .build()
        .map_err(|e| fail("初始化更新失败", &e))?;
    let update = updater
        .check()
        .await
        .map_err(|e| fail("读取更新清单失败（这个版本可能没有发布应用内更新包）", &e))?
        .ok_or_else(|| Error::Other("已经是最新版".into()))?;
    // 确认装的就是弹窗里问过的版本，不被清单里别的版本顶替
    if update.version != version {
        return Err(Error::Other(format!("更新清单里的版本是 {}，和要安装的 {version} 不一致，已取消", update.version)));
    }
    let mut downloaded = 0u64;
    let handle = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = handle.emit(UPDATE_PROGRESS_EVENT, UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| fail("下载或安装失败", &e))
}
