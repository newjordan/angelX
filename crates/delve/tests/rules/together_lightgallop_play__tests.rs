//! Lightgallop's moves and its road, one at a time: the roll and the leap
//! that pop a snuffer, the spark that scatters and is taken back, the hay
//! bale, the pad, the lantern post, the lance-lock chain, the Dimming.
use super::level::{G, HOLD, level};
use super::math::Q;
use super::rider::*;
use super::session::*;
use super::world::*;
use super::*;

fn heroes(n: u32) -> BTreeMap<u32, Hero> {
    (1..=n).map(|id| (id, Hero::new(&format!("Knight {id}"), 0.0, 0.0))).collect()
}

/// A session of `n` knights with the Dimming far behind, and the heroes.
fn ride(n: u32) -> (Session, BTreeMap<u32, Hero>) {
    let h = heroes(n);
    let mut s = Session::new(&h);
    s.dim = -4000 * Q;
    (s, h)
}

const NUM: Numbers = Numbers { slope: 100, loop_min: 512, spur_cap: 2048, lock_reach: 96, dimming: 1000 };

fn keys(x: i8, down: bool, jump: bool, spur: bool) -> Input {
    Input { move_x: x, move_y: i8::from(down), interact: jump, dash: spur, ..Default::default() }
}

fn tick(s: &mut Session, h: &BTreeMap<u32, Hero>, inputs: &[(u32, Input)]) -> Vec<&'static str> {
    let map: BTreeMap<u32, Input> = inputs.iter().copied().collect();
    s.step(&map, h, &NUM)
}

/// Set knight `id` on the main road at column `x` with ground speed `gsp`.
fn put(s: &mut Session, id: u32, x: i32, gsp: i32) {
    let lvl = level();
    let at = lvl.terrain.paths[lvl.main].s_at_x(x);
    let r = &mut s.seats.get_mut(&id).unwrap().rider;
    let keep = (r.collected, r.falls);
    *r = Rider::new_on(&lvl.terrain, lvl.main, at);
    (r.collected, r.falls) = keep;
    r.gsp = gsp;
    r.dir = if gsp < 0 { -1 } else { 1 };
    r.snap_camera(x, G);
}

fn rider(s: &Session, id: u32) -> &Rider {
    &s.seats[&id].rider
}

/// The index of the first foe of `kind` at or after column `x`.
fn foe_after(kind: FoeKind, x: i32) -> usize {
    level().foes.iter().position(|f| f.kind == kind && f.x >= x).unwrap()
}

#[test]
fn a_roll_pops_a_snuffer_and_a_run_into_it_is_a_hit() {
    let i = foe_after(FoeKind::Snuffer, 900);
    // Rolling through.
    let (mut s, h) = ride(1);
    let x = level().foes[i].x;
    put(&mut s, 1, x - 150, 1280);
    let mut tucked = false;
    for t in 0..80 {
        // Tuck at the last moment, as a player would.
        tucked |= rider(&s, 1).roll;
        tick(&mut s, &h, &[(1, keys(1, t > 8, false, false))]);
        if s.foes[i].hp == 0 {
            break;
        }
    }
    assert!(tucked && s.foes[i].hp == 0, "popped by the roll");
    assert_eq!(rider(&s, 1).kills, 1);
    assert!(rider(&s, 1).sparks >= 1, "a spark of returned light");
    assert!(rider(&s, 1).down == 0 && rider(&s, 1).hits == 0, "the roll was not hurt");
    // Running into it, upright.
    let (mut s, h) = ride(1);
    put(&mut s, 1, x - 150, 1280);
    for _ in 0..80 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false))]);
        if rider(&s, 1).hits > 0 {
            break;
        }
    }
    assert_eq!(rider(&s, 1).hits, 1, "a walker is hurt");
    assert_eq!(s.foes[i].hp, 1, "and the snuffer stands");
}

#[test]
fn bouncing_on_a_foe_springs_him_up_and_a_popped_foe_re_forms_later() {
    let i = foe_after(FoeKind::Snuffer, 900);
    let (mut s, h) = ride(1);
    let x = level().foes[i].x;
    put(&mut s, 1, x - 110, 1100);
    let mut bounced = false;
    for t in 0..120 {
        // A leap taken early enough to come down on it.
        let jump = (4..16).contains(&t);
        tick(&mut s, &h, &[(1, keys(1, false, jump, false))]);
        let r = rider(&s, 1);
        if s.foes[i].hp == 0 {
            bounced = r.vy < 0 && !r.grounded();
            break;
        }
    }
    assert!(bounced, "popped from above and thrown up");
    assert_eq!(rider(&s, 1).chain, 1);
    // Out of sight, it comes back.
    put(&mut s, 1, x - 900, 0);
    for _ in 0..REFORM + 40 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
    }
    assert_eq!(s.foes[i].hp, 1, "re-formed for the pack behind");
}

