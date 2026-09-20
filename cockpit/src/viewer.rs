//! Inline image rendering via ratatui-image. `/show <path>` loads an image; the
//! cockpit renders it in the sidebar. `/hide` clears.
//!
//! Still inspection samples a single bounded decoded source on an off-thread worker.
//! Draw only presents completed protocols; portraits retain their own preview cache.
//!
//! Background imagery is intentionally outside this viewer. It renders only
//! terminal-native foreground media such as artifacts and portraits.

use crate::sandbox::process_owner::OwnedCommandExt;
use ratatui::{Frame, layout::Rect};
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};
use std::collections::{HashMap, VecDeque};
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

// Portrait and explicit `/show` previews stay bounded. Scryglass Still inspection
// bypasses this thumbnail cache and retains only its active decoded source.
// 120k pixels (~400×300) keeps decode/resize cheap while leaving a ≥2×
// supersample above the largest halfblock encode box below.
const MAX_INLINE_PREVIEW_PIXELS: u32 = 120_000;
const MAX_INLINE_PREVIEW_CELLS: Rect = Rect::new(0, 0, 192, 64);

/// Halfblock cells show exactly 1×2 pixels each. Encoding at 2×4 pixels per
/// cell keeps the protocol's cell footprint honest — a preview fills its pane
/// instead of shrinking to ~1/10 scale under the crate's arbitrary 10×20
/// physical-font guess — while the encoder's final pass becomes a clean 2×
/// supersampled downscale (built-in antialiasing) rather than a blocky
/// nearest-neighbor round trip.
const HALFBLOCK_FONT_SIZE: (u16, u16) = (2, 4);

fn halfblock_picker() -> Picker {
    // `from_fontsize` is deprecated in favor of stdio probing, which this
    // viewer deliberately avoids (see `Viewer::new`). It is the only
    // probe-free way to pick the halfblock cell density.
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(HALFBLOCK_FONT_SIZE.into());
    // `from_fontsize` may guess iTerm2 from env leftovers; this picker is
    // strictly the terminal-native fallback.
    picker.set_protocol_type(ProtocolType::Halfblocks);
    picker
}
const PREVIEW_CACHE_LIMIT: usize = 32;
const PORTRAIT_CACHE_LIMIT: usize = 24;

#[derive(Clone)]
struct PreviewCacheEntry {
    len: u64,
    modified: Option<SystemTime>,
    original: (u32, u32),
    preview: image::DynamicImage,
}

static PREVIEW_CACHE: OnceLock<Mutex<HashMap<PathBuf, PreviewCacheEntry>>> = OnceLock::new();

struct PreparedPreview {
    label: String,
    original: (u32, u32),
    preview_size: (u32, u32),
    protocol: Protocol,
}

struct PendingPreview {
    rx: mpsc::Receiver<Result<PreparedPreview, String>>,
}

#[derive(Clone, Debug, PartialEq)]
struct ArtifactKey {
    request_id: u64,
    source: crate::media::MediaSource,
    width: u16,
    height: u16,
    scene: Rect,
    font: (u16, u16),
    protocol: ProtocolType,
    layout_epoch: u64,
    view: crate::still_inspector::View,
}

impl ArtifactKey {
    /// A previous view is usable only on this exact source/request/geometry.
    fn same_scene(&self, other: &Self) -> bool {
        let mut prior = self.clone();
        prior.view = other.view;
        prior == *other
    }
}

/// Still must never use Picker's 10×20 guess for native pixel placement. This
/// local picker is also the encoder picker; no stdin probe or global mutation.
fn still_pixel_picker(protocol: ProtocolType, reported: Option<(u16, u16)>) -> Picker {
    if protocol != ProtocolType::Halfblocks
        && let Some(font) = reported.filter(|(w, h)| *w > 0 && *h > 0)
    {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize(font.into());
        picker.set_protocol_type(protocol);
        picker
    } else {
        halfblock_picker()
    }
}

fn terminal_cell_pixels() -> Option<(u16, u16)> {
    // Existing safe TIOCGWINSZ helper, not an escape query on shared stdin.
    let window = ratatui::crossterm::terminal::window_size().ok()?;
    reported_cell_pixels(window.width, window.height, window.columns, window.rows)
}

struct PreparedArtifact {
    decoded: Arc<image::DynamicImage>,
    protocol: Protocol,
}

struct PendingArtifact {
    key: ArtifactKey,
    rx: mpsc::Receiver<Result<PreparedArtifact, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VideoCacheKey {
    identity: crate::scryglass::VideoIdentity,
    width: u16,
    height: u16,
}

struct PendingVideo {
    key: VideoCacheKey,
    sequence: u64,
    rx: mpsc::Receiver<Result<Protocol, String>>,
}

#[derive(Clone)]
struct ShownImage {
    path: PathBuf,
    label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PortraitCacheKey {
    path: PathBuf,
    width: u16,
    height: u16,
    pose: Option<u8>,
}

struct PortraitCacheEntry {
    key: PortraitCacheKey,
    protocol: Protocol,
}

struct PendingPortrait {
    key: PortraitCacheKey,
    rx: mpsc::Receiver<Result<Protocol, String>>,
    prefetch: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DotFrameKey {
    scene: u64,
    geometry: crate::dot_canvas::DotGeometry,
    columns: u16,
    rows: u16,
    generation: u64,
}

impl DotFrameKey {
    fn same_surface(&self, other: &Self) -> bool {
        self.scene == other.scene
            && self.geometry == other.geometry
            && self.columns == other.columns
            && self.rows == other.rows
    }
}

struct DotFrame {
    key: DotFrameKey,
    protocol: crate::dot_protocol::DotProtocol,
}

struct PendingDots {
    key: DotFrameKey,
    rx: mpsc::Receiver<Result<crate::dot_protocol::DotProtocol, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PortalCacheKey {
    // Only live sprite scenes retain an earlier frame while encoding. Bind that
    // continuity to the same workspace/region, never another media surface.
    sequence: u64,
    width: u16,
    height: u16,
}

struct PortalCacheEntry {
    key: PortalCacheKey,
    protocol: Protocol,
}

struct PendingPortal {
    key: PortalCacheKey,
    rx: mpsc::Receiver<Result<Protocol, String>>,
}

/// One compose+encode unit of work for the resident room-plate worker.
type MapEncodeJob = Box<dyn FnOnce() + Send>;

/// Holds the terminal's graphics capability (the `Picker`) and the currently
/// shown image's protocol state.
pub struct Viewer {
    picker: Picker,
    artifact: Option<(ArtifactKey, Result<Protocol, String>)>,
    artifact_pending: Option<PendingArtifact>,
    still_layout_epoch: u64,
    still_protocol: Option<ProtocolType>,
    artifact_identity: Option<(crate::media::MediaSource, u64)>,
    artifact_decoded: Option<Arc<image::DynamicImage>>,
    pub(crate) inspector: crate::still_inspector::Inspector,
    video: Option<(VideoCacheKey, u64, Result<Protocol, String>)>,
    video_pending: Option<PendingVideo>,
    current: Option<Protocol>,
    pending: Option<PendingPreview>,
    /// The `/show`n image; kept so the preview can re-encode when the panel's
    /// real cell geometry becomes known (first paint) or changes (resize).
    shown: Option<ShownImage>,
    /// The cell box the current/pending protocol was encoded for.
    encoded_box: Option<Rect>,
    label: Option<String>,
    defer_first_render: bool,
    /// Agent portraits use the selected terminal graphics protocol and retain a
    /// bounded active/idle cache. Stock terminals still fall back to half blocks.
    portrait_picker: Picker,
    portrait_cache: VecDeque<PortraitCacheEntry>,
    portrait_failures: VecDeque<PortraitCacheKey>,
    portrait_pending: Option<PendingPortrait>,
    portrait_shown: Option<PortraitCacheKey>,
    portrait_enabled: bool,
    portrait_synchronous: bool,
    /// The WebGPU portal is deliberately narrower than ordinary image support:
    /// it paints only through Kitty's in-memory pixel protocol. Other terminals
    /// retain the terminal-native Agent/Realm visualization.
    portal_picker: Picker,
    #[cfg_attr(not(test), allow(dead_code))]
    world_font_size: (u16, u16),
    portal_enabled: bool,
    portal_current: Option<PortalCacheEntry>,
    portal_pending: Option<PendingPortal>,
    portal_failure: Option<PortalCacheKey>,
    /// The authored backed Realm map. Same machinery as the portal, its own
    /// cache slot so a map repaint never evicts an activity frame.
    #[cfg_attr(not(test), allow(dead_code))]
    map_enabled: bool,
    #[cfg_attr(not(test), allow(dead_code))]
    map_current: Option<PortalCacheEntry>,
    #[cfg_attr(not(test), allow(dead_code))]
    map_pending: Option<PendingPortal>,
    #[cfg_attr(not(test), allow(dead_code))]
    map_failure: Option<PortalCacheKey>,
    #[cfg_attr(not(test), allow(dead_code))]
    map_area: Option<Rect>,
    /// One resident worker composes and encodes map frames. The knight's ride
    /// changes the sequence many times a second, so a thread per miss would
    /// mean a spawn per miss on the draw thread.
    map_worker: Option<mpsc::Sender<MapEncodeJob>>,
    dot_current: Option<DotFrame>,
    dot_pending: Option<PendingDots>,
    dot_failure: Option<DotFrameKey>,
    dot_base_id: u32,
    dot_upload: Option<String>,
}

/// Readiness is not ownership: a supported pending sprite must not trigger a
/// second compositor on the draw thread. Legacy routes still use readiness.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorldPixelsState {
    Ready,
    Pending,
    Unavailable,
}

#[cfg(test)]
impl WorldPixelsState {
    fn ready(self) -> bool {
        self == Self::Ready
    }
}

impl Viewer {
    /// Build the image picker without querying the terminal.
    ///
    /// We deliberately do NOT call `Picker::from_query_stdio()`: its terminal
    /// probes can leak escape responses into crossterm's input stream. Select a
    /// backend from an explicit override or trustworthy terminal environment
    /// markers, then retain safe half blocks as the fallback.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new() -> Self {
        let picker = protocol_from_env()
            .map(Self::picker_for_protocol)
            .unwrap_or_else(halfblock_picker);
        Self::from_picker(picker)
    }

    /// Build the live TUI image picker, calibrating an ambiguous terminal.
    ///
    /// This must run after the alternate screen and raw mode are active, but
    /// before the event loop begins. `ratatui-image` then owns the complete
    /// capability-query exchange, so its replies cannot leak into cockpit
    /// keyboard input. Explicit overrides and trustworthy terminal markers
    /// remain probe-free fast paths.
    pub fn calibrated() -> Self {
        let (picker, source) = if let Some(protocol) = protocol_from_env() {
            (Self::picker_for_protocol(protocol), "environment")
        } else if inside_terminal_multiplexer() {
            // A DSR reply is not proof that the subsequent graphics query will
            // finish. Its detached stdin reader can outlive calibration and
            // steal literal bursts or Enter from TerminalInput. Ambiguous tmux
            // sessions use the terminal-native fallback; explicit protocol and
            // trustworthy environment hints above still select pixel graphics.
            (halfblock_picker(), "multiplexer-fallback")
        } else if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            match terminal_answers_status_report(STATUS_REPORT_PREFLIGHT) {
                Ok(()) => {
                    let options = QueryStdioOptions {
                        timeout: CALIBRATION_QUERY_TIMEOUT,
                        ..QueryStdioOptions::default()
                    };
                    (
                        Picker::from_query_stdio_with_options(options)
                            .unwrap_or_else(|_| halfblock_picker()),
                        "terminal-calibration",
                    )
                }
                Err(reason) => {
                    eprintln!("GRAPHICS: calibration skipped — {reason}");
                    (halfblock_picker(), "calibration-skipped")
                }
            }
        } else {
            (halfblock_picker(), "non-interactive-fallback")
        };
        // Say what was detected and what it costs. A silent gate reads as the
        // feature not existing: the Realm art was dark for a user who had no way
        // to see that their terminal had never offered a graphics protocol.
        let protocol = picker.protocol_type();
        let map_note = if map_supported(protocol) {
            "authored Realm graphics available"
        } else {
            "authored Realm graphics unavailable (no graphics protocol) — braille map in use"
        };
        eprintln!(
            "GRAPHICS: protocol={} source={} — {}",
            protocol_label(protocol),
            source,
            map_note,
        );
        Self::from_picker(picker)
    }

    fn from_picker(picker: Picker) -> Self {
        let mut viewer = Self::with_picker(picker);
        viewer.portrait_enabled = true;
        viewer
    }

    fn picker_for_protocol(protocol: ProtocolType) -> Picker {
        match protocol {
            // Real pixel protocols keep the crate's plausible physical
            // font-size estimate; only the halfblock fallback wants the
            // cell-honest 2×4 density.
            protocol if protocol != ProtocolType::Halfblocks => {
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(protocol);
                picker
            }
            _ => halfblock_picker(),
        }
    }

    /// Build a viewer for static text-only preview renders. The preview dump
    /// never displays foreground images, so it can skip image-protocol env reads.
    pub fn static_preview() -> Self {
        Self::with_picker(halfblock_picker())
    }

    /// One-frame headless preview renderer. Unlike normal test viewers, it
    /// prepares the bundled portrait synchronously so
    /// `--dump-preview-portrait` can prove the stock-terminal fallback without
    /// waiting for a second frame.
    pub fn portrait_preview() -> Self {
        let mut viewer = Self::with_picker(halfblock_picker());
        viewer.portrait_enabled = true;
        viewer.portrait_synchronous = true;
        viewer
    }

