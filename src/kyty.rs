//! KytyPS5 auto-updater.
//!
//! KytyPS5 publishes prebuilt Linux builds on GitHub Releases. The launcher can manage its own
//! copy under ~/.local/share/ps5-launcher/kyty:
//!
//!   versions/<tag>/     one extracted release per folder
//!   current -> versions/<tag>
//!   data/               _SaveData, _PipelineCache, ... shared by every version (symlinked in)
//!   state.json          installed / previous tag
//!
//! Kyty writes its data relative to its working directory, so each version folder gets symlinks
//! to `data/`: saves and shader caches survive updates and rollbacks.

use crate::util::{atomic_write, http_json, now_secs};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const REPO: &str = "KytyPS5/KytyPS5";
/// Folders Kyty writes relative to its working directory.
pub const DATA_DIRS: [&str; 6] = ["_SaveData", "_PipelineCache", "_DownloadData", "_TempData", "_Textures", "_Patches"];
pub const CHECK_INTERVAL: f64 = 6.0 * 3600.0;

#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct State {
    pub installed: String,
    pub previous: String,
    pub last_check: f64,
    pub latest: String,
    /// A build the user rolled back from: not re-installed automatically.
    pub skip: String,
}

pub fn root() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::expand_home("~/.local/share"))
        .join(crate::util::APP_NAME)
        .join("kyty")
}

/// Stable emulator path for the managed install (survives updates).
pub fn managed_emulator() -> PathBuf {
    root().join("current").join("kyty_emulator")
}

pub fn is_managed(emulator: &Path) -> bool {
    emulator.starts_with(root())
}

