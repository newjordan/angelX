//! The tray of icons in a knight's bottom corner: the weapon held, the guard
//! or shield, the bombs, the cards in hand, the spells and the ultimate,
//! each a small pixel picture with its key and how ready it is.
//!
//! The pictures are the cards' own art (every card draws itself in ink rows,
//! or falls back to its kind's glyph) and a few hand-drawn icons for what is
//! not a card: the bombs, a bare sword, the guards' own marks. Rows are ink
//! characters, `.` for empty, at most `SIZE` square. They travel as plain
//! data in the HUD, so the terminal paints them as half-block cells and the
//! browser page paints them on a canvas, from the very same rows.

use super::*;
use serde::{Deserialize, Serialize};

/// The most an icon is, in pixels, each way.
pub const SIZE: usize = 12;

/// What a slot holds, for its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Weapon,
    Guard,
    Bomb,
    Card,
    Spell,
    Ult,
}

/// One icon in the tray.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Icon {
    pub slot: Slot,
    pub key: String,
    pub name: String,
    /// From resting (0) to ready (1): the cooldown sweep.
    pub fill: f32,
    /// How many there are, when that means something (bombs).
    pub count: u32,
    /// The picture: ink rows joined by `/`.
    pub art: String,
    /// The weapon's plate: whole pips of valour, lit along its top.
    #[serde(default)]
    pub pips: u32,
}

impl Icon {
    pub fn rows(&self) -> Vec<&str> {
        self.art.split('/').collect()
    }
}

// ─── the hand-drawn icons ────────────────────────────────────────────────────

const FISTS: &[&str] = &[".tttt..tttt.", "tOttOttOttOt", "tOttOttOttOt", ".tOOt..tOOt.", "..tt....tt..", "..OO....OO.."];

const BOMB: &[&str] = &[
    "......c6....",
    ".....Po.....",
    "....PP......",
    "...uUUUu....",
    "..uVcUUUu...",
    ".uVUuuUUUu..",
    ".uUUuuUUUu..",
    ".uUUUuuUUx..",
    ".xUUUUuxux..",
    "..xUUUuxx...",
    "...xxxxx....",
];

const SWORD: &[&str] = &[
    ".........Hhi",
    "........Hhi.",
    ".......Hhi..",
    "......Hhi...",
    ".....Hhi....",
    ".4..Hhi.....",
    "..4Hhi......",
    "...44.......",
    "..4.a4......",
    ".a...a......",
    "a...........",
];

const BOW: &[&str] = &[
    "..RO........",
    "..hRO.......",
    "..h..R......",
    "..h...R.....",
    "..h...R.....",
    "..h....R....",
    "..h...R.....",
    "..h...R.....",
    "..h..R......",
    "..hRO.......",
    "..RO........",
];

const WAND: &[&str] = &[
    ".......3....",
    "......232...",
    ".....29z....",
    "....P2......",
    "...PO.......",
    "..PO........",
    ".PO.........",
    "PO..........",
];

const SHIELD: &[&str] = &[
    "444444444444",
    "477777777774",
    "477755557774",
    "477555555774",
    "477555555774",
    ".4775555774.",
    ".4777777774.",
    "..47777774..",
    "...477774...",
    "....4774....",
    ".....44.....",
];

const ROLL: &[&str] = &[
    "..hHHh......",
    ".hiiiiH.....",
    "hi44iiiH....",
    "hi4774iH....",
    "hi4774iH.J..",
    ".hi44ih..J..",
    "..hhhh.J.J..",
    "......J...J.",
];

const AXE: &[&str] = &[
    "....HHHH....",
    "...HHHHhh...",
    "..HHHHHhhB..",
    "..HHHHhBB...",
    "...HhhBB....",
    ".....BB.....",
    ".....BB.....",
    "....BB......",
    "....BB......",
    "...BB.......",
];

const WALL: &[&str] = &[
    "............",
    "2.2.2.2.2.2.",
    "3U3U3U3U3U3.",
    "2.2.2.2.2.2.",
    "1U1U1U1U1U1.",
    "S.S.S.S.S.S.",
];

const ULT: &[&str] = &[
    ".....c6.....",
    ".....6@.....",
    "..@..6@..@..",
    "...@666@....",
    "....6@@6....",
    "c6666@@666@.",
    "....@oo@....",
    "...@6oo6@...",
    "..o..PP..o..",
    ".....SS.....",
];

fn opaque(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '@' || c == '$'
}

/// Ink rows fitted into `SIZE` square: left as drawn when they fit, else
/// shrunk by taking the commonest ink of each patch, so a 16 by 16 card
/// still reads at 12.
pub fn fit(rows: &[&str]) -> String {
    fit_to(rows, SIZE)
}

