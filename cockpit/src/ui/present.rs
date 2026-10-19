//! The mini-viz presentation surface: one door from cockpit content to
//! the glass window.
//!
//! Everything that used to be composed into the lower-right pane's
//! cells — the world, stills, documents, and in time video and 3D
//! scenes — is handed here as a [`Showing`], and the surface presents
//! it on glass at the window's own density. The pane's rect is the
//! anchor: anchored kinds (picture, stage) own that rect's paper and
//! are fit inside it; declared layers carry absolute placements and
//! may break out of the anchor, floating over the terminal, which
//! stays the background the whole time.
//!
//! Rust stays the bedrock: ids, the upload cache, the fit geometry
//! and the frame sweep live here. What is shown is the river — data
//! handed over per frame, in kinds the window already knows
//! (pictures, layers, stages). New media becomes a new kind, not a
//! new emitter with its own id range and lifecycle.
//!
//! Outside the window every entry point returns `false` and the
//! caller paints cells as before — plain terminals keep the pane
//! exactly as it was. A kind glass cannot present yet (`Mesh`)
//! also returns `false`: declared in the API so producers can
//! be written against it, never shown as something it is not.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// One frame of mini-viz content, handed to [`show`].
#[allow(dead_code)] // Mesh (and Stage/Layers producers) land in later slices.
pub(crate) enum Showing {
    /// Pixels, straight (not premultiplied) RGBA. `key` identifies
    /// the content: the same key at the same size is uploaded once.
    /// Per-frame producers whose pixels are expensive to make should
    /// prefer [`picture`], which only asks for them on a cache miss.
    Picture { key: u64, width: u32, height: u32, rgba: Vec<u8> },
    /// Declared window layers, in absolute cell placements. Nothing
    /// is clipped to the anchor: a layer may break out of it and
    /// float over the terminal. Layer ids are the caller's, unique
    /// within this showing.
    #[cfg(feature = "glass")]
    Layers(Vec<angel_glass::Layer>),
    /// A glass stage: layers in the stage's own pixels, drawn by the
    /// window into its own texture at the window's density. It is
    /// presented under the mini-viz's stage id in the anchor's
    /// cells; the handed-over `id`, `cols` and `rows` are replaced.
    #[cfg(feature = "glass")]
    Stage(angel_glass::Stage),
    /// One frame of a video stream: pixels like [`Showing::Picture`],
    /// but uploaded on every showing — frames change, so Picture's
    /// upload-once would freeze the first one. `key` identifies the
    /// stream (the caller hashes its source), so its frames keep one
    /// window picture, overwritten in place at the same size.
    Video {
        key: u64,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// A 3D scene by name/path. Declared, not yet presentable.
    Mesh { scene: String },
}

/// Present `showing` in the mini-viz anchored at `anchor`. `true`
/// when the window took it; `false` outside the window, or for a
/// kind glass cannot present yet — the caller then paints cells.
#[allow(dead_code)] // First producers (stills, layers) land in later slices.
pub(crate) fn show(showing: &Showing, anchor: Rect, buffer: &mut Buffer) -> bool {
    match showing {
        Showing::Picture { key, width, height, rgba } => {
            picture(*key, (*width, *height), anchor, buffer, || rgba.clone())
        }
        #[cfg(feature = "glass")]
        Showing::Layers(layers) => show_layers(layers),
        #[cfg(feature = "glass")]
        Showing::Stage(stage) => show_stage(stage.clone(), anchor, buffer),
        Showing::Video {
            key,
            width,
            height,
            rgba,
        } => stream_picture(*key, (*width, *height), anchor, buffer, || rgba.clone()),
        Showing::Mesh { scene } => {
            let _ = scene;
            false
        }
    }
}

/// Present a decoded video frame of `source` at decode `sequence` and
/// `size`, fit inside `anchor`. One held picture per source, like
/// [`Showing::Video`]: a new sequence overwrites it in place, and a
/// repaint of the same frame uploads nothing. `pixels` runs only for
/// a new frame inside the glass window.
pub(crate) fn video(
    source: &str,
    sequence: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ("video", source).hash(&mut hasher);
    let key = hasher.finish();
    #[cfg(feature = "glass")]
    let fresh = {
        let mut seen = VIDEO_SEQUENCES.lock().unwrap();
        if seen.len() > 64 && !seen.contains_key(&key) {
            seen.clear();
        }
        seen.insert(key, sequence) != Some(sequence)
    };
    #[cfg(not(feature = "glass"))]
    let fresh = {
        let _ = sequence;
        true
    };
    let shown = present_picture(key, size, anchor, buffer, fresh, pixels);
    #[cfg(feature = "glass")]
    if !shown {
        VIDEO_SEQUENCES.lock().unwrap().remove(&key);
    }
    shown
}

/// The last decode sequence [`video`] uploaded, per source key.
#[cfg(feature = "glass")]
static VIDEO_SEQUENCES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<u64, u64>>,
> = std::sync::LazyLock::new(Default::default);

/// Present a scrolled document (`receipt` + `text` at `scroll`) as a
/// high-density rasterized page inside `anchor`. Rasterization via `swash` in
/// `angel-glass` runs only on a cache miss in the glass window; outside the
/// window (or when no font is available) this returns `false` and the caller
/// paints cells as before.
pub(crate) fn document(
    receipt: &str,
    text: &str,
    scroll: u16,
    anchor: Rect,
    buffer: &mut Buffer,
) -> bool {
    #[cfg(feature = "glass")]
    {
        if crate::ui::glass::host().is_none() {
            return false;
        }
        let Some(cell_px) = crate::ui::glass::cell_pixels() else {
            return false;
        };
        if anchor.width == 0 || anchor.height == 0 || cell_px.0 == 0 || cell_px.1 == 0 {
            return false;
        }
        let size = (
            u32::from(anchor.width) * u32::from(cell_px.0),
            u32::from(anchor.height) * u32::from(cell_px.1),
        );
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ("document", receipt, text, scroll, anchor.width, anchor.height, cell_px)
            .hash(&mut hasher);
        picture(hasher.finish(), size, anchor, buffer, || {
            angel_glass::render_page(
                receipt,
                text,
                scroll,
                anchor.width,
                anchor.height,
                cell_px,
            )
            .map(|(_, _, rgba)| rgba)
            .unwrap_or_default()
        })
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (receipt, text, scroll, anchor, buffer);
        false
    }
}

/// Present pixels identified by `key` at `size`, fit inside `anchor`.
/// `pixels` runs only when the surface does not already hold this
/// key at this size, and never outside the window — a terminal
/// caller pays nothing for asking. The bytes must be straight RGBA
/// of exactly `size.0 * size.1 * 4`; anything else is refused
/// (returns `false`) with the anchor untouched.
pub(crate) fn picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    present_picture(key, size, anchor, buffer, false, pixels)
}

