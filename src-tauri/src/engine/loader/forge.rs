//! Forge / NeoForge：下载官方安装器，解析 install_profile.json，本地执行 processors。
//!
//! - 新格式（1.12.2 末期 ~ 现在）：install_profile 里有 `json`、`data`、`processors`、`libraries`
//! - 旧格式（1.12.2 以前）：install_profile 里有 `install` + `versionInfo`，只需解出 universal jar

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::{fetch_text, maven_versions, sort_newest_first, LoaderContext, LoaderKind, LoaderVersion};
use crate::engine::download::{sha1_file, DownloadTask, Progress};
use crate::engine::install::{collect_tasks, join_rel, Stage};
use crate::engine::meta::version::maven_path;
use crate::engine::meta::{Library, VersionJson};
use crate::engine::mirror;
use crate::engine::{Error, Result};

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEO_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged";

// ---------- 版本列表 ----------

pub async fn list_forge(client: &reqwest::Client, primary: mirror::Source, mc: &str) -> Result<Vec<LoaderVersion>> {
    let xml = fetch_text(client, &format!("{FORGE_MAVEN}/maven-metadata.xml"), primary).await?;
    // 推荐版只是锦上添花，拿不到不影响
    let recommended = async {
        let v: Value = client.get(FORGE_PROMOS).timeout(Duration::from_secs(5)).send().await.ok()?.json().await.ok()?;
        v["promos"][format!("{mc}-recommended")].as_str().map(str::to_string)
    }
    .await;
    let prefix = format!("{mc}-");
    let suffix = format!("-{mc}");
    let mut list: Vec<LoaderVersion> = maven_versions(&xml)
        .into_iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|full| {
            let short = full[prefix.len()..].trim_end_matches(&suffix).to_string();
            LoaderVersion { stable: recommended.as_deref() == Some(short.as_str()), version: short, id: full }
        })
        .collect();
    sort_newest_first(&mut list);
    Ok(list)
}

/// NeoForge 版本号 → Minecraft 版本：21.1.77 → 1.21.1，21.0.5 → 1.21，26.3.0.97 → 26.3
pub fn neoforge_mc(version: &str) -> Option<String> {
    let nums: Vec<u32> = version.split(['.', '-']).take(3).map_while(|s| s.parse().ok()).collect();
    match nums.as_slice() {
        [a, b, c, ..] if *a >= 26 => Some(if *c == 0 { format!("{a}.{b}") } else { format!("{a}.{b}.{c}") }),
        [a, b, ..] if *a < 26 => Some(if *b == 0 { format!("1.{a}") } else { format!("1.{a}.{b}") }),
        _ => None,
    }
}

pub async fn list_neoforge(client: &reqwest::Client, primary: mirror::Source, mc: &str) -> Result<Vec<LoaderVersion>> {
    // 1.20.1 的 NeoForge 还叫 net.neoforged:forge，版本号带 MC 前缀
    if mc == "1.20.1" {
        let xml = fetch_text(client, &format!("{NEO_MAVEN}/forge/maven-metadata.xml"), primary).await?;
        let mut list: Vec<LoaderVersion> = maven_versions(&xml)
            .into_iter()
            .filter(|v| v.starts_with("1.20.1-"))
            .map(|v| LoaderVersion { version: v.trim_start_matches("1.20.1-").to_string(), stable: !v.contains("beta"), id: v })
            .collect();
        sort_newest_first(&mut list);
        return Ok(list);
    }
    let xml = fetch_text(client, &format!("{NEO_MAVEN}/neoforge/maven-metadata.xml"), primary).await?;
    let mut list: Vec<LoaderVersion> = maven_versions(&xml)
        .into_iter()
        .filter(|v| neoforge_mc(v).as_deref() == Some(mc))
        .map(|v| LoaderVersion { version: v.clone(), stable: !v.contains("beta") && !v.contains("alpha"), id: v })
        .collect();
    sort_newest_first(&mut list);
    Ok(list)
}

