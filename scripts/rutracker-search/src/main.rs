//! Anonymous, search-only evaluation of rutracker-api. No torrent downloads.

use rutracker_api::{Client, Order, Sort};
use serde_json::json;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let query = std::env::args().nth(1).unwrap_or_else(|| "[PS5]".into());
    let outcome = tokio::time::timeout(Duration::from_secs(45), search(&query)).await;
    match outcome {
        Ok(Ok(report)) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        other => {
            let message = match other {
                Ok(Err(error)) => error,
                Err(_) => "Search exceeded the 45-second overall timeout".into(),
                Ok(Ok(_)) => unreachable!(),
            };
            println!("{}", serde_json::to_string_pretty(&json!({
                "crate_version": "0.2.1",
                "query": query,
                "status": "failed",
                "error": message,
                "results": null,
                "note": "A failed search does not mean zero matching topics. No torrent or game files were downloaded."
            })).unwrap());
            std::process::exit(1);
        }
    }
}

async fn search(query: &str) -> Result<serde_json::Value, String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("Client construction failed: {error:?}"))?;
    let results = client.search(query)
        .sort(Sort::Seeds)
        .order(Order::Desc)
        .page(1)
        .send().await
        .map_err(|error| format!("Search failed: {error:?}"))?;
    let topics: Vec<_> = results.results.iter().map(|topic| json!({
        "id": topic.id.to_string(),
        "title": topic.title,
        "category": topic.category,
        "size_bytes": topic.size,
        "seeders": topic.seeds,
        "leechers": topic.leeches,
        "url": topic.url,
    })).collect();
    Ok(json!({
        "crate_version": "0.2.1",
        "query": query,
        "status": "ok",
        "total_count_reported": results.total_count,
        "page": results.page,
        "total_pages_reported": results.total_pages,
        "returned_count": topics.len(),
        "results": topics,
        "note": "First page only, ordered by seeders. Search metadata only; no torrent or game downloads."
    }))
}