//! Building where you stand: the Works in the world you walk in.
//!
//! On the blank-slate map the realm's industry is the hillside kingdom
//! (`works_seam`), and the knights build it on the ground they walk, in the
//! flat view and in 3D, not on a desk behind a key. Key 4 puts the builder in
//! hand; the knight keeps walking, and a ghost of the tool in hand stands on
//! the ground just ahead of them with its price and whether the land suits it.
//! Each seat has its own cursor, tool and line in the making (its `Board`);
//! everyone shares the one economy (`Run.home.industry`).
//!
//! Keys in the builder (the desk's, kept where they meant the same):
//! Q or the aim keys change tool, E lays it (hold it while walking to paint
//! road, wall, shaft, rollerway and rail; on a line tool, press at the first
//! building, hold while walking the way it goes and let go at the second), X
//! takes down what is under the ghost (or drops a line half drawn), F pauses
//! it, 1 to 3 take a slot of the hotbar (the up and down arrows another bank
//! of it), `.` changes its recipe (a swape's good, a yard's carts; with the route
//! tool, sends another wagon), `;` links a claim (with the route tool, sells a
//! wagon), `'` turns the Delve supply, P or R turns what is in hand (or the
//! piece of rollerway under the cursor), Space the flow overlay, C opens the
//! ledger (Charters, People, Works, Claims over the world; P turns its pages,
//! C closes it) and 4 puts the builder away.
//!
//! The mouse does the same through [`BuilderAction::Plot`]: a click on the
//! ground sets the cursor there (and lays the tool in hand if it was the left
//! button).

use super::builder_input::BuilderAction;
use super::industry::Board;
use super::*;
use super::builder_rig::{self, Snap};
use crate::kingdom::{Command, Dir, Kingdom, Laid, LineKind, Piece, Placement, Stock, Terrain, Tile, catalog};
use serde::{Deserialize, Serialize};

/// The ledger's pages the world adds to the desk's four (`industry`): the goals,
/// what the party made and used, and what chokes the works. (The desk's own pages
/// of these numbers are not on the hillside's ledger.)
pub const PAGE_GOALS: u8 = 2;
pub const PAGE_PRODUCTION: u8 = 4;
pub const PAGE_CHOKES: u8 = 7;

/// The most tiles a dragged line keeps (the longest line the catalogue allows is far less).
pub const MAX_PATH: usize = 400;

/// A line in the making: where it began and the ground the knight has walked since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub kind: LineKind,
    pub from: u32,
    pub path: Vec<Tile>,
}

/// What the builder has in hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tool {
    Road,
    Plan(&'static str),
    Line(LineKind),
    /// A piece of rollerway, laid a tile at a time (E held while walking paints a run of it).
    Roller(Piece),
    /// Rail, laid a tile at a time like a road; every tile it touches joins it.
    Rail,
    /// A bay: a named place to load and unload, on a rail tile beside a building.
    Bay,
    /// A route: E at the bay it starts from, E at the bay it runs to.
    Route,
    Pattern(&'static str),
    /// A knight's hands on what the Deep has broken: beat out a fire, mend a battered wall, set
    /// a cut line or a burnt works to rights (`Command::Mend`).
    Mend,
}

impl Tool {
    pub fn name(&self) -> String {
        let c = catalog();
        match self {
            Tool::Road => "ROAD".into(),
            Tool::Plan(id) => c.plans[*id].name.to_uppercase(),
            Tool::Line(kind) => c.lines[kind.key()].name.to_uppercase(),
            Tool::Roller(piece) => c.rollers[piece.key()].name.to_uppercase(),
            Tool::Rail => "RAIL".into(),
            Tool::Bay => "BAY".into(),
            Tool::Route => "ROUTE".into(),
            Tool::Mend => "MEND".into(),
            Tool::Pattern(name) => c.patterns.get(*name).map_or_else(|| copy_name(name), |p| p.name.to_uppercase()),
        }
    }
    /// A tool that paints as the knight walks while E is held.
    pub fn brush(&self) -> bool {
        match self {
            Tool::Road => true,
            Tool::Plan(id) => catalog().plans[*id].connector.is_some() || matches!(*id, "wall" | "brick-wall" | "palisade"),
            Tool::Roller(_) | Tool::Rail => true,
            _ => false,
        }
    }
}

/// A kept copy's name on its tool: "COPY A".
fn copy_name(id: &str) -> String {
    format!("COPY {}", id.trim_start_matches("copy-").to_uppercase())
}

/// The plans that stand against the Deep (`deep_raid`): the builder offers them last.
const DEFENCES: [&str; 4] = ["palisade", "wood-gate", "watchtower", "brazier"];

/// Every tool, in the order the builder cycles them: the road, the hillside's
/// works, the lines you lay between them, the patterns you stamp down.
pub fn tools() -> &'static [Tool] {
    static TOOLS: std::sync::OnceLock<Vec<Tool>> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| {
        let c = catalog();
        let mut v = vec![Tool::Road];
        v.extend(c.hillside_menu.iter().filter(|id| !DEFENCES.contains(&id.as_str())).map(|id| Tool::Plan(id.as_str())));
        v.extend([LineKind::Rope, LineKind::Chute, LineKind::Barge].into_iter().filter(|k| c.lines.contains_key(k.key())).map(Tool::Line));
        v.extend(Piece::ALL.into_iter().filter(|p| c.rollers.contains_key(p.key())).map(Tool::Roller));
        if c.lines.contains_key("tram") {
            v.extend([Tool::Rail, Tool::Bay, Tool::Route]);
        }
        v.extend(c.patterns.keys().map(|n| Tool::Pattern(n.as_str())));
        // The copies a party keeps, in the places the realm has for them (`Kingdom::clip`):
        // a tool only while something is kept in its place (`usable`).
        v.extend(crate::kingdom::ghosts::COPY_NAMES.iter().map(|n| Tool::Pattern(*n)));
        // What stands against the Deep, then the hands that mend what it broke: last, so no tool's
        // place (a hotbar keeps places) moves for them.
        v.extend(c.hillside_menu.iter().filter(|id| DEFENCES.contains(&id.as_str())).map(|id| Tool::Plan(id.as_str())));
        v.push(Tool::Mend);
        v
    })
}

/// Whether a tool can be taken in hand in this kingdom: all of them, but a copy whose place
/// holds nothing.
pub fn usable(k: &Kingdom, tool: &Tool) -> bool {
    match tool {
        Tool::Pattern(name) if crate::kingdom::ghosts::COPY_NAMES.contains(name) => k.copies.contains_key(*name),
        _ => true,
    }
}

/// The tool `delta` steps on from `from` that can be taken in hand.
pub fn step_tool(k: &Kingdom, from: usize, delta: i32) -> usize {
    let all = tools();
    let n = all.len() as i32;
    let mut at = from as i32;
    for _ in 0..all.len() {
        at = (at + delta.signum() * delta.abs().max(1)).rem_euclid(n);
        if usable(k, &all[at as usize]) {
            return at as usize;
        }
    }
    from
}

