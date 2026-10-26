//! 更新检测：读 GitHub Releases 列表，和当前版本比较。
//! 不用 `/releases/latest`：它只返回正式版，而我们的版本目前都是预发布，会一直 404。
//! 接口返回的内容当作不可信数据：版本号必须能解析，下载页网址由仓库地址和标签自己拼，不用接口给的链接。

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::{Error, Result};

pub const REPO: &str = "MRTangwin8/TheGodBBSLauncher";
const NOTES_LIMIT: usize = 3000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    core: (u64, u64, u64),
    /// 1.0.0-beta.1 里的 beta.1；有预发布后缀的比同号的正式版旧
    pre: Option<String>,
}

fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let s = s.split('+').next()?; // 丢掉构建元数据
    let (core, pre) = match s.split_once('-') {
        // 后缀会被拼进网址，只允许字母、数字、点和连字符
        Some((_, p)) if p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') => return None,
        Some((c, p)) => (c, Some(p.to_string())),
        None => (s, None),
    };
    let mut it = core.split('.');
    let mut next = || -> Option<u64> { it.next()?.parse().ok() };
    let core = (next()?, next()?, next()?);
    if it.next().is_some() {
        return None;
    }
    Some(Version { core, pre })
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => a.cmp(b),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 当前运行的版本，不带 v
    pub current: String,
    /// 最新的版本；一个发布都没有时为空
    pub latest: Option<String>,
    pub has_update: bool,
    /// 发布页网址（只用于在系统浏览器里打开）
    pub url: Option<String>,
    /// 更新说明，纯文本
    pub notes: String,
    pub published: Option<String>,
    pub prerelease: bool,
}

/// 从发布列表里挑出版本号最高的（跳过草稿和看不懂版本号的），和当前版本比较
fn pick_latest(releases: Vec<GhRelease>, current: &str) -> UpdateInfo {
    let cur = parse_version(current);
    let best = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| parse_version(&r.tag_name).map(|v| (v, r)))
        .max_by(|a, b| a.0.cmp(&b.0));
    let current = current.trim().trim_start_matches(['v', 'V']).to_string();
    let Some((version, release)) = best else {
        return UpdateInfo { current, latest: None, has_update: false, url: None, notes: String::new(), published: None, prerelease: false };
    };
    let has_update = cur.map(|c| version > c).unwrap_or(false);
    let tag = release.tag_name.trim().to_string();
    UpdateInfo {
        current,
        latest: Some(tag.trim_start_matches(['v', 'V']).to_string()),
        has_update,
        // 标签已经通过版本号解析，只含数字、点、字母和连字符，可以放心拼进网址
        url: Some(format!("https://github.com/{REPO}/releases/tag/{tag}")),
        notes: clip(release.body.as_deref().unwrap_or("")),
        published: release.published_at,
        prerelease: release.prerelease,
    }
}

fn clip(s: &str) -> String {
    let s = s.replace("\r\n", "\n");
    let s = s.trim();
    if s.chars().count() <= NOTES_LIMIT {
        return s.to_string();
    }
    let mut out: String = s.chars().take(NOTES_LIMIT).collect();
    out.push_str("\n……（更新说明太长，完整内容在发布页）");
    out
}

pub async fn check(client: &reqwest::Client, current: &str) -> Result<UpdateInfo> {
    let resp = client
        .get(format!("https://api.github.com/repos/{REPO}/releases?per_page=30"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    let status = resp.status();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(Error::Other("GitHub 暂时限制了请求次数，过一会儿再检查更新".into()));
    }
    if !status.is_success() {
        return Err(Error::Other(format!("检查更新失败：GitHub 返回 {status}")));
    }
    let releases: Vec<GhRelease> = resp.json().await.map_err(|_| Error::Other("检查更新失败：GitHub 返回的内容看不懂".into()))?;
    Ok(pick_latest(releases, current))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, draft: bool, pre: bool) -> GhRelease {
        GhRelease { tag_name: tag.into(), body: Some("说明".into()), draft, prerelease: pre, published_at: Some("2026-10-25T00:00:00Z".into()) }
    }

    #[test]
    fn compares_versions_numerically() {
        let v = |s: &str| parse_version(s).unwrap();
        assert!(v("0.10.0") > v("0.9.9"), "不能按字符串比较");
        assert!(v("v1.0.0") > v("0.99.99"));
        assert!(v("1.0.0") > v("1.0.0-beta.1"), "正式版比同号预发布新");
        assert_eq!(v("v0.2.0"), v("0.2.0+build5"));
        assert!(parse_version("latest").is_none());
        assert!(parse_version("1.2").is_none());
        assert!(parse_version("1.2.3.4").is_none());
    }

    #[test]
    fn picks_highest_even_when_all_are_prereleases() {
        let list = vec![rel("v0.1.0", false, true), rel("v0.3.0", false, true), rel("v0.2.0", false, true)];
        let info = pick_latest(list, "0.2.0");
        assert_eq!(info.latest.as_deref(), Some("0.3.0"));
        assert!(info.has_update && info.prerelease);
        assert_eq!(info.url.as_deref(), Some("https://github.com/MRTangwin8/TheGodBBSLauncher/releases/tag/v0.3.0"));
    }

    #[test]
    fn up_to_date_and_newer_than_published_are_not_updates() {
        assert!(!pick_latest(vec![rel("v0.2.0", false, true)], "0.2.0").has_update);
        assert!(!pick_latest(vec![rel("v0.2.0", false, true)], "0.3.0").has_update, "本地开发版比已发布的还新");
    }

    #[test]
    fn ignores_drafts_and_garbage_tags() {
        let list = vec![rel("v9.9.9", true, false), rel("nightly", false, true), rel("v0.2.0", false, true)];
        assert_eq!(pick_latest(list, "0.1.0").latest.as_deref(), Some("0.2.0"));
        let none = pick_latest(vec![rel("v9.9.9", true, false)], "0.1.0");
        assert!(none.latest.is_none() && !none.has_update && none.url.is_none());
    }

    #[test]
    fn hostile_tag_cannot_inject_into_the_url() {
        for tag in ["v1.0.0/../../evil", "v1.0.0-../../evil", "v1.0.0-a b", "v1.0.0-", "v1.0.0-x?y=1"] {
            let info = pick_latest(vec![rel(tag, false, false)], "0.1.0");
            assert!(info.latest.is_none() && info.url.is_none(), "{tag} 应该被丢弃");
        }
        assert!(parse_version("v1.0.0-beta.1").is_some());
    }

    #[test]
    fn long_notes_are_clipped() {
        let mut r = rel("v0.2.0", false, false);
        r.body = Some("字".repeat(5000));
        let notes = pick_latest(vec![r], "0.1.0").notes;
        assert!(notes.chars().count() < 3100 && notes.contains("发布页"));
    }
}
