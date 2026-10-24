//! 模组加载器：Fabric / Quilt（合并 profile json）、Forge / NeoForge（运行官方安装器的 processors）。

pub mod fabric;
pub mod forge;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::download::{Downloader, Progress};
use crate::engine::install::{GameDir, Stage};
use crate::engine::mirror;
use crate::engine::meta::Platform;
use crate::engine::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    /// 安装时传回来的标识（Forge 是完整的 maven 版本号，如 1.7.10-10.13.4.1614-1.7.10）
    pub id: String,
    /// 显示用的加载器版本号
    pub version: String,
    /// 稳定版 / 推荐版
    pub stable: bool,
}

pub struct LoaderContext<'a> {
    pub downloader: &'a Downloader,
    pub client: &'a reqwest::Client,
    pub game: &'a GameDir,
    pub platform: &'a Platform,
    /// 运行 Forge 安装器 processors 用的 java
    pub java: Option<PathBuf>,
    pub report: Arc<dyn Fn(Stage, Progress) + Send + Sync>,
}

pub async fn list_versions(client: &reqwest::Client, primary: mirror::Source, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric::list_versions(client, primary, kind, mc).await,
        LoaderKind::Forge => forge::list_forge(client, primary, mc).await,
        LoaderKind::NeoForge => forge::list_neoforge(client, primary, mc).await,
    }
}

/// 安装加载器，返回新版本的 id。调用前原版必须已经装好。
pub async fn install(ctx: &LoaderContext<'_>, kind: LoaderKind, mc: &str, loader_id: &str) -> Result<String> {
    if !ctx.game.version_json(mc).is_file() {
        return Err(Error::Other(format!("请先安装原版 {mc}")));
    }
    match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric::install(ctx, kind, mc, loader_id).await,
        LoaderKind::Forge | LoaderKind::NeoForge => forge::install(ctx, kind, mc, loader_id).await,
    }
}

/// 拉取文本，按镜像顺序回退
pub(crate) async fn fetch_text(client: &reqwest::Client, url: &str, primary: mirror::Source) -> Result<String> {
    let mut last = None;
    for candidate in mirror::candidates(url, primary) {
        match client.get(&candidate).send().await {
            Ok(r) if r.status().is_success() => return Ok(r.text().await?),
            Ok(r) => last = Some(Error::BadStatus { url: candidate, status: r.status().as_u16() }),
            Err(e) => last = Some(e.into()),
        }
    }
    Err(last.unwrap_or_else(|| Error::Other(format!("无法获取 {url}"))))
}

/// maven-metadata.xml 里的所有 <version>
pub(crate) fn maven_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split_once("</version>").map(|(v, _)| v.trim().to_string()))
        .collect()
}

/// 版本号比较：先比 `-` 前的数字部分，相同时正式版大于测试版（0.20.0 > 0.20.0-beta.9）
pub(crate) fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn nums(s: &str) -> Vec<u64> {
        s.split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect()
    }
    let (core_a, pre_a) = a.split_once('-').map_or((a, None), |(c, p)| (c, Some(p)));
    let (core_b, pre_b) = b.split_once('-').map_or((b, None), |(c, p)| (c, Some(p)));
    nums(core_a).cmp(&nums(core_b)).then_with(|| match (pre_a, pre_b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => nums(x).cmp(&nums(y)),
    })
}

/// 新版本在前
pub(crate) fn sort_newest_first(list: &mut [LoaderVersion]) {
    list.sort_by(|a, b| compare_versions(&b.version, &a.version));
}

/// 根据 version.json 的库判断是哪种加载器（用于版本列表显示）
pub fn detect(libraries: &[crate::engine::meta::Library]) -> Option<LoaderKind> {
    libraries.iter().find_map(|l| {
        let n = l.name.as_str();
        if n.starts_with("net.fabricmc:fabric-loader:") {
            Some(LoaderKind::Fabric)
        } else if n.starts_with("org.quiltmc:quilt-loader:") {
            Some(LoaderKind::Quilt)
        } else if n.starts_with("net.neoforged") && (n.contains(":neoforge:") || n.contains(":forge:") || n.contains(":fancymodloader")) {
            Some(LoaderKind::NeoForge)
        } else if n.starts_with("net.minecraftforge:forge:") || n.starts_with("net.minecraftforge:fmlloader:") {
            Some(LoaderKind::Forge)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven_metadata() {
        let xml = "<metadata><versioning><versions><version>1.20.1-47.1.0</version>\n<version> 1.20.1-47.2.0 </version></versions></versioning></metadata>";
        assert_eq!(maven_versions(xml), ["1.20.1-47.1.0", "1.20.1-47.2.0"]);
    }

    #[test]
    fn sorts_versions_naturally() {
        let mut v: Vec<LoaderVersion> = ["10.13.0.1150", "10.13.4.1614", "10.13.2.1291", "0.20.0-beta.9", "0.20.0", "0.20.0-beta.10", "0.16.10", "0.16.9"]
            .into_iter()
            .map(|s| LoaderVersion { id: s.into(), version: s.into(), stable: true })
            .collect();
        sort_newest_first(&mut v);
        let order: Vec<&str> = v.iter().map(|x| x.version.as_str()).collect();
        assert_eq!(order, ["10.13.4.1614", "10.13.2.1291", "10.13.0.1150", "0.20.0", "0.20.0-beta.10", "0.20.0-beta.9", "0.16.10", "0.16.9"]);
    }

    #[test]
    fn detects_loader_from_libraries() {
        let libs: Vec<crate::engine::meta::Library> = serde_json::from_str(
            r#"[{"name":"org.ow2.asm:asm:9.6"},{"name":"net.fabricmc:fabric-loader:0.16.9"}]"#,
        ).unwrap();
        assert_eq!(detect(&libs), Some(LoaderKind::Fabric));
        let libs: Vec<crate::engine::meta::Library> = serde_json::from_str(r#"[{"name":"net.neoforged.fancymodloader:loader:4.0.0"}]"#).unwrap();
        assert_eq!(detect(&libs), None, "只认主库");
        let libs: Vec<crate::engine::meta::Library> = serde_json::from_str(r#"[{"name":"net.neoforged:neoforge:21.1.77:universal"}]"#).unwrap();
        assert_eq!(detect(&libs), Some(LoaderKind::NeoForge));
    }
}