/// `fit` to some other size: the terminal's cells carry smaller icons.
pub fn fit_to(rows: &[&str], size: usize) -> String {
    let grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    let (h, w) = (
        grid.len(),
        grid.iter().map(Vec::len).max().unwrap_or(0),
    );
    if h == 0 || w == 0 {
        return ".".into();
    }
    let at = |x: usize, y: usize| grid[y].get(x).copied().filter(|&c| opaque(c));
    let (ow, oh, scale) = if h <= size && w <= size {
        (w, h, 1.0)
    } else {
        let s = size as f32 / h.max(w) as f32;
        (
            ((w as f32 * s).round() as usize).max(1),
            ((h as f32 * s).round() as usize).max(1),
            s,
        )
    };
    let mut out = Vec::new();
    for y in 0..oh {
        let mut row = String::new();
        for x in 0..ow {
            if scale == 1.0 {
                row.push(at(x, y).unwrap_or('.'));
                continue;
            }
            let (x0, x1) = (
                (x as f32 / scale) as usize,
                (((x + 1) as f32 / scale).ceil() as usize).min(w),
            );
            let (y0, y1) = (
                (y as f32 / scale) as usize,
                (((y + 1) as f32 / scale).ceil() as usize).min(h),
            );
            let mut seen: Vec<(char, u32)> = Vec::new();
            let mut cells = 0;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    cells += 1;
                    if let Some(c) = at(sx, sy) {
                        match seen.iter_mut().find(|(k, _)| *k == c) {
                            Some((_, n)) => *n += 1,
                            None => seen.push((c, 1)),
                        }
                    }
                }
            }
            let total: u32 = seen.iter().map(|s| s.1).sum();
            let best = seen.iter().max_by_key(|s| s.1).map(|s| s.0);
            row.push(match best {
                Some(c) if total * 2 >= cells.max(1) => c,
                _ => '.',
            });
        }
        out.push(row);
    }
    out.join("/")
}

impl Run {
    fn card_icon(&self, id: Option<&str>, fallback: &[&str]) -> String {
        match id.and_then(|id| self.book.get(id)) {
            Some(card) => fit(&card.art_rows()),
            None => fit(fallback),
        }
    }

    /// Knight `id`'s tray, in the order of the bottom corner: weapon, guard,
    /// the spells on their keys, the ultimate, then bombs and the hand.
    pub fn tray(&self, id: u32, keys: &hud::Keys) -> Vec<Icon> {
        let Some(hero) = self.players.get(&id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let weapon_fallback = match &hero.forged {
            None if hero.unarmed() => FISTS,
            Some(w) if w.melee.is_some() && w.bolt.is_none() => SWORD,
            Some(w) if w.bolt.is_some() && w.spread.is_none() => WAND,
            _ => BOW,
        };
        out.push(Icon {
            slot: Slot::Weapon,
            key: keys.fire.into(),
            name: hero
                .forged
                .as_ref()
                .map(|w| w.name.clone())
                .or_else(|| {
                    hero.arm
                        .as_deref()
                        .and_then(|id| self.book.get(id))
                        .map(|c| c.name.clone())
                })
                .map(|name| format!("{name}{}", hud::tier(hero.temper)))
                .unwrap_or_else(|| hero.weapon.name().to_string()),
            fill: 1.0,
            // The weapon's count is how often it was tempered.
            count: u32::from(hero.temper),
            art: self.card_icon(hero.arm.as_deref(), weapon_fallback),
            pips: self.pips(id),
        });
        let (guard_fallback, rest, total) = match hero.guard {
            cards::Guard::Roll => (ROLL, hero.dash_cooldown, ROLL_REARM),
            cards::Guard::Spin => (AXE, hero.dash_cooldown, spin::REARM),
            cards::Guard::Shield => (SHIELD, 0, 1),
            cards::Guard::Wall(_) => (WALL, 0, 1),
        };
        let fill = match hero.guard {
            cards::Guard::Wall(_) => hero.mana / MAX_MANA,
            _ => 1.0 - rest as f32 / total.max(1) as f32,
        };
        out.push(Icon {
            slot: Slot::Guard,
            key: keys.guard.into(),
            name: hero
                .guard_card
                .as_deref()
                .and_then(|id| self.book.get(id))
                .map_or_else(|| "Roll".into(), |c| c.name.clone()),
            fill: fill.clamp(0.0, 1.0),
            count: 0,
            art: self.card_icon(hero.guard_card.as_deref(), guard_fallback),
            pips: 0,
        });
        for (slot, (card, left)) in hero.spells.iter().zip(hero.spell_cooldowns).enumerate() {
            let Some(c) = card.as_deref().and_then(|id| self.book.get(id)) else {
                continue;
            };
            let total = (c.cooldown.max(1) * HZ) as f32;
            out.push(Icon {
                slot: Slot::Spell,
                key: keys.spells[slot].into(),
                name: c.name.clone(),
                fill: (1.0 - left as f32 / total).clamp(0.0, 1.0),
                count: 0,
                art: fit(&c.art_rows()),
                pips: 0,
            });
        }
        out.push(Icon {
            slot: Slot::Ult,
            key: keys.ult.into(),
            name: hero.ult().name().into(),
            fill: (hero.ult_charge as f32 / ults::ULT_FULL as f32).clamp(0.0, 1.0),
            count: 0,
            art: fit(ULT),
            pips: 0,
        });
        out.push(Icon {
            slot: Slot::Bomb,
            key: keys.bomb.into(),
            name: "Bombs".into(),
            fill: 1.0 - hero.bomb_cooldown as f32 / BOMB_REARM as f32,
            count: hero.bombs,
            art: fit(BOMB),
            pips: 0,
        });
        for (slot, card) in hero.hand.iter().take(cards::HAND).enumerate() {
            let Some(c) = self.book.get(card) else {
                continue;
            };
            out.push(Icon {
                slot: Slot::Card,
                // The quaff key drinks the first; the rest wait their turn.
                key: if slot == 0 { keys.quaff } else { "" }.into(),
                name: c.name.clone(),
                fill: 1.0,
                count: 0,
                art: fit(&c.art_rows()),
                pips: 0,
            });
        }
        out
    }
}