/// [`picture`] for a stream whose frames change: the pixels upload
/// on every showing into the stream's held picture (see
/// [`Showing::Video`]). `pixels` still runs only inside the window.
pub(crate) fn stream_picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    present_picture(key, size, anchor, buffer, true, pixels)
}

fn present_picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    stream: bool,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    #[cfg(feature = "glass")]
    {
        let Some(host) = crate::ui::glass::host() else {
            return false;
        };
        let Some(cell_px) = crate::ui::glass::cell_pixels() else {
            return false;
        };
        if size.0 == 0 || size.1 == 0 || anchor.width == 0 || anchor.height == 0 {
            return false;
        }
        // Placed by placeholder cells, like every pane picture: the
        // window draws the texture where they sit, and cell overlays
        // (captions, dancers) may still replace individual cells
        // over it, exactly as they do over the pane today.
        let placed = fit_span(anchor, cell_px, size);
        let (id, upload) = {
            let mut cache = CACHE.lock().unwrap();
            cache.claim(key, size, (placed.width, placed.height), stream)
        };
        if upload && !send(host, id, size, (placed.width, placed.height), pixels()) {
            CACHE.lock().unwrap().forget(key);
            return false;
        }
        blank(buffer, anchor);
        crate::ui::dots::protocol::placeholders(
            id,
            ratatui::layout::Size::new(placed.width, placed.height),
            placed,
            buffer,
        );
        return true;
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (key, size, anchor, buffer, stream, pixels);
        false
    }
}

/// Store pixels under content `key` at `size` for placeholder
/// `span`, once: the window picture id, or `None` outside the
/// window or for bytes that are not exactly `size` of straight
/// RGBA. For producers that place their own placeholder cells (the
/// viewer's stills, which keep their inspector); [`picture`] is
/// this plus the mini-viz's own placement. One cache and one id
/// band serve both, so the same still shown twice uploads once.
pub(crate) fn upload(key: u64, size: (u32, u32), span: (u16, u16), rgba: Vec<u8>) -> Option<u32> {
    store(key, size, span, rgba, false)
}

