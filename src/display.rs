//! Monitors, window placement, focus, and external video playback.

use slint::winit_030::{winit, WinitWindowAccessor};
#[cfg(target_os = "macos")]
use slint::winit_030::winit::platform::macos::MonitorHandleExtMacOS;
use slint::ComponentHandle;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Monitor {
    /// macOS: the CoreGraphics display id, which is how winit's copy of this display is found.
    /// Zero elsewhere.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only macOS reads it
    pub id: u32,
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

/// One JXA call (JavaScript for Automation: osascript's JavaScript mode) that asks AppKit for the
/// displays and the one under the mouse pointer. Names are the ones System Settings shows. The
/// display id is how the window finds the same display in winit, which names displays "Monitor #n".
/// The first screen is the one with the menu bar. No permission is needed.
#[cfg(target_os = "macos")]
const SCREENS_SCRIPT: &str = "ObjC.import('AppKit'); var p = $.NSEvent.mouseLocation; var s = ObjC.unwrap($.NSScreen.screens); \
    var out = []; var active = -1; for (var i = 0; i < s.length; i++) { var f = s[i].frame; var k = s[i].backingScaleFactor; \
    out.push({ name: ObjC.unwrap(s[i].localizedName), id: ObjC.deepUnwrap(s[i].deviceDescription).NSScreenNumber, \
    w: Math.round(f.size.width * k), h: Math.round(f.size.height * k) }); \
    if (p.x >= f.origin.x && p.x < f.origin.x + f.size.width && p.y >= f.origin.y && p.y < f.origin.y + f.size.height) active = i; } \
    JSON.stringify({ screens: out, active: active })";

/// The displays, and the index of the one under the mouse pointer. None when AppKit can't be asked.
#[cfg(target_os = "macos")]
fn read_screens() -> Option<(Vec<Monitor>, Option<usize>)> {
    let out = Command::new("osascript").args(["-l", "JavaScript", "-e", SCREENS_SCRIPT]).stderr(Stdio::null()).output().ok()?;
    parse_screens(&String::from_utf8_lossy(&out.stdout))
}

/// The JSON that `SCREENS_SCRIPT` prints.
#[cfg(any(target_os = "macos", test))]
fn parse_screens(json: &str) -> Option<(Vec<Monitor>, Option<usize>)> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;
    let mut mons = Vec::new();
    for (i, d) in v["screens"].as_array()?.iter().enumerate() {
        mons.push(Monitor {
            id: d["id"].as_u64()? as u32,
            name: d["name"].as_str()?.to_string(),
            w: d["w"].as_u64()? as u32,
            h: d["h"].as_u64()? as u32,
            x: 0,
            y: 0,
            primary: i == 0,
        });
    }
    let active = v["active"].as_i64().and_then(|i| usize::try_from(i).ok()).filter(|i| *i < mons.len());
    Some((mons, active))
}

/// Connected displays. Empty on failure.
#[cfg(target_os = "macos")]
pub fn monitors() -> Vec<Monitor> {
    read_screens().map(|(mons, _)| mons).unwrap_or_default()
}

