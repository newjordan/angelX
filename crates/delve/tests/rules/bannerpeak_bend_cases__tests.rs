//! Case recorder for bannerpeak (Bend port, games_a): the driver and the cases of the
//! modules that need no private access (layout, world, knight, boss, the Run hooks).
//! Body, foes, session and bot have child recorders mounted in their own files.
//!
//! Run: cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast \
//!   --features bend-cases-games-a-bannerpeak --lib bannerpeak::bend_cases -- --ignored
use super::body::{Cmd, Ev, Knobs, Mover};
use super::boss::{self, Attack, Beat, Boss, Hazard};
use super::knight::Knight;
use super::layout::{self, info};
use super::session::{Phase, Session};
use super::world::{TILE, Tile, World};
use super::*;
use serde::Serialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap as Map;

#[path = "games_a_bend_cases__tests.rs"]
pub mod cases;

pub fn j<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap()
}

pub fn knobs_j(k: &Knobs) -> Value {
    json!({"run": k.run, "hop": k.hop, "grace": k.grace, "slam": k.slam, "ledge": k.ledge})
}
pub fn cmd_j(c: &Cmd) -> Value {
    json!({"x": c.x, "jump": c.jump, "jump_edge": c.jump_edge, "down": c.down,
           "down_edge": c.down_edge, "dive_edge": c.dive_edge, "brace": c.brace})
}
pub fn mover_j(m: &Mover) -> Value {
    json!({"id": m.id, "p": {"x": m.p.x, "y": m.p.y, "w": m.p.w, "lift": m.p.lift}})
}
pub fn movers_j(ms: &[Mover]) -> Value {
    Value::Array(ms.iter().map(mover_j).collect())
}
pub fn ev_j(e: &Ev) -> Value {
    match *e {
        Ev::Jump(k) => json!({"k": "Jump", "s": format!("{k:?}"), "a": 0, "b": 0}),
        Ev::Land { fell } => json!({"k": "Land", "s": "", "a": fell, "b": 0}),
        Ev::Bump { x, y } => json!({"k": "Bump", "s": "", "a": x, "b": y}),
        Ev::Slam { x, y } => json!({"k": "Slam", "s": "", "a": x, "b": y}),
        Ev::Skid => json!({"k": "Skid", "s": "", "a": 0, "b": 0}),
        Ev::Spring => json!({"k": "Spring", "s": "", "a": 0, "b": 0}),
        Ev::Hatch => json!({"k": "Hatch", "s": "", "a": 0, "b": 0}),
    }
}
pub fn evs_j(es: &[Ev]) -> Value {
    Value::Array(es.iter().map(ev_j).collect())
}
pub fn rect_j(r: (i32, i32, i32, i32)) -> Value {
    json!([r.0, r.1, r.2, r.3])
}
pub fn say_j(say: &[&str]) -> Value {
    json!(say)
}
pub fn heroes_j(h: &Map<u32, Hero>) -> Value {
    let m: serde_json::Map<String, Value> = h
        .iter()
        .map(|(id, hero)| (id.to_string(), json!({"away": hero.away, "stone": hero.stone})))
        .collect();
    Value::Object(m)
}
pub fn goal_j(g: &bot::Goal) -> Value {
    use bot::Goal::*;
    match *g {
        Walk(x) => json!({"k": "Walk", "a": x, "b": 0}),
        Slam(x) => json!({"k": "Slam", "a": x, "b": 0}),
        Hatch(x) => json!({"k": "Hatch", "a": x, "b": 0}),
        Down(x) => json!({"k": "Down", "a": x, "b": 0}),
        Backflip(x) => json!({"k": "Backflip", "a": x, "b": 0}),
        Wheel => json!({"k": "Wheel", "a": 0, "b": 0}),
        Climb((x, y)) => json!({"k": "Climb", "a": x, "b": y}),
        Board(x) => json!({"k": "Board", "a": x, "b": 0}),
        Fight => json!({"k": "Fight", "a": 0, "b": 0}),
    }
}

