//! Lightgallop's stage: the Moonwell Road at dusk, as labelled blocks until
//! the art wave paints them (assets/lightgallop/slots.json, ART.md).
//!
//! `layers` is what a seat sees (384 by 224): its own camera on the shared
//! road, led ahead of the way it rides and drawn back when it is very fast.
//! The terrain is drawn from the very curves the rules ride on, in native
//! pixels, and the rider is a block turned about its centre by the lean the
//! rules keep (so it rolls clean round the loop, with no sprite frames to
//! jitter). Art never sets a hitbox or a curve.
use super::super::ink::{Img, Rgb, bayer, hash, shade, text_width};
use super::roundtable::{self as shell, Look};
use crate::rules::lightgallop::{
    self as rules, Session,
    level::level,
    math::{self, Q},
    rider::{GAUGE_MAX, Rider},
    session::{REFORM, WARDEN_FALL},
    terrain::Kind,
    world::{CrateKind, FoeKind, SpringKind, foe_at, foe_body},
};
use crate::rules::{HZ, Run};
use std::{collections::BTreeMap, sync::OnceLock};

const LOOK: Look = Look {
    name: rules::NAME,
    backdrop: 'F',
    edge: 'y',
    accent: 'T',
};

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus)
}

/// The same picture on the terminal's route: its cells are four pixels wide,
/// so the cockpit's text rows (`art/roundtable/lightgallop.rs`) are laid over
/// the top bar and the tally to say the clock, the sparks and the rank.
pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus)
}

fn draw(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let mut im = Img::black(384, 224);
    let Some(s) = run.lightgallop.as_ref() else {
        return (im, Img::new(384, 224));
    };
    let id = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied());
    if let Some((id, seat)) = id.and_then(|id| s.seats.get(&id).map(|seat| (id, seat))) {
        let r = &seat.rider;
        // A heavy blow jolts the picture, never the heads-up display.
        let amp = (i32::from(r.shake) + 2) / 4;
        let jolt = |k: u32| if amp == 0 { 0 } else { hash(s.elapsed as i32, id as i32, k) as i32 % (2 * amp + 1) - amp };
        let v = View { cx: r.cam_x + jolt(1), cy: r.cam_y + jolt(2), zoom: r.zoom, w: 384, h: 224 };
        let words = hud_words(s, &seat.rider);
        // What the riders' tags keep clear of: the heads-up display's words,
        // and the tally's panel when it is up.
        let mut taken: Vec<Rect> = words.iter().map(Word::rect).collect();
        if s.tally.is_some() {
            taken.push(PANEL);
        }
        scene(&mut im, s, id, &v, &taken);
        hud(&mut im, s, id, &words);
    }
    (im, Img::new(384, 224))
}

pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    shell::stub_kingdom(im, origin, rules::ENTRY, &LOOK);
}

// ---- the view -------------------------------------------------------------

/// A window on the road: its top-left in world pixels, the scale (256 is
/// native) and the size of the picture.
#[derive(Clone, Copy)]
pub struct View {
    pub cx: i32,
    pub cy: i32,
    pub zoom: i32,
    pub w: i32,
    pub h: i32,
}

impl View {
    fn sx(&self, wx: i32) -> i32 {
        ((wx - self.cx) * self.zoom).div_euclid(256)
    }
    fn sy(&self, wy: i32) -> i32 {
        ((wy - self.cy) * self.zoom).div_euclid(256)
    }
    /// Screen column to world column.
    fn wx(&self, sx: i32) -> i32 {
        self.cx + (sx * 256).div_euclid(self.zoom)
    }
    fn q(&self, wq: i32, c: i32) -> i32 {
        (((i64::from(wq) - i64::from(c) * i64::from(Q)) * i64::from(self.zoom)).div_euclid(i64::from(Q) * 256)) as i32
    }
    fn qx(&self, xq: i32) -> i32 {
        self.q(xq, self.cx)
    }
    fn qy(&self, yq: i32) -> i32 {
        self.q(yq, self.cy)
    }
    fn len(&self, l: i32) -> i32 {
        (l * self.zoom).div_euclid(256)
    }
}

// ---- the slots ---------------------------------------------------------------

#[derive(serde::Deserialize)]
struct Slot {
    id: String,
    size: [i32; 2],
    ink: char,
    label: String,
    states: BTreeMap<String, Vec<u32>>,
}

struct Anim {
    frames: Vec<Img>,
    /// The same frames without the words their blocks carry (the frame itself
    /// when it carries none, as a painted one will not).
    bare: Vec<Img>,
    /// The words each placeholder block carries, for `stamp_block`.
    names: Vec<Vec<Name>>,
    durations: Vec<u32>,
    /// Painted art (the whole slot is), not the labelled block.
    painted: bool,
}

/// A word on a placeholder block: where its letters start in the block's own
/// pixels, its ink, and whether it sits on a black plate. Drawn at a scale
/// other than native, the block is stamped without it and the word is set on
/// top at its own size (`stamp_block`): scaled with the block, the letters
/// would be dragged into mush.
struct Name {
    x: i32,
    y: i32,
    text: String,
    ink: char,
    plate: bool,
}

/// The reviewed frame `key` of slot `s`, as the stage draws it: its opaque
/// pixels only (a pixel under half opacity is transparent).
fn painted_frame(s: &Slot, key: &str) -> Option<Img> {
    let bytes = super::slot_art::bytes(&s.id, key)?;
    let png = image::load_from_memory(bytes).expect("Lightgallop slot").to_rgba8();
    assert_eq!([png.width() as i32, png.height() as i32], s.size, "slot registration");
    let mut im = Img::new(s.size[0], s.size[1]);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] >= 128 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    Some(im)
}

/// Every slot's frames: the block until the art arrives (`slot_art`). A slot
/// is painted whole or not at all: with one frame of any state missing, every
/// frame of every state is the labelled block, so a prop never changes look
/// when it changes state.
fn bank() -> &'static BTreeMap<(String, String), Anim> {
    static BANK: OnceLock<BTreeMap<(String, String), Anim>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> = serde_json::from_str(include_str!("../../../../assets/lightgallop/slots.json"))
            .expect("Lightgallop slots");
        let mut out = BTreeMap::new();
        for s in slots {
            let painted: Option<BTreeMap<&String, Vec<Img>>> = s
                .states
                .iter()
                .map(|(state, durations)| {
                    (0..durations.len())
                        .map(|i| painted_frame(&s, &format!("{state}-{i}")))
                        .collect::<Option<Vec<_>>>()
                        .map(|frames| (state, frames))
                })
                .collect();
            if let Some(painted) = &painted {
                for (state, frames) in painted {
                    out.insert(
                        (s.id.clone(), (*state).clone()),
                        Anim {
                            names: frames.iter().map(|_| Vec::new()).collect(),
                            bare: frames.clone(),
                            frames: frames.clone(),
                            durations: s.states[*state].clone(),
                            painted: true,
                        },
                    );
                }
                continue;
            }
            for (state, durations) in &s.states {
                let made: Vec<(Img, Img, Vec<Name>)> = (0..durations.len())
                    .map(|i| {
                        let (baked, names) = block(&s, state, i, true);
                        (baked, block(&s, state, i, false).0, names)
                    })
                    .collect();
                let (mut frames, mut bare, mut names) = (Vec::new(), Vec::new(), Vec::new());
                for (f, b, n) in made {
                    frames.push(f);
                    bare.push(b);
                    names.push(n);
                }
                out.insert((s.id.clone(), state.clone()), Anim { frames, bare, names, durations: durations.clone(), painted: false });
            }
        }
        out
    })
}

