//! The knight's blow. Contact and rearm are the arms' (`arms`); this is
//! how far a stroke carries, how long its blade still turns a bullet, and
//! the transient picture of it the wire carries (`State`).
//!
//! - **The slash** carries the cut past the blade's tip: a stroke reaches
//!   half its blade's length again (a long blade `SLASH_MOST` at most) and
//!   sweeps a quarter wider an arc (`slash_reach`, `slash_arc`). What the
//!   crescent covers is what it strikes.
//! - **The parry**: for `PARRY_TICKS` from the stroke, an ordinary monster
//!   bullet that meets the slash's arc is turned back the way it came, the
//!   knight's now, faster and harder (`parry`). Beams, lobs, mines and the
//!   guardians' own shots are no bullets: the stroke cuts down those it
//!   meets as the blade always did, on its own tick, within the blade.
//! - **The weight**: a blow that lands solid asks for a short hold of the
//!   room's time, a killing blow for a longer one and a kick of the camera
//!   (`Landed`, `hitstop`); the shared cooldown keeps a crowded room moving.
//! - **The sound**: a stroke whooshes as its arm does (`Blade`, the same
//!   arm the picture draws: a spear's flick, a sword's cut, an axe's chop,
//!   a cleaver's flat slab, a maul's heave) and lands as heavy as its
//!   heaviest contact (`Tier::sound`), the steel and the meat in one take;
//!   a turned bullet tings off the blade and whizzes back (`PARRY_SOUNDS`).
//!   Sounds are no part of the rules: none is read back, none is sent in a
//!   snapshot.

use super::*;

/// A stroke's whoosh by its arm (`Blade::swing_sound`): sword, axe, maul,
/// spear, cleaver.
pub const SWING_SOUNDS: [&str; 5] = ["swing_sword", "swing_axe", "swing_maul", "swing_spear", "swing_cleaver"];
/// A contact by its weight (`Tier::sound`): light, medium, heavy.
pub const CONTACT_SOUNDS: [&str; 3] = ["hit_flesh_light", "hit_flesh_medium", "hit_flesh_heavy"];
/// A bullet turned off the blade: the ting, and the shot whizzing back.
pub const PARRY_SOUNDS: [&str; 2] = ["parry", "parry_whizz"];

/// What a knight swings, as it is drawn (`render::overworld::arena::blades`)
/// and as it sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blade {
    Sword,
    Axe,
    Maul,
    Spear,
    Cleaver,
}

impl Blade {
    /// The arm knight `hero` holds or is swinging: the knight's own sword
    /// on the sword key, else the arm card's or forged weapon's, by its
    /// name, else by how its stroke goes.
    pub fn of(hero: &Hero) -> Blade {
        let blade = hero.forged.as_ref().and_then(|w| w.melee);
        if blade.is_none() || (hero.sword && hero.swing > 0) {
            return Blade::Sword;
        }
        match hero.arm.as_deref().unwrap_or("") {
            "great-maul" | "prospectors-pick" => return Blade::Maul,
            "twin-axes" | "grave-axes" | "reaver-axes" => return Blade::Axe,
            "grave-staff" => return Blade::Spear,
            "cleaver" => return Blade::Cleaver,
            "knights-blade" | "round-table-blade" => return Blade::Sword,
            _ => {}
        }
        let name = hero.forged.as_ref().map(|w| w.name.to_ascii_lowercase()).unwrap_or_default();
        let has = |words: &[&str]| name.split(|c: char| !c.is_alphabetic()).any(|w| words.contains(&w));
        if has(&["maul", "hammer", "warhammer", "mace", "club"]) {
            Blade::Maul
        } else if has(&["axe", "axes", "battleaxe", "greataxe", "hatchet"]) {
            Blade::Axe
        } else if has(&["spear", "pike", "lance", "staff", "glaive", "halberd"]) {
            Blade::Spear
        } else if has(&["cleaver", "chopper"]) {
            Blade::Cleaver
        } else if blade.is_some_and(|m| m.arc < 80.0) {
            Blade::Spear
        } else {
            Blade::Sword
        }
    }

    /// The arm a stroke implies, when no knight is to hand.
    pub fn of_stroke(stroke: Stroke) -> Blade {
        match stroke {
            Stroke::Overhead | Stroke::Slam => Blade::Maul,
            Stroke::Chain => Blade::Axe,
            Stroke::Thrust => Blade::Spear,
            Stroke::Slash | Stroke::Fist => Blade::Sword,
        }
    }

