use std::sync::atomic::Ordering;

use tauri::State;

use super::AppState;
use crate::engine::auth::microsoft::{self, DeviceCode};
use crate::engine::auth::store::AccountList;
use crate::engine::auth::{offline, yggdrasil, Account, AccountKind};
use crate::engine::{Error, Result};

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<AccountList> {
    Ok(state.accounts.lock().await.list())
}

#[tauri::command]
pub async fn add_offline_account(state: State<'_, AppState>, name: String) -> Result<Account> {
    let name = name.trim().to_string();
    if !offline::is_valid_name(&name) {
        return Err(Error::Other("离线名字需要 3~16 位，只能用英文字母、数字和下划线".into()));
    }
    let account = Account {
        id: new_id(),
        kind: AccountKind::Offline,
        uuid: offline::offline_uuid(&name),
        name,
        server: None,
        server_name: None,
    };
    state.accounts.lock().await.upsert(account, None)
}

#[tauri::command]
pub async fn microsoft_login_start(state: State<'_, AppState>) -> Result<DeviceCode> {
    let dc = microsoft::request_device_code(&state.client).await?;
    state.login_cancel.store(false, Ordering::Relaxed);
    *state.pending_login.lock().await = Some(dc.clone());
    Ok(dc)
}

/// 阻塞到用户在浏览器里完成授权（或过期 / 取消）
#[tauri::command]
pub async fn microsoft_login_finish(state: State<'_, AppState>) -> Result<Account> {
    let dc = state
        .pending_login
        .lock()
        .await
        .take()
        .ok_or_else(|| Error::Other("请先获取登录验证码".into()))?;
    let session = microsoft::poll_device_code(&state.client, &dc, &state.login_cancel).await?;
    let account = Account {
        id: new_id(),
        kind: AccountKind::Microsoft,
        name: session.name,
        uuid: session.uuid,
        server: None,
        server_name: None,
    };
    state.accounts.lock().await.upsert(account, Some(&session.secret))
}

#[tauri::command]
pub async fn microsoft_login_cancel(state: State<'_, AppState>) -> Result<()> {
    state.login_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn add_yggdrasil_account(
    state: State<'_, AppState>,
    server: String,
    username: String,
    password: String,
) -> Result<Account> {
    let info = yggdrasil::resolve_server(&state.client, &server).await?;
    let session = yggdrasil::authenticate(&state.client, &info.api_root, &username, &password).await?;
    let account = Account {
        id: new_id(),
        kind: AccountKind::Yggdrasil,
        name: session.name,
        uuid: session.uuid,
        server: Some(info.api_root),
        server_name: Some(info.name),
    };
    state.accounts.lock().await.upsert(account, Some(&session.secret))
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Result<()> {
    state.accounts.lock().await.remove(&id)
}

#[tauri::command]
pub async fn select_account(state: State<'_, AppState>, id: String) -> Result<()> {
    state.accounts.lock().await.select(&id)
}
