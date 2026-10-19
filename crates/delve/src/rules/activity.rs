//! The games played inside a room, in one registry with one lifecycle.
//!
//! Each game is an `Activity`: a name, whether it is under way, its tick,
//! how it ends, what it pays into the realm, and the `Run` fields that hold
//! it. A ride (the lists, the Hippodrome, a race hall) also takes the keys
//! of the knights it seats for the tick. The sim walks the registry in
//! order (`Run::advance` and `Run::clear_activity`), the cockpit settles
//! through it (`App::settle_home`), and the wire leaves a game's state out
//! for a seat that stands outside the party's room (`wire::seat_tree`).
//!
//! A new game is one entry here: the state on `Run` (an `Option` field with
//! `#[serde(default)]`, nothing else to mirror), a tick that does nothing
//! when its state is absent, and its row in `RIDES` or `FOLLIES`.

use super::*;

pub type Inputs = BTreeMap<u32, Input>;

/// The parts of the host's realm a game pays into at the save: the
/// Undercroft as built, the treasury, and what the realm has won. The host
/// lends them (`settle_all`); the realm itself, and its save, are the
/// host's.
pub struct Realm<'a> {
    pub home: &'a mut home::Home,
    pub treasury: &'a mut Spoils,
    pub reclaimed: &'a BTreeMap<String, u32>,
    pub raids_won: u32,
}

/// What settling a game into the realm did: whether the realm changed, and
/// what is said about it (cue, line).
#[derive(Default)]
pub struct Settled {
    pub changed: bool,
    pub said: Vec<(String, String)>,
}

pub struct Activity {
    pub name: &'static str,
    /// Whether it is under way in this run now.
    pub on: fn(&Run) -> bool,
    /// A ride's tick: it plays, then gives the keys the rest of the tick
    /// sees (None: everyone's, as they came). Only called while `on`.
    pub ride: Option<fn(&mut Run, &Inputs) -> Option<Inputs>>,
    /// Everything else's tick, at its place in the step; a game whose state
    /// is absent does nothing.
    pub tick: fn(&mut Run, &Inputs),
    /// It is over: its state cleared.
    pub end: fn(&mut Run),
    /// Ended when a knight's view leaves its room (`Run::clear_activity`).
    pub ends_outside: bool,
    /// What it pays into the realm at the save.
    pub settle: Option<fn(&mut Realm<'_>, &BTreeMap<String, u32>) -> Settled>,
    /// The `Run` fields that hold it.
    pub fields: &'static [&'static str],
}

fn idle(_: &mut Run, _: &Inputs) {}

