//! What a run stands in the kingdom that no chunk's set has (docs/WORLD-3D.md,
//! "The kingdom"). The chunks are made once from the map with no wishes and
//! painted so; what the party builds on its lots and what the run's seed
//! scatters vary run to run. Each stands up as extra blocks in the chunk it
//! stands in, drawn as a room's extra blocks are (`fit::Variant`): each of
//! its kind's patch of that chunk's plate (`swatches.png`), the nearest
//! kind's when the chunk has none of it (of the kinds that stand, and only
//! one near enough to pass for it: a wall in a path's patch would vanish
//! into the path, a doorway in rock's into the rock; `Chunk::standing_swatch`),
//! else its capture colour lit, as when the chunk has no patches: two
//! shades down its bank, as a chunk with no plate draws its own, so it sits
//! among the painted ones. A fire or a window the chunk has no patch of
//! shines in its own colour, as the capture draws it. The plate is never
//! cast on them.
//!
//! - **The party's lots** (`world::wishes::lots`): each built wish stands as
//!   the kit piece nearest what was wished for ([`kit_for`]) on its lot's
//!   building rows, its door row open before it, sized for its chunk's
//!   camera and what it leaves bare dressed as a yard, as the chunks'
//!   generator stands a place (`chunks::stand`). The flat kingdom clears a
//!   lot of its trees and rocks, so what the chunk's own set stands on a lot
//!   is cut from it ([`Share::cut`]: a box down to what is left of it off
//!   the lot, anything else whole), and the chunk is drawn from its set so
//!   cut, with the same plate. Its name is carved on the ground of its door
//!   row ([`Lots::plaques`]).
//! - **Places a wish moved**: a countryside place stands on the free lot
//!   nearest its spot, so a wish that took its ground moves it
//!   (`world::kingdom`). Its chunk's building where it stood without wishes
//!   is cut, and its piece stands where it stands now.
//! - **The run's finds** (`world::finds`): each a small piece of the kit on
//!   its tile (a shrine, standing stones, a cave mouth, a campfire's logs, a
//!   lost pack, a hermit's lean-to, wall stubs, a well), as its flat art
//!   stands it; a ruin's cache while it is there, a shrine's candle once a
//!   knight has knelt. The wanderer and the hermit stay sprites.
//! - **The King's herald's answer posts** (`rules::crown`) while he is out.
//!
//! Made from the run (its wishes, its finds and which are found, whether the
//! herald is out), so a page across a wire makes the same of the run it is
//! sent; kept for the last few runs' worth, so the party building or
//! clearing something shows the next frame.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use angel_stage::scene::{Material, Node};
use angel_stage::set::{Kind, PLATE_MARGIN, Set, Shape};

use super::blockout::{self, Solid, TILE};
use super::chunks::{self, Build, Land};
use super::fit;
use super::kingdom::{self, Chunk};
use super::kit::{Face, Kit};
use crate::realm::Wish;
use crate::render::overworld::Img;
use crate::rules::Run;
use crate::world::finds::{self, Find};

/// The first id of a chunk's extra blocks' nodes, clear of its set's own.
const EXTRA_IDS: u64 = 0x7e00_0000;

/// How many runs' lots are kept made: a host's run and a seat's view of it.
const KEPT: usize = 4;

/// What a run stands in the kingdom that the chunks' sets have not.
pub struct Lots {
    /// Its own number: the same blocks and cuts, the same key.
    pub key: u64,
    /// Each chunk's share, in the order of the chunks' numbers.
    pub shares: Vec<Share>,
    /// The party's wishes' names, carved on the ground of their door rows.
    pub plaques: Vec<Plaque>,
}

/// A wish's name on the ground before it.
#[derive(Clone)]
pub struct Plaque {
    /// The chunk it lies in.
    pub index: u32,
    /// Its top-left, the kingdom's pixels.
    pub at: (i32, i32),
    pub img: Img,
}

