//! The Delve's spectacle: a flinch for a knight who is hit, the bones a
//! fallen knight leaves, the demon hand that comes for a slain guardian, the
//! hell gate at a dungeon's end and the circles below it, and the sounds
//! that go with all of it.
//!
//! None of it changes how a fight is fought. The state is a few counters on
//! the run and on each knight; the arena draws from them
//! (`arena/spectacle.rs`, `arena/hand.rs`, `arena/gate.rs`), and a friend's
//! mirror and a browser guest's picture show exactly what the host's does.
//! One watcher reads the tick just gone against the tick before it and turns
//! the difference into sounds and poses, so the many places that hurt, heal
//! and strike stay as they were.

use super::*;

pub const STRUCK: u32 = 16;
const HEAVY_HIT: u32 = 22;
const STRIDE: f32 = 3.4;
pub const FOOTSTEPS: bool = false;
const LANDED_EVERY: u64 = 4;

pub const LOOT_WAIT: u32 = 4 * HZ;
pub const CRACK: u32 = 36;
pub const RISE: u32 = 45;
pub const GRAB: u32 = 18;
pub const PULL: u32 = 66;
pub const CLOSE: u32 = 30;
pub const CLAIM: u32 = LOOT_WAIT + CRACK + RISE + GRAB + PULL + CLOSE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Beat {
    Lying,
    Cracking(f32),
    Rising(f32),
    Grabbing(f32),
    Pulling(f32),
    Closing(f32),
    Gone,
}

