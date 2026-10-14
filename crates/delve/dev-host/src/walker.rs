//! `delve-walker`: a native seat that walks a Delve host's knight to every
//! door it can find and tries it, the way a player would: to each kingdom
//! place's stand (then E), and onto each entrance's stair in the room it
//! comes to. It prints what the run did. Development only.
//!
//! ```text
//! delve-walker <link> [--only NAME] [--park] [--stay] [--active SECS]
//! ```
//!
//! - `--only NAME`: only the kingdom places whose name holds NAME.
//! - `--park`: walk to the place's stand, print the seat's token and go
//!   without a goodbye, so a browser can take the seat there (its
//!   `angel.seat.<key>` in local storage) within the resume window.
//! - `--stay`: start where the knight stands, rather than walking out of
//!   the room it is in first.
//! - `--active SECS`: play on where the knight stands, aiming about, and
//!   nothing else (a friend busy inside while another waits outside).

use angel_delve::client::DelveClient;
use angel_delve::rules::layout::{TILE_UNITS, Tile};
use angel_delve::rules::{Run, map, world};
use angel_delve::wire;
use angel_net::client::{Runtime, Status};
use angel_net::input::button;
use angel_serve::client::WsClient;
use angel_serve::ws::Message;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

struct Seat {
    ws: WsClient,
    rt: Runtime<DelveClient>,
    start: Instant,
    next_frame: f64,
    lines: Vec<String>,
}

impl Seat {
    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    /// Run the link for `ms`, holding `bits`.
    fn run(&mut self, ms: f64, bits: u32) {
        let until = self.now() + ms;
        self.rt.hands().set_held(bits);
        while self.now() < until {
            self.pump();
        }
    }

    fn pump(&mut self) {
        for out in self.rt.take_outgoing() {
            let _ = self.ws.send(&out);
        }
        let wait = Duration::from_secs_f64(((self.next_frame - self.now()) / 1000.0).clamp(0.0, 0.02));
        if self.ws.fill(usize::MAX, wait).is_err() {
            panic!("the link closed");
        }
        while let Ok(Some(message)) = self.ws.message() {
            if let Message::Binary(bytes) = message {
                let now = self.now();
                let _ = self.rt.recv(&bytes, now);
                if let Status::Rejected(why, text) = self.rt.status() {
                    panic!("turned away ({why:?}: {text})");
                }
            }
        }
        let now = self.now();
        if now >= self.next_frame {
            self.next_frame = (self.next_frame + 1000.0 / 60.0).max(now);
            self.rt.frame(now);
            for line in self.rt.take_lines() {
                if !line.starts_with("sfx") && !line.starts_with("music") {
                    self.lines.push(line);
                }
            }
        }
    }

    fn knight(&self) -> u32 {
        wire::knight_of(self.rt.seat().expect("seated"))
    }

    fn run_state(&self) -> Option<&Run> {
        self.rt.game().and_then(|g| g.newest())
    }

    fn at(&self) -> Option<(f32, f32)> {
        let run = self.run_state()?;
        let me = run.players.get(&self.knight())?;
        Some((me.x, me.y))
    }

