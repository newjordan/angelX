//! Repeatable ranged art review; the same scene before and after the ink pass.
use super::*;

fn shot(kind: Shot, x: f32, y: f32, hostile: bool) -> Projectile {
    serde_json::from_value(serde_json::json!({
        "x":x,"y":y,"vx":40.0,"vy":0.0,"hostile":hostile,
        "kind":kind,"damage":10,"pierce":0,"ttl":35,
        "traits":{"bounce":0,"homing":0,"chain":0,"mend":0,"owner":1,"arm":!hostile,"beat":18}
    }))
    .unwrap()
}

fn mark(run: &mut Run, p: &crate::rules::Projectile, mark: super::events::Mark) {
    use crate::rules::feel::{IMPACT_TICKS, Impact};
    run.feel.ranged.push(crate::rules::ranged::Event {
        x: p.x,
        y: p.y,
        dx: 1.0,
        dy: 0.0,
        kind: p.kind,
        hostile: p.hostile,
        owner: p.traits.owner,
        radius: crate::rules::patterns::girth(p),
        mark,
        ttl: crate::rules::ranged::LIFE - 1,
        spell: p.traits.spell,
    });
    match mark {
        super::events::Mark::Release => {
            if let Some(hero) = run.players.get_mut(&u32::from(p.traits.owner)) {
                hero.x = p.x;
                hero.y = p.y;
                hero.aim_x = 1.0;
                hero.aim_y = 0.0;
                hero.fire_cooldown = u32::from(p.traits.beat.max(20));
            }
            let mut p = p.clone();
            p.traits.beat = p.traits.beat.max(20);
            run.projectiles.push(p);
        }
        m => run.feel.impacts.push(Impact {
            note: false,
            x: p.x,
            y: p.y,
            kind: p.kind,
            dx: if m == super::events::Mark::Pop {
                0.0
            } else {
                1.0
            },
            dy: 0.0,
            ttl: IMPACT_TICKS - 1,
            boss_art: None,
        }),
    }
}

