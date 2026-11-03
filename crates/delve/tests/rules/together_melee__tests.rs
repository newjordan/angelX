use super::*;

#[test]
fn melee_tracks_combo_and_returns_to_rest() {
    let r = Run::new(21, 1, None);
    let hero = &r.players[&1];
    let mut s = State::default();
    for n in 1..=4 {
        s.swing(1, hero, Stroke::Slash, 2.2, 130.0);
        assert_eq!(s.swings[0].combo, (n - 1) % 3 + 1);
    }
    assert!(s.valid());
    for _ in 0..48 {
        s.tick();
    }
    assert_eq!(s, State::default());
}

#[test]
fn old_feel_defaults_melee_and_new_events_roundtrip() {
    let mut r = Run::new(21, 1, None);
    r.feel
        .melee
        .swing(1, &r.players[&1], Stroke::Thrust, 2.0, 60.0);
    let json = serde_json::to_value(&r.feel).unwrap();
    let restored: feel::Feel = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(restored.melee, r.feel.melee);
    let mut old = json;
    old.as_object_mut().unwrap().remove("melee");
    let restored: feel::Feel = serde_json::from_value(old).unwrap();
    assert_eq!(restored.melee, State::default());
}

#[test]
fn melee_validation_and_caps_are_bounded() {
    let mut s = State::default();
    for _ in 0..100 {
        s.parry((1.0, 2.0), (1.0, 0.0));
    }
    assert!(s.valid());
    assert_eq!(s.hits.len(), 64);
    for x in [f32::NAN, f32::INFINITY, f32::MAX, 4097.0] {
        s.hits[0].x = x;
        assert!(!s.valid());
    }
    s.hits[0].x = 1.0;
    s.hits[0].dx = 2.0;
    assert!(!s.valid());
}

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
    assert!(r.take_up(1, kit));
    let h = r.players.get_mut(&1).unwrap();
    (h.x, h.y, h.aim_x, h.aim_y) = (12.0, 14.0, 1.0, 0.0);
    h.invulnerable = 1000;
    r.spawn_at_for_test(EnemyKind::Demon, 14.0, 14.0);
    let e = &mut r.enemies[0];
    (e.hp, e.max_hp, e.seen_hp) = (10000, 10000, 10000);
    e.sleep_until = 100000;
    r.phase = Phase::Fighting;
    r
}

#[test]
fn q_and_maul_keep_damage_and_rearm_carry_the_slash_and_hold_the_room() {
    let mut q = fight("percival");
    q.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            swing: true,
            ..Default::default()
        },
    )]));
    assert_eq!(10000 - q.enemies[0].hp, 30);
    assert_eq!(q.players[&1].sword_cooldown, SWORD_REARM);
    // The slash carries half the blade again and a quarter wider.
    assert_eq!(q.feel.melee.swings[0].reach, slash_reach(SWORD_REACH));
    assert!((q.feel.melee.swings[0].reach - 3.3).abs() < 1e-5);
    assert!((q.feel.melee.swings[0].arc - 162.5).abs() < 1e-4);
    assert_eq!(q.feel.melee.hits[0].tier, Tier::Medium);
    // A solid blow holds the room a moment.
    assert_eq!(q.hitstop.left, q.tune.hitstop_melee);
    assert_eq!(q.players[&1].parrying.map(|g| g.left), Some(PARRY_TICKS));
    let mut h = fight("hammerer");
    let pos = (h.enemies[0].x, h.enemies[0].y);
    h.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    assert_eq!(10000 - h.enemies[0].hp, 44);
    assert_eq!(h.players[&1].fire_cooldown, 24);
    assert_eq!(h.feel.melee.swings[0].stroke, Stroke::Overhead);
    assert_eq!(h.feel.melee.hits[0].tier, Tier::Heavy);
    assert!(h.feel.melee.hits[0].crack);
    assert_eq!(h.hitstop.left, h.tune.hitstop_heavy);
    assert_eq!(pos, (h.enemies[0].x, h.enemies[0].y));
}

