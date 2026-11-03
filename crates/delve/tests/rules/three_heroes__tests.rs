use super::*;

fn arena(kit: &str) -> Run {
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
    assert!(r.take_up(1, kit), "new kit {kit} must be selectable");
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.invulnerable = 1000;
    for x in [17.0, 20.0, 23.0] {
        r.spawn_at_for_test(EnemyKind::Demon, x, 14.0);
        let e = r.enemies.last_mut().unwrap();
        e.hp = 10000;
        e.max_hp = 10000;
        e.seen_hp = 10000;
        e.age = TELEGRAPH;
    }
    r.phase = Phase::Fighting;
    r
}
fn step(r: &mut Run, input: Input) {
    r.step(&BTreeMap::from([(1, input)]));
}
fn cast(r: &mut Run, key: u8) {
    step(
        r,
        Input {
            cast: key,
            ..Default::default()
        },
    );
}

#[test]
fn three_heroes_have_private_cards_unique_ults_and_safe_old_saves() {
    let mut r = arena("farwatch");
    for kit in ["farwatch", "galeweaver", "cinderheart"] {
        assert!(r.take_up(1, kit));
        let h = &r.players[&1];
        assert_eq!(h.spells.iter().flatten().count(), 3);
        for spell in h.spells.iter().flatten() {
            let c = r.book.get(spell).unwrap();
            assert_eq!((c.drop, c.chest), (0, 0));
        }
    }
    let ults: std::collections::BTreeSet<_> = ["farwatch", "galeweaver", "cinderheart"]
        .map(|k| format!("{:?}", ults::Ult::of(Some(k))))
        .into();
    assert_eq!(ults.len(), 3);
    assert!(r.take_up(1, "percival"));
    assert!(
        r.players[&1]
            .spells
            .iter()
            .flatten()
            .all(|s| !s.starts_with("gale-") && !s.starts_with("ember-") && !s.starts_with("far-"))
    );
}

#[test]
fn three_heroes_powershot_charges_then_pierces_on_release() {
    let mut r = arena("galeweaver");
    cast(&mut r, 1);
    assert!(
        r.enemies.iter().all(|e| e.hp == e.max_hp),
        "charge is not an instant bolt"
    );
    for _ in 0..24 {
        cast(&mut r, 1);
    }
    step(&mut r, Input::default());
    assert!(
        r.enemies.iter().all(|e| e.hp < e.max_hp),
        "released shot pierces every aligned foe"
    );
    let charged = r.enemies[0].max_hp - r.enemies[0].hp;
    let mut tap = arena("galeweaver");
    cast(&mut tap, 1);
    step(&mut tap, Input::default());
    assert!(charged > tap.enemies[0].max_hp - tap.enemies[0].hp);
}

#[test]
fn three_heroes_shackle_roots_exactly_two_without_stunning_them() {
    let mut r = arena("galeweaver");
    cast(&mut r, 2);
    let positions: Vec<_> = r.enemies.iter().map(|e| (e.x, e.y)).collect();
    for _ in 0..12 {
        step(&mut r, Input::default());
    }
    assert_eq!((r.enemies[0].x, r.enemies[0].y), positions[0]);
    assert_eq!((r.enemies[1].x, r.enemies[1].y), positions[1]);
    assert_ne!((r.enemies[2].x, r.enemies[2].y), positions[2]);
    assert_eq!(r.enemies[0].frozen, 0, "root is not a stun");
}

#[test]
fn three_heroes_windrun_evades_and_focus_really_accelerates_attacks() {
    let mut r = arena("galeweaver");
    cast(&mut r, 3);
    let h = r.players.get_mut(&1).unwrap();
    h.invulnerable = 0;
    let hp = h.hp;
    h.attack_hurt(20);
    assert_eq!(h.hp, hp, "windrun evades weapon damage");
    let base = h.cooldown(60);
    h.ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    assert!(r.players[&1].cooldown(60) < base / 2);
}

#[test]
fn three_heroes_fire_pillar_is_delayed_and_cast_stacks_expire() {
    let mut r = arena("cinderheart");
    let base = r.players[&1].cooldown(60);
    cast(&mut r, 2);
    assert_eq!(r.enemies[0].hp, 10000);
    assert!(r.players[&1].cooldown(60) < base);
    for _ in 0..30 {
        step(&mut r, Input::default());
    }
    assert!(r.enemies[0].hp < 10000);
    assert!(r.enemies[0].frozen > 0);
    for _ in 0..400 {
        step(&mut r, Input::default());
    }
    assert_eq!(r.players[&1].cooldown(60), base);
}

