//! The skills every knight carries, beside the weapon and the guard: four
//! keys, each with its own rest.
//!
//! - **Gas** (the first key): a canister thrown ahead bursts into a cloud
//!   that swells, drifts a little and poisons whatever breathes it.
//! - **Fire** (the second): a firebomb bursts and leaves burning ground.
//!   Foes standing in it burn; flame set to a cloud of gas flashes it off.
//! - **Mines** (the third): up to `MINE_MAX` laid at the knight's feet. A
//!   mine arms after a moment and blows when a foe steps on it. Knights
//!   never set one off, nor are hurt by one.
//! - **Barricade** (the fourth): a short palisade that foes cannot walk
//!   through and their shots break against. Damage wears it down; it
//!   crumbles when it is spent or its time is up.
//!
//! - **Shield throw** (the fifth): the knight hurls their own shield. It
//!   flies along the aim, strikes what it meets, ricochets off stone, jumps
//!   to one or two near foes, and comes back to the hand. While it is out
//!   the knight has no shield to raise.
//!
//! Nothing a knight raises hurts a knight. The same clouds and flames, when
//! a room's own vent or jet makes them (`perils`), hurt everyone.
//!
//! All the numbers that make a skill strong or weak are the named constants
//! at the top, so they can be tuned without reading the rest.

use super::hud::Slot;
use super::*;

pub const SKILLS: usize = 5;
pub const GAS: usize = 0;
pub const FIRE: usize = 1;
pub const MINE: usize = 2;
pub const WALL: usize = 3;
pub const SHIELD: usize = 4;
pub const NAMES: [&str; SKILLS] =
    ["Fart Cloud", "Fire Pit", "Mines", "Barrier", "Shield Throw"];
/// Ticks each skill rests after it is used (the mines' is the rest between
/// two laid). Each use also spends a pip of valour (`feel`): the meter, not
/// the rest, is what holds a skill back.
pub const REST: [u32; SKILLS] = [3 * HZ; SKILLS];

/// Skill levels I, II and III. Level I is where every knight starts; II
/// comes with the school's Adept rank; III is taught by a questline's
/// teacher (`questline`). Named constants per level, fixed numbers only.
pub const GAS_LIFE_L: [u32; 3] = [8 * HZ, 9 * HZ, 10 * HZ];
pub const GAS_RADIUS_L: [f32; 3] = [3.5, 4.0, 4.5];
pub const POISON_BASE_L: [u32; 3] = [4, 5, 6];
pub const FIRE_LIFE_L: [u32; 3] = [6 * HZ, 7 * HZ, 8 * HZ];
pub const FIRE_RADIUS_L: [f32; 3] = [2.6, 3.0, 3.4];
pub const MINE_MAX_L: [usize; 3] = [4, 4, 5];
pub const MINE_ARM_L: [u32; 3] = [HZ * 3 / 2, HZ, HZ];
pub const MINE_BLAST_L: [f32; 3] = [2.8, 3.2, 3.6];
pub const WALL_MAX_L: [usize; 3] = [3, 3, 4];
pub const WALL_HP_L: [u32; 3] = [150, 200, 250];
pub const WALL_LIFE_L: [u32; 3] = [12 * HZ, 15 * HZ, 18 * HZ];
pub const SHIELD_SPEED_L: [f32; 3] = [25.0, 28.0, 30.0];
pub const SHIELD_REST_L: [u32; 3] = [3 * HZ, 5 * HZ / 2, 2 * HZ];
pub const SHIELD_LINKS_L: [u8; 3] = [1, 2, 3];

impl Run {
    /// Knight `id`'s level with a skill, I–III. I is the default; II comes
    /// with the school's Adept rank; III only from the skill's questline.
    pub fn skill_level(&self, id: u32, which: usize) -> usize {
        let taught = self
            .players
            .get(&id)
            .and_then(|h| self.home.questlines.level(&super::home::Home::school_key(h), which))
            .unwrap_or(1);
        if taught >= 2 {
            return taught;
        }
        // II rides on the school's Adept rank (or better).
        if self
            .players
            .get(&id)
            .is_some_and(|h| {
                self.home
                    .school
                    .standing(&super::home::Home::school_key(h))
                    .rank
                    >= super::school::Rank::Adept
            })
        {
            2
        } else {
            1
        }
    }

    /// The live constants for knight `id`'s skills: the named constant per
    /// level, in one place.
    pub fn gas_life(&self, id: u32) -> u32 {
        GAS_LIFE_L[self.skill_level(id, GAS).clamp(1, 3) - 1]
    }
    pub fn gas_radius(&self, id: u32) -> f32 {
        GAS_RADIUS_L[self.skill_level(id, GAS).clamp(1, 3) - 1]
    }
    pub fn poison_base(&self, id: u32) -> u32 {
        POISON_BASE_L[self.skill_level(id, GAS).clamp(1, 3) - 1]
    }
    pub fn fire_life(&self, id: u32) -> u32 {
        FIRE_LIFE_L[self.skill_level(id, FIRE).clamp(1, 3) - 1]
    }
    pub fn fire_radius(&self, id: u32) -> f32 {
        FIRE_RADIUS_L[self.skill_level(id, FIRE).clamp(1, 3) - 1]
    }
    pub fn mine_max(&self, id: u32) -> usize {
        MINE_MAX_L[self.skill_level(id, MINE).clamp(1, 3) - 1]
    }
    pub fn mine_arm(&self, id: u32) -> u32 {
        MINE_ARM_L[self.skill_level(id, MINE).clamp(1, 3) - 1]
    }
    pub fn mine_blast(&self, id: u32) -> f32 {
        MINE_BLAST_L[self.skill_level(id, MINE).clamp(1, 3) - 1]
    }
    pub fn wall_max(&self, id: u32) -> usize {
        WALL_MAX_L[self.skill_level(id, WALL).clamp(1, 3) - 1]
    }
    pub fn wall_hp(&self, id: u32) -> u32 {
        WALL_HP_L[self.skill_level(id, WALL).clamp(1, 3) - 1]
    }
    pub fn wall_life(&self, id: u32) -> u32 {
        WALL_LIFE_L[self.skill_level(id, WALL).clamp(1, 3) - 1]
    }
    pub fn shield_speed(&self, id: u32) -> f32 {
        SHIELD_SPEED_L[self.skill_level(id, SHIELD).clamp(1, 3) - 1]
    }
    pub fn shield_rest(&self, id: u32) -> u32 {
        SHIELD_REST_L[self.skill_level(id, SHIELD).clamp(1, 3) - 1]
    }
    pub fn shield_links(&self, id: u32) -> u8 {
        SHIELD_LINKS_L[self.skill_level(id, SHIELD).clamp(1, 3) - 1]
    }
}
/// How far a canister or firebomb flies, in arena units.
const THROW: f32 = 6.0;
/// A ground effect's reach up and down in a side-on hall (the ground it
/// stands on, not the air above).
const SIDE_RISE: f32 = 1.7;

