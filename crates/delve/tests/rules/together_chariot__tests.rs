use super::*;
use crate::realm::chivalry::{Horse, Stable, bloodline};

fn horse() -> Horse {
    Horse {
        name: "Testrunner".into(),
        house: "qwen".into(),
        blood: bloodline("qwen"),
        ..Horse::default()
    }
}

fn race(seed: u64) -> Chariot {
    Chariot::field(1, &horse(), &Stable::default(), seed)
}

#[test]
fn the_field_is_four_chariots_with_the_players_horse_first() {
    let race = race(7);
    assert_eq!(race.racers.len(), 4);
    assert_eq!(race.player().name, "Testrunner");
    assert_eq!(race.player().house, "qwen");
    assert!(race.racers[1..].iter().all(|r| r.house != "qwen"));
    assert_eq!(race.phase, Phase::Gate);
    assert!(race.call().contains("CARNAGE"));
}

#[test]
fn a_race_runs_three_laps_and_pays_every_finisher_a_place() {
    let mut race = race(11);
    let mut finished = Vec::new();
    for _ in 0..30_000 {
        for event in race.tick(Input::default()) {
            if let Event::Finished(_, place) = event {
                finished.push(place);
            }
        }
        if race.phase == Phase::Done {
            break;
        }
    }
    assert_eq!(race.phase, Phase::Done, "the race ended");
    assert!(race.racers.iter().all(|r| r.place.is_some()));
    finished.sort_unstable();
    assert_eq!(finished, [1, 2, 3, 4]);
}

#[test]
fn the_same_seed_and_inputs_run_the_same_race() {
    let (mut a, mut b) = (race(99), race(99));
    let input = Input {
        dash: true,
        move_x: 1,
        ..Input::default()
    };
    for _ in 0..2_000 {
        a.tick(input);
        b.tick(input);
    }
    assert_eq!(a.racers, b.racers);
}

#[test]
fn the_whip_is_speed_bought_with_wind() {
    let mut whipped = race(5);
    let mut honest = race(5);
    let whip = Input {
        dash: true,
        ..Input::default()
    };
    for _ in 0..(GATE_TICKS + 300) {
        whipped.tick(whip);
        honest.tick(Input::default());
    }
    assert!(whipped.player().wind < honest.player().wind);
    assert!(whipped.player().dist > honest.player().dist);
}

#[test]
fn a_caltrop_waits_for_the_chariot_behind() {
    let mut race = race(3);
    let drop = Input {
        bomb: true,
        ..Input::default()
    };
    for _ in 0..=GATE_TICKS {
        race.tick(Input::default());
    }
    race.tick(drop);
    assert_eq!(race.traps.len(), 1);
    assert_eq!(race.player().caltrops, 2);
    // Put a rival right on the nail and let the tick fall.
    race.racers[1].dist = race.traps[0].0;
    race.racers[1].lane = race.traps[0].1;
    let events = race.tick(Input::default());
    assert!(events.contains(&Event::CaltropHit(1)));
    assert!(race.racers[1].wrecked > 0);
}

#[test]
fn rivals_drop_iron_for_whoever_rides_their_tail() {
    // The player never touches the caltrop bag, so any drop is a rival's.
    let mut drops = 0;
    for seed in [42u64, 7, 99] {
        let mut race = race(seed);
        for _ in 0..30_000 {
            drops += race
                .tick(Input::default())
                .iter()
                .filter(|e| **e == Event::CaltropDropped)
                .count();
            if race.phase == Phase::Done {
                break;
            }
        }
        assert_eq!(race.phase, Phase::Done);
    }
    assert!(drops >= 1, "some rival scattered iron across three races");
    assert!(drops <= 27, "but the track is not a nail bed: {drops}");
}

#[test]
fn corners_cap_the_pace_that_straights_allow() {
    let mut race = race(4);
    for _ in 0..=GATE_TICKS {
        race.tick(Input::default());
    }
    // In the middle of this course's first real bend, arriving far too fast.
    let course = race.course();
    race.racers[0].dist = (0..400)
        .map(|k| k as f32 * course.lap / 400.0)
        .find(|&d| bend_on(course, d) > CORNER_BEND)
        .expect("every course has a corner");
    assert!(race.racers[0].cornering(course), "the bend is a corner");
    race.racers[0].speed = 30.0;
    race.tick(Input::default());
    assert!(
        race.racers[0].speed < 30.0 - 0.1,
        "the corner took the speed off"
    );
}

