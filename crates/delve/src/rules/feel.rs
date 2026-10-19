//! How the arms feel: hits that land, power you can see, a meter for the
//! skills, and a ledger of what friends actually did.
//!
//! - **Hits land.** A knight's hit jolts an ordinary monster still for
//!   `JOLT_HIT` ticks (no more often than every `FLINCH` ticks, so a stream
//!   never pins it) and knocks it back by the arm (`knock`). Sparks fly
//!   where a shot strikes; arrows and bolts stick in the stone until the
//!   room ends; the crossbow and the handgonne flash at the muzzle and the
//!   handgonne kicks the knight back and smokes. A death holds a white
//!   frame for `DEATH_HOLD` ticks, then bursts.
//! - **The room's end pays.** Every monster shot still in the air when the
//!   last foe falls turns into a gold coin (`COIN_GOLD`, at most `COINS`)
//!   and flies to the nearest knight.
//! - **Valour.** Three pips, filled by hits landed (`VALOUR_HIT` a hit)
//!   and by grazes (a monster shot passing within `GRAZE` of a knight
//!   without hitting, once a shot). A skill costs a pip (`skills`).
//! - **Tempering.** Picking up the arm already held tempers it, up to
//!   `TEMPER_MAX` (`loot::Weapon::arms` for what each step gives).
//! - **Synergies**, named when they happen: Fire Arrows, Ricochet, Powder
//!   Keg.
//! - **The feel ledger**: with `ANGEL_DELVE_FEEL=1`, one line per knight per
//!   room (shots, hit rate, time to kill, dead air, skills, grazes, kills)
//!   for the cockpit to write beside the delve's save.

use super::*;

/// A hit's jolt on a monster, the ticks before the next may land, and the
/// white frame a death holds before it bursts.
pub const JOLT_HIT: u32 = 2;
pub const FLINCH: u8 = 15;
pub const DEATH_HOLD: u8 = 4;
pub const DEATH_TICKS: u8 = DEATH_HOLD + 10;
/// The shake of a handgonne shot, and of a kill by anything heavy.
pub const SHAKE_SHOT: u32 = 2;
pub const SHAKE_KILL: u32 = 3;
/// The handgonne's kick on the knight who fires it, in arena units.
pub const RECOIL: f32 = 0.6;
/// Gold a cancelled shot is worth, the most coins a room's end makes, and
/// how fast a coin flies home.
pub const COIN_GOLD: u32 = 1;
pub const COINS: usize = 24;
const COIN_SPEED: f32 = 22.0;
const COIN_CATCH: f32 = 0.9;
/// Valour: the units in a pip, what a hit and a graze earn, the pips.
pub const VALOUR_PIP: u32 = 600;
pub const VALOUR_HIT: u32 = 120;
pub const GRAZE_VALOUR: u32 = 120;
pub const PIPS: u32 = 3;
/// A graze: a monster shot passing this near a knight without hitting.
pub const GRAZE: f32 = 0.5;
/// Tempering: the most times an arm can be tempered.
pub const TEMPER_MAX: u8 = 3;
/// Fire Arrows: ticks the bow's arrows burn after a Fire Pit.
pub const FIRE_ARROWS: u32 = 4 * HZ;
/// Visual lists' caps: sparks, stuck shafts, puffs, deaths, tracers,
/// shield blocks.
const IMPACTS: usize = 48;
const STUCK: usize = 24;
const PUFFS: usize = 32;
const DEATHS: usize = 24;
const TRACERS: usize = 16;
const BLOCKS: usize = 16;
const GLINTS: usize = 12;
/// Ticks a spark, a tracer, a muzzle flash and a puff of smoke last.
pub const IMPACT_TICKS: u8 = 6;
pub const TRACER_TICKS: u8 = 8;
pub const FLASH_TICKS: u8 = 3;
pub const SMOKE_TICKS: u8 = 14;
/// Ticks a shield's block shows: the flash, the knock and the shards.
pub const BLOCK_TICKS: u8 = 10;
/// Ticks a roll's dust hangs, and a find's glint rises.
pub const DUST_TICKS: u8 = 12;
pub const GLINT_TICKS: u8 = 16;

