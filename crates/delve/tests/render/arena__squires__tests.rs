use super::*;
#[test]
fn five_small_silhouettes_and_owner_palettes_are_distinct() {
    let all: Vec<_> = Kind::ALL
        .into_iter()
        .map(|kind| figure(kind, 1).rgba_scaled(1))
        .collect();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_ne!(
        figure(Kind::Ranger, 1).rgba_scaled(1),
        figure(Kind::Ranger, 2).rgba_scaled(1)
    );
    assert!(figure(Kind::Ranger, 1).h < sprites::knight(1).h);
}
fn yard_run() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == crate::rules::RoomKind::SquireHall)
        .unwrap();
    run.enter_for_test(yard);
    run.home.squire_hall.refresh(0);
    run
}

fn on_plate(run: &mut Run, id: u32, i: usize) {
    let (c, r, w, h) = PLATES[i];
    let hero = run.players.get_mut(&id).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * 2.0;
    hero.y = (r as f32 + h as f32 / 2.0) * 2.0;
}

fn stocked(run: &mut Run, count: usize, level: u8) {
    let key = Home::knight_key(&run.players[&1]);
    run.home.squire_hall.rosters.insert(
        key,
        crate::realm::squires::Roster {
            active: (count > 0).then_some(1),
            squires: (0..count)
                .map(|i| Squire {
                    id: i as u32 + 1,
                    name: "Pip 'the extraordinarily long'".into(),
                    kind: Kind::ALL[i % Kind::ALL.len()],
                    level,
                    talent: 5,
                })
                .collect(),
        },
    );
}

fn assert_panel_bounds(run: &Run, i: usize) {
    let p = panel(run, 1, &run.players[&1], i);
    let im = panel_image(&p);
    let cv = Img::new(384, 224);
    let (x, y) = dock(&cv, &im);
    assert!(x >= MARGIN && x + im.w <= cv.w - MARGIN);
    assert!(
        y > (PLATES[0].1 + PLATES[0].3) * TILE + 8,
        "dock must clear all plate actors: {p:?}"
    );
    assert!(y + im.h <= cv.h - MARGIN);
    for text in p
        .lines
        .iter()
        .map(|(s, _)| s)
        .chain(std::iter::once(&p.prompt))
    {
        assert!(text_width(text) <= im.w - 16, "clipped text: {text}");
    }
    assert!(
        (2..=3).contains(&p.prompt.split_whitespace().count()),
        "short instruction: {}",
        p.prompt
    );
    assert!(p.lines.len() <= 8, "overlong panel: {p:?}");
    // The last text row and progress rail have separate, padded rows.
    assert!(5 + (p.lines.len() as i32 - 1) * 10 + 7 <= im.h - 20);
}

#[test]
fn yard_signs_and_dock_clear_all_nine_occupied_plates() {
    let mut run = yard_run();
    for id in 2..=6 {
        run.join_seat(id, &format!("Knight {id}"));
    }
    for i in 0..PLATES.len() {
        let (c, r, _, h) = PLATES[i];
        let id = i as u32 % 6 + 1;
        assert_eq!(run.room().tile(c, r), crate::rules::Tile::Floor);
        on_plate(&mut run, id, i);
        assert_eq!(plate_at(run.players[&id].x, run.players[&id].y), Some(i));
        for y in [r * TILE, (r + h) * TILE - 1] {
            let knight = knights::figure(&run.players[&id], id);
            assert!(SIGN_Y + station_sign(&run, i / 3).h + 2 <= y - knight.h + 1);
        }
        assert_panel_bounds(&run, i);
        let im = panel_image(&panel(&run, id, &run.players[&id], i));
        let mut cv = Img::new(384, 224);
        board(&mut cv, &run, Some(id));
        let (x, y) = dock(&cv, &im);
        let mut expected = Img::new(384, 224);
        signs(&mut expected, &run);
        expected.stamp(&im, x, y);
        assert_eq!(cv, expected, "only the focused seat's panel may be painted");
    }
    // Actual body stamps cannot eat even one sign pixel (ELECT / ST regression).
    let mut clean = Img::new(384, 224);
    signs(&mut clean, &run);
    let mut occupied = Img::new(384, 224);
    // All nine positions are stamped, using the six legal knight looks.
    for i in 0..PLATES.len() {
        let id = i as u32 % 6 + 1;
        on_plate(&mut run, id, i);
        let hero = &run.players[&id];
        let (x, y) = at(hero.x, hero.y);
        stand(&mut occupied, &knights::figure(hero, id), x, y);
    }
    // A walker north of the slabs must not erase the wall directory either.
    stand(
        &mut occupied,
        &knights::figure(&run.players[&1], 1),
        58,
        SIGN_Y + 20,
    );
    board(&mut occupied, &run, Some(1));
    for i in 0..3 {
        let sign = station_sign(&run, i);
        let x = PLATES[i * 3].0 * TILE - 6;
        for y in SIGN_Y..SIGN_Y + sign.h {
            for xx in x..x + sign.w {
                assert_eq!(occupied.get(xx, y), clean.get(xx, y));
            }
        }
        if i < 2 {
            assert!(x + sign.w + 4 <= PLATES[(i + 1) * 3].0 * TILE - 6);
        }
    }
}