#[test]
fn the_race_stage_stamps_the_visible_field_and_dots_the_rest_on_the_minimap() {
    use crate::rules::home::{BUY_HOLD, Home};
    use crate::rules::world;
    use crate::render::overworld::arena;
    use crate::render::overworld::ink::ink;
    let mut stable = Stable::default();
    stable.herd.push(horse());
    let home = Home {
        stable,
        ..Home::default()
    };
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(hip, (20.0, 10.5));
    run.step(&BTreeMap::new());
    let interact = BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Input::default()
        },
    )]);
    for _ in 0..BUY_HOLD + 2 {
        run.step(&interact);
    }
    assert!(run.chariot.is_some(), "boarded at the coach plate");
    for _ in 0..GATE_TICKS + 30 {
        run.step(&BTreeMap::new());
    }
    // String the field out so the minimap dots do not sit on each other.
    for (i, racer) in run.chariot.as_mut().unwrap().racers.iter_mut().enumerate() {
        racer.dist += i as f32 * 90.0;
    }
    let raced = arena::frame(&run, 384, 224);
    // On the course stage the camera rides the player, so a racer's box
    // sits at its course position less the camera's origin.
    let race = run.chariot.as_ref().unwrap();
    let course = race.course();
    let (camx, camy) = arena::race_camera(race, 384, 224);
    let mut seen = 0;
    for racer in &race.racers {
        let (tx, ty) = pos_on(course, racer.dist, racer.lane);
        let (cx, cy) = ((tx * 16.0) as i32 - camx, (ty * 16.0) as i32 - camy);
        if !(24..360).contains(&cx) || !(24..200).contains(&cy) {
            continue;
        }
        seen += 1;
        let in_box = |want: Option<[u8; 3]>| {
            (-24..24)
                .flat_map(|dy| (-24..24).map(move |dx| (dx, dy)))
                .filter(|&(dx, dy)| raced.get(cx + dx, cy + dy) == want)
                .count()
        };
        assert!(
            in_box(ink('B')) > 10,
            "{} has its coat at its course position",
            racer.name
        );
        assert!(
            in_box(ink('O')) > 0,
            "{} has its gold harness at its course position",
            racer.name
        );
    }
    assert!(seen >= 2, "at least the player and a rival share the view: {seen}");
    // Whoever the camera left behind is dotted on the minimap
    // (bottom-right box), the player in '9' and each rival in its banner ink.
    let banner = |house: &str| match house {
        "qwen" => ink('z'),
        "grok" => ink('A'),
        "glm" => ink('5'),
        "deepseek" => ink('1'),
        _ => ink('R'),
    };
    for (i, racer) in race.racers.iter().enumerate() {
        let want = if i == 0 { ink('9') } else { banner(&racer.house) };
        let dots = (154..220)
            .flat_map(|y| (276..380).map(move |x| (x, y)))
            .filter(|&(x, y)| raced.get(x, y) == want)
            .count();
        assert!(dots >= 2, "{} is dotted on the minimap: {dots}", racer.name);
    }
}

#[test]
fn a_finished_race_settles_onto_the_horses_record() {
    let mut stable = Stable::default();
    stable.herd.push(horse());
    let marks = BTreeMap::from([("chariot:1:Testrunner".to_string(), 1u32)]);
    let (said, gold) = settle_chariot(&mut stable, &marks);
    assert_eq!(gold, PURSE[0]);
    assert_eq!(stable.herd[0].starts, 1);
    assert_eq!(stable.herd[0].wins, 1);
    assert_eq!(stable.herd[0].earned, PURSE[0]);
    assert!(said[0].1.contains("Testrunner"));
}

fn boarded_run() -> Run {
    use crate::rules::home::{BUY_HOLD, Home};
    use crate::rules::world;
    let mut stable = Stable::default();
    stable.herd.push(horse());
    let home = Home {
        stable,
        ..Home::default()
    };
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(hip, (20.0, 10.5));
    run.step(&BTreeMap::new());
    let interact = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 2 {
        run.step(&interact);
    }
    assert!(run.chariot.is_some(), "hold E at the coach plate boards");
    run
}

