//! What a seat sends its host besides its keys (the ballot, the activity
//! menu and a floor's plate are keys too, in the Delve's own commands): a
//! wish from the phrasebook, a wish for the scroll to learn, a reforge, a
//! reforged card its own angelX drafted, a card, forged runes, a wish for
//! the realm, a card dragged to another place in the hand. Each is one
//! engine action (`ToServer::Action`,
//! `NativeLink::action`) of a kind below.
//!
//! An action is one client message, and a client message is at most 1 KB
//! (`angel_net::proto::MAX_CLIENT_MESSAGE`). A larger one (a card with its
//! sprites) goes in pieces of kind `PIECE`, sent no faster than `PIECE_EVERY`
//! so a seat stays well inside its messages a second; the server puts them
//! back together for each seat (`Uploads`) and hands the whole on.
//!
//! The host checks everything again, as it always did: a piece is bytes
//! until it is whole, and a whole is only what `read` makes of it.

use super::together_shooter::{Card, cards, knights::Part, phrasebook};
use angel_net::codec::{Bytes, put_varint};
use std::collections::BTreeMap;
use std::time::Duration;

pub const BOON: u32 = 1;
pub const LEARN: u32 = 2;
pub const REFORGE: u32 = 3;
pub const REFORGE_CARD: u32 = 4;
pub const CARD: u32 = 5;
pub const FORGE: u32 = 6;
pub const WISH: u32 = 7;
pub const GRANT: u32 = 8;
pub const HAND: u32 = 9;
pub const CREATION: u32 = super::together_shooter::rite::ACTION;
pub const PIECE: u32 = 100;

const PIECE_BYTES: usize = 900;
pub const MAX_WHOLE: usize = cards::MAX_FILE_BYTES * 2 + 1024;
pub const PIECE_EVERY: Duration = Duration::from_millis(20);

#[derive(Clone, Debug)]
pub enum Gear {
    Boon(String),
    Learn(String),
    Reforge(Part, String),
    ReforgeCard(Part, Card),
    Card(Card),
    Weapon(super::together_forge::Weapon),
    Wish(String),
    Grant(String),
    Hand(usize, usize),
    Creation(super::together_shooter::rite::Made),
}

pub fn pieces(kind: u32, body: &[u8]) -> Vec<(u32, Vec<u8>)> {
    angel_net::client::pieces(kind, body)
}

struct Upload {
    kind: u32,
    count: usize,
    bytes: Vec<u8>,
    next: usize,
}

#[derive(Default)]
pub struct Uploads {
    seats: BTreeMap<u32, Upload>,
}

impl Uploads {
    pub fn take(&mut self, seat: u32, kind: u32, body: &[u8]) -> Option<(u32, Vec<u8>)> {
        if kind != PIECE {
            self.seats.remove(&seat);
            return Some((kind, body.to_vec()));
        }
        let Some((whole, count, index, chunk)) = piece(body) else {
            self.seats.remove(&seat);
            return None;
        };
        if index == 0 {
            self.seats.insert(
                seat,
                Upload {
                    kind: whole,
                    count,
                    bytes: Vec::new(),
                    next: 0,
                },
            );
        }
        let upload = self.seats.get_mut(&seat)?;
        if upload.kind != whole
            || upload.count != count
            || upload.next != index
            || count * PIECE_BYTES > MAX_WHOLE + PIECE_BYTES
            || upload.bytes.len() + chunk.len() > MAX_WHOLE
        {
            self.seats.remove(&seat);
            return None;
        }
        upload.bytes.extend_from_slice(chunk);
        upload.next += 1;
        if upload.next < upload.count {
            return None;
        }
        let upload = self.seats.remove(&seat)?;
        Some((upload.kind, upload.bytes))
    }

    pub fn clear(&mut self, seat: u32) {
        self.seats.remove(&seat);
    }
}

fn piece(body: &[u8]) -> Option<(u32, usize, usize, &[u8])> {
    let mut b = Bytes::new(body);
    let kind = u32::try_from(b.varint().ok()?).ok()?;
    let count = usize::try_from(b.varint().ok()?).ok()?;
    let index = usize::try_from(b.varint().ok()?).ok()?;
    let mut header = Vec::new();
    put_varint(&mut header, u64::from(kind));
    put_varint(&mut header, count as u64);
    put_varint(&mut header, index as u64);
    let chunk = body.get(header.len()..)?;
    (body.starts_with(&header) && index < count).then_some((kind, count, index, chunk))
}

fn words(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn read(knight: u32, kind: u32, body: &[u8]) -> Result<Gear, String> {
    let json =
        || serde_json::from_slice::<serde_json::Value>(body).map_err(|_| "send JSON".to_string());
    let field = |v: &serde_json::Value, name: &str| {
        v[name]
            .as_str()
            .map(str::to_string)
            .ok_or(format!("send {{{name}}}"))
    };
    let text = || std::str::from_utf8(body).map_err(|_| "send UTF-8 text".to_string());
    match kind {
        BOON => {
            let wish = field(&json()?, "wish")?;
            phrasebook::get(&wish).ok_or("the phrasebook has no such wish")?;
            Ok(Gear::Boon(wish))
        }
        LEARN => {
            let said = words(&field(&json()?, "words")?, 160);
            if said.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
                return Err("say the wish in a few words".into());
            }
            Ok(Gear::Learn(said))
        }
        REFORGE => {
            let ask = json()?;
            let part =
                Part::from_word(&field(&ask, "part")?).ok_or("part is offense or defense")?;
            let said = words(&field(&ask, "words")?, 200);
            if said.chars().filter(|c| c.is_alphanumeric()).count() < 3 {
                return Err("say what you want in a few words".into());
            }
            Ok(Gear::Reforge(part, said))
        }
        REFORGE_CARD => {
            let forged = json()?;
            let part =
                Part::from_word(&field(&forged, "part")?).ok_or("part is offense or defense")?;
            let mut checked = cards::check("", &field(&forged, "card")?).map_err(|e| e.fix_it())?;
            checked.card.id = format!("p{knight}-{}", checked.card.id);
            Ok(Gear::ReforgeCard(part, checked.card))
        }
        CARD => {
            let mut checked = cards::check("", text()?).map_err(|e| e.fix_it())?;
            checked.card.id = format!("p{knight}-{}", checked.card.id);
            Ok(Gear::Card(checked.card))
        }
        FORGE => {
            let checked = super::together_forge::check(text()?, super::together_forge::START_LOC)
                .map_err(|e| e.fix_it())?;
            Ok(Gear::Weapon(checked.weapon))
        }
        WISH | GRANT => {
            let said = words(text()?, super::together_realm::MAX_WORDS);
            if said.is_empty() {
                return Err("say what you wish for".into());
            }
            Ok(if kind == WISH {
                Gear::Wish(said)
            } else {
                Gear::Grant(said)
            })
        }
        HAND => {
            let moved = json()?;
            let slot = |name: &str| {
                moved[name]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|&n| n < cards::HAND)
                    .ok_or(format!("send {{{name}}}: a slot in the hand"))
            };
            Ok(Gear::Hand(slot("from")?, slot("to")?))
        }
        CREATION => {
            let made: super::together_shooter::rite::Made =
                serde_json::from_slice(body).map_err(|e| format!("a Creator piece: {e}"))?;
            made.validate()?;
            Ok(Gear::Creation(made))
        }
        _ => Err("no such action".into()),
    }
}

#[cfg(test)]
#[path = "../tests/together_gear__tests.rs"]
mod tests;