/// Gas: the cloud's life, final reach and swell time; the poison it leaves
/// (damage a beat, ticks between beats, ticks it clings after the cloud is
/// left) and how much of it a boss shrugs off.
pub const GAS_LIFE: u32 = 7 * HZ;
pub const GAS_RADIUS: f32 = 3.0;
const GAS_SWELL: u32 = HZ / 2;
const GAS_DRIFT: f32 = 1.1;
pub(super) const POISON_BASE: u32 = 4;
const POISON_EVERY: u32 = HZ / 2;
const POISON_LINGER: u32 = 3 * HZ;
/// Fart Bombs: a cloud's life, a knight's rest between two (at N 1), and
/// the most clouds a room holds before hits stop adding them.
const FART_LIFE: u32 = 3 * HZ;
const FART_REST: u32 = 2 * HZ;
const FART_CLOUDS: usize = 32;
const BOSS_SHRUG: u32 = 50;
/// Fire: the bomb's burst damage, the patch's life and reach; the burn
/// (damage a beat, ticks between beats, ticks it clings).
const FIRE_BURST: u32 = 20;
pub const FIRE_LIFE: u32 = 5 * HZ;
pub const FIRE_RADIUS: f32 = 2.2;
pub(super) const BURN_BASE: u32 = 3;
const BURN_EVERY: u32 = HZ / 3;
const BURN_LINGER: u32 = 2 * HZ;
/// Flame meeting gas: the flash's damage to everything in the cloud.
const FLASH_DAMAGE: u32 = 30;
/// Mines: how many a knight may have down, how long one takes to arm, how
/// near a foe sets it off, the blast's reach and damage, how long a mine
/// waits, and the fuse a neighbouring blast leaves it.
pub const MINE_MAX: usize = 3;
pub const MINE_ARM: u32 = HZ * 3 / 2;
const MINE_TRIGGER: f32 = 1.3;
pub const MINE_BLAST: f32 = 2.8;
const MINE_DAMAGE: u32 = 70;
const MINE_LIFE: u32 = 90 * HZ;
const MINE_CHAIN: u8 = 4;
/// Barricade: how wide it stands (top-down) and how high (side-on), how far
/// ahead of the knight, its strength, its time, how many a knight keeps up,
/// and how long it takes to come apart. A foe leaning on it wears it down.
pub const WALL_SPAN: f32 = 4.0;
pub const WALL_HEIGHT: f32 = 2.8;
const WALL_REACH: f32 = 2.4;
pub const WALL_HP: u32 = 150;
pub const WALL_LIFE: u32 = 12 * HZ;
pub const WALL_MAX: usize = 2;
pub const WALL_CRUMBLE: u8 = 16;
const WALL_THICK: f32 = 0.5;
const GNAW: u32 = 8;
const GNAW_EVERY: u64 = 10;

/// Shield throw: how fast it flies, how far it goes out before it turns,
/// what each strike does and how long it stuns, how many times it may
/// glance off stone, how many foes it may jump to and how far it looks for
/// them, how long it may be out at most, and how near the hand catches it.
pub const SHIELD_SPEED: f32 = 22.0;
const SHIELD_RANGE: f32 = 13.0;
const SHIELD_DAMAGE: u32 = 28;
const SHIELD_STUN: u32 = 6;
const SHIELD_RICOCHETS: u8 = 2;
const SHIELD_LINKS: u8 = 2;
const SHIELD_JUMP: f32 = 7.0;
const SHIELD_LIFE: u32 = 4 * HZ;
const SHIELD_CATCH: f32 = 1.1;
const SHIELD_REACH: f32 = 0.9;

/// How far a crouching knight's body drops, side-on, from a shot's reckoning.
pub const DUCK_DROP: f32 = 0.75;

/// One knight's skill bar: ticks each skill still rests, and the key held
/// last tick (a skill answers a press, not a hold).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kit {
    pub rest: [u32; SKILLS],
    pub held: u8,
    /// Crouching on firm ground, side-on: shots at the chest go over.
    #[serde(default)]
    pub duck: bool,
    /// Valour earned by hits and grazes; a skill spends a pip of it
    /// (`feel::VALOUR_PIP`).
    #[serde(default)]
    pub valour: u32,
    /// Fire Arrows: ticks the bow's arrows still burn.
    #[serde(default)]
    pub burning: u32,
    /// Fart Bombs: ticks before this knight's hits may fart again.
    #[serde(default)]
    pub fart: u32,
}

