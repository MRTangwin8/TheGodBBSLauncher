//! Modrinth API v2：https://docs.modrinth.com/api/
//! 官方接口优先，失败时回退到 MCIM 镜像。

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{loader_names, DepKind, Dependency, Project, ResFile, ResourceKind, ResourceSource, ResourceVersion, SearchPage, SearchQuery, SortBy};
use crate::engine::loader::LoaderKind;
use crate::engine::mirror::{self, Source};
use crate::engine::{Error, Result};

const API: &str = "https://api.modrinth.com/v2";

async fn send_json<T: DeserializeOwned>(client: &reqwest::Client, url: &str, body: Option<&Value>) -> Result<T> {
    let mut last = None;
    for candidate in mirror::candidates(url, Source::Official) {
        let req = match body {
            Some(b) => client.post(&candidate).json(b),
            None => client.get(&candidate),
        };
        match req.send().await {
            Ok(r) if r.status().is_success() => return Ok(r.json().await?),
            Ok(r) => last = Some(Error::BadStatus { url: candidate, status: r.status().as_u16() }),
            Err(e) => last = Some(e.into()),
        }
    }
    Err(last.unwrap_or_else(|| Error::Other("Modrinth 请求失败".into())))
}

fn with_params(path: &str, params: &[(&str, String)]) -> String {
    reqwest::Url::parse_with_params(&format!("{API}{path}"), params)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("{API}{path}"))
}

fn project_type(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Mod => "mod",
        ResourceKind::ResourcePack => "resourcepack",
        ResourceKind::Shader => "shader",
        ResourceKind::Modpack => "modpack",
    }
}

#[derive(Deserialize)]
struct SearchResp {
    hits: Vec<Hit>,
    total_hits: u64,
}

#[derive(Deserialize)]
struct Hit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    icon_url: Option<String>,
    downloads: u64,
    #[serde(default)]
    display_categories: Vec<String>,
    date_modified: String,
    project_type: String,
}

