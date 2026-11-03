//! A wish that puts something on the field and keeps making more of it: a
//! barracks, a nest, a beacon, a well that refills a party's shots.
//!
//! The Delve already has everything this needs except the wish: placed things
//! (`altar`, `cauldron`, `bazaar`), allies with their own bodies and minds
//! (`hireling::AllyKind`), and a summon in the card vocabulary. What a wish
//! decides is the *policy* — what appears, how often, and how many may be
//! alive at once — so this module is that policy and nothing else.
//!
//! Everything here is bounded, because a barracks is the first wish that can
//! multiply entities: [`MAX_UNITS`] alive, no faster than [`MIN_EVERY`] ticks,
//! and gone after [`MAX_LIFETIME`]. A wish that asks for more is refused
//! ([`Reject`]) before it ever reaches a floor, and [`plan`] clamps anyway.

use serde::{Deserialize, Serialize};

/// Units one wish's structure may keep alive at once.
pub const MAX_UNITS: u32 = 9;
/// The quickest a structure may add one: half a second at 30 Hz.
pub const MIN_EVERY: u32 = 15;
/// The longest a wished structure may stand: ten minutes of play.
pub const MAX_LIFETIME: u32 = 10 * 60 * 30;
/// What a wished structure may be worth, hit for hit.
pub const MAX_ARMOUR: i64 = 500;
/// The most mana one casting may take.
pub const MAX_MANA: u32 = 100;

/// What a wish's structure makes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// A blade that walks to the nearest foe.
    #[default]
    Ghoul,
    /// A blade that stands where it is put.
    Warden,
    /// A thing that shoots from where it stands.
    Totem,
    /// A blade that is slow and hard to kill.
    Zombie,
}

/// What a wish puts on the field: a body, how often it makes a unit, how many
/// may be alive, and how long it stands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Field {
    /// What the wish calls it: "a little barracks".
    pub name: String,
    /// What one casting of this wish places.
    pub unit: Unit,
    /// Ticks between one unit and the next.
    pub every: u32,
    /// Units alive from this structure at once.
    pub cap: u32,
    /// Ticks the structure stands before it is gone.
    pub life: u32,
    /// What the structure itself is worth, so foes can break it.
    pub armour: i64,
    /// What one casting costs from the knight's pool.
    pub mana: u32,
    /// Ticks before the wish may be cast again.
    pub cooldown: u32,
}

/// Why a wish's structure never reached the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reject {
    Nameless,
    /// More units than a floor may hold.
    TooManyUnits,
    /// A structure that makes units quicker than a breath.
    TooFast,
    /// A structure that outlasts the floor.
    TooLong,
    /// A structure no foe could break.
    TooHard,
    /// More mana than the pool holds.
    TooDear,
    NoCooldown,
}

/// Whether a wish's structure may be granted at all.
pub fn check(field: &Field) -> Result<(), Reject> {
    if field.name.trim().is_empty() {
        return Err(Reject::Nameless);
    }
    if field.cap == 0 || field.cap > MAX_UNITS {
        return Err(Reject::TooManyUnits);
    }
    if field.every < MIN_EVERY {
        return Err(Reject::TooFast);
    }
    if field.life == 0 || field.life > MAX_LIFETIME {
        return Err(Reject::TooLong);
    }
    if field.armour < 1 || field.armour > MAX_ARMOUR {
        return Err(Reject::TooHard);
    }
    if field.mana > MAX_MANA {
        return Err(Reject::TooDear);
    }
    if field.cooldown == 0 {
        return Err(Reject::NoCooldown);
    }
    Ok(())
}

/// Whether a standing structure adds a unit on this tick, and what it adds.
/// `standing` is the ticks it has been up, `alive` how many of its units are
/// alive, and `fallen` whether its body has been broken — a broken barracks
/// makes nothing more, what it made walks on.
pub fn plan(field: &Field, standing: u32, alive: u32, fallen: bool) -> Option<Unit> {
    if fallen || standing >= field.life {
        return None;
    }
    let cap = field.cap.clamp(1, MAX_UNITS);
    if alive >= cap {
        return None;
    }
    let every = field.every.max(MIN_EVERY);
    // The first unit comes on the first interval, not at once: a wish that
    // put a barracks down should still cost the fight time.
    if standing == 0 || standing % every != 0 {
        return None;
    }
    Some(field.unit)
}

/// Ticks a wished structure stands, clamped to what a floor allows.
pub fn lifetime(field: &Field) -> u32 {
    field.life.clamp(1, MAX_LIFETIME)
}

/// Units a wished structure may have alive, clamped.
pub fn cap(field: &Field) -> u32 {
    field.cap.clamp(1, MAX_UNITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barracks() -> Field {
        Field {
            name: "a little barracks".into(),
            unit: Unit::Ghoul,
            every: 60,
            cap: 4,
            life: 900,
            armour: 40,
            mana: 30,
            cooldown: 600,
        }
    }

    #[test]
    fn a_barracks_makes_a_unit_every_interval_and_no_more_than_its_cap() {
        let field = barracks();
        assert_eq!(check(&field), Ok(()));
        assert_eq!(plan(&field, 0, 0, false), None, "nothing on the tick it is placed");
        assert_eq!(plan(&field, 60, 0, false), Some(Unit::Ghoul));
        assert_eq!(plan(&field, 120, 1, false), Some(Unit::Ghoul));
        assert_eq!(plan(&field, 180, 4, false), None, "the cap holds");
        assert_eq!(plan(&field, 900, 0, false), None, "it is gone at its life");
    }

    #[test]
    fn a_broken_barracks_makes_nothing() {
        let field = barracks();
        assert_eq!(plan(&field, 60, 0, true), None);
    }

    #[test]
    fn a_wish_past_the_bounds_never_reaches_a_floor() {
        fn bent(bend: impl Fn(&mut Field), want: Reject) {
            let mut field = barracks();
            bend(&mut field);
            assert_eq!(check(&field), Err(want), "{field:?}");
        }
        bent(|it| it.cap = MAX_UNITS + 1, Reject::TooManyUnits);
        bent(|it| it.cap = 0, Reject::TooManyUnits);
        bent(|it| it.every = 1, Reject::TooFast);
        bent(|it| it.life = MAX_LIFETIME + 1, Reject::TooLong);
        bent(|it| it.armour = 0, Reject::TooHard);
        bent(|it| it.mana = MAX_MANA + 1, Reject::TooDear);
        bent(|it| it.cooldown = 0, Reject::NoCooldown);
        bent(|it| it.name = " ".into(), Reject::Nameless);
    }

    #[test]
    fn an_unchecked_wish_still_cannot_flood_a_floor() {
        let mut field = barracks();
        field.cap = 400;
        field.every = 0;
        field.life = 10_000_000;
        assert_eq!(cap(&field), MAX_UNITS);
        assert_eq!(lifetime(&field), MAX_LIFETIME);
        // One unit per allowed interval at most, and never past the cap.
        assert_eq!(plan(&field, 1, 0, false), None, "no quicker than the floor");
        assert_eq!(plan(&field, MIN_EVERY, MAX_UNITS, false), None, "the cap holds");
        assert_eq!(plan(&field, MIN_EVERY, 0, false), Some(field.unit));
    }
}
