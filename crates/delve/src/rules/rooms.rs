//! Rooms drawn by hand, a pack at a time: the dungeon masters' layouts,
//! laid beside the packs' own templates (`layout::Pack::templates`) and in
//! place of the pillared grid (`layout::great_quadrant`) for the bigger
//! rooms a party is given (`layout::party_room`).
//!
//! The rooms themselves — every quadrant of every pack, the sizes they are
//! drawn for, the authored share and the least floor — are authored in
//! `cockpit/live/rooms-rules/` and hot-load with the rest of the live
//! modules. This file keeps the types the renderer and the layout rules read
//! (`Template`, the sizes, the share) and a thin adapter: it asks the module
//! for a pack's quadrants, checks the answer is a room at all (the right
//! size, only `.` `#` `~`, the doorway lanes open at their ends, the middle
//! open) and keeps the checked art for the life of the process. A room is
//! drawn as its top-left quadrant and mirrored both ways (`layout::mirror`):
//! `.` floor, `#` a block, `~` a hazard pool. The column next to the vertical
//! mirror and the row next to the horizontal one are the doorway lanes and
//! stay open at their ends. Every authored one is also checked whole — door
//! ways reach, floor share — by `together_rooms__tests` beside this crate's
//! layout rules.
use super::layout::{COLS, Pack, ROWS, Template};
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};

use super::layout::{self, Room, RoomKind, Tile};

/// The live module that owns the drawn rooms.
const MODULE: &str = "rooms-rules";

/// The packs `layout::Pack` numbers, in its own order.
const PACKS: usize = 7;

/// The least share of a room's interior, in percent, left as floor; the
/// module owns the number and this adapter refuses a room below it once a
/// room can be laid.
static CONSTS: OnceLock<Consts> = OnceLock::new();

