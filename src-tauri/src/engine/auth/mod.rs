//! 账号：离线、微软正版、外置登录（authlib-injector / Yggdrasil）。

pub mod microsoft;
pub mod offline;
pub mod skin;
pub mod store;
pub mod vault;
pub mod yggdrasil;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Offline,
    Microsoft,
    /// 外置登录（authlib-injector）
    Yggdrasil,
}

/// 给前端看的账号信息，不含任何令牌
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// 启动器内部 id
    pub id: String,
    pub kind: AccountKind,
    /// 游戏内名字
    pub name: String,
    /// 无横线的 UUID
    pub uuid: String,
    /// 外置登录：API 根地址
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// 外置登录：服务器显示名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
}

/// 加密存储的令牌
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    pub access_token: String,
    /// unix 秒；0 表示未知
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub client_token: Option<String>,
}

impl Secret {
    /// 提前 5 分钟视为过期
    pub fn is_expired(&self) -> bool {
        self.expires_at != 0 && now() + 300 >= self.expires_at
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 启动游戏时需要的身份信息
#[derive(Debug, Clone)]
pub struct LaunchAuth {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    /// msa / mojang / legacy
    pub user_type: &'static str,
    pub xuid: String,
}