/// The labelled block standing in for one frame of a slot, and the words it
/// carries (drawn into it when `bake`).
fn block(s: &Slot, state: &str, frame: usize, bake: bool) -> (Img, Vec<Name>) {
    let (w, h) = (s.size[0], s.size[1]);
    let mut im = Img::new(w, h);
    let mut names = Vec::new();
    match s.id.as_str() {
        "lightgallop.rider" => return (rider_block(state, frame, 'z', ""), names),
        "lightgallop.spark" => {
            let c = if frame % 2 == 0 { '6' } else { '5' };
            for k in 0..5 {
                im.rect(5 - k, k, 2 * k + 1, 1, c);
                im.rect(5 - k, 9 - k, 2 * k + 1, 1, c);
            }
            im.rect(4, 4, 2, 2, 'w');
            return (im, names);
        }
        "lightgallop.hay" => {
            let squash = if state == "sprung" { 6 } else { 0 };
            im.rect(1, 4 + squash, w - 2, h - 4 - squash, 'O');
            im.rect(1, 4 + squash, w - 2, 2, 't');
            im.frame(1, 4 + squash, w - 2, h - 4 - squash, 'R');
            im.rect(w / 2 - 1, 4 + squash, 2, h - 4 - squash, '7');
            names.push(Name { x: w / 2 - 8, y: h - 12, text: "HAY".into(), ink: 'b', plate: false });
            return (finish(im, &names, bake), names);
        }
        "lightgallop.pad" => {
            im.rect(0, 2, w, h - 2, 'S');
            im.frame(0, 2, w, h - 2, '2');
            for k in 0..4 {
                let x = 4 + k * 9 + (frame as i32 * 2) % 9;
                im.line(x, 3, x + 3, 5, '3');
                im.line(x + 3, 5, x, 7, '3');
            }
            return (im, names);
        }
        "lightgallop.post" => {
            im.rect(w / 2 - 1, 12, 3, h - 12, 'g');
            im.rect(w / 2 - 5, 10, 11, 2, 'g');
            let lit = state == "lit";
            im.rect(w / 2 - 4, 0, 9, 11, if lit { '5' } else { 's' });
            im.frame(w / 2 - 4, 0, 9, 11, if lit { '4' } else { 'S' });
            if lit {
                im.rect(w / 2 - 2, 2 + (frame as i32 % 2), 5, 6, '6');
            }
            return (im, names);
        }
        "lightgallop.bell" => {
            let swing = if state == "ring" { [-6, -2, 6, 2, -4, 0][frame % 6] } else { 0 };
            im.rect(2, 0, w - 4, 4, 'P');
            im.rect(4, 4, 4, h - 4, 'P');
            im.rect(w - 8, 4, 4, h - 4, 'P');
            let bx = w / 2 - 10 + swing;
            im.rect(bx, 18, 20, 30, '4');
            im.rect(bx - 3, 44, 26, 6, '4');
            im.frame(bx - 3, 44, 26, 6, 'a');
            names.push(Name { x: bx - 6, y: 26, text: "BELL".into(), ink: 'a', plate: false });
            return (finish(im, &names, bake), names);
        }
        "lightgallop.crate" => {
            im.rect(0, 0, w, h, 'o');
            im.frame(0, 0, w, h, 'P');
            im.line(0, 0, w - 1, h - 1, 'P');
            im.line(w - 1, 0, 0, h - 1, 'P');
            let c = match state {
                "shield" => 'z',
                "spur" => '4',
                _ => '6',
            };
            im.rect(w / 2 - 4, h / 2 - 4, 8, 8, c);
            return (im, names);
        }
        _ => {}
    }
    // The plain block: dark fill, a frame in the slot's ink, a cross, a label.
    let layer = s.id.contains("layer");
    im.rect(1, 1, (w - 2).max(0), (h - 2).max(0), if layer { 'f' } else { 'X' });
    im.frame(0, 0, w, h, s.ink);
    im.line(2, 2, w - 3, h - 3, s.ink);
    if w >= 24 && h >= 14 {
        let lx = ((w - text_width(&s.label)) / 2).max(1);
        names.push(Name { x: lx, y: h / 2 - 3, text: s.label.clone(), ink: s.ink, plate: true });
    }
    if matches!(s.id.as_str(), "lightgallop.snuffer" | "lightgallop.floater" | "lightgallop.warden") {
        // Two pale eyes, so which way it is looking can be read.
        let ey = h / 4;
        im.rect(w / 4, ey, 3, 3, 'w');
        im.rect(w / 4 + 7, ey, 3, 3, 'w');
    }
    (finish(im, &names, bake), names)
}

/// `im` with its words drawn in, if they are to be.
fn finish(mut im: Img, names: &[Name], bake: bool) -> Img {
    if bake {
        for n in names {
            let word = Word::new(n.x, n.y, &n.text, n.ink, n.plate);
            let (x, y, w, h) = word.rect();
            if n.plate {
                im.rect(x, y, w, h, 'k');
            }
            im.text(n.x, n.y, &n.text, n.ink);
        }
    }
    im
}

/// The mounted knight, facing right, in a 48 by 40 canvas whose pivot is the
/// body's centre at (24, 24): the horse's head at the right, a lance when it
/// lunges, the knight's colour on the surcoat and a seat digit above.
fn rider_block(state: &str, frame: usize, colour: char, label: &str) -> Img {
    let mut im = Img::new(48, 40);
    if matches!(state, "roll" | "charge") {
        // The tuck roll: a tight ball, a spiral, the knight's colour on top.
        let spin = frame as i32 % 4;
        im.ellipse(24.0, 24.0, 8.0, 8.0, 'R');
        im.ellipse(24.0, 24.0, 6.0, 6.0, 'p');
        im.ellipse(24.0, 24.0, 3.0, 3.0, colour);
        for k in 0..3 {
            let a = (k * 85 + spin * 21) % 256;
            let (c, s) = (math::cos(a), math::sin(a));
            im.put(24 + 7 * c / 4096, 24 + 7 * s / 4096, 'h');
        }
        im.frame(15, 15, 18, 18, if state == "charge" { 'T' } else { 'u' });
        return im;
    }
    let (sway, lift, lean) = match state {
        "gallop" => ([-6, -2, 3, 6, 2, -3][frame % 6], 0, 1),
        "spur" => ([-7, -3, 3, 7][frame % 4], 2, 3),
        "jump-rise" => (-8, 8, 0),
        "jump-fall" => (7, 5, 1),
        "lunge" => (8, 6, 4),
        "skid" => (2, -2, -2),
        "crouch" => (0, -3, 2),
        "hurt" => (0, 0, -3),
        _ => (0, 0, 0),
    };
    // The horse (bay), its legs reaching with the gait.
    let body = if state == "crouch" { 17 } else { 14 };
    im.rect(10, body, 28, 11, 'R');
    im.rect(10, body, 28, 3, 'o');
    im.rect(34, body - 7, 10, 9, 'R');
    im.rect(41, body - 4, 5, 6, 'p');
    im.rect(35, body - 9, 2, 3, 'p');
    im.rect(29, body - 5, 6, 6, 'b');
    im.line(10, body + 2, 3, body + 8 + lift / 2, 'b');
    let foot = 38 - lift;
    for (x, k) in [(33, sway), (28, -sway / 2), (17, -sway), (13, sway / 2)] {
        im.line(x, body + 10, x + k, foot, 'p');
        im.put(x + k, foot, 'b');
    }
    // The knight: surcoat in his colour, a steel helm and a red plume.
    let top = 2 + i32::from(state == "crouch") * 6;
    im.rect(20 + lean, top + 4, 9, 11, colour);
    im.rect(21 + lean, top, 8, 6, 'h');
    im.rect(21 + lean, top + 2, 8, 1, 'k');
    im.rect(17 + lean, top, 4, 2, '7');
    if state == "lunge" || state == "spur" {
        im.line(27 + lean, top + 9, 47, top + 13, 'H');
        im.put(47, top + 13, 'W');
    }
    if state == "victory" {
        im.line(26, top + 5, 30, top - 4, colour);
    }
    // A pale frame at the canvas's edge, so the block reads as a block; red
    // when he is hurt.
    im.frame(0, 0, 48, 40, if state == "hurt" { '7' } else { 'j' });
    if !label.is_empty() {
        let tw = text_width(label);
        im.rect(2, 2, tw + 2, 9, 'k');
        im.text(3, 3, label, colour);
    }
    im
}

/// Whether slot `id` is painted art (every frame of every state), not the
/// labelled blocks.
fn painted(id: &str) -> bool {
    bank().range((id.to_owned(), String::new())..).next().is_some_and(|((slot, _), anim)| slot == id && anim.painted)
}

/// The animation of `state` and the index of its frame at `tick` (looped).
fn anim_at(id: &str, state: &str, tick: u32) -> (&'static Anim, usize) {
    let anim = &bank()[&(id.to_owned(), state.to_owned())];
    let total: u32 = anim.durations.iter().sum();
    let mut time = tick % total.max(1);
    let mut index = 0;
    while index + 1 < anim.frames.len() && time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    (anim, index)
}

/// The frame of `state` at `tick` (looped).
fn frame_of(id: &str, state: &str, tick: u32) -> &'static Img {
    let (anim, index) = anim_at(id, state, tick);
    &anim.frames[index]
}

// ---- drawing helpers -----------------------------------------------------------

