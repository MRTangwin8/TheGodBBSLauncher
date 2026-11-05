//! OptiFine：版本列表和安装包都来自 BMCLAPI（OptiFine 官网没有可供程序调用的下载接口）。
//!
//! 两种装法：
//! - **单独安装**（原版 + OptiFine）：用安装包自带的 `optifine.Patcher <原版 jar> <安装包> <输出>` 生成打好补丁的库，
//!   再把包里的 `launchwrapper-of-<版本>.jar` 放进 libraries，写一个继承原版、主类为 LaunchWrapper、
//!   带 `--tweakClass optifine.OptiFineTweaker` 的版本 json。
//! - **作为模组装进 Forge**：安装包本身就是 Forge 模组，直接放进 mods（见 `commands::loaders::install_optifine`）。
//!
//! 版本号、文件名都来自镜像接口，按不可信数据处理：只接受字母数字和下划线。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::engine::download::{DownloadTask, Downloader};
use crate::engine::install::{join_rel, GameDir};
use crate::engine::meta::version::maven_path;
use crate::engine::meta::VersionJson;
use crate::engine::{Error, Result};

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/optifine";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptiFineVersion {
    /// 版本类型，如 `HD_U`、`HD_U_J1`（预览版的类型里带着正式版号）
    #[serde(rename = "type")]
    pub kind: String,
    /// 补丁号，如 `J1`、`pre10`
    pub patch: String,
    /// 推荐搭配的 Forge，如 `Forge 52.0.16`；不能装进 Forge 的是 None
    pub forge: Option<String>,
    pub preview: bool,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(rename = "type")]
    kind: String,
    patch: String,
    #[serde(default)]
    filename: String,
    #[serde(default)]
    forge: Option<String>,
}

fn token_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn parse(list: Vec<Raw>) -> Vec<OptiFineVersion> {
    let mut out: Vec<OptiFineVersion> = list
        .into_iter()
        .filter(|r| token_ok(&r.kind) && token_ok(&r.patch))
        .map(|r| OptiFineVersion {
            preview: r.filename.starts_with("preview") || r.patch.starts_with("pre"),
            forge: r.forge.filter(|f| !f.contains("N/A") && !f.trim().is_empty()).map(|f| f.chars().take(40).collect()),
            kind: r.kind,
            patch: r.patch,
        })
        .collect();
    // 接口返回的顺序不固定：正式版在前，同类里补丁号大的（新的）在前
    out.sort_by(|a, b| a.preview.cmp(&b.preview).then_with(|| b.kind.cmp(&a.kind)).then_with(|| b.patch.cmp(&a.patch)));
    out
}

pub async fn list(client: &reqwest::Client, mc: &str) -> Result<Vec<OptiFineVersion>> {
    if !crate::engine::safepath::name_ok(mc) {
        return Ok(vec![]);
    }
    let resp = client.get(format!("{BMCLAPI}/{mc}")).send().await?;
    if !resp.status().is_success() {
        return Err(Error::BadStatus { url: format!("{BMCLAPI}/{mc}"), status: resp.status().as_u16() });
    }
    Ok(parse(resp.json::<Vec<Raw>>().await?))
}

impl OptiFineVersion {
    fn check(&self) -> Result<()> {
        if token_ok(&self.kind) && token_ok(&self.patch) {
            Ok(())
        } else {
            Err(Error::Other("OptiFine 版本号不对".into()))
        }
    }

    /// 写进库坐标和版本名的部分，如 `HD_U_J1`
    pub fn edition(&self) -> String {
        format!("{}_{}", self.kind, self.patch)
    }

    /// 安装包文件名
    pub fn jar_name(&self, mc: &str) -> String {
        format!("OptiFine_{mc}_{}.jar", self.edition())
    }
}

pub fn default_version_name(mc: &str, v: &OptiFineVersion) -> String {
    format!("{mc}-OptiFine_{}", v.edition())
}

/// 下载安装包到 `dir`，返回路径；下完检查一下确实是 OptiFine 的安装包
pub async fn download(downloader: &Downloader, dir: &Path, mc: &str, v: &OptiFineVersion, report: crate::engine::download::ProgressFn) -> Result<PathBuf> {
    v.check()?;
    if !crate::engine::safepath::name_ok(mc) {
        return Err(Error::VersionNotFound(mc.to_string()));
    }
    let path = dir.join(v.jar_name(mc));
    let url = format!("{BMCLAPI}/{mc}/{}/{}", v.kind, v.patch);
    let r = downloader.download_all(vec![DownloadTask { url, path: path.clone(), sha1: None, size: None }], report).await;
    if let Some(f) = r.failed.first() {
        return Err(Error::coded("optifine.download", &[("reason", &f.reason)], format!("OptiFine 下载失败：{}", f.reason)));
    }
    let p = path.clone();
    let ok = tokio::task::spawn_blocking(move || is_optifine_jar(&p)).await.unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&path);
        return Err(Error::coded("optifine.badJar", &[], "下载到的文件不是 OptiFine 安装包，已删除，请稍后重试"));
    }
    Ok(path)
}