/// One chunk's share of what a run stands.
pub struct Share {
    pub index: u32,
    /// Where its own set's blocks are cut away, `[x0, z0, x1, z1]` in the
    /// kingdom's pixels: the party's lots, and where a place a wish moved stood.
    pub clear: Vec<[f32; 4]>,
    /// What stands in it that its set has not, in the kingdom's pixels.
    pub extras: Vec<Shape>,
    /// The tiles of its lots a building stands on, which a terminal paints
    /// with what stands tallest there (a find is drawn there as it always is).
    pub tiles: Vec<(i32, i32)>,
    /// The box its extras fill, low and high corners.
    pub bounds: ([f32; 3], [f32; 3]),
    /// The chunk cut, by the uncut chunk's key (`None`: nothing of it to cut).
    cut: Mutex<Option<(u64, Option<Arc<Chunk>>)>>,
    /// Its extras as nodes, by the key of the chunk whose patches they wear.
    nodes: Mutex<Option<(u64, Arc<Vec<Node>>)>>,
    /// The lamps its extras light (a campfire, a shrine's candle), the
    /// kingdom's pixels: light = activity.
    pub lamps: Vec<angel_stage::set::SetLamp>,
    /// Its extras as a set of their own (`set lots`), for a page across a
    /// wire to fill lit as it fills a chunk (the accents of a lit window or
    /// a fire kept), and that set's number.
    pub text: Arc<str>,
    pub key: u64,
    lines: Vec<String>,
    /// Its extras lit and toned (the realm), by the key of the chunk.
    lit: Mutex<Option<(u64, Arc<Vec<Node>>)>>,
}

impl Lots {
    /// Chunk `index`'s share, if anything stands in it or is cut from it.
    pub fn share(&self, index: u32) -> Option<&Share> {
        self.shares.binary_search_by_key(&index, |s| s.index).ok().map(|i| &self.shares[i])
    }

    /// Chunk `chunk` as this run draws it: cut where a lot clears it, else
    /// as it is.
    pub fn chunk(&self, chunk: &Arc<Chunk>) -> Arc<Chunk> {
        match self.share(chunk.index) {
            Some(share) => share.cut(chunk),
            None => Arc::clone(chunk),
        }
    }

    /// Whether nothing stands and nothing is cut.
    pub fn is_empty(&self) -> bool {
        self.shares.is_empty() && self.plaques.is_empty()
    }

    /// Lay the party's names on `img` (whose top-left is the kingdom's
    /// pixel `origin`), those in the chunks `laid` says.
    pub fn lay_plaques(&self, img: &mut Img, origin: (i32, i32), laid: &dyn Fn(u32) -> bool) {
        for p in self.plaques.iter().filter(|p| laid(p.index)) {
            img.stamp(&p.img, p.at.0 - origin.0, p.at.1 - origin.1);
        }
    }
}

impl Share {
    /// Chunk `chunk` (this share's) with what its set stands where this share
    /// clears cut away: its set's lines there commented out, a box's pieces
    /// off the cleared ground after it, as a room laid otherwise is cut from
    /// its item's (`fit::Variant`). The same chunk when nothing of it stands
    /// there. Made once a chunk.
    pub fn cut(&self, chunk: &Arc<Chunk>) -> Arc<Chunk> {
        let mut kept = self.cut.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((key, cut)) = kept.as_ref()
            && *key == chunk.key
        {
            return cut.clone().unwrap_or_else(|| Arc::clone(chunk));
        }
        let cut = self.cut_text(chunk).and_then(|text| {
            chunk.cut(text.into()).map_err(|why| tracing::warn!("world3d: a lot's cut of {} does not read: {why}", chunk.id())).ok().map(Arc::new)
        });
        *kept = Some((chunk.key, cut.clone()));
        cut.unwrap_or_else(|| Arc::clone(chunk))
    }

    fn cut_text(&self, chunk: &Chunk) -> Option<String> {
        if self.clear.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = chunk.text.lines().map(str::to_string).collect();
        let mut any = false;
        for shape in &chunk.set.shapes {
            if shape.on_floor() {
                continue;
            }
            let foot = fit::footprint(shape);
            let boxed = fit::is_box(shape);
            if !self.clear.iter().any(|&r| if boxed { fit::overlaps(foot, r) } else { stands_on(foot, r) }) {
                continue;
            }
            let line = lines.get_mut(shape.line.saturating_sub(1))?;
            let mut out = format!("# cut for a lot of the party's: {}", line.trim());
            if boxed {
                let mut left = vec![foot];
                for &r in &self.clear {
                    left = left.into_iter().flat_map(|piece| fit::subtract(piece, r)).collect();
                }
                for (k, piece) in left.iter().enumerate() {
                    let id = if k == 0 { shape.id.as_deref() } else { None };
                    out.push('\n');
                    out.push_str(&fit::box_line(shape.kind, *piece, shape.at[1], shape.size[1], id));
                }
            }
            *line = out;
            any = true;
        }
        any.then(|| lines.join("\n") + "\n")
    }