/// A word set on the picture. Every word the stage prints is one of these, so
/// where it stands can be read back (and kept clear of the others) in a test.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Word {
    x: i32,
    y: i32,
    text: String,
    ink: char,
    /// Set on a black plate (one pixel round the letters) so it reads over
    /// any picture; a word on a panel is left bare.
    backed: bool,
    /// Wears the laurel: a small crown in the word's ink in the two blanks the
    /// text starts with (the font has no star to mark the best rider with).
    laurel: bool,
}

impl Word {
    fn new(x: i32, y: i32, text: &str, ink: char, backed: bool) -> Word {
        Word { x, y, text: text.to_owned(), ink, backed, laurel: false }
    }

    /// Centred on column `cx`, on a black plate.
    fn centred(cx: i32, y: i32, text: &str, ink: char) -> Word {
        Word::new(cx - text_width(text) / 2, y, text, ink, true)
    }

    /// The pixels the word covers, `(x, y, w, h)`: the letters (5 by 7 in a
    /// 6 pixel step), and the plate round them when it has one.
    fn rect(&self) -> (i32, i32, i32, i32) {
        let w = text_width(&self.text);
        if self.backed { (self.x - 1, self.y - 1, w + 2, 9) } else { (self.x, self.y, w, 7) }
    }
}

