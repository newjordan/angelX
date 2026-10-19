//! The great castles: every castle on the map as one walled exterior.
//!
//! A castle is never a block of a few tiles any more but a curtain wall
//! round a ward, too big to see whole (`docs/REALM-MONOLITH.md`): the
//! sixteen model houses' castles on the March (`super::houses`), the Keep
//! in Castle Town and the Dragon Keep in the Wastes. Each is a rectangle of
//! closed ground the walker goes round, with one gate in its south face
//! and the tile before the gate where its door is entered. The Keep's wall
//! also holds the Rookery and the Chapel, whose doors open in its face
//! beside the gate.
//!
//! [`castles`] is the one list the realm generator builds them from; the
//! walker's closed ground, the roads, the lots, the dressing and the finds
//! all keep off [`footprints`].

use std::sync::OnceLock;

use super::houses::{self, HouseId};
use super::map::Place;

/// Which castle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CastleKind {
    /// A model house's castle on the March.
    House(HouseId),
    /// The Keep in Castle Town: the realm's own castle, its back to the
    /// Kingsriver.
    Keep,
    /// The Dragon Keep in the Wastes: a broken castle in a crater.
    DragonKeep,
}

/// The way a gate opens: the face it is cut in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facing {
    North,
    East,
    South,
    West,
}

/// A great castle as the map holds it. Every rectangle is `(tx, ty, tw,
/// th)` in realm tiles, `ty` growing south.
#[derive(Clone, Debug, PartialEq)]
pub struct Castle {
    pub kind: CastleKind,
    /// What the map calls it: the house's castle, the Keep, the Dragon Keep.
    pub name: String,
    /// Its curtain wall and all it holds: closed ground, never walked.
    pub foot: (i32, i32, i32, i32),
    /// The gate: the tiles of the wall it opens through (`GATE_W` wide, a
    /// tile deep), in the face `facing` names.
    pub gate: (i32, i32, i32, i32),
    pub facing: Facing,
    /// The road tile before the gate: where the house's knight stands at
    /// home and where E goes in.
    pub stand: (i32, i32),
    /// A house's banner inks `(field, charge)` (signal-bank ink letters,
    /// `render::overworld::ink`), once its model has chosen them.
    pub colours: Option<(char, char)>,
    /// Other doors in its face: each place whose building stands inside
    /// the wall, and the tile before its door.
    pub doors: Vec<(Place, (i32, i32))>,
}

impl Castle {
    /// Whether realm tile `(x, y)` lies inside the wall.
    pub fn holds(&self, x: i32, y: i32) -> bool {
        let (tx, ty, tw, th) = self.foot;
        x >= tx && y >= ty && x < tx + tw && y < ty + th
    }

    /// The ground the castle stands on (a terrain byte, `super::map`).
    pub fn ground(&self) -> u8 {
        match self.kind {
            CastleKind::DragonKeep => b'a',
            _ => b'.',
        }
    }

    /// The place it is, for the Keep and the Dragon Keep.
    pub fn place(&self) -> Option<Place> {
        match self.kind {
            CastleKind::Keep => Some(Place::Keep),
            CastleKind::DragonKeep => Some(Place::DragonKeep),
            CastleKind::House(_) => None,
        }
    }
}

/// A gate's width in tiles.
pub const GATE_W: i32 = 3;

/// The Keep's wall: the north side of Castle Town from the North Road to
/// the chunk's east edge, its back on the Kingsriver's bank. The old keep
/// is its gatehouse; the Rookery and the Chapel stand in its face.
pub const KEEP: (i32, i32, i32, i32) = (81, 49, 15, 5);

/// The Dragon Keep's wall: across the crater's north rim, from the
/// chunk's top edge down to the old keep's door.
pub const DRAGON_KEEP: (i32, i32, i32, i32) = (171, 16, 19, 7);

/// Every great castle: the houses' in house order, then the Keep and the
/// Dragon Keep.
pub fn castles() -> &'static [Castle] {
    static ALL: OnceLock<Vec<Castle>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut out: Vec<Castle> = houses::ids()
            .map(|id| {
                let h = houses::get(id);
                build(
                    CastleKind::House(id),
                    h.castle.clone(),
                    houses::footprint(id),
                    houses::stand(id),
                    (!h.placeholder).then_some((h.field, h.charge)),
                )
            })
            .collect();
        out.push(build(CastleKind::Keep, "the Keep".into(), KEEP, Place::Keep.stand_world(), None));
        out.push(build(
            CastleKind::DragonKeep,
            "the Dragon Keep".into(),
            DRAGON_KEEP,
            Place::DragonKeep.stand_world(),
            None,
        ));
        out
    })
}

fn build(
    kind: CastleKind,
    name: String,
    foot: (i32, i32, i32, i32),
    stand: (i32, i32),
    colours: Option<(char, char)>,
) -> Castle {
    let (tx, ty, tw, th) = foot;
    let facing = if stand.1 >= ty + th {
        Facing::South
    } else if stand.1 < ty {
        Facing::North
    } else if stand.0 < tx {
        Facing::West
    } else {
        Facing::East
    };
    let gate = match facing {
        Facing::South => (stand.0 - GATE_W / 2, ty + th - 1, GATE_W, 1),
        Facing::North => (stand.0 - GATE_W / 2, ty, GATE_W, 1),
        Facing::West => (tx, stand.1 - GATE_W / 2, 1, GATE_W),
        Facing::East => (tx + tw - 1, stand.1 - GATE_W / 2, 1, GATE_W),
    };
    let own = match kind {
        CastleKind::Keep => Some(Place::Keep),
        CastleKind::DragonKeep => Some(Place::DragonKeep),
        CastleKind::House(_) => None,
    };
    let doors = Place::ALL
        .into_iter()
        .filter(|&p| Some(p) != own)
        .filter(|p| {
            let (px, py, pw, ph) = p.footprint_world();
            px >= tx && py >= ty && px + pw <= tx + tw && py + ph <= ty + th
        })
        .map(|p| (p, p.stand_world()))
        .collect();
    Castle {
        kind,
        name,
        foot,
        gate,
        facing,
        stand,
        colours,
        doors,
    }
}

/// Every great castle's footprint: closed ground for the walker, kept
/// clear of roads, lots, dressing and finds.
pub fn footprints() -> impl Iterator<Item = (i32, i32, i32, i32)> {
    castles().iter().map(|c| c.foot)
}

/// The castle whose wall holds realm tile `(x, y)`.
pub fn at(x: i32, y: i32) -> Option<&'static Castle> {
    castles().iter().find(|c| c.holds(x, y))
}

/// The great castle of a house.
pub fn of_house(id: HouseId) -> &'static Castle {
    &castles()[usize::from(id.0) % houses::all().len()]
}

#[cfg(test)]
#[path = "../../tests/world/castles__tests.rs"]
mod tests;