impl Beat {
    pub fn of(age: u64) -> Beat {
        let mut left = age;
        let mut next = |len: u32| -> Option<f32> {
            if left < u64::from(len) {
                Some(left as f32 / len as f32)
            } else {
                left -= u64::from(len);
                None
            }
        };
        if next(LOOT_WAIT).is_some() {
            return Beat::Lying;
        }
        if let Some(t) = next(CRACK) {
            return Beat::Cracking(t);
        }
        if let Some(t) = next(RISE) {
            return Beat::Rising(t);
        }
        if let Some(t) = next(GRAB) {
            return Beat::Grabbing(t);
        }
        if let Some(t) = next(PULL) {
            return Beat::Pulling(t);
        }
        if let Some(t) = next(CLOSE) {
            return Beat::Closing(t);
        }
        Beat::Gone
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corpse {
    pub kind: EnemyKind,
    pub boss: Option<u8>,
    pub x: f32,
    pub y: f32,
    pub born: u64,
    pub room: usize,
    pub radius: f32,
}

impl Corpse {
    pub fn beat(&self, tick: u64) -> Beat {
        Beat::of(tick.saturating_sub(self.born))
    }
}

pub const GATE_RISE: u32 = 3 * HZ;
const GATE_REACH: f32 = 1.9;
pub const CARD_TICKS: u32 = 6 * HZ;
const CIRCLE_HP: u32 = 18;
const CIRCLE_HURT: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    pub x: f32,
    pub y: f32,
    pub born: u64,
    pub room: usize,
}

impl Gate {
    pub fn risen(&self, tick: u64) -> f32 {
        (tick.saturating_sub(self.born) as f32 / GATE_RISE as f32).min(1.0)
    }
}

pub fn circle_name(n: u32) -> String {
    const ORDINALS: [&str; 12] = [
        "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth",
        "Tenth", "Eleventh", "Twelfth",
    ];
    match ORDINALS.get((n as usize).wrapping_sub(1)) {
        Some(word) => format!("{word} Circle"),
        None => format!("Circle {n}"),
    }
}

pub fn circle_title(n: u32) -> &'static str {
    match n {
        1 => "Limbo",
        2 => "Lust",
        3 => "Gluttony",
        4 => "Greed",
        5 => "Wrath",
        6 => "Heresy",
        7 => "Violence",
        8 => "Fraud",
        9 => "Treachery",
        _ => "The Bottomless Pit",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spectacle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<PhaseMark>,
    pub corpses: Vec<Corpse>,
    pub gate: Option<Gate>,
    pub circle: u32,
    pub card: Option<(u64, u32)>,
    pub scaled_id: u32,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub circling: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseMark {
    pub x: f32,
    pub y: f32,
    pub born: u64,
    pub room: usize,
    pub rage: bool,
}

pub const PHASE_MARK: u64 = 30;

pub(super) struct SeenHero {
    id: u32,
    x: f32,
    y: f32,
    hp: u32,
    pop: u32,
    shielding: bool,
    spin: u32,
    bombs: u32,
    hand: Vec<String>,
    booby: bool,
}

pub(super) struct Seen {
    sounded: usize,
    cued: usize,
    at: usize,
    heroes: Vec<SeenHero>,
    foes: Vec<(u32, u32, bool)>,
    items: Vec<String>,
}

impl Run {
    pub(super) fn seen(&self) -> Seen {
        Seen {
            sounded: self.sounds.len(),
            cued: self.cues.len(),
            at: self.at,
            heroes: self
                .players
                .iter()
                .map(|(&id, h)| SeenHero {
                    id,
                    x: h.x,
                    y: h.y,
                    hp: h.hp,
                    pop: h.pop,
                    shielding: h.shielding,
                    spin: h.spin,
                    bombs: h.bombs,
                    hand: h.hand.clone(),
                    booby: h.booby.is_some(),
                })
                .collect(),
            foes: {
                let mut foes = super::scratch::take_seen_foes();
                foes.extend(self.enemies.iter().map(|e| (e.id, e.hp, e.raged)));
                foes
            },
            items: self
                .dungeon
                .rooms
                .get(self.at)
                .map(|r| r.items.iter().map(|i| i.card.clone()).collect())
                .unwrap_or_default(),
        }
    }

    pub fn circle(&self) -> u32 {
        self.spectacle.circle
    }

    pub(super) fn watch(&mut self, seen: Seen) {
        if self.spectacle.phase.is_some_and(|p| p.room != self.at || self.tick.saturating_sub(p.born) >= PHASE_MARK) {
            self.spectacle.phase = None;
        }
        self.strength_of_circles();
        self.watch_heroes(&seen);
        self.watch_foes(&seen);
        self.tick_claims();
        self.tick_gate();
        if self.at != seen.at && !self.sounds[seen.sounded..].contains(&"descend") {
            self.sounds.push("door_pass");
        }
        self.watch_items(&seen);
        self.chatter(seen.cued);
        super::scratch::give_seen_foes(seen.foes);
    }

    fn striker(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let foes = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0)
            .map(|e| (e.x, e.y));
        let shots = self
            .projectiles
            .iter()
            .filter(|p| p.hostile)
            .map(|p| (p.x, p.y));
        foes.chain(shots)
            .map(|(fx, fy)| ((fx, fy), (x - fx).hypot(y - fy)))
            .filter(|&(_, d)| d < 8.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(p, _)| p)
    }

    fn floor_step(&self) -> &'static str {
        if self.at_home_now() {
            return match self.room().kind {
                RoomKind::Tavern
                | RoomKind::Stables
                | RoomKind::Lists
                | RoomKind::Hall
                | RoomKind::Quarters => "step_wood",
                _ => "step_stone",
            };
        }
        match self.dungeon.pack {
            Pack::Crypt => "step_stone",
            Pack::Cavern => "step_gravel",
            Pack::Hellforge | Pack::Unknown => "step_ash",
            Pack::Archive | Pack::Swamp => "step_wet",
            Pack::Fungal => "step_moss",
        }
    }

