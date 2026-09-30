//! Official PlayStation artwork and metadata by title ID, with RAWG as a name-based fallback.
//!
//! Sony's public PlayStation App catalog API returns the same assets the PS5 shows: square
//! tile icon (MASTER), game hub background, transparent title logo, portrait cover,
//! screenshots, trailer, star rating. Cached in ~/.cache/ps5-launcher/psn.json (v1 format).

use crate::util::{atomic_write, cache_dir, clean_multiline, http_json, now_secs, query_escape};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

const API: &str = "https://m.np.playstation.com/api/catalog/v2/titles/{tid}_00/concepts/?age=99&country={c}&language={l}";
const FRESH: f64 = 14.0 * 86400.0;
const MISS_RETRY: f64 = 3.0 * 86400.0;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Rating {
    pub score: f64,
    pub total: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AgeRating {
    pub name: String,
    pub icon: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Info {
    pub source: Option<String>,
    pub concept: Option<Value>,
    pub name: String,
    pub publisher: String,
    pub genres: Vec<String>,
    pub release: String,
    pub rating: Option<Rating>,
    pub age: Option<AgeRating>,
    pub players: Option<Value>,
    pub short: String,
    pub long: String,
    pub master: String,
    pub hub: String,
    pub logo: String,
    pub portrait: String,
    pub bg: String,
    pub banner: String,
    pub shots: Vec<String>,
    pub video: String,
    pub poster: String,
    pub store: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Entry {
    pub at: f64,
    pub info: Option<Info>,
}

pub fn region_from_label(v: &str) -> &'static str {
    let v = v.to_uppercase();
    if v.contains("EUR") || v == "GB" || v == "UK" {
        "GB"
    } else if v.contains("JP") || v.contains("JAPAN") {
        "JP"
    } else if v.contains("ASIA") || v.contains("HK") {
        "HK"
    } else {
        "US"
    }
}

pub fn region_from_content_id(cid: &str) -> &'static str {
    match cid.get(..2) {
        Some("EP") => "GB",
        Some("JP") => "JP",
        Some("HP") | Some("KP") => "HK",
        _ => "US",
    }
}

fn store_for(region: &str) -> (&'static str, &'static str) {
    match region {
        "GB" => ("GB", "en-GB"),
        "JP" => ("JP", "ja-JP"),
        "HK" => ("HK", "en-HK"),
        _ => ("US", "en-US"),
    }
}

static TID_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^(PPSA|CUSA)\d{5}$").unwrap());

fn compact(c: &Value, lang: &str) -> Info {
    let mut imgs: HashMap<String, String> = HashMap::new();
    let mut shots = Vec::new();
    for i in c["media"]["images"].as_array().into_iter().flatten() {
        let (t, u) = (i["type"].as_str().unwrap_or(""), i["url"].as_str().unwrap_or(""));
        if u.is_empty() {
            continue;
        }
        if t == "SCREENSHOT" {
            shots.push(u.to_string());
        } else {
            imgs.entry(t.to_string()).or_insert_with(|| u.to_string());
        }
    }
    let video = c["media"]["videos"].as_array().into_iter().flatten().find(|v| v["url"].as_str().is_some());
    let desc = |kind: &str| {
        c["descriptions"].as_array().into_iter().flatten().find(|d| d["type"] == kind).and_then(|d| d["desc"].as_str()).map(clean_multiline).unwrap_or_default()
    };
    let rating = {
        let sr = &c["starRating"];
        let score = sr["score"].as_str().and_then(|s| s.parse::<f64>().ok()).or(sr["score"].as_f64());
        let total = sr["total"].as_str().and_then(|s| s.parse::<i64>().ok()).or(sr["total"].as_i64()).unwrap_or(0);
        score.filter(|_| total > 0).map(|s| Rating { score: (s * 100.0).round() / 100.0, total })
    };
    let cr = &c["contentRating"];
    let img = |k: &str| imgs.get(k).cloned().unwrap_or_default();
    Info {
        source: None,
        concept: c.get("id").cloned(),
        name: c["nameEn"].as_str().or(c["name"].as_str()).unwrap_or("").to_string(),
        publisher: c["publisherName"].as_str().unwrap_or("").to_string(),
        genres: c["localizedGenres"].as_array().into_iter().flatten().filter_map(|g| g["value"].as_str().map(String::from)).collect(),
        release: c["releaseDate"]["date"].as_str().unwrap_or("").chars().take(10).collect(),
        rating,
        age: cr.as_object().map(|_| AgeRating {
            name: cr["description"].as_str().unwrap_or("").to_string(),
            icon: cr["url"].as_str().unwrap_or("").to_string(),
        }),
        players: c["compatibilityNotices"].as_array().into_iter().flatten().find(|n| n["type"] == "NO_OF_PLAYERS").map(|n| n["value"].clone()),
        short: desc("SHORT"),
        long: desc("LONG"),
        master: img("MASTER"),
        hub: img("GAMEHUB_COVER_ART"),
        logo: img("LOGO"),
        portrait: img("PORTRAIT_BANNER"),
        bg: img("BACKGROUND_LAYER_ART"),
        banner: img("FOUR_BY_THREE_BANNER"),
        shots: shots.into_iter().take(12).collect(),
        video: video.and_then(|v| v["url"].as_str()).unwrap_or("").to_string(),
        poster: video.and_then(|v| v["posters"][0].as_str()).unwrap_or("").to_string(),
        store: c.get("id").map(|id| format!("https://store.playstation.com/{}/concept/{}", lang.to_lowercase(), id)).unwrap_or_default(),
    }
}

fn fetch_sony(tid: &str, region: &str) -> Option<Info> {
    let mut order = vec![region];
    order.extend(["US", "GB", "HK", "JP"].into_iter().filter(|r| *r != region));
    for r in order {
        let (country, lang) = store_for(r);
        let url = API.replace("{tid}", tid).replace("{c}", country).replace("{l}", lang);
        let Ok(items) = http_json(&url) else { continue };
        let Some(arr) = items.as_array() else { continue };
        let game = arr.iter().find(|c| c["type"] == "GAME").or(arr.first());
        if let Some(c) = game {
            return Some(compact(c, lang));
        }
    }
    None
}

// ------------------------------------------------------------------ RAWG

pub fn rawg_norm(s: &str) -> String {
    static JUNK: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)\b(ps5|ps4|playstation 5|remastered edition|digital edition)\b|[™®©]").unwrap());
    crate::util::norm(&JUNK.replace_all(s, " "))
}

