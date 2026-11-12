use super::*;
use crate::rules::{
    genie::{self, Choice, Offer, Take, Tip},
    identity, phrasebook, cards, minions, overclass,
};
use std::{collections::BTreeSet, sync::Arc};

pub const CONTRACT: &str = "delve.scroll";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Intent {
    Local {
        words: String,
        known: Option<String>,
    },
    Learn {
        words: String,
    },
    Known {
        wish: String,
    },
}
impl Intent {
    pub fn valid(&self) -> bool {
        match self {
            Self::Local { words, known } => {
                words.len() <= 4096
                    && known
                        .as_ref()
                        .is_none_or(|id| !id.is_empty() && id.len() <= 128)
            }
            Self::Learn { words } => words.len() <= 4096,
            Self::Known { wish } => !wish.is_empty() && wish.len() <= 128,
        }
    }
}
/// Metadata needed to write a choice. No executable lines, sprites, media,
/// avatar or whole-world state cross the policy RPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub tiers: Vec<String>,
    pub learned: bool,
    pub answers: Option<identity::Ask>,
}
impl Entry {
    fn capture(wish: &Wish) -> Self {
        let answers = match identity::piece_of_wish(wish) {
            Some(identity::Piece::Part(_)) => Some(identity::Ask::Body),
            Some(identity::Piece::Weapon) => Some(identity::Ask::Weapon),
            Some(identity::Piece::Ultimate) => Some(identity::Ask::Ultimate),
            _ => None,
        };
        Self {
            id: wish.id.clone(),
            name: wish.name.clone(),
            tiers: wish.tiers.iter().map(|(_, text)| text.clone()).collect(),
            learned: wish.learned,
            answers,
        }
    }
    fn valid(&self) -> bool {
        !self.id.is_empty()
            && self.id.len() <= phrasebook::MAX_ID
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !self.name.trim().is_empty()
            && self.name.len() <= 1024
            && (1..=phrasebook::MAX_TIERS).contains(&self.tiers.len())
            && self.tiers.iter().all(|s| s.len() <= 4096)
    }
    fn card_id(&self, tier: usize) -> String {
        format!("wish-{}-{}", self.id, tier + 1)
    }
    fn next_tier(&self, deck: &[String]) -> Option<usize> {
        let next = (0..self.tiers.len())
            .rev()
            .find(|&tier| deck.contains(&self.card_id(tier)))
            .map_or(0, |t| t + 1);
        (next < self.tiers.len()).then_some(next)
    }
    fn fits(&self, making: Option<&identity::Making>) -> bool {
        making.is_none_or(|m| m.asks().first().is_some_and(|ask| ask.accepts(self.answers)))
    }
    /// How far its ladder climbs on a floor whose top tier is `top`
    /// (`overclass::top_tier`): its length, or that floor's reach.
    fn room(&self, top: usize) -> usize {
        self.tiers.len().min(top + 1)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    pub id: String,
    pub name: String,
    pub tier: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub player: u32,
    pub raid: u64,
    pub epoch: u64,
    pub tick: u64,
    pub seed: u64,
    pub hp: u32,
    pub max_hp: u32,
    pub wishes: u8,
    pub great_wishes: u8,
    /// Fairy wishes held (`nooks::gnome`): a wish to spend like any other.
    #[serde(default)]
    pub fairy_wishes: u8,
    /// A shooting star's wish held (`starfall`): spent first, free, one
    /// step and never a super wish.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub star_wish: bool,
    pub deck: Vec<String>,
    /// The floor the knight stands on: a ladder climbs to its fourth and
    /// fifth tiers only deep enough down (`overclass::top_tier`).
    #[serde(default)]
    pub depth: u32,
    pub making: Option<identity::Making>,
    pub summoner: bool,
    pub spells: Vec<Spell>,
    pub genie: genie::Genie,
    pub previous: Option<Offer>,
}
impl Facts {
    pub fn capture(run: &Run, player: u32) -> Result<Self, String> {
        let hero = run
            .players
            .get(&player)
            .ok_or("the scroll's knight no longer exists")?;
        Ok(Self {
            player,
            raid: run.raid_id,
            epoch: run.seat_epoch(player),
            tick: run.tick,
            seed: run.seed,
            hp: hero.hp,
            max_hp: hero.max_hp,
            wishes: hero.wishes,
            great_wishes: hero.great_wishes,
            fairy_wishes: hero.fairy_wishes,
            star_wish: hero.star_wish,
            deck: hero.deck.clone(),
            depth: run.dungeon.depth,
            making: hero.making(&run.book).filter(|m| !m.made()),
            summoner: minions::summoner(hero),
            spells: hero
                .spells
                .iter()
                .flatten()
                .filter_map(|id| run.book.get(id))
                .filter(|card| !card.id.starts_with("wish-"))
                .map(|card| Spell {
                    id: card.id.clone(),
                    name: card.name.clone(),
                    tier: hero.spell_tier(&card.id) as u8,
                })
                .collect(),
            genie: run.genie.clone(),
            previous: hero.offer.as_ref().map(Offer::view),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub facts: Facts,
    pub said: String,
    pub heard: Option<String>,
    pub known: Option<Entry>,
    pub whims: Vec<Entry>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub offer: Offer,
    pub blood_price: u32,
}
pub type ResultValue = Result<Decision, String>;
/// Native-only capture owns resolved definitions. The worker receives Request
/// metadata; accepted IDs resolve exclusively through this immutable capture.
pub struct Capture {
    pub request: Request,
    book: Arc<Vec<Wish>>,
    known: Option<Wish>,
    previous: Option<Binding>,
}
pub struct Prepared {
    request: Request,
    result: ResultValue,
    binding: Option<Binding>,
    previous: Option<Binding>,
}
impl Capture {
    pub fn resolved(
        run: &Run,
        player: u32,
        said: &str,
        known: Option<&Wish>,
        heard: Option<&str>,
        book: Arc<Vec<Wish>>,
    ) -> Result<Self, String> {
        let request = Request {
            facts: Facts::capture(run, player)?,
            said: said.into(),
            heard: heard.map(str::to_owned),
            known: known.map(Entry::capture),
            whims: book
                .iter()
                .filter(|w| !w.learned)
                .map(Entry::capture)
                .collect(),
        };
        request.validate()?;
        Ok(Self {
            request,
            known: known.cloned(),
            book,
            previous: run.players[&player]
                .offer
                .as_ref()
                .and_then(|o| o.binding.clone()),
        })
    }
    pub fn new(run: &Run, player: u32, intent: &Intent) -> Result<Self, String> {
        if !intent.valid() {
            return Err("invalid scroll intention".into());
        }
        let book = phrasebook::book();
        let hero = run
            .players
            .get(&player)
            .ok_or("the scroll's knight no longer exists")?;
        let (said, known, heard) = match intent {
            Intent::Local { words, known } => {
                let known = known
                    .as_ref()
                    .and_then(|id| book.iter().find(|w| w.id == *id));
                let upgrade = words
                    .trim()
                    .get(..8)
                    .is_some_and(|s| s.eq_ignore_ascii_case("upgrade "));
                let said = if upgrade {
                    words.as_str()
                } else {
                    known.map_or(words.as_str(), |w| w.name.as_str())
                };
                (said.to_owned(), known.cloned(), Some(words.clone()))
            }
            Intent::Learn { words } => {
                let known =
                    phrasebook::find_in(&book, words, &hero.deck).filter(|_| !genie::frees(words));
                (words.clone(), known, Some(words.clone()))
            }
            Intent::Known { wish } => {
                let known = book
                    .iter()
                    .find(|w| w.id == *wish)
                    .ok_or("the phrasebook has no such wish")?;
                (known.name.clone(), Some(known.clone()), None)
            }
        };
        Self::resolved(run, player, &said, known.as_ref(), heard.as_deref(), book)
    }
    pub fn prepare(self, run: &Run, result: ResultValue) -> Result<Prepared, String> {
        self.request.validate_result(&result)?;
        if Facts::capture(run, self.request.facts.player)? != self.request.facts {
            return Err("scroll context changed during preparation".into());
        }
        let binding = match &result {
            Ok(decision) => {
                let mut binding = Binding::capture(
                    run,
                    self.request.facts.player,
                    &decision.offer,
                    &self.book,
                    self.known.as_ref(),
                )?;
                binding.blood_price = decision.blood_price;
                Some(binding)
            }
            Err(_) => None,
        };
        Ok(Prepared {
            request: self.request,
            result,
            binding,
            previous: self.previous,
        })
    }
}
impl Prepared {
    pub fn deliver(self, run: &mut Run) -> Result<String, String> {
        let player = self.request.facts.player;
        if Facts::capture(run, player)? != self.request.facts
            || run.players[&player]
                .offer
                .as_ref()
                .and_then(|o| o.binding.as_ref())
                != self.previous.as_ref()
        {
            return Err("prepared scroll no longer owns the observed state".into());
        }
        let mut decision = self.result?;
        decision.offer.binding = self.binding;
        let said = decision.offer.said.clone();
        run.players.get_mut(&player).unwrap().offer = Some(decision.offer);
        run.sounds.push("card_pickup");
        Ok(said)
    }
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        let f = &self.facts;
        let mut seen = BTreeSet::new();
        if !(1..=super::super::MAX_PLAYERS).contains(&f.player)
            || self.said.len() > 4096
            || self.heard.as_ref().is_some_and(|s| s.len() > 4096)
            || f.deck.len() > 1024
            || f.deck.iter().any(|s| s.len() > 128)
            || f.spells.len() > cards::SPELL_SLOTS
            || f.spells
                .iter()
                .any(|s| s.id.len() > 128 || s.name.len() > 1024 || s.tier > 2)
            || f.previous.as_ref().is_some_and(|o| o.binding.is_some())
            || f.previous.as_ref().is_some_and(|o| !super::valid_view(o))
            || self.known.as_ref().is_some_and(|w| !w.valid())
            || self.whims.len() > 512
            || self
                .whims
                .iter()
                .any(|w| !w.valid() || w.learned || !seen.insert(&w.id))
            || !serde_json::to_vec(self).is_ok_and(|s| s.len() <= 1024 * 1024)
        {
            return Err("invalid scroll policy capture".into());
        }
        Ok(())
    }
    pub fn validate_result(&self, result: &ResultValue) -> Result<(), String> {
        let decision = match result {
            Ok(value) => value,
            Err(error) if error.len() <= 4096 => return Ok(()),
            Err(_) => return Err("scroll policy error exceeds bound".into()),
        };
        let offer = &decision.offer;
        let choices = &offer.choices;
        if offer.binding.is_some()
            || !super::valid_view(offer)
            || offer.opened != self.facts.tick
            || decision.blood_price == 0
            || decision.blood_price > self.facts.max_hp.max(1)
            || (offer.great && self.facts.great_wishes == 0)
            || (!offer.great && self.facts.wishes == 0 && self.facts.fairy_wishes == 0 && !self.facts.star_wish)
            || (offer.star && (offer.great || !self.facts.star_wish))
            || self.facts.hp == 0
            || choices.iter().any(|c| match c.wish.as_deref() {
                None => false,
                Some(id) if id.starts_with("spell:") => !self.facts.spells.iter().any(|s| {
                    Some(s.id.as_str()) == id.strip_prefix("spell:") && c.steps == s.tier + 1
                }),
                Some(id) => !self
                    .known
                    .iter()
                    .chain(self.whims.iter())
                    .any(|w| w.id == id),
            })
            || !serde_json::to_vec(result).is_ok_and(|s| s.len() <= 32 * 1024)
        {
            return Err("scroll policy returned an invalid or uncaptured choice".into());
        }
        Ok(())
    }
}

impl Request {
    pub fn evaluate(&self) -> Result<ResultValue, String> {
        self.validate()?;
        let result = self.decide();
        self.validate_result(&result)?;
        Ok(result)
    }
    fn decide(&self) -> Result<Decision, String> {
        if self.facts.hp == 0
            || (self.facts.wishes.saturating_add(self.facts.great_wishes).saturating_add(self.facts.fairy_wishes) == 0
                && !self.facts.star_wish)
        {
            return Err("No wishes held: each floor brings two, and its boss a super wish.".into());
        }
        // A shooting star's wish is said before any of the knight's own:
        // free, one step, never a super one (`starfall`).
        let star = self.facts.star_wish;
        let said = self.said.as_str();
        let wish = self.known.as_ref();
        if let Some(name) = said
            .trim()
            .get(..8)
            .filter(|s| s.eq_ignore_ascii_case("upgrade "))
            .map(|_| said.trim()[8..].trim())
            && let Some(spell) = self
                .facts
                .spells
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
        {
            if spell.tier == 2 {
                return Err(format!("{} is already granted in full", spell.name));
            }
            return Ok(Decision {
                blood_price: (self.facts.max_hp / genie::BLOOD_PRICE).max(1),
                offer: Offer {
                    said: said.trim().into(),
                    binding: None,
                    great: !star
                        && self.facts.great_wishes > 0
                        && (self.facts.making.is_none() || self.facts.wishes == 0),
                    choices: vec![Choice {
                        take: Take::Said,
                        wish: Some(format!("spell:{}", spell.id)),
                        steps: spell.tier + 1,
                        head: "Upgrade spell".into(),
                        gives: format!("{} {}", spell.name, numeral(usize::from(spell.tier) + 1)),
                        cost: if star { "the star's, free".into() } else { "spends one wish".into() },
                        tip: None,
                    }],
                    line: if star { "Starlight in your own magic.".into() } else { "Your own magic grows with you.".into() },
                    opened: self.facts.tick,
                    ask: None,
                    target: None,
                    star,
                },
            });
        }
        let hero = &self.facts;
        // A knight still unmade wishes only themselves, with plain wishes:
        // a super wish waits for the whole knight (`identity`).
        let making = self.facts.making.clone();
        let great = !star && hero.great_wishes > 0 && making.is_none();
        if let Some(m) = &making {
            let ask = m.asks().first().map_or("What are you?", |a| a.question());
            if hero.wishes == 0 && !star {
                return Err(format!("Your super wish waits until you are {}. {ask}", "whole"));
            }
            if let Some(why) = wish.filter(|w| !w.fits(making.as_ref())).map(|w| {
                format!("{} {ask} {} doesn't answer it.", m.first(), w.name)
            }) {
                return Err(why);
            }
            if genie::frees(said) && wish.is_none() {
                return Err(format!("{} {ask}", m.first()));
            }
        }
        // Freeing the genie takes a super wish, star or no star.
        if genie::frees(said) && wish.is_none() && making.is_none() {
            return self.freedom(hero.great_wishes > 0);
        }
        // A summoner's scroll asks whom it is for, his dead first when the
        // words name them (`dead_wishes`).
        let target = self.facts.summoner.then(|| {
            if super::super::dead_wishes::named(said)
                || self
                    .heard
                    .as_deref()
                    .is_some_and(super::super::dead_wishes::named)
            {
                super::super::dead_wishes::Target::Dead
            } else {
                super::super::dead_wishes::Target::You
            }
        });
        let deck = hero.deck.clone();
        let steps = 1 + u8::from(great);
        let roll = super::super::mix(
            self.facts.seed
                ^ u64::from(self.facts.player).rotate_left(29)
                ^ u64::from(self.facts.genie.sealed).wrapping_mul(0x9e37_79b9),
        );
        // A ladder climbs as far as this floor lets it (`overclass::top_tier`):
        // past that, its tier waits for a deeper floor, as the grant says.
        let top = overclass::top_tier(hero.depth);
        let (name, ladder) = match wish {
            Some(w) => {
                let Some(tier) = w.next_tier(&deck) else {
                    return Err(format!("{} is already granted in full", w.name));
                };
                if let Some(why) = overclass::tier_waits(tier, hero.depth) {
                    return Err(format!("{} climbs no higher here: {why}.", w.name));
                }
                (w.name.clone(), Some((w, tier)))
            }
            None => (said.trim().to_string(), None),
        };
        let reach = |tier: usize, steps: u8, room: usize| (tier + usize::from(steps)).min(room) - 1;
        let gives = |steps: u8| match ladder {
            Some((w, tier)) => format!("{} {}", w.name, numeral(reach(tier, steps, w.room(top)))),
            None if steps > 1 => format!("'{name}', {steps} steps when learned"),
            None => format!("'{name}', once learned"),
        };
        let id_of = ladder.map(|(w, _)| w.id.clone());
        let mut choices = vec![Choice {
            take: Take::Said,
            wish: id_of.clone(),
            steps,
            head: "As you said".into(),
            gives: gives(steps),
            cost: if star { "the star's, free".into() } else { String::new() },
            tip: None,
        }];
        // Bolder when the ladder has the step and the floor gives it; the
        // genie's whim when it has not, or now and then when he feels like it.
        let bolder = !star && ladder.is_none_or(|(w, tier)| tier + usize::from(steps) < w.room(top));
        let asked = ladder.map(|(w, _)| w.id.as_str());
        // His whim is a step further when a wish has the step, else as strong.
        let whim = self
            .whim(&deck, asked, steps + 1, roll, making.as_ref(), top)
            .map(|w| (w, steps + 1))
            .or_else(|| {
                self.whim(&deck, asked, steps, roll, making.as_ref(), top)
                    .map(|w| (w, steps))
            });
        let price = (hero.max_hp / genie::BLOOD_PRICE).max(1);
        let whim = whim.map(|(other, more)| {
            let tier = other.next_tier(&deck).unwrap_or(0);
            Choice {
                take: Take::Whim,
                wish: Some(other.id.clone()),
                steps: more,
                head: "The genie's whim".into(),
                gives: format!(
                    "{} {} instead",
                    other.name,
                    numeral(reach(tier, more, other.room(top)))
                ),
                cost: "not what you asked".into(),
                tip: None,
            }
        });
        // A star's wish is one step: the star gives no more for blood, so
        // its scroll offers the words and the genie's whim at that step.
        let whim = if star {
            self.whim(&deck, asked, steps, roll, making.as_ref(), top).map(|other| {
                let tier = other.next_tier(&deck).unwrap_or(0);
                Choice {
                    take: Take::Whim,
                    wish: Some(other.id.clone()),
                    steps,
                    head: "The genie's whim".into(),
                    gives: format!("{} {} instead", other.name, numeral(reach(tier, steps, other.room(top)))),
                    cost: "not what you asked".into(),
                    tip: None,
                }
            })
        } else {
            whim
        };
        let middle = if bolder && (whim.as_ref().is_none_or(|w| w.steps <= steps) || roll % 3 != 0)
        {
            Some(Choice {
                take: Take::Bolder,
                wish: id_of.clone(),
                steps: steps + 1,
                head: "Bolder".into(),
                gives: gives(steps + 1),
                cost: format!("costs {price} health"),
                tip: None,
            })
        } else if great && whim.as_ref().is_none_or(|w| w.steps <= steps) {
            // A super wish with nowhere further to go: one step now, and a
            // plain wish kept for later.
            Some(Choice {
                take: Take::Split,
                wish: id_of.clone(),
                steps: 1,
                head: "Save some".into(),
                gives: format!("{}, and keep a wish", gives(1)),
                cost: "one step, not two".into(),
                tip: None,
            })
        } else {
            whim
        };
        choices.extend(middle);
        if star {
            // Wished upon a star: nothing to bargain with, and nothing owed.
            let lines = [
                "A star heard you! How should it land?",
                "Wish upon a star: it costs you nothing.",
                "Starlight! Say how, friend.",
            ];
            return Ok(Decision {
                blood_price: price,
                offer: Offer {
                    said: name,
                    binding: None,
                    great: false,
                    choices,
                    line: lines[(roll >> 11) as usize % lines.len()].to_string(),
                    opened: self.facts.tick,
                    ask: making.as_ref().and_then(identity::Making::heading),
                    target,
                    star: true,
                },
            });
        }
        let tip = if roll / 3 % 2 == 0 {
            Tip::Purse(30 + 10 * (roll / 7 % 3) as u32)
        } else {
            Tip::Mend(20)
        };
        choices.push(Choice {
            take: Take::Deal,
            wish: id_of,
            steps,
            head: "The genie's deal".into(),
            gives: match tip {
                Tip::Purse(gold) => format!("{} + {gold} gold", gives(steps)),
                Tip::Mend(gold) => format!("{} + mending + {gold} gold", gives(steps)),
            },
            cost: "fills no freedom".into(),
            tip: Some(tip),
        });
        let lines = [
            "Ooh, a good one. How do you want it?",
            "Your wish! Pick how, friend.",
            "I could do it straight... or not.",
            "Psst. Number three is a bargain.",
        ];
        let made = [
            "Ooh, a fresh one! What will you be?",
            "Let's make you. Pick how, friend.",
            "A shape with a wish. My favourite.",
        ];
        let line = match &making {
            Some(_) => made[(roll >> 11) as usize % made.len()],
            None => lines[(roll >> 11) as usize % lines.len()],
        }
        .to_string();
        let ask = making.as_ref().and_then(identity::Making::heading);
        Ok(Decision {
            blood_price: price,
            offer: Offer {
                said: name,
                binding: None,
                great,
                choices,
                line,
                opened: self.facts.tick,
                ask,
                target,
                star: false,
            },
        })
    }
    fn whim(
        &self,
        deck: &[String],
        asked: Option<&str>,
        steps: u8,
        roll: u64,
        making: Option<&identity::Making>,
        top: usize,
    ) -> Option<Entry> {
        let fits: Vec<_> = self
            .whims
            .iter()
            .filter(|w| Some(w.id.as_str()) != asked && w.fits(making))
            .filter(|w| {
                w.next_tier(deck)
                    .is_some_and(|t| t + usize::from(steps) - 1 < w.room(top))
            })
            .collect();
        (!fits.is_empty()).then(|| fits[(roll >> 17) as usize % fits.len()].clone())
    }
    fn freedom(&self, great: bool) -> Result<Decision, String> {
        if self.facts.genie.free() {
            return Err("The genie is already free, and stays as your friend.".into());
        }
        if !self.facts.genie.ready() {
            return Err(format!(
                "The shackle holds: the gauge is at {}/{}. Seal greater wishes to fill it.",
                self.facts.genie.freedom.min(genie::FREEDOM),
                genie::FREEDOM
            ));
        }
        if !great {
            return Err("Only a super wish breaks the shackle: a floor's boss gives one.".into());
        }
        Ok(Decision {
            blood_price: (self.facts.max_hp / genie::BLOOD_PRICE).max(1),
            offer: Offer {
                said: genie::FREE_WORDS.into(),
                binding: None,
                great: true,
                choices: vec![Choice {
                    take: Take::Free,
                    wish: None,
                    steps: 0,
                    head: "Set him free".into(),
                    gives: "the genie stays on as your friend".into(),
                    cost: "spends your super wish".into(),
                    tip: None,
                }],
                line: "You'd... really do that? For me?".into(),
                opened: self.facts.tick,
                ask: None,
                target: None,
                star: false,
            },
        })
    }
}
fn numeral(tier: usize) -> &'static str {
    ["I", "II", "III", "IV", "V"]
        .get(tier)
        .copied()
        .unwrap_or("+")
}
