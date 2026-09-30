# PS5 Launcher

A fast, native PS5-style game launcher for Linux. It browses a RuTracker PS5 release catalog with the
official PlayStation artwork, and launches your installed games with the
[KytyPS5](https://github.com/KytyPS5/KytyPS5) emulator.

It's a single native binary written in Rust with a GPU-accelerated [Slint](https://slint.dev)
UI, and it doesn't use a browser or Electron. It starts in a fraction of a second and uses no
CPU when idle.

![PS5 Launcher: moving through the Home screen, a Game Hub and the Library](docs/demo/demo-loop.webp)

## Install

### Quick install (recommended)

Run this in a terminal:

```bash
curl -fsSL https://github.com/MohamedAliRashad/ps5-launcher/releases/latest/download/ps5-launcher-linux-x86_64.tar.gz | tar xz && ./ps5-launcher-linux-x86_64/install.sh
```

This downloads the latest release. It installs the launcher to `~/.local/bin` and adds
**PS5 Launcher** to your app menu. You don't need `sudo`, Rust or a compiler.

The prebuilt binary runs on 64-bit Linux with glibc 2.35 or newer. That covers Ubuntu 22.04+,
Debian 12+, Fedora 36+, Linux Mint 21+, Pop!_OS 22.04+, Arch and SteamOS 3. You can also
download the archive yourself from the
[Releases page](https://github.com/MohamedAliRashad/ps5-launcher/releases) and run
`./install.sh` inside it.

To remove it, run `./install.sh --uninstall`. To start it when you log in, install with
`./install.sh --autostart`.

**Updates are automatic.** The launcher checks GitHub for a new release when it starts and
every 6 hours. It downloads it in the background, checks it against GitHub's SHA-256
checksum, makes sure it runs, and swaps it in. The new version starts the next time you open
the launcher, or right away with **Settings → Restart now** (never while a game is running).
The previous binary is kept as `ps5-launcher.previous` next to it. To turn this off, disable
**Settings → Keep PS5 Launcher updated automatically**.

> Installed v1.2.0 or older? Run the install command above once more to get the auto-updater.

### Build from source

```bash
git clone https://github.com/MohamedAliRashad/ps5-launcher.git
cd ps5-launcher
./install.sh
```

When run in a source checkout, the script:

1. checks for Rust and offers to install it with [rustup](https://rustup.rs) (no `sudo`) if it's missing;
2. builds an optimized release binary;
3. installs it to `~/.local/bin/ps5-launcher` and adds **PS5 Launcher** to your app menu.

A source build installed this way updates itself like a release build. A binary run straight
from `target/release/` only tells you about new releases; update it with `git pull` and
`./install.sh`.

| Command | What it does |
|---|---|
| `./install.sh --autostart` | Also start the launcher when you log in (console-style) |
| `./install.sh --uninstall` | Remove the launcher (keeps your settings and cache) |
| `PREFIX=/usr/local sudo -E ./install.sh` | Install for all users |

### If the build fails

It's almost always a missing build package. Install the one for your distro and run
`./install.sh` again:

| Distro | Command |
|---|---|
| Ubuntu / Debian / Mint / Pop!_OS | `sudo apt install build-essential pkg-config libfontconfig1-dev libxkbcommon-dev` |
| Fedora | `sudo dnf install gcc pkgconf-pkg-config fontconfig-devel libxkbcommon-devel` |
| Arch / Manjaro / SteamOS | `sudo pacman -S --needed base-devel fontconfig libxkbcommon` |
| openSUSE | `sudo zypper install gcc pkg-config fontconfig-devel libxkbcommon-devel` |

The declared compiler floor is Rust 1.92, matching the resolved Slint requirement.
The full download implementation was built and tested with Rust 1.98.1; an exact
Rust 1.92 build has not been certified. Use a current stable toolchain.

### Optional helpers

The launcher works without these, but each one turns on a feature:

| Tool | Enables |
|---|---|
| `xdotool` | **Resume** / **Stop** that switch to and close the game window, plus the PS button toggle between game and launcher (X11) |
| `mpv` | Fullscreen trailers inside the launcher. Without it, trailers open in your browser |
| `yt-dlp` | YouTube trailers in `mpv` |
| `xrandr` | Picking the display and scaling the UI to fit it |
| System `libarchive.so.13` (`libarchive13` on Ubuntu/Debian) | Background game installation from supported archives; no development package needed |

On Ubuntu and Debian: `sudo apt install xdotool mpv yt-dlp x11-xserver-utils`

## Launch

- From your app menu, open **PS5 Launcher**.
- From a terminal, run `ps5-launcher`.

| Flag | Effect |
|---|---|
| `--windowed` | Open in a window instead of fullscreen |
| `--monitor DP-2` | Use a specific display (names come from `xrandr --listmonitors`) |
| `--sync` | Reload the local RuTracker JSON snapshot on start |
| `--catalog <PATH>` | Use an English RuTracker PS5 JSON snapshot instead of the bundled catalog |
| `--help`, `--version` | Show help or the version |

**First launch:** a welcome screen gets everything ready before you're let in. Your library's
covers drift behind it as they arrive, and a checklist shows each step as it happens:

1. loads the local RuTracker catalog (614 release topics in the bundled snapshot);
2. downloads the official artwork for catalog titles not already cached;
3. downloads the **latest KytyPS5 build**;
4. downloads every Library cover, and the backgrounds, logos and icons of every game on the Home
   screen;
5. preloads the Home screen and the first page of the Library.

On a typical connection this takes about a minute. After that, the Game Hub art for every
other game downloads in the background, newest first. It takes about 3–4 minutes, with a small
progress line at the bottom of the screen. Sony's image server takes about a second per image,
so after this every Game Hub opens instantly instead of loading. The image cache ends up at
about 500 MB in `~/.cache/ps5-launcher/`.

Then it asks you to press **✕ / Enter** to start. You can skip the wait; anything left keeps
downloading in the background.

**Later launches:** a short splash shows while the Home screen loads from the local cache,
which takes well under a second. Covers for newly added games download quietly in the
background.

**Add your games:** point **Settings → Game folders** at the folder that holds your games. A
game is any folder with a `sce_sys/param.json`.

## RuTracker catalog

The English snapshot is embedded in the binary, so catalog browsing works offline
without Chrome or a source checkout. All release topics are retained, but the
Library shows **one card per game**: matching title IDs, region aliases and normalized
names group editions/versions together. Sequels and remasters with distinct identities
remain separate. Use the **release selector in the Game Hub** to switch exact
versions, regions, peer counts and download magnets. Search matches every release
in a group without duplicating its card. Cards show download size, release count
and **seeders/leechers from the snapshot**, with its date in the Library header.
The Game Hub presents peers and snapshot date on one line, three key overview
facts, and expandable **Technical details** for title ID, languages, firmware,
release provenance and installation information.
Missing counts are unknown, not zero. These are **not live tracker/client counts**.

### Library layout and controls

The **Home / Library** navigation keeps installed games separate from the catalog.
Library cards reserve two title lines, show download size and a subtle grouped-release
count, and use compact **↑ seeders / ↓ leechers** captions (green/red; unknown counts
remain muted). Version/region details stay in the Game Hub. A single dated peer-snapshot
caption applies to the catalog; differing observation dates are shown in each Game Hub.

Use **Comfortable / Compact** to choose grid density. The choice is saved immediately;
smaller windows reduce the column count and maintain readable caption text. Density
changes retain the current scroll position's game rather than jumping back to the start.
Artwork caching, visible-row virtualization and look-ahead prefetching are unchanged.

**All / Installed / In-game on KytyPS5** filters are separate from the **GENRE** row,
and can be combined with genres and search. Counts reflect those combinations without
duplicating release variants. Genre-row arrows expose options beyond the viewport.
Click the sort control or press Enter on it to open all seven sort choices; arrows select,
Enter applies, and Escape cancels. Page Up/Down on the closed sort control cycles directly.
Keyboard/controller navigation follows search/sort/density → status filters → genres → grid.

The welcome screen and app assets use an original white/electric-blue **P5 chassis emblem**,
not the former generic play-button icon. Source installation updates the matching desktop icon.

**Settings → Reload RuTracker catalog** re-imports local JSON; it does not scrape
the website or refresh peer counts over the network. Source precedence is:

1. `--catalog <PATH>` or `PS5_LAUNCHER_CATALOG_PATH`;
2. `$XDG_DATA_HOME/ps5-launcher/rutracker/ps5-topics.json` (normally under `~/.local/share`);
3. the generated snapshot in a source checkout, if present;
4. the embedded English snapshot in [assets/rutracker/ps5-topics.json](assets/rutracker/ps5-topics.json).

The source-specific cache is independent of the old SuperPSX cache. Invalid,
partial or untranslated imports do not overwrite a known-good catalog. Posting
dates were not collected, so **Newest topics** sorts by topic ID; collection and
game-release dates are not misrepresented as dates added to the site.

To collect fresh metadata, use the separate [browser collector](scripts/rutracker-list/README.md),
complete any normal verification yourself, then translate the JSON to English.
The collector is not part of launcher startup. Firmware/test claims remain
uploader-provided console notes, not KytyPS5 compatibility guarantees.

### Dynamic metadata: recommended next layer

JSON remains a useful offline baseline and cache format; it need not be the only
source of discovery. A future explicit **Refresh peer counts** action should fetch
website statistics for the selected release, store a separate topic-keyed observation
with its check time, and keep previous data when refresh fails. New-release discovery
is a separate catalog update, not a side effect of refreshing one game's counts.

That live website action is **not implemented yet**: anonymous RuTracker access can
be blocked by verification/login, and the current collector establishes peer counts
from forum listings, not a reliable per-topic endpoint. Do not interpret local reload
as an online refresh. Refreshing website metadata must never resolve a magnet or
start a torrent just to count peers. Website snapshots and this client's connected
peers should remain separately labeled; neither is a guaranteed live swarm total.

## Background magnet downloads

Use this feature only for content you are authorized to obtain and share.

1. Open a release's **Game Hub → Download** (also available in Options).
2. Choose the destination and **Look up metadata**. This contacts trackers,
  DHT and peers and reveals your IP address, but downloads no payload files.
3. Review the file count, names and total size, then choose **Start download**.
  All files are downloaded; the first 100 names are shown for large torrents.
4. Keep browsing or minimize the window. Open **Downloads** with the top-bar
  disk icon, **Ctrl+D**, or **Settings → Manage downloads**. It shows a progress
  bar, percentage, verified bytes, MiB/s, ETA and this client's connected peers.

**Pause / Resume** preserve and recheck partial pieces. **Cancel** stops the job
and keeps files. **Remove** clears inactive history and cached metadata, never
the payload. **Open folder** opens the saved destination.

**Completed downloads seed by default.** While the launcher remains open, their
original files are shared with peers. Downloads shows **Complete · seeding**, upload
speed and connected peers; **Install / Play** remains available. Use **Stop seeding**
for one transfer, or disable **Settings → Seed completed downloads** to stop all
current seeds and prevent future post-completion seeding. This preference is saved.
Turning it back on affects future completions only; it never restarts stopped or
restored jobs, and never downloads/rechecks old payloads automatically.

The default destination is `~/Downloads/PS5`, editable in Settings or the setup
dialog. Each torrent gets a private `torrent-<infohash>` subfolder, preventing
unrelated releases from overwriting one another. Existing unowned folders,
unsafe paths, symlinks and hardlinks are rejected. Free space is checked before
starting, but filesystem quotas and concurrent filesystem changes can still fail
a transfer. Payload pieces are checked against the torrent's hashes; this does
not establish authenticity or safety of their contents.

**The launcher must stay open.** Graceful quit/restart saves active transfers as
paused; completed seeds remain Complete but stop sharing. Reopening never starts the torrent engine or resumes payload traffic
automatically; choose Resume/Retry explicitly. Transfer history and metadata live
under the XDG configuration directory in the launcher's transfers subfolder.
Download completion does not extract, install, launch a game or rescan the library.
Choose the separate **Install** action when ready.

Transfers can upload pieces while downloading and, by default, after completion,
with a session-wide **128 KiB/s** upload cap. Disabling completed-download seeding
does not disable uploads during downloads. Magnet discovery happens before a
private flag is known; once metadata is known, the engine honors that flag for
payload discovery. A private tracker may require authorized tracker credentials.
Catalog seeders/leechers remain dated website snapshots, not live swarm totals.

The native backend is pinned to **librqbit 9.0.1** with rustls, on a dedicated
Tokio worker. See the [backend notes](docs/rust-torrent-options.md) for APIs,
network behavior and validation limits.

## Download → Install → Play

Once a download completes, choose **Install** in Downloads or its Game Hub.
Confirm the installation destination (default `~/Games/PS5`, editable in Settings
and the confirmation dialog). Extraction/copying runs in a separate background
worker while browsing or minimizing the launcher. Downloads shows its current
phase, progress, errors and **Cancel install**; closing that panel does not cancel it.

The installer checks available space, stages output in a private hidden directory,
and requires exactly one complete game with valid title metadata and a nonempty
`eboot.bin`. Its title ID must match the selected release when known. Only after
validation is the game atomically published without replacing an existing folder,
the destination added to Game folders, and the library refreshed. The action then
becomes **Play**. Original archives and downloaded folders are always retained;
installation itself never executes their contents.

Loose game folders are copied. RAR, ZIP, 7z and TAR-family decoding uses dynamically
loaded system libarchive; no archive helper process is executed. Matching multipart
RAR and split 7z volumes are ordered and checked for gaps, but decoder support varies
with the installed libarchive version. Password-protected archives, PKG decryption,
update/DLC merging, and archives containing multiple complete games are unsupported.
An unsupported or malformed archive fails with an explanation instead of publishing
a partial library entry. RAR4 stored, ZIP, 7z/split 7z and TAR generated fixtures were
tested on libarchive 3.7.2; compressed/solid/multipart RAR variants are not certified.

**Validation is not an authenticity, malware or universal checksum guarantee.**
In particular, libarchive 3.7.2 accepted corrupted stored-RAR4 payload bytes despite
their CRC in a probe; do not rely on it for full RAR integrity verification.

Keep the launcher open until installation finishes. Graceful quit/restart cancels
an active installation and cleans its owned staging directory; retry is explicit.
Interrupted records restore as failed and never auto-extract. A forced crash can
leave hidden staging files, which are not installed games or automatically reused.
Installation records live in the launcher's XDG configuration installs subfolder.

### Catalog validation

`cargo test --offline` runs the launcher unit tests; this is a binary crate,
so do not use `--lib`. After a release build, run
`node scripts/test-catalog.mjs` for native Linux smoke tests. The test requires
Node.js, `unshare`, `xvfb-run`, `xdotool` and `ffmpeg`, plus permission to create
unprivileged user/network namespaces. It uses temporary XDG profiles with
networking disabled and both auto-updaters off. There is no network-enabled
fallback if isolation is unavailable.

The smoke test checks complete metadata/magnet preservation, byte-for-byte
retention of a good cache after invalid/missing imports, and zero versus unknown
peer counts. It also opens the bundled fallback and writes Library/Game Hub
screenshots for visual inspection, leaving the user's settings and cache untouched.

`node scripts/test-library.mjs` exercises sort selection/cancellation, combined filters,
search, actual responsive column counts, and persisted density at 1349×768 and 960×640.
It uses authored poster art, an inert generated library fixture, private PID/network
namespaces and temporary profiles; no real games, magnets or user settings are accessed.
Add `--full-hd` for 1920×1080 evidence. Screenshots and logs are retained under a temporary
artifact directory. `node scripts/test-downloads.mjs` additionally checks the explicit
Download → Install → Play workflow with a generated archive without executing its payload.

`cargo test --offline` also runs generated, tracker/DHT-disabled loopback transfers:
metadata-only preparation, intermediate progress, pause/resume with corrupt-piece
repair, paused recovery, shutdown and cancellation/removal that retain partial
files. No catalog/game magnet is used. `node scripts/test-downloads.mjs` checks
the native progress/metadata/consent UI, text editing and keep-files removal with
fabricated local snapshots in a network-disabled namespace. In addition to the
catalog test prerequisites, it requires Tesseract and ImageMagick. Screenshots
are retained for visual inspection; these UI fixtures do not perform transfers.

## KytyPS5 updates

The launcher installs and updates [KytyPS5](https://github.com/KytyPS5/KytyPS5) for you, using
its official Linux builds from GitHub Releases.

- **Automatic:** it checks for a new build at startup and every 6 hours, and installs it in the
  background. A small progress line shows at the bottom, and a notification appears when it's
  done.
- **Never mid-game:** if a game is running, the update waits until you close it.
- **Verified:** every download is checked against GitHub's SHA-256 checksum. The new build must
  start before the launcher switches to it.
- **Your saves are safe:** Kyty's saves, shader caches and patches (`_SaveData`,
  `_PipelineCache`, …) live in one shared folder that every version uses. Updates never touch them.
- **Rollback:** the previous build is kept. **Settings → Roll back to previous KytyPS5** switches
  back to it, and that build won't be reinstalled automatically.
- **Using your own build:** if you set **Settings → KytyPS5 executable** to a build you compiled,
  the launcher tells you when a newer official build exists but leaves yours alone.
  **Switch to official KytyPS5 builds** moves you to auto-updates and *copies* your saves across;
  your own build folder isn't changed.

To turn this off, disable **Settings → Keep KytyPS5 updated automatically**. Managed builds live
in `~/.local/share/ps5-launcher/kyty/`.

## Controls

| Action | Controller | Keyboard | Mouse |
|---|---|---|---|
| Move | D-pad / left stick | Arrow keys | Scroll wheel over the tiles |
| Select / Play | ✕ | Enter | Click |
| Back | ○ | Esc / Backspace | Click outside a panel |
| Trailer | □ | T | |
| Search | △ | / (or just start typing in the Library) | Click the search box |
| Options menu | Options | O | |
| Switch Games ⇄ Library | L1 / R1 | Tab | Click the tab |
| Page through the Library | L2 / R2 | Page Up / Page Down, Home / End | |
| Switch between game and launcher | PS button | | |
| Quit | | Ctrl+Q | |
| Downloads | Settings → Manage downloads | Ctrl+D | Top-bar disk icon |

The launcher reads controllers directly, so the PS button works even while a game is
fullscreen. This needs your user to have access to `/dev/input`. Most distros give the logged-in
user that access; if yours doesn't, add yourself to the `input` group.

## Features

- **PS5 Home screen:** your installed, ready-to-play games; the whole catalog is the Library
  tab. The selected tile grows and shows its name, the game's hub art fills the screen, and its
  official title logo is shown. Nothing on screen repeats what's already obvious.
- **Official artwork and details:** when available, looked up by title ID in Sony's
  public PlayStation catalog. That includes tile icons, clean covers, backgrounds, logos,
  screenshots, trailers, star ratings, age ratings and publishers.
- **RAWG (optional):** add a RAWG API key in Settings to fill in the rest by name. The key is
  checked before it's saved and is never displayed again. If you don't like a game's artwork,
  **Options → Use RAWG artwork** switches that game to RAWG's background, screenshots and
  description (and back again).
- **Library:** the whole release catalog with instant search, genre filters, and sorting by topic ID, name,
  release, rating or size.
- **Game Hub:** a details page for each game, with a fact grid, a screenshot viewer and an
  About section.
- **Background downloads:** opt-in magnet metadata review, progress/speed/ETA,
  pause/resume and keep-files cancellation; continues while browsing/minimized.
- **Steam-style sessions:**
  - a running game shows a **Playing** badge, a live timer, and **Resume** / **Stop**;
  - **Stop** closes the game cleanly, like Alt+F4, so Kyty keeps its shader cache;
  - it tracks playtime and "last played";
  - games you start from Kyty's own launcher or a terminal are detected too;
  - a crash shows the end of the emulator log.
- **Options menu:** open the game folder, view the emulator log, open the web or Store page,
  or watch the trailer.
- **Multi-monitor:** the launcher opens on one display and never spans two. Choose which one in
  Settings.
- **KytyPS5 compatibility tags:** every game shows how far it gets in KytyPS5, from the
  community [compatibility list](https://kytyps5.github.io/) (the same data KytyPS5's own
  launcher uses, refreshed every 6 hours):

  | Tag | Meaning |
  |---|---|
  | 🟢 **In-game** | Reaches gameplay |
  | 🟡 **Menus** | Reaches the main menu, not gameplay |
  | 🟠 **Boots** | Shows the intro logos, then stops |
  | 🔴 **Doesn't boot** | Doesn't start |
  | ⚪ **Untested** | No reports yet |

  Tags appear on Library covers, on the Home screen and in the Game Hub (with the number of
  reports, when it was last tested and, if different, the result on Linux). In the Library,
  **In-game on KytyPS5** filters to games that reach gameplay, and the **KytyPS5 compatibility**
  sort puts the best-supported games first.
- **KytyPS5 auto-updates:** installs the official build on first launch, then keeps it current,
  with rollback.
- **Launcher auto-updates:** new PS5 Launcher releases install themselves in the background and
  start on the next launch (or with **Restart now**).

| Game Hub | Settings |
|---|---|
| ![Game Hub](docs/screenshots/game-hub.jpg) | ![Settings](docs/screenshots/settings.jpg) |

## Where things are stored

| Path | Contents |
|---|---|
| `~/.config/ps5-launcher/config.json` | Settings |
| `~/.config/ps5-launcher/playtime.json` | Playtime per game |
| `~/.cache/ps5-launcher/` | Catalog, artwork, decoded thumbnails, emulator logs (safe to delete) |
| `~/.cache/ps5-launcher/catalog-rutracker.json` | Imported RuTracker release metadata and snapshot peer counts |
| `~/.local/share/ps5-launcher/kyty/` | Managed KytyPS5 builds and their shared saves and caches (`data/`) |

## Performance

- **Rendering:** the UI is drawn by the GPU through OpenGL. When nothing on screen moves, the
  launcher uses no CPU or GPU.
- **Images:**
  - they're decoded and resized on background threads;
  - resized copies are cached as QOI files, which load about 10× faster than PNG, so a warm
    start never decodes a JPEG or touches the network;
  - memory for images is capped at 200 MB, and the least recently shown are dropped first.
- **Library grid:** only the rows on screen exist as UI elements, so browsing 740 games is as
  smooth as browsing 10.
- **Moving along the tile row:** the neighbouring games' backgrounds and logos load ahead of
  time, so they're ready when you get there.

## Troubleshooting

- **The window is on the wrong screen:** change **Settings → Display**, or start once with
  `--monitor NAME`.
- **Resume or Stop does nothing:** install `xdotool`. On Wayland, Stop falls back to asking
  the emulator to quit.
- **A game exits right away:** open **Options → View emulator log**. Most boot problems come
  from Kyty itself; see its [compatibility list](https://kytyps5.github.io/).
- **The UI is too big or too small:** it scales itself to the window, on any screen size or
  shape. To make everything larger or smaller, set a multiplier, for example
  `PS5_LAUNCHER_SCALE=1.15 ps5-launcher`.
- **Animations stutter:** run `SLINT_DEBUG_PERFORMANCE=overlay ps5-launcher` to show the frame
  rate. Heavy GPU work in the background (for example a machine-learning job) slows the
  launcher's drawing too.
- **Images feel slow:** run `PS5_LAUNCHER_DEBUG=1 ps5-launcher` from a terminal. Every image
  load is logged with where it came from (disk or network) and how long each step took.
- **Reset everything:** `rm -rf ~/.config/ps5-launcher ~/.cache/ps5-launcher`

## Building by hand

```bash
cargo build --release            # binary: target/release/ps5-launcher
cargo test --release             # unit tests
```

To check every screen at 8 screen sizes (720p to 4K, 16:10 and ultrawide) under a real window
manager (needs Xvfb, xfwm4, xdotool and ImageMagick):

```bash
scripts/ui_matrix.py                    # writes dist/ui-matrix/sheet-<size>.png
```

To find cut-off or clipped elements automatically, run the UI audit. It walks the selection
through every screen at 5 screen sizes, with artwork turned off so only interface elements are
on screen. Any selected button, chip or menu row whose edge is flat where the design has a
rounded end is reported, with a cropped image:

```bash
scripts/ui_audit.py                     # writes dist/ui-audit/report.txt; exit code 1 on problems
```

To regenerate the README demo after UI changes (needs Xvfb, xdotool, ffmpeg and ImageMagick):

```bash
scripts/record_demo.py --game /path/to/an/installed/game   # writes dist/demo/
```

It runs the real launcher on a hidden virtual display with a throwaway home folder, drives it
with key presses and captions each step, so nothing personal appears in the recording.

## Disclaimer

PS5 Launcher is an unofficial fan project. It isn't affiliated with or endorsed by Sony
Interactive Entertainment. "PlayStation" and "PS5" are trademarks of Sony Interactive
Entertainment. Game artwork and metadata belong to their owners and are loaded from public
services at runtime. This project doesn't host, distribute or download games. Use only games
you own.
