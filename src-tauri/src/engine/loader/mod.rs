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
/// `name` 为空时用 [`default_version_name`]（带 MC 版本，比安装器自带的 neoforge-21.11.48 直观）。
pub async fn install(ctx: &LoaderContext<'_>, kind: LoaderKind, mc: &str, loader_id: &str, name: Option<&str>) -> Result<String> {
    if !ctx.game.version_json(mc).is_file() {
        return Err(Error::Other(format!("请先安装原版 {mc}")));
    }
    let name = name.map(str::to_string).unwrap_or_else(|| default_version_name(kind, mc, loader_id));
    crate::engine::install::validate_version_name(&name)?;
    if ctx.game.versions().join(&name).exists() {
        return Err(Error::Other(format!("已经有名为「{name}」的版本了，换个名字吧")));
    }
    let installed = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => fabric::install(ctx, kind, mc, loader_id).await?,
        LoaderKind::Forge | LoaderKind::NeoForge => forge::install(ctx, kind, mc, loader_id).await?,
    };
    crate::engine::install::rename_version(ctx.game, &installed, &name)?;
    Ok(name)
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
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
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

/// 判断 version.json 用的是哪种加载器。
/// 只看库不够：新版 NeoForge 的库里没有 net.neoforged:neoforge，要靠主类和 `--fml.neoForgeVersion` 参数识别。
pub fn detect(version: &crate::engine::meta::VersionJson) -> Option<LoaderKind> {
    let main = version.main_class.as_deref().unwrap_or_default();
    let has_lib = |prefix: &str| version.libraries.iter().any(|l| l.name.starts_with(prefix));
    let game_args: Vec<&str> = version
        .arguments
        .iter()
        .flat_map(|a| &a.game)
        .filter_map(|a| match a {
            crate::engine::meta::version::Argument::Plain(s) => Some(s.as_str()),
            _ => None,
        })
        .chain(version.minecraft_arguments.iter().flat_map(|s| s.split_whitespace()))
        .collect();
    let has_arg = |needle: &str| game_args.iter().any(|a| a.contains(needle));

    if has_lib("net.fabricmc:fabric-loader:") || main.starts_with("net.fabricmc.") {
        Some(LoaderKind::Fabric)
    } else if has_lib("org.quiltmc:quilt-loader:") || main.starts_with("org.quiltmc.") {
        Some(LoaderKind::Quilt)
    } else if has_arg("--fml.neoForgeVersion") || main.starts_with("net.neoforged.") || has_lib("net.neoforged:neoforge:") || has_lib("net.neoforged:forge:") {
        // 1.20.1 的 NeoForge 用 net.neoforged:forge 和 --fml.forgeVersion，所以要先于 Forge 判断
        Some(LoaderKind::NeoForge)
    } else if has_arg("--fml.forgeVersion")
        || has_arg("FMLTweaker")
        || has_lib("net.minecraftforge:forge:")
        || has_lib("net.minecraftforge:fmlloader:")
        || main.starts_with("net.minecraftforge.")
    {
        Some(LoaderKind::Forge)
    } else {
        None
    }
}

/// 默认版本名：<MC>-<加载器>-<加载器版本>，比如 1.21.11-NeoForge-21.11.48
pub fn default_version_name(kind: LoaderKind, mc: &str, loader_id: &str) -> String {
    let label = match kind {
        LoaderKind::Fabric => "Fabric",
        LoaderKind::Quilt => "Quilt",
        LoaderKind::Forge => "Forge",
        LoaderKind::NeoForge => "NeoForge",
    };
    // Forge 的 id 是完整 maven 版本（1.7.10-10.13.4.1614-1.7.10），去掉重复的 MC 版本
    let short = loader_id.strip_prefix(&format!("{mc}-")).unwrap_or(loader_id);
    let short = short.strip_suffix(&format!("-{mc}")).unwrap_or(short);
    format!("{mc}-{label}-{short}")
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

    fn v(json: &str) -> crate::engine::meta::VersionJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn detects_loaders() {
        assert_eq!(detect(&v(r#"{"id":"a","libraries":[{"name":"org.ow2.asm:asm:9.6"},{"name":"net.fabricmc:fabric-loader:0.16.9"}]}"#)), Some(LoaderKind::Fabric));
        assert_eq!(detect(&v(r#"{"id":"a","mainClass":"org.quiltmc.loader.impl.launch.knot.KnotClient","libraries":[]}"#)), Some(LoaderKind::Quilt));
        // 真实的 NeoForge 21.11.48：库里没有 neoforge 主库
        let neo = v(r#"{"id":"neoforge-21.11.48","inheritsFrom":"1.21.11","mainClass":"net.neoforged.fml.startup.Client",
            "arguments":{"game":["--fml.neoForgeVersion","21.11.48","--fml.mcVersion","1.21.11"],"jvm":[]},
            "libraries":[{"name":"net.neoforged.fancymodloader:loader:10.0.36"},{"name":"net.minecraftforge:srgutils:0.4.15"}]}"#);
        assert_eq!(detect(&neo), Some(LoaderKind::NeoForge), "不能因为有 net.minecraftforge:srgutils 就当成 Forge");
        // 1.20.1 的 NeoForge 参数和 Forge 一样，靠 net.neoforged:forge 区分
        let neo1201 = v(r#"{"id":"x","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
            "arguments":{"game":["--fml.forgeVersion","47.1.106"],"jvm":[]},"libraries":[{"name":"net.neoforged:forge:1.20.1-47.1.106:universal"}]}"#);
        assert_eq!(detect(&neo1201), Some(LoaderKind::NeoForge));
        let forge = v(r#"{"id":"x","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher","arguments":{"game":["--launchTarget","forgeclient","--fml.forgeVersion","47.4.28"],"jvm":[]},"libraries":[]}"#);
        assert_eq!(detect(&forge), Some(LoaderKind::Forge));
        let legacy = v(r#"{"id":"x","mainClass":"net.minecraft.launchwrapper.Launch","minecraftArguments":"--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker","libraries":[]}"#);
        assert_eq!(detect(&legacy), Some(LoaderKind::Forge));
        assert_eq!(detect(&v(r#"{"id":"1.21.1","mainClass":"net.minecraft.client.main.Main","libraries":[]}"#)), None);
    }

    #[test]
    fn default_names_include_minecraft_version() {
        assert_eq!(default_version_name(LoaderKind::NeoForge, "1.21.11", "21.11.48"), "1.21.11-NeoForge-21.11.48");
        assert_eq!(default_version_name(LoaderKind::Forge, "1.20.1", "1.20.1-47.4.28"), "1.20.1-Forge-47.4.28");
        assert_eq!(default_version_name(LoaderKind::Forge, "1.7.10", "1.7.10-10.13.4.1614-1.7.10"), "1.7.10-Forge-10.13.4.1614");
        assert_eq!(default_version_name(LoaderKind::Fabric, "1.21.1", "0.20.1"), "1.21.1-Fabric-0.20.1");
    }
}
