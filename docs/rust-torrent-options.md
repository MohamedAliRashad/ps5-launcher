# Rust torrent backend investigation

Documentation reviewed on 2026-09-30. **librqbit 9.0.1 is now integrated as an
explicitly activated background worker.** Generated loopback fixtures have been
transferred; no catalog/game magnets were accessed during implementation or
validation. Use downloads only for content you are authorized to obtain.

## Recommendation

**Selected: `librqbit` 9.0.1, default features off, `rust-tls` on.** It fits the
single-binary launcher without installing a daemon. The full graph builds with
Rust 1.98.1. The former 1.88 claim was inconsistent with resolved Slint 1.18.1,
which declares 1.92; the launcher now declares 1.92 too. That is a declared floor,
not proof that the complete graph was tested on 1.92. librqbit uses edition 2024
and publishes no explicit MSRV.

| Option | Strengths | Costs / caveats |
|---|---|---|
| `librqbit` / rqbit 9.0.1 | Rust engine; magnet metadata, selective files, pause/resume, persistence, progress/speeds/peer statistics; Apache-2.0 | Requires Tokio; MSRV not verified; no high-level tracker scrape/count API confirmed |
| Transmission 4.1.3 + Rust RPC | Mature separate daemon; persistent sessions, file selection, tracker scrape counts and timestamps | Daemon management; `transmission-rpc` 0.5.0 is async and targets the deprecated legacy protocol; Transmission engine has GPL terms |
| aria2 + `aria2-ws` 0.5.1 | Established daemon; metadata-only discovery, file selection, RPC notifications | Tokio/WebSocket wrapper; configure session persistence; metadata and payload jobs can have different IDs; engine GPLv2-or-later |

### librqbit APIs confirmed in published documentation

- `Session::new_with_opts()` owns an async session; `AddTorrent::from_url()`
  accepts a magnet or other supported source.
- `AddTorrentOptions::list_only` resolves metadata/listing without payload
  transfer. **Resolving a magnet still contacts trackers/DHT/peers.**
- `only_files`, `only_files_regex`, `Session::update_only_files()` provide file
  selection. Do not automatically start all files after metadata arrives.
- `Session::pause()`, `unpause()`, `stop()` provide lifecycle controls;
  `delete(..., delete_files: bool)` separates removal from file deletion.
- `SessionOptions::persistence` and `fastresume` support saved sessions.
- Torrent statistics expose bytes/progress, uploaded bytes, errors/state,
  per-file progress, speeds, ETA and peer statistics.

The linked engine and launcher lifecycle are exercised by generated loopback
fixtures; public tracker/DHT behavior and authorization are not certified by
those tests. librqbit declares Apache-2.0; include applicable license/NOTICE
material and complete the dependency/distribution audit before publishing binaries.

### Recovery verification

The published 9.0.1 manifest and option documentation were checked again after
the interrupted editor session. Edition 2024, Apache-2.0, `list_only`, file
selection, persistence and fast-resume options are confirmed. The manifest has
no declared `rust-version`; Rust 1.88 is not supported by the resolved Slint graph.
The manifest also exposes a `disable-upload` feature, but it is **not enabled**.
Active downloads may upload pieces, capped at 128 KiB/s for the session. Completed
jobs now continue seeding by default while the launcher is open; the Settings
opt-out or a transfer's Stop seeding removes live handles without deleting files.
Turning the setting on never resumes saved/stopped jobs.
Session defaults enable persistent DHT, reinforcing the requirement for consent
before constructing a session, even for a metadata-only operation.

## Peer counts are not interchangeable

1. **Catalog seeders/leechers:** source-site observations from the JSON snapshot,
   with its collection time. These are what the current UI displays.
2. **Connected peers/seeders:** this client's current connections, not the swarm
   population. aria2's `numSeeders` specifically counts connected seeders.
3. **Tracker-reported counts:** per-tracker announce/scrape data with observation
   time, errors and expiration. Do not add counts across trackers: peers overlap.

DHT discovers peer addresses, not an authoritative seed/leecher census. No public
high-level scrape/count API was confirmed for librqbit 9.0.1; its checked UDP
implementation does not implement scrape requests. Transmission's RPC explicitly
exposes tracker counts and scrape results. A separate count provider may therefore
be necessary if live tracker statistics are a requirement.

Website login requirements do not prove a torrent is private. For metadata with
`private=1`, honor BEP 27 and tracker authorization; an infohash is not a substitute
for authorization and must not be used to bypass discovery restrictions.

## Implemented launcher integration

- Engine linked into the binary, but no session/network is started until an
  explicit user metadata/resume action. A bounded command interface and at most
  32 saved jobs keep launcher state manageable.
- A dedicated worker thread owns a Tokio runtime and the session. Slint remains
  on its event loop and polls bounded snapshots once per second. Stable native
  row models update only changed records, preserving animation and input state.
- Explicit consent **before constructing a session or resolving metadata**. Never
  check the swarm for every catalog entry at startup or on catalog refresh.
