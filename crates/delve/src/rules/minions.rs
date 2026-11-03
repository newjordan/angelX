//! A summoner's own company, after the barrow-lords of the old songs: the
//! Gravecaller keeps his dead at his own side, and none of them is the
//! party's. Where every other knight shares the one place at the party's
//! side (Beaumains, or a card's minion: `hireling`), he raises up to
//! [`KEPT`] of his own, and a fourth lets the oldest go back to the earth.
//!
//! **His spells** (keys 1 to 4, each a spell card with its own wait):
//!
//! - *Raise Ghoul*: a ghoul claws up through the flags beside him and
//!   fights.
//! - *Barrow Warden*: an old knight of the mounds stands up with his
//!   shield, keeps between his caller and what comes, takes the shots
//!   meant for him at half their weight, and roars the foes near him onto
//!   himself.
//! - *Bone Totem*: a stake of skulls is planted where he aims; it spits
//!   grave-fire at the nearest foe, never moves, and breaks when beaten.
//! - *Soul Rip*: where he aims, the souls are torn out of every foe near,
//!   and what is torn mends him and his dead.
//!
//! His aura pulses without a key. Settled foes near him, or killed by him
//! or his dead, fill his corpse bar; holding G raises a golem at its tier.
//! Space keeps its guard and raises five dancing skeletons ahead of him.
//! The golem and wall take none of the places at [`KEPT`].
//!
//! **His ultimate**, the Tombstone (R): a grave slams down a few steps
//! ahead along his aim (at his feet if a wall is in the way), and for
//! [`TOMB_TICKS`] it raises a zombie every second, up to [`TOMB_ZOMBIES`]
//! at once. Zombies shamble to the nearest foe, take hold of it (it walks
//! slowly while held) and bite. The foes see the grave and can break it;
//! when it falls, or its time runs out, its zombies crumble.
//!
//! **Command.** Every minion with a caller (his, or a card's in the
//! party's place) goes for what its caller last struck or aims at while he
//! fights, before anything nearer. Mid-fight the vigil key (G), which never
//! kneels a knight to the vigil while the room fights, calls them to heel
//! instead, or sends them back to the hunt: at heel they keep to their
//! caller and fall only on what comes within [`HEEL_GUARD`] of him. A
//! tombstone's zombies hear no orders.
//!
//! None of the dead lays a hand on a guardian, the dragon or the Pit
//! Tyrant (`hireling`); a totem's grave-fire and the Soul Rip hit
//! whatever they meet.

use super::hireling::{AllyKind, Hireling};
use super::*;

/// The kit that keeps its own dead.
pub const GRAVECALLER: &str = "gravecaller";
/// How many spell minions a summoner keeps, his golem, wall and the
/// tombstone's zombies apart.
pub const KEPT: usize = 3;
/// At heel, what a minion falls on: foes this near its caller.
pub const HEEL_GUARD: f32 = 5.0;
/// A raised minion climbs out of the ground for half a second, out of the
/// fight while it does.
pub const RISE: u32 = HZ / 2;
/// A totem is planted this far along its caller's aim, or nearer.
const PLANT_REACH: f32 = 4.0;
/// What a summoner marks for his minions while he fights: the nearest foe
/// within this cone of his aim (its cosine, about 25°) and this reach.
const MARK_CONE: f32 = 0.9;
const MARK_REACH: f32 = 16.0;
/// How long a summoner's raised staff and his rite's light are seen.
pub const CAST_SHOWN: u32 = HZ / 2;

/// The aura's beat, reach, light and damage; bodies gathered near the
/// caller or credited to him fill eight points each.
pub const AURA_EVERY: u32 = 54;
pub const AURA_REACH: f32 = 6.0;
pub const AURA_SHOWN: u32 = 24;
const AURA_DAMAGE: u32 = 18;
const CORPSE_REACH: f32 = 16.0;
const CORPSE_GAIN: u8 = 8;
/// Six tenths of a second on G; ten seconds of wall, rearmed in eighteen.
pub const GOLEM_HOLD: u32 = 18;
pub const WALL_TICKS: u32 = 300;
pub const WALL_REARM: u32 = 540;
pub const WALL_RADIUS: f32 = 1.0;
pub const WALL_COUNT: usize = 5;

