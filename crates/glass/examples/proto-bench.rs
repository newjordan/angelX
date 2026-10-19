//! The glass side of the Kitty-protocol race, CPU only: how long glass
//! takes to ingest one blast-sized frame (the same four streams
//! `kitty-blast.py` pushes into Kitty: 320x200, 192x160, 160x160,
//! 480x140). GPU upload and present are not in this number; Kitty's
//! query-synced round trip is. Compare like with like and say so.
//!
//! Glass no longer parses escape codes (every picture reaches the window
//! as its own pixels, `Host::upload_picture`), so the ingest measured is
//! what replaced the parser and the upload store: taking each stream's
//! pixels as a `Picture`, the well-formed check, and the premultiply the
//! GPU side stores them with. `png` adds the producer's decode, which now
//! runs on the producer's worker instead of in glass.
//!
//! cargo run --release --example proto-bench [frames] [png|raw]

use angel_glass::graphics::{Picture, premultiply_in_place, well_formed};
use std::time::Instant;

const STREAMS: [(u32, u32, u32); 4] = [
    (11, 320, 200),
    (12, 192, 160),
    (13, 160, 160),
    (14, 480, 140),
];

fn frame_bytes(w: u32, h: u32, n: u32, seed: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            buf[i] = (x.wrapping_add(n * 3).wrapping_add(seed) & 255) as u8;
            buf[i + 1] = ((y + n * 5) & 255) as u8;
            buf[i + 2] = ((x ^ y ^ n) & 255) as u8;
            buf[i + 3] = 255;
        }
    }
    buf
}

fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    writer.finish().unwrap();
    out
}

fn png_decode(bytes: &[u8]) -> Vec<u8> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut out = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut out).unwrap();
    out.truncate(info.buffer_size());
    out
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() as f64 * q) as usize).min(sorted.len() - 1)]
}

fn main() {
    let frames: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000);
    let png_mode = std::env::args().nth(2).is_some_and(|v| v == "png");

    // Six pre-generated variants per stream: the producer's bytes.
    let ring: Vec<Vec<Vec<u8>>> = STREAMS
        .iter()
        .map(|&(id, w, h)| {
            (0..6)
                .map(|v| {
                    let raw = frame_bytes(w, h, v, id);
                    if png_mode { png_bytes(w, h, &raw) } else { raw }
                })
                .collect()
        })
        .collect();
    let encoded_per_frame: usize = ring.iter().map(|s| s[0].len()).sum();

    let mut decode_ms = Vec::with_capacity(frames);
    let mut store_ms = Vec::with_capacity(frames);
    let mut uploads = 0usize;
    for n in 0..frames {
        let t0 = Instant::now();
        let pixels: Vec<Vec<u8>> = ring
            .iter()
            .map(|stream| {
                let bytes = &stream[n % 6];
                if png_mode { png_decode(bytes) } else { bytes.clone() }
            })
            .collect();
        decode_ms.push(t0.elapsed().as_secs_f64() * 1000.0);

        let t0 = Instant::now();
        for (&(id, w, h), rgba) in STREAMS.iter().zip(pixels) {
            let mut picture = Picture { id, width: w, height: h, rgba, cols: 40, rows: 12 };
            assert!(well_formed(&picture));
            premultiply_in_place(&mut picture.rgba);
            std::hint::black_box(&picture);
            uploads += 1;
        }
        store_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    assert_eq!(uploads, frames * STREAMS.len(), "every stream lands");
    store_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    decode_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let total_s: f64 = store_ms.iter().sum::<f64>() / 1000.0;
    println!(
        "GLASS_PROTO mode={} frames={frames} store_p50_ms={:.3} store_p95_ms={:.3} \
         decode_p50_ms={:.3} decode_p95_ms={:.3} encoded_mib_per_frame={:.3} \
         store_mib_per_s={:.1}",
        if png_mode { "png" } else { "raw" },
        percentile(&store_ms, 0.5),
        percentile(&store_ms, 0.95),
        percentile(&decode_ms, 0.5),
        percentile(&decode_ms, 0.95),
        encoded_per_frame as f64 / 1048576.0,
        encoded_per_frame as f64 * frames as f64 / total_s / 1048576.0,
    );
}
