//! 联机（陶瓦联机）。陶瓦进程按需启动，后端轮询它的 `/state`，变化时推送 `multiplayer-state` 事件。
//!
//! 陶瓦全局只有一个实例、一个状态：别的程序也可能在用同一个实例开房间。
//! 所以启动器退出时，只关我们自己拉起、而且没有别人房间在用的实例（见 `shutdown`）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::loaders::job_reporter;
use super::AppState;
use crate::engine::download::Downloader;
use crate::engine::install::Stage;
use crate::engine::meta::Platform;
use crate::engine::multiplayer::api::{self, Api, MpState};
use crate::engine::multiplayer::{process, release, room};
use crate::engine::{Error, Result};

pub const MP_STATE_EVENT: &str = "multiplayer-state";
const POLL: Duration = Duration::from_millis(700);
/// 连续这么多次连不上，就认为陶瓦已经退出
const MAX_FAILS: u32 = 5;

pub struct MpSession {
    api: Api,
    process: process::Process,
    /// 当前房间是我们开或加入的
    ours: Arc<AtomicBool>,
    watcher: tauri::async_runtime::JoinHandle<()>,
    alive: Arc<AtomicBool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpStatus {
    /// 这个平台 / 发行版本能不能用联机
    supported: bool,
    installed: bool,
    version: &'static str,
    running: bool,
    state: Option<MpState>,
}

fn stopped_state() -> MpState {
    MpState { state: "stopped".into(), index: 0, room: None, profiles: vec![], difficulty: None, address: None, error: None }
}

fn check_edition() -> Result<()> {
    if crate::edition::STORE {
        return Err(Error::coded("mp.store", &[], "微软商店版暂时不提供联机功能"));
    }
    Ok(())
}

#[tauri::command]
pub async fn mp_status(state: State<'_, AppState>) -> Result<MpStatus> {
    let platform = Platform::current();
    let supported = !crate::edition::STORE && release::asset(&platform).is_some();
    let installed = supported && release::installed_exe(&state.runtime_dir(), &platform).is_some();
    let session = state.multiplayer.lock().await;
    let live = session.as_ref().filter(|s| s.alive.load(Ordering::SeqCst));
    let current = match live {
        Some(s) => s.api.state().await.ok(),
        None => None,
    };
    Ok(MpStatus { supported, installed, version: release::VERSION, running: current.is_some(), state: current })
}

#[tauri::command]
pub async fn mp_install(app: AppHandle, state: State<'_, AppState>, job_id: String) -> Result<()> {
    check_edition()?;
    let platform = Platform::current();
    if release::asset(&platform).is_none() {
        return Err(release::unsupported(&platform));
    }
    if !state.installing.lock().await.insert(job_id.clone()) {
        return Err(Error::coded("mp.installBusy", &[], "陶瓦联机已经在下载了，别急"));
    }
    let downloader = Downloader::new(state.client.clone(), state.source().await, 1);
    let report = job_reporter(&app, &job_id);
    let job = state.cancellable(&job_id);
    let result = job.run(release::install(&downloader, &state.runtime_dir(), &platform, Arc::new(move |p| report(Stage::Files, p)))).await;
    state.installing.lock().await.remove(&job_id);
    result.map(drop)
}

/// 陶瓦没在跑就启动它，并开始轮询状态
async fn ensure_running(app: &AppHandle, state: &AppState) -> Result<Api> {
    let mut session = state.multiplayer.lock().await;
    if let Some(s) = session.as_ref().filter(|s| s.alive.load(Ordering::SeqCst)) {
        return Ok(s.api.clone());
    }
    let exe = release::installed_exe(&state.runtime_dir(), &Platform::current())
        .ok_or_else(|| Error::coded("mp.notInstalled", &[], "还没有下载陶瓦联机，请先在联机页启用"))?;
    let client = api::client();
    let process = process::start(&exe, &state.data_dir.join("terracotta"), client.clone()).await?;
    let api = Api::new(client, process.port);
    let alive = Arc::new(AtomicBool::new(true));
    let watcher = tauri::async_runtime::spawn(watch(app.clone(), api.clone(), alive.clone()));
    *session = Some(MpSession { api: api.clone(), process, ours: Arc::new(AtomicBool::new(false)), watcher, alive });
    Ok(api)
}

async fn watch(app: AppHandle, api: Api, alive: Arc<AtomicBool>) {
    let mut last: Option<u64> = None;
    let mut fails = 0;
    loop {
        match api.state().await {
            Ok(s) => {
                fails = 0;
                // 陶瓦的 index 单调递增，没变大就是没变化
                if last.is_none_or(|i| s.index > i) {
                    last = Some(s.index);
                    let _ = app.emit(MP_STATE_EVENT, s);
                }
            }
            Err(_) => {
                fails += 1;
                if fails >= MAX_FAILS {
                    alive.store(false, Ordering::SeqCst);
                    let _ = app.emit(MP_STATE_EVENT, stopped_state());
                    return;
                }
            }
        }
        tokio::time::sleep(POLL).await;
    }
}

/// 当前账号的角色名，作为在房间里显示的名字（只传名字，不传令牌和 UUID）
async fn player(state: &AppState) -> Option<String> {
    state.accounts.lock().await.selected().and_then(|a| api::player_name(&a.name))
}

/// 陶瓦只在待机时接受开房 / 加入；停在异常状态时先清掉
async fn ready(api: &Api) -> Result<()> {
    match api.state().await?.state.as_str() {
        "waiting" => Ok(()),
        "exception" => api.idle().await,
        _ => Err(Error::coded("mp.busy", &[], "已经在房间里了，先离开当前房间")),
    }
}

async fn mark_ours(state: &AppState) {
    if let Some(s) = state.multiplayer.lock().await.as_ref() {
        s.ours.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn mp_host(app: AppHandle, state: State<'_, AppState>) -> Result<MpState> {
    check_edition()?;
    let api = ensure_running(&app, &state).await?;
    ready(&api).await?;
    api.host(player(&state).await.as_deref()).await?;
    mark_ours(&state).await;
    api.state().await
}

#[tauri::command]
pub async fn mp_join(app: AppHandle, state: State<'_, AppState>, room: String) -> Result<MpState> {
    check_edition()?;
    let code = room::normalize(&room).ok_or_else(|| Error::coded("mp.badRoom", &[], "房间码不对：格式是 U/XXXX-XXXX-XXXX-XXXX，请检查有没有输错"))?;
    let api = ensure_running(&app, &state).await?;
    ready(&api).await?;
    api.join(&code, player(&state).await.as_deref()).await?;
    mark_ours(&state).await;
    api.state().await
}

/// 离开 / 关闭房间，或者清掉异常，回到待机
#[tauri::command]
pub async fn mp_leave(state: State<'_, AppState>) -> Result<()> {
    let session = state.multiplayer.lock().await;
    let Some(s) = session.as_ref().filter(|s| s.alive.load(Ordering::SeqCst)) else { return Ok(()) };
    s.ours.store(false, Ordering::SeqCst);
    s.api.idle().await
}

/// 启动器退出时调用
pub async fn shutdown(state: &AppState) {
    let Some(s) = state.multiplayer.lock().await.take() else { return };
    s.watcher.abort();
    if !s.alive.load(Ordering::SeqCst) {
        return;
    }
    let ours = s.ours.load(Ordering::SeqCst);
    let busy = s.api.state().await.is_ok_and(|st| !matches!(st.state.as_str(), "waiting" | "exception"));
    if s.process.owned && (ours || !busy) {
        s.process.stop(&s.api).await;
    } else if ours {
        let _ = s.api.idle().await;
    }
}