#[test]
fn a_hit_scatters_the_sparks_in_an_arc_and_they_can_be_taken_back() {
    let b = foe_after(FoeKind::Bramble, 600);
    let x = level().foes[b].x;
    let (mut s, h) = ride(1);
    put(&mut s, 1, x - 60, 700);
    s.seats.get_mut(&1).unwrap().rider.sparks = 30;
    let mut scattered = None;
    for _ in 0..40 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
        let r = rider(&s, 1);
        if r.hits > 0 {
            scattered = Some(r.loose.len());
            break;
        }
    }
    assert_eq!(scattered, Some(SCATTER_MAX as usize), "sixteen fly, the rest are lost");
    let r = rider(&s, 1);
    assert_eq!(r.sparks, 0);
    assert!(r.hurt > 0 && r.down == 0, "hurt, not down");
    // They are not all one heading: an arc of them, some left, some right, all up.
    let (lo, hi) = (r.loose.iter().map(|l| l.vx).min().unwrap(), r.loose.iter().map(|l| l.vx).max().unwrap());
    assert!(lo < -200 && hi > 200 && r.loose.iter().all(|l| l.vy < 0), "{lo} {hi}");
    // They bounce on the road and wait to be taken: stand under them.
    let mut taken = 0;
    for _ in 0..90 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
        taken = taken.max(rider(&s, 1).sparks);
    }
    assert!(taken >= 1, "some were won back");
    // After their time they are gone.
    for _ in 0..LOOSE_LIFE {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
    }
    assert!(rider(&s, 1).loose.is_empty());
}

#[test]
fn a_hit_with_no_sparks_fells_him_and_the_lantern_carries_him_home_in_under_a_second() {
    let b = foe_after(FoeKind::Bramble, 600);
    let x = level().foes[b].x;
    let (mut s, h) = ride(1);
    // A post lit far ahead is where he will wake.
    s.posts |= 1;
    put(&mut s, 1, x - 60, 700);
    let mut fell = 0;
    for t in 0..120 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
        if fell == 0 && rider(&s, 1).down > 0 {
            fell = t;
        }
        if fell > 0 && rider(&s, 1).down == 0 {
            assert!(t - fell <= DOWN_TICKS + 1, "up again within a second: {}", t - fell);
            let p = level().posts[0];
            let r = rider(&s, 1);
            assert!((r.x / Q - p.x).abs() <= 2, "set down at the lit post");
            assert_eq!((r.falls, r.sparks), (1, 0));
            assert!(r.hurt > 0, "a moment of grace");
            return;
        }
    }
    panic!("never fell");
}

#[test]
fn a_shield_spark_takes_one_hit_and_scatters_nothing() {
    let b = foe_after(FoeKind::Bramble, 600);
    let x = level().foes[b].x;
    let (mut s, h) = ride(1);
    put(&mut s, 1, x - 60, 700);
    {
        let r = &mut s.seats.get_mut(&1).unwrap().rider;
        r.shield = true;
        r.sparks = 9;
    }
    for _ in 0..40 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
        if rider(&s, 1).hits > 0 {
            break;
        }
    }
    let r = rider(&s, 1);
    assert!(!r.shield && r.sparks == 9 && r.loose.is_empty() && r.down == 0);
}

#[test]
fn a_crate_breaks_to_a_roll_and_gives_a_shield_and_a_hay_bale_springs_him_up() {
    let lvl = level();
    let c = lvl.crates.iter().position(|c| c.kind == CrateKind::Shield).unwrap();
    let cx = lvl.crates[c].x;
    let (mut s, h) = ride(1);
    put(&mut s, 1, cx - 100, 1100);
    for t in 0..60 {
        tick(&mut s, &h, &[(1, keys(1, t > 3, false, false))]);
    }
    let r = rider(&s, 1);
    assert!(r.shield && r.crates & (1 << c) != 0, "the crate broke and gave its shield");
    // A hay bale throws him high, his pace kept.
    let sp = lvl.springs.iter().position(|b| b.kind == SpringKind::Up).unwrap();
    let (mut s, h) = ride(1);
    put(&mut s, 1, lvl.springs[sp].x - 70, 1100);
    let mut best = i32::MAX;
    for _ in 0..70 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false))]);
        best = best.min(rider(&s, 1).y);
    }
    let ground = lvl.springs[sp].y * Q;
    assert!(ground - best > 70 * Q, "up past seventy pixels: {}", (ground - best) / Q);
    assert!(s.spring_t[sp] > 0, "the bale remembers it was sprung");
}