#[test]
#[ignore]
fn write_ranged_shots() {
    let Some(dir) = std::env::var_os("ANGEL_RANGED_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, run: &Run| {
        let original = std::env::var_os("ANGEL_RANGED_ORIGINAL").is_some();
        super::ranged::before(original);
        let im = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
        for scale in [1, 4] {
            image::RgbaImage::from_raw(
                (im.w * scale) as u32,
                (im.h * scale) as u32,
                im.rgba_scaled(scale as u32),
            )
            .unwrap()
            .save(dir.join(format!("{name}-{scale}x.png")))
            .unwrap();
        }
    };
    let kinds = [
        Shot::Arrow,
        Shot::Bolt,
        Shot::Ball,
        Shot::Orb,
        Shot::Sacred,
        Shot::Ember,
        Shot::Frost,
        Shot::Hex,
        Shot::Missile,
        Shot::Bone,
        Shot::Blade,
        Shot::Wisp,
        Shot::Ore,
        Shot::Flame,
        Shot::Page,
        Shot::Spore,
        Shot::Eye,
    ];
    for pack in [Pack::Crypt, Pack::Cavern] {
        let mut run = Run::new(21, 1, Some("Friend"));
        run.begin_in(pack);
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight)
            .unwrap();
        run.enter_for_test(at);
        run.calm_for_test();
        run.phase = crate::rules::Phase::Fighting;
        run.enemies.clear();
        run.projectiles.clear();
        run.players.get_mut(&1).unwrap().x = 4.0;
        run.players.get_mut(&1).unwrap().y = 6.0;
        run.players.get_mut(&1).unwrap().weapon = crate::rules::Weapon::Bow;
        let label = format!("{pack:?}").to_lowercase();
        for phase in [0, 2, 5, 10] {
            run.projectiles.clear();
            run.feel.puffs.clear();
            run.tick = 100 + phase;
            for (i, kind) in kinds.iter().copied().enumerate() {
                let x = 6.0 + (i % 6) as f32 * 3.5 + phase as f32 * 0.12;
                let y = 5.0 + (i / 6) as f32 * 4.0;
                let mut p = shot(kind, x, y, false);
                p.traits.path.age = phase as u16;
                if kind == Shot::Sacred {
                    p.empowered = true;
                    p.traits.size = 2;
                }
                run.projectiles.push(p);
            }
            save(&format!("{label}-weapons-{phase:02}"), &run);
        }
        // Close-range release, wall, flesh, shield, and charged-shot beats.
        for phase in [0u8, 2, 5, 10, 17] {
            run.projectiles.clear();
            run.feel = Default::default();
            for (i, kind) in [Shot::Arrow, Shot::Bolt, Shot::Ball, Shot::Orb]
                .into_iter()
                .enumerate()
            {
                let x = 7.0 + i as f32 * 4.0;
                let p = shot(kind, x, 5.0, false);
                mark(&mut run, &p, super::events::Mark::Release);
                run.feel.puffs.push(crate::rules::feel::Puff {
                    x,
                    y: 5.0,
                    dx: 1.0,
                    dy: 0.0,
                    smoke: false,
                    ttl: crate::rules::feel::FLASH_TICKS.saturating_sub(phase),
                    dust: 0,
                });
                for (y, event) in [
                    (9.0, super::events::Mark::Wall),
                    (13.0, super::events::Mark::Flesh),
                    (17.0, super::events::Mark::Pop),
                ] {
                    let mut p = p.clone();
                    p.y = y;
                    if event == super::events::Mark::Wall {
                        run.feel.stuck.push(crate::rules::feel::Stuck {
                            x: p.x,
                            y: p.y,
                            dx: 1.0,
                            dy: 0.0,
                            kind,
                        });
                    }
                    run.feel.impacts.push(crate::rules::feel::Impact {
                        note: false,
                        x,
                        y,
                        kind,
                        ttl: crate::rules::feel::IMPACT_TICKS.saturating_sub(phase),
                        dx: 1.0,
                        dy: 0.0,
                        boss_art: None,
                    });
                }
                run.feel.blocks.push(crate::rules::feel::Block {
                    x,
                    y: 20.0,
                    dx: 1.0,
                    dy: 0.0,
                    nx: -1.0,
                    ny: 0.0,
                    kind,
                    weight: if i == 2 { 3 } else { 1 },
                    knight: 1,
                    ttl: crate::rules::feel::BLOCK_TICKS.saturating_sub(phase),
                });
            }
            for e in &mut run.feel.ranged {
                e.ttl = (crate::rules::ranged::LIFE - 1).saturating_sub(phase);
            }
            for hero in run.players.values_mut() {
                hero.fire_cooldown = hero.fire_cooldown.saturating_sub(u32::from(phase));
            }
            run.feel.puffs.retain(|p| p.ttl > 0);
            run.feel.impacts.retain(|e| e.ttl > 0);
            run.feel.blocks.retain(|e| e.ttl > 0);
            run.players.get_mut(&1).unwrap().hero_kit.charging = true;
            run.players.get_mut(&1).unwrap().hero_kit.charge = 30;
            let mut crit = shot(Shot::Bolt, 20.0, 6.5, false);
            crit.empowered = true;
            crit.traits.size = 2;
            run.projectiles.push(crit);
            save(&format!("{label}-effects-{phase:02}"), &run);
        }
        run.players.get_mut(&1).unwrap().hero_kit.charging = false;
        run.feel = Default::default();

        run.projectiles.clear();
        for i in 0..150 {
            let angle = (i % 15) as f32 / 15.0 * std::f32::consts::TAU;
            let mut p = shot(
                kinds[11 + i % 6],
                4.0 + (i % 15) as f32 * 1.4,
                3.0 + (i / 15) as f32 * 1.15,
                true,
            );
            p.vx = angle.cos() * 12.0;
            p.vy = angle.sin() * 12.0;
            p.traits.path.age = 8;
            run.projectiles.push(p);
        }
        run.tune.foe_shots = 50;
        run.tune.foe_shot_speed = 1.5;
        save(&format!("{label}-dense"), &run);
        run.projectiles.clear();
        for id in 2..=6 {
            run.join_seat(id, "Knight");
        }
        for (i, (_, h)) in run.players.iter_mut().enumerate() {
            h.x = 5.0 + i as f32 * 3.0;
            h.y = 10.0;
            h.weapon = [
                crate::rules::Weapon::Bow,
                crate::rules::Weapon::Crossbow,
                crate::rules::Weapon::Handgonne,
            ][i % 3];
        }
        for i in 0..6 {
            for k in 0..5 {
                let mut p = shot(
                    kinds[i % 4],
                    5.0 + i as f32 * 3.0,
                    6.0 + k as f32 * 0.45,
                    false,
                );
                p.vx = 12.0 * (k as f32 - 2.0);
                p.vy = -34.0;
                run.projectiles.push(p);
            }
        }
        save(&format!("{label}-six-knights"), &run);
    }
}