#[test]
fn three_heroes_rifle_fourth_shot_crits_and_prediction_matches() {
    let mut r = arena("farwatch");
    let mut shots = Vec::new();
    let h = r.players.get_mut(&1).unwrap();
    let mut damage = Vec::new();
    for _ in 0..4 {
        h.loose(1, &r.book, false, 0, 0, &mut shots);
        damage.push(shots.last().unwrap().damage);
        assert!(h.fire_cooldown >= 40);
        assert!(shots.last().unwrap().ttl >= 25);
    }
    assert_eq!(damage, [damage[0], damage[0], damage[0], damage[0] * 2]);
    let mut predicted = r.clone();
    r.projectiles.clear();
    predicted.projectiles.clear();
    r.players.get_mut(&1).unwrap().fire_cooldown = 0;
    predicted.players.get_mut(&1).unwrap().fire_cooldown = 0;
    let input = Input {
        fire: true,
        aim_x: 1,
        ..Default::default()
    };
    let mut local = Vec::new();
    let tick = r.tick + 1;
    assert!(predicted.predict(1, input));
    assert!(predicted.predict_fire(1, input, tick, &mut local));
    step(&mut r, input);
    let server = r.projectiles.iter().find(|p| !p.hostile).unwrap();
    assert_eq!(
        (local[0].damage, local[0].vx, local[0].ttl),
        (server.damage, server.vx, server.ttl + 1)
    );
    assert_eq!(
        r.players[&1].fire_cooldown,
        predicted.players[&1].fire_cooldown
    );
}

#[test]
fn three_heroes_windrun_prediction_matches_speed_and_hazards_still_hurt() {
    let mut r = arena("galeweaver");
    let mut predicted = r.clone();
    for tick in 0..5 {
        let input = Input {
            cast: if tick == 0 { 3 } else { 0 },
            move_x: 1,
            ..Default::default()
        };
        assert!(predicted.predict(1, input));
        assert!(predicted.predict_fire(1, input, r.tick + 1, &mut Vec::new()));
        step(&mut r, input);
        assert!((r.players[&1].x - predicted.players[&1].x).abs() < 0.0001);
        assert_eq!(
            r.players[&1].hero_kit.windrun,
            predicted.players[&1].hero_kit.windrun
        );
    }
    let h = r.players.get_mut(&1).unwrap();
    h.invulnerable = 0;
    let hp = h.hp;
    h.hurt(5);
    assert!(h.hp < hp, "windrun is evasion, not immunity to fire/poison");
    let mut plain = arena("galeweaver");
    let x = plain.players[&1].x;
    step(
        &mut plain,
        Input {
            move_x: 1,
            ..Default::default()
        },
    );
    assert!(r.players[&1].x - 12.0 > 5.0 * (plain.players[&1].x - x));
}

#[test]
fn three_heroes_charge_prediction_and_focus_lock_are_deterministic() {
    let mut r = arena("galeweaver");
    let mut local = r.clone();
    for n in 0..30 {
        let input = Input {
            cast: if n < 25 { 1 } else { 0 },
            ..Default::default()
        };
        local.predict(1, input);
        local.predict_fire(1, input, r.tick + 1, &mut Vec::new());
        step(&mut r, input);
        assert_eq!(
            r.players[&1].hero_kit.charging,
            local.players[&1].hero_kit.charging
        );
        if r.players[&1].hero_kit.charging {
            assert_eq!(
                r.players[&1].hero_kit.charge,
                local.players[&1].hero_kit.charge
            );
        }
    }
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    let target = r.players[&1].hero_kit.target.unwrap();
    r.enemies[0].y += 2.0;
    let mut h = r.players[&1].clone();
    heroes::focus_aim(&mut h, &r.enemies);
    let e = r.enemies.iter().find(|e| e.id == target).unwrap();
    let aim = unit(e.x - h.x, e.y - h.y);
    assert_eq!((h.aim_x, h.aim_y), aim);
    r.enemies.iter_mut().find(|e| e.id == target).unwrap().hp = 0;
    heroes::focus_aim(&mut h, &r.enemies);
    assert_eq!(h.hero_kit.focus, 0);
}

