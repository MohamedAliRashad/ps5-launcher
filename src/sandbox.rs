//! Run an untrusted helper so it can only write inside chosen folders, with Linux's Landlock
//! (5.13+). Used for the PKG extractor: it builds output paths from names stored inside the
//! package, so a crafted PKG could otherwise write anywhere the user can.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_RULESET_VERSION: u32 = 1;
const RULE_PATH_BENEATH: u32 = 1;

// Filesystem rights that change something (reads and execution stay allowed).
const WRITE_FILE: u64 = 1 << 1;
const REMOVE_DIR: u64 = 1 << 4;
const REMOVE_FILE: u64 = 1 << 5;
const MAKE_ALL: u64 = 0b111_1111 << 6; // char, dir, reg, sock, fifo, block, sym
const REFER: u64 = 1 << 13; // ABI 2
const TRUNCATE: u64 = 1 << 14; // ABI 3
const NET_TCP: u64 = 0b11; // bind and connect, ABI 4

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
}

#[repr(C, packed)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Keep this alive until the command has been spawned.
pub struct Confinement(#[allow(dead_code)] OwnedFd);

fn abi() -> i64 {
    unsafe { libc::syscall(libc::SYS_landlock_create_ruleset, std::ptr::null::<RulesetAttr>(), 0usize, CREATE_RULESET_VERSION) }
}

/// Whether this kernel can confine helpers.
#[cfg(test)]
pub fn available() -> bool {
    abi() >= 1
}

/// Confine `cmd` (when spawned) to writing inside `writable` folders, plus /dev/null, and
/// to no TCP networking where the kernel supports that.
pub fn confine(cmd: &mut Command, writable: &[&Path]) -> Result<Confinement, String> {
    let abi = abi();
    if abi < 1 {
        return Err("this Linux kernel can't confine the PKG extractor (Landlock, Linux 5.13 or newer, is needed)".into());
    }
    let mut fs = WRITE_FILE | REMOVE_DIR | REMOVE_FILE | MAKE_ALL;
    if abi >= 2 {
        fs |= REFER;
    }
    if abi >= 3 {
        fs |= TRUNCATE;
    }
    let attr = RulesetAttr { handled_access_fs: fs, handled_access_net: if abi >= 4 { NET_TCP } else { 0 } };
    let size = if abi >= 4 { std::mem::size_of::<RulesetAttr>() } else { std::mem::size_of::<u64>() };
    let fd = unsafe { libc::syscall(libc::SYS_landlock_create_ruleset, &attr as *const RulesetAttr, size, 0u32) };
    if fd < 0 {
        return Err(format!("Landlock ruleset: {}", std::io::Error::last_os_error()));
    }
    let ruleset = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    let file_rights = WRITE_FILE | if abi >= 3 { TRUNCATE } else { 0 };
    let rules = writable.iter().map(|p| (p.to_path_buf(), fs)).chain([(Path::new("/dev/null").to_path_buf(), file_rights)]);
    for (path, rights) in rules {
        let file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let beneath = PathBeneathAttr { allowed_access: rights, parent_fd: file.as_raw_fd() };
        let r = unsafe { libc::syscall(libc::SYS_landlock_add_rule, ruleset.as_raw_fd(), RULE_PATH_BENEATH, &beneath as *const PathBeneathAttr, 0u32) };
        if r != 0 {
            return Err(format!("Landlock rule for {}: {}", path.display(), std::io::Error::last_os_error()));
        }
    }
    let raw = ruleset.as_raw_fd();
    // Runs in the child between fork and exec: only async-signal-safe system calls.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 || libc::syscall(libc::SYS_landlock_restrict_self, raw, 0u32) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(Confinement(ruleset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confined_helper_writes_only_inside_its_folder() {
        if !available() {
            eprintln!("Landlock unavailable on this kernel; skipped");
            return;
        }
        let t = tempfile::tempdir().unwrap();
        let inside = t.path().join("inside");
        std::fs::create_dir(&inside).unwrap();
        let script = format!(
            "echo ok > '{0}/a' && mkdir '{0}/d' && echo ok > '{0}/d/b' && rm '{0}/a' && echo x > /dev/null || exit 1; \
             echo bad > '{1}/escaped' 2>/dev/null && exit 2; ln -s '{0}' '{1}/link' 2>/dev/null && exit 3; \
             echo bad > '{0}/../escaped2' 2>/dev/null && exit 4; exit 0",
            inside.display(), t.path().display());
        // Unconfined, the same script escapes: the check below is meaningful.
        let free = tempfile::tempdir().unwrap();
        std::fs::create_dir(free.path().join("inside")).unwrap();
        let free_script = script.replace(&t.path().display().to_string(), &free.path().display().to_string());
        assert_eq!(Command::new("/bin/sh").arg("-c").arg(&free_script).status().unwrap().code(), Some(2));
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&script);
        let guard = confine(&mut cmd, &[&inside]).unwrap();
        let status = cmd.status().unwrap();
        drop(guard);
        assert_eq!(status.code(), Some(0), "writes inside work, every write outside is refused");
        assert!(inside.join("d/b").is_file());
        assert!(!t.path().join("escaped").exists() && !t.path().join("escaped2").exists() && !t.path().join("link").exists());
    }
}
