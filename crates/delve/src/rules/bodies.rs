//! A knight raised in a monster's body (`skulls`) fights with that body's
//! arms, not their own. The fire key looses what the monster looses: its
//! pattern on this floor, walked one volley a press, or its teeth. A spell
//! key casts what the monster casts, if it casts anything. The knight's
//! weapon, sword and spell cards wait for the Sanctuary.
//!
//! A borrowed shot is a monster's shot turned friend (`ShotTraits::
//! borrowed`): it flies at three times the monster's pace, a lob comes down
//! on the monsters, a beam burns them, and a shot that turns after someone
//! turns after them.

use super::*;
use patterns::{Aim, Emitter, Form};

/// Ticks between a borrowed body's volleys: its own beat, quartered, held
/// between these.
pub const BODY_QUICKEST: u32 = 12;
pub const BODY_SLOWEST: u32 = 30;
/// How much faster a borrowed shot flies than the monster's own, and the
/// fastest it flies, units a second.
pub const BODY_PACE: f32 = 3.0;
pub const BODY_TOP: f32 = 36.0;
/// How long a borrowed shot flies before it fades.
pub const BODY_LIFE: u32 = 3 * HZ;
/// A borrowed beam's warning, ticks: the knight need not stand and wait.
pub const BODY_WARN: u8 = 6;
/// Ticks between a borrowed body's spells.
pub const SPELL_REARM: u32 = 5 * HZ;
/// How far, and how near the aim, a monster stands to be what a volley is
/// thrown at.
pub const MARK_REACH: f32 = 14.0;
pub const MARK_CONE: f32 = 0.6;

/// What a borrowed body does on the fire key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arms {
    /// Its volleys, one a press, each shot dealing `damage`.
    Volley {
        chain: &'static [(Emitter, Shot)],
        damage: u32,
    },
    /// Its teeth, claws, tusks or blade: a strike in front, `reach` past
    /// the monster's own size, across `arc` degrees. A leech's drinks:
    /// half of what it deals mends the knight.
    Bite {
        reach: f32,
        arc: f32,
        damage: u32,
        drain: bool,
    },
}

/// A body's spell: one volley of `emitter` throwing `shot`, each dealing
/// `damage`, that leaps on to `chain` more monsters, freezes what it hits
/// for `stun` ticks and chills it `chill` steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spell {
    pub name: &'static str,
    pub emitter: Emitter,
    pub shot: Shot,
    pub damage: u32,
    pub chain: u8,
    pub stun: u8,
    pub chill: u8,
}

/// What a body looses on the fire key, and the ticks between.
pub fn arms(kind: EnemyKind, pack: Pack) -> (Arms, u32) {
    use Emitter::*;
    use EnemyKind as K;
    let bite = |reach: f32, arc: f32, damage: u32, every: u32| {
        (
            Arms::Bite {
                reach,
                arc,
                damage,
                drain: kind == K::Leech,
            },
            every,
        )
    };
    let volley = |chain: &'static [(Emitter, Shot)], damage: u32, every: u32| (Arms::Volley { chain, damage }, every);
    match kind {
        // The Hollow One throws cold bolts between its holes; its sweeping
        // gaze is its spell.
        K::Hollow => volley(&[(Darts, Shot::Frost)], 12, 15),
        K::Bat => bite(1.3, 100.0, 16, 8),
        K::Warboar => bite(1.6, 90.0, 34, 16),
        K::Slime if pack != Pack::Fungal => bite(1.5, 120.0, 22, 12),
        K::Flesher => bite(1.9, 100.0, 30, 14),
        K::Spiderling | K::Biter => bite(1.1, 90.0, 12, 7),
        K::Shard => bite(1.2, 90.0, 16, 8),
        K::Mole => bite(1.5, 110.0, 26, 12),
        K::Leech => bite(1.4, 80.0, 18, 10),
        K::BogHand => bite(1.6, 120.0, 24, 12),
        K::Drowned => bite(2.0, 110.0, 34, 18),
        K::Hound => volley(&[(Lead, Shot::Ember)], 12, 14),
        K::Sapper => volley(&[(Lob, Shot::Ball)], 30, 24),
        K::Hob => volley(&[(Lob, Shot::Ball)], 26, 20),
        K::Spitter if pack != Pack::Fungal => volley(&[(Lob, Shot::Spore)], 16, 18),
        K::Hexer => volley(&[(Burst, Shot::Spore)], 12, 15),
        K::Lich => volley(&[(Darts, Shot::Frost)], 12, 15),
        K::Marksman => volley(&[(Darts, Shot::Bolt)], 20, 24),
        K::Mirrorling => volley(&[(Burst, Shot::Wisp)], 12, 14),
        K::Geode => volley(&[(GapRing, Shot::Ore)], 10, 26),
        K::Broodmarrow => volley(&[(Lob, Shot::Flame)], 24, 22),
        K::Wisp => volley(&[(Flower, Shot::Wisp)], 10, 24),
        _ => match patterns::signature(kind, pack, false) {
            Some(sig) => volley(sig.chain, sig.damage, (sig.every / 4).clamp(BODY_QUICKEST, BODY_SLOWEST)),
            None => volley(&[(Burst, Shot::Wisp)], 12, 15),
        },
    }
}

