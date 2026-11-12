//! The conversion: choose the playable levels, crop them to the fortress,
//! translate every tile into the game's glyphs, and gather the places,
//! people, races and resources.

use crate::input::{self, Building, Export, arr, int_of, str_of};
use crate::levelfile::LevelFile;
use crate::links::{self, Rect};
use crate::rocks::{self, Classifier};
use crate::text::{self, slug, split_camel};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Larger than this is more than the game shows at once; it is kept anyway
/// and reported.
pub const MAX_W: usize = 168;
pub const MAX_H: usize = 108;
/// Room around the fortress's outermost dug, built or furnished tile.
pub const PAD: i32 = 2;
/// How far the surface level looks down for the ground under open sky.
pub const SKY_DEPTH: i32 = 8;
/// The most levels the game plays; the surface entrance is always one.
pub const MAX_LEVELS: usize = 8;
/// Functions (indexes into `FUNCTIONS`) that always get a level: the forges
/// (the steel line) and the barracks.
pub const MUST_KEEP: &[usize] = &[2, 11];

#[derive(Clone, Copy, Debug)]
pub struct Crop {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
}

impl Crop {
    pub fn local(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        let (cx, cy) = (x - self.x0, y - self.y0);
        if cx < 0 || cy < 0 || cx as usize >= self.w || cy as usize >= self.h {
            None
        } else {
            Some((cx as usize, cy as usize))
        }
    }

    /// The note on a crop larger than the game shows at once.
    fn size_note(&self) -> String {
        if self.w > MAX_W || self.h > MAX_H {
            format!(" (larger than {MAX_W} x {MAX_H}; kept)")
        } else {
            String::new()
        }
    }
}

/// A DF staircase tile: up/down, up or down.
fn is_stair(shape: u8) -> bool {
    matches!(shape, b'X' | b'<' | b'>')
}

/// An export `pos` as (x, y, z); -1 for what is missing.
fn pos_of(p: Option<&Value>) -> (i32, i32, i32) {
    let c = |k: &str| p.and_then(|p| int_of(p, k)).unwrap_or(-1) as i32;
    (c("x"), c("y"), c("z"))
}

#[derive(Default)]
struct ZStat {
    floors: usize,
    stairs: usize,
    veg: usize,
    buildings: usize,
}

struct Location {
    kind: String,
    name: String,
    name_english: String,
    deity: Option<String>,
    religion: Option<Religion>,
    profession: Option<String>,
}

/// The religion (an entity) a temple is dedicated to.
struct Religion {
    entity: Option<i64>,
    name: Option<String>,
    name_english: Option<String>,
}

fn locations(root: &Value) -> BTreeMap<i64, Location> {
    let mut out = BTreeMap::new();
    for l in arr(root, "locations") {
        let Some(id) = int_of(l, "id") else { continue };
        // deity_type 0: `deity` is a historical figure. 1: `religion` is an
        // entity (an older exporter wrote a wrong figure as `deity` here, so
        // `deity` is read only for 0).
        let deity_type = int_of(l, "deity_type");
        let deity = l.get("deity").filter(|_| deity_type == Some(0)).and_then(|d| str_of(d, "name_english"));
        let religion = l.get("religion").filter(|r| deity_type == Some(1) && r.is_object()).map(|r| Religion {
            entity: int_of(r, "entity"),
            name: str_of(r, "name"),
            name_english: str_of(r, "name_english"),
        });
        out.insert(
            id,
            Location {
                kind: str_of(l, "type").unwrap_or_default(),
                name: str_of(l, "name").unwrap_or_default(),
                name_english: str_of(l, "name_english").unwrap_or_default(),
                deity,
                religion,
                profession: str_of(l, "profession").filter(|p| p != "NONE"),
            },
        );
    }
    out
}

fn location_kind(t: &str) -> String {
    match t {
        "TEMPLE" => "temple".into(),
        "INN_TAVERN" => "tavern".into(),
        "GUILDHALL" => "guildhall".into(),
        "HOSPITAL" => "hospital".into(),
        "LIBRARY" => "library".into(),
        other => slug(other),
    }
}

/// The place kind of a building, or None when it is furniture rather than a
/// place.
fn place_kind(b: &Building, locs: &BTreeMap<i64, Location>) -> Option<String> {
    match b.kind.as_str() {
        "Civzone" => {
            if let Some(l) = b.location_id.and_then(|i| locs.get(&i)) {
                return Some(location_kind(&l.kind));
            }
            Some(match b.sub() {
                "Bedroom" => "bedroom".into(),
                "Dormitory" => "dormitory".into(),
                "DiningHall" => "dining".into(),
                "MeetingHall" => "meeting".into(),
                "Tomb" => "tomb".into(),
                "Office" => "office".into(),
                "Barracks" => "barracks".into(),
                "ArcheryRange" => "archery".into(),
                "Dungeon" => "dungeon".into(),
                "Pen" => "pen".into(),
                "Pond" => "pond".into(),
                "Dump" => "dump".into(),
                "" => "zone".into(),
                other => slug(&split_camel(other)),
            })
        }
        "Workshop" => Some(format!("workshop:{}", b.sub().to_lowercase())),
        "Furnace" => Some(format!("furnace:{}", b.sub().to_lowercase())),
        "Stockpile" => Some("stockpile".into()),
        "FarmPlot" => Some("farm".into()),
        "Well" => Some("well".into()),
        "TradeDepot" => Some("depot".into()),
        _ => None,
    }
}

const FOOD_SHOPS: &[&str] = &["Still", "Kitchen", "Farmers", "Butchers", "Fishery", "Quern", "Millstone", "Tanners"];

/// (slug, display name) of each level function, in tie-break order.
const FUNCTIONS: &[(&str, &str)] = &[
    ("the-dormitories", "The Dormitories"),
    ("the-workshops", "The Workshops"),
    ("the-forges", "The Forges"),
    ("the-farms", "The Farms and Stills"),
    ("the-great-hall", "The Great Hall"),
    ("the-temples", "The Temples"),
    ("the-taverns", "The Taverns"),
    ("the-guildhalls", "The Guildhalls"),
    ("the-hospital", "The Hospital"),
    ("the-library", "The Library"),
    ("the-catacombs", "The Catacombs"),
    ("the-barracks", "The Barracks"),
    ("the-pump-works", "The Pump Works"),
    ("the-noble-halls", "The Noble Halls"),
    ("the-stores", "The Stores"),
];

