//! 启动：classpath、natives 解压、旧版资源映射、JVM / 游戏参数拼接。
//! 进程管理放在 commands 层（需要往前端推日志）。

pub mod log4j;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::engine::auth::LaunchAuth;
use crate::engine::install::{join_rel, GameDir};
use crate::engine::meta::version::{Argument, AssetIndex};
use crate::engine::meta::{Platform, VersionJson};
use crate::engine::{Error, Result};

pub const LAUNCHER_NAME: &str = "GodBBSLauncher";

/// authlib-injector 参数
#[derive(Debug, Clone)]
pub struct Agent {
    pub jar: PathBuf,
    pub api_root: String,
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub java: PathBuf,
    /// 游戏工作目录（开了版本隔离就是 versions/<id>）
    pub game_dir: PathBuf,
    pub auth: LaunchAuth,
    pub memory_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
    pub resolution: Option<(u32, u32)>,
    pub agent: Option<Agent>,
}

#[derive(Debug, Clone)]
pub struct Prepared {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// Windows 上 Java 21+ 的网络库会在临时目录建 Unix 域套接字。临时目录路径含中文（中文用户名）、
/// 8.3 短名（`~1`）或太长时会报 "Unable to establish loopback connection"，第一个联网的模组就崩了。
/// 这种情况下给它换一个干净的目录。
pub fn unix_socket_dir(temp: &Path, fallback: &Path) -> Option<PathBuf> {
    if is_clean_path(temp) {
        return None;
    }
    [fallback.to_path_buf(), PathBuf::from(r"C:\Users\Public\GodBBSLauncher")]
        .into_iter()
        .find(|p| is_clean_path(p) && std::fs::create_dir_all(p).is_ok())
}

/// 纯英文数字、不含 8.3 短名的 `~`、不太长
fn is_clean_path(p: &Path) -> bool {
    let s = p.display().to_string();
    s.len() <= 60 && s.chars().all(|c| c.is_ascii_alphanumeric() || " _-.:\\/".contains(c))
}

pub fn classpath_separator(platform: &Platform) -> &'static str {
    if platform.os == "windows" {
        ";"
    } else {
        ":"
    }
}

/// 库文件 + 客户端 jar
pub fn classpath(game: &GameDir, version: &VersionJson, platform: &Platform) -> Vec<PathBuf> {
    let libs = game.libraries();
    let mut seen = HashSet::new();
    let mut out: Vec<PathBuf> = version
        .libraries
        .iter()
        .filter(|l| l.is_allowed(platform))
        // 用 artifact_path 而不是 artifact：Forge 内嵌/本地生成的库没有下载地址，但同样要进 classpath
        .filter_map(|l| l.artifact_path())
        .map(|p| join_rel(&libs, &p))
        .filter(|p| seen.insert(p.clone()))
        .collect();
    // Forge/NeoForge 安装时会把原版 jar 复制成 versions/<id>/<id>.jar，有就优先用它
    let own = game.version_jar(&version.id);
    out.push(if own.is_file() { own } else { game.version_jar(version.jar_id()) });
    out
}

/// 需要解压的 natives jar：旧格式 classifiers + 1.19 起的 `:natives-xxx` 独立库
fn native_jars(game: &GameDir, version: &VersionJson, platform: &Platform) -> Vec<(PathBuf, Vec<String>)> {
    let libs = game.libraries();
    version
        .libraries
        .iter()
        .filter(|l| l.is_allowed(platform))
        .filter_map(|l| {
            let exclude = l.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
            if let Some(f) = l.native_artifact(platform) {
                return Some((join_rel(&libs, &f.path), exclude));
            }
            let is_new_native = l.name.split(':').nth(3).is_some_and(|c| c.starts_with("natives-"));
            if is_new_native {
                return l.artifact().map(|f| (join_rel(&libs, &f.path), exclude));
            }
            None
        })
        .collect()
}