pub fn knobs() -> Knobs {
    Knobs {
        run: RUN as i32,
        hop: HOP,
        grace: COYOTE,
        slam: SLAM as i32,
        ledge: LEDGE as i32,
    }
}

/// A knob set a playtester might move to: the same game, other numbers.
pub fn knobs_alt(n: usize) -> Knobs {
    let k = knobs();
    match n % 4 {
        0 => k,
        1 => Knobs { run: 960, hop: 4, grace: 2, slam: 24, ledge: 1200 },
        2 => Knobs { run: 1664, hop: 30, grace: 9, slam: 96, ledge: 4000 },
        _ => Knobs { run: 1300, hop: 16, grace: 7, slam: 50, ledge: 2200 },
    }
}

pub struct Lcg(pub u64);
impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % ((hi - lo + 1) as u64)) as i64
    }
    pub fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
}

thread_local! {
    static CLASSES: RefCell<Map<(String, String), usize>> = const { RefCell::new(Map::new()) };
}

/// Whether a case of this class is still wanted (and if so, reserve its place).
pub fn want(name: &str, class: &str, per_class: usize, total: usize) -> bool {
    if cases::count(name) >= total {
        return false;
    }
    let key = (name.to_string(), class.to_string());
    CLASSES.with(|c| {
        let mut c = c.borrow_mut();
        let n = c.entry(key).or_insert(0);
        if *n >= per_class {
            false
        } else {
            *n += 1;
            true
        }
    })
}

/// Record a case unless `per_class` of its class (or `total` of its name) are written.
pub fn classed(name: &str, class: &str, per_class: usize, total: usize, input: Value, out: Value) {
    if want(name, class, per_class, total) {
        cases::rec(name, &input, &out);
    }
}

/// One tick's sample: the session before the step, the keys and the table.
pub struct Sample<'a> {
    pub s: &'a Session,
    pub inputs: &'a Map<u32, Input>,
    pub heroes: &'a Map<u32, Hero>,
    pub knobs: Knobs,
    pub tick: usize,
}

pub fn input_of(inputs: &Map<u32, Input>, id: u32) -> Input {
    inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default()
}

pub struct Table {
    pub s: Session,
    pub heroes: Map<u32, Hero>,
    pub said: Vec<&'static str>,
    pub knobs: Knobs,
}

impl Table {
    pub fn new(seats: u32) -> Table {
        let mut r = Run::bannerpeak_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        let heroes = r.players.clone();
        Table {
            s: Session::new(&heroes),
            heroes,
            said: Vec::new(),
            knobs: knobs(),
        }
    }
    pub fn tick(&mut self, inputs: &Map<u32, Input>) {
        self.s.step(inputs, &self.heroes, &self.knobs, &mut self.said);
    }
    pub fn put(&mut self, id: u32, cell: (i32, i32)) {
        let (x, y) = layout::cell_feet(cell);
        let k = self.s.seats.get_mut(&id).unwrap();
        k.body.place(x, y);
        k.safe = (x, y);
        k.hurt = 0;
    }
    pub fn bots(&self) -> Map<u32, Input> {
        self.s
            .seats
            .keys()
            .map(|&id| (id, self.s.bot_input(id, &self.heroes[&id])))
            .collect()
    }
}

/// A table of bots at the start of a stage: banners and sparks earned, at its gate.
pub fn stage(seats: u32, banners: u8, sparks: u8, gate: usize) -> Table {
    let mut t = Table::new(seats);
    t.s.banners = banners;
    t.s.sparks = sparks;
    t.s.checkpoint = gate as u8 + 1;
    t.s.gates = (1 << (gate + 1)) - 1;
    t.tick(&Map::new());
    for id in 1..=seats {
        let (cx, cy) = info().gates[gate];
        t.put(id, (cx + (id as i32 - 1), cy));
    }
    t.s.announce = None;
    if gate >= 7 {
        t.s.doors = 3;
    }
    t
}

