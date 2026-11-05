//! 桌面快捷方式：双击直接启动某个版本。
//!
//! 快捷方式运行 `<启动器> --launch <版本 id>`。启动器已经开着时，单实例插件把参数转给已有的窗口
//! （lib.rs），不会再开第二个；没开着就正常启动，界面加载完后取走这个参数去启动游戏。
//!
//! Windows 写 `.lnk`（用 PowerShell 的 WScript.Shell，参数经环境变量传，不拼进脚本）；
//! Linux 写 `.desktop`。macOS 没有不弹终端的简单做法，暂不支持。

use std::path::{Path, PathBuf};

use crate::engine::{Error, Result};

pub const LAUNCH_FLAG: &str = "--launch";

/// 从命令行参数里取出要启动的版本 id；不合格的忽略
pub fn launch_arg(args: &[String]) -> Option<String> {
    let i = args.iter().position(|a| a == LAUNCH_FLAG)?;
    args.get(i + 1).filter(|id| crate::engine::safepath::name_ok(id)).cloned()
}

pub fn supported() -> bool {
    cfg!(any(windows, target_os = "linux"))
}

/// 快捷方式文件名：`<版本> - 神权论坛启动器`
fn file_stem(version_id: &str) -> String {
    format!("{version_id} - 神权论坛启动器")
}

/// 在 `desktop` 里建快捷方式，返回文件路径。`exe` 是启动器程序（Linux AppImage 要传 AppImage 本身的路径）
pub fn create(desktop: &Path, exe: &Path, version_id: &str) -> Result<PathBuf> {
    if !crate::engine::safepath::name_ok(version_id) {
        return Err(Error::VersionNotFound(version_id.to_string()));
    }
    if !supported() {
        return Err(Error::coded("shortcut.unsupported", &[], "这个系统暂时不支持创建桌面快捷方式"));
    }
    std::fs::create_dir_all(desktop)?;
    create_impl(desktop, exe, version_id)
}

#[cfg(windows)]
fn create_impl(desktop: &Path, exe: &Path, version_id: &str) -> Result<PathBuf> {
    use std::os::windows::process::CommandExt;
    let path = desktop.join(format!("{}.lnk", file_stem(version_id)));
    // 值都从环境变量读，脚本本身是固定的，版本名里有什么字符都不会变成命令
    const SCRIPT: &str = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:GODBBS_LNK); \
        $s.TargetPath = $env:GODBBS_EXE; $s.Arguments = $env:GODBBS_ARGS; \
        $s.WorkingDirectory = $env:GODBBS_DIR; $s.IconLocation = $env:GODBBS_EXE + ',0'; \
        $s.Description = $env:GODBBS_DESC; $s.Save()";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT])
        .env("GODBBS_LNK", &path)
        .env("GODBBS_EXE", exe)
        .env("GODBBS_ARGS", format!("{LAUNCH_FLAG} \"{version_id}\""))
        .env("GODBBS_DIR", exe.parent().unwrap_or(Path::new(".")))
        .env("GODBBS_DESC", format!("启动 Minecraft {version_id}"))
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| Error::Other(format!("没法运行 PowerShell：{e}")))?;
    if !out.status.success() || !path.is_file() {
        let err = String::from_utf8_lossy(&out.stderr).chars().take(300).collect::<String>();
        return Err(Error::coded("shortcut.failed", &[("detail", &err)], format!("创建快捷方式失败：{err}")));
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn create_impl(desktop: &Path, exe: &Path, version_id: &str) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = desktop.join(format!("godbbs-{}.desktop", version_id.replace(' ', "_")));
    std::fs::write(&path, desktop_entry(exe, version_id))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn create_impl(_: &Path, _: &Path, _: &str) -> Result<PathBuf> {
    unreachable!("supported() 已经挡住")
}

/// .desktop 的 Exec 字段：参数用双引号包起来，里面的 `"` `` ` `` `$` `\` 要转义，`%` 写成 `%%`
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_entry(exe: &Path, version_id: &str) -> String {
    let name = file_stem(version_id).replace(['\n', '\r'], " ");
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nComment=启动 Minecraft {version_id}\nExec={} {LAUNCH_FLAG} {}\nTerminal=false\nCategories=Game;\n",
        desktop_quote(&exe.display().to_string()),
        desktop_quote(version_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_launch_arg() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(launch_arg(&args(&["godbbs.exe", "--launch", "1.21.1 Fabric"])).as_deref(), Some("1.21.1 Fabric"));
        assert_eq!(launch_arg(&args(&["godbbs.exe"])), None);
        assert_eq!(launch_arg(&args(&["godbbs.exe", "--launch"])), None);
        assert_eq!(launch_arg(&args(&["godbbs.exe", "--launch", "../x"])), None);
        assert_eq!(launch_arg(&args(&["godbbs.exe", "--launch", "CON"])), None);
    }

    #[test]
    fn desktop_entry_quotes_values() {
        let e = desktop_entry(Path::new("/opt/God BBS/app"), "a$b`c%d");
        assert!(e.contains("Exec=\"/opt/God BBS/app\" --launch \"a\\$b\\`c%%d\""), "{e}");
        assert!(e.contains("Terminal=false"));
    }
}
