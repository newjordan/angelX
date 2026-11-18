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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keys {
    pub walk: &'static str,
    pub fire: &'static str,
    pub interact: &'static str,
    pub guard: &'static str,
    pub sword: &'static str,
    pub quaff: &'static str,
    pub cycle: &'static str,
    pub spells: [&'static str; cards::SPELL_SLOTS],
    pub ult: &'static str,
    pub take: &'static str,
    pub bomb: &'static str,
    pub skills: [&'static str; skills::SKILLS],
}

impl Keys {
    pub const FIRST: Keys = Keys {
        walk: "WASD",
        fire: "F",
        interact: "E",
        guard: "Space",
        sword: "Q",
        quaff: "Z",
        cycle: "P",
        spells: ["1", "2", "3", "4"],
        ult: "R",
        take: "E",
        bomb: "X",
        skills: ["M", ",", ".", ";", "'"],
    };
    pub const SECOND: Keys = Keys {
        walk: "IJKL",
        fire: "Enter",
        interact: "H",
        guard: "O",
        sword: "[",
        quaff: "]",
        cycle: "End",
        spells: ["5", "6", "7", "\\"],
        ult: "Y",
        take: "H",
        bomb: "U",
        skills: ["8", "9", "0", "-", "="],
    };

    pub fn of(id: u32, couch: bool) -> &'static Keys {
        if couch && id == 2 {
            &Keys::SECOND
        } else {
            &Keys::FIRST
        }
    }

    pub fn fight_line(&self, side_on: bool) -> String {
        let walk = if side_on {
            "A/D run  W jump  S drop".to_string()
        } else {
            format!("{} walk", self.walk)
        };
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
            "{} sword  {} bomb  {} take  {} spells  {} quaff  {} next card  {} ultimate",
            self.sword,
            self.bomb,
            self.take,
            self.spell_keys(),
            self.quaff,
            self.cycle,
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

    pub fn spell_keys(&self) -> String {
        if self.spells == Keys::FIRST.spells {
            "1-4".into()
        } else {
            self.spells.join(" ")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PitHud {
    pub stage: String,
    pub straps: (u8, u8),
    pub total: String,
    pub stage_time: String,
    pub marker: Option<f32>,
    pub zone: (f32, f32),
    pub hint: String,
    pub board: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WishHud {
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
    pub gear: u8,
    pub mail: u32,
    pub weapon: Slot,
    pub guard: Slot,
    pub spells: Vec<Option<Slot>>,
    pub spell_keys: Vec<String>,
    pub hand: Vec<Option<Slot>>,
    pub bombs: u32,
    pub max_bombs: u32,
    pub ult: Slot,
    pub rune: Option<(String, u32)>,
    pub carrying: String,
    pub take: Option<(String, String)>,
    pub status: String,
    #[serde(default)]
    pub pit: Option<PitHud>,
    #[serde(default)]
    pub perks: Vec<String>,
    #[serde(default)]
    pub skills: Vec<Slot>,
    #[serde(default)]
    pub valour: u32,
    #[serde(default)]
    pub temper: u8,
    #[serde(default)]
    pub key: bool,
    #[serde(default)]
    pub wishes: Option<WishHud>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corpses: Option<Slot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton_wall: Option<Slot>,
}

impl Run {
    pub fn wish_hud(&self, id: u32) -> Option<WishHud> {
        self.players.get(&id).map(WishHud::of)
    }

    pub fn weapon_beside(&self, id: u32) -> Option<&Card> {
        let hero = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
        self.room()
            .items
            .iter()
            .filter(|item| (hero.x - item.x).hypot(hero.y - item.y) < TAKE_REACH)
            .filter_map(|item| {
                let card = self.book.get(&item.card)?;
                (gear::manual_for(hero, card) && hero.takes(card))
                    .then_some((card, (hero.x - item.x).hypot(hero.y - item.y)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(card, _)| card)
    }

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
            cards::Guard::Shield => {
                let wear = hero.shield_wear;
                let left = wear.left(hero.max_hp);
                let (fill, state) = if wear.down > 0 {
                    (wear.back(), "broken".to_string())
                } else if wear.worn > 0 {
                    (left, format!("{:.0}%", left * 100.0))
                } else if hero.shielding {
                    (1.0, "up".to_string())
                } else {
                    (1.0, "hold".to_string())
                };
                Slot::new(keys.guard, guard_name, fill, state)
            }
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
                let total = hero.spell_wait(card) as f32;
                let state = if left == 0 {
                    "ready".to_string()
                } else {
                    format!("{}s", left.div_ceil(HZ))
                };
                Some(Slot::new(
                    keys.spells[slot],
                    &hero.spell_name(card),
                    1.0 - left as f32 / total,
                    state,
                ))
            })
            .collect()
        };
        let hand = (0..cards::HAND)
            .map(|slot| {
                let card = self.book.get(hero.hand.get(slot)?)?;
                let key = hand_key(keys, slot);
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
            max_bombs: bombs::at(self.dungeon.depth).cap.max(hero.bombs.min(MAX_BOMBS)),
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
            corpses: minions::summoner(hero).then(|| {
                let percent = self.minions.graves.get(&id).map_or(0, |g| g.corpses);
                Slot::new("hold G", minions::golem_kind(percent).name(), f32::from(percent) / 100.0, format!("{percent}%"))
            }),
            skeleton_wall: minions::summoner(hero).then(|| {
                let grave = self.minions.graves.get(&id);
                let left = grave.map_or(0, |g| g.wall_rearm);
                let wall = grave.map_or(0, |g| g.wall);
                let state = if wall > 0 { format!("{}s up", wall.div_ceil(HZ)) }
                    else if left > 0 { format!("{}s", left.div_ceil(HZ)) } else { "ready".into() };
                Slot::new(keys.guard, "Skeleton Wall", 1.0 - left as f32 / minions::WALL_REARM as f32, state)
            }),
        })
    }
}

pub fn hand_key(keys: &Keys, slot: usize) -> &'static str {
    match slot {
        0 => keys.quaff,
        1 => keys.cycle,
        _ => "",
    }
}

pub fn tier(temper: u8) -> &'static str {
    match temper {
        0 => "",
        1 => " II",
        2 => " III",
        _ => " IV",
    }
}

fn hero_perks(hero: &Hero) -> Vec<String> {
    let mut out = Vec::new();
    if hero.rider {
        out.push("a frog on your shoulder: it rides to the next Sanctuary".to_string());
    }
    if let Some(gift) = hero.boon.from {
        out.push(gift.perk().to_string());
    }
    if hero.freedom_wishes > 0 {
        out.push(format!(
            "{} freedom wish{}: E at a Sanctuary's genie brazier or the tavern's weapons locker",
            hero.freedom_wishes,
            if hero.freedom_wishes == 1 { "" } else { "es" }
        ));
    }
    if hero.star_wish {
        out.push("a star's wish: T, free, before you leave its room".to_string());
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
    if hero.blessed > 0 {
        out.push(format!(
            "a pilgrim's blessing: +{}% damage and pace for {}s",
            super::wayfarers::BLESS_PERCENT,
            hero.blessed.div_ceil(HZ)
        ));
    }
    out
}