fn installer_url(kind: LoaderKind, id: &str) -> String {
    match kind {
        LoaderKind::NeoForge => {
            let artifact = if id.starts_with("1.20.1-") { "forge" } else { "neoforge" };
            format!("{NEO_MAVEN}/{artifact}/{id}/{artifact}-{id}-installer.jar")
        }
        _ => format!("{FORGE_MAVEN}/{id}/forge-{id}-installer.jar"),
    }
}

// ---------- 安装器 jar 读取 ----------

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let f = std::fs::File::open(path)?;
    zip::ZipArchive::new(f).map_err(|e| Error::Other(format!("{} 不是有效的 jar：{e}", path.display())))
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name.trim_start_matches('/')).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, dest: &Path) -> Result<bool> {
    let Some(data) = read_entry(zip, name) else { return Ok(false) };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, data)?;
    Ok(true)
}

/// jar 的 Main-Class
fn main_class(jar: &Path) -> Result<String> {
    let mut zip = open_zip(jar)?;
    let manifest = read_entry(&mut zip, "META-INF/MANIFEST.MF").ok_or_else(|| Error::Other(format!("{} 没有 MANIFEST", jar.display())))?;
    String::from_utf8_lossy(&manifest)
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:").map(|s| s.trim().to_string()))
        .ok_or_else(|| Error::Other(format!("{} 没有 Main-Class", jar.display())))
}

// ---------- 安装 ----------

pub async fn install(ctx: &LoaderContext<'_>, kind: LoaderKind, mc: &str, id: &str) -> Result<String> {
    let name = match kind {
        LoaderKind::NeoForge => format!("neoforge-{id}-installer.jar"),
        _ => format!("forge-{id}-installer.jar"),
    };
    let installer = ctx.game.root.join(".installers").join(name);
    let report = ctx.report.clone();
    let r = ctx
        .downloader
        .download_all(
            vec![DownloadTask { url: installer_url(kind, id), path: installer.clone(), sha1: None, size: None }],
            Arc::new(move |p| report(Stage::Loader, p)),
        )
        .await;
    if let Some(f) = r.failed.first() {
        return Err(Error::Other(format!("安装器下载失败：{}", f.reason)));
    }

    let profile_bytes = {
        let installer = installer.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
            read_entry(&mut open_zip(&installer)?, "install_profile.json").ok_or_else(|| Error::Other("安装器里没有 install_profile.json".into()))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??
    };
    let profile: Value = serde_json::from_slice(&profile_bytes)?;

    if profile.get("versionInfo").is_some() {
        install_legacy(ctx, mc, &installer, &profile).await
    } else {
        install_modern(ctx, mc, &installer, &profile).await
    }
}

async fn download_libraries(ctx: &LoaderContext<'_>, libraries: Vec<Library>) -> Result<()> {
    let only_libs = VersionJson { libraries, ..Default::default() };
    let tasks = collect_tasks(ctx.game, &only_libs, None, ctx.platform);
    let report = ctx.report.clone();
    let r = ctx.downloader.download_all(tasks, Arc::new(move |p| report(Stage::Loader, p))).await;
    match r.failed.first() {
        Some(f) => Err(Error::Other(format!("{} 个库下载失败，例如 {}：{}", r.failed.len(), f.path, f.reason))),
        None => Ok(()),
    }
}

async fn install_legacy(ctx: &LoaderContext<'_>, mc: &str, installer: &Path, profile: &Value) -> Result<String> {
    let mut info = profile["versionInfo"].clone();
    let id = info["id"].as_str().ok_or_else(|| Error::Other("versionInfo 缺少 id".into()))?.to_string();
    // 很老的安装器没写继承关系
    if info.get("inheritsFrom").is_none() {
        info["inheritsFrom"] = Value::String(mc.to_string());
    }
    if info.get("jar").is_none() {
        info["jar"] = Value::String(mc.to_string());
    }
    let json_path = ctx.game.version_json(&id);
    std::fs::create_dir_all(json_path.parent().expect("版本目录"))?;
    std::fs::write(&json_path, serde_json::to_string_pretty(&info)?)?;

    // universal jar 在安装器根目录，放到 libraries 对应的 maven 路径
    let install = &profile["install"];
    if let (Some(file), Some(coord)) = (install["filePath"].as_str(), install["path"].as_str()) {
        let dest = join_rel(&ctx.game.libraries(), &maven_path(coord).unwrap_or_default());
        let (installer, file) = (installer.to_path_buf(), file.to_string());
        tokio::task::spawn_blocking(move || extract_entry(&mut open_zip(&installer)?, &file, &dest))
            .await
            .map_err(|e| Error::Other(e.to_string()))??;
    }

    let libs: Vec<Library> = serde_json::from_value(info["libraries"].clone())?;
    download_libraries(ctx, libs).await?;
    Ok(id)
}

