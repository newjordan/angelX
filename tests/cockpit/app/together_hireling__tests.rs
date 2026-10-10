use super::*;
use crate::drive::together_realm::Spoils;
use crate::drive::together_shooter::home::{BUY_HOLD, Home, Station};

/// A realm whose west wing is dug: the tavern, and Beaumains in it.
fn tavern_home() -> Home {
    let mut home = Home {
        deepest: 1,
        ..Home::default()
    };
    home.levels.insert(Station::Wing, 1);
    home
}

fn centre((c, r, w, h): (i32, i32, i32, i32)) -> (f32, f32) {
    (
        (c as f32 + w as f32 / 2.0) * TILE_UNITS,
        (r as f32 + h as f32 / 2.0) * TILE_UNITS,
    )
}

#[test]
fn beaumains_greets_a_knight_and_takes_a_wage_at_his_plate() {
    let mut run = Run::at_home(3, 1, None, tavern_home(), Spoils::default());
    let tavern = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Tavern)
        .expect("the tavern is dug");
    run.enter_for_test(tavern);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (
        BEAUMAINS_AT.0 * TILE_UNITS + 2.0,
        BEAUMAINS_AT.1 * TILE_UNITS,
    );
    run.step(&BTreeMap::new());
    assert!(run.cues.iter().any(|c| c == "npc:beaumains"));
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = centre(HIRE_PLATE);
    let fire = BTreeMap::from([(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 1 {
        run.step(&fire);
    }
    assert_eq!(
        run.orders.iter().map(|o| o.station).collect::<Vec<_>>(),
        [Station::Hire]
    );
}

/// One knight down the stair, with Beaumains if `hired`.
fn down(hired: bool) -> Run {
    let mut home = tavern_home();
    home.hire = hired.then(|| "beaumains".into());
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    run.descend_for_test();
    run
}

#[test]
fn hired_he_goes_down_the_stair_and_the_tab_clears() {
    let run = down(true);
    let hire = run.hireling.as_ref().expect("he came down");
    assert_eq!(hire.hp, hire.max_hp);
    assert_eq!(run.home.hire, None);
    assert_eq!(run.marks.get("hired"), Some(&1), "the cockpit clears it");
    assert!(run.cues.iter().any(|c| c == "hired:beaumains"));
    let run = down(false);
    assert!(run.hireling.is_none());
    assert!(!run.marks.contains_key("hired"));
}

/// A first-floor fight room, emptied and calmed, knight one and Beaumains
/// side by side; a bat frozen in the far corner keeps the fight on.
fn with_beaumains() -> Run {
    let mut run = down(true);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    run.spawn_at_for_test(EnemyKind::Bat, 44.0, 3.0);
    if let Some(bat) = run.enemies.last_mut() {
        bat.frozen = u32::MAX / 2;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (10.0, 12.0, 0);
    let hire = run.hireling.as_mut().unwrap();
    (hire.x, hire.y, hire.invulnerable) = (12.0, 14.0, 0);
    run
}

#[test]
fn he_throws_knives_at_the_nearest_monster_from_a_few_steps_off() {
    let mut run = with_beaumains();
    run.spawn_at_for_test(EnemyKind::Bat, 15.0, 14.0);
    let mark = run.enemies.last().unwrap().id;
    if let Some(bat) = run.enemies.last_mut() {
        (bat.frozen, bat.hp, bat.max_hp) = (u32::MAX / 2, 10_000, 10_000);
    }
    for _ in 0..3 * HZ {
        run.players.get_mut(&1).unwrap().hp = 100;
        run.step(&BTreeMap::new());
    }
    let bat = run.enemies.iter().find(|e| e.id == mark).unwrap();
    assert!(bat.hp < 10_000, "his knives found it");
    let hire = run.hireling.as_ref().unwrap();
    let d = (hire.x - bat.x).hypot(hire.y - bat.y);
    assert!(d > 4.0, "he keeps a few steps off: {d}");
}

#[test]
fn felled_he_sits_the_fight_out_and_gets_up_after_it() {
    let mut run = with_beaumains();
    let hire = run.hireling.as_mut().unwrap();
    hire.hp = 5;
    let (x, y) = (hire.x, hire.y);
    run.projectiles.push(Projectile {
        x,
        y,
        vx: 8.0,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 12,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    });
    run.step(&BTreeMap::new());
    assert!(run.hireling.as_ref().unwrap().down());
    assert!(run.cues.iter().any(|c| c == "beaumains_down"));
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    assert!(
        run.hireling.as_ref().unwrap().down(),
        "while the fight lasts"
    );
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let hire = run.hireling.as_ref().unwrap();
    assert_eq!(hire.hp, hire.max_hp / 2, "up again, sore");
    assert!(run.cues.iter().any(|c| c == "beaumains_up"));
}

#[test]
fn he_follows_the_party_through_the_doors() {
    let mut run = with_beaumains();
    run.enemies.clear();
    run.step(&BTreeMap::new());
    let next = (0..4)
        .find_map(|side| run.dungeon.neighbour(run.at, side))
        .expect("a way on");
    run.enter_for_test(next);
    let hero = &run.players[&1];
    let hire = run.hireling.as_ref().unwrap();
    assert!((hire.x - hero.x).hypot(hire.y - hero.y) < 2.5);
}

#[test]
fn he_is_no_knight_of_the_party() {
    // A guardian doesn't grow for him.
    let mut run = down(true);
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(stairs);
    let boss = run.enemies.iter().find(|e| e.boss.is_some()).unwrap();
    let guardian = &run.bosses[usize::from(boss.boss.unwrap())];
    assert_eq!(boss.max_hp, guardian.hp, "one knight's worth");
    // And the party falls when its knights do, whether he stands or not.
    let mut run = with_beaumains();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.hp, hero.winds, hero.talisman) = (0, 0, false);
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Wiped);
}

#[test]
fn a_ravage_throws_him_too() {
    let mut run = with_beaumains();
    let hire = run.hireling.as_ref().unwrap();
    let (x, y, hp) = (hire.x, hire.y, hire.hp);
    run.ravages.push(tide::Ravage {
        x: x + tide::RING_GAP,
        y,
        rings: 1,
        lane: 0.0,
        width: 0.5,
        step: 6,
        damage: 16,
        age: 0,
    });
    for _ in 0..tide::WARN + 1 {
        run.step(&BTreeMap::new());
    }
    assert!(run.hireling.as_ref().unwrap().hp < hp);
}

/// Each summoning card, the minion it calls, and that minion's cue word.
const SUMMONS: [(&str, AllyKind); 9] = [
    ("hearth-bell", AllyKind::Brownie),
    ("rime-whistle", AllyKind::Moth),
    ("salt-thread", AllyKind::Wisp),
    ("glass-needle", AllyKind::Mite),
    ("cinder-wick", AllyKind::AshSprite),
    ("lantern-mote", AllyKind::Mote),
    ("reed-flute", AllyKind::Newt),
    ("choir-crumb", AllyKind::Linnet),
    ("marrow-sip", AllyKind::Leech),
];

/// A first-floor fight room, emptied and calmed, with Beaumains if
/// `hired`; knight one at (12, 12), unhurt, holding `hand`.
fn fight_with(hired: bool, hand: &[&str]) -> Run {
    let mut run = down(hired);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    run.rune = None;
    run.dungeon.rooms[room].items.clear();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y, hero.invulnerable) = (12.0, 12.0, 0);
    hero.hand = hand.iter().map(|id| id.to_string()).collect();
    hero.play_cooldown = 0;
    run
}

