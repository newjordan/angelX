//! The 3D races: the course, each ride's rules, the crashes and the board.
use super::*;

/// A two-knight party walked into a race hall holding `course`, each
/// carrying 100 gold.
fn race_hall(course: Course) -> Run {
    let mut run = (0..80u64)
        .find_map(|seed| {
            let mut r = Run::new(seed, 1, None);
            for _ in 0..6 {
                r.descend_for_test();
                if Course::of_floor(r.dungeon.depth) == course && r.race3d_room.is_some() {
                    return Some(r);
                }
            }
            None
        })
        .expect("a floor with that race");
    run.join_seat(2, "Two");
    run.enemies.clear();
    for hero in run.players.values_mut() {
        let mut purse = Spoils::default();
        purse.add(Spoil::Gold, hero.carried.get(Spoil::Gold));
        hero.carried.take(&purse);
        hero.carried.add(Spoil::Gold, 100);
    }
    run.enter_for_test(run.race3d_room.expect("a race hall"));
    run
}

/// Past the countdown.
fn racing(course: Course) -> Run {
    let mut run = race_hall(course);
    run.race3d.as_mut().unwrap().t = TITLE;
    run
}

fn race(run: &Run) -> &Race3d {
    run.race3d.as_ref().unwrap()
}

fn rider(run: &mut Run, id: u32) -> &mut Rider {
    run.race3d.as_mut().unwrap().riders.get_mut(&id).unwrap()
}

fn keys(pairs: &[(u32, Input)]) -> BTreeMap<u32, Input> {
    pairs.iter().copied().collect()
}

fn ticks(run: &mut Run, n: u32, input: &BTreeMap<u32, Input>) {
    for _ in 0..n {
        run.step(input);
    }
}

#[test]
fn every_course_is_laid_long_enough_and_never_crosses_itself() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let t = track(course);
        assert!(t.len() > 280.0, "{course:?} is {} long", t.len());
        assert!(t.finish < t.len() && t.finish > t.len() - 30.0);
        assert!(t.posts.windows(2).all(|w| w[0] < w[1]));
        // Far-apart stretches of the course keep their distance, so the
        // walls of one never cut through another.
        let room = if course == Course::Griffin { 18.0 } else { 9.0 };
        for (i, a) in t.samples.iter().enumerate().step_by(2) {
            for b in t.samples.iter().skip(i + 40).step_by(2) {
                let flat = (a.p[0] - b.p[0]).hypot(a.p[1] - b.p[1]);
                assert!(
                    flat > room || (a.p[2] - b.p[2]).abs() > 7.0,
                    "{course:?} comes back on itself near {:?}",
                    a.p
                );
            }
        }
        for item in &t.items {
            assert!(item.s > 0.0 && item.s < t.finish, "{course:?} item past the line");
        }
    }
}

#[test]
fn the_floors_hold_their_races_and_the_first_floor_teaches_instead() {
    assert_eq!(Course::of_floor(2), Course::Shaft);
    assert_eq!(Course::of_floor(3), Course::Griffin);
    assert_eq!(Course::of_floor(4), Course::Cart);
    let mut run = Run::new(3, 1, None);
    assert!(run.race3d_room.is_none(), "no race on the first floor");
    run.descend_for_test();
    assert!(run.race3d_room.is_some(), "the second floor has its race hall");
    assert_ne!(run.race3d_room, run.folly_room);
}

#[test]
fn walking_into_the_race_hall_lines_everyone_up_and_takes_the_antes() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let run = race_hall(course);
        let r = race(&run);
        assert_eq!(r.course, course);
        assert_eq!(r.riders.len(), 2, "every knight races");
        assert_eq!(r.ante.values().sum::<u32>(), 20, "a tenth of each knight's gold");
        for hero in run.players.values() {
            assert_eq!(hero.carried.get(Spoil::Gold), 90);
        }
        assert!(run.barred() && run.race3d_showing() && run.valid_snapshot());
        assert!(run.enemies.is_empty(), "a race hall holds no fight");
    }
}