/// What a body casts on a spell key, if it casts anything.
pub fn spell(kind: EnemyKind) -> Option<Spell> {
    use EnemyKind as K;
    let cast = |name, emitter, shot, damage| Spell {
        name,
        emitter,
        shot,
        damage,
        chain: 0,
        stun: 0,
        chill: 0,
    };
    Some(match kind {
        // Rimeleap: frost that turns after the monsters and leaps on.
        K::Lich => Spell {
            chain: lich::BOUNCES,
            chill: 2,
            ..cast("Rimeleap", Emitter::Seeker, Shot::Frost, 30)
        },
        // The hex: what it strikes stands dazed.
        K::Hexer => Spell {
            stun: hexer::HEX_TICKS.min(255) as u8,
            ..cast("Hex", Emitter::Burst, Shot::Hex, 10)
        },
        K::Necromancer => cast("Grave Call", Emitter::Reaim, Shot::Wisp, 18),
        K::Shaman => cast("Fire Rain", Emitter::Rain, Shot::Flame, 20),
        K::Hollow => cast("Void Gaze", Emitter::Sweep, Shot::Eye, 30),
        K::Demon => cast("Hellsweep", Emitter::Sweep, Shot::Flame, 30),
        K::Wraith => cast("Stare", Emitter::Laser, Shot::Eye, 30),
        _ => return None,
    })
}

/// What the knight's fire slot says while it walks in `kind`'s body.
pub fn arms_name(kind: EnemyKind, pack: Pack) -> String {
    match arms(kind, pack).0 {
        Arms::Bite { drain: true, .. } => "Drink".into(),
        Arms::Bite { .. } => "Bite".into(),
        Arms::Volley { chain, .. } => {
            let (emitter, shot) = chain[0];
            let word = |s: &str| {
                let mut c = s.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
            };
            let emitter: Vec<String> = emitter.name().split('_').map(word).collect();
            format!("{:?} {}", shot, emitter.join(" "))
        }
    }
}

/// What a borrowed body did on the fire key.
pub(super) enum Loosed {
    /// This many shots.
    Volley(usize),
    /// The monsters its strike reached, and what each took.
    Bite(Vec<(usize, u32)>),
}

