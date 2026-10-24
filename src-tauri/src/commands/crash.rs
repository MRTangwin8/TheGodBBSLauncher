use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use super::AppState;
use crate::engine::crash::{self, CrashAnalysis, RuleSet, BUILTIN_RULES};
use crate::engine::{Error, Result};

pub fn custom_rules_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("crash-rules.json")
}

/// 内置规则 + 用户目录下的覆盖规则；覆盖规则坏了就只用内置的
pub fn load_rules(data_dir: &std::path::Path) -> RuleSet {
    let custom = std::fs::read_to_string(custom_rules_path(data_dir)).ok();
    RuleSet::with_override(BUILTIN_RULES, custom.as_deref())
        .or_else(|_| RuleSet::parse(BUILTIN_RULES))
        .expect("内置规则必须有效")
}

/// 分析用户粘贴或打开的日志文本
#[tauri::command]
pub async fn analyze_log_text(state: State<'_, AppState>, text: String) -> Result<CrashAnalysis> {
    let rules = state.crash_rules.read().await.clone();
    Ok(CrashAnalysis { exit_code: None, findings: rules.analyze(crash::tail(&text, 4 * 1024 * 1024), None), files: vec![] })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesInfo {
    pub version: u32,
    pub count: usize,
    pub custom: bool,
}

#[tauri::command]
pub async fn crash_rules_info(state: State<'_, AppState>) -> Result<RulesInfo> {
    let rules = state.crash_rules.read().await;
    Ok(RulesInfo { version: rules.version, count: rules.len(), custom: custom_rules_path(&state.data_dir).is_file() })
}

/// 从地址下载规则库（和内置规则同格式），校验通过后保存并立即生效
#[tauri::command]
pub async fn update_crash_rules(state: State<'_, AppState>, url: String) -> Result<RulesInfo> {
    let url = url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(Error::Other("规则库地址需要以 http(s):// 开头".into()));
    }
    let text = state.client.get(url).send().await?.error_for_status()?.text().await?;
    let rules = RuleSet::with_override(BUILTIN_RULES, Some(&text)).map_err(|e| Error::Other(format!("规则库格式不对：{e}")))?;
    let path = custom_rules_path(&state.data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &text)?;
    let info = RulesInfo { version: rules.version, count: rules.len(), custom: true };
    *state.crash_rules.write().await = Arc::new(rules);
    Ok(info)
}

/// 删除自定义规则，恢复内置规则库
#[tauri::command]
pub async fn reset_crash_rules(state: State<'_, AppState>) -> Result<RulesInfo> {
    let _ = std::fs::remove_file(custom_rules_path(&state.data_dir));
    let rules = load_rules(&state.data_dir);
    let info = RulesInfo { version: rules.version, count: rules.len(), custom: false };
    *state.crash_rules.write().await = Arc::new(rules);
    Ok(info)
}
