//! Board A races for docs/RENDER_SCORECARD.md, glass side: the exact
//! streams `scripts/bench/render/kitty_gfx_bench.py` pushes through
//! Kitty's protocol, here through `Host::upload_picture` + layers.
//!
//! STREAM_BENCH_MODE=blast4|big|placements|text (default blast4)
//! STREAM_BENCH_SECONDS=14  STREAM_BENCH_FPS=120 (0 = free-running)
//! STREAM_BENCH_W/H (big), STREAM_BENCH_N (placements)
//! Pair with ANGEL_GLASS_STATS=1 (render percentiles + parts) and, for
//! ingest runs, ANGEL_GLASS_PRESENT=immediate. Pixel rings are
//! pre-generated with the same formula as the Kitty bench, and every
//! frame differs so no upload can be skipped as unchanged.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode};
use ratatui_core::style::Style;
use ratatui_core::terminal::Terminal;

use angel_glass::graphics::Picture;
use angel_glass::{LAYER_PICTURES, Layer, LayerKind, Placement};

/// (id, w, h, z, cols, rows, row, col): the blast's four streams, as in
/// the Kitty bench.
const BLAST4: [(u32, u32, u32, i32, f32, f32, f32, f32); 4] = [
    (11, 320, 200, -1, 40.0, 12.0, 1.0, 1.0),
    (12, 192, 160, 0, 24.0, 10.0, 3.0, 19.0),
    (13, 160, 160, 1, 20.0, 10.0, 5.0, 39.0),
    (14, 480, 140, 2, 60.0, 8.0, 11.0, 4.0),
];
const GROUP: u32 = 0x57EA_11BE;

fn env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The M8 golden scene as straight RGBA: a red alpha ramp on the
/// left half, opaque green/blue checker on the right. One
/// generation rule shared with scripts/bench/render/golden_check.py,
/// which asserts the composited result (sRGB-space blending) from a
/// captured frame.
fn golden_bytes(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let half = w / 2;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if x < half {
                buf[i] = 255;
                buf[i + 3] = (u64::from(x) * 255 / u64::from(half - 1)) as u8;
            } else if (x / 16 + y / 16) % 2 == 1 {
                buf[i + 1] = 220;
                buf[i + 3] = 255;
            } else {
                buf[i + 2] = 220;
                buf[i + 3] = 255;
            }
        }
    }
    buf
}

fn frame_bytes(w: u32, h: u32, n: u32, seed: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let g = ((y + n * 5) & 255) as u8;
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            buf[i] = ((x + n * 3 + seed) & 255) as u8;
            buf[i + 1] = g;
            buf[i + 2] = ((x ^ y ^ n) & 255) as u8;
            buf[i + 3] = 255;
        }
    }
    buf
}

