//! Bannerpeak's feel: the jump vocabulary measured on blocks. Each check puts
//! a body on a bare test stage and presses keys tick by tick, the way a
//! playtester would, and reads back heights, distances and ticks in pixels.
use super::body::*;
use super::world::*;

const K: Knobs = Knobs {
    run: 1152,
    hop: 10,
    grace: 5,
    slam: 36,
    ledge: 3000,
};

/// A flat stage: `air` empty rows over a floor two rows thick of `tile`.
fn flat_of(width: usize, air: usize, tile: &str) -> World {
    let mut rows: Vec<String> = Vec::new();
    for _ in 0..air {
        rows.push(".".repeat(width));
    }
    for _ in 0..2 {
        rows.push(tile.repeat(width));
    }
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    World::parse(&refs)
}
fn flat(width: usize, air: usize) -> World {
    flat_of(width, air, "#")
}

/// A body, its stage and the keys it has been given.
struct Rig {
    world: World,
    body: Body,
    log: Vec<(usize, Ev)>,
    ticks: usize,
    jump: bool,
    down: bool,
    knobs: Knobs,
}

impl Rig {
    /// The body stands at `x` tiles (its left edge), feet on `floor_row`'s top.
    fn on(world: World, x: i32, floor_row: i32) -> Rig {
        Rig {
            body: Body::new(x * TILE, floor_row * TILE),
            world,
            log: Vec::new(),
            ticks: 0,
            jump: false,
            down: false,
            knobs: K,
        }
    }
    fn keys(&mut self, x: i8, jump: bool, down: bool, dive: bool, brace: bool) {
        let c = Cmd {
            x,
            jump,
            jump_edge: jump && !self.jump,
            down,
            down_edge: down && !self.down,
            dive_edge: dive,
            brace,
        };
        self.jump = jump;
        self.down = down;
        let mut out = Vec::new();
        self.body
            .step(c, &self.knobs, &self.world, &[], &[], &mut out);
        for e in out {
            self.log.push((self.ticks, e));
        }
        self.ticks += 1;
    }
    fn run(&mut self, x: i8, n: usize) {
        for _ in 0..n {
            self.keys(x, false, false, false, false);
        }
    }
    fn px(&self) -> (i32, i32) {
        self.body.foot()
    }
    fn saw(&self, f: impl Fn(&Ev) -> bool) -> bool {
        self.log.iter().any(|(_, e)| f(e))
    }
}

/// One jump with the key held for `hold` ticks, the run continuing (`x`)
/// until the body is down again. Returns the rise in pixels, the ticks in
/// the air and the distance covered.
fn arc(r: &mut Rig, x: i8, hold: usize, crouch: usize) -> (i32, usize, i32) {
    let (x0, y0) = r.px();
    let (mut apex, mut air) = (0, 0);
    for n in 0..150 {
        r.keys(x, n < hold, n < crouch, false, false);
        apex = apex.max(y0 - r.px().1);
        if !r.body.ground {
            air += 1;
        } else if n > 3 {
            break;
        }
    }
    (apex, air, (r.px().0 - x0).abs())
}

/// The body running at top speed on a long flat stage.
fn runner() -> Rig {
    let mut r = Rig::on(flat(260, 20), 2, 20);
    r.run(1, 40);
    assert!(r.body.vx >= K.run, "at top speed");
    r
}

#[test]
fn a_press_leaves_the_ground_the_same_tick() {
    let mut r = Rig::on(flat(30, 10), 5, 10);
    r.run(0, 4);
    r.keys(0, true, false, false, false);
    assert!(!r.body.ground, "off the ground on the press tick");
    assert!(r.body.vy < 0, "and already rising");
    assert!(r.saw(|e| matches!(e, Ev::Jump(Jump::Plain))));
}

#[test]
fn a_standing_jump_clears_three_tiles_and_not_four() {
    let mut r = Rig::on(flat(30, 10), 5, 10);
    r.run(0, 4);
    let (apex, air, dist) = arc(&mut r, 0, 40, 0);
    assert!((52..=64).contains(&apex), "standing apex {apex}px");
    assert!((22..=32).contains(&air), "air time {air} ticks");
    assert!(dist <= 3, "straight up");
}

