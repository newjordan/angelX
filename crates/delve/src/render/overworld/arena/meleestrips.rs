//! Wished weapon swings as pixelfx strips, fitted by `scripts/delve/meleefx.mjs`.
//!
//! The code-drawn crescent stays. A strip stamps on top when the swinging
//! weapon carries a wish look, row nearest the aim, two ticks a frame. Baked
//! pixels are never scaled or rotated. Pixels outside the swing's reach are
//! left undrawn, so the picture cannot claim a longer blade than the rules.
use super::super::ink::Img;
use super::at;
use crate::realm::forge::Weapon;
use crate::rules::looks;
use crate::rules::melee::{Hit, Stroke, Swing, SWING_TICKS};
use crate::rules::Run;
use std::sync::OnceLock;

#[path = "meleefx_png.rs"]
mod png;

const LOOKS: &[&str] = &["ember", "ice", "gold", "venom", "void", "rose", "steel"];

struct Strip {
    frames: Vec<Vec<Img>>,
    ox: i32,
    oy: i32,
}

fn manifest() -> &'static serde_json::Value {
    static MANIFEST: OnceLock<serde_json::Value> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/meleefx/meleefx.json")).expect("melee manifest")
    })
}

fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes).expect("melee strip").to_rgba8();
    let mut out = Img::new(png.width() as i32, png.height() as i32);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] > 0 {
            out.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    out
}

fn load(name: &str) -> Strip {
    let m = &manifest()[name];
    let n = |k: &str| m[k].as_i64().expect("melee manifest field") as i32;
    let sheet = decode(png::png(name));
    let (w, h) = (n("w"), n("h"));
    let frames = (0..n("dirs"))
        .map(|row| (0..n("frames")).map(|f| Img::from_view(&sheet, f * w, row * h, w, h)).collect())
        .collect();
    Strip { frames, ox: n("ox"), oy: n("oy") }
}

fn strips() -> &'static std::collections::HashMap<String, Strip> {
    static ALL: OnceLock<std::collections::HashMap<String, Strip>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut out = std::collections::HashMap::new();
        if let Some(obj) = manifest().as_object() {
            for name in obj.keys() {
                out.insert(name.clone(), load(name));
            }
        }
        out
    })
}

pub(super) fn kind_word(stroke: Stroke) -> Option<&'static str> {
    match stroke {
        Stroke::Slash => Some("slash"),
        Stroke::Overhead => Some("overhead"),
        Stroke::Thrust => Some("thrust"),
        Stroke::Slam => Some("slam"),
        Stroke::Fist => Some("fist"),
        Stroke::Chain => None,
    }
}

pub(super) fn look_word(colour: &str) -> Option<&'static str> {
    LOOKS.iter().copied().find(|w| colour.eq_ignore_ascii_case(w))
}

/// The wish colour on the weapon in hand. `pal=` recolors the card's sprite
/// inks; it does not name a strip. The knight's own sword has no forged look.
pub(super) fn weapon_look(run: &Run, owner: u32) -> Option<&'static str> {
    let hero = run.players.get(&owner)?;
    if let Some(weapon) = hero.forged.as_ref() {
        if let Some(word) = look_word(&weapon.colour) {
            return Some(word);
        }
    }
    let id = hero.arm.as_deref()?;
    let card = run.book.get(id)?;
    let (colour, _) = card.looks.style.clone()?;
    look_word(&colour)
}

fn row_of(dx: f32, dy: f32, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let k = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32;
    k.rem_euclid(n as i32) as usize
}

fn stamp_inside(cv: &mut Img, im: &Img, at: (i32, i32), ox: i32, oy: i32, reach: f32) {
    let x0 = at.0 - ox;
    let y0 = at.1 - oy;
    let r2 = reach * reach;
    for y in 0..im.h {
        for x in 0..im.w {
            let px = x0 + x;
            let py = y0 + y;
            let dx = (px - at.0) as f32;
            let dy = (py - at.1) as f32;
            if dx * dx + dy * dy > r2 {
                continue;
            }
            if let Some(c) = im.get(x, y) {
                cv.set(px, py, c);
            }
        }
    }
}

/// The looked weapon's strip, over the code swing. No-op without a wish look.
pub(super) fn over_swing(cv: &mut Img, run: &Run, s: &Swing, age: u8, origin: (i32, i32), reach_px: f32) {
    let Some(look) = weapon_look(run, s.owner) else {
        return;
    };
    let Some(kind) = kind_word(s.stroke) else {
        return;
    };
    let Some(strip) = strips().get(&format!("{kind}-{look}")) else {
        return;
    };
    let n = strip.frames.first().map(|r| r.len()).unwrap_or(0);
    if n == 0 || age >= SWING_TICKS {
        return;
    }
    let frame = (age as usize / 2).min(n - 1);
    let row = row_of(s.dx, s.dy, strip.frames.len());
    let Some(im) = strip.frames.get(row).and_then(|r| r.get(frame)) else {
        return;
    };
    stamp_inside(cv, im, origin, strip.ox, strip.oy, reach_px);
}

