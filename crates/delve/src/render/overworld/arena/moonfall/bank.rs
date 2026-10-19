//! The painted slots: `assets/moonfall/slots.json` names every actor and
//! prop, `slot_art` hands over a reviewed frame by `<state>-<index>`. A state
//! with every frame painted is stamped; any other state keeps the labelled
//! code art in `ground`. Nothing here sets a collision shape.
use super::super::super::ink::Img;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(serde::Deserialize)]
struct Slot {
    id: String,
    size: [i32; 2],
    states: BTreeMap<String, Vec<u32>>,
}

struct Anim {
    frames: Vec<Img>,
    durations: Vec<u32>,
}

/// The painted frame, as the stage draws it: opaque pixels only.
fn painted(bytes: &[u8], size: [i32; 2]) -> Img {
    let png = image::load_from_memory(bytes).expect("Moonfall slot").to_rgba8();
    assert_eq!([png.width() as i32, png.height() as i32], size, "slot registration");
    let mut im = Img::new(size[0], size[1]);
    for (x, y, p) in png.enumerate_pixels() {
        if p[3] >= 128 {
            im.set(x as i32, y as i32, [p[0], p[1], p[2]]);
        }
    }
    im
}

fn bank() -> &'static BTreeMap<(String, String), Anim> {
    static BANK: OnceLock<BTreeMap<(String, String), Anim>> = OnceLock::new();
    BANK.get_or_init(|| {
        let slots: Vec<Slot> =
            serde_json::from_str(include_str!("../../../../../assets/moonfall/slots.json"))
                .expect("Moonfall slots");
        let mut out = BTreeMap::new();
        for s in slots {
            for (state, durations) in &s.states {
                let frames: Option<Vec<Img>> = (0..durations.len())
                    .map(|i| {
                        super::super::slot_art::bytes(&s.id, &format!("{state}-{i}"))
                            .map(|bytes| painted(bytes, s.size))
                    })
                    .collect();
                if let Some(frames) = frames {
                    out.insert((s.id.clone(), state.clone()), Anim { frames, durations: durations.clone() });
                }
            }
        }
        out
    })
}

/// The painted frame of `state` at animation tick `tick`, when every frame of
/// that state is painted.
pub(super) fn frame(id: &str, state: &str, tick: u32) -> Option<&'static Img> {
    let anim = bank().get(&(format!("moonfall.{id}"), state.to_owned()))?;
    let mut time = tick % anim.durations.iter().sum::<u32>().max(1);
    let mut index = 0;
    while index + 1 < anim.durations.len() && time >= anim.durations[index] {
        time -= anim.durations[index];
        index += 1;
    }
    anim.frames.get(index)
}