/// The caller's gathered bodies, aura and wall. Missing fields in an old
/// save start empty; a held G releases once, until it is let go.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grave {
    pub corpses: u8,
    pub aura: u32,
    pub pulse: Option<Pulse>,
    pub wall: u32,
    pub wall_rearm: u32,
    pub guard_key: bool,
    pub hold: u32,
}

impl Default for Grave {
    fn default() -> Self {
        Self { corpses: 0, aura: AURA_EVERY, pulse: None, wall: 0, wall_rearm: 0, guard_key: false, hold: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pulse {
    pub tick: u64,
    pub x: f32,
    pub y: f32,
}

pub fn golem_kind(percent: u8) -> AllyKind {
    match percent {
        0..=33 => AllyKind::FleshGolem,
        34..=66 => AllyKind::BoneGolem,
        67..=99 => AllyKind::GraveGolem,
        _ => AllyKind::Colossus,
    }
}

/// The Tombstone: how long it stands, how much it takes, how often it
/// raises a zombie and how many walk at once, how far ahead it lands, and
/// how wide it is.
pub const TOMB_TICKS: u32 = 8 * HZ;
pub const TOMB_HEALTH: u32 = 240;
const TOMB_RAISE: u32 = HZ;
pub const TOMB_ZOMBIES: usize = 5;
const TOMB_REACH: f32 = 3.5;
pub const TOMB_RADIUS: f32 = 0.9;
/// A foe's blow on the grave, and the grace after it, as a minion's.
const TOMB_KNOCK: u32 = 12;
const TOMB_GRACE: u32 = 24;

/// Soul Rip: how far ahead of its caster it lands, how wide it tears, what
/// each foe torn gives back to him, and what it mends each of his dead.
const RIP_AHEAD: f32 = 5.0;
pub const RIP_REACH: f32 = 3.0;
const RIP_MEND: u32 = 4;
const RIP_MEND_DEAD: u32 = 15;

/// What a summoner has told his minions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    /// Out after what he marks, or the nearest foe.
    #[default]
    Hunt,
    /// At his side, falling only on what comes at him.
    Heel,
}

impl Order {
    pub fn word(self) -> &'static str {
        match self {
            Order::Hunt => "hunt",
            Order::Heel => "heel",
        }
    }
}

/// A Tombstone standing: whose, where, how much it has left to take, its
/// time left, and the ticks until its next zombie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tombstone {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub left: u32,
    pub raise: u32,
    pub invulnerable: u32,
    /// Ticks since it slammed down, for the slam and the zombies' light.
    pub age: u32,
}

/// What a summoner last did with his staff, for the raised staff and the
/// rite's light: a minion called, a soul rip, a grave planted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rite {
    Raise,
    Rip,
    Tomb,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cast {
    pub tick: u64,
    pub rite: Rite,
    pub x: f32,
    pub y: f32,
    /// The foes a soul rip tore, where they stood.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub torn: Vec<(f32, f32)>,
}

/// Every summoner's dead, their orders and their marks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pack {
    /// Each with its `summoner`; the tombstones' zombies among them.
    pub raised: Vec<Hireling>,
    pub orders: BTreeMap<u32, Order>,
    /// The foe (by id) each knight with minions last marked.
    pub quarry: BTreeMap<u32, u32>,
    pub stones: Vec<Tombstone>,
    pub casts: BTreeMap<u32, Cast>,
    pub graves: BTreeMap<u32, Grave>,
}

/// Whether knight `hero` keeps their own dead.
pub fn summoner(hero: &Hero) -> bool {
    hero.knight.as_deref() == Some(GRAVECALLER) && hero.shape.is_none()
}

/// Whether knight `id` has minions of their own to command: raised at
/// their side (not a tombstone's zombies), or a card's in the party's
/// place.
pub(super) fn keeps(pack: &Pack, slot: &Option<Hireling>, id: u32) -> bool {
    pack.raised
        .iter()
        .any(|m| m.summoner == Some(id) && m.kind != AllyKind::Zombie && !m.kind.guild_squire() && !m.down())
        || slot.as_ref().is_some_and(|h| h.summoner == Some(id) && !h.down())
}

impl Pack {
    pub fn is_empty(&self) -> bool {
        *self == Pack::default()
    }