    /// Its whoosh: the spear light and quick, the sword and the axe
    /// between, the cleaver and the maul heavy (`SWING_SOUNDS`).
    pub fn swing_sound(self) -> &'static str {
        SWING_SOUNDS[self as usize]
    }
}

pub const SWING_TICKS: u8 = 9;
pub const HIT_TICKS: u8 = 7;
/// Ticks a turned bullet's ting and its word show.
pub const TURNED_TICKS: u8 = 14;
const LIMIT: usize = 64;
/// The slash's reach past the blade: half the blade's length again, and
/// never more than this many units.
pub const SLASH_MOST: f32 = 1.6;
/// The slash's arc, this much wider than the blade's (a ring stays one).
pub const SLASH_WIDER: f32 = 1.25;
/// Ticks from the stroke the blade still turns a bullet: the first fifth
/// of a second, the stroke's own tick among them.
pub const PARRY_TICKS: u8 = 6;
/// The most bullets one stroke turns: a timely blade answers a few shots,
/// never a storm.
pub const PARRY_MOST: u8 = 2;
/// A turned bullet leaves the blade this much faster, and at least this
/// fast (units a second), and flies this long.
pub const TURN_SPEED: f32 = 1.35;
pub const TURN_LEAST: f32 = 14.0;
pub const TURN_TICKS: u32 = HZ * 3 / 2;
/// How far either side of its way back (radians) and how far off (units)
/// a turned bullet looks for a monster to fly at.
const TURN_SEEK: f32 = 0.6;
const TURN_SIGHT: f32 = 18.0;
/// How far past the slash's edge a bullet is still met.
const PARRY_PAD: f32 = 0.6;

