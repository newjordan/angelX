use super::*;
fn party() -> Run {
    let mut r = roundtable::fixture(&ADVENTURE);
    for id in 2..=6 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.start_deathmatch();
    r.step(&BTreeMap::new());
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = TITLE;
    for f in s.fighters.values_mut() {
        f.protection = 0;
    }
    r
}
fn ready(s: &mut Session, id: u32, at: (f32, f32)) {
    let f = s.fighters.get_mut(&id).unwrap();
    (f.hero.x, f.hero.y) = at;
    f.hero.hp = f.hero.max_hp;
    f.hero.invulnerable = 0;
    f.respawn = 0;
    f.protection = 0;
}
#[test]
fn kills_belong_to_the_attacker_and_ten_ends_the_round_once() {
    let mut r = party();
    let s = r.deathmatch.as_mut().unwrap();
    for n in 1..=GOAL {
        ready(s, 2, (15., 15.));
        s.fighters.get_mut(&2).unwrap().hero.hp = 4;
        assert!(s.hit(1, 2, 20, (1., 0.)));
        assert_eq!(s.fighters[&1].kills, n);
        assert_eq!(s.fighters[&2].deaths, n);
        assert_eq!(s.winner, if n == GOAL { Some(1) } else { None });
        assert!(!s.hit(1, 2, 20, (1., 0.)), "a corpse never pays twice");
    }
    ready(s, 3, (17., 15.));
    assert!(!s.hit(1, 3, 100, (1., 0.)));
    assert_eq!(s.fighters[&1].kills, GOAL);
}
#[test]
fn respawn_waits_then_chooses_the_farthest_spawn_and_wards_it() {
    let mut r = party();
    let book = r.book.clone();
    let s = r.deathmatch.as_mut().unwrap();
    for id in [1, 3, 4, 5, 6] {
        ready(s, id, (5., 5.));
    }
    s.fighters.get_mut(&2).unwrap().hero.hp = 4;
    s.hit(1, 2, 20, (1., 0.));
    for _ in 0..RESPAWN - 1 {
        s.step(&BTreeMap::new(), &book, 1);
    }
    assert_eq!(s.fighters[&2].respawn, 1);
    assert_eq!(s.fighters[&2].hero.hp, 0);
    s.step(&BTreeMap::new(), &book, 1);
    let f = &s.fighters[&2];
    assert_eq!((f.hero.x, f.hero.y), (43., 31.));
    assert_eq!(f.hero.hp, f.hero.max_hp);
    assert_eq!(f.protection, PROTECTION);
    assert!(!s.hit(1, 2, 50, (1., 0.)));
    s.step(
        &[(
            2,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &book,
        1,
    );
    assert_eq!(s.fighters[&2].protection, 0, "firing gives up the ward");
}
#[test]
fn the_real_deck_shapes_damage_rate_volley_fork_and_homing() {
    let mut r = party();
    let mut gift = r.book.get("bow").unwrap().clone();
    gift.id = "arena-gift".into();
    gift.kind = cards::Kind::Hold;
    gift.arm = None;
    gift.effects = vec![
        cards::Effect::Damage(30),
        cards::Effect::Rate(25),
        cards::Effect::Shots(2),
        cards::Effect::Volley(2),
        cards::Effect::Fork(2),
        cards::Effect::Homing(2),
        cards::Effect::Pierce(1),
    ];
    r.book.insert(gift);
    let h = r.players.get_mut(&1).unwrap();
    h.deck.push("arena-gift".into());
    h.rebonus(&r.book);
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    let f = s.fighters.get_mut(&1).unwrap();
    f.yaw = 0.;
    (f.hero.x, f.hero.y) = (15., 15.);
    s.step(
        &[(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    let ours: Vec<_> = s.shots.iter().filter(|p| p.traits.owner == 1).collect();
    assert!(ours.len() >= 3);
    assert!(ours.iter().all(|p| p.damage > Weapon::Bow.arms().damage
        && p.traits.fork == 2
        && p.traits.homing == 2
        && p.pierce >= 1));
    assert!(s.fighters[&1].hero.fire_cooldown < Weapon::Bow.arms().cooldown);
    assert_eq!(s.fighters[&1].hero.deck, r.players[&1].deck);
}
#[test]
fn shots_hit_knights_but_never_through_the_cover() {
    let mut r = party();
    let s = r.deathmatch.as_mut().unwrap();
    ready(s, 1, (15., 15.));
    ready(s, 2, (17., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    let hp = s.fighters[&2].hero.hp;
    for _ in 0..8 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            7,
        );
    }
    assert!(s.fighters[&2].hero.hp < hp);
    s.shots.clear();
    ready(s, 1, (17., 17.));
    ready(s, 2, (23., 17.));
    let hp = s.fighters[&2].hero.hp;
    for _ in 0..45 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            7,
        );
    }
    assert_eq!(s.fighters[&2].hero.hp, hp, "the pillar stops the volley");
}
#[test]
fn all_map_spawns_and_pickups_are_reachable() {
    assert!(MAP.iter().all(|r| r.len() == 24));
    let mut seen = std::collections::BTreeSet::from([(2, 2)]);
    let mut queue = std::collections::VecDeque::from([(2, 2)]);
    while let Some((x, y)) = queue.pop_front() {
        for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if p.0 >= 0
                && p.0 < 24
                && p.1 >= 0
                && p.1 < 18
                && MAP[p.1 as usize].as_bytes()[p.0 as usize] == b'.'
                && seen.insert(p)
            {
                queue.push_back(p);
            }
        }
    }
    for (x, y) in SPAWNS.into_iter().chain(PICKUPS.map(|(x, y, _)| (x, y))) {
        assert!(open(x, y), "{x} {y}");
        assert!(seen.contains(&((x / 2.) as i32, (y / 2.) as i32)));
    }
}
#[test]
fn old_save_and_new_wire_restore_the_round() {
    let r = party();
    let mut json = serde_json::to_value(&r).unwrap();
    json.as_object_mut().unwrap().remove("deathmatch");
    let old: Run = serde_json::from_value(json).unwrap();
    assert!(old.deathmatch.is_none());
    let bytes = serde_json::to_vec(&r).unwrap();
    let restored: Run = serde_json::from_slice(&bytes).unwrap();
    assert!(restored.valid_snapshot());
    let tree = angel_net::doc::value::to_value(&r).unwrap();
    let restored = crate::wire::read(&tree).unwrap();
    assert_eq!(restored.deathmatch.as_ref().unwrap().fighters.len(), 6);
}
#[test]
fn fixed_inputs_replay_identically_and_six_seats_move_independently() {
    let mut r = party();
    let mut a = r.deathmatch.take().unwrap();
    let mut b = a.clone();
    let inputs = [
        (
            1,
            Input {
                move_y: -1,
                aim_x: 1,
                fire: true,
                ..Default::default()
            },
        ),
        (
            6,
            Input {
                move_x: 1,
                ..Default::default()
            },
        ),
    ]
    .into();
    let p = a.fighters[&2].hero.x;
    for _ in 0..100 {
        a.step(&inputs, &r.book, 8);
        b.step(&inputs, &r.book, 8);
    }
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.fighters[&2].hero.x, p);
    assert!(a.valid());
}
#[test]
fn the_town_door_returns_everyone_with_their_original_kit_and_one_purse() {
    let mut r = Run::at_home(41, 1, None, Default::default(), Default::default());
    r.join_seat(2, "Friend");
    for h in r.players.values_mut() {
        h.carried.add(Spoil::Gold, 100);
    }
    r.walk_kingdom(Some(ENTRY));
    r.step(&BTreeMap::new());
    assert_eq!(r.room().tile(ENTRY.0, ENTRY.1), Tile::Floor);
    let hp = r.players[&1].hp;
    let hand = r.players[&1].hand.clone();
    r.step(
        &[(
            1,
            Input {
                interact: true,
                ..Default::default()
            },
        )]
        .into(),
    );
    assert!(r.deathmatch_showing());
    let s = r.deathmatch.as_mut().unwrap();
    s.ticks = TITLE;
    s.fighters.get_mut(&1).unwrap().kills = 9;
    ready(s, 1, (15., 15.));
    ready(s, 2, (17., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    s.fighters.get_mut(&2).unwrap().hero.hp = 4;
    for _ in 0..8 {
        r.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
        );
        if r.deathmatch.as_ref().unwrap().winner.is_some() {
            break;
        }
    }
    assert_eq!(r.deathmatch.as_ref().unwrap().winner, Some(1));
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), 110);
    assert_eq!(r.players[&2].carried.get(Spoil::Gold), 90);
    for _ in 0..PODIUM + 2 {
        r.step(&BTreeMap::new());
    }
    assert!(!r.deathmatch_showing());
    assert_eq!(r.room().kind, RoomKind::Kingdom);
    assert_eq!(r.players[&1].hp, hp);
    assert_eq!(r.players[&1].hand, hand);
    assert_eq!(r.players[&1].carried.get(Spoil::Gold), 110);
}
#[test]
fn protection_shields_and_rolls_all_turn_hits() {
    let mut r = party();
    let s = r.deathmatch.as_mut().unwrap();
    let f = s.fighters.get_mut(&2).unwrap();
    f.hero.guard = cards::Guard::Shield;
    f.hero.shielding = true;
    f.hero.aim_x = -1.;
    f.hero.aim_y = 0.;
    let hp = f.hero.hp;
    assert!(!s.hit(1, 2, 12, (1., 0.)));
    assert_eq!(s.fighters[&2].hero.hp, hp);
    assert!(s.fighters[&2].hero.shield_wear.showing());
    let f = s.fighters.get_mut(&2).unwrap();
    f.hero.shielding = false;
    f.hero.guard = cards::Guard::Roll;
    f.walk(Input {
        dash: true,
        ..Default::default()
    });
    assert!(!s.hit(1, 2, 12, (1., 0.)));
}
#[test]
fn leaving_or_away_cannot_be_farmed_and_a_late_join_gets_their_kit() {
    let mut r = party();
    r.leave_activity(6);
    r.set_away(5, true);
    r.step(&BTreeMap::new());
    let s = r.deathmatch.as_mut().unwrap();
    assert!(!s.fighters[&6].connected);
    assert!(!s.fighters[&5].connected);
    assert!(!s.hit(1, 6, 100, (1., 0.)));
    assert!(!s.hit(1, 5, 100, (1., 0.)));
}

