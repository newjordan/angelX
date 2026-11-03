//! The moments of Lanternward that `crates/delve`'s picture tests start from.
//! That crate has no engine, so the moments are played here, with the shipped
//! Bend rules, and written down (`crates/delve/tests/fixtures/
//! lanternward_moments.json`); one test here fails when the rules play a
//! moment differently from what is written. `ANGEL_ARENA_SHOTS=<dir> cargo
//! test --bin angel write_lanternward_film -- --ignored` writes the verb films.
use crate::lanternward_oracle::review::{duel, review};
use angel_delve::rules::{Input, MAX_PLAYERS, Run};
use serde_json::Value;
use std::collections::BTreeMap;

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/delve/tests/fixtures/lanternward_moments.json");

/// Every moment the review sets up, with how many knights.
pub const MOMENTS: &[(&str, u32)] = &[
    ("stair", 1), ("lock", 1), ("circle", 1), ("slash", 1), ("slash3", 1), ("spin", 1), ("lunge", 1),
    ("shield", 1), ("block", 1), ("parry", 1), ("hop", 1), ("flip", 1), ("plunge", 1), ("roll", 1),
    ("chest", 1), ("down", 2), ("retry", 2), ("talk", 1), ("bomb", 1), ("bomb-open", 1), ("sigil", 1), ("lever", 1),
    ("lamp", 1), ("lamp-lit", 1), ("lamp-lantern", 1), ("lamp-wave", 2), ("lamp-flare", 1), ("lamp-throw", 1),
    ("gallery", 1), ("gallery-tell", 1), ("gallery-parry", 1), ("gallery-solved", 2),
    ("vault", 1), ("vault-flare", 1), ("vault-key", 1), ("vault-door", 1),
    ("warden", 2), ("warden-beam", 1), ("warden-slam", 2), ("warden-open", 2), ("warden-dark", 4),
    ("warden-dead", 1), ("beacon", 1), ("finale", 2),
];

/// The moments the picture tests ask for: each at its own table, alone, and
/// the few they size up (a full table, the four-knight Warden).
fn wanted() -> Vec<(&'static str, u32)> {
    let mut out: Vec<(&str, u32)> = MOMENTS.to_vec();
    out.extend(MOMENTS.iter().map(|&(m, _)| (m, 1)));
    out.extend([("stair", 2), ("stair", 4), ("stair", MAX_PLAYERS)]);
    out.extend([("warden-open", 4), ("vault-flare", 6), ("warden-dark", 6), ("warden-slam", 6), ("lamp-wave", 6)]);
    out.sort();
    out.dedup();
    out
}

/// Every wanted moment as the rules play it now, and the fresh table.
fn played() -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (moment, seats) in wanted() {
        let run = review(moment, seats);
        out.insert(format!("{moment}/{seats}"), serde_json::to_value(run.lanternward.as_ref().unwrap()).unwrap());
    }
    crate::agent::harness::live_policy::install_scripted_rules();
    out.insert(
        "new/1".into(),
        serde_json::to_value(Run::lanternward_for_test().lanternward.as_ref().unwrap()).unwrap(),
    );
    out
}

#[test]
fn the_moments_the_pictures_start_from_are_the_ones_the_rules_play() {
    let written: BTreeMap<String, Value> = serde_json::from_str(&std::fs::read_to_string(FILE).unwrap()).unwrap();
    let played = played();
    assert_eq!(written.keys().collect::<Vec<_>>(), played.keys().collect::<Vec<_>>(), "the moments written down");
    for (name, session) in &played {
        assert!(&written[name] == session, "{name} is not what is written: run write_lanternward_moments");
    }
}

#[test]
#[ignore = "writes the moments crates/delve's picture tests read"]
fn write_lanternward_moments() {
    std::fs::create_dir_all(std::path::Path::new(FILE).parent().unwrap()).unwrap();
    std::fs::write(FILE, serde_json::to_string(&played()).unwrap()).unwrap();
}

/// A knight's verb frame by frame: the 56 by 44 pixels about them, three
/// times over, a tick apiece, side by side.
fn film(name: &str, mut r: Run, script: &[(u32, Input)], dir: &std::path::Path) {
    use angel_delve::rules::lanternward::world::Q;
    let mut frames = Vec::new();
    for &(ticks, input) in script {
        for _ in 0..ticks {
            r.step(&BTreeMap::from([(1, input)]));
            let k = &r.lanternward.as_ref().unwrap().seats[&1];
            let full = crate::stage::world_viz::overworld::arena::frame_for(&r, 384, 224, Some(1));
            let screen = image::RgbaImage::from_raw(full.w as u32, full.h as u32, full.rgba_bytes()).unwrap();
            let (cx, cy) = (k.x / Q - (k.cam.0 / Q - 192), k.y / Q - (k.cam.1 / Q - 112));
            let mut crop = image::RgbaImage::new(56, 44);
            for y in 0..44 {
                for x in 0..56 {
                    let (sx, sy) = (cx - 28 + x, cy - 28 + y);
                    if (0..full.w).contains(&sx) && (0..full.h).contains(&sy) {
                        crop.put_pixel(x as u32, y as u32, *screen.get_pixel(sx as u32, sy as u32));
                    }
                }
            }
            frames.push(image::imageops::resize(&crop, 56 * 3, 44 * 3, image::imageops::FilterType::Nearest));
            eprintln!("{name} {}: {:?} t{} n{} z{} inv{}", frames.len() - 1, k.act, k.t, k.n, k.z / Q, k.inv);
        }
    }
    let cols = 6usize;
    let rows = frames.len().div_ceil(cols);
    let mut sheet = image::RgbaImage::new((cols * (56 * 3 + 2)) as u32, (rows * (44 * 3 + 2)) as u32);
    for (i, f) in frames.iter().enumerate() {
        image::imageops::replace(&mut sheet, f, ((i % cols) * (56 * 3 + 2)) as i64, ((i / cols) * (44 * 3 + 2)) as i64);
    }
    sheet.save(dir.join(format!("film-{name}.png"))).unwrap();
}

#[test]
#[ignore = "writes Lanternward's verb films"]
fn write_lanternward_film() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    let sword = Input { swing: true, ..Default::default() };
    let lock = Input { ult: true, ..Default::default() };
    let none = Input::default();
    film("slash", duel(24), &[(1, sword), (11, none), (1, sword), (11, none), (1, sword), (14, none)], &dir);
    film("spin", duel(30), &[(26, sword), (1, none), (16, none)], &dir);
    film("lunge", duel(40), &[(3, lock), (1, Input { swing: true, ult: true, move_x: 1, ..Default::default() }), (20, lock)], &dir);
    let e = |i: Input| Input { interact: true, ult: true, ..i };
    film("hop", duel(50), &[(3, lock), (1, e(Input { move_y: -1, ..Default::default() })), (20, lock)], &dir);
    film("flip", duel(50), &[(3, lock), (1, e(Input::default())), (20, lock)], &dir);
    film(
        "plunge",
        duel(50),
        &[(3, lock), (1, e(Input { move_x: 1, ..Default::default() })), (7, lock), (1, Input { ult: true, swing: true, ..Default::default() }), (10, lock)],
        &dir,
    );
    film("roll", duel(0), &[(1, Input { interact: true, move_x: 1, ..Default::default() }), (18, none)], &dir);
    film("lock", duel(70), &[(2, none), (1, lock), (12, lock)], &dir);
}
