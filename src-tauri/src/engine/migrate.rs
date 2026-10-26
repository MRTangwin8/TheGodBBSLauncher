//! 迁移游戏目录：把旧目录里的东西搬到新位置。
//!
//! 原则：**新位置确认没问题之前，旧数据一个字节都不动**。
//! - 同一个盘：直接改名，几 GB 也是瞬间完成；中途失败就改回去。
//! - 跨盘：先复制，再核对文件数和总大小，写入新设置（`commit`）之后才删旧的；复制时可以取消，取消或失败都会清掉复制出来的部分。
//! - 旧目录里个别文件删不掉（被占用 / 杀毒软件）不算失败，列在报告里让用户自己处理。

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use super::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Scanning,
    Moving,
    Copying,
    Verifying,
    Cleaning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: Stage,
    pub bytes: u64,
    pub total_bytes: u64,
    pub files: u64,
    pub total_files: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// false = 只改了位置，没搬文件
    pub moved: bool,
    /// true = 同盘改名（瞬间完成），false = 复制
    pub renamed: bool,
    pub bytes: u64,
    pub files: u64,
    /// 已经复制成功，但旧目录里没能删掉的东西
    pub leftover: Vec<String>,
}

fn comps(p: &Path) -> Vec<String> {
    p.components()
        .map(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            if cfg!(windows) {
                s.to_lowercase()
            } else {
                s
            }
        })
        .collect()
}

/// child 是不是 parent 本身或者在 parent 里面（Windows 上不分大小写）
pub fn is_inside(child: &Path, parent: &Path) -> bool {
    let (c, p) = (comps(child), comps(parent));
    c.len() >= p.len() && c[..p.len()] == p[..]
}

fn check(from: &Path, to: &Path) -> Result<()> {
    if !from.is_absolute() || !to.is_absolute() {
        return Err(Error::Other("游戏目录必须是完整路径".into()));
    }
    if to.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::Other("新位置里不能有 .. 这样的路径".into()));
    }
    if !from.is_dir() {
        return Err(Error::Other(format!("旧的游戏目录不存在：{}", from.display())));
    }
    if is_inside(to, from) {
        return Err(Error::Other("新位置不能是旧游戏目录本身，也不能在它里面".into()));
    }
    if is_inside(from, to) {
        return Err(Error::Other("旧的游戏目录不能在新位置里面，请换一个不包含它的位置".into()));
    }
    if to.exists() {
        let empty = to.is_dir() && fs::read_dir(to).map(|mut d| d.next().is_none()).unwrap_or(false);
        if !empty {
            return Err(Error::Other(format!("新位置不是空文件夹：{}。请选一个空文件夹，或者选「只改位置，不搬文件」", to.display())));
        }
    }
    Ok(())
}

fn io_err(what: &str, path: &Path, e: io::Error) -> Error {
    Error::Other(format!("{what}：{}（{e}）", path.display()))
}

/// 统计目录里有多少文件、多少字节。遇到符号链接直接报错：复制它们要么跟出目录、要么留下悬空链接，都不安全
fn scan(dir: &Path) -> Result<(u64, u64)> {
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).map_err(|e| io_err("读不了文件夹", &d, e))? {
            let entry = entry.map_err(|e| io_err("读不了文件夹", &d, e))?;
            let ty = entry.file_type().map_err(|e| io_err("读不了文件信息", &entry.path(), e))?;
            if ty.is_symlink() {
                return Err(Error::Other(format!("里面有符号链接 / 快捷连接，不能自动迁移：{}", entry.path().display())));
            }
            if ty.is_dir() {
                stack.push(entry.path());
            } else {
                files += 1;
                bytes += entry.metadata().map_err(|e| io_err("读不了文件信息", &entry.path(), e))?.len();
            }
        }
    }
    Ok((files, bytes))
}

fn is_cross_device(e: &io::Error) -> bool {
    // Windows: ERROR_NOT_SAME_DEVICE(17)；Unix: EXDEV(18)
    matches!(e.raw_os_error(), Some(17) if cfg!(windows)) || matches!(e.raw_os_error(), Some(18) if cfg!(unix))
}

