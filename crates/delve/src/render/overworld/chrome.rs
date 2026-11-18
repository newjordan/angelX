//! The realm's UI chrome: every box the game draws (place plaques and name
//! tags, key prompts, the ENTER band, boards, panels, menus and the tray's
//! slots) in one hand, so the Delve, the kingdom, the pastimes and the
//! menus read as one kit.
//!
//! A box is cut, not drawn square. Its corners follow a small fixed curve
//! (a pixel or a few off each corner row, more on a bigger box), it is
//! filled with a gentle vertical ramp that is lightest under its top edge
//! and falls toward black, its rim is lit along the top and shaded along
//! the bottom (the realm's light comes from the north-west), a framed box
//! keeps a dark hairline inside its rim, and it drops a one-pixel shadow
//! down and right into the black. Every ink is on the realm palette, and a
//! box costs one write a pixel, as the flat fill it replaced did.

use super::ink::{Img, Rgb, rgb, text_width};

const NIGHT: Rgb = rgb(0x0a0c12);
const NAVY: Rgb = rgb(0x192934);
const SLATE: Rgb = rgb(0x22303a);
const DUSK: Rgb = rgb(0x263b40);
const TEAL: Rgb = rgb(0x2f4a52);
const STEEL: Rgb = rgb(0x456070);
const BRASS_LIT: Rgb = rgb(0xbe914b);
const BRASS: Rgb = rgb(0xa97639);
const BRASS_DIM: Rgb = rgb(0x784620);
const OAK: Rgb = rgb(0x684629);
const WALNUT: Rgb = rgb(0x582b13);
const BARK: Rgb = rgb(0x331a0c);
const PITCH: Rgb = rgb(0x1a0e07);
const FLAG: Rgb = rgb(0x322b2b);
const PAINT: Rgb = rgb(0xe2cb8b);
const PAINT_DIM: Rgb = rgb(0xd8a95e);
const BRASS_DARK: Rgb = rgb(0x7a4a12);
const INK: Rgb = rgb(0x0b0a08);
const GRAIN: Rgb = rgb(0x45291c);
const DUN: Rgb = rgb(0x362715);
const EMBER: Rgb = rgb(0xa8341f);
const CINDER: Rgb = rgb(0x431e0f);
const ASH: Rgb = rgb(0x3d3a36);
const SOOT: Rgb = rgb(0x151614);

const NIGHTFALL: &[Rgb] = &[SLATE, NAVY, NAVY];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub lip: Option<Rgb>,
    pub fill: &'static [Rgb],
    pub rim: [Rgb; 3],
    pub hairline: Option<Rgb>,
}