/// [`upload`] for one frame of a stream (the viewer's game and video
/// frames): every call uploads into the stream's held picture, so
/// the window overwrites its texture in place instead of making a
/// new one per frame — the decode sequence lives in the frames
/// themselves, not in the key.
pub(crate) fn upload_stream(
    key: u64,
    size: (u32, u32),
    span: (u16, u16),
    rgba: Vec<u8>,
) -> Option<u32> {
    store(key, size, span, rgba, true)
}

fn store(key: u64, size: (u32, u32), span: (u16, u16), rgba: Vec<u8>, stream: bool) -> Option<u32> {
    #[cfg(feature = "glass")]
    {
        let host = crate::ui::glass::host()?;
        if size.0 == 0 || size.1 == 0 || span.0 == 0 || span.1 == 0 {
            return None;
        }
        let (id, upload) = {
            let mut cache = CACHE.lock().unwrap();
            cache.claim(key, size, span, stream)
        };
        if upload && !send(host, id, size, span, rgba) {
            CACHE.lock().unwrap().forget(key);
            return None;
        }
        return Some(id);
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (key, size, span, rgba, stream);
        None
    }
}

/// Present `lines` as one rasterized page in `anchor`: the text is
/// rendered to pixels by the glass crate's own font stack
/// (`angel_glass::page`, so the cockpit carries no second one) at
/// the window's cell density, then shown as a [`picture`] under
/// `key` — the caller hashes the visible content, so an unchanged
/// page uploads once and only a scrolled or edited one re-renders,
/// and only inside the window (the raster closure never runs for a
/// terminal). `fg` is the ink; the paper is black, like the pane's.
pub(crate) fn document_page(
    key: u64,
    lines: Vec<String>,
    anchor: Rect,
    buffer: &mut Buffer,
    fg: [u8; 3],
) -> bool {
    #[cfg(feature = "glass")]
    {
        let Some(cell_px) = crate::ui::glass::cell_pixels() else {
            return false;
        };
        let Some(size) = with_pager(cell_px.1, |pager| pager.size(&lines)).flatten() else {
            return false;
        };
        picture(key, size, anchor, buffer, move || {
            with_pager(cell_px.1, move |pager| {
                pager
                    .render(&lines, angel_glass::page::PLAIN, fg, [0, 0, 0, 255])
                    .map(|(_, _, rgba)| rgba)
            })
            .flatten()
            .unwrap_or_default()
        })
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (key, lines, anchor, buffer, fg);
        false
    }
}

/// The presentation surface's page rasterizer: one `Pager` (a loaded
/// font stack; loading runs fontconfig), reloaded when the window's
/// cell height changes. `None` when no font answers.
#[cfg(feature = "glass")]
pub(crate) fn with_pager<R>(
    cell_height: u16,
    f: impl FnOnce(&mut angel_glass::page::Pager) -> R,
) -> Option<R> {
    struct Held {
        pager: angel_glass::page::Pager,
        cell_height: u16,
    }
    static PAGER: std::sync::LazyLock<std::sync::Mutex<Option<Held>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(None));
    let mut held = PAGER.lock().unwrap();
    if held.as_ref().is_none_or(|h| h.cell_height != cell_height) {
        let family = angel_glass::Options::from_environment().font_family;
        *held = angel_glass::page::Pager::load(&family, u32::from(cell_height))
            .map(|pager| Held { pager, cell_height });
    }
    held.as_mut().map(|h| f(&mut h.pager))
}

/// One upload, bytes checked: straight RGBA of exactly `size`, or
/// nothing is sent and the caller falls back.
#[cfg(feature = "glass")]
fn send(
    host: &angel_glass::Host,
    id: u32,
    size: (u32, u32),
    span: (u16, u16),
    rgba: Vec<u8>,
) -> bool {
    let want = (size.0 as usize)
        .checked_mul(size.1 as usize)
        .and_then(|px| px.checked_mul(4));
    if want != Some(rgba.len()) {
        return false;
    }
    host.upload_picture(angel_glass::graphics::Picture {
        id,
        width: size.0,
        height: size.1,
        rgba,
        cols: span.0,
        rows: span.1,
    });
    true
}

