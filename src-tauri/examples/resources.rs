//! 命令行冒烟测试：搜索资源 → 选版本 → 解析前置 → 下载。
//! 用法：cargo run --example resources -- <modrinth|curseforge> <搜索词> <游戏版本id> <游戏目录>
//! 安装到 versions/<游戏版本id>/mods（按版本隔离）。

use std::sync::Arc;

use godbbs_launcher_lib::engine::download::Downloader;
use godbbs_launcher_lib::engine::install::GameDir;
use godbbs_launcher_lib::engine::loader;
use godbbs_launcher_lib::engine::meta::version::base_minecraft_version;
use godbbs_launcher_lib::engine::meta::VersionJson;
use godbbs_launcher_lib::engine::resources::compat::{GameEnv, RuntimeJar};
use godbbs_launcher_lib::engine::resources::resolve::{self, Checker, Target};
use godbbs_launcher_lib::engine::resources::{best_version, ResourceApi, ResourceKind, ResourceSource, SearchQuery, SortBy};
use godbbs_launcher_lib::engine::{http_client, mirror};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = match args.first().map(String::as_str) {
        Some("curseforge") => ResourceSource::CurseForge,
        _ => ResourceSource::Modrinth,
    };
    let query = args.get(1).cloned().unwrap_or_else(|| "sodium extra".into());
    let version_id = args.get(2).expect("缺少游戏版本 id").clone();
    let game = GameDir::new(args.get(3).expect("缺少游戏目录"));

    let merged = VersionJson::resolve(&game.versions(), &version_id).await.expect("版本不存在");
    let loader = loader::detect(&merged);
    let mc = base_minecraft_version(&game.versions(), &version_id).await.unwrap();
    println!("目标：{version_id}（MC {mc}，加载器 {loader:?}）");

    let api = ResourceApi { client: http_client(), curseforge_key: std::env::var("CF_API_KEY").ok() };
    let q = SearchQuery { query, kind: ResourceKind::Mod, game_version: Some(mc.clone()), loader, sort: SortBy::Relevance, offset: 0, limit: 5 };
    let page = api.search(source, &q).await.expect("搜索失败");
    println!("搜索到 {} 个，前几个：{:?}", page.total, page.hits.iter().map(|h| &h.title).collect::<Vec<_>>());
    let hit = page.hits.first().expect("没有结果");

    let versions = api.versions(source, &hit.id, ResourceKind::Mod, Some(&mc), loader).await.expect("版本列表失败");
    let chosen = best_version(&versions).expect("没有兼容版本").clone();
    println!("选择 {} {}（{} 个兼容版本）", hit.title, chosen.version_number, versions.len());

    let dir = game.versions().join(&version_id).join("mods");
    let target = Target { dir: dir.clone(), kind: ResourceKind::Mod, game_version: Some(mc), loader };
    let env = GameEnv::detect(&game, &version_id).await.unwrap();
    let runtime = env.runtime_jar.as_deref().and_then(RuntimeJar::open).map(Arc::new);
    let downloader = Downloader::new(api.client.clone(), mirror::Source::Official, 16)
        .with_curseforge_key(api.curseforge_key.as_deref());
    let checker = Checker { env, runtime, cache_dir: game.root.join(".cache"), downloader: downloader.clone() };
    let plan = resolve::plan(&api, &target, chosen, Some(&checker)).await.expect("解析失败");
    for item in &plan.items {
        println!(
            "PLAN {} {} -> {}{}",
            item.title,
            item.version_number,
            item.file.filename,
            item.required_by.as_ref().map(|p| format!("（{p} 的前置）")).unwrap_or_default()
        );
        if let Some(c) = &item.compat {
            println!("     兼容性 {:?} {:?} {:?}", c.status, c.problems, c.notes);
        }
        if let Some(n) = &item.note {
            println!("     说明：{n}");
        }
        if let Some(a) = &item.alternative {
            println!("     推荐改用：{}", a.version_number);
        }
    }
    println!("已安装：{:?} 未解决：{:?} 可选：{:?} 冲突：{:?}", plan.already_installed, plan.unresolved, plan.optional, plan.incompatible);

    let report = resolve::execute(&downloader, &dir, &plan, Some(&game.root.join(".cache")), Arc::new(|_| {})).await.unwrap();
    println!("下载 {} 跳过 {} 失败 {}", report.downloaded, report.skipped, report.failed.len());
    for f in &report.failed {
        println!("  失败 {}：{}", f.path, f.reason);
    }
}
