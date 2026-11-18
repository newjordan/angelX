//! The wish scroll's genie, and the offers he makes (the operator,
//! 2026-10-19, playing with friends: "there should be an extra value wish
//! menu, we should be presented with a scroll also, and options when we
//! make a wish, and maybe have a cartoon genie friend on that graphic there
//! also, and a sub plot of the game can be trying to get a wish powerful
//! enough to let the genie go").
//!
//! A knight who asks for a wish (`Run::offer_wish`) is not granted it at
//! once: the scroll opens over their own view with three ways to have it,
//! picked with 1, 2 or 3 (Esc rolls the scroll up and nothing is spent).
//! Nobody else waits: the offer is that knight's alone, and they can walk
//! and fight while it is open.
//!
//! - **As you said**: the wish as asked, a super wish two steps.
//! - **Bolder**: a step further up its ladder, for a blood price (a third
//!   of the knight's health), when the ladder has the step and the floor
//!   gives it (a fourth and fifth only deep enough down,
//!   `overclass::top_tier`); else **the genie's whim**: a wish he picks
//!   for you, a step further, no price; or, for a super wish with no step
//!   further, **save some**: one step now and a plain wish kept.
//! - **The genie's deal**: the wish as said, and a purse of gold or a
//!   mending besides; he does it for the tip, so none of it goes to his
//!   freedom.
//!
//! The ladder's own tiers are the only cards ever granted, through `grant`,
//! so the card checker and the wish caps (`cards::wish_most`) still say
//! what a wish can give.
//!
//! **Free the Genie.** He is bound by a shackle. Each wish sealed fills the
//! party's freedom gauge by its grade and the tier it reached, a super wish
//! double (`freedom_of`). When it is full, a knight who holds a super wish
//! may say "I wish the genie were free": he is freed, the whole party sees
//! it, and he stays on as a friend: every floor below gives each knight one
//! more wish ([`FREED_FLOOR_WISHES`]).

use super::*;