/// The tool a board holds.
pub fn tool_of(board: &Board) -> &'static Tool {
    let all = tools();
    &all[board.plan % all.len()]
}

/// How the land took the ghost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It would be taken as it stands.
    Fits,
    /// The land suits it; the camp lacks the price (or a charter).
    Short,
    /// The land refuses it.
    Refused,
}

/// The ghost of the tool in hand on the ground, for the flat view, the 3D view
/// and the words on the screen.
#[derive(Clone, Debug)]
pub struct Ghost {
    pub tool: Tool,
    /// The tile the cursor is on.
    pub cursor: Tile,
    /// The footprint's top-left corner (the cursor for a road or a line).
    pub anchor: Tile,
    /// The footprint, inclusive: `(x0, y0, x1, y1)`.
    pub rect: (i16, i16, i16, i16),
    pub verdict: Verdict,
    pub cost: Stock,
    /// The tool's name and one line on the land or the price, upper case.
    pub name: String,
    pub note: String,
    /// A line in the making: the ground it has crossed.
    pub path: Vec<Tile>,
    /// The builder is laying plans for the camp's hands, not building now.
    pub planning: bool,
    /// A patch being copied: from its marked corner to the cursor, inclusive.
    pub select: Option<(i16, i16, i16, i16)>,
    /// The way a piece of rollerway or a swape would face (0 north, 1 east, 2 south, 3 west).
    pub dir: u8,
}

const FACES: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// The way a piece of rollerway or a swape would face: the way the knight faces, turned by the
/// quarter turns the builder has been given (the P key).
fn facing(board: &Board) -> Dir {
    Dir::from_index(board.face.wrapping_add(board.turn))
}

/// The tile in front of a knight standing on `feet`, facing `face` (0 north, 1
/// east, 2 south, 3 west).
fn ahead(feet: Tile, face: u8) -> Tile {
    let (dx, dy) = FACES[usize::from(face % 4)];
    (feet.0 + dx, feet.1 + dy)
}

/// Where a footprint `w` by `d` sits for a cursor on `tile`: in front of a
/// knight facing `face` (its near edge's middle on the cursor), or, for the
/// mouse, centred on it.
fn anchor_for(tile: Tile, (w, d): (i16, i16), face: u8, pointed: bool) -> Tile {
    if pointed {
        return (tile.0 - (w - 1) / 2, tile.1 - (d - 1) / 2);
    }
    match face % 4 {
        0 => (tile.0 - (w - 1) / 2, tile.1 - (d - 1)),
        1 => (tile.0, tile.1 - (d - 1) / 2),
        2 => (tile.0 - (w - 1) / 2, tile.1),
        _ => (tile.0 - (w - 1), tile.1 - (d - 1) / 2),
    }
}

/// A plan's footprint in tiles, width and depth, in this kingdom.
fn size_of(k: &Kingdom, plan: &str) -> (i16, i16) {
    let (span, depth) = k.dims(&catalog().plans[plan]);
    (i16::from(span) + 1, i16::from(depth) + 1)
}

/// The building at `tile` or the nearest within two tiles of it.
pub fn building_near(k: &Kingdom, tile: Tile) -> Option<u32> {
    if let Some(id) = k.at(tile) {
        return Some(id);
    }
    let mut best: Option<(i32, u32)> = None;
    for dy in -2i16..=2 {
        for dx in -2i16..=2 {
            if let Some(id) = k.at((tile.0 + dx, tile.1 + dy)) {
                let d = i32::from(dx.abs() + dy.abs());
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, id));
                }
            }
        }
    }
    best.map(|(_, id)| id)
}

