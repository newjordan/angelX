//! What a line carries and what a yard stacks: every good the works move,
//! drawn as a little pile in its own colours, so stone, lime, timber, ore
//! and grain read differently on a cart, in a bucket, on a barge, at a
//! building's door. A pile is a few copies of one small item (a block, a
//! log end, a sack) stacked as a pyramid, or a mound for what pours.

use crate::render::overworld::ink::{Img, hash};
use crate::render::overworld::material;

/// A family of goods. The sim has a hundred goods; the eye needs about
/// thirty looks, so each good maps to the nearest ([`Load::of`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Load {
    /// Rough blocks from a quarry face.
    Stone,
    /// Small broken stone: the wall's core.
    Rubble,
    /// Dressed blocks: ashlar, quoins, voussoirs.
    Ashlar,
    /// Burnt lime, slaked lime.
    Lime,
    /// Mortar in a tub.
    Mortar,
    Sand,
    Clay,
    Brick,
    /// Logs, poles, bark.
    Timber,
    /// Planks, beams, shingles, joists.
    Planks,
    Charcoal,
    Coal,
    Ore,
    /// Bloom, wrought iron, nails, steel.
    Iron,
    Lead,
    Slate,
    Grain,
    Flour,
    Bread,
    /// Ale, water-casks, anything in a barrel.
    Ale,
    Hemp,
    Rope,
    Hides,
    Wool,
    Cloth,
    Water,
    Glass,
    /// Anything else: a crate.
    Crate,
}

impl Load {
    pub const ALL: [Load; 28] = [
        Load::Stone,
        Load::Rubble,
        Load::Ashlar,
        Load::Lime,
        Load::Mortar,
        Load::Sand,
        Load::Clay,
        Load::Brick,
        Load::Timber,
        Load::Planks,
        Load::Charcoal,
        Load::Coal,
        Load::Ore,
        Load::Iron,
        Load::Lead,
        Load::Slate,
        Load::Grain,
        Load::Flour,
        Load::Bread,
        Load::Ale,
        Load::Hemp,
        Load::Rope,
        Load::Hides,
        Load::Wool,
        Load::Cloth,
        Load::Water,
        Load::Glass,
        Load::Crate,
    ];

    /// The look of a good, by the sim's good id (`delve/kingdom/frontier.json`):
    /// the id's own words first, then the family it belongs to.
    pub fn of(good: &str) -> Load {
        let has = |w: &str| good.contains(w);
        match good {
            "stone" | "block" | "rough-block" | "field-stone" | "limestone" | "freestone" | "flint" | "basalt" | "tufa" | "voidstone" => Load::Stone,
            _ if has("rubble") => Load::Rubble,
            _ if has("ashlar") || has("quoin") || has("voussoir") || has("plinth") || has("dressed") => Load::Ashlar,
            _ if has("mortar") => Load::Mortar,
            _ if has("lime") => Load::Lime,
            _ if has("sand") => Load::Sand,
            _ if has("clay") => Load::Clay,
            _ if has("brick") => Load::Brick,
            _ if has("charcoal") => Load::Charcoal,
            _ if has("coal") => Load::Coal,
            _ if has("ore") || has("bog") => Load::Ore,
            _ if has("lead") || has("galena") => Load::Lead,
            _ if has("iron") || has("bloom") || has("steel") || has("nail") || has("cramp") || has("hinge") || has("tyre") || has("ingot") || has("portcullis") => Load::Iron,
            _ if has("slate") => Load::Slate,
            _ if has("plank") || has("beam") || has("joist") || has("shingle") || has("centring") || has("board") => Load::Planks,
            _ if has("timber") || has("log") || has("pole") || has("wood") || has("bark") => Load::Timber,
            _ if has("flour") => Load::Flour,
            _ if has("bread") => Load::Bread,
            _ if has("grain") || has("fodder") || has("hay") => Load::Grain,
            _ if has("ale") || has("wine") || has("barrel") || has("cask") => Load::Ale,
            _ if has("hemp") || has("fibre") => Load::Hemp,
            _ if has("rope") => Load::Rope,
            _ if has("hide") || has("leather") || has("harness") => Load::Hides,
            _ if has("wool") => Load::Wool,
            _ if has("cloth") || has("livery") => Load::Cloth,
            _ if has("water") => Load::Water,
            _ if has("glass") || has("potash") => Load::Glass,
            _ => Load::Crate,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Load::Stone => "stone",
            Load::Rubble => "rubble",
            Load::Ashlar => "ashlar",
            Load::Lime => "lime",
            Load::Mortar => "mortar",
            Load::Sand => "sand",
            Load::Clay => "clay",
            Load::Brick => "brick",
            Load::Timber => "timber",
            Load::Planks => "planks",
            Load::Charcoal => "charcoal",
            Load::Coal => "coal",
            Load::Ore => "ore",
            Load::Iron => "iron",
            Load::Lead => "lead",
            Load::Slate => "slate",
            Load::Grain => "grain",
            Load::Flour => "flour",
            Load::Bread => "bread",
            Load::Ale => "ale",
            Load::Hemp => "hemp",
            Load::Rope => "rope",
            Load::Hides => "hides",
            Load::Wool => "wool",
            Load::Cloth => "cloth",
            Load::Water => "water",
            Load::Glass => "glass",
            Load::Crate => "goods",
        }
    }

