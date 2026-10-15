//! Effect contact sheets use the actual render entry points, at four ticks.
use super::*;
use crate::rules::{Shot, classes, feel as state, patterns, skills, ults as ultimate};

const CELL: i32 = 112;
fn quiet() -> Run {
    let mut r = Run::new(7, 1, None);
    r.enemies.clear();
    r.projectiles.clear();
    r.works = Default::default();
    r.feel = Default::default();
    let h = r.players.get_mut(&1).unwrap();
    h.x = 7.0;
    h.y = 7.0;
    r
}
fn page(rows: &[(&str, Box<dyn Fn(&mut Img, u32)>)]) -> Img {
    let mut out = Img::black(90 + CELL * 4, rows.len() as i32 * CELL);
    for (i, (name, draw)) in rows.iter().enumerate() {
        out.text(2, i as i32 * CELL + 48, name, 'J');
        for f in 0..4 {
            let mut im = Img::new(CELL, CELL);
            draw(&mut im, f);
            out.stamp(&im, 90 + f as i32 * CELL, i as i32 * CELL);
        }
    }
    out
}
fn sheets() -> Vec<(&'static str, Img)> {
    let mut pages = Vec::new();
    let mut shots: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for (name, kind) in [
        ("BOW", Shot::Arrow),
        ("CROSSBOW", Shot::Bolt),
        ("HANDGONNE", Shot::Ball),
        ("BLADEWIND", Shot::Blade),
        ("BONE", Shot::Bone),
        ("ORB", Shot::Orb),
        ("FROST", Shot::Frost),
        ("HEX", Shot::Hex),
        ("EMBER", Shot::Ember),
        ("SACRED", Shot::Sacred),
        ("MISSILE", Shot::Missile),
        ("WISP", Shot::Wisp),
        ("ORE", Shot::Ore),
        ("FLAME", Shot::Flame),
        ("PAGE", Shot::Page),
        ("SPORE", Shot::Spore),
        ("EYE", Shot::Eye),
    ] {
        shots.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.projectiles.push(
                    serde_json::from_value(serde_json::json!({
                        "x":7.0,"y":7.0,"vx":1.0,"vy":f as f32*0.2,"hostile":false,
                        "kind":kind,"damage":1,"pierce":0,"ttl":100
                    }))
                    .unwrap(),
                );
                super::shots(cv, &r, false);
            }),
        ));
    }
    for (name, colour) in [("ARCANE", classes::ARCANE), ("HOLY", classes::HOLY)] {
        shots.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 2);
                r.projectiles.push(
                    serde_json::from_value(serde_json::json!({
                        "x":7.0,"y":7.0,"vx":1.0,"vy":0.0,"hostile":false,
                        "kind":Shot::Orb,"look":[colour,"orb"],"damage":1,"pierce":0,"ttl":100
                    }))
                    .unwrap(),
                );
                super::shots(cv, &r, false);
            }),
        ));
    }
    pages.push(("shots", page(&shots)));
    let mut modern: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for (name, kind) in [
        ("V2 WISP", Shot::Wisp),
        ("V2 EMBER", Shot::Ember),
        ("V2 PAGE", Shot::Page),
        ("V2 BALL", Shot::Ball),
    ] {
        modern.push((
            name,
            Box::new(move |cv, f| {
                let im = art_v2::fx_shot_sample(kind, f);
                // This page shows the native world footprint at two RGB samples.
                cv.stamp(&im, -56, -56);
            }),
        ));
    }
    pages.push(("shots-v2", page(&modern)));
    let mut hits: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for (name, kind) in [
        ("BOW HIT", Shot::Arrow),
        ("RAIL HIT", Shot::Bolt),
        ("BALL HIT", Shot::Ball),
        ("HOLY HIT", Shot::Sacred),
    ] {
        hits.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.feel.impacts.push(state::Impact {
                    x: 7.0,
                    y: 7.0,
                    kind,
                    ttl: [6, 4, 2, 1][f as usize],
                });
                feel::over(cv, &r);
            }),
        ));
    }
    for (name, what) in [
        ("TRACER", 0),
        ("MUZZLE", 1),
        ("SMOKE", 2),
        ("DEATH", 3),
        ("COIN", 4),
        ("STRUCK", 5),
    ] {
        hits.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 3);
                match what {
                    0 => r.feel.tracers.push(state::Tracer {
                        from: (3.0, 7.0),
                        to: (11.0, 7.0),
                        thick: true,
                        ttl: [8, 6, 3, 1][f as usize],
                    }),
                    1 | 2 => r.feel.puffs.push(state::Puff {
                        x: 7.0,
                        y: 7.0,
                        dx: 1.0,
                        dy: 0.0,
                        smoke: what == 2,
                        ttl: if what == 1 {
                            [3, 2, 1, 0][f as usize]
                        } else {
                            [14, 10, 6, 1][f as usize]
                        },
                    }),
                    3 => r.feel.deaths.push(state::Death {
                        x: 7.0,
                        y: 7.0,
                        r: 1.0,
                        ttl: [14, 11, 7, 1][f as usize],
                    }),
                    4 => r.feel.coins.push(state::Coin {
                        x: 7.0,
                        y: 7.0,
                        age: f * 3,
                    }),
                    _ => {
                        let h = r.players.get_mut(&1).unwrap();
                        h.struck = crate::rules::spectacle::STRUCK - f * 2;
                        strike::sparks(cv, h, 1, 56, 63);
                    }
                }
                feel::over(cv, &r);
            }),
        ));
    }
    hits.push((
        "HIT FLASH",
        Box::new(|cv, f| {
            let mut r = quiet();
            let h = r.players.get_mut(&1).unwrap();
            h.struck = crate::rules::spectacle::STRUCK - f * 2;
            let body = knights::portrait(h, 1);
            let (im, dx, dy) = strike::pose(&body, h);
            cv.stamp(&im, 56 - im.w / 2 + dx, 64 - im.h + dy);
        }),
    ));
    hits.push((
        "SPIN",
        Box::new(|cv, f| {
            knights::spin_marks(
                cv,
                56,
                56,
                crate::rules::spin::TICKS - f * 3,
                crate::rules::spin::REACH * UNIT,
            )
        }),
    ));
    hits.push((
        "V2 HIT FLASH",
        Box::new(|cv, f| cv.stamp(&art_v2::fx_hit_sample(f), -56, -56)),
    ));
    pages.push(("hits", page(&hits)));
    let mut hazards: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for (name, what) in [
        ("GAS", 0),
        ("SULPHUR", 1),
        ("GAS WALL", 2),
        ("FIRE PIT", 3),
        ("MINE", 4),
        ("PALISADE", 5),
        ("MINE BLAST", 6),
        ("GAS FLASH", 7),
        ("DUST", 8),
        ("BURST", 9),
        ("CLANG", 10),
    ] {
        hazards.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 5);
                match what {
                    0..=2 => r.works.clouds.push(skills::Cloud {
                        x: 7.0,
                        y: 7.0,
                        vx: 0.0,
                        vy: 0.0,
                        age: [0, 10, 35, 100][f as usize],
                        owner: 0,
                        power: 1,
                        tint: what,
                    }),
                    3 => r.works.blazes.push(skills::Blaze {
                        x: 7.0,
                        y: 7.0,
                        age: f * 12,
                        life: 60,
                        owner: 1,
                        power: 1,
                        radius: 2.5,
                    }),
                    4 => r.works.mines.push(skills::Mine {
                        x: 7.0,
                        y: 7.0,
                        age: f * 20,
                        owner: 1,
                        power: 1,
                        fuse: if f == 3 { 3 } else { 0 },
                    }),
                    5 => r.works.barricades.push(skills::Barricade {
                        a: (3.0, 7.0),
                        b: (11.0, 7.0),
                        hp: skills::WALL_HP,
                        age: f * 5,
                        owner: 1,
                        fall: if f == 3 { Some(4) } else { None },
                    }),
                    _ => r.works.booms.push(skills::Boom {
                        x: 7.0,
                        y: 7.0,
                        left: [12, 9, 5, 1][f as usize],
                        of: 12,
                        kind: [
                            skills::BoomKind::Mine,
                            skills::BoomKind::Flash,
                            skills::BoomKind::Dust,
                            skills::BoomKind::Burst,
                            skills::BoomKind::Clang,
                        ][(what - 6) as usize],
                        reach: 3.0,
                    }),
                }
                works::under_with(cv, &r, false);
                works::over(cv, &r);
            }),
        ));
    }
    pages.push(("hazards", page(&hazards)));
    let specials: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = vec![
        (
            "STILLHOUR",
            Box::new(|cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 6);
                r.spheres.push(ultimate::Sphere {
                    x: 7.0,
                    y: 7.0,
                    r: 4.0,
                    left: 100 - f * 30,
                });
                ults::under(cv, &r);
            }),
        ),
        (
            "TREBUCHET",
            Box::new(|cv, f| {
                let mut r = quiet();
                r.strikes.push(ultimate::Strike {
                    x: 7.0,
                    y: 12.0,
                    fall: 36 - f * 10,
                    damage: 1,
                    owner: 1,
                });
                ults::under(cv, &r);
            }),
        ),
        (
            "LADY VEIL",
            Box::new(|cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 6);
                r.players.get_mut(&1).unwrap().angel = 100;
                ults::under(cv, &r);
            }),
        ),
        (
            "WISH",
            Box::new(|cv, f| {
                let mut r = quiet();
                r.tick = u64::from(f * 5);
                r.boons.push((1, 0));
                flourishes(cv, &r);
            }),
        ),
        (
            "BALL BURST",
            Box::new(|cv, f| {
                let mut r = quiet();
                r.blasts.push((
                    7.0,
                    7.0,
                    crate::rules::BLAST_TICKS.saturating_sub(f as u8 * 2),
                ));
                flourishes(cv, &r);
            }),
        ),
        (
            "BOMB",
            Box::new(|cv, f| {
                cv.stamp(&sprites::bomb(f * 2), 54, 54);
            }),
        ),
    ];
    pages.push(("specials", page(&specials)));
    let r = quiet();
    let mut icons: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    let mut supplies: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    let mut charms: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for c in &r.book.cards {
        let c = c.clone();
        let label = c.id.as_str();
        let name: &'static str = Box::leak(
            label
                .to_uppercase()
                .chars()
                .take(14)
                .collect::<String>()
                .into_boxed_str(),
        );
        let family = match c.kind {
            crate::rules::cards::Kind::Arm | crate::rules::cards::Kind::Guard => &mut icons,
            crate::rules::cards::Kind::Take => &mut supplies,
            _ => &mut charms,
        };
        family.push((
            name,
            Box::new(move |cv, f| {
                let im = cards::floor_card(Some(&c), f * 15);
                cv.stamp(&im, 41, 47);
                let art = cards::card_art(&c);
                cv.stamp(&art, 68, 47);
            }),
        ));
    }
    for kind in crate::rules::runes::RuneKind::ALL {
        supplies.push((
            kind.name(),
            Box::new(move |cv, _| cv.stamp(&runes::orb(kind), 51, 51)),
        ));
    }
    pages.push(("pickup-icons", page(&supplies)));
    pages.push(("card-icons", page(&charms)));
    for icon in r.tray(1, &crate::rules::hud::Keys::FIRST) {
        let name = match icon.slot {
            crate::rules::tray::Slot::Bomb => "BOMB TRAY",
            crate::rules::tray::Slot::Ult => "ULT TRAY",
            crate::rules::tray::Slot::Guard => "GUARD TRAY",
            _ => "WEAPON TRAY",
        };
        icons.push((
            name,
            Box::new(move |cv, f| {
                let mut icon = icon.clone();
                icon.fill = f as f32 / 3.0;
                cv.stamp(&tray::cell(&icon), 47, 45);
            }),
        ));
    }
    pages.push(("icons", page(&icons)));
    // Pattern fixtures exercise warning and burn times without advancing gameplay.
    let mut tells: Vec<(&str, Box<dyn Fn(&mut Img, u32)>)> = Vec::new();
    for (name, emit) in [
        ("RAIN", patterns::Emitter::Rain),
        ("LASER", patterns::Emitter::Laser),
        ("SWEEP", patterns::Emitter::Sweep),
        ("RING", patterns::Emitter::GapRing),
        ("FLOWER", patterns::Emitter::Flower),
        ("MINES", patterns::Emitter::Mines),
    ] {
        tells.push((
            name,
            Box::new(move |cv, f| {
                let mut r = quiet();
                r.exhibit_pattern_for_test(emit, Shot::Wisp, (7.0, 7.0), (11.0, 7.0));
                for p in &mut r.projectiles {
                    if p.traits.path.beam.is_some() {
                        p.traits.path.age = [0, 18, 21, 27][f as usize];
                        p.traits.path.beam.as_mut().unwrap().reach = 6.0;
                    } else if p.traits.path.wait > 0 {
                        p.x = 7.0;
                        p.y = 7.0;
                        p.traits.path.wait = [20, 9, 5, 1][f as usize];
                    } else {
                        p.x = 7.0;
                        p.y = 7.0;
                    }
                }
                r.tick = u64::from(f * 3);
                super::shots(cv, &r, false);
            }),
        ));
    }
    pages.push(("warnings", page(&tells)));
    let fixtures = art_v2::testing::matching::fixtures();
    for modern in [false, true] {
        let (w, h) = if modern { (768, 448) } else { (384, 224) };
        let mut out = Img::black(90 + 4 * w, 2 * h);
        for (row, name) in ["ripper", "grinwick"].into_iter().enumerate() {
            let original = &fixtures.iter().find(|(n, _)| *n == name).unwrap().1;
            out.text(2, row as i32 * h + h / 2, name, 'J');
            for phase in 0..4 {
                let mut r = original.clone();
                r.tick += phase as u64 * 4;
                let e = &mut r.enemies[0];
                e.stage = [
                    crate::rules::signatures::INHALE,
                    crate::rules::signatures::TELL,
                    crate::rules::signatures::DASH,
                    crate::rules::signatures::DAZED,
                ][phase];
                let im = if modern {
                    art_v2::fx_boss_sample(&r)
                } else {
                    let mut im = Img::new(w, h);
                    signatures::under_with(&mut im, &r, false);
                    works::under_with(&mut im, &r, false);
                    im
                };
                out.stamp(&im, 90 + phase as i32 * w, row as i32 * h);
            }
        }
        pages.push((
            if modern {
                "boss-patterns-v2"
            } else {
                "boss-patterns"
            },
            out,
        ));
    }
    pages
}
#[test]
#[ignore]
fn write_fx_sheets() {
    let Some(dir) = std::env::var_os("ANGEL_FX_SHEETS") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let all = sheets();
    let hashes: std::collections::BTreeMap<&str, u64> =
        all.iter().map(|(n, im)| (*n, im.content_hash())).collect();
    std::fs::write(
        std::path::Path::new(&dir).join("fx-art.json"),
        serde_json::to_string_pretty(&hashes).unwrap(),
    )
    .unwrap();
    std::fs::write(
        std::path::Path::new(&dir).join("fx-footprints.json"),
        serde_json::to_string_pretty(&footprints()).unwrap(),
    )
    .unwrap();
    for (name, im) in all {
        let im = image::RgbaImage::from_raw(im.w as u32, im.h as u32, im.rgba_bytes()).unwrap();
        let path = std::path::Path::new(&dir).join(format!("{name}.png"));
        im.save(path).unwrap();
    }
}

