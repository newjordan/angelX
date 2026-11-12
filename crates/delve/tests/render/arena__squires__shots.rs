//! Reproducible evidence, no live host, timing loop, GPU or external assets.
//! Driver: `cargo test write_squire_shots -- --ignored --nocapture`.
//! Writes to `SQUIRE_SHOTS` (required).
//! This module is nested under arena::squires, beside the authored sprites.

use super::*;
use crate::realm::{Spoil, Spoils};
use crate::rules::hireling::{AllyKind, Hireling};
use crate::rules::home::Home;
use crate::rules::squires::{Companion, Company, TYPES};
use crate::rules::{EnemyKind, Phase, TILE_UNITS, Tile};
use std::path::{Path, PathBuf};

fn recruit(kind: Kind, id: u32, level: u8) -> Squire {
    let name = match kind {
        Kind::Ninja => "Ash",
        Kind::Healer => "Pip",
        Kind::Ranger => "Rowan",
        Kind::Shield => "Kit",
        Kind::Alchemist => "Briar",
    };
    Squire {
        id,
        kind,
        name: name.into(),
        level,
    }
}

fn yard() -> Run {
    let mut home = Home::default();
    home.squires.board = 2; // all five preview recruits, at their rule level
    home.squires.hall = 2;
    home.squires.training = 2;
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 1240);
    let mut run = Run::at_home(41, 1, Some("Companion"), home, treasury);
    run.players.retain(|&id, _| id == 1);
    assert!(run.take_up(1, "percival"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Yard)
        .expect("the home has its Training Yard");
    run.enter_for_test(room);
    run.floor_plates.clear();
    run.tick = 112;
    run
}

fn company(run: &mut Run) {
    let key = knight_key(&run.players[&1]);
    let roster = TYPES
        .into_iter()
        .enumerate()
        .map(|(i, kind)| recruit(kind, 100 + i as u32, if i == 0 { 2 } else { 3 }))
        .collect::<Vec<_>>();
    run.home.squires.companies.insert(
        key,
        Company {
            roster,
            active: Some(100),
        },
    );
}

fn reader(run: &mut Run, action: Action) {
    let (_, (c, r, w, h)) = squires::plates()
        .into_iter()
        .find(|(a, _)| *a == action)
        .expect("a rule plate per action");
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 * 0.5) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 * 0.5) * TILE_UNITS;
    hero.buying = BUY_HOLD / 2;
    assert_eq!(squires::action_at(hero.x, hero.y), Some(action));
}

fn whole(run: &Run, focus: Option<u32>) -> Img {
    let mut cv = (*super::super::scenery_cached(run)).clone();
    cv.stamp(&super::super::room_layer(run, focus, true), 0, 0);
    cv
}

fn seat(run: &Run, focus: Option<u32>) -> Img {
    let width = super::super::NATIVE_W;
    let height = super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H;
    super::super::seat_scene(run, width, height, focus).compose()
}

fn save(dir: &Path, name: &str, im: &Img, scale: u32) {
    image::RgbaImage::from_raw(
        im.w as u32 * scale,
        im.h as u32 * scale,
        im.rgba_scaled(scale),
    )
    .expect("RGBA image dimensions")
    .save(dir.join(format!("{name}.png")))
    .unwrap_or_else(|e| panic!("writing {name}: {e}"));
}

