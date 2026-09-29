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

/// UI is designed for 1920×1080 logical pixels; scale it to fill the monitor.
pub fn scale_for(m: Option<&Monitor>) -> f32 {
    match m {
        Some(m) => (m.w as f32 / 1920.0).min(m.h as f32 / 1080.0).max(0.5),
        None => 1.0,
    }
}

/// Put the window on one monitor (never spanning two) and make it borderless fullscreen.
pub fn place_window(ui: &crate::AppWindow, target: Option<&Monitor>, windowed: bool) {
    let name = target.map(|m| m.name.clone());
    let geom = target.map(|m| (m.x, m.y, m.w, m.h));
    ui.window().with_winit_window(move |w: &winit::window::Window| {
        let handle = name.as_ref().and_then(|n| w.available_monitors().find(|m| m.name().as_deref() == Some(n.as_str())));
        if windowed {
            if let Some((x, y, mw, mh)) = geom {
                let (ww, wh) = (1600.min(mw - 80), 900.min(mh - 80));
                let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(ww, wh));
                w.set_outer_position(winit::dpi::PhysicalPosition::new(x + (mw - ww) as i32 / 2, y + (mh - wh) as i32 / 2));
            }
        } else {
            if let Some((x, y, _, _)) = geom {
                w.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            }
            w.set_fullscreen(Some(winit::window::Fullscreen::Borderless(handle.or_else(|| w.current_monitor()))));
        }
        w.focus_window();
    });
}

pub fn window_has_focus(ui: &crate::AppWindow) -> bool {
    ui.window().with_winit_window(|w: &winit::window::Window| w.has_focus()).unwrap_or(true)
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

/// Play a video fullscreen with mpv (YouTube links via yt-dlp), or VLC/ffplay for direct files.
pub fn play_video(url: &str, monitor: &str) -> Option<std::process::Child> {
    let youtube = url.contains("youtube.com") || url.contains("youtu.be");
    let mut cmd = if which("mpv") {
        let mut c = Command::new("mpv");
        c.args(["--fs", "--really-quiet", "--force-window=immediate", "--title=Trailer", "--ytdl-format=bestvideo[height<=?1080]+bestaudio/best"]);
        if !monitor.is_empty() {
            c.arg(format!("--fs-screen-name={monitor}"));
        }
        c.arg(url);
        c
    } else if !youtube && which("vlc") {
        let mut c = Command::new("vlc");
        c.args(["--fullscreen", "--play-and-exit", "--no-video-title-show", url]);
        c
    } else if !youtube && which("ffplay") {
        let mut c = Command::new("ffplay");
        c.args(["-fs", "-autoexit", "-loglevel", "quiet", url]);
        c
    } else {
        return None;
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()
}
