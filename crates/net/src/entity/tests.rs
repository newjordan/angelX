use super::*;
use crate::doc::value::Value;
use std::borrow::Cow;

/// A small deterministic random walk for the tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u64) as i32
    }
}

fn state(x: i32, y: i32) -> [u8; 8] {
    let mut s = [0u8; 8];
    s[..4].copy_from_slice(&x.to_le_bytes());
    s[4..].copy_from_slice(&y.to_le_bytes());
    s
}

fn globals(n: u64) -> Value {
    Value::Struct(vec![(Cow::Borrowed("count"), Value::U(n)), (Cow::Borrowed("phase"), Value::Str("muster".into()))])
}

fn view(x: i32, y: i32) -> Interest {
    Interest { x, y, half_w: 200, half_h: 120, keep: 40, own: None }
}

/// A table of `n` things scattered over a square `side` across.
fn scattered(n: u32, side: i32, rng: &mut Rng) -> Table {
    let mut table = Table::new(64);
    table.begin(1);
    for id in 0..n {
        let (x, y) = (rng.range(0, side), rng.range(0, side));
        table.spawn(id, (id % 3) as u16, x, y, format!("thing {id}").as_bytes(), &state(x, y));
    }
    table
}

fn walk(table: &mut Table, tick: u32, rng: &mut Rng, step: i32) {
    table.begin(tick);
    let moves: Vec<(u32, i32, i32)> = table
        .iter()
        .map(|e| (e.id, e.x + rng.range(-step, step), e.y + rng.range(-step, step)))
        .collect();
    for (id, x, y) in moves {
        table.update(id, x, y, &state(x, y));
    }
}

/// Write one snapshot of the table and apply it on the client.
fn send(table: &Table, viewer: &mut Viewer, mirror: &mut Mirror, interest: &Interest, budget: usize) -> Written {
    let world = table.publish();
    let mut body = Vec::new();
    let written = viewer.write_within(&world, &globals(table.len() as u64), interest, budget, &mut body);
    assert_eq!(written.bytes, body.len());
    mirror.apply(world.tick(), written.whole, &body).expect("the client reads what the server wrote");
    written
}

/// The client holds exactly what the server's viewer says it holds, each
/// with the server's spawn bytes and newest state.
fn assert_mirror_matches(table: &Table, viewer: &Viewer, mirror: &Mirror) {
    let mut held: Vec<u32> = viewer.held().collect();
    held.sort_unstable();
    let live: Vec<u32> = mirror.live().map(|(id, _)| id).collect();
    assert_eq!(live, held);
    for (id, m) in mirror.live() {
        let e = table.get(id).expect("the client holds nothing the server does not have");
        assert_eq!(m.kind, e.kind);
        assert_eq!(m.spawn, &e.spawn[..]);
        assert_eq!(m.latest(), e.state.as_slice(), "thing {id}");
    }
}

/// Everything in view is held, and nothing held is past view plus keep.
fn assert_interest_held(table: &Table, interest: &Interest, mirror: &Mirror) {
    for e in table.iter() {
        if interest.sees(e.x, e.y) {
            assert!(mirror.get(e.id).is_some_and(|m| m.gone().is_none()), "thing {} is in view", e.id);
        }
    }
    for (id, _) in mirror.live() {
        let e = table.get(id).unwrap();
        let dx = (e.x - interest.x).abs();
        let dy = (e.y - interest.y).abs();
        assert!(dx <= interest.half_w + interest.keep && dy <= interest.half_h + interest.keep, "thing {id} is out of reach");
    }
}

#[test]
fn the_client_mirror_equals_the_server_within_interest() {
    let mut rng = Rng(7);
    let mut table = scattered(120, 1500, &mut rng);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    let mut eye = (700, 700);
    for tick in 1..200 {
        if tick > 1 {
            walk(&mut table, tick, &mut rng, 6);
        }
        eye = (eye.0 + 3, eye.1 + 1);
        let interest = view(eye.0, eye.1);
        let written = send(&table, &mut viewer, &mut mirror, &interest, 1 << 20);
        assert!(!written.partial, "an unlimited budget leaves nothing behind");
        assert_eq!(written.whole, tick == 1);
        assert_mirror_matches(&table, &viewer, &mirror);
        assert_interest_held(&table, &interest, &mirror);
    }
    assert_eq!(mirror.globals(), &globals(120));
}

