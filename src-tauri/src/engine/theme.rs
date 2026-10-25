//! 主题文件（.godtheme）：把布局、配色、外观细调和背景图打包成一个 JSON，方便分享。
//! 背景图以 base64 内嵌。导入时所有字段都校验一遍：不认识的值丢掉、数值夹到合理范围，
//! 别人的主题文件再怎么写也只能影响外观。

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::{Error, Result};
use crate::config::{CustomPalette, Settings, Tweaks};

pub const EXT: &str = "godtheme";
const FORMAT: &str = "godbbs-theme";
const VERSION: u32 = 1;
/// 背景图大小上限，和设置里的背景图一致
pub const MAX_IMAGE: usize = 20 * 1024 * 1024;

const LAYOUTS: &[&str] = &["classic", "sidebar", "modern"];
const PALETTES: &[&str] = &["mcbbs", "grass", "nether", "end", "ocean", "cherry", "stone", "custom"];
const THEMES: &[&str] = &["light", "dark", "system"];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub layout: Option<String>,
    #[serde(default)]
    pub palette: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub custom_palette: Option<CustomPalette>,
    #[serde(default)]
    pub tweaks: Option<Tweaks>,
    #[serde(default)]
    pub background: Option<Background>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Background {
    /// 扩展名（png / jpg / webp / gif）
    pub ext: String,
    /// base64
    pub data: String,
}

/// 导入前给用户看的预览
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePreview {
    pub name: String,
    pub author: String,
    pub layout: Option<String>,
    pub palette: Option<String>,
    pub theme: Option<String>,
    pub custom_palette: Option<CustomPalette>,
    /// 背景图的 data URL
    pub background: Option<String>,
}

/// 用当前设置生成主题文件
pub fn export(name: &str, author: &str, s: &Settings) -> Result<Vec<u8>> {
    let background = match &s.background_image {
        Some(path) => Some(read_background(path)?),
        None => None,
    };
    let file = ThemeFile {
        format: FORMAT.into(),
        version: VERSION,
        name: name.trim().into(),
        author: author.trim().into(),
        layout: Some(s.layout.clone()),
        palette: Some(s.palette.clone()),
        theme: Some(s.theme.clone()),
        custom_palette: Some(s.custom_palette.clone()),
        tweaks: Some(s.tweaks.clone()),
        background,
    };
    Ok(serde_json::to_vec_pretty(&file)?)
}

fn read_background(path: &Path) -> Result<Background> {
    let ext = image_ext(path).ok_or_else(|| Error::Other("背景图格式不支持，只能是 png / jpg / webp / gif".into()))?;
    let bytes = std::fs::read(path).map_err(|e| Error::Other(format!("读不到背景图 {}：{e}", path.display())))?;
    if bytes.len() > MAX_IMAGE {
        return Err(Error::Other("背景图超过 20MB，换张小一点的再导出".into()));
    }
    Ok(Background { ext, data: base64::engine::general_purpose::STANDARD.encode(bytes) })
}

fn image_ext(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_EXTS.contains(&ext.as_str()).then_some(ext)
}

/// 解析并清洗主题文件
pub fn parse(bytes: &[u8]) -> Result<ThemeFile> {
    let mut f: ThemeFile = serde_json::from_slice(bytes).map_err(|e| Error::Other(format!("不是有效的主题文件：{e}")))?;
    if f.format != FORMAT {
        return Err(Error::Other("不是神权论坛启动器的主题文件".into()));
    }
    if f.version > VERSION {
        return Err(Error::Other("这个主题是新版启动器做的，先更新启动器再导入".into()));
    }
    f.name = clean_text(&f.name, 40);
    f.author = clean_text(&f.author, 40);
    f.layout = f.layout.filter(|v| LAYOUTS.contains(&v.as_str()));
    f.palette = f.palette.filter(|v| PALETTES.contains(&v.as_str()));
    f.theme = f.theme.filter(|v| THEMES.contains(&v.as_str()));
    f.custom_palette = f.custom_palette.map(clean_palette);
    f.tweaks = f.tweaks.map(clean_tweaks);
    if let Some(bg) = &f.background {
        let ext = bg.ext.to_ascii_lowercase();
        if !IMAGE_EXTS.contains(&ext.as_str()) {
            return Err(Error::Other(format!("主题里的背景图格式不支持：{}", bg.ext)));
        }
        // base64 每 4 个字符 3 字节，先粗算大小，不用真的解码
        if bg.data.len() / 4 * 3 > MAX_IMAGE {
            return Err(Error::Other("主题里的背景图超过 20MB".into()));
        }
    }
    Ok(f)
}

pub fn preview(f: &ThemeFile) -> ThemePreview {
    let background = f.background.as_ref().map(|bg| {
        let mime = match bg.ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => "image/png",
        };
        format!("data:{mime};base64,{}", bg.data)
    });
    ThemePreview {
        name: f.name.clone(),
        author: f.author.clone(),
        layout: f.layout.clone(),
        palette: f.palette.clone(),
        theme: f.theme.clone(),
        custom_palette: f.custom_palette.clone(),
        background,
    }
}