fn percentile(sorted: &[Duration], p: usize) -> f64 {
    sorted
        .get((sorted.len().saturating_sub(1)) * p / 100)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

struct Streams {
    specs: Vec<(u32, u32, u32, i32, f32, f32, f32, f32)>,
    ring: Vec<Vec<Vec<u8>>>,
    payload: usize,
    /// Photon mode: every pixel is the frame sequence as colour.
    solid_stamp: bool,
}

impl Streams {
    fn new(specs: Vec<(u32, u32, u32, i32, f32, f32, f32, f32)>) -> Self {
        let payload = specs.iter().map(|s| (s.1 * s.2 * 4) as usize).sum();
        let ring = specs
            .iter()
            .map(|s| (0..4).map(|v| frame_bytes(s.1, s.2, v, s.0)).collect())
            .collect();
        Self {
            specs,
            ring,
            payload,
            solid_stamp: false,
        }
    }

    fn push(&self, host: &angel_glass::Host, n: usize) {
        let mut layers = Vec::with_capacity(self.specs.len());
        for (i, s) in self.specs.iter().enumerate() {
            let id = LAYER_PICTURES + s.0;
            // The frame sequence as a 16x16 block of colour at the
            // picture's origin: an outside observer (framecount) reads
            // which produced frame reached the screen, and when. A
            // block, not one pixel, so scaling filters cannot blend it.
            let mut rgba = self.ring[i][n % self.ring[i].len()].clone();
            let stamp = (n as u32).to_le_bytes();
            if self.solid_stamp {
                for px in rgba.chunks_exact_mut(4) {
                    px[0..3].copy_from_slice(&stamp[0..3]);
                    px[3] = 255;
                }
            } else {
                for y in 0..16.min(s.2) {
                    for x in 0..16.min(s.1) {
                        let at = ((y * s.1 + x) * 4) as usize;
                        rgba[at..at + 3].copy_from_slice(&stamp[0..3]);
                        rgba[at + 3] = 255;
                    }
                }
            }
            host.upload_picture(Picture {
                id,
                width: s.1,
                height: s.2,
                rgba,
                cols: 0,
                rows: 0,
            });
            layers.push(Layer::new(
                u64::from(s.0),
                s.3,
                Placement::Cells([s.7, s.6, s.4, s.5]),
                LayerKind::Picture(id),
            ));
        }
        host.set_layer_group(GROUP, layers);
    }
}

/// Adversarial: `count` fresh 16x16 pictures per frame, ids cycling a
/// 4096 pool — texture-allocation churn against the upload cache, the
/// mirror of the Kitty bench's churn mode.
fn push_churn(host: &angel_glass::Host, img: &[u8], count: usize, seq: &mut u32) {
    let mut layers = Vec::with_capacity(count);
    for k in 0..count {
        let id = LAYER_PICTURES + 1000 + (*seq % 4096);
        *seq = seq.wrapping_add(1);
        host.upload_picture(Picture {
            id,
            width: 16,
            height: 16,
            rgba: img.to_vec(),
            cols: 0,
            rows: 0,
        });
        let col = (k % 50) as f32 * 2.0;
        let row = (k / 50) as f32;
        layers.push(Layer::new(
            u64::from(*seq),
            0,
            Placement::Cells([col, row, 2.0, 1.0]),
            LayerKind::Picture(id),
        ));
    }
    host.set_layer_group(GROUP, layers);
}

fn push_placements(host: &angel_glass::Host, count: usize, n: usize, cols: f32, rows: f32) {
    let id = LAYER_PICTURES + 21;
    let mut layers = Vec::with_capacity(count);
    let span = (cols - 4.0).max(1.0) as usize;
    for i in 0..count {
        let col = ((i * 4 + n) % span) as f32;
        let row = (((i * 4 + n) / span) % (rows - 2.0).max(1.0) as usize) as f32;
        layers.push(Layer::new(
            i as u64,
            0,
            Placement::Cells([col, row, 4.0, 2.0]),
            LayerKind::PicturePart(id, [0.0, 0.0, 64.0, 64.0]),
        ));
    }
    host.set_layer_group(GROUP, layers);
}

fn main() {
    let mode = env("STREAM_BENCH_MODE", "blast4".to_string());
    let seconds = env("STREAM_BENCH_SECONDS", 14u64);
    let fps = env("STREAM_BENCH_FPS", 120u32);
    let result = angel_glass::run(angel_glass::Options::from_environment(), move |host| {
        let mut terminal = Terminal::new(host.backend()).expect("terminal");
        let mut input = host.input_reader().expect("input");
        let streams = match mode.as_str() {
            "blast4" => Some(Streams::new(BLAST4.to_vec())),
            "stamp" => {
                let mut s = Streams::new(vec![(41, 320, 200, 0, 40.0, 12.0, 1.0, 1.0)]);
                s.solid_stamp = true;
                Some(s)
            }
            "golden" => {
                let mut s = Streams::new(vec![(51, 320, 200, 0, 40.0, 12.0, 1.0, 1.0)]);
                s.ring = vec![vec![golden_bytes(320, 200)]];
                Some(s)
            }
            "big" => Some(Streams::new(vec![(
                31,
                env("STREAM_BENCH_W", 1920),
                env("STREAM_BENCH_H", 1080),
                0,
                100.0,
                30.0,
                1.0,
                1.0,
            )])),
            "placements" => {
                host.upload_picture(Picture {
                    id: LAYER_PICTURES + 21,
                    width: 64,
                    height: 64,
                    rgba: frame_bytes(64, 64, 0, 21),
                    cols: 0,
                    rows: 0,
                });
                None
            }
            _ => None,
        };
        let payload = streams.as_ref().map_or(0, |s| s.payload);
        let count = env("STREAM_BENCH_N", 1000usize);
        let churn_img = frame_bytes(16, 16, 1, 7);
        let mut churn_seq = 0u32;
        let plain = Style::default();
        let alphabet = "abcdefghij0123456789";
        let started = Instant::now();
        let mut produced: Vec<Duration> = Vec::new();
        let mut n = 0usize;
        let mut stamp_log = std::env::var("STREAM_BENCH_STAMP_LOG")
            .ok()
            .map(|path| std::io::LineWriter::new(std::fs::File::create(path).expect("stamp log")));
        while started.elapsed() < Duration::from_secs(seconds) {
            let began = Instant::now();
            if let Some(log) = stamp_log.as_mut() {
                use std::io::Write as _;
                let epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("epoch")
                    .as_secs_f64()
                    * 1000.0;
                let _ = writeln!(log, "{n} {epoch:.1}");
            }
            match mode.as_str() {
                "placements" => {
                    let grid = host.grid_size();
                    push_placements(
                        &host,
                        count,
                        n,
                        f32::from(grid.cols),
                        f32::from(grid.rows),
                    );
                }
                "text" => {}
                "churn" => push_churn(&host, &churn_img, count, &mut churn_seq),
                _ => streams.as_ref().expect("streams").push(&host, n),
            }
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    if mode == "text" {
                        for r in 0..area.height {
                            let rot = (n + r as usize) % alphabet.len();
                            let unit = format!(
                                "{}{}",
                                &alphabet[rot..],
                                &alphabet[..rot]
                            );
                            let line: String = unit
                                .repeat(area.width as usize / alphabet.len() + 1)
                                .chars()
                                .take(area.width as usize)
                                .collect();
                            frame.buffer_mut().set_string(0, r, line, plain);
                        }
                    } else {
                        frame.buffer_mut().set_string(
                            0,
                            0,
                            format!("stream-bench {mode} frame {n}"),
                            plain,
                        );
                    }
                })
                .expect("draw");
            if n == 30
                && let Ok(path) = std::env::var("STREAM_BENCH_CAPTURE")
            {
                host.capture(path);
            }
            produced.push(began.elapsed());
            n += 1;
            if let Ok(Some(Event::Key(key))) = input(Duration::ZERO)
                && key.code == KeyCode::Char('q')
            {
                break;
            }
            if fps > 0 {
                let target = Duration::from_secs_f64(n as f64 / f64::from(fps));
                let elapsed = started.elapsed();
                if target > elapsed {
                    std::thread::sleep(target - elapsed);
                }
            }
        }
        // Raw produce series, when asked (bench analysis wants the
        // series, not someone else's percentiles).
        if let Ok(path) = std::env::var("STREAM_BENCH_RAW") {
            let all: Vec<String> = produced
                .iter()
                .skip(30)
                .map(|d| format!("{:.3}", d.as_secs_f64() * 1000.0))
                .collect();
            let _ = std::fs::write(&path, format!("[{}]\n", all.join(",")));
        }
        // Drop the first 30 frames (warm-up), like the Kitty bench.
        let mut times: Vec<Duration> = produced.into_iter().skip(30).collect();
        times.sort_unstable();
        let grid = host.grid_size();
        eprintln!(
            "STREAM_BENCH mode={mode} frames={} cols={} rows={} payload_bytes={payload} produce_p50_ms={:.3} produce_p95_ms={:.3}",
            times.len(),
            grid.cols,
            grid.rows,
            percentile(&times, 50),
            percentile(&times, 95),
        );
    });
    if let Err(error) = result {
        eprintln!("stream-bench: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cross-language constant: `kitty_gfx_bench.frame_bytes(4,2,1,7)`
    /// asserts the same bytes, so both harnesses race identical pixels.
    #[test]
    fn frame_formula_matches_the_kitty_bench() {
        assert_eq!(
            frame_bytes(4, 2, 1, 7),
            vec![
                10, 5, 1, 255, 11, 5, 0, 255, 12, 5, 3, 255, 13, 5, 2, 255, 10, 6, 0, 255, 11, 6, 1,
                255, 12, 6, 2, 255, 13, 6, 3, 255,
            ]
        );
        // Frames must differ: a renderer may skip an unchanged upload.
        assert_ne!(frame_bytes(8, 8, 0, 1), frame_bytes(8, 8, 1, 1));
    }

    /// The golden generation rule golden_check.py asserts against:
    /// alpha ramps 0..=255 across the left half, checker is exact.
    #[test]
    fn golden_scene_formula() {
        let px = |buf: &[u8], x: u32, y: u32| {
            let i = ((y * 320 + x) * 4) as usize;
            [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
        };
        let buf = golden_bytes(320, 200);
        assert_eq!(px(&buf, 0, 100), [255, 0, 0, 0]);
        assert_eq!(px(&buf, 159, 100), [255, 0, 0, 255]);
        assert_eq!(px(&buf, 80, 100)[3], 128);
        assert_eq!(px(&buf, 160, 0), [0, 0, 220, 255]);
        assert_eq!(px(&buf, 176, 0), [0, 220, 0, 255]);
        assert_eq!(px(&buf, 160, 16), [0, 220, 0, 255]);
    }

    #[test]
    fn percentiles_are_order_statistics() {
        let times: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        assert!((percentile(&times, 50) - 50.0).abs() < 1.5);
        assert!((percentile(&times, 95) - 95.0).abs() < 1.5);
        assert_eq!(percentile(&[], 50), 0.0);
    }
}
