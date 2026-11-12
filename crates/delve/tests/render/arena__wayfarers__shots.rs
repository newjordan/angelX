//! The travellers on the kingdom's roads on screen: each kind drawn on its
//! own, then on the road with a knight beside it, the talk panels, the
//! gang's rope and fight, and the squire at the party's heels.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test write_wayfarer_shots -- --ignored`.

use super::*;
use crate::render::overworld::kingdom as geography;
use crate::rules::Input;
use crate::rules::wayfarers::{self, Kind, Stage, Wayfarer};
use std::collections::BTreeMap;

fn on_palette(img: &Img) -> bool {
    img.pixels().all(|c| c == BLACK || crate::ink::palette_index(c).is_some())
}

fn kingdom() -> Run {
    let mut r = Run::at_home(41, 1, Some("Friend"), Default::default(), Default::default());
    r.walk_kingdom(Some(geography::keep_stand()));
    r.players.retain(|&id, _| id == 1);
    r
}

/// A stretch of road with no door or place name within sight: which road,
/// and how far along it.
fn quiet() -> (usize, f32) {
    let geo = geography::geography(&[]);
    wayfarers::paths()
        .iter()
        .enumerate()
        .filter(|(_, p)| p.len() > 60.0)
        .find_map(|(i, path)| {
            (0..path.len() as i32 - 30)
                .step_by(2)
                .map(|a| a as f32 + 14.0)
                .find(|&a| {
                    let (x, y) = path.at(a);
                    geo.sites.iter().all(|s| (s.stand.0 as f32 - x).hypot(s.stand.1 as f32 - y) > 17.0)
                })
                .map(|a| (i, a))
        })
        .expect("a quiet stretch of road")
}

/// The knight on the first road at a quiet stretch, a traveller of `kind`
/// three tiles on, both settled and the camera on them.
fn on_the_road(kind: Kind) -> Run {
    let mut r = kingdom();
    let (road, at) = quiet();
    let path = &wayfarers::paths()[road];
    let (kx, ky) = path.at(at);
    let (wx, wy) = path.at(at + 3.2);
    let hero = r.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (kx * TILE_UNITS, ky * TILE_UNITS);
    hero.aim_x = 1.0;
    r.step(&BTreeMap::new());
    r.wayfarers.on = Some(Wayfarer {
        kind,
        name: "Aldous".into(),
        x: wx * TILE_UNITS,
        y: wy * TILE_UNITS,
        east: false,
        road: road as u16,
        along: at + 3.2,
        dir: -1,
        born: r.tick,
        stage: Stage::Walking,
        moved: 0,
        still: 0,
        met: false,
        done: false,
        sold: 0,
        gang: Vec::new(),
    });
    for _ in 0..90 {
        r.step(&BTreeMap::from([(1, Input { aim_x: 1, ..Default::default() })]));
    }
    r.follow_kingdom_cameras();
    r
}