/// A knight's shot by its kind: how far a hit pushes a monster.
pub fn knock(kind: Shot) -> f32 {
    match kind {
        Shot::Arrow => 0.1,
        Shot::Bolt => 0.5,
        Shot::Ball => 0.9,
        Shot::Sacred => 0.6,
        _ => 0.0,
    }
}

/// A spark where a shot struck, in the shot's colour, and the way it was
/// going (none for a shot popped where it hung).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    pub x: f32,
    pub y: f32,
    pub kind: Shot,
    pub ttl: u8,
    #[serde(default)]
    pub dx: f32,
    #[serde(default)]
    pub dy: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_art: Option<bosses::Art>,
}

/// An arrow or bolt stuck in the stone, pointing the way it flew.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stuck {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub kind: Shot,
}

/// A rail's fading line, from the bow to where it was when it went.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tracer {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub thick: bool,
    pub ttl: u8,
}

/// A muzzle's flash or a handgonne's smoke; or dust off a roll (`dust`:
/// `KICK` as it starts, `TRAIL` along it, `LAND` where it ends).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Puff {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub smoke: bool,
    pub ttl: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dust: u8,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

/// A roll's dust: kicked up as it starts, left along it, thrown out where
/// it lands.
pub const KICK: u8 = 1;
pub const TRAIL: u8 = 2;
pub const LAND: u8 = 3;

/// A monster's death: a white frame, then its parts flying, in the colours
/// of what it was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Death {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub ttl: u8,
    #[serde(default)]
    pub kind: Option<EnemyKind>,
}

/// Something taken off the floor: a glint rising where it lay, by how rare
/// it was (0 a spoil or common find, 1 rare or a spell, 2 a relic).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glint {
    pub x: f32,
    pub y: f32,
    pub tier: u8,
    pub ttl: u8,
}

/// A shot stopped on a raised shield's face (`shield`): where, the way it
/// was going (a unit step), the way the face looks out there, what it was,
/// how heavy (1 to 3) and whose shield it was.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    #[serde(default)]
    pub nx: f32,
    #[serde(default)]
    pub ny: f32,
    pub kind: Shot,
    pub weight: u8,
    pub knight: u32,
    pub ttl: u8,
}

impl Block {
    /// Ticks since it struck: 0 the tick it is first drawn.
    pub fn age(&self) -> u8 {
        (BLOCK_TICKS - 1).saturating_sub(self.ttl)
    }
}

/// A cancelled shot's coin, flying to the nearest knight.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coin {
    pub x: f32,
    pub y: f32,
    pub age: u32,
}

/// One knight's room, for the feel ledger.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Tally {
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    pub damage: u32,
    /// Ticks of the fight in which the knight dealt nothing.
    pub dead: u32,
    pub skills: u32,
    pub grazes: u32,
    /// Ticks from a monster's first wound to its fall, summed over the
    /// knight's kills.
    pub ttk: u64,
}

/// The arms' feel, as the run keeps it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Feel {
    /// The tick of the last party-wide control ultimate (`press`).
    #[serde(default)]
    pub control: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coins: Vec<Coin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impacts: Vec<Impact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stuck: Vec<Stuck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracers: Vec<Tracer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub puffs: Vec<Puff>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deaths: Vec<Death>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glints: Vec<Glint>,
    /// The ledger this room, by knight, and the tick each monster was first
    /// wounded; lines ready for the cockpit to write.
    #[serde(skip)]
    pub ledger: BTreeMap<u32, Tally>,
    #[serde(skip)]
    pub wounded: BTreeMap<u32, u64>,
    #[serde(skip)]
    pub lines: Vec<String>,
    /// Owned hits from skills and other blows this tick, beside the shots.
    #[serde(skip)]
    pub credits: Vec<(u32, u32)>,
}

impl Feel {
    pub(super) fn credit(&mut self, owner: u32, foe: u32, damage: u32, tick: u64) {
        if owner == 0 || damage == 0 {
            return;
        }
        self.credits.push((owner, damage));
        if ledger_on() {
            self.wounded.entry(foe).or_insert(tick);
        }
    }

    pub fn quiet(&self) -> bool {
        self.control.is_none()
            && self.coins.is_empty()
            && self.impacts.is_empty()
            && self.stuck.is_empty()
            && self.tracers.is_empty()
            && self.puffs.is_empty()
            && self.deaths.is_empty()
            && self.blocks.is_empty()
            && self.glints.is_empty()
    }