#[test]
fn station_actions_are_separated_and_fit_the_sign() {
    let run = yard_run();
    for (i, expected) in [
        "NEXT / HIRE / UP",
        "SELECT / REST / UP",
        "TRAIN / ROSTER / UP",
    ]
    .into_iter()
    .enumerate()
    {
        let actions = station_actions(i);
        assert_eq!(actions, expected);
        let sign = station_sign(&run, i);
        let width = station_actions_width(&actions);
        assert!(width <= sign.w - 6);
        let mut expected_sign = paper(sign.w, sign.h);
        let mut x = (sign.w - width) / 2;
        for word in expected.split(' ') {
            expected_sign.text(x, 13, word, 'i');
            x += text_width(word) + 4;
            // Each slash and verb has four blank columns before the next glyph.
            if word != "UP" {
                for xx in x - 4..x {
                    for y in 13..20 {
                        assert_eq!(sign.get(xx, y), Some(PAPER));
                    }
                }
            }
        }
        for y in 13..20 {
            for x in 0..sign.w {
                assert_eq!(sign.get(x, y), expected_sign.get(x, y));
            }
        }
    }
}

#[test]
fn black_paper_uses_only_tonal_greys_and_warm_amber() {
    let run = yard_run();
    let mut images = (0..3).map(|i| station_sign(&run, i)).collect::<Vec<_>>();
    images.extend((0..9).map(|i| panel_image(&panel(&run, 1, &run.players[&1], i))));
    for im in images {
        for p in im.rgba_scaled(1).chunks_exact(4).filter(|p| p[3] != 0) {
            assert!(p[0] >= p[2] && p[1] >= p[2], "no blue or glow bank: {p:?}");
        }
    }
}

#[test]
fn panel_bounds_cover_all_levels_types_full_roster_and_large_treasury() {
    let mut run = yard_run();
    for level in 0..=3 {
        run.home.squire_hall.recruit = level;
        run.home.squire_hall.hire = level;
        run.home.squire_hall.train = level;
        run.home.squire_hall.ready = false;
        run.home.squire_hall.refresh(0);
        run.treasury = Default::default();
        run.treasury.add(crate::realm::Spoil::Gold, u32::MAX);
        for kind in Kind::ALL {
            for s in &mut run.home.squire_hall.offers {
                s.kind = kind;
                s.name = "é'long name goes well beyond the border".into();
            }
            for count in [0, run.home.squire_hall.cap()] {
                stocked(&mut run, count, 3 + level * 2);
                for i in 0..PLATES.len() {
                    on_plate(&mut run, 1, i);
                    assert_panel_bounds(&run, i);
                }
            }
        }
        for i in [2, 5, 8] {
            let p = panel(&run, 1, &run.players[&1], i);
            assert_eq!(
                p.enabled,
                level < 3,
                "each building must retain all three upgrades"
            );
            assert_eq!(p.lines[0].0, format!("UPGRADE {}", STATIONS[i / 3]));
        }
    }
}

