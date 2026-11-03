use super::*;
use crate::realm::{Spoil, Spoils};

// Review regressions deliberately exercise runtime effects, not source text.
fn train_to(r: &mut Run, level: u8) {
    let key = owner_key(&r.players[&1]);
    r.home.squires_lodge.rosters.get_mut(&key).unwrap().members[0].level = level;
    r.sync_squires();
}

#[test]
fn review_duplicate_presence_transfers_the_existing_body_without_reset() {
    for absent in [false, true] {
        let mut r = fight(Kind::Ranger);
        let name = format!("  {}  ", r.players[&1].name.to_uppercase());
        r.join_seat(2, &name);
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        s.body.hp = 31;
        s.body.cooldown = 47;
        let h = r.players.get_mut(&1).unwrap();
        h.absent = absent;
        h.away = !absent;
        r.sync_squires();
        assert!(r.players[&1].squire.is_none());
        let s = r.players[&2]
            .squire
            .as_ref()
            .expect("present duplicate owns the body");
        assert_eq!((s.body.hp, s.body.cooldown), (31, 47));
        r.players.get_mut(&1).unwrap().absent = false;
        r.players.get_mut(&1).unwrap().away = false;
        r.sync_squires();
        assert_eq!(r.players.values().filter(|h| h.squire.is_some()).count(), 1);
        let s = r.players.values().find_map(|h| h.squire.as_ref()).unwrap();
        assert_eq!((s.body.hp, s.body.cooldown), (31, 47));
    }
}

#[test]
fn review_detached_owner_reserves_one_body_across_personal_exit_and_reentry() {
    let mut r = fight(Kind::Ranger);
    let name = r.players[&1].name.clone();
    r.join_seat(2, &name);
    let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
    s.body.hp = 29;
    s.body.cooldown = 43;
    // Use the real optional-activity exit and re-entry, not a new body.
    r.dungeon.rooms[r.at].kind = RoomKind::Ledge;
    assert!(r.leave_activity(1));
    assert!(r.exits.outside.contains_key(&1));
    let detached = r.detached_seats(&BTreeMap::new());
    assert!(detached[&1].squire.is_some());
    r.tick_squires();
    assert!(
        r.players[&2].squire.is_none(),
        "detached body must not be cloned"
    );
    r.players.extend(detached);
    r.rejoin(1);
    r.sync_squires();
    assert_eq!(r.players.values().filter(|h| h.squire.is_some()).count(), 1);
    let s = r.players[&1].squire.as_ref().unwrap();
    assert_eq!((s.body.hp, s.body.cooldown), (29, 43));
}

#[test]
fn review_duplicate_personal_menu_and_split_save_do_not_mint_a_second_body() {
    for menu in [false, true] {
        let mut r = fight(Kind::Ranger);
        let name = r.players[&1].name.clone();
        r.join_seat(2, &name);
        r.players
            .get_mut(&1)
            .unwrap()
            .squire
            .as_mut()
            .unwrap()
            .body
            .hp = 27;
        r.dungeon.rooms[r.at].kind = RoomKind::Ledge;
        if !menu {
            assert!(r.leave_activity(1));
        }
        let inputs = BTreeMap::from([(
            1,
            Input {
                activity_menu: menu,
                ..Default::default()
            },
        )]);
        for _ in 0..3 {
            r.step(&inputs);
            assert_eq!(r.players.values().filter(|h| h.squire.is_some()).count(), 1);
            assert_eq!(r.players[&1].squire.as_ref().unwrap().body.hp, 27);
            assert!(r.players[&2].squire.is_none());
        }
        let mut restored: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        restored.step(&inputs);
        assert_eq!(
            restored
                .players
                .values()
                .filter(|h| h.squire.is_some())
                .count(),
            1
        );
        assert_eq!(restored.players[&1].squire.as_ref().unwrap().body.hp, 27);
        if !menu {
            restored.rejoin(1);
        }
        restored.step(&BTreeMap::new());
        assert_eq!(
            restored
                .players
                .values()
                .filter(|h| h.squire.is_some())
                .count(),
            1
        );
        assert_eq!(restored.players[&1].squire.as_ref().unwrap().body.hp, 27);
    }
}