impl Tone {
    pub const SLATE: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [STEEL, TEAL, SLATE],
        hairline: None,
    };
    pub const KEY: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS, BRASS_DIM, OAK],
        hairline: None,
    };
    pub const BRASS: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS_LIT, BRASS, BRASS_DIM],
        hairline: None,
    };
    pub const FRAMED: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS_LIT, BRASS, BRASS_DIM],
        hairline: Some(NIGHT),
    };
    pub const SIGN: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    pub const NIGHT_SIGN: Tone = Tone {
        lip: None,
        fill: &[NIGHT],
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    pub const TIMBER: Tone = Tone {
        lip: Some(GRAIN),
        fill: &[DUN, DUN, BARK, BARK, PITCH],
        rim: [OAK, WALNUT, PITCH],
        hairline: None,
    };
    pub const EMBER: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [EMBER, CINDER, CINDER],
        hairline: Some(NIGHT),
    };
    pub const WELL: Tone = Tone {
        lip: None,
        fill: &[NIGHT, SOOT, SOOT, SOOT],
        rim: [ASH, ASH, ASH],
        hairline: None,
    };

    pub const BARE: Tone = Tone {
        lip: None,
        fill: NIGHTFALL,
        rim: [SLATE, SLATE, NAVY],
        hairline: None,
    };
    pub const KEYCAP: Tone = Tone {
        lip: Some(rgb(0x63655c)),
        fill: &[ASH, ASH, rgb(0x322b2b)],
        rim: [rgb(0x7a7c72), rgb(0x515349), SOOT],
        hairline: None,
    };
    pub const PLATE: Tone = Tone {
        lip: Some(ASH),
        fill: &[FLAG, FLAG, SOOT],
        rim: BRASS_RIM,
        hairline: None,
    };
    pub const PAINTED: Tone = Tone {
        lip: None,
        fill: &[PAINT, PAINT, PAINT_DIM],
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    pub const NAMEPLATE: Tone = Tone {
        lip: None,
        fill: &[BRASS_DARK],
        rim: BRASS_RIM,
        hairline: None,
    };
    pub const GLASS: Tone = Tone {
        lip: None,
        fill: &[INK],
        rim: [rgb(0x6e8a8e), STEEL, TEAL],
        hairline: Some(TEAL),
    };
    pub const PIP: Tone = Tone {
        lip: Some(rgb(0x8fa3a8)),
        fill: &[rgb(0x6e8a8e)],
        rim: [rgb(0x6e8a8e), rgb(0x6e8a8e), rgb(0x2f4a52)],
        hairline: None,
    };
    pub const CHALK: Tone = Tone {
        lip: Some(rgb(0x263b1c)),
        fill: &[rgb(0x182414), rgb(0x182414), rgb(0x0d1a0c)],
        rim: BRASS_RIM,
        hairline: None,
    };
    pub const HEALTH: Tone = Tone {
        lip: None,
        fill: &[INK],
        rim: [rgb(0x63655c), ASH, rgb(0x322b2b)],
        hairline: None,
    };
    pub const CHIP: Tone = Tone {
        lip: None,
        fill: &[SOOT],
        rim: [SOOT, NIGHT, NIGHT],
        hairline: None,
    };

    pub const fn rimmed(self, rim: [Rgb; 3]) -> Tone {
        Tone { rim, ..self }
    }
}

pub const BRASS_RIM: [Rgb; 3] = [BRASS_LIT, BRASS, BRASS_DIM];
pub const GOLD_RIM: [Rgb; 3] = [rgb(0xf7ca58), rgb(0xecb64a), rgb(0xa97639)];
pub const fn dusk_mark(ch: char) -> char {
    match ch {
        '4' | '5' | '6' | '@' | 'T' | 't' => 'O',
        '9' | 'c' | 'w' | 'H' | 'W' => 'i',
        other => other,
    }
}

pub fn dusk_icon(icon: &Img) -> Img {
    icon.recolor(&[
        ('4', 'O'), ('5', 'O'), ('6', 'O'), ('@', 'O'), ('T', 'O'), ('t', 'O'),
        ('9', 'i'), ('c', 'i'), ('w', 'i'), ('H', 'i'), ('W', 'i'),
    ])
}

pub const COLD_RIM: [Rgb; 3] = [rgb(0x515349), ASH, rgb(0x322b2b)];

pub fn rim_of(ch: char) -> [Rgb; 3] {
    let v = |c: char| super::ink::ink(c).unwrap_or(NIGHT);
    match ch {
        'B' => [OAK, WALNUT, BARK],
        'b' => [rgb(0x452712), BARK, PITCH],
        'P' => [BRASS_DIM, OAK, WALNUT],
        'r' => [BRASS, BRASS_DIM, OAK],
        'o' => BRASS_RIM,
        'O' => [v('t'), BRASS_LIT, BRASS],
        '4' | 'a' => [v('5'), v('4'), BRASS],
        '5' | '6' => [v('6'), v('5'), v('4')],
        '7' => [EMBER, EMBER, CINDER],
        'g' => COLD_RIM,
        'j' | 'G' => [v('J'), v('G'), v('j')],
        'J' | 'h' => [v('h'), v('J'), v('G')],
        'u' => [rgb(0x6e8a8e), STEEL, TEAL],
        'U' => [v('v'), v('U'), STEEL],
        'S' | 'x' => [STEEL, TEAL, SLATE],
        _ => {
            let c = v(ch);
            [c, c, super::material::darker(c)]
        }
    }
}

const SHADOW: Rgb = NIGHT;

