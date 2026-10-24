//! Fabric / Quilt：meta API 直接给出继承原版的 profile json，下载库即可。

use serde::Deserialize;

use super::{fetch_text, sort_newest_first, LoaderContext, LoaderKind, LoaderVersion};
use crate::engine::install::{collect_tasks, Stage};
use crate::engine::meta::{Platform, VersionJson};
use crate::engine::mirror;
use crate::engine::{Error, Result};

fn meta_base(kind: LoaderKind) -> &'static str {
    match kind {
        LoaderKind::Quilt => "https://meta.quiltmc.org/v3",
        _ => "https://meta.fabricmc.net/v2",
    }
}

#[derive(Deserialize)]
struct Entry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

pub async fn list_versions(client: &reqwest::Client, primary: mirror::Source, kind: LoaderKind, mc: &str) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/versions/loader/{mc}", meta_base(kind));
    let text = fetch_text(client, &url, primary).await?;
    // 不支持的版本返回空数组（Quilt 有时返回 400）
    let entries: Vec<Entry> = serde_json::from_str(&text).unwrap_or_default();
    let mut list: Vec<LoaderVersion> = entries
        .into_iter()
        .map(|e| LoaderVersion {
            stable: e.loader.stable.unwrap_or(!e.loader.version.contains("beta") && !e.loader.version.contains("pre")),
            id: e.loader.version.clone(),
            version: e.loader.version,
        })
        .collect();
    sort_newest_first(&mut list);
    Ok(list)
}

pub async fn install(ctx: &LoaderContext<'_>, kind: LoaderKind, mc: &str, loader: &str) -> Result<String> {
    let url = format!("{}/versions/loader/{mc}/{loader}/profile/json", meta_base(kind));
    let text = fetch_text(ctx.client, &url, ctx.downloader.primary).await?;
    let profile: VersionJson =
        serde_json::from_str(&text).map_err(|_| Error::Other(format!("{loader} 不支持 Minecraft {mc}")))?;
    let id = profile.id.clone();

    let path = ctx.game.version_json(&id);
    tokio::fs::create_dir_all(path.parent().expect("版本目录")).await?;
    tokio::fs::write(&path, &text).await?;

    download_libraries(ctx, &profile, ctx.platform).await?;
    Ok(id)
}

/// 只下载 profile 自己新增的库（原版部分已经装过了）
async fn download_libraries(ctx: &LoaderContext<'_>, profile: &VersionJson, platform: &Platform) -> Result<()> {
    let only_libs = VersionJson { id: profile.id.clone(), libraries: profile.libraries.clone(), ..Default::default() };
    let tasks = collect_tasks(ctx.game, &only_libs, None, platform);
    let report = ctx.report.clone();
    let r = ctx.downloader.download_all(tasks, std::sync::Arc::new(move |p| report(Stage::Loader, p))).await;
    match r.failed.first() {
        Some(f) => Err(Error::Other(format!("加载器库下载失败：{}：{}", f.path, f.reason))),
        None => Ok(()),
    }
}