#[cfg(test)]
thread_local! {
    /// Every word said on this thread since the last `said`.
    static SAID: std::cell::RefCell<Vec<Word>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The same for the names on blocks, which belong to the scenery: they
    /// move with the world and are cut by the frame like the block is.
    static ON_BLOCKS: std::cell::RefCell<Vec<Word>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn say(im: &mut Img, w: &Word) {
    #[cfg(test)]
    SAID.with(|said| said.borrow_mut().push(w.clone()));
    print(im, w);
}

/// A name set over a block (`stamp_block`).
fn say_on_block(im: &mut Img, w: &Word) {
    #[cfg(test)]
    ON_BLOCKS.with(|said| said.borrow_mut().push(w.clone()));
    print(im, w);
}

fn print(im: &mut Img, w: &Word) {
    if w.backed {
        let (x, y, w, h) = w.rect();
        im.rect(x, y, w, h, 'k');
    }
    im.text(w.x, w.y, &w.text, w.ink);
    if w.laurel {
        // Three points over a band, five pixels wide, between the blanks.
        for (dx, dy) in [(0, 0), (2, 0), (4, 0)].into_iter().chain((0..5).flat_map(|dx| [(dx, 1), (dx, 2)])) {
            im.put(w.x + 3 + dx, w.y + 2 + dy, w.ink);
        }
    }
}

/// Draw the picture for `focus` and give back every word it printed, in the
/// order they were set down, and the names it set over blocks.
#[cfg(test)]
fn said_with_names(run: &Run, focus: Option<u32>) -> (Vec<Word>, Vec<Word>) {
    SAID.with(|said| said.borrow_mut().clear());
    ON_BLOCKS.with(|said| said.borrow_mut().clear());
    let _ = draw(run, focus);
    (
        SAID.with(|said| std::mem::take(&mut *said.borrow_mut())),
        ON_BLOCKS.with(|said| std::mem::take(&mut *said.borrow_mut())),
    )
}

/// The words (not the names on blocks) the picture for `focus` printed.
#[cfg(test)]
fn said(run: &Run, focus: Option<u32>) -> Vec<Word> {
    said_with_names(run, focus).0
}

/// `src` drawn about `(cx, cy)` rotated by `brads`, scaled by `scale` (256 is
/// native), nearest sample at pixel centres in integer arithmetic: the same
/// picture on every host, and no gaps in the turn.
fn stamp_turned(dst: &mut Img, src: &Img, (cx, cy): (i32, i32), pivot: (i32, i32), brads: i32, scale: i32, flip: bool) {
    let flipped;
    let src = if flip {
        flipped = src.flip_h();
        &flipped
    } else {
        src
    };
    let pivot_x = if flip { src.w - 1 - pivot.0 } else { pivot.0 };
    let (s, c) = (math::sin_b(brads), math::cos_b(brads));
    let reach = (math::isqrt((src.w * src.w + src.h * src.h) as u64) as i32 * scale / 256) / 2 + 3;
    for y in cy - reach..=cy + reach {
        for x in cx - reach..=cx + reach {
            // The pixel's centre, doubled, from the pivot; turned back.
            let (dx, dy) = (2 * (x - cx) + 1, 2 * (y - cy) + 1);
            let (ux, uy) = (i64::from(dx * c + dy * s), i64::from(-dx * s + dy * c));
            let (ux, uy) = (ux * 256 / i64::from(scale), uy * 256 / i64::from(scale));
            let ix = (i64::from(2 * pivot_x + 1) * 4096 + ux).div_euclid(8192) as i32;
            let iy = (i64::from(2 * pivot.1 + 1) * 4096 + uy).div_euclid(8192) as i32;
            if let Some(p) = src.get(ix, iy) {
                dst.set(x, y, p);
            }
        }
    }
}

/// `src` drawn with its bottom-centre at `(x, y)`, scaled (nearest).
fn stamp_scaled(dst: &mut Img, src: &Img, x: i32, y: i32, scale: i32) {
    if scale == 256 {
        dst.stamp(src, x - src.w / 2, y - src.h);
        return;
    }
    let (w, h) = ((src.w * scale / 256).max(1), (src.h * scale / 256).max(1));
    let (ox, oy) = (x - w / 2, y - h);
    for dy in 0..h {
        for dx in 0..w {
            if let Some(p) = src.get(dx * 256 / scale, dy * 256 / scale) {
                dst.set(ox + dx, oy + dy, p);
            }
        }
    }
}

/// The frame of `state` at `tick`, bottom-centre at `(x, y)`, scaled (nearest).
/// A placeholder block's words are set over it at their own size, not scaled
/// with it, so they stay readable when the camera draws back.
fn stamp_block(im: &mut Img, id: &str, state: &str, tick: u32, x: i32, y: i32, scale: i32) {
    let (anim, i) = anim_at(id, state, tick);
    if scale == 256 || anim.names[i].is_empty() {
        stamp_scaled(im, &anim.frames[i], x, y, scale);
        return;
    }
    let bare = &anim.bare[i];
    stamp_scaled(im, bare, x, y, scale);
    let (w, h) = ((bare.w * scale / 256).max(1), (bare.h * scale / 256).max(1));
    let (ox, oy) = (x - w / 2, y - h);
    for n in &anim.names[i] {
        // The word's centre, carried to the scaled block; the letters keep their size.
        let (tw, cx, cy) = (text_width(&n.text), n.x + text_width(&n.text) / 2, n.y + 3);
        let at = (ox + cx * scale / 256 - tw / 2, oy + cy * scale / 256 - 3);
        say_on_block(im, &Word::new(at.0, at.1, &n.text, n.ink, n.plate));
    }
}

/// Where the painted post's lantern hangs, from the middle of its foot:
/// right of the pole and up its length (`lightgallop.post`, 26 by 56).
const POST_LANTERN: (i32, i32) = (6, 34);

/// A circle of radius `r` about `(cx, cy)`, one pixel thick.
fn circle(im: &mut Img, cx: i32, cy: i32, r: i32, c: char) {
    let steps = (r * 8).clamp(32, 256);
    for k in 0..steps {
        let a = k * 256 / steps;
        im.put(
            cx + (i64::from(r) * i64::from(math::cos(a)) / 4096) as i32,
            cy + (i64::from(r) * i64::from(math::sin(a)) / 4096) as i32,
            c,
        );
    }
}

fn rider_ink(seat: u32) -> char {
    ['z', 't', 'M', 'R', 'V', 'Y'][(seat.max(1) as usize - 1) % 6]
}

// ---- the scene -----------------------------------------------------------------

/// Everything in the world, for the viewer `id`, through `v`; the riders'
/// number tags are set where `taken` (rectangles already holding words) is
/// not.
pub fn scene(im: &mut Img, s: &Session, id: u32, v: &View, taken: &[Rect]) {
    let lvl = level();
    let me = &s.seats[&id].rider;
    backdrop(im, v);
    terrain(im, v);
    // Roadside things first.
    for sign in &lvl.signs {
        let (sx, sy) = (v.sx(sign.x), v.sy(sign.y));
        if !(-80..v.w + 80).contains(&sx) {
            continue;
        }
        let tw = text_width(sign.words);
        im.rect(sx - 1, sy - 20, 2, 20, 'b');
        im.rect(sx - tw / 2 - 3, sy - 34, tw + 6, 14, 'P');
        im.frame(sx - tw / 2 - 3, sy - 34, tw + 6, 14, 'B');
        im.text(sx - tw / 2, sy - 31, sign.words, 'T');
    }
    for (i, p) in lvl.pads.iter().enumerate() {
        let sx = v.sx(p.x);
        if (-40..v.w + 40).contains(&sx) {
            let age = s.elapsed.saturating_sub(s.pad_t[i].saturating_sub(1));
            let f = frame_of("lightgallop.pad", "idle", if s.pad_t[i] > 0 && age < 24 { s.elapsed } else { 0 });
            stamp_scaled(im, f, sx, v.sy(p.y) + 2, v.zoom);
        }
    }
    for (i, sp) in lvl.springs.iter().enumerate() {
        let sx = v.sx(sp.x);
        if !(-40..v.w + 40).contains(&sx) {
            continue;
        }
        let sprung = s.spring_t[i] > 0 && s.elapsed + 1 - s.spring_t[i] < 8;
        // A sprung bale plays its frames over the ticks it stays sprung.
        let age = if sprung { s.elapsed + 1 - s.spring_t[i] } else { 0 };
        stamp_block(im, "lightgallop.hay", if sprung { "sprung" } else { "rest" }, age, sx, v.sy(sp.y) + 2, v.zoom);
        if sp.kind == SpringKind::Diag {
            let y = v.sy(sp.y) - 26;
            im.line(sx - 4, y + 6, sx + 6, y - 2, 'T');
            im.line(sx + 6, y - 2, sx + 1, y - 2, 'T');
            im.line(sx + 6, y - 2, sx + 6, y + 3, 'T');
        }
    }
    for (i, c) in lvl.crates.iter().enumerate() {
        let sx = v.sx(c.x);
        if (-40..v.w + 40).contains(&sx) && me.crates >> i & 1 == 0 {
            let st = match c.kind {
                CrateKind::Shield => "shield",
                CrateKind::Sparks => "sparks",
                CrateKind::Spur => "spur",
            };
            stamp_scaled(im, frame_of("lightgallop.crate", st, 0), sx, v.sy(c.y), v.zoom);
        }
    }
    for (i, p) in lvl.posts.iter().enumerate() {
        let sx = v.sx(p.x);
        if !(-40..v.w + 40).contains(&sx) {
            continue;
        }
        let lit = s.posts >> i & 1 == 1;
        let spin = s.elapsed.saturating_sub(s.post_t[i].saturating_sub(1));
        let f = frame_of("lightgallop.post", if lit { "lit" } else { "dark" }, s.elapsed);
        stamp_scaled(im, f, sx, v.sy(p.y), v.zoom);
        if lit && s.post_t[i] > 0 && spin < 30 {
            // The lantern spins as it catches: a ring of light round it.
            let r = 14 - (spin as i32 / 3).min(10);
            // Round the lantern: at the block's lantern, or at the painted post's,
            // which hangs right of the pole.
            let (lx, ly) = if painted("lightgallop.post") { (v.len(POST_LANTERN.0), -v.len(POST_LANTERN.1)) } else { (0, -46) };
            circle(im, sx + lx, v.sy(p.y) + ly, r, '6');
        }
    }
    // The bell.
    let mut over_bell = None;
    {
        let (bx, by) = lvl.bell;
        let sx = v.sx(bx);
        if (-60..v.w + 60).contains(&sx) {
            let ringing = s.bell_t.is_some();
            let f = frame_of("lightgallop.bell", if ringing { "ring" } else { "rest" }, s.elapsed);
            stamp_block(im, "lightgallop.bell", if ringing { "ring" } else { "rest" }, s.elapsed, sx, v.sy(by), v.zoom);
            if !ringing {
                // Over the bell, as tall as the bell is drawn.
                let word = Word::centred(sx, v.sy(by) - v.len(f.h) - 12, "BELL GATE", 'T');
                say(im, &word);
                over_bell = Some(word.rect());
            }
        }
    }
    // Sparks this knight has not yet taken.
    for (i, &(x, y)) in lvl.sparks.iter().enumerate() {
        let sx = v.sx(x);
        if !(-10..v.w + 10).contains(&sx) || me.taken[i / 64] >> (i % 64) & 1 == 1 {
            continue;
        }
        let f = frame_of("lightgallop.spark", "idle", s.elapsed + i as u32 * 3);
        stamp_scaled(im, f, sx, v.sy(y) + 5, v.zoom);
    }
    for l in &me.loose {
        let (sx, sy) = (v.qx(l.x), v.qy(l.y));
        let blink = l.age > 90 && l.age % 6 < 3;
        if !blink {
            stamp_scaled(im, frame_of("lightgallop.spark", "loose", s.elapsed), sx, sy + 5, v.zoom);
        }
    }
    foes(im, s, v);
    // The riders, the viewer's own on top.
    let mut order: Vec<_> = s.seats.iter().collect();
    order.sort_by_key(|(seat, _)| **seat == id);
    let mut tags = Vec::new();
    for (&seat, st) in order {
        tags.extend(rider(im, s, seat, &st.rider, v, seat != id));
    }
    let mut taken = taken.to_vec();
    taken.extend(over_bell);
    for tag in place_tags(tags, &taken) {
        say(im, &tag);
    }
    dimming(im, s, v);
}

/// The three painted strips of land (`lightgallop.layer-far`, `-mid`, `-near`,
/// 192 by 96 each, repeating along the road), with what fills the picture
/// above and below them.
struct Strip {
    img: Img,
    /// The strip's top row, as far as the top of the picture (the far strip
    /// is opaque; the others show what is behind them).
    above: Option<Img>,
    /// The strip's bottom row, down to the foot of the picture.
    below: Img,
}

/// The colour most of `img`'s row `y` holds (the lowest on a tie).
fn row_mode(img: &Img, y: i32) -> Option<Rgb> {
    let mut counts: BTreeMap<Rgb, u32> = BTreeMap::new();
    for x in 0..img.w {
        if let Some(c) = img.get(x, y) {
            *counts.entry(c).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|&(c, n)| (n, std::cmp::Reverse(c))).map(|(c, _)| c)
}

/// A solid `192` by `128` picture of `c`: more than any fill is ever deep.
fn solid(c: Rgb) -> Img {
    Img::from_rgb(192, 128, &vec![c; 192 * 128])
}

/// How much of its own light the near strip keeps. It is the loudest of the
/// three (saturated grass, a fence at the riders' height), so it is drawn at
/// 60 percent, a dark veil over the painted pixels: every pixel is shaded to
/// the nearest ink and none is repainted, its alpha is its own.
const NEAR_DIM: f32 = 0.6;

/// `img` with every opaque pixel at `k` of its light, on the palette.
fn dimmed(img: &Img, k: f32) -> Img {
    let mut out = Img::new(img.w, img.h);
    for y in 0..img.h {
        for x in 0..img.w {
            if let Some(c) = img.get(x, y) {
                out.set(x, y, shade(c, k));
            }
        }
    }
    out
}

/// The painted strips, far to near, when all three are painted; the code's
/// own bands (`backdrop`) otherwise.
fn strips() -> Option<&'static [Strip; 3]> {
    static STRIPS: OnceLock<Option<[Strip; 3]>> = OnceLock::new();
    STRIPS
        .get_or_init(|| {
            let ids = ["lightgallop.layer-far", "lightgallop.layer-mid", "lightgallop.layer-near"];
            if !ids.iter().all(|id| painted(id)) {
                return None;
            }
            let make = |id: &str, sky: bool, dim: f32| {
                let mut img = frame_of(id, "idle", 0).clone();
                if dim < 1.0 {
                    img = dimmed(&img, dim);
                }
                let below = solid(row_mode(&img, img.h - 1)?);
                let above = if sky { Some(solid(row_mode(&img, 0)?)) } else { None };
                Some(Strip { img, above, below })
            };
            Some([make(ids[0], true, 1.0)?, make(ids[1], false, 1.0)?, make(ids[2], false, NEAR_DIM)?])
        })
        .as_ref()
}

/// Where the strips' bottom edges sit in the picture when the camera is at its
/// usual height, far to near: the far strip's horizon and the treeline stand
/// above the road, the fence at its edge.
const STRIP_BASE: [i32; 3] = [142, 152, 154];
/// How much of the camera's travel each layer drifts (the land's scroll is
/// `cx / share`, its rise and fall `(cy - 520) / (share / 2)`), far to near.
const STRIP_SHARE: [i32; 3] = [16, 7, 3];

/// The painted land: each strip repeats along the road at its own share of
/// the camera's travel, as the code's bands do.
fn painted_backdrop(im: &mut Img, v: &View, strips: &[Strip; 3]) {
    for ((strip, base), share) in strips.iter().zip(STRIP_BASE).zip(STRIP_SHARE) {
        let rise = ((v.cy - 520) / (share / 2).max(1)).clamp(-30, 30);
        let bottom = base - rise;
        let top = bottom - strip.img.h;
        let from = -(v.cx.div_euclid(share).rem_euclid(strip.img.w));
        let mut x = from;
        while x < v.w {
            if let Some(above) = &strip.above {
                im.stamp(above, x, top - above.h);
            }
            im.stamp(&strip.img, x, top);
            im.stamp(&strip.below, x, bottom);
            x += strip.img.w;
        }
    }
}

/// The sky, the moon and the three bands of land that drift behind the road:
/// the painted strips when all three have arrived, the code's own until then.
fn backdrop(im: &mut Img, v: &View) {
    if let Some(strips) = strips() {
        painted_backdrop(im, v, strips);
        return;
    }
    // Dusk in bands, deepest at the top.
    let bands = [(0, 's'), (26, 'x'), (58, 'S'), (96, 'u'), (128, 'Q')];
    for y in 0..v.h {
        let mut c = 's';
        for &(from, ch) in &bands {
            if y >= from {
                c = ch;
            }
        }
        im.rect(0, y, v.w, 1, c);
    }
    // A band of ember where the sky meets the hills.
    for y in 150.min(v.h)..v.h.min(190) {
        for x in 0..v.w {
            if bayer(x, y) * 40.0 < (y - 150) as f32 {
                im.put(x, y, 'B');
            }
        }
    }
    // The great pale moon.
    let (mx, my) = (292 - (v.cx / 40) % 400, 44);
    im.ellipse(mx as f32, my as f32, 28.0, 28.0, 'V');
    im.ellipse((mx - 4) as f32, (my - 3) as f32, 22.0, 22.0, 'W');
    for (dx, dy, r) in [(-9, -6, 4.0), (6, 8, 5.0), (12, -10, 3.0)] {
        im.ellipse((mx + dx) as f32, (my + dy) as f32, r, r, 'v');
    }
    // Three bands of land, each scrolling a different share.
    for (share, base, colour, amp, period) in [(16, 168, 'x', 34, 330), (7, 190, 'f', 26, 170), (3, 214, 'D', 18, 90)] {
        let scroll = v.cx / share;
        for sx in 0..v.w {
            let wx = scroll + sx;
            let t = (wx * 256).div_euclid(period);
            let h = amp / 2
                + (i64::from(math::sin(t)) * i64::from(amp) / 2 / 4096) as i32
                + (i64::from(math::sin(t * 3 + 40)) * i64::from(amp) / 6 / 4096) as i32;
            // The land drifts a little with the camera's height too.
            let off = (v.cy - 520) / (share / 2).max(1);
            let top = base - h - off.clamp(-30, 30);
            im.rect(sx, top, 1, (v.h - top).max(0), colour);
            // A pine on the mid band, now and then.
            if colour == 'f' && wx.rem_euclid(53) < 2 {
                let ph = 16 + (wx / 53).rem_euclid(3) * 6;
                for k in 0..ph {
                    let half = k / 3;
                    im.rect(sx - half, top - ph + k, 2 * half + 1, 1, 'f');
                }
            }
        }
    }
}

/// The painted road: a 64 by 48 tile repeated along the road's curve, and the
/// colour that carries on beneath it.
struct Road {
    tile: Img,
    deep: Rgb,
}

/// The tile row the road's surface runs along: the pale lip, with the grass
/// blades above it in the rows before it.
const ROAD_LIP: i32 = 9;

/// The painted road, when `lightgallop.road` is painted; the code's own
/// layers of earth (`terrain`) otherwise.
fn road() -> Option<&'static Road> {
    static ROAD: OnceLock<Option<Road>> = OnceLock::new();
    ROAD.get_or_init(|| {
        let id = "lightgallop.road";
        painted(id).then(|| {
            let tile = frame_of(id, "idle", 0).clone();
            let deep = row_mode(&tile, tile.h - 1)?;
            Some(Road { tile, deep })
        })?
    })
    .as_ref()
}