/// Which function a building argues for, and how strongly.
fn function_vote(b: &Building, locs: &BTreeMap<i64, Location>) -> Option<(usize, u32)> {
    if b.is("Civzone") {
        if let Some(l) = b.location_id.and_then(|i| locs.get(&i)) {
            return match l.kind.as_str() {
                "TEMPLE" => Some((5, 15)),
                "INN_TAVERN" => Some((6, 15)),
                "GUILDHALL" => Some((7, 15)),
                "HOSPITAL" => Some((8, 20)),
                "LIBRARY" => Some((9, 20)),
                _ => None,
            };
        }
        return match b.sub() {
            "Bedroom" => Some((0, 3)),
            "Dormitory" => Some((0, 5)),
            "DiningHall" => Some((4, 8)),
            "MeetingHall" => Some((4, 3)),
            "Tomb" => Some((10, 10)),
            "Office" => Some((13, 12)),
            "Dungeon" => Some((13, 4)),
            "Barracks" | "ArcheryRange" => Some((11, 10)),
            "Pen" => Some((3, 5)),
            "Pond" => Some((12, 2)),
            _ => None,
        };
    }
    Some(match b.kind.as_str() {
        "Bed" => (0, 1),
        "Table" => (4, 1),
        "Workshop" if FOOD_SHOPS.contains(&b.sub()) => (3, 10),
        "Workshop" if b.sub() == "MetalsmithsForge" => (2, 30),
        "Workshop" => (1, 10),
        "Furnace" => (2, 30),
        "FarmPlot" => (3, 10),
        "NestBox" => (3, 1),
        "OfferingPlace" => (5, 3),
        "Coffin" => (10, 3),
        "Bookcase" => (9, 1),
        "ScrewPump" | "Windmill" => (12, 3),
        "GearAssembly" => (12, 2),
        "AxleHorizontal" | "AxleVertical" => (12, 1),
        "Weaponrack" | "Armorstand" => (11, 1),
        "ArcheryTarget" => (11, 3),
        "TractionBench" => (8, 5),
        "Stockpile" => (14, 3),
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Walk,
    Block,
    Door,
    Cover,
    Shop,
    Center,
    Zone,
}

fn style(b: &Building) -> (u8, Mode) {
    use Mode::*;
    match b.kind.as_str() {
        "Door" => (b' ', Door),
        "Bed" => (b'b', Walk),
        "Table" => (b't', Walk),
        "Chair" => (b'c', Walk),
        "Cabinet" => (b'k', Walk),
        "Bookcase" => (b'K', Walk),
        "Box" | "Cage" => (b'h', Walk),
        "Armorstand" => (b'a', Walk),
        "Weaponrack" => (b'r', Walk),
        "Statue" | "Support" => (b's', Block),
        "Well" => (b'w', Block),
        "ScrewPump" => (b'P', Block),
        "GearAssembly" | "AxleHorizontal" | "AxleVertical" | "Rollers" | "WaterWheel" => (b'g', Block),
        "Windmill" => (b'm', Center),
        "Workshop" => (b'W', Shop),
        "Furnace" => (b'F', Shop),
        "TradeDepot" => (b'W', Walk),
        "FarmPlot" => (b'f', Walk),
        "Stockpile" => (b'p', Walk),
        "OfferingPlace" => (b'o', Walk),
        "Coffin" | "Slab" => (b'C', Walk),
        "DisplayFurniture" => (b'd', Walk),
        "NestBox" => (b'n', Walk),
        "Hatch" | "GrateFloor" | "Bridge" => (b'=', Cover),
        "GrateWall" | "BarsVertical" => (b'=', Block),
        "Trap" | "Chain" => (b'x', Walk),
        "ArcheryTarget" => (b'A', Block),
        "TractionBench" => (b'j', Walk),
        _ => (b' ', Zone),
    }
}

fn draw_order(b: &Building) -> u8 {
    match b.kind.as_str() {
        "Civzone" => 0,
        "Stockpile" | "FarmPlot" => 1,
        "Door" | "Hatch" | "GrateFloor" | "Bridge" => 3,
        _ => 2,
    }
}

/// A workshop's solid parts: the back (top) row and the middle of a 3x3, or
/// the middle tile of any other size.
fn shop_solid(b: &Building, x: i32, y: i32) -> bool {
    let (w, h) = (b.x2 - b.x1 + 1, b.y2 - b.y1 + 1);
    if w == 3 && h == 3 {
        y == b.y1 || (x == b.x1 + 1 && y == b.y1 + 1)
    } else {
        x == (b.x1 + b.x2) / 2 && y == (b.y1 + b.y2) / 2
    }
}

struct Ctx<'a> {
    ex: &'a Export,
    surface_z: i32,
    cls: &'a Classifier,
    legend: &'a BTreeMap<u8, (String, String)>,
}

impl Ctx<'_> {
    /// DF draws a tree's trunk as a wall of tree material; its `T` tiles are
    /// branches, twigs and canopy, which hang over open air.
    fn is_trunk(&self, z: i32, x: i32, y: i32) -> bool {
        self.ex.shape(z, x, y) == b'#' && self.ex.mat(z, x, y) == b'p'
    }

    fn deep(&self, z: i32, x: i32, y: i32) -> bool {
        matches!(self.ex.liquid(z, x, y), b'A'..=b'G' | b'4'..=b'7')
    }

    fn air(&self, z: i32, x: i32, y: i32) -> (u8, u8) {
        if z != self.surface_z {
            return (b'~', b' ');
        }
        // The surface level is a hillside: under open sky (or canopy), show
        // the ground below, so the upper world is one walkable map.
        for d in 1..=SKY_DEPTH {
            let zz = z - d;
            let t = self.ex.shape(zz, x, y);
            match t {
                b' ' | b'v' | b'T' => continue,
                b'#' if self.is_trunk(zz, x, y) => return (b'B', b'T'),
                b'?' => return (b'#', b' '),
                b'~' | b'O' => return (b'~', b' '),
                _ => {
                    if self.deep(zz, x, y) {
                        return (b'~', b' ');
                    }
                    return (b'.', if t == b'"' { b'"' } else { b' ' });
                }
            }
        }
        (b'~', b' ')
    }

    fn base(&self, z: i32, x: i32, y: i32) -> (u8, u8) {
        let s = self.ex.shape(z, x, y);
        if s != b'#' && s != b'?' && self.deep(z, x, y) {
            return (b'~', b' ');
        }
        match s {
            b'#' if self.is_trunk(z, x, y) => (b'B', b'T'),
            b'?' | b'#' | b'F' => (b'#', b' '),
            b'"' => (b'.', b'"'),
            b'T' => self.air(z, x, y),
            b'~' | b'O' => (b'~', b' '),
            b' ' => self.air(z, x, y),
            // DF stairs and ramps are plain floor; the game's own links are
            // chosen afterwards (`choose_links`) and only they are `>`.
            _ => (b'.', b' '),
        }
    }

    fn rock_class(&self, z: i32, x: i32, y: i32, fill: u8) -> u8 {
        match self.ex.shape(z, x, y) {
            b'?' => fill,
            b'#' => match self.ex.mat(z, x, y) {
                b's' => b's',
                b'l' => b'o',
                b'f' => b'a',
                b'r' | b'm' => match self.legend.get(&self.ex.rock(z, x, y)) {
                    Some((token, _)) => self.cls.classify(token),
                    None => b'r',
                },
                _ => b' ',
            },
            _ => b' ',
        }
    }

    /// The layer the unrevealed rock of a level most likely is: the commonest
    /// of soil, stone and flux among its revealed natural walls.
    fn fill_class(&self, z: i32) -> u8 {
        let Some(g) = self.ex.grids.get(&z) else { return b'r' };
        let mut n: BTreeMap<u8, usize> = BTreeMap::new();
        for y in 0..g.h as i32 {
            for x in 0..g.w as i32 {
                let c = self.rock_class(z, x, y, b' ');
                if matches!(c, b's' | b'r' | b'f') {
                    *n.entry(c).or_default() += 1;
                }
            }
        }
        n.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(c, _)| c).unwrap_or(b'r')
    }
}

struct Rendered {
    tiles: Vec<Vec<u8>>,
    dress: Vec<Vec<u8>>,
    rock: Vec<Vec<u8>>,
}

fn render(ctx: &Ctx, z: i32, crop: Crop, bs: &[&Building]) -> Rendered {
    let mut tiles = vec![vec![b'#'; crop.w]; crop.h];
    let mut dress = vec![vec![b' '; crop.w]; crop.h];
    let mut rock = vec![vec![b' '; crop.w]; crop.h];
    let fill = ctx.fill_class(z);
    for cy in 0..crop.h {
        for cx in 0..crop.w {
            let (x, y) = (crop.x0 + cx as i32, crop.y0 + cy as i32);
            let (t, d) = ctx.base(z, x, y);
            tiles[cy][cx] = t;
            dress[cy][cx] = d;
            rock[cy][cx] = ctx.rock_class(z, x, y, fill);
        }
    }
    let mut order: Vec<&&Building> = bs.iter().collect();
    order.sort_by_key(|b| (draw_order(b), b.id));
    for b in order {
        let (glyph, mode) = style(b);
        if mode == Mode::Zone {
            continue;
        }
        for (x, y) in b.cells() {
            let Some((cx, cy)) = crop.local(x, y) else { continue };
            if b.unfinished {
                dress[cy][cx] = b'+';
                continue;
            }
            if glyph != b' ' {
                dress[cy][cx] = glyph;
            }
            let t = &mut tiles[cy][cx];
            let solid = match mode {
                Mode::Block => true,
                Mode::Shop => shop_solid(b, x, y),
                Mode::Center => x == (b.x1 + b.x2) / 2 && y == (b.y1 + b.y2) / 2,
                _ => false,
            };
            match mode {
                Mode::Door => *t = b'D',
                Mode::Cover => {
                    if *t == b'~' || *t == b'#' {
                        *t = b'.';
                    }
                }
                _ if solid
                    && !matches!(*t, b'#' | b'>' | b'D') => {
                        *t = b'B';
                    }
                _ => {}
            }
        }
    }
    Rendered { tiles, dress, rock }
}

/// Wall the outermost ring of a level.
fn wall_border(r: &mut Rendered) {
    let h = r.tiles.len();
    for (cy, (trow, drow)) in r.tiles.iter_mut().zip(r.dress.iter_mut()).enumerate() {
        let w = trow.len();
        for cx in 0..w {
            if cx == 0 || cy == 0 || cx == w - 1 || cy == h - 1 {
                trow[cx] = b'#';
                drow[cx] = b' ';
            }
        }
    }
}

/// Wall every walkable tile the level's links cannot reach. Returns how many
/// were walled, and whether the links themselves are in separate regions.
fn wall_unreached(r: &mut Rendered, rects: &[Rect]) -> (usize, bool) {
    let seeds: Vec<(i32, i32)> = rects.iter().flat_map(|a| links::cells(*a)).collect();
    let split = match rects.first() {
        Some(first) => {
            let from_first = links::flood(&r.tiles, &links::cells(*first));
            rects.iter().any(|q| links::cells(*q).iter().all(|&(x, y)| !from_first[y as usize][x as usize]))
        }
        None => false,
    };
    let reach = links::flood(&r.tiles, &seeds);
    let mut n = 0;
    for (cy, row) in r.tiles.iter_mut().enumerate() {
        for (cx, t) in row.iter_mut().enumerate() {
            if links::walkable(*t) && !reach[cy][cx] {
                *t = b'#';
                r.dress[cy][cx] = b' ';
                n += 1;
            }
        }
    }
    // A statue, trunk or hole with no walkable tile around it is now inside
    // solid ground: wall it too.
    let (h, w) = (r.tiles.len(), r.tiles.first().map_or(0, |t| t.len()));
    let mut lone = Vec::new();
    for cy in 0..h {
        for cx in 0..w {
            if r.tiles[cy][cx] == b'#' || links::walkable(r.tiles[cy][cx]) {
                continue;
            }
            let mut near = false;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        near |= links::walkable(r.tiles[ny as usize][nx as usize]);
                    }
                }
            }
            if !near {
                lone.push((cx, cy));
            }
        }
    }
    for (cx, cy) in lone {
        r.tiles[cy][cx] = b'#';
        r.dress[cy][cx] = b' ';
    }
    (n, split)
}

fn rows(v: &[Vec<u8>]) -> Vec<String> {
    v.iter().map(|r| String::from_utf8(r.clone()).expect("ascii")).collect()
}

fn opt<T: Serialize>(m: &mut Map<String, Value>, k: &str, v: Option<T>) {
    if let Some(v) = v {
        m.insert(k.to_string(), serde_json::to_value(v).expect("json"));
    }
}

/// What a person does in the kingdom, in the building game's words.
fn job_from_position(code: &str) -> &'static str {
    match code {
        "MANAGER" | "BOOKKEEPER" | "BROKER" => "clerk",
        "CHIEF_MEDICAL_DWARF" => "doctor",
        c if c.contains("PRIEST") => "priest",
        c if c.starts_with("MILITIA") || c == "SHERIFF" || c == "CAPTAIN_OF_THE_GUARD" || c == "HAMMERER" || c == "CHAMPION" => {
            "guard"
        }
        _ => "noble",
    }
}