/// Connected displays from `xrandr --listmonitors` (X11). Empty on failure.
#[cfg(not(target_os = "macos"))]
pub fn monitors() -> Vec<Monitor> {
    let Ok(out) = Command::new("xrandr").arg("--listmonitors").stderr(Stdio::null()).output() else { return Vec::new() };
    let re = regex::Regex::new(r"^\s*\d+:\s*\+?(\*?)(\S+)\s+(\d+)/\d+x(\d+)/\d+([+-]\d+)([+-]\d+)").unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let c = re.captures(l)?;
            Some(Monitor {
                id: 0,
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

/// Setting value for "start on the display the mouse pointer is on".
pub const ACTIVE: &str = "@active";

/// The display under the mouse pointer. None when it can't be told.
#[cfg(target_os = "macos")]
fn active_monitor(mons: &[Monitor]) -> Option<Monitor> {
    let (screens, active) = read_screens()?;
    let id = screens.get(active?)?.id;
    mons.iter().find(|m| m.id == id).cloned()
}

#[cfg(not(target_os = "macos"))]
fn active_monitor(mons: &[Monitor]) -> Option<Monitor> {
    let out = Command::new("xdotool").args(["getmouselocation", "--shell"]).stderr(Stdio::null()).output().ok()?;
    let (x, y) = parse_mouse_location(&String::from_utf8_lossy(&out.stdout))?;
    monitor_at(mons, x, y).cloned()
}

/// `X=…` and `Y=…` lines from `xdotool getmouselocation --shell`.
#[cfg(any(not(target_os = "macos"), test))]
fn parse_mouse_location(text: &str) -> Option<(i32, i32)> {
    let find = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)?.trim().parse::<i32>().ok());
    Some((find("X=")?, find("Y=")?))
}

#[cfg(any(not(target_os = "macos"), test))]
fn monitor_at(mons: &[Monitor], x: i32, y: i32) -> Option<&Monitor> {
    mons.iter().find(|m| x >= m.x && y >= m.y && x < m.x + m.w as i32 && y < m.y + m.h as i32)
}

pub fn pick(mons: &[Monitor], pref: &str) -> Option<Monitor> {
    pick_with(mons, pref, active_monitor)
}

fn pick_with(mons: &[Monitor], pref: &str, active: impl Fn(&[Monitor]) -> Option<Monitor>) -> Option<Monitor> {
    let wanted = if pref == ACTIVE { active(mons) } else { mons.iter().find(|m| !pref.is_empty() && m.name == pref).cloned() };
    wanted.or_else(|| mons.iter().find(|m| m.primary).cloned()).or_else(|| mons.first().cloned())
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
    #[cfg(not(target_os = "macos"))]
    let name = target.map(|m| m.name.clone());
    #[cfg(target_os = "macos")]
    let id = target.map_or(0, |m| m.id);
    let listed = target.map(|m| (m.x, m.y, m.w, m.h));
    ui.window().with_winit_window(move |w: &winit::window::Window| {
        if !windowed && w.fullscreen().is_some() {
            return true;
        }
        // winit names macOS displays "Monitor #n", so there they are found by display id.
        #[cfg(target_os = "macos")]
        let handle = w.available_monitors().find(|m| id != 0 && MonitorHandleExtMacOS::native_id(m) == id);
        #[cfg(not(target_os = "macos"))]
        let handle = name.as_ref().and_then(|n| w.available_monitors().find(|m| m.name().as_deref() == Some(n.as_str())));
        // macOS can't list positions up front, so take them from the monitor itself; if the display
        // isn't found, just fullscreen on the current one.
        let geom = if cfg!(target_os = "macos") {
            handle.as_ref().map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
        } else {
            listed
        };
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_displays_and_the_pointer_display_from_appkit() {
        let json = r#"{"screens":[{"name":"Built-in Retina Display","id":1,"w":3024,"h":1964},
            {"name":"LEN G32qc-10","id":4,"w":2560,"h":1440}],"active":1}"#;
        let (m, active) = parse_screens(json).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].name.as_str(), m[0].id, m[0].w, m[0].h, m[0].primary), ("Built-in Retina Display", 1, 3024, 1964, true));
        assert_eq!((m[1].name.as_str(), m[1].id, m[1].primary), ("LEN G32qc-10", 4, false));
        assert_eq!(active, Some(1));
        assert_eq!(pick(&m, "LEN G32qc-10").unwrap().id, 4);
        assert_eq!(pick(&m, "").unwrap().id, 1, "the first screen has the menu bar");
    }

    #[test]
    fn appkit_output_without_a_pointer_display_or_with_bad_json() {
        let (_, active) = parse_screens(r#"{"screens":[{"name":"A","id":1,"w":1,"h":1}],"active":-1}"#).unwrap();
        assert_eq!(active, None);
        let (_, active) = parse_screens(r#"{"screens":[{"name":"A","id":1,"w":1,"h":1}],"active":7}"#).unwrap();
        assert_eq!(active, None, "an index past the list is ignored");
        assert!(parse_screens("").is_none());
        assert!(parse_screens(r#"{"screens":[{"name":"A"}]}"#).is_none());
    }

    fn mon(name: &str, x: i32, primary: bool) -> Monitor {
        Monitor { id: 0, name: name.into(), w: 1920, h: 1080, x, y: 0, primary }
    }

    #[test]
    fn active_display_follows_the_pointer_and_falls_back_to_primary() {
        let mons = [mon("A", 0, true), mon("B", 1920, false)];
        let on_b = |m: &[Monitor]| m.iter().find(|x| x.name == "B").cloned();
        assert_eq!(pick_with(&mons, ACTIVE, on_b).unwrap().name, "B");
        assert_eq!(pick_with(&mons, ACTIVE, |_| None).unwrap().name, "A", "pointer unknown: primary display");
        assert_eq!(pick_with(&mons, "B", |_| None).unwrap().name, "B", "a named display ignores the pointer");
        assert_eq!(pick_with(&mons, "", on_b).unwrap().name, "A", "empty means primary, not active");
    }

    #[test]
    fn pointer_position_picks_the_display_it_is_in() {
        let mons = [mon("A", 0, true), mon("B", 1920, false)];
        assert_eq!(parse_mouse_location("X=2000\nY=40\nSCREEN=0\nWINDOW=1\n"), Some((2000, 40)));
        assert_eq!(parse_mouse_location("nothing"), None);
        assert_eq!(monitor_at(&mons, 2000, 40).unwrap().name, "B");
        assert_eq!(monitor_at(&mons, 1919, 0).unwrap().name, "A");
        assert!(monitor_at(&mons, 5000, 0).is_none());
    }
}
