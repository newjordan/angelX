//! The Gravecaller's dead in what he wished for them, on screen at the
//! seat's own scale: his ghoul, warden and totem in bright pink, a
//! tombstone's zombies the same, a helm wished for him and his dead alike
//! on every head, a close sheet of each body plain and dressed, and his
//! scroll asking whom a wish is for. `ANGEL_ARENA_SHOTS=<dir> cargo test
//! write_dead_wish_shots -- --ignored`.

use super::super::*;
use crate::rules::dead_wishes::Target;
use crate::rules::hireling::AllyKind;
use crate::rules::hireling::testing::{fight_with, foe_at};
use crate::rules::minions::{GRAVECALLER, RISE};
use crate::rules::{EnemyKind, HZ, Input, Phase, Run, phrasebook};
use std::collections::BTreeMap;

/// A colour wished over the dead's whole bodies, as the host's learner
/// writes one.
const BRIGHT_PINK: &str = "wish bright-pink | Bright Pink
say make my ghouls bright pink | pink ghouls | pink dead
tier max_hp 10 :: Your dead turn bright pink, and 10 more health.
draw worn at=body
.7777.
7cccc7
7c99c7
7cccc7
7cccc7
.7777.
";

fn seat() -> (i32, i32) {
    (NATIVE_W, NATIVE_H + party::BAND_H + party::HUD_H)
}

/// The Gravecaller in an open fight room, awake, facing the viewer, with
/// three wishes held.
fn field() -> Run {
    let mut run = fight_with(false, &[]);
    assert!(run.take_up(1, GRAVECALLER));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.idle, hero.stone, hero.invulnerable) = (10.0, 14.0, false, false, 0);
    (hero.aim_x, hero.aim_y) = (0.0, 1.0);
    hero.spell_cooldowns = [0; 4];
    hero.wishes = 3;
    run
}

/// Knight 1's wish for `to`, granted as the host grants a learned one.
fn wished(run: &mut Run, entry: Option<&str>, id: &str, to: Target) {
    let wish = match entry {
        Some(text) => phrasebook::read(text).remove(0),
        None => phrasebook::get(id).unwrap(),
    };
    run.aim_wish(1, &wish, to);
    run.grant(1, &wish).unwrap();
}

fn foe(run: &mut Run, kind: EnemyKind, at: (f32, f32)) -> u32 {
    let id = foe_at(run, kind, at, u32::MAX / 2);
    let foe = run.enemies.last_mut().unwrap();
    (foe.hp, foe.max_hp) = (900, 900);
    run.phase = Phase::Fighting;
    id
}

fn cast(run: &mut Run, slot: u8, aim: (i8, i8)) {
    run.players.get_mut(&1).unwrap().spell_cooldowns = [0; 4];
    run.step(&BTreeMap::from([(1, Input { cast: slot, aim_x: aim.0, aim_y: aim.1, ..Default::default() })]));
}

fn wait(run: &mut Run, ticks: u32) {
    for _ in 0..ticks {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        run.step(&BTreeMap::new());
    }
}

/// His whole company out and at work on a skeleton and a goblin: the
/// totem planted ahead, the ghoul gone after the goblin, and the warden
/// (who keeps so close to his caller that the robe hides him) stood a
/// little clear of him for the picture.
fn company(run: &mut Run) {
    foe(run, EnemyKind::Skeleton, (12.0, 21.0));
    foe(run, EnemyKind::Goblin, (24.0, 12.0));
    cast(run, 2, (1, 0));
    cast(run, 3, (1, 0));
    cast(run, 1, (1, 0));
    wait(run, RISE + 2);
    wait(run, 24);
    let (hx, hy) = (run.players[&1].x, run.players[&1].y);
    if let Some(warden) = run.minions.raised.iter_mut().find(|m| m.kind == AllyKind::Warden) {
        (warden.x, warden.y, warden.facing) = (hx + 1.5, hy + 4.0, 1.0);
    }
}