/// What a stage has behind it: banners, sparks (as the play tests stage them).
pub const STAGES: [(u8, u8); 8] = [
    (0, 0),
    (0b11, 0),
    (0b111, 0b11),
    (0b1111, 0b1111),
    (0b11111, 0b11_1111),
    (0b11111, 0b11_1111),
    (0b11_1111, 0xFF & !0b10),
    (0x7F, 0xFF),
];

#[derive(Clone, Copy)]
pub enum Policy {
    Bots,
    Fuzz,
    BotsFuzz,
}

fn fuzz_inputs(t: &Table, g: &mut Lcg, held: &mut Map<u32, Input>) -> Map<u32, Input> {
    let mut out = Map::new();
    for &id in t.s.seats.keys() {
        let cur = held.entry(id).or_default();
        if g.chance(12) {
            cur.move_x = g.range(-1, 1) as i8;
        }
        if g.chance(10) {
            cur.dash = !cur.dash && g.chance(70);
        }
        if g.chance(8) {
            cur.move_y = if g.chance(70) { 0 } else { g.range(-1, 1) as i8 };
        }
        if g.chance(6) {
            cur.fire = g.chance(40);
        }
        if g.chance(6) {
            cur.swing = g.chance(30);
        }
        if g.chance(7) {
            cur.interact = g.chance(40);
        }
        out.insert(id, *cur);
    }
    out
}

/// Run a table `ticks` ticks under a policy; `probe` sees every tick before it steps.
pub fn run(
    t: &mut Table,
    ticks: usize,
    seed: u64,
    policy: Policy,
    every: usize,
    probe: &mut dyn FnMut(&Sample),
) {
    let mut g = Lcg(seed | 1);
    let mut held: Map<u32, Input> = Map::new();
    for tick in 0..ticks {
        let inputs = match policy {
            Policy::Bots => t.bots(),
            Policy::Fuzz => fuzz_inputs(t, &mut g, &mut held),
            Policy::BotsFuzz => {
                let mut b = t.bots();
                let f = fuzz_inputs(t, &mut g, &mut held);
                for (id, i) in b.iter_mut() {
                    if (tick / 40 + *id as usize) % 3 == 0 {
                        *i = f[id];
                    }
                }
                b
            }
        };
        if tick % every == 0 {
            probe(&Sample {
                s: &t.s,
                inputs: &inputs,
                heroes: &t.heroes,
                knobs: t.knobs,
                tick,
            });
        }
        t.tick(&inputs);
        if t.s.completed.is_some() && tick > 0 && t.s.elapsed > t.s.completed.unwrap() + 300 {
            break;
        }
    }
}

/// Every probe of one sample.
pub fn probe_all(sm: &Sample) {
    super::body::bend_cases_body::probe(sm);
    super::foes::bend_cases_foes::probe(sm);
    super::session::bend_cases_session::probe(sm);
    super::bot::bend_cases_bot::probe(sm);
    probe_top(sm);
}

