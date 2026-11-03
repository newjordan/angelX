//! A visitor's wish for the city (the operator, 2026-10-28: "make a wish
//! for their city as a visitor"). Every knight who comes down into a
//! village of the Deep may make one wish for it at its wishing place: the
//! well, the stone, the anvil or the lamp. It spends none of the knight's
//! own wishes and gives the knight nothing. It goes on the ordinary wish
//! scroll, as the tavern mirror's look does (`cosmetic_wish`): E at the
//! wishkeeper opens the wish line, the words come back on the scroll as up
//! to three readings of what the city should get, and 1, 2 or 3 builds it.
//!
//! What is built stands in the village for good, in the next of its six
//! places (`SLOTS`), with the knight's words on it: a lantern tree, a
//! garden, a statue of the knight, a fountain, a bell tower, a market
//! stall, a reading stand, a hearth. The realm keeps it (`Town::wishes`);
//! the village cheers, grows brighter, and shows it to every party after.

use super::village::Village;
use super::*;

/// The places in a village a wish can stand.
pub const SLOTS: usize = 6;
/// The most words a city keeps on a wish.
pub const MOST_WORDS: usize = 120;
/// How long a new improvement takes to rise, in ticks.
pub const RISE: u64 = 2 * HZ as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gift {
    Lanterns,
    Garden,
    Statue,
    Fountain,
    Bell,
    Market,
    Library,
    Hearth,
}

impl Gift {
    pub const ALL: [Gift; 8] = [
        Gift::Lanterns,
        Gift::Garden,
        Gift::Statue,
        Gift::Fountain,
        Gift::Bell,
        Gift::Market,
        Gift::Library,
        Gift::Hearth,
    ];

    /// What the scroll calls it.
    pub fn name(self) -> &'static str {
        match self {
            Gift::Lanterns => "A LANTERN TREE",
            Gift::Garden => "A GLOWING GARDEN",
            Gift::Statue => "A STATUE OF YOU",
            Gift::Fountain => "A FOUNTAIN",
            Gift::Bell => "A BELL TOWER",
            Gift::Market => "A MARKET STALL",
            Gift::Library => "A READING STAND",
            Gift::Hearth => "A GREAT HEARTH",
        }
    }

    /// The words that call it up.
    fn words(self) -> &'static [&'static str] {
        match self {
            Gift::Lanterns => &["light", "lamp", "lantern", "glow", "bright", "shine", "sun", "star", "stars", "sky", "day", "hope"],
            Gift::Garden => &["garden", "flower", "flowers", "tree", "trees", "grow", "green", "plant", "plants", "moss", "bloom", "fruit", "life"],
            Gift::Statue => &["statue", "me", "my", "hero", "knight", "monument", "honor", "honour", "remember", "name", "glory", "us"],
            Gift::Fountain => &["water", "fountain", "well", "spring", "drink", "rain", "pool", "bath", "clean", "river"],
            Gift::Bell => &["bell", "music", "song", "songs", "sing", "dance", "tower", "chime", "festival", "party", "joy", "laugh"],
            Gift::Market => &["market", "trade", "bread", "feast", "food", "gold", "rich", "shop", "plenty", "wealth", "eat", "coin"],
            Gift::Library => &["book", "books", "school", "learn", "story", "stories", "read", "wisdom", "library", "knowledge", "teach"],
            Gift::Hearth => &["warm", "fire", "hearth", "home", "safe", "shelter", "house", "houses", "roof", "peace", "rest", "heal"],
        }
    }
}

/// A wish a visitor made for a city, standing in its slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CityWish {
    /// The knight, by the name the realm keeps them under.
    pub knight: String,
    /// Their name as shown.
    pub shown: String,
    pub words: String,
    pub gift: Gift,
    pub slot: u8,
}

impl CityWish {
    pub fn valid(&self) -> bool {
        usize::from(self.slot) < SLOTS && self.words.chars().count() <= MOST_WORDS && self.knight.len() <= 128 && self.shown.len() <= 128
    }
}

/// What a city might make of `words`, best first: the gifts whose words
/// they use, then the plainest gifts, three in all.
pub fn read(words: &str) -> Vec<Gift> {
    let said: Vec<String> = words
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut scored: Vec<(usize, usize, Gift)> = Gift::ALL
        .iter()
        .enumerate()
        .map(|(i, &g)| (said.iter().filter(|w| g.words().contains(&w.as_str())).count(), i, g))
        .filter(|&(n, _, _)| n > 0)
        .collect();
    scored.sort_by_key(|&(n, i, _)| (std::cmp::Reverse(n), i));
    let mut out: Vec<Gift> = scored.into_iter().map(|(_, _, g)| g).collect();
    // The words' own sound picks the rest, so two wishes read alike still
    // differ.
    let h = words.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut k = h as usize;
    while out.len() < 3 {
        let g = Gift::ALL[k % Gift::ALL.len()];
        if !out.contains(&g) {
            out.push(g);
        }
        k = k.wrapping_add(3);
    }
    out.truncate(3);
    out
}

