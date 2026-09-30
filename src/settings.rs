//! Settings sheet: rows, controller-friendly editing, instant save.

use crate::app::*;
use crate::audio::{self, Sound};
use crate::config::{PRESENT_MODES, RESOLUTIONS};
use crate::util;
use crate::SettingData;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SId {
    Header,
    Emulator,
    KytyUpdate,
    KytyAuto,
    KytyRollback,
    Dirs,
    DownloadDir,
    InstallDir,
    SeedCompleted,
    Downloads,
    Resolution,
    Present,
    Fullscreen,
    Amd,
    Extra,
    ReturnOnExit,
    AppUpdate,
    AppAuto,
    Display,
    Sounds,
    Rawg,
    RawgRemove,
    Refresh,
    Rescan,
    Quit,
}

fn row(kind: i32, label: &str) -> SettingData {
    SettingData { kind, label: label.into(), ..Default::default() }
}

impl App {
    pub fn build_settings(&mut self) {
        let cfg = self.cfg.lock().unwrap().clone();
        let mut rows: Vec<(SId, SettingData)> = Vec::new();
        rows.push((SId::Header, row(0, "EMULATOR")));

        let mut r = row(1, "KytyPS5 executable");
        r.value = util::display_path(&cfg.emulator).into();
        let ok = cfg.emulator_ok();
        r.hint = if ok { "Emulator found".into() } else { "Emulator not found at this path".into() };
        r.hint_kind = if ok { 1 } else { 2 };
        rows.push((SId::Emulator, r));

        // KytyPS5 updates
        let managed = self.kyty_managed();
        let latest = self.kyty.latest.as_ref().map(|r| crate::kyty::pretty(&r.tag));
        let (label, value) = if self.kyty.busy {
            ("Updating KytyPS5…".to_string(), self.kyty.progress.replace("Downloading KytyPS5 ", ""))
        } else if self.kyty.checking {
            ("Checking for KytyPS5 updates…".to_string(), String::new())
        } else if !managed && !ok {
            ("Download KytyPS5".to_string(), latest.clone().map(|l| format!("latest {l}")).unwrap_or_else(|| "latest official build".into()))
        } else if !managed {
            ("Switch to official KytyPS5 builds".to_string(), "auto-updates · saves are copied".to_string())
        } else if self.kyty_update_available() {
            ("Update KytyPS5".to_string(), latest.clone().map(|l| format!("→ {l}")).unwrap_or_default())
        } else {
            ("Check for KytyPS5 updates".to_string(), if latest.is_some() { "up to date".into() } else { String::new() })
        };
        let mut r = row(4, &label);
        r.value = value.into();
        if !self.kyty.error.is_empty() {
            r.hint = self.kyty.error.clone().into();
            r.hint_kind = 2;
        } else {
            let st = crate::kyty::load_state();
            let checked = if st.last_check > 0.0 { format!(" · checked {}", util::fmt_last_played(st.last_check).to_lowercase()) } else { String::new() };
            r.hint = format!("Installed: {}{checked}", if self.kyty.version.is_empty() { "…" } else { &self.kyty.version }).into();
            r.hint_kind = if managed && !self.kyty_update_available() { 1 } else { 0 };
        }
        rows.push((SId::KytyUpdate, r));
        let mut r = row(2, "Keep KytyPS5 updated automatically");
        r.on = cfg.kyty_auto_update;
        rows.push((SId::KytyAuto, r));
        let prev = crate::kyty::load_state().previous;
        if managed && !prev.is_empty() && crate::kyty::root().join("versions").join(&prev).is_dir() {
            let mut r = row(4, "Roll back to previous KytyPS5");
            r.value = crate::kyty::pretty(&prev).into();
            rows.push((SId::KytyRollback, r));
        }

        let mut r = row(1, "Game folders (separate with ;)");
        r.value = cfg.game_dirs.iter().map(|d| util::display_path(d)).collect::<Vec<_>>().join("; ").into();
        let n = self.locals.len();
        r.hint = format!("{n} installed game{} found · scanned for sce_sys/param.json", if n == 1 { "" } else { "s" }).into();
        rows.push((SId::Dirs, r));

        let mut r = row(3, "Resolution");
        r.value = format!("{} × {}", cfg.width, cfg.height).into();
        rows.push((SId::Resolution, r));

        let mut r = row(3, "Present mode");
        r.value = PRESENT_MODES.iter().find(|(k, _)| *k == cfg.present_mode).map(|(_, l)| *l).unwrap_or(cfg.present_mode.as_str()).into();
        rows.push((SId::Present, r));

        let mut r = row(2, "Launch games in fullscreen");
        r.on = cfg.fullscreen;
        rows.push((SId::Fullscreen, r));
        let mut r = row(2, "AMD CPU instruction patches");
        r.on = cfg.amd_cpu;
        rows.push((SId::Amd, r));
        let mut r = row(1, "Extra emulator arguments");
        r.value = cfg.extra_args.clone().into();
        r.hint = "e.g. --vblank-frequency 60 --tessellation".into();
        rows.push((SId::Extra, r));
        let mut r = row(2, "Return to the launcher when a game closes");
        r.on = cfg.return_on_exit;
        rows.push((SId::ReturnOnExit, r));

        rows.push((SId::Header, row(0, "LAUNCHER")));

        let mut r = row(1, "Download folder");
        r.value = cfg.download_dir.clone().into();
        r.hint = "New transfers only · each torrent gets its own folder · not installed automatically".into();
        rows.push((SId::DownloadDir, r));
        let mut r = row(1, "Installation folder");
        r.value = cfg.install_dir.clone().into();
        r.hint = "Explicit Install after download · originals kept · added to the game library after validation".into();
        rows.push((SId::InstallDir, r));
        let mut r = row(2, "Seed completed downloads");
        r.on = cfg.seed_after_download;
        r.hint = "On by default · shares original downloads while open · upload limit 128 KiB/s · off stops current seeds".into();
        rows.push((SId::SeedCompleted, r));
        let mut r = row(4, "Manage downloads");
        r.hint = "Background while the launcher is open · paused on exit · Ctrl+D".into();
        rows.push((SId::Downloads, r));

        // PS5 Launcher updates
        let cur = crate::update::current_version();
        let latest = self.upd.latest.as_ref().map(|r| r.version.clone());
        let (label, value) = if let Some(v) = &self.upd.installed {
            ("Restart now to finish updating".to_string(), format!("{cur} → {v}"))
        } else if self.upd.busy {
            ("Updating PS5 Launcher…".to_string(), self.upd.progress.replace("Downloading PS5 Launcher ", ""))
        } else if self.upd.checking {
            ("Checking for launcher updates…".to_string(), String::new())
        } else if self.app_update_available() {
            ("Update PS5 Launcher".to_string(), latest.as_ref().map(|l| format!("{cur} → {l}")).unwrap_or_default())
        } else {
            ("Check for launcher updates".to_string(), if latest.is_some() { "up to date".into() } else { String::new() })
        };
        let mut r = row(4, &label);
        r.value = value.into();
        if !self.upd.error.is_empty() {
            r.hint = self.upd.error.clone().into();
            r.hint_kind = 2;
        } else {
            r.hint = format!("Version {cur}").into();
            r.hint_kind = if self.upd.installed.is_some() || (latest.is_some() && !self.app_update_available()) { 1 } else { 0 };
        }
        rows.push((SId::AppUpdate, r));
        let mut r = row(2, "Keep PS5 Launcher updated automatically");
        r.on = cfg.app_auto_update;
        rows.push((SId::AppAuto, r));

        let mut r = row(3, "Display");
        r.value = match self.monitors.iter().find(|m| m.name == cfg.monitor) {
            Some(m) => format!("{} · {}×{}", m.name, m.w, m.h),
            None => "Primary display".into(),
        }
        .into();
        rows.push((SId::Display, r));
        let mut r = row(2, "Interface sounds");
        r.on = cfg.sounds;
        rows.push((SId::Sounds, r));

        let mut r = row(1, "RAWG API key");
        r.secret = true;
        let key = cfg.rawg_key.trim();
        r.value = if key.is_empty() { "".into() } else if key.len() >= 8 { format!("••••••••{}", &key[key.len() - 4..]).into() } else { "••••".into() };
        let rawg_games = self.games.iter().filter(|g| g.info.as_ref().and_then(|i| i.source.as_deref()) == Some("rawg")).count();
        if !self.rawg_status.is_empty() {
            r.hint = self.rawg_status.clone().into();
            r.hint_kind = self.rawg_status_kind;
        } else if key.is_empty() {
            r.hint = "Optional. Fills in artwork for games missing from the PlayStation catalog.".into();
        } else {
            r.hint = format!("✓ Key saved · artwork from RAWG for {rawg_games} game{}", if rawg_games == 1 { "" } else { "s" }).into();
            r.hint_kind = 1;
        }
        rows.push((SId::Rawg, r));
        if !key.is_empty() {
            rows.push((SId::RawgRemove, row(4, "Remove RAWG key")));
        }

        let mut r = row(4, "Reload RuTracker catalog");
        r.value = if self.syncing {
            "Reloading…".into()
        } else {
            let snapshot = self.games.first().map(|game| util::fmt_date(util::parse_iso_date(&game.g.peers_observed))).unwrap_or_default();
            format!("{} releases · snapshot {}", self.games.len(), if snapshot.is_empty() { "unknown" } else { &snapshot }).into()
        };
        r.hint = "Reloads local JSON, not the website. Peer counts are snapshot data, not live.".into();
        rows.push((SId::Refresh, r));
        rows.push((SId::Rescan, row(4, "Rescan installed games")));
        let mut r = row(4, "Quit launcher");
        r.danger = true;
        rows.push((SId::Quit, r));

        self.settings_ids = rows.iter().map(|(id, _)| *id).collect();
        self.settings_rows = rows.into_iter().map(|(_, r)| r).collect();
    }

