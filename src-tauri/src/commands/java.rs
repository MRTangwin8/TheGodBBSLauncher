use serde::Serialize;
use tauri::State;

use super::AppState;
use crate::config::auto_memory_mb;
use crate::engine::java::{self, JavaInstall};
use crate::engine::Result;

pub async fn cached_javas(state: &AppState, force: bool) -> Vec<JavaInstall> {
    let mut cache = state.java_cache.lock().await;
    if force || cache.is_none() {
        *cache = Some(java::scan(vec![state.runtime_dir()]).await);
    }
    cache.clone().unwrap_or_default()
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
