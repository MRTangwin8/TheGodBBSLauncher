//! 命令行冒烟测试联机：下载陶瓦 → 启动 → 开房间（或加入）→ 打印状态 → 退出。
//! 用法：cargo run --example multiplayer -- <工作目录> [房间码]
//! 不带房间码时开房间：先在游戏里进存档、点「对局域网开放」，陶瓦扫描到后会打印房间码。

use std::sync::Arc;
use std::time::Duration;

use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::meta::Platform;
use godbbs_launcher_lib::engine::mirror::Source;
use godbbs_launcher_lib::engine::multiplayer::{api, process, release, room};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let work = std::path::PathBuf::from(&args[0]);
    let platform = Platform::current();
    let runtime = work.join("runtime");
    let exe = match release::installed_exe(&runtime, &platform) {
        Some(exe) => exe,
        None => {
            let downloader = Downloader::new(godbbs_launcher_lib::engine::http_client(), Source::Official, 1);
            release::install(&downloader, &runtime, &platform, Arc::new(|_| {})).await.expect("下载失败")
        }
    };
    println!("陶瓦：{}", exe.display());
    let client = api::client();
    let p = process::start(&exe, &work.join("terracotta"), client.clone()).await.expect("启动失败");
    println!("端口 {}，归我们管：{}", p.port, p.owned);
    let a = api::Api::new(client, p.port);
    println!("版本：{}", a.meta().await.unwrap());
    match args.get(1) {
        Some(code) => a.join(&room::normalize(code).expect("房间码不对"), Some("SmokeTest")).await.unwrap(),
        None => a.host(Some("SmokeTest")).await.unwrap(),
    }
    let mut last = None;
    for _ in 0..60 {
        let s = a.state().await.unwrap();
        if last != Some(s.index) {
            println!("{s:?}");
            last = Some(s.index);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    a.idle().await.unwrap();
    println!("回到待机：{:?}", a.state().await.unwrap().state);
    p.stop(&a).await;
    println!("已退出");
}