    pub fn valid(&self) -> bool {
        let point = |x: f32, y: f32| x.is_finite() && y.is_finite();
        self.raised.len() <= MAX_PLAYERS as usize * (KEPT + TOMB_ZOMBIES + WALL_COUNT + 2)
            && self.raised.iter().all(|m| point(m.x, m.y) && m.summoner.is_some())
            && self.orders.len() <= MAX_PLAYERS as usize
            && self.quarry.len() <= MAX_PLAYERS as usize
            && self.stones.len() <= MAX_PLAYERS as usize
            && self.stones.iter().all(|s| point(s.x, s.y))
            && self.casts.len() <= MAX_PLAYERS as usize
            && self.casts.values().all(|c| point(c.x, c.y) && c.torn.len() <= 16)
            && self.graves.len() <= MAX_PLAYERS as usize
            && self.graves.values().all(|g| {
                g.corpses <= 100 && g.aura <= AURA_EVERY && g.wall <= WALL_TICKS
                    && g.wall_rearm <= WALL_REARM && g.hold <= GOLEM_HOLD + 1
                    && g.pulse.as_ref().is_none_or(|p| point(p.x, p.y))
            })
    }

    /// What knight `id` has told their minions.
    pub fn order(&self, id: u32) -> Order {
        self.orders.get(&id).copied().unwrap_or_default()
    }

    /// The minions and graves standing where the foes can see them: they
    /// draw fire as Beaumains does.
    pub(super) fn decoys(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.raised
            .iter()
            .filter(|m| !m.down() && m.rising == 0)
            .map(|m| (m.x, m.y))
            .chain(self.stones.iter().map(|s| (s.x, s.y)))
    }

    /// A warden roaring near `enemy` is all it sees: `seen` holds only
    /// him. A great foe pays no heed.
    pub(super) fn roared(&self, enemy: &Enemy, seen: &mut Vec<(f32, f32)>) {
        if hireling::great(enemy) {
            return;
        }
        let near = |m: &&Hireling| (m.x - enemy.x).hypot(m.y - enemy.y);
        if let Some(warden) = self
            .raised
            .iter()
            .filter(|m| (m.kind == AllyKind::Warden || m.kind == AllyKind::Shieldbearer) && m.taunt > 0 && !m.down())
            .filter(|m| near(m) <= hireling::ROAR_REACH + enemy.radius())
            .min_by(|a, b| near(a).total_cmp(&near(b)))
        {
            seen.clear();
            seen.push((warden.x, warden.y));
        }
    }
}

impl Run {
    /// The aura beats without a key. Space keeps its guard and also calls
    /// the wall; holding G spends the bodies without taking its tap away.
    pub(super) fn tick_gravecallers(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.minions.graves.retain(|id, _| self.players.contains_key(id));
        let callers: Vec<u32> = self.players.iter().filter(|(_, h)| summoner(h)).map(|(&id, _)| id).collect();
        for owner in callers {
            let hero = &self.players[&owner];
            let input = inputs.get(&owner).copied().filter(|i| i.valid()).unwrap_or_default();
            let active = hero.hp > 0 && !hero.stone && !hero.frog() && hero.tossed == 0
                && hero.privy == 0 && hero.pit.stop.is_none() && hero.pit.result.is_none() && hero.possessed.is_none();
            let can_hold = active || (hero.hp > 0 && hero.vigil && !hero.away);
            let grave = self.minions.graves.entry(owner).or_default();
            grave.wall_rearm = grave.wall_rearm.saturating_sub(1);
            grave.wall = if active { grave.wall.saturating_sub(1) } else { 0 };
            if grave.pulse.as_ref().is_some_and(|p| self.tick.saturating_sub(p.tick) >= u64::from(AURA_SHOWN)) {
                grave.pulse = None;
            }
            grave.hold = if input.vigil && can_hold { (grave.hold + 1).min(GOLEM_HOLD + 1) } else { 0 };
            let release = grave.hold == GOLEM_HOLD;
            let wall = active && input.dash && !grave.guard_key && grave.wall_rearm == 0;
            grave.guard_key = input.dash;
            if active {
                grave.aura = grave.aura.saturating_sub(1);
            }
            let pulse = active && grave.aura == 0;
            if pulse {
                grave.aura = AURA_EVERY;
            }
            if release {
                self.release_golem(owner);
            }
            if wall {
                self.skeleton_wall(owner);
            }
            if pulse {
                self.grave_pulse(owner);
            }
        }
        self.walk_skeletons();
    }

