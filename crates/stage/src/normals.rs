//! Normal maps for pictures that stand as cards in a lit world (docs/
//! WORLD-TIERS.md, "Lit cards"): which way each pixel of a figure faces, so
//! a look's light takes it pixel by pixel and the figure reads round, lit
//! from the side its lamp is on.
//!
//! A normal map is a picture the size of the one it belongs to, straight
//! RGBA and opaque: red is how far a pixel turns to the picture's right,
//! green how far up (the OpenGL way, green up), blue how far out toward the
//! eye, each `n * 0.5 + 0.5` of 255. A pixel that faces straight out is
//! [`FLAT`]. A drawer keeps a picture's normal map under
//! [`picture_of`]`(id)`; a card's material names it (`Material::normals`).
//!
//! [`of_picture`] makes one from the picture itself: a height that rises
//! from the figure's outline to full over [`ROUND`] art pixels (a dome over
//! its silhouette, so a body reads rounded and a thin blade stays flat on),
//! with a little of each pixel's brightness over it ([`DETAIL`]) for its
//! surface, softened, and turned into the slope at each pixel. One normal a
//! pixel of the picture: drawn nearest, a sprite's pixel is one face.

/// A picture's normal map is kept under its id with this bit flipped
/// ([`picture_of`]). Layer pictures (`LAYER_PICTURES` up) have the top bit
/// set, so theirs land past `0xC000_0000`, where nothing else is kept.
pub const NORMAL_PICTURES: u32 = 0x4000_0000;

/// The id a drawer keeps picture `id`'s normal map under.
pub fn picture_of(id: u32) -> u32 {
    id ^ NORMAL_PICTURES
}

/// A pixel that faces straight out of the picture.
pub const FLAT: [u8; 4] = [128, 128, 255, 255];

/// How far in from its outline a figure rises to its full height, in art
/// pixels: a knight's body (a dozen pixels across) is rounded all the way,
/// a wider shape keeps a flat middle.
pub const ROUND: f32 = 6.0;

/// How high a figure stands at its fullest, of [`ROUND`]: a round dome. Its
/// outline pixel sits a pixel in from the edge and the height is softened,
/// so the outline turns some 60 degrees, never edge-on.
pub const HEIGHT: f32 = 1.0;

/// How much a pixel's brightness lifts it, in pixels of height from black
/// to white: enough that a dark outline and a lit fill read as a surface,
/// little enough that the art is never embossed.
pub const DETAIL: f32 = 0.3;

/// The least alpha a pixel stands at; fainter is clear.
const SOLID: u8 = 64;