    /// The room our knight stands in: the party's, or the one it waits
    /// outside in.
    fn my_room<'r>(&self, run: &'r Run) -> &'r angel_delve::rules::Room {
        run.exits.outside.get(&self.knight()).map_or(run.room(), |e| &run.dungeon.rooms[e.at])
    }

    fn room(&self) -> String {
        self.run_state().map_or("?".into(), |r| {
            let mine = self.my_room(r);
            let at = r.exits.outside.get(&self.knight()).map_or(r.at, |e| e.at);
            if r.exits.outside.contains_key(&self.knight()) {
                format!("{:?}#{at} (waiting outside; the party is in {:?})", mine.kind, r.room().kind)
            } else {
                format!("{:?}#{at}", mine.kind)
            }
        })
    }

    /// Walk to `goal` (units) along the room's open tiles; true if reached.
    fn walk_to(&mut self, goal: (f32, f32), avoid_stairs: bool, max_ms: f64) -> bool {
        let started = self.now();
        let room_then = self.room();
        let mut last = (self.now(), self.at());
        while self.now() - started < max_ms {
            if self.room() != room_then {
                self.run(50.0, 0);
                return false;
            }
            let Some(me) = self.at() else {
                self.run(30.0, 0);
                continue;
            };
            let near = (me.0 - goal.0).hypot(me.1 - goal.1);
            if near < 0.6 {
                // Let what is in flight land, then look again.
                self.run(300.0, 0);
                if let Some(me) = self.at()
                    && (me.0 - goal.0).hypot(me.1 - goal.1) < 0.9
                {
                    return true;
                }
                continue;
            }
            let way = self.next_point(me, goal, avoid_stairs).unwrap_or(goal);
            let (dx, dy) = (way.0 - me.0, way.1 - me.1);
            let mut bits = 0;
            if dx > 0.2 {
                bits |= button::RIGHT;
            } else if dx < -0.2 {
                bits |= button::LEFT;
            }
            if dy > 0.2 {
                bits |= button::DOWN;
            } else if dy < -0.2 {
                bits |= button::UP;
            }
            // Stuck: wiggle sideways a moment.
            if self.now() - last.0 > 900.0 {
                if last.1.is_some_and(|p| (p.0 - me.0).hypot(p.1 - me.1) < 0.3) {
                    let side = if bits & (button::LEFT | button::RIGHT) != 0 { button::UP } else { button::RIGHT };
                    self.run(250.0, side);
                }
                last = (self.now(), Some(me));
            }
            if near < 3.0 {
                // Close: one tick's walk, then wait for it to land.
                self.run(34.0, bits);
                self.run(150.0, 0);
            } else {
                self.run(34.0, bits);
            }
        }
        self.run(50.0, 0);
        false
    }

    /// Out of the room by its nearest open doorway on the edge.
    fn leave_by_door(&mut self) -> bool {
        let Some(run) = self.run_state() else { return false };
        let room = self.my_room(run);
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let Some(me) = self.at() else { return false };
        let mut doors = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let edge = c == 0 || r == 0 || c == cols - 1 || r == rows - 1;
                if edge && matches!(room.tile(c, r), Tile::Door | Tile::Floor) {
                    let p = ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
                    let out = (if c == 0 { -1 } else if c == cols - 1 { 1 } else { 0 }, if r == 0 { -1 } else if r == rows - 1 { 1 } else { 0 });
                    doors.push(((p.0 - me.0).hypot(p.1 - me.1), p, out));
                }
            }
        }
        doors.sort_by(|a, b| a.0.total_cmp(&b.0));
        let before = self.room();
        for (_, p, (ox, oy)) in doors.into_iter().take(6) {
            self.walk_to(p, true, 20_000.0);
            let mut bits = 0;
            if ox < 0 { bits |= button::LEFT }
            if ox > 0 { bits |= button::RIGHT }
            if oy < 0 { bits |= button::UP }
            if oy > 0 { bits |= button::DOWN }
            self.run(1500.0, bits);
            self.run(800.0, 0);
            if self.room() != before {
                return true;
            }
        }
        false
    }

    /// The centre of the next tile on the shortest open path from `me` to
    /// `goal`, by tiles.
    fn next_point(&self, me: (f32, f32), goal: (f32, f32), avoid_stairs: bool) -> Option<(f32, f32)> {
        let run = self.run_state()?;
        let room = self.my_room(run);
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let tile = |p: (f32, f32)| ((p.0 / TILE_UNITS).floor() as i32, (p.1 / TILE_UNITS).floor() as i32);
        let (from, to) = (tile(me), tile(goal));
        if from == to {
            return Some(goal);
        }
        let open = |c: i32, r: i32| {
            (c, r) == to
                || match room.tile(c, r) {
                    Tile::Floor | Tile::Door | Tile::Ledge => true,
                    Tile::Stairs => !avoid_stairs,
                    _ => false,
                }
        };
        let idx = |c: i32, r: i32| (r * cols + c) as usize;
        let mut prev = vec![u32::MAX; (cols * rows) as usize];
        let mut queue = VecDeque::from([to]);
        prev[idx(to.0, to.1)] = idx(to.0, to.1) as u32;
        // From the goal back, so each tile knows its next step toward it.
        while let Some((c, r)) = queue.pop_front() {
            if (c, r) == from {
                break;
            }
            for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nc, nr) = (c + dc, r + dr);
                if nc < 0 || nr < 0 || nc >= cols || nr >= rows || prev[idx(nc, nr)] != u32::MAX {
                    continue;
                }
                if !open(nc, nr) && (nc, nr) != from {
                    continue;
                }
                prev[idx(nc, nr)] = idx(c, r) as u32;
                queue.push_back((nc, nr));
            }
        }
        if from.0 < 0 || from.1 < 0 || from.0 >= cols || from.1 >= rows {
            return None;
        }
        let step = prev[idx(from.0, from.1)];
        if step == u32::MAX {
            return None;
        }
        let (c, r) = ((step as i32) % cols, (step as i32) / cols);
        if (c, r) == to {
            return Some(goal);
        }
        Some(((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS))
    }

    fn say(&mut self, what: &str) {
        let lines = std::mem::take(&mut self.lines);
        let at = self.at().map_or("?".into(), |(x, y)| format!("({x:.1},{y:.1})"));
        println!("[{:6.1}s] {what} — room {} at {at}{}", self.now() / 1000.0, self.room(), if lines.is_empty() { String::new() } else { format!(" | {}", lines.join(" | ")) });
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let link = args.first().expect("delve-walker <link>");
    let only = args.iter().position(|a| a == "--only").and_then(|i| args.get(i + 1)).cloned();
    let park = args.iter().any(|a| a == "--park");
    let (host, key) = link.trim_start_matches("http://").split_once("/#").expect("http://HOST/#KEY");
    let url = format!("ws://{host}/ws");
    let ws = WsClient::connect(&url, Duration::from_secs(10)).expect("connects");
    let mut seat = Seat { ws, rt: Runtime::new(), start: Instant::now(), next_frame: 0.0, lines: Vec::new() };
    seat.rt.connected(key, "", "walker", angel_net::proto::caps::NATIVE);
    while seat.at().is_none() {
        seat.pump();
        if seat.now() > 10_000.0 {
            panic!("never seated");
        }
    }
    seat.run(500.0, 0);
    seat.say(&format!("seated as knight {}", seat.knight()));
    // `--active SECS`: a friend playing on where they stand, aiming about,
    // and nothing else.
    if let Some(secs) = args.iter().position(|a| a == "--active").and_then(|i| args.get(i + 1)).and_then(|s| s.parse::<f64>().ok()) {
        let until = seat.now() + secs * 1000.0;
        let aims = [button::AIM_LEFT, button::AIM_RIGHT];
        let mut i = 0;
        while seat.now() < until {
            seat.run(500.0, aims[i % 2]);
            i += 1;
        }
        seat.say("done playing on");
        return;
    }
    if !seat.room().starts_with("Kingdom") && !args.iter().any(|a| a == "--stay") {
        let out = seat.leave_by_door();
        seat.say(if out { "walked out to start" } else { "could not walk out to start" });
    }

    let geo = map::geography(&seat.run_state().unwrap().kingdom.wishes);
    let sites: Vec<(String, (i32, i32), String)> =
        geo.sites.iter().map(|s| (s.name.clone(), s.stand, format!("{:?}", s.room))).collect();
    for (name, stand, room) in sites {
        if only.as_ref().is_some_and(|o| !name.to_lowercase().contains(&o.to_lowercase())) {
            continue;
        }
        if !seat.room().starts_with("Kingdom") {
            seat.say("not in the kingdom; stopping the sweep");
            break;
        }
        let goal = ((stand.0 as f32 + 0.5) * TILE_UNITS, (stand.1 as f32 + 0.7) * TILE_UNITS);
        let reached = seat.walk_to(goal, true, 40_000.0);
        if !reached {
            seat.say(&format!("{name}: did not reach its stand {stand:?}"));
            if !seat.room().starts_with("Kingdom") {
                seat.say("walked into somewhere on the way");
            }
            continue;
        }
        if park {
            seat.say(&format!("parked at {name}'s stand; seat token {}", seat.rt.seat_token()));
            println!("TOKEN {}", seat.rt.seat_token());
            // Gone without a goodbye: the seat waits for its resume.
            std::process::exit(0);
        }
        let before = seat.room();
        let detail = |seat: &Seat| {
            let run = seat.run_state().unwrap();
            let me = &run.players[&seat.knight()];
            let geo = map::geography(&run.kingdom.wishes);
            let stone = angel_delve::rules::classes::stones(&geo)
                .iter()
                .map(|&(_, (sx, sy))| (me.x - (sx as f32 + 0.5) * TILE_UNITS).hypot(me.y - (sy as f32 + 0.5) * TILE_UNITS))
                .fold(f32::MAX, f32::min);
            format!("door {:.2} door_held {} stone {:.1} class {:?}", (me.x - goal.0).hypot(me.y - goal.1), run.kingdom.door_held, stone, me.class)
        };
        let pre = detail(&seat);
        seat.run(120.0, button::USE);
        seat.run(1500.0, 0);
        let after = seat.room();
        let post = if after == before { detail(&seat) } else { String::new() };
        seat.say(&format!("{name} ({room}): E at its stand → {} [{pre}] [{post}]", if after != before { "ENTERED" } else { "nothing" }));
        if after != before {
            // Into the place: try its own entrances, then leave.
            try_entrances(&mut seat);
            if !seat.room().starts_with("Kingdom") {
                let out = seat.leave_by_door();
                seat.say(if out { "walked out by a door" } else { "could not walk out" });
            }
            if !seat.room().starts_with("Kingdom") {
                seat.run(150.0, button::OK);
                seat.run(1500.0, 0);
                seat.say("pressed Enter to leave");
            }
            if !seat.room().starts_with("Kingdom") {
                seat.say("still inside; stopping");
                break;
            }
        }
    }
}

/// Each entrance in the room the party stands in: stand on its stair a
/// while, and say where it went.
fn try_entrances(seat: &mut Seat) {
    let Some(kind) = seat.run_state().map(|r| r.room().kind) else { return };
    for e in world::ENTRANCES.iter().filter(|e| e.room == kind) {
        let (c, r, w, h) = e.at;
        let goal = ((c as f32 + w as f32 / 2.0) * TILE_UNITS, (r as f32 + h as f32 / 2.0) * TILE_UNITS);
        let before = seat.room();
        let reached = seat.walk_to(goal, false, 30_000.0);
        seat.run(2500.0, 0);
        let after = seat.room();
        seat.say(&format!(
            "  entrance {} ({}): {} → {}",
            e.id,
            e.label,
            if reached { "stood on it" } else { "did not reach it" },
            if after != before { "WENT" } else { "nothing" }
        ));
        if after != before {
            return;
        }
    }
}