#[test]
fn nobody_moves_during_the_countdown_and_the_knights_stand_still_in_the_hall() {
    let mut run = race_hall(Course::Shaft);
    let go = keys(&[(1, Input { move_y: -1, move_x: 1, fire: true, ..Default::default() })]);
    let stood = (run.players[&1].x, run.players[&1].y);
    let s = race(&run).riders[&1].s;
    ticks(&mut run, TITLE - 1, &go);
    assert_eq!(race(&run).riders[&1].s, s, "held at the gate");
    ticks(&mut run, HZ, &go);
    assert!(race(&run).riders[&1].s > s, "rolling once the gate drops");
    assert_eq!((run.players[&1].x, run.players[&1].y), stood, "the knight stays in the hall");
    assert!(run.projectiles.is_empty(), "F hops; it fires nothing in the hall");
}

#[test]
fn a_ball_rolls_down_the_ramp_and_hops() {
    let mut run = racing(Course::Shaft);
    let hop = keys(&[(1, Input { fire: true, ..Default::default() })]);
    run.step(&hop);
    assert!(race(&run).riders[&1].z > 0.0, "the hop leaves the floor");
    // Held F does not hop again on landing.
    ticks(&mut run, HZ, &hop);
    assert_eq!(race(&run).riders[&1].z, 0.0);
    // Down the timbered ramp the ball gathers pace by itself.
    let ramp = track(Course::Shaft).posts[1] + 4.0;
    rider(&mut run, 2).s = ramp;
    ticks(&mut run, HZ, &BTreeMap::new());
    assert!(race(&run).riders[&2].vs > 1.0, "the slope rolls it");
}

#[test]
fn off_the_plank_bridge_is_down_the_shaft_and_the_rope_brings_you_back() {
    let mut run = racing(Course::Shaft);
    let bridge = track(Course::Shaft).posts[3];
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.vs) = (bridge + 8.0, 0.0, 3.0);
    }
    let left = keys(&[(1, Input { move_x: -1, ..Default::default() })]);
    ticks(&mut run, 20, &left);
    let r = &race(&run).riders[&1];
    assert_eq!(r.wreck, Wreck::Fall);
    assert_eq!(r.crashes, 1);
    while race(&run).riders[&1].crash > 0 {
        run.step(&BTreeMap::new());
    }
    let r = &race(&run).riders[&1];
    assert_eq!(r.s, bridge - RUN_UP, "back with a run-up to the bridge");
    assert_eq!((r.x, r.z, r.vs), (0.0, 0.0, 0.0));
}

#[test]
fn a_rotten_plank_goes_a_second_after_the_first_ball_and_takes_the_next() {
    let mut run = racing(Course::Shaft);
    let t = track(Course::Shaft);
    let plank = (t.posts[9] / 2.0) as u16 + 4;
    let plank = if rotten(plank) { plank } else { plank + 1 };
    let over = plank as f32 * 2.0 + 1.0;
    {
        let r = rider(&mut run, 1);
        (r.s, r.vs) = (over - 0.5, 2.0);
    }
    run.step(&BTreeMap::new());
    assert!(race(&run).planks.contains_key(&plank), "the first ball over starts it");
    assert_eq!(race(&run).riders[&1].wreck, Wreck::None, "the first ball gets across");
    ticks(&mut run, ROT, &BTreeMap::new());
    {
        let r = rider(&mut run, 2);
        (r.s, r.x, r.vs, r.z) = (over, 0.0, 0.0, 0.0);
    }
    run.step(&BTreeMap::new());
    assert_eq!(race(&run).riders[&2].wreck, Wreck::Fall, "the plank has gone");
}

#[test]
fn a_crossing_ore_cart_bonks_a_ball() {
    let mut run = racing(Course::Shaft);
    let t = track(Course::Shaft);
    // Halfway across: the first cart is mid-way over the track.
    run.race3d.as_mut().unwrap().t = 4 * HZ * 3 + HZ - 1;
    let race_t = race(&run).t + 1;
    let (cs, cx) = crossing_carts(t, race_t)[0];
    assert!(cx.abs() < 1.0);
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.vs) = (cs, cx, 0.0);
    }
    run.step(&BTreeMap::new());
    let r = &race(&run).riders[&1];
    assert_eq!(r.wreck, Wreck::Bonk);
    assert!(r.vs < 0.0, "knocked back up the shaft");
}