fn footprints() -> std::collections::BTreeMap<String, u64> {
    let mut out = std::collections::BTreeMap::new();
    let mask = |im: &Img| {
        (0..im.h)
            .flat_map(|y| (0..im.w).map(move |x| im.get(x, y).is_some() as u8))
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
            })
    };
    for kind in [
        Shot::Blade,
        Shot::Arrow,
        Shot::Bolt,
        Shot::Ball,
        Shot::Bone,
        Shot::Orb,
        Shot::Ember,
        Shot::Sacred,
        Shot::Hex,
        Shot::Frost,
        Shot::Missile,
        Shot::Wisp,
        Shot::Ore,
        Shot::Flame,
        Shot::Page,
        Shot::Spore,
        Shot::Eye,
    ] {
        for f in 0..4 {
            let mut im = Img::new(25, 25);
            sprites::shot(&mut im, (12, 12), (1.0, f as f32 * 0.2), kind);
            out.insert(format!("{kind:?}-{f}"), mask(&im));
        }
    }
    for colour in [classes::ARCANE, classes::HOLY] {
        for f in 0..4 {
            let mut im = Img::new(25, 25);
            class_looks::shot(&mut im, (12, 12), (1.0, 0.0), colour, f * 2);
            out.insert(format!("{colour}-{f}"), mask(&im));
        }
    }
    for emitter in [
        patterns::Emitter::Rain,
        patterns::Emitter::Laser,
        patterns::Emitter::Sweep,
        patterns::Emitter::Crossfire,
        patterns::Emitter::Mines,
    ] {
        for f in 0..4 {
            let mut r = quiet();
            r.exhibit_pattern_for_test(emitter, Shot::Wisp, (7.0, 7.0), (11.0, 7.0));
            r.tick = f * 3;
            let mut im = Img::new(112, 112);
            for p in &mut r.projectiles {
                p.traits.path.age = [0, 18, 21, 27][f as usize];
                if let Some(b) = p.traits.path.beam.as_mut() {
                    b.reach = 6.0;
                }
                if p.traits.path.wait > 0 {
                    p.traits.path.wait = [20, 9, 5, 1][f as usize];
                }
            }
            for p in &r.projectiles {
                volleys::draw(&mut im, &r, p);
            }
            out.insert(format!("{emitter:?}-{f}"), mask(&im));
        }
    }
    out
}
#[test]
fn effect_cores_and_warning_footprints_keep_the_approved_geometry() {
    let expected: std::collections::BTreeMap<String, u64> =
        serde_json::from_str(include_str!("fixtures/fx-footprints.json")).unwrap();
    assert_eq!(footprints(), expected);
}
#[test]
fn reviewed_effect_frames_keep_their_pixels() {
    let actual: std::collections::BTreeMap<String, u64> = sheets()
        .into_iter()
        .map(|(n, im)| (n.into(), im.content_hash()))
        .collect();
    let expected: std::collections::BTreeMap<String, u64> =
        serde_json::from_str(include_str!("fixtures/fx-art.json")).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn rgb_gas_volume_stays_below_actors_while_its_rim_is_protected() {
    let mut run = quiet();
    run.works.clouds.push(skills::Cloud {
        x: 7.0,
        y: 7.0,
        vx: 0.0,
        vy: 0.0,
        age: 20,
        owner: 0,
        power: 1,
        tint: 2,
    });
    let mut under = Img::new(112, 112);
    works::under_with(&mut under, &run, false);
    assert!(
        under.get(56, 56).is_some(),
        "default volume belongs below the figures"
    );
    let mut overlay = Img::new(112, 112);
    works::under_with(&mut overlay, &run, true);
    assert!(
        overlay.get(56, 56).is_none(),
        "RGB volume must not enter the protected overlay and cover an actor"
    );
    assert!(
        overlay.pixels().any(|c| c != BLACK),
        "the danger rim remains protected"
    );
}
