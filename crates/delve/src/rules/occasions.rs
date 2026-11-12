//! Occasions: what a rent day makes of a village's temper, and what the
//! village does about it where the party can see — a feast when the
//! granary is full, a tantrum when a soul is one step from walking away,
//! a mourning when somebody dies.
//!
//! Each is the holding's last rent day, kept on the holding until the
//! next one (`Holding::occasion`), so the party walking into a steward's
//! hall finds the village as the realm left it. Each costs or gives
//! something real and says only that:
//!
//! - a feast takes `FEAST_SACKS` from a granary of `FEAST_AT` or more,
//!   and every soul at the table goes home `FEAST_CHEER` the happier;
//! - a tantrum breaks one thing: a course of the thrower's own room if
//!   they staff one, else a sack off the granary if any is left — and the
//!   words say which, or that there was nothing to break;
//! - a mourning follows a death (old age, an empty granary for the frail,
//!   raiders holding the place) and is the grief the village already
//!   felt in its moods, now walked through the hall.
//!
//! The lines each says also go up on the tavern's notice board
//! (`Holdings::tidings`), where every delve begins.

use super::holdings::{room_def, Holding, ROOM_LABOUR, SLOTS};
use super::villagers::{Trade, Villager, MOOD_LEAVES};
use serde::{Deserialize, Serialize};

/// Sacks in the granary from which the village holds a feast.
pub const FEAST_AT: u32 = 30;
/// Sacks a feast eats.
pub const FEAST_SACKS: u32 = 3;
/// Souls it takes to make a feast.
pub const FEAST_FOLK: usize = 3;
/// How much happier a feast leaves everybody.
pub const FEAST_CHEER: i8 = 2;
/// A mood this low throws a tantrum: the last step before walking away.
pub const TANTRUM_AT: i8 = MOOD_LEAVES + 1;
/// Lines the tavern board keeps.
pub const TIDINGS: usize = 4;
/// The longest a line on it runs.
pub const TIDING_LEN: usize = 150;

/// What soured the one who threw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Hunger,
    NoBed,
    Fear,
    Grief,
    Restless,
}

impl Cause {
    pub fn word(self) -> &'static str {
        match self {
            Cause::Hunger => "hungry",
            Cause::NoBed => "bedless",
            Cause::Fear => "afraid",
            Cause::Grief => "grieving",
            Cause::Restless => "restless",
        }
    }
}

/// What a tantrum broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Broke {
    /// A course of the room in this bay.
    Course(u8),
    /// A sack of the granary's.
    Sack,
    /// There was nothing left to break.
    Nothing,
}

/// How a soul died.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dying {
    Years,
    Hunger,
    Raiders,
}

impl Dying {
    pub fn word(self) -> &'static str {
        match self {
            Dying::Years => "of old age",
            Dying::Hunger => "of hunger",
            Dying::Raiders => "to the raiders",
        }
    }
}

/// What a holding's last rent day made of its village.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Occasion {
    Feast,
    Tantrum { who: String, cause: Cause, broke: Broke },
    Mourning { who: String, trade: Trade, dying: Dying },
}

impl Occasion {
    pub fn valid(&self) -> bool {
        match self {
            Occasion::Feast => true,
            Occasion::Tantrum { who, broke, .. } => {
                !who.is_empty() && who.len() <= 24 && !matches!(broke, Broke::Course(b) if usize::from(*b) >= SLOTS)
            }
            Occasion::Mourning { who, .. } => !who.is_empty() && who.len() <= 24,
        }
    }
}

/// Whether a soul is too frail to live through an empty granary.
pub fn frail(f: &Villager) -> bool {
    f.trade == Trade::Child || f.age >= super::villagers::ELDER
}

/// The village feasts, if the granary is full enough and everyone at the
/// table is fed. The sacks come out of the granary; the line says so.
pub fn feast(w: &mut Holding, stores: &mut u32, name: &str) -> Option<String> {
    if w.occasion.is_some()
        || w.raid > 0
        || *stores < FEAST_AT
        || w.folk.len() < FEAST_FOLK
        || w.folk.iter().any(|f| f.hunger > 0)
    {
        return None;
    }
    *stores -= FEAST_SACKS;
    for f in &mut w.folk {
        f.mood = (f.mood + FEAST_CHEER).clamp(-super::villagers::MOOD_CAP, super::villagers::MOOD_CAP);
    }
    w.occasion = Some(Occasion::Feast);
    Some(format!("The granary is full: {name} holds a feast, {FEAST_SACKS} sacks eaten"))
}

/// The first soul at `TANTRUM_AT` throws a tantrum. `cause` names what
/// soured them, from the day's state. The thing broken comes out of the
/// holding or the granary; the line says which.
pub fn tantrum(w: &mut Holding, stores: &mut u32, name: &str, cause: impl Fn(usize, &Villager) -> Cause) -> Option<String> {
    if w.occasion.is_some() {
        return None;
    }
    let n = w.folk.iter().position(|f| f.mood <= TANTRUM_AT && f.mood > MOOD_LEAVES)?;
    let cause = cause(n, &w.folk[n]);
    let (who, trade) = (w.folk[n].name.clone(), w.folk[n].trade);
    let bay = trade.family().and_then(|family| {
        w.rooms
            .iter()
            .position(|r| r.standing() && room_def(&r.id).is_some_and(|d| d.family == family))
    });
    let (broke, what) = match bay {
        Some(b) => {
            w.rooms[b].labour = ROOM_LABOUR - 1;
            let room = room_def(&w.rooms[b].id).map_or("room", |d| d.name);
            (Broke::Course(b as u8), format!("breaks a course of the {room}"))
        }
        None if *stores > 0 => {
            *stores -= 1;
            (Broke::Sack, "spills a sack of grain".to_string())
        }
        None => (Broke::Nothing, "finds nothing left to break".to_string()),
    };
    // Venting ends it for the day; the cause stands, and so will the next.
    w.folk[n].mood += 1;
    w.occasion = Some(Occasion::Tantrum { who: who.clone(), cause, broke });
    Some(format!("{who} the {} is {} and throws a tantrum at {name}: {what}", trade.word(), cause.word()))
}

/// The village mourns `who`, who died `dying`.
pub fn mourn(w: &mut Holding, who: &str, trade: Trade, dying: Dying) {
    w.occasion = Some(Occasion::Mourning { who: who.to_string(), trade, dying });
}

/// A tiding for the tavern's board: the ledger's line, upper-cased and
/// cut to the board's width.
pub fn tiding(line: &str) -> String {
    line.to_uppercase().chars().take(TIDING_LEN).collect()
}
