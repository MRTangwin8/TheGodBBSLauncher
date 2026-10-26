//! 启动后直接进服务器：把用户填的服务器地址变成游戏参数。
//! 地址会变成命令行参数，所以只放行主机名 / IP（含方括号 IPv6）加可选端口，其余一律拒绝。

use crate::engine::{Error, Result};

/// 校验 `host[:port]` / `[ipv6]:port`，返回（主机，端口）。主机会去掉方括号。
pub fn parse_server(input: &str) -> Option<(String, Option<u16>)> {
    let s = input.trim();
    if s.is_empty() || s.len() > 260 {
        return None;
    }
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        // [::1] 或 [::1]:25565
        let (h, after) = rest.split_once(']')?;
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':')?),
        };
        if h.is_empty() || !h.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.') || !h.contains(':') {
            return None;
        }
        (h.to_string(), port)
    } else {
        match s.rsplit_once(':') {
            // 没方括号的地址里只能有一个冒号，否则是没括号的 IPv6，歧义太大，不支持
            Some((h, p)) if !h.contains(':') => (h.to_string(), Some(p)),
            Some(_) => return None,
            None => (s.to_string(), None),
        }
    };
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['-', '.'])
        && !host.ends_with('.')
        && !host.contains("..")
        && (host.contains(':') || host.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_'));
    if !host_ok {
        return None;
    }
    let port = match port {
        None => None,
        Some(p) => Some(p.parse::<u16>().ok().filter(|p| *p > 0)?),
    };
    Some((host, port))
}

/// 这个游戏版本认不认 Quick Play 参数（`--quickPlayMultiplayer`，1.20 / 23w14a 起）
fn supports_quick_play(mc: &str) -> bool {
    let mc = mc.trim();
    // 快照：23w14a 之后
    if let Some((yy, rest)) = mc.split_once('w') {
        if let (Ok(year), Some(week)) = (yy.parse::<u32>(), rest.get(..2).and_then(|w| w.parse::<u32>().ok())) {
            return year > 23 || (year == 23 && week >= 14);
        }
    }
    let mut parts = mc.split(['.', '-']);
    let Some(first) = parts.next().and_then(|p| p.parse::<u32>().ok()) else { return false };
    if first != 1 {
        // 2026 年起的新命名（26.1 之类），都比 1.20 新
        return first > 1;
    }
    parts.next().and_then(|p| p.parse::<u32>().ok()).is_some_and(|minor| minor >= 20)
}

/// 进服务器要追加的游戏参数
pub fn server_join_args(mc_version: &str, server: &str) -> Result<Vec<String>> {
    let (host, port) = parse_server(server).ok_or_else(|| Error::Other(format!("服务器地址不合法：{server}（应为 主机名 或 主机名:端口）")))?;
    if supports_quick_play(mc_version) {
        let addr = match (port, host.contains(':')) {
            (Some(p), true) => format!("[{host}]:{p}"),
            (Some(p), false) => format!("{host}:{p}"),
            (None, true) => format!("[{host}]"),
            (None, false) => host,
        };
        Ok(vec!["--quickPlayMultiplayer".into(), addr])
    } else {
        Ok(vec!["--server".into(), host, "--port".into(), port.unwrap_or(25565).to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hosts_and_ports() {
        assert_eq!(parse_server("mc.example.com"), Some(("mc.example.com".into(), None)));
        assert_eq!(parse_server(" 127.0.0.1:25566 "), Some(("127.0.0.1".into(), Some(25566))));
        assert_eq!(parse_server("[::1]:25565"), Some(("::1".into(), Some(25565))));
        assert_eq!(parse_server("[2001:db8::1]"), Some(("2001:db8::1".into(), None)));
        assert_eq!(parse_server("my_server-1.net"), Some(("my_server-1.net".into(), None)));
    }

    #[test]
    fn rejects_anything_that_could_smuggle_arguments() {
        for bad in [
            "", "   ", "-evil", "--server x", "a b", "a;b", "a&b", "a|b", "$(x)", "`x`", "a\nb", "host:0", "host:70000", "host:abc", "host:",
            ":25565", "a..b", ".a", "a.", "::1", "1:2:3", "[::1", "[]", "[abc]", "[::1]x", "[g::1]:1", "http://x", "x/y", "x\\y", "host:1:2",
        ] {
            assert!(parse_server(bad).is_none(), "{bad:?} 应该被拒绝");
        }
        assert!(parse_server(&"a".repeat(300)).is_none());
    }

    #[test]
    fn quick_play_versions() {
        for v in ["1.20", "1.20.1", "1.21.4", "1.20-pre1", "23w14a", "23w31a", "24w44a", "26.3", "26.1.2"] {
            assert!(supports_quick_play(v), "{v}");
        }
        for v in ["1.19.4", "1.12.2", "1.7.10", "1.8.9", "23w13a", "22w46a", "b1.7.3", "a1.2.6", "", "abc"] {
            assert!(!supports_quick_play(v), "{v}");
        }
    }

    #[test]
    fn builds_arguments_per_version() {
        assert_eq!(server_join_args("1.21.1", "mc.example.com:25570").unwrap(), ["--quickPlayMultiplayer", "mc.example.com:25570"]);
        assert_eq!(server_join_args("1.21.1", "mc.example.com").unwrap(), ["--quickPlayMultiplayer", "mc.example.com"]);
        assert_eq!(server_join_args("1.21.1", "[::1]:25565").unwrap(), ["--quickPlayMultiplayer", "[::1]:25565"]);
        assert_eq!(server_join_args("1.12.2", "mc.example.com").unwrap(), ["--server", "mc.example.com", "--port", "25565"]);
        assert_eq!(server_join_args("1.12.2", "mc.example.com:1234").unwrap(), ["--server", "mc.example.com", "--port", "1234"]);
        assert!(server_join_args("1.21.1", "--bad").is_err());
    }
}
