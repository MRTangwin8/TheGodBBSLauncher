//! 启动游戏：刷新登录 → 补全文件 → 选/下载 Java → 拼参数 → 起进程 → 日志推给前端。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::java::{ensure_java, total_memory_mb};
use super::versions::INSTALL_PROGRESS_EVENT;
use super::{AppState, RunningGame};
use crate::config::{auto_memory_mb, split_args};
use crate::engine::auth::{microsoft, yggdrasil, AccountKind, LaunchAuth};
use crate::engine::auth::redact::LogRedactor;
use crate::engine::crash::{self, CrashAnalysis};
use crate::engine::download::{DownloadTask, Downloader, Progress};
use crate::engine::install::{self, GameDir, Stage};
use crate::engine::instance;
use crate::engine::launch::log4j::Log4jParser;
use crate::engine::launch::{self, Agent, LaunchOptions};
use crate::engine::meta::{AssetIndex, Platform, VersionJson};
use crate::engine::{Error, Result};

pub const GAME_LOG_EVENT: &str = "game-log";
pub const GAME_EXIT_EVENT: &str = "game-exit";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameLog {
    run_id: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameExit {
    run_id: String,
    version_id: String,
    code: Option<i32>,
    killed: bool,
    analysis: Option<CrashAnalysis>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StageProgress {
    version_id: String,
    stage: Stage,
    progress: Progress,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub run_id: String,
    pub version_id: String,
    pub player: String,
}

/// 取当前账号的启动身份，必要时刷新令牌。外置登录额外返回 (API 根地址)
async fn launch_auth(state: &AppState) -> Result<(LaunchAuth, Option<String>)> {
    let (account, secret) = {
        let store = state.accounts.lock().await;
        let acc = store.selected().cloned().ok_or_else(|| Error::Other("还没有账号，先去「个人空间」添加一个吧".into()))?;
        let secret = store.secret(&acc.id)?;
        (acc, secret)
    };
    match account.kind {
        AccountKind::Offline => Ok((
            LaunchAuth {
                name: account.name.clone(),
                uuid: account.uuid.clone(),
                access_token: account.uuid.clone(),
                user_type: "legacy",
                xuid: "0".into(),
            },
            None,
        )),
        AccountKind::Microsoft => {
            let mut secret = secret.ok_or_else(|| Error::Other("正版登录信息丢失，请重新登录".into()))?;
            let mut account = account;
            if secret.is_expired() {
                let refresh = secret.refresh_token.clone().ok_or_else(|| Error::Other("正版登录已过期，请重新登录".into()))?;
                let session = microsoft::refresh(&state.client, &refresh).await?;
                account.name = session.name;
                account.uuid = session.uuid;
                secret = session.secret;
                state.accounts.lock().await.upsert(account.clone(), Some(&secret))?;
            }
            Ok((
                LaunchAuth { name: account.name, uuid: account.uuid, access_token: secret.access_token, user_type: "msa", xuid: "0".into() },
                None,
            ))
        }
        AccountKind::Yggdrasil => {
            let api = account.server.clone().ok_or_else(|| Error::Other("外置登录缺少服务器地址".into()))?;
            let mut secret = secret.ok_or_else(|| Error::Other("外置登录信息丢失，请重新登录".into()))?;
            let mut account = account;
            if let Some(session) = yggdrasil::ensure_valid(&state.client, &api, &secret).await? {
                account.name = session.name;
                account.uuid = session.uuid;
                secret = session.secret;
                state.accounts.lock().await.upsert(account.clone(), Some(&secret))?;
            }
            Ok((
                LaunchAuth { name: account.name, uuid: account.uuid, access_token: secret.access_token, user_type: "mojang", xuid: "0".into() },
                Some(api),
            ))
        }
    }
}

#[tauri::command]
/// `server`：启动后直接进这个服务器（来自首页的快捷按钮）；留空就是普通启动
pub async fn launch_game(app: AppHandle, state: State<'_, AppState>, version_id: String, server: Option<String>) -> Result<LaunchResult> {
    let state: &AppState = &state;
    let settings = state.settings.read().await.clone();
    let platform = Platform::current();
    let game = GameDir::new(settings.game_dir.clone());

    let reporter = |stage: Stage| -> Arc<dyn Fn(Progress) + Send + Sync> {
        let (app, version_id) = (app.clone(), version_id.clone());
        Arc::new(move |progress| {
            let _ = app.emit(INSTALL_PROGRESS_EVENT, StageProgress { version_id: version_id.clone(), stage, progress });
        })
    };

    let (auth, ygg_api) = launch_auth(state).await?;
    // 记住上次启动的版本，首页默认选它
    {
        let mut s = state.settings.write().await;
        if s.last_version.as_deref() != Some(version_id.as_str()) {
            s.last_version = Some(version_id.clone());
            let _ = crate::config::save(&state.config_path, &s);
        }
    }
    let version = VersionJson::resolve(&game.versions(), &version_id).await?;

    // 快速补全：已有文件只比大小，不重算 SHA1
    let mut downloader = Downloader::new(state.client.clone(), state.source().await, settings.concurrency);
    downloader.verify_existing = false;
    let asset_index = match &version.asset_index {
        Some(idx) => {
            let path = game.asset_index(&idx.id);
            let task = DownloadTask { url: idx.url.clone(), path: path.clone(), sha1: idx.sha1.clone(), size: idx.size };
            let r = downloader.download_all(vec![task], reporter(Stage::AssetIndex)).await;
            if let Some(f) = r.failed.first() {
                return Err(Error::Other(format!("资源索引下载失败：{}", f.reason)));
            }
            Some(serde_json::from_str::<AssetIndex>(&tokio::fs::read_to_string(&path).await?)?)
        }
        None => None,
    };
    let tasks = install::collect_tasks(&game, &version, asset_index.as_ref(), &platform);
    let report = downloader.download_all(tasks, reporter(Stage::Files)).await;
    if let Some(f) = report.failed.first() {
        return Err(Error::Other(format!("{} 个文件补全失败，例如 {}：{}", report.failed.len(), f.path, f.reason)));
    }

    // Java
    let java_path = match &settings.java_path {
        Some(p) => p.clone(),
        None => ensure_java(state, &version, &downloader, reporter(Stage::Java)).await?,
    };

    let agent = match ygg_api {
        Some(api) => {
            let jar = yggdrasil::ensure_injector(&downloader, &state.client, &state.runtime_dir()).await?;
            let info = yggdrasil::resolve_server(&state.client, &api).await?;
            Some(Agent { jar, api_root: info.api_root, metadata: info.metadata })
        }
        None => None,
    };

    // 单版本设置（整合包实例）：强制隔离、最少内存、追加参数
    let inst = instance::load(&game, &version.id);
    let server_args = match server.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(addr) => {
            let mc = crate::engine::meta::version::base_minecraft_version(&game.versions(), &version_id).await?;
            launch::server::server_join_args(&mc, addr)?
        }
        None => Vec::new(),
    };
    let game_dir = instance::game_dir(&game, &version.id, settings.isolate_versions);
    let memory = settings.memory_mb.unwrap_or_else(|| auto_memory_mb(total_memory_mb()));
    let opts = LaunchOptions {
        java: java_path,
        game_dir,
        memory_mb: memory.max(inst.min_memory_mb.unwrap_or(0)),
        extra_jvm_args: split_args(&settings.jvm_args).into_iter().chain(inst.jvm_args.iter().cloned()).collect(),
        extra_game_args: split_args(&settings.game_args).into_iter().chain(inst.game_args.iter().cloned()).chain(server_args).collect(),
        resolution: settings.window_width.zip(settings.window_height),
        agent,
        auth: auth.clone(),
    };
    let prepared = launch::prepare(&game, &version, &opts, &platform).await?;

    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let mut cmd = tokio::process::Command::new(&prepared.java);
    cmd.args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| Error::Other(format!("无法启动 Java（{}）：{e}", prepared.java.display())))?;

    // 第一行日志写出启动命令，令牌打码
    let redactor = Arc::new(LogRedactor::new([auth.access_token.clone()]));
    let shown = redactor.redact(&prepared.args.join(" "));
    let _ = app.emit(
        GAME_LOG_EVENT,
        GameLog { run_id: run_id.clone(), line: format!("[神权] {} {}", prepared.java.display(), shown) },
    );

    let log: LogBuffer = Arc::new(std::sync::Mutex::new(VecDeque::new()));
    let pumps: Vec<JoinHandle<()>> = [child.stdout.take().map(boxed), child.stderr.take().map(boxed)]
        .into_iter()
        .flatten()
        .map(|stream| tokio::spawn(pump_lines(app.clone(), run_id.clone(), stream, log.clone(), redactor.clone())))
        .collect();

    let (kill_tx, kill_rx) = oneshot::channel();
    state.games.lock().await.insert(run_id.clone(), RunningGame { version_id: version_id.clone(), kill: Some(kill_tx) });
    let exit = ExitContext {
        run_id: run_id.clone(),
        version_id: version_id.clone(),
        game_root: game.root.clone(),
        game_dir: prepared.cwd.clone(),
        started: SystemTime::now() - Duration::from_secs(2),
        log,
        pumps,
        redactor,
    };
    tokio::spawn(wait_exit(app.clone(), exit, child, kill_rx));

    Ok(LaunchResult { run_id, version_id, player: auth.name })
}

