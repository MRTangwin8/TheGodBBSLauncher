//! 微软正版登录：设备码 → MSA 令牌 → Xbox Live → XSTS → Minecraft 令牌 → 档案。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{now, Secret};
use crate::engine::{Error, Result};

/// Entra 应用注册的客户端 ID（公共客户端，无密钥）
pub const CLIENT_ID: &str = "53251486-3a07-436d-935d-43a1d2273bff";
const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: String,
}

#[derive(Debug, Deserialize)]
struct MsToken {
    access_token: String,
    refresh_token: Option<String>,
}

/// 登录成功后的结果
#[derive(Debug, Clone)]
pub struct McSession {
    pub name: String,
    pub uuid: String,
    pub secret: Secret,
}

fn oauth_error(e: OAuthError) -> Error {
    // AADSTS70002：Entra 里没开"允许公共客户端流"
    let hint = if e.error_description.contains("AADSTS70002") {
        "（请在 Entra 应用的「身份验证」里开启“允许公共客户端流”）"
    } else {
        ""
    };
    Error::Other(format!("微软登录失败：{} {}{hint}", e.error, e.error_description.lines().next().unwrap_or("")))
}

pub async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCode> {
    let resp = client
        .post(format!("{AUTHORITY}/devicecode"))
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(oauth_error(resp.json().await?));
    }
    Ok(resp.json().await?)
}

/// 轮询直到用户在浏览器里完成授权；`cancel` 置位则放弃
pub async fn poll_device_code(client: &reqwest::Client, dc: &DeviceCode, cancel: &AtomicBool) -> Result<McSession> {
    let deadline = Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = dc.interval.max(1);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Other("已取消登录".into()));
        }
        if Instant::now() > deadline {
            return Err(Error::Other("验证码已过期，请重新登录".into()));
        }
        let resp = client
            .post(format!("{AUTHORITY}/token"))
            .form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &dc.device_code),
            ])
            .send()
            .await?;
        if resp.status().is_success() {
            let token: MsToken = resp.json().await?;
            return login_minecraft(client, &token).await;
        }
        let err: OAuthError = resp.json().await?;
        match err.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "authorization_declined" => return Err(Error::Other("你在浏览器里拒绝了授权".into())),
            "expired_token" => return Err(Error::Other("验证码已过期，请重新登录".into())),
            _ => return Err(oauth_error(err)),
        }
    }
}

/// 用 MSA refresh token 重新走一遍完整链路
pub async fn refresh(client: &reqwest::Client, refresh_token: &str) -> Result<McSession> {
    let resp = client
        .post(format!("{AUTHORITY}/token"))
        .form(&[
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(Error::Other(format!(
            "正版登录已失效，请重新登录（{}）",
            resp.json::<OAuthError>().await.map(|e| e.error).unwrap_or_default()
        )));
    }
    let token: MsToken = resp.json().await?;
    let mut session = login_minecraft(client, &token).await?;
    // 微软有时不返回新的 refresh token，沿用旧的
    session.secret.refresh_token.get_or_insert_with(|| refresh_token.to_string());
    Ok(session)
}

async fn post_json(client: &reqwest::Client, url: &str, body: Value) -> Result<(u16, Value)> {
    let resp = client.post(url).header("Accept", "application/json").json(&body).send().await?;
    let status = resp.status().as_u16();
    let text = resp.text().await?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn xsts_error(xerr: u64) -> String {
    match xerr {
        2148916233 => "这个微软账号还没有 Xbox 档案，请先登录一次 xbox.com 创建".into(),
        2148916235 => "Xbox Live 在你所在的国家/地区不可用".into(),
        2148916236 | 2148916237 => "该账号需要在 Xbox 官网完成成人验证（韩国地区）".into(),
        2148916238 => "这是儿童账号，需要家长把它加入家庭组后才能登录".into(),
        other => format!("XSTS 授权失败（XErr {other}）"),
    }
}

async fn login_minecraft(client: &reqwest::Client, ms: &MsToken) -> Result<McSession> {
    // 1. Xbox Live
    let (status, xbl) = post_json(
        client,
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", ms.access_token)},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| Error::Other(format!("Xbox Live 登录失败（HTTP {status}）")))?;

    // 2. XSTS
    let (status, xsts) = post_json(
        client,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    if let Some(xerr) = xsts["XErr"].as_u64() {
        return Err(Error::Other(xsts_error(xerr)));
    }
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| Error::Other(format!("XSTS 授权失败（HTTP {status}）")))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap_or_default();

    // 3. Minecraft
    let (status, mc) = post_json(
        client,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}),
    )
    .await?;
    let mc_token = mc["access_token"].as_str().ok_or_else(|| {
        if status == 403 {
            Error::Other("Minecraft 服务拒绝了登录：Entra 应用可能还没获得 Minecraft API 权限".into())
        } else {
            Error::Other(format!("Minecraft 登录失败（HTTP {status}）"))
        }
    })?;
    let expires_in = mc["expires_in"].as_u64().unwrap_or(86400);

    // 4. 档案（没买游戏会 404）
    let resp = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?;
    if resp.status().as_u16() == 404 {
        return Err(Error::Other("这个微软账号没有购买 Minecraft Java 版（或还没创建游戏档案）".into()));
    }
    if !resp.status().is_success() {
        return Err(Error::Other(format!("获取游戏档案失败（HTTP {}）", resp.status().as_u16())));
    }
    let profile: Value = resp.json().await?;
    Ok(McSession {
        name: profile["name"].as_str().unwrap_or_default().to_string(),
        uuid: profile["id"].as_str().unwrap_or_default().to_string(),
        secret: Secret {
            access_token: mc_token.to_string(),
            expires_at: now() + expires_in,
            refresh_token: ms.refresh_token.clone(),
            client_token: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_code_serializes_camel_case() {
        let dc: DeviceCode = serde_json::from_str(
            r#"{"user_code":"ABCD1234","device_code":"x","verification_uri":"https://www.microsoft.com/link","expires_in":900,"interval":5,"message":"..."}"#,
        ).unwrap();
        let v = serde_json::to_value(&dc).unwrap();
        assert_eq!(v["userCode"], "ABCD1234");
        assert_eq!(v["verificationUri"], "https://www.microsoft.com/link");
    }

    #[test]
    fn explains_xsts_errors() {
        assert!(xsts_error(2148916238).contains("儿童"));
        assert!(xsts_error(1).contains("XErr 1"));
    }
}
