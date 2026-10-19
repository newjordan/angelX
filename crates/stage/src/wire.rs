//! A stage frame on the wire to the browser: its pictures and its quads in
//! one little-endian buffer. [`encode`] writes it in the game's wasm;
//! [`decode`] reads it back where the frame is drawn (the page's renderer
//! module, `crates/web/render`; before that, `shell/stage.js`).
//!
//! ```text
//! "STG2", width, height, pictures, quads, scene        u32 × 5 after the tag
//! enter: x, y (f32, NaN when none)
//! tilt: centre x, centre y, near, zoom, squash (f32, all 0 when flat)
//! impact (f32, 0 to 1: `Frame::impact`)
//! each picture: id, width, height (u32), then width × height × 4 RGBA bytes
//! each quad: x, y, w, h, sx, sy, sw, sh, r, g, b, a, angle,
//!            stand (0 floor, 1 upright, 2 screen), foot x, foot y (f32),
//!            picture, shape (u32)
//! ```
//!
//! A stage that holds 3D props (`scene::Props`) is sent as `"STG3"`: the
//! same, with a block after the last quad that the tag promises. A reader of
//! `"STG2"` (the page's `stage.js`) refuses the other tag and draws the
//! finished frame, so the game sends it only to a page that said it can draw
//! props; `decode` reads both.
//!
//! ```text
//! props: at (a count of quads), grain, nodes   u32 × 3
//! each node (little-endian, 92 bytes):
//!   flags u32: 1 shown, 2 has a parent, 4 lit, 8 double sided, 16 linear,
//!              32 blended, 64 premultiplied (both: refused), 128 has a picture,
//!              256 an accent, bits 9 to 11 its role in a look (0 plain,
//!              1 figure, 2 water, 3 sky, 4 ember)
//!   mesh u32: 0 quad, 1 cube, 2 plane, 3 sphere, 4 gem, 5 cone, 6 uploaded,
//!             7 none (a place for others to hang from), then the primitives:
//!             8 cylinder, 9 frustum, 10 wedge, 11 gable, 12 hip, 13 pyramid,
//!             14 arch, 15 stairs, 16 rock, 17 blob, 18 fence;
//!   mesh argument u32: a sphere's or cone's segments, an uploaded mesh's id
//!   picture u32, id u64, parent u64
//!   translation × 3, rotation × 4 (a quaternion), scale × 3, tint × 4, emissive (f32)
//! ```
//!
//! A stage whose props hold a room in 3D (`scene::World`) made from a set
//! is sent as `"STG4"`: an `STG3` frame with a world block after the props'
//! last node. The world's floor and blocks go as their set's reference
//! (`scene::SetRef`): its text in the frames that carry it (a room's first,
//! or all again), its key in every frame, for the far side to make them
//! again (`set::Sets::fill`); its figures go as nodes, as props do. The
//! plate goes in the same block as the PNG it is (`SetRef::png`), in the
//! frames that carry it, for the far side to decode and keep as picture
//! `plate`: never as pixels. A world that was not made from a set goes as
//! an `STG3` frame, without it.
//!
//! ```text
//! world: at, figures_at (counts of quads), figures, key low, key high,
//!        plate, scale, offset x, y, z (f32), text bytes, png bytes,
//!        extra blocks, then the patches (below)                u32 × 28
//! the set's text (UTF-8), its length padded to a whole number of words
//! the plate's PNG, its length padded the same
//! each figure: a node, as a prop is
//! each extra block (`scene::World::extra`): a node, the same
//! ```
//!
//! The patches (`set::Patches`), what the set's surfaces the plate does not
//! see are drawn with, are 15 words: the picture they are in, then each
//! kind's width and height (`width << 8 | height`, 16 bits) two kinds a
//! word, the first kind in the low half.
//!
//! The camera is not on the wire: it is the one the frame's own floor gives
//! (`Tilt::camera`). A node with per-face materials is left out, as a
//! malformed picture is, and an uploaded mesh is named, not sent: the
//! renderer draws nothing for one it does not have.

use crate::scene::{
    AlphaMode, MAX_GRAIN, MAX_NODES, Material, MeshRef, Node, Props, Sampling, SetRef, TextureSource, Transform, World,
};
use crate::set::Patches;
use crate::{Frame, Quad, Shape, Stand, Tilt};

pub const TAG: &[u8; 4] = b"STG2";
/// The tag of a frame that carries props after its quads.
pub const TAG_PROPS: &[u8; 4] = b"STG3";
/// The tag of a frame that carries a world after its props.
pub const TAG_WORLD: &[u8; 4] = b"STG4";
/// The tag of a frame whose world is made from more than one set (the
/// kingdom's chunks, `scene::World::more`): an `STG4` frame with the other
/// sets' references after its extra blocks.
pub const TAG_WORLDS: &[u8; 4] = b"STG5";
/// The tag of a frame whose world is drawn through a look (`scene::World::
/// look`, docs/REALM-MONOLITH.md): an `STG5` frame (its further sets'
/// count always there, maybe 0) with the look after them, as text
/// (`Look::to_text`): its length (u32), then the text padded to a word.
pub const TAG_LOOK: &[u8; 4] = b"STG6";
/// The longest look text a frame may carry.
pub const MAX_LOOK_TEXT: usize = 64 << 10;
/// Bytes of the head of each further set's reference in an `STG5` frame:
/// key low, key high, plate, scale, offset x, y, z, text bytes, png bytes,
/// the patches.
pub const MORE_HEADER_BYTES: usize = 36 + PATCHES_BYTES;
/// Bytes of a set reference's patches on the wire.
pub const PATCHES_BYTES: usize = 60;
/// The most further sets a world may be made from.
pub const MAX_MORE_SETS: usize = 64;
/// Bytes of a world block's header.
pub const WORLD_HEADER_BYTES: usize = 52 + PATCHES_BYTES;
/// The longest set text a world block may carry.
pub const MAX_SET_TEXT: usize = 1 << 20;
/// The longest plate PNG a world block may carry.
pub const MAX_PLATE_PNG: usize = 32 << 20;
/// Bytes before the first picture, and bytes a quad takes.
pub const HEADER_BYTES: usize = 56;
pub const QUAD_BYTES: usize = 72;
/// Bytes of a props block's header, and bytes a node takes.
pub const PROPS_HEADER_BYTES: usize = 12;
pub const NODE_BYTES: usize = 92;

const SHOWN: u32 = 1;
const HAS_PARENT: u32 = 2;
const LIT: u32 = 4;
const DOUBLE_SIDED: u32 = 8;
const LINEAR: u32 = 16;
const BLENDED: u32 = 32;
const PREMULTIPLIED: u32 = 64;
const HAS_PICTURE: u32 = 128;
/// An accent in a look (`Material::accent`).
const ACCENT: u32 = 256;
/// What it is to a look that lights its scene (`Material::role`), three bits
/// from here: 0 plain, 1 a figure, 2 water, 3 the sky, 4 an ember.
const ROLE_SHIFT: u32 = 9;

/// The longest side a picture (or the stage itself) may take on the
/// wire: the texture limit every host guarantees, the browser's included.
pub const MAX_SIDE: u32 = 8192;

/// A picture the wire can carry truthfully: a real size, and exactly the
/// bytes its size promises. Anything else would desync the reader, which
/// walks pictures by `width × height × 4`.
fn well_formed(p: &crate::Picture) -> bool {
    (1..=MAX_SIDE).contains(&p.width)
        && (1..=MAX_SIDE).contains(&p.height)
        && (p.width as usize).checked_mul(p.height as usize).and_then(|n| n.checked_mul(4))
            == Some(p.rgba.len())
}

/// A set reference's patches as the wire's 15 words.
fn patches_words(p: &Patches) -> [u32; 15] {
    let mut words = [0; 15];
    words[0] = p.atlas;
    for (k, [w, h]) in p.sizes.iter().enumerate() {
        words[1 + k / 2] |= (u32::from(*w) << 8 | u32::from(*h)) << (16 * (k % 2));
    }
    words
}

/// The patches the wire's 15 words (`word(0)` on) say; a size past a cell
/// is a cell.
fn patches_of(word: &dyn Fn(usize) -> u32) -> Patches {
    let mut sizes = [[0; 2]; 27];
    for (k, size) in sizes.iter_mut().enumerate() {
        let half = word(1 + k / 2) >> (16 * (k % 2));
        *size = [((half >> 8) & 0xff).min(crate::set::SWATCH_CELL) as u8, (half & 0xff).min(crate::set::SWATCH_CELL) as u8];
    }
    Patches { atlas: word(0), sizes }
}

