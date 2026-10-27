//! 命令行冒烟测试：离线账号启动已安装的版本，看到主菜单加载完成的日志后自动关闭。
//! 用法：cargo run --example launch -- <版本号> <游戏目录> [超时秒数]

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use godbbs_launcher_lib::engine::auth::{offline, LaunchAuth};
use godbbs_launcher_lib::engine::crash::{self, RuleSet, BUILTIN_RULES};
use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::install::GameDir;
use godbbs_launcher_lib::engine::launch::log4j::Log4jParser;
use godbbs_launcher_lib::engine::launch::{self, LaunchOptions};
use godbbs_launcher_lib::engine::meta::{Platform, VersionJson};
use godbbs_launcher_lib::engine::{http_client, java, mirror};
use tokio::io::{AsyncBufReadExt, BufReader};

/// 这些日志出现说明资源和声音都加载好了，已经到主菜单
const READY: &[&str] = &["Sound engine started", "SoundSystem initialized", "Created: 1024x512"];

#[tokio::main]
async fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| all.iter().position(|a| a == name).and_then(|i| all.get(i + 1)).cloned();
    let mut positional: Vec<String> = Vec::new();
    let mut skip = false;
    for a in &all {
        if skip {
            skip = false;
        } else if a.starts_with("--") {
            skip = true;
        } else {
            positional.push(a.clone());
        }
    }
    let id = positional.first().cloned().unwrap_or_else(|| "1.20.1".into());
    let dir = positional.get(1).cloned().unwrap_or_else(|| "./.minecraft".into());
    let timeout: u64 = positional.get(2).and_then(|s| s.parse().ok()).unwrap_or(120);
    let game = GameDir::new(&dir);
    let platform = Platform::current();

    let version = VersionJson::resolve(&game.versions(), &id).await.expect("读取版本失败");
    let major = version.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    let runtime = game.root.join("runtime");
    let javas = java::scan(vec![runtime.clone()]).await;
    println!("找到 {} 个 Java：{:?}", javas.len(), javas.iter().map(|j| format!("{} {}", j.version, j.arch)).collect::<Vec<_>>());
    let java_path = match flag("--java").map(std::path::PathBuf::from).or_else(|| java::pick(&javas, major, &platform).map(|j| j.path.clone())) {
        Some(p) => p,
        None => {
            let component = version.java_version.as_ref().and_then(|j| j.component.clone()).unwrap_or_else(|| "jre-legacy".into());
            println!("没有合适的 Java {major}，下载 Mojang {component}…");
            let client = http_client();
            let dl = Downloader::new(client.clone(), mirror::Source::Official, 64);
            java::install_runtime(&dl, &client, &runtime, major, &component, &platform, Arc::new(|_| {})).await.expect("下载 Java 失败")
        }
    };
    println!("使用 Java：{}", java_path.display());

    let name = "GodBBS_Test";
    let auth = LaunchAuth {
        name: name.into(),
        uuid: offline::offline_uuid(name),
        access_token: offline::offline_uuid(name),
        user_type: "legacy",
        xuid: "0".into(),
    };
    let opts = LaunchOptions {
        java: java_path,
        game_dir: game.versions().join(&id),
        auth,
        memory_mb: flag("--memory").and_then(|m| m.parse().ok()).unwrap_or(2048),
        extra_jvm_args: flag("--jvm-arg").into_iter().collect(),
        extra_game_args: vec![],
        resolution: Some((854, 480)),
        agent: None,
    };
    let prepared = launch::prepare(&game, &version, &opts, &platform).await.expect("准备启动失败");
    let started = std::time::SystemTime::now() - Duration::from_secs(2);
    let mut child = tokio::process::Command::new(&prepared.java)
        .args(&prepared.args)
        .current_dir(&prepared.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("启动 Java 失败");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    for s in [child.stdout.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
              child.stderr.take().map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Send + Unpin>)].into_iter().flatten() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(s).lines();
            let mut parser = Log4jParser::default();
            while let Ok(Some(l)) = lines.next_line().await {
                for line in parser.feed(&l) {
                    let _ = tx.send(line);
                }
            }
        });
    }
    drop(tx);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout);
    let mut tail: Vec<String> = Vec::new();
    let mut full: Vec<String> = Vec::new();
    loop {
        tokio::select! {
            line = rx.recv() => match line {
                Some(l) => {
                    if READY.iter().any(|r| l.contains(r)) {
                        println!("✔ 进入主菜单：{l}");
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        let _ = child.kill().await;
                        println!("RESULT OK {id}");
                        return;
                    }
                    full.push(l.clone());
                    tail.push(l);
                    if tail.len() > 40 { tail.remove(0); }
                }
                None => {
                    let status = child.wait().await.ok();
                    let code = status.and_then(|s| s.code());
                    println!("游戏提前退出，退出码 {code:?}");
                    let rules = RuleSet::parse(BUILTIN_RULES).unwrap();
                    let files = crash::collect_crash_files(&prepared.cwd, started);
                    let analysis = crash::analyze_run(&rules, &full.join("\n"), &files, code);
                    for f in &analysis.findings {
                        println!("DIAG [{:?}] {} —— {}", f.severity, f.title, f.description);
                    }
                    if analysis.findings.is_empty() {
                        println!("DIAG 未识别。最后的日志：\n{}", tail.join("\n"));
                    }
                    println!("RESULT FAIL {id}");
                    std::process::exit(1);
                }
            },
            _ = tokio::time::sleep_until(deadline) => {
                let _ = child.kill().await;
                println!("超时。最后的日志：\n{}", tail.join("\n"));
                println!("RESULT TIMEOUT {id}");
                std::process::exit(1);
            }
        }
    }
}
