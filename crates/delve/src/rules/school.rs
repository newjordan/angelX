//! The School of the Seven Banners: class missions, ranks and earned gear.
//!
//! A master of each class stands at that class's banner stone in the
//! kingdom (each hall its own schoolroom, so the knight meets their master
//! where they already change class). E at the stone opens the master's
//! board: three missions — Novice, Adept, Master — each a concrete deed the
//! game already counts. Finishing them raises the knight's rank (Squire,
//! Adept, Master), kept by name in the realm save, and the Adept and Master
//! missions hand over signature gear that never drops.
//!
//! Ranks gate the strong powers: every wish tier and every card carries the
//! rank it needs (`LOCKS`, one table). A Squire cannot take a Master card
//! from the floor — it stays for someone who can — and a locked wish
//! answers with its rank and the nearest mission that earns it. Cards a
//! knight already holds stay held: no one loses what they have.

use super::*;
use super::classes::{self, Class};
use super::home::Home;
use std::collections::BTreeSet;

/// A school rank: Squire is where every knight starts, and where an old
/// save loads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    #[default]
    Squire,
    Adept,
    Master,
}

impl std::fmt::Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl Rank {
    pub fn name(self) -> &'static str {
        match self {
            Rank::Squire => "Squire",
            Rank::Adept => "Adept",
            Rank::Master => "Master",
        }
    }

    /// What the sheet says the knight must do next.
    pub fn next_step(self) -> Option<&'static str> {
        match self {
            Rank::Squire => Some("finish your class's Novice mission at its banner stone"),
            Rank::Adept => Some("finish the Adept mission to be a Master"),
            Rank::Master => None,
        }
    }
}

/// One mission on a master's board. `deed` is the counter the run marks;
/// `need` is the fixed number that finishes it.
pub struct Mission {
    pub id: &'static str,
    pub class: &'static str,
    pub step: &'static str,
    pub asks: &'static str,
    pub deed: &'static str,
    pub need: u32,
    /// The signature card this mission hands over (Adept and Master).
    pub gear: Option<&'static str>,
}

