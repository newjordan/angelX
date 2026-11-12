//! The camp as a person lays it out: the streets first, then rows of works along them.
//!
//! The hillside south and east of the Keep is the camp's ground: the Keep's wall is reserved, the
//! woods run along the south, the field stone lies by the camp. The scripted player plans it once:
//!
//! - a **trunk** road east from the camp's doorstep along `TRUNK_Y`, a **spine** north up `SPINE_X`
//!   and **cross streets** off the spine, each a straight run so a cart's way from one work to
//!   another is about the tiles between them;
//! - **rows** of works along each street, a tile apart, doors to the street. Two rows face each other
//!   across a one-tile **lane** (the street's block is street, row, lane, row, street), and a swape
//!   standing in the lane lifts from a work on one side to the work on the other;
//! - the stockyard by the camp end of the trunk, so the goods the camp makes cross the least ground.
//!
//! Nothing here changes the game: it is only where the player chooses to put things, and the streets
//! are laid with `Command::Road` like anybody's.
use crate::*;

/// The trunk road, east from the camp's doorstep.
pub(crate) const TRUNK_Y: i16 = 96;
/// Where the trunk starts: just under the camp's own tile, (140, 95).
pub(crate) const TRUNK_X0: i16 = 140;
/// The spine, north from the trunk, and the cross streets off it.
pub(crate) const SPINE_X: i16 = 147;
pub(crate) const CROSS: [i16; 3] = [90, 84, 78];
/// The southern spine, and the two streets off it that the food block stands on.
pub(crate) const SPINE2_X: i16 = 158;
pub(crate) const SOUTH: [i16; 2] = [102, 108];
/// The farthest east a street runs (the model houses' camps begin at 172 and 174).
pub(crate) const EAST: i16 = 170;

/// A row of works along a street, filled from the west.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub name: &'static str,
    /// The street it faces, and whether it stands north of it.
    pub street: i16,
    pub north: bool,
    pub x0: i16,
    pub x1: i16,
    /// The next tile a work may start on.
    pub cursor: i16,
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub rows: Vec<Row>,
}

impl Layout {
    pub fn new() -> Self {
        let row = |name, street, north, x0, x1| Row { name, street, north, x0, x1, cursor: x0 };
        let mut l = Self {
            rows: vec![
                // Beside the camp: the single row north of the trunk under the Keep's wall (one tile
                // deep) and the first of the south side.
                row("N96W", TRUNK_Y, true, 141, 146),
                row("S96W", TRUNK_Y, false, 136, 146),
                // The houses, along the trunk's western end.
                row("HN", TRUNK_Y, true, 121, 132),
                row("HS", TRUNK_Y, false, 126, 134),
                // The east: the south side of the trunk (the wood's edge to x 159, then the wood's trades), the
                // north side, the first cross street split into the stone's near end and the rest.
                row("S96E", TRUNK_Y, false, SPINE_X + 1, EAST),
                row("N96L", TRUNK_Y, true, SPINE_X + 1, 163),
                row("N96", TRUNK_Y, true, 165, EAST),
                row("S90S", CROSS[0], false, SPINE_X + 1, 159),
                row("S90", CROSS[0], false, 161, EAST),
                row("N90", CROSS[0], true, SPINE_X + 1, EAST),
                row("S84", CROSS[1], false, SPINE_X + 1, EAST),
                row("N84", CROSS[1], true, SPINE_X + 1, EAST),
                row("S78", CROSS[2], false, SPINE_X + 1, EAST),
                row("N78", CROSS[2], true, SPINE_X + 1, EAST),
                // The food block, south-east of the camp on the open ground past the wood.
                row("N102", SOUTH[0], true, SPINE2_X + 1, EAST + 2),
                row("S102", SOUTH[0], false, SPINE2_X + 1, EAST + 2),
                row("N108", SOUTH[1], true, SPINE2_X + 1, EAST + 2),
                row("S108", SOUTH[1], false, SPINE2_X + 1, EAST + 2),
            ],
        };
        // The yard, its swape and the second gang stand along the north side of the trunk, the
        // first gang across the lane from it.
        for r in &mut l.rows {
            match r.name {
                "N96L" => r.cursor = GANGS[1].0 .0 + 4,
                "S90S" => r.cursor = GANGS[0].0 .0 + 4,
                _ => {}
            }
        }
        l
    }

