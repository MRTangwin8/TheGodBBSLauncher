//! 资源中心：Modrinth / CurseForge 搜索、版本筛选、依赖解析、本地资源管理。

pub mod classfile;
pub mod compat;
pub mod curseforge;
pub mod local;
pub mod modrinth;
pub mod resolve;
pub mod klpbbs;

use serde::{Deserialize, Serialize};

use crate::engine::loader::LoaderKind;
use crate::engine::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceSource {
    Modrinth,
    CurseForge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Mod,
    ResourcePack,
    Shader,
    /// 整合包：只用于搜索和下载，下载后交给整合包导入流程，不装进某个版本的目录
    Modpack,
}

impl ResourceKind {
    /// 在游戏目录下的文件夹
    pub fn folder(self) -> &'static str {
        match self {
            ResourceKind::Mod => "mods",
            ResourceKind::ResourcePack => "resourcepacks",
            ResourceKind::Shader => "shaderpacks",
            ResourceKind::Modpack => "modpacks",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    Relevance,
    Downloads,
    Updated,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default)]
    pub query: String,
    pub kind: ResourceKind,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<LoaderKind>,
    #[serde(default)]
    pub sort: SortBy,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub source: ResourceSource,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub categories: Vec<String>,
    pub updated: String,
    pub page_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub hits: Vec<Project>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResFile {
    pub url: String,
    pub filename: String,
    pub sha1: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepKind {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub project_id: String,
    pub version_id: Option<String>,
    pub kind: DepKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceVersion {
    pub source: ResourceSource,
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub published: String,
    /// release / beta / alpha
    pub release_type: String,
    pub file: Option<ResFile>,
    pub dependencies: Vec<Dependency>,
}

/// 两个平台的统一入口。CurseForge 没有 API Key 时走 MCIM 镜像。
#[derive(Clone)]
pub struct ResourceApi {
    pub client: reqwest::Client,
    pub curseforge_key: Option<String>,
}

impl ResourceApi {
    pub async fn search(&self, source: ResourceSource, q: &SearchQuery) -> Result<SearchPage> {
        match source {
            ResourceSource::Modrinth => modrinth::search(&self.client, q).await,
            ResourceSource::CurseForge => curseforge::search(self, q).await,
        }
    }

    /// 兼容指定游戏版本 / 加载器的版本列表，新的在前
    pub async fn versions(
        &self,
        source: ResourceSource,
        project_id: &str,
        kind: ResourceKind,
        game_version: Option<&str>,
        loader: Option<LoaderKind>,
    ) -> Result<Vec<ResourceVersion>> {
        match source {
            ResourceSource::Modrinth => modrinth::versions(&self.client, project_id, kind, game_version, loader).await,
            ResourceSource::CurseForge => curseforge::versions(self, project_id, kind, game_version, loader).await,
        }
    }

    pub async fn version(&self, source: ResourceSource, project_id: &str, version_id: &str) -> Result<ResourceVersion> {
        match source {
            ResourceSource::Modrinth => modrinth::version(&self.client, version_id).await,
            ResourceSource::CurseForge => curseforge::file(self, project_id, version_id).await,
        }
    }

    /// 项目 id → 名称（用于显示前置的名字）
    pub async fn titles(&self, source: ResourceSource, ids: &[String]) -> Result<std::collections::HashMap<String, String>> {
        if ids.is_empty() {
            return Ok(Default::default());
        }
        match source {
            ResourceSource::Modrinth => modrinth::titles(&self.client, ids).await,
            ResourceSource::CurseForge => curseforge::titles(self, ids).await,
        }
    }
}

/// Modrinth 加载器名；Quilt 可以兼容 Fabric 模组
pub(crate) fn loader_names(loader: LoaderKind) -> &'static [&'static str] {
    match loader {
        LoaderKind::Fabric => &["fabric"],
        LoaderKind::Quilt => &["quilt", "fabric"],
        LoaderKind::Forge => &["forge"],
        LoaderKind::NeoForge => &["neoforge"],
    }
}

/// 同一组候选里优先正式版，其次测试版
pub fn best_version(list: &[ResourceVersion]) -> Option<&ResourceVersion> {
    let rank = |v: &ResourceVersion| match v.release_type.as_str() {
        "release" => 0,
        "beta" => 1,
        _ => 2,
    };
    list.iter().filter(|v| v.file.is_some()).min_by_key(|v| rank(v))
}
