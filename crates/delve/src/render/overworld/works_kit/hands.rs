//! The works' people: a hired hand of each trade, drawn on the knights' own
//! rig (`arena::quick_rig`), so a hand walks, swings and stands exactly as
//! a knight does and the same clips drive both. The rig's great helm and
//! steel are taken off: each trade gets its own head (a straw hat, a cowl,
//! a baker's cap), its own cloth, and the tool it works with, turned about
//! the rig's grip socket frame by frame, as a weapon is.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::Load;
use crate::render::overworld::arena::quick_rig::{self, Clip, Playback, WeaponOrientation};
use crate::render::overworld::ink::Img;

/// The trades of the economy (docs ECONOMY.md, "Professions").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Trade {
    Labourer,
    Quarryman,
    BankerMason,
    Setter,
    MasterMason,
    LimeBurner,
    Collier,
    Woodsman,
    Sawyer,
    Carpenter,
    Smith,
    Bloomer,
    Roper,
    Carter,
    Ostler,
    Bargeman,
    Miller,
    Baker,
    Brewer,
    Herder,
    Tanner,
    Wheelwright,
    Clerk,
}

impl Trade {
    pub const ALL: [Trade; 23] = [
        Trade::Labourer,
        Trade::Quarryman,
        Trade::BankerMason,
        Trade::Setter,
        Trade::MasterMason,
        Trade::LimeBurner,
        Trade::Collier,
        Trade::Woodsman,
        Trade::Sawyer,
        Trade::Carpenter,
        Trade::Smith,
        Trade::Bloomer,
        Trade::Roper,
        Trade::Carter,
        Trade::Ostler,
        Trade::Bargeman,
        Trade::Miller,
        Trade::Baker,
        Trade::Brewer,
        Trade::Herder,
        Trade::Tanner,
        Trade::Wheelwright,
        Trade::Clerk,
    ];

    /// The trade a sim id names (`trade=` in the plan catalogue).
    pub fn from_id(id: &str) -> Option<Trade> {
        Some(match id {
            "labourer" => Trade::Labourer,
            "quarryman" => Trade::Quarryman,
            "banker-mason" => Trade::BankerMason,
            "setter" => Trade::Setter,
            "master-mason" => Trade::MasterMason,
            "lime-burner" => Trade::LimeBurner,
            "collier" => Trade::Collier,
            "woodsman" => Trade::Woodsman,
            "sawyer" => Trade::Sawyer,
            "carpenter" => Trade::Carpenter,
            "smith" => Trade::Smith,
            "bloomer" => Trade::Bloomer,
            "roper" => Trade::Roper,
            "carter" => Trade::Carter,
            "ostler" => Trade::Ostler,
            "bargeman" => Trade::Bargeman,
            "miller" => Trade::Miller,
            "baker" => Trade::Baker,
            "brewer" => Trade::Brewer,
            "herder" => Trade::Herder,
            "tanner" => Trade::Tanner,
            "wheelwright" => Trade::Wheelwright,
            "clerk" | "clerk-of-works" => Trade::Clerk,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Trade::Labourer => "labourer",
            Trade::Quarryman => "quarryman",
            Trade::BankerMason => "banker-mason",
            Trade::Setter => "setter",
            Trade::MasterMason => "master-mason",
            Trade::LimeBurner => "lime-burner",
            Trade::Collier => "collier",
            Trade::Woodsman => "woodsman",
            Trade::Sawyer => "sawyer",
            Trade::Carpenter => "carpenter",
            Trade::Smith => "smith",
            Trade::Bloomer => "bloomer",
            Trade::Roper => "roper",
            Trade::Carter => "carter",
            Trade::Ostler => "ostler",
            Trade::Bargeman => "bargeman",
            Trade::Miller => "miller",
            Trade::Baker => "baker",
            Trade::Brewer => "brewer",
            Trade::Herder => "herder",
            Trade::Tanner => "tanner",
            Trade::Wheelwright => "wheelwright",
            Trade::Clerk => "clerk",
        }
    }
}

/// What a hand is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    /// Standing, tool grounded.
    Idle,
    Walk,
    /// At the trade's work: the swing, the thrust, the stroke.
    Work,
    /// Walking with a load on the shoulder.
    Carry(Load),
    /// The trade's posture at `Pose` with no tool in hand: the piece that
    /// draws the work (a saw, a rope, a wheel's tread) puts its own at the
    /// hand's grip.
    Bare(Pose),
}

