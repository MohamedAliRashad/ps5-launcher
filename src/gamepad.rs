//! Controller input straight from evdev (/dev/input/event*). No udev/SDL dependency.
//! Works for DualSense, DualShock 4, Xbox and most pads with the standard Linux mapping,
//! and keeps working while a game has focus (needed for the PS button toggle).

use std::collections::HashMap;
use std::os::fd::RawFd;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pad {
    Up,
    Down,
    Left,
    Right,
    Confirm,  // ✕ / A
    Back,     // ○ / B
    Square,   // □ / X
    Triangle, // △ / Y
    L1,
    R1,
    L2,
    R2,
    Options,
    Ps,
}

const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const BTN_SOUTH: u16 = 0x130;
const BTN_MODE: u16 = 0x13c;

fn map_button(code: u16) -> Option<Pad> {
    Some(match code {
        0x130 => Pad::Confirm,
        0x131 => Pad::Back,
        0x133 => Pad::Triangle,
        0x134 => Pad::Square,
        0x136 => Pad::L1,
        0x137 => Pad::R1,
        0x138 => Pad::L2,
        0x139 => Pad::R2,
        0x13b => Pad::Options,
        0x13c => Pad::Ps,
        0x220 => Pad::Up,
        0x221 => Pad::Down,
        0x222 => Pad::Left,
        0x223 => Pad::Right,
        _ => return None,
    })
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

fn abs_info(fd: RawFd, axis: u16) -> Option<AbsInfo> {
    // EVIOCGABS(axis) = _IOR('E', 0x40 + axis, struct input_absinfo)
    let req = (2u64 << 30) | ((std::mem::size_of::<AbsInfo>() as u64) << 16) | ((b'E' as u64) << 8) | (0x40 + axis as u64);
    let mut info = AbsInfo::default();
    let r = unsafe { libc::ioctl(fd, req as _, &mut info as *mut AbsInfo) };
    (r >= 0 && info.maximum > info.minimum).then_some(info)
}

struct Device {
    fd: RawFd,
    path: String,
    ranges: HashMap<u16, (i32, i32)>,
    axes: HashMap<u16, f32>,
    buttons: [bool; 4], // dpad buttons up/down/left/right
}

/// evdev nodes of gamepads (devices with a joystick handler and a south face button).
fn gamepad_paths() -> Vec<String> {
    let Ok(text) = std::fs::read_to_string("/proc/bus/input/devices") else { return Vec::new() };
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let (mut handlers, mut keys) = ("", "");
        for line in block.lines() {
            if let Some(h) = line.strip_prefix("H: Handlers=") {
                handlers = h;
            } else if let Some(k) = line.strip_prefix("B: KEY=") {
                keys = k;
            }
        }
        if keys.is_empty() || !handlers.split_whitespace().any(|h| h.starts_with("js")) {
            continue;
        }
        // KEY bitmap: space-separated 64-bit words, most significant first.
        let words: Vec<u64> = keys.split_whitespace().filter_map(|w| u64::from_str_radix(w, 16).ok()).collect();
        let bit = |n: u16| {
            let (w, b) = ((n / 64) as usize, n % 64);
            words.len() > w && words[words.len() - 1 - w] >> b & 1 == 1
        };
        if bit(BTN_SOUTH) || bit(BTN_MODE) {
            if let Some(ev) = handlers.split_whitespace().find(|h| h.starts_with("event")) {
                out.push(format!("/dev/input/{ev}"));
            }
        }
    }
    out
}

fn open_device(path: &str) -> Option<Device> {
    let c = std::ffi::CString::new(path).ok()?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    let mut ranges = HashMap::new();
    for axis in [0u16, 1, 16, 17] {
        if let Some(i) = abs_info(fd, axis) {
            ranges.insert(axis, (i.minimum, i.maximum));
        }
    }
    crate::log!("controller connected: {path}");
    Some(Device { fd, path: path.to_string(), ranges, axes: HashMap::new(), buttons: [false; 4] })
}

