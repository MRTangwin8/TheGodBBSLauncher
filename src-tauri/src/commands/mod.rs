//! 前端 IPC。请求/响应走 command，下载进度走 event。

pub mod settings;
pub mod versions;

use std::collections::HashSet;
use std::path::PathBuf;

use tokio::sync::{Mutex, RwLock};

use crate::config::Settings;
use crate::engine::meta::VersionManifest;
use crate::engine::mirror::{self, Source, SourcePref};

pub struct AppState {
    pub settings: RwLock<Settings>,
    pub config_path: PathBuf,
    pub client: reqwest::Client,
    pub manifest: Mutex<Option<VersionManifest>>,
    /// 自动模式下测速选出的源，设置改动后清空重测
    pub auto_source: Mutex<Option<Source>>,
    pub installing: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn new(settings: Settings, config_path: PathBuf) -> Self {
        Self {
            settings: RwLock::new(settings),
            config_path,
            client: crate::engine::http_client(),
            manifest: Mutex::new(None),
            auto_source: Mutex::new(None),
            installing: Mutex::new(HashSet::new()),
        }
    }

    pub async fn source(&self) -> Source {
        match self.settings.read().await.download_source {
            SourcePref::Official => Source::Official,
            SourcePref::Bmclapi => Source::Bmclapi,
            SourcePref::Auto => {
                let mut cached = self.auto_source.lock().await;
                match *cached {
                    Some(s) => s,
                    None => {
                        let s = mirror::fastest(&mirror::probe(&self.client).await);
                        *cached = Some(s);
                        s
                    }
                }
            }
        }
    }
}