#[test]
fn jump_height_grows_with_run_speed() {
    let mut still = Rig::on(flat(60, 10), 2, 10);
    still.run(0, 4);
    let (slow, _, _) = arc(&mut still, 0, 40, 0);
    let mut fast = runner();
    let (high, _, dist) = arc(&mut fast, 1, 40, 0);
    assert!(high >= slow + 10, "a running jump is higher: {high} vs {slow}");
    assert!((68..=82).contains(&high), "run apex {high}px");
    assert!((120..=150).contains(&dist), "run jump spans {dist}px");
}

#[test]
fn a_tapped_jump_is_short_and_a_held_one_is_tall() {
    let mut a = Rig::on(flat(30, 10), 5, 10);
    a.run(0, 4);
    let (tap, _, _) = arc(&mut a, 0, 2, 0);
    let mut b = Rig::on(flat(30, 10), 5, 10);
    b.run(0, 4);
    let (held, _, _) = arc(&mut b, 0, 40, 0);
    assert!(tap + 20 <= held, "tap {tap} against held {held}");
    assert!(tap >= 14, "a tap still leaves the floor by a tile: {tap}");
}

/// Ticks until the body would touch down if left alone, found on a copy.
fn ticks_until_ground(r: &Rig) -> i32 {
    let mut b = r.body.clone();
    for n in 0..80 {
        let mut out = Vec::new();
        b.step(Cmd::default(), &K, &r.world, &[], &[], &mut out);
        if b.ground {
            return n + 1;
        }
    }
    99
}

#[test]
fn a_press_just_before_landing_is_kept_for_the_buffer() {
    for (early, expect) in [(1, true), (3, true), (5, true), (6, false), (9, false)] {
        let mut r = Rig::on(flat(30, 12), 5, 12);
        r.run(0, 4);
        // A short hop, then fall; press `early` ticks before the floor.
        r.keys(0, true, false, false, false);
        r.keys(0, true, false, false, false);
        let jumps_before = r
            .log
            .iter()
            .filter(|(_, e)| matches!(e, Ev::Jump(_)))
            .count();
        let mut pressed = false;
        for _ in 0..80 {
            let press = !pressed && r.body.vy > 0 && ticks_until_ground(&r) == early;
            pressed |= press;
            r.keys(0, press, false, false, false);
            if pressed && r.body.ground {
                // One more tick for the buffered press to act on the floor.
                r.keys(0, false, false, false, false);
                break;
            }
        }
        let jumps = r
            .log
            .iter()
            .filter(|(_, e)| matches!(e, Ev::Jump(_)))
            .count();
        assert_eq!(jumps > jumps_before, expect, "pressed {early} ticks early");
    }
}

#[test]
fn a_late_press_off_a_ledge_still_jumps_for_the_coyote_time() {
    let rows = [
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "#####.....",
        "#####.....",
    ];
    for (late, expect) in [(1usize, true), (3, true), (5, true), (7, false), (9, false)] {
        let mut r = Rig::on(World::parse(&rows), 3, 9);
        r.run(1, 6);
        let mut off = None;
        for n in 0..60 {
            if !r.body.ground && off.is_none() {
                off = Some(n);
            }
            let press = off.is_some_and(|o| n == o + late);
            r.keys(1, press, false, false, false);
            if press {
                break;
            }
        }
        assert!(off.is_some(), "walked off the shelf");
        let jumped = r.saw(|e| matches!(e, Ev::Jump(_)));
        assert_eq!(jumped, expect, "pressed {late} ticks after leaving the shelf");
    }
}

