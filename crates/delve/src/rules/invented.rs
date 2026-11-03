//! A magic the game has never seen, spelled as data: the primitives the
//! engine owns, put together by a wish.
//!
//! Nothing here is compiled per spell. A wish that invents "frost lash" or
//! "nine coins of ruin" fills in a [`Spell`] and the rules compose it into
//! the same bolts every other weapon fires, so a made-up magic costs no build
//! and can be granted while the party is fighting. What a wish may not do is
//! invent a new verb: a spell that rewrites gravity is engine work, and
//! [`check`] turns away anything outside the bounds before it reaches a floor.
//!
//! The composition is deterministic on purpose — a fan is a fan, never a roll
//! — since a client replays it, and a session that cannot be replayed cannot
//! be trusted.

use serde::{Deserialize, Serialize};

/// Bolts one casting may carry. Past this a magic is a floor-clearer.
pub const MAX_BOLTS: u32 = 12;
/// Where a bolt's damage stops, whatever the wish asked for.
pub const MAX_DAMAGE: i64 = 50;
/// The widest fan, in radians either side of the aim.
pub const MAX_SPREAD: f32 = 1.5;
/// The longest a made-up magic may slow what it hits: ten seconds.
pub const MAX_SLOW: u32 = 10_000;
/// The most mana one casting may take.
pub const MAX_MANA: u32 = 100;

/// What a made-up magic is made of: a shape, a number, and what it leaves
/// behind. Every field is one of the engine's primitives.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spell {
    pub name: String,
    /// How many bolts one casting sends.
    pub bolts: u32,
    /// How wide the fan is, in radians either side of the aim.
    pub spread: f32,
    /// What one bolt is worth.
    pub damage: i64,
    pub element: Element,
    pub on_hit: OnHit,
    /// How long what it hits is slowed, for `OnHit::Slow`.
    pub slow: u32,
    /// What one casting costs from the knight's pool.
    pub mana: u32,
    /// Ticks before the spell may be cast again.
    pub cooldown: u32,
}

/// What a bolt looks and reads as. The renderer already knows these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    #[default]
    Plain,
    Frost,
    Fire,
    Coin,
}

/// What a bolt does to what it reaches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnHit {
    #[default]
    Nothing,
    Slow,
    Burn,
    Pierce,
}

/// Why a wish's magic never reached the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reject {
    Nameless,
    TooManyBolts,
    TooWide,
    TooHard,
    TooLong,
    TooDear,
    NoCooldown,
}

/// One primitive a casting puts on the field.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// A shot: where it starts, which way it goes, and what it is worth.
    Bolt {
        from: (f32, f32),
        heading: (f32, f32),
        damage: i64,
        element: Element,
        on_hit: OnHit,
        slow: u32,
    },
}

/// Whether a wish's magic may be granted at all: the bounds every made-up
/// spell passes before it is put in a deck.
pub fn check(spell: &Spell) -> Result<(), Reject> {
    if spell.name.trim().is_empty() {
        return Err(Reject::Nameless);
    }
    if spell.bolts == 0 || spell.bolts > MAX_BOLTS {
        return Err(Reject::TooManyBolts);
    }
    if !spell.spread.is_finite() || spell.spread.abs() > MAX_SPREAD {
        return Err(Reject::TooWide);
    }
    if spell.damage < 1 || spell.damage > MAX_DAMAGE {
        return Err(Reject::TooHard);
    }
    if spell.slow > MAX_SLOW {
        return Err(Reject::TooLong);
    }
    if spell.mana > MAX_MANA {
        return Err(Reject::TooDear);
    }
    if spell.cooldown == 0 {
        return Err(Reject::NoCooldown);
    }
    Ok(())
}

/// Casts a checked spell: where the caster stands, which way they aim, and the
/// primitives it puts on the field. Symmetric about the aim, the same every
/// time, so a client that replays it lands where the server does.
pub fn cast(spell: &Spell, at: (f32, f32), aim: f32) -> Vec<Effect> {
    let bolts = spell.bolts.clamp(1, MAX_BOLTS);
    (0..bolts)
        .map(|i| {
            let offset = match bolts {
                1 => 0.0,
                many => (i as f32 / (many - 1) as f32 - 0.5) * spell.spread.abs(),
            };
            let angle = aim + offset;
            Effect::Bolt {
                from: at,
                heading: (angle.cos(), angle.sin()),
                damage: spell.damage,
                element: spell.element,
                on_hit: spell.on_hit,
                slow: spell.slow,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lash() -> Spell {
        Spell {
            name: "frost lash".into(),
            bolts: 3,
            spread: 0.6,
            damage: 4,
            element: Element::Frost,
            on_hit: OnHit::Slow,
            slow: 1_200,
            mana: 12,
            cooldown: 60,
        }
    }

    #[test]
    fn a_frost_lash_is_three_bolts_across_the_aim() {
        let spell = lash();
        assert_eq!(check(&spell), Ok(()));
        let effects = cast(&spell, (0.0, 0.0), 0.0);
        assert_eq!(effects.len(), 3, "one bolt per point of the fan");
        let mut headings = Vec::new();
        for effect in &effects {
            match effect {
                Effect::Bolt { heading, .. } => headings.push(heading.1),
            }
        }
        assert!(headings[0] < 0.0 && headings[2] > 0.0, "the fan is symmetric: {headings:?}");
        assert!(headings[1].abs() < 1e-6, "the middle bolt goes where the knight aims");
        match effects[0] {
            Effect::Bolt { damage, element, on_hit, slow, .. } => {
                assert_eq!((damage, element, on_hit, slow), (4, Element::Frost, OnHit::Slow, 1_200));
            }
        }
    }

    #[test]
    fn the_same_magic_is_the_same_magic_every_time() {
        let spell = lash();
        assert_eq!(cast(&spell, (1.0, 2.0), 0.3), cast(&spell, (1.0, 2.0), 0.3));
    }

    #[test]
    fn a_wish_past_the_bounds_never_reaches_a_floor() {
        fn bent(bend: impl Fn(&mut Spell), want: Reject) {
            let mut spell = Spell {
                name: "frost lash".into(),
                bolts: 3,
                spread: 0.6,
                damage: 4,
                element: Element::Frost,
                on_hit: OnHit::Slow,
                slow: 1_200,
                mana: 12,
                cooldown: 60,
            };
            bend(&mut spell);
            assert_eq!(check(&spell), Err(want), "{spell:?}");
        }
        bent(|it| it.bolts = MAX_BOLTS + 1, Reject::TooManyBolts);
        bent(|it| it.damage = MAX_DAMAGE + 1, Reject::TooHard);
        bent(|it| it.spread = MAX_SPREAD + 0.1, Reject::TooWide);
        bent(|it| it.slow = MAX_SLOW + 1, Reject::TooLong);
        bent(|it| it.mana = MAX_MANA + 1, Reject::TooDear);
        bent(|it| it.cooldown = 0, Reject::NoCooldown);
        bent(|it| it.name = "  ".into(), Reject::Nameless);
    }

    #[test]
    fn a_casting_never_sends_more_than_the_bound_even_unchecked() {
        let mut spell = lash();
        spell.bolts = 400;
        spell.spread = 99.0;
        assert_eq!(cast(&spell, (0.0, 0.0), 0.0).len(), MAX_BOLTS as usize);
    }
}
