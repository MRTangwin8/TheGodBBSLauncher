//! 外部来源（整合包、版本 json、资源平台接口）给出的路径和文件名的统一检查。
//!
//! 除了 `..` 和绝对路径，还要防住 Windows 上的坑（整合包是在别的系统上做的也一样要防）：
//! - 设备名 `CON`、`NUL`、`COM1` 等（带不带扩展名都算）：往里写会写到设备上，甚至卡住；
//! - 盘符和 `:`（`C:x` 会跳到别的盘，`a:b` 是 NTFS 备用数据流）、`\` 分隔符；
//! - 结尾的 `.` 和空格（Windows 会悄悄去掉，`a.` 和 `a` 变成同一个文件）、`<>"|?*`、控制字符。
//!
//! 符号链接 / junction：解压时从不按压缩包里的条目创建链接（链接条目直接跳过），
//! 往新建的目录里写文件前再用 `no_links` 确认路径上没有链接，免得被预先放好的链接带到别处。

use std::path::{Path, PathBuf};

use crate::engine::{Error, Result};

const RESERVED: &[&str] = &["con", "prn", "aux", "nul"];

fn reserved(seg: &str) -> bool {
    let stem = seg.split('.').next().unwrap_or("").trim_end().to_ascii_lowercase();
    if RESERVED.contains(&stem.as_str()) {
        return true;
    }
    // COM0~9、LPT0~9，还有上标数字 ¹²³ 的写法
    match (stem.get(..3), stem.get(3..)) {
        (Some("com" | "lpt"), Some(d)) => matches!(d, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"),
        _ => false,
    }
}

/// 单个文件名 / 文件夹名能不能用
pub fn name_ok(seg: &str) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && seg.len() <= 255
        && !seg.ends_with(['.', ' '])
        && !seg.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !reserved(seg)
}

/// 相对路径检查并规范成 `a/b/c`（接受 `\`），不合格返回 None
pub fn rel(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    if p.starts_with('/') {
        return None;
    }
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    (!parts.is_empty() && parts.iter().all(|s| name_ok(s))).then(|| parts.join("/"))
}

pub fn join(base: &Path, path: &str) -> Option<PathBuf> {
    rel(path).map(|r| r.split('/').fold(base.to_path_buf(), |p, s| p.join(s)))
}

/// 必须拼出一个路径的地方用（库文件路径等）：去掉 `..`、`.` 和空段，把有问题的段改成安全的样子，
/// 保证结果一定在 `base` 里面、也不会写到设备上。正常的路径原样不变
pub fn join_lossy(base: &Path, path: &str) -> PathBuf {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(|s| {
            if name_ok(s) {
                return s.to_string();
            }
            let mut t: String = s.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
            while t.ends_with(['.', ' ']) {
                t.pop();
            }
            if t.is_empty() || reserved(&t) {
                t.insert(0, '_');
            }
            t
        })
        .fold(base.to_path_buf(), |p, s| p.join(s))
}

/// `target` 以及它在 `base` 下面的每一级上级，只要已经存在就不能是符号链接或 junction
pub fn no_links(base: &Path, target: &Path) -> Result<()> {
    let Ok(rel) = target.strip_prefix(base) else {
        return Err(Error::Other(format!("路径不在目标目录里：{}", target.display())));
    };
    let mut cur = base.to_path_buf();
    for c in rel.components() {
        cur.push(c);
        match std::fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::Other(format!("「{}」是链接，为了安全不往里面写文件", cur.display())));
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_windows_traps() {
        for bad in ["CON", "con.txt", "Nul", "aux.tar.gz", "COM1", "lpt9.log", "COM¹", "a.", "a ", "a:b", "a|b", "a*b", "a\u{1}b", "..", "."] {
            assert!(!name_ok(bad), "{bad:?}");
        }
        for good in ["config", "mods", "options.txt", "COM", "COM10", "console.log", "auxiliary", "nul_mod.jar", "中文.jar", ".minecraft"] {
            assert!(name_ok(good), "{good:?}");
        }
    }

    #[test]
    fn rel_normalizes_and_rejects() {
        assert_eq!(rel("mods\\a.jar").as_deref(), Some("mods/a.jar"));
        assert_eq!(rel("./config//x").as_deref(), Some("config/x"));
        for bad in ["../a", "mods/../../a", "/etc/passwd", "C:/Windows/x", "C:x", "config/CON", "mods/a.jar:evil", "a./b", ""] {
            assert!(rel(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn join_lossy_stays_inside_base() {
        let base = Path::new("/l");
        assert_eq!(join_lossy(base, "org/a/1.0/a-1.0.jar"), base.join("org").join("a").join("1.0").join("a-1.0.jar"));
        assert_eq!(join_lossy(base, "../../etc/x"), base.join("etc").join("x"));
        assert_eq!(join_lossy(base, "a\\..\\..\\x"), base.join("a").join("x"));
        assert_eq!(join_lossy(base, "C:/Windows/x"), base.join("C_").join("Windows").join("x"));
        assert_eq!(join_lossy(base, "lib/CON/nul.jar"), base.join("lib").join("_CON").join("_nul.jar"));
        assert_eq!(join_lossy(base, "a./b "), base.join("a").join("b"));
    }

    #[test]
    fn detects_links_on_the_way() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("game");
        std::fs::create_dir_all(base.join("mods")).unwrap();
        assert!(no_links(&base, &base.join("mods").join("a.jar")).is_ok());
        assert!(no_links(&base, &base.join("new").join("deep").join("a")).is_ok(), "还不存在的路径没问题");
        assert!(no_links(&base, &dir.path().join("elsewhere")).is_err());
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&outside, base.join("link")).is_ok();
        #[cfg(windows)]
        let linked = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(base.join("link"))
            .arg(&outside)
            .output()
            .is_ok_and(|o| o.status.success());
        if linked {
            assert!(no_links(&base, &base.join("link").join("a.jar")).is_err(), "junction / 符号链接要拦住");
        }
    }
}