#[test]
fn balls_that_meet_bounce_apart() {
    let mut run = racing(Course::Shaft);
    let s = track(Course::Shaft).posts[1] + 2.0;
    {
        let a = rider(&mut run, 1);
        (a.s, a.x, a.vs, a.vx) = (s, -0.3, 0.0, 3.0);
    }
    {
        let b = rider(&mut run, 2);
        (b.s, b.x, b.vs, b.vx) = (s, 0.3, 0.0, -3.0);
    }
    run.step(&BTreeMap::new());
    let r = race(&run);
    assert!(r.riders[&1].vx < 0.0 && r.riders[&2].vx > 0.0, "they trade their pace");
    assert!(r.riders[&2].x - r.riders[&1].x >= 2.0 * BALL_R - 0.05);
}

#[test]
fn a_gem_shard_is_for_whoever_reaches_it_first() {
    let mut run = racing(Course::Shaft);
    let t = track(Course::Shaft);
    let (i, gem) = t.items.iter().enumerate().find(|(_, g)| g.kind == ItemKind::Gem && g.x.abs() < 0.1).unwrap();
    for id in [1, 2] {
        let r = rider(&mut run, id);
        (r.s, r.x, r.vs, r.vx) = (gem.s, gem.x, 0.0, 0.0);
    }
    run.step(&BTreeMap::new());
    let r = race(&run);
    assert!(r.taken.contains(&(i as u16)));
    assert_eq!(r.riders[&1].haul + r.riders[&2].haul, 1, "one shard, one knight");
}

#[test]
fn a_cart_left_on_an_ending_line_hits_the_buffer_and_a_switch_saves_it() {
    let mut run = racing(Course::Cart);
    let t = track(Course::Cart);
    // The left line ends where the trestle starts.
    let end = t.posts[3];
    assert!(laid(t, end - 1.0, 0) && !laid(t, end + 0.5, 0));
    for id in [1, 2] {
        let r = rider(&mut run, id);
        (r.s, r.lane, r.x, r.vs) = (end - 3.0, 0, LANES[0], 8.5);
    }
    rider(&mut run, 2).x = LANES[0];
    let switch = keys(&[(2, Input { interact: true, move_x: 1, ..Default::default() })]);
    ticks(&mut run, 12, &switch);
    let r = race(&run);
    assert_eq!(r.riders[&1].wreck, Wreck::Buffer, "stayed on the ending line");
    assert_eq!(r.riders[&2].lane, 1);
    assert_eq!(r.riders[&2].wreck, Wreck::None, "switched in time");
}

#[test]
fn a_broken_rail_wants_a_jump() {
    let t = track(Course::Cart);
    let gap = t.gaps[0];
    let mut run = racing(Course::Cart);
    for id in [1, 2] {
        let r = rider(&mut run, id);
        (r.s, r.lane, r.x, r.vs) = (gap.s.0 - 1.5, gap.lane, LANES[usize::from(gap.lane)], 8.5);
    }
    rider(&mut run, 2).s = gap.s.0 - 1.0;
    let jump = keys(&[(1, Input { fire: true, ..Default::default() })]);
    ticks(&mut run, 20, &jump);
    let r = race(&run);
    assert_eq!(r.riders[&1].wreck, Wreck::None, "jumped it");
    assert!(r.riders[&1].s > gap.s.1);
    assert_eq!(r.riders[&2].wreck, Wreck::Gap, "rolled into it");
}