    pub fn row(&self, name: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.name == name)
    }

    /// The street tiles a work on `row` at `x..x+w` needs laid, from the camp: the trunk to the spine (or
    /// as far as the work), up the spine to a cross street, along it.
    pub fn street_to(&self, row: &Row, x: i16, w: i16) -> Vec<Tile> {
        if x + w <= TRUNK_X0 {
            // A row west of the camp's end of the trunk: the trunk runs out to it.
            return (x - 1..=TRUNK_X0).map(|tx| (tx, TRUNK_Y)).collect();
        }
        let far = (x + w).min(EAST + 2);
        let (to, spine) = if SOUTH.contains(&row.street) {
            (SPINE2_X, SPINE2_X)
        } else if row.street == TRUNK_Y {
            (far.max(TRUNK_X0), SPINE_X)
        } else {
            (SPINE_X, SPINE_X)
        };
        let mut tiles: Vec<Tile> = (TRUNK_X0..=to).map(|tx| (tx, TRUNK_Y)).collect();
        if SOUTH.contains(&row.street) {
            tiles.extend((TRUNK_Y + 1..=row.street).map(|y| (spine, y)));
            tiles.extend((spine + 1..=far).map(|tx| (tx, row.street)));
        } else if row.street != TRUNK_Y {
            tiles.extend((row.street..TRUNK_Y).rev().map(|y| (spine, y)));
            tiles.extend((SPINE_X + 1..=far).map(|tx| (tx, row.street)));
        }
        tiles
    }

    /// Where a work of `w` by `h` tiles would stand on `row` at column `x`: its top-left tile.
    pub fn top_left(row: &Row, x: i16, w: i16, h: i16) -> Tile {
        let _ = w;
        (x, if row.north { row.street - h } else { row.street + 1 })
    }
}

/// Whether a tile is on one of the planned streets, laid or not: no work is raised on it.
pub(crate) fn is_street((x, y): Tile) -> bool {
    (y == TRUNK_Y && (119..=EAST).contains(&x))
        || (x == SPINE_X && (CROSS[2]..=TRUNK_Y).contains(&y))
        || (CROSS.contains(&y) && (SPINE_X..=EAST).contains(&x))
        || (x == SPINE2_X && (TRUNK_Y..=SOUTH[1]).contains(&y))
        || (SOUTH.contains(&y) && (SPINE2_X..=EAST + 2).contains(&x))
}

/// Where the stockyard stands: the camp end of the trunk, across the lane from the first ditch gang.
pub(crate) const YARD: Tile = (148, 94);
/// The ditch gangs by the yard and the swape that lifts each one's spoil into it: `(gang, swape, way the swape faces)`.
/// The first gang stands across the lane (the swape in the lane), the second beside the yard (the swape between).
pub(crate) const GANGS: [(Tile, Tile, crate::rollers::Dir); 2] =
    [((148, 91), (149, 93), crate::rollers::Dir::S), ((152, 94), (151, 94), crate::rollers::Dir::W)];

/// The rows a kind of work is raised in, in the order tried. The timber chain stands along the south of the
/// trunk by the wood; the lodges and the stone by the stockyard; the food chain in a block south-east of the
/// camp; the rest on the cross streets beyond.
pub(crate) fn zone(plan: &str) -> &'static [&'static str] {
    match plan {
        "woodsman-camp" | "clamp" => &["S96W", "S96E"],
        "sawpit" | "carpentry" | "wheelwright" | "depot" => &["S96E", "S96W"],
        "lodge" | "hod-gang" => &["N96L", "S90S"],
        "field-quarry" | "mason" => &["S90S", "S90"],
        "farmstead" | "pasture" | "mill" | "bakery" | "well" | "brewery" | "tanyard" => &["N102", "S102", "N108", "S108"],
        "hut" | "cottage" => &["HN", "HS"],
        "smithy" | "forge-shop" | "bloomery" | "lime-kiln" | "slaking-pit" | "mortar-mill" | "sand-pit" | "tracing-floor" => &["S90", "N96", "S84", "N90"],
        "banker" => &["N90", "S84", "N84", "S78"],
        _ => &["N84", "S78", "N78", "N90", "S84", "S90", "N96"],
    }
}
