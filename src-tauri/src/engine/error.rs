use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("网络请求失败：{0}")]
    Http(#[from] reqwest::Error),
    #[error("文件读写失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 解析失败：{0}")]
    Json(#[from] serde_json::Error),
    #[error("找不到版本 {0}")]
    VersionNotFound(String),
    #[error("校验失败：{path} 期望 SHA1 {expected}，实际 {actual}")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("服务器返回 {status}：{url}")]
    BadStatus { url: String, status: u16 },
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

// 前端只需要一段可读的中文错误信息
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