/// A posture of the rig.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pose {
    Stand,
    Walk,
    /// Arms up and down.
    Overhead,
    /// Arms across.
    Slash,
    /// Arms out and in.
    Thrust,
}

/// A head: its hat decides the trade at a glance.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hat {
    Straw,
    Cap,
    Hood,
    Kerchief,
    Tall,
    Round,
    Felt,
    Bare,
    Knit,
}

/// The tools, drawn pointing up with the hand at the foot.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pick,
    Hammer,
    Sledge,
    Chisel,
    Axe,
    Saw,
    Spade,
    Trowel,
    Tongs,
    Rake,
    Crook,
    Pole,
    Peel,
    Paddle,
    Fork,
    Tablet,
    Hank,
    Scraper,
    Plumb,
}

struct Dress {
    /// Tunic: light, field, shade.
    tunic: [char; 3],
    /// Hose and sleeves.
    hose: char,
    hat: Hat,
    /// The hat's cloth: light, shade.
    cap: [char; 2],
    tool: Tool,
    /// The rig's clip that drives the work, and how many rig ticks a loop.
    clip: Clip,
    /// An apron over the tunic.
    apron: Option<char>,
}

fn dress(trade: Trade) -> Dress {
    use Hat::*;
    let d = |tunic: [char; 3], hose: char, hat: Hat, cap: [char; 2], tool: Tool, clip: Clip, apron: Option<char>| Dress { tunic, hose, hat, cap, tool, clip, apron };
    match trade {
        Trade::Labourer => d(['O', 'o', 'B'], 'B', Straw, ['T', 'O'], Tool::Spade, Clip::Thrust, None),
        Trade::Quarryman => d(['J', 'G', 'j'], 'g', Hood, ['G', 'j'], Tool::Pick, Clip::Overhead, Some('B')),
        Trade::BankerMason => d(['i', 'h', 'J'], 'G', Cap, ['H', 'i'], Tool::Chisel, Clip::Slash, Some('H')),
        Trade::Setter => d(['v', 'U', 'u'], 'S', Cap, ['U', 'u'], Tool::Trowel, Clip::Slash, Some('i')),
        Trade::MasterMason => d([']', ')', '('], '(', Round, [']', ')'], Tool::Plumb, Clip::Idle, Some('V')),
        Trade::LimeBurner => d(['H', 'i', 'h'], 'J', Kerchief, ['H', 'i'], Tool::Rake, Clip::Thrust, None),
        Trade::Collier => d(['g', 'K', 'k'], 'K', Hood, ['K', 'k'], Tool::Rake, Clip::Thrust, Some('X')),
        Trade::Woodsman => d(['m', 'E', 'F'], 'F', Felt, ['l', 'E'], Tool::Axe, Clip::Overhead, None),
        Trade::Sawyer => d(['P', 'B', 'I'], 'I', Cap, ['o', 'B'], Tool::Saw, Clip::Thrust, Some('T')),
        Trade::Carpenter => d(['R', 'r', 'B'], 'B', Cap, ['O', 'r'], Tool::Hammer, Clip::Overhead, Some('O')),
        Trade::Smith => d(['t', 'O', 'o'], 'X', Cap, ['K', 'X'], Tool::Hammer, Clip::Overhead, Some('B')),
        Trade::Bloomer => d(['g', 'j', 'X'], 'X', Hood, ['X', 'K'], Tool::Tongs, Clip::Thrust, Some('b')),
        Trade::Roper => d(['A', 'M', 'L'], 'N', Kerchief, ['y', 'M'], Tool::Hank, Clip::Slash, None),
        Trade::Carter => d(['U', 'u', 'S'], 'S', Cap, ['u', 'S'], Tool::Crook, Clip::Idle, None),
        Trade::Ostler => d(['R', 'r', 'B'], 'I', Straw, ['T', 'O'], Tool::Fork, Clip::Thrust, None),
        Trade::Bargeman => d(['z', 'Q', 'q'], 'q', Knit, ['Q', 'q'], Tool::Pole, Clip::Thrust, None),
        Trade::Miller => d(['H', 'i', 'h'], 'h', Cap, ['H', 'i'], Tool::Paddle, Clip::Slash, None),
        Trade::Baker => d(['H', 'i', 'h'], 'h', Tall, ['H', 'i'], Tool::Peel, Clip::Thrust, Some('H')),
        Trade::Brewer => d(['R', 'p', 'B'], 'I', Cap, ['r', 'p'], Tool::Paddle, Clip::Slash, Some('O')),
        Trade::Herder => d(['m', 'l', 'E'], 'N', Straw, ['T', 'O'], Tool::Crook, Clip::Idle, None),
        Trade::Tanner => d(['B', 'I', 'b'], 'b', Hood, ['I', 'b'], Tool::Scraper, Clip::Slash, Some('n')),
        Trade::Wheelwright => d(['o', 'r', 'B'], 'I', Cap, ['O', 'o'], Tool::Hammer, Clip::Overhead, Some('P')),
        Trade::Clerk => d([')', '(', 'd'], 'd', Round, ['[', ')'], Tool::Tablet, Clip::Idle, None),
    }
}