/// Six real knight kits, six owned squires, plus one deliberately felled
/// shield-bearer. No game stepping: the evidence isn't dependent on AI RNG.
fn dungeon_company() -> Run {
    let mut run = crate::rules::hireling::testing::fight_with(false, &[]);
    for id in 2..=6 {
        run.join_seat(id, &format!("Knight {id}"));
    }
    // The first six catalogue entries are actual knight kits, not placeholders.
    for (i, kit) in crate::rules::knights::COMPANY.iter().take(6).enumerate() {
        assert!(run.take_up(i as u32 + 1, kit.id));
    }
    let room = run.at;
    for row in 1..run.room().rows - 1 {
        for col in 1..run.room().cols - 1 {
            run.dungeon.rooms[room].set(col, row, Tile::Floor);
        }
    }
    run.enemies.clear();
    run.projectiles.clear();
    run.minions = Default::default();
    run.hireling = None;
    run.squires.clear();
    run.floor_plates.clear();
    run.tick = 112;
    run.phase = Phase::Fighting;
    // Two staggered ranks keep every knight and every companion visible.
    for id in 1..=6 {
        let column = (id - 1) % 3;
        let row = (id - 1) / 3;
        let (x, y) = (13.0 + column as f32 * 10.0, 11.0 + row as f32 * 9.0);
        let hero = run.players.get_mut(&id).unwrap();
        (hero.x, hero.y, hero.aim_x, hero.aim_y) = (x, y, 1.0, 0.0);
        hero.invulnerable = 0;
        let kind = TYPES[(id as usize - 1) % TYPES.len()];
        let squire = recruit(kind, id + 100, 3);
        let hp = squire.health();
        // Tests construct the public data, without needing a rules-only
        // constructor or inventing a rendering hook on the gameplay side.
        let body = Hireling {
            kind: AllyKind::Beaumains,
            x: x - 2.8,
            y: y + 2.0,
            hp: if id == 4 {
                0
            } else if id == 5 {
                hp / 2
            } else {
                hp
            },
            max_hp: hp,
            invulnerable: 0,
            cooldown: 12,
            facing: 1.0,
            side: 1.0,
            summoner: Some(id),
            power: squire.power(),
            taunt: 0,
            rising: 0,
            made: None,
        };
        let key = knight_key(hero);
        run.home.squires.companies.insert(
            key,
            Company {
                roster: vec![squire.clone()],
                active: Some(squire.id),
            },
        );
        run.squires.push(Companion {
            owner: id,
            squire,
            body,
            smoke: if id == 6 { 18 } else { 0 },
            vigil: 0,
            rescued: false,
        });
    }
    // A few still foes give the frame a clear dungeon context without
    // obscuring the company, which is the subject of this evidence.
    for (kind, x, y) in [
        (EnemyKind::Skeleton, 40.0, 10.0),
        (EnemyKind::Demon, 40.0, 20.0),
    ] {
        crate::rules::hireling::testing::foe_at(&mut run, kind, (x, y), 900);
    }
    run
}

fn sprite_sheet() -> Img {
    let mut im = Img::black(440, 332);
    im.frame(3, 3, 434, 326, 'g');
    im.frame(5, 5, 430, 322, 'X');
    im.text(14, 13, "SQUIRES / FIVE AUTHORED TYPES", 'T');
    im.text(14, 26, "SIX OWNER CLOTHS / DOWNED BELOW", 'h');
    im.line(14, 38, 425, 38, 'G');
    for (i, kind) in TYPES.into_iter().enumerate() {
        let x = 65 + i as i32 * 76;
        let label = short_kind(kind).to_uppercase();
        im.text(x - text_width(&label) / 2, 46, &label, 'i');
    }
    for id in 1..=6 {
        let y = 62 + (id as i32 - 1) * 33;
        im.text(13, y + 6, &format!("P{id}"), knights::seat_cloth(id));
        for (i, kind) in TYPES.into_iter().enumerate() {
            let x = 65 + i as i32 * 76;
            let sprite = sprite(kind, id, 112, true, false);
            im.stamp(&sprite, x - sprite.w / 2, y);
            im.line(x - 12, y + 22, x + 12, y + 22, 'X');
        }
    }
    im.line(14, 261, 425, 261, 'G');
    im.text(14, 270, "DOWNED / EQUIPMENT AND CLOTH KEPT", 'h');
    for (i, kind) in TYPES.into_iter().enumerate() {
        let x = 65 + i as i32 * 76;
        let fallen = sprite(kind, i as u32 + 1, 112, false, true);
        im.stamp(&fallen, x - fallen.w / 2, 291);
    }
    im.text(14, 313, "NO GLOW / NATIVE PIXELS", 'G');
    im
}

#[test]
fn squire_art_has_five_distinct_silhouettes_and_six_cloths() {
    for (i, kind) in TYPES.into_iter().enumerate() {
        let first = sprite(kind, 1, 0, false, false);
        assert!(first.w <= 24 && first.h <= 24);
        assert!(
            first
                .rgba_scaled(1)
                .chunks_exact(4)
                .filter(|p| p[3] != 0)
                .count()
                > 70
        );
        for other in TYPES.into_iter().skip(i + 1) {
            assert_ne!(
                first.rgba_scaled(1),
                sprite(other, 1, 0, false, false).rgba_scaled(1),
                "{kind:?} != {other:?}"
            );
        }
        for owner in 2..=6 {
            assert_ne!(
                first.rgba_scaled(1),
                sprite(kind, owner, 0, false, false).rgba_scaled(1),
                "owner cloth {owner}"
            );
        }
        let down = sprite(kind, 1, 0, false, true);
        assert!(
            down.h < first.h && down.w >= first.w,
            "low, authored downed pose"
        );
    }
}