#[test]
fn review_training_unlocks_real_kind_abilities() {
    for level in 1..=3 {
        let mut r = fight(Kind::Ninja);
        train_to(&mut r, level);
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        s.body.hp = 10;
        foe(&mut r, 20.0, 12.0);
        r.tick_squires();
        assert_eq!(
            r.players[&1].squire.as_ref().unwrap().vanished > 0,
            level >= 3
        );
        assert_eq!(
            !r.projectiles.is_empty(),
            level >= 2,
            "stars require training"
        );

        let mut r = fight(Kind::Healer);
        train_to(&mut r, level);
        let h = r.players.get_mut(&1).unwrap();
        h.hp = 10;
        h.hexed = 90;
        r.tick_squires();
        assert!(r.players[&1].hp > 10, "basic healing is always available");
        assert_eq!(r.players[&1].hexed == 0, level >= 2);

        let mut r = fight(Kind::Ranger);
        train_to(&mut r, level);
        foe(&mut r, 20.0, 12.0);
        r.tick_squires();
        assert!(
            !r.projectiles.is_empty(),
            "basic arrows are always available"
        );
        assert_eq!(
            r.players[&1].squire.as_ref().unwrap().mark.is_some(),
            level >= 2
        );

        let mut r = fight(Kind::Shieldbearer);
        train_to(&mut r, level);
        let s = &r.players[&1].squire.as_ref().unwrap().body;
        let at = (s.x, s.y);
        let i = foe(&mut r, at.0 + 1.0, at.1);
        r.enemies[i].frozen = 0;
        r.tick_squires();
        assert!(r.enemies[i].hp < 1000, "basic melee is always available");
        assert_eq!(r.enemies[i].frozen > 0, level >= 2, "bash is trained");

        let mut r = fight(Kind::Alchemist);
        train_to(&mut r, level);
        r.players.get_mut(&1).unwrap().hp = 10;
        foe(&mut r, 20.0, 12.0);
        r.tick_squires();
        let shot = r
            .projectiles
            .last()
            .expect("basic bombs are always available");
        assert_eq!(shot.kind, Shot::Ball);
        assert_eq!(shot.traits.burst > 0, level >= 2, "splash is trained");
        assert_eq!(r.players[&1].hp > 10, level >= 3, "potions are trained");
    }
}

#[test]
fn review_support_healing_uses_stakes_scaling_budget_and_accounting() {
    use super::super::stakes::{HEALED, Mend, Stakes};
    for kind in [Kind::Healer, Kind::Alchemist] {
        for stakes in [Stakes::Fight, Stakes::Boss] {
            for count in [1, 6] {
                let mut r = fight(kind);
                train_to(&mut r, 3);
                let name = r.players[&1].name.clone();
                let template = r.home.squires_lodge.rosters[&owner_key(&r.players[&1])].clone();
                for id in 2..=count {
                    r.join_seat(id, &format!("{name} {id}"));
                    let key = owner_key(&r.players[&id]);
                    r.home.squires_lodge.rosters.insert(key, template.clone());
                    let h = r.players.get_mut(&id).unwrap();
                    (h.x, h.y) = (12.0, 12.0);
                }
                r.sync_squires();
                for h in r.players.values_mut() {
                    h.stakes = stakes;
                    h.hp = h.max_hp / 2;
                }
                let mut expected = r.players[&1].clone();
                expected.squire = None;
                let raw = if kind == Kind::Healer { 21 } else { 32 };
                let gain = expected.mend(raw, Mend::Passive);
                HEALED.with(|h| *h.borrow_mut() = [0; 4]);
                let before: u32 = r.players.values().map(|h| h.hp).sum();
                r.tick_squires();
                let after: u32 = r.players.values().map(|h| h.hp).sum();
                assert_eq!(
                    after - before,
                    gain * count,
                    "{kind:?} {stakes:?} count={count}"
                );
                HEALED.with(|h| assert_eq!(h.borrow()[Mend::Passive as usize], after - before));
                // All supports share the recipient's existing per-second budget.
                for h in r.players.values_mut() {
                    h.passive = 1000;
                    h.squire.as_mut().unwrap().support = 0;
                }
                r.tick_squires();
                assert_eq!(r.players.values().map(|h| h.hp).sum::<u32>(), after);
            }
        }
    }
}

#[test]
fn review_fast_shot_meets_nearest_squire_before_hero() {
    for kind in Kind::ALL {
        let mut r = fight(kind);
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (18.0, 12.0);
        let s = h.squire.as_mut().unwrap();
        (s.body.x, s.body.y) = (14.0, 12.0);
        let hp = h.hp;
        let body_hp = s.body.hp;
        r.projectiles.push(Projectile {
            x: 10.0,
            y: 12.0,
            vx: 600.0,
            vy: 0.0,
            hostile: true,
            look: None,
            kind: Shot::Arrow,
            damage: 20,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: HZ,
        });
        r.step(&BTreeMap::new());
        assert_eq!(
            r.players[&1].hp, hp,
            "{kind:?} must intercept before owner hit"
        );
        assert!(r.players[&1].squire.as_ref().unwrap().body.hp < body_hp);
    }
}

