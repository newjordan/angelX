//! The slot bank: every actor and prop is a registered slot
//! (`assets/lanternward/slots.json`), a labelled block until a painted frame
//! arrives (`slot_art`). A block's shape and colour are for the eye only;
//! nothing here sets a collision shape.
use super::super::super::ink::{Img, text_width};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(serde::Deserialize)]
struct Slot {
    id: String,
    label: String,
    size: [i32; 2],
    shape: String,
    fill: String,
    ink: String,
    states: BTreeMap<String, Vec<u32>>,
}

pub(super) struct Anim {
    pub frames: Vec<Img>,
    pub durations: Vec<u32>,
    pub painted: bool,
}

fn block(s: &Slot) -> Img {
    let (w, h) = (s.size[0], s.size[1]);
    let fill = s.fill.chars().next().unwrap_or('X');
    let ink = s.ink.chars().next().unwrap_or('H');
    let mut im = Img::new(w, h);
    match s.shape.as_str() {
        "round" => {
            im.ellipse(w as f32 / 2.0, h as f32 / 2.0, w as f32 / 2.0, h as f32 / 2.0, ink);
            im.ellipse(
                w as f32 / 2.0,
                h as f32 / 2.0,
                (w as f32 / 2.0 - 1.0).max(1.0),
                (h as f32 / 2.0 - 1.0).max(1.0),
                fill,
            );
        }
        "tuft" => {
            for (i, bx) in [2, 5, 8, 11, 13].into_iter().enumerate() {
                let top = 2 + (i as i32 * 3) % 5;
                im.rect(bx.min(w - 2), top, 1, h - top, fill);
                im.put(bx.min(w - 2), top, ink);
            }
            im.rect(1, h - 3, w - 2, 3, fill);
        }
        _ => {
            im.rect(0, 0, w, h, ink);
            im.rect(1, 1, w - 2, h - 2, fill);
            if s.shape == "tall" {
                im.line(1, 2, w - 2, 2, ink);
            }
        }
    }
    let mut label = s.label.as_str();
    while text_width(label) > w - 2 && label.len() > 1 {
        label = &label[..label.len() - 1];
    }
    if text_width(label) <= w - 2 && h >= 9 && s.shape != "tuft" {
        let lx = (w - text_width(label)) / 2;
        im.text(lx, (h - 7) / 2, label, ink);
    }
    im
}

pub(super) fn whole_or_blocks(states: &mut [(String, Vec<(Img, bool)>)], block: &Img) {
    if states.iter().all(|(_, made)| made.iter().all(|(_, painted)| *painted)) {
        return;
    }
    for (_, made) in states {
        for frame in made.iter_mut() {
            *frame = (block.clone(), false);
        }
    }
}

fn inked(s: &Slot, state: &str, i: usize) -> Option<Img> {
    let slot = s.id.strip_prefix("lanternward.")?;
    let set = super::art_data::SETS.iter().find(|set| set.slot == slot && set.state == state)?;
    let frame = set.frames.get(i)?;
    let mut im = Img::new(s.size[0], s.size[1]);
    for (y, row) in frame.rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            im.put(i32::from(frame.x) + x as i32, i32::from(frame.y) + y as i32, ch);
        }
    }
    Some(im)
}

fn bank() -> &'static BTreeMap<(String, String), Anim> {
    static BANK: OnceLock<BTreeMap<(String, String), Anim>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> =
            serde_json::from_str(include_str!("../../../../../assets/lanternward/slots.json"))
                .expect("Lanternward slots");
        let mut out = BTreeMap::new();
        for s in slots {
            let mut states: Vec<(String, Vec<(Img, bool)>)> = s
                .states
                .iter()
                .map(|(state, durations)| {
                    let made = durations
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            let key = format!("{state}-{i}");
                            if let Some(bytes) = super::super::slot_art::bytes(&s.id, &key) {
                                let png = image::load_from_memory(bytes)
                                    .expect("Lanternward slot")
                                    .to_rgba8();
                                assert_eq!(
                                    [png.width() as i32, png.height() as i32],
                                    s.size,
                                    "slot registration"
                                );
                                let mut im = Img::new(s.size[0], s.size[1]);
                                for (x, y, p) in png.enumerate_pixels() {
                                    if p[3] >= 128 {
                                        im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
                                    }
                                }
                                return (im, true);
                            }
                            if let Some(im) = inked(&s, state, i) {
                                return (im, true);
                            }
                            (block(&s), false)
                        })
                        .collect();
                    (state.clone(), made)
                })
                .collect();
            whole_or_blocks(&mut states, &block(&s));
            for (state, made) in states {
                let painted = made.iter().all(|(_, painted)| *painted);
                out.insert(
                    (s.id.clone(), state.clone()),
                    Anim {
                        frames: made.into_iter().map(|(im, _)| im).collect(),
                        durations: s.states[&state].clone(),
                        painted,
                    },
                );
            }
        }
        out
    })
}

pub(super) fn painted(id: &str, state: &str) -> bool {
    bank()
        .get(&(format!("lanternward.{id}"), state.to_owned()))
        .is_some_and(|anim| anim.painted)
}

pub(super) fn frame(id: &str, state: &str, tick: u32) -> &'static Img {
    let anim = bank()
        .get(&(format!("lanternward.{id}"), state.to_owned()))
        .unwrap_or_else(|| panic!("slot lanternward.{id} has no state {state}"));
    let mut time = tick % anim.durations.iter().sum::<u32>().max(1);
    let mut index = 0;
    while index + 1 < anim.durations.len() && time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    &anim.frames[index]
}

pub(super) fn frame_at(id: &str, state: &str, index: usize) -> &'static Img {
    let anim = bank()
        .get(&(format!("lanternward.{id}"), state.to_owned()))
        .unwrap_or_else(|| panic!("slot lanternward.{id} has no state {state}"));
    &anim.frames[index.min(anim.frames.len() - 1)]
}

pub(super) fn slot(im: &mut Img, id: &str, state: &str, tick: u32, x: i32, foot: i32) {
    let pic = frame(id, state, tick);
    im.stamp(pic, x - pic.w / 2, foot - pic.h);
}

pub(super) fn slot_mid(im: &mut Img, id: &str, state: &str, tick: u32, x: i32, y: i32) {
    let pic = frame(id, state, tick);
    im.stamp(pic, x - pic.w / 2, y - pic.h / 2);
}
