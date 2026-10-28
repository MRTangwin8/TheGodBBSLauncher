//! 游戏目录里已安装的资源：列出、识别来源、启用/禁用（.disabled 后缀）、删除。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use futures::StreamExt;

use super::{curseforge, modrinth, ResourceApi, ResourceKind, ResourceVersion};
use crate::engine::download::sha1_file;
use crate::engine::loader::LoaderKind;
use crate::engine::{Error, Result};

const DISABLED: &str = ".disabled";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalResource {
    pub filename: String,
    pub enabled: bool,
    pub size: u64,
    /// 通过 Modrinth 哈希识别到的项目
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
}

fn extensions(kind: ResourceKind) -> &'static [&'static str] {
    match kind {
        ResourceKind::Mod => &[".jar"],
        _ => &[".zip"],
    }
}

/// 文件名只能是单个文件名，防止 ../ 之类越出目录
pub fn safe_path(dir: &Path, filename: &str) -> Result<PathBuf> {
    let ok = !filename.is_empty() && !filename.contains(['/', '\\']) && filename != "." && filename != "..";
    if !ok {
        return Err(Error::Other(format!("非法文件名：{filename}")));
    }
    Ok(dir.join(filename))
}

/// 列出目录里的资源文件（不识别来源）
pub fn scan(dir: &Path, kind: ResourceKind) -> Vec<LocalResource> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<LocalResource> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            let base = name.strip_suffix(DISABLED).unwrap_or(&name);
            // 资源包/光影也可以是文件夹
            let is_res = extensions(kind).iter().any(|ext| base.to_lowercase().ends_with(ext)) || (kind != ResourceKind::Mod && meta.is_dir());
            is_res.then(|| LocalResource {
                enabled: !name.ends_with(DISABLED),
                size: if meta.is_dir() { 0 } else { meta.len() },
                filename: name,
                project_id: None,
                title: None,
                version: None,
            })
        })
        .collect();
    out.sort_by_key(|r| r.filename.to_lowercase());
    out
}

/// 计算文件 SHA1：文件名 → sha1（文件夹跳过）
async fn hashes(dir: &Path, list: &[LocalResource]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for r in list.iter().filter(|r| r.size > 0) {
        if let Ok(h) = sha1_file(&dir.join(&r.filename)).await {
            out.insert(r.filename.clone(), h);
        }
    }
    out
}

/// 列出并尽量识别来源（识别失败不影响列表）
pub async fn list(client: &reqwest::Client, dir: &Path, kind: ResourceKind) -> Vec<LocalResource> {
    let mut list = scan(dir, kind);
    let by_file = hashes(dir, &list).await;
    let sha1s: Vec<String> = by_file.values().cloned().collect();
    let Ok(found) = modrinth::identify(client, &sha1s).await else { return list };
    let ids: Vec<String> = found.values().map(|(p, _)| p.clone()).collect();
    let titles = modrinth::titles(client, &ids).await.unwrap_or_default();
    for r in &mut list {
        if let Some((pid, ver)) = by_file.get(&r.filename).and_then(|h| found.get(h)) {
            r.title = titles.get(pid).cloned();
            r.project_id = Some(pid.clone());
            r.version = Some(ver.clone());
        }
    }
    list
}

/// 一个可以更新的模组
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdate {
    pub filename: String,
    pub enabled: bool,
    pub project_id: String,
    pub title: String,
    /// 现在装的版本号
    pub current: String,
    pub latest: ResourceVersion,
}

/// latest 是不是 current 的更新：文件不同、发布得更晚，并且新文件名是个普通文件名
fn is_update(current: &ResourceVersion, current_sha1: &str, latest: &ResourceVersion) -> bool {
    let Some(file) = &latest.file else { return false };
    let same_file = file.sha1.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(current_sha1));
    // 两个平台的时间都是 RFC 3339 UTC，按字符串比较即可；作者给旧游戏版本补发的旧分支不算更新
    !same_file && latest.id != current.id && latest.published > current.published && safe_path(Path::new("."), &file.filename).is_ok()
}

/// 检查结果：可更新的模组，以及两个平台都认不出的文件数
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub updates: Vec<ModUpdate>,
    pub unknown: usize,
}

/// 检查 mods 文件夹里的模组有没有新版本（按这个版本的游戏版本和加载器筛选）。
/// 先用 SHA1 问 Modrinth，认不出的再用指纹问 CurseForge；一边出错不影响另一边，两边都失败才报错
pub async fn check_updates(api: &ResourceApi, dir: &Path, game_version: Option<&str>, loader: Option<LoaderKind>) -> Result<UpdateCheck> {
    let list = scan(dir, ResourceKind::Mod);
    let by_file = hashes(dir, &list).await;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mr = modrinth_updates(&api.client, &list, &by_file, game_version, loader).await;
    if let Ok((updates, known)) = &mr {
        out.extend(updates.iter().cloned());
        seen.extend(known.iter().cloned());
    }
    let rest: Vec<&LocalResource> = list.iter().filter(|r| r.size > 0 && !seen.contains(&r.filename)).collect();
    let cf = curseforge_updates(api, dir, &rest, game_version, loader).await;
    if let Ok((updates, known)) = &cf {
        out.extend(updates.iter().cloned());
        seen.extend(known.iter().cloned());
    }
    if let (Err(e), Err(_)) = (&mr, &cf) {
        return Err(Error::Other(format!("检查模组更新失败：{e}")));
    }
    out.sort_by_key(|u| u.title.to_lowercase());
    Ok(UpdateCheck { unknown: list.iter().filter(|r| !seen.contains(&r.filename)).count(), updates: out })
}