#[derive(Deserialize)]
struct Processor {
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    outputs: HashMap<String, String>,
    #[serde(default)]
    sides: Option<Vec<String>>,
}

async fn install_modern(ctx: &LoaderContext<'_>, mc: &str, installer: &Path, profile: &Value) -> Result<String> {
    let game = ctx.game;
    let libs_dir = game.libraries();
    let json_name = profile["json"].as_str().unwrap_or("/version.json").to_string();
    let tmp = game.root.join(".installers").join(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&tmp)?;

    let mut profile_libs: Vec<Library> = serde_json::from_value(profile["libraries"].clone()).unwrap_or_default();
    let data: HashMap<String, String> = profile["data"]
        .as_object()
        .map(|o| o.iter().filter_map(|(k, v)| v["client"].as_str().map(|c| (k.clone(), c.to_string()))).collect())
        .unwrap_or_default();
    let main_coord = profile["path"].as_str().map(str::to_string);

    let version_text = read_entry(&mut open_zip(installer)?, &json_name)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .ok_or_else(|| Error::Other(format!("安装器里没有 {json_name}")))?;
    let version: VersionJson = serde_json::from_str(&version_text)?;
    let id = version.id.clone();

    // 从安装器里解出内嵌的 maven 库（maven/ 目录）和 data 里引用的 /xxx 文件
    let embedded: Vec<String> = version
        .libraries
        .iter()
        .chain(&profile_libs)
        .filter_map(Library::artifact_path)
        .chain(main_coord.as_deref().and_then(maven_path))
        .collect();
    let files: Vec<String> = data.values().filter(|v| v.starts_with('/')).cloned().collect();
    {
        let (installer, libs_dir, tmp) = (installer.to_path_buf(), libs_dir.clone(), tmp.clone());
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut zip = open_zip(&installer)?;
            for path in embedded {
                let dest = join_rel(&libs_dir, &path);
                if !dest.is_file() {
                    extract_entry(&mut zip, &format!("maven/{path}"), &dest)?;
                }
            }
            for f in files {
                extract_entry(&mut zip, &f, &join_rel(&tmp, &f))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    }

    let json_path = game.version_json(&id);
    std::fs::create_dir_all(json_path.parent().expect("版本目录"))?;
    std::fs::write(&json_path, &version_text)?;
    // Forge 1.17+ 的 -DignoreList 用 ${version_name}.jar 识别原版 jar，所以按新版本 id 复制一份
    let vanilla_jar = game.version_jar(mc);
    if vanilla_jar.is_file() {
        std::fs::copy(&vanilla_jar, game.version_jar(&id))?;
    }

    profile_libs.extend(version.libraries.iter().cloned());
    download_libraries(ctx, profile_libs).await?;

    // data 变量
    let lib_path = |coord: &str| join_rel(&libs_dir, &maven_path(coord).unwrap_or_default());
    let mut vars: HashMap<String, String> = HashMap::new();
    for (k, v) in &data {
        let resolved = if let Some(coord) = v.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            lib_path(coord).display().to_string()
        } else if let Some(lit) = v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            lit.to_string()
        } else if v.starts_with('/') {
            join_rel(&tmp, v).display().to_string()
        } else {
            v.clone()
        };
        vars.insert(k.clone(), resolved);
    }
    vars.insert("SIDE".into(), "client".into());
    vars.insert("MINECRAFT_JAR".into(), vanilla_jar.display().to_string());
    vars.insert("MINECRAFT_VERSION".into(), mc.to_string());
    vars.insert("ROOT".into(), game.root.display().to_string());
    vars.insert("INSTALLER".into(), installer.display().to_string());
    vars.insert("LIBRARY_DIR".into(), libs_dir.display().to_string());

    let resolve = |s: &str| -> Result<String> {
        if let Some(key) = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
            return vars.get(key).cloned().ok_or_else(|| Error::Other(format!("安装器缺少变量 {key}")));
        }
        if let Some(coord) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            return Ok(lib_path(coord).display().to_string());
        }
        Ok(s.to_string())
    };

    let processors: Vec<Processor> = serde_json::from_value(profile["processors"].clone()).unwrap_or_default();
    let processors: Vec<Processor> = processors
        .into_iter()
        .filter(|p| p.sides.as_ref().is_none_or(|s| s.iter().any(|x| x == "client")))
        .collect();
    let java = match (&ctx.java, processors.is_empty()) {
        (_, true) => None,
        (Some(j), false) => Some(j.clone()),
        (None, false) => return Err(Error::Other("运行 Forge 安装器需要 Java".into())),
    };

    let total = processors.len();
    let sep = crate::engine::launch::classpath_separator(ctx.platform);
    for (i, p) in processors.iter().enumerate() {
        (ctx.report)(Stage::Processor, Progress { total_files: total, finished_files: i, ..Default::default() });

        let mut outputs = Vec::new();
        for (file, sha) in &p.outputs {
            outputs.push((PathBuf::from(resolve(file)?), resolve(sha)?));
        }
        if !outputs.is_empty() && outputs_ok(&outputs).await {
            continue;
        }

        let jar = lib_path(&p.jar);
        let main = main_class(&jar)?;
        let cp: Vec<String> = std::iter::once(jar.display().to_string())
            .chain(p.classpath.iter().map(|c| lib_path(c).display().to_string()))
            .collect();
        let args: Vec<String> = p.args.iter().map(|a| resolve(a)).collect::<Result<_>>()?;

        let mut cmd = tokio::process::Command::new(java.as_ref().expect("上面检查过"));
        cmd.arg("-cp").arg(cp.join(sep)).arg(&main).args(&args).current_dir(&tmp).stdin(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let out = cmd.output().await.map_err(|e| Error::Other(format!("无法运行 Java：{e}")))?;
        if !out.status.success() {
            let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
            let tail: Vec<&str> = log.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
            return Err(Error::Other(format!("安装器步骤 {}/{total}（{main}）失败：\n{}", i + 1, tail.join("\n"))));
        }
        if !outputs.is_empty() && !outputs_ok(&outputs).await {
            return Err(Error::Other(format!("安装器步骤 {}/{total}（{main}）的输出校验失败", i + 1)));
        }
    }
    (ctx.report)(Stage::Processor, Progress { total_files: total, finished_files: total, ..Default::default() });
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(id)
}

async fn outputs_ok(outputs: &[(PathBuf, String)]) -> bool {
    for (path, sha) in outputs {
        let sha = sha.trim_matches('\'');
        match sha1_file(path).await {
            Ok(h) if h.eq_ignore_ascii_case(sha) => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_neoforge_versions() {
        assert_eq!(neoforge_mc("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_mc("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_mc("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_mc("20.2.3-beta").as_deref(), Some("1.20.2"));
        assert_eq!(neoforge_mc("26.3.0.97").as_deref(), Some("26.3"));
        assert_eq!(neoforge_mc("26.1.2.5").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_mc("x").as_deref(), None);
    }

    #[test]
    fn installer_urls() {
        assert_eq!(
            installer_url(LoaderKind::Forge, "1.20.1-47.2.0"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar"
        );
        assert!(installer_url(LoaderKind::NeoForge, "21.1.77").ends_with("/neoforge/21.1.77/neoforge-21.1.77-installer.jar"));
        assert!(installer_url(LoaderKind::NeoForge, "1.20.1-47.1.106").ends_with("/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar"));
    }
}