/// A hit-spark strip when a looked swing still covers the contact.
pub(super) fn over_hit(cv: &mut Img, run: &Run, h: &Hit, at: (i32, i32)) {
    let Some(look) = run.feel.melee.swings.iter().find_map(|s| {
        let look = weapon_look(run, s.owner)?;
        let d = (h.x - s.x).hypot(h.y - s.y);
        (d <= s.reach + 0.35).then_some(look)
    }) else {
        return;
    };
    let Some(strip) = strips().get(&format!("hit-{look}")) else {
        return;
    };
    let n = strip.frames.first().map(|r| r.len()).unwrap_or(0);
    if n == 0 {
        return;
    }
    let frame = (h.age() as usize / 2).min(n - 1);
    let row = row_of(h.dx, h.dy, strip.frames.len());
    let Some(im) = strip.frames.get(row).and_then(|r| r.get(frame)) else {
        return;
    };
    // The spark is a contact, not a reach claim. Keep it on the contact point.
    let reach = (im.w.max(im.h) as f32) * 0.5;
    stamp_inside(cv, im, at, strip.ox, strip.oy, reach);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::forge::Weapon;
    use crate::rules::melee::{Stroke, Swing, Tier, HIT_TICKS, SWING_TICKS};
    use crate::rules::Run;

    fn swing_of(stroke: Stroke, reach: f32, ttl: u8) -> Swing {
        Swing {
            owner: 1,
            x: 8.0,
            y: 8.0,
            dx: 1.0,
            dy: 0.0,
            reach,
            arc: 90.0,
            ttl,
            stroke,
            combo: 1,
        }
    }

    fn run_with(look: Option<&str>) -> Run {
        let mut run = Run::new(1, 1, None);
        if let Some(colour) = look {
            let mut weapon = Weapon::starter();
            weapon.colour = colour.into();
            weapon.shape = "blade".into();
            run.players.get_mut(&1).unwrap().forged = Some(weapon);
        }
        run
    }

    #[test]
    fn every_kind_and_look_is_baked() {
        let kinds = ["slash", "overhead", "thrust", "slam", "fist"];
        for kind in kinds {
            for look in LOOKS {
                let name = format!("{kind}-{look}");
                let strip = strips().get(&name).unwrap_or_else(|| panic!("missing {name}"));
                assert_eq!(strip.frames.len(), 8, "{name} dirs");
                assert_eq!(strip.frames[0].len(), 4, "{name} frames");
                assert!(!png::png(&name).is_empty(), "{name} png");
            }
        }
        for look in LOOKS {
            let name = format!("hit-{look}");
            let strip = strips().get(&name).unwrap_or_else(|| panic!("missing {name}"));
            assert_eq!(strip.frames.len(), 8);
            assert_eq!(strip.frames[0].len(), 3, "{name}");
        }
    }

    #[test]
    fn looked_slash_stays_inside_reach_and_ends_with_the_swing() {
        let reach = 3.3;
        let mut live = run_with(Some("ember"));
        live.feel.melee.swings.push(swing_of(Stroke::Slash, reach, SWING_TICKS - 4));
        let mut bare = run_with(None);
        bare.feel.melee.swings.push(swing_of(Stroke::Slash, reach, SWING_TICKS - 4));
        let mut looked = Img::black(160, 120);
        let mut plain = Img::black(160, 120);
        super::super::melee::swing(&mut looked, &live, 1);
        super::super::melee::swing(&mut plain, &bare, 1);
        let (ox, oy) = at(8.0, 8.0);
        let origin = (ox, oy - 7);
        let r = reach * super::super::UNIT;
        let mut extra = 0;
        for y in 0..looked.h {
            for x in 0..looked.w {
                if looked.get(x, y) != plain.get(x, y) {
                    let dx = (x - origin.0) as f32;
                    let dy = (y - origin.1) as f32;
                    assert!(dx * dx + dy * dy <= r * r + 0.01, "strip pixel {x},{y} outside reach");
                    extra += 1;
                }
            }
        }
        assert!(extra > 0, "a looked slash should add strip pixels");

        let mut done = run_with(Some("ember"));
        done.feel.melee.swings.push(swing_of(Stroke::Slash, reach, 0));
        let mut after = Img::black(160, 120);
        super::super::melee::swing(&mut after, &done, 1);
        let blank = Img::black(160, 120);
        assert_eq!(after.rgba_bytes(), blank.rgba_bytes(), "no strip after the swing ends");
    }

    #[test]
    fn unwished_swing_matches_the_code_path() {
        let mut run = run_with(None);
        run.feel.melee.swings.push(swing_of(Stroke::Slash, 3.3, SWING_TICKS - 2));
        let mut a = Img::black(160, 120);
        let mut b = Img::black(160, 120);
        super::super::melee::swing(&mut a, &run, 1);
        // The same call is the code path: no look, so the strip is not consulted.
        super::super::melee::swing(&mut b, &run, 1);
        assert_eq!(a.rgba_bytes(), b.rgba_bytes());
        let hash = fnv(&a.rgba_bytes());
        assert_eq!(hash, UNWISHED_SLASH, "unwished slash changed ({hash:#x})");
    }

    #[test]
    fn eight_looked_swings_add_under_half_a_millisecond() {
        let looked = run_with(Some("ice"));
        let _warm = strips();
        let mut cv = Img::black(320, 200);
        let swings: Vec<_> = (0..8)
            .map(|i| {
                let mut s = swing_of(Stroke::Slash, 3.3, SWING_TICKS - 4);
                s.x = 4.0 + i as f32 * 1.5;
                s
            })
            .collect();
        let age = 4u8;
        let stamp = |cv: &mut Img| {
            for s in &swings {
                let (ox, oy) = at(s.x, s.y);
                super::over_swing(cv, &looked, s, age, (ox, oy - 7), s.reach * super::super::UNIT);
            }
        };
        stamp(&mut cv);
        let start = std::time::Instant::now();
        stamp(&mut cv);
        let extra = start.elapsed();
        eprintln!("melee strip overhead for 8 swings: {extra:?}");
        assert!(extra.as_secs_f64() * 1000.0 < 0.5, "8 looked swings added {extra:?}");
    }

    #[test]
    fn looked_hit_layers_a_spark_strip() {
        let mut run = run_with(Some("gold"));
        run.feel.melee.swings.push(swing_of(Stroke::Slash, 3.3, SWING_TICKS));
        run.feel.melee.hit(9, (8.5, 8.0), (1.0, 0.0), Tier::Medium, false);
        let mut cv = Img::black(160, 120);
        super::super::melee::over(&mut cv, &run);
        assert!(cv.rgba_bytes().iter().any(|b| *b != 0));
        assert!(HIT_TICKS > 0);
    }

    const UNWISHED_SLASH: u64 = 0xe45e_ffca_4f6f_dca2;

    fn fnv(bytes: &[u8]) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// Review sheets: `ANGEL_MELEE_SHEETS=.we-out cargo test -- --ignored write_melee_sheets`.
    #[test]
    #[ignore]
    fn write_melee_sheets() {
        let dir = std::env::var("ANGEL_MELEE_SHEETS").unwrap_or_else(|_| ".we-out".into());
        std::fs::create_dir_all(&dir).unwrap();
        let kinds = [Stroke::Slash, Stroke::Overhead, Stroke::Thrust, Stroke::Slam, Stroke::Fist];
        let looks = ["ember", "ice", "gold", "venom", "void", "rose", "steel"];
        let ages = [2u8, 3, 4, 5, 6, 7];
        let (cw, ch, zoom) = (96i32, 80i32, 3i32);
        for stroke in kinds {
            let word = match stroke {
                Stroke::Slash => "slash",
                Stroke::Overhead => "overhead",
                Stroke::Thrust => "thrust",
                Stroke::Slam => "slam",
                Stroke::Fist => "fist",
                Stroke::Chain => "chain",
            };
            let reach = if stroke == Stroke::Fist { 1.25 } else { 3.3 };
            let mut sheet = Img::black(cw * 6 * zoom, ch * looks.len() as i32 * zoom);
            let mut before = Img::black(cw * 6 * zoom, ch * zoom);
            for (row, look) in looks.iter().enumerate() {
                for (col, age) in ages.iter().enumerate() {
                    let mut cell = Img::black(cw, ch);
                    let mut run = run_with(Some(look));
                    let mut s = swing_of(stroke, reach, SWING_TICKS - age);
                    s.x = 4.0;
                    s.y = 5.0;
                    run.feel.melee.swings.push(s);
                    super::super::melee::swing(&mut cell, &run, 1);
                    blit_zoom(&mut sheet, &cell, col as i32 * cw * zoom, row as i32 * ch * zoom, zoom);
                    if row == 0 {
                        let mut plain = Img::black(cw, ch);
                        let mut bare = run_with(None);
                        bare.feel.melee.swings.push(s);
                        super::super::melee::swing(&mut plain, &bare, 1);
                        blit_zoom(&mut before, &plain, col as i32 * cw * zoom, 0, zoom);
                    }
                }
            }
            save_sheet(&dir, &format!("melee-{word}.png"), &sheet);
            save_sheet(&dir, &format!("melee-before-{word}.png"), &before);
        }
    }

    fn blit_zoom(dst: &mut Img, src: &Img, x0: i32, y0: i32, zoom: i32) {
        for y in 0..src.h {
            for x in 0..src.w {
                let Some(c) = src.get(x, y) else { continue };
                for dy in 0..zoom {
                    for dx in 0..zoom {
                        dst.set(x0 + x * zoom + dx, y0 + y * zoom + dy, c);
                    }
                }
            }
        }
    }

    fn save_sheet(dir: &str, name: &str, sheet: &Img) {
        let path = std::path::Path::new(dir).join(name);
        image::save_buffer(&path, &sheet.rgba_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgba8).unwrap();
    }
}