pub async fn search(client: &reqwest::Client, q: &SearchQuery) -> Result<SearchPage> {
    let mut facets = vec![vec![format!("project_type:{}", project_type(q.kind))]];
    if let Some(v) = &q.game_version {
        facets.push(vec![format!("versions:{v}")]);
    }
    if let (ResourceKind::Mod, Some(l)) = (q.kind, q.loader) {
        facets.push(loader_names(l).iter().map(|n| format!("categories:{n}")).collect());
    }
    let index = match q.sort {
        SortBy::Relevance => "relevance",
        SortBy::Downloads => "downloads",
        SortBy::Updated => "updated",
    };
    let url = with_params(
        "/search",
        &[
            ("query", q.query.clone()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index.into()),
            ("offset", q.offset.to_string()),
            ("limit", q.limit.min(100).to_string()),
        ],
    );
    let resp: SearchResp = send_json(client, &url, None).await?;
    Ok(SearchPage {
        total: resp.total_hits,
        hits: resp
            .hits
            .into_iter()
            .map(|h| Project {
                source: ResourceSource::Modrinth,
                page_url: format!("https://modrinth.com/{}/{}", h.project_type, h.slug),
                id: h.project_id,
                slug: h.slug,
                title: h.title,
                description: h.description,
                author: h.author,
                icon_url: h.icon_url.filter(|s| !s.is_empty()),
                downloads: h.downloads,
                categories: h.display_categories,
                updated: h.date_modified,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct MrVersion {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    date_published: String,
    version_type: String,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: Vec<MrDep>,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct MrDep {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

impl From<MrVersion> for ResourceVersion {
    fn from(v: MrVersion) -> Self {
        let file = v.files.iter().find(|f| f.primary).or(v.files.first()).map(|f| ResFile {
            url: f.url.clone(),
            filename: f.filename.clone(),
            sha1: f.hashes.get("sha1").cloned(),
            size: f.size,
        });
        let dependencies = v
            .dependencies
            .into_iter()
            .map(|d| {
                let kind = match d.dependency_type.as_str() {
                    "required" => DepKind::Required,
                    "optional" => DepKind::Optional,
                    "incompatible" => DepKind::Incompatible,
                    _ => DepKind::Embedded,
                };
                // 只给了 version_id 的依赖，project_id 留空，解析时再查
                Dependency { project_id: d.project_id.unwrap_or_default(), version_id: d.version_id, kind }
            })
            .collect();
        ResourceVersion {
            source: ResourceSource::Modrinth,
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            version_number: v.version_number,
            game_versions: v.game_versions,
            loaders: v.loaders,
            published: v.date_published,
            release_type: v.version_type,
            file,
            dependencies,
        }
    }
}

pub async fn versions(
    client: &reqwest::Client,
    project_id: &str,
    kind: ResourceKind,
    game_version: Option<&str>,
    loader: Option<LoaderKind>,
) -> Result<Vec<ResourceVersion>> {
    let mut params = Vec::new();
    if let Some(v) = game_version {
        params.push(("game_versions", json!([v]).to_string()));
    }
    if let (ResourceKind::Mod, Some(l)) = (kind, loader) {
        params.push(("loaders", json!(loader_names(l)).to_string()));
    }
    let url = with_params(&format!("/project/{project_id}/version"), &params);
    let list: Vec<MrVersion> = send_json(client, &url, None).await?;
    Ok(list.into_iter().map(Into::into).collect())
}

pub async fn version(client: &reqwest::Client, version_id: &str) -> Result<ResourceVersion> {
    let v: MrVersion = send_json(client, &format!("{API}/version/{version_id}"), None).await?;
    Ok(v.into())
}

#[derive(Deserialize)]
struct MrProject {
    id: String,
    title: String,
}

pub async fn titles(client: &reqwest::Client, ids: &[String]) -> Result<HashMap<String, String>> {
    let url = with_params("/projects", &[("ids", serde_json::to_string(ids)?)]);
    let list: Vec<MrProject> = send_json(client, &url, None).await?;
    Ok(list.into_iter().map(|p| (p.id, p.title)).collect())
}

/// 按 SHA1 识别本地文件属于哪个项目：sha1 → (project_id, version_number)
pub async fn identify(client: &reqwest::Client, sha1s: &[String]) -> Result<HashMap<String, (String, String)>> {
    if sha1s.is_empty() {
        return Ok(HashMap::new());
    }
    let body = json!({"hashes": sha1s, "algorithm": "sha1"});
    let map: HashMap<String, MrVersion> = send_json(client, &format!("{API}/version_files"), Some(&body)).await?;
    Ok(map.into_iter().map(|(h, v)| (h, (v.project_id, v.version_number))).collect())
}

/// 按 SHA1 找到 Modrinth 上对应的文件：sha1 → (下载地址, sha512)。导出 mrpack 用
pub async fn identify_files(client: &reqwest::Client, sha1s: &[String]) -> Result<HashMap<String, (String, Option<String>)>> {
    if sha1s.is_empty() {
        return Ok(HashMap::new());
    }
    let body = json!({"hashes": sha1s, "algorithm": "sha1"});
    let map: HashMap<String, MrVersion> = send_json(client, &format!("{API}/version_files"), Some(&body)).await?;
    Ok(map
        .into_iter()
        .filter_map(|(h, v)| {
            let f = v.files.into_iter().find(|f| f.hashes.get("sha1").is_some_and(|s| s.eq_ignore_ascii_case(&h)))?;
            Some((h, (f.url, f.hashes.get("sha512").cloned())))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_version_json() {
        let v: MrVersion = serde_json::from_str(r#"{
            "id": "YEWu6red", "project_id": "PtjYWJkn", "name": "Sodium Extra 0.9.4", "version_number": "mc1.21.1-0.9.4+fabric",
            "game_versions": ["1.21.1"], "loaders": ["fabric"], "date_published": "2024-10-01T00:00:00Z", "version_type": "release",
            "files": [
                {"url": "https://cdn.modrinth.com/a-sources.jar", "filename": "a-sources.jar", "hashes": {"sha1": "s"}, "primary": false, "size": 1},
                {"url": "https://cdn.modrinth.com/a.jar", "filename": "a.jar", "hashes": {"sha1": "abc"}, "primary": true, "size": 2}
            ],
            "dependencies": [
                {"version_id": null, "project_id": "AANobbMI", "file_name": null, "dependency_type": "required"},
                {"version_id": null, "project_id": "EE4vxUHj", "file_name": null, "dependency_type": "optional"}
            ]
        }"#).unwrap();
        let r: ResourceVersion = v.into();
        assert_eq!(r.file.as_ref().unwrap().filename, "a.jar", "优先 primary 文件");
        assert_eq!(r.file.as_ref().unwrap().sha1.as_deref(), Some("abc"));
        assert_eq!(r.dependencies[0].kind, DepKind::Required);
        assert_eq!(r.dependencies[1].kind, DepKind::Optional);
    }
}
