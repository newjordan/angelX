//! A picture against a reference (a painted PNG, or another recipe's
//! render): their sizes and figure boxes, how much their silhouettes
//! overlap once their feet are set on the same spot, the inks each uses,
//! and a picture: the two side by side and their silhouettes laid over each
//! other (only the first in blue, only the reference in orange, both grey).

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use crate::palette::Palette;

use super::Rgba;

/// What two pictures share and where they part.
#[derive(Clone, Debug)]
pub struct Comparison {
    pub a: Side,
    pub b: Side,
    /// The silhouettes' overlap over their union, feet together.
    pub iou: f32,
    /// How far the reference is moved to set its feet on the first's.
    pub shift: (i32, i32),
    /// Opaque pixels that hold the same colour, feet together.
    pub same: u32,
    /// Inks both use.
    pub shared: Vec<char>,
}

/// One picture's numbers.
#[derive(Clone, Debug)]
pub struct Side {
    pub w: u32,
    pub h: u32,
    /// Its figure's box, `(x, y, w, h)`.
    pub figure: Option<(i32, i32, i32, i32)>,
    /// Its figure's pixels.
    pub px: u32,
    /// Its inks by key, most first; `?` for a colour that is no realm ink.
    pub inks: Vec<(char, u32)>,
    /// Pixels in no realm ink.
    pub off_palette: u32,
    /// Its figure's mean lightness, 0..1.
    pub luma: f32,
}

fn side(img: &Rgba) -> Side {
    let p = Palette::realm();
    let by_rgb: HashMap<[u8; 3], char> = p.inks().filter_map(|i| p.rgb(i).map(|c| (c, p.key(i)))).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut inks: BTreeMap<char, u32> = BTreeMap::new();
    let (mut px, mut off, mut luma) = (0u32, 0u32, 0f64);
    for y in 0..img.h as i32 {
        for x in 0..img.w as i32 {
            let c = img.get(x, y);
            if c[3] < 128 {
                continue;
            }
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            px += 1;
            let key = by_rgb.get(&[c[0], c[1], c[2]]).copied().unwrap_or('?');
            if key == '?' {
                off += 1;
            }
            *inks.entry(key).or_default() += 1;
            luma += (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) / 255.0;
        }
    }
    let mut inks: Vec<(char, u32)> = inks.into_iter().collect();
    inks.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Side {
        w: img.w,
        h: img.h,
        figure: (x0 <= x1).then_some((x0, y0, x1 - x0 + 1, y1 - y0 + 1)),
        px,
        inks,
        off_palette: off,
        luma: if px > 0 { (luma / px as f64) as f32 } else { 0.0 },
    }
}

/// Compare `a` with the reference `b`, both at one grain.
pub fn compare(a: &Rgba, b: &Rgba) -> Comparison {
    let (sa, sb) = (side(a), side(b));
    let shift = match (sa.figure, sb.figure) {
        (Some((ax, ay, aw, ah)), Some((bx, by, bw, bh))) => ((ax + aw / 2) - (bx + bw / 2), (ay + ah) - (by + bh)),
        _ => (0, 0),
    };
    let (mut both, mut either, mut same) = (0u32, 0u32, 0u32);
    let (x0, y0) = (0.min(shift.0), 0.min(shift.1));
    let (x1, y1) = ((a.w as i32).max(b.w as i32 + shift.0), (a.h as i32).max(b.h as i32 + shift.1));
    for y in y0..y1 {
        for x in x0..x1 {
            let ca = a.get(x, y);
            let cb = b.get(x - shift.0, y - shift.1);
            let (ia, ib) = (ca[3] >= 128, cb[3] >= 128);
            if ia && ib {
                both += 1;
                if ca[..3] == cb[..3] {
                    same += 1;
                }
            }
            if ia || ib {
                either += 1;
            }
        }
    }
    let shared = sa.inks.iter().map(|k| k.0).filter(|k| *k != '?' && sb.inks.iter().any(|j| j.0 == *k)).collect();
    Comparison { iou: if either == 0 { 1.0 } else { both as f32 / either as f32 }, shift, same, shared, a: sa, b: sb }
}