    fn grave_pulse(&mut self, owner: u32) {
        let hero = &self.players[&owner];
        let at = (hero.x, hero.y);
        let damage = hero.scaled(AURA_DAMAGE);
        for (i, dealt) in aoe::strike(&mut self.enemies, at, damage, |e| {
            e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH
                && (e.x - at.0).hypot(e.y - at.1) <= AURA_REACH + e.radius()
        }) {
            let enemy = &mut self.enemies[i];
            enemy.wits.foe = owner;
            self.feel.credit(owner, enemy.id, dealt, self.tick);
            if dealt > 0 && enemy.is_boss() {
                self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
            }
        }
        self.minions.graves.entry(owner).or_default().pulse = Some(Pulse { tick: self.tick, x: at.0, y: at.1 });
    }

    /// Called only for settled deaths, so a body never fills the bar twice.
    pub(super) fn gather_corpses(&mut self, slain: &[gore::Slain]) {
        for (&owner, hero) in self.players.iter().filter(|(_, h)| summoner(h) && h.hp > 0) {
            let count = slain.iter().filter(|s| s.kind != EnemyKind::Dummy
                && (s.foe == owner || (s.x - hero.x).hypot(s.y - hero.y) <= CORPSE_REACH)).count();
            if count > 0 {
                let grave = self.minions.graves.entry(owner).or_default();
                grave.corpses = (u32::from(grave.corpses) + count as u32 * u32::from(CORPSE_GAIN)).min(100) as u8;
            }
        }
    }

    fn release_golem(&mut self, owner: u32) {
        let Some(hero) = self.players.get(&owner).filter(|h| h.hp > 0 && summoner(h)) else { return };
        let percent = self.minions.graves.get(&owner).map_or(0, |g| g.corpses);
        if percent == 0 {
            return;
        }
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let at = hireling::beside(&grid, (hero.x, hero.y));
        let kind = golem_kind(percent);
        let mut golem = Hireling::minion(kind, 0, self.dungeon.depth, at, owner);
        golem.rising = RISE;
        golem.max_hp += self.gifts_for(&golem).hp;
        golem.hp = golem.max_hp;
        self.minions.raised.retain(|m| m.summoner != Some(owner) || !m.kind.golem());
        self.minions.raised.push(golem);
        self.minions.graves.entry(owner).or_default().corpses = 0;
        self.minions.casts.insert(owner, Cast { tick: self.tick, rite: Rite::Raise, x: at.0, y: at.1, torn: Vec::new() });
        self.found = Some((self.tick, owner, format!("{} RAISED", kind.name().to_uppercase())));
        self.cues.push(format!("summoned:{}", kind.word()));
        self.sounds.push("hand_rise");
    }

    fn skeleton_wall(&mut self, owner: u32) {
        let hero = &self.players[&owner];
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let mut wall = Vec::new();
        for place in 0..WALL_COUNT {
            let at = wall_spot(&grid, hero, place);
            let mut skeleton = Hireling::minion(AllyKind::Skeleton, place as u32, self.dungeon.depth, at, owner);
            skeleton.max_hp += self.gifts_for(&skeleton).hp;
            skeleton.hp = skeleton.max_hp;
            wall.push(skeleton);
        }
        self.minions.raised.retain(|m| m.summoner != Some(owner) || m.kind != AllyKind::Skeleton);
        self.minions.raised.extend(wall);
        let grave = self.minions.graves.entry(owner).or_default();
        grave.wall = WALL_TICKS;
        grave.wall_rearm = WALL_REARM;
        self.cues.push("skeleton_wall".into());
        self.sounds.push("hand_rise");
    }

