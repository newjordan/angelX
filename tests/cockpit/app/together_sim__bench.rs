//! `cargo test --release ... delve_engine_bench -- --ignored --nocapture`:
//! a headless four-seat Delve on the engine, timed per tick and per frame,
//! with the bytes each seat's share of a tick costs.
use crate::drive::together_shooter::{Input, RoomKind, Run};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn four_seats() -> Run {
    let mut run = Run::new(3, 9, Some("Two"));
    for (id, name) in [(3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(fight);
    run
}

fn inputs(i: usize) -> BTreeMap<u32, Input> {
    (1..=4)
        .map(|seat| {
            let turn = (i / 20 + seat as usize) % 4;
            let (mx, my) = [(1, 0), (0, 1), (-1, 0), (0, -1)][turn];
            (
                seat,
                Input {
                    move_x: mx,
                    move_y: my,
                    aim_x: 1,
                    fire: i % 3 == 0,
                    ..Default::default()
                },
            )
        })
        .collect()
}

pub(crate) struct Stats(Vec<Duration>);
impl Stats {
    pub(crate) fn new() -> Self {
        Stats(Vec::new())
    }
    pub(crate) fn time<T>(&mut self, f: impl FnOnce() -> T) -> T {
        let at = Instant::now();
        let out = f();
        self.0.push(at.elapsed());
        out
    }
    pub(crate) fn line(&mut self, what: &str) -> String {
        self.0.sort();
        let n = self.0.len().max(1);
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mean = self.0.iter().map(|d| ms(*d)).sum::<f64>() / n as f64;
        let p95 = self.0.get(n * 95 / 100).map_or(0.0, |d| ms(*d));
        format!(
            "{what}: mean {mean:.3} ms, p95 {p95:.3} ms (n={})",
            self.0.len()
        )
    }
}

fn kingdom_four() -> Run {
    let mut run = Run::at_home(41, 1, Some("Two"), Default::default(), Default::default());
    for (id, name) in [(3, "Three"), (4, "Four")] {
        run.join_seat(id, name);
    }
    run.walk_kingdom(None);
    assert_eq!(run.room().kind, RoomKind::Kingdom);
    run
}

/// One scene on the engine: the step and the publish (the cockpit's
/// snapshot and the tree, once a tick) on the server's sim; each friend's
/// share of the tree diffed and encoded on its own writer; one seat's
/// client reading each tick and drawing a frame.
fn engine_scene(name: &str, mut run: Run) {
    use crate::drive::together_seat::Client;
    use angel_delve::wire;
    use angel_net::client::{Frame, GameClient, View};
    use angel_net::proto::{PROTOCOL, Welcome};
    use angel_serve::Replicate;
    let (mut step, mut snap, mut tree, mut encode, mut read, mut draw) = (
        Stats::new(),
        Stats::new(),
        Stats::new(),
        Stats::new(),
        Stats::new(),
        Stats::new(),
    );
    let rep = angel_serve::DocRep(crate::drive::together_sim::SeatView);
    let mut viewers: Vec<_> = (0..4u32).map(|seat| rep.viewer(Some(seat))).collect();
    let params = wire::Params::of(angel_delve::settings::get()).encode();
    let welcome = Welcome {
        protocol: PROTOCOL,
        game: Client::GAME,
        seat: Some(1),
        seat_token: "",
        tick_hz: 30,
        snapshot_hz: 30,
        tick: 0,
        params: &params,
    };
    let mut client = Client::boot(&welcome).unwrap();
    let mut frame = Frame::default();
    let mut bytes = 0usize;
    let ticks = 300;
    for i in 0..ticks {
        step.time(|| run.step(&inputs(i)));
        let copy = snap.time(|| run.clone());
        let doc = angel_serve::DocTick {
            tree: tree.time(|| wire::publish(&copy).unwrap()),
        };
        for (seat, viewer) in viewers.iter_mut().enumerate() {
            let mut out = Vec::new();
            let whole = encode.time(|| rep.snapshot(&doc, Some(seat as u32), viewer, &mut out));
            bytes += out.len();
            if seat == 1 {
                read.time(|| client.snapshot(i as u32 + 1, whole, &out).unwrap());
            }
        }
        let view = View {
            render_tick: i as f64 + 0.5,
            newest_tick: i as u32 + 1,
            ack: 0,
            pending: &[],
            partial: 0.5,
            hands: Default::default(),
            seat: Some(1),
            now_ms: i as f64 * 33.3,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        draw.time(|| client.draw(&view, &mut frame));
    }
    println!(
        "DELVE ENGINE BENCH {name}: {ticks} ticks, 4 seats, frame {}x{}",
        frame.w, frame.h
    );
    println!("{}", step.line("step (sim)"));
    println!("{}", snap.line("snapshot for the cockpit (sim)"));
    println!("{}", tree.line("tick tree, once for every seat (sim)"));
    println!(
        "{}",
        encode.line("one seat's share diffed and encoded (its writer)")
    );
    println!("{}", read.line("a client reads a tick"));
    println!("{}", draw.line("a client draws a frame"));
    println!("bytes per tick per seat: {}", bytes / ticks / 4);
}

#[test]
#[ignore]
fn delve_engine_bench() {
    engine_scene("fight", four_seats());
    engine_scene("kingdom", kingdom_four());
}
