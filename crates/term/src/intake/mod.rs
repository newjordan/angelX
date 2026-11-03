//! The picture intake: what a program's output says beyond text.
//!
//! A streaming [`Splitter`] sits in front of the VT engine. Ordinary bytes
//! pass through untouched; media sequences are lifted out whole, keeping
//! their order relative to the text around them: APC sequences whose
//! payload starts with `G`, DCS sixel (a DCS whose final byte is `q` with
//! no intermediates, so `DCS $ q` and `DCS + q` pass through), OSC
//! `1337;File=` and the multipart form (`MultipartFile`, `FilePart`,
//! `FileEnd`), and a multiplexer's passthrough (`ESC P tmux ;` … `ESC \`,
//! every ESC in the body doubled) which is unwrapped and fed back through
//! this splitter. Everything else — other APC, DCS and OSC included —
//! reaches the engine exactly as written.
//!
//! Adapters ([`apc_graphics`], [`sixel`], [`osc1337`]) turn each dialect's
//! sequences into one shared description: [`Picture`]s and [`Placement`]s.
//! They never touch the window; they reach the terminal only through
//! [`Engine`] and [`Canvas`]. The store keeps pictures attached to the text
//! they were placed with, bounds memory by a budget of RGBA bytes (the
//! oldest pictures go first), and hands a frame's [`Description`] to the
//! session, which applies it to the host — so tests run without a window.

pub mod apc_graphics;
pub mod glass_dialect;
pub mod glass_gltf;
pub mod glass_scene;
pub mod glass_tween;
pub mod osc1337;
pub mod placeholders;
pub mod sixel;

pub(crate) mod motion;
mod frames;

#[cfg(test)]
mod animation;
#[cfg(test)]
mod position_tests;

#[cfg(test)]
mod conformance;
#[cfg(test)]
mod glass_conformance;
#[cfg(test)]
mod gltf_tests;
#[cfg(test)]
mod tween_tests;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// What a frame is showing when the description is drawn: the absolute line
/// at the top of the view (counted from the start of output), how many rows
/// it holds, and whether it is the alternate screen.
pub struct View {
    pub top: u64,
    pub rows: u16,
    pub alternate: bool,
}

/// A rectangle of cells in absolute lines: the engine side's words for
/// "these cells changed" and "these cells were cleared".
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// Absolute lines, as placements are pinned to.
    pub lines: std::ops::Range<u64>,
    /// Columns, 0-based.
    pub columns: std::ops::Range<u16>,
}

impl Area {
    /// Whether a showing's cell rectangle meets this area. The showing
    /// spans `line..line + height` rows and `column..column + width`
    /// columns.
    fn meets(&self, line: u64, height: f32, column: f32, width: f32) -> bool {
        let bottom = line + height.max(1.0).ceil() as u64;
        let right = column + width.max(1.0);
        line < self.lines.end
            && bottom > self.lines.start
            && column < f32::from(self.columns.end.max(1))
            && right > f32::from(self.columns.start)
    }
}

/// Ids the intake mints itself — an `I` transmit, a sixel or an OSC 1337
/// picture — start here, at the top of the id space, where a client's own
/// `i` numbers never reach: the client's ids and the intake's never
/// collide and never share gpu ids.
pub const MINT_BASE: u32 = 1 << 30;

/// One picture the program sent: pixels under the id its dialect named (or
/// one minted here when the dialect has none).
/// Which dialect a picture belongs to. The store keeps every dialect's
/// pictures in one list, but the key is this and the dialect's own id, so
/// a client id can never replace another dialect's picture. Window ids are
/// minted from a disjoint range per dialect for the same reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    /// The APC graphics dialect: a client's `i=` and the ids this intake mints for it.
    Apc,
    /// glass's own dialect. The id is the window id (`ID_BASE` + the program's id).
    Glass,
    /// DCS sixel. The id is one this intake minted.
    Sixel,
    /// OSC 1337. The id is one this intake minted.
    Osc,
}

impl Space {
    fn slot(self) -> usize {
        match self {
            Space::Apc => 0,
            Space::Sixel => 1,
            Space::Osc => 2,
            Space::Glass => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub id: u32,
    /// Which dialect this id belongs to. Two dialects may use the same number.
    pub space: Space,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Picture {
    /// The same picture under another id.
    pub(crate) fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }
}

/// One picture showing at a spot in the text.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// The picture's id, as [`Picture::id`].
    pub picture: u32,
    /// The line the picture is pinned to, counted from the start of output:
    /// it moves as that text scrolls.
    pub line: u64,
    /// The column of the picture's left edge, in cells.
    pub column: f32,
    /// Its width in cells; fractions allowed. Zero means the picture's own
    /// size, worked out from its pixels.
    pub width: f32,
    /// Its height in cells; fractions allowed. Zero as `width`.
    pub height: f32,
    /// Where it draws against the text: negative is under the text.
    pub z: i32,
    /// The part of the picture to show, `[x, y, w, h]` in its pixels; the
    /// whole picture when `None`.
    pub source: Option<[f32; 4]>,
    /// The dialect's own id for this showing, when it has one (the APC
    /// graphics dialect's `p=` key): showing again with the same id moves
    /// it rather than adding another.
    pub placement_id: Option<u32>,
    /// Placed on the alternate screen: it goes when the program leaves it.
    pub alternate: bool,
    /// Drawn straight over the text, as the sixel and OSC 1337 dialects
    /// draw: text written over its cells takes it away. Placements made
    /// through the APC graphics dialect are never covered.
    pub in_cell: bool,
    /// Which dialect's picture this showing names.
    pub space: Space,
    /// A virtual placement: a prototype for placeholder cells. It draws
    /// nothing and has no place on the screen of its own.
    pub virtual_place: bool,
    /// Parent image and parent placement id, when this showing is relative.
    pub parent: Option<(u32, Option<u32>)>,
    /// Cell offset from the parent. Positive is right and down.
    pub offset_h: i32,
    pub offset_v: i32,
    /// The cell box a virtual placement fits its picture into (`c` by `r`).
    pub box_cols: u32,
    pub box_rows: u32,
}

/// Taking pictures or placings away, as the APC graphics dialect's delete
/// action asks. The "visible" forms keep to what is on the screen now;
/// the others take the scrollback too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delete {
    #[allow(dead_code)]
    Everything,
    AllVisible(bool),
    /// An image's visible showings, data kept (`d=i`).
    PictureVisible(u32),
    /// An image everywhere, data freed (`d=I`).
    PictureEverywhere(u32),
    /// One showing, by image id and placement id (`d=i,i=,p=`). The
    /// flag is the freeing form (`d=I,i=,p=`).
    PlacementOf(u32, u32, bool),
    /// One showing, by image number and placement id (`d=n,I=,p=`). The
    /// flag is the freeing form.
    #[allow(dead_code)]
    PlacementOfNumber(u32, u32, bool),
    /// Showings that cross the cursor cell (`d=c`; the flag is `C`).
    AtCursorVisible(bool),
    /// Showings that cross a cell (`d=p`, `x`/`y` 1-based). The flag is
    /// the freeing form.
    AtCell(u16, u16, bool),
    /// Showings that cross a cell at one stacking level (`d=q`). The
    /// flag is the freeing form.
    AtCellZ(u16, u16, i32, bool),
    /// Showings that cross a column (`d=x`). The flag is the freeing
    /// form.
    Column(u16, bool),
    /// Showings that cross a row (`d=y`). The flag is the freeing form.
    Row(u16, bool),
    /// Showings at a stacking level (`d=z`). The flag is the freeing
    /// form.
    Z(i32, bool),
    /// Showings of images whose id runs from `x` to `y` inclusive
    /// (`d=r`), freeing.
    IdRange(u32, u32),
    /// Frames of animations (`d=f`): the named image's when one is
    /// given, every image's when none is. The picture stays.
    Frames(Option<u32>),
}

/// What an animation is doing between frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimState {
    /// `s=1`: not advancing; the current frame stays.
    Stopped,
    /// `s=2`: advancing, but at the last frame it waits for more frames
    /// instead of looping.
    Loading,
    /// `s=3`: advancing, looping back to the first frame.
    Running,
}

/// One frame of an animation. Frame 1 is the picture as first
/// transmitted: it shares the picture's own pixels and window id until
/// a client edits it.
pub struct Frame {
    /// Straight RGBA, the frame's whole rectangle: the picture's size.
    pub rgba: Arc<Vec<u8>>,
    /// The gap before the next frame, in milliseconds. `None` when the
    /// transmit left it unset (the root frame waits nothing, every later
    /// frame the dialect's 40ms default). Negative is a gapless frame.
    pub gap: Option<i64>,
    pub width: u32,
    pub height: u32,
    /// The window's id for these pixels. Frame 1's is the picture's own
    /// until the frame is edited.
    pub gpu: u32,
    /// Whether the window has had these pixels yet.
    pub uploaded: bool,
    /// Whether these bytes are counted against the budget on their own.
    pub counted: bool,
}

/// One animation: the frames, where the showing has got to, and when
/// the next frame falls due.
pub struct Animation {
    /// Frame 1 first.
    pub frames: Vec<Frame>,
    /// The frame currently showing, 0-based.
    pub current: usize,
    pub state: AnimState,
    /// Loops still allowed after this one. `None` is forever.
    pub loops: Option<u64>,
    /// Wraps already played. Stopping resets this.
    pub looped: u64,
    /// When the current frame began, on the store's clock.
    pub since: u64,
}

/// One frame transmission (`a=f`).
pub struct FramePut {
    pub image: u32,
    /// The frame `r=` names to edit, 1-based. Absent appends a frame.
    pub edit: Option<usize>,
    /// The frame `c=` names as the canvas to start from, 1-based.
    pub base: Option<usize>,
    /// `Y=`, packed `0xAABBGGRR`.
    pub background: Option<u32>,
    /// `X=1`: replace pixels instead of blending.
    pub replace: bool,
    pub x: u32,
    pub y: u32,
    pub data_size: (u32, u32),
    pub rgba: Vec<u8>,
    /// `z=`. Absent when the key was missing or zero.
    pub gap: Option<i64>,
}

/// One animation control (`a=a`).
pub struct AnimCtrl {
    pub image: u32,
    pub state: Option<AnimState>,
    /// `v=`. `1` is forever.
    pub loops: Option<u64>,
    /// `c=`, 1-based.
    pub current: Option<usize>,
    /// `r=` of a gap change, 1-based.
    pub gap_frame: Option<usize>,
    pub gap: Option<i64>,
}

/// One composition (`a=c`).
pub struct Compose {
    pub image: u32,
    /// Source frame, 1-based (`r=`).
    pub source: usize,
    /// Destination frame, 1-based (`c=`).
    pub dest: usize,
    /// `X,Y,w,h` in the source. A zero side means the whole image.
    pub src: (u32, u32, u32, u32),
    /// `x,y` in the destination.
    pub dst: (u32, u32),
    /// `C=1`: replace instead of blending.
    pub replace: bool,
}

/// What the adapters may do while reading one media sequence: reach the
/// engine (where the cursor is), and change this store. Nothing else — the
/// window is the session's business.
pub trait Canvas {
    /// The cell size in pixels.
    fn cell(&self) -> (u32, u32);
    /// The cursor, `(row, column)`, zero-based, as the engine sees it now.
    fn cursor(&mut self) -> (u16, u16);
    /// The screen's size in cells.
    fn size(&mut self) -> (u16, u16);
    /// Whether the alternate screen is up.
    fn alternate(&mut self) -> bool;
    /// A picture id no dialect has used yet.
    fn mint(&mut self) -> u32;
    /// The last id `mint` handed out. The engine's exact-count wiring
    /// reads it.
    #[allow(dead_code)]
    fn last_minted(&self) -> Option<u32>;
    /// The newest picture with this dialect image number, if any.
    fn number(&self, number: u32) -> Option<u32>;
    /// Note that this image number now means this picture id.
    fn name_number(&mut self, number: u32, id: u32);
    /// Keep a picture under this id, replacing what the id held.
    fn store(&mut self, picture: Picture);
    /// Show a picture at a spot in the text.
    fn place(&mut self, placement: Placement);
    /// Draw a picture straight over the text at the cursor: showings
    /// whose cells it covers go first, as text drawn over them would take
    /// them. Returns the new picture's id.
    fn overlay(&mut self, picture: Picture, at: (u16, u16)) -> u32;
    /// Take pictures or showings away.
    fn delete(&mut self, delete: Delete);
    /// Whether a picture with this id is kept.
    fn has(&self, id: u32) -> bool;
    /// A kept picture's pixel size, if it is kept.
    fn kept_size(&self, id: u32) -> Option<(u32, u32)>;
    /// Bytes back to the program: the dialect's answers.
    fn reply(&mut self, bytes: Vec<u8>);
    /// Bytes back into the engine: the cursor moves a dialect asks for.
    fn feed(&mut self, bytes: &[u8]);
    /// Add one chunk of a longer transmission. While `more` holds, the
    /// answer is empty; the chunk that completes it returns the whole.
    fn chunk(&mut self, key: u32, bytes: &[u8], more: bool) -> Vec<u8>;
    /// The one transmission whose chunks are still open, if there is
    /// exactly one.
    fn open_chunk(&mut self) -> Option<u32>;
    /// Open a transmission, dropping any partial one, and hand the key
    /// its chunks arrive under.
    fn begin_chunk(&mut self) -> u32;
    /// Drop the open transmission, half-read.
    #[allow(dead_code)]
    fn abort_chunk(&mut self);
    /// Open an OSC 1337 multipart file, dropping any abandoned one.
    /// `args` is the text after `MultipartFile=`, parsed as `File=` args.
    fn multipart_open(&mut self, args: &[u8]);
    /// Append one base64 piece. The transfer is dropped if it would pass
    /// the store's budget; false means it was dropped or none was open.
    fn multipart_append(&mut self, chunk: &[u8]) -> bool;
    /// Take the open transfer's args and assembled base64.
    fn multipart_finish(&mut self) -> Option<(Vec<u8>, Vec<u8>)>;
    /// Keep the first chunk's key text with its transmission.
    fn chunk_head(&mut self, key: u32, keys: Vec<u8>);
    /// Take the first chunk's key text, when its transmission completed.
    fn take_chunk_head(&mut self, key: u32) -> Vec<u8>;
    /// Lines off the top of the screen that is up, for an adapter that
    /// anchors to the text: the engine's live count, mid-read.
    fn scrolled(&mut self) -> u64;
    /// A program picture id (an id this crate's dialect named) to the
    /// window id the store gave it, when it was kept.
    fn gpu_of(&mut self, id: u32) -> Option<u32>;
    /// The size of one of glass's own dialect's pictures, by its window id.
    /// A client id of the same number is a different picture.
    fn glass_size(&self, id: u32) -> Option<(u32, u32)>;
    /// glass's own dialect's state, for its adapter alone.
    fn expansion(&mut self) -> &mut glass_dialect::Expansion;
    /// An anonymous placement (no image id, no image number) at this cell
    /// replaces the last anonymous placement there, picture and all, so a
    /// stream of frames does not grow the store. The incoming id is noted
    /// as anonymous for the next frame. The default does nothing.
    fn supplant_anonymous(&mut self, _row: u16, _column: f32, _incoming: u32) {}
    /// Why a relative placement cannot be made, if it cannot.
    fn relative_fault(
        &self,
        _image: u32,
        _pid: Option<u32>,
        _parent_image: u32,
        _parent_pid: Option<u32>,
    ) -> Option<(&'static str, &'static str)> {
        None
    }
    /// Whether the open chunked transmission is an animation frame.
    fn open_frame_chunk(&self) -> bool {
        false
    }
    /// Put one animation frame (`a=f`).
    fn frame_put(&mut self, _frame: FramePut) -> Result<(), (String, String)> {
        Err(("ENOSYS".to_string(), "no store behind this canvas".to_string()))
    }
    /// Control an animation (`a=a`).
    fn animate(&mut self, _control: AnimCtrl) -> Result<(), (String, String)> {
        Err(("ENOSYS".to_string(), "no store behind this canvas".to_string()))
    }
    /// Compose a rectangle of one frame onto another (`a=c`).
    /// Drop a picture of glass's own dialect from the store (an asset's
    /// texture no scene shows any more).
    fn forget_glass(&mut self, _id: u32) {}
    fn compose(&mut self, _compose: Compose) -> Result<(), (String, String)> {
        Err(("ENOSYS".to_string(), "no store behind this canvas".to_string()))
    }
}

/// The VT engine as the intake needs it: where the cursor is, how big the
/// screen is, how far the text has scrolled, which screen is up, and bytes
/// fed back in. Nothing here knows which engine it is, so the engine can be
/// replaced without touching the intake.
pub trait Engine {
    /// The cursor, `(row, column)`, zero-based.
    fn cursor(&mut self) -> (u16, u16);
    /// The screen's size in cells.
    fn size(&mut self) -> (u16, u16);
    /// Lines scrolled off the top of the screen since the start of output,
    /// as exactly as the engine can say.
    fn scrolled(&mut self) -> u64;
    /// Whether the alternate screen is up.
    fn alternate(&mut self) -> bool;
    /// Feed bytes to the engine, as if the program wrote them.
    fn feed(&mut self, bytes: &[u8]);
}

/// One media sequence, lifted whole out of the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Media {
    /// An APC sequence whose payload starts with `G`: the payload, `G`
    /// included, without its `ESC _` and terminator.
    Apc(Vec<u8>),
    /// An APC sequence whose body starts `glass;`: glass's own dialect,
    /// from `glass;` on, without its `ESC _` and terminator.
    Glass(Vec<u8>),
    /// A DCS whose final byte is `q` with no intermediates (sixel): the DCS
    /// body — parameters, the `q`, the data — without `ESC P` and its
    /// terminator.
    Sixel(Vec<u8>),
    /// OSC `1337;File=`: the payload after `1337;` — so it starts with
    /// `File=` — without the terminator.
    Osc1337(Vec<u8>),
}

impl Media {
    /// The payload bytes, whichever dialect they speak.
    #[allow(dead_code)] // tests and the next engine read it directly
    pub fn payload(&self) -> &[u8] {
        match self {
            Media::Apc(bytes) | Media::Sixel(bytes) | Media::Osc1337(bytes) | Media::Glass(bytes) => {
                bytes
            }
        }
    }
}

