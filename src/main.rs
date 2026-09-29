//! PS5 Launcher: a fast, native PS5-style game launcher for Linux.

mod app;
mod audio;
mod boot;
mod catalog;
mod compat;
mod config;
mod display;
mod gamepad;
mod images;
mod kyty;
mod kyty_ui;
mod update;
mod library;
mod present;
mod psn;
mod sessions;
mod settings;
mod util;

slint::include_modules!();

const HELP: &str = "\
PS5 Launcher — a PS5-style game launcher for Linux

USAGE:
    ps5-launcher [OPTIONS]

OPTIONS:
    --windowed          Open in a normal window instead of fullscreen
    --monitor <NAME>    Display to use (e.g. DP-2); overrides Settings
    --sync              Refresh the game catalog on start
    --version           Print the version
    -h, --help          Show this help
";

fn main() {
    let mut windowed = false;
    let mut monitor: Option<String> = None;
    let mut force_sync = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--windowed" => windowed = true,
            "--monitor" => monitor = args.next(),
            "--sync" => force_sync = true,
            "--version" | "-V" => {
                println!("ps5-launcher {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-h" | "--help" => {
                print!("{HELP}");
                return;
            }
            other => {
                eprintln!("unknown option: {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    if force_sync {
        // Mark the cached catalog stale; the app syncs on start.
        let _ = std::fs::remove_file(catalog::CatalogFile::path());
    }

    let cfg = config::Config::load();
    let mons = display::monitors();
    let target = display::pick(&mons, monitor.as_deref().unwrap_or(&cfg.monitor));
    // Lay the UI out on a 1920×1080 canvas scaled to the chosen display.
    let scale = display::scale_for(target.as_ref()) * if windowed { 0.8 } else { 1.0 };
    if std::env::var_os("SLINT_SCALE_FACTOR").is_none() {
        std::env::set_var("SLINT_SCALE_FACTOR", format!("{scale:.4}"));
    }
    // Only the winit backend is compiled in; make sure Slint picks the OpenGL renderer.
    if std::env::var_os("SLINT_BACKEND").is_none() {
        std::env::set_var("SLINT_BACKEND", "winit-femtovg");
    }

    let ui = AppWindow::new().expect("could not create the window (is a graphical session running?)");
    app::run(ui, mons, scale, target, windowed);
}
