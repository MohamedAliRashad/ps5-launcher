//! PS5 games catalog from superpsx.com's public WordPress API.

use crate::util::{atomic_write, cache_dir, http_json, now_secs, strip_tags};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

const SITE: &str = "https://www.superpsx.com";
const CATEGORY_SLUG: &str = "ps5-games";
pub const CATALOG_TTL: f64 = 6.0 * 3600.0;
pub const CATALOG_SCHEMA: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub date: String,
    pub link: String,
    pub cover: String,
    pub trailer: String,
    pub genres: Vec<String>,
    pub mode: String,
    pub release: String,
    pub size: String,
    pub size_gb: Option<f64>,
    pub version: String,
    pub title_id: String,
    pub update: String,
    pub excerpt: String,
    pub description: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CatalogFile {
    pub updated: f64,
    pub schema: u32,
    pub games: Vec<Game>,
}

impl CatalogFile {
    pub fn path() -> std::path::PathBuf {
        cache_dir().join("catalog.json")
    }

    pub fn load() -> CatalogFile {
        std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn stale(&self) -> bool {
        now_secs() - self.updated > CATALOG_TTL || self.schema != CATALOG_SCHEMA || self.games.is_empty()
    }
}

static YT_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?:youtube(?:-nocookie)?\.com/(?:embed/|watch\?v=)|youtu\.be/)([\w-]{11})").unwrap()
});
static ROW_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?is)<tr[^>]*>\s*<td[^>]*>(.*?)</td>\s*<td[^>]*>(.*?)</td>").unwrap());
static PARA_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap());
static SKIP_PARA_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?i)download|install|password|link").unwrap());
static SIZE_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?i)([\d.,]+)\s*(TB|GB|MB)").unwrap());
// Tolerates typos seen in posts such as "P PSA16106" and "PPPSA30140".
static TITLE_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:^|[^A-Z])(?:P\s*)+S\s*A\s*(\d{5})(?:\D|$)|(?:^|[^A-Z])C\s*U\s*S\s*A\s*(\d{5})(?:\D|$)").unwrap()
});
static PS5_SUFFIX_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\s+PS5$").unwrap());

pub fn find_title_id(texts: &[&str]) -> String {
    for t in texts {
        if let Some(c) = TITLE_ID_RE.captures(t) {
            if let Some(m) = c.get(1) {
                return format!("PPSA{}", m.as_str());
            }
            if let Some(m) = c.get(2) {
                return format!("CUSA{}", m.as_str());
            }
        }
    }
    String::new()
}

fn parse_size_gb(s: &str) -> Option<f64> {
    let c = SIZE_RE.captures(s)?;
    let n: f64 = c[1].replace(',', ".").parse().ok()?;
    Some(n * match c[2].to_uppercase().as_str() {
        "TB" => 1024.0,
        "MB" => 1.0 / 1024.0,
        _ => 1.0,
    })
}

fn parse_post(p: &serde_json::Value) -> Option<Game> {
    let content = p["content"]["rendered"].as_str().unwrap_or("");
    let mut info: Vec<(String, String)> = Vec::new();
    for c in ROW_RE.captures_iter(content) {
        let (k, v) = (strip_tags(&c[1]), strip_tags(&c[2]));
        if !k.is_empty() && !v.is_empty() && k.len() < 30 {
            info.push((k.to_lowercase(), v));
        }
    }
    let get = |k: &str| info.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();

    // Description: prose paragraphs only (no links, buttons or instructions).
    let mut description = Vec::new();
    for c in PARA_RE.captures_iter(content) {
        let t = strip_tags(&c[1]);
        if t.chars().count() < 60 || SKIP_PARA_RE.is_match(&t) {
            continue;
        }
        description.push(t);
        if description.len() >= 3 {
            break;
        }
    }

    let title = strip_tags(p["title"]["rendered"].as_str().unwrap_or(""));
    let game_name = get("game name");
    let name = if game_name.is_empty() { PS5_SUFFIX_RE.replace(&title, "").trim().to_string() } else { game_name };
    let version = get("version");
    let all_values: String = info.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>().join(" ");
    let size = get("size");
    Some(Game {
        id: p["id"].as_i64()?,
        name,
        title,
        date: p["date"].as_str().unwrap_or("").to_string(),
        link: p["link"].as_str().unwrap_or("").to_string(),
        cover: p["yoast_head_json"]["og_image"][0]["url"].as_str().unwrap_or("").to_string(),
        trailer: YT_RE.captures(content).map(|c| c[1].to_string()).unwrap_or_default(),
        genres: get("genre").split([',', '/', '|']).map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
        mode: get("mode"),
        release: get("release date"),
        size_gb: parse_size_gb(&size),
        size,
        title_id: find_title_id(&[&version, &all_values]),
        version,
        update: get("update"),
        excerpt: strip_tags(p["excerpt"]["rendered"].as_str().unwrap_or("")),
        description,
    })
}

