//! Read-only smoke test for the public KLPBBS catalog; no keys, login, or downloads.

use godbbs_launcher_lib::engine::resources::{klpbbs, ResourceKind, SearchQuery, SortBy};
use godbbs_launcher_lib::engine::http_client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = SearchQuery {
        query: std::env::args().nth(1).unwrap_or_else(|| "sodium".into()),
        kind: ResourceKind::Mod,
        game_version: None,
        loader: None,
        sort: SortBy::Downloads,
        offset: 0,
        limit: 5,
    };
    let page = klpbbs::search(&http_client(), &query).await?;
    println!("KLPBBS: {} matches, {} returned", page.total, page.hits.len());
    for project in page.hits {
        println!("{:?} {}: {}", project.source, project.id, project.title);
    }
    Ok(())
}