    /// Its extras as nodes in the kingdom's world (unplaced), wearing chunk
    /// `chunk`'s patches as a room's extra blocks wear its item's
    /// (`fit::Variant::extra_nodes`); `swatch` names the picture each kind's
    /// patch is. Ids clear of the chunk's own. Made once a chunk.
    pub fn nodes(&self, chunk: &Chunk, swatch: &dyn Fn(Kind) -> u32) -> Arc<Vec<Node>> {
        let mut kept = self.nodes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((key, nodes)) = kept.as_ref()
            && *key == chunk.key
        {
            return Arc::clone(nodes);
        }
        let (w, h) = chunk.set.camera.size();
        let (pw, ph) = chunk.set.plate;
        let density = (1.0 - 2.0 * PLATE_MARGIN) * (pw as f32 / w.max(1) as f32).min(ph as f32 / h.max(1) as f32);
        let mut nodes = Vec::new();
        for shape in &self.extras {
            let patch = wears(chunk, shape.kind).map(|s| (swatch(s.kind), s.width as f32 / density, s.height as f32 / density));
            match patch {
                Some((picture, cw, ch)) if fit::is_box(shape) => fit::box_cells(shape, picture, (cw, ch), &mut nodes),
                Some((picture, ..)) => {
                    let material = Material { lit: true, emissive: 0.7, ..Material::picture(picture) };
                    let mut node = Node::new(0, shape.mesh, material);
                    node.transform = shape.transform();
                    nodes.push(node);
                }
                None => {
                    let [r, g, b] = plain(shape.kind);
                    let tint = [f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0];
                    let mut node = Node::new(0, shape.mesh, Material::lit(tint, if shape.kind.shines() { 1.0 } else { 0.0 }));
                    node.transform = shape.transform();
                    nodes.push(node);
                }
            }
        }
        let ids = chunk.ids();
        for (k, node) in nodes.iter_mut().enumerate() {
            node.id = ids | (EXTRA_IDS + k as u64);
        }
        let nodes = Arc::new(nodes);
        *kept = Some((chunk.key, Arc::clone(&nodes)));
        nodes
    }

    /// Its extras as the realm draws them (docs/REALM-MONOLITH.md): each its
    /// kind's tone, lit; a fire or a window an accent in its own colour; in
    /// the kingdom's world (unplaced), ids clear of chunk `chunk`'s own.
    /// Made once a chunk.
    pub fn lit(&self, chunk: &Chunk) -> Arc<Vec<Node>> {
        let mut kept = self.lit.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((key, nodes)) = kept.as_ref()
            && *key == chunk.key
        {
            return Arc::clone(nodes);
        }
        let ids = chunk.ids();
        let nodes: Vec<Node> = self
            .extras
            .iter()
            .enumerate()
            .map(|(k, shape)| {
                let kind = shape.kind;
                let material = if kind.shines() {
                    let [r, g, b] = kind.capture();
                    Material { accent: true, ..Material::color([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0]) }
                } else {
                    let t = kind.tone();
                    Material { role: kind.role(), ..Material::lit([t, t, t, 1.0], 0.0) }
                };
                let mut node = Node::new(ids | (EXTRA_IDS + k as u64), shape.mesh, material);
                node.transform = shape.transform();
                node
            })
            .collect();
        let nodes = Arc::new(nodes);
        *kept = Some((chunk.key, Arc::clone(&nodes)));
        nodes
    }

    /// The extra standing tallest over tile `(c, r)`, if one stands on it.
    pub fn over_tile(&self, (c, r): (i32, i32)) -> Option<&Shape> {
        let tile = blockout::px((c, r, 1, 1));
        self.extras
            .iter()
            .filter(|s| !s.on_floor() && fit::overlaps(fit::footprint(s), tile))
            .max_by(|a, b| (a.at[1] + a.size[1]).total_cmp(&(b.at[1] + b.size[1])))
    }

