//! A counting allocator for tests. One per crate, so the ignored step
//! census and the stacked-wishes bench share it. Production builds do not
//! include this module.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

std::thread_local! {
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static ASKED: Cell<u64> = const { Cell::new(0) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

pub struct Counting;

fn note(grow: i64, asked: u64) {
    let _ = LIVE.try_with(|l| l.set(l.get() + grow));
    if asked > 0 {
        let _ = ASKED.try_with(|a| a.set(a.get() + asked));
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        if CENSUS.load(Ordering::Relaxed) {
            record(asked);
        }
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size() as i64, layout.size() as u64);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size() as i64, layout.size() as u64);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note(-(layout.size() as i64), 0);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        note(new as i64 - layout.size() as i64, new as u64);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

/// (live bytes, bytes asked for, allocations) on this thread so far.
pub fn now() -> (i64, u64, u64) {
    (LIVE.with(Cell::get), ASKED.with(Cell::get), COUNT.with(Cell::get))
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Size census for the ignored step bench. Off unless a test asks, so the
/// other tests keep the allocator they already had.
static CENSUS: AtomicBool = AtomicBool::new(false);
const BUCKETS: usize = 28;
static BUCKET_COUNT: [AtomicU64; BUCKETS] = [const { AtomicU64::new(0) }; BUCKETS];
static BUCKET_BYTES: [AtomicU64; BUCKETS] = [const { AtomicU64::new(0) }; BUCKETS];

const LARGE: usize = 48;
struct Large {
    size: AtomicUsize,
    count: AtomicU64,
    bytes: AtomicU64,
}
static LARGE_SIZES: [Large; LARGE] = [const {
    Large { size: AtomicUsize::new(0), count: AtomicU64::new(0), bytes: AtomicU64::new(0) }
}; LARGE];

fn record(asked: u64) {
    let mut shift = 0u32;
    let mut n = asked;
    while n > 1 {
        n >>= 1;
        shift += 1;
    }
    let i = (shift as usize).min(BUCKETS - 1);
    BUCKET_COUNT[i].fetch_add(1, Ordering::Relaxed);
    BUCKET_BYTES[i].fetch_add(asked, Ordering::Relaxed);
    if asked < 256 {
        return;
    }
    let size = asked as usize;
    for slot in &LARGE_SIZES {
        let cur = slot.size.load(Ordering::Relaxed);
        if cur == size {
            slot.count.fetch_add(1, Ordering::Relaxed);
            slot.bytes.fetch_add(asked, Ordering::Relaxed);
            return;
        }
        if cur == 0
            && slot
                .size
                .compare_exchange(0, size, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            slot.count.fetch_add(1, Ordering::Relaxed);
            slot.bytes.fetch_add(asked, Ordering::Relaxed);
            return;
        }
    }
}

pub fn census(on: bool) {
    CENSUS.store(on, Ordering::Relaxed);
}

pub fn reset_census() {
    for c in &BUCKET_COUNT {
        c.store(0, Ordering::Relaxed);
    }
    for b in &BUCKET_BYTES {
        b.store(0, Ordering::Relaxed);
    }
    for slot in &LARGE_SIZES {
        slot.size.store(0, Ordering::Relaxed);
        slot.count.store(0, Ordering::Relaxed);
        slot.bytes.store(0, Ordering::Relaxed);
    }
}

/// Power-of-two buckets: (lo, count, bytes), then exact sizes at or above 256.
pub fn census_report() -> (Vec<(u64, u64, u64)>, Vec<(usize, u64, u64)>) {
    let mut buckets = Vec::new();
    for i in 0..BUCKETS {
        let count = BUCKET_COUNT[i].load(Ordering::Relaxed);
        if count == 0 {
            continue;
        }
        let lo = if i == 0 { 0 } else { 1u64 << (i - 1) };
        buckets.push((lo, count, BUCKET_BYTES[i].load(Ordering::Relaxed)));
    }
    let mut large = Vec::new();
    for slot in &LARGE_SIZES {
        let size = slot.size.load(Ordering::Relaxed);
        let count = slot.count.load(Ordering::Relaxed);
        if size == 0 || count == 0 {
            continue;
        }
        large.push((size, count, slot.bytes.load(Ordering::Relaxed)));
    }
    large.sort_by(|a, b| b.2.cmp(&a.2));
    (buckets, large)
}