/// After each drawn frame (called from `glass::frame_done`): the
/// mini-viz stage leaves the window on any frame that did not show
/// it. Layer groups are swept by the glass module's own bookkeeping.
pub(crate) fn frame_done() {
    #[cfg(feature = "glass")]
    if let Some(host) = crate::ui::glass::host() {
        let mut stage = STAGE_STATE.lock().unwrap();
        if stage.shown && !stage.drawn {
            host.remove_stage(STAGE_ID);
            stage.shown = false;
        }
        stage.drawn = false;
    }
}

/// The mini-viz layer group ("MINI"): one showing at a time, swept
/// with every other group when a frame stops drawing it.
#[cfg(feature = "glass")]
#[allow(dead_code)] // First Layers producer lands in a later slice.
const GROUP: u32 = 0x4D49_4E49;

/// The mini-viz stage's picture id: fits the placeholder's 24 bits
/// and is a multiple of four, which the viewer's own pictures never
/// are — the same discipline as the Delve's stage id, a different
/// number.
#[cfg(feature = "glass")]
const STAGE_ID: u32 = 0x00B1_0B1C;

/// Black paper under an anchored showing, as the pane's own cell
/// painting lays it: the presented pixels decide the rest.
#[cfg(feature = "glass")]
fn blank(buffer: &mut Buffer, area: Rect) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(" ").set_bg(ratatui::style::Color::Rgb(0, 0, 0));
            }
        }
    }
}

#[cfg(feature = "glass")]
#[allow(dead_code)] // First Layers producer lands in a later slice.
fn show_layers(layers: &[angel_glass::Layer]) -> bool {
    let Some(host) = crate::ui::glass::host() else {
        return false;
    };
    crate::ui::glass::show_group(host, GROUP, layers.to_vec());
    true
}

#[cfg(feature = "glass")]
#[allow(dead_code)] // First Stage producer lands in a later slice.
fn show_stage(mut stage: angel_glass::Stage, anchor: Rect, buffer: &mut Buffer) -> bool {
    let Some(host) = crate::ui::glass::host() else {
        return false;
    };
    if anchor.width == 0 || anchor.height == 0 || stage.size.0 == 0 || stage.size.1 == 0 {
        return false;
    }
    stage.id = STAGE_ID;
    stage.cols = anchor.width;
    stage.rows = anchor.height;
    host.set_stage(stage);
    {
        let mut state = STAGE_STATE.lock().unwrap();
        state.drawn = true;
        state.shown = true;
    }
    blank(buffer, anchor);
    crate::ui::dots::protocol::placeholders(
        STAGE_ID,
        ratatui::layout::Size::new(anchor.width, anchor.height),
        anchor,
        buffer,
    );
    true
}

/// Whether the mini-viz stage was drawn this frame, and is up at all.
#[cfg(feature = "glass")]
#[derive(Default)]
struct StageState {
    drawn: bool,
    shown: bool,
}

#[cfg(feature = "glass")]
static STAGE_STATE: std::sync::LazyLock<std::sync::Mutex<StageState>> =
    std::sync::LazyLock::new(Default::default);

/// Window pictures for presented pixels start here. Placeholder
/// cells carry a picture id in their foreground colour — 24 bits —
/// so unlike layer pictures (`LAYER_PICTURES` and up, carried in
/// instance data) these ids must fit under `0x0100_0000`. The band
/// sits past the viewer's stills (`0x00E0_0000..0x00F0_0000`) and
/// steps by four, which the viewer's dot ids (2 mod 4) never are;
/// the Delve's stage id lives further up the same band.
#[cfg(feature = "glass")]
const PICTURE_BASE: u32 = 0x00F1_0000;
/// Ids handed out before the counter wraps (each is 4 apart, so
/// the band ends at `0x00F4_FFFC`).
#[cfg(feature = "glass")]
const PICTURE_SPAN: u32 = 0x4000;
/// Contents whose uploads the surface remembers at once; older ones
/// are forgotten (and re-uploaded if shown again), never leaked.
#[cfg(feature = "glass")]
const CACHE_KEEP: usize = 64;

