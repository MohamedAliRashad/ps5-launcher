//! Monitors, window placement, focus, and external video playback.

use slint::winit_030::{winit, WinitWindowAccessor};
use slint::ComponentHandle;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Monitor {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

/// Connected displays from `xrandr --listmonitors` (X11). Empty on failure.
pub fn monitors() -> Vec<Monitor> {
    let Ok(out) = Command::new("xrandr").arg("--listmonitors").stderr(Stdio::null()).output() else { return Vec::new() };
    let re = regex::Regex::new(r"^\s*\d+:\s*\+?(\*?)(\S+)\s+(\d+)/\d+x(\d+)/\d+([+-]\d+)([+-]\d+)").unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let c = re.captures(l)?;
            Some(Monitor {
                primary: !c[1].is_empty(),
                name: c[2].trim_start_matches(['+', '*']).to_string(),
                w: c[3].parse().ok()?,
                h: c[4].parse().ok()?,
                x: c[5].parse().ok()?,
                y: c[6].parse().ok()?,
            })
        })
        .collect()
}

pub fn pick(mons: &[Monitor], pref: &str) -> Option<Monitor> {
    mons.iter()
        .find(|m| !pref.is_empty() && m.name == pref)
        .or_else(|| mons.iter().find(|m| m.primary))
        .or_else(|| mons.first())
        .cloned()
}

/// Put the window on one monitor (never spanning two) and make it borderless fullscreen.
/// The native window only exists once the event loop runs, so this retries until the window
/// is there and the window manager has actually made it fullscreen.
pub fn place_window(ui: &crate::AppWindow, target: Option<&Monitor>, windowed: bool) {
    fn attempt(weak: slint::Weak<crate::AppWindow>, target: Option<Monitor>, windowed: bool, tries: u32) {
        let Some(ui) = weak.upgrade() else { return };
        let done = place_now(&ui, target.as_ref(), windowed);
        if !done && tries > 0 {
            slint::Timer::single_shot(std::time::Duration::from_millis(120), move || attempt(weak, target, windowed, tries - 1));
        }
    }
    attempt(ui.as_weak(), target.cloned(), windowed, 25);
}

/// Returns true once the window exists and is in the requested state.
fn place_now(ui: &crate::AppWindow, target: Option<&Monitor>, windowed: bool) -> bool {
    let name = target.map(|m| m.name.clone());
    let geom = target.map(|m| (m.x, m.y, m.w, m.h));
    ui.window().with_winit_window(move |w: &winit::window::Window| {
        if !windowed && w.fullscreen().is_some() {
            return true;
        }
        let handle = name.as_ref().and_then(|n| w.available_monitors().find(|m| m.name().as_deref() == Some(n.as_str())));
        if windowed {
            if let Some((x, y, mw, mh)) = geom {
                let (ww, wh) = (1600.min(mw - 80), 900.min(mh - 80));
                let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(ww, wh));
                w.set_outer_position(winit::dpi::PhysicalPosition::new(x + (mw - ww) as i32 / 2, y + (mh - wh) as i32 / 2));
            }
        } else {
            if let Some((x, y, mw, mh)) = geom {
                w.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                // Fill the monitor even if the window manager ignores the fullscreen hint.
                let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(mw, mh));
            }
            w.set_fullscreen(Some(winit::window::Fullscreen::Borderless(handle.or_else(|| w.current_monitor()))));
        }
        w.focus_window();
        windowed
    })
    .unwrap_or(false)
}

pub fn window_has_focus(ui: &crate::AppWindow) -> bool {
    ui.window().with_winit_window(|w: &winit::window::Window| w.has_focus()).unwrap_or(true)
}


