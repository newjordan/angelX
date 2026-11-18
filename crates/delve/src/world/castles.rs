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
//!
//! That is the full plan. In the blank-slate plan (`super::plan`) there is
//! one castle, the Keep, a tiny one just begun, and the sixteen houses'
//! sites are camps ([`Castle::is_camp`]): two tents, a fire and a banner, no
//! wall, where each house's castle will be built. The Dragon Keep is not on
//! the map.

use std::sync::OnceLock;

use super::houses::{self, HouseId};
use super::map::Place;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CastleKind {
    House(HouseId),
    Keep,
    DragonKeep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facing {
    North,
    East,
    South,
    West,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Castle {
    pub kind: CastleKind,
    pub name: String,
    pub foot: (i32, i32, i32, i32),
    pub gate: (i32, i32, i32, i32),
    pub facing: Facing,
    pub stand: (i32, i32),
    pub colours: Option<(char, char)>,
    pub doors: Vec<(Place, (i32, i32))>,
}

impl Castle {
    pub fn holds(&self, x: i32, y: i32) -> bool {
        let (tx, ty, tw, th) = self.foot;
        x >= tx && y >= ty && x < tx + tw && y < ty + th
    }

    pub fn ground(&self) -> u8 {
        match self.kind {
            CastleKind::DragonKeep => b'a',
            _ => b'.',
        }
    }

    pub fn is_camp(&self) -> bool {
        matches!(self.kind, CastleKind::House(_)) && super::plan::tabula()
    }

    pub fn is_tiny(&self) -> bool {
        self.kind == CastleKind::Keep && super::plan::tabula()
    }

    pub fn place(&self) -> Option<Place> {
        match self.kind {
            CastleKind::Keep => Some(Place::Keep),
            CastleKind::DragonKeep => Some(Place::DragonKeep),
            CastleKind::House(_) => None,
        }
    }
}

pub const GATE_W: i32 = 3;

pub const KEEP: (i32, i32, i32, i32) = (81, 49, 15, 5);

pub const DRAGON_KEEP: (i32, i32, i32, i32) = (171, 16, 19, 7);

pub fn keep_foot() -> (i32, i32, i32, i32) {
    if super::plan::tabula() { super::plan::tabula::KEEP } else { KEEP }
}

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
        out.push(build(CastleKind::Keep, "the Keep".into(), keep_foot(), Place::Keep.stand_world(), None));
        if super::plan::full() {
            out.push(build(
                CastleKind::DragonKeep,
                "the Dragon Keep".into(),
                DRAGON_KEEP,
                Place::DragonKeep.stand_world(),
                None,
            ));
        }
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
        .filter(|&p| Some(p) != own && p.on_map())
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

pub fn footprints() -> impl Iterator<Item = (i32, i32, i32, i32)> {
    castles().iter().map(|c| c.foot)
}

pub fn at(x: i32, y: i32) -> Option<&'static Castle> {
    castles().iter().find(|c| c.holds(x, y))
}

pub fn of_house(id: HouseId) -> &'static Castle {
    &castles()[usize::from(id.0) % houses::all().len()]
}

#[cfg(test)]
#[path = "../../tests/world/castles__tests.rs"]
mod tests;
