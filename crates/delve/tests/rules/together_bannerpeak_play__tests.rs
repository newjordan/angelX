//! Bannerpeak's play: the classic verbs through the session, one at a time,
//! with scripted keys on the real map. Cells are `layout::SECTION.at(col, row)`
//! (the air cell a knight stands in; the floor is the row below).
use super::body::{Knobs, Move, Q};
use super::foes::{CapState, Foe, FoeKind, Item, ItemKind};
use super::knight::{BACK_PIPS, BUBBLE_TICKS, Size, START_PIPS};
use super::layout::{self, HUB, MARSH, SNOW, cell_feet, info};
use super::session::{Phase, Session, WIPE_TICKS};
use super::world::{Tile, TILE};
use super::*;

const KNOBS: Knobs = Knobs {
    run: RUN as i32,
    hop: HOP,
    grace: COYOTE,
    slam: SLAM as i32,
    ledge: LEDGE as i32,
};

/// A table of `seats` knights and the keys they press.
struct Table {
    s: Session,
    heroes: BTreeMap<u32, Hero>,
    said: Vec<&'static str>,
    knobs: Knobs,
}

fn inp(x: i8, jump: bool, down: bool) -> Input {
    Input {
        move_x: x,
        dash: jump,
        move_y: i8::from(down),
        ..Default::default()
    }
}

impl Table {
    fn new(seats: u32) -> Table {
        let mut r = Run::bannerpeak_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        let heroes = r.players.clone();
        Table {
            s: Session::new(&heroes),
            heroes,
            said: Vec::new(),
            knobs: KNOBS,
        }
    }
    fn tick(&mut self, keys: &[(u32, Input)]) {
        let inputs: BTreeMap<u32, Input> = keys.iter().copied().collect();
        self.s
            .step(&inputs, &self.heroes, &self.knobs, &mut self.said);
    }
    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.tick(&[]);
        }
    }
    /// Knight `id` standing in the air cell `cell`, still, settled.
    fn put(&mut self, id: u32, cell: (i32, i32)) {
        let (x, y) = cell_feet(cell);
        let k = self.s.seats.get_mut(&id).unwrap();
        k.body.place(x, y);
        k.safe = (x, y);
        k.hurt = 0;
    }
    fn k(&self, id: u32) -> &Knight {
        &self.s.seats[&id]
    }
    fn heard(&self, name: &str) -> bool {
        self.said.contains(&name)
    }
    /// Press jump (held `hold` ticks), then keep `x` pressed for `after` more.
    fn jump(&mut self, id: u32, x: i8, hold: usize, after: usize) {
        for n in 0..hold + after {
            self.tick(&[(id, inp(x, n < hold, false))]);
        }
    }
}

fn marsh(c: i32, r: i32) -> (i32, i32) {
    MARSH.at(c, r)
}

#[test]
fn a_rune_block_struck_from_below_gives_a_coin_and_goes_dull() {
    let mut t = Table::new(1);
    let (bx, by) = marsh(10, 9);
    assert_eq!(t.s.tile(bx, by), Tile::Coin);
    t.put(1, marsh(10, 13));
    t.idle(10);
    let mut pinged = false;
    for n in 0..30 {
        t.tick(&[(1, inp(0, n < 20, false))]);
        pinged |= t.s.fx.iter().any(|f| f.k == super::session::FxKind::Coin);
    }
    assert_eq!(t.s.tile(bx, by), Tile::Spent, "dull after the strike");
    assert_eq!(t.k(1).coins, 1);
    assert!(t.heard("gold") && t.heard("clang"), "a ping and a bump");
    assert!(pinged, "the coin pings up out of the block");
    // Struck again: nothing more.
    t.idle(30);
    t.jump(1, 0, 20, 10);
    assert_eq!(t.k(1).coins, 1);
}

#[test]
fn a_many_block_gives_a_coin_a_bump_until_it_is_dull() {
    let mut t = Table::new(1);
    let (bx, by) = marsh(28, 9);
    assert_eq!(t.s.tile(bx, by), Tile::Many(5));
    t.s.foes.clear();
    for n in 1..=5u32 {
        t.put(1, marsh(28, 13));
        t.idle(6);
        t.jump(1, 0, 20, 14);
        assert_eq!(t.k(1).coins, n, "bump {n}");
    }
    assert_eq!(t.s.tile(bx, by), Tile::Spent);
}

#[test]
fn a_loaf_rises_out_of_its_block_and_a_small_knight_grows_a_big_one_is_mended() {
    let mut t = Table::new(1);
    let (bx, by) = marsh(11, 9);
    assert_eq!(t.s.tile(bx, by), Tile::Power);
    t.put(1, marsh(11, 13));
    t.idle(8);
    let mut first = None;
    for n in 0..30 {
        t.tick(&[(1, inp(0, n < 20, false))]);
        if first.is_none() && !t.s.items.is_empty() {
            first = Some((t.s.items[0].y, t.s.items[0].x));
            assert!(t.s.items[0].rising(), "it rises out of the block");
            for _ in 0..5 {
                t.tick(&[]);
            }
            assert!(t.s.items[0].y < first.unwrap().0, "up, not out sideways");
            assert_eq!(t.s.items[0].x, first.unwrap().1);
        }
    }
    assert_eq!(t.s.items.len(), 1, "a loaf");
    assert_eq!(t.s.items[0].kind, ItemKind::Loaf);
    assert!(!t.s.items[0].rising(), "out of the block now, and walking");
    // The knight meets it: grown.
    let (ix, iy) = (t.s.items[0].x, t.s.items[0].y);
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.body.place(ix / Q, iy / Q);
    }
    t.tick(&[]);
    assert_eq!(t.k(1).size, Size::Big);
    assert!(t.k(1).body.dims().1 > 20, "a bigger body");
    assert!(t.s.items.is_empty() && t.heard("potion"));
    // A big knight eating another is mended, a wedge pair.
    let k = t.s.seats.get_mut(&1).unwrap();
    k.pips = 4;
    t.s.items.push(Item::new(ItemKind::Loaf, 100, 100));
    t.s.items[0].t = 40;
    let (x, y) = (t.s.items[0].x / Q, t.s.items[0].y / Q);
    t.s.seats.get_mut(&1).unwrap().body.place(x, y);
    t.tick(&[]);
    assert_eq!(t.k(1).pips, 6);
    assert_eq!(t.k(1).size, Size::Big);
}

