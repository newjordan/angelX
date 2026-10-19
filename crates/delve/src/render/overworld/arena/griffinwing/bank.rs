//! The painted slots: `assets/griffinwing/slots.json` names every slot,
//! `slot_art` hands over a reviewed frame by `<state>-<index>`. A slot is
//! painted whole or not at all: if any frame of any of its states is missing,
//! every state of it is the stage's own code art, so a set piece never changes
//! look when it changes state. Nothing here sets a collision shape and no
//! duration is invented: the rules' and the sheet's ticks are the slot's.
use super::super::super::ink::{Img, nearest_plain};
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
    levels: OnceLock<Vec<Img>>,
}

struct Anim {
    frames: Vec<Pic>,
    durations: Vec<u32>,
}

/// How many halvings a picture keeps for a far set piece (the picture, then
/// a half, a quarter and an eighth).
const LEVELS: usize = 4;

impl Pic {
    /// The picture halved `level` times (0 to 3), each dot the box average of
    /// the dots under it kept when half of them are opaque, snapped to the
    /// palette's plain inks, so a far set piece is drawn from a picture near
    /// its own size and not from every fifth dot of a big one.
    pub fn level(&self, level: usize) -> &Img {
        let levels = self.levels.get_or_init(|| {
            let mut out = vec![self.img.clone()];
            for l in 1..LEVELS {
                let above = &out[l - 1];
                let (w, h) = ((above.w / 2).max(1), (above.h / 2).max(1));
                let mut im = Img::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let four: Vec<[u8; 3]> = [(0, 0), (1, 0), (0, 1), (1, 1)]
                            .iter()
                            .filter_map(|(dx, dy)| above.get(x * 2 + dx, y * 2 + dy))
                            .collect();
                        if four.len() * 2 >= 4 {
                            let mean = |c: usize| {
                                four.iter().map(|p| f32::from(p[c])).sum::<f32>() / four.len() as f32
                            };
                            im.set(x, y, nearest_plain([mean(0), mean(1), mean(2)]));
                        }
                    }
                }
                out.push(im);
            }
            out
        });
        &levels[level.min(LEVELS - 1)]
    }
}

/// Where a slot's anchor is in its own PNG: the rings at the centre of their
/// clear hole, everything else at the bottom centre.
fn anchor(id: &str, w: i32, h: i32) -> (i32, i32) {
    match id {
        "griffinwing.gate" | "griffinwing.hearth-ring" | "griffinwing.canyon-tex" => (w / 2, h / 2),
        _ => (w / 2, h),
    }
}

fn painted(id: &str, bytes: &[u8], size: [i32; 2]) -> Pic {
    let png = image::load_from_memory(bytes)
        .expect("Griffinwing slot")
        .to_rgba8();
    let (w, h) = (png.width() as i32, png.height() as i32);
    assert!(
        (w, h) == (size[0], size[1]),
        "{id}: slot registration {w}x{h} for {size:?}"
    );
    let mut im = Img::new(w, h);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] >= 128 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    let (ax, ay) = anchor(id, w, h);
    Pic { img: im, ax, ay, levels: OnceLock::new() }
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
        states.insert(
            state.clone(),
            Anim {
                frames: frames?,
                durations: durations.clone(),
            },
        );
    }
    Some(states)
}

fn bank() -> &'static BTreeMap<String, BTreeMap<String, Anim>> {
    static BANK: OnceLock<BTreeMap<String, BTreeMap<String, Anim>>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> =
            serde_json::from_str(include_str!("../../../../../assets/griffinwing/slots.json"))
                .expect("Griffinwing slots");
        slots
            .into_iter()
            .filter_map(|s| {
                let id = s.id.strip_prefix("griffinwing.")?.to_owned();
                whole(&s).map(|states| (id, states))
            })
            .collect()
    })
}

/// Frame `index` of `state`, when the slot is painted.
pub(super) fn pic(id: &str, state: &str, index: usize) -> Option<&'static Pic> {
    bank().get(id)?.get(state)?.frames.get(index)
}

/// The frame of `state` at animation tick `tick`, by the slot's own durations
/// (the animation repeats), when the slot is painted.
pub(super) fn at(id: &str, state: &str, tick: u32) -> Option<&'static Pic> {
    let anim = bank().get(id)?.get(state)?;
    let mut time = tick % anim.durations.iter().sum::<u32>().max(1);
    let mut index = 0;
    while index + 1 < anim.durations.len() && time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    anim.frames.get(index)
}
