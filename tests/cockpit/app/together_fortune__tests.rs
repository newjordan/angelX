use super::*;
use crate::drive::together_realm::{Spoil, Spoils};
use crate::drive::together_shooter::home::{BUY_HOLD, Home};

fn home_with_wheel(level: u8) -> Home {
    let mut home = Home {
        deepest: 3,
        ..Home::default()
    };
    if level > 0 {
        home.levels.insert(Station::Wheel, level);
    }
    home
}

fn hold(fire: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(
        1,
        Input {
            fire,
            ..Input::default()
        },
    )])
}

/// A run in Fortune's hall with knight one on the lever.
fn at_the_lever(seed: u64, level: u8) -> Run {
    let mut run = Run::at_home(seed, 1, None, home_with_wheel(level), Spoils::default());
    run.enter_for_test(1);
    assert_eq!(run.room().kind, RoomKind::Fortune);
    let (c, r, w, h) = LEVER;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    run
}

fn pull(run: &mut Run) {
    for _ in 0..BUY_HOLD + 1 {
        run.step(&hold(true));
    }
    run.step(&hold(false));
}

#[test]
fn fortunes_hall_is_east_of_the_undercroft() {
    let mut run = Run::at_home(3, 1, None, Home::default(), Spoils::default());
    assert_eq!(
        run.dungeon.rooms.len(),
        4,
        "the Undercroft, Fortune's hall, the yard, the Trophy Hall"
    );
    assert!(run.valid_snapshot());
    // Walk out of the Undercroft's east door.
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (run.dungeon.rooms[0].width() - 0.3, 14.0);
    run.step(&BTreeMap::new());
    assert_eq!(run.room().kind, RoomKind::Fortune);
    assert!(run.at_home_now() && run.active());
}

#[test]
fn the_wheel_spins_once_a_delve_and_lands_on_an_open_wedge() {
    for seed in 0..24 {
        let mut run = at_the_lever(seed, 0);
        pull(&mut run);
        let spin = run.spin.expect("the lever spins the wheel");
        assert_eq!(run.mode, Mode::LongWayDown, "not until it stops");
        assert!(
            fortune::open_wedges(0).contains(&spin.wedge),
            "only opened wedges: {spin:?}"
        );
        for _ in 0..SPIN_TICKS {
            run.step(&BTreeMap::new());
        }
        assert_eq!(run.mode, Mode::WHEEL[usize::from(spin.wedge)]);
        assert_eq!(run.mode.rung(), 0);
        assert!(run.cues.iter().any(|c| c.starts_with("wheel:")));
        // At rest, the pointer is over the winning wedge.
        let rest = spin.angle(run.tick).rem_euclid(std::f32::consts::TAU);
        let w = std::f32::consts::TAU / Mode::WHEEL.len() as f32;
        let under = ((std::f32::consts::TAU - rest).rem_euclid(std::f32::consts::TAU) / w) as u8;
        assert_eq!(under, spin.wedge);
        // Once a delve.
        pull(&mut run);
        assert_eq!(run.spin.unwrap().started, spin.started);
    }
}

#[test]
fn fortunes_ladder_opens_the_wild_wedges() {
    assert_eq!(fortune::open_wedges(0).len(), 4);
    assert_eq!(fortune::open_wedges(1).len(), 6);
    assert_eq!(fortune::open_wedges(2).len(), 8);
    assert_eq!(fortune::open_wedges(3).len(), 11, "the third rung's night");
    assert_eq!(fortune::open_wedges(4).len(), 14, "the fourth rung");
    let landed: std::collections::BTreeSet<Mode> = (0..80)
        .map(|seed| {
            let mut run = at_the_lever(seed, 2);
            pull(&mut run);
            Mode::WHEEL[usize::from(run.spin.unwrap().wedge)]
        })
        .collect();
    assert!(landed.contains(&Mode::HoldTheStair) || landed.contains(&Mode::Gauntlet));
    assert!(landed.len() >= 6, "{landed:?}");
}