fn keys(move_x: i8, dash: bool, bomb: bool) -> BTreeMap<u32, Input> {
    BTreeMap::from([(1, Input { move_x, dash, bomb, ..Input::default() })])
}

#[test]
fn a_full_race_through_the_public_controls() {
    let mut run = boarded_run();
    assert_eq!(run.chariot.as_ref().unwrap().call(), "LET THE CARNAGE BEGIN!");
    // Through the gate, then A pulls to the rail and D pushes back out.
    for _ in 0..GATE_TICKS + 5 {
        run.step(&keys(0, false, false));
    }
    assert_ne!(run.chariot.as_ref().unwrap().call(), "LET THE CARNAGE BEGIN!");
    for _ in 0..10 {
        run.step(&keys(-1, false, false));
    }
    let in_lane = run.chariot.as_ref().unwrap().player().lane;
    for _ in 0..20 {
        run.step(&keys(1, false, false));
    }
    let out_lane = run.chariot.as_ref().unwrap().player().lane;
    assert!(in_lane < -0.6, "A steered in: {in_lane}");
    assert!(out_lane > in_lane + 0.3, "D steered out: {out_lane}");
    // The whip spends wind; rested on the straight, wind comes back.
    for _ in 0..240 {
        run.step(&keys(0, true, false));
    }
    let blown = run.chariot.as_ref().unwrap().player().wind;
    assert!(blown < 0.9, "whipping spent the wind: {blown}");
    let mut recovered = blown;
    for _ in 0..600 {
        run.step(&keys(0, false, false));
        let r = run.chariot.as_ref().unwrap();
        if !r.player().cornering(r.course()) {
            // A few strides into the straight, so the resting ticks land.
            for _ in 0..30 {
                run.step(&keys(0, false, false));
            }
            recovered = run.chariot.as_ref().unwrap().player().wind;
            break;
        }
    }
    assert!(recovered > blown, "wind recovered on the straight: {blown} -> {recovered}");
    // X drops a caltrop behind the chariot.
    run.step(&keys(0, false, true));
    {
        let r = run.chariot.as_ref().unwrap();
        assert_eq!(r.player().caltrops, 2);
        assert_eq!(r.traps.len(), 1, "the caltrop waits on the track");
    }
    // Ride it out: three laps, a place, and the purse settled from marks.
    let mut place = None;
    for _ in 0..40_000 {
        run.step(&keys(0, false, false));
        match &run.chariot {
            None => break,
            Some(r) if r.player().place.is_some() => place = r.player().place,
            _ => {}
        }
    }
    let place = place.expect("the player finished");
    assert!(run.chariot.is_none(), "the result stood and the driver stepped down");
    let mut stable = run.home.stable.clone();
    let (said, gold) = settle_chariot(&mut stable, &run.marks);
    assert_eq!(gold, PURSE[(place - 1) as usize], "place {place} pays its purse");
    assert_eq!(stable.herd[0].starts, 1);
    assert!(!said.is_empty());
}

#[test]
fn shouldering_in_a_corner_gives_the_rail_to_nerve() {
    let mut race = race(23);
    for _ in 0..=GATE_TICKS {
        race.tick(Input::default());
    }
    race.racers[0].blood.nerve = 9;
    race.racers[1].blood.nerve = 3;
    // Wherever this seed's course first bends hard, meet the rival there.
    let course = race.course();
    let bendy = (0..400)
        .map(|k| k as f32 * course.lap / 400.0)
        .find(|&d| bend_on(course, d) > CORNER_BEND)
        .expect("every course has a corner");
    race.racers[0].dist = bendy;
    race.racers[1].dist = bendy;
    race.racers[0].lane = race.racers[1].lane;
    race.racers[1].speed = 6.0;
    let events = race.tick(Input::default());
    assert!(events.contains(&Event::Shoulder(1)), "{events:?}");
    assert!(race.racers[1].speed < 6.0, "the rival gave ground");
}