/// A cloud of gas. `owner` 0 is the room's own (a vent): it hurts knights.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cloud {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age: u32,
    pub owner: u32,
    pub power: u32,
    /// The thrower's tint of this cloud's bad air: 0 marsh green, 1 sulphur.
    #[serde(default)]
    pub tint: u8,
}

impl Cloud {
    pub fn wall(&self) -> bool {
        matches!(self.tint, 2 | 3)
    }
    pub fn wall_radius(&self) -> f32 {
        if self.tint == 3 { 1.35 } else { 1.15 }
    }
    /// How far it reaches now: swelling at first, thinning at the last.
    pub fn radius(&self) -> f32 {
        if self.wall() {
            return self.wall_radius()
                * (GAS_LIFE.saturating_sub(self.age) as f32 / HZ as f32).clamp(0.65, 1.0);
        }
        let swell = (self.age as f32 / GAS_SWELL as f32).min(1.0);
        let thin = (GAS_LIFE.saturating_sub(self.age) as f32 / HZ as f32).min(1.0);
        GAS_RADIUS * (0.4 + 0.6 * swell) * (0.55 + 0.45 * thin)
    }

    pub fn hostile(&self) -> bool {
        self.owner == 0
    }
}

/// Burning ground.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blaze {
    pub x: f32,
    pub y: f32,
    pub age: u32,
    pub life: u32,
    pub owner: u32,
    pub power: u32,
    pub radius: f32,
}

impl Blaze {
    pub fn hostile(&self) -> bool {
        self.owner == 0
    }
}

/// A mine in the ground; `fuse` counts down once a blast beside it has lit it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mine {
    pub x: f32,
    pub y: f32,
    pub age: u32,
    pub owner: u32,
    pub power: u32,
    #[serde(default)]
    pub fuse: u8,
}

impl Mine {
    pub fn armed(&self) -> bool {
        self.age >= MINE_ARM
    }
}

/// A palisade between two points. `fall` counts down while it comes apart.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Barricade {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub hp: u32,
    pub age: u32,
    pub owner: u32,
    #[serde(default)]
    pub fall: Option<u8>,
}

impl Barricade {
    pub fn standing(&self) -> bool {
        self.fall.is_none()
    }

    pub fn share(&self) -> f32 {
        self.hp as f32 / WALL_HP as f32
    }
}

/// A shield in the air: out along its throw (`going`) or on its way home.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub going: bool,
    pub age: u32,
    pub power: u32,
    pub bounces: u8,
    pub links: u8,
    pub travelled: f32,
    /// Foes already struck on this leg.
    pub struck: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoomKind {
    /// A shield's strike: a small starburst.
    Clang,
    /// A mine's blast.
    Mine,
    /// Flame meeting gas.
    Flash,
    /// A barricade's dust, a floor's dust.
    Dust,
    /// A fire jet's or a bomb's burst of flame.
    Burst,
}

/// A blast, flash or puff of dust playing out: `left` ticks of `of`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boom {
    pub x: f32,
    pub y: f32,
    pub left: u8,
    pub of: u8,
    pub kind: BoomKind,
    pub reach: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bane {
    Poison,
    Burn,
}

/// A monster poisoned or burning.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ailment {
    pub foe: u32,
    pub kind: Bane,
    #[serde(default)]
    pub owner: u32,
    pub left: u32,
    pub power: u32,
}

/// Input's skill field is left out of the wire when no skill is pressed.
pub fn unpressed(skill: &u8) -> bool {
    *skill == 0
}