/// One screen column `sx` of the painted road, whose surface is at row `y` and
/// which stands over the world's column `x`: the tile's column, lip on the
/// surface (the road's curve, never the art, says where that is), the blades
/// standing over it, and below the tile its last colour.
fn painted_road_column(im: &mut Img, road: &Road, v: &View, sx: i32, x: i32, y: i32) {
    let u = x.rem_euclid(road.tile.w);
    let first = y - (ROAD_LIP * v.zoom).div_euclid(256) - 1;
    for row in first.max(0)..v.h {
        let r = ROAD_LIP + ((row - y) * 256).div_euclid(v.zoom);
        let c = if r >= road.tile.h { Some(road.deep) } else { road.tile.get(u, r) };
        if let Some(c) = c {
            im.set(sx, row, c);
        }
    }
}

/// The curves, as the rules ride them: earth under the roads, planks for the
/// rails, a stone band for the loops.
fn terrain(im: &mut Img, v: &View) {
    let lvl = level();
    let t = &lvl.terrain;
    let road = road();
    // The roads, a column at a time.
    for p in t.paths.iter().filter(|p| p.kind != Kind::Ring) {
        let (lo, hi) = p.span_px();
        for sx in 0..v.w {
            let x = v.wx(sx);
            if x < lo || x > hi {
                continue;
            }
            let y = v.sy(p.y_at_x(x) / Q);
            if p.kind == Kind::Road {
                if let Some(road) = road {
                    painted_road_column(im, road, v, sx, x, y);
                    continue;
                }
                for row in y.max(0)..v.h {
                    let d = row - y;
                    let c = if d < 1 {
                        'M'
                    } else if d < 4 {
                        'l'
                    } else if d < 8 {
                        'P'
                    } else if d < 20 {
                        if hash(x, row, 5) % 7 == 0 { 'I' } else { 'B' }
                    } else if d < 60 {
                        if hash(x, row, 9) % 9 == 0 { 'I' } else { 'b' }
                    } else {
                        'n'
                    };
                    im.put(sx, row, c);
                }
            } else {
                // A rail: a plank and a shadow, held up by posts.
                im.put(sx, y, 'T');
                im.put(sx, y + 1, 'o');
                im.put(sx, y + 2, 'P');
                im.put(sx, y + 3, 'b');
                if x.rem_euclid(48) == 0 {
                    let road_y = v.sy(t.paths[lvl.main].y_at_x(x) / Q);
                    im.rect(sx - 1, y + 3, 3, (road_y - y - 3).clamp(0, 400), 'b');
                }
            }
        }
    }
    // The ends of the world: a stone wall behind the gate and past the bell.
    {
        let main = &t.paths[lvl.main];
        let (lo, hi) = main.span_px();
        for sx in 0..v.w {
            let x = v.wx(sx);
            let edge = if x < lo { Some(lo - x) } else if x > hi { Some(x - hi) } else { None };
            let Some(d) = edge else { continue };
            let y = v.sy(main.y_at_x(if x < lo { lo } else { hi }) / Q);
            let top = y - v.len(70);
            for row in top.max(0)..v.h {
                let brick = (row / 6 + sx / 12) % 2 == 0;
                let c = if row < top + 2 {
                    'h'
                } else if (row % 6 == 0) || (sx % 12 == 0 && brick) {
                    'g'
                } else if d < 10 {
                    'G'
                } else {
                    'j'
                };
                im.put(sx, row, c);
            }
        }
        // The gate: two timber posts and a banner over the start.
        let gx = v.sx(40);
        let gy = v.sy(main.y_at_x(40) / Q);
        if (-40..v.w + 40).contains(&gx) {
            im.rect(gx - 22, gy - 66, 4, 66, 'P');
            im.rect(gx + 22, gy - 66, 4, 66, 'P');
            im.rect(gx - 22, gy - 70, 48, 4, 'B');
            im.rect(gx - 18, gy - 62, 40, 10, '7');
            im.frame(gx - 18, gy - 62, 40, 10, 'T');
            im.rect(gx, gy - 59, 4, 4, '6');
        }
    }
    // The loops: a stone band outside the rider's side of the curve.
    for p in t.paths.iter().filter(|p| p.kind == Kind::Ring) {
        let (lo, hi) = p.span_px();
        if v.sx(hi + 12) < 0 || v.sx(lo - 12) > v.w {
            continue;
        }
        let thick = v.len(7).max(3);
        for i in 0..p.pts.len() - 1 {
            let (a, b) = (p.pts[i], p.pts[i + 1]);
            let d = p.dir[i];
            // Out from the rider: the opposite of his up, which is (ty, -tx).
            let (nx, ny) = (-d.1, d.0);
            for k in 0..=thick {
                let (ox, oy) = ((nx * k) >> 12, (ny * k) >> 12);
                let c = if k == 0 {
                    'h'
                } else if k == 1 {
                    'i'
                } else if k >= thick - 1 {
                    'g'
                } else if k % 3 == 0 {
                    'G'
                } else {
                    'j'
                };
                im.line(v.qx(a.0) + ox, v.qy(a.1) + oy, v.qx(b.0) + ox, v.qy(b.1) + oy, c);
            }
        }
    }
}