pub fn curve(w: i32, h: i32) -> &'static [i32] {
    match w.min(h) {
        ..=7 => &[1],
        ..=13 => &[2, 1],
        ..=40 => &[3, 1, 1],
        _ => &[4, 2, 1, 1],
    }
}

fn inset_of(curve: &[i32], y: i32, h: i32) -> i32 {
    let edge = y.min(h - 1 - y);
    if edge < 0 {
        return i32::MAX;
    }
    curve.get(edge as usize).copied().unwrap_or(0)
}

fn span(curve: &[i32], w: i32, h: i32, y: i32) -> (i32, i32) {
    if y < 0 || y >= h {
        return (0, 0);
    }
    let c = inset_of(curve, y, h);
    (c, w - c)
}

fn off((from, to): (i32, i32), x: i32) -> bool {
    x < from || x >= to
}

fn on_rim(curve: &[i32], w: i32, h: i32, x: i32, y: i32) -> bool {
    let here = span(curve, w, h, y);
    !off(here, x)
        && (x == here.0
            || x == here.1 - 1
            || off(span(curve, w, h, y - 1), x)
            || off(span(curve, w, h, y + 1), x))
}

pub fn panel(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tone: Tone) {
    draw(im, (x, y, w, h), curve(w, h), tone, true);
}

pub fn inset_panel(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tone: Tone) {
    draw(im, (x, y, w, h), curve(w, h), tone, false);
}

pub fn shaped(im: &mut Img, rect: (i32, i32, i32, i32), curve: &[i32], tone: Tone, shadow: bool) {
    draw(im, rect, curve, tone, shadow);
}

fn draw(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), curve: &[i32], tone: Tone, shadow: bool) {
    if w <= 0 || h <= 0 {
        return;
    }
    let row = |dy: i32| span(curve, w, h, dy);
    if shadow {
        for dy in 1..=h {
            let (from, to) = row(dy - 1);
            let here = row(dy);
            for dx in from + 1..to + 1 {
                if off(here, dx) {
                    im.set(x + dx, y + dy, SHADOW);
                }
            }
        }
    }
    let lip_row = 1 + i32::from(tone.hairline.is_some());
    let top = lip_row + i32::from(tone.lip.is_some());
    let rows = (h - 1 - top).max(1);
    let n = tone.fill.len() as i32;
    let band = |dy: i32| match tone.lip {
        Some(lip) if dy == lip_row => lip,
        _ => tone.fill[(((dy - top).clamp(0, rows - 1) * n) / rows) as usize],
    };
    let deep = curve.len() as i32 + 2;
    let rim = |dx: i32, dy: i32| on_rim(curve, w, h, dx, dy);
    for dy in 0..h {
        let (from, to) = row(dy);
        let (up, down) = (row(dy - 1), row(dy + 1));
        let fill = band(dy);
        let edge_row = dy < deep || dy >= h - deep;
        for dx in from..to {
            let ink = if !edge_row && dx >= from + 2 && dx < to - 2 {
                fill
            } else if off(up, dx) {
                tone.rim[0]
            } else if off(down, dx) {
                tone.rim[2]
            } else if dx == from || dx == to - 1 {
                tone.rim[1]
            } else if let Some(line) = tone
                .hairline
                .filter(|_| rim(dx, dy - 1) || rim(dx, dy + 1) || rim(dx - 1, dy) || rim(dx + 1, dy))
            {
                line
            } else {
                fill
            };
            im.set(x + dx, y + dy, ink);
        }
    }
}

pub fn ring(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), inset: i32) {
    let (x, y, w, h) = (x + inset, y + inset, w - 2 * inset, h - 2 * inset);
    if w <= 2 || h <= 2 {
        return;
    }
    let curve = curve(w, h);
    let deep = curve.len() as i32 + 2;
    for dy in 0..h {
        let (from, to) = span(curve, w, h, dy);
        let edge_row = dy < deep || dy >= h - deep;
        for dx in from..to {
            if (edge_row || dx < from + 2 || dx >= to - 2) && on_rim(curve, w, h, dx, dy) {
                im.set(x + dx, y + dy, if dy == 0 { STEEL } else if dy == h - 1 { SLATE } else { TEAL });
            }
        }
    }
}

