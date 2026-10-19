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

// The palette's steps the kit is built from (`ink::PALETTE`). The two
// without a sprite ink of their own are the stone bank's deepest step and
// the one between 'x' and 'S'.
const NIGHT: Rgb = rgb(0x0a0c12);
const NAVY: Rgb = rgb(0x192934); // 's'
const SLATE: Rgb = rgb(0x22303a); // 'x'
const DUSK: Rgb = rgb(0x263b40);
const TEAL: Rgb = rgb(0x2f4a52); // 'S'
const STEEL: Rgb = rgb(0x456070); // 'u'
const BRASS_LIT: Rgb = rgb(0xbe914b); // 'O'
const BRASS: Rgb = rgb(0xa97639); // 'o'
const BRASS_DIM: Rgb = rgb(0x784620); // 'r'
const OAK: Rgb = rgb(0x684629); // 'P'
const WALNUT: Rgb = rgb(0x582b13); // 'B'
const BARK: Rgb = rgb(0x331a0c); // 'b'
const PITCH: Rgb = rgb(0x1a0e07); // 'n'
const FLAG: Rgb = rgb(0x322b2b); // 'X', warm dark stone
const PAINT: Rgb = rgb(0xe2cb8b); // 'T'
const PAINT_DIM: Rgb = rgb(0xd8a95e); // 't'
const BRASS_DARK: Rgb = rgb(0x7a4a12); // 'a'
const INK: Rgb = rgb(0x0b0a08); // 'k'
// The timber bank's quiet browns between 'b' and 'B'.
const GRAIN: Rgb = rgb(0x45291c);
const DUN: Rgb = rgb(0x362715);
const EMBER: Rgb = rgb(0xa8341f); // '7'
const CINDER: Rgb = rgb(0x431e0f); // '8'
const ASH: Rgb = rgb(0x3d3a36); // 'g'
const SOOT: Rgb = rgb(0x151614); // 'K'

/// The slate ramp most boxes are filled with: slate falling to night blue.
const NIGHTFALL: &[Rgb] = &[SLATE, NAVY, NAVY];

/// What a box is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    /// A lit row just under the top rim, as light catches a bevel.
    pub lip: Option<Rgb>,
    /// The fill, top to bottom: each ink takes an equal share of the rows
    /// inside the rim (below the lip).
    pub fill: &'static [Rgb],
    /// The rim along the top, down the sides and along the bottom.
    pub rim: [Rgb; 3],
    /// A dark line just inside the rim, for a framed board.
    pub hairline: Option<Rgb>,
}

