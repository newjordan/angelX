//! Creature materials and selective contours, baked once per authored pose.
//! Authored texture steps survive; feet, centre and ink banks stay fixed.
use super::super::ink::{Img, Rgb, bank_of, ink, is_signal};
use crate::rules::EnemyKind;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub(super) fn dress(src: &Img, kind: EnemyKind) -> Img {
    dress_with(
        src,
        kind,
        matches!(
            kind,
            EnemyKind::Slime | EnemyKind::Skeleton | EnemyKind::Wraith | EnemyKind::Imp
        ),
    )
}

pub(super) fn ripper(src: &Img) -> Img {
    dress_with(src, EnemyKind::Boss, true)
}

fn dress_with(src: &Img, kind: EnemyKind, rounded: bool) -> Img {
    static POSES: OnceLock<Mutex<HashMap<(u8, bool, u64), Img>>> = OnceLock::new();
    let key = (kind as u8, rounded, src.content_hash());
    let mut poses = POSES.get_or_init(Default::default).lock().unwrap();
    poses
        .entry(key)
        .or_insert_with(|| {
            if rounded {
                bake(src, kind)
            } else {
                retain_materials(src, kind)
            }
        })
        .clone()
}

/// Authored marks are material information: fur tufts, ribbed membranes,
/// seams, facets and robe stitches must survive the colour treatment.
/// Only the neutral ramp and exposed contours move; timber, green hide,
/// blue crystal and state inks retain their separate authored steps.
fn retain_materials(src: &Img, kind: EnemyKind) -> Img {
    let mut out = src.clone();
    for y in 0..src.h {
        for x in 0..src.w {
            let Some(c) = src.get(x, y) else { continue };
            if is_signal(c) {
                if kind == EnemyKind::Flesher && c == ink('9').unwrap() {
                    out.put(x, y, 'H');
                }
                continue;
            }
            let edge = src.edge(x, y);
            let neutral = [
                ('k', if edge { 's' } else { 'k' }),
                ('K', 'x'),
                ('Z', 'b'),
                ('X', 'x'),
                ('g', 'S'),
                ('j', 'u'),
                ('G', 'Q'),
                ('J', 'U'),
                ('h', 'v'),
                ('i', 'i'),
                ('H', 'H'),
            ];
            if let Some((_, to)) = neutral.iter().find(|&&(from, _)| ink(from) == Some(c)) {
                out.put(x, y, *to);
            } else if edge {
                let top = src.get(x, y - 1).is_none();
                let left = src.get(x - 1, y).is_none();
                let lower = src.get(x, y + 1).is_none() || src.get(x + 1, y).is_none();
                let contour = if top || left {
                    [('n', 'b'), ('b', 'B'), ('B', 'p'), ('r', 'R'), ('F', 'e')]
                        .into_iter()
                        .find(|&(from, _)| ink(from) == Some(c))
                } else if lower {
                    [('n', 's'), ('b', 's'), ('f', 's'), ('D', 'x')]
                        .into_iter()
                        .find(|&(from, _)| ink(from) == Some(c))
                } else {
                    None
                };
                if let Some((_, to)) = contour {
                    out.put(x, y, to);
                }
            }
        }
    }
    out
}

fn bake(src: &Img, kind: EnemyKind) -> Img {
    use EnemyKind::*;
    let bone_kind = matches!(kind, Skeleton | Shaman);
    let mut out = Img::new(src.w, src.h);
    for y in 0..src.h {
        for x in 0..src.w {
            let Some(c) = src.get(x, y) else {
                continue;
            };
            // Lamps, wound seams and telegraph eyes retain their exact state
            // colour. The Flesher's old signal cream was flesh, not a tell.
            if is_signal(c) && !(kind == Flesher && c == ink('9').unwrap()) {
                out.set(x, y, c);
                continue;
            }
            let bank = bank_of(c).unwrap_or(0);
            let lum = (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
            let bone =
                bone_kind || (kind == Lich && y <= 5) || (kind == Boss && bank == 0 && lum >= 140);
            let ramp = match bank {
                1 => ['D', 'F', 'l', 'y'], // cool green hide / warm slime
                2 => ['b', 'B', 'r', 't'], // plum brown / honey leather
                3 if bone => ['S', 'v', 'i', 'H'],
                3 if kind == Wraith => ['s', 'u', 'U', 'V'],
                3 => ['s', 'S', 'v', 'V'], // iron, slate and cloth
                _ if bone => ['S', 'v', 'i', 'H'],
                _ if kind == Flesher => ['b', 'p', 'o', 'H'],
                _ => ['s', 'S', 'u', 'i'], // chitin and cool stone
            };
            let stops = match bank {
                1 => [30, 60, 128],
                2 => [25, 65, 135],
                _ if bone => [45, 115, 178],
                _ => [34, 86, 175],
            };
            let mut step = stops.iter().filter(|&&stop| lum > stop).count();
            let top = src.get(x, y - 1).is_none();
            let left = src.get(x - 1, y).is_none();
            let bottom = src.get(x, y + 1).is_none();
            let right = src.get(x + 1, y).is_none();
            // Broken coloured contour: lit on upper-left curves, cool on
            // lower-right curves. One-pixel fingers, legs and weapons keep
            // their midtone so articulation survives a terminal reduction.
            let neighbours = [(0, -1), (-1, 0), (1, 0), (0, 1)]
                .iter()
                .filter(|&&(dx, dy)| src.get(x + dx, y + dy).is_some())
                .count();
            if neighbours >= 3 {
                if (top || left) && lum >= 48 {
                    step = (step + 1).min(3);
                } else if bottom || right {
                    step = step.saturating_sub(1);
                }
            } else if lum >= 48 {
                step = step.max(2);
            }
            let colour: Rgb = ink(ramp[step]).unwrap();
            out.set(x, y, colour);
        }
    }
    out
}

/// Tight, quiet contact beneath feet; floating actors have a detached pool.
/// Integer spans only, and outside the sprite's gameplay bounds.
pub(super) fn shadow(cv: &mut Img, x: i32, y: i32, width: i32, floating: bool) {
    let r = (width / 4).clamp(2, 14);
    let y = y + if floating { 2 } else { 1 };
    for (dy, inset, colour) in [(-1, 2, 's'), (0, 0, 'x'), (1, 2, 's')] {
        for dx in -r + inset..=r - inset {
            cv.put(x + dx, y + dy, colour);
        }
    }
}

/// Whole drawings keyed by authored pose selectors. Hot frames clone baked
/// pixels directly, avoiding even the source-row construction and pixel hash.
pub(super) fn pose(category: u8, state: [u32; 4], paint: impl FnOnce() -> Img) -> Img {
    type Poses = HashMap<(u8, [u32; 4]), Img>;
    static POSES: OnceLock<Mutex<Poses>> = OnceLock::new();
    let mut poses = POSES.get_or_init(Default::default).lock().unwrap();
    poses.entry((category, state)).or_insert_with(paint).clone()
}
