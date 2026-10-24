//! 命令行冒烟测试：不开界面，直接安装一个原版版本。
//! 用法：cargo run --example install -- <版本号> <游戏目录> [official|bmclapi]

use std::sync::Arc;
use std::time::Instant;

use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::install::{self, GameDir};
use godbbs_launcher_lib::engine::meta::Platform;
use godbbs_launcher_lib::engine::{http_client, mirror};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "1.20.1".into());
    let dir = args.next().unwrap_or_else(|| "./.minecraft".into());
    let client = http_client();
    let source = match args.next().as_deref() {
        Some("official") => mirror::Source::Official,
        Some("bmclapi") => mirror::Source::Bmclapi,
        _ => {
            let r = mirror::probe(&client).await;
            println!("测速：{r:?}");
            mirror::fastest(&r)
        }
    };
    println!("使用下载源 {source:?}，安装 {id} 到 {dir}");

    let manifest = install::fetch_manifest(&client, source).await.expect("获取版本清单失败");
    let entry = manifest.find(&id).expect("版本不存在").clone();
    let downloader = Downloader::new(client, source, 64);
    let start = Instant::now();
    let report = install::install_vanilla(
        &downloader,
        &GameDir::new(dir),
        &entry,
        &Platform::current(),
        Arc::new(|stage, p| {
            if p.speed == 0 && p.finished_files + p.failed_files < p.total_files {
                return;
            }
            eprintln!(
                "{stage:?} {}/{} 文件（跳过 {}，失败 {}） {:.1}/{:.1} MB  {:.2} MB/s",
                p.finished_files, p.total_files, p.skipped_files, p.failed_files,
                p.downloaded_bytes as f64 / 1e6, p.total_bytes as f64 / 1e6, p.speed as f64 / 1e6
            );
        }),
    )
    .await
    .expect("安装失败");
    println!(
        "完成：共 {} 个文件，下载 {}，跳过 {}，失败 {}，用时 {:.1}s",
        report.total, report.downloaded, report.skipped, report.failed.len(), start.elapsed().as_secs_f64()
    );
    for f in report.failed.iter().take(10) {
        println!("  失败 {}：{}", f.path, f.reason);
    }
}
