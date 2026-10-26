//! 启动按钮、导航栏和首页快捷按钮的自定义。
//!
//! 注意：这些**不进 `.godtheme` 主题文件**。快捷按钮能运行本地程序，不能让别人分享的主题带进来。
//! 设置文件里的内容按不可信数据处理：保存和读取时都过一遍 `sanitized`，不合格的整个丢掉。

use serde::{Deserialize, Serialize};

use crate::engine::launch::server::parse_server;

/// 首页最多放几个快捷按钮
pub const MAX_QUICK: usize = 8;
const NAV_LIMIT: usize = 24;
const STYLES: &[&str] = &["stone", "flat", "pill"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Buttons {
    /// 启动按钮上的文字；空表示默认的"开始游戏"
    pub launch_label: String,
    /// stone（Minecraft 石头按钮）/ flat（扁平）/ pill（胶囊）
    pub launch_style: String,
    /// #rrggbb；空表示跟随配色方案的强调色
    pub launch_color: String,
    /// 点击启动时发出一声提示音
    pub launch_sound: bool,
    /// Ctrl+Enter（macOS 是 ⌘+Enter）直接启动
    pub launch_shortcut: bool,
    /// 导航栏顺序（路由路径）；没列出的按默认顺序排在后面
    pub nav_order: Vec<String>,
    pub nav_hidden: Vec<String>,
    pub quick: Vec<QuickButton>,
}

impl Default for Buttons {
    fn default() -> Self {
        Self {
            launch_label: String::new(),
            launch_style: "stone".into(),
            launch_color: String::new(),
            launch_sound: false,
            launch_shortcut: true,
            nav_order: Vec::new(),
            nav_hidden: Vec::new(),
            quick: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickButton {
    pub id: String,
    pub label: String,
    /// 一个 emoji 或一两个字，可以留空
    #[serde(default)]
    pub icon: String,
    pub action: QuickAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum QuickAction {
    /// 在文件管理器里打开文件夹
    OpenFolder { path: String },
    /// 用系统浏览器打开网址（只允许 http / https）
    OpenUrl { url: String },
    /// 启动某个版本，可选直接进服务器
    Launch {
        version_id: String,
        #[serde(default)]
        server: Option<String>,
    },
    /// 运行本地程序 / 脚本，等同于在文件管理器里双击；每次运行前界面都会让用户确认
    RunProgram { path: String },
}

pub(crate) fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

pub(crate) fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 路径：不能为空、不能带控制字符（含 NUL）、别太长
fn clean_path(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.chars().count() <= 1024 && !s.chars().any(|c| c.is_control())).then(|| s.to_string())
}

/// 路由路径，比如 /download
fn clean_route(s: &str) -> Option<String> {
    let s = s.trim();
    (s.starts_with('/') && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/')).then(|| s.to_string())
}

fn clean_id(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then(|| s.to_string())
}

fn clean_routes(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for route in list.iter().filter_map(|r| clean_route(r)) {
        if !out.contains(&route) && out.len() < NAV_LIMIT {
            out.push(route);
        }
    }
    out
}

impl Buttons {
    pub fn sanitized(mut self) -> Self {
        self.launch_label = clean_text(&self.launch_label, 12);
        if !STYLES.contains(&self.launch_style.as_str()) {
            self.launch_style = "stone".into();
        }
        self.launch_color = if is_hex_color(&self.launch_color) { self.launch_color.to_ascii_lowercase() } else { String::new() };
        self.nav_order = clean_routes(std::mem::take(&mut self.nav_order));
        self.nav_hidden = clean_routes(std::mem::take(&mut self.nav_hidden));
        let mut seen = Vec::new();
        self.quick = std::mem::take(&mut self.quick)
            .into_iter()
            .filter_map(QuickButton::sanitized)
            .filter(|b| {
                let fresh = !seen.contains(&b.id);
                seen.push(b.id.clone());
                fresh
            })
            .take(MAX_QUICK)
            .collect();
        self
    }
}

impl QuickButton {
    fn sanitized(self) -> Option<Self> {
        let label = clean_text(&self.label, 16);
        if label.is_empty() {
            return None;
        }
        Some(Self { id: clean_id(&self.id)?, label, icon: clean_text(&self.icon, 4), action: self.action.sanitized()? })
    }
}

impl QuickAction {
    fn sanitized(self) -> Option<Self> {
        Some(match self {
            Self::OpenFolder { path } => Self::OpenFolder { path: clean_path(&path)? },
            Self::RunProgram { path } => Self::RunProgram { path: clean_path(&path)? },
            Self::OpenUrl { url } => {
                let parsed = reqwest::Url::parse(url.trim()).ok()?;
                if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() {
                    return None;
                }
                Self::OpenUrl { url: parsed.to_string() }
            }
            Self::Launch { version_id, server } => {
                let version_id = version_id.trim().to_string();
                if version_id.is_empty() || version_id.len() > 128 || version_id == "." || version_id == ".." || version_id.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
                    return None;
                }
                // 服务器地址不合格就当没填，只启动游戏，不把乱七八糟的东西塞进启动参数
                let server = server.map(|s| s.trim().to_string()).filter(|s| parse_server(s).is_some());
                Self::Launch { version_id, server }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick(id: &str, label: &str, action: QuickAction) -> QuickButton {
        QuickButton { id: id.into(), label: label.into(), icon: String::new(), action }
    }

    #[test]
    fn old_settings_without_buttons_get_defaults() {
        let b: Buttons = serde_json::from_str("{}").unwrap();
        assert_eq!(b, Buttons::default());
        assert!(b.launch_shortcut && !b.launch_sound && b.launch_style == "stone");
    }

    #[test]
    fn actions_roundtrip_in_the_shape_the_frontend_uses() {
        let json = r##"{"launchLabel":"冲！","quick":[
            {"id":"a","label":"存档","icon":"📁","action":{"type":"openFolder","path":"C:/mc/saves"}},
            {"id":"b","label":"进服","action":{"type":"launch","versionId":"1.21.1","server":"mc.example.com"}},
            {"id":"c","label":"站点","action":{"type":"openUrl","url":"https://example.com/"}},
            {"id":"d","label":"脚本","action":{"type":"runProgram","path":"C:/tools/backup.bat"}}]}"##;
        let b: Buttons = serde_json::from_str(json).unwrap();
        assert_eq!(b.quick.len(), 4);
        assert_eq!(b.quick[1].action, QuickAction::Launch { version_id: "1.21.1".into(), server: Some("mc.example.com".into()) });
        let again: serde_json::Value = serde_json::to_value(&b).unwrap();
        assert_eq!(again["quick"][1]["action"]["versionId"], "1.21.1");
        assert_eq!(again["quick"][3]["action"]["type"], "runProgram");
    }

    #[test]
    fn sanitizing_drops_or_fixes_bad_values() {
        let b = Buttons {
            launch_label: "  开始\n游戏  ".into(),
            launch_style: "fancy".into(),
            launch_color: "red;}body{".into(),
            nav_order: vec!["/download".into(), "/download".into(), "javascript:x".into(), "/a b".into(), "/versions".into()],
            nav_hidden: vec!["/log".into(), "log".into()],
            quick: vec![
                quick("ok", "存档", QuickAction::OpenFolder { path: "C:/mc".into() }),
                quick("ok", "重复 id", QuickAction::OpenFolder { path: "C:/x".into() }),
                quick("bad id!", "非法 id", QuickAction::OpenFolder { path: "C:/mc".into() }),
                quick("e1", "   ", QuickAction::OpenFolder { path: "C:/mc".into() }),
                quick("e2", "空路径", QuickAction::RunProgram { path: "  ".into() }),
                quick("e3", "带 NUL", QuickAction::RunProgram { path: "C:/a\0b".into() }),
                quick("u1", "js", QuickAction::OpenUrl { url: "javascript:alert(1)".into() }),
                quick("u2", "file", QuickAction::OpenUrl { url: "file:///C:/Windows/System32/cmd.exe".into() }),
                quick("u3", "带密码", QuickAction::OpenUrl { url: "https://user:pw@example.com".into() }),
                quick("u4", "好网址", QuickAction::OpenUrl { url: " https://example.com/a?b=1 ".into() }),
                quick("l1", "路径版本", QuickAction::Launch { version_id: "../x".into(), server: None }),
                quick("l2", "坏服务器", QuickAction::Launch { version_id: "1.21.1".into(), server: Some("--server evil".into()) }),
            ],
            ..Buttons::default()
        }
        .sanitized();
        assert_eq!(b.launch_label, "开始游戏");
        assert_eq!((b.launch_style.as_str(), b.launch_color.as_str()), ("stone", ""));
        assert_eq!(b.nav_order, ["/download", "/versions"]);
        assert_eq!(b.nav_hidden, ["/log"]);
        let ids: Vec<&str> = b.quick.iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["ok", "u4", "l2"], "只留下合格的");
        assert_eq!(b.quick[1].action, QuickAction::OpenUrl { url: "https://example.com/a?b=1".into() });
        assert_eq!(b.quick[2].action, QuickAction::Launch { version_id: "1.21.1".into(), server: None });
    }

    #[test]
    fn color_is_normalized_and_quick_buttons_are_capped() {
        let b = Buttons { launch_color: "#AABBCC".into(), launch_style: "pill".into(), ..Buttons::default() }.sanitized();
        assert_eq!((b.launch_color.as_str(), b.launch_style.as_str()), ("#aabbcc", "pill"));
        let many = (0..20).map(|i| quick(&format!("b{i}"), "按钮", QuickAction::OpenFolder { path: "C:/mc".into() })).collect();
        assert_eq!(Buttons { quick: many, ..Buttons::default() }.sanitized().quick.len(), MAX_QUICK);
    }
}
