//! The muster's client half: the snapshots into a mirror, everyone else
//! drawn 100 ms behind between two snapshots, your own knight predicted
//! from your commands the server has not answered yet, and a 320×180
//! picture drawn in software. Builds for wasm unchanged.

use crate::font;
use crate::rules::*;
use angel_net::client::{Frame, GameClient, View};
use angel_net::cmd::Command;
use angel_net::codec::{Bytes, Malformed};
use angel_net::doc::value::Value;
use angel_net::entity::Mirror;
use angel_net::input::Hands;
use angel_net::proto::Welcome;
use std::collections::VecDeque;

/// The touch layout: the stick on the left, two buttons on the right.
const CONTROLS: &str = r#"{"stick":"left","buttons":[{"id":"fire","label":"Wave","bit":16},{"id":"use","label":"Banner","bit":32}]}"#;

/// A correction bigger than this (in sub-pixels) is a jump, not a slip:
/// it is shown at once rather than eased.
const SNAP: f32 = (64 * SUB) as f32;
/// Name tags drawn at most, nearest first: a crowd stays readable.
const NAME_TAGS: usize = 24;
/// Predictions remembered per command, for measuring corrections.
const SHOWN_KEPT: usize = 64;

const VOID: u32 = 0xFF14_1712;
const GRASS: [u32; 2] = [0xFF2B_3527, 0xFF2E_3929];
const TUFT: u32 = 0xFF3B_4A33;
const FENCE: u32 = 0xFF5C_4D38;
const SHADOW: u32 = 0xFF1E_2620;
const HELM: u32 = 0xFFA7_ADB6;
const VISOR: u32 = 0xFF2B_2E35;
const LEGS: u32 = 0xFF3F_434B;
const POLE: u32 = 0xFF8A_6A44;
const WOOL: [u32; 3] = [0xFFC9_C9BF, 0xFFBA_B6AB, 0xFFAA_A69C];
const HEAD: u32 = 0xFF70_645B;
const TEXT: u32 = 0xFFD0_D4DA;
const TEXT_SHADOW: u32 = 0xFF10_1310;

enum Thing {
    Knight { k: Knight, colour: u32 },
    Sheep { facing: Facing, walking: bool, wool: u32 },
    Banner { colour: u32 },
}

/// One thing to draw, at its feet, in screen pixels.
struct Sprite {
    x: i32,
    y: i32,
    id: u32,
    thing: Thing,
}

pub struct MusterClient {
    mirror: Mirror,
    seat: Option<u32>,
    /// Where each of our commands put our knight when it was first
    /// predicted: when the server answers one, the difference is a correction.
    shown: VecDeque<(u32, i32, i32)>,
    last_ack: u32,
    /// The correction being eased out, and when it started.
    offset: (f32, f32),
    offset_at: f64,
    sprites: Vec<Sprite>,
    tags: Vec<(i64, i32, i32, u32)>,
}

impl MusterClient {
    pub fn new(seat: Option<u32>) -> MusterClient {
        MusterClient {
            mirror: Mirror::new(),
            seat,
            shown: VecDeque::new(),
            last_ack: 0,
            offset: (0.0, 0.0),
            offset_at: 0.0,
            sprites: Vec::new(),
            tags: Vec::new(),
        }
    }

    pub fn mirror(&self) -> &Mirror {
        &self.mirror
    }

    pub fn seat(&self) -> Option<u32> {
        self.seat
    }

    /// Our knight as the server last said, then through every command it
    /// has not answered: (before the newest command, after it).
    pub fn predict(&self, pending: &[(u32, MusterCmd)]) -> Option<(Knight, Knight)> {
        let seat = self.seat?;
        let base = Knight::from_state(self.mirror.get(seat)?.latest())?;
        let mut banner = self.mirror.get(BANNER_BASE + seat).and_then(|b| read_place(b.latest())).map(|(x, y, _, _)| (x, y));
        let (mut before, mut k) = (base, base);
        for (_, cmd) in pending {
            before = k;
            match k.step(cmd, banner) {
                Some(Deed::Plant) => banner = Some((k.x, k.y)),
                Some(Deed::Lift) => banner = None,
                _ => {}
            }
        }
        Some((before, k))
    }

    /// The correction still showing at `now`.
    fn easing(&self, now: f64, over_ms: f64) -> (f32, f32) {
        let left = if over_ms > 0.0 { (1.0 - (now - self.offset_at) / over_ms).clamp(0.0, 1.0) as f32 } else { 0.0 };
        (self.offset.0 * left, self.offset.1 * left)
    }