fn play(run: &mut Run, slot: u8) {
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: slot,
            ..Default::default()
        },
    )]));
}

/// A foe at `(x, y)`, awake, that holds still by itself (`frozen`).
fn foe_at(run: &mut Run, kind: EnemyKind, (x, y): (f32, f32), frozen: u32) -> u32 {
    run.spawn_at_for_test(kind, x, y);
    let foe = run.enemies.last_mut().unwrap();
    foe.frozen = frozen;
    foe.id
}

fn foe(run: &Run, id: u32) -> &Enemy {
    run.enemies.iter().find(|e| e.id == id).unwrap()
}

fn orb(x: f32, y: f32, vx: f32, vy: f32) -> Projectile {
    Projectile {
        x,
        y,
        vx,
        vy,
        hostile: true,
        look: None,
        kind: Shot::Orb,
        damage: 4,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: HZ,
    }
}

/// The ally, moved to `(x, y)` and ready to act.
fn ready(run: &mut Run, (x, y): (f32, f32)) {
    let hire = run.hireling.as_mut().unwrap();
    (hire.x, hire.y, hire.cooldown) = (x, y, 0);
}

#[test]
fn every_summoning_card_calls_its_own_minion_at_once_and_none_throws() {
    for (card, kind) in SUMMONS {
        let mut run = fight_with(false, &[card]);
        // A bat held far off keeps the fight on, out of every minion's sight.
        foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
        assert_eq!(
            run.book.get(card).and_then(Card::summon).map(|s| s.0),
            Some(kind)
        );
        play(&mut run, 1);
        let hire = run
            .hireling
            .as_ref()
            .unwrap_or_else(|| panic!("{card} called"));
        assert_eq!(hire.kind, kind, "{card}");
        assert_eq!(hire.summoner, Some(1));
        assert!(run.players[&1].hand.is_empty(), "{card} is spent");
        assert!(
            run.cues
                .iter()
                .any(|c| *c == format!("summoned:{}", kind.word())),
            "{card}: {:?}",
            run.cues
        );
        assert_ne!(kind.name(), "Beaumains", "{card} speaks for itself");
        for _ in 0..2 * HZ {
            run.step(&BTreeMap::new());
        }
        assert!(
            run.projectiles.iter().all(|p| p.hostile),
            "{card}: a minion throws no knife"
        );
    }
}

