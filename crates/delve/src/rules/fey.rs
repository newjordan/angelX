//! The fey craftsman: a practiced hand of a trade the village keeps seizes
//! the workshop and makes a named artifact — a real curio (`curios`) the
//! knights can take from the workshop and carry in their pockets.
//!
//! A craftsman of grade `FEY_SKILL` or more — fed, content, in their prime,
//! with their workshop standing and no masterwork of theirs yet — takes
//! the workshop when the granary can pay `FEY_SACKS` for the work. For
//! `FEY_DAYS` rent days the workshop is theirs: its bay gives the party
//! nothing, and the board says whose it is. Then the work stands
//! finished (`Masterwork`), and its maker keeps vigil beside it until a
//! knight takes it (`Run::folk_talk`).
//!
//! The artifact does what its card says because its card is made from
//! what it does: each design is a few curio rules, and the boon and bane
//! its card reads are written out of those rules (`describe`), never
//! typed beside them. One knight carries it at a time; it is in the
//! workshop again for the next run.

use super::curios::{Curio, Icon, Rule};
use super::holdings::{room_def, Family, Holding};
use super::villagers::{Trade, Villager, COMING_OF_AGE, ELDER, NAMES};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Skill grade from which a craftsman is seized.
pub const FEY_SKILL: u8 = 2;
/// Sacks the work eats, taken when it begins.
pub const FEY_SACKS: u32 = 4;
/// Rent days the workshop is the craftsman's.
pub const FEY_DAYS: u8 = 2;
/// Masterworks one holding keeps.
pub const MAX_WORKS: usize = 3;

/// A craftsman at their seized workshop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seizure {
    pub who: String,
    pub trade: Trade,
    /// Rent days of work left.
    pub days: u8,
}

/// A finished artifact standing in its maker's workshop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Masterwork {
    pub maker: String,
    pub trade: Trade,
}

impl Seizure {
    pub fn valid(&self) -> bool {
        !self.who.is_empty() && self.who.len() <= 24 && is_craft(self.trade) && (1..=FEY_DAYS).contains(&self.days)
    }
}

impl Masterwork {
    pub fn valid(&self) -> bool {
        is_craft(self.trade) && artifact(&id_of(&self.maker, self.trade)).is_some()
    }

    /// The curio this is.
    pub fn curio(&self) -> Option<&'static Curio> {
        artifact(&id_of(&self.maker, self.trade))
    }
}

/// The trades that keep a workshop.
pub fn is_craft(trade: Trade) -> bool {
    matches!(trade, Trade::Brewer | Trade::Smith | Trade::Acolyte | Trade::Trader)
}

/// The curio id of the work `maker` of `trade` makes.
pub fn id_of(maker: &str, trade: Trade) -> String {
    format!("fey-{}-{}", maker.to_lowercase(), trade.word())
}

/// A design: what a craft's masterwork is called and what its rules do.
struct Design {
    trade: Trade,
    thing: &'static str,
    rules: &'static [Rule],
}

const DESIGNS: [Design; 8] = [
    Design { trade: Trade::Smith, thing: "Whetted Edge", rules: &[Rule::Damage(25), Rule::Pace(-10)] },
    Design { trade: Trade::Smith, thing: "Tempered Plate", rules: &[Rule::Hurt(-25), Rule::Pace(-15)] },
    Design { trade: Trade::Brewer, thing: "Second Cask", rules: &[Rule::Snack(15), Rule::Fire(-15)] },
    Design { trade: Trade::Brewer, thing: "Long Draught", rules: &[Rule::Hale { full: 30, hurt: -10 }, Rule::Pace(-10)] },
    Design { trade: Trade::Acolyte, thing: "Quiet Censer", rules: &[Rule::Hurt(-20), Rule::Fire(-15)] },
    Design { trade: Trade::Acolyte, thing: "Reliquary", rules: &[Rule::Snack(10), Rule::Hurt(-10), Rule::Damage(-15)] },
    Design { trade: Trade::Trader, thing: "Fair Scales", rules: &[Rule::Gold(40), Rule::Hurt(10)] },
    Design { trade: Trade::Trader, thing: "Ledger of Debts", rules: &[Rule::Gold(25), Rule::Coupon(20), Rule::Damage(-10)] },
];

