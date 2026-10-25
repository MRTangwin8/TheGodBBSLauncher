//! CurseForge API：https://docs.curseforge.com/rest-api/
//! 填了 API Key 走官方接口；没填走 MCIM 镜像（https://mod.mcimirror.top/curseforge）。

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{DepKind, Dependency, Project, ResFile, ResourceApi, ResourceKind, ResourceSource, ResourceVersion, SearchPage, SearchQuery, SortBy};
use crate::engine::loader::LoaderKind;
use crate::engine::http;
use crate::engine::{Error, Result};

const OFFICIAL: &str = "https://api.curseforge.com/v1";
const MCIM: &str = "https://mod.mcimirror.top/curseforge/v1";
const MINECRAFT: u32 = 432;

fn base(api: &ResourceApi) -> &'static str {
    if api.curseforge_key.as_deref().is_some_and(|key| !key.trim().is_empty()) {
        OFFICIAL
    } else {
        MCIM
    }
}

async fn send<T: DeserializeOwned>(api: &ResourceApi, path: &str, params: &[(&str, String)], body: Option<&Value>) -> Result<T> {
    let url = format!("{}{path}", base(api));
    let mut req = match body {
        Some(b) => api.client.post(&url).json(b),
        None => api.client.get(&url).query(params),
    };
    let resp = if let Some(key) = api.curseforge_key.as_deref().map(str::trim).filter(|key| !key.is_empty()) {
        req = req.header("x-api-key", http::api_key_header(key)?);
        http::credential_client().execute(req.build()?).await?
    } else {
        req.send().await?
    };
    match resp.status().as_u16() {
        200..=299 => Ok(resp.json().await?),
        401 | 403 if base(api) == OFFICIAL => Err(Error::Other("CurseForge 拒绝了请求，请检查「设置」里的 API Key 和应用权限".into())),
        401 | 403 => Err(Error::Other("CurseForge 社区镜像拒绝了请求；可稍后重试，或在本地设置自己的 API Key 后使用官方接口".into())),
        429 => Err(Error::Other("CurseForge 请求过于频繁，请稍后重试".into())),
        s => Err(Error::BadStatus { url, status: s }),
    }
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

fn class_id(kind: ResourceKind) -> u32 {
    match kind {
        ResourceKind::Mod => 6,
        ResourceKind::ResourcePack => 12,
        ResourceKind::Shader => 6552,
    }
}

fn loader_type(l: LoaderKind) -> u32 {
    match l {
        LoaderKind::Forge => 1,
        LoaderKind::Fabric => 4,
        LoaderKind::Quilt => 5,
        LoaderKind::NeoForge => 6,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    slug: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    authors: Vec<Named>,
    logo: Option<Logo>,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    categories: Vec<Named>,
    #[serde(default)]
    date_modified: String,
    links: Option<Links>,
    #[serde(default)]
    class_id: Option<u32>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Logo {
    thumbnail_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Links {
    website_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResp {
    data: Vec<CfMod>,
    pagination: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    total_count: u64,
}

pub async fn search(api: &ResourceApi, q: &SearchQuery) -> Result<SearchPage> {
    let sort = match q.sort {
        SortBy::Relevance => 2,
        SortBy::Downloads => 6,
        SortBy::Updated => 3,
    };
    let mut params = vec![
        ("gameId", MINECRAFT.to_string()),
        ("classId", class_id(q.kind).to_string()),
        ("searchFilter", q.query.clone()),
        ("sortField", sort.to_string()),
        ("sortOrder", "desc".into()),
        ("index", q.offset.to_string()),
        ("pageSize", q.limit.min(50).to_string()),
    ];
    if let Some(v) = &q.game_version {
        params.push(("gameVersion", v.clone()));
    }
    if let (ResourceKind::Mod, Some(l)) = (q.kind, q.loader) {
        params.push(("modLoaderType", loader_type(l).to_string()));
    }
    let resp: SearchResp = send(api, "/mods/search", &params, None).await?;
    Ok(SearchPage {
        // CurseForge 最多只能翻到第 10000 条
        total: resp.pagination.total_count.min(10_000),
        hits: resp
            .data
            .into_iter()
            .map(|m| Project {
                source: ResourceSource::CurseForge,
                id: m.id.to_string(),
                page_url: m.links.and_then(|l| l.website_url).unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/mc-mods/{}", m.slug)),
                slug: m.slug,
                title: m.name,
                description: m.summary,
                author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
                icon_url: m.logo.and_then(|l| l.thumbnail_url),
                downloads: m.download_count as u64,
                categories: m.categories.into_iter().map(|c| c.name).collect(),
                updated: m.date_modified,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    mod_id: u64,
    display_name: String,
    file_name: String,
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CfHash>,
    #[serde(default)]
    file_length: u64,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    release_type: u32,
    #[serde(default)]
    dependencies: Vec<CfDep>,
}

#[derive(Deserialize)]
struct CfHash {
    value: String,
    algo: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfDep {
    mod_id: u64,
    relation_type: u32,
}

impl From<CfFile> for ResourceVersion {
    fn from(f: CfFile) -> Self {
        let loaders = f
            .game_versions
            .iter()
            .filter(|v| ["Forge", "Fabric", "Quilt", "NeoForge"].contains(&v.as_str()))
            .map(|v| v.to_lowercase())
            .collect();
        let game_versions = f.game_versions.iter().filter(|v| v.starts_with(|c: char| c.is_ascii_digit())).cloned().collect();
        ResourceVersion {
            source: ResourceSource::CurseForge,
            id: f.id.to_string(),
            project_id: f.mod_id.to_string(),
            name: f.display_name.clone(),
            version_number: f.display_name,
            game_versions,
            loaders,
            published: f.file_date,
            release_type: match f.release_type {
                1 => "release",
                2 => "beta",
                _ => "alpha",
            }
            .into(),
            file: f.download_url.filter(|url| !url.trim().is_empty()).map(|url| ResFile {
                url,
                sha1: f.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone()),
                filename: f.file_name,
                size: f.file_length,
            }),
            dependencies: f
                .dependencies
                .into_iter()
                .filter_map(|d| {
                    let kind = match d.relation_type {
                        3 => DepKind::Required,
                        2 => DepKind::Optional,
                        5 => DepKind::Incompatible,
                        1 | 6 => DepKind::Embedded,
                        _ => return None, // 4 = Tool
                    };
                    Some(Dependency { project_id: d.mod_id.to_string(), version_id: None, kind })
                })
                .collect(),
        }
    }
}

pub async fn versions(
    api: &ResourceApi,
    project_id: &str,
    kind: ResourceKind,
    game_version: Option<&str>,
    loader: Option<LoaderKind>,
) -> Result<Vec<ResourceVersion>> {
    let fetch = |lt: Option<u32>| async move {
        let mut params = vec![("pageSize", "50".to_string())];
        if let Some(v) = game_version {
            params.push(("gameVersion", v.to_string()));
        }
        if let Some(lt) = lt {
            params.push(("modLoaderType", lt.to_string()));
        }
        let resp: Data<Vec<CfFile>> = send(api, &format!("/mods/{project_id}/files"), &params, None).await?;
        Ok::<_, Error>(resp.data)
    };
    let lt = if kind == ResourceKind::Mod { loader.map(loader_type) } else { None };
    let mut files = fetch(lt).await?;
    // Quilt 上可以用 Fabric 模组
    if files.is_empty() && loader == Some(LoaderKind::Quilt) && kind == ResourceKind::Mod {
        files = fetch(Some(loader_type(LoaderKind::Fabric))).await?;
    }
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    Ok(files.into_iter().map(Into::into).collect())
}

pub async fn file(api: &ResourceApi, project_id: &str, file_id: &str) -> Result<ResourceVersion> {
    let resp: Data<CfFile> = send(api, &format!("/mods/{project_id}/files/{file_id}"), &[], None).await?;
    Ok(resp.data.into())
}

pub async fn titles(api: &ResourceApi, ids: &[String]) -> Result<HashMap<String, String>> {
    let ids: Vec<u64> = ids.iter().filter_map(|i| i.parse().ok()).collect();
    let resp: Data<Vec<CfMod>> = send(api, "/mods", &[], Some(&json!({"modIds": ids}))).await?;
    Ok(resp.data.into_iter().map(|m| (m.id.to_string(), m.name)).collect())
}

/// 按文件 id 批量查文件
pub async fn files_by_ids(api: &ResourceApi, file_ids: &[u64]) -> Result<Vec<ResourceVersion>> {
    let mut out = Vec::new();
    // 一次请求别太大
    for chunk in file_ids.chunks(500) {
        let resp: Data<Vec<CfFile>> = send(api, "/mods/files", &[], Some(&json!({"fileIds": chunk}))).await?;
        out.extend(resp.data.into_iter().map(ResourceVersion::from));
    }
    Ok(out)
}

/// 项目 id → classId（6 模组、12 资源包、6552 光影）
pub async fn mod_classes(api: &ResourceApi, ids: &[String]) -> Result<HashMap<String, u32>> {
    let ids: Vec<u64> = ids.iter().filter_map(|i| i.parse().ok()).collect();
    let mut out = HashMap::new();
    for chunk in ids.chunks(500) {
        let resp: Data<Vec<CfMod>> = send(api, "/mods", &[], Some(&json!({"modIds": chunk}))).await?;
        out.extend(resp.data.into_iter().filter_map(|m| m.class_id.map(|c| (m.id.to_string(), c))));
    }
    Ok(out)
}

/// CurseForge 文件指纹：去掉空白字节（9、10、13、32）后算 MurmurHash2，种子 1
pub fn fingerprint(data: &[u8]) -> u32 {
    let bytes: Vec<u8> = data.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    const M: u32 = 0x5bd1_e995;
    let mut h: u32 = 1 ^ (bytes.len() as u32);
    let (chunks, rest) = bytes.as_chunks::<4>();
    for c in chunks {
        let mut k = u32::from_le_bytes(*c);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if !rest.is_empty() {
        for (i, b) in rest.iter().enumerate() {
            h ^= (*b as u32) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintResp {
    #[serde(default)]
    exact_matches: Vec<FingerprintMatch>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintMatch {
    id: u64,
    file: FingerprintFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintFile {
    id: u64,
    file_fingerprint: u32,
}

/// 指纹 → (项目 id, 文件 id)
pub async fn match_fingerprints(api: &ResourceApi, fps: &[u32]) -> Result<HashMap<u32, (u64, u64)>> {
    if fps.is_empty() {
        return Ok(HashMap::new());
    }
    let resp: Data<FingerprintResp> = send(api, "/fingerprints", &[], Some(&json!({"fingerprints": fps}))).await?;
    Ok(resp.data.exact_matches.into_iter().map(|m| (m.file.file_fingerprint, (m.id, m.file.id))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_whitespace() {
        assert_eq!(fingerprint(b"ab c
	d"), fingerprint(b"abcd"));
        assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
        // 长度不是 4 的倍数时也要处理尾部
        assert_ne!(fingerprint(b"abcde"), fingerprint(b"abcd"));
    }

    #[test]
    fn converts_file_with_an_explicit_download_url() {
        let f: CfFile = serde_json::from_str(r#"{
            "id": 9298209, "modId": 238222, "displayName": "15.74.0.229 for Forge 1.20.1",
            "fileName": "jei 1.20.1+forge.jar", "downloadUrl": "https://edge.forgecdn.net/files/9298/209/jei.jar", "fileLength": 6892564,
            "gameVersions": ["Client", "1.20.1", "Forge", "Server"], "releaseType": 2, "fileDate": "2026-10-23T14:03:50Z",
            "hashes": [{"value": "f3ef", "algo": 1}, {"value": "md5", "algo": 2}],
            "dependencies": [{"modId": 1, "relationType": 2}, {"modId": 2, "relationType": 3}, {"modId": 3, "relationType": 4}]
        }"#).unwrap();
        let v: ResourceVersion = f.into();
        let file = v.file.unwrap();
        assert_eq!(file.url, "https://edge.forgecdn.net/files/9298/209/jei.jar");
        assert_eq!(file.sha1.as_deref(), Some("f3ef"));
        assert_eq!(v.loaders, ["forge"]);
        assert_eq!(v.game_versions, ["1.20.1"]);
        assert_eq!(v.release_type, "beta");
        assert_eq!(v.dependencies.len(), 2, "Tool 类依赖忽略");
        assert_eq!(v.dependencies[1].kind, DepKind::Required);
    }

    #[test]
    fn does_not_guess_download_urls_for_restricted_files() {
        for download_url in [Value::Null, json!(""), json!("   ")] {
            let file: CfFile = serde_json::from_value(json!({
                "id": 1, "modId": 2, "displayName": "Restricted file",
                "fileName": "restricted.jar", "downloadUrl": download_url
            })).unwrap();
            let version: ResourceVersion = file.into();
            assert!(version.file.is_none());
            assert_eq!(version.project_id, "2");
        }
    }

    #[test]
    fn blank_keys_use_the_community_mirror() {
        let mut api = ResourceApi { client: reqwest::Client::new(), curseforge_key: Some(" \t ".into()) };
        assert_eq!(base(&api), MCIM);
        api.curseforge_key = Some("local-test-key".into());
        assert_eq!(base(&api), OFFICIAL);
    }
}
