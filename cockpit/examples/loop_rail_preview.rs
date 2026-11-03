//! Entirely CPU/offscreen rail proof. Never creates a host, display, or window.
//! cargo run --manifest-path cockpit/Cargo.toml --profile fast --example loop_rail_preview
#![allow(dead_code, unused_imports)]
#[path = "../src/ui/loop_card.rs"]
mod loop_card_source;
#[path = "../src/drive/loop_dialog.rs"]
mod loop_dialog_source;
#[path = "../src/ui/loop_rail.rs"]
mod loop_rail;
mod drive {
    pub(crate) use crate::loop_dialog_source as loop_dialog;
}

use image::{Rgba, RgbaImage};
use loop_rail::{Activity, Path as RailPath};
use std::{path::PathBuf, time::Instant};

fn main() {
    let out = std::env::var_os("ANGEL_RAIL_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("loop-rail/shots"));
    std::fs::create_dir_all(&out).unwrap();
    let path = RailPath {
        width: 1500.0,
        height: 880.0,
        inset: 8.0,
        radius: 16.0,
    };
    // A 150 x 44 cell window at 10 x 20 px, matching the cockpit preview size.
    let root = ratatui::layout::Rect::new(2, 1, 146, 42);
    let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 150, 44));
    let race = loop_card_source::Race {
        laps_done: 4,
        laps: 12,
        clock: Some((84, 600)),
        fuel: None,
        rolls: false,
        paused: false,
        complete: false,
        task: "ship the parser rewrite behind a feature flag".into(),
    };
    loop_card_source::render_ticker(
        &mut buffer,
        ratatui::layout::Rect::new(2, 3, 146, 3),
        &race,
        None,
        2.0,
        true,
    );
    let mut text = String::new();
    for y in 0..44 {
        for x in 0..150 {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    std::fs::write(out.join("ticker.txt"), text).unwrap();
    std::fs::write(out.join("geometry.json"), format!("{{\"width\":1500,\"height\":880,\"cell_width\":10,\"cell_height\":20,\"content_cells\":[{},{},{},{}]}}\n", root.x, root.y, root.width, root.height)).unwrap();
    let start = Instant::now();
    let frames = 240;
    // Arc-length reparameterization uses production corner pacing. Spend real
    // frames banking instead of whipping 90 degrees between two GIF samples.
    let mut clock = vec![0.0f64];
    for i in 0..10_000 {
        clock.push(clock.last().unwrap() + 1.0 / path.pace(i as f64 / 10_000.0));
    }
    let mut progress = Vec::new();
    for frame in 0..frames {
        let target = clock.last().unwrap() * frame as f64 / (frames - 1) as f64;
        let sample = clock.partition_point(|v| *v < target).min(10_000);
        let turns = sample as f64 / 10_000.0;
        progress.push(turns);
        let time = frame as f32 / 30.0;
        let tile = loop_rail::art::tile(path, turns, time, 1.0, Activity::Working);
        let mut image = RgbaImage::new(1500, 880);
        for (i, color) in tile.rgba.chunks_exact(4).enumerate() {
            let x = tile.x + i as i32 % tile.side as i32;
            let y = tile.y + i as i32 / tile.side as i32;
            if x >= 0 && y >= 0 && x < 1500 && y < 880 {
                image.put_pixel(x as u32, y as u32, Rgba(color.try_into().unwrap()));
            }
        }
        image
            .save(out.join(format!("rail-{frame:03}.png")))
            .unwrap();
        let crop = RgbaImage::from_raw(tile.side, tile.side, tile.rgba).unwrap();
        crop.save(out.join(format!("toy-{frame:03}.png"))).unwrap();
    }
    std::fs::write(
        out.join("progress.json"),
        serde_json::to_vec(&progress).unwrap(),
    )
    .unwrap();
    for (name, activity, speed) in [
        ("working", Activity::Working, 1.0),
        ("waiting", Activity::Waiting, 0.32),
        ("operator", Activity::Operator, 0.08),
        ("chequers", Activity::Finished, 0.15),
    ] {
        let tile = loop_rail::art::tile(path, 0.05, 2.3, speed, activity);
        RgbaImage::from_raw(tile.side, tile.side, tile.rgba)
            .unwrap()
            .save(out.join(format!("state-{name}.png")))
            .unwrap();
    }
    let film = angel_delve::render::racers::card::racer_turns(64, 16);
    for (i, rgba) in film.frames.into_iter().enumerate() {
        RgbaImage::from_raw(64, 64, rgba)
            .unwrap()
            .save(out.join(format!("raster-turn-{i:02}.png")))
            .unwrap();
    }
    // Producer-only cost; no PNG encoding or window/render/upload work included.
    let bench = Instant::now();
    for i in 0..10_000 {
        std::hint::black_box(loop_rail::art::tile(
            path,
            i as f64 / 10_000.0,
            i as f32 / 30.0,
            1.0,
            Activity::Working,
        ));
    }
    let active_us = bench.elapsed().as_secs_f64() * 1e6 / 10_000.0;
    let mut state = loop_rail::State::default();
    let bench = Instant::now();
    for _ in 0..1_000_000 {
        std::hint::black_box(state.step_paced(std::hint::black_box(None), 1.0, 1.0));
    }
    let idle_ns = bench.elapsed().as_secs_f64() * 1e9 / 1_000_000.0;
    let metrics = format!(
        "frames={frames}\nactive_producer_us={active_us:.3}\nidle_state_ns={idle_ns:.3}\ntotal_seconds={:.3}\n",
        start.elapsed().as_secs_f64()
    );
    std::fs::write(out.join("producer-cost.txt"), &metrics).unwrap();
    print!("{metrics}");
}
