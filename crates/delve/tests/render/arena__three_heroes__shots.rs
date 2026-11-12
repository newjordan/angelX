use super::*;
use crate::rules::{EnemyKind, Input, RoomKind, Run};
use std::collections::BTreeMap;

#[test]
fn three_heroes_original_silhouettes_have_distinct_standing_and_casting_pixels() {
    let mut images = Vec::new();
    for kit in heroes::KITS {
        let mut r = Run::new(21, 1, None);
        assert!(r.take_up(1, kit));
        let h = r.players.get_mut(&1).unwrap();
        h.aim_x = 1.0;
        h.aim_y = 0.0;
        let stand = figure(h).rgba_scaled(1);
        h.hero_kit.cast = 12;
        let cast = figure(h).rgba_scaled(1);
        assert_ne!(stand, cast, "{kit} must show its cast");
        assert!(stand.chunks(4).filter(|px| px[3] > 0).count() > 80);
        assert!(images.iter().all(|old| old != &stand));
        images.push(stand);
    }
}

/// Explicit opt-in PNGs; the normal suite never writes review artifacts.
#[test]
#[ignore = "exports original hero standing/casting PNGs for visual review"]
fn three_heroes_export_pngs() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_HEROES_SHOTS").expect("set DELVE_HEROES_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |name: &str, im: &Img, k: u32| {
        image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    for kit in heroes::KITS {
        let mut r = Run::new(21, 1, None);
        let room = r
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight && !r.great())
            .unwrap();
        r.enter_for_test(room);
        r.calm_for_test();
        r.enemies.clear();
        r.projectiles.clear();
        assert!(r.take_up(1, kit));
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
        h.invulnerable = 1000;
        r.spawn_at_for_test(EnemyKind::Demon, 17.0, 14.0);
        save(&format!("{kit}-standing"), &figure(&r.players[&1]), 8);
        let key = if kit == "cinderheart" { 2 } else { 1 };
        r.step(&BTreeMap::from([(
            1,
            Input {
                cast: key,
                ..Default::default()
            },
        )]));
        save(&format!("{kit}-casting"), &figure(&r.players[&1]), 8);
        // Existing native arena renderer, so the same artifacts prove kit
        // placement, spell tell and the client's scene pipeline too.
        let scene = super::super::seat_scene(
            &r,
            super::super::NATIVE_W,
            super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
            Some(1),
        )
        .compose();
        save(&format!("{kit}-scene-casting"), &scene, 3);
    }
}

/// Each spell of the weave is shown for exactly the frames its strip holds,
/// two ticks a frame, and a spell cast along the aim has a row for each
/// eighth of a turn.
#[test]
fn every_cast_spell_plays_its_whole_strip() {
    use crate::rules::weave::SHOWN;
    let strips = super::super::spellfx::strips();
    for (spell, shown) in SHOWN.iter().enumerate() {
        let name = crate::rules::weave::SPELLS[spell].name;
        match (&strips[spell], *shown) {
            (_, 0) => {}
            (Some(strip), t) => {
                assert_eq!(t, 2 * strip.frames[0].len() as u32, "{name}");
                assert!(matches!(strip.frames.len(), 1 | 8), "{name}");
            }
            (None, _) => panic!("{name} is shown with no strip"),
        }
    }
    assert_eq!(strips[3].as_ref().unwrap().frames.len(), 8, "Scald follows the aim");
    assert_eq!(strips[4].as_ref().unwrap().frames.len(), 8, "Wildfire follows the aim");
}