fn foes(im: &mut Img, s: &Session, v: &View) {
    let lvl = level();
    for (i, spot) in lvl.foes.iter().enumerate() {
        if spot.min_seats > s.provisioned {
            continue;
        }
        let st = s.foes[i];
        let (fx, fy, _) = foe_at(lvl, spot, s.elapsed);
        let (sx, sy) = (v.sx(fx), v.sy(fy));
        if !(-70..v.w + 70).contains(&sx) {
            continue;
        }
        let (_, fh) = foe_body(spot.kind);
        if st.hp == 0 {
            // Cold smoke and a warm spark for a moment.
            if spot.kind == FoeKind::Warden && st.pop > 0 && st.pop < WARDEN_FALL {
                // The warden's fall: rings of cold smoke climbing off it and a
                // fountain of warm sparks.
                let age = i32::from(st.pop);
                for k in 0..4 {
                    let r = 8 + age * 2 / 3 + k * 14;
                    if r < 90 {
                        circle(im, sx, sy - fh / 2, r, if k % 2 == 0 { 'V' } else { 'U' });
                    }
                }
                for k in 0..18i32 {
                    let h = hash(k, 3, 17) as i32;
                    let (vx, vy) = (h.rem_euclid(9) - 4, 3 + h.rem_euclid(5));
                    let x = sx + vx * age / 2;
                    let y = sy - fh / 2 - vy * age + age * age / 7;
                    im.rect(x, y, 3, 3, if k % 2 == 0 { '6' } else { '5' });
                }
            }
            let age = REFORM.saturating_sub(st.pop) as i32;
            if spot.kind != FoeKind::Warden && age < 12 {
                let r = 4 + age;
                circle(im, sx, sy - fh / 2, r, 'V');
                im.rect(sx - 1, sy - fh / 2 - 1 - age / 2, 3, 3, '6');
            }
            continue;
        }
        let (id, state) = match spot.kind {
            FoeKind::Snuffer => ("lightgallop.snuffer", if st.flinch > 0 { "hurt" } else { "idle" }),
            FoeKind::Floater => ("lightgallop.floater", "idle"),
            FoeKind::Bramble => ("lightgallop.bramble", "a"),
            FoeKind::Warden => ("lightgallop.warden", if st.flinch > 0 { "hit" } else { "idle" }),
        };
        // The placeholder blocks carry words, so they are never mirrored; the
        // art will face the way `foe_at` says.
        let f = frame_of(id, state, s.elapsed / 2);
        // Floaters hover: the block is centred on the post's height.
        let drop = if spot.kind == FoeKind::Floater { (f.h - fh) / 2 } else { 0 };
        stamp_block(im, id, state, s.elapsed / 2, sx, sy + drop, v.zoom);
        if spot.kind == FoeKind::Warden {
            // Its hits left, over its head.
            let top = sy - v.len(fh) - 10;
            let hits = i32::from(st.hp);
            for k in 0..hits {
                im.rect(sx - hits * 4 + k * 8 + 1, top, 6, 4, '7');
            }
        }
    }
}

/// How many ticks of a hit's grace show the hurt block.
const FLASH: u32 = 70;

fn rider(im: &mut Img, s: &Session, seat: u32, r: &Rider, v: &View, other: bool) -> Option<Tag> {
    let t = &level().terrain;
    if v.qx(r.x) < -60 || v.qx(r.x) > v.w + 60 {
        return None;
    }
    // Which state of the block.
    let state = if r.down > 0 || (r.hurt > FLASH && r.ctrl > 0) {
        "hurt"
    } else if r.finish.is_some() {
        "victory"
    } else if r.lock.is_some() || r.lunge > 0 {
        "lunge"
    } else if r.roll || (r.ball && !r.grounded()) {
        "roll"
    } else if r.crouch {
        if r.charge > 0 { "charge" } else { "crouch" }
    } else if r.skid {
        "skid"
    } else if !r.grounded() {
        if r.spur {
            "spur"
        } else if r.vy < 0 {
            "jump-rise"
        } else {
            "jump-fall"
        }
    } else if r.spur {
        "spur"
    } else if r.gsp.abs() < 64 {
        "idle"
    } else {
        "gallop"
    };
    // Speed lines streaming back, and hoof dust at pace.
    {
        let speed = r.speed();
        let (sx, sy) = (v.qx(r.x), v.qy(r.y));
        if speed > 1500 && r.down == 0 {
            let n = ((speed - 1500) / 100).clamp(2, 9);
            for k in 0..n {
                let h = hash(k, s.elapsed as i32 / 2, seat) as i32;
                let len = 10 + (speed - 1500) / 40 + h.rem_euclid(14);
                let dy = h.rem_euclid(40) - 34;
                let x0 = sx - r.dir * (18 + h.rem_euclid(30));
                im.line(x0, sy + dy, x0 - r.dir * len, sy + dy, if k % 2 == 0 { 'i' } else { 'v' });
            }
        }
        if r.grounded() && speed > 700 && r.down == 0 {
            for k in 0..3 {
                let h = hash(k, s.elapsed as i32, seat + 9) as i32;
                let age = (s.elapsed as i32 + k * 3) % 9;
                im.rect(
                    sx - r.dir * (6 + age * 3) + h.rem_euclid(3),
                    sy - 1 - age / 2 - h.rem_euclid(3),
                    (3 + age / 3).max(1),
                    (2 + age / 4).max(1),
                    if r.skid { 'T' } else { 'J' },
                );
            }
        }
        if r.draft && r.down == 0 {
            // In a pack's wake: wind drawn in over the horse's head.
            for k in 0..3 {
                let h = hash(k, s.elapsed as i32 / 2, seat + 5) as i32;
                let x0 = sx + r.dir * (16 + h.rem_euclid(14));
                im.line(x0, sy - 22 + k * 6, x0 + r.dir * (10 + h.rem_euclid(10)), sy - 22 + k * 6, 'V');
            }
        }
        if r.spur && r.down == 0 {
            // The lantern flame streaming from the saddle.
            for k in 0..4 {
                im.rect(sx - r.dir * (14 + k * 4), sy - 14 + k % 2, 3 - k / 2, 3, if k < 2 { '6' } else { '4' });
            }
        }
        if r.crouch && r.charge > 0 {
            let rr = 6 + r.charge / 160;
            im.frame(sx - rr, sy - rr, 2 * rr, rr, '4');
        }
    }
    // The gait quickens with the pace: a frame for every few pixels run.
    let gait = (r.x / Q / 11) as u32;
    let colour = rider_ink(seat);
    let label = format!("{seat}");
    let frame = match state {
        "gallop" | "spur" | "roll" | "idle" | "charge" => {
            let n = bank()[&("lightgallop.rider".to_owned(), state.to_owned())].frames.len() as u32;
            (gait % n) as usize
        }
        _ => 0,
    };
    let img = rider_block(state, frame, colour, &label);
    // Blink through a rider's grace after a hit.
    if r.hurt > 0 && r.down == 0 && (s.elapsed / 3) % 2 == 0 {
        return None;
    }
    let c = r.centre(t);
    let (sx, sy) = (v.qx(c.0), v.qy(c.1));
    let ang = if state == "roll" || state == "charge" {
        // A rolling ball turns as it goes, a spin of its own.
        (r.x / Q * 9) * r.dir
    } else if r.down > 0 {
        (s.elapsed as i32 * 40) * r.dir
    } else {
        r.ang
    };
    stamp_turned(im, &img, (sx, sy), (24, 24), ang, v.zoom, r.dir < 0);
    // Another knight's number over the horse, if the horse is in the picture.
    let on_screen = sx > -24 && sx < v.w + 24;
    (other && on_screen).then(|| Tag { cx: sx, y: sy - 34, text: label, ink: colour })
}

/// A rectangle, `(x, y, w, h)`.
type Rect = (i32, i32, i32, i32);

/// The number over another knight's horse, where it would like to stand.
struct Tag {
    cx: i32,
    y: i32,
    text: String,
    ink: char,
}