pub fn board(im: &mut Img, rect: (i32, i32, i32, i32), frame: char) {
    panel(im, rect, Tone::SIGN.rimmed(rim_of(frame)));
}

pub fn chip(im: &mut Img, rect: (i32, i32, i32, i32)) {
    draw(im, rect, &[1], Tone::BARE, false);
}

pub fn floor_plate(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), lit: bool, open: bool) {
    let rim = match (open, lit) {
        (true, true) => rim_of('5'),
        (true, false) => BRASS_RIM,
        (false, _) => COLD_RIM,
    };
    draw(im, (x, y, w, h), curve(w, h), Tone::PLATE.rimmed(rim), false);
}

pub fn studs(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), ch: char) {
    for (dx, dy) in [(2, 2), (w - 3, 2), (2, h - 3), (w - 3, h - 3)] {
        im.put(x + dx, y + dy, ch);
    }
}

pub fn pip(im: &mut Img, rect: (i32, i32, i32, i32), filled: bool) {
    let tone = if filled { Tone::PIP } else { Tone::WELL.rimmed(rim_of('J')) };
    draw(im, rect, &[1], tone, false);
}

pub fn corners(im: &Img, (x, y, w, h): (i32, i32, i32, i32), curve: &[i32]) -> Vec<(i32, i32, Option<Rgb>)> {
    let mut out = Vec::new();
    for dy in 0..h {
        let c = inset_of(curve, dy, h).min(w);
        for dx in (0..c).chain((w - c).max(c)..w) {
            out.push((x + dx, y + dy, im.get(x + dx, y + dy)));
        }
    }
    out
}

pub fn restore(im: &mut Img, kept: &[(i32, i32, Option<Rgb>)]) {
    for &(x, y, c) in kept {
        match c {
            Some(c) => im.set(x, y, c),
            None => im.clear(x, y),
        }
    }
}

pub fn keycap(im: &mut Img, rect: (i32, i32, i32, i32)) {
    draw(
        im,
        rect,
        &[1],
        Tone::KEYCAP,
        false,
    );
}

pub fn tag(im: &mut Img, rect: (i32, i32, i32, i32), tone: Tone) {
    panel(im, rect, tone);
}

pub fn label_rect((x, y): (i32, i32), text: &str) -> (i32, i32, i32, i32) {
    (x - 3, y - 2, text_width(text) + 6, 11)
}

pub fn label(im: &mut Img, at: (i32, i32), text: &str, ch: char, tone: Tone) -> (i32, i32, i32, i32) {
    let rect = label_rect(at, text);
    tag(im, rect, tone);
    im.text(at.0, at.1, text, dusk_mark(ch));
    rect
}

pub fn band(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), edge: Rgb) {
    for dy in 0..h.max(0) {
        let ink = match dy {
            0 => edge,
            1 => NAVY,
            _ => NIGHT,
        };
        for dx in 0..w {
            im.set(x + dx, y + dy, ink);
        }
    }
}

pub fn rule(im: &mut Img, x0: i32, x1: i32, y: i32) {
    for x in x0..=x1 {
        im.set(x, y, NIGHT);
        im.set(x, y + 1, TEAL);
    }
}

pub fn highlight(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32)) {
    draw(
        im,
        (x, y, w, h),
        &[1],
        Tone {
            lip: None,
            fill: &[TEAL, DUSK, DUSK],
            rim: [TEAL, DUSK, SLATE],
            hairline: None,
        },
        false,
    );
}

pub fn is_highlight(c: Rgb) -> bool {
    [TEAL, DUSK].contains(&c)
}

pub fn meter(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), filled: i32, ink: Rgb) {
    draw(
        im,
        (x, y, w, h),
        &[1],
        Tone {
            lip: None,
            fill: &[NIGHT, SOOT],
            rim: [NIGHT, SOOT, SLATE],
            hairline: None,
        },
        false,
    );
    let filled = filled.clamp(0, w - 2);
    for dy in 1..h - 1 {
        for dx in 1..1 + filled {
            im.set(x + dx, y + dy, ink);
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__chrome_tests.rs"]
mod tests;
