//! The game's formats: ink rows at grain 1; HD pack entries at grain 4
//! (`hd_pack.rs`'s `pack.json`, each drawing named and keyed by the look it
//! redraws); and a quick-rig export (`assets/quick-rig/knight.json`'s
//! version 1: per clip, per frame, ticks, rows and the six sockets).
//!
//! Both the quick-rig and the HD pack place the recipe's canvas in the
//! game's own rig frame ([`Frame46`]: 46 x 46, the pivot at 23, 30, as the
//! game's knight is exported), so a frame's rows and its HD drawing land
//! where the game's knight does. The HD entry's look key is the game's
//! `look_key` over those exact rows: it matches wherever the game draws them
//! unchanged.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::grain::Grain;
use crate::palette::Palette;
use crate::raster::Raster;
use crate::recipe::Recipe;
use crate::render::{self, Frame};
use crate::rig::QUICK_RIG_SOCKETS;

/// The game's ticks a second, for a frame's `ticks`.
pub const GAME_HZ: f32 = 30.0;

/// The clip names the game's quick-rig reads.
pub const QUICK_RIG_CLIPS: [&str; 13] = ["idle", "walk", "run", "roll", "hurt", "die", "cast", "slash", "overhead", "thrust", "flail", "bow", "gun"];

/// Where a canvas sits in an export's frame, at grain 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The export's size.
    pub w: u32,
    pub h: u32,
    /// Where the canvas's top-left lands.
    pub x: i32,
    pub y: i32,
}

/// The game's rig frame: 46 x 46, the pivot at 23, 30.
pub struct Frame46;

impl Frame46 {
    /// The recipe's canvas placed so its pivot lands on the frame's; an
    /// error when it does not fit.
    pub fn place(recipe: &Recipe) -> Result<Place, String> {
        let [w, h] = recipe.src.canvas;
        let (px, py) = (recipe.src.pivot[0].floor() as i32, (recipe.src.pivot[1].ceil() as i32 - 1).max(0));
        let p = Place { w: 46, h: 46, x: 23 - px, y: 30 - py };
        if p.x < 0 || p.y < 0 || p.x + w as i32 > 46 || p.y + h as i32 > 46 {
            return Err(format!("a {w}x{h} canvas with its pivot at {px},{py} does not fit the game's 46x46 rig frame"));
        }
        Ok(p)
    }

    /// The recipe's canvas as it is: no margin.
    pub fn none(recipe: &Recipe) -> Place {
        Place { w: recipe.src.canvas[0], h: recipe.src.canvas[1], x: 0, y: 0 }
    }
}

/// A frame's raster in the export's frame, at its own grain.
pub fn placed(frame: &Frame, at: Place) -> Raster {
    let g = frame.grain.0 as u32;
    let mut out = Raster::new(at.w * g, at.h * g);
    let (ox, oy) = (at.x * g as i32, at.y * g as i32);
    let r = &frame.raster;
    for y in 0..r.h {
        for x in 0..r.w {
            let i = (y * r.w + x) as usize;
            if !r.ink[i].is_clear() {
                out.put(x as i32 + ox, y as i32 + oy, r.ink[i], r.depth[i], r.part[i]);
            }
        }
    }
    out
}

/// A raster mirrored left to right (`x = w - 1 - x`).
pub fn mirrored(r: &Raster) -> Raster {
    let mut out = Raster::new(r.w, r.h);
    for y in 0..r.h {
        for x in 0..r.w {
            let i = (y * r.w + x) as usize;
            let j = (y * r.w + (r.w - 1 - x)) as usize;
            out.ink[j] = r.ink[i];
            out.depth[j] = r.depth[i];
            out.part[j] = r.part[i];
        }
    }
    out
}

/// The game's look key of a picture: FNV-1a over its size and its pixels as
/// the game keeps them (`0xffrrggbb`, clear 0); `room_scene::look_key`.
pub fn look_key(r: &Raster) -> u64 {
    let p = Palette::realm();
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |word: u32| {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(r.w);
    eat(r.h);
    for &ink in &r.ink {
        eat(p.rgb(ink).map_or(0, |[r, g, b]| 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)));
    }
    hash
}

/// A socket's pixel in the export's frame, at grain 1.
fn anchor(frame: &Frame, name: &str, at: Place, pivot: [i32; 2]) -> [i32; 2] {
    match frame.sockets.get(name) {
        Some([x, y]) => [x.floor() as i32 + at.x, y.floor() as i32 + at.y],
        None => pivot,
    }
}

