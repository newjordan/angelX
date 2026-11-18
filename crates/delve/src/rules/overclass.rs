//! Wishes, held by each knight (the operator, 2026-10-18: "when game starts
//! we get 3 wishes now, and killing bosses grants a super wish, and new
//! floors grant 2 smaller wishes ... so each round in effect feel like you
//! have 3 wishes").
//!
//! Every knight holds [`START_WISHES`] when a delve starts, gets
//! [`FLOOR_WISHES`] more at each floor below, and a super wish when the
//! floor's boss falls (its guardian, the dragon in its lair). A knight
//! makes a wish whenever they hold one (`Run::wish`); a known wish (the
//! phrasebook) is granted at once, in the running game, and every friend's
//! mirror shows it next tick. A super wish takes two steps of the wish's
//! ladder, and a ladder climbs to its fourth and fifth tiers only deep
//! enough down ([`top_tier`]). The window is only the moment the game says
//! so: its sign and the Herald's call, closed by the next fight.

use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub opened: u64,
    pub wished: Vec<u32>,
    #[serde(default)]
    pub given: u8,
    #[serde(default)]
    pub great: bool,
}

pub const START_WISHES: u8 = 3;
pub const FLOOR_WISHES: u8 = 2;
pub const MOST_WISHES: u8 = 9;

pub const TIER_FLOORS: [u32; phrasebook::MAX_TIERS - 3] = [2, FLOORS];

pub fn top_tier(depth: u32) -> usize {
    2 + TIER_FLOORS.iter().filter(|&&floor| depth >= floor).count()
}

pub fn tier_floor(tier: usize) -> Option<u32> {
    tier.checked_sub(3).and_then(|t| TIER_FLOORS.get(t)).copied()
}

pub fn tier_waits(tier: usize, depth: u32) -> Option<String> {
    if tier <= top_tier(depth) {
        return None;
    }
    let numeral = ["I", "II", "III", "IV", "V"].get(tier).copied().unwrap_or("+");
    Some(match tier_floor(tier) {
        Some(floor) => format!("its tier {numeral} waits for floor {floor}"),
        None => format!("its tier {numeral} waits for a deeper floor"),
    })
}

pub fn wish_returned(why: &str) -> String {
    let why = clean_why(why);
    if why.is_empty() {
        "Your wish is given back.".to_string()
    } else {
        format!("Your wish is given back: {why}.")
    }
}

pub fn wish_kept(why: &str) -> String {
    let why = clean_why(why);
    if why.is_empty() {
        "Your wish stays with you.".to_string()
    } else {
        format!("Your wish stays with you: {why}.")
    }
}

fn clean_why(why: &str) -> &str {
    why.trim()
        .trim_matches(|c: char| c == '(' || c == ')' || c == '.' || c.is_whitespace())
}

impl Run {
    pub(super) fn floor_boss_here(&self) -> bool {
        self.room().kind == RoomKind::Lair
            || match &self.boss_gates {
                Some(g) => g.guardian == self.at,
                None => self.room().kind == RoomKind::Stairs,
            }
    }

