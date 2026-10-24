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
const MCIM: &str = "https://mod.mcimirror.top";

/// 官方前缀 → (镜像根地址, 镜像路径前缀)。
/// Minecraft / 加载器走 BMCLAPI（https://bmclapidoc.bangbang93.com/），
/// Modrinth / CurseForge 走 MCIM 社区镜像（https://mod.mcimirror.top）。
const MIRROR_RULES: &[(&str, &str, &str)] = &[
    ("https://piston-meta.mojang.com", BMCLAPI, ""),
    ("https://launchermeta.mojang.com", BMCLAPI, ""),
    ("https://piston-data.mojang.com", BMCLAPI, ""),
    ("https://launcher.mojang.com", BMCLAPI, ""),
    ("https://resources.download.minecraft.net", BMCLAPI, "/assets"),
    ("https://libraries.minecraft.net", BMCLAPI, "/maven"),
    ("https://maven.minecraftforge.net", BMCLAPI, "/maven"),
    ("https://files.minecraftforge.net/maven", BMCLAPI, "/maven"),
    ("https://maven.neoforged.net/releases", BMCLAPI, "/maven"),
    ("https://meta.fabricmc.net", BMCLAPI, "/fabric-meta"),
    ("https://maven.fabricmc.net", BMCLAPI, "/maven"),
    ("https://meta.quiltmc.org", BMCLAPI, "/quilt-meta"),
    ("https://maven.quiltmc.org/repository/release", BMCLAPI, "/maven"),
    ("https://authlib-injector.yushi.moe", BMCLAPI, "/mirrors/authlib-injector"),
    ("https://api.modrinth.com", MCIM, "/modrinth"),
    ("https://cdn.modrinth.com", MCIM, ""),
    ("https://edge.forgecdn.net", MCIM, ""),
    ("https://mediafilez.forgecdn.net", MCIM, ""),
];

/// 改写成国内镜像地址；没有镜像的地址返回 None
pub fn to_mirror(url: &str) -> Option<String> {
    // 部分老 json 里还是 http://
    let normalized = match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    };
    MIRROR_RULES.iter().find_map(|(official, host, prefix)| {
        normalized
            .strip_prefix(official)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .map(|rest| format!("{host}{prefix}{rest}"))
    })
}

/// 按优先级排好的候选地址，首选源在前，另一个源作为回退
pub fn candidates(url: &str, primary: Source) -> Vec<String> {
    match to_mirror(url) {
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
            Source::Bmclapi => to_mirror(MANIFEST_URL).expect("清单地址一定有镜像"),
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
            to_mirror("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json").unwrap(),
            "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json"
        );
        assert_eq!(
            to_mirror("https://resources.download.minecraft.net/ab/abcdef").unwrap(),
            "https://bmclapi2.bangbang93.com/assets/ab/abcdef"
        );
        assert_eq!(
            to_mirror("http://libraries.minecraft.net/a/b.jar").unwrap(),
            "https://bmclapi2.bangbang93.com/maven/a/b.jar"
        );
        assert!(to_mirror("https://example.com/x").is_none());
        assert_eq!(
            to_mirror("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar").unwrap(),
            "https://mod.mcimirror.top/data/AANobbMI/versions/x/sodium.jar"
        );
        assert_eq!(to_mirror("https://api.modrinth.com/v2/search").unwrap(), "https://mod.mcimirror.top/modrinth/v2/search");
        // 只匹配完整主机名，不能被 libraries.minecraft.net.evil 之类骗到
        assert!(to_mirror("https://libraries.minecraft.net.evil.com/x").is_none());
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