#[test]
fn running_has_acceleration_and_a_skid() {
    let mut r = Rig::on(flat(120, 10), 2, 10);
    let mut to_top = None;
    for n in 0..60 {
        r.keys(1, false, false, false, false);
        if r.body.vx >= K.run && to_top.is_none() {
            to_top = Some(n + 1);
        }
    }
    let t = to_top.expect("reaches top speed");
    assert!((14..=26).contains(&t), "top speed in {t} ticks");
    // Reversing at speed skids, then turns.
    let mut skid_ticks = 0;
    for _ in 0..40 {
        r.keys(-1, false, false, false, false);
        if r.body.skid {
            skid_ticks += 1;
        }
        if r.body.vx < 0 {
            break;
        }
    }
    assert!(skid_ticks >= 3, "a skid you can see: {skid_ticks} ticks");
    assert!(r.saw(|e| matches!(e, Ev::Skid)));
    // Letting go coasts to a stop in about half a second.
    let mut s = runner();
    let mut ticks = 0;
    while s.body.vx != 0 && ticks < 60 {
        s.keys(0, false, false, false, false);
        ticks += 1;
    }
    assert!((8..=22).contains(&ticks), "stops in {ticks} ticks");
}

/// Three hops in rhythm, `wait` ticks of running between landings: each hop's
/// rise.
fn hops(hop_window: u32, wait: usize) -> Vec<i32> {
    let mut r = runner();
    r.knobs.hop = hop_window;
    let mut rises = Vec::new();
    for _ in 0..3 {
        let (rise, _, _) = arc(&mut r, 1, 18, 0);
        rises.push(rise);
        r.run(1, wait);
    }
    rises
}

#[test]
fn three_landings_in_rhythm_make_a_triple_jump() {
    let h = hops(10, 0);
    assert!(h[0] < h[1] && h[1] < h[2], "each higher: {h:?}");
    assert!((115..=135).contains(&h[2]), "the third is a big somersault: {h:?}");
    assert!(h[2] >= h[0] * 3 / 2, "third well above the first: {h:?}");
    let mut r = runner();
    for _ in 0..3 {
        arc(&mut r, 1, 18, 0);
    }
    assert!(r.saw(|e| matches!(e, Ev::Jump(Jump::Second))));
    assert!(r.saw(|e| matches!(e, Ev::Jump(Jump::Triple))));
}

#[test]
fn the_hop_window_is_the_knob_and_waiting_loses_the_chain() {
    // Still in time, a few ticks after landing.
    let in_time = hops(10, 6);
    assert!(in_time[2] >= 115, "{in_time:?}");
    // Past the window, the chain is gone.
    let late = hops(10, 12);
    assert!(late[2] <= late[0] + 6, "{late:?}");
    // A longer window forgives the wait; none at all never chains.
    let wide = hops(20, 12);
    assert!(wide[2] >= 115, "{wide:?}");
    let none = hops(0, 0);
    assert!(none[2] <= none[0] + 6, "{none:?}");
}

#[test]
fn a_hop_chain_needs_speed() {
    let mut r = Rig::on(flat(100, 14), 2, 14);
    let mut rises = Vec::new();
    for _ in 0..3 {
        rises.push(arc(&mut r, 0, 18, 0).0);
    }
    assert!(rises.iter().all(|&h| h >= 40), "they did jump: {rises:?}");
    assert!(rises[2] <= rises[0] + 4, "standing hops stay equal: {rises:?}");
}

#[test]
fn a_long_jump_is_low_and_much_longer_than_a_run_jump() {
    let mut run = runner();
    let (run_apex, _, run_dist) = arc(&mut run, 1, 40, 0);
    let mut long = runner();
    // Crouch while running; jump with the crouch still held.
    long.keys(1, false, true, false, false);
    long.keys(1, false, true, false, false);
    let (apex, _, dist) = arc(&mut long, 1, 40, 3);
    assert!(long.saw(|e| matches!(e, Ev::Jump(Jump::Long))));
    assert!(apex + 14 <= run_apex, "lower: {apex} against {run_apex}");
    assert!(dist * 2 >= run_dist * 3, "farther: {dist} against {run_dist}");
    assert!(dist >= 185, "{dist}px");
}

#[test]
fn crouch_then_jump_standing_is_a_backflip() {
    let mut plain = Rig::on(flat(40, 14), 20, 14);
    plain.run(0, 4);
    let (still, _, _) = arc(&mut plain, 0, 40, 0);
    let mut b = Rig::on(flat(40, 14), 20, 14);
    b.run(0, 4);
    b.keys(0, false, true, false, false);
    b.keys(0, false, true, false, false);
    let x0 = b.px().0;
    let (apex, _, _) = arc(&mut b, 0, 40, 3);
    assert!(b.saw(|e| matches!(e, Ev::Jump(Jump::Back))));
    assert!(apex >= still + 28, "higher than a plain jump: {apex} against {still}");
    assert!(b.px().0 < x0, "and backwards");
}