#[test]
fn a_fallen_timber_flips_a_cart_that_does_not_jump() {
    let t = track(Course::Cart);
    let timber = t.items.iter().find(|i| i.kind == ItemKind::Timber).unwrap();
    let lane = LANES.iter().position(|&x| (x - timber.x).abs() < 0.01).unwrap() as u8;
    let mut run = racing(Course::Cart);
    {
        let r = rider(&mut run, 1);
        (r.s, r.lane, r.x, r.vs) = (timber.s - 1.0, lane, timber.x, 8.5);
    }
    ticks(&mut run, 8, &BTreeMap::new());
    assert_eq!(race(&run).riders[&1].wreck, Wreck::Timber);
}

#[test]
fn a_cart_into_the_back_of_another_shunts_it_on() {
    let mut run = racing(Course::Cart);
    let s = track(Course::Cart).posts[1] + 4.0;
    {
        let a = rider(&mut run, 1);
        (a.s, a.lane, a.x, a.vs) = (s, 1, 0.0, 11.0);
    }
    {
        let b = rider(&mut run, 2);
        (b.s, b.lane, b.x, b.vs) = (s + 1.2, 1, 0.0, 5.0);
    }
    run.step(&BTreeMap::new());
    let r = race(&run);
    assert!(r.riders[&2].vs > r.riders[&1].vs, "the front cart is shunted on");
}

#[test]
fn a_ring_bursts_a_griffin_and_a_spire_tumbles_one() {
    let t = track(Course::Griffin);
    let (_, ring) = t.items.iter().enumerate().find(|(_, i)| i.kind == ItemKind::Ring).unwrap();
    let mut run = racing(Course::Griffin);
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.z, r.vs) = (ring.s - 0.2, ring.x, ring.z, 10.0);
    }
    run.step(&BTreeMap::new());
    let r = &race(&run).riders[&1];
    assert_eq!(r.haul, 1);
    assert!(r.burst > 0);
    let spire = t.blocks[0];
    {
        let r = rider(&mut run, 2);
        (r.s, r.x, r.z, r.vs) = (spire.s - 0.4, (spire.x.0 + spire.x.1) / 2.0, spire.z.1 - 0.5, 10.0);
    }
    run.step(&BTreeMap::new());
    assert_eq!(race(&run).riders[&2].wreck, Wreck::Tumble);
}

#[test]
fn a_bolt_smashes_a_gargoyle_or_ruffles_a_rival_unless_it_rolls() {
    let t = track(Course::Griffin);
    let (gi, goyle) = t.items.iter().enumerate().find(|(_, i)| i.kind == ItemKind::Gargoyle).unwrap();
    let mut run = racing(Course::Griffin);
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.z, r.vs) = (goyle.s - 9.0, goyle.x, goyle.z, 10.0);
    }
    rider(&mut run, 2).s = 1.0;
    let fire = keys(&[(1, Input { fire: true, ..Default::default() })]);
    ticks(&mut run, 10, &fire);
    let r = race(&run);
    assert!(r.taken.contains(&(gi as u16)), "smashed");
    assert_eq!(r.riders[&1].haul, 1);
    // A rival ahead in the line of fire is ruffled...
    let s = t.posts[1];
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.z, r.reload) = (s, 0.0, 0.0, 0);
    }
    {
        let r = rider(&mut run, 2);
        (r.s, r.x, r.z, r.crash) = (s + 5.0, 0.0, 0.0, 0);
    }
    ticks(&mut run, 6, &fire);
    assert!(race(&run).riders[&2].stun > 0, "a bolt in the feathers");
    // ...unless it rolls.
    rider(&mut run, 2).stun = 0;
    {
        let r = rider(&mut run, 1);
        (r.s, r.x, r.z, r.reload) = (s, 0.0, 0.0, 0);
    }
    {
        let r = rider(&mut run, 2);
        (r.s, r.x, r.z, r.rest) = (s + 5.0, 0.0, 0.0, 0);
    }
    let roll = keys(&[
        (1, Input { fire: true, ..Default::default() }),
        (2, Input { interact: true, ..Default::default() }),
    ]);
    run.step(&roll);
    ticks(&mut run, 4, &keys(&[(2, Input { interact: true, ..Default::default() })]));
    assert_eq!(race(&run).riders[&2].stun, 0, "rolled clear");
}

