//! 导出整合包：能在 Modrinth / CurseForge 上找到的模组只写引用（包小、合规），找不到的放进 overrides。

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::{Digest, Sha1};
use sha2::Sha512;

use super::PackFormat;
use crate::engine::download::Progress;
use crate::engine::loader::LoaderKind;
use crate::engine::resources::{curseforge, modrinth, ResourceApi};
use crate::engine::{Error, Result};

/// 可以勾选导出的顶层条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntry {
    pub name: String,
    pub is_dir: bool,
    pub default_on: bool,
}

/// 永远不导出：启动器和游戏运行时生成的东西
const NEVER: &[&str] = &["natives", "logs", "crash-reports", ".fabric", ".mixin.out", "downloads", "usercache.json", "usernamecache.json", "realms_persistence.json", "godbbs.json", ".cache"];
/// 默认勾选
const DEFAULT_ON: &[&str] = &["mods", "config", "defaultconfigs", "kubejs", "scripts", "resourcepacks", "shaderpacks", "options.txt", "global_packs", "datapacks"];

pub fn candidates(game_dir: &Path, version_id: &str) -> Vec<ExportEntry> {
    let Ok(entries) = std::fs::read_dir(game_dir) else { return vec![] };
    let mut out: Vec<ExportEntry> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let own = name == format!("{version_id}.json") || name == format!("{version_id}.jar");
            let skip = own || NEVER.contains(&name.as_str()) || name.starts_with("natives-") || name.starts_with("hs_err_pid");
            (!skip).then(|| ExportEntry {
                default_on: DEFAULT_ON.contains(&name.as_str()),
                is_dir: e.file_type().map(|t| t.is_dir()).unwrap_or(false),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.default_on.cmp(&a.default_on).then(a.name.cmp(&b.name)));
    out
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub format: PackFormat,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub summary: String,
    /// 选中的顶层条目
    pub include: Vec<String>,
}

/// 游戏环境（从版本信息得到，由调用方填）
#[derive(Debug, Clone)]
pub struct ExportEnv {
    pub mc: String,
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<String>,
    pub min_memory_mb: Option<u32>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub path: String,
    /// 以下载链接形式引用的文件数
    pub linked: usize,
    /// 直接打包进去的文件数
    pub bundled: usize,
}

fn walk(base: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let path = base.join(rel);
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(&path) else { return };
        for e in entries.flatten() {
            walk(base, &format!("{rel}/{}", e.file_name().to_string_lossy()), out);
        }
    } else if path.is_file() {
        out.push((rel.to_string(), path));
    }
}

/// 可以用平台链接代替的文件：mods / resourcepacks / shaderpacks 下启用的 jar/zip
fn linkable(rel: &str) -> bool {
    let top = rel.split('/').next().unwrap_or_default();
    ["mods", "resourcepacks", "shaderpacks"].contains(&top) && rel.matches('/').count() == 1 && (rel.ends_with(".jar") || rel.ends_with(".zip"))
}

fn loader_key(l: LoaderKind, mrpack: bool) -> &'static str {
    match (l, mrpack) {
        (LoaderKind::Fabric, true) => "fabric-loader",
        (LoaderKind::Quilt, true) => "quilt-loader",
        (LoaderKind::Fabric, false) => "fabric",
        (LoaderKind::Quilt, false) => "quilt",
        (LoaderKind::Forge, _) => "forge",
        (LoaderKind::NeoForge, _) => "neoforge",
    }
}

