//! The Siege Trail: everyone marches, everyone besieges.
//! Verb tests, a whole two-knight heat, and the purse.

use super::*;
use crate::rules::pastimes::{EVENT_WAIT, FIELD_W, MOURN, RETREAT};

fn yard() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    for id in 2..=3 {
        run.join_seat(id, &format!("Seat {id}"));
    }
    let room = world::room_of(&run.dungeon, RoomKind::Pasture).unwrap();
    run.arrive(room, (12.0, 11.0));
    run
}

fn place(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn tap() -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { fire: true, ..Default::default() })])
}

fn tap_as(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { fire: true, ..Default::default() })])
}

fn dials(id: u32, up: i32, right: i32) -> BTreeMap<u32, Input> {
    let (ay, ax) = (up.clamp(-1, 1) as i8, right.clamp(-1, 1) as i8);
    BTreeMap::from([(
        id,
        Input {
            aim_y: ay,
            move_y: ay,
            aim_x: ax,
            move_x: ax,
            ..Default::default()
        },
    )])
}

fn target_dials(leg: u8) -> (u8, u8) {
    let castle = match leg {
        1..=2 => 1,
        3..=4 => 2,
        _ => 3,
    };
    match castle {
        1 => (10, 7),
        2 => (9, 8),
        _ => (9, 9),
    }
}

fn besiege(run: &mut Run, id: u32) {
    let fire = || tap_as(id);
    let step = |run: &mut Run| run.tick_pastimes(&fire());
    for _castle in 1..=3 {
        for _ in 0..(EVENT_WAIT + MOURN + RETREAT) {
            step(run);
        }
        let mut guard = 0u32;
        loop {
            let round = run
                .pastimes
 .round
                .as_ref()
                .or_else(|| run.pastimes.heats.get(&id).map(|_| unreachable!()))
                .cloned();
            let Some(r) = round else { break };
            let s = r.siege.as_ref().unwrap();
            if s.cheer > 0 || s.retreat > 0 || !s.fighting {
                step(run);
                guard += 1;
            } else if s.aim < 8 {
                run.tick_pastimes(&dials(id, 1, 0));
            } else if s.power < 8 {
                run.tick_pastimes(&dials(id, 0, 1));
            } else if s.shot.is_none() && s.reload == 0 && s.stones > 0 {
                step(run);
            } else {
                step(run);
            }
            guard += 1;
            if guard > 240 * HZ {
                break;
            }
        }
    }
}

#[test]
fn the_engine_looses_aims_and_digs_craters() {
    let mut run = yard();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.round.is_some(), "the ring starts a round");
    let round = run.pastimes.round.as_ref().unwrap();
    assert_eq!(round.game, Game::Siege);
    assert!(round.siege.is_some());
    for _ in 0..(EVENT_WAIT + 1) {
        run.tick_pastimes(&BTreeMap::new());
    }
    for _ in 0..(MOURN + 1) {
        run.tick_pastimes(&BTreeMap::new());
    }
    let s = run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap();
    assert!(s.fighting, "the first castle begins");
    assert!(s.shot.is_none());
    for _ in 0..10 {
        run.tick_pastimes(&BTreeMap::from([(
            1,
            Input {
                aim_y: 1,
                move_x: 1,
                ..Default::default()
            },
        )]));
    }
    run.tick_pastimes(&tap());
    let s = run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap();
    assert!(s.shot.is_some() || s.reload > 0, "the engine loosed");
    let before: u16 = run
        .pastimes
        .round
        .as_ref()
        .unwrap()
        .siege
        .as_ref()
        .unwrap()
        .craters
        .iter()
        .map(|&c| u16::from(c))
        .sum();
    for _ in 0..(5 * HZ) {
        run.tick_pastimes(&BTreeMap::new());
        let s = run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap();
        if s.shot.is_none() {
            break;
        }
    }
    let s = run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap();
    let after: u16 = s.craters.iter().map(|&c| u16::from(c)).sum();
    assert!(
        after > before || s.damage > 0,
        "the shot lands: a crater or a broken block"
    );
    assert!(run.valid_snapshot());
}

