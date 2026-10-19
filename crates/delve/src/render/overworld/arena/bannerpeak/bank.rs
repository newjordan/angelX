//! The painted slots: `assets/bannerpeak/slots.json` names every actor and
//! prop, `slot_art` hands over a reviewed frame by `<state>-<index>`. A slot is
//! painted whole or not at all: if any frame of any of its states is missing,
//! every state of it is the labelled code art in the stage, so a prop never
//! changes look when it changes state. Nothing here sets a collision shape and
//! no duration is invented: the rules' and the sheet's ticks are the slot's.
use super::super::super::ink::Img;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(serde::Deserialize)]
struct Slot {
    id: String,
    size: [i32; 2],
    states: BTreeMap<String, Vec<u32>>,
}

/// A painted frame, as the stage draws it (opaque pixels only), and where its
/// anchor sits in it.
pub(super) struct Pic {
    pub img: Img,
    pub ax: i32,
    pub ay: i32,
}

struct Anim {
    frames: Vec<Pic>,
    durations: Vec<u32>,
}

/// Where a slot's anchor is in its own PNG. A painted frame may be a wider
/// canvas than the slot's box (the pole's yard, a pennon that streams past its
/// post), so the anchor is not always the box's: a pole is anchored at the
/// post, a star and a wheel at their centre, the rest at the bottom centre.
fn anchor(id: &str, w: i32, h: i32) -> (i32, i32) {
    match id {
        "bannerpeak.spark" | "bannerpeak.wheel" => (w / 2, h / 2),
        "bannerpeak.gate" => (8, h),
        "bannerpeak.flagpole" => (14, h),
        _ => (w / 2, h),
    }
}

fn painted(id: &str, bytes: &[u8], size: [i32; 2]) -> Pic {
    let png = image::load_from_memory(bytes).expect("Bannerpeak slot").to_rgba8();
    let (w, h) = (png.width() as i32, png.height() as i32);
    assert!(h == size[1] && w >= size[0], "{id}: slot registration {w}x{h} for {size:?}");
    let mut im = Img::new(w, h);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] >= 128 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    let (ax, ay) = anchor(id, w, h);
    Pic { img: im, ax, ay }
}

/// Every frame of one slot, or nothing.
fn whole(s: &Slot) -> Option<BTreeMap<String, Anim>> {
    let mut states = BTreeMap::new();
    for (state, durations) in &s.states {
        let frames: Option<Vec<Pic>> = (0..durations.len())
            .map(|i| {
                super::super::slot_art::bytes(&s.id, &format!("{state}-{i}"))
                    .map(|bytes| painted(&s.id, bytes, s.size))
            })
            .collect();
        states.insert(state.clone(), Anim { frames: frames?, durations: durations.clone() });
    }
    Some(states)
}

fn bank() -> &'static BTreeMap<String, BTreeMap<String, Anim>> {
    static BANK: OnceLock<BTreeMap<String, BTreeMap<String, Anim>>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> =
            serde_json::from_str(include_str!("../../../../../assets/bannerpeak/slots.json"))
                .expect("Bannerpeak slots");
        slots
            .into_iter()
            .filter_map(|s| {
                let id = s.id.strip_prefix("bannerpeak.")?.to_owned();
                whole(&s).map(|states| (id, states))
            })
            .collect()
    })
}

/// Frame `index` of `state`, when the slot is painted.
pub(super) fn pic(id: &str, state: &str, index: usize) -> Option<&'static Pic> {
    bank().get(id)?.get(state)?.frames.get(index)
}

/// The index of the frame of `state` at animation tick `tick`, by the slot's
/// own durations (the animation repeats), when the slot is painted.
pub(super) fn index_at(id: &str, state: &str, tick: u32) -> Option<usize> {
    let anim = bank().get(id)?.get(state)?;
    let mut time = tick % anim.durations.iter().sum::<u32>().max(1);
    let mut index = 0;
    while index + 1 < anim.durations.len() && time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    Some(index)
}

/// The frame of `state` at animation tick `tick`, when the slot is painted.
pub(super) fn at(id: &str, state: &str, tick: u32) -> Option<&'static Pic> {
    pic(id, state, index_at(id, state, tick)?)
}

/// How many frames `state` has.
pub(super) fn frames(id: &str, state: &str) -> usize {
    bank().get(id).and_then(|s| s.get(state)).map_or(0, |a| a.frames.len())
}

/// `pic` stamped with its anchor on `(x, y)`.
pub(super) fn stamp(im: &mut Img, pic: &Pic, x: i32, y: i32) {
    im.stamp(&pic.img, x - pic.ax, y - pic.ay);
}