static SMALL: [OnceLock<&'static [Template]>; PACKS] = [const { OnceLock::new() }; PACKS];
static GREAT: [OnceLock<&'static [&'static [&'static str]]>; PACKS] =
    [const { OnceLock::new() }; PACKS];

/// The great-room quadrants already asked for, by (pack, width, height).
static SIZED: Mutex<Vec<((usize, usize, usize), &'static [&'static [&'static str]])>> =
    Mutex::new(Vec::new());

#[derive(Deserialize)]
struct Quad {
    w: u32,
    h: u32,
    rows: Vec<String>,
}

#[derive(Deserialize)]
struct Art {
    quads: Vec<Quad>,
}

#[derive(Debug, Deserialize)]
struct Consts {
    sizes: Vec<(u32, u32)>,
    share: u32,
    least_floor: u32,
}

/// A pack's hand-drawn one-screen quadrants, eleven by six.
pub fn small(pack: Pack) -> &'static [Template] {
    SMALL[index(pack)].get_or_init(|| {
        let want = (COLS / 2 - 1, ROWS / 2 - 1);
        let art = draw(pack, "small", Some(want));
        let mut out = Vec::with_capacity(art.quads.len());
        for quad in &art.quads {
            checked(quad, Some(want)).unwrap_or_else(|error| refuse(pack, "small", error));
            out.push(template(quad));
        }
        Box::leak(out.into_boxed_slice())
    })
}

/// Every quadrant a pack has drawn for the bigger rooms, of any size.
pub fn all_great(pack: Pack) -> &'static [&'static [&'static str]] {
    GREAT[index(pack)].get_or_init(|| quadrants(draw(pack, "great", None), pack, None))
}

/// A pack's hand-drawn quadrants `w` wide and `h` high, in authored order.
pub fn great(pack: Pack, w: usize, h: usize) -> Vec<&'static [&'static str]> {
    sized(pack, (w, h)).to_vec()
}

fn sized(pack: Pack, size: (usize, usize)) -> &'static [&'static [&'static str]] {
    let key = (index(pack), size.0, size.1);
    {
        let kept = SIZED.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some((_, art)) = kept.iter().find(|(seen, _)| *seen == key) {
            return art;
        }
    }
    let art = quadrants(draw(pack, "sized", Some(size)), pack, Some(size));
    let mut kept = SIZED.lock().unwrap_or_else(|poison| poison.into_inner());
    // Another thread may have asked for the same size while this one asked the
    // module; either answer is the module's, so the first one kept stands.
    if let Some((_, art)) = kept.iter().find(|(seen, _)| *seen == key) {
        return art;
    }
    kept.push((key, art));
    art
}

/// The quadrant sizes of the bigger rooms (interior halves less the lanes):
/// a party's rooms (34x20, 42x24, 48x28) and the great halls the packs lay
/// (32x20, 36x20, 40x24).
pub fn sizes() -> Vec<(usize, usize)> {
    consts()
        .sizes
        .iter()
        .map(|&(w, h)| (w as usize, h as usize))
        .collect()
}

/// The rule every drawn quadrant keeps, in a line: what [`fault`] checks.
pub const RULE: &str = "a quadrant keeps its doorway lanes open at their doors, its middle 3x3 open, every floor tile walkable from every door, and at least 55% floor";

/// The room a quadrant makes at its size, every doorway open.
pub(crate) fn laid(quadrant: &[&str]) -> Room {
    let (w, h) = (quadrant[0].len(), quadrant.len());
    let (cols, rows) = (2 * (w + 1), 2 * (h + 1));
    let mut room = Room::kingdom(vec![Tile::Floor; cols * rows], cols, rows);
    room.kind = RoomKind::Fight;
    room.doors = [true; 4];
    let rows_of: Vec<Vec<u8>> = quadrant.iter().map(|r| r.bytes().collect()).collect();
    layout::mirror(&mut room, [true; 4], &rows_of);
    room
}

/// What is wrong with a drawn quadrant, if anything: rows of one length in
/// only the three marks, the doorway lanes open at their doors, the middle
/// open, every floor tile walkable from the doors, enough floor.
pub fn fault(quadrant: &[&str]) -> Option<String> {
    let w = quadrant.first()?.len();
    if quadrant.iter().any(|r| r.len() != w) {
        return Some("rows of different lengths".into());
    }
    if let Some(c) = quadrant.iter().flat_map(|r| r.chars()).find(|c| !matches!(c, '.' | '#' | '~')) {
        return Some(format!("a mark that is none of . # ~: {c:?}"));
    }
    // The doorway lanes' far ends: the last column at the top row and the
    // last row at the left column, where the doorways come in.
    let h = quadrant.len();
    if h < 3 || w < 3 {
        return Some("smaller than the middle it must keep open".into());
    }
    if quadrant[0].as_bytes()[w - 1] != b'.' || quadrant[h - 1].as_bytes()[0] != b'.' {
        return Some("a doorway lane is shut at its door".into());
    }
    // The middle of the room (each quadrant's corner by both lanes) stays
    // open: the stairs and a guardian stand there.
    if quadrant[h - 3..].iter().any(|r| r.as_bytes()[w - 3..].iter().any(|&b| b != b'.')) {
        return Some("the middle of the room is not open (the last 3x3 of the quadrant)".into());
    }
    let room = laid(quadrant);
    if !layout::connected(&room) {
        return Some("a doorway does not reach the centre".into());
    }
    let walk = |t: Tile| matches!(t, Tile::Floor | Tile::Door);
    let (cols, rows) = (room.cols, room.rows);
    let mut seen = vec![false; cols * rows];
    let start = (cols as i32 / 2, 0);
    let mut queue = std::collections::VecDeque::from([start]);
    seen[start.0 as usize] = true;
    while let Some((c, r)) = queue.pop_front() {
        for (dc, dr) in layout::DIRS {
            let (nc, nr) = (c + dc, r + dr);
            if walk(room.tile(nc, nr)) && !seen[nr as usize * cols + nc as usize] {
                seen[nr as usize * cols + nc as usize] = true;
                queue.push_back((nc, nr));
            }
        }
    }
    let floor: Vec<(i32, i32)> = (0..rows as i32)
        .flat_map(|r| (0..cols as i32).map(move |c| (c, r)))
        .filter(|&(c, r)| room.tile(c, r) == Tile::Floor)
        .collect();
    if let Some(&(c, r)) = floor.iter().find(|&&(c, r)| !seen[r as usize * cols + c as usize]) {
        return Some(format!("floor at {c},{r} that no one can walk to"));
    }
    let interior = (cols - 2) * (rows - 2);
    if floor.len() * 100 < interior * least_floor() {
        return Some(format!("only {}% floor", floor.len() * 100 / interior));
    }
    None
}


/// How often, in percent, a bigger room of a size someone has drawn is one
/// of theirs rather than the pillared grid.
pub fn authored_share() -> u32 {
    consts().share
}

/// The least share of a room's interior, in percent, left as floor.
pub fn least_floor() -> usize {
    consts().least_floor as usize
}

fn consts() -> &'static Consts {
    CONSTS.get_or_init(|| {
        let consts: Consts = answer(&serde_json::json!({"op": "consts"}));
        let sizes_sane = !consts.sizes.is_empty()
            && consts.sizes.iter().all(|&(w, h)| w > 2 && h > 2 && w < 64 && h < 64);
        if !sizes_sane || consts.share > 100 || consts.least_floor > 100 {
            panic!("{MODULE}: the shared sizes and shares are not room numbers: {consts:?}");
        }
        consts
    })
}

/// The pack's index in the module's arms.
fn index(pack: Pack) -> usize {
    match pack {
        Pack::Crypt => 0,
        Pack::Cavern => 1,
        Pack::Hellforge => 2,
        Pack::Archive => 3,
        Pack::Fungal => 4,
        Pack::Unknown => 5,
        Pack::Swamp => 6,
    }
}

fn draw(pack: Pack, op: &'static str, size: Option<(usize, usize)>) -> Art {
    let mut input = serde_json::json!({"op": op, "pack": index(pack) as u32});
    if let Some((w, h)) = size {
        input["w"] = (w as u32).into();
        input["h"] = (h as u32).into();
    }
    let art: Art = answer(&input);
    let sizes = consts();
    for quad in &art.quads {
        let known = size.is_some_and(|want| (quad.w as usize, quad.h as usize) == want)
            || sizes.sizes.iter().any(|&(w, h)| (quad.w, quad.h) == (w, h));
        if !known {
            panic!(
                "{MODULE}: a {}x{} quadrant is no size a room is drawn at",
                quad.w, quad.h
            );
        }
    }
    art
}

/// Every quadrant of an answer, checked and kept for the process.
fn quadrants(
    art: Art,
    pack: Pack,
    size: Option<(usize, usize)>,
) -> &'static [&'static [&'static str]] {
    let mut out = Vec::with_capacity(art.quads.len());
    for quad in &art.quads {
        checked(quad, size).unwrap_or_else(|error| refuse(pack, "great", error));
        out.push(quadrant(quad));
    }
    Box::leak(out.into_boxed_slice())
}

/// What is wrong with a drawn quadrant, if anything: the size the module says
/// it drew, then the marks and the lanes the layout rules rely on.
fn checked(quad: &Quad, size: Option<(usize, usize)>) -> Result<(), String> {
    let (w, h) = (quad.w as usize, quad.h as usize);
    if w < 4 || h < 4 {
        return Err(format!("a room {w} by {h} is too small to lay"));
    }
    if let Some(want) = size {
        if (w, h) != want {
            return Err(format!("{w}x{h} drawn for a room laid {want:?}"));
        }
    }
    if quad.rows.len() != h {
        return Err(format!("{h} rows asked for, {} answered", quad.rows.len()));
    }
    for row in &quad.rows {
        if row.len() != w {
            return Err(format!("a row of {} marks in a room {w} wide", row.len()));
        }
        if let Some(c) = row.chars().find(|c| !matches!(c, '.' | '#' | '~')) {
            return Err(format!("a mark that is none of . # ~: {c:?}"));
        }
    }
    // The doorway lanes' far ends: the last column at the top row and the last
    // row at the left column, where the doorways come in.
    if quad.rows[0].as_bytes()[w - 1] != b'.' || quad.rows[h - 1].as_bytes()[0] != b'.' {
        return Err("a doorway lane is shut at its door".into());
    }
    // The middle of the room (each quadrant's corner by both lanes) stays open:
    // the stairs and a guardian stand there.
    if quad.rows[h - 3..]
        .iter()
        .any(|row| row.as_bytes()[w - 3..].iter().any(|&mark| mark != b'.'))
    {
        return Err("the middle of the room is not open".into());
    }
    Ok(())
}

fn refuse(pack: Pack, op: &str, error: String) -> ! {
    panic!("{MODULE}: {pack:?} answered no {op} room: {error}")
}

/// One one-screen quadrant as the layout rules read it.
fn template(quad: &Quad) -> Template {
    let rows: Vec<&'static str> = quad.rows.iter().map(|row| kept(row)).collect();
    rows.try_into()
        .unwrap_or_else(|rows: Vec<&'static str>| panic!("{MODULE}: a one-screen room of {} rows", rows.len()))
}

fn quadrant(quad: &Quad) -> &'static [&'static str] {
    let rows: Vec<&'static str> = quad.rows.iter().map(|row| kept(row)).collect();
    Box::leak(rows.into_boxed_slice())
}

fn kept(row: &str) -> &'static str {
    Box::leak(row.to_owned().into_boxed_str())
}

fn answer<T: serde::de::DeserializeOwned>(input: &serde_json::Value) -> T {
    #[cfg(test)]
    test_engine::install();
    crate::scripted::step(MODULE, input)
        .unwrap_or_else(|error| panic!("{MODULE} refused {input}: {error}"))
}

/// The Delve carries no JavaScript engine, so its own tests answer from the
/// module's captured answers, written from the compiled module by
/// `cockpit/live/rooms-rules/rooms-art.json` and checked against the moved
/// oracle in the cockpit's rules tests. Only the marshalling lives here.
#[cfg(test)]
mod test_engine {
    use std::sync::OnceLock;

    const ART: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../cockpit/live/rooms-rules/rooms-art.json"
    ));

    static PARSED: OnceLock<serde_json::Value> = OnceLock::new();

    pub(super) fn install() {
        crate::scripted::install_for_test(super::MODULE, |_, input| engine(input));
    }

    fn engine(input: &serde_json::Value) -> Result<serde_json::Value, String> {
        let art = PARSED.get_or_init(|| {
            serde_json::from_str(ART).expect("the rooms module's captured answers parse")
        });
        let op = match input["op"].as_str() {
            Some(op) => op,
            None => return Err("rooms-rules: an op is asked for by name".into()),
        };
        let pack = input["pack"].as_u64().unwrap_or(0).to_string();
        let answer = match op {
            "consts" => &art["consts"],
            "small" | "great" => &art[op][&pack],
            "sized" => {
                let key = format!(
                    "{pack},{},{}",
                    input["w"].as_u64().unwrap_or(0),
                    input["h"].as_u64().unwrap_or(0)
                );
                &art["sized"][&key]
            }
            other => return Err(format!("rooms-rules: unknown op {other}")),
        };
        if answer.is_null() {
            return Err(format!("rooms-rules: no captured answer for {input}"));
        }
        Ok(answer.clone())
    }
}