/// Explicit opt-in PNGs of the Mage's weave: the essences drawn, then every
/// spell at three moments after it is woven (`weave`).
#[test]
#[ignore = "exports the Mage's weave scenes for visual review"]
fn weave_export_pngs() {
    let dir = &std::path::PathBuf::from(std::env::var_os("DELVE_WEAVE_SHOTS").expect("set DELVE_WEAVE_SHOTS to the output directory"));
    std::fs::create_dir_all(dir).unwrap();
    let save = |name: &str, im: &Img, k: u32| {
        image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let scene = |r: &Run| {
        super::super::seat_scene(
            r,
            super::super::NATIVE_W,
            super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
            Some(1),
        )
        .compose()
    };
    let tap = |r: &mut Run, cast: u8| {
        r.step(&BTreeMap::from([(1, Input { cast, ..Default::default() })]));
        r.step(&BTreeMap::new());
    };
    for (name, keys) in [
        ("hoarfrost", [2u8, 2, 2]),
        ("thunderhead", [3, 3, 3]),
        ("sunfall", [1, 1, 1]),
        ("scald", [1, 1, 2]),
        ("wildfire", [1, 1, 3]),
        ("frost-ring", [2, 2, 1]),
        ("sleet", [2, 2, 3]),
        ("ball-lightning", [3, 3, 1]),
        ("static-ward", [3, 3, 2]),
        ("cataclysm", [1, 2, 3]),
    ] {
        let mut r = Run::new(21, 1, None);
        let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
        r.enter_for_test(room);
        r.calm_for_test();
        r.enemies.clear();
        r.projectiles.clear();
        let mage = crate::rules::classes::CLASSES.iter().find(|c| c.name == "Mage").unwrap();
        r.become_class(1, mage);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
        h.invulnerable = 1000;
        for (x, y) in [(17.0, 13.0), (19.0, 15.0), (20.0, 12.5), (9.0, 14.0), (21.0, 14.5)] {
            r.spawn_at_for_test(EnemyKind::Demon, x, y);
        }
        for k in keys {
            tap(&mut r, k);
        }
        if name == "hoarfrost" {
            save("mage-orbs-drawn", &scene(&r), 3);
            // Deeper down: six slots, four spells known, two of them cooling.
            let mut deep = r.clone();
            deep.dungeon.depth = 9;
            let w = &mut deep.players.get_mut(&1).unwrap().weave;
            w.known = vec![0, 1, 2, 7];
            w.cooldowns[0] = crate::rules::weave::SPELLS[0].cooldown / 2;
            w.cooldowns[7] = crate::rules::weave::SPELLS[7].cooldown / 4;
            save("mage-slots-deep", &scene(&deep), 3);
        }
        r.players.get_mut(&1).unwrap().weave.known.clear();
        tap(&mut r, 4);
        // Three moments of the spell: just woven, a third of a second on, and
        // most of a second on.
        let mut gone = 1;
        for (moment, at) in [3u32, 10, 24].into_iter().enumerate() {
            for _ in gone..at {
                r.step(&BTreeMap::new());
            }
            gone = at;
            save(&format!("mage-{name}-{moment}"), &scene(&r), 3);
        }
    }
}

/// Film only (Knights Wish film, 2026-11-04): the Mage's weave a tick a frame,
/// native size, three spells back to back among demons.
/// `KW_WEAVE_FILM=<dir> cargo test kw_weave_film -- --ignored`
#[test]
#[ignore = "film export"]
fn kw_weave_film() {
    let dir = &std::path::PathBuf::from(std::env::var_os("KW_WEAVE_FILM").expect("set KW_WEAVE_FILM"));
    std::fs::create_dir_all(dir).unwrap();
    let mut n = 0u32;
    let mut shoot = |r: &Run| {
        let im = super::super::seat_scene(r, super::super::NATIVE_W, super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H, Some(1)).compose();
        image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1)).unwrap().save(dir.join(format!("{n:05}.png"))).unwrap();
        n += 1;
    };
    let mut r = Run::new(21, 1, None);
    let room = r.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Fight && !r.great()).unwrap();
    r.enter_for_test(room);
    r.calm_for_test();
    r.enemies.clear();
    r.projectiles.clear();
    r.dungeon.depth = 7;
    let mage = crate::rules::classes::CLASSES.iter().find(|c| c.name == "Mage").unwrap();
    r.become_class(1, mage);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (8.0, 14.0, 1.0, 0.0);
    h.invulnerable = 100_000;
    let foes = |r: &mut Run| for (x, y) in [(25.0, 13.0), (27.0, 15.0), (28.0, 12.5), (26.0, 16.5), (29.0, 14.5), (24.0, 11.0)] { r.spawn_at_for_test(EnemyKind::Demon, x, y); };
    foes(&mut r);
    let idle = |r: &mut Run, ticks: u32, shoot: &mut dyn FnMut(&Run)| for _ in 0..ticks { r.step(&BTreeMap::new()); shoot(r); };
    let tap = |r: &mut Run, cast: u8, shoot: &mut dyn FnMut(&Run)| { r.step(&BTreeMap::from([(1, Input { cast, ..Default::default() })])); shoot(r); };
    idle(&mut r, 20, &mut shoot);
    for (keys, hold) in [([1u8, 2, 3], 70u32), ([3, 3, 3], 55), ([1, 1, 1], 60)] {
        for k in keys { tap(&mut r, k, &mut shoot); idle(&mut r, 9, &mut shoot); }
        idle(&mut r, 6, &mut shoot);
        tap(&mut r, 4, &mut shoot);
        idle(&mut r, hold, &mut shoot);
        if r.enemies.len() < 3 { foes(&mut r); }
    }
    idle(&mut r, 20, &mut shoot);
}
