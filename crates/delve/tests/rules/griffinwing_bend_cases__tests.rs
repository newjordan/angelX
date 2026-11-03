//! Griffinwing's Bend-port case recorder (feature `bend-cases-games-a-griffinwing`):
//! bots and a few scripted mistakes fly whole adventures at one to six seats,
//! and the functions of the game are recorded before and after on the
//! sessions they pass through. Run:
//! `cl.sh crates test -p angel-delve --profile fast --features bend-cases-games-a-griffinwing --lib griffinwing_bend -- --ignored`
use super::bend_kit::{self as kit, j, rec, want, want_with, Rng};
use super::bot::Skill;
use super::*;
use serde_json::{json, Value};

/// One run's plan.
struct Plan {
    seats: u32,
    /// Seats flown by the novice's hands (a bit each, from seat 1).
    novice: u32,
    /// 0: bots only; 1: some random presses; 2: random bursts for everyone.
    chaos: u8,
    nums: Nums,
    /// A late knight sits down at 400, one stands up at 1600 and returns at 2200.
    churn: bool,
    limit: u32,
}

fn plans() -> Vec<Plan> {
    let d = Nums::default();
    let mut v = Vec::new();
    for seats in 1..=6 {
        v.push(Plan { seats, novice: 0, chaos: 0, nums: d, churn: false, limit: 12_000 });
    }
    for seats in [1, 2, 3, 4, 6] {
        v.push(Plan { seats, novice: 0b101010, chaos: 0, nums: d, churn: false, limit: 12_000 });
    }
    for seats in [1, 2, 3, 5] {
        v.push(Plan { seats, novice: 0b11, chaos: 1, nums: d, churn: false, limit: 12_000 });
    }
    for seats in [1, 2, 4, 6] {
        v.push(Plan { seats, novice: 0b1111, chaos: 2, nums: d, churn: false, limit: 9_000 });
    }
    v.push(Plan { seats: 2, novice: 0, chaos: 0, nums: d, churn: true, limit: 12_000 });
    v.push(Plan { seats: 3, novice: 0b10, chaos: 1, nums: d, churn: true, limit: 12_000 });
    let odd = Nums { charge: 25, roll: 55, window: 85, spur: 150, tell: 90 };
    v.push(Plan { seats: 2, novice: 0, chaos: 0, nums: odd, churn: false, limit: 12_000 });
    v.push(Plan { seats: 4, novice: 0b11, chaos: 1, nums: odd, churn: false, limit: 12_000 });
    let odd2 = Nums { charge: 85, roll: 20, window: 42, spur: 35, tell: 45 };
    v.push(Plan { seats: 3, novice: 0, chaos: 0, nums: odd2, churn: false, limit: 12_000 });
    v.push(Plan { seats: 5, novice: 0b10101, chaos: 1, nums: odd2, churn: false, limit: 12_000 });
    v
}

fn random_input(rng: &mut Rng) -> Input {
    let ax = |rng: &mut Rng| rng.range(-1, 1) as i8;
    Input {
        move_x: ax(rng),
        move_y: ax(rng),
        aim_x: ax(rng),
        aim_y: ax(rng),
        fire: rng.chance(2),
        dash: rng.chance(6),
        bomb: rng.chance(40),
        swing: rng.chance(6),
        interact: rng.chance(4),
        ..Default::default()
    }
}

#[test]
#[ignore = "records the Bend port's cases"]
fn record_griffinwing_bend_cases() {
    for (ri, p) in plans().iter().enumerate() {
        play(ri + 1, p);
    }
    fixed_cases();
    for name in NAMES {
        eprintln!("{name}: {}", kit::cases::count(name));
    }
}

