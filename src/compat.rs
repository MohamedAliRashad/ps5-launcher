//! KytyPS5 compatibility reports from the community list (kytyps5.github.io), by title ID.
//! Same data KytyPS5's own launcher uses. Cached in ~/.cache/ps5-launcher/compatibility.json.

use crate::util::{atomic_write, cache_dir, http_get};
use std::collections::HashMap;

const URL: &str = "https://kytyps5.github.io/data/compatibility.json";
pub const LIST_PAGE: &str = "https://kytyps5.github.io/";
pub const REFRESH: f64 = 6.0 * 3600.0;
/// KytyPS5's "Game Emulation Status Report" form; reports there feed the list above.
const REPORT_FORM: &str = "https://github.com/KytyPS5/KytyPS5/issues/new?template=kytyps5-game-emulation.yaml";

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
    /// The Linux result when someone tested on Linux; otherwise the best result elsewhere
    /// (`on_linux` says which). The list's own top-level status is the best of any OS, which
    /// would let a Windows "In-game" hide a Linux "Doesn't boot".
    pub status: Status,
    pub on_linux: bool,
    /// Status on Windows, when someone tested there.
    pub windows: Option<Status>,
    pub reports: u64,
    /// Kyty build it was last tested on, e.g. "KytyPS5-2026-08-16-bc2f077".
    pub version: String,
    /// Platforms with reports, e.g. ["Linux", "Windows"].
    pub platforms: Vec<String>,
}