    fn walk_skeletons(&mut self) {
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        self.minions.raised.retain(|m| m.kind != AllyKind::Skeleton || (!m.down()
            && m.summoner.and_then(|id| self.minions.graves.get(&id)).is_some_and(|g| g.wall > 0)
            && m.summoner.and_then(|id| self.players.get(&id)).is_some_and(|h| summoner(h) && h.hp > 0 && !h.stone)));
        for skeleton in self.minions.raised.iter_mut().filter(|m| m.kind == AllyKind::Skeleton) {
            let hero = &self.players[&skeleton.summoner.unwrap()];
            let at = wall_spot(&grid, hero, skeleton.power.min(4) as usize);
            let (dx, dy) = (at.0 - skeleton.x, at.1 - skeleton.y);
            let distance = dx.hypot(dy);
            let stride = distance.min(0.65);
            let (ux, uy) = unit(dx, dy);
            (skeleton.x, skeleton.y) = grid.slide((skeleton.x, skeleton.y), (ux * stride, uy * stride), HERO_RADIUS, Mover::Hero);
            skeleton.facing = if hero.aim_x < 0.0 { -1.0 } else { 1.0 };
            skeleton.invulnerable = skeleton.invulnerable.saturating_sub(1);
        }
    }

    /// The dancers hold their ground and take the blows of bodies meeting
    /// them. The shot flight checks their swept guard before a knight's.
    pub(super) fn skeletons_block(&mut self) {
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        for skeleton in self.minions.raised.iter_mut().filter(|m| m.kind == AllyKind::Skeleton && !m.down()) {
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy && !e.buried()) {
                let (dx, dy) = (enemy.x - skeleton.x, enemy.y - skeleton.y);
                let distance = dx.hypot(dy);
                let reach = enemy.radius() + WALL_RADIUS;
                if distance >= reach {
                    continue;
                }
                if enemy.frozen == 0 && !enemy.asleep(self.tick) {
                    skeleton.hurt(12);
                }
                let (ux, uy) = if distance > 0.01 { unit(dx, dy) } else { (skeleton.facing, 0.0) };
                let mover = if enemy.kind.flies() { Mover::Flier } else { Mover::Walker };
                (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), (ux * (reach - distance), uy * (reach - distance)), enemy.radius(), mover);
                if skeleton.down() {
                    break;
                }
            }
        }
    }

    /// Knight `owner` played or cast card `card_id`: the minion it calls,
    /// or the soul rip it tears.
    pub(super) fn call_minion(&mut self, owner: u32, card_id: &str) {
        let Some(card) = self.book.get(card_id) else {
            return;
        };
        let rips: Vec<u32> = card
            .effects
            .iter()
            .filter_map(|e| match *e {
                cards::Effect::Rip(damage) => Some(damage),
                _ => None,
            })
            .collect();
        let call = card.minion();
        for damage in rips {
            self.soul_rip(owner, card_id, damage);
        }
        if let Some((kind, power)) = call {
            self.raise(owner, kind, power);
        }
    }

    /// Knight `owner` calls a `kind`: at their own side if they keep their
    /// own dead (a fourth lets the oldest go), else in the party's place,
    /// which Beaumains never gives up. A totem is planted along the aim.
    pub(super) fn raise(&mut self, owner: u32, kind: AllyKind, power: u32) {
        let Some(hero) = self.players.get(&owner).filter(|h| h.hp > 0) else {
            return;
        };
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let at = if kind == AllyKind::Totem {
            along(&grid, hero, PLANT_REACH, HERO_RADIUS)
        } else {
            hireling::beside(&grid, (hero.x, hero.y))
        };
        let depth = self.dungeon.depth;
        if !summoner(hero) {
            if self.hireling.as_ref().is_some_and(|h| h.kind == AllyKind::Beaumains) {
                self.found = Some((self.tick, owner, "Beaumains will not share the road with it".into()));
                return;
            }
            self.hireling = Some(Hireling::minion(kind, power, depth, at, owner));
            self.cues.push(format!("summoned:{}", kind.word()));
            return;
        }
        let mine: Vec<usize> = self
            .minions
            .raised
            .iter()
            .enumerate()
            .filter(|(_, m)| m.summoner == Some(owner) && m.kind.kept())
            .map(|(i, _)| i)
            .collect();
        if mine.len() >= KEPT {
            let gone = self.minions.raised.remove(mine[0]);
            self.cues.push(format!("crumbled:{}", gone.kind.word()));
        }
        let mut minion = Hireling::minion(kind, power, depth, at, owner);
        minion.rising = RISE;
        // As hardy as his wishes for his dead make them (`dead_wishes`).
        minion.max_hp += self.gifts_for(&minion).hp;
        minion.hp = minion.max_hp;
        self.minions.raised.push(minion);
        self.minions.casts.insert(owner, Cast { tick: self.tick, rite: Rite::Raise, x: at.0, y: at.1, torn: Vec::new() });
        self.cues.push(format!("summoned:{}", kind.word()));
        self.sounds.push("hand_rise");
    }

    /// Soul Rip: every foe within reach of the point ahead of knight
    /// `owner`'s aim takes `damage` (one stroke, `aoe`), and what is torn
    /// mends the knight and every one of their dead.
    fn soul_rip(&mut self, owner: u32, spell: &str, damage: u32) {
        let Some(hero) = self.players.get(&owner).filter(|h| h.hp > 0) else {
            return;
        };
        let damage = hero.spell_damage(spell, damage);
        let reach = hero.spell_step(spell).rip_reach;
        let mend = hero.spell_power(spell, RIP_MEND);
        let mend_dead = hero.spell_power(spell, RIP_MEND_DEAD);
        let (w, h) = (self.room().width(), self.room().height());
        let at = (
            (hero.x + hero.aim_x * RIP_AHEAD).clamp(1.0, w - 1.0),
            (hero.y + hero.aim_y * RIP_AHEAD).clamp(1.0, h - 1.0),
        );
        let caught = aoe::reached(&self.enemies, at, |e| {
            e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH && (e.x - at.0).hypot(e.y - at.1) <= reach + e.radius()
        });
        let mut torn = Vec::new();
        for (rank, i) in caught.into_iter().enumerate() {
            let enemy = &mut self.enemies[i];
            let dealt = aoe::part(damage, rank).min(enemy.hp);
            enemy.hp -= dealt;
            enemy.wits.foe = owner;
            if dealt > 0 && enemy.is_boss() {
                self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
            }
            self.feel.credit(owner, enemy.id, dealt, self.tick);
            if torn.len() < 16 {
                torn.push((enemy.x, enemy.y));
            }
        }
        if let Some(hero) = self.players.get_mut(&owner) {
            hero.leech(mend * torn.len() as u32, stakes::Mend::Party);
        }
        if !torn.is_empty() {
            for dead in self.minions.raised.iter_mut().filter(|m| m.summoner == Some(owner) && !m.down()) {
                dead.hp = (dead.hp + mend_dead).min(dead.max_hp);
            }
        }
        self.minions.casts.insert(owner, Cast { tick: self.tick, rite: Rite::Rip, x: at.0, y: at.1, torn });
        self.cues.push("soul_rip".into());
        self.sounds.push("hand_grab");
    }

    /// The Tombstone slams down along knight `owner`'s aim: a few steps
    /// ahead, so its dead rise between the knight and what comes, or
    /// nearer where a wall stands, or at the feet. A grave of theirs
    /// already standing falls first, its zombies with it.
    pub(super) fn plant_tombstone(&mut self, owner: u32) {
        let Some(hero) = self.players.get(&owner) else {
            return;
        };
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let (x, y) = along(&grid, hero, TOMB_REACH, TOMB_RADIUS);
        self.minions.stones.retain(|s| s.owner != owner);
        self.minions.raised.retain(|m| !(m.summoner == Some(owner) && m.kind == AllyKind::Zombie));
        self.minions.stones.push(Tombstone {
            owner,
            x,
            y,
            hp: TOMB_HEALTH,
            left: TOMB_TICKS,
            raise: 0,
            invulnerable: 0,
            age: 0,
        });
        self.minions.casts.insert(owner, Cast { tick: self.tick, rite: Rite::Tomb, x, y, torn: Vec::new() });
        self.fx.push(Fx::Shake(8));
        self.sounds.push("ground_close");
    }

    /// Each tick: the marks, the graves and their zombies, and every
    /// summoner's dead. A minion whose caller has left goes with them.
    pub(super) fn tick_minions(&mut self) {
        if self.minions.is_empty() && !self.hireling.as_ref().is_some_and(|h| h.summoner.is_some()) {
            return;
        }
        let here: Vec<u32> = self.players.keys().copied().collect();
        let pack = &mut self.minions;
        pack.raised.retain(|m| m.summoner.is_some_and(|id| here.contains(&id)));
        pack.stones.retain(|s| here.contains(&s.owner));
        pack.orders.retain(|id, _| here.contains(id));
        let shown = self.tick.saturating_sub(u64::from(CAST_SHOWN));
        pack.casts.retain(|id, c| here.contains(id) && c.tick > shown);
        self.fit_dead();
        self.mark_quarry();
        self.tick_stones();
        let mut raised = std::mem::take(&mut self.minions.raised);
        for minion in &mut raised {
            self.drive_hireling(minion);
        }
        raised.retain(|m| !(m.down() && m.kind.breaks()));
        // What the drive called up meanwhile stays (nothing does today).
        raised.append(&mut self.minions.raised);
        self.minions.raised = raised;
    }

    /// Each knight with minions marks a foe for them: the nearest in the
    /// cone of their aim while they fight, else the one they just struck.
    /// A fallen quarry is forgotten.
    fn mark_quarry(&mut self) {
        let keepers: Vec<u32> = self
            .players
            .iter()
            .filter(|&(&id, h)| h.hp > 0 && keeps(&self.minions, &self.hireling, id))
            .map(|(&id, _)| id)
            .collect();
        for id in keepers {
            let hero = &self.players[&id];
            let (hx, hy) = (hero.x, hero.y);
            let fair = |e: &&Enemy| e.hp > 0 && e.age >= TELEGRAPH && e.kind != EnemyKind::Dummy;
            let near = |e: &&Enemy| (e.x - hx).hypot(e.y - hy);
            let aim = unit(hero.aim_x, hero.aim_y);
            let aimed = (hero.fire_cooldown > 0 || hero.swing > 0)
                .then(|| {
                    self.enemies
                        .iter()
                        .filter(fair)
                        .filter(|e| {
                            let d = near(e);
                            let (ux, uy) = unit(e.x - hx, e.y - hy);
                            d <= MARK_REACH && (d < 0.5 || ux * aim.0 + uy * aim.1 >= MARK_CONE)
                        })
                        .min_by(|a, b| near(a).total_cmp(&near(b)))
                        .map(|e| e.id)
                })
                .flatten();
            let struck = self
                .enemies
                .iter()
                .filter(fair)
                .filter(|e| e.hp < e.seen_hp && e.wits.foe == id)
                .min_by(|a, b| near(a).total_cmp(&near(b)))
                .map(|e| e.id);
            if let Some(foe) = aimed.or(struck) {
                self.minions.quarry.insert(id, foe);
            }
        }
        let enemies = &self.enemies;
        self.minions
            .quarry
            .retain(|_, foe| enemies.iter().any(|e| e.id == *foe && e.hp > 0));
    }

    /// The graves: their time runs down, the foes' shots and blows wear
    /// them, and each raises its zombies. A grave fallen or spent takes its
    /// zombies down with it.
    fn tick_stones(&mut self) {
        let mut stones = std::mem::take(&mut self.minions.stones);
        for stone in &mut stones {
            stone.age += 1;
            stone.left = stone.left.saturating_sub(1);
            stone.invulnerable = stone.invulnerable.saturating_sub(1);
            for shot in self.projectiles.iter_mut().filter(|p| {
                p.hostile
                    && p.ttl > 0
                    && p.in_flight()
                    && p.traits.path.beam.is_none()
                    && p.traits.path.form != patterns::Form::Lob
            }) {
                if (shot.x - stone.x).hypot(shot.y - stone.y) < TOMB_RADIUS + patterns::girth(shot) {
                    shot.ttl = 0;
                    stone.hp = stone.hp.saturating_sub(shot.damage);
                }
            }
            if stone.invulnerable == 0
                && self.enemies.iter().any(|e| {
                    e.hp > 0
                        && e.age >= TELEGRAPH
                        && e.frozen == 0
                        && !e.asleep(self.tick)
                        && e.kind != EnemyKind::Dummy
                        && (stone.x - e.x).hypot(stone.y - e.y) < e.radius() + TOMB_RADIUS + 0.3
                })
            {
                stone.hp = stone.hp.saturating_sub(TOMB_KNOCK);
                stone.invulnerable = TOMB_GRACE;
            }
            stone.raise = stone.raise.saturating_sub(1);
            let walking = self
                .minions
                .raised
                .iter()
                .filter(|m| m.summoner == Some(stone.owner) && m.kind == AllyKind::Zombie)
                .count();
            if stone.hp > 0 && stone.left > 0 && stone.raise == 0 && walking < TOMB_ZOMBIES {
                stone.raise = TOMB_RAISE;
                let grid = Grid {
                    room: &self.dungeon.rooms[self.at],
                    barred: true,
                };
                let turn = stone.age as f32 * 0.37 + walking as f32 * 2.4;
                let at = (0..6)
                    .map(|k| {
                        let a = turn + k as f32 * 1.05;
                        (stone.x + a.cos() * 1.6, stone.y + a.sin() * 1.3)
                    })
                    .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
                    .unwrap_or((stone.x, stone.y + 1.0));
                let mut zombie = Hireling::minion(AllyKind::Zombie, 0, self.dungeon.depth, at, stone.owner);
                zombie.rising = RISE;
                zombie.max_hp += self.gifts_for(&zombie).hp;
                zombie.hp = zombie.max_hp;
                self.minions.raised.push(zombie);
                self.cues.push("summoned:zombie".into());
            }
        }
        for stone in stones.iter().filter(|s| s.hp == 0 || s.left == 0) {
            self.minions
                .raised
                .retain(|m| !(m.summoner == Some(stone.owner) && m.kind == AllyKind::Zombie));
            self.cues.push(if stone.hp == 0 { "tombstone_broken" } else { "tombstone_spent" }.into());
            if self.blasts.len() < 16 {
                self.blasts.push((stone.x, stone.y, BLAST_TICKS));
            }
            self.sounds.push("crack_open");
        }
        stones.retain(|s| s.hp > 0 && s.left > 0);
        stones.append(&mut self.minions.stones);
        self.minions.stones = stones;
    }

    /// Into a new room: each summoner's dead come through the door beside
    /// them; a totem, a grave and its zombies stay where they stood.
    pub(super) fn minions_follow(&mut self) {
        if self.minions.is_empty() {
            return;
        }
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: true,
        };
        let lead = self.players.values().find(|h| h.hp > 0).map(|h| (h.x, h.y));
        let pack = &mut self.minions;
        pack.raised.retain(|m| !m.kind.breaks());
        pack.stones.clear();
        pack.quarry.clear();
        pack.casts.clear();
        for grave in pack.graves.values_mut() {
            grave.wall = 0;
            grave.pulse = None;
        }
        for minion in &mut pack.raised {
            let Some(at) = minion
                .summoner
                .and_then(|id| self.players.get(&id))
                .filter(|h| h.hp > 0)
                .map(|h| (h.x, h.y))
                .or(lead)
            else {
                continue;
            };
            (minion.x, minion.y) = hireling::beside(&grid, at);
            minion.invulnerable = minion.invulnerable.max(20);
            minion.taunt = 0;
        }
    }
}

