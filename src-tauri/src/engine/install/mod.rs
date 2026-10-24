//! 原版安装：版本 json → 资源索引 → client.jar / 库 / natives / 资源 / 日志配置。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::engine::download::{DownloadReport, DownloadTask, Downloader, Progress};
use crate::engine::meta::{AssetIndex, ManifestEntry, Platform, VersionJson, VersionManifest, MANIFEST_URL};
use crate::engine::mirror::{self, Source};
use crate::engine::{Error, Result};

const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

/// 游戏根目录（.minecraft）里的各个路径
#[derive(Debug, Clone)]
pub struct GameDir {
    pub root: PathBuf,
}

impl GameDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.versions().join(id).join(format!("{id}.jar"))
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }
}

/// 按源优先级依次尝试，拿到第一个成功的 JSON
pub async fn fetch_json<T: DeserializeOwned>(client: &reqwest::Client, url: &str, primary: Source) -> Result<T> {
    let mut last_err = None;
    for candidate in mirror::candidates(url, primary) {
        let r = async {
            let resp = client.get(&candidate).send().await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(Error::BadStatus { url: candidate.clone(), status: status.as_u16() });
            }
            Ok(resp.json::<T>().await?)
        }
        .await;
        match r {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| Error::Other(format!("无法获取 {url}"))))
}

pub async fn fetch_manifest(client: &reqwest::Client, primary: Source) -> Result<VersionManifest> {
    fetch_json(client, MANIFEST_URL, primary).await
}

/// 安装过程中报告给前端的阶段
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    VersionJson,
    AssetIndex,
    Files,
    Java,
    /// 下载加载器和它的库
    Loader,
    /// 运行 Forge 安装器 processors（totalFiles = 步骤数）
    Processor,
}

pub type ReportFn = Arc<dyn Fn(Stage, Progress) + Send + Sync>;

/// 安装（或补全）一个原版版本。已存在且校验通过的文件会被跳过，所以重复调用就是"补全文件"。
pub async fn install_vanilla(
    downloader: &Downloader,
    game: &GameDir,
    entry: &ManifestEntry,
    platform: &Platform,
    report: ReportFn,
) -> Result<DownloadReport> {
    let stage_fn = |stage: Stage| -> Arc<dyn Fn(Progress) + Send + Sync> {
        let report = report.clone();
        Arc::new(move |p| report(stage, p))
    };

    let json_path = game.version_json(&entry.id);
    let r = downloader
        .download_all(
            vec![DownloadTask { url: entry.url.clone(), path: json_path.clone(), sha1: entry.sha1.clone(), size: None }],
            stage_fn(Stage::VersionJson),
        )
        .await;
    ensure_ok(&r)?;
    let version: VersionJson = serde_json::from_str(&tokio::fs::read_to_string(&json_path).await?)?;

    let asset_index = match &version.asset_index {
        Some(idx) => {
            let path = game.asset_index(&idx.id);
            let r = downloader
                .download_all(
                    vec![DownloadTask { url: idx.url.clone(), path: path.clone(), sha1: idx.sha1.clone(), size: idx.size }],
                    stage_fn(Stage::AssetIndex),
                )
                .await;
            ensure_ok(&r)?;
            Some(serde_json::from_str::<AssetIndex>(&tokio::fs::read_to_string(&path).await?)?)
        }
        None => None,
    };

    let tasks = collect_tasks(game, &version, asset_index.as_ref(), platform);
    Ok(downloader.download_all(tasks, stage_fn(Stage::Files)).await)
}

fn ensure_ok(r: &DownloadReport) -> Result<()> {
    match r.failed.first() {
        Some(f) => Err(Error::Other(format!("下载 {} 失败：{}", f.path, f.reason))),
        None => Ok(()),
    }
}

