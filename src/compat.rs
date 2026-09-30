//! KytyPS5 compatibility reports from the community list (kytyps5.github.io), by title ID.
//! Same data KytyPS5's own launcher uses. Cached in ~/.cache/ps5-launcher/compatibility.json.

use crate::util::{atomic_write, cache_dir, http_get};
use std::collections::HashMap;

const URL: &str = "https://kytyps5.github.io/data/compatibility.json";
pub const LIST_PAGE: &str = "https://kytyps5.github.io/";
pub const REFRESH: f64 = 6.0 * 3600.0;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Status {
    // Ordered best first (used for sorting).
    InGame,
    MainMenu,
    Logo,
    DoesntBoot,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        Some(match s {
            "InGame" => Status::InGame,
            "MainMenu" => Status::MainMenu,
            "Logo" => Status::Logo,
            "DoesntBoot" => Status::DoesntBoot,
            _ => return None,
        })
    }

    /// Short label for badges and chips.
    pub fn label(self) -> &'static str {
        match self {
            Status::InGame => "In-game",
            Status::MainMenu => "Menus",
            Status::Logo => "Boots",
            Status::DoesntBoot => "Doesn't boot",
        }
    }

    /// One-line explanation for the Game Hub.
    pub fn meaning(self) -> &'static str {
        match self {
            Status::InGame => "Reaches gameplay",
            Status::MainMenu => "Main menu, not gameplay",
            Status::Logo => "Intro logos, then stops",
            Status::DoesntBoot => "Doesn't start",
        }
    }

    /// Index into the colour palette in ui/theme.slint (Theme.compat-color).
    pub fn level(self) -> i32 {
        match self {
            Status::InGame => 1,
            Status::MainMenu => 2,
            Status::Logo => 3,
            Status::DoesntBoot => 4,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub status: Status,
    pub reports: u64,
    /// Kyty build it was last tested on, e.g. "KytyPS5-2026-08-16-bc2f077".
    pub version: String,
    /// Platforms with reports, e.g. ["Linux", "Windows"].
    pub platforms: Vec<String>,
    /// Status on Linux specifically, when someone tested there.
    pub linux: Option<Status>,
}

impl Entry {
    /// "2026-08-16" out of whatever version text the report has.
    pub fn tested_date(&self) -> String {
        let v = &self.version;
        for (i, _) in v.char_indices() {
            let s = &v[i..];
            if s.len() >= 10 && crate::util::parse_iso_date(s).is_some() && s.as_bytes()[4] == b'-' {
                return s[..10].to_string();
            }
        }
        v.trim_start_matches('v').to_string()
    }
}

pub type Db = HashMap<String, Entry>;

fn path() -> std::path::PathBuf {
    cache_dir().join("compatibility.json")
}

fn parse(bytes: &[u8]) -> Option<Db> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let mut db = Db::new();
    for (tid, e) in v.as_object()? {
        let Some(status) = e["status"].as_str().and_then(Status::parse) else { continue };
        let plat = e["platforms"].as_object();
        let mut platforms: Vec<String> = plat
            .map(|p| {
                p.keys()
                    .map(|k| match k.as_str() {
                        "linux" => "Linux".into(),
                        "windows" => "Windows".into(),
                        "macos" => "macOS".into(),
                        o => o.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        platforms.sort();
        db.insert(
            tid.to_uppercase(),
            Entry {
                status,
                reports: e["reports"].as_u64().unwrap_or(1),
                version: plat
                    .and_then(|p| p.values().filter_map(|x| x["version"].as_str()).max_by_key(|s| s.len()).map(String::from))
                    .unwrap_or_default(),
                platforms,
                linux: plat.and_then(|p| p.get("linux")).and_then(|l| l["status"].as_str()).and_then(Status::parse),
            },
        );
    }
    Some(db)
}

/// Cached copy (instant) and its age in seconds.
pub fn load() -> (Db, f64) {
    let age = std::fs::metadata(path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(f64::MAX);
    let db = std::fs::read(path()).ok().and_then(|b| parse(&b)).unwrap_or_default();
    (db, age)
}

pub fn fetch() -> Result<Db, String> {
    let bytes = http_get(URL)?;
    let db = parse(&bytes).ok_or("unexpected data")?;
    let _ = atomic_write(&path(), &bytes);
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_feed() {
        let json = br#"{"PPSA02929":{"status":"InGame","reports":2,"platforms":{"windows":{"status":"InGame","version":"0.2.2 (KytyPS5-2026-08-18-6bf6929)"},"linux":{"status":"MainMenu","version":"v0.3.0"}}},"PPSA1":{"status":"Weird"}}"#;
        let db = parse(json).unwrap();
        assert_eq!(db.len(), 1);
        let e = &db["PPSA02929"];
        assert_eq!(e.status, Status::InGame);
        assert_eq!(e.linux, Some(Status::MainMenu));
        assert_eq!(e.platforms, vec!["Linux", "Windows"]);
        assert_eq!(e.tested_date(), "2026-08-18");
        assert!(Status::InGame < Status::DoesntBoot);
    }
}