fn top_level(dir: &Path) -> Result<Vec<PathBuf>> {
    fs::read_dir(dir)
        .map_err(|e| io_err("读不了文件夹", dir, e))?
        .map(|e| e.map(|e| e.path()).map_err(|e| io_err("读不了文件夹", dir, e)))
        .collect()
}

/// 把 `from` 里的东西搬到 `to`。
/// - `force_copy`：不尝试改名，直接复制（测试用，也可以用来强制走最保守的路径）
/// - `on_progress`：返回 false 表示用户取消（只在复制阶段生效）
/// - `commit`：新位置准备好之后、删旧数据之前调用，用来写入新的设置；失败会撤销整个迁移
pub fn migrate(from: &Path, to: &Path, force_copy: bool, on_progress: &dyn Fn(&Progress) -> bool, commit: &dyn Fn() -> Result<()>) -> Result<Report> {
    check(from, to)?;
    let created_to = !to.exists();
    fs::create_dir_all(to).map_err(|e| io_err("没法创建新位置", to, e))?;
    // 出任何问题都把新位置恢复成迁移前的样子：我们建的就整个删掉，原来就有的空文件夹就清空
    let undo = || {
        if created_to {
            let _ = fs::remove_dir_all(to);
        } else if let Ok(entries) = top_level(to) {
            for p in entries {
                let _ = if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
            }
        }
    };

    if !force_copy {
        match rename_all(from, to, on_progress, commit) {
            Ok(report) => return Ok(report),
            // 不在同一个盘：改名做不到，改用复制
            Err(RenameError::CrossDevice) => {}
            Err(RenameError::Failed(e)) => {
                undo();
                return Err(e);
            }
        }
    }
    copy_all(from, to, on_progress, commit).inspect_err(|_| undo())
}

enum RenameError {
    CrossDevice,
    Failed(Error),
}

fn rename_all(from: &Path, to: &Path, on_progress: &dyn Fn(&Progress) -> bool, commit: &dyn Fn() -> Result<()>) -> std::result::Result<Report, RenameError> {
    let entries = top_level(from).map_err(RenameError::Failed)?;
    let total = entries.len() as u64;
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let rollback = |done: &[(PathBuf, PathBuf)]| {
        for (src, dst) in done.iter().rev() {
            let _ = fs::rename(dst, src);
        }
    };
    for (i, src) in entries.iter().enumerate() {
        let dst = to.join(src.file_name().unwrap_or_default());
        if let Err(e) = fs::rename(src, &dst) {
            if done.is_empty() && is_cross_device(&e) {
                return Err(RenameError::CrossDevice);
            }
            rollback(&done);
            return Err(RenameError::Failed(io_err("移动失败（文件可能被占用，关掉游戏和杀毒软件的扫描再试）", src, e)));
        }
        done.push((src.clone(), dst));
        on_progress(&Progress { stage: Stage::Moving, bytes: 0, total_bytes: 0, files: i as u64 + 1, total_files: total });
    }
    if let Err(e) = commit() {
        rollback(&done);
        return Err(RenameError::Failed(e));
    }
    let _ = fs::remove_dir(from); // 旧目录现在是空的，能删就删
    Ok(Report { moved: true, renamed: true, bytes: 0, files: total, leftover: Vec::new() })
}