    pub fn push_settings(&mut self) {
        let ui = self.ui();
        ui.set_settings(model(self.settings_rows.clone()));
        ui.set_edit_index(self.edit_index);
    }

    pub fn scroll_settings(&mut self) {
        // Estimated row heights matching app.slint.
        let mut y = 64.0 + 54.0 + 24.0;
        let mut target = 0.0;
        for (i, r) in self.settings_rows.iter().enumerate() {
            if i as i32 == self.idx {
                target = y;
            }
            y += match r.kind {
                0 => 36.0 + 10.0 + 22.0,
                1 => 8.0 + 26.0 + 6.0 + 54.0 + 4.0,
                _ => 8.0 + 64.0 + 4.0,
            } + if r.hint.is_empty() { 0.0 } else { 26.0 };
        }
        let (_, h) = self.logical_size();
        let max = (y + 120.0 - h).max(0.0);
        self.ui().set_settings_y(-(target - h * 0.4).clamp(0.0, max) * self.scale);
    }

    fn save_cfg(&mut self, f: impl FnOnce(&mut crate::config::Config)) {
        let mut c = self.cfg.lock().unwrap();
        f(&mut c);
        c.save();
    }

    fn refresh_settings(&mut self) {
        self.build_settings();
        self.push_settings();
    }

