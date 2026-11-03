//! The Mage's weave. A Mage carries no bow: F is the staff's spark, a
//! basic shot and nothing more. The magic is woven: keys 1, 2 and 3 draw
//! an essence (Ember, Frost, Storm) into the ring of three that circles the
//! Mage, the oldest giving way, and key 4 weaves the three into the spell
//! their mix makes and casts it where the Mage aims. Ten mixes, ten spells
//! that strike an area, each on its own cooldown.
//!
//! A Mage knows only as many spells as they have slots: two on the first
//! floor, one more every second floor down, ten at most (`slots`). Weaving a
//! mix not yet known learns it into a free slot; once the slots are full only
//! the spells known weave, so which spells a Mage learns is their build.
use super::heroes::{Cast, Pillar, Zone, ZoneKind};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Essence {
    Ember,
    Frost,
    Storm,
}

impl Essence {
    pub const ALL: [Essence; 3] = [Essence::Ember, Essence::Frost, Essence::Storm];
    pub fn name(self) -> &'static str {
        match self {
            Essence::Ember => "Ember",
            Essence::Frost => "Frost",
            Essence::Storm => "Storm",
        }
    }
}

/// What a Mage holds: the ring of essences, each spell's cooldown, the
/// spell key down last tick (a key draws or weaves once per press), and the
/// last spell woven, for its name over the Mage.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weave {
    pub orbs: Vec<Essence>,
    /// The spells learned into the Mage's slots, in the order learned.
    pub known: Vec<u8>,
    pub cooldowns: [u32; SPELLS.len()],
    pub held: u8,
    pub last: Option<u8>,
    pub shown: u32,
}

impl Weave {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// A spell of the weave: its name, its mix (Ember, Frost, Storm), its
/// cooldown and a line of what it does.
pub struct Spell {
    pub name: &'static str,
    pub mix: [u8; 3],
    pub cooldown: u32,
    pub does: &'static str,
}

pub const SPELLS: [Spell; 10] = [
    Spell { name: "Sunfall", mix: [3, 0, 0], cooldown: 12 * HZ, does: "a star falls where you aim and leaves it burning" },
    Spell { name: "Hoarfrost", mix: [0, 3, 0], cooldown: 9 * HZ, does: "frost bursts round you, chilling all it touches" },
    Spell { name: "Thunderhead", mix: [0, 0, 3], cooldown: 10 * HZ, does: "lightning strikes five foes near where you aim" },
    Spell { name: "Scald", mix: [2, 1, 0], cooldown: 6 * HZ, does: "a cone of steam scalds and slows" },
    Spell { name: "Wildfire", mix: [2, 0, 1], cooldown: 9 * HZ, does: "fire runs out along your aim and burns" },
    Spell { name: "Frost Ring", mix: [1, 2, 0], cooldown: 8 * HZ, does: "twelve frost shards fly out all round" },
    Spell { name: "Sleet", mix: [0, 2, 1], cooldown: 10 * HZ, does: "a field of sleet slows and stings" },
    Spell { name: "Ball Lightning", mix: [1, 0, 2], cooldown: 7 * HZ, does: "a slow orb crackles through the crowd" },
    Spell { name: "Static Ward", mix: [0, 1, 2], cooldown: 11 * HZ, does: "a shock round you, and a moment no blow lands" },
    Spell { name: "Cataclysm", mix: [1, 1, 1], cooldown: 14 * HZ, does: "all three at once: a burst and a ring of every element" },
];

/// How many ticks each spell is shown where it was cast (`heroes::Cast`):
/// its picture's frames, two ticks each. Sleet is drawn with its field and
/// Ball Lightning with its orb, so neither leaves a cast behind.
pub const SHOWN: [u32; SPELLS.len()] = [54, 72, 50, 24, 16, 64, 0, 0, 38, 38];

/// The spell three essences make, whatever their order.
pub fn spell_of(orbs: &[Essence]) -> Option<usize> {
    if orbs.len() != 3 {
        return None;
    }
    let mut mix = [0u8; 3];
    for &o in orbs {
        mix[o as usize] += 1;
    }
    SPELLS.iter().position(|s| s.mix == mix)
}

/// The most spell slots a Mage has, and how many they start a delve with.
pub const MAX_SLOTS: usize = 10;
pub const FIRST_SLOTS: usize = 2;

/// A Mage's spell slots on floor `depth`: two on the first, one more every
/// second floor down, ten at most.
pub fn slots(depth: u32) -> usize {
    (FIRST_SLOTS + depth.saturating_sub(1) as usize / 2).min(MAX_SLOTS)
}

/// What a press of the weave key came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Woven {
    /// Spell `n` is cast (and learned, if it was new).
    Cast(usize),
    /// The mix is a spell not known, and every slot is full.
    Full(usize),
}