    pub fn valid(&self) -> bool {
        let point = |x: f32, y: f32| x.is_finite() && y.is_finite();
        self.coins.len() <= COINS
            && self.impacts.len() <= IMPACTS
            && self.stuck.len() <= STUCK
            && self.tracers.len() <= TRACERS
            && self.puffs.len() <= PUFFS
            && self.deaths.len() <= DEATHS
            && self.blocks.len() <= BLOCKS
            && self.glints.len() <= GLINTS
            && self.coins.iter().all(|c| point(c.x, c.y))
            && self
                .impacts
                .iter()
                .all(|i| point(i.x, i.y) && point(i.dx, i.dy) && i.ttl <= IMPACT_TICKS)
            && self
                .stuck
                .iter()
                .all(|s| point(s.x, s.y) && point(s.dx, s.dy))
            && self.tracers.iter().all(|t| {
                point(t.from.0, t.from.1) && point(t.to.0, t.to.1) && t.ttl <= TRACER_TICKS
            })
            && self
                .puffs
                .iter()
                .all(|p| point(p.x, p.y) && point(p.dx, p.dy) && p.ttl <= SMOKE_TICKS && p.dust <= LAND)
            && self
                .deaths
                .iter()
                .all(|d| point(d.x, d.y) && d.r.is_finite() && d.r > 0.0 && d.ttl <= DEATH_TICKS)
            && self.blocks.iter().all(|b| {
                point(b.x, b.y) && point(b.dx, b.dy) && point(b.nx, b.ny) && b.ttl <= BLOCK_TICKS && (1..=3).contains(&b.weight)
            })
            && self.glints.iter().all(|g| point(g.x, g.y) && g.ttl <= GLINT_TICKS && g.tier <= 2)
    }

    /// A shot of `kind` going `way` struck at `(x, y)`; `(0, 0)` for one
    /// popped where it hung.
    pub(super) fn impact(&mut self, x: f32, y: f32, kind: Shot, way: (f32, f32)) {
        if self.impacts.len() < IMPACTS {
            let (dx, dy) = unit(way.0, way.1);
            self.impacts.push(Impact {
                x,
                y,
                kind,
                ttl: IMPACT_TICKS,
                dx,
                dy,
                boss_art: None,
            });
        }
    }

    pub(super) fn boss_impact(&mut self, shot: &Projectile) {
        if let Some(art) = bosses::Art::of_shot(shot) {
            if self.impacts.len() < IMPACTS {
                let (dx, dy) = unit(shot.vx, shot.vy);
                self.impacts.push(Impact {
                    x: shot.x,
                    y: shot.y,
                    kind: shot.kind,
                    ttl: IMPACT_TICKS,
                    dx,
                    dy,
                    boss_art: Some(art),
                });
            }
        }
    }

    pub(super) fn stick(&mut self, x: f32, y: f32, (vx, vy): (f32, f32), kind: Shot) {
        if self.stuck.len() >= STUCK {
            self.stuck.remove(0);
        }
        let (dx, dy) = unit(vx, vy);
        self.stuck.push(Stuck { x, y, dx, dy, kind });
    }

    pub(super) fn tracer(&mut self, from: (f32, f32), to: (f32, f32), thick: bool) {
        if self.tracers.len() >= TRACERS {
            self.tracers.remove(0);
        }
        self.tracers.push(Tracer {
            from,
            to,
            thick,
            ttl: TRACER_TICKS,
        });
    }

    pub(super) fn puff(&mut self, x: f32, y: f32, (dx, dy): (f32, f32), smoke: bool) {
        if self.puffs.len() >= PUFFS {
            self.puffs.remove(0);
        }
        self.puffs.push(Puff {
            x,
            y,
            dx,
            dy,
            smoke,
            ttl: if smoke { SMOKE_TICKS } else { FLASH_TICKS },
            dust: 0,
        });
    }

