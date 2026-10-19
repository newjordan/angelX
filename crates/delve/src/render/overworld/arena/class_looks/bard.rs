//! Swappable, deliberately plain Bard placeholders. No final art lives here.
use super::{Img, Layer, View};
use crate::rules::{Run, bard::SONG_REACH};
use std::sync::OnceLock;

struct Slots {
    views: [View; 3],
    note: Img,
    sleep: Img,
    ring: Img,
}

pub(super) fn rows(value: &serde_json::Value) -> &'static [&'static str] {
    Box::leak(value.as_array().expect("Bard rows").iter().map(|s| {
        &*Box::leak(s.as_str().expect("Bard ink row").to_string().into_boxed_str())
    }).collect::<Vec<_>>().into_boxed_slice())
}

pub(super) fn layer(value: &serde_json::Value) -> Layer {
    Layer {
        at: (value["at"][0].as_i64().unwrap() as i32, value["at"][1].as_i64().unwrap() as i32),
        rows: rows(&value["rows"]),
    }
}

fn slots() -> &'static Slots {
    static SLOTS: OnceLock<Slots> = OnceLock::new();
    SLOTS.get_or_init(|| {
        let spec: serde_json::Value = serde_json::from_str(include_str!("../../../../../assets/bard/slots.json")).expect("Bard slots");
        let views = std::array::from_fn(|i| {
            let v = &spec["knight"]["views"][i];
            View { body: rows(&v["body"]), item: layer(&v["item"]), strike: layer(&v["strike"]) }
        });
        let mut sleep = Img::new(11, 7);
        sleep.text(0, 0, spec["sleep"]["label"].as_str().unwrap(), '4');
        let r = (SONG_REACH * super::super::UNIT) as i32;
        let mut ring = Img::new(2 * r + 1, 2 * r + 1);
        for y in -r..=r {
            for x in -r..=r {
                let d = x * x + y * y;
                if ((r - 1) * (r - 1)..=r * r).contains(&d) { ring.put(x + r, y + r, '4'); }
            }
        }
        Slots { views, note: Img::from_rows(rows(&spec["note"]["rows"])), sleep, ring }
    })
}

pub(super) fn views() -> &'static [View; 3] { &slots().views }
pub(crate) fn note() -> &'static Img { &slots().note }

pub(crate) fn song_rings(cv: &mut Img, run: &Run) {
    for song in run.bard_songs.iter().filter(|s| s.until > run.tick) {
        if let Some(hero) = run.players.get(&song.owner).filter(|h| h.hp > 0) {
            let (x, y) = super::super::at(hero.x, hero.y);
            let ring = &slots().ring;
            cv.stamp(ring, x - ring.w / 2, y - ring.h / 2);
        }
    }
}

pub(crate) fn sleep_marks(cv: &mut Img, run: &Run) {
    for enemy in run.enemies.iter().filter(|e| e.hp > 0 && e.asleep(run.tick)) {
        let (x, y) = super::super::at(enemy.x, enemy.y);
        let im = &slots().sleep;
        cv.stamp(im, x - im.w / 2, y - 24);
    }
}