/// 返回 (可更新的, Modrinth 认得的文件名)
async fn modrinth_updates(
    client: &reqwest::Client,
    list: &[LocalResource],
    by_file: &HashMap<String, String>,
    game_version: Option<&str>,
    loader: Option<LoaderKind>,
) -> Result<(Vec<ModUpdate>, Vec<String>)> {
    let sha1s: Vec<String> = by_file.values().cloned().collect();
    let current = modrinth::identify_versions(client, &sha1s).await?;
    let known: Vec<String> = sha1s.iter().filter(|h| current.contains_key(*h)).cloned().collect();
    let latest = modrinth::latest_versions(client, &known, game_version, loader).await?;
    let mut out: Vec<ModUpdate> = list
        .iter()
        .filter_map(|r| {
            let h = by_file.get(&r.filename)?;
            let (cur, new) = (current.get(h)?, latest.get(h)?);
            is_update(cur, h, new).then(|| new_update(r, cur, new))
        })
        .collect();
    let ids: Vec<String> = out.iter().map(|u| u.project_id.clone()).collect();
    let titles = modrinth::titles(client, &ids).await.unwrap_or_default();
    for u in &mut out {
        u.title = titles.get(&u.project_id).cloned().unwrap_or_else(|| u.filename.clone());
    }
    let known_files = list.iter().filter(|r| by_file.get(&r.filename).is_some_and(|h| current.contains_key(h))).map(|r| r.filename.clone()).collect();
    Ok((out, known_files))
}

/// 返回 (可更新的, CurseForge 认得的文件名)。CurseForge 没有"按文件查最新版"的接口，只能每个项目查一次文件列表
async fn curseforge_updates(
    api: &ResourceApi,
    dir: &Path,
    list: &[&LocalResource],
    game_version: Option<&str>,
    loader: Option<LoaderKind>,
) -> Result<(Vec<ModUpdate>, Vec<String>)> {
    if list.is_empty() {
        return Ok((vec![], vec![]));
    }
    let paths: Vec<(String, PathBuf)> = list.iter().map(|r| (r.filename.clone(), dir.join(&r.filename))).collect();
    let fps: Vec<(String, u32)> = tokio::task::spawn_blocking(move || {
        paths.into_iter().filter_map(|(name, p)| Some((name, curseforge::fingerprint(&std::fs::read(p).ok()?)))).collect()
    })
    .await
    .unwrap_or_default();
    let matched = curseforge::match_fingerprints(api, &fps.iter().map(|(_, fp)| *fp).collect::<Vec<_>>()).await?;
    // 文件名 → (项目 id, 当前文件 id)
    let found: Vec<(String, u64, u64)> = fps.iter().filter_map(|(name, fp)| matched.get(fp).map(|(p, f)| (name.clone(), *p, *f))).collect();
    if found.is_empty() {
        return Ok((vec![], vec![]));
    }
    let current: HashMap<String, ResourceVersion> =
        curseforge::files_by_ids(api, &found.iter().map(|(_, _, f)| *f).collect::<Vec<_>>()).await?.into_iter().map(|v| (v.id.clone(), v)).collect();
    let mut projects: Vec<u64> = found.iter().map(|(_, p, _)| *p).collect();
    projects.sort_unstable();
    projects.dedup();
    let latest: HashMap<String, ResourceVersion> = futures::stream::iter(projects)
        .map(|p| async move {
            let list = curseforge::versions(api, &p.to_string(), ResourceKind::Mod, game_version, loader).await.ok()?;
            // 已按发布时间从新到旧排好；作者关了第三方下载的文件没有下载地址，跳过
            list.into_iter().find(|v| v.file.is_some()).map(|v| (p.to_string(), v))
        })
        .buffer_unordered(4)
        .filter_map(|x| async move { x })
        .collect()
        .await;
    let mut out = Vec::new();
    for (name, project, file_id) in &found {
        let Some(r) = list.iter().find(|r| &r.filename == name) else { continue };
        let (Some(cur), Some(new)) = (current.get(&file_id.to_string()), latest.get(&project.to_string())) else { continue };
        let cur_sha1 = cur.file.as_ref().and_then(|f| f.sha1.clone()).unwrap_or_default();
        if is_update(cur, &cur_sha1, new) {
            out.push(new_update(r, cur, new));
        }
    }
    let ids: Vec<String> = out.iter().map(|u| u.project_id.clone()).collect();
    let titles = curseforge::titles(api, &ids).await.unwrap_or_default();
    for u in &mut out {
        u.title = titles.get(&u.project_id).cloned().unwrap_or_else(|| u.filename.clone());
    }
    Ok((out, found.into_iter().map(|(name, _, _)| name).collect()))
}