/// Rides take the keys of the knights they seat. The first one under way
/// rides this tick.
pub static RIDES: &[Activity] = &[
    Activity {
        name: "Pennon Rush", on: |run| run.room().kind == RoomKind::Muster,
        ride: Some(|run, inputs| {run.tick_pennon(inputs);run.pennon.as_ref().map(|_| BTreeMap::new())}),
        tick: idle, end: |run| run.pennon = None, ends_outside: true,
        settle: None, fields: &["pennon"],
    },
    Activity {
        name: "Forest Road", on: Run::forest_road_showing,
        ride: Some(|run, inputs| { run.tick_forest_road(inputs); Some(BTreeMap::new()) }),
        tick: idle, end: |run| run.forest_road = None, ends_outside: true,
        settle: Some(|realm, marks| {
            let changed = marks.contains_key(forest_road::CAMPAIGN_MARK) && !realm.home.siege_campaign_begun;
            if changed { realm.home.siege_campaign_begun = true; }
            Settled { changed, said: if changed { vec![("forest".into(), "Siege campaign begun. Old Oak stands by the Dark Forest.".into())] } else { vec![] } }
        }), fields: &["forest_road"],
    },
    // The Round Table adventures: each its own entry, in its own module.
    // Lanternward
    lanternward::ACTIVITY,
    // Moonfall
    moonfall::ACTIVITY,
    // Bannerpeak
    bannerpeak::ACTIVITY,
    // Griffinwing
    griffinwing::ACTIVITY,
    // Lightgallop
    lightgallop::ACTIVITY,
    Activity {
        name: "Rafterwake", on: |run| run.room().kind == RoomKind::Workshop,
        ride: Some(|run, inputs| { run.tick_rafterwake(inputs); run.rafterwake.as_ref().map(|_| BTreeMap::new()) }),
        tick: idle, end: |run| run.rafterwake = None, ends_outside: true,
        settle: None, fields: &["rafterwake"],
    },
    Activity {
        name: "Brasswake", on: |run| run.room().kind == RoomKind::Tavern,
        ride: Some(|run, inputs| { run.tick_brasswake(inputs); run.brasswake.as_ref().map(|_| BTreeMap::new()) }),
        tick: idle, end: |run| run.brasswake = None, ends_outside: true,
        settle: None, fields: &["brasswake"],
    },
    Activity {
        name: "Mossbell", on: |run| run.room().kind == RoomKind::Bathhouse,
        ride: Some(|run, inputs| {run.tick_mossbell(inputs);run.mossbell.as_ref().map(|_| BTreeMap::new())}),
        tick: idle, end: |run| run.mossbell = None, ends_outside: true,
        settle: None, fields: &["mossbell"],
    },
    Activity {
        name: "Latch Duel",
        on: |run| run.room().kind == RoomKind::Smithy,
        ride: Some(|run, inputs| {
            run.tick_latch(inputs);
            run.latch.as_ref().map(|_| BTreeMap::new())
        }),
        tick: idle,
        end: |run| run.latch = None,
        ends_outside: true,
        settle: None,
        fields: &["latch"],
    },
    Activity {
        name: "Cinder Stack",
        on: |run| run.room().kind == RoomKind::CollegeLab,
        ride: Some(|run, inputs| {
            run.tick_cinder(inputs);
            run.cinder.as_ref().map(|_| BTreeMap::new())
        }),
        tick: idle,
        end: |run| run.cinder = None,
        ends_outside: true,
        settle: None,
        fields: &["cinder"],
    },
    Activity {
        name: "the lists",
        on: |run| run.at_home_now() && run.room().kind == RoomKind::Lists,
        ride: Some(|run, inputs| {
            run.tick_lists(inputs);
            run.joust
                .as_ref()
                .map(joust::Joust::seated)
                .map(|seated| joust::reins_all(inputs, &seated))
        }),
        tick: idle,
        end: |run| run.joust = None,
        ends_outside: true,
        settle: Some(|realm, marks| {
            let mut settled = Settled::default();
            // The stables and the lists: the saddled mount, the tending, bouts.
            settled.said = joust::settle_stable(&mut realm.home.stable, marks);
            settled.changed = marks
                .keys()
                .any(|m| m.starts_with("joust:") || m.starts_with("stable:"));
            // The horse market's pens: paid from the realm's own purse.
            settled.said.extend(joust::settle_market(
                &mut realm.home.stable,
                realm.treasury,
                marks,
            ));
            // The lists' lost entries and wagers: paid from the realm's purse.
            let lists = joust::settle_lists(realm.treasury, marks);
            settled.changed |= !lists.is_empty();
            settled.said.extend(lists);
            settled
        }),
        fields: &["joust"],
    },
    Activity {
        name: "the Hippodrome",
        on: |run| run.at_home_now() && run.room().kind == RoomKind::Hippodrome && !run.race3d_showing(),
        ride: Some(|run, inputs| {
            run.tick_hippodrome_race(inputs);
            if run.race3d_showing() { return Some(BTreeMap::new()); }
            // Every driver's keys are the race's: the one who boarded and
            // each friend at the reins of a rival team.
            run.chariot.as_ref().map(|c| {
                let seated: Vec<u32> = c.seats().into_iter().map(|(seat, _)| seat).collect();
                joust::reins_all(inputs, &seated)
            })
        }),
        tick: idle,
        end: |run| run.chariot = None,
        ends_outside: true,
        settle: Some(|realm, marks| {
            // A herd horse's record, and its purse.
            let (said, purse) = chariot::settle_chariot(&mut realm.home.stable, marks);
            let mut changed = !said.is_empty();
            if purse > 0 {
                realm
                    .treasury
                    .add(crate::realm::Spoil::Gold, purse);
                changed = true;
            }
            Settled { changed, said }
        }),
        fields: &["chariot"],
    },
    Activity {
        name: "a race hall",
        on: Run::race3d_showing,
        // The keys drive the race, and the knights stand still in the hall.
        ride: Some(|run, inputs| {
            run.tick_race3d(inputs);
            Some(BTreeMap::new())
        }),
        tick: idle,
        end: |run| run.race3d = None,
        ends_outside: false,
        settle: None,
        fields: &["race3d"],
    },
];

