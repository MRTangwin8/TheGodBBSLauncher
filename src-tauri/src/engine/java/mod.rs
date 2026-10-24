//! Java 管理：扫描本机 JRE、识别版本和架构、按需下载 Mojang 官方 runtime。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::download::{DownloadTask, Downloader, Progress};
use crate::engine::install::{fetch_json, join_rel};
use crate::engine::meta::version::DownloadInfo;
use crate::engine::meta::Platform;
use crate::engine::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    /// java 可执行文件
    pub path: PathBuf,
    pub major: u32,
    pub version: String,
    /// x86_64 / x86 / arm64 / 未知时为空
    pub arch: String,
    pub vendor: Option<String>,
}

#[cfg(windows)]
pub const JAVA_EXE: &str = "java.exe";
#[cfg(not(windows))]
pub const JAVA_EXE: &str = "java";

/// "1.8.0_392" → 8，"17.0.8" → 17，"21" → 21
pub fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.trim().trim_matches('"').split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn normalize_arch(arch: &str) -> String {
    match arch.trim().trim_matches('"') {
        "amd64" | "x86_64" | "x64" => "x86_64",
        "x86" | "i386" | "i586" | "i686" => "x86",
        "aarch64" | "arm64" => "arm64",
        other => other,
    }
    .to_string()
}

/// JAVA_HOME/release 文件的 KEY="VALUE" 格式
fn parse_release(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').to_string()))
        .collect()
}

/// 识别一个 java 可执行文件。优先读 release 文件（不用起进程），读不到再跑 `java -XshowSettings`。
pub async fn inspect(java: &Path) -> Option<JavaInstall> {
    if !java.is_file() {
        return None;
    }
    if let Some(home) = java.parent().and_then(Path::parent) {
        if let Ok(text) = tokio::fs::read_to_string(home.join("release")).await {
            let r = parse_release(&text);
            if let Some(version) = r.get("JAVA_VERSION") {
                return Some(JavaInstall {
                    path: java.to_path_buf(),
                    major: parse_major(version)?,
                    version: version.clone(),
                    arch: r.get("OS_ARCH").map(|a| normalize_arch(a)).unwrap_or_default(),
                    vendor: r.get("IMPLEMENTOR").cloned(),
                });
            }
        }
    }
    let mut cmd = tokio::process::Command::new(java);
    cmd.args(["-XshowSettings:properties", "-version"]).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output()).await.ok()?.ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let prop = |key: &str| {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(key)?.trim().strip_prefix('=').map(|v| v.trim().to_string()))
    };
    let version = prop("java.version")?;
    Some(JavaInstall {
        path: java.to_path_buf(),
        major: parse_major(&version)?,
        version,
        arch: prop("os.arch").map(|a| normalize_arch(&a)).unwrap_or_default(),
        vendor: prop("java.vendor"),
    })
}

/// 常见安装位置下的 JDK/JRE 根目录
fn candidate_homes(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = extra_roots.to_vec();
    #[cfg(windows)]
    {
        for base in ["C:\\Program Files", "C:\\Program Files (x86)"] {
            for vendor in ["Java", "Eclipse Adoptium", "Eclipse Foundation", "Zulu", "Microsoft", "BellSoft", "Amazon Corretto", "Semeru", "Liberica"] {
                roots.push(Path::new(base).join(vendor));
            }
        }
        if let Some(appdata) = dirs::data_dir() {
            // 官方启动器自带的 runtime
            roots.push(appdata.join(".minecraft").join("runtime"));
        }
        if let Some(local) = dirs::data_local_dir() {
            roots.push(local.join("Packages").join("Microsoft.4297127D64EC6_8wekyb3d8bbwe").join("LocalCache").join("Local").join("runtime"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Library/Java/JavaVirtualMachines"));
            roots.push(home.join("Library/Application Support/minecraft/runtime"));
        }
    }
    #[cfg(target_os = "linux")]
    {
        roots.push(PathBuf::from("/usr/lib/jvm"));
        roots.push(PathBuf::from("/usr/java"));
        roots.push(PathBuf::from("/opt"));
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".minecraft/runtime"));
            roots.push(home.join(".sdkman/candidates/java"));
        }
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".jdks"));
    }
    roots
}