pub fn rawg_cache_key(name: &str) -> String {
    format!("rawg:{}", rawg_norm(name))
}

fn fetch_rawg(name: &str, key: &str) -> Option<Info> {
    let k = query_escape(key);
    let want = rawg_norm(name);
    // PS5 listings first; many ports are only tagged for PC/Switch on RAWG, so then any platform.
    let mut best: Option<Value> = None;
    for platform in ["&platforms=187", ""] {
        let Ok(res) = http_json(&format!(
            "https://api.rawg.io/api/games?key={k}&search={}{platform}&page_size=6&search_precise=true",
            query_escape(name)
        )) else { continue };
        let results = res["results"].as_array().cloned().unwrap_or_default();
        let hit = results.iter().find(|r| rawg_norm(r["name"].as_str().unwrap_or("")) == want).or_else(|| {
            results.iter().find(|r| {
                let n = rawg_norm(r["name"].as_str().unwrap_or(""));
                // Partial matches only when the names are nearly the same ("Let's Build" is a
                // different game from "Let's Build a Zoo").
                let (short, long) = if n.len() < want.len() { (&n, &want) } else { (&want, &n) };
                !short.is_empty() && long.contains(short.as_str()) && short.len() * 10 >= long.len() * 8
            })
        });
        if let Some(h) = hit {
            best = Some(h.clone());
            break;
        }
    }
    let best = &best?;
    let full = http_json(&format!("https://api.rawg.io/api/games/{}?key={k}", best["id"])).unwrap_or_else(|_| best.clone());
    let rating = match (full["rating"].as_f64(), full["ratings_count"].as_i64()) {
        (Some(s), Some(t)) if s > 0.0 && t > 0 => Some(Rating { score: (s * 100.0).round() / 100.0, total: t }),
        _ => None,
    };
    let list = |v: &Value| -> Vec<String> { v.as_array().into_iter().flatten().filter_map(|x| x["name"].as_str().map(String::from)).collect() };
    Some(Info {
        source: Some("rawg".into()),
        name: full["name"].as_str().unwrap_or(name).to_string(),
        publisher: list(&full["publishers"]).join(", "),
        genres: list(&full["genres"]),
        release: full["released"].as_str().unwrap_or("").to_string(),
        rating,
        age: full["esrb_rating"]["name"].as_str().map(|n| AgeRating { name: format!("ESRB {n}"), icon: String::new() }),
        long: full["description_raw"].as_str().unwrap_or("").trim().to_string(),
        hub: full["background_image"].as_str().unwrap_or("").to_string(),
        bg: full["background_image_additional"].as_str().unwrap_or("").to_string(),
        shots: best["short_screenshots"].as_array().into_iter().flatten().skip(1).take(12).filter_map(|s| s["image"].as_str().map(String::from)).collect(),
        ..Default::default()
    })
}