/// Whether the knight weaves: the Mage does, and only the Mage.
pub fn weaves(hero: &Hero) -> bool {
    classes::of(hero).is_some_and(|c| c.name == "Mage")
}

/// A Mage's spell keys this tick, with `slots` spell slots: a fresh press
/// of 1–3 draws its essence, of 4 weaves the ring into its spell if that
/// spell is ready and known, or new with a slot free. Cooldowns run down
/// here too, once a tick.
pub(super) fn keys(hero: &mut Hero, cast: u8, slots: usize) -> Option<Woven> {
    let w = &mut hero.weave;
    for cd in &mut w.cooldowns {
        *cd = cd.saturating_sub(1);
    }
    w.shown = w.shown.saturating_sub(1);
    let fresh = cast != 0 && cast != w.held;
    w.held = cast;
    if !fresh || hero.hp == 0 || hero.stone {
        return None;
    }
    match cast {
        1..=3 => {
            if w.orbs.len() == 3 {
                w.orbs.remove(0);
            }
            w.orbs.push(Essence::ALL[usize::from(cast) - 1]);
            None
        }
        4 => {
            let spell = spell_of(&w.orbs)?;
            if !w.known.contains(&(spell as u8)) {
                if w.known.len() >= slots {
                    return Some(Woven::Full(spell));
                }
                w.known.push(spell as u8);
            }
            if w.cooldowns[spell] > 0 {
                return None;
            }
            w.cooldowns[spell] = SPELLS[spell].cooldown;
            w.last = Some(spell as u8);
            w.shown = 2 * HZ;
            Some(Woven::Cast(spell))
        }
        _ => None,
    }
}

