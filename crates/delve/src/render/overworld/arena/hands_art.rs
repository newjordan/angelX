//! The Keep's work by hand, drawn (the Works, `rules::hands`): Bomrekedan's
//! teachers in the bailey, the outcrop with its pile of rough blocks, the
//! mortar tub, the tracing floor, the pegged course with the stone about to
//! be set hanging over it, and over each knight at a job its gauge: the ring
//! closing on the beat, the plumb bob and the level's bubble. In the flat
//! view they are stamped on the ground's picture; in 3D they stand up as
//! figures among the stones.
//!
//! Also the trade plate in the view's corner: the knight's trade, how far
//! they are in it, what is here to do and the chain's stock.

use super::*;
use crate::rules::hands::{self, Carry, Grade, Meter};
use crate::rules::trades::{self, Rank, SCHOOLS, Trade};
use crate::world::works;

/// A picture put in the world: stamped on the ground's picture in the flat
/// view, stood up as a figure in 3D (`lift` pixels off the ground when it is
/// an air gauge, its own shadow when `grounded`).
struct Put<'a> {
    over: &'a mut Img,
    figures: Option<&'a mut Vec<Figure>>,
    origin: (i32, i32),
    view: (i32, i32),
}

impl Put<'_> {
    fn seen(&self, (ax, ay): (i32, i32), slack: i32) -> bool {
        let (fx, fy) = (ax - self.origin.0, ay - self.origin.1);
        fx > -slack && fy > -slack - 60 && fx < self.view.0 + slack && fy < self.view.1 + slack + 60
    }

    /// `im` standing with its feet at the ground point `(ax, ay)` (kingdom
    /// pixels), with a shadow when `shadow`; `lift` pixels in the air.
    fn stand(&mut self, im: &Img, (ax, ay): (i32, i32), shadow: bool, lift: i32, bias: f32) {
        if !self.seen((ax, ay), 40) {
            return;
        }
        let (fx, fy) = (ax - self.origin.0, ay - self.origin.1);
        match self.figures.as_deref_mut() {
            Some(figures) => {
                let mut img = Img::new(im.w + 4, im.h + 3);
                if shadow {
                    img.ellipse(img.w as f32 / 2.0, im.h as f32 - 0.5, im.w as f32 / 3.0, 1.6, 'k');
                }
                img.stamp(im, 2, 0);
                figures.push(Figure {
                    img,
                    x: ax - im.w / 2 - 2,
                    y: ay - im.h,
                    frac: (0.0, 0.0),
                    depth: ay as f32 / UNIT + bias,
                    height: lift as f32,
                    additive: false,
                    bodies: Vec::new(),
                    foe: false,
                    paints: Vec::new(),
                });
            }
            None => {
                if shadow {
                    self.over.ellipse(fx as f32, fy as f32, im.w as f32 / 3.0, 1.6, 'k');
                }
                self.over.stamp(im, fx - im.w / 2, fy - im.h + 1 - lift);
            }
        }
    }

    /// `im` laid on the ground with its top left at `(ax, ay)`: a decal.
    fn lay(&mut self, im: &Img, (ax, ay): (i32, i32)) {
        if self.seen((ax, ay), 60) {
            self.over.stamp(im, ax - self.origin.0, ay - self.origin.1);
        }
    }
}

/// One of Bomrekedan's teachers at the Keep: a villager in the school's
/// colour with a banner at their side that carries the figure their
/// questline file draws (the school's sign).
fn teacher(q: &crate::rules::questline::Questline, cloak: char) -> Img {
    let body = kit::villager(0, false).recolor(&[('U', cloak)]);
    let rows: Vec<&str> = q.art.iter().map(String::as_str).collect();
    let icon = Img::from_rows(&rows);
    let (cloth_w, cloth_h) = (icon.w + 2, icon.h + 3);
    let pole = cloth_h + 9;
    let mut im = Img::new(body.w + cloth_w + 3, body.h.max(pole));
    im.stamp(&body, 0, im.h - body.h);
    let px = body.w + 1;
    im.rect(px, im.h - pole, 1, pole, 'r');
    im.rect(px + 1, im.h - pole, cloth_w, cloth_h, 'T');
    im.frame(px + 1, im.h - pole, cloth_w, cloth_h, 'o');
    im.stamp(&icon, px + 2, im.h - pole + 1);
    im
}

