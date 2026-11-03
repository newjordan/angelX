//! The remote link's messages and how they are framed (docs/REMOTE_GLASS.md).
//!
//! The link is a byte stream both ways: an SSH command's stdin and stdout, or
//! a session's Unix socket. Before its first frame the session's end writes
//! [`MAGIC`], so a window can skip whatever a login shell or a launcher
//! printed first. Then every message is one frame:
//!
//! ```text
//! length u32 (little-endian, the bytes after it), flags u8 (1: deflated),
//! then the message, bincode (varint integers)
//! ```
//!
//! What crosses is state, never a picture of the screen: the cockpit's grid
//! as runs of changed cells, the window's layers, stages and scenes as their
//! descriptions, and pictures and meshes as content-addressed blobs that go
//! once and are kept by their hash on the far side.

use std::io::{self, Read, Write};
use std::sync::Arc;

use bincode::Options as _;
use crossterm::event::Event;
use ratatui_core::buffer::Cell;
use ratatui_core::style::{Color, Modifier};
use serde::{Deserialize, Serialize};

use crate::scene::{Node, Scene};
use crate::{GridSize, Layer, Mark, Showing, Stage};

/// The protocol's version. A window and a session speak only their own.
pub const VERSION: u32 = 1;

/// What the session's end writes before its first frame.
pub const MAGIC: &[u8; 20] = b"\0ANGEL-REMOTE-GLASS\n";

/// The most bytes a window reads looking for [`MAGIC`] (a login banner, a
/// build's progress) before it gives up.
pub const MAX_PREAMBLE: usize = 1 << 20;

/// The largest frame either end accepts.
pub const MAX_FRAME: usize = 512 << 20;

/// Frames smaller than this are never deflated: the work outweighs the bytes.
const DEFLATE_FROM: usize = 2048;

/// How many of the last pictures sent each end keeps, any id, as bases for
/// a [`Down::PictureDelta`].
pub const RECENT: usize = 8;

/// A blob's name: 128 bits of its bytes.
pub type Hash = [u64; 2];

/// Who is on the other end of a session's socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A glass window: draws everything.
    Window,
    /// A terminal: draws the grid, pictures as half blocks.
    Terminal,
    /// A one-shot question (`--sessions`, `--kill-session`): no frames.
    Probe,
    /// The presence feed only (who is at work in the session), read-only
    /// and without attaching: a kingdom drawing every session's knights.
    Watch,
}

/// Who is at work in a session, for a kingdom that draws the knights of
/// every session it can reach. State, never frames; read-only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub session: String,
    /// The workspace's name (its folder), not its path.
    pub workspace: String,
    pub knights: Vec<Knight>,
}

/// One knight (a seat), drawn by its model family's look as the local
/// realm draws it. Knights are models, never machines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Knight {
    /// The model family (`gpt`, `grok`, `qwen`...): its look.
    pub family: String,
    pub persona: String,
    /// What it is doing, in a few words (a tool, "thinking").
    pub doing: String,
    pub status: KnightStatus,
    /// The current task line.
    pub task: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnightStatus {
    Working,
    Waiting,
    #[default]
    Idle,
}

/// A window to its session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Up {
    /// The first message. `have` names the blobs the window still holds
    /// from an earlier link, so they are not sent again.
    Hello { version: u32, kind: Kind, size: GridSize, have: Vec<Hash>, who: String },
    /// A key, a click, the wheel, a paste, focus: the window's input as the
    /// terminal events the cockpit reads.
    Input(Event),
    /// The window's grid changed size.
    Resize(GridSize),
    /// What the operator did to the showcase panel since the last report.
    Showcase(Feedback),
    /// End the session (`--kill-session`).
    Kill,
}

/// The showcase panel's answers, as deltas since the last report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Feedback {
    /// The session's serial of the showing this is about.
    pub serial: u64,
    pub closed: bool,
    pub clicks: u32,
    pub copies: u32,
    pub steps: i32,
    pub seek: Option<f32>,
}

/// A session, as `--sessions` lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub name: String,
    pub pid: u32,
    pub clients: u32,
    pub size: GridSize,
    /// Seconds since the epoch when it started.
    pub started: u64,
    pub title: String,
    /// Who is at work there (as a `Kind::Watch` link hears it).
    pub presence: Presence,
}