    /// Paint its lots' tiles on `img` (whose top-left is the kingdom's pixel
    /// `origin`), for a terminal with chunk `chunk`'s plate laid down: each
    /// with the patch of what stands tallest there (`chunk`'s, the nearest
    /// kind's when it has none of it), tiled mirrored at the plate's
    /// density, else that kind's capture colour, as a room laid otherwise is
    /// painted (`room::RoomWorld::picture`).
    pub fn paint(&self, img: &mut Img, origin: (i32, i32), chunk: &Chunk) {
        let (w, h) = chunk.set.camera.size();
        let (pw, ph) = chunk.set.plate;
        let density = (1.0 - 2.0 * PLATE_MARGIN) * (pw as f32 / w.max(1) as f32).min(ph as f32 / h.max(1) as f32);
        let mirrored = |v: i32, n: u32| {
            let n = n.max(1) as i32;
            let t = v.rem_euclid(2 * n);
            if t < n { t } else { 2 * n - 1 - t }
        };
        for &(c, r) in &self.tiles {
            let Some(shape) = self.over_tile((c, r)) else { continue };
            let swatch = wears(chunk, shape.kind);
            let t = TILE as i32;
            for y in r * t..(r + 1) * t {
                for x in c * t..(c + 1) * t {
                    let rgb = match swatch {
                        Some(s) => {
                            let px = mirrored((x as f32 * density).floor() as i32, s.width);
                            let py = mirrored((y as f32 * density).floor() as i32, s.height);
                            let i = ((py as u32 * s.width + px as u32) * 4) as usize;
                            [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]]
                        }
                        None => plain(shape.kind),
                    };
                    img.set(x - origin.0, y - origin.1, rgb);
                }
            }
        }
    }
}

/// The patch of chunk `chunk`'s a block of kind `kind` wears: its own, else
/// the nearest standing kind's near enough (`Chunk::standing_swatch`); none
/// for a light (a fire, a window) the chunk has no patch of, which shines in
/// its own colour instead.
fn wears(chunk: &Chunk, kind: Kind) -> Option<&super::assets::Swatch> {
    let own = chunk.swatches().iter().any(|s| s.kind == kind);
    if kind.shines() && !own {
        return None;
    }
    chunk.standing_swatch(kind)
}

/// The colour a block of kind `kind` wears with no patch to wear: as a
/// chunk with no plate draws its own (`kingdom::darker`, two shades down
/// its bank, so it sits among the painted ones), but a light, which shines
/// in its own colour as the capture draws it.
fn plain(kind: Kind) -> [u8; 3] {
    if kind.shines() { kind.capture() } else { kingdom::darker(kind.capture()) }
}

/// Whether a shape that is not a box, its footprint `foot`, stands on
/// cleared ground `r`: its middle on it, or well over its edge (a crown
/// spilling a hair over the lot's edge stays).
fn stands_on(foot: [f32; 4], r: [f32; 4]) -> bool {
    let middle = [(foot[0] + foot[2]) / 2.0, (foot[1] + foot[3]) / 2.0];
    let inside = middle[0] > r[0] && middle[0] < r[2] && middle[1] > r[1] && middle[1] < r[3];
    let over = fit::clip(foot, r).is_some_and(|c| c[2] - c[0] > 3.0 && c[3] - c[1] > 3.0);
    inside || over
}

// ----------------------------------------------------------- the pieces ----

/// One thing a run stands: the chunk it stands in, its blocks (the
/// kingdom's pixels), the ground it clears, and the tiles a terminal paints.
#[derive(Clone, Debug, Default)]
struct Piece {
    chunk: (i32, i32),
    /// Its blocks as set lines (the realm's, `realm_mono::Piece::line`).
    lines: Vec<String>,
    clear: Vec<[f32; 4]>,
    tiles: Vec<(i32, i32)>,
    lamps: Vec<angel_stage::set::SetLamp>,
}

impl Piece {
    /// A piece of what the realm stands (`realm_place::Stood`).
    fn stood(chunk: (i32, i32), stood: super::realm_place::Stood) -> Self {
        Piece {
            chunk,
            lines: stood.pieces.iter().map(super::realm_mono::Piece::line).collect(),
            lamps: stood.lamps.into_iter().map(|(_, l)| l).collect(),
            ..Piece::default()
        }
    }
}

