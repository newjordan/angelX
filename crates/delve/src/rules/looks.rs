//! Looks: what a card shows on the play field, not only on its face.
//!
//! A card may carry sprites for five slots, each drawn in realm inks:
//!
//! - `worn`: over the knight, centred on their sixteen-pixel body (a bigger
//!   shield, spiked pauldrons, a different helm); up to 24 by 24, so it may
//!   stand four pixels proud of the figure on every side.
//! - `shot`: the knight's shots and the spells they cast; up to 16 by 16,
//!   drawn pixel for pixel in a shot's footprint (an 8-pixel shot at twice
//!   size), so finer art keeps its detail.
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
//! A sprite may also be painted: `draw <slot> paint=<sha256>` names a
//! master picture of it in the host's paint store, shown at the top level
//! of a wish where a client has it; the ink rows stay, for the terminal,
//! the lower levels and any client without the picture. `move=` names
//! motion words ([`Move`]) that set the sprite moving, inks and painting
//! alike. A `paint <slot> <file.png>` line is cut like a sheet and stored
//! by its hash, so what travels is still checked text and a hash. A
//! painting that is pixel art says so (`pixel`): a window draws it with
//! hard pixel edges, never smoothed like an illustration.
//!
//! A sprite may colour inks of its own: `pal=` on the `draw` line gives up
//! to sixteen of `script::PAL_INKS` any colour (`pal=%7a2bd1,&b45cff`), the
//! way a 16-bit sprite carried its own palette. The player picks the hue;
//! how bright and saturated it shows follows the wish's power
//! (`fidelity`).
//!
//! Looks are data. They change what is drawn and nothing else.

use super::cards::{Book, Card};
use super::{Hero, Projectile};
use serde::{Deserialize, Serialize};

mod fit;
pub use fit::{profile, Body, Part, fit_size, ink_bounds};
pub(super) use fit::repair;

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
            Slot::Shot => (16, 16),
            Slot::Orbit => (12, 12),
            Slot::Summon => (16, 16),
            Slot::Aura => (32, 24),
        }
    }

    /// The side a PNG sheet's frame is cut to when the line names none.
    pub fn sheet_side(self) -> u32 {
        match self {
            Slot::Worn => 14,
            Slot::Shot => 16,
            Slot::Orbit => 10,
            Slot::Summon => 12,
            Slot::Aura => 24,
        }
    }

    /// What the slot shows, for the card format and the briefs.
    pub fn says(self) -> &'static str {
        match self {
            Slot::Worn => {
                "drawn for a named body part, with transparent margins cropped: head 10x8 (bottom at 8,4), face 8x4, body 12x8, back 16x12, shoulders 16x5, left 8x12, right 8x16, feet 12x5 (bottom at 8,15), aura 32x8; whole body 16x16. Anchors use the unpadded 16x16 knight; other bodies fit the same part with nearest whole pixels"
            }
            Slot::Shot => "the knight's shots and spell casts (up to 16x16, drawn pixel for pixel at a shot's size)",
            Slot::Orbit => "what spins round the knight instead of the morningstar (up to 12x12)",
            Slot::Summon => "the creatures a `summon` effect calls (up to 16x16)",
            Slot::Aura => "a glow on the ground under the knight (up to 32x24)",
        }
    }
}

/// How a sprite moves on the field, a motion word a card names
/// (`move=stretch,bob`). Applied the same way to its inks and its painting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Move {
    /// Squash along the line of flight (for a shot).
    Stretch,
    /// Rise and fall.
    Bob,
    /// Rock side to side.
    Wobble,
    /// Turn round.
    Spin,
    /// A two-lobe squash-bounce.
    Jiggle,
    /// A wave down its length, the trailing end swinging most.
    Wiggle,
    /// Swell and settle.
    Pulse,
    /// A fast small jitter.
    Shake,
}