#[test]
fn flight_ink_has_no_per_shot_allocations_and_foe_disc_matches_girth() {
    let run = Run::new(11, 1, Some("Knight"));
    let mut cv = Img::new(100, 100);
    let mut p = shot(Shot::Arrow, 5.0, 5.0, false);
    let before = crate::test_alloc::now().2;
    for _ in 0..200 {
        assert!(super::ranged::flight(&mut cv, &run, &p));
    }
    assert_eq!(
        crate::test_alloc::now().2,
        before,
        "direct ink has no heap work"
    );
    p.hostile = true;
    for form in [
        crate::rules::patterns::Form::Plain,
        crate::rules::patterns::Form::Dart,
        crate::rules::patterns::Form::Orb,
    ] {
        p.traits.path.form = form;
        cv.clear_all();
        let before = crate::test_alloc::now().2;
        assert!(super::ranged::flight(&mut cv, &run, &p));
        assert_eq!(crate::test_alloc::now().2, before);
        let (x, y) = at(p.x, p.y);
        let radius = (crate::rules::patterns::girth(&p) * UNIT).max(0.5);
        let r = (radius + 0.5).floor() as i32;
        assert_eq!(cv.get(x, y), super::super::super::ink::ink('6'));
        assert!(cv.get(x - r, y).is_some(), "disc reaches its collision rim");
        assert!(
            cv.get(x - r - 1, y).is_none(),
            "nothing solid outside rasterized hit disc"
        );
    }
}

#[test]
fn shot_profiles_separate_weapons_and_recoil_returns_to_rest() {
    use super::ranged::{Body, look};
    assert_eq!(look(Shot::Arrow).body, Body::Arrow);
    assert_eq!(look(Shot::Bolt).body, Body::Bolt);
    assert!(look(Shot::Arrow).length > look(Shot::Bolt).length);
    assert!(look(Shot::Bolt).width > look(Shot::Arrow).width);
    let mut run = Run::new(11, 1, Some("Knight"));
    run.players.get_mut(&1).unwrap().weapon = crate::rules::Weapon::Handgonne;
    mark(
        &mut run,
        &shot(Shot::Ball, 5.0, 5.0, false),
        super::events::Mark::Release,
    );
    assert!(super::recoil(&run, 1, &run.players[&1]) > 0.0);
    for _ in 0..4 {
        run.tick_feel(&[], &[]);
    }
    assert_eq!(super::recoil(&run, 1, &run.players[&1]), 0.0);
}