    /// Dust off a roll at a knight's feet `(x, y)`, the roll going `way`:
    /// `KICK`, `TRAIL` or `LAND`.
    pub(super) fn dust(&mut self, x: f32, y: f32, way: (f32, f32), dust: u8) {
        if self.puffs.len() >= PUFFS {
            self.puffs.remove(0);
        }
        let (dx, dy) = unit(way.0, way.1);
        self.puffs.push(Puff {
            x,
            y,
            dx,
            dy,
            smoke: false,
            ttl: DUST_TICKS,
            dust,
        });
    }

    /// A find taken off the floor at `(x, y)`.
    pub(super) fn glint(&mut self, x: f32, y: f32, tier: u8) {
        if self.glints.len() >= GLINTS {
            self.glints.remove(0);
        }
        self.glints.push(Glint {
            x,
            y,
            tier: tier.min(2),
            ttl: GLINT_TICKS,
        });
    }

    pub(super) fn death(&mut self, x: f32, y: f32, kind: EnemyKind) {
        if self.deaths.len() < DEATHS {
            self.deaths.push(Death {
                x,
                y,
                r: kind.radius(),
                ttl: DEATH_TICKS,
                kind: Some(kind),
            });
        }
    }

    /// A shot stopped on knight `knight`'s shield at `at`, where the face
    /// looks out along `out`, going `way`. The oldest block gives way when
    /// the list is full: the newest is what a knight is looking at.
    pub(super) fn block(&mut self, knight: u32, at: (f32, f32), out: (f32, f32), way: (f32, f32), kind: Shot, weight: u8) {
        if self.blocks.len() >= BLOCKS {
            self.blocks.remove(0);
        }
        let (dx, dy) = unit(way.0, way.1);
        let (nx, ny) = unit(out.0, out.1);
        self.blocks.push(Block {
            x: at.0,
            y: at.1,
            dx,
            dy,
            nx,
            ny,
            kind,
            weight: weight.clamp(1, 3),
            knight,
            ttl: BLOCK_TICKS,
        });
    }

    /// A new room: what stuck in the last one, and its ledger, are gone.
    pub(super) fn clear_room(&mut self) {
        self.impacts.clear();
        self.stuck.clear();
        self.tracers.clear();
        self.puffs.clear();
        self.deaths.clear();
        self.blocks.clear();
        self.glints.clear();
        self.ledger.clear();
        self.wounded.clear();
        self.credits.clear();
    }

    /// One tick of the visuals fading.
    fn fade(&mut self) {
        for t in self.impacts.iter_mut().map(|i| &mut i.ttl) {
            *t = t.saturating_sub(1);
        }
        for t in self.tracers.iter_mut().map(|i| &mut i.ttl) {
            *t = t.saturating_sub(1);
        }
        for p in &mut self.puffs {
            p.ttl = p.ttl.saturating_sub(1);
            if p.smoke {
                // Smoke drifts on the way the ball went, and rises.
                p.x += p.dx * 1.5 * DT;
                p.y += p.dy * 1.5 * DT - 0.6 * DT;
            } else if p.dust > 0 {
                // Dust rolls on a little the way the knight went.
                p.x += p.dx * 0.9 * DT;
                p.y += p.dy * 0.9 * DT;
            }
        }
        for t in self.deaths.iter_mut().map(|i| &mut i.ttl) {
            *t = t.saturating_sub(1);
        }
        for t in self.blocks.iter_mut().map(|b| &mut b.ttl) {
            *t = t.saturating_sub(1);
        }
        for t in self.glints.iter_mut().map(|g| &mut g.ttl) {
            *t = t.saturating_sub(1);
        }
        self.impacts.retain(|i| i.ttl > 0);
        self.tracers.retain(|i| i.ttl > 0);
        self.puffs.retain(|i| i.ttl > 0);
        self.deaths.retain(|i| i.ttl > 0);
        self.blocks.retain(|b| b.ttl > 0);
        self.glints.retain(|g| g.ttl > 0);
    }
}

/// Whether the feel ledger is kept (`ANGEL_DELVE_FEEL=1`, as the host
/// sets it: `settings::Settings::feel_ledger`).
pub fn ledger_on() -> bool {
    crate::settings::get().feel_ledger
}

