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
use super::juice;
use super::roundtable::{self as shell, Look};
use crate::rules::lightgallop::{
    self as rules, Session,
    level::level,
    math::{self, Q},
    rider::{GAUGE_MAX, Rider},
    session::WARDEN_FALL,
    terrain::Kind,
    show::Kind as Event,
    world::{CrateKind, FoeKind, SpringKind, foe_at, foe_body},
};
use crate::rules::{HZ, Run};
use std::{collections::BTreeMap, sync::OnceLock};

mod finish;
mod fx;
mod moments;

const LOOK: Look = Look {
    name: rules::NAME,
    backdrop: 'F',
    edge: 'y',
    accent: 'T',
};

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    draw(run, focus)
}

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
        let jolt = super::camera::smooth_shake(u64::from(s.elapsed), u32::from(r.shake), 12, 3.0);
        let kick = fx::jolt(s, id);
        let v = View { cx: r.cam_x + jolt.0 + kick.0, cy: r.cam_y + jolt.1 + kick.1, zoom: r.zoom, w: 384, h: 224 };
        let words = hud_words(s, id, &seat.rider);
        let taken: Vec<Rect> = words.iter().map(Word::rect).collect();
        scene(&mut im, s, id, &v, &taken);
        let lean = fx::tilt_of(s, id);
        if lean != 0 {
            im = fx::tilted(&im, lean);
        }
        moments::rims(&mut im, s, id);
        hud(&mut im, s, id, &words);
        match &s.tally {
            Some(y) => finish::scene(&mut im, run, s, y),
            None => {
                moments::fly_ups(&mut im, s, &v, id);
                if let Some(call) = moments::call(s, id) {
                    moments::banner(&mut im, &call);
                }
            }
        }
    }
    (im, Img::new(384, 224))
}

pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    shell::stub_kingdom(im, origin, rules::ENTRY, &LOOK);
}

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
    bare: Vec<Img>,
    names: Vec<Vec<Name>>,
    durations: Vec<u32>,
    painted: bool,
    cloth: Vec<Vec<(i32, i32, u8)>>,
}

fn cloth_of(id: &str, key: &str) -> Vec<(i32, i32, u8)> {
    let Some(bytes) = super::slot_art::bytes(id, &format!("{key}-seat-mask")) else {
        return Vec::new();
    };
    let mask = image::load_from_memory(bytes).expect("Lightgallop seat mask").to_rgba8();
    mask.enumerate_pixels().filter(|(_, _, p)| p[3] > 0).map(|(x, y, p)| (x as i32, y as i32, (p[0] / 70).min(3))).collect()
}

const CLOTH: [[char; 4]; 6] = [
    ['s', 'q', '1', 'z'],
    ['I', 'O', '4', 't'],
    ['D', 'l', 'A', 'Y'],
    ['8', 'p', '7', 'R'],
    ['d', '(', ')', '['],
    ['!', '?', ';', '%'],
];

fn dressed(frame: &Img, cloth: &[(i32, i32, u8)], seat: u32) -> Img {
    let ramp = CLOTH[(seat.max(1) as usize - 1) % 6];
    let mut out = frame.clone();
    for &(x, y, band) in cloth {
        out.put(x, y, ramp[usize::from(band)]);
    }
    out
}

struct Name {
    x: i32,
    y: i32,
    text: String,
    ink: char,
    plate: bool,
}

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
                            cloth: (0..frames.len()).map(|i| cloth_of(&s.id, &format!("{state}-{i}"))).collect(),
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
                out.insert((s.id.clone(), state.clone()), Anim { cloth: vec![Vec::new(); frames.len()], frames, bare, names, durations: durations.clone(), painted: false });
            }
        }
        out
    })
}

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
    let layer = s.id.contains("layer");
    im.rect(1, 1, (w - 2).max(0), (h - 2).max(0), if layer { 'f' } else { 'X' });
    im.frame(0, 0, w, h, s.ink);
    im.line(2, 2, w - 3, h - 3, s.ink);
    if w >= 24 && h >= 14 {
        let lx = ((w - text_width(&s.label)) / 2).max(1);
        names.push(Name { x: lx, y: h / 2 - 3, text: s.label.clone(), ink: s.ink, plate: true });
    }
    if matches!(s.id.as_str(), "lightgallop.snuffer" | "lightgallop.floater" | "lightgallop.warden") {
        let ey = h / 4;
        im.rect(w / 4, ey, 3, 3, 'w');
        im.rect(w / 4 + 7, ey, 3, 3, 'w');
    }
    (finish(im, &names, bake), names)
}

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