fn is_optifine_jar(path: &Path) -> bool {
    std::fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|mut z| z.by_name("optifine/Patcher.class").is_ok())
}

/// 读出安装包里的 launchwrapper 版本号（`launchwrapper-of.txt`），并把对应的 jar 解压到 `dest`
fn extract_launchwrapper(installer: &Path, libraries: &Path) -> Result<String> {
    let zerr = |e: zip::result::ZipError| Error::Other(format!("OptiFine 安装包损坏：{e}"));
    let mut zip = zip::ZipArchive::new(std::fs::File::open(installer)?).map_err(zerr)?;
    let mut ver = String::new();
    zip.by_name("launchwrapper-of.txt").map_err(zerr)?.read_to_string(&mut ver)?;
    let ver = ver.trim().to_string();
    if !ver.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.') || ver.is_empty() {
        return Err(Error::Other("OptiFine 安装包里的 launchwrapper 版本号不对".into()));
    }
    let dest = join_rel(libraries, &maven_path(&format!("optifine:launchwrapper-of:{ver}")).unwrap_or_default());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut entry = zip.by_name(&format!("launchwrapper-of-{ver}.jar")).map_err(zerr)?;
    std::io::copy(&mut entry, &mut std::fs::File::create(&dest)?)?;
    Ok(ver)
}

/// 版本 json：继承原版，主类换成 LaunchWrapper，加上 OptiFine 的 tweaker
fn version_json(name: &str, mc: &str, vanilla: &VersionJson, edition: &str, lw: &str) -> serde_json::Value {
    let local_lib = |coord: String| {
        let path = maven_path(&coord).unwrap_or_default();
        // url 为空：库是安装时在本地生成的，启动前补全时不下载
        json!({ "name": coord, "downloads": { "artifact": { "path": path, "url": "", "sha1": null, "size": null } } })
    };
    let mut v = json!({
        "id": name,
        "inheritsFrom": mc,
        "type": vanilla.kind.clone().unwrap_or_else(|| "release".into()),
        "releaseTime": vanilla.release_time,
        "time": vanilla.time,
        "mainClass": "net.minecraft.launchwrapper.Launch",
        "libraries": [local_lib(format!("optifine:launchwrapper-of:{lw}")), local_lib(format!("optifine:OptiFine:{mc}_{edition}"))],
    });
    match &vanilla.minecraft_arguments {
        // 1.12 及以前：minecraftArguments 是整串，子版本的会整个替换父版本的
        Some(args) if vanilla.arguments.is_none() => v["minecraftArguments"] = json!(format!("{args} --tweakClass optifine.OptiFineTweaker")),
        _ => v["arguments"] = json!({ "game": ["--tweakClass", "optifine.OptiFineTweaker"] }),
    }
    v
}

/// 单独安装（原版 + OptiFine）。原版要先装好；`java` 用来运行安装包里的 Patcher
pub async fn install(game: &GameDir, java: &Path, installer: &Path, mc: &str, v: &OptiFineVersion, name: &str) -> Result<String> {
    v.check()?;
    crate::engine::install::validate_version_name(name)?;
    if game.versions().join(name).exists() {
        return Err(Error::coded("install.nameTaken", &[("name", &name)], format!("已经有名为「{name}」的版本了，换个名字吧")));
    }
    let vanilla_jar = game.version_jar(mc);
    if !vanilla_jar.is_file() {
        return Err(Error::coded("loader.needVanilla", &[("mc", &mc)], format!("请先安装原版 {mc}")));
    }
    let vanilla = VersionJson::resolve(&game.versions(), mc).await?;
    let edition = v.edition();
    let libraries = game.libraries();
    let patched = join_rel(&libraries, &maven_path(&format!("optifine:OptiFine:{mc}_{edition}")).unwrap_or_default());
    if let Some(parent) = patched.parent() {
        std::fs::create_dir_all(parent)?;
    }

    crate::engine::cancel::check()?;
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-cp").arg(installer).arg("optifine.Patcher").arg(&vanilla_jar).arg(installer).arg(&patched);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd.output().await.map_err(|e| Error::Other(format!("无法运行 Java：{e}")))?;
    if !out.status.success() || !patched.is_file() {
        let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = log.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(Error::coded("optifine.patch", &[("log", &tail.join("\n"))], format!("OptiFine 安装失败（Patcher 出错）：\n{}", tail.join("\n"))));
    }

    let (inst, libs) = (installer.to_path_buf(), libraries.clone());
    let lw = tokio::task::spawn_blocking(move || extract_launchwrapper(&inst, &libs)).await.map_err(|e| Error::Other(e.to_string()))??;

    let dir = game.versions().join(name);
    std::fs::create_dir_all(&dir)?;
    let json = version_json(name, mc, &vanilla, &edition, &lw);
    std::fs::write(game.version_json(name), serde_json::to_string_pretty(&json)?)?;
    Ok(name.to_string())
}

