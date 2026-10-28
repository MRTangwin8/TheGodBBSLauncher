//! 命令行冒烟测试：检查一个 mods 文件夹里的模组有没有更新（先问 Modrinth，认不出的再问 CurseForge）。
//! 用法：cargo run --example updates -- <mods 文件夹> <游戏版本，如 1.20.1> [fabric|quilt|forge|neoforge]
//! 只检查不下载；设了环境变量 CF_API_KEY 时 CurseForge 走官方接口，否则走 MCIM 镜像。

use std::path::PathBuf;

use godbbs_launcher_lib::engine::http_client;
use godbbs_launcher_lib::engine::loader::LoaderKind;
use godbbs_launcher_lib::engine::resources::local::check_updates;
use godbbs_launcher_lib::engine::resources::ResourceApi;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().expect("缺少 mods 文件夹"));
    let mc = args.get(1).expect("缺少游戏版本").clone();
    let loader = match args.get(2).map(String::as_str) {
        Some("fabric") => Some(LoaderKind::Fabric),
        Some("quilt") => Some(LoaderKind::Quilt),
        Some("forge") => Some(LoaderKind::Forge),
        Some("neoforge") => Some(LoaderKind::NeoForge),
        _ => None,
    };
    let api = ResourceApi { client: http_client(), curseforge_key: std::env::var("CF_API_KEY").ok() };
    let r = check_updates(&api, &dir, Some(&mc), loader).await.expect("检查失败");
    println!("可更新 {} 个，认不出 {} 个", r.updates.len(), r.unknown);
    for u in r.updates {
        println!("  [{:?}] {}：{} → {}（{}）", u.latest.source, u.title, u.current, u.latest.version_number, u.latest.file.map(|f| f.filename).unwrap_or_default());
    }
}