#[test]
fn a_pad_kicks_a_slow_rider_to_the_pad_s_pace_and_lets_a_faster_one_be() {
    let lvl = level();
    let p = lvl.pads[0];
    let (mut s, h) = ride(1);
    put(&mut s, 1, p.x - 40, 600);
    for _ in 0..20 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false))]);
    }
    assert!(rider(&s, 1).gsp > TOP + 400, "kicked past the pace: {}", rider(&s, 1).gsp);
    let (mut s, h) = ride(1);
    put(&mut s, 1, p.x - 40, 2400);
    for _ in 0..8 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false))]);
    }
    assert!(rider(&s, 1).gsp > 2300, "a faster rider keeps his speed");
}

#[test]
fn a_post_lit_by_one_knight_is_lit_for_all_and_holds_the_dimming_back() {
    let lvl = level();
    let post = lvl.posts[0];
    let (mut s, h) = ride(2);
    s.dim = (post.x - 90) * Q;
    put(&mut s, 2, post.x - 80, 1000);
    put(&mut s, 1, post.x + 400, 0);
    for _ in 0..40 {
        tick(&mut s, &h, &[(2, keys(1, false, false, false))]);
        if s.posts & 1 == 1 {
            break;
        }
    }
    assert_eq!(s.posts & 1, 1, "lit");
    assert!(s.dim <= (post.x - HOLD + 400) * Q, "and the Dimming thrown back: {}", s.dim / Q);
    assert!(s.post_t[0] > 0);
    // Knight 1 never touched it, yet a fall wakes him at that post.
    let b = foe_after(FoeKind::Bramble, 600);
    let bx = level().foes[b].x;
    s.dim = -4000 * Q;
    put(&mut s, 1, bx - 60, 700);
    for _ in 0..80 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
    }
    let r = rider(&s, 1);
    assert!((r.x / Q - post.x).abs() <= 3 && r.falls == 1, "carried to the pack's post: {} falls {} hits {} down {}", r.x / Q, r.falls, r.hits, r.down);
}

#[test]
fn a_lance_lock_dashes_at_the_nearest_foe_and_chains_to_the_next_refilling_the_gauge() {
    let lvl = level();
    let floaters: Vec<usize> = (0..lvl.foes.len()).filter(|&i| lvl.foes[i].kind == FoeKind::Floater && lvl.foes[i].min_seats == 0).collect();
    let first = lvl.foes[floaters[0]];
    let (mut s, h) = ride(1);
    {
        let r = &mut s.seats.get_mut(&1).unwrap().rider;
        *r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(first.x - 70));
        // Airborne beside the first wisp, drifting forward.
        r.ground = None;
        r.x = (first.x - 70) * Q;
        r.y = (first.y + 20) * Q;
        r.vx = 800;
        r.vy = -300;
        r.gauge = 0;
        r.jumping = false;
        r.snap_camera(first.x, first.y);
    }
    let mut pressed = false;
    for _ in 0..160 {
        let r = rider(&s, 1);
        // A fresh press each time a wisp is in the cone, as a player taps.
        let want = r.lock.is_none() && r.lock_cd == 0 && !r.grounded() && s.find_lock(r, &NUM).is_some();
        let press = want && !r.jump_key;
        pressed |= press;
        tick(&mut s, &h, &[(1, keys(1, false, press, false))]);
        if rider(&s, 1).grounded() {
            break;
        }
    }
    let r = rider(&s, 1);
    assert!(pressed);
    assert!(r.best_chain >= 3, "a chain of three or more: {}", r.best_chain);
    assert!(r.gauge >= 300, "the chain refills the gauge: {}", r.gauge);
    assert!(r.kills >= 3);
}

#[test]
fn with_no_foe_in_the_cone_a_press_in_the_air_lunges_once() {
    let (mut s, h) = ride(1);
    put(&mut s, 1, 400, 1000);
    let mut lunged = 0;
    for t in 0..40 {
        tick(&mut s, &h, &[(1, keys(1, false, t % 4 < 2 && t > 8 || t < 4, false))]);
        if rider(&s, 1).lunge > 0 {
            lunged += 1;
        }
    }
    assert!(lunged > 0 && lunged <= LUNGE_TICKS as usize + 1);
}