pub async fn export(
    api: &ResourceApi,
    game_dir: &Path,
    opts: &ExportOptions,
    env: &ExportEnv,
    out_path: &Path,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<ExportSummary> {
    let mut files = Vec::new();
    for entry in &opts.include {
        if super::safe_rel(entry).is_some_and(|e| !e.contains('/')) {
            walk(game_dir, entry, &mut files);
        }
    }
    files.sort();
    let total = files.len();
    let report = |done: usize| on_progress(Progress { total_files: total, finished_files: done, ..Default::default() });

    // 找出能在平台上找到的文件
    let candidates: Vec<&(String, PathBuf)> = files.iter().filter(|(rel, _)| linkable(rel)).collect();
    let mut mr_links: HashMap<String, (String, String, Option<String>, u64)> = HashMap::new(); // rel → (url, sha1, sha512, size)
    let mut cf_links: HashMap<String, (u64, u64)> = HashMap::new(); // rel → (project, file)
    match opts.format {
        PackFormat::Modrinth => {
            let mut by_sha1 = HashMap::new();
            for (rel, path) in &candidates {
                let data = std::fs::read(path)?;
                by_sha1.insert(hex::encode(Sha1::digest(&data)), (rel.clone(), hex::encode(Sha512::digest(&data)), data.len() as u64));
            }
            let found = modrinth::identify_files(&api.client, &by_sha1.keys().cloned().collect::<Vec<_>>()).await.unwrap_or_default();
            for (sha1, (url, sha512)) in found {
                if let Some((rel, local512, size)) = by_sha1.get(&sha1) {
                    mr_links.insert(rel.clone(), (url, sha1.clone(), sha512.or(Some(local512.clone())), *size));
                }
            }
        }
        PackFormat::CurseForge => {
            let mut by_fp = HashMap::new();
            for (rel, path) in &candidates {
                by_fp.insert(curseforge::fingerprint(&std::fs::read(path)?), rel.clone());
            }
            let found = curseforge::match_fingerprints(api, &by_fp.keys().copied().collect::<Vec<_>>()).await.unwrap_or_default();
            for (fp, ids) in found {
                if let Some(rel) = by_fp.get(&fp) {
                    cf_links.insert(rel.clone(), ids);
                }
            }
        }
        PackFormat::Mcbbs => {}
    }

    let mut zip = zip::ZipWriter::new(std::fs::File::create(out_path)?);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let zerr = |e: zip::result::ZipError| Error::Other(format!("写入整合包失败：{e}"));
    let mut bundled = 0;
    let mut sha1s: Vec<(String, String)> = Vec::new();
    for (i, (rel, path)) in files.iter().enumerate() {
        if !mr_links.contains_key(rel) && !cf_links.contains_key(rel) {
            let data = std::fs::read(path)?;
            if opts.format == PackFormat::Mcbbs {
                sha1s.push((rel.clone(), hex::encode(Sha1::digest(&data))));
            }
            zip.start_file(format!("overrides/{rel}"), opt).map_err(zerr)?;
            zip.write_all(&data)?;
            bundled += 1;
        }
        report(i + 1);
    }

    let loader = env.loader.zip(env.loader_version.clone());
    match opts.format {
        PackFormat::Modrinth => {
            let mut deps = serde_json::Map::new();
            deps.insert("minecraft".into(), json!(env.mc));
            if let Some((l, v)) = &loader {
                deps.insert(loader_key(*l, true).into(), json!(v));
            }
            let mut entries: Vec<_> = mr_links.iter().collect();
            entries.sort_by_key(|(rel, _)| rel.as_str());
            let index = json!({
                "formatVersion": 1,
                "game": "minecraft",
                "versionId": opts.version,
                "name": opts.name,
                "summary": opts.summary,
                "files": entries.iter().map(|(rel, (url, sha1, sha512, size))| json!({
                    "path": rel,
                    "hashes": {"sha1": sha1, "sha512": sha512},
                    "env": {"client": "required", "server": "required"},
                    "downloads": [url],
                    "fileSize": size,
                })).collect::<Vec<_>>(),
                "dependencies": deps,
            });
            zip.start_file("modrinth.index.json", opt).map_err(zerr)?;
            zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;
        }
        PackFormat::CurseForge | PackFormat::Mcbbs => {
            let mut refs: Vec<_> = cf_links.values().collect();
            refs.sort();
            let manifest = json!({
                "minecraft": {
                    "version": env.mc,
                    "modLoaders": loader.iter().map(|(l, v)| json!({"id": format!("{}-{v}", loader_key(*l, false)), "primary": true})).collect::<Vec<_>>(),
                },
                "manifestType": "minecraftModpack",
                "manifestVersion": 1,
                "name": opts.name,
                "version": opts.version,
                "author": opts.author,
                "files": refs.iter().map(|(p, f)| json!({"projectID": p, "fileID": f, "required": true})).collect::<Vec<_>>(),
                "overrides": "overrides",
            });
            zip.start_file("manifest.json", opt).map_err(zerr)?;
            zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
            if opts.format == PackFormat::Mcbbs {
                let mut addons = vec![json!({"id": "game", "version": env.mc})];
                if let Some((l, v)) = &loader {
                    addons.push(json!({"id": loader_key(*l, false), "version": v}));
                }
                let meta = json!({
                    "manifestType": "minecraftModpack",
                    "manifestVersion": 2,
                    "name": opts.name,
                    "version": opts.version,
                    "author": opts.author,
                    "description": opts.summary,
                    "files": sha1s.iter().map(|(p, h)| json!({"path": p, "hash": h, "force": true, "type": "addon"})).collect::<Vec<_>>(),
                    "addons": addons,
                    "launchInfo": {
                        "minMemory": env.min_memory_mb.unwrap_or(0),
                        "launchArgument": env.game_args,
                        "javaArgument": env.jvm_args,
                    },
                });
                zip.start_file("mcbbs.packmeta", opt).map_err(zerr)?;
                zip.write_all(serde_json::to_string_pretty(&meta)?.as_bytes())?;
            }
        }
    }
    zip.finish().map_err(zerr)?;
    Ok(ExportSummary { path: out_path.display().to_string(), linked: mr_links.len() + cf_links.len(), bundled })
}
