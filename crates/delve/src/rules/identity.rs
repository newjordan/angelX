//! The identity start (the user, 2026-10-20: "When the players start, I
//! think they should start as a shape. square, triangle, circle, etc ...
//! the players have to wish their body in parts"; "the shapes are just the
//! shape, they don't play different"; "the atari floor should be very
//! short btw, like 3 rooms, one room per wish, to decide body, weapon, and
//! ultimate - this is like a dnd type setup where character skills, random
//! attributes etc, are bestowed on the player based on their wishes"; "no
//! random roll. entirely up to the AI granting the wish how things pan
//! out").
//!
//! Experimental, off unless the host sets `ANGEL_DELVE_IDENTITY=1`
//! ([`on`]); the Atari board (`arena::atari`) comes on with it.
//!
//! A knight begins a delve as one plain shape in their seat's colour
//! ([`Shape`]): a square, a triangle, a circle, a diamond, a hexagon or a
//! star. The shape is a look and nothing more: every shape has the same
//! health, speed and arm. What a shape has is the least the game gives:
//! [`BALL`], a small square ball thrown straight, and the dodge roll. No
//! class kit, no spells, no ultimate, no kept gear.
//!
//! The first floor is character creation: three rooms in a line
//! (`layout::identity_floor`), each room a wish, given as the party first
//! walks in, and the scroll asks that room's question ([`Ask`]):
//!
//! 1. **What are you?** The body: a held card worn on the knight, whose
//!    words (health, armour, speed and the rest) are the frame it gives.
//! 2. **What do you fight with?** The weapon: an arm.
//! 3. **What is your ultimate?** A held card naming the ultimate
//!    (`ult <name>`), cast with R.
//!
//! What a wish bestows is decided by whatever grants it: the phrasebook's
//! own answers, or for words of the player's own the host's model, which is
//! asked (`dungeon_reforge`'s brief) to read the words for the character's
//! attributes and skills and to say why on the scroll's line. The card
//! checker and the wish caps bound it as they bound every wish; the game
//! adds no tuning of its own. With no model to ask, the scroll grants its
//! plain answer to the question ([`neutral`]).
//!
//! While a knight is unmade the scroll grants only answers to its open
//! questions, in order, and a super wish waits. A knight with a body, a
//! weapon and an ultimate is whole: the party hears it, and from the
//! second floor on the game is the ordinary one, spells and wishes of
//! power and all. Parts past the body are ordinary wishes.
//!
//! A head wished guards as the helmet does and a body (or shoulders) as the
//! mail (`gear::BARE_HEAD`, `gear::BARE_BODY`).

use super::*;

/// Whether delves begin with the identity start: only with
/// `ANGEL_DELVE_IDENTITY=1` (experimental).
pub fn on() -> bool {
    std::env::var_os("ANGEL_DELVE_IDENTITY").is_some_and(|v| v == "1")
}

/// Whether the identity start's first floor is the streamlined one: a
/// single plain room, nothing in it, every wish given as the party walks
/// in (`layout::identity_room`). On unless `ANGEL_DELVE_ATARI_SIMPLE=0`,
/// which brings back the three rooms. Tests: off unless `set_simple`.
pub fn simple() -> bool {
    #[cfg(not(test))]
    return std::env::var_os("ANGEL_DELVE_ATARI_SIMPLE").is_none_or(|v| v != "0");
    #[cfg(test)]
    SIMPLE.with(|c| c.get())
}

#[cfg(test)]
std::thread_local! {
    static SIMPLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Turn the streamlined first floor on or off for this test's thread.
#[cfg(test)]
pub fn set_simple(on: bool) {
    SIMPLE.with(|c| c.set(on));
}

/// The card of the shape's one arm.
pub const BALL: &str = "the-ball";
/// Parts a knight wishes before they are whole (with a weapon and an
/// ultimate): the body.
pub const MADE_PARTS: usize = 1;
/// The rooms of the first floor, a wish each.
pub const ROOMS: usize = 3;
/// A knight being made wears every part and one more worn look.
pub const MOST_WORN: usize = 7;
/// A shape's health: every shape the same, the hearth's warmth on top.
pub const SHAPE_HP: u32 = 100;
/// The places a part may be wished at, top to toe.
pub const PLACES: [&str; 6] = ["head", "shoulders", "body", "left", "right", "feet"];

/// The shape a knight begins as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Square,
    Triangle,
    Circle,
    Diamond,
    Hexagon,
    Star,
}