#[test]
fn review_wounded_patient_cannot_starve_trained_skull_escort() {
    for level in [1, 2, 3] {
        let mut r = fight(Kind::Healer);
        train_to(&mut r, level);
        r.join_seat(2, "fallen friend");
        r.tick = u64::from(2 * HZ);
        let h = r.players.get_mut(&2).unwrap();
        h.hp = 0;
        h.fell = 1;
        h.skull_at = Some((20.0, 12.0));
        foe(&mut r, 18.0, 12.0); // even retreat pressure cannot override the errand
        for _ in 0..180 {
            let h = r.players.get_mut(&1).unwrap();
            h.hp = 30;
            h.squire.as_mut().unwrap().support = HZ;
            r.tick_squires();
        }
        assert_eq!(r.players[&1].carrying_skull, (level >= 3).then_some(2));
        assert_eq!(r.players[&2].hp, 0, "escort must not bypass revival ritual");
    }
}

#[test]
fn review_backstab_requires_training_and_boosts_actual_melee_damage() {
    for level in [1, 2] {
        let mut r = fight(Kind::Ninja);
        train_to(&mut r, level);
        let i = foe(&mut r, 16.0, 12.0);
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        (s.body.x, s.body.y) = (18.0, 12.0);
        r.tick_squires();
        let multiplier = if level >= 2 { 3 } else { 1 };
        assert_eq!(
            1000 - r.enemies[i].hp,
            Kind::Ninja.damage(level) * multiplier
        );
        assert_eq!(r.cues.iter().any(|c| c == "squire:backstab"), level >= 2);
    }
}

#[test]
fn review_interception_orders_nearest_hero_and_companion_and_ignores_inactive_bodies() {
    for inactive in 0..6 {
        let mut r = fight(Kind::Ranger);
        r.join_seat(2, "unprotected witness");
        let h = r.players.get_mut(&1).unwrap();
        (h.x, h.y) = (18.0, 12.0);
        let s = h.squire.as_mut().unwrap();
        (s.body.x, s.body.y) = (14.0, 12.0);
        match inactive {
            0 => s.vanished = HZ,
            1 => s.body.hp = 0,
            2 => h.absent = true,
            3 => h.away = true,
            4 => s.owner = "stale identity".into(),
            _ => (s.body.x, s.body.y) = (22.0, 12.0), // hero is first
        }
        let h = r.players.get_mut(&2).unwrap();
        (h.x, h.y) = (18.0, 12.0);
        r.projectiles.push(Projectile {
            x: 10.0,
            y: 12.0,
            vx: 900.0,
            vy: 0.0,
            hostile: true,
            look: None,
            kind: Shot::Arrow,
            damage: 20,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: Default::default(),
            ttl: HZ,
        });
        // Direct authoritative flight avoids room recovery/rename sync changing
        // the deliberately inactive body before the interception test.
        let mut sortie = r.knights_turn(&BTreeMap::new());
        r.shots_fly(&mut sortie);
        assert!(
            r.players[&1].hp < 100 || r.players[&2].hp < 100,
            "inactive={inactive}"
        );
    }
    let mut r = fight(Kind::Ranger);
    let name = r.players[&1].name.clone();
    r.join_seat(2, "nearer squire");
    let mut second = r.players[&1].squire.clone().unwrap();
    second.owner = owner_key(&r.players[&2]);
    (second.body.x, second.body.y) = (14.0, 12.0);
    let second_hp = second.body.hp;
    let h = r.players.get_mut(&2).unwrap();
    (h.x, h.y) = (22.0, 12.0);
    h.squire = Some(second);
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y) = (22.0, 12.0);
    h.squire.as_mut().unwrap().body.x = 18.0;
    let first_hp = h.squire.as_ref().unwrap().body.hp;
    r.projectiles.push(Projectile {
        x: 10.0,
        y: 12.0,
        vx: 900.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Arrow,
        damage: 20,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    let mut sortie = r.knights_turn(&BTreeMap::new());
    r.shots_fly(&mut sortie);
    assert_eq!(r.players[&1].name, name);
    assert_eq!(r.players[&1].squire.as_ref().unwrap().body.hp, first_hp);
    assert_eq!(
        r.players[&2].squire.as_ref().unwrap().body.hp,
        second_hp - 20
    );
}

#[test]
fn review_training_and_body_clocks_roundtrip_and_replay_deterministically() {
    for kind in Kind::ALL {
        let mut r = fight(kind);
        train_to(&mut r, 3);
        foe(&mut r, 20.0, 12.0);
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        s.body.hp -= 17;
        s.body.cooldown = 21;
        s.special = 37;
        s.support = 19;
        let save = serde_json::to_vec(&r).unwrap();
        let mut restored: Run = serde_json::from_slice(&save).unwrap();
        assert_eq!(restored.players[&1].squire, r.players[&1].squire);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), save);
        for _ in 0..90 {
            r.step(&BTreeMap::new());
            restored.step(&BTreeMap::new());
        }
        assert_eq!(
            serde_json::to_vec(&restored).unwrap(),
            serde_json::to_vec(&r).unwrap()
        );
        assert_eq!(
            r.home.squires_lodge.rosters[&owner_key(&r.players[&1])].members[0].level,
            3
        );
        let summary = kind.ability_summary(3);
        for &(ability, level) in kind.abilities() {
            assert!(summary.contains(ability.name()));
            assert!(kind.unlocked(level, ability));
            assert!(!kind.unlocked(level - 1, ability));
        }
    }
}

