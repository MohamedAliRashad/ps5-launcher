//! Application state, navigation and event handling. All of it lives on the UI thread;
//! background threads hand results back through `slint::invoke_from_event_loop`.

use crate::audio::{self, Sound};
use crate::catalog::{self, CatalogFile, Game};
use crate::config::Config;
use crate::display::Monitor;
use crate::gamepad::Pad;
use crate::images;
use crate::library::{self, LocalGame};
use crate::psn::{self, Info};
use crate::sessions::{Session, Sessions};
use crate::util::{self, norm};
use crate::{AppWindow, ToastData};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Focus zones (mirrors `Zone` in ui/theme.slint).
pub const Z_TABS: i32 = 0;
pub const Z_TOP: i32 = 1;
pub const Z_ROW: i32 = 2;
pub const Z_ACTIONS: i32 = 3;
pub const Z_SEARCH: i32 = 4;
pub const Z_SORT: i32 = 5;
pub const Z_CHIPS: i32 = 6;
pub const Z_GRID: i32 = 7;
pub const Z_HUB: i32 = 8;
pub const Z_SHOTS: i32 = 9;
pub const Z_DESC: i32 = 10;
pub const Z_SETTINGS: i32 = 11;
pub const Z_MENU: i32 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    None = 0,
    Hub = 1,
    Settings = 2,
    Viewer = 3,
    Launch = 4,
    Menu = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Trailer,
    Search,
    Options,
    Settings,
    TabPrev,
    TabNext,
    PageUp,
    PageDown,
    First,
    Last,
}

pub const GENRES: [(&str, &str); 16] = [
    ("Action", r"(?i)action|hack|slash|beat.?.?em|brawler|souls"),
    ("Adventure", r"(?i)adventure|narrative|exploration|open.world|story"),
    ("RPG", r"(?i)rpg|role"),
    ("Shooter", r"(?i)shoot|fps|gun"),
    ("Horror", r"(?i)horror"),
    ("Survival", r"(?i)survival"),
    ("Racing", r"(?i)racing|driving|kart|motor"),
    ("Sports", r"(?i)sport|football|soccer|basketball|golf|tennis|wrestling|boxing|skate|baseball|hockey|cricket|ufc"),
    ("Fighting", r"(?i)fight|martial"),
    ("Platformer", r"(?i)platform|metroidvania"),
    ("Puzzle", r"(?i)puzzle"),
    ("Simulation", r"(?i)simulat|management|farming|sandbox|builder|life"),
    ("Strategy", r"(?i)strateg|tactic|tower|4x"),
    ("Stealth", r"(?i)stealth"),
    ("Family", r"(?i)party|family|casual|kids|music|rhythm"),
    ("VR", r"(?i)\bvr\b|virtual reality"),
];

pub const SORTS: [&str; 6] = ["Recently added", "Name (A–Z)", "Release date", "Top rated", "Size (largest)", "Size (smallest)"];

/// A catalog game plus derived data.
pub struct GameV {
    pub g: Game,
    pub info: Option<Info>,
    pub name: String,
    pub buckets: Vec<&'static str>,
    pub norm: String,
    pub rel_day: i64,
    pub is_new: bool,
    pub local: Option<usize>,
}

pub struct LocalV {
    pub l: LocalGame,
    pub info: Option<Info>,
    pub cat: Option<usize>,
    pub name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowItem {
    Local(usize),
    Cat(usize),
    All,
}

/// What the game hub / options menu is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Target {
    pub game: Option<usize>,
    pub local: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ActionDef {
    pub id: &'static str,
    pub label: String,
    pub icon: &'static str,
    pub primary: bool,
    pub danger: bool,
    pub round: bool,
}

pub struct App {
    pub ui: slint::Weak<AppWindow>,
    pub cfg: Arc<Mutex<Config>>,
    pub games: Vec<GameV>,
    pub locals: Vec<LocalV>,
    pub library: Arc<Mutex<Vec<LocalGame>>>,
    pub art: psn::Shared,
    pub sessions: Sessions,
    pub live: Vec<Session>,
    pub images: images::Store,
    pub monitors: Vec<Monitor>,
    pub scale: f32,

    pub view: i32,
    pub overlay: Overlay,
    pub zone: i32,
    pub idx: i32,
    pub stack: Vec<(Overlay, i32, i32)>,

    pub row: Vec<RowItem>,
    pub sel: usize,
    pub hero_actions: Vec<ActionDef>,
    pub bg_key: String,
    pub bg_show_b: bool,

    pub filtered: Vec<usize>,
    pub genre: String,
    pub genre_list: Vec<(String, usize)>,
    pub sort: usize,
    pub query: String,
    pub search_editing: bool,
    pub cols: usize,
    pub card_w: f32,
    pub row_h: f32,
    pub grid_window: (usize, usize),
    pub grid_model: Rc<VecModel<crate::CardRow>>,
    pub tile_model: Rc<VecModel<crate::TileData>>,

    pub hub: Option<Target>,
    pub hub_actions: Vec<ActionDef>,
    pub hub_shots: Vec<String>,
    pub viewer_i: usize,
    pub menu_target: Option<Target>,
    pub menu_actions: Vec<ActionDef>,

    pub settings_rows: Vec<crate::SettingData>,
    pub edit_index: i32,
    pub rawg_status: String,
    pub rawg_status_kind: i32,