#[test]
fn two_knights_march_and_besiege_a_heat() {
    let mut run = yard();
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.heat.is_some(), "stepping on starts a heat");
    assert!(run.pastimes.heats.contains_key(&2) && run.pastimes.heats.contains_key(&3));
    let mut ticks = 0u32;
    while run.pastimes.heat.is_some() && ticks < 240 * HZ {
        let s = run
            .pastimes
            .heats
            .get(&2)
            .and_then(|r| r.siege.as_ref());
        let mine = s.map(|s| (s.fighting, s.leg, s.aim, s.power, s.reload, s.shot.is_some(), s.cheer, s.retreat, s.mourn));
        let input = match mine {
            Some((true, leg, aim, power, reload, in_air, cheer, retreat, _)) if cheer == 0 && retreat == 0 => {
                let (want_aim, want_power) = target_dials(leg);
                if aim < want_aim {
                    dials(2, -1, 0)
                } else if power < want_power {
                    dials(2, 0, 1)
                } else if reload > 0 || in_air {
                    BTreeMap::new()
                } else {
                    tap_as(2)
                }
            }
            _ => tap_as(2),
        };
        run.tick_pastimes(&input);
        ticks += 1;
    }
    assert!(ticks < 240 * HZ, "the heat ends inside the round clock");
    assert!(run.pastimes.round.is_none() && run.pastimes.heats.is_empty());
    let (_, _, said) = run.found.clone().unwrap();
    assert!(said.contains("SEAT 2 WINS"), "{said}");
    assert!(run.players[&2].laurel, "the winner wears the laurel");
    assert!(run.players[&1].booby.is_some() && run.players[&3].booby.is_some());
    assert!(run.valid_snapshot());
}

#[test]
fn the_purse_pays_for_damage_and_survivors() {
    let mut run = yard();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    let plays = run.home.countryside.games.plays_for(Game::Siege);
    assert_eq!(plays, 1, "the round is counted as played");
    let mut ticks = 0u32;
    while run.pastimes.round.is_some() && ticks < 240 * HZ {
        let mine = run
            .pastimes
            .round
            .as_ref()
            .and_then(|r| r.siege.as_ref())
            .map(|s| (s.fighting, s.leg, s.aim, s.power, s.reload, s.shot.is_some(), s.cheer, s.retreat, s.mourn));
        let input = match mine {
            Some((true, leg, aim, power, reload, in_air, cheer, retreat, _)) if cheer == 0 && retreat == 0 => {
                let (want_aim, want_power) = target_dials(leg);
                if aim < want_aim {
                    dials(1, -1, 0)
                } else if power < want_power {
                    dials(1, 0, 1)
                } else if reload > 0 || in_air {
                    BTreeMap::new()
                } else {
                    tap()
                }
            }
            _ => tap(),
        };
        run.tick_pastimes(&input);
        ticks += 1;
    }
    let said = run
        .found
        .clone()
        .map(|(_, _, s)| s)
        .unwrap_or_default();
    let score = run.score;
    assert!(score > 0, "the purse pays into the shared score: {said}");
    let rec = &run.home.countryside.games;
    assert!(rec.plays_for(Game::Siege) >= 1);
    assert!(run.valid_snapshot());
}