/// A rule's words: (is it a boon, what it does), one or two a rule.
pub fn describe(rule: Rule) -> Vec<(bool, String)> {
    use Rule::*;
    let more = |n: i32, up: &str, down: &str| (n > 0, if n > 0 { format!("{n}% {up}") } else { format!("{}% {down}", -n) });
    match rule {
        Damage(n) => vec![more(n, "MORE DAMAGE", "LESS DAMAGE")],
        Fire(n) => vec![(n > 0, if n > 0 { format!("FIRE {n}% FASTER") } else { format!("FIRE {}% SLOWER", -n) })],
        Pace(n) => vec![(n > 0, if n > 0 { format!("WALK {n}% FASTER") } else { format!("WALK {}% SLOWER", -n) })],
        Hurt(n) => vec![(n < 0, if n < 0 { format!("{}% LESS FROM EVERY BLOW", -n) } else { format!("{n}% MORE FROM EVERY BLOW") })],
        Gold(n) => vec![more(n, "MORE GOLD FROM EVERY PICK-UP", "LESS GOLD FROM EVERY PICK-UP")],
        Snack(h) => vec![(true, format!("+{h} HEALTH WHEN A ROOM CLEARS"))],
        Coupon(n) => vec![(true, format!("{n}% OFF EVERYTHING YOU BUY"))],
        Hale { full, hurt } => vec![
            (true, format!("{full}% MORE DAMAGE AT FULL HEALTH")),
            (false, format!("{}% LESS DAMAGE WHEN HURT", -hurt)),
        ],
        _ => Vec::new(),
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn table() -> &'static Vec<Curio> {
    static TABLE: OnceLock<Vec<Curio>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out = Vec::new();
        for name in NAMES {
            for trade in [Trade::Brewer, Trade::Smith, Trade::Acolyte, Trade::Trader] {
                let design = design_of(name, trade);
                let (mut boon, mut bane) = (Vec::new(), Vec::new());
                for rule in design.rules {
                    for (good, text) in describe(*rule) {
                        if good { boon.push(text) } else { bane.push(text) }
                    }
                }
                out.push(Curio {
                    id: leak(id_of(name, trade)),
                    name: leak(format!("{name}'s {}", design.thing)),
                    price: 0,
                    says: leak(format!("{name} made it in a fey mood, and would not say how.")),
                    boon: leak(boon.join(", ")),
                    bane: leak(bane.join(", ")),
                    words: &[],
                    rules: design.rules,
                    icon: match trade {
                        Trade::Smith => Icon::Spoon,
                        Trade::Brewer => Icon::Jar,
                        Trade::Acolyte => Icon::Bell,
                        _ => Icon::Scroll,
                    },
                });
            }
        }
        out
    })
}

/// The design a craftsman's hands settle on: fixed by their name and trade.
fn design_of(maker: &str, trade: Trade) -> &'static Design {
    let kin: Vec<&Design> = DESIGNS.iter().filter(|d| d.trade == trade).collect();
    kin[(super::villagers::mix(super::villagers::seed_of(maker), 29) % kin.len() as u64) as usize]
}

/// The artifact called `id`, if some craftsman of the realm's folk could
/// make it.
pub fn artifact(id: &str) -> Option<&'static Curio> {
    table().iter().find(|c| c.id == id)
}

/// The bay of the workshop `trade` keeps at `w`: the first standing room
/// of its family, as the roster and the boards read it.
pub fn workshop(w: &Holding, trade: Trade) -> Option<usize> {
    let family: Family = trade.family()?;
    w.rooms.iter().position(|r| r.standing() && room_def(&r.id).is_some_and(|d| d.family == family))
}

/// Whether the workshop in `bay` is a craftsman's, seized.
pub fn seized_bay(w: &Holding, bay: usize) -> bool {
    w.seized.as_ref().is_some_and(|s| workshop(w, s.trade) == Some(bay))
}

fn candidate(w: &Holding, f: &Villager) -> bool {
    f.skill >= FEY_SKILL
        && f.hunger == 0
        && f.mood >= 2
        && (COMING_OF_AGE..ELDER).contains(&f.age)
        && is_craft(f.trade)
        && artifact(&id_of(&f.name, f.trade)).is_some()
        && !w.works.iter().any(|m| m.maker == f.name)
        && workshop(w, f.trade).is_some()
}

/// One rent day of the fey: a work in hand comes a day nearer done, or
/// finishes; with none in hand, a craftsman worth it is seized if the
/// granary can pay. The lines it says.
pub fn tend(w: &mut Holding, stores: &mut u32, name: &str) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(mut s) = w.seized.take() {
        let here = w.folk.iter().any(|f| f.name == s.who && f.trade == s.trade);
        if !here {
            said.push(format!("{} is gone from {name}: the unfinished work is left to rust", s.who));
            return said;
        }
        s.days = s.days.saturating_sub(1);
        if s.days > 0 {
            w.seized = Some(s);
            return said;
        }
        let work = Masterwork { maker: s.who.clone(), trade: s.trade };
        if let Some(curio) = work.curio() {
            said.push(format!(
                "{} the {} finishes {} at {name}: the knights may take it",
                s.who,
                s.trade.word(),
                curio.name
            ));
            if let Some(f) = w.folk.iter_mut().find(|f| f.name == s.who) {
                f.mood = (f.mood + 3).min(super::villagers::MOOD_CAP);
            }
            w.works.push(work);
        }
        return said;
    }
    if w.works.len() >= MAX_WORKS || *stores < FEY_SACKS {
        return said;
    }
    let Some(n) = w.folk.iter().position(|f| candidate(w, f)) else {
        return said;
    };
    *stores -= FEY_SACKS;
    let (who, trade) = (w.folk[n].name.clone(), w.folk[n].trade);
    let room = workshop(w, trade).and_then(|b| room_def(&w.rooms[b].id)).map_or("workshop", |d| d.name);
    said.push(format!(
        "{who} the {} is seized by a fey mood at {name}: the {room} is theirs for {FEY_DAYS} rent days, {FEY_SACKS} sacks for the work",
        trade.word()
    ));
    w.seized = Some(Seizure { who, trade, days: FEY_DAYS });
    said
}