#[test]
fn sparks_taken_fill_the_spur_gauge_and_are_taken_once_per_knight() {
    let lvl = level();
    let (mut s, h) = ride(2);
    put(&mut s, 1, 200, 800);
    put(&mut s, 2, 200, 800);
    s.seats.get_mut(&1).unwrap().rider.gauge = 0;
    for _ in 0..60 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false)), (2, keys(1, false, false, false))]);
    }
    let (a, b) = (rider(&s, 1), rider(&s, 2));
    assert!(a.sparks >= 4 && a.gauge >= 4 * SPARK_GAUGE - 12, "{} sparks, gauge {}", a.sparks, a.gauge);
    assert_eq!(a.sparks, b.sparks, "each knight takes the same road's sparks for himself");
    let first = lvl.sparks.iter().position(|p| p.0 >= 220).unwrap();
    assert!(a.taken[first / 64] >> (first % 64) & 1 == 1);
}

#[test]
fn the_dimming_swallows_a_straggler_and_the_lantern_carries_him_home_to_be_held_back() {
    let (mut s, h) = ride(1);
    s.dim = 0;
    let mut fell = false;
    let mut home = false;
    for _ in 0..400 {
        tick(&mut s, &h, &[(1, keys(0, false, false, false))]);
        let r = rider(&s, 1);
        fell |= r.down > 0;
        if fell && r.down == 0 {
            home = true;
            break;
        }
    }
    assert!(fell && home, "swallowed and carried");
    let r = rider(&s, 1);
    assert_eq!(r.falls, 1);
    assert!(s.dim < r.x - 300 * Q, "and the Dimming is held off him again: {}", (r.x - s.dim) / Q);
}

#[test]
fn more_knights_mean_a_quicker_dimming_more_foes_and_a_tougher_warden_never_less() {
    let pace = |n: u32| {
        let (mut s, h) = ride(n);
        // Just behind the rearmost knight, inside the chase's reach.
        s.dim = -300 * Q;
        let before = s.dim;
        tick(&mut s, &h, &[]);
        s.dim - before
    };
    let paces: Vec<i32> = (1..=6).map(pace).collect();
    assert!(paces.windows(2).all(|w| w[0] < w[1]), "{paces:?}");
    let lvl = level();
    let awake = |n: u32| (0..lvl.foes.len()).filter(|&i| lvl.foes[i].min_seats <= n).count();
    let counts: Vec<usize> = (1..=6).map(awake).collect();
    assert!(counts.windows(2).all(|w| w[0] <= w[1]) && counts[5] > counts[0] + 8, "{counts:?}");
    let w = lvl.foes.iter().position(|f| f.kind == FoeKind::Warden).unwrap();
    let hits: Vec<u8> = (1..=6).map(|n| ride(n).0.foes[w].hp).collect();
    assert_eq!(hits, [3, 4, 5, 6, 7, 8]);
    // A knight who leaves does not make it weaker; one who joins makes it harder.
    let (mut s, mut h) = ride(4);
    s.foes[w].hp -= 1;
    h.remove(&4);
    tick(&mut s, &h, &[]);
    assert_eq!(s.foes[w].hp, 5, "leaving takes nothing off");
    h.insert(4, Hero::new("Knight 4", 0.0, 0.0));
    h.insert(5, Hero::new("Knight 5", 0.0, 0.0));
    tick(&mut s, &h, &[]);
    assert_eq!((s.foes[w].hp, s.provisioned), (6, 5), "one more seat provisioned: one more hit; the rejoining one not twice");
}

#[test]
fn a_pack_s_wake_carries_the_knight_behind_past_a_lone_rider() {
    let (mut s, h) = ride(3);
    for (id, x) in [(1, 150), (2, 120), (3, 2)] {
        put(&mut s, id, x, 1200);
    }
    let mut drafted = false;
    for _ in 0..70 {
        tick(
            &mut s,
            &h,
            &[1, 2, 3].map(|id| (id, keys(1, false, false, false))),
        );
        drafted |= rider(&s, 2).draft;
    }
    assert!(drafted, "the knight close behind is in the wake");
    assert!(!rider(&s, 3).draft, "the lone rider far behind is not");
    assert!(rider(&s, 2).gsp >= rider(&s, 3).gsp, "{} against {}", rider(&s, 2).gsp, rider(&s, 3).gsp);
}

