//! The realm's plan: which upper world the map is.
//!
//! The kingdom map (`super::map`, 240 x 176 tiles) can be laid out two ways,
//! and this module is the one switch between them:
//!
//! - [`Plan::Full`], the old realm: eleven biomes, the fourteen authored
//!   set pieces, about thirty-seven countryside places, roads, fingerposts,
//!   halts, finds and dressing, the Keep and the Dragon Keep as great walled
//!   castles, and the sixteen model houses' castles on the March
//!   (`docs/KINGDOM-OPEN.md`, `docs/REALM-MONOLITH.md`);
//! - [`Plan::Tabula`], a blank slate (the operator, 2026-11-08: "one tiny
//!   castle, started, for the main castle, and then surrounding hillsides
//!   with nothing, but a camp, where the other models castles should be
//!   built"): one open grass country, the Keep a tiny castle just begun in
//!   the middle of it, a camp for each model house on a loose ring of
//!   hillside round it, and the way down to the Delve beside the Keep. (The
//!   Keep is now the Works' site: the great tower, raised stone by stone,
//!   with the Delve's gate and the spawn inside its bailey.)
//!
//! **To bring the full old realm back, change [`PLAN`] to [`Plan::Full`].**
//! Nothing is deleted: every authored screen, region, road, room and castle
//! is still in the code, and the plan only decides what the map holds. Each
//! module that decides what stands on the map asks [`active`] (or
//! [`tabula`]/[`full`]) once, at the edge: `map::Realm`, `biome`, `roads`,
//! `wild`, `houses` (where the castles stand), `castles`, `doors`,
//! `kingdom::geography`, `wishes` (the Delve's gate), `college`, `signs`,
//! `finds`, `dressing`, `waystations` and the scene's staging
//! (`render::overworld::scene`).
//!
//! The Tabula plan's numbers live here, in [`tabula`], as plain pure
//! functions of nothing, so a test can check the plan's facts without
//! standing the global map up (`tests/world/plan__tests.rs`).
//!
//! A test build of this crate keeps the Full plan unless the environment says
//! `ANGEL_REALM_PLAN=tabula` (the rooms' and systems' tests assert the old
//! layout; the Tabula checks build their plan directly), and so does a crate
//! whose tests turn on `full-realm-tests` (realm-host's mini view). Other
//! builds with the `testing` feature (the dev host) follow [`PLAN`] and may
//! say `ANGEL_REALM_PLAN=full` to stand the old realm up.

#[cfg(any(test, feature = "testing"))]
use std::sync::OnceLock;

/// Which upper world the map is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plan {
    /// The old realm, whole.
    Full,
    /// One tiny castle, a camp for each model house, open hillside.
    Tabula,
}

/// THE SWITCH. `Plan::Full` brings the full old realm back.
pub const PLAN: Plan = Plan::Tabula;

/// The plan the map is laid out by.
#[cfg(not(any(test, feature = "testing")))]
pub const fn active() -> Plan {
    PLAN
}

/// The plan the map is laid out by: [`PLAN`], the old one in this crate's own
/// tests, or what `ANGEL_REALM_PLAN` (`tabula` or `full`) says.
#[cfg(any(test, feature = "testing"))]
pub fn active() -> Plan {
    static ACTIVE: OnceLock<Plan> = OnceLock::new();
    *ACTIVE.get_or_init(|| {
        match std::env::var("ANGEL_REALM_PLAN").ok().as_deref().map(str::trim) {
            Some(v) if v.eq_ignore_ascii_case("tabula") => Plan::Tabula,
            Some(v) if v.eq_ignore_ascii_case("full") => Plan::Full,
            _ if cfg!(test) || cfg!(feature = "full-realm-tests") => Plan::Full,
            _ => PLAN,
        }
    })
}

/// Whether the blank-slate plan is the one standing.
pub fn tabula() -> bool {
    active() == Plan::Tabula
}

/// Whether the full old realm is the one standing.
pub fn full() -> bool {
    active() == Plan::Full
}

/// A rectangle in realm tiles `(tx, ty, tw, th)`.
pub type Rect = (i32, i32, i32, i32);

/// The Tabula plan, as plain data and pure functions.
pub mod tabula {
    use super::Rect;
    use crate::world::map::{CHUNK_H, CHUNK_W, MAP_H, MAP_W};

    /// The one biome the whole country is: the frontier, "The March".
    pub const BIOME: crate::world::biome::Biome = crate::world::biome::Biome::March;

    /// The Keep's site: the great tower raised stone by stone (`world::works`,
    /// docs: the Works), its ditch and all, `28 x 28` tiles on tile corner
    /// `works::ORIGIN`. The static map holds the whole of it closed (nothing
    /// is placed, no one is sent across it); what a knight may walk on inside
    /// is the site's (`rules::site`): the bailey, the gate's causeway, the
    /// Delve's gate and the spawn open, the ditch, the walls and the keep
    /// shut as they are dug and laid. Over chunks (4, 4), (5, 4), (4, 5) and
    /// (5, 5).
    pub const KEEP: Rect = (119, 67, 28, 28);