fn press(r: &mut Run) {
    r.step(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    r.step(&BTreeMap::from([(1, Input { aim_x: 1, ..Default::default() })]));
}

fn say(r: &mut Run, pick: u8) {
    r.step(&BTreeMap::from([(1, Input { cast: pick, ..Default::default() })]));
    r.step(&BTreeMap::from([(1, Input { aim_x: 1, ..Default::default() })]));
}

#[test]
fn every_traveller_shows_its_plate_its_key_and_its_talk_on_the_palette() {
    for kind in wayfarers::KINDS {
        let mut r = on_the_road(kind);
        let (frame, said) = crate::ink::said::read(|| frame_for(&r, NATIVE_W, NATIVE_H, Some(1)));
        assert!(on_palette(&frame), "{kind:?} on the road leaves the realm palette");
        let lines: Vec<&str> = said.lines.iter().map(|l| l.text.as_str()).collect();
        assert!(lines.iter().any(|l| l.contains("ALDOUS")), "{kind:?}: no plate in {lines:?}");
        assert!(lines.contains(&"E: TALK"), "{kind:?}: no key in {lines:?}");
        press(&mut r);
        press(&mut r);
        let (frame, said) = crate::ink::said::read(|| frame_for(&r, NATIVE_W, NATIVE_H, Some(1)));
        assert!(on_palette(&frame), "{kind:?} talking leaves the realm palette");
        let lines: Vec<String> = said.lines.iter().map(|l| l.text.clone()).collect();
        let first = r.wayfarer_choices()[0].1.clone();
        assert!(lines.iter().any(|l| *l == first), "{kind:?}: the first answer {first:?} is not on screen: {lines:?}");
        assert!(lines.iter().any(|l| l == "E: WALK ON"));
        assert!(!lines.iter().any(|l| l == "E: TALK"), "the key goes while they talk");
    }
}

#[test]
fn in_3d_the_traveller_stands_up_as_a_figure_beside_the_knight() {
    let r = on_the_road(Kind::Pilgrim);
    let (_, _, figures) = super::kingdom::layers_3d(&r, Some(1));
    assert_eq!(figures.len(), 2, "the knight and the pilgrim");
    // And the gang, when they draw steel, each its own body.
    let mut r = on_the_road(Kind::Tollkeeper);
    press(&mut r);
    press(&mut r);
    say(&mut r, 3);
    r.follow_kingdom_cameras();
    let (_, _, figures) = super::kingdom::layers_3d(&r, Some(1));
    assert_eq!(figures.iter().filter(|f| f.foe).count(), wayfarers::GANG.len());
    assert_eq!(figures.len(), 1 + wayfarers::GANG.len());
}

#[test]
#[ignore]
fn write_wayfarer_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        assert!(on_palette(img), "{name} leaves the realm palette");
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    // Each kind alone on black paper: two strides facing east, then lit
    // as they are when they wait to be spoken to, facing west, and the
    // peddler's cart beside the one who owns it.
    let cell = (26, 24);
    let mut sheet = Img::new(cell.0 * 5 + 40 + 6, cell.1 * 6 + 6);
    for (i, kind) in wayfarers::KINDS.into_iter().enumerate() {
        let y = 3 + i as i32 * cell.1;
        let a = super::kingdom::wayfarers::traveller(kind, 0, false, 0);
        let b = super::kingdom::wayfarers::traveller(kind, 1, false, 0);
        let c = super::kingdom::wayfarers::traveller(kind, 0, true, 0).flip_h();
        sheet.stamp(&a, 3, y);
        sheet.stamp(&b, 3 + cell.0, y);
        sheet.stamp(&c, 3 + cell.0 * 2, y);
    }
    sheet.stamp(&super::kingdom::wayfarers::cart(true), 3 + cell.0 * 3 + 6, 3 + cell.1);
    save("wayfarers-sheet", &sheet, 5);
    for kind in wayfarers::KINDS {
        let mut r = on_the_road(kind);
        let word = kind.word().replace(' ', "-");
        save(&format!("road-{word}"), &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
        press(&mut r);
        save(&format!("talk-{word}-1-greeting"), &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
        press(&mut r);
        save(&format!("talk-{word}-2-offer"), &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    }
    // The reeve's rope across the road, and the gang when they draw steel.
    let mut r = on_the_road(Kind::Tollkeeper);
    press(&mut r);
    press(&mut r);
    say(&mut r, 3);
    for _ in 0..45 {
        r.step(&BTreeMap::from([(1, Input { aim_x: 1, ..Default::default() })]));
    }
    r.follow_kingdom_cameras();
    save("gang-fight", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    // Felled: what they dropped lies where it fell, and the purse is owed.
    for e in &mut r.enemies {
        e.hp = 0;
    }
    for _ in 0..30 {
        r.step(&BTreeMap::new());
    }
    r.follow_kingdom_cameras();
    save("gang-routed", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    // The toll paid: the rope lies slack, and they step aside.
    let mut r = on_the_road(Kind::Tollkeeper);
    r.treasury.add(crate::realm::Spoil::Gold, 200);
    press(&mut r);
    press(&mut r);
    say(&mut r, 1);
    save("gang-paid", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    // The squire at the party's heels, a few tiles on.
    let mut r = on_the_road(Kind::Squire);
    r.home.holdings.owned.push(crate::rules::holdings::Holding {
        key: crate::rules::holdings::Deed::Floor(crate::rules::Pack::Crypt).key(),
        rooms: vec![crate::rules::holdings::Built { id: "taproom".into(), labour: crate::rules::holdings::ROOM_LABOUR }],
        steward: Some("vell".into()),
        ..Default::default()
    });
    press(&mut r);
    press(&mut r);
    say(&mut r, 1);
    press(&mut r);
    let (road, at) = quiet();
    let path = &wayfarers::paths()[road];
    for k in 1..=70 {
        let h = r.players.get_mut(&1).unwrap();
        let (sx, sy) = path.at(at - 9.0 * k as f32 / 70.0);
        (h.x, h.y) = (sx * TILE_UNITS, sy * TILE_UNITS);
        r.step(&BTreeMap::from([(1, Input { move_x: -1, ..Default::default() })]));
    }
    r.follow_kingdom_cameras();
    save("squire-follows", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    // After the blessing, the pilgrim's last words; and a minstrel's song.
    let mut r = on_the_road(Kind::Pilgrim);
    press(&mut r);
    press(&mut r);
    say(&mut r, 1);
    save("pilgrim-blessed", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
    let mut r = on_the_road(Kind::Troubadour);
    r.home.deepest = 4;
    r.home.bestiary.insert("bat".into(), 41);
    r.home.trophies.insert("cinderjaw".into(), 2);
    press(&mut r);
    press(&mut r);
    say(&mut r, 1);
    save("minstrel-sings", &frame_for(&r, NATIVE_W, NATIVE_H, Some(1)), 3);
}