#[test]
#[ignore]
fn write_ranged_closeups() {
    let Some(dir) = std::env::var_os("ANGEL_RANGED_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let names = [
        "ARROW",
        "BOLT",
        "BALL",
        "ORB",
        "SACRED",
        "EMBER",
        "FROST",
        "HEX",
        "MISSILE",
        "BONE",
        "BLADE",
        "WISP",
        "ORE",
        "FLAME",
        "PAGE",
        "SPORE",
        "EYE",
        "FARWATCH",
        "GALEWEAVER",
        "CINDERHEART",
        "ARCANE",
        "HOLY",
    ];
    let kinds = [
        Shot::Arrow,
        Shot::Bolt,
        Shot::Ball,
        Shot::Orb,
        Shot::Sacred,
        Shot::Ember,
        Shot::Frost,
        Shot::Hex,
        Shot::Missile,
        Shot::Bone,
        Shot::Blade,
        Shot::Wisp,
        Shot::Ore,
        Shot::Flame,
        Shot::Page,
        Shot::Spore,
        Shot::Eye,
        Shot::Bolt,
        Shot::Arrow,
        Shot::Orb,
        Shot::Orb,
        Shot::Sacred,
    ];
    super::ranged::before(std::env::var_os("ANGEL_RANGED_ORIGINAL").is_some());
    let phases = [0u8, 2, 5, 10, 17];
    let mut cv = Img::new(680, 22 * 40 + 55);
    cv.rect(0, 0, cv.w, cv.h, 'K');
    cv.text(
        8,
        8,
        "RANGED PARTS / FLIGHT + RELEASE + WALL + FLESH + SHIELD",
        'o',
    );
    for (i, kind) in kinds.into_iter().enumerate() {
        let row = 35 + i as i32 * 40;
        cv.text(4, row + 15, names[i], 'H');
        for (column, phase) in phases.into_iter().enumerate() {
            let origin = (130 + column as i32 * 108, row + 20);
            let mut panel = Img::new(108, 40);
            panel.rect(0, 0, 108, 40, if i % 2 == 0 { 'X' } else { 'g' });
            let mut run = Run::new(11, 1, Some("Knight"));
            run.tick = 100 + u64::from(phase);
            let mut p = shot(kind, 2.0, 2.5, false);
            p.empowered = i == 17;
            if i == 19 {
                p.look = Some(("ember".into(), "bolt".into()));
            }
            if i == 20 {
                p.look = Some(("arcane".into(), "bolt".into()));
            }
            if i == 21 {
                p.look = Some(("holy".into(), "bolt".into()));
            }
            p.traits.arm = false;
            p.x += f32::from(phase) * 0.12;
            run.projectiles.push(p.clone());
            super::shots(&mut panel, &run);
            run.projectiles.clear();
            for (event, x) in [
                (super::events::Mark::Release, 5.5),
                (super::events::Mark::Wall, 8.0),
                (super::events::Mark::Flesh, 10.5),
            ] {
                p.x = x;
                mark(&mut run, &p, event);
            }
            for e in &mut run.feel.ranged {
                e.ttl = (crate::rules::ranged::LIFE - 1).saturating_sub(phase);
            }
            for hero in run.players.values_mut() {
                hero.fire_cooldown = hero.fire_cooldown.saturating_sub(u32::from(phase));
            }
            run.feel.impacts.push(crate::rules::feel::Impact {
                note: false,
                x: 10.5,
                y: 2.5,
                kind,
                ttl: crate::rules::feel::IMPACT_TICKS.saturating_sub(phase),
                dx: 1.0,
                dy: 0.0,
                boss_art: None,
            });
            run.feel.blocks.push(crate::rules::feel::Block {
                x: 12.5,
                y: 2.5,
                dx: 1.0,
                dy: 0.0,
                nx: -1.0,
                ny: 0.0,
                kind,
                weight: 1,
                knight: 1,
                ttl: crate::rules::feel::BLOCK_TICKS.saturating_sub(phase),
            });
            run.feel.impacts.retain(|e| e.ttl > 0);
            run.feel.blocks.retain(|e| e.ttl > 0);
            super::super::vfx::under_shots(&mut panel, &run, Some(1));
            panel.text(2, 1, &format!("T{phase:02}"), 'J');
            cv.stamp(&panel, origin.0, origin.1 - 20);
        }
    }
    for scale in [1, 4] {
        image::RgbaImage::from_raw(
            cv.w as u32 * scale,
            cv.h as u32 * scale,
            cv.rgba_scaled(scale),
        )
        .unwrap()
        .save(dir.join(format!("ranged-closeups-{scale}x.png")))
        .unwrap();
    }
}

