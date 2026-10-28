//! 全局设置：JSON 文件，放在系统应用配置目录。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::mirror::SourcePref;

mod buttons;
pub use buttons::{Buttons, QuickAction};
pub(crate) use buttons::{clean_text, is_hex_color};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// .minecraft 根目录（libraries / assets 在所有实例间共享）
    pub game_dir: PathBuf,
    pub download_source: SourcePref,
    pub concurrency: usize,
    /// 手动指定的 java；None 表示自动选择
    pub java_path: Option<PathBuf>,
    /// 最大内存 MB；None 表示自动
    pub memory_mb: Option<u32>,
    /// 额外 JVM 参数（空格分隔，支持引号）
    pub jvm_args: String,
    /// 额外游戏参数
    pub game_args: String,
    /// 版本隔离：每个版本用 versions/<id> 作为游戏目录（存档、mods、配置互不影响）
    pub isolate_versions: bool,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    /// 崩溃规则库更新地址（留空表示只用内置规则）
    pub crash_rules_url: String,
    /// CurseForge API Key；留空则通过 MCIM 镜像访问
    pub curseforge_api_key: String,
    /// 界面布局：classic（第一版论坛风）/ sidebar（侧边栏）/ modern（现代）
    pub layout: String,
    /// 配色方案：mcbbs / grass / nether / end / ocean / cherry / stone
    pub palette: String,
    /// light / dark / system
    pub theme: String,
    /// 自定义背景图；None 用内置像素风景
    pub background_image: Option<PathBuf>,
    /// 首页默认选中的版本（上次启动的）
    pub last_version: Option<String>,
    /// 启动时自动检查 GitHub 上有没有新版本
    pub auto_check_update: bool,
    /// 用户选了"跳过此版本"的版本号（不带 v）
    pub skipped_update: Option<String>,
    /// 做完（或跳过）首次启动引导。老版本的设置文件里没有这个字段，读取时按"已完成"处理，不打扰老用户
    pub onboarded: bool,
    /// 启动时和运行期间定期从 GitHub 拉公告
    pub announcements_enabled: bool,
    /// 用户关掉的公告 id（最多记 200 个）
    pub dismissed_announcements: Vec<String>,
    /// palette = "custom" 时用的自定义配色
    pub custom_palette: CustomPalette,
    /// 外观细调（圆角、透明度、字体……）
    pub tweaks: Tweaks,
    /// 启动按钮、导航栏、首页快捷按钮的自定义（不进主题文件，见 buttons.rs）
    pub buttons: Buttons,
}

/// 自定义配色：四个基础色，其余变量（深色版本、边框、悬停色）由前端推算
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomPalette {
    /// 强调色（按钮、选中项）
    pub accent: String,
    /// 卡片底色
    pub surface: String,
    pub text: String,
    /// "木头"色：导航、边框、标题
    pub wood: String,
}

impl Default for CustomPalette {
    fn default() -> Self {
        Self { accent: "#5e9c35".into(), surface: "#fbf2db".into(), text: "#3d2f1f".into(), wood: "#6b4a2b".into() }
    }
}

/// 外观细调；Option 为 None 表示用布局自己的默认值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tweaks {
    /// 卡片圆角 px
    pub radius: Option<u32>,
    /// 卡片不透明度 0~100
    pub card_opacity: Option<u32>,
    /// 卡片毛玻璃强度 px
    pub card_blur: Option<u32>,
    /// 背景模糊 px
    pub bg_blur: u32,
    /// 背景压暗 0~100
    pub bg_dim: u32,
    /// 基础字号 px
    pub font_size: u32,
    /// 字体；空字符串表示默认
    pub font: String,
    pub animations: bool,
}

impl Default for Tweaks {
    fn default() -> Self {
        Self { radius: None, card_opacity: None, card_blur: None, bg_blur: 0, bg_dim: 0, font_size: 13, font: String::new(), animations: true }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            game_dir: default_game_dir(),
            download_source: SourcePref::Auto,
            concurrency: 8,
            java_path: None,
            memory_mb: None,
            jvm_args: String::new(),
            game_args: String::new(),
            isolate_versions: true,
            window_width: None,
            window_height: None,
            crash_rules_url: String::new(),
            curseforge_api_key: String::new(),
            layout: "classic".into(),
            palette: "mcbbs".into(),
            theme: "light".into(),
            background_image: None,
            last_version: None,
            auto_check_update: true,
            skipped_update: None,
            onboarded: false,
            announcements_enabled: true,
            dismissed_announcements: Vec::new(),
            custom_palette: CustomPalette::default(),
            tweaks: Tweaks::default(),
            buttons: Buttons::default(),
        }
    }
}

