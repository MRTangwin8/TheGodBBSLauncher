//! 版本第一次启动时按启动器 / 系统语言预设游戏语言（`options.txt` 的 `lang`）。
//! 只在 `options.txt` 还不存在时写，不会改掉玩家在游戏里自己选的语言。

use std::path::Path;

use crate::engine::Result;

/// 1.11 起语言代码全小写（`zh_cn`），之前是 `zh_CN`
const LOWERCASE_SINCE: &str = "2016-11-14";

/// `lang` 来自界面，按不可信数据处理：只接受 `xx_yy` 这种形状
fn valid(lang: &str) -> bool {
    let mut parts = lang.split('_');
    let ok = |p: Option<&str>| p.is_some_and(|p| (2..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_lowercase()));
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

fn for_version(lang: &str, release_time: Option<&str>) -> String {
    match release_time {
        Some(t) if t < LOWERCASE_SINCE => match lang.split_once('_') {
            Some((l, r)) => format!("{l}_{}", r.to_ascii_uppercase()),
            None => lang.to_string(),
        },
        _ => lang.to_string(),
    }
}

/// 写了返回 true
pub fn seed_language(game_dir: &Path, lang: &str, release_time: Option<&str>) -> Result<bool> {
    let file = game_dir.join("options.txt");
    if !valid(lang) || file.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(game_dir)?;
    std::fs::write(file, format!("lang:{}\n", for_version(lang, release_time)))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_only_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("versions").join("1.21");
        assert!(seed_language(&game, "zh_cn", Some("2024-06-13T08:24:03+00:00")).unwrap());
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "lang:zh_cn\n");
        std::fs::write(game.join("options.txt"), "lang:en_us\nfov:0.5\n").unwrap();
        assert!(!seed_language(&game, "ja_jp", None).unwrap(), "玩家已经有设置了，不动");
        assert_eq!(std::fs::read_to_string(game.join("options.txt")).unwrap(), "lang:en_us\nfov:0.5\n");
    }

    #[test]
    fn old_versions_use_uppercase_region() {
        assert_eq!(for_version("zh_cn", Some("2016-06-23T09:17:32+00:00")), "zh_CN");
        assert_eq!(for_version("zh_cn", Some("2016-11-14T13:16:58+00:00")), "zh_cn");
        assert_eq!(for_version("zh_cn", None), "zh_cn");
    }

    #[test]
    fn rejects_odd_values() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["", "zh", "ZH_CN", "zh_cn\nfov:1", "../x_y", "zh_cn_x", "zhcn_cn"] {
            assert!(!seed_language(dir.path(), bad, None).unwrap(), "{bad:?}");
        }
        assert!(!dir.path().join("options.txt").exists());
    }
}