/// A session to its windows.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Down {
    Hello { version: u32, session: String },
    /// The answer to a probe.
    Status(Status),
    /// The session's grid size (the window that sizes it is the last one
    /// that was used: tmux's `latest`).
    Size(GridSize),
    Grid(Grid),
    /// A blob's bytes, sent before the first message that names it.
    Blob { hash: Hash, deflated: bool, bytes: Vec<u8> },
    /// A picture from a blob. `keep: false` is one frame of a stream (a
    /// video): the window drops the blob once the picture is up.
    Picture { id: u32, width: u32, height: u32, cols: u16, rows: u16, hash: Hash, keep: bool },
    /// A picture as a change of one of the last [`RECENT`] pictures the
    /// window was sent (`base`, any id: an animation's pictures take turns
    /// under two): `bytes` (deflated or not) XOR its pixels are the new
    /// ones, named `hash`. The Delve's painted parts and the realm's map change a few
    /// pixels a frame, so this is most of a stream's frames.
    PictureDelta { id: u32, width: u32, height: u32, cols: u16, rows: u16, base: Hash, hash: Hash, deflated: bool, bytes: Vec<u8> },
    /// A mesh for scenes to name, from a blob of its bincode.
    Mesh { id: u32, hash: Hash },
    /// The window's whole layer list.
    Layers(Arc<Vec<Layer>>),
    /// A stage set or taken down. Its world's meshes travel as blobs:
    /// `(mesh id, blob)` to put back in `props.world.meshes`.
    Stage { id: u32, stage: Option<Stage>, meshes: Vec<(u32, Hash)> },
    Scene { id: u32, scene: Option<(Scene, (u32, u32))> },
    ShowcaseOpen { serial: u64, showing: Showing, marks: Vec<Mark>, model: Option<Vec<Node>> },
    ShowcaseClose,
    ShowcaseProgress { at: Option<f32>, paused: bool },
    Title(String),
    Attention,
    Clipboard(String),
    GameKeyboard(bool),
    /// The end of one of the session's frames: show what came before it.
    Present,
    /// The session is over (the cockpit quit, or it was killed).
    Ended(String),
    /// Who is at work in the session: sent to a `Kind::Watch` link when
    /// it attaches and whenever it changes.
    Presence(Presence),
}

/// The grid, as runs of cells that changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    /// Every cell is in the runs (a new window, a new size): the window
    /// starts from blank cells.
    pub full: bool,
    /// `(first cell index, the cells from there)`.
    pub runs: Vec<(u32, Vec<WireCell>)>,
    pub cursor: Option<(u16, u16)>,
    pub card: CardChange,
}

/// The card (`/setup`, an approval: `Host::set_card`), when it changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum CardChange {
    #[default]
    Same,
    Gone,
    Card(WireCard),
}

/// A card: its place, what it says and how it stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireCard {
    pub area: [u16; 4],
    pub cells: Vec<WireCell>,
    pub title: String,
    pub serial: u64,
    pub cursor: Option<u16>,
    pub field: Option<u16>,
    pub knight: crate::card::Knight,
    pub busy: bool,
    pub entrance: crate::card::Entrance,
}

/// One cell: its symbol, colours (`pack_color`) and modifier bits.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCell {
    pub symbol: String,
    pub fg: u32,
    pub bg: u32,
    pub underline: u32,
    pub modifier: u16,
}

impl WireCell {
    pub fn of(cell: &Cell) -> Self {
        Self {
            symbol: cell.symbol().to_string(),
            fg: pack_color(cell.fg),
            bg: pack_color(cell.bg),
            underline: pack_color(cell.underline_color),
            modifier: cell.modifier.bits(),
        }
    }

    pub fn cell(&self) -> Cell {
        let mut cell = Cell::default();
        cell.set_symbol(&self.symbol);
        cell.fg = unpack_color(self.fg);
        cell.bg = unpack_color(self.bg);
        cell.underline_color = unpack_color(self.underline);
        cell.modifier = Modifier::from_bits_truncate(self.modifier);
        cell
    }
}