    pub fn settings_change(&mut self, i: usize, dir: i32) {
        let Some(id) = self.settings_ids.get(i).copied() else { return };
        match id {
            SId::Resolution => {
                let cur = { let c = self.cfg.lock().unwrap(); (c.width, c.height) };
                let pos = RESOLUTIONS.iter().position(|r| *r == cur).unwrap_or(2) as i32;
                let (w, h) = RESOLUTIONS[(pos + dir).rem_euclid(RESOLUTIONS.len() as i32) as usize];
                self.save_cfg(|c| {
                    c.width = w;
                    c.height = h;
                });
            }
            SId::Present => {
                let cur = self.cfg.lock().unwrap().present_mode.clone();
                let pos = PRESENT_MODES.iter().position(|(k, _)| *k == cur).unwrap_or(0) as i32;
                let next = PRESENT_MODES[(pos + dir).rem_euclid(PRESENT_MODES.len() as i32) as usize].0.to_string();
                self.save_cfg(|c| c.present_mode = next);
            }
            SId::Display => {
                let names: Vec<String> = std::iter::once(String::new()).chain(self.monitors.iter().map(|m| m.name.clone())).collect();
                let cur = self.cfg.lock().unwrap().monitor.clone();
                let pos = names.iter().position(|n| *n == cur).unwrap_or(0) as i32;
                let next = names[(pos + dir).rem_euclid(names.len() as i32) as usize].clone();
                self.save_cfg(|c| c.monitor = next.clone());
                self.move_to_monitor(&next);
            }
            SId::SeedCompleted => {
                let on = dir > 0;
                if let Err(error) = self.downloads.set_seed_after_download(on) {
                    self.toast("Seeding setting unavailable", &error.to_string(), 2);
                    return;
                }
                self.save_cfg(|c| c.seed_after_download = on);
                self.push_downloads();
            }
            SId::Fullscreen | SId::Amd | SId::ReturnOnExit | SId::Sounds | SId::KytyAuto | SId::AppAuto => {
                let on = dir > 0;
                self.save_cfg(|c| match id {
                    SId::Fullscreen => c.fullscreen = on,
                    SId::Amd => c.amd_cpu = on,
                    SId::ReturnOnExit => c.return_on_exit = on,
                    SId::KytyAuto => c.kyty_auto_update = on,
                    SId::AppAuto => c.app_auto_update = on,
                    _ => c.sounds = on,
                });
                if id == SId::KytyAuto && on {
                    self.kyty_check(false);
                }
                if id == SId::Sounds {
                    audio::set_enabled(on);
                }
            }
            _ => return,
        }
        audio::play(Sound::Move);
        self.refresh_settings();
    }