/// The 27 missions: three per class, each a deed the game already counts
/// (or can count cheaply at a hook the school owns).
pub static MISSIONS: &[Mission] = &[
    // Archer (Rookery): the long strand.
    Mission {
        id: "archer-novice",
        class: "Archer",
        step: "Novice",
        asks: "Put one arrow through two foes",
        deed: "archer-pierce",
        need: 3,
        gear: None,
    },
    Mission {
        id: "archer-adept",
        class: "Archer",
        step: "Adept",
        asks: "Win the longbow pastime",
        deed: "archer-longbow",
        need: 1,
        gear: Some("crescent-quiver"),
    },
    Mission {
        id: "archer-master",
        class: "Archer",
        step: "Master",
        asks: "Clear three rooms without being hit",
        deed: "floor-untouched",
        need: 3,
        gear: Some("rookery-longbow"),
    },
    // Shieldmaiden (Gatehouse): the gate.
    Mission {
        id: "shield-novice",
        class: "Shieldmaiden",
        step: "Novice",
        asks: "Fell 50 foes",
        deed: "shield-hold",
        need: 50,
        gear: None,
    },
    Mission {
        id: "shield-adept",
        class: "Shieldmaiden",
        step: "Adept",
        asks: "Clear 10 rooms",
        deed: "shield-clears",
        need: 10,
        gear: Some("gate-aegis"),
    },
    Mission {
        id: "shield-master",
        class: "Shieldmaiden",
        step: "Master",
        asks: "Clear 5 rooms without a scratch",
        deed: "shield-unmoved",
        need: 5,
        gear: Some("oathshield"),
    },
    // Gunner (Smithy): the powder.
    Mission {
        id: "gunner-novice",
        class: "Gunner",
        step: "Novice",
        asks: "Fell 20 foes with the handgonne",
        deed: "gunner-ball",
        need: 20,
        gear: None,
    },
    Mission {
        id: "gunner-adept",
        class: "Gunner",
        step: "Adept",
        asks: "Fell 30 foes with bombs",
        deed: "gunner-bombs",
        need: 30,
        gear: Some("forge-matchcord"),
    },
    Mission {
        id: "gunner-master",
        class: "Gunner",
        step: "Master",
        asks: "Fell a guardian with powder alone",
        deed: "gunner-guardian",
        need: 1,
        gear: Some("smithys-dragonmouth"),
    },
    // Swordsman (Round Table): the table.
    Mission {
        id: "sword-novice",
        class: "Swordsman",
        step: "Novice",
        asks: "Fell 50 foes with the blade",
        deed: "sword-duel",
        need: 50,
        gear: None,
    },
    Mission {
        id: "sword-adept",
        class: "Swordsman",
        step: "Adept",
        asks: "Temper a blade to its last temper",
        deed: "sword-temper",
        need: 1,
        gear: Some("table-gauntlet"),
    },
    Mission {
        id: "sword-master",
        class: "Swordsman",
        step: "Master",
        asks: "Fell 3 guardians",
        deed: "sword-guardian",
        need: 3,
        gear: Some("round-table-blade"),
    },
    // Berserker (Mines): the deeps.
    Mission {
        id: "berserk-novice",
        class: "Berserker",
        step: "Novice",
        asks: "Cut 30 foes with the spin",
        deed: "berserk-spin",
        need: 30,
        gear: None,
    },
    Mission {
        id: "berserk-adept",
        class: "Berserker",
        step: "Adept",
        asks: "Cut 4 foes with one spin",
        deed: "berserk-wide",
        need: 1,
        gear: Some("deeps-harness"),
    },
    Mission {
        id: "berserk-master",
        class: "Berserker",
        step: "Master",
        asks: "Fell 3 guardians",
        deed: "berserk-guardian",
        need: 3,
        gear: Some("grave-axes"),
    },
    // Mage (Labyrinth College): the orb.
    Mission {
        id: "mage-novice",
        class: "Mage",
        step: "Novice",
        asks: "Cast 40 spells",
        deed: "mage-cast",
        need: 40,
        gear: None,
    },
    Mission {
        id: "mage-adept",
        class: "Mage",
        step: "Adept",
        asks: "Fell 30 foes with the staff",
        deed: "mage-staff",
        need: 30,
        gear: Some("college-orb"),
    },
    Mission {
        id: "mage-master",
        class: "Mage",
        step: "Master",
        asks: "Fell 3 guardians",
        deed: "mage-guardian",
        need: 3,
        gear: Some("sigil-staff"),
    },
    // Healer (Chapel): the habit.
    Mission {
        id: "healer-novice",
        class: "Healer",
        step: "Novice",
        asks: "Mend friends for 200",
        deed: "healer-mend",
        need: 200,
        gear: None,
    },
    Mission {
        id: "healer-adept",
        class: "Healer",
        step: "Adept",
        asks: "Bring a fallen friend up the stairs 3 times",
        deed: "healer-carry",
        need: 3,
        gear: Some("chapel-cincture"),
    },
    Mission {
        id: "healer-master",
        class: "Healer",
        step: "Master",
        asks: "Finish 3 floors with nobody falling",
        deed: "healer-unfallen",
        need: 3,
        gear: Some("vestal-censer"),
    },
    // Bard (Scriptorium): the song. The Bard came after the seven banners;
    // its gear is its notes, so no card is handed over.
    Mission {
        id: "bard-novice",
        class: "Bard",
        step: "Novice",
        asks: "Fell 30 foes",
        deed: "bard-fell",
        need: 30,
        gear: None,
    },
    Mission {
        id: "bard-adept",
        class: "Bard",
        step: "Adept",
        asks: "Sing 20 foes to sleep",
        deed: "bard-lull",
        need: 20,
        gear: None,
    },
    Mission {
        id: "bard-master",
        class: "Bard",
        step: "Master",
        asks: "Fell 3 guardians",
        deed: "bard-guardian",
        need: 3,
        gear: None,
    },
    // Peasant (Homecoming): the long way home. No card either: a Peasant
    // throws what the road gives.
    Mission {
        id: "peasant-novice",
        class: "Peasant",
        step: "Novice",
        asks: "Fell 30 foes",
        deed: "peasant-fell",
        need: 30,
        gear: None,
    },
    Mission {
        id: "peasant-adept",
        class: "Peasant",
        step: "Adept",
        asks: "Desert 10 times",
        deed: "peasant-desert",
        need: 10,
        gear: None,
    },
    Mission {
        id: "peasant-master",
        class: "Peasant",
        step: "Master",
        asks: "Fell 3 guardians",
        deed: "peasant-guardian",
        need: 3,
        gear: None,
    },
];