#[test]
fn hire_and_training_blockers_and_ability_names_are_explicit() {
    let mut run = yard_run();
    let offer = run.home.squire_hall.offers[0].clone();
    let price = run.home.squire_hall.wage(&offer);
    let poor = panel(&run, 1, &run.players[&1], 1);
    assert!(!poor.enabled);
    assert!(
        poor.lines
            .iter()
            .any(|(s, _)| s == &format!("NEED {price}G"))
    );
    assert_eq!(poor.prompt, "HIRE BLOCKED");
    run.treasury.add(crate::realm::Spoil::Gold, price);
    assert!(panel(&run, 1, &run.players[&1], 1).enabled);
    let cap = run.home.squire_hall.cap();
    stocked(&mut run, cap, 3);
    let full = panel(&run, 1, &run.players[&1], 1);
    assert!(!full.enabled);
    assert!(full.lines.iter().any(|(s, _)| s == "ROSTER FULL"));
    let capped = panel(&run, 1, &run.players[&1], 6);
    assert!(!capped.enabled);
    assert!(capped.lines.iter().any(|(s, _)| s == "UPGRADE TRAIN"));
    for kind in Kind::ALL {
        let key = Home::knight_key(&run.players[&1]);
        run.home.squire_hall.rosters.get_mut(&key).unwrap().squires[0].kind = kind;
        let p = panel(&run, 1, &run.players[&1], 3);
        assert_eq!(
            p.lines
                .iter()
                .filter(|(s, _)| s == &format!("KIT {}", kind.abilities(3).to_uppercase()))
                .count(),
            1
        );
        assert!(
            !p.lines
                .iter()
                .any(|(s, _)| s.contains("one active") || s.contains("Select / Roster"))
        );
    }
}

#[test]
fn training_previews_active_squires_stats_and_unlocks_at_every_level() {
    let mut run = yard_run();
    run.home.squire_hall.train = 3;
    stocked(&mut run, 1, 1);
    let key = Home::knight_key(&run.players[&1]);
    for kind in Kind::ALL {
        for level in 1..=9 {
            let s = &mut run.home.squire_hall.rosters.get_mut(&key).unwrap().squires[0];
            s.kind = kind;
            s.level = level;
            let current = s.clone();
            for gold in [0, u32::MAX] {
                run.treasury = Default::default();
                run.treasury.add(crate::realm::Spoil::Gold, gold);
                let p = panel(&run, 1, &run.players[&1], 6);
                assert!(p.lines[0].0.contains("ACTIVE SQUIRE"));
                assert!(p.lines.iter().any(|(s, _)| s == &format!(
                    "KIT {}", kind.abilities(level).to_uppercase()
                )));
                assert_eq!(p.enabled, level < 9 && gold > 0);
                if level < 9 {
                    let mut next = current.clone();
                    next.level += 1;
                    assert!(p.lines.iter().any(|(s, _)| s
                        == &format!(
                            "NEXT LV{} / HP {} / POWER {}",
                            next.level,
                            next.health(),
                            next.power()
                        )));
                    let unlock = match (kind, level) {
                        (Kind::Ninja, 1) => "UNLOCK BACKSTAB",
                        (Kind::Ninja, 2) => "UNLOCK SMOKE",
                        (Kind::Healer, 1) => "UNLOCK CLEANSE",
                        (Kind::Healer, 2) => "UNLOCK SKULL AID",
                        (Kind::Ranger, 1) => "UNLOCK MARK",
                        (Kind::Shield, 1) => "UNLOCK GUARD",
                        (Kind::Alchemist, 1) => "UNLOCK POTION",
                        _ => "KIT UNCHANGED",
                    };
                    assert!(
                        p.lines.iter().any(|(s, _)| s == unlock),
                        "{kind:?} LV{level}: {p:?}"
                    );
                } else {
                    assert!(p.lines.iter().any(|(s, _)| s == "LEVEL CAP 9"));
                    assert!(
                        !p.lines
                            .iter()
                            .any(|(s, _)| s.starts_with("NEXT ") || s.starts_with("UNLOCK "))
                    );
                }
                assert_panel_bounds(&run, 6);
            }
        }
    }
    run.home.squire_hall.rosters.get_mut(&key).unwrap().active = None;
    let p = panel(&run, 1, &run.players[&1], 6);
    assert!(!p.enabled);
    assert!(p.lines.iter().any(|(s, _)| s == "SELECT A SQUIRE"));
    assert!(
        !p.lines.iter().any(|(s, _)| s.contains("ACTIVE")
            || s.starts_with("NEXT ")
            || s.starts_with("UNLOCK "))
    );
}