/// The cloak of each school, in the order of `trades::SCHOOLS`.
const CLOAKS: [char; 7] = ['S', 'R', 'g', '[', 'u', ']', 'C'];

/// The outcrop: boulders of grey rock leaning on one another, lit from the
/// upper left, seamed where the quarrymen mean to cut and chalked where they
/// have; the blocks come away from its foot in a pile as the stock grows.
fn outcrop(rough: u32) -> Img {
    let mut im = Img::new(40, 28);
    // The shadow it casts, and the body in four overlapping rounds.
    im.ellipse(20.0, 24.0, 18.0, 3.0, 'j');
    for &(cx, cy, rx, ry) in &[(20.0, 18.0, 17.0, 7.5), (12.5, 13.0, 9.5, 7.0), (27.0, 11.5, 8.5, 7.5), (19.0, 8.0, 6.5, 5.0)] {
        im.ellipse(cx, cy, rx, ry, 'G');
    }
    // Light on the tops, a bright edge on the upper left of each.
    for &(cx, cy, rx, ry) in &[(11.5, 10.0, 6.5, 3.5), (25.5, 7.5, 6.0, 3.5), (18.0, 5.5, 4.5, 2.5)] {
        im.ellipse(cx, cy, rx, ry, 'J');
        im.ellipse(cx - 1.2, cy - 1.0, rx * 0.6, ry * 0.5, 'h');
    }
    // The dark underside, where the rounds meet the ground.
    for x in 4..37 {
        im.put(x, 22, 'j');
        if x % 3 != 0 {
            im.put(x, 21, 'j');
        }
    }
    // Seams, cracks, and the chalk line the quarrymen cut along.
    for (x0, y0, x1, y1) in [(10, 14, 12, 21), (20, 10, 21, 21), (28, 12, 26, 21), (16, 7, 18, 11)] {
        im.line(x0, y0, x1, y1, 'g');
    }
    for x in 5..34 {
        if x % 2 == 0 {
            im.put(x, 15 + (x / 9) % 2, 'c');
        }
    }
    // Moss in the cracks at its foot.
    for x in [6, 7, 15, 24, 31, 32] {
        im.put(x, 21, 'm');
        im.put(x + 1, 22, 'l');
    }
    // Blocks come away from it: a pile at its foot as the stock grows.
    for k in 0..rough.min(8) {
        let (bx, by) = (3 + (k as i32 % 4) * 8, 23 - (k as i32 / 4) * 3);
        im.rect(bx, by, 7, 4, 'U');
        im.rect(bx, by, 7, 1, 'v');
        im.rect(bx + 6, by + 1, 1, 3, 'u');
    }
    im
}

/// The mortar tub: a wooden tub, grey mortar in it as full as it is.
fn tub(fill: u32) -> Img {
    let mut im = Img::new(14, 14);
    im.rect(1, 4, 12, 9, 'r');
    im.rect(1, 4, 12, 1, 'o');
    im.rect(1, 8, 12, 1, 'B');
    im.rect(1, 12, 12, 1, 'B');
    im.rect(2, 3, 10, 2, 'g');
    let level = (fill * 5 / hands::TUB_MAX.max(1)) as i32;
    im.rect(2, 4 - level.min(3), 10, 2 + level.min(3), 'h');
    im
}

/// The tracing floor: boards laid flat, a plank floor with chalked arcs on it.
fn tracing_floor() -> Img {
    let mut im = Img::new(42, 28);
    im.rect(0, 0, 42, 28, 'P');
    for y in (4..28).step_by(5) {
        im.line(0, y, 41, y, 'B');
    }
    im.frame(0, 0, 42, 28, 'b');
    // A chalked arch of voussoirs and the springing lines.
    for k in 0..17 {
        let a = k as f32 / 16.0 * std::f32::consts::PI;
        im.put(21 + (a.cos() * 14.0) as i32, 22 - (a.sin() * 12.0) as i32, 'c');
        im.put(21 + (a.cos() * 10.0) as i32, 22 - (a.sin() * 8.5) as i32, 'c');
    }
    for k in 0..9 {
        let a = k as f32 / 8.0 * std::f32::consts::PI;
        im.line(21 + (a.cos() * 10.0) as i32, 22 - (a.sin() * 8.5) as i32, 21 + (a.cos() * 14.0) as i32, 22 - (a.sin() * 12.0) as i32, 'c');
    }
    im
}

