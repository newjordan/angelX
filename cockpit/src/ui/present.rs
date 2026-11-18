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

#[allow(dead_code)]
pub(crate) enum Showing {
    Picture { key: u64, width: u32, height: u32, rgba: Vec<u8> },
    #[cfg(feature = "glass")]
    Layers(Vec<angel_glass::Layer>),
    #[cfg(feature = "glass")]
    Stage(Box<angel_glass::Stage>),
    Video {
        key: u64,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Mesh { scene: String },
}

#[allow(dead_code)]
pub(crate) fn show(showing: &Showing, anchor: Rect, buffer: &mut Buffer) -> bool {
    match showing {
        Showing::Picture { key, width, height, rgba } => {
            picture(*key, (*width, *height), anchor, buffer, || rgba.clone())
        }
        #[cfg(feature = "glass")]
        Showing::Layers(layers) => show_layers(layers),
        #[cfg(feature = "glass")]
        Showing::Stage(stage) => show_stage(angel_glass::Stage::clone(stage), anchor, buffer),
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
    let shown = present_picture(key, size, anchor, buffer, None, fresh, pixels);
    #[cfg(feature = "glass")]
    if !shown {
        VIDEO_SEQUENCES.lock().unwrap().remove(&key);
    }
    shown
}

#[cfg(feature = "glass")]
static VIDEO_SEQUENCES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<u64, u64>>,
> = std::sync::LazyLock::new(Default::default);

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

pub(crate) fn picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    present_picture(key, size, anchor, buffer, None, false, pixels)
}

pub(crate) fn picture_in_place(
    slot: u64,
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    present_picture(key, size, anchor, buffer, Some(slot), false, pixels)
}

pub(crate) fn stream_picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    pixels: impl FnOnce() -> Vec<u8>,
) -> bool {
    present_picture(key, size, anchor, buffer, None, true, pixels)
}

fn present_picture(
    key: u64,
    size: (u32, u32),
    anchor: Rect,
    buffer: &mut Buffer,
    slot: Option<u64>,
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
        let placed = fit_span(anchor, cell_px, size);
        let (id, upload) = {
            let mut cache = CACHE.lock().unwrap();
            match slot {
                Some(slot) => cache.claim_in_slot(slot, key, size, (placed.width, placed.height)),
                None => cache.claim(key, size, (placed.width, placed.height), stream),
            }
        };
        if upload && !send(host, id, size, (placed.width, placed.height), pixels()) {
            let mut cache = CACHE.lock().unwrap();
            match slot {
                Some(slot) => cache.forget_slot(slot),
                None => cache.forget(key),
            }
            return false;
        }
        blank(buffer, anchor);
        crate::ui::dots::protocol::placeholders(
            id,
            ratatui::layout::Size::new(placed.width, placed.height),
            placed,
            buffer,
        );
        true
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (key, size, anchor, buffer, slot, stream, pixels);
        false
    }
}

pub(crate) fn upload(key: u64, size: (u32, u32), span: (u16, u16), rgba: Vec<u8>) -> Option<u32> {
    store(key, size, span, rgba, false)
}

pub(crate) fn store(
    key: u64,
    size: (u32, u32),
    span: (u16, u16),
    rgba: Vec<u8>,
    stream: bool,
) -> Option<u32> {
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
        Some(id)
    }
    #[cfg(not(feature = "glass"))]
    {
        let _ = (key, size, span, rgba, stream);
        None
    }
}

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

#[cfg(feature = "glass")]
const GROUP: u32 = 0x4D49_4E49;

#[cfg(feature = "glass")]
const STAGE_ID: u32 = 0x00B1_0B1C;

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
fn show_layers(layers: &[angel_glass::Layer]) -> bool {
    let Some(host) = crate::ui::glass::host() else {
        return false;
    };
    crate::ui::glass::show_group(host, GROUP, layers.to_vec());
    true
}

#[cfg(feature = "glass")]
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

#[cfg(feature = "glass")]
#[derive(Default)]
struct StageState {
    drawn: bool,
    shown: bool,
}

#[cfg(feature = "glass")]
static STAGE_STATE: std::sync::LazyLock<std::sync::Mutex<StageState>> =
    std::sync::LazyLock::new(Default::default);

#[cfg(feature = "glass")]
const PICTURE_BASE: u32 = 0x00F1_0000;
#[cfg(feature = "glass")]
const PICTURE_SPAN: u32 = 0x4000;
#[cfg(feature = "glass")]
const CACHE_KEEP: usize = 64;

#[cfg(feature = "glass")]
#[derive(Default)]
struct Cache {
    by_key: std::collections::HashMap<u64, (u32, (u32, u32), (u16, u16))>,
    order: std::collections::VecDeque<u64>,
    next: u32,
    slots: [Option<Slot>; SLOT_KEEP],
    clock: u64,
}

#[cfg(feature = "glass")]
const SLOT_BASE: u32 = 0x00F5_0000;
#[cfg(feature = "glass")]
const SLOT_KEEP: usize = 8;