#[test]
fn the_heading_follows_the_oval_so_corners_do_not_ride_sideways() {
    let mut race = race(7);
    let at = |race: &mut Chariot, around: f32| {
        race.racers[0].dist = LAP * around;
        heading(&race.racers[0])
    };
    assert_eq!(at(&mut race, 0.0), Heading::South, "down the east end, at the viewer");
    assert_eq!(at(&mut race, 0.25), Heading::West, "along the south straight");
    assert_eq!(at(&mut race, 0.5), Heading::North, "up the west end, away");
    assert_eq!(at(&mut race, 0.75), Heading::East, "along the north straight");
    // A caltrop dropped just past the line wraps to just before it.
    assert_eq!(track_at_dist(-6.0, 0.0), track_at_dist(LAP - 6.0, 0.0));
    race.racers[0].dist = 432.0;
    assert_eq!(track_at(&race.racers[0]), track_at_dist(432.0, race.racers[0].lane));
}

#[test]
fn every_yard_runs_in_its_own_banner_ink_and_caltrops_show_on_the_track() {
    use crate::render::overworld::arena;
    use crate::render::overworld::ink::ink;
    let mut run = boarded_run();
    // String the field along the Rusty Oval, all of it inside the
    // camera's view of the player. (Mid-race, past the gate grid's
    // painted fan, so `pos_on` less the camera is where each is drawn.)
    {
        let race = run.chariot.as_mut().unwrap();
        race.track = 0;
        race.phase = Phase::Racing;
        race.t = 100;
        for (racer, dist) in race.racers.iter_mut().zip([300.0, 240.0, 360.0, 400.0]) {
            racer.dist = dist;
        }
        race.traps.push((330.0, 0.0));
    }
    let frame = arena::frame(&run, 384, 224);
    let banner = |house: &str| match house {
        "qwen" => ink('z'),
        "grok" => ink('A'),
        "glm" => ink('5'),
        "deepseek" => ink('1'),
        "keep" => ink('h'),
        _ => ink('R'),
    };
    let race = run.chariot.as_ref().unwrap();
    let course = race.course();
    let (camx, camy) = arena::race_camera(race, 384, 224);
    for racer in &race.racers {
        let (tx, ty) = pos_on(course, racer.dist, racer.lane);
        let (cx, cy) = ((tx * 16.0) as i32 - camx, (ty * 16.0) as i32 - camy);
        assert!((30..354).contains(&cx) && (40..200).contains(&cy), "{} is in the camera's view at {cx},{cy}", racer.name);
        let own = banner(&racer.house);
        let hits = (-30..4)
            .flat_map(|dy| (-22..22).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| frame.get(cx + dx, cy + dy) == own)
            .count();
        assert!(hits > 8, "{} ({}) wears its banner: {hits}", racer.name, racer.house);
        // …and wears no other yard's: the whole point of the repaint.
        for other in &race.racers {
            if other.house != racer.house {
                let foreign = banner(&other.house);
                let stray = (-30..4)
                    .flat_map(|dy| (-22..22).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| frame.get(cx + dx, cy + dy) == foreign)
                    .count();
                assert_eq!(stray, 0, "{} wears {}'s ink", racer.name, other.house);
            }
        }
    }
    let (tx, ty) = pos_on(course, 330.0, 0.0);
    let (px, py) = ((tx * 16.0) as i32 - camx, (ty * 16.0) as i32 - camy);
    assert_eq!(frame.get(px, py), ink('9'), "the caltrop's heart sits on the track");
}

#[test]
fn every_course_has_straights_corners_and_headings_that_follow_its_curve() {
    for track in &TRACKS {
        // The field stays inside the course's country, lanes and all.
        for k in 0..240 {
            let d = k as f32 * track.lap / 240.0;
            for lane in [-1.0, 1.0] {
                let (x, y) = pos_on(track, d, lane);
                assert!(
                    x > 1.0 && x < track.map.0 - 1.0 && y > 1.0 && y < track.map.1 - 1.0,
                    "{} leaves its map at {d}: {x},{y}",
                    track.name
                );
            }
            // Heading follows the curve's own travel, measured off
            // successive positions rather than the heading code itself.
            let (x0, y0) = pos_on(track, d - 4.0, 0.0);
            let (x1, y1) = pos_on(track, d + 4.0, 0.0);
            let (vx, vy) = (x1 - x0, y1 - y0);
            let want = if vy.abs() > vx.abs() {
                if vy > 0.0 { Heading::South } else { Heading::North }
            } else if vx > 0.0 {
                Heading::East
            } else {
                Heading::West
            };
            // Near a loop seam the ±4 pace chord can straddle the wrap;
            // the sampled points are 1/240th of a lap apart, so only
            // sample 0 is ever that close to it.
            if k > 0 {
                assert_eq!(heading_on(track, d), want, "{} heading at {d}", track.name);
            }
        }
        // A course is a rhythm: some of it bends, some of it flies.
        let corners = (0..480)
            .filter(|&k| bend_on(track, k as f32 * track.lap / 480.0) > CORNER_BEND)
            .count();
        let share = corners as f32 / 480.0;
        assert!(
            (0.12..0.65).contains(&share),
            "{} is {share:.2} corner — neither a drag strip nor a knot",
            track.name
        );
        println!("{}: lap {} paces, corner share {share:.2}", track.name, track.lap);
    }
}

