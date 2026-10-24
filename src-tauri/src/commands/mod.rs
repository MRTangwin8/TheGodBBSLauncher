//! 前端 IPC。请求/响应走 command，下载进度和游戏日志走 event。

pub mod accounts;
pub mod crash;
pub mod game;
pub mod java;
pub mod loaders;
pub mod resources;
pub mod settings;
pub mod versions;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex, RwLock};

use crate::config::Settings;
use crate::engine::auth::microsoft::DeviceCode;
use crate::engine::auth::store::AccountStore;
use crate::engine::crash::RuleSet;
use crate::engine::java::JavaInstall;
use crate::engine::meta::VersionManifest;
use crate::engine::mirror::{self, Source, SourcePref};

pub struct RunningGame {
    pub version_id: String,
    pub kill: Option<oneshot::Sender<()>>,
}

pub struct AppState {
    pub settings: RwLock<Settings>,
    pub config_path: PathBuf,
    /// 启动器自己的数据目录（Java runtime、authlib-injector）
    pub data_dir: PathBuf,
    pub client: reqwest::Client,
    pub manifest: Mutex<Option<VersionManifest>>,
    /// 自动模式下测速选出的源，设置改动后清空重测
    pub auto_source: Mutex<Option<Source>>,
    pub installing: Mutex<HashSet<String>>,
    pub accounts: Mutex<AccountStore>,
    pub pending_login: Mutex<Option<DeviceCode>>,
    pub login_cancel: Arc<AtomicBool>,
    pub java_cache: Mutex<Option<Vec<JavaInstall>>>,
    pub games: Mutex<HashMap<String, RunningGame>>,
    pub crash_rules: RwLock<Arc<RuleSet>>,
}

impl AppState {
    pub fn new(settings: Settings, config_path: PathBuf, data_dir: PathBuf, accounts: AccountStore) -> Self {
        let crash_rules = RwLock::new(Arc::new(crash::load_rules(&data_dir)));
        Self {
            settings: RwLock::new(settings),
            config_path,
            data_dir,
            client: crate::engine::http_client(),
            manifest: Mutex::new(None),
            auto_source: Mutex::new(None),
            installing: Mutex::new(HashSet::new()),
            accounts: Mutex::new(accounts),
            pending_login: Mutex::new(None),
            login_cancel: Arc::new(AtomicBool::new(false)),
            java_cache: Mutex::new(None),
            games: Mutex::new(HashMap::new()),
            crash_rules,
        }
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.data_dir.join("runtime")
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
