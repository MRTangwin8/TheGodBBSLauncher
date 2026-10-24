use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use super::loaders::{install_game, job_reporter, loader_id_for};
use super::AppState;
use crate::engine::download::{DownloadTask, Downloader};
use crate::engine::install::{validate_version_name, GameDir, Stage};
use crate::engine::instance::{self, InstanceConfig, ModpackOrigin};
use crate::engine::modpack::export::{self, ExportEntry, ExportEnv, ExportOptions, ExportSummary};
use crate::engine::modpack::{Modpack, ModpackInfo};
use crate::engine::resources::compat::GameEnv;
use crate::engine::resources::ResourceApi;
use crate::engine::{Error, Result};

async fn api(state: &AppState) -> ResourceApi {
    let key = state.settings.read().await.curseforge_api_key.trim().to_string();
    ResourceApi { client: state.client.clone(), curseforge_key: (!key.is_empty()).then_some(key) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub info: ModpackInfo,
    /// 建议的版本名
    pub suggested_name: String,
}

fn sanitize_name(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if "/\\:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    cleaned.trim().trim_end_matches('.').to_string()
}

#[tauri::command]
pub async fn modpack_preview(state: State<'_, AppState>, path: String) -> Result<Preview> {
    let p = PathBuf::from(path);
    let pack = tokio::task::spawn_blocking(move || Modpack::open(&p)).await.map_err(|e| Error::Other(e.to_string()))??;
    let game = GameDir::new(state.settings.read().await.game_dir.clone());
    let base = sanitize_name(&format!("{}{}", pack.info.name, if pack.info.version.is_empty() { String::new() } else { format!("-{}", pack.info.version) }));
    let base = if base.is_empty() { "整合包".to_string() } else { base };
    // 重名就加序号
    let mut suggested = base.clone();
    let mut n = 2;
    while game.versions().join(&suggested).exists() {
        suggested = format!("{base} ({n})");
        n += 1;
    }
    Ok(Preview { info: pack.info, suggested_name: suggested })
}

/// 导入整合包：装游戏和加载器 → 下载文件 → 解压 overrides → 写实例设置。返回版本 id
#[tauri::command]
pub async fn modpack_import(app: AppHandle, state: State<'_, AppState>, job_id: String, path: String, name: String) -> Result<String> {
    let state: &AppState = &state;
    let name = name.trim().to_string();
    validate_version_name(&name)?;
    let game = GameDir::new(state.settings.read().await.game_dir.clone());
    if game.versions().join(&name).exists() {
        return Err(Error::Other(format!("已经有名为「{name}」的版本了")));
    }
    let p = PathBuf::from(&path);
    let pack = tokio::task::spawn_blocking(move || Modpack::open(&p)).await.map_err(|e| Error::Other(e.to_string()))??;
    let info = pack.info.clone();
    let report = job_reporter(&app, &job_id);

    // 1. 游戏 + 加载器
    let loader_id = match (info.loader, &info.loader_version) {
        (Some(kind), Some(v)) => Some((kind, loader_id_for(state, kind, &info.mc, v).await)),
        _ => None,
    };
    let id = install_game(state, report.clone(), &info.mc, loader_id.as_ref().map(|(k, v)| (*k, v.as_str())), Some(&name)).await?;

    // 2. 实例设置：整合包必须隔离
    instance::save(
        &game,
        &id,
        &InstanceConfig {
            isolated: true,
            min_memory_mb: info.min_memory_mb,
            jvm_args: info.jvm_args.clone(),
            game_args: info.game_args.clone(),
            modpack: Some(ModpackOrigin { format: info.format.label().into(), name: info.name.clone(), version: info.version.clone() }),
        },
    )?;
    let dir = game.versions().join(&id);

    // 3. 下载整合包里引用的文件
    let (files, warnings) = pack.files(&api(state).await).await?;
    let concurrency = state.settings.read().await.concurrency;
    let downloader = Downloader::new(state.client.clone(), state.source().await, concurrency);
    let tasks: Vec<DownloadTask> = files
        .into_iter()
        .map(|f| DownloadTask {
            url: f.url,
            path: f.path.split('/').fold(dir.clone(), |p, s| p.join(s)),
            sha1: f.sha1,
            size: (f.size > 0).then_some(f.size),
        })
        .collect();
    let r = report.clone();
    let result = downloader.download_all(tasks, Arc::new(move |p| r(Stage::Files, p))).await;
    if let Some(f) = result.failed.first() {
        return Err(Error::Other(format!("整合包有 {} 个文件下载失败，例如 {}：{}", result.failed.len(), f.path, f.reason)));
    }

    // 4. 包内自带的文件（配置、脚本等）
    let d = dir.clone();
    tokio::task::spawn_blocking(move || pack.extract_overrides(&d)).await.map_err(|e| Error::Other(e.to_string()))??;

    if !warnings.is_empty() {
        // 不致命的问题写进实例目录，方便排查
        let _ = std::fs::write(dir.join("modpack-warnings.txt"), warnings.join("\n"));
    }
    Ok(id)
}

#[tauri::command]
pub async fn export_candidates(state: State<'_, AppState>, version_id: String) -> Result<Vec<ExportEntry>> {
    let settings = state.settings.read().await.clone();
    let game = GameDir::new(settings.game_dir);
    Ok(export::candidates(&instance::game_dir(&game, &version_id, settings.isolate_versions), &version_id))
}

#[tauri::command]
pub async fn modpack_export(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
    version_id: String,
    options: ExportOptions,
    out_path: String,
) -> Result<ExportSummary> {
    let settings = state.settings.read().await.clone();
    let game = GameDir::new(settings.game_dir.clone());
    let env = GameEnv::detect(&game, &version_id).await?;
    let inst = instance::load(&game, &version_id);
    let export_env = ExportEnv {
        mc: env.mc,
        loader: env.loader,
        loader_version: env.loader_version,
        min_memory_mb: inst.min_memory_mb.or(settings.memory_mb),
        jvm_args: inst.jvm_args,
        game_args: inst.game_args,
    };
    let game_dir = instance::game_dir(&game, &version_id, settings.isolate_versions);
    let report = job_reporter(&app, &job_id);
    export::export(&api(&state).await, &game_dir, &options, &export_env, &PathBuf::from(out_path), Arc::new(move |p| report(Stage::Files, p))).await
}
