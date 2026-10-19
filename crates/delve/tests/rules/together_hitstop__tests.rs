use super::*;
use crate::rules::feel::testing::{a_foe, quiet};

fn boss() -> Run {
    let mut r = quiet();
    for id in 2..=4 {
        r.join_seat(id, &format!("Knight {id}"));
    }
    r.spawn_boss_at_for_test(0, 24.0, 9.0);
    r.enemies[0].age = TELEGRAPH + 1;
    r
}
fn shot(r: &mut Run, kind: Shot, empowered: bool, damage: u32) {
    let e = &r.enemies[0];
    r.projectiles.push(Projectile {
        x: e.x,
        y: e.y,
        vx: 0.0,
        vy: 0.0,
        hostile: false,
        kind,
        look: None,
        damage,
        pierce: 0,
        last_hit: None,
        ttl: HZ,
        empowered,
        traits: ShotTraits::default(),
    });
}
fn hold(r: &mut Run, impact: Impact) {
    r.hitstop.request(&r.tune, impact);
    r.finish_hitstop();
}
fn room_bytes(r: &Run) -> Vec<u8> {
    let mut r = r.clone();
    r.hitstop = State::default();
    serde_json::to_vec(&r).unwrap()
}

#[test]
fn hitstop_holds_every_room_field_and_the_rng_for_exactly_six_transport_ticks() {
    let mut r = boss();
    hold(&mut r, Impact::BossKill);
    let bytes = room_bytes(&r);
    let tick = r.tick;
    for n in (0..6).rev() {
        r.step(&BTreeMap::from([(
            1,
            Input {
                move_x: 1,
                fire: true,
                ..Default::default()
            },
        )]));
        assert_eq!(r.hitstop.left, n);
        assert_eq!(r.tick, tick);
        assert_eq!(room_bytes(&r), bytes);
        assert!(!r.predictable(1));
    }
    r.step(&BTreeMap::new());
    assert_eq!(r.tick, tick + 1);
    assert!(!r.hitstop.frozen());
}

