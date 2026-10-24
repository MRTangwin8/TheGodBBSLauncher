//! 命令行冒烟测试：离线账号启动已安装的版本，看到主菜单加载完成的日志后自动关闭。
//! 用法：cargo run --example launch -- <版本号> <游戏目录> [超时秒数]

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use godbbs_launcher_lib::engine::auth::{offline, LaunchAuth};
use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::install::GameDir;
use godbbs_launcher_lib::engine::launch::{self, LaunchOptions};
use godbbs_launcher_lib::engine::meta::{Platform, VersionJson};
use godbbs_launcher_lib::engine::{http_client, java, mirror};
use tokio::io::{AsyncBufReadExt, BufReader};

/// 这些日志出现说明资源和声音都加载好了，已经到主菜单
const READY: &[&str] = &["Sound engine started", "SoundSystem initialized", "Created: 1024x512"];

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "1.20.1".into());
    let dir = args.next().unwrap_or_else(|| "./.minecraft".into());
    let timeout: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(120);
    let game = GameDir::new(&dir);
    let platform = Platform::current();

    let version = VersionJson::resolve(&game.versions(), &id).await.expect("读取版本失败");
    let major = version.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    let runtime = game.root.join("runtime");
    let javas = java::scan(vec![runtime.clone()]).await;
    println!("找到 {} 个 Java：{:?}", javas.len(), javas.iter().map(|j| format!("{} {}", j.version, j.arch)).collect::<Vec<_>>());
    let java_path = match java::pick(&javas, major, &platform) {
        Some(j) => j.path.clone(),
        None => {
            let component = version.java_version.as_ref().and_then(|j| j.component.clone()).unwrap_or_else(|| "jre-legacy".into());
            println!("没有合适的 Java {major}，下载 Mojang {component}…");
            let client = http_client();
            let dl = Downloader::new(client.clone(), mirror::Source::Official, 64);
            java::install_runtime(&dl, &client, &runtime, &component, &platform, Arc::new(|_| {})).await.expect("下载 Java 失败")
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
        memory_mb: 2048,
        extra_jvm_args: vec![],
        extra_game_args: vec![],
        resolution: Some((854, 480)),
        agent: None,
    };
    let prepared = launch::prepare(&game, &version, &opts, &platform).await.expect("准备启动失败");
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
            while let Ok(Some(l)) = lines.next_line().await {
                let _ = tx.send(l);
            }
        });
    }
    drop(tx);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout);
    let mut tail: Vec<String> = Vec::new();
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
                    tail.push(l);
                    if tail.len() > 40 { tail.remove(0); }
                }
                None => {
                    let status = child.wait().await;
                    println!("游戏提前退出：{status:?}\n最后的日志：\n{}", tail.join("\n"));
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