/// Every function the recorders write, for the count at the end.
const NAMES: &[&str] = &[
    "griffinwing.types.norm",
    "griffinwing.types.scale3",
    "griffinwing.types.tri",
    "griffinwing.types.near",
    "griffinwing.types.flat",
    "griffinwing.types.Foe.alive",
    "griffinwing.types.Foe.lockable",
    "griffinwing.types.Craft.new",
    "griffinwing.types.Craft.valid",
    "griffinwing.flight.Craft.reflecting",
    "griffinwing.types.Beast.valid",
    "griffinwing.types.Tally.valid",
    "griffinwing.types.Tally.bank",
    "griffinwing.things.shutter_gap",
    "griffinwing.flight.bits",
    "griffinwing.flight.slot",
    "griffinwing.flight.rider_slot",
    "griffinwing.bit",
    "griffinwing.Nums.of",
    "griffinwing.Nums.wx",
    "griffinwing.Nums.wz",
    "griffinwing.Nums.reflect",
    "griffinwing.Session.new",
    "griffinwing.Session.sit",
    "griffinwing.Session.anchor",
    "griffinwing.Session.pressure",
    "griffinwing.Session.next_id",
    "griffinwing.Session.sound",
    "griffinwing.Session.say",
    "griffinwing.Session.spark",
    "griffinwing.Session.voice",
    "griffinwing.Session.fill_wing",
    "griffinwing.Session.muster",
    "griffinwing.Session.step",
    "griffinwing.Session.tick_lists",
    "griffinwing.Session.end_beat",
    "griffinwing.Session.next_beat",
    "griffinwing.Session.restart",
    "griffinwing.Session.finish",
    "griffinwing.Session.medal_now",
    "griffinwing.Session.purse",
    "griffinwing.Session.finished",
    "griffinwing.Session.permille",
    "griffinwing.Session.valid",
    "griffinwing.Session.repair",
    "griffinwing.flight.Session.craft_s",
    "griffinwing.flight.Session.craft_at",
    "griffinwing.flight.Session.fly",
    "griffinwing.flight.Session.wing_fly",
    "griffinwing.flight.Session.step_craft",
    "griffinwing.flight.Session.push_bolt",
    "griffinwing.combat.Session.spawn_foe",
    "griffinwing.combat.Session.hurt_craft",
    "griffinwing.combat.Session.latched",
    "griffinwing.combat.Session.target",
    "griffinwing.combat.Session.plane_shot",
    "griffinwing.combat.Session.step_foes",
    "griffinwing.combat.Session.marks",
    "griffinwing.combat.Session.hit_foe",
    "griffinwing.combat.Session.starfall",
    "griffinwing.combat.Session.step_bolts",
    "griffinwing.boss.Session.rise",
    "griffinwing.boss.Session.step_boss",
    "griffinwing.script.resume",
    "griffinwing.script.script",
    "griffinwing.script.Session.run_script",
    "griffinwing.script.Session.act",
    "griffinwing.script.Session.victory_gates",
    "griffinwing.things.Session.step_props",
    "griffinwing.bot.Session.bot_with",
    "griffinwing.bot.Session.bot_input",
];

/// The tick's inputs: bots (sure or novice hands), some scripted mistakes.
fn inputs_for(
    s: &Session,
    heroes: &BTreeMap<u32, Hero>,
    p: &Plan,
    tick: u32,
    rng: &mut Rng,
) -> BTreeMap<u32, Input> {
    let mut m = BTreeMap::new();
    for (&id, h) in heroes {
        if h.away {
            continue;
        }
        let skill = if p.novice >> (id - 1) & 1 == 1 { Skill::Novice } else { Skill::Sure };
        let mut i = s.bot_with(id, &p.nums, skill);
        match p.chaos {
            1 if (tick / 240 + id) % 3 == 0 => i = random_input(rng),
            2 if (tick / 90 + id) % 2 == 0 => i = random_input(rng),
            _ => {}
        }
        m.insert(id, i);
    }
    m
}