    pub fn give_wishes(&mut self, plain: u8, great: u8) {
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            hero.wishes = hero.wishes.saturating_add(plain).min(MOST_WISHES);
            hero.great_wishes = hero.great_wishes.saturating_add(great).min(MOST_WISHES);
        }
        self.window = Some(Window { opened: self.tick, wished: Vec::new(), given: plain, great: great > 0 });
        self.cues.push("window".into());
    }

    pub(super) fn boss_wishes(&mut self) {
        self.give_wishes(0, 1);
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            hero.carried.add(Spoil::Wishmote, 1);
        }
    }

    pub(super) fn wishes_home(&mut self, whole: bool) {
        let depth = self.dungeon.depth;
        if depth == 0 {
            return;
        }
        let behind = depth - 1 + u32::from(whole);
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            let unspent = u32::from(hero.wishes) + u32::from(hero.great_wishes);
            hero.carried.add(Spoil::Wishmote, unspent.min(behind));
            (hero.wishes, hero.great_wishes) = (0, 0);
        }
    }

    pub fn can_wish(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && (h.wishes + h.great_wishes + h.fairy_wishes > 0 || h.star_wish))
    }

    pub fn spend_wish(&mut self, id: u32) -> Option<bool> {
        let unmade = self.players.get(&id).is_some_and(|h| h.unmade(&self.book));
        let hero = self.players.get(&id).filter(|h| h.hp > 0)?;
        let great = if hero.great_wishes > 0 && !(unmade && hero.wishes > 0) {
            true
        } else if hero.wishes > 0 {
            false
        } else {
            return None;
        };
        self.spend_wish_kind(id, great).then_some(great)
    }

    pub(super) fn spend_wish_kind(&mut self, id: u32, great: bool) -> bool {
        let Some(hero) = self.players.get_mut(&id).filter(|h| h.hp > 0) else {
            return false;
        };
        let held = if great {
            &mut hero.great_wishes
        } else {
            &mut hero.wishes
        };
        if *held == 0 {
            return false;
        }
        *held -= 1;
        if let Some(window) = self.window.as_mut() {
            window.wished.push(id);
        }
        true
    }

    pub fn spend_fairy_or_wish(&mut self, id: u32, fairy: bool) -> Option<bool> {
        if fairy && let Some(hero) = self.players.get_mut(&id).filter(|h| h.fairy_wishes > 0) {
            hero.fairy_wishes -= 1;
            if let Some(window) = self.window.as_mut() {
                window.wished.push(id);
            }
            return Some(false);
        }
        self.spend_wish(id)
    }

    pub fn tell(&mut self, id: u32, line: impl Into<String>) {
        let line = line.into();
        if line.is_empty() {
            return;
        }
        let held = self.seat_notices.iter().filter(|(k, _)| *k == id).count();
        if held < 8 {
            self.seat_notices.push((id, line));
        }
    }

    pub fn refund_wish(&mut self, id: u32, great: bool) {
        self.refund_told(id, great, "it did not go through");
    }

    pub fn refund_told(&mut self, id: u32, great: bool, why: &str) {
        if let Some(hero) = self.players.get_mut(&id) {
            if great {
                hero.great_wishes = hero.great_wishes.saturating_add(1);
            } else {
                hero.wishes = hero.wishes.saturating_add(1).min(MOST_WISHES);
            }
        }
        self.tell(id, wish_returned(why));
    }

    pub fn wish(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        if !self.can_wish(id) {
            return Err("No wishes held: each floor brings two, and its boss a super wish.".into());
        }
        if let Some(why) = self.identity_refusal(id, wish) {
            self.tell(id, wish_kept(&why));
            return Err(why);
        }
        let star = self.star_wish_held(id);
        let great = !star && self.players.get(&id).is_some_and(|h| h.great_wishes > 0 && !h.unmade(&self.book));
        let name = match self.grant(id, wish) {
            Ok(name) => name,
            Err(why) => {
                self.tell(id, wish_kept(&why));
                return Err(why);
            }
        };
        if star {
            self.spend_star_wish(id);
            self.star_wish_granted(id);
        } else {
            self.spend_wish(id);
        }
        Ok(if great { self.grant_more(id, wish).unwrap_or(name) } else { name })
    }

    pub fn grant_more(&mut self, id: u32, wish: &phrasebook::Wish) -> Option<String> {
        match self.grant(id, wish) {
            Ok(name) => Some(name),
            Err(why) => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.wishes = hero.wishes.saturating_add(1).min(MOST_WISHES);
                }
                self.tell(id, wish_returned(&why));
                None
            }
        }
    }

    pub(super) fn keep_wish_floor(&mut self) {
        let floor = self.dungeon.depth;
        for hero in self.players.values_mut() {
            if hero.wish_floor != floor {
                hero.wish_floor = floor;
                if hero.deck.iter().any(|c| c.starts_with("wish-")) {
                    hero.rebonus(&self.book);
                }
            }
        }
    }

    pub(super) fn wishes_for_newcomer(&mut self, id: u32) {
        if self.dungeon.depth >= 1
            && let Some(hero) = self.players.get_mut(&id)
            && hero.wishes + hero.great_wishes == 0
        {
            hero.wishes = START_WISHES;
        }
    }

    pub fn made(&mut self, id: u32, wish: &str, name: &str) {
        let name = match name.trim() {
            "" => self.players.get(&id).map_or_else(String::new, |h| h.name.clone()),
            name => name.to_string(),
        };
        if !name.is_empty() {
            self.makers.insert(wish.to_string(), (id, name));
        }
    }

    pub fn locked(&self, id: u32, wish: &phrasebook::Wish, tier: usize) -> Option<String> {
        self.players.get(&id)?;
        tier_waits(tier, self.dungeon.depth).map(|why| format!("{} climbs no higher here: {why}", wish.name))
    }

    pub fn grant(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        let Some(hero) = self.players.get(&id) else {
            return Err("no such knight".into());
        };
        if let Some(tier) = wish.next_tier(&hero.deck)
            && let Some(why) = self.locked(id, wish, tier)
        {
            return Err(why);
        }
        let Some(tier) = wish.next_tier(&hero.deck) else {
            return Err(format!("{} is already granted in full", wish.name));
        };
        let mut card = if self.fairy_grant == Some(id) {
            wish.card_graded(tier, artistry::Grade::Legendary)?
        } else {
            wish.card(tier)?
        };
        let maker = self.makers.get(&wish.id).cloned();
        if let Some((_, name)) = &maker {
            card.by = name.clone();
        }
        if self.players.values().any(|h| h.deck.contains(&card.id))
            && let Some(existing) = self.book.get(&card.id)
        {
            if !super::card_paint::same_definition(existing, &card)
                || !super::card_paint::retains_identity(existing, &card)
            {
                return Err("A knight already holds a different version of this gift; reopen with a distinct wish. Your wish is kept.".into());
            }
            card = existing.clone();
        }
        let is_tier = |id: &str| dead_wishes::wish_of(id) == Some(wish.id.as_str());
        let spell_slot = if card.kind == cards::Kind::Spell {
            Some(
                hero.spells
                    .iter()
                    .position(|slot| slot.as_ref().is_some_and(|c| is_tier(c)))
                    .or_else(|| hero.spells.iter().position(Option::is_none))
                    .or_else(|| {
                        let last = cards::SPELL_SLOTS - 1;
                        hero.spells[last].as_deref().is_some_and(knights::kit_spell).then_some(last)
                    })
                    .ok_or("all four spell slots are full")?,
            )
        } else {
            None
        };
        let name = card.name.clone();
        let card_id = card.id.clone();
        let more_hp = |card: &Card| {
            card.effects
                .iter()
                .map(|e| match *e {
                    cards::Effect::MaxHp(v) => v,
                    _ => 0,
                })
                .sum::<u32>()
        };
        let before = tier
            .checked_sub(1)
            .and_then(|t| self.book.get(&wish.card_id(t)).map(more_hp))
            .unwrap_or(0);
        let target = hero.wished_for(&wish.id);
        let gained = target.his(more_hp(&card)).saturating_sub(target.his(before));
        let held_families = |run: &Run| {
            run.players.get(&id).map_or_else(Default::default, |h| {
                combos::held_families(h.deck.iter().filter_map(|c| run.book.get(c)))
            })
        };
        let families_before = held_families(self);
        if !self.book.insert(card.clone()) {
            return Err("the book is full".into());
        }
        if wish.learned {
            let says = wish.tiers.iter().map(|(_, says)| says.clone()).collect();
            self.ladders.insert(wish.id.clone(), (wish.name.clone(), says));
        }
        let hero = self.players.get_mut(&id).expect("checked");
        hero.deck.retain(|c| !is_tier(c));
        hero.deck.push(card_id.clone());
        if let Some(ult) = card.ult {
            hero.ult = Some(ult);
        }
        match card.kind {
            cards::Kind::Guard => {
                hero.guard = card.guard.unwrap_or_default();
                hero.guard_card = Some(card_id);
                hero.shielding = false;
                hero.walling = false;
                hero.wall_spent = false;
                hero.dash_ticks = 0;
            }
            cards::Kind::Arm => hero.equip(&card),
            cards::Kind::Spell => {
                let slot = spell_slot.expect("checked before changing the book");
                if !hero.spells[slot].as_ref().is_some_and(|c| is_tier(c)) {
                    hero.spell_cooldowns[slot] = 0;
                }
                hero.spells[slot] = Some(card_id);
            }
            _ => {}
        }
        hero.rebonus(&self.book);
        hero.max_hp += gained;
        hero.hp = (hero.hp + gained).min(hero.max_hp);
        let who = match maker {
            Some((knight, name)) if knight == id => name,
            _ => hero.name.clone(),
        };
        let grade = wish.grade().map(|(g, _)| format!("{g} "));
        let grade = grade.as_deref().unwrap_or("");
        let families = held_families(self);
        let unlocks: Vec<&str> = combos::made(&families)
            .into_iter()
            .filter(|r| !combos::made(&families_before).iter().any(|b| b.effect == r.effect))
            .map(|r| r.name)
            .collect();
        let waits: Vec<String> = card
            .effects
            .iter()
            .filter_map(combos::effect_word)
            .filter(|w| !combos::unlocked(w, &families))
            .filter_map(combos::recipe)
            .map(|r| {
                let missing: Vec<&str> = r.needs.iter().filter(|f| !families.contains(f)).map(|f| f.word()).collect();
                format!("{} waits on {}", r.name, missing.join(" and "))
            })
            .collect();
        let title = if tier == 0 { wish.name.clone() } else { name.clone() };
        let mut line = match target {
            dead_wishes::Target::You => format!("{who}'s {grade}{title}"),
            dead_wishes::Target::Dead => format!("{who}'s dead wear {grade}{title}"),
            dead_wishes::Target::Both => format!("{who} and their dead wear {grade}{title}"),
        };
        if !unlocks.is_empty() {
            line.push_str(&format!(": {} unlocked", unlocks.join(", ")));
        }
        let mut name = format!("{grade}{name}");
        if target.theirs() {
            name.push_str(if target == dead_wishes::Target::Dead { ", for your dead" } else { ", for you and your dead" });
        }
        if !unlocks.is_empty() {
            name.push_str(&format!(". Unlocked: {}", unlocks.join(", ")));
        }
        if !waits.is_empty() {
            name.push_str(&format!(". {}. Families: {}", waits.join("; "), combos::list(&families)));
        }
        self.found = Some((self.tick, 0, line.clone()));
        if self.notices.len() < 8 {
            self.notices.push(line);
        }
        self.cues.push("wish_granted".into());
        self.keep_recruit_ult(id, &card);
        self.after_making(id);
        self.floor_tally.wished(id, &title);
        self.boons.push((id, self.tick));
        self.rewarded_grant(id, &wish.name, tier);
        let land = land_sound(tier);
        let landing = self.sounded_at.get(LAND_SOUNDS[0]) == Some(&self.tick);
        match self.sounds.iter().rposition(|s| LAND_SOUNDS.contains(s)).filter(|_| landing) {
            Some(at) => {
                let rank = |s: &str| LAND_SOUNDS.iter().position(|&l| l == s);
                if rank(land) > rank(self.sounds[at]) {
                    self.sounds[at] = land;
                }
            }
            None => self.sounds.push(land),
        }
        self.sounded_at.insert(LAND_SOUNDS[0], self.tick);
        Ok(name)
    }
}

pub const LAND_SOUNDS: [&str; 3] = ["wish_land", "wish_land_grand", "wish_land_mythic"];

pub fn land_sound(tier: usize) -> &'static str {
    LAND_SOUNDS[(tier / 2).min(LAND_SOUNDS.len() - 1)]
}

#[cfg(test)]
#[path = "../../tests/rules/together_overclass__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/overclass_bend_cases__tests.rs"]
mod overclass_bend_cases_tests;