impl Shape {
    pub const ALL: [Shape; 6] = [Shape::Square, Shape::Triangle, Shape::Circle, Shape::Diamond, Shape::Hexagon, Shape::Star];

    /// Seat `id`'s shape: one each for the six seats.
    pub fn of_seat(id: u32) -> Shape {
        Shape::ALL[(id.saturating_sub(1) % 6) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Shape::Square => "Square",
            Shape::Triangle => "Triangle",
            Shape::Circle => "Circle",
            Shape::Diamond => "Diamond",
            Shape::Hexagon => "Hexagon",
            Shape::Star => "Star",
        }
    }
}

/// What a wish makes of a knight being made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// A part of the body, at its place.
    Part(&'static str),
    Weapon,
    /// A spell: no question of the identity start asks for one.
    Magic,
    /// A creature that fights for the knight (a spell or play card that
    /// calls one): only the rite of passage asks for it (`rite`).
    Minion,
    Ultimate,
}

/// What `card` makes of a knight: an arm their weapon, a spell their
/// magic, a held or guard card worn on them a part at its place (a piece
/// drawn whole, with no place, is the body).
pub fn piece_of(card: &Card) -> Option<Piece> {
    if card.ult.is_some() {
        return Some(Piece::Ultimate);
    }
    match card.kind {
        cards::Kind::Arm => Some(Piece::Weapon),
        cards::Kind::Spell | cards::Kind::Play if card.imps().is_some() || card.minion().is_some() => Some(Piece::Minion),
        cards::Kind::Spell => Some(Piece::Magic),
        cards::Kind::Hold | cards::Kind::Guard => card
            .looks
            .worn
            .as_ref()
            .map(|s| Piece::Part(s.at.as_deref().and_then(looks::place).unwrap_or("body"))),
        _ => None,
    }
}

/// What a known wish makes of a knight: its first tier's piece.
pub fn piece_of_wish(wish: &phrasebook::Wish) -> Option<Piece> {
    wish.card(0).ok().as_ref().and_then(piece_of)
}

/// One of the scroll's questions to a knight still unmade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ask {
    Body,
    Weapon,
    Ultimate,
}

impl Ask {
    /// The first floor's rooms ask these, one a room.
    pub const ROOMS: [Ask; ROOMS] = [Ask::Body, Ask::Weapon, Ask::Ultimate];

    pub fn question(self) -> &'static str {
        match self {
            Ask::Body => "What are you?",
            Ask::Weapon => "What do you fight with?",
            Ask::Ultimate => "What is your ultimate?",
        }
    }

    /// What an answer looks like, for the scroll's line under it.
    pub fn hint(self) -> &'static str {
        match self {
            Ask::Body => "your body: an ogre, a bird, a knight of iron",
            Ask::Weapon => "a bow, a blade, a gun: anything that hits",
            Ask::Ultimate => "your great power, cast with R",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Ask::Body => "body",
            Ask::Weapon => "weapon",
            Ask::Ultimate => "ultimate",
        }
    }

    /// Whether an entry that answers `answers` (`scrolls::Entry`) answers
    /// this question.
    pub fn accepts(self, answers: Option<Ask>) -> bool {
        answers == Some(self)
    }
}

/// The scroll's own plain answer to `ask`, granted when no model is there
/// to learn a knight's words (the stub route).
pub fn neutral(ask: Ask) -> &'static str {
    match ask {
        Ask::Body => "plain-frame",
        Ask::Weapon => "yew-bow",
        Ask::Ultimate => "pilgrims-arrow",
    }
}

/// How far a knight has wished themselves into being.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Making {
    pub shape: Option<Shape>,
    /// The places wished, top to toe.
    pub parts: Vec<String>,
    pub weapon: bool,
    pub ultimate: bool,
    /// Whole once, whole for good (a spell brewed away unmakes nobody).
    pub made: bool,
    /// The recruit whose ultimate is wished, when the question is theirs
    /// and not the knight's own (`recruit_ults`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recruit: Option<String>,
}

impl Making {
    pub fn made(&self) -> bool {
        self.made || (self.parts.len() >= MADE_PARTS && self.weapon && self.ultimate)
    }

    /// The questions still open, in the scroll's order.
    pub fn asks(&self) -> Vec<Ask> {
        if self.made() {
            return Vec::new();
        }
        let mut asks = Vec::new();
        if self.parts.len() < MADE_PARTS {
            asks.push(Ask::Body);
        }
        if !self.weapon {
            asks.push(Ask::Weapon);
        }
        if !self.ultimate {
            asks.push(Ask::Ultimate);
        }
        asks
    }

