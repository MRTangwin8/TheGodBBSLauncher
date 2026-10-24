//! 全局设置：JSON 文件，放在系统应用配置目录。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::mirror::SourcePref;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// .minecraft 根目录（libraries / assets 在所有实例间共享）
    pub game_dir: PathBuf,
    pub download_source: SourcePref,
    pub concurrency: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            game_dir: default_game_dir(),
            download_source: SourcePref::Auto,
            concurrency: 64,
        }
    }
}

pub fn default_game_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GodBBSLauncher")
        .join(".minecraft")
}

/// 读不到或解析失败都回退默认值，不让坏配置挡住启动
pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(settings)?)
}
