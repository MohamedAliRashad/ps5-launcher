//! Read-only exFAT disk images, for releases shipped as a single `.exfat` file. Implements just
//! enough of Microsoft's published exFAT layout to list files and stream their contents. Every
//! offset, cluster number, chain and name is checked, so a damaged or hostile image fails with an
//! error instead of reading outside the image or naming a path outside the installer's stage.

use anyhow::{ensure, Context, Result};
use std::{fs::File, os::unix::fs::FileExt};

const SIGNATURE: &[u8] = b"EXFAT   ";
/// Cluster numbers at or above this end a chain; one below marks a bad cluster.
const END: u32 = 0xFFFF_FFF8;
const BAD: u32 = 0xFFFF_FFF7;
const MAX_DIR: u64 = 64 * 1024 * 1024;
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 128;
const CHUNK: usize = 1024 * 1024;

/// Whether a file starts with an exFAT boot sector.
pub fn is_image(file: &File) -> bool {
    let mut b = [0u8; 11];
    file.read_exact_at(&mut b, 0).is_ok() && &b[3..11] == SIGNATURE
}

/// A file or folder inside the image. `path` uses `/` and is relative to the image root.
#[derive(Clone, Debug)]
pub struct Node {
    pub path: String,
    pub directory: bool,
    pub size: u64,
    valid: u64,
    first: u32,
    contiguous: bool,
}

#[derive(Debug)]
pub struct Volume {
    file: File,
    cluster: u64,
    fat: u64,
    heap: u64,
    count: u32,
    root: u32,
}

impl Volume {
    pub fn open(file: File) -> Result<Self> {
        let mut b = [0u8; 512];
        file.read_exact_at(&mut b, 0).context("Read exFAT boot sector")?;
        ensure!(&b[3..11] == SIGNATURE && b[510] == 0x55 && b[511] == 0xAA, "Not an exFAT image");
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as u64;
        let (sector_shift, cluster_shift) = (b[108] as u32, b[109] as u32);
        ensure!((9..=12).contains(&sector_shift) && sector_shift + cluster_shift <= 25, "Unsupported exFAT sector or cluster size");
        let sector = 1u64 << sector_shift;
        let cluster = 1u64 << (sector_shift + cluster_shift);
        let (fat, fat_len, heap, count, root) = (u32_at(80) * sector, u32_at(84) * sector, u32_at(88) * sector, u32_at(92), u32_at(96));
        let len = file.metadata()?.len();
        ensure!(count >= 1 && fat_len >= (count + 2) * 4 && fat + fat_len <= len, "Invalid exFAT allocation table");
        ensure!(heap.checked_add(count * cluster).is_some_and(|end| end <= len),
            "The exFAT image is shorter than its header says; wait until the download is complete");
        ensure!(root >= 2 && root - 2 < count, "Invalid exFAT root folder");
        Ok(Self { file, cluster, fat, heap, count: count as u32, root: root as u32 })
    }

    fn check(&self, c: u32) -> Result<()> {
        ensure!(c >= 2 && c - 2 < self.count, "exFAT cluster {c} is outside the image");
        Ok(())
    }

    fn next(&self, c: u32) -> Result<u32> {
        let mut b = [0u8; 4];
        self.file.read_exact_at(&mut b, self.fat + c as u64 * 4).context("Read exFAT allocation table")?;
        Ok(u32::from_le_bytes(b))
    }

    /// Runs of consecutive clusters (first, count) that hold `len` bytes.
    fn runs(&self, first: u32, len: u64, contiguous: bool) -> Result<Vec<(u32, u64)>> {
        if len == 0 {
            return Ok(Vec::new());
        }
        let needed = len.div_ceil(self.cluster);
        ensure!(needed <= self.count as u64, "exFAT file is larger than the image");
        self.check(first)?;
        if contiguous {
            ensure!((first - 2) as u64 + needed <= self.count as u64, "exFAT file runs past the end of the image");
            return Ok(vec![(first, needed)]);
        }
        let mut runs: Vec<(u32, u64)> = Vec::new();
        let mut c = first;
        for i in 0..needed {
            self.check(c)?;
            match runs.last_mut() {
                Some((start, n)) if *start as u64 + *n == c as u64 => *n += 1,
                _ => runs.push((c, 1)),
            }
            if i + 1 < needed {
                c = self.next(c)?;
                ensure!(c != BAD && c < END, "exFAT cluster chain ends before the file does");
            }
        }
        Ok(runs)
    }