#[test]
fn a_big_knight_gets_a_flame_that_throws_bouncing_bolts_that_beat_snuffers() {
    let mut t = Table::new(1);
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.size = Size::Big;
        k.body.big = true;
    }
    let (bx, by) = marsh(50, 7);
    assert_eq!(t.s.tile(bx, by), Tile::Power);
    t.s.foes.clear();
    t.put(1, marsh(50, 11));
    t.idle(8);
    t.jump(1, 0, 24, 2);
    assert_eq!(t.s.items.last().map(|i| i.kind), Some(ItemKind::Flame));
    t.idle(30);
    let (ix, iy) = (t.s.items[0].x / Q, t.s.items[0].y / Q);
    t.s.seats.get_mut(&1).unwrap().body.place(ix, iy);
    t.tick(&[]);
    assert_eq!(t.k(1).size, Size::Flame);
    // Fire: a bolt leaves, bounces along the ground and beats the snuffer ahead.
    t.put(1, marsh(14, 13));
    t.s.foes.retain(|f| f.kind == FoeKind::Wisp);
    t.s.foes
        .push(Foe::new(FoeKind::Snuffer, marsh(22, 13), 900));
    let n = t.s.foes.len();
    t.tick(&[(
        1,
        Input {
            fire: true,
            ..Default::default()
        },
    )]);
    assert_eq!(t.s.bolts.len(), 1);
    assert!(t.heard("shot_spark"));
    let mut bounced = false;
    for _ in 0..60 {
        let y = t.s.bolts.first().map(|b| b.y);
        t.tick(&[]);
        if let (Some(a), Some(b)) = (y, t.s.bolts.first().map(|b| b.y)) {
            bounced |= b < a;
        }
    }
    assert!(bounced, "the bolt bounces");
    assert!(
        t.s.foes.iter().filter(|f| f.alive()).count() < n,
        "the snuffer is beaten"
    );
}

/// A snuffer standing on the marsh's first bank, held still.
fn snuffer_at(t: &mut Table, col: i32, kind: FoeKind) -> usize {
    t.s.foes.retain(|f| f.kind == FoeKind::Wisp);
    t.s.foes.push(Foe::new(kind, marsh(col, 13), 901));
    let n = t.s.foes.len() - 1;
    t.s.foes[n].dir = 1;
    n
}

#[test]
fn a_stomp_beats_a_snuffer_and_a_held_jump_bounces_higher() {
    let mut heights = Vec::new();
    for held in [false, true] {
        let mut t = Table::new(1);
        let n = snuffer_at(&mut t, 14, FoeKind::Snuffer);
        t.s.foes[n].vx = 0;
        // High above it, falling.
        {
            let (x, y) = cell_feet(marsh(14, 6));
            let k = t.s.seats.get_mut(&1).unwrap();
            k.body.place(x, y);
            k.body.ground = false;
            k.body.mode = Move::Air;
        }
        let mut bounce = 0;
        for _ in 0..40 {
            let was = t.s.foes[n].alive();
            t.tick(&[(1, inp(0, held, false))]);
            if was && !t.s.foes[n].alive() {
                bounce = t.k(1).body.vy;
                break;
            }
        }
        assert!(bounce < 0, "bounced up after the stomp");
        assert!(!t.s.foes[n].alive(), "the snuffer pops");
        assert!(t.heard("kill"));
        assert!(t.k(1).score >= 100);
        heights.push(bounce);
    }
    assert!(heights[1] < heights[0] - 400, "held {heights:?}");
}

#[test]
fn a_capped_snuffer_leaves_its_cap_to_kick_or_carry() {
    let mut t = Table::new(1);
    let n = snuffer_at(&mut t, 14, FoeKind::Capped);
    t.s.foes[n].vx = 0;
    let (x, y) = cell_feet(marsh(14, 8));
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.body.place(x, y);
        k.body.ground = false;
        k.body.mode = Move::Air;
    }
    for _ in 0..40 {
        t.tick(&[]);
        if !t.s.foes[n].alive() {
            break;
        }
    }
    assert_eq!(t.s.items.len(), 1);
    let cap = &t.s.items[0];
    assert_eq!((cap.kind, cap.cap), (ItemKind::Cap, CapState::Rest));
    // Walked into, a cap flies the way it was run at.
    let (cx, cy) = (cap.x / Q, cap.y / Q);
    t.idle(10);
    t.s.seats.get_mut(&1).unwrap().body.place(cx - 22, cy);
    t.s.foes.clear();
    t.s.foes.push(Foe::new(FoeKind::Snuffer, marsh(17, 13), 902));
    for _ in 0..40 {
        t.tick(&[(1, inp(1, false, false))]);
        if t.s.items[0].cap == CapState::Slide {
            break;
        }
    }
    assert_eq!(t.s.items[0].cap, CapState::Slide, "kicked");
    assert_eq!(t.k(1).pips, START_PIPS, "its kicker is not hurt");
    for _ in 0..60 {
        t.tick(&[]);
    }
    assert!(
        t.s.foes.iter().all(|f| f.mark != 902 || !f.alive()),
        "a flying cap flips what it meets"
    );
    // Carried with fire held, thrown on release.
    let mut t = Table::new(1);
    t.s.foes.clear();
    t.s.items.push(Item::new(ItemKind::Cap, 15 * 16 + 8 + MARSH.ox * 16, (MARSH.oy + 14) * 16));
    t.put(1, marsh(13, 13));
    let fire = Input {
        fire: true,
        ..Default::default()
    };
    let mut held = Input {
        move_x: 1,
        ..fire
    };
    for _ in 0..30 {
        t.tick(&[(1, held)]);
        if t.k(1).carry {
            break;
        }
        held.move_x = 1;
    }
    assert!(t.k(1).carry, "picked up with fire held");
    assert_eq!(t.s.items[0].cap, CapState::Carried);
    t.tick(&[(1, fire)]);
    t.tick(&[]);
    assert_eq!(t.s.items[0].cap, CapState::Slide, "thrown on release");
    assert!(!t.k(1).carry);
}