/// Each exported clip's frames at `grain`, without held gear: the recipe's
/// quick-rig clips, or the still as idle when it has no idle.
pub fn body_clips(recipe: &Recipe, grain: Grain) -> (BTreeMap<String, (f32, Vec<Frame>)>, Vec<String>) {
    let mut out = BTreeMap::new();
    let mut skipped = Vec::new();
    for (name, clip) in &recipe.src.clips {
        if !QUICK_RIG_CLIPS.contains(&name.as_str()) {
            skipped.push(name.clone());
            continue;
        }
        out.insert(name.clone(), (clip.fps, render::clip_frames(recipe, grain, name, false)));
    }
    if !out.contains_key("idle") {
        let still = render::draw(recipe, grain, &crate::rig::Pose::rest(recipe), &render::shown(recipe, None, false));
        out.insert("idle".into(), (8.0, vec![still]));
    }
    (out, skipped)
}

/// The quick-rig export (version 1), and the recipe's clips it left out
/// (names the game does not read).
pub fn quick_rig(recipe: &Recipe) -> Result<(Value, Vec<String>), String> {
    let at = Frame46::place(recipe)?;
    let pivot = [
        recipe.src.pivot[0].floor() as i32 + at.x,
        (recipe.src.pivot[1].ceil() as i32 - 1).max(0) + at.y,
    ];
    let p = Palette::realm();
    let (clips, skipped) = body_clips(recipe, Grain::GAME);
    let mut out = serde_json::Map::new();
    for (name, (fps, frames)) in clips {
        let ticks = (GAME_HZ / fps).round().max(1.0) as u32;
        let mut list = Vec::new();
        for f in &frames {
            let rows = placed(f, at).ink_rows(p);
            let mut anchors = serde_json::Map::new();
            for s in QUICK_RIG_SOCKETS {
                let [x, y] = anchor(f, s, at, pivot);
                if x < 0 || y < 0 || x >= at.w as i32 || y >= at.h as i32 {
                    return Err(format!("{name}: socket {s} at {x},{y} is off the 46x46 rig frame"));
                }
                anchors.insert(s.into(), json!([x, y]));
            }
            list.push(json!({ "ticks": ticks, "rows": rows, "anchors": anchors }));
        }
        out.insert(name, Value::Array(list));
    }
    Ok((json!({ "version": 1, "canvas": [at.w, at.h], "pivot": pivot, "clips": out }), skipped))
}

/// One HD pack drawing: its file in the pack's folder and its picture.
pub struct Drawing {
    pub file: String,
    pub raster: Raster,
}

/// An HD pack for the recipe's body (every exported frame at grain 4, and
/// each one's mirror keyed for facing left): the drawings and `pack.json`.
pub fn hd_pack(recipe: &Recipe) -> Result<(Vec<Drawing>, Value), String> {
    let at = Frame46::place(recipe)?;
    let (small, _) = body_clips(recipe, Grain::GAME);
    let (big, _) = body_clips(recipe, Grain::HD);
    let mut drawings = Vec::new();
    let mut art = Vec::new();
    let name = &recipe.src.name;
    for (clip, (_, frames)) in &small {
        for (i, f) in frames.iter().enumerate() {
            let look = placed(f, at);
            let file = format!("{name}/{clip}-{i}.png");
            let entry = format!("{name}/{clip}-{i}");
            art.push(json!({ "look": format!("{:016x}", look_key(&look)), "name": entry, "file": file, "w": at.w, "h": at.h }));
            art.push(json!({ "look": format!("{:016x}", look_key(&mirrored(&look))), "name": format!("{entry}-left"),
                             "file": file, "flip": true, "w": at.w, "h": at.h }));
            drawings.push(Drawing { file, raster: placed(&big[clip].1[i], at) });
        }
    }
    Ok((drawings, json!({ "scale": 4, "art": art })))
}

/// Ink rows of a frame, `.` clear.
pub fn ink_rows(frame: &Frame) -> Vec<String> {
    frame.raster.ink_rows(Palette::realm())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_key_is_the_games() {
        // One opaque black pixel: the game's FNV-1a over 1, 1, 0xff0b0a08.
        let mut r = Raster::new(1, 1);
        r.put(0, 0, Palette::realm().ink('k').unwrap(), 0.0, 1);
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for word in [1u32, 1, 0xff0b_0a08] {
            for byte in word.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        assert_eq!(look_key(&r), hash);
    }

    #[test]
    fn the_knight_exports_a_quick_rig_the_game_reads() {
        let r = Recipe::parse(include_str!("../recipes/knight.json")).unwrap();
        let (v, _) = quick_rig(&r).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["canvas"], json!([46, 46]));
        let idle = v["clips"]["idle"].as_array().unwrap();
        let rows = idle[0]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 46);
        assert!(rows.iter().all(|r| r.as_str().unwrap().len() == 46));
        for s in QUICK_RIG_SOCKETS {
            assert!(idle[0]["anchors"][s].is_array(), "{s}");
        }
    }
}