/// Downloads the whole category (≈8 requests of 100 posts, fetched in parallel).
pub fn sync(progress: &dyn Fn(String)) -> Result<CatalogFile, String> {
    progress("Contacting server".into());
    let cats = http_json(&format!("{SITE}/wp-json/wp/v2/categories?slug={CATEGORY_SLUG}&_fields=id,count"))?;
    let cat_id = cats[0]["id"].as_i64().ok_or("category not found")?;
    let count = cats[0]["count"].as_i64().unwrap_or(100).max(1);
    let per = 100;
    let pages = ((count + per - 1) / per) as usize;
    let fields = "id,title,link,date,excerpt,content,yoast_head_json.og_image";

    let done = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<Result<Vec<Game>, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = (1..=pages)
            .map(|page| {
                let done = &done;
                s.spawn(move || {
                    let url = format!(
                        "{SITE}/wp-json/wp/v2/posts?categories={cat_id}&per_page={per}&page={page}&orderby=date&order=desc&_fields={fields}"
                    );
                    let mut last = String::new();
                    for attempt in 0..3 {
                        match http_json(&url) {
                            Ok(v) => {
                                done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                return Ok(v.as_array().map(|a| a.iter().filter_map(parse_post).collect()).unwrap_or_default());
                            }
                            Err(e) if e == "HTTP 400" => return Ok(Vec::new()), // past the last page
                            Err(e) => {
                                last = e;
                                std::thread::sleep(std::time::Duration::from_millis(1500 * (attempt + 1)));
                            }
                        }
                    }
                    Err(last)
                })
            })
            .collect();
        // Report progress while pages come in.
        loop {
            let n = done.load(std::sync::atomic::Ordering::Relaxed);
            progress(format!("Downloading catalog {n}/{pages}"));
            if handles.iter().all(|h| h.is_finished()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("worker panicked".into()))).collect()
    });

    let mut games = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for r in results {
        for g in r? {
            if seen.insert(g.id) {
                games.push(g);
            }
        }
    }
    games.sort_by(|a, b| b.date.cmp(&a.date));
    let file = CatalogFile { updated: now_secs(), schema: CATALOG_SCHEMA, games };
    if let Ok(json) = serde_json::to_vec(&file) {
        let _ = atomic_write(&CatalogFile::path(), &json);
    }
    crate::log!("catalog synced: {} games", file.games.len());
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_ids() {
        assert_eq!(find_title_id(&["P PSA16106 – EUR"]), "PPSA16106");
        assert_eq!(find_title_id(&["PPPSA30140 – EUR"]), "PPSA30140");
        assert_eq!(find_title_id(&["PPSA30094 – USA"]), "PPSA30094");
        assert_eq!(find_title_id(&["PS5 Digital Edition"]), "");
        assert_eq!(find_title_id(&["CUSA12345"]), "CUSA12345");
    }
    #[test]
    fn sizes() {
        assert_eq!(parse_size_gb("57 GB"), Some(57.0));
        assert_eq!(parse_size_gb("100 MB").map(|x| (x * 1024.0).round()), Some(100.0));
    }
}
