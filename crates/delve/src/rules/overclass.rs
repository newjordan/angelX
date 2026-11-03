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

/// Wishes just given: the tick, the knights who have wished since, and
/// what was given (for its sign).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub opened: u64,
    pub wished: Vec<u32>,
    /// Plain wishes each knight was given.
    #[serde(default)]
    pub given: u8,
    /// A super wish was given.
    #[serde(default)]
    pub great: bool,
}

/// Wishes each knight holds when a delve starts, and gets at each floor
/// below; the most held at once.
pub const START_WISHES: u8 = 3;
pub const FLOOR_WISHES: u8 = 2;
pub const MOST_WISHES: u8 = 9;

/// The floors a wish's fourth and fifth tiers open on: every ladder climbs
/// to its third anywhere (2026-10-22: no lock keeps the third), to its
/// fourth from the second floor down, to its fifth from Dragon Keep's. The
/// first floor's three wishes and its boss's super wish are five steps, a
/// whole ladder; this keeps the fifth for a delve that has gone deep.
pub const TIER_FLOORS: [u32; phrasebook::MAX_TIERS - 3] = [2, FLOORS];

/// The highest tier (from 0) a wish climbs to on floor `depth`
/// (`TIER_FLOORS`): the third at home and on the first floor.
pub fn top_tier(depth: u32) -> usize {
    2 + TIER_FLOORS.iter().filter(|&&floor| depth >= floor).count()
}

/// The floor tier `tier` (from 0) opens on, past the third.
pub fn tier_floor(tier: usize) -> Option<u32> {
    tier.checked_sub(3).and_then(|t| TIER_FLOORS.get(t)).copied()
}

/// Why tier `tier` (from 0) of a ladder waits on floor `depth`, if it does:
/// the scroll's and the sheet's words for it.
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

/// The line a knight sees on their own screen when a spent wish comes back.
pub fn wish_returned(why: &str) -> String {
    let why = clean_why(why);
    if why.is_empty() {
        "Your wish is given back.".to_string()
    } else {
        format!("Your wish is given back: {why}.")
    }
}

