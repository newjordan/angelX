//! Lanternward's stage through the real arena: every room and moment draws on
//! both routes, and `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
//! write_lanternward_review -- --ignored` writes the pictures to open.
use super::super::super::ink::ink;
use super::super::{art_v2, frame_for};
use super::*;
use crate::rules::{Input, MAX_PLAYERS, Run, lanternward as lw};
use std::collections::BTreeMap;

fn party(seats: u32) -> Run {
    let mut r = Run::lanternward_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

fn save(dir: &std::path::Path, name: &str, img: &Img) {
    image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

#[test]
fn the_stage_draws_on_both_routes_at_every_size() {
    for seats in [1, 2, 4, MAX_PLAYERS] {
        let r = party(seats);
        let (base, fx) = layers(&r, Some(1));
        assert_eq!((base.w, base.h, fx.w, fx.h), (384, 224, 384, 224));
        assert!(base.pixels().any(|p| p != [0, 0, 0]));
        let (cells, _) = terminal_layers(&r, Some(1));
        assert_eq!((cells.w, cells.h), (384, 224));
        assert!(art_v2::terminal_layers(&r, Some(1)).is_some());
        for (w, h) in [(384, 224), (768, 448), (1152, 672), (96, 56)] {
            let img = frame_for(&r, w, h, Some(1));
            assert_eq!((img.w, img.h), (w, h));
        }
    }
}

/// Every moment the review sets up, with how many knights.
const MOMENTS: &[(&str, u32)] = &[
    ("stair", 1), ("lock", 1), ("circle", 1), ("slash", 1), ("slash3", 1), ("spin", 1), ("lunge", 1),
    ("shield", 1), ("block", 1), ("parry", 1), ("hop", 1), ("flip", 1), ("plunge", 1), ("roll", 1),
    ("chest", 1), ("down", 2), ("retry", 2), ("talk", 1), ("bomb", 1), ("bomb-open", 1), ("sigil", 1), ("lever", 1),
    ("lamp", 1), ("lamp-lit", 1), ("lamp-lantern", 1), ("lamp-wave", 2), ("lamp-flare", 1), ("lamp-throw", 1),
    ("gallery", 1), ("gallery-tell", 1), ("gallery-parry", 1), ("gallery-solved", 2),
    ("vault", 1), ("vault-flare", 1), ("vault-key", 1), ("vault-door", 1),
    ("warden", 2), ("warden-beam", 1), ("warden-slam", 2), ("warden-open", 2), ("warden-dark", 4),
    ("warden-dead", 1), ("beacon", 1), ("finale", 2),
];

#[test]
fn every_moment_draws_without_panicking() {
    for &(m, seats) in MOMENTS {
        let r = Run::lanternward_review_for_test(m, seats);
        let (base, _) = layers(&r, Some(1));
        assert!(base.pixels().any(|p| p != [0, 0, 0]), "{m}");
        let (cells, _) = terminal_layers(&r, Some(1));
        assert_eq!((cells.w, cells.h), (384, 224), "{m}");
    }
}

/// A knight's verb frame by frame: the 64 by 48 pixels about them, three
/// times over, a tick apiece, side by side.
fn film(name: &str, mut r: Run, script: &[(u32, Input)], dir: &std::path::Path) {
    let mut frames = Vec::new();
    for &(ticks, input) in script {
        for _ in 0..ticks {
            r.step(&BTreeMap::from([(1, input)]));
            let s = r.lanternward.as_ref().unwrap();
            let k = &s.seats[&1];
            let full = layers(&r, Some(1)).0;
            let (cx, cy) = (k.x / lw::world::Q - (k.cam.0 / lw::world::Q - 192), k.y / lw::world::Q - (k.cam.1 / lw::world::Q - 112));
            let mut crop = Img::black(56, 44);
            for y in 0..44 {
                for x in 0..56 {
                    if let Some(c) = full.get(cx - 28 + x, cy - 28 + y) {
                        crop.set(x, y, c);
                    }
                }
            }
            frames.push(crop);
            eprintln!("{name} {}: {:?} t{} n{} z{} inv{}", frames.len() - 1, k.act, k.t, k.n, k.z / lw::world::Q, k.inv);
        }
    }
    let cols = 6usize;
    let rows = frames.len().div_ceil(cols);
    let mut sheet = Img::black((cols * 56 * 3 + cols * 2) as i32, (rows * 44 * 3 + rows * 2) as i32);
    for (i, f) in frames.iter().enumerate() {
        let (x, y) = ((i % cols) as i32 * (56 * 3 + 2), (i / cols) as i32 * (44 * 3 + 2));
        let mut big = Img::black(56 * 3, 44 * 3);
        big.stamp_times(f, 0, 0, 3);
        sheet.stamp(&big, x, y);
    }
    save(dir, &format!("film-{name}"), &sheet);
}

#[test]
#[ignore = "writes Lanternward's review pictures"]
fn write_lanternward_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    let only = std::env::var("LW_ONLY").ok();
    for &(m, seats) in MOMENTS {
        if only.as_deref().is_some_and(|o| !o.split(',').any(|x| x == m)) {
            continue;
        }
        let r = Run::lanternward_review_for_test(m, seats);
        save(&dir, &format!("{m}-1x"), &frame_for(&r, 384, 224, Some(1)));
        save(&dir, &format!("{m}-2x"), &frame_for(&r, 768, 448, Some(1)));
        save(&dir, &format!("{m}-3x"), &frame_for(&r, 1152, 672, Some(1)));
    }
    // Two screens of the same party, side by side: each seat's own camera.
    let r = Run::lanternward_review_for_test("warden-open", 4);
    let mut both = Img::black(768, 224);
    both.stamp(&frame_for(&r, 384, 224, Some(1)), 0, 0);
    both.stamp(&frame_for(&r, 384, 224, Some(3)), 384, 0);
    save(&dir, "two-screens", &both);
    let sword = Input { swing: true, ..Default::default() };
    let lock = Input { ult: true, ..Default::default() };
    let none = Input::default();
    let duel = |d| Run::lanternward_duel_for_test(d);
    film("slash", duel(24), &[(1, sword), (11, none), (1, sword), (11, none), (1, sword), (14, none)], &dir);
    film("spin", duel(30), &[(26, sword), (1, none), (16, none)], &dir);
    film("lunge", duel(40), &[(3, lock), (1, Input { swing: true, ult: true, move_x: 1, ..Default::default() }), (20, lock)], &dir);
    let e = |i: Input| Input { interact: true, ult: true, ..i };
    film("hop", duel(50), &[(3, lock), (1, e(Input { move_y: -1, ..Default::default() })), (20, lock)], &dir);
    film("flip", duel(50), &[(3, lock), (1, e(Input::default())), (20, lock)], &dir);
    film("plunge", duel(50), &[(3, lock), (1, e(Input { move_x: 1, ..Default::default() })), (7, lock), (1, Input { ult: true, swing: true, ..Default::default() }), (10, lock)], &dir);
    film("roll", duel(0), &[(1, Input { interact: true, move_x: 1, ..Default::default() }), (18, none)], &dir);
    film("lock", duel(70), &[(2, none), (1, lock), (12, lock)], &dir);
    let _ = (lw::NAME, party(1));
}

/// Where the brazier nearest the first knight stands on his screen: its middle
/// column and its foot.
fn brazier_spot(r: &Run) -> (i32, i32) {
    let s = r.lanternward.as_ref().unwrap();
    let seat = &s.seats[&1];
    let p = s
        .props
        .iter()
        .filter(|p| p.kind == lw::Pk::Brazier)
        .min_by_key(|p| (p.x - seat.x).abs() + (p.y - seat.y).abs())
        .unwrap();
    (
        p.x / lw::world::Q - (seat.cam.0 / lw::world::Q - 192),
        p.y / lw::world::Q - (seat.cam.1 / lw::world::Q - 112) + 8,
    )
}

/// Whether every opaque pixel of slot `id`'s `state` frame at `tick` is on
/// `screen`, its bottom centre at `(x, foot)`.
fn frame_shows(screen: &Img, spot: (i32, i32), id: &str, state: &str, tick: u32) -> bool {
    frame_misses(screen, spot, id, state, tick).0 == 0
}

/// How many of `state`'s opaque pixels are not the frame's own on `screen`,
/// and how many it has.
fn frame_misses(screen: &Img, (x, foot): (i32, i32), id: &str, state: &str, tick: u32) -> (usize, usize) {
    let pic = bank::frame(id, state, tick);
    let mut counts = (0, 0);
    for y in 0..pic.h {
        for px in 0..pic.w {
            if let Some(c) = pic.get(px, y) {
                counts.1 += 1;
                counts.0 += usize::from(screen.get(x - pic.w / 2 + px, foot - pic.h + y) != Some(c));
            }
        }
    }
    counts
}

fn brazier_shows(screen: &Img, spot: (i32, i32), state: &str, tick: u32) -> bool {
    frame_shows(screen, spot, "brazier", state, tick)
}

#[test]
fn a_painted_brazier_burns_down_through_its_dying_flame_once_and_in_order() {
    if !bank::painted("brazier", "gutter") {
        return;
    }
    let mut r = Run::lanternward_review_for_test("lamp-lit", 1);
    let spot = brazier_spot(&r);
    let shows = |r: &mut Run, t: u16, state: &str, tick: u32| {
        for p in r.lanternward.as_mut().unwrap().props.iter_mut().filter(|p| p.kind == lw::Pk::Brazier) {
            p.t = t;
            p.on = 0;
        }
        brazier_shows(&layers(r, Some(1)).0, spot, state, tick)
    };
    let elapsed = r.lanternward.as_ref().unwrap().elapsed;
    assert!(shows(&mut r, 400, "lit", elapsed), "alight");
    assert!(!shows(&mut r, 13, "gutter", 0), "thirteen ticks left is still the lit flame");
    // The last twelve ticks, four to a frame, first to last.
    for (t, tick) in [(12, 0), (9, 3), (8, 4), (5, 7), (4, 8), (1, 11)] {
        assert!(shows(&mut r, t, "gutter", tick), "{t} ticks left");
    }
    assert!(!shows(&mut r, 8, "gutter", 0), "the frames are not played out of order");
    assert!(shows(&mut r, 0, "out", 0), "burnt out");
    // A brazier lit for good never gutters.
    for p in r.lanternward.as_mut().unwrap().props.iter_mut().filter(|p| p.kind == lw::Pk::Brazier) {
        p.t = 3;
        p.on = 1;
    }
    assert!(!brazier_shows(&layers(&r, Some(1)).0, spot, "gutter", 9));
}

/// One brazier, tick by tick at six times its size: lit through its flame
/// shapes, then burning down to its last twelve ticks and out.
#[test]
#[ignore = "writes the brazier's review strip"]
fn write_lanternward_brazier_strip() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    let mut r = Run::lanternward_review_for_test("lamp-lit", 1);
    let (w, h, k) = (28, 34, 6);
    let mut frames = Vec::new();
    let grab = |r: &Run, frames: &mut Vec<Img>| {
        let (x, foot) = brazier_spot(r);
        let full = layers(r, Some(1)).0;
        let mut crop = Img::black(w, h);
        for y in 0..h {
            for x0 in 0..w {
                if let Some(c) = full.get(x - w / 2 + x0, foot - h + 4 + y) {
                    crop.set(x0, y, c);
                }
            }
        }
        let mut big = Img::black(w * k, h * k);
        big.stamp_times(&crop, 0, 0, k);
        frames.push(big);
    };
    let set_t = |r: &mut Run, t: u16| {
        for p in r.lanternward.as_mut().unwrap().props.iter_mut().filter(|p| p.kind == lw::Pk::Brazier) {
            p.t = t;
            p.on = 0;
        }
    };
    set_t(&mut r, 0);
    grab(&r, &mut frames);
    set_t(&mut r, 400);
    for _ in 0..16 {
        r.step(&BTreeMap::new());
        grab(&r, &mut frames);
    }
    set_t(&mut r, 15);
    for _ in 0..16 {
        r.step(&BTreeMap::new());
        grab(&r, &mut frames);
    }
    let cols = 9usize;
    let rows = frames.len().div_ceil(cols);
    let mut sheet = Img::black((cols as i32) * (w * k + 2), (rows as i32) * (h * k + 2));
    for (i, f) in frames.iter().enumerate() {
        sheet.stamp(f, (i % cols) as i32 * (w * k + 2), (i / cols) as i32 * (h * k + 2));
    }
    save(&dir, "brazier-strip", &sheet);
    // The terminal's cell size, through the same fitter the cells use.
    let r = Run::lanternward_review_for_test("lamp-lit", 1);
    save(&dir, "lamp-lit-cells", &frame_for(&r, 96, 56, Some(1)));
}

