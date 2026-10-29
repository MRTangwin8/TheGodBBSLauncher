use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::java::ensure_java;
use super::versions::{cached_manifest, INSTALL_PROGRESS_EVENT};
use super::AppState;
use crate::engine::download::{Downloader, Progress};
use crate::engine::install::{self, GameDir, Stage};
use crate::engine::loader::{self, LoaderContext, LoaderKind, LoaderVersion};
use crate::engine::meta::{Platform, VersionJson};
use crate::engine::{Error, Result};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobProgress {
    version_id: String,
    stage: Stage,
    progress: Progress,
}

#[tauri::command]
pub async fn list_loader_versions(state: State<'_, AppState>, kind: LoaderKind, mc_version: String) -> Result<Vec<LoaderVersion>> {
    loader::list_versions(&state.client, state.source().await, kind, &mc_version).await
}

/// 安装原版 + 加载器，返回新版本 id。进度事件的 versionId 用前端给的 jobId。
#[tauri::command]
pub async fn install_loader(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    mc_version: String,
    kind: LoaderKind,
    loader_id: String,
    name: Option<String>,
) -> Result<String> {
    let state: &AppState = &state;
    let _task = state.begin_task()?;
    if !state.installing.lock().await.insert(job_id.clone()) {
        return Err(Error::Other("这个加载器已经在安装了，别急".into()));
    }
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let result = do_install(&app, state, &job_id, &mc_version, kind, &loader_id, name.as_deref()).await;
    state.installing.lock().await.remove(&job_id);
    result
}

pub type ReportFn = Arc<dyn Fn(Stage, Progress) + Send + Sync>;

/// 把进度事件发给前端，versionId 用任务 id
pub fn job_reporter(app: &AppHandle, job_id: &str) -> ReportFn {
    let (app, job_id) = (app.clone(), job_id.to_string());
    Arc::new(move |stage, progress| {
        let _ = app.emit(INSTALL_PROGRESS_EVENT, JobProgress { version_id: job_id.clone(), stage, progress });
    })
}

async fn do_install(app: &AppHandle, state: &AppState, job_id: &str, mc: &str, kind: LoaderKind, loader_id: &str, name: Option<&str>) -> Result<String> {
    install_game(state, job_reporter(app, job_id), mc, Some((kind, loader_id)), name).await
}

/// 安装原版 + 可选的加载器，返回版本 id。
/// 没有加载器但给了名字时，生成一个继承原版的同名版本（整合包实例需要自己的文件夹）。
pub async fn install_game(state: &AppState, report: ReportFn, mc: &str, loader: Option<(LoaderKind, &str)>, name: Option<&str>) -> Result<String> {
    let (game, concurrency) = {
        let s = state.settings.read().await;
        (GameDir::new(s.game_dir.clone()), s.concurrency)
    };
    let platform = Platform::current();
    let mut downloader = Downloader::new(state.client.clone(), state.source().await, concurrency);

    // 原版已经装过就只做快速补全，不重算几百 MB 的 SHA1
    downloader.verify_existing = !game.version_json(mc).is_file();
    let manifest = cached_manifest(state).await?;
    let entry = manifest.find(mc).ok_or_else(|| Error::VersionNotFound(mc.to_string()))?.clone();
    let r = install::install_vanilla(&downloader, &game, &entry, &platform, report.clone()).await?;
    if let Some(f) = r.failed.first() {
        return Err(Error::Other(format!("原版 {mc} 有 {} 个文件下载失败：{}", r.failed.len(), f.reason)));
    }
    downloader.verify_existing = true;

    let Some((kind, loader_id)) = loader else {
        return match name {
            Some(name) if name != mc => {
                install::create_inheriting_version(&game, name, mc)?;
                Ok(name.to_string())
            }
            _ => Ok(mc.to_string()),
        };
    };

    // Forge / NeoForge 安装器需要 Java（按原版要求挑）
    let java = match kind {
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let vanilla = VersionJson::resolve(&game.versions(), mc).await?;
            let r = report.clone();
            Some(ensure_java(state, &vanilla, &downloader, Arc::new(move |p| r(Stage::Java, p))).await?)
        }
        _ => None,
    };

    let ctx = LoaderContext { downloader: &downloader, client: &state.client, game: &game, platform: &platform, java, report };
    loader::install(&ctx, kind, mc, loader_id, name).await
}

/// 整合包里写的加载器版本号 → 安装用的 id（Forge 的 id 是完整 maven 版本，比如 1.20.1-47.2.0）
pub async fn loader_id_for(state: &AppState, kind: LoaderKind, mc: &str, version: &str) -> String {
    if let Ok(list) = loader::list_versions(&state.client, state.source().await, kind, mc).await {
        if let Some(v) = list.iter().find(|v| v.version == version || v.id == version) {
            return v.id.clone();
        }
    }
    match kind {
        LoaderKind::Forge => format!("{mc}-{version}"),
        _ => version.to_string(),
    }
}
