//! 模组兼容性检查（下载时自动做）：
//! 1. 平台：Forge / NeoForge / Fabric 模组装错了加载器
//! 2. 依赖声明：模组要求的 Minecraft / 加载器版本范围
//! 3. Mixin 注入点：模组要修改的游戏方法在当前游戏 jar 里是否真的存在。
//!    只看必需的注入（require ≥ 1），只在游戏代码是可读名字（NeoForge 1.20.5+、MC 26.x 等）时做，
//!    否则说明"无法核对"而不是乱猜。

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::classfile::{self, ElementValue};
use crate::engine::install::GameDir;
use crate::engine::loader::{self, compare_versions, LoaderKind};
use crate::engine::meta::version::{base_minecraft_version, Argument};
use crate::engine::meta::VersionJson;
use crate::engine::Result;

/// 安装目标的游戏环境
#[derive(Debug, Clone)]
pub struct GameEnv {
    pub mc: String,
    pub loader: Option<LoaderKind>,
    pub loader_version: Option<String>,
    /// 运行时真正加载的游戏 jar（NeoForge 是打过补丁的那个）
    pub runtime_jar: Option<PathBuf>,
}

fn arg_after(version: &VersionJson, flag: &str) -> Option<String> {
    let args: Vec<&str> = version
        .arguments
        .iter()
        .flat_map(|a| &a.game)
        .filter_map(|a| match a {
            Argument::Plain(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].to_string())
}

fn lib_version(version: &VersionJson, prefix: &str) -> Option<String> {
    version.libraries.iter().find_map(|l| l.name.strip_prefix(prefix).map(|v| v.split(':').next().unwrap_or(v).to_string()))
}

impl GameEnv {
    pub async fn detect(game: &GameDir, version_id: &str) -> Result<Self> {
        let merged = VersionJson::resolve(&game.versions(), version_id).await?;
        let mc = base_minecraft_version(&game.versions(), version_id).await?;
        let loader = loader::detect(&merged);
        let loader_version = match loader {
            Some(LoaderKind::NeoForge) => arg_after(&merged, "--fml.neoForgeVersion"),
            Some(LoaderKind::Forge) => arg_after(&merged, "--fml.forgeVersion").or_else(|| {
                lib_version(&merged, "net.minecraftforge:forge:").map(|v| v.trim_start_matches(&format!("{mc}-")).to_string())
            }),
            Some(LoaderKind::Fabric) => lib_version(&merged, "net.fabricmc:fabric-loader:"),
            Some(LoaderKind::Quilt) => lib_version(&merged, "org.quiltmc:quilt-loader:"),
            None => None,
        };
        let libs = game.libraries();
        let candidates = match (loader, &loader_version) {
            // 26.x：minecraft-client-patched；20.x/21.x：neoforge-<v>-client.jar
            (Some(LoaderKind::NeoForge), Some(v)) => vec![
                libs.join("net/neoforged/minecraft-client-patched").join(v).join(format!("minecraft-client-patched-{v}.jar")),
                libs.join("net/neoforged/neoforge").join(v).join(format!("neoforge-{v}-client.jar")),
                libs.join("net/neoforged/forge").join(format!("{mc}-{v}")).join(format!("forge-{mc}-{v}-client.jar")),
            ],
            (Some(LoaderKind::Forge), Some(v)) => vec![libs.join("net/minecraftforge/forge").join(format!("{mc}-{v}")).join(format!("forge-{mc}-{v}-client.jar"))],
            _ => vec![game.version_jar(&mc)],
        };
        let runtime_jar = candidates.into_iter().find(|p| p.is_file());
        Ok(Self { mc, loader, loader_version, runtime_jar })
    }
}

/// 一个类的方法列表：(名字, 描述符)
type Methods = Vec<(String, String)>;

/// 游戏 jar 里的类，按需解析并缓存
pub struct RuntimeJar {
    zip: Mutex<zip::ZipArchive<std::fs::File>>,
    cache: Mutex<HashMap<String, Option<Methods>>>,
}

impl RuntimeJar {
    pub fn open(path: &Path) -> Option<Self> {
        let zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
        Some(Self { zip: Mutex::new(zip), cache: Mutex::new(HashMap::new()) })
    }

    /// 类的方法 (名字, 描述符)；类不在游戏 jar 里返回 None
    pub fn methods(&self, class: &str) -> Option<Methods> {
        if let Some(hit) = self.cache.lock().ok()?.get(class) {
            return hit.clone();
        }
        let parsed = (|| {
            let mut zip = self.zip.lock().ok()?;
            let mut entry = zip.by_name(&format!("{class}.class")).ok()?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            let info = classfile::parse(&buf).ok()?;
            Some(info.methods.into_iter().map(|m| (m.name, m.desc)).collect::<Vec<_>>())
        })();
        self.cache.lock().ok()?.insert(class.to_string(), parsed.clone());
        parsed
    }

    /// 游戏代码是不是可读名字：混淆/SRG 环境下 Minecraft.getInstance 不叫这个名字
    pub fn readable(&self) -> bool {
        self.methods("net/minecraft/client/Minecraft").is_some_and(|m| m.iter().any(|(n, _)| n == "getInstance"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompatStatus {
    Ok,
    Problem,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatReport {
    pub status: CompatStatus,
    pub problems: Vec<String>,
    /// 做了哪些检查（"已核对 37 个注入点"之类）
    pub notes: Vec<String>,
}

// ---------- 版本范围 ----------

/// Maven 版本范围（mods.toml 用）：`[1.20,1.21)`、`[47,)`、`[1.0]`；不带括号的是"推荐版本"，任何版本都行
pub fn maven_range_matches(range: &str, version: &str) -> bool {
    let range = range.trim();
    if range.is_empty() || range == "*" || !range.starts_with(['[', '(']) {
        return true;
    }
    // 加载器用 Maven 原样解析，不会去掉空格（"[1.21, 1.21.1)" 实际能在 1.21.1 上加载）。
    // 这种写法的真实效果说不准，宁可不判断也不误报
    if range.contains(char::is_whitespace) {
        return true;
    }
    let mut rest = range;
    while let Some(start) = rest.find(['[', '(']) {
        let Some(end_rel) = rest[start..].find([']', ')']) else { return true };
        let end = start + end_rel;
        let (open, close) = (&rest[start..=start], &rest[end..=end]);
        let body = &rest[start + 1..end];
        let ok = match body.split_once(',') {
            None => compare_versions(version, body.trim()).is_eq(),
            Some((lo, hi)) => {
                let (lo, hi) = (lo.trim(), hi.trim());
                let lo_ok = lo.is_empty() || {
                    let c = compare_versions(version, lo);
                    if open == "[" { c.is_ge() } else { c.is_gt() }
                };
                let hi_ok = hi.is_empty() || {
                    let c = compare_versions(version, hi);
                    if close == "]" { c.is_le() } else { c.is_lt() }
                };
                lo_ok && hi_ok
            }
        };
        if ok {
            return true;
        }
        rest = &rest[end + 1..];
    }
    false
}

fn bump(v: &str, idx: usize) -> String {
    let mut nums: Vec<u64> = v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    while nums.len() <= idx {
        nums.push(0);
    }
    nums[idx] += 1;
    nums.truncate(idx + 1);
    nums.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(".")
}

fn fabric_term_matches(term: &str, version: &str) -> bool {
    let term = term.trim();
    if term.is_empty() || term == "*" {
        return true;
    }
    for op in [">=", "<=", ">", "<", "=", "~", "^"] {
        if let Some(v) = term.strip_prefix(op) {
            let c = compare_versions(version, v);
            return match op {
                ">=" => c.is_ge(),
                "<=" => c.is_le(),
                ">" => c.is_gt(),
                "<" => c.is_lt(),
                "=" => c.is_eq(),
                "~" => c.is_ge() && compare_versions(version, &bump(v, 1)).is_lt(),
                _ => c.is_ge() && compare_versions(version, &bump(v, 0)).is_lt(),
            };
        }
    }
    // 1.21.x / 1.21.* 通配
    if let Some(prefix) = term.strip_suffix(".x").or_else(|| term.strip_suffix(".X")).or_else(|| term.strip_suffix(".*")) {
        return version == prefix || version.starts_with(&format!("{prefix}."));
    }
    compare_versions(version, term).is_eq()
}

/// Fabric 版本谓词：字符串里空格分隔为"且"，数组为"或"
pub fn fabric_predicate_matches(pred: &Value, version: &str) -> bool {
    match pred {
        Value::String(s) => s.split_whitespace().all(|t| fabric_term_matches(t, version)) || s.trim().is_empty(),
        Value::Array(items) => items.is_empty() || items.iter().any(|i| fabric_predicate_matches(i, version)),
        _ => true,
    }
}

/// 只有正式版号才比较；快照（24w44a）、pre/rc 之类跳过，免得误判
fn plain_version(v: &str) -> bool {
    !v.is_empty() && v.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

// ---------- jar 内容 ----------

type Unit = zip::ZipArchive<Cursor<Vec<u8>>>;

fn read(unit: &mut Unit, name: &str) -> Option<Vec<u8>> {
    let mut e = unit.by_name(name).ok()?;
    let mut buf = Vec::new();
    e.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// 外层 jar + Jar-in-Jar 里嵌套的 jar（NeoForge: META-INF/jarjar，Fabric: META-INF/jars）
fn units(bytes: Vec<u8>, depth: usize, out: &mut Vec<Unit>) {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(bytes)) else { return };
    if depth < 2 {
        let nested: Vec<String> = zip
            .file_names()
            .filter(|n| (n.starts_with("META-INF/jarjar/") || n.starts_with("META-INF/jars/")) && n.ends_with(".jar"))
            .map(str::to_string)
            .collect();
        for n in nested {
            if let Some(b) = read(&mut zip, &n) {
                units(b, depth + 1, out);
            }
        }
    }
    out.push(zip);
}

#[derive(Default)]
struct Platforms {
    neoforge: bool,
    forge: bool,
    fabric: bool,
    quilt: bool,
}

fn mixin_configs(unit: &mut Unit, env: &GameEnv) -> Vec<String> {
    let mut out = Vec::new();
    let toml_name = match env.loader {
        Some(LoaderKind::NeoForge) if unit.by_name("META-INF/neoforge.mods.toml").is_ok() => Some("META-INF/neoforge.mods.toml"),
        Some(LoaderKind::NeoForge) | Some(LoaderKind::Forge) => Some("META-INF/mods.toml"),
        _ => None,
    };
    if let Some(t) = toml_name.and_then(|n| read(unit, n)).and_then(|b| String::from_utf8(b).ok()).and_then(|s| s.parse::<toml::Table>().ok()) {
        for m in t.get("mixins").and_then(|v| v.as_array()).into_iter().flatten() {
            // 依赖其它模组才启用的配置不检查
            if m.get("requiredMods").is_some() {
                continue;
            }
            if let Some(c) = m.get("config").and_then(|v| v.as_str()) {
                out.push(c.to_string());
            }
        }
    }
    if matches!(env.loader, Some(LoaderKind::Fabric) | Some(LoaderKind::Quilt)) {
        if let Some(v) = read(unit, "fabric.mod.json").and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            for m in v["mixins"].as_array().into_iter().flatten() {
                match m {
                    Value::String(s) => out.push(s.clone()),
                    Value::Object(o) if o.get("environment").and_then(|e| e.as_str()) != Some("server") => {
                        out.extend(o.get("config").and_then(|c| c.as_str()).map(str::to_string));
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(manifest) = read(unit, "META-INF/MANIFEST.MF").map(|b| String::from_utf8_lossy(&b).into_owned()) {
        if let Some(line) = manifest.lines().find_map(|l| l.strip_prefix("MixinConfigs:")) {
            out.extend(line.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 解析选择器 `name(desc)ret` / `name` / `Lowner;name(desc)V`；通配、正则、@Desc 之类返回 None
fn parse_selector(s: &str) -> Option<(String, Option<String>)> {
    let s = s.trim();
    if s.is_empty() || s.starts_with('/') || s.contains('*') || s.contains('@') {
        return None;
    }
    let s = match (s.starts_with('L'), s.find(';')) {
        (true, Some(i)) if s[..i].contains('/') => &s[i + 1..],
        _ => s,
    };
    Some(match s.find('(') {
        Some(i) => (s[..i].to_string(), Some(s[i..].to_string())),
        None => (s.to_string(), None),
    })
}

fn is_injector(desc: &str) -> bool {
    const SPONGE: [&str; 6] = ["Inject", "Redirect", "ModifyArg", "ModifyArgs", "ModifyVariable", "ModifyConstant"];
    desc.strip_prefix("Lorg/spongepowered/asm/mixin/injection/")
        .and_then(|r| r.strip_suffix(';'))
        .is_some_and(|n| SPONGE.contains(&n))
        || desc.starts_with("Lcom/llamalad7/mixinextras/injector/")
}

fn short(class: &str) -> &str {
    class.rsplit(['/', '.']).next().unwrap_or(class)
}

struct MixinResult {
    checked: usize,
    problems: Vec<String>,
}

fn check_mixins(unit: &mut Unit, env: &GameEnv, runtime: &RuntimeJar, out: &mut MixinResult) {
    for config in mixin_configs(unit, env) {
        let Some(cfg) = read(unit, &config).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else { continue };
        let package = cfg["package"].as_str().unwrap_or_default().replace('.', "/");
        let default_require = cfg["injectors"]["defaultRequire"].as_i64().unwrap_or(0);
        let classes = cfg["mixins"].as_array().into_iter().flatten().chain(cfg["client"].as_array().into_iter().flatten());
        for name in classes.filter_map(|v| v.as_str()) {
            let path = format!("{package}/{}.class", name.replace('.', "/"));
            let Some(info) = read(unit, &path).and_then(|b| classfile::parse(&b).ok()) else { continue };
            // @Pseudo：目标可能根本不存在，本来就是可选的
            if info.annotation("Lorg/spongepowered/asm/mixin/Pseudo;").is_some() {
                continue;
            }
            let Some(mixin) = info.annotation("Lorg/spongepowered/asm/mixin/Mixin;") else { continue };
            let mut targets: Vec<String> = Vec::new();
            if let Some(ElementValue::Array(v)) = mixin.get("value") {
                targets.extend(v.iter().filter_map(|e| match e {
                    ElementValue::Class(d) => d.strip_prefix('L').and_then(|d| d.strip_suffix(';')).map(str::to_string),
                    _ => None,
                }));
            }
            if let Some(ElementValue::Array(v)) = mixin.get("targets") {
                targets.extend(v.iter().filter_map(|e| match e {
                    ElementValue::Str(s) => Some(s.replace('.', "/")),
                    _ => None,
                }));
            }
            for method in &info.methods {
                for ann in method.annotations.iter().filter(|a| is_injector(&a.desc)) {
                    let require = match ann.get("require") {
                        Some(ElementValue::Int(r)) => *r,
                        _ => default_require,
                    };
                    if require < 1 {
                        continue;
                    }
                    let selectors: Vec<(String, Option<String>)> = match ann.get("method") {
                        Some(ElementValue::Array(v)) => v
                            .iter()
                            .filter_map(|e| match e {
                                ElementValue::Str(s) => parse_selector(s),
                                _ => None,
                            })
                            .collect(),
                        _ => vec![],
                    };
                    if selectors.is_empty() {
                        continue;
                    }
                    for target in &targets {
                        // 目标不是游戏类（可能是加载器或别的模组的类），没法核对
                        let Some(methods) = runtime.methods(target) else { continue };
                        out.checked += 1;
                        let found = selectors.iter().any(|(n, d)| methods.iter().any(|(mn, md)| mn == n && d.as_ref().is_none_or(|d| d == md)));
                        if !found {
                            let (n, d) = &selectors[0];
                            out.problems.push(format!(
                                "{} 要修改的 {}.{}{} 在当前游戏里不存在",
                                short(name),
                                short(target),
                                n,
                                d.as_deref().map(|d| if d.len() > 60 { "(…)" } else { d }).unwrap_or_default()
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// 检查一个模组 jar 能否在 `env` 里运行
pub fn check_jar(path: &Path, env: &GameEnv, runtime: Option<&RuntimeJar>) -> CompatReport {
    let Ok(bytes) = std::fs::read(path) else {
        return CompatReport { status: CompatStatus::Unknown, problems: vec![], notes: vec!["无法读取文件".into()] };
    };
    let mut list = Vec::new();
    units(bytes, 0, &mut list);

    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut plat = Platforms::default();
    let mc_plain = plain_version(&env.mc);

    for unit in &mut list {
        let has = |u: &mut Unit, n: &str| u.by_name(n).is_ok();
        plat.neoforge |= has(unit, "META-INF/neoforge.mods.toml");
        plat.forge |= has(unit, "META-INF/mods.toml");
        plat.fabric |= has(unit, "fabric.mod.json");
        plat.quilt |= has(unit, "quilt.mod.json");

        // 依赖声明里的游戏 / 加载器版本范围
        if matches!(env.loader, Some(LoaderKind::NeoForge) | Some(LoaderKind::Forge)) {
            let name = if env.loader == Some(LoaderKind::NeoForge) && has(unit, "META-INF/neoforge.mods.toml") { "META-INF/neoforge.mods.toml" } else { "META-INF/mods.toml" };
            let loader_id = if env.loader == Some(LoaderKind::NeoForge) { "neoforge" } else { "forge" };
            if let Some(t) = read(unit, name).and_then(|b| String::from_utf8(b).ok()).and_then(|s| s.parse::<toml::Table>().ok()) {
                for deps in t.get("dependencies").and_then(|d| d.as_table()).into_iter().flat_map(|d| d.values()) {
                    for dep in deps.as_array().into_iter().flatten() {
                        let required = match (dep.get("type").and_then(|v| v.as_str()), dep.get("mandatory").and_then(|v| v.as_bool())) {
                            (Some(t), _) => t.eq_ignore_ascii_case("required"),
                            (None, Some(m)) => m,
                            (None, None) => true,
                        };
                        let side_ok = dep.get("side").and_then(|v| v.as_str()).is_none_or(|s| !s.eq_ignore_ascii_case("SERVER"));
                        let (Some(id), Some(range)) = (dep.get("modId").and_then(|v| v.as_str()), dep.get("versionRange").and_then(|v| v.as_str())) else { continue };
                        if !required || !side_ok {
                            continue;
                        }
                        if id == "minecraft" && mc_plain && !maven_range_matches(range, &env.mc) {
                            problems.push(format!("需要 Minecraft {range}，当前是 {}", env.mc));
                        }
                        if id == loader_id {
                            if let Some(lv) = &env.loader_version {
                                if !maven_range_matches(range, lv) {
                                    problems.push(format!("需要 {loader_id} {range}，当前是 {lv}"));
                                }
                            }
                        }
                    }
                }
            }
        }
        if matches!(env.loader, Some(LoaderKind::Fabric)) {
            if let Some(v) = read(unit, "fabric.mod.json").and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                if let Some(p) = v["depends"].get("minecraft") {
                    if mc_plain && !fabric_predicate_matches(p, &env.mc) {
                        problems.push(format!("需要 Minecraft {}，当前是 {}", p.to_string().trim_matches('"'), env.mc));
                    }
                }
                if let (Some(p), Some(lv)) = (v["depends"].get("fabricloader"), &env.loader_version) {
                    if !fabric_predicate_matches(p, lv) {
                        problems.push(format!("需要 Fabric Loader {}，当前是 {lv}", p.to_string().trim_matches('"')));
                    }
                }
            }
        }
    }

    // 平台
    let platform_problem = match env.loader {
        Some(LoaderKind::NeoForge) if !plat.neoforge => {
            let new_neo = compare_versions(&env.mc, "1.20.5").is_ge();
            if plat.forge && new_neo {
                Some("这是 Forge 模组，NeoForge 1.20.5 起不再兼容 Forge 模组")
            } else if !plat.forge && (plat.fabric || plat.quilt) {
                Some("这是 Fabric 模组，NeoForge 不能加载")
            } else {
                None
            }
        }
        Some(LoaderKind::Forge) if !plat.forge => {
            if plat.neoforge {
                Some("这是 NeoForge 模组，Forge 不能加载")
            } else if plat.fabric || plat.quilt {
                Some("这是 Fabric 模组，Forge 不能加载")
            } else {
                None
            }
        }
        Some(LoaderKind::Fabric) if !plat.fabric => {
            if plat.quilt {
                Some("这是 Quilt 专用模组，Fabric 不能加载")
            } else if plat.forge || plat.neoforge {
                Some("这是 Forge/NeoForge 模组，Fabric 不能加载")
            } else {
                None
            }
        }
        Some(LoaderKind::Quilt) if !plat.fabric && !plat.quilt && (plat.forge || plat.neoforge) => Some("这是 Forge/NeoForge 模组，Quilt 不能加载"),
        _ => None,
    };
    if let Some(p) = platform_problem {
        problems.insert(0, p.to_string());
    }
    let is_mod = plat.neoforge || plat.forge || plat.fabric || plat.quilt;

    // Mixin 注入点（平台都不对就不用看了）
    if platform_problem.is_none() && is_mod {
        match runtime {
            Some(rt) if rt.readable() => {
                let mut r = MixinResult { checked: 0, problems: vec![] };
                for unit in &mut list {
                    check_mixins(unit, env, rt, &mut r);
                }
                if r.checked > 0 {
                    notes.push(format!("已核对 {} 个必需的 Mixin 注入点", r.checked));
                }
                let extra = r.problems.len().saturating_sub(3);
                problems.extend(r.problems.into_iter().take(3));
                if extra > 0 {
                    problems.push(format!("……另有 {extra} 处注入点不存在"));
                }
            }
            Some(_) => notes.push("当前游戏代码是混淆名，跳过了注入点检查".into()),
            None => notes.push("找不到游戏 jar，跳过了注入点检查".into()),
        }
    }

    // 外层和嵌套 jar 可能声明了同样的依赖
    let mut seen = std::collections::HashSet::new();
    problems.retain(|p| seen.insert(p.clone()));

    let status = if !problems.is_empty() {
        CompatStatus::Problem
    } else if is_mod {
        CompatStatus::Ok
    } else {
        notes.push("不是能识别的模组文件".into());
        CompatStatus::Unknown
    };
    CompatReport { status, problems, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::resources::classfile::tests::ClassBuilder;
    use std::io::Write;

    #[test]
    fn maven_ranges() {
        assert!(maven_range_matches("[26.1.2.10-beta,)", "26.3.0.97"));
        assert!(!maven_range_matches("[26.3.1,)", "26.3.0.97"));
        assert!(maven_range_matches("[1.20,1.21)", "1.20.1"));
        assert!(!maven_range_matches("[1.20,1.21)", "1.21"));
        assert!(maven_range_matches("[1.20,1.21]", "1.21"));
        assert!(maven_range_matches("(,1.19],[1.20.1,)", "1.20.4"));
        assert!(!maven_range_matches("(,1.19],[1.20.1,)", "1.19.4"));
        assert!(maven_range_matches("[1.21.1]", "1.21.1"));
        assert!(maven_range_matches("47", "40.0.1"), "裸版本号是推荐版本，不限制");
        assert!(maven_range_matches("[1.21, 1.21.1)", "1.21.1"), "JEI 真实写法：带空格的范围不判断");
        assert!(maven_range_matches("*", "1.0"));
    }

    #[test]
    fn fabric_predicates() {
        let p = |s: &str| Value::String(s.into());
        assert!(fabric_predicate_matches(&p(">=0.15.0"), "0.16.9"));
        assert!(!fabric_predicate_matches(&p(">=0.17.0"), "0.16.9"));
        assert!(fabric_predicate_matches(&p("~1.21"), "1.21.1"));
        assert!(!fabric_predicate_matches(&p("~1.21.1"), "1.22"));
        assert!(fabric_predicate_matches(&p("1.21.x"), "1.21.4"));
        assert!(!fabric_predicate_matches(&p("1.21.x"), "1.20.1"));
        assert!(fabric_predicate_matches(&p(">=1.21 <1.22"), "1.21.1"));
        assert!(fabric_predicate_matches(&serde_json::json!(["1.20.1", "1.21.1"]), "1.21.1"));
        assert!(!fabric_predicate_matches(&serde_json::json!(["1.20.1", "1.21"]), "1.21.1"));
        assert!(fabric_predicate_matches(&p("*"), "26.3"));
    }

    fn zip_bytes(files: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    /// 造一个"游戏 jar"：Minecraft.getInstance + LevelExtractor.extract(F)V
    fn runtime(dir: &Path) -> RuntimeJar {
        let mut mc = ClassBuilder::default();
        mc.method("getInstance", "()Lnet/minecraft/client/Minecraft;", vec![]);
        let mut le = ClassBuilder::default();
        le.method("extract", "(F)V", vec![]);
        let path = dir.join("client.jar");
        std::fs::write(
            &path,
            zip_bytes(vec![
                ("net/minecraft/client/Minecraft.class", mc.build("net/minecraft/client/Minecraft")),
                ("net/minecraft/client/LevelExtractor.class", le.build("net/minecraft/client/LevelExtractor")),
            ]),
        )
        .unwrap();
        RuntimeJar::open(&path).unwrap()
    }

    /// 造一个 NeoForge 模组：一个 Mixin 注入 LevelExtractor 的 `selector`，可以套一层 jar-in-jar
    fn neoforge_mod(dir: &Path, selector: &str, require: Option<i32>, default_require: i32, nested: bool, neo_range: &str) -> PathBuf {
        let mut b = ClassBuilder::default();
        let target = b.class_value("Lnet/minecraft/client/LevelExtractor;");
        let mixin = b.annotation("Lorg/spongepowered/asm/mixin/Mixin;", vec![("value", ClassBuilder::array_value(vec![target]))]);
        b.class_annotation(mixin);
        let sel = b.str_value(selector);
        let mut values = vec![("method", ClassBuilder::array_value(vec![sel]))];
        if let Some(r) = require {
            values.push(("require", b.int_value(r)));
        }
        let inject = b.annotation("Lorg/spongepowered/asm/mixin/injection/Inject;", values);
        b.method("handler", "(Lorg/spongepowered/asm/mixin/injection/callback/CallbackInfo;)V", vec![inject]);
        let toml = format!(
            "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\n[[mods]]\nmodId=\"testmod\"\n[[mixins]]\nconfig=\"test.mixins.json\"\n[[dependencies.testmod]]\nmodId=\"neoforge\"\ntype=\"required\"\nversionRange=\"{neo_range}\"\n"
        );
        let cfg = format!(r#"{{"package":"me.test.mixin","injectors":{{"defaultRequire":{default_require}}},"client":["LevelExtractorMixin"]}}"#);
        let inner = zip_bytes(vec![
            ("META-INF/neoforge.mods.toml", toml.into_bytes()),
            ("test.mixins.json", cfg.into_bytes()),
            ("me/test/mixin/LevelExtractorMixin.class", b.build("me/test/mixin/LevelExtractorMixin")),
        ]);
        let bytes = if nested {
            zip_bytes(vec![("META-INF/neoforge.mods.toml", b"modLoader=\"javafml\"\n".to_vec()), ("META-INF/jarjar/inner.jar", inner)])
        } else {
            inner
        };
        let path = dir.join(format!("mod-{}.jar", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn env() -> GameEnv {
        GameEnv { mc: "26.3".into(), loader: Some(LoaderKind::NeoForge), loader_version: Some("26.3.0.97".into()), runtime_jar: None }
    }

    #[test]
    fn finds_missing_injection_target_inside_jar_in_jar() {
        let dir = tempfile::tempdir().unwrap();
        let rt = runtime(dir.path());
        assert!(rt.readable());
        // 和 Sodium 0.9.2 一样：defaultRequire=1，目标方法签名在新版里变了
        let bad = neoforge_mod(dir.path(), "extract(FLjava/lang/Object;)V", None, 1, true, "[26.1,)");
        let r = check_jar(&bad, &env(), Some(&rt));
        assert_eq!(r.status, CompatStatus::Problem, "{r:?}");
        assert!(r.problems[0].contains("LevelExtractor.extract"), "{r:?}");

        let good = neoforge_mod(dir.path(), "extract(F)V", None, 1, true, "[26.1,)");
        let r = check_jar(&good, &env(), Some(&rt));
        assert_eq!(r.status, CompatStatus::Ok, "{r:?}");
        assert!(r.notes[0].contains("1 个"));

        let by_name = neoforge_mod(dir.path(), "extract", None, 1, false, "[26.1,)");
        assert_eq!(check_jar(&by_name, &env(), Some(&rt)).status, CompatStatus::Ok, "只写方法名也算");
    }

    #[test]
    fn optional_injections_are_not_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let rt = runtime(dir.path());
        let optional = neoforge_mod(dir.path(), "gone()V", None, 0, false, "[26.1,)");
        assert_eq!(check_jar(&optional, &env(), Some(&rt)).status, CompatStatus::Ok, "require=0 时找不到目标不会崩");
        let forced = neoforge_mod(dir.path(), "gone()V", Some(1), 0, false, "[26.1,)");
        assert_eq!(check_jar(&forced, &env(), Some(&rt)).status, CompatStatus::Problem, "注解上写了 require=1");
    }

    #[test]
    fn declared_loader_range_and_platform() {
        let dir = tempfile::tempdir().unwrap();
        let too_new = neoforge_mod(dir.path(), "extract(F)V", None, 1, false, "[26.4,)");
        let r = check_jar(&too_new, &env(), None);
        assert_eq!(r.status, CompatStatus::Problem);
        assert!(r.problems[0].contains("neoforge [26.4,)"), "{r:?}");

        let fabric_mod = dir.path().join("fabric.jar");
        std::fs::write(&fabric_mod, zip_bytes(vec![("fabric.mod.json", br#"{"schemaVersion":1,"id":"x","depends":{"minecraft":"~1.21"}}"#.to_vec())])).unwrap();
        let r = check_jar(&fabric_mod, &env(), None);
        assert_eq!(r.problems[0], "这是 Fabric 模组，NeoForge 不能加载");

        let fabric_env = GameEnv { mc: "1.20.1".into(), loader: Some(LoaderKind::Fabric), loader_version: Some("0.16.9".into()), runtime_jar: None };
        let r = check_jar(&fabric_mod, &fabric_env, None);
        assert!(r.problems.iter().any(|p| p.contains("需要 Minecraft ~1.21")), "{r:?}");
    }

    #[test]
    fn obfuscated_runtime_is_skipped_not_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let mut mc = ClassBuilder::default();
        mc.method("m_91087_", "()Lnet/minecraft/client/Minecraft;", vec![]);
        let path = dir.path().join("srg.jar");
        std::fs::write(&path, zip_bytes(vec![("net/minecraft/client/Minecraft.class", mc.build("net/minecraft/client/Minecraft"))])).unwrap();
        let rt = RuntimeJar::open(&path).unwrap();
        assert!(!rt.readable());
        let m = neoforge_mod(dir.path(), "whatever()V", None, 1, false, "[26.1,)");
        let r = check_jar(&m, &env(), Some(&rt));
        assert_eq!(r.status, CompatStatus::Ok);
        assert!(r.notes.iter().any(|n| n.contains("混淆")));
    }

    #[test]
    fn selectors() {
        assert_eq!(parse_selector("tick()V"), Some(("tick".into(), Some("()V".into()))));
        assert_eq!(parse_selector("Lnet/minecraft/A;render(F)V"), Some(("render".into(), Some("(F)V".into()))));
        assert_eq!(parse_selector("<init>"), Some(("<init>".into(), None)));
        assert_eq!(parse_selector("render*"), None);
        assert_eq!(parse_selector("/rend.*/"), None);
    }
}