const PROFESSION_JOBS: &[(&str, &str)] = &[
    ("bowyer", "carpenter"),
    ("miner", "miner"),
    ("mason", "mason"),
    ("stone", "mason"),
    ("engraver", "mason"),
    ("gem", "jeweler"),
    ("jewel", "jeweler"),
    ("smith", "smith"),
    ("smelter", "smith"),
    ("forge", "smith"),
    ("armorer", "smith"),
    ("metal", "smith"),
    ("furnace", "smith"),
    ("cheese", "cook"),
    ("cook", "cook"),
    ("brewer", "brewer"),
    ("farmer", "farmer"),
    ("planter", "farmer"),
    ("grower", "farmer"),
    ("herbalist", "farmer"),
    ("thresher", "farmer"),
    ("miller", "farmer"),
    ("presser", "farmer"),
    ("carpenter", "carpenter"),
    ("wood", "carpenter"),
    ("mechanic", "mechanic"),
    ("pump", "mechanic"),
    ("siege", "mechanic"),
    ("doctor", "doctor"),
    ("surgeon", "doctor"),
    ("diagnos", "doctor"),
    ("bone setter", "doctor"),
    ("suturer", "doctor"),
    ("healer", "doctor"),
    ("butcher", "butcher"),
    ("tanner", "crafter"),
    ("fish", "fisher"),
    ("animal", "herder"),
    ("milker", "herder"),
    ("shearer", "herder"),
    ("trapper", "herder"),
    ("priest", "priest"),
    ("monk", "priest"),
    ("acolyte", "priest"),
    ("clerk", "clerk"),
    ("bookkeeper", "clerk"),
    ("administrator", "clerk"),
    ("trader", "clerk"),
    ("scribe", "clerk"),
    ("scholar", "scholar"),
    ("philosopher", "scholar"),
    ("historian", "scholar"),
    ("mathematician", "scholar"),
    ("astronomer", "scholar"),
    ("naturalist", "scholar"),
    ("chemist", "scholar"),
    ("geographer", "scholar"),
    ("librarian", "scholar"),
    ("bard", "performer"),
    ("poet", "performer"),
    ("dancer", "performer"),
    ("musician", "performer"),
    ("storyteller", "performer"),
    ("craft", "crafter"),
    ("weaver", "crafter"),
    ("clothier", "crafter"),
    ("tailor", "crafter"),
    ("leather", "crafter"),
    ("spinner", "crafter"),
    ("dyer", "crafter"),
    ("bone carver", "crafter"),
    ("glass", "crafter"),
    ("potter", "crafter"),
    ("glazer", "crafter"),
    ("soap", "crafter"),
    ("axe", "guard"),
    ("sword", "guard"),
    ("spear", "guard"),
    ("pike", "guard"),
    ("mace", "guard"),
    ("hammer", "guard"),
    ("marks", "guard"),
    ("bow", "guard"),
    ("recruit", "guard"),
    ("wrestler", "guard"),
    ("champion", "guard"),
    ("lasher", "guard"),
    ("whip", "guard"),
    ("soldier", "guard"),
    ("guard", "guard"),
    ("knight", "guard"),
    ("mercenary", "guard"),
    ("slayer", "guard"),
    ("hunter", "guard"),
    ("lord", "guard"),
    ("lady", "guard"),
    ("peasant", "hauler"),
    ("child", "hauler"),
    ("baby", "hauler"),
];

fn job_from_skill(s: &str) -> Option<&'static str> {
    Some(match s {
        "MINING" => "miner",
        "CUT_STONE" | "CARVE_STONE" | "ENGRAVE_STONE" | "MASONRY" => "mason",
        "SMELT" | "FORGE_WEAPON" | "FORGE_ARMOR" | "FORGE_FURNITURE" | "METALCRAFT" => "smith",
        "COOK" | "CHEESEMAKING" => "cook",
        "BREWING" => "brewer",
        "PLANT" | "HERBALISM" | "PROCESSPLANTS" | "MILLING" => "farmer",
        "CARPENTRY" | "BOWYER" | "WOODCUTTING" => "carpenter",
        "WOODCRAFT" | "STONECRAFT" | "BONECARVE" | "LEATHERWORK" | "CLOTHESMAKING" | "WEAVING" | "SPINNING" | "DYER"
        | "TANNER" | "GLASSMAKER" | "POTTERY" | "GLAZING" | "WAX_WORKING" | "SOAP_MAKING" | "LYE_MAKING"
        | "WOOD_BURNING" | "PAPERMAKING" | "BOOKBINDING" => "crafter",
        "CUTGEM" | "ENCRUSTGEM" => "jeweler",
        "MECHANICS" | "OPERATE_PUMP" | "SIEGECRAFT" | "SIEGEOPERATE" => "mechanic",
        "DIAGNOSE" | "SURGERY" | "SET_BONE" | "SUTURE" | "DRESS_WOUNDS" => "doctor",
        "RECORD_KEEPING" | "APPRAISAL" | "ORGANIZATION" | "WRITING" => "clerk",
        "BUTCHER" | "GELD" => "butcher",
        "FISH" | "PROCESSFISH" | "DISSECT_FISH" => "fisher",
        "ANIMALTRAIN" | "ANIMALCARE" | "MILK" | "SHEARING" | "TRAPPING" | "DISSECT_VERMIN" => "herder",
        "AXE" | "SWORD" | "MACE" | "HAMMER" | "SPEAR" | "CROSSBOW" | "BOW" | "DAGGER" | "PIKE" | "WHIP" | "BLOWGUN"
        | "THROW" | "MELEE_COMBAT" | "RANGED_COMBAT" | "WRESTLING" | "BITE" | "GRASP_STRIKE" | "STANCE_STRIKE"
        | "SHIELD" | "ARMOR" | "DODGING" | "MILITARY_TACTICS" | "MISC_WEAPON" => "guard",
        "MAKE_MUSIC" | "SING_MUSIC" | "DANCE" | "POETRY" | "PROSE" | "PLAY_KEYBOARD_INSTRUMENT"
        | "PLAY_STRINGED_INSTRUMENT" | "PLAY_WIND_INSTRUMENT" | "PLAY_PERCUSSION_INSTRUMENT" => "performer",
        _ => return None,
    })
}

/// Place kinds a home job works at, to find the level that is its district.
fn job_places(job: &str) -> &'static [&'static str] {
    match job {
        "mason" => &["workshop:masons"],
        "carpenter" => &["workshop:carpenters", "workshop:bowyers"],
        "crafter" => &[
            "workshop:craftsdwarfs",
            "workshop:clothiers",
            "workshop:leatherworks",
            "workshop:loom",
            "workshop:dyers",
            "workshop:tanners",
        ],
        "jeweler" => &["workshop:jewelers"],
        "mechanic" => &["workshop:mechanics", "workshop:siege"],
        "smith" => &[
            "workshop:metalsmithsforge",
            "furnace:smelter",
            "furnace:woodfurnace",
            "furnace:magmasmelter",
            "furnace:magmaforge",
        ],
        "cook" => &["workshop:kitchen"],
        "brewer" => &["workshop:still"],
        "farmer" => &["farm", "workshop:farmers", "workshop:quern", "workshop:millstone"],
        "butcher" => &["workshop:butchers", "workshop:tanners"],
        "fisher" => &["workshop:fishery"],
        "herder" => &["pen"],
        "doctor" => &["hospital"],
        "clerk" | "noble" => &["office"],
        "priest" => &["temple"],
        "guard" => &["barracks", "archery"],
        "performer" => &["tavern"],
        "scholar" => &["library"],
        _ => &[],
    }
}

fn spoil_of(ty: &str, mat: &str, ores: &BTreeSet<String>) -> Option<&'static str> {
    let m = mat.to_lowercase();
    if ty == "COIN" || m == "gold" {
        return Some("gold");
    }
    if ["bone", "tooth", "horn", "hoof", "ivory", "shell"].iter().any(|w| m.contains(w)) {
        return Some("bone");
    }
    if m.contains("wax") || m.contains("tallow") {
        return Some("wax");
    }
    if matches!(m.as_str(), "coal" | "charcoal" | "coke" | "bituminous coal" | "lignite") {
        return Some("ember");
    }
    if matches!(ty, "ROUGH" | "SMALLGEM" | "GEM") {
        return Some("gem");
    }
    if ty == "BOULDER" && ores.contains(&m) {
        return Some("ore");
    }
    if ty == "BAR" && !matches!(m.as_str(), "potash" | "pearlash" | "ash" | "unknown") && !m.contains("soap") {
        return Some("ore");
    }
    if m.contains("scale") {
        return Some("scale");
    }
    None
}

fn level_function(bs: &[&Building], locs: &BTreeMap<i64, Location>) -> Option<usize> {
    let mut score = vec![0u32; FUNCTIONS.len()];
    for b in bs {
        if let Some((f, w)) = function_vote(b, locs) {
            score[f] += w;
        }
    }
    let best = (0..FUNCTIONS.len()).max_by(|&a, &b| score[a].cmp(&score[b]).then(b.cmp(&a)))?;
    if score[best] == 0 { None } else { Some(best) }
}