impl Move {
    pub const ALL: [Move; 8] = [
        Move::Stretch,
        Move::Bob,
        Move::Wobble,
        Move::Spin,
        Move::Jiggle,
        Move::Wiggle,
        Move::Pulse,
        Move::Shake,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Move::Stretch => "stretch",
            Move::Bob => "bob",
            Move::Wobble => "wobble",
            Move::Spin => "spin",
            Move::Jiggle => "jiggle",
            Move::Wiggle => "wiggle",
            Move::Pulse => "pulse",
            Move::Shake => "shake",
        }
    }

    pub fn parse(word: &str) -> Option<Move> {
        let word = word.to_ascii_lowercase();
        Move::ALL.into_iter().find(|m| m.word() == word)
    }

    /// The words of `move=`, comma-separated, each once: unknown ones are
    /// an error, like a bad ink.
    pub fn parse_list(words: &str) -> Result<Vec<Move>, String> {
        let mut moves = Vec::new();
        for word in words.split(',') {
            let Some(m) = Move::parse(word.trim()) else {
                return Err(format!("move `{word}`: use {}", Move::words()));
            };
            if moves.contains(&m) {
                return Err(format!("move `{word}` appears twice"));
            }
            moves.push(m);
        }
        Ok(moves)
    }

    /// Every motion word, for the card format and the errors.
    pub fn words() -> String {
        Move::ALL.map(Move::word).join(", ")
    }
}

/// Whether `sha` names a picture in the paint store: 64 lower-case hex.
pub fn paint_name(sha: &str) -> bool {
    sha.len() == 64 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The square each frame of a picture strip is cut from: frame `f` is the
/// `span` square at (`f * cell + x`, `y`) of the strip, what is beyond its
/// cell clear. It is the box all frames draw together, squared about its
/// middle, so an animation does not jitter; a sheet's inks are this square
/// shrunk, so a painting drawn from it fills the inks' place exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    pub cell: u32,
    pub x: i64,
    pub y: i64,
    pub span: u32,
}

/// The crop of a `width` by `height` strip of `frames` frames, `lit` saying
/// which of its pixels are drawn (alpha at least half). None when nothing
/// is, or the frames do not divide the strip.
pub fn crop(width: u32, height: u32, frames: usize, lit: impl Fn(u32, u32) -> bool) -> Option<Crop> {
    if frames == 0 || width % frames as u32 != 0 {
        return None;
    }
    let cell = width / frames as u32;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for f in 0..frames as u32 {
        for y in 0..height {
            for x in 0..cell {
                if lit(f * cell + x, y) {
                    left = left.min(x);
                    right = right.max(x);
                    top = top.min(y);
                    bottom = bottom.max(y);
                }
            }
        }
    }
    if left > right {
        return None;
    }
    let (bw, bh) = (right - left + 1, bottom - top + 1);
    let span = bw.max(bh);
    Some(Crop {
        cell,
        x: i64::from(left) - i64::from((span - bw) / 2),
        y: i64::from(top) - i64::from((span - bh) / 2),
        span,
    })
}

/// One sprite: up to four frames of ink rows (`.` is clear), the painting
/// that shows in their place where a client has it, and how it moves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub frames: Vec<Vec<String>>,
    /// The master picture's sha256 (hex) in the host's paint store, a strip
    /// of as many frames as the inks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<String>,
    /// The painting's normal map's sha256 (hex) in the same store, a strip
    /// the painting's size (`normal=`): which way each of its pixels faces
    /// in a lit room (docs/WORLD-TIERS.md, "Normal maps"). None: one is made
    /// from the painting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<String>,
    /// A painting that shows in the inks' place only deep enough down
    /// (`hd=<sha256>`, `paintings::HD_FLOOR`): a wish's own painting, at a
    /// grant short of its top, where its inks are that painting downgraded
    /// (`fidelity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hd: Option<String>,
    /// The painting is pixel art (`pixel`): drawn with hard pixel edges at
    /// any size, its pixels never averaged or smoothed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pixel: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<Move>,
    /// Where a worn piece was put on the knight (`at=<place>`, one of
    /// [`PLACES`]): its frames are already on the 24 by 24 canvas there,
    /// and the place says which part of a knight it is (`identity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The sprite's own inks (`pal=`): each a character of
    /// [`script::PAL_INKS`](super::script::PAL_INKS) and any colour the
    /// player chose. How bright and how saturated they show is the wish's
    /// power (`fidelity`), not theirs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pal: Vec<(char, [u8; 3])>,
}

