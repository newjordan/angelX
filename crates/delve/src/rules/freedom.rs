//! Freedom wishes (the operator, 2026-10-28: "people LOVE the wish system
//! so we should add more systems and minigames and wish oriented features
//! like if you set the genie free you get to have a wish for the safe room,
//! or weapons locker etc.").
//!
//! When the party sets the genie free (`genie`), he thanks every knight
//! standing there with a **freedom wish**. It is never spent on the knight.
//! It is spent on one of the company's own places, and what it makes there
//! stays, delve after delve:
//!
//! - **The safe room**: the Sanctuary every deeper floor begins in. A
//!   brazier of the genie's gold burns in its south-east corner, and what is
//!   wished stands along its south wall, either side of the door.
//! - **The weapons locker**: a tall armoire beside the free arms in the
//!   Sozzled Griffin. What is wished fills its bays.
//!
//! The knight walks up to the place's wishing spot, presses E, and says the
//! wish as any wish is said (the lamp's wish line). The words are read for
//! what they ask (`read`): a family of words names one of the place's gifts,
//! each with its own look and its own small good. Words the place has no
//! gift for, or a gift it has already, still stand there as a curio painted
//! by the words, so a freedom wish is never wasted. The scroll shows what the
//! words became before it is sealed with 1; Esc rolls it up and the wish is
//! kept.
//!
//! What is wished is the realm's (`Home::wished`), kept beside the wardrobe
//! and the privy wall from delve to delve, six to a place. A freedom wish
//! not yet spent is kept there too, by the knight's name, so a wish for the
//! weapons locker earned deep in a delve waits for the next one.

use super::*;
use std::collections::BTreeSet;

pub const MOST_PER_PLACE: usize = 6;
pub const MOST_HELD: u8 = 3;
pub const WORDS: usize = 48;
pub const REACH: f32 = 2.2;
pub const LANDING_TICKS: u64 = 4 * HZ as u64;
pub const RESTED_WARD: u32 = 4 * HZ;
pub const HEARTH_SHARE: u32 = 5;
pub const WHETSTONE: u32 = 10;
pub const FLETCHING: u32 = 10;
pub const DUMMY_SHARE: u32 = 4;

pub const LOCKER_AT: (f32, f32) = (25.4, 17.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Safe,
    Locker,
}