/// Where each tag stands: over its horse if that is clear, else shifted along
/// (to the right first, a plate and two pixels at a time, so a pack on one
/// spot reads 2 3 4 5 6 from the left), then up or down a little. A tag stays
/// inside the picture, under the heads-up display, and clear of every word in
/// `taken` and of one another; one with no clear place at all is left out (the
/// number is on the horse's block too).
fn place_tags(mut tags: Vec<Tag>, taken: &[Rect]) -> Vec<Word> {
    tags.sort_by_key(|t| (t.cx, t.text.clone()));
    let shifts = [0, 9, 18, 27, 36, 45, 54, 63, 72, -9, -18, -27, -36, -45, -54, -63, -72];
    let rises = [0, -10, 10, -20, 20, -30];
    let mut placed: Vec<Word> = Vec::new();
    for tag in tags {
        let home = Word::centred(tag.cx, tag.y, &tag.text, tag.ink);
        let clear = |w: &Word, placed: &[Word]| {
            let r = w.rect();
            r.0 >= 0
                && r.0 + r.2 <= 384
                && r.1 >= TAGS_FROM
                && r.1 + r.3 <= 224
                && !taken.iter().any(|&o| meet(r, o))
                && !placed.iter().any(|o| meet(r, o.rect()))
        };
        let found = rises
            .iter()
            .flat_map(|&dy| shifts.iter().map(move |&dx| (dx, dy)))
            .map(|(dx, dy)| Word { x: home.x + dx, y: home.y + dy, ..home.clone() })
            .find(|w| clear(w, &placed));
        placed.extend(found);
    }
    placed
}

/// The first picture row a tag may take: under the heads-up display's bar
/// and the pack's strip.
const TAGS_FROM: i32 = 31;

/// The Dimming: a wall of violet-black smoke with pale eyes.
fn dimming(im: &mut Img, s: &Session, v: &View) {
    let front = v.qx(s.dim);
    if front <= 0 {
        return;
    }
    let edge = front.min(v.w);
    for sx in 0..edge {
        // Deeper toward the left; a ragged, swaying edge.
        let near = front - sx;
        let sway = (i64::from(math::sin((sx * 3 + s.elapsed as i32 * 4) % 256)) * 6 / 4096) as i32;
        for row in 0..v.h {
            let wave = (i64::from(math::sin((row * 2 + s.elapsed as i32 * 3) % 256)) * 5 / 4096) as i32;
            let d = near + sway + wave;
            if d < 0 {
                continue;
            }
            // Where the smoke is thin, the road shows through.
            if d < 3 && bayer(sx, row) > 0.5 {
                continue;
            }
            let c = if d < 3 {
                'x'
            } else if d < 12 {
                if bayer(sx, row) < 0.5 { 'K' } else { 'x' }
            } else {
                'k'
            };
            im.put(sx, row, c);
        }
    }
    if front > 16 {
        for k in 0..4 {
            let y = 40 + k * 46 + ((s.elapsed as i32 / 6 + k * 5) % 7);
            let x = (front - 14 - (k % 2) * 10).min(v.w - 8);
            im.rect(x, y, 3, 3, 'W');
            im.rect(x + 6, y, 3, 3, 'W');
        }
    }
}

// ---- the heads-up display --------------------------------------------------------

fn clock(ticks: u32) -> String {
    let s = ticks / HZ;
    format!("{}:{:02}", s / 60, s % 60)
}

// The top bar's columns, left to right: the act and the sparks at the edge, the
// clock and the chain, the spur's gauge, the lantern pips, the Dimming's gap.
// Each stands at least eight pixels from the next, a chain of any length too.
const CLOCK_X: i32 = 100;
const GAUGE_X: i32 = 160;
const PIPS_X: i32 = 256;
const GLOAM_X: i32 = 324;

// The words that come and go stand in the sky, above the riders' number tags
// and clear of the road, on lines of their own: the title card (two lines),
// the fall, the bell. The terminal's text rows (`art/roundtable/lightgallop.rs`)
// stand on the same lines, eight pixels to a row.
const TITLE_Y: i32 = 36;
const FALL_Y: i32 = 59;
const BELL_Y: i32 = 70;

/// The heads-up display's words for the viewer `r` (the tally has its own).
fn hud_words(s: &Session, r: &Rider) -> Vec<Word> {
    let acts = ["DOWNHILL GATE", "WHEEL ARCH", "REED FEN", "BELL GATE"];
    let gap = (r.x - s.dim) / Q;
    let near = gap < 260;
    let mut out = vec![
        Word::new(4, 3, acts[Session::act_at(r.x / Q)], 'v', false),
        Word::new(4, 13, &format!("SPARKS {:3}", r.sparks), 'T', false),
        Word::new(CLOCK_X, 3, &clock(s.elapsed), 'H', false),
    ];
    if r.chain >= 2 {
        out.push(Word::new(CLOCK_X, 13, &format!("CHAIN {}", r.chain), 'z', false));
    }
    out.push(Word::new(GAUGE_X, 14, "SPUR", 'U', false));
    out.push(Word::new(GLOAM_X, 3, "GLOAM", if near { '7' } else { 'v' }, false));
    out.push(Word::new(
        GLOAM_X,
        13,
        &format!("{:4}", gap.max(0)),
        if near && (s.elapsed / 8) % 2 == 0 { '7' } else { 'H' },
        false,
    ));
    // The bottom: keys at the start, then only the way out.
    if s.elapsed < 360 {
        out.push(Word::centred(192, 213, "A D RUN   E LEAP   SPACE SPUR", 'T'));
    }
    // The title card as the ride begins.
    if s.elapsed < 75 {
        out.push(Word::centred(192, TITLE_Y, "MOONWELL ROAD", 'T'));
        out.push(Word::centred(192, TITLE_Y + 11, "THE DIMMING COMES", 'v'));
    }
    if r.down > 0 {
        out.push(Word::centred(192, FALL_Y, "THE LANTERN CARRIES YOU", 'T'));
    }
    if s.tally.is_none() && s.bell_t.is_some() {
        out.push(Word::centred(192, BELL_Y, "RING THE BELL", 'T'));
    }
    out
}

fn hud(im: &mut Img, s: &Session, id: u32, words: &[Word]) {
    let lvl = level();
    let Some(seat) = s.seats.get(&id) else { return };
    let r = &seat.rider;
    // The top bar.
    im.rect(0, 0, 384, 22, 'k');
    im.line(0, 22, 383, 22, 'g');
    // The spur's lantern gauge.
    im.frame(GAUGE_X, 5, 82, 8, 'u');
    im.rect(GAUGE_X + 1, 6, 80 * r.gauge / GAUGE_MAX, 6, if r.gauge < 150 { '7' } else if r.spur { '6' } else { '4' });
    // Lantern posts lit, a pip each.
    for n in 0..lvl.posts.len() {
        let lit = s.posts >> n & 1 == 1;
        im.rect(PIPS_X + n as i32 * 9, 4, 6, 8, if lit { '5' } else { 'S' });
        im.frame(PIPS_X + n as i32 * 9, 4, 6, 8, if lit { '4' } else { 'u' });
    }
    // The pack on the road: a strip with everyone's place and the Dimming.
    let scale = |x: i32| 6 + x * 372 / lvl.width;
    im.rect(6, 25, 372, 3, 'K');
    im.rect(6, 25, (scale(s.dim / Q) - 6).clamp(0, 372), 3, 'X');
    for (n, p) in lvl.posts.iter().enumerate() {
        let lit = s.posts >> n & 1 == 1;
        im.rect(scale(p.x), 24, 1, 5, if lit { '5' } else { 'u' });
    }
    im.rect(scale(lvl.bell.0), 23, 2, 7, '4');
    for (&seat_id, st) in &s.seats {
        let x = scale(st.rider.x / Q);
        let c = rider_ink(seat_id);
        im.rect(x - 1, 25, 3, 3, c);
        if seat_id == id {
            im.rect(x - 2, 24, 5, 5, c);
            im.rect(x - 1, 25, 3, 3, 'k');
            im.rect(x, 26, 1, 1, c);
        }
    }
    for w in words {
        say(im, w);
    }
    if let Some(y) = &s.tally {
        tally(im, s, y);
    }
}

/// The tally's panel, and the frame inside it that the words stay within.
const PANEL: (i32, i32, i32, i32) = (56, 36, 272, 168);