/// Look one game up on RAWG now (uses the cache if it already has a result).
pub fn rawg_lookup(store: &Shared, name: &str, key: &str) -> Option<Info> {
    let k = rawg_cache_key(name);
    if let Some(i) = store.lock().unwrap().get(&k) {
        return Some(i.clone());
    }
    let info = fetch_rawg(name, key);
    store.lock().unwrap().put(k, info.clone());
    info
}

/// Returns "" if RAWG accepts the key, otherwise a message.
pub fn check_rawg_key(key: &str) -> String {
    match crate::util::http_get(&format!("https://api.rawg.io/api/platforms?page_size=1&key={}", query_escape(key))) {
        Ok(_) => String::new(),
        Err(e) if e == "HTTP 401" || e == "HTTP 403" => "RAWG rejected this key".into(),
        Err(e) if e.starts_with("HTTP") => format!("RAWG returned {e}"),
        Err(e) => format!("Could not reach RAWG: {e}"),
    }
}

// ------------------------------------------------------------------ cache

#[derive(Default)]
pub struct Store {
    pub data: HashMap<String, Entry>,
    dirty: usize,
}

pub type Shared = Arc<Mutex<Store>>;

impl Store {
    fn path() -> std::path::PathBuf {
        cache_dir().join("psn.json")
    }

    pub fn load() -> Shared {
        let data = std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Arc::new(Mutex::new(Store { data, dirty: 0 }))
    }

    pub fn save(&mut self) {
        self.dirty = 0;
        if let Ok(json) = serde_json::to_vec(&self.data) {
            let _ = atomic_write(&Self::path(), &json);
        }
    }

    pub fn get(&self, key: &str) -> Option<&Info> {
        self.data.get(key).and_then(|e| e.info.as_ref())
    }

    pub fn fresh(&self, key: &str) -> bool {
        self.data.get(key).is_some_and(|e| now_secs() - e.at < if e.info.is_some() { FRESH } else { MISS_RETRY })
    }

    fn put(&mut self, key: String, info: Option<Info>) {
        let prev = self.data.get(&key).and_then(|e| e.info.clone());
        // Keep old data if the service is temporarily unreachable.
        self.data.insert(key, Entry { at: now_secs(), info: info.or(prev) });
        self.dirty += 1;
        if self.dirty >= 40 {
            self.save();
        }
    }
}

pub fn valid_title_id(tid: &str) -> bool {
    TID_RE.is_match(tid)
}

/// Fetch missing/stale Sony entries in parallel, then RAWG for names still lacking art.
/// `progress` receives status text; returns true if anything changed.
pub fn enrich(store: &Shared, titles: Vec<(String, &'static str)>, rawg: Option<(String, Vec<String>)>, progress: &(dyn Fn(String) + Sync)) -> bool {
    let todo: Vec<(String, &str)> = {
        let s = store.lock().unwrap();
        let mut seen = std::collections::HashSet::new();
        titles.into_iter().filter(|(t, _)| valid_title_id(t) && !s.fresh(t) && seen.insert(t.clone())).collect()
    };
    let mut changed = false;
    if !todo.is_empty() {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let done = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..6 {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((tid, region)) = todo.get(i) else { break };
                    let info = fetch_sony(tid, region);
                    store.lock().unwrap().put(tid.clone(), info);
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    progress(format!("Downloading artwork {n}/{}", todo.len()));
                });
            }
        });
        changed = true;
    }
    if let Some((key, names)) = rawg {
        let names: Vec<String> = {
            let s = store.lock().unwrap();
            names.into_iter().filter(|n| !s.fresh(&rawg_cache_key(n))).collect()
        };
        for (i, name) in names.iter().enumerate() {
            progress(format!("Downloading artwork (RAWG) {}/{}", i + 1, names.len()));
            let info = fetch_rawg(name, &key);
            store.lock().unwrap().put(rawg_cache_key(name), info);
            changed = true;
        }
    }
    if changed {
        store.lock().unwrap().save();
    }
    changed
}

/// Sony image CDN resizes on request; other hosts ignore the parameter, so only add it there.
pub fn sized(url: &str, w: u32) -> String {
    if url.contains("image.api.playstation.com") && !url.contains('?') {
        format!("{url}?w={w}")
    } else {
        url.to_string()
    }
}
