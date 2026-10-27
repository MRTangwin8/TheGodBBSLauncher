//! Java 管理：扫描本机 JRE、识别版本和架构、按需下载 Java。
//!
//! 下载顺序：Mojang 官方 runtime（要的那个没有时，16 及以上换更高的；Windows ARM 上的 Java 8 用 x64 版，系统自带转译）
//! → 都没有时（Apple 芯片 / Linux ARM 上的 Java 8、Linux ARM 上的所有版本）从 Azul Zulu 下载，见 zulu.rs。

mod zulu;

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

/// "1.8.0_392" → 8，"17.0.8" → 17，"21" → 21，"8u202" → 8
pub fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.trim().trim_matches('"').split(['.', '_', '-', '+', 'u']);
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
    if let Some(text) = run_java(java, &["-XshowSettings:properties", "-version"]).await {
        if let Some(version) = prop(&text, "java.version") {
            return Some(JavaInstall {
                path: java.to_path_buf(),
                major: parse_major(&version)?,
                version,
                arch: prop(&text, "os.arch").map(|a| normalize_arch(&a)).unwrap_or_default(),
                vendor: prop(&text, "java.vendor"),
            });
        }
    }
    // Java 6 不认识 -XshowSettings（Java 7 才有），退回解析 `java -version`
    let text = run_java(java, &["-version"]).await?;
    let (version, bits64) = parse_version_output(&text)?;
    Some(JavaInstall { path: java.to_path_buf(), major: parse_major(&version)?, version, arch: guess_arch(bits64), vendor: None })
}

/// 运行 java，返回 stderr（版本信息都打在 stderr 上）
async fn run_java(java: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = tokio::process::Command::new(java);
    cmd.args(args).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = tokio::time::timeout(std::time::Duration::from_secs(10), cmd.output()).await.ok()?.ok()?;
    Some(String::from_utf8_lossy(&out.stderr).into_owned())
}

/// `-XshowSettings:properties` 输出里的 `    key = value`
fn prop(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix(key)?.trim().strip_prefix('=').map(|v| v.trim().to_string()))
}

/// `java version "1.6.0_45"` / `openjdk version "17.0.8" 2023-07-18` → (版本号, 是不是 64 位)
fn parse_version_output(text: &str) -> Option<(String, bool)> {
    let line = text.lines().find(|l| l.contains(" version \""))?;
    let version = line.split('"').nth(1)?.trim().to_string();
    (!version.is_empty()).then_some((version, text.contains("64-Bit")))
}

/// 老 Java 不报告架构，只知道是不是 64 位：64 位就当作和系统一致
fn guess_arch(bits64: bool) -> String {
    match (bits64, Platform::current().arch) {
        (false, _) => "x86",
        (true, "arm64") => "arm64",
        (true, _) => "x86_64",
    }
    .to_string()
}