fn copy_all(from: &Path, to: &Path, on_progress: &dyn Fn(&Progress) -> bool, commit: &dyn Fn() -> Result<()>) -> Result<Report> {
    let mut p = Progress { stage: Stage::Scanning, bytes: 0, total_bytes: 0, files: 0, total_files: 0 };
    on_progress(&p);
    let (total_files, total_bytes) = scan(from)?;
    p.total_files = total_files;
    p.total_bytes = total_bytes;
    p.stage = Stage::Copying;

    let cancelled = || Error::Other("已取消迁移，旧目录没有任何改动".into());
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src_dir, dst_dir)) = stack.pop() {
        for entry in fs::read_dir(&src_dir).map_err(|e| io_err("读不了文件夹", &src_dir, e))? {
            let entry = entry.map_err(|e| io_err("读不了文件夹", &src_dir, e))?;
            let src = entry.path();
            let dst = dst_dir.join(entry.file_name());
            if entry.file_type().map_err(|e| io_err("读不了文件信息", &src, e))?.is_dir() {
                fs::create_dir_all(&dst).map_err(|e| io_err("没法创建文件夹", &dst, e))?;
                stack.push((src, dst));
                continue;
            }
            let copied = fs::copy(&src, &dst).map_err(|e| io_err("复制失败", &src, e))?;
            let expected = entry.metadata().map_err(|e| io_err("读不了文件信息", &src, e))?.len();
            if copied != expected {
                return Err(Error::Other(format!("复制出来的文件大小不对：{}", src.display())));
            }
            p.files += 1;
            p.bytes += copied;
            if !on_progress(&p) {
                return Err(cancelled());
            }
        }
    }

    p.stage = Stage::Verifying;
    on_progress(&p);
    let (new_files, new_bytes) = scan(to)?;
    if (new_files, new_bytes) != (total_files, total_bytes) {
        return Err(Error::Other(format!("复制完核对对不上（应有 {total_files} 个文件 / {total_bytes} 字节，实际 {new_files} 个 / {new_bytes} 字节），已撤销，旧目录没有改动")));
    }
    commit()?;

    // 到这里新位置已经生效，删旧的；删不掉的列出来
    p.stage = Stage::Cleaning;
    on_progress(&p);
    let mut leftover = Vec::new();
    for path in top_level(from)? {
        let removed = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
        if removed.is_err() {
            leftover.push(path.display().to_string());
        }
    }
    let _ = fs::remove_dir(from);
    Ok(Report { moved: true, renamed: false, bytes: total_bytes, files: total_files, leftover })
}