    fn with_picker(picker: Picker) -> Self {
        let portrait_picker = picker.clone();
        let portal_picker = picker.clone();
        let world_font_size = (picker.font_size().width, picker.font_size().height);
        let portal_enabled = picker.protocol_type() == ProtocolType::Kitty;
        // The map is ordinary pixels, so it works on any real graphics
        // protocol — unlike the portal, which is Kitty-only by design.
        // Halfblocks is excluded deliberately: it caps at one pixel per cell
        // half, so a 512px-wide island would land in ~45 pixels and read as
        // mush. Sixel is capable but the authored plates read as a dead still
        // on most hosts; prefer the living braille world unless the operator
        // forces pixel plates (`ANGEL_BACKED_MAP=1`). Kitty/iTerm2 keep plates.
        let map_enabled = map_desired(picker.protocol_type());
        Self {
            picker,
            artifact: None,
            artifact_pending: None,
            still_layout_epoch: 0,
            still_protocol: None,
            artifact_identity: None,
            artifact_decoded: None,
            inspector: Default::default(),
            video: None,
            video_pending: None,
            current: None,
            pending: None,
            shown: None,
            encoded_box: None,
            label: None,
            defer_first_render: false,
            portrait_picker,
            portrait_cache: VecDeque::new(),
            portrait_failures: VecDeque::new(),
            portrait_pending: None,
            portrait_shown: None,
            portrait_enabled: false,
            portrait_synchronous: false,
            portal_picker,
            world_font_size,
            portal_enabled,
            portal_current: None,
            portal_pending: None,
            portal_failure: None,
            map_enabled,
            map_current: None,
            map_pending: None,
            map_failure: None,
            map_area: None,
            map_worker: None,
            dot_current: None,
            dot_pending: None,
            dot_failure: None,
            dot_base_id: dot_image_id(),
            dot_upload: None,
        }
    }

    /// Load an image file for display. The preview itself is encoded on the
    /// next render, once the panel's real cell geometry is known.
    pub fn show(&mut self, path: &Path) -> Result<String, String> {
        let (w, h) = image_dimensions_fast(path)
            .map_err(|e| format!("read dimensions {}: {e}", path.display()))?;
        let label_path = path.display().to_string();
        self.label = Some(format!("{label_path} · {w}×{h} · loading preview"));
        self.current = None;
        self.pending = None;
        self.shown = Some(ShownImage {
            path: path.to_path_buf(),
            label: label_path.clone(),
        });
        self.encoded_box = None;
        self.defer_first_render = true;
        Ok(format!("showing {label_path} ({w}×{h})"))
    }

    pub fn clear(&mut self) {
        self.current = None;
        self.pending = None;
        self.shown = None;
        self.encoded_box = None;
        self.label = None;
        self.defer_first_render = false;
    }

    pub fn has_image(&self) -> bool {
        self.shown.is_some()
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[cfg(test)]
    pub fn preview_ready(&self) -> bool {
        self.current.is_some()
    }

    /// Render the loaded image into `area` (no-op if nothing is loaded).
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.defer_first_render {
            self.defer_first_render = false;
            return;
        }
        self.promote_pending_preview();
        self.refresh_preview_geometry(area);
        if let Some(proto) = self.current.as_ref() {
            frame.render_widget(Image::new(proto), fixed_protocol_area(proto, area));
        }
    }

    /// Forget authority/cache, but do not abandon a live worker and spawn a
    /// second decoder. Its stale completion will be drained by the next request.
    pub(crate) fn clear_still(&mut self) {
        if self
            .artifact_pending
            .as_ref()
            .is_some_and(|p| !matches!(p.rx.try_recv(), Err(mpsc::TryRecvError::Empty)))
        {
            self.artifact_pending = None;
        }
        self.still_layout_epoch = self.still_layout_epoch.wrapping_add(1);
        self.still_protocol = None;
        self.artifact_identity = None;
        self.artifact_decoded = None;
        self.artifact = None;
        self.inspector = Default::default();
    }

    pub(crate) fn invalidate_still_layout(&mut self) {
        self.still_layout_epoch = self.still_layout_epoch.wrapping_add(1);
        self.artifact = None;
        self.inspector.painted = None;
        self.inspector.viewport = None;
        self.inspector.scene = None;
        self.inspector.drag = None;
    }

    pub(crate) fn still_matches(&self, source: &crate::media::MediaSource, request: u64) -> bool {
        self.artifact_identity
            .as_ref()
            .is_some_and(|(s, r)| s == source && *r == request)
    }

    /// One in flight, with the current draw providing the latest desired key.
    /// Stale same-scene completion may paint while the latest view encodes.
    /// Changed geometry/identity may donate a source only, never a protocol.
    pub(crate) fn render_artifact(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        source: crate::media::MediaSource,
        request_id: u64,
    ) -> Result<bool, String> {
        let picker = still_pixel_picker(self.picker.protocol_type(), terminal_cell_pixels());
        self.render_artifact_with_picker(frame, area, source, request_id, picker)
    }

