//! Delivered empty HUD plates. Live values and glyphs are stamped afterward.
use super::{Img, Run, slot_art};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

pub fn enabled(run: &Run) -> bool {
    slot_art::enabled() && !run.at_home_now() && !run.side_on() && run.circle() == 0
}

/// Nine-slice the native chrome once per envelope, preserving its end caps.
/// There is no frame-time resampling or per-pixel colour conversion.
pub fn skin(name: &'static str, w: i32, h: i32) -> Option<Arc<Img>> {
    if !slot_art::enabled() {
        return None;
    }
    type Cache = Mutex<BTreeMap<(&'static str, i32, i32), Arc<Img>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(im) = cache.get(&(name, w, h)) {
        return Some(im.clone());
    }
    static SOURCES: OnceLock<BTreeMap<&'static str, Img>> = OnceLock::new();
    let sources = SOURCES.get_or_init(|| {
        [
            "ready", "cooldown", "disabled", "pressed", "health", "stamina", "boss", "seat",
            "floor", "minimap", "status", "toast",
        ]
        .into_iter()
        .filter_map(|name| {
            slot_art::bytes("dgn.hud.chrome", name)
                .map(|_| (name, slot_art::load("dgn.hud.chrome", name, &[])))
        })
        .collect()
    });
    let source = sources.get(name)?;
    let mut im = Img::new(w, h);
    let edge = |at: i32, dest: i32, src: i32, cap: i32| {
        if at < cap {
            at
        } else if at >= dest - cap {
            src - (dest - at)
        } else {
            cap + (at - cap) * (src - 2 * cap) / (dest - 2 * cap).max(1)
        }
    };
    let ex = 3.min(w / 3).min(source.w / 3);
    let ey = 2.min(h / 3).min(source.h / 3);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = source.get(edge(x, w, source.w, ex), edge(y, h, source.h, ey)) {
                im.set(x, y, c);
            }
        }
    }
    let im = Arc::new(im);
    cache.insert((name, w, h), im.clone());
    Some(im)
}

pub(super) fn warm() {
    for (name, w, h) in [
        ("ready", 18, 18),
        ("cooldown", 18, 18),
        ("disabled", 18, 18),
        ("ready", 10, 10),
        ("cooldown", 10, 10),
        ("disabled", 10, 10),
        ("health", 18, 4),
        ("health", 24, 2),
        ("seat", 24, 2),
    ] {
        let _ = skin(name, w, h);
    }
}

pub fn plate(cv: &mut Img, name: &'static str, (x, y, w, h): (i32, i32, i32, i32)) -> bool {
    let Some(im) = skin(name, w, h) else {
        return false;
    };
    cv.stamp(&im, x, y);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_seat_trays_read_live_values_without_mutating_the_run() {
        let mut run = Run::new(733, 1, Some("Bot 1"));
        for id in 2..=4 {
            run.join_seat(id, &format!("Bot {id}"));
        }
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == crate::rules::RoomKind::Fight)
            .unwrap();
        run.enter_for_test(room);
        let before = serde_json::to_vec(&run).unwrap();
        for id in 1..=4 {
            let native =
                super::super::tray::image(&run, id, &crate::rules::hud::Keys::FIRST, 384).unwrap();
            let (cells, keys) =
                super::super::tray::small(&run, id, &crate::rules::hud::Keys::FIRST, 96).unwrap();
            assert!(native.w > 0 && cells.w > 0 && !keys.is_empty());
        }
        assert_eq!(before, serde_json::to_vec(&run).unwrap());
    }
    #[test]
    fn chrome_envelopes_cache_native_pixels() {
        for name in [
            "ready", "cooldown", "disabled", "pressed", "health", "stamina", "boss", "seat",
            "floor", "minimap", "status", "toast",
        ] {
            for (w, h) in [(10, 10), (18, 18), (48, 11), (120, 13)] {
                let im = skin(name, w, h).unwrap();
                assert_eq!((im.w, im.h), (w, h));
                assert!(Arc::ptr_eq(&im, &skin(name, w, h).unwrap()));
                assert!((0..h).any(|y| (0..w).any(|x| im.get(x, y).is_some())));
            }
        }
    }
}
