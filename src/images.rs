//! Image loading: fetch → decode → resize → cache, on a priority thread pool.
//!
//! Two disk caches under ~/.cache/ps5-launcher:
//!   img/<sha1[:2]>/<sha1(url)>   original downloads (same layout as v1)
//!   thumbs/<sha1(key)>.qoi        decoded + resized pixels; QOI decodes ~10x faster than PNG
//! A warm start therefore never touches the network or a JPEG/PNG decoder.

use crate::util::{cache_dir, http_get, sha1_hex};
use fast_image_resize as fr;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Src {
    Url(String),
    File(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Job {
    pub key: String,
    pub src: Src,
    pub max_w: u32,
    /// Fraction of the height to cut from the top (removes the PS5 banner on site covers).
    pub crop_top: f32,
    pub prio: u8,
    seq: u64,
}

impl PartialEq for Job {
    fn eq(&self, o: &Self) -> bool {
        self.prio == o.prio && self.seq == o.seq
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Job {
    // Highest priority first; newest request first within a priority (what the user looks at now).
    fn cmp(&self, o: &Self) -> Ordering {
        self.prio.cmp(&o.prio).then(self.seq.cmp(&o.seq))
    }
}

pub mod prio {
    pub const PREFETCH: u8 = 1;
    pub const CARD: u8 = 3;
    pub const TILE: u8 = 5;
    pub const DETAIL: u8 = 6;
    pub const HERO: u8 = 8;
}

struct Queue {
    heap: BinaryHeap<Job>,
    queued: HashSet<String>,
    seq: u64,
}

type Done = Arc<dyn Fn(String, Option<SharedPixelBuffer<Rgba8Pixel>>) + Send + Sync>;

#[derive(Clone)]
pub struct Pool {
    q: Arc<(Mutex<Queue>, Condvar)>,
}

impl Pool {
    pub fn new(threads: usize, done: Done) -> Pool {
        let q = Arc::new((Mutex::new(Queue { heap: BinaryHeap::new(), queued: HashSet::new(), seq: 0 }), Condvar::new()));
        for i in 0..threads {
            let q = q.clone();
            let done = done.clone();
            std::thread::Builder::new()
                .name(format!("img-{i}"))
                .spawn(move || loop {
                    let job = {
                        let (lock, cv) = &*q;
                        let mut st = lock.lock().unwrap();
                        loop {
                            if let Some(j) = st.heap.pop() {
                                break j;
                            }
                            st = cv.wait(st).unwrap();
                        }
                    };
                    let result = std::panic::catch_unwind(|| load(&job)).ok().flatten();
                    q.0.lock().unwrap().queued.remove(&job.key);
                    done(job.key, result);
                })
                .ok();
        }
        Pool { q }
    }

    pub fn request(&self, key: String, src: Src, max_w: u32, crop_top: f32, prio: u8) {
        let (lock, cv) = &*self.q;
        let mut st = lock.lock().unwrap();
        if !st.queued.insert(key.clone()) {
            // Already queued: bump its priority by re-inserting a newer copy.
            let old: Vec<Job> = st.heap.drain().collect();
            st.seq += 1;
            let seq = st.seq;
            for mut j in old {
                if j.key == key {
                    j.prio = j.prio.max(prio);
                    j.seq = seq;
                }
                st.heap.push(j);
            }
            return;
        }
        st.seq += 1;
        let seq = st.seq;
        st.heap.push(Job { key, src, max_w, crop_top, prio, seq });
        cv.notify_one();
    }

    /// Drop queued (not yet started) jobs that are no longer wanted.
    pub fn retain(&self, keep: impl Fn(&Job) -> bool) -> Vec<String> {
        let mut st = self.q.0.lock().unwrap();
        let old: Vec<Job> = st.heap.drain().collect();
        let mut dropped = Vec::new();
        for j in old {
            if keep(&j) {
                st.heap.push(j);
            } else {
                st.queued.remove(&j.key);
                dropped.push(j.key);
            }
        }
        dropped
    }
}

fn raw_cache_path(url: &str) -> PathBuf {
    let h = sha1_hex(url);
    cache_dir().join("img").join(&h[..2]).join(h)
}

fn thumb_path(key: &str) -> PathBuf {
    cache_dir().join("thumbs").join(format!("{}.qoi", sha1_hex(key)))
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    let path = raw_cache_path(url);
    if let Ok(b) = std::fs::read(&path) {
        return Some(b);
    }
    let mut candidates = vec![url.to_string()];
    // YouTube: maxres art is missing for some videos; fall back gracefully.
    if url.contains("i.ytimg.com") && url.ends_with("/maxresdefault.jpg") {
        let base = url.rsplit_once('/').map(|x| x.0).unwrap_or(url);
        candidates.push(format!("{base}/sddefault.jpg"));
        candidates.push(format!("{base}/hqdefault.jpg"));
    }
    for c in candidates {
        if let Ok(data) = http_get(&c) {
            // ytimg serves a tiny grey placeholder for missing art.
            if !data.is_empty() && (data.len() > 1500 || !c.contains("i.ytimg.com")) {
                let _ = crate::util::atomic_write(&path, &data);
                return Some(data);
            }
        }
    }
    None
}

fn load(job: &Job) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let tp = thumb_path(&job.key);
    if let Ok(bytes) = std::fs::read(&tp) {
        if let Ok((h, px)) = qoi::decode_to_vec(&bytes) {
            if px.len() == (h.width * h.height * 4) as usize {
                return Some(SharedPixelBuffer::clone_from_slice(&px, h.width, h.height));
            }
        }
    }
    let bytes = match &job.src {
        Src::Url(u) => fetch(u)?,
        Src::File(p) => std::fs::read(p).ok()?,
    };
    let mut img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    if job.crop_top > 0.0 {
        let cut = (img.height() as f32 * job.crop_top) as u32;
        img = image::imageops::crop_imm(&img, 0, cut, img.width(), img.height() - cut).to_image();
    }
    let (w, h) = img.dimensions();
    let (px, w, h) = if job.max_w > 0 && w > job.max_w {
        let nh = ((h as u64 * job.max_w as u64) / w as u64).max(1) as u32;
        let src = fr::images::Image::from_vec_u8(w, h, img.into_raw(), fr::PixelType::U8x4).ok()?;
        let mut dst = fr::images::Image::new(job.max_w, nh, fr::PixelType::U8x4);
        let opts = fr::ResizeOptions::new().resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::CatmullRom));
        fr::Resizer::new().resize(&src, &mut dst, &opts).ok()?;
        (dst.into_vec(), job.max_w, nh)
    } else {
        (img.into_raw(), w, h)
    };
    if let Ok(enc) = qoi::encode_to_vec(&px, w, h) {
        let _ = crate::util::atomic_write(&tp, &enc);
    }
    Some(SharedPixelBuffer::clone_from_slice(&px, w, h))
}

/// UI-thread side: decoded images by key with an LRU byte budget.
pub struct Store {
    map: HashMap<String, (Image, usize, u64)>,
    pending: HashSet<String>,
    failed: HashSet<String>,
    total: usize,
    tick: u64,
    budget: usize,
    pub pool: Pool,
}

impl Store {
    pub fn new(pool: Pool, budget_mb: usize) -> Store {
        Store { map: HashMap::new(), pending: HashSet::new(), failed: HashSet::new(), total: 0, tick: 0, budget: budget_mb << 20, pool }
    }

    pub fn get(&mut self, key: &str) -> Option<Image> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(key).map(|e| {
            e.2 = t;
            e.0.clone()
        })
    }

    /// Returns the image if ready, otherwise queues it and returns None.
    pub fn want(&mut self, key: &str, src: Src, max_w: u32, crop_top: f32, prio: u8) -> Option<Image> {
        if let Some(i) = self.get(key) {
            return Some(i);
        }
        if !self.failed.contains(key) {
            self.pending.insert(key.to_string());
            self.pool.request(key.to_string(), src, max_w, crop_top, prio);
        }
        None
    }

    pub fn is_pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    pub fn insert(&mut self, key: String, buf: Option<SharedPixelBuffer<Rgba8Pixel>>) {
        self.pending.remove(&key);
        let Some(buf) = buf else {
            self.failed.insert(key);
            return;
        };
        let bytes = (buf.width() * buf.height() * 4) as usize;
        self.tick += 1;
        if let Some(old) = self.map.insert(key, (Image::from_rgba8(buf), bytes, self.tick)) {
            self.total -= old.1;
        }
        self.total += bytes;
        if self.total > self.budget {
            let mut entries: Vec<(u64, String)> = self.map.iter().map(|(k, v)| (v.2, k.clone())).collect();
            entries.sort_unstable();
            for (_, k) in entries {
                if self.total <= self.budget * 8 / 10 {
                    break;
                }
                if let Some(e) = self.map.remove(&k) {
                    self.total -= e.1;
                }
            }
        }
    }

    /// Forget queued-but-unstarted requests the UI no longer needs.
    pub fn cancel_unless(&mut self, keep: impl Fn(&Job) -> bool) {
        for k in self.pool.retain(keep) {
            self.pending.remove(&k);
        }
    }

    pub fn clear_failures(&mut self) {
        self.failed.clear();
    }
}