#[test]
fn three_heroes_wave_hits_many_verdict_hits_one_and_casts_stack_to_a_cap() {
    let mut r = arena("cinderheart");
    cast(&mut r, 1);
    assert!(r.enemies.iter().all(|e| e.hp < 10000));
    let rate = r.players[&1].cooldown(100);
    for _ in 0..8 {
        r.players.get_mut(&1).unwrap().spell_cooldowns[2] = 0;
        cast(&mut r, 3);
    }
    assert_eq!(r.players[&1].hero_kit.stacks, 5);
    assert!(r.players[&1].cooldown(100) < rate);
    r.enemies.iter_mut().for_each(|e| e.hp = 10000);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    assert_eq!(r.enemies.iter().filter(|e| e.hp < 10000).count(), 1);
}

#[test]
fn three_heroes_scope_and_mark_require_line_of_sight_and_shrapnel_slows() {
    let mut r = arena("farwatch");
    let mut plain = r.clone();
    cast(&mut r, 2);
    step(&mut plain, Input::default());
    let a = r.enemies[0].x;
    let b = plain.enemies[0].x;
    step(&mut r, Input::default());
    step(&mut plain, Input::default());
    assert!((r.enemies[0].x - a).abs() < (plain.enemies[0].x - b).abs());
    cast(&mut r, 1);
    assert!(r.enemies[0].hp < 10000);
    assert_eq!(r.enemies[1].hp, 10000);
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    let hp = r.enemies[0].hp;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    assert_eq!(r.enemies[0].hp, hp);
    // Remove shrapnel so only the delayed mark can damage this target.
    r.heroes.zones.clear();
    for _ in 0..HZ {
        step(&mut r, Input::default());
    }
    assert!(r.enemies[0].hp + 250 < hp);
}

#[test]
fn three_heroes_legacy_defaults_roundtrip_and_kit_cleanup() {
    let mut r = arena("galeweaver");
    cast(&mut r, 2);
    cast(&mut r, 3);
    let tree = crate::wire::publish(&r).unwrap();
    let back = crate::wire::read(&tree).unwrap();
    assert_eq!(back.heroes, r.heroes);
    assert_eq!(back.players[&1].hero_kit, r.players[&1].hero_kit);
    let mut json = serde_json::to_value(&r).unwrap();
    json.as_object_mut().unwrap().remove("heroes");
    for h in json["players"].as_object_mut().unwrap().values_mut() {
        h.as_object_mut().unwrap().remove("hero_kit");
    }
    let legacy: Run = serde_json::from_value(json).unwrap();
    assert!(legacy.heroes.is_empty());
    assert!(legacy.players[&1].hero_kit.is_empty());
    assert!(r.take_up(1, "percival"));
    assert!(r.heroes.roots.is_empty());
    assert!(r.players[&1].hero_kit.is_empty());
}

#[test]
fn three_heroes_pair_requires_two_and_breaks_when_either_dies() {
    let mut r = arena("galeweaver");
    r.enemies.truncate(1);
    cast(&mut r, 2);
    assert!(r.heroes.roots.is_empty());
    let mut r = arena("galeweaver");
    cast(&mut r, 2);
    assert_eq!(r.heroes.roots.len(), 1);
    let id = r.heroes.roots[0].b;
    r.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0;
    step(&mut r, Input::default());
    assert!(r.heroes.roots.is_empty());
}

#[test]
fn three_heroes_scope_cannot_shoot_through_stone_and_room_change_clears_fields() {
    let mut r = arena("farwatch");
    let room = &mut r.dungeon.rooms[r.at];
    for row in 1..room.rows - 1 {
        room.set_for_test(7, row, Tile::Wall);
    }
    cast(&mut r, 1);
    assert!(r.enemies.iter().all(|e| e.hp == e.max_hp));
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    assert!(r.heroes.marks.is_empty());
    assert_eq!(r.players[&1].ult_charge, ults::ULT_FULL);
    let mut r = arena("galeweaver");
    cast(&mut r, 2);
    cast(&mut r, 3);
    assert!(!r.heroes.is_empty());
    let at = r.at;
    r.enter_for_test(at);
    assert!(r.heroes.is_empty());
    assert!(r.players[&1].hero_kit.is_empty());
}

