//! 公告栏：从 GitHub 仓库里的 `announcements.json` 拉公告。
//!
//! 启动器没有服务器，所以"推送"的做法是：维护者把公告提交推送到仓库，启动器启动时和运行期间定期去拉。
//! 发公告不用发新版本。文件里的内容当作不可信数据处理：纯文本显示，链接只放行 http(s)，条数和长度都有上限。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::update::{version_in_range, REPO};
use super::{Error, Result};
use crate::config::clean_text;

const MAX_ITEMS: usize = 10;
const MAX_FILE: usize = 64 * 1024;
const LEVELS: &[&str] = &["info", "notice", "warn"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    /// YYYY-MM-DD，可以为空
    pub date: String,
    /// info / notice / warn
    pub level: String,
    pub pinned: bool,
    pub link: Option<String>,
    pub link_text: String,
    /// YYYY-MM-DD；这天之后不再显示
    pub expires: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawFile {
    announcements: Vec<RawItem>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawItem {
    id: String,
    title: String,
    body: String,
    date: String,
    level: String,
    pinned: bool,
    link: String,
    link_text: String,
    expires: String,
    min_version: String,
    max_version: String,
}

/// 公告 id：字母数字和 - _ .，用来记录"已关闭"
pub(crate) fn clean_announce_id(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))).then(|| s.to_string())
}

/// YYYY-MM-DD；格式不对返回 None
fn clean_date(s: &str) -> Option<String> {
    let s = s.trim();
    let b = s.as_bytes();
    let ok = b.len() == 10 && b.iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() });
    let month = s.get(5..7).and_then(|m| m.parse::<u32>().ok()).unwrap_or(0);
    let day = s.get(8..10).and_then(|d| d.parse::<u32>().ok()).unwrap_or(0);
    (ok && (1..=12).contains(&month) && (1..=31).contains(&day)).then(|| s.to_string())
}

/// 正文：保留换行，去掉其他控制字符
fn clean_body(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let kept: String = s.chars().filter(|c| *c == '\n' || !c.is_control()).take(800).collect();
    kept.trim().to_string()
}

fn clean_link(s: &str) -> Option<String> {
    let url = reqwest::Url::parse(s.trim()).ok()?;
    let ok = matches!(url.scheme(), "http" | "https") && url.host_str().is_some() && url.username().is_empty() && url.password().is_none();
    (ok && url.as_str().len() <= 300).then(|| url.to_string())
}

impl RawItem {
    fn sanitized(self, current: &str) -> Option<Announcement> {
        let id = clean_announce_id(&self.id)?;
        let title = clean_text(&self.title, 60);
        if title.is_empty() {
            return None;
        }
        // 只给某些版本看（比如"这个问题 0.3.3 修复了"只给 0.3.2 及以下）；版本号写错就当没限制
        let bound = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
        if !version_in_range(current, bound(&self.min_version).as_deref(), bound(&self.max_version).as_deref()) {
            return None;
        }
        let link = clean_link(&self.link);
        Some(Announcement {
            id,
            title,
            body: clean_body(&self.body),
            date: clean_date(&self.date).unwrap_or_default(),
            level: if LEVELS.contains(&self.level.as_str()) { self.level } else { "info".into() },
            pinned: self.pinned,
            link_text: if link.is_some() { clean_text(&self.link_text, 16) } else { String::new() },
            link,
            expires: clean_date(&self.expires),
        })
    }
}

/// 解析并清洗公告文件。置顶的在前，其余按日期从新到旧
pub fn parse(bytes: &[u8], current_version: &str) -> Result<Vec<Announcement>> {
    if bytes.len() > MAX_FILE {
        return Err(Error::Other("公告文件太大，已忽略".into()));
    }
    let file: RawFile = serde_json::from_slice(bytes).map_err(|e| Error::Other(format!("公告文件格式不对：{e}")))?;
    let mut seen: Vec<String> = Vec::new();
    let mut items: Vec<Announcement> = file
        .announcements
        .into_iter()
        .filter_map(|r| r.sanitized(current_version))
        .filter(|a| {
            let fresh = !seen.contains(&a.id);
            seen.push(a.id.clone());
            fresh
        })
        .collect();
    items.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.date.cmp(&a.date)));
    items.truncate(MAX_ITEMS);
    Ok(items)
}

