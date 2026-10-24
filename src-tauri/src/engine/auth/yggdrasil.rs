//! 外置登录：Yggdrasil API + authlib-injector。
//! 规范见 https://github.com/yushijinhun/authlib-injector/wiki

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use super::Secret;
use crate::engine::download::{DownloadTask, Downloader};
use crate::engine::install::fetch_json;
use crate::engine::{Error, Result};

const INJECTOR_LATEST: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";

#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// API 根地址，不带结尾斜杠
    pub api_root: String,
    pub name: String,
    /// 根地址返回的元数据原文，启动时 base64 后预先喂给 authlib-injector，省一次请求
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub struct YggSession {
    pub name: String,
    pub uuid: String,
    pub secret: Secret,
}

/// 用户输入的地址可能是皮肤站首页，按 ALI 头（X-Authlib-Injector-API-Location）找到真正的 API 根
pub async fn resolve_server(client: &reqwest::Client, input: &str) -> Result<ServerInfo> {
    let mut url = input.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("https://{url}");
    }
    let resp = client.get(&url).send().await?;
    if let Some(loc) = resp.headers().get("x-authlib-injector-api-location").and_then(|v| v.to_str().ok()) {
        let base = reqwest::Url::parse(&url).map_err(|e| Error::Other(e.to_string()))?;
        let api = base.join(loc).map_err(|e| Error::Other(e.to_string()))?.to_string();
        if api.trim_end_matches('/') != url.trim_end_matches('/') {
            url = api;
        }
    }
    let url = url.trim_end_matches('/').to_string();
    let metadata = client.get(&url).send().await?.text().await?;
    let v: Value = serde_json::from_str(&metadata)
        .map_err(|_| Error::Other("这个地址不是 Yggdrasil 认证服务器".into()))?;
    if v.get("meta").is_none() {
        return Err(Error::Other("这个地址不是 Yggdrasil 认证服务器".into()));
    }
    Ok(ServerInfo {
        name: v["meta"]["serverName"].as_str().unwrap_or("外置登录").to_string(),
        api_root: url,
        metadata,
    })
}

async fn post(client: &reqwest::Client, url: String, body: Value) -> Result<(u16, Value)> {
    let resp = client.post(url).json(&body).send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn ygg_error(status: u16, v: &Value) -> Error {
    let msg = v["errorMessage"].as_str().or(v["error"].as_str()).unwrap_or("未知错误");
    Error::Other(format!("外置登录失败（{status}）：{msg}"))
}

fn session_from(v: &Value, client_token: &str) -> Result<YggSession> {
    // 有多个角色时服务器可能不给 selectedProfile，先用第一个角色
    let profile = if v["selectedProfile"].is_object() { &v["selectedProfile"] } else { &v["availableProfiles"][0] };
    if !profile.is_object() {
        return Err(Error::Other("这个账号下还没有角色，请先去皮肤站创建".into()));
    }
    Ok(YggSession {
        name: profile["name"].as_str().unwrap_or_default().to_string(),
        uuid: profile["id"].as_str().unwrap_or_default().to_string(),
        secret: Secret {
            access_token: v["accessToken"].as_str().unwrap_or_default().to_string(),
            expires_at: 0,
            refresh_token: None,
            client_token: Some(client_token.to_string()),
        },
    })
}

/// 用账号密码登录。密码只用这一次，不会保存
pub async fn authenticate(client: &reqwest::Client, api_root: &str, username: &str, password: &str) -> Result<YggSession> {
    let client_token = uuid::Uuid::new_v4().simple().to_string();
    let (status, v) = post(
        client,
        format!("{api_root}/authserver/authenticate"),
        json!({
            "agent": {"name": "Minecraft", "version": 1},
            "username": username,
            "password": password,
            "clientToken": client_token,
            "requestUser": true
        }),
    )
    .await?;
    if status != 200 {
        return Err(ygg_error(status, &v));
    }
    session_from(&v, &client_token)
}

/// 令牌仍然有效返回 None；失效则刷新并返回新会话
pub async fn ensure_valid(client: &reqwest::Client, api_root: &str, secret: &Secret) -> Result<Option<YggSession>> {
    let client_token = secret.client_token.clone().unwrap_or_default();
    let (status, _) = post(
        client,
        format!("{api_root}/authserver/validate"),
        json!({"accessToken": secret.access_token, "clientToken": client_token}),
    )
    .await?;
    if status == 204 {
        return Ok(None);
    }
    let (status, v) = post(
        client,
        format!("{api_root}/authserver/refresh"),
        json!({"accessToken": secret.access_token, "clientToken": client_token, "requestUser": true}),
    )
    .await?;
    match status {
        200 => session_from(&v, &client_token).map(Some),
        _ => Err(Error::Other("外置登录已失效，请重新登录".into())),
    }
}

#[derive(Deserialize)]
struct InjectorArtifact {
    download_url: String,
}

/// 确保 authlib-injector.jar 存在（已存在就不重复下载）
pub async fn ensure_injector(downloader: &Downloader, client: &reqwest::Client, dir: &Path) -> Result<PathBuf> {
    let path = dir.join("authlib-injector.jar");
    if path.is_file() {
        return Ok(path);
    }
    let latest: InjectorArtifact = fetch_json(client, INJECTOR_LATEST, downloader.primary).await?;
    let report = downloader
        .download_all(
            vec![DownloadTask { url: latest.download_url, path: path.clone(), sha1: None, size: None }],
            std::sync::Arc::new(|_| {}),
        )
        .await;
    match report.failed.first() {
        Some(f) => Err(Error::Other(format!("下载 authlib-injector 失败：{}", f.reason))),
        None => Ok(path),
    }
}