#[test]
fn whiff_has_no_impact_or_hitstop_and_slam_has_one_floor_crack() {
    let mut r = fight("hammerer");
    r.enemies[0].x = 28.0;
    r.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]));
    assert!(r.feel.melee.hits.is_empty());
    assert_eq!(r.hitstop.left, 0);
    assert_eq!(r.feel.melee.swings.len(), 1);
    let mut r = fight("hammerer");
    r.step_sim_for_test(&BTreeMap::from([(
        1,
        Input {
            cast: 2,
            ..Default::default()
        },
    )]));
    for _ in 0..champions::SLAM_WINDUP {
        r.step_sim_for_test(&BTreeMap::new());
    }
    assert_eq!(r.feel.melee.hits.iter().filter(|h| h.crack).count(), 1);
    assert!(
        r.feel
            .melee
            .hits
            .iter()
            .any(|h| h.foe == r.enemies[0].id && h.tier == Tier::Heavy)
    );
}

#[test]
fn third_q_is_presentation_only_finisher_and_room_change_clears_it() {
    let mut r = fight("percival");
    for n in 1..=3 {
        let hp = r.enemies[0].hp;
        r.step_sim_for_test(&BTreeMap::from([(
            1,
            Input {
                swing: true,
                ..Default::default()
            },
        )]));
        assert_eq!(hp - r.enemies[0].hp, 30);
        assert_eq!(r.feel.melee.swings[0].combo, n);
        assert_eq!(
            r.feel.melee.hits.last().unwrap().tier,
            if n == 3 { Tier::Heavy } else { Tier::Medium }
        );
        while r.players[&1].sword_cooldown > 1 {
            r.step_sim_for_test(&BTreeMap::new());
        }
    }
    r.feel.clear_room();
    assert_eq!(r.feel.melee, State::default());
}

/// A monster's plain bullet at `(x, y)` going `(vx, vy)`.
fn bullet(x: f32, y: f32, vx: f32, vy: f32) -> Projectile {
    Projectile::bullet_for_test((x, y), (vx, vy), Shot::Arrow, 10)
}

/// The sword key, for one tick.
fn q(r: &mut Run) {
    r.step_sim_for_test(&BTreeMap::from([(1, Input { swing: true, ..Default::default() })]));
}

/// Played as before this pass, then put back.
fn before<T>(play: impl FnOnce() -> T) -> T {
    BEFORE.with(|b| b.set(true));
    let out = play();
    BEFORE.with(|b| b.set(false));
    out
}

#[test]
fn before_the_pass_the_blade_kept_its_reach_and_cut_shots_down_without_a_hold() {
    before(|| {
        let mut r = fight("percival");
        r.enemies[0].x = 28.0;
        r.projectiles.push(bullet(13.0, 14.0, -1.0, 0.0));
        q(&mut r);
        assert_eq!(r.feel.melee.swings[0].reach, SWORD_REACH);
        assert_eq!(r.feel.melee.swings[0].arc, SWORD_ARC);
        assert!(r.feel.melee.hits.iter().any(|h| h.tier == Tier::Blocked));
        assert!(r.projectiles.iter().all(|p| p.hostile));
        assert!(r.players[&1].parrying.is_none());
        assert_eq!(r.hitstop.left, 0);
    });
}

#[test]
fn the_slash_strikes_past_the_blade_and_wider_but_never_through_stone() {
    // Past the blade's reach, and wide of its arc: the slash's alone.
    let r0 = fight("percival");
    let radius = r0.enemies[0].radius();
    let far = 12.0 + SWORD_REACH + radius + 0.6;
    let wide = 75f32.to_radians();
    let places = [(far, 14.0), (12.0 + wide.cos() * 2.0, 14.0 + wide.sin() * 2.0), (12.0 + 3.3 + radius + 0.5, 14.0)];
    let struck = |old: bool| {
        let play = || {
            places
                .iter()
                .map(|&(x, y)| {
                    let mut r = fight("percival");
                    (r.enemies[0].x, r.enemies[0].y) = (x, y);
                    q(&mut r);
                    r.enemies[0].hp < 10000
                })
                .collect::<Vec<_>>()
        };
        if old { before(play) } else { play() }
    };
    assert_eq!(struck(true), [false, false, false]);
    assert_eq!(struck(false), [true, true, false]);
    // Stone between: the slash stops at it.
    let mut r = fight("percival");
    (r.players.get_mut(&1).unwrap().x, r.enemies[0].x) = (11.0, 14.2);
    let at = r.at;
    r.dungeon.rooms[at].set_for_test(6, 7, Tile::Block);
    q(&mut r);
    assert_eq!(r.enemies[0].hp, 10000);
}