fn money(cost: &Stock) -> String {
    let c = catalog();
    cost.iter()
        .map(|(r, n)| format!("{n} {}", c.resources.get(r).map_or(r.as_str(), |x| x.name.as_str()).to_uppercase()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn stamp_cost(k: &Kingdom, name: &str, origin: Tile, turns: u8) -> Result<Stock, String> {
    // A stamp's price is found on a copy with a bottomless treasury: kept for the
    // last answer, since the ghost asks every frame and the same thing every time.
    type Key = (u64, String, Tile, u8);
    static LAST: std::sync::Mutex<Option<(Key, Result<Stock, String>)>> = std::sync::Mutex::new(None);
    let key = (k.rev, name.to_string(), origin, turns);
    if let Ok(last) = LAST.lock()
        && let Some((k0, answer)) = last.as_ref()
        && *k0 == key
    {
        return answer.clone();
    }
    let answer = k.stamp_cost(catalog(), name, origin, turns);
    if let Ok(mut last) = LAST.lock() {
        *last = Some((key, answer.clone()));
    }
    answer
}

/// The ghost of what `board` holds, on `k`'s ground.
pub fn ghost(board: &Board, k: &Kingdom) -> Ghost {
    let c = catalog();
    let tool = tool_of(board).clone();
    let cursor = board.tile;
    let mut g = Ghost {
        name: tool.name(),
        note: String::new(),
        cost: Stock::new(),
        verdict: Verdict::Fits,
        path: board.draft.as_ref().map(|d| d.path.clone()).unwrap_or_default(),
        planning: board.rig.planning,
        select: board.rig.mark.map(|m| (m.0.min(cursor.0), m.1.min(cursor.1), m.0.max(cursor.0), m.1.max(cursor.1))),
        anchor: cursor,
        rect: (cursor.0, cursor.1, cursor.0, cursor.1),
        cursor,
        tool: tool.clone(),
        dir: facing(board).index(),
    };
    match &tool {
        Tool::Road => {
            let river = k.world.terrain(cursor) == Terrain::River;
            g.cost = if river { Stock::from([("plank".to_string(), 4), ("stone".to_string(), 2)]) } else { Stock::from([("timber".to_string(), 1)]) };
            let refused = if k.roads.contains(&cursor) {
                Some("A ROAD IS LAID HERE".to_string())
            } else if k.at(cursor).is_some() {
                Some("A BUILDING STANDS HERE".to_string())
            } else if river {
                None
            } else {
                k.check_road_site(cursor).err().map(|why| why.to_uppercase())
            };
            match refused {
                Some(why) => {
                    g.verdict = Verdict::Refused;
                    g.note = why;
                }
                None => {
                    g.note = if river { format!("A BRIDGE: {}", money(&g.cost)) } else { money(&g.cost) };
                    if g.cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                        g.note += "  (A PLAN: THE CAMP LACKS IT)";
                    }
                }
            }
        }
        Tool::Plan(id) => {
            let (w, d) = size_of(k, id);
            let anchor = anchor_for(cursor, (w, d), board.face, board.pointed);
            g.anchor = anchor;
            g.rect = (anchor.0, anchor.1, anchor.0 + w - 1, anchor.1 + d - 1);
            if c.plans[*id].connector.is_some() || matches!(*id, "wall" | "brick-wall") {
                g.anchor = cursor;
                g.rect = (cursor.0, cursor.1, cursor.0, cursor.1);
            }
            let p: Placement = k.placement(c, id, g.anchor);
            g.cost = p.cost.clone();
            match (&p.land, &p.short) {
                (Err(why), _) => {
                    g.verdict = Verdict::Refused;
                    g.note = why.to_uppercase();
                }
                (Ok(()), Some(short)) => {
                    g.verdict = Verdict::Short;
                    // Short, so the price and the reason fit one line of the builder's strip.
                    let short = short.to_uppercase().replace("RESEARCH ", "NEEDS ").replace(" FIRST", "");
                    g.note = format!("{}  (A PLAN: {})", money(&p.cost), short);
                }
                (Ok(()), None) => {
                    g.note = format!("{}  THE LAND SUITS IT", money(&p.cost));
                    if board.rig.planning {
                        g.verdict = Verdict::Short;
                        g.note = format!("{}  (A PLAN: THE HANDS WILL RAISE IT)", money(&p.cost));
                    }
                }
            }
            if c.plans[*id].swape > 0 {
                g.note += &format!("  FACING {}", facing(board).name().to_uppercase());
            }
        }
        Tool::Roller(piece) => {
            let spec = &c.rollers[piece.key()];
            g.cost = spec.cost.clone();
            let dir = facing(board);
            let same = k.roller_at(cursor).is_some_and(|x| x.piece == *piece && x.dir == dir);
            match k.check_roller_site(c, *piece, cursor) {
                Err(why) => {
                    g.verdict = Verdict::Refused;
                    g.note = why.to_uppercase();
                }
                Ok(()) if same => {
                    g.verdict = Verdict::Refused;
                    g.note = "IT IS LAID SO ALREADY".to_string();
                }
                Ok(()) => {
                    let wants: Vec<&String> = spec.requires.iter().filter(|r| !k.researched.contains(*r)).collect();
                    let price = if g.cost.is_empty() { "A LABOURER A TILE".to_string() } else { money(&g.cost) };
                    g.note = format!("{price}  FACING {}", dir.name().to_uppercase());
                    if !wants.is_empty() {
                        g.verdict = Verdict::Short;
                        g.note += &format!("  (A CHARTER IS WANTED: {})", wants.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join(", "));
                    } else if g.cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                        g.note += "  (THE CAMP LACKS IT)";
                    }
                }
            }
        }
        Tool::Rail => {
            let spec = &c.lines["tram"];
            g.cost = spec.tile_cost.clone();
            let wants: Vec<&String> = spec.requires.iter().filter(|r| !k.researched.contains(*r)).collect();
            match k.check_rail_site(c, cursor) {
                _ if k.rails.cells.contains_key(&cursor) => {
                    g.verdict = Verdict::Refused;
                    g.note = "RAIL IS LAID HERE".into();
                }
                Err(why) => {
                    g.verdict = Verdict::Refused;
                    g.note = why.to_uppercase();
                }
                Ok(()) => {
                    g.note = money(&g.cost);
                    if !wants.is_empty() {
                        g.verdict = Verdict::Short;
                        g.note += &format!("  (A CHARTER IS WANTED: {})", wants.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join(", "));
                    } else if g.cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                        g.note += "  (THE CAMP LACKS IT)";
                    }
                }
            }
        }
        Tool::Bay => {
            let beside = crate::kingdom::Dir::ALL.into_iter().find_map(|d| k.at(d.step(cursor)).filter(|id| c.plans[&k.buildings[id].plan].swape == 0).map(|id| (d, id)));
            match (k.rails.cells.get(&cursor), beside) {
                (None, _) => {
                    g.verdict = Verdict::Refused;
                    g.note = "A BAY STANDS ON A TILE OF RAIL".into();
                }
                (Some(r), _) if r.bay.is_some() => {
                    g.verdict = Verdict::Refused;
                    g.note = format!("THE {} BAY IS HERE", r.bay.clone().unwrap_or_default().to_uppercase());
                }
                (Some(_), None) => {
                    g.verdict = Verdict::Refused;
                    g.note = "A BAY STANDS BESIDE A BUILDING".into();
                }
                (Some(_), Some((_, id))) => g.note = format!("A BAY FOR THE {}: E NAMES IT", c.plans[&k.buildings[&id].plan].name.to_uppercase()),
            }
        }
        Tool::Route => {
            let here = k.rails.cells.get(&cursor).and_then(|r| r.bay.clone());
            match (&board.stop, here) {
                (None, None) => {
                    g.verdict = Verdict::Refused;
                    g.note = "STAND AT A BAY AND PRESS E: WHERE THE WAGONS LOAD".into();
                }
                (None, Some(name)) => g.note = format!("E: THE WAGONS LOAD AT THE {}", name.to_uppercase()),
                (Some(from), None) => {
                    g.verdict = Verdict::Refused;
                    g.note = format!("LOADING AT THE {}: GO TO THE BAY THEY UNLOAD AT", from.to_uppercase());
                }
                (Some(from), Some(name)) if *from == name => {
                    g.verdict = Verdict::Refused;
                    g.note = "A ROUTE RUNS BETWEEN TWO BAYS".into();
                }
                (Some(from), Some(name)) => {
                    let cost = c.lines["tram"].vehicle_cost.clone();
                    g.note = format!("THE {} TO THE {}: {} A WAGON", from.to_uppercase(), name.to_uppercase(), money(&cost));
                    g.cost = cost.clone();
                    if cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                        g.note += "  (THE CAMP LACKS IT)";
                    }
                }
            }
        }
        Tool::Line(kind) => {
            let spec = &c.lines[kind.key()];
            match &board.draft {
                None => {
                    g.note = match building_near(k, cursor) {
                        Some(id) => format!("PRESS E TO BEGIN AT THE {}", c.plans[&k.buildings[&id].plan].name.to_uppercase()),
                        None => "STAND BY A BUILDING, PRESS E, WALK, LET GO AT ANOTHER".to_string(),
                    };
                    g.verdict = if building_near(k, cursor).is_some() { Verdict::Fits } else { Verdict::Refused };
                }
                Some(d) => {
                    let tiles = d.path.len();
                    let track = if *kind == LineKind::Tram { spec.tile_cost.clone() } else { spec.tile_cost.clone() };
                    let mut cost = Stock::new();
                    for (r, n) in &track {
                        *cost.entry(r.clone()).or_default() += n * tiles as u32;
                    }
                    for (r, n) in &spec.vehicle_cost {
                        *cost.entry(r.clone()).or_default() += n;
                    }
                    g.cost = cost.clone();
                    g.note = format!("{tiles} TILES: {}  LET GO AT THE OTHER BUILDING", money(&cost));
                    if cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                    }
                }
            }
        }
        Tool::Mend => {
            // What is broken here, in words.
            let id = k.at(cursor);
            g.note = if id.is_some_and(|id| k.havoc.fires.contains_key(&id)) {
                "A FIRE: E BEATS AT THE FLAMES, A WELL BESIDE IT PUTS IT OUT".into()
            } else if let Some(gid) = k.ghost_at(c, cursor).filter(|g| k.plans.ghosts[g].by == 0) {
                g.cost = k.ghost_cost(c, &k.plans.ghosts[&gid].plan);
                match k.ghost_wait(c, gid) {
                    Some(why) => {
                        g.verdict = Verdict::Short;
                        format!("BROKEN: {} (THE HANDS WAIT)", why.to_uppercase())
                    }
                    None => format!("BROKEN: E SETS IT TO RIGHTS NOW, {}", money(&g.cost)),
                }
            } else if let Some((left, max)) = id.and_then(|id| k.defence_of(c, id)).filter(|(l, m)| l < m) {
                format!("BATTERED: {left} OF {max} HOLD. E MENDS IT")
            } else {
                g.verdict = Verdict::Refused;
                "NOTHING THE DEEP HAS BROKEN IS HERE".into()
            };
        }
        Tool::Pattern(name) => {
            g.anchor = cursor;
            let Some(p) = k.pattern(c, name) else {
                g.verdict = Verdict::Refused;
                g.note = "NOTHING KEPT HERE: M COPIES A GROUP OF WORKS".into();
                return g;
            };
            // The pattern's reach, turned.
            let mut lo = (0i16, 0i16);
            let mut hi = (0i16, 0i16);
            let turned = |(dx, dy): (i16, i16)| match board.turn % 4 {
                0 => (dx, dy),
                1 => (-dy, dx),
                2 => (-dx, -dy),
                _ => (dy, -dx),
            };
            let mut reach = |off: (i16, i16), plan: Option<&str>| {
                let (a, b) = (turned(off), plan.map_or((0, 0), |pl| turned((i16::from(k.dims(&c.plans[pl]).0), i16::from(k.dims(&c.plans[pl]).1)))));
                for (x, y) in [a, (a.0 + b.0, a.1 + b.1)] {
                    lo = (lo.0.min(x), lo.1.min(y));
                    hi = (hi.0.max(x), hi.1.max(y));
                }
            };
            for r in p.roads.iter().chain(&p.shafts) {
                reach(*r, None);
            }
            for b in &p.builds {
                reach(b.at, Some(&b.plan));
            }
            g.rect = (cursor.0 + lo.0, cursor.1 + lo.1, cursor.0 + hi.0, cursor.1 + hi.1);
            match stamp_cost(k, name, cursor, board.turn) {
                Err(why) => {
                    g.verdict = Verdict::Refused;
                    g.note = why.to_uppercase();
                }
                Ok(cost) => {
                    g.cost = cost.clone();
                    g.note = format!("{}  R TURNS IT", money(&cost));
                    if cost.iter().any(|(r, n)| k.amount(r) < *n) {
                        g.verdict = Verdict::Short;
                        g.note += "  (A PLAN: THE CAMP LACKS IT)";
                    } else if board.rig.planning {
                        g.verdict = Verdict::Short;
                    }
                }
            }
        }
    }
    g
}