/// One piece of a program's output: text for the engine, or a media
/// sequence lifted out for the adapters.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece<'a> {
    /// Bytes for the engine, exactly as the program wrote them.
    Text(Cow<'a, [u8]>),
    /// A media sequence the adapters take.
    Media(Media),
}

mod splitter;
pub use splitter::Splitter;
#[cfg(test)]
pub(crate) use splitter::BODY_CAP;

/// Each side of a picture, in pixels, that any dialect here will take.
pub(crate) const SIDE_CAP: u32 = 8192;

/// Standard base64, as the dialects carry their bytes.
pub(crate) fn base64(data: &[u8]) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let mut out = Vec::with_capacity(data.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    // Wrappers fold their output at 76 columns; ASCII whitespace
    // between the characters is skipped, padding and all else is not.
    for &c in data
        .iter()
        .filter(|&&c| c != b'=' && !c.is_ascii_whitespace())
    {
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Decode a still (PNG, JPEG, GIF's first frame, TIFF, WebP, BMP, ICO, TGA,
/// PNM) to RGBA, under limits: no side past the intake's cap, and no
/// allocation past the budget, so a hostile file cannot make the reader
/// allocate gigabytes before the store ever sees it.
///
/// An extension hint, when a dialect has one, is tried before guessing.
/// TGA has no magic bytes, so when guessing finds no format a valid header
/// is tried under those same limits: image type 1, 2, 3, 9, 10 or 11, a
/// pixel depth of 8, 15, 16, 24 or 32, and nonzero sides. Anything else
/// returns `None` without calling the TGA decoder.
pub(crate) fn decode_image(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    decode_image_hinted(bytes, None)
}

/// As [`decode_image`], trying `extension` (a file suffix, no dot) as the
/// format hint before guessing.
pub(crate) fn decode_image_hinted(bytes: &[u8], extension: Option<&str>) -> Option<(u32, u32, Vec<u8>)> {
    let limits = image_limits();
    if let Some(format) = extension.and_then(image::ImageFormat::from_extension)
        && (format != image::ImageFormat::Tga || valid_tga_header(bytes))
        && let Some(image) = decode_as(bytes, format, limits.clone())
    {
        return Some(image);
    }
    if let Some(format) = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.format())
    {
        return decode_as(bytes, format, limits);
    }
    if valid_tga_header(bytes) {
        return decode_as(bytes, image::ImageFormat::Tga, limits);
    }
    None
}

fn image_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(SIDE_CAP);
    limits.max_image_height = Some(SIDE_CAP);
    limits.max_alloc = Some(RGBA_BUDGET as u64);
    limits
}

fn decode_as(
    bytes: &[u8],
    format: image::ImageFormat,
    limits: image::Limits,
) -> Option<(u32, u32, Vec<u8>)> {
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    reader.limits(limits);
    let decoded = reader.decode().ok()?.into_rgba8();
    let (width, height) = decoded.dimensions();
    if width == 0 || height == 0 || width > SIDE_CAP || height > SIDE_CAP {
        return None;
    }
    Some((width, height, decoded.into_raw()))
}

/// A TGA header the intake will try. Checked before the decoder runs, so
/// random bytes guessing rejects stay `None` without a decode.
fn valid_tga_header(bytes: &[u8]) -> bool {
    let Some(header) = bytes.get(..18) else { return false };
    if !matches!(header[2], 1 | 2 | 3 | 9 | 10 | 11) {
        return false;
    }
    if !matches!(header[16], 8 | 15 | 16 | 24 | 32) {
        return false;
    }
    let width = u16::from_le_bytes([header[12], header[13]]);
    let height = u16::from_le_bytes([header[14], header[15]]);
    width != 0 && height != 0
}

/// Pictures the intake may keep at once: 256 MiB of RGBA. Past it, the
/// oldest pictures go first, and their showings with them.
pub const RGBA_BUDGET: usize = 256 << 20;

/// How many pictures the intake may keep at once. The GPU frees the
/// least-recently-drawn picture past its own cap, so keeping fewer than
/// that here means a picture still named here is still drawn there.
pub const PICTURE_CAP: usize = 4096;

/// The window keeps at most this many pictures (`PICTURE_COUNT` in
/// `crates/gpu/src/picture.rs`) and drops the least recently drawn past
/// it. A frame the store still names must not be that one.
const WINDOW_PICTURE_COUNT: usize = 8192;

/// Pictures plus frame uploads stay at or under this: 192 under the
/// window's count, so the window's cache still has room when both caps
/// are full.
const PICTURE_FRAME_ROOM: usize = 8000;

/// Most frame uploads the store will keep. [`PICTURE_CAP`] pictures plus
/// this many frames is 8000, which is the window's 8192 (`PICTURE_COUNT`
/// in `crates/gpu/src/picture.rs`) less 192. A full picture cap plus every
/// frame therefore stays under the window's count, so a frame this store
/// still names cannot be the one the window drops. Frame 1 shares the
/// picture's window id until `a=f` or `a=c` gives it its own pixels, so it
/// is not an extra upload and is not counted here. Every later frame is.
pub const FRAME_CAP: usize = PICTURE_FRAME_ROOM - PICTURE_CAP;

const _: () = assert!(WINDOW_PICTURE_COUNT - PICTURE_FRAME_ROOM == 192);
const _: () = assert!(PICTURE_CAP + FRAME_CAP == PICTURE_FRAME_ROOM);
struct Kept {
    /// The id its dialect speaks for it.
    dialect: u32,
    /// Which dialect owns this picture. A client id never names another dialect's picture.
    space: Space,
    /// The id the window knows it by.
    gpu: u32,
    width: u32,
    height: u32,
    rgba: Arc<Vec<u8>>,
    /// Pixels changed since the window last had them.
    dirty: bool,
    /// When it arrived: the oldest go first when the budget is spent.
    seq: u64,
}

/// One showing of a picture ([`Placement`], with the bookkeeping the store
/// needs).
#[derive(Clone)]
struct Put {
    seq: u64,
    placement_id: Option<u32>,
    picture: u32,
    line: u64,
    column: f32,
    width: f32,
    height: f32,
    z: i32,
    source: Option<[f32; 4]>,
    alternate: bool,
    /// Drawn straight over the text, as the sixel and OSC 1337 dialects
    /// draw: text written over its cells takes it away. Placements made
    /// through the APC graphics dialect are never covered.
    in_cell: bool,
    space: Space,
    virtual_place: bool,
    parent: Option<(u32, Option<u32>)>,
    offset_h: i32,
    offset_v: i32,
    box_cols: u32,
    box_rows: u32,
    /// Covered cells of an in-cell picture, as merged column spans per
    /// row offset from the picture's top cell.
    covered: motion::Covers,
}

impl From<&Put> for Placement {
    fn from(put: &Put) -> Self {
        Self {
            picture: put.picture,
            line: put.line,
            column: put.column,
            width: put.width,
            height: put.height,
            z: put.z,
            source: put.source,
            placement_id: put.placement_id,
            alternate: put.alternate,
            in_cell: put.in_cell,
            space: put.space,
            virtual_place: put.virtual_place,
            parent: put.parent,
            offset_h: put.offset_h,
            offset_v: put.offset_v,
            box_cols: put.box_cols,
            box_rows: put.box_rows,
        }
    }
}

/// A frame's worth of pictures: what to upload, and where to show them.
#[derive(Default)]
pub struct Description {
    /// Pictures whose pixels the window has not had yet.
    pub pictures: Vec<Upload>,
    /// The showings visible in this view.
    pub placements: Vec<Shown>,
    /// Window ids of pictures dropped since the last description: nothing
    /// shown names them any more, so the window can let them go once the
    /// placements above are up.
    pub freed: Vec<u32>,
}

/// Pixels for the window, under the id it knows.
pub struct Upload {
    pub gpu: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

/// One showing, placed against the view: row and column in cells from the
/// view's top left, size in cells.
pub struct Shown {
    /// A stable id for this showing (the layer's).
    pub id: u64,
    /// The picture's window id.
    pub gpu: u32,
    pub row: f32,
    pub column: f32,
    pub width: f32,
    pub height: f32,
    pub z: i32,
    pub source: Option<[f32; 4]>,
}

/// The store: every picture the program sent and every spot it put one.
/// What one screen's scrolling was measured to. `scrolled` counts lines
/// off that screen's top from its own start: the primary screen from the
/// start of output, the alternate from when it was entered.
#[derive(Clone, Copy, Default)]
struct Screen {
    scrolled: u64,
    /// How much history the engine still holds of this screen.
    history: usize,
}

pub struct Intake {
    pictures: Vec<Kept>,
    placements: Vec<Put>,
    /// Half-finished chunked transmissions: at most one is open, keyed
    /// by a token the store mints, never by a dialect's image id.
    chunks: HashMap<u32, Vec<u8>>,
    /// The first chunk's key text, for the last chunk to decode with.
    chunk_heads: HashMap<u32, Vec<u8>>,
    /// The token the open transmission's chunks arrive under.
    chunk_key: u32,
    /// An open OSC 1337 multipart file: the args and the base64 so far.
    /// Dropped by a new `MultipartFile` or when the pieces pass the budget.
    multipart: Option<(Vec<u8>, Vec<u8>)>,
    replies: Vec<u8>,
    cell: (u32, u32),
    /// Which screen was up at the last read, so the reader can notice the
    /// program leaving the alternate screen.
    alternate_was: bool,
    /// Lines off the top of each screen, and how much history the engine
    /// still holds of the primary one, as the reader last measured them.
    /// The two screens are kept apart: a measurement taken while the
    /// alternate screen is up says nothing about the primary's text.
    primary: Screen,
    alternate_screen: Screen,
    rows: u16,
    next_id: u32,
    /// The last id `mint` handed out, so a chunked transmit with no id
    /// keys its pieces together.
    last_minted: Option<u32>,
    /// Image numbers (`I`) to the newest picture id minted for them.
    numbers: HashMap<u32, u32>,
    /// Pictures placed with no image id and no image number, so a later
    /// anonymous put at the same cell can replace the last one.
    anonymous: HashSet<u32>,
    gpu_base: u32,
    /// How many window ids each dialect has minted, in disjoint ranges.
    gpu_n: [u32; 4],
    seq: u64,
    bytes: usize,
    budget: usize,
    /// Whether the delete in flight frees data, as its letter said.
    pending_free: Option<bool>,
    /// glass's own dialect's state: the layers, groups, scenes and
    /// showcase the program asked for, beside the pictures above.
    pub expansion: glass_dialect::Expansion,
    /// Animations of APC pictures, keyed by the dialect's image id.
    animated: HashMap<u32, Animation>,
    /// Frame uploads that have their own window id. Shared frame 1 is the
    /// picture's upload and is not counted. Pictures plus this stay at or
    /// under 8000; see [`FRAME_CAP`].
    frame_uploads: usize,
    /// Milliseconds the animation clock has reached.
    clock: u64,
    /// Window ids of dropped pictures, for the next description. glass's
    /// own dialect names its pictures from layer records that outlive the
    /// store's copy, so its pictures are not listed.
    freed: Vec<u32>,
}

/// What a delete leaves: the pictures whose data may go, and a
/// predicate naming the showings that stay.
type DeleteOutcome = (Vec<u32>, Box<dyn Fn(&Put) -> bool>);

/// A showing's key: dialect, picture, and the dialect's placement id.
/// A relative showing names its parent by this key.
type ShowingKey = (Space, u32, Option<u32>);

/// Write a scrolled or cropped rectangle back onto a showing. Covered
/// spans stay with the picture; a cut from the top shifts them.
fn apply_moved(put: &mut Put, rect: &motion::Rect) {
    let cut_top = if rect.height + 0.01 < put.height && rect.line >= put.line {
        (rect.line - put.line) as u16
    } else {
        0
    };
    if cut_top > 0 || rect.height + 0.01 < put.height {
        put.covered.crop_rows(cut_top, rect.height.ceil().max(0.0) as u16);
    }
    put.line = rect.line;
    put.column = rect.column;
    put.width = rect.width;
    put.height = rect.height;
    put.source = rect.source;
}

fn part_source(
    source: Option<[f32; 4]>,
    image: (u32, u32),
    part: (u16, u16, u16, u16),
    total: (u16, u16),
) -> Option<[f32; 4]> {
    let [sx, sy, sw, sh] = source.unwrap_or([0.0, 0.0, image.0 as f32, image.1 as f32]);
    let (col, cols, row, rows) = part;
    let (tw, th) = (f32::from(total.0.max(1)), f32::from(total.1.max(1)));
    Some([
        sx + sw * f32::from(col) / tw,
        sy + sh * f32::from(row) / th,
        sw * f32::from(cols.max(1)) / tw,
        sh * f32::from(rows.max(1)) / th,
    ])
}

fn showing_key(p: &Put) -> ShowingKey {
    (p.space, p.picture, p.placement_id)
}

/// True when any showing is relative. An index walk, so a full pass over
/// a capped store stays cheap in an unoptimized build.
fn showing_has_parent(placements: &[Put]) -> bool {
    let n = placements.len();
    let mut i = 0;
    while i + 4 <= n {
        if placements[i].parent.is_some()
            || placements[i + 1].parent.is_some()
            || placements[i + 2].parent.is_some()
            || placements[i + 3].parent.is_some()
        {
            return true;
        }
        i += 4;
    }
    while i < n {
        if placements[i].parent.is_some() {
            return true;
        }
        i += 1;
    }
    false
}

/// Index of the oldest showing. Seq rises with insertion, so this is the
/// first unless a seq was rewritten.
fn oldest_showing(placements: &[Put]) -> usize {
    let mut oldest_at = 0;
    let mut oldest_seq = placements[0].seq;
    let mut i = 1;
    let n = placements.len();
    while i < n {
        if placements[i].seq < oldest_seq {
            oldest_seq = placements[i].seq;
            oldest_at = i;
        }
        i += 1;
    }
    oldest_at
}

impl Intake {
    /// An empty store, minting window ids from `gpu_base` up.
    pub fn new(gpu_base: u32) -> Self {
        Self::with_budget(gpu_base, RGBA_BUDGET)
    }

    /// The same, with a smaller budget (for tests).
    pub fn with_budget(gpu_base: u32, budget: usize) -> Self {
        Self {
            pictures: Vec::new(),
            placements: Vec::new(),
            chunks: HashMap::new(),
            chunk_heads: HashMap::new(),
            chunk_key: 0,
            multipart: None,
            replies: Vec::new(),
            cell: (1, 1),
            alternate_was: false,
            primary: Screen::default(),
            alternate_screen: Screen::default(),
            rows: 0,
            next_id: MINT_BASE,
            last_minted: None,
            numbers: HashMap::new(),
            anonymous: HashSet::new(),
            gpu_base,
            gpu_n: [0; 4],
            seq: 0,
            bytes: 0,
            budget: budget.max(1),
            pending_free: None,
            expansion: glass_dialect::Expansion::new(),
            animated: HashMap::new(),
            frame_uploads: 0,
            clock: 0,
            freed: Vec::new(),
        }
    }

    /// The cell size in pixels, for turning pixel sizes into cells.
    pub fn set_cell(&mut self, cell: (u32, u32)) {
        self.cell = (cell.0.max(1), cell.1.max(1));
    }

    /// A measurement of the primary screen, whichever screen is up.
    pub fn set_scrolled_primary(&mut self, scrolled: u64, history: usize, rows: u16) {
        self.primary = Screen { scrolled, history };
        self.rows = rows;
    }

    /// A measurement of the alternate screen, from its own line 0.
    #[allow(dead_code)]
    pub fn set_scrolled_alternate(&mut self, scrolled: u64, rows: u16) {
        self.alternate_screen = Screen { scrolled, history: 0 };
        self.rows = rows;
    }

    /// The alternate screen's own scrolled count, from 0 each time it is
    /// entered: the engine merge calls this when the program enters it.
    #[allow(dead_code)]
    pub fn enter_alternate(&mut self) {
        self.alternate_screen = Screen::default();
    }

    /// Lines off the top of the screen so far, whichever was measured
    /// last.
    #[allow(dead_code)] // the engine swap reads these beside the reader
    pub fn scrolled(&self) -> u64 {
        if self.alternate_was {
            self.alternate_screen.scrolled
        } else {
            self.primary.scrolled
        }
    }

    /// How much history the engine still holds of the primary screen.
    #[allow(dead_code)] // the engine swap reads these beside the reader
    pub fn history(&self) -> usize {
        self.primary.history
    }

    /// The one place a screen row becomes an absolute line. Every
    /// adapter, the overlay path and the deletes go through here, so the
    /// two screens can never drift apart in how they count. The alternate
    /// screen's line 0 is the absolute line its own `scrolled` says, so a
    /// screen hop moves nothing that was pinned before it.
    fn anchor(&self, alternate: bool, row: u16) -> u64 {
        self.anchor_with(alternate, row, if alternate { self.alternate_screen.scrolled } else { self.primary.scrolled })
    }

    /// The same, when the caller knows the count to anchor by — an
    /// adapter's, measured live mid-read, or the store's own.
    fn anchor_with(&self, alternate: bool, row: u16, scrolled: u64) -> u64 {
        let _ = alternate;
        scrolled.saturating_add(u64::from(row))
    }

    /// Hand one lifted media sequence to its adapter.
    pub fn handle(&mut self, media: Media, engine: &mut dyn Engine) {
        let mut canvas = CanvasImpl { intake: self, engine };
        match media {
            Media::Apc(payload) => apc_graphics::handle(&payload, &mut canvas),
            Media::Sixel(body) => sixel::handle(&body, &mut canvas),
            Media::Osc1337(payload) => osc1337::handle(&payload, &mut canvas),
            Media::Glass(body) => glass_dialect::handle(&body, &mut canvas),
        }
    }

    /// The answers waiting for the program, and the pty bytes they ride in.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    /// Showings placed on the alternate screen go when the program leaves
    /// it; the pictures themselves stay until their budget runs out.
    pub fn drop_alternate_placements(&mut self) {
        let removed = self.retain_showings(|p| !p.alternate);
        self.drop_parentless_if_removed(removed);
    }

    /// A picture drawn straight over the text: showings whose cells it
    /// covers go first, as text drawn over them would take them. Returns
    /// the new picture's id.
    #[allow(unused_variables)]
    pub fn overlay(&mut self, picture: Picture, at: (u16, u16), scrolled: u64, alternate: bool) -> u32 {
        let id = self.next_id.saturating_add(1).max(MINT_BASE);
        self.next_id = id;
        self.last_minted = Some(id);
        let line = self.anchor(alternate, at.0);
        let column = f32::from(at.1);
        // Sizes here are in cells, from the picture's pixels and the
        // cell's pixels.
        let (cw, ch) = (self.cell.0.max(1) as f32, self.cell.1.max(1) as f32);
        let (w, h) = (picture.width as f32 / cw, picture.height as f32 / ch);
        // The sixel and inline dialects have no picture ids of their own;
        // a new one covers whatever sat on those cells — on the screen it
        // draws on: what the other screen holds stays.
        let (top, bottom) = (line, line + h.max(1.0).ceil() as u64);
        let (left, right) = (column, column + w.max(1.0));
        let removed = self.retain_showings(|p| {
            let overlaps = p.alternate == alternate
                && p.line + p.height.max(1.0).ceil() as u64 > top
                && p.line < bottom
                && p.column + p.width.max(1.0) > left
                && p.column < right;
            !overlaps
        });
        self.drop_parentless_if_removed(removed);
        let space = picture.space;
        self.store(picture.with_id(id));
        self.place(Placement {
            picture: id,
            line,
            column,
            width: w,
            height: h,
            z: 0,
            source: None,
            placement_id: None,
            alternate,
            // Overlaid straight over the text, as the sixel and inline
            // dialects draw: text over its cells takes it away.
            in_cell: true,
            space,
            virtual_place: false,
            parent: None,
            offset_h: 0,
            offset_v: 0,
            box_cols: 0,
            box_rows: 0,
        });
        id
    }

    /// Text has covered cells of an in-cell picture. Only those cells
    /// hide; the rest of the picture still shows. A picture with every
    /// cell covered goes.
    #[allow(dead_code)]
    pub fn covered(&mut self, alternate: bool, area: Area) {
        let mut gone = Vec::new();
        for put in &mut self.placements {
            if !(put.in_cell && put.alternate == alternate) {
                continue;
            }
            let rows = put.height.max(1.0).ceil() as u16;
            let left = put.column.floor().max(0.0) as u16;
            let right = (put.column + put.width.max(1.0)).ceil().max(0.0) as u16;
            let cols = right.saturating_sub(left);
            if rows == 0 || cols == 0 {
                continue;
            }
            let bottom = put.line + u64::from(rows);
            if !area.meets(put.line, f32::from(rows), f32::from(left), f32::from(cols)) {
                continue;
            }
            let row0 = area.lines.start.saturating_sub(put.line) as u16;
            let row1 = (area.lines.end.min(bottom) - put.line) as u16;
            let col0 = area.columns.start.saturating_sub(left);
            let col1 = area.columns.end.saturating_sub(left).min(cols);
            if col0 >= col1 {
                continue;
            }
            for row in row0..row1 {
                put.covered.add(row, col0, col1);
            }
            if put.covered.full(rows, cols) {
                gone.push(showing_key(put));
            }
        }
        if gone.is_empty() {
            return;
        }
        let removed = self.retain_showings(|p| !gone.contains(&showing_key(p)));
        self.drop_parentless_if_removed(removed);
    }

    /// Text inside a scroll region moved. Anchored showings on that
    /// screen move with it; one that leaves the region is cut at the
    /// margin. Virtual and relative showings are not touched.
    pub fn region_scroll(&mut self, band: motion::BandScroll) {
        self.scroll_placements(&band);
        let glass_px: Vec<(u32, u32, u32)> = self
            .pictures
            .iter()
            .filter(|p| p.space == Space::Glass)
            .map(|p| (p.dialect, p.width, p.height))
            .collect();
        self.expansion.scroll_text(&band, |id| {
            glass_px
                .iter()
                .find(|p| p.0 == id)
                .map(|p| (p.1, p.2))
                .unwrap_or((0, 0))
        });
    }

    /// A width change reflowed the primary grid. Anchored showings stay
    /// on the same text, at the same character offset.
    pub fn reflow(&mut self, reflow: &motion::Reflow) {
        self.reflow_placements(reflow);
        self.expansion.reflow_text(reflow);
    }

    fn scroll_placements(&mut self, band: &motion::BandScroll) {
        let images: Vec<(Space, u32, u32, u32)> = self
            .pictures
            .iter()
            .map(|p| (p.space, p.dialect, p.width, p.height))
            .collect();
        let px = |space, id| {
            images
                .iter()
                .find(|p| p.0 == space && p.1 == id)
                .map(|p| (p.2, p.3))
                .unwrap_or((0, 0))
        };
        let mut gone = Vec::new();
        let mut extra: Vec<(usize, Put)> = Vec::new();
        for (index, put) in self.placements.iter_mut().enumerate() {
            if put.virtual_place || put.parent.is_some() || put.alternate != band.alternate {
                continue;
            }
            let image = px(put.space, put.picture);
            let rect = motion::Rect {
                line: put.line,
                column: put.column,
                width: put.width,
                height: put.height,
                source: put.source,
            };
            match motion::scroll_rect(rect, band, image) {
                motion::Fate::Keep(next) => apply_moved(put, &next),
                motion::Fate::Split(first, rest) => {
                    apply_moved(put, &first);
                    for (n, piece) in rest.into_iter().enumerate() {
                        let mut born = put.clone();
                        born.seq = born.seq.wrapping_add((n as u64 + 1).wrapping_mul(1 << 20));
                        born.placement_id = None;
                        born.parent = None;
                        apply_moved(&mut born, &piece);
                        extra.push((index, born));
                    }
                }
                motion::Fate::Gone => gone.push(showing_key(put)),
            }
        }
        for (index, born) in extra.into_iter().rev() {
            let at = (index + 1).min(self.placements.len());
            self.placements.insert(at, born);
        }
        if !gone.is_empty() {
            let removed = self.retain_showings(|p| !gone.contains(&showing_key(p)));
            self.drop_parentless_if_removed(removed);
        }
    }

    fn reflow_placements(&mut self, reflow: &motion::Reflow) {
        let mut gone = Vec::new();
        for put in &mut self.placements {
            if put.virtual_place || put.parent.is_some() || put.alternate {
                continue;
            }
            match motion::map_anchor(reflow, put.line, put.column) {
                Some((line, column)) => {
                    put.line = line;
                    put.column = column;
                }
                None => gone.push(showing_key(put)),
            }
        }
        if !gone.is_empty() {
            let removed = self.retain_showings(|p| !gone.contains(&showing_key(p)));
            self.drop_parentless_if_removed(removed);
        }
    }

    /// Exclusive absolute line past the lowest picture or text layer a
    /// scroll of this screen could move, or `None` when nothing is
    /// anchored there. Virtual and relative placements are not moved by
    /// a region scroll.
    pub fn motion_end(&self, alternate: bool) -> Option<u64> {
        let mut end = None;
        for p in &self.placements {
            if p.virtual_place || p.parent.is_some() || p.alternate != alternate {
                continue;
            }
            let bottom = p.line.saturating_add(p.height.max(1.0).ceil() as u64);
            end = Some(end.unwrap_or(0).max(bottom));
        }
        if let Some(layer_end) = self.expansion.anchored_end(alternate) {
            end = Some(end.unwrap_or(0).max(layer_end));
        }
        end
    }

    /// That screen's in-cell pictures' cell rectangles, so the engine side
    /// checks cell writes only when there are some.
    #[allow(dead_code)]
    pub fn in_cell_areas(&self, alternate: bool) -> Vec<Area> {
        self.placements
            .iter()
            .filter(|p| p.in_cell && p.alternate == alternate)
            .map(|p| {
                let bottom = p.line + p.height.max(1.0).ceil() as u64;
                let left = p.column.floor().max(0.0);
                let right = (p.column + p.width.max(1.0)).ceil();
                Area {
                    lines: p.line..bottom,
                    columns: left.max(0.0) as u16..right.min(f32::from(u16::MAX)).max(0.0) as u16,
                }
            })
            .collect()
    }

    /// `ED 2` on a screen: that screen's showings on the screen rows go,
    /// from every dialect; its scrollback's stay. On the alternate screen
    /// all its showings go, since it holds no scrollback of its own.
    #[allow(dead_code)]
    pub fn clear_screen(&mut self, alternate: bool, scrolled: u64) {
        let removed = self.retain_showings(|p| {
            if p.virtual_place || p.parent.is_some() {
                return true;
            }
            if p.alternate != alternate {
                return true;
            }
            if alternate {
                return false;
            }
            p.line < scrolled
        });
        self.drop_parentless_if_removed(removed);
    }

    /// `ED 3`: the primary screen's showings above the screen go.
    #[allow(dead_code)]
    pub fn clear_saved_lines(&mut self, scrolled: u64) {
        let removed =
            self.retain_showings(|p| p.virtual_place || p.parent.is_some() || p.alternate || p.line >= scrolled);
        self.drop_parentless_if_removed(removed);
    }

    /// `RIS`: everything goes, pictures too.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        let freed = self.pictures.iter().filter(|k| k.space != Space::Glass).map(|k| k.gpu);
        self.freed.extend(freed.collect::<Vec<_>>());
        self.pictures.clear();
        self.placements.clear();
        self.chunks.clear();
        self.chunk_heads.clear();
        self.chunk_key = 0;
        self.numbers.clear();
        self.anonymous.clear();
        self.bytes = 0;
        // RIS frees every frame upload, so the count cap has its room back.
        self.animated.clear();
        self.frame_uploads = 0;
    }

    /// Whether the alternate screen was up at the last read, so the reader
    /// can notice the program leaving it.
    pub fn was_alternate(&self) -> bool {
        self.alternate_was
    }

    /// Record which screen is up now.
    pub fn set_alternate(&mut self, now: bool) {
        self.alternate_was = now;
    }

    /// A cleared screen takes its pictures with it: `ESC [ 2 J` drops what
    /// is on the screen, `ESC [ 3 J` the saved lines too. Best effort — a
    /// clear split across reads is not seen.
    #[allow(dead_code)] // the engine swap calls this from its own ED hook
    pub fn clear(&mut self, saved_lines_too: bool, scrolled: u64) {
        let removed = if saved_lines_too {
            self.retain_showings(|p| p.alternate)
        } else {
            self.retain_showings(|p| p.alternate || p.line < scrolled)
        };
        self.drop_parentless_if_removed(removed);
    }

    fn mint_gpu(&mut self, space: Space) -> u32 {
        let slot = space.slot();
        // Disjoint ranges above the picture base, so one dialect's window
        // id can never be another's even when the client ids match.
        let id = self.gpu_base.saturating_add((slot as u32) << 26).saturating_add(self.gpu_n[slot]);
        self.gpu_n[slot] = self.gpu_n[slot].saturating_add(1);
        id
    }

    fn store(&mut self, picture: Picture) {
        let bytes = picture.rgba.len();
        let seq = self.bump();
        let space = picture.space;
        let id = picture.id;
        let same = |k: &Kept| k.space == space && k.dialect == id;
        let mut removed = Vec::new();
        if let Some(at) = self.pictures.iter().position(same) {
            // Re-transmitting an id replaces its data and takes away its
            // showings: the new data shows only when placed again. Another
            // dialect's picture of the same number is not this one.
            removed = self.retain_showings(|p| !(p.space == space && p.picture == id));
            if space == Space::Apc {
                self.drop_animation(id);
                self.numbers.retain(|_, n| *n != id);
            }
            self.bytes = self.bytes - self.pictures[at].rgba.len() + bytes;
            let kept = &mut self.pictures[at];
            kept.rgba = Arc::new(picture.rgba);
            kept.width = picture.width;
            kept.height = picture.height;
            kept.dirty = true;
            kept.seq = seq;
        } else {
            self.bytes += bytes;
            let gpu = self.mint_gpu(space);
            self.seq += 1;
            self.pictures.push(Kept {
                dialect: id,
                space,
                gpu,
                width: picture.width,
                height: picture.height,
                rgba: Arc::new(picture.rgba),
                dirty: true,
                seq: self.seq,
            });
        }
        removed.extend(self.evict());
        self.drop_parentless_if_removed(removed);
    }

    fn place(&mut self, placement: Placement) {
        if let Some(pid) = placement.placement_id
            && let Some(put) = self.placements.iter_mut().find(|p| {
                p.space == placement.space && p.picture == placement.picture && p.placement_id == Some(pid)
            })
        {
            put.picture = placement.picture;
            put.line = placement.line;
            put.column = placement.column;
            put.width = placement.width;
            put.height = placement.height;
            put.z = placement.z;
            put.source = placement.source;
            put.alternate = placement.alternate;
            put.in_cell = placement.in_cell;
            put.space = placement.space;
            put.virtual_place = placement.virtual_place;
            put.parent = placement.parent;
            put.offset_h = placement.offset_h;
            put.offset_v = placement.offset_v;
            put.box_cols = placement.box_cols;
            put.box_rows = placement.box_rows;
            return;
        }
        self.seq += 1;
        self.placements.push(Put {
            seq: self.seq,
            placement_id: placement.placement_id,
            picture: placement.picture,
            line: placement.line,
            column: placement.column,
            width: placement.width,
            height: placement.height,
            z: placement.z,
            source: placement.source,
            alternate: placement.alternate,
            in_cell: placement.in_cell,
            space: placement.space,
            virtual_place: placement.virtual_place,
            parent: placement.parent,
            offset_h: placement.offset_h,
            offset_v: placement.offset_v,
            box_cols: placement.box_cols,
            box_rows: placement.box_rows,
            covered: motion::Covers::default(),
        });
        self.cap_placements();
    }

    /// Showings are capped: the oldest past the cap go, the same budget
    /// logic the pictures follow. The intake's ids (the APC graphics
    /// dialect's `p=` and its placements) keep working, since a client
    /// asking again for a placement id re-places it.
    fn cap_placements(&mut self) {
        const PLACEMENT_CAP: usize = 8192;
        if self.placements.len() <= PLACEMENT_CAP {
            return;
        }
        // No relative showing: a removed placement cannot orphan a child.
        // The scan allocates nothing. Showings are appended with an
        // increasing seq and removals keep that order, so the oldest is
        // the first unless a later seq was rewritten below the first.
        if !showing_has_parent(&self.placements) {
            while self.placements.len() > PLACEMENT_CAP {
                let at = if self.placements[0].seq <= self.placements[self.placements.len() - 1].seq {
                    0
                } else {
                    oldest_showing(&self.placements)
                };
                self.placements.remove(at);
            }
            return;
        }
        let mut removed = Vec::new();
        while self.placements.len() > PLACEMENT_CAP {
            let at = oldest_showing(&self.placements);
            removed.push(showing_key(&self.placements[at]));
            self.placements.remove(at);
        }
        self.drop_parentless_if_removed(removed);
    }

    fn delete(&mut self, delete: Delete, cursor: (u16, u16), alternate: bool, rows: u16) {
        // A delete while a transmission is half-read aborts it.
        self.chunks.clear();
        self.chunk_heads.clear();
        let scrolled = if alternate { self.alternate_screen.scrolled } else { self.primary.scrolled };
        let visible = move |p: &Put| p.alternate == alternate && (scrolled..scrolled + u64::from(rows)).contains(&p.line);
        let cursor_line = self.anchor(alternate, cursor.0);
        let (ccol, crow) = (f32::from(cursor.1), cursor_line);
        let hits_cursor = move |p: &Put| {
            p.alternate == alternate && p.line == crow && p.column <= ccol && p.column + p.width.max(1.0) > ccol
        };
        // Does a showing cross this cell? The dialect's `x`/`y` are
        // 1-based cells, already made 0-based by the adapter.
        let crosses = move |p: &Put, x: u16, y: u16| {
            let xf = f32::from(x);
            let yl = scrolled + u64::from(y);
            p.alternate == alternate
                && p.column <= xf
                && p.column + p.width.max(1.0) > xf
                && p.line <= yl
                && p.line + p.height.max(1.0).ceil() as u64 > yl
        };
        /// What a delete does to the showings: which pictures' data goes
        /// (the freeing forms, once nothing shows them), and which
        /// showings stay.
        type Keep = Box<dyn Fn(&Put) -> bool>;
        let (free_ids, keep): (Vec<u32>, Keep) = match delete {
            Delete::Everything => (
                self.pictures.iter().filter(|k| k.space == Space::Apc).map(|k| k.dialect).collect(),
                Box::new(|_| false),
            ),
            Delete::AllVisible(free) => {
                self.pending_free = Some(free);
                (Vec::new(), Box::new(move |p: &Put| !visible(p)) as Keep)
            }
            Delete::PictureVisible(id) => (Vec::new(), Box::new(move |p| p.picture != id || !visible(p))),
            Delete::PictureEverywhere(id) => {
                self.pending_free = Some(true);
                (vec![id], Box::new(move |p| p.picture != id))
            }
            Delete::PlacementOf(id, pid, free) => {
                if free {
                    (vec![id], Box::new(move |p: &Put| p.picture != id || p.placement_id != Some(pid)) as Keep)
                } else {
                    (Vec::new(), Box::new(move |p: &Put| p.picture != id || p.placement_id != Some(pid)) as Keep)
                }
            }
            Delete::PlacementOfNumber(number, pid, free) => {
                // A number means the newest picture minted for it.
                match self.numbers.get(&number).copied() {
                    Some(id) => {
                        if free {
                            (vec![id], Box::new(move |p: &Put| p.picture != id || p.placement_id != Some(pid)) as Keep)
                        } else {
                            (Vec::new(), Box::new(move |p: &Put| p.picture != id || p.placement_id != Some(pid)) as Keep)
                        }
                    }
                    None => (Vec::new(), Box::new(|_| true)),
                }
            }
            Delete::AtCursorVisible(free) => {
                self.pending_free = Some(free);
                (Vec::new(), Box::new(move |p: &Put| !hits_cursor(p) || !visible(p)) as Keep)
            }
            Delete::AtCell(x, y, free) => self.geometric(free, move |p| crosses(p, x, y)),
            Delete::AtCellZ(x, y, z, free) => self.geometric(free, move |p| p.z == z && crosses(p, x, y)),
            Delete::Column(x, free) => {
                let xf = f32::from(x);
                self.geometric(free, move |p| p.column <= xf && p.column + p.width.max(1.0) > xf)
            }
            Delete::Row(y, free) => {
                let yl = scrolled + u64::from(y);
                self.geometric(free, move |p| p.line <= yl && p.line + p.height.max(1.0).ceil() as u64 > yl)
            }
            Delete::Z(z, free) => self.geometric(free, move |p| p.z == z),
            Delete::IdRange(lo, hi) => {
                // `d=r`/`d=R` takes the images in the id range, their
                // showings with them.
                self.pending_free = Some(true);
                let ids: Vec<u32> = self
                    .pictures
                    .iter()
                    .filter(|k| k.space == Space::Apc)
                    .map(|k| k.dialect)
                    .filter(|id| u64::from(*id) >= u64::from(lo) && u64::from(*id) <= u64::from(hi))
                    .collect();
                let keep_ids = ids.clone();
                let keep = Box::new(move |p: &Put| !keep_ids.contains(&p.picture)) as Keep;
                (ids, keep)
            }
            // No animation frames are kept, so nothing goes.
            Delete::Frames(image) => {
                self.free_frames(image);
                (Vec::new(), Box::new(|_| true))
            }
        };
        let free = self.pending_free.take().unwrap_or(false);
        // A delete from the APC graphics dialect never touches another
        // dialect's pictures. Virtual placements have no screen place, so
        // only the deletes that name an image or an id range take them.
        let touches_virtual = matches!(
            delete,
            Delete::Everything
                | Delete::PictureVisible(_)
                | Delete::PictureEverywhere(_)
                | Delete::PlacementOf(_, _, _)
                | Delete::PlacementOfNumber(_, _, _)
                | Delete::IdRange(_, _)
        );
        let removed = self.retain_showings(|p| {
            if p.space != Space::Apc {
                return true;
            }
            if p.virtual_place && !touches_virtual {
                return true;
            }
            keep(p)
        });
        self.drop_parentless_if_removed(removed);
        // The freeing forms drop the picture once nothing shows it.
        let mut orphans: Vec<u32> = self
            .pictures
            .iter()
            .filter(|k| k.space == Space::Apc)
            .map(|k| k.dialect)
            .filter(|id| !self.placements.iter().any(|p| p.space == Space::Apc && p.picture == *id))
            .collect();
        for id in free_ids {
            if !self.placements.iter().any(|p| p.space == Space::Apc && p.picture == id) && !orphans.contains(&id) {
                orphans.push(id);
            }
        }
        if free {
            for id in orphans {
                self.drop_picture(Space::Apc, id);
            }
        }
    }


    /// The dialect's answer when a relative placement cannot be made.
    fn relative_fault(
        &self,
        image: u32,
        pid: Option<u32>,
        parent_image: u32,
        parent_pid: Option<u32>,
    ) -> Option<(&'static str, &'static str)> {
        let index = placement_index(&self.placements);
        let mut depth = 1u32;
        let mut img = parent_image;
        let mut placement = parent_pid;
        let mut seen = Vec::new();
        loop {
            if depth > RELATIVE_DEPTH {
                return Some(("ETOODEEP", "the relative chain is too deep"));
            }
            if (img, placement) == (image, pid) || seen.contains(&(img, placement)) {
                return Some(("ECYCLE", "the relative placement would cycle"));
            }
            seen.push((img, placement));
            let Some(at) = index.get(Space::Apc, img, placement) else {
                return Some(("ENOPARENT", "the parent placement does not exist"));
            };
            if let Some((next_img, next_pid)) = self.placements[at].parent {
                depth += 1;
                img = next_img;
                placement = next_pid;
            } else {
                return None;
            }
        }
    }

    /// Relatives live with their parent. Clean them only after a removal,
    /// not on a frame that dropped nothing.
    ///
    /// When no placement has a parent, return at once. That scan allocates
    /// nothing. Otherwise follow only the keys just removed: one retain
    /// pass per hop, up to `RELATIVE_DEPTH`, using a small set of those
    /// keys — never a set of every placement. Pictures left with no showing
    /// go with the children.
    fn drop_parentless_if_removed(&mut self, mut removed: Vec<ShowingKey>) {
        if removed.is_empty() || !showing_has_parent(&self.placements) {
            return;
        }
        let mut images = Vec::new();
        // A chain in insertion order leaves in one pass: a numbered child
        // that sits later is matched as soon as its parent is dropped.
        // A child that sits earlier needs another hop, up to RELATIVE_DEPTH.
        const GONE_CAP: usize = 16;
        for _hop in 0..RELATIVE_DEPTH {
            removed.retain(|key| self.showing_key_is_gone(*key));
            if removed.is_empty() {
                break;
            }
            let mut next = Vec::new();
            if removed.len() <= GONE_CAP {
                let mut gone = [removed[0]; GONE_CAP];
                let mut nkeys = 0;
                for key in &removed {
                    if nkeys == GONE_CAP {
                        break;
                    }
                    gone[nkeys] = *key;
                    nkeys += 1;
                }
                let mut kept = Vec::with_capacity(self.placements.len());
                let mut late: Vec<ShowingKey> = Vec::new();
                let mut late_end = 0usize;
                for p in self.placements.drain(..) {
                    let Some((img, pid)) = p.parent else {
                        kept.push(p);
                        continue;
                    };
                    let pk = (p.space, img, pid);
                    let mut k = 0;
                    let mut hit = false;
                    while k < nkeys {
                        if gone[k] == pk {
                            hit = true;
                            break;
                        }
                        k += 1;
                    }
                    if !hit {
                        kept.push(p);
                        continue;
                    }
                    let key = showing_key(&p);
                    if key.2.is_some() && nkeys < GONE_CAP {
                        late.push(key);
                        late_end = kept.len();
                        gone[nkeys] = key;
                        nkeys += 1;
                    } else {
                        next.push(key);
                    }
                    images.push((p.space, p.picture));
                }
                self.placements = kept;
                if late_end > 0 && !late.is_empty() {
                    let mut extra = Vec::new();
                    let end = late_end.min(self.placements.len());
                    for i in 0..end {
                        let hit = match self.placements[i].parent {
                            Some((img, pid)) => late.iter().any(|key| *key == (self.placements[i].space, img, pid)),
                            None => false,
                        };
                        if hit {
                            extra.push(i);
                        }
                    }
                    for i in extra.into_iter().rev() {
                        let p = self.placements.remove(i);
                        next.push(showing_key(&p));
                        images.push((p.space, p.picture));
                    }
                }
            } else {
                let mut gone: HashSet<ShowingKey> = removed.iter().copied().collect();
                self.placements.retain(|p| {
                    let Some((img, pid)) = p.parent else {
                        return true;
                    };
                    if !gone.contains(&(p.space, img, pid)) {
                        return true;
                    }
                    let key = showing_key(p);
                    if key.2.is_some() {
                        gone.insert(key);
                    }
                    next.push(key);
                    images.push((p.space, p.picture));
                    false
                });
            }
            if next.is_empty() {
                break;
            }
            removed = next;
        }
        self.drop_unshown(images);
    }

    /// True when no remaining showing has this key. A numbered id is unique,
    /// so once a removal site handed it over it is gone.
    fn showing_key_is_gone(&self, key: ShowingKey) -> bool {
        if key.2.is_some() {
            return true;
        }
        !self.placements.iter().any(|p| p.placement_id.is_none() && p.space == key.0 && p.picture == key.1)
    }

    /// Drop pictures the child walk left with no showing.
    fn drop_unshown(&mut self, images: Vec<(Space, u32)>) {
        if images.is_empty() {
            return;
        }
        let mut seen = HashSet::new();
        for (space, id) in images {
            if seen.insert((space, id)) && !self.placements.iter().any(|p| p.space == space && p.picture == id) {
                self.drop_picture(space, id);
            }
        }
    }

    /// Keep showings `keep` accepts, in insertion order. When a relative
    /// showing exists, hand back the keys this pass removed so their
    /// children can follow. When none exist, nothing is allocated.
    fn retain_showings(&mut self, mut keep: impl FnMut(&Put) -> bool) -> Vec<ShowingKey> {
        if !showing_has_parent(&self.placements) {
            self.placements.retain(keep);
            return Vec::new();
        }
        let mut removed = Vec::new();
        self.placements.retain(|p| {
            if keep(p) {
                true
            } else {
                removed.push(showing_key(p));
                false
            }
        });
        removed
    }

    /// Take away the showings a predicate names. The freeing forms also
    /// drop the pictures' data once nothing shows them.
    fn geometric(&mut self, free: bool, hits: impl Fn(&Put) -> bool + 'static) -> DeleteOutcome {
        self.pending_free = Some(free);
        (
            if free {
                self.pictures.iter().filter(|k| k.space == Space::Apc).map(|k| k.dialect).collect()
            } else {
                Vec::new()
            },
            Box::new(move |p| !hits(p)),
        )
    }

    fn drop_picture(&mut self, space: Space, id: u32) -> Vec<ShowingKey> {
        if space == Space::Apc {
            self.drop_animation(id);
            self.anonymous.remove(&id);
        }
        if let Some(at) = self.pictures.iter().position(|k| k.space == space && k.dialect == id) {
            self.bytes -= self.pictures[at].rgba.len();
            let kept = self.pictures.remove(at);
            if space != Space::Glass {
                self.freed.push(kept.gpu);
            }
            self.retain_showings(|p| !(p.space == space && p.picture == id))
        } else {
            Vec::new()
        }
    }

    /// An anonymous put at this cell replaces the last anonymous put there,
    /// picture and placement, so a stream of frames does not grow the store.
    /// `incoming` is noted as anonymous for the next frame. Anonymous puts
    /// belong to the APC graphics dialect.
    fn supplant_anonymous(
        &mut self,
        row: u16,
        column: f32,
        alternate: bool,
        scrolled: u64,
        incoming: u32,
    ) {
        let line = self.anchor_with(alternate, row, scrolled);
        let old: Vec<u32> = self
            .placements
            .iter()
            .filter(|p| {
                p.space == Space::Apc
                    && p.alternate == alternate
                    && p.line == line
                    && (p.column - column).abs() < 1e-3
                    && self.anonymous.contains(&p.picture)
            })
            .map(|p| p.picture)
            .collect();
        if !old.is_empty() {
            let removed = self.retain_showings(|p| {
                !(p.space == Space::Apc
                    && p.alternate == alternate
                    && p.line == line
                    && (p.column - column).abs() < 1e-3
                    && old.contains(&p.picture))
            });
            self.drop_parentless_if_removed(removed);
            for id in old {
                self.anonymous.remove(&id);
                if !self.placements.iter().any(|p| p.space == Space::Apc && p.picture == id) {
                    let removed = self.drop_picture(Space::Apc, id);
                    self.drop_parentless_if_removed(removed);
                }
            }
        }
        self.anonymous.insert(incoming);
    }

    fn kept_in(&self, space: Space, id: u32) -> Option<&Kept> {
        self.pictures.iter().find(|k| k.space == space && k.dialect == id)
    }

    fn kept(&self, id: u32) -> Option<&Kept> {
        // The APC graphics dialect asks by its own ids.
        self.kept_in(Space::Apc, id)
    }

    fn bump(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// Spend the budget and the count cap: the oldest pictures go
    /// first, and their showings with them.
    fn evict(&mut self) -> Vec<ShowingKey> {
        self.evict_except(None)
    }

    /// As [`Self::evict`], but a picture being framed is not the one that
    /// goes: refusing a frame must not destroy the picture it belongs to.
    fn evict_except(&mut self, skip: Option<(Space, u32)>) -> Vec<ShowingKey> {
        let mut removed = Vec::new();
        let oldest = |pictures: &[Kept]| {
            pictures
                .iter()
                .enumerate()
                .filter(|(_, k)| skip != Some((k.space, k.dialect)))
                .min_by_key(|(_, k)| k.seq)
                .map(|(at, _)| at)
        };
        while self.pictures.len() > PICTURE_CAP {
            let Some(at) = oldest(&self.pictures) else { break };
            let space = self.pictures[at].space;
            let id = self.pictures[at].dialect;
            removed.extend(self.drop_picture(space, id));
        }
        while self.bytes > self.budget {
            let Some(at) = oldest(&self.pictures) else { break };
            let space = self.pictures[at].space;
            let id = self.pictures[at].dialect;
            removed.extend(self.drop_picture(space, id));
        }
        removed
    }

    /// A kept picture's pixels by the id its dialect gave it: width,
    /// height, straight RGBA, and when it arrived (a re-sent picture
    /// arrives anew), for a consumer that shows it whole, such as the
    /// showcase glass's own dialect opens.
    pub fn picture_pixels(&self, dialect: u32) -> Option<(u32, u32, Arc<Vec<u8>>, u64)> {
        // The showcase asks by glass's own window id. A client id of the
        // same number is a different picture.
        self.pixels_of(Space::Glass, dialect).or_else(|| self.pixels_of(Space::Apc, dialect))
    }

    /// Pixels of one dialect's picture, never another's of the same number.
    pub fn pixels_of(&self, space: Space, id: u32) -> Option<(u32, u32, Arc<Vec<u8>>, u64)> {
        self.kept_in(space, id).map(|k| (k.width, k.height, Arc::clone(&k.rgba), k.seq))
    }

    /// The window id of one dialect's picture.
    pub fn gpu_in(&self, space: Space, id: u32) -> Option<u32> {
        self.kept_in(space, id).map(|k| k.gpu)
    }

    /// Whether any virtual placement exists. The frame asks this before
    /// building a cell for every visible cell.
    pub fn has_virtual(&self) -> bool {
        self.placements.iter().any(|p| p.virtual_place)
    }

    /// Whether nothing here hangs on the text: no showing on either
    /// screen, and no layer of glass's own dialect anchored to a line.
    /// While that holds, what text does (writes, scrolls, clears, a
    /// screen switch) moves, covers and drops nothing, so the reader may
    /// feed a run of text pieces to the engine at once.
    pub fn quiet(&self) -> bool {
        self.placements.is_empty() && !self.expansion.anchors_text()
    }

    /// Placeholder cells and the relative showings that hang from a virtual
    /// placement, as layers for this frame. Empty unless a virtual placement exists.
    pub fn placeholder_layers(
        &self,
        cells: &[placeholders::CellRef],
        cols: u16,
        cell_w: f32,
        cell_h: f32,
    ) -> Vec<Shown> {
        if !self.has_virtual() {
            return Vec::new();
        }
        let animating = !self.animated.is_empty();
        let boxes: Vec<placeholders::Virtual> = self
            .placements
            .iter()
            .filter(|p| p.virtual_place)
            .filter_map(|p| {
                let kept = self.kept_in(p.space, p.picture)?;
                Some(placeholders::Virtual {
                    image: p.picture,
                    placement: p.placement_id,
                    columns: p.box_cols.max(1),
                    rows: p.box_rows.max(1),
                    width: kept.width.max(1),
                    height: kept.height.max(1),
                    gpu: if animating {
                        self.shown_frame_gpu(p.space, p.picture).unwrap_or(kept.gpu)
                    } else {
                        kept.gpu
                    },
                })
            })
            .collect();
        let (parts, origins) = placeholders::parts(cells, cols, &boxes, cell_w.max(1.0), cell_h.max(1.0));
        let mut out = Vec::new();
        for (i, part) in parts.into_iter().enumerate() {
            out.push(Shown {
                id: 0x10EE_0000_0000 | i as u64,
                gpu: part.gpu,
                column: part.column,
                row: part.row,
                width: part.width,
                height: part.height,
                z: 0,
                source: Some(part.source),
            });
        }
        let index = placement_index(&self.placements);
        for i in 0..self.placements.len() {
            if !chain_ends_virtual(&self.placements, &index, i) {
                continue;
            }
            let p = &self.placements[i];
            let Some((column, row)) = virtual_child_place(&self.placements, &index, i, &origins) else { continue };
            let Some(kept) = self.kept_in(p.space, p.picture) else { continue };
            let gpu = if animating {
                self.shown_frame_gpu(p.space, p.picture).unwrap_or(kept.gpu)
            } else {
                kept.gpu
            };
            out.push(Shown {
                id: p.seq,
                gpu,
                column,
                row,
                width: p.width,
                height: p.height,
                z: p.z,
                source: p.source,
            });
        }
        out
    }

    /// Drop the showings whose text has scrolled past the scrollback, then
    /// describe what this view shows. Each screen prunes by its own count:
    /// a measurement taken while the alternate screen is up never prunes
    /// or moves the primary screen's showings.
    pub fn describe(&mut self, view: View) -> Description {
        let gone_primary = self.primary.scrolled.saturating_sub(self.primary.history as u64);
        let gone_alternate =
            self.alternate_screen.scrolled.saturating_sub(self.alternate_screen.history as u64);
        let removed = self.retain_showings(|p| {
            // A virtual placement and a relative showing have no line of
            // their own; they live with what they refer to.
            if p.virtual_place || p.parent.is_some() {
                return true;
            }
            let gone = if p.alternate { gone_alternate } else { gone_primary };
            p.line + p.height.max(1.0).ceil() as u64 > gone
        });
        // Only a scroll-off that actually removed a showing can orphan a
        // relative. A quiet frame does not walk the placements again.
        self.drop_parentless_if_removed(removed);
        let mut pictures = Vec::new();
        for kept in &mut self.pictures {
            if kept.dirty {
                pictures.push(Upload {
                    gpu: kept.gpu,
                    width: kept.width,
                    height: kept.height,
                    rgba: Arc::clone(&kept.rgba),
                });
                kept.dirty = false;
            }
        }
        // Frames upload once, under their own window ids, and only when
        // an animation exists: a quiet frame does not walk them.
        let animating = !self.animated.is_empty();
        if animating {
            pictures.extend(self.take_frame_uploads());
        }
        // One lookup table per frame, so the frame costs placements plus
        // pictures, not their product. Parents are followed through a
        // second table, not a scan of every placement at every hop.
        let by_id: std::collections::HashMap<(Space, u32), &Kept> =
            self.pictures.iter().map(|k| ((k.space, k.dialect), k)).collect();
        let index = placement_index(&self.placements);
        let mut placements = Vec::with_capacity(self.placements.len());
        for i in 0..self.placements.len() {
            let p = &self.placements[i];
            if p.alternate != view.alternate || p.virtual_place {
                continue;
            }
            // A showing relative to a virtual placement is positioned from
            // the placeholder cells, in the render scan. `resolved_place`
            // refuses that chain, so it is not walked a second time here.
            let Some((line, column)) = resolved_place(&self.placements, &index, i) else { continue };
            let Some(&kept) = by_id.get(&(p.space, p.picture)) else { continue };
            let gpu = if animating {
                self.shown_frame_gpu(p.space, p.picture).unwrap_or(kept.gpu)
            } else {
                kept.gpu
            };
            let (w, h) = if p.width > 0.0 && p.height > 0.0 {
                (p.width, p.height)
            } else {
                (
                    kept.width as f32 / self.cell.0 as f32,
                    kept.height as f32 / self.cell.1 as f32,
                )
            };
            let row = line as f32 - view.top as f32;
            if p.in_cell && !p.covered.is_empty() {
                let cols = p.width.max(1.0).ceil() as u16;
                let rows = p.height.max(1.0).ceil() as u16;
                let parts = p.covered.uncovered(rows, cols);
                let image = (kept.width, kept.height);
                for (n, (off_row, off_col, part_rows, part_cols)) in parts.iter().enumerate() {
                    let part_h = f32::from(*part_rows).min(h - f32::from(*off_row)).max(0.0);
                    let part_w = f32::from(*part_cols).min(w - f32::from(*off_col)).max(0.0);
                    if part_h <= 0.0 || part_w <= 0.0 {
                        continue;
                    }
                    let part_row = row + f32::from(*off_row);
                    if part_row < f32::from(view.rows) && part_row + part_h > 0.0 {
                        placements.push(Shown {
                            id: if parts.len() == 1 {
                                p.seq
                            } else {
                                p.seq.wrapping_mul(64).wrapping_add(n as u64)
                            },
                            gpu,
                            row: part_row,
                            column: column + f32::from(*off_col),
                            width: part_w,
                            height: part_h,
                            z: p.z,
                            source: part_source(
                        p.source,
                        image,
                        (*off_col, *part_cols, *off_row, *part_rows),
                        (cols, rows),
                    ),
                        });
                    }
                }
            } else if row < f32::from(view.rows) && row + h > 0.0 {
                placements.push(Shown {
                    id: p.seq,
                    gpu,
                    row,
                    column,
                    width: w,
                    height: h,
                    z: p.z,
                    source: p.source,
                });
            }
        }
        Description { pictures, placements, freed: std::mem::take(&mut self.freed) }
    }
}

struct CanvasImpl<'a, 'e> {
    intake: &'a mut Intake,
    engine: &'e mut dyn Engine,
}


/// How long a chain of relative placements may be. The dialect requires at least 8.
const RELATIVE_DEPTH: u32 = 8;

/// `(space, picture, placement id)` to the first showing of that id.
enum PlaceIndex {
    /// One picture, ids packed into a table. `usize::MAX` is absent.
    Dense { space: Space, picture: u32, slots: Vec<usize> },
    Map(HashMap<(Space, u32, Option<u32>), usize>),
}

impl PlaceIndex {
    fn dense(placements: &[Put]) -> Option<Self> {
        let first = placements.first()?;
        first.placement_id?;
        let (space, picture) = (first.space, first.picture);
        let mut max_id = 0u32;
        for p in placements {
            if p.space != space || p.picture != picture {
                return None;
            }
            let id = p.placement_id?;
            if id > max_id {
                max_id = id;
            }
        }
        // A sparse id must not allocate a table the size of the id space.
        if max_id as usize > placements.len().saturating_mul(4) || max_id > 1_048_575 {
            return None;
        }
        let mut slots = vec![usize::MAX; max_id as usize + 1];
        for (i, p) in placements.iter().enumerate() {
            let id = p.placement_id?;
            if slots[id as usize] == usize::MAX {
                slots[id as usize] = i;
            }
        }
        Some(Self::Dense { space, picture, slots })
    }

    fn get(&self, space: Space, picture: u32, pid: Option<u32>) -> Option<usize> {
        match self {
            Self::Dense { space: s, picture: pic, slots } => {
                if space != *s || picture != *pic {
                    return None;
                }
                let slot = *slots.get(pid? as usize)?;
                if slot == usize::MAX { None } else { Some(slot) }
            }
            Self::Map(map) => map.get(&(space, picture, pid)).copied(),
        }
    }
}

/// `(space, picture, placement id)` to the first showing of that id. A later
/// showing of the same id does not hide the first: chains follow the earliest.
fn placement_index(placements: &[Put]) -> PlaceIndex {
    // One picture, packed ids: a table addressed by id. Same map otherwise.
    if let Some(index) = PlaceIndex::dense(placements) {
        return index;
    }
    let mut map = HashMap::with_capacity(placements.len());
    for (i, p) in placements.iter().enumerate() {
        map.entry((p.space, p.picture, p.placement_id)).or_insert(i);
    }
    PlaceIndex::Map(map)
}

fn chain_key(p: &Put) -> (u8, u32, Option<u32>) {
    let space = match p.space {
        Space::Apc => 0,
        Space::Sixel => 1,
        Space::Osc => 2,
        Space::Glass => 3,
    };
    (space, p.picture, p.placement_id)
}

/// The absolute line and column of a showing, following its parent chain.
/// A chain that ends at a virtual placement is not resolved here.
fn resolved_place(placements: &[Put], index: &PlaceIndex, start: usize) -> Option<(u64, f32)> {
    let mut depth = 0u32;
    let mut line_off = 0i64;
    let mut col_off = 0.0f32;
    let mut idx = start;
    // The chain is at most eight hops. An array avoids a heap allocation
    // on every showing of every frame.
    let mut seen = [(0u8, 0u32, None); RELATIVE_DEPTH as usize + 1];
    let mut seen_n = 0usize;
    loop {
        if depth > RELATIVE_DEPTH {
            return None;
        }
        let cur = placements.get(idx)?;
        let key = chain_key(cur);
        if seen[..seen_n].contains(&key) {
            return None;
        }
        seen[seen_n] = key;
        seen_n += 1;
        if let Some((img, pid)) = cur.parent {
            line_off += i64::from(cur.offset_v);
            col_off += cur.offset_h as f32;
            let space = cur.space;
            idx = index.get(space, img, pid)?;
            if placements[idx].virtual_place {
                return None;
            }
            depth += 1;
        } else {
            let line = (cur.line as i64).saturating_add(line_off).max(0) as u64;
            return Some((line, cur.column + col_off));
        }
    }
}

fn chain_ends_virtual(placements: &[Put], index: &PlaceIndex, start: usize) -> bool {
    let mut idx = start;
    let mut seen = [(0u8, 0u32, None); RELATIVE_DEPTH as usize + 1];
    for seen_n in 0..=RELATIVE_DEPTH as usize {
        let Some(cur) = placements.get(idx) else { return false };
        let key = chain_key(cur);
        if seen[..seen_n].contains(&key) {
            return true;
        }
        seen[seen_n] = key;
        let Some((img, pid)) = cur.parent else { return false };
        let space = cur.space;
        let Some(next) = index.get(space, img, pid) else { return false };
        if placements[next].virtual_place {
            return true;
        }
        idx = next;
    }
    true
}


fn virtual_child_place(
    placements: &[Put],
    index: &PlaceIndex,
    start: usize,
    origins: &[placeholders::Origin],
) -> Option<(f32, f32)> {
    let mut idx = start;
    let mut col_off = 0.0f32;
    let mut row_off = 0.0f32;
    let mut seen = [(0u8, 0u32, None); RELATIVE_DEPTH as usize + 1];
    for seen_n in 0..=RELATIVE_DEPTH as usize {
        let cur = placements.get(idx)?;
        let key = chain_key(cur);
        if seen[..seen_n].contains(&key) {
            return None;
        }
        seen[seen_n] = key;
        col_off += cur.offset_h as f32;
        row_off += cur.offset_v as f32;
        let (img, pid) = cur.parent?;
        let space = cur.space;
        let next = index.get(space, img, pid)?;
        if placements[next].virtual_place {
            let origin = origins.iter().find(|o| o.image == img && o.placement == pid)?;
            return Some((origin.column + col_off, origin.row + row_off));
        }
        idx = next;
    }
    None
}

impl Canvas for CanvasImpl<'_, '_> {
    fn cell(&self) -> (u32, u32) {
        self.intake.cell
    }

    fn cursor(&mut self) -> (u16, u16) {
        self.engine.cursor()
    }

    fn size(&mut self) -> (u16, u16) {
        self.engine.size()
    }

    fn alternate(&mut self) -> bool {
        self.engine.alternate()
    }

    fn mint(&mut self) -> u32 {
        self.intake.next_id += 1;
        self.intake.last_minted = Some(self.intake.next_id);
        self.intake.next_id
    }

    fn last_minted(&self) -> Option<u32> {
        self.intake.last_minted
    }

    fn number(&self, number: u32) -> Option<u32> {
        self.intake.numbers.get(&number).copied()
    }

    fn name_number(&mut self, number: u32, id: u32) {
        self.intake.numbers.insert(number, id);
    }

    fn store(&mut self, picture: Picture) {
        self.intake.store(picture);
    }

    fn place(&mut self, mut placement: Placement) {
        // The one anchor: adapters hand screen rows, the store pins lines.
        // An adapter places mid-read, so the engine's live count anchors
        // it; the store's own counts take over between reads.
        let alternate = self.engine.alternate();
        let scrolled = self.engine.scrolled();
        let line = self.intake.anchor_with(alternate, placement.line as u16, scrolled);
        placement.line = line;
        placement.alternate = alternate;
        self.intake.place(placement);
    }

    fn supplant_anonymous(&mut self, row: u16, column: f32, incoming: u32) {
        let alternate = self.engine.alternate();
        let scrolled = self.engine.scrolled();
        self.intake.supplant_anonymous(row, column, alternate, scrolled, incoming);
    }

    fn overlay(&mut self, picture: Picture, at: (u16, u16)) -> u32 {
        let alternate = self.engine.alternate();
        let scrolled = self.engine.scrolled();
        self.intake.overlay(picture, at, scrolled, alternate)
    }

    fn delete(&mut self, delete: Delete) {
        let cursor = self.engine.cursor();
        let alternate = self.engine.alternate();
        let rows = self.engine.size().0;
        self.intake.delete(delete, cursor, alternate, rows);
    }

    fn has(&self, id: u32) -> bool {
        self.intake.kept(id).is_some()
    }

    fn kept_size(&self, id: u32) -> Option<(u32, u32)> {
        self.intake.kept(id).map(|k| (k.width, k.height))
    }

    fn reply(&mut self, bytes: Vec<u8>) {
        self.intake.replies.extend_from_slice(&bytes);
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.engine.feed(bytes);
    }

    fn chunk(&mut self, key: u32, bytes: &[u8], more: bool) -> Vec<u8> {
        // The bytes held for an open transmission count against the same
        // budget the pictures do; past it the transmission fails.
        let held = self.intake.chunks.entry(key).or_default();
        if held.len() + bytes.len() > self.intake.budget {
            self.intake.chunks.clear();
            self.intake.chunk_heads.clear();
            return Vec::new();
        }
        held.extend_from_slice(bytes);
        if more {
            Vec::new()
        } else {
            self.intake.chunks.remove(&key).unwrap_or_default()
        }
    }

    fn open_chunk(&mut self) -> Option<u32> {
        let mut keys = self.intake.chunks.keys().copied().collect::<Vec<_>>();
        keys.sort_unstable();
        let [one] = keys[..] else { return None };
        Some(one)
    }

    fn begin_chunk(&mut self) -> u32 {
        // Only one transmission is open at a time: a new first chunk
        // drops whatever the old one had.
        self.intake.chunks.clear();
        self.intake.chunk_heads.clear();
        self.intake.chunk_key = self.intake.chunk_key.wrapping_add(1);
        self.intake.chunk_key
    }

    fn abort_chunk(&mut self) {
        self.intake.chunks.clear();
        self.intake.chunk_heads.clear();
    }

    fn multipart_open(&mut self, args: &[u8]) {
        if args.len() > self.intake.budget {
            self.intake.multipart = None;
            return;
        }
        self.intake.multipart = Some((args.to_vec(), Vec::new()));
    }

    fn multipart_append(&mut self, chunk: &[u8]) -> bool {
        let Some(open) = self.intake.multipart.as_mut() else { return false };
        let next = open.0.len().saturating_add(open.1.len()).saturating_add(chunk.len());
        if next > self.intake.budget {
            self.intake.multipart = None;
            return false;
        }
        open.1.extend_from_slice(chunk);
        true
    }

    fn multipart_finish(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
        self.intake.multipart.take()
    }

    fn chunk_head(&mut self, key: u32, keys: Vec<u8>) {
        self.intake.chunk_heads.insert(key, keys);
    }

    fn take_chunk_head(&mut self, key: u32) -> Vec<u8> {
        self.intake.chunk_heads.remove(&key).unwrap_or_default()
    }

    fn scrolled(&mut self) -> u64 {
        self.engine.scrolled()
    }

    fn gpu_of(&mut self, id: u32) -> Option<u32> {
        // glass's own dialect asks by its window id. A client id of the
        // same number is a different picture.
        self.intake.gpu_in(Space::Glass, id)
    }

    fn glass_size(&self, id: u32) -> Option<(u32, u32)> {
        self.intake.kept_in(Space::Glass, id).map(|k| (k.width, k.height))
    }

    fn relative_fault(
        &self,
        image: u32,
        pid: Option<u32>,
        parent_image: u32,
        parent_pid: Option<u32>,
    ) -> Option<(&'static str, &'static str)> {
        self.intake.relative_fault(image, pid, parent_image, parent_pid)
    }

    fn expansion(&mut self) -> &mut glass_dialect::Expansion {
        &mut self.intake.expansion
    }

    fn forget_glass(&mut self, id: u32) {
        let removed = self.intake.drop_picture(Space::Glass, id);
        self.intake.drop_parentless_if_removed(removed);
    }

    fn open_frame_chunk(&self) -> bool {
        self.intake.open_frame_chunk()
    }

    fn frame_put(&mut self, frame: FramePut) -> Result<(), (String, String)> {
        self.intake.frame_put(frame)
    }

    fn animate(&mut self, control: AnimCtrl) -> Result<(), (String, String)> {
        self.intake.animate(control)
    }

    fn compose(&mut self, compose: Compose) -> Result<(), (String, String)> {
        self.intake.compose(compose)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A recording engine and canvas, so the adapters and the store can be
    //! exercised without a terminal or a window.

    use super::*;

    /// A small PNG of a known pattern, encoded in-test.
    pub fn encode_png(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let data: Vec<u8> = (0..w * h * 4).map(|i| (i % 251) as u8).collect();
            writer.write_image_data(&data).unwrap();
        }
        bytes
    }

    /// An engine that answers from its fields and records what is fed back.
    pub struct FakeEngine {
        pub cursor: (u16, u16),
        pub size: (u16, u16),
        pub scrolled: u64,
        pub alternate: bool,
        pub fed: Vec<u8>,
    }

    impl FakeEngine {
        pub fn new() -> Self {
            Self { cursor: (0, 0), size: (24, 80), scrolled: 0, alternate: false, fed: Vec::new() }
        }
    }

    impl Engine for FakeEngine {
        fn cursor(&mut self) -> (u16, u16) {
            self.cursor
        }

        fn scrolled(&mut self) -> u64 {
            self.scrolled
        }

        fn size(&mut self) -> (u16, u16) {
            self.size
        }

        fn alternate(&mut self) -> bool {
            self.alternate
        }

        fn feed(&mut self, bytes: &[u8]) {
            self.fed.extend_from_slice(bytes);
        }
    }

    /// Everything the canvas did, recorded.
    #[derive(Default)]
    pub struct Record {
        pub cell: (u32, u32),
        pub pictures: Vec<Picture>,
        pub placements: Vec<Placement>,
        pub deletes: Vec<Delete>,
        pub replies: Vec<u8>,
        pub fed: Vec<u8>,
        pub mints: u32,
        last_minted: Option<u32>,
        numbers: HashMap<u32, u32>,
        has: HashMap<u32, (u32, u32)>,
        chunks: HashMap<u32, Vec<u8>>,
        chunk_heads: HashMap<u32, Vec<u8>>,
        chunk_key: u32,
        /// A budget for an open multipart file. Zero means no cap, so a
        /// recording canvas does not drop a transfer a test did not limit.
        pub budget: usize,
        multipart: Option<(Vec<u8>, Vec<u8>)>,
        /// glass's own dialect's state, when a test drives it.
        pub expansion: Option<glass_dialect::Expansion>,
    }

    impl Record {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl Canvas for Record {
        fn cell(&self) -> (u32, u32) {
            self.cell
        }

        fn cursor(&mut self) -> (u16, u16) {
            (0, 0)
        }

        fn size(&mut self) -> (u16, u16) {
            (24, 80)
        }

        fn alternate(&mut self) -> bool {
            false
        }

        fn mint(&mut self) -> u32 {
            self.mints += 1;
            self.last_minted = Some(self.mints);
            self.mints
        }

        fn last_minted(&self) -> Option<u32> {
            self.last_minted
        }

        fn number(&self, number: u32) -> Option<u32> {
            self.numbers.get(&number).copied()
        }

        fn name_number(&mut self, number: u32, id: u32) {
            self.numbers.insert(number, id);
        }

        fn store(&mut self, picture: Picture) {
            self.has.insert(picture.id, (picture.width, picture.height));
            self.pictures.push(picture);
        }

        fn place(&mut self, placement: Placement) {
            self.placements.push(placement);
        }

        fn overlay(&mut self, picture: Picture, at: (u16, u16)) -> u32 {
            self.mints += 1;
            self.last_minted = Some(self.mints);
            let id = self.mints;
            let (w, h) = (picture.width as f32, picture.height as f32);
            let line = u64::from(at.0);
            let column = f32::from(at.1);
            self.placements.retain(|p| {
                !(p.line + p.height.max(1.0).ceil() as u64 > line
                    && p.line < line + h.ceil() as u64
                    && p.column + p.width.max(1.0) > column
                    && p.column < column + w)
            });
            self.has.insert(id, (picture.width, picture.height));
            self.pictures.push(picture.with_id(id));
            self.placements.push(Placement {
                picture: id,
                line,
                column,
                width: w,
                height: h,
                z: 0,
                source: None,
                placement_id: None,
                alternate: false,
                in_cell: false,
                space: Space::Apc,
                virtual_place: false,
                parent: None,
                offset_h: 0,
                offset_v: 0,
                box_cols: 0,
                box_rows: 0,
            });
            id
        }

        fn delete(&mut self, delete: Delete) {
            self.deletes.push(delete);
        }

        fn has(&self, id: u32) -> bool {
            self.has.contains_key(&id)
        }

        fn kept_size(&self, id: u32) -> Option<(u32, u32)> {
            self.has.get(&id).copied()
        }

        fn reply(&mut self, bytes: Vec<u8>) {
            self.replies.extend_from_slice(&bytes);
        }

        fn feed(&mut self, bytes: &[u8]) {
            self.fed.extend_from_slice(bytes);
        }

        fn chunk(&mut self, key: u32, bytes: &[u8], more: bool) -> Vec<u8> {
            self.chunks.entry(key).or_default().extend_from_slice(bytes);
            if more {
                Vec::new()
            } else {
                self.chunks.remove(&key).unwrap_or_default()
            }
        }

        fn open_chunk(&mut self) -> Option<u32> {
            let mut keys = self.chunks.keys().copied().collect::<Vec<_>>();
            keys.sort_unstable();
            let [one] = keys[..] else { return None };
            Some(one)
        }

        fn begin_chunk(&mut self) -> u32 {
            self.chunks.clear();
            self.chunk_heads.clear();
            self.chunk_key = self.chunk_key.wrapping_add(1);
            self.chunk_key
        }

        fn abort_chunk(&mut self) {
            self.chunks.clear();
            self.chunk_heads.clear();
        }

        fn multipart_open(&mut self, args: &[u8]) {
            let limit = if self.budget == 0 { usize::MAX } else { self.budget };
            if args.len() > limit {
                self.multipart = None;
                return;
            }
            self.multipart = Some((args.to_vec(), Vec::new()));
        }

        fn multipart_append(&mut self, chunk: &[u8]) -> bool {
            let limit = if self.budget == 0 { usize::MAX } else { self.budget };
            let Some(open) = self.multipart.as_mut() else { return false };
            let next = open.0.len().saturating_add(open.1.len()).saturating_add(chunk.len());
            if next > limit {
                self.multipart = None;
                return false;
            }
            open.1.extend_from_slice(chunk);
            true
        }

        fn multipart_finish(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
            self.multipart.take()
        }

        fn chunk_head(&mut self, key: u32, keys: Vec<u8>) {
            self.chunk_heads.insert(key, keys);
        }

        fn take_chunk_head(&mut self, key: u32) -> Vec<u8> {
            self.chunk_heads.remove(&key).unwrap_or_default()
        }

        fn scrolled(&mut self) -> u64 {
            0
        }

        fn gpu_of(&mut self, id: u32) -> Option<u32> {
            self.has.get(&id).map(|_| id)
        }

        fn glass_size(&self, id: u32) -> Option<(u32, u32)> {
            self.has.get(&id).copied()
        }

        fn expansion(&mut self) -> &mut glass_dialect::Expansion {
            self.expansion.get_or_insert_with(glass_dialect::Expansion::new)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{FakeEngine, Record};
    use super::*;

    fn split(input: &[u8]) -> Vec<Piece<'_>> {
        let mut splitter = Splitter::default();
        let mut out = Vec::new();
        splitter.split(input, &mut out);
        out
    }

    fn split_by_byte(input: &[u8]) -> Vec<Vec<u8>> {
        // Feed one byte at a time: sequences split across reads must come
        // out exactly as they do whole. Adjacent text pieces join into
        // one run, as they are when the input arrives whole, so a whole
        // read and a dribble can be compared.
        let mut splitter = Splitter::default();
        let mut runs: Vec<Vec<u8>> = Vec::new();
        let mut last_was_text = false;
        for b in input {
            let dribble = [*b];
            let mut pieces = Vec::new();
            splitter.split(&dribble, &mut pieces);
            for p in pieces {
                match p {
                    Piece::Text(t) => {
                        if last_was_text {
                            if let Some(last) = runs.last_mut() {
                                last.extend_from_slice(&t);
                            } else {
                                runs.push(t.into_owned());
                            }
                        } else {
                            runs.push(t.into_owned());
                            last_was_text = true;
                        }
                    }
                    Piece::Media(media) => {
                        runs.push(media.payload().to_vec());
                        last_was_text = false;
                    }
                }
            }
        }
        runs
    }

    fn text(piece: &Piece<'_>) -> Vec<u8> {
        match piece {
            Piece::Text(t) => t.to_vec(),
            Piece::Media(_) => panic!("expected text"),
        }
    }

    #[test]
    fn plain_text_passes_through_untouched() {
        // Piece for piece is not promised — a sequence the intake does not
        // lift may come out in pieces — but every byte, in order, is.
        let pieces = split(b"hello\x1b[31mworld\x1b[0m\r\n");
        let joined: Vec<u8> = pieces.iter().flat_map(|p| text(p)).collect();
        assert_eq!(joined, b"hello\x1b[31mworld\x1b[0m\r\n");
    }

    #[test]
    fn an_apc_graphics_sequence_is_lifted_and_the_text_around_keeps_order() {
        let pieces = split(b"before\x1b_Ga=T,f=24,s=1,v=1;red\x1b\\after");
        assert_eq!(pieces.len(), 3);
        assert_eq!(text(&pieces[0]), b"before");
        assert_eq!(
            pieces[1],
            Piece::Media(Media::Apc(b"Ga=T,f=24,s=1,v=1;red".to_vec()))
        );
        assert_eq!(text(&pieces[2]), b"after");
    }

    #[test]
    fn an_apc_of_another_dialect_passes_through() {
        // The guarantee is byte-exact text in order, not how many pieces
        // it leaves in: a pass-through body streams through as it arrives.
        let (t, m) = one_shot(b"\x1b_|ain't graphics\x1b\\tail");
        assert_eq!(t, b"\x1b_|ain't graphics\x1b\\tail".to_vec());
        assert!(m.is_empty());
    }

    #[test]
    fn sixel_is_lifted_but_decrqss_and_xtgettcap_pass_through() {
        // DCS $ q (DECRQSS) and DCS + q (XTGETTCAP): intermediates, so not
        // sixel, however their final byte reads.
        let dcss: [&[u8]; 2] = [b"\x1bP$qm\x1b\\", b"\x1bP+q6d74\x1b\\"];
        for dcs in dcss {
            let pieces = split(dcs);
            let joined: Vec<u8> = pieces.iter().flat_map(|p| text(p)).collect();
            assert_eq!(joined, dcs.to_vec(), "{dcs:?}");
        }
        let pieces = split(b"\x1bP0;1q#0~~\x1b\\");
        assert_eq!(pieces, vec![Piece::Media(Media::Sixel(b"0;1q#0~~".to_vec()))]);
    }

    #[test]
    fn osc_1337_files_are_lifted_and_other_osc_passes_through() {
        let (t, m) = one_shot(b"\x1b]1337;File=inline=1;base64,aGk=\x1b\\\x1b]0;title\x07");
        assert_eq!(t, b"\x1b]0;title\x07".to_vec());
        assert_eq!(m, vec![Media::Osc1337(b"File=inline=1;base64,aGk=".to_vec())]);
    }

    #[test]
    fn osc_1337_of_another_kind_passes_through() {
        let (t, m) = one_shot(b"\x1b]1337;CurrentDir=/tmp\x07");
        assert_eq!(t, b"\x1b]1337;CurrentDir=/tmp\x07".to_vec());
        assert!(m.is_empty());
    }

    #[test]
    fn bel_also_ends_apc_and_osc_sequences() {
        let pieces = split(b"\x1b_Ga=t;..\x07\x1b]1337;File=inline=1;base64,aGk=\x07");
        assert_eq!(
            pieces[0],
            Piece::Media(Media::Apc(b"Ga=t;..".to_vec()))
        );
        assert_eq!(
            pieces[1],
            Piece::Media(Media::Osc1337(b"File=inline=1;base64,aGk=".to_vec()))
        );
    }

    #[test]
    fn sequences_split_across_reads_come_out_whole() {
        let whole = b"text\x1b_Ga=T;data\x1b\\\x1bPq#0~\x1b\\\x1b]1337;File=inline=1\x07end";
        // Whole: text, three media, text.
        let once: Vec<Vec<u8>> = split(whole)
            .into_iter()
            .map(|p| match p {
                Piece::Text(t) => t.into_owned(),
                Piece::Media(media) => media.payload().to_vec(),
            })
            .collect();
        assert_eq!(once.len(), 5);
        assert_eq!(split_by_byte(whole), once);
    }

    #[test]
    fn an_unfinished_sequence_waits_for_the_rest_of_it() {
        let mut splitter = Splitter::default();
        let mut pieces = Vec::new();
        splitter.split(b"abc\x1b_Ga=T,f=2", &mut pieces);
        assert_eq!(pieces.len(), 1);
        assert_eq!(text(&pieces[0]), b"abc");
        pieces.clear();
        splitter.split(b"4;data\x1b\\xyz", &mut pieces);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0], Piece::Media(Media::Apc(b"Ga=T,f=24;data".to_vec())));
        assert_eq!(text(&pieces[1]), b"xyz");
    }

    #[test]
    fn a_partial_osc_that_will_not_be_1337_passes_through_as_it_grows() {
        let mut splitter = Splitter::default();
        let mut pieces = Vec::new();
        // `1338` diverges at the last byte of the prefix: the held head
        // goes out with the divergence, and the terminator joins the
        // read's run. The guarantee is the bytes, not the piece count.
        splitter.split(b"\x1b]133", &mut pieces);
        assert!(pieces.is_empty());
        splitter.split(b"8;hello\x07", &mut pieces);
        let joined: Vec<u8> = pieces.iter().flat_map(|p| text(p)).collect();
        assert_eq!(joined, b"\x1b]1338;hello\x07".to_vec());
    }

    #[test]
    fn pictures_stay_attached_to_their_text_as_it_scrolls() {
        let mut intake = Intake::new(1);
        intake.set_cell((10, 20));
        intake.set_scrolled_primary(0, 0, 24);
        let mut engine = FakeEngine { cursor: (5, 3), ..FakeEngine::new() };
        intake.handle(Media::Apc(b"Ga=T,f=24,s=2,v=2,i=9;AAAAAAAAAAAAAAAA".to_vec()), &mut engine);
        let Description { placements, .. } = intake.describe(View { top: 0, rows: 24, alternate: false });
        assert_eq!((placements[0].row, placements[0].column), (5.0, 3.0));
        // Thirty lines scroll off; the view follows the text down, so the
        // picture's line 5 is above the view and shows nothing.
        intake.set_scrolled_primary(30, 30, 24);
        let Description { placements, .. } = intake.describe(View { top: 30, rows: 24, alternate: false });
        assert!(placements.is_empty(), "line 5 is above the view");
        // Back at the top of the scrollback it is where its text is.
        let Description { placements, .. } = intake.describe(View { top: 0, rows: 24, alternate: false });
        assert_eq!((placements[0].row, placements[0].column), (5.0, 3.0));
        // A view at top 25 covers lines 25..49; line 5 is above it.
        let Description { placements, .. } = intake.describe(View { top: 25, rows: 24, alternate: false });
        assert!(placements.is_empty(), "line 5 is above this view too");
        // Gone from the view entirely.
        let Description { placements, .. } = intake.describe(View { top: 50, rows: 24, alternate: false });
        assert!(placements.is_empty());
    }

    #[test]
    fn showings_go_when_their_text_scrolls_past_the_scrollback() {
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        // The picture rides on line 30, which will still be in history.
        let mut engine = FakeEngine { scrolled: 30, ..FakeEngine::new() };
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=1;////".to_vec()), &mut engine);
        // 40 lines have scrolled off; history keeps 20 of them (lines
        // 20..39), so line 30 is still in it.
        intake.set_scrolled_primary(40, 20, 24);
        intake.describe(View { top: 20, rows: 24, alternate: false });
        assert_eq!(intake.placements.len(), 1, "line 30 is still in history");
        // Enough scrolls that line 30 leaves the scrollback: gone is
        // past 31 (history still 20).
        intake.set_scrolled_primary(52, 20, 24);
        intake.describe(View { top: 32, rows: 24, alternate: false });
        assert!(intake.placements.is_empty(), "line 30 scrolled past the scrollback");
    }

    #[test]
    fn alternate_screen_showings_go_when_the_program_leaves_it() {
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine { alternate: true, ..FakeEngine::new() };
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=1;////".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 1);
        let Description { placements, .. } = intake.describe(View { top: 0, rows: 24, alternate: false });
        assert!(placements.is_empty(), "not shown on the main screen");
        intake.drop_alternate_placements();
        assert!(intake.placements.is_empty());
        assert_eq!(intake.pictures.len(), 1, "the picture itself stays until the budget spends it");
    }

    #[test]
    fn the_budget_spends_the_oldest_pictures_first() {
        let mut intake = Intake::with_budget(1, 2 * 2 * 4 * 2 + 1);
        let mut engine = FakeEngine::new();
        for id in 1..=3 {
            engine.scrolled = id;
            intake.handle(
                Media::Apc(format!("Ga=T,f=32,s=2,v=2,i={id};{}", "A".repeat(32)).into_bytes()),
                &mut engine,
            );
        }
        let ids: Vec<u32> = intake.pictures.iter().map(|k| k.dialect).collect();
        assert_eq!(ids, vec![2, 3], "the first was evicted, and its showing with it");
        assert!(intake.placements.iter().all(|p| p.picture != 1));
        assert_eq!(intake.bytes, 2 * 2 * 4 * 2);
    }

    #[test]
    fn a1_payloads_are_base64_decoded() {
        // A1: a real base64 PNG payload decodes; garbage is an error, not
        // a stored picture answered OK.
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine::new();
        let png = testing::encode_png(2, 2);
        let b64: String = {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for c in png.chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | u32::from(*c.get(2).unwrap_or(&0));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        intake.handle(Media::Apc(format!("Ga=T,f=100,i=3;{b64}").into_bytes()), &mut engine);
        assert_eq!(intake.pictures.len(), 1);
        assert_eq!((intake.pictures[0].width, intake.pictures[0].height), (2, 2));
        assert_eq!(intake.replies, b"\x1b_Gi=3;OK\x1b\\");

        // Raw f=32 with a real base64 payload: four bytes, one pixel.
        intake.handle(Media::Apc(b"Ga=T,f=32,s=1,v=1,i=4;AAAAAA==".to_vec()), &mut engine);
        assert_eq!(&intake.pictures[1].rgba[..], &[0, 0, 0, 0]);
    }

    #[test]
    fn a2_continuation_chunks_carry_only_m_and_q() {
        // A2: chunks after the first carry only `m` (and `q`); they join
        // the first chunk's transmission, and the final chunk decodes with
        // the first chunk's keys.
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=5,m=1;//8=".to_vec()), &mut engine);
        intake.handle(Media::Apc(b"Gm=1;////".to_vec()), &mut engine);
        assert!(intake.pictures.is_empty(), "the last chunk has not arrived");
        intake.handle(Media::Apc(b"Gm=0;q=2".to_vec()), &mut engine);
        assert_eq!(intake.pictures.len(), 1, "the assembled payload decodes");
        assert_eq!(intake.pictures[0].rgba.len(), 4, "one pixel of three channels plus alpha");
        assert!(intake.chunks.is_empty(), "nothing is left open");
    }

    #[test]
    fn a3_a_query_loads_and_answers_but_stores_nothing() {
        // A3: the standard probe loads its payload, answers OK, keeps
        // nothing and shows nothing.
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA".to_vec()), &mut engine);
        assert_eq!(intake.replies, b"\x1b_Gi=31;OK\x1b\\");
        assert!(intake.pictures.is_empty());
        assert!(intake.placements.is_empty());
    }

    #[test]
    fn a5_replies_only_when_an_id_or_number_was_given() {
        // A5: no `i`/`I`, no reply — the answer would land in the shell
        // as typed junk. Deletes never get an OK.
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1;////".to_vec()), &mut engine);
        intake.handle(Media::Apc(b"Ga=d".to_vec()), &mut engine);
        assert!(intake.replies.is_empty(), "nothing to read back");
        // An id answers with that id.
        intake.handle(Media::Apc(b"Ga=T,f=32,s=1,v=1,i=9;AAAAAA==".to_vec()), &mut engine);
        assert_eq!(intake.replies, b"\x1b_Gi=9;OK\x1b\\");
    }

    #[test]
    fn a6_x_y_w_h_name_the_source_rectangle() {
        // A6: x/y/w/h clip what shows of the picture; X/Y offset inside
        // the first cell. The placement does not move or scale.
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        intake.set_scrolled_primary(0, 0, 24);
        let mut engine = FakeEngine { cursor: (5, 3), ..FakeEngine::new() };
        let payload: String = "A".repeat((8_usize * 8 * 4).div_ceil(3) * 4);
        intake.handle(Media::Apc(format!("Ga=T,f=32,s=8,v=8,i=2;{payload}").into_bytes()), &mut engine);
        // show the middle 4x4 of it
        intake.handle(Media::Apc(b"Ga=p,i=2,x=2,y=2,w=4,h=4".to_vec()), &mut engine);
        let p = intake.placements.last().unwrap();
        assert_eq!((p.line, p.column), (5, 3.0), "the source rectangle does not move it");
        assert_eq!(p.source, Some([2.0, 2.0, 4.0, 4.0]), "the source rectangle in pixels");
    }

    #[test]
    fn a7_the_delete_table_targets_what_the_keys_name() {
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        for id in 1..=2 {
            intake.handle(Media::Apc(format!("Ga=T,f=24,s=1,v=1,i={id};////").into_bytes()), &mut engine);
        }
        // d=i,i=10 deletes nothing here (no image 10), the showings stay.
        intake.handle(Media::Apc(b"Ga=d,d=i,i=10".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 2);
        // d=i,i=1 takes image 1's showings, data kept.
        intake.handle(Media::Apc(b"Ga=d,d=i,i=1".to_vec()), &mut engine);
        assert!(intake.placements.iter().all(|p| p.picture != 1));
        assert_eq!(intake.pictures.len(), 2, "lowercase keeps the data");
        // d=I,i=1 frees it too.
        intake.handle(Media::Apc(b"Ga=d,d=I,i=1".to_vec()), &mut engine);
        assert_eq!(intake.pictures.len(), 1);
        // d=z,z=-1 only takes z -1, not z 0.
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=3,z=-1;////".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 2);
        intake.handle(Media::Apc(b"Ga=d,d=z,z=-1".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 1, "only the z=-1 showing went");
    }

    #[test]
    fn a8_image_numbers_mint_and_map() {
        // A8: each I transmit mints a fresh id; the number maps to the
        // newest; the reply carries both; i and I together is EINVAL.
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,I=13;////".to_vec()), &mut engine);
        let first = intake.pictures.last().unwrap().dialect;
        assert!(intake.replies.ends_with(
            format!("\x1b_Gi={first},I=13;OK\x1b\\").as_bytes()
        ));
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,I=13;////".to_vec()), &mut engine);
        let second = intake.pictures.last().unwrap().dialect;
        assert_eq!(intake.pictures.len(), 2, "a number never replaces");
        assert!(second > first, "each transmit mints a fresh id");
        assert!(intake.replies.ends_with(
            format!("\x1b_Gi={second},I=13;OK\x1b\\").as_bytes()
        ));
        // A put by number shows the newest.
        intake.handle(Media::Apc(b"Ga=p,I=13".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 3, "two transmits showed twice, the put once more");
        assert_eq!(intake.placements.last().unwrap().picture, second, "the newest of the number");
        // i and I together is refused.
        intake.replies.clear();
        intake.handle(Media::Apc(b"Ga=T,i=1,I=2".to_vec()), &mut engine);
        assert_eq!(intake.replies, b"\x1b_Gi=1;EINVAL:i and I together\x1b\\");
    }

    #[test]
    fn a9_placement_ids_are_per_image() {
        // A9: image 2's p=1 does not replace image 1's p=1; a reply that
        // names a placement carries it.
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=1,p=1;////".to_vec()), &mut engine);
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=2,p=1;////".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 2, "each image keeps its own");
        assert_eq!(&intake.replies[intake.replies.len() - b"\x1b_Gi=2,p=1;OK\x1b\\".len()..], b"\x1b_Gi=2,p=1;OK\x1b\\");
    }

    #[test]
    fn a10_the_cursor_lands_after_the_picture() {
        // A10: right past the last column, down rows-1 with LF so the
        // screen scrolls; C=1 leaves the cursor alone.
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine { cursor: (2, 0), ..FakeEngine::new() };
        let payload: String = "A".repeat((16_usize * 32 * 4).div_ceil(3) * 4);
        intake.handle(Media::Apc(format!("Ga=T,f=32,s=16,v=32,i=1;{payload}").into_bytes()), &mut engine);
        assert_eq!(engine.fed, b"\x1b[2C\n", "two columns right, then one row down");
        engine.fed.clear();
        intake.handle(Media::Apc(b"Ga=p,i=1,C=1".to_vec()), &mut engine);
        assert!(engine.fed.is_empty(), "C=1 does not move the cursor");
    }

    #[test]
    fn a11_one_side_follows_the_shape_through_the_cell() {
        // A11: c=4 on a 80x160 px picture with 8x16 px cells is 4 rows,
        // not 8.
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        intake.set_scrolled_primary(0, 0, 24);
        let mut engine = FakeEngine::new();
        let payload: String = "A".repeat((80_usize * 160 * 4).div_ceil(3) * 4);
        intake.handle(Media::Apc(format!("Ga=T,f=32,s=80,v=160,i=1,c=4;{payload}").into_bytes()), &mut engine);
        let p = &intake.placements[0];
        assert_eq!(p.width, 4.0, "four columns as asked");
        assert_eq!(p.height, 4.0, "four rows: the shape through the cell's pixels");
    }

    #[test]
    fn a12_retransmitting_an_id_takes_its_showings() {
        // A12: new data replaces the old; it shows nothing until placed
        // again.
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=t,f=24,s=1,v=1,i=7;////".to_vec()), &mut engine);
        intake.handle(Media::Apc(b"Ga=p,i=7".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 1);
        intake.handle(Media::Apc(b"Ga=t,f=24,s=1,v=1,i=7;////".to_vec()), &mut engine);
        assert!(intake.placements.is_empty(), "the old showing went");
        assert_eq!(intake.pictures.len(), 1, "the data replaced the old");
    }

    #[test]
    fn a13_huge_dimensions_are_refused_not_stored() {
        // A13: a side past the cap answers EINVAL and stores nothing.
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=2147483648,v=2147483648,i=1;////".to_vec()), &mut engine);
        assert!(intake.pictures.is_empty());
        assert_eq!(intake.replies, b"\x1b_Gi=1;EINVAL:each side is capped at 8192\x1b\\");
    }

    #[test]
    fn a15_a_file_failure_answers_one_identical_error() {
        // A15: a missing file, a directory, anything unreadable — one
        // answer, so the reply teaches nothing about the filesystem.
        // Paths here are names inside the test's own view; none of them
        // is opened successfully.
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        let enc = |path: &str| {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for c in path.as_bytes().chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | u32::from(*c.get(2).unwrap_or(&0));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        for path in ["/nonexistent/no-such-file.png", "/proc/self/mem", "/etc"] {
            intake
                .handle(Media::Apc(format!("Ga=T,f=100,t=f,i=1;{}", enc(path)).into_bytes()), &mut engine);
        }
        // Each answer is one ESC ... ST sequence; cut at the ST and drop
        // the empty tail the last ST leaves.
        let mut seqs: Vec<&[u8]> = Vec::new();
        {
            let rest = intake.replies.as_slice();
            let mut start = 0;
            while let Some(at) = rest[start..]
                .windows(2)
                .position(|w| w == b"\x1b\\")
                .map(|i| start + i)
            {
                seqs.push(&rest[start..at]);
                start = at + 2;
            }
        }
        let seqs: Vec<&[u8]> = seqs.into_iter().filter(|p| !p.is_empty()).collect();
        assert_eq!(seqs.len(), 3, "each sequence answered: {seqs:?}");
        // Finding: every refusal says which check refused, so live
        // failures can be told apart (corrected with that change).
        let text: Vec<String> =
            seqs.iter().map(|s| String::from_utf8_lossy(s).into_owned()).collect();
        assert!(text[0].contains("not a readable path"), "{text:?}");
        assert!(text[1].contains("not a regular file in a place the dialect reads"), "{text:?}");
        assert!(text[2].contains("not a regular file in a place the dialect reads"), "{text:?}");
    }

    #[test]
    fn a_query_probe_with_size_and_offset_reads_only_that_span() {
        // icat's detection probe names a 3-byte temporary file but says
        // `S=47`: `S` is honoured as given, and a short read is not a
        // failure while the format needs no more bytes than were read.
        // Under the crate's own target, as the conformance tests do:
        // the machine's temp directory is shared with anything else
        // running there, and a file in it can vanish under our feet.
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/intake-test-files/span");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("three-bytes");
        std::fs::write(&path, b"abc").unwrap();
        let enc = |bytes: &[u8]| {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for c in bytes.chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | (u32::from(*c.get(2).unwrap_or(&0)));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        let probe = format!("Ga=q,f=24,t=f,s=1,v=1,S=47,i=2;{}", enc(path.to_str().unwrap().as_bytes()));
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(probe.into_bytes()), &mut engine);
        assert!(
            String::from_utf8_lossy(&intake.replies).contains(";OK"),
            "the probe answers OK: {:?}",
            String::from_utf8_lossy(&intake.replies)
        );
        // With `S` absent the whole file is read, and that answers OK
        // too: three bytes are enough for a query.
        let probe = format!("Ga=q,f=24,t=f,s=1,v=1,i=2;{}", enc(path.to_str().unwrap().as_bytes()));
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(probe.into_bytes()), &mut engine);
        assert!(
            String::from_utf8_lossy(&intake.replies).contains(";OK"),
            "the whole-file probe answers OK: {:?}",
            String::from_utf8_lossy(&intake.replies)
        );
        let _ = std::fs::remove_dir_all(std::path::Path::new(&path).parent().unwrap());
    }

    #[test]
    fn an_unpadded_base64_payload_is_taken_in() {
        // icat sends the shared memory name unpadded; so may a file path.
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/intake-test-files/unpadded");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload");
        std::fs::write(&path, b"abcd").unwrap();
        // base64("…/glass-test-pad") with the tail cut: unpadded.
        let full = {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let bytes = path.to_str().unwrap().as_bytes();
            let mut out = String::new();
            for c in bytes.chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | (u32::from(*c.get(2).unwrap_or(&0)));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        let unpadded = full.trim_end_matches('=');
        let probe = format!("Ga=q,f=24,t=f,s=1,v=1,i=9;{unpadded}");
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(probe.into_bytes()), &mut engine);
        assert!(
            String::from_utf8_lossy(&intake.replies).contains(";OK"),
            "unpadded base64 answers OK: {:?}",
            String::from_utf8_lossy(&intake.replies)
        );
        let _ = std::fs::remove_dir_all(std::path::Path::new(&path).parent().unwrap());
    }

    #[test]
    fn shared_memory_transmissions_are_taken_in() {
        // `t=s`: the payload names a POSIX shared memory object the test
        // creates and removes itself. The dialect reads `S` bytes from
        // `O` (the whole object when `S` is absent), then unlinks it.
        use std::ffi::CString;
        let name = "/glass-term-test-shm";
        let c = CString::new(name).unwrap();
        let bytes: Vec<u8> = (0..64u8).collect();
        unsafe {
            let fd = libc::shm_open(c.as_ptr(), libc::O_RDWR | libc::O_CREAT | libc::O_EXCL, 0o600);
            assert!(fd >= 0, "the test object could be created");
            let wrote = libc::write(fd, bytes.as_ptr().cast(), bytes.len());
            assert_eq!(wrote, bytes.len() as isize);
            libc::close(fd);
        }
        let enc = |data: &[u8]| {
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for c in data.chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | (u32::from(*c.get(2).unwrap_or(&0)));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        let png = testing::encode_png(2, 2);
        // Put the PNG in the object, ask for it with `S` covering it.
        let object_name = enc(name.as_bytes());
        unsafe {
            let fd = libc::shm_open(c.as_ptr(), libc::O_RDWR, 0o600);
            assert!(fd >= 0);
            libc::ftruncate(fd, png.len() as i64);
            libc::pwrite(fd, png.as_ptr().cast(), png.len(), 0);
            libc::close(fd);
        }
        let probe = format!(
            "Ga=q,f=100,t=s,s=2,v=2,S={},i=3;{}",
            png.len(),
            object_name
        );
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(probe.into_bytes()), &mut engine);
        let answer = String::from_utf8_lossy(&intake.replies).into_owned();
        assert!(answer.contains(";OK"), "the shared memory probe answers OK: {answer:?}");
        // The dialect unlinked it: a second open fails.
        let gone = unsafe { libc::shm_open(c.as_ptr(), libc::O_RDWR, 0o600) };
        if gone >= 0 {
            unsafe { libc::close(gone) };
            let _ = unsafe { libc::shm_unlink(c.as_ptr()) };
            panic!("the object was unlinked after the read");
        }
        // A name with a `/` past the first character is refused.
        let bad = enc(b"no/such");
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        intake.handle(
            Media::Apc(format!("Ga=q,f=100,t=s,s=2,v=2,i=4;{bad}").into_bytes()),
            &mut engine,
        );
        let answer = String::from_utf8_lossy(&intake.replies).into_owned();
        assert!(answer.contains("bad shared memory name"), "{answer:?}");
    }

    #[test]
    fn icats_last_chunk_without_m_completes_the_transmission() {
        // `kitten icat` sends a first chunk with `a=T,q=2,f=100,m=1` and
        // the rest as `a=T,q=2` — no `m` at all. The final chunk is a
        // continuation that closes the transmission.
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        let png = testing::encode_png(2, 2);
        let b64: String = {
            const T: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for c in png.chunks(3) {
                let n = (u32::from(c[0]) << 16)
                    | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                    | u32::from(*c.get(2).unwrap_or(&0));
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        let at = b64.len() / 2;
        let (first, last) = (&b64[..at], &b64[at..]);
        intake.handle(
            Media::Apc(format!("Ga=T,q=2,f=100,m=1,s=2,v=2;{first}").into_bytes()),
            &mut engine,
        );
        assert!(intake.pictures.is_empty(), "the transmission stays open");
        intake.handle(Media::Apc(format!("Ga=T,q=2;{last}").into_bytes()), &mut engine);
        assert_eq!(intake.pictures.len(), 1, "the final chunk closed it");
        let picture = &intake.pictures[0];
        assert_eq!((picture.width, picture.height), (2, 2));
        assert_eq!(picture.rgba.len(), 2 * 2 * 4);
    }

    #[test]
    fn the_live_icat_chunked_stream_shows_its_picture() {
        // The exact APC sequences `kitten icat --transfer-mode=stream`
        // sent for a 140 KB PNG, live-captured: one first chunk and one
        // final chunk with no `m`.
        let stream = include_bytes!("testdata/icat-stream-chunked.apc");
        let mut pieces: Vec<Media> = Vec::new();
        let mut rest: &[u8] = stream;
        while let (Some(at), Some(rel)) = (
            rest.windows(3).position(|w| w == b"\x1b_G"),
            rest.windows(3)
                .position(|w| w == b"\x1b_G")
                .and_then(|at| rest[at..].windows(2).position(|w| w == b"\x1b\\")),
        ) {
            let end = at + rel + 2;
            pieces.push(Media::Apc(rest[at + 2..end - 2].to_vec()));
            rest = &rest[end..];
        }
        assert!(pieces.len() >= 2, "the capture holds both chunks");
        // The real budget: a 140 KB PNG must fit while it is held.
        let mut intake = Intake::with_budget(1, 8 << 20);
        intake.set_cell((10, 20));
        let mut engine = FakeEngine::new();
        for piece in &pieces {
            intake.handle(piece.clone(), &mut engine);
        }
        assert_eq!(intake.pictures.len(), 1, "the whole stream made one picture");
        let picture = &intake.pictures[0];
        assert_eq!((picture.width, picture.height), (160, 160));
        assert_eq!(picture.rgba.len(), 160 * 160 * 4);
        // `a=T` on the first chunk puts it too, and the cursor moves past it.
        assert_eq!(intake.placements.len(), 1, "the transmission was put");
        assert!(!engine.fed.is_empty(), "the cursor moves after the put");
    }

    #[test]
    fn chunked_transmissions_wait_for_their_last_chunk() {
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=5,m=1;////".to_vec()), &mut engine);
        assert!(intake.pictures.is_empty());
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=5,m=0;////".to_vec()), &mut engine);
        assert_eq!(intake.pictures.len(), 1);
        assert_eq!(intake.pictures[0].rgba.len(), 4, "abcd truncates to one RGBA pixel");
    }

    #[test]
    fn w2_the_engine_merges_operations_on_the_store() {
        // W2: the four operations the engine side calls on the store.
        let mut intake = Intake::new(1);
        intake.set_cell((1, 1));
        let mut engine = FakeEngine::new();
        // One showing in the scrollback (line 0), one on screen (line 15).
        engine.scrolled = 0;
        engine.cursor = (0, 0);
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=1;////".to_vec()), &mut engine);
        engine.scrolled = 10;
        engine.cursor = (5, 0);
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=2;////".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 2);

        // ED 2: the screen's showings go, the scrollback's stay.
        intake.clear_screen(false, 10);
        assert_eq!(intake.placements.len(), 1, "the scrollback's stays");
        assert_eq!(intake.placements[0].line, 0);

        // ED 3: the scrollback's go. The screen's showing already went
        // with ED 2, so this takes the one that was left.
        intake.clear_saved_lines(10);
        assert!(intake.placements.is_empty(), "the scrollback's went too");

        // RIS: everything goes, pictures too.
        engine.scrolled = 0;
        engine.cursor = (0, 0);
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=3;////".to_vec()), &mut engine);
        assert_eq!(intake.pictures.len(), 3);
        intake.reset();
        assert!(intake.pictures.is_empty());
        assert!(intake.placements.is_empty());
        assert!(intake.numbers.is_empty());

        // Text over cells where a picture sits takes its showing.
        engine.cursor = (3, 0);
        let payload: String = "A".repeat((8_usize * 3).div_ceil(3) * 4);
        intake.handle(Media::Apc(format!("Ga=T,f=24,s=8,v=1,i=4;{payload}").into_bytes()), &mut engine);
        assert_eq!(intake.placements.len(), 1);
        // An APC placement is never covered; an in-cell one is.
        intake.covered(
            false,
            Area { lines: 3..4, columns: 0..80 },
        );
        assert_eq!(intake.placements.len(), 1, "an APC showing is never covered");
        let mut record = Record::new();
        record.cell = (8, 1);
        record.store(Picture { id: 9, width: 8, height: 8, rgba: vec![0; 8 * 8 * 4], space: Space::Apc });
        record.place(Placement { picture: 9, line: 3, column: 0.0, width: 1.0, height: 1.0, z: 0, source: None, placement_id: None, alternate: false, in_cell: false, space: Space::Apc, virtual_place: false, parent: None, offset_h: 0, offset_v: 0, box_cols: 0, box_rows: 0 });
        assert_eq!(intake.pictures.len(), 1, "the data stays");
    }

    #[test]
    fn a_clear_screen_takes_its_showings_but_not_the_scrollback() {
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        // One picture on a line still on screen, one already scrolled off.
        engine.scrolled = 0;
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=1;////".to_vec()), &mut engine);
        engine.scrolled = 10;
        intake.handle(Media::Apc(b"Ga=T,f=24,s=1,v=1,i=2;////".to_vec()), &mut engine);
        intake.clear(false, 10);
        assert_eq!(intake.placements.len(), 1, "the line already scrolled off stays");
        intake.clear(true, 10);
        assert!(intake.placements.is_empty(), "ESC [ 3 J takes the saved lines too");
    }

    #[test]
    fn showing_again_with_the_same_placement_id_moves_it() {
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine { cursor: (2, 0), ..FakeEngine::new() };
        // Keep the picture first, then show it twice under one placement id.
        intake.handle(Media::Apc(b"Ga=t,f=24,s=1,v=1,i=7;////".to_vec()), &mut engine);
        intake.handle(Media::Apc(b"Ga=p,i=7,p=3".to_vec()), &mut engine);
        engine.cursor = (6, 4);
        intake.handle(Media::Apc(b"Ga=p,i=7,p=3".to_vec()), &mut engine);
        assert_eq!(intake.placements.len(), 1);
        assert_eq!(intake.placements[0].line, 6);
        assert_eq!(intake.placements[0].column, 4.0);
    }

    #[test]
    fn delete_targets_what_the_dialect_names() {
        let mut intake = Intake::new(1);
        let mut engine = FakeEngine::new();
        for id in 1..=2 {
            intake.handle(Media::Apc(format!("Ga=T,f=24,s=1,v=1,i={id};////").into_bytes()), &mut engine);
        }
        intake.delete(Delete::PlacementOf(1, 7, false), (0, 0), false, 24);
        // No placement carried that pair, so nothing went.
        assert_eq!(intake.placements.len(), 2);
        intake.delete(Delete::PictureEverywhere(1), (0, 0), false, 24);
        assert!(intake.placements.iter().all(|p| p.picture != 1));
        assert_eq!(intake.pictures.len(), 1);
    }

    /// What one whole read produced: the text with the lifted sequences
    /// removed, and the payloads of the media in order.
    fn one_shot(input: &[u8]) -> (Vec<u8>, Vec<Media>) {
        let mut splitter = Splitter::default();
        let mut pieces = Vec::new();
        splitter.split(input, &mut pieces);
        fold(pieces)
    }

    fn fold(pieces: Vec<Piece<'_>>) -> (Vec<u8>, Vec<Media>) {
        let mut text = Vec::new();
        let mut media = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Text(t) => text.extend_from_slice(&t),
                Piece::Media(m) => media.push(m),
            }
        }
        (text, media)
    }

    /// Feed `input` as the given reads, one after another, into one
    /// splitter.
    fn run_reads(reads: &[&[u8]]) -> (Vec<u8>, Vec<Media>) {
        let mut splitter = Splitter::default();
        let mut text = Vec::new();
        let mut media = Vec::new();
        for read in reads {
            let mut pieces = Vec::new();
            splitter.split(read, &mut pieces);
            let (t, m) = fold(pieces);
            text.extend_from_slice(&t);
            media.extend(m);
        }
        (text, media)
    }

    /// Real streams, one per line of terminal life: shell output with
    /// colours and cursor saves, DECRQSS and XTGETTCAP queries with their
    /// ST, OSC titles with BEL and with ST, each of the three media
    /// dialects, aborted media, CAN and SUB, and a lone trailing ESC.
    fn corpus() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
        // (name, input, the text it must yield: the input with the lifted
        // sequences taken away). One entry per line of terminal life.
        vec![
            (
                "coloured shell output",
                b"\x1b[1;32mfrosty@\x1b[0m:~$ ls -l\r\ntotal 0\r\n".to_vec(),
                b"\x1b[1;32mfrosty@\x1b[0m:~$ ls -l\r\ntotal 0\r\n".to_vec(),
            ),
            (
                "cursor saves and DECDHL",
                b"\x1b7saved\x1b8\x1b=\x1b>\x1b(Bover\x1b[2J".to_vec(),
                b"\x1b7saved\x1b8\x1b=\x1b>\x1b(Bover\x1b[2J".to_vec(),
            ),
            (
                "DECRQSS",
                b"\x1bP$qm\x1b\\after".to_vec(),
                b"\x1bP$qm\x1b\\after".to_vec(),
            ),
            (
                "XTGETTCAP then an APC query",
                b"\x1bP+q6d74\x1b\\\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\".to_vec(),
                b"\x1bP+q6d74\x1b\\".to_vec(),
            ),
            (
                "OSC title with BEL",
                b"\x1b]0;my title\x07text".to_vec(),
                b"\x1b]0;my title\x07text".to_vec(),
            ),
            (
                "OSC title with ST",
                b"\x1b]2;other\x1b\\text".to_vec(),
                b"\x1b]2;other\x1b\\text".to_vec(),
            ),
            (
                "APC graphics",
                b"pre\x1b_Ga=T,f=100;png\x1b\\post".to_vec(),
                b"prepost".to_vec(),
            ),
            (
                "sixel",
                b"\x1bP0;1q#0;2;0;0;0~~\x1b\\".to_vec(),
                b"".to_vec(),
            ),
            (
                "OSC 1337 file",
                b"\x1b]1337;File=inline=1:aGk=\x07".to_vec(),
                b"".to_vec(),
            ),
            (
                "aborted APC",
                b"\x1b_Ga=T;xyz\x1b[1mQ".to_vec(),
                b"\x1b[1mQ".to_vec(),
            ),
            (
                "aborted by CAN",
                b"\x1b_Ga=T;xyz\x18rest".to_vec(),
                b"rest".to_vec(),
            ),
            (
                "aborted by SUB",
                b"\x1bPq#0~\x1astream".to_vec(),
                b"stream".to_vec(),
            ),
            (
                "DCS = 1 s",
                b"\x1bP=1sbody\x1b\\tail".to_vec(),
                b"\x1bP=1sbody\x1b\\tail".to_vec(),
            ),
            (
                "lone trailing ESC",
                b"hello \x1b".to_vec(),
                b"hello ".to_vec(),
            ),
            (
                // `ESC P tmux ;` is a passthrough. An undoubled ESC aborts it
                // and starts the next sequence, so the wrapper is not text
                // and the APC is lifted; the trailing ST is text.
                "tmux passthrough",
                b"\x1bPtmux;\x1b_Gi=1;ok\x1b\\\x1b\\".to_vec(),
                b"\x1b\\".to_vec(),
            ),
            (
                "plain text",
                b"just words, then a newline\n".to_vec(),
                b"just words, then a newline\n".to_vec(),
            ),
        ]
    }

#[test]
    fn every_split_of_every_corpus_stream_matches_the_one_shot_result() {
        for (name, input, want_text) in corpus() {
            let (once_text, once_media) = one_shot(&input);
            // The text is the input with the lifted sequences removed.
            assert_eq!(once_text, want_text, "{name}: the text is wrong");
            // Every single-byte cut.
            for at in 0..input.len() {
                let reads = [&input[..at], &input[at..]];
                let (text, media) = run_reads(&reads);
                assert_eq!(text, once_text, "{name}: cut at {at} changed the text");
                assert_eq!(media, once_media, "{name}: cut at {at} changed the media");
            }
            // One byte per read.
            let byte_reads: Vec<[u8; 1]> = input.iter().map(|b| [*b]).collect();
            let reads: Vec<&[u8]> = byte_reads.iter().map(|b| b.as_slice()).collect();
            let (text, media) = run_reads(&reads);
            assert_eq!(text, once_text, "{name}: byte-at-a-time changed the text");
            assert_eq!(media, once_media, "{name}: byte-at-a-time changed the media");
        }
    }
        /// S1: a read ending on ESC kept indices into the old buffer; the
    /// open text run and the ESC were lost, or the next read panicked.
    #[test]
    fn s1_a_read_ending_on_escape_keeps_the_text_and_the_escape() {
        let (text, _) = run_reads(&[b"abc\x1b", b"[31mX"]);
        assert_eq!(text, b"abc\x1b[31mX".to_vec());
        // The old code panicked here (`range end index out of range`).
        // `ESC P` is an open DCS: held until a later read resolves it,
        // then out exactly once.
        let (text, _) = run_reads(&[b"hello\x1b", b"P", b"\x1b\\"]);
        assert_eq!(text, b"hello\x1bP\x1b\\".to_vec());
        // And an `ESC P` that never ends stays held, holding nothing
        // else back after it.
        let (text, media) = run_reads(&[b"hello\x1b", b"P"]);
        assert_eq!(text, b"hello".to_vec());
        assert!(media.is_empty());
        let (text, media) = run_reads(&[b"hello world\x1b", b"_Gi=1;abcdef\x1b\\"]);
        assert_eq!(text, b"hello world".to_vec());
        assert_eq!(media, vec![Media::Apc(b"Gi=1;abcdef".to_vec())]);
    }

    /// S2: a read ending inside a pass-through escape duplicated and
    /// reordered its bytes.
    #[test]
    fn s2_a_read_ending_inside_an_escape_does_not_repeat_bytes() {
        let (text, _) = run_reads(&[b"abc\x1b[3", b"1mX"]);
        assert_eq!(text, b"abc\x1b[31mX".to_vec());
        let (text, _) = run_reads(&[b"abc\x1b7"]);
        assert_eq!(text, b"abc\x1b7".to_vec());
    }

    /// S3: a DCS that is not sixel never ended, and its body was
    /// re-emitted on every read.
    #[test]
    fn s3_a_pass_through_dcs_ends_once_and_is_not_repeated() {
        let (text, _) = run_reads(&[b"\x1bP$qm\x1b\\abc", b"def", b"ghi"]);
        assert_eq!(text, b"\x1bP$qm\x1b\\abcdefghi".to_vec());
        // An APC after a pass-through DCS is still lifted.
        let (_, media) =
            run_reads(&[b"\x1bP+q6d74\x1b\\\x1b_Ga=T;x\x1b\\"]);
        assert_eq!(media, vec![Media::Apc(b"Ga=T;x".to_vec())]);
    }

    /// S4: `ESC 7`, `ESC 8`, `ESC =`, `ESC >` hid the media after them,
    /// because only 0x40..=0x7e was taken as a final byte.
    #[test]
    fn s4_short_escapes_end_and_do_not_hide_media() {
        for escape in [b"\x1b7", b"\x1b8", b"\x1b=", b"\x1b>"] {
            let (text, media) = run_reads(&[escape, b"\x1b_Ga=T;x\x1b\\"]);
            assert_eq!(text, escape.to_vec());
            assert_eq!(media.len(), 1, "{escape:?}");
        }
    }

    /// S5 (by reading): lifted and pass-through buffers had no cap. A
    /// body over the cap is dropped and the splitter returns to text.
    #[test]
    fn s5_an_overlong_lifted_body_is_dropped_and_text_resumes() {
        let mut splitter = Splitter::with_body_cap(8);
        let mut pieces = Vec::new();
        splitter.split(b"\x1b_Ga=T;", &mut pieces);
        pieces.clear();
        splitter.split(b"0123456789abcd!\x1b\\tail", &mut pieces);
        let (text, media) = fold(pieces);
        // The cap keeps the first 8 bytes and then drops them; the rest of
        // the body is read and dropped until its terminator, so none of
        // the body reaches the engine and text resumes after the ST.
        assert_eq!(text, b"tail".to_vec());
        assert!(media.is_empty(), "the overlong body must not be lifted");
    }

    /// S6: aborted sequences printed their bodies without the introducer
    /// and dropped the next byte.
    #[test]
    fn s6_an_aborted_body_is_dropped_and_the_next_sequence_stands() {
        let (text, _) = run_reads(&[b"\x1b_Ga=T;xyz\x1b[1mQ"]);
        assert_eq!(text, b"\x1b[1mQ".to_vec());
        let (text, _) = run_reads(&[b"\x1b_Ga=T;xyz\x18rest"]);
        assert_eq!(text, b"rest".to_vec());
    }

    /// S7: an ESC right after `ESC _` or `ESC P` repeated earlier text.
    #[test]
    fn s7_an_escape_after_an_introducer_does_not_repeat_text() {
        let (text, _) = run_reads(&[b"ab\x1b_\x1b[1mX"]);
        assert_eq!(text, b"ab\x1b_\x1b[1mX".to_vec());
    }

    /// S8: `ESC ] 1 BEL` swallowed output up to the next terminator, and
    /// CAN/SUB were not handled in pass-through OSC/APC.
    #[test]
    fn s8_a_bel_breaking_the_osc_prefix_ends_the_osc() {
        let (text, _) = run_reads(&[b"\x1b]1\x07prompt> "]);
        assert_eq!(text, b"\x1b]1\x07prompt> ".to_vec());
        let (text, _) = run_reads(&[b"\x1b]0;t\x18more"]);
        assert_eq!(text, b"\x1b]0;t\x18more".to_vec());
    }

    #[test]
    fn the_canvas_records_what_adapters_ask_of_it() {
        let mut record = Record::new();
        record.cell = (8, 16);
        assert_eq!(record.cell(), (8, 16));
        record.store(Picture { id: 1, width: 2, height: 2, rgba: vec![0; 16], space: Space::Apc });
        assert!(record.has(1));
        assert_eq!(record.chunk(0, b"ab", true), Vec::<u8>::new());
        assert_eq!(record.chunk(0, b"cd", false), b"abcd".to_vec());
    }

    /// `ESC P tmux ;` + body with every ESC doubled + `ESC \`.
    fn tmux_wrap(body: &[u8]) -> Vec<u8> {
        let mut out = b"\x1bPtmux;".to_vec();
        for &byte in body {
            if byte == 0x1b {
                out.push(0x1b);
            }
            out.push(byte);
        }
        out.extend_from_slice(b"\x1b\\");
        out
    }

    #[test]
    fn tmux_passthrough_lifts_apc_sixel_and_osc_1337() {
        let cases = [
            (b"\x1b_Gi=1;ok\x1b\\".as_slice(), Media::Apc(b"Gi=1;ok".to_vec())),
            (b"\x1bPq#0~\x1b\\".as_slice(), Media::Sixel(b"q#0~".to_vec())),
            (
                b"\x1b]1337;File=inline=1:aGk=\x1b\\".as_slice(),
                Media::Osc1337(b"File=inline=1:aGk=".to_vec()),
            ),
        ];
        for (body, media) in cases {
            let wrapped = tmux_wrap(body);
            let (text, lifted) = one_shot(&wrapped);
            assert!(text.is_empty(), "wrapper is consumed: {text:?}");
            assert_eq!(lifted, vec![media]);
            // Every cut, including one between the two ESCs of a doubled pair.
            for split in 0..=wrapped.len() {
                let (t, m) = run_reads(&[&wrapped[..split], &wrapped[split..]]);
                assert_eq!(t, text, "split {split}");
                assert_eq!(m, lifted, "split {split}");
            }
        }
    }

    #[test]
    fn a_doubled_escape_across_a_read_boundary_is_one_escape() {
        let wrapped = tmux_wrap(b"\x1b[31mhi");
        let at = wrapped.windows(2).position(|w| w == b"\x1b\x1b").unwrap();
        let (text, media) = run_reads(&[&wrapped[..at + 1], &wrapped[at + 1..]]);
        assert!(media.is_empty());
        assert_eq!(text, b"\x1b[31mhi");
    }

    #[test]
    fn a_passthrough_past_the_cap_is_dropped() {
        let mut splitter = Splitter::with_body_cap(4);
        let mut out = Vec::new();
        let stream = tmux_wrap(b"0123456789tail");
        // The terminator is inside the wrap; text after it must resume.
        let mut whole = stream.clone();
        whole.extend_from_slice(b"after");
        splitter.split(&whole, &mut out);
        let (text, media) = fold(out);
        assert!(media.is_empty());
        assert_eq!(text, b"after");
    }

    #[test]
    fn a_passthrough_aborts_on_can_and_sub() {
        let (text, media) = one_shot(b"\x1bPtmux;hello\x18after");
        assert!(media.is_empty());
        assert_eq!(text, b"after");
        let (text, media) = one_shot(b"\x1bPtmux;hello\x1aafter");
        assert!(media.is_empty());
        assert_eq!(text, b"after");
    }

    #[test]
    fn a_nested_passthrough_is_not_unwrapped() {
        let inner = tmux_wrap(b"\x1b_Gi=1;ok\x1b\\");
        let outer = tmux_wrap(&inner);
        let (text, media) = one_shot(&outer);
        assert!(media.is_empty(), "one level only: {media:?}");
        assert_eq!(text, inner);
    }

    #[test]
    fn multipart_osc_1337_is_lifted() {
        let stream = b"\x1b]1337;MultipartFile=inline=1\x07\x1b]1337;FilePart=abcd\x07\x1b]1337;FileEnd\x07";
        let (text, media) = one_shot(stream);
        assert!(text.is_empty());
        assert_eq!(
            media,
            vec![
                Media::Osc1337(b"MultipartFile=inline=1".to_vec()),
                Media::Osc1337(b"FilePart=abcd".to_vec()),
                Media::Osc1337(b"FileEnd".to_vec()),
            ]
        );
        for split in 0..=stream.len() {
            let (t, m) = run_reads(&[&stream[..split], &stream[split..]]);
            assert_eq!((t, m), (text.clone(), media.clone()), "split {split}");
        }
    }

    /// A 7×5 TGA from the image crate's encoder, and the RGBA it carries.
    fn tga_7x5() -> (Vec<u8>, Vec<u8>) {
        let (w, h) = (7u32, 5u32);
        let rgba: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| [x as u8 * 30, y as u8 * 40, 90, 255]))
            .collect();
        let image = image::RgbaImage::from_raw(w, h, rgba.clone()).unwrap();
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut std::io::Cursor::new(&mut encoded), image::ImageFormat::Tga)
            .unwrap();
        (encoded, rgba)
    }

    fn b64_encode(data: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let n = (u32::from(chunk[0]) << 16)
                | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
                | u32::from(*chunk.get(2).unwrap_or(&0));
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
        }
        out
    }

    #[test]
    fn a_tga_through_f100_keeps_its_pixels() {
        let (tga, rgba) = tga_7x5();
        let payload = b64_encode(&tga);
        let mut intake = Intake::new(1);
        intake.set_cell((8, 16));
        let mut engine = FakeEngine::new();
        intake.handle(Media::Apc(format!("Ga=T,f=100,i=1;{payload}").into_bytes()), &mut engine);
        assert_eq!(intake.pictures.len(), 1, "f=100 takes the TGA");
        let picture = &intake.pictures[0];
        assert_eq!((picture.width, picture.height, picture.rgba.as_slice()), (7, 5, rgba.as_slice()));
    }

    #[test]
    fn random_bytes_guessing_rejects_stay_none_within_a_second() {
        let start = std::time::Instant::now();
        let mut state = 0xA5A5_1234_u64;
        for _ in 0..10_000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let len = (state as usize) % 48;
            let mut buf = vec![0u8; len];
            for byte in &mut buf {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (state >> 33) as u8;
            }
            let guessed = image::ImageReader::new(std::io::Cursor::new(buf.as_slice()))
                .with_guessed_format()
                .ok()
                .and_then(|reader| reader.format());
            if guessed.is_some() {
                continue;
            }
            assert!(
                decode_image(&buf).is_none(),
                "guessing rejected these bytes; they are not a picture"
            );
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "10 000 short buffers took {:?}",
            start.elapsed()
        );
    }
}
