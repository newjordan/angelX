use super::*;
use crate::rules::hireling::testing::{fight_with, foe_at};
use crate::rules::minions::{GRAVECALLER, Grave, Pulse};
use crate::rules::{EnemyKind, Input, Phase};
use std::collections::BTreeMap;

fn save(name: &str, img: &Img, scale: u32) {
    if let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbaImage::from_raw(img.w as u32 * scale, img.h as u32 * scale, img.rgba_scaled(scale))
            .unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    }
}

#[test]
fn gravecaller_golems_grow_and_the_thriller_dance_loops_in_sync() {
    let tiers = [AllyKind::FleshGolem, AllyKind::BoneGolem, AllyKind::GraveGolem, AllyKind::Colossus];
    let mut sheet = Img::black(256, 96);
    let mut last = (0, 0);
    for (i, tier) in tiers.into_iter().enumerate() {
        let im = body(tier, 0).unwrap();
        assert!(im.w > last.0 && im.h > last.1);
        last = (im.w, im.h);
        stand(&mut sheet, &im, 28 + i as i32 * 64, 42);
        sheet.text(3 + i as i32 * 64, 46, ["FLESH", "BONE", "GRAVE", "COLOSSUS"][i], 'i');
    }
    let first = body(AllyKind::Skeleton, 0).unwrap().rgba_scaled(1);
    assert_eq!(first, body(AllyKind::Skeleton, 48).unwrap().rgba_scaled(1));
    assert_ne!(first, body(AllyKind::Skeleton, 6).unwrap().rgba_scaled(1));
    for i in 0..8 {
        let im = body(AllyKind::Skeleton, i * 6).unwrap();
        stand(&mut sheet, &im, 16 + i as i32 * 32, 88);
        assert!(im.rgba_scaled(1).iter().any(|b| *b > 0));
    }
    save("gravecaller_golems_and_dance", &sheet, 4);
}

#[test]
fn gravecaller_wall_aura_and_corpse_bar_render_in_a_native_seat() {
    let mut run = fight_with(false, &[]);
    assert!(run.take_up(1, GRAVECALLER));
    run.floor_plates.clear();
    run.dare = None;
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.aim_x, hero.aim_y) = (20.0, 24.0, 0.0, -1.0);
    (hero.stone, hero.idle) = (false, false);
    foe_at(&mut run, EnemyKind::Skeleton, (40.0, 12.0), u32::MAX / 2);
    run.phase = Phase::Fighting;
    run.minions.graves.insert(1, Grave { corpses: 100, ..Default::default() });
    run.step(&BTreeMap::from([(1, Input { dash: true, aim_y: -1, ..Default::default() })]));
    for _ in 0..crate::rules::minions::GOLEM_HOLD {
        run.step(&BTreeMap::from([(1, Input { vigil: true, ..Default::default() })]));
    }
    for minion in &mut run.minions.raised { minion.rising = 0; }
    run.found = None;
    let hero = &run.players[&1];
    let grave = run.minions.graves.get_mut(&1).unwrap();
    grave.corpses = 72;
    grave.pulse = Some(Pulse { tick: run.tick - 12, x: hero.x, y: hero.y });
    let (w, h) = super::super::party::seat_size();
    let scene = super::super::seat_scene(&run, w as i32, h as i32, Some(1)).compose();
    save("gravecaller_wall_aura_hud", &scene, 3);
    let hud = super::super::party::hud_band(&run, Some(1));
    let mut empty = run.clone();
    empty.players.get_mut(&1).unwrap().knight = Some("percival".into());
    let other = super::super::party::hud_band(&empty, Some(1));
    assert_ne!(hud.rgba_scaled(1), other.rgba_scaled(1), "the corpse gauge is visible on the HUD");
    assert_eq!(run.minions.raised.iter().filter(|m| m.kind == AllyKind::Skeleton).count(), 5);
    assert!(run.minions.raised.iter().any(|m| m.kind == AllyKind::Colossus));
}
