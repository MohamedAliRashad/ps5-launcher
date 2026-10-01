# PS5 Launcher

A PS5-style home screen for your Linux PC. Browse PS5 games with their official artwork, download
and install them, and play them with the [KytyPS5](https://github.com/KytyPS5/KytyPS5) emulator,
all from one app you can use with a controller.

![PS5 Launcher: moving through the Home screen, a Game Hub and the Library](docs/demo/demo-loop.webp)

**Narrated walkthrough:** [Watch the 9-minute practical tour](docs/demo/ps5-launcher-walkthrough.mp4)
([smaller 720p copy](docs/demo/ps5-launcher-walkthrough-720p.mp4),
[transcript](docs/demo/ps5-launcher-walkthrough-transcript.md)). Real launcher screens, calm
English synthetic narration, optional captions and chapters — no music or promotional effects.
Download and installation examples use clearly labeled sample data, not emulator gameplay.

## What it does

- **Looks and feels like a PS5.** Your games sit on a Home screen with their official
  backgrounds, logos and trailers.
- **Shows the whole catalog.** The Library lists more than 400 PS5 games, with search, genre
  filters and sorting.
- **Tells you what runs.** Every game shows how far it gets in KytyPS5, from "Doesn't boot" to
  "In-game".
- **Downloads and installs games for you.** Pick a release, download it in the background, then
  press Install. When it's done, press Play.
- **Sets up the emulator for you.** It installs KytyPS5 on first launch and keeps it up to date.
- **Remembers your playtime** for every game.
- **Works with a controller, keyboard or mouse.**

## Requirements

- A 64-bit **Linux** PC: Ubuntu 22.04 or newer, Debian 12+, Fedora 36+, Linux Mint 21+,
  Pop!_OS 22.04+, Arch or SteamOS 3.
- A graphics card that can run the KytyPS5 emulator. See KytyPS5's own requirements.

Windows and macOS aren't supported yet.

## Install

Open a terminal, paste this line and press Enter:

```bash
curl -fsSL https://github.com/MohamedAliRashad/ps5-launcher/releases/latest/download/ps5-launcher-linux-x86_64.tar.gz | tar xz && ./ps5-launcher-linux-x86_64/install.sh
```

That's it. **PS5 Launcher** is now in your app menu. You don't need an admin password.

**Recommended extras.** These turn on a few more features (switching between a game and the
launcher, trailers inside the app, and installing games from archives). On Ubuntu, Debian or
Mint:

```bash
sudo apt install xdotool mpv yt-dlp libarchive13
```

**Uninstall:** run `./ps5-launcher-linux-x86_64/install.sh --uninstall`. Your settings are kept.

## Getting started

1. **Open PS5 Launcher** from your app menu.
2. **Wait for the first-time setup.** It downloads the game artwork and the KytyPS5 emulator,
   which takes about a minute. Press **Enter** (or **✕** on a controller) when it's ready.
3. **Add your games.** If you already have PS5 games, go to **Settings → Game folders** and
   choose the folder they're in. They appear on the Home screen.
4. **Pick a game and press Play.**

### Getting games through the launcher

1. Open the **Library** tab and pick a game.
2. In its Game Hub, choose **Download**, pick where to save it, and choose **Look up metadata**.
3. Check the files and size, then choose **Start download**. You can keep browsing while it
   downloads. Press **Ctrl+D** to see your downloads.
4. When it's finished, choose **Install**, then **Play**.

Keep the launcher open while a game downloads or installs. If you close it, the download is
paused, and you can resume it later from Downloads.

> Only download games you own or are allowed to download. Downloads use BitTorrent, so other
> people can see your IP address, and finished downloads are shared with others while the
> launcher is open. You can turn this sharing off in **Settings → Seed completed downloads**.

## Controls

| Action | Controller | Keyboard | Mouse |
|---|---|---|---|
| Move | D-pad / left stick | Arrow keys | Scroll wheel |
| Select / Play | ✕ | Enter | Click |
| Back | ○ | Esc | Click outside a panel |
| Watch the trailer | □ | T | |
| Search | △ | / or just start typing | Click the search box |
| Options for a game | Options | O | |
| Switch Home ⇄ Library | L1 / R1 | Tab | Click the tab |
| Switch between a game and the launcher | PS button | | |
| Downloads | | Ctrl+D | Download icon in the top bar |
| Quit | | Ctrl+Q | |

## Will my game run?

Every game shows a tag from the community
[KytyPS5 compatibility list](https://kytyps5.github.io/):

| Tag | What it means |
|---|---|
| 🟢 **In-game** | You can play it |
| 🟡 **Menus** | Reaches the main menu, but not gameplay |
| 🟠 **Boots** | Shows the intro logos, then stops |
| 🔴 **Doesn't boot** | Doesn't start |
| ⚪ **Untested** | Nobody has reported it yet |

Tags show the **Linux** result whenever someone has tested the game on Linux. Results from
Windows are marked, for example **Win · In-game** on a Library cover, because a game can behave
differently on Linux. The Game Hub shows the Linux and Windows results separately. In the
Library, the **In-game on Linux** filter shows only games confirmed to reach gameplay on Linux.

The launcher downloads the latest list every 6 hours. The tags are only as up to date as the
reports people send, and most games have no report yet, especially on Linux. After you play a
game, you can help: open its **Options → Report how it runs**. Your browser opens KytyPS5's
report form, already filled in with the game, your PC's details and the end of the emulator log,
and the folder with the full log opens next to it. Choose how far the game got, drag the log
file into the form, and submit (you need a free GitHub account). Once KytyPS5 adds your report
to the list, every launcher shows the new tag within 6 hours.

## Updates

Both the launcher and KytyPS5 update themselves in the background. Updates never happen while a
game is running, and your saves are never touched. To turn updates off, use the two
**Keep … updated automatically** switches in **Settings**.

If a new KytyPS5 version breaks a game, use **Settings → Roll back to previous KytyPS5**.

## Screenshots

| Game Hub | Settings |
|---|---|
| ![Game Hub](docs/screenshots/game-hub.jpg) | ![Settings](docs/screenshots/settings.jpg) |

## Help

- **A game closes right away.** Check its tag first: many games don't run in KytyPS5 yet. To see
  what happened, open **Options → View emulator log**.
- **Resume or Stop doesn't work.** Install `xdotool` (see [Install](#install)).
- **The launcher opens on the wrong screen.** Change **Settings → Display**.
- **You want it in a window, not fullscreen.** Start it from a terminal with
  `ps5-launcher --windowed`.
- **Everything looks too big or too small.** Start it with `PS5_LAUNCHER_SCALE=1.15 ps5-launcher`.
  Use a bigger number to make things larger, or a smaller one to make them smaller.
- **The controller's PS button doesn't work in games.** Add yourself to the `input` group with
  `sudo usermod -aG input $USER`, then log out and back in.
- **Start over from scratch:** `rm -rf ~/.config/ps5-launcher ~/.cache/ps5-launcher`

Still stuck? [Open an issue](https://github.com/MohamedAliRashad/ps5-launcher/issues).

## For developers

To build from source, run the tests, or learn how the catalog, downloads and installer work,
see the [developer guide](docs/DEVELOPMENT.md).

## Disclaimer

PS5 Launcher is an unofficial fan project. It isn't affiliated with or endorsed by Sony
Interactive Entertainment. "PlayStation" and "PS5" are trademarks of Sony Interactive
Entertainment. Game artwork and information belong to their owners and are loaded from public
services while the app runs. Use only games you own.

---

<a href="https://slint.dev"><img src="docs/badges/MadeWithSlint-logo-whitebg.png" alt="#MadeWithSlint" height="48"></a>