/// The keys of a knight with the builder in hand, other than the walking: what
/// the world gets of their input while they build (the rest is the builder's).
pub fn walking_input(input: &Input) -> Input {
    Input { move_x: input.move_x, move_y: input.move_y, ..Input::default() }
}

/// Put the cursor where the knight's feet and face say, or where the mouse
/// pointed it, and the board's turn and facing up to date. `bits` as the desk
/// reads the keys.
pub(super) fn aim_cursor(board: &mut Board, hero: Option<(f32, f32)>, input: &Input) {
    if input.move_x != 0 || input.move_y != 0 {
        board.pointed = false;
        board.face = if input.move_y.abs() >= input.move_x.abs() && input.move_y != 0 {
            if input.move_y < 0 { 0 } else { 2 }
        } else if input.move_x > 0 {
            1
        } else {
            3
        };
    }
    if !board.pointed
        && let Some((x, y)) = hero
    {
        let feet = ((x / TILE_UNITS).floor() as i16, (y / TILE_UNITS).floor() as i16);
        board.tile = ahead(feet, board.face);
    }
}

/// What E does with the tool in hand at `tile`: build it now if the camp can pay and the
/// builder is not planning, else lay it as a plan for the camp's hands. Returns the words
/// and what was laid (for taking it back).
fn lay(board: &Board, k: &mut Kingdom, tool: &Tool, at: Tile, seat: u32) -> (Result<String, String>, Vec<Laid>) {
    let c = catalog();
    let snap = Snap::of(k);
    let planning = board.rig.planning;
    let plan = |k: &mut Kingdom, plan: &str, tile: Tile| k.apply(c, Command::Plan { plan: plan.to_string(), tile, priority: 1, by: seat });
    let result = match tool {
        Tool::Road => {
            let river = k.world.terrain(at) == Terrain::River;
            let cost = if river { Stock::from([("plank".to_string(), 4), ("stone".to_string(), 2)]) } else { Stock::from([("timber".to_string(), 1)]) };
            let paid = cost.iter().all(|(r, n)| k.amount(r) >= *n);
            if river {
                if planning || !paid { plan(k, crate::kingdom::ghosts::BRIDGE, at) } else { k.apply(c, Command::Bridge(at)) }
            } else if planning || !paid {
                plan(k, crate::kingdom::ghosts::ROAD, at)
            } else {
                k.apply(c, Command::Road(at))
            }
        }
        Tool::Plan(id) => {
            let p = k.placement(c, id, at);
            if planning || (p.land.is_ok() && !p.ok()) {
                plan(k, id, at)
            } else {
                let said = k.apply(c, Command::Build { plan: (*id).to_string(), tile: at });
                // A swape is raised facing the way the builder faces.
                if said.is_ok() && c.plans[*id].swape > 0 {
                    let _ = k.apply(c, Command::Face { tile: at, dir: facing(board) });
                }
                said
            }
        }
        Tool::Pattern(name) => {
            let ok = stamp_cost(k, name, at, board.turn).ok().filter(|cost| cost.iter().all(|(r, n)| k.amount(r) >= *n));
            if planning || ok.is_none() {
                k.apply(c, Command::PlanStamp { pattern: (*name).to_string(), origin: at, turns: board.turn, priority: 1, by: seat })
            } else {
                k.apply(c, Command::Stamp { pattern: (*name).to_string(), origin: at, turns: board.turn })
            }
        }
        Tool::Line(_) => Err("lines are drawn by walking".into()),
        Tool::Roller(_) | Tool::Rail | Tool::Bay | Tool::Route => Err("laid by their own keys".into()),
        Tool::Mend => k.apply(c, Command::Mend(at)),
    };
    let what = if result.is_ok() { snap.laid(k) } else { Vec::new() };
    (result, what)
}

