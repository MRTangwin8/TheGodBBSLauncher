//! 一键分享日志：上传到 mclo.gs（专门给 Minecraft 日志用的公开粘贴站），拿到一个链接发到论坛求助。
//!
//! 上传前先整理：启动器显示的日志里令牌已经打过码（`LogRedactor`），这里再把用户目录换成 `~`
//! （路径里常带着系统用户名）；太长时只保留结尾（崩溃信息在最后），不超过 mclo.gs 的上限。
//! 上传的内容任何拿到链接的人都能看到，界面上传前会让用户确认。

use std::path::Path;

use serde::Deserialize;

use crate::engine::{Error, Result};

const ENDPOINT: &str = "https://api.mclo.gs/1/log";
const SITE: &str = "https://mclo.gs/";
/// mclo.gs 的上限是 10 MiB / 25000 行，留点余量
const MAX_LINES: usize = 24_000;
const MAX_BYTES: usize = 9 * 1024 * 1024;

/// 换掉用户目录、截到上限以内
pub fn prepare(text: &str, home: Option<&Path>) -> String {
    let mut text = text.to_string();
    if let Some(home) = home.map(|h| h.display().to_string()).filter(|h| h.len() > 3) {
        for form in [home.clone(), home.replace('\\', "/"), home.replace('/', "\\"), home.replace('\\', "\\\\")] {
            text = text.replace(&form, "~");
        }
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut bytes = 0;
    for line in lines.iter().rev() {
        if kept.len() >= MAX_LINES || bytes + line.len() + 1 > MAX_BYTES {
            break;
        }
        bytes += line.len() + 1;
        kept.push(line);
    }
    kept.reverse();
    let dropped = lines.len() - kept.len();
    let mut out = String::new();
    if dropped > 0 {
        out.push_str(&format!("[神权] 日志太长，前 {dropped} 行没有上传\n"));
    }
    out.push_str(&kept.join("\n"));
    out
}

#[derive(Deserialize)]
struct Response {
    success: bool,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// 上传，返回分享链接
pub async fn upload(client: &reqwest::Client, text: &str) -> Result<String> {
    if text.trim().is_empty() {
        return Err(Error::coded("share.empty", &[], "日志是空的，没有可分享的内容"));
    }
    let resp = client.post(ENDPOINT).form(&[("content", text)]).send().await?;
    let status = resp.status().as_u16();
    let body: Response = resp.json().await.map_err(|_| Error::BadStatus { url: ENDPOINT.into(), status })?;
    match (body.success, body.url) {
        // 只认 mclo.gs 自己的链接，链接会显示给用户点
        (true, Some(url)) if url.starts_with(SITE) && url[SITE.len()..].chars().all(|c| c.is_ascii_alphanumeric()) => Ok(url),
        (_, _) => {
            let reason = body.error.unwrap_or_else(|| format!("HTTP {status}")).chars().take(200).collect::<String>();
            Err(Error::coded("share.failed", &[("reason", &reason)], format!("日志上传失败：{reason}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_home_dir_in_every_spelling() {
        let home = Path::new(r"C:\Users\Steve");
        let text = "at C:\\Users\\Steve\\.minecraft\nat C:/Users/Steve/x\nescaped C:\\\\Users\\\\Steve\\\\y";
        let out = prepare(text, Some(home));
        assert!(!out.contains("Steve"), "{out}");
        assert!(out.contains("~\\.minecraft"));
    }

    #[test]
    fn keeps_the_tail_when_too_long() {
        let text: String = (0..30_000).map(|i| format!("line {i}\n")).collect();
        let out = prepare(&text, None);
        assert!(out.starts_with("[神权] 日志太长，前 6000 行没有上传"));
        assert!(out.ends_with("line 29999"));
        assert_eq!(out.lines().count(), MAX_LINES + 1);
    }
}