#[test]
fn every_melee_arm_carries_half_again_and_a_quarter_wider() {
    let r = Run::new(21, 1, None);
    let mut seen = 0;
    for card in &r.book.cards {
        let Some(crate::rules::cards::Arm::Forged(w)) = card.arm.as_ref() else { continue };
        let Some(m) = w.melee else { continue };
        seen += 1;
        let k = slash_reach(m.reach) / m.reach;
        assert!((1.4..=1.5 + 1e-4).contains(&k), "{}: {} to {}", card.id, m.reach, slash_reach(m.reach));
        assert!((blade(slash_reach(m.reach)) - m.reach).abs() < 1e-4, "{}", card.id);
        let arc = slash_arc(m.arc);
        assert!(arc >= m.arc && arc <= 360.0, "{}", card.id);
        if m.arc < 270.0 {
            assert!((arc - m.arc * SLASH_WIDER).abs() < 1e-3, "{}", card.id);
        }
    }
    assert!(seen >= 7, "the arm cards with a blade: {seen}");
}

#[test]
fn a_bullet_in_the_slash_turns_back_the_knights_own() {
    let mut r = fight("percival");
    r.enemies[0].x = 28.0;
    r.projectiles.push(bullet(14.5, 14.0, -6.0, 0.0));
    q(&mut r);
    let shot = &r.projectiles[0];
    assert!(!shot.hostile && shot.traits.turned);
    assert_eq!(shot.traits.owner, 1);
    assert!(shot.vx > 0.0 && shot.vx.hypot(shot.vy) >= TURN_LEAST - 1e-3);
    assert!(shot.damage >= 20);
    assert!(r.feel.melee.hits.iter().any(|h| h.tier == Tier::Turned));
    assert!(r.feel.melee.valid());
    // The ting holds the room a moment too.
    assert_eq!(r.hitstop.left, r.tune.hitstop_melee);
    // It tings off the blade and whizzes back, never the shield's clang.
    assert!(PARRY_SOUNDS.iter().all(|s| r.sounds.contains(s)), "{:?}", r.sounds);
    assert!(!r.sounds.contains(&"clang"));
}

#[test]
fn one_stroke_turns_a_few_bullets_never_a_storm() {
    let mut r = fight("percival");
    r.enemies[0].x = 28.0;
    // Four in the slash past the blade, and two within it.
    for k in 0..4 {
        r.projectiles.push(bullet(15.0, 13.4 + k as f32 * 0.4, -1.0, 0.0));
    }
    for k in 0..2 {
        r.projectiles.push(bullet(13.4, 13.8 + k as f32 * 0.4, -1.0, 0.0));
    }
    q(&mut r);
    let turned = r.projectiles.iter().filter(|p| p.traits.turned).count();
    assert_eq!(turned, usize::from(PARRY_MOST));
    // The rest: what the blade itself reached is cut down, as it always
    // was; past it they fly on, still the monsters'.
    let flying: Vec<_> = r.projectiles.iter().filter(|p| !p.traits.turned && p.ttl > 0).collect();
    assert!(flying.iter().all(|p| p.hostile && p.x > 14.0), "{flying:?}");
    assert!(r.players[&1].parrying.is_none_or(|g| g.turns == 0));
}

#[test]
fn the_blade_turns_bullets_for_six_ticks_from_the_stroke_and_no_longer() {
    for k in 1..=PARRY_TICKS + 1 {
        let mut r = fight("percival");
        r.enemies[0].x = 28.0;
        q(&mut r);
        for _ in 1..k {
            r.step_sim_for_test(&BTreeMap::new());
        }
        // A bullet arriving in the slash's arc on the stroke's k-th tick.
        let h = &r.players[&1];
        r.projectiles.push(bullet(h.x + 2.6, h.y, -1.0, 0.0));
        r.step_sim_for_test(&BTreeMap::new());
        let turned = r.projectiles.first().is_some_and(|p| p.traits.turned);
        assert_eq!(turned, k < PARRY_TICKS, "tick {k} after the stroke");
    }
}

