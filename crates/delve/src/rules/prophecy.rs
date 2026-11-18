//! The prophecy gate: a wish is measured on a fork of the run before it is
//! granted, and *never* refused.
//!
//! A wish that would end the floor in four seconds is still a wish someone
//! asked for. So this gate has no `Reject`: it finds the strongest version of
//! the wish the floor can hold ([`fit`]), and when even the weakest version is
//! too much, it raises the price instead of saying no. The wish always lands.
//!
//! That shape is deliberate. A checker that can say no is a checker the wish
//! pipeline routes around, and then the crazy asks stop arriving. A gate that
//! only ever answers "granted, like this, for this price" keeps every door
//! open and keeps the floor survivable.
//!
//! Nothing here simulates anything: the caller hands in a measuring function
//! (the real one forks `Run` and steps it), so this stays pure, fast and
//! testable without a dungeon.

pub const FLOOR: f32 = 0.25;
pub const PRICE_STEP: f32 = 0.5;
pub const MAX_MARKUP: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Wish {
    pub name: String,
    pub potency: f32,
    pub effects: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    pub party_damage: f32,
    pub party_deaths: u32,
    pub enemies_killed: u32,
    pub entities: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    pub max_damage: f32,
    pub max_deaths: u32,
    pub max_entities: u32,
    pub max_kills: u32,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance { max_damage: 0.6, max_deaths: 0, max_entities: 24, max_kills: 40 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ruling {
    pub granted: Wish,
    pub price: f32,
    pub measured: Outcome,
    pub says: String,
}

pub fn fit<F>(wish: &Wish, tolerance: &Tolerance, mut simulate: F) -> Ruling
where
    F: FnMut(&Wish) -> Outcome,
{
    let asked = wish.potency.max(FLOOR);
    let mut potency = asked;
    let mut measured = simulate(&wish_at(wish, potency));
    let mut price = 1.0;

    while !holds(&measured, tolerance) && potency > FLOOR {
        potency = (potency / 2.0).max(FLOOR);
        measured = simulate(&wish_at(wish, potency));
    }

    while !holds(&measured, tolerance) && price < MAX_MARKUP {
        price += PRICE_STEP;
    }

    let granted = wish_at(wish, potency);
    let says = if potency >= asked && price <= 1.0 {
        format!("granted as asked: {}", granted.name)
    } else {
        let strength = match potency < asked {
            true => format!("at {:.0}% strength", potency * 100.0),
            false => "as asked".to_owned(),
        };
        match price > 1.0 {
            true => format!(
                "granted {strength} and at {price:.1}x its price: {} did too much to the floor, \
                 so the dungeon will notice it",
                granted.name
            ),
            false => format!(
                "granted {strength}: {} at full strength did too much to the floor",
                granted.name
            ),
        }
    };
    Ruling { granted, price, measured, says }
}

fn wish_at(wish: &Wish, potency: f32) -> Wish {
    Wish { potency, ..wish.clone() }
}

fn holds(outcome: &Outcome, tolerance: &Tolerance) -> bool {
    outcome.party_damage <= tolerance.max_damage
        && outcome.party_deaths <= tolerance.max_deaths
        && outcome.entities <= tolerance.max_entities
        && outcome.enemies_killed <= tolerance.max_kills
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wish() -> Wish {
        Wish { name: "a little barracks".into(), potency: 1.0, effects: 3 }
    }

    fn floor(per_potency: f32) -> impl FnMut(&Wish) -> Outcome {
        move |wish: &Wish| Outcome {
            party_damage: per_potency * wish.potency,
            party_deaths: 0,
            enemies_killed: (10.0 * wish.potency) as u32,
            entities: (4.0 * wish.potency) as u32,
        }
    }

    #[test]
    fn a_wish_that_fits_is_granted_exactly_as_asked() {
        let ruling = fit(&wish(), &Tolerance::default(), floor(0.4));
        assert_eq!(ruling.granted.potency, 1.0);
        assert_eq!(ruling.price, 1.0);
        assert!(ruling.says.contains("granted as asked"));
    }

    #[test]
    fn a_wish_that_does_too_much_is_granted_weaker_never_refused() {
        let ruling = fit(&wish(), &Tolerance::default(), floor(1.2));
        assert!(ruling.granted.potency < 1.0, "played weaker: {}", ruling.granted.potency);
        assert!(ruling.granted.potency >= FLOOR, "never below the floor");
        assert!(ruling.says.contains("strength"), "{}", ruling.says);
        assert_eq!(ruling.granted.effects, 3, "the wish itself is whole");
    }

    #[test]
    fn a_wish_that_ends_the_floor_at_any_strength_is_still_granted_and_priced() {
        let lethal = |_: &Wish| Outcome {
            party_damage: 5.0,
            party_deaths: 4,
            enemies_killed: 900,
            entities: 400,
        };
        let ruling = fit(&wish(), &Tolerance::default(), lethal);
        assert_eq!(ruling.granted.potency, FLOOR, "as weak as it can be played");
        assert!(ruling.price > 1.0, "and priced: {}", ruling.price);
        assert!(ruling.says.contains("dungeon will notice"), "{}", ruling.says);
    }

    #[test]
    fn the_gate_measures_at_most_four_times_and_never_searches() {
        let mut calls = 0;
        let counting = |wish: &Wish| {
            calls += 1;
            Outcome {
                party_damage: 9.0 * wish.potency,
                party_deaths: 1,
                enemies_killed: 0,
                entities: 0,
            }
        };
        let _ = fit(&wish(), &Tolerance::default(), counting);
        assert!(calls <= 4, "the gate is cheap: {calls} measurements");
    }

    #[test]
    fn the_same_wish_on_the_same_floor_is_granted_the_same_way() {
        let a = fit(&wish(), &Tolerance::default(), floor(1.2));
        let b = fit(&wish(), &Tolerance::default(), floor(1.2));
        assert_eq!(a, b, "a gate that answers differently each time is not a gate");
    }

    #[test]
    fn there_is_no_ruling_that_refuses() {
        for damage in [0.0, 0.5, 1.0, 10.0, 1_000.0] {
            let ruling = fit(&wish(), &Tolerance::default(), move |wish: &Wish| Outcome {
                party_damage: damage * wish.potency,
                party_deaths: 0,
                enemies_killed: 0,
                entities: 1,
            });
            assert!(ruling.granted.potency >= FLOOR, "granted at {damage}");
            assert!(ruling.price >= 1.0);
            assert!(!ruling.says.is_empty());
        }
    }
}