#[test]
fn a_minion_comes_on_the_tick_its_card_is_played_even_if_its_caller_falls() {
    let mut run = fight_with(false, &["hearth-bell"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.hp, hero.winds, hero.talisman) = (1, 0, false);
    // A second knight keeps the party standing.
    run.join("Matt");
    run.projectiles.push(orb(12.0, 12.0, 0.0, 0.0));
    play(&mut run, 1);
    assert_eq!(run.players[&1].hp, 0, "the caller fell that tick");
    assert_eq!(
        run.hireling.as_ref().map(|h| h.kind),
        Some(AllyKind::Brownie),
        "the bell was not spent for nothing"
    );
}

#[test]
fn a_summons_takes_the_slot_from_a_minion_but_never_from_beaumains() {
    let mut run = fight_with(false, &["hearth-bell", "rime-whistle"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    assert_eq!(run.hireling.as_ref().unwrap().kind, AllyKind::Brownie);
    run.players.get_mut(&1).unwrap().play_cooldown = 0;
    play(&mut run, 1);
    assert_eq!(
        run.hireling.as_ref().unwrap().kind,
        AllyKind::Moth,
        "the moth takes the brownie's place"
    );
    assert!(run.players[&1].hand.is_empty());

    let mut run = fight_with(true, &["rime-whistle", "potion"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    assert_eq!(run.hireling.as_ref().unwrap().kind, AllyKind::Beaumains);
    assert_eq!(run.players[&1].hand, ["rime-whistle", "potion"], "kept");
    assert_eq!(
        run.found.as_ref().map(|f| f.2.as_str()),
        Some("Beaumains will not share the road with it")
    );
    // The other cards still play.
    run.players.get_mut(&1).unwrap().play_cooldown = 0;
    play(&mut run, 2);
    assert_eq!(run.players[&1].hand, ["rime-whistle"]);
}

#[test]
fn while_beaumains_walks_with_the_party_summoning_cards_stay_on_the_floor() {
    for hired in [true, false] {
        let mut run = fight_with(hired, &[]);
        foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
        let at = run.at;
        for card in ["hearth-bell", "potion"] {
            run.dungeon.rooms[at].items.push(Item {
                card: card.into(),
                x: 12.0,
                y: 12.0,
                held_off: None,
            });
        }
        run.step(&BTreeMap::new());
        run.step(&BTreeMap::new());
        let hand = &run.players[&1].hand;
        assert!(
            hand.iter().any(|c| c == "potion"),
            "hired {hired}: {hand:?}"
        );
        assert_eq!(
            hand.iter().any(|c| c == "hearth-bell"),
            !hired,
            "hired {hired}: {hand:?}"
        );
        assert_eq!(
            run.room().items.iter().any(|i| i.card == "hearth-bell"),
            hired,
            "the bell waits on the floor while he is hired"
        );
    }
}

#[test]
fn two_knights_summoning_on_one_tick_leave_one_minion() {
    let mut run = Run::new(3, 9, Some("Matt"));
    run.calm_for_test();
    run.enemies.clear();
    for (id, card) in [(1u32, "hearth-bell"), (2, "rime-whistle")] {
        let hero = run.players.get_mut(&id).unwrap();
        hero.hand = vec![card.into()];
        hero.play_cooldown = 0;
    }
    run.step(&BTreeMap::from([
        (
            1,
            Input {
                play: 1,
                ..Default::default()
            },
        ),
        (
            2,
            Input {
                play: 1,
                ..Default::default()
            },
        ),
    ]));
    let hire = run.hireling.as_ref().expect("one ally answered");
    assert_eq!((hire.kind, hire.summoner), (AllyKind::Moth, Some(2)));
    assert!(run.players.values().all(|h| h.hand.is_empty()));
}

#[test]
fn a_brownie_catches_a_shot_then_rests() {
    let mut run = fight_with(false, &["hearth-bell"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    ready(&mut run, (20.0, 12.0));
    run.projectiles.push(orb(22.0, 12.0, 0.0, 0.0));
    run.step(&BTreeMap::new());
    assert!(
        run.cues.iter().any(|c| c == "brownie_catch"),
        "{:?}",
        run.cues
    );
    assert!(run.projectiles.iter().all(|p| p.ttl == 0 || !p.hostile));
    run.projectiles.push(orb(21.0, 12.0, 0.0, 0.0));
    run.cues.clear();
    run.step(&BTreeMap::new());
    assert!(!run.cues.iter().any(|c| c == "brownie_catch"), "it rests");
}

#[test]
fn a_rime_moth_freezes_for_its_cards_frost_and_a_thawed_foe_walks_free_a_while() {
    // A friend's whistle with a longer frost: the frost is the played card's.
    let mut run = fight_with(false, &[]);
    let card = cards::check("long-whistle", "name Long Whistle\nkind play\nmoth 80\n")
        .unwrap()
        .card;
    run.book.insert(card);
    run.players.get_mut(&1).unwrap().hand = vec!["long-whistle".into()];
    let bat = foe_at(&mut run, EnemyKind::Skeleton, (20.0, 12.0), 0);
    run.enemies.last_mut().unwrap().age = u32::MAX / 2;
    play(&mut run, 1);
    assert_eq!(foe(&run, bat).frozen, 80, "frozen on the tick it came");
    assert!(run.cues.iter().any(|c| c == "moth_chill"));
    // Through the frost and the thaw: never frozen again on the tick it thaws.
    let mut thawed = 0;
    for _ in 0..80 + MOTH_REST - 5 {
        run.enemies.iter_mut().for_each(|e| e.hp = e.max_hp);
        let hero = run.players.get_mut(&1).unwrap();
        (hero.hp, hero.x, hero.y) = (hero.max_hp, 4.0, 4.0);
        run.step(&BTreeMap::new());
        if foe(&run, bat).frozen == 0 {
            thawed += 1;
        }
    }
    assert!(
        thawed >= MOTH_REST - 10,
        "it walked free a while ({thawed} ticks)"
    );
}

#[test]
fn no_minion_freezes_pins_moves_or_sips_the_great_foes() {
    for (card, kind) in SUMMONS {
        if !matches!(
            kind,
            AllyKind::Moth
                | AllyKind::Newt
                | AllyKind::Wisp
                | AllyKind::AshSprite
                | AllyKind::Leech
        ) {
            continue;
        }
        let mut run = fight_with(false, &[card]);
        play(&mut run, 1);
        let mut greats = Vec::new();
        for (kind, at) in [
            (EnemyKind::Dragon, (24.0, 8.0)),
            (EnemyKind::PitTyrant, (24.0, 20.0)),
            (EnemyKind::Dummy, (30.0, 14.0)),
        ] {
            greats.push(foe_at(&mut run, kind, at, 0));
        }
        run.spawn_boss(0);
        let boss = run.enemies.last_mut().unwrap();
        (boss.x, boss.y, boss.age) = (18.0, 14.0, TELEGRAPH);
        greats.push(boss.id);
        let before: Vec<(f32, f32, u32)> = greats
            .iter()
            .map(|&id| (foe(&run, id).x, foe(&run, id).y, foe(&run, id).hp))
            .collect();
        let mut hire = run.hireling.take().unwrap();
        (hire.x, hire.y, hire.cooldown) = (24.0, 14.0, 0);
        match kind {
            AllyKind::Moth => run.chill_for_moth(&mut hire),
            AllyKind::Newt => run.pin_for_newt(&mut hire),
            AllyKind::Wisp => run.shove_for_wisp(&mut hire),
            AllyKind::AshSprite => run.pull_for_sprite(&mut hire),
            _ => run.sip_for_leech(&mut hire),
        }
        for (&id, was) in greats.iter().zip(before) {
            let e = foe(&run, id);
            assert_eq!((e.x, e.y, e.hp), was, "{card} left {:?} be", e.kind);
            assert_eq!(e.frozen, 0, "{card} did not hold {:?}", e.kind);
        }
        assert_eq!(hire.cooldown, 0, "{card} found nothing it may touch");
    }
}

#[test]
fn a_wisp_and_an_ash_sprite_move_foes_only_over_open_floor() {
    for (card, cue) in [
        ("salt-thread", "wisp_shove"),
        ("cinder-wick", "sprite_pull"),
    ] {
        let mut run = fight_with(false, &[card]);
        play(&mut run, 1);
        // Open floor: a skeleton held still on a clear stretch of the room
        // is moved a step along it.
        let (x, y) = {
            let grid = Grid {
                room: run.room(),
                barred: true,
            };
            let clear = |x: f32, y: f32| {
                (0..=24).all(|i| grid.clear(x - 6.0 + i as f32 * 0.5, y, 0.7, Mover::Walker))
            };
            let room = run.room();
            (2..room.rows - 2)
                .flat_map(|r| (4..room.cols - 4).map(move |c| (c, r)))
                .map(|(c, r)| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS))
                .find(|&(x, y)| clear(x, y))
                .expect("a clear stretch of floor")
        };
        let bat = foe_at(&mut run, EnemyKind::Skeleton, (x, y), u32::MAX / 2);
        let before = foe(&run, bat).x;
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (x - 6.0, y);
        ready(&mut run, (x - 3.0, y));
        run.step(&BTreeMap::new());
        assert!(run.cues.iter().any(|c| c == cue), "{card}: {:?}", run.cues);
        let after = foe(&run, bat).x;
        if cue == "wisp_shove" {
            assert!(after > before + 1.0, "shoved: {before} to {after}");
        } else {
            assert!(after < before - 1.0, "pulled: {before} to {after}");
        }
        // Against stone: a skeleton standing on the last floor tile before
        // the east wall (shoved at it) or the west wall (pulled at it).
        run.enemies.clear();
        let room = run.room();
        let (cols, w) = (room.cols as i32, room.width());
        let row = (1..room.rows as i32 - 1)
            .find(|&r| {
                room.tile(1, r) == Tile::Floor
                    && room.tile(cols - 2, r) == Tile::Floor
                    && room.tile(0, r) == Tile::Wall
                    && room.tile(cols - 1, r) == Tile::Wall
            })
            .expect("a row with floor along both walls");
        let y = (row as f32 + 0.5) * TILE_UNITS;
        let (at, minion, knight) = if cue == "wisp_shove" {
            ((w - 2.75, y), (w - 6.0, y), (w - 8.0, y))
        } else {
            ((2.75, y), (8.0, y), (1.0, y))
        };
        let skeleton = foe_at(&mut run, EnemyKind::Skeleton, at, u32::MAX / 2);
        for _ in 0..6 {
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y, hero.hp) = (knight.0, knight.1, hero.max_hp);
            ready(&mut run, minion);
            run.step(&BTreeMap::new());
            let e = foe(&run, skeleton);
            let grid = Grid {
                room: run.room(),
                barred: true,
            };
            assert!(
                grid.clear(e.x, e.y, e.radius(), Mover::Walker),
                "{card}: never into the wall ({}, {})",
                e.x,
                e.y
            );
        }
    }
}

#[test]
fn a_glass_mite_turns_a_shot_and_a_lantern_mote_snuffs_one() {
    let mut run = fight_with(false, &["glass-needle"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    ready(&mut run, (20.0, 12.0));
    let before = run.projectiles.len();
    run.projectiles.push(orb(21.5, 12.0, 4.0, -2.0));
    run.step(&BTreeMap::new());
    assert!(
        run.cues.iter().any(|c| c == "mite_reflect"),
        "{:?}",
        run.cues
    );
    assert_eq!(run.projectiles.len(), before + 1, "turned, not added to");
    assert!(run.projectiles.iter().any(|p| !p.hostile && p.vx < 0.0));

    let mut run = fight_with(false, &["lantern-mote"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    ready(&mut run, (20.0, 12.0));
    let before = run.projectiles.len();
    run.projectiles.push(orb(21.5, 12.0, 4.0, -2.0));
    run.step(&BTreeMap::new());
    assert!(run.cues.iter().any(|c| c == "mote_snuff"), "{:?}", run.cues);
    assert_eq!(run.projectiles.len(), before, "snuffed, nothing added");
    assert!(run.projectiles.iter().all(|p| !p.hostile || p.ttl == 0));
}

#[test]
fn a_reed_newt_pins_the_nearest_foe_briefly_and_never_shortens_a_hold() {
    let mut run = fight_with(false, &["reed-flute"]);
    play(&mut run, 1);
    ready(&mut run, (12.0, 20.0));
    let dead = foe_at(&mut run, EnemyKind::Bat, (13.0, 20.0), 0);
    run.enemies.last_mut().unwrap().hp = 0;
    let near = foe_at(&mut run, EnemyKind::Bat, (17.0, 20.0), 0);
    let far = foe_at(&mut run, EnemyKind::Bat, (22.0, 20.0), 3 * HZ);
    let mut hire = run.hireling.take().unwrap();
    run.pin_for_newt(&mut hire);
    assert_eq!(foe(&run, near).frozen, NEWT_PIN, "the nearest living foe");
    assert_eq!(foe(&run, far).frozen, 3 * HZ, "a longer hold is left");
    assert_eq!(foe(&run, dead).frozen, 0);
    assert_eq!(hire.cooldown, NEWT_EVERY);
    assert!(NEWT_EVERY > NEWT_PIN, "it rests longer than it pins");
}

#[test]
fn a_linnet_mends_the_nearest_wounded_knight_a_little_then_rests() {
    let mut run = fight_with(false, &["choir-crumb"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    assert_eq!(
        run.hireling.as_ref().unwrap().cooldown,
        0,
        "nobody hurt yet"
    );
    run.join("Matt");
    let at = (
        run.hireling.as_ref().unwrap().x,
        run.hireling.as_ref().unwrap().y,
    );
    let near = run.players.get_mut(&1).unwrap();
    near.hp -= 20;
    let (near_hp, far_hp) = (run.players[&1].hp, {
        let far = run.players.get_mut(&2).unwrap();
        (far.x, far.y, far.hp) = (at.0 + 12.0, at.1, far.max_hp - 30);
        far.hp
    });
    ready(&mut run, at);
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].hp, near_hp + LINNET_MEND);
    assert_eq!(run.players[&2].hp, far_hp, "the farther knight waits");
    assert!(run.cues.iter().any(|c| c == "linnet_mend"));
    assert_eq!(run.hireling.as_ref().unwrap().cooldown, LINNET_EVERY);
}

#[test]
fn a_leech_sips_blood_not_bone_and_mends_its_caller_by_half() {
    let mut run = fight_with(false, &["marrow-sip"]);
    play(&mut run, 1);
    ready(&mut run, (20.0, 12.0));
    // A skeleton is nearer, but has nothing in it.
    let bones = foe_at(&mut run, EnemyKind::Skeleton, (21.0, 12.0), u32::MAX / 2);
    let imp = foe_at(&mut run, EnemyKind::Imp, (24.0, 12.0), u32::MAX / 2);
    let hero = run.players.get_mut(&1).unwrap();
    hero.hp -= 20;
    let (hp, imp_hp, bone_hp) = (hero.hp, foe(&run, imp).hp, foe(&run, bones).hp);
    let mut hire = run.hireling.take().unwrap();
    run.sip_for_leech(&mut hire);
    assert_eq!(foe(&run, bones).hp, bone_hp, "no blood in bone");
    assert_eq!(foe(&run, imp).hp, imp_hp - LEECH_SIP);
    assert_eq!(run.players[&1].hp, hp + LEECH_SIP / 2, "half of it mends");
    assert_eq!(hire.cooldown, LEECH_EVERY);
}

#[test]
fn only_beaumains_speaks_as_beaumains() {
    let mut run = fight_with(false, &["hearth-bell"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    let hire = run.hireling.as_mut().unwrap();
    (hire.hp, hire.invulnerable, hire.cooldown) = (3, 0, 99);
    let (x, y) = (hire.x, hire.y);
    run.projectiles.push(orb(x, y, 0.0, 0.0));
    run.step(&BTreeMap::new());
    assert!(run.hireling.as_ref().unwrap().down());
    run.enemies.clear();
    run.step(&BTreeMap::new());
    assert!(!run.hireling.as_ref().unwrap().down(), "up after the fight");
    assert!(
        !run.cues.iter().any(|c| c.starts_with("beaumains")),
        "{:?}",
        run.cues
    );
}

#[test]
fn the_side_panel_names_the_minion_and_not_beaumains() {
    let mut run = fight_with(false, &["marrow-sip"]);
    foe_at(&mut run, EnemyKind::Bat, (44.0, 3.0), u32::MAX / 2);
    play(&mut run, 1);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::viz::shooter_viz::render(
                frame,
                &run,
                frame.area(),
                "",
                true,
                false,
                false,
                "",
                None,
                None,
                None,
                None,
                false,
            );
        })
        .unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    let hire = run.hireling.as_ref().unwrap();
    assert!(
        text.contains(&format!("Leech: {}/{}", hire.hp, hire.max_hp)),
        "the leech by name"
    );
    assert!(!text.contains("Beaumains"));
}

#[test]
fn a_summons_belongs_on_a_play_card() {
    for kind in ["take", "hold"] {
        let errors = cards::check("x", &format!("name X\nkind {kind}\nbrownie 1\n"))
            .unwrap_err()
            .errors
            .join(" | ");
        assert!(
            errors.contains("brownie works on play cards"),
            "{kind}: {errors}"
        );
    }
    assert!(cards::check("x", "name X\nkind play\nlinnet 1\n").is_ok());
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_minion_shots -- --ignored`:
/// each minion beside a knight, in a real frame, for the eye.
#[test]
#[ignore]
fn write_minion_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let kinds = std::iter::once(AllyKind::Beaumains).chain(SUMMONS.iter().map(|s| s.1));
    for kind in kinds {
        let mut run = fight_with(false, &[]);
        foe_at(&mut run, EnemyKind::Skeleton, (36.0, 8.0), u32::MAX / 2);
        let mut hire = Hireling::minion(kind, 45, 1, (15.0, 12.0), 1);
        hire.facing = -1.0;
        run.hireling = Some(hire);
        run.players.get_mut(&1).unwrap().invulnerable = 0;
        let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("minion_{}.ppm", kind.word())), out).unwrap();
    }
    // And the nine cards that call them, lying where their secrets left them.
    let mut run = fight_with(false, &[]);
    foe_at(&mut run, EnemyKind::Skeleton, (36.0, 8.0), u32::MAX / 2);
    let at = run.at;
    for (i, (card, _)) in SUMMONS.iter().enumerate() {
        run.dungeon.rooms[at].items.push(Item {
            card: card.to_string(),
            x: 16.0 + (i % 5) as f32 * 3.5,
            y: 9.0 + (i / 5) as f32 * 4.0,
            held_off: Some(1),
        });
    }
    let img = crate::stage::world_viz::overworld::arena::frame(&run, 384, 224);
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(dir.join("minion_cards.ppm"), out).unwrap();
}
