//! 命令行冒烟测试：安装原版 + 模组加载器。
//! 用法：cargo run --example loader -- <fabric|quilt|forge|neoforge> <MC 版本> <游戏目录> [加载器版本]
//! 不指定加载器版本时用第一个稳定版（没有就用最新版）。成功时最后一行输出 `VERSION_ID <id>`。

use std::sync::Arc;
use std::time::Instant;

use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::install::{self, GameDir};
use godbbs_launcher_lib::engine::loader::{self, LoaderContext, LoaderKind};
use godbbs_launcher_lib::engine::meta::{Platform, VersionJson};
use godbbs_launcher_lib::engine::{http_client, java, mirror};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kind = match args.first().map(String::as_str) {
        Some("fabric") => LoaderKind::Fabric,
        Some("quilt") => LoaderKind::Quilt,
        Some("forge") => LoaderKind::Forge,
        Some("neoforge") => LoaderKind::NeoForge,
        _ => panic!("用法：loader <fabric|quilt|forge|neoforge> <mc> <dir> [version]"),
    };
    let mc = args.get(1).expect("缺少 MC 版本").clone();
    let game = GameDir::new(args.get(2).expect("缺少游戏目录"));
    let client = http_client();
    let source = mirror::Source::Official;
    let platform = Platform::current();
    let downloader = Downloader::new(client.clone(), source, 64);
    let quiet: Arc<dyn Fn(install::Stage, godbbs_launcher_lib::engine::download::Progress) + Send + Sync> =
        Arc::new(|stage, p| {
            if matches!(stage, install::Stage::Processor) {
                eprintln!("  processor {}/{}", p.finished_files, p.total_files);
            }
        });

    let versions = loader::list_versions(&client, source, kind, &mc).await.expect("获取加载器版本失败");
    println!("{kind:?} {mc}：共 {} 个版本，前几个：{:?}", versions.len(), versions.iter().take(3).map(|v| &v.version).collect::<Vec<_>>());
    let chosen = match args.get(3) {
        Some(v) => versions.iter().find(|x| &x.version == v || &x.id == v).expect("找不到指定版本").clone(),
        None => versions.iter().find(|v| v.stable).or(versions.first()).expect("没有可用版本").clone(),
    };
    println!("选择 {} ({})", chosen.version, chosen.id);

    let start = Instant::now();
    let manifest = install::fetch_manifest(&client, source).await.expect("版本清单");
    let entry = manifest.find(&mc).expect("原版不存在").clone();
    let r = install::install_vanilla(&downloader, &game, &entry, &platform, quiet.clone()).await.expect("原版安装失败");
    println!("原版就绪：下载 {} 跳过 {}", r.downloaded, r.skipped);

    let vanilla = VersionJson::resolve(&game.versions(), &mc).await.unwrap();
    let major = vanilla.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    let javas = java::scan(vec![game.root.join("runtime")]).await;
    let java_path = match java::pick(&javas, major, &platform) {
        Some(j) => j.path.clone(),
        None => {
            let comp = vanilla.java_version.as_ref().and_then(|j| j.component.clone()).unwrap_or_else(|| "jre-legacy".into());
            java::install_runtime(&downloader, &client, &game.root.join("runtime"), major, &comp, &platform, Arc::new(|_| {})).await.unwrap()
        }
    };

    let ctx = LoaderContext { downloader: &downloader, client: &client, game: &game, platform: &platform, java: Some(java_path), report: quiet };
    match loader::install(&ctx, kind, &mc, &chosen.id, None).await {
        Ok(id) => {
            println!("安装完成，用时 {:.1}s", start.elapsed().as_secs_f64());
            println!("VERSION_ID {id}");
        }
        Err(e) => {
            println!("安装失败：{e}");
            std::process::exit(1);
        }
    }
}