/// Put the first knight's camera on the `nth` prop of `kind`: the prop's middle
/// column and its foot on the knight's screen (in its upper half, clear of the
/// stair's printed line), the knight well clear of it.
fn camera_on(r: &mut Run, kind: lw::Pk, nth: usize) -> (usize, (i32, i32)) {
    let s = r.lanternward.as_mut().unwrap();
    let i = s.props.iter().enumerate().filter(|(_, p)| p.kind == kind).map(|(i, _)| i).nth(nth).expect("the room has the prop");
    let (x, y) = (s.props[i].x, s.props[i].y);
    let seat = s.seats.get_mut(&1).unwrap();
    seat.x = x - 7 * 16 * lw::world::Q;
    seat.y = y + 4 * 16 * lw::world::Q;
    seat.cam = (x, y + 48 * lw::world::Q);
    (i, (192, 64 + 8))
}

/// Draw the first knight's screen with prop `i` as `on` and `t`, at tick `tick`.
fn draw_prop(r: &mut Run, i: usize, on: u8, t: u16, tick: u32) -> Img {
    let s = r.lanternward.as_mut().unwrap();
    s.props[i].on = on;
    s.props[i].t = t;
    s.elapsed = tick;
    layers(r, Some(1)).0
}

#[test]
fn a_painted_prop_stamps_its_own_frames_and_the_code_adds_no_art_of_its_own() {
    use lw::Pk;
    // (moment, kind, nth, [(on, t, state, ticks to try)])
    let cases: [(&str, Pk, usize, &[(u8, u16, &str, &[u32])]); 5] = [
        ("lamp", Pk::Great, 0, &[(0, 0, "out", &[0, 5]), (1, 0, "lit", &[0, 4, 8, 12])]),
        ("stair", Pk::Chest, 0, &[(0, 0, "closed", &[0]), (1, 0, "open", &[0])]),
        ("gallery", Pk::Gate, 0, &[(0, 0, "closed", &[0])]),
        ("gallery", Pk::Crystal, 0, &[(0, 0, "dormant", &[0, 12]), (1, 0, "lit", &[0, 4, 8])]),
        ("beacon", Pk::Beacon, 0, &[(0, 0, "cold", &[0]), (0, 9, "kindling", &[0, 6]), (1, 0, "lit", &[0, 4, 8, 12])]),
    ];
    for (moment, kind, nth, states) in cases {
        let mut r = Run::lanternward_review_for_test(moment, 1);
        let (i, spot) = camera_on(&mut r, kind, nth);
        let id = format!("{kind:?}").to_lowercase();
        let id = id.as_str();
        for &(on, t, state, ticks) in states {
            if !bank::painted(id, state) {
                continue;
            }
            for &tick in ticks {
                let screen = draw_prop(&mut r, i, on, t, tick);
                // A lit beacon's pillar and ring of rays are drawn over its top by
                // design (the stage's light, not the block's art): the frame is
                // otherwise its own to within a tenth of its pixels.
                let (missed, of) = frame_misses(&screen, spot, id, state, tick);
                let allowed = if (id, state) == ("beacon", "lit") { of / 10 } else { 0 };
                assert!(missed <= allowed, "{id} {state} at tick {tick}: {missed} of {of} pixels are not the frame's");
            }
        }
    }
}

