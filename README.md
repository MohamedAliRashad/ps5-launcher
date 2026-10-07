# PS5 Launcher

A PS5-style home screen for Linux and macOS. Browse PS4 and PS5 games with their official artwork,
download and install them, and play them with the [shadPS4](https://github.com/shadps4-emu/shadPS4) (PS4) emulators and [KytyPS5](https://github.com/KytyPS5/KytyPS5)
(PS5), using a controller,
keyboard or mouse.

![PS5 Launcher: the Home screen, a Game Hub, then the Library filtered to PS4 games and a PS4 game's Hub](docs/demo/demo-loop.webp)

- **Your games on a PS5-style Home screen**, with official backgrounds, logos and trailers that
  play right in the launcher.
- **A Library of hundreds of PS5 and PS4 games** with search, genre filters and sorting.
- **See what runs:** every game shows how far it gets in its emulator.
- **Download, install and play** without leaving the launcher. It installs games from folders,
  archives (RAR, ZIP, 7z, TAR), exFAT disk images and PS5 debug `.pkg` files.
- **Keeps itself and both emulators up to date**, without touching your saves, or runs your own
  emulator builds.
- **A status bar that tells you what matters:** the clock, how many controllers are connected, and
  how much disk your games and downloads use.
- **Runs as a desktop app, or as the whole screen** of a PC, like a game console.

## Install

Pick the one that matches your computer.

| You have | Go to |
|---|---|
| Linux with a desktop (KDE, GNOME and others) | [Linux](#linux) |
| A Mac | [macOS](#macos) |
| A PC you want to use like a console | [As a console](#as-a-console) |

### Linux

You need 64-bit Linux: Ubuntu 22.04+, Debian 12+, Fedora 36+, Mint 21+, Arch or SteamOS 3.
Choose one way to install.

**A package** (it adds PS5 Launcher to the app menu and installs the libraries it needs).
Download the file for your distribution from the
[Releases page](https://github.com/MohamedAliRashad/ps5-launcher/releases/latest), then:

```bash
sudo apt install ./ps5-launcher_*_amd64.deb           # Ubuntu, Debian, Mint
sudo dnf install ./ps5-launcher-*.x86_64.rpm          # Fedora
sudo pacman -U ps5-launcher-*-x86_64.pkg.tar.zst      # Arch
```

Your package manager updates it.

**An AppImage** (one file, nothing to install). Download `ps5-launcher-linux-x86_64.AppImage`, then:

```bash
chmod +x ps5-launcher-linux-x86_64.AppImage && ./ps5-launcher-linux-x86_64.AppImage
```

Most systems need FUSE 2 for AppImages (`sudo apt install libfuse2`). Without it, start it with
`APPIMAGE_EXTRACT_AND_RUN=1 ./ps5-launcher-linux-x86_64.AppImage`. The AppImage updates itself by
replacing its own file, so keep it in a folder you can write to. It needs the helpers (see **Helpers**).

**One command, for your user only:**

```bash
curl -fsSL https://github.com/MohamedAliRashad/ps5-launcher/releases/latest/download/ps5-launcher-linux-x86_64.tar.gz | tar xz && ./ps5-launcher-linux-x86_64/install.sh
```

This also adds the menu entry. It updates itself. To remove it, run
`./ps5-launcher-linux-x86_64/install.sh --uninstall`.

**Helpers.** A package installs them for you. For the AppImage and the one-command install, add
them for trailers, switching between a game and the launcher, and installing from archives:

```bash
sudo apt install xdotool mpv libarchive13      # Ubuntu, Debian, Mint
```

### macOS

macOS 11 or later, Apple Silicon or Intel. KytyPS5's macOS build is x86-64 (with its own copy of
MoltenVK for Vulkan), so on Apple Silicon it runs through Rosetta 2.

1. Install the prerequisites once (Rosetta is for Apple Silicon only):

   ```bash
   softwareupdate --install-rosetta --agree-to-license
   brew install libarchive mpv   # mpv provides libmpv, which plays trailers inside the launcher
   ```

2. Download `ps5-launcher-macos-universal.zip` from the Releases page and unzip it.
3. Move **PS5 Launcher** to Applications.
4. Open it once. If macOS blocks it, see **"Not Opened: Apple could not verify…"**.
5. Continue with [Getting started](#getting-started). The launcher downloads KytyPS5 for you.

**"Not Opened: Apple could not verify…"** The app isn't notarized (that needs a paid Apple
Developer account), so macOS blocks the first launch of anything downloaded through a browser. It
doesn't mean the file is damaged. Do one of:

- Right-click **PS5 Launcher**, choose **Open**, then **Open** again in the dialog.
- Open **System Settings → Privacy & Security**, scroll down and click **Open Anyway** next to
  the blocked "PS5 Launcher" message (it appears after the first blocked attempt).
- Or in a terminal, remove the download flag:

  ```bash
  xattr -dr com.apple.quarantine "/Applications/PS5 Launcher.app"
  ```

Run the app, not the bare `ps5-launcher` file inside it: the bare file has no icon and some window
features work worse.

What's different on macOS:

- **Many games won't start yet, and on Apple Silicon possibly none.** KytyPS5 needs Vulkan
  features that MoltenVK (the Vulkan-on-Metal layer) doesn't provide on Apple GPUs, and it exits
  at startup with "Could not find suitable device … image view minLod / shaderBufferInt64Atomics /
  shaderCullDistance is not supported". The launcher shows that reason when it happens. This is a
  limit of the emulator and MoltenVK, not of the launcher. See the compatibility tags on the game
  cards and [kytyps5.github.io](https://kytyps5.github.io/).
- **Switching between a game and the launcher** (the PS button, Resume) and bringing a new game
  to the front need *Privacy & Security → Accessibility* permission for PS5 Launcher. Without it
  the launcher still starts and stops games, but can't raise or close their windows. The app is
  signed ad hoc, so macOS may ask again after an update.
- **PS4 games** are not available: shadPS4 and its package extractor are Linux-only.
- **Updates** replace the whole app, so it must be somewhere you can write to (such as
  `/Applications` as an admin user). Otherwise download the new zip.

### As a console

**Without a desktop.** The Linux packages add a login session called **PS5 Launcher**. Install
[gamescope](https://github.com/ValveSoftware/gamescope) (`sudo dnf install gamescope`,
`sudo pacman -S gamescope`; Ubuntu 24.04 does not ship it), log out, and choose **PS5 Launcher** at
the login screen. The launcher then fills the screen and is the only thing running. On a PC without
a desktop, run `ps5-launcher-session` from a text console. In this session **Settings** offers
**Restart** and **Power off** instead of Quit. To log in to it by itself, see
[packaging/linux](packaging/linux/README.md).

**A whole operating system.** **PS5 Launcher OS** makes a PC start straight into the launcher. It's
[Bazzite](https://bazzite.gg), a gaming Linux, with the launcher built in. The launcher, its
emulators and the system keep themselves up to date.

You need a USB stick (2 GB or more), an internet connection (cable or Wi-Fi), and a PC that can run
the games: a 4-core CPU from 2013 or later (Intel Haswell or AMD Excavator and newer), 8 GB RAM
(16 GB for PS5 games), a graphics card with Vulkan 1.3 (AMD, NVIDIA or Intel) and a 64 GB disk.
**Installing erases the disk you pick.**

1. Download [ps5-launcher-os-x86_64.iso](https://github.com/MohamedAliRashad/ps5-launcher/releases/latest/download/ps5-launcher-os-x86_64.iso)
   and write it to the USB stick with [balenaEtcher](https://etcher.balena.io) or
   [Fedora Media Writer](https://fedoraproject.org/workstation/download).
2. Start the PC from the USB stick (press F12, F11, F8 or Esc while it starts) and choose
   **Install PS5 Launcher OS**.
3. In the installer, choose your language, then on the summary screen:
   - **Network & Host Name:** connect to Wi-Fi if you have no cable.
   - **Installation Destination:** pick the disk and press **Done**. Leave **Encrypt my data**
     off, or the PC asks for a passphrase every time it starts.
   - **Time & Date:** pick your city (the launcher shows the time).
   - **User Creation:** your name and a password.

   Then **Begin Installation**. It downloads the system (about 7 GB), so it takes a while. The
   installer says "Fedora": Bazzite is built on Fedora and uses its installer.
4. Press **Reboot System**, remove the USB stick and stay at the PC. If Secure Boot is on, a blue
   **Perform MOK management** screen appears once and waits about 5 minutes. Use the arrow keys
   and Enter: **Enroll MOK**, **Continue**, then **Yes** (it starts on No), type `universalblue`,
   Enter, then **Reboot**. Missed it, and now the PC stops with "bad shim signature"? Start from
   the USB stick again, choose **Troubleshooting → Enroll the Secure Boot key again**, and the
   blue screen comes back.

The PC now starts straight into PS5 Launcher, and it picks the NVIDIA version of the system by
itself when it finds an NVIDIA graphics card. To get to the desktop, quit the launcher
(**Settings → Quit**); it opens again at the next start or from the app menu.

## Getting started

1. Open **PS5 Launcher** from your app menu, or run `ps5-launcher` in a terminal. The first launch
   downloads artwork and KytyPS5, which takes about a minute.
2. Already have games? Add their folder in **Settings → Game folders**.
3. To get a game, open it in the **Library**, choose **Download**, then **Install**, then
   **Play**. Keep the launcher open while it downloads.

> Only download games you own. Downloads use BitTorrent: other people can see your IP address,
> and finished downloads are shared while the launcher is open (turn this off in
> **Settings → Keep sharing finished downloads**).

A PS5 game from a release that holds several files installs by itself: the launcher picks the
archive or package that holds the game and ignores extras such as a firmware backport. An install
needs free space for the whole unpacked game, which can be several times the size of a package.

## Controls

| Action | Controller | Keyboard |
|---|---|---|
| Move / Select / Back | D-pad, ✕, ○ | Arrows, Enter, Esc |
| Options for a game | Options | O |
| Search | △ | / |
| Switch Home ⇄ Library | L1 / R1 | Tab |
| Switch between game and launcher | PS button | |
| Downloads | | Ctrl+D |

**Games play best with a controller.** DualSense, DualShock 4, Xbox and most other controllers
work in games over USB or Bluetooth with no setup; connect yours before starting a game. Without
one, games use the keyboard: **J I K L** for ✕ △ □ ○, **W A S D** to move, **arrow keys** for the
D-pad and **Enter** for Options (PS4 games use **N C V B** for ✕ △ □ ○). See **Settings →
Keyboard controls in games** for every key.

## Will my game run?

Each game shows a tag from its emulator's community list
([shadPS4](https://github.com/shadps4-compatibility/shadps4-game-compatibility) for PS4, [KytyPS5](https://kytyps5.github.io/) for PS5):
🟢 **In-game**, 🟡 **Menus**, 🟠 **Boots** (intro logos only), 🔴 **Doesn't boot**, or ⚪ **Untested**.

Linux results come first. A result from Windows is marked **Win**, because games can behave
differently on Linux. In the Library, **In-game** lists every game that reaches gameplay, and
**In-game on Linux** only the ones confirmed on Linux.

**Help others:** when you close a game, the launcher asks how far it got, and your answer becomes
its tag on your PC. Every so often, choose **Settings → Share your game ratings** to send your
results to the community lists (needs a free GitHub account).

## Games that come as `.pkg` files

- **PS4 packages** install through a small extractor the launcher fetches the first time (Linux
  5.13 or newer). It unpacks the game, its update and its DLC for shadPS4.
- **PS5 debug packages** (the kind that start with `FIH`) install like any other release. The
  launcher unpacks them itself and needs no key. Retail PS5 packages need a console key and are
  refused.

## Settings worth knowing

- **Display:** choose a display, or **Active display** to start on the one the mouse pointer is
  on (macOS, and Linux with `xdotool`). For a window instead of fullscreen, run
  `ps5-launcher --windowed`.
- **Game output resolution** (PS5 games, under **Show advanced settings**): the screen resolution
  the game is told it runs on, and the size it renders at.
  - **Game default:** what a PS5 reports for that game (4K or 1080p). The most accurate.
  - **1080p (Full HD):** the fastest. Games that pick 4K by default run much faster.
  - **4K (Ultra HD):** the sharpest and the slowest.

  The **Resolution** setting is the window size only. The game's picture is scaled to
  fit the window.
- **Your own emulator builds:** under **Settings → Show advanced settings**, set **KytyPS5 location**
  or **shadPS4 location**. The launcher then runs your build and does not install or update that
  emulator. Clear the field to go back to the one the launcher manages.
- **Updates:** **Update automatically** covers the launcher, shadPS4 and KytyPS5.
  **Go back to the previous shadPS4 / KytyPS5** undoes an update that broke a game.

## Help

- **A game closes right away:** check its tag, then **Options → View emulator log**.
- **Resume or Stop doesn't work:** install `xdotool`.
- **Wrong screen:** change **Settings → Display**.
- **Too big or too small:** run `PS5_LAUNCHER_SCALE=1.15 ps5-launcher` (bigger number, bigger UI).
- **A new KytyPS5 broke a game:** **Settings → Go back to the previous KytyPS5**.

Still stuck? [Open an issue](https://github.com/MohamedAliRashad/ps5-launcher/issues).
Developers: see the [developer guide](docs/DEVELOPMENT.md) and how releases are built in
[packaging](packaging/README.md).

---

PS5 Launcher is an unofficial fan project, not affiliated with or endorsed by Sony Interactive
Entertainment. "PlayStation" and "PS5" are trademarks of Sony Interactive Entertainment. Game
artwork and information belong to their owners. Use only games you own.

<a href="https://slint.dev"><img src="docs/badges/MadeWithSlint-logo-whitebg.png" alt="#MadeWithSlint" height="40"></a>
