//! Shared emission stamps. Colour and soft volume are baked; the simulation
//! still owns each anchor, radius, warning line and animation lifetime.
use super::super::ink::{BLACK, Img, PALETTE, Rgb, RgbMap, nearest_plain, rgb};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy)]
pub(super) enum Hue {
    Warm,
    Cold,
    Gas,
}
struct Halo {
    dots: Vec<(i32, i32, usize)>,
    colours: RgbMap<[Rgb; 4]>,
}
fn halos() -> &'static [Halo; 3] {
    static HALOS: OnceLock<[Halo; 3]> = OnceLock::new();
    HALOS.get_or_init(|| {
        std::array::from_fn(|h| {
            let tint = [[48, 28, 18], [17, 39, 48], [30, 37, 19]][h];
            let mut dots = Vec::new();
            for y in -5..=5 {
                for x in -5..=5 {
                    let d = x * x + y * y;
                    if d <= 25 {
                        dots.push((
                            x,
                            y,
                            if d < 4 {
                                3
                            } else if d < 10 {
                                2
                            } else if d < 18 {
                                1
                            } else {
                                0
                            },
                        ));
                    }
                }
            }
            let mut colours = RgbMap::default();
            for (i, &p) in PALETTE.iter().enumerate() {
                let c = rgb(p);
                colours.insert(
                    c,
                    std::array::from_fn(|level| {
                        if i >= 60 {
                            return c;
                        }
                        // Light can lift a shadow, but cannot erase a brighter figure.
                        nearest_plain(std::array::from_fn(|k| {
                            c[k].max(tint[k] * (level as u8 + 1) / 4) as f32
                        }))
                    }),
                );
            }
            colours.insert(
                BLACK,
                std::array::from_fn(|l| {
                    nearest_plain(std::array::from_fn(|k| {
                        (tint[k] * (l as u8 + 1) / 4) as f32
                    }))
                }),
            );
            Halo { dots, colours }
        })
    })
}
/// A quiet halo below a crisp core. Signal pixels already drawn always win.
pub(super) fn glow(cv: &mut Img, x: i32, y: i32, hue: Hue) {
    let h = &halos()[hue as usize];
    for &(dx, dy, l) in &h.dots {
        let c = cv.get(x + dx, y + dy).unwrap_or(BLACK);
        if let Some(ramp) = h.colours.get(&c) {
            cv.set(x + dx, y + dy, ramp[l]);
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Stamp {
    Smoke,
    Death,
    Gas,
    Sulphur,
    GasWall,
    SulphurWall,
    Fireball,
    Flash,
    Embers,
    GasRim,
    SulphurRim,
}
type Key = (Stamp, i32, i32, u8);
/// Each radius/phase is authored once. Hits reuse sparse stamps instead of
/// evaluating a radial ramp, noise or seven gas lobes for every frame.
pub(super) fn stamp(kind: Stamp, rx: i32, ry: i32, phase: u8) -> Arc<Img> {
    static CACHE: OnceLock<Mutex<BTreeMap<Key, Arc<Img>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    cache
        .entry((kind, rx, ry, phase))
        .or_insert_with(|| Arc::new(bake(kind, rx, ry, phase)))
        .clone()
}
fn bake(kind: Stamp, rx: i32, ry: i32, phase: u8) -> Img {
    let mut im = Img::new(rx * 2 + 1, ry * 2 + 1);
    for y in -ry..=ry {
        for x in -rx..=rx {
            let d = (x as f32 / rx.max(1) as f32).powi(2) + (y as f32 / ry.max(1) as f32).powi(2);
            if d > 1.0 {
                continue;
            }
            let top = (y as f32 / ry.max(1) as f32 + x as f32 / rx.max(1) as f32 * 0.3) * 0.5;
            let ink = match kind {
                // Three quiet rounded curls fit inside the original puff radius.
                Stamp::Smoke => {
                    let nx = x as f32 / rx.max(1) as f32;
                    let ny = y as f32 / ry.max(1) as f32;
                    let curl = [(-0.3, -0.1, 0.7), (0.3, -0.3, 0.63), (0.1, 0.35, 0.6)]
                        .iter()
                        .map(|&(cx, cy, r)| {
                            ((nx - cx) * (nx - cx) + (ny - cy) * (ny - cy)) / (r * r)
                        })
                        .fold(f32::MAX, f32::min);
                    if curl > 1.0 {
                        continue;
                    }
                    if phase >= 3 {
                        if curl > 0.7 { 's' } else { 'x' }
                    } else if phase >= 2 {
                        if curl > 0.8 {
                            's'
                        } else if top < -0.12 {
                            'S'
                        } else {
                            'x'
                        }
                    } else if curl > 0.8 {
                        'x'
                    } else if top < -0.12 {
                        'u'
                    } else {
                        'S'
                    }
                }
                // A held death flash retains the disc and its four held ticks.
                Stamp::Death => {
                    if x == 0 && y == 0 {
                        '9'
                    } else if d < 0.08 {
                        'c'
                    } else if d < 0.45 {
                        '$'
                    } else if d < 0.78 {
                        'U'
                    } else {
                        'S'
                    }
                }
                // Gas has a legible danger rim and low rolling lobe shading.
                Stamp::Gas | Stamp::Sulphur | Stamp::GasWall | Stamp::SulphurWall => {
                    let green = matches!(kind, Stamp::Gas | Stamp::GasWall);
                    let nx = x as f32 / rx.max(1) as f32;
                    let ny = y as f32 / ry.max(1) as f32;
                    let roll = (phase as f32 * 0.8).sin() * 0.12;
                    let volume = [
                        (-0.4, -0.08, 0.62),
                        (0.0, -0.3, 0.7),
                        (0.44, 0.05, 0.57),
                        (0.0, 0.35, 0.58),
                    ]
                    .iter()
                    .map(|&(cx, cy, r)| {
                        1.0 - ((nx - cx - roll) * (nx - cx - roll) + (ny - cy) * (ny - cy))
                            / (r * r)
                    })
                    .fold(0.0f32, f32::max);
                    if d > 0.91 {
                        if green { 'C' } else { 'Y' }
                    } else if volume < 0.1 {
                        's'
                    } else if volume < 0.3 {
                        if green { 'e' } else { 'N' }
                    } else if top < -0.1 {
                        if green { 'm' } else { 'A' }
                    } else if volume > 0.7 {
                        if green { 'F' } else { 'N' }
                    } else {
                        's'
                    }
                }
                // Fireball, from a pin of ivory into amber and cool smoke.
                Stamp::Fireball => {
                    if x == 0 && y == 0 && phase < 2 {
                        '9'
                    } else if d < 0.04 && phase < 2 {
                        'c'
                    } else if d < 0.22 && phase < 3 {
                        '6'
                    } else if d < 0.5 && phase < 4 {
                        '@'
                    } else if d < 0.78 {
                        'o'
                    } else {
                        'x'
                    }
                }
                // Soft flash through gas. The outer extent still follows its age.
                Stamp::Flash => {
                    if phase >= 4 {
                        if d < 0.3 { 'N' } else { 's' }
                    } else if phase >= 3 {
                        if d < 0.3 {
                            'A'
                        } else if d < 0.5 {
                            'N'
                        } else {
                            's'
                        }
                    } else if d < 0.05 && phase < 2 {
                        'c'
                    } else if d < 0.3 && phase < 3 {
                        '@'
                    } else if d < 0.55 {
                        'A'
                    } else if d < 0.8 {
                        'N'
                    } else {
                        's'
                    }
                }
                // A wall's boundary is stamped after all its cloud bodies,
                // so an overlapping neighbour cannot hide a danger disc.
                Stamp::GasRim | Stamp::SulphurRim => {
                    if d < 0.72 {
                        continue;
                    }
                    if kind == Stamp::SulphurRim {
                        if top < -0.1 { '6' } else { 'Y' }
                    } else if top < -0.1 {
                        'Y'
                    } else {
                        'C'
                    }
                }
                // Glowing embers are quiet and grouped underneath the flames.
                Stamp::Embers => {
                    if d < 0.3 {
                        'o'
                    } else if d < 0.65 {
                        'P'
                    } else {
                        's'
                    }
                }
            };
            im.put(rx + x, ry + y, ink);
        }
    }
    im
}
/// Render-authored icons retain custom card art; familiar kit glyphs get
/// round volumes and a selective warm/cool rim inside their existing well.
pub(super) fn icon(rows: &[&str]) -> Img {
    static ICONS: OnceLock<Mutex<BTreeMap<String, Img>>> = OnceLock::new();
    let key = rows.join("/");
    ICONS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(key)
        .or_insert_with(|| {
            Img::from_rows(rows).recolor(&[
                ('H', 'W'),
                ('h', 'U'),
                ('i', 'V'),
                ('G', 'u'),
                ('j', 'S'),
                ('g', 'x'),
                ('w', '9'),
            ])
        })
        .clone()
}