    /// The spawn and the walkers' start: in the bailey, in the lee of the
    /// gate. The castle is raised over the way down, which is the story.
    pub const KEEP_STAND: (i32, i32) = (133, 89);

    /// The Delve's gate, a stair into the hillside beside the Keep.
    pub const DELVE_GATE: Rect = (139, 84, 3, 3);

    /// A camp's ground: two tents, a fire and a banner pole, `7 x 4` tiles,
    /// with the knight's stand a tile below its middle and the house's name
    /// carved on the ground under that.
    pub const CAMP_W: i32 = 7;
    pub const CAMP_H: i32 = 4;

    /// The ring of camps: one chunk each of the sixteen round the middle one
    /// (the ring of chunks two out from the Keep's), clockwise from the north,
    /// each camp set by hand at its own spot in its chunk so the ring is
    /// loose, never a grid; the camps are 29 to 59 tiles from the Keep (five
    /// to nine seconds on foot) and at least 15 from each other. House
    /// `i` of `houses::ids()` camps at `CAMPS[i]`.
    pub const CAMPS: [Rect; 16] = [
        (128, 54, CAMP_W, CAMP_H),
        (149, 52, CAMP_W, CAMP_H),
        (172, 56, CAMP_W, CAMP_H),
        (181, 70, CAMP_W, CAMP_H),
        (174, 86, CAMP_W, CAMP_H),
        (181, 100, CAMP_W, CAMP_H),
        (173, 117, CAMP_W, CAMP_H),
        (155, 119, CAMP_W, CAMP_H),
        (127, 115, CAMP_W, CAMP_H),
        (109, 118, CAMP_W, CAMP_H),
        (78, 116, CAMP_W, CAMP_H),
        (76, 101, CAMP_W, CAMP_H),
        (84, 87, CAMP_W, CAMP_H),
        (82, 70, CAMP_W, CAMP_H),
        (77, 55, CAMP_W, CAMP_H),
        (108, 52, CAMP_W, CAMP_H),
    ];

    /// The tile a camp's knight stands on at home: a tile below its middle.
    pub const fn camp_stand(camp: Rect) -> (i32, i32) {
        (camp.0 + camp.2 / 2, camp.1 + camp.3)
    }

    /// The Delve gate's stand: the tile before its mouth.
    pub const fn delve_stand() -> (i32, i32) {
        (DELVE_GATE.0 + DELVE_GATE.2 / 2, DELVE_GATE.1 + DELVE_GATE.3)
    }

    /// The terrain of the whole realm: open meadow, edge to edge.
    pub fn terrain() -> Vec<u8> {
        vec![b'.'; (MAP_W * MAP_H) as usize]
    }

    /// Every rectangle of closed ground: the Keep, each camp and the Delve's
    /// gate. Everything else is open.
    pub fn closed() -> Vec<Rect> {
        let mut out = vec![KEEP, DELVE_GATE];
        out.extend(CAMPS);
        out
    }

    /// Whether `(x, y)` is a tile a knight may walk on in this plan: on the
    /// map, one tile in from its rim, and not on closed ground. The Keep's
    /// site counts as open here (its walls and ditch are the Works', laid as
    /// the castle rises: `rules::site`, tested against every state of the
    /// build in `world::works`).
    pub fn walkable(x: i32, y: i32) -> bool {
        let site = |&r: &Rect| r == KEEP;
        x > 0
            && y > 0
            && x < MAP_W - 1
            && y < MAP_H - 1
            && !closed().iter().filter(|r| !site(r)).any(|&(tx, ty, tw, th)| x >= tx && y >= ty && x < tx + tw && y < ty + th)
    }

    /// The chunk `(cx, cy)` a rectangle lies wholly inside, if it does.
    pub fn chunk_of((tx, ty, tw, th): Rect) -> Option<(i32, i32)> {
        let (cx, cy) = (tx.div_euclid(CHUNK_W), ty.div_euclid(CHUNK_H));
        let inside = (tx + tw - 1).div_euclid(CHUNK_W) == cx && (ty + th - 1).div_euclid(CHUNK_H) == cy;
        inside.then_some((cx, cy))
    }

    /// Whether the knight can walk from the Keep's stand to `to`, over the
    /// open ground of this plan (four ways, as the walkers step).
    pub fn reaches(to: (i32, i32)) -> bool {
        let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
        let mut seen = vec![false; (MAP_W * MAP_H) as usize];
        let mut open = std::collections::VecDeque::from([KEEP_STAND]);
        seen[idx(KEEP_STAND)] = true;
        while let Some((x, y)) = open.pop_front() {
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if walkable(next.0, next.1) && !seen[idx(next)] {
                    seen[idx(next)] = true;
                    open.push_back(next);
                }
            }
        }
        to.0 >= 0 && to.1 >= 0 && to.0 < MAP_W && to.1 < MAP_H && seen[idx(to)]
    }
}

#[cfg(test)]
#[path = "../../tests/world/plan__tests.rs"]
mod tests;