#[test]
fn the_first_home_takes_the_pot_and_the_laurel_and_everyone_keeps_their_haul() {
    let mut run = racing(Course::Shaft);
    let finish = track(Course::Shaft).finish;
    rider(&mut run, 1).haul = 3;
    rider(&mut run, 2).haul = 5;
    {
        let r = rider(&mut run, 2);
        (r.s, r.vs) = (finish - 0.05, 3.0);
    }
    run.step(&BTreeMap::new());
    assert_eq!(race(&run).riders[&2].finish, Some(race(&run).t));
    assert!(!race(&run).done, "still racing while one is out on the course");
    {
        let r = rider(&mut run, 1);
        (r.s, r.vs) = (finish - 0.05, 3.0);
    }
    run.step(&BTreeMap::new());
    let r = race(&run);
    assert!(r.done);
    assert_eq!(r.order(), vec![2, 1]);
    assert_eq!(r.won[&2], 20 + 5 * HAUL_GOLD, "the pot and the shards");
    assert_eq!(r.won[&1], 3 * HAUL_GOLD);
    assert_eq!(run.players[&2].carried.get(Spoil::Gold), 90 + 20 + 10);
    assert_eq!(run.players[&1].carried.get(Spoil::Gold), 90 + 6);
    assert!(run.players[&2].laurel && !run.players[&1].laurel);
    assert!(run.room().cleared && !run.barred());
    assert!(run.found.as_ref().unwrap().2.contains("TWO WON THE SHAFT RUN"));
    // The board stays up a while, then the hall comes back.
    assert!(run.race3d_showing());
    ticks(&mut run, PODIUM, &BTreeMap::new());
    assert!(!run.race3d_showing());
    assert!(run.valid_snapshot());
}

#[test]
fn the_clock_ranks_whoever_is_out_on_the_course_by_how_far_they_got() {
    let mut run = racing(Course::Cart);
    rider(&mut run, 1).s = 100.0;
    rider(&mut run, 2).s = 140.0;
    run.race3d.as_mut().unwrap().t = TITLE + RACE_TIME - 1;
    run.step(&BTreeMap::new());
    let r = race(&run);
    assert!(r.done);
    assert_eq!(r.order()[0], 2);
    assert!(run.players[&2].laurel);
}

#[test]
fn a_friends_mirror_carries_the_race() {
    let mut run = racing(Course::Griffin);
    ticks(&mut run, 10, &keys(&[(1, Input { fire: true, move_x: 1, ..Default::default() })]));
    let friend = run.wire_copy();
    assert_eq!(friend.race3d, run.race3d);
}