/// 目录的大小和文件数，给界面显示用；读不了的地方按 0 算，不报错
pub fn stats(dir: &Path) -> (u64, u64) {
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else { continue };
        for entry in read.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t) if t.is_file() => {
                    files += 1;
                    bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
                _ => {}
            }
        }
    }
    (files, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn sample(root: &Path) {
        fs::create_dir_all(root.join("versions/1.21.1/saves/世界")).unwrap();
        fs::create_dir_all(root.join("assets/objects/ab")).unwrap();
        fs::write(root.join("versions/1.21.1/1.21.1.json"), b"{}").unwrap();
        fs::write(root.join("versions/1.21.1/saves/世界/level.dat"), vec![7u8; 4096]).unwrap();
        fs::write(root.join("assets/objects/ab/abcdef"), vec![1u8; 10_000]).unwrap();
        fs::write(root.join("launcher_profiles.json"), b"{\"a\":1}").unwrap();
    }

    fn quiet(_: &Progress) -> bool {
        true
    }
    fn ok() -> Result<()> {
        Ok(())
    }

    #[test]
    fn same_volume_is_a_rename() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("old/.minecraft"), t.path().join("new/.minecraft"));
        sample(&from);
        let committed = Cell::new(false);
        let r = migrate(&from, &to, false, &quiet, &|| {
            committed.set(true);
            Ok(())
        })
        .unwrap();
        assert!(r.renamed && committed.get());
        assert_eq!(fs::read(to.join("versions/1.21.1/saves/世界/level.dat")).unwrap().len(), 4096);
        assert!(to.join("assets/objects/ab/abcdef").is_file() && to.join("launcher_profiles.json").is_file());
        assert!(!from.exists(), "改名完旧目录是空的，应该被删掉");
    }

    #[test]
    fn copy_mode_copies_verifies_and_removes_the_source() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("a"), t.path().join("b"));
        sample(&from);
        let r = migrate(&from, &to, true, &quiet, &ok).unwrap();
        assert!(!r.renamed && r.leftover.is_empty());
        assert_eq!((r.files, r.bytes), (4, 2 + 4096 + 10_000 + 7));
        assert_eq!(fs::read(to.join("assets/objects/ab/abcdef")).unwrap(), vec![1u8; 10_000]);
        assert!(!from.exists());
        assert_eq!(stats(&to), (4, 14_105));
    }

    #[test]
    fn cancelling_leaves_the_source_untouched_and_cleans_the_copy() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("a"), t.path().join("b"));
        sample(&from);
        let seen = Cell::new(0);
        let cancel_after_first = |p: &Progress| {
            if p.stage == Stage::Copying {
                seen.set(seen.get() + 1);
            }
            seen.get() < 1
        };
        let err = migrate(&from, &to, true, &cancel_after_first, &ok).unwrap_err().to_string();
        assert!(err.contains("已取消"), "{err}");
        assert!(!to.exists(), "复制出来的部分要清掉");
        assert_eq!(stats(&from), (4, 14_105), "旧目录一个字节都不能少");
    }

    #[test]
    fn a_failing_commit_undoes_both_modes() {
        for force_copy in [false, true] {
            let t = tempfile::tempdir().unwrap();
            let (from, to) = (t.path().join("a"), t.path().join("b"));
            sample(&from);
            let err = migrate(&from, &to, force_copy, &quiet, &|| Err(Error::Other("写设置失败".into()))).unwrap_err().to_string();
            assert!(err.contains("写设置失败"));
            assert!(!to.exists(), "force_copy={force_copy}");
            assert_eq!(stats(&from), (4, 14_105), "force_copy={force_copy}：旧数据必须原样在");
        }
    }

    #[test]
    fn existing_empty_target_is_reused_and_restored_on_failure() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("a"), t.path().join("b"));
        sample(&from);
        fs::create_dir_all(&to).unwrap();
        assert!(migrate(&from, &to, true, &quiet, &|| Err(Error::Other("x".into()))).is_err());
        assert!(to.is_dir() && fs::read_dir(&to).unwrap().next().is_none(), "原来就有的空文件夹要保留下来");
        assert!(migrate(&from, &to, true, &quiet, &ok).is_ok());
    }

    #[test]
    fn unsafe_targets_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        let from = t.path().join("mc");
        sample(&from);
        let msg = |to: &Path| migrate(&from, to, false, &quiet, &ok).unwrap_err().to_string();
        assert!(msg(&from).contains("不能是旧游戏目录本身"));
        assert!(msg(&from.join("inner")).contains("不能是旧游戏目录本身"), "不能搬进自己里面");
        assert!(msg(t.path()).contains("不能在新位置里面"), "不能搬到自己的上一级");
        assert!(msg(&t.path().join("a/../b")).contains(".."));
        assert!(msg(Path::new("relative/path")).contains("完整路径"));
        let full = t.path().join("full");
        fs::create_dir_all(&full).unwrap();
        fs::write(full.join("别的启动器的文件"), b"x").unwrap();
        assert!(msg(&full).contains("不是空文件夹"));
        assert_eq!(stats(&from).0, 4, "校验不通过时旧目录原封不动");
        assert!(migrate(&t.path().join("没有这个"), &t.path().join("n"), false, &quiet, &ok).is_err());
    }

    #[test]
    fn empty_source_is_fine() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("a"), t.path().join("b"));
        fs::create_dir_all(&from).unwrap();
        assert!(migrate(&from, &to, true, &quiet, &ok).is_ok());
        assert!(to.is_dir());
    }

    #[test]
    fn inside_check_ignores_case_on_windows_only() {
        assert!(is_inside(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(is_inside(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!is_inside(Path::new("/a/bc"), Path::new("/a/b")), "前缀相同但不是子目录");
        assert_eq!(is_inside(Path::new("D:/Games/MC"), Path::new("d:/games")), cfg!(windows));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_before_anything_is_copied() {
        let t = tempfile::tempdir().unwrap();
        let (from, to) = (t.path().join("a"), t.path().join("b"));
        sample(&from);
        std::os::unix::fs::symlink("/etc", from.join("link")).unwrap();
        let err = migrate(&from, &to, true, &quiet, &ok).unwrap_err().to_string();
        assert!(err.contains("符号链接"), "{err}");
        assert!(!to.exists());
    }
}