/// The kit piece that stands for wish `wish` on a building footprint of
/// `foot` tiles: the first of its name's words, then of its words in its
/// maker's own, that names something the kit builds; else by its size.
pub fn kit_for(wish: &Wish, foot: (i32, i32)) -> Kit {
    const TABLE: &[(&[&str], Kit)] = &[
        (&["beacon", "tower", "spire", "watchtower", "turret", "lookout"], Kit::Tower),
        (&["shrine", "wayside"], Kit::Shrine),
        (&["chapel", "church", "temple", "abbey", "altar", "grail", "sanctuary"], Kit::Chapel),
        (&["observatory", "telescope"], Kit::Observatory),
        (&["windmill", "mill"], Kit::Windmill),
        (&["well", "fountain"], Kit::Well),
        (&["forge", "smithy", "anvil", "foundry"], Kit::Forge),
        (&["workshop", "kiln", "studio", "loom"], Kit::Workshop),
        (&["stable", "stables"], Kit::Stable),
        (&["barn", "granary", "storehouse"], Kit::Barn),
        (&["garden", "flowers", "herbs"], Kit::Garden),
        (&["stall", "market", "booth", "shop"], Kit::Stall),
        (&["tent", "pavilion"], Kit::Pavilion),
        (&["table"], Kit::RoundTable),
        (&["stone", "stones", "monolith", "menhir", "cairn", "banner", "flag", "flags", "standard", "monument", "statue", "obelisk", "memorial"], Kit::Flags),
        (&["hall", "lodge", "library", "guild", "school", "academy", "archive", "court", "manor", "keep"], Kit::Hall),
        (&["tavern", "inn", "alehouse", "house", "cottage", "home", "hut", "cabin"], Kit::Cottage),
    ];
    let named = |text: &str| {
        let words: Vec<String> = text.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_ascii_lowercase).collect();
        words.iter().find_map(|word| TABLE.iter().find(|(names, _)| names.contains(&word.as_str())).map(|&(_, kit)| kit))
    };
    named(&wish.name).or_else(|| named(&wish.words)).unwrap_or(match foot {
        (1, 1) => Kit::Well,
        (w, h) if w * h >= 6 => Kit::Hall,
        _ => Kit::Cottage,
    })
}

/// Its blocks as shapes, read as a set reads them (each line as the set
/// file's: the chunks' sets and these agree to the pixel).
fn shapes_of(lines: &[String]) -> Vec<Shape> {
    let mut text = String::from("set lots\ncamera room 16 16\n");
    for l in lines {
        text.push_str(l);
        text.push('\n');
    }
    match Set::parse(&text) {
        Ok(set) => set.shapes,
        Err(why) => {
            tracing::warn!("world3d: a lot's blocks do not read: {:?}", why.first());
            Vec::new()
        }
    }
}

/// What the party's wishes stand, and where their places a wish moved
/// stand now. Made once a set of wishes (the lots' search scans the wild).
fn built(wishes: &Arc<Vec<Wish>>) -> Arc<(Vec<Piece>, Vec<Plaque>)> {
    type Kept = Mutex<VecDeque<(Arc<Vec<Wish>>, Arc<(Vec<Piece>, Vec<Plaque>)>)>>;
    static MADE: Kept = Mutex::new(VecDeque::new());
    if let Ok(kept) = MADE.lock()
        && let Some((_, made)) = kept.iter().find(|(w, _)| Arc::ptr_eq(w, wishes) || **w == **wishes)
    {
        return Arc::clone(made);
    }
    let made = Arc::new(build_wishes(wishes));
    if let Ok(mut kept) = MADE.lock() {
        while kept.len() >= KEPT {
            kept.pop_front();
        }
        kept.push_back((Arc::clone(wishes), Arc::clone(&made)));
    }
    made
}

