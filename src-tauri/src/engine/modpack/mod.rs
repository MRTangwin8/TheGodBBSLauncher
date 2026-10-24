//! 整合包：导入 / 导出 Modrinth（.mrpack）、CurseForge（manifest.json）、MCBBS（mcbbs.packmeta）三种格式。

pub mod export;

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::engine::loader::LoaderKind;
use crate::engine::resources::curseforge;
use crate::engine::resources::ResourceApi;
use crate::engine::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackFormat {
    Modrinth,
    CurseForge,
    Mcbbs,
}

impl PackFormat {
    pub fn label(self) -> &'static str {
        match self {
            PackFormat::Modrinth => "Modrinth",
            PackFormat::CurseForge => "CurseForge",
            PackFormat::Mcbbs => "MCBBS",
        }
    }
}

/// 导入前给用户看的信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInfo {
    pub format: PackFormat,
    pub name: String,
    pub version: String,
    pub author: String,
    pub summary: String,
    pub mc: String,
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<String>,
    /// 需要下载的文件数
    pub file_count: usize,
    /// 包内直接带的文件数（overrides）
    pub override_count: usize,
    pub min_memory_mb: Option<u32>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub warnings: Vec<String>,
}

/// 需要下载的文件，path 相对游戏目录
#[derive(Debug, Clone, PartialEq)]
pub struct PackFile {
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_id: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

fn default_overrides() -> String {
    "overrides".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Debug, Clone, Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileRef {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

fn yes() -> bool {
    true
}

/// HMCL 定义的 MCBBS 整合包格式
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McbbsMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    file_api: Option<String>,
    #[serde(default)]
    files: Vec<Value>,
    #[serde(default)]
    addons: Vec<McbbsAddon>,
    #[serde(default)]
    launch_info: Option<McbbsLaunch>,
}

#[derive(Debug, Clone, Deserialize)]
struct McbbsAddon {
    id: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McbbsLaunch {
    #[serde(default)]
    min_memory: Option<u32>,
    #[serde(default)]
    launch_argument: Vec<String>,
    #[serde(default)]
    java_argument: Vec<String>,
}

#[derive(Debug, Clone)]
enum Manifest {
    Modrinth(MrIndex),
    CurseForge(CfManifest),
    Mcbbs(McbbsMeta),
}

pub struct Modpack {
    pub path: PathBuf,
    pub info: ModpackInfo,
    /// 清单所在的目录前缀（有的包外面多套了一层文件夹）
    prefix: String,
    manifest: Manifest,
}

/// 包内路径只能是相对路径且不能跳出游戏目录（防 zip slip）
pub fn safe_rel(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    if p.starts_with('/') || p.contains(':') {
        return None;
    }
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }
    Some(parts.join("/"))
}

fn join_safe(base: &Path, rel: &str) -> Option<PathBuf> {
    safe_rel(rel).map(|r| r.split('/').fold(base.to_path_buf(), |p, s| p.join(s)))
}

fn loader_from_id(id: &str) -> Option<LoaderKind> {
    match id {
        "forge" => Some(LoaderKind::Forge),
        "neoforge" => Some(LoaderKind::NeoForge),
        "fabric" | "fabric-loader" => Some(LoaderKind::Fabric),
        "quilt" | "quilt-loader" => Some(LoaderKind::Quilt),
        _ => None,
    }
}

fn read_text(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String> {
    let mut e = zip.by_name(name).map_err(|_| Error::Other(format!("整合包里没有 {name}")))?;
    let mut s = String::new();
    e.read_to_string(&mut s)?;
    // 有的工具会写 UTF-8 BOM
    Ok(s.trim_start_matches('\u{feff}').to_string())
}

impl Modpack {
    pub fn open(path: &Path) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(|_| Error::Other("不是有效的整合包（zip）文件".into()))?;
        let names: Vec<String> = zip.file_names().map(str::to_string).collect();
        // 取层级最浅的清单
        let find = |file: &str| {
            names
                .iter()
                .filter(|n| *n == file || n.ends_with(&format!("/{file}")))
                .min_by_key(|n| n.matches('/').count())
                .cloned()
        };
        let (manifest_path, manifest) = if let Some(p) = find("modrinth.index.json") {
            let m: MrIndex = serde_json::from_str(&read_text(&mut zip, &p)?)?;
            (p, Manifest::Modrinth(m))
        } else if let Some(p) = find("mcbbs.packmeta") {
            let m: McbbsMeta = serde_json::from_str(&read_text(&mut zip, &p)?)?;
            (p, Manifest::Mcbbs(m))
        } else if let Some(p) = find("manifest.json") {
            let text = read_text(&mut zip, &p)?;
            let v: Value = serde_json::from_str(&text)?;
            if v["manifestType"].as_str() != Some("minecraftModpack") {
                return Err(Error::Other("manifest.json 不是 CurseForge 整合包清单".into()));
            }
            (p, Manifest::CurseForge(serde_json::from_value(v)?))
        } else {
            return Err(Error::Other("没认出整合包格式：需要 modrinth.index.json、manifest.json 或 mcbbs.packmeta".into()));
        };
        let prefix = manifest_path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();

        let override_dirs = override_dirs(&manifest, &prefix);
        let override_count = names.iter().filter(|n| !n.ends_with('/') && override_dirs.iter().any(|d| n.starts_with(d.as_str()))).count();
        let info = match &manifest {
            Manifest::Modrinth(m) => {
                let (loader, loader_version) = m
                    .dependencies
                    .iter()
                    .find_map(|(k, v)| loader_from_id(k).map(|l| (Some(l), Some(v.clone()))))
                    .unwrap_or((None, None));
                ModpackInfo {
                    format: PackFormat::Modrinth,
                    name: m.name.clone(),
                    version: m.version_id.clone(),
                    author: String::new(),
                    summary: m.summary.clone().unwrap_or_default(),
                    mc: m.dependencies.get("minecraft").cloned().ok_or_else(|| Error::Other("整合包没写 Minecraft 版本".into()))?,
                    loader,
                    loader_version,
                    file_count: m.files.iter().filter(|f| client_needed(f)).count(),
                    override_count,
                    min_memory_mb: None,
                    jvm_args: vec![],
                    game_args: vec![],
                    warnings: vec![],
                }
            }
            Manifest::CurseForge(m) => {
                let primary = m.minecraft.mod_loaders.iter().find(|l| l.primary).or(m.minecraft.mod_loaders.first());
                let (loader, loader_version) = primary
                    .and_then(|l| l.id.split_once('-'))
                    .and_then(|(k, v)| loader_from_id(k).map(|k| (Some(k), Some(v.to_string()))))
                    .unwrap_or((None, None));
                ModpackInfo {
                    format: PackFormat::CurseForge,
                    name: m.name.clone(),
                    version: m.version.clone(),
                    author: m.author.clone(),
                    summary: String::new(),
                    mc: m.minecraft.version.clone(),
                    loader,
                    loader_version,
                    file_count: m.files.iter().filter(|f| f.required).count(),
                    override_count,
                    min_memory_mb: None,
                    jvm_args: vec![],
                    game_args: vec![],
                    warnings: vec![],
                }
            }
            Manifest::Mcbbs(m) => {
                let mc = m.addons.iter().find(|a| a.id == "game").map(|a| a.version.clone()).ok_or_else(|| Error::Other("整合包没写 Minecraft 版本".into()))?;
                let loader = m.addons.iter().find_map(|a| loader_from_id(&a.id).map(|l| (l, a.version.clone())));
                let mut warnings = Vec::new();
                if m.addons.iter().any(|a| a.id == "optifine") {
                    warnings.push("整合包要求 OptiFine，启动器暂不支持自动安装，需要手动放进 mods".into());
                }
                let launch = m.launch_info.clone();
                ModpackInfo {
                    format: PackFormat::Mcbbs,
                    name: m.name.clone(),
                    version: m.version.clone(),
                    author: m.author.clone(),
                    summary: m.description.clone(),
                    mc,
                    loader: loader.as_ref().map(|(l, _)| *l),
                    loader_version: loader.map(|(_, v)| v),
                    file_count: m.files.iter().filter(|f| mcbbs_needs_download(f, &m.file_api)).count(),
                    override_count,
                    min_memory_mb: launch.as_ref().and_then(|l| l.min_memory).filter(|m| *m > 0),
                    jvm_args: launch.as_ref().map(|l| l.java_argument.clone()).unwrap_or_default(),
                    game_args: launch.map(|l| l.launch_argument).unwrap_or_default(),
                    warnings,
                }
            }
        };
        Ok(Self { path: path.to_path_buf(), info, prefix, manifest })
    }

    /// 需要下载的文件；CurseForge 引用要查 API 换成下载地址
    pub async fn files(&self, api: &ResourceApi) -> Result<(Vec<PackFile>, Vec<String>)> {
        let mut warnings = Vec::new();
        let mut out = Vec::new();
        match &self.manifest {
            Manifest::Modrinth(m) => {
                for f in m.files.iter().filter(|f| client_needed(f)) {
                    let (Some(path), Some(url)) = (safe_rel(&f.path), f.downloads.first()) else {
                        warnings.push(format!("跳过不安全或没有下载地址的文件：{}", f.path));
                        continue;
                    };
                    out.push(PackFile { path, url: url.clone(), sha1: f.hashes.get("sha1").cloned(), size: f.file_size });
                }
            }
            Manifest::CurseForge(m) => {
                let refs: Vec<(u64, u64)> = m.files.iter().filter(|f| f.required).map(|f| (f.project_id, f.file_id)).collect();
                out.extend(resolve_curseforge(api, &refs, &mut warnings).await?);
            }
            Manifest::Mcbbs(m) => {
                let mut curse = Vec::new();
                for f in &m.files {
                    match f["type"].as_str() {
                        Some("curse") => {
                            if let (Some(p), Some(id)) = (f["projectID"].as_u64(), f["fileID"].as_u64()) {
                                curse.push((p, id));
                            }
                        }
                        _ if mcbbs_needs_download(f, &m.file_api) => {
                            let (Some(path), Some(api_base)) = (f["path"].as_str().and_then(safe_rel), m.file_api.as_deref()) else { continue };
                            out.push(PackFile {
                                url: format!("{}/{}", api_base.trim_end_matches('/'), path),
                                sha1: f["hash"].as_str().map(str::to_string),
                                size: 0,
                                path,
                            });
                        }
                        _ => {}
                    }
                }
                out.extend(resolve_curseforge(api, &curse, &mut warnings).await?);
            }
        }
        Ok((out, warnings))
    }

    /// 把包内直接带的文件解压到游戏目录，返回文件数
    pub fn extract_overrides(&self, game_dir: &Path) -> Result<usize> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&self.path)?).map_err(|e| Error::Other(e.to_string()))?;
        let mut count = 0;
        // 顺序有意义：mrpack 的 client-overrides 覆盖 overrides
        for dir in override_dirs(&self.manifest, &self.prefix) {
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
                let name = entry.name().to_string();
                let Some(rel) = name.strip_prefix(&dir) else { continue };
                if entry.is_dir() || rel.is_empty() {
                    continue;
                }
                let Some(dest) = join_safe(game_dir, rel) else { continue };
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(&mut entry, &mut out)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

fn override_dirs(manifest: &Manifest, prefix: &str) -> Vec<String> {
    match manifest {
        Manifest::Modrinth(_) => vec![format!("{prefix}overrides/"), format!("{prefix}client-overrides/")],
        Manifest::CurseForge(m) => vec![format!("{prefix}{}/", m.overrides.trim_matches('/'))],
        Manifest::Mcbbs(_) => vec![format!("{prefix}overrides/")],
    }
}

fn client_needed(f: &MrFile) -> bool {
    f.env.as_ref().and_then(|e| e.get("client")).is_none_or(|c| c != "unsupported")
}

/// MCBBS 的 addon 文件一般已经在 overrides 里；只有提供了 fileApi 时才需要下载
fn mcbbs_needs_download(f: &Value, file_api: &Option<String>) -> bool {
    f["type"].as_str() == Some("curse") || (file_api.as_deref().is_some_and(|a| !a.is_empty()) && f["path"].is_string())
}

/// CurseForge 的 (projectID, fileID) → 下载地址，按项目类型放进 mods / resourcepacks / shaderpacks
async fn resolve_curseforge(api: &ResourceApi, refs: &[(u64, u64)], warnings: &mut Vec<String>) -> Result<Vec<PackFile>> {
    if refs.is_empty() {
        return Ok(vec![]);
    }
    let ids: Vec<u64> = refs.iter().map(|(_, f)| *f).collect();
    let files = curseforge::files_by_ids(api, &ids).await?;
    let mod_ids: Vec<String> = refs.iter().map(|(p, _)| p.to_string()).collect();
    let classes = curseforge::mod_classes(api, &mod_ids).await.unwrap_or_default();
    let found: std::collections::HashSet<String> = files.iter().map(|v| v.id.clone()).collect();
    for (p, f) in refs {
        if !found.contains(&f.to_string()) {
            warnings.push(format!("CurseForge 上找不到文件 {f}（项目 {p}），可能已被作者删除"));
        }
    }
    Ok(files
        .into_iter()
        .filter_map(|v| {
            let file = v.file?;
            let folder = match classes.get(&v.project_id) {
                Some(12) => "resourcepacks",
                Some(6552) => "shaderpacks",
                _ => "mods",
            };
            Some(PackFile { path: format!("{folder}/{}", file.filename), url: file.url, sha1: file.sha1, size: file.size })
        })
        .collect())
}

#[cfg(test)]
mod tests;