#[test]
fn a_painted_great_brazier_beacon_and_chest_leave_the_blocks_code_art_out() {
    use lw::Pk;
    // Above the frame's top row, where the block's flame is drawn, a painted lit
    // prop draws what its unlit twin does.
    let above = |screen: &Img, (x, foot): (i32, i32), tall: i32| -> Vec<Option<[u8; 3]>> {
        (1..=10).flat_map(|dy| (-5..=5).map(move |dx| (dx, dy))).map(|(dx, dy)| screen.get(x + dx, foot - tall - dy)).collect()
    };
    for (moment, kind, tall, lit, unlit, id, state) in [
        ("lamp", Pk::Great, 22, (1u8, 0u16), (0u8, 0u16), "great", "lit"),
        ("beacon", Pk::Beacon, 36, (0, 9), (0, 0), "beacon", "kindling"),
    ] {
        if !bank::painted(id, state) {
            continue;
        }
        let mut r = Run::lanternward_review_for_test(moment, 1);
        let (i, spot) = camera_on(&mut r, kind, 0);
        let cold = above(&draw_prop(&mut r, i, unlit.0, unlit.1, 0), spot, tall);
        let burning = above(&draw_prop(&mut r, i, lit.0, lit.1, 0), spot, tall);
        assert_eq!(cold, burning, "{id}: no code flame over a painted {state} frame");
    }
    // An open painted chest has its own mouth: the block's black one is not
    // drawn over its lid.
    if bank::painted("chest", "open") {
        let mut r = Run::lanternward_review_for_test("stair", 1);
        let (i, (x, foot)) = camera_on(&mut r, Pk::Chest, 0);
        let screen = draw_prop(&mut r, i, 1, 0, 0);
        let pic = bank::frame("chest", "open", 0);
        let (px, py) = (pic.w / 2 - 5, pic.h - 13);
        let mouth = (0..3).flat_map(|dy| (0..10).map(move |dx| (dx, dy)));
        for (dx, dy) in mouth {
            let want = pic.get(px + dx, py + dy);
            let got = screen.get(x - 5 + dx, foot - 13 + dy);
            assert!(want.is_none() || want == got, "the open chest's lid pixel ({dx}, {dy}) is its own");
        }
    }
}

