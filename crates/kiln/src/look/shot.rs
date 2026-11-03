//! Sheets to look at: the grain ladder at true screen size on a floor, the
//! zoom, a reference beside them; frame strips with onion skins; a clip's
//! frames for a GIF, in the realm's inks.
//!
//! True screen size is the game's: a 16 px tile at grain 1 is drawn four
//! times over, and an HD pack's 64 px tile lands in the same rect at one to
//! one. So grain `g` is shown at `4 / g` and every grain stands the same
//! height, as the player would see it.

use crate::grain::Grain;
use crate::palette::{Ink, Palette};
use crate::raster::Raster;
use crate::render::Frame;

use super::{Rgba, text_width};

/// The sheet's ground.
pub const PAPER: [u8; 4] = [0x18, 0x17, 0x16, 255];
/// The default floor and its tile lines.
pub const FLOOR: [u8; 4] = [0x2a, 0x28, 0x26, 255];
pub const FLOOR_LINE: [u8; 4] = [0x33, 0x31, 0x2e, 255];
/// Words on the sheet.
pub const INK: [u8; 4] = [0xc7, 0xc4, 0xbb, 255];
pub const DIM: [u8; 4] = [0x7a, 0x7c, 0x72, 255];

/// How a sheet is laid out.
#[derive(Clone, Debug)]
pub struct ShotOpts {
    /// The zoom row's pixels per screen pixel of true size.
    pub zoom: u32,
    /// A reference picture and the grain it is drawn at.
    pub reference: Option<(Rgba, Grain)>,
    /// A floor tile at grain 1 (16 px a tile), tiled under every panel.
    pub floor: Option<Rgba>,
    /// Socket marks on the zoom.
    pub sockets: bool,
    pub title: String,
}

impl Default for ShotOpts {
    fn default() -> ShotOpts {
        ShotOpts { zoom: 4, reference: None, floor: None, sockets: false, title: String::new() }
    }
}

/// The colours socket marks are drawn in, by name.
pub fn socket_colour(name: &str) -> [u8; 4] {
    match name {
        "grip" => [0xff, 0x50, 0x50, 255],
        "off_hand" => [0x50, 0xa0, 0xff, 255],
        "head" => [0xff, 0xe0, 0x40, 255],
        "back" => [0xc0, 0x60, 0xff, 255],
        "feet" => [0x40, 0xff, 0x80, 255],
        "tip" => [0xff, 0x90, 0x20, 255],
        _ => [0xff, 0xff, 0xff, 255],
    }
}

/// A panel's ground, `w` x `h` screen pixels, at `per` screen pixels a
/// game pixel: the floor tiled, or the default floor with a line every tile.
pub fn ground(w: u32, h: u32, per: u32, floor: Option<&Rgba>) -> Rgba {
    if let Some(f) = floor {
        return Rgba::tiled(f, w, h, per);
    }
    let mut out = Rgba::filled(w, h, FLOOR);
    let tile = 16 * per;
    for y in 0..h {
        for x in 0..w {
            if x % tile == 0 || y % tile == 0 {
                out.set(x as i32, y as i32, FLOOR_LINE);
            }
        }
    }
    out
}

