//! 首页快捷按钮的后端动作：打开文件夹、运行本地程序。
//! 「启动游戏」「打开网址」不需要专门的命令：前者走 `launch_game`，后者由前端的 opener 打开。

use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::AppState;
use crate::config::QuickAction;
use crate::engine::{Error, Result};

/// 在文件管理器里打开一个文件夹
#[tauri::command]
pub async fn quick_open_folder(app: AppHandle, path: String) -> Result<()> {
    let path = path.trim();
    if !Path::new(path).is_dir() {
        return Err(Error::Other(format!("文件夹不存在：{path}")));
    }
    app.opener().open_path(path, None::<&str>).map_err(|e| Error::Other(format!("打不开文件夹：{e}")))
}

/// 运行一个本地程序或脚本，等同于在文件管理器里双击它。
/// 只会运行**已经保存在快捷按钮里**的路径：就算界面被人动了手脚，也没法借这个命令运行任意程序。
/// 每次运行前界面还会弹窗让用户确认完整路径。
#[tauri::command]
pub async fn quick_run_program(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<()> {
    let saved = state
        .settings
        .read()
        .await
        .buttons
        .quick
        .iter()
        .any(|b| matches!(&b.action, QuickAction::RunProgram { path: p } if *p == path));
    if !saved {
        return Err(Error::Other("这个程序不在你保存的快捷按钮里，已拒绝运行".into()));
    }
    let target = Path::new(&path);
    // macOS 的 .app 其实是个文件夹
    if !target.is_file() && !(cfg!(target_os = "macos") && target.is_dir()) {
        return Err(Error::Other(format!("找不到这个程序：{path}")));
    }
    app.opener().open_path(path.as_str(), None::<&str>).map_err(|e| Error::Other(format!("运行失败：{e}")))
}