#[test]
fn hold_rail_measures_only_the_current_action_and_resets_on_release() {
    let mut run = yard_run();
    on_plate(&mut run, 1, 0);
    let input = [(
        1,
        crate::rules::Input {
            interact: true,
            ..Default::default()
        },
    )]
    .into_iter()
    .collect();
    let hold = crate::rules::home::BUY_HOLD;
    for _ in 0..hold / 2 {
        run.step(&input);
    }
    let p = panel(&run, 1, &run.players[&1], 0);
    assert_eq!(p.progress, hold / 2);
    let im = panel_image(&p);
    let filled = (0..PANEL_W)
        .filter(|&x| im.get(x, im.h - 8) == super::super::super::ink::ink('O'))
        .count();
    assert_eq!(
        filled as i32,
        (PANEL_W - 16) * p.progress as i32 / hold as i32
    );
    on_plate(&mut run, 1, 1);
    assert_eq!(
        panel(&run, 1, &run.players[&1], 1).progress,
        0,
        "no borrowed progress"
    );
    run.step(&Default::default());
    on_plate(&mut run, 1, 0);
    assert_eq!(panel(&run, 1, &run.players[&1], 0).progress, 0);
    for _ in 0..hold - 1 {
        run.step(&input);
    }
    assert_eq!(panel(&run, 1, &run.players[&1], 0).progress, hold - 1);
    run.step(&input);
    assert_eq!(
        run.squire_hall.offers.get(&1),
        Some(&1),
        "one completed hold browses once"
    );
    assert_eq!(panel(&run, 1, &run.players[&1], 0).progress, 0);
    for _ in 0..hold {
        run.step(&input);
    }
    assert_eq!(
        run.squire_hall.offers.get(&1),
        Some(&1),
        "the input latch still requires release"
    );
    run.step(&Default::default());
    for _ in 0..hold {
        run.step(&input);
    }
    assert_eq!(run.squire_hall.offers.get(&1), Some(&2));
}

