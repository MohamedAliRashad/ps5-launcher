//! User settings, stored in ~/.config/ps5-launcher/config.json (same format as v1).

use crate::util::{atomic_write, config_dir, expand_home};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub emulator: String,
    pub game_dirs: Vec<String>,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    pub present_mode: String,
    pub amd_cpu: bool,
    pub extra_args: String,
    pub sounds: bool,
    pub return_on_exit: bool,
    pub rawg_key: String,
    /// Output name (e.g. "DP-2"); empty = primary display.
    pub monitor: String,
    /// Keep the launcher-managed KytyPS5 on the latest official build.
    pub kyty_auto_update: bool,
    /// Install new PS5 Launcher releases automatically (applied on the next start).
    pub app_auto_update: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            emulator: String::new(),
            game_dirs: vec!["~/Games/PS5".into()],
            fullscreen: true,
            width: 1920,
            height: 1080,
            present_mode: "Mailbox".into(),
            amd_cpu: false,
            extra_args: String::new(),
            sounds: true,
            return_on_exit: true,
            rawg_key: String::new(),
            monitor: String::new(),
            kyty_auto_update: true,
            app_auto_update: true,
        }
    }
}

pub const RESOLUTIONS: [(u32, u32); 5] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];
pub const PRESENT_MODES: [(&str, &str); 3] =
    [("Mailbox", "Mailbox (low latency)"), ("Fifo", "Fifo (V-Sync)"), ("Immediate", "Immediate (uncapped)")];

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.json")
    }

    pub fn load() -> Config {
        let mut cfg: Config = std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if cfg.emulator.is_empty() || !cfg.emulator_path().is_file() {
            let managed = crate::kyty::managed_emulator();
            if managed.is_file() {
                cfg.emulator = managed.to_string_lossy().into_owned();
            } else if let Some(found) = detect_emulator() {
                cfg.emulator = found.to_string_lossy().into_owned();
            }
        }
        cfg
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            if let Err(e) = atomic_write(&Self::path(), &json) {
                crate::log!("could not save config: {e}");
            }
        }
    }

    pub fn emulator_path(&self) -> PathBuf {
        expand_home(self.emulator.trim())
    }

    pub fn emulator_ok(&self) -> bool {
        let p = self.emulator_path();
        std::fs::metadata(&p).map(|m| m.is_file() && is_executable(&m)).unwrap_or(false)
    }

    pub fn game_dir_paths(&self) -> Vec<PathBuf> {
        self.game_dirs.iter().filter(|d| !d.trim().is_empty()).map(|d| expand_home(d.trim())).collect()
    }
}

fn is_executable(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    m.permissions().mode() & 0o111 != 0
}

/// Look for kyty_emulator in PATH and a few common build locations.
pub fn detect_emulator() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join("kyty_emulator")));
    }
    let home = expand_home("~");
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut roots = vec![home.clone(), home.join("projects"), home.join("Games"), home.join("Applications"), home.join("src")];
    if let Some(d) = &exe_dir {
        roots.extend(d.ancestors().take(4).map(|p| p.to_path_buf()));
    }
    for root in roots {
        for sub in ["KytyPS5", "kyty", "Kyty"] {
            for build in ["_Build/release-linux", "_Build/linux/install", "_Build/linux", "build", ""] {
                candidates.push(root.join(sub).join(build).join("kyty_emulator"));
            }
        }
        // One level deeper, e.g. ~/projects/PS5/KytyPS5
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten().take(200) {
                let p = e.path();
                if p.is_dir() {
                    for build in ["KytyPS5/_Build/release-linux", "KytyPS5/_Build/linux/install"] {
                        candidates.push(p.join(build).join("kyty_emulator"));
                    }
                }
            }
        }
    }
    candidates.into_iter().find(|p| std::fs::metadata(p).map(|m| m.is_file() && is_executable(&m)).unwrap_or(false))
}