/// The lock table: which rank each strong power needs. One place, as data,
/// so the operator can move things around.
///
/// Everything not listed is free to a Squire: every common card, and the
/// wishes named in the brief as Squire-tier (Triple Volley, Ricochet,
/// Seeker Fletching, Quick Hands, Heavy Heads, Fleet Foot, Stout Heart,
/// Iron Skin I, Bodkin Heads I, Twin Fletch, Fart Bombs).
pub struct Lock {
    pub power: &'static str,
    pub rank: Rank,
    /// The nearest mission that earns it, in one plain line.
    pub mission: &'static str,
}

pub static LOCKS: &[Lock] = &[
    // Adept: chain, mend, burst, orbit; the middle wish tiers.
    Lock { power: "wish:chain", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:mend", rank: Rank::Adept, mission: "the Healer's Adept mission" },
    Lock { power: "wish:powder", rank: Rank::Adept, mission: "the Gunner's Adept mission" },
    Lock { power: "wish:fork", rank: Rank::Adept, mission: "the Archer's Adept mission" },
    Lock { power: "wish:morningstar", rank: Rank::Adept, mission: "the Berserker's Adept mission" },
    // The top tiers of the free wishes stay Adept.
    Lock { power: "wish:volley:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:ricochet:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:seeker:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:quick:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:heavy:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:fleet:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:stout:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:iron:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:pierce:3", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "wish:twin:2", rank: Rank::Adept, mission: "any Adept mission" },
    // Master: lifesteal, the awakened fork, morningstar II.
    Lock { power: "wish:leech", rank: Rank::Master, mission: "any Master mission" },
    Lock { power: "wish:fork:2", rank: Rank::Master, mission: "the Archer's Master mission" },
    Lock { power: "wish:morningstar:3", rank: Rank::Master, mission: "the Berserker's Master mission" },
    // Rare cards with chain, mend, burst or orbit are Adept; every vamp
    // card is Master, and so are fork 2 and morningstar II as cards.
    Lock { power: "card:rare:chain", rank: Rank::Adept, mission: "any Adept mission" },
    Lock { power: "card:rare:mend", rank: Rank::Adept, mission: "the Healer's Adept mission" },
    Lock { power: "card:rare:burst", rank: Rank::Adept, mission: "the Gunner's Adept mission" },
    Lock { power: "card:rare:orbit", rank: Rank::Adept, mission: "the Berserker's Adept mission" },
    Lock { power: "card:vamp", rank: Rank::Master, mission: "any Master mission" },
    Lock { power: "card:fork:2", rank: Rank::Master, mission: "the Archer's Master mission" },
    Lock { power: "card:orbit:2", rank: Rank::Master, mission: "the Berserker's Master mission" },
];

impl Lock {
    /// The rank a wish tier needs, by wish id and tier index (1-based).
    /// Tiers not in the table are free; a wish-id row with no tier covers
    /// every tier of that wish.
    pub fn wish(id: &str, tier: usize) -> Option<(Rank, &'static str)> {
        LOCKS.iter()
            .find(|l| l.power == format!("wish:{id}:{tier}").as_str())
            .or_else(|| {
                LOCKS.iter().find(|l| l.power == format!("wish:{id}").as_str())
            })
            .map(|l| (l.rank, l.mission))
    }

    /// The rank a card needs. Signature gear never drops, so it is never
    /// met on the floor; every other card walks the table: vamp cards are
    /// Master, rares with chain, mend, burst or orbit are Adept, fork 2 and
    /// orbit 2 are Master, everything else is free.
    pub fn card(card: &cards::Card) -> Option<(Rank, &'static str)> {
        use cards::Effect;
        if gear_piece(&card.id).is_some() {
            return None;
        }
        if card.effects.iter().any(|e| matches!(e, Effect::Vamp(_))) {
            return lock("card:vamp");
        }
        if card.effects.iter().any(|e| matches!(e, Effect::Fork(2))) {
            return lock("card:fork:2");
        }
        if card.effects.iter().any(|e| matches!(e, Effect::Orbit(2))) {
            return lock("card:orbit:2");
        }
        if card.rarity == cards::Rarity::Rare {
            let key = card.effects.first().and_then(|e| match e {
                Effect::Chain(_) => Some("card:rare:chain"),
                Effect::Mend(_) => Some("card:rare:mend"),
                Effect::Burst(_) => Some("card:rare:burst"),
                Effect::Orbit(_) => Some("card:rare:orbit"),
                _ => None,
            });
            if let Some(key) = key {
                return lock(key);
            }
        }
        None
    }
}

fn lock(power: &str) -> Option<(Rank, &'static str)> {
    LOCKS
        .iter()
        .find(|l| l.power == power)
        .map(|l| (l.rank, l.mission))
}

/// One knight's standing with the school, kept by name in the realm save.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    #[serde(default)]
    pub rank: Rank,
    /// Each mission's count, by mission id.
    #[serde(default)]
    pub deeds: BTreeMap<String, u32>,
    /// Missions finished; each stays finished.
    #[serde(default)]
    pub done: BTreeSet<String>,
    /// Signature gear handed over, by card id (each once).
    #[serde(default)]
    pub gear: BTreeSet<String>,
}