/// A reference's figure box, `(x, y, w, h)`, by its opaque pixels.
fn figure_box(img: &Rgba) -> Option<(i32, i32, i32, i32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..img.h as i32 {
        for x in 0..img.w as i32 {
            if img.get(x, y)[3] > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    (x0 <= x1).then_some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// One panel: a frame at `per` screen pixels a game pixel on its ground.
fn panel(frame: &Frame, per: u32, floor: Option<&Rgba>, sockets: bool) -> Rgba {
    let g = frame.grain.0 as u32;
    let (w, h) = (frame.raster.w / g * per, frame.raster.h / g * per);
    let mut out = ground(w, h, per, floor);
    let scale = (per / g).max(1);
    out.blit(&Rgba::of(&frame.raster), 0, 0, scale);
    if sockets {
        for (name, [x, y]) in &frame.sockets {
            let (sx, sy) = ((x * per as f32) as i32, (y * per as f32) as i32);
            let c = socket_colour(name);
            let r = (per as i32 / 4).max(1);
            for d in -r..=r {
                out.set(sx + d, sy, c);
                out.set(sx, sy + d, c);
            }
        }
    }
    out
}

/// A reference in a panel the canvas's size, at `per` screen pixels a game
/// pixel, its figure's foot at the canvas's pivot.
fn ref_panel(r: &Rgba, rg: Grain, canvas: (u32, u32), pivot: [f32; 2], per: u32, floor: Option<&Rgba>) -> Rgba {
    let (w, h) = (canvas.0 * per, canvas.1 * per);
    let mut out = ground(w, h, per, floor);
    let scale = (per / rg.0 as u32).max(1);
    let (bx, by, bw, bh) = figure_box(r).unwrap_or((0, 0, r.w as i32, r.h as i32));
    let foot_x = (pivot[0] * per as f32) as i32;
    let foot_y = (pivot[1] * per as f32) as i32;
    let x = foot_x - (bx + bw / 2) * scale as i32;
    let y = foot_y - (by + bh) * scale as i32;
    out.blit(r, x, y, scale);
    out
}

/// Panels in a row with their labels above, `gap` apart.
fn row(panels: &[(String, Rgba)], gap: u32) -> Rgba {
    let w = panels.iter().map(|(l, p)| p.w.max(text_width(l, 2))).sum::<u32>() + gap * (panels.len() as u32 + 1);
    let h = panels.iter().map(|(_, p)| p.h).max().unwrap_or(0) + 14 + gap;
    let mut out = Rgba::filled(w, h, PAPER);
    let mut x = gap as i32;
    for (label, p) in panels {
        out.text(x, 2, label, INK, 2);
        out.blit(p, x, 14, 1);
        x += (p.w.max(text_width(label, 2)) + gap) as i32;
    }
    out
}

/// Rows stacked, the first under a title.
fn stack(title: &str, rows: &[Rgba]) -> Rgba {
    let w = rows.iter().map(|r| r.w).max().unwrap_or(0).max(text_width(title, 2) + 16);
    let h = rows.iter().map(|r| r.h).sum::<u32>() + 18;
    let mut out = Rgba::filled(w, h, PAPER);
    out.text(8, 4, title, INK, 2);
    let mut y = 18;
    for r in rows {
        out.blit(r, 0, y, 1);
        y += r.h as i32;
    }
    out
}

/// The sheet for one moment: `frames` is the moment at each grain. The top
/// row is the grain ladder at true screen size (and the reference); under
/// it, grains 1 and 4 (and the reference) at `zoom` times that.
pub fn sheet(frames: &[Frame], canvas: (u32, u32), pivot: [f32; 2], opts: &ShotOpts) -> Rgba {
    let floor = opts.floor.as_ref();
    let mut top = Vec::new();
    for f in frames {
        let g = f.grain.0 as u32;
        if g > 4 {
            continue;
        }
        top.push((format!("g{g} x{}", 4 / g), panel(f, 4, floor, false)));
    }
    if let Some((r, rg)) = &opts.reference {
        top.push((format!("ref g{}", rg.0), ref_panel(r, *rg, canvas, pivot, 4, floor)));
    }
    let per = 4 * opts.zoom;
    let mut zoom = Vec::new();
    for f in frames {
        let g = f.grain.0 as u32;
        if g == 1 || g == 4 || (g == 8 && frames.len() == 1) {
            zoom.push((format!("g{g} x{}", per / g), panel(f, per, floor, opts.sockets)));
        }
    }
    if let Some((r, rg)) = &opts.reference {
        zoom.push((format!("ref g{} x{}", rg.0, per / rg.0 as u32), ref_panel(r, *rg, canvas, pivot, per, floor)));
    }
    stack(&opts.title, &[row(&top, 8), row(&zoom, 8)])
}

/// A clip's frames side by side at `per` screen pixels a game pixel, each
/// numbered, with the frame before it as a faint ghost when `onion`.
pub fn strip(frames: &[Frame], per: u32, onion: bool, sockets: bool, floor: Option<&Rgba>, title: &str) -> Rgba {
    let mut panels = Vec::new();
    for (i, f) in frames.iter().enumerate() {
        let mut p = panel(f, per, floor, sockets);
        if onion && i > 0 {
            let prev = &frames[i - 1];
            let g = prev.grain.0 as u32;
            let scale = (per / g).max(1);
            for y in 0..prev.raster.h {
                for x in 0..prev.raster.w {
                    if prev.raster.is_inked(x as i32, y as i32) && !f.raster.is_inked(x as i32, y as i32) {
                        p.fill((x * scale) as i32, (y * scale) as i32, scale, scale, [0x63, 0x9d, 0xc2, 70]);
                    }
                }
            }
        }
        panels.push((format!("{i}"), p));
    }
    stack(title, &[row(&panels, 6)])
}

/// A clip's frames on their ground at `per` screen pixels a game pixel,
/// ready for a GIF.
pub fn reel(frames: &[Frame], per: u32, floor: Option<&Rgba>) -> Vec<Rgba> {
    frames.iter().map(|f| panel(f, per, floor, false)).collect()
}

/// The realm's inks a GIF's floor is laid in: its ground and its tile lines.
pub const GIF_FLOOR: (char, char) = ('g', 'X');

/// A clip's frames on a floor at `per` screen pixels a game pixel, every
/// pixel one of the realm's inks (a GIF's palette is the realm's): the
/// floor tile, when given, is read ink by nearest ink.
pub fn reel_inks(frames: &[Frame], per: u32, floor: Option<&Rgba>) -> Vec<Raster> {
    let p = Palette::realm();
    let nearest = |c: [u8; 4]| -> Ink {
        p.inks()
            .min_by_key(|&i| {
                let [r, g, b] = p.rgb(i).unwrap_or([0, 0, 0]);
                let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
                d(r, c[0]) + d(g, c[1]) + d(b, c[2])
            })
            .unwrap_or(Ink(1))
    };
    let (ground, line) = (p.ink(GIF_FLOOR.0).unwrap_or(Ink(1)), p.ink(GIF_FLOOR.1).unwrap_or(Ink(1)));
    frames
        .iter()
        .map(|f| {
            let g = f.grain.0 as u32;
            let (w, h) = (f.raster.w / g * per, f.raster.h / g * per);
            let mut out = Raster::new(w, h);
            let tile = 16 * per;
            for y in 0..h {
                for x in 0..w {
                    let ink = match floor {
                        Some(fl) => {
                            let (tw, th) = ((fl.w * per).max(1), (fl.h * per).max(1));
                            nearest(fl.get(((x % tw) / per) as i32, ((y % th) / per) as i32))
                        }
                        None if x % tile == 0 || y % tile == 0 => line,
                        None => ground,
                    };
                    out.ink[(y * w + x) as usize] = ink;
                }
            }
            let scale = (per / g).max(1);
            for y in 0..f.raster.h {
                for x in 0..f.raster.w {
                    let ink = f.raster.ink[(y * f.raster.w + x) as usize];
                    if ink.is_clear() {
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let (px, py) = (x * scale + dx, y * scale + dy);
                            if px < w && py < h {
                                out.ink[(py * w + px) as usize] = ink;
                            }
                        }
                    }
                }
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::Recipe;
    use crate::render;

    #[test]
    fn every_grain_stands_the_same_height_on_the_sheet() {
        let r = Recipe::parse(
            r#"{ "kiln": 0, "name": "ball", "canvas": [8, 8], "pivot": [4, 8],
            "materials": { "m": { "ramp": "sxSuUvVW", "finish": "metal" } },
            "parts": [ { "id": "b", "shape": "ellipsoid", "at": [4, 4, 0], "r": [3, 3, 3], "mat": "m" } ] }"#,
        )
        .unwrap();
        let frames: Vec<Frame> = [1, 2, 4].map(|g| render::still(&r, Grain(g))).into_iter().collect();
        let opts = ShotOpts::default();
        for f in &frames {
            assert_eq!(panel(f, 4, None, false).h, 32);
        }
        let s = sheet(&frames, (8, 8), [4.0, 8.0], &opts);
        assert!(s.w > 3 * 32 && s.h > 32 + 4 * 32);
    }
}