/// Session::step as a whole, the knight's small methods, the boss on the live state.
fn probe_top(sm: &Sample) {
    let s = sm.s;
    let mut s2 = s.clone();
    let mut say = Vec::new();
    s2.step(sm.inputs, sm.heroes, &sm.knobs, &mut say);
    classed(
        "bannerpeak.session.Session.step",
        &format!("{:?}", s.phase),
        if matches!(s.phase, Phase::Play) { 200 } else { 30 },
        260,
        json!({"session": j(s), "inputs": j(sm.inputs), "heroes": heroes_j(sm.heroes), "knobs": knobs_j(&sm.knobs)}),
        json!({"session": j(&s2), "say": say_j(&say)}),
    );
    // Session::new from the table's heroes
    if sm.tick == 0 {
        cases::rec(
            "bannerpeak.session.Session.new",
            &json!({"heroes": heroes_j(sm.heroes)}),
            &j(&Session::new(sm.heroes)),
        );
    }
    for (&id, k) in &s.seats {
        let inp = input_of(sm.inputs, id);
        let (cmd, bits) = k.cmd(inp);
        classed(
            "bannerpeak.knight.Knight.cmd",
            &format!("{}{}", k.size as u8, k.carry),
            30,
            90,
            json!({"knight": j(k), "input": j(&inp)}),
            json!([cmd_j(&cmd), bits]),
        );
        for from in [-1i8, 1] {
            let mut k2 = k.clone();
            let ok = k2.hit(from);
            classed(
                "bannerpeak.knight.Knight.hit",
                &format!("{ok}{:?}", k.size),
                25,
                90,
                json!({"knight": j(k), "from": from}),
                json!({"knight": j(&k2), "ok": ok}),
            );
        }
        cases::rec_cap(
            "bannerpeak.knight.Knight.valid",
            &json!({"knight": j(k)}),
            &k.valid(),
            60,
        );
        for _ in 0..1 {
            let b = &k.body;
            let (fx, fy) = b.foot();
            let cam = follow_cam(k, sm.tick);
            let r = super::knight::follow(cam, fx, fy, b.face, b.vx, b.vy);
            cases::rec_cap(
                "bannerpeak.knight.follow",
                &json!({"cam": [cam.0, cam.1], "fx": fx, "fy": fy, "face": b.face, "vx": b.vx, "vy": b.vy}),
                &json!([r.0, r.1]),
                120,
            );
        }
        cases::rec_cap(
            "bannerpeak.knight.Knight.small",
            &json!({"knight": j(k), "bits": bits}),
            &json!({"free": k.free(), "alive": k.alive(), "big": k.big(), "act": k.act_edge(bits), "use": k.use_edge(bits), "bits": bits}),
            60,
        );
    }
    // the boss on the live state
    let feet: Vec<(i32, i32)> = s.seats.values().filter(|k| k.alive()).map(|k| k.body.foot()).collect();
    let seats = s.seats.len().max(1) as u8;
    let mut bo = s.boss.clone();
    super::oracle::step(&mut bo, &feet, seats, s.elapsed);
    classed(
        "bannerpeak.oracle.step",
        &format!("{:?}{}", s.boss.beat, s.boss.phase),
        10,
        400,
        json!({"boss": j(&s.boss), "feet": j(&feet), "seats": seats, "elapsed": s.elapsed}),
        j(&bo),
    );
}

fn follow_cam(k: &Knight, tick: usize) -> (i32, i32) {
    if tick % 3 == 0 {
        (0, 0)
    } else {
        k.cam
    }
}

fn boss_in(g: &mut Lcg, seats: u8) -> Boss {
    let mut b = Boss::new(seats);
    let beats = [
        Beat::Dormant,
        Beat::Rest,
        Beat::Tell,
        Beat::Strike,
        Beat::Wince,
        Beat::Rage,
        Beat::Down,
    ];
    b.beat = beats[g.range(0, 6) as usize];
    b.kind = [Attack::Stomp, Attack::Sweep, Attack::Embers][g.range(0, 2) as usize];
    b.phase = if b.beat == Beat::Down { 3 } else { g.range(0, 2) as u8 };
    b.t = g.range(0, 160) as u32;
    b.plates = g.range(0, seats as i64) as u8;
    b.count = g.range(0, 5) as u8;
    b.x = boss::home() + g.range(-110, 110) as i32;
    b.x_before = b.x;
    b.head = g.range(boss::HEAD_DOWN as i64, boss::HEAD_UP as i64) as i32;
    b.head_before = b.head;
    b.flinch = g.range(0, 14) as u32;
    for _ in 0..g.range(0, 3) {
        b.hazards.push(Hazard {
            x: g.range(6300, 6900) as i32,
            y: g.range(150, 340) as i32,
            w: g.range(14, 100) as i32,
            h: g.range(14, 200) as i32,
            vx: g.range(-3, 3) as i32,
            tell: g.range(0, 40) as u32,
            live: g.range(0, 70) as u32,
        });
    }
    b
}

