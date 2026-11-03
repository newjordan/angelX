//! Pixel-art and room integration checks, deliberately local to the washroom.
use super::*;
use crate::render::overworld::ink::{ink, is_signal};

fn paint() -> (Img, Img, Vec<Light>) {
    let (w, h) = (a::HALL.0 as i32 * T, a::HALL.1 as i32 * T);
    let mut cv = Img::new(w, h);
    let mut fixtures = Img::new(w, h);
    let mut lights = Vec::new();
    bathroom(&mut cv, &mut fixtures, &mut lights);
    (cv, fixtures, lights)
}

#[test]
fn bathroom_mirror_is_framed_reflective_glass_at_the_public_location() {
    let (_, fixtures, _) = paint();
    let (x, y) = (
        (a::MIRROR_AT.0 * T as f32) as i32,
        (a::MIRROR_AT.1 * T as f32) as i32,
    );
    assert_eq!(fixtures.get(x - 12, y), ink('B'), "oak frame");
    assert_eq!(fixtures.get(x - 9, y), ink('U'), "metal bevel");
    assert_eq!(fixtures.get(x, y), ink('u'), "cool glass, not a portrait");
    assert_eq!(fixtures.get(x - 5, y + 1), ink('V'), "diagonal reflection");
    assert_eq!(fixtures.get(x, y + 9), ink('V'), "lower bevel");
}

#[test]
fn bathroom_has_separate_basin_jug_bucket_and_towel_clusters_in_plain_inks() {
    let (cv, fixtures, lights) = paint();
    for (name, x, y, w, h, min) in [
        ("basin", 38 * T + 11, 2 * T, 21, 9, 70),
        ("jug", 38 * T + 1, 2 * T - 7, 11, 11, 35),
        ("bucket", 41 * T, 5 * T, 16, 16, 80),
        ("towel", 43 * T - 24, T + 3, 12, 22, 190),
    ] {
        let count = (y..y + h)
            .flat_map(|py| (x..x + w).map(move |px| (px, py)))
            .filter(|&(x, y)| fixtures.get(x, y).is_some())
            .count();
        assert!(count >= min, "{name} silhouette is missing: {count}");
    }
    for img in [&cv, &fixtures] {
        for y in 0..img.h {
            for x in 0..img.w {
                if let Some(c) = img.get(x, y) {
                    // The existing lantern's flame is intentionally emissive;
                    // crockery, glass and linen are ordinary lit materials.
                    let flame =
                        (36 * T + 8..36 * T + 11).contains(&x) && (T + 5..T + 8).contains(&y);
                    assert!(
                        !is_signal(c) || flame,
                        "only the lantern flame uses emissive ink"
                    );
                }
            }
        }
    }
    assert!(lights.iter().any(|l| l.fire), "warm local lantern light");
    let (c, r, w, h) = a::BATHROOM_DOOR;
    assert!(cv.get(c * T + w * T / 2, r * T + h * T / 2).is_some());
    assert!(
        fixtures.get(c * T + w * T / 2, r * T + h * T / 2).is_none(),
        "no fixture closes the doorway"
    );
}

#[test]
fn bathroom_survives_the_tavern_scenery_lighting_and_old_halls_still_render() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.walk_alehouse();
    let cv = scenery(&r);
    assert_eq!((cv.w, cv.h), (a::HALL.0 as i32 * T, a::HALL.1 as i32 * T));
    let (x, y) = (
        (a::MIRROR_AT.0 * T as f32) as i32,
        (a::MIRROR_AT.1 * T as f32) as i32,
    );
    assert_ne!(
        cv.get(x, y),
        cv.get(x - 12, y),
        "glass must remain distinct from oak after dusk"
    );
    if let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbaImage::from_raw(cv.w as u32, cv.h as u32, cv.rgba_bytes())
            .unwrap()
            .save(dir.join("alehouse-bathroom.png"))
            .unwrap();
    }
    // An already-saved hall predating the annex has no extra light or art.
    let index = r.at;
    let original = r.room();
    let tiles: Vec<_> = (0..20)
        .flat_map(|row| (0..36).map(move |col| (col, row)))
        .map(|(col, row)| {
            if col == 35 {
                Tile::Wall
            } else {
                original.tile(col, row)
            }
        })
        .collect();
    let mut old = serde_json::to_value(original).unwrap();
    old["cols"] = serde_json::json!(36);
    old["tiles"] = serde_json::to_value(tiles).unwrap();
    r.dungeon.rooms[index] = serde_json::from_value(old).unwrap();
    assert!(r.valid_snapshot(), "an old hall remains a legal checkpoint");
    assert_eq!(scenery(&r).w, 36 * T);
}
