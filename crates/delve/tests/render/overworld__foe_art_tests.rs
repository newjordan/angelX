use super::*;
use crate::rules::bestiary::ENTRIES;
use std::collections::HashSet;

#[test]
fn dragons_breath_lights_its_throat_and_belly_inside_the_same_body() {
    let rest = sprites::enemy(EnemyKind::Dragon, 89);
    let fire = sprites::enemy(EnemyKind::Dragon, 90);
    let cool = sprites::enemy(EnemyKind::Dragon, 120);
    assert_eq!((rest.w, rest.h), (72, 46));
    assert_eq!(rest, cool, "the breath glow ends with the breath window");
    for (x, y) in [(51, 18), (31, 35)] {
        assert_ne!(rest.get(x, y), fire.get(x, y), "internal fire at {x},{y}");
        assert!(super::super::ink::is_signal(fire.get(x, y).unwrap()));
    }
    // Horn, both wing spars and the tip of the curled tail are solid in
    // both states; none is supplied by the procedural breath pixels.
    for (x, y) in [(51, 3), (24, 6), (36, 4), (5, 27)] {
        assert!(rest.solid(x, y), "dragon feature at {x},{y}");
        assert_eq!(rest.get(x, y), fire.get(x, y));
    }
    for y in 0..rest.h {
        for x in 0..59 {
            assert_eq!(rest.solid(x, y), fire.solid(x, y), "body coverage {x},{y}");
        }
    }
}