impl Settings {
    /// 来自界面或磁盘的值都不可信：把按钮自定义里不合格的部分修正或丢掉
    pub fn sanitized(mut self) -> Self {
        // 游戏目录必须是绝对路径：相对路径会跟着启动时的工作目录跑，装出一堆找不到的文件
        if self.game_dir.as_os_str().is_empty() || !self.game_dir.is_absolute() {
            self.game_dir = default_game_dir();
        }
        self.buttons = self.buttons.sanitized();
        let mut dismissed: Vec<String> = Vec::new();
        for id in self.dismissed_announcements.iter().filter_map(|id| crate::engine::announce::clean_announce_id(id)) {
            if !dismissed.contains(&id) {
                dismissed.push(id);
            }
        }
        let skip = dismissed.len().saturating_sub(200);
        self.dismissed_announcements = dismissed.split_off(skip);
        self
    }
}

/// 游戏目录的默认位置：启动器所在目录下的 `.minecraft`，这样用户一眼就能找到，和启动器放在一起。
/// 启动器目录不适合放游戏数据时（只读、系统目录、macOS 的 .app 包里、AppImage 的临时挂载点）退回到用户数据目录。
pub fn default_game_dir() -> PathBuf {
    launcher_game_dir().unwrap_or_else(fallback_game_dir)
}

/// 启动器目录下的 `.minecraft`；这个位置不合适就返回 None
pub fn launcher_game_dir() -> Option<PathBuf> {
    launcher_dir_for(&std::env::current_exe().ok()?, dir_writable)
}

/// 旧版本的默认位置（用户数据目录）。老用户的游戏文件还在这里，设置里会提示他们迁移
pub fn legacy_game_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("GodBBSLauncher").join(".minecraft")
}

/// 启动器目录不能用时的兜底：本机用户数据目录（不会随账号漫游同步）。
/// 商店版（MSIX）往 AppData 写的东西会被重定向到应用私有目录，卸载时连存档一起删掉，所以改放用户主目录
fn fallback_game_dir() -> PathBuf {
    if crate::edition::STORE {
        if let Some(home) = dirs::home_dir() {
            return home.join("GodBBSLauncher").join(".minecraft");
        }
    }
    dirs::data_local_dir().or_else(dirs::data_dir).unwrap_or_else(|| PathBuf::from(".")).join("GodBBSLauncher").join(".minecraft")
}

/// 可执行文件所在目录下的 `.minecraft`，目录不合适就返回 None。`writable` 用来探测目录能不能写，方便测试。
fn launcher_dir_for(exe: &Path, writable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let lower = dir.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    let unsuitable = lower.contains(".app/contents") // macOS 应用包：更新会整个替换，里面不能放数据
        || lower.contains("/program files") // 普通权限写不了；写得了也不该往里放几个 GB
        || lower.contains("/windowsapps/")
        || ["/usr/", "/opt/", "/snap/", "/nix/", "/tmp/.mount_"].iter().any(|p| lower.starts_with(p)); // Linux 系统目录、AppImage 挂载点
    (!unsuitable && writable(dir)).then(|| dir.join(".minecraft"))
}

/// 这个目录能不能创建文件：真的建一个临时文件试一下，比看权限位可靠
pub fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".godbbs-write-test-{}", std::process::id()));
    let ok = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// 旧版本用过的应用标识符（mcbbs.me 不是本项目的域名，已改用 co.mcbbs.*）
pub const LEGACY_IDENTIFIER: &str = "me.mcbbs.godbbs";

/// 把旧标识符的同级目录整体改名成新目录。新目录已存在、旧目录不存在、或改名失败都什么也不做。
/// 成功时返回旧目录，供调用方修正设置里指向旧目录的路径。
pub fn migrate_legacy_dir(new_dir: &Path) -> Option<PathBuf> {
    let old_dir = new_dir.parent()?.join(LEGACY_IDENTIFIER);
    if new_dir.exists() || !old_dir.is_dir() {
        return None;
    }
    std::fs::rename(&old_dir, new_dir).ok()?;
    Some(old_dir)
}