#[test]
fn review_six_supports_share_one_wounded_recipient_budget() {
    for kind in [Kind::Healer, Kind::Alchemist] {
        for stakes in [stakes::Stakes::Fight, stakes::Stakes::Boss] {
            let mut r = fight(kind);
            train_to(&mut r, 3);
            let roster = r.home.squires_lodge.rosters[&owner_key(&r.players[&1])].clone();
            for id in 2..=6 {
                r.join_seat(id, &format!("support {id}"));
                r.home
                    .squires_lodge
                    .rosters
                    .insert(owner_key(&r.players[&id]), roster.clone());
            }
            r.sync_squires();
            for h in r.players.values_mut() {
                (h.x, h.y) = (12.0, 12.0);
                h.stakes = stakes;
                h.hp = h.max_hp;
                let s = h.squire.as_mut().unwrap();
                (s.body.x, s.body.y) = (13.0, 12.0);
            }
            r.players.get_mut(&1).unwrap().hp = 10;
            stakes::HEALED.with(|h| *h.borrow_mut() = [0; 4]);
            r.tick_squires();
            assert_eq!(r.players[&1].hp, 11, "{kind:?} {stakes:?}");
            stakes::HEALED.with(|h| assert_eq!(h.borrow()[stakes::Mend::Passive as usize], 1));
        }
    }
}

#[test]
fn review_paid_training_unlocks_selected_name_owned_members_without_healing_reset() {
    for kind in Kind::ALL {
        let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
        r.treasury.add(Spoil::Gold, 10_000);
        let recruit = r
            .squire_recruits(1)
            .into_iter()
            .find(|m| m.kind == kind)
            .unwrap();
        let id = recruit.id;
        r.squire_action(1, Action::Hire(id)).unwrap();
        r.squire_action(1, Action::Select(Some(id))).unwrap();
        let key = owner_key(&r.players[&1]);
        let h = r.players.get_mut(&1).unwrap();
        let s = h.squire.as_mut().unwrap();
        s.body.hp = 19;
        s.body.cooldown = 17;
        s.special = 29;
        let before = r.treasury.0.get(&Spoil::Gold).copied().unwrap_or(0);
        r.squire_action(1, Action::Train(id)).unwrap();
        let s = r.players[&1].squire.as_ref().unwrap();
        assert_eq!(s.member.level, 2);
        assert_eq!(s.body.max_hp, kind.health(2));
        assert_eq!((s.body.hp, s.body.cooldown, s.special), (19, 17, 29));
        assert_eq!(
            before - r.treasury.0.get(&Spoil::Gold).copied().unwrap_or(0),
            80
        );
        assert_eq!(r.home.squires_lodge.rosters[&key].members[0].level, 2);
        for &(ability, level) in kind.abilities() {
            if level == 2 {
                assert!(kind.unlocked(s.member.level, ability));
            }
            if level == 3 {
                assert!(!kind.unlocked(s.member.level, ability));
            }
        }
        r.join_seat(2, "different owner");
        assert!(r.players[&2].squire.is_none());
        assert_eq!(r.squire_action(2, Action::Train(id)), Err(Error::Missing));
        r.squire_action(1, Action::Upgrade(Building::Training))
            .unwrap();
        r.squire_action(1, Action::Train(id)).unwrap();
        let s = r.players[&1].squire.as_ref().unwrap();
        assert_eq!(s.member.level, 3);
        assert_eq!((s.body.hp, s.body.cooldown, s.special), (19, 17, 29));
        for &(ability, _) in kind.abilities() {
            assert!(kind.unlocked(s.member.level, ability));
        }
    }
}

