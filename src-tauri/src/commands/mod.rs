//! 前端 IPC。请求/响应走 command，下载进度和游戏日志走 event。

pub mod accounts;
pub mod announce;
pub mod buttons;
pub mod crash;
pub mod game;
pub mod gamedir;
pub mod instance;
pub mod java;
pub mod loaders;
pub mod modpack;
pub mod resources;
pub mod settings;
pub mod versions;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    /// 正在迁移游戏目录：这期间不能启动游戏、安装任何东西
    pub migrating: AtomicBool,
    pub migrate_cancel: AtomicBool,
    /// 正在进行、会写游戏目录的任务数（安装、导入、下载资源、启动前的准备……），见 `begin_task`
    pub busy: AtomicUsize,
}

pub struct TaskGuard<'a>(&'a AppState);

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::SeqCst);
    }
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
            migrating: AtomicBool::new(false),
            migrate_cancel: AtomicBool::new(false),
            busy: AtomicUsize::new(0),
        }
    }

    /// 启动 / 安装类命令开头调用：迁移游戏目录时拒绝
    pub fn ensure_not_migrating(&self) -> crate::engine::Result<()> {
        if self.migrating.load(Ordering::SeqCst) {
            return Err(crate::engine::Error::Other("正在迁移游戏目录，等迁移完成再操作".into()));
        }
        Ok(())
    }

    /// 会往游戏目录写东西的长任务开头调用；返回的守卫活着期间，迁移不会开始。
    /// 这里先登记再看迁移标志，迁移那边先立标志再看登记数，两边不会同时放行。
    pub fn begin_task(&self) -> crate::engine::Result<TaskGuard<'_>> {
        self.busy.fetch_add(1, Ordering::SeqCst);
        let guard = TaskGuard(self);
        self.ensure_not_migrating()?;
        Ok(guard)
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