impl Run {
    /// The arms' tick: visuals fade, coins fly home, valour fills from the
    /// tick's damage and grazes, and the ledger counts.
    pub fn tick_feel(&mut self, credits: &[(u32, u32)], grazes: &[u32]) {
        self.feel.fade();
        let mut dealt: BTreeMap<u32, u32> = BTreeMap::new();
        let mut valour: BTreeMap<u32, u32> = BTreeMap::new();
        let other = std::mem::take(&mut self.feel.credits);
        for &(id, damage) in credits.iter().chain(&other) {
            *dealt.entry(id).or_default() += damage;
            if damage > 0 {
                *valour.entry(id).or_default() += VALOUR_HIT;
            }
        }
        self.tally_floor(&dealt);
        if ledger_on() && self.phase == Phase::Fighting {
            for &id in self.players.keys() {
                let tally = self.feel.ledger.entry(id).or_default();
                match dealt.get(&id) {
                    Some(&d) => tally.damage += d,
                    None => tally.dead += 1,
                }
                tally.grazes += grazes.iter().filter(|&&g| g == id).count() as u32;
            }
        }
        for kit in self.works.kits.values_mut() {
            kit.burning = kit.burning.saturating_sub(1);
        }
        for &id in grazes {
            *valour.entry(id).or_default() += GRAZE_VALOUR;
        }
        for (&id, &gain) in &valour {
            if self.players.contains_key(&id) {
                let kit = self.works.kits.entry(id).or_default();
                kit.valour = (kit.valour + gain).min(PIPS * VALOUR_PIP);
            }
        }
        self.fly_coins();
    }

    /// The room is won: every monster shot still in the air becomes a coin.
    pub(super) fn cancel_shots(&mut self) {
        self.cancel_shots_where(|_| true);
    }

    /// A fight is won: each monster shot in the air that `near` takes
    /// becomes a coin.
    pub(super) fn cancel_shots_where(&mut self, near: impl Fn(&Projectile) -> bool) {
        let room = COINS.saturating_sub(self.feel.coins.len());
        let coins: Vec<Coin> = self
            .projectiles
            .iter()
            .filter(|p| p.hostile && near(p))
            .take(room)
            .map(|p| Coin {
                x: p.x,
                y: p.y,
                age: 0,
            })
            .collect();
        if !coins.is_empty() {
            self.sounds.push("spoil_pickup");
        }
        self.feel.coins.extend(coins);
    }

    /// Coins fly to the nearest knight standing, faster as they go, and are
    /// carried home as gold.
    fn fly_coins(&mut self) {
        if self.feel.coins.is_empty() {
            return;
        }
        let knights: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let mut caught: Vec<u32> = Vec::new();
        self.feel.coins.retain_mut(|coin| {
            coin.age += 1;
            let Some(&(id, kx, ky)) = knights.iter().min_by(|a, b| {
                (a.1 - coin.x)
                    .hypot(a.2 - coin.y)
                    .total_cmp(&(b.1 - coin.x).hypot(b.2 - coin.y))
            }) else {
                return true;
            };
            let d = (kx - coin.x).hypot(ky - coin.y);
            if d < COIN_CATCH {
                caught.push(id);
                return false;
            }
            // A short hop up before it turns home.
            let speed = COIN_SPEED * (coin.age as f32 / 10.0).min(1.0);
            let step = (speed * DT).min(d);
            let (ux, uy) = unit(kx - coin.x, ky - coin.y);
            coin.x += ux * step;
            coin.y += uy * step - if coin.age < 6 { 0.08 } else { 0.0 };
            true
        });
        for id in &caught {
            if let Some(hero) = self.players.get_mut(id) {
                hero.carried.add(Spoil::Gold, COIN_GOLD);
            }
        }
        if !caught.is_empty() {
            self.score += caught.len() as u32 * COIN_GOLD;
            self.sounds.push("spoil_pickup");
        }
    }

