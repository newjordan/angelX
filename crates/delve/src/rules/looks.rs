//! Looks: what a card shows on the play field, not only on its face.
//!
//! A card may carry sprites for five slots, each drawn in realm inks:
//!
//! - `worn`: over the knight, centred on their sixteen-pixel body (a bigger
//!   shield, spiked pauldrons, a different helm); up to 24 by 24, so it may
//!   stand four pixels proud of the figure on every side.
//! - `shot`: the knight's shots and the spells they cast, drawn at twice
//!   size like every shot; up to 12 by 12.
//! - `orbit`: what spins round the knight in place of the morningstar; up to
//!   12 by 12.
//! - `summon`: the creatures a `summon` effect calls; up to 16 by 16.
//! - `aura`: a ground glow under the knight's feet; up to 32 by 24.
//!
//! A sprite is one to four frames. In a card file it is a `draw <slot>`
//! block: rows of inks, the frames side by side with `|` between, the way a
//! sprite sheet lays them out. A PNG sheet (`sheet <slot> <file.png>`) is
//! cut into the same text when the card loads (`together_sheets`), so what
//! travels to a friend, and into a save, is always checked text.
//!
//! Looks are data. They change what is drawn and nothing else.

use super::cards::{Book, Card};
use super::{Hero, Projectile};
use serde::{Deserialize, Serialize};

/// A sprite has at most this many frames.
pub const MAX_FRAMES: usize = 4;
/// Game ticks a frame shows for (30 ticks a second: five frames a second).
pub const TICKS_PER_FRAME: u64 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Worn,
    Shot,
    Orbit,
    Summon,
    Aura,
}

impl Slot {
    pub const ALL: [Slot; 5] = [
        Slot::Worn,
        Slot::Shot,
        Slot::Orbit,
        Slot::Summon,
        Slot::Aura,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Slot::Worn => "worn",
            Slot::Shot => "shot",
            Slot::Orbit => "orbit",
            Slot::Summon => "summon",
            Slot::Aura => "aura",
        }
    }

    /// The slot a word names (`foe` is the summon slot too).
    pub fn parse(word: &str) -> Option<Slot> {
        Some(match word.to_ascii_lowercase().as_str() {
            "worn" => Slot::Worn,
            "shot" => Slot::Shot,
            "orbit" => Slot::Orbit,
            "summon" | "foe" => Slot::Summon,
            "aura" => Slot::Aura,
            _ => return None,
        })
    }

    /// The largest frame the slot takes, width by height.
    pub fn max_side(self) -> (usize, usize) {
        match self {
            Slot::Worn => (24, 24),
            Slot::Shot | Slot::Orbit => (12, 12),
            Slot::Summon => (16, 16),
            Slot::Aura => (32, 24),
        }
    }

    /// The side a PNG sheet's frame is cut to when the line names none.
    pub fn sheet_side(self) -> u32 {
        match self {
            Slot::Worn => 14,
            Slot::Shot => 8,
            Slot::Orbit => 10,
            Slot::Summon => 12,
            Slot::Aura => 24,
        }
    }

    /// What the slot shows, for the card format and the briefs.
    pub fn says(self) -> &'static str {
        match self {
            Slot::Worn => {
                "drawn over the knight, centred on their 16x16 body (up to 24x24: a bigger shield, spiked pauldrons, a new helm)"
            }
            Slot::Shot => "the knight's shots and spell casts (up to 12x12, drawn at twice size)",
            Slot::Orbit => "what spins round the knight instead of the morningstar (up to 12x12)",
            Slot::Summon => "the creatures a `summon` effect calls (up to 16x16)",
            Slot::Aura => "a glow on the ground under the knight (up to 32x24)",
        }
    }
}

/// One sprite: up to four frames of ink rows (`.` is clear).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub frames: Vec<Vec<String>>,
}

impl Sprite {
    /// The frame showing at `tick`.
    pub fn frame(&self, tick: u64) -> &[String] {
        if self.frames.is_empty() {
            return &[];
        }
        let n = self.frames.len() as u64;
        &self.frames[((tick / TICKS_PER_FRAME) % n) as usize]
    }

    /// The widest and tallest frame.
    #[cfg(any(test, feature = "testing"))]
    pub fn size(&self) -> (usize, usize) {
        let w = self
            .frames
            .iter()
            .flatten()
            .map(|r| r.chars().count())
            .max()
            .unwrap_or(0);
        let h = self.frames.iter().map(Vec::len).max().unwrap_or(0);
        (w, h)
    }