fn rider_block(state: &str, frame: usize, colour: char, label: &str) -> Img {
    let mut im = Img::new(48, 40);
    if matches!(state, "roll" | "charge") {
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
    im.frame(0, 0, 48, 40, if state == "hurt" { '7' } else { 'j' });
    if !label.is_empty() {
        let tw = text_width(label);
        im.rect(2, 2, tw + 2, 9, 'k');
        im.text(3, 3, label, colour);
    }
    im
}

fn painted(id: &str) -> bool {
    bank().range((id.to_owned(), String::new())..).next().is_some_and(|((slot, _), anim)| slot == id && anim.painted)
}

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

fn frame_of(id: &str, state: &str, tick: u32) -> &'static Img {
    let (anim, index) = anim_at(id, state, tick);
    &anim.frames[index]
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Word {
    x: i32,
    y: i32,
    text: String,
    ink: char,
    backed: bool,
    laurel: bool,
}

impl Word {
    fn new(x: i32, y: i32, text: &str, ink: char, backed: bool) -> Word {
        Word { x, y, text: text.to_owned(), ink, backed, laurel: false }
    }

    fn centred(cx: i32, y: i32, text: &str, ink: char) -> Word {
        Word::new(cx - text_width(text) / 2, y, text, ink, true)
    }

    fn rect(&self) -> (i32, i32, i32, i32) {
        let w = text_width(&self.text);
        if self.backed { (self.x - 1, self.y - 1, w + 2, 9) } else { (self.x, self.y, w, 7) }
    }
}

#[cfg(test)]
thread_local! {
    static SAID: std::cell::RefCell<Vec<Word>> = const { std::cell::RefCell::new(Vec::new()) };
    static ON_BLOCKS: std::cell::RefCell<Vec<Word>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn say(im: &mut Img, w: &Word) {
    #[cfg(test)]
    SAID.with(|said| said.borrow_mut().push(w.clone()));
    print(im, w);
}

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
        for (dx, dy) in [(0, 0), (2, 0), (4, 0)].into_iter().chain((0..5).flat_map(|dx| [(dx, 1), (dx, 2)])) {
            im.put(w.x + 3 + dx, w.y + 2 + dy, w.ink);
        }
    }
}

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

#[cfg(test)]
fn said(run: &Run, focus: Option<u32>) -> Vec<Word> {
    said_with_names(run, focus).0
}

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
        let (tw, cx, cy) = (text_width(&n.text), n.x + text_width(&n.text) / 2, n.y + 3);
        let at = (ox + cx * scale / 256 - tw / 2, oy + cy * scale / 256 - 3);
        say_on_block(im, &Word::new(at.0, at.1, &n.text, n.ink, n.plate));
    }
}

const POST_LANTERN: (i32, i32) = (6, 34);

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

pub fn scene(im: &mut Img, s: &Session, id: u32, v: &View, taken: &[Rect]) {
    let lvl = level();
    let me = &s.seats[&id].rider;
    backdrop(im, v);
    terrain(im, v);
    for sign in &lvl.signs {
        let (sx, sy) = (v.sx(sign.x), v.sy(sign.y));
        if !(-80..v.w + 80).contains(&sx) {
            continue;
        }
        let tw = text_width(&sign.words);
        im.rect(sx - 1, sy - 20, 2, 20, 'b');
        im.rect(sx - tw / 2 - 3, sy - 34, tw + 6, 14, 'P');
        im.frame(sx - tw / 2 - 3, sy - 34, tw + 6, 14, 'B');
        im.text(sx - tw / 2, sy - 31, &sign.words, 'T');
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
            let r = 14 - (spin as i32 / 3).min(10);
            let (lx, ly) = if painted("lightgallop.post") { (v.len(POST_LANTERN.0), -v.len(POST_LANTERN.1)) } else { (0, -46) };
            circle(im, sx + lx, v.sy(p.y) + ly, r, '6');
        }
    }
    let mut over_bell = None;
    {
        let (bx, by) = lvl.bell;
        let sx = v.sx(bx);
        if (-60..v.w + 60).contains(&sx) {
            let ringing = s.bell_t.is_some();
            let f = frame_of("lightgallop.bell", if ringing { "ring" } else { "rest" }, s.elapsed);
            stamp_block(im, "lightgallop.bell", if ringing { "ring" } else { "rest" }, s.elapsed, sx, v.sy(by), v.zoom);
            if !ringing {
                let word = Word::centred(sx, v.sy(by) - v.len(f.h) - 12, "BELL GATE", 'T');
                say(im, &word);
                over_bell = Some(word.rect());
            }
        }
    }
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
    let mut order: Vec<_> = s.seats.iter().collect();
    order.sort_by_key(|(seat, _)| **seat == id);
    let mut tags = Vec::new();
    for (&seat, st) in order {
        tags.extend(rider(im, s, seat, &st.rider, v, id));
    }
    fx::events(im, s, v, id);
    fx::pack(im, s, v, id);
    let mut taken = taken.to_vec();
    taken.extend(over_bell);
    if s.tally.is_none() {
        for tag in place_tags(tags, &taken) {
            say(im, &tag);
        }
    }
    dimming(im, s, v);
}

