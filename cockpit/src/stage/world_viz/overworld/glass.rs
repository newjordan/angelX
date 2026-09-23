//! The scrying glass: other modalities framed over the map.
//!
//! The map is the base layer; a glass is a window laid over it at a place —
//! the Dotmax ride while the knight travels, a location painting when he
//! arrives, any other picture the stage wants to show. Whatever the source,
//! the picture is box-filtered to the glass and snapped to the realm palette
//! (never the signal bank), so a window reads as part of the same drawing.
//! A dotted signal line ties the glass to its place when the place is on
//! screen.

use std::sync::Arc;

use super::ink::{BLACK, Img, nearest_plain};
use super::map::{Place, TILE};

/// Picture size inside the frame, in pixels (8:7, the plates' own shape).
pub(crate) const GLASS_W: i32 = 88;
pub(crate) const GLASS_H: i32 = 77;
const BORDER: i32 = 3;
const TITLE_H: i32 = 10;
const MARGIN: i32 = 6;

/// A picture from another modality, framed over the map at a place.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Glass {
    pub(crate) anchor: Place,
    pub(crate) title: String,
    /// A live source (the ride) shows a signal pip in its title bar.
    pub(crate) live: bool,
    /// Palette-snapped picture, `GLASS_W` x `GLASS_H`.
    pub(crate) picture: Arc<Img>,
    /// Identity of source and frame, for the scene key.
    pub(crate) sequence: u64,
}

/// Box-filter RGBA pixels of any size down (or up) to the glass and snap
/// every pixel to the realm palette.
pub(crate) fn picture_from_rgba(rgba: &[u8], w: u32, h: u32) -> Img {
    let mut out = Img::black(GLASS_W, GLASS_H);
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return out;
    }
    let (w, h) = (w as f32, h as f32);
    for gy in 0..GLASS_H {
        for gx in 0..GLASS_W {
            let x0 = gx as f32 * w / GLASS_W as f32;
            let x1 = ((gx + 1) as f32 * w / GLASS_W as f32).max(x0 + 1.0);
            let y0 = gy as f32 * h / GLASS_H as f32;
            let y1 = ((gy + 1) as f32 * h / GLASS_H as f32).max(y0 + 1.0);
            let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
            for sy in y0 as u32..(y1 as u32).min(h as u32) {
                for sx in x0 as u32..(x1 as u32).min(w as u32) {
                    let i = ((sy * w as u32 + sx) * 4) as usize;
                    for c in 0..3 {
                        sum[c] += rgba[i + c] as f32;
                    }
                    n += 1.0;
                }
            }
            if n > 0.0 {
                let avg = [sum[0] / n, sum[1] / n, sum[2] / n];
                let snapped = if avg.iter().all(|&c| c < 6.0) {
                    BLACK
                } else {
                    nearest_plain(avg)
                };
                out.set(gx, gy, snapped);
            }
        }
    }
    out
}

fn overlap((ax, ay, aw, ah): (i32, i32, i32, i32), (bx, by, bw, bh): (i32, i32, i32, i32)) -> i32 {
    let w = (ax + aw).min(bx + bw) - ax.max(bx);
    let h = (ay + ah).min(by + bh) - ay.max(by);
    if w > 0 && h > 0 { w * h } else { 0 }
}

/// Where the glass sits: the corner of the play field (below `top`) that
/// covers least of what matters — weighted rectangles in frame pixels —
/// nearer the place on a tie.
fn placement(
    f: &Img,
    anchor_px: Option<(i32, i32)>,
    top: i32,
    keep_clear: &[((i32, i32, i32, i32), i32)],
) -> (i32, i32) {
    let (ww, wh) = (GLASS_W + BORDER * 2, GLASS_H + BORDER * 2 + TITLE_H);
    let xs = [MARGIN, f.w - ww - MARGIN];
    let ys = [top + MARGIN, f.h - wh - MARGIN];
    let mut best = (i64::MAX, (xs[1], ys[0]));
    for &y in &ys {
        for &x in &xs {
            let covered: i64 = keep_clear
                .iter()
                .map(|&(rect, weight)| i64::from(overlap((x, y, ww, wh), rect) * weight))
                .sum();
            let reach = anchor_px.map_or(0, |(ax, ay)| {
                i64::from((ax - (x + ww / 2)).abs() + (ay - (y + wh / 2)).abs())
            });
            let score = covered * 1000 + reach;
            if score < best.0 {
                best = (score, (x, y));
            }
        }
    }
    best.1
}