fn build_wishes(wishes: &[Wish]) -> (Vec<Piece>, Vec<Plaque>) {
    let lots = crate::world::wishes::lots(wishes);
    if lots.iter().all(Option::is_none) {
        return (Vec::new(), Vec::new());
    }
    let geo = crate::world::kingdom::geography(wishes);
    let land = Land::of(&geo);
    let bare = kingdom::land();
    let mut pieces = Vec::new();
    // A place a wish moved: cut where it stood, stood where it stands.
    let same = |a: &Build, b: &Build| a.kit == b.kit && a.foot == b.foot;
    let terrain = super::realm_ground::terrain();
    for b in land.builds.iter().filter(|b| !bare.builds.iter().any(|o| same(o, b))) {
        let chunk = land.owner(b);
        let mut piece = Piece::stood(chunk, super::realm_place::place(b, terrain));
        piece.clear = vec![blockout::px(b.foot)];
        piece.tiles = tiles_of(b.foot);
        pieces.push(piece);
    }
    for o in bare.builds.iter().filter(|o| !land.builds.iter().any(|b| same(o, b))) {
        pieces.push(Piece { chunk: bare.owner(o), clear: vec![blockout::px(o.foot)], ..Piece::default() });
    }
    let mut plaques = Vec::new();
    for (wish, lot) in wishes.iter().zip(lots.iter()) {
        let Some(lot) = lot else { continue };
        // The building on its rows, its door row open before it.
        let foot = (lot.tx, lot.ty, lot.tw, (lot.th - 1).max(1));
        let seed = crate::render::overworld::ink::hash(lot.tx * 7 + lot.tw, lot.ty * 5 + lot.th, 983);
        let b = Build { name: wish.name.clone(), place: None, kit: kit_for(wish, (foot.2, foot.3)), foot, face: Face::South, seed };
        let chunk = land.owner(&b);
        let whole = blockout::px((lot.tx, lot.ty, lot.tw, lot.th));
        let mut piece = Piece::stood(chunk, super::realm_place::place(&b, terrain));
        piece.clear = vec![whole];
        piece.tiles = tiles_of(foot);
        pieces.push(piece);
        let img = crate::world::wishes::label(wish);
        let at = (lot.tx * TILE as i32 + (lot.tw * TILE as i32 - img.w) / 2, (lot.ty + lot.th - 1) * TILE as i32 + (TILE as i32 - img.h) / 2);
        let (cx, cz) = (lot.tx + lot.tw / 2, lot.ty + lot.th - 1);
        let index = kingdom::index(cx.div_euclid(chunks::CHUNK.0), cz.div_euclid(chunks::CHUNK.1)).unwrap_or(0);
        plaques.push(Plaque { index, at, img });
    }
    (pieces, plaques)
}

fn tiles_of((c, r, w, h): blockout::Rect) -> Vec<(i32, i32)> {
    (r..r + h).flat_map(|y| (c..c + w).map(move |x| (x, y))).collect()
}

/// The kit piece a find stands as, its footprint's width and depth (the
/// kingdom's pixels), and how far its middle is from its tile's.
pub fn find_piece(kind: finds::Kind) -> (Kit, f32, f32, f32) {
    use finds::Kind as F;
    match kind {
        F::Shrine => (Kit::Shrine, 16.0, 12.0, 0.0),
        F::Ruin => (Kit::Stubs, 28.0, 10.0, 0.0),
        F::Campfire => (Kit::Campfire, 12.0, 10.0, 0.0),
        F::Stones => (Kit::Stones, 40.0, 12.0, 0.0),
        F::Cave => (Kit::Cave, 24.0, 14.0, 1.0),
        F::LostPack => (Kit::Pack, 16.0, 8.0, 0.0),
        F::Hermit => (Kit::LeanTo, 20.0, 12.0, -2.0),
        F::Well => (Kit::Well, 16.0, 16.0, 0.0),
    }
}

/// Find `find` (number `i` of the run's, found or not) standing on its tile,
/// as its flat art stands it there, its foot on the tile's foot, in the
/// realm's language (`realm_place::find`).
fn find_blocks(find: &Find, i: usize, found: bool) -> Piece {
    let (_, w, d, dx) = find_piece(find.kind);
    let t = TILE;
    let (x, front) = (find.at.0 as f32 * t + t / 2.0 + dx, (find.at.1 + 1) as f32 * t);
    let foot = [x - w / 2.0, front - d, x + w / 2.0, front];
    let chunk = (find.at.0.div_euclid(chunks::CHUNK.0), find.at.1.div_euclid(chunks::CHUNK.1));
    Piece::stood(chunk, super::realm_place::find(find.kind, foot, 0x5eed ^ i as u32, found))
}