#[test]
#[ignore]
fn write_dead_wish_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save_img = |name: &str, img: &Img, k: u32| {
        let rgba = img.rgba_scaled(k);
        image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, rgba)
            .unwrap()
            .save(dir.join(format!("{name}.png")))
            .unwrap();
    };
    let (w, h) = seat();
    let shot = |name: &str, run: &Run| {
        let img = seat_scene(run, w, h, Some(1)).compose();
        save_img(name, &img, 3);
    };

    // His company as it was, and in the bright pink wished for them.
    let mut plain = field();
    company(&mut plain);
    shot("dead_wishes_01_company_plain", &plain);
    let mut run = field();
    wished(&mut run, Some(BRIGHT_PINK), "bright-pink", Target::Dead);
    company(&mut run);
    shot("dead_wishes_02_company_pink", &run);

    // A tombstone's zombies come up pink too.
    let mut zombies = run.clone();
    foe(&mut zombies, EnemyKind::Skeleton, (22.0, 22.0));
    zombies.players.get_mut(&1).unwrap().ult_charge = crate::rules::ults::ULT_FULL;
    zombies.step(&BTreeMap::from([(1, Input { ult: true, aim_x: 1, ..Default::default() })]));
    wait(&mut zombies, 4 * HZ);
    shot("dead_wishes_03_zombies_pink", &zombies);

    // A helm wished for him and his dead alike: on his head and on theirs.
    let mut helmed = field();
    wished(&mut helmed, Some(BRIGHT_PINK), "bright-pink", Target::Dead);
    wished(&mut helmed, None, "great-helm", Target::Both);
    company(&mut helmed);
    shot("dead_wishes_04_helm_for_both", &helmed);

    // Close: each of his dead plain, in pink, and in pink with the helm.
    let mut figures: Vec<Img> = Vec::new();
    for kind in [AllyKind::Ghoul, AllyKind::Warden, AllyKind::Totem, AllyKind::Zombie] {
        let body = Img::from_rows(rows_of(kind));
        let mut hire = helmed.minions.raised[0].clone();
        hire.kind = kind;
        figures.push(body.clone());
        figures.push(super::dressed(&run, &hire, &body).0);
        figures.push(super::dressed(&helmed, &hire, &body).0);
    }
    let gap = 4;
    let wide: i32 = figures.iter().map(|f| f.w + gap).sum::<i32>() + gap;
    let tall = figures.iter().map(|f| f.h).max().unwrap() + 2 * gap;
    let mut sheet = Img::new(wide, tall);
    sheet.rect(0, 0, wide, tall, 'k');
    let mut x = gap;
    for f in &figures {
        sheet.stamp(f, x, tall - gap - f.h);
        x += f.w + gap;
    }
    save_img("dead_wishes_05_sheet", &sheet, 6);

    // His scroll asks whom the wish is for: his dead, from his words; and
    // turned to both.
    let scroll = |run: &Run, w: i32, h: i32| {
        in_seat(|| {
            let mut img = frame_for(run, w, view_rows(h), Some(1));
            crate::render::genie::paint(&mut img, run, Some(1));
            let map = MapView { full: false, model: None, party: None };
            seat_lift_frame_with(&img, run, Some(1), &map)
        })
    };
    let mut asking = field();
    asking.offer_wish(1, "make my ghouls bright pink", None).unwrap();
    save_img("dead_wishes_06_scroll_dead", &scroll(&asking, 768, 448), 1);
    asking.turn_wish(1);
    save_img("dead_wishes_07_scroll_both", &scroll(&asking, 768, 448), 1);
    save_img("dead_wishes_08_scroll_384", &scroll(&asking, 384, 224), 2);
}

/// What an ally of `kind` is drawn from (`hireling::rows_of`).
fn rows_of(kind: AllyKind) -> &'static [&'static str] {
    match kind {
        AllyKind::Ghoul => &super::GHOUL[..],
        AllyKind::Warden => &super::WARDEN[..],
        AllyKind::Totem => &super::TOTEM[..],
        _ => &super::ZOMBIE[..],
    }
}