impl Tone {
    /// Night-blue slate under a cool steel rim: names, plaques, the quiet
    /// boards and panels.
    pub const SLATE: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [STEEL, TEAL, SLATE],
        hairline: None,
    };
    /// Slate in a dim brass rim: a key's prompt, the gold of its words
    /// brighter than the rim around them.
    pub const KEY: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS, BRASS_DIM, OAK],
        hairline: None,
    };
    /// Slate under a lit brass rim: whatever a key opens or answers.
    pub const BRASS: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS_LIT, BRASS, BRASS_DIM],
        hairline: None,
    };
    /// A framed brass board: the brass rim, a dark hairline inside it.
    pub const FRAMED: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [BRASS_LIT, BRASS, BRASS_DIM],
        hairline: Some(NIGHT),
    };
    /// Slate in a timber edge: the realm's signs and ledgers.
    pub const SIGN: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    /// A sign standing in the realm's lit map: night black in a timber
    /// edge. The map's dusk steps every prop down its bank with a dither;
    /// the stone bank's last step has nowhere darker to go, so the sign
    /// stays one flat dark and its letters read.
    pub const NIGHT_SIGN: Tone = Tone {
        lip: None,
        fill: &[NIGHT],
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    /// Walnut: carved and painted wooden boards.
    pub const TIMBER: Tone = Tone {
        lip: Some(GRAIN),
        fill: &[DUN, DUN, BARK, BARK, PITCH],
        rim: [OAK, WALNUT, PITCH],
        hairline: None,
    };
    /// Slate in Fortune's red: a dare, a warning.
    pub const EMBER: Tone = Tone {
        lip: Some(DUSK),
        fill: NIGHTFALL,
        rim: [EMBER, CINDER, CINDER],
        hairline: Some(NIGHT),
    };
    /// A slot's well: recessed, so its top lies in shadow and its floor
    /// catches the light, under a rim the caller chooses.
    pub const WELL: Tone = Tone {
        lip: None,
        fill: &[NIGHT, SOOT, SOOT, SOOT],
        rim: [ASH, ASH, ASH],
        hairline: None,
    };

    /// Slate without a rim to speak of, for tags too tight to frame.
    pub const BARE: Tone = Tone {
        lip: None,
        fill: NIGHTFALL,
        rim: [SLATE, SLATE, NAVY],
        hairline: None,
    };
    /// A raised key: grey stone lit on top.
    pub const KEYCAP: Tone = Tone {
        lip: Some(rgb(0x63655c)),
        fill: &[ASH, ASH, rgb(0x322b2b)],
        rim: [rgb(0x7a7c72), rgb(0x515349), SOOT],
        hairline: None,
    };
    /// A floor plate a knight stands on: warm dark stone raised a step, its
    /// lip lit and its foot falling to soot, under a rim its state picks
    /// (`floor_plate`).
    pub const PLATE: Tone = Tone {
        lip: Some(ASH),
        fill: &[FLAG, FLAG, SOOT],
        rim: BRASS_RIM,
        hairline: None,
    };
    /// Cream paint on a timber board: a picket sign.
    pub const PAINTED: Tone = Tone {
        lip: None,
        fill: &[PAINT, PAINT, PAINT_DIM],
        rim: [OAK, WALNUT, BARK],
        hairline: None,
    };
    /// A brass nameplate: dark brass in a brass rim.
    pub const NAMEPLATE: Tone = Tone {
        lip: None,
        fill: &[BRASS_DARK],
        rim: BRASS_RIM,
        hairline: None,
    };
    /// A scrying glass's mat: ink black under a steel rim and a slate line.
    pub const GLASS: Tone = Tone {
        lip: None,
        fill: &[INK],
        rim: [rgb(0x6e8a8e), STEEL, TEAL],
        hairline: Some(TEAL),
    };
    /// A progress pip, filled: stone teal lit along its top, nothing
    /// brighter than the realm's cool greys.
    pub const PIP: Tone = Tone {
        lip: Some(rgb(0x8fa3a8)),
        fill: &[rgb(0x6e8a8e)],
        rim: [rgb(0x6e8a8e), rgb(0x6e8a8e), rgb(0x2f4a52)],
        hairline: None,
    };
    /// A chalkboard: dark green slate in a brass frame.
    pub const CHALK: Tone = Tone {
        lip: Some(rgb(0x263b1c)),
        fill: &[rgb(0x182414), rgb(0x182414), rgb(0x0d1a0c)],
        rim: BRASS_RIM,
        hairline: None,
    };
    /// A health bar's track: ink black in a grey rim lit along its top.
    pub const HEALTH: Tone = Tone {
        lip: None,
        fill: &[INK],
        rim: [rgb(0x63655c), ASH, rgb(0x322b2b)],
        hairline: None,
    };
    /// A small dark chip behind a count.
    pub const CHIP: Tone = Tone {
        lip: None,
        fill: &[SOOT],
        rim: [SOOT, NIGHT, NIGHT],
        hairline: None,
    };

    /// The same box with another rim (top, sides, bottom).
    pub const fn rimmed(self, rim: [Rgb; 3]) -> Tone {
        Tone { rim, ..self }
    }
}

/// A brass rim, lit (`Tone::rimmed`).
pub const BRASS_RIM: [Rgb; 3] = [BRASS_LIT, BRASS, BRASS_DIM];
/// A gold rim for what is ready to fire and matters most (the tray's
/// ultimate; nothing over the world wears it).
pub const GOLD_RIM: [Rgb; 3] = [rgb(0xf7ca58), rgb(0xecb64a), rgb(0xa97639)];
/// What lettering and little pictures over the world wear at dusk: the
/// bright golds, cream and white of the signal bank and the timber's pale
/// top step come down to the brass `O` (5:1 on a board's slate) and the
/// quiet grey `i` (7:1), so a board reads without glaring. The parchment
/// `$`, the bronzes and every other ink are left as they are.
pub const fn dusk_mark(ch: char) -> char {
    match ch {
        '4' | '5' | '6' | '@' | 'T' | 't' => 'O',
        '9' | 'c' | 'w' | 'H' | 'W' => 'i',
        other => other,
    }
}