#[test]
#[ignore = "writes the squire yard and companion review sheets"]
fn write_squire_shots() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("ANGEL_SQUIRE_SHOTS").expect("ANGEL_SQUIRE_SHOTS"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, im: Img| {
        image::RgbaImage::from_raw(im.w as u32 * 3, im.h as u32 * 3, im.rgba_scaled(3))
            .unwrap()
            .save(dir.join(name))
            .unwrap()
    };
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    let yard = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == crate::rules::RoomKind::SquireHall)
        .unwrap();
    run.enter_for_test(yard);
    run.home.squire_hall.refresh(0);
    for id in 2..=6 {
        run.join_seat(id, &format!("Knight {id}"));
    }
    for id in 1..=6 {
        let h = run.players.get_mut(&id).unwrap();
        h.x = 8.0 + id as f32 * 5.0;
        h.y = 10.0;
    }
    let h = run.players.get_mut(&1).unwrap();
    h.x = 9.0;
    h.y = 9.0;
    let mut cv = super::super::yard::scenery(&run);
    super::yard(&mut cv, &run);
    super::super::lessons::figures(&mut cv, &run);
    super::super::figures::figures(&mut cv, &run);
    board(&mut cv, &run, Some(1));
    save("squires-yard.png", cv);
    let seat = super::super::seat_scene(
        &run,
        super::super::NATIVE_W,
        super::super::NATIVE_H + super::super::party::BAND_H + super::super::party::HUD_H,
        Some(1),
    )
    .compose();
    save("squires-yard-seat.png", seat);
    run.treasury.add(crate::realm::Spoil::Gold, 1500);
    on_plate(&mut run, 1, 1);
    run.squire_hall.holds.insert(1, 1);
    run.players.get_mut(&1).unwrap().buying = crate::rules::home::BUY_HOLD / 2;
    save(
        "squires-yard-hire-ready.png",
        super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
    );
    stocked(&mut run, 3, 2);
    let key = Home::knight_key(&run.players[&1]);
    for (s, name) in run
        .home
        .squire_hall
        .rosters
        .get_mut(&key)
        .unwrap()
        .squires
        .iter_mut()
        .zip(["Pip", "Rowan", "Fern"])
    {
        s.name = name.into();
    }
    for (name, plate) in [
        ("squires-yard-select.png", 3),
        ("squires-yard-train.png", 6),
        ("squires-yard-upgrade-recruit.png", 2),
        ("squires-yard-upgrade-hiring.png", 5),
        ("squires-yard-upgrade-train.png", 8),
    ] {
        on_plate(&mut run, 1, plate);
        run.squire_hall.holds.insert(1, plate);
        save(
            name,
            super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
        );
    }
    let active = &mut run.home.squire_hall.rosters.get_mut(&key).unwrap().squires[0];
    active.kind = Kind::Shield;
    on_plate(&mut run, 1, 6);
    save(
        "squires-yard-train-shield.png",
        super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
    );
    run.home.squire_hall.hire = 3;
    stocked(&mut run, 9, 3);
    on_plate(&mut run, 1, 3);
    save(
        "squires-yard-roster-full.png",
        super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
    );
    on_plate(&mut run, 1, 6);
    save(
        "squires-yard-train-cap.png",
        super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
    );
    on_plate(&mut run, 1, 5);
    save(
        "squires-yard-upgrade-complete.png",
        super::super::seat_scene(&run, 384, 296, Some(1)).compose(),
    );

    let mut cv = Img::black(240, 120);
    for owner in 1..=6 {
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            let x = 20 + i as i32 * 44;
            let y = owner as i32 * 18;
            stand(&mut cv, &figure(kind, owner), x, y);
        }
    }
    save("squires-types-owners.png", cv);
}

#[test]
fn inactive_owners_squires_are_never_drawn_after_a_full_step() {
    use crate::rules::{Run, squire_hall::Companion};
    let mut run = super::super::testing::fight();
    run.join_seat(2, "Still here");
    let body = crate::rules::hireling::Hireling {
        kind: crate::rules::hireling::AllyKind::Beaumains,
        x: 12.0,
        y: 12.0,
        hp: 80,
        max_hp: 80,
        facing: 1.0,
        side: 1.0,
        cooldown: 0,
        invulnerable: 0,
        summoner: None,
        power: 0,
        taunt: 0,
        rising: 0,
            made: None,
    };
    run.squire_hall.companions.push(Companion {
        owner: 1,
        squire: crate::realm::squires::Squire {
            id: 1,
            name: "Pip".into(),
            kind: Kind::Ranger,
            level: 1,
            talent: 1,
        },
        body,
        aid: 0,
        smoke: 0,
        mark: None,
    });
    let blank = Img::black(384, 224).rgba_scaled(1);
    let mut visible = Img::black(384, 224);
    delve(&mut visible, &run);
    assert_ne!(visible.rgba_scaled(1), blank);
    for state in 0..3 {
        let mut hidden: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
        match state {
            0 => hidden.set_absent(1, true),
            1 => {
                hidden.exits.outside.insert(
                    1,
                    crate::rules::exits::Entry {
                        at: hidden.at,
                        party: Default::default(),
                    },
                );
            }
            _ => {
                hidden.players.remove(&1);
            }
        }
        hidden.step(&Default::default());
        let mut cv = Img::black(384, 224);
        delve(&mut cv, &hidden);
        assert_eq!(cv.rgba_scaled(1), blank, "inactive state {state}");
    }
}