/// 常见安装位置下的 JDK/JRE 根目录
fn candidate_homes(extra_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = extra_roots.to_vec();
    #[cfg(windows)]
    {
        // 所有盘上的 Program Files，以及直接装在盘根目录的 X:\Java
        for drive in windows_drives() {
            for base in ["Program Files", "Program Files (x86)"] {
                for vendor in ["Java", "Eclipse Adoptium", "Eclipse Foundation", "AdoptOpenJDK", "Zulu", "Microsoft", "BellSoft", "Amazon Corretto", "Semeru", "IBM", "Liberica", "GraalVM"] {
                    roots.push(drive.join(base).join(vendor));
                }
            }
            roots.push(drive.join("Java"));
        }
        // 安装程序写进注册表的位置（装到了别的文件夹也能找到）
        roots.extend(windows_registry_homes());
        // scoop 装的 Java：apps/<名字>/current
        let scoop = dirs::home_dir().map(|h| h.join("scoop")).into_iter().chain([PathBuf::from("C:\\ProgramData\\scoop")]);
        for dir in scoop {
            let Ok(apps) = std::fs::read_dir(dir.join("apps")) else { continue };
            for app in apps.flatten() {
                let name = app.file_name().to_string_lossy().to_lowercase();
                if ["jdk", "jre", "java", "temurin", "zulu", "corretto", "liberica", "graalvm", "semeru"].iter().any(|k| name.contains(k)) {
                    roots.push(app.path().join("current"));
                }
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

/// 存在的盘符根目录（C:\ D:\ …）
#[cfg(windows)]
fn windows_drives() -> Vec<PathBuf> {
    (b'C'..=b'Z').map(|c| PathBuf::from(format!("{}:\\", c as char))).filter(|p| p.is_dir()).collect()
}

/// 读注册表里 Java 安装程序登记的目录
#[cfg(windows)]
fn windows_registry_homes() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;
    let queries: [(&str, &str); 7] = [
        ("HKLM\\SOFTWARE\\JavaSoft", "JavaHome"),
        ("HKLM\\SOFTWARE\\WOW6432Node\\JavaSoft", "JavaHome"),
        ("HKLM\\SOFTWARE\\Eclipse Adoptium", "Path"),
        ("HKLM\\SOFTWARE\\Eclipse Foundation", "Path"),
        ("HKLM\\SOFTWARE\\AdoptOpenJDK", "Path"),
        ("HKLM\\SOFTWARE\\Microsoft\\JDK", "Path"),
        ("HKLM\\SOFTWARE\\Azul Systems\\Zulu", "InstallationPath"),
    ];
    let mut out = Vec::new();
    for (key, value) in queries {
        let Ok(o) = std::process::Command::new("reg").args(["query", key, "/s", "/v", value]).creation_flags(0x0800_0000).output() else { continue };
        out.extend(parse_reg_output(&String::from_utf8_lossy(&o.stdout), value));
    }
    out
}

/// `reg query … /v JavaHome` 的输出：`    JavaHome    REG_SZ    C:\Program Files\Java\jre1.8.0_421`
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_reg_output(text: &str, value: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix(value)?;
            let (_, path) = rest.split_once("REG_SZ").or_else(|| rest.split_once("REG_EXPAND_SZ"))?;
            let path = path.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        })
        .collect()
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
pub struct RuntimeEntry {
    manifest: DownloadInfo,
    #[serde(default)]
    version: Option<RuntimeVersion>,
}

#[derive(Deserialize)]
struct RuntimeVersion {
    name: String,
}

/// 平台 → 组件 → 版本（Mojang 的 all.json）
pub type RuntimeIndex = HashMap<String, HashMap<String, Vec<RuntimeEntry>>>;

/// 选中的 Mojang runtime
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeChoice {
    pub platform_key: String,
    pub component: String,
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

/// 这个平台上可以试的 Mojang 平台：Windows ARM 上没有的，用 x64 版（系统自带 x64 转译）
fn runtime_platforms(p: &Platform) -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = runtime_platform(p).into_iter().collect();
    if (p.os, p.arch) == ("windows", "arm64") {
        keys.push("windows-x64");
    }
    keys
}

/// 在 Mojang 的 runtime 里给「要 `major`、最好是 `component`」挑一个：
/// 有原组件就用；16 及以上可以换成主版本更高的组件（比如 Apple 芯片上没有 16 就用 17）；
/// 8 之类的老版本只认原组件。都没有返回 None，交给第三方 JDK。
pub fn plan_runtime(index: &RuntimeIndex, platform: &Platform, component: &str, major: u32) -> Option<RuntimeChoice> {
    for key in runtime_platforms(platform) {
        let Some(comps) = index.get(key) else { continue };
        let available = |c: &str| comps.get(c).is_some_and(|v| !v.is_empty());
        if available(component) {
            return Some(RuntimeChoice { platform_key: key.into(), component: component.into() });
        }
        if major >= 16 {
            let better = comps
                .iter()
                .filter(|(name, v)| !name.contains("snapshot") && !v.is_empty())
                .filter_map(|(name, v)| Some((parse_major(&v[0].version.as_ref()?.name)?, name)))
                .filter(|(m, _)| *m >= major)
                .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
            if let Some((_, name)) = better {
                return Some(RuntimeChoice { platform_key: key.into(), component: name.clone() });
            }
        }
    }
    None
}

/// runtime 目录里 java 可执行文件的位置（macOS 是 bundle 结构）
pub fn runtime_java(dir: &Path, platform: &Platform) -> PathBuf {
    if platform.os == "osx" {
        dir.join("jre.bundle/Contents/Home/bin/java")
    } else {
        dir.join("bin").join(JAVA_EXE)
    }
}

/// 下载能跑「要 `major`、版本文件里写的是 `component`」的 Java，返回 java 路径。
/// 先找 Mojang 官方 runtime（见 plan_runtime），没有再从 Azul Zulu 下载。
pub async fn install_runtime(
    downloader: &Downloader,
    client: &reqwest::Client,
    runtime_root: &Path,
    major: u32,
    component: &str,
    platform: &Platform,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    if !runtime_platforms(platform).is_empty() {
        let index: RuntimeIndex = fetch_json(client, RUNTIME_INDEX, downloader.primary).await?;
        if let Some(choice) = plan_runtime(&index, platform, component, major) {
            return install_mojang(downloader, client, runtime_root, &index, &choice, platform, on_progress).await;
        }
    }
    zulu::install(downloader, client, runtime_root, major, platform, on_progress).await
}

async fn install_mojang(
    downloader: &Downloader,
    client: &reqwest::Client,
    runtime_root: &Path,
    index: &RuntimeIndex,
    choice: &RuntimeChoice,
    platform: &Platform,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<PathBuf> {
    let RuntimeChoice { platform_key: key, component } = choice;
    let entry = index
        .get(key)
        .and_then(|m| m.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| Error::Other(format!("Mojang 没有为 {key} 提供 {component}，请手动安装 Java")))?;
    let manifest: RuntimeManifest = fetch_json(client, &entry.manifest.url, downloader.primary).await?;

    // 借用 x64 版时放到单独的文件夹，免得和以后可能出现的原生版混在一起
    let dir = if Some(key.as_str()) == runtime_platform(platform) { runtime_root.join(component) } else { runtime_root.join(format!("{component}-{key}")) };
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
        assert_eq!(parse_major("8u202"), Some(8));
        assert_eq!(parse_major("1.6.0_45"), Some(6));
    }

    #[test]
    fn parses_old_java_version_output() {
        let java6 = "java version \"1.6.0_45\"\nJava(TM) SE Runtime Environment (build 1.6.0_45-b06)\nJava HotSpot(TM) 64-Bit Server VM (build 20.45-b01, mixed mode)\n";
        assert_eq!(parse_version_output(java6), Some(("1.6.0_45".into(), true)));
        let open = "openjdk version \"17.0.8\" 2023-07-18\nOpenJDK Runtime Environment Temurin-17.0.8+7 (build 17.0.8+7)\nOpenJDK Client VM (build 17.0.8+7, mixed mode)\n";
        assert_eq!(parse_version_output(open), Some(("17.0.8".into(), false)));
        assert_eq!(parse_version_output("Unrecognized option: -XshowSettings"), None);
    }

    #[test]
    fn parses_registry_output() {
        let text = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\JavaSoft\\Java Runtime Environment\\1.8\r\n    JavaHome    REG_SZ    D:\\Java\\jre1.8.0_421\r\n\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\JavaSoft\\JDK\\17\r\n    JavaHome    REG_SZ    C:\\Program Files\\Java\\jdk-17\r\n\r\n搜索结束: 找到 2 匹配。\r\n";
        assert_eq!(parse_reg_output(text, "JavaHome"), vec![PathBuf::from("D:\\Java\\jre1.8.0_421"), PathBuf::from("C:\\Program Files\\Java\\jdk-17")]);
        assert!(parse_reg_output(text, "Path").is_empty());
    }

    fn index(json: &str) -> RuntimeIndex {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn plans_mojang_runtime_with_fallbacks() {
        let e = |v: &str| format!(r#"[{{"manifest":{{"url":"u","sha1":"s","size":1}},"version":{{"name":"{v}"}}}}]"#);
        let idx = index(&format!(
            r#"{{"mac-os-arm64":{{"java-runtime-alpha":[],"java-runtime-gamma":{g},"java-runtime-gamma-snapshot":{g},"java-runtime-delta":{d},"jre-legacy":[]}},
                "windows-arm64":{{"java-runtime-gamma":{g},"jre-legacy":[]}},
                "windows-x64":{{"java-runtime-alpha":{a},"jre-legacy":{l}}}}}"#,
            g = e("17.0.15"),
            d = e("21.0.7"),
            a = e("16.0.1.9.1"),
            l = e("8u51-cacert462b08"),
        ));
        let p = |os: &'static str, arch: &'static str| Platform { os, arch, os_version: String::new() };
        let c = |k: &str, comp: &str| Some(RuntimeChoice { platform_key: k.into(), component: comp.into() });
        // Apple 芯片：要 16 没有，换 17；要 21 直接有；要 8 没有，交给第三方
        assert_eq!(plan_runtime(&idx, &p("osx", "arm64"), "java-runtime-alpha", 16), c("mac-os-arm64", "java-runtime-gamma"));
        assert_eq!(plan_runtime(&idx, &p("osx", "arm64"), "java-runtime-delta", 21), c("mac-os-arm64", "java-runtime-delta"));
        assert_eq!(plan_runtime(&idx, &p("osx", "arm64"), "jre-legacy", 8), None);
        // Windows ARM：16 换原生 17；8 借 x64 版
        assert_eq!(plan_runtime(&idx, &p("windows", "arm64"), "java-runtime-alpha", 16), c("windows-arm64", "java-runtime-gamma"));
        assert_eq!(plan_runtime(&idx, &p("windows", "arm64"), "jre-legacy", 8), c("windows-x64", "jre-legacy"));
        // 要 25 谁都没有
        assert_eq!(plan_runtime(&idx, &p("osx", "arm64"), "java-runtime-epsilon", 25), None);
        // Linux ARM 没有 Mojang runtime
        assert!(runtime_platforms(&p("linux", "arm64")).is_empty());
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