    pub toasts: Vec<ToastData>,
    pub toast_seq: i32,
    pub status: String,
    pub status_busy: bool,
    pub catalog_updated: f64,
    pub syncing: bool,
    pub enriching: bool,
    pub pad_hints: bool,
    pub clock: String,
    pub trailer: Option<std::process::Child>,
    pub genre_res: Vec<(&'static str, regex::Regex)>,

    // Keys of images currently on screen, so a finished load updates only what shows it.
    pub tile_keys: Vec<String>,
    pub grid_keys: std::collections::HashSet<String>,
    pub hero_logo_key: String,
    pub hub_keys: std::collections::HashSet<String>,
    pub viewer_key: String,
    pub launch_keys: Vec<String>,
    pub launch_local: Option<usize>,
    pub bg_pending: Option<String>,
    pub settings_ids: Vec<crate::settings::SId>,
    pub boot: crate::boot::Boot,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` with the app. If the app is busy (re-entrant callback), retry on the next loop turn.
pub fn with_app(f: impl FnOnce(&mut App) + 'static) {
    let f = RefCell::new(Some(f));
    let done = APP.with(|a| match a.try_borrow_mut() {
        Ok(mut guard) => {
            if let (Some(app), Some(f)) = (guard.as_mut(), f.borrow_mut().take()) {
                f(app);
            }
            true
        }
        Err(_) => false,
    });
    if !done {
        if let Some(f) = f.into_inner() {
            slint::Timer::single_shot(Duration::ZERO, move || with_app(f));
        }
    }
}

/// Same, for callbacks that must return a value synchronously.
fn with_app_ret<R: Default>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| match a.try_borrow_mut() {
        Ok(mut g) => g.as_mut().map(f).unwrap_or_default(),
        Err(_) => R::default(),
    })
}

fn post(f: impl FnOnce(&mut App) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || with_app(f));
}

// ====================================================================== startup

pub fn run(ui: AppWindow, monitors: Vec<Monitor>, scale: f32, target_monitor: Option<Monitor>, windowed: bool) {
    let cfg = Arc::new(Mutex::new(Config::load()));
    audio::init();
    audio::set_enabled(cfg.lock().unwrap().sounds);

    let library = Arc::new(Mutex::new(library::scan(&cfg.lock().unwrap().game_dir_paths())));
    let art = psn::Store::load();
    let sessions = Sessions::start(library.clone(), cfg.clone(), || post(|app| app.on_sessions()));
    let pool = images::Pool::new(8, Arc::new(|key, buf| post(move |app| app.on_image(key, buf))));

    let genre_res = GENRES.iter().map(|(n, re)| (*n, regex::Regex::new(re).unwrap())).collect();
    let catalog = CatalogFile::load();
    let mut app = App {
        ui: ui.as_weak(),
        cfg: cfg.clone(),
        games: Vec::new(),
        locals: Vec::new(),
        library,
        art,
        sessions,
        live: Vec::new(),
        images: images::Store::new(pool, 200),
        monitors,
        scale,
        view: 0,
        overlay: Overlay::None,
        zone: Z_ROW,
        idx: 0,
        stack: Vec::new(),
        row: Vec::new(),
        sel: 0,
        hero_actions: Vec::new(),
        bg_key: String::new(),
        bg_show_b: false,
        filtered: Vec::new(),
        genre: "All".into(),
        genre_list: Vec::new(),
        sort: 0,
        query: String::new(),
        search_editing: false,
        cols: 7,
        card_w: 214.0,
        row_h: 380.0,
        grid_window: (0, 0),
        grid_model: Rc::new(VecModel::default()),
        tile_model: Rc::new(VecModel::default()),
        hub: None,
        hub_actions: Vec::new(),
        hub_shots: Vec::new(),
        viewer_i: 0,
        menu_target: None,
        menu_actions: Vec::new(),
        settings_rows: Vec::new(),
        edit_index: -1,
        rawg_status: String::new(),
        rawg_status_kind: 0,
        toasts: Vec::new(),
        toast_seq: 0,
        status: String::new(),
        status_busy: false,
        catalog_updated: catalog.updated,
        syncing: false,
        enriching: false,
        pad_hints: false,
        clock: String::new(),
        trailer: None,
        genre_res,
        tile_keys: Vec::new(),
        grid_keys: Default::default(),
        hero_logo_key: String::new(),
        hub_keys: Default::default(),
        viewer_key: String::new(),
        launch_keys: Vec::new(),
        launch_local: None,
        bg_pending: None,
        settings_ids: Vec::new(),
        boot: Default::default(),
    };
    ui.set_grid_rows(ModelRc::from(app.grid_model.clone()));
    ui.set_tiles(ModelRc::from(app.tile_model.clone()));
    app.set_catalog(catalog.games.clone());
    app.relayout();
    app.push_all();
    let stale = catalog.stale();
    let first_run = catalog.games.is_empty() || !Config::path().exists();
    if !Config::path().exists() {
        cfg.lock().unwrap().save(); // remember detected defaults
    }
    APP.with(|a| *a.borrow_mut() = Some(app));
    with_app(move |app| app.boot_start(first_run));

    wire_callbacks(&ui);
    crate::gamepad::spawn(|p| post(move |app| app.on_pad(p)));

    // Clock + session timers: one cheap tick per second (only changed text is pushed).
    let tick = slint::Timer::default();
    tick.start(slint::TimerMode::Repeated, Duration::from_secs(1), || with_app(|app| app.tick()));
    with_app(|app| app.tick());

    with_app(move |app| {
        if stale {
            app.start_sync();
        } else {
            app.start_enrich();
        }
    });

    ui.show().expect("could not open window");
    crate::display::place_window(&ui, target_monitor.as_ref(), windowed);
    slint::run_event_loop().expect("event loop failed");
    drop(tick);
    with_app(|app| {
        if let Some(mut t) = app.trailer.take() {
            let _ = t.kill();
        }
    });
}

fn wire_callbacks(ui: &AppWindow) {
    ui.on_key(|text, ctrl, alt, repeat| with_app_ret(|app| app.on_key(&text, ctrl, alt, repeat)));
    ui.on_click(|zone, idx| with_app(move |app| app.on_click(zone, idx)));
    ui.on_row_wheel(|d| {
        with_app(move |app| {
            if d.abs() >= 1.0 {
                app.select_tile(app.sel as i64 + if d > 0.0 { 1 } else { -1 });
            }
        })
    });
    ui.on_grid_scrolled(|| with_app(|app| app.push_grid_window()));
    ui.on_resized(|| with_app(|app| {
        app.relayout();
        app.push_grid();
    }));
    ui.on_search_edited(|t| with_app(move |app| app.on_search(t.to_string())));
    ui.on_search_done(|_| with_app(|app| {
        app.stop_search_edit();
        if !app.filtered.is_empty() {
            app.set_focus(Z_GRID, 0);
            app.ensure_grid_visible();
        }
        audio::play(Sound::Select);
    }));
    ui.on_edit_done(|t, _| with_app(move |app| app.finish_edit(Some(t.to_string()))));
}

// ====================================================================== data

impl App {
    pub fn ui(&self) -> AppWindow {
        self.ui.upgrade().expect("window gone")
    }

    pub fn art_for_game(&self, g: &Game) -> Option<Info> {
        let store = self.art.lock().unwrap();
        let sony = if g.title_id.is_empty() { None } else { store.get(&g.title_id) };
        if sony.is_some_and(|i| !i.master.is_empty()) {
            return sony.cloned();
        }
        store.get(&psn::rawg_cache_key(&g.name)).or(sony).cloned()
    }

    fn display_name(site: &str, info: Option<&Info>) -> String {
        static SUFFIX: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)\s+(PS4\s*(&|and)\s*)?PS5$").unwrap());
        let Some(n) = info.map(|i| i.name.as_str()).filter(|n| !n.is_empty()) else { return site.to_string() };
        if n.chars().any(|c| matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xac00..=0xd7af)) {
            return site.to_string();
        }
        let cleaned: String = n.chars().filter(|c| !matches!(c, '™' | '®' | '©')).collect();
        let cleaned = SUFFIX.replace(cleaned.trim(), "").split_whitespace().collect::<Vec<_>>().join(" ");
        if cleaned.is_empty() { site.to_string() } else { cleaned }
    }

    /// Japan-region store entries come back in Japanese; drop those text fields so the
    /// English site data is used instead (artwork is kept).
    fn sanitize(info: Option<Info>) -> Option<Info> {
        let cjk = |s: &str| s.chars().any(|c| matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x9fff | 0xac00..=0xd7af));
        info.map(|mut i| {
            if i.genres.iter().any(|g| cjk(g)) {
                i.genres.clear();
            }
            if cjk(&i.short) {
                i.short.clear();
            }
            if cjk(&i.long) {
                i.long.clear();
            }
            if cjk(&i.publisher) {
                i.publisher.clear();
            }
            i
        })
    }

    pub fn set_catalog(&mut self, games: Vec<Game>) {
        let today = (util::now_secs() / 86400.0) as i64;
        let mut out = Vec::with_capacity(games.len());
        for g in games {
            let info = Self::sanitize(self.art_for_game(&g));
            let name = Self::display_name(&g.name, info.as_ref());
            let genre_text = g.genres.iter().chain(info.iter().flat_map(|i| i.genres.iter())).cloned().collect::<Vec<_>>().join(" | ");
            let mut buckets: Vec<&'static str> = self.genre_res.iter().filter(|(_, re)| re.is_match(&genre_text)).map(|(n, _)| *n).collect();
            if buckets.is_empty() {
                buckets.push("Other");
            }
            let rel = util::parse_long_date(&g.release).or_else(|| info.as_ref().and_then(|i| util::parse_iso_date(&i.release)));
            let added = util::parse_iso_date(&g.date).map(|(y, m, d)| util::days_from_civil(y, m, d)).unwrap_or(0);
            let publisher = info.as_ref().map(|i| i.publisher.clone()).unwrap_or_default();
            out.push(GameV {
                norm: norm(&format!("{} {} {} {}", name, g.name, g.title_id, publisher)),
                name,
                buckets,
                rel_day: rel.map(|(y, m, d)| util::days_from_civil(y, m, d)).unwrap_or(0),
                is_new: today - added < 14,
                local: None,
                info,
                g,
            });
        }
        self.games = out;
        self.refresh_locals();
    }

    /// Rebuild installed games and link them to catalog entries.
    pub fn refresh_locals(&mut self) {
        let libs = self.library.lock().unwrap().clone();
        let mut by_tid: HashMap<&str, usize> = HashMap::new();
        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, g) in self.games.iter().enumerate() {
            if !g.g.title_id.is_empty() {
                by_tid.insert(&g.g.title_id, i);
            }
            by_name.insert(norm(&g.name), i);
            by_name.insert(norm(&g.g.name), i);
        }
        let mut locals = Vec::new();
        let store = self.art.lock().unwrap();
        for l in libs {
            let cat = by_tid.get(l.title_id.as_str()).copied().or_else(|| by_name.get(&norm(&l.name)).copied());
            let info = Self::sanitize(store.get(&l.title_id).cloned()).or_else(|| cat.and_then(|c| self.games[c].info.clone()));
            let name = Self::display_name(&l.name, info.as_ref());
            locals.push(LocalV { l, info, cat, name });
        }
        drop(store);
        for g in &mut self.games {
            g.local = None;
        }
        for (i, l) in locals.iter().enumerate() {
            if let Some(c) = l.cat {
                self.games[c].local = Some(i);
            }
        }
        self.locals = locals;
        self.build_row();
        self.build_genres();
        self.apply_filter();
    }

    fn playtime_key(l: &LocalGame) -> String {
        if l.title_id.is_empty() { l.path.to_string_lossy().into_owned() } else { l.title_id.clone() }
    }

    pub fn playtime(&self, l: &LocalGame) -> crate::sessions::PlayStats {
        self.sessions.playtime(&Self::playtime_key(l))
    }

    pub fn build_row(&mut self) {
        let prev = self.row.get(self.sel).copied();
        let mut locals: Vec<usize> = (0..self.locals.len()).collect();
        locals.sort_by(|a, b| {
            let (pa, pb) = (self.playtime(&self.locals[*a].l).last, self.playtime(&self.locals[*b].l).last);
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal).then_with(|| self.locals[*a].name.cmp(&self.locals[*b].name))
        });
        let mut row: Vec<RowItem> = locals.into_iter().map(RowItem::Local).collect();
        row.extend((0..self.games.len()).filter(|i| self.games[*i].local.is_none()).take(30).map(RowItem::Cat));
        if !self.games.is_empty() {
            row.push(RowItem::All);
        }
        self.row = row;
        self.sel = prev.and_then(|p| self.row.iter().position(|r| *r == p)).unwrap_or(self.sel.min(self.row.len().saturating_sub(1)));
    }

    pub fn build_genres(&mut self) {
        let mut counts: HashMap<&'static str, usize> = HashMap::new();
        for g in &self.games {
            for b in &g.buckets {
                *counts.entry(b).or_default() += 1;
            }
        }
        let mut list: Vec<(String, usize)> = vec![("All".into(), self.games.len())];
        let installed = self.games.iter().filter(|g| g.local.is_some()).count();
        if installed > 0 {
            list.push(("Installed".into(), installed));
        }
        let mut rest: Vec<(&str, usize)> = counts.into_iter().collect();
        rest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        list.extend(rest.into_iter().map(|(n, c)| (n.to_string(), c)));
        if !list.iter().any(|(n, _)| *n == self.genre) {
            self.genre = "All".into();
        }
        self.genre_list = list;
    }

    pub fn apply_filter(&mut self) {
        let words: Vec<String> = norm(&self.query).split_whitespace().map(String::from).collect();
        let genre = self.genre.as_str();
        let mut list: Vec<usize> = (0..self.games.len())
            .filter(|&i| {
                let g = &self.games[i];
                (genre == "All" || (genre == "Installed" && g.local.is_some()) || g.buckets.contains(&genre))
                    && words.iter().all(|w| g.norm.contains(w.as_str()))
            })
            .collect();
        let gs = &self.games;
        let score = |i: usize| gs[i].info.as_ref().and_then(|x| x.rating.as_ref()).map(|r| (r.score, r.total)).unwrap_or((-1.0, 0));
        match self.sort {
            1 => list.sort_by(|a, b| gs[*a].name.to_lowercase().cmp(&gs[*b].name.to_lowercase())),
            2 => list.sort_by(|a, b| gs[*b].rel_day.cmp(&gs[*a].rel_day)),
            3 => list.sort_by(|a, b| score(*b).partial_cmp(&score(*a)).unwrap_or(std::cmp::Ordering::Equal)),
            4 => list.sort_by(|a, b| gs[*b].g.size_gb.unwrap_or(-1.0).partial_cmp(&gs[*a].g.size_gb.unwrap_or(-1.0)).unwrap_or(std::cmp::Ordering::Equal)),
            5 => list.sort_by(|a, b| gs[*a].g.size_gb.unwrap_or(1e9).partial_cmp(&gs[*b].g.size_gb.unwrap_or(1e9)).unwrap_or(std::cmp::Ordering::Equal)),
            _ => list.sort_by(|a, b| gs[*b].g.date.cmp(&gs[*a].g.date)),
        }
        self.filtered = list;
    }

    // ------------------------------------------------------------------ background jobs

    pub fn start_sync(&mut self) {
        if self.syncing {
            return;
        }
        self.syncing = true;
        self.set_status("Updating catalog…", true);
        std::thread::spawn(|| {
            let res = catalog::sync(&|p| post(move |app| app.set_status(&p, true)));
            post(move |app| {
                app.syncing = false;
                match res {
                    Ok(file) => {
                        let first = app.games.is_empty();
                        app.catalog_updated = file.updated;
                        app.set_catalog(file.games);
                        app.push_all();
                        if !first {
                            app.toast(&format!("Catalog updated · {} games", app.games.len()), "", 1);
                        }
                        app.set_status("", false);
                        app.start_enrich();
                    }
                    Err(e) => {
                        app.set_status("", false);
                        app.toast("Catalog refresh failed", &e, 2);
                        if app.boot.active && app.games.is_empty() {
                            app.ui().set_boot_status("Offline · the catalog could not be downloaded".into());
                            app.boot.waiting_art = false;
                            app.boot_ready();
                        }
                        app.start_enrich();
                    }
                }
            });
        });
    }

    pub fn start_enrich(&mut self) {
        if self.enriching {
            return;
        }
        let titles: Vec<(String, &'static str)> = self
            .games
            .iter()
            .map(|g| (g.g.title_id.clone(), psn::region_from_version(&g.g.version)))
            .chain(self.locals.iter().map(|l| (l.l.title_id.clone(), psn::region_from_content_id(&l.l.content_id))))
            .collect();
        let key = self.cfg.lock().unwrap().rawg_key.trim().to_string();
        let store = self.art.clone();
        let needs_art: Vec<String> = {
            let s = store.lock().unwrap();
            self.games
                .iter()
                .filter(|g| !(psn::valid_title_id(&g.g.title_id) && s.get(&g.g.title_id).is_some_and(|i| !i.master.is_empty())))
                .map(|g| g.g.name.clone())
                .collect()
        };
        self.enriching = true;
        std::thread::spawn(move || {
            let last = Mutex::new(std::time::Instant::now() - Duration::from_secs(1));
            // RAWG only for games Sony has no data for; Sony first, so compute after.
            let changed = psn::enrich(&store, titles, None, &|p| {
                let mut l = last.lock().unwrap();
                if l.elapsed() > Duration::from_millis(120) {
                    *l = std::time::Instant::now();
                    post(move |app| app.set_status(&p, true));
                }
            });
            let mut changed_rawg = false;
            if !key.is_empty() {
                let names: Vec<String> = {
                    let s = store.lock().unwrap();
                    needs_art.into_iter().filter(|n| s.get(&psn::rawg_cache_key(n)).is_none() || !s.fresh(&psn::rawg_cache_key(n))).collect()
                };
                changed_rawg = psn::enrich(&store, Vec::new(), Some((key, names)), &|p| post(move |app| app.set_status(&p, true)));
            }
            post(move |app| {
                app.enriching = false;
                app.set_status("", false);
                if changed || changed_rawg {
                    let games: Vec<Game> = app.games.iter().map(|g| g.g.clone()).collect();
                    app.set_catalog(games);
                    app.images.clear_failures();
                    if !app.boot.active {
                        app.push_all();
                    }
                }
                app.boot_art_done();
            });
        });
    }

    pub fn rescan_library(&mut self) {
        let dirs = self.cfg.lock().unwrap().game_dir_paths();
        *self.library.lock().unwrap() = library::scan(&dirs);
        self.refresh_locals();
        self.push_all();
    }

    // ------------------------------------------------------------------ events

    pub fn on_image(&mut self, key: String, buf: Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>) {
        self.images.insert(key.clone(), buf);
        self.refresh_images(&key);
    }

    pub fn on_sessions(&mut self) {
        let before: Vec<u32> = self.live.iter().map(|s| s.pid).collect();
        self.live = self.sessions.live();
        let after: Vec<u32> = self.live.iter().map(|s| s.pid).collect();
        for e in self.sessions.take_ended() {
            let played = util::fmt_duration(e.played);
            if e.stopped {
                self.toast(&format!("{} stopped", e.name), &format!("Played {played}"), 1);
            } else if let Some(code) = e.exit_code.filter(|c| *c != 0) {
                let tail: Vec<&str> = e.log_tail.trim().lines().collect();
                let tail = tail[tail.len().saturating_sub(6)..].join("\n");
                self.toast(&format!("{} exited with code {code}", e.name), &format!("{tail}\n\nLog: {}", e.log.display()), 2);
                audio::play(Sound::Error);
            } else {
                self.toast(&format!("{} closed", e.name), &format!("Played {played}"), 1);
            }
        }
        if before != after {
            // Play ⇄ Resume/Stop changes the buttons; keep focus on the first one.
            if self.zone == Z_ACTIONS || self.zone == Z_HUB {
                self.set_focus(self.zone, 0);
            }
            if self.zone == Z_TOP && self.live.is_empty() && self.idx < 2 {
                self.set_focus(Z_TOP, 2);
            }
            self.build_row();
            self.push_row();
            self.push_hero();
            if self.overlay == Overlay::Hub {
                self.push_hub();
            }
            self.tick();
        }
    }

    pub fn tick(&mut self) {
        let tm = util::local_time();
        let (h, m) = (tm.tm_hour, tm.tm_min);
        let clock = format!("{}:{:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, m, if h < 12 { "AM" } else { "PM" });
        let ui = self.ui();
        if clock != self.clock {
            ui.set_clock(clock.clone().into());
            self.clock = clock;
        }
        let running = match self.live.first() {
            Some(s) => crate::RunningData { active: true, name: s.name.clone().into(), time: util::fmt_clock(util::now_secs() - s.since).into() },
            None => crate::RunningData::default(),
        };
        if running.active || ui.get_running().active {
            ui.set_running(running);
            // The kicker / row label show the live session time too.
            if self.view == 0 && self.overlay == Overlay::None && self.current_session().is_some() {
                self.push_row_text();
                self.push_hero_kicker();
            }
        }
    }

    pub fn on_pad(&mut self, p: Pad) {
        if p == Pad::Ps {
            self.sessions.toggle_focus();
            return;
        }
        // Close a trailer player that has taken over the screen.
        if self.trailer_running() && matches!(p, Pad::Back | Pad::Confirm) {
            if let Some(mut t) = self.trailer.take() {
                let _ = t.kill();
            }
            crate::sessions::show_launcher();
            return;
        }
        if !crate::display::window_has_focus(&self.ui()) {
            return;
        }
        if !self.pad_hints {
            self.pad_hints = true;
            self.ui().set_pad_hints(true);
        }
        let act = match p {
            Pad::Up => Act::Up,
            Pad::Down => Act::Down,
            Pad::Left => Act::Left,
            Pad::Right => Act::Right,
            Pad::Confirm => Act::Confirm,
            Pad::Back => Act::Back,
            Pad::Square => Act::Trailer,
            Pad::Triangle => Act::Search,
            Pad::Options => Act::Options,
            Pad::L1 => Act::TabPrev,
            Pad::R1 => Act::TabNext,
            Pad::L2 => Act::PageUp,
            Pad::R2 => Act::PageDown,
            Pad::Ps => return,
        };
        if self.search_editing || self.edit_index >= 0 {
            // Controller input ends text editing.
            if self.search_editing {
                self.stop_search_edit();
            } else {
                self.finish_edit(None);
            }
            if act == Act::Back {
                return;
            }
        }
        self.act(act);
    }

    fn trailer_running(&mut self) -> bool {
        match &mut self.trailer {
            Some(c) => match c.try_wait() {
                Ok(None) => true,
                _ => {
                    self.trailer = None;
                    false
                }
            },
            None => false,
        }
    }

    pub fn on_key(&mut self, text: &str, ctrl: bool, alt: bool, repeat: bool) -> bool {
        if self.pad_hints {
            self.pad_hints = false;
            self.ui().set_pad_hints(false);
        }
        let k = |key: Key| SharedString::from(key) == text;
        let dir = if k(Key::UpArrow) {
            Some(Act::Up)
        } else if k(Key::DownArrow) {
            Some(Act::Down)
        } else if k(Key::LeftArrow) {
            Some(Act::Left)
        } else if k(Key::RightArrow) {
            Some(Act::Right)
        } else {
            None
        };

        // Text editing: the TextInput handles typing; we get what it doesn't use.
        if self.search_editing {
            if k(Key::Escape) {
                self.stop_search_edit();
                return true;
            }
            if let Some(d) = dir {
                self.stop_search_edit();
                self.act(d);
                return true;
            }
            return false;
        }
        if self.edit_index >= 0 {
            if k(Key::Escape) {
                self.finish_edit(None);
                return true;
            }
            if matches!(dir, Some(Act::Up | Act::Down)) {
                let cur = self.ui().get_edit_text().to_string();
                self.finish_edit(Some(cur));
                self.act(dir.unwrap());
                return true;
            }
            return false;
        }
        if ctrl && (text == "q" || text == "w") {
            slint::quit_event_loop().ok();
            return true;
        }
        if ctrl || alt {
            return false;
        }
        if let Some(d) = dir {
            self.act(d);
            return true;
        }
        let act = if text == "\n" || text == "\r" || k(Key::Return) || text == " " {
            if repeat {
                return true;
            }
            Act::Confirm
        } else if k(Key::Escape) || k(Key::Backspace) {
            Act::Back
        } else if k(Key::PageUp) {
            Act::PageUp
        } else if k(Key::PageDown) {
            Act::PageDown
        } else if k(Key::Home) {
            Act::First
        } else if k(Key::End) {
            Act::Last
        } else if k(Key::Menu) || text == "o" || text == "O" {
            Act::Options
        } else if text == "t" || text == "T" {
            Act::Trailer
        } else if text == "/" {
            Act::Search
        } else if (text == "s" || text == "S") && self.overlay == Overlay::None && self.view == 0 {
            Act::Settings
        } else if k(Key::Tab) {
            Act::TabNext
        } else if self.overlay == Overlay::None && self.view == 1 && text.chars().count() == 1 && text.chars().all(|c| c.is_alphanumeric()) {
            // Type-to-search in the library.
            self.start_search_edit();
            let q = format!("{}{}", self.query, text);
            self.ui().set_query(q.clone().into());
            self.on_search(q);
            return true;
        } else if (text == "f" || text == "s") && self.overlay == Overlay::None {
            Act::Search
        } else {
            return false;
        };
        self.act(act);
        true
    }

    pub fn on_click(&mut self, zone: i32, idx: i32) {
        if self.boot.active {
            return self.boot_confirm();
        }
        if self.search_editing && zone != Z_SEARCH {
            self.stop_search_edit();
        }
        if self.edit_index >= 0 && !(zone == Z_SETTINGS && idx == self.edit_index) {
            let cur = self.ui().get_edit_text().to_string();
            self.finish_edit(Some(cur));
        }
        // Clicks on an overlay backdrop close it.
        if idx < 0 {
            self.back();
            return;
        }
        if zone == Z_ROW && self.overlay == Overlay::None {
            if self.zone == Z_ROW && self.sel == idx as usize {
                self.activate_row();
            } else {
                self.set_focus(Z_ROW, 0);
                self.select_tile(idx as i64);
            }
            return;
        }
        if zone == Z_SEARCH {
            self.set_focus(Z_SEARCH, 0);
            self.start_search_edit();
            return;
        }
        self.set_focus(zone, idx);
        if zone == Z_GRID {
            self.ensure_grid_visible();
        }
        self.act(Act::Confirm);
    }

    pub fn on_search(&mut self, q: String) {
        if q == self.query {
            return;
        }
        self.query = q;
        self.apply_filter();
        self.ui().set_grid_y(0.0);
        self.push_grid();
    }

    pub fn start_search_edit(&mut self) {
        if self.view != 1 {
            self.switch_view(1, false);
        }
        self.search_editing = true;
        self.set_focus(Z_SEARCH, 0);
        self.ui().invoke_focus_search();
    }

    pub fn stop_search_edit(&mut self) {
        self.search_editing = false;
        self.ui().invoke_focus_root();
    }

    // ------------------------------------------------------------------ navigation

    pub fn set_focus(&mut self, zone: i32, idx: i32) {
        self.zone = zone;
        self.idx = idx;
        let ui = self.ui();
        ui.set_zone(zone);
        ui.set_idx(idx);
    }

    fn move_focus(&mut self, zone: i32, idx: i32) {
        self.set_focus(zone, idx);
        audio::play(Sound::Move);
    }

    fn top_items(&self) -> Vec<i32> {
        if self.live.is_empty() { vec![2, 3] } else { vec![0, 1, 2, 3] }
    }

    pub fn act(&mut self, a: Act) {
        if self.boot.active {
            if a == Act::Confirm && (self.boot.first || self.boot.ready) {
                self.boot_confirm();
            }
            return;
        }
        match self.overlay {
            Overlay::Launch => {}
            Overlay::Viewer => self.act_viewer(a),
            Overlay::Menu => self.act_menu(a),
            Overlay::Settings => self.act_settings(a),
            Overlay::Hub => self.act_hub(a),
            Overlay::None => self.act_main(a),
        }
    }

    fn act_main(&mut self, a: Act) {
        match a {
            Act::Back => return self.back(),
            Act::Search => return self.start_search_edit(),
            Act::Settings => return self.open_settings(),
            Act::TabPrev | Act::TabNext => return self.switch_view(1 - self.view, true),
            Act::Trailer => {
                if let Some(t) = self.focused_target() {
                    self.play_trailer(t);
                }
                return;
            }
            Act::Options => {
                match self.focused_target() {
                    Some(t) => self.open_menu(t),
                    None => self.open_settings(),
                }
                return;
            }
            _ => {}
        }
        match self.zone {
            Z_TABS => match a {
                Act::Left if self.idx > 0 => self.move_focus(Z_TABS, self.idx - 1),
                Act::Right if self.idx < 1 => self.move_focus(Z_TABS, self.idx + 1),
                Act::Right => self.move_focus(Z_TOP, self.top_items()[0]),
                Act::Down => {
                    if self.view == 0 { self.move_focus(Z_ROW, 0) } else { self.move_focus(Z_SEARCH, 0) }
                }
                Act::Confirm => self.switch_view(self.idx, true),
                _ => {}
            },
            Z_TOP => {
                let items = self.top_items();
                let pos = items.iter().position(|i| *i == self.idx).unwrap_or(0);
                match a {
                    Act::Left if pos > 0 => self.move_focus(Z_TOP, items[pos - 1]),
                    Act::Left => self.move_focus(Z_TABS, 1),
                    Act::Right if pos + 1 < items.len() => self.move_focus(Z_TOP, items[pos + 1]),
                    Act::Down => {
                        if self.view == 0 { self.move_focus(Z_ROW, 0) } else { self.move_focus(Z_SEARCH, 0) }
                    }
                    Act::Confirm => match self.idx {
                        0 => self.resume_game(),
                        1 => self.stop_game(None),
                        2 => self.start_search_edit(),
                        _ => self.open_settings(),
                    },
                    _ => {}
                }
            }
            Z_ROW => match a {
                Act::Left => self.select_tile(self.sel as i64 - 1),
                Act::Right => self.select_tile(self.sel as i64 + 1),
                Act::First | Act::PageUp => self.select_tile(0),
                Act::Last | Act::PageDown => self.select_tile(self.row.len() as i64 - 1),
                Act::Up => self.move_focus(Z_TABS, 0),
                Act::Down if !self.hero_actions.is_empty() => self.move_focus(Z_ACTIONS, 0),
                Act::Confirm => self.activate_row(),
                _ => {}
            },
            Z_ACTIONS => match a {
                Act::Left if self.idx > 0 => self.move_focus(Z_ACTIONS, self.idx - 1),
                Act::Right if (self.idx as usize) + 1 < self.hero_actions.len() => self.move_focus(Z_ACTIONS, self.idx + 1),
                Act::Up => self.move_focus(Z_ROW, 0),
                Act::Confirm => {
                    if let Some(act) = self.hero_actions.get(self.idx as usize).map(|a| a.id) {
                        let t = self.row_target(self.sel);
                        self.run_action(act, t);
                    }
                }
                _ => {}
            },
            Z_SEARCH => match a {
                Act::Right => self.move_focus(Z_SORT, 0),
                Act::Up => self.move_focus(Z_TABS, 1),
                Act::Down => self.focus_chip(),
                Act::Confirm => self.start_search_edit(),
                _ => {}
            },
            Z_SORT => match a {
                Act::Left => self.move_focus(Z_SEARCH, 0),
                Act::Up => self.move_focus(Z_TABS, 1),
                Act::Down => self.focus_chip(),
                Act::Confirm | Act::Right => self.cycle_sort(1),
                _ => {}
            },
            Z_CHIPS => match a {
                Act::Left if self.idx > 0 => {
                    self.move_focus(Z_CHIPS, self.idx - 1);
                    self.push_genres();
                }
                Act::Right if (self.idx as usize) + 1 < self.genre_list.len() => {
                    self.move_focus(Z_CHIPS, self.idx + 1);
                    self.push_genres();
                }
                Act::Up => self.move_focus(Z_SEARCH, 0),
                Act::Down if !self.filtered.is_empty() => {
                    let first = self.first_visible_card();
                    self.move_focus(Z_GRID, first as i32);
                    self.focus_card_changed();
                }
                Act::Confirm => {
                    if let Some((g, _)) = self.genre_list.get(self.idx as usize).cloned() {
                        self.genre = g;
                        self.apply_filter();
                        self.ui().set_grid_y(0.0);
                        self.push_library();
                        audio::play(Sound::Select);
                    }
                }
                _ => {}
            },
            Z_GRID => self.act_grid(a),
            _ => {}
        }
    }

    fn focus_chip(&mut self) {
        let i = self.genre_list.iter().position(|(g, _)| *g == self.genre).unwrap_or(0);
        self.move_focus(Z_CHIPS, i as i32);
        self.push_genres();
    }

    fn act_grid(&mut self, a: Act) {
        let n = self.filtered.len() as i64;
        if n == 0 {
            if a == Act::Up {
                self.focus_chip();
            }
            return;
        }
        let cols = self.cols as i64;
        let i = self.idx as i64;
        let page = (((self.ui().window().size().height as f32 / self.scale) - 262.0) / self.row_h).floor().max(1.0) as i64 * cols;
        let j = match a {
            Act::Left => i - 1,
            Act::Right => i + 1,
            Act::Up if i < cols => return self.focus_chip(),
            Act::Up => i - cols,
            Act::Down if i + cols >= n => {
                if i / cols < (n - 1) / cols { n - 1 } else { return }
            }
            Act::Down => i + cols,
            Act::PageUp => (i - page).max(i % cols),
            Act::PageDown => (i + page).min(n - 1),
            Act::First => 0,
            Act::Last => n - 1,
            Act::Confirm => {
                if let Some(&gi) = self.filtered.get(i as usize) {
                    audio::play(Sound::Select);
                    let local = self.games[gi].local;
                    self.open_hub(Target { game: Some(gi), local });
                }
                return;
            }
            _ => return,
        };
        if j < 0 || j >= n || j == i {
            return;
        }
        self.move_focus(Z_GRID, j as i32);
        self.ensure_grid_visible();
        self.focus_card_changed();
    }

    fn act_hub(&mut self, a: Act) {
        match a {
            Act::Back => return self.back(),
            Act::Trailer => {
                if let Some(t) = self.hub {
                    self.play_trailer(t);
                }
                return;
            }
            Act::Options => {
                if let Some(t) = self.hub {
                    self.open_menu(t);
                }
                return;
            }
            _ => {}
        }
        let shots = self.hub_shots.len() as i32;
        match (self.zone, a) {
            (Z_HUB, Act::Left) if self.idx > 0 => self.move_focus(Z_HUB, self.idx - 1),
            (Z_HUB, Act::Right) if (self.idx as usize) + 1 < self.hub_actions.len() => self.move_focus(Z_HUB, self.idx + 1),
            (Z_HUB, Act::Down) => {
                if shots > 0 { self.move_focus(Z_SHOTS, 0) } else { self.move_focus(Z_DESC, 0) }
                self.scroll_hub();
            }
            (Z_HUB, Act::Confirm) => {
                if let (Some(act), Some(t)) = (self.hub_actions.get(self.idx as usize).map(|a| a.id), self.hub) {
                    self.run_action(act, t);
                }
            }
            (Z_SHOTS, Act::Left) if self.idx > 0 => {
                self.move_focus(Z_SHOTS, self.idx - 1);
                self.scroll_shots();
            }
            (Z_SHOTS, Act::Right) if self.idx + 1 < shots => {
                self.move_focus(Z_SHOTS, self.idx + 1);
                self.scroll_shots();
            }
            (Z_SHOTS, Act::Up) => {
                self.move_focus(Z_HUB, 0);
                self.scroll_hub();
            }
            (Z_SHOTS, Act::Down) => {
                self.move_focus(Z_DESC, 0);
                self.scroll_hub();
            }
            (Z_SHOTS, Act::Confirm) => self.open_viewer(self.idx as usize),
            (Z_DESC, Act::Up) => {
                if self.idx > 0 {
                    self.idx -= 1;
                    self.scroll_hub();
                } else if shots > 0 {
                    self.move_focus(Z_SHOTS, 0);
                    self.scroll_hub();
                } else {
                    self.move_focus(Z_HUB, 0);
                    self.scroll_hub();
                }
            }
            (Z_DESC, Act::Down | Act::PageDown) => {
                self.idx += 1;
                self.scroll_hub();
            }
            _ => {}
        }
    }

    fn act_viewer(&mut self, a: Act) {
        let n = self.hub_shots.len().max(1);
        match a {
            Act::Left => {
                self.viewer_i = (self.viewer_i + n - 1) % n;
                self.push_viewer();
                audio::play(Sound::Move);
            }
            Act::Right => {
                self.viewer_i = (self.viewer_i + 1) % n;
                self.push_viewer();
                audio::play(Sound::Move);
            }
            Act::Back | Act::Confirm => self.back(),
            _ => {}
        }
    }

    fn act_menu(&mut self, a: Act) {
        let n = self.menu_actions.len() as i32;
        match a {
            Act::Up if self.idx > 0 => self.move_focus(Z_MENU, self.idx - 1),
            Act::Down if self.idx + 1 < n => self.move_focus(Z_MENU, self.idx + 1),
            Act::Back | Act::Options => self.back(),
            Act::Confirm => {
                let act = self.menu_actions.get(self.idx as usize).map(|a| a.id);
                let t = self.menu_target;
                self.back();
                if let (Some(act), Some(t)) = (act, t) {
                    self.run_action(act, t);
                }
            }
            _ => {}
        }
    }

    pub fn back(&mut self) {
        if self.overlay != Overlay::None {
            audio::play(Sound::Back);
            if self.overlay == Overlay::Settings && self.edit_index >= 0 {
                self.finish_edit(None);
            }
            let (ov, zone, idx) = self.stack.pop().unwrap_or((Overlay::None, if self.view == 0 { Z_ROW } else { Z_GRID }, 0));
            self.overlay = ov;
            self.ui().set_overlay(ov as i32);
            self.set_focus(zone, idx);
            if ov == Overlay::None {
                self.ui().set_hub_open(false);
                self.hub = None;
                self.push_hero();
                self.push_row();
                if self.view == 0 {
                    self.want_background(self.hero_bg_url(self.sel), true);
                } else {
                    self.focus_card_changed();
                }
            } else if ov == Overlay::Hub {
                self.push_hub();
            }
            return;
        }
        if self.view == 1 {
            if !self.query.is_empty() && self.zone == Z_SEARCH {
                self.ui().set_query("".into());
                self.on_search(String::new());
                return;
            }
            self.switch_view(0, true);
            audio::play(Sound::Back);
            return;
        }
        if self.zone != Z_ROW {
            self.move_focus(Z_ROW, 0);
        } else if self.sel != 0 {
            self.select_tile(0);
        }
    }

    pub fn push_overlay(&mut self, ov: Overlay, zone: i32, idx: i32) {
        self.stack.push((self.overlay, self.zone, self.idx));
        self.overlay = ov;
        let ui = self.ui();
        ui.set_overlay(ov as i32);
        ui.set_hub_open(self.hub.is_some() && (ov == Overlay::Hub || self.stack.iter().any(|s| s.0 == Overlay::Hub)));
        self.set_focus(zone, idx);
    }

    pub fn switch_view(&mut self, v: i32, focus: bool) {
        if self.view != v {
            audio::play(Sound::Select);
        }
        self.view = v;
        let ui = self.ui();
        ui.set_view(v);
        ui.set_bg_dim(v == 1);
        if v == 1 {
            self.push_library();
            if focus {
                if self.filtered.is_empty() {
                    self.set_focus(Z_SEARCH, 0);
                } else {
                    let first = self.first_visible_card();
                    self.set_focus(Z_GRID, first as i32);
                    self.focus_card_changed();
                }
            }
        } else {
            self.want_background(self.hero_bg_url(self.sel), true);
            if focus {
                self.set_focus(Z_ROW, 0);
            }
        }
    }

    pub fn select_tile(&mut self, i: i64) {
        if self.row.is_empty() {
            return;
        }
        let i = i.clamp(0, self.row.len() as i64 - 1) as usize;
        if i == self.sel {
            return;
        }
        self.sel = i;
        audio::play(Sound::Move);
        self.ui().set_sel(i as i32);
        self.push_row_text();
        self.push_hero();
        self.want_background(self.hero_bg_url(i), false);
        self.prefetch_neighbors();
    }

    pub fn row_target(&self, i: usize) -> Target {
        match self.row.get(i) {
            Some(RowItem::Local(l)) => Target { game: self.locals[*l].cat, local: Some(*l) },
            Some(RowItem::Cat(g)) => Target { game: Some(*g), local: self.games[*g].local },
            _ => Target { game: None, local: None },
        }
    }

    fn focused_target(&self) -> Option<Target> {
        let t = match (self.view, self.zone) {
            (0, Z_ROW | Z_ACTIONS) => self.row_target(self.sel),
            (1, Z_GRID) => {
                let g = *self.filtered.get(self.idx as usize)?;
                Target { game: Some(g), local: self.games[g].local }
            }
            _ => return None,
        };
        (t.game.is_some() || t.local.is_some()).then_some(t)
    }

    fn activate_row(&mut self) {
        match self.row.get(self.sel).copied() {
            Some(RowItem::All) => self.switch_view(1, true),
            Some(RowItem::Local(l)) => {
                if self.session_for_local(l).is_some() {
                    audio::play(Sound::Select);
                    self.resume_game();
                } else {
                    self.launch(l);
                }
            }
            Some(RowItem::Cat(g)) => {
                audio::play(Sound::Select);
                self.open_hub(Target { game: Some(g), local: self.games[g].local });
            }
            None => {}
        }
    }

    pub fn session_for_local(&self, l: usize) -> Option<&Session> {
        let lv = &self.locals[l];
        self.live.iter().find(|s| s.game_id == lv.l.id || (!lv.l.title_id.is_empty() && s.title_id == lv.l.title_id))
    }

    pub fn current_session(&self) -> Option<&Session> {
        match self.row.get(self.sel) {
            Some(RowItem::Local(l)) => self.session_for_local(*l),
            Some(RowItem::Cat(g)) => self.games[*g].local.and_then(|l| self.session_for_local(l)),
            _ => None,
        }
    }

    // ------------------------------------------------------------------ actions

    pub fn actions_for(&self, t: Target, in_hub: bool) -> Vec<ActionDef> {
        let mut a = Vec::new();
        let mk = |id, label: &str, icon, primary| ActionDef { id, label: label.into(), icon, primary, danger: false, round: false };
        let info = self.target_info(t);
        if let Some(l) = t.local {
            if self.session_for_local(l).is_some() {
                a.push(mk("resume", "Resume", "play", true));
                a.push(ActionDef { danger: true, ..mk("stop", "Stop", "stop", false) });
            } else {
                a.push(mk("play", "Play", "play", true));
            }
        }
        if !in_hub && (t.game.is_some() || t.local.is_some()) {
            a.push(mk("hub", if t.local.is_some() { "Game Hub" } else { "View Game" }, "info", t.local.is_none()));
        }
        if info.as_ref().is_some_and(|i| !i.video.is_empty()) || t.game.is_some_and(|g| !self.games[g].g.trailer.is_empty()) {
            a.push(mk("trailer", "Trailer", "film", false));
        }
        if t.game.is_some() {
            a.push(ActionDef { round: true, ..mk("web", "Open web page", "web", false) });
        }
        if in_hub && info.as_ref().is_some_and(|i| !i.store.is_empty()) {
            a.push(ActionDef { round: true, ..mk("store", "PlayStation Store", "bag", false) });
        }
        if !a.iter().any(|x| x.primary) {
            if let Some(f) = a.first_mut() {
                f.primary = true;
            }
        }
        a
    }

    pub fn target_info(&self, t: Target) -> Option<Info> {
        t.local.and_then(|l| self.locals[l].info.clone()).or_else(|| t.game.and_then(|g| self.games[g].info.clone()))
    }

    pub fn run_action(&mut self, id: &str, t: Target) {
        match id {
            "play" => {
                if let Some(l) = t.local {
                    self.launch(l);
                }
            }
            "resume" => {
                audio::play(Sound::Select);
                self.resume_game();
            }
            "stop" => self.stop_game(t.local),
            "hub" => {
                audio::play(Sound::Select);
                self.open_hub(t);
            }
            "trailer" => self.play_trailer(t),
            "web" => {
                if let Some(g) = t.game {
                    let link = self.games[g].g.link.clone();
                    open_url(&link);
                    self.toast("Opened in your browser", "", 1);
                }
            }
            "store" => {
                if let Some(i) = self.target_info(t).filter(|i| !i.store.is_empty()) {
                    open_url(&i.store);
                    self.toast("Opened the PlayStation Store page", "", 1);
                }
            }
            "folder" => {
                if let Some(l) = t.local {
                    open_url(&self.locals[l].l.path.to_string_lossy());
                }
            }
            "log" => {
                if let Some(l) = t.local {
                    let lg = &self.locals[l].l;
                    let p = util::cache_dir().join("logs").join(format!("{}.log", if lg.title_id.is_empty() { &lg.id } else { &lg.title_id }));
                    if p.is_file() {
                        open_url(&p.to_string_lossy());
                    } else {
                        self.toast("No log yet", "Play the game once to create a log.", 0);
                    }
                }
            }
            "settings" => self.open_settings(),
            _ => {}
        }
    }

    pub fn launch(&mut self, l: usize) {
        if let Some(s) = self.live.first() {
            let name = s.name.clone();
            self.toast(&format!("{name} is already running"), "Stop it first, or choose Resume.", 2);
            audio::play(Sound::Error);
            return;
        }
        let game = self.locals[l].l.clone();
        match self.sessions.launch(&game) {
            Ok(()) => {
                audio::play(Sound::Start);
                self.live = self.sessions.live();
                self.show_launch_splash(l);
                self.build_row();
                self.push_row();
                self.push_hero();
                if self.overlay == Overlay::Hub {
                    self.push_hub();
                }
            }
            Err(e) => {
                audio::play(Sound::Error);
                self.toast("Could not start the game", &e, 2);
            }
        }
    }

    pub fn stop_game(&mut self, local: Option<usize>) {
        let s = match local {
            Some(l) => self.session_for_local(l).cloned(),
            None => self.live.first().cloned(),
        };
        if let Some(s) = s {
            audio::play(Sound::Back);
            self.toast(&format!("Stopping {}…", s.name), "", 0);
            self.sessions.stop(s.pid);
        }
    }

    pub fn resume_game(&mut self) {
        if !self.sessions.resume() {
            self.toast("Could not switch to the game", "Its window was not found (is xdotool installed?)", 2);
        }
    }

    pub fn play_trailer(&mut self, t: Target) {
        let info = self.target_info(t);
        let yt = t.game.map(|g| self.games[g].g.trailer.clone()).unwrap_or_default();
        let url = match info.as_ref().filter(|i| !i.video.is_empty()) {
            Some(i) => i.video.clone(),
            None if !yt.is_empty() => format!("https://www.youtube.com/watch?v={yt}"),
            None => return,
        };
        if let Some(mut old) = self.trailer.take() {
            let _ = old.kill();
        }
        audio::play(Sound::Select);
        let monitor = self.cfg.lock().unwrap().monitor.clone();
        match crate::display::play_video(&url, &monitor) {
            Some(child) => {
                self.trailer = Some(child);
                self.toast("Playing trailer", if self.pad_hints { "Press ○ to close" } else { "Press Q or Esc in the player to close" }, 0);
            }
            None => {
                open_url(&url);
                self.toast("Opened the trailer in your browser", "Install mpv for in-launcher fullscreen trailers.", 0);
            }
        }
    }

    pub fn open_hub(&mut self, t: Target) {
        self.hub = Some(t);
        self.hub_actions = self.actions_for(t, true);
        self.hub_shots = self.target_info(t).map(|i| i.shots.clone()).unwrap_or_default();
        self.ui().set_hub_y(0.0);
        self.ui().set_shots_x(0.0);
        if self.overlay == Overlay::Hub {
            self.set_focus(Z_HUB, 0);
        } else {
            self.push_overlay(Overlay::Hub, Z_HUB, 0);
        }
        // The hub uses the game's hub art as its background.
        let bg = self.target_bg_url(t);
        self.want_background(bg, true);
        self.push_hub();
    }

    fn open_viewer(&mut self, i: usize) {
        if self.hub_shots.is_empty() {
            return;
        }
        self.viewer_i = i;
        audio::play(Sound::Select);
        self.push_overlay(Overlay::Viewer, Z_SHOTS, i as i32);
        self.push_viewer();
    }

    pub fn open_menu(&mut self, t: Target) {
        let mut items: Vec<ActionDef> = Vec::new();
        let mk = |id: &'static str, label: &str, icon: &'static str| ActionDef { id, label: label.into(), icon, primary: false, danger: false, round: false };
        for a in self.actions_for(t, true) {
            let label = match a.id {
                "web" => "Open web page".to_string(),
                "store" => "PlayStation Store page".to_string(),
                "trailer" => "Watch trailer".to_string(),
                _ => a.label.clone(),
            };
            items.push(ActionDef { label, ..a });
        }
        if self.overlay != Overlay::Hub {
            items.insert(items.iter().position(|a| a.id == "trailer").unwrap_or(items.len()).min(if t.local.is_some() { 1 } else { 0 }), mk("hub", "Game Hub", "info"));
        }
        if t.local.is_some() {
            items.push(mk("folder", "Open game folder", "folder"));
            items.push(mk("log", "View emulator log", "log"));
        }
        items.push(mk("settings", "Settings", "gear"));
        let title = t.local.map(|l| self.locals[l].name.clone()).or_else(|| t.game.map(|g| self.games[g].name.clone())).unwrap_or_default();
        self.menu_target = Some(t);
        self.menu_actions = items;
        audio::play(Sound::Select);
        self.push_overlay(Overlay::Menu, Z_MENU, 0);
        self.push_menu(&title);
    }

    fn show_launch_splash(&mut self, l: usize) {
        self.push_launch(l);
        self.push_overlay(Overlay::Launch, self.zone, self.idx);
        slint::Timer::single_shot(Duration::from_millis(2600), || {
            with_app(|app| {
                if app.overlay == Overlay::Launch {
                    let (ov, z, i) = app.stack.pop().unwrap_or((Overlay::None, Z_ROW, 0));
                    app.overlay = ov;
                    app.ui().set_overlay(ov as i32);
                    app.set_focus(z, i);
                }
            })
        });
    }

    // ------------------------------------------------------------------ settings

    pub fn open_settings(&mut self) {
        audio::play(Sound::Select);
        self.build_settings();
        let first = self.settings_rows.iter().position(|r| r.kind != 0).unwrap_or(0) as i32;
        self.ui().set_settings_y(0.0);
        self.push_overlay(Overlay::Settings, Z_SETTINGS, first);
        self.push_settings();
    }

    fn act_settings(&mut self, a: Act) {
        let n = self.settings_rows.len() as i32;
        let focusable = |rows: &Vec<crate::SettingData>, i: i32| i >= 0 && i < rows.len() as i32 && rows[i as usize].kind != 0;
        match a {
            Act::Back => self.back(),
            Act::Up | Act::Down => {
                let step = if a == Act::Up { -1 } else { 1 };
                let mut j = self.idx + step;
                while j >= 0 && j < n && !focusable(&self.settings_rows, j) {
                    j += step;
                }
                if focusable(&self.settings_rows, j) {
                    self.move_focus(Z_SETTINGS, j);
                    self.scroll_settings();
                }
            }
            Act::Left | Act::Right => self.settings_change(self.idx as usize, if a == Act::Left { -1 } else { 1 }),
            Act::Confirm => self.settings_activate(self.idx as usize),
            _ => {}
        }
    }

    pub fn finish_edit(&mut self, text: Option<String>) {
        let i = self.edit_index;
        if i < 0 {
            return;
        }
        self.edit_index = -1;
        let ui = self.ui();
        ui.set_edit_index(-1);
        ui.invoke_focus_root();
        if let Some(t) = text {
            self.settings_commit_text(i as usize, t);
        }
        self.build_settings();
        self.push_settings();
    }

    // ------------------------------------------------------------------ toasts / status

    pub fn toast(&mut self, text: &str, sub: &str, kind: i32) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toasts.push(ToastData { id, text: text.into(), sub: sub.into(), kind });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
        self.push_toasts();
        let ms = if kind == 2 { 9000 } else { 3500 };
        slint::Timer::single_shot(Duration::from_millis(ms), move || {
            with_app(move |app| {
                app.toasts.retain(|t| t.id != id);
                app.push_toasts();
            })
        });
    }

    pub fn set_status(&mut self, text: &str, busy: bool) {
        self.boot_status(text);
        if self.status != text || self.status_busy != busy {
            self.status = text.to_string();
            self.status_busy = busy;
            let ui = self.ui();
            ui.set_status(text.into());
            ui.set_status_busy(busy);
        }
    }
}

pub fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub fn target_name(app: &App, t: Target) -> String {
    t.local.map(|l| app.locals[l].name.clone()).or_else(|| t.game.map(|g| app.games[g].name.clone())).unwrap_or_default()
}

pub fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

