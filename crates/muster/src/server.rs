//! The muster's server half: knights for every seat, sheep wandering among
//! them, banners where knights plant them. Entity mode.

use crate::rules::*;
use angel_net::codec::put_varint;
use angel_net::doc::value::Value;
use angel_net::entity::{Interest, Table};
use angel_serve::{EntityRep, EntityTick, Events, GameServer, Rates};
use std::borrow::Cow;

/// Sheep run from a knight this close, and run this much faster than they walk.
const FLEE_PX: i32 = 40;
const SHEEP_WALK: i32 = 10;
const SHEEP_RUN: i32 = 40;
/// Sheep start within this far of the muster point.
const FLOCK_PX: i32 = 700;
/// How long a sheep grazes, and walks, in ticks (from..from+span).
const GRAZE: (u32, u32) = (40, 120);
const AMBLE: (u32, u32) = (20, 40);

struct Seated {
    knight: Knight,
    name: String,
    colour: u8,
    banner: Option<(i32, i32)>,
}

struct Sheep {
    x: i32,
    y: i32,
    facing: Facing,
    /// Sub-pixels a tick; zero while grazing.
    dx: i32,
    dy: i32,
    timer: u32,
}

pub struct MusterServer {
    seats: Vec<Option<Seated>>,
    sheep: Vec<Sheep>,
    table: Table,
    rng: u32,
}

impl Default for MusterServer {
    fn default() -> MusterServer {
        MusterServer::new(SHEEP)
    }
}

impl MusterServer {
    pub fn new(sheep: u32) -> MusterServer {
        let mut server = MusterServer {
            seats: (0..SEATS).map(|_| None).collect(),
            sheep: Vec::new(),
            table: Table::new(CELL_PX * SUB),
            rng: 0x9e37_79b9,
        };
        let centre = FIELD / 2;
        for i in 0..sheep {
            let x = centre + server.range(-FLOCK_PX, FLOCK_PX) * SUB;
            let y = centre + server.range(-FLOCK_PX, FLOCK_PX) * SUB;
            let timer = server.range(0, GRAZE.1 as i32) as u32;
            server.sheep.push(Sheep { x, y, facing: Facing::Down, dx: 0, dy: 0, timer });
            server.table.spawn(SHEEP_BASE + i, KIND_SHEEP, x, y, &[(i % 3) as u8], &place_state(x, y, Facing::Down, false));
        }
        server
    }

    /// A small deterministic generator, for the sheep.
    fn next(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng
    }

    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }

    fn interest(x: i32, y: i32, own: Option<u32>) -> Interest {
        Interest {
            x,
            y,
            half_w: (VIEW_W as i32 / 2 + MARGIN_PX) * SUB,
            half_h: (VIEW_H as i32 / 2 + MARGIN_PX) * SUB,
            keep: KEEP_PX * SUB,
            own,
        }
    }

    fn sheep_step(&mut self) {
        let knights: Vec<(i32, i32)> = self.seats.iter().flatten().map(|s| (s.knight.x, s.knight.y)).collect();
        for i in 0..self.sheep.len() {
            let (x, y) = (self.sheep[i].x, self.sheep[i].y);
            let near = knights
                .iter()
                .filter(|(kx, ky)| (kx - x).abs() < FLEE_PX * SUB && (ky - y).abs() < FLEE_PX * SUB)
                .min_by_key(|(kx, ky)| i64::from(kx - x).pow(2) + i64::from(ky - y).pow(2));
            if let Some(&(kx, ky)) = near {
                let (ax, ay) = (x - kx, y - ky);
                let len = (i64::from(ax).pow(2) + i64::from(ay).pow(2)).isqrt().max(1) as i32;
                let s = &mut self.sheep[i];
                s.dx = ax * SHEEP_RUN / len;
                s.dy = ay * SHEEP_RUN / len;
                s.timer = AMBLE.0;
            } else if self.sheep[i].timer == 0 {
                if self.sheep[i].dx != 0 || self.sheep[i].dy != 0 {
                    let t = self.range(GRAZE.0 as i32, (GRAZE.0 + GRAZE.1) as i32) as u32;
                    let s = &mut self.sheep[i];
                    (s.dx, s.dy, s.timer) = (0, 0, t);
                } else {
                    let (dx, dy) = (self.range(-SHEEP_WALK, SHEEP_WALK), self.range(-SHEEP_WALK, SHEEP_WALK));
                    let t = self.range(AMBLE.0 as i32, (AMBLE.0 + AMBLE.1) as i32) as u32;
                    let s = &mut self.sheep[i];
                    (s.dx, s.dy, s.timer) = (dx, dy, t);
                }
            } else {
                self.sheep[i].timer -= 1;
            }
            let s = &mut self.sheep[i];
            let (nx, ny) = (s.x + s.dx, s.y + s.dy);
            if !(0..FIELD).contains(&nx) {
                s.dx = -s.dx;
            }
            if !(0..FIELD).contains(&ny) {
                s.dy = -s.dy;
            }
            s.x = nx.clamp(0, FIELD - 1);
            s.y = ny.clamp(0, FIELD - 1);
            if s.dx != 0 || s.dy != 0 {
                s.facing = if s.dx.abs() > s.dy.abs() {
                    if s.dx < 0 { Facing::Left } else { Facing::Right }
                } else if s.dy < 0 {
                    Facing::Up
                } else {
                    Facing::Down
                };
            }
            let walking = s.dx != 0 || s.dy != 0;
            self.table.update(SHEEP_BASE + i as u32, s.x, s.y, &place_state(s.x, s.y, s.facing, walking));
        }
    }

    fn globals(&self) -> Value {
        let knights = self.seats.iter().flatten().count() as u64;
        let banners = self.seats.iter().flatten().filter(|s| s.banner.is_some()).count() as u64;
        Value::Struct(vec![
            (Cow::Borrowed("knights"), Value::U(knights)),
            (Cow::Borrowed("banners"), Value::U(banners)),
            (Cow::Borrowed("sheep"), Value::U(self.sheep.len() as u64)),
        ])
    }
}