    fn render_artifact_with_picker(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        source: crate::media::MediaSource,
        request_id: u64,
        picker: Picker,
    ) -> Result<bool, String> {
        if area.width == 0 || area.height == 0 {
            self.clear_still();
            return Ok(false);
        }
        if !self.still_matches(&source, request_id) {
            self.clear_still();
            self.artifact_identity = Some((source.clone(), request_id));
        }
        let font = picker.font_size();
        let font = (font.width, font.height);
        // Bound physical output, never source detail. This exact local picker
        // supplies the physical cache/input geometry AND encodes the raster.
        let mut width = area.width.min(MAX_INLINE_PREVIEW_CELLS.width);
        let mut height = area.height.min(MAX_INLINE_PREVIEW_CELLS.height);
        const MAX_STILL_OUTPUT_PIXELS: u64 = 4 * 1024 * 1024;
        let output = u64::from(width) * u64::from(height) * u64::from(font.0) * u64::from(font.1);
        if output > MAX_STILL_OUTPUT_PIXELS {
            let ratio = (MAX_STILL_OUTPUT_PIXELS as f64 / output as f64).sqrt();
            width = (f64::from(width) * ratio).floor() as u16;
            height = (f64::from(height) * ratio).floor() as u16;
            if width == 0 || height == 0 {
                self.clear_still();
                return Err("terminal image cell geometry exceeds raster budget".into());
            }
        }
        let scene = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        let pixels = (
            u32::from(width) * u32::from(font.0),
            u32::from(height) * u32::from(font.1),
        );
        if self.inspector.scene != Some(scene)
            || self.inspector.pixels != pixels
            || self.still_protocol != Some(picker.protocol_type())
        {
            self.invalidate_still_layout();
        }
        self.still_protocol = Some(picker.protocol_type());
        self.inspector.geometry(scene, pixels);
        let key = ArtifactKey {
            source,
            request_id,
            width,
            height,
            scene,
            font,
            protocol: picker.protocol_type(),
            layout_epoch: self.still_layout_epoch,
            view: self.inspector.view,
        };
        if self
            .artifact
            .as_ref()
            .is_some_and(|(ready, _)| !ready.same_scene(&key))
        {
            self.artifact = None;
            self.inspector.viewport = None;
            self.inspector.painted = None;
            self.inspector.drag = None;
        }
        if let Some(pending) = &self.artifact_pending {
            let result = match pending.rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("requested image worker stopped".into()))
                }
            };
            if let Some(result) = result {
                let completed = pending.key.clone();
                self.artifact_pending = None;
                if completed.source == key.source && completed.request_id == key.request_id {
                    match result {
                        Ok(prepared) => {
                            self.inspector.source =
                                Some((prepared.decoded.width(), prepared.decoded.height()));
                            self.artifact_decoded = Some(prepared.decoded);
                            if completed.same_scene(&key)
                                && (completed == key
                                    || !self
                                        .artifact
                                        .as_ref()
                                        .is_some_and(|(ready, _)| ready == &key))
                            {
                                self.artifact = Some((completed, Ok(prepared.protocol)));
                            }
                        }
                        Err(error) if completed.same_scene(&key) => {
                            self.clear_still();
                            return Err(error);
                        }
                        Err(_) => {} // Superseded geometry cannot latch a fault.
                    }
                }
            }
        }
        if let Some((ready, Ok(protocol))) = &self.artifact
            && ready.same_scene(&key)
        {
            frame.render_widget(Image::new(protocol), scene);
            self.inspector.viewport = Some(scene);
            self.inspector.painted = Some(ready.view);
            self.inspector.loading = ready != &key;
            if ready == &key {
                return Ok(true);
            }
        } else {
            self.inspector.viewport = None;
            self.inspector.painted = None;
        }
        // Keep the last completed same-scene protocol while coalescing desired
        // transforms. False means latest view is pending, NOT necessarily blank.
        self.inspector.loading = true;
        if self.artifact_pending.is_none() {
            let worker_key = key.clone();
            // Move the sole decoded allocation to the worker, then back with its
            // result. Gestures never decode again; rapid identity changes wait.
            let decoded = self.artifact_decoded.take();
            let (tx, rx) = mpsc::channel();
            let spawn = std::thread::Builder::new()
                .name("angel-still-inspector".into())
                .spawn(move || {
                    let result = (|| {
                        let decoded = match decoded {
                            Some(image) => image,
                            None => Arc::new(artifact_preview(&worker_key.source)?),
                        };
                        let t = worker_key
                            .view
                            .transform((decoded.width(), decoded.height()), pixels);
                        let mut image = t.raster(&decoded).into_rgba8();
                        if picker.protocol_type() == ProtocolType::Halfblocks {
                            for pixel in image.pixels_mut() {
                                let alpha = u16::from(pixel[3]);
                                for channel in &mut pixel.0[..3] {
                                    *channel = (u16::from(*channel) * alpha / 255) as u8;
                                }
                                pixel[3] = 255;
                            }
                        }
                        let protocol = picker
                            .new_protocol(
                                image::DynamicImage::ImageRgba8(image),
                                Rect::new(0, 0, worker_key.width, worker_key.height).into(),
                                Resize::Fit(None),
                            )
                            .map_err(|e| format!("prepare requested image: {e}"))?;
                        Ok(PreparedArtifact { decoded, protocol })
                    })();
                    let _ = tx.send(result);
                });
            if let Err(error) = spawn {
                self.clear_still();
                return Err(format!("start requested image worker: {error}"));
            }
            self.artifact_pending = Some(PendingArtifact { key, rx });
        }
        Ok(false)
    }

    /// Present the latest already-decoded video frame on the resident encoder.
    /// The decoder/mailbox own timing; this layer never queues a timeline.
    pub(crate) fn video_decode_viewport(&self, area: Rect) -> (usize, usize) {
        let font = self.picker.font_size();
        // The decoder's existing scaler API uses 2x4 pixel units. Native
        // graphics must request physical pixels, not upscale halfblock-sized
        // input. Both dimensions stay within the 98,304-pixel decode budget.
        (
            (u32::from(area.width) * u32::from(font.width))
                .div_ceil(2)
                .clamp(1, 192) as usize,
            (u32::from(area.height) * u32::from(font.height))
                .div_ceil(4)
                .clamp(1, 64) as usize,
        )
    }

    /// Present decoded pixels without rebuilding the source decoder.
    pub(crate) fn render_video(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        pixels: Arc<crate::scryglass::VideoPixels>,
    ) -> Result<bool, String> {
        if area.width == 0 || area.height == 0 {
            return Ok(false);
        }
        let key = VideoCacheKey {
            identity: pixels.identity.clone(),
            width: area.width.min(MAX_INLINE_PREVIEW_CELLS.width),
            height: area.height.min(MAX_INLINE_PREVIEW_CELLS.height),
        };
        if let Some(pending) = &self.video_pending {
            let result = match pending.rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("video image worker stopped".into()))
                }
            };
            if let Some(result) = result {
                self.video = Some((pending.key.clone(), pending.sequence, result));
                self.video_pending = None;
            }
        }
        let mut painted = false;
        if let Some((ready, sequence, result)) = &self.video
            && ready == &key
        {
            let protocol = result.as_ref().map_err(Clone::clone)?;
            frame.render_widget(Image::new(protocol), fixed_protocol_area(protocol, area));
            painted = true;
            if *sequence == pixels.sequence {
                return Ok(true);
            }
        }
        // Keep an earlier frame only within the exact reveal/seek generation
        // and cell geometry. Pending obsolete work is drained, never piled up.
        if self.video_pending.is_none() {
            let sequence = pixels.sequence;
            let picker = self.picker.clone();
            let target = video_encode_area(
                &picker,
                pixels.rgba.width(),
                pixels.rgba.height(),
                Rect::new(0, 0, key.width, key.height),
            );
            let (tx, rx) = mpsc::channel();
            self.send_map_job(Box::new(move || {
                let font = picker.font_size();
                let encoded_pixels = u64::from(target.width)
                    * u64::from(target.height)
                    * u64::from(font.width)
                    * u64::from(font.height);
                let result = if pixels.rgba.width() == 0 || pixels.rgba.height() == 0 {
                    Err("video frame has no pixels".into())
                } else if u64::from(pixels.rgba.width()) * u64::from(pixels.rgba.height())
                    > u64::from(MAX_INLINE_PREVIEW_PIXELS)
                    || encoded_pixels > u64::from(MAX_INLINE_PREVIEW_PIXELS)
                {
                    Err("video preview exceeds the pixel budget".into())
                } else {
                    picker
                        .new_protocol(
                            image::DynamicImage::ImageRgba8(pixels.rgba.clone()),
                            target.into(),
                            Resize::Scale(Some(image::imageops::FilterType::Triangle)),
                        )
                        .map_err(|error| format!("prepare video frame: {error}"))
                };
                let _ = tx.send(result);
            }))
            .map_err(|()| "start video image worker".to_string())?;
            self.video_pending = Some(PendingVideo { key, sequence, rx });
        }
        Ok(painted)
    }

    /// (Re)encode the shown image for the pane it actually paints in: the
    /// first frame after `/show` and any later pane resize both land here.
    /// The last good protocol stays visible while its replacement encodes.
    fn refresh_preview_geometry(&mut self, area: Rect) {
        if self.pending.is_some() || area.width == 0 || area.height == 0 {
            return;
        }
        let Some(shown) = self.shown.clone() else {
            return;
        };
        let target = Rect::new(
            0,
            0,
            area.width.min(MAX_INLINE_PREVIEW_CELLS.width),
            area.height.min(MAX_INLINE_PREVIEW_CELLS.height),
        );
        if self.encoded_box == Some(target) {
            return;
        }
        self.encoded_box = Some(target);
        let picker = self.picker.clone();
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("angel-image-preview".to_string())
            .spawn(move || {
                let result = cached_bounded_preview(&shown.path).and_then(|(preview, original)| {
                    let preview_size = (preview.width(), preview.height());
                    let protocol = picker
                        .new_protocol(
                            preview,
                            target.into(),
                            Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                        )
                        .map_err(|e| format!("prepare {}: {e}", shown.path.display()))?;
                    Ok(PreparedPreview {
                        label: shown.label,
                        original,
                        preview_size,
                        protocol,
                    })
                });
                let _ = tx.send(result);
            });
        self.pending = Some(PendingPreview { rx });
    }

    /// Render a bundled agent portrait through the selected terminal graphics
    /// protocol. The production path decodes/resizes off the draw thread, then
    /// reuses a bounded protocol cache keyed by asset and exact cell geometry.
    pub fn render_portrait(&mut self, frame: &mut Frame, area: Rect, path: &Path) -> bool {
        self.render_portrait_variant(frame, area, path, None)
    }

    pub(crate) fn render_helm(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        path: &Path,
        pose: u8,
    ) -> bool {
        self.render_portrait_variant(frame, area, path, Some(pose))
    }

    fn render_portrait_variant(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        path: &Path,
        pose: Option<u8>,
    ) -> bool {
        if !self.portrait_enabled || area.width == 0 || area.height == 0 {
            return false;
        }
        self.promote_pending_portrait();
        let mut key = Self::portrait_key(area, path);
        key.pose = pose;
        if self.render_cached_portrait(frame, area, &key) {
            return true;
        }
        // Retain the last painted pose only for this same agent and geometry.
        // Loading a new pose must not flash the ASCII fallback or another agent.
        let prior = self
            .portrait_shown
            .as_ref()
            .filter(|old| {
                old.path == key.path
                    && old.width == key.width
                    && old.height == key.height
                    && old.pose.is_some()
                    && key.pose.is_some()
            })
            .cloned();
        let retained = prior
            .as_ref()
            .is_some_and(|old| self.render_cached_portrait(frame, area, old));
        if self.portrait_failures.contains(&key) {
            return retained;
        }
        if let Some(pending) = &self.portrait_pending {
            if pose.is_none() && pending.prefetch && pending.key != key {
                // Preserve the explicit legacy portrait API's visible-priority
                // behavior. Stateful helm playback admits only one worker.
                self.portrait_pending = None;
            } else {
                return retained;
            }
        }

        let size = Rect::new(0, 0, area.width, area.height);
        if !self.portrait_synchronous {
            self.queue_portrait(key, false);
            return retained;
        }

        let protocol = match portrait_image(path, pose).and_then(|image| {
            self.portrait_picker
                .new_protocol(
                    image,
                    size.into(),
                    Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                )
                .map_err(|error| error.to_string())
        }) {
            Ok(protocol) => protocol,
            Err(_) => {
                self.insert_portrait_failure(key);
                return false;
            }
        };
        self.insert_portrait_protocol(key.clone(), protocol);
        self.render_cached_portrait(frame, area, &key)
    }

    pub fn supports_agentviz_portal(&self) -> bool {
        self.portal_enabled
    }

    pub fn agentviz_portal_pending(&self) -> bool {
        self.portal_pending.is_some()
    }

    /// Encode and paint one validated raw WebGPU frame through Kitty. Encoding
    /// stays off the draw thread and is keyed by exact sequence/cell geometry,
    /// so a resize or a newer activity state cannot reuse stale placement data.
    pub fn render_agentviz_portal(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        portal_frame: &crate::agentviz_portal::PortalFrame,
    ) -> bool {
        if !self.portal_enabled || area.width == 0 || area.height == 0 {
            return false;
        }
        self.promote_pending_portal();
        let key = PortalCacheKey {
            sequence: portal_frame.sequence,
            width: area.width,
            height: area.height,
        };
        if let Some(current) = self
            .portal_current
            .as_ref()
            .filter(|entry| entry.key == key)
        {
            frame.render_widget(
                Image::new(&current.protocol),
                fixed_protocol_area(&current.protocol, area),
            );
            return true;
        }
        if self.portal_failure.as_ref() == Some(&key) {
            return false;
        }
        // A newer activity sequence is already available to the next draw.
        // Let the admitted encode settle before taking that latest snapshot;
        // abandoning its receiver here would spawn one worker per fast frame.
        if self.portal_pending.is_some() {
            return false;
        }
        let pixels = Arc::clone(&portal_frame.pixels);
        let picker = self.portal_picker.clone();
        let size = Rect::new(0, 0, area.width, area.height);
        let worker_key = key.clone();
        let (tx, rx) = mpsc::channel();
        let spawn = std::thread::Builder::new()
            .name("angel-webgpu-portal-kitty".to_string())
            .spawn(move || {
                let result = image::RgbaImage::from_raw(
                    crate::agentviz_portal::FRAME_WIDTH,
                    crate::agentviz_portal::FRAME_HEIGHT,
                    pixels.as_ref().to_vec(),
                )
                .ok_or_else(|| "portal frame length did not match RGBA dimensions".to_string())
                .and_then(|rgba| {
                    picker
                        .new_protocol(
                            image::DynamicImage::ImageRgba8(rgba),
                            size.into(),
                            Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                        )
                        .map_err(|error| format!("prepare Kitty portal: {error}"))
                });
                let _ = tx.send(result);
            });
        match spawn {
            Ok(_) => {
                self.portal_pending = Some(PendingPortal {
                    key: worker_key,
                    rx,
                });
            }
            Err(_) => {
                self.portal_failure = Some(key);
            }
        }
        false
    }

    /// Exercise native room-image encoding without halfblock admission.
    #[cfg(test)]
    pub fn render_native_location<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> bool
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels(frame, area, sequence, false, compose)
            .ready()
    }

    /// Explicit ambient stills also admit ordinary-terminal half blocks. Their
    /// broad room composition survives the lower pixel count, and filled color
    /// preserves the plate's material/lighting hierarchy. The caller retains
    /// the Realm on/off policy and the 3D view renders independently.
    #[cfg(test)]
    pub fn render_ambient_location<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> bool
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels(frame, area, sequence, true, compose)
            .ready()
    }

    /// Live pixel art uses the same bounded resident worker on ordinary terminals.
    /// Fine world dots require real pixel placement. Other protocols keep
    /// ordinary Braille; half blocks cannot represent independent dot spacing.
    pub(crate) fn dot_geometry(&self, area: Rect) -> Option<crate::dot_canvas::DotGeometry> {
        if self.portal_picker.protocol_type() != ProtocolType::Kitty
            || area.width > 256
            || area.height > 256
            || std::env::var_os("TMUX").is_some()
        {
            return None;
        }
        let pitch = dot_pitch(std::env::var("ANGEL_DOTMAX_PITCH").ok().as_deref())?;
        let picker = self.world_pixel_picker();
        let font = picker.font_size();
        crate::dot_canvas::DotGeometry::new(
            area.width,
            area.height,
            (font.width, font.height),
            pitch,
        )
    }

    /// Accepts only composed Braille cells. Neither file paths nor source PNG
    /// bytes can enter this transport. One worker converts/encodes the newest
    /// admitted frame while this same scene retains its last completed dots.
    pub(crate) fn render_world_dots(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        scene: u64,
        geometry: crate::dot_canvas::DotGeometry,
        image: Arc<crate::terminal_art::ColoredBrailleImage>,
    ) -> bool {
        use std::hash::{Hash, Hasher};
        if self.portal_picker.protocol_type() != ProtocolType::Kitty {
            return false;
        }
        let mut generation = std::collections::hash_map::DefaultHasher::new();
        image.width.hash(&mut generation);
        image.height.hash(&mut generation);
        for cell in &image.cells {
            cell.glyph.hash(&mut generation);
            cell.fg.hash(&mut generation);
        }
        let key = DotFrameKey {
            scene,
            geometry,
            columns: area.width,
            rows: area.height,
            generation: generation.finish(),
        };
        if let Some(pending) = self.dot_pending.take() {
            match pending.rx.try_recv() {
                Ok(Ok(protocol)) if pending.key.same_surface(&key) => {
                    self.dot_current = Some(DotFrame {
                        key: pending.key,
                        protocol,
                    });
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.dot_failure = Some(pending.key)
                }
                Err(mpsc::TryRecvError::Empty) => self.dot_pending = Some(pending),
                Ok(Ok(_)) => {} // Old workspace/room/resize completion cannot paint here.
            }
        }
        if self
            .dot_current
            .as_ref()
            .is_some_and(|current| !current.key.same_surface(&key))
        {
            self.dot_current = None;
        }
        let mut painted = false;
        let mut fresh = false;
        if let Some(current) = &mut self.dot_current {
            current.protocol.render(area, frame.buffer_mut());
            if let Some(upload) = current.protocol.take_upload() {
                self.dot_upload = Some(upload);
            }
            painted = true;
            fresh = current.key == key;
        }
        if fresh || self.dot_pending.is_some() || self.dot_failure.as_ref() == Some(&key) {
            return painted;
        }
        let size = Rect::new(0, 0, area.width, area.height);
        // Two reusable IDs bound terminal-side frame storage. Upload the slot
        // that is not currently visible, then replace its placeholder cells.
        let id = if self
            .dot_current
            .as_ref()
            .is_some_and(|current| current.protocol.image_id() == self.dot_base_id)
        {
            self.dot_base_id + 1
        } else {
            self.dot_base_id
        };
        let (tx, rx) = mpsc::channel();
        let job: MapEncodeJob = Box::new(move || {
            let result =
                crate::dot_protocol::DotProtocol::encode(geometry, &image, size.into(), id);
            let _ = tx.send(result);
        });
        if self.send_map_job(job).is_ok() {
            self.dot_pending = Some(PendingDots { key, rx });
        } else {
            self.dot_failure = Some(key);
        }
        painted
    }

    /// Flush at the end of the composed frame. A dancer, approval overlay, or
    /// pane border cannot erase the one-time image upload by replacing a cell.
    pub(crate) fn flush_dot_upload(&mut self, frame: &mut Frame) {
        let Some(upload) = self.dot_upload.take() else {
            return;
        };
        let area = frame.area();
        if let Some(cell) = frame.buffer_mut().cell_mut((area.x, area.y)) {
            let symbol = format!("{upload}{}", cell.symbol());
            cell.set_symbol(&symbol)
                .set_diff_option(ratatui::buffer::CellDiffOption::ForcedWidth(
                    std::num::NonZeroU16::new(1).unwrap(),
                ));
        }
    }

    /// Whether the terminal can display a full raster instead of cell glyphs.
    pub(crate) fn native_world_graphics(&self) -> bool {
        matches!(
            self.portal_picker.protocol_type(),
            ProtocolType::Kitty | ProtocolType::Sixel | ProtocolType::Iterm2
        )
    }

    fn world_pixel_picker(&self) -> Picker {
        if self.native_world_graphics() {
            // TIOCGWINSZ does not consume terminal input or issue escape probes.
            // Keep the existing estimate only when the host omits pixel sizes.
            if let Some(font) = terminal_cell_pixels() {
                #[allow(deprecated)]
                let mut picker = Picker::from_fontsize(font.into());
                picker.set_protocol_type(self.portal_picker.protocol_type());
                return picker;
            }
        }
        self.portal_picker.clone()
    }

    #[cfg(test)]
    fn render_world_pixels<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        ambient: bool,
        compose: F,
    ) -> WorldPixelsState
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.map_area = None;
        let picker = self.world_pixel_picker();
        let font = (picker.font_size().width, picker.font_size().height);
        if font != self.world_font_size {
            self.map_current = None;
            // Keep the outstanding admission until it settles. Dropping it
            // here allowed rapid DPI changes to queue unbounded obsolete jobs.
            self.map_failure = None;
            self.world_font_size = font;
        }
        let supported = self.map_enabled
            || (ambient && self.portal_picker.protocol_type() == ProtocolType::Halfblocks);
        if !supported || area.width == 0 || area.height == 0 {
            return WorldPixelsState::Unavailable;
        }
        self.promote_pending_map();
        let key = PortalCacheKey {
            sequence,
            width: area.width,
            height: area.height,
        };
        if let Some(current) = self.map_current.as_ref().filter(|entry| entry.key == key) {
            let painted_area = fixed_protocol_area(&current.protocol, area);
            frame.render_widget(Image::new(&current.protocol), painted_area);
            self.map_area = Some(painted_area);
            return WorldPixelsState::Ready;
        }
        if self.map_failure.as_ref() == Some(&key) {
            return WorldPixelsState::Unavailable;
        }
        // Coalesce changing world sequences behind the one admitted encode.
        // The next draw snapshots the latest state after it settles, keeping
        // the resident worker's FIFO at zero queued obsolete frames.
        if self.map_pending.is_some() {
            return WorldPixelsState::Pending;
        }
        // Only now is a frame actually needed, and the closure only snapshots
        // world state: the per-pixel compose hides behind the pixels'
        // `as_ref`, which the worker takes just before encoding. A miss costs
        // the draw thread a snapshot, not a 31k-pixel compose.
        let (pixels, width, height) = compose();
        let size = Rect::new(0, 0, area.width, area.height);
        let worker_key = key.clone();
        let (tx, rx) = mpsc::channel();
        let job: MapEncodeJob = Box::new(move || {
            let started = std::time::Instant::now();
            let bytes = pixels.as_ref().to_vec();
            let composed = started.elapsed();
            let result = image::RgbaImage::from_raw(width, height, bytes)
                .ok_or_else(|| "map frame length did not match RGBA dimensions".to_string())
                .and_then(|rgba| {
                    picker
                        .new_protocol(
                            image::DynamicImage::ImageRgba8(rgba),
                            size.into(),
                            // Scale, not Fit. Fit returns early when the
                            // image already fits the area in cells and
                            // renders it at natural size — a 152x208 map
                            // is ~13x8 cells, so in a 43x30 pane it sat as
                            // a stamp in the middle and never enlarged.
                            // Scale is excluded from that early-return and
                            // always fills the pane.
                            //
                            // Nearest keeps the pixel art crisp; a smooth
                            // filter turns 8px tiles into mush.
                            Resize::Scale(Some(image::imageops::FilterType::Nearest)),
                        )
                        .map_err(|error| format!("prepare world image: {error}"))
                });
            #[cfg(test)]
            eprintln!(
                "world_raster {width}x{height} compose_us={} sample_encode_us={}",
                composed.as_micros(),
                started.elapsed().saturating_sub(composed).as_micros()
            );
            tracing::debug!(target: "world_raster", width, height,
                compose_us = composed.as_micros() as u64,
                sample_encode_us = started.elapsed().saturating_sub(composed).as_micros() as u64,
                "bounded world raster worker");
            let _ = tx.send(result);
        });
        match self.send_map_job(job) {
            Ok(()) => {
                self.map_pending = Some(PendingPortal {
                    key: worker_key,
                    rx,
                });
            }
            Err(()) => {
                self.map_failure = Some(key);
            }
        }
        if self.map_pending.is_some() {
            WorldPixelsState::Pending
        } else {
            WorldPixelsState::Unavailable
        }
    }

    #[cfg(test)]
    pub(crate) fn hold_room_worker_for_test(&mut self) -> mpsc::Sender<()> {
        let (tx, rx) = mpsc::channel();
        self.send_map_job(Box::new(move || {
            let _ = rx.recv();
        }))
        .unwrap();
        tx
    }

    /// Hand one job to the resident map worker, spawning it on first use. The
    /// worker exits when the viewer (its only sender) is dropped.
    fn send_map_job(&mut self, job: MapEncodeJob) -> Result<(), ()> {
        if self.map_worker.is_none() {
            let (tx, rx) = mpsc::channel::<MapEncodeJob>();
            let spawned = std::thread::Builder::new()
                .name("angel-room-plates".to_string())
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        job();
                    }
                });
            if spawned.is_ok() {
                self.map_worker = Some(tx);
            }
        }
        let Some(worker) = self.map_worker.as_ref() else {
            return Err(());
        };
        if worker.send(job).is_err() {
            // The worker died; drop its sender so a later miss respawns one.
            self.map_worker = None;
            return Err(());
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn promote_pending_map(&mut self) {
        let Some(pending) = self.map_pending.as_ref() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(protocol)) => {
                let key = pending.key.clone();
                self.map_pending = None;
                self.map_current = Some(PortalCacheEntry { key, protocol });
            }
            Ok(Err(_)) => {
                let key = pending.key.clone();
                self.map_pending = None;
                self.map_failure = Some(key);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                let key = pending.key.clone();
                self.map_pending = None;
                self.map_failure = Some(key);
            }
        }
    }

    /// Warm another portrait state for the same cell geometry without painting
    /// it. Synchronous/headless preview mode deliberately skips this so a static
    /// proof decodes only the image it actually shows.
    pub fn prefetch_portrait(&mut self, area: Rect, path: &Path) {
        self.prefetch_portrait_variant(area, path, None);
    }

    pub(crate) fn prefetch_helm(&mut self, area: Rect, path: &Path, pose: u8) {
        self.prefetch_portrait_variant(area, path, Some(pose));
    }

    fn prefetch_portrait_variant(&mut self, area: Rect, path: &Path, pose: Option<u8>) {
        if !self.portrait_enabled
            || self.portrait_synchronous
            || area.width == 0
            || area.height == 0
        {
            return;
        }
        self.promote_pending_portrait();
        let mut key = Self::portrait_key(area, path);
        key.pose = pose;
        if self.portrait_cache.iter().any(|entry| entry.key == key)
            || self.portrait_failures.contains(&key)
            || self.portrait_pending.is_some()
        {
            return;
        }
        self.queue_portrait(key, true);
    }

    fn portrait_key(area: Rect, path: &Path) -> PortraitCacheKey {
        PortraitCacheKey {
            path: path.to_path_buf(),
            width: area.width,
            height: area.height,
            pose: None,
        }
    }

    fn queue_portrait(&mut self, key: PortraitCacheKey, prefetch: bool) {
        let path = key.path.clone();
        let pose = key.pose;
        let size = Rect::new(0, 0, key.width, key.height);
        let picker = self.portrait_picker.clone();
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("angel-agent-portrait".to_string())
            .spawn(move || {
                let result = portrait_image(&path, pose).and_then(|image| {
                    picker
                        .new_protocol(
                            image,
                            size.into(),
                            Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                        )
                        .map_err(|error| error.to_string())
                });
                let _ = tx.send(result);
            });
        self.portrait_pending = Some(PendingPortrait { key, rx, prefetch });
    }

    fn promote_pending_portrait(&mut self) {
        let Some(pending) = self.portrait_pending.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(protocol)) => self.insert_portrait_protocol(pending.key, protocol),
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                self.insert_portrait_failure(pending.key)
            }
            Err(mpsc::TryRecvError::Empty) => self.portrait_pending = Some(pending),
        }
    }

    fn promote_pending_portal(&mut self) {
        let Some(pending) = self.portal_pending.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(protocol)) => {
                self.portal_failure = None;
                self.portal_current = Some(PortalCacheEntry {
                    key: pending.key,
                    protocol,
                });
            }
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                self.portal_failure = Some(pending.key);
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.portal_pending = Some(pending);
            }
        }
    }

    fn insert_portrait_protocol(&mut self, key: PortraitCacheKey, protocol: Protocol) {
        self.portrait_failures.retain(|failed| failed != &key);
        if self.portrait_cache.len() >= PORTRAIT_CACHE_LIMIT {
            self.portrait_cache.pop_front();
        }
        self.portrait_cache
            .push_back(PortraitCacheEntry { key, protocol });
    }

    fn insert_portrait_failure(&mut self, key: PortraitCacheKey) {
        if self.portrait_failures.contains(&key) {
            return;
        }
        if self.portrait_failures.len() >= PORTRAIT_CACHE_LIMIT {
            self.portrait_failures.pop_front();
        }
        self.portrait_failures.push_back(key);
    }

    fn render_cached_portrait(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        key: &PortraitCacheKey,
    ) -> bool {
        let Some(entry) = self.portrait_cache.iter().find(|entry| &entry.key == key) else {
            return false;
        };
        frame.render_widget(
            Image::new(&entry.protocol),
            bottom_right_protocol_area(&entry.protocol, area),
        );
        self.portrait_shown = Some(key.clone());
        true
    }

    fn promote_pending_preview(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(prepared)) => {
                let (w, h) = prepared.original;
                let (pw, ph) = prepared.preview_size;
                let suffix = if (pw, ph) == (w, h) {
                    String::new()
                } else {
                    format!(" · preview {pw}×{ph}")
                };
                self.label = Some(format!("{} · {w}×{h}{suffix}", prepared.label));
                self.current = Some(prepared.protocol);
            }
            Ok(Err(e)) => {
                self.label = Some(format!("image preview failed: {e}"));
                self.current = None;
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending = Some(pending);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.label = Some("image preview failed: worker disconnected".to_string());
                self.current = None;
            }
        }
    }
}