#[test]
fn a_gate_opened_is_the_painted_open_arch_and_the_blocks_posts_only_without_one() {
    let mut r = Run::lanternward_review_for_test("gallery", 1);
    let (i, (x, foot)) = camera_on(&mut r, lw::Pk::Gate, 0);
    let spot = (x, foot);
    let closed = draw_prop(&mut r, i, 0, 0, 0);
    let open = draw_prop(&mut r, i, 1, 0, 0);
    if bank::painted("gate", "open") {
        assert!(frame_shows(&open, spot, "gate", "open", 0), "an open gate is its painted arch");
        assert!(frame_shows(&closed, spot, "gate", "closed", 0), "a shut gate is its painted bars, with none of the block's");
    } else {
        // The block's two posts, a tile's edges, and nothing between.
        assert_eq!(open.get(x - 8, foot - 16), ink('j'));
        assert_eq!(open.get(x + 7, foot - 16), ink('j'));
    }
}

/// A prop painted in some of its states and a block in others looks different
/// when it changes state: a bank is painted whole or not at all.
#[test]
fn a_bank_with_one_frame_missing_is_blocks_in_every_state() {
    let (painted, block) = (Img::black(4, 4), Img::new(4, 4));
    let mut states = vec![
        ("a".to_owned(), vec![(painted.clone(), true), (painted.clone(), true)]),
        ("b".to_owned(), vec![(painted.clone(), true), (block.clone(), false)]),
    ];
    bank::whole_or_blocks(&mut states, &block);
    assert!(states.iter().all(|(_, frames)| frames.iter().all(|(im, p)| !p && *im == block)));
    let mut whole = vec![("a".to_owned(), vec![(painted.clone(), true)]), ("b".to_owned(), vec![(painted.clone(), true)])];
    bank::whole_or_blocks(&mut whole, &block);
    assert!(whole.iter().all(|(_, frames)| frames.iter().all(|(im, p)| *p && *im == painted)));
    // And every slot of the real bank is one or the other.
    let slots: Vec<serde_json::Value> = serde_json::from_str(include_str!("../../assets/lanternward/slots.json")).unwrap();
    for slot in slots {
        let id = slot["id"].as_str().unwrap().trim_start_matches("lanternward.").to_owned();
        let flags: Vec<bool> = slot["states"].as_object().unwrap().keys().map(|state| bank::painted(&id, state)).collect();
        assert!(flags.iter().all(|f| *f) || flags.iter().all(|f| !*f), "{id}: {flags:?}");
    }
}