/// 去掉已经过期的（`expires` 当天还显示）
pub fn visible(items: Vec<Announcement>, today: &str) -> Vec<Announcement> {
    items.into_iter().filter(|a| a.expires.as_deref().is_none_or(|e| e >= today)).collect()
}

/// 今天的 UTC 日期 YYYY-MM-DD
pub fn today_utc() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    date_from_days(secs.div_euclid(86_400))
}

/// 自 1970-01-01 起的天数 → YYYY-MM-DD（Howard Hinnant 的 civil_from_days）
fn date_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// 先读 GitHub 原始文件，不通再读 jsDelivr 的镜像（国内更容易连上，但有缓存，更新会慢一点）
pub async fn fetch(client: &reqwest::Client, current_version: &str) -> Result<Vec<Announcement>> {
    let urls = [
        format!("https://raw.githubusercontent.com/{REPO}/main/announcements.json"),
        format!("https://cdn.jsdelivr.net/gh/{REPO}@main/announcements.json"),
    ];
    let mut last_error = Error::Other("获取公告失败".into());
    for url in urls {
        match fetch_one(client, &url).await {
            Ok(Some(bytes)) => return parse(&bytes, current_version),
            // 仓库里还没有公告文件：就是没有公告，不算错误
            Ok(None) => return Ok(Vec::new()),
            Err(e) => last_error = e,
        }
    }
    Err(last_error)
}