impl Entry {
    /// Where `status` comes from: "Linux", or the OS it was tested on instead.
    pub fn source(&self) -> &'static str {
        if self.on_linux {
            "Linux"
        } else if self.windows.is_some() {
            "Windows"
        } else if self.platforms.iter().any(|p| p == "macOS") {
            "macOS"
        } else {
            "another OS"
        }
    }

    /// Short tag for Library covers: "In-game", or "Win · In-game" for a result from elsewhere.
    pub fn tag(&self) -> String {
        match self.source() {
            "Linux" => self.status.label().to_string(),
            "Windows" => format!("Win · {}", self.status.label()),
            "macOS" => format!("Mac · {}", self.status.label()),
            _ => format!("Other OS · {}", self.status.label()),
        }
    }

    /// Chip text: "In-game on Linux", or "In-game on Windows · untested on Linux".
    pub fn chip_text(&self) -> String {
        if self.on_linux {
            format!("{} on Linux", self.status.label())
        } else {
            format!("{} on {} · untested on Linux", self.status.label(), self.source())
        }
    }

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
        let Some(best) = e["status"].as_str().and_then(Status::parse) else { continue };
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
        let on = |os: &str| plat.and_then(|p| p.get(os)).and_then(|l| l["status"].as_str()).and_then(Status::parse);
        let linux = on("linux");
        db.insert(
            tid.to_uppercase(),
            Entry {
                status: linux.unwrap_or(best),
                on_linux: linux.is_some(),
                windows: on("windows"),
                reports: e["reports"].as_u64().unwrap_or(1),
                version: plat
                    .and_then(|p| p.values().filter_map(|x| x["version"].as_str()).max_by_key(|s| s.len()).map(String::from))
                    .unwrap_or_default(),
                platforms,
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

/// The report form, pre-filled with the game, this PC and the end of the emulator log. The
/// player still picks the result and attaches the full log; nothing is sent until they submit.
pub fn report_url(name: &str, title_id: &str, kyty_version: &str, log: &str) -> String {
    let q = crate::util::query_escape;
    let mut url = format!("{REPORT_FORM}&title={}", q(&format!("[GAME STATUS]: {name} (linux)")));
    let sys = system_info();
    let log = log_excerpt(log);
    for (field, value) in [
        ("game-title", name),
        ("game-id", title_id),
        ("kyty-version", kyty_version),
        ("os", &sys.os),
        ("cpu", &sys.cpu),
        ("gpu", &sys.gpu),
        ("ram-vram", &sys.ram),
        ("log-file", &log),
    ] {
        if !value.is_empty() {
            url.push_str(&format!("&{field}={}", q(value)));
        }
    }
    url
}

/// The last lines of a log, sized to keep the link well under GitHub's URL limit.
fn log_excerpt(log: &str) -> String {
    let lines: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut tail: Vec<&str> = Vec::new();
    let mut len = 0;
    for line in lines.iter().rev().take(40) {
        len += line.len() + 1;
        if len > 2500 {
            break;
        }
        tail.push(line);
    }
    if tail.is_empty() {
        return String::new();
    }
    tail.reverse();
    format!("Last lines of the log (full log attached below):\n```\n{}\n```\n", tail.join("\n"))
}

#[derive(Default)]
struct SystemInfo {
    os: String,
    cpu: String,
    gpu: String,
    ram: String,
}

fn system_info() -> SystemInfo {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let field = |text: &str, key: &str| {
        text.lines().find_map(|l| l.strip_prefix(key)).map(|v| v.trim_start_matches([' ', '\t', ':', '=']).trim().trim_matches('"').to_string()).unwrap_or_default()
    };
    let mut os = field(&read("/etc/os-release"), "PRETTY_NAME");
    let kernel = read("/proc/sys/kernel/osrelease").trim().to_string();
    if !kernel.is_empty() {
        os = format!("{os} (kernel {kernel})").trim().to_string();
    }
    let ram = field(&read("/proc/meminfo"), "MemTotal")
        .trim_end_matches("kB")
        .trim()
        .parse::<f64>()
        .map(|kb| format!("{:.0} GB RAM", kb / 1024.0 / 1024.0))
        .unwrap_or_default();
    // lspci names the graphics card; it's optional, so leave the field empty without it.
    let gpu = std::process::Command::new("lspci")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("VGA compatible controller") || l.contains("3D controller") || l.contains("Display controller"))
        .filter_map(|l| l.splitn(3, ':').nth(2).map(|s| s.trim().to_string()))
        .collect::<Vec<_>>()
        .join("; ");
    SystemInfo { os, cpu: field(&read("/proc/cpuinfo"), "model name"), gpu, ram }
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
        // Linux wins over a better Windows result.
        assert_eq!(e.status, Status::MainMenu);
        assert!(e.on_linux);
        assert_eq!(e.windows, Some(Status::InGame));
        assert_eq!(e.tag(), "Menus");
        assert_eq!(e.platforms, vec!["Linux", "Windows"]);
        assert_eq!(e.tested_date(), "2026-08-18");
        assert!(Status::InGame < Status::DoesntBoot);
    }

    #[test]
    fn windows_only_results_are_marked() {
        let json = br#"{"PPSA1":{"status":"InGame","platforms":{"windows":{"status":"InGame"}}},"PPSA2":{"status":"Logo"}}"#;
        let db = parse(json).unwrap();
        let w = &db["PPSA1"];
        assert!(!w.on_linux);
        assert_eq!(w.tag(), "Win · In-game");
        assert_eq!(w.chip_text(), "In-game on Windows · untested on Linux");
        assert_eq!(db["PPSA2"].tag(), "Other OS · Boots");
    }

    #[test]
    fn report_url_prefills_the_form() {
        let url = report_url("Dreaming Sarah", "PPSA02929", "KytyPS5-2026-09-30-b7a1fac", "boot\n\nCould not find suitable device\n");
        assert!(url.starts_with(REPORT_FORM));
        assert!(url.contains("&game-id=PPSA02929"));
        assert!(url.contains("&game-title=Dreaming%20Sarah"));
        assert!(url.contains("&kyty-version=KytyPS5%2D2026%2D09%2D30%2Db7a1fac"));
        assert!(!url.contains("compatibility-status"));
        assert!(url.contains("&log-file="));
        assert!(url.contains("Could%20not%20find%20suitable%20device"));
        assert!(report_url("A", "B", "C", "").find("log-file").is_none());
        let long = "x".repeat(200) + "\n";
        assert!(log_excerpt(&long.repeat(100)).len() < 2600);
    }
}