#[test]
fn three_heroes_fields_patch_into_the_browser_without_full_decode_fallback() {
    use crate::wire::{Guest, Publisher, Shots};
    use angel_net::doc::delta::diff;
    let mut r = arena("cinderheart");
    let mut pubr = Publisher::default();
    let mut guest = Guest::default();
    let before = pubr.publish(&r).unwrap();
    guest.read(&before, None, &Shots::Rebuild).unwrap();
    cast(&mut r, 2);
    let tree = pubr.publish(&r).unwrap();
    let patch = diff(&before, &tree).unwrap();
    let fields = Guest::fields(&patch).unwrap();
    let shots = Guest::shots_of(&patch);
    let back = guest.read(&tree, Some(&fields), &shots).unwrap();
    assert_eq!(back.heroes, r.heroes);
    assert_eq!(back.players[&1].hero_kit, r.players[&1].hero_kit);
    assert_eq!(guest.stats().2, "");
}

#[test]
fn three_heroes_focus_fires_more_shots_and_cast_start_suppresses_basic_fire() {
    let mut r = arena("galeweaver");
    step(
        &mut r,
        Input {
            cast: 1,
            fire: true,
            ..Default::default()
        },
    );
    assert!(r.projectiles.iter().all(|p| p.hostile));
    step(
        &mut r,
        Input {
            cast: 1,
            fire: true,
            ..Default::default()
        },
    );
    assert!(r.projectiles.iter().all(|p| p.hostile));
    let mut r = arena("galeweaver");
    let mut normal = r.clone();
    r.players.get_mut(&1).unwrap().ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Default::default()
        },
    );
    step(&mut normal, Input::default());
    for _ in 0..90 {
        let input = Input {
            fire: true,
            aim_x: 1,
            ..Default::default()
        };
        step(&mut r, input);
        step(&mut normal, input);
    }
    assert!(r.players[&1].hero_kit.shots > normal.players[&1].hero_kit.shots * 2);
}

#[test]
fn three_heroes_max_charge_auto_releases_and_replays_identically() {
    let mut r = arena("galeweaver");
    let mut replay = r.clone();
    for _ in 0..heroes::CHARGE_MAX + 2 {
        let input = Input {
            cast: 1,
            ..Default::default()
        };
        step(&mut r, input);
        step(&mut replay, input);
    }
    assert!(!r.players[&1].hero_kit.charging);
    assert!(r.enemies.iter().all(|e| e.hp < e.max_hp));
    assert_eq!(
        crate::wire::publish(&r).unwrap(),
        crate::wire::publish(&replay).unwrap()
    );
}

#[test]
fn three_heroes_learned_spells_also_stack_and_dead_owners_leave_no_fields() {
    let mut r = arena("cinderheart");
    // Stacking is a cast passive, not an alias for the three bespoke spells.
    r.players.get_mut(&1).unwrap().spells[3] = Some("cabbage-storm".into());
    let before = r.players[&1].cooldown(100);
    cast(&mut r, 4);
    assert_eq!(r.players[&1].hero_kit.stacks, 1);
    assert!(r.players[&1].cooldown(100) < before);
    cast(&mut r, 2);
    assert!(!r.heroes.pillars.is_empty());
    r.players.get_mut(&1).unwrap().hp = 0;
    step(&mut r, Input::default());
    assert!(r.heroes.pillars.is_empty());
    assert!(r.players[&1].hero_kit.is_empty());
}

#[test]
fn three_heroes_borrowed_last_light_keeps_a_living_non_kit_owner() {
    let mut r = arena("percival");
    r.enemies.truncate(1);
    r.enemies[0].hp = 10;
    let target = r.enemies[0].id;
    assert!(ults::Ult::ALL.contains(&ults::Ult::LastLight));
    let h = r.players.get_mut(&1).unwrap();
    // All Random can lend any ultimate, independently of the knight's kit.
    h.ult = Some(ults::Ult::LastLight);
    h.ult_charge = ults::ULT_FULL;
    step(
        &mut r,
        Input {
            ult: true,
            ..Input::default()
        },
    );
    assert_eq!(
        r.heroes.marks.len(),
        1,
        "a living Percival keeps his borrowed mark"
    );
    assert_eq!(r.heroes.marks[0].owner, 1);
    assert!(
        r.enemies
            .iter()
            .find(|e| e.id == target)
            .map_or(0, |e| e.hp)
            > 0,
        "Last Light still waits before finishing"
    );
    for _ in 0..HZ {
        step(&mut r, Input::default());
    }
    assert_eq!(
        r.enemies
            .iter()
            .find(|e| e.id == target)
            .map_or(0, |e| e.hp),
        0,
        "the borrowed mark must finish"
    );
    assert!(r.heroes.marks.is_empty());
}