fn boxed<R: AsyncRead + Send + Unpin + 'static>(r: R) -> Box<dyn AsyncRead + Send + Unpin> {
    Box::new(r)
}

/// 每个进程保留最近的日志行，用于崩溃分析
type LogBuffer = Arc<std::sync::Mutex<VecDeque<String>>>;
const LOG_KEEP: usize = 20_000;

async fn pump_lines(app: AppHandle, run_id: String, stream: Box<dyn AsyncRead + Send + Unpin>, log: LogBuffer, redactor: Arc<LogRedactor>) {
    let mut reader = BufReader::new(stream);
    let mut parser = Log4jParser::default();
    let mut buf = Vec::new();
    // 按字节读行再宽松解码，避免 GBK 之类的输出让读取中断
    while reader.read_until(b'\n', &mut buf).await.unwrap_or(0) > 0 {
        let raw = String::from_utf8_lossy(&buf).trim_end().to_string();
        buf.clear();
        for line in parser.feed(&raw) {
            let line = redactor.redact(&line);
            if let Ok(mut log) = log.lock() {
                if log.len() >= LOG_KEEP {
                    log.pop_front();
                }
                log.push_back(line.clone());
            }
            let _ = app.emit(GAME_LOG_EVENT, GameLog { run_id: run_id.clone(), line });
        }
    }
}