/// Where a borrowed volley is thrown: the nearest monster near the knight's
/// aim, else a way down the aim.
fn mark(hero: &Hero, enemies: &[Enemy]) -> (f32, f32) {
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    enemies
        .iter()
        .filter(|e| e.hp > 0 && !e.buried())
        .filter_map(|e| {
            let (dx, dy) = (e.x - hero.x, e.y - hero.y);
            let d = dx.hypot(dy);
            (d > 0.0 && d <= MARK_REACH && (dx * ax + dy * ay) / d >= MARK_CONE.cos()).then_some((d, (e.x, e.y)))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or((hero.x + ax * MARK_REACH / 2.0, hero.y + ay * MARK_REACH / 2.0), |(_, at)| at)
}

/// One volley of `emitter` from the knight, turned friend.
#[allow(clippy::too_many_arguments)]
fn throw(
    hero: &Hero,
    id: u32,
    grid: &Grid,
    enemies: &[Enemy],
    salt: u64,
    (emitter, kind, damage): (Emitter, Shot, u32),
    shots: &mut Vec<Projectile>,
) -> usize {
    let at = mark(hero, enemies);
    // A ring's way through faces behind the knight, not at what it aims at.
    let at = if emitter == Emitter::GapRing {
        (2.0 * hero.x - at.0, 2.0 * hero.y - at.1)
    } else {
        at
    };
    let aim = Aim {
        from: (hero.x, hero.y),
        at,
        pace: (0.0, 0.0),
        grid,
        count: hero.loosed,
        salt,
        step: 0,
        kind,
        damage,
        guardian: None,
    };
    let start = shots.len();
    patterns::emit(emitter, &aim, shots);
    for shot in &mut shots[start..] {
        shot.hostile = false;
        shot.traits.owner = id as u8;
        shot.traits.borrowed = true;
        let path = &mut shot.traits.path;
        if let Some(beam) = &mut path.beam {
            beam.warn = beam.warn.min(BODY_WARN);
            shot.ttl = u32::from(beam.warn) + u32::from(beam.burn) + 1;
            continue;
        }
        if path.form == Form::Lob {
            continue;
        }
        let speed = shot.vx.hypot(shot.vy).max(path.top);
        if speed > 0.0 {
            let k = BODY_PACE.min(BODY_TOP / speed).max(1.0);
            (shot.vx, shot.vy) = (shot.vx * k, shot.vy * k);
            path.top *= k;
            path.accel *= k;
        }
        shot.ttl = shot.ttl.min(BODY_LIFE + u32::from(path.wait));
    }
    shots.len() - start
}

/// The fire key in a borrowed body: its volley or its strike, with the
/// rearm set. `None` outside one.
pub(super) fn loose(
    hero: &mut Hero,
    id: u32,
    pack: Pack,
    grid: &Grid,
    enemies: &mut [Enemy],
    salt: u64,
    shots: &mut Vec<Projectile>,
) -> Option<Loosed> {
    let kind = hero.possessed?;
    let (arms, every) = arms(kind, pack);
    hero.fire_cooldown = every;
    match arms {
        Arms::Volley { chain, damage } => {
            let (emitter, shot) = chain[hero.loosed as usize % chain.len()];
            let n = throw(hero, id, grid, enemies, salt, (emitter, shot, damage), shots);
            hero.loosed = hero.loosed.wrapping_add(1);
            Some(Loosed::Volley(n))
        }
        Arms::Bite { reach, arc, damage, drain } => {
            hero.swing = 6;
            hero.sword = false;
            let angle = hero.aim_y.atan2(hero.aim_x);
            let (hx, hy) = (hero.x, hero.y);
            let reach = reach + kind.radius();
            let struck = aoe::strike(enemies, (hx, hy), damage, |e| {
                let (dx, dy) = (e.x - hx, e.y - hy);
                let off = (dy.atan2(dx) - angle).sin().atan2((dy.atan2(dx) - angle).cos()).abs();
                !e.buried()
                    && dx.hypot(dy) <= reach + e.radius()
                    && off <= arc.to_radians() / 2.0
                    && grid.open((hx + e.x) / 2.0, (hy + e.y) / 2.0, Mover::Hero)
            });
            for &(i, _) in &struck {
                enemies[i].wits.foe = id;
            }
            if drain {
                let drunk: u32 = struck.iter().map(|&(_, dealt)| dealt).sum::<u32>() / 2;
                hero.hp = (hero.hp + drunk).min(hero.max_hp);
            }
            Some(Loosed::Bite(struck))
        }
    }
}

/// A spell key in a borrowed body: its spell, if it casts one and it is
/// ready. Its name, when it was cast.
pub(super) fn cast(
    hero: &mut Hero,
    id: u32,
    grid: &Grid,
    enemies: &[Enemy],
    salt: u64,
    shots: &mut Vec<Projectile>,
) -> Option<&'static str> {
    let spell = spell(hero.possessed?)?;
    if hero.body_rearm > 0 {
        return None;
    }
    hero.body_rearm = SPELL_REARM;
    let start = shots.len();
    throw(hero, id, grid, enemies, salt, (spell.emitter, spell.shot, spell.damage), shots);
    for shot in &mut shots[start..] {
        shot.traits.chain = spell.chain;
        shot.traits.stun = spell.stun;
        shot.traits.chill = spell.chill;
    }
    Some(spell.name)
}

#[cfg(test)]
#[path = "../../tests/rules/together_bodies__tests.rs"]
mod tests;