#[test]
fn a_turned_bullet_strikes_the_monster_that_loosed_it() {
    let mut r = fight("percival");
    let shooter = r.enemies[0].id;
    (r.enemies[0].x, r.enemies[0].frozen) = (20.0, 100_000);
    r.projectiles.push(bullet(14.5, 14.0, -6.0, 0.0));
    q(&mut r);
    for _ in 0..40 {
        r.step_sim_for_test(&BTreeMap::new());
    }
    let e = r.enemies.iter().find(|e| e.id == shooter).unwrap();
    assert!(e.hp < 10000, "the bullet came home");
    assert_eq!(e.wits.foe, 1);
}

#[test]
fn beams_lobs_mines_and_a_guardians_own_shots_are_never_turned() {
    let shots = || {
        let mut beam = bullet(13.4, 14.0, 0.0, 0.0);
        beam.traits.path.beam = Some(patterns::Beam::default());
        let mut lob = bullet(13.4, 14.4, -2.0, 0.0);
        lob.traits.path.form = patterns::Form::Lob;
        let mut mine = bullet(13.4, 13.6, 0.0, 0.0);
        mine.traits.path.wait = 30;
        let mut guardian = bullet(13.4, 14.2, -2.0, 0.0);
        guardian.look = bosses::Art::Cinderjaw.look();
        [beam, lob, mine, guardian]
    };
    for shot in shots() {
        assert!(!turnable(&shot), "{:?}", shot.traits.path);
    }
    assert!(turnable(&bullet(13.4, 14.0, -2.0, 0.0)));
    // Within the blade on the stroke's tick: cut down, as the blade did.
    let mut r = fight("percival");
    r.enemies[0].x = 28.0;
    r.projectiles.extend(shots());
    q(&mut r);
    assert!(r.projectiles.iter().all(|p| !p.traits.turned));
    assert_eq!(r.feel.melee.hits.iter().filter(|h| h.tier == Tier::Blocked).count(), 4);
    // Past the blade, in the slash: left alone, as hostile as they were.
    let mut r = fight("percival");
    r.enemies[0].x = 28.0;
    for mut shot in shots() {
        shot.x += 1.6;
        r.projectiles.push(shot);
    }
    q(&mut r);
    assert!(r.projectiles.iter().filter(|p| p.ttl > 0).all(|p| p.hostile && !p.traits.turned));
}

#[test]
fn a_killing_blow_holds_longer_and_kicks_the_camera() {
    let mut r = fight("percival");
    r.enemies[0].hp = 20;
    q(&mut r);
    assert!(r.enemies.iter().all(|e| e.hp == 0) || r.enemies.is_empty());
    assert_eq!(r.hitstop.left, r.tune.hitstop_melee_kill);
    assert!(r.shake >= feel::SHAKE_KILL);
    // Knobs at none: no hold at all.
    let mut r = fight("percival");
    r.tune.hitstop_melee = 0;
    q(&mut r);
    assert_eq!(r.hitstop.left, 0);
}

#[test]
fn the_guard_is_bounded_and_rides_a_save() {
    let mut r = fight("percival");
    q(&mut r);
    let guard = r.players[&1].parrying.expect("a young stroke guards");
    assert!(guard.valid() && r.valid_snapshot());
    for bad in [
        Guard { left: PARRY_TICKS + 1, ..guard },
        Guard { reach: f32::NAN, ..guard },
        Guard { dx: 2.0, ..guard },
        Guard { arc: 400.0, ..guard },
        Guard { turns: PARRY_MOST + 1, ..guard },
    ] {
        assert!(!bad.valid(), "{bad:?}");
    }
    let json = serde_json::to_value(&r.players[&1]).unwrap();
    let back: Hero = serde_json::from_value(json).unwrap();
    assert_eq!(back.parrying, Some(guard));
    // Spent, it is left out of the knight's writing.
    for _ in 0..PARRY_TICKS {
        r.step_sim_for_test(&BTreeMap::new());
    }
    assert!(r.players[&1].parrying.is_none());
    assert!(!serde_json::to_string(&r.players[&1]).unwrap().contains("parrying"));
}