/// Whether authored Realm plates can be drawn under this protocol.
///
/// Wider than the WebGPU portal's gate, which is Kitty-only by design — the
/// map is ordinary pixels and any real graphics protocol carries it. Halfblocks
/// is excluded on resolution, not capability: it yields one pixel per cell
/// half, so a 512px-wide island lands in roughly 45 pixels and reads as mush.
/// Below that floor the braille map is genuinely the better picture.
fn map_supported(protocol: ProtocolType) -> bool {
    matches!(
        protocol,
        ProtocolType::Kitty | ProtocolType::Sixel | ProtocolType::Iterm2
    )
}

/// Whether this host should *prefer* pixel Realm plates over the living braille
/// world. Capability (`map_supported`) is not enough: sixel is often muddy and
/// static for the current plate set, so braille is the better default there.
fn map_desired(protocol: ProtocolType) -> bool {
    if !map_supported(protocol) {
        return false;
    }
    // Explicit operator override wins either way.
    if let Ok(raw) = std::env::var("ANGEL_BACKED_MAP").or_else(|_| std::env::var("ANGEL_TILE_MAP"))
    {
        let off = matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "off" | "false" | "no"
        );
        return !off;
    }
    matches!(protocol, ProtocolType::Kitty | ProtocolType::Iterm2)
}

fn protocol_label(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Halfblocks => "halfblocks",
        ProtocolType::Kitty => "kitty",
        ProtocolType::Sixel => "sixel",
        ProtocolType::Iterm2 => "iterm2",
    }
}

fn image_dimensions_fast(path: &Path) -> Result<(u32, u32), image::ImageError> {
    if let Some(dimensions) = png_dimensions_fast(path) {
        return Ok(dimensions);
    }
    image::image_dimensions(path)
}

fn png_dimensions_fast(path: &Path) -> Option<(u32, u32)> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return None;
    }

    let mut header = [0u8; 24];
    let mut file = std::fs::File::open(path).ok()?;
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(header[16..20].try_into().ok()?),
        u32::from_be_bytes(header[20..24].try_into().ok()?),
    ))
}

/// Fit media in physical-pixel space, then encode only the bounded content
/// rectangle. `Resize::Fit` alone leaves small decoded frames as tiny stamps
/// on native high-resolution protocols because it refuses to upscale.
fn video_encode_area(picker: &Picker, width: u32, height: u32, available: Rect) -> Rect {
    let font = picker.font_size();
    let fw = f64::from(font.width.max(1));
    let fh = f64::from(font.height.max(1));
    let width = f64::from(width.max(1));
    let height = f64::from(height.max(1));
    let scale = (f64::from(available.width) * fw / width)
        .min(f64::from(available.height) * fh / height)
        .min((f64::from(MAX_INLINE_PREVIEW_PIXELS) / (width * height)).sqrt());
    Rect::new(
        0,
        0,
        ((width * scale / fw).floor() as u16)
            .max(1)
            .min(available.width),
        ((height * scale / fh).floor() as u16)
            .max(1)
            .min(available.height),
    )
}

fn fixed_protocol_area(proto: &Protocol, available: Rect) -> Rect {
    let image = proto.size();
    let width = image.width.min(available.width);
    let height = image.height.min(available.height);
    Rect {
        x: available.x + available.width.saturating_sub(width) / 2,
        y: available.y + available.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn bottom_right_protocol_area(proto: &Protocol, available: Rect) -> Rect {
    let image = proto.size();
    let width = image.width.min(available.width);
    let height = image.height.min(available.height);
    Rect {
        x: available.x + available.width.saturating_sub(width),
        y: available.y + available.height.saturating_sub(height),
        width,
        height,
    }
}

fn dot_image_id() -> u32 {
    use std::hash::BuildHasher;
    let hash = std::collections::hash_map::RandomState::new().hash_one(std::process::id());
    ((hash as u32) & 0x00ff_fffc) | 2
}

fn dot_pitch(raw: Option<&str>) -> Option<u16> {
    match raw.map(str::trim) {
        Some("0" | "text" | "off") => None,
        Some(value) => Some(
            value
                .parse::<u16>()
                .ok()
                .filter(|n| (2..=8).contains(n))
                .unwrap_or(2),
        ),
        None => Some(2),
    }
}

fn portrait_image(path: &Path, pose: Option<u8>) -> Result<image::DynamicImage, String> {
    match pose {
        Some(pose) => crate::helm::frame_image(path, pose),
        None => cached_bounded_preview(path).map(|(image, _)| image),
    }
}

fn cached_bounded_preview(path: &Path) -> Result<(image::DynamicImage, (u32, u32)), String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("stat {}: {e}", path.display()))?;
    let len = metadata.len();
    let modified = metadata.modified().ok();

    let cache = PREVIEW_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(entry) = cache.get(path)
        && entry.len == len
        && entry.modified == modified
    {
        return Ok((entry.preview.clone(), entry.original));
    }

    let img = image::ImageReader::open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("decode {}: {e}", path.display()))?;
    let original = (img.width(), img.height());
    let preview = bounded_preview(img);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() >= PREVIEW_CACHE_LIMIT
            && let Some(oldest) = cache.keys().next().cloned()
        {
            cache.remove(&oldest);
        }
        cache.insert(
            path.to_path_buf(),
            PreviewCacheEntry {
                len,
                modified,
                original,
                preview: preview.clone(),
            },
        );
    }
    Ok((preview, original))
}

fn artifact_preview(source: &crate::media::MediaSource) -> Result<image::DynamicImage, String> {
    let file = source.open()?;
    if file.metadata().map_err(|error| error.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("image preview exceeds the 64 MiB source limit".into());
    }
    let mut reader = image::ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .map_err(|error| format!("Cannot identify image: {error}"))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("Cannot decode requested image: {error}"))?;
    if image.as_bytes().len() > 128 * 1024 * 1024 {
        return Err("image exceeds the 128 MiB decoded source limit".into());
    }
    if u64::from(image.width()) * u64::from(image.height()) > 32 * 1024 * 1024 {
        return Err("image preview exceeds the 32-megapixel raster limit".into());
    }
    if image.width() == 0 || image.height() == 0 {
        return Err("requested image is empty".into());
    }
    Ok(image)
}