/// Which window picture holds each content key: its pixel size and
/// the cell span it was uploaded for (the placeholder grid the
/// window slices by — a resized anchor needs a fresh upload).
#[cfg(feature = "glass")]
#[derive(Default)]
struct Cache {
    by_key: std::collections::HashMap<u64, (u32, (u32, u32), (u16, u16))>,
    order: std::collections::VecDeque<u64>,
    next: u32,
}

#[cfg(feature = "glass")]
impl Cache {
    /// The picture id for `key`, and whether the caller must
    /// upload: a held key at the same size and span needs nothing;
    /// a new key, or either changed, uploads (again). A stream
    /// claim always uploads — its frames change under a stable key.
    fn claim(&mut self, key: u64, size: (u32, u32), span: (u16, u16), stream: bool) -> (u32, bool) {
        if let Some((id, held_size, held_span)) = self.by_key.get_mut(&key) {
            let upload = stream || *held_size != size || *held_span != span;
            *held_size = size;
            *held_span = span;
            return (*id, upload);
        }
        let id = PICTURE_BASE + 4 * (self.next % PICTURE_SPAN);
        self.next = self.next.wrapping_add(1);
        self.by_key.insert(key, (id, size, span));
        self.order.push_back(key);
        while self.order.len() > CACHE_KEEP {
            if let Some(old) = self.order.pop_front() {
                self.by_key.remove(&old);
            }
        }
        (id, true)
    }

    /// Drop a claim whose upload never landed, so the next showing
    /// tries again instead of placing cells over a missing picture.
    fn forget(&mut self, key: u64) {
        self.by_key.remove(&key);
        self.order.retain(|held| *held != key);
    }
}

#[cfg(feature = "glass")]
static CACHE: std::sync::LazyLock<std::sync::Mutex<Cache>> =
    std::sync::LazyLock::new(Default::default);