    /// How a heap of it is built: a stack of items, or a mound that pours.
    fn style(self) -> Style {
        match self {
            Load::Lime | Load::Sand | Load::Grain | Load::Clay | Load::Charcoal | Load::Coal | Load::Rubble | Load::Mortar | Load::Hemp | Load::Wool | Load::Glass | Load::Ore => Style::Mound,
            _ => Style::Stack,
        }
    }

    /// The ramp a mound is shaded on, dark to light.
    pub(super) fn ramp(self) -> &'static [char] {
        match self {
            Load::Lime => &['G', 'j', 'h', 'i', 'H'],
            Load::Mortar => &['g', 'G', 'J', 'o', 'h'],
            Load::Sand => &['B', 'P', 'o', 'O', 't'],
            Load::Grain => &['P', 'o', 'O', 't', 'T'],
            Load::Clay => &['b', 'B', 'r', 'R', 'o'],
            Load::Charcoal => &['k', 'K', 'X', 'g', 'G'],
            Load::Coal => &['k', 'K', 'X', 'g', 'J'],
            Load::Rubble => &['g', 'j', 'G', 'J', 'h'],
            Load::Hemp => &['N', 'L', 'A', 'M', 'y'],
            Load::Wool => &['G', 'J', 'h', 'i', 'H'],
            Load::Glass => &['S', 'u', 'U', 'v', 'V'],
            Load::Ore => &['b', 'p', 'B', 'R', 'r'],
            _ => &['g', 'j', 'G', 'J', 'h'],
        }
    }

    /// One item of a stack, ink rows, five or six wide.
    fn item(self) -> &'static [&'static str] {
        match self {
            Load::Stone => &[".hhhh.", "hJJJJG", "JJJGGj", ".GGjj."],
            Load::Ashlar => &["HHHHHH", "iiiiih", "ihhhhh", "jjjjjj"],
            Load::Brick => &["oooooo", "RRRRRr", "rrrrrB"],
            Load::Timber => &[".bBBb.", "BBtoBB", "BBotBB", ".bBBb."],
            Load::Planks => &["TTTTTTTT", "OOOOOOOo", "oooooooB"],
            Load::Iron => &["VVVVVV", "uuuuuU", "SSSSSS"],
            Load::Lead => &[".UUUU.", "uUUUUS", "SSSSSS"],
            Load::Slate => &["uUUUUU", "SSSSSS", "xxxxxx"],
            Load::Flour => &["HHHHH.", "iHHHhJ", "ihihhJ", "jhhhjj"],
            Load::Bread => &[".OOO.", "OtTtO", "oOOOo"],
            Load::Ale => &[".rrr.", "BRRrb", "UrRrU", "BRrRb", ".bbb."],
            Load::Rope => &[".OOO.", "OtTtO", "OTotO", ".ooo."],
            Load::Hides => &["OOOOOO", "ttOOtB", "BBBBBB"],
            Load::Cloth => &["))))))", "][[[[[", "((((((", "[[[[[["],
            Load::Water => &[".zzz.", "UQQQU", "UQ3QU", ".UUU."],
            _ => &["rrrrrr", "OBBBBo", "rBrrBo", "bbbbbb"],
        }
    }

    /// The colour that stands for it on a map or a sign (an ink).
    pub fn ink(self) -> char {
        self.ramp()[3]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Stack,
    Mound,
}