#[test]
fn a_hit_takes_a_size_before_a_wedge() {
    let mut t = Table::new(1);
    for (size, pips, expect) in [
        (Size::Flame, 6, (Size::Big, 6)),
        (Size::Big, 6, (Size::Small, 6)),
        (Size::Small, 6, (Size::Small, 5)),
    ] {
        t.s.foes.retain(|f| f.kind == FoeKind::Wisp);
        t.put(1, marsh(14, 13));
        let (x, y) = {
            let k = t.s.seats.get_mut(&1).unwrap();
            k.size = size;
            k.body.big = size != Size::Small;
            k.pips = pips;
            k.hurt = 0;
            k.body.foot()
        };
        let mut f = Foe::new(FoeKind::Snuffer, (x / TILE + 1, y / TILE - 1), 903);
        f.x = (x + 8) * Q;
        f.y = y * Q;
        t.s.foes.push(f);
        t.tick(&[]);
        let k = t.k(1);
        assert_eq!((k.size, k.pips), expect, "{size:?}");
        assert!(k.hurt > 0, "a moment of grace");
        assert!(t.heard("knight_hit"));
        t.s.seats.get_mut(&1).unwrap().body.mode = Move::Ground;
    }
}

#[test]
fn a_gate_is_raised_by_whoever_reaches_it_and_is_where_the_party_returns() {
    let mut t = Table::new(2);
    assert_eq!(t.s.checkpoint, 0);
    let gate = info().gates[1];
    t.put(2, (gate.0, gate.1));
    t.tick(&[]);
    assert_eq!(t.s.checkpoint, 2, "the second gate");
    assert!(t.s.gates & 0b10 != 0);
    // A knight arriving late sits down at that gate.
    let mut heroes = t.heroes.clone();
    let mut r = Run::bannerpeak_for_test();
    for id in 2..=3 {
        r.join_seat(id, "Late");
    }
    heroes.insert(3, r.players[&3].clone());
    t.heroes = heroes;
    t.tick(&[]);
    assert!(t.s.seats.contains_key(&3));
    let (x, _) = t.k(3).body.foot();
    assert!((x - cell_feet(gate).0).abs() <= 24, "at the gate: {x}");
    assert!(t.k(3).joined > 0);
    // Never put back earlier by a gate behind.
    t.put(1, info().gates[0]);
    t.tick(&[]);
    assert_eq!(t.s.checkpoint, 2);
}

#[test]
fn the_flagpole_scores_by_height_slides_down_and_raises_the_marsh_banner() {
    let mut scores = Vec::new();
    for row in [12, 5] {
        let mut t = Table::new(1);
        let (px, py) = info().pole;
        let x = px - 1;
        let (fx, fy) = cell_feet((x, MARSH.oy + row));
        {
            let k = t.s.seats.get_mut(&1).unwrap();
            k.body.place(fx, fy);
            k.body.ground = false;
            k.body.mode = Move::Air;
        }
        let mut grabbed = false;
        for _ in 0..30 {
            t.tick(&[(1, inp(1, false, false))]);
            if t.k(1).pole > 0 {
                grabbed = true;
                break;
            }
        }
        assert!(grabbed, "the pole is taken from row {row}");
        assert!(t.s.banners & 4 != 0, "the marsh banner");
        assert!(t.heard("wish_grant"));
        scores.push(t.k(1).score);
        // Slides to the foot, then walks off.
        for _ in 0..200 {
            t.tick(&[]);
            if t.k(1).pole == 0 && t.k(1).poled {
                break;
            }
        }
        assert!(t.k(1).poled && t.k(1).pole == 0, "control returns");
        let (fx2, fy2) = t.k(1).body.foot();
        assert_eq!(fy2, (py + 1) * TILE, "at the foot of the pole");
        assert!(fx2 > px * TILE + 8, "walked off, past it");
        // And is not grabbed twice.
        t.idle(10);
        assert!(t.k(1).free() || t.k(1).pole == 0);
    }
    assert!(scores[1] > scores[0] * 4, "the height you grab scores: {scores:?}");
}

#[test]
fn a_fall_into_the_bog_costs_a_wedge_and_puts_the_knight_on_the_last_safe_ledge() {
    let mut t = Table::new(1);
    t.put(1, marsh(16, 13));
    t.idle(12);
    assert_eq!(t.k(1).safe, cell_feet(marsh(16, 13)));
    // Walk off into the first bog.
    let mut fell_at = None;
    let mut safe = t.k(1).safe;
    for n in 0..80 {
        t.tick(&[(1, inp(1, false, false))]);
        if t.k(1).pit > 0 {
            fell_at = Some(n);
            break;
        }
        safe = t.k(1).safe;
    }
    assert!(fell_at.is_some(), "into the water");
    let (sx, sy) = safe;
    assert!(sx < (MARSH.ox + 18) * TILE && sy == cell_feet(marsh(16, 13)).1, "safe is still dry ground: {safe:?}");
    assert_eq!(t.k(1).pips, START_PIPS - 1);
    let mut back = 0;
    while t.k(1).pit > 0 {
        t.tick(&[]);
        back += 1;
    }
    assert!(back <= 20, "back inside a second: {back} ticks");
    assert_eq!(t.k(1).body.foot(), safe, "at the last safe ledge");
    assert!(t.k(1).hurt > 0, "a moment of grace");
}

#[test]
fn a_cellar_hatch_goes_to_the_bonus_room_and_back() {
    let mut t = Table::new(1);
    let up = MARSH.at(44, 12);
    t.put(1, (up.0, up.1 - 1));
    t.idle(6);
    for _ in 0..3 {
        t.tick(&[(1, inp(0, false, true))]);
    }
    let (x, y) = t.k(1).body.foot();
    assert!(y > (MARSH.oy + 20) * TILE, "down in the room: {x},{y}");
    // Spark in the room; the way out is the other hatch.
    let out = MARSH.at(56, 26);
    t.put(1, (out.0, out.1 - 1));
    t.idle(6);
    for _ in 0..3 {
        t.tick(&[(1, inp(0, false, true))]);
    }
    assert!(t.k(1).body.foot().1 < (MARSH.oy + 14) * TILE, "back up");
}

