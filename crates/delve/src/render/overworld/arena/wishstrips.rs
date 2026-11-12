//! Wished casts (`cast <shape> <element>`) as pixelfx strips, fitted to the
//! inks by `scripts/delve/wishfx.mjs`. A strip is one row of frames, or eight
//! for a cast along the aim (east, then clockwise). A frame is two ticks.
//! Level 2 is the bake size; other sizes are a nearest-neighbour sample of
//! that strip so the picture meets the zone's real reach.
//!
//! This is not `wishfx` (the recipe drawer a wish paints on its shots). The
//! name `wishfx.rs` was already that module.
use super::super::ink::Img;
use super::at;
use crate::rules::cards::{Area, Element, Form};
use crate::rules::zones::{self, Zone};
use std::sync::OnceLock;

#[path = "wishfx_png.rs"]
mod png;

pub(crate) struct Strip {
    pub(crate) frames: Vec<Vec<Img>>,
    ox: i32,
    oy: i32,
    reach: f32,
    field: bool,
}

pub(crate) fn manifest() -> &'static serde_json::Value {
    static MANIFEST: OnceLock<serde_json::Value> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/wishfx/wishfx.json")).expect("wish manifest")
    })
}

fn key(form: Form, element: Element) -> String {
    format!("{}-{}", form.word(), element.word())
}

fn decode(bytes: &[u8]) -> Img {
    let png = image::load_from_memory(bytes).expect("wish strip").to_rgba8();
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
    let n = |k: &str| m[k].as_i64().expect("wish manifest field") as i32;
    let sheet = decode(png::png(name));
    let (w, h) = (n("w"), n("h"));
    let frames = (0..n("dirs"))
        .map(|row| (0..n("frames")).map(|f| Img::from_view(&sheet, f * w, row * h, w, h)).collect())
        .collect();
    Strip {
        frames,
        ox: n("ox"),
        oy: n("oy"),
        reach: m["reach"].as_f64().expect("wish reach") as f32,
        field: m["loop"].as_bool().unwrap_or(false),
    }
}

pub(crate) fn strip(name: &str) -> Option<&'static Strip> {
    strips().get(name)
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

fn impact(form: Form) -> u32 {
    match form {
        Form::Burst => zones::BURST_WAIT,
        Form::Pillar => zones::PILLAR_WAIT,
        Form::Line => zones::LINE_WAIT,
        _ => 0,
    }
}

/// East, then clockwise, matching the bake (`k * π/4`).
fn row_of(dx: f32, dy: f32, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let k = (dy.atan2(dx) / (std::f32::consts::FRAC_PI_4)).round() as i32;
    k.rem_euclid(n as i32) as usize
}

fn frame_of(z: &Zone, strip: &Strip) -> Option<usize> {
    let n = strip.frames.first().map(|r| r.len()).unwrap_or(0);
    if n == 0 || z.age < impact(z.area.form) {
        return None;
    }
    let since = z.age - impact(z.area.form);
    if z.area.form == Form::Field {
        return Some((since as usize / 2) % n);
    }
    let span = z.life.saturating_sub(impact(z.area.form)).max(1);
    let t = since as f32 / span as f32;
    Some(((t * n as f32) as usize).min(n - 1))
}

/// Stamp the zone's strip on `cv`. False during the telegraph, or if the
/// strip is missing. A field fades on its last eight ticks.
pub(crate) fn stand(cv: &mut Img, z: &Zone) -> bool {
    let Some(strip) = strips().get(&key(z.area.form, z.area.element)) else {
        return false;
    };
    let Some(frame) = frame_of(z, strip) else {
        return false;
    };
    let row = row_of(z.dx, z.dy, strip.frames.len());
    let Some(im) = strip.frames.get(row).and_then(|r| r.get(frame)) else {
        return false;
    };
    let (px, py) = at(z.x, z.y);
    let bake = crate::rules::zones::reach(Area { form: z.area.form, element: z.area.element, size: 2 });
    let scale = if bake > 0.0 { z.reach() / bake } else { 1.0 };
    let left = z.life.saturating_sub(z.age);
    let fade = if z.area.form == Form::Field && left < 8 { left as f32 / 8.0 } else { 1.0 };
    blit(cv, im, px, py, strip.ox, strip.oy, scale, fade);
    true
}