/// Each prop through its states on its own room's floor, one tile and a half
/// either side, at 1x and 3x: what the review pictures are cut from.
#[test]
#[ignore = "writes the five props' review strips"]
fn write_lanternward_prop_strips() {
    use lw::Pk;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    // (name, moment, kind, nth, [(on, t, tick, label)])
    let cases: [(&str, &str, Pk, usize, &[(u8, u16, u32, &str)]); 6] = [
        ("great", "lamp", Pk::Great, 0, &[(0, 0, 0, "out"), (1, 0, 0, "lit-0"), (1, 0, 4, "lit-1"), (1, 0, 8, "lit-2"), (1, 0, 12, "lit-3")]),
        ("chest", "stair", Pk::Chest, 0, &[(0, 0, 0, "closed"), (1, 0, 0, "open")]),
        ("gate", "gallery", Pk::Gate, 0, &[(0, 0, 0, "closed"), (1, 0, 0, "open")]),
        ("crystal", "gallery", Pk::Crystal, 0, &[(0, 0, 0, "dormant-0"), (0, 0, 12, "dormant-1"), (1, 0, 0, "lit-0"), (1, 0, 4, "lit-1"), (1, 0, 8, "lit-2")]),
        ("beacon", "beacon", Pk::Beacon, 0, &[(0, 0, 0, "cold"), (0, 9, 0, "kindling-0"), (0, 9, 6, "kindling-1"), (1, 0, 0, "lit-0"), (1, 0, 4, "lit-1"), (1, 0, 8, "lit-2"), (1, 0, 12, "lit-3")]),
        ("chest-vault", "vault", Pk::Chest, 0, &[(0, 0, 0, "closed"), (1, 0, 0, "open")]),
    ];
    for (name, moment, kind, nth, states) in cases {
        let mut r = Run::lanternward_review_for_test(moment, 1);
        let (i, (x, foot)) = camera_on(&mut r, kind, nth);
        // The dark vault shows what its knight's own light reaches: stand beside.
        if moment == "vault" {
            let s = r.lanternward.as_mut().unwrap();
            let (px, py) = (s.props[i].x, s.props[i].y);
            let seat = s.seats.get_mut(&1).unwrap();
            seat.x = px - 24 * lw::world::Q;
            seat.y = py + 8 * lw::world::Q;
        }
        let (w, h) = (56, 64);
        let mut strip = Img::black((states.len() as i32) * (w * 3 + 2), h * 3);
        let mut native = Img::black((states.len() as i32) * (w + 2), h);
        for (n, &(on, t, tick, _)) in states.iter().enumerate() {
            let full = draw_prop(&mut r, i, on, t, tick);
            let mut crop = Img::black(w, h);
            for yy in 0..h {
                for xx in 0..w {
                    if let Some(c) = full.get(x - w / 2 + xx, foot - h + 12 + yy) {
                        crop.set(xx, yy, c);
                    }
                }
            }
            native.stamp(&crop, n as i32 * (w + 2), 0);
            let mut big = Img::black(w * 3, h * 3);
            big.stamp_times(&crop, 0, 0, 3);
            strip.stamp(&big, n as i32 * (w * 3 + 2), 0);
        }
        save(&dir, &format!("prop-{name}-1x"), &native);
        save(&dir, &format!("prop-{name}-3x"), &strip);
    }
}