/// What the cursor holds in hand, as a tool: the plan or work standing there, the road, the
/// line.
fn grab(k: &Kingdom, cursor: Tile) -> Option<Tool> {
    let c = catalog();
    let plan_tool = |plan: &str| tools().iter().find(|t| matches!(t, Tool::Plan(id) if *id == plan)).cloned();
    if let Some(id) = k.ghost_at(c, cursor) {
        let plan = k.plans.ghosts[&id].plan.as_str();
        return if plan == crate::kingdom::ghosts::ROAD || plan == crate::kingdom::ghosts::BRIDGE { Some(Tool::Road) } else { plan_tool(plan) };
    }
    if let Some(id) = building_near(k, cursor).filter(|_| k.at(cursor).is_some()) {
        return plan_tool(&k.buildings[&id].plan);
    }
    if let Some(r) = k.roller_at(cursor) {
        return Some(Tool::Roller(r.piece));
    }
    if k.rails.cells.contains_key(&cursor) {
        return Some(Tool::Rail);
    }
    if k.roads.contains(&cursor) {
        return Some(Tool::Road);
    }
    k.lines.values().find(|l| l.path.contains(&cursor)).and_then(|l| tools().iter().find(|t| matches!(t, Tool::Line(kind) if *kind == l.kind)).cloned())
}

/// What one press of E (or the tiles laid while it is held) laid, kept for taking back as one.
fn remember(board: &mut Board, what: Vec<Laid>, first: bool, held: bool) {
    let extend = board.rig.held_step && !first;
    if extend && let Some(last) = board.rig.undo.last_mut() {
        last.what.extend(what);
    } else {
        board.rig.laid(what);
    }
    board.rig.held_step = held;
}

fn index_of(tool: &Tool) -> usize {
    tools().iter().position(|t| t == tool).unwrap_or(0)
}