/// The herald's two answer posts (`rules::crown::herald`), while he is out:
/// a timber post, its crossbar and the board hung from it.
fn herald_posts(wishes: &[Wish]) -> Vec<Piece> {
    let geo = crate::world::kingdom::geography(wishes);
    let Some((_, posts)) = crate::rules::crown::herald(&geo) else {
        return Vec::new();
    };
    posts
        .iter()
        .map(|&(tx, ty)| {
            let (x, z) = (tx as f32 * TILE + TILE / 2.0, (ty + 1) as f32 * TILE - 2.0);
            let solids = vec![
                Solid::block(Kind::Wood, [x - 2.0, z - 1.0, x + 2.0, z + 1.0], 0.0, 1.0),
                Solid::block(Kind::Wood, [x - 1.0, z - 1.0, x + 1.0, z + 1.0], 0.0, 11.0),
                Solid::block(Kind::Wood, [x - 5.0, z - 0.75, x + 5.0, z + 0.75], 9.0, 1.0),
                Solid::block(Kind::Wood, [x - 4.0, z + 1.0, x + 4.0, z + 2.0], 3.5, 4.5),
            ];
            Piece {
                chunk: (tx.div_euclid(chunks::CHUNK.0), ty.div_euclid(chunks::CHUNK.1)),
                lines: solids.iter().map(fit::line_of).collect(),
                ..Piece::default()
            }
        })
        .collect()
}

/// What the run is made from, to keep its lots by.
#[derive(Clone, PartialEq)]
struct Made {
    wishes: Arc<Vec<Wish>>,
    finds: Vec<(usize, Find)>,
    found: u32,
    herald: bool,
}

/// What run `run` stands in the kingdom that the chunks' sets have not
/// (made once for what it is made from, [`KEPT`] kept); `None` when it
/// stands nothing.
pub fn of(run: &Run) -> Option<Arc<Lots>> {
    type Kept = Mutex<VecDeque<(Made, Option<Arc<Lots>>)>>;
    static KEPT_LOTS: Kept = Mutex::new(VecDeque::new());
    let made = Made {
        wishes: Arc::clone(&run.kingdom.wishes),
        finds: run.standing_finds(),
        found: run.kingdom.found,
        herald: run.home.holdings.card.is_some(),
    };
    if let Ok(kept) = KEPT_LOTS.lock()
        && let Some((_, lots)) = kept.iter().find(|(m, _)| *m == made)
    {
        return lots.clone();
    }
    let (wished, plaques) = &*built(&made.wishes);
    let mut pieces = wished.clone();
    pieces.extend(made.finds.iter().map(|(i, f)| find_blocks(f, *i, made.found & (1 << i) != 0)));
    if made.herald {
        pieces.extend(herald_posts(&made.wishes));
    }
    let lots = assemble(pieces, plaques.clone());
    let lots = (!lots.is_empty()).then(|| Arc::new(lots));
    if let Ok(mut kept) = KEPT_LOTS.lock() {
        while kept.len() >= KEPT {
            kept.pop_front();
        }
        kept.push_back((made, lots.clone()));
    }
    lots
}

