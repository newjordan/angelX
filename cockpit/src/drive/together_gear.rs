//! What a seat sends its host besides its keys (the ballot, the activity
//! menu and a floor's plate are keys too, in the Delve's own commands): a
//! wish from the phrasebook, a wish for the scroll to learn, a reforge, a
//! reforged card its own angelX drafted, a card, forged runes, a wish for
//! the realm. Each is one engine action (`ToServer::Action`,
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

/// A known wish (the phrasebook's id), for the open Overclass window: `{wish}`.
pub(crate) const BOON: u32 = 1;
/// A wish the phrasebook doesn't know: the host's angelX teaches it: `{words}`.
pub(crate) const LEARN: u32 = 2;
/// Reforge one part of the seat's kit, in their words, in a Sanctuary:
/// `{part, words}`.
pub(crate) const REFORGE: u32 = 3;
/// A reforged card the seat's own angelX drafted: `{part, card}`.
pub(crate) const REFORGE_CARD: u32 = 4;
/// A card for the book: its file's text.
pub(crate) const CARD: u32 = 5;
/// Forged runes for the seat's weapon: the file's text.
pub(crate) const FORGE: u32 = 6;
/// A wish for the realm, in the seat's own words: plain text.
pub(crate) const WISH: u32 = 7;
/// Raise a drafted wish the treasury can pay for: its id.
pub(crate) const GRANT: u32 = 8;
/// One piece of a larger action: its kind, how many pieces, which this is,
/// then its bytes.
pub(crate) const PIECE: u32 = 100;

/// The bytes a piece carries: the action's framing and the piece's header
/// still fit in a client message.
const PIECE_BYTES: usize = 900;
/// The most an action may be once whole: a card with its sprites as JSON.
pub(crate) const MAX_WHOLE: usize = cards::MAX_FILE_BYTES * 2 + 1024;
/// A seat sends at most one piece this often: 50 a second, beside its 30
/// commands, inside the 120 messages a second a socket may send.
pub(crate) const PIECE_EVERY: Duration = Duration::from_millis(20);

/// What a seat's action asks of the host, checked.
#[derive(Clone, Debug)]
pub(crate) enum Gear {
    Boon(String),
    Learn(String),
    Reforge(Part, String),
    /// Checked here and again by the host's run before it is equipped.
    ReforgeCard(Part, Card),
    Card(Card),
    Weapon(super::together_forge::Weapon),
    Wish(String),
    Grant(String),
}

/// The messages that carry one action: itself when it fits in one,
/// else its pieces, in order.
pub(crate) fn pieces(kind: u32, body: &[u8]) -> Vec<(u32, Vec<u8>)> {
    if body.len() <= PIECE_BYTES {
        return vec![(kind, body.to_vec())];
    }
    let chunks: Vec<&[u8]> = body.chunks(PIECE_BYTES).collect();
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = Vec::with_capacity(chunk.len() + 12);
            put_varint(&mut out, u64::from(kind));
            put_varint(&mut out, chunks.len() as u64);
            put_varint(&mut out, i as u64);
            out.extend_from_slice(chunk);
            (PIECE, out)
        })
        .collect()
}

/// One seat's action coming in pieces.
struct Upload {
    kind: u32,
    count: usize,
    bytes: Vec<u8>,
    next: usize,
}

/// Each seat's action coming in pieces, put back together.
#[derive(Default)]
pub(crate) struct Uploads {
    seats: BTreeMap<u32, Upload>,
}

impl Uploads {
    /// An action from `seat`: the whole action, once it is (at once for
    /// one that came whole). A piece out of order, an action grown past
    /// `MAX_WHOLE`, or a malformed piece drops what the seat had sent.
    pub(crate) fn take(&mut self, seat: u32, kind: u32, body: &[u8]) -> Option<(u32, Vec<u8>)> {
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

    /// The seat changed hands: what it was sending is dropped.
    pub(crate) fn clear(&mut self, seat: u32) {
        self.seats.remove(&seat);
    }
}

/// A piece's header (the whole's kind, how many pieces, which this is)
/// and its bytes.
fn piece(body: &[u8]) -> Option<(u32, usize, usize, &[u8])> {
    let mut b = Bytes::new(body);
    let kind = u32::try_from(b.varint().ok()?).ok()?;
    let count = usize::try_from(b.varint().ok()?).ok()?;
    let index = usize::try_from(b.varint().ok()?).ok()?;
    // What is left after the header is the piece's own bytes.
    let mut header = Vec::new();
    put_varint(&mut header, u64::from(kind));
    put_varint(&mut header, count as u64);
    put_varint(&mut header, index as u64);
    let chunk = body.get(header.len()..)?;
    (body.starts_with(&header) && index < count).then_some((kind, count, index, chunk))
}

/// Text a player typed: no control characters, at most `max` of them.
fn words(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

/// What an action from knight `knight` asks for, checked as the host's
/// HTTP routes once checked it, or why not.
pub(crate) fn read(knight: u32, kind: u32, body: &[u8]) -> Result<Gear, String> {
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
            // A friend's card never replaces one of the host's.
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
        _ => Err("no such action".into()),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_gear__tests.rs"]
mod tests;