/// Lay the glass over a frame whose play field starts at `top` and shows the
/// realm from world pixel `(view_x, view_y)`. `knight` (world pixels, feet)
/// and the `live` place stay uncovered where a corner allows.
pub(crate) fn draw(
    f: &mut Img,
    glass: &Glass,
    (view_x, view_y): (i32, i32),
    top: i32,
    tick: u32,
    knight: (f32, f32),
    live: Option<Place>,
) {
    let to_frame = |place: Place| {
        let (tx, ty, tw, th) = place.footprint_world();
        (
            tx * TILE - view_x,
            ty * TILE - view_y + top,
            tw * TILE,
            th * TILE,
        )
    };
    let (ax0, ay0, aw, ah) = to_frame(glass.anchor);
    let (ax, ay) = (ax0 + aw / 2, ay0 + ah / 2);
    let on_screen = ax >= 0 && ax < f.w && ay >= top && ay < f.h;
    let anchor_px = on_screen.then_some((ax, ay));
    let (kx, ky) = (knight.0 as i32 - view_x, knight.1 as i32 - view_y + top);
    let mut keep_clear = vec![
        ((kx - 10, ky - 34, 20, 36), 8),
        ((ax0, ay0 - 8, aw, ah + 8), 6),
    ];
    if let Some(place) = live {
        let (lx, ly, lw, lh) = to_frame(place);
        keep_clear.push(((lx, ly - 8, lw, lh + 8), 4));
    }
    let (x, y) = placement(f, anchor_px, top, &keep_clear);
    let (ww, wh) = (GLASS_W + BORDER * 2, GLASS_H + BORDER * 2 + TITLE_H);

    // The tether: dotted signal from the glass's near edge to the place.
    if let Some((ax, ay)) = anchor_px {
        let ex = if ax < x { x } else { x + ww - 1 };
        let ey = y + wh / 2;
        let steps = (ax - ex).abs().max((ay - ey).abs()).max(1);
        for s in (0..=steps).step_by(2) {
            let px = ex + (ax - ex) * s / steps;
            let py = ey + (ay - ey) * s / steps;
            f.put(px, py, '2');
        }
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1), (0, 0)] {
            f.put(ax + dx, ay + dy, '3');
        }
    }

    // Stone frame, bevelled, on a black mat.
    f.rect(x, y, ww, wh, 'k');
    for i in 1..BORDER {
        f.frame(
            x + i - 1,
            y + i - 1,
            ww - 2 * (i - 1),
            wh - 2 * (i - 1),
            if i == 1 { 'u' } else { 'S' },
        );
    }
    for (cx, cy) in [
        (x, y),
        (x + ww - 2, y),
        (x, y + wh - 2),
        (x + ww - 2, y + wh - 2),
    ] {
        f.rect(cx, cy, 2, 2, 'o');
    }
    // Title bar.
    let bar_y = y + BORDER;
    f.rect(x + BORDER, bar_y, GLASS_W, TITLE_H - 1, 'K');
    let room = ((GLASS_W - 10) / 6).max(0) as usize;
    let title: String = glass.title.to_uppercase().chars().take(room).collect();
    f.text(x + BORDER + 3, bar_y + 1, &title, '9');
    if glass.live {
        let pip = if (tick / 2) % 2 == 0 { '2' } else { '3' };
        f.rect(x + BORDER + GLASS_W - 6, bar_y + 3, 3, 3, pip);
    }
    f.stamp(&glass.picture, x + BORDER, bar_y + TITLE_H);
}
