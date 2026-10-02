//! shadPS4 (PS4 games) inside the app: installed the first time a PS4 game is played, then
//! kept up to date in the background like KytyPS5, with rollback.

use crate::app::*;
use crate::shad::{self, Release};
use std::time::Duration;

#[derive(Default)]
pub struct ShadUi {
    pub busy: bool,
    pub progress: String,
    pub error: String,
    /// A PS4 game (by local game id) to start once shadPS4 is installed.
    pub launch_after: Option<String>,
    /// An update found while a game was running; installed when it closes.
    pub pending: Option<Release>,
}

fn post(f: impl FnOnce(&mut App) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}

impl App {
    /// Background update checks, only once shadPS4 is installed (PS5-only users never get it).
    pub fn shad_start(&mut self) {
        let check = |app: &mut App| {
            let auto = app.cfg.lock().unwrap().shad_auto_update;
            if auto && shad::installed() && crate::util::now_secs() - shad::load_state().last_check > shad::CHECK_INTERVAL {
                app.shad_check(false);
            }
        };
        check(self);
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, Duration::from_secs(30 * 60), move || with_app(check));
        std::mem::forget(t); // lives for the whole session
    }

    /// Look for a newer build; install it when there is one (unless rolled back from it).
    pub fn shad_check(&mut self, manual: bool) {
        if self.shad.busy {
            return;
        }
        std::thread::spawn(move || {
            let res = shad::latest_release();
            post(move |app| match res {
                Ok(rel) => {
                    shad::mark_checked(&rel.tag);
                    let st = shad::load_state();
                    if rel.tag != st.installed && (manual || rel.tag != st.skip) {
                        app.shad_install(rel, None);
                    } else if manual {
                        app.toast("shadPS4 is up to date", &format!("You have the latest release, {}.", shad::pretty(&rel.tag)), 1);
                    }
                    app.refresh_settings();
                }
                Err(e) => {
                    crate::log!("shadPS4 update check failed: {e}");
                    if manual {
                        app.toast("Couldn't check for shadPS4 updates", &e, 2);
                    }
                }
            });
        });
    }

    /// PS4 games need shadPS4: download it now, then start the game.
    pub fn shad_install_then_launch(&mut self, game_id: String) {
        self.toast("Getting shadPS4 for PS4 games", "It downloads once (about 35 MB), then your game starts.", 0);
        self.shad.launch_after = Some(game_id);
        if self.shad.busy {
            return;
        }
        self.shad.busy = true;
        std::thread::spawn(|| {
            let res = shad::latest_release();
            post(move |app| {
                app.shad.busy = false;
                match res {
                    Ok(rel) => {
                        let after = app.shad.launch_after.clone();
                        app.shad_install(rel, after);
                    }
                    Err(e) => {
                        app.shad.launch_after = None;
                        app.toast("Couldn't get shadPS4", &format!("{e}. Check your connection and try again."), 2);
                    }
                }
            });
        });
    }

    pub fn shad_install(&mut self, rel: Release, launch_after: Option<String>) {
        if self.shad.busy {
            return;
        }
        if !self.live.is_empty() && launch_after.is_none() {
            self.shad.pending = Some(rel);
            return;
        }
        self.shad.busy = true;
        self.shad.error.clear();
        self.shad.progress = "Starting download…".into();
        self.refresh_settings();
        let updating = shad::installed();
        std::thread::spawn(move || {
            let last = std::sync::Mutex::new(String::new());
            let res = shad::install(&rel, &|p, _| {
                let mut l = last.lock().unwrap();
                if *l != p {
                    *l = p.clone();
                    post(move |app| {
                        app.shad.progress = p.clone();
                        app.set_status(&p, true);
                    });
                }
            });
            post(move |app| {
                app.shad.busy = false;
                app.shad.progress.clear();
                app.set_status("", false);
                match res {
                    Ok(()) => {
                        let what = if updating { "shadPS4 updated" } else { "shadPS4 installed" };
                        app.toast(what, &format!("PS4 games run on shadPS4 {}.", shad::pretty(&rel.tag)), 1);
                        if let Some(id) = launch_after.or(app.shad.launch_after.take()) {
                            app.shad.launch_after = None;
                            if let Some(l) = app.locals.iter().position(|g| g.l.id == id) {
                                app.launch(l);
                            }
                        }
                    }
                    Err(e) => {
                        crate::log!("shadPS4 install failed: {e}");
                        app.shad.error = e.clone();
                        app.shad.launch_after = None;
                        app.toast("Couldn't install shadPS4", &e, 2);
                    }
                }
                app.refresh_settings();
            });
        });
    }

    /// Called when all games have closed.
    pub fn shad_games_closed(&mut self) {
        if let Some(rel) = self.shad.pending.take() {
            self.shad_install(rel, None);
        }
    }

    pub fn shad_rollback(&mut self) {
        match shad::rollback() {
            Ok(tag) => self.toast("shadPS4 rolled back", &format!("PS4 games now run on {}.", shad::pretty(&tag)), 1),
            Err(e) => self.toast("Can't roll back shadPS4", &e, 2),
        }
        self.refresh_settings();
    }
}