impl Comparison {
    /// The comparison as text.
    pub fn text(&self, a_name: &str, b_name: &str) -> String {
        let mut s = String::new();
        let line = |s: &mut String, name: &str, x: &Side| {
            let fig = x.figure.map_or("-".into(), |(fx, fy, w, h)| format!("{fx},{fy} {w}x{h}"));
            let inks: String = x.inks.iter().take(24).map(|k| k.0).collect();
            let _ = writeln!(
                s,
                "{name}: {}x{}, figure {fig}, {} px, lightness {:.2}, {} inks: {inks}{}",
                x.w,
                x.h,
                x.px,
                x.luma,
                x.inks.len(),
                if x.off_palette > 0 { format!(" ({} px off the realm's palette)", x.off_palette) } else { String::new() }
            );
        };
        line(&mut s, a_name, &self.a);
        line(&mut s, b_name, &self.b);
        if let (Some(a), Some(b)) = (self.a.figure, self.b.figure) {
            let _ = writeln!(
                s,
                "size: {a_name} is {:.0}% as wide and {:.0}% as tall as {b_name}",
                a.2 as f32 * 100.0 / b.2.max(1) as f32,
                a.3 as f32 * 100.0 / b.3.max(1) as f32
            );
        }
        let _ = writeln!(
            s,
            "silhouette overlap, feet together: {:.0}% ({b_name} moved {},{}); same colour on {} px; {} shared inks: {}",
            self.iou * 100.0,
            self.shift.0,
            self.shift.1,
            self.same,
            self.shared.len(),
            self.shared.iter().collect::<String>()
        );
        s
    }
}

/// The two side by side and their silhouettes laid over each other, each
/// pixel `scale` square.
pub fn picture(a: &Rgba, b: &Rgba, c: &Comparison, scale: u32) -> Rgba {
    let (dx, dy) = c.shift;
    let (x0, y0) = (0.min(dx), 0.min(dy));
    let (x1, y1) = ((a.w as i32).max(b.w as i32 + dx), (a.h as i32).max(b.h as i32 + dy));
    let (ow, oh) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut over = Rgba::filled(ow, oh, [0x2a, 0x28, 0x26, 255]);
    for y in y0..y1 {
        for x in x0..x1 {
            let (ia, ib) = (a.get(x, y)[3] >= 128, b.get(x - dx, y - dy)[3] >= 128);
            let col = match (ia, ib) {
                (true, true) => [0x94, 0x96, 0x8c, 255],
                (true, false) => [0x50, 0x90, 0xe0, 255],
                (false, true) => [0xe8, 0x90, 0x40, 255],
                _ => continue,
            };
            over.set(x - x0, y - y0, col);
        }
    }
    let gap = 4 * scale;
    let w = (a.w + b.w + ow) * scale + gap * 4;
    let h = a.h.max(b.h).max(oh) * scale + gap * 2;
    let mut out = Rgba::filled(w, h, [0x18, 0x17, 0x16, 255]);
    let mut x = gap as i32;
    for img in [a, b, &over] {
        out.fill(x, gap as i32, img.w * scale, img.h * scale, [0x2a, 0x28, 0x26, 255]);
        out.blit(img, x, gap as i32, scale);
        x += (img.w * scale + gap) as i32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_matches_itself_and_a_shifted_copy() {
        let mut a = Rgba::filled(6, 6, [0, 0, 0, 0]);
        a.fill(1, 2, 3, 4, [0x0b, 0x0a, 0x08, 255]);
        let c = compare(&a, &a);
        assert_eq!(c.iou, 1.0);
        assert_eq!(c.a.inks, vec![('k', 12)]);
        let mut b = Rgba::filled(8, 8, [0, 0, 0, 0]);
        b.fill(3, 4, 3, 4, [0x0b, 0x0a, 0x08, 255]);
        let c = compare(&a, &b);
        assert_eq!((c.iou, c.shift), (1.0, (-2, -2)));
        assert!(picture(&a, &b, &c, 2).w > 0);
    }
}