#[test]
fn captured_standard_round_batches_its_fallback_sprite() {
    let mut run = Run::new(11, 1, Some("Knight"));
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    let hero = &run.players[&1];
    run.projectiles
        .push(shot(Shot::Arrow, hero.x + 2.0, hero.y, false));
    draw_parts_outside(true);
    let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
    draw_parts_outside(false);
    assert!(!scene.shots.is_empty());
    assert!(scene.shots.iter().all(|s| s.mark.words().is_empty()));
    assert!(scene.shots.iter().all(|s| s.scale > 0.0));
}

#[test]
fn ranged_cues_are_read_only_and_plain_spawn_returns_to_rest() {
    let mut run = Run::new(11, 1, Some("Knight"));
    let mut p = shot(Shot::Bone, 10.0, 10.0, true);
    p.ttl = 8 * crate::rules::HZ - 1;
    mark(&mut run, &p, super::events::Mark::Release);
    let before = serde_json::to_vec(&run).unwrap();
    let mut count = 0;
    super::events::each(&run, |_| count += 1);
    assert_eq!(count, 1, "one spawn cue for a fresh plain round");
    assert_eq!(
        serde_json::to_vec(&run).unwrap(),
        before,
        "rendering never changes wire state"
    );
    for _ in 0..crate::rules::ranged::LIFE {
        run.tick_feel(&[], &[]);
    }
    count = 0;
    super::events::each(&run, |_| count += 1);
    assert_eq!(count, 0, "plain rounds do not keep a permanent spawn ring");
}

#[test]
fn hostile_danger_ink_ignores_authored_cosmetic_identity() {
    let run = Run::new(11, 1, Some("Knight"));
    let mut p = shot(Shot::Bone, 5.0, 5.0, true);
    let mut plain = Img::new(100, 100);
    assert!(super::ranged::flight(&mut plain, &run, &p));
    p.traits.borrowed = true;
    let mut dressed = Img::new(100, 100);
    assert!(super::ranged::flight(&mut dressed, &run, &p));
    assert!(
        plain.words() == dressed.words(),
        "hostility owns the danger silhouette"
    );
    p.traits.borrowed = false;
    p.look = Some(("card".into(), "custom-shot".into()));
    dressed.clear_all();
    assert!(super::ranged::flight(&mut dressed, &run, &p));
    assert!(
        plain.words() == dressed.words(),
        "hostile card art uses the same hit disc"
    );
    p.hostile = false;
    assert!(
        !super::ranged::flight(&mut dressed, &run, &p),
        "friendly card art remains authored"
    );
}

#[test]
fn muzzle_uses_release_class_not_the_nearest_knights_weapon() {
    use crate::rules::feel::{FLASH_TICKS, Puff};
    let mut run = Run::new(11, 1, Some("Knight"));
    let p = shot(Shot::Ball, 10.0, 10.0, false);
    mark(&mut run, &p, super::events::Mark::Release);
    let puff = Puff {
        x: 10.3,
        y: 10.0,
        dx: 1.0,
        dy: 0.0,
        smoke: false,
        ttl: FLASH_TICKS,
        dust: 0,
    };
    run.players.get_mut(&1).unwrap().weapon = crate::rules::Weapon::Handgonne;
    let mut powder = Img::new(200, 200);
    assert!(super::muzzle(&mut powder, &run, &puff));
    // The event still belongs to the powder shot even when an overlapping
    // knight's pose (or a weapon swap) makes a bow the nearest held arm.
    run.players.get_mut(&1).unwrap().weapon = crate::rules::Weapon::Bow;
    let mut crowded = Img::new(200, 200);
    assert!(super::muzzle(&mut crowded, &run, &puff));
    assert!(
        powder.words() == crowded.words(),
        "the release, not current equipment, owns the muzzle"
    );
}
