//! Welcome / boot screen. On the first run it downloads the catalog and official artwork
//! with a progress bar; every launch it waits (briefly) until the Home screen's tiles,
//! background and logo are decoded, so the UI never appears half-loaded.

use crate::app::*;
use crate::audio::{self, Sound};
use crate::images::prio;
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub const OVERLAY_BOOT: i32 = 6;

#[derive(Default)]
pub struct Boot {
    pub active: bool,
    pub first: bool,
    pub waiting_art: bool,
    pub ready: bool,
    pub keys: HashSet<String>,
    pub total: usize,
    pub started: Option<Instant>,
}

fn parse_progress(s: &str) -> Option<f32> {
    let (a, b) = s.rsplit_once(' ')?.1.split_once('/')?;
    let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
    (b > 0.0).then(|| (a / b).clamp(0.0, 1.0))
}

impl App {
    pub fn boot_start(&mut self, first: bool) {
        self.boot = Boot { active: true, first, waiting_art: first, started: Some(Instant::now()), ..Default::default() };
        let ui = self.ui();
        ui.set_overlay(OVERLAY_BOOT);
        ui.set_boot_first(first);
        ui.set_boot_ready(false);
        ui.set_boot_progress(0.0);
        let cfg = self.cfg.lock().unwrap().clone();
        let emu = if cfg.emulator_ok() { "KytyPS5 emulator found".to_string() } else { "KytyPS5 emulator not found · you can set it in Settings".to_string() };
        let n = self.locals.len();
        ui.set_boot_detail(format!("{emu} · {n} installed game{}", if n == 1 { "" } else { "s" }).into());
        ui.set_boot_status(if first { "Getting things ready…".into() } else { "Loading…".into() });
        if !first {
            self.boot_collect();
        }
        // Never keep a returning user waiting on a slow network.
        let limit = if first { 0 } else { 5 };
        if limit > 0 {
            slint::Timer::single_shot(Duration::from_secs(limit), || with_app(|app| app.boot_ready()));
        }
    }

    /// Status text from background work (catalog sync, artwork downloads) drives the progress bar.
    pub fn boot_status(&mut self, text: &str) {
        if !self.boot.active || !self.boot.first || self.boot.ready {
            return;
        }
        let p = parse_progress(text).unwrap_or(0.0);
        let overall = if text.contains("catalog") { 0.05 + p * 0.25 } else if text.contains("artwork") { 0.3 + p * 0.55 } else { return };
        let ui = self.ui();
        ui.set_boot_status(text.into());
        ui.set_boot_progress(overall.max(ui.get_boot_progress()));
    }

    /// Called when background artwork enrichment has finished.
    pub fn boot_art_done(&mut self) {
        if self.boot.active && self.boot.waiting_art {
            self.boot.waiting_art = false;
            self.boot_collect();
        }
    }

    /// Request everything the Home screen shows first and wait for it.
    pub fn boot_collect(&mut self) {
        if !self.boot.active || self.boot.ready {
            return;
        }
        if self.games.is_empty() && (self.syncing || self.boot.first) && self.boot.waiting_art {
            return;
        }
        let mut reqs = Vec::new();
        for (i, it) in self.row.clone().into_iter().enumerate() {
            if i < 12 {
                reqs.extend(self.tile_req(it));
            }
        }
        reqs.extend(self.hero_bg_url(self.sel));
        if let Some(it) = self.row.get(self.sel).copied() {
            if it != RowItem::All {
                let info = self.target_info(self.row_target(self.sel));
                if let Some(r) = info.as_ref().and_then(|i| crate::present::req_url(&i.logo, 960, 0.0)) {
                    reqs.push(r);
                }
            }
        }
        let mut keys = HashSet::new();
        for r in reqs {
            if self.images.want(&r.key, r.src.clone(), r.w, r.crop, prio::HERO).is_none() && self.images.is_pending(&r.key) {
                keys.insert(r.key);
            }
        }
        self.boot.total = keys.len();
        self.boot.keys = keys;
        let ui = self.ui();
        ui.set_boot_status("Preparing your games…".into());
        if self.boot.keys.is_empty() {
            self.boot_ready();
        }
    }

    pub fn boot_image_loaded(&mut self, key: &str) {
        if !self.boot.active || !self.boot.keys.remove(key) {
            return;
        }
        if self.boot.first {
            let done = 1.0 - self.boot.keys.len() as f32 / self.boot.total.max(1) as f32;
            self.ui().set_boot_progress(0.85 + done * 0.15);
        }
        if self.boot.keys.is_empty() {
            self.boot_ready();
        }
    }

    pub fn boot_ready(&mut self) {
        if !self.boot.active || self.boot.ready {
            return;
        }
        self.boot.ready = true;
        self.push_all();
        let ui = self.ui();
        ui.set_boot_progress(1.0);
        if self.boot.first {
            ui.set_boot_status("All set".into());
            ui.set_boot_ready(true);
            audio::play(Sound::Select);
        } else {
            // Keep the splash up for a short, smooth beat even when everything is cached.
            let min = Duration::from_millis(650);
            let wait = min.saturating_sub(self.boot.started.map(|s| s.elapsed()).unwrap_or(min));
            slint::Timer::single_shot(wait, || with_app(|app| app.boot_finish()));
        }
    }

    /// Confirm on the welcome screen (also allows skipping a slow first download).
    pub fn boot_confirm(&mut self) {
        if !self.boot.active {
            return;
        }
        if self.boot.first && !self.boot.ready {
            // Skip waiting: artwork keeps downloading in the background.
            self.boot.waiting_art = false;
            self.boot.ready = true;
            self.push_all();
        }
        audio::play(Sound::Start);
        self.boot_finish();
    }

    pub fn boot_finish(&mut self) {
        if !self.boot.active {
            return;
        }
        self.boot.active = false;
        self.overlay = Overlay::None;
        let ui = self.ui();
        ui.set_overlay(0);
        self.set_focus(Z_ROW, 0);
        self.want_background(self.hero_bg_url(self.sel), true);
        self.prefetch_neighbors();
        ui.invoke_focus_root();
    }
}