/// A normal map of `rgba` (straight RGBA, `width` by `height`), drawn
/// `grain` of its pixels to an art pixel (1 for a sprite, more for a finer
/// drawing of one, so it rounds over the same span of the figure). Its
/// clear pixels are [`FLAT`]. Empty when `rgba` is not that size.
pub fn of_picture(rgba: &[u8], (width, height): (u32, u32), grain: f32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return Vec::new();
    }
    let grain = if grain.is_finite() { grain.clamp(1.0, 64.0) } else { 1.0 };
    // The figure's body: what is drawn, but its signal inks (a mark on the
    // floor round its feet, a ring; `look::UNLIT_ALPHA`, never lit) unless
    // the body runs on above and below them (a tabard in a team's colour,
    // an eye).
    let body: Vec<bool> = rgba.chunks_exact(4).map(|p| p[3] >= SOLID && p[3] != crate::look::UNLIT_ALPHA).collect();
    let mut span = vec![(usize::MAX, 0usize); w];
    for (i, _) in body.iter().enumerate().filter(|(_, b)| **b) {
        let (x, y) = (i % w, i / w);
        span[x] = (span[x].0.min(y), span[x].1.max(y));
    }
    let solid: Vec<bool> = (0..w * h)
        .map(|i| {
            if body[i] || rgba[i * 4 + 3] != crate::look::UNLIT_ALPHA {
                return body[i];
            }
            let (x, y) = (i % w, i / w);
            span[x].0 < y && y < span[x].1
        })
        .collect();
    let distance = distance_in(&solid, w, h);
    // The dome: an ellipse's quarter from the outline up, flat past ROUND.
    let round = ROUND * grain;
    let mut height_at: Vec<f32> = distance
        .iter()
        .zip(&solid)
        .map(|(&d, &s)| {
            if !s {
                return 0.0;
            }
            let u = (d / round).min(1.0);
            HEIGHT * round * (1.0 - (1.0 - u) * (1.0 - u)).sqrt()
        })
        .collect();
    // Its brightness, softened over a pixel, a little over the dome: the
    // softening takes a pixel's speckle out and keeps its shapes.
    let mut light: Vec<f32> = rgba
        .chunks_exact(4)
        .zip(&solid)
        .map(|(p, &s)| if s { (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])) / 255.0 } else { 0.0 })
        .collect();
    let soft = (grain * 0.5).round().max(1.0) as usize;
    blur(&mut light, w, h, soft);
    for ((v, l), &s) in height_at.iter_mut().zip(&light).zip(&solid) {
        if s {
            *v += DETAIL * l;
        }
    }
    // The whole height softened over half the rounding, so its creases
    // (where two outlines meet) round off.
    blur(&mut height_at, w, h, ((round * 0.25).round() as usize).max(1));
    let at = |x: isize, y: isize| -> f32 {
        let (x, y) = (x.clamp(0, w as isize - 1) as usize, y.clamp(0, h as isize - 1) as usize);
        height_at[y * w + x]
    };
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if !solid[y * w + x] {
                out[o..o + 4].copy_from_slice(&FLAT);
                continue;
            }
            let (xi, yi) = (x as isize, y as isize);
            // The slope over one pixel each way; rows run down, so up is
            // the way the rows' height falls.
            let dx = (at(xi + 1, yi) - at(xi - 1, yi)) * 0.5;
            let dy = (at(xi, yi + 1) - at(xi, yi - 1)) * 0.5;
            out[o..o + 4].copy_from_slice(&encode([-dx, dy, 1.0]));
        }
    }
    out
}

/// A normal as a pixel of a normal map, made unit.
pub fn encode(n: [f32; 3]) -> [u8; 4] {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if !(len.is_finite() && len > 1e-6) {
        return FLAT;
    }
    let byte = |v: f32| ((v / len * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
    [byte(n[0]), byte(n[1]), byte(n[2]), 255]
}

/// A pixel of a normal map as the normal it is, unit.
pub fn decode(p: [u8; 4]) -> [f32; 3] {
    let v = |b: u8| f32::from(b) / 255.0 * 2.0 - 1.0;
    let n = [v(p[0]), v(p[1]), v(p[2])];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    [n[0] / len, n[1] / len, n[2] / len]
}

/// A normal map mirrored left to right, as its picture is (a figure facing
/// the other way): each pixel moved and turned the other way across.
pub fn mirrored(normals: &[u8], (width, height): (u32, u32)) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    if normals.len() != w * h * 4 {
        return Vec::new();
    }
    let mut out = vec![0u8; normals.len()];
    for y in 0..h {
        for x in 0..w {
            let (from, to) = ((y * w + x) * 4, (y * w + w - 1 - x) * 4);
            out[to..to + 4].copy_from_slice(&normals[from..from + 4]);
            out[to] = 255 - normals[from];
        }
    }
    out
}

/// A normal map made fit to draw: opaque, every pixel unit (an authored
/// map may be stored unnormalized, or a picture's averaging may have
/// shortened its normals), one turned into the picture made flat.
pub fn tidied(normals: &mut [u8]) {
    let v = |b: u8| f32::from(b) / 255.0 * 2.0 - 1.0;
    for p in normals.chunks_exact_mut(4) {
        let n = [v(p[0]), v(p[1]), v(p[2]).max(0.0)];
        // Hardly any length: no direction to keep.
        let tidy = if n[0] * n[0] + n[1] * n[1] + n[2] * n[2] < 0.01 { FLAT } else { encode(n) };
        p.copy_from_slice(&tidy);
    }
}

/// How far each pixel is from the nearest clear one (the picture's edge
/// counts as clear), in pixels: a 3-4 chamfer, near enough to a true
/// distance that the dome is round, not square. An outline pixel is 1.
fn distance_in(solid: &[bool], w: usize, h: usize) -> Vec<f32> {
    const FAR: u32 = u32::MAX / 4;
    let mut d: Vec<u32> = solid.iter().map(|&s| if s { FAR } else { 0 }).collect();
    let get = |d: &[u32], x: isize, y: isize| -> u32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize { 0 } else { d[y as usize * w + x as usize] }
    };
    for y in 0..h as isize {
        for x in 0..w as isize {
            let i = y as usize * w + x as usize;
            if d[i] == 0 {
                continue;
            }
            let best = (get(&d, x - 1, y) + 3).min(get(&d, x, y - 1) + 3).min(get(&d, x - 1, y - 1) + 4).min(get(&d, x + 1, y - 1) + 4);
            d[i] = d[i].min(best);
        }
    }
    for y in (0..h as isize).rev() {
        for x in (0..w as isize).rev() {
            let i = y as usize * w + x as usize;
            if d[i] == 0 {
                continue;
            }
            let best = (get(&d, x + 1, y) + 3).min(get(&d, x, y + 1) + 3).min(get(&d, x + 1, y + 1) + 4).min(get(&d, x - 1, y + 1) + 4);
            d[i] = d[i].min(best);
        }
    }
    d.into_iter().map(|v| v as f32 / 3.0).collect()
}