#[cfg(feature = "glass")]
#[derive(Clone, Copy)]
struct Slot {
    name: u64,
    key: u64,
    size: (u32, u32),
    span: (u16, u16),
    used: u64,
}

#[cfg(feature = "glass")]
impl Cache {
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

    fn forget(&mut self, key: u64) {
        self.by_key.remove(&key);
        self.order.retain(|held| *held != key);
    }

    fn claim_in_slot(&mut self, name: u64, key: u64, size: (u32, u32), span: (u16, u16)) -> (u32, bool) {
        self.clock += 1;
        let at = self
            .slots
            .iter()
            .position(|s| s.is_some_and(|s| s.name == name))
            .or_else(|| self.slots.iter().position(Option::is_none))
            .unwrap_or_else(|| {
                (0..SLOT_KEEP)
                    .min_by_key(|&i| self.slots[i].map_or(0, |s| s.used))
                    .unwrap_or(0)
            });
        let held = self.slots[at].filter(|s| s.name == name);
        let upload = held.is_none_or(|s| s.key != key || s.size != size || s.span != span);
        self.slots[at] = Some(Slot { name, key, size, span, used: self.clock });
        (SLOT_BASE + 4 * at as u32, upload)
    }

    fn forget_slot(&mut self, name: u64) {
        for slot in &mut self.slots {
            if slot.is_some_and(|s| s.name == name) {
                *slot = None;
            }
        }
    }
}

#[cfg(feature = "glass")]
static CACHE: std::sync::LazyLock<std::sync::Mutex<Cache>> =
    std::sync::LazyLock::new(Default::default);

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
        let cell = (10, 20);
        assert_eq!(fit_span(anchor, cell, (200, 200)), Rect::new(10, 5, 40, 20));
        assert_eq!(fit_span(anchor, cell, (400, 100)), Rect::new(10, 12, 40, 5));
        assert_eq!(fit_span(anchor, cell, (100, 400)), Rect::new(25, 5, 10, 20));
        assert_eq!(fit_span(anchor, cell, (400, 400)), Rect::new(10, 5, 40, 20));
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
        let asked = std::cell::Cell::new(false);
        assert!(!picture(7, (4, 4), anchor, &mut buffer, || {
            asked.set(true);
            vec![0; 64]
        }));
        assert!(!asked.get(), "pixels are only made for the window");
        assert_eq!(upload(7, (2, 2), (1, 1), vec![0; 16]), None);
        assert_eq!(store(7, (2, 2), (1, 1), vec![0; 16], true), None);
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
    fn a_picture_that_changes_in_place_keeps_its_window_picture() {
        let mut cache = Cache::default();
        let (id, up) = cache.claim_in_slot(9, 100, (320, 180), (40, 12));
        assert!(up, "first sight uploads");
        assert_eq!(cache.claim_in_slot(9, 100, (320, 180), (40, 12)), (id, false), "same content held");
        for key in 101..140 {
            assert_eq!(
                cache.claim_in_slot(9, key, (320, 180), (40, 12)),
                (id, true),
                "new content goes into the same picture, again and again"
            );
        }
        assert_eq!(cache.claim_in_slot(9, 139, (320, 180), (40, 12)), (id, false), "and settles");
        assert_eq!(cache.claim_in_slot(9, 139, (640, 360), (40, 12)), (id, true), "a new size uploads");
        assert_eq!(cache.claim_in_slot(9, 139, (640, 360), (60, 12)), (id, true), "a new span uploads");
        let (other, up) = cache.claim_in_slot(10, 100, (320, 180), (40, 12));
        assert!(up && other != id);
        let (content, _) = cache.claim(100, (320, 180), (40, 12), false);
        assert!(content != id && content != other, "content ids never meet slot ids");
        assert!((SLOT_BASE..SLOT_BASE + 4 * SLOT_KEEP as u32).contains(&id));
        assert!(id < 0x0100_0000 && id % 4 == 0 && id < 0x00FA_57A4, "ids survive the placeholder colour");
        cache.forget_slot(10);
        let (_, up) = cache.claim_in_slot(10, 100, (320, 180), (40, 12));
        assert!(up);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn slots_wrap_over_the_least_recently_shown() {
        let mut cache = Cache::default();
        let ids: Vec<u32> = (0..SLOT_KEEP as u64).map(|n| cache.claim_in_slot(n, n, (4, 4), (1, 1)).0).collect();
        let mut distinct = ids.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), SLOT_KEEP, "each slot has its own id");
        cache.claim_in_slot(0, 0, (4, 4), (1, 1));
        let (taken, up) = cache.claim_in_slot(99, 1, (4, 4), (1, 1));
        assert_eq!(taken, ids[1]);
        assert!(up);
        assert!(cache.claim_in_slot(1, 1, (4, 4), (1, 1)).1);
    }

    #[cfg(feature = "glass")]
    #[test]
    fn picture_ids_survive_placeholder_encoding() {
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

    #[test]
    fn a_stage_does_not_inflate_showing() {
        assert!(std::mem::size_of::<Showing>() < 128);
    }
}