#[test]
fn squire_scroll_is_black_paper_and_keeps_its_bounds() {
    let lines = vec![
        ("ASH / NINJA / LEVEL 3".into(), 'i'),
        ("BACKSTAB / VANISH".into(), 'T'),
    ];
    let scroll = scroll(&lines, "HOLD E: TRAIN", BUY_HOLD / 2, true);
    assert_eq!(scroll.get(12, 22), Some([0, 0, 0]));
    assert!(scroll.w < super::super::NATIVE_W && scroll.h < super::super::NATIVE_H);
}

#[test]
fn squire_board_respects_focus_and_rendering_does_not_spend() {
    let mut run = yard();
    company(&mut run);
    reader(&mut run, Action::Train(0));
    run.join_seat(2, "Other");
    let other = run.players.get_mut(&2).unwrap();
    (other.x, other.y) = (4.0, 13.0); // no squire plate
    let original_yard = run.home.squires.clone();
    let original_gold = run.treasury.get(Spoil::Gold);
    let mut mine = Img::new(384, 224);
    boards(&mut mine, &run, Some(1));
    let mut theirs = Img::new(384, 224);
    boards(&mut theirs, &run, Some(2));
    assert_ne!(mine.rgba_scaled(1), theirs.rgba_scaled(1));
    assert_eq!(theirs.rgba_scaled(1), Img::new(384, 224).rgba_scaled(1));
    assert_eq!(run.home.squires, original_yard);
    assert_eq!(run.treasury.get(Spoil::Gold), original_gold);
}

#[test]
#[ignore = "PNG evidence exporter; run explicitly after driver build verification"]
fn write_squire_shots() {
    let dir = std::env::var_os("SQUIRE_SHOTS")
        .map(PathBuf::from)
        .expect("set SQUIRE_SHOTS to the output directory");
    std::fs::create_dir_all(&dir).expect("create squire shot directory");
    let mut run = yard();
    run.home.squires.board = 0;
    run.home.squires.hall = 0;
    run.home.squires.training = 0;
    reader(&mut run, Action::Hire(0));
    save(&dir, "squires-00-first-visit", &seat(&run, Some(1)), 3);
    run.home.squires.board = 2;
    run.home.squires.hall = 2;
    run.home.squires.training = 2;
    {
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (25.0, 16.0); // away from all boards
    }
    save(&dir, "squires-01-yard-recruits", &whole(&run, Some(1)), 3);
    reader(&mut run, Action::Hire(0));
    save(&dir, "squires-02-offer-wage", &seat(&run, Some(1)), 3);
    for offer in 1..5 {
        reader(&mut run, Action::Hire(offer));
        save(
            &dir,
            &format!("squires-02-offer-{offer}"),
            &whole(&run, Some(1)),
            3,
        );
    }
    company(&mut run);
    reader(&mut run, Action::Select(0));
    save(&dir, "squires-03-roster-active", &seat(&run, Some(1)), 3);
    reader(&mut run, Action::Train(0));
    save(&dir, "squires-04-train-ability", &seat(&run, Some(1)), 3);
    for building in 0..3 {
        reader(&mut run, Action::Upgrade(building));
        save(
            &dir,
            &format!("squires-05-upgrade-{building}"),
            &seat(&run, Some(1)),
            3,
        );
    }
    reader(&mut run, Action::Train(4));
    run.treasury = Spoils::default();
    save(
        &dir,
        "squires-05-insufficient-gold",
        &seat(&run, Some(1)),
        3,
    );
    reader(&mut run, Action::Rest);
    save(&dir, "squires-06-rest-company", &whole(&run, Some(1)), 3);
    let fight = dungeon_company();
    save(
        &dir,
        "squires-07-six-knight-company",
        &whole(&fight, None),
        3,
    );
    save(&dir, "squires-08-dungeon-seat", &seat(&fight, Some(1)), 3);
    save(&dir, "squires-09-five-types-six-cloths", &sprite_sheet(), 3);
    println!("Squire PNG evidence: {}", dir.display());
}
