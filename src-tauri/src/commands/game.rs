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
    /// 一批日志行。游戏疯狂刷日志时（崩溃时成千上万行堆栈）一行发一个事件会把界面卡死，所以攒一小段时间一起发
    lines: Vec<String>,
}

/// 多久把攒下的日志推给界面一次
const LOG_FLUSH: Duration = Duration::from_millis(100);
/// 一批最多推这么多行（界面每个进程只留 5000 行，再多也是白推）；超出的只留最后这些
const LOG_BATCH_MAX: usize = 5000;

/// 还没推给界面的日志行
type Pending = Arc<std::sync::Mutex<Vec<String>>>;

/// 取出攒下的日志；太多时丢掉前面的，并留一行说明
fn take_batch(pending: &Pending) -> Vec<String> {
    let mut lines = match pending.lock() {
        Ok(mut p) => std::mem::take(&mut *p),
        Err(_) => return Vec::new(),
    };
    if lines.len() > LOG_BATCH_MAX {
        let dropped = lines.len() - LOG_BATCH_MAX;
        lines.drain(..dropped);
        lines.insert(0, format!("[神权] 日志刷得太快，这里略过了 {dropped} 行（完整内容见游戏目录的 logs/latest.log）"));
    }
    lines
}

fn flush_log(app: &AppHandle, run_id: &str, pending: &Pending) {
    let lines = take_batch(pending);
    if !lines.is_empty() {
        let _ = app.emit(GAME_LOG_EVENT, GameLog { run_id: run_id.to_string(), lines });
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameExit {
    run_id: String,
    version_id: String,
    code: Option<i32>,
    killed: bool,
    /// 算正常退出：退出码 0，或者退出码不是 0 但日志显示游戏是自己正常关闭的、也没查出问题
    normal: bool,
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
        let acc = store.selected().cloned().ok_or_else(|| Error::coded("launch.noAccount", &[], "还没有账号，先去「个人空间」添加一个吧"))?;
        crate::edition::check_account_kind(acc.kind)?;
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
            let mut secret = secret.ok_or_else(|| Error::coded("auth.msLost", &[], "正版登录信息丢失，请重新登录"))?;
            let mut account = account;
            if secret.is_expired() {
                let refresh = secret.refresh_token.clone().ok_or_else(|| Error::coded("auth.msExpired", &[], "正版登录已过期，请重新登录"))?;
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
            let api = account.server.clone().ok_or_else(|| Error::coded("auth.yggNoServer", &[], "外置登录缺少服务器地址"))?;
            let mut secret = secret.ok_or_else(|| Error::coded("auth.yggLost", &[], "外置登录信息丢失，请重新登录"))?;
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
/// `server`：启动后直接进这个服务器（来自首页的快捷按钮）；留空就是普通启动。
/// `game_language`：这个版本第一次启动（还没有 options.txt）时给游戏设的语言，如 `zh_cn`
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
    server: Option<String>,
    game_language: Option<String>,
) -> Result<LaunchResult> {
    let state: &AppState = &state;
    // 补文件、下 Java 的阶段可以取消；游戏一旦启动就不受影响（见 JobGuard::run）
    let job = state.cancellable(&version_id);
    job.run(prepare_and_launch(app, state, version_id.clone(), server, game_language)).await
}

async fn prepare_and_launch(app: AppHandle, state: &AppState, version_id: String, server: Option<String>, game_language: Option<String>) -> Result<LaunchResult> {
    // 补文件、下 Java 这段时间游戏还没登记进 games，靠它挡住迁移；返回前游戏已经登记
    let _task = state.begin_task()?;
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
                return Err(Error::coded("launch.assetIndex", &[("reason", &f.reason)], format!("资源索引下载失败：{}", f.reason)));
            }
            Some(serde_json::from_str::<AssetIndex>(&tokio::fs::read_to_string(&path).await?)?)
        }
        None => None,
    };
    let tasks = install::collect_tasks(&game, &version, asset_index.as_ref(), &platform);
    let report = downloader.download_all(tasks, reporter(Stage::Files)).await;
    if let Some(f) = report.failed.first() {
        return Err(Error::coded(
            "launch.filesFailed",
            &[("n", &report.failed.len()), ("path", &f.path), ("reason", &f.reason)],
            format!("{} 个文件补全失败，例如 {}：{}", report.failed.len(), f.path, f.reason),
        ));
    }
    // 启动前的补全把整条继承链的文件都下齐了，之前没装完的标记可以去掉
    install::mark_complete(&game, &version_id);
    if let Ok(base) = crate::engine::meta::version::base_minecraft_version(&game.versions(), &version_id).await {
        install::mark_complete(&game, &base);
    }

    // 单版本设置：专用 Java / 内存、强制隔离、最少内存、追加参数
    let inst = instance::load(&game, &version.id);

    // Java：版本自己的 → 全局设置的 → 自动选择
    let java_path = match inst.java_path.as_ref().or(settings.java_path.as_ref()) {
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

    let server_args = match server.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(addr) => {
            let mc = crate::engine::meta::version::base_minecraft_version(&game.versions(), &version_id).await?;
            launch::server::server_join_args(&mc, addr)?
        }
        None => Vec::new(),
    };
    let game_dir = instance::game_dir(&game, &version.id, settings.isolate_versions);
    if let Some(lang) = game_language.as_deref() {
        // 写不进去也不影响启动，游戏会用默认的英文
        let _ = launch::options::seed_language(&game_dir, lang, version.release_time.as_deref());
    }
    let memory = inst.memory_mb.or(settings.memory_mb).unwrap_or_else(|| auto_memory_mb(total_memory_mb()));
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
    // 准备期间点了取消：到这里为止什么都还没启动，直接停
    crate::engine::cancel::check()?;

    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let mut cmd = tokio::process::Command::new(&prepared.java);
    cmd.args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut child = cmd.spawn().map_err(|e| {
        let java = prepared.java.display();
        Error::coded("launch.javaSpawn", &[("java", &java), ("detail", &e)], format!("无法启动 Java（{java}）：{e}"))
    })?;

    // 第一行日志写出启动命令，令牌打码
    let redactor = Arc::new(LogRedactor::new([auth.access_token.clone()]));
    let shown = redactor.redact(&prepared.args.join(" "));
    let _ = app.emit(
        GAME_LOG_EVENT,
        GameLog { run_id: run_id.clone(), lines: vec![format!("[神权] {} {}", prepared.java.display(), shown)] },
    );

    let log: LogBuffer = Arc::new(std::sync::Mutex::new(VecDeque::new()));
    let pending: Pending = Arc::new(std::sync::Mutex::new(Vec::new()));
    let pumps: Vec<JoinHandle<()>> = [child.stdout.take().map(boxed), child.stderr.take().map(boxed)]
        .into_iter()
        .flatten()
        .map(|stream| tokio::spawn(pump_lines(stream, log.clone(), pending.clone(), redactor.clone())))
        .collect();
    // 定时把攒下的日志推给界面，游戏退出后由 wait_exit 停掉
    let flusher = {
        let (app, run_id, pending) = (app.clone(), run_id.clone(), pending.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(LOG_FLUSH).await;
                flush_log(&app, &run_id, &pending);
            }
        })
    };

    let (kill_tx, kill_rx) = oneshot::channel();
    state.games.lock().await.insert(run_id.clone(), RunningGame { version_id: version_id.clone(), kill: Some(kill_tx) });
    let exit = ExitContext {
        run_id: run_id.clone(),
        version_id: version_id.clone(),
        game_root: game.root.clone(),
        game_dir: prepared.cwd.clone(),
        started: SystemTime::now() - Duration::from_secs(2),
        log,
        pending,
        pumps,
        flusher,
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

async fn pump_lines(stream: Box<dyn AsyncRead + Send + Unpin>, log: LogBuffer, pending: Pending, redactor: Arc<LogRedactor>) {
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
            if let Ok(mut p) = pending.lock() {
                p.push(line);
            }
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
    pending: Pending,
    pumps: Vec<JoinHandle<()>>,
    flusher: JoinHandle<()>,
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

    // 剩下的日志先推完，保证界面先收到全部日志、再收到退出事件
    ctx.flusher.abort();
    let _ = ctx.flusher.await;
    flush_log(&app, &ctx.run_id, &ctx.pending);

    use tauri::Manager;
    let state = app.try_state::<AppState>();
    if let Some(state) = &state {
        state.games.lock().await.remove(&ctx.run_id);
    }

    // 用户手动结束的不分析。退出码 0 也要看：Forge 的模组加载错误界面关掉后是正常退出的
    let files = crash::collect_crash_files(&ctx.game_dir, ctx.started);
    let text = ctx.log.lock().map(|l| l.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_default();
    // 退出码不是 0 不一定是崩溃：不少模组会让正常关闭的游戏带着非 0 的退出码。
    // 日志里有正常关闭的记录、又没有崩溃报告，就按正常退出看
    let stopped = files.is_empty() && crash::stopped_normally(&text);
    let clean_exit = (code == Some(0) || stopped) && files.is_empty();
    let analysis = match (&state, killed) {
        (Some(state), false) => {
            let rules = state.crash_rules.read().await.clone();
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
    let normal = code == Some(0) || (!killed && stopped && analysis.is_none());
    let _ = app.emit(GAME_EXIT_EVENT, GameExit { run_id: ctx.run_id, version_id: ctx.version_id, code, killed, normal, analysis });
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

/// 桌面快捷方式带来的「启动这个版本」请求（只取一次）
#[tauri::command]
pub async fn take_pending_launch(state: State<'_, AppState>) -> Result<Option<String>> {
    Ok(state.pending_launch.lock().unwrap_or_else(|e| e.into_inner()).take())
}

/// 在桌面建一个直接启动这个版本的快捷方式，返回文件路径
#[tauri::command]
pub async fn create_shortcut(state: State<'_, AppState>, version_id: String) -> Result<String> {
    if crate::edition::STORE {
        // 商店版程序在 WindowsApps 里，不能用路径直接启动
        return Err(Error::coded("shortcut.store", &[], "微软商店版暂时不支持创建桌面快捷方式"));
    }
    let game = GameDir::new(state.settings.read().await.game_dir.clone());
    if !game.version_json(&version_id).is_file() {
        return Err(Error::VersionNotFound(version_id));
    }
    // AppImage 运行时 current_exe 是临时挂载点里的程序，要用 AppImage 文件本身
    let exe = std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()).map_or_else(std::env::current_exe, Ok)?;
    let desktop = dirs::desktop_dir().ok_or_else(|| Error::Other("找不到桌面文件夹".into()))?;
    let path = tokio::task::spawn_blocking(move || crate::engine::shortcut::create(&desktop, &exe, &version_id))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_batches_are_drained_and_capped() {
        let pending: Pending = Arc::new(std::sync::Mutex::new(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(take_batch(&pending), ["a", "b"]);
        assert!(take_batch(&pending).is_empty(), "取走之后就空了");

        *pending.lock().unwrap() = (0..LOG_BATCH_MAX + 10).map(|i| format!("line {i}")).collect();
        let batch = take_batch(&pending);
        assert_eq!(batch.len(), LOG_BATCH_MAX + 1, "多一行说明");
        assert!(batch[0].contains("略过了 10 行"));
        assert_eq!(batch[1], "line 10");
        assert_eq!(batch.last().unwrap(), &format!("line {}", LOG_BATCH_MAX + 9));
    }
}