#[test]
fn a_crowd_never_takes_more_than_the_budget_and_arrives_once_it_settles() {
    let mut rng = Rng(11);
    // 300 things inside one view: far more than one snapshot carries.
    let mut table = scattered(300, 200, &mut rng);
    let interest = Interest { own: Some(17), ..view(100, 100) };
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    let mut partial = 0;
    for tick in 1..120 {
        if tick > 1 {
            walk(&mut table, tick, &mut rng, 3);
        }
        let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
        assert!(w.bytes <= BUDGET, "{} bytes", w.bytes);
        partial += usize::from(w.partial);
        // The client's own knight is current in every snapshot.
        assert_eq!(mirror.get(17).unwrap().latest(), table.get(17).unwrap().state.as_slice());
    }
    assert!(partial > 100, "the crowd overflowed the budget");
    // Once nobody moves, what was left behind arrives.
    for tick in 120..140 {
        table.begin(tick);
        let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
        assert!(w.bytes <= BUDGET);
    }
    assert_mirror_matches(&table, &viewer, &mirror);
    assert_interest_held(&table, &interest, &mirror);
    assert_eq!(mirror.live().count(), 300);
}

#[test]
fn far_things_are_not_starved_by_near_ones() {
    let mut rng = Rng(5);
    let mut table = Table::new(64);
    table.begin(1);
    // 200 near the middle, moving every tick; one at the edge, moving too.
    for id in 0..200 {
        let (x, y) = (rng.range(-20, 20), rng.range(-20, 20));
        table.spawn(id, 0, x, y, b"", &state(x, y));
    }
    table.spawn(999, 0, 190, 0, b"", &state(190, 0));
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    // (tick it was last updated, the longest gap between updates)
    let mut last_far: Option<u32> = None;
    let mut longest = 0;
    for tick in 1..200 {
        if tick > 1 {
            walk(&mut table, tick, &mut rng, 2);
            let x = 185 + (tick as i32 % 10);
            table.update(999, x, 0, &state(x, 0));
        }
        send(&table, &mut viewer, &mut mirror, &interest, 400);
        if let Some(m) = mirror.get(999) {
            let at = m.latest_tick();
            if let Some(was) = last_far {
                longest = longest.max(at - was);
            }
            last_far = Some(at);
        }
    }
    // The near ones are each sent about every fifth tick; the far one, at a
    // quarter of their weight, about every twentieth, but never starved.
    assert!(last_far.is_some_and(|t| t > 170), "the far thing was last sent on {last_far:?}");
    assert!(longest <= 30, "the far thing waited {longest} ticks");
}

#[test]
fn a_thing_that_leaves_interest_is_despawned_and_one_at_the_edge_is_kept() {
    let mut table = Table::new(64);
    table.begin(1);
    table.spawn(1, 0, 0, 0, b"a", &state(0, 0));
    table.spawn(2, 0, 100, 0, b"b", &state(100, 0));
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(mirror.live().count(), 2);

    // Just past the view, inside the keep: still held.
    table.begin(2);
    table.update(2, 230, 0, &state(230, 0));
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert!(viewer.holds(2));
    assert_eq!(mirror.get(2).unwrap().latest(), &state(230, 0));

    // Past the keep: let go.
    table.begin(3);
    table.update(2, 260, 0, &state(260, 0));
    let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(w.despawns, 1);
    assert!(!viewer.holds(2));
    assert_eq!(mirror.get(2).unwrap().gone(), Some(3));
    assert_eq!(mirror.live().count(), 1);

    // Back past the edge of the keep but not in view: not sent.
    table.begin(4);
    table.update(2, 220, 0, &state(220, 0));
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert!(!viewer.holds(2));

    // Drawing passes the despawn: the client lets it go.
    mirror.prune(2.5);
    assert!(mirror.get(2).is_some());
    mirror.prune(3.0);
    assert!(mirror.get(2).is_none());

    // In view again: sent whole.
    table.begin(5);
    table.update(2, 150, 0, &state(150, 0));
    let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(w.spawns, 1);
    assert_eq!(mirror.get(2).unwrap().spawn, b"b");

    // Gone from the world: despawned.
    table.begin(6);
    table.remove(1);
    let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(w.despawns, 1);
    assert_mirror_matches(&table, &viewer, &mirror);
}

#[test]
fn a_client_that_skipped_ticks_converges() {
    let mut rng = Rng(23);
    let mut table = scattered(80, 600, &mut rng);
    let interest = view(300, 300);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    for tick in 1..100 {
        if tick > 1 {
            walk(&mut table, tick, &mut rng, 5);
        }
        // A slow client: its writer only gets to every seventh tick.
        if tick % 7 == 1 {
            let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
            assert!(w.bytes <= BUDGET);
        }
    }
    for tick in 100..104 {
        table.begin(tick);
        send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    }
    assert_mirror_matches(&table, &viewer, &mirror);
    assert_interest_held(&table, &interest, &mirror);
}