    /// A strip of rows, frames side by side with `|` between, checked for
    /// `slot`: every row splits into the same number of frames, at most
    /// four, each within the slot's size, each with an inked pixel.
    pub fn from_strip(slot: Slot, rows: &[String]) -> Result<Sprite, String> {
        let word = slot.word();
        if rows.is_empty() {
            return Err(format!("draw {word} has no rows"));
        }
        let count = rows[0].split('|').count();
        if count > MAX_FRAMES {
            return Err(format!(
                "draw {word} has {count} frames; at most {MAX_FRAMES}"
            ));
        }
        let mut frames: Vec<Vec<String>> = vec![Vec::new(); count];
        for (i, row) in rows.iter().enumerate() {
            if !super::script::sprite_row(row) {
                return Err(format!(
                    "draw {word} row {}: use realm inks, . and |",
                    i + 1
                ));
            }
            let cells: Vec<&str> = row.split('|').collect();
            if cells.len() != count {
                return Err(format!(
                    "draw {word} row {} has {} frames, the first row {count}; put `|` between the same frames on every row",
                    i + 1,
                    cells.len()
                ));
            }
            for (frame, cell) in frames.iter_mut().zip(cells) {
                frame.push(cell.to_string());
            }
        }
        let (mw, mh) = slot.max_side();
        for (n, frame) in frames.iter().enumerate() {
            let w = frame.iter().map(|r| r.chars().count()).max().unwrap_or(0);
            if w > mw || frame.len() > mh {
                return Err(format!(
                    "draw {word} frame {} is {w}x{}; at most {mw}x{mh}",
                    n + 1,
                    frame.len()
                ));
            }
            if !frame
                .iter()
                .any(|r| r.chars().any(|c| c != '.' && c != ' '))
            {
                return Err(format!("draw {word} frame {} has no inked pixel", n + 1));
            }
        }
        Ok(Sprite { frames })
    }

    /// The sprite as the card block that makes it.
    pub fn to_block(&self, slot: Slot) -> String {
        let h = self.frames.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = self
            .frames
            .iter()
            .map(|f| f.iter().map(|r| r.chars().count()).max().unwrap_or(0))
            .collect();
        let mut out = format!("draw {}\n", slot.word());
        for y in 0..h {
            let row: Vec<String> = self
                .frames
                .iter()
                .zip(&widths)
                .map(|(f, &w)| {
                    let r = f.get(y).map_or("", String::as_str);
                    format!("{r:.<w$}")
                })
                .collect();
            out.push_str(&row.join("|"));
            out.push('\n');
        }
        out
    }
}

/// A card's looks on the field, slot by slot, and the forged colour and
/// shape (`look <colour> <shape>`) its shots take when it draws no shot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Looks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worn: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orbit: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summon: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aura: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<(String, String)>,
}

impl Looks {
    pub fn is_empty(&self) -> bool {
        *self == Looks::default()
    }

    pub fn get(&self, slot: Slot) -> Option<&Sprite> {
        match slot {
            Slot::Worn => self.worn.as_ref(),
            Slot::Shot => self.shot.as_ref(),
            Slot::Orbit => self.orbit.as_ref(),
            Slot::Summon => self.summon.as_ref(),
            Slot::Aura => self.aura.as_ref(),
        }
    }

    pub fn slot_mut(&mut self, slot: Slot) -> &mut Option<Sprite> {
        match slot {
            Slot::Worn => &mut self.worn,
            Slot::Shot => &mut self.shot,
            Slot::Orbit => &mut self.orbit,
            Slot::Summon => &mut self.summon,
            Slot::Aura => &mut self.aura,
        }
    }

    /// Whether the card's shots look like anything of their own.
    pub fn dresses_shots(&self) -> bool {
        self.shot.is_some() || self.style.is_some()
    }
}

/// The places a worn piece may be put on the knight, and where its centre
/// lands on the 24 by 24 worn canvas (the 16-pixel body in its middle).
pub const PLACES: &str =
    "head, shoulders, body, left (the shield arm), right (the weapon hand), feet";

fn anchor(at: &str) -> Option<(i32, i32)> {
    Some(match at {
        "head" | "helm" => (12, 7),
        "shoulders" | "back" => (12, 11),
        "body" | "chest" | "centre" | "center" => (12, 14),
        "left" | "shield" => (6, 13),
        "right" | "hand" | "weapon" => (17, 14),
        "feet" | "legs" => (12, 19),
        _ => return None,
    })
}