async fn fetch_one(client: &reqwest::Client, url: &str) -> Result<Option<Vec<u8>>> {
    let mut resp = tokio::time::timeout(std::time::Duration::from_secs(10), client.get(url).send())
        .await
        .map_err(|_| Error::Other("获取公告超时".into()))??;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(Error::Other(format!("获取公告失败：服务器返回 {}", resp.status())));
    }
    // 边读边数，超过上限就停，不把一个巨大的文件整个读进内存
    let mut body = Vec::new();
    while let Some(chunk) = tokio::time::timeout(std::time::Duration::from_secs(10), resp.chunk())
        .await
        .map_err(|_| Error::Other("获取公告超时".into()))??
    {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_FILE {
            return Err(Error::Other("公告文件太大，已忽略".into()));
        }
    }
    Ok(Some(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(s: &str, v: &str) -> Vec<Announcement> {
        parse(s.as_bytes(), v).unwrap()
    }

    #[test]
    fn parses_a_normal_file() {
        let items = parse_str(
            r#"{"version":1,"announcements":[
              {"id":"old","title":"旧的","date":"2026-09-01"},
              {"id":"new","title":"新的","body":"第一行\r\n第二行","date":"2026-10-27","level":"warn","link":"https://example.com/a","linkText":"详情"},
              {"id":"top","title":"置顶","date":"2026-01-01","pinned":true}]}"#,
            "0.3.2",
        );
        let ids: Vec<&str> = items.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["top", "new", "old"], "置顶在前，其余按日期新到旧");
        assert_eq!(items[1].body, "第一行\n第二行");
        assert_eq!((items[1].level.as_str(), items[1].link_text.as_str()), ("warn", "详情"));
        assert_eq!(items[1].link.as_deref(), Some("https://example.com/a"));
    }

    #[test]
    fn missing_fields_and_empty_files_are_fine() {
        assert!(parse_str("{}", "0.3.2").is_empty());
        assert!(parse_str(r#"{"announcements":[]}"#, "0.3.2").is_empty());
        let items = parse_str(r#"{"announcements":[{"id":"a","title":"只有标题"}]}"#, "0.3.2");
        assert_eq!((items[0].level.as_str(), items[0].date.as_str(), items[0].pinned), ("info", "", false));
    }

    #[test]
    fn hostile_content_is_neutralized() {
        let items = parse_str(
            r##"{"announcements":[
              {"id":"x1","title":"<script>alert(1)</script>","body":"<img src=x onerror=alert(1)>\u0007\u0000ok","link":"javascript:alert(1)","linkText":"点我"},
              {"id":"x2","title":"带密码的链接","link":"https://u:p@example.com/"},
              {"id":"x3","title":"file 链接","link":"file:///C:/Windows/System32/cmd.exe"},
              {"id":"bad id!","title":"非法 id"},
              {"id":"x4","title":"   "},
              {"id":"x5","title":"坏等级","level":"<b>","date":"2026-13-45","expires":"never"}]}"##,
            "0.3.2",
        );
        let ids: Vec<&str> = items.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["x1", "x2", "x3", "x5"]);
        // 标题和正文原样保留成文本（界面按纯文本显示，不会当 HTML）；控制字符被去掉
        assert_eq!(items[0].title, "<script>alert(1)</script>");
        assert_eq!(items[0].body, "<img src=x onerror=alert(1)>ok");
        for a in &items[..3] {
            assert!(a.link.is_none() && a.link_text.is_empty(), "{} 的链接应该被丢弃", a.id);
        }
        assert_eq!((items[3].level.as_str(), items[3].date.as_str(), items[3].expires.clone()), ("info", "", None));
    }

    #[test]
    fn limits_are_enforced() {
        let long = "字".repeat(1000);
        let many: Vec<String> = (0..12).map(|i| format!(r#"{{"id":"n{i}","title":"{}","body":"{long}","date":"2026-10-{:02}"}}"#, "题".repeat(100), i % 28 + 1)).collect();
        let items = parse_str(&format!(r#"{{"announcements":[{}]}}"#, many.join(",")), "0.3.2");
        assert_eq!(items.len(), MAX_ITEMS);
        assert!(items.iter().all(|a| a.title.chars().count() <= 60 && a.body.chars().count() <= 800));
        assert!(parse(&vec![b' '; MAX_FILE + 1], "0.3.2").is_err());
        assert!(parse(b"not json", "0.3.2").is_err());
    }

    #[test]
    fn duplicate_ids_keep_the_first() {
        let items = parse_str(r#"{"announcements":[{"id":"a","title":"第一"},{"id":"a","title":"第二"}]}"#, "0.3.2");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "第一");
    }

    #[test]
    fn version_targeting() {
        let file = r#"{"announcements":[
          {"id":"all","title":"所有人"},
          {"id":"old","title":"只给旧版本","maxVersion":"0.3.2"},
          {"id":"new","title":"只给新版本","minVersion":"0.4.0"},
          {"id":"typo","title":"版本写错了","minVersion":"abc"}]}"#;
        let ids = |v: &str| parse_str(file, v).iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids("0.3.2"), ["all", "old", "typo"]);
        assert_eq!(ids("0.3.3"), ["all", "typo"]);
        assert_eq!(ids("0.4.0"), ["all", "new", "typo"]);
    }

    #[test]
    fn expiry_is_inclusive() {
        let items = parse_str(r#"{"announcements":[{"id":"a","title":"到期","expires":"2026-10-27"},{"id":"b","title":"长期"}]}"#, "0.3.2");
        assert_eq!(visible(items.clone(), "2026-10-27").len(), 2, "到期当天还显示");
        assert_eq!(visible(items, "2026-10-28").len(), 1);
    }

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(11_017), "2000-03-01");
        assert_eq!(date_from_days(19_723), "2024-01-01");
        assert_eq!(date_from_days(19_782), "2024-02-29");
        assert_eq!(date_from_days(-1), "1969-12-31");
        let today = today_utc();
        assert!(clean_date(&today).is_some() && today.as_str() >= "2026-01-01", "{today}");
    }
}