#[test]
fn a_slam_breaks_the_cracked_floor_and_the_cellar_beneath_holds_a_banner() {
    let mut t = Table::new(1);
    let cracked = HUB.at(23, 18);
    assert_eq!(t.s.tile(cracked.0, cracked.1), Tile::Cracked);
    t.put(1, HUB.at(23, 17));
    t.idle(8);
    // Jump, then slam.
    for n in 0..60 {
        let k = t.k(1);
        let down = n > 16 && k.body.vy > -200;
        t.tick(&[(1, inp(0, n < 20, down))]);
        if t.s.tile(cracked.0, cracked.1) == Tile::Air {
            break;
        }
    }
    assert_eq!(t.s.tile(cracked.0, cracked.1), Tile::Air, "the floor is broken");
    assert!(t.heard("crack_open") && t.heard("rock_land"));
    // Into the cellar.
    t.idle(60);
    assert!(t.k(1).body.foot().1 >= HUB.at(0, 21).1 * TILE, "down below");
}

#[test]
fn a_friend_reaches_a_bubble_and_a_lone_fall_is_a_quick_retry() {
    let mut t = Table::new(2);
    t.put(1, marsh(14, 13));
    t.put(2, marsh(20, 12));
    t.s.seats.get_mut(&1).unwrap().pips = 1;
    // A snuffer that takes the last wedge.
    t.s.foes.retain(|f| f.kind == FoeKind::Wisp);
    let (x, y) = t.k(1).body.foot();
    let mut f = Foe::new(FoeKind::Snuffer, (x / TILE + 1, y / TILE - 1), 904);
    f.x = (x + 8) * Q;
    f.y = y * Q;
    t.s.foes.push(f);
    t.tick(&[]);
    let b = t.k(1).bubble.expect("a bubble");
    assert_eq!(t.k(1).pips, 0);
    let (bx, by) = (b.x / Q, b.y / Q);
    assert!(by < y, "the bubble floats up");
    // The bubble's knight steers it (they are not a spectator): right.
    let before = t.k(1).bubble.unwrap().x;
    t.tick(&[(1, inp(1, false, false))]);
    assert!(t.k(1).bubble.unwrap().x > before);
    // The friend arrives: the knight is back with a few wedges.
    let (bx2, by2) = (t.k(1).bubble.unwrap().x / Q, t.k(1).bubble.unwrap().y / Q);
    t.s.seats.get_mut(&2).unwrap().body.place(bx2, by2 + 12);
    t.tick(&[]);
    assert!(t.k(1).bubble.is_none(), "popped by a friend");
    assert_eq!(t.k(1).pips, BACK_PIPS);
    assert!(t.heard("wish_grant"));
    let _ = (bx, by);
    // Alone: down, and the checkpoint takes the knight back inside a second.
    let mut solo = Table::new(1);
    solo.put(1, marsh(14, 13));
    solo.s.seats.get_mut(&1).unwrap().pips = 1;
    let (x, y) = solo.k(1).body.foot();
    let mut f = Foe::new(FoeKind::Snuffer, (x / TILE + 1, y / TILE - 1), 905);
    f.x = (x + 8) * Q;
    f.y = y * Q;
    solo.s.foes.retain(|f| f.kind == FoeKind::Wisp);
    solo.s.foes.push(f);
    solo.tick(&[]);
    assert!(solo.k(1).bubble.is_some());
    let mut n = 0;
    while solo.k(1).bubble.is_some() {
        solo.tick(&[]);
        n += 1;
        assert!(n < 60);
    }
    assert!(n <= WIPE_TICKS as usize + 2, "back after {n} ticks");
    assert_eq!(solo.k(1).body.foot().1, cell_feet(info().start).1);
    assert!(solo.k(1).pips >= 4);
    // The world kept its progress: the snuffer that struck is still there.
    assert!(solo.s.foes.iter().any(|f| f.kind == FoeKind::Snuffer));
    let _ = BUBBLE_TICKS;
}

#[test]
fn a_bubble_nobody_reaches_floats_a_while_then_the_gate_takes_the_knight_back() {
    let mut t = Table::new(2);
    t.put(1, marsh(14, 13));
    t.put(2, marsh(60, 11));
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.pips = 0;
    }
    let (x, y) = t.k(1).body.foot();
    t.s.seats.get_mut(&1).unwrap().bubble = Some(super::knight::Bubble {
        x: x * Q,
        y: (y - 30) * Q,
        t: 0,
    });
    let mut n = 0;
    while t.k(1).bubble.is_some() && n < 800 {
        t.tick(&[(2, inp(0, false, false))]);
        n += 1;
    }
    assert!(t.k(1).bubble.is_none(), "not a spectator for ever");
    assert!(n <= BUBBLE_TICKS as usize + 2, "{n}");
    assert_eq!(t.k(1).pips, BACK_PIPS);
}

#[test]
fn a_raised_shield_is_a_ledge_for_a_friend_and_springs_them_higher() {
    let mut t = Table::new(2);
    t.put(1, HUB.at(12, 17));
    t.put(2, HUB.at(40, 17));
    // Seat 2 stands with the shield up; seat 1 jumps without help, then with.
    t.idle(8);
    let mut own = 0;
    let y0 = t.k(1).body.foot().1;
    for n in 0..50 {
        t.tick(&[(1, inp(0, n < 30, false))]);
        own = own.max(y0 - t.k(1).body.foot().1);
    }
    t.put(1, HUB.at(40, 12));
    t.s.seats.get_mut(&1).unwrap().body.ground = false;
    t.s.seats.get_mut(&1).unwrap().body.mode = Move::Air;
    let brace = Input {
        interact: true,
        ..Default::default()
    };
    let mut top = i32::MAX;
    let mut stood = 0;
    for n in 0..120 {
        let jump = stood >= 3;
        if t.k(1).body.ground {
            stood += 1;
        }
        t.tick(&[(2, brace), (1, inp(0, jump, false))]);
        top = top.min(t.k(1).body.foot().1);
        let _ = n;
    }
    let floor = cell_feet(HUB.at(40, 17)).1;
    let rise = floor - top;
    assert!(rise > own + 40, "a spring off a friend's shield: {rise} against {own}");
    assert!(t.heard("shield_up"));
}