/// A sprite carries at most this many inks of its own.
pub const MAX_PAL: usize = 16;

/// A `pal=` value: `<ink><rrggbb>` entries, `,` between, each ink one of
/// [`script::PAL_INKS`](super::script::PAL_INKS) and named once.
pub fn parse_pal(value: &str) -> Result<Vec<(char, [u8; 3])>, String> {
    let mut pal: Vec<(char, [u8; 3])> = Vec::new();
    for entry in value.split(',').filter(|e| !e.is_empty()) {
        let mut chars = entry.chars();
        let ink = chars.next().unwrap_or(' ');
        let hex = chars.as_str();
        if !super::script::PAL_INKS.contains(ink) {
            return Err(format!(
                "pal=`{entry}` — a sprite's own ink is one of {}",
                super::script::PAL_INKS
            ));
        }
        let rgb = (hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(hex, 16).ok())
            .flatten()
            .map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        let Some(rgb) = rgb else {
            return Err(format!("pal=`{entry}` — an ink then its colour, six hex digits (`%7a2bd1`)"));
        };
        if pal.iter().any(|(c, _)| *c == ink) {
            return Err(format!("pal= names `{ink}` twice"));
        }
        pal.push((ink, rgb));
    }
    if pal.len() > MAX_PAL {
        return Err(format!("pal= has {} inks; at most {MAX_PAL}", pal.len()));
    }
    Ok(pal)
}

/// The `pal=` a `draw` line gives, if any and if it reads.
pub fn header_pal(header: &str) -> Vec<(char, [u8; 3])> {
    header
        .split_whitespace()
        .find_map(|w| w.strip_prefix("pal="))
        .and_then(|v| parse_pal(v).ok())
        .unwrap_or_default()
}

/// `rows` with each of the sprite's own inks as the realm ink nearest its
/// colour: what a reader of realm inks alone (the artistry marks) sees.
pub fn realm_rows(rows: &[String], pal: &[(char, [u8; 3])]) -> Vec<String> {
    if pal.is_empty() {
        return rows.to_vec();
    }
    let ink = crate::ink::ink;
    let near = |rgb: [u8; 3]| {
        super::script::INKS
            .chars()
            .filter_map(|c| ink(c).map(|k| (c, k)))
            .min_by_key(|(_, k)| k.iter().zip(rgb).map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2)).sum::<i32>())
            .map_or('.', |(c, _)| c)
    };
    let swap: Vec<(char, char)> = pal.iter().map(|&(c, rgb)| (c, near(rgb))).collect();
    rows.iter()
        .map(|r| r.chars().map(|c| swap.iter().find(|(k, _)| *k == c).map_or(c, |(_, to)| *to)).collect())
        .collect()
}

/// A `pal=` value for `pal` (empty for none).
pub fn pal_words(pal: &[(char, [u8; 3])]) -> String {
    pal.iter()
        .map(|(c, [r, g, b])| format!("{c}{r:02x}{g:02x}{b:02x}"))
        .collect::<Vec<_>>()
        .join(",")
}

