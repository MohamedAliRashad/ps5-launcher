//! Settings sheet: rows, controller-friendly editing, instant save.

use crate::app::*;
use crate::audio::{self, Sound};
use crate::config::{PRESENT_MODES, RESOLUTIONS};
use crate::util;
use crate::SettingData;
use slint::ComponentHandle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SId {
    Header,
    Emulator,
    Dirs,
    Resolution,
    Present,
    Fullscreen,
    Amd,
    Extra,
    ReturnOnExit,
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
        r.value = cfg.emulator.clone().into();
        let ok = cfg.emulator_ok();
        r.hint = if ok { "Emulator found".into() } else { "Emulator not found at this path".into() };
        r.hint_kind = if ok { 1 } else { 2 };
        rows.push((SId::Emulator, r));

        let mut r = row(1, "Game folders (separate with ;)");
        r.value = cfg.game_dirs.join("; ").into();
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

        let mut r = row(4, "Refresh catalog");
        r.value = if self.syncing {
            "Updating…".into()
        } else {
            format!("{} games · updated {}", self.games.len(), if self.catalog_updated > 0.0 { util::fmt_last_played(self.catalog_updated).to_lowercase() } else { "never".into() }).into()
        };
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
        self.ui().set_settings_y(-(target - h * 0.4).clamp(0.0, max));
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
            SId::Fullscreen | SId::Amd | SId::ReturnOnExit | SId::Sounds => {
                let on = dir > 0;
                self.save_cfg(|c| match id {
                    SId::Fullscreen => c.fullscreen = on,
                    SId::Amd => c.amd_cpu = on,
                    SId::ReturnOnExit => c.return_on_exit = on,
                    _ => c.sounds = on,
                });
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
            SId::Emulator | SId::Dirs | SId::Extra | SId::Rawg => {
                let cfg = self.cfg.lock().unwrap().clone();
                let text = match id {
                    SId::Emulator => cfg.emulator,
                    SId::Dirs => cfg.game_dirs.join("; "),
                    SId::Extra => cfg.extra_args,
                    _ => String::new(),
                };
                audio::play(Sound::Select);
                self.edit_index = i as i32;
                let ui = self.ui();
                ui.set_edit_text(text.into());
                ui.set_edit_index(i as i32);
            }
            SId::Fullscreen | SId::Amd | SId::ReturnOnExit | SId::Sounds => {
                let on = self.settings_rows[i].on;
                self.settings_change(i, if on { -1 } else { 1 });
            }
            SId::Resolution | SId::Present | SId::Display => self.settings_change(i, 1),
            SId::RawgRemove => {
                self.save_cfg(|c| c.rawg_key.clear());
                self.rawg_status.clear();
                self.toast("RAWG key removed", "", 1);
                self.refresh_settings();
                let first = self.settings_ids.iter().position(|s| *s == SId::Rawg).unwrap_or(0);
                self.set_focus(Z_SETTINGS, first as i32);
            }
            SId::Refresh => {
                audio::play(Sound::Select);
                self.start_sync();
                self.toast("Refreshing catalog…", "", 0);
                self.refresh_settings();
            }
            SId::Rescan => {
                audio::play(Sound::Select);
                self.rescan_library();
                let n = self.locals.len();
                self.toast(&format!("{n} installed game{} found", if n == 1 { "" } else { "s" }), "", 1);
                self.refresh_settings();
            }
            SId::Quit => {
                let _ = slint::quit_event_loop();
            }
            SId::Header => {}
        }
    }

    pub fn settings_commit_text(&mut self, i: usize, text: String) {
        let Some(id) = self.settings_ids.get(i).copied() else { return };
        let t = text.trim().to_string();
        match id {
            SId::Emulator => {
                self.save_cfg(|c| c.emulator = t);
                let ok = self.cfg.lock().unwrap().emulator_ok();
                self.toast(if ok { "Emulator path saved" } else { "Emulator not found at that path" }, "", if ok { 1 } else { 2 });
            }
            SId::Dirs => {
                let dirs: Vec<String> = t.split([';', '\n']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                self.save_cfg(|c| c.game_dirs = dirs);
                self.rescan_library();
                let n = self.locals.len();
                self.toast(&format!("{n} installed game{} found", if n == 1 { "" } else { "s" }), "", 1);
            }
            SId::Extra => self.save_cfg(|c| c.extra_args = t),
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
                                app.toast("RAWG key verified and saved", "Fetching missing artwork in the background", 1);
                                app.start_enrich();
                            } else {
                                app.rawg_status = err.clone();
                                app.rawg_status_kind = 2;
                                app.toast("RAWG key not saved", &err, 2);
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

    /// Move the fullscreen window to another display and rescale for it.
    pub fn move_to_monitor(&mut self, name: &str) {
        let target = crate::display::pick(&self.monitors, name);
        let ui = self.ui();
        crate::display::place_window(&ui, target.as_ref(), false);
        let scale = crate::display::scale_for(target.as_ref());
        if (scale - self.scale).abs() > 0.01 {
            self.scale = scale;
            ui.window().dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: scale });
        }
        self.relayout();
        self.push_grid();
    }
}