#[test]
fn unchanged_things_are_not_sent_again() {
    let mut rng = Rng(3);
    let mut table = scattered(50, 230, &mut rng);
    let interest = view(115, 115);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    let first = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(first.spawns, 50);
    table.begin(2);
    // The same bytes again: no new version.
    let e = table.get(4).unwrap().clone();
    table.update(4, e.x, e.y, e.state.as_slice());
    let quiet = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!((quiet.spawns, quiet.updates, quiet.despawns), (0, 0, 0));
    // Flags, globals unchanged, three empty lists.
    assert_eq!(quiet.bytes, 5);
    table.begin(3);
    table.update(4, e.x + 1, e.y, &state(e.x + 1, e.y));
    let one = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!((one.spawns, one.updates), (0, 1));
}

#[test]
fn new_spawn_bytes_send_the_thing_whole_again() {
    let mut table = Table::new(64);
    table.begin(1);
    table.spawn(5, 1, 0, 0, b"Gwen", &state(0, 0));
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    table.begin(2);
    table.spawn(5, 1, 0, 0, b"Gwen", &state(0, 0));
    assert_eq!(send(&table, &mut viewer, &mut mirror, &interest, BUDGET).spawns, 0);
    table.begin(3);
    table.spawn(5, 2, 0, 0, b"Gwen the Bold", &state(0, 0));
    assert_eq!(send(&table, &mut viewer, &mut mirror, &interest, BUDGET).spawns, 1);
    let m = mirror.get(5).unwrap();
    assert_eq!((m.kind, &m.spawn[..]), (2, &b"Gwen the Bold"[..]));
}

#[test]
fn the_globals_travel_as_patches() {
    let mut table = Table::new(64);
    table.begin(1);
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    let world = table.publish();
    let mut body = Vec::new();
    let a = Value::Struct(vec![(Cow::Borrowed("score"), Value::U(1)), (Cow::Borrowed("phase"), Value::Str("lobby".into()))]);
    let w = viewer.write(&world, &a, &interest, &mut body);
    mirror.apply(1, w.whole, &body).unwrap();
    assert_eq!(mirror.globals(), &a);

    let b = Value::Struct(vec![(Cow::Borrowed("score"), Value::U(2)), (Cow::Borrowed("phase"), Value::Str("lobby".into()))]);
    body.clear();
    let w = viewer.write(&world, &b, &interest, &mut body);
    assert!(!w.whole);
    mirror.apply(2, false, &body).unwrap();
    assert_eq!(mirror.globals(), &b);
}

#[test]
fn a_thing_that_stood_still_is_drawn_standing_until_it_moved() {
    let mut table = Table::new(64);
    table.begin(1);
    table.spawn(1, 0, 0, 0, b"", &state(0, 0));
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    for tick in 2..=10 {
        table.begin(tick);
        send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    }
    table.begin(11);
    table.update(1, 10, 0, &state(10, 0));
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    let m = mirror.get(1).unwrap();
    // Between ticks 1 and 10 it stood at 0.
    let (a, b, _) = m.at(9.5);
    assert_eq!((a, b), (&state(0, 0)[..], &state(0, 0)[..]));
    // Between 10 and 11 it walked.
    let (a, b, alpha) = m.at(10.5);
    assert_eq!((a, b), (&state(0, 0)[..], &state(10, 0)[..]));
    assert!((alpha - 0.5).abs() < 1e-6);
    assert_eq!(m.at(12.0).0, &state(10, 0));
}

#[test]
fn a_deferred_snapshot_does_not_claim_the_unmentioned_are_unchanged() {
    let mut table = Table::new(64);
    table.begin(1);
    for id in 0..100 {
        table.spawn(id, 0, (id as i32 % 10) * 10, (id as i32 / 10) * 10, b"", &state(0, 0));
    }
    let interest = view(50, 50);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    send(&table, &mut viewer, &mut mirror, &interest, 1 << 20);
    // Everyone moves; the budget only takes some.
    table.begin(2);
    for id in 0..100 {
        let e = table.get(id).unwrap().clone();
        table.update(id, e.x, e.y, &state(1, 1));
    }
    let world = table.publish();
    let mut body = Vec::new();
    let w = viewer.write_within(&world, &globals(100), &interest, 300, &mut body);
    assert!(w.partial && w.updates < 100);
    assert_eq!(body[0] & DEFERRED, DEFERRED);
    mirror.apply(2, false, &body).unwrap();
    let left_behind = (0..100).find(|id| mirror.get(*id).unwrap().latest() == state(0, 0)).unwrap();
    // It moved somewhere between 1 and 4: the client must not draw it
    // standing until 2 and then jumping.
    table.begin(4);
    send(&table, &mut viewer, &mut mirror, &interest, 1 << 20);
    let m = mirror.get(left_behind).unwrap();
    assert_eq!(m.samples().map(|(t, _)| t).collect::<Vec<_>>(), vec![1, 4]);
}

