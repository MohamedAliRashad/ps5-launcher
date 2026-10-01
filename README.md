# PS5 Launcher

A PS5-style home screen for Linux. Browse PS5 games with their official artwork, download and
install them, and play them with the [KytyPS5](https://github.com/KytyPS5/KytyPS5) emulator,
using a controller, keyboard or mouse.

![PS5 Launcher: moving through the Home screen, a Game Hub and the Library](docs/demo/demo-loop.webp)

- **Your games on a PS5-style Home screen**, with official backgrounds, logos and trailers.
- **A Library of 400+ PS5 games** with search, genre filters and sorting.
- **See what runs:** every game shows how far it gets in KytyPS5.
- **Download, install and play** without leaving the launcher.
- **Keeps itself and KytyPS5 up to date**, without touching your saves.

## Install

You need 64-bit Linux: Ubuntu 22.04+, Debian 12+, Fedora 36+, Mint 21+, Arch or SteamOS 3.
Open a terminal and run:

```bash
curl -fsSL https://github.com/MohamedAliRashad/ps5-launcher/releases/latest/download/ps5-launcher-linux-x86_64.tar.gz | tar xz && ./ps5-launcher-linux-x86_64/install.sh
```

**PS5 Launcher** is now in your app menu. For trailers, switching between a game and the
launcher, and installing from archives, also run (Ubuntu, Debian, Mint):

```bash
sudo apt install xdotool mpv yt-dlp libarchive13
```

To uninstall: `./ps5-launcher-linux-x86_64/install.sh --uninstall`

## Getting started

1. Open **PS5 Launcher**. The first launch downloads artwork and KytyPS5, which takes about a
   minute.
2. Already have games? Add their folder in **Settings → Game folders**.
3. To get a game, open it in the **Library**, choose **Download**, then **Install**, then
   **Play**. Keep the launcher open while it downloads.

> Only download games you own. Downloads use BitTorrent: other people can see your IP address,
> and finished downloads are shared while the launcher is open (turn this off in
> **Settings → Seed completed downloads**).

## Controls

| Action | Controller | Keyboard |
|---|---|---|
| Move / Select / Back | D-pad, ✕, ○ | Arrows, Enter, Esc |
| Options for a game | Options | O |
| Search | △ | / |
| Switch Home ⇄ Library | L1 / R1 | Tab |
| Switch between game and launcher | PS button | |
| Downloads | | Ctrl+D |

## Will my game run?

Each game shows a tag from the [KytyPS5 compatibility list](https://kytyps5.github.io/):
🟢 **In-game**, 🟡 **Menus**, 🟠 **Boots** (intro logos only), 🔴 **Doesn't boot**, or ⚪ **Untested**.

Linux results come first. A result from Windows is marked **Win**, because games can behave
differently on Linux. In the Library, **In-game** lists every game that reaches gameplay, and
**In-game on Linux** only the ones confirmed on Linux.

**Help others:** when you close a game, the launcher asks how far it got, and your answer
becomes its tag on your PC. Every so often, choose **Settings → Share your results with
KytyPS5** to send your results to the community list (needs a free GitHub account).

## Help

- **A game closes right away:** check its tag, then **Options → View emulator log**.
- **Resume or Stop doesn't work:** install `xdotool`.
- **Wrong screen:** change **Settings → Display**. For a window, run `ps5-launcher --windowed`.
- **Too big or too small:** run `PS5_LAUNCHER_SCALE=1.15 ps5-launcher` (bigger number, bigger UI).
- **A new KytyPS5 broke a game:** **Settings → Roll back to previous KytyPS5**.

Still stuck? [Open an issue](https://github.com/MohamedAliRashad/ps5-launcher/issues).
Developers: see the [developer guide](docs/DEVELOPMENT.md).

---

PS5 Launcher is an unofficial fan project, not affiliated with or endorsed by Sony Interactive
Entertainment. "PlayStation" and "PS5" are trademarks of Sony Interactive Entertainment. Game
artwork and information belong to their owners. Use only games you own.

<a href="https://slint.dev"><img src="docs/badges/MadeWithSlint-logo-whitebg.png" alt="#MadeWithSlint" height="40"></a>
