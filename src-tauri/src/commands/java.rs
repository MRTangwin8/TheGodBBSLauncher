use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use super::AppState;
use crate::config::auto_memory_mb;
use crate::engine::download::{Downloader, Progress};
use crate::engine::java::{self, JavaInstall};
use crate::engine::meta::{Platform, VersionJson};
use crate::engine::Result;

pub async fn cached_javas(state: &AppState, force: bool) -> Vec<JavaInstall> {
    let mut cache = state.java_cache.lock().await;
    if force || cache.is_none() {
        *cache = Some(java::scan(vec![state.runtime_dir()]).await);
    }
    cache.clone().unwrap_or_default()
}

/// 按版本的 javaVersion 挑本机 Java，没有合适的就下载 Mojang 官方 runtime
pub async fn ensure_java(
    state: &AppState,
    version: &VersionJson,
    downloader: &Downloader,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    let platform = Platform::current();
    let major = version.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    if let Some(j) = java::pick(&cached_javas(state, false).await, major, &platform) {
        return Ok(j.path.clone());
    }
    let component = version
        .java_version
        .as_ref()
        .and_then(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".into());
    let path = java::install_runtime(downloader, &state.client, &state.runtime_dir(), &component, &platform, on_progress).await?;
    *state.java_cache.lock().await = None;
    Ok(path)
}

#[tauri::command]
pub async fn list_java(state: State<'_, AppState>, force: bool) -> Result<Vec<JavaInstall>> {
    Ok(cached_javas(&state, force).await)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_mb: u64,
    pub auto_mb: u32,
}

pub fn total_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

#[tauri::command]
pub async fn memory_info() -> Result<MemoryInfo> {
    let total_mb = total_memory_mb();
    Ok(MemoryInfo { total_mb, auto_mb: auto_memory_mb(total_mb) })
}
