//! Headless check for the page raster service (`angel_glass::page`):
//! how long a document page takes to rasterize, cold (glyphs scaled)
//! and warm (memoized, the scroll case).
//!
//! PAGE_BENCH_LINES=48 PAGE_BENCH_COLS=96 cargo run --release --example page-bench

use std::time::Instant;

use angel_glass::page::{Pager, PLAIN};

fn env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
    let rows: usize = env("PAGE_BENCH_LINES", 48);
    let cols: usize = env("PAGE_BENCH_COLS", 96);
    let height: u32 = env("PAGE_BENCH_CELL_H", 23);
    let words = "the quick brown fox jumps over the lazy dog 0123456789 ";
    let lines: Vec<String> = (0..rows)
        .map(|r| {
            let rot = r % words.len();
            format!("{}{}", &words[rot..], &words[..rot])
                .repeat(cols / words.len() + 1)[..cols]
                .to_string()
        })
        .collect();
    let t0 = Instant::now();
    let mut pager = Pager::load("monospace", height).expect("pager");
    println!("load_ms={:.2} cell={:?}", t0.elapsed().as_secs_f64() * 1000.0, pager.cell());
    let render = |pager: &mut Pager| {
        let t = Instant::now();
        let page = pager.render(&lines, PLAIN, [230, 220, 200], [12, 10, 8, 255]).expect("render");
        (t.elapsed().as_secs_f64() * 1000.0, page.0, page.1)
    };
    let (cold, w, h) = render(&mut pager);
    let mut warm = Vec::new();
    for _ in 0..50 {
        warm.push(render(&mut pager).0);
    }
    warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "PAGE_BENCH lines={rows} cols={cols} px={w}x{h} cold_ms={cold:.2} warm_p50_ms={:.2} warm_p95_ms={:.2}",
        warm[warm.len() / 2],
        warm[warm.len() * 95 / 100]
    );
}