#[cfg(test)]
thread_local! {
    /// On, a knight swings as before this pass: the blade's own reach and
    /// arc, a stroke that cuts down every monster shot in it on its own
    /// tick and turns none, no hold for an ordinary blow, and finishers
    /// chosen by their variant. The same run, before and after, in one
    /// build (`wire__tests`).
    pub static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn before() -> bool {
    #[cfg(test)]
    return BEFORE.with(std::cell::Cell::get);
    #[cfg(not(test))]
    false
}

/// How far a stroke of a blade `blade` units long carries.
pub fn slash_reach(blade: f32) -> f32 {
    if before() { blade } else { blade + (blade * 0.5).min(SLASH_MOST) }
}

/// How wide a stroke of a blade sweeping `arc` degrees cuts.
pub fn slash_arc(arc: f32) -> f32 {
    if before() || arc >= 270.0 { arc } else { (arc * SLASH_WIDER).min(360.0) }
}

/// The blade inside a slash that reaches `reach` (`slash_reach` undone),
/// for the picture: where the steel ends and the crescent begins.
pub fn blade(reach: f32) -> f32 {
    if reach <= SLASH_MOST * 3.0 { reach / 1.5 } else { reach - SLASH_MOST }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stroke {
    Slash,
    Overhead,
    Thrust,
    Chain,
    Slam,
    Fist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    Light,
    Medium,
    Heavy,
    Blocked,
    /// A bullet turned back off the blade.
    Turned,
}

impl Tier {
    /// A contact's sound, by its weight (`CONTACT_SOUNDS`); a blocked
    /// blow and a turned bullet have their own.
    pub fn sound(self) -> Option<&'static str> {
        match self {
            Tier::Light => Some(CONTACT_SOUNDS[0]),
            Tier::Medium => Some(CONTACT_SOUNDS[1]),
            Tier::Heavy => Some(CONTACT_SOUNDS[2]),
            Tier::Blocked | Tier::Turned => None,
        }
    }

    /// How heavy a contact is, for the heaviest of a stroke's.
    fn weight(self) -> u8 {
        match self {
            Tier::Light => 1,
            Tier::Medium => 2,
            Tier::Heavy => 3,
            Tier::Blocked | Tier::Turned => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swing {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub reach: f32,
    pub arc: f32,
    pub stroke: Stroke,
    pub combo: u8,
    pub ttl: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    pub foe: u32,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub tier: Tier,
    pub crack: bool,
    pub ttl: u8,
}

impl Hit {
    /// The ticks it was given.
    pub fn life(&self) -> u8 {
        match (self.crack, self.tier) {
            (true, _) => 18,
            (_, Tier::Turned) => TURNED_TICKS,
            _ => HIT_TICKS,
        }
    }

    /// Ticks since it struck.
    pub fn age(&self) -> u8 {
        self.life().saturating_sub(self.ttl)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swings: Vec<Swing>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hits: Vec<Hit>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chains: BTreeMap<u32, (u8, u8)>,
}

pub fn stroke(hero: &Hero, arc: f32) -> Stroke {
    match hero.arm.as_deref().unwrap_or("") {
        "great-maul" | "prospectors-pick" => Stroke::Overhead,
        "grave-staff" => Stroke::Thrust,
        "twin-axes" | "grave-axes" | "reaver-axes" => Stroke::Chain,
        _ if arc < 80.0 => Stroke::Thrust,
        _ if arc >= 270.0 => Stroke::Chain,
        _ => Stroke::Slash,
    }
}

impl State {
    pub fn quiet(&self) -> bool {
        self.swings.is_empty() && self.hits.is_empty() && self.chains.is_empty()
    }
    pub fn tick(&mut self) {
        for s in &mut self.swings {
            s.ttl = s.ttl.saturating_sub(1);
        }
        for h in &mut self.hits {
            h.ttl = h.ttl.saturating_sub(1);
        }
        for (_, ttl) in self.chains.values_mut() {
            *ttl = ttl.saturating_sub(1);
        }
        self.swings.retain(|s| s.ttl > 0);
        self.hits.retain(|h| h.ttl > 0);
        self.chains.retain(|_, (_, ttl)| *ttl > 0);
    }

    pub fn valid(&self) -> bool {
        let point =
            |x: f32, y: f32| (-4096.0..=4096.0).contains(&x) && (-4096.0..=4096.0).contains(&y);
        let direction = |x: f32, y: f32| (-1.0..=1.0).contains(&x) && (-1.0..=1.0).contains(&y);
        self.swings.len() <= LIMIT
            && self.hits.len() <= LIMIT
            && self.chains.len() <= LIMIT
            && self.swings.iter().all(|s| {
                point(s.x, s.y)
                    && direction(s.dx, s.dy)
                    && s.reach.is_finite()
                    && (0.0..=16.0).contains(&s.reach)
                    && s.arc.is_finite()
                    && (0.0..=360.0).contains(&s.arc)
                    && s.ttl <= SWING_TICKS
                    && (1..=3).contains(&s.combo)
            })
            && self
                .hits
                .iter()
                .all(|h| point(h.x, h.y) && direction(h.dx, h.dy) && h.ttl <= h.life())
            && self
                .chains
                .values()
                .all(|&(n, t)| (1..=3).contains(&n) && t <= 48)
    }

    pub fn swing(&mut self, owner: u32, hero: &Hero, stroke: Stroke, reach: f32, arc: f32) {
        let combo = self.chains.get(&owner).map_or(1, |&(n, _)| n % 3 + 1);
        if self.chains.len() < LIMIT || self.chains.contains_key(&owner) {
            self.chains.insert(owner, (combo, 48));
        }
        self.swings.retain(|s| s.owner != owner);
        if self.swings.len() >= LIMIT {
            self.swings.remove(0);
        }
        self.swings.push(Swing {
            owner,
            x: hero.x,
            y: hero.y,
            dx: hero.aim_x,
            dy: hero.aim_y,
            reach,
            arc,
            stroke,
            combo,
            ttl: SWING_TICKS,
        });
    }

    /// Where knight `owner`'s three-stroke chain stands: 1 to 3, 1 with
    /// none under way.
    pub fn combo(&self, owner: u32) -> u8 {
        self.swings.iter().find(|s| s.owner == owner).map_or(1, |s| s.combo)
    }

    pub fn contact(
        &mut self,
        owner: u32,
        hero: &Hero,
        enemy: &Enemy,
        damage: u32,
        crack: bool,
    ) -> Tier {
        let finish = self.swings.iter().any(|s| s.owner == owner && s.combo == 3);
        let tier = if crack || damage >= 45 || finish {
            Tier::Heavy
        } else if damage >= 20 {
            Tier::Medium
        } else {
            Tier::Light
        };
        self.hit(
            enemy.id,
            (enemy.x, enemy.y),
            unit(enemy.x - hero.x, enemy.y - hero.y),
            tier,
            crack,
        );
        tier
    }

    pub fn hit(&mut self, foe: u32, pos: (f32, f32), way: (f32, f32), tier: Tier, crack: bool) {
        if self.hits.len() >= LIMIT {
            self.hits.remove(0);
        }
        let mut hit = Hit {
            foe,
            x: pos.0,
            y: pos.1,
            dx: way.0,
            dy: way.1,
            tier,
            crack,
            ttl: 0,
        };
        hit.ttl = hit.life();
        self.hits.push(hit);
    }

    pub fn parry(&mut self, pos: (f32, f32), way: (f32, f32)) {
        self.hit(0, pos, way, Tier::Blocked, false);
    }

    /// A bullet turned off the blade at `pos`, going `way` now.
    pub fn turned(&mut self, pos: (f32, f32), way: (f32, f32)) {
        self.hit(0, pos, way, Tier::Turned, false);
    }
}

/// A stroke's guard while it is young: the way it was aimed, how far and
/// how wide its slash reaches, what a bullet turned off it strikes for at
/// least, the ticks it still turns them (`PARRY_TICKS` down) and how many
/// more it may turn (`PARRY_MOST` down).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guard {
    pub dx: f32,
    pub dy: f32,
    pub reach: f32,
    pub arc: f32,
    pub damage: u32,
    pub left: u8,
    pub turns: u8,
}

impl Guard {
    /// The guard of a stroke the knight just made, a `reach` and `arc`
    /// slash dealing `damage`; none as the blade was before this pass.
    pub fn of(hero: &Hero, reach: f32, arc: f32, damage: u32) -> Option<Guard> {
        (!before()).then_some(Guard {
            dx: hero.aim_x,
            dy: hero.aim_y,
            reach,
            arc,
            damage,
            left: PARRY_TICKS,
            turns: PARRY_MOST,
        })
    }

    pub fn valid(&self) -> bool {
        let direction = |v: f32| (-1.0..=1.0).contains(&v);
        direction(self.dx)
            && direction(self.dy)
            && self.reach.is_finite()
            && (0.0..=16.0).contains(&self.reach)
            && self.arc.is_finite()
            && (0.0..=360.0).contains(&self.arc)
            && self.left <= PARRY_TICKS
            && self.turns <= PARRY_MOST
    }

    /// The stroke's guard a tick older: gone once its window has passed,
    /// or it has turned all it may.
    pub fn aged(self) -> Option<Guard> {
        (self.left > 1 && self.turns > 0).then_some(Guard { left: self.left - 1, ..self })
    }
}

impl Projectile {
    /// A monster's plain bullet of `kind` at `at` flying `v`, for a fixture
    /// to swing at.
    #[cfg(any(test, feature = "testing"))]
    pub fn bullet_for_test(at: (f32, f32), v: (f32, f32), kind: Shot, damage: u32) -> Projectile {
        Projectile {
            x: at.0,
            y: at.1,
            vx: v.0,
            vy: v.1,
            hostile: true,
            kind,
            look: None,
            damage,
            pierce: 0,
            last_hit: None,
            ttl: 4 * HZ,
            empowered: false,
            traits: ShotTraits::default(),
        }
    }
}

/// Whether `(dx, dy)` from the knight lies within `reach` of them and
/// `arc` degrees about the way `(ax, ay)`.
fn within((dx, dy): (f32, f32), (ax, ay): (f32, f32), reach: f32, arc: f32) -> bool {
    let angle = ay.atan2(ax);
    let delta = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
    dx.hypot(dy) <= reach && delta <= arc.to_radians() / 2.0
}

/// A bullet the blade can turn: a monster's ordinary shot in flight. A
/// beam, a lob, a mine still waiting, a guardian's own shot or a piece of
/// its charge-up is not one.
pub fn turnable(shot: &Projectile) -> bool {
    shot.hostile
        && shot.ttl > 0
        && shot.in_flight()
        && shot.traits.path.form != patterns::Form::Lob
        && shot.traits.blow == 0
        && !shot.traits.borrowed
        && bosses::Art::of_shot(shot).is_none()
}

/// The way a bullet turned at `at` goes: back the way it came, or at the
/// nearest monster near that line.
fn turn_way(shot: &Projectile, at: (f32, f32), aim: (f32, f32), enemies: &[Enemy]) -> (f32, f32) {
    let back = unit(-shot.vx, -shot.vy);
    let back = if back == (0.0, 0.0) { aim } else { back };
    let ahead = back.1.atan2(back.0);
    enemies
        .iter()
        .filter(|e| e.hp > 0 && !e.buried())
        .filter_map(|e| {
            let (dx, dy) = (e.x - at.0, e.y - at.1);
            let d = dx.hypot(dy);
            let off = (dy.atan2(dx) - ahead).sin().atan2((dy.atan2(dx) - ahead).cos()).abs();
            (d > 0.0 && d <= TURN_SIGHT && off <= TURN_SEEK).then_some((d * (1.0 + off), (dx / d, dy / d)))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(back, |(_, way)| way)
}

/// Knight `id`'s blade against the monsters' shots this tick. While its
/// guard (`Hero::parrying`) lasts, every ordinary bullet in the slash is
/// turned back, the knight's now; on the stroke's own tick, `cut` (the
/// blade's reach and arc) cuts down what cannot be turned, as the blade
/// always has. How many bullets it turned.
pub(super) fn parry(
    id: u32,
    hero: &mut Hero,
    shots: &mut [Projectile],
    enemies: &[Enemy],
    feel: &mut State,
    sounds: &mut Vec<&'static str>,
    cut: Option<(f32, f32)>,
) -> u32 {
    let (hx, hy) = (hero.x, hero.y);
    let aim = (hero.aim_x, hero.aim_y);
    if before() {
        if let Some((reach, arc)) = cut {
            for shot in shots.iter_mut().filter(|p| p.hostile) {
                if within((shot.x - hx, shot.y - hy), aim, reach + PARRY_PAD, arc) {
                    feel.parry((shot.x, shot.y), aim);
                    shot.ttl = 0;
                }
            }
        }
        return 0;
    }
    let mut turned = 0;
    for shot in shots.iter_mut().filter(|p| p.hostile && p.ttl > 0) {
        let off = (shot.x - hx, shot.y - hy);
        let guarded = hero
            .parrying
            .filter(|g| g.turns > 0 && within(off, (g.dx, g.dy), g.reach + PARRY_PAD, g.arc));
        if let Some(guard) = guarded.filter(|_| turnable(shot)) {
            if let Some(g) = &mut hero.parrying {
                g.turns -= 1;
            }
            let way = turn_way(shot, (shot.x, shot.y), (guard.dx, guard.dy), enemies);
            let speed = (shot.vx.hypot(shot.vy) * TURN_SPEED).max(TURN_LEAST);
            (shot.vx, shot.vy) = (way.0 * speed, way.1 * speed);
            shot.hostile = false;
            shot.damage = (shot.damage * 2).max(guard.damage);
            shot.ttl = TURN_TICKS;
            shot.last_hit = None;
            let path = shot.traits.path;
            shot.traits.path = patterns::Path { age: path.age, form: path.form, ..Default::default() };
            shot.traits.owner = id.min(u32::from(u8::MAX)) as u8;
            shot.traits.once = false;
            shot.traits.turned = true;
            feel.turned((shot.x, shot.y), way);
            turned += 1;
        } else if let Some((reach, arc)) = cut
            && within(off, aim, reach + PARRY_PAD, arc)
        {
            feel.parry((shot.x, shot.y), aim);
            shot.ttl = 0;
        }
    }
    if turned > 0 {
        sounds.extend(PARRY_SOUNDS);
    }
    turned
}

/// What one stroke did, for the weight it lands with: whether any of it
/// landed solid (a medium or heavy contact), its heaviest contact and how
/// many it killed.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Landed {
    solid: bool,
    heaviest: Option<Tier>,
    kills: u32,
}

impl Landed {
    pub(super) fn contact(&mut self, tier: Tier) {
        self.solid |= matches!(tier, Tier::Medium | Tier::Heavy);
        if self.heaviest.is_none_or(|h| tier.weight() > h.weight()) {
            self.heaviest = Some(tier);
        }
    }

    /// The stroke's sound as it lands: its heaviest contact's, once
    /// however many it struck.
    pub(super) fn sound(&self) -> Option<&'static str> {
        self.heaviest.and_then(Tier::sound)
    }

    pub(super) fn kill(&mut self) {
        self.kills += 1;
    }

    /// The hold the stroke asks of the room's time (`hitstop`), and on a
    /// kill the camera's kick: the picture's shake, which a calm screen
    /// keeps still.
    pub(super) fn weigh(self, hitstop: &mut hitstop::State, tune: &tune::Tune, fx: &mut Vec<Fx>) {
        if before() {
            return;
        }
        if self.kills > 0 {
            hitstop.request(tune, hitstop::Impact::MeleeKill);
            fx.push(Fx::Shake(feel::SHAKE_KILL));
        } else if self.solid {
            hitstop.request(tune, hitstop::Impact::Melee);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_melee__tests.rs"]
mod tests;
