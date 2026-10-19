//! The stubs through the real arena: each draws its own block scene on the
//! native route and the terminal's, one box per seat, and a door on the
//! kingdom map. `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
//! write_roundtable_review -- --ignored` writes the pictures to open.
use super::super::{art_v2, frame_for};
use super::*;
use crate::rules::{MAX_PLAYERS, Run, roundtable::ADVENTURES};
use std::collections::BTreeMap;

fn party(slug: &str, seats: u32) -> Run {
    let mut r = Run::roundtable_for_test(slug).unwrap();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.step(&BTreeMap::new());
    r
}

/// The adventures whose terminal picture is the native scene without its
/// pixel words, because the cockpit prints those words as text rows (four
/// pixels to a cell is too coarse to read them). Everyone else's terminal
/// picture is the native one, exactly.
const TERMINAL_WORDS_AS_ROWS: [&str; 1] = ["bannerpeak"];

/// The window asks the arena for a room in parts (`scene_for`). The
/// adventures stand in the Dark Forest's room, as the Forest Road does, so a
/// room that "comes apart" would draw the forest and never the adventure:
/// each must stay one picture, which is `frame_for`'s.
#[test]
fn an_adventure_and_the_forest_road_stay_one_picture_on_the_parts_route() {
    use super::super::scene_for;
    for a in &ADVENTURES {
        let r = party(a.slug, 2);
        let scene = scene_for(&r, 384, 224, Some(1));
        assert!(scene.is_flat(), "{} is drawn as the forest's room", a.slug);
        assert_eq!(scene.compose(), frame_for(&r, 384, 224, Some(1)), "{}", a.slug);
    }
    let r = Run::forest_road_for_test();
    assert!(scene_for(&r, 384, 224, Some(1)).is_flat(), "the Forest Road is drawn as the forest's room");
}

#[test]
fn each_stub_draws_its_own_scene_on_both_routes() {
    let mut seen = std::collections::BTreeSet::new();
    for a in &ADVENTURES {
        let r = party(a.slug, 4);
        let (base, fx) = layers(&r, Some(1)).expect("the adventure under way draws");
        assert_eq!(
            (base.w, base.h, fx.w, fx.h),
            (384, 224, 384, 224),
            "{}",
            a.slug
        );
        let (cells, _) = terminal_layers(&r, Some(1)).unwrap();
        if TERMINAL_WORDS_AS_ROWS.contains(&a.slug) {
            assert_eq!((cells.w, cells.h), (base.w, base.h), "{}", a.slug);
            assert_ne!(
                cells.content_hash(),
                base.content_hash(),
                "{} leaves its pixel words to the terminal's rows",
                a.slug
            );
        } else {
            assert_eq!(cells.content_hash(), base.content_hash(), "{}", a.slug);
        }
        assert!(
            art_v2::terminal_layers(&r, Some(1)).is_some(),
            "{} reaches the cell painter",
            a.slug
        );
        for (w, h) in [(384, 224), (768, 448), (96, 56)] {
            let img = frame_for(&r, w, h, Some(1));
            assert_eq!((img.w, img.h), (w, h), "{} at {w}x{h}", a.slug);
            assert!(
                img.pixels().any(|p| p != [0, 0, 0]),
                "{} is not black",
                a.slug
            );
        }
        assert!(
            seen.insert(base.content_hash()),
            "{} draws a scene of its own",
            a.slug
        );
    }
}

/// The stub scene itself (`stub_layers`), not any adventure's own: once a
/// builder draws the real game these still hold for the block scene.
const LOOK: Look = Look {
    name: "Test",
    backdrop: 'x',
    edge: 'u',
    accent: 'O',
};

#[test]
fn the_stub_scene_has_one_box_per_seat_and_the_viewers_own_is_lit() {
    let hashes: Vec<_> = (1..=MAX_PLAYERS)
        .map(|n| {
            stub_layers(&party("lanternward", n), Some(1), &LOOK)
                .0
                .content_hash()
        })
        .collect();
    let distinct: std::collections::BTreeSet<_> = hashes.iter().collect();
    assert_eq!(
        distinct.len(),
        MAX_PLAYERS as usize,
        "every added seat adds a box"
    );
    let r = party("lanternward", 2);
    assert_ne!(
        stub_layers(&r, Some(1), &LOOK).0.content_hash(),
        stub_layers(&r, Some(2), &LOOK).0.content_hash(),
        "each seat sees itself lit"
    );
}

#[test]
fn the_stub_scene_drops_a_seat_that_left_and_a_moving_mark_shows_the_sim_ticks() {
    let mut r = party("moonfall", 3);
    let before = stub_layers(&r, Some(1), &LOOK).0.content_hash();
    assert!(r.leave_activity(3));
    r.step(&BTreeMap::new());
    assert_ne!(
        stub_layers(&r, Some(1), &LOOK).0.content_hash(),
        before,
        "the box is gone"
    );
    let a = stub_layers(&r, Some(1), &LOOK).0.content_hash();
    for _ in 0..8 {
        r.step(&BTreeMap::new());
    }
    assert_ne!(
        stub_layers(&r, Some(1), &LOOK).0.content_hash(),
        a,
        "the mark slides"
    );
}

#[test]
fn each_door_is_drawn_at_its_own_tile_only_while_in_view() {
    let r = Run::at_home(41, 1, None, Default::default(), Default::default());
    for a in &ADVENTURES {
        let centred = (a.entry.0 * 16 + 8 - 192, a.entry.1 * 16 + 8 - 112);
        let mut near = Img::new(384, 224);
        kingdom(&mut near, &r, centred);
        assert_ne!(
            near.content_hash(),
            Img::new(384, 224).content_hash(),
            "{} has a door",
            a.slug
        );
        let mut far = Img::new(384, 224);
        kingdom(
            &mut far,
            &r,
            (a.entry.0 * 16 + 8 + 2000, a.entry.1 * 16 + 8 + 2000),
        );
        assert_eq!(
            far.content_hash(),
            Img::new(384, 224).content_hash(),
            "{} is out of view",
            a.slug
        );
    }
}

#[test]
#[ignore = "writes the Round Table stubs' review pictures"]
fn write_roundtable_review() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    for a in &ADVENTURES {
        for seats in [1, 2, 4, MAX_PLAYERS] {
            let r = party(a.slug, seats);
            save(
                &format!("{}-seats{seats}-native", a.slug),
                &frame_for(&r, 384, 224, Some(1)),
            );
        }
        let r = party(a.slug, 4);
        save(
            &format!("{}-seats4-768", a.slug),
            &frame_for(&r, 768, 448, Some(2)),
        );
        let (base, fx) = art_v2::terminal_layers(&r, Some(1)).unwrap();
        let mut both = base.clone();
        both.stamp(&fx, 0, 0);
        save(&format!("{}-terminal-layers", a.slug), &both);
        let mut k = Run::at_home(
            41,
            1,
            Some("Gawain"),
            Default::default(),
            Default::default(),
        );
        k.walk_kingdom(Some((a.entry.0, a.entry.1 + 2)));
        k.follow_kingdom_cameras();
        save(
            &format!("{}-kingdom", a.slug),
            &frame_for(&k, 384, 224, Some(1)),
        );
    }
}