fn new_update(r: &LocalResource, cur: &ResourceVersion, new: &ResourceVersion) -> ModUpdate {
    ModUpdate {
        filename: r.filename.clone(),
        enabled: r.enabled,
        project_id: cur.project_id.clone(),
        title: String::new(),
        current: cur.version_number.clone(),
        latest: new.clone(),
    }
}

/// 已安装的 Modrinth 项目：project_id → 文件名列表
pub async fn installed_projects(client: &reqwest::Client, dir: &Path, kind: ResourceKind) -> HashMap<String, Vec<String>> {
    let list = scan(dir, kind);
    let by_file = hashes(dir, &list).await;
    let sha1s: Vec<String> = by_file.values().cloned().collect();
    let found = modrinth::identify(client, &sha1s).await.unwrap_or_default();
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (file, h) in by_file {
        if let Some((pid, _)) = found.get(&h) {
            out.entry(pid.clone()).or_default().push(file);
        }
    }
    out
}

/// 启用/禁用：加上或去掉 .disabled 后缀，返回新文件名
pub fn set_enabled(dir: &Path, filename: &str, enabled: bool) -> Result<String> {
    let from = safe_path(dir, filename)?;
    let base = filename.strip_suffix(DISABLED).unwrap_or(filename);
    let target = if enabled { base.to_string() } else { format!("{base}{DISABLED}") };
    if target != filename {
        std::fs::rename(&from, dir.join(&target))?;
    }
    Ok(target)
}

pub fn delete(dir: &Path, filename: &str) -> Result<()> {
    let path = safe_path(dir, filename)?;
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_toggles_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("b.jar"), b"x").unwrap();
        std::fs::write(d.join("A.jar.disabled"), b"y").unwrap();
        std::fs::write(d.join("readme.txt"), b"z").unwrap();
        let list = scan(d, ResourceKind::Mod);
        assert_eq!(list.iter().map(|r| (r.filename.as_str(), r.enabled)).collect::<Vec<_>>(), [("A.jar.disabled", false), ("b.jar", true)]);

        assert_eq!(set_enabled(d, "b.jar", false).unwrap(), "b.jar.disabled");
        assert_eq!(set_enabled(d, "A.jar.disabled", true).unwrap(), "A.jar");
        assert!(d.join("A.jar").is_file() && d.join("b.jar.disabled").is_file());

        delete(d, "A.jar").unwrap();
        assert!(!d.join("A.jar").exists());
        assert!(delete(d, "../outside.jar").is_err());
        assert!(set_enabled(d, "sub/x.jar", true).is_err());
    }

    fn ver(id: &str, published: &str, filename: &str, sha1: &str) -> ResourceVersion {
        ResourceVersion {
            source: super::super::ResourceSource::Modrinth,
            id: id.into(),
            project_id: "P".into(),
            name: id.into(),
            version_number: id.into(),
            game_versions: vec![],
            loaders: vec![],
            published: published.into(),
            release_type: "release".into(),
            file: Some(super::super::ResFile { url: "https://cdn.modrinth.com/x".into(), filename: filename.into(), sha1: Some(sha1.into()), size: 1 }),
            dependencies: vec![],
        }
    }

    #[test]
    fn only_newer_different_files_count_as_updates() {
        let cur = ver("a", "2026-01-01T00:00:00Z", "m-1.jar", "aaa");
        assert!(is_update(&cur, "aaa", &ver("b", "2026-02-01T00:00:00Z", "m-2.jar", "bbb")));
        assert!(!is_update(&cur, "AAA", &ver("a", "2026-01-01T00:00:00Z", "m-1.jar", "aaa")), "已经是最新");
        assert!(!is_update(&cur, "aaa", &ver("c", "2025-12-01T00:00:00Z", "m-0.jar", "ccc")), "发布更早的不算更新");
        assert!(!is_update(&cur, "aaa", &ver("d", "2026-03-01T00:00:00Z", "../evil.jar", "ddd")), "文件名带路径的不要");
        let mut no_file = ver("e", "2026-03-01T00:00:00Z", "m.jar", "eee");
        no_file.file = None;
        assert!(!is_update(&cur, "aaa", &no_file));
    }

    #[test]
    fn resource_packs_can_be_folders() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyPack")).unwrap();
        std::fs::write(dir.path().join("pack.zip"), b"x").unwrap();
        assert_eq!(scan(dir.path(), ResourceKind::ResourcePack).len(), 2);
        assert_eq!(scan(dir.path(), ResourceKind::Mod).len(), 0);
    }
}