fn play(ri: usize, p: &Plan) {
    let mut r = Run::griffinwing_for_test();
    let late = if p.churn && p.seats < MAX_PLAYERS { p.seats + 1 } else { 0 };
    for id in 2..=p.seats.max(late) {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let mut heroes = r.players.clone();
    // The late knight sits out until 400.
    if let Some(h) = heroes.get_mut(&late) {
        h.away = true;
    }
    let mut s = Session::new(&heroes);
    let mut rng = Rng::new(ri as u64, 11);
    let mut tally_ticks = 0;
    for tick in 0..p.limit {
        kit::at(ri, tick);
        if p.churn {
            if tick == 400 {
                heroes.get_mut(&late).unwrap().away = false;
            }
            if tick == 1600 {
                heroes.get_mut(&1).unwrap().away = true;
            }
            if tick == 2200 {
                heroes.get_mut(&1).unwrap().away = false;
            }
        }
        let inputs = inputs_for(&s, &heroes, p, tick, &mut rng);
        sample(&s, &inputs, &heroes, &p.nums, tick);
        s.step(&inputs, &heroes, &p.nums);
        if s.finished() {
            tally_ticks += 1;
            if tally_ticks > 200 {
                break;
            }
        }
    }
}

/// The pure arithmetic, and the odd corners no flight reaches.
fn fixed_cases() {
    kit::at(0, 0);
    script::bend_script_tests::tables();
    bot::bend_bot_tests::noise_cases();
    let mut rng = Rng::new(77, 3);
    // norm / flat over the ranges the sim lives in, and past them.
    for k in 0..80 {
        let span = match k % 4 {
            0 => 300,
            1 => 6000,
            2 => 40_000,
            _ => 3_000_000,
        };
        let d = (rng.range(-span, span), rng.range(-span, span), rng.range(-span, span));
        rec("griffinwing.types.norm", v3j(d), json!(norm(d)));
        let (ax, az, bx, bz) = (rng.range(-span, span), rng.range(-span, span), rng.range(-span, span), rng.range(-span, span));
        rec("griffinwing.types.flat", json!([ax, az, bx, bz]), json!(flat(ax, az, bx, bz)));
        let len = rng.range(-700, 700);
        rec("griffinwing.types.scale3", json!({"d": v3j(d), "len": len}), v3j(scale3(d, len)));
    }
    rec("griffinwing.types.norm", v3j((0, 0, 0)), json!(0));
    rec("griffinwing.types.scale3", json!({"d": v3j((0, 0, 0)), "len": 17}), v3j(scale3((0, 0, 0), 17)));
    for k in 0..80 {
        let t = rng.below(5000) as u32;
        let period = [2u32, 3, 60, 96, 108, 120, 130, 146, 150, 260, 1, 0][k % 12];
        let amp = [dm(16), dm(6), dm(24), dm(30), dm(18), 1000, 7, -dm(10)][k % 8];
        rec("griffinwing.types.tri", json!({"t": t, "period": period, "amp": amp}), json!(tri(t, period, amp)));
    }
    // swept tests: through, beside, past, a point, and the real thing's magnitudes.
    for k in 0..120 {
        let reach = [60, 200, 2500, 12000][k % 4];
        let a = (rng.range(-reach, reach), rng.range(-reach, reach), rng.range(-reach, reach));
        let b = if k % 9 == 0 { a } else { (a.0 + rng.range(-1200, 1200), a.1 + rng.range(-600, 600), a.2 + rng.range(-600, 600)) };
        let p = if k % 3 == 0 {
            // a point near the segment
            (a.0 + (b.0 - a.0) / 2 + rng.range(-90, 90), a.1 + (b.1 - a.1) / 2 + rng.range(-90, 90), a.2 + (b.2 - a.2) / 2 + rng.range(-90, 90))
        } else {
            (rng.range(-reach, reach), rng.range(-reach, reach), rng.range(-reach, reach))
        };
        let r = [dm(9), dm(5), dm(14), dm(21), 100, 3, 0][k % 7];
        rec("griffinwing.types.near", json!({"a": v3j(a), "b": v3j(b), "p": v3j(p), "r": r}), json!(near(a, b, p, r)));
    }
    // the table
    for id in 1..=8u32 {
        rec("griffinwing.flight.slot", json!(id), json!(flight::slot(id)));
        rec("griffinwing.bit", json!(id), json!(bit(id)));
    }
    rec("griffinwing.bit", json!(0), json!(bit(0)));
    rec("griffinwing.bit", json!(9), json!(bit(9)));
    for k in 0..4usize {
        rec("griffinwing.flight.rider_slot", json!(k), json!(flight::rider_slot(k)));
    }
    // inputs to bits
    for k in 0..40 {
        let mut rng2 = Rng::new(k, 5);
        let i = random_input(&mut rng2);
        rec("griffinwing.flight.bits", j(&i), json!(flight::bits(i)));
    }
    // the five numbers
    for k in 0..60u32 {
        let mut tune = tune::Tune::default();
        for i in 0..5 {
            tune.roundtable[SLOT][i] = (k * 37 + i as u32 * 11) % 160;
        }
        let n = Nums::of(&tune);
        rec("griffinwing.Nums.of", json!({"tune": {"roundtable": tune.roundtable}}), kit::nums_j(&n));
        rec("griffinwing.Nums.wx", kit::nums_j(&n), json!(n.wx()));
        rec("griffinwing.Nums.wz", kit::nums_j(&n), json!(n.wz()));
        rec("griffinwing.Nums.reflect", kit::nums_j(&n), json!(n.reflect()));
    }
    let n0 = Nums::default();
    rec("griffinwing.Nums.of", json!({"tune": {"roundtable": tune::Tune::default().roundtable}}), kit::nums_j(&n0));
    // shutters
    for k in 0..60u32 {
        let p = Prop { thing: Thing::Shutter, s: 0, x: 0, z: 0, w: 0, h: 0, len: dm(30), taken: 0, phase: (k * 13 % 150) as u16 };
        let t = k * 31;
        let wx = Nums::default().wx();
        rec("griffinwing.things.shutter_gap", json!({"p": j(&p), "t": t, "wx": wx}), json!(things::shutter_gap(&p, t, wx)));
    }
    // the tally and the beast and the craft, whole and broken
    for k in 0..60u32 {
        let mut t = Tally { pts: k * 997, base: k * 311, offered: k * 677, kills: k, saved: (k % 7) as u16, lost: (k % 5) as u16, gates: (k % 11) as u16 };
        rec("griffinwing.types.Tally.valid", j(&t), json!(t.valid()));
        if k % 3 == 0 {
            t.pts = 1_000_000 + k;
            rec("griffinwing.types.Tally.valid", j(&t), json!(t.valid()));
        }
        let u = Tally { pts: k * 11, base: k * 3, offered: k * 5, kills: k % 9, saved: (k % 4) as u16, lost: 1, gates: (k % 6) as u16 };
        let mut sum = t;
        sum.bank(&u);
        rec("griffinwing.types.Tally.bank", json!({"a": j(&t), "b": j(&u)}), j(&sum));
    }
    for (x, z) in [(0, 0), (dm(22), dm(-4)), (-dm(40), -dm(8)), (7, -9), (dm(160), dm(120)), (-3, 0)] {
        rec("griffinwing.types.Craft.new", json!({"x": x, "z": z}), j(&Craft::new(x, z)));
    }
}

fn v3j(t: (i32, i32, i32)) -> Value {
    json!([t.0, t.1, t.2])
}

/// Everything recorded on a state the flight passes through.
fn sample(s: &Session, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>, n: &Nums, tick: u32) {
    let (ri, _) = kit::clock();
    let mut rng = Rng::new(ri as u64 * 1_000_003, tick as u64 + 1);
    types_cases(s, &mut rng);
    session_cases(s, inputs, heroes, n, &mut rng);
    flight::bend_flight_tests::sample(s, n, &mut rng);
    combat::bend_combat_tests::sample(s, n, &mut rng);
    boss::bend_boss_tests::sample(s, n, &mut rng);
    script::bend_script_tests::sample(s, n, &mut rng);
    things::bend_things_tests::sample(s, n, &mut rng);
    bot::bend_bot_tests::sample(s, n, &mut rng);
}

/// The types' readers and checks over the live session.
fn types_cases(s: &Session, rng: &mut Rng) {
    if want("griffinwing.types.Foe.alive") {
        for f in s.foes.iter().take(3) {
            rec("griffinwing.types.Foe.alive", j(f), json!(f.alive()));
            rec("griffinwing.types.Foe.lockable", j(f), json!(f.lockable()));
        }
        // each state of each kind
        for (k, st) in [St::Fly, St::Tell, St::Rest, St::Hunt, St::Latched, St::Flee, St::Hide, St::Dive, St::Shut, St::Open, St::Broken].iter().enumerate() {
            if let Some(f) = s.foes.get(k % s.foes.len().max(1)) {
                let mut f = f.clone();
                f.st = *st;
                if k % 4 == 3 {
                    f.hp = 0;
                }
                rec("griffinwing.types.Foe.alive", j(&f), json!(f.alive()));
                rec("griffinwing.types.Foe.lockable", j(&f), json!(f.lockable()));
            }
        }
    }
    if want("griffinwing.types.Craft.valid") {
        for (_, seat) in s.seats.iter() {
            rec("griffinwing.types.Craft.valid", j(&seat.craft), json!(seat.craft.valid()));
            let mut c = seat.craft.clone();
            match rng.below(14) {
                0 => c.x = dm(170),
                1 => c.shield = -1,
                2 => c.dir = 0,
                3 => c.hurt = 601,
                4 => c.locks = (0..7).map(|k| Lock { foe: k, n: 1 }).collect(),
                5 => c.meter = METER + 1,
                6 => c.bombs = 10,
                7 => c.ds = -1,
                8 => c.last_x = 2,
                9 => c.charge = 241,
                10 => c.back = -dm(81),
                11 => c.held = 36_001,
                12 => c.roll = 121,
                _ => c.chain_ticks = 601,
            }
            rec("griffinwing.types.Craft.valid", j(&c), json!(c.valid()));
        }
        for w in &s.wing {
            rec("griffinwing.types.Craft.valid", j(&w.craft), json!(w.craft.valid()));
            let c = w.craft.clone();
            rec("griffinwing.flight.Craft.reflecting", json!({"c": j(&c), "reflect": 20}), json!(c.reflecting(20)));
        }
    }
    if want("griffinwing.flight.Craft.reflecting") {
        for (_, seat) in s.seats.iter() {
            for reflect in [8u32, 20, 30] {
                let mut c = seat.craft.clone();
                if rng.chance(2) {
                    c.roll_len = 30;
                    c.roll = rng.below(31) as u32;
                }
                rec("griffinwing.flight.Craft.reflecting", json!({"c": j(&c), "reflect": reflect}), json!(c.reflecting(reflect)));
            }
        }
    }
    if let Some(b) = &s.boss {
        if want("griffinwing.types.Beast.valid") {
            rec("griffinwing.types.Beast.valid", j(b), json!(b.valid()));
            let mut c = b.clone();
            match rng.below(8) {
                0 => c.phase = 3,
                1 => c.atk = 3,
                2 => c.ticks = 3001,
                3 => c.x = dm(201),
                4 => c.marks = vec![(0, 0); 9],
                5 => c.marks = vec![(dm(301), 0)],
                6 => c.side = 2,
                _ => c.col = -dm(201),
            }
            rec("griffinwing.types.Beast.valid", j(&c), json!(c.valid()));
        }
    }
}

/// The session's own readers and mutators.
fn session_cases(s: &Session, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>, n: &Nums, rng: &mut Rng) {
    if want("griffinwing.Session.anchor") {
        rec("griffinwing.Session.anchor", kit::sj_keys(s, &["stage", "t"]), json!(s.anchor()));
        rec("griffinwing.Session.pressure", kit::sj_keys(s, &["seats"]), json!(s.pressure()));
        rec("griffinwing.Session.voice", kit::sj_keys(s, &["wing"]), json!(s.voice()));
        for (_, seat) in s.seats.iter().take(2) {
            rec(
                "griffinwing.flight.Session.craft_s",
                json!({"s": kit::sj_keys(s, &["stage", "t"]), "c": j(&seat.craft)}),
                json!(s.craft_s(&seat.craft)),
            );
        }
        for id in 0..=11u8 {
            let at = s.craft_at(id).map(|(a, b, c, d)| json!([a, b, c, d])).unwrap_or(Value::Null);
            rec("griffinwing.flight.Session.craft_at", json!({"s": kit::sj_keys(s, &["stage", "t", "seats", "wing"]), "id": id}), at);
        }
    }
    if want("griffinwing.Session.medal_now") {
        rec("griffinwing.Session.medal_now", kit::sj_keys(s, &["bank", "retries"]), j(&s.medal_now()));
        rec("griffinwing.Session.purse", kit::sj_keys(s, &["first", "medal"]), json!(s.purse()));
        rec("griffinwing.Session.finished", kit::sj_keys(s, &["mode"]), json!(s.finished()));
        rec("griffinwing.Session.permille", kit::sj_keys(s, &["bank"]), json!(s.permille()));
        for k in 0..6u32 {
            let mut c = s.clone();
            c.bank.base = rng.below(900) as u32 * 10;
            c.bank.offered = if k == 0 { 0 } else { rng.below(1000) as u32 * 10 };
            c.bank.lost = rng.below(9) as u16;
            c.retries = rng.below(7) as u8;
            c.medal = [None, Some(Medal::Bronze), Some(Medal::Silver), Some(Medal::Gold)][(k % 4) as usize];
            c.first = k % 2 == 0;
            c.mode = [Mode::Play, Mode::Tally, Mode::Retry, Mode::Victory][(k % 4) as usize];
            rec("griffinwing.Session.medal_now", kit::sj_keys(&c, &["bank", "retries"]), j(&c.medal_now()));
            rec("griffinwing.Session.purse", kit::sj_keys(&c, &["first", "medal"]), json!(c.purse()));
            rec("griffinwing.Session.finished", kit::sj_keys(&c, &["mode"]), json!(c.finished()));
            rec("griffinwing.Session.permille", kit::sj_keys(&c, &["bank"]), json!(c.permille()));
        }
    }
    if want("griffinwing.Session.valid") {
        rec("griffinwing.Session.valid", kit::sj(s), json!(s.valid()));
        for k in 0..6 {
            let mut c = s.clone();
            match (k + rng.below(8) as u32) % 12 {
                0 => c.elapsed = LIMIT + 1,
                1 => c.stage = 3,
                2 => c.cp = c.t.max(rail::BOSS_AT) + 1,
                3 => c.paid = 64,
                4 => c.home = -1,
                5 => {
                    if let Some(f) = c.foes.first_mut() {
                        f.hp = f.full + 1;
                    } else {
                        c.fx.push(Flare { what: Burst::Hit, s: 50_000_000, x: 0, z: 0, ticks: 5, n: 0 });
                    }
                }
                6 => {
                    if let Some(b) = c.bolts.first_mut() {
                        b.life = 601;
                    } else {
                        c.calls.push(Call { who: 11, line: Line::Hello, ticks: 5 });
                    }
                }
                7 => {
                    if let Some(p) = c.props.first_mut() {
                        p.taken = 64;
                    } else {
                        c.done = Some(c.elapsed + 1);
                    }
                }
                8 => {
                    if let Some(w) = c.wing.first_mut() {
                        w.id = 3;
                    } else {
                        c.bank.pts = 2_000_000;
                    }
                }
                9 => {
                    if let Some(seat) = c.seats.values_mut().next() {
                        seat.craft.shield = 200;
                    }
                }
                10 => {
                    if let Some(f) = c.foes.first_mut() {
                        f.vx = 5 * Q;
                    } else {
                        c.mode_t = LIMIT + 1;
                    }
                }
                _ => c.t = LIMIT + 1,
            }
            rec("griffinwing.Session.valid", kit::sj(&c), json!(c.valid()));
            let mut r = c.clone();
            r.repair();
            rec("griffinwing.Session.repair", kit::sj(&c), kit::sj(&r));
        }
    }
    if want("griffinwing.Session.sound") {
        for name in ["gold", "roll", "gold", "kill", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m"] {
            let mut c = s.clone();
            if rng.chance(2) {
                for q in 0..rng.below(13) {
                    c.snd.push(["gold", "roll", "kill", "hero_hurt", "clang"][(q % 5) as usize]);
                }
            }
            let before = kit::sj(&c);
            c.sound(Box::leak(name.to_string().into_boxed_str()));
            rec("griffinwing.Session.sound", json!({"s": before, "name": name}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.say") {
        let lines = [Line::Hello, Line::Hold, Line::Ring, Line::Latched, Line::Thanks, Line::Lost, Line::Join, Line::Low, Line::Chain, Line::Home];
        for k in 0..8 {
            let mut c = s.clone();
            for q in 0..rng.below(6) {
                c.calls.push(Call { who: (q % 10) as u8 + 1, line: lines[rng.below(10) as usize], ticks: rng.below(101) as u8 });
            }
            c.calls.truncate(MAX_CALLS);
            let (who, line) = ((rng.below(10) + 1) as u8, lines[(k + rng.below(10) as usize) % 10]);
            let before = kit::sj(&c);
            c.say(who, line);
            rec("griffinwing.Session.say", json!({"s": before, "who": who, "line": j(&line)}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.spark") {
        let burst = [Burst::Pop, Burst::Hit, Burst::Turn, Burst::Lock, Burst::Gate, Burst::Scrape, Burst::Fall, Burst::Hurt, Burst::Lamp, Burst::Starfall];
        for k in 0..8 {
            let mut c = s.clone();
            if rng.chance(2) {
                for q in 0..(MAX_FX as u32 + 2) {
                    c.fx.push(Flare { what: burst[(q % 10) as usize], s: q as i32, x: 1, z: 2, ticks: 9, n: 0 });
                }
                c.fx.truncate(MAX_FX);
            }
            let (b, a, x, z, id) = (burst[(k + rng.below(10) as usize) % 10], rng.range(-500, 90_000), rng.range(-900, 900), rng.range(-900, 900), rng.below(8) as u8);
            let before = kit::sj(&c);
            c.spark(b, a, x, z, id);
            rec("griffinwing.Session.spark", json!({"s": before, "what": j(&b), "at": [a, x, z], "n": id}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.next_id") {
        for k in 0..6 {
            let mut c = s.clone();
            c.ids = [1u16, 59_999, 60_000, 60_001, 7, 100][k];
            let before = kit::sj(&c);
            let id = c.next_id();
            rec("griffinwing.Session.next_id", json!({"s": before}), json!({"s": kit::sj(&c), "r": id}));
        }
    }
    if want("griffinwing.Session.fill_wing") {
        for k in 0..4 {
            let mut c = s.clone();
            if k == 1 {
                c.wing.clear();
            }
            if k == 2 {
                while c.wing.len() < 4 {
                    let id = 7 + c.wing.len() as u8;
                    c.wing.push(Wingman { id, craft: Craft::new(3, 4), lost: false, fire: 1 });
                }
            }
            if k == 3 {
                c.wing.truncate(1);
            }
            let before = kit::sj(&c);
            c.fill_wing();
            rec("griffinwing.Session.fill_wing", json!({"s": before}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.new") {
        rec("griffinwing.Session.new", json!({"players": kit::heroes_j(heroes)}), kit::sj(&Session::new(heroes)));
        let mut h2 = heroes.clone();
        for (k, h) in h2.values_mut().enumerate() {
            h.away = k % 3 == 1;
            h.stone = k % 4 == 2;
        }
        rec("griffinwing.Session.new", json!({"players": kit::heroes_j(&h2)}), kit::sj(&Session::new(&h2)));
    }
    if want("griffinwing.Session.sit") {
        for k in 0..4u32 {
            let mut c = s.clone();
            let id = [1, 3, 6, 9, 0][(k + rng.below(5) as u32) as usize % 5];
            let joined = k * 100;
            let before = kit::sj(&c);
            c.sit(id, joined);
            rec("griffinwing.Session.sit", json!({"s": before, "id": id, "joined": joined}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.muster") {
        let mut c = s.clone();
        let mut hs = heroes.clone();
        if rng.chance(2) {
            if let Some(h) = hs.values_mut().next() {
                h.away = true;
            }
        }
        let before = kit::sj(&c);
        c.muster(&hs);
        rec("griffinwing.Session.muster", json!({"s": before, "heroes": kit::heroes_j(&hs)}), json!({"s": kit::sj(&c)}));
    }
    if want("griffinwing.Session.tick_lists") {
        let mut c = s.clone();
        let before = kit::sj(&c);
        c.tick_lists();
        rec("griffinwing.Session.tick_lists", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
    if want_with("griffinwing.Session.step", 6, 100) {
        let mut c = s.clone();
        let before = kit::sj(&c);
        let out = c.step(inputs, heroes, n);
        rec(
            "griffinwing.Session.step",
            json!({"s": before, "inputs": kit::inputs_j(inputs), "heroes": kit::heroes_j(heroes), "n": kit::nums_j(n)}),
            json!({"s": kit::sj(&c), "out": {"sounds": out.sounds, "leave": out.leave}}),
        );
    }
    // the phases of a tick, on the state the tick passes through
    phases(s, inputs, heroes, n);
    // the transitions, on states made to need them
    transitions(s, rng);
    // what the bots press
    for (&id, _) in s.seats.iter() {
        for skill in [Skill::Sure, Skill::Novice] {
            if want_with("griffinwing.bot.Session.bot_with", 6, 90) {
                let i = s.bot_with(id, n, skill);
                rec(
                    "griffinwing.bot.Session.bot_with",
                    json!({"s": kit::sj(s), "id": id, "n": kit::nums_j(n), "novice": skill == Skill::Novice}),
                    j(&i),
                );
            }
        }
    }
    if want("griffinwing.bot.Session.bot_input") {
        if let Some((&id, _)) = s.seats.iter().next() {
            let hero = heroes.get(&id).cloned();
            if let Some(hero) = hero {
                rec("griffinwing.bot.Session.bot_input", json!({"s": kit::sj(s), "id": id}), j(&s.bot_input(id, &hero)));
            }
        }
    }
}

/// Phases of `Session::step` replayed one by one on a copy, each recorded
/// before and after.
fn phases(s: &Session, inputs: &BTreeMap<u32, Input>, heroes: &BTreeMap<u32, Hero>, n: &Nums) {
    if s.mode != Mode::Play {
        return;
    }
    if !want_with("griffinwing.phases", 8, 40) {
        return;
    }
    let mut c = s.clone();
    c.elapsed = (c.elapsed + 1).min(LIMIT);
    c.muster(heroes);
    let live: Vec<u32> = c.seats.keys().copied().collect();
    let keys = |id: u32| inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
    c.t = (c.t + 1).min(LIMIT);
    for &id in &live {
        let before = kit::sj(&c);
        c.fly(id, keys(id), n);
        if kit::cases::count("griffinwing.flight.Session.fly") < kit::CAP {
            rec(
                "griffinwing.flight.Session.fly",
                json!({"s": before, "id": id, "i": j(&keys(id)), "n": kit::nums_j(n)}),
                json!({"s": kit::sj(&c)}),
            );
        }
    }
    macro_rules! phase {
        ($name:expr, $call:expr) => {{
            let before = kit::sj(&c);
            $call;
            if kit::cases::count($name) < kit::CAP {
                rec($name, json!({"s": before, "n": kit::nums_j(n)}), json!({"s": kit::sj(&c)}));
            }
        }};
    }
    phase!("griffinwing.flight.Session.wing_fly", c.wing_fly(n));
    phase!("griffinwing.script.Session.run_script", c.run_script(n));
    phase!("griffinwing.combat.Session.step_foes", c.step_foes(n));
    phase!("griffinwing.boss.Session.step_boss", c.step_boss(n));
    phase!("griffinwing.combat.Session.step_bolts", c.step_bolts(n));
    phase!("griffinwing.things.Session.step_props", c.step_props(n));
    phase!("griffinwing.flight.Session.step_craft", c.step_craft(n));
}

/// The beat's transitions, on the session as it stands and on made-up ends.
fn transitions(s: &Session, rng: &mut Rng) {
    if want("griffinwing.Session.end_beat") {
        let mut variants = vec![s.clone()];
        let mut a = s.clone();
        for seat in a.seats.values_mut() {
            seat.craft.down = 30;
        }
        variants.push(a);
        let mut b = s.clone();
        b.t = rail::LEN[usize::from(b.stage.min(1))];
        variants.push(b);
        let mut d = s.clone();
        d.stage = 2;
        d.t = 900;
        d.boss = Some(boss_for_test(&d));
        if let Some(bb) = &mut d.boss {
            bb.st = Wyrm::Dying;
            bb.ticks = 0;
        }
        variants.push(d);
        for mut c in variants {
            let before = kit::sj(&c);
            c.end_beat();
            rec("griffinwing.Session.end_beat", json!({"s": before}), json!({"s": kit::sj(&c)}));
        }
    }
    if want("griffinwing.Session.next_beat") {
        let mut c = s.clone();
        c.stage = (rng.below(2)) as u8;
        let before = kit::sj(&c);
        c.next_beat();
        rec("griffinwing.Session.next_beat", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
    if want("griffinwing.Session.restart") {
        let mut c = s.clone();
        c.cp = (rng.below(500)) as u32;
        c.mode = Mode::Retry;
        c.t = c.cp + rng.below(900) as u32;
        let before = kit::sj(&c);
        c.restart();
        rec("griffinwing.Session.restart", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
    if want("griffinwing.Session.finish") {
        let mut c = s.clone();
        c.mode = Mode::Victory;
        let before = kit::sj(&c);
        c.finish();
        rec("griffinwing.Session.finish", json!({"s": before}), json!({"s": kit::sj(&c)}));
    }
}

/// A Wyrm for the states that need one.
fn boss_for_test(s: &Session) -> Beast {
    Beast {
        phase: 2,
        st: Wyrm::Idle,
        ticks: 10,
        x: 0,
        z: dm(12),
        lamps: [0, 0, 0],
        marks: Vec::new(),
        gap: 0,
        side: 1,
        col: 0,
        atk: 0,
        seq: s.t as u8,
    }
}