#[test]
fn a_knight_who_joins_mid_run_sits_down_at_the_last_post() {
    let lvl = level();
    let (mut s, mut h) = ride(1);
    s.posts |= 0b11;
    tick(&mut s, &h, &[]);
    h.insert(2, Hero::new("Knight 2", 0.0, 0.0));
    tick(&mut s, &h, &[]);
    let late = &s.seats[&2];
    assert!(late.joined > 0);
    assert!((late.rider.x / Q - lvl.posts[1].x).abs() <= 2, "at the last lit post");
    assert_eq!(s.provisioned, 2);
    assert!(s.valid());
}

#[test]
fn the_dimming_stops_at_the_bell_and_the_pack_has_time_to_ring_it() {
    let lvl = level();
    let (mut s, h) = ride(2);
    s.foes.iter_mut().zip(&lvl.foes).for_each(|(f, spot)| {
        if spot.kind == FoeKind::Warden {
            f.hp = 0;
        }
    });
    let bx = lvl.bell.0;
    put(&mut s, 1, bx - 120, 1300);
    put(&mut s, 2, bx - 700, 0);
    let mut dim = 0;
    for t in 0..GRACE + 60 {
        tick(&mut s, &h, &[(1, keys(1, false, false, false))]);
        if s.bell_t.is_some() && dim == 0 {
            dim = s.dim;
        }
        if s.tally.is_some() {
            assert!(t >= 5, "not before the first bell");
            break;
        }
    }
    let y = s.tally.as_ref().expect("a tally at the end of the grace");
    assert_eq!(s.dim, dim, "the Dimming held at the bell");
    assert!(y.points <= 1000 && y.scores.len() == 2);
    assert!(s.seats.values().all(|k| k.rider.finish.is_some()));
}

#[test]
fn the_five_numbers_move_what_they_say() {
    let run_loop = |n: Numbers| {
        let lvl = level();
        let mut r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(4650));
        r.gsp = 1150;
        let mut ev = Vec::new();
        for _ in 0..200 {
            r.step(Keys { x: 1, ..Keys::default() }, &lvl.terrain, &n, &mut ev);
        }
        (ev.contains(&Ev::Fell), r.x / Q)
    };
    let base = Numbers::default();
    assert!(!run_loop(base).0, "a good pace holds the wall");
    assert!(run_loop(Numbers { loop_min: 1100, ..base }).0, "a higher number asks for more speed");
    // Slope: the same downhill gives more speed at a higher number.
    let down = |slope: i32| {
        let lvl = level();
        let mut r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(1730));
        r.gsp = 700;
        let mut ev = Vec::new();
        for _ in 0..30 {
            r.step(Keys::default(), &lvl.terrain, &Numbers { slope, ..base }, &mut ev);
        }
        r.gsp
    };
    assert!(down(200) > down(100) && down(100) > down(40), "{} {} {}", down(40), down(100), down(200));
    // Spur: the cap.
    let spur = |cap: i32| {
        let lvl = level();
        let mut r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(100));
        r.gsp = 1280;
        r.gauge = GAUGE_MAX;
        let mut ev = Vec::new();
        for _ in 0..40 {
            r.step(Keys { x: 1, spur: true, ..Keys::default() }, &lvl.terrain, &Numbers { spur_cap: cap, ..base }, &mut ev);
        }
        r.gsp
    };
    assert!(spur(2800) > spur(1600) + 300);
    // Lock: the reach.
    let (mut s, _) = ride(1);
    let lvl = level();
    let first = lvl.foes[lvl.foes.iter().position(|f| f.kind == FoeKind::Floater).unwrap()];
    {
        let r = &mut s.seats.get_mut(&1).unwrap().rider;
        r.ground = None;
        r.x = (first.x - 140) * Q;
        r.y = first.y * Q;
    }
    let r = rider(&s, 1).clone();
    assert!(s.find_lock(&r, &Numbers { lock_reach: 96, ..base }).is_none());
    assert!(s.find_lock(&r, &Numbers { lock_reach: 200, ..base }).is_some());
    // Dimming: the pace.
    let pace = |d: i32| {
        let h = heroes(1);
        let mut s = Session::new(&h);
        let before = s.dim;
        s.step(&BTreeMap::new(), &h, &Numbers { dimming: d, ..base });
        s.dim - before
    };
    assert!(pace(1200) > pace(1000) && pace(1000) > pace(100));
}