    /// Stream `len` bytes stored in `runs` to `sink`, in chunks of up to 1 MiB.
    fn read(&self, runs: &[(u32, u64)], mut len: u64, sink: &mut dyn FnMut(&[u8]) -> Result<()>) -> Result<()> {
        let mut buf = vec![0u8; CHUNK];
        for &(start, n) in runs {
            let mut offset = self.heap + (start - 2) as u64 * self.cluster;
            let mut left = (n * self.cluster).min(len);
            while left > 0 {
                let take = left.min(CHUNK as u64) as usize;
                self.file.read_exact_at(&mut buf[..take], offset).context("Read exFAT image")?;
                sink(&buf[..take])?;
                offset += take as u64;
                left -= take as u64;
                len -= take as u64;
            }
        }
        ensure!(len == 0, "exFAT file is shorter than its directory entry says");
        Ok(())
    }

    fn folder(&self, first: u32, size: Option<u64>, contiguous: bool) -> Result<Vec<u8>> {
        let runs = match size {
            Some(size) => {
                ensure!(size <= MAX_DIR, "exFAT folder is too large");
                self.runs(first, size, contiguous)?
            }
            // The root folder has no recorded size: follow its chain to the end.
            None => {
                let (mut runs, mut c, mut total) = (Vec::new(), first, 0u64);
                loop {
                    self.check(c)?;
                    total += self.cluster;
                    ensure!(total <= MAX_DIR, "exFAT root folder is too large or loops");
                    runs.push((c, 1));
                    c = self.next(c)?;
                    if c >= END {
                        break;
                    }
                    ensure!(c != BAD, "exFAT root folder chain is damaged");
                }
                runs
            }
        };
        let len = runs.iter().map(|(_, n)| n * self.cluster).sum::<u64>().min(size.unwrap_or(u64::MAX));
        let mut data = Vec::with_capacity(len as usize);
        self.read(&runs, len, &mut |chunk| {
            data.extend_from_slice(chunk);
            Ok(())
        })?;
        Ok(data)
    }

    /// Every file and folder in the image, parents before their contents.
    pub fn list(&self, cancel: &dyn Fn() -> Result<()>) -> Result<Vec<Node>> {
        let mut out = Vec::new();
        let mut pending = vec![(String::new(), self.root, None, false, 0usize)];
        while let Some((prefix, first, size, contiguous, depth)) = pending.pop() {
            cancel()?;
            ensure!(depth < MAX_DEPTH, "exFAT folders are nested too deeply");
            for e in parse_folder(&self.folder(first, size, contiguous)?)? {
                let path = if prefix.is_empty() { e.name } else { format!("{prefix}/{}", e.name) };
                if e.directory {
                    pending.push((path.clone(), e.first, Some(e.size), e.contiguous, depth + 1));
                }
                out.push(Node { path, directory: e.directory, size: if e.directory { 0 } else { e.size }, valid: e.valid, first: e.first, contiguous: e.contiguous });
                ensure!(out.len() <= MAX_NODES, "exFAT image has too many files");
            }
        }
        Ok(out)
    }

    /// Stream a file's contents to `sink`. Bytes past its valid data length read as zeros.
    pub fn copy(&self, node: &Node, sink: &mut dyn FnMut(&[u8]) -> Result<()>) -> Result<()> {
        ensure!(!node.directory, "Not a file: {}", node.path);
        self.read(&self.runs(node.first, node.valid, node.contiguous)?, node.valid, sink)?;
        let zeros = vec![0u8; CHUNK];
        let mut left = node.size - node.valid;
        while left > 0 {
            let take = left.min(CHUNK as u64) as usize;
            sink(&zeros[..take])?;
            left -= take as u64;
        }
        Ok(())
    }
}