/// Put a worn piece's frames on the knight's 24 by 24 worn canvas, each
/// centred at the place named.
pub fn place_on_body(frames: &[Vec<String>], at: &str) -> Result<Vec<Vec<String>>, String> {
    let (cx, cy) = anchor(at).ok_or_else(|| format!("at=`{at}`: use {PLACES}"))?;
    Ok(frames
        .iter()
        .map(|frame| {
            let mut canvas = vec![vec!['.'; 24]; 24];
            let h = frame.len() as i32;
            let w = frame.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
            let (x0, y0) = (cx - w / 2, cy - h / 2);
            for (y, row) in frame.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    let (px, py) = (x0 + x as i32, y0 + y as i32);
                    if c != '.' && (0..24).contains(&px) && (0..24).contains(&py) {
                        canvas[py as usize][px as usize] = c;
                    }
                }
            }
            canvas
                .into_iter()
                .map(|r| r.into_iter().collect())
                .collect()
        })
        .collect())
}

/// The format of the five slots, a line each, for the card rules and the
/// briefs.
pub fn slots() -> String {
    Slot::ALL
        .iter()
        .map(|s| format!("  draw {:<6} {}", s.word(), s.says()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A shot dressed by a card's sprite carries `("card", <card id>)` as its
/// `look`, beside the forged `(colour, shape)` looks.
pub const CARD_LOOK: &str = "card";

impl Projectile {
    /// The look a shot takes from `card`: its `draw shot`, or its
    /// `look <colour> <shape>`.
    pub fn look_of(card: &Card) -> Option<(String, String)> {
        if card.looks.shot.is_some() {
            Some((CARD_LOOK.to_string(), card.id.clone()))
        } else {
            card.looks.style.clone()
        }
    }

    /// The card whose `draw shot` this shot wears, if any.
    pub fn sprite_card(&self) -> Option<&str> {
        match &self.look {
            Some((kind, id)) if kind == CARD_LOOK => Some(id),
            _ => None,
        }
    }
}

impl Hero {
    /// What the knight holds, oldest first: held cards, then the weapon's
    /// card and the guard's.
    fn worn_cards<'a>(&self, book: &'a Book) -> impl Iterator<Item = &'a Card> {
        self.deck
            .iter()
            .filter_map(|id| book.get(id))
            // A wish's deck marker also records spell/arm/guard tiers;
            // only held cards dress the knight through the deck itself.
            .filter(|c| c.kind == super::cards::Kind::Hold)
            .chain(self.arm.as_deref().and_then(|id| book.get(id)))
            .chain(self.guard_card.as_deref().and_then(|id| book.get(id)))
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// The look the knight's shots take from what they hold: the newest
    /// held card that dresses shots, else the weapon's card.
    pub fn shot_look(&self, book: &Book) -> Option<(String, String)> {
        self.deck
            .iter()
            .rev()
            .filter_map(|id| book.get(id))
            .find(|c| c.kind == super::cards::Kind::Hold && c.looks.dresses_shots())
            .or_else(|| {
                self.arm
                    .as_deref()
                    .and_then(|id| book.get(id))
                    .filter(|c| c.looks.dresses_shots())
            })
            .and_then(Projectile::look_of)
    }

    /// The sprites the knight wears, oldest under newest: at most three.
    pub fn worn<'a>(&self, book: &'a Book) -> Vec<&'a Sprite> {
        let mut seen = Vec::new();
        let all: Vec<&Sprite> = self
            .worn_cards(book)
            .filter_map(|c| {
                let sprite = c.looks.worn.as_ref()?;
                if seen.contains(&c.id) {
                    return None;
                }
                seen.push(c.id.clone());
                Some(sprite)
            })
            .collect();
        all[all.len().saturating_sub(3)..].to_vec()
    }

    /// The newest sprite of `slot` the knight holds (orbit, aura).
    pub fn newest<'a>(&self, book: &'a Book, slot: Slot) -> Option<&'a Sprite> {
        self.worn_cards(book)
            .filter_map(|c| c.looks.get(slot))
            .last()
    }
}

/// The big spiked pavise the brief's example draws, a piece put on the
/// shield arm: a fixture for the tests (and the cockpit's).
#[cfg(any(test, feature = "testing"))]
pub const PAVISE: &str = "\
..W..W..W..
..H..H..H..
.jJJJJJJJj.
jJiiiiihhJj
WJiihhhhhJW
jJih4@4hhJj
HJih@6@hhJH
WJih4@4hhJW
jJihhhhhhJj
.jJihhhhJj.
.jJhhhhhJj.
..jJhhhJj..
...jJhJj...";

#[cfg(test)]
#[path = "../../tests/rules/together_looks__tests.rs"]
mod tests;