fn event(kind: u8, seat: u32, name: Option<&str>) -> Vec<u8> {
    let mut out = vec![kind];
    put_varint(&mut out, u64::from(seat));
    if let Some(name) = name {
        out.extend_from_slice(name.as_bytes());
    }
    out
}

impl GameServer for MusterServer {
    type Cmd = MusterCmd;
    type Rep = EntityRep;
    const GAME: &'static str = GAME;

    fn rates(&self) -> Rates {
        Rates { tick_hz: TICK_HZ, snapshot_hz: SNAPSHOT_HZ }
    }

    fn seats(&self) -> u32 {
        SEATS
    }

    fn params(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_varint(&mut out, crate::PARAMS_VERSION);
        put_varint(&mut out, FIELD_PX as u64);
        out
    }

    fn replicator(&self) -> EntityRep {
        EntityRep
    }

    fn decode_cmd(bytes: &[u8]) -> Option<MusterCmd> {
        MusterCmd::decode(bytes)
    }

    fn join(&mut self, seat: u32, name: &str, events: &mut Events) {
        let (x, y) = spawn_point(seat);
        let knight = Knight::new(x, y);
        let colour = colour_of(seat);
        let mut spawn = vec![colour];
        spawn.extend_from_slice(name.as_bytes());
        spawn.truncate(angel_net::entity::MAX_SPAWN);
        self.table.spawn(seat, KIND_KNIGHT, x, y, &spawn, &knight.state());
        self.seats[seat as usize] = Some(Seated { knight, name: name.to_string(), colour, banner: None });
        events.all(event(EV_JOIN, seat, Some(name)));
    }

    fn leave(&mut self, seat: u32, events: &mut Events) {
        if let Some(gone) = self.seats[seat as usize].take() {
            events.all(event(EV_LEAVE, seat, Some(&gone.name)));
        }
        self.table.remove(seat);
        self.table.remove(BANNER_BASE + seat);
    }

    fn step(&mut self, tick: u32, cmds: &[Option<MusterCmd>], events: &mut Events) {
        self.table.begin(tick);
        for (seat, cmd) in cmds.iter().enumerate() {
            let (Some(cmd), Some(s)) = (cmd, self.seats[seat].as_mut()) else { continue };
            let seat = seat as u32;
            let k = &mut s.knight;
            match k.step(cmd, s.banner) {
                Some(Deed::Plant) => {
                    s.banner = Some((k.x, k.y));
                    self.table.spawn(BANNER_BASE + seat, KIND_BANNER, k.x, k.y, &[s.colour], &place_state(k.x, k.y, Facing::Down, false));
                    events.near(k.x, k.y, event(EV_PLANT, seat, None));
                }
                Some(Deed::Lift) => {
                    s.banner = None;
                    self.table.remove(BANNER_BASE + seat);
                    events.near(k.x, k.y, event(EV_LIFT, seat, None));
                }
                Some(Deed::Wave) => events.near(k.x, k.y, event(EV_WAVE, seat, None)),
                None => {}
            }
            self.table.update(seat, k.x, k.y, &k.state());
        }
        self.sheep_step();
    }