#[test]
fn old_home_defaults_and_refresh_is_deterministic() {
    let home: home::Home = serde_json::from_str("{}").unwrap();
    assert_eq!(home.squires_lodge, Guild::default());
    let mut a = Guild::default();
    let mut b = Guild::default();
    assert_eq!(a.recruits("jordan"), b.recruits("jordan"));
    assert_eq!(
        a.recruits("jordan")
            .iter()
            .map(|r| r.kind)
            .collect::<Vec<_>>(),
        Kind::ALL
    );
    let first = a.recruits("jordan");
    a.advance_delve();
    b.advance_delve();
    assert_ne!(first, a.recruits("jordan"));
    assert_eq!(a.recruits("jordan"), b.recruits("jordan"));
    a.rent_day();
    assert_ne!(a.recruits("jordan"), b.recruits("jordan"));
}

#[test]
fn treasury_and_name_owned_roster_survive_seat_changes() {
    let mut run = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    let recruit = run.squire_recruits(1)[0].clone();
    assert_eq!(
        run.squire_action(1, Action::Hire(recruit.id)),
        Err(Error::Funds)
    );
    assert!(run.squire_roster(1).is_none());
    run.treasury.add(Spoil::Gold, 10_000);
    run.squire_action(1, Action::Hire(recruit.id)).unwrap();
    assert_eq!(run.squire_roster(1).unwrap().active, Some(recruit.id));
    run.squire_action(1, Action::Train(recruit.id)).unwrap();
    let name = run.players[&1].name.clone();
    run.join_seat(2, &name);
    run.players.get_mut(&1).unwrap().name = "Someone Else".into();
    assert!(run.squire_roster(1).is_none());
    assert_eq!(run.squire_roster(2).unwrap().members[0].level, 2);
    let saved = serde_json::to_string(&run.home).unwrap();
    let restored: home::Home = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.squires_lodge, run.home.squires_lodge);
}

fn fight(kind: Kind) -> Run {
    let mut r = hireling::testing::fight_with(false, &[]);
    let owner = owner_key(&r.players[&1]);
    let member = Member {
        id: 42,
        name: "Pip Reed".into(),
        kind,
        level: 1,
    };
    r.home.squires_lodge.rosters.insert(
        owner,
        Roster {
            members: vec![member],
            active: Some(42),
        },
    );
    r.sync_squires();
    let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
    (s.body.x, s.body.y, s.body.cooldown) = (14.0, 12.0, 0);
    r
}
fn foe(r: &mut Run, x: f32, y: f32) -> usize {
    r.spawn_at_for_test(EnemyKind::Bat, x, y);
    let i = r.enemies.len() - 1;
    let e = &mut r.enemies[i];
    (e.age, e.hp, e.max_hp, e.frozen) = (TELEGRAPH + 1, 1000, 1000, 10000);
    i
}

#[test]
fn independent_buildings_and_atomic_failures() {
    let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    r.treasury.add(Spoil::Gold, 5000);
    let before = r.treasury.get(Spoil::Gold);
    let offer = r.squire_recruits(1)[0].clone();
    r.squire_action(1, Action::Upgrade(Building::Hiring))
        .unwrap();
    assert!(r.home.squires_lodge.hire_price(offer.kind) < Guild::default().hire_price(offer.kind));
    assert_eq!((r.home.squires_lodge.recruiting, r.home.squires_lodge.training), (0, 0));
    assert_eq!(r.treasury.get(Spoil::Gold), before - 250);
    r.squire_action(1, Action::Hire(offer.id)).unwrap();
    r.squire_action(1, Action::Train(offer.id)).unwrap();
    let state = (r.home.clone(), r.treasury.clone());
    assert_eq!(
        r.squire_action(1, Action::Train(offer.id)),
        Err(Error::MaxLevel)
    );
    assert_eq!(
        r.squire_action(1, Action::Hire(offer.id)),
        Err(Error::AlreadyHired)
    );
    assert_eq!(
        r.squire_action(1, Action::Select(Some(123))),
        Err(Error::Missing)
    );
    assert_eq!((r.home.clone(), r.treasury.clone()), state);
    r.squire_action(1, Action::Upgrade(Building::Training))
        .unwrap();
    r.squire_action(1, Action::Train(offer.id)).unwrap();
    r.squire_action(1, Action::Upgrade(Building::Recruiting))
        .unwrap();
    assert_eq!(r.squire_recruits(1).len(), 6);
    r.squire_action(1, Action::Select(None)).unwrap();
    assert!(r.players[&1].squire.is_none());
}