/// The rank's stamp at tally tick `t`: `(x, y, w, h, grow)`, big at first and
/// then settling. `None` before it lands.
fn rank_stamp(letter: &str, t: u32) -> Option<(i32, i32, i32, i32, i32)> {
    if t < 110 {
        return None;
    }
    let grow = if t < 118 { 6 } else { 4 };
    let (w, h) = (text_width(letter) * grow + 12, 7 * grow + 10);
    Some((294 - w / 2, 96 - (h - 38) / 2, w, h, grow))
}

/// The shower of sparks at tally tick `t`: where each piece is (a 3 pixel
/// square) and its colour, falling over the party while the numbers tick.
fn confetti(t: u32) -> Vec<(i32, i32, char)> {
    if t >= 420 {
        return Vec::new();
    }
    (0..28i32)
        .filter_map(|k| {
            let h = hash(k, 7, 11) as i32;
            let (x, fall) = (h.rem_euclid(384), 20 + h.rem_euclid(5));
            let y = (h.rem_euclid(224) + t as i32 * fall / 10).rem_euclid(224);
            // Below the heads-up display's bar and the pack's strip.
            (y > 30).then_some((x, y, if k % 3 == 0 { '5' } else { '6' }))
        })
        .collect()
}

/// Everyone's own score, in rows of at most four (three for a table of five or
/// six, which would not fit across), centred on the panel. The best rider
/// wears the laurel.
fn score_words(y: &rules::Tally) -> Vec<Word> {
    let entries: Vec<(u32, String)> = y
        .scores
        .iter()
        .map(|(&sid, v)| (sid, format!("{}{sid} {v}", if sid == y.best { "  " } else { "" })))
        .collect();
    let per_row = if entries.len() > 4 { 3 } else { entries.len().max(1) };
    let mut out = Vec::new();
    for (n, row) in entries.chunks(per_row).enumerate() {
        let across = row.iter().map(|(_, text)| text_width(text)).sum::<i32>() + 12 * (row.len() as i32 - 1);
        let mut x = 192 - across / 2;
        for (sid, text) in row {
            let mut word = Word::new(x, 158 + n as i32 * 11, text, rider_ink(*sid), false);
            word.laurel = *sid == y.best;
            out.push(word);
            x += text_width(text) + 12;
        }
    }
    out
}

/// Every word on the tally panel at this moment, the title first.
fn tally_words(s: &Session, y: &rules::Tally) -> Vec<Word> {
    let lvl = level();
    let t = s.elapsed.saturating_sub(y.at);
    let mut out = vec![Word::centred(192, 44, "MOONWELL ROAD CLEAR", 'T')];
    // Rows tick up one after another.
    let tick = |from: u32, to: u32, len: u32| -> u32 { to * t.saturating_sub(from).min(len) / len };
    let secs = y.time / HZ;
    let par = lvl.par / HZ;
    // The party's points carry a bonus for the lanterns lit (a hundred for all
    // six, `make_tally`), which each knight's own line below leaves out: it
    // gets a row of its own, between the lanterns and the total it is part of.
    let bonus = 100 * y.posts / lvl.posts.len().max(1) as u32;
    let rows: [(&str, String, bool); 6] = [
        ("TIME", format!("{}  PAR {}", clock(tick(6, y.time, 30)), clock(par * HZ)), secs <= par),
        ("SPARKS", format!("{} / {}", tick(24, y.sparks, 30), lvl.sparks.len()), true),
        ("BEST CHAIN", format!("{}", tick(42, y.chain, 20)), y.chain >= 4),
        ("LANTERNS", format!("{} / {}", tick(54, y.posts, 12), lvl.posts.len()), y.posts as usize == lvl.posts.len()),
        ("LANTERN BONUS", format!("+{}", tick(58, bonus, 10)), bonus > 0),
        ("POINTS", format!("{}", tick(66, y.points, 40)), true),
    ];
    for (n, (name, value, good)) in rows.iter().enumerate() {
        let ry = 58 + n as i32 * 13;
        out.push(Word::new(76, ry, name, 'v', false));
        out.push(Word::new(180, ry, value, if *good { 'T' } else { 'H' }, false));
    }
    if t >= 100 {
        out.push(Word::centred(294, 70, "RANK", 'v'));
    }
    out.push(Word::centred(192, 146, "EACH KNIGHT", 'v'));
    out.extend(score_words(y));
    if t >= 45 {
        out.push(Word::centred(192, 190, "E RIDE AGAIN   HOLD ESC LEAVE", if (s.elapsed / 15) % 2 == 0 { 'T' } else { 'H' }));
    }
    out
}

fn tally(im: &mut Img, s: &Session, y: &rules::Tally) {
    let t = s.elapsed.saturating_sub(y.at);
    im.rect(PANEL.0, PANEL.1, PANEL.2, PANEL.3, 'k');
    im.frame(PANEL.0, PANEL.1, PANEL.2, PANEL.3, 'T');
    im.frame(PANEL.0 + 2, PANEL.1 + 2, PANEL.2 - 4, PANEL.3 - 4, 'B');
    let letter = ["C", "B", "A", "S"][usize::from(y.rank.min(3))];
    let stamp = rank_stamp(letter, t);
    let words = tally_words(s, y);
    // The shower falls behind the words: a piece that would land on a word or
    // on the stamp, or touch one (it would read as a stop or a dot), is left
    // out, so every number stays plain to read.
    let keep_clear: Vec<_> =
        words.iter().map(Word::rect).chain(stamp.map(|(x, y, w, h, _)| (x, y, w, h))).collect();
    for (x, y, c) in confetti(t) {
        if !keep_clear.iter().any(|&r| meet(r, (x - 3, y - 3, 9, 9))) {
            im.rect(x, y, 3, 3, c);
        }
    }
    // The rank lands with a stamp, big at first, then settling.
    if let Some((bx, by, bw, bh, grow)) = stamp {
        let mut big = Img::new(text_width(letter), 7);
        big.text(0, 0, letter, if y.rank == 3 { '6' } else { 'T' });
        im.rect(bx, by, bw, bh, 'X');
        im.frame(bx, by, bw, bh, 'T');
        im.stamp_times(&big, bx + 6, by + 5, grow);
    }
    for w in &words {
        say(im, w);
    }
}

/// Whether two rectangles `(x, y, w, h)` share a pixel.
fn meet(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Every slot's blocks laid out in a grid, state by state and frame by frame,
/// each at `k` times its size, for the art wave and the reviewer.
#[cfg(test)]
pub fn block_sheet(k: i32) -> Img {
    let mut cells: Vec<(String, Img)> = Vec::new();
    for ((id, state), anim) in bank() {
        for (i, f) in anim.frames.iter().enumerate() {
            cells.push((format!("{} {state} {i}", id.trim_start_matches("lightgallop.")), f.clone()));
        }
    }
    let cols = 8;
    let (cw, ch) = (140, 120);
    let rows = (cells.len() as i32 + cols - 1) / cols;
    let mut sheet = Img::black(cols * cw, rows * ch);
    sheet.rect(0, 0, cols * cw, rows * ch, 'f');
    for (n, (name, f)) in cells.iter().enumerate() {
        let (cx, cy) = ((n as i32 % cols) * cw, (n as i32 / cols) * ch);
        let scale = k.min((cw - 8) / f.w.max(1)).min((ch - 24) / f.h.max(1)).max(1);
        let mut big = Img::new(f.w * scale, f.h * scale);
        big.stamp_times(f, 0, 0, scale);
        sheet.text(cx + 3, cy + 3, &name.chars().take(23).collect::<String>(), 'T');
        sheet.stamp(&big, cx + (cw - big.w) / 2, cy + 16);
    }
    sheet
}

/// A picture of a stretch of the road from column `from`, `zoom` of native
/// (256 is native), for the reviewer: the curves, the fork, the things that
/// stand on it.
#[cfg(test)]
pub fn overview(run: &Run, from: i32, w: i32, zoom: i32) -> Img {
    let lvl = level();
    let mut im = Img::black(w, (620 * zoom / 256).max(100));
    let s = run.lightgallop.as_ref().expect("a ride");
    let v = View { cx: from, cy: 450, zoom, w, h: im.h };
    let id = *s.seats.keys().next().unwrap();
    scene(&mut im, s, id, &v, &[]);
    for (i, p) in lvl.posts.iter().enumerate() {
        im.text(v.sx(p.x) - 6, v.sy(p.y) + 8, &format!("P{i}"), 'T');
    }
    for k in 0..lvl.acts.len() {
        let x = v.sx(lvl.acts[k]);
        im.line(x, 0, x, im.h - 1, 'T');
        im.text(x + 3, 4, ["GATE", "ARCH", "FEN", "BELL"][k], 'T');
    }
    im
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__lightgallop__tests.rs"]
mod tests;