impl Place {
    pub fn name(self) -> &'static str {
        match self {
            Place::Safe => "the safe room",
            Place::Locker => "the weapons locker",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gift {
    Hearth,
    Spring,
    Feast,
    WarMap,
    Featherbed,
    Whetstone,
    Fletching,
    PowderKeg,
    ArmourStand,
    Dummy,
    #[serde(other)]
    Curio,
}

pub struct Kind {
    pub gift: Gift,
    pub place: Place,
    pub key: &'static str,
    pub name: &'static str,
    pub does: &'static str,
    pub words: &'static [&'static str],
}

pub const KINDS: [Kind; 10] = [
    Kind {
        gift: Gift::Hearth,
        place: Place::Safe,
        key: "hearth",
        name: "A Hearth",
        does: "Each Sanctuary mends a fifth more",
        words: &[
            "hearth", "fire", "fireplace", "fireside", "firepit", "campfire", "bonfire", "flame", "blaze",
            "ember", "warm", "warmth", "cosy", "cozy", "log", "stove", "inglenook", "chimney", "heat",
        ],
    },
    Kind {
        gift: Gift::Spring,
        place: Place::Safe,
        key: "spring",
        name: "A Hot Spring",
        does: "Spells come ready, curses wash off",
        words: &[
            "spring", "hotspring", "bath", "bathhouse", "tub", "pool", "steam", "soak", "onsen", "spa",
            "sauna", "jacuzzi", "geyser", "fountain", "water", "basin", "font", "hot tub",
        ],
    },
    Kind {
        gift: Gift::Feast,
        place: Place::Safe,
        key: "feast",
        name: "A Feast Table",
        does: "A potion for every hand, each floor",
        words: &[
            "feast", "food", "banquet", "dinner", "supper", "breakfast", "lunch", "meal", "bread", "pie",
            "roast", "meat", "ham", "cheese", "cake", "stew", "soup", "ale", "mead", "wine", "beer",
            "kitchen", "pantry", "larder", "snack", "buffet", "picnic", "cook", "chef", "oven", "potion",
            "elixir", "draught",
        ],
    },
    Kind {
        gift: Gift::WarMap,
        place: Place::Safe,
        key: "war_map",
        name: "A War Map",
        does: "Every room of the floor charted",
        words: &[
            "map", "chart", "atlas", "scout", "compass", "globe", "plan", "strategy", "cartograph",
            "survey", "spyglass", "telescope", "war table", "war room", "battle plan", "campaign",
        ],
    },
    Kind {
        gift: Gift::Featherbed,
        place: Place::Safe,
        key: "featherbed",
        name: "A Featherbed",
        does: "Well rested: a ward out of the door",
        words: &[
            "bed", "featherbed", "feather", "pillow", "cushion", "sleep", "nap", "rest", "cot", "hammock",
            "blanket", "quilt", "mattress", "bunk", "dream", "couch", "sofa", "futon", "snooze", "slumber",
        ],
    },
    Kind {
        gift: Gift::Whetstone,
        place: Place::Locker,
        key: "whetstone",
        name: "A Whetstone",
        does: "+10% damage the first floor",
        words: &[
            "whetstone", "whet", "sharpen", "grindstone", "grind", "hone", "edge", "blade", "sword",
            "steel", "oil", "anvil", "forge", "smith", "blacksmith", "razor", "keen",
        ],
    },
    Kind {
        gift: Gift::Fletching,
        place: Place::Locker,
        key: "fletching",
        name: "A Fletcher's Rack",
        does: "+10% rate of fire the first floor",
        words: &[
            "fletch", "fletching", "fletcher", "arrow", "bow", "bowstring", "string", "crossbow", "quiver",
            "archer", "archery", "bolt", "longbow", "shortbow", "feather", "nock",
        ],
    },
    Kind {
        gift: Gift::PowderKeg,
        place: Place::Locker,
        key: "powder_keg",
        name: "A Powder Keg",
        does: "A bomb more for every belt",
        words: &[
            "bomb", "powder", "gunpowder", "keg", "barrel", "explosive", "grenade", "dynamite", "boom",
            "ammo", "ammunition", "blast", "cannon", "shell", "fuse", "firework", "tnt",
        ],
    },
    Kind {
        gift: Gift::ArmourStand,
        place: Place::Locker,
        key: "armour_stand",
        name: "An Armour Stand",
        does: "Every knight goes down helmed",
        words: &[
            "armour", "armor", "armoury", "armory", "helm", "helmet", "mail", "chainmail", "plate",
            "cuirass", "breastplate", "mannequin", "gauntlet", "visor", "hauberk", "shield", "greaves",
            "gorget",
        ],
    },
    Kind {
        gift: Gift::Dummy,
        place: Place::Locker,
        key: "dummy",
        name: "A Practice Dummy",
        does: "Ultimates begin a quarter charged",
        words: &[
            "dummy", "practice", "training", "train", "target", "drill", "spar", "sparring", "scarecrow",
            "bullseye", "quintain", "dojo", "punching bag", "sandbag",
        ],
    },
];

pub const CURIO_DOES: &str = "For the look of it";

pub fn kind(gift: Gift) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.gift == gift)
}