/// A shaft four tiles wide with a floor.
fn shaft(rows: usize) -> World {
    let mut v: Vec<String> = Vec::new();
    for _ in 0..rows {
        v.push("###....###".to_string());
    }
    v.push("##########".to_string());
    let refs: Vec<&str> = v.iter().map(String::as_str).collect();
    World::parse(&refs)
}

#[test]
fn two_close_walls_are_climbed_by_wall_kicks() {
    let mut r = Rig::on(shaft(40), 4, 40);
    let start = r.px().1;
    let mut dir: i8 = 1;
    for n in 0..14 {
        r.keys(dir, n < 14, false, false, false);
    }
    let mut kicks = 0;
    let mut best = 0;
    for _ in 0..600 {
        let sliding = r.body.mode == Move::Wall;
        r.keys(dir, sliding, false, false, false);
        if sliding && r.body.jump == Jump::Kick {
            kicks += 1;
            dir = -dir;
        }
        best = best.max(start - r.px().1);
        if best >= 360 {
            break;
        }
    }
    assert!(kicks >= 5, "kicked off the walls {kicks} times");
    assert!(best >= 360, "climbed {best}px between the walls");
    assert!(r.saw(|e| matches!(e, Ev::Jump(Jump::Kick))));
}

#[test]
fn a_wall_slide_is_slow_and_ends_on_the_floor() {
    let mut r = Rig::on(shaft(20), 4, 20);
    for n in 0..14 {
        r.keys(1, n < 14, false, false, false);
    }
    let mut slid = 0;
    for _ in 0..200 {
        r.keys(1, false, false, false, false);
        if r.body.mode == Move::Wall {
            slid += 1;
            assert!(r.body.vy <= WALL_SLIDE, "slow on the wall");
        }
        if r.body.ground {
            break;
        }
    }
    assert!(slid >= 8, "slid for {slid} ticks");
    assert!(r.body.ground && r.body.mode == Move::Ground);
}

#[test]
fn a_slam_hovers_drops_and_lands_with_a_shock_and_a_hitstop() {
    let mut r = Rig::on(flat(30, 14), 5, 14);
    r.run(0, 4);
    for n in 0..8 {
        r.keys(0, n < 20, false, false, false);
    }
    r.keys(0, true, true, false, false);
    assert_eq!(r.body.mode, Move::Wind, "hover first");
    let hover = r.px().1;
    for _ in 0..4 {
        r.keys(0, true, true, false, false);
    }
    assert!((r.px().1 - hover).abs() <= 1, "held in place while it winds up");
    let mut landed = false;
    for _ in 0..30 {
        r.keys(0, true, true, false, false);
        if r.saw(|e| matches!(e, Ev::Slam { .. })) {
            landed = true;
            break;
        }
    }
    assert!(landed, "the slam lands");
    assert_eq!(r.body.freeze, 3, "a hit-stop on the knight");
    assert_eq!(r.body.mode, Move::Recover);
    // A jump pressed on landing springs high.
    r.keys(0, false, false, false, false);
    r.keys(0, false, false, false, false);
    r.keys(0, false, false, false, false);
    r.keys(0, false, false, false, false);
    r.keys(0, true, false, false, false);
    let y0 = r.px().1;
    let mut apex = 0;
    for _ in 0..60 {
        r.keys(0, true, false, false, false);
        apex = apex.max(y0 - r.px().1);
    }
    assert!(apex >= 80, "a slam-jump rises {apex}px");
}