/// The follies and the room's own small games, ticked in this order where
/// the step reaches them.
pub static FOLLIES: &[Activity] = &[
    Activity {
        name: "a field banner",
        on: |run| run.room().field_banner().is_some(),
        ride: None,
        tick: |run, inputs| run.tick_field_banner(inputs),
        end: |_| {},
        ends_outside: false,
        settle: None,
        fields: &[],
    },
    Activity {
        name: "the Wax Maze",
        on: |run| run.folly.is_some(),
        ride: None,
        tick: |run, _| run.tick_folly(),
        end: |run| run.folly = None,
        ends_outside: false,
        settle: None,
        fields: &["folly"],
    },
    Activity {
        name: "the Eyrie",
        on: |run| run.at_home_now() && run.room().kind == RoomKind::Eyrie,
        ride: None,
        tick: |run, _| run.tick_eyrie(),
        end: |_| {},
        ends_outside: false,
        settle: None,
        fields: &[],
    },
    Activity {
        name: "a nook",
        on: |run| run.nook.is_some(),
        ride: None,
        tick: |run, inputs| run.tick_nook(inputs),
        end: |run| run.nook = None,
        ends_outside: false,
        settle: None,
        fields: &["nook"],
    },
    Activity {
        name: "the Belfry Shoot",
        on: |run| run.bats.is_some(),
        ride: None,
        tick: |run, _| run.tick_bats(),
        end: |run| run.bats = None,
        ends_outside: false,
        settle: None,
        fields: &["bats"],
    },
    Activity {
        name: "the Keg Gantry",
        on: |run| run.gantry.is_some(),
        ride: None,
        tick: |run, _| run.tick_gantry(),
        end: |run| run.gantry = None,
        ends_outside: false,
        settle: None,
        fields: &["gantry"],
    },
    Activity {
        name: "the Powder Room",
        on: |run| run.powder.is_some(),
        ride: None,
        tick: |run, inputs| run.tick_powder(inputs),
        end: |run| run.powder = None,
        ends_outside: false,
        settle: None,
        fields: &["powder"],
    },
    Activity {
        name: "Book Hop",
        on: |run| run.hop.is_some(),
        ride: None,
        tick: |run, _| run.tick_book_hop(),
        end: |run| run.hop = None,
        ends_outside: false,
        settle: None,
        fields: &["hop"],
    },
];

/// A place's own game, found by its room: the first whose `on` holds.
const fn place(
    name: &'static str,
    on: fn(&Run) -> bool,
    tick: fn(&mut Run, &Inputs),
) -> Activity {
    Activity {
        name,
        on,
        ride: None,
        tick,
        end: |_| {},
        ends_outside: false,
        settle: None,
        fields: &[],
    }
}

fn in_room(run: &Run, kind: RoomKind) -> bool {
    run.room().kind == kind
}

/// The Undercroft's rooms (`Run::tick_home`): the first whose room the party
/// stands in plays, and the stair's own landing plays when none does.
pub static HOME_ROOMS: &[Activity] = &[
    place("Fortune's wheel", |run| in_room(run, RoomKind::Fortune), |run, inputs| {
        run.tick_fortune(inputs)
    }),
    place("the yard", |run| in_room(run, RoomKind::Yard), |run, inputs| {
        run.tick_yard(inputs)
    }),
    place("the Trophy Hall", |run| in_room(run, RoomKind::Trophies), |run, inputs| {
        run.tick_wing_plate(inputs);
        run.tick_trophies();
    }),
    place("the tavern", |run| in_room(run, RoomKind::Tavern), |run, inputs| {
        run.tick_tavern(inputs)
    }),
    place(
        "a quiet room",
        |run| {
            matches!(
                run.room().kind,
                RoomKind::Hall | RoomKind::Stockpile | RoomKind::Muster | RoomKind::Workshop | RoomKind::Quarters
            )
        },
        |_, _| {},
    ),
    place("the world", |run| run.room().kind.in_world(), |run, inputs| {
        run.tick_world(inputs)
    }),
];

/// The world's rooms (`Run::tick_world`), every one that is on, in order.
pub static WORLD: &[Activity] = &[
    place("the kingdom", |_| true, |run, inputs| run.tick_kingdom(inputs)),
    place("the stables", |run| in_room(run, RoomKind::Stables), |run, inputs| {
        run.tick_stables(inputs)
    }),
    place("the Dark Forest", |run| in_room(run, RoomKind::Forest), |run, _| {
        run.tick_forest()
    }),
    place("the countryside", |_| true, |run, inputs| {
        run.tick_countryside(inputs)
    }),
    place("the pastimes", |_| true, |run, inputs| run.tick_pastimes(inputs)),
];