/// `v` softened twice by a box `radius` pixels each way, across then down
/// (near a Gaussian), the edges held.
fn blur(v: &mut [f32], w: usize, h: usize, radius: usize) {
    if radius == 0 {
        return;
    }
    let mut line = Vec::new();
    for _ in 0..2 {
        for y in 0..h {
            line.clear();
            line.extend_from_slice(&v[y * w..(y + 1) * w]);
            box_line(&line, &mut v[y * w..(y + 1) * w], radius);
        }
        let mut column = vec![0.0; h];
        for x in 0..w {
            line.clear();
            line.extend((0..h).map(|y| v[y * w + x]));
            box_line(&line, &mut column, radius);
            for (y, c) in column.iter().enumerate() {
                v[y * w + x] = *c;
            }
        }
    }
}

/// One line boxed by a running sum.
fn box_line(from: &[f32], to: &mut [f32], radius: usize) {
    let n = from.len();
    let at = |i: isize| from[i.clamp(0, n as isize - 1) as usize];
    let r = radius as isize;
    let mut sum: f32 = (-r..=r).map(at).sum();
    let span = (2 * radius + 1) as f32;
    for (i, t) in to.iter_mut().enumerate().take(n) {
        *t = sum / span;
        let i = i as isize;
        sum += at(i + r + 1) - at(i - r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A disc `r` pixels round in a square picture, grey.
    fn disc(side: u32, r: f32) -> Vec<u8> {
        let mut rgba = vec![0u8; (side * side * 4) as usize];
        let c = side as f32 / 2.0;
        for y in 0..side {
            for x in 0..side {
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                if dx * dx + dy * dy <= r * r {
                    let o = ((y * side + x) * 4) as usize;
                    rgba[o..o + 4].copy_from_slice(&[140, 140, 140, 255]);
                }
            }
        }
        rgba
    }

    fn normal(map: &[u8], side: u32, x: u32, y: u32) -> [f32; 3] {
        let o = ((y * side + x) * 4) as usize;
        decode([map[o], map[o + 1], map[o + 2], map[o + 3]])
    }

    #[test]
    fn a_disc_reads_round_its_edges_turned_out_and_its_middle_to_the_eye() {
        let side = 32;
        let map = of_picture(&disc(side, 12.0), (side, side), 1.0);
        assert_eq!(map.len(), (side * side * 4) as usize);
        let middle = normal(&map, side, 16, 16);
        assert!(middle[2] > 0.97, "the middle faces out: {middle:?}");
        let left = normal(&map, side, 5, 16);
        let right = normal(&map, side, 26, 16);
        let top = normal(&map, side, 16, 5);
        let bottom = normal(&map, side, 16, 26);
        assert!(left[0] < -0.4 && right[0] > 0.4, "the sides turn out: {left:?} {right:?}");
        assert!(top[1] > 0.4 && bottom[1] < -0.4, "green is up: {top:?} {bottom:?}");
        // Never so far turned it reads as an edge-on face (blown out), and
        // symmetric across.
        for n in [left, right, top, bottom] {
            assert!(n[2] > 0.35, "{n:?}");
        }
        assert!((left[0] + right[0]).abs() < 0.05 && (top[1] + bottom[1]).abs() < 0.05);
    }

    #[test]
    fn clear_pixels_are_flat_and_the_map_is_opaque() {
        let side = 16;
        let map = of_picture(&disc(side, 5.0), (side, side), 1.0);
        assert_eq!(&map[..4], &FLAT);
        assert!(map.chunks_exact(4).all(|p| p[3] == 255));
        assert!(of_picture(&[0; 12], (2, 2), 1.0).is_empty(), "the wrong size makes none");
    }

    #[test]
    fn a_finer_drawing_rounds_over_the_same_span_of_the_figure() {
        let coarse = of_picture(&disc(32, 12.0), (32, 32), 1.0);
        let fine = of_picture(&disc(128, 48.0), (128, 128), 4.0);
        // The same place of the figure, a third of the way in from its left.
        let a = normal(&coarse, 32, 8, 16);
        let b = normal(&fine, 128, 33, 64);
        assert!((a[0] - b[0]).abs() < 0.15, "{a:?} against {b:?}");
    }

    #[test]
    fn brightness_lifts_a_pixel_a_little() {
        // A flat slab with a lighter stripe down its middle: the stripe's
        // sides lean, a little.
        let (w, h) = (40u32, 40u32);
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let o = ((y * w + x) * 4) as usize;
                let v = if (18..22).contains(&x) { 230 } else { 40 };
                rgba[o..o + 3].copy_from_slice(&[v, v, v]);
            }
        }
        let map = of_picture(&rgba, (w, h), 1.0);
        let side = normal(&map, w, 17, 20);
        assert!(side[0] < -0.02, "the stripe's left side leans left: {side:?}");
        assert!(side[2] > 0.95, "and only a little: {side:?}");
    }

    #[test]
    fn a_signal_ink_round_the_feet_is_no_part_of_the_body_and_one_in_it_is() {
        let side = 32;
        let mut rgba = disc(side, 10.0);
        // A ring on the floor under it, and an eye in its middle.
        for x in 2..30 {
            let o = ((30 * side + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[200, 40, 40, crate::look::UNLIT_ALPHA]);
        }
        let eye = ((16 * side + 16) * 4) as usize;
        rgba[eye..eye + 4].copy_from_slice(&[200, 40, 40, crate::look::UNLIT_ALPHA]);
        let map = of_picture(&rgba, (side, side), 1.0);
        let o = ((30 * side + 16) * 4) as usize;
        assert_eq!(&map[o..o + 4], &FLAT, "the ring is not domed");
        let near_eye = normal(&map, side, 15, 16);
        assert!(near_eye[0].abs() < 0.2, "no pit round the eye: {near_eye:?}");
    }

    #[test]
    fn a_mirrored_map_turns_the_other_way() {
        let side = 32;
        let map = of_picture(&disc(side, 12.0), (side, side), 1.0);
        let back = mirrored(&map, (side, side));
        let a = normal(&map, side, 5, 16);
        let b = normal(&back, side, 26, 16);
        assert!((a[0] + b[0]).abs() < 0.02 && (a[1] - b[1]).abs() < 0.02);
        assert_eq!(picture_of(picture_of(0x8000_0042)), 0x8000_0042);
        assert!(picture_of(0x8000_0042) >= 0xC000_0000);
    }

    #[test]
    fn a_tidied_map_is_unit_and_never_turned_in() {
        let mut map = vec![128, 128, 128, 0, 255, 128, 0, 77];
        tidied(&mut map);
        assert_eq!(&map[..4], &FLAT, "a zero normal is flat");
        let n = decode([map[4], map[5], map[6], map[7]]);
        assert!(n[2] >= 0.0 && map[7] == 255, "{n:?}");
    }
}