#[test]
fn a_race_finishes_on_every_course() {
    for (t, track) in TRACKS.iter().enumerate() {
        let mut race = race(11);
        race.track = t;
        let mut finished = Vec::new();
        for _ in 0..40_000 {
            for event in race.tick(Input::default()) {
                if let Event::Finished(_, place) = event {
                    finished.push(place);
                }
            }
            if race.phase == Phase::Done {
                break;
            }
        }
        assert_eq!(race.phase, Phase::Done, "{} finished", track.name);
        finished.sort_unstable();
        assert_eq!(finished, [1, 2, 3, 4], "{} placed the field", track.name);
    }
}

#[test]
fn the_camera_keeps_the_players_chariot_in_frame_round_every_course() {
    use crate::render::overworld::arena;
    for (t, track) in TRACKS.iter().enumerate() {
        let mut race = race(3);
        race.track = t;
        race.phase = Phase::Racing;
        race.t = 100;
        for k in 0..60 {
            race.racers[0].dist = k as f32 * track.lap / 60.0;
            let (camx, camy) = arena::race_camera(&race, 384, 224);
            let (tx, ty) = pos_on(track, race.racers[0].dist, race.racers[0].lane);
            let (sx, sy) = ((tx * 16.0) as i32 - camx, (ty * 16.0) as i32 - camy);
            assert!(
                (30..354).contains(&sx) && (36..224).contains(&sy),
                "{} at pace {}: player off-frame at {sx},{sy}",
                track.name,
                race.racers[0].dist
            );
        }
    }
}

#[test]
fn the_balance_probe_finishes_every_seed_and_the_field_shares_the_wins() {
    // Two hands at every seed: a passenger who never whips, and a plain
    // stable policy (whip while the team has wind to spend). The probe is
    // the handoff's standing balance check: the field shares its wins,
    // passivity loses, and judged whipping pays without sweeping.
    let (mut idle_winners, mut idle_purse, mut idle_player_wins) = (Vec::new(), 0u32, 0u32);
    let (mut driven_purse, mut driven_wins) = (0u32, 0u32);
    for seed in [1u64, 7, 13, 42, 99, 123, 777, 2026] {
        for driven in [false, true] {
            let mut race = race(seed);
            for _ in 0..30_000 {
                let dash = driven && race.player().wind > 0.5;
                race.tick(Input { dash, ..Input::default() });
                if race.phase == Phase::Done {
                    break;
                }
            }
            assert_eq!(race.phase, Phase::Done, "seed {seed} finished");
            let place = race.player().place.unwrap();
            if driven {
                driven_purse += PURSE[(place - 1) as usize];
                driven_wins += u32::from(place == 1);
            } else {
                idle_purse += PURSE[(place - 1) as usize];
                idle_player_wins += u32::from(place == 1);
                idle_winners.push(race.racers.iter().position(|r| r.place == Some(1)).unwrap());
            }
        }
    }
    idle_winners.sort_unstable();
    idle_winners.dedup();
    println!("balance probe: idle purse {idle_purse} (player wins {idle_player_wins}), driven purse {driven_purse} (wins {driven_wins})");
    assert!(idle_winners.len() >= 2, "no single yard owns the oval: {idle_winners:?}");
    assert!(idle_player_wins <= 1, "a passenger does not win races: {idle_player_wins}");
    assert!(driven_purse > idle_purse, "judged whipping pays: {driven_purse} vs {idle_purse}");
    assert!((2..=7).contains(&driven_wins), "skill wins often, never always: {driven_wins}/8");
}

