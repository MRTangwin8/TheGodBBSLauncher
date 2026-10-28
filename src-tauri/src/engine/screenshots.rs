//! 截图浏览：游戏目录下 screenshots 文件夹里的图片（F2 截的图）。
//! 文件名只认单个文件名 + 图片后缀，读、删都先过 `path`，不会碰到文件夹外面的东西。

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use super::{Error, Result};

pub const DIR: &str = "screenshots";
const EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];
/// 单张图最大 64 MB，再大不像是游戏截图，也不往界面里送
const MAX_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub size: u64,
    /// 修改时间，Unix 秒
    pub modified: u64,
}

fn is_image(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// 新的在前
pub fn list(game_dir: &Path) -> Vec<Screenshot> {
    let Ok(entries) = std::fs::read_dir(game_dir.join(DIR)) else { return vec![] };
    let mut out: Vec<Screenshot> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let meta = e.metadata().ok()?;
            (meta.is_file() && is_image(&name) && meta.len() <= MAX_SIZE).then(|| Screenshot {
                modified: meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0),
                size: meta.len(),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.name.cmp(&a.name)));
    out
}

/// 校验文件名并返回完整路径：只能是 screenshots 里直接放着的图片
pub fn path(game_dir: &Path, name: &str) -> Result<PathBuf> {
    let plain = !name.is_empty() && !name.contains(['/', '\\', ':']) && name != "." && name != "..";
    if !plain || !is_image(name) {
        return Err(Error::Other(format!("不是截图文件：{name}")));
    }
    let p = game_dir.join(DIR).join(name);
    let meta = std::fs::symlink_metadata(&p).map_err(|_| Error::Other(format!("截图不存在：{name}")))?;
    if !meta.is_file() || meta.len() > MAX_SIZE {
        return Err(Error::Other(format!("截图不存在：{name}")));
    }
    Ok(p)
}

pub fn delete(game_dir: &Path, name: &str) -> Result<()> {
    std::fs::remove_file(path(game_dir, name)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_images_newest_first_and_guards_names() {
        let dir = tempfile::tempdir().unwrap();
        let shots = dir.path().join(DIR);
        std::fs::create_dir_all(shots.join("sub.png")).unwrap();
        std::fs::write(shots.join("2026-10-01_10.00.00.png"), b"a").unwrap();
        std::fs::write(shots.join("b.JPG"), b"bb").unwrap();
        std::fs::write(shots.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("outside.png"), b"x").unwrap();
        let names: Vec<String> = list(dir.path()).into_iter().map(|s| s.name).collect();
        assert_eq!(names.len(), 2, "文件夹和非图片都不算：{names:?}");
        assert!(names.contains(&"b.JPG".to_string()));

        assert!(path(dir.path(), "b.JPG").is_ok());
        for bad in ["../outside.png", "..\\outside.png", "notes.txt", "sub.png", "missing.png", "C:evil.png", ""] {
            assert!(path(dir.path(), bad).is_err(), "{bad:?} 应该被拒绝");
        }
        delete(dir.path(), "b.JPG").unwrap();
        assert!(!shots.join("b.JPG").exists());
        assert!(list(&dir.path().join("nothing")).is_empty());
    }
}