/// All authored cycles and state poses, independent of room light and camera.
#[test]
#[ignore]
fn write_foe_art_review() {
    let Some(dir) = std::env::var_os("ANGEL_FOE_REVIEW") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut manifest = Vec::new();
    let mut saved = HashSet::new();
    let mut save = |group: &str, pose: &str, im: Img| {
        let hash = im.content_hash();
        let name = format!("{hash:016x}.png");
        if saved.insert(hash) {
            image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_scaled(1))
                .unwrap()
                .save(dir.join(&name))
                .unwrap();
        }
        manifest.push(serde_json::json!({"group":group,"pose":pose,"file":name,"w":im.w,"h":im.h,"hash":hash}));
    };
    let mut run = art_v2::testing::fight();
    run.enemies.clear();
    let mut kinds: Vec<_> = ENTRIES.iter().map(|e| e.kind).collect();
    kinds.extend([
        EnemyKind::Risen,
        EnemyKind::Broodmarrow,
        EnemyKind::Oathbound,
        EnemyKind::Whirlgrave,
        EnemyKind::Dummy,
    ]);
    let mut seen = HashSet::new();
    for kind in kinds {
        if !seen.insert(format!("{kind:?}")) || kind == EnemyKind::Boss {
            continue;
        }
        run.spawn_at_for_test(kind, 24.0, 18.0);
        let mut e = run.enemies.pop().unwrap();
        e.age = 100;
        e.hurt = 0;
        let group = format!("v1-{kind:?}");
        for stage in [0, 1, 2, 3, 4, 8, 9] {
            e.stage = stage;
            for timer in [0, 12, 40] {
                e.timer = timer;
                for tick in 0..120 {
                    save(
                        &group,
                        &format!("s{stage}-t{timer}-f{tick:03}"),
                        foe_figure(&run, &e, tick),
                    );
                }
            }
        }
    }
    for tick in 0..6 {
        save(
            "v1-Impling",
            &format!("f{tick}"),
            sprites::flung_impling(tick),
        );
    }
    for dir in [
        sprites::RisenDir::South,
        sprites::RisenDir::North,
        sprites::RisenDir::East,
    ] {
        for pose in [
            sprites::RisenPose::Idle,
            sprites::RisenPose::Walk,
            sprites::RisenPose::WindUp,
            sprites::RisenPose::Shove,
            sprites::RisenPose::Hit,
            sprites::RisenPose::Rest,
        ] {
            for frame in 0..2 {
                save(
                    "v1-Risen-directions",
                    &format!("{dir:?}-{pose:?}-{frame}"),
                    sprites::risen(pose, dir, frame),
                );
            }
        }
    }
    for burden in 0..4 {
        for running in [false, true] {
            for tick in [0, 3] {
                save(
                    "v1-Goblin-sacks",
                    &format!("b{burden}-run{running}-f{tick}"),
                    sprites::goblin(burden, running, tick),
                );
            }
        }
    }
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Archive,
        Pack::Fungal,
        Pack::Hellforge,
        Pack::Unknown,
    ] {
        run.pack_bosses_for_test(pack);
    }
    for index in 0..run.bosses.len() {
        run.spawn_boss_at_for_test(index, 24.0, 18.0);
        let mut e = run.enemies.pop().unwrap();
        e.age = 100;
        e.signature.active = true;
        let group = format!("v1-{}", run.bosses[index].id);
        for stage in 0..=5 {
            e.stage = stage;
            let timers: Vec<u32> = if run.bosses[index].id == crate::rules::signatures::RIPPER
                && stage == crate::rules::signatures::DASH
            {
                (0..=33).collect()
            } else {
                vec![0, 12, 24, 48, 72]
            };
            for timer in timers {
                e.timer = timer;
                for tick in 0..120 {
                    save(
                        &group,
                        &format!("s{stage}-t{timer}-f{tick:03}"),
                        foe_figure(&run, &e, tick),
                    );
                }
            }
        }
        let old = run.dungeon.rooms[run.at].kind;
        run.dungeon.rooms[run.at].kind = crate::rules::RoomKind::Stairs;
        e.stage = 0;
        e.timer = 0;
        save(&group, "guardian", foe_figure(&run, &e, 0));
        run.dungeon.rooms[run.at].kind = old;
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("golden-candidate.json"),
        serde_json::to_vec_pretty(&approved_foe_hashes()).unwrap(),
    )
    .unwrap();
    let fight = art_v2::testing::fight();
    for (name, r) in [
        ("crypt", fight.clone()),
        ("boss", {
            let mut r = fight;
            r.enemies.clear();
            r.projectiles.clear();
            let i = r
                .bosses
                .iter()
                .position(|b| b.id == crate::rules::signatures::ENGINE)
                .unwrap();
            r.spawn_boss_at_for_test(i, 24.0, 15.0);
            r.enemies[0].signature.active = true;
            for _ in 0..90 {
                for h in r.players.values_mut() {
                    h.hp = h.max_hp;
                }
                r.step(&Default::default());
            }
            r
        }),
    ] {
        for kit in [1, 2] {
            let im = if kit == 1 {
                frame_v1(&r, 768, 448, Some(1))
            } else {
                art_v2::frame(&r, 768, 448, Some(1))
            };
            image::RgbImage::from_raw(im.w as u32, im.h as u32, im.rgb_bytes())
                .unwrap()
                .save(dir.join(format!("play-{name}-v{kit}.png")))
                .unwrap();
        }
    }
}