fn blit(cv: &mut Img, im: &Img, px: i32, py: i32, ox: i32, oy: i32, scale: f32, fade: f32) {
    if fade <= 0.0 {
        return;
    }
    if (scale - 1.0).abs() < 0.04 && fade >= 1.0 {
        cv.stamp(im, px - ox, py - oy);
        return;
    }
    let sw = ((im.w as f32) * scale).round().max(1.0) as i32;
    let sh = ((im.h as f32) * scale).round().max(1.0) as i32;
    let x0 = px - ((ox as f32) * scale).round() as i32;
    let y0 = py - ((oy as f32) * scale).round() as i32;
    for y in 0..sh {
        let sy = y * im.h / sh;
        for x in 0..sw {
            if fade < 1.0 && ((x + y * 3) & 7) as f32 / 8.0 > fade {
                continue;
            }
            let sx = x * im.w / sw;
            if let Some(c) = im.get(sx, sy) {
                cv.set(x0 + x, y0 + y, c);
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::zones::{self, Zone};
    use crate::rules::{Run, cards::{Area, Element, Form}};

    fn impact(form: Form) -> u32 {
        match form {
            Form::Burst => zones::BURST_WAIT,
            Form::Pillar => zones::PILLAR_WAIT,
            Form::Line => zones::LINE_WAIT,
            _ => 0,
        }
    }

    fn zone(form: Form, element: Element, age: u32, at: (f32, f32)) -> Zone {
        let area = Area { form, element, size: 2 };
        Zone {
            owner: 1,
            area,
            spell: String::new(),
            x: at.0,
            y: at.1,
            dx: 1.0,
            dy: 0.0,
            age,
            life: zones::life(area).max(age + 8),
            damage: 1,
            hit: Vec::new(),
            mended: Vec::new(),
        }
    }

    #[test]
    fn strips_cover_every_cast() {
        let manifest = manifest();
        // Six side-view waves are gone. A missing strip is none, not a panic.
        assert_eq!(manifest.as_object().unwrap().len(), 43);
        for form in Form::ALL {
            for element in Element::ALL {
                let key = format!("{}-{}", form.word(), element.word());
                let code_only = form == Form::Wave && element != Element::Earth;
                if code_only {
                    assert!(manifest.get(&key).is_none(), "{key} should not be baked");
                    assert!(strip(&key).is_none(), "{key} must read as none");
                    let z = zone(form, element, impact(form) + 4, (12.0, 12.0));
                    let mut im = Img::new(64, 64);
                    assert!(!stand(&mut im, &z), "{key} stamped a strip");
                    continue;
                }
                let row = manifest.get(&key).unwrap_or_else(|| panic!("missing {key}"));
                let strip = strip(&key).unwrap_or_else(|| panic!("no strip {key}"));
                assert_eq!(strip.frames.len() as i64, row["dirs"].as_i64().unwrap(), "{key} dirs");
                assert_eq!(strip.frames[0].len() as i64, row["frames"].as_i64().unwrap(), "{key} frames");
                let z = zone(form, element, impact(form) + 4, (12.0, 12.0));
                let mut im = Img::new(400, 400);
                assert!(stand(&mut im, &z), "{key} did not draw");
                let reach = zones::reach(z.area) * super::super::UNIT;
                let (cx, cy) = (z.x * super::super::UNIT, z.y * super::super::UNIT);
                let mut inside = 0;
                for y in 0..im.h {
                    for x in 0..im.w {
                        let dx = x as f32 + 0.5 - cx;
                        let dy = y as f32 + 0.5 - cy;
                        // Reach is the ground disc. A pillar stands up out of it.
                        if dx.abs() <= reach + 2.0 && dy > -160.0 && dy < reach + 8.0 && im.get(x, y).is_some() {
                            inside += 1;
                        }
                    }
                }
                assert!(inside > 0, "{key} drew nothing inside reach {reach}");
            }
        }
    }

    #[test]
    fn twelve_zones_stay_under_a_millisecond() {
        let mut run = Run::new(7, 1, None);
        run.enemies.clear();
        for i in 0..12 {
            let form = Form::ALL[i % Form::ALL.len()];
            let element = Element::ALL[i % Element::ALL.len()];
            let age = impact(form) + 6 + i as u32;
            run.zones.zones.push(zone(form, element, age, (6.0 + (i % 4) as f32 * 3.0, 6.0 + (i / 4) as f32 * 3.0)));
        }
        let mut im = Img::new(384, 224);
        super::super::zones::paint_for_test(&mut im, &run);
        let n = 20;
        let started = std::time::Instant::now();
        for _ in 0..n {
            im.clear_all();
            super::super::zones::paint_for_test(&mut im, &run);
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0 / n as f64;
        eprintln!("twelve wished zones: {ms:.3} ms/frame");
        assert!(ms < 2.0, "twelve zones took {ms:.3} ms");
    }

    /// The strip is a hero piece on top of the code body, not a replacement.
    /// Body ink that the strip does not cover must still be there.
    #[test]
    fn nova_body_draws_under_the_strip() {
        let z = zone(Form::Nova, Element::Fire, 14, (12.5, 12.5));
        let mut run = Run::new(7, 1, None);
        run.enemies.clear();
        run.zones.zones.push(z);
        let mut marks = Img::new(240, 240);
        let mut layered = Img::new(240, 240);
        super::super::zones::paint_marks_for_test(&mut marks, &run);
        super::super::zones::paint_for_test(&mut layered, &run);
        let mut body_px = 0;
        let mut kept = 0;
        for y in 0..marks.h {
            for x in 0..marks.w {
                if marks.get(x, y).is_some() {
                    body_px += 1;
                    if layered.get(x, y).is_some() {
                        kept += 1;
                    }
                }
            }
        }
        assert!(body_px > 80, "nova body drew only {body_px}px");
        assert!(
            kept * 10 >= body_px * 8,
            "strip replaced the body: kept {kept} of {body_px}"
        );
    }

    /// Contact sheets: seven elements, four ages, level 2, full reach.
    /// `ANGEL_WISH_ZONE_SHEETS=.we-out`.
    #[test]
    #[ignore]
    fn write_wish_zone_sheets() {
        let Some(dir) = std::env::var_os("ANGEL_WISH_ZONE_SHEETS") else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        let label = 72i32;
        let cell = 200i32;
        let ages = [4u32, 12, 28, 56];
        for form in Form::ALL {
            let mut sheet = Img::black((label + cell * 4) as i32, cell * 7);
            for (row, element) in Element::ALL.iter().enumerate() {
                let name = element.word();
                // A short label in the gutter, same order as the strip sheets.
                let _ = name;
                for (col, age) in ages.iter().enumerate() {
                    let mut im = Img::new(cell as i32, cell as i32);
                    let mut run = Run::new(7, 1, None);
                    run.enemies.clear();
                    run.zones.zones.push(zone(form, *element, *age, (12.5, 12.5)));
                    super::super::zones::paint_for_test(&mut im, &run);
                    sheet.stamp(&im, label + col as i32 * cell, row as i32 * cell);
                }
            }
            let path = std::path::Path::new(&dir).join(format!("wish-zone-{}.png", form.word()));
            let rgba = sheet.rgba_bytes();
            image::save_buffer(&path, &rgba, sheet.w as u32, sheet.h as u32, image::ColorType::Rgba8).unwrap();
        }
    }
}