/// The line a knight sees when a grant is refused before the wish is spent:
/// the scroll closed, the wish did not.
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
    /// Whether the room just cleared is the floor's boss's: its guardian's
    /// (`boss_gates`), the dragon's lair, or with no gates the stairs room.
    pub(super) fn floor_boss_here(&self) -> bool {
        self.room().kind == RoomKind::Lair
            || match &self.boss_gates {
                Some(g) => g.guardian == self.at,
                None => self.room().kind == RoomKind::Stairs,
            }
    }

    /// Every knight here is given `plain` wishes and `great` super ones,
    /// and the game says so.
    pub fn give_wishes(&mut self, plain: u8, great: u8) {
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            hero.wishes = hero.wishes.saturating_add(plain).min(MOST_WISHES);
            hero.great_wishes = hero.great_wishes.saturating_add(great).min(MOST_WISHES);
        }
        self.window = Some(Window { opened: self.tick, wished: Vec::new(), given: plain, great: great > 0 });
        self.cues.push("window".into());
    }

    /// A floor's boss fell: a super wish each, and a wishmote each to carry
    /// home.
    pub(super) fn boss_wishes(&mut self) {
        self.give_wishes(0, 1);
        for hero in self.players.values_mut().filter(|h| !h.absent) {
            hero.carried.add(Spoil::Wishmote, 1);
        }
    }

    /// A delve ends here, cleared or not (`Run::bank_all` runs at every
    /// floor, so this is not in it): each knight's unspent wishes turn to
    /// wishmotes to carry home, one for each floor truly behind them at most,
    /// so a stair walked down and back up makes none. The wishes are spent
    /// by it: a floor deeper holds more than the wishes it gave.
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

    /// Knight `id` may wish now: they stand and hold a wish, a fairy's or a
    /// shooting star's (`starfall`) among them.
    pub fn can_wish(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && (h.wishes + h.great_wishes + h.fairy_wishes > 0 || h.star_wish))
    }

    /// Knight `id` spends a wish they hold, a super one first: `Some(true)`
    /// for a super wish, `None` when they hold none.
    pub fn spend_wish(&mut self, id: u32) -> Option<bool> {
        // A knight still wishing themselves into being spends a plain one
        // first: the super wish waits for the whole knight (`identity`).
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

    /// Consume exactly the currency an open offer displayed. A later super
    /// wish cannot silently replace a plain wish in that retained agreement.
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

    /// A fairy wish spent when it was one (`nooks::gnome`), else a wish as
    /// `spend_wish` spends it.
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

    /// A short line for knight `id`'s own screen, capped like the lines
    /// everyone is shown. The server drains it as a notice that seat alone
    /// hears (`wire::take_notices`).
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

    /// A wish that did not go through is given back (the user, 2026-10-19:
    /// "if a wish doesn't go through it should be refunded"): a super one
    /// as a super one, a plain one under the most a knight may hold. The
    /// knight is told, on their own screen, that it came back.
    pub fn refund_wish(&mut self, id: u32, great: bool) {
        self.refund_told(id, great, "it did not go through");
    }

    /// [`refund_wish`], and the knight hears `why` — the grant that failed,
    /// not a guess.
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

    /// Knight `id` makes known wish `wish` with a wish they hold: granted
    /// (`grant`), two steps for a super wish. Nothing is spent when it is
    /// refused.
    pub fn wish(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        if !self.can_wish(id) {
            return Err("No wishes held: each floor brings two, and its boss a super wish.".into());
        }
        if let Some(why) = self.identity_refusal(id, wish) {
            self.tell(id, wish_kept(&why));
            return Err(why);
        }
        // A star's wish is said first, and free (`starfall`).
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

    /// The second step of a super wish (`wish`), or of a wish the scroll
    /// learned after its knight spent a super wish on it: when the ladder
    /// has no step left to give, the knight keeps a plain wish instead.
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

    /// Every knight's wishes held to the budget of the floor they stand on
    /// (`cards::WISH_BUDGET`): counted again when the floor changes.
    pub(super) fn keep_wish_floor(&mut self) {
        let floor = self.dungeon.depth;
        for hero in self.players.values_mut() {
            if hero.wish_floor != floor {
                hero.wish_floor = floor;
                // Only wishes answer to the floor's budget.
                if hero.deck.iter().any(|c| c.starts_with("wish-")) {
                    hero.rebonus(&self.book);
                }
            }
        }
    }

    /// A knight who joins a delve under way holds a start's wishes.
    pub(super) fn wishes_for_newcomer(&mut self, id: u32) {
        if self.dungeon.depth >= 1
            && let Some(hero) = self.players.get_mut(&id)
            && hero.wishes + hero.great_wishes == 0
        {
            hero.wishes = START_WISHES;
        }
    }

    /// Knight `id` made wish `wish` in their own words (`name`, or their
    /// knight's when empty): its cards say so, and it is theirs on the sheet.
    pub fn made(&mut self, id: u32, wish: &str, name: &str) {
        let name = match name.trim() {
            "" => self.players.get(&id).map_or_else(String::new, |h| h.name.clone()),
            name => name.to_string(),
        };
        if !name.is_empty() {
            self.makers.insert(wish.to_string(), (id, name));
        }
    }

    /// Why knight `id` may not be granted tier `tier` (from 0) of `wish`,
    /// if something keeps it. No teacher does: every knight may take every
    /// tier to the third (2026-10-22: "get rid of the squire/upgrade system
    /// that locks level 3"), and the teachers still teach their skills. Only
    /// the floor keeps a ladder's fourth and fifth (`top_tier`), and the
    /// same for every knight on it.
    pub fn locked(&self, id: u32, wish: &phrasebook::Wish, tier: usize) -> Option<String> {
        self.players.get(&id)?;
        tier_waits(tier, self.dungeon.depth).map(|why| format!("{} climbs no higher here: {why}", wish.name))
    }

    /// Grant knight `id` a known wish: the next tier of its ladder, held
    /// at once (the tier below is let go). Spends no wish: `wish` does.
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
        // A fairy wish's grant (`nooks::gnome`) is drawn Legendary.
        let mut card = if self.fairy_grant == Some(id) {
            wish.card_graded(tier, artistry::Grade::Legendary)?
        } else {
            wish.card(tier)?
        };
        // A wish a knight made is theirs: its card names them.
        let maker = self.makers.get(&wish.id).cloned();
        if let Some((_, name)) = &maker {
            card.by = name.clone();
        }
        // An open scroll owns its old source, but it cannot replace a
        // different canonical definition another knight already holds.
        if self.players.values().any(|h| h.deck.contains(&card.id))
            && let Some(existing) = self.book.get(&card.id)
        {
            if !super::card_paint::same_definition(existing, &card)
                || !super::card_paint::retains_identity(existing, &card)
            {
                return Err("A knight already holds a different version of this gift; reopen with a distinct wish. Your wish is kept.".into());
            }
            // Presentation can advance while a scroll is open. Keep that
            // canonical art and maker when the executable definition agrees.
            card = existing.clone();
        }
        let is_tier = |id: &str| dead_wishes::wish_of(id) == Some(wish.id.as_str());
        // Check capacity before changing the book, deck or window. An
        // upgraded spell keeps its slot and recharge rather than duplicating.
        let spell_slot = if card.kind == cards::Kind::Spell {
            Some(
                hero.spells
                    .iter()
                    .position(|slot| slot.as_ref().is_some_and(|c| is_tier(c)))
                    .or_else(|| hero.spells.iter().position(Option::is_none))
                    // A kit that fills all four (the Gravecaller's) gives
                    // its last up to a wish: a hero keeps one slot for
                    // wishing, as the champions do.
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
        // A summoner's wish for his dead gives them its health, not him; a
        // wish for both, half (`dead_wishes`).
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
        // A seat's phrasebook is the delve's own: a learned wish's ladder
        // rides with the run, so its wish line can say what comes next.
        if wish.learned {
            let says = wish.tiers.iter().map(|(_, says)| says.clone()).collect();
            self.ladders.insert(wish.id.clone(), (wish.name.clone(), says));
        }
        let hero = self.players.get_mut(&id).expect("checked");
        hero.deck.retain(|c| !is_tier(c));
        // Keep the ladder marker in the deck for every capability, so a
        // repeated wish replaces its lower tier instead of stacking it.
        hero.deck.push(card_id.clone());
        // An ultimate wished (`identity`): R casts it from now on.
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
        // Everyone is told, once: "Ada's Bigger Arrows".
        let who = match maker {
            Some((knight, name)) if knight == id => name,
            _ => hero.name.clone(),
        };
        // How well it is drawn (`artistry`), and what it makes with the
        // knight's other wishes (`combos`).
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
        // A tier up is a moment of its own, and the realm's sky takes a
        // star (`rewards`).
        self.rewarded_grant(id, &wish.name, tier);
        // A great wish climbing several tiers at once lands once, at the
        // highest it reached: the takes share a key and a beat, and rung
        // together they would play too loud. The first landing's tick is
        // kept with the other sounds' (`sound_every`).
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

/// A wish landing on its knight, by the tier it lands at: a sparkling
/// chord swelling (I and II), grander with its lower voices and a boom
/// (III and IV), and with a bell tolling over it all (V). A great wish
/// climbing tiers at once lands once, at the highest (`Run::grant`).
pub const LAND_SOUNDS: [&str; 3] = ["wish_land", "wish_land_grand", "wish_land_mythic"];

/// The landing of tier `tier` (from 0, as `Wish::next_tier` counts).
pub fn land_sound(tier: usize) -> &'static str {
    LAND_SOUNDS[(tier / 2).min(LAND_SOUNDS.len() - 1)]
}

#[cfg(test)]
#[path = "../../tests/rules/together_overclass__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-home"))]
#[path = "../../tests/rules/overclass_bend_cases__tests.rs"]
mod overclass_bend_cases_tests;
