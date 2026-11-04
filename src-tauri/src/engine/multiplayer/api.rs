//! 陶瓦的本机 HTTP 接口（只监听 127.0.0.1，全是 GET）和状态解析。
//!
//! `/state` 的内容来自陶瓦，里面的玩家名是房间里其他人填的：
//! 这里整理成我们自己的结构再交给界面，名字限长、去掉控制字符，地址和房间码不合格就丢掉。

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::room;
use crate::engine::{Error, Result};

const MAX_NAME: usize = 32;
const MAX_PROFILES: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    /// `HOST` 房主 / `LOCAL` 本机 / `GUEST` 其他人
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpState {
    /// waiting / host-scanning / host-starting / host-ok / guest-connecting / guest-starting / guest-ok / exception / unknown
    pub state: String,
    pub index: u64,
    pub room: Option<String>,
    pub profiles: Vec<Profile>,
    /// NAT 连接难度：UNKNOWN / EASIEST / SIMPLE / MEDIUM / TOUGH
    pub difficulty: Option<String>,
    /// 加入成功后的本机地址，如 `127.0.0.1:25566`
    pub address: Option<String>,
    /// exception 的类型（0~5）
    pub error: Option<u8>,
}

const STATES: &[&str] =
    &["waiting", "host-scanning", "host-starting", "host-ok", "guest-connecting", "guest-starting", "guest-ok", "exception"];
const DIFFICULTIES: &[&str] = &["UNKNOWN", "EASIEST", "SIMPLE", "MEDIUM", "TOUGH"];
const KINDS: &[&str] = &["HOST", "LOCAL", "GUEST"];

fn clean_name(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string()
}

/// `127.0.0.1` 或 `127.0.0.1:<端口>`，别的一律不认
fn clean_address(s: &str) -> Option<String> {
    let ok = match s.strip_prefix("127.0.0.1") {
        Some("") => true,
        Some(rest) => rest.strip_prefix(':').and_then(|p| p.parse::<u16>().ok()).is_some_and(|p| p > 0),
        None => false,
    };
    ok.then(|| s.to_string())
}