#[test]
fn no_valley_is_a_trap_a_knight_at_rest_in_any_dip_rides_out_without_the_spur() {
    use super::bot::{Route, Skill};
    // The trench's foot, the half-pipe's floor, the rhythm's valleys, the
    // valley road, the fen's floor and the marsh.
    let bottoms = [1855, 5420 + 160, 7820 + 120, 7820 + 420, 7820 + 720, 8720 + 450, 11300 + 900, 15000 + 330];
    for x in bottoms {
        let (mut s, h) = ride(1);
        put(&mut s, 1, x, 0);
        let mut out = false;
        for _ in 0..600 {
            let input = s.drive_as(1, Route::Lower, &NUM, Skill::SLOW);
            let map = BTreeMap::from([(1, input)]);
            s.step(&map, &h, &NUM);
            s.dim = -4000 * Q;
            if rider(&s, 1).x / Q > x + 340 {
                out = true;
                break;
            }
        }
        assert!(out, "stuck at the dip by column {x}: now at {}", rider(&s, 1).x / Q);
    }
}

#[test]
fn the_real_loops_hold_a_gallop_and_a_spur_and_drop_a_rider_who_slowed() {
    let lvl = level();
    // Each loop's mouth, and how long a run-up he gets.
    for (mouth, run) in [(4700, 340), (16760, 440)] {
        let go = |from: i32, spur: bool| {
            let mut r = Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(from));
            let mut ev = Vec::new();
            for _ in 0..400 {
                r.step(Keys { x: 1, spur, ..Keys::default() }, &lvl.terrain, &NUM, &mut ev);
            }
            (ev.contains(&Ev::Fell), r.x / Q)
        };
        // A full gallop from the foot of the run: round and away.
        let (fell, x) = go(mouth - run, false);
        assert!(!fell && x > mouth + 400, "a gallop holds the loop at {mouth}: fell {fell} x {x}");
        // Standing a few strides off the mouth, the trot falls from the wall,
        let (fell, _) = go(mouth - 40, false);
        assert!(fell, "a trot falls at {mouth}");
        // and the spur, from the same place, goes round.
        let (fell, x) = go(mouth - 40, true);
        assert!(!fell && x > mouth + 400, "the spur makes it at {mouth}: fell {fell} x {x}");
    }
}

/// A small deterministic dice: no clock, no ambient randomness.
struct Dice(u64);
impl Dice {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 20) as u32
    }
}

#[test]
fn a_fuzz_of_keys_and_teleports_never_breaks_a_ride_or_its_state() {
    for seed in [3u64, 99, 2026] {
        let mut dice = Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let (mut s, h) = ride(6);
        let mut held: BTreeMap<u32, (Input, u32)> = BTreeMap::new();
        for t in 0..6000 {
            let mut inputs = BTreeMap::new();
            for id in 1..=6u32 {
                let (k, left) = held.entry(id).or_insert((Input::default(), 0));
                if *left == 0 {
                    let r = dice.next();
                    *k = Input {
                        move_x: (r % 3) as i8 - 1,
                        move_y: ((r >> 2) % 4) as i8 - 1,
                        interact: (r >> 4) % 3 == 0,
                        dash: (r >> 6) % 4 == 0,
                        fire: (r >> 8) % 7 == 0,
                        ..Default::default()
                    };
                    *left = 1 + (r >> 10) % 25;
                }
                *left -= 1;
                inputs.insert(id, *k);
            }
            // Now and then a knight is set down somewhere else on the road, at
            // any speed, even on a loop's ceiling or a rail.
            if t % 97 == 0 {
                let id = 1 + dice.next() % 6;
                let lvl = level();
                let r = &mut s.seats.get_mut(&id).unwrap().rider;
                let path = (dice.next() as usize) % lvl.terrain.paths.len();
                let len = lvl.terrain.paths[path].len();
                let at = (dice.next() as i64 * i64::from(len) / i64::from(u32::MAX >> 12)) as i32 % len;
                *r = Rider::new_on(&lvl.terrain, path, at);
                r.gsp = (dice.next() % 4000) as i32 - 1500;
                r.dir = if r.gsp < 0 { -1 } else { 1 };
                r.snap_camera(r.x / Q, r.y / Q);
            }
            s.step(&inputs, &h, &NUM);
            if t % 7 == 0 {
                assert!(s.valid(), "seed {seed} tick {t}");
            }
        }
        // The whole state is plain data: it survives the wire.
        let json = serde_json::to_string(&s).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert!(back == s && back.valid());
    }
}
