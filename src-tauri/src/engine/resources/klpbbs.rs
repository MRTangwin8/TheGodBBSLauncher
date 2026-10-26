//! Public Chinese catalog. Upstream IDs remain authoritative for versions and dependency resolution.

use reqwest::Url;
use serde::Deserialize;
use serde_json::Value;

use super::{Project, ResourceKind, ResourceSource, SearchPage, SearchQuery, SortBy};
use crate::engine::{Error, Result};

const API: &str = "https://hub.klpbbs.com/api/v1/resources";

#[derive(Deserialize)]
struct Envelope {
    code: u16,
    data: Option<Page>,
}

#[derive(Deserialize)]
struct Page {
    items: Vec<Resource>,
    pagination: Pagination,
}

#[derive(Deserialize)]
struct Pagination {
    total: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resource {
    source: ResourceSource,
    source_id: String,
    slug: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    author: String,
    icon_url: Option<String>,
    source_url: Option<String>,
    #[serde(default)]
    downloads: Value,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    updated_at: String,
}

fn https_url(url: &str) -> Option<Url> {
    Url::parse(url).ok().filter(|url| {
        url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
    })
}

impl Resource {
    fn into_project(self) -> Result<Project> {
        let valid_id = !self.source_id.is_empty()
            && self.source_id.bytes().all(|byte| match self.source {
                ResourceSource::CurseForge => byte.is_ascii_digit(),
                ResourceSource::Modrinth => byte.is_ascii_alphanumeric(),
            });
        if !valid_id {
            return Err(Error::Other("KLPBBS 返回的上游项目 ID 无效，无法安全查询版本".into()));
        }
        let origin = match self.source {
            ResourceSource::Modrinth => "https://modrinth.com",
            ResourceSource::CurseForge => "https://www.curseforge.com",
        };
        let page_url = self.source_url.as_deref().and_then(https_url)
            .filter(|url| match self.source {
                ResourceSource::Modrinth => url.host_str() == Some("modrinth.com"),
                ResourceSource::CurseForge => matches!(url.host_str(), Some("curseforge.com" | "www.curseforge.com")),
            })
            .unwrap_or_else(|| {
                let mut url = Url::parse(origin).expect("fixed resource origin");
                let prefix = match self.source {
                    ResourceSource::Modrinth => vec!["project"],
                    ResourceSource::CurseForge => vec!["minecraft", "mc-mods"],
                };
                url.path_segments_mut().expect("https origin").extend(prefix).push(&self.slug);
                url
            });
        Ok(Project {
            source: self.source,
            id: self.source_id,
            slug: self.slug,
            title: self.name,
            description: self.summary,
            author: self.author,
            icon_url: self.icon_url.as_deref().and_then(https_url).map(|url| url.to_string()),
            downloads: self.downloads.as_u64()
                .or_else(|| self.downloads.as_str().and_then(|value| value.parse().ok())).unwrap_or(0),
            categories: self.categories,
            updated: self.updated_at,
            page_url: page_url.to_string(),
        })
    }
}

pub async fn search(client: &reqwest::Client, query: &SearchQuery) -> Result<SearchPage> {
    if query.query.chars().count() > 100 {
        return Err(Error::Other("KLPBBS 搜索词不能超过 100 个字符".into()));
    }
    let size = query.limit.clamp(1, 50);
    let mut params = vec![
        ("q", query.query.trim().to_string()),
        ("locale", "zh-CN".into()),
        ("source", "all".into()),
        ("type", match query.kind {
            ResourceKind::Mod => "mod",
            ResourceKind::ResourcePack => "resource-pack",
            ResourceKind::Shader => "shader",
            ResourceKind::Modpack => "modpack",
        }.into()),
        ("sort", match query.sort {
            SortBy::Downloads | SortBy::Relevance => "downloads",
            SortBy::Updated => "updated",
        }.into()),
        ("page", (query.offset / size).saturating_add(1).to_string()),
        ("pageSize", size.to_string()),
    ];
    if let Some(version) = &query.game_version {
        params.push(("version", version.clone()));
    }
    if let Some(loader) = query.loader.filter(|_| query.kind == ResourceKind::Mod) {
        use crate::engine::loader::LoaderKind;
        params.push(("loader", match loader {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
        }.into()));
    }
    let response = client.get(API).query(&params).send().await?;
    if !response.status().is_success() {
        return Err(Error::Other(match response.status().as_u16() {
            429 => "KLPBBS 请求过于频繁，请稍后重试",
            502..=504 => "KLPBBS 或上游资源服务暂时不可用，请稍后重试",
            _ => "KLPBBS 资源目录请求失败，请稍后重试",
        }.into()));
    }
    let envelope: Envelope = response.json().await?;
    if envelope.code != 200 {
        return Err(Error::Other("KLPBBS 资源目录返回了错误状态".into()));
    }
    let page = envelope.data.ok_or_else(|| Error::Other("KLPBBS 资源目录未返回数据".into()))?;
    Ok(SearchPage { total: page.pagination.total, hits: page.items.into_iter().map(Resource::into_project).collect::<Result<_>>()? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(source: &str, id: &str) -> Resource {
        serde_json::from_value(serde_json::json!({
            "source": source, "sourceId": id, "slug": "test-mod", "name": "Chinese title",
            "summary": "Localized summary", "downloads": "123456789",
            "sourceUrl": "javascript:alert(1)", "iconUrl": "file:///private/image.png"
        })).unwrap()
    }

    #[test]
    fn preserves_upstream_ids_and_normalizes_catalog_metadata() {
        let project = resource("curseforge", "238222").into_project().unwrap();
        assert_eq!(project.source, ResourceSource::CurseForge);
        assert_eq!(project.id, "238222");
        assert_eq!(project.downloads, 123456789);
        assert!(project.page_url.starts_with("https://www.curseforge.com/"));
        assert!(project.icon_url.is_none());
        let project = resource("modrinth", "AANobbMI").into_project().unwrap();
        assert_eq!(project.source, ResourceSource::Modrinth);
        assert!(project.page_url.starts_with("https://modrinth.com/"));
    }

    #[test]
    fn rejects_ids_that_could_change_the_upstream_api_path() {
        assert!(resource("curseforge", "../238222").into_project().is_err());
        assert!(resource("modrinth", "project/version").into_project().is_err());
        assert!(resource("curseforge", "").into_project().is_err());
    }
}