struct Strip {
    img: Img,
    above: Option<Img>,
    below: Img,
}

fn row_mode(img: &Img, y: i32) -> Option<Rgb> {
    let mut counts: BTreeMap<Rgb, u32> = BTreeMap::new();
    for x in 0..img.w {
        if let Some(c) = img.get(x, y) {
            *counts.entry(c).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|&(c, n)| (n, std::cmp::Reverse(c))).map(|(c, _)| c)
}

fn solid(c: Rgb) -> Img {
    Img::from_rgb(192, 128, &vec![c; 192 * 128])
}

const NEAR_DIM: f32 = 0.6;

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

const STRIP_BASE: [i32; 3] = [142, 152, 154];
const STRIP_SHARE: [i32; 3] = [16, 7, 3];

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

fn backdrop(im: &mut Img, v: &View) {
    if let Some(strips) = strips() {
        painted_backdrop(im, v, strips);
        return;
    }
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
    for y in 150.min(v.h)..v.h.min(190) {
        for x in 0..v.w {
            if bayer(x, y) * 40.0 < (y - 150) as f32 {
                im.put(x, y, 'B');
            }
        }
    }
    let (mx, my) = (292 - (v.cx / 40) % 400, 44);
    im.ellipse(mx as f32, my as f32, 28.0, 28.0, 'V');
    im.ellipse((mx - 4) as f32, (my - 3) as f32, 22.0, 22.0, 'W');
    for (dx, dy, r) in [(-9, -6, 4.0), (6, 8, 5.0), (12, -10, 3.0)] {
        im.ellipse((mx + dx) as f32, (my + dy) as f32, r, r, 'v');
    }
    for (share, base, colour, amp, period) in [(16, 168, 'x', 34, 330), (7, 190, 'f', 26, 170), (3, 214, 'D', 18, 90)] {
        let scroll = v.cx / share;
        for sx in 0..v.w {
            let wx = scroll + sx;
            let t = (wx * 256).div_euclid(period);
            let h = amp / 2
                + (i64::from(math::sin(t)) * i64::from(amp) / 2 / 4096) as i32
                + (i64::from(math::sin(t * 3 + 40)) * i64::from(amp) / 6 / 4096) as i32;
            let off = (v.cy - 520) / (share / 2).max(1);
            let top = base - h - off.clamp(-30, 30);
            im.rect(sx, top, 1, (v.h - top).max(0), colour);
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

struct Road {
    tile: Img,
    deep: Rgb,
}

const ROAD_LIP: i32 = 9;

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

fn terrain(im: &mut Img, v: &View) {
    let lvl = level();
    let t = &lvl.terrain;
    let road = road();
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
    for p in t.paths.iter().filter(|p| p.kind == Kind::Ring) {
        let (lo, hi) = p.span_px();
        if v.sx(hi + 12) < 0 || v.sx(lo - 12) > v.w {
            continue;
        }
        let thick = v.len(7).max(3);
        for i in 0..p.pts.len() - 1 {
            let (a, b) = (p.pts[i], p.pts[i + 1]);
            let d = p.dir[i];
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
            if spot.kind == FoeKind::Warden && st.pop > 0 && st.pop < WARDEN_FALL {
                let age = i32::from(st.pop);
                if age < 16 {
                    stamp_scaled(im, frame_of("lightgallop.warden", "defeat", age as u32), sx, sy, v.zoom);
                }
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
            continue;
        }
        let alert = s.seats.values().any(|st| {
            let rx = st.rider.x / Q;
            st.rider.down == 0 && rx < spot.x + 40 && rx > spot.x - if spot.kind == FoeKind::Warden { 240 } else { 150 }
        });
        let (id, state) = match spot.kind {
            FoeKind::Snuffer => ("lightgallop.snuffer", if st.flinch > 0 { "hurt" } else if alert { "alert" } else { "idle" }),
            FoeKind::Floater => ("lightgallop.floater", if alert { "alert" } else { "idle" }),
            FoeKind::Bramble => ("lightgallop.bramble", "a"),
            FoeKind::Warden => ("lightgallop.warden", if st.flinch > 0 { "hit" } else if alert { "roar" } else { "idle" }),
        };
        let (_, _, facing) = foe_at(lvl, spot, s.elapsed);
        let f = frame_of(id, state, s.elapsed / 2);
        let drop = if spot.kind == FoeKind::Floater { (f.h - fh) / 2 } else { 0 };
        if painted(id) && facing > 0 {
            stamp_scaled(im, &f.flip_h(), sx, sy + drop, v.zoom);
        } else {
            stamp_block(im, id, state, s.elapsed / 2, sx, sy + drop, v.zoom);
        }
        if spot.kind == FoeKind::Warden {
            let top = sy - v.len(fh) - 10;
            let hits = i32::from(st.hp);
            for k in 0..hits {
                im.rect(sx - hits * 4 + k * 8 + 1, top, 6, 4, '7');
            }
        }
    }
}

const FLASH: u32 = 70;

fn rider(im: &mut Img, s: &Session, seat: u32, r: &Rider, v: &View, me: u32) -> Option<Tag> {
    let other = seat != me;
    let t = &level().terrain;
    if v.qx(r.x) < -60 || v.qx(r.x) > v.w + 60 {
        return None;
    }
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
    let c = r.centre(t);
    let (fan_x, fan_y) = fan(s, me, seat);
    let (sx, sy) = (v.qx(c.0) + fan_x, v.qy(c.1) + fan_y);
    if !other {
        fx::lines(im, s, v, seat, r, false, (sx, sy));
    }
    fx::dust(im, s, v, seat, r);
    fx::loop_light(im, s, v, seat, r);
    {
        let (fx_, fy_) = (v.qx(r.x), v.qy(r.y));
        if r.draft && r.down == 0 {
            for k in 0..3 {
                let h = hash(k, s.elapsed as i32 / 2, seat + 5) as i32;
                let x0 = fx_ + r.dir * (16 + h.rem_euclid(14));
                im.line(x0, fy_ - 22 + k * 6, x0 + r.dir * (10 + h.rem_euclid(10)), fy_ - 22 + k * 6, 'V');
            }
        }
        if r.spur && r.down == 0 {
            for k in 0..4 {
                im.rect(fx_ - r.dir * (14 + k * 4), fy_ - 14 + k % 2, 3 - k / 2, 3, if k < 2 { '6' } else { '4' });
            }
        }
        if r.crouch && r.charge > 0 {
            let rr = 6 + r.charge / 160;
            im.frame(fx_ - rr, fy_ - rr, 2 * rr, rr, '4');
        }
    }
    let gait = (r.x / Q / 9) as u32;
    let colour = rider_ink(seat);
    let label = format!("{seat}");
    let img = if painted("lightgallop.rider") {
        let anim = &bank()[&("lightgallop.rider".to_owned(), state.to_owned())];
        let n = anim.frames.len();
        let i = match state {
            "gallop" | "spur" => gait as usize % n,
            "roll" | "charge" => (s.elapsed / 2) as usize % n,
            "idle" => (s.elapsed / 10) as usize % n,
            "lunge" => usize::from(r.vy > 0 && n > 1),
            "victory" | "hurt" => (s.elapsed / 8) as usize % n,
            _ => 0,
        };
        dressed(&anim.frames[i], &anim.cloth[i], seat)
    } else {
        let n = bank()[&("lightgallop.rider".to_owned(), state.to_owned())].frames.len() as u32;
        let frame = match state {
            "gallop" | "spur" | "roll" | "idle" | "charge" => (gait % n) as usize,
            _ => 0,
        };
        rider_block(state, frame, colour, &label)
    };
    let struck = s.show.fx.iter().any(|f| {
        u32::from(f.seat) == seat && matches!(f.kind, Event::Hit | Event::Fall) && f.age(s.elapsed) < f.weight.flash()
    });
    let img = if struck { juice::impact_frame(&img, 'H') } else { img };
    let img = if r.hurt > 0 && r.down == 0 && (s.elapsed / 3) % 2 == 0 {
        let mut thin = img.clone();
        for y in 0..thin.h {
            for x in 0..thin.w {
                if (x + y) % 2 == 0 {
                    thin.clear(x, y);
                }
            }
        }
        thin
    } else {
        img
    };
    let ang = if state == "roll" || state == "charge" {
        (r.x / Q * 9) * r.dir
    } else if r.down > 0 {
        (s.elapsed as i32 * 40) * r.dir
    } else {
        r.ang
    };
    fx::ghosts(im, v, seat, r, &img, ang, r.dir < 0);
    stamp_turned(im, &img, (sx, sy), (24, 24), ang, v.zoom, r.dir < 0);
    fx::lock_on(im, s, v, r, (sx, sy));
    if !other {
        fx::lines(im, s, v, seat, r, true, (sx, sy));
    }
    let on_screen = sx > -24 && sx < v.w + 24;
    (other && on_screen).then(|| Tag { cx: sx, y: sy - 34, text: label, ink: colour })
}

fn fan(s: &Session, me: u32, seat: u32) -> (i32, i32) {
    if seat == me {
        return (0, 0);
    }
    let t = &level().terrain;
    let key = |id: u32| (id != me, id);
    let at = |id: u32| {
        let c = s.seats[&id].rider.centre(t);
        (c.0 / Q, c.1 / Q)
    };
    let (px, py) = at(seat);
    let behind = s
        .seats
        .keys()
        .filter(|&&o| key(o) < key(seat))
        .filter(|&&o| {
            let (ox, oy) = at(o);
            (ox - px).abs() < 16 && (oy - py).abs() < 20
        })
        .count() as i32;
    (behind * 5, -behind * 3)
}

type Rect = (i32, i32, i32, i32);

struct Tag {
    cx: i32,
    y: i32,
    text: String,
    ink: char,
}

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

const TAGS_FROM: i32 = 31;

fn dimming(im: &mut Img, s: &Session, v: &View) {
    let front = v.qx(s.dim);
    if front <= 0 {
        return;
    }
    let edge = front.min(v.w);
    for sx in 0..edge {
        let near = front - sx;
        let sway = (i64::from(math::sin((sx * 3 + s.elapsed as i32 * 4) % 256)) * 6 / 4096) as i32;
        for row in 0..v.h {
            let wave = (i64::from(math::sin((row * 2 + s.elapsed as i32 * 3) % 256)) * 5 / 4096) as i32;
            let d = near + sway + wave;
            if d < 0 {
                continue;
            }
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

fn clock(ticks: u32) -> String {
    let s = ticks / HZ;
    format!("{}:{:02}", s / 60, s % 60)
}

const CLOCK_X: i32 = 100;
const GAUGE_X: i32 = 160;
const PIPS_X: i32 = 256;
const GLOAM_X: i32 = 324;

const BELL_Y: i32 = 70;

fn hud_words(s: &Session, id: u32, r: &Rider) -> Vec<Word> {
    let acts = ["DOWNHILL GATE", "WHEEL ARCH", "REED FEN", "BELL GATE"];
    let gap = (r.x - s.dim) / Q;
    let near = gap < 260;
    let mut out = vec![
        Word::new(4, 3, acts[Session::act_at(r.x / Q)], 'v', false),
        Word::new(4, 13, &format!("SPARKS {:3}", r.sparks), if moments::last_gain(s, id).is_some_and(|a| a < 8) { '6' } else { 'T' }, false),
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
    if s.elapsed < 360 {
        out.push(Word::centred(192, 213, "A D RUN   E LEAP   SPACE SPUR", 'T'));
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
    im.rect(0, 0, 384, 22, 'k');
    im.line(0, 22, 383, 22, 'g');
    im.frame(GAUGE_X, 5, 82, 8, 'u');
    im.rect(GAUGE_X + 1, 6, 80 * r.gauge / GAUGE_MAX, 6, if r.gauge < 150 { '7' } else if r.spur { '6' } else { '4' });
    for n in 0..lvl.posts.len() {
        let lit = s.posts >> n & 1 == 1;
        im.rect(PIPS_X + n as i32 * 9, 4, 6, 8, if lit { '5' } else { 'S' });
        im.frame(PIPS_X + n as i32 * 9, 4, 6, 8, if lit { '4' } else { 'u' });
    }
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
}

fn meet(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

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

#[cfg(test)]
#[path = "../../../../tests/render/arena__lightgallop_film__tests.rs"]
mod film;