/// 在目录下最多往下找 4 层 bin/java（runtime 目录结构比较深）
fn find_java_under(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let exe = root.join("bin").join(JAVA_EXE);
    if exe.is_file() {
        out.push(exe);
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for e in entries.flatten() {
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            find_java_under(&e.path(), depth - 1, out);
        }
    }
}

/// 扫描本机所有 Java，`extra_roots` 放启动器自己下载的 runtime 目录
pub async fn scan(extra_roots: Vec<PathBuf>) -> Vec<JavaInstall> {
    let exes = tokio::task::spawn_blocking(move || {
        let mut exes = Vec::new();
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            exes.push(PathBuf::from(home).join("bin").join(JAVA_EXE));
        }
        if let Some(path) = std::env::var_os("PATH") {
            exes.extend(std::env::split_paths(&path).map(|p| p.join(JAVA_EXE)));
        }
        for root in candidate_homes(&extra_roots) {
            find_java_under(&root, 4, &mut exes);
        }
        let mut seen = HashSet::new();
        exes.into_iter()
            .filter(|p| p.is_file())
            .filter(|p| seen.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();

    let mut found = Vec::new();
    for exe in exes {
        if let Some(j) = inspect(&exe).await {
            if !found.iter().any(|f: &JavaInstall| f.version == j.version && f.arch == j.arch && f.vendor == j.vendor) {
                found.push(j);
            }
        }
    }
    found.sort_by_key(|j| std::cmp::Reverse(j.major));
    found
}

/// 为需要 `major` 的版本选 Java：优先同主版本；16+ 允许用更高版本；Java 8 只认 8（旧版和高版本 Java 兼容性差）。
/// 同条件下优先和系统架构一致的。
pub fn pick<'a>(installs: &'a [JavaInstall], major: u32, platform: &Platform) -> Option<&'a JavaInstall> {
    let arch_ok = |j: &JavaInstall| j.arch.is_empty() || j.arch == platform.arch;
    let score = |j: &JavaInstall| -> Option<(u32, u32)> {
        let distance = if j.major == major {
            0
        } else if major >= 16 && j.major > major {
            j.major - major
        } else {
            return None;
        };
        Some((distance, if arch_ok(j) { 0 } else { 1 }))
    };
    installs
        .iter()
        .filter_map(|j| score(j).map(|s| (s, j)))
        .min_by_key(|(s, _)| *s)
        .map(|(_, j)| j)
}

// ---------- Mojang 官方 Java runtime ----------

const RUNTIME_INDEX: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

#[derive(Deserialize)]
struct RuntimeEntry {
    manifest: DownloadInfo,
}

#[derive(Deserialize)]
struct RuntimeManifest {
    files: HashMap<String, RuntimeFile>,
}

#[derive(Deserialize)]
struct RuntimeFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    #[cfg_attr(windows, allow(dead_code))]
    executable: bool,
    #[serde(default)]
    downloads: Option<RuntimeDownloads>,
    #[serde(default)]
    #[cfg_attr(windows, allow(dead_code))]
    target: Option<String>,
}

#[derive(Deserialize)]
struct RuntimeDownloads {
    raw: DownloadInfo,
}

fn runtime_platform(p: &Platform) -> Option<&'static str> {
    Some(match (p.os, p.arch) {
        ("windows", "x86_64") => "windows-x64",
        ("windows", "x86") => "windows-x86",
        ("windows", "arm64") => "windows-arm64",
        ("osx", "arm64") => "mac-os-arm64",
        ("osx", _) => "mac-os",
        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",
        _ => return None,
    })
}