    fn watch_heroes(&mut self, seen: &Seen) {
        let tick = self.tick;
        let step = self.floor_step();
        let circle = self.spectacle.circle;
        let mut sounds: Vec<&'static str> = Vec::new();
        let mut shake = 0;
        let mut downed: Vec<u32> = Vec::new();
        let strikers: Vec<(u32, Option<(f32, f32)>)> = seen
            .heroes
            .iter()
            .filter_map(|before| {
                let hero = self.players.get(&before.id)?;
                (hero.hp < before.hp).then(|| (before.id, self.striker(hero.x, hero.y)))
            })
            .collect();
        for before in &seen.heroes {
            let Some(hero) = self.players.get_mut(&before.id) else {
                continue;
            };
            hero.struck = hero.struck.saturating_sub(1);
            if before.hp > 0 && hero.hp < before.hp {
                if circle > 0 {
                    let more = (before.hp - hero.hp) * CIRCLE_HURT * circle / 100;
                    let least = before.hp.saturating_sub(hero.max_hp * super::hero::ONE_HIT_MOST / 100);
                    hero.hp = hero.hp.saturating_sub(more).max(least.min(hero.hp));
                }
                let lost = before.hp - hero.hp;
                hero.struck = STRUCK;
                hero.struck_big = lost.saturating_mul(100)
                    >= hero.max_hp.max(1).saturating_mul(HEAVY_HIT);
                hero.struck_dir = strikers
                    .iter()
                    .find(|(id, _)| *id == before.id)
                    .and_then(|(_, at)| *at)
                    .map_or_else(
                        || (-(hero.aim_x.round() as i8), -(hero.aim_y.round() as i8)),
                        |(fx, fy)| {
                            let (dx, dy) = (hero.x - fx, hero.y - fy);
                            let len = dx.hypot(dy).max(0.01);
                            ((dx / len).round() as i8, (dy / len).round() as i8)
                        },
                    );
                if hero.hp > 0 {
                    sounds.push(if hero.struck_big {
                        "knight_hit_big"
                    } else {
                        "knight_hit"
                    });
                }
                if hero.struck_big {
                    shake = shake.max(7);
                }
                if hero.hp == 0 {
                    hero.fell = tick.max(1);
                    hero.struck = 0;
                    downed.push(before.id);
                    sounds.push("knight_fall");
                    shake = shake.max(10);
                    if self.phase == Phase::Wiped {
                        sounds.push("bones_clatter");
                    }
                }
            } else if before.hp > 0 && hero.hp == 0 {
                hero.fell = tick.max(1);
                downed.push(before.id);
                sounds.push("knight_fall");
            }
            if hero.hp == 0 && hero.fell != 0 && tick == hero.fell + 9 {
                sounds.push("bones_clatter");
            }
            if hero.hp > 0 {
                hero.fell = 0;
            }
            if before.pop == 0 && hero.pop > 0 {
                if hero.gear_was == 0 && hero.gear >= 1 {
                    sounds.push("helmet_pop");
                }
            }
            if hero.gear >= 2 && hero.gear_was < 2 && hero.pop == gear::POP_TICKS - 5 {
                sounds.push("armour_fall");
            }
            if !before.shielding && hero.shielding {
                sounds.push("shield_up");
            }
            if before.spin == 0 && hero.spin > 0 {
                sounds.push("axe_spin");
            }
            if before.booby && hero.booby.is_none() && hero.invulnerable > 0 {
                sounds.push("helmet_pop");
                sounds.push("clang");
            }
            if hero.bombs < before.bombs {
                sounds.push("bomb_toss");
            }
            let moved = (hero.x - before.x).hypot(hero.y - before.y);
            if hero.hp > 0
                && !hero.stone
                && hero.dash_ticks == 0
                && hero.tossed == 0
                && hero.privy == 0
                && moved > 0.02
                && moved < 2.5
                && self.at == seen.at
            {
                hero.stride += moved;
                if hero.stride >= STRIDE {
                    hero.stride -= STRIDE;
                    if FOOTSTEPS {
                        sounds.push(step);
                    }
                }
            } else {
                hero.stride = 0.0;
            }
            if hero.hand.len() < before.hand.len()
                && let Some(card) = before
                    .hand
                    .iter()
                    .find(|c| hero.hand.iter().filter(|h| h == c).count() < before.hand.iter().filter(|b| b == c).count())
                    .and_then(|id| self.book.get(id))
            {
                sounds.push(played_sound(card));
            }
        }
        self.fx.push(Fx::Shake(shake));
        for id in downed {
            self.leave_cairn(id);
        }
        self.sounds.extend(sounds);
    }

    fn watch_foes(&mut self, seen: &Seen) {
        let struck = seen.foes.iter().any(|&(id, hp, _)| {
            self.enemies.iter().find(|e| e.id == id).is_none_or(|e| e.hp < hp)
        });
        if struck && let Some((by, sound)) = self.landed_sound(seen) {
            let fresh = &mut self.sounds[seen.sounded..];
            let plain = fresh.iter().filter(|&&s| s == "hit").count();
            if plain > 0 || fresh.is_empty() {
                let kept: Vec<&'static str> = fresh.iter().copied().filter(|&s| s != "hit").collect();
                self.sounds.truncate(seen.sounded);
                self.sounds.extend(kept);
            }
            let rung = self.sounds[seen.sounded..]
                .iter()
                .any(|s| *s == sound || melee::CONTACT_SOUNDS.contains(s) || vfx::IMPACT_SOUNDS.contains(s));
            if !rung && sound == super::bard::HIT {
                self.seat_sound_every(by, sound, LANDED_EVERY);
            } else if !rung {
                self.sound_every(sound, LANDED_EVERY);
            }
        }
        for e in &self.enemies {
            let before = seen.foes.iter().find(|f| f.0 == e.id);
            if e.is_boss() && before.is_none() && e.age < 2 * HZ {
                self.sounds.push("boss_roar");
            } else if e.raged && before.is_some_and(|f| !f.2) {
                self.sounds.push("boss_rage");
            }
        }
    }