    /// Leaving a room: coins still flying are caught at once.
    pub(super) fn bank_coins(&mut self) {
        for coin in std::mem::take(&mut self.feel.coins) {
            if let Some(hero) = self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone)
                .min_by(|a, b| {
                    (a.x - coin.x)
                        .hypot(a.y - coin.y)
                        .total_cmp(&(b.x - coin.x).hypot(b.y - coin.y))
                })
            {
                hero.carried.add(Spoil::Gold, COIN_GOLD);
                self.score += COIN_GOLD;
            }
        }
    }

    /// Knight `id`'s whole pips of valour.
    pub fn pips(&self, id: u32) -> u32 {
        self.works
            .kits
            .get(&id)
            .map_or(0, |k| k.valour / VALOUR_PIP)
    }

    /// A named synergy went off for knight `id`: said on the notice line.
    pub(super) fn synergy(&mut self, id: u32, name: &str, word: &str) {
        self.found = Some((self.tick, id, format!("{name}!")));
        self.cues.push(format!("synergy:{word}"));
    }

    /// The ledger counts a shot loosed, a hit landed, a skill used, a kill.
    pub(super) fn ledger_skill(&mut self, id: u32) {
        if ledger_on() {
            self.feel.ledger.entry(id).or_default().skills += 1;
        }
    }

    /// The room's ledger, a line per knight, for the cockpit to write.
    pub(super) fn close_ledger(&mut self, how: &str) {
        if !ledger_on() {
            return;
        }
        let room = self.at;
        let depth = self.dungeon.depth;
        let knights = self.party_len();
        for (id, tally) in std::mem::take(&mut self.feel.ledger) {
            let Some(hero) = self.players.get(&id) else {
                continue;
            };
            let line = serde_json::json!({
                "raid": self.raid_id,
                "tick": self.tick,
                "depth": depth,
                "room": room,
                "end": how,
                "knights": knights,
                "knight": hero.knight_name().unwrap_or_else(|| hero.name.clone()),
                "arm": hero.forged.as_ref().map(|w| w.name.clone())
                    .unwrap_or_else(|| hero.weapon.name().to_string()),
                "temper": hero.temper,
                "shots": tally.shots,
                "hits": tally.hits,
                "hit_rate": tally.hits as f32 / tally.shots.max(1) as f32,
                "kills": tally.kills,
                "damage": tally.damage,
                "secs_to_kill": if tally.kills > 0 {
                    tally.ttk as f32 / tally.kills as f32 / HZ as f32
                } else {
                    0.0
                },
                "dead_air_secs": tally.dead as f32 / HZ as f32,
                "skills": tally.skills,
                "grazes": tally.grazes,
            });
            self.feel.lines.push(line.to_string());
        }
        self.feel.wounded.clear();
    }
}

impl Run {
    /// A skill that meets the arm in hand: a Fire Pit thrown by a knight
    /// with a bow sets their arrows burning (Fire Arrows).
    pub(super) fn skill_synergy(&mut self, id: u32, which: usize) {
        let bow = self
            .players
            .get(&id)
            .is_some_and(|h| h.forged.is_none() && h.weapon == Weapon::Bow);
        if which == skills::FIRE && bow {
            if let Some(kit) = self.works.kits.get_mut(&id) {
                kit.burning = FIRE_ARROWS;
            }
            self.synergy(id, "Fire Arrows", "fire_arrows");
        }
    }
}

impl Run {
    /// The synergies this tick's shots set off: Fire Arrows' burning,
    /// Ricochet's notice, and the clouds a Powder Keg ball lit.
    pub(super) fn land_synergies(
        &mut self,
        burns: Vec<(u32, u8, u8)>,
        turned: Vec<u8>,
        mut kegs: Vec<(usize, u8)>,
    ) {
        // A `burn N` wish burns N times as hot as Fire Arrows.
        for (foe, owner, hot) in burns {
            let power = self
                .players
                .get(&u32::from(owner))
                .map_or(skills::BURN_BASE, |h| h.scaled(skills::BURN_BASE))
                * u32::from(hot.clamp(1, 3));
            skills::afflict(
                &mut self.works.banes,
                foe,
                skills::Bane::Burn,
                power,
                u32::from(owner),
            );
        }
        for owner in turned {
            self.synergy(u32::from(owner), "Ricochet", "ricochet");
        }
        kegs.sort_by(|a, b| b.0.cmp(&a.0));
        for (i, owner) in kegs {
            if i < self.works.clouds.len() {
                let cloud = self.works.clouds.remove(i);
                self.flash(cloud);
                self.synergy(u32::from(owner), "Powder Keg", "powder_keg");
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_feel__tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../tests/rules/together_feel__fixtures.rs"]
pub mod testing;