#[test]
fn hitstop_buffers_a_released_dash_and_bomb_for_each_of_four_seats() {
    let mut r = boss();
    for h in r.players.values_mut() {
        h.x = 20.0;
        h.y = 17.0;
        h.bombs = 2;
        h.dash_cooldown = 0;
    }
    hold(&mut r, Impact::BossKill);
    let keys = (1..=4)
        .map(|id| {
            (
                id,
                Input {
                    move_x: 1,
                    dash: true,
                    bomb: true,
                    ..Default::default()
                },
            )
        })
        .collect();
    r.step(&keys);
    for _ in 0..5 {
        r.step(&BTreeMap::new());
    }
    let saved = serde_json::to_vec(&r).unwrap();
    let mut restored: Run = serde_json::from_slice(&saved).unwrap();
    for game in [&mut r, &mut restored] {
        game.step(&BTreeMap::new());
        assert!(game.hitstop.buffered.is_empty());
        for h in game.players.values() {
            assert_eq!(h.bombs, 1);
            assert!(h.dash_cooldown > 0);
            assert!(h.x > 20.0);
        }
    }
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn hitstop_off_releases_buffered_actions_and_clears_the_hold() {
    let mut r = boss();
    hold(&mut r, Impact::BossKill);
    r.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    let bombs = r.players[&1].bombs;
    let tick = r.tick;
    r.tune.hitstop = 0;
    r.step(&BTreeMap::new());
    assert_eq!(r.tick, tick + 1);
    assert_eq!(r.players[&1].bombs, bombs - 1);
    assert!(r.hitstop.quiet());
}

#[test]
fn hitstop_plain_shots_are_free_but_empowered_and_ultimate_shots_hold_two_ticks() {
    for (kind, empowered, ticks) in [
        (Shot::Arrow, false, 0),
        (Shot::Ball, false, 0),
        (Shot::Arrow, true, 2),
        (Shot::Sacred, false, 2),
    ] {
        let mut r = boss();
        shot(&mut r, kind, empowered, 1);
        r.step(&BTreeMap::new());
        assert_eq!(r.hitstop.left, ticks, "{kind:?}/{empowered}");
        if ticks > 0 {
            assert!(r.enemies[0].hurt > 0);
            assert!(r.shake > 0);
        }
    }
}

#[test]
fn hitstop_bombs_phase_changes_and_staggers_use_the_tuned_durations() {
    let mut r = boss();
    r.tune.hitstop_heavy = 5;
    r.step(&BTreeMap::from([(
        1,
        Input {
            bomb: true,
            ..Default::default()
        },
    )]));
    assert_eq!(r.hitstop.left, 5);
    let mut r = boss();
    r.enemies[0].hp = r.enemies[0].max_hp / 10;
    r.bosses[0].rage = 0.5;
    r.step(&BTreeMap::new());
    assert_eq!(r.hitstop.left, 4);
    let mut r = boss();
    shot(&mut r, Shot::Sacred, false, 1);
    r.projectiles.last_mut().unwrap().traits.stun = 8;
    r.step(&BTreeMap::new());
    assert_eq!(r.hitstop.left, 4);
}

#[test]
fn hitstop_kills_override_cooldown_and_distinguish_large_bodies() {
    for (kind, ticks) in [(EnemyKind::Demon, 3), (EnemyKind::Skeleton, 0)] {
        let mut r = quiet();
        a_foe(&mut r, kind, 24.0, 9.0);
        r.enemies[0].hp = 1;
        r.hitstop.cooldown = 14;
        shot(&mut r, Shot::Arrow, false, 10);
        r.step(&BTreeMap::new());
        assert_eq!(r.hitstop.left, ticks, "{kind:?}");
    }
    let mut r = boss();
    r.hitstop.cooldown = 14;
    r.enemies[0].hp = 1;
    shot(&mut r, Shot::Sacred, false, 10);
    r.step(&BTreeMap::new());
    assert_eq!(r.hitstop.left, 6);
}

#[test]
fn hitstop_four_players_coalesce_impacts_and_cooldown_uses_transport_ticks() {
    let tune = tune::Tune::default();
    let mut state = State::default();
    for _ in 0..4 {
        state.request(&tune, Impact::Heavy);
    }
    assert!(state.finish(&tune));
    assert_eq!(state.left, 2);
    let players = BTreeMap::new();
    for t in 1..14 {
        state.inputs(&tune, &players, &BTreeMap::new());
        state.request(&tune, Impact::Phase);
        assert!(
            !state.finish(&tune),
            "second hold after {t} transport ticks"
        );
    }
    state.inputs(&tune, &players, &BTreeMap::new());
    state.request(&tune, Impact::Phase);
    assert!(state.finish(&tune));
    assert_eq!(state.left, 4);
    state.request(&tune, Impact::BossKill);
    assert!(state.finish(&tune));
    assert_eq!(state.left, 6);
}

#[test]
fn hitstop_save_and_wire_keep_countdown_and_buffer_without_client_motion() {
    let mut r = boss();
    shot(&mut r, Shot::Arrow, false, 1);
    let before = r.pose();
    hold(&mut r, Impact::Heavy);
    r.step(&BTreeMap::from([(
        1,
        Input {
            dash: true,
            move_x: 1,
            ..Default::default()
        },
    )]));
    let mut client = r.wire_copy();
    let x = client.players[&1].x;
    assert!(!client.predict(
        1,
        Input {
            move_x: 1,
            ..Default::default()
        }
    ));
    assert_eq!(client.players[&1].x, x);
    let bytes = room_bytes(&client);
    client.drawn_with(&before, 0.0, None, |drawn| {
        assert_eq!(room_bytes(drawn), bytes)
    });
    assert_eq!(client.hitstop, r.hitstop);
    let mut restored: Run = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
    for _ in 0..10 {
        r.step(&BTreeMap::new());
        restored.step(&BTreeMap::new());
    }
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
}

#[test]
fn hitstop_knobs_are_host_owned_valid_and_change_each_duration() {
    let tune = tune::Tune::default();
    assert!(tune.valid());
    for (name, impact) in [
        ("hitstop_boss_kill", Impact::BossKill),
        ("hitstop_elite_kill", Impact::EliteKill),
        ("hitstop_phase", Impact::Phase),
        ("hitstop_heavy", Impact::Heavy),
    ] {
        let mut tune = tune;
        assert!(tune.set(name, 7.0));
        let mut s = State::default();
        s.request(&tune, impact);
        assert!(s.finish(&tune));
        assert_eq!(s.left, 7);
        assert!(tune.set(name, 0.0));
        let mut s = State::default();
        s.request(&tune, impact);
        assert!(!s.finish(&tune));
    }
    let mut tune = tune;
    tune.set("hitstop_cooldown", 30.0);
    let mut s = State::default();
    s.request(&tune, Impact::Heavy);
    s.finish(&tune);
    assert_eq!(s.cooldown, 30);
    tune.set("hitstop", 0.0);
    let mut s = State::default();
    s.request(&tune, Impact::BossKill);
    assert!(!s.finish(&tune));
    for knob in tune
        .knobs()
        .iter()
        .filter(|k| k.name.starts_with("hitstop"))
    {
        assert_eq!(knob.side, angel_net::knob::Side::Server);
    }
}

#[test]
fn hitstop_frozen_wire_keeps_fast_projectile_identity() {
    let mut r = boss();
    shot(&mut r, Shot::Arrow, false, 1);
    r.projectiles[0].vx = 40.0;
    hold(&mut r, Impact::BossKill);
    let mut publisher = crate::wire::Publisher::default();
    let first = publisher.publish(&r).unwrap();
    for _ in 0..6 {
        r.step(&BTreeMap::new());
        let next = publisher.publish(&r).unwrap();
        assert_eq!(first.field("projectiles"), next.field("projectiles"));
    }
}