/// The realm's standing places, ticked every step in this order, and what
/// they pay into the realm at the save.
pub static PLACES: &[Activity] = &[
    place("the Undercroft", |run| run.dungeon.depth == 0, |run, inputs| {
        if run.dungeon.depth == 0 {
            run.tick_home(inputs);
        }
    }),
    place("the barony", |_| true, |run, inputs| run.tick_barony(inputs)),
    Activity {
        settle: Some(|realm, marks| {
            // The Court of Deeds: holdings bought, built, staffed, chartered
            // and their rents collected, from the realm's own purse.
            let yards = realm.home.stable.yards.clone();
            let reclaimed = realm.reclaimed.clone();
            let mut changed = holdings::note_realm(&mut realm.home.holdings, &reclaimed, &yards);
            let held = holdings::settle(&mut realm.home.holdings, realm.treasury, marks);
            changed |= held.changed;
            Settled {
                changed,
                said: held.said,
            }
        }),
        ..place("the holdings", |_| true, |run, inputs| run.tick_holdings(inputs))
    },
    place("Labyrinth College", |_| true, |run, inputs| run.tick_college(inputs)),
    Activity {
        settle: Some(|realm, marks| {
            // The horse ranch: a herd mare brought to Helios, the fee, the roll.
            let raids = realm.raids_won;
            let (changed, said) = ranch::settle(
                &mut realm.home.ranch,
                realm.treasury,
                &mut realm.home.stable,
                raids,
                marks,
            );
            Settled {
                changed,
                said: said.into_iter().map(|line| ("ranch".to_string(), line)).collect(),
            }
        }),
        ..place("the ranch", |_| true, |run, inputs| run.tick_ranch(inputs))
    },
    Activity {
        settle: Some(|realm, marks| {
            // The Swamp of Horrors: the notice read, and the white horse
            // into the herd once she is out of the pen.
            let (changed, said) = swamp::settle(
                &mut realm.home.countryside,
                &mut realm.home.stable,
                marks,
            );
            Settled {
                changed,
                said: said.into_iter().map(|line| ("swamp".to_string(), line)).collect(),
            }
        }),
        ..place("the Swamp of Horrors", |_| true, |run, inputs| run.tick_swamp_notice(inputs))
    },
    place("the merchant", |_| true, |run, _| run.tick_merchant()),
];

/// Every registered game and place.
pub fn all() -> impl Iterator<Item = &'static Activity> {
    RIDES
        .iter()
        .chain(FOLLIES)
        .chain(HOME_ROOMS)
        .chain(WORLD)
        .chain(PLACES)
}

impl Run {
    /// The ride under way, if any, plays its tick; the keys the rest of the
    /// step sees.
    pub(super) fn ride(&mut self, inputs: &Inputs) -> Option<Inputs> {
        let ride = RIDES.iter().find(|a| (a.on)(self))?;
        (ride.ride?)(self, inputs)
    }

    /// The follies' ticks, in order.
    pub(super) fn tick_follies(&mut self, inputs: &Inputs) {
        for folly in FOLLIES {
            (folly.tick)(self, inputs);
        }
    }

    /// The realm's places' ticks, in order.
    pub(super) fn tick_places(&mut self, inputs: &Inputs) {
        for place in PLACES {
            (place.tick)(self, inputs);
        }
    }

    /// The Undercroft room the party stands in plays; true when one did
    /// (the stair's landing plays otherwise).
    pub(super) fn tick_home_room(&mut self, inputs: &Inputs) -> bool {
        match HOME_ROOMS.iter().find(|a| (a.on)(self)) {
            Some(room) => {
                (room.tick)(self, inputs);
                true
            }
            None => false,
        }
    }

    /// Every one of the world's games that is on, in order.
    pub(super) fn tick_world_games(&mut self, inputs: &Inputs) {
        for game in WORLD {
            if (game.on)(self) {
                (game.tick)(self, inputs);
            }
        }
    }

    /// End every game that ends when a view leaves its room.
    pub(super) fn end_outside_activities(&mut self) {
        for activity in all().filter(|a| a.ends_outside) {
            (activity.end)(self);
        }
    }

    /// The names of the games under way now.
    pub fn activities_on(&self) -> Vec<&'static str> {
        all().filter(|a| (a.on)(self)).map(|a| a.name).collect()
    }
}

/// Settle every game into the realm, in registry order.
pub fn settle_all(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let mut all_settled = Settled::default();
    for settle in all().filter_map(|a| a.settle) {
        let settled = settle(realm, marks);
        all_settled.changed |= settled.changed;
        all_settled.said.extend(settled.said);
    }
    all_settled
}

#[cfg(test)]
#[path = "../../tests/rules/together_activity__tests.rs"]
mod tests;