/// The point faces the aim; the next two ranks spread behind it.
fn wall_spot(grid: &Grid, hero: &Hero, place: usize) -> (f32, f32) {
    let (ahead, side) = [(3.6, 0.0), (2.4, -1.0), (2.4, 1.0), (1.2, -2.0), (1.2, 2.0)][place];
    let aim = unit(hero.aim_x, hero.aim_y);
    let (ax, ay) = if aim == (0.0, 0.0) { (1.0, 0.0) } else { aim };
    let step = ((ax * ahead - ay * side) / 12.0, (ay * ahead + ax * side) / 12.0);
    let mut at = (hero.x, hero.y);
    for _ in 0..12 {
        at = grid.slide(at, step, HERO_RADIUS, Mover::Hero);
    }
    at
}

/// Open floor along `hero`'s aim, `reach` ahead at most, for a thing of
/// `radius`: the farthest of four steps that is clear, else the feet.
fn along(grid: &Grid, hero: &Hero, reach: f32, radius: f32) -> (f32, f32) {
    let (ax, ay) = unit(hero.aim_x, hero.aim_y);
    [1.0, 0.75, 0.5, 0.25]
        .into_iter()
        .map(|k| (hero.x + ax * reach * k, hero.y + ay * reach * k))
        .find(|&(x, y)| grid.clear(x, y, radius, Mover::Hero))
        .unwrap_or((hero.x, hero.y))
}

#[cfg(test)]
#[path = "../../tests/rules/together_minions__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/gravecaller__tests.rs"]
mod gravecaller_tests;