#[test]
fn a_dive_throws_the_knight_forward_and_slides_on_the_belly() {
    let mut r = Rig::on(flat(100, 14), 5, 14);
    r.run(1, 30);
    for n in 0..6 {
        r.keys(1, n < 20, false, false, false);
    }
    let x0 = r.px().0;
    r.keys(1, false, false, true, false);
    assert_eq!(r.body.mode, Move::Dive);
    let mut belly = false;
    for _ in 0..50 {
        r.keys(1, false, false, false, false);
        belly |= r.body.mode == Move::Belly;
    }
    assert!(belly, "lands sliding");
    assert!(r.px().0 > x0 + 60, "carried far: {}", r.px().0 - x0);
    assert_eq!(r.body.mode, Move::Ground, "and gets up");
}

#[test]
fn snow_is_slippery_and_stone_is_not() {
    let stop = |mut r: Rig| {
        r.run(1, 60);
        let x0 = r.px().0;
        r.run(0, 100);
        r.px().0 - x0
    };
    let a = stop(Rig::on(flat(200, 10), 2, 10));
    let b = stop(Rig::on(flat_of(200, 10, "%"), 2, 10));
    assert!(b > a * 2, "snow slides further: {b} against {a}");
}

#[test]
fn a_head_that_clips_a_block_rim_slips_past_and_a_full_hit_bumps() {
    let rows = [
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "....#.....",
        "..........",
        "..........",
        "..........",
        "..........",
        "#########.",
    ];
    // Directly beneath: bump, with the block's cell reported.
    let mut r = Rig::on(World::parse(&rows), 4, 10);
    r.body.x = 72 * Q;
    r.run(0, 3);
    for _ in 0..30 {
        r.keys(0, true, false, false, false);
    }
    assert!(r.saw(|e| matches!(e, Ev::Bump { x: 4, y: 5 })));
    // Clipping the rim by two pixels: slides past, no bump.
    let mut r = Rig::on(World::parse(&rows), 4, 10);
    r.body.x = 60 * Q;
    r.run(0, 3);
    for _ in 0..30 {
        r.keys(0, true, false, false, false);
    }
    assert!(!r.saw(|e| matches!(e, Ev::Bump { .. })), "slipped by");
}

#[test]
fn a_ledge_is_landed_on_from_above_and_passed_through_from_below() {
    let rows = [
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..====....",
        "..........",
        "##########",
    ];
    let mut r = Rig::on(World::parse(&rows), 3, 10);
    r.body.x = 56 * Q;
    r.run(0, 2);
    let mut through = false;
    for n in 0..60 {
        r.keys(0, n < 20, false, false, false);
        if r.px().1 < 8 * TILE {
            through = true;
        }
    }
    assert!(through, "up through the ledge");
    assert!(
        r.body.ground && r.px().1 == 8 * TILE,
        "and standing on it: {:?}",
        r.px()
    );
}

#[test]
fn a_raised_shield_springs_a_friend_higher_than_a_jump() {
    let stage = flat(40, 20);
    let mut a = Rig::on(stage.clone(), 10, 20);
    a.run(0, 4);
    let (own, _, _) = arc(&mut a, 0, 40, 0);
    // The friend stands bracing; the other lands on that shield and jumps.
    let mut guard = Body::new(10 * TILE, 20 * TILE);
    let mut spring = Body::new(10 * TILE, 20 * TILE - 80);
    spring.ground = false;
    spring.mode = Move::Air;
    let mut out = Vec::new();
    let brace = Cmd {
        brace: true,
        ..Cmd::default()
    };
    for _ in 0..8 {
        guard.step(brace, &K, &stage, &[], &[], &mut out);
    }
    let shield = guard.shield(SHIELD_ID + 1).expect("raised after a moment");
    assert_eq!(guard.mode, Move::Brace);
    let movers = [shield];
    let mut standing = 0;
    let mut top = spring.y;
    let mut prev_jump = false;
    for _ in 0..90 {
        if spring.ground && spring.plank.is_some() {
            standing += 1;
        }
        // Press once it has stood a moment, then hold it down.
        let jump = standing >= 3;
        let c = Cmd {
            jump,
            jump_edge: jump && !prev_jump,
            ..Cmd::default()
        };
        prev_jump = jump;
        spring.step(c, &K, &stage, &movers, &movers, &mut out);
        top = top.min(spring.y);
    }
    assert!(out.iter().any(|e| matches!(e, Ev::Jump(Jump::Spring))));
    let rise = (shield.p.y * Q - top) / Q;
    assert!(rise > own + 30, "springs {rise}px against a jump's {own}");
}