impl Standing {
    pub fn valid(&self) -> bool {
        self.deeds.len() <= MISSIONS.len()
            && self.done.len() <= MISSIONS.len()
            && self.done.iter().all(|id| mission(id).is_some())
            && self.deeds.keys().all(|id| mission(id).is_some())
            && self.deeds.values().all(|&n| n > 0 && n < 100_000)
    }

    pub fn mission_done(&self, id: &str) -> bool {
        self.done.contains(id)
    }

    /// A mission's count, capped at its need.
    pub fn count(&self, m: &Mission) -> u32 {
        self.deeds.get(m.id).copied().unwrap_or(0).min(m.need)
    }

    /// The rank this standing has earned in `class`: the Novice mission
    /// makes an Adept, all three finished make a Master.
    pub fn earned(&self, class: &str) -> Rank {
        let finished = MISSIONS
            .iter()
            .filter(|m| m.class == class && self.mission_done(m.id))
            .count();
        if finished >= 3 {
            Rank::Master
        } else if finished >= 1 {
            Rank::Adept
        } else {
            Rank::Squire
        }
    }
}

/// The school as the realm keeps it: each knight's standing, by name, so it
/// lasts between sessions. Old saves load as the default: every Squire.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct School {
    #[serde(default)]
    pub knights: BTreeMap<String, Standing>,
}

impl School {
    pub fn valid(&self) -> bool {
        self.knights.values().all(Standing::valid)
    }

    pub fn standing(&self, who: &str) -> Standing {
        self.knights.get(who).cloned().unwrap_or_default()
    }
}

/// The run's own school state: whose board is open at which master's
/// banner, and who has fallen on this floor (for the nobody-fell missions).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The reading knight, and the master's class index.
    #[serde(default)]
    pub owner: Option<u32>,
    #[serde(default)]
    pub class: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallen: Vec<u32>,
    /// Deeds counted this tick, waiting to be folded into the realm's
    /// school records (`flush`): the combat loops already hold the knights,
    /// so the queue takes the note and the flush pays it in.
    #[serde(skip)]
    pub pending: Vec<(u32, &'static str, u32)>,
}