#[test]
fn six_squires_coexist_with_global_hireling_and_minions_without_scaling() {
    let mut r = fight(Kind::Ranger);
    for id in 2..=MAX_PLAYERS {
        r.join_seat(id, &format!("knight {id}"));
    }
    let count = r.scaled();
    for id in 2..=MAX_PLAYERS {
        let owner = owner_key(&r.players[&id]);
        let member = Member {
            id: u64::from(id),
            name: format!("Squire {id}"),
            kind: Kind::ALL[(id as usize - 1) % 5],
            level: 1,
        };
        r.home.squires_lodge.rosters.insert(
            owner,
            Roster {
                active: Some(member.id),
                members: vec![member],
            },
        );
    }
    r.hireling = Some(Hireling::new(1, (10.0, 10.0)));
    let global = r.hireling.clone();
    let minions = r.minions.clone();
    r.sync_squires();
    assert_eq!(r.squire_views().len(), 6);
    assert_eq!(r.scaled(), count);
    assert_eq!(r.hireling, global);
    assert_eq!(r.minions, minions);
    let json = serde_json::to_string(&r).unwrap();
    let restored: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.squire_views(), r.squire_views());
    assert_eq!(
        r.squire_views()
            .iter()
            .map(|v| v.owner_ink)
            .collect::<Vec<_>>(),
        ['7', '1', 'A', '4', '2', 'c']
    );
    // Duplicate names cannot create two instances of the same companion.
    r.players.get_mut(&2).unwrap().name = r.players[&1].name.clone();
    r.sync_squires();
    assert!(r.players[&2].squire.is_none());
}

#[test]
fn healer_chooses_lowest_fraction_cleanses_and_keeps_distance() {
    let mut r = fight(Kind::Healer);
    train_to(&mut r, 2);
    r.join_seat(2, "wounded");
    {
        let h = r.players.get_mut(&1).unwrap();
        h.hp = 50;
        h.max_hp = 100;
    }
    {
        let h = r.players.get_mut(&2).unwrap();
        h.hp = 60;
        h.max_hp = 200;
        h.hexed = 99;
        h.chilled = 99;
        h.mired = 99;
        (h.x, h.y) = (15.0, 12.0);
    }
    foe(&mut r, 16.0, 12.0);
    let mut expected = r.players[&2].clone();
    expected.mend(18, stakes::Mend::Passive);
    r.tick_squires();
    assert_eq!(r.players[&1].hp, 50);
    let h = &r.players[&2];
    assert_eq!(h.hp, expected.hp);
    assert_eq!((h.hexed, h.chilled, h.mired), (0, 0, 0));
    assert!(r.players[&1].squire.as_ref().unwrap().body.x < 14.0);
}

#[test]
fn ranger_marks_and_repeated_arrows_are_stronger() {
    let mut r = fight(Kind::Ranger);
    train_to(&mut r, 2);
    let i = foe(&mut r, 20.0, 12.0);
    r.tick_squires();
    let first = r.projectiles.last().unwrap().damage;
    assert_eq!(r.projectiles.last().unwrap().kind, Shot::Arrow);
    assert_eq!(
        r.players[&1].squire.as_ref().unwrap().mark,
        Some(r.enemies[i].id)
    );
    r.players
        .get_mut(&1)
        .unwrap()
        .squire
        .as_mut()
        .unwrap()
        .body
        .cooldown = 0;
    r.tick_squires();
    assert!(r.projectiles.last().unwrap().damage > first);
}