    pub fn settings_activate(&mut self, i: usize) {
        let Some(id) = self.settings_ids.get(i).copied() else { return };
        match id {
            SId::Emulator | SId::Dirs | SId::Extra | SId::Rawg | SId::DownloadDir | SId::InstallDir => {
                let cfg = self.cfg.lock().unwrap().clone();
                let text = match id {
                    SId::Emulator => cfg.emulator,
                    SId::Dirs => cfg.game_dirs.join("; "),
                    SId::Extra => cfg.extra_args,
                    SId::DownloadDir => cfg.download_dir,
                    SId::InstallDir => cfg.install_dir,
                    _ => String::new(),
                };
                audio::play(Sound::Select);
                self.edit_index = i as i32;
                let ui = self.ui();
                ui.set_edit_text(text.into());
                ui.set_edit_index(i as i32);
            }
            SId::Fullscreen | SId::Amd | SId::ReturnOnExit | SId::Sounds | SId::KytyAuto | SId::AppAuto | SId::SeedCompleted => {
                let on = self.settings_rows[i].on;
                self.settings_change(i, if on { -1 } else { 1 });
            }
            SId::Resolution | SId::Present | SId::Display => self.settings_change(i, 1),
            SId::RawgRemove => {
                self.save_cfg(|c| c.rawg_key.clear());
                self.rawg_status.clear();
                self.toast("RAWG key removed", "Artwork from RAWG stays until the next refresh.", 1);
                self.refresh_settings();
                let first = self.settings_ids.iter().position(|s| *s == SId::Rawg).unwrap_or(0);
                self.set_focus(Z_SETTINGS, first as i32);
            }
            SId::KytyUpdate => {
                audio::play(Sound::Select);
                if self.kyty.busy || self.kyty.checking {
                    return;
                }
                let managed = self.kyty_managed();
                if !managed {
                    self.kyty_switch_to_managed();
                } else if self.kyty_update_available() {
                    if let Some(rel) = self.kyty.latest.clone() {
                        self.kyty_install(rel);
                    }
                } else {
                    self.kyty_check(true);
                }
                self.refresh_settings();
            }
            SId::AppUpdate => {
                audio::play(Sound::Select);
                if self.upd.installed.is_some() {
                    self.app_restart();
                } else if self.upd.busy || self.upd.checking {
                    return;
                } else if self.app_update_available() {
                    if let Some(rel) = self.upd.latest.clone() {
                        self.app_update_install(rel);
                    }
                } else {
                    self.app_update_check(true);
                }
                self.refresh_settings();
            }
            SId::KytyRollback => {
                audio::play(Sound::Select);
                self.kyty_rollback();
                self.refresh_settings();
            }
            SId::Refresh => {
                audio::play(Sound::Select);
                self.start_sync();
                self.toast("Refreshing the catalog…", "New games appear as soon as it's done.", 0);
                self.refresh_settings();
            }
            SId::Rescan => {
                audio::play(Sound::Select);
                self.rescan_library();
                let n = self.locals.len();
                self.toast("Installed games rescanned", &format!("{n} game{} found.", if n == 1 { "" } else { "s" }), 1);
                self.refresh_settings();
            }
            SId::Quit => {
                let _ = slint::quit_event_loop();
            }
            SId::Downloads => self.open_downloads(None),
            SId::Header => {}
        }
    }