impl Board {
    /// Queue a deed from inside a loop that holds the knights or the foes:
    /// a field-local borrow, so the combat code can note without letting go.
    pub fn note(&mut self, id: u32, deed: &'static str, n: u32) {
        if n > 0 && self.pending.len() < 64 {
            self.pending.push((id, deed, n));
        }
    }

    pub fn close(&mut self) {
        self.owner = None;
    }
    pub fn valid(&self) -> bool {
        self.pending.len() <= 64
            && self.owner.is_none_or(|id| (1..=MAX_PLAYERS).contains(&id))
            && usize::from(self.class) < classes::CLASSES.len()
            && self.fallen.iter().all(|&id| (1..=MAX_PLAYERS).contains(&id))
            && self.fallen.len() <= MAX_PLAYERS as usize
    }
}

impl Home {
    /// The name the realm keeps a knight's school standing under: the same
    /// name Sir Ector's talents use, so one knight is one knight.
    pub fn school_key(hero: &Hero) -> String {
        super::talents::knight_key(hero)
    }

    pub fn standing(&self, hero: &Hero) -> Standing {
        self.school.standing(&Self::school_key(hero))
    }
}

impl Home {
    /// The sheet's questline note for a power: its lock (the teacher and
    /// where they live) or the knight's step on the line.
    pub fn quest_sheet(
        &self,
        hero: &Hero,
        power: &str,
    ) -> Option<String> {
        let book = super::questline::book();
        let q = book.get(power)?;
        let walk = self.questlines.walk(&Self::school_key(hero), power);
        if walk.finished() {
            return Some(format!("done — {}", q.title));
        }
        match walk.at(q) {
            None => Some(format!(
                "the trial: {} ({}/{})",
                q.trial.name,
                walk.count(&q.trial.counter).min(q.trial.need),
                q.trial.need
            )),
            Some((n, step)) => Some(format!(
                "step {n}/{} — {} ({}/{}) at {}",
                q.steps.len(),
                step.name,
                walk.count(&step.counter).min(step.need),
                step.need,
                q.place
            )),
        }
    }
}

/// Whether `card` is above `hero`'s school rank: the floor keeps it, for
/// someone who can. The rank is the realm's, by knight name.
pub fn locked(school: &School, hero: &Hero, card: &cards::Card) -> bool {
    Lock::card(card).is_some_and(|(rank, _)| school.standing(&Home::school_key(hero)).rank < rank)
}

fn mission(id: &str) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.id == id)
}

/// The mission that hands over a signature card, if `id` is one.
pub fn gear_piece(id: &str) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.gear == Some(id))
}

/// A master's board rows: each mission in plain words with its progress,
/// and the gear it earns.
pub fn board_rows(school: &School, hero: &Hero, class: &Class) -> Vec<String> {
    let standing = school.standing(&Home::school_key(hero));
    let mut rows = Vec::new();
    for m in MISSIONS.iter().filter(|m| m.class == class.name) {
        let have = standing.count(m);
        let state = if standing.mission_done(m.id) { "done" } else { "open" };
        let gear = match m.gear {
            Some(id) => format!("  earns {id}"),
            None => String::new(),
        };
        rows.push(format!(
            "{:<7} {:<38} {:<5} {}/{}{gear}",
            m.step, m.asks, state, have, m.need
        ));
    }
    rows
}

/// The sheet's rank line for a knight.
pub fn sheet_rank(school: &School, hero: &Hero) -> String {
    let standing = school.standing(&Home::school_key(hero));
    let class = classes::of(hero).map(|c| c.name).unwrap_or("knight");
    let rank = standing.rank.max(standing.earned(class));
    match rank.next_step() {
        Some(step) => format!("Rank {rank} of the {class} — next: {step}"),
        None => format!("Rank {rank} of the {class} — the school honours you"),
    }
}