#[test]
fn ninja_flanks_backstabs_throws_stars_and_vanishes() {
    let mut r = fight(Kind::Ninja);
    train_to(&mut r, 3);
    let i = foe(&mut r, 16.0, 12.0);
    // Owner west, ninja east: a true flanking backstab.
    {
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        s.body.x = 18.0;
    }
    r.tick_squires();
    assert_eq!(r.enemies[i].hp, 1000 - Kind::Ninja.damage(3) * 3);
    {
        let s = r.players.get_mut(&1).unwrap().squire.as_mut().unwrap();
        s.body.x = 10.0;
        s.body.cooldown = 0;
        s.body.hp = 20;
    }
    r.tick_squires();
    assert_eq!(r.projectiles.last().unwrap().kind, Shot::Blade);
    assert!(r.players[&1].squire.as_ref().unwrap().vanished > 0);
    let hp = r.players[&1].squire.as_ref().unwrap().body.hp;
    let s = &r.players[&1].squire.as_ref().unwrap().body;
    r.projectiles.push(Projectile {
        x: s.x,
        y: s.y,
        vx: 1.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Arrow,
        damage: 999,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    r.tick_squires();
    assert_eq!(r.players[&1].squire.as_ref().unwrap().body.hp, hp);
}

#[test]
fn shield_intercepts_owner_damage_and_physically_blocks_shots() {
    let mut r = fight(Kind::Shieldbearer);
    let h = r.players.get_mut(&1).unwrap();
    let mut plain = h.clone();
    plain.squire = None;
    h.hurt(20);
    plain.hurt(20);
    assert!(h.hp > plain.hp, "shield must intercept actual Hero damage");
    assert!(h.squire.as_ref().unwrap().body.hp < 160);
    let s = &r.players[&1].squire.as_ref().unwrap().body;
    r.projectiles.push(Projectile {
        x: s.x,
        y: s.y,
        vx: -1.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Arrow,
        damage: 20,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    r.step(&BTreeMap::new());
    assert!(
        r.projectiles.is_empty(),
        "authoritative flight consumes the blocked shot"
    );
}

#[test]
fn alchemist_lobs_explosive_bombs_and_makes_potions() {
    let mut r = fight(Kind::Alchemist);
    train_to(&mut r, 3);
    r.players.get_mut(&1).unwrap().hp = 30;
    foe(&mut r, 20.0, 12.0);
    r.tick_squires();
    let mut expected = Hero::new("expected", 0.0, 0.0);
    expected.hp = 30;
    expected.mend(32, stakes::Mend::Passive);
    assert_eq!(r.players[&1].hp, expected.hp);
    let shot = r.projectiles.last().unwrap();
    assert_eq!(shot.kind, Shot::Ball);
    assert_eq!(shot.traits.burst, 100);
    assert_eq!(shot.traits.owner, 1);
}

#[test]
fn downed_squire_sits_out_fight_recovers_room_and_floor_without_roster_loss() {
    let mut r = fight(Kind::Ranger);
    foe(&mut r, 20.0, 12.0);
    r.players
        .get_mut(&1)
        .unwrap()
        .squire
        .as_mut()
        .unwrap()
        .body
        .hp = 0;
    r.tick_squires();
    assert!(r.projectiles.is_empty());
    assert!(r.players[&1].squire.as_ref().unwrap().down());
    r.enemies.clear();
    r.tick_squires();
    assert!(r.players[&1].squire.as_ref().unwrap().body.hp > 0);
    r.players
        .get_mut(&1)
        .unwrap()
        .squire
        .as_mut()
        .unwrap()
        .body
        .hp = 0;
    r.squires_follow();
    assert!(r.players[&1].squire.as_ref().unwrap().body.hp > 0);
    r.dungeon.depth += 1;
    r.squires_follow();
    let s = r.players[&1].squire.as_ref().unwrap();
    assert_eq!(s.body.hp, s.body.max_hp);
    assert_eq!(r.squire_roster(1).unwrap().members.len(), 1);
}

#[test]
fn normal_step_fires_real_projectiles_that_damage_monsters() {
    let mut r = fight(Kind::Ranger);
    train_to(&mut r, 2);
    let i = foe(&mut r, 20.0, 12.0);
    let id = r.enemies[i].id;
    for _ in 0..2 * HZ {
        r.step(&BTreeMap::new());
    }
    assert!(r.enemies.iter().find(|e| e.id == id).unwrap().hp < 1000);
    assert!(r.cues.iter().any(|c| c == "squire:mark"));
}

#[test]
fn refresh_occurs_once_per_departure_not_floor_or_room() {
    let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    r.descend_for_test();
    assert_eq!(r.home.squires_lodge.delves, 1);
    let board = r.squire_recruits(1);
    r.hireling_follows();
    r.tick_squires();
    assert_eq!(r.squire_recruits(1), board);
}

#[test]
fn healer_escorts_a_skull_back_instead_of_resurrecting_or_teleporting_it() {
    let mut r = fight(Kind::Healer);
    train_to(&mut r, 3);
    r.join_seat(2, "fallen friend");
    r.tick = u64::from(2 * HZ);
    {
        let h = r.players.get_mut(&2).unwrap();
        h.hp = 0;
        h.fell = 1;
        h.skull_at = Some((20.0, 12.0));
    }
    for _ in 0..120 {
        r.tick_squires();
    }
    assert_eq!(r.players[&1].carrying_skull, Some(2));
    assert_eq!(
        r.players[&2].hp, 0,
        "the existing weak-monster ritual still matters"
    );
    assert!(r.cues.iter().any(|c| c == "squire:skull"));
}

#[test]
fn stale_recruits_and_dungeon_purchases_are_rejected_without_payment() {
    let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    r.treasury.add(Spoil::Gold, 5000);
    let id = r.squire_recruits(1)[0].id;
    r.home.squires_lodge.rent_day();
    assert_eq!(r.squire_action(1, Action::Hire(id)), Err(Error::Missing));
    assert_eq!(r.treasury.get(Spoil::Gold), 5000);
    let id = r.squire_recruits(1)[0].id;
    r.squire_action(1, Action::Hire(id)).unwrap();
    r.descend_for_test();
    let saved = (r.home.clone(), r.treasury.clone());
    assert_eq!(r.squire_action(1, Action::Train(id)), Err(Error::NotHome));
    assert_eq!((r.home.clone(), r.treasury.clone()), saved);
}

#[test]
fn shield_is_a_real_foe_target_and_preview_agrees() {
    let mut r = fight(Kind::Shieldbearer);
    let i = foe(&mut r, 20.0, 12.0);
    let e = &r.enemies[i];
    let s = r.players[&1].squire.as_ref().unwrap();
    assert_eq!(press::warning_target(&r, e), Some((s.body.x, s.body.y)));
}

#[test]
fn squires_do_not_pick_up_loot_or_share_player_classes() {
    let mut r = fight(Kind::Ranger);
    r.join_seat(2, "another ranger");
    r.players.get_mut(&1).unwrap().knight = Some("archer".into());
    r.players.get_mut(&2).unwrap().knight = Some("archer".into());
    assert!(r.squire_roster(2).is_none(), "class ids are not identities");
    let s = &r.players[&1].squire.as_ref().unwrap().body;
    r.dungeon.rooms[r.at]
        .items
        .push(loot::Item::lying("longbow".into(), s.x, s.y, 0));
    let before = serde_json::to_value(&r.dungeon.rooms[r.at].items).unwrap();
    r.tick_squires();
    assert_eq!(
        serde_json::to_value(&r.dungeon.rooms[r.at].items).unwrap(),
        before
    );
}

#[test]
fn join_and_rename_reconcile_immediately_not_one_tick_later() {
    let mut r = fight(Kind::Shieldbearer);
    let name = r.players[&1].name.clone();
    r.join_seat(2, &name);
    r.join_seat(2, "new person");
    assert!(r.players[&2].squire.is_none());
    r.join_seat(2, &name);
    r.players.get_mut(&1).unwrap().name = "old owner left".into();
    r.join_seat(2, &name);
    assert!(
        r.players[&2].squire.is_some(),
        "rejoin must restore companion immediately"
    );
}

#[test]
fn bombs_really_splash_nearby_monsters_through_the_normal_step() {
    let mut r = fight(Kind::Alchemist);
    train_to(&mut r, 2);
    let i = foe(&mut r, 20.0, 12.0);
    let first = r.enemies[i].id;
    let i = foe(&mut r, 20.0, 14.5);
    let second = r.enemies[i].id;
    for _ in 0..HZ {
        r.step(&BTreeMap::new());
    }
    assert!(r.enemies.iter().find(|e| e.id == first).unwrap().hp < 1000);
    assert!(
        r.enemies.iter().find(|e| e.id == second).unwrap().hp < 1000,
        "a bomb must splash, not just look different"
    );
}

#[test]
fn stale_owner_cannot_guard_a_new_name_and_away_players_get_no_active_body() {
    let mut r = fight(Kind::Shieldbearer);
    let h = r.players.get_mut(&1).unwrap();
    h.name = "a new owner".into();
    let mut plain = h.clone();
    plain.squire = None;
    h.hurt(20);
    plain.hurt(20);
    assert_eq!(h.hp, plain.hp);
    r.sync_squires();
    assert!(r.players[&1].squire.is_none());
    let mut r = fight(Kind::Ranger);
    foe(&mut r, 20.0, 12.0);
    r.players.get_mut(&1).unwrap().away = true;
    r.tick_squires();
    assert!(r.projectiles.is_empty());
    assert!(r.squire_views().is_empty());
}

#[test]
fn old_hero_serde_defaults_to_no_companion() {
    let mut value = serde_json::to_value(Hero::new("legacy", 2.0, 3.0)).unwrap();
    value.as_object_mut().unwrap().remove("squire");
    let h: Hero = serde_json::from_value(value).unwrap();
    assert!(h.squire.is_none());
}

#[test]
fn hiring_hall_grows_the_roster_and_cuts_wages() {
    let mut r = Run::at_home(3, 1, None, home::Home::default(), Spoils::default());
    r.treasury.add(Spoil::Gold, 10_000);
    let first = r.squire_recruits(1)[0].clone();
    let initial_cap = r.home.squires_lodge.roster_cap();
    let initial_price = r.home.squires_lodge.hire_price(first.kind);
    assert_eq!(initial_cap, 3);
    for _ in 0..MAX_BUILDING {
        let before = r.home.squires_lodge.roster_cap();
        r.squire_action(1, Action::Upgrade(Building::Hiring)).unwrap();
        assert!(r.home.squires_lodge.roster_cap() > before);
    }
    assert_eq!(r.home.squires_lodge.roster_cap(), MAX_ROSTER);
    assert!(r.home.squires_lodge.hire_price(first.kind) < initial_price);
}

#[test]
fn yard_advertises_only_unlocked_training_abilities() {
    for kind in Kind::ALL {
        for level in 1..=MAX_LEVEL {
            assert_eq!(super::super::squires_town::ability(kind, level), kind.ability_summary(level));
        }
    }
}