impl Run {
    /// Spell `spell` of the weave, cast by knight `owner` toward their aim.
    pub(super) fn weave_cast(&mut self, owner: u32, spell: usize) {
        let Some(h) = self.players.get(&owner).filter(|h| h.hp > 0 && !h.stone) else {
            return;
        };
        let (hx, hy) = (h.x, h.y);
        let (ax, ay) = unit(h.aim_x, h.aim_y);
        let (ax, ay) = if ax == 0.0 && ay == 0.0 { (0.0, 1.0) } else { (ax, ay) };
        let ahead = |d: f32| (hx + ax * d, hy + ay * d);
        self.found = Some((self.tick, owner, format!("{} weaves {}", h.name, SPELLS[spell].name)));
        self.sounds.push("spell_cast");
        // The eighth of a turn the aim points along, clockwise from east.
        let dir = (ay.atan2(ax) / std::f32::consts::FRAC_PI_4).round().rem_euclid(8.0) as u8;
        // Where the spell plays out: Sunfall where the star falls,
        // Thunderhead on each foe it strikes (below), the rest on the Mage.
        let (x, y) = if spell == 0 { ahead(7.0) } else { (hx, hy) };
        if SHOWN[spell] > 0 && spell != 2 {
            self.heroes.casts.push(Cast { spell: spell as u8, x, y, dir, age: 0, wait: 0 });
        }
        match spell {
            // Sunfall: the star's delayed strike, then its fire.
            0 => {
                let (x, y) = ahead(7.0);
                self.heroes.pillars.push(Pillar { owner, x, y, left: heroes::PILLAR_DELAY });
                self.heroes.zones.push(Zone { owner, x, y, left: 4 * HZ, kind: ZoneKind::Fire });
            }
            // Hoarfrost: frost all round.
            1 => self.weave_burst(owner, (hx, hy), 7.0, 55, HZ / 3, 3 * HZ),
            // Thunderhead: the five nearest the aimed point.
            2 => {
                let at = ahead(8.0);
                let mut near: Vec<(f32, usize)> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.hp > 0)
                    .map(|(i, e)| ((e.x - at.0).hypot(e.y - at.1), i))
                    .filter(|(d, _)| *d <= 8.0)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                // The bolts fall one after another, a tenth of a second apart.
                for (k, (_, i)) in near.into_iter().take(5).enumerate() {
                    let (ex, ey) = (self.enemies[i].x, self.enemies[i].y);
                    self.hero_hit(owner, i, 70, HZ / 5);
                    self.heroes.casts.push(Cast { spell: 2, x: ex, y: ey, dir, age: 0, wait: 3 * k as u32 });
                }
            }
            // Scald: a cone of steam ahead.
            3 => {
                let hits: Vec<usize> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        let (dx, dy) = (e.x - hx, e.y - hy);
                        let along = dx * ax + dy * ay;
                        e.hp > 0 && along > 0.0 && along <= 9.0 && (dx * ay - dy * ax).abs() <= along * 0.7 + e.radius()
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in hits {
                    self.hero_hit(owner, i, 50, 0);
                    self.enemies[i].chilled = self.enemies[i].chilled.max(3 * HZ / 2);
                }
            }
            // Wildfire: a line of flame along the aim, burning where it ran.
            4 => {
                self.hero_line(owner, 10.0, 1.0, 30, true, false);
                for d in [3.0, 6.0, 9.0] {
                    let (x, y) = ahead(d);
                    self.heroes.zones.push(Zone { owner, x, y, left: 3 * HZ, kind: ZoneKind::Fire });
                }
            }
            // Frost Ring: shards out all round.
            5 => self.weave_ring(owner, 12, 22, true),
            // Sleet: a field that slows and stings.
            6 => {
                let (x, y) = ahead(7.0);
                self.heroes.zones.push(Zone { owner, x, y, left: 5 * HZ, kind: ZoneKind::Frost });
            }
            // Ball Lightning: one slow orb through the crowd.
            7 => {
                let Some(h) = self.players.get(&owner) else { return };
                let damage = h.scaled(45);
                let traits = ShotTraits { owner: owner as u8, chain: 2, ..ShotTraits::of(&h.bonus) };
                self.projectiles.push(Projectile {
                    x: hx + ax * 0.6,
                    y: hy + ay * 0.6,
                    vx: ax * 12.0,
                    vy: ay * 12.0,
                    hostile: false,
                    kind: Shot::Orb,
                    look: Some((classes::ARCANE.into(), "bolt".into())),
                    damage,
                    pierce: 8,
                    last_hit: None,
                    empowered: true,
                    traits,
                    ttl: 40,
                });
            }
            // Static Ward: a shock round the Mage, and a moment untouched.
            8 => {
                self.weave_burst(owner, (hx, hy), 6.0, 40, HZ, 0);
                if let Some(h) = self.players.get_mut(&owner) {
                    h.invulnerable = h.invulnerable.max(2 * HZ);
                }
            }
            // Cataclysm: every element at once.
            _ => {
                self.weave_burst(owner, (hx, hy), 5.0, 45, HZ / 3, 2 * HZ);
                self.weave_ring(owner, 16, 30, false);
            }
        }
    }

    /// Everything within `reach` of `at` is struck, stunned `stun` ticks and
    /// chilled `chill` ticks; the spell's cast shows the burst.
    fn weave_burst(&mut self, owner: u32, at: (f32, f32), reach: f32, damage: u32, stun: u32, chill: u32) {
        let hits: Vec<usize> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0 && (e.x - at.0).hypot(e.y - at.1) <= reach + e.radius())
            .map(|(i, _)| i)
            .collect();
        for i in hits {
            self.hero_hit(owner, i, damage, stun);
            if chill > 0 {
                self.enemies[i].chilled = self.enemies[i].chilled.max(chill);
            }
        }
    }

    /// `count` shots out all round the Mage, frost-touched or of every
    /// element by turns.
    fn weave_ring(&mut self, owner: u32, count: u32, damage: u32, frost: bool) {
        let Some(h) = self.players.get(&owner) else { return };
        let (hx, hy) = (h.x, h.y);
        let damage = h.scaled(damage);
        let bonus = ShotTraits::of(&h.bonus);
        for k in 0..count {
            let t = k as f32 * std::f32::consts::TAU / count as f32;
            let (dx, dy) = (t.cos(), t.sin());
            let look = match (frost, k % 3) {
                (true, _) | (false, 1) => "frost",
                (false, 0) => "ember",
                _ => classes::ARCANE,
            };
            self.projectiles.push(Projectile {
                x: hx + dx * 0.6,
                y: hy + dy * 0.6,
                vx: dx * 22.0,
                vy: dy * 22.0,
                hostile: false,
                kind: Shot::Orb,
                look: Some((look.into(), "bolt".into())),
                damage,
                pierce: 2,
                last_hit: None,
                empowered: false,
                traits: ShotTraits { owner: owner as u8, chill: if look == "frost" { 2 } else { 0 }, ..bonus.clone() },
                ttl: 14,
            });
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/weave__tests.rs"]
mod tests;