/// 把主题写进设置；背景图落盘到 bg_dir（按内容哈希命名，重复导入不会堆文件）。
/// 主题没带背景图就恢复成布局自带的背景。
pub fn apply(f: ThemeFile, s: &mut Settings, bg_dir: &Path) -> Result<()> {
    s.background_image = match f.background {
        Some(bg) => Some(save_background(&bg, bg_dir)?),
        None => None,
    };
    if let Some(v) = f.layout {
        s.layout = v;
    }
    if let Some(v) = f.palette {
        s.palette = v;
    }
    if let Some(v) = f.theme {
        s.theme = v;
    }
    if let Some(v) = f.custom_palette {
        s.custom_palette = v;
    }
    s.tweaks = f.tweaks.unwrap_or_default();
    Ok(())
}

fn save_background(bg: &Background, dir: &Path) -> Result<PathBuf> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bg.data.trim())
        .map_err(|_| Error::Other("主题里的背景图数据损坏".into()))?;
    if bytes.len() > MAX_IMAGE {
        return Err(Error::Other("主题里的背景图超过 20MB".into()));
    }
    std::fs::create_dir_all(dir)?;
    let hash = hex::encode(Sha1::digest(&bytes));
    let path = dir.join(format!("{}.{}", &hash[..16], bg.ext.to_ascii_lowercase()));
    if !path.exists() {
        std::fs::write(&path, bytes)?;
    }
    Ok(path)
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn clean_palette(p: CustomPalette) -> CustomPalette {
    let d = CustomPalette::default();
    let pick = |v: String, def: String| if is_hex_color(&v) { v.to_ascii_lowercase() } else { def };
    CustomPalette {
        accent: pick(p.accent, d.accent),
        surface: pick(p.surface, d.surface),
        text: pick(p.text, d.text),
        wood: pick(p.wood, d.wood),
    }
}

fn clean_tweaks(t: Tweaks) -> Tweaks {
    Tweaks {
        radius: t.radius.map(|v| v.min(24)),
        card_opacity: t.card_opacity.map(|v| v.min(100)),
        card_blur: t.card_blur.map(|v| v.min(40)),
        bg_blur: t.bg_blur.min(30),
        bg_dim: t.bg_dim.min(90),
        font_size: t.font_size.clamp(11, 18),
        // 字体名会写进 CSS 变量，只留字体名里会出现的字符
        font: t
            .font
            .chars()
            .filter(|c| c.is_alphanumeric() || " -_,\"'".contains(*c))
            .take(100)
            .collect(),
        animations: t.animations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_background() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("bg.PNG");
        std::fs::write(&img, b"\x89PNG fake").unwrap();
        let mut s = Settings {
            layout: "modern".into(),
            palette: "custom".into(),
            background_image: Some(img),
            ..Default::default()
        };
        s.custom_palette.accent = "#ff0000".into();
        s.tweaks.radius = Some(6);

        let bytes = export(" 红石 ", "Steve", &s).unwrap();
        let f = parse(&bytes).unwrap();
        assert_eq!(f.name, "红石");
        assert!(preview(&f).background.unwrap().starts_with("data:image/png;base64,"));

        let mut target = Settings::default();
        apply(f, &mut target, &dir.path().join("themes")).unwrap();
        assert_eq!(target.layout, "modern");
        assert_eq!(target.custom_palette.accent, "#ff0000");
        assert_eq!(target.tweaks.radius, Some(6));
        let bg = target.background_image.unwrap();
        assert_eq!(std::fs::read(&bg).unwrap(), b"\x89PNG fake");
        assert_eq!(bg.extension().unwrap(), "png");
    }

    #[test]
    fn hostile_values_are_cleaned() {
        let json = r##"{"format":"godbbs-theme","version":1,"layout":"evil","palette":"custom",
            "customPalette":{"accent":"red;}body{display:none","surface":"#ABCDEF"},
            "tweaks":{"radius":999,"fontSize":2,"font":"Arial;}*{color:red}","bgDim":100}}"##;
        let f = parse(json.as_bytes()).unwrap();
        assert_eq!(f.layout, None);
        let p = f.custom_palette.unwrap();
        assert_eq!(p.accent, CustomPalette::default().accent);
        assert_eq!(p.surface, "#abcdef");
        let t = f.tweaks.unwrap();
        assert_eq!((t.radius, t.font_size, t.bg_dim), (Some(24), 11, 90));
        assert_eq!(t.font, "Arialcolorred");
        assert!(t.animations);
    }

    #[test]
    fn rejects_foreign_and_future_files() {
        assert!(parse(br#"{"format":"other","version":1}"#).is_err());
        assert!(parse(br#"{"format":"godbbs-theme","version":99}"#).is_err());
        assert!(parse(br#"{"format":"godbbs-theme","version":1,"background":{"ext":"exe","data":""}}"#).is_err());
    }
}
