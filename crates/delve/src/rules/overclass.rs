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
//! ladder. The window is only the moment the game says so: its sign and
//! the Herald's call, closed by the next fight.

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

    /// The moment wishes are given, with nothing given (tests, and the
    /// sign's old callers).
    pub(super) fn open_window(&mut self) {
        self.window = Some(Window { opened: self.tick, ..Window::default() });
        self.cues.push("window".into());
    }

    /// Knight `id` may wish now: they stand and hold a wish.
    pub fn can_wish(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && h.wishes + h.great_wishes > 0)
    }

    /// Knight `id` spends a wish they hold, a super one first: `Some(true)`
    /// for a super wish, `None` when they hold none.
    pub fn spend_wish(&mut self, id: u32) -> Option<bool> {
        let hero = self.players.get_mut(&id).filter(|h| h.hp > 0)?;
        let great = if hero.great_wishes > 0 {
            hero.great_wishes -= 1;
            true
        } else if hero.wishes > 0 {
            hero.wishes -= 1;
            false
        } else {
            return None;
        };
        if let Some(window) = self.window.as_mut() {
            window.wished.push(id);
        }
        Some(great)
    }

    /// Knight `id` makes known wish `wish` with a wish they hold: granted
    /// (`grant`), two steps for a super wish. Nothing is spent when it is
    /// refused.
    pub fn wish(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        if !self.can_wish(id) {
            return Err("No wishes held: each floor brings two, and its boss a super wish.".into());
        }
        let great = self.players.get(&id).is_some_and(|h| h.great_wishes > 0);
        let name = self.grant(id, wish)?;
        self.spend_wish(id);
        Ok(if great { self.grant_more(id, wish).unwrap_or(name) } else { name })
    }

    /// The second step of a super wish (`wish`), or of a wish the scroll
    /// learned after its knight spent a super wish on it: when the ladder
    /// has no step left to give, the knight keeps a plain wish instead.
    pub fn grant_more(&mut self, id: u32, wish: &phrasebook::Wish) -> Option<String> {
        match self.grant(id, wish) {
            Ok(name) => Some(name),
            Err(_) => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.wishes = hero.wishes.saturating_add(1).min(MOST_WISHES);
                }
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

    /// Grant knight `id` a known wish: the next tier of its ladder, held
    /// at once (the tier below is let go). Spends no wish: `wish` does.
    pub fn grant(&mut self, id: u32, wish: &phrasebook::Wish) -> Result<String, String> {
        let Some(hero) = self.players.get(&id) else {
            return Err("no such knight".into());
        };
        // The school's locks: a wish above the knight's rank answers with
        // its rank and the nearest mission that earns it. The teachers'
        // lock sits above it: tier 3 and above needs the power's questline
        // walked to its end, whatever the rank. One line, naming the
        // teacher and where they live. Tiers 1 and 2 stay as the school
        // set them.
        if let Some(tier) = wish.next_tier(&hero.deck) {
            if tier + 1 >= 3 {
                let power = format!("wish:{}", wish.id);
                if !self
                    .home
                    .questlines
                    .walk(&super::talents::knight_key(hero), &power)
                    .finished()
                {
                    let q = super::questline::book().get(&power);
                    return Err(match q {
                        Some(q) => format!(
                            "{} tier 3 is taught, not wished: find {} at {}",
                            wish.name, q.teacher, q.place
                        ),
                        None => format!("{} tier 3 is taught, not wished", wish.name),
                    });
                }
            }
            let standing = self.home.school.standing(&super::talents::knight_key(hero));
            if let Some((rank, mission)) = school::Lock::wish(&wish.id, tier + 1) {
                if standing.rank < rank {
                    return Err(format!(
                        "the school keeps that under lock: {name} needs {rank} — {mission}",
                        name = wish.name,
                        rank = rank.name(),
                    ));
                }
            }
        }
        let Some(tier) = wish.next_tier(&hero.deck) else {
            return Err(format!("{} is already granted in full", wish.name));
        };
        let mut card = wish.card(tier)?;
        // A wish a knight made is theirs: its card names them.
        let maker = self.makers.get(&wish.id).cloned();
        if let Some((_, name)) = &maker {
            card.by = name.clone();
        }
        let prefix = format!("wish-{}-", wish.id);
        // Check capacity before changing the book, deck or window. An
        // upgraded spell keeps its slot and recharge rather than duplicating.
        let spell_slot = if card.kind == cards::Kind::Spell {
            Some(
                hero.spells
                    .iter()
                    .position(|slot| slot.as_ref().is_some_and(|c| c.starts_with(&prefix)))
                    .or_else(|| hero.spells.iter().position(Option::is_none))
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
        let gained = more_hp(&card).saturating_sub(before);
        let held_families = |run: &Run| {
            run.players.get(&id).map_or_else(Default::default, |h| {
                combos::held_families(h.deck.iter().filter_map(|c| run.book.get(c)))
            })
        };
        let families_before = held_families(self);
        if !self.book.insert(card.clone()) {
            return Err("the book is full".into());
        }
        let hero = self.players.get_mut(&id).expect("checked");
        hero.deck.retain(|c| !c.starts_with(&prefix));
        // Keep the ladder marker in the deck for every capability, so a
        // repeated wish replaces its lower tier instead of stacking it.
        hero.deck.push(card_id.clone());
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
                if hero.spells[slot].is_none() {
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
        let mut line = format!("{who}'s {grade}{title}");
        if !unlocks.is_empty() {
            line.push_str(&format!(": {} unlocked", unlocks.join(", ")));
        }
        let mut name = format!("{grade}{name}");
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
        self.floor_tally.wished(id, &title);
        self.boons.push((id, self.tick));
        self.sounds.push("wish_grant");
        Ok(name)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_overclass__tests.rs"]
mod tests;