/// The wire's code for a mesh: its kind, and its argument.
fn mesh_code(mesh: Option<MeshRef>) -> (u32, u32) {
    let Some(mesh) = mesh else {
        return (7, 0);
    };
    match mesh {
        MeshRef::Quad => (0, 0),
        MeshRef::Cube => (1, 0),
        MeshRef::Plane => (2, 0),
        MeshRef::Sphere { segments } => (3, u32::from(segments)),
        MeshRef::Gem => (4, 0),
        MeshRef::Cone { segments } => (5, u32::from(segments)),
        MeshRef::Uploaded(id) => (6, id),
        MeshRef::Cylinder { segments } => (8, u32::from(segments)),
        MeshRef::Frustum { segments } => (9, u32::from(segments)),
        MeshRef::Wedge => (10, 0),
        MeshRef::Gable => (11, 0),
        MeshRef::Hip => (12, 0),
        MeshRef::Pyramid => (13, 0),
        MeshRef::Arch { segments } => (14, u32::from(segments)),
        MeshRef::Stairs { steps } => (15, u32::from(steps)),
        MeshRef::Rock { seed } => (16, u32::from(seed)),
        MeshRef::Blob { seed } => (17, u32::from(seed)),
        MeshRef::Fence { posts } => (18, u32::from(posts)),
    }
}

/// The mesh a code names: `Some(None)` for no mesh, `None` for a code that
/// names nothing.
fn mesh_of(kind: u32, arg: u32) -> Option<Option<MeshRef>> {
    let segments = arg.min(u32::from(u16::MAX)) as u16;
    Some(Some(match kind {
        0 => MeshRef::Quad,
        1 => MeshRef::Cube,
        2 => MeshRef::Plane,
        3 => MeshRef::Sphere { segments },
        4 => MeshRef::Gem,
        5 => MeshRef::Cone { segments },
        6 => MeshRef::Uploaded(arg),
        7 => return Some(None),
        8 => MeshRef::Cylinder { segments },
        9 => MeshRef::Frustum { segments },
        10 => MeshRef::Wedge,
        11 => MeshRef::Gable,
        12 => MeshRef::Hip,
        13 => MeshRef::Pyramid,
        14 => MeshRef::Arch { segments },
        15 => MeshRef::Stairs { steps: segments },
        16 => MeshRef::Rock { seed: segments },
        17 => MeshRef::Blob { seed: segments },
        18 => MeshRef::Fence { posts: segments },
        _ => return None,
    }))
}

/// A node on the wire, or nothing for one it cannot carry (per-face materials).
fn role_code(role: crate::scene::Role) -> u32 {
    use crate::scene::Role;
    match role {
        Role::Plain => 0,
        Role::Figure => 1,
        Role::Water => 2,
        Role::Sky => 3,
        Role::Ember => 4,
    }
}

fn role_of(code: u32) -> crate::scene::Role {
    use crate::scene::Role;
    match code & 7 {
        1 => Role::Figure,
        2 => Role::Water,
        3 => Role::Sky,
        4 => Role::Ember,
        _ => Role::Plain,
    }
}

fn encode_node(node: &Node, out: &mut Vec<u8>) -> bool {
    if !node.face_materials.is_empty() {
        return false;
    }
    let m = &node.material;
    let mut flags = 0;
    for (on, bit) in [
        (node.visible, SHOWN),
        (node.parent.is_some(), HAS_PARENT),
        (m.lit, LIT),
        (m.double_sided, DOUBLE_SIDED),
        (m.sampling == Sampling::Linear, LINEAR),
        (m.alpha == AlphaMode::Blend, BLENDED),
        (m.alpha == AlphaMode::Premultiplied, PREMULTIPLIED),
        (matches!(m.source, TextureSource::Picture(_)), HAS_PICTURE),
        (m.accent, ACCENT),
    ] {
        if on {
            flags |= bit;
        }
    }
    flags |= role_code(m.role) << ROLE_SHIFT;
    let (kind, arg) = mesh_code(node.mesh);
    let picture = match m.source {
        TextureSource::Picture(id) => id,
        TextureSource::None => 0,
    };
    let parent = node.parent.unwrap_or(0);
    let t = &node.transform;
    for v in [flags, kind, arg, picture, node.id as u32, (node.id >> 32) as u32, parent as u32, (parent >> 32) as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in t.translation.iter().chain(&t.rotation).chain(&t.scale).chain(&m.tint).chain([&m.emissive]) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    true
}

/// Write `frame`'s pictures and `quads` into `out` (cleared first), and its
/// props if it has some (the tag then says so). A props' `at` counts quads
/// here; a producer that has layers gets the count from [`crate::expand_at`].
/// Pictures that are not well-formed are left out (and out of the count),
/// so the buffer always reads exactly as its header says; a quad naming a
/// dropped picture simply does not draw on the far side.
pub fn encode(frame: &Frame, quads: &[Quad], out: &mut Vec<u8>) {
    out.clear();
    let world = frame.props.as_ref().and_then(|p| p.world.as_ref()).and_then(|w| Some((w, w.set.as_ref()?)));
    out.extend_from_slice(match (&frame.props, world) {
        (_, Some((w, _))) if w.look.is_some() => TAG_LOOK,
        (_, Some((w, _))) if !w.more.is_empty() => TAG_WORLDS,
        (_, Some(_)) => TAG_WORLD,
        (Some(_), None) => TAG_PROPS,
        (None, None) => TAG,
    });
    let u32s = |out: &mut Vec<u8>, values: &[u32]| {
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    let pictures: Vec<_> = frame.pictures.iter().filter(|p| well_formed(p)).collect();
    u32s(
        out,
        &[
            frame.size.0,
            frame.size.1,
            pictures.len() as u32,
            quads.len() as u32,
            frame.scene as u32,
        ],
    );
    let [ex, ey] = frame.enter.unwrap_or([f32::NAN; 2]);
    let tilt = Tilt::for_stage(frame.tilt, frame.size)
        .map_or([0.0; 5], |t| [t.centre[0], t.centre[1], t.near, t.zoom, t.squash]);
    let impact = if frame.impact.is_finite() { frame.impact.clamp(0.0, 1.0) } else { 0.0 };
    for v in [ex, ey].iter().chain(&tilt).chain(&[impact]) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for p in pictures {
        u32s(out, &[p.id, p.width, p.height]);
        out.extend_from_slice(&p.rgba);
    }
    for q in quads {
        let (stand, [fx, fy]) = match q.stand {
            Stand::Floor => (0.0, [0.0; 2]),
            Stand::Upright(foot) => (1.0, foot),
            Stand::Screen => (2.0, [0.0; 2]),
        };
        let turn = [q.angle, stand, fx, fy];
        for v in q.rect.iter().chain(&q.source).chain(&q.color).chain(&turn) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let shape = match q.shape {
            Shape::Fill => 0u32,
            Shape::Round => 1,
        };
        u32s(out, &[q.picture, shape]);
    }
    if let Some(props) = &frame.props {
        let head = out.len();
        u32s(out, &[props.at.min(quads.len()) as u32, props.grain.clamp(1, MAX_GRAIN).into(), 0]);
        let mut count = 0u32;
        for node in props.nodes.iter().take(MAX_NODES) {
            if encode_node(node, out) {
                count += 1;
            }
        }
        out[head + 8..head + 12].copy_from_slice(&count.to_le_bytes());
    }
    if let Some((world, set)) = world {
        let text = set.text.as_deref().filter(|t| t.len() <= MAX_SET_TEXT).unwrap_or("");
        let png: &[u8] = set.png.as_deref().filter(|p| p.len() <= MAX_PLATE_PNG).unwrap_or(&[]);
        let head = out.len();
        u32s(
            out,
            &[
                world.at.min(quads.len()) as u32,
                world.figures_at.min(quads.len()) as u32,
                0,
                set.key as u32,
                (set.key >> 32) as u32,
                set.plate,
            ],
        );
        for v in [set.scale].iter().chain(&set.offset) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        u32s(out, &[text.len() as u32, png.len() as u32, 0]);
        u32s(out, &patches_words(&set.patches));
        out.extend_from_slice(text.as_bytes());
        out.resize(out.len() + (4 - text.len() % 4) % 4, 0);
        out.extend_from_slice(png);
        out.resize(out.len() + (4 - png.len() % 4) % 4, 0);
        let mut count = 0u32;
        for node in world.figures.iter().take(MAX_NODES) {
            if encode_node(node, out) {
                count += 1;
            }
        }
        out[head + 8..head + 12].copy_from_slice(&count.to_le_bytes());
        let mut extra = 0u32;
        for node in world.extra.iter().take(MAX_NODES) {
            if encode_node(node, out) {
                extra += 1;
            }
        }
        out[head + 48..head + 52].copy_from_slice(&extra.to_le_bytes());
        if !world.more.is_empty() || world.look.is_some() {
            let more = &world.more[..world.more.len().min(MAX_MORE_SETS)];
            u32s(out, &[more.len() as u32]);
            for set in more {
                let text = set.text.as_deref().filter(|t| t.len() <= MAX_SET_TEXT).unwrap_or("");
                let png: &[u8] = set.png.as_deref().filter(|p| p.len() <= MAX_PLATE_PNG).unwrap_or(&[]);
                u32s(out, &[set.key as u32, (set.key >> 32) as u32, set.plate]);
                for v in [set.scale].iter().chain(&set.offset) {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                u32s(out, &[text.len() as u32, png.len() as u32]);
                u32s(out, &patches_words(&set.patches));
                out.extend_from_slice(text.as_bytes());
                out.resize(out.len() + (4 - text.len() % 4) % 4, 0);
                out.extend_from_slice(png);
                out.resize(out.len() + (4 - png.len() % 4) % 4, 0);
            }
        }
        if let Some(look) = &world.look {
            let text = look.to_text();
            let text = if text.len() <= MAX_LOOK_TEXT { text } else { String::new() };
            u32s(out, &[text.len() as u32]);
            out.extend_from_slice(text.as_bytes());
            out.resize(out.len() + (4 - text.len() % 4) % 4, 0);
        }
    }
}

/// Why a buffer is not a frame. The decoder draws nothing from a buffer it
/// refuses, whole: no half of a frame is ever handed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Shorter than the header.
    Short,
    /// Does not begin with the tag.
    Tag,
    /// The stage is empty or past [`MAX_SIDE`] on a side.
    Size,
    /// A picture is empty or past [`MAX_SIDE`] on a side.
    Picture,
    /// The pictures or the quads the header counts do not fit the buffer.
    Truncated,
    /// The props block says what cannot be: a count of nodes past the limit,
    /// a place past the quads, a grain out of range, a mesh that is none or
    /// a node blended two ways.
    Props,
    /// The world block says what cannot be: a place past the quads, too many
    /// figures, a set text too long or not UTF-8, a placing that is not a
    /// finite number (or no scale), a figure that cannot be.
    World,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Refused::Short => "shorter than a stage header",
            Refused::Tag => "not a stage frame (no STG2 to STG6 tag)",
            Refused::Size => "the stage has no size, or one past the texture limit",
            Refused::Picture => "a picture has no size, or one past the texture limit",
            Refused::Truncated => "the buffer is shorter than its header says",
            Refused::Props => "the props block says what cannot be",
            Refused::World => "the world block says what cannot be",
        })
    }
}