#[test]
fn the_race_board_stays_up_from_the_gate_to_the_flag() {
    use crate::render::overworld::arena;
    use crate::render::overworld::ink::ink;
    // The board is stamped at (4, 13) with a 'B' frame; a solid run of it
    // along the top edge is the announcer's board and nothing else.
    let board_up = |run: &Run| {
        let frame = arena::frame(run, 384, 224);
        (4..140).filter(|&x| frame.get(x, 13) == ink('B')).count() > 100
    };
    let mut run = boarded_run();
    assert!(board_up(&run), "the countdown board is up at the gate");
    for _ in 0..GATE_TICKS + 200 {
        run.step(&keys(0, true, false));
    }
    assert!(board_up(&run), "and it stays up mid-race, wherever the driver is");
}

#[test]
fn successive_hippodrome_frames_move_and_render_fast() {
    use crate::render::overworld::arena;
    let mut run = boarded_run();
    for _ in 0..GATE_TICKS + 10 {
        run.step(&keys(0, false, false));
    }
    let a = arena::frame(&run, 384, 224);
    let start = std::time::Instant::now();
    let mut frames = Vec::new();
    for _ in 0..30 {
        run.step(&keys(0, true, false));
        frames.push(arena::frame(&run, 384, 224));
    }
    let per_frame = start.elapsed() / 30;
    let moved = frames.windows(2).any(|pair| {
        (0..pair[0].h).any(|y| (0..pair[0].w).any(|x| pair[0].get(x, y) != pair[1].get(x, y)))
    });
    assert!(moved, "successive frames actually move");
    let advanced = (0..a.h)
        .flat_map(|y| (0..a.w).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get(x, y) != frames[29].get(x, y))
        .count();
    assert!(advanced > 100, "the picture advanced: {advanced} pixels changed");
    assert!(per_frame < std::time::Duration::from_millis(50), "frame budget: {per_frame:?}");
    println!("hippodrome frame: {per_frame:?} for 30 rendered frames");
}