#[test]
fn two_banners_open_the_courtyard_door_and_seven_open_the_summit() {
    let mut t = Table::new(1);
    let door = HUB.at(76, 15);
    assert_eq!(t.s.tile(door.0, door.1), Tile::Gate(0));
    let mut said = Vec::new();
    t.s.award(0, None, &mut said);
    t.idle(2);
    assert_eq!(t.s.tile(door.0, door.1), Tile::Gate(0), "one is not enough");
    t.s.award(1, None, &mut said);
    t.idle(2);
    assert_eq!(t.s.tile(door.0, door.1), Tile::Air, "the tapestry rises");
    let summit = SNOW.at(100, 8);
    assert_eq!(t.s.tile(summit.0, summit.1), Tile::Gate(1));
    for b in 2..=5 {
        t.s.award(b, None, &mut said);
    }
    t.idle(2);
    assert_eq!(t.s.tile(summit.0, summit.1), Tile::Gate(1), "six is not seven");
    t.s.award(6, None, &mut said);
    t.idle(2);
    assert_eq!(t.s.tile(summit.0, summit.1), Tile::Air);
    assert_eq!(t.s.door_count(), 7);
}

#[test]
fn the_ring_of_eight_sparks_is_a_banner_of_its_own() {
    let mut t = Table::new(1);
    let sparks = info().sparks.clone();
    for (n, &c) in sparks.iter().enumerate() {
        assert_eq!(t.s.banners & (1 << 6), 0, "not before the eighth");
        t.put(1, (c.0, c.1 + 1));
        t.s.seats.get_mut(&1).unwrap().body.y = (c.1 + 1) * TILE * Q;
        t.tick(&[]);
        assert!(t.s.sparks & (1 << n) != 0, "spark {n}");
        t.idle(60);
    }
    assert!(t.s.banners & (1 << 6) != 0, "the eighth raises the banner");
    assert_eq!(t.s.announce.map(|a| a.banner), Some(6));
}