/// The pieces dealt out to their chunks: each piece's blocks to the chunk it
/// stands in, the ground it clears to every chunk whose blocks may stand on
/// it (a crown spills over its chunk's edge), its tiles to the chunk each is in.
fn assemble(pieces: Vec<Piece>, plaques: Vec<Plaque>) -> Lots {
    let mut shares: Vec<Share> = Vec::new();
    // Chunk `index`'s share's place in `shares`, a new one if it has none.
    fn share(shares: &mut Vec<Share>, index: u32) -> usize {
        match shares.binary_search_by_key(&index, |s| s.index) {
            Ok(i) => i,
            Err(i) => {
                shares.insert(i, Share {
                    index,
                    clear: Vec::new(),
                    extras: Vec::new(),
                    tiles: Vec::new(),
                    bounds: ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]),
                    cut: Mutex::new(None),
                    nodes: Mutex::new(None),
                    lamps: Vec::new(),
                    lit: Mutex::new(None),
                    text: Arc::from(""),
                    key: 0,
                    lines: Vec::new(),
                });
                i
            }
        }
    }
    let mut text = String::new();
    for piece in pieces {
        for l in &piece.lines {
            text.push_str(l);
            text.push('\n');
        }
        if let Some(index) = kingdom::index(piece.chunk.0, piece.chunk.1) {
            let at = share(&mut shares, index);
            shares[at].extras.extend(shapes_of(&piece.lines));
            shares[at].lamps.extend(piece.lamps.iter().copied());
            shares[at].lines.extend(piece.lines.iter().cloned());
        }
        for &r in &piece.clear {
            text.push_str(&format!("clear {r:?}\n"));
            let (c0, r0) = (((r[0] - TILE) / kingdom::STAGE.0).floor() as i32, ((r[1] - TILE) / kingdom::STAGE.1).floor() as i32);
            let (c1, r1) = (((r[2] + TILE) / kingdom::STAGE.0).floor() as i32, ((r[3] + TILE) / kingdom::STAGE.1).floor() as i32);
            for cz in r0..=r1 {
                for cx in c0..=c1 {
                    if let Some(index) = kingdom::index(cx, cz) {
                        let at = share(&mut shares, index);
                        shares[at].clear.push(r);
                    }
                }
            }
        }
        for &(c, r) in &piece.tiles {
            if let Some(index) = kingdom::index(c.div_euclid(chunks::CHUNK.0), r.div_euclid(chunks::CHUNK.1)) {
                let at = share(&mut shares, index);
                shares[at].tiles.push((c, r));
            }
        }
    }
    for s in &mut shares {
        let mut set = String::from("set lots\ncamera room 16 16\n");
        for l in &s.lines {
            set.push_str(l);
            set.push('\n');
        }
        let mut key = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(s.index);
        for &byte in set.as_bytes() {
            key ^= u64::from(byte);
            key = key.wrapping_mul(0x0000_0100_0000_01b3);
        }
        s.text = set.into();
        s.key = key;
        for shape in &s.extras {
            let [x0, z0, x1, z1] = fit::footprint(shape);
            let (lo, hi) = &mut s.bounds;
            *lo = [lo[0].min(x0), lo[1].min(shape.at[1]), lo[2].min(z0)];
            *hi = [hi[0].max(x1), hi[1].max(shape.at[1] + shape.size[1]), hi[2].max(z1)];
        }
    }
    let mut key = 0xcbf2_9ce4_8422_2325u64;
    for &byte in text.as_bytes() {
        key ^= u64::from(byte);
        key = key.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Lots { key, shares, plaques }
}

// ------------------------------------------------------------ proofs ----

/// A wish as a `.wish` file of the realm's catalog says it
/// (`cockpit/assets/realm/wishes/`): `name`, `words`, `near`, then `art`
/// and its rows. Built, raised `raised`.
#[cfg(any(test, feature = "testing"))]
pub fn read_wish(id: &str, text: &str, raised: u32) -> Option<Wish> {
    let mut wish = Wish {
        id: id.to_string(),
        name: String::new(),
        words: String::new(),
        by: String::new(),
        status: crate::realm::Status::Built,
        price: Default::default(),
        art: Vec::new(),
        near: String::new(),
        paid: Vec::new(),
        raised,
    };
    let mut art = false;
    for line in text.lines() {
        if art {
            if !line.trim().is_empty() {
                wish.art.push(line.trim_end().to_string());
            }
            continue;
        }
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match word {
            "name" => wish.name = rest.trim().to_string(),
            "words" => wish.words = rest.trim().to_string(),
            "near" => wish.near = rest.trim().to_string(),
            "art" => art = true,
            _ => {}
        }
    }
    (!wish.name.is_empty() && !wish.art.is_empty()).then_some(wish)
}

/// The realm's catalog wishes of `ids` (`cockpit/assets/realm/wishes/`), as
/// built, each wished beside `near`, raised in that order: the party's lots
/// for a proof.
#[cfg(any(test, feature = "testing"))]
pub fn proof_wishes(ids: &[&str], near: &str) -> Vec<Wish> {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/realm/wishes");
    ids.iter()
        .enumerate()
        .filter_map(|(k, id)| {
            let text = std::fs::read_to_string(folder.join(format!("{id}.wish"))).ok()?;
            let mut wish = read_wish(id, &text, k as u32 + 1)?;
            wish.near = near.to_string();
            Some(wish)
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/world3d/lots__tests.rs"]
mod tests;
