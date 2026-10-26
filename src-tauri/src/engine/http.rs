use std::sync::OnceLock;

use reqwest::{header::HeaderValue, Client, Url};

use super::{Error, Result};

/// 搜索这类请求最多等多久。对方卡住时给出明确提示，别让界面一直转圈
pub(crate) const SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) async fn within<T>(limit: std::time::Duration, fut: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(limit, fut)
        .await
        .map_err(|_| Error::Other(format!("请求超时（超过 {} 秒没有响应），请稍后重试或换一个来源", limit.as_secs())))?
}

pub(crate) fn credential_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        super::http_client_builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("初始化凭据 HTTP 客户端失败")
    })
}

pub(crate) fn api_key_header(key: &str) -> Result<HeaderValue> {
    let mut value = HeaderValue::from_str(key.trim())
        .map_err(|_| Error::Other("CurseForge API Key 格式无效，请检查是否混入换行或其他字符".into()))?;
    value.set_sensitive(true);
    Ok(value)
}

pub(crate) fn curseforge_download(url: &Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host_str(), Some("edge.forgecdn.net" | "mediafilez.forgecdn.net"))
        && url.path().starts_with("/files/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn slow_requests_give_up_with_a_clear_message() {
        let slow = within(std::time::Duration::from_millis(30), std::future::pending::<Result<()>>()).await;
        let message = slow.unwrap_err().to_string();
        assert!(message.contains("请求超时") && message.contains("换一个来源"), "{message}");
        // 没超时就原样返回结果和错误
        assert_eq!(within(std::time::Duration::from_secs(5), async { Ok(7) }).await.unwrap(), 7);
        assert!(within(std::time::Duration::from_secs(5), async { Err::<(), _>(Error::Other("boom".into())) }).await.unwrap_err().to_string().contains("boom"));
    }

    #[test]
    fn restricts_download_credentials_to_official_https_file_endpoints() {
        for url in [
            "https://edge.forgecdn.net/files/1/2/mod.jar",
            "https://mediafilez.forgecdn.net/files/1/2/mod.jar",
        ] {
            assert!(curseforge_download(&Url::parse(url).unwrap()), "{url}");
        }
        for url in [
            "http://edge.forgecdn.net/files/1/2/mod.jar",
            "https://edge.forgecdn.net.evil.test/files/1/2/mod.jar",
            "https://evil.test/edge.forgecdn.net/files/mod.jar",
            "https://mod.mcimirror.top/files/1/2/mod.jar",
            "https://cdn.modrinth.com/data/mod.jar",
            "https://edge.forgecdn.net:8443/files/1/2/mod.jar",
            "https://user@edge.forgecdn.net/files/1/2/mod.jar",
            "https://edge.forgecdn.net/not-files/mod.jar",
        ] {
            assert!(!curseforge_download(&Url::parse(url).unwrap()), "{url}");
        }
    }

    #[test]
    fn api_keys_are_sensitive_and_invalid_values_do_not_appear_in_errors() {
        let key = api_key_header("  local-test-key  ").unwrap();
        assert!(key.is_sensitive());
        assert_eq!(key.to_str().unwrap(), "local-test-key");
        let error = api_key_header("secret-value\r\ninjected: true").unwrap_err().to_string();
        assert!(!error.contains("secret-value"));
    }
}