#[test]
fn fortunes_plate_orders_her_next_rung() {
    let mut run = Run::at_home(3, 1, None, home_with_wheel(0), Spoils::default());
    run.enter_for_test(1);
    let (c, r, w, h) = FORTUNE_PLATE;
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (r as f32 + h as f32 / 2.0) * TILE_UNITS;
    for _ in 0..BUY_HOLD + 1 {
        run.step(&hold(true));
    }
    assert_eq!(run.orders.len(), 1);
    assert_eq!(run.orders[0].station, Station::Wheel);
    assert!(run.spin.is_none(), "the plate is not the lever");
}

/// A delve under `mode`, down the stair onto floor one.
fn delve(mode: Mode) -> Run {
    let mut run = Run::at_home(5, 1, Some("Friend"), Home::default(), Spoils::default());
    run.mode = mode;
    run.descend_for_test();
    run
}

#[test]
fn giants_are_bigger_tougher_and_generous() {
    let mut plain = delve(Mode::LongWayDown);
    let mut giants = delve(Mode::GiantsFeast);
    plain.spawn_at_for_test(EnemyKind::Skeleton, 20.0, 10.0);
    giants.spawn_at_for_test(EnemyKind::Skeleton, 20.0, 10.0);
    let (p, g) = (
        plain.enemies.last().unwrap(),
        giants.enemies.last().unwrap(),
    );
    assert!(g.radius() > p.radius() * 1.3);
    assert!(g.max_hp > p.max_hp * 3 / 2);
    // A party leaving keeps the giant's share, as with any monster.
    giants.leave(2);
    let g = giants.enemies.last().unwrap();
    assert_eq!(g.max_hp, EnemyKind::Skeleton.hp() * 9 / 5);
}

#[test]
fn glass_jaw_doubles_every_hit_both_ways() {
    let mut run = delve(Mode::GlassJaw);
    assert!(run.players.values().all(|h| h.glass));
    let hero = run.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let before = hero.hp;
    hero.hurt(10);
    assert_eq!(before - hero.hp, 20);
    assert_eq!(hero.scaled(30), 60);
    let mut plain = delve(Mode::LongWayDown);
    assert!(plain.players.values().all(|h| !h.glass));
    let hero = plain.players.get_mut(&1).unwrap();
    hero.invulnerable = 0;
    let before = hero.hp;
    hero.hurt(10);
    assert_eq!(before - hero.hp, 10);
}

#[test]
fn the_collapse_brings_the_ceiling_down_after_its_time() {
    let mut run = delve(Mode::Collapse);
    run.calm_for_test();
    let secs = run.collapse_in().expect("a floor's time");
    assert_eq!(secs, Mode::collapse_secs(1));
    let at = run.collapse_at.unwrap();
    run.tick = at - 2;
    for _ in 0..3 * HZ {
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
            hero.invulnerable = 0;
        }
        run.step(&BTreeMap::new());
    }
    assert!(!run.rocks.is_empty(), "rocks fall past the floor's time");
    assert!(run.cues.iter().any(|c| c == "collapse"));
    // The stairs reset the clock.
    run.descend_for_test();
    assert_eq!(run.collapse_in(), Some(Mode::collapse_secs(2)));
    // Other delves never collapse.
    assert_eq!(delve(Mode::LongWayDown).collapse_in(), None);
}

#[test]
fn a_mimic_fair_leaves_a_chest_in_every_won_fight() {
    let mut run = delve(Mode::MimicFair);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(fight);
    run.clear_for_test();
    assert!(run.room().chest.is_some_and(|c| !c.open));
}