#[test]
#[ignore]
fn write_siege_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("siege3");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = crate::render::overworld::arena::frame_v1(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let at_walls = |run: &mut Run, taps: u32| {
        for _ in 0..(EVENT_WAIT + MOURN + taps) {
            run.tick_pastimes(&tap());
        }
    };
    let mut run = yard();
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    run.tick_pastimes(&BTreeMap::new());
    save(&run, "1-start");
    let mut run4 = yard();
    place(&mut run4, 1, Game::Siege.start());
    run4.tick_pastimes(&BTreeMap::new());
    at_walls(&mut run4, 2);
    run4.tick_pastimes(&dials(1, -1, 0));
    for _ in 0..10 {
        run4.tick_pastimes(&tap());
    }
    save(&run4, "2-haul-four");
    let mut run1 = yard();
    place(&mut run1, 1, Game::Siege.start());
    run1.tick_pastimes(&BTreeMap::new());
    at_walls(&mut run1, 2);
    {
        let mut guard = 0u32;
        loop {
            let n = run1
                .pastimes
                .round
                .as_ref()
                .and_then(|r| r.siege.as_ref())
                .map(|s| (s.soldiers, s.fighting, s.stones))
                .unwrap_or((0, false, 0));
            if n.0 <= 1 && n.1 {
                break;
            }
            run1.tick_pastimes(&BTreeMap::new());
            guard += 1;
            if guard > 240 * crate::rules::HZ {
                break;
            }
        }
        let s = run1.pastimes.round.as_ref().and_then(|r| r.siege.as_ref());
        eprintln!(
            "2-haul-one: soldiers={} leg={} fighting={} guard={}",
            s.map(|s| s.soldiers).unwrap_or(0),
            s.map(|s| s.leg).unwrap_or(0),
            s.map(|s| s.fighting).unwrap_or(false),
            guard
        );
    }
    save(&run1, "2-haul-one");
    let mut cow = yard();
    place(&mut cow, 1, Game::Siege.start());
    cow.tick_pastimes(&BTreeMap::new());
    at_walls(&mut cow, 2);
    for _ in 0..8 {
        cow.tick_pastimes(&dials(1, -1, 0));
    }
    cow.tick_pastimes(&tap());
    for _ in 0..22 {
        cow.tick_pastimes(&BTreeMap::new());
    }
    for _ in 0..10 {
        cow.tick_pastimes(&dials(1, 0, 1));
    }
    cow.tick_pastimes(&tap());
    for _ in 0..10 {
        cow.tick_pastimes(&BTreeMap::new());
    }
    save(&cow, "3-cow-flight");
    let mut hive = yard();
    place(&mut hive, 1, Game::Siege.start());
    hive.tick_pastimes(&BTreeMap::new());
    at_walls(&mut hive, 2);
    for _ in 0..2 {
        for _ in 0..10 {
            hive.tick_pastimes(&dials(1, 0, -1));
        }
        hive.tick_pastimes(&tap());
        for _ in 0..24 {
            hive.tick_pastimes(&BTreeMap::new());
        }
    }
    for _ in 0..10 {
        hive.tick_pastimes(&dials(1, 0, -1));
    }
    hive.tick_pastimes(&tap());
    for _ in 0..10 {
        hive.tick_pastimes(&BTreeMap::new());
    }
    for _ in 0..6 {
        hive.tick_pastimes(&BTreeMap::new());
    }
    save(&hive, "4-hive-splat");
    {
        let s = run1
            .pastimes
            .round
            .as_ref()
            .and_then(|r| r.siege.as_ref());
        if s.map(|s| s.fighting).unwrap_or(false) && s.map(|s| s.soldiers).unwrap_or(0) <= 2 {
            let mut flung = run1.clone();
            for _ in 0..8 {
                flung.tick_pastimes(&dials(1, -1, 0));
            }
            flung.tick_pastimes(&tap());
            for _ in 0..12 {
                flung.tick_pastimes(&BTreeMap::new());
            }
            save(&flung, "5-flung");
        } else {
            save(&run1, "5-flung");
        }
    }
    save(&hive, "6-moon");
    {
        let mut pot = yard();
        place(&mut pot, 1, Game::Siege.start());
        pot.tick_pastimes(&BTreeMap::new());
        at_walls(&mut pot, 2);
        let mut guard = 0u32;
        loop {
            let st = pot
                .pastimes
                .round
                .as_ref()
                .and_then(|r| r.siege.as_ref());
            if st.map(|s| s.pot > 0 && s.castle_shot.is_some()).unwrap_or(false)
                || guard > 200
            {
                break;
            }
            pot.tick_pastimes(&BTreeMap::new());
            guard += 1;
        }
        save(&pot, "6b-pot-tipped");
    }
    let mut board = yard();
    place(&mut board, 1, Game::Siege.start());
    board.tick_pastimes(&BTreeMap::new());
    for _ in 0..(EVENT_WAIT + MOURN + RETREAT) {
        board.tick_pastimes(&tap());
    }
    for _ in 0..6 {
        board.tick_pastimes(&BTreeMap::new());
    }
    save(&board, "7-board");
    save(&run1, "8-graves");
    let mut end = yard();
    place(&mut end, 1, Game::Siege.start());
    end.tick_pastimes(&BTreeMap::new());
    besiege(&mut end, 1);
    let mut ticks = 0u32;
    while end.pastimes.round.is_some() && ticks < 240 * crate::rules::HZ {
        end.tick_pastimes(&tap());
        ticks += 1;
    }
    assert!(
        end.found_line().is_some_and(|l| l.contains("WINS THE SIEGE TRAIL")),
        "the trail's verdict is announced: {:?}",
        end.found_line()
    );
    save(&end, "9-end");
}