#[test]
fn only_interact_boards_and_race_inputs_do_not_spend_the_knights_kit() {
    use crate::rules::{home::{BUY_HOLD, Home}, world};
    let mut run = Run::at_home(7, 1, None, Home::default(), Default::default());
    run.home.countryside.built |= 1 << super::super::hippodrome::PLOT;
    let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(hip, (20.0, 10.5));
    run.step(&BTreeMap::new());
    let fire = BTreeMap::from([(1, Input { fire: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 2 { run.step(&fire); }
    assert!(run.chariot.is_none(), "F only fires");
    assert_eq!(run.room().kind, RoomKind::Hippodrome);
    let interact = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 2 { run.step(&interact); }
    assert!(run.chariot.is_some(), "E boards even with fast coach travel unlocked");
    assert_eq!(run.room().kind, RoomKind::Hippodrome);
    for _ in 0..GATE_TICKS + 3 { run.step(&BTreeMap::new()); }
    let bombs = run.players[&1].bombs;
    let bag = run.chariot.as_ref().unwrap().player().caltrops;
    run.step(&interact);
    run.step(&fire);
    assert_eq!(run.chariot.as_ref().unwrap().player().caltrops, bag, "E/F do not drop iron");
    for _ in 0..HZ * 5 {
        if run.chariot.as_ref().unwrap().player().wrecked == 0 { break; }
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.chariot.as_ref().unwrap().player().wrecked, 0);
    run.step(&keys(0, true, true));
    assert_eq!(run.chariot.as_ref().unwrap().player().caltrops, bag - 1);
    assert_eq!(run.players[&1].bombs, bombs, "X is consumed by the race alone");
    assert!(run.projectiles.is_empty(), "the reined driver does not fire");
}

#[test]
fn the_coach_uses_selection_and_never_races_helios_or_a_growing_foal() {
    use crate::rules::{home::Home, ranch};
    let mut home = Home::default();
    let mut selected = horse();
    selected.name = "Chosen".into();
    selected.blood = Bloodline { pace: 3, stamina: 3, corner: 3, nerve: 3 };
    home.stable.herd = vec![horse(), selected];
    home.stable.selected_herd = Some(1);
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    run.board_chariot(1);
    assert_eq!(run.chariot.as_ref().unwrap().horse, "Chosen");
    run.home.stable.herd = vec![Horse { name: "Helios".into(), blood: ranch::HELIOS_BLOOD, ..Horse::default() },
        ranch::helios_foal("Aurora", true, 0, &horse())];
    run.home.stable.selected_herd = Some(0);
    run.board_chariot(1);
    assert_eq!(run.chariot.as_ref().unwrap().horse, "Yard Nag");
    run.home.ranch.wins = ranch::GROWN;
    run.board_chariot(1);
    assert_eq!(run.chariot.as_ref().unwrap().horse, "Aurora");
    assert!(run.home.stable.market(77).iter().all(|h| h.name != "Helios"));
    let helios = run.home.stable.herd[0].clone();
    assert!(run.home.stable.buy(helios, 1_000_000).is_none());
}

/// A race boarded by knight 1 with friend 2, named "Two", standing in the
/// Hippodrome at the off.
fn boarded_pair() -> Run {
    use crate::rules::home::{BUY_HOLD, Home};
    use crate::rules::world;
    let mut stable = Stable::default();
    stable.herd.push(horse());
    let home = Home { stable, ..Home::default() };
    let mut run = Run::at_home(7, 1, None, home, Default::default());
    run.join_seat(2, "Two");
    let hip = world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap();
    run.arrive(hip, (20.0, 10.5));
    {
        let friend = run.players.get_mut(&2).unwrap();
        (friend.x, friend.y) = (12.0 * TILE_UNITS, 10.5 * TILE_UNITS);
    }
    run.step(&BTreeMap::new());
    let interact = BTreeMap::from([(1, Input { interact: true, ..Input::default() })]);
    for _ in 0..BUY_HOLD + 2 {
        run.step(&interact);
    }
    run
}

#[test]
fn friends_in_the_room_drive_the_rival_teams_and_the_best_is_crowned() {
    let mut run = boarded_pair();
    let race = run.chariot.as_ref().expect("boarded");
    assert_eq!(race.friends, vec![(1, 2)], "the friend takes the first rival team");
    assert_eq!(race.racers[1].name, "Two");
    // The friend's own keys steer their own team.
    for _ in 0..GATE_TICKS + 5 {
        run.step(&BTreeMap::new());
    }
    let lane = run.chariot.as_ref().unwrap().racers[1].lane;
    for _ in 0..20 {
        run.step(&BTreeMap::from([(2, Input { move_x: 1, ..Input::default() })]));
    }
    assert!(run.chariot.as_ref().unwrap().racers[1].lane > lane + 0.3);
    // Their screen shows the race from their own chariot.
    let view = run.view_for_seat(2).unwrap();
    assert_eq!(view.chariot.as_ref().unwrap().player().name, "Two");
    // Run it home: someone is crowned, the other wears a cabbage.
    for _ in 0..HZ * 200 {
        run.step(&BTreeMap::new());
        if run.chariot.is_none() {
            break;
        }
    }
    let crowned = run.players.values().filter(|h| h.laurel).count();
    let cabbaged = run.players.values().filter(|h| h.booby.is_some()).count();
    assert_eq!((crowned, cabbaged), (1, 1));
    assert!(run.valid_snapshot());
}

#[test]
fn a_friends_hands_drive_their_own_team_and_no_one_elses() {
    let mut race = race(5);
    race.seat_friend(2, "Friend");
    assert_eq!(race.racer_of(1), Some(0), "the one who boarded drives racer 0");
    assert_eq!(race.racer_of(2), Some(1), "the friend drives the first rival team");
    assert_eq!(race.racer_of(99), None, "a spectator drives nothing");
    assert_eq!(race.seats(), vec![(1, 0), (2, 1)]);
    let idle = [Input::default()];
    for _ in 0..=GATE_TICKS {
        race.tick_with(Input::default(), &idle);
    }
    // Steering: ten ticks left takes the lane in by exactly the hand's rate.
    let lane = race.racers[1].lane;
    let left = [Input { move_x: -1, ..Input::default() }];
    for _ in 0..10 {
        race.tick_with(Input::default(), &left);
    }
    assert!(
        (race.racers[1].lane - (lane - 0.3)).abs() < 0.001,
        "the friend's reins steer: {lane} -> {}",
        race.racers[1].lane
    );
    assert!((race.racers[0].lane - -0.6).abs() < 0.001, "and not the player's");
    // The whip spends the friend's wind, no other hand involved.
    let whip = [Input { dash: true, ..Input::default() }];
    for _ in 0..200 {
        race.tick_with(Input::default(), &whip);
    }
    assert!(race.racers[1].wind < 0.7, "whipped down to {}", race.racers[1].wind);
    assert!(race.racers[0].wind > 0.9, "the player's team kept its wind");
    // One press of the bag, one caltrop; holding it drops no more.
    let bomb = [Input { bomb: true, ..Input::default() }];
    race.tick_with(Input::default(), &bomb);
    assert_eq!(race.racers[1].caltrops, 2, "the friend's iron is on the course");
    race.tick_with(Input::default(), &bomb);
    assert_eq!(race.racers[1].caltrops, 2, "a held key acts once");
    assert_eq!(race.racers[0].caltrops, 3);
    // With no keys from the friend the team is still theirs: it runs on,
    // and the rivals' own driving does not take it over.
    let before = race.racers[1].dist;
    for _ in 0..60 {
        race.tick_with(Input::default(), &[]);
        assert!(!race.racers[1].whipped, "no hand whips a friend's idle team");
    }
    assert!(race.racers[1].dist > before, "the friend's team keeps racing");
}

#[test]
fn a_friend_races_through_the_public_controls_on_their_own_screen() {
    use crate::render::overworld::arena;
    let mut run = boarded_pair();
    let both = |p1: Input, p2: Input| BTreeMap::from([(1, p1), (2, p2)]);
    for _ in 0..GATE_TICKS + 5 {
        run.step(&both(Input::default(), Input::default()));
    }
    // The friend's iron goes on the course, and their keys are the race's
    // alone: no bomb, shot or dash for the knight in the room.
    let bombs = run.players[&2].bombs;
    let iron = Input { bomb: true, fire: true, dash: true, ..Input::default() };
    run.step(&both(Input::default(), iron));
    assert_eq!(run.chariot.as_ref().unwrap().racers[1].caltrops, 2, "friend dropped iron");
    assert_eq!(run.players[&2].bombs, bombs, "X is consumed by the race alone");
    assert!(run.projectiles.is_empty(), "the reined friend does not fire");
    // Each seat's view rides its own chariot, and every chariot keeps its
    // own knight in the friend's view.
    {
        let race = run.chariot.as_mut().unwrap();
        let lap = race.course().lap;
        race.racers[0].dist = lap * 0.1;
        race.racers[1].dist = lap * 0.55;
    }
    let view = run.view_for_seat(2).unwrap();
    let seen = view.chariot.as_ref().unwrap();
    assert_eq!((seen.racer_of(2), seen.racer_of(1)), (Some(0), Some(1)));
    assert_eq!(seen.racers[1].name, "Testrunner", "the host's horse in the friend's field");
    let host_view = arena::frame_for(&run, 384, 224, Some(1));
    let friend_view = arena::frame_for(&view, 384, 224, Some(2));
    assert_ne!(host_view, friend_view, "two reins, two cameras");
    let (camx, camy) = arena::race_camera(seen, 384, 224);
    let (tx, ty) = pos_on(seen.course(), seen.racers[0].dist, seen.racers[0].lane);
    let (cx, cy) = ((tx * 16.0) as i32 - camx, (ty * 16.0) as i32 - camy);
    assert!(
        (20..364).contains(&cx) && (20..204).contains(&cy),
        "the friend's chariot in their frame at {cx},{cy}"
    );
    // Ride it out together: both drivers step down at the coach, and only
    // the host's herd horse goes on the record.
    for _ in 0..60_000 {
        run.step(&both(Input::default(), Input::default()));
        if run.chariot.is_none() {
            break;
        }
    }
    assert!(run.chariot.is_none(), "the result stood and both stepped down");
    let (c, r, w, h) = super::super::hippodrome::COACH;
    let friend = &run.players[&2];
    assert!(
        (friend.y - (r as f32 + h as f32 + 0.6) * TILE_UNITS).abs() < 0.01
            && friend.x > (c as f32 - 1.0) * TILE_UNITS
            && friend.x < (c + w) as f32 * TILE_UNITS,
        "the friend was set down at the coach too: {},{}",
        friend.x,
        friend.y
    );
    let mut stable = run.home.stable.clone();
    let (_said, _gold) = settle_chariot(&mut stable, &run.marks);
    assert_eq!(stable.herd[0].starts, 1, "only the host's herd horse is recorded");
}