impl Sprite {
    /// The colour of `c` in this sprite: its own ink, or the realm's.
    pub fn rgb(&self, c: char) -> Option<[u8; 3]> {
        self.pal
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, rgb)| *rgb)
            .or_else(|| crate::ink::ink(c))
    }

    /// The first ink of its own the sprite draws with but gives no colour.
    pub fn unnamed_ink(&self) -> Option<char> {
        self.frames
            .iter()
            .flatten()
            .flat_map(|r| r.chars())
            .find(|&c| super::script::PAL_INKS.contains(c) && self.rgb(c).is_none())
    }

    /// The frame showing at `tick`.
    pub fn frame(&self, tick: u64) -> &[String] {
        if self.frames.is_empty() {
            return &[];
        }
        &self.frames[self.frame_index(tick)]
    }

    /// Which frame shows at `tick` (0 for a sprite with none): its
    /// painting shows the same one.
    pub fn frame_index(&self, tick: u64) -> usize {
        let n = self.frames.len().max(1) as u64;
        ((tick / TICKS_PER_FRAME) % n) as usize
    }

    /// The square its painting fills, `(x, y, side)` in its frames' pixels:
    /// the box every frame inks together, squared about its middle, as the
    /// painting's own frames are cut ([`crop`]). None with nothing inked.
    pub fn ink_square(&self) -> Option<(i32, i32, i32)> {
        let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for frame in &self.frames {
            for (y, row) in frame.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    if c != '.' && c != ' ' {
                        left = left.min(x as i32);
                        right = right.max(x as i32);
                        top = top.min(y as i32);
                        bottom = bottom.max(y as i32);
                    }
                }
            }
        }
        if left > right {
            return None;
        }
        let (w, h) = (right - left + 1, bottom - top + 1);
        let side = w.max(h);
        Some((left - (side - w) / 2, top - (side - h) / 2, side))
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
                    "draw {word} row {}: use realm inks, its own (`pal=`), . and |",
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
        Ok(Sprite {
            frames,
            ..Sprite::default()
        })
    }

    /// The header's words after the slot (`paint=`, `normal=`, `pixel`,
    /// `hd=`, `move=`), with a space before them; empty for a sprite in inks
    /// alone.
    pub fn header_words(&self) -> String {
        let mut out = String::new();
        // Already on the canvas at its place: `placed=` keeps the place
        // without putting it there twice.
        if let Some(at) = &self.at {
            out.push_str(&format!(" placed={at}"));
        }
        if let Some(sha) = &self.paint {
            out.push_str(&format!(" paint={sha}"));
            if let Some(normal) = &self.normal {
                out.push_str(&format!(" normal={normal}"));
            }
            if self.pixel {
                out.push_str(" pixel");
            }
        }
        if let Some(sha) = &self.hd {
            out.push_str(&format!(" hd={sha}"));
        }
        if !self.moves.is_empty() {
            let words: Vec<&str> = self.moves.iter().map(|m| m.word()).collect();
            out.push_str(&format!(" move={}", words.join(",")));
        }
        if !self.pal.is_empty() {
            out.push_str(&format!(" pal={}", pal_words(&self.pal)));
        }
        out
    }

    /// The sprite as the card block that makes it.
    pub fn to_block(&self, slot: Slot) -> String {
        let h = self.frames.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = self
            .frames
            .iter()
            .map(|f| f.iter().map(|r| r.chars().count()).max().unwrap_or(0))
            .collect();
        let mut out = format!("draw {}{}\n", slot.word(), self.header_words());
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
    /// How its magic moves (`vfx` lines): left out, the card's own words
    /// give it one (`vfx::generated`).
    #[serde(default, skip_serializing_if = "super::vfx::Recipe::is_empty")]
    pub vfx: super::vfx::Recipe,
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
    "head, face, shoulders, body, back, left (the shield arm), right (the weapon hand), feet, aura, whole";

/// The place a word names, as [`PLACES`] spells it: `helm` is the head,
/// `shield` the left arm.
pub fn place(at: &str) -> Option<&'static str> {
    Some(match at {
        "head" | "helm" => "head",
        "face" | "mask" => "face",
        "shoulders" => "shoulders",
        "back" | "cape" => "back",
        "whole" => "whole",
        "aura" => "aura",
        "body" | "chest" | "centre" | "center" => "body",
        "left" | "shield" => "left",
        "right" | "hand" | "hands" | "weapon" => "right",
        "feet" | "legs" => "feet",
        _ => return None,
    })
}

/// Legacy entries sometimes omitted `at=`. Gear nouns provide a useful
/// repair; an unqualified body or colour keeps its whole-figure meaning.
pub fn default_worn_part(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();
    let words: Vec<_> = name.split(|c: char| !c.is_alphanumeric()).collect();
    if words.iter().any(|w| matches!(*w, "cape" | "cloak" | "mantle")) { "back" }
    else if words.iter().any(|w| matches!(*w, "armor" | "armour" | "cuirass" | "mail")) { "body" }
    else if words.iter().any(|w| matches!(*w, "helmet" | "helm" | "hat" | "head" | "mohawk" | "crown")) { "head" }
    else if words.iter().any(|w| matches!(*w, "boots" | "shoes" | "heels" | "kneepads" | "feet")) { "feet" }
    else if words.iter().any(|w| matches!(*w, "shield" | "buckler")) { "left" }
    else if words.iter().any(|w| matches!(*w, "sword" | "blade" | "axe" | "weapon" | "spear" | "staff")) { "right" }
    else if words.iter().any(|w| matches!(*w, "pauldrons" | "shoulders")) { "shoulders" }
    else { "whole" }
}