    /// The places not yet wished.
    pub fn open_places(&self) -> Vec<&'static str> {
        PLACES.into_iter().filter(|p| !self.parts.iter().any(|q| q == p)).collect()
    }

    /// Whether a wish making `piece` answers the open question: the
    /// questions are asked in order, the body, the weapon, the ultimate.
    pub fn wants(&self, piece: Piece) -> bool {
        match (self.asks().first(), piece) {
            (Some(Ask::Body), Piece::Part(_)) => true,
            (Some(Ask::Weapon), Piece::Weapon) => true,
            (Some(Ask::Ultimate), Piece::Ultimate) => true,
            _ => false,
        }
    }

    /// What an answer looks like, for the scroll's line under the heading.
    pub fn hint(&self) -> &'static str {
        self.asks().first().map_or("", |a| a.hint())
    }

    /// What a refusal opens with, and what the super wish waits for: the
    /// knight's making.
    pub fn first(&self) -> &'static str {
        "First, wish yourself into being."
    }

    /// The scroll's heading for a knight still unmade: the open question.
    pub fn heading(&self) -> Option<String> {
        if let Some(kit) = &self.recruit {
            return Some(super::recruit_ults::question(kit));
        }
        self.asks().first().map(|a| a.question().to_string())
    }

    /// One line for the learner: what this knight is, and the question
    /// their words answer.
    pub fn brief(&self) -> String {
        if let Some(kit) = &self.recruit {
            let (name, about) = knights::knight(kit).map_or((kit.as_str(), ""), |k| (k.name, k.about));
            return format!(
                "the first knight to recruit {name} ({about}), wishing the recruit's ultimate, answering the scroll's question \"{}\" (ultimate)",
                super::recruit_ults::question(kit)
            );
        }
        let shape = self.shape.map_or("shape", Shape::name).to_lowercase();
        let ask = self.asks().first().copied().unwrap_or(Ask::Body);
        format!("a {shape} wishing themselves into being, answering the scroll's question \"{}\" ({})", ask.question(), ask.word())
    }
}

impl Hero {
    /// How far this knight has wished themselves into being, `None` for a
    /// knight who began with a kit.
    pub fn making(&self, book: &cards::Book) -> Option<Making> {
        // The first to recruit a knight wishes the recruit's ultimate
        // before anything else (`recruit_ults`).
        if self.shape.is_none()
            && let Some(kit) = &self.recruit_ult
        {
            return Some(super::recruit_ults::making(kit));
        }
        let shape = self.shape?;
        let mut m = Making { shape: Some(shape), made: self.made, ..Making::default() };
        for id in self.deck.iter().filter(|c| c.starts_with("wish-")) {
            match book.get(id).and_then(piece_of) {
                Some(Piece::Part(place)) if !m.parts.iter().any(|p| p == place) => m.parts.push(place.to_string()),
                Some(Piece::Weapon) => m.weapon = true,
                Some(Piece::Ultimate) => m.ultimate = true,
                _ => {}
            }
        }
        m.parts.sort_by_key(|p| PLACES.iter().position(|q| q == p));
        Some(m)
    }

    /// A knight still wishing themselves into being.
    pub fn unmade(&self, book: &cards::Book) -> bool {
        self.making(book).is_some_and(|m| !m.made())
    }
}

impl Run {
    /// Knight `id` begins as their shape: the ball, the roll, nothing else
    /// of a kit, at a shape's health. A shape that has wished something is
    /// not begun again (a friend taking the seat keeps what was wished);
    /// one dressed in a kit since (the intro's knight) is.
    pub fn unmake(&mut self, id: u32) {
        let ball = self.book.get(BALL).cloned();
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if hero.shape.is_some() && (hero.made || hero.deck.iter().any(|c| c.starts_with("wish-"))) {
            return;
        }
        hero.shape = Some(Shape::of_seat(id));
        hero.made = false;
        hero.owned = gear::Owned::default();
        hero.weapon = Weapon::Fists;
        hero.arm = None;
        hero.forged = None;
        hero.temper = 0;
        if let Some(ball) = ball {
            hero.equip(&ball);
        }
        hero.guard = cards::Guard::Roll;
        hero.guard_card = Some("dodge-roll".into());
        hero.shielding = false;
        hero.hand.clear();
        hero.bombs = 0;
        hero.spells = Default::default();
        hero.spell_cooldowns = [0; cards::SPELL_SLOTS];
        hero.bar = Default::default();
        hero.ult = None;
        hero.ult_charge = 0;
        hero.deck.retain(|c| !c.starts_with("wish-"));
        hero.max_hp = SHAPE_HP + hero.home_hp;
        hero.hp = hero.max_hp;
        hero.rebonus(&self.book);
        hero.dress();
    }