#[test]
#[ignore]
fn record_bannerpeak() {
    // ------------------------------------------------ pure layout / world
    let mut g = Lcg(0xBA77_E12B_EAC1);
    for i in 0..200i32 {
        let deg = match i % 4 {
            0 => g.range(-400, 800) as i32,
            1 => g.range(0, 360) as i32,
            2 => i - 100,
            _ => g.range(-100_000, 100_000) as i32,
        };
        cases::rec("bannerpeak.layout.sine", &json!({"deg": deg}), &layout::sine(deg));
        cases::rec("bannerpeak.layout.cosine", &json!({"deg": deg}), &layout::cosine(deg));
        let period = 2 * g.range(1, 200) as u32;
        let (t, phase) = (g.range(0, 5000) as u32, g.range(0, 300) as u32);
        let (lo, hi) = (g.range(-500, 500) as i32, g.range(-500, 3000) as i32);
        cases::rec(
            "bannerpeak.layout.ping_pong",
            &json!({"t": t, "period": period, "phase": phase, "lo": lo, "hi": hi}),
            &layout::ping_pong(t, period, phase, lo, hi),
        );
        let t = if i % 5 == 0 { g.range(0, 100_000) as u32 } else { g.range(0, 3000) as u32 };
        cases::rec("bannerpeak.layout.movers", &json!({"t": t}), &movers_j(&layout::movers(t)));
        let x = g.range(-5, 440) as i32;
        cases::rec("bannerpeak.layout.section_of", &json!({"x": x}), &layout::section_of(x));
        let (cx, cy) = (g.range(-3, 440) as i32, g.range(-3, 70) as i32);
        cases::rec(
            "bannerpeak.layout.cell",
            &json!({"cx": cx, "cy": cy}),
            &json!({"centre": [layout::cell_centre((cx, cy)).0, layout::cell_centre((cx, cy)).1],
                    "feet": [layout::cell_feet((cx, cy)).0, layout::cell_feet((cx, cy)).1]}),
        );
        // hatches: both real ones and arbitrary tiles
        let tile = if i % 7 == 0 {
            layout::MARSH.at(44, 12)
        } else if i % 7 == 1 {
            layout::MARSH.at(56, 26)
        } else {
            (g.range(0, 434) as i32, g.range(0, 64) as i32)
        };
        let to = layout::hatch_to(tile);
        cases::rec(
            "bannerpeak.layout.hatch_to",
            &json!({"tx": tile.0, "ty": tile.1}),
            &json!({"some": to.is_some(), "x": to.map_or(0, |t| t.0), "y": to.map_or(0, |t| t.1)}),
        );
        // tiles
        let w = layout::world();
        let (tx, ty) = (g.range(-6, 440) as i32, g.range(-6, 70) as i32);
        cases::rec(
            "bannerpeak.world.World.at",
            &json!({"tx": tx, "ty": ty}),
            &w.at(tx, ty).code(),
        );
    }
    for b in 0..40u8 {
        let t = Tile::from_code(b);
        cases::rec(
            "bannerpeak.world.Tile.known",
            &json!({"b": b}),
            &t.is_some(),
        );
        if let Some(t) = t {
            cases::rec(
                "bannerpeak.world.Tile.props",
                &json!({"t": t.code()}),
                &json!({"solid": t.solid(), "one_way": t.one_way(), "rune": t.rune(), "slippery": t.slippery()}),
            );
        }
    }
    // the whole composed world and the lists info() reads
    let w = layout::world();
    let rows: Vec<Vec<u8>> = (0..64).map(|y| (0..434).map(|x| w.at(x, y).code()).collect()).collect();
    cases::rec("bannerpeak.layout.world", &json!({}), &j(&rows));
    let i = info();
    let pts = |v: &[(i32, i32)]| -> Value { j(&v.iter().map(|p| json!([p.0, p.1])).collect::<Vec<_>>()) };
    cases::rec(
        "bannerpeak.layout.info",
        &json!({}),
        &json!({
            "start": [i.start.0, i.start.1],
            "gates": pts(&i.gates),
            "sparks": pts(&i.sparks),
            "coins": pts(&i.coins),
            "banners": i.banners.iter().map(|b| b.map_or(json!(null), |p| json!([p.0, p.1]))).collect::<Vec<_>>(),
            "pole": [i.pole.0, i.pole.1],
            "great_pole": [i.great_pole.0, i.great_pole.1],
            "finale": [i.finale.0, i.finale.1],
            "arch": [i.arch.0, i.arch.1],
            "board": [i.board.0, i.board.1],
            "cinder": [i.cinder.0, i.cinder.1],
            "foes": i.foes.iter().map(|m| json!([m.k as u32, m.x, m.y])).collect::<Vec<_>>(),
            "hints": i.hints.iter().map(|m| json!([m.k as u32, m.x, m.y])).collect::<Vec<_>>(),
        }),
    );

    // ------------------------------------------------ boss, synthetic states
    let mut g = Lcg(0xC1_4DE2);
    for n in 0..320usize {
        let seats = 1 + (n % 6) as u8;
        let mut b = boss_in(&mut g, seats);
        let nfeet = g.range(0, 4) as usize;
        let feet: Vec<(i32, i32)> = (0..nfeet)
            .map(|_| {
                if g.chance(80) {
                    (g.range(6300, 6900) as i32, g.range(200, 360) as i32)
                } else {
                    (g.range(0, 6000) as i32, g.range(0, 800) as i32)
                }
            })
            .collect();
        let elapsed = g.range(0, 9000) as u32;
        // a few ticks in a row from the same start: the beats turn over
        for _ in 0..6 {
            let before = b.clone();
            super::oracle::step(&mut b, &feet, seats, elapsed);
            cases::rec_cap(
                "bannerpeak.oracle.step",
                &json!({"boss": j(&before), "feet": j(&feet), "seats": seats, "elapsed": elapsed}),
                &j(&b),
                1200,
            );
            if g.chance(30) {
                break;
            }
        }
        let mut b = boss_in(&mut g, seats);
        if g.chance(60) {
            b.beat = Beat::Wince;
            b.plates = g.range(1, seats as i64) as u8;
        }
        let c = b.crown();
        let (x, y) = match c {
            Some(c) if g.chance(70) => (b.x + g.range(-30, 30) as i32, c.3 + g.range(-5, 5) as i32),
            _ => (g.range(6000, 7000) as i32, g.range(100, 400) as i32),
        };
        let before = b.clone();
        let hit = super::oracle::slam(&mut b, x, y);
        cases::rec(
            "bannerpeak.oracle.slam",
            &json!({"boss": j(&before), "x": x, "y": y}),
            &json!({"boss": j(&b), "hit": hit}),
        );
        let b = boss_in(&mut g, seats);
        cases::rec("bannerpeak.boss.Boss.new", &json!({"seats": seats}), &j(&Boss::new(seats)));
        let sm = if n % 3 == 0 { 6 } else { seats };
        cases::rec(
            "bannerpeak.boss.Boss.valid",
            &json!({"boss": j(&b), "seats_max": sm}),
            &b.valid(sm),
        );
        let mut bad = b.clone();
        match n % 8 {
            0 => bad.x += 300,
            1 => bad.phase = 4,
            2 => bad.count = 9,
            3 => bad.head = 99,
            4 => bad.flinch = 30,
            5 => bad.t = 500,
            6 => bad.plates = 9,
            _ => bad.hazards.push(Hazard { x: 0, y: 0, w: 500, h: 1, vx: 0, tell: 0, live: 0 }),
        }
        cases::rec(
            "bannerpeak.boss.Boss.valid",
            &json!({"boss": j(&bad), "seats_max": 6}),
            &bad.valid(6),
        );
        cases::rec(
            "bannerpeak.boss.Boss.shape",
            &json!({"boss": j(&b)}),
            &json!({
                "defeated": b.defeated(),
                "body": rect_j(b.body()),
                "plank": j(&json!({"x": b.plank(false).x, "y": b.plank(false).y, "w": b.plank(false).w, "lift": b.plank(false).lift})),
                "plank_before": j(&json!({"x": b.plank(true).x, "y": b.plank(true).y, "w": b.plank(true).w, "lift": b.plank(true).lift})),
                "crown": b.crown().map_or(json!(null), rect_j),
            }),
        );
        let was = g.range(0, 5) as u8;
        let now = g.range(0, 6) as u8;
        let mut bs = b.clone();
        bs.seated(was, now);
        cases::rec(
            "bannerpeak.boss.Boss.seated",
            &json!({"boss": j(&b), "was": was, "now": now}),
            &j(&bs),
        );
    }
    cases::rec("bannerpeak.boss.floor", &json!({}), &boss::floor());
    cases::rec("bannerpeak.boss.home", &json!({}), &boss::home());
    for p in 0..3u8 {
        cases::rec(
            "bannerpeak.oracle.pattern",
            &json!({"phase": p}),
            &j(&super::oracle::pattern(p).iter().map(|a| format!("{a:?}")).collect::<Vec<_>>()),
        );
    }

    // ------------------------------------------------ the games, sampled
    let mut probe = |sm: &Sample| probe_all(sm);
    // whole runs by the bots at several table sizes
    for (seats, seed) in [(1u32, 1u64), (2, 2), (3, 3), (6, 6), (4, 4)] {
        let mut t = Table::new(seats);
        run(&mut t, 14000, seed, Policy::Bots, if seats == 1 { 23 } else { 41 }, &mut probe);
    }
    // each stage: bots from its gate
    for gate in 0..8usize {
        for (seats, seed) in [(1u32, 10 + gate as u64), (4, 40 + gate as u64)] {
            let (banners, sparks) = STAGES[gate];
            let mut t = stage(seats, banners, sparks, gate);
            run(&mut t, 1800, seed, Policy::Bots, 37, &mut probe);
        }
    }
    // random keys, with the bots lending a hand
    for gate in 0..8usize {
        for (seats, seed, policy) in [
            (1u32, 100 + gate as u64, Policy::Fuzz),
            (3, 200 + gate as u64, Policy::BotsFuzz),
            (2, 300 + gate as u64, Policy::Fuzz),
        ] {
            let (banners, sparks) = STAGES[gate];
            let mut t = stage(seats, banners, sparks, gate);
            run(&mut t, 900, seed, policy, 13, &mut probe);
        }
    }
    // a late knight sits down, one steps away
    let mut t = stage(2, 0b11, 0, 1);
    run(&mut t, 200, 77, Policy::Bots, 19, &mut probe);
    let hero = t.heroes[&2].clone();
    t.heroes.insert(3, hero.clone());
    run(&mut t, 200, 78, Policy::Fuzz, 7, &mut probe);
    t.heroes.get_mut(&1).unwrap().away = true;
    run(&mut t, 100, 79, Policy::Fuzz, 7, &mut probe);
    t.heroes.get_mut(&1).unwrap().away = false;
    t.heroes.get_mut(&2).unwrap().stone = true;
    run(&mut t, 100, 80, Policy::Fuzz, 7, &mut probe);

    // ------------------------------------------------ the Run's hooks
    record_hooks();
}