#[test]
fn the_course_board_starts_the_party_where_the_choice_says() {
    let mut t = Table::new(3);
    t.s.award(0, None, &mut Vec::new());
    t.s.award(1, None, &mut Vec::new());
    t.idle(2);
    // E at the courtyard door opens the board for everyone.
    let arch = info().arch;
    t.put(1, arch);
    t.tick(&[(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    assert_eq!(t.s.phase, Phase::Select);
    assert_eq!(t.s.cursor, 0, "the first banner not raised: the marsh flag");
    // Another seat moves the cursor down; the third takes the choice.
    t.tick(&[(2, inp(0, false, true))]);
    assert_eq!(t.s.cursor, 1);
    t.tick(&[]);
    t.tick(&[(
        3,
        Input {
            interact: true,
            ..Default::default()
        },
    )]);
    assert_eq!(t.s.phase, Phase::Play);
    let mill = info().gates[2];
    for id in 1..=3 {
        let (x, y) = t.k(id).body.foot();
        assert!((x - cell_feet(mill).0).abs() <= 30 && y == cell_feet(mill).1, "seat {id}");
    }
    assert_eq!(t.s.checkpoint, 3);
    // The summit line is locked until its door opens.
    t.s.phase = Phase::Select;
    t.s.cursor = 5;
    t.s.phase_t = super::session::SELECT_TICKS - 1;
    t.tick(&[]);
    assert_eq!(t.s.phase, Phase::Select, "the summit door is shut");
    // A board nobody answers takes the cursor's line after its time.
    t.s.cursor = 0;
    t.s.phase_t = super::session::SELECT_TICKS - 1;
    t.tick(&[]);
    assert_eq!(t.s.phase, Phase::Play);
}

#[test]
fn more_seats_bring_more_foes_and_never_fewer() {
    let counts: Vec<usize> = [1u32, 2, 3, 5, 6]
        .iter()
        .map(|&n| Table::new(n).s.foes.len())
        .collect();
    assert_eq!(counts[0], counts[1], "two is the same table of foes");
    assert!(counts[2] > counts[1], "three adds some: {counts:?}");
    assert!(counts[3] > counts[2], "five adds more: {counts:?}");
    assert_eq!(counts[3], counts[4]);
    // A knight who joins mid-run scales the world once; leaving does not undo it.
    let mut t = Table::new(1);
    let base = t.s.foes.len();
    let mut r = Run::bannerpeak_for_test();
    for id in 2..=5 {
        r.join_seat(id, "Late");
    }
    t.heroes = r.players.clone();
    t.tick(&[]);
    assert_eq!(t.s.seats.len(), 5);
    let grown = t.s.foes.len();
    assert!(grown > base);
    t.heroes.retain(|&id, _| id == 1);
    t.tick(&[]);
    assert_eq!(t.s.foes.len(), grown, "never weaker");
    t.heroes = r.players.clone();
    t.tick(&[]);
    assert_eq!(t.s.foes.len(), grown, "rejoining a provisioned table adds nothing");
}

#[test]
fn the_camera_leads_the_way_and_never_loses_the_knight() {
    let mut t = Table::new(1);
    t.put(1, marsh(2, 13));
    t.idle(10);
    let mut worst = (i32::MAX, i32::MIN);
    for _ in 0..80 {
        t.tick(&[(1, inp(1, false, false))]);
        let k = t.k(1);
        let sx = k.body.foot().0 - k.cam.0;
        worst = (worst.0.min(sx), worst.1.max(sx));
    }
    let k = t.k(1);
    let sx = k.body.foot().0 - k.cam.0;
    assert!(sx < 192 - 8, "running right, the view leads ahead: knight at {sx}");
    assert!(worst.0 >= 90 && worst.1 <= 300, "never near the edge: {worst:?}");
    // Running left, it swings the other way.
    for _ in 0..80 {
        t.tick(&[(1, inp(-1, false, false))]);
    }
    let k = t.k(1);
    assert!(k.body.foot().0 - k.cam.0 > 192 + 8, "leads left");
    // A long fall pulls the view down after the knight.
    let (x, y) = cell_feet(HUB.at(30, 3));
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.body.place(x, y);
        k.body.ground = false;
        k.body.mode = Move::Air;
        k.cam = super::knight::follow(k.cam, x, y, 1, 0, 0);
    }
    let cam0 = t.k(1).cam.1;
    for _ in 0..20 {
        t.tick(&[]);
    }
    assert!(t.k(1).cam.1 > cam0, "following the fall");
    let k = t.k(1);
    assert!(k.body.foot().1 - k.cam.1 < 224 - 20, "the knight is still on screen");
}

#[test]
fn the_world_is_the_party_s_and_a_retry_keeps_its_progress() {
    let mut t = Table::new(2);
    t.s.award(0, None, &mut Vec::new());
    t.s.taken.insert(0);
    let spent = marsh(10, 9);
    t.s.changed.insert((spent.1 * layout::WIDTH + spent.0) as u32, Tile::Spent.code());
    for id in [1, 2] {
        t.s.seats.get_mut(&id).unwrap().pips = 0;
        let (x, y) = t.k(id).body.foot();
        t.s.seats.get_mut(&id).unwrap().bubble = Some(super::knight::Bubble {
            x: x * Q,
            y: (y - 20) * Q,
            t: 0,
        });
    }
    for _ in 0..WIPE_TICKS + 2 {
        t.tick(&[]);
    }
    assert!(t.s.seats.values().all(|k| k.bubble.is_none() && k.pips >= 4));
    assert!(t.s.banners & 1 != 0 && t.s.taken.contains(&0));
    assert_eq!(t.s.tile(spent.0, spent.1), Tile::Spent);
}




/// Bots at the table: every seat's keys from `bot_input`.
fn bots(t: &mut Table, ticks: usize) {
    for _ in 0..ticks {
        let keys: Vec<(u32, Input)> = t
            .s
            .seats
            .keys()
            .map(|&id| (id, t.s.bot_input(id, &t.heroes[&id])))
            .collect();
        t.tick(&keys);
    }
}

/// A table of `seats` bots at the start of a stage: the banners and sparks
/// earned before it, standing at its gate.
fn stage(seats: u32, banners: u8, sparks: u8, gate: usize) -> Table {
    let mut t = Table::new(seats);
    t.s.banners = banners;
    t.s.sparks = sparks;
    t.s.checkpoint = gate as u8 + 1;
    t.s.gates = (1 << (gate + 1)) - 1;
    t.tick(&[]);
    for id in 1..=seats {
        let (cx, cy) = info().gates[gate];
        t.put(id, (cx + (id as i32 - 1), cy));
    }
    t.s.announce = None;
    t
}

/// Bots at the table until `done`, at most `limit` ticks: the ticks taken.
fn until(t: &mut Table, limit: usize, done: impl Fn(&Session) -> bool) -> usize {
    for n in 0..limit {
        if done(&t.s) {
            return n;
        }
        bots(t, 1);
        if n % 97 == 0 {
            assert!(t.s.valid(), "a valid session at tick {n}");
        }
    }
    panic!(
        "not done in {limit} ticks: banners {:#b} sparks {:#b} at {:?}",
        t.s.banners,
        t.s.sparks,
        t.s.seats.values().map(|k| k.body.foot()).collect::<Vec<_>>()
    );
}

#[test]
fn the_bot_wins_both_courtyard_banners_and_goes_through_the_door() {
    for seats in [1, 4] {
        let mut t = Table::new(seats);
        let ticks = until(&mut t, 2400, |s| {
            s.banners & 3 == 3 && s.seats.values().all(|k| k.body.foot().0 > MARSH.ox * TILE + 100)
        });
        println!("courtyard: {seats} seats, {ticks} ticks");
        assert_eq!(t.s.doors & 1, 1, "the tapestry rose");
        assert_eq!(t.s.phase, Phase::Play);
        assert!(t.s.seen, "the board came up for the way in");
    }
}

#[test]
fn the_bot_runs_the_marsh_takes_its_sparks_and_the_flagpole() {
    let mut t = stage(1, 0b11, 0, 0);
    let ticks = until(&mut t, 2600, |s| s.banners & 4 != 0 && s.sparks & 0b11 == 0b11);
    println!("marsh: {ticks} ticks");
    assert!(ticks > 600, "not a walk: {ticks}");
}

#[test]
fn the_bot_crosses_the_mill_by_the_wheel_and_flips_up_for_a_spark() {
    let mut t = stage(1, 0b111, 0b11, 2);
    let ticks = until(&mut t, 4800, |s| s.banners & 8 != 0 && s.sparks & 0b1111 == 0b1111);
    println!("mill: {ticks} ticks");
}

#[test]
fn the_bot_climbs_the_bell_shaft_by_wall_kicks() {
    let mut t = stage(1, 0b1111, 0b1111, 3);
    let ticks = until(&mut t, 3000, |s| s.banners & 16 != 0 && s.sparks & 0b11_1111 == 0b11_1111);
    println!("shaft: {ticks} ticks");
}

#[test]
fn the_bot_runs_the_snow_ledge_under_the_boulders() {
    let mut t = stage(1, 0b11111, 0b11_1111, 5);
    let ticks = until(&mut t, 4000, |s| s.banners & 32 != 0 && s.sparks == 0xFF && s.doors & 2 != 0);
    println!("snow: {ticks} ticks");
    assert!(t.s.boulders.len() <= 4);
}

#[test]
fn the_bot_beats_king_cinder_at_any_table() {
    for seats in [1, 2, 6] {
        let mut t = stage(seats, 0x7F, 0xFF, 7);
        t.s.doors = 3;
        let ticks = until(&mut t, 4000, |s| s.banners == 0xFF);
        println!("cinder: {seats} seats, {ticks} ticks");
        assert!(t.s.boss.defeated());
        assert_eq!(t.s.boss.plates, 0);
    }
}

#[test]
fn a_party_that_missed_a_ring_spark_goes_back_for_it_by_the_board() {
    for seats in [1, 3] {
        // Snow behind them, the marsh's spark over the bog never taken, the
        // summit door shut: back to the last gate, E, the board, the marsh.
        let mut t = stage(seats, 0b11_1111, 0xFF & !0b10, 6);
        assert_eq!(t.s.doors & 2, 0);
        let ticks = until(&mut t, 3000, |s| s.doors & 2 != 0);
        println!("missed spark: {seats} seats, {ticks} ticks");
        assert_eq!(t.s.sparks, 0xFF);
        assert!(t.s.banners & 64 != 0, "the ring banner flew");
    }
}

/// The whole adventure, every knight a bot, from the door to the finale.
fn whole(seats: u32) -> (usize, Table) {
    let mut t = Table::new(seats);
    let ticks = until(&mut t, 12000, |s| s.completed.is_some());
    (ticks, t)
}

fn whole_checks(seats: u32, limit: usize) {
    let (ticks, t) = whole(seats);
    println!("whole adventure: {seats} seats, {ticks} ticks ({} s)", ticks / 30);
    assert!(ticks < limit, "{ticks} ticks");
    assert!(ticks > 2400, "a real adventure, not a dash: {ticks} ticks");
    assert_eq!((t.s.banners, t.s.sparks), (0xFF, 0xFF));
    assert!(t.s.boss.defeated() && t.s.phase == Phase::Finale);
    assert_eq!(t.s.seats.len(), seats as usize);
    // The finale is at the Great Pole, every knight home.
    let pole = layout::cell_feet(info().great_pole).0;
    for k in t.s.seats.values() {
        assert!((k.body.foot().0 - pole).abs() < 120, "at the pole");
    }
    // What it saves and reads back.
    let back: Session = serde_json::from_str(&serde_json::to_string(&t.s).unwrap()).unwrap();
    assert_eq!(back, t.s);
}

#[test]
fn a_scripted_knight_clears_the_adventure_alone() {
    whole_checks(1, 9000);
}
#[test]
fn a_scripted_pair_clears_the_adventure() {
    whole_checks(2, 9000);
}
#[test]
fn a_scripted_three_and_five_clear_the_adventure() {
    whole_checks(3, 9000);
    whole_checks(5, 9000);
}
#[test]
fn a_scripted_four_clear_the_adventure() {
    whole_checks(4, 9000);
}
#[test]
fn a_scripted_six_clear_the_adventure() {
    whole_checks(6, 9000);
}

#[test]
fn a_raised_shield_gets_a_friend_up_to_the_tower_roof() {
    // The roof is seven tiles up: past a plain jump, a run jump and nearly a
    // triple. A friend with the shield up is a step, and a spring.
    let roof_edge = (HUB.ox + 56) * TILE;
    let roof_y = (HUB.oy + 11) * TILE;
    let mut t = Table::new(2);
    t.put(2, HUB.at(54, 17));
    t.put(1, HUB.at(40, 17));
    let brace = Input {
        interact: true,
        ..Default::default()
    };
    for _ in 0..8 {
        t.tick(&[(2, brace)]);
    }
    assert_eq!(t.k(2).body.mode, Move::Brace);
    // The other knight comes down on the raised shield, then springs off it
    // toward the roof.
    let (sx, _) = t.k(2).body.foot();
    {
        let k = t.s.seats.get_mut(&1).unwrap();
        k.body.place(sx, 700);
        k.body.ground = false;
        k.body.mode = Move::Air;
    }
    let mut landed = false;
    let mut on_roof = false;
    let mut standing = 0;
    for _ in 0..100 {
        landed |= t.k(1).body.plank.is_some();
        if t.k(1).body.ground && t.k(1).body.plank.is_some() {
            standing += 1;
        }
        let jump = standing >= 3;
        let dir = if t.k(1).body.vy < 0 || !t.k(1).body.ground && standing >= 3 { 1 } else { 0 };
        t.tick(&[(2, brace), (1, inp(dir, jump && !on_roof, false))]);
        let f = t.k(1).body.foot();
        on_roof |= t.k(1).body.ground && f.1 == roof_y && f.0 >= roof_edge;
        if on_roof {
            break;
        }
    }
    assert!(landed, "stood on the shield");
    assert!(on_roof, "sprang up onto the roof: {:?}", t.k(1).body.foot());
}

#[test]
fn the_bell_boulder_comes_quicker_and_in_twos_for_a_fuller_table() {
    // (ticks between boulders, pixels they roll in a tick, most at once)
    let mut seen = Vec::new();
    for seats in [1u32, 2, 4, 6] {
        let mut t = Table::new(seats);
        t.s.foes.clear();
        for id in 1..=seats {
            t.put(id, SNOW.at(8 + id as i32, 9));
        }
        let mut spawns = Vec::new();
        let mut speed = 0;
        let mut most = 0;
        let mut was = 0usize;
        for n in 0..1400 {
            // The knights stand and are not hurt: the clock is what counts.
            for k in t.s.seats.values_mut() {
                k.hurt = 70;
            }
            let before = t.s.boulders.first().map(|b| b.x);
            t.tick(&[]);
            if t.s.boulders.len() > was {
                spawns.push(n);
            }
            was = t.s.boulders.len();
            most = most.max(was);
            if let (Some(a), Some(b)) = (before, t.s.boulders.first().map(|b| b.x)) {
                if a > b {
                    speed = (a - b) / Q;
                }
            }
        }
        let gap = spawns.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
        println!("boulders at {seats} seats: spawns {spawns:?}, {speed} px a tick? gap {gap}, most {most}");
        seen.push((seats, spawns.len(), speed, most));
    }
    assert!(seen[0].1 >= 3, "they roll: {seen:?}");
    assert!(seen[3].1 > seen[0].1, "more of them for six: {seen:?}");
    assert!(seen[3].2 >= seen[0].2, "never slower: {seen:?}");
    assert!(seen[3].3 >= seen[0].3, "{seen:?}");
}

#[test]
fn a_bell_boulder_hurts_from_the_side_and_is_a_springboard_from_above() {
    let mut t = Table::new(1);
    t.s.foes.clear();
    t.put(1, SNOW.at(40, 9));
    let at = (SNOW.ox + 41) * TILE * Q;
    t.s.boulders.push(super::foes::Boulder { x: at, t: 0 });
    for _ in 0..10 {
        t.tick(&[]);
        if t.k(1).hurt > 0 {
            break;
        }
    }
    assert!(t.k(1).hurt > 0 && t.k(1).pips == START_PIPS - 1, "knocked");
    // From above: a bounce, no wedge.
    let mut t = Table::new(1);
    t.s.foes.clear();
    t.s.boulders.push(super::foes::Boulder {
        x: ((SNOW.ox + 40) * TILE + 18) * Q,
        t: 0,
    });
    {
        // A hand above the boulder's top, falling onto it.
        let (x, _) = cell_feet(SNOW.at(40, 4));
        let k = t.s.seats.get_mut(&1).unwrap();
        k.body.place(x, layout::SNOW_FLOOR * TILE - 30);
        k.body.ground = false;
        k.body.mode = Move::Air;
    }
    let mut bounced = false;
    for _ in 0..30 {
        t.tick(&[]);
        bounced |= t.k(1).body.vy < -1000;
    }
    assert!(bounced && t.k(1).pips == START_PIPS, "sprang off the top");
}

// ---------------------------------------------------------------------
// The tuning sweeps (`cargo test -p angel-delve --lib write_bannerpeak_tuning
// -- --ignored --nocapture`): each knob across its F2 range with the others at
// their shipped values, the scripted party playing the whole adventure.

fn sweep_whole(seats: u32, knobs: Knobs) -> Option<usize> {
    let mut t = Table::new(seats);
    t.knobs = knobs;
    for n in 0..9000 {
        if t.s.completed.is_some() {
            return Some(n);
        }
        bots(&mut t, 1);
    }
    None
}

#[test]
#[ignore = "prints the tuning table's bot rows"]
fn write_bannerpeak_tuning() {
    let show = |name: &str, value: i64, knobs: Knobs| {
        let cell = |seats: u32| match sweep_whole(seats, knobs) {
            Some(n) => format!("{:>5}", n),
            None => " fail".to_string(),
        };
        println!(
            "SWEEP {name:<6} {value:>5}  1 seat {}  4 seats {}",
            cell(1),
            cell(4)
        );
    };
    for v in [640, 896, 960, 1152, 1408, 1664, 2048] {
        show("run", i64::from(v), Knobs { run: v, ..KNOBS });
    }
    for v in [0, 4, 10, 20, 30] {
        show("hop", i64::from(v), Knobs { hop: v, ..KNOBS });
    }
    for v in [2, 3, 5, 7, 9] {
        show("grace", i64::from(v), Knobs { grace: v, ..KNOBS });
    }
    for v in [16, 24, 36, 64, 96] {
        show("slam", i64::from(v), Knobs { slam: v, ..KNOBS });
    }
    for v in [1200, 2000, 3000, 4000] {
        show("ledge", i64::from(v), Knobs { ledge: v, ..KNOBS });
    }
}

/// How many of a ring of snuffers a slam beats, by how far each stood from
/// where the knight landed: the radius knob's reach.
#[test]
#[ignore = "prints the tuning table's slam rows"]
fn write_bannerpeak_slam_reach() {
    for radius in [16, 24, 36, 64, 96] {
        let mut beaten = Vec::new();
        for dist in [10, 20, 30, 40, 60, 90] {
            let mut t = Table::new(1);
            t.knobs = Knobs {
                slam: radius,
                ..KNOBS
            };
            t.put(1, marsh(14, 13));
            let (fx, fy) = cell_feet(marsh(14, 13));
            let n = snuffer_at(&mut t, 14, FoeKind::Snuffer);
            t.s.foes[n].x = (fx + dist) * Q;
            t.s.foes[n].y = fy * Q;
            t.s.foes[n].vx = 0;
            for _ in 0..4 {
                t.s.foes[n].x = (fx + dist) * Q;
                t.s.foes[n].y = fy * Q;
                t.tick(&[(1, inp(0, true, false))]);
            }
            let mut hit = false;
            for _ in 0..70 {
                t.s.foes[n].x = (fx + dist) * Q;
                t.s.foes[n].y = fy * Q;
                t.tick(&[(1, inp(0, true, true))]);
                if !t.s.foes[n].alive() {
                    hit = true;
                    break;
                }
            }
            beaten.push((dist, hit));
        }
        println!("SLAM radius {radius:>3}: beaten at {beaten:?}");
    }
}

/// The shield a friend stands on is held as long as the friend needs it. The
/// stance's own clock must never count the session out: a host that stops
/// being valid is one every other screen refuses to draw.
#[test]
fn a_shield_held_past_the_old_clock_keeps_the_session_valid() {
    let mut t = Table::new(2);
    t.idle(8);
    let brace = Input {
        interact: true,
        ..Default::default()
    };
    for n in 0..700 {
        t.tick(&[(2, brace)]);
        assert!(t.s.valid(), "a valid session at tick {n} of the hold");
    }
    assert_eq!(t.k(2).body.mode, Move::Brace, "the hold really was a hold");
}

/// A table left at the Great Pole after the finale, for as long as it likes.
#[test]
fn a_table_idling_at_the_pole_past_the_limit_stays_valid() {
    let mut t = Table::new(2);
    t.idle(8);
    t.s.banners = 0xFF;
    t.s.phase = Phase::Finale;
    t.s.phase_t = LIMIT - 2;
    assert!(t.s.valid(), "the finale as it stands is valid");
    for n in 0..6 {
        t.idle(1);
        assert!(t.s.valid(), "a valid session {n} ticks past the limit");
    }
}

/// A velocity no sim reaches is a velocity `valid` refuses, the smallest
/// number included: its absolute value does not fit, and a check built on
/// `abs()` would let it through in a release build.
#[test]
fn the_smallest_velocity_is_not_a_valid_one() {
    let mut t = Table::new(1);
    assert!(t.s.valid());
    t.s.seats.get_mut(&1).unwrap().body.vx = i32::MIN;
    assert!(!t.s.valid());
}
