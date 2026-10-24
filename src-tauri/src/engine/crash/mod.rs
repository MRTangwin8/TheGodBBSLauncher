//! 崩溃分析：用 JSON 规则库匹配游戏日志 / 崩溃报告 / JVM 错误日志，输出中文原因和修复建议。
//!
//! 规则格式见 `rules.json`。规则可以热更新：用户目录下的 crash-rules.json 会按 id 覆盖内置规则。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::engine::{Error, Result};

/// 内置规则库
pub const BUILTIN_RULES: &str = include_str!("rules.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    // 顺序即排序：错误在前
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleFile {
    pub version: u32,
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleDef {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    /// 正则；任意一条命中即视为命中（`all` 为 true 时要求全部命中）
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub all: bool,
    /// 只按退出码判断（和 patterns 同时存在时两者都要满足）
    #[serde(default)]
    pub exit_codes: Vec<i32>,
    /// 说明，可用 `{捕获组名}` 引用正则里的命名捕获
    pub description: String,
    #[serde(default)]
    pub solutions: Vec<String>,
    /// 越大越靠前
    #[serde(default)]
    pub priority: i32,
    /// 兜底规则：已经有更具体的结论时不显示
    #[serde(default)]
    pub generic: bool,
    /// 本规则命中时，这些规则的结论不再显示（更具体的解释取代笼统的）
    #[serde(default)]
    pub supersedes: Vec<String>,
}

struct CompiledRule {
    def: RuleDef,
    regexes: Vec<Regex>,
}

pub struct RuleSet {
    pub version: u32,
    rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub description: String,
    pub solutions: Vec<String>,
    /// 命中的那一行日志
    pub evidence: Option<String>,
    /// 来自兜底规则
    #[serde(skip)]
    pub generic: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashAnalysis {
    pub exit_code: Option<i32>,
    pub findings: Vec<Finding>,
    /// 参与分析的文件（崩溃报告、hs_err 等）
    pub files: Vec<String>,
}

impl RuleSet {
    /// 解析规则 JSON。单条正则写错会报出是哪条规则，避免一条坏规则拖垮整个库。
    pub fn parse(json: &str) -> Result<Self> {
        let file: RuleFile = serde_json::from_str(json)?;
        Self::from_file(file)
    }

    fn from_file(file: RuleFile) -> Result<Self> {
        let mut rules = Vec::with_capacity(file.rules.len());
        for def in file.rules {
            let regexes = def
                .patterns
                .iter()
                .map(|p| Regex::new(p).map_err(|e| Error::Other(format!("规则 {} 的正则有误：{e}", def.id))))
                .collect::<Result<Vec<_>>>()?;
            if regexes.is_empty() && def.exit_codes.is_empty() {
                return Err(Error::Other(format!("规则 {} 既没有 patterns 也没有 exitCodes", def.id)));
            }
            rules.push(CompiledRule { def, regexes });
        }
        Ok(Self { version: file.version, rules })
    }

    /// 内置规则 + 用户覆盖（按 id 替换或追加）
    pub fn with_override(builtin: &str, custom: Option<&str>) -> Result<Self> {
        let mut base: RuleFile = serde_json::from_str(builtin)?;
        if let Some(custom) = custom {
            let extra: RuleFile = serde_json::from_str(custom)?;
            base.version = base.version.max(extra.version);
            for rule in extra.rules {
                match base.rules.iter_mut().find(|r| r.id == rule.id) {
                    Some(existing) => *existing = rule,
                    None => base.rules.push(rule),
                }
            }
        }
        Self::from_file(base)
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn analyze(&self, text: &str, exit_code: Option<i32>) -> Vec<Finding> {
        let mut hits: Vec<(&CompiledRule, Finding)> = Vec::new();
        for rule in &self.rules {
            if !rule.def.exit_codes.is_empty() && !exit_code.is_some_and(|c| rule.def.exit_codes.contains(&c)) {
                continue;
            }
            let mut vars: HashMap<String, String> = HashMap::new();
            let mut evidence = None;
            let matched = if rule.regexes.is_empty() {
                true
            } else {
                let mut any = false;
                let mut all = true;
                for re in &rule.regexes {
                    match re.captures(text) {
                        Some(caps) => {
                            any = true;
                            for name in re.capture_names().flatten() {
                                if let Some(m) = caps.name(name) {
                                    vars.entry(name.to_string()).or_insert_with(|| m.as_str().trim().to_string());
                                }
                            }
                            if evidence.is_none() {
                                let m = caps.get(0).expect("整体匹配");
                                evidence = Some(line_around(text, m.start()));
                            }
                        }
                        None => all = false,
                    }
                }
                if rule.def.all { all } else { any }
            };
            if !matched {
                continue;
            }
            if let Some(code) = exit_code {
                vars.insert("exit".into(), code.to_string());
            }
            derive_vars(&mut vars);
            let fill = |s: &str| fill_vars(s, &vars);
            hits.push((
                rule,
                Finding {
                    id: rule.def.id.clone(),
                    title: fill(&rule.def.title),
                    severity: rule.def.severity,
                    description: fill(&rule.def.description),
                    solutions: rule.def.solutions.iter().map(|s| fill(s)).collect(),
                    evidence,
                    generic: rule.def.generic,
                },
            ));
        }

        let has_specific = hits.iter().any(|(r, _)| !r.def.generic);
        let superseded: HashSet<&str> = hits.iter().flat_map(|(r, _)| r.def.supersedes.iter().map(String::as_str)).collect();
        let mut out: Vec<(&CompiledRule, Finding)> = hits
            .iter()
            .filter(|(r, _)| !(has_specific && r.def.generic) && !superseded.contains(r.def.id.as_str()))
            .map(|(r, f)| (*r, f.clone()))
            .collect();
        out.sort_by(|(ra, a), (rb, b)| a.severity.cmp(&b.severity).then(rb.def.priority.cmp(&ra.def.priority)));
        let mut seen = HashSet::new();
        out.into_iter().map(|(_, f)| f).filter(|f| seen.insert(f.id.clone())).collect()
    }
}

/// 一些需要计算的变量：class 文件版本 → Java 版本
fn derive_vars(vars: &mut HashMap<String, String>) {
    if let Some(v) = vars.get("classver").and_then(|v| v.parse::<u32>().ok()) {
        vars.insert("java".into(), v.saturating_sub(44).to_string());
    }
}

/// 替换 `{name}`；没捕获到的占位符直接去掉，顺手清理因此变空的括号
fn fill_vars(s: &str, vars: &HashMap<String, String>) -> String {
    static PLACEHOLDER: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = PLACEHOLDER.get_or_init(|| Regex::new(r"\{([A-Za-z_][A-Za-z0-9_]*)\}").expect("固定正则"));
    let out = re.replace_all(s, |c: &regex::Captures| vars.get(&c[1]).cloned().unwrap_or_default());
    out.replace("（）", "").replace("()", "").replace("「」", "")
}

/// 命中位置所在的那一行（截断到 300 字）
fn line_around(text: &str, pos: usize) -> String {
    let start = text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[pos..].find('\n').map(|i| pos + i).unwrap_or(text.len());
    let line = text[start..end].trim();
    match line.char_indices().nth(300) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// 只保留末尾 `max` 字节（按字符边界），日志太长时开头基本是无关的加载信息
pub fn tail(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// 游戏目录里在 `since` 之后生成的崩溃报告和 JVM 错误日志
pub fn collect_crash_files(game_dir: &Path, since: SystemTime) -> Vec<PathBuf> {
    let newer = |p: &Path| p.metadata().and_then(|m| m.modified()).is_ok_and(|t| t >= since);
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(game_dir.join("crash-reports")) {
        let mut reports: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| newer(p)).collect();
        reports.sort();
        files.extend(reports.pop());
    }
    if let Ok(entries) = std::fs::read_dir(game_dir) {
        files.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("hs_err_pid") && n.ends_with(".log")))
                .filter(|p| newer(p)),
        );
    }
    files
}

/// 游戏以退出码 0 结束且没有崩溃报告时（比如 Forge 的模组加载错误界面被玩家关掉），
/// 只保留明确的错误结论，避免把正常日志里的噪音当成问题
pub fn keep_for_clean_exit(analysis: &mut CrashAnalysis) {
    analysis.findings.retain(|f| f.severity == Severity::Error && !f.generic);
}

/// 把日志行和崩溃文件拼起来分析
pub fn analyze_run(rules: &RuleSet, log: &str, files: &[PathBuf], exit_code: Option<i32>) -> CrashAnalysis {
    let mut text = tail(log, 2 * 1024 * 1024).to_string();
    for f in files {
        if let Ok(bytes) = std::fs::read(f) {
            text.push('\n');
            text.push_str(tail(&String::from_utf8_lossy(&bytes), 512 * 1024));
        }
    }
    CrashAnalysis {
        exit_code,
        findings: rules.analyze(&text, exit_code),
        files: files.iter().map(|p| p.display().to_string()).collect(),
    }
}

#[cfg(test)]
mod tests;