fn run_json(r: &Run) -> Value {
    super::super::runtime_state::capture_for_rules(r).unwrap().run
}

fn record_hooks() {
    for seats in 1..=6u32 {
        let mut r = Run::bannerpeak_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.step(&Map::new());
        for round in 0..5usize {
            // showing / valid / seated / bot / knobs
            cases::rec_cap(
                "bannerpeak.Run.bannerpeak_showing",
                &json!({"run": run_json(&r)}),
                &r.bannerpeak_showing(),
                14,
            );
            cases::rec_cap(
                "bannerpeak.ADVENTURE.valid",
                &json!({"run": run_json(&r)}),
                &(ADVENTURE.valid)(&r),
                14,
            );
            cases::rec_cap(
                "bannerpeak.ADVENTURE.seated",
                &json!({"run": run_json(&r)}),
                &(ADVENTURE.seated)(&r),
                14,
            );
            for id in 1..=seats.min(2) {
                cases::rec_cap(
                    "bannerpeak.ADVENTURE.bot",
                    &json!({"run": run_json(&r), "id": id}),
                    &j(&(ADVENTURE.bot)(&r, id)),
                    14,
                );
            }
            cases::rec_cap(
                "bannerpeak.Run.bannerpeak_knobs",
                &json!({"run": run_json(&r)}),
                &knobs_j(&r.bannerpeak_knobs()),
                10,
            );
            // tick_bannerpeak under the bots' keys, then a mid-run state each round
            let inputs = r.roundtable_inputs();
            let before = run_json(&r);
            let mut r2 = r.clone();
            r2.tick_bannerpeak(&inputs);
            cases::rec_cap(
                "bannerpeak.Run.tick_bannerpeak",
                &json!({"run": before, "inputs": j(&inputs)}),
                &run_json(&r2),
                40,
            );
            let before = run_json(&r);
            let mut r3 = r.clone();
            let rode = (ACTIVITY.ride.unwrap())(&mut r3, &inputs);
            cases::rec_cap(
                "bannerpeak.ACTIVITY.ride",
                &json!({"run": before, "inputs": j(&inputs)}),
                &json!({"run": run_json(&r3), "some": rode.is_some()}),
                20,
            );
            for _ in 0..(40 + 90 * round) {
                let inputs = r.roundtable_inputs();
                r.step(&inputs);
            }
        }
        // start: clear the session, start again
        let mut r = Run::bannerpeak_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.bannerpeak = None;
        let before = run_json(&r);
        r.start_bannerpeak();
        cases::rec(
            "bannerpeak.Run.start_bannerpeak",
            &json!({"run": before}),
            &run_json(&r),
        );
        // clear and end
        let before = run_json(&r);
        let mut r4 = r.clone();
        (ADVENTURE.clear)(&mut r4);
        cases::rec("bannerpeak.ADVENTURE.clear", &json!({"run": before}), &run_json(&r4));
        let mut r5 = r.clone();
        (ACTIVITY.end)(&mut r5);
        cases::rec("bannerpeak.ACTIVITY.end", &json!({"run": before}), &run_json(&r5));
        // the pay at the finale, first and later times
        for (n, first) in [true, false].into_iter().enumerate() {
            let mut r = Run::bannerpeak_for_test();
            for id in 2..=seats {
                r.join_seat(id, &format!("Knight {id}"));
            }
            if !first {
                r.home.roundtable[SLOT] |= BEGUN;
            }
            let mut s = r.bannerpeak.clone().unwrap();
            s.elapsed = 600 + 1000 * (n as u32 + seats) * 3;
            s.completed = Some(s.elapsed);
            let before = run_json(&r);
            let mut r6 = r.clone();
            r6.bannerpeak_pays(&s);
            cases::rec(
                "bannerpeak.Run.bannerpeak_pays",
                &json!({"run": before, "session": j(&s)}),
                &run_json(&r6),
            );
        }
        // a finished table: E takes the knights back (needs the exits group)
        let mut r = Run::bannerpeak_for_test();
        for id in 2..=seats {
            r.join_seat(id, &format!("Knight {id}"));
        }
        let mut s = r.bannerpeak.take().unwrap();
        s.banners = 0xFF;
        s.sparks = 0xFF;
        s.phase = Phase::Finale;
        s.phase_t = super::session::FINALE_TICKS - 2;
        r.bannerpeak = Some(s);
        for _ in 0..3 {
            r.step(&Map::new());
        }
        let press = Input { interact: true, ..Default::default() };
        let inputs: Map<u32, Input> = (1..=seats).map(|id| (id, press)).collect();
        let before = run_json(&r);
        let mut r7 = r.clone();
        r7.tick_bannerpeak(&inputs);
        cases::rec(
            "bannerpeak.Run.tick_bannerpeak.done",
            &json!({"run": before, "inputs": j(&inputs)}),
            &run_json(&r7),
        );
    }
}
