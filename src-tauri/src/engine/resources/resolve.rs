//! 依赖解析：从选中的版本出发，递归找出必需的前置，生成安装计划；确认后再下载。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::compat::{check_jar, CompatReport, CompatStatus, GameEnv, RuntimeJar};
use super::{best_version, local, DepKind, ResFile, ResourceApi, ResourceKind, ResourceSource, ResourceVersion};
use crate::engine::download::{DownloadReport, DownloadTask, Downloader, Progress};
use crate::engine::loader::LoaderKind;
use crate::engine::Result;

/// 安装到哪里、按什么条件挑前置版本
#[derive(Debug, Clone)]
pub struct Target {
    pub dir: PathBuf,
    pub kind: ResourceKind,
    pub game_version: Option<String>,
    pub loader: Option<LoaderKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub source: ResourceSource,
    pub project_id: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub file: ResFile,
    /// 作为谁的前置被拉进来；None 表示用户选中的本体
    pub required_by: Option<String>,
    /// 会被替换掉的旧文件（同一项目的旧版本）
    pub replaces: Vec<String>,
    /// 下载时做的兼容性检查结果
    #[serde(default)]
    pub compat: Option<CompatReport>,
    /// 自动换版本之类的说明
    #[serde(default)]
    pub note: Option<String>,
    /// 用户选的版本不兼容时，推荐的兼容版本
    #[serde(default)]
    pub alternative: Option<ResourceVersion>,
}

/// 下载时检查兼容性需要的东西
pub struct Checker {
    pub env: GameEnv,
    pub runtime: Option<Arc<RuntimeJar>>,
    /// 检查时先下载到这里，确认安装时直接复制过去
    pub cache_dir: PathBuf,
    pub downloader: Downloader,
}

pub fn cache_path(cache_dir: &std::path::Path, file: &ResFile) -> PathBuf {
    let name: String = file.filename.chars().map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    cache_dir.join(format!("{}-{name}", file.sha1.as_deref().unwrap_or("nohash")))
}

async fn check_file(c: &Checker, file: &ResFile) -> CompatReport {
    let path = cache_path(&c.cache_dir, file);
    let task = DownloadTask { url: file.url.clone(), path: path.clone(), sha1: file.sha1.clone(), size: (file.size > 0).then_some(file.size) };
    let r = c.downloader.download_all(vec![task], Arc::new(|_| {})).await;
    if !r.failed.is_empty() {
        return CompatReport { status: CompatStatus::Unknown, problems: vec![], notes: vec!["下载失败，没能检查兼容性".into()] };
    }
    let (env, rt) = (c.env.clone(), c.runtime.clone());
    tokio::task::spawn_blocking(move || check_jar(&path, &env, rt.as_deref()))
        .await
        .unwrap_or(CompatReport { status: CompatStatus::Unknown, problems: vec![], notes: vec![] })
}

/// 同一个项目里找第一个能通过检查的版本（新的在前，测试版也算），最多试 6 个
async fn find_compatible(api: &ResourceApi, c: &Checker, target: &Target, source: ResourceSource, project_id: &str, exclude: &str) -> Option<(ResourceVersion, CompatReport)> {
    let list = api.versions(source, project_id, ResourceKind::Mod, target.game_version.as_deref(), target.loader).await.ok()?;
    for v in list.iter().filter(|v| v.id != exclude && v.file.is_some()).take(6) {
        let report = check_file(c, v.file.as_ref()?).await;
        if report.status == CompatStatus::Ok {
            return Some((v.clone(), report));
        }
    }
    None
}

fn type_hint(release_type: &str) -> &'static str {
    match release_type {
        "beta" => "（测试版）",
        "alpha" => "（开发版）",
        _ => "",
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub items: Vec<PlanItem>,
    /// 已经装好的前置
    pub already_installed: Vec<String>,
    /// 找不到兼容版本的前置
    pub unresolved: Vec<String>,
    /// 可选前置（不自动安装，只提示）
    pub optional: Vec<String>,
    /// 已安装且与新模组冲突的
    pub incompatible: Vec<String>,
}