/// The builder's keys for one seat, one tick (the world mode of the desk):
/// `fresh` the keys newly down, `released` the keys just let go, `bits` the
/// keys down, laid out as `industry_inputs` lays them. Returns the words
/// for the board's message line.
pub(super) fn world_keys(board: &mut Board, k: &mut Kingdom, tick: u64, input: &Input, fresh: u32, released: u32, bits: u32, seat: u32) {
    let c = catalog();
    let n = tools().len();
    let say = |board: &mut Board, r: Result<String, String>| board.message = r.unwrap_or_else(|e| e);
    // The tool in hand: Q and the side arrows step through every tool, the number keys take a
    // slot of the hotbar, the up and down arrows another bank of it. The slot in hand holds
    // whatever is in hand.
    let before = board.plan;
    if fresh & 4 != 0 {
        board.plan = step_tool(k, board.plan, 1);
        board.draft = None;
    }
    if input.aim_x != 0 && tick.saturating_sub(board.plan_moved) >= 6 {
        board.plan = step_tool(k, board.plan, i32::from(input.aim_x));
        board.plan_moved = tick;
        board.draft = None;
    }
    if input.aim_y != 0 && tick.saturating_sub(board.plan_moved) >= 6 {
        board.plan = board.rig.bank_by(i32::from(input.aim_y));
        board.plan_moved = tick;
        board.draft = None;
    } else {
        board.rig.keep(board.plan.min(n - 1));
    }
    for (bit, i) in [(32u32, 0usize), (64, 1), (128, 2)] {
        if fresh & bit != 0 {
            board.plan = board.rig.take(board.rig.bank() * builder_rig::BANK + i);
            board.draft = None;
        }
    }
    if board.plan >= n {
        board.plan = 0;
    }
    if board.plan != before {
        board.rig.mark = None;
    }
    if fresh & (256 | (1 << 13)) != 0 {
        // On a piece of rollerway or a swape, P turns the one under the cursor a quarter; else it
        // turns what is about to be laid.
        let at = board.tile;
        let turns_there = matches!(tool_of(board), Tool::Roller(_) | Tool::Plan("swape")) && (k.roller_at(at).is_some() || k.at(at).is_some_and(|id| k.is_swape(c, id)));
        if turns_there {
            let dir = k.roller_at(at).map(|x| x.dir).or_else(|| k.at(at).map(|id| Dir::from_index(k.buildings[&id].facing))).unwrap_or(Dir::E);
            let target = k.at(at).map(|id| k.buildings[&id].tile).unwrap_or(at);
            board.message = k.apply(c, Command::Face { tile: target, dir: dir.right() }).unwrap_or_else(|e| e);
        } else {
            board.turn = (board.turn + 1) % 4;
        }
    }
    if fresh & 1024 != 0 {
        board.overlay = !board.overlay;
    }
    if fresh & 512 != 0 {
        board.page = 1;
        board.selection = 0;
        board.draft = None;
        board.rig.card = false;
        return;
    }
    if fresh & (1 << 14) != 0 {
        board.rig.card = !board.rig.card;
        board.message = if board.rig.card { "THE CARD OF WHAT YOU POINT AT".into() } else { "CARD AWAY".into() };
    }
    if fresh & (1 << 16) != 0 {
        board.rig.planning = !board.rig.planning;
        board.message = if board.rig.planning { "PLANNING: WHAT YOU LAY WAITS FOR THE CAMP'S HANDS".into() } else { "BUILDING NOW: WHAT THE CAMP CAN PAY FOR GOES UP AT ONCE".into() };
    }
    let tool = tool_of(board).clone();
    let ghost = ghost(board, k);
    let cursor = board.tile;
    board.anchor = ghost.anchor;
    let here = building_near(k, cursor);
    let held = bits & 2 != 0;
    if released & 2 != 0 {
        board.rig.held_step = false;
    }
    // Z takes back the last thing laid.
    if fresh & (1 << 11) != 0 {
        match board.rig.undo.pop() {
            Some(step) => say(board, k.apply(c, Command::Retract(step.what))),
            None => board.message = "NOTHING TO TAKE BACK".into(),
        }
    }
    // G takes in hand what the cursor is on.
    if fresh & (1 << 12) != 0 {
        match grab(k, cursor) {
            Some(t) => {
                board.plan = index_of(&t);
                board.rig.keep(board.plan);
                board.message = format!("IN HAND: {}", t.name());
            }
            None => board.message = "NOTHING HERE TO TAKE IN HAND".into(),
        }
    }
    // M marks the corner of a patch, then the other, and copies what stands in it.
    if fresh & (1 << 15) != 0 {
        match board.rig.mark.take() {
            None => {
                board.rig.mark = Some(cursor);
                board.message = "COPYING: WALK TO THE OTHER CORNER AND PRESS M AGAIN, X DROPS IT".into();
            }
            Some(corner) => {
                let names = crate::kingdom::ghosts::COPY_NAMES;
                let name = names.iter().find(|n| !k.copies.contains_key(**n)).copied().unwrap_or(names[(k.rev as usize) % names.len()]);
                match k.apply(c, Command::Clip { name: name.to_string(), from: corner, to: cursor }) {
                    Ok(words) => {
                        board.plan = index_of(&Tool::Pattern(name));
                        board.rig.keep(board.plan);
                        board.message = format!("{}: E STAMPS IT, R TURNS IT", words.to_uppercase());
                    }
                    Err(e) => board.message = e.to_uppercase(),
                }
            }
        }
    }
    // E: lay the tool (or walk a line).
    match &tool {
        Tool::Road | Tool::Plan(_) | Tool::Pattern(_) => {
            // Holding E while walking lays a row, as a mouse dragged does (a stamp is one press).
            let brush = held && cursor != board.painted && !matches!(tool, Tool::Pattern(_));
            if fresh & 2 != 0 || brush {
                board.painted = cursor;
                let at = if matches!(tool, Tool::Plan(_)) && !tool.brush() { ghost.anchor } else { cursor };
                let first = fresh & 2 != 0;
                let (result, what) = lay(board, k, &tool, at, seat);
                match &result {
                    Ok(_) => {
                        remember(board, what, first, held);
                        say(board, result);
                    }
                    // Walking along laying a row, a place that cannot take another is no error.
                    Err(_) if !first => {}
                    Err(_) => say(board, result),
                }
            }
        }
        Tool::Roller(_) | Tool::Rail => {
            let first = fresh & 2 != 0;
            // Held while walking, E lays a run of it, tile by tile.
            if first || (held && cursor != board.painted) {
                let last = board.painted;
                board.painted = cursor;
                let snap = Snap::of(k);
                let laid = match &tool {
                    Tool::Roller(piece) => {
                        // Walking a run of it: each tile faces the next, so a corner turns itself, and
                        // the new tile faces the way the knight walks.
                        let step = (!first).then(|| Dir::between(last, cursor)).flatten();
                        let dir = step.unwrap_or_else(|| facing(board));
                        if let Some(d) = step
                            && k.roller_at(last).is_some_and(|x| matches!(x.piece, Piece::Chain | Piece::Roller) && x.dir != d)
                        {
                            let _ = k.apply(c, Command::Face { tile: last, dir: d });
                        }
                        k.apply(c, Command::Roller { piece: *piece, tile: cursor, dir })
                    }
                    _ => k.apply(c, Command::Rail(cursor)),
                };
                match &laid {
                    Ok(_) => {
                        remember(board, snap.laid(k), first, held);
                        say(board, laid);
                    }
                    // Painting over what is laid so is no error.
                    Err(e) if !first && (e.contains("so already") || e.contains("laid here already")) => {}
                    Err(_) => say(board, laid),
                }
            }
        }
        Tool::Bay => {
            if fresh & 2 != 0 {
                say(board, k.apply(c, Command::Bay { tile: cursor, name: None }));
            }
        }
        Tool::Mend => {
            // A press beats at a fire, mends a wall, or sets a cut line right.
            if fresh & 2 != 0 {
                board.painted = cursor;
                say(board, k.apply(c, Command::Mend(cursor)));
            }
        }
        Tool::Route => {
            if fresh & 2 != 0 {
                let here = k.rails.cells.get(&cursor).and_then(|r| r.bay.clone());
                match (board.stop.take(), here) {
                    (None, Some(name)) => {
                        board.message = format!("LOADING AT THE {}: GO TO THE BAY THEY UNLOAD AT, PRESS E", name.to_uppercase());
                        board.stop = Some(name);
                    }
                    (Some(from), Some(to)) if from != to => {
                        let stops = vec![
                            crate::kingdom::Stop { bay: from, act: crate::kingdom::Act::Load, good: None },
                            crate::kingdom::Stop { bay: to, act: crate::kingdom::Act::Unload, good: None },
                        ];
                        say(board, k.apply(c, Command::Route { name: None, stops, wagons: 1 }));
                    }
                    (Some(from), _) => {
                        board.message = format!("LOADING AT THE {}: GO TO ANOTHER BAY", from.to_uppercase());
                        board.stop = Some(from);
                    }
                    (None, None) => board.message = "STAND AT A BAY AND PRESS E".into(),
                }
            }
        }
        Tool::Line(kind) => {
            if fresh & 2 != 0 {
                match building_near(k, cursor) {
                    Some(id) => {
                        // The way begins on the ground the knight stands on (the cursor is the tile ahead).
                        let (dx, dy) = FACES[usize::from(board.face % 4)];
                        let feet = (cursor.0 - dx, cursor.1 - dy);
                        let first = if k.at(feet).is_none() { vec![feet] } else { Vec::new() };
                        board.draft = Some(Draft { kind: *kind, from: id, path: first });
                        board.message = format!("{} BEGUN AT THE {}", kind.key().to_uppercase(), c.plans[&k.buildings[&id].plan].name.to_uppercase());
                    }
                    None => board.message = "STAND BY A BUILDING TO BEGIN A LINE".into(),
                }
            } else if held {
                if let Some(d) = board.draft.as_mut() {
                    let from_tile = k.buildings.get(&d.from).map(|b| b.tile);
                    let outside = !from_tile.is_some_and(|t| t == cursor) && k.at(cursor).is_none();
                    if outside && d.path.last() != Some(&cursor) && d.path.len() < MAX_PATH {
                        // Walking back over its own way takes the tail off.
                        if let Some(at) = d.path.iter().position(|t| *t == cursor) {
                            d.path.truncate(at + 1);
                        } else {
                            // A step across a corner is two steps along the sides.
                            if let Some(&last) = d.path.last()
                                && last.0 != cursor.0
                                && last.1 != cursor.1
                            {
                                let corner = (cursor.0, last.1);
                                if k.at(corner).is_none() && !d.path.contains(&corner) {
                                    d.path.push(corner);
                                }
                            }
                            d.path.push(cursor);
                        }
                    }
                }
            }
            if released & 2 != 0 {
                if let Some(d) = board.draft.take() {
                    let snap = Snap::of(k);
                    let result = finish_line(k, &d, cursor);
                    if result.is_ok() {
                        let what = snap.laid(k);
                        board.rig.laid(what);
                    }
                    say(board, result);
                }
            }
        }
    }
    // X: drop a copy half marked or a line half drawn, else cancel the plan under the ghost,
    // else take down what is under it.
    if fresh & 8 != 0 {
        if board.rig.mark.take().is_some() {
            board.message = "COPY DROPPED".into();
        } else if board.stop.take().is_some() {
            board.message = "ROUTE DROPPED".into();
        } else if board.draft.take().is_some() {
            board.message = "LINE DROPPED".into();
        } else if let Some(id) = k.ghost_at(c, cursor) {
            say(board, k.apply(c, Command::Unplan(id)));
        } else {
            let target = if (k.roller_at(cursor).is_some() || k.rails.cells.contains_key(&cursor)) && k.at(cursor).is_none() {
                cursor
            } else {
                here.map(|id| k.buildings[&id].tile).unwrap_or(cursor)
            };
            if let Tool::Line(_) = tool {
                // Take down the line whose track is under the cursor.
                let hit = k.lines.iter().find(|(_, l)| l.path.contains(&cursor)).map(|(id, _)| *id);
                match hit {
                    Some(id) => say(board, k.apply(c, Command::Unline(id))),
                    None => say(board, k.apply(c, Command::Demolish(target))),
                }
            } else {
                say(board, k.apply(c, Command::Demolish(target)));
            }
        }
    }
    // F pauses what is under the ghost, or sets how urgent a plan is.
    if fresh & 16 != 0 {
        if let Some(id) = k.ghost_at(c, cursor) {
            let next = (k.plans.ghosts[&id].priority + 1) % 3;
            say(board, k.apply(c, Command::Rank { ghost: id, priority: next }));
        } else {
            match here {
                Some(id) => say(board, k.apply(c, Command::Pause(id))),
                None => board.message = "NOTHING HERE TO PAUSE".into(),
            }
        }
    }
    // . changes its recipe (at a carter's yard, the carts it keeps; at a swape, the one good it
    // lifts; with the route tool in hand, it sends another wagon out); ; links a claim (with the
    // route tool, sells a wagon); ' turns the supply.
    if fresh & ((1 << 17) | (1 << 18)) != 0 && matches!(tool, Tool::Route) {
        // . sends another wagon out on the route that serves the bay here; ; sells one.
        let bay = k.rails.cells.get(&cursor).and_then(|r| r.bay.clone());
        let route = bay.and_then(|name| k.rails.routes.iter().find(|(_, r)| r.stops.iter().any(|s| s.bay == name)).map(|(&i, _)| i));
        match route {
            Some(id) => {
                let have = k.rails.wagons.values().filter(|w| w.route == id).count() as u16;
                let want = if fresh & (1 << 17) != 0 { have + 1 } else { have.saturating_sub(1).max(1) };
                say(board, k.apply(c, Command::Wagons { route: id, wagons: want }));
            }
            None => board.message = "NO ROUTE RUNS FROM HERE".into(),
        }
    } else if fresh & (1 << 17) != 0 {
        if let Some(id) = here {
            let b = &k.buildings[&id];
            let p = &c.plans[&b.plan];
            let recipes = &p.recipes;
            if p.swape > 0 {
                // A swape lifts whatever comes, or one good of those it stands between.
                let goods = k.swape_goods(c, id);
                let next = match &b.filter {
                    None => goods.first().cloned(),
                    Some(g) => goods.iter().position(|x| x == g).and_then(|i| goods.get(i + 1)).cloned(),
                };
                say(board, k.apply(c, Command::Filter { building: id, good: next }));
            } else if p.stalls > 0 {
                let have = b.carts.len() as u16;
                let next = if have >= p.stalls { 2 } else { (have / 2 + 1) * 2 }.min(p.stalls);
                say(board, k.apply(c, Command::Carts { yard: id, carts: next }).map(|m| format!("{m}: {next} CARTS")));
            } else if recipes.is_empty() {
                board.message = "IT HAS NO RECIPES".into();
            } else {
                let next = b.recipe.as_ref().and_then(|r| recipes.iter().position(|v| v == r)).map_or(0, |i| (i + 1) % recipes.len());
                say(board, k.apply(c, Command::Recipe { building: id, recipe: recipes[next].clone() }));
            }
        }
    }
    if fresh & (1 << 18) != 0 {
        if let Some(id) = here {
            let current = k.buildings[&id].claim.clone();
            let free: Vec<String> = k
                .claims
                .iter()
                .filter(|(key, q)| q.secured && !k.buildings.iter().any(|(&o, b)| o != id && b.claim.as_ref() == Some(*key)))
                .map(|(key, _)| key.clone())
                .collect();
            if free.is_empty() {
                board.message = "NO FREE CLAIM: WIN A SEAM IN THE DELVE".into();
            } else {
                let next = current.and_then(|q| free.iter().position(|v| *v == q)).map_or(0, |i| (i + 1) % free.len());
                say(board, k.apply(c, Command::Link { building: id, claim: free[next].clone() }));
            }
        }
    }
    if fresh & (1 << 19) != 0 {
        say(board, k.apply(c, Command::Provision));
    }
    builder_rig::refresh(board, k);
}