struct Raw {
    name: String,
    directory: bool,
    size: u64,
    valid: u64,
    first: u32,
    contiguous: bool,
}

/// Directory entry sets: a File entry (0x85), a Stream Extension (0xC0) and File Name entries
/// (0xC1). Deleted sets (in-use bit clear) and volume entries (bitmap, label, …) are skipped.
fn parse_folder(data: &[u8]) -> Result<Vec<Raw>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 32 <= data.len() {
        match data[i] {
            0x00 => break,
            0x85 => {}
            _ => {
                i += 32;
                continue;
            }
        }
        let secondary = data[i + 1] as usize;
        ensure!((2..=18).contains(&secondary), "Invalid exFAT file entry");
        let end = i + 32 * (1 + secondary);
        ensure!(end <= data.len(), "exFAT folder entry is cut off");
        let set = &data[i..end];
        ensure!(checksum(set) == u16::from_le_bytes([set[2], set[3]]), "exFAT folder entry checksum mismatch; the image is damaged");
        let attributes = u16::from_le_bytes([set[4], set[5]]);
        let s = &set[32..64];
        ensure!(s[0] == 0xC0, "exFAT file entry has no stream extension");
        let (flags, name_len) = (s[1], s[3] as usize);
        let valid = u64::from_le_bytes(s[8..16].try_into().unwrap());
        let first = u32::from_le_bytes(s[20..24].try_into().unwrap());
        let size = u64::from_le_bytes(s[24..32].try_into().unwrap());
        ensure!(name_len >= 1 && name_len.div_ceil(15) < secondary, "Invalid exFAT file name length");
        let mut units = Vec::with_capacity(name_len.div_ceil(15) * 15);
        for e in set[64..].chunks(32).take(name_len.div_ceil(15)) {
            ensure!(e[0] == 0xC1, "exFAT file name entry is missing");
            units.extend(e[2..32].chunks(2).map(|u| u16::from_le_bytes([u[0], u[1]])));
        }
        units.truncate(name_len);
        let name = String::from_utf16(&units).context("exFAT file name is not valid text")?;
        ensure!(name != "." && name != ".." && !name.chars().any(|c| c == '/' || c == '\\' || c.is_control()),
            "Unsafe file name in exFAT image: {name:?}");
        ensure!(valid <= size, "exFAT file {name:?} has more valid data than its size");
        ensure!(flags & 1 != 0 || size == 0, "exFAT file {name:?} has data but no clusters");
        out.push(Raw { name, directory: attributes & 0x10 != 0, size, valid, first, contiguous: flags & 2 != 0 });
        i = end;
    }
    Ok(out)
}

/// The entry-set checksum from the exFAT specification (bytes 2–3 of the first entry excluded).
fn checksum(set: &[u8]) -> u16 {
    set.iter().enumerate().filter(|(k, _)| *k != 2 && *k != 3)
        .fold(0u16, |c, (_, b)| c.rotate_right(1).wrapping_add(*b as u16))
}

/// Builds small exFAT images for tests: 512-byte clusters, folders created from the paths, and
/// optionally every file scattered across non-consecutive clusters behind a FAT chain.
#[cfg(test)]
pub mod build {
    use super::checksum;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Image {
        clusters: Vec<[u8; 512]>,
        fat: Vec<u32>,
    }

    impl Image {
        /// Store bytes; returns (first cluster, stored without a FAT chain).
        fn store(&mut self, bytes: &[u8], scatter: bool) -> (u32, bool) {
            let n = bytes.len().div_ceil(512).max(1);
            let mut ids: Vec<u32> = (0..n).map(|_| {
                self.clusters.push([0; 512]);
                self.clusters.len() as u32 + 1
            }).collect();
            if scatter {
                ids.reverse();
            }
            self.fat.resize(self.clusters.len() + 2, 0);
            for (k, c) in ids.iter().enumerate() {
                let chunk = &bytes[(k * 512).min(bytes.len())..((k + 1) * 512).min(bytes.len())];
                self.clusters[*c as usize - 2][..chunk.len()].copy_from_slice(chunk);
                self.fat[*c as usize] = ids.get(k + 1).copied().unwrap_or(0xFFFF_FFFF);
            }
            (ids[0], !scatter)
        }
    }