/// 根据（已合并继承的）版本信息列出所有需要下载的文件
pub fn collect_tasks(
    game: &GameDir,
    version: &VersionJson,
    asset_index: Option<&AssetIndex>,
    platform: &Platform,
) -> Vec<DownloadTask> {
    let mut tasks = Vec::new();

    if let Some(client) = version.downloads.get("client") {
        tasks.push(DownloadTask {
            url: client.url.clone(),
            path: game.version_jar(version.jar_id()),
            sha1: client.sha1.clone(),
            size: client.size,
        });
    }

    let libs = game.libraries();
    for lib in version.libraries.iter().filter(|l| l.is_allowed(platform)) {
        for file in [lib.artifact(), lib.native_artifact(platform)].into_iter().flatten() {
            tasks.push(DownloadTask {
                url: file.url,
                path: join_rel(&libs, &file.path),
                sha1: file.sha1,
                size: file.size,
            });
        }
    }

    if let Some(file) = version.logging.as_ref().and_then(|l| l.client.as_ref()).map(|c| &c.file) {
        tasks.push(DownloadTask {
            url: file.url.clone(),
            path: game.assets().join("log_configs").join(&file.id),
            sha1: file.sha1.clone(),
            size: file.size,
        });
    }

    if let Some(index) = asset_index {
        let objects = game.assets().join("objects");
        // hash 会被拼进路径，只接受 40 位十六进制
        let valid = |h: &str| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit());
        for obj in index.objects.values().filter(|o| valid(&o.hash)) {
            let prefix = &obj.hash[..2];
            tasks.push(DownloadTask {
                url: format!("{RESOURCES_URL}/{prefix}/{}", obj.hash),
                path: objects.join(prefix).join(&obj.hash),
                sha1: Some(obj.hash.clone()),
                size: Some(obj.size),
            });
        }
    }

    tasks
}

/// 把 `a/b/c.jar` 这种正斜杠相对路径拼到目录下（Windows 也能用）
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|s| !s.is_empty() && *s != "..").fold(base.to_path_buf(), |p, seg| p.join(seg))
}

/// 版本名会成为文件夹名，不能有路径分隔符和 Windows 不允许的字符
pub fn validate_version_name(name: &str) -> Result<()> {
    let bad = name.is_empty()
        || name != name.trim()
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name.chars().any(|c| "/\\:*?\"<>|".contains(c) || c.is_control());
    if bad {
        return Err(Error::Other(format!("版本名「{name}」不合法：不能为空，不能包含 / \\ : * ? \" < > |")));
    }
    Ok(())
}

/// 重命名版本：整个文件夹一起改名（版本隔离下的存档、模组跟着走），再改 json 里的 id 和 jar 文件名
pub fn rename_version(game: &GameDir, old: &str, new: &str) -> Result<()> {
    if old == new {
        return Ok(());
    }
    validate_version_name(new)?;
    if game.versions().join(new).exists() {
        return Err(Error::Other(format!("已经有名为「{new}」的版本了")));
    }
    // 被别的版本继承时改名会把它们弄坏
    if let Ok(entries) = std::fs::read_dir(game.versions()) {
        for e in entries.flatten() {
            let id = e.file_name().to_string_lossy().to_string();
            if id == old {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(game.version_json(&id)) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            if v["inheritsFrom"].as_str() == Some(old) {
                return Err(Error::Other(format!("版本「{id}」依赖「{old}」，不能改名")));
            }
        }
    }
    let old_json = game.version_json(old);
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&old_json).map_err(|_| Error::VersionNotFound(old.into()))?)?;
    std::fs::rename(game.versions().join(old), game.versions().join(new))?;
    let dir = game.versions().join(new);
    v["id"] = serde_json::Value::String(new.to_string());
    std::fs::write(game.version_json(new), serde_json::to_string_pretty(&v)?)?;
    let _ = std::fs::remove_file(dir.join(format!("{old}.json")));
    let old_jar = dir.join(format!("{old}.jar"));
    if old_jar.is_file() {
        std::fs::rename(old_jar, game.version_jar(new))?;
    }
    Ok(())
}

/// 已安装版本（扫描 versions 目录）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    pub id: String,
    pub kind: Option<String>,
    pub inherits_from: Option<String>,
    pub release_time: Option<String>,
    pub has_jar: bool,
    pub loader: Option<crate::engine::loader::LoaderKind>,
    /// 实际的 Minecraft 版本（沿 inheritsFrom 找到底）
    pub mc_version: Option<String>,
}