    fn landed_sound(&self, seen: &Seen) -> Option<(u32, &'static str)> {
        let hurt: Vec<(f32, f32)> = seen
            .foes
            .iter()
            .filter_map(|&(id, hp, _)| {
                let e = self.enemies.iter().find(|e| e.id == id)?;
                (e.hp < hp).then_some((e.x, e.y))
            })
            .collect();
        let &(fx, fy) = hurt.first()?;
        let (&by, hero) = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .min_by(|(_, a), (_, b)| {
                let d = |h: &Hero| (h.x - fx).hypot(h.y - fy);
                d(a).total_cmp(&d(b))
            })?;
        let close = (hero.x - fx).hypot(hero.y - fy) < 4.2;
        Some((by, if hero.spin > 0 && close {
            "hit_axe"
        } else if hero.swing > 0 && close {
            if !hero.sword && matches!(hero.arm.as_deref(), Some("twin-axes" | "grave-axes" | "reaver-axes")) { "hit_axe" } else { "hit_blade" }
        } else if hero.forged.as_ref().is_some_and(|w| w.melee.is_some() && w.bolt.is_none()) {
            "hit_blade"
        } else if hero.forged.is_none()
            && (hero.weapon == Weapon::Notes || super::classes::of(hero).is_some_and(|c| c.name == "Bard"))
        {
            super::bard::HIT
        } else if hero.forged.as_ref().is_some_and(|w| w.bolt.is_some())
            || super::classes::shot_look(hero).is_some()
        {
            "hit_magic"
        } else {
            match hero.weapon {
                Weapon::Fists => "hit",
                Weapon::Bow => "hit_arrow",
                Weapon::Notes => super::bard::HIT,
                Weapon::Stones => "hit_magic",
                Weapon::Crossbow => "hit_bolt",
                Weapon::Handgonne => "hit_blast",
            }
        }))
    }

    fn watch_items(&mut self, seen: &Seen) {
        if self.at != seen.at {
            return;
        }
        let now: Vec<&str> = self.room().items.iter().map(|i| i.card.as_str()).collect();
        let taken = seen
            .items
            .iter()
            .filter(|id| {
                now.iter().filter(|n| *n == id).count()
                    < seen.items.iter().filter(|s| s == id).count()
            })
            .find_map(|id| self.book.get(id));
        let gold = taken.is_some_and(|card| {
            card.id == "gold"
                || card
                    .effects
                    .iter()
                    .any(|e| matches!(e, cards::Effect::Gold(_)))
        });
        if gold {
            self.sounds.retain(|&s| s != "card_pickup" && s != "spoil_pickup");
            self.sounds.push("gold");
        }
    }

    pub(super) fn leave_corpses(
        &mut self,
        fallen: &[(EnemyKind, f32, f32)],
        guardians: &[(usize, f32, f32)],
    ) {
        if !guardians.is_empty() || fallen.iter().any(|f| matches!(f.0,
            EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Risen)) {
            self.spectacle.phase = None;
        }
        for &(index, x, y) in guardians {
            let radius = self.bosses.get(index).map_or(1.5, |b| b.radius);
            self.spectacle.corpses.push(Corpse {
                kind: EnemyKind::Boss,
                boss: Some(index as u8),
                x,
                y,
                born: self.tick,
                room: self.at,
                radius,
            });
        }
        for &(kind, x, y) in fallen {
            if matches!(
                kind,
                EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Risen
            ) {
                self.spectacle.corpses.push(Corpse {
                    kind,
                    boss: None,
                    x,
                    y,
                    born: self.tick,
                    room: self.at,
                    radius: kind.radius(),
                });
            }
        }
        self.spectacle.corpses.truncate(8);
        if !guardians.is_empty()
            || fallen.iter().any(|f| {
                matches!(
                    f.0,
                    EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Risen
                )
            })
        {
            self.sounds.push("boss_death");
        }
    }