fn bounded_preview(img: image::DynamicImage) -> image::DynamicImage {
    let pixels = img.width().saturating_mul(img.height());
    if pixels <= MAX_INLINE_PREVIEW_PIXELS {
        return img;
    }
    let scale = (MAX_INLINE_PREVIEW_PIXELS as f64 / pixels as f64).sqrt();
    let width = ((img.width() as f64 * scale).round() as u32).max(1);
    let height = ((img.height() as f64 * scale).round() as u32).max(1);
    // This runs off the draw thread and lands in PREVIEW_CACHE, so the
    // one-time cost of a quality downscale is worth crisp cells forever after.
    img.resize(width, height, image::imageops::FilterType::Lanczos3)
}

fn inside_terminal_multiplexer() -> bool {
    multiplexer_markers(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var_os("TMUX").is_some(),
    )
}

fn multiplexer_markers(term: Option<&str>, program: Option<&str>, tmux: bool) -> bool {
    tmux || term.is_some_and(|t| t.starts_with("tmux") || t.starts_with("screen"))
        || program == Some("tmux")
}

/// How long the terminal gets to answer a Device Status Report before the
/// graphics calibration is skipped for this session.
const STATUS_REPORT_PREFLIGHT: Duration = Duration::from_millis(800);
/// The crate's own query timeout once the terminal has proven it answers.
const CALIBRATION_QUERY_TIMEOUT: Duration = Duration::from_secs(2);

/// Prove the terminal answers a Device Status Report (`ESC [ 5 n`) before the
/// graphics calibration runs.
///
/// `ratatui-image` queries the terminal from a helper thread and gives up on a
/// timeout, but that helper stays blocked in `read(stdin)` and swallows the
/// first keystrokes the user types afterwards (measured 2026-09-09: every pin
/// dropped the whole first input burst under a tmux whose passthrough was
/// off). The crate ends its query on this same DSR, so a bounded `poll` for
/// the reply is the proof that the crate's query will also complete. Mirrors
/// the crate's tmux detection and passthrough wrapping so the probe travels
/// exactly the path the query will take. Runs before the event loop, so any
/// typed-ahead bytes it consumes are bounded to the pre-flight window.
#[cfg(unix)]
fn terminal_answers_status_report(timeout: Duration) -> Result<(), String> {
    use std::io::Write;
    let is_tmux = std::env::var("TERM").is_ok_and(|t| t.starts_with("tmux"))
        || std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "tmux");
    if is_tmux {
        // The same step the crate takes before its own query.
        let _ = std::process::Command::new("tmux")
            .args(["set", "-p", "allow-passthrough", "on"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status_owned();
    }
    let query: &[u8] = if is_tmux {
        b"\x1bPtmux;\x1b\x1b[5n\x1b\\"
    } else {
        b"\x1b[5n"
    };
    {
        let mut out = std::io::stdout().lock();
        out.write_all(query)
            .map_err(|e| format!("stdout write failed: {e}"))?;
        out.flush()
            .map_err(|e| format!("stdout flush failed: {e}"))?;
    }
    let deadline = std::time::Instant::now() + timeout;
    let mut seen: Vec<u8> = Vec::new();
    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "terminal did not answer a status report within {} ms{}",
                timeout.as_millis(),
                if is_tmux {
                    " (tmux passthrough off?)"
                } else {
                    ""
                }
            ));
        }
        let wait = (deadline - now).as_millis().min(i32::MAX as u128) as i32;
        let mut pfd = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut pfd, 1, wait) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("poll(stdin) failed: {err}"));
        }
        if ready == 0 {
            continue;
        }
        let mut buf = [0u8; 64];
        let n = unsafe {
            libc::read(
                libc::STDIN_FILENO,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("read(stdin) failed: {err}"));
        }
        if n == 0 {
            return Err("stdin closed during the status-report probe".into());
        }
        seen.extend_from_slice(&buf[..n as usize]);
        if status_report_answered(&seen) {
            return Ok(());
        }
        if seen.len() > 4096 {
            return Err("stdin flooded during the status-report probe".into());
        }
    }
}

#[cfg(not(unix))]
fn terminal_answers_status_report(_timeout: Duration) -> Result<(), String> {
    Ok(())
}

/// `ESC [ 0 n` (ready) and `ESC [ 3 n` (malfunction) both prove a live reply.
fn status_report_answered(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|w| w == b"\x1b[0n" || w == b"\x1b[3n")
}

fn protocol_from_env() -> Option<ProtocolType> {
    if let Ok(raw) = std::env::var("ANGEL_IMAGE_PROTOCOL") {
        let raw = raw.trim();
        if !raw.is_empty() && !raw.eq_ignore_ascii_case("auto") {
            return protocol_from_value(raw);
        }
    }
    protocol_from_terminal_hints(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        std::env::var_os("WT_SESSION").is_some(),
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
        std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some(),
    )
}

fn protocol_from_terminal_hints(
    term: Option<&str>,
    term_program: Option<&str>,
    windows_terminal: bool,
    kitty_window: bool,
    ghostty_resources: bool,
) -> Option<ProtocolType> {
    let term = term.unwrap_or_default().to_ascii_lowercase();
    let program = term_program.unwrap_or_default().to_ascii_lowercase();
    if kitty_window
        || ghostty_resources
        || term.contains("kitty")
        || program.contains("ghostty")
        || program.contains("wezterm")
    {
        Some(ProtocolType::Kitty)
    } else if windows_terminal {
        Some(ProtocolType::Sixel)
    } else if program.contains("iterm") {
        Some(ProtocolType::Iterm2)
    } else {
        None
    }
}