/// Where the place `at` names has its middle on the 24 by 24 worn canvas.
pub fn anchor(at: &str) -> Option<(i32, i32)> {
    profile("knight").part(at).map(|p| (p.anchor.0 + 4, p.anchor.1 + 4))
}

/// Fit a local worn drawing to its reference part and retain it on the
/// legacy wire canvas. Empty margins never move its subject or cut it off.
pub fn place_on_body(frames: &[Vec<String>], at: &str) -> Result<Vec<Vec<String>>, String> {
    let part = profile("knight").part(at).ok_or_else(|| format!("at=`{at}`: use {PLACES}"))?;
    let mut sprite = Sprite { frames: frames.to_vec(), ..Default::default() };
    super::artistry::fit_worn(&mut sprite, at)?;
    let [x, y, w, h] = ink_bounds(&sprite.frames).ok_or("the worn piece has no ink")?;
    if w > 24 || h > 24 { return Err(format!("at={at}: the piece is {w}x{h}; at most 24x24")); }
    let (px, py) = part.origin((w, h));
    let (px, py) = ((px + 4).clamp(0, 24 - w), (py + 4).clamp(0, 24 - h));
    Ok(sprite.frames.iter().map(|frame| {
        let mut canvas = vec![vec!['.'; 24]; 24];
        for ty in 0..h { for tx in 0..w {
            canvas[(py + ty) as usize][(px + tx) as usize] = frame.get((y + ty) as usize)
                .and_then(|r| r.chars().nth((x + tx) as usize)).unwrap_or('.');
        }}
        canvas.into_iter().map(|r| r.into_iter().collect()).collect()
    }).collect())
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
            // only held cards dress the knight through the deck itself,
            // and a summoner's for his dead alone dress them instead
            // (`dead_wishes`).
            .filter(|c| c.kind == super::cards::Kind::Hold && self.card_for(&c.id) != super::dead_wishes::Target::Dead)
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
            .find(|c| {
                c.kind == super::cards::Kind::Hold
                    && c.looks.dresses_shots()
                    && self.card_for(&c.id) != super::dead_wishes::Target::Dead
            })
            .or_else(|| {
                self.arm
                    .as_deref()
                    .and_then(|id| book.get(id))
                    .filter(|c| c.looks.dresses_shots())
            })
            .and_then(Projectile::look_of)
    }

    /// The newest wish the knight wears that is only a colour, drawn over
    /// the whole body ([`super::dead_wishes::colours_body`]): its look is
    /// the colour, its name says which.
    pub fn worn_colour<'a>(&self, book: &'a Book) -> Option<&'a Card> {
        self.worn_cards(book).filter(|c| super::dead_wishes::colours_body(c)).last()
    }

    /// The sprites the knight wears, oldest under newest: at most three,
    /// or for a knight wishing themselves into being every part and one
    /// more ([`super::identity::MOST_WORN`]).
    pub fn worn<'a>(&self, book: &'a Book) -> Vec<&'a Sprite> {
        let mut seen = Vec::new();
        let all: Vec<&Sprite> = self
            .worn_cards(book)
            .filter_map(|c| {
                let sprite = c.looks.worn.as_ref()?;
                if super::dead_wishes::colours_body(c) { return None; }
                if seen.contains(&c.id) {
                    return None;
                }
                seen.push(c.id.clone());
                Some(sprite)
            })
            .collect();
        let most = if self.shape.is_some() { super::identity::MOST_WORN } else { 3 };
        let mut worn = all[all.len().saturating_sub(most)..].to_vec();
        // Mirror ink is not held equipment: it lives outside the deck.
        if let Some(sprite) = book.get(&super::cosmetic_wish::key(self.mirror_owner()))
            .and_then(|c| c.looks.worn.as_ref())
        {
            worn = worn[worn.len().saturating_sub(most.saturating_sub(1))..].to_vec();
            worn.push(sprite);
        }
        worn
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