/// The point on segment `a`–`b` nearest `(x, y)`.
pub fn nearest_on(a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((x - a.0) * dx + (y - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.0 + t * dx, a.1 + t * dy)
}

/// Foes cannot walk through a standing barricade: whatever has been moved
/// into one is pushed back out along the way it came.
pub(super) fn shove(enemy: &mut Enemy, barricades: &[Barricade]) {
    for wall in barricades.iter().filter(|w| w.standing()) {
        let (px, py) = nearest_on(wall.a, wall.b, enemy.x, enemy.y);
        let reach = enemy.radius() * 0.7 + WALL_THICK;
        let (dx, dy) = (enemy.x - px, enemy.y - py);
        let d = dx.hypot(dy);
        if d >= reach {
            continue;
        }
        let (ux, uy) = if d > 0.001 {
            (dx / d, dy / d)
        } else {
            // Dead centre on the line: out along the wall's normal.
            unit(-(wall.b.1 - wall.a.1), wall.b.0 - wall.a.0)
        };
        (enemy.x, enemy.y) = (px + ux * reach, py + uy * reach);
    }
}

pub(super) fn afflict(banes: &mut Vec<Ailment>, foe: u32, kind: Bane, power: u32, owner: u32) {
    let left = match kind {
        Bane::Poison => POISON_LINGER,
        Bane::Burn => BURN_LINGER,
    };
    if let Some(bane) = banes.iter_mut().find(|b| b.foe == foe && b.kind == kind) {
        bane.left = bane.left.max(left);
        bane.power = bane.power.max(power);
        bane.owner = owner;
    } else if banes.len() < 128 {
        banes.push(Ailment {
            foe,
            kind,
            left,
            power,
            owner,
        });
    }
}

impl Run {
    /// Whether a knight can use a skill now.
    fn skilful(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| {
            h.hp > 0
                && !h.stone
                && h.privy == 0
                && h.pit.stop.is_none()
                && h.tossed == 0
                && !h.frog()
                && h.slashes == 0
                && h.aiming.is_none()
        }) && self.active()
            && (!self.at_home_now()
                || self.room().kind == RoomKind::Yard
                || self.room().kind.in_world())
    }

    /// Whether a point lies in a ground effect centred `(cx, cy)` of reach
    /// `r`. Side-on, the effect lies on the ground and the air above it is
    /// clear.
    fn reaches(&self, (cx, cy): (f32, f32), r: f32, (x, y): (f32, f32), slack: f32) -> bool {
        if self.side_on() {
            (x - cx).abs() <= r + slack && (y - cy).abs() <= SIDE_RISE + slack
        } else {
            (x - cx).hypot(y - cy) <= r + slack
        }
    }

    /// Where a canister thrown by knight `id` lands, `reach` units along
    /// their aim: short of any stone, and on the ground side-on.
    fn lob_target(&self, id: u32, reach: f32) -> (f32, f32) {
        let Some(hero) = self.players.get(&id) else {
            return (0.0, 0.0);
        };
        let grid = Grid {
            room: self.room(),
            barred: self.barred(),
        };
        let (mut ux, mut uy) = unit(hero.aim_x, hero.aim_y);
        if self.side_on() {
            (ux, uy) = (if hero.aim_x < 0.0 { -1.0 } else { 1.0 }, 0.0);
        } else if ux == 0.0 && uy == 0.0 {
            uy = -1.0;
        }
        let (mut x, mut y) = (hero.x, hero.y);
        let mut flown = 0.0;
        while flown < reach {
            let (nx, ny) = (x + ux * 0.4, y + uy * 0.4);
            if grid.stops_shot(nx, ny) {
                break;
            }
            (x, y) = (nx, ny);
            flown += 0.4;
        }
        if self.side_on() {
            y = ledge::stand_on(self.room(), x, y);
        }
        (x, y)
    }

    /// Knight `id` uses skill `which`, if it has rested.
    fn use_skill(&mut self, id: u32, which: usize) {
        if !self.skilful(id) {
            return;
        }
        if self
            .works
            .kits
            .get(&id)
            .is_none_or(|k| k.rest[which] > 0 || k.valour < feel::VALOUR_PIP)
        {
            return;
        }
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let side = self.side_on();
        let seed = mix(self.seed ^ self.tick ^ u64::from(id) << 40);
        match which {
            GAS => {
                let power = hero.scaled(POISON_BASE);
                let (x, y) = self.lob_target(id, THROW);
                let angle = (seed % 628) as f32 / 100.0;
                self.works.clouds.push(Cloud {
                    x,
                    y,
                    vx: angle.cos() * GAS_DRIFT,
                    vy: if side { 0.0 } else { angle.sin() * GAS_DRIFT },
                    age: 0,
                    owner: id,
                    power,
                    tint: 0,
                });
                self.sounds.push("gas_release");
            }
            FIRE => {
                let power = hero.scaled(BURN_BASE);
                let burst = hero.scaled(FIRE_BURST);
                let (x, y) = self.lob_target(id, THROW);
                self.works.blazes.push(Blaze {
                    x,
                    y,
                    age: 0,
                    life: FIRE_LIFE,
                    owner: id,
                    power,
                    radius: FIRE_RADIUS,
                });
                self.boom(x, y, BoomKind::Burst, FIRE_RADIUS);
                let struck = aoe::strike(&mut self.enemies, (x, y), burst, |enemy| {
                    if side {
                        (enemy.x - x).abs() <= FIRE_RADIUS + enemy.radius() * 0.5
                            && (enemy.y - y).abs() <= SIDE_RISE
                    } else {
                        (enemy.x - x).hypot(enemy.y - y) <= FIRE_RADIUS + enemy.radius() * 0.5
                    }
                });
                for (i, damage) in struck {
                    let enemy = &mut self.enemies[i];
                    enemy.wits.foe = id;
                    self.feel.credit(id, enemy.id, damage, self.tick);
                }
                self.sounds.push("spell_fire");
            }
            MINE => {
                let mine_count = self.works.mines.iter().filter(|m| m.owner == id).count();
                if mine_count >= MINE_MAX {
                    return;
                }
                let power = hero.scaled(MINE_DAMAGE);
                let (x, mut y) = (hero.x, hero.y);
                if side {
                    y = ledge::stand_on(self.room(), x, y);
                }
                self.works.mines.push(Mine {
                    x,
                    y,
                    age: 0,
                    owner: id,
                    power,
                    fuse: 0,
                });
                self.sounds.push("card_pickup");
            }
            SHIELD => {
                if self.works.flights.iter().any(|f| f.owner == id) {
                    return;
                }
                let power = hero.scaled(SHIELD_DAMAGE);
                let (mut ux, mut uy) = unit(hero.aim_x, hero.aim_y);
                if ux == 0.0 && uy == 0.0 {
                    uy = -1.0;
                }
                if side && uy.abs() > 0.8 && ux == 0.0 {
                    ux = 1.0;
                    uy = 0.0;
                }
                self.works.flights.push(Flight {
                    owner: id,
                    x: hero.x + ux * 0.8,
                    y: hero.y + uy * 0.8,
                    vx: ux * SHIELD_SPEED,
                    vy: uy * SHIELD_SPEED,
                    going: true,
                    age: 0,
                    power,
                    bounces: SHIELD_RICOCHETS,
                    links: SHIELD_LINKS,
                    travelled: 0.0,
                    struck: Vec::new(),
                });
                self.sounds.push("swing");
            }
            WALL => {
                let (hx, hy, ax, ay) = (hero.x, hero.y, hero.aim_x, hero.aim_y);
                let (a, b) = if side {
                    let dir = if ax < 0.0 { -1.0 } else { 1.0 };
                    let x = (hx + dir * WALL_REACH).clamp(1.0, self.room().width() - 1.0);
                    let feet = ledge::stand_on(self.room(), x, hy) + ledge::FOOT;
                    ((x, feet - WALL_HEIGHT), (x, feet))
                } else {
                    let (ux, uy) = if ax == 0.0 && ay == 0.0 {
                        (0.0, -1.0)
                    } else {
                        unit(ax, ay)
                    };
                    let (cx, cy) = (hx + ux * WALL_REACH, hy + uy * WALL_REACH);
                    let (px, py) = (-uy, ux);
                    let half = WALL_SPAN / 2.0;
                    let grid = Grid {
                        room: self.room(),
                        barred: self.barred(),
                    };
                    // Each end stops short of stone.
                    let end = |sign: f32| {
                        let mut at = (cx, cy);
                        for _ in 0..10 {
                            let next = (
                                at.0 + px * sign * half / 10.0,
                                at.1 + py * sign * half / 10.0,
                            );
                            if grid.stops_shot(next.0, next.1) {
                                break;
                            }
                            at = next;
                        }
                        at
                    };
                    (end(-1.0), end(1.0))
                };
                let mine: Vec<usize> = self
                    .works
                    .barricades
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.owner == id && w.standing())
                    .map(|(i, _)| i)
                    .collect();
                if mine.len() >= WALL_MAX {
                    // The oldest comes down to make room.
                    let oldest = self.works.barricades[mine[0]];
                    self.works.barricades[mine[0]].fall = Some(WALL_CRUMBLE);
                    let mid = (
                        (oldest.a.0 + oldest.b.0) / 2.0,
                        (oldest.a.1 + oldest.b.1) / 2.0,
                    );
                    self.boom(mid.0, mid.1, BoomKind::Dust, 1.6);
                }
                if self.works.barricades.len() < 24 {
                    self.works.barricades.push(Barricade {
                        a,
                        b,
                        hp: WALL_HP,
                        age: 0,
                        owner: id,
                        fall: None,
                    });
                }
                self.sounds.push("skill_barrier");
            }
            _ => return,
        }
        if let Some(kit) = self.works.kits.get_mut(&id) {
            kit.rest[which] = REST[which];
            kit.valour = kit.valour.saturating_sub(feel::VALOUR_PIP);
        }
        self.ledger_skill(id);
        self.skill_synergy(id, which);
    }

    /// A blast, flash or puff of dust.
    pub(super) fn boom(&mut self, x: f32, y: f32, kind: BoomKind, reach: f32) {
        cap_booms(&mut self.works.booms);
        let of = match kind {
            BoomKind::Mine => 12,
            BoomKind::Flash => 14,
            BoomKind::Dust => 18,
            BoomKind::Burst => 10,
            BoomKind::Clang => 7,
        };
        self.works.booms.push(Boom {
            x,
            y,
            left: of,
            of,
            kind,
            reach,
        });
    }

    /// The skills' tick: presses first, then the world they made.
    pub(super) fn tick_skills(&mut self, inputs: &BTreeMap<u32, Input>) {
        for kit in self.works.kits.values_mut() {
            for rest in &mut kit.rest {
                *rest = rest.saturating_sub(1);
            }
            kit.fart = kit.fart.saturating_sub(1);
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let want = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .map_or(0, |i| i.skill);
            let kit = self.works.kits.entry(id).or_default();
            let pressed = want != 0 && want != kit.held;
            kit.held = want;
            if pressed {
                self.use_skill(id, usize::from(want) - 1);
            }
        }
        self.tick_clouds();
        self.tick_blazes();
        self.tick_mines();
        self.tick_barricades();
        self.tick_flights();
        self.tick_banes();
        for boom in &mut self.works.booms {
            boom.left = boom.left.saturating_sub(1);
        }
        self.works.booms.retain(|b| b.left > 0);
    }

    /// Fart Bombs: a hit leaves a short sulphur cloud where it struck, at
    /// most once per `FART_REST / N` ticks a knight, and never past
    /// `FART_CLOUDS` clouds in the room.
    pub(super) fn fart_hits(&mut self, farts: Vec<(f32, f32, u8, u8)>) {
        let side = self.side_on();
        for (x, y, owner, n) in farts {
            let id = u32::from(owner);
            let Some(power) = self.players.get(&id).map(|h| h.scaled(POISON_BASE)) else {
                continue;
            };
            if self.works.clouds.len() >= FART_CLOUDS {
                return;
            }
            let kit = self.works.kits.entry(id).or_default();
            if kit.fart > 0 {
                continue;
            }
            kit.fart = FART_REST / u32::from(n.max(1));
            let angle = (mix(self.seed ^ self.tick ^ u64::from(id) << 40) % 628) as f32 / 100.0;
            self.works.clouds.push(Cloud {
                x,
                y,
                vx: angle.cos() * GAS_DRIFT * 0.5,
                vy: if side { 0.0 } else { angle.sin() * GAS_DRIFT * 0.5 },
                age: GAS_LIFE - FART_LIFE,
                owner: id,
                power,
                tint: 1,
            });
            self.sounds.push("gas_release");
        }
    }

    /// Clouds drift, swell and thin; whatever breathes them is poisoned.
    fn tick_clouds(&mut self) {
        let tick = self.tick;
        let side = self.side_on();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        for cloud in &mut self.works.clouds {
            cloud.age += 1;
            if cloud.wall() {
                continue;
            }
            // A slow wander on top of the first push.
            let sway = (cloud.age as f32 * 0.07 + cloud.owner as f32).sin() * 0.04;
            cloud.vx = (cloud.vx + sway) * 0.97;
            cloud.vy = if side {
                0.0
            } else {
                (cloud.vy - sway * 0.6) * 0.97
            };
            let (nx, ny) = (cloud.x + cloud.vx * DT, cloud.y + cloud.vy * DT);
            if grid.open(nx, ny, Mover::Flier) {
                (cloud.x, cloud.y) = (nx, ny);
            } else {
                (cloud.vx, cloud.vy) = (-cloud.vx, -cloud.vy);
            }
        }
        self.works.clouds.retain(|c| c.age < GAS_LIFE);
        let clouds = self.works.clouds.clone();
        for cloud in &clouds {
            let r = cloud.radius();
            // The nearest in full, the rest at the share past them (`aoe`).
            let caught = aoe::reached(&self.enemies, (cloud.x, cloud.y), |enemy| {
                self.reaches(
                    (cloud.x, cloud.y),
                    r,
                    (enemy.x, enemy.y),
                    enemy.radius() * 0.5,
                )
            });
            for (rank, i) in caught.into_iter().enumerate().filter(|_| !cloud.wall()) {
                afflict(
                    &mut self.works.banes,
                    self.enemies[i].id,
                    Bane::Poison,
                    aoe::part(cloud.power, rank).max(1),
                    cloud.owner,
                );
            }
            if cloud.hostile() && tick.is_multiple_of(u64::from(POISON_EVERY)) {
                let hits: Vec<u32> = self
                    .players
                    .iter()
                    .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
                    .filter(|(_, h)| self.reaches((cloud.x, cloud.y), r, (h.x, h.y), HERO_RADIUS))
                    .map(|(&id, _)| id)
                    .collect();
                for id in hits {
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.hurt(perils::GAS_HARM);
                    }
                }
            }
        }
    }

    /// Burning ground burns; flame reaching a cloud flashes it off.
    fn tick_blazes(&mut self) {
        let tick = self.tick;
        for blaze in &mut self.works.blazes {
            blaze.age += 1;
        }
        self.works.blazes.retain(|b| b.age < b.life);
        let blazes = self.works.blazes.clone();
        for blaze in &blazes {
            // The nearest in full, the rest at the share past them (`aoe`).
            let caught = aoe::reached(&self.enemies, (blaze.x, blaze.y), |enemy| {
                self.reaches(
                    (blaze.x, blaze.y),
                    blaze.radius,
                    (enemy.x, enemy.y),
                    enemy.radius() * 0.5,
                )
            });
            for (rank, i) in caught.into_iter().enumerate() {
                afflict(
                    &mut self.works.banes,
                    self.enemies[i].id,
                    Bane::Burn,
                    aoe::part(blaze.power, rank).max(1),
                    blaze.owner,
                );
            }
            if blaze.hostile() && tick.is_multiple_of(u64::from(BURN_EVERY)) {
                let hits: Vec<u32> = self
                    .players
                    .iter()
                    .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
                    .filter(|(_, h)| {
                        self.reaches((blaze.x, blaze.y), blaze.radius, (h.x, h.y), HERO_RADIUS)
                    })
                    .map(|(&id, _)| id)
                    .collect();
                for id in hits {
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.hurt(perils::FIRE_HARM);
                    }
                }
            }
            // Flame in gas: the cloud goes up in one flash.
            let lit: Vec<usize> = self
                .works
                .clouds
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    (c.x - blaze.x).hypot(c.y - blaze.y) < (c.radius() + blaze.radius) * 0.6
                })
                .map(|(i, _)| i)
                .collect();
            for i in lit.into_iter().rev() {
                let cloud = self.works.clouds.remove(i);
                self.flash(cloud);
            }
        }
    }

    /// A cloud goes up: everything in it takes the flash.
    pub(super) fn flash(&mut self, cloud: Cloud) {
        let r = cloud.radius();
        let damage = FLASH_DAMAGE.max(cloud.power * 6);
        let (cx, cy) = (cloud.x, cloud.y);
        let caught = aoe::reached(&self.enemies, (cx, cy), |e| {
            self.reaches((cx, cy), r, (e.x, e.y), e.radius() * 0.5)
        });
        for (rank, i) in caught.into_iter().enumerate() {
            let e = &mut self.enemies[i];
            let damage = aoe::part(damage, rank).min(e.hp);
            e.hp = e.hp.saturating_sub(damage);
            if cloud.owner > 0 {
                e.wits.foe = cloud.owner;
            }
            self.feel.credit(cloud.owner, e.id, damage, self.tick);
        }
        let hits: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .filter(|(_, h)| self.reaches((cx, cy), r, (h.x, h.y), HERO_RADIUS))
            .map(|(&id, _)| id)
            .collect();
        if cloud.hostile() {
            for id in hits {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.hurt(perils::FIRE_HARM * 2);
                }
            }
        }
        self.boom(cx, cy, BoomKind::Flash, r);
        self.sounds.push("bomb");
        self.fx.push(Fx::Shake(4));
    }

    /// Mines arm, and blow under a foe's feet or a neighbour's blast.
    fn tick_mines(&mut self) {
        for mine in &mut self.works.mines {
            mine.age += 1;
        }
        let mut blown: Vec<Mine> = Vec::new();
        let walkers: Vec<(f32, f32, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0 && !e.kind.flies() && e.kind != EnemyKind::Dummy)
            .map(|e| (e.x, e.y, e.radius()))
            .collect();
        let mut index = 0;
        while index < self.works.mines.len() {
            let mine = self.works.mines[index];
            let lit = mine.fuse > 0;
            let fused = lit && mine.fuse == 1;
            let stepped = mine.armed()
                && !lit
                && walkers.iter().any(|&(x, y, r)| {
                    self.reaches((mine.x, mine.y), MINE_TRIGGER, (x, y), r * 0.5)
                });
            if fused || stepped || mine.age > MINE_LIFE {
                blown.push(self.works.mines.remove(index));
                continue;
            }
            if lit {
                self.works.mines[index].fuse -= 1;
            }
            index += 1;
        }
        for mine in blown {
            self.detonate(mine);
        }
    }

    /// A mine's blast: monsters in reach are hurt, a neighbouring mine is lit,
    /// a cracked wall may come down.
    fn detonate(&mut self, mine: Mine) {
        let (x, y) = (mine.x, mine.y);
        let mut felled = 0;
        let caught = aoe::reached(&self.enemies, (x, y), |e| {
            self.reaches((x, y), MINE_BLAST, (e.x, e.y), e.radius() * 0.5)
        });
        for (rank, i) in caught.into_iter().enumerate() {
            let e = &mut self.enemies[i];
            let damage = aoe::part(mine.power, rank).min(e.hp);
            e.hp = e.hp.saturating_sub(damage);
            e.wits.foe = mine.owner;
            self.feel.credit(mine.owner, e.id, damage, self.tick);
            felled += u32::from(e.hp == 0);
        }
        if felled >= 3 {
            self.notice("demolitions");
        }
        for other in &mut self.works.mines {
            if other.fuse == 0 && (other.x - x).hypot(other.y - y) <= MINE_BLAST {
                other.fuse = MINE_CHAIN;
            }
        }
        self.boom(x, y, BoomKind::Mine, MINE_BLAST);
        self.sounds.push("bomb");
        self.fx.push(Fx::Shake(6));
        self.blast_wall(Some((x, y)));
    }

    /// Barricades take shots and the weight of foes; they crumble when spent
    /// or when their time is up.
    fn tick_barricades(&mut self) {
        let tick = self.tick;
        let shots: Vec<usize> = (0..self.projectiles.len())
            .filter(|&i| self.projectiles[i].hostile && self.projectiles[i].ttl > 0)
            .collect();
        let mut broke = false;
        for wall in self.works.barricades.iter_mut().filter(|w| w.standing()) {
            wall.age += 1;
            for &i in &shots {
                let shot = &mut self.projectiles[i];
                let before = (shot.x - shot.vx * DT, shot.y - shot.vy * DT);
                let (px, py) = nearest_on(wall.a, wall.b, shot.x, shot.y);
                if shot.ttl > 0
                    && (crosses(before, (shot.x, shot.y), wall.a, wall.b)
                        || (shot.x - px).hypot(shot.y - py) < WALL_THICK)
                {
                    shot.ttl = 0;
                    wall.hp = wall.hp.saturating_sub(shot.damage);
                    self.sounds.push("shield_block");
                }
            }
            if tick.is_multiple_of(GNAW_EVERY) {
                for enemy in self.enemies.iter().filter(|e| e.hp > 0) {
                    let (px, py) = nearest_on(wall.a, wall.b, enemy.x, enemy.y);
                    if (enemy.x - px).hypot(enemy.y - py) < enemy.radius() * 0.7 + WALL_THICK + 0.15
                    {
                        let weight = if enemy.is_boss() { GNAW * 3 } else { GNAW };
                        wall.hp = wall.hp.saturating_sub(weight);
                    }
                }
            }
            if wall.hp == 0 || wall.age > WALL_LIFE {
                wall.fall = Some(WALL_CRUMBLE);
                broke = true;
            }
        }
        let mut dust: Vec<(f32, f32)> = Vec::new();
        for wall in &mut self.works.barricades {
            if let Some(left) = wall.fall {
                if left == WALL_CRUMBLE {
                    dust.push(((wall.a.0 + wall.b.0) / 2.0, (wall.a.1 + wall.b.1) / 2.0));
                }
                wall.fall = Some(left.saturating_sub(1));
            }
        }
        self.works.barricades.retain(|w| w.fall != Some(0));
        for (x, y) in dust {
            self.boom(x, y, BoomKind::Dust, 1.8);
        }
        if broke {
            self.sounds.push("rock_land");
        }
    }

    /// Whether knight `id`'s shield is in the air, so there is none to raise.
    pub fn shield_out(&self, id: u32) -> bool {
        self.works.flights.iter().any(|f| f.owner == id)
    }

    /// Thrown shields fly, strike, glance off stone, jump to a near foe and
    /// come home; shots they meet are knocked down.
    fn tick_flights(&mut self) {
        let mut flights = std::mem::take(&mut self.works.flights);
        let mut clangs = 0;
        flights.retain_mut(|f| {
            f.age += 1;
            let Some(hero) = self.players.get(&f.owner).filter(|h| h.hp > 0 && !h.stone) else {
                return false;
            };
            let (hx, hy) = (hero.x, hero.y);
            if f.age > SHIELD_LIFE {
                return false;
            }
            if !f.going {
                // Home to the hand.
                let (ux, uy) = unit(hx - f.x, hy - f.y);
                (f.vx, f.vy) = (ux * SHIELD_SPEED, uy * SHIELD_SPEED);
                if (hx - f.x).hypot(hy - f.y) < SHIELD_CATCH {
                    self.sounds.push("shield_block");
                    return false;
                }
            }
            let (ox, oy) = (f.x, f.y);
            let (nx, ny) = (f.x + f.vx * DT, f.y + f.vy * DT);
            let grid = Grid {
                room: &self.dungeon.rooms[self.at],
                barred: !self.enemies.is_empty() || self.phase == Phase::Fighting,
            };
            if f.going && grid.stops_shot(nx, ny) {
                // Off the stone: across, along, or both in a corner.
                if f.bounces > 0 {
                    let across = grid.stops_shot(nx, oy);
                    let along = grid.stops_shot(ox, ny);
                    if across || !along {
                        f.vx = -f.vx;
                    }
                    if along || !across {
                        f.vy = -f.vy;
                    }
                    f.bounces -= 1;
                    f.struck.clear();
                    clangs += 1;
                    cap_booms(&mut self.works.booms);
                    self.works.booms.push(Boom {
                        x: ox,
                        y: oy,
                        left: 7,
                        of: 7,
                        kind: BoomKind::Clang,
                        reach: 1.0,
                    });
                } else {
                    f.going = false;
                    f.struck.clear();
                }
                return true;
            }
            (f.x, f.y) = (nx, ny);
            if f.going {
                f.travelled += (f.vx * DT).hypot(f.vy * DT);
                if f.travelled >= SHIELD_RANGE {
                    f.going = false;
                    f.struck.clear();
                }
            }
            // Strike what it flies through.
            let mut hit_at: Option<(f32, f32)> = None;
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                if f.struck.contains(&enemy.id) {
                    continue;
                }
                if (enemy.x - f.x).hypot(enemy.y - f.y) <= enemy.radius() + SHIELD_REACH {
                    let damage = f.power.min(enemy.hp);
                    enemy.hp = enemy.hp.saturating_sub(f.power);
                    enemy.wits.foe = f.owner;
                    self.feel.credit(f.owner, enemy.id, damage, self.tick);
                    enemy.frozen = enemy.frozen.max(SHIELD_STUN);
                    f.struck.push(enemy.id);
                    clangs += 1;
                    hit_at = Some((enemy.x, enemy.y));
                }
            }
            if let Some((hx, hy)) = hit_at {
                cap_booms(&mut self.works.booms);
                self.works.booms.push(Boom {
                    x: hx,
                    y: hy,
                    left: 7,
                    of: 7,
                    kind: BoomKind::Clang,
                    reach: 1.0,
                });
                // On to the nearest foe it has not struck, within reach.
                if f.going && f.links > 0 {
                    let next = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0 && !f.struck.contains(&e.id))
                        .map(|e| (e, (e.x - f.x).hypot(e.y - f.y)))
                        .filter(|(_, d)| *d <= SHIELD_JUMP)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(e, _)| (e.x, e.y));
                    match next {
                        Some((tx, ty)) => {
                            let (ux, uy) = unit(tx - f.x, ty - f.y);
                            (f.vx, f.vy) = (ux * SHIELD_SPEED, uy * SHIELD_SPEED);
                            f.links -= 1;
                            f.travelled = f.travelled.min(SHIELD_RANGE * 0.5);
                        }
                        None => {
                            f.going = false;
                            f.struck.clear();
                        }
                    }
                }
            }
            for shot in self
                .projectiles
                .iter_mut()
                .filter(|p| p.hostile && p.ttl > 0)
            {
                if (shot.x - f.x).hypot(shot.y - f.y) <= SHIELD_REACH {
                    shot.ttl = 0;
                    clangs += 1;
                }
            }
            true
        });
        self.works.flights = flights;
        if clangs > 0 {
            self.sounds.push("clang");
        }
    }

    /// Poison and burns do their damage on the beat.
    fn tick_banes(&mut self) {
        let tick = self.tick;
        for bane in &mut self.works.banes {
            bane.left = bane.left.saturating_sub(1);
        }
        for bane in self.works.banes.clone() {
            let every = match bane.kind {
                Bane::Poison => POISON_EVERY,
                Bane::Burn => BURN_EVERY,
            };
            if !(tick + u64::from(bane.foe)).is_multiple_of(u64::from(every)) {
                continue;
            }
            if let Some(enemy) = self
                .enemies
                .iter_mut()
                .find(|e| e.id == bane.foe && e.hp > 0)
            {
                let damage = if enemy.is_boss() {
                    bane.power * (100 - BOSS_SHRUG) / 100
                } else {
                    bane.power
                };
                let damage = damage.max(1).min(enemy.hp);
                enemy.hp = enemy.hp.saturating_sub(damage);
                if bane.owner > 0 {
                    enemy.wits.foe = bane.owner;
                }
                self.feel.credit(bane.owner, enemy.id, damage, self.tick);
            }
        }
        let alive: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0)
            .map(|e| e.id)
            .collect();
        self.works
            .banes
            .retain(|b| b.left > 0 && alive.contains(&b.foe));
    }

    /// Knight `id`'s skill bar for the HUD, keys labelled by `keys`.
    pub fn skill_slots(&self, id: u32, keys: &[&str; SKILLS]) -> Vec<Slot> {
        let kit = self.works.kits.get(&id).cloned().unwrap_or_default();
        (0..SKILLS)
            .map(|which| {
                let left = kit.rest[which];
                let name = if which == MINE {
                    let down = self.works.mines.iter().filter(|m| m.owner == id).count();
                    format!("Mines {down}/{MINE_MAX}")
                } else {
                    NAMES[which].to_string()
                };
                // A skill wants a pip of valour as well as its rest.
                let pips = kit.valour / feel::VALOUR_PIP;
                let state = if which == SHIELD && self.shield_out(id) {
                    "out".to_string()
                } else if left > 0 {
                    format!("{}s", left.div_ceil(HZ))
                } else if pips == 0 {
                    "valour".to_string()
                } else {
                    "ready".to_string()
                };
                let fill = if pips == 0 {
                    (kit.valour % feel::VALOUR_PIP) as f32 / feel::VALOUR_PIP as f32
                } else {
                    1.0 - left as f32 / REST[which] as f32
                };
                Slot::new(keys[which], name, fill, state)
            })
            .collect()
    }
}

/// Room for one more boom: the oldest goes past 32, so a storm of blasts and
/// clangs never outgrows what the snapshot accepts.
fn cap_booms(booms: &mut Vec<Boom>) {
    if booms.len() >= 32 {
        booms.remove(0);
    }
}
