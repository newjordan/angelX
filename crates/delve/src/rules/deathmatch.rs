//! Knights Wish: a first-to-ten free-for-all behind each knight's own eyes.
//! The town door uses the adventure shell; combat borrows the knight's kit,
//! never their wounds or purse. Fixed steps and seat order decide every hit.
//! The match itself (timers, respawn, hits and scoring, pickups, the podium and
//! the pot) is the Bend leg `deathmatch-rules`; this file keeps the bodies, the
//! saves and the order a tick walks them in, and asks the leg at each decision.
#[macro_use]
mod leg;
mod combat;
use super::activity::Activity;
use super::roundtable::{Adventure, Number};
use super::*;
pub use combat::{Field, FieldKind};
use serde_json::{Value, json};
use std::sync::OnceLock;

pub const NAME: &str = "Knights Wish";
pub const ENTRY: (i32, i32) = (137, 24);
pub const GOAL: u16 = 10;
pub const TITLE: u32 = 3 * HZ;
pub const RESPAWN: u32 = 2 * HZ;
pub const PROTECTION: u32 = 2 * HZ;
pub const PODIUM: u32 = 6 * HZ;
pub const PICKUP_REST: u32 = 15 * HZ;
pub const POWER_TIME: u32 = 8 * HZ;
pub const PACE: f32 = 9.0;
pub const TURN: f32 = 2.6;
const NUMBERS: [Number; 5] = [Number::stub("deathmatch_unused", "First to ten", "deathmatch::GOAL"); 5];

/// Original loop corridors around a broad hub. The same cells build the
/// collision grid and the cached wall mesh; a wall cannot be shot through.
pub const MAP: [&str; 18] = [
    "########################",
    "#..........##..........#",
    "#..........##..........#",
    "#..##..............##..#",
    "#..##..###....###..##..#",
    "#......###....###......#",
    "#......................#",
    "###..................###",
    "###......##..##......###",
    "###......##..##......###",
    "#......................#",
    "#......................#",
    "#......###....###......#",
    "#..##..###....###..##..#",
    "#..##..............##..#",
    "#..........##..........#",
    "#..........##..........#",
    "########################",
];
pub const SPAWNS: [(f32, f32); 8] = [(5., 5.), (43., 31.), (43., 5.), (5., 31.), (17., 17.), (31., 21.), (23., 9.), (25., 29.)];
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Pickup {
    Health,
    Armor,
    Power,
}
pub const PICKUPS: [(f32, f32, Pickup); 5] =
    [(9., 13., Pickup::Health), (39., 23., Pickup::Health), (9., 25., Pickup::Armor), (39., 11., Pickup::Armor), (25., 17., Pickup::Power)];