/// The sheet's lock note for a wish's next tier, if it is locked here.
pub fn wish_locked(school: &School, hero: &Hero, wish_id: &str, tier: usize) -> Option<String> {
    let standing = school.standing(&Home::school_key(hero));
    let (rank, mission) = Lock::wish(wish_id, tier)?;
    (standing.rank < rank).then(|| format!("needs {rank} ({mission})"))
}

/// The sheet's per-class line: the class's next open mission, or its rank.
pub fn next_mission(school: &School, hero: &Hero, class: &Class) -> String {
    let standing = school.standing(&Home::school_key(hero));
    let mine = class.name == classes::of(hero).map(|c| c.name).unwrap_or("");
    let next = MISSIONS
        .iter()
        .filter(|m| m.class == class.name && !standing.mission_done(m.id))
        .next()
        .map(|m| {
            format!(
                "{} {} ({}/{})",
                m.step,
                m.asks,
                standing.count(m),
                m.need
            )
        })
        .unwrap_or_else(|| "all missions done".into());
    let mark = if mine { "*" } else { " " };
    format!("{mark}{}: {}", class.name, next)
}

impl Run {
    /// The run counted a deed, where nothing else is borrowed: it queues,
    /// and `flush_school` pays it in at the tick's safe point.
    pub(super) fn school_note(&mut self, id: u32, deed: &'static str, n: u32) {
        self.school.note(id, deed, n);
    }

    /// Fold the queued deeds into the realm's school records. Called once a
    /// tick, after the combat loops let the knights go.
    pub fn flush_school(&mut self) {
        let pending = std::mem::take(&mut self.school.pending);
        for (id, deed, n) in pending {
            self.school_fold(id, deed, n);
        }
    }

    fn school_fold(&mut self, id: u32, deed: &str, n: u32) {
        if n == 0 {
            return;
        }
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let Some(class) = classes::of(hero) else {
            return;
        };
        let Some(m) = MISSIONS
            .iter()
            .find(|m| m.class == class.name && m.deed == deed)
        else {
            return;
        };
        let key = Home::school_key(hero);
        let standing = self.home.school.knights.entry(key).or_default();
        if standing.done.contains(m.id) {
            return;
        }
        let have = standing.deeds.entry(m.id.to_string()).or_insert(0);
        *have = (*have + n).min(m.need);
        if *have >= m.need {
            standing.done.insert(m.id.to_string());
            let rank = standing.earned(class.name);
            if rank > standing.rank {
                standing.rank = rank;
            }
            self.cues.push("school_mission".into());
            self.sounds.push("wheel_land");
            let name = hero.name.to_uppercase();
            self.found = Some((
                self.tick,
                id,
                format!("{name}: {step} MISSION DONE — {asks}", step = m.step, asks = m.asks),
            ));
        }
    }

    /// A knight fell this floor: out of the nobody-fell missions.
    pub(super) fn school_fell(&mut self, id: u32) {
        if !self.school.fallen.contains(&id) {
            self.school.fallen.push(id);
        }
    }

    /// The stairs down: a floor finished. Whoever never fell this floor
    /// stands a step nearer the school's highest honours.
    pub(super) fn school_stairs(&mut self) {
        let clean = self
            .players
            .keys()
            .copied()
            .filter(|id| !self.school.fallen.contains(id))
            .collect::<Vec<_>>();
        for id in clean {
            self.school_note(id, "healer-unfallen", 1);
        }
        self.school.fallen.clear();
    }

