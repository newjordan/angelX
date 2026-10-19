use super::*;
use crate::rules::Pack;

struct Original;
impl Original { fn new() -> Self { super::super::slot_art::use_original(true); Self } }
impl Drop for Original { fn drop(&mut self) { super::super::slot_art::use_original(false); } }


#[test]
fn every_delve_floor_has_a_distinct_deterministic_palette_plate() {
    let _original = Original::new();
    let mut hashes = std::collections::HashSet::new();
    for first in [Pack::Crypt, Pack::Cavern] {
        for depth in 1..=6 {
            assert!(DESTINATIONS.contains(&(Kind::of(Pack::at(depth, first)), depth)));
        }
    }
    for &(kind, depth) in &DESTINATIONS {
        let im = painting(kind, depth, 20);
        assert_eq!(im, painting(kind, depth, 20));
        assert_eq!((im.w, im.h), (384, 224));
        assert!(
            hashes.insert(im.content_hash()),
            "distinct composition for {kind:?} {depth}"
        );
        assert!(
            im.pixels()
                .all(|c| super::super::super::ink::palette_index(c).is_some())
        );
        for (w, h) in [
            (384, 224),
            (768, 448),
            (135, 78),
            (80, 44),
            (53, 30),
            (0, 0),
        ] {
            let p = Playing {
                kind,
                depth,
                age: 20,
                results: 0,
            };
            let fitted = frame(p, w, h);
            assert_eq!((fitted.w, fitted.h), (w, h));
            assert_eq!(fitted, frame(p, w, h));
            assert!(
                fitted
                    .pixels()
                    .all(|c| super::super::super::ink::palette_index(c).is_some())
            );
        }
        assert_ne!(im, painting(kind, depth, 31), "motion in {kind:?}");
    }
}

#[test]
fn floor_plate_motion_preserves_the_intertitle_and_most_of_the_painting() {
    let _original = Original::new();
    for &(kind, depth) in &DESTINATIONS {
        let before = painting(kind, depth, 20);
        let after = painting(kind, depth, 31);
        let mut changed = 0;
        for y in 0..H {
            for x in 0..W {
                if y >= HORIZON {
                    assert_eq!(before.get(x, y), after.get(x, y), "stable title {kind:?}");
                } else if before.get(x, y) != after.get(x, y) {
                    changed += 1;
                }
            }
        }
        assert!(changed > 0, "live light in {kind:?} {depth}");
        assert!(
            changed < W * HORIZON / 8,
            "static scenery in {kind:?} {depth}"
        );
    }
}

#[test]
fn repeated_destinations_have_different_scenes_above_the_title() {
    let _original = Original::new();
    for kind in [Kind::Crypt, Kind::Mines] {
        let first = painting(kind, 1, 27);
        let second = painting(kind, 2, 27);
        let changed = (0..HORIZON)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .filter(|&(x, y)| first.get(x, y) != second.get(x, y))
            .count();
        assert!(
            changed > (W * HORIZON / 4) as usize,
            "separate scenes for {kind:?}"
        );
    }
}

#[test]
fn paintings_disappear_into_black_before_the_canvas_edges() {
    let _original = Original::new();
    for &(kind, depth) in &DESTINATIONS {
        let im = painting(kind, depth, 27);
        for x in 0..W {
            assert_eq!(im.get(x, 0), Some(BLACK));
            assert_eq!(im.get(x, HORIZON - 1), Some(BLACK));
        }
        for y in 0..HORIZON {
            assert_eq!(im.get(0, y), Some(BLACK));
            assert_eq!(im.get(W - 1, y), Some(BLACK));
        }
    }
}

/// Offline native and terminal frames, including a native cache timing sample.
#[test]
#[ignore]
fn write_floor_plate_shots() {
    let Some(dir) = std::env::var_os("ANGEL_PLATE_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: &Img| {
        image::RgbImage::from_raw(im.w as u32, im.h as u32, im.rgb_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    for &(kind, depth) in &DESTINATIONS {
        let name = format!("{kind:?}-{depth}").to_lowercase();
        for age in [8, 27, 45] {
            save(&format!("{name}-{age:02}.png"), &painting(kind, depth, age));
            save(
                &format!("{name}-{age:02}-terminal.png"),
                &frame(Playing { kind, depth, age, results: 0 }, 135, 78),
            );
        }
    }
    // Measure warmed caches without adding a wall-time assertion to CI.
    let mut timings =
        String::from("Cached 384x224 painting, debug test build, 200 frames per destination\n");
    for &(kind, depth) in &DESTINATIONS {
        let start = std::time::Instant::now();
        for sample in 0..200 {
            std::hint::black_box(painting(kind, depth, (sample % DURATION as u32) as u8));
        }
        timings.push_str(&format!(
            "{kind:?}-{depth}: {:.3} ms/frame\n",
            start.elapsed().as_secs_f64() * 5.0
        ));
    }
    std::fs::write(dir.join("frame-times.txt"), timings).unwrap();
    for age in [2, 20, 51] {
        save(
            &format!("transition-{age:02}.png"),
            &frame(
                Playing {
                    kind: Kind::Crypt,
                    depth: 1,
                    age,
                    results: 0,
                },
                W,
                H,
            ),
        );
    }
}

#[test]
fn delivered_plates_fill_both_native_grains_and_keep_the_original_titles() {
    // The swamp's plate is painted here; no plate is delivered for it.
    for &(kind, depth) in DESTINATIONS.iter().filter(|(k, _)| *k != Kind::Swamp) {
        for scale in [1,2] {
            let im = delivered(kind,depth,scale).expect("reviewed plate embedded");
            assert_eq!((im.w,im.h),(W*scale,H*scale));
            assert!(im.pixels().all(|c| super::super::super::ink::palette_index(c).is_some()));
            let frame = frame(Playing { kind, depth, age:27, results: 0 },W*scale,H*scale);
            assert_eq!(&frame,im);
            assert_eq!(frame,super::frame(Playing {kind,depth,age:20,results:0},W*scale,H*scale));
            let mut label = Img::new(W,H); title(&mut label,kind,depth);
            for y in 189..H { for x in 0..W { if let Some(c)=label.get(x,y) {
                assert_eq!(im.get(x*scale,y*scale),Some(c));
            } } }
            assert!(im.get(W*scale/2,50*scale).is_some_and(|c|c!=BLACK));
        }
        let low = delivered(kind,depth,1).unwrap();
        let high = delivered(kind,depth,2).unwrap();
        assert!((0..HORIZON*2).any(|y|(0..W*2).any(|x|high.get(x,y)!=low.get(x/2,y/2))),"v2 must sample the rich source independently");
    }
}