/// A knight who keys the obvious things: forward, middle, hop the drift,
/// switch off an ending line, jump what is ahead, fly round the stone.
fn autopilot(run: &Run, id: u32) -> Input {
    let r = race(run);
    let me = &r.riders[&id];
    let t = track(r.course);
    let mut input = Input::default();
    match r.course {
        Course::Shaft => {
            input.move_y = -1;
            let at = t.at(me.s);
            let aim = if at.ground == Ground::Lodestone { 0.6 } else { 0.0 };
            let drift = me.x + me.vx * 0.5;
            if drift > aim + 0.25 {
                input.move_x = -1;
            } else if drift < aim - 0.25 {
                input.move_x = 1;
            }
            let plank = ((me.s + 1.2) / 2.0) as u16;
            let gone = t.at(me.s + 1.2).ground == Ground::Collapse
                && rotten(plank)
                && r.planks.get(&plank).is_some_and(|&b| r.t + 3 >= b + ROT);
            input.fire = gone && me.z == 0.0;
            if (at.width < 3.0 || t.at(me.s + 6.0).width < 3.0) && me.vs > 5.0 {
                input.move_y = 1;
            }
        }
        Course::Cart => {
            if (1..=8).any(|d| !laid(t, me.s + d as f32, me.lane)) {
                input.interact = r.t % 2 == 0;
                input.move_x = if me.lane == 0 || (me.lane == 1 && laid(t, me.s + 8.0, 2)) { 1 } else { -1 };
            }
            let v = me.vs.max(1.0);
            let gap = t.gaps.iter().any(|g| g.lane == me.lane && (0.3..0.6 + 0.2 * v).contains(&(g.s.0 - me.s)));
            let timber = t.items.iter().any(|i| {
                i.kind == ItemKind::Timber && (i.x - me.x).abs() < 1.0 && (0.6 + 0.2 * v..0.6 + 0.4 * v).contains(&(i.s - me.s))
            });
            let threat = gap || timber;
            input.fire = threat && me.z == 0.0 && me.keys & 1 == 0;
        }
        Course::Griffin => {
            // The clearest spot through the next stretch of stone.
            let mut best = (f32::MAX, 0.0f32, 0.0f32);
            for gx in [-3.0f32, -1.5, 0.0, 1.5, 3.0] {
                for gz in [-2.0f32, 0.0, 2.0] {
                    let blocked = t.blocks.iter().any(|b| {
                        b.s > me.s - 1.0 && b.s < me.s + 18.0 && gx + 1.0 > b.x.0 && gx - 1.0 < b.x.1 && gz + 0.9 > b.z.0 && gz - 0.9 < b.z.1
                    }) || t.items.iter().enumerate().any(|(i, g)| {
                        g.kind == ItemKind::Gargoyle && !r.taken.contains(&(i as u16)) && g.s > me.s && g.s < me.s + 12.0 && (g.x - gx).hypot(g.z - gz) < 1.6
                    });
                    let cost = if blocked { 1000.0 } else { 0.0 } + (gx - me.x).abs() + (gz - me.z).abs();
                    if cost < best.0 {
                        best = (cost, gx, gz);
                    }
                }
            }
            if best.1 > me.x + 0.3 {
                input.move_x = 1;
            } else if best.1 < me.x - 0.3 {
                input.move_x = -1;
            }
            if best.2 > me.z + 0.3 {
                input.move_y = -1;
            } else if best.2 < me.z - 0.3 {
                input.move_y = 1;
            }
            input.fire = r.t % 12 == 0;
        }
    }
    input
}

#[test]
fn a_steady_knight_gets_home_in_a_minute_or_so_on_every_course() {
    for course in [Course::Shaft, Course::Cart, Course::Griffin] {
        let mut run = race_hall(course);
        let mut n = 0;
        while race(&run).showing() && n < TITLE + RACE_TIME + PODIUM + 10 {
            let inputs: BTreeMap<u32, Input> = [1, 2].iter().map(|&id| (id, autopilot(&run, id))).collect();
            run.step(&inputs);
            n += 1;
            assert!(run.valid_snapshot());
        }
        let r = race(&run);
        assert!(r.done, "{course:?} ended");
        for (id, x) in &r.riders {
            eprintln!("{course:?} seat {id}: s {:.1} x {:.2} z {:.2} vs {:.2} crash {} {:?} crashes {}", x.s, x.x, x.z, x.vs, x.crash, x.wreck, x.crashes);
        }
        let home: Vec<(u32, u32, u16)> = r
            .riders
            .iter()
            .filter_map(|(&id, x)| x.finish.map(|f| (id, (f - TITLE) / HZ, x.crashes)))
            .collect();
        eprintln!("{course:?}: home (seat, seconds, crashes) {home:?}");
        assert_eq!(home.len(), 2, "{course:?}: both home before the clock");
        for &(_, secs, _) in &home {
            assert!((40..=88).contains(&secs), "{course:?} took {secs}s");
        }
    }
}



// ─── the Griffin Eyrie ──────────────────────────────────────────────────────

use crate::rules::{home::Home, world};
use crate::render::overworld::kingdom as geography;

fn at_the_eyrie(seats: u32) -> Run {
    let mut run = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    for hero in run.players.values_mut() {
        hero.carried.add(Spoil::Gold, 50);
    }
    run.arrive(world::room_of(&run.dungeon, RoomKind::Eyrie).expect("the eyrie"), (12.0, 11.0));
    run
}