/// The goals, production and chokes pages of the ledger, one tick of one seat's keys on
/// them: the arrows and Q move the selection; E on a choke sends the view to look at it.
pub(super) fn ledger_screen(board: &mut Board, k: &Kingdom, tick: u64, input: &Input, fresh: u32) {
    let numbers = board.rig.numbers.as_ref();
    let count = match board.page {
        PAGE_GOALS => k.goals(catalog()).nodes.len(),
        PAGE_PRODUCTION => numbers.and_then(|n| n.production.as_ref()).map_or(0, |p| p.goods.len()),
        _ => numbers.map_or(0, |n| n.chokes.len()),
    }
    .max(1);
    board.selection = board.selection.min(count - 1);
    if tick.saturating_sub(board.moved) >= 5 {
        let delta = i32::from(input.move_y) + i32::from(input.move_x);
        if delta != 0 {
            board.selection = (board.selection as i32 + delta).rem_euclid(count as i32) as usize;
            board.moved = tick;
        }
    }
    if fresh & 4 != 0 {
        board.selection = (board.selection + 1) % count;
    }
    if board.page == PAGE_CHOKES && fresh & 2 != 0 {
        if let Some(ch) = board.rig.numbers.as_ref().and_then(|n| n.chokes.get(board.selection)).cloned() {
            // The cursor goes there, the view with it: walking brings it back.
            board.tile = ch.at;
            board.pointed = true;
            board.rig.look = Some((f32::from(ch.at.0) + 0.5, f32::from(ch.at.1) + 0.5));
            board.rig.card = true;
            board.page = 0;
            board.selection = 0;
            board.message = format!("THE VIEW IS AT: {}", ch.text);
        }
    }
    builder_rig::refresh(board, k);
}

/// Lay the line a knight has walked from `d.from` to the building by the cursor.
/// The ground they crossed is the line's way when the sim takes it; else the
/// shortest way between the two is laid, and the words say so.
fn finish_line(k: &mut Kingdom, d: &Draft, cursor: Tile) -> Result<String, String> {
    let c = catalog();
    let Some(to) = building_near(k, cursor).filter(|to| *to != d.from) else {
        return Err("A LINE ENDS AT ANOTHER BUILDING: IT WAS DROPPED".into());
    };
    let (Some(a), Some(b)) = (k.buildings.get(&d.from).map(|b| b.tile), k.buildings.get(&to).map(|b| b.tile)) else {
        return Err("A LINE NEEDS TWO BUILDINGS".into());
    };
    let _ = (a, b);
    // The building's own tile each end of the way starts from, and the ground between it and
    // where the knight began or ended (a tile or two of a gap, when they let go a step off).
    let (from, tail_from) = gap(k, d.from, d.path.first().copied());
    let (end, tail_to) = gap(k, to, d.path.last().copied());
    let walked = match (from, end) {
        (Some(from_tile), Some(to_tile)) if !d.path.is_empty() => {
            let mut way = vec![from_tile];
            way.extend(tail_from);
            way.extend(d.path.iter().copied());
            way.extend(tail_to.into_iter().rev());
            way.push(to_tile);
            let mut seen = std::collections::BTreeSet::new();
            way.retain(|t| seen.insert(*t));
            Some(Command::Line { kind: d.kind, from: d.from, to, path: way, vehicles: 1 })
        }
        _ => None,
    };
    let Some(walked) = walked else {
        return k.apply(c, Command::Line { kind: d.kind, from: d.from, to, path: Vec::new(), vehicles: 1 });
    };
    match k.apply(c, walked) {
        Ok(m) => Ok(m),
        Err(first) => {
            let planned = Command::Line { kind: d.kind, from: d.from, to, path: Vec::new(), vehicles: 1 };
            match k.apply(c, planned) {
                Ok(m) => Ok(format!("{m} (BY THE SHORTEST WAY: {})", first.to_uppercase())),
                Err(e) => Err(e),
            }
        }
    }
}

