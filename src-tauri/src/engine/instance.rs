//! 单个版本（实例）自己的设置：versions/<id>/godbbs.json。
//! 整合包导入后需要强制版本隔离，并带上整合包要求的内存和参数；没有这个文件时全部跟随全局设置。

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::install::GameDir;
use crate::engine::Result;

pub const FILE_NAME: &str = "godbbs.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceConfig {
    /// 强制版本隔离（整合包必须）
    pub isolated: bool,
    /// 最少内存 MB：启动时取它和全局设置里较大的那个
    pub min_memory_mb: Option<u32>,
    /// 额外 JVM 参数，追加在全局参数之后
    pub jvm_args: Vec<String>,
    /// 额外游戏参数
    pub game_args: Vec<String>,
    /// 来自哪个整合包
    pub modpack: Option<ModpackOrigin>,
    /// 累计游戏时长（秒）
    pub play_time_secs: u64,
    /// 上次启动时间（unix 秒）
    pub last_played: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackOrigin {
    pub format: String,
    pub name: String,
    pub version: String,
}

pub fn load(game: &GameDir, id: &str) -> InstanceConfig {
    load_from(&game.versions().join(id))
}

fn load_from(dir: &Path) -> InstanceConfig {
    std::fs::read_to_string(dir.join(FILE_NAME))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(game: &GameDir, id: &str, config: &InstanceConfig) -> Result<()> {
    let dir = game.versions().join(id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(FILE_NAME), serde_json::to_string_pretty(config)?)?;
    Ok(())
}

/// 记一次游戏会话：累加时长、更新最近游玩时间
pub fn record_session(game: &GameDir, id: &str, started_at: u64, secs: u64) -> Result<()> {
    if !game.versions().join(id).is_dir() {
        return Ok(());
    }
    let mut cfg = load(game, id);
    cfg.play_time_secs += secs;
    cfg.last_played = Some(started_at);
    save(game, id, &cfg)
}

/// 这个版本运行时用的游戏目录
pub fn game_dir(game: &GameDir, id: &str, global_isolate: bool) -> std::path::PathBuf {
    if global_isolate || load(game, id).isolated {
        game.versions().join(id)
    } else {
        game.root.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_defaults_and_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let game = GameDir::new(dir.path());
        assert_eq!(load(&game, "x"), InstanceConfig::default());
        assert_eq!(game_dir(&game, "x", false), game.root);

        let cfg = InstanceConfig { isolated: true, min_memory_mb: Some(6144), jvm_args: vec!["-XX:+UseZGC".into()], ..Default::default() };
        save(&game, "x", &cfg).unwrap();
        assert_eq!(load(&game, "x"), cfg);
        assert_eq!(game_dir(&game, "x", false), game.versions().join("x"), "整合包实例即使全局关了隔离也要隔离");
    }

    #[test]
    fn records_play_time() {
        let dir = tempfile::tempdir().unwrap();
        let game = GameDir::new(dir.path());
        std::fs::create_dir_all(game.versions().join("v")).unwrap();
        record_session(&game, "v", 100, 60).unwrap();
        record_session(&game, "v", 200, 30).unwrap();
        let cfg = load(&game, "v");
        assert_eq!((cfg.play_time_secs, cfg.last_played), (90, Some(200)));
        record_session(&game, "gone", 1, 1).unwrap();
        assert!(!game.versions().join("gone").exists(), "版本被删了就不记");
    }
}
