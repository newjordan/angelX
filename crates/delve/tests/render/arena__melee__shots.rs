use super::*;
use crate::rules::{Input, Phase, RoomKind};
use std::collections::BTreeMap;

fn fight(kit: &str) -> Run {
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
    match kit {
        "hammerer" | "butcher" | "ironbraid" | "galahad" | "peasant" => {
            assert!(r.take_up(1, kit));
        }
        "fists" => {
            let h = r.players.get_mut(&1).unwrap();
            h.weapon = crate::rules::Weapon::Fists;
            h.forged = None;
            h.arm = None;
        }
        "sword" | "spin" => {
            if kit == "spin" {
                assert!(r.take_up(1, "ironbraid"));
            }
        }
        "great-maul" => {
            assert!(r.take_up(1, "hammerer"));
        }
        "cleaver" => {
            assert!(r.take_up(1, "butcher"));
        }
        arm => {
            let card = r.book.get(arm).unwrap();
            let crate::rules::cards::Arm::Forged(w) = card.arm.as_ref().unwrap() else {
                panic!("melee arm")
            };
            let h = r.players.get_mut(&1).unwrap();
            h.arm = Some(arm.into());
            h.forged = Some(w.clone());
        }
    }
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.invulnerable = 1000;
    r.spawn_at_for_test(crate::rules::EnemyKind::Demon, 14.0, 14.0);
    for e in &mut r.enemies {
        (e.hp, e.max_hp, e.seen_hp) = (10000, 10000, 10000);
        e.sleep_until = 100000;
    }
    r.phase = Phase::Fighting;
    r
}

fn crop(r: &Run) -> Img {
    let mut cv = (*scenery_cached(r)).clone();
    cv.stamp(&room_layer(r, None, true), 0, 0);
    let (x, y) = at(12.0, 14.0);
    let mut out = Img::new(96, 80);
    for dy in 0..80 {
        for dx in 0..96 {
            if let Some(c) = cv.get(x - 40 + dx, y - 45 + dy) {
                out.set(dx, dy, c);
            }
        }
    }
    out
}

fn save(dir: &std::path::Path, name: &str, im: &Img) {
    for k in [1, 4] {
        image::RgbaImage::from_raw(im.w as u32 * k, im.h as u32 * k, im.rgba_scaled(k))
            .unwrap()
            .save(dir.join(format!("{name}-{k}x.png")))
            .unwrap();
    }
}

#[test]
#[ignore]
fn write_melee_shots() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    for kit in [
        "sword",
        "knights-blade",
        "round-table-blade",
        "great-maul",
        "cleaver",
        "twin-axes",
        "grave-axes",
        "grave-staff",
        "fists",
        "spin",
    ] {
        let mut r = fight(kit);
        let frames = if kit == "spin" { 32 } else { 10 };
        let mut strip = Img::new(96 * frames, 80);
        for frame in 0..frames {
            if frame == 1 {
                r.step(&BTreeMap::from([(
                    1,
                    Input {
                        swing: kit == "sword",
                        fire: kit != "sword" && kit != "spin",
                        dash: kit == "spin",
                        aim_x: 1,
                        ..Default::default()
                    },
                )]));
                assert!(
                    r.players[&1].swing > 0 || r.players[&1].spin > 0,
                    "{kit} must attack"
                );
            } else if frame > 1 {
                r.step_sim_for_test(&BTreeMap::new());
            }
            strip.stamp(&crop(&r), frame * 96, 0);
        }
        save(&dir, kit, &strip);
    }
}