pub async fn plan(api: &ResourceApi, target: &Target, root: ResourceVersion, checker: Option<&Checker>) -> Result<InstallPlan> {
    let source = root.source;
    // 只有 Modrinth 能按哈希识别已装的文件
    let installed: HashMap<String, Vec<String>> = if source == ResourceSource::Modrinth {
        local::installed_projects(&api.client, &target.dir, target.kind).await
    } else {
        HashMap::new()
    };

    let mut plan = InstallPlan::default();
    let mut seen: HashSet<String> = HashSet::from([root.project_id.clone()]);
    let mut queue: VecDeque<(ResourceVersion, Option<String>)> = VecDeque::from([(root, None)]);
    let (mut installed_ids, mut unresolved_ids, mut optional_ids, mut incompatible_ids) = (vec![], vec![], vec![], vec![]);

    while let Some((v, parent)) = queue.pop_front() {
        let Some(file) = v.file.clone() else {
            unresolved_ids.push(v.project_id.clone());
            continue;
        };
        let replaces: Vec<String> = installed
            .get(&v.project_id)
            .map(|files| files.iter().filter(|f| **f != file.filename).cloned().collect())
            .unwrap_or_default();
        // 前置的同名文件已经在了就算装好；本体则照常放进计划（下载器会校验后跳过）
        if parent.is_some() && target.dir.join(&file.filename).is_file() {
            installed_ids.push(v.project_id.clone());
        } else {
            plan.items.push(PlanItem {
                source,
                project_id: v.project_id.clone(),
                title: v.project_id.clone(),
                version_id: v.id.clone(),
                version_number: v.version_number.clone(),
                file,
                required_by: parent,
                replaces,
                compat: None,
                note: None,
                alternative: None,
            });
        }

        for dep in &v.dependencies {
            // Modrinth 有时只给 version_id
            let (project_id, pinned) = if dep.project_id.is_empty() {
                match &dep.version_id {
                    Some(vid) => match api.version(source, "", vid).await {
                        Ok(dv) => (dv.project_id.clone(), Some(dv)),
                        Err(_) => continue,
                    },
                    None => continue,
                }
            } else {
                (dep.project_id.clone(), None)
            };
            match dep.kind {
                DepKind::Optional => {
                    if !installed.contains_key(&project_id) {
                        optional_ids.push(project_id);
                    }
                }
                DepKind::Incompatible => {
                    if installed.contains_key(&project_id) {
                        incompatible_ids.push(project_id);
                    }
                }
                DepKind::Embedded => {}
                DepKind::Required => {
                    if installed.contains_key(&project_id) {
                        installed_ids.push(project_id);
                        continue;
                    }
                    if !seen.insert(project_id.clone()) {
                        continue;
                    }
                    let resolved = match (pinned, &dep.version_id) {
                        (Some(dv), _) => Some(dv),
                        (None, Some(vid)) => api.version(source, &project_id, vid).await.ok(),
                        (None, None) => api
                            .versions(source, &project_id, ResourceKind::Mod, target.game_version.as_deref(), target.loader)
                            .await
                            .ok()
                            .and_then(|list| best_version(&list).cloned()),
                    };
                    match resolved {
                        Some(dv) => queue.push_back((dv, Some(v.project_id.clone()))),
                        None => unresolved_ids.push(project_id),
                    }
                }
            }
        }
    }

    // 兼容性：前置不兼容就自动换版本；用户自己选的版本只标出来并给出推荐
    if let (Some(c), ResourceKind::Mod) = (checker, target.kind) {
        for item in &mut plan.items {
            let report = check_file(c, &item.file).await;
            if report.status == CompatStatus::Problem {
                if let Some((alt, alt_report)) = find_compatible(api, c, target, source, &item.project_id, &item.version_id).await {
                    if item.required_by.is_some() {
                        item.note = Some(format!(
                            "原定的 {} 不兼容（{}），已自动换成 {}{}",
                            item.version_number,
                            report.problems.first().map(String::as_str).unwrap_or("未通过检查"),
                            alt.version_number,
                            type_hint(&alt.release_type)
                        ));
                        item.version_id = alt.id.clone();
                        item.version_number = alt.version_number.clone();
                        item.file = alt.file.clone().expect("候选版本都有文件");
                        item.compat = Some(alt_report);
                        continue;
                    }
                    item.alternative = Some(alt);
                }
            }
            item.compat = Some(report);
        }
    }

    // 一次性查出所有项目名
    let mut ids: Vec<String> = plan.items.iter().map(|i| i.project_id.clone()).collect();
    ids.extend(installed_ids.iter().chain(&unresolved_ids).chain(&optional_ids).chain(&incompatible_ids).cloned());
    ids.sort();
    ids.dedup();
    let titles = api.titles(source, &ids).await.unwrap_or_default();
    let name = |id: &String| titles.get(id).cloned().unwrap_or_else(|| id.clone());
    for item in &mut plan.items {
        item.title = name(&item.project_id);
        item.required_by = item.required_by.as_ref().map(name);
    }
    let names = |list: Vec<String>| {
        let mut out: Vec<String> = list.iter().map(name).collect();
        out.sort();
        out.dedup();
        out
    };
    plan.already_installed = names(installed_ids);
    plan.unresolved = names(unresolved_ids);
    plan.optional = names(optional_ids);
    plan.incompatible = names(incompatible_ids);
    Ok(plan)
}

/// 按计划下载；全部成功后删除被替换的旧版本
pub async fn execute(
    downloader: &Downloader,
    dir: &std::path::Path,
    plan: &InstallPlan,
    cache_dir: Option<&std::path::Path>,
    on_progress: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<DownloadReport> {
    let mut tasks = Vec::new();
    for item in &plan.items {
        // 检查兼容性时已经下载过的，直接复制（下载器校验通过后会跳过）
        if let Some(cache) = cache_dir {
            let cached = cache_path(cache, &item.file);
            let dest = local::safe_path(dir, &item.file.filename)?;
            if cached.is_file() && !dest.exists() {
                let _ = std::fs::copy(&cached, &dest);
            }
        }
        tasks.push(DownloadTask {
            url: item.file.url.clone(),
            path: local::safe_path(dir, &item.file.filename)?,
            sha1: item.file.sha1.clone(),
            size: (item.file.size > 0).then_some(item.file.size),
        });
    }
    let report = downloader.download_all(tasks, on_progress).await;
    if report.failed.is_empty() {
        // 新旧文件同名时，下载已经原地覆盖了，不能再删
        let new_files: Vec<&str> = plan.items.iter().map(|i| i.file.filename.as_str()).collect();
        for old in plan.items.iter().flat_map(|i| &i.replaces).filter(|old| !new_files.contains(&old.as_str())) {
            let _ = local::delete(dir, old);
        }
    }
    Ok(report)
}
