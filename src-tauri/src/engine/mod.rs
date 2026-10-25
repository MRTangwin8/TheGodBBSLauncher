//! 启动器核心：和 Tauri 无关的纯逻辑，方便单元测试和以后复用。

pub mod download;
pub mod error;
pub mod install;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loader;
pub mod auth;
pub mod crash;
pub mod meta;
pub mod mirror;
pub mod modpack;
pub mod resources;
pub mod theme;
mod http;

pub use error::{Error, Result};

/// 统一的 HTTP 客户端。BMCLAPI 要求带上能识别启动器的 UA。
pub fn http_client() -> reqwest::Client {
    http_client_builder()
        .build()
        .expect("初始化 HTTP 客户端失败")
}

fn http_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(concat!("GodBBSLauncher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(30))
}