impl Settings {
    /// 设置里指向 old 目录下的路径改成 new 目录下的同一位置；有改动返回 true
    pub fn rebase_paths(&mut self, old: &Path, new: &Path) -> bool {
        let fix = |p: &mut PathBuf| match p.strip_prefix(old) {
            Ok(rest) => {
                *p = new.join(rest);
                true
            }
            Err(_) => false,
        };
        let mut changed = fix(&mut self.game_dir);
        if let Some(p) = self.java_path.as_mut() {
            changed |= fix(p);
        }
        if let Some(p) = self.background_image.as_mut() {
            changed |= fix(p);
        }
        changed
    }
}

/// 读不到或解析失败都回退默认值，不让坏配置挡住启动
pub fn load(path: &Path) -> Settings {
    let text = std::fs::read_to_string(path).ok();
    let mut settings = text.as_deref().and_then(|s| serde_json::from_str::<Settings>(s).ok()).unwrap_or_default();
    // 设置文件是旧版本写的（没有 onboarded 字段）：说明不是新用户，不弹引导
    if text.as_deref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()).is_some_and(|v| v.get("onboarded").is_none()) {
        settings.onboarded = true;
    }
    settings.sanitized()
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(settings)?)
}

/// 按空白切分参数，双引号/单引号内的空白保留
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has_token = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has_token = true;
            }
            (None, c) if c.is_whitespace() => {
                if has_token || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if has_token || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 自动内存：物理内存的 1/4，限制在 1~4 GB，按 256MB 取整
pub fn auto_memory_mb(total_mb: u64) -> u32 {
    ((total_mb / 4).clamp(1024, 4096) / 256 * 256) as u32
}

#[cfg(test)]
mod tests {
    #[test]
    fn onboarding_is_skipped_for_settings_files_from_older_versions() {
        let dir = std::env::temp_dir().join(format!("godbbs-onboard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert!(!load(&path).onboarded, "没有设置文件 = 新用户");
        std::fs::write(&path, r#"{"concurrency": 32}"#).unwrap();
        assert!(load(&path).onboarded, "老文件没有这个字段 = 老用户");
        std::fs::write(&path, r#"{"onboarded": false}"#).unwrap();
        assert!(!load(&path).onboarded);
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;

    /// macOS 的窗口配置（原生红绿灯 + 覆盖式标题栏）在 Windows 上没法真机试，
    /// 这里用 Tauri 自己的解析器检查：字段名写错会被 deny_unknown_fields 拒绝，各平台的 CI 都能提前发现
    #[test]
    fn macos_window_config_parses_with_native_traffic_lights() {
        use tauri::utils::{config::WindowConfig, TitleBarStyle};
        let file: serde_json::Value = serde_json::from_str(include_str!("../../tauri.macos.conf.json")).unwrap();
        let windows = file["app"]["windows"].as_array().expect("平台配置按 JSON 合并补丁合并，数组会被整体替换，必须写出完整的窗口");
        assert_eq!(windows.len(), 1);
        let window: WindowConfig = serde_json::from_value(windows[0].clone()).unwrap();
        assert!(window.decorations, "红绿灯需要保留系统窗口边框");
        assert_eq!(window.title_bar_style, TitleBarStyle::Overlay);
        assert!(window.hidden_title);
        // Tauri 的 y 不是红绿灯离顶边的距离，而是「标题栏容器高度 - 按钮高度」，按钮在容器里还保持系统默认的离底距离（约 7~9pt），
        // 实际顶边距约等于 y - 8。写 12 时红绿灯几乎贴着窗口顶边，20 才落到 layouts.css 留出的 y 12~24
        let pos = window.traffic_light_position.expect("要给红绿灯定位");
        assert!((18.0..=22.0).contains(&pos.y), "红绿灯 y = {}，会贴顶或压住标题", pos.y);
        // 和主配置里的窗口尺寸保持一致，免得两处改漏
        let base: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        for key in ["title", "width", "height", "minWidth", "minHeight", "devtools"] {
            assert_eq!(windows[0][key], base["app"]["windows"][0][key], "{key} 和 tauri.conf.json 不一致");
        }
    }

    #[test]
    fn game_dir_defaults_next_to_the_launcher_when_that_is_safe() {
        let ok = |_: &Path| true;
        let no = |_: &Path| false;
        let dir = |exe: &str, w: bool| launcher_dir_for(Path::new(exe), |d| if w { ok(d) } else { no(d) });
        // Windows 普通安装 / 自定义安装目录
        assert_eq!(dir("D:/games/Minecraft/神权论坛启动器/godbbs-launcher.exe", true), Some(PathBuf::from("D:/games/Minecraft/神权论坛启动器/.minecraft")));
        assert!(dir("C:/Users/me/AppData/Local/神权论坛启动器/godbbs-launcher.exe", true).is_some());
        // 目录不可写（比如 MSI 默认装进 Program Files，普通权限写不了）
        assert_eq!(dir("D:/games/app/godbbs-launcher.exe", false), None);
        // 就算写得了，也不往系统目录和应用包里放游戏数据
        for exe in [
            "C:/Program Files/神权论坛启动器/godbbs-launcher.exe",
            "C:/Program Files (x86)/x/godbbs-launcher.exe",
            "C:/Program Files/WindowsApps/x/godbbs-launcher.exe",
            "/Applications/神权论坛启动器.app/Contents/MacOS/godbbs-launcher",
            "/Users/me/Apps/神权论坛启动器.app/Contents/MacOS/godbbs-launcher",
            "/usr/bin/godbbs-launcher",
            "/opt/godbbs/godbbs-launcher",
            "/tmp/.mount_GodBBSabc123/usr/bin/godbbs-launcher",
        ] {
            assert_eq!(dir(exe, true), None, "{exe}");
        }
        // Linux 解压到家目录的绿色版可以用
        assert_eq!(dir("/home/me/games/godbbs/godbbs-launcher", true), Some(PathBuf::from("/home/me/games/godbbs/.minecraft")));
    }

    #[test]
    fn default_game_dir_is_absolute_and_relative_paths_are_replaced() {
        assert!(default_game_dir().is_absolute());
        assert!(legacy_game_dir().ends_with("GodBBSLauncher/.minecraft") || legacy_game_dir().ends_with("GodBBSLauncher\\.minecraft"));
        for bad in ["", "relative/path", ".minecraft"] {
            let s = Settings { game_dir: PathBuf::from(bad), ..Settings::default() }.sanitized();
            assert!(s.game_dir.is_absolute(), "{bad:?} 应该被替换成默认位置");
        }
        let keep = if cfg!(windows) { "D:/games/mc" } else { "/data/mc" };
        assert_eq!(Settings { game_dir: PathBuf::from(keep), ..Settings::default() }.sanitized().game_dir, PathBuf::from(keep));
    }

    #[test]
    fn dir_writable_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        assert!(dir_writable(dir.path()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "探测用的临时文件要删掉");
        assert!(!dir_writable(&dir.path().join("不存在的目录")));
    }

    #[test]
    fn splits_quoted_args() {
        assert_eq!(split_args(r#"-Xss1m  -Dname="a b" '' -Dx=1"#), ["-Xss1m", "-Dname=a b", "", "-Dx=1"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn auto_memory_bounds() {
        assert_eq!(auto_memory_mb(2048), 1024);
        assert_eq!(auto_memory_mb(16 * 1024), 4096);
        assert_eq!(auto_memory_mb(10 * 1024), 2560);
    }

    #[test]
    fn migrates_legacy_dir_and_rebases_paths() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(old.join("themes")).unwrap();
        std::fs::write(old.join("settings.json"), "{}").unwrap();
        let new = root.path().join("co.mcbbs.godbbs");

        assert_eq!(migrate_legacy_dir(&new), Some(old.clone()));
        assert!(new.join("settings.json").exists() && !old.exists());
        assert_eq!(migrate_legacy_dir(&new), None, "只迁移一次");

        let mut s = Settings { background_image: Some(old.join("themes").join("a.png")), ..Default::default() };
        assert!(s.rebase_paths(&old, &new));
        assert_eq!(s.background_image.unwrap(), new.join("themes").join("a.png"));
        assert!(!Settings::default().rebase_paths(&old, &new));
    }

    #[test]
    fn old_settings_file_still_loads() {
        let s: Settings = serde_json::from_str(r#"{"gameDir":"/mc","downloadSource":"bmclapi","concurrency":32}"#).unwrap();
        assert!(s.isolate_versions);
        assert_eq!(s.concurrency, 32);
    }
}
