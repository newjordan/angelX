//! Standalone native-grain art stage. It has no Run or gameplay integration.
use super::{art, overworld::ink::Img};
use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};
include!(concat!(env!("OUT_DIR"), "/arcade_assets.rs"));
struct Atlas {
    frames: HashMap<String, [Img; 4]>,
    manifests: [Value; 2],
}
fn hex(s: &str) -> [u8; 3] {
    let n = u32::from_str_radix(s, 16).unwrap();
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}
fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| {
        let bank: Value =
            serde_json::from_str(include_str!("../../assets/arcade/palette.json")).unwrap();
        let cloth: Vec<_> = bank["ramps"]["cloth"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| hex(v.as_str().unwrap()))
            .collect();
        let mut frames = HashMap::new();
        for &(name, bytes) in FILES {
            let source = image::load_from_memory(bytes)
                .expect("arcade sprite")
                .to_rgba8();
            let seats = std::array::from_fn(|seat| {
                let ramp: Vec<_> = bank["seats"][seat]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| hex(v.as_str().unwrap()))
                    .collect();
                let mut out = Img::new(source.width() as i32, source.height() as i32);
                for (x, y, p) in source.enumerate_pixels() {
                    if p[3] == 255 {
                        let mut c = [p[0], p[1], p[2]];
                        if name.starts_with("knight/") {
                            if let Some(i) = cloth.iter().position(|v| *v == c) {
                                c = ramp[i];
                            }
                        }
                        // Exact same actor grade, baked once rather than per frame.
                        c = art::grade_actor(c, [0.35, 0.30, 0.28]);
                        out.set(x as i32, y as i32, c);
                    }
                }
                out
            });
            frames.insert(name.to_owned(), seats);
        }
        Atlas {
            frames,
            manifests: [
                serde_json::from_str(include_str!("../../assets/arcade/knight/manifest.json"))
                    .unwrap(),
                serde_json::from_str(include_str!("../../assets/arcade/fx/manifest.json")).unwrap(),
            ],
        }
    })
}
pub fn manifest(kit: &str) -> &'static Value {
    &atlas().manifests[usize::from(kit == "fx")]
}
pub fn states(kit: &str) -> Vec<String> {
    manifest(kit)["states"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}
pub fn frame_count(kit: &str, state: &str) -> usize {
    manifest(kit)["states"][state]["frames"]
        .as_array()
        .unwrap()
        .len()
}
pub fn frame_index(kit: &str, state: &str, ms: u64) -> usize {
    let times = manifest(kit)["states"][state]["durations_ms"]
        .as_array()
        .unwrap();
    let total: u64 = times.iter().map(|v| v.as_u64().unwrap()).sum();
    let mut t = if manifest(kit)["states"][state]["loop"].as_bool().unwrap() {
        ms % total
    } else {
        ms.min(total - 1)
    };
    for (i, time) in times.iter().enumerate() {
        let d = time.as_u64().unwrap();
        if t < d {
            return i;
        }
        t -= d;
    }
    0
}
pub fn elapsed_for(kit: &str, state: &str, index: usize) -> u64 {
    manifest(kit)["states"][state]["durations_ms"]
        .as_array()
        .unwrap()
        .iter()
        .take(index)
        .map(|v| v.as_u64().unwrap())
        .sum()
}
pub fn sprite(kit: &str, state: &str, index: usize, seat: usize) -> &'static Img {
    let file = manifest(kit)["states"][state]["frames"][index]["file"]
        .as_str()
        .unwrap();
    &atlas().frames[&format!("{kit}/{file}")][seat]
}
/// 384×224 at one native sample per pixel; the anchor is presentation-only.
pub fn layers(kit: &str, state: &str, ms: u64, seat: usize) -> (Img, Img) {
    let mut base = Img::black(384, 224);
    let mut fx = Img::new(384, 224);
    for y in 181..189 {
        for x in 96..288 {
            base.set(x, y, [10, 12, 18]);
        }
    }
    let m = manifest(kit);
    let anchor = (
        m["anchor"][0].as_i64().unwrap() as i32,
        m["anchor"][1].as_i64().unwrap() as i32,
    );
    let im = sprite(kit, state, frame_index(kit, state, ms), seat);
    let (ox, oy) = (192 - anchor.0, 180 - anchor.1);
    base.stamp(im, ox, oy);
    if kit == "fx" {
        // Peripheral soot is atmosphere; hot lobes and impact cores stay solid.
        for y in 0..im.h {
            for x in 0..im.w {
                if let Some(c) = im.get(x, y) {
                    if c.iter().copied().max().unwrap() < 95 && y < im.h - 10 {
                        fx.set(ox + x, oy + y, c);
                    }
                }
            }
        }
    }
    (base, fx)
}
pub fn frame(kit: &str, state: &str, ms: u64, seat: usize) -> Img {
    let (base, fx) = layers(kit, state, ms, seat);
    art::native(&base, &fx)
}