pub fn load_state() -> State {
    std::fs::read(root().join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_state(s: &State) {
    if let Ok(j) = serde_json::to_vec_pretty(s) {
        let _ = atomic_write(&root().join("state.json"), &j);
    }
}

pub fn mark_checked(latest: &str) {
    let mut s = load_state();
    s.last_check = now_secs();
    s.latest = latest.to_string();
    save_state(&s);
}

/// Short, human version of a tag: "KytyPS5-2026-09-29-59a1760" -> "2026-09-29 · 59a1760".
pub fn pretty(tag: &str) -> String {
    let t = tag.trim_start_matches("KytyPS5-");
    match t.rsplit_once('-') {
        Some((date, sha)) if sha.len() >= 7 && date.len() == 10 => format!("{date} · {sha}"),
        _ => t.to_string(),
    }
}

/// Commit id of a tag ("…-59a1760" -> "59a1760").
pub fn tag_commit(tag: &str) -> &str {
    tag.rsplit('-').next().unwrap_or("")
}

/// Latest Linux x86_64 build on GitHub.
pub fn latest_release() -> Result<Release, String> {
    let v = http_json(&format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    let tag = v["tag_name"].as_str().ok_or("no releases found")?.to_string();
    let asset = v["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.contains("Linux") && n.contains("x86_64") && n.ends_with(".tar.gz")))
        .ok_or("this release has no Linux build yet")?;
    Ok(Release {
        tag,
        url: asset["browser_download_url"].as_str().unwrap_or("").to_string(),
        size: asset["size"].as_u64().unwrap_or(0),
        sha256: asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).unwrap_or("").to_string(),
    })
}

/// Version info of any emulator binary, from its `--help` banner ("git = 6799ecb, date = 2026.09.29").
pub fn binary_version(emulator: &Path) -> Option<(String, String)> {
    let out = Command::new(emulator).arg("--help").current_dir(emulator.parent()?).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?;
    let field = |k: &str| line.split(&format!("{k} = ")).nth(1).map(|s| s.split(',').next().unwrap_or("").trim().to_string());
    Some((field("git")?, field("date").unwrap_or_default().replace('.', "-")))
}

/// Large downloads can take minutes: no overall deadline, only a stall timeout per read.
static DOWNLOADER: std::sync::LazyLock<ureq::Agent> = std::sync::LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .user_agent(crate::util::USER_AGENT)
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
});

fn download(rel: &Release, dest: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let resp = DOWNLOADER.get(&rel.url).call().map_err(|e| format!("download failed: {e}"))?;
    let total = resp.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(rel.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    if !rel.sha256.is_empty() {
        let got = format!("{:x}", hasher.finalize());
        if !got.eq_ignore_ascii_case(&rel.sha256) {
            return Err("download is corrupted (checksum mismatch)".into());
        }
    }
    Ok(())
}

fn link_data_dirs(version_dir: &Path) -> Result<(), String> {
    let data = root().join("data");
    for d in DATA_DIRS {
        let shared = data.join(d);
        std::fs::create_dir_all(&shared).map_err(|e| e.to_string())?;
        let link = version_dir.join(d);
        if link.symlink_metadata().is_ok() {
            if link.is_symlink() {
                std::fs::remove_file(&link).map_err(|e| e.to_string())?;
            } else {
                // A real folder shipped in the archive: fold its contents into the shared one.
                let _ = Command::new("cp").args(["-an"]).arg(link.join(".")).arg(&shared).status();
                std::fs::remove_dir_all(&link).map_err(|e| e.to_string())?;
            }
        }
        std::os::unix::fs::symlink(&shared, &link).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Copy saves and caches from a self-built/custom Kyty folder into the shared data folder
/// (only into folders that are still empty; the source is never modified).
pub fn import_data_from(emulator_dir: &Path) -> usize {
    let data = root().join("data");
    let mut copied = 0;
    for d in DATA_DIRS {
        let src = emulator_dir.join(d);
        let dst = data.join(d);
        if !src.is_dir() || src.is_symlink() {
            continue;
        }
        let empty = std::fs::read_dir(&dst).map(|mut r| r.next().is_none()).unwrap_or(true);
        if !empty {
            continue;
        }
        let _ = std::fs::create_dir_all(&dst);
        if Command::new("cp").args(["-a"]).arg(src.join(".")).arg(&dst).status().is_ok_and(|s| s.success()) {
            copied += 1;
        }
    }
    copied
}

fn switch_current(tag: &str) -> Result<(), String> {
    let r = root();
    let tmp = r.join("current.new");
    let _ = std::fs::remove_file(&tmp);
    std::os::unix::fs::symlink(Path::new("versions").join(tag), &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, r.join("current")).map_err(|e| e.to_string())
}

/// Download, verify, extract and activate a release. Keeps the previous version for rollback.
pub fn install(rel: &Release, progress: &dyn Fn(String, f32)) -> Result<(), String> {
    let r = root();
    let versions = r.join("versions");
    std::fs::create_dir_all(&versions).map_err(|e| e.to_string())?;
    let archive = r.join(format!("{}.tar.gz.part", rel.tag));
    let last = std::cell::Cell::new(0u64);
    download(rel, &archive, &|done, total| {
        let pct = if total > 0 { done * 100 / total } else { 0 };
        if pct != last.get() {
            last.set(pct);
            progress(format!("Downloading KytyPS5 {} · {pct}%", pretty(&rel.tag)), pct as f32 / 100.0 * 0.9);
        }
    })
    .inspect_err(|_| {
        let _ = std::fs::remove_file(&archive);
    })?;

    progress("Installing KytyPS5…".into(), 0.95);
    let staging = versions.join(format!("{}.tmp", rel.tag));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let ok = Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(&staging).status().is_ok_and(|s| s.success());
    let _ = std::fs::remove_file(&archive);
    if !ok {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("could not extract the archive (is `tar` installed?)".into());
    }
    // Some archives wrap everything in one top-level folder.
    let mut dir = staging.clone();
    if !dir.join("kyty_emulator").is_file() {
        if let Some(inner) = std::fs::read_dir(&staging).ok().and_then(|rd| rd.flatten().map(|e| e.path()).find(|p| p.join("kyty_emulator").is_file())) {
            dir = inner;
        }
    }
    let emu = dir.join("kyty_emulator");
    if binary_version(&emu).is_none() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("the downloaded emulator does not run on this system".into());
    }
    link_data_dirs(&dir)?;
    let final_dir = versions.join(&rel.tag);
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&dir, &final_dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&staging);

    let mut st = load_state();
    if st.installed != rel.tag {
        st.previous = std::mem::take(&mut st.installed);
    }
    st.installed = rel.tag.clone();
    st.latest = rel.tag.clone();
    if st.skip == rel.tag {
        st.skip.clear();
    }
    st.last_check = now_secs();
    switch_current(&rel.tag)?;
    save_state(&st);
    cleanup(&st);
    crate::log!("KytyPS5 installed: {}", rel.tag);
    Ok(())
}

/// Switch back to the previously installed version.
pub fn rollback() -> Result<String, String> {
    let mut st = load_state();
    if st.previous.is_empty() || !root().join("versions").join(&st.previous).is_dir() {
        return Err("no previous version to roll back to".into());
    }
    switch_current(&st.previous)?;
    st.skip = st.installed.clone();
    std::mem::swap(&mut st.installed, &mut st.previous);
    save_state(&st);
    Ok(st.installed)
}

/// Keep only the installed and previous versions.
fn cleanup(st: &State) {
    let Ok(rd) = std::fs::read_dir(root().join("versions")) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name != st.installed && name != st.previous {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tags() {
        assert_eq!(pretty("KytyPS5-2026-09-29-59a1760"), "2026-09-29 · 59a1760");
        assert_eq!(tag_commit("KytyPS5-2026-09-29-59a1760"), "59a1760");
    }
}