/// Each prop's states standing on every room's own floor, a tile from the
/// first knight (so the dark vault's lantern reaches it), at 3x: where the
/// painted frames are judged against the stone they stand on.
#[test]
#[ignore = "writes the props' floor-legibility strips"]
fn write_lanternward_prop_floors() {
    use lw::Pk;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("lanternward");
    std::fs::create_dir_all(&dir).unwrap();
    // (name, kind, [(on, t, tick)])
    let kinds: [(&str, Pk, &[(u8, u16, u32)]); 5] = [
        ("great", Pk::Great, &[(0, 0, 0), (1, 0, 0), (1, 0, 4)]),
        ("chest", Pk::Chest, &[(0, 0, 0), (1, 0, 0)]),
        ("gate", Pk::Gate, &[(0, 0, 0), (1, 0, 0)]),
        ("crystal", Pk::Crystal, &[(0, 0, 0), (1, 0, 0)]),
        ("beacon", Pk::Beacon, &[(0, 0, 0), (0, 9, 0), (1, 0, 0)]),
    ];
    let rooms = ["stair", "lamp", "gallery", "vault", "warden"];
    for (name, kind, states) in kinds {
        let (w, h) = (64, 56);
        let mut sheet = Img::black(states.len() as i32 * (w * 3 + 2), rooms.len() as i32 * (h * 3 + 2));
        for (row, moment) in rooms.iter().enumerate() {
            let mut r = Run::lanternward_review_for_test(moment, 1);
            let s = r.lanternward.as_mut().unwrap();
            // A room with none of the kind borrows one: a prop to stand on its floor.
            let i = s.props.iter().position(|p| p.kind == kind).unwrap_or_else(|| {
                s.props.push(lw::props::Prop::new(kind, 0, 0, 0));
                s.props.len() - 1
            });
            let seat = s.seats.get(&1).unwrap();
            let (kx, ky) = (seat.x, seat.y);
            let (tx, ty) = (kx.div_euclid(lw::world::Q) / 16 + 1, ky.div_euclid(lw::world::Q) / 16);
            s.props[i].tx = tx as u8;
            s.props[i].ty = ty as u8;
            s.props[i].x = lw::world::middle(tx);
            s.props[i].y = lw::world::middle(ty);
            let (px, py) = (s.props[i].x, s.props[i].y);
            s.seats.get_mut(&1).unwrap().cam = ((kx + px) / 2, py);
            for (col, &(on, t, tick)) in states.iter().enumerate() {
                let full = draw_prop(&mut r, i, on, t, tick);
                let mut crop = Img::black(w, h);
                for yy in 0..h {
                    for xx in 0..w {
                        if let Some(c) = full.get(192 - w / 2 + xx, 112 + 12 - h + yy) {
                            crop.set(xx, yy, c);
                        }
                    }
                }
                let mut big = Img::black(w * 3, h * 3);
                big.stamp_times(&crop, 0, 0, 3);
                sheet.stamp(&big, col as i32 * (w * 3 + 2), row as i32 * (h * 3 + 2));
            }
        }
        save(&dir, &format!("floors-{name}"), &sheet);
    }
}

#[test]
fn a_frame_is_cheap_even_in_the_dark_with_six_knights_and_a_beam_up() {
    for (moment, seats) in [("vault-flare", 6), ("warden-dark", 6), ("warden-slam", 6), ("lamp-wave", 6)] {
        let r = Run::lanternward_review_for_test(moment, seats);
        // Warm any cache, then time ten frames of the native route.
        let _ = layers(&r, Some(1));
        let t = std::time::Instant::now();
        for _ in 0..10 {
            let _ = layers(&r, Some(1));
        }
        let each = t.elapsed() / 10;
        eprintln!("FRAME {moment}: {each:?}");
        assert!(each < std::time::Duration::from_millis(60), "{moment}: {each:?} a frame");
    }
}