pub fn parse_state(v: &Value) -> MpState {
    let str_of = |k: &str| v.get(k).and_then(Value::as_str);
    let raw = str_of("state").unwrap_or("");
    let state = if STATES.contains(&raw) { raw } else { "unknown" };
    let profiles = v
        .get("profiles")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .take(MAX_PROFILES)
                .map(|p| Profile {
                    name: clean_name(p.get("name").and_then(Value::as_str).unwrap_or("")),
                    kind: p.get("kind").and_then(Value::as_str).filter(|k| KINDS.contains(k)).unwrap_or("GUEST").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    MpState {
        state: state.to_string(),
        index: v.get("index").and_then(Value::as_u64).unwrap_or(0),
        room: str_of("room").filter(|r| room::is_valid(r)).map(str::to_string),
        profiles,
        difficulty: str_of("difficulty").filter(|d| DIFFICULTIES.contains(d)).map(str::to_string),
        address: str_of("url").and_then(clean_address),
        error: (state == "exception").then(|| v.get("type").and_then(Value::as_u64).map(|t| t.min(255) as u8).unwrap_or(255)),
    }
}

/// 连本机陶瓦用的客户端：不走代理，超时短（对方就在本机，卡住说明它出问题了）
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("初始化 HTTP 客户端失败")
}

#[derive(Clone)]
pub struct Api {
    client: reqwest::Client,
    port: u16,
}

impl Api {
    pub fn new(client: reqwest::Client, port: u16) -> Self {
        Self { client, port }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<reqwest::Response> {
        let resp = self.client.get(self.url(path)).query(query).send().await?;
        if !resp.status().is_success() {
            return Err(Error::BadStatus { url: path.to_string(), status: resp.status().as_u16() });
        }
        Ok(resp)
    }

    /// 确认对面真的是陶瓦：返回它的版本号
    pub async fn meta(&self) -> Result<String> {
        let v: Value = self.get("/meta", &[]).await?.json().await?;
        v.get("easytier_version")
            .and(v.get("version"))
            .and_then(Value::as_str)
            .map(|s| s.chars().take(32).collect())
            .ok_or_else(|| Error::Other("本机端口上的程序不是陶瓦联机".into()))
    }

    pub async fn state(&self) -> Result<MpState> {
        Ok(parse_state(&self.get("/state", &[]).await?.json().await?))
    }

    pub async fn host(&self, player: Option<&str>) -> Result<()> {
        let q: Vec<(&str, &str)> = player.map(|p| ("player", p)).into_iter().collect();
        self.get("/state/scanning", &q).await.map(drop)
    }

    pub async fn join(&self, room: &str, player: Option<&str>) -> Result<()> {
        let mut q = vec![("room", room)];
        q.extend(player.map(|p| ("player", p)));
        self.get("/state/guesting", &q).await.map(drop).map_err(|e| match e {
            Error::BadStatus { status: 400, .. } => Error::coded("mp.joinRejected", &[], "陶瓦联机不接受这个房间码，或者当前已经在房间里了"),
            e => e,
        })
    }

    /// 回到待机（离开房间 / 关闭房间 / 清掉异常）
    pub async fn idle(&self) -> Result<()> {
        self.get("/state/ide", &[]).await.map(drop)
    }

    /// 让陶瓦自己退出
    pub async fn shutdown(&self) -> Result<()> {
        self.get("/panic", &[("peaceful", "true")]).await.map(drop)
    }
}

/// 传给陶瓦的显示名：用当前账号的角色名，整理一下
pub fn player_name(name: &str) -> Option<String> {
    Some(clean_name(name)).filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_host_ok_and_cleans_untrusted_fields() {
        let long = "a".repeat(100);
        let s = parse_state(&json!({
            "state": "host-ok", "index": 4, "room": "U/YNZE-U61D-2206-HXRG", "profile_index": 0,
            "profiles": [
                { "machine_id": "x", "name": "Steve", "vendor": "v", "kind": "HOST" },
                { "machine_id": "y", "name": format!("<b>\u{7}{long}"), "vendor": "v", "kind": "WHAT" },
            ]
        }));
        assert_eq!(s.state, "host-ok");
        assert_eq!(s.index, 4);
        assert_eq!(s.room.as_deref(), Some("U/YNZE-U61D-2206-HXRG"));
        assert_eq!(s.profiles[0], Profile { name: "Steve".into(), kind: "HOST".into() });
        assert_eq!(s.profiles[1].name.chars().count(), MAX_NAME);
        assert!(!s.profiles[1].name.contains('\u{7}'));
        assert_eq!(s.profiles[1].kind, "GUEST");
        assert_eq!(s.error, None);
    }

    #[test]
    fn parses_guest_and_exception_states() {
        let s = parse_state(&json!({ "state": "guest-ok", "index": 7, "url": "127.0.0.1:25566", "profiles": [] }));
        assert_eq!(s.address.as_deref(), Some("127.0.0.1:25566"));
        assert_eq!(parse_state(&json!({ "state": "guest-ok", "url": "127.0.0.1" })).address.as_deref(), Some("127.0.0.1"));
        assert_eq!(parse_state(&json!({ "state": "guest-ok", "url": "evil.com:1" })).address, None);
        assert_eq!(parse_state(&json!({ "state": "guest-ok", "url": "127.0.0.1:0" })).address, None);
        assert_eq!(parse_state(&json!({ "state": "guest-ok", "url": "127.0.0.1.evil.com" })).address, None);

        let s = parse_state(&json!({ "state": "guest-starting", "index": 6, "room": "U/YNZE-U61D-2206-HXRG", "difficulty": "TOUGH" }));
        assert_eq!(s.difficulty.as_deref(), Some("TOUGH"));
        assert_eq!(parse_state(&json!({ "state": "guest-starting", "difficulty": "HUH" })).difficulty, None);

        let s = parse_state(&json!({ "state": "exception", "index": 8, "type": 4 }));
        assert_eq!((s.state.as_str(), s.error), ("exception", Some(4)));
        assert_eq!(parse_state(&json!({ "state": "exception" })).error, Some(255));
    }

    #[test]
    fn unknown_state_and_bad_room_are_tolerated() {
        let s = parse_state(&json!({ "state": "future-thing", "index": 9, "room": "nope" }));
        assert_eq!(s.state, "unknown");
        assert_eq!(s.index, 9);
        assert_eq!(s.room, None);
        assert_eq!(parse_state(&json!(null)).state, "unknown");
    }

    #[test]
    fn player_name_is_cleaned() {
        assert_eq!(player_name(" Steve ").as_deref(), Some("Steve"));
        assert_eq!(player_name("\u{1}\u{2}"), None);
    }
}