    fn tick_claims(&mut self) {
        let (tick, at) = (self.tick, self.at);
        self.spectacle
            .corpses
            .retain(|c| c.room == at && tick.saturating_sub(c.born) < u64::from(CLAIM) + 1);
        let mut sounds: Vec<&'static str> = Vec::new();
        let mut shake = 0;
        for c in &self.spectacle.corpses {
            let age = tick.saturating_sub(c.born);
            let marks = [
                (u64::from(LOOT_WAIT), "crack_open"),
                (u64::from(LOOT_WAIT + CRACK), "hand_rise"),
                (u64::from(LOOT_WAIT + CRACK + RISE), "hand_grab"),
                (u64::from(LOOT_WAIT + CRACK + RISE + GRAB), "hand_drag"),
                (
                    u64::from(LOOT_WAIT + CRACK + RISE + GRAB + PULL),
                    "ground_close",
                ),
            ];
            for (at, sound) in marks {
                if age == at {
                    sounds.push(sound);
                }
            }
            shake = shake.max(match c.beat(tick) {
                Beat::Cracking(_) => 2,
                Beat::Rising(_) => 3,
                Beat::Grabbing(_) => 4,
                Beat::Pulling(_) => 5,
                Beat::Closing(t) if t < 0.3 => 7,
                _ => 0,
            });
        }
        self.fx.push(Fx::Shake(shake));
        self.sounds.extend(sounds);
    }

    pub(super) fn raise_hell_gate(&mut self) {
        let room = self.room();
        let (x, y) = (room.width() / 2.0 + 11.0, room.height() / 2.0 + 6.0);
        self.spectacle.gate = Some(Gate {
            x,
            y,
            born: self.tick,
            room: self.at,
        });
        self.sounds.push("hell_gate_rise");
        self.cues.push("hell_gate".into());
        self.fx.push(Fx::Shake(14));
    }

    fn tick_gate(&mut self) {
        let (tick, at) = (self.tick, self.at);
        if self.spectacle.gate.is_some_and(|g| g.room != at) {
            self.spectacle.gate = None;
        }
        if let Some(g) = self.spectacle.gate
            && tick.saturating_sub(g.born) < u64::from(GATE_RISE)
        {
            self.fx.push(Fx::Shake(3));
        }
    }

    pub(super) fn step_into_gate(&mut self) -> bool {
        let Some(g) = self.spectacle.gate else {
            return false;
        };
        if g.risen(self.tick) < 1.0
            || !self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - g.x).hypot(h.y - g.y) < GATE_REACH)
        {
            return false;
        }
        self.enter_circle();
        true
    }

    fn enter_circle(&mut self) {
        self.spectacle.gate = None;
        self.spectacle.corpses.clear();
        self.spectacle.circle += 1;
        self.spectacle.circling = true;
        let floor = self.dungeon.depth;
        self.descend();
        self.spectacle.circling = false;
        if self.dungeon.depth != floor || self.crossroads.is_some() {
            for hero in self.players.values_mut() {
                hero.relieved = None;
                hero.pit.visits = 0;
            }
        }
        let circle = self.spectacle.circle;
        self.spectacle.card = Some((self.tick, circle));
        self.sounds.push("hell_gate_enter");
        self.sounds.push("circle_toll");
        self.fx.push(Fx::Shake(18));
        self.cues.push(format!("circle:{circle}"));
    }

    fn strength_of_circles(&mut self) {
        let circle = self.spectacle.circle;
        let from = self.spectacle.scaled_id;
        self.spectacle.scaled_id = self.next_id;
        if circle == 0 {
            return;
        }
        for e in self.enemies.iter_mut().filter(|e| e.id > from) {
            e.max_hp = e.max_hp * (100 + CIRCLE_HP * circle) / 100;
            e.hp = e.hp * (100 + CIRCLE_HP * circle) / 100;
            e.seen_hp = e.hp;
        }
    }

    pub fn circle_card(&self) -> Option<(u32, f32)> {
        let (at, circle) = self.spectacle.card?;
        let age = self.tick.saturating_sub(at);
        (age < u64::from(CARD_TICKS)).then(|| (circle, age as f32 / CARD_TICKS as f32))
    }
}

fn played_sound(card: &Card) -> &'static str {
    let name = card.name.to_lowercase();
    if name.contains("holy") {
        "holy_water"
    } else if card
        .effects
        .iter()
        .any(|e| matches!(e, cards::Effect::Heal(_)))
    {
        "potion"
    } else {
        "play_card"
    }
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    pub fn hurt_for_test(&mut self, id: u32, damage: u32) {
        let seen = self.seen();
        if let Some(hero) = self.players.get_mut(&id) {
            hero.hurt(damage);
        }
        self.watch(seen);
    }

}

#[cfg(test)]
#[path = "../../tests/rules/polish_feel__tests.rs"]
mod polish_feel_tests;
