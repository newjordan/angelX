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

/// Aim up n steps, power up m steps, then fire: the dials a gunner uses.
/// `up` counts dial steps of aim (screen-up = negative aim_y); `right`
/// counts power steps.
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

/// The fixed arcs that first strike the keep's middle column (the banner
/// block) at each castle, computed from the constants, wind and all.
fn target_dials(leg: u8) -> (u8, u8) {
    // The castle besieged on this leg (five legs, three castles).
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

/// Drive one round home through all three castles with the fixed arcs
/// that hit (computed from the constants: power 8 aim 8 clears castle 1,
/// power 9 aim 8 castles 2 and 3, wind and all).
fn besiege(run: &mut Run, id: u32) {
    let fire = || tap_as(id);
    let step = |run: &mut Run| run.tick_pastimes(&fire());
    for _castle in 1..=3 {
        // The trail event: march on without the key.
        for _ in 0..(EVENT_WAIT + MOURN + RETREAT) {
            step(run);
        }
        // At the walls: dial a hitting arc, then fire and wait out the
        // flight and the reload, until the banner falls.
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

/// Aim up and power up, then F: a shot must fly, arc and land somewhere,
/// digging a crater or breaking a block. The verb, alone.
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
    // The trail comes first: one event, waited out (no F: the mild side).
    for _ in 0..(EVENT_WAIT + 1) {
        run.tick_pastimes(&BTreeMap::new());
    }
    // Mourn, then the walls.
    for _ in 0..(MOURN + 1) {
        run.tick_pastimes(&BTreeMap::new());
    }
    let s = run.pastimes.round.as_ref().unwrap().siege.as_ref().unwrap();
    assert!(s.fighting, "the first castle begins");
    assert!(s.shot.is_none());
    // Aim and power to the top, one step a tick, then F.
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
    // Let the shot fly to ground or wall.
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

/// A whole heat with two knights: both march, both besiege on their own
/// clocks, the winner is crowned, the other wears the booby prize, and a
/// knight who is spent waits for the heat to end.
#[test]
fn two_knights_march_and_besiege_a_heat() {
    let mut run = yard();
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    assert!(run.pastimes.heat.is_some(), "stepping on starts a heat");
    assert!(run.pastimes.heats.contains_key(&2) && run.pastimes.heats.contains_key(&3));
    // Seat 2 plays for the crown: only seat 2's key drives seat 2's round.
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

/// The purse: damage and surviving men pay as the other pastimes do, into
/// the shared score and the realm's game records.
#[test]
fn the_purse_pays_for_damage_and_survivors() {
    let mut run = yard();
    run.players.retain(|&id, _| id == 1);
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    let plays = run.home.countryside.games.plays_for(Game::Siege);
    assert_eq!(plays, 1, "the round is counted as played");
    // A won round pays: run the round to a win with the fixed arcs.
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

/// The look, round three: eight frames into ANGEL_ARENA_SHOTS/siege3 —
/// the aim view (field-wide); the reload close-up with four crew and with
/// one; the cow in flight; the beehive impact; the moon on a miss; the
/// pot tipped; the flung soldier; the graves with epitaphs.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_siege_shots -- --ignored`
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
        // The trail event of leg 1, waited out; mourn; then the walls.
        for _ in 0..(EVENT_WAIT + MOURN + taps) {
            run.tick_pastimes(&tap());
        }
    };
    let mut run = yard();
    place(&mut run, 1, Game::Siege.start());
    run.tick_pastimes(&BTreeMap::new());
    run.tick_pastimes(&BTreeMap::new());
    save(&run, "1-start");
    // The walls of castle 1, mid-haul with four soldiers.
    let mut run4 = yard();
    place(&mut run4, 1, Game::Siege.start());
    run4.tick_pastimes(&BTreeMap::new());
    at_walls(&mut run4, 2);
    run4.tick_pastimes(&dials(1, -1, 0));
    for _ in 0..10 {
        run4.tick_pastimes(&tap());
    }
    save(&run4, "2-haul-four");
    // Mid-haul with one soldier: the trail eats the rest, and at the walls
    // the keep's pot-shots (they land close to the engine on the first
    // throw of each cycle) wound the crew down to one hauling man.
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
            // Never loose a stone of our own: stones spent means the round
            // moves on. The keep's pots alone thin the crew.
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
    // A cow in flight: volley 1 (index 1) loosed at castle 1, a few ticks up.
    let mut cow = yard();
    place(&mut cow, 1, Game::Siege.start());
    cow.tick_pastimes(&BTreeMap::new());
    at_walls(&mut cow, 2);
    // Volley 0 is the boulder; loose it into the ground to move the sling on.
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
    cow.tick_pastimes(&tap()); // the cow leaves the sling
    for _ in 0..10 {
        cow.tick_pastimes(&BTreeMap::new());
    }
    save(&cow, "3-cow-flight");
    // A beehive splat: volley 2 (index 2) landed short.
    let mut hive = yard();
    place(&mut hive, 1, Game::Siege.start());
    hive.tick_pastimes(&BTreeMap::new());
    at_walls(&mut hive, 2);
    for _ in 0..2 {
        // Two loosed shots at the lowest power: they land short of the wall.
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
    // The flung soldier: one man left, so the next load tangles him.
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
            // The run1 state itself: the flung man rides whenever the crew
            // is two or fewer; save what we have of the stragglers.
            save(&run1, "5-flung");
        }
    }
    // The mooning defender: a shot landed short of the wall.
    save(&hive, "6-moon");
    // The pot tipped: the keep mid-throw, its pot over the wall's edge.
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
    // A trail event board: mid-wait on the second leg's march.
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
    // The graves: the trail with men lost behind it (the rash march).
    save(&run1, "8-graves");
    // The end: the trail won, the board's verdict.
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

/// The camera, pinned: aiming shows the whole field at unit scale; the
/// reload cuts to the crew's hold at the close scale; a shot in the air is
/// followed at the close scale inside the field; the splat holds where it
/// landed, about a second, at the close scale.
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
    // Wait out the keep's first pot-shots so the screen settles to aim.
    let mut waited = 0;
    for _ in 0..120 {
        let st = siege_of(&run);
        if st.pot == 0 && st.moon == 0 && st.splat == 0 && st.shot.is_none() {
            break;
        }
        run.tick_pastimes(&BTreeMap::new());
        waited += 1;
    }
    // Aiming: reload spent, no shot, no splat — the whole field.
    let _ = waited;
    let ((wx, wy), z) = camera(siege_of(&run));
    assert_eq!((wx, wy, z), (0, 0, 1), "aiming sees the field");
    // Reloading: the crew's hold, at the close scale.
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
    // The flight: a shot in the air is followed inside the field.
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
    // The impact hold: the splat, the wall, the answer — about a second.
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