/// A peg with a chalk flag: where the next stone goes.
fn peg() -> Img {
    Img::from_rows(&["..66", ".66.", "66..", "6...", "o...", "o...", "o...", "B..."])
}

/// The stone being set: cut stone with its mortar bed, `wet` as the mortar
/// is (a dark bed under it).
fn stone() -> Img {
    let mut im = Img::new(13, 8);
    im.rect(0, 0, 13, 6, 'U');
    im.rect(0, 0, 13, 2, 'V');
    im.rect(0, 5, 13, 1, 'u');
    im.rect(1, 6, 11, 2, 'g');
    im
}

/// The gauge over a knight at work.
fn gauge(meter: Meter, now: u64) -> Img {
    match meter {
        Meter::Beat { off, hits, need, .. } => {
            let mut im = Img::new(19, 25);
            let (cx, cy) = (9.0, 9.0);
            // The target the ring closes on, and the ring.
            let near = off < 0.16 || off > 0.84;
            im.ellipse(cx, cy, 8.0, 8.0, 'K');
            for k in 0..28 {
                let a = k as f32 / 28.0 * std::f32::consts::TAU;
                im.put((cx + a.cos() * 8.0) as i32, (cy + a.sin() * 8.0) as i32, 'j');
                let r = 3.0 + 5.0 * off.min(1.0);
                im.put((cx + a.cos() * r) as i32, (cy + a.sin() * r) as i32, if near { '6' } else { 'y' });
            }
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                im.put((cx + a.cos() * 3.0) as i32, (cy + a.sin() * 3.0) as i32, 'W');
            }
            // The blows that have landed, a pip each.
            let w = i32::from(need) * 3;
            let x0 = 9 - w / 2;
            for k in 0..i32::from(need) {
                im.rect(x0 + k * 3, 20, 2, 2, if k < i32::from(hits) { '6' } else { 'j' });
            }
            let _ = now;
            im
        }
        Meter::Hold { frac } => {
            let mut im = Img::new(26, 7);
            im.rect(0, 0, 26, 7, 'K');
            im.frame(0, 0, 26, 7, 'j');
            im.rect(2, 2, (frac * 22.0).round() as i32, 3, '6');
            im
        }
        Meter::Set { bob, bubble, plumb, level, landed } => {
            let mut im = Img::new(50, 31);
            im.rect(0, 0, 50, 31, 'K');
            im.frame(0, 0, 50, 31, 'j');
            let tone = |g: Option<Grade>, live: char| match g {
                Some(Grade::Perfect) => '6',
                Some(Grade::Good) => 'y',
                Some(Grade::Off) => '7',
                None => live,
            };
            // The plumb: a track with a true mark, the bob swinging on it.
            im.text(3, 2, "PLUMB", tone(plumb, 'h'));
            im.line(4, 12, 46, 12, 'g');
            im.rect(24, 10, 2, 5, 'C');
            let bx = 25 + (bob * 20.0).round() as i32;
            if plumb.is_none() {
                im.line(bx, 10, bx, 13, 'o');
                im.rect(bx - 1, 13, 3, 3, 'O');
            } else {
                im.rect(24, 13, 3, 3, tone(plumb, 'h'));
            }
            // The level: a tube with a mark at its middle, the bubble in it.
            im.text(3, 17, "LEVEL", tone(level, 'h'));
            im.frame(4, 25, 42, 4, 'j');
            im.rect(24, 24, 2, 6, 'C');
            if let Some(b) = bubble.filter(|_| level.is_none()) {
                let lx = 25 + (b * 19.0).round() as i32;
                im.rect(lx - 2, 25, 5, 4, 'W');
            } else if level.is_some() {
                im.rect(23, 25, 5, 4, tone(level, 'h'));
            }
            if landed {
                im.text(32, 17, "SET", '6');
            }
            let _ = now;
            im
        }
    }
}

/// A plate of text: black, framed, lines of ink.
fn plate(lines: &[(String, char)]) -> Img {
    let w = lines.iter().map(|(t, _)| super::super::super::ink::text_width(t)).max().unwrap_or(0) + 6;
    let h = lines.len() as i32 * 9 + 3;
    let mut im = Img::new(w, h);
    im.rect(0, 0, w, h, 'K');
    im.frame(0, 0, w, h, 'g');
    for (k, (t, ink)) in lines.iter().enumerate() {
        im.text(3, 2 + k as i32 * 9, t, *ink);
    }
    im
}