/// The cells a `pic`-pixel picture occupies inside `anchor` (cells
/// of `cell_px` window pixels each): aspect kept (contain) at the
/// window's pixel density, centered, in whole cells — placeholder
/// grids are whole cells by construction. The texture itself is
/// the picture's own pixels, so clarity comes from the pixels, not
/// from subdividing cells. Any zero in the inputs keeps the whole
/// anchor, and the caller has already refused to present.
#[cfg(any(feature = "glass", test))]
pub(crate) fn fit_span(anchor: Rect, cell_px: (u16, u16), pic: (u32, u32)) -> Rect {
    if cell_px.0 == 0 || cell_px.1 == 0 || pic.0 == 0 || pic.1 == 0 {
        return anchor;
    }
    let (cw, ch) = (f32::from(cell_px.0), f32::from(cell_px.1));
    let (aw, ah) = (f32::from(anchor.width) * cw, f32::from(anchor.height) * ch);
    let scale = (aw / pic.0 as f32).min(ah / pic.1 as f32);
    let cols = ((pic.0 as f32 * scale / cw).round() as u16).clamp(1, anchor.width);
    let rows = ((pic.1 as f32 * scale / ch).round() as u16).clamp(1, anchor.height);
    Rect {
        x: anchor.x + (anchor.width - cols) / 2,
        y: anchor.y + (anchor.height - rows) / 2,
        width: cols,
        height: rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_and_centers() {
        let anchor = Rect::new(10, 5, 40, 20);
        let cell = (10, 20); // anchor is 400x400 window pixels
        // Square picture: fills exactly.
        assert_eq!(fit_span(anchor, cell, (200, 200)), Rect::new(10, 5, 40, 20));
        // Wide picture: full width, letterboxed (100px at scale 1 is
        // 5 rows of 20px; (20-5)/2 rounds down to a 7-row top margin).
        assert_eq!(fit_span(anchor, cell, (400, 100)), Rect::new(10, 12, 40, 5));
        // Tall picture: full height, pillarboxed.
        assert_eq!(fit_span(anchor, cell, (100, 400)), Rect::new(25, 5, 10, 20));
        // A picture at the exact pixel size sits one to one.
        assert_eq!(fit_span(anchor, cell, (400, 400)), Rect::new(10, 5, 40, 20));
        // Degenerate inputs keep the whole anchor.
        assert_eq!(fit_span(anchor, (0, 20), (100, 100)), anchor);
        assert_eq!(fit_span(anchor, cell, (0, 100)), anchor);
    }

    #[test]
    fn unpresentable_kinds_and_a_missing_window_say_no() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 4));
        let anchor = Rect::new(0, 0, 10, 4);
        assert!(!document("receipt", "hello", 0, anchor, &mut buffer));
        assert!(!video("ride.mp4", 1, (2, 2), anchor, &mut buffer, || vec![0; 16]));
        assert!(!show(&Showing::Mesh { scene: "keep".into() }, anchor, &mut buffer));
        // A video frame outside the window is refused like a picture.
        assert!(!show(
            &Showing::Video {
                key: 1,
                width: 2,
                height: 2,
                rgba: vec![0; 16]
            },
            anchor,
            &mut buffer
        ));
        // No window in a test: a picture is refused and its pixels
        // are never asked for — the terminal path pays nothing.
        let asked = std::cell::Cell::new(false);
        assert!(!picture(7, (4, 4), anchor, &mut buffer, || {
            asked.set(true);
            vec![0; 64]
        }));
        assert!(!asked.get(), "pixels are only made for the window");
        // The stills' upload doors likewise need the window.
        assert_eq!(upload(7, (2, 2), (1, 1), vec![0; 16]), None);
        assert_eq!(upload_stream(7, (2, 2), (1, 1), vec![0; 16]), None);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn a_held_key_uploads_once_until_it_changes_or_is_forgotten() {
        let mut cache = Cache::default();
        let (id, up) = cache.claim(42, (10, 10), (4, 2), false);
        assert!(up, "first sight uploads");
        assert_eq!(
            cache.claim(42, (10, 10), (4, 2), false),
            (id, false),
            "same content held"
        );
        assert_eq!(
            cache.claim(42, (20, 10), (4, 2), false),
            (id, true),
            "new size re-uploads"
        );
        assert_eq!(
            cache.claim(42, (20, 10), (8, 2), false),
            (id, true),
            "new span re-uploads: the placeholder grid changed"
        );
        assert_eq!(
            cache.claim(42, (20, 10), (8, 2), false),
            (id, false),
            "and settles again"
        );
        cache.forget(42);
        let (_, up) = cache.claim(42, (20, 10), (8, 2), false);
        assert!(up, "a failed upload is retried, not remembered as held");
        let (other, up) = cache.claim(43, (10, 10), (4, 2), false);
        assert!(up && other != id, "another key, another picture");
        // Flood past the keep limit: key 42 is forgotten.
        for k in 100..100 + CACHE_KEEP as u64 {
            cache.claim(k, (1, 1), (1, 1), false);
        }
        let (_, up) = cache.claim(42, (20, 10), (8, 2), false);
        assert!(up, "forgotten content uploads again");
    }

    #[cfg(feature = "glass")]
    #[test]
    fn a_stream_key_keeps_its_picture_and_uploads_every_frame() {
        let mut cache = Cache::default();
        let (id, up) = cache.claim(7, (320, 180), (40, 12), true);
        assert!(up, "first frame uploads");
        for _ in 0..3 {
            assert_eq!(
                cache.claim(7, (320, 180), (40, 12), true),
                (id, true),
                "every stream frame uploads into the same picture"
            );
        }
        assert_eq!(
            cache.claim(7, (320, 180), (40, 12), false),
            (id, false),
            "a still claim on the held stream key still dedupes"
        );
        let (other, _) = cache.claim(8, (320, 180), (40, 12), true);
        assert_ne!(other, id, "another stream, another picture");
    }

    #[cfg(feature = "glass")]
    #[test]
    fn picture_ids_survive_placeholder_encoding() {
        // Placeholder cells carry the id in a foreground colour:
        // 24 bits survive, and the viewer's dot ids are 2 mod 4, so
        // presented pictures stay 0 mod 4, past its stills band and
        // short of the Delve's stage id (0x00FA_57A4).
        let mut cache = Cache::default();
        let mut seen = std::collections::HashSet::new();
        for k in 0..300u64 {
            let (id, _) = cache.claim(k, (1, 1), (1, 1), false);
            assert!(id < 0x0100_0000, "id {id:#x} survives the 24-bit colour");
            assert_eq!(id % 4, 0, "id {id:#x} never reads as a dot id");
            assert!(id >= 0x00F0_0000, "id {id:#x} clears the viewer's stills");
            assert!(seen.insert(id), "ids are distinct per content");
        }
        assert!(STAGE_ID < 0x0100_0000 && STAGE_ID % 4 == 0);
    }
}
