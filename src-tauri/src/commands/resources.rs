use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::versions::INSTALL_PROGRESS_EVENT;
use super::AppState;
use crate::engine::download::{DownloadReport, Downloader, Progress};
use crate::engine::install::{GameDir, Stage};
use crate::engine::loader::{self, LoaderKind};
use crate::engine::meta::version::base_minecraft_version;
use crate::engine::meta::VersionJson;
use crate::engine::resources::local::{self, LocalResource};
use crate::engine::resources::compat::{check_jar, CompatReport, GameEnv, RuntimeJar};
use crate::engine::resources::resolve::{self, Checker, InstallPlan, Target};
use crate::engine::resources::{ResourceApi, ResourceKind, ResourceSource, ResourceVersion, SearchPage, SearchQuery};
use crate::engine::http::{within, SEARCH_TIMEOUT};
use crate::engine::Result;

async fn api(state: &AppState) -> ResourceApi {
    let key = state.settings.read().await.curseforge_api_key.trim().to_string();
    ResourceApi { client: state.client.clone(), curseforge_key: (!key.is_empty()).then_some(key) }
}

/// 某个已安装版本的资源目标：游戏版本、加载器、资源目录
async fn target(state: &AppState, version_id: &str, kind: ResourceKind) -> Result<Target> {
    let settings = state.settings.read().await.clone();
    let game = GameDir::new(settings.game_dir.clone());
    let merged = VersionJson::resolve(&game.versions(), version_id).await?;
    let mc = base_minecraft_version(&game.versions(), version_id).await?;
    let root = crate::engine::instance::game_dir(&game, version_id, settings.isolate_versions);
    Ok(Target { dir: root.join(kind.folder()), kind, game_version: Some(mc), loader: loader::detect(&merged) })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub mc_version: Option<String>,
    pub loader: Option<LoaderKind>,
    pub dir: String,
    pub curseforge_via_mirror: bool,
}

#[tauri::command]
pub async fn resource_target(state: State<'_, AppState>, version_id: String, kind: ResourceKind) -> Result<TargetInfo> {
    let t = target(&state, &version_id, kind).await?;
    Ok(TargetInfo {
        mc_version: t.game_version,
        loader: t.loader,
        dir: t.dir.display().to_string(),
        curseforge_via_mirror: api(&state).await.curseforge_key.is_none(),
    })
}

#[tauri::command]
pub async fn resource_search(state: State<'_, AppState>, source: ResourceSource, query: SearchQuery) -> Result<SearchPage> {
    within(SEARCH_TIMEOUT, api(&state).await.search(source, &query)).await
}

#[tauri::command]
pub async fn klpbbs_search(state: State<'_, AppState>, query: SearchQuery) -> Result<SearchPage> {
    within(SEARCH_TIMEOUT, crate::engine::resources::klpbbs::search(&state.client, &query)).await
}

/// 兼容所选游戏版本的资源版本
#[tauri::command]
pub async fn resource_versions(
    state: State<'_, AppState>,
    source: ResourceSource,
    project_id: String,
    kind: ResourceKind,
    version_id: String,
) -> Result<Vec<ResourceVersion>> {
    let t = target(&state, &version_id, kind).await?;
    api(&state).await.versions(source, &project_id, kind, t.game_version.as_deref(), t.loader).await
}

#[tauri::command]
pub async fn resource_plan(
    state: State<'_, AppState>,
    kind: ResourceKind,
    version_id: String,
    resource: ResourceVersion,
) -> Result<InstallPlan> {
    let t = target(&state, &version_id, kind).await?;
    let checker = checker(&state, &version_id).await?;
    resolve::plan(&api(&state).await, &t, resource, Some(&checker)).await
}

fn cache_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("cache").join("resources")
}

/// 兼容性检查用的游戏环境（游戏 jar 打开一次，供整个计划复用）
async fn checker(state: &AppState, version_id: &str) -> Result<Checker> {
    let settings = state.settings.read().await.clone();
    let game = GameDir::new(settings.game_dir.clone());
    let env = GameEnv::detect(&game, version_id).await?;
    let runtime = match env.runtime_jar.clone() {
        Some(p) => tokio::task::spawn_blocking(move || RuntimeJar::open(&p)).await.ok().flatten().map(Arc::new),
        None => None,
    };
    let downloader = Downloader::new(state.client.clone(), state.source().await, settings.concurrency)
        .with_curseforge_key(Some(&settings.curseforge_api_key));
    Ok(Checker { env, runtime, cache_dir: cache_dir(state), downloader })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCompat {
    pub filename: String,
    pub report: CompatReport,
}

/// 检查已安装模组的兼容性
#[tauri::command]
pub async fn local_compat(state: State<'_, AppState>, version_id: String) -> Result<Vec<LocalCompat>> {
    let t = target(&state, &version_id, ResourceKind::Mod).await?;
    let c = checker(&state, &version_id).await?;
    let files = local::scan(&t.dir, ResourceKind::Mod);
    let dir = t.dir.clone();
    Ok(tokio::task::spawn_blocking(move || {
        files
            .into_iter()
            .map(|f| LocalCompat { report: check_jar(&dir.join(&f.filename), &c.env, c.runtime.as_deref()), filename: f.filename })
            .collect()
    })
    .await
    .unwrap_or_default())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobProgress {
    version_id: String,
    stage: Stage,
    progress: Progress,
}

#[tauri::command]
pub async fn resource_install(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    kind: ResourceKind,
    version_id: String,
    plan: InstallPlan,
) -> Result<DownloadReport> {
    let t = target(&state, &version_id, kind).await?;
    std::fs::create_dir_all(&t.dir)?;
    let settings = state.settings.read().await.clone();
    let downloader = Downloader::new(state.client.clone(), state.source().await, settings.concurrency)
        .with_curseforge_key(Some(&settings.curseforge_api_key));
    let report = Arc::new(move |progress| {
        let _ = app.emit(INSTALL_PROGRESS_EVENT, JobProgress { version_id: job_id.clone(), stage: Stage::Files, progress });
    });
    resolve::execute(&downloader, &t.dir, &plan, Some(&cache_dir(&state)), report).await
}

#[tauri::command]
pub async fn local_resources(state: State<'_, AppState>, version_id: String, kind: ResourceKind) -> Result<Vec<LocalResource>> {
    let t = target(&state, &version_id, kind).await?;
    Ok(local::list(&state.client, &t.dir, kind).await)
}

#[tauri::command]
pub async fn local_resource_toggle(
    state: State<'_, AppState>,
    version_id: String,
    kind: ResourceKind,
    filename: String,
    enabled: bool,
) -> Result<String> {
    let t = target(&state, &version_id, kind).await?;
    local::set_enabled(&t.dir, &filename, enabled)
}

#[tauri::command]
pub async fn local_resource_delete(state: State<'_, AppState>, version_id: String, kind: ResourceKind, filename: String) -> Result<()> {
    let t = target(&state, &version_id, kind).await?;
    local::delete(&t.dir, &filename)
}