/// Spawns the input thread. `emit` receives presses (directions auto-repeat while held).
pub fn spawn(emit: impl Fn(Pad) + Send + 'static) {
    std::thread::Builder::new()
        .name("gamepad".into())
        .spawn(move || run(emit))
        .ok();
}

fn run(emit: impl Fn(Pad)) {
    let mut devices: Vec<Device> = Vec::new();
    let mut last_scan = Instant::now() - Duration::from_secs(60);
    let mut held: HashMap<Pad, Instant> = HashMap::new(); // direction -> next repeat time
    let mut buf = [0u8; 24 * 64];
    loop {
        if last_scan.elapsed() > Duration::from_secs(3) {
            last_scan = Instant::now();
            for p in gamepad_paths() {
                if !devices.iter().any(|d| d.path == p) {
                    if let Some(d) = open_device(&p) {
                        devices.push(d);
                    }
                }
            }
        }
        if devices.is_empty() {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        // Wait for input, or until the next key-repeat is due.
        let now = Instant::now();
        let timeout = held.values().map(|t| t.saturating_duration_since(now)).min().unwrap_or(Duration::from_millis(3000));
        let mut fds: Vec<libc::pollfd> = devices.iter().map(|d| libc::pollfd { fd: d.fd, events: libc::POLLIN, revents: 0 }).collect();
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, timeout.as_millis().min(3000) as i32) };

        let mut gone = Vec::new();
        for (i, pfd) in fds.iter().enumerate() {
            if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                gone.push(i);
                continue;
            }
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let dev = &mut devices[i];
            let n = unsafe { libc::read(dev.fd, buf.as_mut_ptr() as *mut _, buf.len()) };
            if n <= 0 {
                if n < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EAGAIN) {
                    gone.push(i);
                }
                continue;
            }
            for ev in buf[..n as usize].chunks_exact(24) {
                let typ = u16::from_ne_bytes([ev[16], ev[17]]);
                let code = u16::from_ne_bytes([ev[18], ev[19]]);
                let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                match typ {
                    EV_KEY => match map_button(code) {
                        Some(p @ (Pad::Up | Pad::Down | Pad::Left | Pad::Right)) => {
                            dev.buttons[p as usize] = value != 0;
                        }
                        Some(p) if value == 1 => emit(p),
                        _ => {}
                    },
                    EV_ABS if [0, 1, 16, 17].contains(&code) => {
                        let v = match dev.ranges.get(&code) {
                            Some(&(lo, hi)) if code < 16 => (value - lo) as f32 / (hi - lo) as f32 * 2.0 - 1.0,
                            _ => value.signum() as f32,
                        };
                        dev.axes.insert(code, v);
                    }
                    _ => {}
                }
            }
        }
        for i in gone.into_iter().rev() {
            crate::log!("controller disconnected: {}", devices[i].path);
            unsafe { libc::close(devices[i].fd) };
            devices.remove(i);
        }

        // Combine d-pad (hat or buttons) and left stick into directions with auto-repeat.
        let mut dirs = [false; 4];
        for d in &devices {
            let ax = |c: u16| d.axes.get(&c).copied().unwrap_or(0.0);
            dirs[0] |= d.buttons[0] || ax(17) < -0.5 || ax(1) < -0.55;
            dirs[1] |= d.buttons[1] || ax(17) > 0.5 || ax(1) > 0.55;
            dirs[2] |= d.buttons[2] || ax(16) < -0.5 || ax(0) < -0.55;
            dirs[3] |= d.buttons[3] || ax(16) > 0.5 || ax(0) > 0.55;
        }
        let now = Instant::now();
        for (i, p) in [Pad::Up, Pad::Down, Pad::Left, Pad::Right].into_iter().enumerate() {
            if !dirs[i] {
                held.remove(&p);
            } else if let Some(next) = held.get_mut(&p) {
                if now >= *next {
                    *next = now + Duration::from_millis(85);
                    emit(p);
                }
            } else {
                held.insert(p, now + Duration::from_millis(380));
                emit(p);
            }
        }
    }
}
