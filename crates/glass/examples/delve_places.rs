//! The surface places and their mini-game boards as the Delve paints them
//! (crates/delve render/overworld/arena): the start rings, name boards,
//! prompts and standings that lie over the world, shot on the CPU, no window.
//!
//! cargo run --release -p angel-glass --example delve_places -- <outdir> [--scale N] [--only NAME]...
//!
//! Writes `<place>.png` for each of a few places as a knight stands in them,
//! and `round-<game>.png` for each pastime's round once the host has started
//! it (the rules' own step), at `N` pixels to a stage pixel (3 unless said).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use angel_delve::render::overworld::arena;
use angel_delve::rules::{Input, RoomKind, Run, home::Home, pastimes::Game, world};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("delve_places: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let usage = "usage: delve_places <outdir> [--scale N] [--only NAME]...";
    let mut words = std::env::args().skip(1);
    let (mut out, mut scale, mut only) = (None, 3u32, Vec::new());
    while let Some(word) = words.next() {
        match word.as_str() {
            "--scale" => {
                scale = words.next().and_then(|w| w.parse().ok()).filter(|&s| (1..=8).contains(&s)).ok_or("--scale takes 1 to 8")?;
            }
            "--only" => only.push(words.next().ok_or(usage)?),
            _ if out.is_none() => out = Some(PathBuf::from(word)),
            other => return Err(format!("`{other}`: {usage}")),
        }
    }
    let out = out.ok_or(usage)?;
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (w, h) = arena::seat_size();
    let shoot = |name: &str, run: &Run| -> Result<(), String> {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            return Ok(());
        }
        let img = arena::seat_scene(run, w as i32, h as i32, Some(1)).compose();
        write_png(&out.join(format!("{name}.png")), (img.w as u32 * scale, img.h as u32 * scale), &img.rgba_scaled(scale))?;
        println!("{name}.png");
        Ok(())
    };
    let mut home = Run::at_home(17, 1, Some("Friend"), Home::default(), Default::default());
    home.come_to_the_gate();
    for (kind, name) in [
        (RoomKind::Village, "place-village"),
        (RoomKind::Fairground, "place-fairground"),
        (RoomKind::Forest, "place-forest"),
        (RoomKind::Lighthouse, "place-lighthouse"),
        (RoomKind::Hippodrome, "place-hippodrome"),
        (RoomKind::Lists, "place-lists"),
        (RoomKind::Riverbank, "place-riverbank"),
    ] {
        let mut r = home.clone();
        if let Some(i) = world::room_of(&r.dungeon, kind) {
            r.arrive(i, (12.0, 9.5));
            shoot(name, &r)?;
        }
    }
    for game in Game::ALL {
        let mut r = home.clone();
        let Some(i) = world::room_of(&r.dungeon, game.room()) else { continue };
        r.arrive(i, game.start());
        if let Some(friend) = r.players.get_mut(&2) {
            (friend.x, friend.y) = (42.0, 23.0);
        }
        r.step(&BTreeMap::<u32, Input>::new());
        shoot(&format!("round-{game:?}").to_lowercase(), &r)?;
    }
    Ok(())
}

fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(|e| fail(&e))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| fail(&e))?;
    writer.write_image_data(rgba).map_err(|e| fail(&e))
}