/// A little picture (a coin, a crown) with its inks stepped by [`dusk_mark`].
pub fn dusk_icon(icon: &Img) -> Img {
    icon.recolor(&[
        ('4', 'O'), ('5', 'O'), ('6', 'O'), ('@', 'O'), ('T', 'O'), ('t', 'O'),
        ('9', 'i'), ('c', 'i'), ('w', 'i'), ('H', 'i'), ('W', 'i'),
    ])
}

/// A rim gone cold: stone grey, for what is not ready.
pub const COLD_RIM: [Rgb; 3] = [rgb(0x515349), ASH, rgb(0x322b2b)];

/// The rim a box framed in the sprite ink `ch` wears in this kit: that ink
/// down the sides, a step lighter along the top, a step darker along the
/// foot. The colour of a board's frame keeps its meaning (gold for the
/// best offer, red for a dare, grey for what is spent).
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

/// The shadow a box drops into the black.
const SHADOW: Rgb = NIGHT;

/// How much of each corner row a box of `w` by `h` loses, from the corner
/// row inward: the bigger the box, the rounder.
pub fn curve(w: i32, h: i32) -> &'static [i32] {
    match w.min(h) {
        ..=7 => &[1],
        ..=13 => &[2, 1],
        ..=40 => &[3, 1, 1],
        _ => &[4, 2, 1, 1],
    }
}

/// The pixels cut from each end of row `y` of a box `h` tall.
fn inset_of(curve: &[i32], y: i32, h: i32) -> i32 {
    let edge = y.min(h - 1 - y);
    if edge < 0 {
        return i32::MAX;
    }
    curve.get(edge as usize).copied().unwrap_or(0)
}

/// Row `y`'s run of a box `w` by `h` cut on `curve`: `[from, to)`, empty
/// off the box.
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

/// Whether `(x, y)` is on the rim of a box: in it, with open paper above,
/// below or beside it.
fn on_rim(curve: &[i32], w: i32, h: i32, x: i32, y: i32) -> bool {
    let here = span(curve, w, h, y);
    !off(here, x)
        && (x == here.0
            || x == here.1 - 1
            || off(span(curve, w, h, y - 1), x)
            || off(span(curve, w, h, y + 1), x))
}

/// A box at `(x, y)`, `w` by `h`, of `tone`, its corners rounded to suit
/// its size, with its shadow.
pub fn panel(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tone: Tone) {
    draw(im, (x, y, w, h), curve(w, h), tone, true);
}

/// A box without the shadow: one set inside another, or flush with an edge.
pub fn inset_panel(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), tone: Tone) {
    draw(im, (x, y, w, h), curve(w, h), tone, false);
}

/// A box with the corners the caller asks for.
pub fn shaped(im: &mut Img, rect: (i32, i32, i32, i32), curve: &[i32], tone: Tone, shadow: bool) {
    draw(im, rect, curve, tone, shadow);
}

