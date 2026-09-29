//! Turning app state into Slint models, and image requests for what is on screen.

use crate::app::*;
use crate::images::{prio, Src};
use crate::psn::{sized, Info};
use crate::util;
use crate::{ActionData, CardData, CardRow, ChipData, Fact, FactRow, GenreChip, HeroData, HubData, MenuItem, Shot, TileData};
use slint::{ComponentHandle, Image, Model, SharedString};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct ImgReq {
    pub key: String,
    pub src: Src,
    pub w: u32,
    pub crop: f32,
}

pub fn req_url(url: &str, w: u32, crop: f32) -> Option<ImgReq> {
    (!url.is_empty()).then(|| ImgReq { key: format!("{url}#{w}#{crop}"), src: Src::Url(sized(url, w)), w, crop })
}

pub fn req_file(p: &std::path::Path, w: u32) -> ImgReq {
    ImgReq { key: format!("file:{}#{w}", p.display()), src: Src::File(p.to_path_buf()), w, crop: 0.0 }
}

const COVER_CROP: f32 = 0.12; // superpsx covers carry a PS5 banner on the top ~12%

fn yt_art(id: &str) -> Option<ImgReq> {
    (!id.is_empty()).then(|| req_url(&format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg"), 1920, 0.0)).flatten()
}

fn chip(icon: &str, text: impl Into<SharedString>, gold: bool) -> ChipData {
    ChipData { icon: icon.into(), text: text.into(), gold, dot: 0, stars: 0.0 }
}

/// "● In-game on KytyPS5" / "● Untested on KytyPS5".
fn compat_chip(e: Option<&crate::compat::Entry>) -> ChipData {
    match e {
        Some(e) => ChipData { icon: "".into(), text: format!("{} on KytyPS5", e.status.label()).into(), gold: false, dot: e.status.level(), stars: 0.0 },
        None => ChipData { icon: "".into(), text: "Untested on KytyPS5".into(), gold: false, dot: 5, stars: 0.0 },
    }
}

fn action_data(a: &ActionDef) -> ActionData {
    ActionData { id: a.id.into(), label: a.label.clone().into(), icon: a.icon.into(), primary: a.primary, danger: a.danger, round: a.round }
}

fn logo_size(img: &Image, max_w: f32, max_h: f32) -> (f32, f32) {
    let s = img.size();
    if s.width == 0 || s.height == 0 {
        return (max_w, max_h);
    }
    let k = (max_w / s.width as f32).min(max_h / s.height as f32);
    (s.width as f32 * k, s.height as f32 * k)
}

impl App {
    fn get_img(&mut self, r: &Option<ImgReq>, p: u8) -> Option<Image> {
        let r = r.as_ref()?;
        self.images.want(&r.key, r.src.clone(), r.w, r.crop, p)
    }

    // ------------------------------------------------------------------ artwork choices

    pub fn tile_req(&self, item: RowItem) -> Option<ImgReq> {
        let (info, local, cover) = match item {
            RowItem::Local(l) => (self.locals[l].info.as_ref(), Some(&self.locals[l].l), self.locals[l].cat.map(|c| self.games[c].g.cover.clone())),
            RowItem::Cat(g) => (self.games[g].info.as_ref(), None, Some(self.games[g].g.cover.clone())),
            RowItem::All => return None,
        };
        if let Some(r) = info.and_then(|i| req_url(&i.master, 256, 0.0)) {
            return Some(r);
        }
        if let Some(p) = local.and_then(|l| l.icon0.as_ref()) {
            return Some(req_file(p, 256));
        }
        if let Some(r) = cover.and_then(|c| req_url(&c, 256, COVER_CROP)) {
            return Some(r);
        }
        info.and_then(|i| req_url(&i.hub, 512, 0.0))
    }

    pub fn target_bg_url(&self, t: Target) -> Option<ImgReq> {
        let info = self.target_info(t);
        if let Some(r) = info.as_ref().and_then(|i| req_url(&i.hub, 1920, 0.0)) {
            return Some(r);
        }
        if let Some(p) = t.local.and_then(|l| self.locals[l].l.pic0.clone()) {
            return Some(req_file(&p, 1920));
        }
        let g = t.game.map(|g| &self.games[g].g)?;
        yt_art(&g.trailer).or_else(|| req_url(&g.cover, 1280, COVER_CROP))
    }

    /// The small cover shown while a big background downloads (usually already on disk).
    pub fn cover_req(&self, t: Target) -> Option<ImgReq> {
        t.game.and_then(|g| self.card_req(g))
            .or_else(|| t.local.and_then(|l| self.locals[l].l.icon0.clone()).map(|p| req_file(&p, 512)))
    }

    pub fn row_cover_req(&self, i: usize) -> Option<ImgReq> {
        match self.row.get(i)? {
            RowItem::All => None,
            _ => self.cover_req(self.row_target(i)),
        }
    }

    /// Show row item `i`'s background, with its cover standing in until it has downloaded.
    pub fn show_row_background(&mut self, i: usize, force: bool) {
        let (bg, cover) = (self.hero_bg_url(i), self.row_cover_req(i));
        self.want_background_or(bg, force, cover);
    }

    pub fn hero_bg_url(&self, i: usize) -> Option<ImgReq> {
        match self.row.get(i)? {
            RowItem::All => {
                let g = self.games.iter().find(|g| g.info.as_ref().is_some_and(|i| !i.hub.is_empty()))?;
                req_url(&g.info.as_ref()?.hub, 1920, 0.0)
            }
            _ => self.target_bg_url(self.row_target(i)),
        }
    }

    fn logo_req(info: Option<&Info>) -> Option<ImgReq> {
        info.and_then(|i| req_url(&i.logo, 960, 0.0))
    }

    pub fn card_req(&self, gi: usize) -> Option<ImgReq> {
        let g = &self.games[gi];
        let info = g.info.as_ref();
        info.and_then(|i| req_url(&i.portrait, 440, 0.0))
            .or_else(|| info.and_then(|i| req_url(&i.master, 440, 0.0)))
            .or_else(|| req_url(&g.g.cover, 440, COVER_CROP))
            .or_else(|| info.and_then(|i| req_url(&i.hub, 640, 0.0)))
    }

    // ------------------------------------------------------------------ push everything

    pub fn push_all(&mut self) {
        self.push_row();
        self.push_hero();
        self.push_library();
        if self.overlay == Overlay::Hub {
            self.push_hub();
        }
        if self.view == 0 && self.overlay == Overlay::None {
            self.show_row_background(self.sel, true);
        }
    }

    // ------------------------------------------------------------------ home

    pub fn push_row(&mut self) {
        let items = self.row.clone();
        let mut tiles = Vec::with_capacity(items.len());
        let mut keys = Vec::with_capacity(items.len());
        for (i, it) in items.iter().enumerate() {
            let req = self.tile_req(*it);
            let p = if i.abs_diff(self.sel) < 12 { prio::TILE } else { prio::PREFETCH };
            let img = self.get_img(&req, p);
            keys.push(req.map(|r| r.key).unwrap_or_default());
            tiles.push(TileData { loaded: img.is_some(), image: img.unwrap_or_default(), special: *it == RowItem::All, badge: self.tile_badge(*it).into() });
        }
        self.tile_keys = keys;
        self.tile_model.set_vec(tiles);
        let ui = self.ui();
        ui.set_sel(self.sel as i32);
        self.push_row_text();
    }

    fn tile_badge(&self, it: RowItem) -> &'static str {
        match it {
            RowItem::Local(l) if self.session_for_local(l).is_some() => "Playing",
            RowItem::Local(_) => "Installed",
            _ => "",
        }
    }

    pub fn push_row_text(&mut self) {
        let (name, sub) = match self.row.get(self.sel).copied() {
            None => (String::new(), String::new()),
            Some(RowItem::All) => ("Game Library".into(), format!("{} games", self.games.len())),
            Some(RowItem::Local(l)) => {
                let lv = &self.locals[l];
                let sub = if let Some(s) = self.session_for_local(l) {
                    format!("Playing · {}", util::fmt_clock(util::now_secs() - s.since))
                } else {
                    let pt = self.playtime(&lv.l);
                    let status = self.target_compat(Target { game: lv.cat, local: Some(l) }).map(|e| e.status.label().to_lowercase());
                    if pt.last > 0.0 {
                        format!("Last played {}", util::fmt_last_played(pt.last).to_lowercase())
                    } else if let Some(s) = status {
                        format!("Installed · {s} on KytyPS5")
                    } else {
                        "Installed · Ready to play".into()
                    }
                };
                (lv.name.clone(), sub)
            }
            Some(RowItem::Cat(g)) => {
                let gv = &self.games[g];
                let genre = gv.info.as_ref().and_then(|i| i.genres.first().cloned()).or_else(|| gv.g.genres.first().cloned()).unwrap_or_default();
                let status = self.game_compat(gv).map(|e| format!("{} on KytyPS5", e.status.label())).unwrap_or_default();
                (gv.name.clone(), [genre, gv.g.size.clone(), status].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "))
            }
        };
        let ui = self.ui();
        ui.set_row_name(name.into());
        ui.set_row_sub(sub.into());
    }

    fn kicker_for(&self, t: Target) -> (String, String, bool) {
        if let Some(l) = t.local {
            if let Some(s) = self.session_for_local(l) {
                return (util::fmt_clock(util::now_secs() - s.since), "● PLAYING".into(), true);
            }
            let lv = &self.locals[l];
            let pt = self.playtime(&lv.l);
            let text = if pt.total > 0.0 {
                format!("PLAYED {} · {}", util::fmt_duration(pt.total).to_uppercase(), util::fmt_last_played(pt.last).to_uppercase())
            } else {
                lv.l.title_id.clone()
            };
            return (text, "● INSTALLED".into(), false);
        }
        if let Some(g) = t.game {
            let gv = &self.games[g];
            let added = util::fmt_date(util::parse_iso_date(&gv.g.date)).to_uppercase();
            return (if gv.is_new { format!("NEW · ADDED {added}") } else { format!("ADDED {added}") }, String::new(), false);
        }
        (String::new(), String::new(), false)
    }

    fn chips_for(&self, t: Target) -> Vec<ChipData> {
        let info = self.target_info(t);
        let g = t.game.map(|g| &self.games[g]);
        let mut c: Vec<ChipData> = self.target_compat(t).map(|e| compat_chip(Some(e))).into_iter().collect();
        if let Some(r) = info.as_ref().and_then(|i| i.rating.as_ref()) {
            c.push(ChipData { stars: r.score as f32, ..chip("", format!("{:.1} · {} ratings", r.score, thousands(r.total)), false) });
        }
        let genres: Vec<String> = info.as_ref().filter(|i| !i.genres.is_empty()).map(|i| i.genres.clone()).or_else(|| g.map(|g| g.g.genres.clone())).unwrap_or_default();
        if !genres.is_empty() {
            c.push(chip("tag", genres.iter().take(2).cloned().collect::<Vec<_>>().join(" · "), false));
        }
        if let Some(g) = g.filter(|g| !g.g.size.is_empty()) {
            c.push(chip("disk", g.g.size.clone(), false));
        }
        let rel = g.map(|g| g.g.release.clone()).filter(|r| !r.is_empty()).or_else(|| info.as_ref().map(|i| util::fmt_date(util::parse_iso_date(&i.release)))).unwrap_or_default();
        if !rel.is_empty() {
            c.push(chip("cal", rel, false));
        }
        if g.is_none() {
            if let Some(l) = t.local {
                c.push(chip("tag", self.locals[l].l.title_id.clone(), false));
            }
        }
        c
    }

    pub fn push_hero_kicker(&mut self) {
        let ui = self.ui();
        let mut h = ui.get_hero();
        if let Some(it) = self.row.get(self.sel).copied() {
            if it != RowItem::All {
                let (k, a, live) = self.kicker_for(self.row_target(self.sel));
                h.kicker = k.into();
                h.kicker_accent = a.into();
                h.kicker_live = live;
                ui.set_hero(h);
            }
        }
    }

    pub fn push_hero(&mut self) {
        let it = self.row.get(self.sel).copied();
        let mut h = HeroData::default();
        self.hero_logo_key.clear();
        match it {
            None => {
                h.title = if self.syncing { "Loading catalog…".into() } else { "No games yet".into() };
                h.desc = "Open Settings to add game folders or refresh the catalog.".into();
                self.hero_actions = Vec::new();
            }
            Some(RowItem::All) => {
                h.kicker = "COLLECTION".into();
                h.title = "Game Library".into();
                h.chips = model(vec![chip("grid", format!("{} games", self.games.len()), false)]);
                h.desc = "Browse every PS5 game in the catalog. Search, filter by genre and sort by date, name, rating or size.".into();
                self.hero_actions = vec![ActionDef { id: "library", label: "Open Library".into(), icon: "grid", primary: true, danger: false, round: false }];
            }
            Some(_) => {
                let t = self.row_target(self.sel);
                let info = self.target_info(t);
                let (k, a, live) = self.kicker_for(t);
                h.kicker = k.into();
                h.kicker_accent = a.into();
                h.kicker_live = live;
                h.title = target_name(self, t).into();
                let lr = Self::logo_req(info.as_ref());
                if let Some(r) = &lr {
                    self.hero_logo_key = r.key.clone();
                    match self.get_img(&lr, prio::HERO) {
                        Some(img) => {
                            let (w, hh) = logo_size(&img, 620.0, 200.0);
                            h.logo = img;
                            h.has_logo = true;
                            h.logo_w = w * self.scale;
                            h.logo_h = hh * self.scale;
                        }
                        // On disk: it decodes in a few ms, so keep the space empty instead of flashing
                        // the text name. Still downloading: show the name as text meanwhile.
                        None if self.images.on_disk(&r.key, &r.src) => h.title = SharedString::default(),
                        None => {}
                    }
                }
                h.chips = model(self.chips_for(t));
                let g = t.game.map(|g| &self.games[g].g);
                let desc = info.as_ref().map(|i| i.short.clone()).filter(|s| s.chars().count() > 30 && s.chars().any(|c| c.is_lowercase()))
                    .or_else(|| g.and_then(|g| g.description.first().cloned()))
                    .or_else(|| g.map(|g| g.excerpt.clone()))
                    .unwrap_or_else(|| "Installed locally. Launches with the KytyPS5 emulator.".into());
                h.desc = truncate(&desc, 260).into();
                self.hero_actions = self.actions_for(t, false);
            }
        }
        h.actions = model(self.hero_actions.iter().map(action_data).collect());
        if self.zone == Z_ACTIONS && self.idx as usize >= self.hero_actions.len() {
            self.set_focus(Z_ACTIONS, self.hero_actions.len().saturating_sub(1) as i32);
        }
        self.ui().set_hero(h);
    }

    pub fn prefetch_neighbors(&mut self) {
        for d in [1i64, -1, 2] {
            let i = self.sel as i64 + d;
            if i < 0 || i as usize >= self.row.len() {
                continue;
            }
            let i = i as usize;
            if let Some(r) = self.hero_bg_url(i) {
                self.images.want(&r.key, r.src, r.w, r.crop, prio::PREFETCH);
            }
            let info = match self.row[i] {
                RowItem::All => None,
                _ => self.target_info(self.row_target(i)),
            };
            if let Some(r) = Self::logo_req(info.as_ref()) {
                self.images.want(&r.key, r.src, r.w, r.crop, prio::PREFETCH + 1);
            }
        }
    }

    // ------------------------------------------------------------------ background

    /// Crossfade the backdrop to `req`, with a stand-in image (usually already on disk) to show
    /// while `req` downloads.
    pub fn want_background_or(&mut self, req: Option<ImgReq>, force: bool, standin: Option<ImgReq>) {
        let Some(r) = req else { return };
        if r.key == self.bg_key && !force {
            return;
        }
        if r.key == self.bg_key {
            return;
        }
        self.bg_pending = Some(r.key.clone());
        self.bg_standin = None;
        let keep = r.key.clone();
        // Moving quickly: drop queued backgrounds we already moved past.
        self.images.cancel_unless(move |j| j.prio != prio::HERO || j.key == keep || j.max_w < 1000);
        let downloading = !self.images.on_disk(&r.key, &r.src);
        if let Some(img) = self.images.want(&r.key, r.src, r.w, r.crop, prio::HERO) {
            self.apply_bg(r.key, img);
            return;
        }
        // Not downloaded yet (≈1 s from Sony's servers): show the game's cover meanwhile, so the
        // previous game's art never lingers behind this game's details.
        if downloading {
            if let Some(c) = standin {
                self.bg_standin = Some(c.key.clone());
                if let Some(img) = self.images.want(&c.key, c.src, c.w, c.crop, prio::HERO) {
                    self.apply_bg_standin(img);
                }
            }
        }
    }

    fn apply_bg_standin(&mut self, img: Image) {
        self.bg_standin = None;
        self.bg_key = String::new();
        self.bg_show_b = !self.bg_show_b;
        let ui = self.ui();
        if self.bg_show_b {
            ui.set_bg_b(img);
        } else {
            ui.set_bg_a(img);
        }
        ui.set_bg_show_b(self.bg_show_b);
    }

    fn apply_bg(&mut self, key: String, img: Image) {
        self.bg_pending = None;
        self.bg_key = key;
        self.bg_show_b = !self.bg_show_b;
        let ui = self.ui();
        if self.bg_show_b {
            ui.set_bg_b(img);
        } else {
            ui.set_bg_a(img);
        }
        ui.set_bg_show_b(self.bg_show_b);
    }

    /// An image finished loading: update whatever shows it.
    pub fn refresh_images(&mut self, key: &str) {
        if self.bg_standin.as_deref() == Some(key) && self.bg_pending.is_some() {
            if let Some(img) = self.images.get(key) {
                self.apply_bg_standin(img);
            }
        }
        if self.bg_pending.as_deref() == Some(key) {
            if let Some(img) = self.images.get(key) {
                self.apply_bg(key.to_string(), img);
            }
        }
        if let Some(i) = self.tile_keys.iter().position(|k| k == key) {
            if let (Some(img), Some(mut t)) = (self.images.get(key), self.tile_model.row_data(i)) {
                t.image = img;
                t.loaded = true;
                self.tile_model.set_row_data(i, t);
            }
        }
        if key == self.hero_logo_key {
            self.push_hero();
        }
        if self.grid_keys.contains(key) {
            self.push_grid_rows();
        }
        if self.hub_keys.contains(key) {
            self.push_hub();
        }
        if key == self.viewer_key {
            self.push_viewer();
        }
        if self.launch_keys.iter().any(|k| k == key) {
            if let Some(l) = self.launch_local {
                self.push_launch(l);
            }
        }
        self.boot_image_loaded(key);
    }

    // ------------------------------------------------------------------ library

    /// Recompute grid geometry from the window size.
    pub fn relayout(&mut self) {
        let (w, _) = self.logical_size();
        let inner = (w - 192.0).max(400.0);
        let (min_w, gap) = (190.0, 24.0);
        self.cols = (((inner + gap) / (min_w + gap)).floor() as usize).max(2);
        self.card_w = (inner - (self.cols as f32 - 1.0) * gap) / self.cols as f32;
        self.row_h = self.card_w * 1.5 + 14.0 + 50.0 + 24.0 + 30.0;
        let (ui, k) = (self.ui(), self.scale);
        ui.set_card_w(self.card_w * k);
        ui.set_row_h(self.row_h * k);
        ui.set_grid_gap(gap * k);
    }

    /// Window size in design units (the 1920×1080 canvas the UI is laid out on).
    pub fn logical_size(&self) -> (f32, f32) {
        let ui = self.ui();
        let s = ui.window().size();
        let sf = ui.window().scale_factor().max(0.1);
        if s.width == 0 {
            return (1920.0, 1080.0);
        }
        (s.width as f32 / sf / self.scale, s.height as f32 / sf / self.scale)
    }

    /// Map the design canvas onto the real window. Returns true if the scale changed.
    pub fn update_scale(&mut self) -> bool {
        let ui = self.ui();
        let s = ui.window().size();
        let sf = ui.window().scale_factor().max(0.1);
        if s.width < 100 || s.height < 100 {
            return false;
        }
        let (w, h) = (s.width as f32 / sf, s.height as f32 / sf);
        let user = std::env::var("PS5_LAUNCHER_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let k = ((w / 1920.0).min(h / 1080.0) * user).clamp(0.3, 4.0);
        if (k - self.scale).abs() < 0.001 {
            return false;
        }
        self.scale = k;
        ui.global::<crate::S>().set_k(k);
        true
    }

    pub fn push_library(&mut self) {
        let ui = self.ui();
        ui.set_sort_label(SORTS[self.sort].into());
        self.push_genres();
        self.push_grid();
    }

    pub fn push_genres(&mut self) {
        let mut x = 0.0f32;
        let mut chips = Vec::new();
        let mut focus_x = 0.0;
        for (i, (label, count)) in self.genre_list.iter().enumerate() {
            let c = count.to_string();
            let w = 40.0 + label.chars().count() as f32 * 10.4 + 6.0 + c.len() as f32 * 9.0;
            if (self.zone == Z_CHIPS && i as i32 == self.idx) || (self.zone != Z_CHIPS && *label == self.genre) {
                focus_x = x;
            }
            chips.push(GenreChip { label: label.clone().into(), count: c.into(), on: *label == self.genre, x: x * self.scale, w: w * self.scale });
            x += w + 10.0;
        }
        let (win_w, _) = self.logical_size();
        // Keep the last chip clear of the right-edge fade.
        let view_w = win_w - 92.0 - 110.0;
        let max_scroll = (x - view_w).max(0.0);
        let scroll = (focus_x - view_w * 0.35).clamp(0.0, max_scroll);
        let ui = self.ui();
        ui.set_genres(model(chips));
        ui.set_genres_x(-scroll * self.scale);
        ui.set_genres_more_right(x - scroll > view_w + 60.0);
    }

    pub fn push_grid(&mut self) {
        let rows = self.filtered.len().div_ceil(self.cols.max(1));
        let ui = self.ui();
        ui.set_grid_h((rows as f32 * self.row_h + 60.0) * self.scale);
        ui.set_count_label(format!("{} of {} games", self.filtered.len(), self.games.len()).into());
        ui.set_grid_empty(if !self.filtered.is_empty() {
            "".into()
        } else if self.games.is_empty() {
            (if self.syncing { "Loading catalog…" } else { "Catalog is empty" }).into()
        } else {
            "No games match your search".into()
        });
        if self.zone == Z_GRID && self.idx as usize >= self.filtered.len() {
            let i = self.filtered.len().saturating_sub(1) as i32;
            self.set_focus(if self.filtered.is_empty() { Z_CHIPS } else { Z_GRID }, i.max(0));
        }
        self.grid_window = (usize::MAX, 0);
        self.push_grid_window();
    }

    /// Only rows around the viewport exist as UI elements; this keeps the grid instant with any catalog size.
    pub fn push_grid_window(&mut self) {
        let (_, h) = self.logical_size();
        let view_h = h - 262.0;
        let y = -self.ui().get_grid_y() / self.scale;
        let rows = self.filtered.len().div_ceil(self.cols.max(1));
        let first = ((y / self.row_h).floor() as i64 - 1).max(0) as usize;
        let last = (((y + view_h) / self.row_h).ceil() as usize + 1).min(rows);
        if (first, last) == self.grid_window {
            return;
        }
        self.grid_window = (first, last);
        self.push_grid_rows();
        // Look one screen ahead so scrolling finds covers ready.
        let ahead_from = last * self.cols;
        let ahead_to = ((last + 3) * self.cols).min(self.filtered.len());
        for k in ahead_from..ahead_to {
            let r = self.card_req(self.filtered[k]);
            if let Some(r) = r {
                self.images.want(&r.key, r.src, r.w, r.crop, prio::PREFETCH);
            }
        }
    }

    pub fn push_grid_rows(&mut self) {
        let (first, last) = self.grid_window;
        let mut rows = Vec::new();
        let mut keys = HashSet::new();
        for r in first..last.max(first) {
            let mut cards = Vec::new();
            for c in 0..self.cols {
                let k = r * self.cols + c;
                let Some(&gi) = self.filtered.get(k) else { break };
                let req = self.card_req(gi);
                let img = self.get_img(&req, prio::CARD);
                if let Some(r) = &req {
                    keys.insert(r.key.clone());
                }
                let g = &self.games[gi];
                let ce = self.game_compat(g);
                let (compat, compat_level) = ce.map(|e| (e.status.label(), e.status.level())).unwrap_or(("", 0));
                // Short genre names ("RPG", not "Role Playing Games") keep the line from being cut off.
                let genre = g.buckets.first().filter(|b| **b != "Other").map(|b| b.to_string())
                    .or_else(|| g.info.as_ref().and_then(|i| i.genres.first().cloned()))
                    .or_else(|| g.g.genres.first().cloned()).unwrap_or_default();
                let region = g.g.version.split(['–', '-']).nth(1).map(|s| s.trim().to_string()).filter(|s| s.len() <= 5).unwrap_or_default();
                let meta = [genre, g.g.size.clone(), region].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                cards.push(CardData {
                    index: k as i32,
                    title: g.name.clone().into(),
                    meta: meta.into(),
                    rating: g.info.as_ref().and_then(|i| i.rating.as_ref()).map(|r| format!("{:.1}", r.score)).unwrap_or_default().into(),
                    badge: if g.local.is_some() { "Installed".into() } else if g.is_new { "New".into() } else { "".into() },
                    compat: compat.into(),
                    compat_level,
                    loaded: img.is_some(),
                    image: img.unwrap_or_default(),
                });
            }
            rows.push(CardRow { index: r as i32, cards: model(cards) });
        }
        // Cancel queued covers for rows that scrolled away.
        let keep = keys.clone();
        self.images.cancel_unless(move |j| j.prio != prio::CARD || keep.contains(&j.key));
        self.grid_keys = keys;
        self.grid_model.set_vec(rows);
    }

    pub fn first_visible_card(&self) -> usize {
        let y = -self.ui().get_grid_y() / self.scale;
        // First row whose top is on screen (a row scrolled away by a few pixels still counts).
        let row = ((y - 20.0) / self.row_h).ceil().max(0.0) as usize;
        (row * self.cols).min(self.filtered.len().saturating_sub(1))
    }

    pub fn ensure_grid_visible(&mut self) {
        let (_, h) = self.logical_size();
        let view_h = h - 262.0;
        let row = self.idx.max(0) as usize / self.cols.max(1);
        let top = row as f32 * self.row_h;
        let y = -self.ui().get_grid_y() / self.scale;
        let new_y = if top < y + 10.0 {
            (top - 10.0).max(0.0)
        } else if top + self.row_h > y + view_h {
            // Snap to a row boundary so no half-row peeks out under the header.
            let min_y = top + self.row_h - view_h + 20.0;
            let snapped = (min_y / self.row_h).ceil() * self.row_h - 10.0;
            if snapped <= top - 10.0 { snapped } else { min_y }
        } else {
            return;
        };
        self.ui().set_grid_y(-new_y * self.scale);
        self.push_grid_window();
    }

    /// The Library's backdrop follows the selected game, once the selection rests for a moment
    /// (scrolling past twenty games shouldn't start twenty full-screen downloads).
    pub fn focus_card_changed(&mut self) {
        self.bg_timer.start(slint::TimerMode::SingleShot, std::time::Duration::from_millis(160), || {
            crate::app::with_app(|app| {
                if app.view != 1 || app.zone != Z_GRID || app.overlay != Overlay::None {
                    return;
                }
                if let Some(&gi) = app.filtered.get(app.idx as usize) {
                    let t = Target { game: Some(gi), local: app.games[gi].local };
                    let (bg, cover) = (app.target_bg_url(t), app.cover_req(t));
                    app.want_background_or(bg, false, cover);
                }
            })
        });
    }

    pub fn cycle_sort(&mut self, d: i32) {
        self.sort = (self.sort as i32 + d).rem_euclid(SORTS.len() as i32) as usize;
        audio_move();
        self.apply_filter();
        self.ui().set_grid_y(0.0);
        self.push_library();
    }

    // ------------------------------------------------------------------ game hub

    pub fn push_hub(&mut self) {
        let Some(t) = self.hub else { return };
        let info = self.target_info(t);
        let mut keys = HashSet::new();
        let mut h = HubData::default();
        let (k, a, live) = self.kicker_for(t);
        h.kicker = k.into();
        h.kicker_accent = a.into();
        h.kicker_live = live;
        h.title = target_name(self, t).into();

        let lr = Self::logo_req(info.as_ref());
        if let Some(r) = &lr {
            keys.insert(r.key.clone());
            match self.get_img(&lr, prio::DETAIL) {
                Some(img) => {
                    let (w, hh) = logo_size(&img, 700.0, 220.0);
                    h.logo = img;
                    h.has_logo = true;
                    h.logo_w = w * self.scale;
                    h.logo_h = hh * self.scale;
                }
                None if self.images.on_disk(&r.key, &r.src) => h.title = SharedString::default(),
                None => {}
            }
        }
        // Same image as the Library card: it's already on disk, so the Hub opens with it instantly.
        let cover = t.game.and_then(|g| self.card_req(g))
            .or_else(|| info.as_ref().and_then(|i| req_url(&i.portrait, 440, 0.0)))
            .or_else(|| info.as_ref().and_then(|i| req_url(&i.master, 440, 0.0)))
            .or_else(|| t.local.and_then(|l| self.locals[l].l.icon0.clone()).map(|p| req_file(&p, 512)));
        if let Some(r) = &cover {
            keys.insert(r.key.clone());
        }
        if let Some(img) = self.get_img(&cover, prio::DETAIL) {
            h.cover = img;
            h.cover_loaded = true;
        }

        let mut chips = vec![compat_chip(self.target_compat(t))];
        if let Some(r) = info.as_ref().and_then(|i| i.rating.as_ref()) {
            chips.push(ChipData { stars: r.score as f32, ..chip("", format!("{:.2} · {} ratings", r.score, thousands(r.total)), false) });
        }
        if let Some(a) = info.as_ref().and_then(|i| i.age.as_ref()).filter(|a| !a.name.is_empty()) {
            chips.push(chip("", a.name.clone(), false));
        }
        let genres: Vec<String> = info.as_ref().filter(|i| !i.genres.is_empty()).map(|i| i.genres.clone())
            .or_else(|| t.game.map(|g| self.games[g].buckets.iter().map(|s| s.to_string()).collect())).unwrap_or_default();
        for g in genres.into_iter().take(3) {
            chips.push(chip("", g, false));
        }
        h.chips = model(chips);

        self.hub_actions = self.actions_for(t, true);
        h.actions = model(self.hub_actions.iter().map(action_data).collect());

        let g_owned = t.game.map(|i| self.games[i].g.clone());
        let g = g_owned.as_ref();
        let region = g.and_then(|g| g.version.split(['–', '-']).nth(1).map(|s| s.trim().to_string())).unwrap_or_default();
        let ce = self.target_compat(t).cloned();
        let mut facts: Vec<(&str, String)> = vec![
            ("KYTYPS5", match &ce {
                Some(e) => format!("{} · {}", e.status.meaning(), if e.reports == 1 { "1 report".into() } else { format!("{} reports", e.reports) }),
                None => "No reports yet".into(),
            }),
            ("LAST TESTED", ce.as_ref().map(|e| {
                let d = util::fmt_date(util::parse_iso_date(&e.tested_date()));
                let p = e.platforms.join(", ");
                [if d.is_empty() { e.tested_date() } else { d }, p].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
            }).unwrap_or_default()),
            ("ON LINUX", ce.as_ref().and_then(|e| e.linux).filter(|l| Some(*l) != ce.as_ref().map(|e| e.status)).map(|l| l.label().to_string()).unwrap_or_default()),
            ("PUBLISHER", info.as_ref().map(|i| i.publisher.clone()).unwrap_or_default()),
            ("RELEASE", g.map(|g| g.release.clone()).filter(|s| !s.is_empty()).or_else(|| info.as_ref().map(|i| util::fmt_date(util::parse_iso_date(&i.release)))).unwrap_or_default()),
            ("SIZE", g.map(|g| g.size.clone()).unwrap_or_default()),
            ("MODE", g.map(|g| g.mode.clone()).filter(|s| !s.is_empty()).or_else(|| info.as_ref().and_then(|i| i.players.as_ref()).and_then(|p| p.as_i64()).map(|p| format!("{p} player{}", if p > 1 { "s" } else { "" }))).unwrap_or_default()),
            ("TITLE ID", g.map(|g| g.title_id.clone()).filter(|s| !s.is_empty()).or_else(|| t.local.map(|l| self.locals[l].l.title_id.clone())).unwrap_or_default()),
            ("REGION", region),
            ("UPDATE", g.map(|g| g.update.clone()).unwrap_or_default()),
            ("ADDED", g.map(|g| util::fmt_date(util::parse_iso_date(&g.date))).unwrap_or_default()),
        ];
        if let Some(l) = t.local {
            let lv = &self.locals[l];
            let pt = self.playtime(&lv.l);
            facts.push(("PLAY TIME", if pt.total > 0.0 { util::fmt_duration(pt.total) } else { "Never played".into() }));
            facts.push(("LAST PLAYED", util::fmt_last_played(pt.last)));
            facts.push(("INSTALLED VERSION", lv.l.version.clone()));
            facts.push(("LOCATION", util::display_path(&lv.l.path.to_string_lossy())));
        }
        let facts: Vec<Fact> = facts.into_iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| Fact { key: k.into(), value: v.into() }).collect();
        h.facts = model(facts.chunks(3).map(|c| FactRow { items: model(c.to_vec()) }).collect());

        let shots_urls = self.hub_shots.clone();
        let mut shots = Vec::new();
        for u in &shots_urls {
            let r = req_url(u, 480, 0.0);
            if let Some(r) = &r {
                keys.insert(r.key.clone());
            }
            let img = self.get_img(&r, prio::DETAIL - 1);
            shots.push(Shot { loaded: img.is_some(), image: img.unwrap_or_default() });
        }
        h.shots = model(shots);

        let about: Vec<SharedString> = match info.as_ref().filter(|i| !i.long.is_empty()) {
            Some(i) => i.long.split('\n').map(str::trim).filter(|s| !s.is_empty()).map(SharedString::from).collect(),
            None => g.map(|g| if g.description.is_empty() { vec![g.excerpt.clone()] } else { g.description.clone() }).unwrap_or_default()
                .into_iter().filter(|s| !s.is_empty()).map(SharedString::from).collect(),
        };
        h.about = model(about);
        self.hub_keys = keys;
        if self.overlay == Overlay::Hub && self.zone == Z_HUB && self.idx as usize >= self.hub_actions.len() {
            self.set_focus(Z_HUB, 0);
        }
        self.ui().set_hub(h);
    }

    pub fn scroll_hub(&mut self) {
        // Rough layout estimate (see app.slint hub column).
        let facts_rows = self.ui().get_hub().facts.row_count() as f32;
        let shots_top = 130.0 + 30.0 + 16.0 + 210.0 + 22.0 + 40.0 + 30.0 + 68.0 + 40.0 + facts_rows * 106.0;
        let y = match self.zone {
            Z_SHOTS => (shots_top - 320.0).max(0.0),
            Z_DESC => shots_top + if self.hub_shots.is_empty() { 0.0 } else { 230.0 } - 200.0 + self.idx as f32 * 360.0,
            _ => 0.0,
        };
        self.ui().set_hub_y(-y.max(0.0) * self.scale);
    }

    pub fn scroll_shots(&mut self) {
        let (w, _) = self.logical_size();
        let view = (w - 540.0 - 96.0).min(1060.0);
        let x = self.idx as f32 * 314.0;
        let total = self.hub_shots.len() as f32 * 314.0 + 12.0;
        self.ui().set_shots_x(-(x - view * 0.4).clamp(0.0, (total - view).max(0.0)) * self.scale);
    }

    pub fn push_viewer(&mut self) {
        let Some(u) = self.hub_shots.get(self.viewer_i).cloned() else { return };
        let r = req_url(&u, 1920, 0.0);
        self.viewer_key = r.as_ref().map(|r| r.key.clone()).unwrap_or_default();
        let img = self.get_img(&r, prio::HERO);
        // Preload the neighbours for instant browsing.
        for d in [1, self.hub_shots.len().saturating_sub(1)] {
            if let Some(n) = self.hub_shots.get((self.viewer_i + d) % self.hub_shots.len().max(1)).cloned() {
                let r = req_url(&n, 1920, 0.0);
                self.get_img(&r, prio::PREFETCH);
            }
        }
        let ui = self.ui();
        // Show the already-loaded thumbnail instantly while the full-size image decodes.
        let thumb = req_url(&u, 480, 0.0).and_then(|r| self.images.get(&r.key));
        if let Some(img) = img.or(thumb) {
            ui.set_viewer_image(img);
        }
        ui.set_viewer_count(format!("{} / {}", self.viewer_i + 1, self.hub_shots.len()).into());
    }

    pub fn push_menu(&mut self, title: &str) {
        let items: Vec<MenuItem> = self.menu_actions.iter().map(|a| MenuItem { label: a.label.clone().into(), icon: a.icon.into() }).collect();
        let ui = self.ui();
        ui.set_menu_title(title.into());
        ui.set_menu_items(model(items));
    }

    pub fn push_launch(&mut self, l: usize) {
        self.launch_local = Some(l);
        let t = Target { game: self.locals[l].cat, local: Some(l) };
        let bg = self.target_bg_url(t);
        let icon = self.tile_req(RowItem::Local(l));
        self.launch_keys = [&bg, &icon].iter().filter_map(|r| r.as_ref().map(|r| r.key.clone())).collect();
        let (bgi, ici) = (self.get_img(&bg, prio::HERO), self.get_img(&icon, prio::HERO));
        let ui = self.ui();
        ui.set_launch_name(self.locals[l].name.clone().into());
        ui.set_launch_bg(bgi.unwrap_or_default());
        ui.set_launch_icon(ici.unwrap_or_default());
    }

    pub fn push_toasts(&mut self) {
        self.ui().set_toasts(model(self.toasts.clone()));
    }
}

fn audio_move() {
    crate::audio::play(crate::audio::Sound::Move);
}

pub fn thousands(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{}…", cut.rsplit_once(' ').map(|x| x.0).unwrap_or(&cut))
}