    /// E at a class's banner stone: the master's board opens, and a fresh
    /// press at a board's stone hands over any gear the standing has earned
    /// but not yet taken. The class change itself stays the stone's own
    /// first press (`tick_class_stones`).
    pub(super) fn tick_school(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Kingdom {
            self.school.close();
            return;
        }
        // An open board closes when its knight walks away or leaves.
        if let Some(owner) = self.school.owner
            && !inputs
                .get(&owner)
                .copied()
                .is_some_and(|i| i.valid() && i.interact)
        {
            self.school.close();
        }
        let geo = map::geography(&self.kingdom.wishes);
        let stones = classes::stones(&geo);
        for (&id, hero) in &self.players {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down || hero.hp == 0 || hero.stone {
                continue;
            }
            let Some((i, _)) = stones.iter().find(|&&(_, (sx, sy))| {
                (hero.x - (sx as f32 + 0.5) * TILE_UNITS)
                    .hypot(hero.y - (sy as f32 + 0.5) * TILE_UNITS)
                    < classes::REACH * TILE_UNITS
            }) else {
                continue;
            };
            // The stone's first press changes class; a second press, after
            // the change, opens the master's board.
            if self.kingdom.class_held.contains(&id) {
                self.school.owner = Some(id);
                self.school.class = *i as u8;
            }
        }
        // Hand over earned gear while the board is open: one piece a press,
        // by the reading knight.
        if let Some(owner) = self.school.owner {
            self.school_grant(owner);
        }
    }

    /// The master hands over signature gear the knight has earned and not
    /// yet taken: the card joins the book and the deck, held at once.
    pub fn school_grant(&mut self, owner: u32) {
        let Some(hero) = self.players.get(&owner) else {
            return;
        };
        let key = Home::school_key(hero);
        let standing = self.home.school.standing(&key);
        let class = classes::of(hero);
        let earned: Vec<&'static str> = MISSIONS
            .iter()
            .filter(|m| {
                Some(m.class) == class.map(|c| c.name)
                    && standing.mission_done(m.id)
                    && m.gear.is_some()
                    && !standing.gear.contains(m.gear.unwrap())
            })
            .filter_map(|m| m.gear)
            .collect();
        let Some(id) = earned.first().copied() else {
            return;
        };
        let Some(card) = gear_cards().find(|c| c.id == id) else {
            return;
        };
        self.book.insert(card.clone());
        let Some(hero) = self.players.get_mut(&owner) else {
            return;
        };
        hero.deck.push(id.to_string());
        hero.rebonus(&self.book);
        let standing = self.home.school.knights.entry(key).or_default();
        standing.gear.insert(id.to_string());
        let name = hero.name.to_uppercase();
        let card_name = card.name.clone();
        self.found = Some((
            self.tick,
            owner,
            format!("{name} TAKES UP THE {card_name}"),
        ));
        self.sounds.push("play_card");
    }
}

/// The signature gear as checked cards, in the class's look, drawn in realm
/// inks. Built here, checked by the card checker like any card, drop and
/// chest zero: gear that can only be earned, never found.
pub fn gear_cards() -> impl Iterator<Item = cards::Card> {
    GEAR.iter().map(|(id, raw)| {
        cards::check(id, raw)
            .expect("signature gear passes its own checker")
            .card
    })
}