/// The tile of building `id` nearest `tile`, and the free tiles in a straight run between them
/// (the gap, none when they touch; the first step is along x).
fn gap(k: &Kingdom, id: u32, tile: Option<Tile>) -> (Option<Tile>, Vec<Tile>) {
    let (Some(b), Some(tile)) = (k.buildings.get(&id), tile) else { return (None, Vec::new()) };
    let Some(near) = k.covered(b).min_by_key(|t| t.0.abs_diff(tile.0) + t.1.abs_diff(tile.1)) else { return (None, Vec::new()) };
    let mut between = Vec::new();
    let mut at = near;
    while at != tile {
        at = if at.0 != tile.0 { (at.0 + (tile.0 - at.0).signum(), at.1) } else { (at.0, at.1 + (tile.1 - at.1).signum()) };
        if at != tile {
            if k.at(at).is_some() {
                return (None, Vec::new());
            }
            between.push(at);
        }
    }
    (Some(near), between)
}

/// A click: the cursor goes to the tile, and the tool is laid there if it was
/// the left button.
pub(super) fn plot(board: &mut Board, tile: Tile) {
    board.tile = tile;
    board.pointed = true;
}

/// Whether this seat has the builder out in the world.
pub fn building(run: &Run, seat: u32) -> bool {
    run.hill_world() && run.kingdom.industry.get(&seat).is_some_and(|b| b.open && b.page == 0)
}

impl Run {
    /// The industry is the hillside kingdom and the party is on the surface.
    pub fn hill_world(&self) -> bool {
        self.dungeon.depth == 0 && self.room().kind == RoomKind::Kingdom && self.sim_hillside()
    }

    /// The ghost seat `seat` sees, if their builder is out in the world.
    pub fn hill_ghost(&self, seat: u32) -> Option<Ghost> {
        let board = self.kingdom.industry.get(&seat)?;
        (building(self, seat)).then(|| ghost(board, &self.home.industry))
    }

    /// Leave the builder's pointer action to the board: set the cursor. Returns whether it
    /// is a press (E, as far as the board's tool is concerned) and whether it is a release.
    /// The button held down is E held: a row laid as the mouse is dragged along it, a line
    /// drawn from one building to another.
    pub(super) fn hill_pointer(board: &mut Board, action: BuilderAction) -> (bool, bool) {
        match action {
            BuilderAction::Plot { tile, build } => {
                let tile = (i16::from(tile.0), i16::from(tile.1));
                plot(board, tile);
                // A right click points: its card. A left click lays, and the card goes.
                board.rig.card = !build;
                board.rig.drag = build.then_some((tile, tile));
                (build, false)
            }
            BuilderAction::Hold(tile) => {
                let tile = (i16::from(tile.0), i16::from(tile.1));
                plot(board, tile);
                if let Some(d) = board.rig.drag.as_mut() {
                    d.1 = tile;
                }
                (false, false)
            }
            BuilderAction::Let(tile) => {
                plot(board, (i16::from(tile.0), i16::from(tile.1)));
                (false, board.rig.drag.take().is_some())
            }
            BuilderAction::Slot(n) => {
                board.plan = board.rig.take(usize::from(n));
                (false, false)
            }
            BuilderAction::Plan(plan) => {
                board.plan = usize::from(plan) % tools().len();
                board.rig.keep(board.plan);
                (false, false)
            }
            BuilderAction::NextPlan { back } => {
                let n = tools().len();
                board.plan = (board.plan + if back { n - 1 } else { 1 }) % n;
                board.rig.keep(board.plan);
                (false, false)
            }
            BuilderAction::Confirm => (true, false),
            _ => (false, false),
        }
    }

    /// The tiles under the hillside's buildings are shut in the realm's room, as
    /// the Keep's stonework is (`site::patch_room`), and open again when a building
    /// comes down. Read when the kingdom has changed; a knight a building
    /// went up on is moved to the nearest open ground.
    pub(super) fn follow_works_tiles(&mut self, index: usize) {
        use std::collections::BTreeSet;
        if !self.sim_hillside() {
            return;
        }
        let k = &self.home.industry;
        if self.hands.seen.walled_rev == k.rev && self.hands.seen.walled_rev != 0 {
            return;
        }
        let c = catalog();
        let keep = crate::kingdom::keep_rect();
        let now: BTreeSet<Tile> = k
            .buildings
            .values()
            .filter(|b| !c.plans[&b.plan].passable)
            .flat_map(|b| k.covered(b))
            .filter(|t| !crate::kingdom::hillside_in_rect(*t, keep))
            .collect();
        self.hands.seen.walled_rev = k.rev;
        if now == self.hands.seen.walled {
            return;
        }
        let room = &mut self.dungeon.rooms[index];
        for t in self.hands.seen.walled.difference(&now) {
            if (t.0 as usize) < room.cols && (t.1 as usize) < room.rows {
                room.set(t.0 as usize, t.1 as usize, layout::Tile::Floor);
            }
        }
        for t in now.difference(&self.hands.seen.walled) {
            if (t.0 as usize) < room.cols && (t.1 as usize) < room.rows {
                room.set(t.0 as usize, t.1 as usize, layout::Tile::Wall);
            }
        }
        self.hands.seen.walled = now;
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        // A building went up on a knight: out to the nearest open ground.
        let room = &self.dungeon.rooms[index];
        let open = |x: i32, y: i32| x >= 0 && y >= 0 && (x as usize) < room.cols && (y as usize) < room.rows && room.tile(x, y) == layout::Tile::Floor;
        for h in self.players.values_mut() {
            let (tx, ty) = ((h.x / TILE_UNITS) as i32, (h.y / TILE_UNITS) as i32);
            if open(tx, ty) {
                continue;
            }
            'ring: for r in 1i32..12 {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs().max(dy.abs()) == r && open(tx + dx, ty + dy) {
                            h.x = ((tx + dx) as f32 + 0.5) * TILE_UNITS;
                            h.y = ((ty + dy) as f32 + 0.5) * TILE_UNITS;
                            break 'ring;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/hillside__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/rig__tests.rs"]
mod rig_tests;

#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/hillside__shots.rs"]
mod shots;

#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/factory__shots.rs"]
mod factory_shots;

#[cfg(all(test, feature = "renderer"))]
#[path = "../../tests/render/threat__shots.rs"]
mod threat_shots;