    fn entry_set(name: &str, directory: bool, first: u32, size: u64, contiguous: bool) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let names = units.len().div_ceil(15);
        let mut set = vec![0u8; 32 * (2 + names)];
        set[0] = 0x85;
        set[1] = (1 + names) as u8;
        set[4..6].copy_from_slice(&(if directory { 0x10u16 } else { 0x20 }).to_le_bytes());
        set[32] = 0xC0;
        set[33] = 1 | if contiguous { 2 } else { 0 };
        set[35] = units.len() as u8;
        set[40..48].copy_from_slice(&size.to_le_bytes());
        set[52..56].copy_from_slice(&first.to_le_bytes());
        set[56..64].copy_from_slice(&size.to_le_bytes());
        for (k, chunk) in units.chunks(15).enumerate() {
            let e = &mut set[64 + 32 * k..96 + 32 * k];
            e[0] = 0xC1;
            for (j, u) in chunk.iter().enumerate() {
                e[2 + 2 * j..4 + 2 * j].copy_from_slice(&u.to_le_bytes());
            }
        }
        let sum = checksum(&set);
        set[2..4].copy_from_slice(&sum.to_le_bytes());
        set
    }

    fn folder(img: &mut Image, path: &str, files: &[(&str, &[u8])], scatter: bool) -> (u32, u64) {
        let mut children: BTreeMap<&str, Option<&[u8]>> = BTreeMap::new();
        for (p, data) in files {
            let Some(rest) = (if path.is_empty() { Some(*p) } else { p.strip_prefix(path).and_then(|r| r.strip_prefix('/')) }) else { continue };
            match rest.split_once('/') {
                Some((dir, _)) => { children.entry(dir).or_insert(None); }
                None => { children.insert(rest, Some(data)); }
            }
        }
        // A deleted entry set first: readers must skip it.
        let mut data = entry_set("deleted.bin", false, 0, 0, true);
        for b in data.iter_mut().step_by(32) {
            *b &= 0x7F;
        }
        for (name, content) in children {
            let full = if path.is_empty() { name.to_string() } else { format!("{path}/{name}") };
            data.extend(match content {
                Some(bytes) => {
                    let (first, contiguous) = img.store(bytes, scatter);
                    entry_set(name, false, first, bytes.len() as u64, contiguous)
                }
                None => {
                    let (first, size) = folder(img, &full, files, scatter);
                    entry_set(name, true, first, size, false)
                }
            });
        }
        data.resize(data.len().div_ceil(512).max(1) * 512 + if path.is_empty() { 512 } else { 0 }, 0);
        let (first, _) = img.store(&data, false);
        (first, data.len() as u64)
    }

    pub fn image(files: &[(&str, &[u8])], scatter: bool) -> Vec<u8> {
        let mut img = Image::default();
        let (root, _) = folder(&mut img, "", files, scatter);
        let count = img.clusters.len() as u32;
        img.fat.resize(count as usize + 2, 0);
        img.fat[0] = 0xFFFF_FFF8;
        img.fat[1] = 0xFFFF_FFFF;
        let fat_sectors = (img.fat.len() * 4).div_ceil(512) as u32;
        let (fat_offset, heap_offset) = (24u32, 24 + fat_sectors);
        let mut out = vec![0u8; 512 * heap_offset as usize];
        let b = &mut out[..512];
        b[..3].copy_from_slice(&[0xEB, 0x76, 0x90]);
        b[3..11].copy_from_slice(b"EXFAT   ");
        b[72..80].copy_from_slice(&((heap_offset + count) as u64).to_le_bytes());
        b[80..84].copy_from_slice(&fat_offset.to_le_bytes());
        b[84..88].copy_from_slice(&fat_sectors.to_le_bytes());
        b[88..92].copy_from_slice(&heap_offset.to_le_bytes());
        b[92..96].copy_from_slice(&count.to_le_bytes());
        b[96..100].copy_from_slice(&root.to_le_bytes());
        b[104..106].copy_from_slice(&0x0100u16.to_le_bytes());
        b[108] = 9;
        b[109] = 0;
        b[110] = 1;
        b[510] = 0x55;
        b[511] = 0xAA;
        for (k, v) in img.fat.iter().enumerate() {
            let o = 512 * fat_offset as usize + 4 * k;
            out[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        for c in &img.clusters {
            out.extend_from_slice(c);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn open(bytes: &[u8]) -> (tempfile::NamedTempFile, Volume) {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        let v = Volume::open(File::open(f.path()).unwrap()).unwrap();
        (f, v)
    }

    fn contents(v: &Volume, n: &Node) -> Vec<u8> {
        let mut out = Vec::new();
        v.copy(n, &mut |c| { out.extend_from_slice(c); Ok(()) }).unwrap();
        out
    }

    #[test]
    fn lists_and_reads_contiguous_and_fragmented_files() {
        let big: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let long_name = "a file name longer than fifteen characters.bin";
        for scatter in [false, true] {
            let files: [(&str, &[u8]); 4] = [("game/sce_sys/param.json", b"{}"), ("game/eboot.bin", &big), ("game/empty", b""), (long_name, b"x")];
            let (_f, v) = open(&build::image(&files, scatter));
            let nodes = v.list(&|| Ok(())).unwrap();
            let mut paths: Vec<_> = nodes.iter().map(|n| (n.path.as_str(), n.directory)).collect();
            paths.sort();
            assert_eq!(paths, [(long_name, false), ("game", true), ("game/eboot.bin", false), ("game/empty", false), ("game/sce_sys", true), ("game/sce_sys/param.json", false)]);
            for (path, data) in files {
                let n = nodes.iter().find(|n| n.path == path).unwrap();
                assert_eq!(contents(&v, n), data, "{path} (scattered: {scatter})");
            }
        }
    }

    #[test]
    fn damaged_or_hostile_images_fail() {
        let good = build::image(&[("eboot.bin", b"x")], false);
        assert!(Volume::open(File::open("/dev/null").unwrap()).is_err());
        // Truncated download.
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(&good[..good.len() - 512]).unwrap();
        assert!(Volume::open(File::open(f.path()).unwrap()).unwrap_err().to_string().contains("shorter"));
        // A name with a path separator, and a corrupted entry set.
        let root_entries = |img: &mut Vec<u8>| {
            let heap = u32::from_le_bytes(img[88..92].try_into().unwrap()) as usize * 512;
            let root = u32::from_le_bytes(img[96..100].try_into().unwrap()) as usize;
            heap + (root - 2) * 512
        };
        let mut evil = build::image(&[("ab", b"x")], false);
        let at = root_entries(&mut evil) + 32 * 3; // after the deleted set: File, Stream, Name
        evil[at + 32 * 2 + 2] = b'/';
        let (_f, v) = open(&evil);
        assert!(v.list(&|| Ok(())).unwrap_err().to_string().contains("checksum"));
        let set = &mut evil[at..at + 96];
        let sum = checksum(set);
        set[2..4].copy_from_slice(&sum.to_le_bytes());
        let (_f, v) = open(&evil);
        assert!(v.list(&|| Ok(())).unwrap_err().to_string().contains("Unsafe file name"));
        // A file pointing outside the image.
        let mut far = build::image(&[("x", b"data")], false);
        let at = root_entries(&mut far) + 32 * 3;
        far[at + 52..at + 56].copy_from_slice(&0xFFFFu32.to_le_bytes());
        let set = &mut far[at..at + 96];
        let sum = checksum(set);
        set[2..4].copy_from_slice(&sum.to_le_bytes());
        let (_f, v) = open(&far);
        let n = v.list(&|| Ok(())).unwrap().into_iter().find(|n| n.path == "x").unwrap();
        assert!(v.copy(&n, &mut |_| Ok(())).unwrap_err().to_string().contains("outside"));
    }
}
