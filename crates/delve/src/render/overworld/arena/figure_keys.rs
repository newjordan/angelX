//! Named figure keys (docs/LADDER.md, "Keys"). A figure stamped from a
//! source the game knows (the quick rig's knight: its design, clip, frame,
//! seat and facing) carries a name beside its look's pixel hash
//! (`room_scene::look_key`), so an HD pack can draw it by what it is:
//! `knight/realm/slash/3/seat2/l`. A recolour, a flip, a pose or a new
//! weapon changes the hash and orphans a drawing made for it; the name
//! stays put, and what changed is said in the name's parts or its dress.
//!
//! - **key**: `<kind>/<design>/<clip>/<frame>/<colours>/<facing>`. Kind is
//!   `knight` today (foes and props come later, the same way). Design is the
//!   rig's (`design_of`: a seat's knight, `quick_rig::Design::name`, as
//!   `great-helm`; `realm` for the old knight every design falls back to).
//!   Clip and frame are the
//!   rig's sample (`quick_rig::Clip`, the frame's index in it). Colours are
//!   `seat<N>` for a seat's own (`knights::realm_colours`), or `colours-<hash>`
//!   for a company's. Facing is `r` as the rig is drawn, `l` turned over.
//! - **dress**: what the figure carries over its body, as stamped: the held
//!   arm (`bow`, `sword`, `axe`, ...), its shield and the way it faces
//!   (`shield-across`, `shield-up-toward`, ...), a hat. A pack entry may ask
//!   for one dress; one that asks for none draws the figure whatever it
//!   carries (a portrait still).
//! - **pivot**: the figure's feet in its own picture's pixels (the rig's
//!   pivot as stamped), so a drawing of any size lands on them.
//! - **rung**: the finest rung the figure may be drawn at
//!   (`rules::resolution`).
//! - **bare**: the rig's body alone as it was stamped, before its shield,
//!   arm and hat, in the figure's own picture: a drawing of the body alone
//!   (`covers: body` in a pack) takes the place of these pixels and leaves
//!   what the knight carries in the game's inks over it.
//!
//! What is not the rig's own frame is never named: a worn wish or a tint
//! over it, a body wished whole, a frog, a statue, a rolled or tossed or
//! struck figure bent from its frame. Those keep their pixel hash, and a
//! pack that has no drawing for that hash leaves their inks.

use std::sync::Arc;

use super::super::ink::Img;
use super::quick_rig::{self, Clip, Rig};
use crate::rules::resolution::Rung;

/// A figure's name, as it was stamped (`room_scene::Body::named`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub key: Arc<str>,
    pub dress: Arc<str>,
    pub pivot: [i32; 2],
    pub rung: Rung,
    pub bare: Option<Arc<Img>>,
}

impl Named {
    /// The same figure turned over left to right in a picture `width` wide:
    /// its facing the other way, its feet mirrored (`x = w - 1 - x`).
    pub fn mirrored(&self, width: i32) -> Self {
        let key = match self.key.rsplit_once('/') {
            Some((base, "r")) => format!("{base}/l"),
            Some((base, "l")) => format!("{base}/r"),
            _ => self.key.to_string(),
        };
        Self {
            key: key.into(),
            pivot: [width - 1 - self.pivot[0], self.pivot[1]],
            bare: self.bare.as_ref().map(|b| Arc::new(b.flip_h())),
            ..self.clone()
        }
    }

}

/// A clip's name, as the rig's JSON and a key say it.
pub fn clip_name(clip: Clip) -> &'static str {
    match clip {
        Clip::Idle => "idle",
        Clip::Walk => "walk",
        Clip::Run => "run",
        Clip::Roll => "roll",
        Clip::Hurt => "hurt",
        Clip::Die => "die",
        Clip::Cast => "cast",
        Clip::Slash => "slash",
        Clip::Overhead => "overhead",
        Clip::Thrust => "thrust",
        Clip::Flail => "flail",
        Clip::Bow => "bow",
        Clip::Gun => "gun",
    }
}

/// The design a knight's rig is, for its key: a seat's knight by its
/// design's name (`quick_rig::Design::name`), the old knight (`knight.json`,
/// which a design that fails to read falls back to) as `realm`. A rig the
/// game does not know by name (a wished one) has none, and its figures are
/// not named.
pub fn design_of(rig: &Rig) -> Option<&'static str> {
    if quick_rig::knight().is_some_and(|k| std::ptr::eq(k, rig)) {
        return Some("realm");
    }
    quick_rig::Design::ALL
        .into_iter()
        .find(|&d| quick_rig::design(d).is_some_and(|r| std::ptr::eq(r, rig)))
        .map(quick_rig::Design::name)
}

/// The colours part of a key: a seat's own, or a company's by a hash of
/// its pairs.
pub fn colours_part(seat: u32, company: &[(char, char)]) -> String {
    if company.is_empty() {
        return format!("seat{seat}");
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &(a, b) in company {
        for c in [a, b] {
            hash ^= u64::from(u32::from(c));
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("colours-{:08x}", hash as u32)
}

/// A knight's key, facing as the rig draws it (`r`).
pub fn knight_key(design: &str, clip: Clip, frame: usize, colours: &str) -> String {
    format!("knight/{design}/{}/{frame}/{colours}/r", clip_name(clip))
}

/// The parts of a key, if it is one: `(kind, design, clip, frame, colours,
/// facing)`.
pub fn parts(key: &str) -> Option<[&str; 6]> {
    let mut it = key.split('/');
    let out = [it.next()?, it.next()?, it.next()?, it.next()?, it.next()?, it.next()?];
    (it.next().is_none() && out.iter().all(|p| !p.is_empty()) && matches!(out[5], "r" | "l")).then_some(out)
}

/// Whether `key` is a well-formed figure key (what a pack entry may name).
pub fn is_key(key: &str) -> bool {
    parts(key).is_some_and(|p| {
        p.iter().all(|s| s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'))
    })
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__figure_keys__tests.rs"]
mod tests;