    /// Our knight's place to draw, in sub-pixels, and its pose.
    fn own(&mut self, view: &View<MusterCmd>) -> Option<(f32, f32, Knight)> {
        let (_, after) = self.predict(view.pending)?;
        // The server answered more of our commands: compare its word with
        // what we showed for the last one it took.
        if view.ack != self.last_ack {
            self.last_ack = view.ack;
            let base = Knight::from_state(self.mirror.get(self.seat?)?.latest())?;
            if let Some(&(_, sx, sy)) = self.shown.iter().find(|(seq, _, _)| *seq == view.ack) {
                let (ex, ey) = ((sx - base.x) as f32, (sy - base.y) as f32);
                if ex != 0.0 || ey != 0.0 {
                    let (cx, cy) = self.easing(view.now_ms, view.correct_ms);
                    let (nx, ny) = (cx + ex, cy + ey);
                    self.offset = if nx.abs() > SNAP || ny.abs() > SNAP { (0.0, 0.0) } else { (nx, ny) };
                    self.offset_at = view.now_ms;
                }
            }
            while self.shown.front().is_some_and(|(seq, _, _)| *seq <= view.ack) {
                self.shown.pop_front();
            }
        }
        // Remember what each new command was first predicted to do.
        if let Some(&(seq, _)) = view.pending.last()
            && self.shown.back().is_none_or(|(last, _, _)| *last < seq)
        {
            self.shown.push_back((seq, after.x, after.y));
            if self.shown.len() > SHOWN_KEPT {
                self.shown.pop_front();
            }
        }
        // Then the part of the coming tick the hands held now have walked:
        // a key shows on the next display frame, not at the next command
        // tick 50 ms on. The sampler walks a tick for any direction held
        // in it, so this is what the next command does and nothing drawn
        // here is taken back. Held only: a press acts once, when sampled.
        let mut ahead = after;
        ahead.step(&MusterCmd::from_hands(&view.hands).held_only(), None);
        let t = view.partial;
        let (ox, oy) = self.easing(view.now_ms, view.correct_ms);
        let x = after.x as f32 + (ahead.x - after.x) as f32 * t + ox;
        let y = after.y as f32 + (ahead.y - after.y) as f32 * t + oy;
        let pose = Knight { facing: ahead.facing, walking: ahead.walking, ..after };
        Some((x, y, pose))
    }

    fn count(&self, field: &str) -> u64 {
        match self.mirror.globals().field(field) {
            Some(Value::U(n)) => *n,
            _ => 0,
        }
    }
}

fn lerp(a: i32, b: i32, t: f32) -> f32 {
    a as f32 + (b - a) as f32 * t
}

fn shade(colour: u32, num: u32) -> u32 {
    let c = |shift: u32| (((colour >> shift) & 0xff) * num / 10) << shift;
    0xFF00_0000 | c(16) | c(8) | c(0)
}

/// The muster's frame and its drawing tools.
struct Canvas<'a> {
    px: &'a mut [u32],
    w: i32,
    h: i32,
}