fn draw(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), curve: &[i32], tone: Tone, shadow: bool) {
    if w <= 0 || h <= 0 {
        return;
    }
    let row = |dy: i32| span(curve, w, h, dy);
    if shadow {
        // The box moved a pixel down and right, where the box is not: the
        // row above's run, shifted, less this row's run.
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
    // The fill's ink for a row: the lip under the top rim, then equal
    // bands of the ramp. Clean bands of near inks read as a slope at the
    // scale the game is shown; dithering their edges read as stitching.
    let lip_row = 1 + i32::from(tone.hairline.is_some());
    let top = lip_row + i32::from(tone.lip.is_some());
    let rows = (h - 1 - top).max(1);
    let n = tone.fill.len() as i32;
    let band = |dy: i32| match tone.lip {
        Some(lip) if dy == lip_row => lip,
        _ => tone.fill[(((dy - top).clamp(0, rows - 1) * n) / rows) as usize],
    };
    // Rows this near the top or foot, and columns this near a side, may
    // be rim or hairline; everything else is fill.
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

/// A rounded line inside a big framed panel, `inset` pixels in from its
/// edge: the cool inner rule of a window's double frame.
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

/// A board framed in sprite ink `frame` (`rim_of`): slate inside, with
/// its shadow.
pub fn board(im: &mut Img, rect: (i32, i32, i32, i32), frame: char) {
    panel(im, rect, Tone::SIGN.rimmed(rim_of(frame)));
}

/// A tag too tight for a rim (its letters touch its edges): slate with a
/// pixel off each corner, no rim and no shadow.
pub fn chip(im: &mut Img, rect: (i32, i32, i32, i32)) {
    draw(im, rect, &[1], Tone::BARE, false);
}

/// A floor plate on `rect`: gold while a knight stands on it, brass while
/// it is open, stone grey while it is shut. Flush with the floor, so no
/// shadow.
pub fn floor_plate(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), lit: bool, open: bool) {
    let rim = match (open, lit) {
        (true, true) => rim_of('5'),
        (true, false) => BRASS_RIM,
        (false, _) => COLD_RIM,
    };
    draw(im, (x, y, w, h), curve(w, h), Tone::PLATE.rimmed(rim), false);
}

/// A stud in each corner of `rect`, just inside its rounded corners.
pub fn studs(im: &mut Img, (x, y, w, h): (i32, i32, i32, i32), ch: char) {
    for (dx, dy) in [(2, 2), (w - 3, 2), (2, h - 3), (w - 3, h - 3)] {
        im.put(x + dx, y + dy, ch);
    }
}

/// A progress pip on `rect`: filled teal, or an empty dark well.
pub fn pip(im: &mut Img, rect: (i32, i32, i32, i32), filled: bool) {
    let tone = if filled { Tone::PIP } else { Tone::WELL.rimmed(rim_of('J')) };
    draw(im, rect, &[1], tone, false);
}

/// The pixels a box on `rect` cut on `curve` leaves out at its corners,
/// as they are now: drawn over by a square fill (planks, a mat), then put
/// back with `restore`, the box comes out rounded over whatever was there.
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

/// Put back what `corners` kept.
pub fn restore(im: &mut Img, kept: &[(i32, i32, Option<Rgb>)]) {
    for &(x, y, c) in kept {
        match c {
            Some(c) => im.set(x, y, c),
            None => im.clear(x, y),
        }
    }
}

/// A key's cap: a raised grey key, lit on top, its lower edge in shadow.
pub fn keycap(im: &mut Img, rect: (i32, i32, i32, i32)) {
    draw(
        im,
        rect,
        &[1],
        Tone::KEYCAP,
        false,
    );
}

/// A small tag (a name, a prompt): the box, a pixel off each corner on a
/// short tag, and its shadow.
pub fn tag(im: &mut Img, rect: (i32, i32, i32, i32), tone: Tone) {
    panel(im, rect, tone);
}

/// The tag a line of the 5x7 hand at `(x, y)` sits on: its rim a pixel
/// clear of the letters all round.
pub fn label_rect((x, y): (i32, i32), text: &str) -> (i32, i32, i32, i32) {
    (x - 3, y - 2, text_width(text) + 6, 11)
}

/// A line of the 5x7 hand on its tag (`label_rect`). Returns the tag's
/// rectangle.
pub fn label(im: &mut Img, at: (i32, i32), text: &str, ch: char, tone: Tone) -> (i32, i32, i32, i32) {
    let rect = label_rect(at, text);
    tag(im, rect, tone);
    im.text(at.0, at.1, text, dusk_mark(ch));
    rect
}

/// A band across a view's foot (the kingdom's door footer, the college's
/// way out): a lit edge along its top and a row of slate under it, then
/// night, quiet enough that the plaque on it reads first.
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

/// A rule across a board: a dark line with a lit line under it, so it reads
/// as a groove cut into the slate.
pub fn rule(im: &mut Img, x0: i32, x1: i32, y: i32) {
    for x in x0..=x1 {
        im.set(x, y, NIGHT);
        im.set(x, y + 1, TEAL);
    }
}

/// A row picked out on a board or menu: a lighter, rounded bar behind it.
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

/// Whether `c` is the slate fill of a panel (its body, not its words): for
/// the second art kit, which paints its own panel under ours.
pub fn is_slate_fill(c: Rgb) -> bool {
    [SLATE, NAVY, NIGHT].contains(&c)
}

/// Whether `c` is a highlight bar's or a rule's lit ink.
pub fn is_highlight(c: Rgb) -> bool {
    [TEAL, DUSK].contains(&c)
}

/// A small bar of `fill` out of `w`: a rounded dark track, the filled part
/// in `ink` with its top row a step lighter where the palette has one.
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