fn protocol_from_value(raw: &str) -> Option<ProtocolType> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        None
    } else if raw.eq_ignore_ascii_case("kitty")
        || raw == "1"
        || raw.eq_ignore_ascii_case("true")
        || raw.eq_ignore_ascii_case("on")
    {
        Some(ProtocolType::Kitty)
    } else if raw.eq_ignore_ascii_case("sixel") {
        Some(ProtocolType::Sixel)
    } else if raw.eq_ignore_ascii_case("iterm2") || raw.eq_ignore_ascii_case("iterm") {
        Some(ProtocolType::Iterm2)
    } else if raw.eq_ignore_ascii_case("halfblocks")
        || raw.eq_ignore_ascii_case("halfblock")
        || raw.eq_ignore_ascii_case("text")
        || raw == "0"
        || raw.eq_ignore_ascii_case("false")
        || raw.eq_ignore_ascii_case("off")
    {
        Some(ProtocolType::Halfblocks)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn r04c_cont2_multiplexer_markers_choose_query_free_fallback() {
        assert!(super::multiplexer_markers(
            Some("tmux-256color"),
            None,
            false
        ));
        assert!(super::multiplexer_markers(
            Some("screen-256color"),
            None,
            false
        ));
        assert!(super::multiplexer_markers(
            Some("xterm-256color"),
            None,
            true
        ));
        assert!(super::multiplexer_markers(None, Some("tmux"), false));
        assert!(!super::multiplexer_markers(
            Some("xterm-256color"),
            None,
            false
        ));
    }

    #[test]
    fn status_report_reply_is_recognised_inside_noise() {
        assert!(super::status_report_answered(b"garbage\x1b[0nmore"));
        assert!(super::status_report_answered(b"\x1b[3n"));
        assert!(!super::status_report_answered(b"\x1b[5n\x1b[?1;2c"));
        assert!(!super::status_report_answered(b""));
    }

    use super::*;
    use std::time::Duration;

    const ASYNC_IMAGE_TEST_TIMEOUT: Duration = Duration::from_secs(10);

    fn assert_responsive_draw_samples(samples: &mut [Duration], label: &str) {
        assert!(
            samples.len() >= 2,
            "{label} needs a queueing and a promotion draw"
        );
        samples.sort_unstable();
        let median = samples[samples.len() / 2];
        assert!(
            median < Duration::from_millis(50),
            "{label} blocked typical cockpit draws: median {median:?}"
        );
    }

    #[test]
    fn ambient_halfblocks_use_one_resident_encode_and_keep_map_policy() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        let _guard = crate::tests::env_lock();
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let mut terminal = Terminal::new(TestBackend::new(40, 14)).expect("test terminal");
        let composed = AtomicUsize::new(0);
        let compose = || {
            composed.fetch_add(1, Ordering::SeqCst);
            ([120, 72, 32, 255].repeat(256 * 224), 256, 224)
        };
        terminal
            .draw(|frame| {
                assert!(!viewer.render_native_location(frame, frame.area(), 77, compose));
            })
            .unwrap();
        assert_eq!(
            composed.load(Ordering::SeqCst),
            0,
            "live map still uses braille"
        );
        let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut ready = false;
        while !ready && Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    ready = viewer.render_ambient_location(frame, frame.area(), 88, compose);
                })
                .unwrap();
            if !ready {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        assert!(ready, "halfblock image worker did not finish");
        assert_eq!(composed.load(Ordering::SeqCst), 1);
        assert!(viewer.map_worker.is_some());
        assert!(viewer.map_pending.is_none());
        assert!(
            terminal.backend().buffer().content.iter().any(|cell| {
                matches!(cell.symbol(), "▀" | "▄" | "█" | " ")
                    && (matches!(cell.fg, ratatui::style::Color::Rgb(..))
                        || matches!(cell.bg, ratatui::style::Color::Rgb(..)))
            }),
            "ordinary terminal did not receive filled color cells"
        );
        for _ in 0..32 {
            terminal
                .draw(|frame| {
                    assert!(viewer.render_ambient_location(frame, frame.area(), 88, compose));
                })
                .unwrap();
        }
        assert_eq!(
            composed.load(Ordering::SeqCst),
            1,
            "cached frames must not compose again"
        );
        assert!(viewer.map_pending.is_none());

        // Hold a different admitted encode. A new destination or size must
        // return false to its current-location braille fallback, never paint
        // the previous room under the new caption while the worker catches up.
        let (_hold, rx) = mpsc::channel();
        viewer.map_pending = Some(PendingPortal {
            key: PortalCacheKey {
                sequence: 99,
                width: 40,
                height: 14,
            },
            rx,
        });
        terminal
            .draw(|frame| {
                assert!(!viewer.render_ambient_location(frame, frame.area(), 100, compose));
            })
            .unwrap();
        assert_eq!(
            composed.load(Ordering::SeqCst),
            1,
            "no obsolete jobs were queued"
        );
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| { cell.symbol() == " " && cell.bg == ratatui::style::Color::Reset }),
            "previous location was painted under a newer key"
        );
    }

    #[test]
    fn room_pixels_distinguish_pending_failed_and_empty_surfaces() {
        use ratatui::{Terminal, backend::TestBackend};
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let hold = viewer.hold_room_worker_for_test();
        let mut terminal = Terminal::new(TestBackend::new(40, 14)).unwrap();
        terminal
            .draw(|frame| {
                assert_eq!(
                    viewer.render_world_pixels(frame, frame.area(), 1, true, || (
                        vec![0u8; 1],
                        16,
                        16
                    )),
                    WorldPixelsState::Pending
                );
                assert_eq!(
                    viewer.render_world_pixels::<Vec<u8>, _>(
                        frame,
                        frame.area(),
                        2,
                        true,
                        || panic!("coalesce obsolete scene")
                    ),
                    WorldPixelsState::Pending
                );
                assert_eq!(
                    viewer.render_world_pixels::<Vec<u8>, _>(
                        frame,
                        Rect::default(),
                        1,
                        true,
                        || panic!("empty area")
                    ),
                    WorldPixelsState::Unavailable
                );
            })
            .unwrap();
        drop(hold);
        let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        while viewer.map_failure.is_none() && std::time::Instant::now() < deadline {
            viewer.promote_pending_map();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(viewer.map_failure.is_some());
        terminal
            .draw(|frame| {
                assert_eq!(
                    viewer.render_world_pixels::<Vec<u8>, _>(
                        frame,
                        frame.area(),
                        1,
                        true,
                        || panic!("failed frame must not retry")
                    ),
                    WorldPixelsState::Unavailable
                );
            })
            .unwrap();
    }

    #[test]
    fn protocol_env_values_select_real_inline_backends() {
        assert_eq!(protocol_from_value("kitty"), Some(ProtocolType::Kitty));
        assert_eq!(protocol_from_value("sixel"), Some(ProtocolType::Sixel));
        assert_eq!(
            protocol_from_value("halfblocks"),
            Some(ProtocolType::Halfblocks)
        );
        assert_eq!(protocol_from_value("auto"), None);
    }

    #[test]
    fn trusted_terminal_hints_select_resident_pixel_protocols() {
        assert_eq!(
            protocol_from_terminal_hints(Some("xterm-kitty"), None, false, false, false),
            Some(ProtocolType::Kitty)
        );
        assert_eq!(
            protocol_from_terminal_hints(
                Some("xterm-256color"),
                Some("ghostty"),
                false,
                false,
                false,
            ),
            Some(ProtocolType::Kitty)
        );
        assert_eq!(
            protocol_from_terminal_hints(Some("xterm-256color"), None, true, false, false,),
            Some(ProtocolType::Sixel)
        );
        assert_eq!(
            protocol_from_terminal_hints(Some("xterm-256color"), None, false, false, false,),
            None
        );
    }

    #[test]
    fn static_preview_viewer_starts_empty() {
        let viewer = Viewer::static_preview();
        assert!(!viewer.has_image());
        assert_eq!(viewer.label(), None);
        assert!(!viewer.supports_agentviz_portal());
    }

    #[test]
    fn kitty_portal_frame_is_encoded_off_thread_and_cached() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::time::Instant;

        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let mut viewer = Viewer::with_picker(picker);
        assert!(viewer.supports_agentviz_portal());
        let portal_frame = crate::agentviz_portal::PortalFrame {
            sequence: 77,
            pixels: Arc::from(vec![32; crate::agentviz_portal::FRAME_BYTES]),
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
        let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut ready = false;
        while !ready && Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    ready = viewer.render_agentviz_portal(frame, frame.area(), &portal_frame);
                })
                .expect("render portal");
            if !ready {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(ready, "Kitty portal adapter did not finish");
        assert!(viewer.portal_current.is_some());
        assert!(!viewer.agentviz_portal_pending());
    }

    #[test]
    fn changing_portal_and_map_sequences_coalesce_behind_one_pending_encode() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let mut viewer = Viewer::with_picker(picker);
        let old_key = PortalCacheKey {
            sequence: 1,
            width: 40,
            height: 10,
        };

        let (_portal_hold, portal_rx) = mpsc::channel();
        viewer.portal_pending = Some(PendingPortal {
            key: old_key.clone(),
            rx: portal_rx,
        });
        let portal_frame = crate::agentviz_portal::PortalFrame {
            sequence: 2,
            pixels: Arc::from(vec![32; crate::agentviz_portal::FRAME_BYTES]),
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
        terminal
            .draw(|frame| {
                assert!(!viewer.render_agentviz_portal(frame, frame.area(), &portal_frame));
            })
            .expect("render coalesced portal");
        assert_eq!(
            viewer.portal_pending.as_ref().map(|pending| &pending.key),
            Some(&old_key),
            "a newer portal sequence replaced the admitted encode"
        );

        let (_map_hold, map_rx) = mpsc::channel();
        viewer.map_pending = Some(PendingPortal {
            key: old_key.clone(),
            rx: map_rx,
        });
        let composed = AtomicUsize::new(0);
        terminal
            .draw(|frame| {
                assert!(!viewer.render_native_location(frame, frame.area(), 2, || {
                    composed.fetch_add(1, Ordering::Relaxed);
                    (vec![0u8; 16 * 8 * 4], 16, 8)
                },));
            })
            .expect("render coalesced map");
        assert_eq!(composed.load(Ordering::Relaxed), 0);
        assert_eq!(
            viewer.map_pending.as_ref().map(|pending| &pending.key),
            Some(&old_key),
            "a newer map sequence queued behind the admitted encode"
        );
    }

    #[test]
    fn kitty_map_composes_off_the_draw_thread_on_one_resident_worker() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::time::Instant;

        // Byte access is where the lazy compose runs; record which thread
        // takes it so the test pins compose placement, not just encoding.
        struct LazyPixels {
            bytes: Vec<u8>,
            seen: Arc<Mutex<Vec<std::thread::ThreadId>>>,
        }
        impl AsRef<[u8]> for LazyPixels {
            fn as_ref(&self) -> &[u8] {
                self.seen.lock().unwrap().push(std::thread::current().id());
                &self.bytes
            }
        }

        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        let mut viewer = Viewer::with_picker(picker);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).expect("test terminal");
        for sequence in [1u64, 2] {
            let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
            let mut ready = false;
            while !ready && Instant::now() < deadline {
                terminal
                    .draw(|frame| {
                        let seen = Arc::clone(&seen);
                        ready = viewer.render_native_location(
                            frame,
                            frame.area(),
                            sequence,
                            move || {
                                let bytes = vec![9; 16 * 8 * 4];
                                (LazyPixels { bytes, seen }, 16, 8)
                            },
                        );
                    })
                    .expect("render world map");
                if !ready {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            assert!(ready, "map frame for sequence {sequence} never landed");
        }
        assert!(viewer.map_current.is_some());
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "each sequence miss composes exactly once");
        let draw_thread = std::thread::current().id();
        assert!(
            seen.iter().all(|id| *id != draw_thread),
            "map compose ran on the draw thread"
        );
        assert_eq!(seen[0], seen[1], "misses did not reuse one worker thread");
    }

    #[test]
    fn portrait_character_protocol_is_anchored_to_the_bottom_right() {
        let picker = Picker::halfblocks();
        let protocol = picker
            .new_protocol(
                image::DynamicImage::ImageRgba8(image::RgbaImage::new(32, 32)),
                Rect::new(0, 0, 20, 12).into(),
                Resize::Fit(None),
            )
            .unwrap();
        let available = Rect::new(7, 9, 40, 25);
        let placed = bottom_right_protocol_area(&protocol, available);
        assert_eq!(placed.right(), available.right());
        assert_eq!(placed.bottom(), available.bottom());
        assert!(
            placed.y > available.y,
            "spare height belongs above the character"
        );
    }

    #[test]
    fn portrait_preview_renders_bundled_asset_as_halfblocks_and_reuses_cache() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/sparky-neutral.png");
        let mut viewer = Viewer::portrait_preview();
        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        for _ in 0..2 {
            terminal
                .draw(|frame| {
                    assert!(viewer.render_portrait(frame, frame.area(), &path));
                })
                .expect("render portrait");
        }
        assert_eq!(viewer.portrait_cache.len(), 1);
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .any(|cell| matches!(cell.symbol(), "▀" | "▄" | "█")),
            "half-block portrait should paint terminal-native image cells"
        );
    }

    #[test]
    fn production_portrait_preparation_never_blocks_the_draw_thread() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::time::{Duration, Instant};

        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/codex-active-v2.png");
        let mut viewer = Viewer::with_picker(Picker::halfblocks());
        viewer.portrait_enabled = true;
        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut visible = false;
        let mut draw_samples = Vec::new();
        while Instant::now() < deadline && !visible {
            let started = Instant::now();
            terminal
                .draw(|frame| {
                    visible = viewer.render_portrait(frame, frame.area(), &path);
                })
                .expect("render async portrait");
            draw_samples.push(started.elapsed());
            if !visible {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        assert!(visible, "async agent portrait never became visible");
        assert_responsive_draw_samples(&mut draw_samples, "portrait preparation");
    }

    #[test]
    fn portrait_prefetch_makes_the_first_opposite_state_frame_cache_hot() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use std::time::{Duration, Instant};

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
        let neutral = root.join("sparky-neutral.png");
        let active = root.join("sparky-active.png");
        let mut viewer = Viewer::with_picker(Picker::halfblocks());
        viewer.portrait_enabled = true;
        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        let deadline = Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut neutral_visible = false;
        while Instant::now() < deadline
            && !viewer
                .portrait_cache
                .iter()
                .any(|entry| entry.key.path == active)
        {
            terminal
                .draw(|frame| {
                    neutral_visible = viewer.render_portrait(frame, frame.area(), &neutral);
                })
                .expect("render neutral portrait");
            if neutral_visible {
                viewer.prefetch_portrait(Rect::new(0, 0, 28, 14), &active);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(neutral_visible, "neutral portrait never became visible");
        assert!(
            viewer
                .portrait_cache
                .iter()
                .any(|entry| entry.key.path == active)
        );

        let started = Instant::now();
        terminal
            .draw(|frame| {
                assert!(viewer.render_portrait(frame, frame.area(), &active));
            })
            .expect("render prefetched active portrait");
        assert!(
            started.elapsed() < Duration::from_millis(50),
            "a prefetched active portrait should paint on its first frame"
        );
    }

    #[test]
    fn synchronous_portrait_preview_skips_opposite_state_prefetch() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
        let neutral = root.join("sparky-neutral.png");
        let active = root.join("sparky-active.png");
        let mut viewer = Viewer::portrait_preview();
        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        terminal
            .draw(|frame| {
                assert!(viewer.render_portrait(frame, frame.area(), &neutral));
                viewer.prefetch_portrait(frame.area(), &active);
            })
            .expect("render static portrait preview");
        assert_eq!(viewer.portrait_cache.len(), 1);
        assert!(viewer.portrait_pending.is_none());
    }

    #[test]
    fn visible_portrait_request_supersedes_a_different_prefetch() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents");
        let speculative = root.join("sparky-active.png");
        let visible = root.join("atlas-neutral.png");
        let area = Rect::new(0, 0, 28, 14);
        let mut viewer = Viewer::with_picker(Picker::halfblocks());
        viewer.portrait_enabled = true;
        viewer.prefetch_portrait(area, &speculative);
        assert!(
            viewer
                .portrait_pending
                .as_ref()
                .is_some_and(|pending| pending.prefetch)
        );

        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        terminal
            .draw(|frame| {
                assert!(!viewer.render_portrait(frame, frame.area(), &visible));
            })
            .expect("queue visible portrait");
        let pending = viewer
            .portrait_pending
            .as_ref()
            .expect("visible portrait worker");
        assert_eq!(pending.key.path, visible);
        assert!(!pending.prefetch);
    }

    #[test]
    fn helm_pending_pose_retains_its_agent_and_never_borrows_another_identity() {
        use ratatui::{Terminal, backend::TestBackend};
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = root.join(crate::helm::sheet(crate::agent_profile::AgentKey::Turbo));
        let other = root.join(crate::helm::sheet(crate::agent_profile::AgentKey::Atlas));
        let mut viewer = Viewer::portrait_preview();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        terminal
            .draw(|frame| assert!(viewer.render_helm(frame, frame.area(), &path, 0)))
            .unwrap();
        let first = terminal.backend().buffer().clone();
        viewer.portrait_synchronous = false;
        let mut key = Viewer::portrait_key(Rect::new(0, 0, 20, 10), &path);
        key.pose = Some(3);
        let (held, rx) = mpsc::channel();
        viewer.portrait_pending = Some(PendingPortrait {
            key: key.clone(),
            rx,
            prefetch: false,
        });
        for pose in 1..8 {
            terminal
                .draw(|frame| assert!(viewer.render_helm(frame, frame.area(), &path, pose)))
                .unwrap();
            assert_eq!(
                terminal.backend().buffer(),
                &first,
                "no ASCII/empty flash between poses"
            );
            assert_eq!(
                viewer.portrait_pending.as_ref().unwrap().key,
                key,
                "one admitted decode"
            );
        }
        terminal
            .draw(|frame| assert!(!viewer.render_helm(frame, frame.area(), &other, 0)))
            .unwrap();
        assert!(
            viewer
                .portrait_cache
                .iter()
                .all(|entry| entry.key.path != other)
        );
        drop(held);
    }

    #[test]
    fn generated_world_dots_hold_frames_and_reject_old_room_or_geometry_completions() {
        use crate::terminal_art::{ColoredBrailleCell, ColoredBrailleImage};
        use ratatui::{Terminal, backend::TestBackend};
        let _guard = crate::tests::env_lock();
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((8, 16).into());
        picker.set_protocol_type(ProtocolType::Kitty);
        let mut viewer = Viewer::with_picker(picker);
        let area = Rect::new(2, 2, 12, 6);
        let geometry = crate::dot_canvas::DotGeometry::new(12, 6, (8, 16), 2).unwrap();
        let image = Arc::new(ColoredBrailleImage {
            width: geometry.grid_width,
            height: geometry.grid_height,
            cells: vec![
                ColoredBrailleCell {
                    glyph: '\u{28ff}',
                    fg: [80, 140, 160]
                };
                geometry.grid_width * geometry.grid_height
            ],
        });
        let mut terminal = Terminal::new(TestBackend::new(20, 12)).unwrap();
        let hold = viewer.hold_room_worker_for_test();
        terminal
            .draw(|frame| {
                assert!(!viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&image)))
            })
            .unwrap();
        assert!(viewer.dot_pending.is_some());
        drop(hold);
        let start = std::time::Instant::now();
        while viewer.dot_current.is_none() && start.elapsed() < Duration::from_secs(3) {
            terminal
                .draw(|frame| {
                    viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&image));
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        let first = viewer
            .dot_current
            .as_ref()
            .expect("completed generated canvas")
            .key
            .clone();
        assert_eq!(
            viewer.dot_current.as_ref().unwrap().protocol.size(),
            Rect::new(0, 0, area.width, area.height).into()
        );
        let hold = viewer.hold_room_worker_for_test();
        let mut next = (*image).clone();
        next.cells[0].fg = [170, 90, 40];
        let next = Arc::new(next);
        for _ in 0..20 {
            terminal
                .draw(|frame| {
                    assert!(viewer.render_world_dots(frame, area, 1, geometry, Arc::clone(&next)))
                })
                .unwrap();
            assert_eq!(
                viewer.dot_current.as_ref().unwrap().key,
                first,
                "retain exact last completed frame"
            );
        }
        terminal
            .draw(|frame| {
                assert!(!viewer.render_world_dots(frame, area, 2, geometry, Arc::clone(&image)))
            })
            .unwrap();
        assert!(
            viewer.dot_current.is_none(),
            "old room must disappear immediately"
        );
        assert_eq!(
            viewer.dot_pending.as_ref().unwrap().key.scene,
            1,
            "do not spawn obsolete work while pending"
        );
        drop(hold);
        let start = std::time::Instant::now();
        while viewer.dot_current.is_none() && start.elapsed() < Duration::from_secs(3) {
            terminal
                .draw(|frame| {
                    viewer.render_world_dots(frame, area, 2, geometry, Arc::clone(&image));
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(viewer.dot_current.as_ref().unwrap().key.scene, 2);
        let smaller = Rect::new(2, 2, 10, 5);
        let resized = crate::dot_canvas::DotGeometry::new(10, 5, (8, 16), 2).unwrap();
        terminal
            .draw(|frame| {
                assert!(!viewer.render_world_dots(frame, smaller, 2, resized, Arc::clone(&image)))
            })
            .unwrap();
        assert!(
            viewer.dot_current.is_none(),
            "old geometry cannot cover adjacent text"
        );
    }

    #[test]
    fn fine_dot_pitch_has_a_portable_text_fallback() {
        assert_eq!(dot_pitch(None), Some(2));
        assert_eq!(dot_pitch(Some("3")), Some(3));
        assert_eq!(dot_pitch(Some("0")), None);
        assert_eq!(dot_pitch(Some("text")), None);
        assert_eq!(dot_pitch(Some("huge")), Some(2));
        assert!(
            Viewer::static_preview()
                .dot_geometry(Rect::new(0, 0, 40, 20))
                .is_none()
        );
    }

    #[test]
    fn missing_portrait_is_negatively_cached_instead_of_retried_each_frame() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/definitely-missing.png");
        let mut viewer = Viewer::portrait_preview();
        let mut terminal = Terminal::new(TestBackend::new(28, 14)).expect("test terminal");
        for _ in 0..2 {
            terminal
                .draw(|frame| {
                    assert!(!viewer.render_portrait(frame, frame.area(), &path));
                })
                .expect("render missing portrait fallback");
        }
        assert_eq!(viewer.portrait_failures.len(), 1);
        assert!(viewer.portrait_cache.is_empty());
    }
}

#[cfg(test)]
mod map_gate_tests {
    use super::*;

    #[test]
    fn native_video_iterm_payload_retains_details_above_halfblock_resolution() {
        use base64::Engine as _;
        use ratatui::{Terminal, backend::TestBackend};
        let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Iterm2));
        assert_eq!(
            viewer.video_decode_viewport(Rect::new(0, 0, 72, 24)),
            (192, 64)
        );
        let pixels = Arc::new(crate::scryglass::VideoPixels {
            identity: crate::scryglass::VideoIdentity {
                source: crate::media::MediaSource::operator(PathBuf::from("controlled-detail.mp4")),
                request_id: 1,
                generation: 1,
            },
            sequence: 1,
            rgba: image::RgbaImage::from_fn(320, 200, |x, _| {
                let value = if x / 4 % 2 == 0 { 0 } else { 255 };
                image::Rgba([value, value, value, 255])
            }),
        });
        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut ready = false;
        while !ready && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    ready = viewer
                        .render_video(frame, frame.area(), Arc::clone(&pixels))
                        .unwrap()
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ready);
        let sequence = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .find(|symbol| symbol.contains("]1337;File="))
            .unwrap();
        let payload = sequence
            .split_once("]1337;File=")
            .unwrap()
            .1
            .split_once(':')
            .unwrap()
            .1
            .split('\u{7}')
            .next()
            .unwrap();
        let decoded = image::load_from_memory(
            &base64::engine::general_purpose::STANDARD
                .decode(payload)
                .unwrap(),
        )
        .unwrap()
        .to_rgba8();
        assert!(decoded.width() > 300 && decoded.height() > 150);
        assert!(decoded.width() * decoded.height() <= MAX_INLINE_PREVIEW_PIXELS);
        assert!(sequence.len() < 700_000);
        let row: Vec<_> = (0..decoded.width())
            .map(|x| decoded.get_pixel(x, decoded.height() / 2)[0] > 127)
            .collect();
        let transitions = row.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(
            transitions >= 70,
            "{transitions} transitions: a 40-column halfblock grid would lose this detail"
        );
        eprintln!(
            "native iTerm2 worker payload={}x{} transitions={transitions} bytes={}",
            decoded.width(),
            decoded.height(),
            sequence.len()
        );
    }

    #[test]
    fn native_video_pixels_are_bounded_exact_and_coalesce_without_stale_generations() {
        use ratatui::{Terminal, backend::TestBackend};
        let pixels = |generation, sequence, color| {
            Arc::new(crate::scryglass::VideoPixels {
                identity: crate::scryglass::VideoIdentity {
                    source: crate::media::MediaSource::operator(PathBuf::from("controlled.mp4")),
                    request_id: 7,
                    generation,
                },
                sequence,
                rgba: image::RgbaImage::from_pixel(32, 24, image::Rgba(color)),
            })
        };
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
        let red = pixels(1, 1, [230, 10, 20, 255]);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut ready = false;
        while !ready && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    ready = viewer
                        .render_video(frame, frame.area(), Arc::clone(&red))
                        .unwrap()
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ready);
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| { cell.fg == ratatui::style::Color::Rgb(230, 10, 20) }),
            "decoded color must reach actual terminal cells"
        );
        let (tx, rx) = mpsc::channel();
        let pending_key = viewer.video.as_ref().unwrap().0.clone();
        viewer.video_pending = Some(PendingVideo {
            key: pending_key.clone(),
            sequence: 2,
            rx,
        });
        terminal
            .draw(|frame| {
                assert!(
                    viewer
                        .render_video(frame, frame.area(), pixels(1, 99, [10, 230, 20, 255]))
                        .unwrap(),
                    "same-generation frame remains during a slow encode"
                );
            })
            .unwrap();
        assert_eq!(
            viewer.video_pending.as_ref().unwrap().sequence,
            2,
            "latest frames do not queue behind pending work"
        );
        for (generation, width, request_id, source) in [
            (2, 40, 7, "controlled.mp4"),
            (1, 20, 7, "controlled.mp4"),
            (1, 40, 8, "controlled.mp4"),
            (1, 40, 7, "other.mp4"),
        ] {
            let mut changed = pixels(generation, 3, [10, 230, 20, 255]);
            let identity = &mut Arc::get_mut(&mut changed).unwrap().identity;
            identity.request_id = request_id;
            identity.source.path = PathBuf::from(source);
            terminal
                .draw(|frame| {
                    assert!(
                        !viewer
                            .render_video(frame, Rect::new(0, 0, width, 16), changed.clone())
                            .unwrap()
                    );
                })
                .unwrap();
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .all(|cell| cell.symbol() == " "),
                "seek/reveal or resize must never paint an old protocol"
            );
        }
        drop(tx);
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_video(frame, frame.area(), pixels(2, 4, [10, 230, 20, 255]))
                        .unwrap()
                );
            })
            .unwrap();
        assert_eq!(
            viewer
                .video_pending
                .as_ref()
                .unwrap()
                .key
                .identity
                .generation,
            2
        );

        for protocol in [
            ProtocolType::Halfblocks,
            ProtocolType::Kitty,
            ProtocolType::Iterm2,
            ProtocolType::Sixel,
        ] {
            let picker = Viewer::picker_for_protocol(protocol);
            let area = video_encode_area(&picker, 16, 16, Rect::new(0, 0, 192, 64));
            let font = picker.font_size();
            let pw = u32::from(area.width) * u32::from(font.width);
            let ph = u32::from(area.height) * u32::from(font.height);
            assert!(pw * ph <= MAX_INLINE_PREVIEW_PIXELS);
            assert!(
                pw.abs_diff(ph) <= u32::from(font.width.max(font.height)),
                "square media stays square in physical pixels"
            );
            picker
                .new_protocol(
                    image::DynamicImage::ImageRgba8(red.rgba.clone()),
                    area.into(),
                    Resize::Scale(Some(image::imageops::FilterType::Triangle)),
                )
                .unwrap();
        }
    }

    #[test]
    fn the_tile_map_accepts_every_real_graphics_protocol() {
        // This gate was originally copied from the WebGPU portal, which is
        // Kitty-only on purpose. That left the map dark on sixel terminals —
        // the protocol this project's own sessions actually negotiate — for no
        // reason, because a composed frame is just pixels.
        for protocol in [
            ProtocolType::Kitty,
            ProtocolType::Sixel,
            ProtocolType::Iterm2,
        ] {
            assert!(map_supported(protocol), "{protocol:?} should carry the map");
        }
    }

    #[test]
    fn sixel_prefers_living_braille_unless_backed_map_forced() {
        // Capability stays open (operator can force plates), but the default
        // picture on sixel is the living braille world — static sixel plates
        // were reading as a broken still.
        let _lock = crate::tests::env_lock();
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_BACKED_MAP") };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_TILE_MAP") };
        assert!(map_supported(ProtocolType::Sixel));
        assert!(!map_desired(ProtocolType::Sixel));
        assert!(map_desired(ProtocolType::Kitty));
        assert!(map_desired(ProtocolType::Iterm2));
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_BACKED_MAP", "1") };
        assert!(map_desired(ProtocolType::Sixel));
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_BACKED_MAP", "0") };
        assert!(!map_desired(ProtocolType::Kitty));
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_BACKED_MAP") };
    }

    #[test]
    fn halfblocks_falls_back_to_the_braille_map() {
        // Excluded on resolution, not capability: one pixel per cell half
        // cannot hold a 512px island, and braille reads better than mush.
        assert!(!map_supported(ProtocolType::Halfblocks));
    }

    #[test]
    fn artifact_still_pixels_preserve_portrait_rgb_flat_opacity_and_fail_closed() {
        use ratatui::{Terminal, backend::TestBackend, style::Color};
        let root = std::env::temp_dir().join(format!("angel-artifact-rgb-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let color = [34, 153, 221];
        for (name, alpha) in [
            ("flat.png", 255),
            ("transparent.png", 0),
            ("partial.png", 128),
        ] {
            image::RgbaImage::from_pixel(
                16,
                16,
                image::Rgba([color[0], color[1], color[2], alpha]),
            )
            .save(root.join(name))
            .unwrap();
        }
        std::fs::write(root.join("corrupt.png"), b"not a png").unwrap();
        let portrait =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/agents/apollo-neutral.png");
        for (index, path) in [
            portrait,
            root.join("flat.png"),
            root.join("transparent.png"),
            root.join("partial.png"),
            root.join("corrupt.png"),
        ]
        .into_iter()
        .enumerate()
        {
            let source = crate::media::MediaSource::operator(path);
            let mut viewer = Viewer::with_picker(halfblock_picker());
            let (width, height) = if index == 0 { (52, 20) } else { (8, 4) };
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for area in [Rect::new(0, 0, 0, height), Rect::new(0, 0, width, 0)] {
                terminal
                    .draw(|frame| {
                        assert!(
                            !viewer
                                .render_artifact(frame, area, source.clone(), 1)
                                .unwrap()
                        )
                    })
                    .unwrap();
                assert!(
                    viewer.artifact_pending.is_none(),
                    "empty geometry must not start decoding"
                );
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut result = Ok(false);
            while matches!(result, Ok(false)) && std::time::Instant::now() < deadline {
                terminal
                    .draw(|frame| {
                        result = viewer.render_artifact(frame, frame.area(), source.clone(), 1)
                    })
                    .unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
            let cells = &terminal.backend().buffer().content;
            if index == 4 {
                assert!(result.is_err(), "corrupt requested image must fail");
                assert!(cells.iter().all(|cell| cell.symbol() == " "));
                continue;
            }
            assert_eq!(
                result,
                Ok(true),
                "actual artifact pixels must finish encoding"
            );
            let source_color = Color::Rgb(color[0], color[1], color[2]);
            match index {
                0 => {
                    assert!(
                        cells.iter().filter(|cell| cell.symbol() != " ").count() > cells.len() / 4,
                        "portrait must remain dense and legible"
                    );
                    assert!(cells.iter().any(|cell| [cell.fg, cell.bg].iter().any(|color| matches!(color, Color::Rgb(r, g, b) if !crate::terminal_art::DMD_PALETTE.contains(&[*r, *g, *b])))), "artifact RGB must not collapse to the DMD palette");
                }
                1 => assert!(
                    // Uniform halfblocks use a colored space: its background
                    // still covers the whole terminal cell opaquely.
                    cells
                        .iter()
                        .all(|cell| cell.fg == source_color && cell.bg == source_color),
                    "opaque flat image must fill its exact-fit raster with source RGB"
                ),
                2 => assert!(
                    cells.iter().all(
                        |cell| cell.fg == Color::Rgb(0, 0, 0) && cell.bg == Color::Rgb(0, 0, 0)
                    ),
                    "transparent RGB must contribute no ink to the black canvas"
                ),
                3 => assert!(
                    cells.iter().all(|cell| cell.fg == Color::Rgb(17, 76, 110)
                        && cell.bg == Color::Rgb(17, 76, 110)),
                    "partial alpha must blend exact source channels against black"
                ),
                _ => unreachable!(),
            }
        }
        use base64::Engine as _;
        let source = crate::media::MediaSource::operator(root.join("partial.png"));
        let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Iterm2));
        let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut painted = false;
        while !painted && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    painted = viewer
                        .render_artifact_with_picker(
                            frame,
                            frame.area(),
                            source.clone(),
                            1,
                            still_pixel_picker(ProtocolType::Iterm2, Some((10, 20))),
                        )
                        .unwrap()
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(painted);
        let sequence = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .find(|symbol| symbol.contains("]1337;File="))
            .unwrap();
        let payload = sequence
            .split_once("]1337;File=")
            .unwrap()
            .1
            .split_once(':')
            .unwrap()
            .1
            .split('\u{7}')
            .next()
            .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .unwrap();
        let native = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert!(
            native.pixels().all(|pixel| pixel.0 == [34, 153, 221, 128]),
            "native artifact payload must preserve source RGB and alpha"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn still_inspector_native_geometry_pending_continuity_and_failure() {
        use crate::still_inspector::Action;
        use ratatui::{Terminal, backend::TestBackend};
        let source =
            crate::media::MediaSource::operator(PathBuf::from("not-opened-geometry-test.png"));
        let decoded = Arc::new(image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(112, 112, image::Rgba([240, 60, 30, 255])),
        ));
        let mut viewer = Viewer::with_picker(Viewer::picker_for_protocol(ProtocolType::Kitty));
        let mut terminal = Terminal::new(TestBackend::new(16, 8)).unwrap();
        let scene = Rect::new(2, 1, 8, 4);
        // Prime identity/source, not protocol; the real worker encodes with the
        // same ioctl-derived picker that defines the cache and pointer mapping.
        viewer.artifact_identity = Some((source.clone(), 1));
        viewer.artifact_decoded = Some(decoded);
        let draw = |viewer: &mut Viewer, terminal: &mut Terminal<TestBackend>, font| {
            let mut ready = false;
            terminal
                .draw(|f| {
                    ready = viewer
                        .render_artifact_with_picker(
                            f,
                            scene,
                            source.clone(),
                            1,
                            still_pixel_picker(ProtocolType::Kitty, font),
                        )
                        .unwrap()
                })
                .unwrap();
            ready
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !draw(&mut viewer, &mut terminal, Some((14, 28))) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(viewer.inspector.pixels, (112, 112));
        assert_eq!(viewer.inspector.pointer(2, 1), (7.0, 14.0));
        let symbols: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(
            symbols.contains("s=112,v=112"),
            "native payload must match actual cells, not 10x20"
        );
        let original = viewer.artifact.as_ref().unwrap().0.clone();
        let original_view = viewer.inspector.painted;
        viewer.inspector.action(Action::ZoomIn);
        let mut blocked = original.clone();
        blocked.view = viewer.inspector.view;
        let (tx, rx) = mpsc::channel();
        viewer.artifact_pending = Some(PendingArtifact { key: blocked, rx });
        viewer.inspector.drag = Some((4, 2));
        for _ in 0..6 {
            viewer.inspector.zoom(true, (56.0, 56.0));
            assert!(!draw(&mut viewer, &mut terminal, Some((14, 28))));
            assert_eq!(viewer.inspector.viewport, Some(scene));
            assert_eq!(viewer.inspector.painted, original_view);
            assert!(viewer.inspector.loading);
            assert_eq!(viewer.inspector.drag, Some((4, 2)));
            assert_eq!(viewer.artifact.as_ref().unwrap().0, original);
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .any(|c| c.symbol().contains('\u{10eeee}'))
            );
        }
        // Any same-scene encoder failure cancels old authority, even if desired
        // view advanced. No stale-success ready flag and no faulted old image.
        tx.send(Err("controlled encoder failure".into())).unwrap();
        terminal
            .draw(|f| {
                assert!(
                    viewer
                        .render_artifact_with_picker(
                            f,
                            scene,
                            source.clone(),
                            1,
                            still_pixel_picker(ProtocolType::Kitty, Some((14, 28)))
                        )
                        .is_err()
                )
            })
            .unwrap();
        assert!(viewer.inspector.viewport.is_none());
        assert!(viewer.inspector.painted.is_none());
        assert!(viewer.inspector.drag.is_none());
        assert!(viewer.artifact.is_none());

        // Explicit resize epoch prevents A→B→A resurrecting a pending protocol.
        viewer.artifact_identity = Some((source.clone(), 1));
        let (tx, rx) = mpsc::channel();
        viewer.artifact_pending = Some(PendingArtifact {
            key: original.clone(),
            rx,
        });
        viewer.invalidate_still_layout();
        assert!(!draw(&mut viewer, &mut terminal, Some((16, 32))));
        assert!(viewer.inspector.viewport.is_none());
        assert_eq!(viewer.inspector.pixels, (128, 128));
        assert!(!draw(&mut viewer, &mut terminal, Some((14, 28))));
        assert_ne!(viewer.still_layout_epoch, original.layout_epoch);
        assert!(!draw(&mut viewer, &mut terminal, None));
        assert_eq!(viewer.inspector.pixels, (16, 16));
        assert_eq!(viewer.still_protocol, Some(ProtocolType::Halfblocks));
        assert!(viewer.inspector.viewport.is_none());
        drop(tx);
        viewer.clear_still();
    }

    #[test]
    fn still_inspector_source_decode_once_coalesces_and_cleans_error_identity_resize() {
        use crate::still_inspector::Action;
        use ratatui::{Terminal, backend::TestBackend};
        let root = std::env::temp_dir().join(format!(
            "angel-still-detail-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("detail.png");
        let mut raster = image::RgbaImage::from_pixel(2048, 2048, image::Rgba([27, 41, 53, 255]));
        raster.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        raster.put_pixel(2047, 2047, image::Rgba([0, 255, 0, 255]));
        raster.save(&path).unwrap();
        let source = crate::media::MediaSource {
            path,
            root: Some(root.clone()),
        };
        let decoded = artifact_preview(&source).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (2048, 2048),
            "decoder must not return the 120k thumbnail"
        );
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [255, 0, 0, 255]);
        drop(decoded);
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
        fn ready(
            viewer: &mut Viewer,
            terminal: &mut Terminal<TestBackend>,
            area: Rect,
            source: &crate::media::MediaSource,
            request: u64,
        ) {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                let mut painted = false;
                terminal
                    .draw(|f| {
                        painted = viewer
                            .render_artifact(f, area, source.clone(), request)
                            .unwrap()
                    })
                    .unwrap();
                if painted {
                    break;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let area = Rect::new(2, 3, 48, 16);
        ready(&mut viewer, &mut terminal, area, &source, 1);
        let allocation = Arc::as_ptr(viewer.artifact_decoded.as_ref().unwrap());
        // Once opened, gestures and resize must succeed even with file removed.
        std::fs::remove_file(&source.path).unwrap();
        viewer.inspector.action(Action::ZoomIn);
        terminal
            .draw(|f| assert!(!viewer.render_artifact(f, area, source.clone(), 1).unwrap()))
            .unwrap();
        let pending = viewer.artifact_pending.as_ref().unwrap().key.clone();
        assert!(viewer.inspector.viewport.is_some());
        assert!(viewer.inspector.loading);
        assert_ne!(viewer.inspector.painted, Some(viewer.inspector.view));
        for _ in 0..12 {
            viewer.inspector.action(Action::ZoomIn);
        }
        viewer.inspector.action(Action::Right);
        viewer.inspector.drag = Some((10, 10));
        let resized = Rect::new(4, 2, 40, 20);
        ready(&mut viewer, &mut terminal, resized, &source, 1);
        assert!(viewer.inspector.drag.is_none());
        assert_eq!(viewer.inspector.viewport, Some(resized));
        let admitted = &viewer.artifact.as_ref().unwrap().0;
        assert_ne!(admitted, &pending);
        assert_eq!(admitted.view, viewer.inspector.view);
        assert_eq!(admitted.scene, resized);
        assert_eq!(
            Arc::as_ptr(viewer.artifact_decoded.as_ref().unwrap()),
            allocation,
            "all gestures reuse the single source allocation"
        );
        assert_eq!(
            Arc::strong_count(viewer.artifact_decoded.as_ref().unwrap()),
            1
        );
        // Same path + new request must NOT use cached decoded data. Missing file
        // produces an error and clears every input/pixel/cache authority.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut fault = None;
        while fault.is_none() && std::time::Instant::now() < deadline {
            terminal
                .draw(|f| fault = viewer.render_artifact(f, resized, source.clone(), 2).err())
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(fault.is_some());
        assert!(viewer.artifact_decoded.is_none());
        assert!(viewer.artifact.is_none());
        assert!(viewer.inspector.source.is_none());
        assert!(viewer.inspector.viewport.is_none());
        assert!(viewer.inspector.drag.is_none());
        assert!(viewer.artifact_identity.is_none());
        terminal
            .draw(|f| {
                assert!(
                    !viewer
                        .render_artifact(f, Rect::default(), source.clone(), 3)
                        .unwrap()
                )
            })
            .unwrap();
        assert!(viewer.artifact_pending.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_artifact_pixels_preserve_source_resize_and_failure_identity() {
        use ratatui::{Terminal, backend::TestBackend};
        const ASYNC_IMAGE_TEST_TIMEOUT: Duration = Duration::from_secs(10);
        let root = std::env::temp_dir().join(format!(
            "angel-artifact-pixels-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("requested.png");
        image::RgbaImage::from_pixel(24, 16, image::Rgba([230, 40, 20, 255]))
            .save(&path)
            .unwrap();
        let source = crate::media::MediaSource {
            path,
            root: Some(root.clone()),
        };
        let decoded = artifact_preview(&source).unwrap();
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0).0, [230, 40, 20, 255]);
        let mut viewer = Viewer::with_picker(halfblock_picker());
        let mut terminal = Terminal::new(TestBackend::new(32, 16)).unwrap();
        // Browsing cannot abandon a running decoder and launch another one.
        let (blocked_tx, blocked_rx) = mpsc::channel();
        let blocked_key = ArtifactKey {
            request_id: 0,
            source: source.clone(),
            width: 32,
            height: 8,
            scene: Rect::new(0, 0, 32, 8),
            font: HALFBLOCK_FONT_SIZE,
            protocol: ProtocolType::Halfblocks,
            layout_epoch: 0,
            view: Default::default(),
        };
        viewer.artifact_pending = Some(PendingArtifact {
            key: blocked_key.clone(),
            rx: blocked_rx,
        });
        for width in [8, 16, 32] {
            terminal
                .draw(|frame| {
                    assert!(
                        !viewer
                            .render_artifact(frame, Rect::new(0, 0, width, 8), source.clone(), 1,)
                            .unwrap()
                    );
                })
                .unwrap();
            assert_eq!(viewer.artifact_pending.as_ref().unwrap().key, blocked_key);
        }
        blocked_tx
            .send(Ok(PreparedArtifact {
                decoded: Arc::new(decoded.clone()),
                protocol: halfblock_picker()
                    .new_protocol(decoded, Rect::new(0, 0, 32, 8).into(), Resize::Fit(None))
                    .unwrap(),
            }))
            .unwrap();
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_artifact(frame, Rect::new(0, 0, 32, 8), source.clone(), 1,)
                        .unwrap(),
                    "a completed older reveal cannot satisfy a newer same-path request"
                );
            })
            .unwrap();
        assert_eq!(viewer.artifact_pending.as_ref().unwrap().key.request_id, 1);
        for area in [Rect::new(0, 0, 32, 16), Rect::new(0, 0, 16, 8)] {
            let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
            let mut painted = false;
            terminal
                .draw(|frame| {
                    assert!(
                        !viewer
                            .render_artifact(frame, area, source.clone(), 1)
                            .unwrap()
                    );
                })
                .unwrap();
            while !painted && std::time::Instant::now() < deadline {
                terminal
                    .draw(|frame| {
                        painted = viewer
                            .render_artifact(frame, area, source.clone(), 1)
                            .unwrap();
                    })
                    .unwrap();
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(painted, "actual halfblock pixels must finish encoding");
            assert!(terminal.backend().buffer().content.iter().any(|cell| {
                cell.fg == ratatui::style::Color::Rgb(230, 40, 20)
                    || cell.bg == ratatui::style::Color::Rgb(230, 40, 20)
            }));
        }
        image::RgbaImage::from_pixel(24, 16, image::Rgba([20, 210, 50, 255]))
            .save(&source.path)
            .unwrap();
        let mut refreshed = false;
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_artifact(frame, Rect::new(0, 0, 16, 8), source.clone(), 2,)
                        .unwrap(),
                    "an explicit reopen must reload changed bytes at the same path"
                );
            })
            .unwrap();
        let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        while !refreshed && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    refreshed = viewer
                        .render_artifact(frame, Rect::new(0, 0, 16, 8), source.clone(), 2)
                        .unwrap();
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(refreshed);
        assert!(terminal.backend().buffer().content.iter().any(|cell| {
            cell.fg == ratatui::style::Color::Rgb(20, 210, 50)
                || cell.bg == ratatui::style::Color::Rgb(20, 210, 50)
        }));
        let missing = crate::media::MediaSource {
            path: root.join("missing.png"),
            root: Some(root.clone()),
        };
        terminal
            .draw(|frame| {
                assert!(
                    !viewer
                        .render_artifact(frame, frame.area(), missing.clone(), 3)
                        .unwrap()
                );
            })
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .all(|cell| cell.symbol() == " "),
            "a missing requested artifact must not paint the previous image"
        );
        let deadline = std::time::Instant::now() + ASYNC_IMAGE_TEST_TIMEOUT;
        let mut failed = false;
        while !failed && std::time::Instant::now() < deadline {
            terminal
                .draw(|frame| {
                    failed = viewer
                        .render_artifact(frame, frame.area(), missing.clone(), 3)
                        .is_err();
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(failed);
        #[cfg(unix)]
        {
            let link = root.join("escape.png");
            std::os::unix::fs::symlink(&source.path, &link).unwrap();
            assert!(
                artifact_preview(&crate::media::MediaSource {
                    path: link,
                    root: Some(root.clone())
                })
                .is_err(),
                "confined sources retain no-follow protection"
            );
        }
        let oversized = root.join("oversized.png");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(64 * 1024 * 1024 + 1)
            .unwrap();
        assert!(
            artifact_preview(&crate::media::MediaSource {
                path: oversized,
                root: Some(root.clone())
            })
            .unwrap_err()
            .contains("64 MiB")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}

fn reported_cell_pixels(width: u16, height: u16, columns: u16, rows: u16) -> Option<(u16, u16)> {
    if columns == 0 || rows == 0 || width < columns || height < rows {
        return None;
    }
    Some((
        ((u32::from(width) + u32::from(columns) / 2) / u32::from(columns)) as u16,
        ((u32::from(height) + u32::from(rows) / 2) / u32::from(rows)) as u16,
    ))
}

#[cfg(test)]
#[test]
fn terminal_reported_pixels_override_cell_density_without_guessing_missing_geometry() {
    assert_eq!(reported_cell_pixels(1440, 960, 120, 40), Some((12, 24)));
    assert_eq!(reported_cell_pixels(1792, 1008, 128, 36), Some((14, 28)));
    assert_eq!(reported_cell_pixels(0, 0, 120, 40), None);
    for reported in [None, Some((0, 28)), Some((14, 0))] {
        let picker = still_pixel_picker(ProtocolType::Kitty, reported);
        assert_eq!(picker.protocol_type(), ProtocolType::Halfblocks);
        assert_eq!(
            (picker.font_size().width, picker.font_size().height),
            HALFBLOCK_FONT_SIZE
        );
    }
    assert_eq!(reported_cell_pixels(1440, 960, 0, 40), None);
}
