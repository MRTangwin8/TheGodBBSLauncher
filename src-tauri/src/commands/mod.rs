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
pub mod multiplayer;
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
use crate::engine::cancel::{self, CancelToken};
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
    /// 当前这次微软登录的取消标志；每次重新开始都换一个新的，并把上一次的置位，免得旧的轮询还在跑
    pub login_cancel: Mutex<Arc<AtomicBool>>,
    pub java_cache: Mutex<Option<Vec<JavaInstall>>>,
    pub games: Mutex<HashMap<String, RunningGame>>,
    pub crash_rules: RwLock<Arc<RuleSet>>,
    /// 正在迁移游戏目录：这期间不能启动游戏、安装任何东西
    pub migrating: AtomicBool,
    pub migrate_cancel: AtomicBool,
    /// 正在进行、会写游戏目录的任务数（安装、导入、下载资源、启动前的准备……），见 `begin_task`
    pub busy: AtomicUsize,
    /// 正在进行、可以取消的任务：任务 id（界面上的 jobId）→ 取消标志，见 `cancellable`
    pub jobs: std::sync::Mutex<HashMap<String, CancelToken>>,
    /// 联机（陶瓦）进程和状态轮询，用到时才启动
    pub multiplayer: Mutex<Option<multiplayer::MpSession>>,
}

/// 商店版（MSIX）往 AppData 写的文件会被重定向到应用私有目录：启动器自己读得到，
/// 但 java.exe 按真实路径找 jli.dll 等文件时找不到（报「由于找不到 jli.dll，无法继续执行代码」）。
/// 所以商店版和游戏目录一样放到用户主目录下
fn runtime_dir_for(store: bool, data_dir: &std::path::Path, home: Option<PathBuf>) -> PathBuf {
    match home {
        Some(home) if store => home.join("GodBBSLauncher").join("runtime"),
        _ => data_dir.join("runtime"),
    }
}

pub struct TaskGuard<'a>(&'a AppState);

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 一个可取消的任务；守卫活着期间界面可以按 id 取消它
pub struct JobGuard<'a> {
    state: &'a AppState,
    id: String,
    token: CancelToken,
}

impl JobGuard<'_> {
    /// 在取消标志下运行任务。被取消后任务多半以「下载失败」之类的错误收尾，这里统一换成「已取消」；
    /// 已经成功的结果原样返回（比如游戏刚好启动了，不能当成失败）
    pub async fn run<T>(&self, fut: impl std::future::Future<Output = crate::engine::Result<T>>) -> crate::engine::Result<T> {
        let r = cancel::scope(self.token.clone(), fut).await;
        match r {
            Err(_) if self.token.is_cancelled() => Err(cancel::error()),
            r => r,
        }
    }
}

impl JobGuard<'_> {
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        let mut jobs = self.state.jobs.lock().unwrap_or_else(|e| e.into_inner());
        if jobs.get(&self.id).is_some_and(|t| t.same(&self.token)) {
            jobs.remove(&self.id);
        }
    }
}

impl AppState {
    /// 登记一个可取消的任务（同 id 的旧任务被顶掉，不会误取消新的）
    pub fn cancellable(&self, id: &str) -> JobGuard<'_> {
        let token = CancelToken::default();
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), token.clone());
        JobGuard { state: self, id: id.to_string(), token }
    }

    /// 取消任务；任务不存在（已经结束）时什么也不做
    pub fn cancel_job(&self, id: &str) {
        if let Some(t) = self.jobs.lock().unwrap_or_else(|e| e.into_inner()).get(id) {
            t.cancel();
        }
    }

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
            login_cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
            java_cache: Mutex::new(None),
            games: Mutex::new(HashMap::new()),
            crash_rules,
            migrating: AtomicBool::new(false),
            migrate_cancel: AtomicBool::new(false),
            busy: AtomicUsize::new(0),
            jobs: std::sync::Mutex::new(HashMap::new()),
            multiplayer: Mutex::new(None),
        }
    }

    /// 启动 / 安装类命令开头调用：迁移游戏目录时拒绝
    pub fn ensure_not_migrating(&self) -> crate::engine::Result<()> {
        if self.migrating.load(Ordering::SeqCst) {
            return Err(crate::engine::Error::coded("migrating", &[], "正在迁移游戏目录，等迁移完成再操作"));
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

    /// Java runtime 和 authlib-injector 放这里
    pub fn runtime_dir(&self) -> PathBuf {
        runtime_dir_for(crate::edition::STORE, &self.data_dir, dirs::home_dir())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_edition_keeps_runtime_out_of_appdata() {
        let data = PathBuf::from("C:/Users/a/AppData/Roaming/co.mcbbs.godbbs");
        let home = Some(PathBuf::from("C:/Users/a"));
        assert_eq!(runtime_dir_for(false, &data, home.clone()), data.join("runtime"));
        assert_eq!(runtime_dir_for(true, &data, home), PathBuf::from("C:/Users/a").join("GodBBSLauncher").join("runtime"));
        assert_eq!(runtime_dir_for(true, &data, None), data.join("runtime"), "找不到主目录时退回数据目录");
    }
}