#[test]
#[ignore]
fn write_melee_impact_shots() {
    use crate::rules::melee::Tier;
    let dir =
        std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut sheet = Img::new(96 * 7, 80 * 4);
    for (row, tier) in [Tier::Light, Tier::Medium, Tier::Heavy, Tier::Blocked]
        .into_iter()
        .enumerate()
    {
        let mut r = fight("sword");
        let foe = r.enemies[0].id;
        r.feel.melee.hit(foe, (14.0, 14.0), (1.0, 0.0), tier, false);
        for col in 0..7 {
            sheet.stamp(&crop(&r), col * 96, row as i32 * 80);
            r.feel.melee.tick();
        }
        sheet.text(
            2,
            row as i32 * 80 + 2,
            match tier {
                Tier::Light => "LIGHT",
                Tier::Medium => "MEDIUM",
                Tier::Heavy => "HEAVY",
                Tier::Blocked => "BLOCKED",
                Tier::Turned => "TURNED",
            },
            'u',
        );
    }
    save(&dir, "spark-tiers", &sheet);
    let mut r = fight("hammerer");
    r.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            cast: 2,
            ..Default::default()
        },
    )]));
    let mut sheet = Img::new(96 * 8, 80);
    for col in 0..8 {
        for _ in 0..3 {
            r.step_sim_for_test(&BTreeMap::new());
        }
        sheet.stamp(&crop(&r), col * 96, 0);
    }
    save(&dir, "groundslam", &sheet);
    let mut r = fight("sword");
    let mut sheet = Img::new(96 * 6, 80 * 3);
    for row in 0..3 {
        r.step_sim_for_test(&BTreeMap::from([(
            1,
            Input {
                swing: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.feel.melee.swings[0].combo, row as u8 + 1);
        for col in 0..6 {
            sheet.stamp(&crop(&r), col * 96, row * 80);
            r.step_sim_for_test(&BTreeMap::new());
        }
        while r.players[&1].sword_cooldown > 1 {
            r.step_sim_for_test(&BTreeMap::new());
        }
    }
    save(&dir, "three-hit-combo", &sheet);
    let mut r = fight("hammerer");
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
        assert!(r.take_up(id, if id % 2 == 0 { "ironbraid" } else { "butcher" }));
        let h = r.players.get_mut(&id).unwrap();
        h.invulnerable = 1000;
    }
    for id in 1..=6 {
        let h = r.players.get_mut(&id).unwrap();
        (h.x, h.y, h.aim_x, h.aim_y) = (
            8.0 + ((id - 1) % 3) as f32 * 6.0,
            9.0 + ((id - 1) / 3) as f32 * 7.0,
            1.0,
            0.0,
        );
    }
    let inputs = (1..=6)
        .map(|id| {
            (
                id,
                Input {
                    fire: true,
                    aim_x: 1,
                    ..Default::default()
                },
            )
        })
        .collect();
    r.step_sim_for_test(&inputs);
    assert_eq!(r.feel.melee.swings.len(), 6);
    r.step_sim_for_test(&BTreeMap::new());
    let mut cv = (*scenery_cached(&r)).clone();
    cv.stamp(&room_layer(&r, None, true), 0, 0);
    save(&dir, "six-knight-brawl", &cv);
}

#[test]
fn melee_native_picture_is_identical_after_wire_restore() {
    let mut r = fight("great-maul");
    r.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    for _ in 0..9 {
        let mirror = crate::wire::read(&crate::wire::publish(&r).unwrap()).unwrap();
        assert_eq!(crop(&r).rgba_scaled(1), crop(&mirror).rgba_scaled(1));
        r.step_sim_for_test(&BTreeMap::new());
    }
}

#[test]
#[ignore]
fn write_melee_special_shots() {
    let dir =
        std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, kit, key) in [
        ("storm-hammer", "hammerer", 1),
        ("groundslam-detail", "hammerer", 2),
        ("meat-hook", "butcher", 1),
        ("rot", "butcher", 2),
        ("chain-haul", "butcher", 3),
        ("earthsplitter", "hammerer", 0),
        ("dismember", "butcher", 0),
        ("bladewind", "galahad", 0),
        ("axe-storm", "ironbraid", 0),
        ("morningstar", "sword", 4),
    ] {
        let mut r = fight(kit);
        if key == 0 {
            r.players.get_mut(&1).unwrap().ult_charge = crate::rules::ults::ULT_FULL;
        }
        if key == 4 {
            r.players.get_mut(&1).unwrap().bonus.orbit = 1;
        }
        let mut sheet = Img::new(96 * 12, 80);
        sheet.stamp(&crop(&r), 0, 0);
        r.step_sim_for_test(&BTreeMap::from([(
            1,
            Input {
                cast: if key < 4 { key } else { 0 },
                ult: key == 0,
                aim_x: 1,
                ..Default::default()
            },
        )]));
        for col in 1..12 {
            sheet.stamp(&crop(&r), col * 96, 0);
            for _ in 0..3 {
                r.step_sim_for_test(&BTreeMap::new());
            }
        }
        save(&dir, name, &sheet);
    }
}