fn at_the_perch(run: &mut Run) {
    for hero in run.players.values_mut() {
        (hero.x, hero.y) = (PERCH.0 * TILE_UNITS, (PERCH.1 + 1.0) * TILE_UNITS);
    }
}

#[test]
fn the_griffin_eyrie_stands_on_the_mountain_with_a_road_to_its_door() {
    let geo = geography::geography(&[]);
    let site = geo.sites.iter().find(|s| s.room == RoomKind::Eyrie).expect("an eyrie door");
    assert!(geography::is_road(&geo, site.stand), "on a road");
    // The Mountain is tiles 32..64 across and 0..22 down (its stand may be
    // the row below its lot).
    assert!((32..64).contains(&site.stand.0) && (0..24).contains(&site.stand.1), "on the Mountain: {:?}", site.stand);
    // In from the kingdom's door, and out again by the yard gate.
    let mut run = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    run.join_seat(2, "Two");
    run.walk_kingdom(Some(geography::keep_stand()));
    run.floor_plates.clear();
    let geo = geography::geography(&run.kingdom.wishes);
    let site = geo.sites.iter().position(|s| s.room == RoomKind::Eyrie).unwrap();
    run.enter_kingdom_place(site);
    assert_eq!(run.room().kind, RoomKind::Eyrie);
    assert!(run.valid_snapshot());
    for _ in 0..HZ * 3 {
        run.step(&BTreeMap::from([(1, Input { move_y: 1, ..Default::default() }), (2, Input { move_y: 1, ..Default::default() })]));
        if run.room().kind == RoomKind::Kingdom {
            break;
        }
    }
    assert_eq!(run.room().kind, RoomKind::Kingdom);
}

#[test]
fn e_at_the_perch_flies_everyone_free_and_again_after_the_board() {
    let mut run = at_the_eyrie(2);
    let gold: Vec<u32> = run.players.values().map(|h| h.carried.get(Spoil::Gold)).collect();
    // Away from the perch, E does nothing.
    run.step(&keys(&[(1, Input { interact: true, ..Default::default() })]));
    run.step(&BTreeMap::new());
    assert!(run.race3d.is_none());
    at_the_perch(&mut run);
    run.step(&keys(&[(1, Input { interact: true, ..Default::default() })]));
    let r = race(&run);
    assert!(r.practice && r.course == Course::Griffin);
    assert_eq!(r.riders.len(), 2, "every knight flies");
    assert!(r.ante.is_empty());
    assert!(run.race3d_showing() && run.valid_snapshot());
    // Home first: no gold changes hands, but a race is won.
    assert!(!run.race_won());
    let finish = track(Course::Griffin).finish;
    run.race3d.as_mut().unwrap().t = TITLE;
    rider(&mut run, 2).s = finish - 0.05;
    rider(&mut run, 2).vs = 10.0;
    rider(&mut run, 1).s = finish - 0.05;
    rider(&mut run, 1).vs = 10.0;
    run.step(&BTreeMap::new());
    assert!(race(&run).done);
    let after: Vec<u32> = run.players.values().map(|h| h.carried.get(Spoil::Gold)).collect();
    assert_eq!(gold, after, "a practice flight stakes nothing");
    assert!(!run.players.values().any(|h| h.laurel));
    assert!(run.race_won(), "the first win leaves its mark");
    assert!(run.found.as_ref().unwrap().2.contains("FLEW THE GORGE FIRST"));
    for _ in 0..PODIUM {
        run.step(&BTreeMap::new());
    }
    assert!(!run.race3d_showing());
    at_the_perch(&mut run);
    run.step(&keys(&[(2, Input { interact: true, ..Default::default() })]));
    assert!(!race(&run).done, "another flight");
}