/// 已安装版本里的 OptiFine 版本（库坐标 `optifine:OptiFine:<mc>_<edition>`），没有返回 None
pub fn detect(version: &VersionJson) -> Option<String> {
    version.libraries.iter().find_map(|l| {
        let rest = l.name.strip_prefix("optifine:OptiFine:")?;
        Some(rest.split_once('_').map(|(_, e)| e).unwrap_or(rest).to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(kind: &str, patch: &str, filename: &str, forge: &str) -> Raw {
        Raw { kind: kind.into(), patch: patch.into(), filename: filename.into(), forge: Some(forge.into()) }
    }

    #[test]
    fn parses_sorts_and_drops_bad_entries() {
        let list = parse(vec![
            raw("HD_U_J1", "pre10", "preview_OptiFine_1.21.1_HD_U_J1_pre10.jar", "Forge 52.0.10"),
            raw("HD_U", "J1", "OptiFine_1.21.1_HD_U_J1.jar", "Forge 52.0.16"),
            raw("HD_U", "I9", "OptiFine_1.21.1_HD_U_I9.jar", "Forge N/A"),
            raw("HD_U", "../x", "evil.jar", ""),
        ]);
        assert_eq!(list.len(), 3, "坏的版本号丢掉");
        assert_eq!((list[0].edition(), list[0].preview), ("HD_U_J1".to_string(), false));
        assert_eq!(list[0].forge.as_deref(), Some("Forge 52.0.16"));
        assert_eq!(list[1].edition(), "HD_U_I9");
        assert_eq!(list[1].forge, None, "Forge N/A 表示不能装进 Forge");
        assert!(list[2].preview);
    }

    #[test]
    fn version_json_for_new_and_old_versions() {
        let new: VersionJson = serde_json::from_str(r#"{"id":"1.21.1","type":"release","releaseTime":"2024-08-08T12:24:45+00:00","arguments":{"game":["--username","${auth_player_name}"]}}"#).unwrap();
        let v = version_json("1.21.1-OptiFine_HD_U_J1", "1.21.1", &new, "HD_U_J1", "2.3");
        assert_eq!(v["inheritsFrom"], "1.21.1");
        assert_eq!(v["mainClass"], "net.minecraft.launchwrapper.Launch");
        assert_eq!(v["arguments"]["game"], json!(["--tweakClass", "optifine.OptiFineTweaker"]));
        assert_eq!(v["libraries"][1]["downloads"]["artifact"]["path"], "optifine/OptiFine/1.21.1_HD_U_J1/OptiFine-1.21.1_HD_U_J1.jar");
        let parsed: VersionJson = serde_json::from_value(v).unwrap();
        assert!(parsed.libraries.iter().all(|l| l.artifact().is_none()), "本地生成的库不下载");
        assert_eq!(detect(&parsed).as_deref(), Some("HD_U_J1"));

        let old: VersionJson = serde_json::from_str(r#"{"id":"1.12.2","minecraftArguments":"--username ${auth_player_name} --version ${version_name}"}"#).unwrap();
        let v = version_json("x", "1.12.2", &old, "HD_U_G5", "2.2");
        assert_eq!(v["minecraftArguments"], "--username ${auth_player_name} --version ${version_name} --tweakClass optifine.OptiFineTweaker");
        assert!(v.get("arguments").is_none());
    }

    #[test]
    fn rejects_odd_versions() {
        let v = OptiFineVersion { kind: "HD_U".into(), patch: "J1/../../x".into(), forge: None, preview: false };
        assert!(v.check().is_err());
    }
}
