//! 公告栏：定期从 GitHub 拉取公告，缓存到磁盘，断网时显示上次拿到的。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;

use super::AppState;
use crate::engine::announce::{self, Announcement};
use crate::engine::Result;

/// 缓存多久内不重新去拉（手动刷新除外）
const FRESH_SECS: u64 = 20 * 60;

#[derive(Serialize, Deserialize)]
struct Cache {
    fetched_at: u64,
    /// 公告按版本号筛选过，换了版本缓存就作废
    version: String,
    items: Vec<Announcement>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementFeed {
    pub items: Vec<Announcement>,
    /// 这批公告是什么时候拿到的（Unix 秒）
    pub fetched_at: u64,
    /// true 表示这次没拉成功，显示的是上次缓存的
    pub stale: bool,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn feed(items: Vec<Announcement>, fetched_at: u64, stale: bool) -> AnnouncementFeed {
    AnnouncementFeed { items: announce::visible(items, &announce::today_utc()), fetched_at, stale }
}

/// 取公告。`force` 为 true（设置里的"立即刷新"）时不看缓存新不新
#[tauri::command]
pub async fn get_announcements(state: State<'_, AppState>, force: bool) -> Result<AnnouncementFeed> {
    let version = env!("CARGO_PKG_VERSION");
    let path = state.data_dir.join("cache").join("announcements.json");
    let cached: Option<Cache> = tokio::fs::read(&path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Cache>(&bytes).ok())
        .filter(|c| c.version == version);

    if let Some(c) = cached.as_ref().filter(|c| !force && now_secs().saturating_sub(c.fetched_at) < FRESH_SECS) {
        return Ok(feed(c.items.clone(), c.fetched_at, false));
    }
    match announce::fetch(&state.client, version).await {
        Ok(items) => {
            let at = now_secs();
            if let Some(dir) = path.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            if let Ok(bytes) = serde_json::to_vec(&Cache { fetched_at: at, version: version.into(), items: items.clone() }) {
                let _ = tokio::fs::write(&path, bytes).await;
            }
            Ok(feed(items, at, false))
        }
        // 断网或 GitHub 连不上：有缓存就用缓存，没有才报错
        Err(e) => match cached {
            Some(c) => Ok(feed(c.items, c.fetched_at, true)),
            None => Err(e),
        },
    }
}