pub fn room() -> &'static Room {
    static ROOM: OnceLock<Room> = OnceLock::new();
    ROOM.get_or_init(|| {
        Room::kingdom(MAP.iter().flat_map(|row| row.bytes().map(|b| if b == b'#' { Tile::Wall } else { Tile::Floor })).collect(), 24, 18)
    })
}
pub fn grid() -> Grid<'static> {
    Grid { room: room(), barred: false }
}
pub fn open(x: f32, y: f32) -> bool {
    grid().clear(x, y, HERO_RADIUS, Mover::Hero)
}
pub fn sight(a: (f32, f32), b: (f32, f32)) -> bool {
    let n = ((b.0 - a.0).hypot(b.1 - a.1) * 5.).ceil().max(1.) as u32;
    (0..=n).all(|i| {
        let f = i as f32 / n as f32;
        grid().open(a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f, Mover::Hero)
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fighter {
    pub hero: Hero,
    pub yaw: f32,
    pub kills: u16,
    pub deaths: u16,
    pub respawn: u32,
    pub protection: u32,
    pub power: u32,
    pub flash: u32,
    pub bob: f32,
    pub connected: bool,
    #[serde(default)]
    pub stun: u32,
    #[serde(default)]
    pub burn: u32,
    #[serde(default)]
    pub burner: u32,
    #[serde(default)]
    pub fart_wait: u32,
}
impl Fighter {
    pub fn new(hero: &Hero, slot: usize, book: &Book) -> Self {
        let mut hero = hero.clone();
        hero.rebonus(book);
        hero.hp = hero.max_hp;
        hero.invulnerable = 0;
        hero.fire_cooldown = 0;
        hero.spell_cooldowns = [0; cards::SPELL_SLOTS];
        hero.stone = false;
        hero.idle = false;
        hero.vigil = false;
        hero.booby = None;
        hero.incoming = 100;
        hero.hit_ward = 0;
        hero.dash_ticks = 0;
        hero.dash_cooldown = 0;
        hero.privy = 0;
        hero.aloft = false;
        hero.flee_ticks = 0;
        hero.champ = Default::default();
        hero.hero_kit = Default::default();
        (hero.x, hero.y) = SPAWNS[slot % SPAWNS.len()];
        let yaw = (18. - hero.y).atan2(24. - hero.x);
        Self {
            hero,
            yaw,
            kills: 0,
            deaths: 0,
            respawn: 0,
            protection: PROTECTION,
            power: 0,
            flash: 0,
            bob: 0.,
            connected: true,
            stun: 0,
            burn: 0,
            burner: 0,
            fart_wait: 0,
        }
    }
    pub fn alive(&self) -> bool {
        self.connected && self.hero.hp > 0 && self.respawn == 0
    }
    /// Shared by the host and the browser's pending-command replay.
    pub fn walk_arena(&mut self, input: Input, fields: &[Field]) {
        let before = (self.hero.x, self.hero.y);
        self.walk(input);
        if fields.iter().any(|f| f.blocks(self.hero.x, self.hero.y, HERO_RADIUS)) {
            (self.hero.x, self.hero.y) = before;
        }
    }
    pub fn walk(&mut self, input: Input) {
        if !self.alive() || self.stun > 0 {
            return;
        }
        self.yaw = (self.yaw + input.aim_x as f32 * TURN * DT).rem_euclid(std::f32::consts::TAU);
        let (s, c) = self.yaw.sin_cos();
        let (forward, strafe) = (-input.move_y as f32, input.move_x as f32);
        let (mx, my) = unit(c * forward - s * strafe, s * forward + c * strafe);
        // The guard reads local movement, then its roll is turned into the
        // camera's world axes. own_step keeps the guard's ordinary pace.
        let ((_, _), _) = self.hero.own_keys(&Input { aim_x: 0, aim_y: 0, ..input }, false, true);
        (self.hero.aim_x, self.hero.aim_y) = (c, s);
        if self.hero.dash_ticks == ROLL_TICKS {
            (self.hero.dash_x, self.hero.dash_y) = if mx != 0. || my != 0. { (mx, my) } else { (c, s) };
        }
        let ((dx, dy, speed), _) = self.hero.own_step((mx, my), &grid(), 19., |h| {
            PACE * (100 + h.bonus.speed) as f32 / 100. * if h.chilled > 0 { lich::CHILL_PACE } else { 1. }
        });
        let before = (self.hero.x, self.hero.y);
        (self.hero.x, self.hero.y) = grid().slide(before, (dx * speed * DT, dy * speed * DT), HERO_RADIUS, Mover::Hero);
        self.bob += (self.hero.x - before.0).hypot(self.hero.y - before.1) * 1.8;
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frag {
    pub killer: u32,
    pub victim: u32,
    pub tick: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    #[serde(default, skip_serializing_if = "melee::State::quiet")]
    pub melee: melee::State,
    pub fighters: BTreeMap<u32, Fighter>,
    pub ticks: u32,
    pub shots: Vec<Projectile>,
    pub pickups: [u32; 5],
    pub feed: Vec<Frag>,
    pub winner: Option<u32>,
    pub ended: u32,
    pub ante: BTreeMap<u32, u32>,
    pub purse: u32,
    #[serde(default)]
    pub fields: Vec<Field>,
}
impl Session {
    pub fn new(players: &BTreeMap<u32, Hero>, book: &Book) -> Self {
        Self {
            fighters: players
                .iter()
                .filter(|(_, h)| !h.absent)
                .take(MAX_PLAYERS as usize)
                .enumerate()
                .map(|(i, (&id, h))| (id, Fighter::new(h, i, book)))
                .collect(),
            ticks: 0,
            shots: Vec::new(),
            pickups: [0; 5],
            feed: Vec::new(),
            winner: None,
            ended: 0,
            ante: BTreeMap::new(),
            purse: 0,
            fields: Vec::new(),
            melee: melee::State::default(),
        }
    }
    pub fn valid(&self) -> bool {
        self.melee.valid() && self.fighters.len() <= MAX_PLAYERS as usize
            && self.shots.len() <= MAX_PROJECTILES
            && self.fields.len() <= 32
            && self.feed.len() <= 5
            && self.fighters.iter().all(|(&id, f)| {
                (1..=MAX_PLAYERS).contains(&id)
                    && [f.hero.x, f.hero.y, f.yaw, f.bob].iter().all(|v| v.is_finite())
                    && open(f.hero.x, f.hero.y)
                    && f.hero.hp <= f.hero.max_hp
                    && f.kills <= GOAL
                    && f.respawn <= RESPAWN
                    && f.protection <= PROTECTION
            })
            && self
                .shots
                .iter()
                .all(|p| [p.x, p.y, p.vx, p.vy].iter().all(|v| v.is_finite()) && self.fighters.contains_key(&u32::from(p.traits.owner)))
            && self.fields.iter().all(|f| f.valid() && self.fighters.contains_key(&f.owner))
            && self.winner.is_none_or(|id| self.fighters.get(&id).is_some_and(|f| f.kills == GOAL))
    }
    pub fn order(&self) -> Vec<u32> {
        let mut ids: Vec<_> = self.fighters.keys().copied().collect();
        ids.sort_by_key(|id| (std::cmp::Reverse(self.fighters[id].kills), self.fighters[id].deaths, *id));
        ids
    }
    /// The knight stands again at the spawn farthest from the living, with their
    /// kit restored; what else the respawn clears the leg says.
    fn spawn(&mut self, id: u32) -> Result<(), String> {
        let others = |p: &(f32, f32)| {
            let near = self.fighters.iter().filter(|(other, f)| **other != id && f.alive());
            near.map(|(_, f)| (p.0 - f.hero.x).hypot(p.1 - f.hero.y)).fold(100., f32::min)
        };
        let fresh = leg::ask("spawn", json!({ "clear": SPAWNS.iter().map(others).collect::<Vec<_>>() }))?;
        let point = *SPAWNS.get(leg::num(&fresh["index"]) as usize).ok_or("no such spawn")?;
        let f = self.fighters.get_mut(&id).ok_or("no such knight")?;
        (f.hero.x, f.hero.y) = point;
        f.yaw = (18. - point.1).atan2(24. - point.0);
        f.hero.hp = f.hero.max_hp;
        f.hero.dress();
        f.hero.armor = f.hero.bonus.armor.min(MAX_ARMOR);
        (f.hero.dash_ticks, f.hero.shielding, f.hero.walling, f.hero.champ) = (0, false, false, Default::default());
        put!(fresh => f; protection, power, stun, burn);
        put!(fresh => f.hero; invulnerable, fire_cooldown, spell_cooldowns);
        f.hero.hero_kit = Default::default();
        Ok(())
    }
    /// One tick of the round. A tick the leg cannot answer leaves the round as it
    /// stood; nothing is guessed natively.
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>, book: &Book, seed: u64) {
        let _ = self.play(inputs, book, seed);
    }
    pub(super) fn play(&mut self, inputs: &BTreeMap<u32, Input>, book: &Book, seed: u64) -> Result<(), String> {
        let on = self.fighters.values().filter(|f| f.connected).count();
        let open = leg::ask(
            "begin",
            json!({ "winner": self.winner.is_some(), "ended": self.ended, "connected": on, "ticks": self.ticks, "pickups": self.pickups }),
        )?;
        if leg::ask("melee_tick", json!({"winner": self.winner.is_some(), "connected": on}))? == true { self.melee.tick(); }
        put!(open => self; ended, ticks, pickups);
        if open["go"] != true {
            return Ok(());
        }
        let ids: Vec<_> = self.fighters.keys().copied().collect();
        // One question asks every knight's turn as the tick found them; a knight a rival
        // has struck since (the only change a turn can see) is asked again, alone.
        let asked: Vec<Value> = ids.iter().map(|&id| self.turn_facts(id)).collect();
        let batch = leg::ask("turns", json!({ "turns": asked }))?;
        for (i, &id) in ids.iter().enumerate() {
            let facts = self.turn_facts(id);
            let t = if facts == asked[i] { batch[i].clone() } else { leg::ask("turn", json!({ "turn": facts }))? };
            let f = self.fighters.get_mut(&id).unwrap();
            put!(t["fighter"] => f; respawn, protection, power, flash, stun, fart_wait, burn);
            put!(t["hero"] => f.hero; leeched, passive, chilled, invulnerable, fire_cooldown, sword_cooldown, play_cooldown, bomb_cooldown, spell_cooldowns);
            put!(t["champ"] => f.hero.champ; fury);
            match t["act"].as_str() {
                Some("Skip") => continue,
                Some("Spawn") => {
                    self.spawn(id)?;
                    continue;
                }
                _ => {}
            }
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            f.walk_arena(input, &self.fields);
            let (blow, owner) = (leg::num(&t["burn_blow"]), f.burner);
            if blow > 0 {
                self.hit(owner, id, blow, (0., 0.));
            }
            // The slam roots a walking knight on the old count; it runs down now.
            let h = &mut self.fighters.get_mut(&id).unwrap().hero;
            put!(t["champ"] => h.champ; windup, rot);
            let (at, slam, rot) = (
                (h.x, h.y),
                (h.spell_step("groundslam").slam_reach, h.spell_damage("groundslam", champions::SLAM_DAMAGE)),
                (h.spell_step("rot").rot_reach, h.spell_damage("rot", champions::ROT_DAMAGE)),
            );
            if t["slam"] == true {
                self.area(id, at, slam.0, slam.1);
            }
            if t["pulse"] == true {
                self.area(id, at, rot.0, rot.1);
            }
            if t["orbit"] == true {
                self.orbits(id);
            }
            // The turn already said whether the knight may attack; that stands unless a blow or a win has landed since.
            let unchanged =
                self.winner.is_some() == (facts["winner"] == true) && (self.fighters[&id].hero.hp > 0) == (facts["living"] == true);
            self.attack(id, input, book, seed, leg::num(&t["power_num"]), unchanged.then(|| t["armed"] == true))?;
            if self.winner.is_some() {
                break;
            }
            self.gather(id)?;
        }
        let live = self.winner.is_none();
        if live {
            self.fly()?;
        }
        let fields = if live { json!(self.fields) } else { json!([]) };
        let sweep = leg::ask("close", json!({ "sweep": { "ticks": self.ticks, "fields": fields, "feed": self.feed } }))?;
        // The frags the pulses below score are fresh, so forgetting the old ones first is the same.
        put!(sweep => self; feed);
        if live {
            let pulses: Vec<Field> = serde_json::from_value(sweep["pulses"].clone()).map_err(|e| e.to_string())?;
            self.fields.clear();
            for f in pulses {
                self.area(f.owner, (f.x, f.y), f.radius, f.damage);
            }
            put!(sweep => self; fields);
        }
        Ok(())
    }
    /// What a knight's turn is asked: the tick's clock and the counters that run down.
    fn turn_facts(&self, id: u32) -> Value {
        let f = &self.fighters[&id];
        json!({ "ticks": self.ticks, "orbit_every": ORBIT_EVERY, "rot_every": champions::ROT_EVERY, "winner": self.winner.is_some(), "living": f.hero.hp > 0,
            "fighter": pick!(f; connected, respawn, protection, power, flash, stun, fart_wait, burn),
            "hero": pick!(f.hero; leeched, passive, chilled, invulnerable, fire_cooldown, sword_cooldown, play_cooldown, bomb_cooldown, spell_cooldowns),
            "champ": pick!(f.hero.champ; fury, windup, rot) })
    }
    /// The knight takes what pickups they stand on; the leg says what each gives.
    fn gather(&mut self, id: u32) -> Result<(), String> {
        let f = &self.fighters[&id];
        let near = |&(x, y, _): &(f32, f32, Pickup)| !((x - f.hero.x).hypot(y - f.hero.y) > 1.2);
        if !f.alive() || !PICKUPS.iter().any(near) {
            return Ok(());
        }
        let spots: Vec<Value> =
            PICKUPS.iter().zip(self.pickups).map(|(p, wait)| json!({ "near": near(p), "kind": p.2 as u8, "wait": wait })).collect();
        let grab =
            json!({ "spots": spots, "hp": f.hero.hp, "max_hp": f.hero.max_hp, "armor": f.hero.armor, "cap": MAX_ARMOR, "power": f.power });
        let taken = leg::ask("grab", json!({ "grab": grab }))?;
        put!(taken => self; pickups);
        let f = self.fighters.get_mut(&id).unwrap();
        put!(taken => f.hero; hp, armor);
        put!(taken => f; power);
        Ok(())
    }
    /// A blow from `owner` on `target` along `way`: true when it hurt. Whether it
    /// lands and what it scores is the leg's; the shield, the mail and the wound
    /// are the kit's.
    pub fn hit(&mut self, owner: u32, target: u32, damage: u32, way: (f32, f32)) -> bool {
        self.strike(owner, target, damage, way).unwrap_or(false)
    }
    fn strike(&mut self, owner: u32, target: u32, damage: u32, way: (f32, f32)) -> Result<bool, String> {
        let t = self.fighters.get(&target);
        let blocked =
            t.is_some_and(|f| shield::turns(&f.hero, way) || (f.hero.walling && -(way.0 * f.hero.aim_x + way.1 * f.hero.aim_y) > 0.));
        let blow = json!({ "owner": owner, "target": target, "winner": self.winner.is_some(), "owner_in": self.fighters.contains_key(&owner), "target_in": t.is_some(), "connected": t.is_some_and(|f| f.connected), "hp": t.map_or(0, |f| f.hero.hp), "respawn": t.map_or(0, |f| f.respawn), "protection": t.map_or(0, |f| f.protection), "blocked": blocked });
        let verdict = leg::ask("hit", json!({ "blow": blow }))?;
        let (Some(kills), Some(f)) = (self.fighters.get(&owner).map(|k| k.kills), self.fighters.get_mut(&target)) else {
            return Ok(false);
        };
        match verdict.as_str() {
            Some("Strike") => {}
            Some("Turned") => {
                f.hero.shield_turned(damage);
                self.melee.parry((f.hero.x, f.hero.y), unit(way.0, way.1));
                return Ok(false);
            }
            _ => return Ok(false),
        }
        let before = f.hero.hp;
        f.hero.hit_ward = 0;
        f.hero.hurt(damage);
        f.hero.dress();
        if f.hero.hp != before {
            let tier = match leg::ask("contact_tier", json!({"damage": damage}))?.as_str() {
                Some("Heavy") => melee::Tier::Heavy,
                Some("Medium") => melee::Tier::Medium,
                Some("Light") => melee::Tier::Light,
                _ => return Err("unknown contact tier".into()),
            };
            self.melee.hit(target, (f.hero.x, f.hero.y), unit(way.0, way.1), tier, false);
        }
        let struck = json!({ "owner": owner, "target": target, "before": before, "after": f.hero.hp, "ticks": self.ticks, "kills": kills, "deaths": f.deaths, "feed": self.feed });
        let s = leg::ask("struck", json!({ "struck": struck }))?;
        if s["died"] == true {
            put!(s => f; respawn, deaths);
            put!(s => self; feed);
            put!(s => self.fighters.get_mut(&owner).unwrap(); kills);
            if s["winner"] == true {
                (self.winner, self.ended) = (Some(owner), 0);
            }
        }
        Ok(s["landed"] == true)
    }
}

pub const ADVENTURE: Adventure = Adventure {
    slug: "deathmatch",
    name: NAME,
    entry: ENTRY,
    slot: usize::MAX,
    marks: &[],
    numbers: &NUMBERS,
    showing: Run::deathmatch_showing,
    start: Run::start_deathmatch,
    valid: |r| r.deathmatch.as_ref().is_none_or(Session::valid),
    clear: |r| r.deathmatch = None,
    seated: |r| r.deathmatch.as_ref().map_or_else(Vec::new, |s| s.fighters.keys().copied().collect()),
    bot: |_, _| Input::default(),
};
pub const ACTIVITY: Activity = Activity {
    name: NAME,
    on: Run::deathmatch_showing,
    ride: Some(|run, inputs| {
        run.tick_deathmatch(inputs);
        Some(BTreeMap::new())
    }),
    tick: |_, _| {},
    end: |r| r.deathmatch = None,
    ends_outside: true,
    settle: None,
    fields: &["deathmatch"],
};
impl Run {
    pub fn deathmatch_showing(&self) -> bool {
        self.deathmatch.is_some()
    }
    /// Clean entry for the town door and a future mini-game ballot.
    pub fn start_deathmatch(&mut self) {
        if self.room().kind != RoomKind::Forest || self.deathmatch.is_some() {
            return;
        }
        self.forest.leave();
        let mut s = Session::new(&self.players, &self.book);
        // The same tenth-of-carried-gold stake as a race hall. It changes
        // hands only at a win; abandoning a round loses nobody's purse.
        s.ante = self
            .players
            .iter()
            .filter(|(id, _)| s.fighters.contains_key(id))
            .map(|(&id, h)| (id, h.carried.get(Spoil::Gold) / 10))
            .collect();
        self.deathmatch = Some(s);
        self.found = Some((self.tick, 0, "Knights Wish: WASD moves, arrows turn, F fires, Space guards, 1-4 cast. First to ten.".into()));
    }
    pub fn tick_deathmatch(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(mut s) = self.deathmatch.take() else {
            return;
        };
        let home = self.deathmatch_round(&mut s, inputs).unwrap_or(false);
        self.deathmatch = Some(s);
        if home {
            for id in self.players.keys().copied().collect::<Vec<_>>() {
                self.leave_activity(id);
            }
            self.deathmatch = None;
        }
    }
    /// One tick of the round and what the room owes it: seats arriving late get
    /// their real kit, nobody outside, absent or away can be farmed, the shots are
    /// heard, and a win pays the pot. True when the podium has run and the knights
    /// go home.
    fn deathmatch_round(&mut self, s: &mut Session, inputs: &BTreeMap<u32, Input>) -> Result<bool, String> {
        let crossroads = self.crossroads.is_some();
        let gold = |id: &u32| self.players.get(id).map_or(0, |h| h.carried.get(Spoil::Gold));
        let ids: std::collections::BTreeSet<u32> = self.players.keys().chain(s.fighters.keys()).copied().collect();
        let rows: Vec<Value> = ids.iter().map(|id| {
            let h = self.players.get(id);
            json!({ "id": id, "player": h.is_some(), "absent": h.is_some_and(|h| h.absent), "away": h.is_some_and(|h| h.away), "outside": self.exits.outside.contains_key(id), "fighter": s.fighters.contains_key(id), "gold": gold(id) })
        }).collect();
        let seats = leg::ask("sync", json!({ "crossroads": crossroads, "winner": s.winner.is_some(), "rows": rows }))?;
        for seat in seats.as_array().into_iter().flatten() {
            let id = leg::num(&seat["id"]);
            if let (true, Some(h)) = (seat["join"] == true, self.players.get(&id)) {
                s.fighters.insert(id, Fighter::new(h, id.saturating_sub(1) as usize, &self.book));
                s.ante.insert(id, leg::num(&seat["stake"]));
            }
            if let Some(f) = s.fighters.get_mut(&id) {
                f.connected = seat["connected"] == true;
            }
        }
        let (before, tick) = (s.winner, s.ticks);
        let kills = |s: &Session| s.fighters.values().map(|f| u32::from(f.kills)).sum::<u32>();
        let kills_before = kills(s);
        s.play(inputs, &self.book, self.seed)?;
        let muzzles: Vec<Value> = s.fighters.iter().map(|(id, f)| json!({ "id": id, "alive": f.alive(), "flash": f.flash })).collect();
        let stakes: Vec<Value> = s
            .ante
            .iter()
            .map(|(seat, stake)| json!({ "seat": seat, "stake": stake, "present": self.players.contains_key(seat), "gold": gold(seat) }))
            .collect();
        let wrap = json!({ "crossroads": crossroads, "fresh": before.is_none(), "winner": s.winner.is_some(), "winner_id": s.winner.unwrap_or(0), "moved": s.ticks > tick, "kills_before": kills_before, "kills_after": kills(s), "ended": s.ended, "purse": s.purse, "muzzles": muzzles, "stakes": stakes });
        let done = leg::ask("wrap", json!({ "wrap": wrap }))?;
        put!(done => s; purse);
        if done["settle"] == true {
            for take in done["takes"].as_array().into_iter().flatten().filter(|t| t["present"] == true) {
                if let Some(h) = self.players.get_mut(&leg::num(&take["seat"])) {
                    h.carried.take(&Spoils([(Spoil::Gold, leg::num(&take["amount"]))].into()));
                    h.laurel = take["laurel"] == true;
                }
            }
            if let Some(h) = s.winner.and_then(|id| self.players.get_mut(&id)) {
                h.carried.add(Spoil::Gold, s.purse);
            }
        }
        for cue in done["cues"].as_array().into_iter().flatten() {
            match cue.as_str() {
                Some("Kill") => self.sounds.push("hit"),
                Some("Card") => self.sounds.push("play_card"),
                Some("Won") => self.cues.push("pastime_won".into()),
                _ => {
                    // A shot's cue names its shooter; the sound is that fighter's.
                    let shooter = leg::num(&cue["id"]);
                    self.sounds.extend(s.fighters.get(&shooter).map(Fighter::shot_sound));
                }
            }
        }
        Ok(done["home"] == true)
    }
}