#[test]
fn the_camera_follows_the_action_and_holds_named_spots() {
    use crate::render::overworld::arena::siege_stage::{
        camera, HOLD_IMPACT_X, HOLD_RELOAD_X, HOLD_Y, ZOOM,
    };
    let mut run = yard();
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    for _ in 0..(EVENT_WAIT + MOURN + 2) {
        run.tick_pastimes(&tap());
    }
    fn siege_of(r: &Run) -> &Siege {
        r.pastimes
            .round
            .as_ref()
            .and_then(|round| round.siege.as_ref())
            .unwrap()
    }
    let at_walls = siege_of(&run);
    assert!(at_walls.fighting, "the walls are up");
    let mut waited = 0;
    for _ in 0..120 {
        let st = siege_of(&run);
        if st.pot == 0 && st.moon == 0 && st.splat == 0 && st.shot.is_none() {
            break;
        }
        run.tick_pastimes(&BTreeMap::new());
        waited += 1;
    }
    let _ = waited;
    let ((wx, wy), z) = camera(siege_of(&run));
    assert_eq!((wx, wy, z), (0, 0, 1), "aiming sees the field");
    for _ in 0..10 {
        run.tick_pastimes(&tap());
    }
    let st = siege_of(&run);
    if st.reload > 0 {
        let ((wx, wy), z) = camera(st);
        assert_eq!(z, ZOOM, "the reload cuts close");
        let win_w = FIELD_W / ZOOM;
        assert!(
            wx <= HOLD_RELOAD_X && HOLD_RELOAD_X < wx + win_w,
            "the reload hold {HOLD_RELOAD_X} is in the window {wx}..{}",
            wx + win_w
        );
        assert_eq!(wy, HOLD_Y);
    }
    let mut flown = run.clone();
    for _ in 0..8 {
        flown.tick_pastimes(&dials(1, -1, 0));
    }
    flown.tick_pastimes(&tap());
    for _ in 0..6 {
        flown.tick_pastimes(&BTreeMap::new());
    }
    let st = siege_of(&flown);
    if let Some(sh) = &st.shot {
        let ((wx, _), z) = camera(st);
        assert_eq!(z, ZOOM, "the flight is followed close");
        let win_w = FIELD_W / ZOOM;
        assert!(
            wx <= sh.x / 16 && sh.x / 16 < wx + win_w,
            "the shot {} is in the window {wx}..{}",
            sh.x / 16,
            wx + win_w
        );
    }
    let mut landed = flown;
    for _ in 0..40 {
        if siege_of(&landed).shot.is_none() {
            break;
        }
        landed.tick_pastimes(&BTreeMap::new());
    }
    let st = siege_of(&landed);
    let held = st.splat > 0 || st.moon > 0 || st.cheer > 0;
    assert!(
        held,
        "the landing holds: splat={} moon={} cheer={}",
        st.splat, st.moon, st.cheer
    );
    let ((wx, wy), z) = camera(st);
    assert_eq!((wy, z), (HOLD_Y, ZOOM), "the impact holds close");
    let win_w = FIELD_W / ZOOM;
    let centre = if st.splat_miss { st.splat_x } else { HOLD_IMPACT_X };
    assert!(
        wx <= centre && centre < wx + win_w,
        "the landing {centre} is in the window {wx}..{}",
        wx + win_w
    );
}
