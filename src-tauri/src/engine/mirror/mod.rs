//! 下载源：官方 / BMCLAPI。把官方地址改写成镜像地址，失败时按顺序回退。

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::engine::meta::MANIFEST_URL;

/// 用户设置里的下载源
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourcePref {
    /// 测速后自动选
    #[default]
    Auto,
    Official,
    Bmclapi,
}

/// 实际使用的源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Official,
    Bmclapi,
}

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com";

/// 官方前缀 → BMCLAPI 前缀（BMCLAPI 文档：https://bmclapidoc.bangbang93.com/）
const BMCLAPI_RULES: &[(&str, &str)] = &[
    ("https://piston-meta.mojang.com", ""),
    ("https://launchermeta.mojang.com", ""),
    ("https://piston-data.mojang.com", ""),
    ("https://launcher.mojang.com", ""),
    ("https://resources.download.minecraft.net", "/assets"),
    ("https://libraries.minecraft.net", "/maven"),
    ("https://maven.minecraftforge.net", "/maven"),
    ("https://files.minecraftforge.net/maven", "/maven"),
    ("https://maven.neoforged.net/releases", "/maven"),
    ("https://meta.fabricmc.net", "/fabric-meta"),
    ("https://maven.fabricmc.net", "/maven"),
    ("https://meta.quiltmc.org", "/quilt-meta"),
    ("https://maven.quiltmc.org/repository/release", "/maven"),
    ("https://authlib-injector.yushi.moe", "/mirrors/authlib-injector"),
];

/// 改写成 BMCLAPI 地址；BMCLAPI 不镜像的地址返回 None
pub fn to_bmclapi(url: &str) -> Option<String> {
    // 部分老 json 里还是 http://
    let normalized = match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    };
    BMCLAPI_RULES.iter().find_map(|(official, mirror)| {
        normalized
            .strip_prefix(official)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .map(|rest| format!("{BMCLAPI}{mirror}{rest}"))
    })
}

/// 按优先级排好的候选地址，首选源在前，另一个源作为回退
pub fn candidates(url: &str, primary: Source) -> Vec<String> {
    match to_bmclapi(url) {
        None => vec![url.to_string()],
        Some(mirror) => match primary {
            Source::Bmclapi => vec![mirror, url.to_string()],
            Source::Official => vec![url.to_string(), mirror],
        },
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub source: Source,
    /// 拉取版本清单的耗时；失败为 None
    pub millis: Option<u64>,
}

/// 同时从两个源拉一次版本清单，看谁快
pub async fn probe(client: &reqwest::Client) -> Vec<ProbeResult> {
    let timed = |source: Source| async move {
        let url = match source {
            Source::Official => MANIFEST_URL.to_string(),
            Source::Bmclapi => to_bmclapi(MANIFEST_URL).expect("清单地址一定有镜像"),
        };
        let start = Instant::now();
        let ok = async {
            let resp = client.get(&url).timeout(Duration::from_secs(8)).send().await?;
            resp.error_for_status()?.bytes().await
        }
        .await
        .is_ok();
        ProbeResult {
            source,
            millis: ok.then(|| start.elapsed().as_millis() as u64),
        }
    };
    let (a, b) = tokio::join!(timed(Source::Official), timed(Source::Bmclapi));
    vec![a, b]
}

/// 测速结果里最快的源；都失败时用 BMCLAPI（国内更可能连得上）
pub fn fastest(results: &[ProbeResult]) -> Source {
    results
        .iter()
        .filter_map(|r| r.millis.map(|m| (m, r.source)))
        .min_by_key(|(m, _)| *m)
        .map(|(_, s)| s)
        .unwrap_or(Source::Bmclapi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_known_hosts() {
        assert_eq!(
            to_bmclapi("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json").unwrap(),
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
        );
        assert_eq!(
            to_bmclapi("https://resources.download.minecraft.net/ab/abcdef").unwrap(),
            "https://bmclapi2.bangbang93.com/assets/ab/abcdef"
        );
        assert_eq!(
            to_bmclapi("http://libraries.minecraft.net/a/b.jar").unwrap(),
            "https://bmclapi2.bangbang93.com/maven/a/b.jar"
        );
        assert!(to_bmclapi("https://example.com/x").is_none());
        // 只匹配完整主机名，不能被 libraries.minecraft.net.evil 之类骗到
        assert!(to_bmclapi("https://libraries.minecraft.net.evil.com/x").is_none());
    }

    #[test]
    fn candidate_order_follows_primary() {
        let url = "https://libraries.minecraft.net/a.jar";
        assert_eq!(candidates(url, Source::Bmclapi)[0], "https://bmclapi2.bangbang93.com/maven/a.jar");
        assert_eq!(candidates(url, Source::Official)[0], url);
        assert_eq!(candidates("https://example.com/a", Source::Bmclapi).len(), 1);
    }

    #[test]
    fn picks_fastest() {
        let r = [
            ProbeResult { source: Source::Official, millis: Some(900) },
            ProbeResult { source: Source::Bmclapi, millis: Some(120) },
        ];
        assert_eq!(fastest(&r), Source::Bmclapi);
        let r = [
            ProbeResult { source: Source::Official, millis: Some(90) },
            ProbeResult { source: Source::Bmclapi, millis: None },
        ];
        assert_eq!(fastest(&r), Source::Official);
    }
}