pub const FREEDOM: u32 = 100;
pub const BLOOD_PRICE: u32 = 3;
pub const GRANTED_TICKS: u64 = 3 * HZ as u64;
pub const FREED_TICKS: u64 = 6 * HZ as u64;
pub const FREED_FLOOR_WISHES: u8 = overclass::FLOOR_WISHES + 1;
pub const FREE_WORDS: &str = "I wish the genie were free";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Take {
    Said,
    Bolder,
    Whim,
    Split,
    Deal,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tip {
    Purse(u32),
    Mend(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub take: Take,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wish: Option<String>,
    pub steps: u8,
    pub head: String,
    pub gives: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cost: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip: Option<Tip>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub said: String,
    #[serde(default, skip_serializing_if = "super::scrolls::omit")]
    pub binding: Option<super::scrolls::Binding>,
    #[serde(default)]
    pub great: bool,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub line: String,
    pub opened: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<dead_wishes::Target>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub star: bool,
}

impl Offer {
    pub(crate) fn view(&self) -> Self {
        Self {
            said: self.said.clone(),
            binding: None,
            great: self.great,
            choices: self.choices.clone(),
            line: self.line.clone(),
            opened: self.opened,
            ask: self.ask.clone(),
            target: self.target,
            star: self.star,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sealed {
    pub knight: u32,
    pub tick: u64,
    pub line: String,
    #[serde(default)]
    pub cheer: bool,
    #[serde(default)]
    pub quiet: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genie {
    #[serde(default)]
    pub freedom: u32,
    #[serde(default)]
    pub sealed: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Sealed>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freed: Option<(u64, String)>,
}

impl Genie {
    pub fn quiet(&self) -> bool {
        *self == Genie::default()
    }

    pub fn free(&self) -> bool {
        self.freed.is_some()
    }

    pub fn ready(&self) -> bool {
        self.freedom >= FREEDOM
    }

    pub fn hint(&self) -> &'static str {
        if self.free() {
            "Free! Every floor now gives 3 wishes"
        } else if self.ready() {
            "Full! Wish him free with a super wish"
        } else {
            "Seal great wishes to fill it"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cosmetic: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cosmetic_name: String,
    pub knight: u32,
    pub words: String,
    pub steps: u8,
    pub great: bool,
    pub fills: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fairy: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub star: bool,
    pub making: Option<identity::Making>,
    pub target: dead_wishes::Target,
}

pub fn frees(words: &str) -> bool {
    let words = words.to_lowercase();
    words.contains("genie")
        && ["free", "freedom", "release", "let him go", "let the genie go", "liberat", "unchain", "unshackle"]
            .iter()
            .any(|w| words.contains(w))
}

pub fn freedom_of(grade: Option<artistry::Grade>, tier: usize, great: bool) -> u32 {
    let points = match grade {
        Some(artistry::Grade::Crude) => 1,
        Some(artistry::Grade::Fine) | None => 2,
        Some(artistry::Grade::Masterwork) => 3,
        Some(artistry::Grade::Legendary) => 4,
    };
    points * (tier as u32 + 1) * if great { 2 } else { 1 }
}

fn numeral(tier: usize) -> &'static str {
    ["I", "II", "III", "IV", "V"].get(tier).copied().unwrap_or("+")
}

pub const UPGRADE_HEAD: &str = "Upgrade what you hold";
pub const NO_WISHES: &str = "No wishes held: each floor brings two";
pub const NOTHING_HELD: &str = "Nothing held yet: say a new wish";
pub const STAR_HELD: &str = "A star's wish: free, one step. Say it before you leave its room";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upgrade {
    pub name: String,
    pub spell: bool,
    pub tier: usize,
    pub next: Option<usize>,
    pub says: String,
    pub available: bool,
}

impl Upgrade {
    pub fn words(&self) -> String {
        format!("upgrade {}", self.name)
    }

    pub fn tiers(&self) -> String {
        match self.next {
            Some(next) => format!("{} → {}", numeral(self.tier), numeral(next)),
            None => numeral(self.tier).to_string(),
        }
    }
}

impl Run {
    pub fn remember_ladders(&mut self, book: &[phrasebook::Wish]) {
        for wish in book.iter().filter(|w| w.learned) {
            if self.players.values().any(|h| h.deck.iter().any(|c| super::dead_wishes::wish_of(c) == Some(wish.id.as_str()))) {
                let lines = wish.tiers.iter().map(|(_, says)| says.clone()).collect();
                self.ladders.insert(wish.id.clone(), (wish.name.clone(), lines));
            }
        }
    }

    pub fn upgrades(&self, id: u32) -> Vec<Upgrade> {
        let Some(hero) = self.players.get(&id) else {
            return Vec::new();
        };
        let book = phrasebook::book();
        let mut held = BTreeMap::new();
        for card_id in &hero.deck {
            let Some((wish, n)) = card_id.strip_prefix("wish-").and_then(|s| s.rsplit_once('-')) else {
                continue;
            };
            let Some(tier) = n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)) else {
                continue;
            };
            held.entry(wish).and_modify(|t: &mut usize| *t = (*t).max(tier)).or_insert(tier);
        }
        let mut list: Vec<Upgrade> = held.into_iter().filter_map(|(wish, tier)| {
            let known = book.iter().find(|w| w.id == wish);
            let (name, lines) = if let Some((name, lines)) = self.ladders.get(wish) {
                (name.clone(), lines.clone())
            } else {
                let w = known?;
                (w.name.clone(), w.tiers.iter().map(|(_, says)| says.clone()).collect())
            };
            let next = (tier + 1 < lines.len()).then_some(tier + 1);
            let card_id = format!("wish-{wish}-{}", tier + 1);
            let spell = self.book.get(&card_id).map_or_else(
                || known.is_some_and(|w| w.kind == "spell"),
                |c| c.kind == cards::Kind::Spell,
            );
            let waits = next.and_then(|t| overclass::tier_waits(t, self.dungeon.depth));
            Some(Upgrade {
                name, spell, tier, next,
                says: match (&waits, next) {
                    (Some(why), _) => format!("{}{}", why[..1].to_uppercase(), &why[1..]),
                    (None, Some(t)) => lines[t].clone(),
                    (None, None) => String::new(),
                },
                available: next.is_some() && waits.is_none() && self.can_wish(id),
            })
        }).collect();
        list.extend(self.spell_upgrades(id));
        list.sort_by(|a, b| b.spell.cmp(&a.spell).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        list
    }

    pub fn upgrade_note(&self, id: u32, list: &[Upgrade]) -> Option<&'static str> {
        if !self.can_wish(id) {
            Some(NO_WISHES)
        } else if self.star_wish_held(id) {
            Some(STAR_HELD)
        } else if list.is_empty() {
            Some(NOTHING_HELD)
        } else {
            None
        }
    }
}

impl Run {
    pub fn offer_wish(
        &mut self,
        id: u32,
        said: &str,
        wish: Option<&phrasebook::Wish>,
    ) -> Result<(), String> {
        if self.players.get(&id).is_some_and(|h| h.place_wish.is_some()) {
            return self.offer_place_wish(id, said);
        }
        if self.players.get(&id).is_some_and(|h| h.mirror_wish) {
            return self.offer_mirror_wish(id, said, wish);
        }
        if self.city_wishing(id) {
            return self.offer_city_wish(id, said);
        }
        let capture =
            super::scrolls::Capture::resolved(self, id, said, wish, None, phrasebook::book())?;
        let result = capture.request.evaluate()?;
        capture.prepare(self, result)?.deliver(self)?;
        Ok(())
    }

    pub fn drop_offer(&mut self, id: u32) -> bool {
        if let Some(h) = self.players.get_mut(&id) {
            h.mirror_wish = false;
            h.place_wish = None;
            h.wish_open = false;
            h.wish_line.clear();
        }
        self.drop_city_wish(id);
        self.players.get_mut(&id).and_then(|h| h.offer.take()).is_some()
    }

    pub fn open_wish_line(&mut self, id: u32) -> bool {
        let Some(hero) = self.players.get_mut(&id) else {
            return false;
        };
        if hero.offer.is_some() {
            return false;
        }
        hero.wish_open = true;
        hero.wish_line.clear();
        hero.wish_pick = 0;
        true
    }

    pub fn type_wish_letter(&mut self, id: u32, c: char) -> bool {
        let Some(hero) = self.players.get_mut(&id) else {
            return false;
        };
        if !hero.wish_open || hero.offer.is_some() {
            return false;
        }
        if c == '\u{8}' || c == '\u{7f}' {
            hero.wish_line.pop();
            return true;
        }
        if !c.is_ascii_graphic() && c != ' ' {
            return false;
        }
        if hero.wish_line.len() >= 80 {
            return true;
        }
        hero.wish_line.push(c);
        true
    }

    pub fn choose(&mut self, id: u32, n: usize) -> Result<String, String> {
        if self.players.get(&id).is_some_and(|h| h.place_wish.is_some()) {
            return self.choose_place_wish(id, n);
        }
        if self.players.get(&id).is_some_and(|h| h.mirror_wish) {
            return self.choose_mirror_wish(id, n);
        }
        if self.city_wishing(id) {
            return self.choose_city_wish(id, n);
        }
        let Some(offer) = self.players.get(&id).and_then(|h| h.offer.clone()) else {
            return Err("No wish is open on the scroll.".into());
        };
        if !self.can_wish(id) {
            self.drop_offer(id);
            return Err("No wish to spend: the scroll rolls itself up.".into());
        }
        let Some(choice) = offer.choices.get(n).cloned() else {
            return Err("No such choice.".into());
        };
        let binding = offer.binding.as_ref().ok_or("This scroll has no retained source; reopen it. Your wish is kept.")?;
        let known = binding.chosen(self, id, &offer, n)?;
        let hero = &self.players[&id];
        if offer.star {
            if !hero.star_wish {
                return Err("The star's wish has faded; reopen the scroll. Your own wishes are kept.".into());
            }
        } else if (offer.great && hero.great_wishes == 0) || (!offer.great && hero.wishes == 0) {
            return Err("The wish this scroll offered is no longer held; reopen it. Your remaining wishes are kept.".into());
        }
        let previous_hero = self.players[&id].clone();
        if let Some(hero) = self.players.get_mut(&id) {
            hero.offer = None;
        }
        if choice.take == Take::Free {
            let result = self.free_genie(id);
            if result.is_err() { self.players.insert(id, previous_hero); }
            return result;
        }
        if let Some(spell) = choice.wish.as_deref().and_then(|w| w.strip_prefix("spell:")) {
            let result = self.climb_spell(id, spell, choice.steps, offer.great, offer.star);
            if result.is_err() { self.players.insert(id, previous_hero); }
            return result;
        }
        let making = self.players.get(&id).and_then(|h| h.making(&self.book)).filter(|m| !m.made());
        let fairy = !offer.star && making.is_none() && self.players.get(&id).is_some_and(|h| h.fairy_wishes > 0);
        let mut choice = choice;
        if fairy {
            choice.steps = choice.steps.max(nooks::gnome::FAIRY_STEPS);
        }
        let great = offer.great && !fairy;
        let fills = choice.take != Take::Deal;
        let great = great && choice.take != Take::Split;
        let target = offer.target.unwrap_or_default();
        let name = match known {
            Some(wish) => {
                self.aim_wish(id, &wish, target);
                if fairy {
                    self.fairy_grant = Some(id);
                }
                let name = match self.grant(id, &wish) {
                    Ok(name) => name,
                    Err(why) => {
                        self.fairy_grant = None;
                        self.players.insert(id, previous_hero);
                        self.tell(id, super::overclass::wish_kept(&why));
                        return Err(why);
                    }
                };
                if offer.star {
                    self.spend_star_wish(id);
                } else if fairy {
                    self.spend_fairy_or_wish(id, true);
                } else {
                    self.spend_wish_kind(id, offer.great);
                }
                let mut name = name;
                for _ in 1..choice.steps {
                    match self.grant_more(id, &wish) {
                        Some(more) => name = more,
                        None => break,
                    }
                }
                self.sealed(id, &wish, great, fills);
                self.fairy_grant = None;
                if offer.star {
                    self.sealed_quietly(id);
                    self.star_wish_granted(id);
                }
                name
            }
            None => {
                if offer.star {
                    self.spend_star_wish(id);
                } else if fairy {
                    self.spend_fairy_or_wish(id, true);
                } else {
                    self.spend_wish_kind(id, offer.great);
                }
                self.asked.push(Asked {
                    cosmetic: false, cosmetic_name: String::new(),
                    knight: id, words: offer.said.clone(), steps: choice.steps, great, fills, making, target, fairy,
                    star: offer.star,
                });
                format!("the scroll is learning '{}'", offer.said)
            }
        };
        if choice.take == Take::Split
            && let Some(hero) = self.players.get_mut(&id)
        {
            hero.wishes = hero.wishes.saturating_add(1).min(overclass::MOST_WISHES);
        }
        let gold = self.genie_gold(id, &choice);
        if choice.take == Take::Bolder
            && let Some(hero) = self.players.get_mut(&id)
        {
            let price = binding.blood_price;
            hero.hp = hero.hp.saturating_sub(price).max(1);
        }
        if gold > 0 {
            self.sounds.push("gold");
        }
        Ok(name)
    }

    fn genie_gold(&mut self, id: u32, choice: &Choice) -> u32 {
        let Some(hero) = self.players.get_mut(&id) else {
            return 0;
        };
        match choice.tip {
            Some(Tip::Purse(gold)) => {
                hero.carried.add(Spoil::Gold, gold);
                gold
            }
            Some(Tip::Mend(gold)) => {
                hero.hp = hero.max_hp;
                hero.carried.add(Spoil::Gold, gold);
                gold
            }
            None => 0,
        }
    }

    pub fn sealed(&mut self, id: u32, wish: &phrasebook::Wish, great: bool, fills: bool) {
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let tier = wish.next_tier(&hero.deck).map_or(wish.tiers.len(), |t| t).saturating_sub(1);
        let grade = if self.fairy_grant == Some(id) { Some(artistry::Grade::Legendary) } else { wish.grade().map(|(g, _)| g) };
        self.genie.sealed += 1;
        if fills && !self.genie.free() {
            self.genie.freedom = (self.genie.freedom + freedom_of(grade, tier, great)).min(FREEDOM);
        }
        let top = tier + 1 >= wish.tiers.len();
        let cheer = grade.is_some_and(|g| g >= artistry::Grade::Masterwork) || top;
        let grade = grade.map_or(String::new(), |g| format!("{g} "));
        if let Some(hero) = self.players.get_mut(&id) {
            hero.wish_open = false;
            hero.wish_line.clear();
        }
        self.genie.last = Some(Sealed {
            knight: id,
            tick: self.tick,
            line: format!("{grade}{} {}", wish.name, numeral(tier)),
            cheer,
            quiet: false,
        });
    }

    pub fn sealed_quietly(&mut self, id: u32) {
        if let Some(last) = self.genie.last.as_mut().filter(|s| s.knight == id && s.tick == self.tick) {
            last.quiet = true;
        }
    }

    fn free_genie(&mut self, id: u32) -> Result<String, String> {
        if self.genie.free() || !self.genie.ready() {
            return Err("The shackle holds.".into());
        }
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.great_wishes > 0) else {
            return Err("Only a super wish breaks the shackle.".into());
        };
        hero.great_wishes -= 1;
        let who = hero.name.clone();
        if let Some(window) = self.window.as_mut() {
            window.wished.push(id);
        }
        self.genie.freed = Some((self.tick, who.clone()));
        self.genie.sealed += 1;
        let line = format!("{who} set the genie free! He stays as a friend: every floor now gives three wishes.");
        self.found = Some((self.tick, id, line.clone()));
        if self.notices.len() < 8 {
            self.notices.push(line);
        }
        self.cues.push("genie_free".into());
        self.sounds.push("wish_grant");
        self.shake = self.shake.max(8);
        self.grant_freedom_wishes();
        Ok("The genie is free".into())
    }

    pub fn floor_wishes(&self) -> u8 {
        if self.genie.free() { FREED_FLOOR_WISHES } else { overclass::FLOOR_WISHES }
    }

    pub(super) fn offer_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        self.offer_keys.retain(|id, _| inputs.contains_key(id));
        let busy = inputs.iter().any(|(&id, input)| {
            let scroll = self.players.get(&id).is_some_and(|h| h.wish_open || h.offer.is_some());
            input.valid()
                && (scroll
                    || (self.players.get(&id).is_some_and(|h| (h.mirror_wish || h.place_wish.is_some()) && input.activity_menu))
                    || (self.city_wishing(id) && input.activity_menu)
                    || self.offer_keys.get(&id).copied().unwrap_or(0) != 0)
        });
        if !busy {
            return None;
        }
        let mut out = inputs.clone();
        for (&id, input) in out.iter_mut() {
            if !input.valid() {
                continue;
            }
            let scroll = self.players.get(&id).is_some_and(|h| h.wish_open || h.offer.is_some());
            let open = scroll
                || (self.players.get(&id).is_some_and(|h| (h.mirror_wish || h.place_wish.is_some()) && input.activity_menu))
                || (self.city_wishing(id) && input.activity_menu);
            if scroll {
                let n = self.players.get(&id).and_then(|h| h.offer.as_ref()).map(|o| o.choices.len()).unwrap_or(0);
                if n > 0 && input.aim_y < 0 {
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.wish_pick = hero.wish_pick.saturating_sub(1);
                    }
                } else if n > 0 && input.aim_y > 0 {
                    if let Some(hero) = self.players.get_mut(&id) {
                        let cap = (n - 1) as u8;
                        hero.wish_pick = hero.wish_pick.saturating_add(1).min(cap);
                    }
                }
                input.move_x = 0;
                input.move_y = 0;
                input.fire = false;
                input.dash = false;
                input.bomb = false;
                input.swing = false;
                input.aim_x = 0;
                input.aim_y = 0;
                input.interact = false;
            }
            let was = self.offer_keys.get(&id).copied().unwrap_or(0);
            if !open && was == 0 {
                continue;
            }
            if open && input.activity_menu && !self.players.get(&id).is_some_and(|h| h.wish_open && h.offer.is_none()) {
                input.activity_menu = false;
                self.drop_offer(id);
            } else if open && input.cast != 0 && input.cast != was && !(input.cast == 4 && self.turn_wish(id)) {
                let picked = self.choose(id, usize::from(input.cast) - 1);
                if let Err(why) = picked
                    && self.notices.len() < 8
                {
                    let name = self.players.get(&id).map_or_else(String::new, |h| h.name.clone());
                    self.notices.push(format!("{name}: {why}"));
                }
            }
            if input.cast != 0 {
                self.offer_keys.insert(id, input.cast);
                input.cast = 0;
            } else {
                self.offer_keys.remove(&id);
            }
        }
        Some(out)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/genie__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/wish_scroll__tests.rs"]
mod wish_scroll_tests;
