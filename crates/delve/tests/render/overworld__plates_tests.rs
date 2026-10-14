use super::*;
use crate::rules::Pack;

#[test]
fn every_delve_floor_has_a_distinct_deterministic_palette_plate() {
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

/// Offline only: no services or model calls. The moodboard remains external.
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
    let mood = image::open(
        std::env::var_os("ANGEL_PLATE_MOODBOARD")
            .unwrap_or_else(|| "/work/tmp/plates-moodboard.png".into()),
    )
    .unwrap()
    .to_rgb8();
    let mut sheet = image::RgbImage::new(2168, 712);
    let mood = image::imageops::resize(&mood, 984, 522, image::imageops::FilterType::Lanczos3);
    image::imageops::replace(&mut sheet, &mood, 0, 20);
    for (i, &(kind, depth)) in DESTINATIONS.iter().enumerate() {
        let name = format!("{:?}-{depth}", kind).to_lowercase();
        let im = painting(kind, depth, 20);
        save(&format!("{name}.png"), &im);
        let png = image::RgbImage::from_raw(W as u32, H as u32, im.rgb_bytes()).unwrap();
        image::imageops::replace(
            &mut sheet,
            &png,
            1000 + (i as i64 % 3) * 392,
            16 + (i as i64 / 3) * 232,
        );
        save(
            &format!("{name}-terminal.png"),
            &frame(
                Playing {
                    kind,
                    depth,
                    age: 20,
                },
                135,
                78,
            ),
        );
    }
    sheet.save(dir.join("contact-sheet.png")).unwrap();
    // Measure already-cached native frames without making CI depend on wall time.
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
                },
                W,
                H,
            ),
        );
    }
}