#[test]
fn champions_keep_the_maul_and_their_own_spell_slots() {
    let mut r = party();
    assert!(r.take_up(1, "hammerer"));
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    ready(&mut s, 1, (15., 15.));
    ready(&mut s, 2, (17., 15.));
    let f = s.fighters.get_mut(&1).unwrap();
    f.yaw = 0.;
    assert!(f.hero.forged.as_ref().unwrap().melee.is_some());
    assert_eq!(f.hero.spells[0].as_deref(), Some("storm-hammer"));
    s.step(
        &[(
            1,
            Input {
                cast: 3,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(s.fighters[&1].hero.champ.fury > 0);
    let hp = s.fighters[&2].hero.hp;
    s.step(
        &[(
            1,
            Input {
                fire: true,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(s.fighters[&2].hero.hp < hp);
    assert!(
        s.shots.is_empty(),
        "the maul must not silently become a bow"
    );
    s.step(
        &[(
            1,
            Input {
                cast: 1,
                ..Default::default()
            },
        )]
        .into(),
        &r.book,
        1,
    );
    assert!(
        s.shots
            .iter()
            .any(|p| p.traits.owner == 1 && p.kind == Shot::Sacred)
    );
}
#[test]
fn pickups_are_shared_and_reappear_after_their_wait() {
    let mut r = party();
    let s = r.deathmatch.as_mut().unwrap();
    let (x, y, _) = PICKUPS[0];
    ready(s, 1, (x, y));
    s.fighters.get_mut(&1).unwrap().hero.hp = 20;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&1].hero.hp, 55);
    assert_eq!(s.pickups[0], PICKUP_REST);
    ready(s, 2, (x, y));
    s.fighters.get_mut(&2).unwrap().hero.hp = 20;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&2].hero.hp, 20);
    let (x, y, _) = PICKUPS[4];
    ready(s, 1, (x, y));
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&1].power, POWER_TIME);
    s.pickups[0] = 1;
    s.step(&BTreeMap::new(), &r.book, 1);
    assert_eq!(s.fighters[&2].hero.hp, 55);
}
#[test]
fn a_restored_round_keeps_hit_timing_and_patch_readers_keep_every_seat() {
    use crate::wire::{self, Guest, Shots};
    use std::borrow::Cow;
    let mut r = party();
    let mut s = r.deathmatch.take().unwrap();
    let mut loaded: Session = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    for round in [&mut s, &mut loaded] {
        ready(round, 2, (15., 15.));
        round.hit(1, 2, 8, (1., 0.));
    }
    assert_eq!(
        s.fighters[&2].hero.invulnerable,
        loaded.fighters[&2].hero.invulnerable
    );
    assert_eq!(s.fighters[&2].hero.hp, loaded.fighters[&2].hero.hp);
    r.deathmatch = Some(s);
    let tree = wire::publish(&r).unwrap();
    let mut guest = Guest::default();
    guest.read(&tree, None, &Shots::Rebuild).unwrap();
    r.step(
        &[(
            6,
            Input {
                move_x: 1,
                ..Default::default()
            },
        )]
        .into(),
    );
    let tree = wire::publish(&r).unwrap();
    let back = guest
        .read(
            &tree,
            Some(&[Cow::Borrowed("tick"), Cow::Borrowed("deathmatch")]),
            &Shots::Same,
        )
        .unwrap();
    assert_eq!(
        serde_json::to_vec(&back.deathmatch).unwrap(),
        serde_json::to_vec(&r.deathmatch).unwrap()
    );
}
#[test]
fn the_round_suspends_the_room_and_its_original_heroes() {
    let mut r = party();
    let hp: Vec<_> = r
        .players
        .values()
        .map(|h| (h.hp, h.x, h.y, h.bombs, h.hand.clone()))
        .collect();
    let foe = r.enemies.clone();
    for _ in 0..60 {
        r.step(
            &[(
                1,
                Input {
                    fire: true,
                    bomb: true,
                    ..Default::default()
                },
            )]
            .into(),
        );
    }
    assert_eq!(
        hp,
        r.players
            .values()
            .map(|h| (h.hp, h.x, h.y, h.bombs, h.hand.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        serde_json::to_vec(&foe).unwrap(),
        serde_json::to_vec(&r.enemies).unwrap()
    );
}

#[test]
fn held_orbits_clouds_and_chill_reach_knight_targets() {
    let mut r = party();
    let mut card = r.book.get("bow").unwrap().clone();
    card.id = "arena-passives".into();
    card.kind = cards::Kind::Hold;
    card.arm = None;
    card.effects = vec![
        cards::Effect::Orbit(3),
        cards::Effect::Fart(1),
        cards::Effect::Chill(1),
    ];
    r.book.insert(card);
    r.players
        .get_mut(&1)
        .unwrap()
        .deck
        .push("arena-passives".into());
    let mut s = Session::new(&r.players, &r.book);
    s.ticks = TITLE;
    ready(&mut s, 1, (15., 15.));
    s.fighters.get_mut(&1).unwrap().yaw = 0.;
    let orb = orbit_points(&s.fighters[&1].hero, 1, u64::from(TITLE))[0];
    ready(&mut s, 2, orb);
    let hp = s.fighters[&2].hero.hp;
    s.orbits(1);
    assert!(s.fighters[&2].hero.hp < hp);
    ready(&mut s, 2, (17., 15.));
    for _ in 0..4 {
        s.step(
            &[(
                1,
                Input {
                    fire: true,
                    ..Default::default()
                },
            )]
            .into(),
            &r.book,
            4,
        );
    }
    assert!(s.fields.iter().any(|f| f.owner == 1 && f.damage > 0));
    assert!(s.fighters[&2].hero.chilled > 0);
}
