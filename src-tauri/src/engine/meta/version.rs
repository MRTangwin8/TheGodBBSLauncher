//! version.json 解析：新旧参数格式、规则过滤、inheritsFrom 继承合并。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::Platform;
use crate::engine::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    /// 1.12.2 及以前的旧格式：一整串空格分隔的游戏参数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    /// 1.13+ 的新格式
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub downloads: HashMap<String, DownloadInfo>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// 客户端 jar 所在的版本 id；继承时指向父版本
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jar: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl Argument {
    /// 规则通过时返回参数值（还没替换 `${...}` 占位符）
    pub fn values(&self, platform: &Platform, features: &HashMap<String, bool>) -> Vec<String> {
        match self {
            Argument::Plain(s) => vec![s.clone()],
            Argument::Conditional { rules, value } => {
                if !rules_allow(rules, platform, features) {
                    return vec![];
                }
                match value {
                    ArgValue::One(s) => vec![s.clone()],
                    ArgValue::Many(v) => v.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

impl Rule {
    fn matches(&self, platform: &Platform, features: &HashMap<String, bool>) -> bool {
        if let Some(os) = &self.os {
            if os.name.as_deref().is_some_and(|n| n != platform.os) {
                return false;
            }
            if os.arch.as_deref().is_some_and(|a| a != platform.arch) {
                return false;
            }
            if let Some(v) = &os.version {
                if !platform.os_version.is_empty() && !version_prefix_matches(v, &platform.os_version) {
                    return false;
                }
            }
        }
        if let Some(required) = &self.features {
            for (k, v) in required {
                if features.get(k).copied().unwrap_or(false) != *v {
                    return false;
                }
            }
        }
        true
    }
}

/// Mojang 的 os.version 是正则（如 `^10\\.`），实际只出现前缀匹配，这里不引入正则库。
fn version_prefix_matches(pattern: &str, version: &str) -> bool {
    let prefix: String = pattern
        .trim_start_matches('^')
        .replace("\\.", ".")
        .chars()
        .take_while(|c| !"\\[]()*+?$|".contains(*c))
        .collect();
    version.starts_with(&prefix)
}

/// 规则列表为空则允许；否则从"不允许"开始，按顺序由最后一条匹配的规则决定。
pub fn rules_allow(rules: &[Rule], platform: &Platform, features: &HashMap<String, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule.matches(platform, features) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    pub major_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    /// Maven 坐标 group:artifact:version[:classifier][@ext]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    /// Fabric 等加载器只给 Maven 仓库地址
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
    /// 旧版 natives：系统名 → classifier（可能含 `${arch}`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<Extract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// 要下载的库文件；`path` 相对于 libraries 目录
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFile {
    pub url: String,
    pub path: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

impl Library {
    pub fn is_allowed(&self, platform: &Platform) -> bool {
        rules_allow(&self.rules, platform, &HashMap::new())
    }

    pub fn native_classifier(&self, platform: &Platform) -> Option<String> {
        self.natives
            .as_ref()?
            .get(platform.os)
            .map(|c| c.replace("${arch}", platform.arch_bits()))
    }

    /// 主 jar。只有 natives 分类没有主 jar 的库（如旧版 lwjgl-platform）返回 None。
    pub fn artifact(&self) -> Option<LibraryFile> {
        match &self.downloads {
            Some(d) => {
                let a = d.artifact.as_ref()?;
                let path = a.path.clone().or_else(|| maven_path(&self.name))?;
                // Forge 的部分库 url 为空，表示由安装器本地生成，不需要下载
                if a.url.is_empty() {
                    return None;
                }
                Some(LibraryFile {
                    url: a.url.clone(),
                    path,
                    sha1: a.sha1.clone(),
                    size: a.size,
                })
            }
            None => {
                if self.natives.is_some() {
                    return None;
                }
                let path = maven_path(&self.name)?;
                let base = self.url.as_deref().unwrap_or(MOJANG_LIBRARIES);
                let base = base.trim_end_matches('/');
                Some(LibraryFile {
                    url: format!("{base}/{path}"),
                    path,
                    sha1: self.sha1.clone(),
                    size: self.size,
                })
            }
        }
    }

    /// 旧格式 natives jar（1.18 及以前）
    pub fn native_artifact(&self, platform: &Platform) -> Option<LibraryFile> {
        let classifier = self.native_classifier(platform)?;
        let a = self.downloads.as_ref()?.classifiers.get(&classifier)?;
        let path = a
            .path
            .clone()
            .or_else(|| maven_path(&format!("{}:{classifier}", self.name)))?;
        Some(LibraryFile {
            url: a.url.clone(),
            path,
            sha1: a.sha1.clone(),
            size: a.size,
        })
    }

    /// 去重用的键：group:artifact[:classifier]，不含版本号
    fn dedupe_key(&self) -> String {
        let coord = self.name.split('@').next().unwrap_or(&self.name);
        let parts: Vec<&str> = coord.split(':').collect();
        match parts.as_slice() {
            [g, a, _, c, ..] => format!("{g}:{a}:{c}"),
            [g, a, ..] => format!("{g}:{a}"),
            _ => self.name.clone(),
        }
    }
}

/// Maven 坐标 → 相对路径，如 `org.lwjgl:lwjgl:3.3.1:natives-windows` →
/// `org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar`
pub fn maven_path(coord: &str) -> Option<String> {
    let (coord, ext) = coord.split_once('@').unwrap_or((coord, "jar"));
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let (artifact, version) = (parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

impl VersionJson {
    /// 合并父版本：子版本的标量字段优先；库列表子版本在前并按坐标去重；参数父版本在前。
    pub fn merge_parent(mut self, parent: VersionJson) -> VersionJson {
        let mut seen = HashSet::new();
        let mut libraries = Vec::new();
        for lib in self.libraries.drain(..).chain(parent.libraries) {
            if seen.insert(lib.dedupe_key()) {
                libraries.push(lib);
            }
        }

        let arguments = match (parent.arguments, self.arguments.take()) {
            (None, None) => None,
            (p, c) => {
                let (p, c) = (p.unwrap_or_default(), c.unwrap_or_default());
                Some(Arguments {
                    game: p.game.into_iter().chain(c.game).collect(),
                    jvm: p.jvm.into_iter().chain(c.jvm).collect(),
                })
            }
        };

        VersionJson {
            id: self.id,
            inherits_from: None,
            kind: self.kind.or(parent.kind),
            main_class: self.main_class.or(parent.main_class),
            minecraft_arguments: self.minecraft_arguments.or(parent.minecraft_arguments),
            arguments,
            asset_index: self.asset_index.or(parent.asset_index),
            assets: self.assets.or(parent.assets),
            downloads: if self.downloads.is_empty() {
                parent.downloads
            } else {
                self.downloads
            },
            libraries,
            logging: self.logging.or(parent.logging),
            java_version: self.java_version.or(parent.java_version),
            release_time: self.release_time.or(parent.release_time),
            time: self.time.or(parent.time),
            jar: self.jar.or(parent.jar).or(Some(parent.id)),
        }
    }

    /// 客户端 jar 对应的版本 id
    pub fn jar_id(&self) -> &str {
        self.jar.as_deref().unwrap_or(&self.id)
    }

    /// 从 `versions/<id>/<id>.json` 读取并沿 inheritsFrom 链合并成完整版本。
    pub async fn resolve(versions_dir: &Path, id: &str) -> Result<VersionJson> {
        let mut chain = Vec::new();
        let mut next = Some(id.to_string());
        while let Some(cur) = next {
            if chain.iter().any(|v: &VersionJson| v.id == cur) {
                return Err(Error::Other(format!("版本 {cur} 的 inheritsFrom 出现循环")));
            }
            let path = versions_dir.join(&cur).join(format!("{cur}.json"));
            let text = tokio::fs::read_to_string(&path)
                .await
                .map_err(|_| Error::VersionNotFound(cur.clone()))?;
            let v: VersionJson = serde_json::from_str(&text)?;
            next = v.inherits_from.clone();
            chain.push(v);
        }
        let mut merged = chain.pop().expect("至少有一个版本");
        while let Some(child) = chain.pop() {
            merged = child.merge_parent(merged);
        }
        Ok(merged)
    }
}

/// assets/indexes/<id>.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    /// 1.6 以前：资源要复制到游戏目录 resources/
    #[serde(default)]
    pub map_to_resources: bool,
    /// 1.7.2 附近：资源要复制到 assets/virtual/legacy
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win() -> Platform {
        Platform { os: "windows", arch: "x86_64", os_version: "10.0".into() }
    }
    fn mac_arm() -> Platform {
        Platform { os: "osx", arch: "arm64", os_version: String::new() }
    }

    const VANILLA_1_20_1: &str = r#"{
        "id": "1.20.1", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assetIndex": {"id": "5", "sha1": "abc", "size": 1, "totalSize": 2, "url": "https://piston-meta.mojang.com/v1/packages/abc/5.json"},
        "assets": "5",
        "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
        "downloads": {"client": {"sha1": "c1", "size": 10, "url": "https://piston-data.mojang.com/v1/objects/c1/client.jar"}},
        "arguments": {
            "game": ["--username", "${auth_player_name}",
                     {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"}],
            "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                    {"rules": [{"action": "allow", "os": {"name": "windows", "version": "^10\\."}}], "value": ["-Dos.name=Windows 10", "-Dos.version=10.0"]},
                    "-cp", "${classpath}"]
        },
        "libraries": [
            {"name": "com.mojang:logging:0.1.0", "downloads": {"artifact": {"path": "com/mojang/logging/0.1.0/logging-0.1.0.jar", "sha1": "l1", "size": 1, "url": "https://libraries.minecraft.net/com/mojang/logging/0.1.0/logging-0.1.0.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.1:natives-windows", "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar", "sha1": "n1", "size": 1, "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"}},
             "rules": [{"action": "allow", "os": {"name": "windows"}}]},
            {"name": "org.ow2.asm:asm:9.3", "downloads": {"artifact": {"path": "org/ow2/asm/asm/9.3/asm-9.3.jar", "sha1": "a1", "size": 1, "url": "https://libraries.minecraft.net/org/ow2/asm/asm/9.3/asm-9.3.jar"}}}
        ]
    }"#;

    #[test]
    fn parses_new_format_and_filters_arguments() {
        let v: VersionJson = serde_json::from_str(VANILLA_1_20_1).unwrap();
        let none = HashMap::new();
        let jvm: Vec<String> = v.arguments.as_ref().unwrap().jvm.iter().flat_map(|a| a.values(&win(), &none)).collect();
        assert_eq!(jvm, ["-Dos.name=Windows 10", "-Dos.version=10.0", "-cp", "${classpath}"]);
        let jvm_mac: Vec<String> = v.arguments.as_ref().unwrap().jvm.iter().flat_map(|a| a.values(&mac_arm(), &none)).collect();
        assert_eq!(jvm_mac[0], "-XstartOnFirstThread");
        let game: Vec<String> = v.arguments.as_ref().unwrap().game.iter().flat_map(|a| a.values(&win(), &none)).collect();
        assert!(!game.contains(&"--demo".to_string()));
        assert_eq!(v.java_version.unwrap().major_version, 17);
    }

    #[test]
    fn library_rules_respect_os() {
        let v: VersionJson = serde_json::from_str(VANILLA_1_20_1).unwrap();
        let on_win: Vec<_> = v.libraries.iter().filter(|l| l.is_allowed(&win())).collect();
        let on_mac: Vec<_> = v.libraries.iter().filter(|l| l.is_allowed(&mac_arm())).collect();
        assert_eq!(on_win.len(), 3);
        assert_eq!(on_mac.len(), 2);
    }

    #[test]
    fn disallow_rule_wins_when_last() {
        // 旧版 lwjgl 在 osx 上的写法：allow 全部，再 disallow osx
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#,
        ).unwrap();
        assert!(rules_allow(&rules, &win(), &HashMap::new()));
        assert!(!rules_allow(&rules, &mac_arm(), &HashMap::new()));
    }

    #[test]
    fn legacy_natives_classifier() {
        let lib: Library = serde_json::from_str(r#"{
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
            "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows-${arch}"},
            "downloads": {"classifiers": {
                "natives-windows-64": {"path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.4-nightly-20150209/lwjgl-platform-2.9.4-nightly-20150209-natives-windows-64.jar", "sha1": "w64", "size": 1, "url": "https://libraries.minecraft.net/x.jar"}
            }}
        }"#).unwrap();
        assert!(lib.artifact().is_none());
        let n = lib.native_artifact(&win()).unwrap();
        assert_eq!(n.sha1.as_deref(), Some("w64"));
        assert!(lib.native_artifact(&mac_arm()).is_none());
    }

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(),
            "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
        assert_eq!(maven_path("org.lwjgl:lwjgl:3.3.1:natives-macos-arm64").unwrap(),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar");
        assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(),
            "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
        assert!(maven_path("bad").is_none());
    }

    #[test]
    fn maven_only_library_uses_repo_url() {
        let lib: Library = serde_json::from_str(
            r#"{"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"}"#,
        ).unwrap();
        assert_eq!(lib.artifact().unwrap().url,
            "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar");
    }

    #[test]
    fn merges_inherits_from() {
        let parent: VersionJson = serde_json::from_str(VANILLA_1_20_1).unwrap();
        let child: VersionJson = serde_json::from_str(r#"{
            "id": "fabric-loader-0.16.9-1.20.1", "inheritsFrom": "1.20.1",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
            "libraries": [
                {"name": "org.ow2.asm:asm:9.6", "url": "https://maven.fabricmc.net/"},
                {"name": "net.fabricmc:fabric-loader:0.16.9", "url": "https://maven.fabricmc.net/"}
            ]
        }"#).unwrap();
        let m = child.merge_parent(parent);
        assert_eq!(m.main_class.as_deref(), Some("net.fabricmc.loader.impl.launch.knot.KnotClient"));
        assert_eq!(m.jar_id(), "1.20.1");
        assert!(m.inherits_from.is_none());
        assert_eq!(m.asset_index.as_ref().unwrap().id, "5");
        // asm 被子版本的 9.6 覆盖，不会出现两份
        let asm: Vec<_> = m.libraries.iter().filter(|l| l.name.starts_with("org.ow2.asm:asm:")).collect();
        assert_eq!(asm.len(), 1);
        assert_eq!(asm[0].name, "org.ow2.asm:asm:9.6");
        // natives 带 classifier，不会被当成重复
        assert_eq!(m.libraries.len(), 4);
        // 父版本参数在前
        let jvm = &m.arguments.as_ref().unwrap().jvm;
        assert!(matches!(jvm.last(), Some(Argument::Plain(s)) if s.starts_with("-DFabricMcEmu")));
    }

    #[test]
    fn parses_legacy_minecraft_arguments() {
        let v: VersionJson = serde_json::from_str(r#"{
            "id": "1.8.9", "minecraftArguments": "--username ${auth_player_name} --version ${version_name}",
            "mainClass": "net.minecraft.client.main.Main", "libraries": []
        }"#).unwrap();
        assert!(v.arguments.is_none());
        assert!(v.minecraft_arguments.unwrap().starts_with("--username"));
    }

    #[tokio::test]
    async fn resolve_reads_chain_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        for (id, body) in [
            ("1.20.1", VANILLA_1_20_1.to_string()),
            ("child", r#"{"id":"child","inheritsFrom":"1.20.1","libraries":[]}"#.to_string()),
        ] {
            std::fs::create_dir_all(dir.path().join(id)).unwrap();
            std::fs::write(dir.path().join(id).join(format!("{id}.json")), body).unwrap();
        }
        let v = VersionJson::resolve(dir.path(), "child").await.unwrap();
        assert_eq!(v.id, "child");
        assert_eq!(v.libraries.len(), 3);
        assert!(matches!(VersionJson::resolve(dir.path(), "nope").await, Err(Error::VersionNotFound(_))));
    }
}