#[test]
fn bad_bodies_are_refused() {
    let mut table = Table::new(64);
    table.begin(1);
    table.spawn(1, 0, 0, 0, b"x", &state(0, 0));
    let world = table.publish();
    let mut viewer = Viewer::new();
    let mut body = Vec::new();
    viewer.write(&world, &Value::Unit, &view(0, 0), &mut body);
    for cut in 0..body.len() {
        assert!(Mirror::new().apply(1, true, &body[..cut]).is_err(), "cut at {cut}");
    }
    let mut trailing = body.clone();
    trailing.push(0);
    assert!(Mirror::new().apply(1, true, &trailing).is_err());
    // An update for a thing never spawned.
    let mut mirror = Mirror::new();
    mirror.apply(1, true, &body).unwrap();
    let update = [0u8, GLOBALS_SAME, 0, 0, 1, 9, 1, 0];
    assert!(mirror.apply(2, false, &update).is_err());
}

#[test]
fn the_grid_finds_exactly_the_things_in_a_rectangle() {
    let mut rng = Rng(99);
    let mut table = Table::new(50);
    table.begin(1);
    for id in 0..500 {
        let (x, y) = (rng.range(-400, 400), rng.range(-400, 400));
        table.spawn(id, 0, x, y, b"", &[]);
    }
    let world = table.publish();
    for _ in 0..50 {
        let (x0, y0) = (rng.range(-500, 300), rng.range(-500, 300));
        let (x1, y1) = (x0 + rng.range(0, 300), y0 + rng.range(0, 300));
        let mut found = Vec::new();
        world.within(x0, y0, x1, y1, |e| found.push(e.id));
        found.sort_unstable();
        let expected: Vec<u32> = world
            .entities()
            .iter()
            .filter(|e| (x0..=x1).contains(&e.x) && (y0..=y1).contains(&e.y))
            .map(|e| e.id)
            .collect();
        assert_eq!(found, expected);
    }
}

#[test]
fn an_update_carries_only_the_bytes_that_changed() {
    let mut table = Table::new(64);
    table.begin(1);
    table.spawn(3, 0, 0, 0, b"", &[10, 20, 30, 40, 50]);
    let interest = view(0, 0);
    let mut viewer = Viewer::new();
    let mut mirror = Mirror::new();
    send(&table, &mut viewer, &mut mirror, &interest, BUDGET);

    // One byte of five: flags, globals, three counts, then id 3, the
    // mask's head (0b00010 * 2 + 1) and the one byte.
    table.begin(2);
    table.update(3, 0, 0, &[10, 21, 30, 40, 50]);
    let world = table.publish();
    let mut body = Vec::new();
    viewer.write(&world, &globals(1), &interest, &mut body);
    assert_eq!(&body[2..], &[0, 0, 1, 3, 0b101, 21]);
    mirror.apply(2, false, &body).unwrap();
    assert_eq!(mirror.get(3).unwrap().latest(), &[10, 21, 30, 40, 50]);

    // A new length goes whole.
    table.begin(3);
    table.update(3, 0, 0, &[1, 2]);
    let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(w.updates, 1);
    assert_eq!(mirror.get(3).unwrap().latest(), &[1, 2]);

    // Changed and changed back between two writes: nothing to send.
    table.begin(4);
    table.update(3, 0, 0, &[9, 9]);
    table.begin(5);
    table.update(3, 0, 0, &[1, 2]);
    let w = send(&table, &mut viewer, &mut mirror, &interest, BUDGET);
    assert_eq!(w.updates, 0);
    table.begin(6);
    table.update(3, 0, 0, &[1, 3]);
    assert_eq!(send(&table, &mut viewer, &mut mirror, &interest, BUDGET).updates, 1);
    assert_eq!(mirror.get(3).unwrap().latest(), &[1, 3]);

    // A mask naming bytes past the held state is refused.
    let mut bad = body.clone();
    let at = bad.len() - 2;
    bad[at] = (0b100000 << 1) | 1;
    assert!(mirror.apply(7, false, &bad).is_err());
}

#[test]
fn a_rectangle_far_bigger_than_the_world_looks_only_at_its_rows() {
    let mut table = Table::new(16);
    table.begin(1);
    table.spawn(1, 0, 5, 5, b"", &[]);
    table.spawn(2, 0, -40, 900, b"", &[]);
    let world = table.publish();
    let mut found = Vec::new();
    world.within(i32::MIN, i32::MIN, i32::MAX, i32::MAX, |e| found.push(e.id));
    found.sort_unstable();
    assert_eq!(found, vec![1, 2]);
    let mut none = Vec::new();
    Table::new(16).publish().within(i32::MIN, i32::MIN, i32::MAX, i32::MAX, |e| none.push(e.id));
    assert!(none.is_empty());
}