#[test]
fn hold_the_stair_is_a_horde_in_one_great_hall() {
    let run = delve(Mode::HoldTheStair);
    assert_eq!(run.dungeon.rooms.len(), 2);
    let hall = &run.dungeon.rooms[1];
    assert_eq!(hall.kind, RoomKind::Fight);
    assert!(hall.great());
    let mut stairs = 0;
    for r in 0..hall.rows as i32 {
        for c in 0..hall.cols as i32 {
            stairs += usize::from(hall.tile(c, r) == Tile::Stairs);
        }
    }
    assert_eq!(stairs, 4, "the way down is in the middle of the horde");
    let mut run = run;
    run.enter_for_test(1);
    assert_eq!(run.waves.total, 5, "a horde's waves on floor one");
    // The last floor is the lair, with waves of its own.
    run.descend_for_test();
    run.descend_for_test();
    assert_eq!(run.dungeon.rooms[1].kind, RoomKind::Lair);
}

#[test]
fn the_gauntlet_goes_straight_to_the_guardian() {
    let mut run = delve(Mode::Gauntlet);
    let kinds: Vec<RoomKind> = run.dungeon.rooms.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![RoomKind::Start, RoomKind::Hall, RoomKind::Stairs]
    );
    run.descend_for_test();
    assert_eq!(run.dungeon.rooms[0].kind, RoomKind::Sanctuary);
    run.descend_for_test();
    assert_eq!(run.dungeon.rooms[2].kind, RoomKind::Lair);
    assert!(run.valid_snapshot());
}

#[test]
fn riskier_delves_pay_more_spoils() {
    let mut spoils = Spoils::default();
    spoils.add(Spoil::Gold, 101);
    spoils.add(Spoil::Bone, 3);
    let doubled = spoils.scaled(Mode::Collapse.spoils());
    assert_eq!(doubled.get(Spoil::Gold), 202);
    assert_eq!(doubled.get(Spoil::Bone), 6);
    let half_again = spoils.scaled(Mode::LightsOut.spoils());
    assert_eq!(half_again.get(Spoil::Gold), 152);
    assert_eq!(half_again.get(Spoil::Bone), 5);
    let mut run = delve(Mode::GlassJaw);
    run.players.get_mut(&1).unwrap().carried = spoils;
    run.retreat();
    let banked = &run.bank[0].spoils;
    assert_eq!(banked.get(Spoil::Gold), 100, "half of 101, doubled");
}

#[test]
fn the_mirror_carries_the_wheel_and_its_mode() {
    let mut run = at_the_lever(9, 1);
    pull(&mut run);
    let live = run.live();
    let mut friend: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    friend.spin = None;
    friend.mode = Mode::LongWayDown;
    assert!(friend.apply_live(live));
    assert_eq!(friend.spin, run.spin);
    assert_eq!(friend.home, run.home);
}

#[test]
fn turbo_runs_the_delve_a_third_faster_but_not_at_home() {
    let ticks = |mut run: Run| {
        let start = run.tick;
        for _ in 0..30 {
            run.step(&BTreeMap::new());
        }
        run.tick - start
    };
    assert_eq!(ticks(delve(Mode::Turbo)), 40);
    assert_eq!(ticks(delve(Mode::LongWayDown)), 30);
    let mut home = Run::at_home(5, 1, None, Home::default(), Spoils::default());
    home.mode = Mode::Turbo;
    assert_eq!(ticks(home), 30, "the Undercroft keeps its own pace");
    assert_eq!(Mode::Turbo.spoils(), (2, 1));
}

