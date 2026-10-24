//! 读取玩家皮肤：会话服务器的档案里 textures 属性是 base64 的 JSON，里面有皮肤地址和模型（slim / 默认）。
//! 正版走 Mojang 会话服务器，外置登录走认证服务器自己的 sessionserver。

use std::path::Path;
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::{Error, Result};

const MOJANG_SESSION: &str = "https://sessionserver.mojang.com/session/minecraft/profile";
/// 缓存多久后重新拉取
const CACHE_TTL: Duration = Duration::from_secs(6 * 3600);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// PNG 的 data URL
    pub data_url: String,
    /// Alex 式细手臂
    pub slim: bool,
}

#[derive(Serialize, Deserialize)]
struct CacheMeta {
    slim: bool,
}

/// 从档案 JSON 里取出 (皮肤地址, 是否细手臂)
pub fn parse_textures(profile: &Value) -> Option<(String, bool)> {
    let value = profile["properties"].as_array()?.iter().find(|p| p["name"] == "textures")?["value"].as_str()?;
    let decoded: Value = serde_json::from_slice(&B64.decode(value).ok()?).ok()?;
    let skin = &decoded["textures"]["SKIN"];
    let url = skin["url"].as_str()?.replace("http://", "https://");
    Some((url, skin["metadata"]["model"] == "slim"))
}

fn session_url(api_root: Option<&str>, uuid: &str) -> String {
    match api_root {
        Some(api) => format!("{}/sessionserver/session/minecraft/profile/{uuid}", api.trim_end_matches('/')),
        None => format!("{MOJANG_SESSION}/{uuid}"),
    }
}

async fn download(client: &reqwest::Client, api_root: Option<&str>, uuid: &str) -> Result<Option<(Vec<u8>, bool)>> {
    let resp = client.get(session_url(api_root, uuid)).send().await?;
    // 204：档案存在但没有皮肤
    if resp.status().as_u16() == 204 || resp.status().as_u16() == 404 {
        return Ok(None);
    }
    let profile: Value = resp.error_for_status()?.json().await?;
    let Some((url, slim)) = parse_textures(&profile) else { return Ok(None) };
    let bytes = client.get(&url).send().await?.error_for_status()?.bytes().await?;
    if !bytes.starts_with(b"\x89PNG") {
        return Err(Error::Other("皮肤文件不是 PNG".into()));
    }
    Ok(Some((bytes.to_vec(), slim)))
}

fn to_skin(bytes: &[u8], slim: bool) -> Skin {
    Skin { data_url: format!("data:image/png;base64,{}", B64.encode(bytes)), slim }
}

/// 取皮肤：缓存新鲜就直接用；否则联网拉取，失败时退回旧缓存
pub async fn fetch(client: &reqwest::Client, cache_dir: &Path, api_root: Option<&str>, uuid: &str) -> Result<Option<Skin>> {
    let safe: String = uuid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let png = cache_dir.join(format!("{safe}.png"));
    let meta = cache_dir.join(format!("{safe}.json"));
    let cached = || -> Option<Skin> {
        let bytes = std::fs::read(&png).ok()?;
        let m: CacheMeta = serde_json::from_str(&std::fs::read_to_string(&meta).ok()?).ok()?;
        Some(to_skin(&bytes, m.slim))
    };
    let fresh = png.metadata().and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < CACHE_TTL);
    if fresh {
        if let Some(s) = cached() {
            return Ok(Some(s));
        }
    }
    match download(client, api_root, &safe).await {
        Ok(Some((bytes, slim))) => {
            std::fs::create_dir_all(cache_dir)?;
            std::fs::write(&png, &bytes)?;
            std::fs::write(&meta, serde_json::to_string(&CacheMeta { slim })?)?;
            Ok(Some(to_skin(&bytes, slim)))
        }
        Ok(None) => Ok(None),
        Err(e) => cached().map(Some).ok_or(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_textures_property() {
        let textures = r#"{"timestamp":1,"profileId":"x","profileName":"Alex","textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#;
        let profile = serde_json::json!({"id": "x", "name": "Alex", "properties": [{"name": "textures", "value": B64.encode(textures)}]});
        assert_eq!(parse_textures(&profile), Some(("https://textures.minecraft.net/texture/abc".into(), true)));

        let classic = r#"{"textures":{"SKIN":{"url":"https://t/abc"}}}"#;
        let profile = serde_json::json!({"properties": [{"name": "textures", "value": B64.encode(classic)}]});
        assert_eq!(parse_textures(&profile), Some(("https://t/abc".into(), false)));

        assert_eq!(parse_textures(&serde_json::json!({"properties": []})), None);
    }

    #[test]
    fn session_urls() {
        assert_eq!(session_url(None, "abc"), "https://sessionserver.mojang.com/session/minecraft/profile/abc");
        assert_eq!(session_url(Some("https://littleskin.cn/api/yggdrasil/"), "abc"), "https://littleskin.cn/api/yggdrasil/sessionserver/session/minecraft/profile/abc");
    }
}