#[test]
fn maud_and_the_gold_arrow_send_knights_to_the_griffins_until_a_race_is_won() {
    let mut home = Home::default();
    home.residents.insert("maud".into());
    home.deepest = 1;
    let mut run = Run::at_home(3, 1, None, home, Spoils::default());
    run.maud_gossips_for_test();
    assert_eq!(run.kingdom.moments.errand, Some(RoomKind::Eyrie));
    assert!(run.found.as_ref().unwrap().2.contains("NOBODY'S WON A RACE"));
    run.kingdom.moments.errand = None;
    run.walk_kingdom(Some(geography::keep_stand()));
    assert_eq!(run.kingdom_goal(1).expect("a goal").name, "Griffin Eyrie");
    // A race won (and settled into the realm): the arrow and Maud move on.
    run.marks.insert(RACE_MARK.into(), 1);
    let mut treasury = Spoils::default();
    let marks = run.marks.clone();
    let (changed, news) = countryside::settle_games(&mut run.home.countryside, &mut treasury, &marks);
    assert!(changed && run.home.countryside.race_won && news.iter().any(|n| n.contains("race")));
    run.marks.clear();
    assert!(run.race_won());
    assert_ne!(run.kingdom_goal(1).expect("a goal").name, "Griffin Eyrie");
    run.found = None;
    run.maud_gossips_for_test();
    assert_ne!(run.kingdom.moments.errand, Some(RoomKind::Eyrie));
    assert!(countryside::intent_valid(RACE_MARK, 1));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_eyrie_shots -- --ignored`: the
/// Eyrie on the kingdom's map, the gold arrow toward it, its yard, and a
/// practice flight from the countdown to the board.
#[test]
#[ignore]
fn write_eyrie_shots() {
    use crate::render::overworld::arena;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir).join("eyrie");
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, name: &str| {
        let img = arena::frame_for(run, 768, 448, Some(1));
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(dir.join(format!("{name}.ppm")), out).unwrap();
    };
    let mut home = Home::default();
    home.deepest = 1;
    let mut run = Run::at_home(53, 1, None, home, Spoils::default());
    run.join_seat(2, "Lynette");
    for (id, class) in [(1u32, "Archer"), (2, "Mage")] {
        run.players.get_mut(&id).unwrap().class = Some(class.into());
    }
    let geo = geography::geography(&run.kingdom.wishes);
    let site = geo.sites.iter().find(|s| s.room == RoomKind::Eyrie).unwrap();
    // The gold arrow from the Keep, then the Eyrie's door on the Mountain.
    run.walk_kingdom(Some(geography::keep_stand()));
    run.floor_plates.clear();
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    save(&run, "e1-arrow-from-the-keep");
    run.walk_kingdom(Some((site.stand.0, site.stand.1 + 3)));
    for _ in 0..10 {
        run.step(&BTreeMap::new());
    }
    save(&run, "e2-the-eyrie-on-the-mountain");
    // In the yard, by the perch.
    let index = geo.sites.iter().position(|s| s.room == RoomKind::Eyrie).unwrap();
    run.enter_kingdom_place(index);
    // Past the arrival's blink.
    for _ in 0..HZ {
        run.step(&BTreeMap::new());
    }
    save(&run, "e3-the-yard");
    at_the_perch(&mut run);
    for _ in 0..4 {
        run.step(&BTreeMap::new());
    }
    save(&run, "e4-at-the-perch");
    run.step(&keys(&[(1, Input { interact: true, ..Default::default() })]));
    run.race3d.as_mut().unwrap().t = HZ;
    save(&run, "e5-practice-title");
    run.race3d.as_mut().unwrap().t = TITLE;
    let fly = keys(&[
        (1, Input { fire: true, move_x: 1, ..Default::default() }),
        (2, Input { move_y: -1, ..Default::default() }),
    ]);
    for _ in 0..HZ * 3 {
        run.step(&fly);
    }
    save(&run, "e6-practice-flight");
    let finish = track(Course::Griffin).finish;
    for (id, ds) in [(2u32, 0.05), (1, 0.05)] {
        let r = rider(&mut run, id);
        (r.s, r.vs) = (finish - ds, 10.0);
        run.race3d.as_mut().unwrap().t += 25 * HZ;
        run.step(&BTreeMap::new());
    }
    save(&run, "e7-practice-board");
}