/// runtime 目录里 java 可执行文件的位置（macOS 是 bundle 结构）
pub fn runtime_java(dir: &Path, platform: &Platform) -> PathBuf {
    if platform.os == "osx" {
        dir.join("jre.bundle/Contents/Home/bin/java")
    } else {
        dir.join("bin").join(JAVA_EXE)
    }
}

/// 下载 Mojang 的 runtime（如 java-runtime-gamma / jre-legacy）到 `runtime_root/<component>`，返回 java 路径
pub async fn install_runtime(
    downloader: &Downloader,
    client: &reqwest::Client,
    runtime_root: &Path,
    component: &str,
    platform: &Platform,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    let key = runtime_platform(platform).ok_or_else(|| Error::Other("Mojang 没有为这个平台提供 Java".into()))?;
    let index: HashMap<String, HashMap<String, Vec<RuntimeEntry>>> = fetch_json(client, RUNTIME_INDEX, downloader.primary).await?;
    let entry = index
        .get(key)
        .and_then(|m| m.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| Error::Other(format!("Mojang 没有为 {key} 提供 {component}，请手动安装 Java")))?;
    let manifest: RuntimeManifest = fetch_json(client, &entry.manifest.url, downloader.primary).await?;

    let dir = runtime_root.join(component);
    let mut tasks = Vec::new();
    for (rel, file) in &manifest.files {
        let path = join_rel(&dir, rel);
        match (file.kind.as_str(), &file.downloads) {
            ("directory", _) => tokio::fs::create_dir_all(&path).await?,
            ("file", Some(d)) => tasks.push(DownloadTask {
                url: d.raw.url.clone(),
                path,
                sha1: d.raw.sha1.clone(),
                size: d.raw.size,
            }),
            _ => {}
        }
    }
    let report = downloader.download_all(tasks, on_progress).await;
    if let Some(f) = report.failed.first() {
        return Err(Error::Other(format!("Java 下载失败：{}：{}", f.path, f.reason)));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for (rel, file) in &manifest.files {
            let path = join_rel(&dir, rel);
            if file.kind == "file" && file.executable {
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            }
            if let ("link", Some(target)) = (file.kind.as_str(), &file.target) {
                let _ = std::fs::remove_file(&path);
                let _ = std::os::unix::fs::symlink(target, &path);
            }
        }
    }
    let java = runtime_java(&dir, platform);
    if !java.is_file() {
        return Err(Error::Other(format!("下载完成但找不到 {}", java.display())));
    }
    Ok(java)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(major: u32, arch: &str) -> JavaInstall {
        JavaInstall { path: PathBuf::from(format!("/j{major}{arch}")), major, version: major.to_string(), arch: arch.into(), vendor: None }
    }

    #[test]
    fn parses_versions() {
        assert_eq!(parse_major("1.8.0_392"), Some(8));
        assert_eq!(parse_major("17.0.8"), Some(17));
        assert_eq!(parse_major("\"21\""), Some(21));
        assert_eq!(parse_major("22-ea"), Some(22));
        assert_eq!(parse_major("abc"), None);
    }

    #[test]
    fn parses_release_file() {
        let r = parse_release("IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"17.0.8\"\nOS_ARCH=\"amd64\"\n");
        assert_eq!(r["JAVA_VERSION"], "17.0.8");
        assert_eq!(normalize_arch(&r["OS_ARCH"]), "x86_64");
    }

    #[test]
    fn picks_matching_java() {
        let p = Platform { os: "windows", arch: "x86_64", os_version: String::new() };
        let all = [j(8, "x86"), j(8, "x86_64"), j(21, "x86_64"), j(17, "x86_64")];
        assert_eq!(pick(&all, 8, &p).unwrap().arch, "x86_64");
        assert_eq!(pick(&all, 17, &p).unwrap().major, 17);
        assert_eq!(pick(&all, 16, &p).unwrap().major, 17);
        assert!(pick(&[j(17, "x86_64")], 8, &p).is_none());
        assert!(pick(&[j(17, "x86_64")], 21, &p).is_none());
    }
}