// ---------------------------------------------------------------------
// The tuning table's physical rows (`cargo test -p angel-delve --lib
// write_bannerpeak_feel_table -- --ignored --nocapture`): what each knob
// does to the body, in pixels and ticks, the others at their shipped values.

fn latest_coyote_press(grace: u32) -> usize {
    let rows = [
        "..........", "..........", "..........", "..........", "..........", "..........",
        "..........", "..........", "..........", "#####.....", "#####.....",
    ];
    let mut best = 0;
    for late in 0..14usize {
        let mut r = Rig::on(World::parse(&rows), 3, 9);
        r.knobs.grace = grace;
        r.run(1, 6);
        let mut off = None;
        for n in 0..60 {
            if !r.body.ground && off.is_none() {
                off = Some(n);
            }
            let press = off.is_some_and(|o| n == o + late);
            r.keys(1, press, false, false, false);
            if press {
                break;
            }
        }
        if r.saw(|e| matches!(e, Ev::Jump(_))) {
            best = late;
        }
    }
    best
}

fn earliest_buffer_press(grace: u32) -> usize {
    let mut best = 0;
    for early in 1..14usize {
        let mut r = Rig::on(flat(30, 12), 5, 12);
        r.knobs.grace = grace;
        r.run(0, 4);
        r.keys(0, true, false, false, false);
        r.keys(0, true, false, false, false);
        let before = r.log.iter().filter(|(_, e)| matches!(e, Ev::Jump(_))).count();
        let mut pressed = false;
        for _ in 0..80 {
            let press = !pressed && r.body.vy > 0 && {
                let mut b = r.body.clone();
                let mut n = 99;
                for k in 0..80 {
                    let mut out = Vec::new();
                    b.step(Cmd::default(), &r.knobs, &r.world, &[], &[], &mut out);
                    if b.ground {
                        n = k + 1;
                        break;
                    }
                }
                n == early
            };
            pressed |= press;
            r.keys(0, press, false, false, false);
            if pressed && r.body.ground {
                r.keys(0, false, false, false, false);
                break;
            }
        }
        let after = r.log.iter().filter(|(_, e)| matches!(e, Ev::Jump(_))).count();
        if after > before {
            best = early;
        }
    }
    best
}

#[test]
#[ignore = "prints the tuning table's physical rows"]
fn write_bannerpeak_feel_table() {
    for run in [640, 896, 960, 1152, 1408, 1664, 2048] {
        let mut a = Rig::on(flat(300, 20), 2, 20);
        a.knobs.run = run;
        a.run(1, 60);
        let (apex, air, dist) = arc(&mut a, 1, 40, 0);
        let mut b = Rig::on(flat(300, 20), 2, 20);
        b.knobs.run = run;
        b.run(1, 60);
        b.keys(1, false, true, false, false);
        b.keys(1, false, true, false, false);
        let (lapex, _, ldist) = arc(&mut b, 1, 40, 3);
        println!(
            "FEEL run {run:>4}: run-jump {dist:>3}px over {air} ticks rising {apex}px; long jump {ldist}px rising {lapex}px; marsh bog is 160px"
        );
    }
    for window in [0u32, 4, 10, 20, 30] {
        let line: Vec<String> = [0usize, 6, 12]
            .iter()
            .map(|&wait| {
                let mut r = runner();
                r.knobs.hop = window;
                let mut rises = Vec::new();
                for _ in 0..3 {
                    rises.push(arc(&mut r, 1, 18, 0).0);
                    r.run(1, wait);
                }
                format!("wait {wait:>2}: {rises:?}")
            })
            .collect();
        println!("FEEL hop {window:>2}: rises of three hops, {}", line.join("; "));
    }
    for grace in 2..=9u32 {
        println!(
            "FEEL grace {grace}: a jump pressed up to {} ticks before landing is kept; up to {} ticks after leaving a ledge still jumps",
            earliest_buffer_press(grace),
            latest_coyote_press(grace)
        );
    }
}