/// The Keep's fixtures, for the view whose top left is `origin` (kingdom
/// pixels): the tracing floor on the ground, the outcrop, the mortar tub and
/// the teachers. Laid before the knights, so a knight walks over the floor.
pub(super) fn props(over: &mut Img, run: &Run, origin: (i32, i32), view: (i32, i32), figures: Option<&mut Vec<Figure>>) {
    if !works::on_map() || run.room().kind != RoomKind::Kingdom {
        return;
    }
    let Some(site) = run.sites.keep() else { return };
    let Some(castle) = site.castle() else { return };
    let mut put = Put { over, figures, origin, view };
    let place = |p: [f32; 2]| {
        let w = castle.world([p[0], p[1], 0.0]);
        (w[0].round() as i32, w[2].round() as i32)
    };
    let y = &run.hands.yard;
    {
        let at = place(hands::FLOOR);
        let im = tracing_floor();
        put.lay(&im, (at.0 - im.w / 2, at.1 - im.h / 2));
    }
    // The bay to raise: a chalk cross on the ground.
    if run.hands.seats.values().any(|h| h.trade == Some(Trade::Carpenter))
        && let Some(bay) = hands::next_bay(castle, y, run.site_shown(site))
    {
        let at = place(bay.at);
        put.lay(&Img::from_rows(&["c...c", ".c.c.", "..c..", ".c.c.", "c...c"]), (at.0 - 2, at.1 - 2));
    }
    // What is freed goes to the stockyard at the Delve's gate, so no pile
    // stands at the foot of the face.
    put.stand(&outcrop(0), place(hands::QUARRY), true, 0, -0.2);
    put.stand(&tub(y.tub), place(hands::TUB), true, 0, 0.0);
    // (The name of the teacher a knight stands before is a floating mark of
    // their own view, `floats`.)
    for (k, school) in SCHOOLS.iter().enumerate() {
        let Some(q) = crate::rules::questline::book().get(school.power) else { continue };
        let fig = teacher(q, CLOAKS[k % CLOAKS.len()]);
        put.stand(&fig, place(school.at), true, 0, 0.0);
    }
}

/// How long a word of the hands floats over a knight, ticks: long enough to
/// read a few short words.
pub const WORD_TICKS: u64 = 90;
/// How many letters a floating word is: the rest of a line is the plate's
/// in the corner.
const WORD_LETTERS: usize = 30;
/// How far above a knight's feet their floating mark stands, pixels.
const FLOAT_ABOVE: i32 = 44;

/// What floats over the world for one knight's view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floating {
    /// The knight's own gauge: the ring on the beat, the plumb and the level.
    Gauge,
    /// The last thing their hands said, as a few words.
    Word,
}

/// One floating mark: its picture and its top left in the kingdom's pixels.
pub struct Float {
    pub what: Floating,
    pub img: Img,
    pub at: (i32, i32),
}

impl Float {
    /// Its box in the kingdom's pixels: x, y, width, height.
    pub fn rect(&self) -> (i32, i32, i32, i32) {
        (self.at.0, self.at.1, self.img.w, self.img.h)
    }
}

/// Whether two boxes (x, y, w, h) touch, with `gap` pixels to spare.
pub fn touches(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32), gap: i32) -> bool {
    a.0 < b.0 + b.2 + gap && b.0 < a.0 + a.2 + gap && a.1 < b.1 + b.3 + gap && b.1 < a.1 + a.3 + gap
}

/// The first few words of a line the hands said: what comes before its
/// colon, capped. The rest is the corner plate's.
pub fn short_word(line: &str) -> String {
    let head = line.split(':').next().unwrap_or("").trim();
    let mut out = String::new();
    for word in head.split_whitespace() {
        let next = if out.is_empty() { word.to_string() } else { format!("{out} {word}") };
        if next.chars().count() > WORD_LETTERS {
            break;
        }
        out = next;
    }
    // A single word longer than a line is cut where it stands.
    if out.is_empty() { head.chars().take(WORD_LETTERS).collect() } else { out }
}