#[test]
fn every_arm_whooshes_its_own_way() {
    for (kit, blade, sound) in [
        ("percival", Blade::Sword, "swing_sword"),
        ("ironbraid", Blade::Axe, "swing_axe"),
        ("hammerer", Blade::Maul, "swing_maul"),
        ("butcher", Blade::Cleaver, "swing_cleaver"),
        ("gravecaller", Blade::Spear, "swing_spear"),
    ] {
        let mut r = fight(kit);
        assert_eq!(Blade::of(&r.players[&1]), blade, "{kit}");
        assert_eq!(blade.swing_sound(), sound);
        // Its arm's stroke (the sword key for Percival, the arm for the rest).
        r.sounds.clear();
        let input = if kit == "percival" { Input { swing: true, ..Default::default() } } else { Input { fire: true, ..Default::default() } };
        r.step_sim_for_test(&BTreeMap::from([(1, input)]));
        assert!(r.sounds.contains(&sound), "{kit}: {:?}", r.sounds);
        assert!(!r.sounds.contains(&"swing"), "{kit}: {:?}", r.sounds);
    }
    // A stroke with no knight to read implies its arm.
    assert_eq!(Blade::of_stroke(Stroke::Overhead), Blade::Maul);
    assert_eq!(Blade::of_stroke(Stroke::Thrust), Blade::Spear);
    assert!(SWING_SOUNDS.iter().enumerate().all(|(i, s)| s.starts_with("swing_") && SWING_SOUNDS[..i].iter().all(|t| t != s)));
}

#[test]
fn a_stroke_lands_once_as_heavy_as_its_heaviest_contact() {
    // A solid blow: one medium contact, and the spectacle rings no second.
    let mut r = fight("percival");
    r.sounds.clear();
    q(&mut r);
    let landed: Vec<&str> = r.sounds.iter().copied().filter(|s| s.starts_with("hit")).collect();
    assert_eq!(landed, ["hit_flesh_medium"], "{:?}", r.sounds);
    // Two monsters in the slash: still one.
    let mut r = fight("percival");
    r.spawn_at_for_test(EnemyKind::Demon, 13.6, 14.6);
    for e in &mut r.enemies {
        (e.hp, e.max_hp, e.seen_hp, e.sleep_until) = (10000, 10000, 10000, 100000);
    }
    r.sounds.clear();
    q(&mut r);
    assert!(r.enemies.iter().all(|e| e.hp < 10000), "both struck");
    assert_eq!(r.sounds.iter().filter(|s| CONTACT_SOUNDS.contains(s)).count(), 1, "{:?}", r.sounds);
    // The third of the chain is a finisher: heavy.
    let mut r = fight("percival");
    let mut last = Vec::new();
    for _ in 0..3 {
        while r.players[&1].sword_cooldown > 0 {
            r.step_sim_for_test(&BTreeMap::new());
        }
        r.sounds.clear();
        q(&mut r);
        last = r.sounds.clone();
    }
    assert_eq!(r.feel.melee.combo(1), 3);
    assert!(last.contains(&"hit_flesh_heavy"), "{last:?}");
    // A whiff whooshes and lands nothing.
    let mut r = fight("percival");
    r.enemies[0].x = 30.0;
    r.sounds.clear();
    q(&mut r);
    assert!(r.sounds.contains(&"swing_sword"));
    assert!(!r.sounds.iter().any(|s| s.starts_with("hit")), "{:?}", r.sounds);
    // The tiers' own takes, and none for a block or a turn.
    assert_eq!(Tier::Light.sound(), Some("hit_flesh_light"));
    assert_eq!(Tier::Heavy.sound(), Some("hit_flesh_heavy"));
    assert_eq!((Tier::Blocked.sound(), Tier::Turned.sound()), (None, None));
}