impl Canvas<'_> {
    fn put(&mut self, x: i32, y: i32, c: u32) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    fn row(&mut self, x0: i32, x1: i32, y: i32, c: u32) {
        if y < 0 || y >= self.h {
            return;
        }
        let (a, b) = (x0.max(0), x1.min(self.w));
        if a < b {
            let at = (y * self.w) as usize;
            self.px[at + a as usize..at + b as usize].fill(c);
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, c: u32) {
        for (i, ch) in text.chars().enumerate() {
            let g = font::glyph_of(ch);
            let gx = x + i as i32 * font::ADVANCE;
            for row in 0..font::HEIGHT {
                for col in 0..3 {
                    if g >> (14 - (row * 3 + col)) & 1 != 0 {
                        self.put(gx + col, y + row, c);
                    }
                }
            }
        }
    }

    fn label(&mut self, x: i32, y: i32, text: &str) {
        self.label_in(x, y, text, TEXT);
    }

    fn label_in(&mut self, x: i32, y: i32, text: &str, ink: u32) {
        self.text(x + 1, y + 1, text, TEXT_SHADOW);
        self.text(x, y, text, ink);
    }

    /// Rows of a sprite: each char a pixel, `.` clear.
    fn sprite(&mut self, x: i32, y: i32, rows: &[&[u8]], flip: bool, colour: impl Fn(u8) -> u32) {
        for (r, row) in rows.iter().enumerate() {
            let w = row.len() as i32;
            for (c, &p) in row.iter().enumerate() {
                if p != b'.' {
                    let cx = if flip { w - 1 - c as i32 } else { c as i32 };
                    self.put(x + cx, y + r as i32, colour(p));
                }
            }
        }
    }

    fn ground(&mut self, cam_x: i32, cam_y: i32) {
        const TILE: i32 = 16;
        for sy in 0..self.h {
            let wy = cam_y + sy;
            if !(0..FIELD_PX).contains(&wy) {
                self.row(0, self.w, sy, VOID);
                continue;
            }
            let ty = wy.div_euclid(TILE);
            let mut sx = 0;
            while sx < self.w {
                let wx = cam_x + sx;
                let end = (sx + TILE - wx.rem_euclid(TILE)).min(self.w);
                let c = if !(0..FIELD_PX).contains(&wx) { VOID } else { GRASS[((wx.div_euclid(TILE) + ty) & 1) as usize] };
                self.row(sx, end, sy, c);
                sx = end;
            }
        }
        // A tuft in some tiles, where a hash of the tile puts it.
        let (tx0, ty0) = (cam_x.div_euclid(TILE), cam_y.div_euclid(TILE));
        for ty in ty0..=ty0 + self.h / TILE + 1 {
            for tx in tx0..=tx0 + self.w / TILE + 1 {
                if !(0..FIELD_PX / TILE).contains(&tx) || !(0..FIELD_PX / TILE).contains(&ty) {
                    continue;
                }
                let h = (tx as u32).wrapping_mul(73_856_093) ^ (ty as u32).wrapping_mul(19_349_663);
                if h & 3 == 0 {
                    let (x, y) = (tx * TILE + (h >> 4 & 15) as i32 - cam_x, ty * TILE + (h >> 8 & 15) as i32 - cam_y);
                    self.put(x, y, TUFT);
                    self.put(x + 1, y - 1, TUFT);
                    self.put(x + 2, y, TUFT);
                }
            }
        }
        // The fence round the field.
        for edge in [0, FIELD_PX] {
            let x = edge - cam_x;
            for sy in 0..self.h {
                let wy = cam_y + sy;
                if (0..=FIELD_PX).contains(&wy) {
                    self.put(x, sy, FENCE);
                }
            }
            let y = edge - cam_y;
            self.row((0 - cam_x).max(0), (FIELD_PX - cam_x).min(self.w), y, FENCE);
        }
    }

    fn knight(&mut self, x: i32, y: i32, k: &Knight, colour: u32, phase: bool) {
        let dark = shade(colour, 7);
        self.row(x - 2, x + 3, y + 1, SHADOW);
        let visor: &[u8] = if k.facing == Facing::Up { b".HHHHH." } else { b".HVVVH." };
        let legs: &[u8] = if k.walking && phase { b".L...L." } else { b"..L.L.." };
        let arms: &[u8] = if k.wave > 0 { b"C.CCC.." } else { b"C.CCC.C" };
        let rows: [&[u8]; 10] =
            [b"..HHH..", b".HHHHH.", visor, b"..HHH..", b".CCCCC.", b"CCCCCCC", arms, b"..DDD..", legs, legs];
        self.sprite(x - 3, y - 9, &rows, false, |p| match p {
            b'H' => HELM,
            b'V' => VISOR,
            b'C' => colour,
            b'D' => dark,
            _ => LEGS,
        });
        if k.wave > 0 {
            // The arm up, the hand swinging.
            let swing = i32::from(k.wave / 3 % 2);
            for r in 5..8 {
                self.put(x + 3, y - r, colour);
            }
            self.put(x + 3 + swing, y - 8, HELM);
        }
        if k.carrying {
            for r in 0..13 {
                self.put(x + 4, y - r, POLE);
            }
            for r in 10..13 {
                self.row(x + 5, x + 9, y - r, colour);
            }
        }
    }

    fn banner(&mut self, x: i32, y: i32, colour: u32) {
        self.row(x - 1, x + 2, y + 1, SHADOW);
        for r in 0..15 {
            self.put(x, y - r, POLE);
        }
        for r in 11..15 {
            self.row(x + 1, x + 6, y - r, colour);
        }
        self.row(x + 1, x + 6, y - 11, shade(colour, 7));
    }

    fn sheep(&mut self, x: i32, y: i32, facing: Facing, walking: bool, wool: u32, phase: bool) {
        self.row(x - 3, x + 4, y + 1, SHADOW);
        let legs: &[u8] = if walking && phase { b"h....h.." } else { b".h..h..." };
        let rows: [&[u8]; 6] = [b".WWWW...", b"WWWWWWhh", b"WWWWWWhe", b"WWWWWW.h", b".WWWW...", legs];
        self.sprite(x - 4, y - 5, &rows, facing == Facing::Left, |p| match p {
            b'W' => wool,
            b'e' => wool,
            _ => HEAD,
        });
    }
}