    pub fn settings_commit_text(&mut self, i: usize, text: String) {
        let Some(id) = self.settings_ids.get(i).copied() else { return };
        let t = text.trim().to_string();
        match id {
            SId::Emulator => {
                self.save_cfg(|c| c.emulator = t);
                self.kyty_refresh_version();
                let ok = self.cfg.lock().unwrap().emulator_ok();
                if ok { self.toast("Emulator path saved", "Games will start with this KytyPS5.", 1) } else { self.toast("Emulator not found", "There's no runnable kyty_emulator at that path.", 2) }
            }
            SId::Dirs => {
                let dirs: Vec<String> = t.split([';', '\n']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                self.save_cfg(|c| c.game_dirs = dirs);
                self.rescan_library();
                let n = self.locals.len();
                self.toast("Installed games rescanned", &format!("{n} game{} found.", if n == 1 { "" } else { "s" }), 1);
            }
            SId::Extra => self.save_cfg(|c| c.extra_args = t),
            SId::DownloadDir => {
                if t.is_empty() || !util::expand_home(&t).is_absolute() {
                    self.toast("Invalid download folder", "Use an absolute path or ~/Downloads/PS5.", 2);
                } else {
                    self.save_cfg(|c| c.download_dir = t);
                }
            }
            SId::InstallDir => {
                if t.is_empty() || !util::expand_home(&t).is_absolute() {
                    self.toast("Invalid installation folder", "Use an absolute path or ~/Games/PS5.", 2);
                } else { self.save_cfg(|c| c.install_dir = t); }
            }
            SId::Rawg => {
                if t.is_empty() {
                    return; // empty keeps the saved key
                }
                self.rawg_status = "Checking key with RAWG…".into();
                self.rawg_status_kind = 0;
                std::thread::spawn(move || {
                    let err = crate::psn::check_rawg_key(&t);
                    let _ = slint::invoke_from_event_loop(move || {
                        with_app(move |app| {
                            if err.is_empty() {
                                app.save_cfg(|c| c.rawg_key = t);
                                app.rawg_status.clear();
                                app.toast("RAWG key saved", "Missing artwork is downloading in the background.", 1);
                                app.start_enrich();
                            } else {
                                app.rawg_status = err.clone();
                                app.rawg_status_kind = 2;
                                app.toast("RAWG key not saved", &format!("{err}."), 2);
                                audio::play(Sound::Error);
                            }
                            if app.overlay == Overlay::Settings {
                                app.build_settings();
                                app.push_settings();
                            }
                        })
                    });
                });
            }
            _ => {}
        }
    }

    /// Switch to another display. The UI scale is fixed per window at startup, so the
    /// launcher restarts itself on the new display (about a second).
    pub fn move_to_monitor(&mut self, name: &str) {
        let label = if name.is_empty() { "the primary display".to_string() } else { name.to_string() };
        if !self.live.is_empty() {
            self.toast(&format!("Moves to {label} next time"), "The launcher can't restart while a game is running.", 0);
            return;
        }
        self.toast(&format!("Moving to {label}…"), "The launcher restarts on that display.", 0);
        slint::Timer::single_shot(std::time::Duration::from_millis(600), || with_app(|app| app.app_restart()));
    }
}
