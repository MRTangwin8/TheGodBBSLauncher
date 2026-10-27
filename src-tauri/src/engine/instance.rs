//! 单个版本（实例）自己的设置：versions/<id>/godbbs.json。
//! 整合包导入后需要强制版本隔离，并带上整合包要求的内存和参数；没有这个文件时全部跟随全局设置。
//! 「版本管理 → 管理」页面也在这里改：单独的 Java、内存、参数。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::install::GameDir;
use crate::engine::Result;

pub const FILE_NAME: &str = "godbbs.json";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceConfig {
    /// 强制版本隔离（整合包必须）
    pub isolated: bool,
    /// 这个版本专用的 Java；空着就跟随全局设置
    pub java_path: Option<PathBuf>,
    /// 这个版本专用的内存 MB；空着就跟随全局设置
    pub memory_mb: Option<u32>,
    /// 最少内存 MB（整合包要求）：启动时取它和实际内存里较大的那个
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

/// 用户在管理页里能改的那部分
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceSettings {
    pub isolated: bool,
    pub java_path: Option<PathBuf>,
    pub memory_mb: Option<u32>,
    /// 一行文本，和全局设置里的写法一样（空格分隔，带空格的参数用引号括起来）
    pub jvm_args: String,
    pub game_args: String,
}

pub const MEMORY_RANGE: std::ops::RangeInclusive<u32> = 512..=262_144;

impl InstanceConfig {
    pub fn settings(&self) -> InstanceSettings {
        InstanceSettings {
            isolated: self.isolated,
            java_path: self.java_path.clone(),
            memory_mb: self.memory_mb,
            jvm_args: join_args(&self.jvm_args),
            game_args: join_args(&self.game_args),
        }
    }

    /// 应用管理页的修改。整合包实例不能关掉隔离（它的模组、配置都在隔离目录里）
    pub fn apply(&mut self, s: InstanceSettings) -> Result<()> {
        if let Some(mb) = s.memory_mb {
            if !MEMORY_RANGE.contains(&mb) {
                return Err(crate::engine::Error::Other(format!("内存要在 {} ~ {} MB 之间", MEMORY_RANGE.start(), MEMORY_RANGE.end())));
            }
        }
        let java_path = s.java_path.filter(|p| !p.as_os_str().is_empty());
        if let Some(p) = &java_path {
            if !p.is_file() {
                return Err(crate::engine::Error::Other(format!("找不到这个 Java：{}", p.display())));
            }
        }
        self.isolated = s.isolated || self.modpack.is_some();
        self.java_path = java_path;
        self.memory_mb = s.memory_mb;
        self.jvm_args = crate::config::split_args(&s.jvm_args);
        self.game_args = crate::config::split_args(&s.game_args);
        Ok(())
    }
}

/// 参数列表 → 一行文本（split_args 的逆操作）
fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() || a.contains(char::is_whitespace) || a.contains(['"', '\'']) {
                let q = if a.contains('"') { '\'' } else { '"' };
                format!("{q}{a}{q}")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    fn settings_roundtrip_and_validation() {
        let mut cfg = InstanceConfig { jvm_args: vec!["-Dfoo=a b".into(), "-Xss2m".into()], play_time_secs: 99, ..Default::default() };
        let s = cfg.settings();
        assert_eq!(s.jvm_args, "\"-Dfoo=a b\" -Xss2m");
        assert_eq!(crate::config::split_args(&s.jvm_args), cfg.jvm_args, "显示的文本再解析回来要一样");

        cfg.apply(InstanceSettings { memory_mb: Some(4096), game_args: "--demo".into(), ..s.clone() }).unwrap();
        assert_eq!((cfg.memory_mb, cfg.game_args.clone(), cfg.play_time_secs), (Some(4096), vec!["--demo".to_string()], 99));

        assert!(cfg.apply(InstanceSettings { memory_mb: Some(100), ..Default::default() }).is_err());
        assert!(cfg.apply(InstanceSettings { java_path: Some("/没有/这个/java".into()), ..Default::default() }).is_err());
        cfg.apply(InstanceSettings { java_path: Some(PathBuf::new()), ..Default::default() }).unwrap();
        assert_eq!(cfg.java_path, None, "空路径当作跟随全局");

        let mut pack = InstanceConfig { isolated: true, modpack: Some(ModpackOrigin { format: "modrinth".into(), name: "p".into(), version: "1".into() }), ..Default::default() };
        pack.apply(InstanceSettings::default()).unwrap();
        assert!(pack.isolated, "整合包实例关不掉隔离");
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
