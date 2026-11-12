//! Built chunks of the world, kept between frames: a frame only gathers them.
use crate::render::world3d::mesh::Tri;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Key { pub course: u32, pub kind: u8, pub a: i32, pub b: i32, pub lod: u8 }

pub type Tris = Arc<[Tri]>;

struct Entry { tris: Tris, used: u64 }
#[derive(Default)]
struct Cache { map: HashMap<Key, Entry>, clock: u64 }

/// Entries kept before the least recently used half is dropped.
const KEEP: usize = 1400;

fn cache() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The chunk at `key`, built by `make` the first time it is asked for.
pub fn get(key: Key, make: impl FnOnce() -> Vec<Tri>) -> Tris {
    {
        let mut c = cache().lock().unwrap();
        c.clock += 1;
        let now = c.clock;
        if let Some(e) = c.map.get_mut(&key) {
            e.used = now;
            return e.tris.clone();
        }
    }
    #[cfg(test)]
    misses().lock().unwrap().entry(key.course).and_modify(|n| *n += 1).or_insert(1);
    // Built outside the lock: a chunk can take a while and its maker may ask
    // the cache for other chunks.
    let tris: Tris = make().into();
    let mut c = cache().lock().unwrap();
    c.clock += 1;
    let now = c.clock;
    if c.map.len() >= KEEP {
        let mut by_age: Vec<_> = c.map.iter().map(|(k, e)| (e.used, *k)).collect();
        by_age.sort();
        for (_, k) in by_age.into_iter().take(KEEP / 2) { c.map.remove(&k); }
    }
    c.map.insert(key, Entry { tris: tris.clone(), used: now });
    tris
}

/// A fact about a chunk that is slow to find and never changes.
pub fn flag(course: u32, a: i32, b: i32, make: impl FnOnce() -> bool) -> bool {
    static F: OnceLock<Mutex<HashMap<(u32, i32, i32), bool>>> = OnceLock::new();
    if let Some(&v) = F.get_or_init(Default::default).lock().unwrap().get(&(course, a, b)) { return v; }
    let v = make();
    F.get_or_init(Default::default).lock().unwrap().insert((course, a, b), v);
    v
}

#[cfg(test)]
fn misses() -> &'static Mutex<HashMap<u32, usize>> {
    static M: OnceLock<Mutex<HashMap<u32, usize>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

/// How many chunks have ever been built for `course`, whatever has since been dropped.
#[cfg(test)]
pub fn built_for(course: u32) -> usize { misses().lock().unwrap().get(&course).copied().unwrap_or(0) }