pub static GEAR: &[(&str, &str)] = &[
    // Archer — the Rookery.
    (
        "crescent-quiver",
        r#"name   Crescent Quiver
kind   hold
rarity relic
text   The Rookery's Adept gift: a quiver cut crescent, so every arrow flies a little wider.
drop   0
chest  0
volley 6
art
W8.8W.8.8W
8.lW8.Wl8
8.l8.8l8
..W...W..
"#,
    ),
    (
        "rookery-longbow",
        r#"name   Rookery Longbow
kind   arm
rarity relic
text   The Rookery's Master gift: a yew stave sworn to the nest.
drop   0
chest  0
bolt   damage=12 speed=14 range=32
art
....l7....
...l7W....
..l7......
.l7.......
l7........
"#,
    ),
    // Shieldmaiden — the Gatehouse.
    (
        "gate-aegis",
        r#"name   Gate Aegis
kind   guard
rarity relic
text   The Gatehouse's Adept gift: the gate's own shape, hung on the arm.
drop   0
chest  0
guard  shield
art
.VVVVVV.
VUtOOtUV
VUt77tUV
.UUtOOu.
..uUUu..
"#,
    ),
    (
        "oathshield",
        r#"name   Oath of the Gate
kind   hold
rarity relic
text   The Gatehouse's Master gift: the oath itself, worn under the mail.
drop   0
chest  0
armor  2
max_hp 15
art
uVVVVVVVu
VUtOOOOtUV
VUtO77OtUV
UUOO77OOU
uUUtOOOUux
..uUUUUu..
"#,
    ),
    // Gunner — the Smithy.
    (
        "forge-matchcord",
        r#"name   Forge Matchcord
kind   hold
rarity relic
text   The Smithy's Adept gift: a slow cord that never misses its spark.
drop   0
chest  0
damage 10
burst  10
art
8....7
87..7.
.877..
..8...
"#,
    ),
    (
        "smithys-dragonmouth",
        r#"name   Smithy's Dragonmouth
kind   arm
rarity relic
text   The Smithy's Master gift: a bell-mouth that roars.
drop   0
chest  0
bolt   damage=9 speed=10 range=26
spread shots=4 arc=40
art
..W77W..
.W7ww7W.
W7w88w7W
.W7ww7W.
..W77W..
"#,
    ),
    // Swordsman — the Round Table.
    (
        "table-gauntlet",
        r#"name   Table Gauntlet
kind   hold
rarity relic
text   The Round Table's Adept gift: a steel grip for a steel arm.
drop   0
chest  0
rate   10
armor  1
art
7JJJ7
JiiiJ
Ji.iJ
JiiiJ
7JJJ7
"#,
    ),
    (
        "round-table-blade",
        r#"name   Blade of the Table
kind   arm
rarity relic
text   The Round Table's Master gift: the seat itself, edged.
drop   0
chest  0
melee  damage=16 reach=2.6 arc=140
art
.....7
....7W
...7W.
..7W..
.7W...
7W....
"#,
    ),
    // Berserker — the Mines.
    (
        "deeps-harness",
        r#"name   Deeps Harness
kind   hold
rarity relic
text   The Mines' Adept gift: dwarf iron across the shoulders.
drop   0
chest  0
armor  1
max_hp 10
art
R7..7R
7JJJJ7
.JiiJ.
"#,
    ),
    (
        "grave-axes",
        r#"name   Grave Axes
kind   arm
rarity relic
text   The Mines' Master gift: two axes, older than the grave they came from.
drop   0
chest  0
melee  damage=18 reach=2.2 arc=360
art
7....7
7W..W7
.7WW7.
..77..
"#,
    ),
    // Mage — the Labyrinth College.
    (
        "college-orb",
        r#"name   College Orb
kind   hold
rarity relic
text   The College's Adept gift: a pale orb that remembers its lessons.
drop   0
chest  0
chain  1
art
..zzz..
.zZZZz.
zZWWWZz
.zZZZz.
..zzz..
"#,
    ),
    (
        "sigil-staff",
        r#"name   Sigil Staff
kind   arm
rarity relic
text   The College's Master gift: a staff cut with the tower's sigils.
drop   0
chest  0
look   void shard
bolt   damage=10 speed=12 range=30
art
..zWz..
...z...
...z...
...z...
...z...
"#,
    ),
    // Healer — the Chapel.
    (
        "chapel-cincture",
        r#"name   Chapel Cincture
kind   hold
rarity relic
text   The Chapel's Adept gift: a rope belt that binds the vow.
drop   0
chest  0
mend   1
art
..8888..
.8....8.
.8.88.8.
.8....8.
..8888..
"#,
    ),
    (
        "vestal-censer",
        r#"name   Vestal Censer
kind   hold
rarity relic
text   The Chapel's Master gift: the chapel's own censer, still smoking.
drop   0
chest  0
mend   2
art
...88...
..8..8..
.8.77.8.
..8778..
...88...
"#,
    ),
];

#[cfg(test)]
#[path = "../../tests/rules/together_school__tests.rs"]
mod tests;