#[test]
fn rune_rush_wells_a_rune_in_every_fight_and_another_once_it_is_taken() {
    let mut run = delve(Mode::RuneRush);
    let fights: Vec<usize> = (0..run.dungeon.rooms.len())
        .filter(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight)
        .collect();
    assert!(!fights.is_empty());
    for &room in &fights {
        run.enter_for_test(room);
        assert!(run.rune.is_some(), "a rune in every fight");
    }
    // Take it; a fresh one comes while the fight goes on.
    run.calm_for_test();
    let rune = run.rune.unwrap();
    while run.tick < rune.at {
        run.step(&BTreeMap::new());
    }
    for hero in run.players.values_mut() {
        hero.hp = hero.max_hp;
        hero.invulnerable = 1000;
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (rune.x, rune.y);
    run.step(&BTreeMap::new());
    assert!(run.rune.is_none(), "taken");
    for _ in 0..12 * HZ {
        if run.rune.is_some() {
            break;
        }
        for hero in run.players.values_mut() {
            (hero.hp, hero.invulnerable) = (hero.max_hp, 1000);
        }
        run.step(&BTreeMap::new());
    }
    assert!(
        run.rune.is_some() || run.phase != Phase::Fighting,
        "another while the fight lasts"
    );
}

#[test]
fn all_random_hands_every_knight_a_strangers_ultimate_half_charged() {
    use crate::drive::together_shooter::ults::{ULT_FULL, Ult};
    let run = delve(Mode::AllRandom);
    for hero in run.players.values() {
        let dealt = hero.ult.expect("a stranger's ultimate");
        assert_ne!(dealt, Ult::of(hero.knight.as_deref()));
        assert_eq!(hero.ult(), dealt);
        assert!(hero.ult_charge >= ULT_FULL / 2);
    }
    assert!(run.cues.iter().any(|c| c == "all_random"));
    let mut dealt = std::collections::BTreeSet::new();
    for seed in 0..40 {
        let mut run = Run::at_home(seed, 1, None, Home::default(), Spoils::default());
        run.mode = Mode::AllRandom;
        run.descend_for_test();
        dealt.insert(format!("{:?}", run.players[&1].ult()));
    }
    assert!(dealt.len() >= 7, "{dealt:?}");
    let run = delve(Mode::LongWayDown);
    for hero in run.players.values() {
        assert_eq!(hero.ult, None, "any other way down, their own");
        assert_eq!(hero.ult_charge, 0);
    }
}

#[test]
fn ironman_takes_every_second_wind_away() {
    let mut home = Home::default();
    home.levels.insert(Station::Chapel, 2);
    home.residents.insert("anselm".into());
    home.round = Some("ale".into());
    let down = |mode: Mode| {
        let mut run = Run::at_home(5, 1, Some("Friend"), home.clone(), Spoils::default());
        run.mode = mode;
        run.descend_for_test();
        let winds: Vec<u8> = run.players.values().map(|h| h.winds).collect();
        run.descend_for_test();
        (
            winds,
            run.players.values().map(|h| h.winds).collect::<Vec<u8>>(),
        )
    };
    let (top, next) = down(Mode::Ironman);
    assert!(top.iter().all(|&w| w == 0), "none at the top: {top:?}");
    assert!(
        next.iter().all(|&w| w == 0),
        "none on the next floor: {next:?}"
    );
    let (top, _) = down(Mode::LongWayDown);
    assert!(
        top.iter().all(|&w| w >= 2),
        "the chapel, Anselm and the ale: {top:?}"
    );
    assert_eq!(Mode::Ironman.spoils(), (3, 1));
}

#[test]
fn hollow_walls_hides_a_vault_on_every_floor() {
    for seed in 0..12u64 {
        let mut run = Run::at_home(seed, 1, None, Home::default(), Spoils::default());
        run.mode = Mode::HollowWalls;
        for _ in 1..DEEPEST {
            run.descend_for_test();
            assert!(
                run.dungeon.secret.is_some(),
                "seed {seed} floor {} keeps a vault",
                run.dungeon.depth
            );
        }
    }
}

#[test]
fn sponsors_night_doubles_the_audience() {
    let crowd = |mode: Mode| {
        let mut run = delve(mode);
        let before = run.audience;
        run.thrill_for_test(100);
        run.audience - before
    };
    assert_eq!(crowd(Mode::Sponsors), 2 * crowd(Mode::LongWayDown));
}