fn tokens(words: &str) -> Vec<String> {
    words
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn says(tokens: &[String], phrase: &str, stem: &str) -> bool {
    if stem.contains(' ') {
        return format!(" {phrase} ").contains(&format!(" {stem} "));
    }
    tokens.iter().any(|t| {
        t == stem
            || t.strip_suffix('s').is_some_and(|t| t == stem)
            || t.strip_suffix("es").is_some_and(|t| t == stem)
            || (stem.len() >= 5 && t.starts_with(stem))
    })
}

pub fn read(place: Place, words: &str) -> Gift {
    let tokens = tokens(words);
    let phrase = tokens.join(" ");
    let mut best: Option<(usize, Gift)> = None;
    for kind in KINDS.iter().filter(|k| k.place == place) {
        let n = kind.words.iter().filter(|stem| says(&tokens, &phrase, stem)).count();
        if n > 0 && best.is_none_or(|(b, _)| n > b) {
            best = Some((n, kind.gift));
        }
    }
    best.map_or(Gift::Curio, |(_, gift)| gift)
}

pub fn plaque_words(words: &str) -> String {
    let line = words.split_whitespace().collect::<Vec<_>>().join(" ");
    line.chars().filter(|c| !c.is_control()).take(WORDS).collect::<String>().trim().to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upgrade {
    pub gift: Gift,
    pub words: String,
    pub by: String,
}

impl Upgrade {
    pub fn name(&self) -> String {
        match kind(self.gift) {
            Some(k) => k.name.to_string(),
            None => self.words.clone(),
        }
    }

    pub fn does(&self) -> &'static str {
        kind(self.gift).map_or(CURIO_DOES, |k| k.does)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wished {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub safe: Vec<Upgrade>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locker: Vec<Upgrade>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub held: BTreeMap<String, u8>,
}

impl Wished {
    pub fn is_empty(&self) -> bool {
        *self == Wished::default()
    }

    pub fn valid(&self) -> bool {
        let upgrade = |u: &Upgrade| u.words.chars().count() <= WORDS && u.by.chars().count() <= 32;
        self.safe.len() <= MOST_PER_PLACE
            && self.locker.len() <= MOST_PER_PLACE
            && self.safe.iter().chain(&self.locker).all(upgrade)
            && self.held.len() <= 256
            && self.held.iter().all(|(name, &n)| name.chars().count() <= 32 && (1..=MOST_HELD).contains(&n))
    }

    pub fn of(&self, place: Place) -> &[Upgrade] {
        match place {
            Place::Safe => &self.safe,
            Place::Locker => &self.locker,
        }
    }

    fn of_mut(&mut self, place: Place) -> &mut Vec<Upgrade> {
        match place {
            Place::Safe => &mut self.safe,
            Place::Locker => &mut self.locker,
        }
    }

    pub fn has(&self, place: Place, gift: Gift) -> bool {
        gift != Gift::Curio && self.of(place).iter().any(|u| u.gift == gift)
    }

    pub fn full(&self, place: Place) -> bool {
        self.of(place).len() >= MOST_PER_PLACE
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Landed {
    pub place: Place,
    pub slot: u8,
    pub knight: u32,
    pub tick: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landed: Option<Landed>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub given: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub charted: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rested: bool,
    #[serde(
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    held: BTreeSet<u32>,
}

impl State {
    pub fn omit(v: &State) -> bool {
        v.landed.is_none()
            && !v.given
            && !v.charted
            && !v.rested
            && (crate::rules::runtime_state::omit(v) || v.held.is_empty())
    }

    pub fn valid(&self) -> bool {
        let seat = |id: &u32| (1..=MAX_PLAYERS).contains(id);
        self.held.len() <= MAX_PLAYERS as usize
            && self.held.iter().all(seat)
            && self.landed.as_ref().is_none_or(|l| usize::from(l.slot) < MOST_PER_PLACE && seat(&l.knight))
    }

    pub(super) fn new_floor(&mut self) {
        self.given = false;
        self.charted = false;
        self.rested = false;
    }
}

pub fn brazier_at(room: &Room) -> (f32, f32) {
    let (cols, rows) = (room.cols as f32, room.rows as f32);
    ((cols / 2.0 + 9.7) * TILE_UNITS, (rows - 2.1) * TILE_UNITS)
}

pub fn slot_at(room: &Room, slot: usize) -> (f32, f32) {
    const FROM_DOOR: [f32; MOST_PER_PLACE] = [2.6, -2.6, 5.1, -5.1, 7.6, -7.6];
    let (cols, rows) = (room.cols as f32, room.rows as f32);
    let dx = FROM_DOOR[slot.min(MOST_PER_PLACE - 1)];
    ((cols / 2.0 + dx) * TILE_UNITS, (rows - 2.1) * TILE_UNITS)
}

fn choice(gift: Gift, words: &str, again: bool) -> genie::Choice {
    let (head, gives) = match kind(gift) {
        Some(k) => (k.name.to_string(), k.does.to_string()),
        None if again => ("A curio".to_string(), format!("\"{words}\": the gift is there already")),
        None => ("A curio".to_string(), format!("\"{words}\", for the look of it")),
    };
    genie::Choice {
        take: genie::Take::Said,
        wish: Some(format!("place:{}", kind(gift).map_or("curio", |k| k.key))),
        steps: 1,
        head,
        gives,
        cost: "A freedom wish".into(),
        tip: None,
    }
}

impl Run {
    pub(super) fn grant_freedom_wishes(&mut self) {
        let ids: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
        for id in ids {
            let held = self.players.get(&id).map_or(0, |h| h.freedom_wishes);
            self.set_freedom_wishes(id, held.saturating_add(1).min(MOST_HELD));
        }
        self.cues.push("freedom_wish".into());
    }

    fn set_freedom_wishes(&mut self, id: u32, n: u8) {
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.freedom_wishes = n;
        let name = hero.name.clone();
        if n == 0 {
            self.home.wished.held.remove(&name);
        } else {
            self.home.wished.held.insert(name, n);
        }
    }

    pub(super) fn restore_freedom_wishes(&mut self, id: u32) {
        if let Some(hero) = self.players.get_mut(&id) {
            hero.freedom_wishes = self.home.wished.held.get(&hero.name).copied().unwrap_or(0).min(MOST_HELD);
        }
    }

    pub fn freedom_place(&self) -> Option<Place> {
        match self.room().kind {
            RoomKind::Sanctuary if !self.at_home_now() => Some(Place::Safe),
            RoomKind::Alehouse => Some(Place::Locker),
            _ => None,
        }
    }

    pub fn freedom_spot(&self) -> Option<(Place, (f32, f32))> {
        let place = self.freedom_place()?;
        let shown = !self.home.wished.of(place).is_empty()
            || self.genie.free()
            || self.players.values().any(|h| !h.absent && h.freedom_wishes > 0);
        if !shown {
            return None;
        }
        let at = match place {
            Place::Safe => brazier_at(self.room()),
            Place::Locker => (LOCKER_AT.0 * TILE_UNITS, LOCKER_AT.1 * TILE_UNITS),
        };
        Some((place, at))
    }

    pub fn near_freedom_spot(&self, id: u32) -> bool {
        let (Some((_, (x, y))), Some(hero)) = (self.freedom_spot(), self.players.get(&id)) else {
            return false;
        };
        hero.hp > 0 && (hero.x - x).hypot(hero.y - y) < REACH * TILE_UNITS
    }

    pub fn wish_line_opens(&self, id: u32) -> bool {
        self.can_wish(id) || self.players.get(&id).is_some_and(|h| h.mirror_wish || h.place_wish.is_some()) || self.city_wishing(id)
    }

    pub fn special_wish_note(&self, id: u32) -> Option<&'static str> {
        let hero = self.players.get(&id)?;
        match hero.place_wish {
            Some(Place::Safe) => Some("A freedom wish for the safe room: say what it should have."),
            Some(Place::Locker) => Some("A freedom wish for the weapons locker: say what it should hold."),
            None if hero.mirror_wish => Some("Wish a look: cosmetic only; no wish spent."),
            None if self.city_wishing(id) => Some("A wish for the city: say what it should have. None of yours is spent."),
            None => None,
        }
    }

    pub(super) fn tick_freedom(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.freedom.landed.as_ref().is_some_and(|l| self.tick.saturating_sub(l.tick) >= LANDING_TICKS) {
            self.freedom.landed = None;
        }
        let here = self.freedom_place();
        for hero in self.players.values_mut() {
            if hero.place_wish.is_some() && hero.place_wish != here {
                hero.place_wish = None;
                hero.offer = None;
            }
        }
        if self.freedom_spot().is_none() {
            self.freedom.held.clear();
            return;
        }
        let mut pressed = Vec::new();
        for (&id, hero) in &self.players {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down {
                self.freedom.held.remove(&id);
                continue;
            }
            if hero.hp == 0 || hero.absent || !self.freedom.held.insert(id) {
                continue;
            }
            if self.near_freedom_spot(id) {
                pressed.push(id);
            }
        }
        for id in pressed {
            match self.begin_place_wish(id) {
                Ok(()) => self.sounds.push("spell_cast"),
                Err(why) => self.found = Some((self.tick, id, why)),
            }
        }
    }

    pub fn begin_place_wish(&mut self, id: u32) -> Result<(), String> {
        let place = self.freedom_place().ok_or("NO WISHING SPOT HERE")?;
        if self.home.wished.full(place) {
            return Err(format!("{} HOLDS ALL THE WISHES IT CAN", place.name().to_uppercase()));
        }
        let hero = self.players.get_mut(&id).filter(|h| h.hp > 0 && !h.absent).ok_or("NO KNIGHT AT THE SPOT")?;
        if hero.offer.is_some() {
            return Err("YOUR WISH SCROLL IS OPEN: CHOOSE FROM IT FIRST".into());
        }
        if hero.freedom_wishes == 0 {
            return Err("A FREEDOM WISH IS SPENT HERE: SET THE GENIE FREE TO EARN ONE".into());
        }
        hero.mirror_wish = false;
        hero.place_wish = Some(place);
        self.lamp.rubbed.insert(id, self.tick);
        Ok(())
    }

    pub(crate) fn offer_place_wish(&mut self, id: u32, words: &str) -> Result<(), String> {
        let words = plaque_words(words);
        if words.is_empty() {
            return Err("Say what the place should have.".into());
        }
        let place = self.players.get(&id).and_then(|h| h.place_wish).ok_or("No freedom wish begun.")?;
        if self.home.wished.full(place) {
            return Err(format!("{} holds all the wishes it can.", place.name()));
        }
        let asked = read(place, &words);
        let again = self.home.wished.has(place, asked);
        let gift = if again { Gift::Curio } else { asked };
        let line = match (gift, place) {
            (Gift::Curio, _) if again => "Already there, friend! I'll paint your words instead.",
            (Gift::Curio, _) => "Not a thing I know... but I'll paint it there for you!",
            (_, Place::Safe) => "Freely given, friend! The safe room will keep it.",
            (_, Place::Locker) => "Freely given, friend! The locker will keep it.",
        };
        let tick = self.tick;
        let hero = self.players.get_mut(&id).filter(|h| h.hp > 0).ok_or("No knight at the spot.")?;
        hero.offer = Some(genie::Offer {
            said: words.clone(),
            binding: None,
            great: false,
            choices: vec![choice(gift, &words, again)],
            line: line.into(),
            opened: tick,
            ask: None,
            target: None,
            star: false,
        });
        Ok(())
    }

    pub(super) fn choose_place_wish(&mut self, id: u32, n: usize) -> Result<String, String> {
        let hero = self.players.get(&id).ok_or("No knight.")?;
        let offer = hero.offer.clone().ok_or("No wish is open on the scroll.")?;
        let place = hero.place_wish.ok_or("No freedom wish begun.")?;
        let choice = offer.choices.get(n).ok_or("No such choice.")?;
        if hero.freedom_wishes == 0 {
            self.drop_offer(id);
            return Err("No freedom wish to spend: the scroll rolls itself up.".into());
        }
        if self.home.wished.full(place) {
            self.drop_offer(id);
            return Err(format!("{} holds all the wishes it can.", place.name()));
        }
        let gift = choice
            .wish
            .as_deref()
            .and_then(|w| w.strip_prefix("place:"))
            .and_then(|key| KINDS.iter().find(|k| k.key == key && k.place == place))
            .map(|k| k.gift)
            .filter(|&g| !self.home.wished.has(place, g))
            .unwrap_or(Gift::Curio);
        let by = hero.name.clone();
        let held = hero.freedom_wishes;
        if let Some(hero) = self.players.get_mut(&id) {
            hero.offer = None;
            hero.place_wish = None;
        }
        self.set_freedom_wishes(id, held - 1);
        let upgrade = Upgrade { gift, words: offer.said.clone(), by: by.clone() };
        let name = upgrade.name();
        let list = self.home.wished.of_mut(place);
        list.push(upgrade);
        let slot = (list.len() - 1) as u8;
        self.freedom.landed = Some(Landed { place, slot, knight: id, tick: self.tick });
        if place == Place::Safe && gift != Gift::Curio {
            self.safe_gift(gift);
            self.freedom.given = true;
        }
        let line = format!("{} WISHED {} INTO {}!", by.to_uppercase(), name.to_uppercase(), place.name().to_uppercase());
        self.found = Some((self.tick, id, line.clone()));
        if self.notices.len() < 8 {
            self.notices.push(line);
        }
        self.cues.push("freedom_wish_landed".into());
        self.sounds.push("wish_grant");
        self.shake = self.shake.max(5);
        Ok(name)
    }

    pub(super) fn freedom_hallow(&mut self) {
        if self.freedom.given || self.home.wished.safe.iter().all(|u| u.gift == Gift::Curio) {
            return;
        }
        self.freedom.given = true;
        let gifts: Vec<Gift> = self.home.wished.safe.iter().map(|u| u.gift).collect();
        for gift in gifts {
            self.safe_gift(gift);
        }
    }

    fn safe_gift(&mut self, gift: Gift) {
        match gift {
            Gift::Hearth => {
                for hero in self.players.values_mut().filter(|h| h.hp > 0) {
                    hero.mend(hero.max_hp / HEARTH_SHARE, stakes::Mend::Rest);
                }
            }
            Gift::Spring => {
                for hero in self.players.values_mut() {
                    hero.spell_cooldowns = [0; cards::SPELL_SLOTS];
                    hero.mana = MAX_MANA;
                    (hero.hexed, hero.chilled, hero.mired) = (0, 0, 0);
                }
            }
            Gift::Feast => {
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.absent) {
                    if hero.hand.len() < cards::HAND {
                        hero.hand.push("potion".into());
                    }
                }
            }
            Gift::WarMap => self.freedom.charted = true,
            Gift::Featherbed => self.freedom.rested = true,
            _ => {}
        }
    }

    pub(super) fn freedom_leave_safe_room(&mut self) {
        if !std::mem::take(&mut self.freedom.rested) {
            return;
        }
        for hero in self.players.values_mut().filter(|h| h.hp > 0) {
            hero.invulnerable = hero.invulnerable.max(RESTED_WARD);
        }
    }

    pub(super) fn freedom_kit_out(&mut self) {
        let gifts: Vec<Gift> = self.home.wished.locker.iter().map(|u| u.gift).collect();
        for gift in gifts {
            for hero in self.players.values_mut() {
                match gift {
                    Gift::Whetstone => hero.boon.damage = hero.boon.damage.max(WHETSTONE),
                    Gift::Fletching => hero.boon.rate = hero.boon.rate.max(FLETCHING),
                    Gift::PowderKeg => hero.give_bombs(1),
                    Gift::ArmourStand => {
                        hero.owned.helmet = true;
                        hero.dress();
                    }
                    Gift::Dummy => hero.ult_charge = hero.ult_charge.max(ults::ULT_FULL / DUMMY_SHARE),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/freedom__tests.rs"]
mod tests;