impl std::error::Error for Refused {}

/// A frame read off the wire: the frame (its `layers` empty: the wire
/// carries quads) and the quads, back to front.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub frame: Frame,
    pub quads: Vec<Quad>,
}

/// Read one frame from `bytes`, as [`encode`] wrote it (an `STG2` frame, or
/// an `STG3` one with its props, whose `at` stays a count of quads: that is
/// layers once [`crate::layers_of`] has turned the quads back). This is the other
/// half of the wire and holds it to what the page's reader always did
/// (`shell/stage.js`): the buffer is checked whole before anything is read
/// out of it, so a buffer that lies (a short one, a count past its end, a
/// picture or a stage without a size or past [`MAX_SIDE`]) is refused and
/// never half read. Every size is worked out in 64 bits, because the
/// browser's module is 32-bit and a count of four billion quads must not
/// wrap. Bytes after the last quad (of an `STG3` frame, the last node) are
/// ignored. Floats are not judged here:
/// the drawing side calls `Layer::sanitized` on everything it draws.
///
/// Two things differ from the page's reader, both toward safe:
/// - the floor: the wire repeats what [`Tilt::for_stage`] derives from the
///   frame's `tilt`, so the tilt is read back from `near x height`, the one
///   number that carries it, and the floor is derived again. A floor that is
///   not finite, or has no zoom, is flat, as it was.
/// - `enter` is a direction, so it is kept only when both parts are finite.
pub fn decode(bytes: &[u8]) -> Result<Decoded, Refused> {
    if bytes.len() < HEADER_BYTES {
        return Err(Refused::Short);
    }
    let with_look = &bytes[..4] == TAG_LOOK;
    let with_worlds = &bytes[..4] == TAG_WORLDS || with_look;
    let with_world = &bytes[..4] == TAG_WORLD || with_worlds;
    let with_props = &bytes[..4] == TAG_PROPS || with_world;
    if &bytes[..4] != TAG && !with_props {
        return Err(Refused::Tag);
    }
    let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let f32_at = |at: usize| f32::from_bits(u32_at(at));
    let (width, height) = (u32_at(4), u32_at(8));
    let (pictures, quads) = (u32_at(12), u32_at(16));
    let sided = |v: u32| (1..=MAX_SIDE).contains(&v);
    if !sided(width) || !sided(height) {
        return Err(Refused::Size);
    }
    // Walk the pictures by their headers, in 64 bits, before reading any.
    let len = bytes.len() as u64;
    let mut need = HEADER_BYTES as u64;
    for _ in 0..pictures {
        if need + 12 > len {
            return Err(Refused::Truncated);
        }
        let at = need as usize;
        let (w, h) = (u32_at(at + 4), u32_at(at + 8));
        if !sided(w) || !sided(h) {
            return Err(Refused::Picture);
        }
        need += 12 + u64::from(w) * u64::from(h) * 4;
        if need > len {
            return Err(Refused::Truncated);
        }
    }
    need += u64::from(quads) * QUAD_BYTES as u64;
    if need > len {
        return Err(Refused::Truncated);
    }
    // An STG3 frame's props block: counted before any of it is read.
    let mut props_at = None;
    if with_props {
        if need + PROPS_HEADER_BYTES as u64 > len {
            return Err(Refused::Truncated);
        }
        let at = need as usize;
        let (place, grain, count) = (u32_at(at), u32_at(at + 4), u32_at(at + 8));
        if u64::from(place) > u64::from(quads) || !(1..=u32::from(MAX_GRAIN)).contains(&grain) || count as usize > MAX_NODES {
            return Err(Refused::Props);
        }
        if need + PROPS_HEADER_BYTES as u64 + u64::from(count) * NODE_BYTES as u64 > len {
            return Err(Refused::Truncated);
        }
        props_at = Some((at + PROPS_HEADER_BYTES, place as usize, grain as u8, count));
        need += PROPS_HEADER_BYTES as u64 + u64::from(count) * NODE_BYTES as u64;
    }
    // An STG4 frame's world block, after the props: counted the same way.
    let mut world_at = None;
    if with_world {
        if need + WORLD_HEADER_BYTES as u64 > len {
            return Err(Refused::Truncated);
        }
        let at = need as usize;
        let (place, figures_place, count, text, png, extra) =
            (u32_at(at), u32_at(at + 4), u32_at(at + 8), u32_at(at + 40), u32_at(at + 44), u32_at(at + 48));
        let placing = [24, 28, 32, 36].map(|k| f32_at(at + k));
        if u64::from(place) > u64::from(quads)
            || u64::from(figures_place) > u64::from(quads)
            || count as usize > MAX_NODES
            || extra as usize > MAX_NODES
            || text as usize > MAX_SET_TEXT
            || png as usize > MAX_PLATE_PNG
            || !placing.iter().all(|v| v.is_finite())
            || placing[0] <= 0.0
        {
            return Err(Refused::World);
        }
        let padded = u64::from(text).div_ceil(4) * 4;
        let png_padded = u64::from(png).div_ceil(4) * 4;
        let nodes = u64::from(count) + u64::from(extra);
        if need + WORLD_HEADER_BYTES as u64 + padded + png_padded + nodes * NODE_BYTES as u64 > len {
            return Err(Refused::Truncated);
        }
        let text_at = at + WORLD_HEADER_BYTES;
        let text = match text {
            0 => None,
            n => Some(std::str::from_utf8(&bytes[text_at..text_at + n as usize]).map_err(|_| Refused::World)?),
        };
        let png_at = text_at + padded as usize;
        let set = SetRef {
            key: u64::from(u32_at(at + 12)) | u64::from(u32_at(at + 16)) << 32,
            text: text.map(Into::into),
            plate: u32_at(at + 20),
            png: (png > 0).then(|| bytes[png_at..png_at + png as usize].into()),
            scale: placing[0],
            offset: [placing[1], placing[2], placing[3]],
            patches: patches_of(&|k| u32_at(at + 52 + 4 * k)),
        };
        need += WORLD_HEADER_BYTES as u64 + padded + png_padded + nodes * NODE_BYTES as u64;
        // An STG5 frame's further sets, after the extra blocks.
        let mut more = Vec::new();
        if with_worlds {
            if need + 4 > len {
                return Err(Refused::Truncated);
            }
            let sets = u32_at(need as usize);
            if sets as usize > MAX_MORE_SETS {
                return Err(Refused::World);
            }
            need += 4;
            for _ in 0..sets {
                if need + MORE_HEADER_BYTES as u64 > len {
                    return Err(Refused::Truncated);
                }
                let at = need as usize;
                let (text, png) = (u32_at(at + 28), u32_at(at + 32));
                let placing = [12, 16, 20, 24].map(|k| f32_at(at + k));
                if text as usize > MAX_SET_TEXT || png as usize > MAX_PLATE_PNG || !placing.iter().all(|v| v.is_finite()) || placing[0] <= 0.0 {
                    return Err(Refused::World);
                }
                let padded = u64::from(text).div_ceil(4) * 4;
                let png_padded = u64::from(png).div_ceil(4) * 4;
                if need + MORE_HEADER_BYTES as u64 + padded + png_padded > len {
                    return Err(Refused::Truncated);
                }
                let text_at = at + MORE_HEADER_BYTES;
                let png_at = text_at + padded as usize;
                more.push(SetRef {
                    key: u64::from(u32_at(at)) | u64::from(u32_at(at + 4)) << 32,
                    text: match text {
                        0 => None,
                        n => Some(std::str::from_utf8(&bytes[text_at..text_at + n as usize]).map_err(|_| Refused::World)?.into()),
                    },
                    plate: u32_at(at + 8),
                    png: (png > 0).then(|| bytes[png_at..png_at + png as usize].into()),
                    scale: placing[0],
                    offset: [placing[1], placing[2], placing[3]],
                    patches: patches_of(&|k| u32_at(at + 36 + 4 * k)),
                });
                need += MORE_HEADER_BYTES as u64 + padded + png_padded;
            }
        }
        // An STG6 frame's look, after the further sets.
        let mut look = None;
        if with_look {
            if need + 4 > len {
                return Err(Refused::Truncated);
            }
            let n = u32_at(need as usize);
            if n as usize > MAX_LOOK_TEXT || need + 4 + u64::from(n).div_ceil(4) * 4 > len {
                return Err(Refused::World);
            }
            let text_at = need as usize + 4;
            let text = std::str::from_utf8(&bytes[text_at..text_at + n as usize]).map_err(|_| Refused::World)?;
            look = crate::look::Look::from_text(text);
        }
        world_at = Some((png_at + png_padded as usize, place as usize, figures_place as usize, count, extra, set, more, look));
    }

    let [ex, ey] = [f32_at(24), f32_at(28)];
    let enter = (ex.is_finite() && ey.is_finite()).then_some([ex, ey]);
    let floor = [32, 36, 40, 44, 48].map(f32_at);
    let tilt = if floor.iter().all(|v| v.is_finite()) && floor[3] > 0.0 {
        (floor[2] * height as f32).clamp(0.0, crate::MAX_TILT)
    } else {
        0.0
    };

    let impact = f32_at(52);
    let impact = if impact.is_finite() { impact.clamp(0.0, 1.0) } else { 0.0 };

    // Everything below lies inside `need`, which the walk above checked.
    let mut at = HEADER_BYTES;
    let mut decoded_pictures = Vec::with_capacity(pictures as usize);
    for _ in 0..pictures {
        let (id, w, h) = (u32_at(at), u32_at(at + 4), u32_at(at + 8));
        let size = w as usize * h as usize * 4;
        at += 12;
        decoded_pictures.push(crate::Picture { id, width: w, height: h, rgba: bytes[at..at + size].to_vec() });
        at += size;
    }
    let mut decoded_quads = Vec::with_capacity(quads as usize);
    for _ in 0..quads {
        let floats = |from: usize| [0, 4, 8, 12].map(|k| f32_at(at + from + k));
        let [angle, stand, fx, fy] = floats(48);
        // The page's thresholds: under a half is the floor, under one and a
        // half what stands on it, anything else (a NaN too) the screen's own.
        let stand = if stand < 0.5 {
            Stand::Floor
        } else if stand < 1.5 {
            Stand::Upright([fx, fy])
        } else {
            Stand::Screen
        };
        decoded_quads.push(Quad {
            rect: floats(0),
            source: floats(16),
            color: floats(32),
            angle,
            stand,
            picture: u32_at(at + 64),
            // Any shape but 0 is the round one, as the page's shader read it.
            shape: if u32_at(at + 68) == 0 { Shape::Fill } else { Shape::Round },
        });
        at += QUAD_BYTES;
    }
    let props = match props_at {
        Some((first, at, grain, count)) => {
            let mut nodes = Vec::with_capacity(count as usize);
            for k in 0..count as usize {
                nodes.push(decode_node(&|i| u32_at(first + k * NODE_BYTES + 4 * i)).ok_or(Refused::Props)?);
            }
            let world = match world_at {
                Some((first, at, figures_at, count, extra, set, more, look)) => {
                    let node = |k: usize| decode_node(&|i| u32_at(first + k * NODE_BYTES + 4 * i)).ok_or(Refused::World);
                    let figures = (0..count as usize).map(node).collect::<Result<Vec<_>, _>>()?;
                    let extra = (count as usize..(count + extra) as usize).map(node).collect::<Result<Vec<_>, _>>()?;
                    Some(World { extra, at, figures, figures_at, set: Some(set), more, look, ..World::default() })
                }
                None => None,
            };
            Some(Props { at, grain, nodes, world })
        }
        None => None,
    };
    Ok(Decoded {
        frame: Frame {
            size: (width, height),
            layers: Vec::new(),
            pictures: decoded_pictures,
            scene: u64::from(u32_at(20)),
            enter,
            tilt,
            props,
            impact,
        },
        quads: decoded_quads,
    })
}

