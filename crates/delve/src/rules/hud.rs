//! What a knight's HUD shows, as data: health and the stage of dress it has
//! cost, the weapon and guard in hand, spells and their recharge, the hand
//! of play cards, bombs, the ultimate's charge, a rune, and the weapon lying
//! beside them that the take key would swap to.
//!
//! One reading serves every screen: the host's terminal and a friend's
//! joined angelX draw it as cells, a browser guest's page as markup from the
//! same fields, so a knight sees the same things wherever they sit.

use super::*;
use serde::{Deserialize, Serialize};

/// One thing a knight holds: its key, its name, and how ready it is, from
/// nothing (0) to ready (1). `state` is the word that goes with it
/// ("ready", "3s", "62%"), left empty when there is nothing to say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub key: String,
    pub name: String,
    pub fill: f32,
    pub state: String,
}

impl Slot {
    pub(super) fn new(
        key: &str,
        name: impl Into<String>,
        fill: f32,
        state: impl Into<String>,
    ) -> Slot {
        Slot {
            key: key.into(),
            name: name.into(),
            fill: fill.clamp(0.0, 1.0),
            state: state.into(),
        }
    }
}

/// The keys a seat plays with: the labels on the HUD, the tray and the
/// footer, and the help, so a remap lives here and nowhere else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keys {
    /// The keys that walk the knight, as one word.
    pub walk: &'static str,
    pub fire: &'static str,
    /// Plates, levers, cages, the folk and their booths.
    pub interact: &'static str,
    pub guard: &'static str,
    pub sword: &'static str,
    /// Drinks the first card in hand; the rest move up.
    pub quaff: &'static str,
    pub spells: [&'static str; cards::SPELL_SLOTS],
    pub ult: &'static str,
    pub take: &'static str,
    pub bomb: &'static str,
    /// Fart Cloud, Fire Pit, Mines, Barrier, Shield Throw.
    pub skills: [&'static str; skills::SKILLS],
}

impl Keys {
    /// The first knight on a keyboard, a friend's angelX, a browser guest.
    pub const FIRST: Keys = Keys {
        walk: "WASD",
        fire: "F",
        interact: "E",
        guard: "Space",
        sword: "Q",
        quaff: "Z",
        spells: ["1", "2", "3", "4"],
        ult: "R",
        take: "E",
        bomb: "X",
        skills: ["M", ",", ".", ";", "'"],
    };
    /// A second knight sharing the keyboard: H takes and meets the world.
    pub const SECOND: Keys = Keys {
        walk: "IJKL",
        fire: "Enter",
        interact: "H",
        guard: "O",
        sword: "[",
        quaff: "]",
        spells: ["5", "6", "7", "\\"],
        ult: "Y",
        take: "H",
        bomb: "U",
        skills: ["8", "9", "0", "-", "="],
    };

    /// The keys a seat plays with: the second only for a second knight on
    /// this keyboard, never for a friend playing from afar.
    pub fn of(id: u32, couch: bool) -> &'static Keys {
        if couch && id == 2 {
            &Keys::SECOND
        } else {
            &Keys::FIRST
        }
    }

    /// What this seat moves and fights with, two spaces between keys: the
    /// first line of the footer and the help.
    pub fn fight_line(&self, side_on: bool) -> String {
        let walk = if side_on {
            "A/D run  W jump  S drop".to_string()
        } else {
            format!("{} walk", self.walk)
        };
        // The first knight aims with the arrows; a second fires where they walk.
        let aim = if self.walk == Keys::FIRST.walk {
            "arrows aim/fire  "
        } else {
            ""
        };
        format!(
            "{walk}  {aim}{} fire  {} interact  {} guard",
            self.fire, self.interact, self.guard
        )
    }

    pub fn kit_line(&self) -> String {
        format!(
            "{} sword  {} bomb  {} take  {} spells  {} quaff  {} ultimate",
            self.sword,
            self.bomb,
            self.take,
            self.spell_keys(),
            self.quaff,
            self.ult
        )
    }

    pub fn skills_line(&self) -> String {
        self.skills
            .iter()
            .zip(skills::NAMES)
            .map(|(key, name)| format!("{key} {name}"))
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// The spell keys as one word: "1-4", or "5 6 7 \\".
    pub fn spell_keys(&self) -> String {
        if self.spells == Keys::FIRST.spells {
            "1-4".into()
        } else {
            self.spells.join(" ")
        }
    }
}

/// A pit stop at the privy, as the HUD shows it: the stage, the straps, the
/// marker on its bar and the green it must meet, and the clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PitHud {
    pub stage: String,
    /// Straps done, and straps in the stage.
    pub straps: (u8, u8),
    /// The whole stop's clock and this stage's, in seconds.
    pub total: String,
    pub stage_time: String,
    /// The marker on its bar, 0 to 1; none outside a strap stage.
    pub marker: Option<f32>,
    /// The bar's good and clean zones, as half-widths about the middle.
    pub zone: (f32, f32),
    /// What to press.
    pub hint: String,
    /// The party's best times, fastest first: name and seconds.
    pub board: Vec<(String, String)>,
}