struct ExitContext {
    run_id: String,
    version_id: String,
    /// .minecraft 根目录（记游戏时长用）
    game_root: PathBuf,
    game_dir: PathBuf,
    started: SystemTime,
    log: LogBuffer,
    pumps: Vec<JoinHandle<()>>,
    redactor: Arc<LogRedactor>,
}

async fn wait_exit(app: AppHandle, ctx: ExitContext, mut child: tokio::process::Child, kill_rx: oneshot::Receiver<()>) {
    let (status, killed) = tokio::select! {
        s = child.wait() => (s.ok(), false),
        _ = kill_rx => {
            let _ = child.kill().await;
            (child.wait().await.ok(), true)
        }
    };
    let code = status.and_then(|s| s.code());
    if let Ok(played) = ctx.started.elapsed() {
        let started_at = ctx.started.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = instance::record_session(&GameDir::new(&ctx.game_root), &ctx.version_id, started_at, played.as_secs());
    }
    // 等日志读完再分析；游戏派生的子进程可能一直占着管道，所以设个上限
    for mut pump in ctx.pumps {
        if tokio::time::timeout(Duration::from_secs(3), &mut pump).await.is_err() {
            pump.abort();
            let _ = pump.await;
        }
    }

    use tauri::Manager;
    let state = app.try_state::<AppState>();
    if let Some(state) = &state {
        state.games.lock().await.remove(&ctx.run_id);
    }

    // 用户手动结束的不分析。退出码 0 也要看：Forge 的模组加载错误界面关掉后是正常退出的
    let files = crash::collect_crash_files(&ctx.game_dir, ctx.started);
    let clean_exit = code == Some(0) && files.is_empty();
    let analysis = match (&state, killed) {
        (Some(state), false) => {
            let rules = state.crash_rules.read().await.clone();
            let text = ctx.log.lock().map(|l| l.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();
            let redactor = ctx.redactor.clone();
            tokio::task::spawn_blocking(move || crash::analyze_run_with_redactor(&rules, &text, &files, code, &redactor))
                .await
                .ok()
                .and_then(|mut a| {
                    if clean_exit {
                        crash::keep_for_clean_exit(&mut a);
                        if a.findings.is_empty() {
                            return None;
                        }
                    }
                    Some(a)
                })
        }
        _ => None,
    };
    let _ = app.emit(GAME_EXIT_EVENT, GameExit { run_id: ctx.run_id, version_id: ctx.version_id, code, killed, analysis });
}

#[tauri::command]
pub async fn kill_game(state: State<'_, AppState>, run_id: String) -> Result<()> {
    if let Some(game) = state.games.lock().await.get_mut(&run_id) {
        if let Some(tx) = game.kill.take() {
            let _ = tx.send(());
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningInfo {
    pub run_id: String,
    pub version_id: String,
}

#[tauri::command]
pub async fn list_running(state: State<'_, AppState>) -> Result<Vec<RunningInfo>> {
    Ok(state
        .games
        .lock()
        .await
        .iter()
        .map(|(id, g)| RunningInfo { run_id: id.clone(), version_id: g.version_id.clone() })
        .collect())
}