fn gale_with_retained_spell() -> Run {
    let mut r = arena("percival");
    let learned = r.book.get("the-imposture").unwrap().clone();
    assert!(!knights::kit_spell(&learned.id));
    assert!(r.players.get_mut(&1).unwrap().equip_spell(&learned));
    assert_eq!(r.players[&1].spells[0].as_deref(), Some("the-imposture"));
    assert!(r.take_up(1, "galeweaver"));
    assert_eq!(r.players[&1].spells[0].as_deref(), Some("the-imposture"));
    assert_eq!(r.players[&1].spells[1].as_deref(), Some("gale-power"));
    r
}

#[test]
fn three_heroes_retained_spell_moves_power_to_slot_two_in_host_and_prediction() {
    let mut r = gale_with_retained_spell();
    r.enemies.truncate(1);
    r.enemies[0].hp = 500;
    let target = r.enemies[0].id;
    let mut p = r.clone();
    for tick in 0..10 {
        let input = Input {
            cast: 2,
            aim_x: 1,
            ..Input::default()
        };
        assert!(p.predict(1, input));
        let mut shots = Vec::new();
        assert!(p.predict_fire(1, input, r.tick, &mut shots));
        step(&mut r, input);
        assert!(
            r.players[&1].hero_kit.charging,
            "host releases held slot 2 on tick {tick}"
        );
        assert!(
            p.players[&1].hero_kit.charging,
            "prediction releases held slot 2 on tick {tick}"
        );
        assert_eq!(r.players[&1].hero_kit.charge, tick + 1);
        assert_eq!(r.players[&1].hero_kit, p.players[&1].hero_kit);
        assert_eq!(r.players[&1].spell_cooldowns, p.players[&1].spell_cooldowns);
        assert!(shots.is_empty());
        assert!(r.heroes.beams.is_empty());
        assert_eq!(
            r.enemies
                .iter()
                .find(|e| e.id == target)
                .map_or(0, |e| e.hp),
            500
        );
    }
    // Slot 1 is the retained spell, not a key that can keep Power charging.
    let release = Input {
        cast: 1,
        ..Input::default()
    };
    assert!(p.predict(1, release));
    assert!(p.predict_fire(1, release, r.tick, &mut Vec::new()));
    step(&mut r, release);
    assert!(!r.players[&1].hero_kit.charging);
    assert_eq!(r.players[&1].hero_kit.charge, 0);
    assert_eq!(r.players[&1].hero_kit, p.players[&1].hero_kit);
    assert!(
        r.enemies
            .iter()
            .find(|e| e.id == target)
            .map_or(0, |e| e.hp)
            < 500,
        "releasing the actual slot fires Power"
    );
    assert!(
        r.players[&1].spell_cooldowns[0] > 0,
        "the retained spell still casts normally"
    );
}

#[test]
fn three_heroes_desertion_refuses_stance_spells_in_host_and_prediction() {
    for kit in heroes::KITS {
        for slot in 1..=3 {
            let mut r = arena(kit);
            let h = r.players.get_mut(&1).unwrap();
            h.ult = Some(ults::Ult::Desertion);
            h.ult_charge = ults::ULT_FULL;
            step(
                &mut r,
                Input {
                    ult: true,
                    ..Input::default()
                },
            );
            assert!(r.players[&1].flee_ticks > 0);
            let mut p = r.clone();
            let input = Input {
                cast: slot,
                ..Input::default()
            };
            for _ in 0..2 {
                assert!(p.predict(1, input));
                assert!(p.predict_fire(1, input, r.tick, &mut Vec::new()));
                step(&mut r, input);
                let host = &r.players[&1];
                let predicted = &p.players[&1];
                assert!(host.flee_ticks > 0);
                assert!(host.hero_kit.is_empty(), "host {kit} slot {slot}");
                assert_eq!(
                    predicted.hero_kit, host.hero_kit,
                    "Desertion {kit} slot {slot}"
                );
                assert_eq!(predicted.spell_cooldowns, host.spell_cooldowns);
                assert!(host.spell_cooldowns.iter().all(|cd| *cd == 0));
            }
        }
    }
}