/// The room's wish allowance, read from the existing window and knight.
/// Wishes do not accumulate between rooms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WishHud {
    /// Wishes held: plain ones, and super ones.
    Ready { wishes: u8, great: u8 },
    Spent,
    Closed,
    Fallen,
}

impl WishHud {
    pub fn remaining(self) -> u8 {
        match self {
            Self::Ready { wishes, great } => wishes + great,
            _ => 0,
        }
    }

    /// A super wish is held.
    pub fn great(self) -> bool {
        matches!(self, Self::Ready { great, .. } if great > 0)
    }

    pub fn state(self) -> &'static str {
        match self {
            Self::Ready { great, .. } if great > 0 => "T super wish",
            Self::Ready { .. } => "T wish",
            Self::Spent => "spent",
            Self::Closed => "next floor",
            Self::Fallen => "fallen",
        }
    }

    /// Knight `hero`'s wishes as the HUD shows them.
    pub fn of(hero: &Hero) -> WishHud {
        if hero.hp == 0 {
            Self::Fallen
        } else if hero.wishes + hero.great_wishes > 0 {
            Self::Ready { wishes: hero.wishes, great: hero.great_wishes }
        } else {
            Self::Closed
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hud {
    pub name: String,
    pub hp: u32,
    pub max_hp: u32,
    /// 0 whole, 1 bare-headed, 2 in smallclothes: the colour of the bar.
    pub gear: u8,
    pub mail: u32,
    pub weapon: Slot,
    pub guard: Slot,
    pub spells: Vec<Option<Slot>>,
    /// The keys of the spell slots, for the empty ones.
    pub spell_keys: Vec<String>,
    /// Play cards by slot; empty slots are `None`.
    pub hand: Vec<Option<Slot>>,
    pub bombs: u32,
    pub max_bombs: u32,
    pub ult: Slot,
    pub rune: Option<(String, u32)>,
    pub carrying: String,
    /// The weapon beside the knight that the take key swaps to, by name.
    pub take: Option<(String, String)>,
    /// A word for a state that overrides the rest: fallen, stone, a vigil.
    pub status: String,
    /// The pit stop under way at the privy, if one is.
    #[serde(default)]
    pub pit: Option<PitHud>,
    /// Lines for what rides with the knight: a frog on the shoulder, a
    /// frog's gift for the floor, the relief the privy left.
    #[serde(default)]
    pub perks: Vec<String>,
    /// The skill bar: gas, fire, mines, barricade, shield.
    #[serde(default)]
    pub skills: Vec<Slot>,
    /// Whole pips of valour (`feel::PIPS` at most): a skill spends one.
    #[serde(default)]
    pub valour: u32,
    /// Times the arm in hand has been tempered.
    #[serde(default)]
    pub temper: u8,
    /// This knight carries the floor's Boss Key.
    #[serde(default)]
    pub key: bool,
    /// Derived locally; older HUD snapshots may omit it.
    #[serde(default)]
    pub wishes: Option<WishHud>,
}

impl Run {
    pub fn wish_hud(&self, id: u32) -> Option<WishHud> {
        self.players.get(&id).map(WishHud::of)
    }

    /// The take key's weapon for knight `id`: the nearest lying within
    /// reach, as `(key, name)`.
    pub fn weapon_beside(&self, id: u32) -> Option<&Card> {
        let hero = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
        self.room()
            .items
            .iter()
            .filter(|item| (hero.x - item.x).hypot(hero.y - item.y) < TAKE_REACH)
            .filter_map(|item| {
                let card = self.book.get(&item.card)?;
                (gear::manual(card) && hero.takes(card))
                    .then_some((card, (hero.x - item.x).hypot(hero.y - item.y)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(card, _)| card)
    }

    /// Knight `id`'s HUD, with the labels of the keys they play.
    pub fn hud(&self, id: u32, keys: &Keys) -> Option<Hud> {
        let hero = self.players.get(&id)?;
        let card_name =
            |id: Option<&str>| id.and_then(|id| self.book.get(id)).map(|c| c.name.clone());
        let weapon = hero
            .forged
            .as_ref()
            .map(|w| w.name.clone())
            .or_else(|| card_name(hero.arm.as_deref()))
            .unwrap_or_else(|| hero.weapon.name().to_string());
        let weapon = match hero.possessed {
            // A borrowed body's arms, not the knight's (`bodies`).
            Some(kind) => bodies::arms_name(kind, self.dungeon.pack),
            None => format!("{weapon}{}", tier(hero.temper)),
        };
        let guard_name = card_name(hero.guard_card.as_deref()).unwrap_or_else(|| "Roll".into());
        let guard = match hero.guard {
            cards::Guard::Roll => {
                let left = hero.dash_cooldown;
                let state = if left == 0 {
                    "ready".to_string()
                } else {
                    format!("{}s", left.div_ceil(HZ))
                };
                Slot::new(
                    keys.guard,
                    guard_name,
                    1.0 - left as f32 / ROLL_REARM as f32,
                    state,
                )
            }
            cards::Guard::Spin => {
                let left = hero.dash_cooldown;
                let state = if hero.spin > 0 {
                    "spinning".to_string()
                } else if left == 0 {
                    "ready".to_string()
                } else {
                    format!("{}s", left.div_ceil(HZ))
                };
                let fill = if hero.spin > 0 {
                    1.0
                } else {
                    1.0 - left as f32 / spin::REARM as f32
                };
                Slot::new(keys.guard, guard_name, fill, state)
            }
            cards::Guard::Shield => Slot::new(
                keys.guard,
                guard_name,
                1.0,
                if hero.shielding { "up" } else { "hold" },
            ),
            cards::Guard::Wall(_) => Slot::new(
                keys.guard,
                guard_name,
                hero.mana / MAX_MANA,
                if hero.wall_spent {
                    "spent".to_string()
                } else {
                    format!("{:.0}", hero.mana)
                },
            ),
        };
        let spells = if let Some(kind) = hero.possessed {
            // A borrowed body's one spell, on the first key; the knight's
            // cards wait.
            let spell = bodies::spell(kind).map(|spell| {
                let left = hero.body_rearm;
                let state = if left == 0 {
                    "ready".to_string()
                } else {
                    format!("{}s", left.div_ceil(HZ))
                };
                Slot::new(
                    keys.spells[0],
                    spell.name,
                    1.0 - left as f32 / bodies::SPELL_REARM as f32,
                    state,
                )
            });
            (0..cards::SPELL_SLOTS).map(|slot| if slot == 0 { spell.clone() } else { None }).collect()
        } else {
            hero
            .spells
            .iter()
            .zip(hero.spell_cooldowns)
            .enumerate()
            .map(|(slot, (id, left))| {
                let card = self.book.get(id.as_deref()?)?;
                let total = (card.cooldown.max(1) * HZ) as f32;
                let state = if left == 0 {
                    "ready".to_string()
                } else {
                    format!("{}s", left.div_ceil(HZ))
                };
                Some(Slot::new(
                    keys.spells[slot],
                    &card.name,
                    1.0 - left as f32 / total,
                    state,
                ))
            })
            .collect()
        };
        // The quaff key drinks the first card; the rest wait their turn.
        let hand = (0..cards::HAND)
            .map(|slot| {
                let card = self.book.get(hero.hand.get(slot)?)?;
                let key = if slot == 0 { keys.quaff } else { "" };
                Some(Slot::new(key, &card.name, 1.0, ""))
            })
            .collect();
        let ult = hero.ult();
        let ready = hero.ult_charge >= ults::ULT_FULL;
        let friend = |id: u32| {
            self.players
                .get(&id)
                .map(|h| h.shown_name())
                .unwrap_or_default()
        };
        let status = if hero.hp == 0 && !hero.absent {
            match self.skull_carrier(id) {
                Some(carrier) => format!("FALLEN: {} carries your skull", friend(carrier)),
                None => "FALLEN: a friend can carry your skull".to_string(),
            }
        } else if let Some(skull) = hero.carrying_skull {
            format!("SKULL: {} on a weak foe raises {}", keys.interact, friend(skull))
        } else if let Some(kind) = hero.possessed {
            format!("RAISED as a {}: until the Sanctuary", format!("{kind:?}").to_lowercase())
        } else if hero.vigil {
            "VIGIL: mending those near".to_string()
        } else if hero.stone {
            "STONE: the host is away".to_string()
        } else {
            String::new()
        };
        Some(Hud {
            name: hero.shown_name(),
            hp: hero.hp,
            max_hp: hero.max_hp,
            gear: hero.gear,
            mail: hero.armor,
            weapon: Slot::new(keys.fire, weapon, 1.0, ""),
            guard,
            spells,
            spell_keys: keys.spells.iter().map(|k| k.to_string()).collect(),
            hand,
            bombs: hero.bombs,
            max_bombs: MAX_BOMBS,
            ult: Slot::new(
                keys.ult,
                ult.name(),
                hero.ult_charge as f32 / ults::ULT_FULL as f32,
                if ready {
                    "ready".to_string()
                } else {
                    format!("{}%", hero.ult_charge * 100 / ults::ULT_FULL)
                },
            ),
            rune: hero
                .rune
                .map(|held| (held.kind.name().to_string(), held.left.div_ceil(HZ))),
            carrying: hero.carried.label(),
            take: self
                .weapon_beside(id)
                .map(|card| (keys.take.to_string(), card.name.clone())),
            status,
            pit: self.pit_hud(id, keys),
            perks: hero_perks(hero),
            skills: self.skill_slots(id, &keys.skills),
            valour: self.pips(id),
            temper: hero.temper,
            key: self.key_bearer() == Some(id),
            wishes: self.wish_hud(id),
        })
    }
}

/// A tempered arm's mark after its name: " II" to " IV".
pub fn tier(temper: u8) -> &'static str {
    match temper {
        0 => "",
        1 => " II",
        2 => " III",
        _ => " IV",
    }
}

/// What rides with a knight, as the HUD says it.
fn hero_perks(hero: &Hero) -> Vec<String> {
    let mut out = Vec::new();
    if hero.rider {
        out.push("a frog on your shoulder: it rides to the next Sanctuary".to_string());
    }
    if let Some(gift) = hero.boon.from {
        out.push(gift.perk().to_string());
    }
    if hero.pit.speed > 0 {
        out.push(format!(
            "relieved: faster for {}s",
            hero.pit.speed.div_ceil(HZ)
        ));
    }
    if hero.pit.damage > 0 {
        out.push(format!(
            "a quick stop: sharper for {}s",
            hero.pit.damage.div_ceil(HZ)
        ));
    }
    out
}