    fn publish(&mut self, _tick: u32) -> EntityTick {
        let seats = self
            .seats
            .iter()
            .enumerate()
            .map(|(seat, s)| s.as_ref().map(|s| Self::interest(s.knight.x, s.knight.y, Some(seat as u32))))
            .collect();
        EntityTick {
            world: self.table.publish(),
            globals: self.globals(),
            seats,
            watch: Self::interest(FIELD / 2, FIELD / 2, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_serve::To;

    fn seated(server: &mut MusterServer, seats: u32) -> Vec<Option<MusterCmd>> {
        let mut events = Events::default();
        for seat in 0..seats {
            server.join(seat, &format!("Knight {seat}"), &mut events);
        }
        let mut cmds = vec![None; SEATS as usize];
        for c in cmds.iter_mut().take(seats as usize) {
            *c = Some(MusterCmd::default());
        }
        cmds
    }

    #[test]
    fn a_knight_joins_where_its_seat_says_and_leaves_with_its_banner() {
        let mut server = MusterServer::new(0);
        let mut events = Events::default();
        server.join(3, "Gwen", &mut events);
        assert!(matches!(events.list()[0].to, To::All));
        let tick = server.publish(1);
        let k = tick.world.get(3).unwrap();
        assert_eq!((k.x, k.y), spawn_point(3));
        assert_eq!(&k.spawn[1..], b"Gwen");
        assert!(tick.seats[3].is_some() && tick.seats[0].is_none());

        let mut cmds = vec![None; SEATS as usize];
        cmds[3] = Some(MusterCmd { pressed: BANNER, ..Default::default() });
        server.step(2, &cmds, &mut events);
        assert!(server.publish(2).world.get(BANNER_BASE + 3).is_some());
        server.leave(3, &mut events);
        let after = server.publish(3);
        assert!(after.world.get(3).is_none() && after.world.get(BANNER_BASE + 3).is_none());
    }

    #[test]
    fn a_banner_is_lifted_from_near_and_a_wave_is_heard_near() {
        let mut server = MusterServer::new(0);
        let mut cmds = seated(&mut server, 1);
        let mut events = Events::default();
        cmds[0] = Some(MusterCmd { pressed: BANNER, ..Default::default() });
        server.step(1, &cmds, &mut events);
        // Walk off a little, come back, lift it.
        cmds[0] = Some(MusterCmd { x: 127, ..Default::default() });
        server.step(2, &cmds, &mut events);
        cmds[0] = Some(MusterCmd { pressed: BANNER, ..Default::default() });
        let mut events = Events::default();
        server.step(3, &cmds, &mut events);
        assert!(server.publish(3).world.get(BANNER_BASE).is_none());
        assert_eq!(events.list()[0].bytes[0], EV_LIFT);
        cmds[0] = Some(MusterCmd { pressed: WAVE, ..Default::default() });
        let mut events = Events::default();
        server.step(4, &cmds, &mut events);
        let wave = &events.list()[0];
        assert_eq!(wave.bytes[0], EV_WAVE);
        let (x, y) = spawn_point(0);
        assert!(matches!(wave.to, To::Near(wx, wy) if (wx - x).abs() < 4 * SUB && wy == y));
    }

    #[test]
    fn sheep_run_from_a_knight_beside_them() {
        let mut server = MusterServer::new(1);
        let (sx, sy) = (server.sheep[0].x, server.sheep[0].y);
        let mut events = Events::default();
        server.join(0, "Gwen", &mut events);
        // Put the knight just left of the sheep.
        server.seats[0].as_mut().unwrap().knight = Knight::new(sx - 10 * SUB, sy);
        let mut cmds = vec![None; SEATS as usize];
        cmds[0] = Some(MusterCmd::default());
        for tick in 1..10 {
            server.step(tick, &cmds, &mut events);
        }
        assert!(server.sheep[0].x > sx, "the sheep ran right");
    }
}