impl Run {
    /// Whether knight `id`'s scroll is open for a city's wish.
    pub fn city_wishing(&self, id: u32) -> bool {
        self.deep.as_ref().and_then(|d| d.village.as_ref()).is_some_and(|v| v.wishing.contains(&id))
    }

    /// Whether knight `id` has made their wish for this village already.
    pub(super) fn wished_here(&self, v: &Village, id: u32) -> bool {
        let Some(h) = self.players.get(&id) else {
            return true;
        };
        let key = super::super::talents::knight_key(h);
        self.home.deep.town(v.floor).is_some_and(|t| t.wishes.iter().any(|w| w.knight == key))
    }

    /// The wishkeeper hands knight `id` the wish line: the lamp's own rub
    /// opens it on their screen, and the words that come back are the
    /// city's (`offer_city_wish`).
    pub(super) fn begin_city_wish(&mut self, v: &mut Village, id: u32) {
        let Some(h) = self.players.get_mut(&id) else {
            return;
        };
        h.offer = None;
        v.wishing.insert(id);
        self.lamp.rubbed.insert(id, self.tick);
        self.sounds.push("play_card");
        self.found = Some((self.tick, id, "SAY YOUR WISH FOR THE CITY. NONE OF YOUR OWN IS SPENT.".into()));
    }

    /// Knight `id`'s words for the city, on the scroll as its readings.
    pub fn offer_city_wish(&mut self, id: u32, words: &str) -> Result<(), String> {
        let words = words.trim();
        if words.is_empty() || words.chars().count() > 200 {
            return Err("Say a wish for the city in 1 to 200 characters.".into());
        }
        let culture = self.deep.as_ref().and_then(|d| d.village.as_ref()).map(|v| v.culture).ok_or("No city to wish for here.")?;
        let h = self.players.get_mut(&id).filter(|h| h.hp > 0).ok_or("No knight at the wishing place.")?;
        let kept: String = words.chars().take(MOST_WORDS).collect();
        h.offer = Some(genie::Offer {
            said: kept.clone(),
            binding: None,
            great: false,
            opened: self.tick,
            ask: None,
            target: None,
            star: false,
            line: format!("A wish for {}: none of your own spent.", culture.people()),
            choices: read(&kept)
                .into_iter()
                .map(|gift| genie::Choice {
                    take: genie::Take::Said,
                    wish: Some(format!("city:{gift:?}")),
                    steps: 1,
                    head: gift.name().into(),
                    gives: kept.clone(),
                    cost: "For the city".into(),
                    tip: None,
                })
                .collect(),
        });
        Ok(())
    }

    /// Knight `id` picks reading `n`: the city builds it.
    pub(crate) fn choose_city_wish(&mut self, id: u32, n: usize) -> Result<String, String> {
        let offer = self.players.get(&id).and_then(|h| h.offer.clone()).ok_or("No wish is open on the scroll.")?;
        let choice = offer.choices.get(n).ok_or("No such choice.")?;
        let gift = Gift::ALL
            .into_iter()
            .find(|g| choice.wish.as_deref() == Some(format!("city:{g:?}").as_str()))
            .ok_or("That is no wish for a city.")?;
        let mut deep = self.deep.take().ok_or("No city to wish for here.")?;
        let result = match deep.village.as_mut() {
            Some(v) => self.build_wish(v, id, &offer.said, gift),
            None => Err("No city to wish for here.".into()),
        };
        self.deep = Some(deep);
        if let Some(h) = self.players.get_mut(&id) {
            h.offer = None;
        }
        result
    }

    /// The city builds `gift` for knight `id`, in its next free place, and
    /// cheers.
    fn build_wish(&mut self, v: &mut Village, id: u32, words: &str, gift: Gift) -> Result<String, String> {
        v.wishing.remove(&id);
        if self.wished_here(v, id) {
            return Err("Your wish stands here already.".into());
        }
        let h = self.players.get(&id).ok_or("No knight at the wishing place.")?;
        let (key, shown) = (super::super::talents::knight_key(h), h.name.clone());
        let town = self.home.deep.town_mut(v.floor).ok_or("The realm has no record of this city.")?;
        let Some(slot) = (0..SLOTS as u8).find(|s| !town.wishes.iter().any(|w| w.slot == *s)) else {
            return Err("Every place in the city holds a wish already.".into());
        };
        town.wishes.push(CityWish { knight: key, shown: shown.clone(), words: words.chars().take(MOST_WORDS).collect(), gift, slot });
        v.raised.push((slot, self.tick));
        v.cheer = self.tick + 3 * u64::from(HZ);
        self.sounds.push("wish_grant");
        self.cues.push("deep_city_wish".into());
        self.shake = self.shake.max(6);
        let said = format!("{} WILL KEEP IT: {}, FOR {}.", v.culture.people(), gift.name(), shown.to_uppercase());
        self.found = Some((self.tick, 0, said.clone()));
        self.raise_hope(v, 2);
        Ok(said)
    }

    /// Knight `id` rolls the city's scroll up: nothing is built.
    pub(crate) fn drop_city_wish(&mut self, id: u32) {
        if let Some(v) = self.deep.as_mut().and_then(|d| d.village.as_mut()) {
            v.wishing.remove(&id);
        }
    }
}
