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
    /// 可翻译的错误：界面按 key（语言包里的 err.<key>）显示当前语言，日志等其他地方用中文原文 text
    #[error("{text}")]
    Coded {
        key: &'static str,
        args: Vec<(&'static str, String)>,
        text: String,
    },
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// 常见错误（启动、安装、登录）用它：`key` 对应前端语言包 `err.<key>`，`args` 是消息里的 {参数}，`text` 是中文原文
    pub fn coded(key: &'static str, args: &[(&'static str, &dyn std::fmt::Display)], text: impl Into<String>) -> Self {
        Error::Coded { key, args: args.iter().map(|(k, v)| (*k, v.to_string())).collect(), text: text.into() }
    }

    /// 发给前端的形式：(key, 参数)。没有 key 的错误返回 None，前端直接显示中文
    fn i18n(&self) -> Option<(&'static str, Vec<(&'static str, String)>)> {
        Some(match self {
            Error::Http(e) => ("http", vec![("detail", e.to_string())]),
            Error::Io(e) => ("io", vec![("detail", e.to_string())]),
            Error::Json(e) => ("json", vec![("detail", e.to_string())]),
            Error::VersionNotFound(id) => ("versionNotFound", vec![("id", id.clone())]),
            Error::HashMismatch { path, expected, actual } => {
                ("hashMismatch", vec![("path", path.clone()), ("expected", expected.clone()), ("actual", actual.clone())])
            }
            Error::BadStatus { url, status } => ("badStatus", vec![("url", url.clone()), ("status", status.to_string())]),
            Error::Coded { key, args, .. } => (*key, args.clone()),
            Error::Other(_) => return None,
        })
    }
}

// 前端收到的是一个字符串：可翻译的错误是 JSON {"i18n":"err.<key>","args":{...},"text":"中文原文"}（src/i18n/index.ts 的 errText 解析），
// 其余是可读的中文错误信息
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self.i18n() {
            Some((key, args)) => {
                let args: serde_json::Map<String, serde_json::Value> = args.into_iter().map(|(k, v)| (k.to_string(), v.into())).collect();
                // 手动拼，保证 "i18n" 在最前面（serde_json 的对象会按字母排序，前端靠这个前缀识别）
                let json = format!(
                    "{{\"i18n\":{},\"args\":{},\"text\":{}}}",
                    serde_json::Value::from(format!("err.{key}")),
                    serde_json::Value::Object(args),
                    serde_json::Value::from(self.to_string())
                );
                s.serialize_str(&json)
            }
            None => s.serialize_str(&self.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(e: &Error) -> String {
        serde_json::from_str::<String>(&serde_json::to_string(e).unwrap()).unwrap()
    }

    #[test]
    fn coded_errors_reach_the_ui_as_json_with_chinese_fallback() {
        let e = Error::coded("launch.filesFailed", &[("n", &3), ("path", &"a.jar")], "3 个文件补全失败");
        assert_eq!(e.to_string(), "3 个文件补全失败", "日志里还是中文原文");
        let v: serde_json::Value = serde_json::from_str(&wire(&e)).unwrap();
        assert_eq!(v["i18n"], "err.launch.filesFailed");
        assert_eq!(v["args"]["n"], "3");
        assert_eq!(v["args"]["path"], "a.jar");
        assert_eq!(v["text"], "3 个文件补全失败");
        assert!(wire(&e).starts_with("{\"i18n\":"), "前端按这个前缀识别");
    }

    #[test]
    fn generic_variants_are_translatable_and_other_stays_plain() {
        let v: serde_json::Value = serde_json::from_str(&wire(&Error::VersionNotFound("1.21".into()))).unwrap();
        assert_eq!(v["i18n"], "err.versionNotFound");
        assert_eq!(v["args"]["id"], "1.21");
        assert_eq!(wire(&Error::Other("出错了".into())), "出错了");
    }
}