/// The floating marks of the view of knight `focus`, and where each lies:
/// only that knight's own, one at a time. A gauge while their hands are at a
/// beat or a set; else, for a moment after, the few words they last said.
/// (A teacher's name is the corner plate's, in what is offered.) Nothing floats
/// for the other knights: a party's gauges are not stacked on one another. A mark stands
/// above the knight and is shifted clear of the tally board and of the corner
/// plate (`avoid`, kingdom pixels) if it would lie on them.
pub fn floats(run: &Run, focus: Option<u32>, avoid: &[(i32, i32, i32, i32)]) -> Vec<Float> {
    let Some(id) = focus.or_else(|| run.players.keys().next().copied()) else { return Vec::new() };
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else { return Vec::new() };
    let Some(hand) = run.hands.seats.get(&id) else { return Vec::new() };
    let (what, img) = if let Some(m) = hand.meter(run.tick) {
        (Floating::Gauge, gauge(m, run.tick))
    } else if let Some((_, line)) = hand.said.as_ref().filter(|(t, _)| run.tick.saturating_sub(*t) < WORD_TICKS) {
        (Floating::Word, plate(&[(short_word(line), 'H')]))
    } else {
        return Vec::new();
    };
    let feet = ((hero.x * UNIT).round() as i32, (hero.y * UNIT).round() as i32);
    // Above the head; else further up; else to either side of it.
    let (w, h) = (img.w, img.h);
    let mut tries = vec![(0, 0)];
    tries.extend([(0, -12), (0, -24), (w / 2 + 14, 0), (-(w / 2 + 14), 0), (0, 18)]);
    let mut at = (feet.0 - w / 2, feet.1 - FLOAT_ABOVE - h);
    for (dx, dy) in tries {
        let cand = (feet.0 - w / 2 + dx, feet.1 - FLOAT_ABOVE - h + dy);
        at = cand;
        if !avoid.iter().any(|&r| touches((cand.0, cand.1, w, h), r, 1)) {
            break;
        }
    }
    vec![Float { what, img, at }]
}

