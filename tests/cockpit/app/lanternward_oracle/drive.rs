//! Scripted sessions for the parity tests: who is at the table, what they
//! press, what is set by hand between ticks. Everything here is derived from
//! the oracle's own session and a seeded stream, so any engine can be stepped
//! on the same inputs and compared with it, tick by tick.
use super::kit::{B_BOMB, B_CTX, B_GUARD, B_LAMP, B_LOCK, B_SWORD};
use super::*;
use serde_json::{Value, json};

/// A small seeded stream (xorshift64*), so a scenario is its seed.
#[derive(Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0 | 1;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next() >> 33) % u64::from(n.max(1))) as u32
    }
    /// True `per_mille` times in a thousand.
    pub fn chance(&mut self, per_mille: u32) -> bool {
        self.below(1000) < per_mille
    }
    pub fn axis(&mut self) -> i8 {
        self.below(3) as i8 - 1
    }
}

/// The session as the engine reads it: serde's shape with the 64-bit seed as
/// its two words, since a JSON number past 2^53 would lose bits in a script.
pub fn wire(s: &Session) -> Value {
    let mut v = serde_json::to_value(s).unwrap();
    v["seed"] = json!([(s.seed >> 32) as u32, s.seed as u32]);
    v
}

/// `n` knights at the table, as a run seats them.
pub fn heroes(n: u32) -> BTreeMap<u32, Hero> {
    let mut run = angel_delve::rules::Run::lanternward_for_test();
    for id in 2..=n {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.players.clone()
}

/// The seven buttons the adventure reads, as the engine is told them.
pub fn mask(i: &Input) -> u32 {
    u32::from(kit::buttons(i))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    /// Every seat plays as the scripted bot.
    Bots,
    /// Every seat presses random keys, held for a few ticks at a time.
    Fuzz,
    /// The bot, with a random key now and then.
    Noisy,
    /// Random keys with values the wire may not carry, now and then: the rules
    /// must take what they cannot read as no keys at all.
    Edge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The adventure as a door opens it.
    Fresh,
    /// The party in this room, the rooms before it done.
    Room(usize),
    /// A random room in a random state of being solved.
    Random,
}

pub struct Scenario {
    pub name: String,
    pub session: Session,
    pub heroes: BTreeMap<u32, Hero>,
    pub rules: Rules,
    pub driver: Driver,
    pub rng: Rng,
    pub tick: u32,
    pub chaos: bool,
    held: BTreeMap<u32, Input>,
    back: BTreeMap<u32, u32>,
}

/// The party in room `room` having done the rooms before it.
pub fn rooms_to(s: &mut Session, room: usize, r: &Rules) {
    s.enter_room(room, r);
    s.inv.map = room >= 1;
    s.inv.lantern = room >= 2;
    if room >= 2 {
        for k in s.seats.values_mut() {
            k.flame = r.flame;
        }
    }
    if room >= 4 {
        s.inv.boss = Hold::Spent;
    }
}

impl Scenario {
    pub fn new(seed: u64, players: u32, start: Start, driver: Driver, chaos: bool) -> Scenario {
        let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
        rng.next();
        let heroes = heroes(players);
        let mut rules = Rules::default();
        if rng.chance(500) {
            rules = Rules {
                lock: (48 + rng.below(153)) as i32 * Q,
                parry: 3 + rng.below(12),
                combo: 6 + rng.below(19),
                flame: 60 + rng.below(241),
                tell: (40 + rng.below(51)).max(TELL_FLOOR),
            };
        }
        let mut session = Session::new(&heroes);
        if rng.chance(700) {
            session.seed = rng.next();
        }
        let mut name = format!("{seed}-{players}p-{driver:?}");
        match start {
            Start::Fresh => {}
            Start::Room(r) => {
                rooms_to(&mut session, r, &rules);
                name += &format!("-room{r}");
            }
            Start::Random => {
                let room = rng.below(5) as usize;
                rooms_to(&mut session, room, &rules);
                name += &format!("-rand{room}");
                scatter(&mut session, &mut rng);
            }
        }
        Scenario {
            name,
            session,
            heroes,
            rules,
            driver,
            rng,
            tick: 0,
            chaos,
            held: BTreeMap::new(),
            back: BTreeMap::new(),
        }
    }

    /// What each seat presses on the next tick.
    pub fn inputs(&mut self) -> BTreeMap<u32, Input> {
        let ids: Vec<u32> = self.heroes.keys().copied().collect();
        let mut out = BTreeMap::new();
        for id in ids {
            let bot = self.session.bot_input(id, &self.heroes[&id], &self.rules);
            let key = self.fuzz(id);
            let i = match self.driver {
                Driver::Bots => bot,
                Driver::Fuzz => key,
                Driver::Noisy => {
                    if self.rng.chance(120) {
                        key
                    } else {
                        bot
                    }
                }
                Driver::Edge => {
                    let mut i = if self.rng.chance(500) { bot } else { key };
                    match self.rng.below(8) {
                        0 => i.move_x = 2,
                        1 => i.aim_y = -3,
                        2 => i.play = 255,
                        3 => i.cast = 250,
                        4 => i.skill = 200,
                        5 => i.move_y = i8::MIN,
                        _ => {}
                    }
                    i
                }
            };
            out.insert(id, i);
        }
        out
    }

    fn fuzz(&mut self, id: u32) -> Input {
        let mut i = self.held.get(&id).copied().unwrap_or_default();
        if self.rng.chance(180) {
            i.move_x = self.rng.axis();
            i.move_y = self.rng.axis();
        }
        if self.rng.chance(120) {
            i.aim_x = self.rng.axis();
            i.aim_y = self.rng.axis();
        }
        for (flag, per) in [
            (&mut i.ult, 90),
            (&mut i.dash, 110),
            (&mut i.swing, 220),
            (&mut i.interact, 140),
            (&mut i.fire, 100),
            (&mut i.bomb, 40),
        ] {
            if self.rng.chance(per) {
                *flag = !*flag;
            }
        }
        self.held.insert(id, i);
        i
    }

    /// A knight stands up from the table or sits back down, now and then.
    pub fn churn(&mut self) {
        if !self.chaos {
            return;
        }
        let tick = self.tick;
        let back = &mut self.back;
        for (id, hero) in self.heroes.iter_mut() {
            if hero.away && back.get(id).is_some_and(|&t| tick >= t) {
                hero.away = false;
                back.remove(id);
            }
        }
        if self.rng.chance(4) {
            let id = 1 + self.rng.below(self.heroes.len() as u32);
            // The table is never emptied.
            if self.heroes.values().filter(|h| !h.away).count() > 1 && !self.heroes[&id].away {
                self.heroes.get_mut(&id).unwrap().away = true;
                self.back.insert(id, tick + 20 + self.rng.below(160));
            }
        }
        if self.rng.chance(2) {
            let id = 1 + self.rng.below(self.heroes.len() as u32);
            let awake = self.heroes.values().filter(|h| !h.away && !h.stone).count();
            let h = self.heroes.get_mut(&id).unwrap();
            if h.stone || awake > 1 {
                h.stone = !h.stone;
            }
        }
    }

    /// A hand on the session between ticks: true when it changed.
    pub fn meddle(&mut self) -> bool {
        if !self.chaos || !self.rng.chance(6) {
            return false;
        }
        scatter(&mut self.session, &mut self.rng);
        true
    }
}

fn open_spot(s: &Session, rng: &mut Rng) -> (i32, i32) {
    let p = plan(usize::from(s.room));
    for _ in 0..200 {
        let (tx, ty) = (rng.below(p.w() as u32) as i32, rng.below(p.h() as u32) as i32);
        let (x, y) = (middle(tx), middle(ty));
        if s.clear(x, y, KNIGHT_R) && !s.pit(x, y) {
            return (x, y);
        }
    }
    s.start_of(1)
}

/// Throw the session about: a few random edits, each one a state some play
/// could reach (or that a hurried save might hold).
pub fn scatter(s: &mut Session, rng: &mut Rng) {
    for _ in 0..1 + rng.below(3) {
        if s.seats.is_empty() {
            return;
        }
        match rng.below(16) {
            0 => {
                let ids: Vec<u32> = s.seats.keys().copied().collect();
                let id = ids[rng.below(ids.len() as u32) as usize];
                let at = open_spot(s, rng);
                let k = s.seats.get_mut(&id).unwrap();
                (k.x, k.y) = at;
                k.safe = at;
                k.cam = at;
            }
            1 => {
                for k in s.seats.values_mut() {
                    k.hp = 1 + rng.below(k.max_hp);
                }
            }
            2 => {
                let ids: Vec<u32> = s.seats.keys().copied().collect();
                let id = ids[rng.below(ids.len() as u32) as usize];
                s.seats.get_mut(&id).unwrap().bombs = rng.below(BOMB_CAP + 1);
            }
            3 => {
                s.inv.lantern = true;
                for k in s.seats.values_mut() {
                    k.flame = rng.below(300);
                }
            }
            4 => {
                s.inv.small_keys = rng.below(3) as u8;
            }
            5 => {
                let at = open_spot(s, rng);
                let kind = if rng.chance(500) { Fk::Snuffer } else { Fk::Turret };
                let hooded = rng.chance(400);
                s.spawn_foe(kind, at.0, at.1, hooded);
            }
            6 => {
                if let Some(w) = s.boss.as_mut() {
                    w.hp = (1 + rng.below(w.full)).min(w.full);
                    w.start = w.hp;
                    w.phase = if w.hp <= w.full / 3 {
                        2
                    } else if w.hp <= w.full / 3 * 2 {
                        1
                    } else {
                        0
                    };
                }
            }
            7 => {
                let now = s.elapsed;
                if let Some(w) = s.boss.as_mut() {
                    for t in w.lit.iter_mut() {
                        *t = if rng.chance(500) { now } else { 0 };
                    }
                }
            }
            8 => {
                for f in s.foes.iter_mut() {
                    if rng.chance(500) {
                        f.hp = 1 + rng.below(f.full);
                    }
                }
            }
            9 => {
                let at = open_spot(s, rng);
                s.drop_at(Dk::Heart, at.0, at.1);
                let at = open_spot(s, rng);
                s.drop_at(Dk::Gem, at.0, at.1);
                let at = open_spot(s, rng);
                s.drop_at(Dk::Bomb, at.0, at.1);
            }
            10 => {
                let n = s.props.len() as u32;
                for _ in 0..1 + rng.below(4) {
                    let i = rng.below(n) as usize;
                    let p = &mut s.props[i];
                    p.on = match p.kind {
                        Pk::Pot | Pk::Grass | Pk::Gate | Pk::Lock | Pk::Boss | Pk::Cracked | Pk::Plate | Pk::Crystal
                        | Pk::Great | Pk::Lever | Pk::Sigil | Pk::Chest | Pk::Beacon => rng.below(2) as u8,
                        Pk::Brazier => rng.below(2) as u8,
                        _ => p.on,
                    };
                    if p.kind == Pk::Brazier {
                        p.t = rng.below(600) as u16;
                    }
                }
            }
            11 => {
                s.stage = rng.below(5) as u8;
                s.stage_t = rng.below(400);
            }
            12 => {
                let ids: Vec<u32> = s.seats.keys().copied().collect();
                let id = ids[rng.below(ids.len() as u32) as usize];
                let k = s.seats.get_mut(&id).unwrap();
                k.hp = 0;
                k.act = Act::Down;
                k.t = 0;
                k.lock = None;
            }
            13 => {
                s.elapsed = (s.elapsed + 1 + rng.below(3000)).min(LIMIT - 100);
            }
            14 => {
                if rng.chance(300) {
                    s.inv.containers = rng.below(4) as u8;
                    for k in s.seats.values_mut() {
                        k.max_hp = START_HP + HEART * u32::from(s.inv.containers);
                        k.hp = k.hp.min(k.max_hp);
                    }
                }
                s.inv.map = true;
            }
            _ => {
                if s.fuses.len() < FUSE_CAP {
                    let at = open_spot(s, rng);
                    s.fuses.push(Fuse { x: at.0, y: at.1, t: 1 + rng.below(70) as u8, owner: 1 });
                }
            }
        }
    }
}

/// One tick of a scenario, for whoever is stepping an engine beside it.
pub struct Tick {
    pub reset: Option<Value>,
    pub inputs: BTreeMap<u32, Input>,
    pub here: Vec<u32>,
    pub rules: Rules,
    pub before: Session,
    pub after: Session,
    pub sounds: Vec<&'static str>,
    pub stop: Option<Stop>,
    pub won: bool,
    pub trace: Vec<(&'static str, Session, Vec<&'static str>, Option<Stop>)>,
}

/// Play `ticks` of a scenario on the oracle, handing each tick to `on_tick`.
/// The oracle's session is the truth: whatever `on_tick` does with its own
/// engine, the next tick's inputs come from this one.
pub fn play(sc: &mut Scenario, ticks: u32, mut on_tick: impl FnMut(&Scenario, Tick)) {
    for _ in 0..ticks {
        sc.tick += 1;
        let reset = if sc.meddle() { Some(wire(&sc.session)) } else { None };
        sc.churn();
        let inputs = sc.inputs();
        let here: Vec<u32> = sc
            .heroes
            .iter()
            .filter(|(_, h)| !h.away && !h.stone)
            .map(|(&id, _)| id)
            .collect();
        let before = sc.session.clone();
        let mut trace = Vec::new();
        let ev = sc.session.step_traced(&inputs, &sc.heroes, &sc.rules, &mut trace);
        let tick = Tick {
            reset,
            inputs,
            here,
            rules: sc.rules,
            before,
            after: sc.session.clone(),
            sounds: ev.sounds.clone(),
            stop: ev.stop,
            won: ev.won,
            trace,
        };
        on_tick(sc, tick);
        if sc.session.done.is_some_and(|t| sc.session.elapsed >= t + FINALE_TICKS + 300) {
            break;
        }
    }
}

/// The engine's ask for one tick: the session in its wire shape, one row of
/// `[id, move_x, move_y, aim_x, aim_y, buttons]` for every knight at the
/// table, and the five numbers.
pub fn ask(t: &Tick) -> Value {
    let rows: Vec<Value> = t
        .here
        .iter()
        .map(|id| {
            let i = t.inputs.get(id).copied().filter(|i| i.valid()).unwrap_or_default();
            json!([id, i.move_x, i.move_y, i.aim_x, i.aim_y, mask(&i)])
        })
        .collect();
    json!({
        "op": "tick",
        "session": wire(&t.before),
        "here": rows,
        "rules": {"lock": t.rules.lock, "parry": t.rules.parry, "combo": t.rules.combo, "flame": t.rules.flame, "tell": t.rules.tell},
    })
}

pub const SOUNDS: [&str; 46] = [
    "axe_spin", "bomb", "bomb_toss", "boss_death", "boss_fall", "boss_rage", "boss_roar", "card_pickup", "chest_open",
    "circle_toll", "clang", "crack_open", "descend", "door_open", "door_pass", "door_seal", "gold", "hit_axe",
    "hit_blade", "hit_magic", "holy_water", "kill", "knight_fall", "knight_hit", "knight_hit_big", "mana_empty",
    "play_card", "potion", "rock_land", "roll", "shield_block", "shield_up", "shot_arcane", "shot_bolt", "shot_spark",
    "spell_cast", "spell_fire", "spoil_pickup", "step_ash", "step_moss", "step_stone", "swing", "wall_up", "wave_gate",
    "wheel_land", "wish_grant",
];