#[test]
fn baked_cycles_keep_every_animation_and_attack_state() {
    for kind in ENTRIES.iter().map(|e| e.kind).chain([EnemyKind::Dummy]) {
        for tick in 0..240 {
            assert_eq!(
                sprites::enemy(kind, tick),
                sprites::uncached_enemy(kind, tick),
                "{kind:?} frame {tick}"
            );
        }
        if kind.critter() {
            for stage in [0, 1, 2, 3, 4, 8, 9] {
                for timer in [0, 7, 12, 15, 28, 40] {
                    for tick in 0..96 {
                        let source = critters::figure_raw(kind, stage, timer, tick);
                        let expected = foe_material::dress(&source, kind);
                        assert_eq!(
                            critters::figure(kind, stage, timer, tick),
                            expected,
                            "{kind:?} state {stage} timer {timer} frame {tick}"
                        );
                        for y in 0..source.h {
                            for x in 0..source.w {
                                if let Some(c) = source
                                    .get(x, y)
                                    .filter(|&c| super::super::ink::is_signal(c))
                                {
                                    assert_eq!(
                                        expected.get(x, y),
                                        Some(c),
                                        "{kind:?} state ink {x},{y}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn approved_foe_hashes() -> std::collections::BTreeMap<String, u64> {
    let mut hashes = std::collections::BTreeMap::new();
    let mut run = art_v2::testing::fight();
    run.enemies.clear();
    let fold = |sum: u64, im: Img| (sum ^ im.content_hash()).wrapping_mul(0x100000001b3);
    for kind in ENTRIES.iter().map(|e| e.kind).chain([EnemyKind::Dummy]) {
        run.spawn_at_for_test(kind, 24.0, 18.0);
        let mut e = run.enemies.pop().unwrap();
        e.hurt = 0;
        let mut sum = 0xcbf29ce484222325;
        for stage in [0, 1, 2, 3, 4, 8, 9] {
            e.stage = stage;
            for timer in [0, 12, 40] {
                e.timer = timer;
                for tick in [0, 4, 8, 12, 16, 24, 28, 36, 48, 90, 119] {
                    sum = fold(sum, foe_figure(&run, &e, tick));
                }
            }
        }
        hashes.insert(format!("v1-{kind:?}"), sum);
    }
    for pack in [
        Pack::Crypt,
        Pack::Cavern,
        Pack::Archive,
        Pack::Fungal,
        Pack::Hellforge,
        Pack::Unknown,
    ] {
        run.pack_bosses_for_test(pack);
    }
    for index in 0..run.bosses.len() {
        run.spawn_boss_at_for_test(index, 24.0, 18.0);
        let mut e = run.enemies.pop().unwrap();
        e.signature.active = true;
        let mut sum = 0xcbf29ce484222325;
        for stage in 0..=5 {
            e.stage = stage;
            for timer in [0, 12, 24, 48, 72] {
                e.timer = timer;
                for tick in [0, 4, 8, 12, 16, 24, 28, 36, 48, 90, 119] {
                    sum = fold(sum, foe_figure(&run, &e, tick));
                }
            }
        }
        hashes.insert(format!("v1-{}", run.bosses[index].id), sum);
    }
    hashes
}

#[test]
fn approved_bestiary_pixels_stay_exact() {
    let expected: std::collections::BTreeMap<String, u64> =
        serde_json::from_str(include_str!("fixtures/foe-art.json")).unwrap();
    assert_eq!(approved_foe_hashes(), expected);
}

/// Same pose and tick through each existing hit interval, in both kits.
#[test]
#[ignore]
fn write_foe_hit_shots() {
    let Some(dir) = std::env::var_os("ANGEL_FOE_HIT_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut fixtures = art_v2::testing::matching::fixtures()
        .into_iter()
        .filter(|(n, _)| matches!(*n, "ripper" | "grinwick"))
        .collect::<Vec<_>>();
    for (name, kind) in [
        ("ghost", EnemyKind::Wraith),
        ("slime", EnemyKind::Slime),
        ("skeleton", EnemyKind::Skeleton),
        ("imp", EnemyKind::Imp),
        ("dragon", EnemyKind::Dragon),
        ("oathbound", EnemyKind::Oathbound),
    ] {
        let mut run = art_v2::testing::fight();
        run.enemies.clear();
        run.spawn_at_for_test(kind, 15.0, 10.0);
        fixtures.push((name, run));
    }
    for (name, mut run) in fixtures {
        run.enemies.truncate(1);
        for hurt in [0, 6, 4, 3, 1] {
            run.enemies[0].hurt = hurt;
            let src = foe_figure(&run, &run.enemies[0], run.tick as u32);
            let im = marks::flashed(&src, &run.enemies[0], &run);
            image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes())
                .unwrap()
                .save(dir.join(format!("{name}-v1-{hurt}.png")))
                .unwrap();
            if art_v2::foe_supported(&run, &run.enemies[0]) {
                let im = art_v2::foe_hit_sample(&run);
                image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes())
                    .unwrap()
                    .save(dir.join(format!("{name}-v2-{hurt}.png")))
                    .unwrap();
            }
        }
    }
}

#[test]
fn foe_hits_keep_material_detail_coverage_and_the_existing_intervals() {
    use super::super::ink::{BANK, bank_of, palette_index};
    let mut fixtures = art_v2::testing::matching::fixtures();
    fixtures.retain(|(n, _)| matches!(*n, "ripper" | "grinwick"));
    for (name, kind) in [
        ("ghost", EnemyKind::Wraith),
        ("slime", EnemyKind::Slime),
        ("skeleton", EnemyKind::Skeleton),
        ("imp", EnemyKind::Imp),
        ("dragon", EnemyKind::Dragon),
    ] {
        let mut run = art_v2::testing::fight();
        run.enemies.clear();
        run.spawn_at_for_test(kind, 15.0, 10.0);
        fixtures.push((name, run));
    }
    for (name, run) in fixtures {
        let mut e = run.enemies[0].clone();
        e.hp = e.max_hp;
        e.hurt = 0;
        let src = foe_figure(&run, &e, run.tick as u32);
        assert_eq!(marks::flashed(&src, &e, &run), src);
        let mut phases = Vec::new();
        for hurt in [6, 4, 3, 1] {
            e.hurt = hurt;
            let hit = marks::flashed(&src, &e, &run);
            let mut colors = HashSet::new();
            let mut brightness = 0u64;
            for y in 0..src.h {
                for x in 0..src.w {
                    assert_eq!(src.solid(x, y), hit.solid(x, y), "{name}: coverage {x},{y}");
                    if let Some(c) = hit.get(x, y) {
                        colors.insert(c);
                        assert!(palette_index(c).is_some(), "{name}: palette {c:?}");
                        assert!(
                            c.iter().copied().max().unwrap() <= 228,
                            "{name}: white bound"
                        );
                        let source_bank = bank_of(src.get(x, y).unwrap());
                        if name == "grinwick" && hurt >= 4 && source_bank == Some(2) {
                            assert!(
                                matches!(palette_index(c), Some(41..=44 | 56..=59 | 70..=71)),
                                "engine: brass must catch the bounded pale metal ramp"
                            );
                        } else if matches!(source_bank, Some(1 | 2)) {
                            assert_eq!(bank_of(c), source_bank, "{name}: own ramp");
                        }
                        brightness += c.iter().map(|&n| u64::from(n)).sum::<u64>();
                    }
                }
            }
            assert!(
                colors.len() >= 4,
                "{name}: hurt={hurt} erased material planes ({BANK}-ink banks)"
            );
            phases.push((hit, brightness));
        }
        assert_eq!(phases[0].0, phases[1].0, "held threshold remains 4");
        assert_eq!(phases[2].0, phases[3].0, "settling interval remains 1..4");
        assert!(
            phases[0].1 > phases[2].1,
            "{name}: held hit must read brighter than settling"
        );
        if name == "grinwick" {
            let mut seen = 0;
            let mut lifted = 0;
            let luma =
                |c: [u8; 3]| (u32::from(c[0]) * 3 + u32::from(c[1]) * 6 + u32::from(c[2])) / 10;
            for y in 0..src.h {
                for x in 0..src.w {
                    if let Some(c) = src.get(x, y) {
                        seen += 1;
                        lifted += usize::from(luma(phases[0].0.get(x, y).unwrap()) >= luma(c) + 30);
                    }
                }
            }
            assert!(
                lifted * 3 >= seen * 2,
                "engine's body and joints must read as hit at 1×"
            );
        }
    }
}
