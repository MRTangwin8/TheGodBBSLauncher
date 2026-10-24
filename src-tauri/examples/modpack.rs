//! 命令行冒烟测试：导入 / 导出整合包。
//! 导入：cargo run --example modpack -- import <整合包文件> <游戏目录> <版本名>
//! 导出：cargo run --example modpack -- export <游戏目录> <版本id> <modrinth|curseforge|mcbbs> <输出文件>
//! 导入成功最后一行输出 `VERSION_ID <id>`。

use std::path::Path;
use std::sync::Arc;

use godbbs_launcher_lib::engine::download::{DownloadTask, Downloader};
use godbbs_launcher_lib::engine::install::{self, GameDir};
use godbbs_launcher_lib::engine::instance::{self, InstanceConfig};
use godbbs_launcher_lib::engine::loader::{self, LoaderContext, LoaderKind};
use godbbs_launcher_lib::engine::meta::{Platform, VersionJson};
use godbbs_launcher_lib::engine::modpack::export::{self, ExportEnv, ExportOptions};
use godbbs_launcher_lib::engine::modpack::{Modpack, PackFormat};
use godbbs_launcher_lib::engine::resources::compat::GameEnv;
use godbbs_launcher_lib::engine::resources::ResourceApi;
use godbbs_launcher_lib::engine::{http_client, java, mirror};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let client = http_client();
    let api = ResourceApi { client: client.clone(), curseforge_key: std::env::var("CF_API_KEY").ok() };
    match args.first().map(String::as_str) {
        Some("import") => import(&api, Path::new(&args[1]), GameDir::new(&args[2]), &args[3]).await,
        Some("export") => {
            let format = match args[3].as_str() {
                "curseforge" => PackFormat::CurseForge,
                "mcbbs" => PackFormat::Mcbbs,
                _ => PackFormat::Modrinth,
            };
            export_pack(&api, GameDir::new(&args[1]), &args[2], format, Path::new(&args[4])).await
        }
        _ => eprintln!("用法见文件头注释"),
    }
}

async fn import(api: &ResourceApi, path: &Path, game: GameDir, name: &str) {
    let pack = Modpack::open(path).expect("打开整合包失败");
    let info = pack.info.clone();
    println!("{:?} 整合包「{}」{}：MC {}，{:?} {:?}，{} 个文件需下载，{} 个自带文件", info.format, info.name, info.version, info.mc, info.loader, info.loader_version, info.file_count, info.override_count);

    let platform = Platform::current();
    let downloader = Downloader::new(api.client.clone(), mirror::Source::Official, 64);
    let quiet = Arc::new(|_, _| {});
    let manifest = install::fetch_manifest(&api.client, mirror::Source::Official).await.unwrap();
    let entry = manifest.find(&info.mc).expect("原版不存在").clone();
    install::install_vanilla(&downloader, &game, &entry, &platform, quiet.clone()).await.unwrap();

    let id = match (info.loader, &info.loader_version) {
        (Some(kind), Some(v)) => {
            let list = loader::list_versions(&api.client, mirror::Source::Official, kind, &info.mc).await.unwrap();
            let lid = list.iter().find(|x| &x.version == v || &x.id == v).map(|x| x.id.clone()).unwrap_or_else(|| v.clone());
            let java = if matches!(kind, LoaderKind::Forge | LoaderKind::NeoForge) {
                let vanilla = VersionJson::resolve(&game.versions(), &info.mc).await.unwrap();
                let major = vanilla.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
                java::pick(&java::scan(vec![game.root.join("runtime")]).await, major, &platform).map(|j| j.path.clone())
            } else {
                None
            };
            let ctx = LoaderContext { downloader: &downloader, client: &api.client, game: &game, platform: &platform, java, report: quiet };
            loader::install(&ctx, kind, &info.mc, &lid, Some(name)).await.expect("加载器安装失败")
        }
        _ => {
            install::create_inheriting_version(&game, name, &info.mc).unwrap();
            name.to_string()
        }
    };
    instance::save(&game, &id, &InstanceConfig { isolated: true, min_memory_mb: info.min_memory_mb, ..Default::default() }).unwrap();

    let dir = game.versions().join(&id);
    let (files, warnings) = pack.files(api).await.expect("解析文件失败");
    for w in &warnings {
        println!("  警告：{w}");
    }
    let tasks: Vec<DownloadTask> = files
        .into_iter()
        .map(|f| DownloadTask { path: f.path.split('/').fold(dir.clone(), |p, s| p.join(s)), url: f.url, sha1: f.sha1, size: (f.size > 0).then_some(f.size) })
        .collect();
    let r = downloader.download_all(tasks, Arc::new(|_| {})).await;
    println!("下载 {} 跳过 {} 失败 {}", r.downloaded, r.skipped, r.failed.len());
    for f in r.failed.iter().take(5) {
        println!("  失败 {}：{}", f.path, f.reason);
    }
    let n = pack.extract_overrides(&dir).unwrap();
    println!("解压自带文件 {n} 个");
    println!("VERSION_ID {id}");
}

async fn export_pack(api: &ResourceApi, game: GameDir, id: &str, format: PackFormat, out: &Path) {
    let env = GameEnv::detect(&game, id).await.unwrap();
    let dir = game.versions().join(id);
    let include: Vec<String> = export::candidates(&dir, id).into_iter().filter(|e| e.default_on).map(|e| e.name).collect();
    println!("导出条目：{include:?}");
    let opts = ExportOptions { format, name: format!("{id} 导出"), version: "1.0".into(), author: "神权论坛".into(), summary: String::new(), include };
    let eenv = ExportEnv { mc: env.mc, loader: env.loader, loader_version: env.loader_version, min_memory_mb: Some(4096), jvm_args: vec![], game_args: vec![] };
    let s = export::export(api, &dir, &opts, &eenv, out, Arc::new(|_| {})).await.unwrap();
    println!("导出完成：{}，引用 {} 个，打包 {} 个，大小 {} KB", s.path, s.linked, s.bundled, std::fs::metadata(out).map(|m| m.len() / 1024).unwrap_or(0));
}
