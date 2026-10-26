//! 启动器核心：和 Tauri 无关的纯逻辑，方便单元测试和以后复用。

pub mod download;
pub mod error;
pub mod install;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loader;
pub mod announce;
pub mod auth;
pub mod crash;
pub mod meta;
pub mod mirror;
pub mod modpack;
pub mod resources;
pub mod theme;
pub mod update;
pub(crate) mod http;

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

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// gzip 压缩的 `{"ok":true}`
    const GZIP: [u8; 31] = [
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0xca, 0xcf, 0x56, 0xb2, 0x2a, 0x29, 0x2a, 0x4d, 0xad, 0x05, 0x00, 0x90,
        0x5f, 0xd4, 0xa7, 0x0b, 0x00, 0x00, 0x00,
    ];

    /// 接口请求会声明支持压缩，并能自动解压。KLPBBS / CurseForge 镜像的搜索结果不压缩有几百 KB，慢网络下要等十几秒
    #[tokio::test]
    async fn api_client_accepts_and_decodes_gzip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                GZIP.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(&GZIP).await.unwrap();
            request
        });
        let client = super::http_client_builder().no_proxy().build().unwrap();
        let value: serde_json::Value = client.get(&url).send().await.unwrap().json().await.unwrap();
        assert_eq!(value["ok"], true);
        assert!(server.await.unwrap().contains("accept-encoding: gzip"));
    }
}