    /// Why knight `id` may not have `wish` now: a knight still unmade is
    /// granted only what makes them.
    pub fn identity_refusal(&self, id: u32, wish: &phrasebook::Wish) -> Option<String> {
        let m = self.players.get(&id)?.making(&self.book)?;
        if m.made() {
            return None;
        }
        if piece_of_wish(wish).is_some_and(|p| m.wants(p)) {
            return None;
        }
        if let Some(kit) = &m.recruit {
            return Some(format!("First, wish the recruit's ultimate. {} {} doesn't answer it.", super::recruit_ults::question(kit), wish.name));
        }
        let ask = m.asks().first().map_or("What are you?", |a| a.question());
        Some(format!("{} {ask} {} doesn't answer it.", m.first(), wish.name))
    }

    /// The first floor's rooms under the identity start: walking into
    /// room `index` for the first time gives every knight that room's wish.
    pub(super) fn identity_room(&mut self, index: usize, first_visit: bool) {
        if !self.identity || !first_visit || self.dungeon.depth != 1 || index >= ROOMS || self.dungeon.open.is_some() {
            return;
        }
        // The streamlined floor's one room gives all three at once, and
        // holds nothing to fight.
        let simple = self.simple_creation();
        if simple {
            self.enemies.clear();
            self.frogs.wild.clear();
        }
        self.give_wishes(if simple { ROOMS as u8 } else { 1 }, 0);
        let ask = Ask::ROOMS[index];
        let line = format!("The room asks: {} Press T.", ask.question());
        if self.notices.len() < 8 {
            self.notices.push(line);
        }
        self.cues.push(format!("identity_room:{}", ask.word()));
    }

    /// Whether the run stands on the streamlined first floor ([`simple`]).
    pub fn simple_creation(&self) -> bool {
        self.identity && self.dungeon.depth == 1 && self.dungeon.open.is_none() && self.dungeon.rooms.len() == 1 && simple()
    }

    /// On the streamlined floor the stairs wait while a standing knight
    /// still holds a wish to make themselves with.
    pub(super) fn creation_holds_stairs(&self) -> bool {
        self.simple_creation()
            && self.players.values().any(|h| !h.absent && h.hp > 0 && h.wishes > 0 && h.unmade(&self.book))
    }

    /// A knight still unmade whose words could not be learned (no model
    /// to ask): the scroll grants its plain answer to their question.
    pub fn grant_neutral(&mut self, id: u32) -> Result<String, String> {
        self.grant_neutral_for(id, "")
    }

    /// [`grant_neutral`](Self::grant_neutral) for a wish said in `words`.
    pub fn grant_neutral_for(&mut self, id: u32, _words: &str) -> Result<String, String> {
        let ask = self
            .players
            .get(&id)
            .and_then(|h| h.making(&self.book))
            .and_then(|m| m.asks().first().copied())
            .ok_or("nothing left to make")?;
        let wish = phrasebook::get(neutral(ask)).ok_or("the scroll has no plain answer")?;
        self.grant(id, &wish)
    }

    /// A wish granted to knight `id` may have made them: a head guards like
    /// a helmet, a body like mail, and the moment they are whole the party
    /// hears it.
    pub(super) fn after_making(&mut self, id: u32) {
        let Some(m) = self.players.get(&id).and_then(|h| h.making(&self.book)) else {
            return;
        };
        let whole = m.made();
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        hero.owned.helmet = m.parts.iter().any(|p| p == "head");
        hero.owned.body = m.parts.iter().any(|p| p == "body" || p == "shoulders");
        hero.owned.shield = hero.owned.shield || m.parts.iter().any(|p| p == "left");

        hero.dress();
        if whole && !hero.made {
            hero.made = true;
            let line = format!(
                "{} is whole: the {} wished into being. Wishes of power are open.",
                hero.shown_name(),
                m.shape.map_or("shape", Shape::name)
            );
            self.found = Some((self.tick, id, line.clone()));
            if self.notices.len() < 8 {
                self.notices.push(line);
            }
            self.cues.push("made".into());
            self.sounds.push("wheel_land");
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/identity__tests.rs"]
mod tests;
