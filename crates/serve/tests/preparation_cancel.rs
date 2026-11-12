//! A browser generation whose owner is dropped stops within a small bounded
//! time and gives its capacity back, even when the bundle is as large as the
//! Delve's (about a thousand files and one ~40 MB wasm). The `/dungeon host`
//! stop-then-host-again path depends on it: at most two generations are
//! admitted, so a cancelled one that ran its hashing and compression to the
//! end kept the next host answering "browser generations busy" for seconds.
//!
//! `cancelled_generation_gives_back_its_capacity_promptly` is the regression
//! test. `bench_cancelled_generation_release` is the measurement: run it with
//! `ANGEL_CANCEL_BENCH_OUT=<file.json>` for the numbers (see its doc).

use angel_serve::http::{Assets, ClientBundle, PreparedAssets};
use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Admission capacity is process-wide, so one scenario at a time.
static SERIAL: Mutex<()> = Mutex::new(());

const WASM_BYTES: usize = 40 * 1024 * 1024;

/// Deterministic filler. `alphabet` is a power of two: 256 is incompressible
/// (images, sound), 16 shrinks about as slowly as a real wasm does.
fn filler(len: usize, seed: u64, alphabet: u64) -> Vec<u8> {
    let mut x = seed | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(((x >> 24) & (alphabet - 1)) as u8);
    }
    out
}

/// The Delve's shape: its wasm first, then about a thousand small files.
fn large_bundle() -> Arc<ClientBundle> {
    // The measurement can use the real Delve wasm (`ANGEL_CANCEL_BENCH_WASM`).
    let wasm = std::env::var_os("ANGEL_CANCEL_BENCH_WASM")
        .map(|path| std::fs::read(path).unwrap())
        .unwrap_or_else(|| filler(WASM_BYTES, 1, 16));
    let mut files = vec![("delve.wasm".to_string(), wasm, "application/wasm".to_string())];
    for i in 0..700u64 {
        files.push((format!("sprite-{i}.png"), filler(6 * 1024, 100 + i, 256), "image/png".into()));
    }
    for i in 0..250u64 {
        files.push((
            format!("module-{i}.js"),
            filler(16 * 1024, 2000 + i, 64),
            "text/javascript; charset=utf-8".into(),
        ));
    }
    for i in 0..50u64 {
        files.push((format!("track-{i}.ogg"), filler(64 * 1024, 5000 + i, 256), "audio/ogg".into()));
    }
    Arc::new(ClientBundle { index_html: "<script src=\"{{asset:delve.wasm}}\"></script>".into(), files })
}

fn placeholder() -> io::Result<ClientBundle> {
    Ok(ClientBundle::placeholder())
}

/// Admit a generation of the large bundle beside one already-resident
/// generation (capacity is two), drop its owner once its worker is inside
/// preparation, and return the time until a new generation is admitted.
fn release_after_owner_drop(bundle: &Arc<ClientBundle>) -> Duration {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let resident = PreparedAssets::start(|_| placeholder()).unwrap();
    let (captured, in_preparation) = mpsc::channel();
    let source = bundle.clone();
    let victim = PreparedAssets::start(move |_| {
        let owned = (*source).clone();
        captured.send(()).unwrap();
        Ok(owned)
    })
    .unwrap();
    assert_eq!(
        PreparedAssets::start(|_| placeholder()).unwrap_err().kind(),
        io::ErrorKind::WouldBlock,
        "capacity is two generations"
    );
    in_preparation.recv().unwrap();
    // The worker has its bytes and is hashing the wasm.
    std::thread::sleep(Duration::from_millis(40));
    let dropped = Instant::now();
    drop(victim);
    let released = loop {
        match PreparedAssets::start(|_| placeholder()) {
            Ok(generation) => {
                let at = dropped.elapsed();
                drop(generation);
                break at;
            }
            Err(error) => {
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
                assert!(dropped.elapsed() < Duration::from_secs(120), "the cancelled generation never released");
                std::thread::sleep(Duration::from_micros(200));
            }
        }
    };
    drop(resident);
    // Both slots are free again once the retained placeholders retire.
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let a = PreparedAssets::start(|_| placeholder());
        let b = PreparedAssets::start(|_| placeholder());
        if a.is_ok() && b.is_ok() {
            break;
        }
        assert!(Instant::now() < until, "capacity leaked");
        std::thread::sleep(Duration::from_millis(1));
    }
    released
}

#[test]
fn cancelled_generation_gives_back_its_capacity_promptly() {
    let bundle = large_bundle();
    let started = Instant::now();
    Assets::prepare(&bundle).unwrap();
    let full = started.elapsed();
    let released = release_after_owner_drop(&bundle);
    eprintln!("full preparation {full:?}; release after owner drop {released:?}");
    // Not an absolute bound (a loaded box schedules late): the cancelled
    // generation must stop far short of finishing its work.
    assert!(
        released * 4 < full,
        "released after {released:?}, while the whole preparation takes {full:?}"
    );
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// `ANGEL_CANCEL_BENCH_OUT=/path/out.json ANGEL_CANCEL_BENCH_LABEL=before \
///  ANGEL_CANCEL_BENCH_WASM=/path/to/delve.wasm \
///  cargo test --profile fast -p angel-serve --test preparation_cancel \
///  bench_cancelled_generation_release -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check"]
fn bench_cancelled_generation_release() {
    let runs: usize = std::env::var("ANGEL_CANCEL_BENCH_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(9);
    let bundle = large_bundle();
    let mut full_ms = Vec::new();
    let mut release_ms = Vec::new();
    for _ in 0..runs {
        let started = Instant::now();
        Assets::prepare(&bundle).unwrap();
        full_ms.push(started.elapsed().as_secs_f64() * 1e3);
        release_ms.push(release_after_owner_drop(&bundle).as_secs_f64() * 1e3);
    }
    let summary = |name: &str, samples: &[f64]| {
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let list: Vec<String> = samples.iter().map(|v| format!("{v:.1}")).collect();
        format!(
            "\"{name}\": {{\"samples_ms\": [{}], \"median_ms\": {:.1}, \"p90_ms\": {:.1}, \"min_ms\": {:.1}, \"max_ms\": {:.1}}}",
            list.join(", "),
            percentile(&sorted, 0.5),
            percentile(&sorted, 0.9),
            sorted[0],
            sorted[sorted.len() - 1],
        )
    };
    let json = format!(
        "{{\n  \"label\": \"{}\",\n  \"what\": \"time from dropping the owner of a generation mid-preparation (large Delve-shaped bundle: {} files, one {} MiB wasm, {}) until a new PreparedAssets::start is admitted, with the other of the two slots resident\",\n  \"runs\": {runs},\n  {},\n  {}\n}}\n",
        std::env::var("ANGEL_CANCEL_BENCH_LABEL").unwrap_or_default(),
        bundle.files.len(),
        bundle.files[0].1.len() >> 20,
        if std::env::var_os("ANGEL_CANCEL_BENCH_WASM").is_some() { "the real delve.wasm" } else { "a synthetic wasm" },
        summary("release_after_owner_drop", &release_ms),
        summary("uncancelled_full_preparation", &full_ms),
    );
    println!("{json}");
    if let Some(path) = std::env::var_os("ANGEL_CANCEL_BENCH_OUT") {
        std::fs::write(path, json).unwrap();
    }
}
