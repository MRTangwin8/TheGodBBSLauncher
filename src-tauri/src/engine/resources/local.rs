//! 游戏目录里已安装的资源：列出、识别来源、启用/禁用（.disabled 后缀）、删除。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{modrinth, ResourceKind};
use crate::engine::download::sha1_file;
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

    #[test]
    fn resource_packs_can_be_folders() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyPack")).unwrap();
        std::fs::write(dir.path().join("pack.zip"), b"x").unwrap();
        assert_eq!(scan(dir.path(), ResourceKind::ResourcePack).len(), 2);
        assert_eq!(scan(dir.path(), ResourceKind::Mod).len(), 0);
    }
}