impl GameClient for MusterClient {
    type Cmd = MusterCmd;
    const GAME: &'static str = GAME;

    fn boot(welcome: &Welcome) -> Result<Self, String> {
        let mut b = Bytes::new(welcome.params);
        let version = b.varint().map_err(|_| "the muster's params did not read")?;
        if version != crate::PARAMS_VERSION {
            return Err("this page is out of date: reload it".into());
        }
        let field = b.varint().map_err(|_| "the muster's params did not read")?;
        if field != FIELD_PX as u64 {
            return Err("this page is out of date: reload it".into());
        }
        Ok(MusterClient::new(welcome.seat))
    }

    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        self.mirror.apply(tick, whole, body)
    }

    fn events(&mut self, _tick: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        for e in events {
            let mut b = Bytes::new(e);
            let (Ok(kind), Ok(_seat)) = (b.byte(), b.varint()) else { continue };
            let name = String::from_utf8_lossy(b.rest);
            match kind {
                EV_WAVE => lines.push("sfx wave".into()),
                EV_PLANT | EV_LIFT => lines.push("sfx banner".into()),
                EV_JOIN => lines.push(format!("notice {name} joined the muster")),
                EV_LEAVE => lines.push(format!("notice {name} left the muster")),
                _ => {}
            }
        }
    }

    fn command(&mut self, hands: &Hands) -> MusterCmd {
        MusterCmd::from_hands(hands)
    }

    fn encode_cmd(cmd: &MusterCmd, out: &mut Vec<u8>) {
        out.extend_from_slice(&cmd.encode());
    }

    fn draw(&mut self, view: &View<MusterCmd>, frame: &mut Frame) {
        frame.resize(VIEW_W, VIEW_H);
        self.mirror.prune(view.render_tick);
        let own = self.own(view);
        let (cam_sub_x, cam_sub_y) = own.map_or((FIELD as f32 / 2.0, FIELD as f32 / 2.0), |(x, y, _)| (x, y));
        let cam_x = (cam_sub_x / SUB as f32).round() as i32 - VIEW_W as i32 / 2;
        let cam_y = (cam_sub_y / SUB as f32).round() as i32 - VIEW_H as i32 / 2;
        let mut canvas = Canvas { px: &mut frame.px, w: VIEW_W as i32, h: VIEW_H as i32 };
        canvas.ground(cam_x, cam_y);

        // Everyone, where they were 100 ms ago, between two snapshots.
        self.sprites.clear();
        let on_screen = |x: i32, y: i32| x > -24 && x < VIEW_W as i32 + 24 && y > -8 && y < VIEW_H as i32 + 24;
        let to_screen = |sx: f32, sy: f32| ((sx / SUB as f32).round() as i32 - cam_x, (sy / SUB as f32).round() as i32 - cam_y);
        for (id, m) in self.mirror.iter() {
            if Some(id) == self.seat && own.is_some() {
                continue;
            }
            let (a, b, t) = m.at(view.render_tick);
            let late = t >= 0.5;
            let thing_and_place = match m.kind {
                KIND_KNIGHT => {
                    let (Some(ka), Some(kb)) = (Knight::from_state(a), Knight::from_state(b)) else { continue };
                    let colour = PALETTE[usize::from(*m.spawn.first().unwrap_or(&0)) % PALETTE.len()];
                    let k = if late { kb } else { ka };
                    (Thing::Knight { k, colour }, lerp(ka.x, kb.x, t), lerp(ka.y, kb.y, t))
                }
                KIND_SHEEP => {
                    let (Some(pa), Some(pb)) = (read_place(a), read_place(b)) else { continue };
                    let (_, _, facing, walking) = if late { pb } else { pa };
                    let wool = WOOL[usize::from(*m.spawn.first().unwrap_or(&0)) % WOOL.len()];
                    (Thing::Sheep { facing, walking, wool }, lerp(pa.0, pb.0, t), lerp(pa.1, pb.1, t))
                }
                KIND_BANNER => {
                    let Some((x, y, _, _)) = read_place(b) else { continue };
                    let colour = PALETTE[usize::from(*m.spawn.first().unwrap_or(&0)) % PALETTE.len()];
                    (Thing::Banner { colour }, x as f32, y as f32)
                }
                _ => continue,
            };
            let (thing, wx, wy) = thing_and_place;
            let (x, y) = to_screen(wx, wy);
            if on_screen(x, y) {
                self.sprites.push(Sprite { x, y, id, thing });
            }
        }
        if let (Some((x, y, k)), Some(seat)) = (own, self.seat) {
            let colour = PALETTE[usize::from(colour_of(seat))];
            let (x, y) = to_screen(x, y);
            self.sprites.push(Sprite { x, y, id: seat, thing: Thing::Knight { k, colour } });
        }
        self.sprites.sort_unstable_by_key(|s| (s.y, s.id));
        self.tags.clear();
        for s in &self.sprites {
            let phase = ((s.x + cam_x + s.y + cam_y) / 6) & 1 == 1;
            match &s.thing {
                Thing::Knight { k, colour } => {
                    canvas.knight(s.x, s.y, k, *colour, phase);
                    let (dx, dy) = (s.x - VIEW_W as i32 / 2, s.y - VIEW_H as i32 / 2);
                    self.tags.push((i64::from(dx * dx + dy * dy), s.x, s.y, s.id));
                }
                Thing::Sheep { facing, walking, wool } => canvas.sheep(s.x, s.y, *facing, *walking, *wool, phase),
                Thing::Banner { colour } => canvas.banner(s.x, s.y, *colour),
            }
        }

        // Names over the nearest knights; your own in your colour.
        self.tags.sort_unstable();
        for &(_, x, y, id) in self.tags.iter().take(NAME_TAGS) {
            let Some(m) = self.mirror.get(id) else { continue };
            let name = String::from_utf8_lossy(m.spawn.get(1..).unwrap_or(&[]));
            let ink = if Some(id) == self.seat { PALETTE[usize::from(colour_of(id))] } else { TEXT };
            canvas.label_in(x - font::width(&name) / 2, y - 17, &name, ink);
        }

        let count = format!("{} KNIGHTS", self.count("knights"));
        canvas.label(3, 3, &count);
        if self.seat.is_none() {
            let text = "WATCHING";
            canvas.label(VIEW_W as i32 - 3 - font::width(text), 3, text);
        } else if own.is_none() {
            let text = "JOINING";
            canvas.label(VIEW_W as i32 - 3 - font::width(text), 3, text);
        }
    }

    fn controls(&self) -> &'static str {
        CONTROLS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use angel_net::entity::{Interest, Table, Viewer};
    use angel_net::proto::PROTOCOL;

    fn welcome(seat: Option<u32>) -> MusterClient {
        let mut params = Vec::new();
        angel_net::codec::put_varint(&mut params, crate::PARAMS_VERSION);
        angel_net::codec::put_varint(&mut params, FIELD_PX as u64);
        let w = Welcome {
            protocol: PROTOCOL,
            game: GAME,
            seat,
            seat_token: "t",
            tick_hz: TICK_HZ,
            snapshot_hz: SNAPSHOT_HZ,
            tick: 0,
            params: &params,
        };
        MusterClient::boot(&w).unwrap()
    }

    fn interest(x: i32, y: i32) -> Interest {
        Interest { x, y, half_w: 200 * SUB, half_h: 120 * SUB, keep: 0, own: Some(0) }
    }

    #[test]
    fn our_knight_is_predicted_through_the_commands_not_yet_answered() {
        let mut client = welcome(Some(0));
        let mut table = Table::new(CELL_PX * SUB);
        table.begin(1);
        let start = Knight::new(FIELD / 2, FIELD / 2);
        table.spawn(0, KIND_KNIGHT, start.x, start.y, b"\x01Gwen", &start.state());
        let mut viewer = Viewer::new();
        let mut body = Vec::new();
        let w = viewer.write(&table.publish(), &Value::Unit, &interest(start.x, start.y), &mut body);
        client.snapshot(1, w.whole, &body).unwrap();
        let right = MusterCmd { x: 127, ..Default::default() };
        let pending = [(1, right), (2, right)];
        let (before, after) = client.predict(&pending).unwrap();
        assert_eq!(before.x, start.x + KNIGHT_SPEED);
        assert_eq!(after.x, start.x + 2 * KNIGHT_SPEED);
        // The server's answer to the first matches, so nothing is corrected.
        let mut server = start;
        server.step(&right, None);
        table.begin(2);
        table.update(0, server.x, server.y, &server.state());
        body.clear();
        viewer.write(&table.publish(), &Value::Unit, &interest(server.x, server.y), &mut body);
        client.snapshot(2, false, &body).unwrap();
        assert_eq!(client.predict(&pending[1..]).unwrap().1, after);
    }

    #[test]
    fn a_key_held_now_moves_our_knight_before_its_command_is_sent() {
        let mut client = welcome(Some(0));
        let mut table = Table::new(CELL_PX * SUB);
        table.begin(1);
        let start = Knight::new(FIELD / 2, FIELD / 2);
        table.spawn(0, KIND_KNIGHT, start.x, start.y, b"\x01Gwen", &start.state());
        let mut viewer = Viewer::new();
        let mut body = Vec::new();
        let w = viewer.write(&table.publish(), &Value::Unit, &interest(start.x, start.y), &mut body);
        client.snapshot(1, w.whole, &body).unwrap();
        let hands = Hands { x: 127, ..Default::default() };
        let view = |partial, hands| View {
            render_tick: 1.0,
            newest_tick: 1,
            ack: 0,
            pending: &[],
            partial,
            hands,
            seat: Some(0),
            now_ms: 0.0,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        let (still, _, _) = client.own(&view(0.5, Hands::default())).unwrap();
        assert_eq!(still, start.x as f32);
        // Halfway into a command tick, D down: half a step drawn already,
        // facing the way it goes.
        let (x, _, pose) = client.own(&view(0.5, hands)).unwrap();
        assert_eq!(x, (start.x + KNIGHT_SPEED / 2) as f32);
        assert!(pose.walking && pose.facing == Facing::Right);
    }

    #[test]
    fn a_frame_draws_the_field_and_everyone_on_it() {
        let mut client = welcome(Some(0));
        let mut table = Table::new(CELL_PX * SUB);
        table.begin(1);
        let me = Knight::new(FIELD / 2, FIELD / 2);
        table.spawn(0, KIND_KNIGHT, me.x, me.y, b"\x00Gwen", &me.state());
        let other = Knight::new(FIELD / 2 + 40 * SUB, FIELD / 2);
        table.spawn(1, KIND_KNIGHT, other.x, other.y, b"\x01Bors", &other.state());
        table.spawn(SHEEP_BASE, KIND_SHEEP, FIELD / 2 - 40 * SUB, FIELD / 2, &[0], &place_state(FIELD / 2 - 40 * SUB, FIELD / 2, Facing::Left, false));
        let mut viewer = Viewer::new();
        let mut body = Vec::new();
        let w = viewer.write(&table.publish(), &Value::Unit, &interest(me.x, me.y), &mut body);
        client.snapshot(1, w.whole, &body).unwrap();
        let mut frame = Frame::default();
        let view = View {
            render_tick: 1.0,
            newest_tick: 1,
            ack: 0,
            pending: &[],
            partial: 0.0,
            hands: Hands::default(),
            seat: Some(0),
            now_ms: 0.0,
            correct_ms: angel_net::clock::CORRECT_MS,
        };
        client.draw(&view, &mut frame);
        assert_eq!((frame.w, frame.h), (VIEW_W, VIEW_H));
        let at = |x: i32, y: i32| frame.px[(y * VIEW_W as i32 + x) as usize];
        let (cx, cy) = (VIEW_W as i32 / 2, VIEW_H as i32 / 2);
        // Our knight's helmet in the middle, the other's body 40 px right
        // in its colour, the sheep's wool 40 px left.
        assert_eq!(at(cx, cy - 9), HELM);
        assert_eq!(at(cx + 40, cy - 5), PALETTE[1]);
        assert_eq!(at(cx - 40, cy - 3), WOOL[0]);
        // The sheep faces left: its face is left of its wool.
        assert_eq!(at(cx - 43, cy - 3), HEAD);
        assert!(frame.px.iter().all(|p| p >> 24 == 0xff));
    }

    #[test]
    fn the_controls_name_the_fire_and_use_bits() {
        let client = welcome(None);
        let c = client.controls();
        assert!(c.contains(&format!("\"bit\":{}", angel_net::input::button::FIRE)));
        assert!(c.contains(&format!("\"bit\":{}", angel_net::input::button::USE)));
    }
}