pub async fn list_installed(game: &GameDir) -> Result<Vec<InstalledVersion>> {
    let mut out = Vec::new();
    let mut dir = match tokio::fs::read_dir(game.versions()).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    while let Some(entry) = dir.next_entry().await? {
        let id = entry.file_name().to_string_lossy().to_string();
        let Ok(text) = tokio::fs::read_to_string(game.version_json(&id)).await else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<VersionJson>(&text) else {
            continue;
        };
        out.push(InstalledVersion {
            has_jar: tokio::fs::try_exists(game.version_jar(v.jar.as_deref().unwrap_or(&id))).await.unwrap_or(false)
                || v.inherits_from.is_some(),
            mc_version: crate::engine::meta::version::base_minecraft_version(&game.versions(), &id).await.ok(),
            id,
            loader: crate::engine::loader::detect(&v),
            kind: v.kind,
            inherits_from: v.inherits_from,
            release_time: v.release_time,
        });
    }
    out.sort_by(|a, b| b.release_time.cmp(&a.release_time));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_all_file_kinds() {
        let version: VersionJson = serde_json::from_str(r#"{
            "id": "1.20.1",
            "downloads": {"client": {"sha1": "c1", "size": 10, "url": "https://piston-data.mojang.com/c.jar"}},
            "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml",
                "file": {"id": "client-1.12.xml", "sha1": "x1", "size": 3, "url": "https://piston-data.mojang.com/client-1.12.xml"}}},
            "libraries": [
                {"name": "a:b:1", "downloads": {"artifact": {"path": "a/b/1/b-1.jar", "sha1": "s", "size": 1, "url": "https://libraries.minecraft.net/a/b/1/b-1.jar"}}},
                {"name": "mac:only:1", "rules": [{"action": "allow", "os": {"name": "osx"}}],
                 "downloads": {"artifact": {"path": "mac/only/1/only-1.jar", "url": "https://libraries.minecraft.net/x.jar"}}}
            ]
        }"#).unwrap();
        let index: AssetIndex = serde_json::from_str(
            r#"{"objects": {"icons/icon.png": {"hash": "bdf48ef6b5d0d23bbb02e17d04865216179f510a", "size": 3665}}}"#,
        ).unwrap();
        let game = GameDir::new("/mc");
        let win = Platform { os: "windows", arch: "x86_64", os_version: String::new() };
        let tasks = collect_tasks(&game, &version, Some(&index), &win);
        assert_eq!(tasks.len(), 4);
        assert_eq!(tasks[0].path, game.version_jar("1.20.1"));
        assert_eq!(tasks[1].path, Path::new("/mc").join("libraries").join("a").join("b").join("1").join("b-1.jar"));
        assert_eq!(tasks[2].path, Path::new("/mc").join("assets").join("log_configs").join("client-1.12.xml"));
        let asset = &tasks[3];
        assert_eq!(asset.url, "https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a");
        assert_eq!(asset.sha1.as_deref(), Some("bdf48ef6b5d0d23bbb02e17d04865216179f510a"));
    }

    #[test]
    fn renames_version_folder_json_and_jar() {
        let dir = tempfile::tempdir().unwrap();
        let game = GameDir::new(dir.path());
        let old = game.versions().join("neoforge-21.11.48");
        std::fs::create_dir_all(old.join("saves").join("world")).unwrap();
        std::fs::write(old.join("neoforge-21.11.48.json"), r#"{"id":"neoforge-21.11.48","inheritsFrom":"1.21.11"}"#).unwrap();
        std::fs::write(old.join("neoforge-21.11.48.jar"), b"jar").unwrap();

        rename_version(&game, "neoforge-21.11.48", "1.21.11-NeoForge-21.11.48").unwrap();
        let new = game.versions().join("1.21.11-NeoForge-21.11.48");
        assert!(!old.exists());
        assert!(new.join("saves").join("world").is_dir(), "隔离目录里的存档要跟着走");
        assert!(game.version_jar("1.21.11-NeoForge-21.11.48").is_file());
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(game.version_json("1.21.11-NeoForge-21.11.48")).unwrap()).unwrap();
        assert_eq!(v["id"], "1.21.11-NeoForge-21.11.48");
        assert_eq!(v["inheritsFrom"], "1.21.11");
        assert!(!new.join("neoforge-21.11.48.json").exists());
    }

    #[test]
    fn rename_rejects_conflicts_and_bad_names() {
        let dir = tempfile::tempdir().unwrap();
        let game = GameDir::new(dir.path());
        for (id, body) in [("1.21.11", r#"{"id":"1.21.11"}"#), ("child", r#"{"id":"child","inheritsFrom":"1.21.11"}"#), ("other", r#"{"id":"other"}"#)] {
            std::fs::create_dir_all(game.versions().join(id)).unwrap();
            std::fs::write(game.version_json(id), body).unwrap();
        }
        assert!(rename_version(&game, "1.21.11", "x").is_err(), "被继承的版本不能改名");
        assert!(rename_version(&game, "other", "child").is_err(), "不能和已有版本重名");
        assert!(rename_version(&game, "other", "a/b").is_err());
        assert!(rename_version(&game, "other", "con?").is_err());
        assert!(rename_version(&game, "other", " other2").is_err());
    }

    #[test]
    fn join_rel_strips_parent_segments() {
        assert_eq!(join_rel(Path::new("/l"), "../../etc/x"), Path::new("/l").join("etc").join("x"));
    }
}