/// 把 natives jar 里的动态库平铺解压到 `dir`。
/// 大小没变的文件不重写：每次启动都重新生成 DLL 会触发杀毒软件扫描，扫描期间 LWJGL 加载会失败。
fn extract_natives(jars: &[(PathBuf, Vec<String>)], dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for (jar, exclude) in jars {
        let file = std::fs::File::open(jar).map_err(|e| Error::Other(format!("缺少 natives 文件 {}：{e}", jar.display())))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} 不是有效的 jar：{e}", jar.display())))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| Error::Other(e.to_string()))?;
            let name = entry.name().to_string();
            if entry.is_dir() || exclude.iter().any(|x| name.starts_with(x.as_str())) {
                continue;
            }
            let is_lib = [".dll", ".so", ".dylib", ".jnilib"].iter().any(|ext| name.ends_with(ext));
            let Some(file_name) = Path::new(&name).file_name() else { continue };
            if !is_lib {
                continue;
            }
            let dest = dir.join(file_name);
            if dest.metadata().is_ok_and(|m| m.len() == entry.size()) {
                continue;
            }
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

/// 1.7.2 以前的版本需要把资源按原文件名复制出来，返回游戏用的资源目录
fn map_legacy_assets(game: &GameDir, index: &AssetIndex, game_dir: &Path) -> Result<Option<PathBuf>> {
    let target = if index.map_to_resources {
        game_dir.join("resources")
    } else if index.is_virtual {
        game.assets().join("virtual").join("legacy")
    } else {
        return Ok(None);
    };
    let objects = game.assets().join("objects");
    for (name, obj) in &index.objects {
        if obj.hash.len() < 2 {
            continue;
        }
        let dst = join_rel(&target, name);
        if dst.metadata().map(|m| m.len() == obj.size).unwrap_or(false) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(objects.join(&obj.hash[..2]).join(&obj.hash), &dst)?;
    }
    Ok(Some(target))
}

/// 准备好完整命令行。会解压 natives、映射旧版资源，所以要在文件补全之后调用。
pub async fn prepare(game: &GameDir, version: &VersionJson, opts: &LaunchOptions, platform: &Platform) -> Result<Prepared> {
    std::fs::create_dir_all(&opts.game_dir)?;
    let natives_dir = game.versions().join(&version.id).join(format!("natives-{}-{}", platform.os, platform.arch));
    let jars = native_jars(game, version, platform);
    let dir = natives_dir.clone();
    tokio::task::spawn_blocking(move || extract_natives(&jars, &dir))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;

    let assets_index_name = version
        .assets
        .clone()
        .or_else(|| version.asset_index.as_ref().map(|a| a.id.clone()))
        .unwrap_or_else(|| "legacy".into());
    let mut game_assets = game.assets();
    if let Ok(text) = std::fs::read_to_string(game.asset_index(&assets_index_name)) {
        let index: AssetIndex = serde_json::from_str(&text)?;
        let (g, gd) = (game.clone(), opts.game_dir.clone());
        if let Some(dir) = tokio::task::spawn_blocking(move || map_legacy_assets(&g, &index, &gd))
            .await
            .map_err(|e| Error::Other(e.to_string()))??
        {
            game_assets = dir;
        }
    }

    let sep = classpath_separator(platform);
    let cp = classpath(game, version, platform);
    if let Some(missing) = cp.iter().find(|p| !p.is_file()) {
        return Err(Error::Other(format!("缺少文件 {}，请先补全文件", missing.display())));
    }
    let cp: Vec<String> = cp.iter().map(|p| p.display().to_string()).collect();

    let mut vars: HashMap<&str, String> = HashMap::new();
    let p = |path: &Path| path.display().to_string();
    vars.insert("auth_player_name", opts.auth.name.clone());
    vars.insert("version_name", version.id.clone());
    vars.insert("game_directory", p(&opts.game_dir));
    vars.insert("assets_root", p(&game.assets()));
    vars.insert("game_assets", p(&game_assets));
    vars.insert("assets_index_name", assets_index_name);
    vars.insert("auth_uuid", opts.auth.uuid.clone());
    vars.insert("auth_access_token", opts.auth.access_token.clone());
    vars.insert("auth_session", format!("token:{}:{}", opts.auth.access_token, opts.auth.uuid));
    vars.insert("auth_xuid", opts.auth.xuid.clone());
    vars.insert("clientid", String::new());
    vars.insert("user_type", opts.auth.user_type.to_string());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", "神权论坛".into());
    vars.insert("natives_directory", p(&natives_dir));
    vars.insert("launcher_name", LAUNCHER_NAME.into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", cp.join(sep));
    vars.insert("classpath_separator", sep.into());
    vars.insert("library_directory", p(&game.libraries()));
    if let Some((w, h)) = opts.resolution {
        vars.insert("resolution_width", w.to_string());
        vars.insert("resolution_height", h.to_string());
    }
    if let Some(client) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = game.assets().join("log_configs").join(&client.file.id);
        if path.is_file() {
            vars.insert("path", p(&path));
        }
    }

    let mut jvm_prefix = Vec::new();
    if let Some(agent) = &opts.agent {
        use base64::Engine;
        jvm_prefix.push(format!("-javaagent:{}={}", agent.jar.display(), agent.api_root));
        jvm_prefix.push(format!(
            "-Dauthlibinjector.yggdrasil.prefetched={}",
            base64::engine::general_purpose::STANDARD.encode(&agent.metadata)
        ));
    }
    // 用户自己写了就听用户的
    if platform.os == "windows" && !opts.extra_jvm_args.iter().any(|a| a.contains("jdk.net.unixdomain.tmpdir")) {
        if let Some(dir) = unix_socket_dir(&std::env::temp_dir(), &game.root.join(".tmp")) {
            jvm_prefix.push(format!("-Djdk.net.unixdomain.tmpdir={}", dir.display()));
        }
    }
    jvm_prefix.extend(opts.extra_jvm_args.iter().cloned());

    let args = build_arguments(version, &vars, platform, opts.memory_mb, &jvm_prefix, &opts.extra_game_args, opts.resolution.is_some());
    Ok(Prepared { java: opts.java.clone(), args, cwd: opts.game_dir.clone() })
}

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// 拼完整参数（不含 java 本身）。顺序：内存与默认 JVM 参数 → 用户 JVM 参数 → 版本 JVM 参数 → 日志配置 → 主类 → 游戏参数 → 用户游戏参数
pub fn build_arguments(
    version: &VersionJson,
    vars: &HashMap<&str, String>,
    platform: &Platform,
    memory_mb: u32,
    extra_jvm: &[String],
    extra_game: &[String],
    custom_resolution: bool,
) -> Vec<String> {
    let features: HashMap<String, bool> = [("has_custom_resolution".to_string(), custom_resolution)].into();
    let mut args: Vec<String> = vec![
        format!("-Xmx{memory_mb}m"),
        "-XX:+UseG1GC".into(),
        "-XX:-UseAdaptiveSizePolicy".into(),
        "-XX:-OmitStackTraceInFastThrow".into(),
        "-Dfile.encoding=UTF-8".into(),
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
        // 旧版本自带的 log4j 有 Log4Shell 漏洞
        "-Dlog4j2.formatMsgNoLookups=true".into(),
        "-Dfml.ignoreInvalidMinecraftCertificates=true".into(),
        "-Dfml.ignorePatchDiscrepancies=true".into(),
    ];
    args.extend(extra_jvm.iter().cloned());

    match version.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(a) => args.extend(a.jvm.iter().flat_map(|x| x.values(platform, &features))),
        None => {
            if platform.os == "osx" {
                args.push("-XstartOnFirstThread".into());
            }
            if platform.os == "windows" {
                args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            }
            args.extend(["-Djava.library.path=${natives_directory}".into(), "-cp".into(), "${classpath}".into()]);
        }
    }
    if vars.contains_key("path") {
        if let Some(client) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
            args.push(client.argument.clone());
        }
    }

    args.push(version.main_class.clone().unwrap_or_else(|| "net.minecraft.client.main.Main".into()));

    let game_args: Vec<String> = match (&version.arguments, &version.minecraft_arguments) {
        (_, Some(legacy)) => {
            let mut v: Vec<String> = legacy.split_whitespace().map(str::to_string).collect();
            if custom_resolution {
                v.extend(["--width".into(), "${resolution_width}".into(), "--height".into(), "${resolution_height}".into()]);
            }
            v
        }
        (Some(a), None) => a.game.iter().flat_map(|x: &Argument| x.values(platform, &features)).collect(),
        (None, None) => vec![],
    };
    args.extend(game_args);
    args.extend(extra_game.iter().cloned());
    args.into_iter().map(|a| substitute(&a, vars)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> HashMap<&'static str, String> {
        [
            ("auth_player_name", "Steve"),
            ("version_name", "1.20.1"),
            ("natives_directory", "/n"),
            ("classpath", "a.jar:b.jar"),
            ("resolution_width", "854"),
            ("resolution_height", "480"),
            ("path", "/log.xml"),
        ]
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect()
    }

    fn mac() -> Platform {
        Platform { os: "osx", arch: "arm64", os_version: String::new() }
    }

    #[test]
    fn substitutes_known_and_keeps_unknown() {
        let v = vars();
        assert_eq!(substitute("--username ${auth_player_name}", &v), "--username Steve");
        assert_eq!(substitute("${nope} x", &v), "${nope} x");
        assert_eq!(substitute("-Dp=${natives_directory}/${natives_directory}", &v), "-Dp=/n//n");
        assert_eq!(substitute("broken ${", &v), "broken ${");
    }

    #[test]
    fn legacy_version_arguments() {
        let version: VersionJson = serde_json::from_str(r#"{
            "id": "1.8.9", "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --version ${version_name}"
        }"#).unwrap();
        let args = build_arguments(&version, &vars(), &mac(), 2048, &["-Dcustom=1".into()], &["--demo".into()], true);
        assert_eq!(args[0], "-Xmx2048m");
        let main = args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        let jvm = &args[..main];
        assert!(jvm.contains(&"-XstartOnFirstThread".to_string()));
        assert!(jvm.contains(&"-Djava.library.path=/n".to_string()));
        assert!(jvm.windows(2).any(|w| w[0] == "-cp" && w[1] == "a.jar:b.jar"));
        assert!(jvm.iter().position(|a| a == "-Dcustom=1") < jvm.iter().position(|a| a == "-cp"));
        assert_eq!(&args[main + 1..], ["--username", "Steve", "--version", "1.20.1", "--width", "854", "--height", "480", "--demo"]);
    }

    #[test]
    fn new_version_arguments_with_logging() {
        let version: VersionJson = serde_json::from_str(r#"{
            "id": "1.20.1", "mainClass": "net.minecraft.client.main.Main",
            "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml",
                "file": {"id": "client-1.12.xml", "url": "u"}}},
            "arguments": {
                "game": ["--username", "${auth_player_name}",
                    {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}], "value": ["--width", "${resolution_width}"]}],
                "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                        "-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]
            }
        }"#).unwrap();
        let args = build_arguments(&version, &vars(), &mac(), 4096, &[], &[], false);
        assert!(args.contains(&"-Dlog4j.configurationFile=/log.xml".to_string()));
        assert!(!args.contains(&"--width".to_string()));
        assert_eq!(args.iter().filter(|a| *a == "-XstartOnFirstThread").count(), 1);
        assert_eq!(args.last().unwrap(), "Steve");
    }

    #[test]
    fn detects_unusual_temp_paths() {
        assert!(is_clean_path(Path::new(r"C:\Users\Steve\AppData\Local\Temp")));
        assert!(!is_clean_path(Path::new(r"C:\Users\STEVE~1\AppData\Local\Temp")), "8.3 短名");
        assert!(!is_clean_path(Path::new(r"C:\Users\张三\AppData\Local\Temp")), "中文用户名");
        assert!(!is_clean_path(Path::new(&format!(r"C:\{}", "a".repeat(80)))), "太长");
        assert_eq!(unix_socket_dir(Path::new(r"C:\Users\Steve\AppData\Local\Temp"), Path::new("unused")), None, "正常路径不加参数");
    }

    #[test]
    fn classpath_ends_with_client_jar() {
        let version: VersionJson = serde_json::from_str(r#"{
            "id": "child", "jar": "1.20.1",
            "libraries": [{"name": "a:b:1", "url": "https://m/"}, {"name": "a:b:1", "url": "https://m/"}]
        }"#).unwrap();
        let game = GameDir::new("/mc");
        let cp = classpath(&game, &version, &mac());
        assert_eq!(cp.len(), 2);
        assert_eq!(cp[1], game.version_jar("1.20.1"));
    }
}