/// One node from its words (`word(i)` is the i-th u32 of its record), or
/// nothing for a record that says what cannot be.
fn decode_node(word: &dyn Fn(usize) -> u32) -> Option<Node> {
    let f = |i: usize| f32::from_bits(word(i));
    let flags = word(0);
    if flags & BLENDED != 0 && flags & PREMULTIPLIED != 0 {
        return None;
    }
    let mesh = mesh_of(word(1), word(2))?;
    let alpha = if flags & BLENDED != 0 {
        AlphaMode::Blend
    } else if flags & PREMULTIPLIED != 0 {
        AlphaMode::Premultiplied
    } else {
        AlphaMode::Opaque
    };
    let long = |lo: usize| u64::from(word(lo)) | u64::from(word(lo + 1)) << 32;
    Some(Node {
        id: long(4),
        parent: (flags & HAS_PARENT != 0).then(|| long(6)),
        transform: Transform {
            translation: [f(8), f(9), f(10)],
            rotation: [f(11), f(12), f(13), f(14)],
            scale: [f(15), f(16), f(17)],
        },
        mesh,
        material: Material {
            source: if flags & HAS_PICTURE != 0 { TextureSource::Picture(word(3)) } else { TextureSource::None },
            tint: [f(18), f(19), f(20), f(21)],
            sampling: if flags & LINEAR != 0 { Sampling::Linear } else { Sampling::Nearest },
            alpha,
            double_sided: flags & DOUBLE_SIDED != 0,
            lit: flags & LIT != 0,
            emissive: f(22),
            // What it is to a world's look travels with it (`STG6`).
            accent: flags & ACCENT != 0,
            // Nor does a projector: a projected plate is a scene's own.
            projector: None,
            patch: None,
            role: role_of(flags >> ROLE_SHIFT),
            ceiling: 0,
        },
        face_materials: Vec::new(),
        visible: flags & SHOWN != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_TILT, Picture};
    use crate::set::Kind;

    #[test]
    fn a_frame_is_its_header_its_pictures_then_its_quads() {
        let frame = Frame {
            size: (384, 224),
            pictures: vec![Picture { id: 5, width: 1, height: 1, rgba: vec![1, 2, 3, 4] }],
            scene: 9,
            ..Default::default()
        };
        let quad = Quad {
            rect: [1.0, 2.0, 3.0, 4.0],
            picture: 5,
            source: [0.0, 0.0, 1.0, 1.0],
            color: [1.0; 4],
            shape: Shape::Round,
            angle: 0.5,
            stand: Stand::Upright([7.0, 8.0]),
        };
        let mut out = Vec::new();
        encode(&frame, &[quad], &mut out);
        assert_eq!(&out[..4], TAG);
        assert_eq!(out.len(), HEADER_BYTES + 12 + 4 + QUAD_BYTES);
        let u32_at = |i: usize| u32::from_le_bytes(out[i..i + 4].try_into().unwrap());
        let f32_at = |i: usize| f32::from_le_bytes(out[i..i + 4].try_into().unwrap());
        assert_eq!((u32_at(4), u32_at(8), u32_at(12), u32_at(16), u32_at(20)), (384, 224, 1, 1, 9));
        assert_eq!(f32_at(32), 0.0, "a flat frame has no tilt");
        assert_eq!(&out[HEADER_BYTES + 12..HEADER_BYTES + 16], &[1, 2, 3, 4]);
        let quad_at = HEADER_BYTES + 16;
        assert_eq!((f32_at(quad_at + 48), f32_at(quad_at + 52)), (0.5, 1.0), "turned, upright");
        assert_eq!((f32_at(quad_at + 56), f32_at(quad_at + 60)), (7.0, 8.0), "on its foot");
        assert_eq!(u32_at(out.len() - 4), 1, "round");
    }

    #[test]
    fn an_impact_rides_in_the_header_held_to_one() {
        let mut out = Vec::new();
        for (sent, back) in [(0.0, 0.0), (0.4, 0.4), (3.0, 1.0), (-1.0, 0.0), (f32::NAN, 0.0)] {
            encode(&Frame { size: (384, 224), impact: sent, ..Default::default() }, &[], &mut out);
            assert_eq!(out.len(), HEADER_BYTES, "four bytes, the same in every frame");
            assert_eq!(decode(&out).expect("a frame").frame.impact, back, "{sent}");
        }
    }

    #[test]
    fn malformed_pictures_stay_off_the_wire_and_out_of_the_count() {
        let frame = Frame {
            size: (384, 224),
            pictures: vec![
                // Claims 2x2, carries one pixel: would desync the reader.
                Picture { id: 5, width: 2, height: 2, rgba: vec![1, 2, 3, 4] },
                Picture { id: 6, width: 0, height: 4, rgba: vec![] },
                Picture { id: 7, width: 9000, height: 1, rgba: vec![0; 36000] },
                Picture { id: 8, width: 1, height: 1, rgba: vec![9, 9, 9, 9] },
            ],
            scene: 1,
            ..Default::default()
        };
        let mut out = Vec::new();
        encode(&frame, &[], &mut out);
        let u32_at = |i: usize| u32::from_le_bytes(out[i..i + 4].try_into().unwrap());
        assert_eq!(u32_at(12), 1, "only the sound picture is counted");
        // Header + one picture header (12) + its pixel (4), no quads.
        assert_eq!(out.len(), HEADER_BYTES + 16);
        assert_eq!(u32_at(HEADER_BYTES), 8, "the sound picture is the one written");
        assert_eq!(&out[HEADER_BYTES + 12..HEADER_BYTES + 16], &[9, 9, 9, 9]);
    }

    fn frame_with(pictures: Vec<Picture>, tilt: f32, enter: Option<[f32; 2]>) -> Frame {
        Frame { size: (384, 224), pictures, scene: 9, enter, tilt, ..Default::default() }
    }

    fn quad(i: u32) -> Quad {
        let f = i as f32;
        Quad {
            rect: [f, f + 1.0, 10.0, 12.0],
            picture: i % 3,
            source: [0.0, 1.0, 2.0, 3.0],
            color: [0.1, 0.2, 0.3, 0.5],
            shape: if i.is_multiple_of(2) { Shape::Fill } else { Shape::Round },
            angle: 0.25 * f,
            stand: [Stand::Floor, Stand::Upright([f, 2.0 * f]), Stand::Screen][i as usize % 3],
        }
    }

    #[test]
    fn a_frame_decodes_to_what_was_encoded() {
        let pictures = vec![
            Picture { id: 5, width: 2, height: 1, rgba: vec![1, 2, 3, 4, 5, 6, 7, 8] },
            Picture { id: 0x8000_0001, width: 1, height: 2, rgba: vec![9; 8] },
        ];
        let quads: Vec<Quad> = (0..9).map(quad).collect();
        for (tilt, enter) in [(0.0, None), (0.3, Some([1.0, 0.0])), (MAX_TILT, Some([0.0, -1.0]))] {
            let frame = frame_with(pictures.clone(), tilt, enter);
            let mut out = Vec::new();
            encode(&frame, &quads, &mut out);
            let back = decode(&out).expect("what encode wrote");
            assert_eq!(back.frame.size, frame.size);
            assert_eq!(back.frame.pictures, frame.pictures);
            assert_eq!(back.frame.scene, 9);
            assert_eq!(back.frame.enter, enter);
            assert!(back.frame.layers.is_empty(), "the wire carries quads, not layers");
            assert!((back.frame.tilt - tilt).abs() < 1e-6, "{} for {tilt}", back.frame.tilt);
            assert_eq!(back.quads, quads);
        }
    }

    #[test]
    fn a_frame_with_nothing_decodes_to_nothing() {
        let mut out = Vec::new();
        encode(&Frame { size: (1, 1), ..Default::default() }, &[], &mut out);
        let back = decode(&out).expect("the header alone");
        assert!(back.frame.pictures.is_empty() && back.quads.is_empty());
        assert_eq!((back.frame.tilt, back.frame.enter), (0.0, None));
    }

    /// A frame as the page's reader fuzzes it (`probe/stage-fuzz.mjs`), the
    /// header's counts settable apart from what follows.
    fn raw(w: u32, h: u32, pics: &[(u32, u32, u32, usize)], quads: usize, counts: Option<(u32, u32)>) -> Vec<u8> {
        let mut b = TAG.to_vec();
        let (np, nq) = counts.unwrap_or((pics.len() as u32, quads as u32));
        for v in [w, h, np, nq, 9] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [f32::NAN, f32::NAN, w as f32 / 2.0, h as f32 * 0.85, 0.3 / h as f32, 1.0, 0.925, 0.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for &(id, pw, ph, bytes) in pics {
            for v in [id, pw, ph] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.resize(b.len() + bytes, 7);
        }
        for i in 0..quads {
            for _ in 0..12 {
                b.extend_from_slice(&1.0f32.to_le_bytes());
            }
            for v in [0.3f32, (i % 3) as f32, 5.0, 9.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.extend_from_slice(&5u32.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
        }
        b
    }

    #[test]
    fn a_buffer_that_lies_is_refused_whole() {
        let ok = |b: &[u8]| decode(b).map(|d| (d.frame.pictures.len(), d.quads.len()));
        assert_eq!(ok(&raw(384, 224, &[(5, 1, 1, 4)], 1, None)), Ok((1, 1)));
        assert_eq!(ok(&[0; 10]), Err(Refused::Short));
        let mut tag = raw(384, 224, &[(5, 1, 1, 4)], 1, None);
        tag[0] = 0;
        assert_eq!(ok(&tag), Err(Refused::Tag));
        assert_eq!(ok(&raw(0, 224, &[], 0, None)), Err(Refused::Size));
        assert_eq!(ok(&raw(384, 0, &[], 0, None)), Err(Refused::Size));
        assert_eq!(ok(&raw(9000, 224, &[], 0, None)), Err(Refused::Size));
        assert_eq!(ok(&raw(384, 224, &[(5, 9000, 1, 36000)], 0, None)), Err(Refused::Picture));
        assert_eq!(ok(&raw(384, 224, &[(5, 0, 1, 0)], 0, None)), Err(Refused::Picture));
        assert_eq!(ok(&raw(384, 224, &[(5, 1, 0, 0)], 0, None)), Err(Refused::Picture));
        // The picture's pixels run out; so do the quads'.
        let mut short = raw(384, 224, &[(5, 1, 1, 4)], 1, None);
        short.truncate(short.len() - 74);
        assert_eq!(ok(&short), Err(Refused::Truncated));
        let mut short = raw(384, 224, &[(5, 1, 1, 4)], 1, None);
        short.truncate(short.len() - 10);
        assert_eq!(ok(&short), Err(Refused::Truncated));
        // Counts past the end, up to the most a u32 can say: no wrap on a
        // 32-bit target, no allocation of what is not there.
        // (Reading the quad as a picture header, it is a picture past the limit.)
        assert!(ok(&raw(384, 224, &[(5, 1, 1, 4)], 1, Some((5, 1)))).is_err());
        assert_eq!(ok(&raw(384, 224, &[(5, 1, 1, 4)], 1, Some((1, 100_000)))), Err(Refused::Truncated));
        assert_eq!(ok(&raw(384, 224, &[], 0, Some((u32::MAX, 0)))), Err(Refused::Truncated));
        assert_eq!(ok(&raw(384, 224, &[], 0, Some((0, u32::MAX)))), Err(Refused::Truncated));
        assert_eq!(ok(&raw(384, 224, &[(5, 8192, 8192, 4)], 0, None)), Err(Refused::Truncated), "a picture that cannot all be there");
        // Bytes past the last quad are not read.
        let mut long = raw(384, 224, &[(5, 1, 1, 4)], 1, None);
        long.extend_from_slice(&[1, 2, 3]);
        assert_eq!(ok(&long), Ok((1, 1)));
    }

    #[test]
    fn a_floor_that_is_not_one_is_flat_and_a_direction_that_is_not_one_is_none() {
        let mut b = raw(384, 224, &[], 0, None);
        // The floor's five numbers are at 32.., zoom the fourth.
        b[44..48].copy_from_slice(&0.0f32.to_le_bytes());
        assert_eq!(decode(&b).unwrap().frame.tilt, 0.0, "no zoom, flat");
        let mut b = raw(384, 224, &[], 0, None);
        b[36..40].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert_eq!(decode(&b).unwrap().frame.tilt, 0.0, "not finite, flat");
        let mut b = raw(384, 224, &[], 0, None);
        b[24..28].copy_from_slice(&1.0f32.to_le_bytes());
        assert_eq!(decode(&b).unwrap().frame.enter, None, "one part NaN");
        b[28..32].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert_eq!(decode(&b).unwrap().frame.enter, None, "one part infinite");
    }

    #[test]
    fn mutated_and_garbage_buffers_never_panic_and_what_parses_is_sound() {
        // The wire arrives from a socket: any byte may be anything.
        let mut frame = frame_with(
            vec![Picture { id: 5, width: 2, height: 2, rgba: (0..16).collect() }],
            0.3,
            Some([1.0, 0.0]),
        );
        frame.scene = 4;
        let quads: Vec<Quad> = (0..3).map(quad).collect();
        let mut valid = Vec::new();
        encode(&frame, &quads, &mut valid);
        let mut state = 0x2545_f491_u32;
        let mut rnd = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            state as f64 / 4_294_967_296.0
        };
        let (mut parsed, mut refused) = (0, 0);
        let mut check = |buf: &[u8]| match decode(buf) {
            Ok(d) => {
                parsed += 1;
                assert!((1..=MAX_SIDE).contains(&d.frame.size.0) && (1..=MAX_SIDE).contains(&d.frame.size.1));
                for p in &d.frame.pictures {
                    assert!(well_formed(p), "a picture decode hands on is one encode would write");
                }
                // Everything it handed on was in the buffer.
                let pixels: usize = d.frame.pictures.iter().map(|p| p.rgba.len() + 12).sum();
                assert!(HEADER_BYTES + pixels + d.quads.len() * QUAD_BYTES <= buf.len());
                assert!(d.frame.tilt.is_finite() && (0.0..=crate::MAX_TILT).contains(&d.frame.tilt));
                assert!(d.frame.enter.is_none_or(|e| e.iter().all(|v| v.is_finite())));
            }
            Err(_) => refused += 1,
        };
        for _ in 0..3000 {
            let mut buf = valid.clone();
            for _ in 0..1 + (rnd() * 8.0) as usize {
                let at = (rnd() * buf.len() as f64) as usize;
                buf[at] = (rnd() * 256.0) as u8;
            }
            let len = if rnd() < 0.25 { (rnd() * buf.len() as f64) as usize } else { buf.len() };
            check(&buf[..len]);
        }
        for _ in 0..500 {
            let buf: Vec<u8> = (0..(rnd() * 400.0) as usize).map(|_| (rnd() * 256.0) as u8).collect();
            check(&buf);
        }
        // Garbage that opens with the tag and a plausible size.
        for _ in 0..500 {
            let mut buf = TAG.to_vec();
            buf.extend((0..(rnd() * 300.0) as usize).map(|_| (rnd() * 256.0) as u8));
            check(&buf);
        }
        // Some of the mutations land in pixels or floats and still parse;
        // most that hit the header or a count do not.
        assert!(parsed > 100 && refused > 100, "parsed {parsed}, refused {refused}");
    }

    // ---- props (STG3) ----

    fn prop(id: u64, parent: Option<u64>, mesh: Option<MeshRef>) -> Node {
        Node {
            id,
            parent,
            transform: Transform {
                translation: [1.5, 2.5, -3.5],
                rotation: [0.0, 0.70710677, 0.0, 0.70710677],
                scale: [8.0, 1.0, 0.5],
            },
            mesh,
            material: Material {
                source: if id.is_multiple_of(2) { TextureSource::Picture(77) } else { TextureSource::None },
                tint: [0.25, 0.5, 0.75, 0.5],
                sampling: Sampling::Linear,
                alpha: AlphaMode::Blend,
                double_sided: true,
                lit: true,
                emissive: 0.4,
                accent: false,
                projector: None,
                patch: None,
                role: Default::default(),
                ceiling: 0,
            },
            face_materials: Vec::new(),
            visible: id != 3,
        }
    }

    fn with_props(props: Props) -> Frame {
        Frame { props: Some(props), ..frame_with(vec![], 0.3, None) }
    }

    #[test]
    fn a_frame_with_props_is_an_stg3_and_one_without_stays_an_stg2() {
        let mut plain = Vec::new();
        encode(&frame_with(vec![], 0.3, None), &[], &mut plain);
        assert_eq!(&plain[..4], TAG);
        assert_eq!(plain.len(), HEADER_BYTES, "an STG2 frame is the bytes it always was");
        assert_eq!(decode(&plain).unwrap().frame.props, None);

        let nodes = vec![prop(1, None, Some(MeshRef::Gem)), prop(2, Some(1), None)];
        let mut out = Vec::new();
        encode(&with_props(Props { at: 0, grain: 2, nodes, world: None }), &[], &mut out);
        assert_eq!(&out[..4], TAG_PROPS);
        assert_eq!(out.len(), HEADER_BYTES + PROPS_HEADER_BYTES + 2 * NODE_BYTES);
        // Everything before the block reads as the STG2 header does.
        assert_eq!(&out[4..HEADER_BYTES], &plain[4..HEADER_BYTES]);
    }

    #[test]
    fn props_come_back_as_they_went() {
        let meshes = [
            Some(MeshRef::Quad),
            Some(MeshRef::Cube),
            Some(MeshRef::Plane),
            Some(MeshRef::Sphere { segments: 12 }),
            Some(MeshRef::Gem),
            Some(MeshRef::Cone { segments: 5 }),
            Some(MeshRef::Uploaded(9001)),
            None,
        ];
        let mut nodes: Vec<Node> = meshes
            .iter()
            .enumerate()
            .map(|(i, &m)| prop(i as u64 + 1, (i > 0).then_some(i as u64), m))
            .collect();
        // Ids and parents are 64-bit, flags are each their own, and a premultiplied one is not a blended one.
        nodes[1].id = 0x1234_5678_9abc_def0;
        nodes[2].parent = Some(u64::MAX);
        nodes[4].material = Material { alpha: AlphaMode::Premultiplied, sampling: Sampling::Nearest, ..Material::default() };
        nodes[5].material = Material::lit([1.0, 0.0, 0.0, 1.0], 1.0);
        let quads: Vec<Quad> = (0..5).map(quad).collect();
        for tilt in [0.0, 0.3] {
            let mut frame = with_props(Props { at: 3, grain: 4, nodes: nodes.clone(), world: None });
            frame.tilt = tilt;
            let mut out = Vec::new();
            encode(&frame, &quads, &mut out);
            let back = decode(&out).expect("what encode wrote");
            assert_eq!(back.quads, quads);
            assert_eq!(back.frame.props, frame.props);
            assert!((back.frame.tilt - tilt).abs() < 1e-6);
        }
    }

    #[test]
    fn what_the_wire_cannot_carry_is_left_out_not_garbled() {
        let mut faces = prop(1, None, Some(MeshRef::Cube));
        faces.face_materials = vec![Material::default()];
        let props = Props { at: 0, grain: 1, nodes: vec![faces, prop(2, None, Some(MeshRef::Gem))], world: None };
        let mut out = Vec::new();
        encode(&with_props(props), &[], &mut out);
        let back = decode(&out).unwrap().frame.props.unwrap();
        assert_eq!(back.nodes.len(), 1);
        assert_eq!(back.nodes[0].id, 2);
        // A place past the quads, and a grain out of range, are said as the most there is.
        let props = Props { at: 99, grain: 200, nodes: vec![], world: None };
        encode(&with_props(props), &[quad(0)], &mut out);
        let back = decode(&out).unwrap().frame.props.unwrap();
        assert_eq!((back.at, back.grain), (1, MAX_GRAIN));
    }

    /// A frame with a world made from a set, its figures `figures`, and a
    /// prop of its own.
    fn with_world(text: Option<&str>, figures: Vec<Node>) -> Frame {
        let png: Option<std::sync::Arc<[u8]>> = text.map(|_| b"\x89PNG plate".as_slice().into());
        let set = SetRef { key: 0x0123_4567_89ab_cdef, text: text.map(Into::into), plate: 0x8000_0028, png, scale: 0.5, offset: [-3.5, 0.0, 12.25], patches: Patches::of(0x8000_1000, [(Kind::Ground, 64, 64), (Kind::Path, 9, 33), (Kind::Fire, 64, 8)]) };
        let world = World {
            floor: vec![prop(9, None, Some(MeshRef::Plane))],
            blocks: vec![],
            extra: vec![prop(11, None, Some(MeshRef::Quad)), prop(12, None, Some(MeshRef::Cube))],
            at: 1,
            figures,
            figures_at: 2,
            set: Some(set),
            more: Vec::new(),
            look: None,
            meshes: Vec::new(),
        };
        with_props(Props { at: 3, grain: 2, nodes: vec![prop(1, None, Some(MeshRef::Gem))], world: Some(world) })
    }

    #[test]
    fn a_world_with_a_look_goes_as_stg6_and_its_look_comes_back() {
        let mut frame = with_world(Some("set a\n"), vec![]);
        let mut look = crate::look::Look::new(vec![[10, 20, 30], [200, 210, 220]]);
        look.sun = Some(crate::look::Sun { toward: [-0.5, 0.5, 0.5], strength: 1.0, ambient: 0.3, shadow: None, rise: 0.0 });
        look.metre = 11.0;
        frame.props.as_mut().unwrap().world.as_mut().unwrap().look = Some(look.clone());
        let mut out = Vec::new();
        encode(&frame, &[], &mut out);
        assert_eq!(&out[..4], TAG_LOOK);
        let world = decode(&out).unwrap().frame.props.unwrap().world.unwrap();
        assert_eq!(world.look, Some(look), "the look, exactly");
        assert!(world.more.is_empty());
        assert_eq!(world.set.as_ref().and_then(|s| s.text.as_deref()), Some("set a\n"));
        // Cut anywhere, refused whole.
        for cut in [out.len() - 1, out.len() - 8] {
            assert!(decode(&out[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn a_world_of_several_sets_goes_by_each_ones_reference() {
        let mut frame = with_world(Some("set a\n"), vec![]);
        let more = vec![
            SetRef { key: 7, text: Some("set b\n".into()), plate: 0x8000_0031, png: Some(b"\x89PNG b".as_slice().into()), scale: 1.0, offset: [-9.0, 0.0, -4.0], patches: Patches::of(0x8000_2000, [(Kind::Fire, 17, 64)]) },
            SetRef { key: 8, text: None, plate: 0x8000_0032, png: None, scale: 1.0, offset: [1.5, 0.0, 2.5], patches: Patches::NONE },
        ];
        frame.props.as_mut().unwrap().world.as_mut().unwrap().more = more.clone();
        let mut out = Vec::new();
        encode(&frame, &[quad(0), quad(1), quad(2)], &mut out);
        assert_eq!(&out[..4], TAG_WORLDS);
        let world = decode(&out).unwrap().frame.props.unwrap().world.unwrap();
        assert_eq!(world.more, more, "each further set by its reference, its text and PNG when they go");
        assert_eq!(world.extra.len(), 2, "its extra blocks as before");
        assert_eq!(world.set.unwrap().key, 0x0123_4567_89ab_cdef);
        // Cut anywhere in the further sets, it is refused whole.
        for cut in [4, 20, 40] {
            assert_eq!(decode(&out[..out.len() - cut]).map(|_| ()), Err(Refused::Truncated));
        }
        // One set is STG4, as ever.
        frame.props.as_mut().unwrap().world.as_mut().unwrap().more.clear();
        encode(&frame, &[quad(0), quad(1), quad(2)], &mut out);
        assert_eq!(&out[..4], TAG_WORLD);
    }

    #[test]
    fn a_world_goes_by_its_sets_reference_and_its_figures() {
        let text = "set yard\ncamera room 768 448\nbox stone at 1 0 1 size 2 2 2\n";
        let figures: Vec<Node> = (0..2).map(|i| prop(i + 1, None, Some(MeshRef::Quad))).collect();
        let quads: Vec<Quad> = (0..4).map(quad).collect();
        for text in [Some(text), Some("é"), None] {
            let frame = with_world(text, figures.clone());
            let mut out = Vec::new();
            encode(&frame, &quads, &mut out);
            assert_eq!(&out[..4], TAG_WORLD);
            let back = decode(&out).expect("what encode wrote");
            assert_eq!(back.quads, quads);
            let props = back.frame.props.unwrap();
            assert_eq!(props.nodes, frame.props.as_ref().unwrap().nodes, "the props as ever");
            let world = props.world.expect("the world");
            let sent = frame.props.as_ref().unwrap().world.as_ref().unwrap();
            // Its floor and blocks are not on the wire: the far side makes them from the set.
            assert!(world.floor.is_empty() && world.blocks.is_empty());
            assert_eq!((world.at, world.figures_at, &world.figures), (1, 2, &figures));
            assert_eq!(world.extra, sent.extra, "the blocks its set has not go as nodes");
            assert_eq!(world.set, sent.set, "the reference, its text with it when it went");
        }
        // A world made of nodes alone does not go: the frame is an STG3 without it.
        let mut frame = with_world(None, figures.clone());
        frame.props.as_mut().unwrap().world.as_mut().unwrap().set = None;
        let mut out = Vec::new();
        encode(&frame, &quads, &mut out);
        assert_eq!(&out[..4], TAG_PROPS);
        assert_eq!(decode(&out).unwrap().frame.props.unwrap().world, None);
        // Places past the quads are said as the most there is.
        encode(&with_world(None, vec![]), &[], &mut out);
        let world = decode(&out).unwrap().frame.props.unwrap().world.unwrap();
        assert_eq!((world.at, world.figures_at), (0, 0));
    }

    #[test]
    fn a_world_block_that_lies_is_refused_whole() {
        let text = "set yard\ncamera room 768 448\n";
        let quads: Vec<Quad> = (0..4).map(quad).collect();
        let mut good = Vec::new();
        encode(&with_world(Some(text), vec![prop(1, None, Some(MeshRef::Quad))]), &quads, &mut good);
        assert!(decode(&good).is_ok());
        // Where the world block starts: after the props' one node.
        let head = HEADER_BYTES + 4 * QUAD_BYTES + PROPS_HEADER_BYTES + NODE_BYTES;
        let word = |b: &mut Vec<u8>, at: usize, v: u32| b[head + at..head + at + 4].copy_from_slice(&v.to_le_bytes());
        let lied = |at: usize, v: u32| {
            let mut b = good.clone();
            word(&mut b, at, v);
            decode(&b).map(|_| ())
        };
        assert_eq!(lied(0, 5), Err(Refused::World), "a place past the quads");
        assert_eq!(lied(4, 5), Err(Refused::World), "the figures' place past the quads");
        assert_eq!(lied(8, MAX_NODES as u32 + 1), Err(Refused::World), "too many figures");
        assert_eq!(lied(8, 2), Err(Refused::Truncated), "more figures than there are");
        assert_eq!(lied(24, 0.0f32.to_bits()), Err(Refused::World), "no scale");
        assert_eq!(lied(28, f32::NAN.to_bits()), Err(Refused::World), "an offset that is not a number");
        assert_eq!(lied(40, MAX_SET_TEXT as u32 + 1), Err(Refused::World), "a text too long");
        assert_eq!(lied(40, 4000), Err(Refused::Truncated), "a text longer than the bytes");
        assert_eq!(lied(44, MAX_PLATE_PNG as u32 + 1), Err(Refused::World), "a plate too long");
        assert_eq!(lied(44, 4000), Err(Refused::Truncated), "a plate longer than the bytes");
        assert_eq!(lied(48, MAX_NODES as u32 + 1), Err(Refused::World), "too many extra blocks");
        assert_eq!(lied(48, 3), Err(Refused::Truncated), "more extra blocks than there are");
        let mut b = good.clone();
        b[head + WORLD_HEADER_BYTES] = 0xff;
        assert_eq!(decode(&b).map(|_| ()), Err(Refused::World), "a text that is not UTF-8");
        // Cut anywhere inside it: refused, never half read.
        for cut in head..good.len() {
            assert!(decode(&good[..cut]).is_err(), "cut at {cut}");
        }
    }

    /// A props block as it is read, the header's words and then `body`.
    fn block(head: [u32; 3], body: &[u8]) -> Vec<u8> {
        let mut b = raw(384, 224, &[], 2, None);
        b[..4].copy_from_slice(TAG_PROPS);
        for v in head {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(body);
        b
    }

    fn one_node_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        encode(&with_props(Props { at: 0, grain: 1, nodes: vec![prop(1, None, Some(MeshRef::Gem))], world: None }), &[], &mut out);
        out[HEADER_BYTES + PROPS_HEADER_BYTES..].to_vec()
    }

    #[test]
    fn a_props_block_that_lies_is_refused_whole() {
        let node = one_node_bytes();
        assert_eq!(node.len(), NODE_BYTES);
        let ok = |b: &[u8]| decode(b).map(|d| d.frame.props.map(|p| (p.at, p.grain, p.nodes.len())));
        assert_eq!(ok(&block([2, 3, 1], &node)), Ok(Some((2, 3, 1))));
        assert_eq!(ok(&block([0, 1, 0], &[])), Ok(Some((0, 1, 0))), "an empty block");
        // No block at all, or half a header, though the tag promises one.
        assert_eq!(ok(&block([0, 1, 0], &[])[..HEADER_BYTES + 2 * QUAD_BYTES]), Err(Refused::Truncated));
        assert_eq!(ok(&block([0, 1, 0], &[])[..HEADER_BYTES + 2 * QUAD_BYTES + 8]), Err(Refused::Truncated));
        // A count the bytes do not cover, up to the most a u32 can say.
        assert_eq!(ok(&block([0, 1, 1], &[])), Err(Refused::Truncated));
        assert_eq!(ok(&block([0, 1, 1], &node[..NODE_BYTES - 1])), Err(Refused::Truncated));
        assert_eq!(ok(&block([0, 1, 2], &node)), Err(Refused::Truncated));
        assert_eq!(ok(&block([0, 1, MAX_NODES as u32], &node)), Err(Refused::Truncated));
        assert_eq!(ok(&block([0, 1, MAX_NODES as u32 + 1], &node)), Err(Refused::Props));
        assert_eq!(ok(&block([0, 1, u32::MAX], &node)), Err(Refused::Props));
        // A place past the quads, a grain out of range.
        assert_eq!(ok(&block([3, 1, 1], &node)), Err(Refused::Props));
        assert_eq!(ok(&block([u32::MAX, 1, 1], &node)), Err(Refused::Props));
        assert_eq!(ok(&block([0, 0, 1], &node)), Err(Refused::Props));
        assert_eq!(ok(&block([0, u32::from(MAX_GRAIN) + 1, 1], &node)), Err(Refused::Props));
        // A mesh that is none, a node blended two ways. (19 is past every
        // mesh kind, the first free number on the wire.)
        let mut bad = node.clone();
        bad[4..8].copy_from_slice(&19u32.to_le_bytes());
        assert_eq!(ok(&block([0, 1, 1], &bad)), Err(Refused::Props));
        let mut bad = node.clone();
        let flags = u32::from_le_bytes(bad[..4].try_into().unwrap()) | BLENDED | PREMULTIPLIED;
        bad[..4].copy_from_slice(&flags.to_le_bytes());
        assert_eq!(ok(&block([0, 1, 1], &bad)), Err(Refused::Props));
        // Bytes past the last node are not read; an STG2 frame's extra block is not one.
        let mut long = block([0, 1, 1], &node);
        long.extend_from_slice(&[9; 5]);
        assert_eq!(ok(&long), Ok(Some((0, 1, 1))));
        let mut stg2 = block([0, 1, 1], &node);
        stg2[..4].copy_from_slice(TAG);
        assert_eq!(ok(&stg2), Ok(None));
    }

    #[test]
    fn mutated_props_never_panic_and_what_parses_is_sound() {
        let nodes: Vec<Node> = (1..=6).map(|i| prop(i, (i > 1).then_some(i - 1), Some(MeshRef::Sphere { segments: 8 }))).collect();
        let mut valid = Vec::new();
        encode(&with_props(Props { at: 2, grain: 2, nodes, world: None }), &(0..3).map(quad).collect::<Vec<_>>(), &mut valid);
        let mut state = 0x9e37_79b9_u32;
        let mut rnd = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            state as f64 / 4_294_967_296.0
        };
        let (mut parsed, mut refused) = (0, 0);
        for _ in 0..4000 {
            let mut buf = valid.clone();
            for _ in 0..1 + (rnd() * 8.0) as usize {
                let at = (rnd() * buf.len() as f64) as usize;
                buf[at] = (rnd() * 256.0) as u8;
            }
            let len = if rnd() < 0.25 { (rnd() * buf.len() as f64) as usize } else { buf.len() };
            match decode(&buf[..len]) {
                Ok(d) => {
                    parsed += 1;
                    if let Some(p) = d.frame.props {
                        assert!(p.at <= d.quads.len() && (1..=MAX_GRAIN).contains(&p.grain));
                        assert!(p.nodes.len() <= MAX_NODES);
                        // Everything it handed on was in the buffer.
                        assert!(p.nodes.len() * NODE_BYTES + PROPS_HEADER_BYTES <= buf.len());
                    }
                }
                Err(_) => refused += 1,
            }
        }
        assert!(parsed > 200 && refused > 200, "parsed {parsed}, refused {refused}");
    }

    #[test]
    fn props_keep_their_place_among_the_layers_through_the_wire() {
        use crate::{Layer, LayerKind, Particles, Placement, expand, expand_at, layers_of};
        let solid = |id: u64| Layer::new(id, 0, Placement::Pixels([0.0, 0.0, 4.0, 4.0]), LayerKind::Solid([1.0; 4]));
        let motes = Layer::new(
            9,
            0,
            Placement::Pixels([0.0, 0.0, 8.0, 8.0]),
            LayerKind::Particles(Particles { color: [1.0; 4], count: 4, size: 1.0, life: 2.0, rise: 0.0, sway: 0.0, seed: 1, burst: None }),
        );
        // The props go after the first two layers, the second of which is a handful of quads.
        let layers = [solid(1), motes, solid(3), solid(4)];
        let (quads, under) = expand_at(&layers, 2, 0.5, |_| None);
        assert!(under > 2 && under < quads.len());
        let frame = with_props(Props { at: under, grain: 2, nodes: vec![prop(1, None, Some(MeshRef::Gem))], world: None });
        let mut out = Vec::new();
        encode(&frame, &quads, &mut out);
        let back = decode(&out).unwrap();
        let at = back.frame.props.expect("props").at;
        assert_eq!(at, under);
        // Quads come back as layers one for one, so `at` is a layer index again,
        // and what lies under it is what the first two layers made.
        let layers_back = layers_of(&back.quads);
        assert_eq!(layers_back.len(), quads.len());
        assert_eq!(layers_of(&expand(&layers[..2], 0.5, |_| None)).len(), at);
        assert_eq!(layers_back[..at].iter().map(|l| l.placement.clone()).collect::<Vec<_>>(), layers_of(&expand(&layers[..2], 0.5, |_| None)).iter().map(|l| l.placement.clone()).collect::<Vec<_>>());
    }
}