const NAMED: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// A colour in one number: 0 reset, 1 to 16 the named sixteen, `0x100 | i`
/// an indexed one, `0x0100_0000 | rgb` a true colour.
pub fn pack_color(color: Color) -> u32 {
    match color {
        Color::Reset => 0,
        Color::Indexed(i) => 0x100 | u32::from(i),
        Color::Rgb(r, g, b) => 0x0100_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        named => NAMED.iter().position(|c| *c == named).map_or(0, |i| i as u32 + 1),
    }
}

pub fn unpack_color(packed: u32) -> Color {
    match packed {
        0 => Color::Reset,
        1..=16 => NAMED[packed as usize - 1],
        0x100..=0x1ff => Color::Indexed((packed & 0xff) as u8),
        _ if packed & 0x0100_0000 != 0 => {
            Color::Rgb((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
        }
        _ => Color::Reset,
    }
}

/// The runs of `now` that differ from `before` (same length): changes
/// closer than a few cells join one run, which is shorter on the wire than
/// two.
pub fn diff(before: &[Cell], now: &[Cell]) -> Vec<(u32, Vec<WireCell>)> {
    const JOIN: usize = 4;
    let mut runs: Vec<(u32, Vec<WireCell>)> = Vec::new();
    let mut i = 0;
    let n = now.len().min(before.len());
    while i < n {
        if before[i] == now[i] {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 1;
        let mut gap = 0;
        let mut j = i + 1;
        while j < n {
            if before[j] != now[j] {
                end = j + 1;
                gap = 0;
            } else {
                gap += 1;
                if gap > JOIN {
                    break;
                }
            }
            j += 1;
        }
        runs.push((start as u32, now[start..end].iter().map(WireCell::of).collect()));
        i = end;
    }
    runs
}

/// Every cell as one run.
pub fn whole(cells: &[Cell]) -> Vec<(u32, Vec<WireCell>)> {
    if cells.is_empty() {
        return Vec::new();
    }
    vec![(0, cells.iter().map(WireCell::of).collect())]
}

/// A blob's name: two independent multiply-rotate lanes over its words and
/// its length. Not a defence against an adversary; a name for what the
/// session itself made.
pub fn hash(bytes: &[u8]) -> Hash {
    const A: u64 = 0x9E37_79B9_7F4A_7C15;
    const B: u64 = 0xC2B2_AE3D_27D4_EB4F;
    let (mut a, mut b) = (0x243F_6A88_85A3_08D3_u64, 0x1319_8A2E_0370_7344_u64);
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let w = u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8]));
        a = (a ^ w).wrapping_mul(A).rotate_left(29);
        b = (b.rotate_left(17) ^ w).wrapping_mul(B);
    }
    let mut tail = [0u8; 8];
    let rest = chunks.remainder();
    tail[..rest.len()].copy_from_slice(rest);
    let w = u64::from_le_bytes(tail) ^ (bytes.len() as u64).rotate_left(32);
    a = (a ^ w).wrapping_mul(A);
    b = (b ^ w).wrapping_mul(B);
    let fin = |mut x: u64| {
        x ^= x >> 33;
        x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        x ^= x >> 33;
        x = x.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        x ^ (x >> 33)
    };
    [fin(a ^ b.rotate_left(7)), fin(b ^ a.rotate_left(41))]
}

fn options() -> impl bincode::Options {
    bincode::DefaultOptions::new().with_limit(MAX_FRAME as u64)
}

/// A message as bincode.
pub fn encode<T: Serialize>(message: &T) -> io::Result<Vec<u8>> {
    options().serialize(message).map_err(io::Error::other)
}

/// A message from bincode.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> io::Result<T> {
    options().deserialize(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Deflate `bytes` at the fastest level; `None` when that saves under a
/// quarter (already compressed, or noise).
pub fn deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    use flate2::write::DeflateEncoder;
    let mut encoder = DeflateEncoder::new(Vec::with_capacity(bytes.len() / 4), flate2::Compression::fast());
    encoder.write_all(bytes).ok()?;
    let packed = encoder.finish().ok()?;
    (packed.len() < bytes.len() - bytes.len() / 4).then_some(packed)
}

/// What turns `old` into `new` (same length): their XOR, deflated when
/// that pays, as `(deflated, bytes)`.
pub fn delta(old: &[u8], new: &[u8]) -> (bool, Vec<u8>) {
    let xor: Vec<u8> = old.iter().zip(new).map(|(a, b)| a ^ b).collect();
    match deflate(&xor) {
        Some(packed) => (true, packed),
        None => (false, xor),
    }
}