- Separate metadata discovery, file/destination confirmation, and payload start.
  Show total bytes, file names, network/upload behavior and destination first;
  check available space against missing/unallocated payload before starting.
- Validate magnet schemes/infohashes; reject path traversal, absolute paths and
  unsafe symlink destinations. Constrain deletion to owned transfer paths.
- Pause, Cancel and Remove all keep payload files. No destructive deletion action
  is exposed. App-managed persistence restores active jobs paused; cached metadata
  is reused and pieces are verified on explicit resume. Idle engine sessions stop.
- Define upload limits and post-completion seeding behavior. A download may upload
  while in progress; “do not seed after completion” does not mean “no uploads.”
  In aria2, `max-upload-limit=0` means unlimited, not disabled.
- Do not automatically extract archives, install packages, launch games, or mark
  partial content as an installed game. Any subsequent workflow needs a separate
  explicit action and validation.
  Implemented: completed transfers offer a separate Install confirmation with an
  editable destination. A lazy, cancellable installer thread stages, validates,
  publishes without overwriting, and registers the game before exposing Play.
  Downloads and archives remain intact; no payload is executed during installation.

### Verified behavior and remaining limits

The 54-test Rust suite includes 17 torrent tests and 17 installation tests: unsafe magnets/metadata/path
rejection, ownership markers, disk-space accounting for missing/sparse files,
stale/aborted/panicked resolver generations, metadata-only discovery, intermediate
verified progress, pause/restart/resume with a deliberately corrupted piece,
active shutdown and cancellation/removal that retain partial files. Additional
seeding tests verify upload to a second loopback leecher, default/disabled completion,
immediate Settings-off behavior for multiple seeds, manual Stop/Remove retaining
payloads, and completed shutdown/restart with no automatic traffic. Runtime seeding
flags and upload speeds are not persisted; completion remains compatible with Install.
Loopback
tests disable DHT, trackers, local discovery and UPnP and use freshly generated
512-KiB content. No public release was downloaded.

Native network-isolated UI smoke checks generated paused/Ready snapshots, progress
bars and file review, destination editing, cancelled consent, keyboard navigation,
keep-files removal, explicit generated-TAR installation/registration and graceful quit. This is UI validation, not a public swarm
integration test.

Magnet lookup can discover peers before the private flag is known; the consent
screen states this. Payload discovery preserves the engine's private-torrent
restrictions. Tests do not guarantee public tracker availability, permissions,
filesystem quota/CoW behavior, disk exhaustion or resistance to concurrent
same-user filesystem replacement attacks. Keep owned folders private; downloaded
payload integrity is not a malware/authenticity guarantee.

Installation tests use self-generated TAR, ZIP, 7z/split 7z and stored RAR4 archives,
with exact extracted bytes/source digests, cancellation, unsafe paths/links/duplicate
entries, invalid game metadata/IDs, and atomic no-overwrite publication. System
libarchive 3.7.2 was used; compressed/solid/multipart RAR variants are not certified.
This runtime accepted corrupted stored-RAR4 payload data despite its CRC during a
probe, so archive decoding is not a universal integrity guarantee. See the
[installation workflow](../README.md#download--install--play) for supported inputs,
explicit confirmation, recovery and runtime requirements.

## Licensing and sources

librqbit is Apache-2.0; preserve applicable license/NOTICE material and audit its
dependencies. The Rust daemon wrappers are MIT, but that does not relicense the
daemon engines. Separate-process RPC differs from linking a GPL engine; bundling
a daemon still requires compliance with its distribution license. Review the
actual packaging before shipping.

- [librqbit stable crate](https://crates.io/crates/librqbit)
- [Published 9.0.1 manifest](https://docs.rs/crate/librqbit/9.0.1/source/Cargo.toml.orig)
- [Session API](https://docs.rs/librqbit/9.0.1/librqbit/struct.Session.html)
- [AddTorrentOptions](https://docs.rs/librqbit/9.0.1/librqbit/struct.AddTorrentOptions.html)
- [SessionOptions](https://docs.rs/librqbit/9.0.1/librqbit/struct.SessionOptions.html)
- [librqbit statistics](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/torrent_state/stats.rs)
- [Tracker communication](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/tracker_comms/src/tracker_comms.rs)
- [UDP tracker implementation](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/tracker_comms/src/tracker_comms_udp.rs)
- [transmission-rpc](https://crates.io/crates/transmission-rpc)
- [Transmission 4.1.3 RPC specification](https://github.com/transmission/transmission/blob/4.1.3/docs/rpc-spec.md)
- [Transmission license](https://github.com/transmission/transmission/blob/4.1.3/COPYING)
- [aria2-ws](https://crates.io/crates/aria2-ws)
- [aria2 manual and licensing](https://aria2.github.io/manual/en/html/aria2c.html)
- [BEP 5: DHT](https://www.bittorrent.org/beps/bep_0005.html)
- [BEP 27: private torrents](https://www.bittorrent.org/beps/bep_0027.html)