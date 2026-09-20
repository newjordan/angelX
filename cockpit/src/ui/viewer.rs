//! Inline image rendering via ratatui-image. `/show <path>` loads an image; the
//! cockpit renders it in the sidebar. `/hide` clears.
//!
//! Still inspection samples a single bounded decoded source on an off-thread worker.
//! Draw only presents completed protocols; portraits retain their own preview cache.
//!
//! Background imagery is intentionally outside this viewer. It renders only
//! terminal-native foreground media such as artifacts and portraits.

use crate::agent::sandbox::process_owner::OwnedCommandExt;
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
    source: crate::ui::media::MediaSource,
    width: u16,
    height: u16,
    scene: Rect,
    font: (u16, u16),
    protocol: ProtocolType,
    layout_epoch: u64,
    view: crate::ui::still_inspector::View,
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
    identity: crate::ui::scryglass::VideoIdentity,
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
    geometry: crate::ui::dots::canvas::DotGeometry,
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
    protocol: crate::ui::dots::protocol::DotProtocol,
}

struct PendingDots {
    key: DotFrameKey,
    rx: mpsc::Receiver<Result<crate::ui::dots::protocol::DotProtocol, String>>,
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
    artifact_identity: Option<(crate::ui::media::MediaSource, u64)>,
    artifact_decoded: Option<Arc<image::DynamicImage>>,
    pub(crate) inspector: crate::ui::still_inspector::Inspector,
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

    pub(crate) fn still_matches(
        &self,
        source: &crate::ui::media::MediaSource,
        request: u64,
    ) -> bool {
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
        source: crate::ui::media::MediaSource,
        request_id: u64,
    ) -> Result<bool, String> {
        let picker = still_pixel_picker(self.picker.protocol_type(), terminal_cell_pixels());
        self.render_artifact_with_picker(frame, area, source, request_id, picker)
    }

    fn render_artifact_with_picker(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        source: crate::ui::media::MediaSource,
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
        pixels: Arc<crate::ui::scryglass::VideoPixels>,
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
            let image = anchor_portrait_canvas(image, &self.portrait_picker, size);
            self.portrait_picker
                .new_protocol(
                    image,
                    size.into(),
                    // `anchor_portrait_canvas` already produced the exact
                    // rounded pixel canvas; Crop prevents a second Fit pass
                    // from reintroducing origin padding.
                    Resize::Crop(None),
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
        portal_frame: &crate::ui::viz::agentviz_portal::PortalFrame,
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
                    crate::ui::viz::agentviz_portal::FRAME_WIDTH,
                    crate::ui::viz::agentviz_portal::FRAME_HEIGHT,
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
    pub(crate) fn dot_geometry(&self, area: Rect) -> Option<crate::ui::dots::canvas::DotGeometry> {
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
        crate::ui::dots::canvas::DotGeometry::new(
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
        geometry: crate::ui::dots::canvas::DotGeometry,
        image: Arc<crate::ui::term::art::ColoredBrailleImage>,
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
                crate::ui::dots::protocol::DotProtocol::encode(geometry, &image, size.into(), id);
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
                    let image = anchor_portrait_canvas(image, &picker, size);
                    picker
                        .new_protocol(image, size.into(), Resize::Crop(None))
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
        Some(pose) => crate::ui::helm::frame_image(path, pose),
        None => cached_bounded_preview(path).map(|(image, _)| image),
    }
}

/// Prepare a portrait on the exact pixel canvas that ratatui-image will use.
/// Its `Fit` resize preserves aspect ratio by placing the fitted image at the
/// canvas origin, which leaves rounded cell pixels on the lower and right
/// edges. Translate the authored pixels within that same canvas so the visible
/// alpha bounds finish at its lower-right edge. The existing Fit scale is
/// preserved; alignment adds only a translation, without mirroring the pose.
fn anchor_portrait_canvas(
    image: image::DynamicImage,
    picker: &Picker,
    size: Rect,
) -> image::DynamicImage {
    if size.width == 0 || size.height == 0 {
        return image;
    }
    let fit = Resize::Fit(Some(image::imageops::FilterType::Lanczos3));
    let font = picker.font_size();
    let desired = Resize::natural_size(&image, font);
    // Match Picker::new_protocol: retain a source that already fits in the
    // requested cells, including small portraits, instead of upscaling it to
    // the whole bay. Only the same rounded target that Picker would resize to
    // is prepared before the alpha translation.
    let fitted = if desired.width <= size.width
        && desired.height <= size.height
        && (image.width() == u32::from(desired.width) * u32::from(font.width)
            || image.height() == u32::from(desired.height) * u32::from(font.height))
    {
        image
    } else {
        let target = fit.size_for(&image, font, size.into());
        fit.resize(&image, font, target, None)
    };
    let source = fitted.to_rgba8();
    let (mut left, mut top) = source.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] != 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if right <= left || bottom <= top {
        return image::DynamicImage::ImageRgba8(source);
    }
    let mut anchored = image::RgbaImage::new(source.width(), source.height());
    let crop = image::imageops::crop_imm(&source, left, top, right - left, bottom - top).to_image();
    image::imageops::replace(
        &mut anchored,
        &crop,
        i64::from(source.width() - (right - left)),
        i64::from(source.height() - (bottom - top)),
    );
    image::DynamicImage::ImageRgba8(anchored)
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

fn artifact_preview(source: &crate::ui::media::MediaSource) -> Result<image::DynamicImage, String> {
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
#[path = "../../../tests/cockpit/app/viewer__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/viewer__map_gate_tests.rs"]
mod map_gate_tests;

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
include!("../../../tests/cockpit/app/viewer__standalone_tests.rs");