fn trait_words(traits: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(m)) = traits else { return vec![] };
    let mut v: Vec<(i64, String, i64)> = m
        .iter()
        .filter_map(|(k, x)| x.as_i64().map(|n| ((n - 50).abs(), k.clone(), n)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter()
        .take(3)
        .map(|(_, k, n)| format!("{} {}", if n >= 50 { "high" } else { "low" }, k.to_lowercase().replace('_', " ")))
        .collect()
}

fn value_words(values: &[Value]) -> Vec<String> {
    let mut v: Vec<(i64, String, i64)> = values
        .iter()
        .filter_map(|x| Some((int_of(x, "strength")?.abs(), str_of(x, "value")?, int_of(x, "strength")?)))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    v.into_iter()
        .take(3)
        .map(|(_, k, n)| format!("{} {}", if n > 0 { "values" } else { "scorns" }, k.to_lowercase().replace('_', " ")))
        .collect()
}

/// The modded races of "Additional Races", by the mod's name.
const MODDED: &[(&str, &str)] = &[
    ("Additional Races. Gnolls", "GNOLL"),
    ("Additional Races. Naga", "NAGA"),
    ("Additional Races. Ratfolk", "RATFOLK"),
    ("Additional Races. Erinyes", "VALKYRIE"),
];
const NOTABLE: &[&str] = &[
    "DWARF",
    "HUMAN",
    "ELF",
    "GOBLIN",
    "KOBOLD",
    "GIANT",
    "ETTIN",
    "CYCLOPS",
    "TROLL",
    "OGRE",
    "BLIND_CAVE_OGRE",
    "TROGLODYTE",
    "MINOTAUR",
];

pub struct Summary {
    pub lines: Vec<String>,
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    v.serialize(&mut ser).map_err(|e| e.to_string())?;
    buf.push(b'\n');
    fs::write(path, buf).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run(input_dir: &Path, kingdom_dir: &Path, game_dir: &Path) -> Result<Summary, String> {
    let ex = input::load(input_dir)?;
    let root = &ex.root;
    let mut lines = Vec::new();
    let locs = locations(root);
    let cls = Classifier::new(&ex.prospect);
    let tiles_meta = root.get("tiles").cloned().unwrap_or(Value::Null);
    let mut legend: BTreeMap<u8, (String, String)> = BTreeMap::new();
    for r in arr(&tiles_meta, "rock_legend") {
        if let (Some(sym), Some(tok)) = (str_of(r, "sym"), str_of(r, "token"))
            && let Some(&b) = sym.as_bytes().first() {
                legend.insert(b, (tok, str_of(r, "name").unwrap_or_default()));
            }
    }

    let mut by_z: BTreeMap<i32, Vec<&Building>> = BTreeMap::new();
    for b in &ex.buildings {
        by_z.entry(b.z).or_default().push(b);
    }

    // 1. Which levels qualify (built, or dug), and which are kept: the
    //    surface entrance and the ones with the most fortress content.
    let mut stats: BTreeMap<i32, ZStat> = BTreeMap::new();
    for (&z, g) in &ex.grids {
        let mut s = ZStat { buildings: by_z.get(&z).map_or(0, |v| v.len()), ..Default::default() };
        for i in 0..g.w * g.h {
            let (sh, m) = (g.shape[i], g.mat[i]);
            match sh {
                b'.' => s.floors += 1,
                b'T' | b'"' => s.veg += 1,
                _ if is_stair(sh) => s.stairs += 1,
                _ => {}
            }
            if m == b'g' || m == b'o' {
                s.veg += 1;
            }
        }
        stats.insert(z, s);
    }
    // The surface, where the upper world comes in: the trade depot's level
    // (one the export has tiles for), else the highest level with stairs.
    let surface_z = ex
        .buildings
        .iter()
        .filter(|b| b.is("TradeDepot") && stats.contains_key(&b.z))
        .map(|b| b.z)
        .min()
        .or_else(|| stats.iter().filter(|(_, s)| s.stairs > 0).map(|(z, _)| *z).max())
        .ok_or("no surface level: no trade depot and no stairs")?;
    let qualifying: BTreeSet<i32> = stats
        .iter()
        .filter(|(z, s)| s.buildings >= 3 || (s.stairs > 0 && s.floors >= 300 && s.veg == 0) || **z == surface_z)
        .map(|(z, _)| *z)
        .collect();
    let content = |z: i32| -> u32 {
        let votes: u32 = by_z.get(&z).into_iter().flatten().filter_map(|b| function_vote(b, &locs)).map(|(_, w)| w).sum();
        // Every building counts a little, so a richly furnished hall outranks
        // a sparse one of the same function.
        votes + stats[&z].buildings as u32 / 4
    };
    let mut ranked: Vec<(u32, i32)> =
        qualifying.iter().filter(|z| **z != surface_z).map(|&z| (content(z), z)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut playable: BTreeSet<i32> = BTreeSet::new();
    playable.insert(surface_z);
    for &(_, z) in ranked.iter().take(MAX_LEVELS - 1) {
        playable.insert(z);
    }
    // The building game's industries must have a level each, whatever their
    // content score: the best-ranked level of each such function is kept
    // too, and the crop grows to hold them.
    let function_of = |z: i32| level_function(by_z.get(&z).map(|v| v.as_slice()).unwrap_or(&[]), &locs);
    let mut extra: BTreeSet<i32> = BTreeSet::new();
    for &f in MUST_KEEP {
        if playable.iter().any(|&z| z != surface_z && function_of(z) == Some(f)) {
            continue;
        }
        if let Some(&(_, z)) = ranked.iter().find(|&&(_, z)| function_of(z) == Some(f)) {
            playable.insert(z);
            extra.insert(z);
            lines.push(format!("kept z{z} {} for the building game", FUNCTIONS[f].1));
        }
    }
    let mut dropped = Vec::new();
    for &(c, z) in ranked.iter().filter(|(_, z)| !playable.contains(z)) {
        let bs = by_z.get(&z).map(|v| v.as_slice()).unwrap_or(&[]);
        let f = level_function(bs, &locs).map(|f| FUNCTIONS[f].1).unwrap_or("The Mines");
        dropped.push(json!({"z": z, "function": f, "content": c, "buildings": stats[&z].buildings}));
        lines.push(format!("dropped z{z} {f} (content {c}, {} buildings)", stats[&z].buildings));
    }

    // 2. One crop for every level, so stairs line up.
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut grow = |x: i32, y: i32| {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    };
    for &z in &playable {
        for b in by_z.get(&z).into_iter().flatten() {
            for (x, y) in b.cells() {
                grow(x, y);
            }
        }
        let g = &ex.grids[&z];
        let underground = stats[&z].veg < 50;
        for y in 0..g.h as i32 {
            for x in 0..g.w as i32 {
                let (sh, m) = (g.shape(x, y), g.mat(x, y));
                let dug = is_stair(sh)
                    || m == b'c'
                    || (underground && matches!(sh, b'.' | b'^' | b'v' | b'F' | b','));
                if dug {
                    grow(x, y);
                }
            }
        }
    }
    if x0 > x1 {
        return Err("nothing dug or built on the playable levels".into());
    }
    let x0 = (x0 - PAD).max(0);
    let y0 = (y0 - PAD).max(0);
    let x1 = (x1 + PAD).min(ex.map_w - 1);
    let y1 = (y1 + PAD).min(ex.map_h - 1);
    if x0 > x1 || y0 > y1 {
        return Err(format!("the fortress (x {x0}..{x1} y {y0}..{y1}) lies off the {} x {} map", ex.map_w, ex.map_h));
    }
    let crop = Crop { x0, y0, w: (x1 - x0 + 1) as usize, h: (y1 - y0 + 1) as usize };
    lines.push(format!("crop: x {x0}..{x1} y {y0}..{y1} = {} x {} tiles{}", crop.w, crop.h, crop.size_note()));

    // 3. Names, from each level's dominant function, numbered top-down when
    //    several levels share one.
    let zs_top_down: Vec<i32> = playable.iter().rev().copied().collect();
    let mut base_names: BTreeMap<i32, (String, String)> = BTreeMap::new();
    for &z in &zs_top_down {
        let bs = by_z.get(&z).map(|v| v.as_slice()).unwrap_or(&[]);
        let (s, n) = if z == surface_z {
            ("the-gate", "The Gate")
        } else {
            match level_function(bs, &locs) {
                Some(f) => FUNCTIONS[f],
                None => ("the-mines", "The Mines"),
            }
        };
        base_names.insert(z, (s.to_string(), n.to_string()));
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut total: BTreeMap<String, usize> = BTreeMap::new();
    for (s, _) in base_names.values() {
        *total.entry(s.clone()).or_default() += 1;
    }
    let mut names: BTreeMap<i32, (String, String)> = BTreeMap::new();
    for &z in &zs_top_down {
        let (s, n) = &base_names[&z];
        if total[s] > 1 {
            let k = seen.entry(s.clone()).or_default();
            *k += 1;
            names.insert(z, (format!("{s}-{k}"), format!("{n} {}", text::roman(*k))));
        } else {
            names.insert(z, (s.clone(), n.clone()));
        }
    }
    let slug_of = |z: i32| names.get(&z).map(|n| n.0.clone());

    // 4. The levels: render, border, links, entrance, one walkable region.
    let ctx = Ctx { ex: &ex, surface_z, cls: &cls, legend: &legend };
    let mut rend: BTreeMap<i32, Rendered> = BTreeMap::new();
    for &z in &zs_top_down {
        let bs = by_z.get(&z).map(|v| v.as_slice()).unwrap_or(&[]);
        let mut r = render(&ctx, z, crop, bs);
        wall_border(&mut r);
        rend.insert(z, r);
    }
    let stair_mask = |z: i32| -> Vec<Vec<bool>> {
        (0..crop.h)
            .map(|cy| (0..crop.w).map(|cx| is_stair(ex.shape(z, crop.x0 + cx as i32, crop.y0 + cy as i32))).collect())
            .collect()
    };
    let center = (crop.w as i32 / 2, crop.h as i32 / 2);
    let mut used: BTreeMap<i32, Vec<Rect>> = BTreeMap::new();
    // (to z, rect on this level, shares the DF staircase with the other end,
    //  deep stair)
    let mut link_list: BTreeMap<i32, Vec<(i32, Rect, bool, bool)>> = BTreeMap::new();
    // Pairs with the fewest shared stair tiles choose first, so a narrow
    // real shaft is not taken by a neighbour's link on the wide main stair.
    let masks: BTreeMap<i32, Vec<Vec<bool>>> = zs_top_down.iter().map(|&z| (z, stair_mask(z))).collect();
    // The stair tiles of pair k (level k and the one below it) in common.
    let commons: Vec<Vec<Vec<bool>>> = zs_top_down
        .windows(2)
        .map(|p| {
            let (ma, mb) = (&masks[&p[0]], &masks[&p[1]]);
            ma.iter().zip(mb).map(|(ra, rb)| ra.iter().zip(rb).map(|(p, q)| *p && *q).collect()).collect()
        })
        .collect();
    let mut pair_order: Vec<(usize, usize)> =
        commons.iter().enumerate().map(|(k, c)| (c.iter().flatten().filter(|c| **c).count(), k)).collect();
    pair_order.sort();
    for (_, k) in pair_order {
        let (a, b) = (zs_top_down[k], zs_top_down[k + 1]);
        let (ma, mb) = (&masks[&a], &masks[&b]);
        let common = &commons[k];
        // A link must open onto the level's floor, not a stair sealed in rock.
        let fits = |z: i32, r: Rect| {
            links::all_floor(&rend[&z].tiles, r)
                && !used.get(&z).into_iter().flatten().any(|u| links::overlaps(*u, r))
                && links::flood(&rend[&z].tiles, &links::cells(r)).iter().flatten().filter(|c| **c).count() >= 25
        };
        let (ra, rb, aligned) = match links::pick(common, k, center, |r| fits(a, r) && fits(b, r)) {
            Some(r) => (r, r, true),
            None => {
                let one = |z: i32, m: &[Vec<bool>]| -> Result<Rect, String> {
                    links::pick(m, k, center, |r| fits(z, r))
                        .or_else(|| {
                            let avoid = used.get(&z).cloned().unwrap_or_default();
                            links::free_block(&rend[&z].tiles, &rend[&z].dress, (2, 2), center, &avoid, 1, None)
                        })
                        .ok_or_else(|| format!("z{z}: no room for a link"))
                };
                let (ra, rb) = (one(a, ma)?, one(b, mb)?);
                lines.push(format!(
                    "link {} <-> {}: no staircase shared at the same x,y; each end uses its own level's stairs",
                    names[&a].0, names[&b].0
                ));
                (ra, rb, false)
            }
        };
        // A shaft: the DF stairs run unbroken through every level between
        // the two ends at one of the link's tiles. Otherwise the link is a
        // "deep stair", the game's shortcut where the fort has none.
        let shaft = aligned
            && links::cells(ra).iter().any(|&(x, y)| {
                (b + 1..a).all(|zz| ex.shape(zz, crop.x0 + x, crop.y0 + y) == b'X')
                    && matches!(ex.shape(a, crop.x0 + x, crop.y0 + y), b'X' | b'>')
                    && matches!(ex.shape(b, crop.x0 + x, crop.y0 + y), b'X' | b'<')
            });
        if !shaft {
            lines.push(format!("link {} <-> {}: deep stair (no DF shaft runs between them)", names[&a].0, names[&b].0));
        }
        used.entry(a).or_default().push(ra);
        used.entry(b).or_default().push(rb);
        link_list.entry(a).or_default().push((b, ra, aligned, !shaft));
        link_list.entry(b).or_default().push((a, rb, aligned, !shaft));
    }
    for v in link_list.values_mut() {
        v.sort_by_key(|l| std::cmp::Reverse(l.0));
    }
    for (z, rs) in &used {
        let r = rend.get_mut(z).unwrap();
        for &rc in rs {
            for (x, y) in links::cells(rc) {
                r.tiles[y as usize][x as usize] = b'>';
            }
        }
    }
    // The entrance from the upper world: a 3x3 of clear floor on the surface
    // nearest the trade depot, reachable from the level's stairs.
    let top = surface_z;
    let anchor = ex
        .buildings
        .iter()
        .filter(|b| b.is("TradeDepot") && b.z == top)
        .min_by_key(|b| b.id)
        .and_then(|b| crop.local((b.x1 + b.x2) / 2, (b.y1 + b.y2) / 2))
        .map(|(x, y)| (x as i32, y as i32))
        .unwrap_or(center);
    let entrance = {
        let r = &rend[&top];
        let avoid = used.get(&top).cloned().unwrap_or_default();
        let seeds: Vec<(i32, i32)> = avoid.iter().flat_map(|a| links::cells(*a)).collect();
        let reach = links::flood(&r.tiles, &seeds);
        links::free_block(&r.tiles, &r.dress, (3, 3), anchor, &avoid, 2, if seeds.is_empty() { None } else { Some(&reach) })
            .ok_or("no room for the surface entrance")?
    };
    {
        let r = rend.get_mut(&top).unwrap();
        for (x, y) in links::cells(entrance) {
            r.tiles[y as usize][x as usize] = b'>';
        }
        used.entry(top).or_default().push(entrance);
    }
    let mut walled: BTreeMap<i32, usize> = BTreeMap::new();
    let mut split: BTreeSet<i32> = BTreeSet::new();
    for &z in &zs_top_down {
        let rects = used.get(&z).cloned().unwrap_or_default();
        let (n, s) = wall_unreached(rend.get_mut(&z).unwrap(), &rects);
        *walled.entry(z).or_default() += n;
        if s {
            split.insert(z);
        }
    }

    // 5. Crop again, to what can be walked to below the surface (outdoor
    //    slopes and pockets were just walled) plus the surface's own links
    //    and entrance, padded the same way. The open hillside of the
    //    surface beyond that is cut off.
    let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    let mut take = |cx: usize, cy: usize| {
        bx0 = bx0.min(cx);
        by0 = by0.min(cy);
        bx1 = bx1.max(cx);
        by1 = by1.max(cy);
    };
    for (z, r) in &rend {
        if *z == top && rend.len() > 1 {
            continue;
        }
        for (cy, row) in r.tiles.iter().enumerate() {
            for (cx, &t) in row.iter().enumerate() {
                if links::walkable(t) {
                    take(cx, cy);
                }
            }
        }
    }
    for q in used.iter().filter(|(z, _)| **z == top || extra.contains(z)).flat_map(|(_, v)| v) {
        for (x, y) in links::cells(*q) {
            take(x as usize, y as usize);
        }
    }
    let nx0 = bx0.saturating_sub(PAD as usize);
    let ny0 = by0.saturating_sub(PAD as usize);
    let nx1 = (bx1 + PAD as usize).min(crop.w - 1);
    let ny1 = (by1 + PAD as usize).min(crop.h - 1);
    for r in rend.values_mut() {
        for grid in [&mut r.tiles, &mut r.dress, &mut r.rock] {
            *grid = grid[ny0..=ny1].iter().map(|row| row[nx0..=nx1].to_vec()).collect();
        }
        wall_border(r);
    }
    let shift = |q: Rect| (q.0 - nx0 as i32, q.1 - ny0 as i32, q.2, q.3);
    for v in used.values_mut() {
        for q in v.iter_mut() {
            *q = shift(*q);
        }
    }
    for v in link_list.values_mut() {
        for l in v.iter_mut() {
            l.1 = shift(l.1);
        }
    }
    let entrance = shift(entrance);
    let crop = Crop { x0: crop.x0 + nx0 as i32, y0: crop.y0 + ny0 as i32, w: nx1 - nx0 + 1, h: ny1 - ny0 + 1 };
    // The cut may have severed some of the surface: wall what is now cut off.
    for &z in &zs_top_down {
        let rects = used.get(&z).cloned().unwrap_or_default();
        let (n, s) = wall_unreached(rend.get_mut(&z).unwrap(), &rects);
        *walled.entry(z).or_default() += n;
        if s {
            split.insert(z);
        }
    }
    for z in &split {
        lines.push(format!("WARNING {}: its links are not joined by walkable floor", names[z].0));
    }
    lines.push(format!(
        "final crop to the walkable fortress: df x {}..{} y {}..{} = {} x {} tiles{}",
        crop.x0,
        crop.x0 + crop.w as i32 - 1,
        crop.y0,
        crop.y0 + crop.h as i32 - 1,
        crop.w,
        crop.h,
        crop.size_note()
    ));

    let levels_dir = game_dir.join("levels");
    fs::create_dir_all(&levels_dir).map_err(|e| format!("{}: {e}", levels_dir.display()))?;
    let mut level_json = Vec::new();
    let mut seams = Map::new();
    let mut res_levels = Map::new();
    let mut overall_classes: BTreeMap<String, u64> = BTreeMap::new();
    let mut overall_minerals: BTreeMap<String, u64> = BTreeMap::new();
    let mut written = BTreeSet::new();
    for &z in &zs_top_down {
        let (lslug, lname) = names[&z].clone();
        let bs = by_z.get(&z).map(|v| v.as_slice()).unwrap_or(&[]);
        let r = &rend[&z];
        let file = LevelFile {
            slug: lslug.clone(),
            name: lname.clone(),
            z,
            w: crop.w,
            h: crop.h,
            origin: (crop.x0, crop.y0),
            tiles: rows(&r.tiles),
            dress: rows(&r.dress),
            rock: rows(&r.rock),
        };
        let fname = format!("{lslug}.txt");
        fs::write(levels_dir.join(&fname), file.to_text()).map_err(|e| e.to_string())?;
        written.insert(fname.clone());

        let rects = used.get(&z).cloned().unwrap_or_default();
        let rj = |q: Rect| json!({"x": q.0, "y": q.1, "w": q.2, "h": q.3});
        let arrive_of = |q: Rect| -> Value {
            match links::arrive(&r.tiles, q, &rects) {
                Some((x, y)) => json!({"x": x, "y": y}),
                None => Value::Null,
            }
        };
        let mut link_vals = Vec::new();
        for &(to, q, aligned, deep) in link_list.get(&z).into_iter().flatten() {
            let arrive = arrive_of(q);
            if arrive.is_null() {
                lines.push(format!("WARNING {lslug}: link to z{to} has no 5-wide arrival floor"));
            }
            link_vals.push(json!({
                "to": slug_of(to),
                "direction": if to > z { "up" } else { "down" },
                "at": rj(q),
                "arrive": arrive,
                "same_xy_as_other_end": aligned,
                "deep_stair": deep,
            }));
        }
        let entrance_val = if z == top {
            let a = arrive_of(entrance);
            json!({"at": rj(entrance), "arrive": a, "near": "trade depot"})
        } else {
            Value::Null
        };
        let mut df_stairs = 0;
        for cy in 0..crop.h as i32 {
            for cx in 0..crop.w as i32 {
                if is_stair(ex.shape(z, crop.x0 + cx, crop.y0 + cy)) {
                    df_stairs += 1;
                }
            }
        }
        let count = |f: &dyn Fn(&Building) -> bool| bs.iter().filter(|b| f(b)).count();
        let mut counts = BTreeMap::new();
        counts.insert("beds", count(&|b| b.is("Bed")));
        counts.insert("bedrooms", count(&|b| b.is("Civzone") && b.sub() == "Bedroom"));
        counts.insert("tables", count(&|b| b.is("Table")));
        counts.insert("chairs", count(&|b| b.is("Chair")));
        counts.insert("doors", count(&|b| b.is("Door")));
        counts.insert("statues", count(&|b| b.is("Statue")));
        counts.insert("cabinets", count(&|b| b.is("Cabinet")));
        counts.insert("bookcases", count(&|b| b.is("Bookcase")));
        counts.insert("boxes", count(&|b| b.is("Box")));
        counts.insert("workshops", count(&|b| b.is("Workshop")));
        counts.insert("furnaces", count(&|b| b.is("Furnace")));
        counts.insert("stockpiles", count(&|b| b.is("Stockpile")));
        counts.insert("farm_plots", count(&|b| b.is("FarmPlot")));
        counts.insert("wells", count(&|b| b.is("Well")));
        counts.insert("coffins", count(&|b| b.is("Coffin")));
        counts.insert("altars", count(&|b| b.is("OfferingPlace")));
        counts.insert(
            "machines",
            count(&|b| {
                matches!(b.kind.as_str(), "ScrewPump" | "GearAssembly" | "AxleHorizontal" | "AxleVertical" | "Windmill")
            }),
        );
        counts.insert("zones", count(&|b| b.is("Civzone")));
        counts.insert("unfinished", count(&|b| b.unfinished));
        counts.insert("df_stair_tiles", df_stairs);
        let counts: BTreeMap<&str, usize> =
            counts.into_iter().filter(|(k, v)| *v > 0 || *k == "beds" || *k == "workshops").collect();

        // Seams: what the walls are made of, in the crop.
        let mut classes: BTreeMap<String, u64> = BTreeMap::new();
        let mut minerals: BTreeMap<String, u64> = BTreeMap::new();
        for cy in 0..crop.h {
            for cx in 0..crop.w {
                let c = r.rock[cy][cx];
                if c == b' ' {
                    continue;
                }
                *classes.entry(rocks::class_word(c).to_string()).or_default() += 1;
                let (x, y) = (crop.x0 + cx as i32, crop.y0 + cy as i32);
                if ex.shape(z, x, y) == b'#'
                    && let Some((_, name)) = legend.get(&ex.rock(z, x, y)) {
                        *minerals.entry(name.clone()).or_default() += 1;
                    }
            }
        }
        for (k, v) in &classes {
            *overall_classes.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &minerals {
            *overall_minerals.entry(k.clone()).or_default() += v;
        }
        let top_of = |m: &BTreeMap<String, u64>, n: usize| -> Vec<Value> {
            let mut v: Vec<(&String, &u64)> = m.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            v.into_iter().take(n).map(|(k, c)| json!({"name": k, "tiles": c})).collect()
        };
        let top_minerals: Vec<Value> = {
            let ore_or_gem: BTreeMap<String, u64> = minerals
                .iter()
                .filter(|(name, _)| {
                    legend
                        .values()
                        .find(|(_, n)| n == *name)
                        .is_some_and(|(tok, _)| !matches!(cls.classify(tok), b'r' | b's'))
                })
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            top_of(&ore_or_gem, 12)
        };
        seams.insert(
            lslug.clone(),
            json!({
                "z": z,
                "classes": classes,
                "top_minerals": top_minerals,
                "top_named": top_of(&minerals, 8),
            }),
        );
        res_levels.insert(lslug.clone(), json!(classes));
        let mut lv = json!({
            "slug": lslug,
            "name": lname,
            "z": z,
            "file": format!("levels/{fname}"),
            "links": link_vals,
            "walled_pockets": walled.get(&z).copied().unwrap_or(0),
            "counts": counts,
        });
        if !entrance_val.is_null() {
            lv["entrance"] = entrance_val;
        }
        level_json.push(lv);
        lines.push(format!(
            "level z{z:>3} {lslug:<18} {lname:<22} {}x{} beds {:>3} workshops {:>2} furnaces {} links {} walled {}",
            crop.w,
            crop.h,
            counts.get("beds").unwrap_or(&0),
            counts.get("workshops").unwrap_or(&0),
            counts.get("furnaces").unwrap_or(&0),
            link_list.get(&z).map_or(0, |l| l.len()),
            walled.get(&z).copied().unwrap_or(0),
        ));
    }
    // Level files from an earlier run that this run did not write.
    if let Ok(rd) = fs::read_dir(&levels_dir) {
        let mut stale: Vec<_> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".txt") && !written.contains(n))
            .collect();
        stale.sort();
        for n in stale {
            let _ = fs::remove_file(levels_dir.join(&n));
            lines.push(format!("removed stale levels/{n}"));
        }
    }

    // 5. Places.
    let citizens = arr(root, "citizens");
    let mut name_of_unit: BTreeMap<i64, String> = BTreeMap::new();
    for c in citizens {
        if let (Some(id), Some(n)) = (int_of(c, "id"), str_of(c, "name_english")) {
            name_of_unit.insert(id, n);
        }
    }
    let folk = root.get("folk").cloned().unwrap_or(Value::Null);
    for p in arr(&folk, "people") {
        if let (Some(id), Some(n)) = (int_of(p, "id"), str_of(p, "name_english")) {
            name_of_unit.entry(id).or_insert(n);
        }
    }
    struct Place {
        id: String,
        kind: String,
        z: i32,
        rect: (i32, i32, i32, i32),
        json: Map<String, Value>,
    }
    let mut places: Vec<Place> = Vec::new();
    // Every place id given so far: a location's first zone takes the
    // location's slug, its others (and a second location of the same name)
    // add their building id.
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut unreachable_places = 0usize;
    for &z in playable.iter().rev() {
        let mut bs: Vec<&Building> = by_z.get(&z).cloned().unwrap_or_default();
        bs.sort_by_key(|b| b.id);
        for b in bs {
            let Some(kind) = place_kind(b, &locs) else { continue };
            let loc = b.location_id.and_then(|i| locs.get(&i).map(|l| (i, l)));
            let id = match loc {
                Some((li, l)) => {
                    let base = slug(&l.name_english);
                    let base = if base.is_empty() { format!("{}-{}", slug(&kind), li) } else { base };
                    if taken.contains(&base) { format!("{base}-{}", b.id) } else { base }
                }
                None => format!("{}-{}", slug(&kind.replace(':', "-")), b.id),
            };
            taken.insert(id.clone());
            let owner_name = b.owner.and_then(|o| name_of_unit.get(&o).cloned());
            let name = match (loc, &owner_name) {
                (Some((_, l)), _) if !l.name_english.is_empty() => l.name_english.clone(),
                (_, Some(o)) if kind == "bedroom" => format!("{o}'s Bedroom"),
                _ if b.name.starts_with("Activity Zone") => {
                    let num = b.name.rsplit('#').next().unwrap_or("").to_string();
                    format!("{} #{num}", split_camel(b.sub()))
                }
                _ => b.name.clone(),
            };
            let Some((rx, ry)) = crop.local(b.x1, b.y1) else { continue };
            let rect = (rx as i32, ry as i32, b.x2 - b.x1 + 1, b.y2 - b.y1 + 1);
            let mut m = Map::new();
            m.insert("id".into(), json!(id));
            m.insert("name".into(), json!(name));
            m.insert("kind".into(), json!(kind));
            m.insert("level".into(), json!(slug_of(z)));
            m.insert("rect".into(), json!({"x": rect.0, "y": rect.1, "w": rect.2, "h": rect.3}));
            m.insert("df_building_id".into(), json!(b.id));
            if let Some((li, l)) = loc {
                m.insert("location".into(), json!(l.name_english));
                m.insert("location_native".into(), json!(l.name));
                m.insert("location_id".into(), json!(li));
                opt(&mut m, "deity", l.deity.clone());
                if let Some(r) = &l.religion {
                    opt(&mut m, "religion", r.name_english.clone());
                    opt(&mut m, "religion_native", r.name.clone());
                    opt(&mut m, "religion_entity", r.entity);
                }
                opt(&mut m, "guild_profession", l.profession.as_ref().map(|p| p.to_lowercase()));
            }
            opt(&mut m, "owner", b.owner);
            opt(&mut m, "owner_name", owner_name);
            opt(&mut m, "material", b.material.clone());
            opt(&mut m, "quality", b.quality);
            if b.unfinished {
                m.insert("unfinished".into(), json!(true));
            }
            // Reachable: a tile of it, or one beside it, is walkable floor
            // that survived the flood fill.
            let tl = &rend[&z].tiles;
            let walk = |x: i32, y: i32| crop.local(x, y).is_some_and(|(cx, cy)| links::walkable(tl[cy][cx]));
            let reachable =
                b.cells().iter().any(|&(x, y)| walk(x, y) || walk(x - 1, y) || walk(x + 1, y) || walk(x, y - 1) || walk(x, y + 1));
            if !reachable {
                m.insert("reachable".into(), json!(false));
                unreachable_places += 1;
            }
            places.push(Place { id, kind, z, rect, json: m });
        }
    }

    // Bedroom blocks: rooms that touch, by row of rooms.
    let mut blocks = Vec::new();
    for &z in playable.iter().rev() {
        let idx: Vec<usize> = (0..places.len()).filter(|&i| places[i].z == z && places[i].kind == "bedroom").collect();
        let n = idx.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while p[i] != i {
                p[i] = p[p[i]];
                i = p[i];
            }
            i
        }
        for a in 0..n {
            for b in a + 1..n {
                let (ra, rb) = (places[idx[a]].rect, places[idx[b]].rect);
                let touch = ra.0 - 1 <= rb.0 + rb.2 && rb.0 - 1 <= ra.0 + ra.2 && ra.1 - 1 <= rb.1 + rb.3 && rb.1 - 1 <= ra.1 + ra.3;
                if touch {
                    let (fa, fb) = (find(&mut parent, a), find(&mut parent, b));
                    if fa != fb {
                        parent[fa.max(fb)] = fa.min(fb);
                    }
                }
            }
        }
        let mut groups: BTreeMap<(usize, i32), Vec<usize>> = BTreeMap::new();
        for a in 0..n {
            let root = find(&mut parent, a);
            groups.entry((root, places[idx[a]].rect.1)).or_default().push(idx[a]);
        }
        let mut gl: Vec<Vec<usize>> = groups.into_values().collect();
        for g in &mut gl {
            g.sort_by_key(|&i| (places[i].rect.0, places[i].rect.1));
        }
        gl.sort_by_key(|g| (places[g[0]].rect.1, places[g[0]].rect.0));
        for g in gl {
            let k = blocks.len() + 1;
            let bid = format!("bedroom-block-{k}");
            let (mut bx0, mut by0, mut bx1, mut by1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            let mut owned = 0;
            let mut ids = Vec::new();
            for &i in &g {
                let r = places[i].rect;
                bx0 = bx0.min(r.0);
                by0 = by0.min(r.1);
                bx1 = bx1.max(r.0 + r.2 - 1);
                by1 = by1.max(r.1 + r.3 - 1);
                if places[i].json.contains_key("owner") {
                    owned += 1;
                }
                places[i].json.insert("block".into(), json!(bid));
                ids.push(places[i].id.clone());
            }
            blocks.push(json!({
                "id": bid,
                "name": format!("Bedroom Block {k}"),
                "level": slug_of(z),
                "rect": {"x": bx0, "y": by0, "w": bx1 - bx0 + 1, "h": by1 - by0 + 1},
                "rooms": ids.len(),
                "owned": owned,
                "bedrooms": ids,
            }));
        }
    }
    let mut place_counts: BTreeMap<String, usize> = BTreeMap::new();
    for p in &places {
        *place_counts.entry(p.kind.clone()).or_default() += 1;
    }
    lines.push(format!(
        "places: {} ({} bedrooms in {} blocks; {} unreachable)",
        places.len(),
        place_counts.get("bedroom").unwrap_or(&0),
        blocks.len(),
        unreachable_places
    ));

    // Where each kind of work is done: level with the most such places.
    let district_of = |job: &str| -> Option<String> {
        if job == "miner" {
            return playable
                .iter()
                .rev()
                .filter(|z| names[z].0.starts_with("the-mines"))
                .max_by(|a, b| stats[a].floors.cmp(&stats[b].floors).then(a.cmp(b)))
                // No mine level kept: the deepest level, nearest the dug rock.
                .or_else(|| playable.iter().next())
                .and_then(|z| slug_of(*z));
        }
        let want = job_places(job);
        if want.is_empty() {
            return None;
        }
        let mut n: BTreeMap<i32, usize> = BTreeMap::new();
        for p in &places {
            if want.contains(&p.kind.as_str()) {
                *n.entry(p.z).or_default() += 1;
            }
        }
        n.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0))).and_then(|(z, _)| slug_of(z))
    };

    // 6. Artifacts.
    let mut artifacts = Vec::new();
    for a in arr(root, "artifacts") {
        let mut m = Map::new();
        m.insert("name".into(), json!(str_of(a, "name_english").or_else(|| str_of(a, "name"))));
        opt(&mut m, "name_native", str_of(a, "name"));
        opt(&mut m, "item", str_of(a, "item").map(|s| text::clean_item(&s)));
        opt(&mut m, "type", str_of(a, "type").map(|s| s.to_lowercase()));
        opt(&mut m, "material", str_of(a, "material"));
        if let Some(mk) = a.get("maker") {
            opt(&mut m, "maker", str_of(mk, "name_english"));
            opt(&mut m, "maker_race", str_of(mk, "race"));
        }
        if let Some(p) = a.get("pos") {
            let (x, y, z) = pos_of(Some(p));
            // slug_of names only the playable levels.
            match (slug_of(z), crop.local(x, y)) {
                (Some(l), Some((cx, cy))) => {
                    m.insert("level".into(), json!(l));
                    m.insert("x".into(), json!(cx));
                    m.insert("y".into(), json!(cy));
                }
                _ => {
                    m.insert("df_pos".into(), json!({"x": x, "y": y, "z": z}));
                }
            }
        }
        opt(&mut m, "df_artifact_id", int_of(a, "id"));
        artifacts.push(Value::Object(m));
    }

    // kingdom.json
    let fort = root.get("fortress").cloned().unwrap_or(Value::Null);
    let place_values: Vec<Value> = places.iter().map(|p| Value::Object(p.json.clone())).collect();
    let kingdom = json!({
        "schema": "bomrekedan.kingdom/v1",
        "source": {
            "site": str_of(&fort, "site_name_english"),
            "site_native": str_of(&fort, "site_name"),
            "civ": str_of(&fort, "civ_name_english"),
            "group": str_of(&fort, "group_name_english"),
            "world": str_of(&fort, "world_name_english"),
            "world_native": str_of(&fort, "world_name"),
            "year": int_of(&fort, "year"),
            "exported_from": "tools/angelx-kingdom-export.lua -> expansions/bomrekedan/df-export",
        },
        "crop": {
            "df_x0": crop.x0, "df_y0": crop.y0, "w": crop.w, "h": crop.h,
            "over_game_view": crop.w > MAX_W || crop.h > MAX_H,
            "pad": PAD,
        },
        "surface_level": slug_of(surface_z),
        "dropped_levels": dropped,
        "levels": level_json,
        "place_counts": place_counts,
        "bedroom_blocks": blocks,
        "places": place_values,
        "seams": seams,
        "artifacts": artifacts,
        "wealth": root.get("wealth"),
    });

    // 7. People.
    let mut cit_by_id: BTreeMap<i64, &Value> = BTreeMap::new();
    for c in citizens {
        if let Some(id) = int_of(c, "id") {
            cit_by_id.insert(id, c);
        }
    }
    let mut people = Vec::new();
    let mut by_role: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_race: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_job: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_district: BTreeMap<String, usize> = BTreeMap::new();
    let mut folk_people: Vec<&Value> = arr(&folk, "people").iter().collect();
    folk_people.sort_by_key(|p| int_of(p, "id").unwrap_or(0));
    let excluded = ["dead", "hostile", "invader", "undead", "wildlife", "pet"];
    let mut hostile_people = Vec::new();
    for p in folk_people {
        let role = str_of(p, "role").unwrap_or_default();
        let Some(id) = int_of(p, "id") else { continue };
        if excluded.contains(&role.as_str()) {
            if role != "dead" {
                hostile_people.push(json!({
                    "id": id, "race": str_of(p, "race"), "race_name": str_of(p, "race_name"),
                    "role": role, "name": str_of(p, "name_english"),
                }));
            }
            continue;
        }
        let cit = cit_by_id.get(&id).copied();
        // Skills: the citizen record has every skill with its experience;
        // the folk record only those at level 3 and up.
        let src = cit.unwrap_or(p);
        let mut skills: Vec<(i64, i64, String)> = arr(src, "skills")
            .iter()
            .filter_map(|s| Some((int_of(s, "level")?, int_of(s, "experience").unwrap_or(0), str_of(s, "skill")?)))
            .collect();
        skills.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        let positions: Vec<String> = arr(p, "positions").iter().filter_map(|x| x.as_str().map(String::from)).collect();
        let squad = p.get("squad").filter(|s| !s.is_null());
        let profession = str_of(p, "profession").unwrap_or_default();
        let job = positions
            .first()
            .map(|c| job_from_position(c))
            .or(if squad.is_some() { Some("guard") } else { None })
            .or_else(|| {
                let lp = profession.to_lowercase();
                PROFESSION_JOBS.iter().find(|(k, _)| lp.contains(k)).map(|(_, j)| *j)
            })
            .or_else(|| skills.iter().filter(|s| s.0 > 0).find_map(|s| job_from_skill(&s.2)))
            .unwrap_or("hauler");
        let (x, y, z) = pos_of(p.get("pos"));
        let mut m = Map::new();
        m.insert("id".into(), json!(id));
        opt(&mut m, "name", str_of(p, "name"));
        opt(&mut m, "name_english", str_of(p, "name_english"));
        opt(&mut m, "race", str_of(p, "race"));
        opt(&mut m, "race_name", str_of(p, "race_name"));
        opt(&mut m, "caste", str_of(p, "caste").map(|c| c.to_lowercase()));
        opt(&mut m, "age", p.get("age").and_then(|a| a.as_f64()).map(|a| a.floor() as i64));
        m.insert("role".into(), json!(role));
        m.insert("profession".into(), json!(profession));
        m.insert("positions".into(), json!(positions));
        if let Some(s) = squad {
            m.insert("squad".into(), json!(str_of(s, "name").filter(|n| !n.is_empty()).or_else(|| int_of(s, "id").map(|i| format!("squad {i}")))));
        }
        opt(&mut m, "civ", str_of(p, "civ"));
        m.insert(
            "skills".into(),
            json!(skills.iter().take(5).map(|s| json!({"skill": s.2.to_lowercase(), "level": s.0})).collect::<Vec<_>>()),
        );
        // Where they stand, when it is on a playable level (slug_of names
        // only those) inside the crop.
        let stand = slug_of(z).zip(crop.local(x, y));
        match &stand {
            Some((l, (cx, cy))) => {
                m.insert("level".into(), json!(l));
                m.insert("x".into(), json!(cx));
                m.insert("y".into(), json!(cy));
            }
            None => {
                m.insert("level".into(), Value::Null);
                m.insert("df_pos".into(), json!({"x": x, "y": y, "z": z}));
            }
        }
        let stand = stand.map(|(l, _)| l);
        m.insert("home_job".into(), json!(job));
        let district = district_of(job).or(stand.clone()).or_else(|| slug_of(surface_z));
        m.insert("district".into(), json!(district));
        let mut notes = trait_words(cit.and_then(|c| c.get("traits")));
        notes.extend(value_words(cit.map(|c| arr(c, "values")).unwrap_or(&[])));
        m.insert("notable".into(), json!(notes));
        *by_role.entry(role.clone()).or_default() += 1;
        *by_race.entry(str_of(p, "race_name").unwrap_or_default()).or_default() += 1;
        *by_job.entry(job.to_string()).or_default() += 1;
        *by_district.entry(district.clone().unwrap_or_default()).or_default() += 1;
        people.push(Value::Object(m));
    }
    lines.push(format!("people: {} living ({:?})", people.len(), by_role));
    let people_json = json!({
        "schema": "bomrekedan.people/v1",
        "count": people.len(),
        "by_role": by_role,
        "by_race": by_race,
        "by_home_job": by_job,
        "by_district": by_district,
        "people": people,
    });

    // 8. Races.
    let wr = root.get("world_races").cloned().unwrap_or(Value::Null);
    let groups = arr(&folk, "groups");
    let mut fort_races: BTreeMap<String, (String, BTreeMap<String, i64>)> = BTreeMap::new();
    for g in groups.iter().filter(|g| g.get("intelligent").and_then(|x| x.as_bool()).unwrap_or(false)) {
        let race = str_of(g, "race").unwrap_or_default();
        let e = fort_races.entry(race).or_insert_with(|| (str_of(g, "race_name").unwrap_or_default(), BTreeMap::new()));
        *e.1.entry(str_of(g, "role").unwrap_or_default()).or_default() += int_of(g, "count").unwrap_or(0);
    }
    let mut fort_peoples: Vec<Value> = fort_races
        .iter()
        .map(|(race, (name, roles))| {
            let living: i64 = roles.iter().filter(|(r, _)| !matches!(r.as_str(), "dead" | "hostile" | "invader")).map(|(_, c)| c).sum();
            json!({"race": race, "race_name": name, "living": living, "roles": roles})
        })
        .collect();
    fort_peoples.sort_by(|a, b| {
        int_of(b, "living").cmp(&int_of(a, "living")).then(str_of(a, "race").cmp(&str_of(b, "race")))
    });
    let mut civs_by_race: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for c in arr(&wr, "civilizations") {
        civs_by_race.entry(str_of(c, "race").unwrap_or_default()).or_default().push(c);
    }
    let mut civ_groups: Vec<Value> = civs_by_race
        .iter()
        .map(|(race, cs)| {
            let mut named: Vec<&&Value> = cs.iter().filter(|c| str_of(c, "name_english").is_some_and(|n| !n.is_empty())).collect();
            named.sort_by(|a, b| int_of(b, "sites").cmp(&int_of(a, "sites")).then(int_of(a, "id").cmp(&int_of(b, "id"))));
            json!({
                "race": race,
                "race_name": cs.first().and_then(|c| str_of(c, "race_name")),
                "civilizations": cs.len(),
                "sites": cs.iter().filter_map(|c| int_of(c, "sites")).sum::<i64>(),
                "figures": cs.iter().filter_map(|c| int_of(c, "figures")).sum::<i64>(),
                "unnamed": cs.len() - named.len(),
                "named": named.iter().map(|c| json!({
                    "id": int_of(c, "id"), "name": str_of(c, "name_english"), "name_native": str_of(c, "name"),
                    "sites": int_of(c, "sites"), "figures": int_of(c, "figures"),
                    "ours": c.get("ours").and_then(|o| o.as_bool()).unwrap_or(false),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    civ_groups.sort_by(|a, b| {
        int_of(b, "civilizations").cmp(&int_of(a, "civilizations")).then(str_of(a, "race").cmp(&str_of(b, "race")))
    });
    let races_all = arr(&wr, "intelligent_races");
    let mods: Vec<String> = arr(&wr, "mods").iter().filter_map(|m| m.as_str().map(String::from)).collect();
    let mut notable_ids: BTreeSet<String> = NOTABLE.iter().map(|s| s.to_string()).collect();
    for (m, r) in MODDED {
        if mods.iter().any(|x| x == m) {
            notable_ids.insert(r.to_string());
        }
    }
    notable_ids.extend(fort_races.keys().cloned());
    let dead_list = arr(root, "dead");
    let mut notable = Vec::new();
    for rid in &notable_ids {
        let rec = races_all.iter().find(|r| str_of(r, "creature_id").as_deref() == Some(rid.as_str()));
        let roles = fort_races.get(rid).map(|r| r.1.clone()).unwrap_or_default();
        let civ = civs_by_race.get(rid);
        if rec.is_none() && roles.is_empty() && civ.is_none() {
            continue;
        }
        let modded = MODDED.iter().find(|(_, r)| r == rid).map(|(m, _)| *m).filter(|m| mods.iter().any(|x| x == m));
        let made = arr(root, "artifacts")
            .iter()
            .filter(|a| a.get("maker").and_then(|mk| str_of(mk, "race")).as_deref() == Some(rid.as_str()))
            .count();
        let mut notes = Vec::new();
        let living: Vec<String> = roles
            .iter()
            .filter(|(r, _)| !matches!(r.as_str(), "dead" | "hostile" | "invader"))
            .map(|(r, c)| format!("{c} {r}"))
            .collect();
        if !living.is_empty() {
            notes.push(format!("living in the fort: {}", living.join(", ")));
        }
        let dead = *roles.get("dead").unwrap_or(&0);
        let pname = rec.and_then(|r| str_of(r, "plural")).unwrap_or_else(|| rid.to_lowercase());
        let sname = rec.and_then(|r| str_of(r, "name")).unwrap_or_else(|| rid.to_lowercase());
        match dead {
            0 => {}
            1 => notes.push(format!("one {sname} lies dead on the map")),
            n if n >= 20 => notes.push(format!("{n} {pname} lie dead on the map: a defeated host")),
            n => notes.push(format!("{n} {pname} lie dead on the map")),
        }
        if let Some(h) = roles.get("hostile") {
            notes.push(format!("{h} hostile on the map at export"));
        }
        if made > 0 {
            notes.push(format!("made {made} of the artifacts kept in the fort"));
        }
        if rid == "DWARF" {
            notes.push(format!("{} of the fort's own dead are recorded by name", dead_list.len()));
        }
        let mut m = Map::new();
        m.insert("race".into(), json!(rid));
        opt(&mut m, "name", rec.and_then(|r| str_of(r, "name")));
        opt(&mut m, "plural", rec.and_then(|r| str_of(r, "plural")));
        opt(&mut m, "description", rec.and_then(|r| str_of(r, "description")));
        opt(&mut m, "castes", rec.map(|r| arr(r, "castes").to_vec()));
        opt(&mut m, "mod", modded);
        m.insert("civilizations".into(), json!(civ.map_or(0, |c| c.len())));
        m.insert("sites".into(), json!(civ.map_or(0, |c| c.iter().filter_map(|x| int_of(x, "sites")).sum::<i64>())));
        m.insert("in_fort".into(), json!(roles));
        m.insert("history".into(), json!(notes));
        notable.push(Value::Object(m));
    }
    let mut all_names: Vec<String> = races_all.iter().filter_map(|r| str_of(r, "name")).filter(|n| !n.is_empty()).collect();
    all_names.sort();
    all_names.dedup();
    let races_json = json!({
        "schema": "bomrekedan.races/v1",
        "fort_peoples": fort_peoples,
        "not_in_people_json": hostile_people,
        "world_civilizations": civ_groups,
        "notable_races": notable,
        "additional_races_mods": mods.iter().filter(|m| m.starts_with("Additional Races")).collect::<Vec<_>>(),
        "intelligent_race_count": races_all.len(),
        "intelligent_race_names": all_names,
    });

    // 9. Resources.
    let ore_names: BTreeSet<String> = ex
        .prospect
        .get("Ores")
        .map(|v| v.iter().map(|(t, _)| t.to_lowercase().replace('_', " ")).collect())
        .unwrap_or_default();
    let ore_names: BTreeSet<String> = ore_names
        .into_iter()
        .chain(legend.values().filter(|(tok, _)| matches!(cls.classify(tok), b'i' | b'c' | b't' | b'g' | b'v' | b'P' | b'n')).map(|(_, n)| n.to_lowercase()))
        .collect();
    let mut spoils: BTreeMap<&str, (i64, i64, usize)> = BTreeMap::new();
    let mut unmapped = (0i64, 0i64, 0usize);
    let mut stock_lines: Vec<(i64, String, String, i64, Option<&str>)> = Vec::new();
    for s in arr(root, "stocks") {
        let ty = str_of(s, "type").unwrap_or_default();
        let mat = str_of(s, "material").unwrap_or_default();
        let (count, value) = (int_of(s, "count").unwrap_or(0), int_of(s, "value").unwrap_or(0));
        let sp = spoil_of(&ty, &mat, &ore_names);
        let slot = match sp {
            Some(w) => spoils.entry(w).or_default(),
            None => &mut unmapped,
        };
        slot.0 += count;
        slot.1 += value;
        slot.2 += 1;
        stock_lines.push((value, ty, mat, count, sp));
    }
    stock_lines.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let prospect_json: BTreeMap<String, BTreeMap<String, u64>> = ex
        .prospect
        .iter()
        .filter(|(k, _)| matches!(k.as_str(), "Ores" | "Gems" | "Layer materials" | "Other vein stone" | "Liquids" | "Base materials"))
        .map(|(k, v)| (slug(k).replace('-', "_"), v.iter().cloned().collect()))
        .collect();
    let mut top_overall: Vec<(&String, &u64)> = overall_minerals.iter().collect();
    top_overall.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let resources = json!({
        "schema": "bomrekedan.resources/v1",
        "rock_classes": rocks::CLASSES.iter().map(|(c, w)| ((*c as char).to_string(), *w)).collect::<BTreeMap<_, _>>(),
        "mineable_by_level": res_levels,
        "mineable_overall": overall_classes,
        "minerals_overall": top_overall.iter().take(40).map(|(k, v)| json!({"name": k, "tiles": v})).collect::<Vec<_>>(),
        "map_prospect": prospect_json,
        "spoils": spoils.iter().map(|(k, v)| (k.to_string(), json!({"count": v.0, "value": v.1, "lines": v.2}))).collect::<BTreeMap<_, _>>(),
        "unmapped_stock": {"count": unmapped.0, "value": unmapped.1, "lines": unmapped.2},
        "top_stock_by_value": stock_lines.iter().take(40).map(|(v, t, m, c, sp)| json!({
            "type": t.to_lowercase(), "material": m, "count": c, "value": v, "spoil": sp,
        })).collect::<Vec<_>>(),
        "wealth": root.get("wealth"),
    });

    fs::create_dir_all(kingdom_dir).map_err(|e| e.to_string())?;
    write_json(&game_dir.join("kingdom.json"), &kingdom)?;
    write_json(&game_dir.join("people.json"), &people_json)?;
    write_json(&kingdom_dir.join("races.json"), &races_json)?;
    write_json(&kingdom_dir.join("resources.json"), &resources)?;
    Ok(Summary { lines })
}
