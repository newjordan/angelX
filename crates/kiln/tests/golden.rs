//! The knight's renders, pinned: the same recipe draws the same pixels on
//! every machine. A change to the renderer or the recipe that moves a pixel
//! shows here; look at the shots (`kiln shot crates/kiln/recipes/knight.json`)
//! and, when the new pixels are wanted, pin them anew.
//!
//! `cargo test -p angel-kiln --profile fast --test golden -- --nocapture`
//! also says how long a frame takes at each grain.

use std::time::Instant;

use angel_kiln::export::look_key;
use angel_kiln::render;
use angel_kiln::{Grain, Recipe};

fn knight() -> Recipe {
    Recipe::parse(include_str!("../recipes/knight.json")).expect("the knight reads")
}

/// The game's look key over a frame: FNV-1a of its size and pixels.
fn key(f: &render::Frame) -> String {
    format!("{:016x}", look_key(&f.raster))
}

#[test]
fn the_knight_draws_its_pinned_pixels() {
    let r = knight();
    let got = [
        ("still g1", key(&render::still(&r, Grain::GAME))),
        ("still g4", key(&render::still(&r, Grain::HD))),
        ("slash#2 g1", key(&render::clip_frame(&r, Grain::GAME, "slash", 2, true).unwrap())),
        ("slash#2 g4", key(&render::clip_frame(&r, Grain::HD, "slash", 2, true).unwrap())),
        ("walk#0 g1", key(&render::clip_frame(&r, Grain::GAME, "walk", 0, true).unwrap())),
    ];
    let want = [
        ("still g1", "591c46dbb1face1d"),
        ("still g4", "e6288814a82529bb"),
        ("slash#2 g1", "ac0241e00dcaee29"),
        ("slash#2 g4", "5e4fe1a7eb2fd97d"),
        ("walk#0 g1", "6eb44762c16a3211"),
    ];
    for (name, g) in &got {
        println!("{name}: {g}");
    }
    for ((name, g), (_, w)) in got.iter().zip(want) {
        assert_eq!(g, w, "{name} moved");
    }
}

#[test]
fn a_frame_takes_milliseconds() {
    let r = knight();
    for g in [Grain(1), Grain(2), Grain(4), Grain(8)] {
        let frames = r.clip("walk").unwrap().frames;
        let t = Instant::now();
        let runs = 8;
        for _ in 0..runs {
            for i in 0..frames {
                std::hint::black_box(render::clip_frame(&r, g, "walk", i, true));
            }
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / (runs * frames) as f64;
        println!("g{}: {ms:.3} ms a frame", g.0);
        // Generous: a debug build on a slow machine still looks dozens of times a minute.
        assert!(ms < 250.0, "g{} took {ms:.1} ms a frame", g.0);
    }
}

/// Every JSON example in docs/KILN-RECIPE.md reads as written: a whole
/// recipe as it is, a part, decals, a rig or clips set into a small one.
#[test]
fn the_reference_examples_read() {
    use serde_json::{Value, json};
    let doc = include_str!("../../../docs/KILN-RECIPE.md");
    let base = || {
        json!({ "kiln": 0, "name": "doc", "canvas": [28, 20], "pivot": [14, 19.5],
            "materials": { "steel": { "ramp": "sxSuUvVW", "finish": "metal" }, "red": { "ramp": "n87", "finish": "cloth" },
                           "leather": { "ramp": "nbIBpr", "finish": "matte" } },
            "parts": [ { "id": "helm", "shape": "ellipsoid", "mat": "steel", "at": [14, 8.7, 0], "r": [3.25, 3.15, 2.7] },
                       { "id": "tabard", "shape": "box", "mat": "red", "at": [14, 14, 2], "size": [4, 4, 0.4] } ],
            "rig": { "bones": [ { "id": "root", "at": [14, 19.5] }, { "id": "torso", "parent": "root", "at": [14, 15] },
                                { "id": "arm_near", "parent": "torso", "at": [17.2, 12.6] },
                                { "id": "hand_near", "parent": "arm_near", "at": [17.7, 15.6] },
                                { "id": "leg_near", "parent": "root", "at": [15, 15.6] }, { "id": "foot_near", "parent": "leg_near", "at": [15, 18.5] },
                                { "id": "leg_far", "parent": "root", "at": [13, 15.6] } ] } })
    };
    let mut seen = 0;
    for block in doc.split("```json").skip(1) {
        let text = block.split("```").next().unwrap();
        let v: Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}:\n{text}"));
        let recipe = if v.get("kiln").is_some() {
            v
        } else {
            let mut r = base();
            if v.get("id").is_some() {
                let parts = r["parts"].as_array_mut().unwrap();
                parts.retain(|p| p["id"] != v["id"]);
                parts.push(v);
            } else if v.get("bones").is_some() {
                r["rig"] = v;
            } else if v.get("4").is_some() {
                r["decals"] = v;
            } else {
                r["clips"] = v;
            }
            r
        };
        Recipe::parse(&recipe.to_string()).unwrap_or_else(|e| panic!("{e}:\n{text}"));
        seen += 1;
    }
    assert!(seen >= 9, "{seen} examples");
}