/// The work at the Keep: over the knight whose view it is, their own gauge or
/// few words, the stone being set hanging over its seat, the peg where the next
/// one goes. Laid after the knights.
pub(super) fn work(over: &mut Img, run: &Run, origin: (i32, i32), view: (i32, i32), figures: Option<&mut Vec<Figure>>, focus: Option<u32>) {
    if !works::on_map() || run.room().kind != RoomKind::Kingdom {
        return;
    }
    let Some(site) = run.sites.keep() else { return };
    let Some(castle) = site.castle() else { return };
    let mut put = Put { over, figures, origin, view };
    let shown = run.site_shown(site);
    let place = |p: [f32; 2]| {
        let w = castle.world([p[0], p[1], 0.0]);
        (w[0].round() as i32, w[2].round() as i32)
    };
    // The knight whose view it is has their own small gauge or their few
    // words over their head, and nothing of the others' (`floats`).
    if focus.is_some() {
        let mut avoid = vec![crew::board_rect(run, site)];
        avoid.extend(corner_boxes(run, focus, origin));
        for f in floats(run, focus, &avoid) {
            let (x, y, w, h) = f.rect();
            put.stand(&f.img, (x + w / 2, y + h), false, 0, 4.0);
        }
    }
    // The stone being set hangs over its seat, lowering as the taps land.
    let now = run.tick;
    let mut hung = false;
    for hand in run.hands.seats.values() {
        let Some(hands::Work::Set { index, began, plumb, tapped, lands, .. }) = hand.work else { continue };
        if index >= castle.len() {
            continue;
        }
        let b = castle.stones[index as usize].aabb().map(|v| f32::from(v) / 100.0);
        let (cx, cy) = ((b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0);
        // How high above its seat, metres: held up, then let down in two.
        let held = 0.9f32;
        let up = if lands != 0 {
            let t = now.saturating_sub(lands.saturating_sub(hands::SETTLE)) as f32 / hands::SETTLE as f32;
            0.25 * (1.0 - t.clamp(0.0, 1.0))
        } else if plumb.is_some() {
            let t = now.saturating_sub(tapped) as f32 / 45.0;
            0.25 + (held - 0.25) * (1.0 - t.clamp(0.0, 1.0)) * 0.45
        } else {
            let t = now.saturating_sub(began) as f32 / 12.0;
            held * t.clamp(0.0, 1.0)
        };
        // The stone's seat is its own base: lift is its base height and the hang.
        let lift = (b[2] + up) * works::PX_PER_M;
        put.stand(&stone(), place([cx, cy]), false, lift.round() as i32, 2.0);
        hung = true;
    }
    // The stone just set rings once on its bed: a flash of chalk dust spreading.
    if let Some((index, at)) = run.hands.yard.settled
        && now.saturating_sub(at) < 14
        && index < castle.len()
    {
        let age = now.saturating_sub(at) as f32;
        let b = castle.stones[index as usize].aabb().map(|v| f32::from(v) / 100.0);
        let mut ring = Img::new(30, 18);
        let (rx, ry) = (4.0 + age * 0.9, 2.0 + age * 0.5);
        for k in 0..32 {
            let a = k as f32 / 32.0 * std::f32::consts::TAU;
            ring.put((15.0 + a.cos() * rx) as i32, (9.0 + a.sin() * ry) as i32, if age < 5.0 { 'W' } else { 'i' });
        }
        put.stand(&ring, place([(b[0] + b[3]) / 2.0, (b[1] + b[4]) / 2.0]), false, (b[5] * works::PX_PER_M).round() as i32, 2.5);
    }
    // Where the next stone goes, pegged, while a setter has the tool in hand.
    let setters = run.hands.seats.values().any(|h| h.trade == Some(Trade::Setter));
    if setters && !hung && shown < castle.len() {
        let [cx, cy] = hands::stone_at(castle, shown);
        let at = place([cx, cy]);
        put.stand(&peg(), at, false, 0, 1.0);
        if (run.tick / 12) % 2 == 0 {
            put.lay(&Img::from_rows(&["6..6", ".66.", ".66.", "6..6"]), (at.0 - 2, at.1 - 2));
        }
    }
}

/// The trade plate in a view's corner for the knight `focus` looks through: their trade
/// and how far they are in it, what is here to do, and the stock of the chain.
/// It lies at the view's foot, left, clear of the work.
pub(crate) fn hud(into: &mut Img, run: &Run, focus: Option<u32>) {
    // The builder's strip takes the view's foot while it is out (`hillside_art::hud`).
    if focus.is_some_and(|id| crate::rules::hillside::building(run, id)) {
        return;
    }
    let lines = hud_lines(run, focus);
    if lines.is_empty() {
        return;
    }
    let p = plate(&lines);
    super::hud_log::note("trade", (5, into.h - p.h - 4, p.w, p.h));
    into.stamp(&p, 5, into.h - p.h - 4);
}

/// The corner plates of the view of `focus` whose top left is `origin`, in
/// the kingdom's pixels: the progress readout at the top left and the trade
/// plate at the foot. A floating mark keeps off them.
pub(crate) fn corner_boxes(run: &Run, focus: Option<u32>, origin: (i32, i32)) -> Vec<(i32, i32, i32, i32)> {
    let (x, y, w, h) = super::super::stand_back::corner_rect(run, focus);
    let mut out = vec![(origin.0 + x, origin.1 + y, w, h)];
    let lines = hud_lines(run, focus);
    if !lines.is_empty() {
        let p = plate(&lines);
        out.push((origin.0 + 5, origin.1 + NATIVE_H - p.h - 4, p.w, p.h));
    }
    out
}

/// The lines of the trade plate for the knight `focus` looks through.
fn hud_lines(run: &Run, focus: Option<u32>) -> Vec<(String, char)> {
    let Some(id) = focus.or_else(|| run.players.keys().next().copied()) else { return Vec::new() };
    let Some(hero) = run.players.get(&id) else { return Vec::new() };
    let key = crate::rules::home::Home::profile_key(hero);
    let reading = run.trades();
    let mut lines: Vec<(String, char)> = Vec::new();
    let hand = run.hands.seats.get(&id);
    match hand.and_then(|h| h.trade) {
        Some(t) => {
            let deeds = reading.deeds(&key, t);
            let rank = Rank::of(deeds).unwrap_or(Rank::Apprentice);
            let next = Rank::next_at(deeds).map_or(String::new(), |n| format!("/{n}"));
            lines.push((format!("{} {} {}{}", t.name(), rank.name(), deeds, next), 'o'));
        }
        None => lines.push(("NO TRADE: A TEACHER AT THE BAILEY".to_string(), 'j')),
    }
    if let Some(p) = run.hand_prompt(id) {
        lines.push((p, 'H'));
    }
    if hand.is_some_and(|h| h.trade.is_some()) || !reading.held(&key).is_empty() {
        let y = &run.hands.yard;
        let load = match hand.map(|h| h.carry) {
            Some(Carry::Stone) => " STONE ON THE HOD",
            Some(Carry::Mortar) => " MORTAR ON THE HOD",
            _ => "",
        };
        lines.push((format!("ROUGH {} CUT {} WALL {} MORTAR {}{}", run.yard_view().held(), y.cut, y.course, y.mortar, load), 'G'));
    }
    if let Some(site) = run.sites.keep()
        && site.tally().iter().any(|&(seat, _)| seat != 0)
    {
        // Who has set how many of the stones, the most first: the tally board, to hand.
        let mut line = String::from("TALLY");
        for (seat, n) in site.tally().into_iter().take(3) {
            let name = if seat == 0 { "REALM".to_string() } else { run.players.get(&u32::from(seat)).map_or_else(|| format!("SEAT{seat}"), |h| h.name.to_uppercase()) };
            line.push_str(&format!(" {} {}", name.chars().take(6).collect::<String>(), n));
        }
        if site.true_count() > 0 {
            line.push_str(&format!(" TRUE {}", site.true_count()));
        }
        lines.push((line, 'h'));
    }
    if reading.holds(&key, Trade::ClerkOfWorks)
        && let Some(castle) = run.sites.keep().and_then(|s| s.castle())
    {
        lines.push((run.clerk_line(castle), 'y'));
    }
    let _ = trades::MASTER;
    // Stood back, only who they are and the tally: the rest is for the close view.
    if super::super::stand_back::on() {
        let first = lines.first().cloned();
        let tally = lines.iter().find(|(t, _)| t.starts_with("TALLY")).cloned();
        lines = first.into_iter().chain(tally).collect();
    }
    // No line runs past the view: a long one goes on a second.
    lines.into_iter().flat_map(|(t, ink)| wrap(&t, HUD_WIDTH).into_iter().map(move |l| (l, ink))).collect()
}

/// How wide the corner plate's text may be, pixels: the view less its margins.
const HUD_WIDTH: i32 = NATIVE_W - 20;

/// `text` broken at spaces into lines no wider than `width` pixels.
fn wrap(text: &str, width: i32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match out.last_mut() {
            Some(line) if super::super::super::ink::text_width(&format!("{line} {word}")) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => out.push(word.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Input;
    use std::collections::BTreeMap;

    #[test]
    fn the_gauges_and_props_draw() {
        assert!(outcrop(8).solid(12, 12) && outcrop(8).solid(5, 24));
        assert!(tub(9).solid(5, 8));
        assert!(tracing_floor().solid(3, 3));
        assert!(stone().solid(4, 2));
        let beat = gauge(Meter::Beat { job: hands::Job::Bench, off: 0.5, hits: 2, need: 4 }, 0);
        assert!(beat.solid(9, 9) || beat.solid(9, 2));
        let set = gauge(Meter::Set { bob: 0.3, bubble: None, plumb: None, level: None, landed: false }, 0);
        assert!(set.w >= 50);
    }

    /// A party of three at the course, set to work at their own beats, and one
    /// more knight at the tally board and a teacher.
    fn three_at_the_wall() -> Run {
        let laid = 1_837;
        let castle = works::castle(works::KEEP).expect("the great tower");
        let spots: Vec<[f32; 2]> = (0..3).map(|k| {
            let stone = hands::stone_at(castle, laid + k);
            [stone[0] + k as f32 * 1.2 - 1.2, stone[1] - 2.6]
        }).collect();
        let mut run = crate::world3d::kingdom::proof_run(&[[0.0; 2], [0.0; 2], [0.0; 2]]);
        run.lay_to("great-tower", laid);
        run.hands.yard.timber = 4_000;
        for (k, p) in spots.iter().enumerate() {
            let id = k as u32 + 1;
            let h = run.players.get_mut(&id).expect("a seat");
            (h.x, h.y) = hands::units(*p);
            h.camera = Some((h.x, h.y - 1.0));
            h.stone = false;
            h.idle = false;
            run.teach_trade(id, Trade::Setter, 34);
        }
        run.hands.yard.course = 12;
        run.hands.yard.mortar = 30;
        run
    }

    #[test]
    #[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib hands_art -- --include-ignored"]
    fn a_party_at_work_floats_one_small_thing_apiece_and_nothing_lies_on_anything() {
        let mut run = three_at_the_wall();
        let site = run.sites.keep().expect("a site").clone();
        let mut seen = [0usize; 3];
        let mut words = std::collections::BTreeSet::new();
        for t in 0..1_500u64 {
            // Each knight swings at their own beat and now and then at a loss:
            // gauges, words, and the stones set one after another.
            let inputs: BTreeMap<u32, Input> = (1..=3u32)
                .map(|id| (id, Input { swing: (t + 11 * u64::from(id)) % (7 + u64::from(id)) == 0, interact: t % 97 < 4, ..Default::default() }))
                .collect();
            run.hands.yard.course = run.hands.yard.course.max(6);
            run.hands.yard.mortar = run.hands.yard.mortar.max(12);
            run.step(&inputs);
            for focus in 1..=3u32 {
                let origin = camera(&run, Some(focus));
                let mut avoid = vec![crew::board_rect(&run, &site)];
                avoid.extend(corner_boxes(&run, Some(focus), origin));
                let floating = floats(&run, Some(focus), &avoid);
                assert!(floating.len() <= 1, "tick {t}: {} marks float over knight {focus}", floating.len());
                for f in &floating {
                    seen[focus as usize - 1] += 1;
                    if f.what == Floating::Word {
                        words.insert(short_word(&run.hands.seats[&focus].said.as_ref().expect("a word").1));
                    }
                    let (most_w, most_h) = if f.what == Floating::Gauge { (52, 33) } else { (170, 14) };
                    assert!(f.img.w <= most_w && f.img.h <= most_h, "a floating {:?} is small: {}x{}", f.what, f.img.w, f.img.h);
                    for r in &avoid {
                        assert!(!touches(f.rect(), *r, 0), "tick {t}: knight {focus}'s {:?} {:?} lies on {r:?}", f.what, f.rect());
                    }
                }
            }
        }
        assert!(seen.iter().all(|&n| n > 50), "every knight's gauge and words were shown: {seen:?}");
        assert!(words.len() >= 2, "the words said were several: {words:?}");
        assert!(words.iter().all(|w| w.chars().count() <= WORD_LETTERS), "{words:?}");
    }

    #[test]
    #[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib hands_art -- --include-ignored"]
    fn a_mark_beside_the_tally_board_and_the_corner_plates_moves_clear_of_them() {
        let mut run = three_at_the_wall();
        let site = run.sites.keep().expect("a site").clone();
        // A clerk a few paces south of the board, a word on their lips: it
        // would stand on the board's foot were nothing in the way.
        run.teach_trade(1, Trade::ClerkOfWorks, 12);
        let h = run.players.get_mut(&1).expect("a seat");
        (h.x, h.y) = hands::units([hands::BOARD[0], hands::BOARD[1] - 4.4]);
        run.hands.seats.get_mut(&1).expect("hands").said = Some((run.tick, "THE LEDGER READS".to_string()));
        let origin = camera(&run, Some(1));
        let mut avoid = vec![crew::board_rect(&run, &site)];
        avoid.extend(corner_boxes(&run, Some(1), origin));
        // Where it would stand if nothing were in the way, it is on the board.
        let floating = floats(&run, Some(1), &[]);
        assert!(touches(floating[0].rect(), avoid[0], 0), "the board is under the knight's word: {:?} {:?}", floating[0].rect(), avoid[0]);
        let floating = floats(&run, Some(1), &avoid);
        assert!(avoid.iter().all(|r| !touches(floating[0].rect(), *r, 0)), "{:?} lies on {avoid:?}", floating[0].rect());
    }

    #[test]
    fn a_word_is_what_comes_before_the_colon_and_short() {
        assert_eq!(short_word("A PERFECT SET: THE STONE IS TRUE"), "A PERFECT SET");
        assert_eq!(short_word("WAITING ON THE CARPENTER: THE BAY IS NOT RAISED"), "WAITING ON THE CARPENTER");
        // Never cut in the middle of a word.
        assert_eq!(short_word("A VERY LONG LINE WITHOUT ANY COLON IN IT AT ALL"), "A VERY LONG LINE WITHOUT ANY");
    }
}