/// `old` changed by a [`delta`]; `None` when it does not fit.
pub fn undelta(old: &[u8], deflated: bool, bytes: &[u8]) -> Option<Vec<u8>> {
    let xor = if deflated { inflate(bytes).ok()? } else { bytes.to_vec() };
    (xor.len() == old.len()).then(|| old.iter().zip(&xor).map(|(a, b)| a ^ b).collect())
}

/// Inflate what [`deflate`] made, refusing anything past [`MAX_FRAME`].
pub fn inflate(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len() * 4);
    flate2::read::DeflateDecoder::new(bytes)
        .take(MAX_FRAME as u64 + 1)
        .read_to_end(&mut out)?;
    if out.len() > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "an inflated frame past the limit"));
    }
    Ok(out)
}

/// One frame's bytes, header included: what a writer puts on the link.
pub fn frame<T: Serialize>(message: &T) -> io::Result<Arc<[u8]>> {
    let body = encode(message)?;
    let (flags, body) = match (body.len() >= DEFLATE_FROM).then(|| deflate(&body)).flatten() {
        Some(packed) => (1u8, packed),
        None => (0u8, body),
    };
    let mut out = Vec::with_capacity(body.len() + 5);
    out.extend_from_slice(&(body.len() as u32 + 1).to_le_bytes());
    out.push(flags);
    out.extend_from_slice(&body);
    Ok(out.into())
}

/// Write one message as a frame.
pub fn write<T: Serialize>(out: &mut impl Write, message: &T) -> io::Result<()> {
    out.write_all(&frame(message)?)?;
    out.flush()
}

/// Read one frame's message. `Ok(None)` at a clean end of the stream.
pub fn read<T: for<'a> Deserialize<'a>>(input: &mut impl Read) -> io::Result<Option<T>> {
    let mut header = [0u8; 4];
    match input.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(header) as usize;
    if len == 0 || len > MAX_FRAME + 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a frame of {len} bytes")));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    let message = match body[0] {
        0 => decode(&body[1..])?,
        1 => decode(&inflate(&body[1..])?)?,
        flags => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("frame flags {flags}")));
        }
    };
    Ok(Some(message))
}