/// A head, six rows, seven wide: `1` hat light, `2` hat shade, `s` skin,
/// `S` skin in shade, `e` an eye, `h` hair.
fn head(hat: Hat) -> [&'static str; 6] {
    match hat {
        Hat::Straw => ["..111..", ".11111.", "1111111", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Cap => ["..222..", ".21111.", ".21111.", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Hood => [".22222.", "2211122", "221sse2", "22sssS2", ".2sssS.", "..222.."],
        Hat::Kerchief => ["..111..", ".11112.", ".hssse.", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Tall => ["..111..", "..111..", ".11111.", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Round => ["..222..", ".21112.", ".22222.", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Felt => [".1.....", ".2222..", "2222222", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Bare => ["..hhh..", ".hhhhh.", ".hsssh.", ".sesse.", ".sssss.", "..SSS.."],
        Hat::Knit => ["..222..", ".21212.", ".22222.", ".sesse.", ".sssss.", "..SSS.."],
    }
}

/// A tool's ink rows, pointing up; the hand holds the bottom middle.
fn tool_rows(tool: Tool) -> &'static [&'static str] {
    match tool {
        Tool::Pick => &["vVVVVVv", ".v.B.v.", "...B...", "...B...", "...B...", "...B...", "...b..."],
        Tool::Hammer => &[".UUUU.", ".UVVU.", ".uuuu.", "..BB..", "..BB..", "..BB..", "..bb.."],
        Tool::Sledge => &["UUUUUU", "UVVVVU", "uuuuuu", "..BB..", "..BB..", "..BB..", "..BB..", "..bb.."],
        Tool::Chisel => &["..W..", "..V..", "..V..", "..B..", "..B..", "..b.."],
        Tool::Axe => &[".vVV.", ".vVVv", ".uUV.", "..B..", "..B..", "..B..", "..B..", "..b.."],
        Tool::Saw => &["oBo", ".V.", ".V.", ".V.", ".v.", ".V.", ".V.", ".v.", "oBo"],
        Tool::Spade => &[".VV.", "VVVV", "vVVv", ".B..", ".B..", ".B..", ".B..", ".b.."],
        Tool::Trowel => &["..V..", ".vVv.", ".vVv.", "..B..", "..b.."],
        Tool::Tongs => &["U.U", "U.U", ".U.", ".U.", ".U.", ".u."],
        Tool::Rake => &["U.U.U", "UUUUU", "..B..", "..B..", "..B..", "..B..", "..B..", "..b.."],
        Tool::Crook => &[".BB.", "B..B", "B...", "B...", "B...", "B...", "B...", "b..."],
        Tool::Pole => &["..O..", ".OOO.", "..B..", "..B..", "..B..", "..B..", "..B..", "..B..", "..b.."],
        Tool::Peel => &[".OOOO.", "OOOOOO", "OTTTTO", "..BB..", "..BB..", "..BB..", "..bb.."],
        Tool::Paddle => &["B.B.B", "BBBBB", "..B..", "..B..", "..B..", "..B..", "..b.."],
        Tool::Fork => &["V.V.V", "VVVVV", "..B..", "..B..", "..B..", "..B..", "..B..", "..b.."],
        Tool::Tablet => &["oooo", "oHHo", "oHHo", "oHHo", "oooo", ".B..", ".b.."],
        Tool::Hank => &["yMy", "MyM", "yMy", ".B.", ".b."],
        Tool::Scraper => &["V.....V", ".vVVVv.", "...B...", "...b..."],
        Tool::Plumb => &["..B..", "..B..", "..B..", "..t..", ".UUU.", "..U.."],
    }
}

/// The picture of one hand in one step of its loop: sixteen high on the
/// rig's own scale, a swing's reach above it; its feet on row
/// [`HAND_FOOT`], its middle on column [`HAND_MID`].
pub const HAND_W: i32 = 30;
pub const HAND_H: i32 = 30;
pub const HAND_MID: i32 = 15;
pub const HAND_FOOT: i32 = 24;

/// One hand, made: the picture and where the rig holds things in it.
pub struct Hand {
    pub img: Img,
    /// The grip, in the picture's own pixels.
    pub grip: (i32, i32),
}

/// A hand of `trade` at `act`, step `t` of its eight-step loop, facing
/// right (`left` mirrors it). Made once and shared.
pub fn hand(trade: Trade, act: Act, t: u32, left: bool) -> Arc<Hand> {
    type Key = (Trade, Act, u32, bool);
    static MADE: std::sync::OnceLock<Mutex<HashMap<Key, Arc<Hand>>>> = std::sync::OnceLock::new();
    let t = t % super::FRAMES;
    let still = matches!(act, Act::Idle | Act::Bare(Pose::Stand));
    let key = (trade, act, if still { 0 } else { t }, left);
    let memory = MADE.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(compose(trade, act, key.2, left));
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}

/// Put a hand on `im` with its feet on `foot` and its middle on column `x`.
/// Returns where its grip fell on `im`.
pub fn stamp(im: &mut Img, x: i32, foot: i32, trade: Trade, act: Act, t: u32, left: bool) -> (i32, i32) {
    let h = hand(trade, act, t, left);
    let (ox, oy) = (x - HAND_MID, foot - HAND_FOOT);
    im.stamp(&h.img, ox, oy);
    (h.grip.0 + ox, h.grip.1 + oy)
}

fn compose(trade: Trade, act: Act, t: u32, left: bool) -> Hand {
    let dress = dress(trade);
    let Some(rig) = quick_rig::seat_knight(1) else {
        return Hand { img: Img::new(HAND_W, HAND_H), grip: (HAND_MID, HAND_FOOT - 8) };
    };
    let clip = match act {
        Act::Idle | Act::Bare(Pose::Stand) => Clip::Idle,
        Act::Walk | Act::Carry(_) | Act::Bare(Pose::Walk) => Clip::Walk,
        Act::Bare(Pose::Overhead) => Clip::Overhead,
        Act::Bare(Pose::Slash) => Clip::Slash,
        Act::Bare(Pose::Thrust) => Clip::Thrust,
        Act::Work => dress.clip,
    };
    // The loop of eight steps spread over the clip's own ticks.
    let elapsed = u64::from(t) * rig.duration(clip) / u64::from(super::FRAMES);
    let sample = rig.sample(clip, elapsed, Playback::Loop, false);
    let anchors = sample.anchors();
    let body = sample.image();
    let (px, py) = (rig.pivot()[0], rig.pivot()[1]);
    // The hand's own canvas: the rig's pivot on (MID, FOOT).
    let (ox, oy) = (HAND_MID - px, HAND_FOOT - py);
    let mut im = Img::new(HAND_W, HAND_H);
    for y in 0..body.h {
        for x in 0..body.w {
            let Some(c) = body.get(x, y) else { continue };
            let ink = reclothe(&dress, c);
            if let Some(ink) = ink {
                im.set(x + ox, y + oy, ink);
            }
        }
    }
    // The helm off, the trade's head on.
    let [hx, hy] = anchors.head;
    let (cx, top) = (hx + ox, hy - 3 + oy);
    for y in top..top + 6 {
        for x in cx - 3..=cx + 3 {
            im.clear(x, y);
        }
    }
    let rows = head(dress.hat);
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let ink = match ch {
                '1' => dress.cap[0],
                '2' => dress.cap[1],
                's' => 't',
                'S' => 'o',
                'e' => 'k',
                'h' => 'B',
                _ => continue,
            };
            im.put(cx - 3 + c as i32, top + r as i32, ink);
        }
    }
    // The apron over the tunic's front.
    if let Some(apron) = dress.apron {
        let [bx, by] = anchors.back;
        for y in by - 1..by + 3 {
            for x in bx - 2..=bx + 1 {
                if im.get(x + ox, y + oy).is_some() {
                    im.put(x + ox, y + oy, apron);
                }
            }
        }
    }
    // What it holds.
    match act {
        Act::Carry(load) => {
            let [bx, by] = anchors.back;
            let mut pile = Img::new(9, 8);
            super::loads::heap(&mut pile, 4, 8, 8, 6, load, 3);
            im.stamp(&pile, bx + ox - 3, by + oy - 7);
        }
        Act::Idle => {
            // The tool grounded in the near hand, held upright.
            let [gx, gy] = anchors.grip;
            let rows = tool_rows(dress.tool);
            let tool = Img::from_rows(rows);
            let contact = ((tool.w - 1) / 2, tool.h - 1);
            im.stamp(&tool, gx + ox - contact.0, gy + oy - contact.1 + 4);
        }
        Act::Bare(_) => {}
        Act::Walk | Act::Work => {
            let rows = tool_rows(dress.tool);
            let tool = Img::from_rows(rows);
            let contact = [(tool.w - 1) / 2, tool.h - 1];
            let [gx, gy] = anchors.grip;
            let [tx, ty] = anchors.tip;
            // A walking hand carries its tool upright over the shoulder.
            let tip = if act == Act::Walk || (tx == gx && ty == gy) { [gx + 1, gy - 8] } else { [tx, ty] };
            let o = WeaponOrientation::new(contact, [gx + ox, gy + oy], [tip[0] + ox, tip[1] + oy]);
            let (turned, at) = o.stamp(&tool);
            im.stamp(&turned, at[0], at[1]);
        }
    }
    let grip = (anchors.grip[0] + ox, anchors.grip[1] + oy);
    if left {
        Hand { img: im.flip_h(), grip: (HAND_W - 1 - grip.0, grip.1) }
    } else {
        Hand { img: im, grip }
    }
}

/// A rig ink in the tunic's, hose's and skin's colours; `None` where the
/// rig's steel was a weapon's or a helm's leftover.
fn reclothe(d: &Dress, c: [u8; 3]) -> Option<[u8; 3]> {
    use crate::render::overworld::ink::{ink, palette_index};
    // Which of the rig's inks it is, by its palette slot.
    let ch = ink_char(c)?;
    let out = match ch {
        // The surcoat: red field, dark edge, yellow cross.
        '7' => d.tunic[1],
        '8' => d.tunic[2],
        '5' | '4' | '6' | '@' => d.tunic[0],
        'P' | 'I' | 'B' => 'B',
        // Steel: arms and legs become sleeves and hose, by how bright.
        's' | 'x' | 'S' => d.hose,
        'u' => d.hose,
        'U' => d.tunic[2],
        'v' | 'V' | 'W' => d.tunic[0],
        other => other,
    };
    let _ = palette_index;
    ink(out)
}

/// The ink character of a palette colour, if it is a plain one.
fn ink_char(c: [u8; 3]) -> Option<char> {
    use crate::render::overworld::ink::ink;
    static TABLE: std::sync::OnceLock<Vec<([u8; 3], char)>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            ('!'..='~').filter_map(|ch| ink(ch).map(|rgb| (rgb, ch))).collect()
        })
        .iter()
        .find(|(rgb, _)| *rgb == c)
        .map(|&(_, ch)| ch)
}
