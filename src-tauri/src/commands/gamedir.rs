//! 游戏目录：查看推荐位置、迁移旧数据、只改位置。

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::AppState;
use crate::config;
use crate::engine::migrate::{self, Progress, Report};
use crate::engine::{Error, Result};

pub const MIGRATE_PROGRESS_EVENT: &str = "migrate-progress";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirInfo {
    pub current: String,
    /// 启动器所在目录旁边的 .minecraft（装在 Program Files 等不能写的地方时是备用位置）
    pub recommended: String,
    /// 老版本默认放的位置（%APPDATA%/GodBBSLauncher/.minecraft）
    pub legacy: String,
    /// 现在用的正是老位置：界面会建议迁到启动器旁边
    pub current_is_legacy: bool,
    /// 现在用的不是老位置，但老位置里还留着游戏数据（比如手动改过路径没搬）：界面会建议把它迁到现在的目录
    pub legacy_has_data: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirStats {
    pub exists: bool,
    pub empty: bool,
    pub files: u64,
    pub bytes: u64,
}

fn has_data(dir: &std::path::Path) -> bool {
    dir.join("versions").is_dir()
}

#[tauri::command]
pub async fn game_dir_info(state: State<'_, AppState>) -> Result<GameDirInfo> {
    let current = state.settings.read().await.game_dir.clone();
    let legacy = config::legacy_game_dir();
    let current_is_legacy = migrate::is_inside(&legacy, &current) && migrate::is_inside(&current, &legacy);
    let legacy_has_data = !current_is_legacy && has_data(&legacy);
    Ok(GameDirInfo {
        current: current.display().to_string(),
        recommended: config::default_game_dir().display().to_string(),
        legacy: legacy.display().to_string(),
        current_is_legacy,
        legacy_has_data,
    })
}

/// 看一个文件夹的大小和文件数（扫描可能很慢，放到阻塞线程里）
#[tauri::command]
pub async fn dir_stats(path: String) -> Result<DirStats> {
    let dir = PathBuf::from(path);
    tokio::task::spawn_blocking(move || {
        if !dir.is_dir() {
            return DirStats { exists: dir.exists(), empty: false, files: 0, bytes: 0 };
        }
        let (files, bytes) = migrate::stats(&dir);
        let empty = std::fs::read_dir(&dir).map(|mut d| d.next().is_none()).unwrap_or(false);
        DirStats { exists: true, empty, files, bytes }
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))
}

struct Migrating<'a>(&'a AppState);
impl Drop for Migrating<'_> {
    fn drop(&mut self) {
        self.0.migrating.store(false, Ordering::SeqCst);
    }
}

/// 换游戏目录。`move_files` 为 true 时把旧目录的内容搬过去；false 只改位置（旧数据留在原处）。
/// `from_legacy`：搬的不是当前游戏目录，而是老版本默认位置里留下的数据（只允许这一个来源）。
#[tauri::command]
pub async fn game_dir_migrate(app: AppHandle, state: State<'_, AppState>, to: String, move_files: bool, from_legacy: Option<bool>) -> Result<Report> {
    let state: &AppState = &state;
    let to = PathBuf::from(to.trim());
    if !to.is_absolute() {
        return Err(Error::Other("请选一个完整的文件夹路径".into()));
    }
    if state.migrating.swap(true, Ordering::SeqCst) {
        return Err(Error::Other("正在迁移，别急".into()));
    }
    let _guard = Migrating(state);
    state.migrate_cancel.store(false, Ordering::SeqCst);
    // 先看任务登记、再看运行中的游戏：启动游戏时是先登记进 games 再撤掉任务登记的，这个顺序不会两头都漏掉
    if state.busy.load(Ordering::SeqCst) > 0 || !state.installing.lock().await.is_empty() {
        return Err(Error::Other("还有下载 / 安装 / 导入任务在进行（或者游戏正在启动），等它们完成再换目录".into()));
    }
    if !state.games.lock().await.is_empty() {
        return Err(Error::Other("游戏还在运行，先关掉游戏再换目录".into()));
    }
    let from = if from_legacy == Some(true) { config::legacy_game_dir() } else { state.settings.read().await.game_dir.clone() };
    if migrate::is_inside(&from, &to) && migrate::is_inside(&to, &from) {
        return Err(Error::Other("新位置和现在的一样".into()));
    }

    let mut new_settings = state.settings.read().await.clone();
    new_settings.game_dir = to.clone();
    let config_path = state.config_path.clone();

    if !move_files || !from.is_dir() {
        std::fs::create_dir_all(&to).map_err(|e| Error::Other(format!("没法创建新位置：{}（{e}）", to.display())))?;
        if !config::dir_writable(&to) {
            return Err(Error::Other(format!("新位置没法写入：{}，换一个位置试试", to.display())));
        }
        config::save(&config_path, &new_settings)?;
        *state.settings.write().await = new_settings;
        return Ok(Report::default());
    }

    let handle = tokio::runtime::Handle::current();
    let cancel = &state.migrate_cancel;
    let settings_lock = &state.settings;
    let last = std::cell::Cell::new(Instant::now() - Duration::from_secs(1));
    // 文件操作是阻塞的；block_in_place 让出当前 worker，别的命令照常响应
    let report = tokio::task::block_in_place(|| {
        migrate::migrate(
            &from,
            &to,
            false,
            &|p: &Progress| {
                if last.get().elapsed() >= Duration::from_millis(150) {
                    last.set(Instant::now());
                    let _ = app.emit(MIGRATE_PROGRESS_EVENT, p);
                }
                !cancel.load(Ordering::SeqCst)
            },
            &|| {
                config::save(&config_path, &new_settings)?;
                *handle.block_on(settings_lock.write()) = new_settings.clone();
                Ok(())
            },
        )
    })?;
    Ok(report)
}

#[tauri::command]
pub fn game_dir_migrate_cancel(state: State<'_, AppState>) {
    state.migrate_cancel.store(true, Ordering::SeqCst);
}