/// A heap of `load` standing on row `foot`, centred on `cx`, about `w`
/// wide and `h` high (a mound as wide as that; a stack as many items).
pub fn heap(im: &mut Img, cx: i32, foot: i32, w: i32, h: i32, load: Load, seed: u32) {
    match load.style() {
        Style::Mound => {
            // A half-ellipse standing on its foot: nothing below the row.
            let keep = |_: i32, y: i32| y < foot;
            let (rx, ry) = (w as f32 / 2.0, h as f32 * 1.15);
            material::lobe(im, (cx as f32, foot as f32 - 1.0), (rx, ry), load.ramp(), seed, 1.4, 0.35, &keep);
            // A few pieces on it, so a mound of ore is not a mound of dirt.
            sprinkle(im, cx, foot, w, h, load, seed);
        }
        Style::Stack => {
            let item = Img::from_rows(load.item());
            let (iw, ih) = (item.w, item.h);
            let cols = (w / iw).max(1);
            let rows = ((h + ih - 2) / (ih - 1)).clamp(1, cols);
            for r in 0..rows {
                let n = cols - r;
                let left = cx - (n * iw) / 2;
                for k in 0..n {
                    // A stack is never quite square: every other block a pixel up.
                    let jitter = i32::from(hash(k, r, seed) % 5 == 0);
                    im.stamp(&item, left + k * iw, foot - ih - r * (ih - 1) - jitter);
                }
            }
        }
    }
}

/// The little marks that tell a mound what it is.
fn sprinkle(im: &mut Img, cx: i32, foot: i32, w: i32, h: i32, load: Load, seed: u32) {
    let marks: &[(char, i32)] = match load {
        Load::Ore => &[('v', 5), ('U', 4)],
        Load::Coal => &[('h', 6)],
        Load::Charcoal => &[('j', 5)],
        Load::Lime => &[('w', 3)],
        Load::Hemp => &[('T', 4)],
        Load::Glass => &[('W', 4), ('3', 5)],
        Load::Grain => &[('T', 4)],
        _ => &[],
    };
    for &(ch, every) in marks {
        for k in 0..w {
            let v = hash(k, seed as i32, 91) as i32;
            if v % every == 0 {
                let x = cx - w / 2 + k;
                let y = foot - 1 - (v >> 3) % h.max(1);
                // Only on the mound: where there is ink already.
                if im.get(x, y).is_some() {
                    im.put(x, y, ch);
                }
            }
        }
    }
}

/// The sign-sized picture of a good: one or two of it, eight pixels square
/// at most, for a board over a building that has run out of it.
pub fn icon(load: Load) -> Img {
    let mut im = Img::new(9, 9);
    match load.style() {
        Style::Mound => heap(&mut im, 4, 9, 9, 7, load, 7),
        Style::Stack => {
            let item = Img::from_rows(load.item());
            let x = (9 - item.w).max(0) / 2;
            im.stamp(&item, x, 9 - item.h);
            if item.h <= 4 {
                let two = Img::from_rows(load.item());
                im.stamp(&two, x, 9 - item.h - (item.h - 1));
            }
        }
    }
    im
}