/// Read until [`MAGIC`], keeping what came before it (a launcher's error,
/// a login banner) to say if it never comes. `Err` carries that text.
pub fn await_magic(input: &mut impl Read) -> Result<(), String> {
    let mut seen: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(0) => return Err(String::from_utf8_lossy(&seen).trim().to_string()),
            Ok(_) => {
                seen.push(byte[0]);
                if seen.ends_with(MAGIC) {
                    return Ok(());
                }
                if seen.len() > MAX_PREAMBLE {
                    return Err("no session answered (too much output before it)".into());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn cell(symbol: &'static str, fg: Color, bg: Color) -> Cell {
        let mut cell = Cell::new(symbol);
        cell.fg = fg;
        cell.bg = bg;
        cell
    }

    #[test]
    fn every_colour_survives_packing() {
        let mut colours = vec![Color::Reset, Color::Rgb(0, 0, 0), Color::Rgb(255, 128, 1)];
        colours.extend(NAMED);
        colours.extend((0..=255).map(Color::Indexed));
        for colour in colours {
            assert_eq!(unpack_color(pack_color(colour)), colour, "{colour:?}");
        }
    }

    #[test]
    fn a_cell_survives_the_wire() {
        let mut c = cell("⣿", Color::Rgb(10, 20, 30), Color::Indexed(17));
        c.modifier = Modifier::BOLD | Modifier::UNDERLINED;
        c.underline_color = Color::LightCyan;
        assert_eq!(WireCell::of(&c).cell(), c);
        let wide = cell("界", Color::Reset, Color::Reset);
        assert_eq!(WireCell::of(&wide).cell(), wide);
    }

    #[test]
    fn the_diff_rebuilds_the_grid() {
        let before: Vec<Cell> = (0..400).map(|i| cell(if i % 3 == 0 { "a" } else { " " }, Color::Reset, Color::Reset)).collect();
        let mut now = before.clone();
        for i in [0usize, 1, 7, 9, 200, 399] {
            now[i] = cell("x", Color::Red, Color::Rgb(1, 2, 3));
        }
        let runs = diff(&before, &now);
        assert!(runs.len() < 6, "near changes join: {}", runs.len());
        let mut rebuilt = before.clone();
        for (start, cells) in &runs {
            for (k, c) in cells.iter().enumerate() {
                rebuilt[*start as usize + k] = c.cell();
            }
        }
        assert_eq!(rebuilt, now);
        assert!(diff(&now, &now).is_empty());
    }

    #[test]
    fn messages_round_trip_through_frames() {
        let key = Event::Key(KeyEvent {
            code: KeyCode::Char('é'),
            modifiers: KeyModifiers::SHIFT | KeyModifiers::ALT,
            kind: KeyEventKind::Release,
            state: KeyEventState::KEYPAD,
        });
        let size = GridSize { cols: 120, rows: 40, width_px: 1200, height_px: 800, cell_width: 10, cell_height: 20 };
        let ups = vec![
            Up::Hello { version: VERSION, kind: Kind::Window, size, have: vec![[1, 2]], who: "workstation".into() },
            Up::Input(key),
            Up::Resize(size),
            Up::Showcase(Feedback { serial: 3, closed: true, clicks: 1, copies: 0, steps: -1, seek: Some(0.5) }),
            Up::Kill,
        ];
        let mut link = Vec::new();
        for up in &ups {
            write(&mut link, up).unwrap();
        }
        let mut reader = &link[..];
        for up in &ups {
            let back: Up = read(&mut reader).unwrap().unwrap();
            assert_eq!(format!("{back:?}"), format!("{up:?}"));
        }
        assert!(read::<Up>(&mut reader).unwrap().is_none(), "a clean end");
    }

    #[test]
    fn a_large_frame_is_deflated_and_comes_back() {
        let cells: Vec<Cell> = (0..20_000).map(|i| cell(if i % 2 == 0 { "─" } else { "a" }, Color::Rgb(9, 9, 9), Color::Reset)).collect();
        let grid = Down::Grid(Grid { cols: 200, rows: 100, full: true, runs: whole(&cells), cursor: Some((3, 4)), card: CardChange::Gone });
        let bytes = frame(&grid).unwrap();
        assert_eq!(bytes[4], 1, "deflated");
        assert!(bytes.len() < encode(&grid).unwrap().len() / 4);
        let back: Down = read(&mut &bytes[..]).unwrap().unwrap();
        assert_eq!(format!("{back:?}"), format!("{grid:?}"));
    }

    #[test]
    fn junk_before_the_magic_is_skipped_and_reported_without_it() {
        let mut link = b"building cockpit...\nwarning: x\n".to_vec();
        link.extend_from_slice(MAGIC);
        write(&mut link, &Down::Present).unwrap();
        let mut reader = &link[..];
        await_magic(&mut reader).unwrap();
        assert!(matches!(read::<Down>(&mut reader).unwrap(), Some(Down::Present)));
        let mut old = &b"angelX: unknown or trailing interactive argument\n"[..];
        assert_eq!(await_magic(&mut old).unwrap_err(), "angelX: unknown or trailing interactive argument");
    }

    #[test]
    fn a_delta_turns_the_old_picture_into_the_new() {
        let old: Vec<u8> = (0..384 * 224 * 4).map(|i| (i % 7) as u8).collect();
        let mut new = old.clone();
        for i in (1000..1400).step_by(4) {
            new[i] = 255;
        }
        let (deflated, bytes) = delta(&old, &new);
        assert!(deflated && bytes.len() < old.len() / 100, "a few pixels cost a few bytes: {}", bytes.len());
        assert_eq!(undelta(&old, deflated, &bytes), Some(new));
        assert_eq!(undelta(&old[..8], deflated, &bytes), None);
    }

    #[test]
    fn hashes_tell_content_apart() {
        let a = vec![0u8; 4096];
        let mut b = a.clone();
        b[4095] = 1;
        assert_ne!(hash(&a), hash(&b));
        assert_ne!(hash(&a), hash(&a[..4095]));
        assert_eq!(hash(&a), hash(&a.clone()));
    }

    #[test]
    fn a_hostile_length_is_refused() {
        let mut bad = (u32::MAX).to_le_bytes().to_vec();
        bad.push(0);
        assert!(read::<Down>(&mut &bad[..]).is_err());
    }
}
