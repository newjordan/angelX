//! Scryglass: the artifacts pane's terminal-native visual stage.
//!
//! The stage preserves a live world beneath short, interruptible media reveals.
//! It owns display timing, media delivery, the visual-only camera,
//! and the bounded delivery queue. Agent execution and world simulation never
//! depend on this module.

use crate::media::Media;
#[cfg(feature = "scryglass-video")]
use crate::media::MediaSource;
use crate::terminal_art::ColoredBrailleImage;
use crate::world_viz::Building;
use crate::{harness::ToolEventId, lifecycle_viz::CeremonyKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use std::collections::VecDeque;
#[cfg(feature = "scryglass-video")]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "scryglass-video")]
use std::sync::Mutex;
#[cfg(any(test, feature = "scryglass-video"))]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "scryglass-video")]
use std::sync::mpsc;
use std::time::{Duration, Instant};

mod document;

const STILL_REVEAL: Duration = Duration::from_secs(8);
const ARRIVAL_REVEAL: Duration = Duration::from_millis(1_250);
const FAULT_REVEAL: Duration = Duration::from_secs(3);
const MAX_PENDING_REVEALS: usize = 8;
static NEXT_MEDIA_REQUEST: AtomicU64 = AtomicU64::new(1);
#[cfg(feature = "scryglass-video")]
static VIDEO_RENDER_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

type VideoFrame = (
    Option<Arc<VideoPixels>>,
    Duration,
    Option<Duration>,
    bool,
    bool,
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VideoIdentity {
    pub source: crate::media::MediaSource,
    pub request_id: u64,
    pub generation: u64,
}

pub(crate) struct VideoPixels {
    pub identity: VideoIdentity,
    pub sequence: u64,
    pub rgba: image::RgbaImage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorldMode {
    FirstPerson,
    Map,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageSurface {
    Hidden,
    Research,
    Lesson,
    Catalog,
    Workshop,
    Lifecycle,
    Moa,
    Raytrace,
    Loop,
    Quest,
    Observatory,
    Reinforce,
    AgentGraph,
    WorldFirstPerson,
    WorldMap,
    Arrival(Building),
    Still(usize),
    Document(usize),
    Video(usize),
    Vault,
    Fault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageRoute {
    Realm,
    Research,
    Explore(Building),
    Formation,
    Observatory,
    Quest,
    Vault,
    Workshop,
    Loop,
    Raytrace,
    Reinforce,
    AgentGraph,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StageOverlay {
    Lesson,
    Catalog,
    Journey {
        call_id: ToolEventId,
        destination: Building,
    },
    Arrival {
        destination: Building,
    },
    Media {
        index: usize,
    },
    Lifecycle {
        kind: CeremonyKind,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct StageController {
    route: StageRoute,
    back_stack: Vec<StageRoute>,
    overlay: Option<StageOverlay>,
    overlay_owner: Option<StageRoute>,
}

impl Default for StageController {
    fn default() -> Self {
        Self {
            route: StageRoute::Realm,
            back_stack: Vec::new(),
            overlay: None,
            overlay_owner: None,
        }
    }
}

impl StageController {
    pub(crate) fn route(&self) -> StageRoute {
        self.route
    }

    pub(crate) fn underlying_route(&self) -> StageRoute {
        self.overlay_owner.unwrap_or(self.route)
    }

    pub(crate) fn overlay(&self) -> Option<&StageOverlay> {
        self.overlay.as_ref()
    }

    pub(crate) fn overlay_owner(&self) -> Option<StageRoute> {
        self.overlay_owner
    }

    pub(crate) fn navigate(&mut self, route: StageRoute) {
        if self.route == route {
            return;
        }
        self.back_stack.push(self.route);
        if self.back_stack.len() > 8 {
            self.back_stack.remove(0);
        }
        self.route = route;
        if matches!(
            self.overlay,
            Some(StageOverlay::Journey { .. } | StageOverlay::Arrival { .. })
        ) {
            self.overlay = None;
            self.overlay_owner = None;
        }
    }

    pub(crate) fn reset(&mut self, route: StageRoute) {
        self.route = route;
        self.back_stack.clear();
        self.clear_overlay();
    }

    /// Close a route because its owning subsystem stopped, without dismissing
    /// a user-opened overlay. Media keeps playing and returns to the route that
    /// preceded the closed subsystem.
    pub(crate) fn leave_route(&mut self, route: StageRoute) -> bool {
        if self.route != route {
            return false;
        }
        let target = self.back_stack.pop().unwrap_or(StageRoute::Realm);
        self.route = target;
        if self.overlay_owner == Some(route) {
            self.overlay_owner = Some(target);
        }
        true
    }

    pub(crate) fn show_overlay(&mut self, overlay: StageOverlay) -> bool {
        let teaching_surface = matches!(overlay, StageOverlay::Lesson | StageOverlay::Catalog);
        if teaching_surface {
            // Lesson and Catalog are two views of one teaching workspace. They
            // may replace each other directly; unrelated automatic/media
            // overlays still keep their existing ownership rules.
            let replaces_teaching_surface = matches!(
                self.overlay,
                Some(StageOverlay::Lesson | StageOverlay::Catalog)
            );
            if self.overlay.is_some() && !replaces_teaching_surface {
                return false;
            }
        }
        let automatic_travel = matches!(
            overlay,
            StageOverlay::Journey { .. } | StageOverlay::Arrival { .. }
        );
        let current_blocks_automatic = matches!(
            self.overlay,
            Some(
                StageOverlay::Media { .. }
                    | StageOverlay::Lifecycle { .. }
                    | StageOverlay::Lesson
                    | StageOverlay::Catalog
            )
        );
        if automatic_travel && (!self.automatic_overlay_allowed() || current_blocks_automatic) {
            return false;
        }
        let current_is_media = matches!(self.overlay, Some(StageOverlay::Media { .. }));
        if current_is_media && !matches!(overlay, StageOverlay::Media { .. }) {
            return false;
        }
        let current_is_teaching = matches!(
            self.overlay,
            Some(StageOverlay::Lesson | StageOverlay::Catalog)
        );
        if current_is_teaching
            && !matches!(
                overlay,
                StageOverlay::Lesson | StageOverlay::Catalog | StageOverlay::Media { .. }
            )
        {
            return false;
        }
        self.overlay_owner.get_or_insert(self.route);
        self.overlay = Some(overlay);
        true
    }

    pub(crate) fn clear_overlay(&mut self) {
        self.overlay = None;
        self.overlay_owner = None;
    }

    /// Back dismisses one overlay, then one recorded route, then returns false
    /// so the caller can restore focus to Core.
    pub(crate) fn back(&mut self) -> bool {
        if self.overlay.is_some() {
            self.clear_overlay();
            return true;
        }
        if let Some(route) = self.back_stack.pop() {
            self.route = route;
            return true;
        }
        if !matches!(self.route, StageRoute::Realm | StageRoute::Research) {
            self.route = StageRoute::Realm;
            return true;
        }
        false
    }

    fn automatic_overlay_allowed(&self) -> bool {
        matches!(self.route, StageRoute::Realm | StageRoute::Explore(_))
    }

    /// The only scene-resolution match. Automatic journey/arrival cues remain
    /// recorded but never displace an operator-selected destination.
    ///
    /// `quest_owns_pane` is `World::quest_owns_pane()` — true while the
    /// adventure stands anywhere but Castle Town. Z5: an automatic arrival
    /// then keeps the ride surface instead of cutting to a town establishing
    /// shot, because the plate underneath is the Mines and the landmark is
    /// not. Operator-chosen routes and every other overlay are untouched.
    pub(crate) fn resolved_scene(
        &self,
        media_is_video: bool,
        media_fault: bool,
        quest_owns_pane: bool,
    ) -> StageSurface {
        if let Some(overlay) = &self.overlay {
            match overlay {
                StageOverlay::Lesson => return StageSurface::Lesson,
                StageOverlay::Catalog => return StageSurface::Catalog,
                StageOverlay::Media { index } => {
                    return if media_fault {
                        StageSurface::Fault
                    } else if media_is_video {
                        StageSurface::Video(*index)
                    } else {
                        StageSurface::Still(*index)
                    };
                }
                StageOverlay::Lifecycle { .. } => return StageSurface::Lifecycle,
                StageOverlay::Journey { .. } if self.automatic_overlay_allowed() => {
                    return StageSurface::WorldFirstPerson;
                }
                StageOverlay::Arrival { .. }
                    if quest_owns_pane && self.automatic_overlay_allowed() =>
                {
                    return StageSurface::WorldFirstPerson;
                }
                StageOverlay::Arrival { destination } if self.automatic_overlay_allowed() => {
                    return StageSurface::Arrival(*destination);
                }
                StageOverlay::Journey { .. } | StageOverlay::Arrival { .. } => {}
            }
        }
        match self.route {
            StageRoute::Realm => StageSurface::WorldMap,
            StageRoute::Research => StageSurface::Research,
            StageRoute::Explore(_) => StageSurface::WorldFirstPerson,
            StageRoute::Formation => StageSurface::Moa,
            StageRoute::Observatory => StageSurface::Observatory,
            StageRoute::Quest => StageSurface::Quest,
            StageRoute::Vault => StageSurface::Vault,
            StageRoute::Workshop => StageSurface::Workshop,
            StageRoute::Loop => StageSurface::Loop,
            StageRoute::Raytrace => StageSurface::Raytrace,
            StageRoute::Reinforce => StageSurface::Reinforce,
            StageRoute::AgentGraph => StageSurface::AgentGraph,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RevealState {
    Loading,
    Showing,
    Fault,
}

#[derive(Clone, Debug)]
struct Reveal {
    request_id: u64,
    media_index: usize,
    pinned: bool,
    state: RevealState,
    visible_for: Duration,
    last_visible_at: Option<Instant>,
    error: Option<String>,
    ending: bool,
}

impl Reveal {
    fn new(media_index: usize, pinned: bool) -> Self {
        Self {
            request_id: NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed),
            media_index,
            pinned,
            state: RevealState::Loading,
            visible_for: Duration::ZERO,
            last_visible_at: None,
            error: None,
            ending: false,
        }
    }

    fn note_visible(&mut self, now: Instant) {
        if let Some(last) = self.last_visible_at.replace(now) {
            self.visible_for += now.saturating_duration_since(last);
        }
    }

    fn pause(&mut self) {
        self.last_visible_at = None;
    }

    fn expired(&self, is_video_ended: bool) -> bool {
        if self.pinned {
            return false;
        }
        match self.state {
            RevealState::Loading => false,
            RevealState::Fault => self.visible_for >= FAULT_REVEAL,
            RevealState::Showing if is_video_ended || self.ending => {
                self.visible_for >= Duration::from_secs(2)
            }
            RevealState::Showing => self.visible_for >= STILL_REVEAL,
        }
    }
}

#[derive(Clone, Debug)]
struct ArrivalReveal {
    building: Building,
    visible_for: Duration,
    last_visible_at: Option<Instant>,
}

impl ArrivalReveal {
    fn new(building: Building) -> Self {
        Self {
            building,
            visible_for: Duration::ZERO,
            last_visible_at: None,
        }
    }

    fn note_visible(&mut self, now: Instant) {
        if let Some(last) = self.last_visible_at.replace(now) {
            self.visible_for += now.saturating_duration_since(last);
        }
    }
}

#[cfg(any(test, feature = "scryglass-video"))]
struct StillRenderLease {
    in_flight: &'static AtomicBool,
}

#[cfg(any(test, feature = "scryglass-video"))]
impl Drop for StillRenderLease {
    fn drop(&mut self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

#[cfg(any(test, feature = "scryglass-video"))]
fn acquire_still_render(in_flight: &'static AtomicBool) -> Option<StillRenderLease> {
    in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| StillRenderLease { in_flight })
}

#[cfg(feature = "scryglass-video")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct VideoKey {
    source: MediaSource,
    request_id: u64,
    width: usize,
    height: usize,
    fps_cap: u8,
    autoplay: bool,
}

#[cfg(feature = "scryglass-video")]
#[derive(Debug)]
enum VideoCommand {
    Toggle,
    Seek(i64, u64),
    Restart(u64),
    Visible(bool),
    Resize(usize, usize),
    Stop,
}

#[cfg(feature = "scryglass-video")]
#[derive(Clone, Default)]
struct VideoSnapshot {
    frame: Option<Arc<VideoPixels>>,
    generation: u64,
    position: Duration,
    duration: Option<Duration>,
    paused: bool,
    ended: bool,
    error: Option<String>,
}

#[cfg(feature = "scryglass-video")]
struct ActiveVideo {
    key: VideoKey,
    tx: mpsc::Sender<VideoCommand>,
    mailbox: Arc<Mutex<VideoSnapshot>>,
    visible: bool,
    cancelled: Arc<AtomicBool>,
}

#[cfg(feature = "scryglass-video")]
impl Drop for ActiveVideo {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        // Wake a worker parked at pause/EOF. The flag bypasses queued controls.
        let _ = self.tx.send(VideoCommand::Stop);
    }
}

#[cfg(feature = "scryglass-video")]
#[derive(Default)]
struct VideoSurface {
    active: Option<ActiveVideo>,
}

#[cfg(feature = "scryglass-video")]
impl VideoSurface {
    fn stop(&mut self) {
        self.active = None;
    }

    fn set_visible(&mut self, visible: bool) {
        if let Some(active) = self.active.as_mut()
            && active.visible != visible
        {
            active.visible = visible;
            let _ = active.tx.send(VideoCommand::Visible(visible));
        }
    }

    fn command(&mut self, mut command: VideoCommand) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let was_ended = active
            .mailbox
            .lock()
            .ok()
            .is_some_and(|snapshot| snapshot.ended);
        if was_ended && matches!(command, VideoCommand::Toggle) {
            command = VideoCommand::Restart(0);
        }
        if matches!(&command, VideoCommand::Seek(..) | VideoCommand::Restart(_))
            && let Ok(mut snapshot) = active.mailbox.lock()
        {
            // Invalidate synchronously: a decode already in flight may finish,
            // but cannot publish into the newly requested playback generation.
            let generation = NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed);
            match &mut command {
                VideoCommand::Seek(_, value) | VideoCommand::Restart(value) => *value = generation,
                _ => unreachable!(),
            }
            snapshot.generation = generation;
            snapshot.frame = None;
            snapshot.error = None;
        }
        let Err(error) = active.tx.send(command) else {
            return;
        };

        // A decoder can still disappear because its input could not be opened
        // or its worker could not be spawned. Explicit controls are recovery
        // points: replace the dead generation and replay the user's command.
        // Normal EOF never takes this path because the worker parks and waits.
        let key = active.key.clone();
        let visible = active.visible;
        let Some(mut replacement) = spawn_video(key) else {
            self.active = None;
            return;
        };
        replacement.visible = visible;
        if !visible {
            let _ = replacement.tx.send(VideoCommand::Visible(false));
        }
        let mut replay = error.0;
        if let Ok(mut snapshot) = replacement.mailbox.lock() {
            let generation = NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed);
            match &mut replay {
                VideoCommand::Seek(_, value) | VideoCommand::Restart(value) => {
                    *value = generation;
                    snapshot.generation = generation;
                    snapshot.frame = None;
                }
                _ => {}
            }
        }
        // One recovery attempt, never recursive respawning on a bad file.
        let _ = replacement.tx.send(replay);
        self.active = Some(replacement);
    }

    fn request(
        &mut self,
        source: MediaSource,
        request_id: u64,
        width: usize,
        height: usize,
        fps_cap: u8,
        autoplay: bool,
    ) -> VideoSnapshot {
        let key = VideoKey {
            source,
            request_id,
            width,
            height,
            fps_cap,
            autoplay,
        };
        if let Some(active) = self.active.as_mut()
            && active.key.source == key.source
            && active.key.request_id == key.request_id
            && active.key.fps_cap == key.fps_cap
            && active.key.autoplay == key.autoplay
            && (active.key.width != width || active.key.height != height)
        {
            active.key.width = width;
            active.key.height = height;
            // Resize the existing scaler, not the decoder or playback clock.
            // A paused frame remains visible at its original timestamp.
            let _ = active.tx.send(VideoCommand::Resize(width, height));
        }
        if self.active.as_ref().is_none_or(|active| active.key != key) {
            self.stop();
            self.active = spawn_video(key);
        }
        self.active
            .as_ref()
            .and_then(|active| active.mailbox.lock().ok().map(|state| state.clone()))
            .unwrap_or_default()
    }

    fn snapshot(&self) -> VideoSnapshot {
        self.active
            .as_ref()
            .and_then(|active| active.mailbox.lock().ok().map(|state| state.clone()))
            .unwrap_or_default()
    }
}

#[cfg(feature = "scryglass-video")]
fn spawn_video(key: VideoKey) -> Option<ActiveVideo> {
    let lease = acquire_still_render(&VIDEO_RENDER_IN_FLIGHT)?;
    let (tx, rx) = mpsc::channel();
    let generation = NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed);
    let mailbox = Arc::new(Mutex::new(VideoSnapshot {
        generation,
        ..VideoSnapshot::default()
    }));
    let worker_mailbox = Arc::clone(&mailbox);
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let worker_key = key.clone();
    let spawn_result = std::thread::Builder::new()
        .name("angel-scryglass-video".to_string())
        .spawn(move || {
            video_worker(worker_key, rx, worker_mailbox, generation, worker_cancelled);
            drop(lease);
        });
    if let Err(error) = spawn_result
        && let Ok(mut state) = mailbox.lock()
    {
        state.error = Some(format!("could not start video worker: {error}"));
        state.paused = true;
    }
    Some(ActiveVideo {
        key,
        tx,
        mailbox,
        visible: true,
        cancelled,
    })
}

#[cfg(feature = "scryglass-video")]
fn contain_video_cells(
    source_width: u32,
    source_height: u32,
    max_width: usize,
    max_height: usize,
) -> (usize, usize) {
    // At 2x4 source pixels per cell this is at most 98,304 RGBA pixels.
    let max_width = max_width.clamp(1, 192);
    let max_height = max_height.clamp(1, 64);
    if source_width == 0 || source_height == 0 {
        return (max_width.max(1), max_height.max(1));
    }
    let max_dot_w = (max_width.max(1) * 2) as f64;
    let max_dot_h = (max_height.max(1) * 4) as f64;
    let scale = (max_dot_w / f64::from(source_width)).min(max_dot_h / f64::from(source_height));
    let dot_w = (f64::from(source_width) * scale).floor().max(2.0) as usize;
    let dot_h = (f64::from(source_height) * scale).floor().max(4.0) as usize;
    (
        (dot_w / 2).max(1).min(max_width),
        (dot_h / 4).max(1).min(max_height),
    )
}

#[cfg(feature = "scryglass-video")]
#[derive(Debug)]
struct VideoPlayback {
    visible: bool,
    paused: bool,
    ended: bool,
    faulted: bool,
    position: Duration,
    duration: Option<Duration>,
    anchor_wall: Instant,
    anchor_pts: Duration,
}

#[cfg(feature = "scryglass-video")]
impl VideoPlayback {
    fn new(duration: Option<Duration>, now: Instant) -> Self {
        Self {
            visible: true,
            paused: false,
            ended: false,
            faulted: false,
            position: Duration::ZERO,
            duration,
            anchor_wall: now,
            anchor_pts: Duration::ZERO,
        }
    }

    fn is_playing(&self) -> bool {
        self.visible && !self.paused && !self.ended && !self.faulted
    }

    fn reanchor(&mut self, now: Instant) {
        self.anchor_wall = now;
        self.anchor_pts = self.position;
    }

    fn set_visible(&mut self, visible: bool, now: Instant) {
        let was_playing = self.is_playing();
        self.visible = visible;
        if !was_playing && self.is_playing() {
            self.reanchor(now);
        }
    }

    fn toggle(&mut self, now: Instant) {
        if self.ended || self.faulted {
            return;
        }
        self.paused = !self.paused;
        if !self.paused {
            self.reanchor(now);
        }
    }

    fn pause(&mut self) {
        self.paused = true;
    }

    fn seek_target(&self, delta: i64) -> Duration {
        let target = if delta < 0 {
            self.position
                .saturating_sub(Duration::from_secs(delta.unsigned_abs()))
        } else {
            self.position
                .saturating_add(Duration::from_secs(delta as u64))
        };
        self.duration
            .map_or(target, |duration| target.min(duration))
    }

    fn seek_succeeded(&mut self, target: Duration, resume: bool, now: Instant) {
        self.position = self
            .duration
            .map_or(target, |duration| target.min(duration));
        self.ended = false;
        self.faulted = false;
        if resume {
            self.paused = false;
        }
        self.reanchor(now);
    }

    fn frame_published(&mut self, pts: Duration) {
        self.position = self.duration.map_or(pts, |duration| pts.min(duration));
        self.ended = false;
        self.faulted = false;
    }

    fn mark_ended(&mut self) {
        if let Some(duration) = self.duration {
            self.position = duration;
        }
        self.ended = true;
        self.faulted = false;
        self.paused = true;
    }

    fn mark_faulted(&mut self) {
        self.ended = false;
        self.faulted = true;
        self.paused = true;
    }

    fn frame_deadline(&self, pts: Duration) -> Instant {
        self.anchor_wall
            .checked_add(pts.saturating_sub(self.anchor_pts))
            .unwrap_or(self.anchor_wall)
    }

    fn write_snapshot(&self, snapshot: &mut VideoSnapshot) {
        snapshot.position = self.position;
        snapshot.duration = self.duration;
        snapshot.paused = self.paused;
        snapshot.ended = self.ended;
    }
}

#[cfg(feature = "scryglass-video")]
fn video_sample_every(source_fps: f64, fps_cap: u8) -> usize {
    (source_fps / f64::from(fps_cap.max(1))).ceil().max(1.0) as usize
}

#[cfg(feature = "scryglass-video")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VideoWorkerControl {
    KeepFrame,
    DropFrame,
    Stop,
}

#[cfg(feature = "scryglass-video")]
fn publish_video_state(
    mailbox: &Arc<Mutex<VideoSnapshot>>,
    playback: &VideoPlayback,
    clear_error: bool,
    generation: u64,
) {
    if let Ok(mut snapshot) = mailbox.lock()
        && snapshot.generation == generation
    {
        playback.write_snapshot(&mut snapshot);
        if clear_error {
            snapshot.error = None;
        }
    }
}

#[cfg(feature = "scryglass-video")]
fn publish_video_error(
    mailbox: &Arc<Mutex<VideoSnapshot>>,
    playback: &mut VideoPlayback,
    error: impl ToString,
    generation: u64,
) {
    playback.mark_faulted();
    if let Ok(mut snapshot) = mailbox.lock()
        && snapshot.generation == generation
    {
        playback.write_snapshot(&mut snapshot);
        snapshot.error = Some(error.to_string());
    }
}

#[cfg(feature = "scryglass-video")]
fn seek_video_player(
    player: &mut dotmax::media::VideoPlayer,
    playback: &mut VideoPlayback,
    mailbox: &Arc<Mutex<VideoSnapshot>>,
    target: Duration,
    resume: bool,
    now: Instant,
    generation: u64,
) -> VideoWorkerControl {
    match player.seek(target) {
        Ok(()) => {
            playback.seek_succeeded(target, resume, now);
            publish_video_state(mailbox, playback, true, generation);
        }
        Err(error) => publish_video_error(mailbox, playback, error, generation),
    }
    VideoWorkerControl::DropFrame
}

#[cfg(feature = "scryglass-video")]
fn handle_video_command(
    command: VideoCommand,
    player: &mut dotmax::media::VideoPlayer,
    playback: &mut VideoPlayback,
    mailbox: &Arc<Mutex<VideoSnapshot>>,
    generation: &mut u64,
) -> VideoWorkerControl {
    let now = Instant::now();
    match command {
        VideoCommand::Stop => VideoWorkerControl::Stop,
        VideoCommand::Visible(visible) => {
            playback.set_visible(visible, now);
            publish_video_state(mailbox, playback, false, *generation);
            VideoWorkerControl::KeepFrame
        }
        VideoCommand::Resize(width, height) => {
            let (width, height) =
                contain_video_cells(player.width(), player.height(), width, height);
            dotmax::media::MediaPlayer::handle_resize(player, width, height);
            VideoWorkerControl::KeepFrame
        }
        VideoCommand::Toggle if playback.ended => {
            // EOF can race the UI's Toggle dispatch. Start a new generation
            // here too, but never overwrite a newer already-requested seek.
            let previous = *generation;
            *generation = NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut snapshot) = mailbox.lock()
                && snapshot.generation == previous
            {
                snapshot.generation = *generation;
                snapshot.frame = None;
            }
            seek_video_player(
                player,
                playback,
                mailbox,
                Duration::ZERO,
                true,
                now,
                *generation,
            )
        }
        VideoCommand::Toggle => {
            playback.toggle(now);
            publish_video_state(mailbox, playback, false, *generation);
            VideoWorkerControl::KeepFrame
        }
        VideoCommand::Restart(next_generation) => {
            *generation = next_generation;
            seek_video_player(
                player,
                playback,
                mailbox,
                Duration::ZERO,
                true,
                now,
                *generation,
            )
        }
        VideoCommand::Seek(delta, next_generation) => {
            *generation = next_generation;
            let target = playback.seek_target(delta);
            seek_video_player(player, playback, mailbox, target, false, now, *generation)
        }
    }
}

#[cfg(feature = "scryglass-video")]
fn video_worker(
    key: VideoKey,
    rx: mpsc::Receiver<VideoCommand>,
    mailbox: Arc<Mutex<VideoSnapshot>>,
    mut generation: u64,
    cancelled: Arc<AtomicBool>,
) {
    use dotmax::media::{MediaPlayer, VideoPlayer};

    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let source_file = match key.source.open() {
        Ok(file) => file,
        Err(error) => {
            if let Ok(mut state) = mailbox.lock() {
                state.error = Some(error);
                state.paused = true;
            }
            return;
        }
    };
    if source_file
        .metadata()
        .map_or(true, |metadata| metadata.len() > 512 * 1024 * 1024)
    {
        if let Ok(mut state) = mailbox.lock() {
            state.error = Some("video preview exceeds the 512 MiB source limit".into());
            state.paused = true;
        }
        return;
    }
    #[cfg(unix)]
    let decoder_path = {
        use std::os::fd::AsRawFd;
        #[cfg(target_os = "linux")]
        let prefix = "/proc/self/fd";
        #[cfg(not(target_os = "linux"))]
        let prefix = "/dev/fd";
        PathBuf::from(format!("{prefix}/{}", source_file.as_raw_fd()))
    };
    #[cfg(not(unix))]
    let decoder_path = {
        if let Ok(mut state) = mailbox.lock() {
            state.error = Some("descriptor-backed video is unavailable on this platform".into());
        }
        return;
        #[allow(unreachable_code)]
        PathBuf::new()
    };
    // source_file deliberately remains owned until the player/worker exits.
    if cancelled.load(Ordering::Acquire) {
        return;
    }
    let mut player = match VideoPlayer::new_local(&decoder_path) {
        Ok(player) => player,
        Err(error) => {
            if let Ok(mut state) = mailbox.lock() {
                state.error = Some(error.to_string());
                state.paused = true;
            }
            return;
        }
    };
    let (width, height) =
        contain_video_cells(player.width(), player.height(), key.width, key.height);
    MediaPlayer::handle_resize(&mut player, width, height);
    let mut playback = VideoPlayback::new(player.duration(), Instant::now());
    publish_video_state(&mailbox, &playback, false, generation);

    let sample_every = video_sample_every(player.fps(), key.fps_cap);
    let mut pause_after_first = !key.autoplay;
    let mut skip_before_next = false;
    let mut pending_frame = None;
    let mut refresh_frame = false;
    let mut seek_floor = None;
    let mut sequence = 0u64;

    loop {
        loop {
            if cancelled.load(Ordering::Acquire) {
                return;
            }
            match rx.try_recv() {
                Ok(command) => {
                    match handle_video_command(
                        command,
                        &mut player,
                        &mut playback,
                        &mailbox,
                        &mut generation,
                    ) {
                        VideoWorkerControl::KeepFrame => {}
                        VideoWorkerControl::DropFrame => {
                            pending_frame = None;
                            skip_before_next = false;
                            refresh_frame = true;
                            seek_floor = Some(playback.position);
                        }
                        VideoWorkerControl::Stop => return,
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }

        if !(playback.is_playing()
            || refresh_frame && playback.visible && !playback.faulted && !playback.ended)
        {
            let Ok(command) = rx.recv() else {
                return;
            };
            match handle_video_command(
                command,
                &mut player,
                &mut playback,
                &mailbox,
                &mut generation,
            ) {
                VideoWorkerControl::KeepFrame => {}
                VideoWorkerControl::DropFrame => {
                    pending_frame = None;
                    skip_before_next = false;
                    refresh_frame = true;
                    seek_floor = Some(playback.position);
                }
                VideoWorkerControl::Stop => return,
            }
            continue;
        }

        if pending_frame.is_none() {
            let mut reached_eof = false;
            if skip_before_next {
                for _ in 1..sample_every {
                    if cancelled.load(Ordering::Acquire) {
                        return;
                    }
                    match player.skip_next_frame() {
                        Ok(Some(_)) => {}
                        Ok(None) => {
                            reached_eof = true;
                            break;
                        }
                        Err(error) => {
                            publish_video_error(&mailbox, &mut playback, error, generation);
                            skip_before_next = false;
                            reached_eof = true;
                            break;
                        }
                    }
                }
            }
            if playback.faulted {
                continue;
            }
            if reached_eof {
                playback.mark_ended();
                publish_video_state(&mailbox, &playback, true, generation);
                skip_before_next = false;
                continue;
            }
            pending_frame = match player.next_timed_rgba_frame() {
                Ok(Some(frame)) => Some(frame),
                Ok(None) => {
                    playback.mark_ended();
                    publish_video_state(&mailbox, &playback, true, generation);
                    skip_before_next = false;
                    None
                }
                Err(error) => {
                    publish_video_error(&mailbox, &mut playback, error, generation);
                    skip_before_next = false;
                    None
                }
            };
            if pending_frame.is_none() {
                continue;
            }
            // Container seeks can land on an earlier keyframe. Decode forward
            // without publishing obsolete pixels or delaying at old timestamps.
            if seek_floor.is_some_and(|floor| pending_frame.as_ref().unwrap().pts < floor) {
                pending_frame = None;
                continue;
            }
            seek_floor = None;
        }

        let deadline = playback.frame_deadline(
            pending_frame
                .as_ref()
                .map(|frame| frame.pts)
                .unwrap_or(playback.position),
        );
        if let Some(wait) = deadline
            .checked_duration_since(Instant::now())
            .filter(|_| !playback.paused)
        {
            match rx.recv_timeout(wait) {
                Ok(command) => {
                    match handle_video_command(
                        command,
                        &mut player,
                        &mut playback,
                        &mailbox,
                        &mut generation,
                    ) {
                        VideoWorkerControl::KeepFrame => {}
                        VideoWorkerControl::DropFrame => {
                            pending_frame = None;
                            skip_before_next = false;
                            refresh_frame = true;
                            seek_floor = Some(playback.position);
                        }
                        VideoWorkerControl::Stop => return,
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }

        let Some(timed) = pending_frame.take() else {
            continue;
        };
        if cancelled.load(Ordering::Acquire) {
            return;
        }
        playback.frame_published(timed.pts);
        if pause_after_first {
            playback.pause();
            pause_after_first = false;
        }
        refresh_frame = false;
        sequence = sequence.wrapping_add(1);
        if let Ok(mut state) = mailbox.lock()
            && state.generation == generation
        {
            state.frame = Some(Arc::new(VideoPixels {
                identity: VideoIdentity {
                    source: key.source.clone(),
                    request_id: key.request_id,
                    generation,
                },
                sequence,
                rgba: timed.rgba,
            }));
            playback.write_snapshot(&mut state);
            state.error = None;
        }
        skip_before_next = true;
    }
}

#[cfg(not(feature = "scryglass-video"))]
#[derive(Default)]
struct VideoSurface;

#[cfg(not(feature = "scryglass-video"))]
impl VideoSurface {
    fn stop(&mut self) {}
    fn set_visible(&mut self, _visible: bool) {}
}

pub(crate) struct Scryglass {
    pub(crate) controller: StageController,
    pub(crate) follow_agent: bool,
    pub(crate) look_yaw: f32,
    pub(crate) look_pitch: f32,
    pub(crate) fov: f32,
    pub(crate) selected: usize,
    reveal: Option<Reveal>,
    queue: VecDeque<usize>,
    arrival: Option<ArrivalReveal>,
    arrival_synced: bool,
    last_arrived: Option<Building>,
    document: document::DocumentSurface,
    video: VideoSurface,
    lesson: Option<crate::term_lookup::QuickLookup>,
    lesson_scroll: u16,
    catalog_scroll: u16,
    catalog_selection: usize,
    lesson_returns_to_catalog: bool,
    #[cfg(test)]
    queued_lesson_outcome: Option<crate::term_lookup::TestLookupOutcome>,
    pub(crate) surface: StageSurface,
    pub(crate) visible: bool,
    pub(crate) renderable: bool,
}

impl Default for Scryglass {
    fn default() -> Self {
        Self {
            controller: StageController::default(),
            follow_agent: true,
            look_yaw: 0.0,
            look_pitch: 0.0,
            fov: 1.05,
            selected: 0,
            reveal: None,
            queue: VecDeque::new(),
            arrival: None,
            arrival_synced: false,
            last_arrived: None,
            document: document::DocumentSurface::default(),
            video: {
                #[cfg(feature = "scryglass-video")]
                {
                    VideoSurface::default()
                }
                #[cfg(not(feature = "scryglass-video"))]
                {
                    VideoSurface
                }
            },
            lesson: None,
            lesson_scroll: 0,
            catalog_scroll: 0,
            catalog_selection: 0,
            lesson_returns_to_catalog: false,
            #[cfg(test)]
            queued_lesson_outcome: None,
            surface: StageSurface::Hidden,
            visible: false,
            renderable: false,
        }
    }
}

impl Scryglass {
    /// Open the default Dotmax exploration pane. Realm and Explore share the
    /// sole outdoor renderer while retaining their own controls and routes.
    pub(crate) fn for_world(destination: Building) -> Self {
        let mut stage = Self::default();
        stage.navigate(StageRoute::Explore(destination));
        stage
    }
}

/// World-frame ink is averaged per braille cell, so neighbouring cells land on
/// near-identical truecolor values that still break SGR runs at the flush: a
/// genuinely new ride frame emits one ~19-byte truecolor header every ~1.8
/// cells (~2k headers at 100×38, measured by `ride_frame_ink_runs_bound_the_sgr_flood`).
///
/// Default quantizer is RGB444 (4 bits/channel, max error 8/255) — the remaining
/// ~20% SGR cut after the 2026-07-27 RGB555 wave. `ANGEL_WORLD_INK=rgb555`
/// restores the gentler SNES-word quantizer (error ≤4); `full`/`off`/`0` leaves
/// exact truecolor (debug only — terminal parse cost balloons). Media stills
/// and video keep exact ink; only the world/ride paint quantizes.
fn world_ink_mode() -> WorldInkMode {
    #[cfg(not(test))]
    {
        static MODE: std::sync::OnceLock<WorldInkMode> = std::sync::OnceLock::new();
        *MODE.get_or_init(world_ink_mode_from_env)
    }
    #[cfg(test)]
    world_ink_mode_from_env()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorldInkMode {
    Rgb444,
    Rgb555,
    Full,
}

fn world_ink_mode_from_env() -> WorldInkMode {
    match std::env::var("ANGEL_WORLD_INK")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
    {
        Some("rgb555" | "555" | "snes") => WorldInkMode::Rgb555,
        Some("full" | "off" | "0" | "false" | "no" | "exact") => WorldInkMode::Full,
        _ => WorldInkMode::Rgb444,
    }
}

fn quantize_world_ink(fg: [u8; 3]) -> [u8; 3] {
    match world_ink_mode() {
        WorldInkMode::Full => fg,
        WorldInkMode::Rgb555 => fg.map(|channel| {
            let level = (u16::from(channel) * 31 + 127) / 255;
            ((level * 255 + 15) / 31) as u8
        }),
        WorldInkMode::Rgb444 => fg.map(|channel| {
            let level = (u16::from(channel) * 15 + 127) / 255;
            ((level * 255 + 7) / 15) as u8
        }),
    }
}

/// Whether world/ride paint should quantize ink at all.
fn world_ink_quantizes() -> bool {
    world_ink_mode() != WorldInkMode::Full
}

impl Scryglass {
    pub(crate) fn overlay_remaining_ms(&self, now: Instant) -> Option<u128> {
        let visible_elapsed = |visible_for: Duration, last_visible_at: Option<Instant>| {
            visible_for
                + last_visible_at
                    .map(|last| now.saturating_duration_since(last))
                    .unwrap_or_default()
        };
        match self.controller.overlay()? {
            StageOverlay::Arrival { .. } => self.arrival.as_ref().map(|arrival| {
                ARRIVAL_REVEAL
                    .saturating_sub(visible_elapsed(
                        arrival.visible_for,
                        arrival.last_visible_at,
                    ))
                    .as_millis()
            }),
            StageOverlay::Media { .. } => self.reveal.as_ref().and_then(|reveal| {
                if reveal.pinned || reveal.state == RevealState::Loading {
                    return None;
                }
                let lifetime = if reveal.state == RevealState::Fault {
                    FAULT_REVEAL
                } else if reveal.ending {
                    Duration::from_secs(2)
                } else {
                    STILL_REVEAL
                };
                Some(
                    lifetime
                        .saturating_sub(visible_elapsed(reveal.visible_for, reveal.last_visible_at))
                        .as_millis(),
                )
            }),
            StageOverlay::Lesson
            | StageOverlay::Catalog
            | StageOverlay::Lifecycle { .. }
            | StageOverlay::Journey { .. } => None,
        }
    }

    pub(crate) fn reveal_media(&mut self, index: usize, pinned: bool) {
        self.document.clear();
        self.lesson = None;
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = false;
        self.catalog_scroll = 0;
        self.selected = index;
        if pinned {
            self.queue.clear();
            self.reveal = Some(Reveal::new(index, true));
            self.arrival = None;
            self.video.stop();
            self.controller.show_overlay(StageOverlay::Media { index });
            return;
        }
        if self.reveal.as_ref().is_some_and(|r| r.media_index == index)
            || self.queue.contains(&index)
        {
            return;
        }
        if self.reveal.is_none() {
            self.reveal = Some(Reveal::new(index, false));
            self.video.stop();
            self.controller.show_overlay(StageOverlay::Media { index });
        } else {
            if self.queue.len() >= MAX_PENDING_REVEALS {
                self.queue.pop_front();
            }
            self.queue.push_back(index);
        }
    }

    pub(crate) fn return_to_world(&mut self) {
        self.document.clear();
        self.queue.clear();
        self.reveal = None;
        self.arrival = None;
        self.video.stop();
        self.lesson = None;
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = false;
        self.catalog_scroll = 0;
        self.controller.reset(StageRoute::Realm);
    }

    fn start_next(&mut self) {
        if let Some(index) = self.queue.pop_front() {
            self.reveal = Some(Reveal::new(index, false));
            self.controller.show_overlay(StageOverlay::Media { index });
        }
    }

    pub(crate) fn navigate(&mut self, route: StageRoute) {
        let had_arrival_overlay = matches!(
            self.controller.overlay(),
            Some(StageOverlay::Arrival { .. })
        );
        self.controller.navigate(route);
        if had_arrival_overlay
            && !matches!(
                self.controller.overlay(),
                Some(StageOverlay::Arrival { .. })
            )
        {
            self.arrival = None;
        }
    }

    /// Persistent map/explore mode is route state, not a second navigation
    /// latch. Transient Journey overlays may resolve to a first-person scene
    /// without changing this operator-selected mode.
    pub(crate) fn world_mode(&self) -> WorldMode {
        if matches!(self.controller.route(), StageRoute::Explore(_)) {
            WorldMode::FirstPerson
        } else {
            WorldMode::Map
        }
    }

    pub(crate) fn begin_journey(
        &mut self,
        call_id: ToolEventId,
        destination: Building,
        reduced_motion: bool,
    ) {
        if reduced_motion {
            return;
        }
        self.controller.show_overlay(StageOverlay::Journey {
            call_id,
            destination,
        });
    }

    pub(crate) fn begin_lesson(&mut self, term: String) -> bool {
        self.begin_lesson_from(term, false)
    }

    fn begin_lesson_from(&mut self, term: String, return_to_catalog: bool) -> bool {
        if !self.controller.show_overlay(StageOverlay::Lesson) {
            return false;
        }
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = return_to_catalog;
        #[cfg(test)]
        if let Some(outcome) = self.queued_lesson_outcome.take() {
            self.lesson = Some(crate::term_lookup::QuickLookup::queued(&term, outcome));
            return true;
        }
        self.lesson = Some(crate::term_lookup::QuickLookup::spawn(term));
        true
    }

    pub(crate) fn open_catalog(&mut self) -> bool {
        if !self.controller.show_overlay(StageOverlay::Catalog) {
            return false;
        }
        self.lesson = None;
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = false;
        self.catalog_scroll = self.catalog_selection.min(u16::MAX as usize) as u16;
        true
    }

    pub(crate) fn catalog_selection(&self) -> usize {
        self.catalog_selection
            .min(crate::library::catalog_shelves().len().saturating_sub(1))
    }

    pub(crate) fn selected_shelf(&self) -> &'static crate::library::CurriculumShelf {
        &crate::library::catalog_shelves()[self.catalog_selection()]
    }

    pub(crate) fn move_catalog_selection(&mut self, delta: isize) {
        let len = crate::library::catalog_shelves().len();
        if len == 0 {
            self.catalog_selection = 0;
            return;
        }
        self.catalog_selection =
            (self.catalog_selection as isize + delta).rem_euclid(len as isize) as usize;
        self.catalog_scroll = self.catalog_selection.min(u16::MAX as usize) as u16;
    }

    pub(crate) fn select_catalog_first(&mut self) {
        self.catalog_selection = 0;
        self.catalog_scroll = 0;
    }

    pub(crate) fn select_catalog_last(&mut self) {
        self.catalog_selection = crate::library::catalog_shelves().len().saturating_sub(1);
        self.catalog_scroll = self.catalog_selection.min(u16::MAX as usize) as u16;
    }

    pub(crate) fn begin_selected_lesson(&mut self) -> bool {
        let shelf = self.selected_shelf();
        let Some(lesson) = crate::term_lookup::QuickLookup::for_shelf(shelf.id) else {
            return false;
        };
        if !self.controller.show_overlay(StageOverlay::Lesson) {
            return false;
        }
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = true;
        self.lesson = Some(lesson);
        true
    }

    pub(crate) fn catalog_open(&self) -> bool {
        matches!(self.controller.overlay(), Some(StageOverlay::Catalog))
    }

    pub(crate) fn teaching_surface_open(&self) -> bool {
        self.lesson.is_some() || self.catalog_open()
    }

    pub(crate) fn lesson(&self) -> Option<&crate::term_lookup::QuickLookup> {
        self.lesson.as_ref()
    }

    pub(crate) fn lesson_loading(&self) -> bool {
        self.lesson
            .as_ref()
            .is_some_and(crate::term_lookup::QuickLookup::is_loading)
    }

    pub(crate) fn lesson_roll_pending(&self) -> bool {
        self.lesson
            .as_ref()
            .is_some_and(crate::term_lookup::QuickLookup::roll_pending)
    }

    pub(crate) fn poll_lesson(&mut self, animate: bool) {
        let was_loading = self.lesson_loading();
        if let Some(lesson) = self.lesson.as_mut() {
            lesson.poll_and_roll(animate);
        }
        if was_loading && !self.lesson_loading() {
            self.lesson_scroll = 0;
        }
    }

    pub(crate) fn clear_lesson(&mut self) {
        self.lesson = None;
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = false;
        if matches!(self.controller.overlay(), Some(StageOverlay::Lesson)) {
            self.controller.clear_overlay();
        }
    }

    #[cfg(test)]
    pub(crate) fn set_ready_lesson(&mut self, term: &str, title: &str, summary: &str) -> bool {
        if !self.controller.show_overlay(StageOverlay::Lesson) {
            return false;
        }
        self.lesson_scroll = 0;
        self.lesson_returns_to_catalog = false;
        self.lesson = Some(crate::term_lookup::QuickLookup::ready(term, title, summary));
        true
    }

    /// Ready lesson with roll-in still pending (for A2 invisible-tax tests).
    #[cfg(test)]
    pub(crate) fn set_unrolled_lesson(&mut self, term: &str, title: &str, summary: &str) -> bool {
        if !self.controller.show_overlay(StageOverlay::Lesson) {
            return false;
        }
        self.lesson_scroll = 0;
        self.lesson = Some(crate::term_lookup::QuickLookup::ready_unrolled(
            term, title, summary,
        ));
        true
    }

    #[cfg(test)]
    pub(crate) const fn lesson_scroll(&self) -> u16 {
        self.lesson_scroll
    }

    pub(crate) fn clamp_lesson_scroll(&mut self, maximum: u16) -> u16 {
        self.lesson_scroll = self.lesson_scroll.min(maximum);
        self.lesson_scroll
    }

    pub(crate) fn scroll_lesson_up(&mut self, rows: u16) {
        self.lesson_scroll = self.lesson_scroll.saturating_sub(rows);
    }

    pub(crate) fn scroll_lesson_down(&mut self, rows: u16) {
        self.lesson_scroll = self.lesson_scroll.saturating_add(rows);
    }

    pub(crate) fn scroll_lesson_top(&mut self) {
        self.lesson_scroll = 0;
    }

    pub(crate) fn scroll_lesson_bottom(&mut self) {
        self.lesson_scroll = u16::MAX;
    }

    #[cfg(test)]
    pub(crate) const fn catalog_scroll(&self) -> u16 {
        self.catalog_scroll
    }

    pub(crate) fn scroll_teaching_up(&mut self, rows: u16) {
        if self.catalog_open() {
            self.move_catalog_selection(-(rows as isize));
        } else {
            self.scroll_lesson_up(rows);
        }
    }

    pub(crate) fn scroll_teaching_down(&mut self, rows: u16) {
        if self.catalog_open() {
            self.move_catalog_selection(rows as isize);
        } else {
            self.scroll_lesson_down(rows);
        }
    }

    pub(crate) fn scroll_teaching_top(&mut self) {
        if self.catalog_open() {
            self.select_catalog_first();
        } else {
            self.scroll_lesson_top();
        }
    }

    pub(crate) fn scroll_teaching_bottom(&mut self) {
        if self.catalog_open() {
            self.select_catalog_last();
        } else {
            self.scroll_lesson_bottom();
        }
    }

    #[cfg(test)]
    pub(crate) fn queue_lesson_outcome(&mut self, outcome: crate::term_lookup::TestLookupOutcome) {
        self.queued_lesson_outcome = Some(outcome);
    }

    pub(crate) fn toggle_world_route(&mut self, destination: Building) {
        if matches!(
            self.controller.overlay(),
            Some(StageOverlay::Journey { .. })
        ) {
            self.controller.clear_overlay();
            if self.controller.route() != StageRoute::Realm {
                self.navigate(StageRoute::Realm);
            }
            return;
        }
        match self.controller.route() {
            StageRoute::Realm => self.navigate(StageRoute::Explore(destination)),
            StageRoute::Explore(_) => self.navigate(StageRoute::Realm),
            _ => self.navigate(StageRoute::Realm),
        }
    }

    pub(crate) fn back_overlay(&mut self) -> bool {
        if self.controller.overlay().is_none() {
            return false;
        }
        self.document.clear();
        if matches!(self.controller.overlay(), Some(StageOverlay::Lesson)) {
            self.lesson = None;
            self.lesson_scroll = 0;
            if self.lesson_returns_to_catalog {
                self.lesson_returns_to_catalog = false;
                self.controller.show_overlay(StageOverlay::Catalog);
                return true;
            }
            self.lesson_returns_to_catalog = false;
        }
        self.queue.clear();
        self.reveal = None;
        self.arrival = None;
        self.video.stop();
        self.controller.clear_overlay();
        true
    }

    /// Arrival plates and overlay chrome. Hidden / comp-mode still track
    /// `last_arrived` so a later visible frame can present the journey; they
    /// do not spawn ARRIVAL overlays.
    pub(crate) fn arrival_overlay_allowed() -> bool {
        crate::comp_mode::ambient_stage_sim_allowed()
    }

    pub(crate) fn sync_arrival(&mut self, arrived: Option<Building>) {
        // The world is already parked at the Keep before the first frame. Treat
        // that observation as the baseline, never as a journey outcome; only a
        // later transition may create ARRIVAL.
        if !self.arrival_synced {
            self.arrival_synced = true;
            self.last_arrived = arrived;
            return;
        }
        match arrived {
            None => {
                self.arrival = None;
                self.last_arrived = None;
                if matches!(
                    self.controller.overlay(),
                    Some(StageOverlay::Arrival { .. })
                ) {
                    self.controller.clear_overlay();
                }
            }
            Some(building) if self.last_arrived != Some(building) => {
                self.last_arrived = Some(building);
                if Self::arrival_overlay_allowed()
                    && self.reveal.is_none()
                    && self.controller.show_overlay(StageOverlay::Arrival {
                        destination: building,
                    })
                {
                    self.arrival = Some(ArrivalReveal::new(building));
                }
            }
            Some(_) => {}
        }
    }

    pub(crate) fn active_media(&self) -> Option<usize> {
        self.reveal.as_ref().map(|reveal| reveal.media_index)
    }

    pub(crate) fn media_request_id(&self) -> u64 {
        self.reveal.as_ref().map_or(0, |reveal| reveal.request_id)
    }

    pub(crate) fn active_error(&self) -> Option<&str> {
        self.reveal
            .as_ref()
            .and_then(|reveal| reveal.error.as_deref())
    }

    pub(crate) fn media_ready(&self) -> bool {
        self.reveal
            .as_ref()
            .is_some_and(|reveal| reveal.state != RevealState::Loading)
    }

    pub(crate) fn arrival(&self) -> Option<Building> {
        self.arrival.as_ref().map(|arrival| arrival.building)
    }

    pub(crate) fn pin_active(&mut self) {
        if let Some(reveal) = self.reveal.as_mut() {
            reveal.pinned = !reveal.pinned;
        }
    }

    /// Manual inspection is sticky; repeated gestures must never unpin. Drop
    /// queued automatic reveals so a timer/tool cannot replace the inspected asset.
    pub(crate) fn pin_inspection(&mut self) {
        if let Some(reveal) = self.reveal.as_mut() {
            reveal.pinned = true;
            self.queue.clear();
        }
    }

    pub(crate) fn active_pinned(&self) -> bool {
        self.reveal.as_ref().is_some_and(|reveal| reveal.pinned)
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.queue.len()
    }

    pub(crate) fn animating(&self) -> bool {
        self.visible && self.renderable && (self.reveal.is_some() || self.arrival.is_some())
    }

    /// Start a logical UI frame with no Stage scene. The Stage renderer earns
    /// visibility and resolves a concrete surface later in the same draw; any
    /// layout branch that omits it remains truthfully `Hidden` for inspection.
    pub(crate) fn begin_frame(&mut self) {
        self.surface = StageSurface::Hidden;
        // This is layout bookkeeping, not a real hide. Sending false here
        // then true during paint reanchors video on every frame and prevents
        // its next presentation deadline from ever arriving.
        self.visible = false;
        self.renderable = false;
    }

    pub(crate) fn finish_frame(&mut self) {
        // Layouts which omitted Stage must commit a genuine hide as well.
        self.set_stage_visibility(self.visible, self.renderable);
    }

    pub(crate) fn set_stage_visibility(&mut self, visible: bool, renderable: bool) {
        self.visible = visible;
        self.renderable = renderable;
        self.video.set_visible(visible && renderable);
        if !visible || !renderable {
            if let Some(reveal) = self.reveal.as_mut() {
                reveal.pause();
            }
            if let Some(arrival) = self.arrival.as_mut() {
                arrival.last_visible_at = None;
            }
        }
    }

    pub(crate) fn tick_visible(&mut self, now: Instant, is_video: bool, video_ended: bool) {
        if !self.visible || !self.renderable {
            return;
        }
        if let Some(reveal) = self.reveal.as_mut() {
            if is_video && video_ended && !reveal.ending {
                reveal.ending = true;
                reveal.visible_for = Duration::ZERO;
                reveal.last_visible_at = None;
            }
            reveal.note_visible(now);
            if reveal.expired(is_video && video_ended) {
                self.reveal = None;
                self.start_next();
                if self.reveal.is_none() {
                    self.controller.clear_overlay();
                }
            }
            return;
        }
        if let Some(arrival) = self.arrival.as_mut() {
            arrival.note_visible(now);
            if arrival.visible_for >= ARRIVAL_REVEAL {
                let destination = arrival.building;
                self.arrival = None;
                if matches!(
                    self.controller.overlay(),
                    Some(StageOverlay::Arrival { .. })
                ) {
                    if self.controller.automatic_overlay_allowed()
                        && matches!(self.controller.route(), StageRoute::Realm)
                    {
                        self.controller.navigate(StageRoute::Explore(destination));
                    } else {
                        self.controller.clear_overlay();
                    }
                }
            }
        }
    }

    pub(crate) fn note_media_painted(&mut self) {
        if let Some(reveal) = self.reveal.as_mut()
            && reveal.state == RevealState::Loading
        {
            reveal.state = RevealState::Showing;
            reveal.visible_for = Duration::ZERO;
            reveal.last_visible_at = None;
        }
    }

    pub(crate) fn note_media_fault(&mut self, error: impl Into<String>) {
        if let Some(reveal) = self.reveal.as_mut() {
            reveal.state = RevealState::Fault;
            reveal.error = Some(error.into());
            reveal.visible_for = Duration::ZERO;
            reveal.last_visible_at = None;
        }
    }

    pub(crate) fn document_frame(
        &mut self,
        media: &Media,
    ) -> Result<Option<Arc<document::Document>>, String> {
        self.document.request(media)
    }

    pub(crate) fn document_scroll(
        &mut self,
        width: u16,
        height: u16,
        rows: impl FnOnce() -> usize,
    ) -> u16 {
        let total = match self.document.wrapped_rows {
            Some((old_width, total)) if old_width == width => total,
            _ => {
                let total = rows().min(u16::MAX as usize) as u16;
                self.document.wrapped_rows = Some((width, total));
                total
            }
        };
        self.document.scroll = self.document.scroll.min(total.saturating_sub(height));
        self.document.scroll
    }

    pub(crate) fn scroll_document(&mut self, rows: i32) {
        self.document.scroll =
            (i32::from(self.document.scroll) + rows).clamp(0, u16::MAX as i32) as u16;
    }

    #[cfg(feature = "scryglass-video")]
    pub(crate) fn video_frame(
        &mut self,
        media: &Media,
        width: usize,
        height: usize,
        fps_cap: u8,
        autoplay: bool,
    ) -> Result<VideoFrame, String> {
        let source = media
            .source()
            .ok_or_else(|| "video target is not a local file".to_string())?;
        let snapshot = self.video.request(
            source,
            self.media_request_id(),
            width,
            height,
            fps_cap,
            autoplay,
        );
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        Ok((
            snapshot.frame,
            snapshot.position,
            snapshot.duration,
            snapshot.paused,
            snapshot.ended,
        ))
    }

    #[cfg(not(feature = "scryglass-video"))]
    pub(crate) fn video_frame(
        &mut self,
        _media: &Media,
        _width: usize,
        _height: usize,
        _fps_cap: u8,
        _autoplay: bool,
    ) -> Result<VideoFrame, String> {
        Err("video support is unavailable in this build".to_string())
    }

    #[cfg(feature = "scryglass-video")]
    fn reset_video_control_reveal(&mut self) {
        if let Some(reveal) = self.reveal.as_mut()
            && (reveal.ending || reveal.state == RevealState::Fault)
        {
            reveal.ending = false;
            if reveal.state == RevealState::Fault {
                reveal.state = RevealState::Loading;
                reveal.error = None;
            }
            reveal.visible_for = Duration::ZERO;
            reveal.last_visible_at = None;
        }
    }

    pub(crate) fn toggle_video(&mut self) {
        #[cfg(feature = "scryglass-video")]
        {
            if self.video.snapshot().ended {
                self.reset_video_control_reveal();
            }
            self.video.command(VideoCommand::Toggle);
        }
    }

    pub(crate) fn seek_video(&mut self, _seconds: i64) {
        #[cfg(feature = "scryglass-video")]
        {
            self.reset_video_control_reveal();
            self.video.command(VideoCommand::Seek(_seconds, 0));
        }
    }

    pub(crate) fn restart_video(&mut self) {
        #[cfg(feature = "scryglass-video")]
        {
            self.reset_video_control_reveal();
            self.video.command(VideoCommand::Restart(0));
        }
    }

    pub(crate) fn video_status(&self) -> (Duration, Option<Duration>, bool, bool) {
        #[cfg(feature = "scryglass-video")]
        {
            let state = self.video.snapshot();
            (state.position, state.duration, state.paused, state.ended)
        }
        #[cfg(not(feature = "scryglass-video"))]
        {
            (Duration::ZERO, None, true, false)
        }
    }

    /// Headless visual-dump helper: inspect the settled reveal rather than an
    /// arbitrary cell-dissolve frame. Interactive timing never calls this.
    pub(crate) fn settle_reveal_for_preview(&mut self) {
        if let Some(reveal) = self.reveal.as_mut() {
            reveal.visible_for = Duration::from_millis(350);
        }
    }

    pub(crate) fn browse(&mut self, delta: isize, media: &[Media]) -> Option<usize> {
        if media.is_empty() {
            return None;
        }
        let position = self.selected.min(media.len() - 1);
        let index = (position as isize + delta).rem_euclid(media.len() as isize) as usize;
        self.reveal_media(index, true);
        Some(index)
    }

    pub(crate) fn adjust_look(&mut self, yaw: f32, pitch: f32) {
        self.follow_agent = false;
        self.look_yaw = (self.look_yaw + yaw).rem_euclid(std::f32::consts::TAU);
        self.look_pitch = (self.look_pitch + pitch).clamp(-0.30, 0.30);
    }

    pub(crate) fn adjust_fov(&mut self, delta: f32) {
        self.follow_agent = false;
        self.fov = (self.fov + delta).clamp(0.70, 1.40);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn camera_readout(&self) -> String {
        let fov = self.fov.to_degrees().round() as i16;
        if self.follow_agent {
            return format!("FOLLOW · {fov}°");
        }
        let yaw = if self.look_yaw > std::f32::consts::PI {
            self.look_yaw - std::f32::consts::TAU
        } else {
            self.look_yaw
        }
        .to_degrees()
        .round() as i16;
        let pitch = self.look_pitch.to_degrees().round() as i16;
        format!("FREE · Y{yaw:+}° P{pitch:+}° · {fov}°")
    }

    pub(crate) fn follow(&mut self) {
        self.follow_agent = true;
        self.look_yaw = 0.0;
        self.look_pitch = 0.0;
        self.fov = 1.05;
    }

    /// World/ride braille only. Media uses the viewer's native pixel protocols.
    pub(crate) fn paint_image(frame: &mut Frame, area: Rect, image: &ColoredBrailleImage) {
        let quantize = world_ink_quantizes();
        let width = image.width.min(area.width as usize);
        let height = image.height.min(area.height as usize);
        let ox = area.x + area.width.saturating_sub(width as u16) / 2;
        let oy = area.y + area.height.saturating_sub(height as u16) / 2;
        let buffer = frame.buffer_mut();
        for y in 0..height {
            for x in 0..width {
                let cell = image.cell(x, y).unwrap_or_default();
                let fg = if quantize {
                    quantize_world_ink(cell.fg)
                } else {
                    cell.fg
                };
                if let Some(target) = buffer.cell_mut((ox + x as u16, oy + y as u16)) {
                    target.set_char(cell.glyph);
                    target.set_fg(Color::Rgb(fg[0], fg[1], fg[2]));
                    target.set_bg(Color::Rgb(5, 8, 12));
                }
            }
        }
    }

    pub(crate) fn paint_loading(frame: &mut Frame, area: Rect, tick: u64) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let phases = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let glyph = phases[tick as usize % phases.len()];
        let x = area.x + area.width / 2;
        let y = area.y + area.height / 2;
        if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
            cell.set_char(glyph);
            cell.set_fg(crate::hud::HUD_PHOSPHOR);
            cell.set_style(
                ratatui::style::Style::new()
                    .fg(crate::hud::HUD_PHOSPHOR)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_STILL_RENDER_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

    #[test]
    fn render_lease_admission_survives_abandoned_requests() {
        let lease = acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).unwrap();
        assert!(acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).is_none());

        drop(lease);
        assert!(acquire_still_render(&TEST_STILL_RENDER_IN_FLIGHT).is_some());
    }

    #[test]
    fn automatic_reveals_queue_and_manual_reveal_supersedes() {
        let mut stage = Scryglass::default();
        stage.reveal_media(1, false);
        stage.reveal_media(2, false);
        assert_eq!(stage.active_media(), Some(1));
        assert_eq!(stage.pending_count(), 1);
        stage.reveal_media(3, true);
        assert_eq!(stage.active_media(), Some(3));
        assert_eq!(stage.pending_count(), 0);
        assert!(stage.active_pinned());
    }

    #[test]
    fn camera_is_bounded_and_follow_resets_it() {
        let mut stage = Scryglass::default();
        stage.adjust_look(9.0, 1.0);
        stage.adjust_fov(8.0);
        assert!(!stage.follow_agent);
        assert_eq!(stage.look_pitch, 0.30);
        assert_eq!(stage.fov, 1.40);
        stage.follow();
        assert!(stage.follow_agent);
        assert_eq!(stage.look_yaw, 0.0);
        assert_eq!(stage.fov, 1.05);
    }

    #[test]
    fn still_inspection_pin_is_sticky_against_timer_and_queued_tools() {
        let mut stage = Scryglass::default();
        stage.navigate(StageRoute::Explore(Building::Smithy));
        stage.reveal_media(0, false);
        stage.note_media_painted();
        let request = stage.media_request_id();
        stage.reveal_media(1, false);
        assert_eq!(stage.pending_count(), 1);
        stage.pin_inspection();
        stage.pin_inspection();
        assert_eq!(stage.pending_count(), 0);
        stage.reveal_media(2, false);
        let start = Instant::now();
        stage.set_stage_visibility(true, true);
        stage.tick_visible(start, false, false);
        stage.tick_visible(start + Duration::from_secs(3600), false, false);
        assert_eq!(stage.active_media(), Some(0));
        assert_eq!(stage.media_request_id(), request);
        assert!(stage.active_pinned());
        assert!(stage.back_overlay());
        assert_eq!(
            stage.controller.route(),
            StageRoute::Explore(Building::Smithy)
        );
        assert_eq!(stage.pending_count(), 0);
        assert!(stage.active_media().is_none());
    }

    #[test]
    fn still_reveal_expires_after_eight_visible_seconds() {
        let mut stage = Scryglass::default();
        stage.reveal_media(4, false);
        let reveal = stage.reveal.as_mut().unwrap();
        reveal.state = RevealState::Showing;
        reveal.visible_for = Duration::from_millis(7_999);
        assert!(!reveal.expired(false));
        reveal.visible_for = Duration::from_secs(8);
        assert!(reveal.expired(false));
    }

    #[test]
    fn show_work_compositor_bookkeeping_preserves_visible_reveal_time() {
        let mut stage = Scryglass::default();
        stage.reveal_media(0, false);
        stage.note_media_painted();
        let start = Instant::now();
        for seconds in 0..=8 {
            stage.begin_frame();
            stage.set_stage_visibility(true, true);
            stage.tick_visible(start + Duration::from_secs(seconds), false, false);
            stage.finish_frame();
        }
        assert!(
            stage.active_media().is_none(),
            "continuous compositor frames must consume visible reveal time"
        );
        stage.reveal_media(1, false);
        stage.note_media_painted();
        stage.set_stage_visibility(true, true);
        stage.tick_visible(start, false, false);
        stage.begin_frame();
        stage.finish_frame();
        stage.set_stage_visibility(true, true);
        stage.tick_visible(start + Duration::from_secs(60), false, false);
        assert_eq!(
            stage.active_media(),
            Some(1),
            "a genuinely hidden interval must remain excluded"
        );
    }

    #[test]
    fn realm_is_the_startup_surface_without_an_arrival() {
        let stage = Scryglass::default();
        assert_eq!(stage.world_mode(), WorldMode::Map);
        assert_eq!(stage.arrival(), None);
        assert_eq!(stage.controller.route(), StageRoute::Realm);
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldMap
        );
    }

    #[test]
    fn persistent_world_mode_follows_unwound_route_history() {
        let mut stage = Scryglass::default();
        stage.navigate(StageRoute::Explore(Building::Smithy));
        stage.navigate(StageRoute::Observatory);
        stage.navigate(StageRoute::Realm);
        assert_eq!(stage.world_mode(), WorldMode::Map);

        assert!(stage.controller.back());
        assert_eq!(stage.controller.route(), StageRoute::Observatory);
        assert!(stage.controller.back());
        assert_eq!(
            stage.controller.route(),
            StageRoute::Explore(Building::Smithy)
        );
        assert_eq!(stage.world_mode(), WorldMode::FirstPerson);
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldFirstPerson
        );
    }

    #[test]
    fn controller_back_dismisses_overlay_then_unwinds_a_bounded_route_stack() {
        let mut controller = StageController::default();
        for route in [
            StageRoute::Observatory,
            StageRoute::Quest,
            StageRoute::Workshop,
            StageRoute::Formation,
            StageRoute::Vault,
            StageRoute::Raytrace,
            StageRoute::Loop,
            StageRoute::Explore(Building::Smithy),
            StageRoute::Observatory,
        ] {
            controller.navigate(route);
        }
        assert_eq!(controller.back_stack.len(), 8);
        controller.show_overlay(StageOverlay::Media { index: 4 });
        assert_eq!(
            controller.resolved_scene(false, false, false),
            StageSurface::Still(4)
        );
        assert!(controller.back());
        assert_eq!(controller.route(), StageRoute::Observatory);
        assert!(controller.overlay().is_none());
        assert!(controller.back());
        assert_eq!(controller.route(), StageRoute::Explore(Building::Smithy));
    }

    #[test]
    fn subsystem_route_close_rehomes_media_to_the_recorded_owner() {
        let mut controller = StageController::default();
        controller.navigate(StageRoute::Observatory);
        controller.navigate(StageRoute::Raytrace);
        controller.show_overlay(StageOverlay::Media { index: 3 });

        assert!(controller.leave_route(StageRoute::Raytrace));
        assert_eq!(controller.route(), StageRoute::Observatory);
        assert_eq!(controller.overlay_owner(), Some(StageRoute::Observatory));
        assert_eq!(
            controller.resolved_scene(false, false, false),
            StageSurface::Still(3)
        );
        assert!(controller.back());
        assert_eq!(controller.route(), StageRoute::Observatory);
    }

    #[test]
    fn explicit_reset_clears_route_history_and_overlay_ownership() {
        let mut controller = StageController::default();
        controller.navigate(StageRoute::Observatory);
        controller.navigate(StageRoute::Quest);
        controller.show_overlay(StageOverlay::Media { index: 5 });

        controller.reset(StageRoute::Realm);
        assert_eq!(controller.route(), StageRoute::Realm);
        assert!(controller.overlay().is_none());
        assert_eq!(controller.overlay_owner(), None);
        assert!(
            !controller.back(),
            "reset history must not reopen an abandoned route"
        );
    }

    #[test]
    fn automatic_travel_never_displaces_an_operator_selected_destination() {
        let mut controller = StageController::default();
        controller.navigate(StageRoute::Observatory);
        assert!(!controller.show_overlay(StageOverlay::Journey {
            call_id: ToolEventId("call-9".to_string()),
            destination: Building::Smithy,
        }));
        assert!(!controller.show_overlay(StageOverlay::Arrival {
            destination: Building::Smithy,
        }));
        assert_eq!(
            controller.resolved_scene(false, false, false),
            StageSurface::Observatory
        );
        assert!(controller.overlay().is_none());
        assert!(controller.back());
        assert_eq!(controller.route(), StageRoute::Realm);
    }

    #[test]
    fn world_lesson_is_sourced_overlay_and_back_clears_its_state() {
        let mut stage = Scryglass::default();
        assert!(stage.set_ready_lesson(
            "Poisson",
            "Poisson distribution",
            "A discrete probability distribution."
        ));
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::Lesson
        );
        assert!(stage.lesson().is_some());
        stage.scroll_lesson_down(7);
        assert_eq!(stage.lesson_scroll(), 7);
        assert!(!stage.controller.show_overlay(StageOverlay::Journey {
            call_id: ToolEventId("call-during-lesson".to_string()),
            destination: Building::Smithy,
        }));
        assert!(stage.back_overlay());
        assert_eq!(stage.lesson_scroll(), 0);
        assert!(stage.lesson().is_none());
        assert!(stage.controller.overlay().is_none());
    }

    #[test]
    fn living_catalog_is_a_scrollable_world_overlay_with_clean_back_state() {
        let mut stage = Scryglass::default();
        stage.navigate(StageRoute::Explore(Building::Scriptorium));
        assert!(stage.open_catalog());
        assert!(stage.catalog_open());
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::Catalog
        );

        stage.scroll_teaching_down(11);
        assert_eq!(stage.catalog_scroll(), 11);
        assert!(!stage.controller.show_overlay(StageOverlay::Journey {
            call_id: ToolEventId("call-during-catalog".to_string()),
            destination: Building::Smithy,
        }));
        assert!(stage.set_ready_lesson(
            "matrix",
            "Matrix",
            "A rectangular array in linear algebra."
        ));
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::Lesson
        );

        assert!(stage.back_overlay());
        assert_eq!(stage.lesson_scroll(), 0);
        assert!(!stage.catalog_open());
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldFirstPerson
        );
    }

    #[test]
    fn explicit_media_can_replace_the_catalog_and_retires_its_scroll() {
        let mut stage = Scryglass::default();
        assert!(stage.open_catalog());
        stage.scroll_teaching_bottom();
        stage.reveal_media(4, true);
        assert_eq!(stage.catalog_scroll(), 0);
        assert!(!stage.catalog_open());
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Media { index: 4 })
        ));
    }

    #[test]
    fn operator_media_blocks_lessons_but_can_explicitly_replace_one() {
        let mut stage = Scryglass::default();
        stage.reveal_media(3, true);
        assert!(!stage.set_ready_lesson(
            "matrix",
            "Matrix",
            "A rectangular array in linear algebra."
        ));
        assert!(stage.lesson().is_none());
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Media { index: 3 })
        ));

        stage.back_overlay();
        assert!(stage.set_ready_lesson(
            "matrix",
            "Matrix",
            "A rectangular array in linear algebra."
        ));
        stage.scroll_lesson_bottom();
        stage.reveal_media(4, true);
        assert_eq!(stage.lesson_scroll(), 0);
        assert!(
            stage.lesson().is_none(),
            "explicit media must retire the lesson object as it replaces the overlay"
        );
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Media { index: 4 })
        ));
    }

    #[test]
    fn a_completed_lesson_can_be_replaced_and_world_reset_clears_it() {
        let mut stage = Scryglass::default();
        assert!(stage.set_ready_lesson(
            "matrix",
            "Matrix",
            "A rectangular array in linear algebra."
        ));
        stage.scroll_lesson_down(9);
        assert!(stage.set_ready_lesson(
            "shader",
            "Shader",
            "A program used in computer graphics rendering."
        ));
        assert_eq!(
            stage.lesson().map(crate::term_lookup::QuickLookup::term),
            Some("shader")
        );
        assert_eq!(stage.lesson_scroll(), 0);

        stage.scroll_lesson_down(5);
        stage.return_to_world();
        assert_eq!(stage.lesson_scroll(), 0);
        assert!(stage.lesson().is_none());
        assert!(stage.controller.overlay().is_none());
        assert_eq!(stage.controller.route(), StageRoute::Realm);
    }

    #[test]
    fn a_new_local_lesson_replaces_loading_enrichment_without_waiting() {
        let mut stage = Scryglass::default();
        stage.queue_lesson_outcome(crate::term_lookup::TestLookupOutcome::Success {
            title: "Poisson distribution",
            summary: "A discrete probability distribution.",
            source_url: "https://en.wikipedia.org/?curid=24268",
        });
        assert!(stage.begin_lesson("Poisson".to_string()));
        assert!(stage.lesson_loading());

        stage.queue_lesson_outcome(crate::term_lookup::TestLookupOutcome::Success {
            title: "Matrix",
            summary: "A rectangular array in linear algebra.",
            source_url: "https://en.wikipedia.org/?curid=189106",
        });
        assert!(stage.begin_lesson("matrix".to_string()));
        assert_eq!(
            stage.lesson().map(crate::term_lookup::QuickLookup::term),
            Some("matrix")
        );
        assert_eq!(stage.lesson_scroll(), 0);
    }

    #[test]
    fn catalog_selection_wraps_and_study_back_returns_to_the_same_shelf() {
        let mut stage = Scryglass::default();
        assert!(stage.open_catalog());
        stage.move_catalog_selection(3);
        let selected = stage.catalog_selection();
        assert_eq!(selected, 3);
        assert!(stage.begin_selected_lesson());
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::Lesson
        );
        assert!(stage.back_overlay());
        assert!(stage.catalog_open());
        assert_eq!(stage.catalog_selection(), selected);
        stage.select_catalog_first();
        stage.move_catalog_selection(-1);
        assert_eq!(
            stage.catalog_selection(),
            crate::library::CURRICULUM.len() - 1
        );
    }

    #[test]
    fn camera_readout_is_truthful_about_follow_and_free_look() {
        let mut stage = Scryglass::default();
        assert!(stage.camera_readout().starts_with("FOLLOW"));
        stage.adjust_look(0.17, -0.05);
        let readout = stage.camera_readout();
        assert!(readout.starts_with("FREE"));
        assert!(readout.contains("Y+10°"));
        stage.follow();
        assert!(stage.camera_readout().starts_with("FOLLOW"));
    }

    #[test]
    fn loading_lesson_can_be_cancelled_without_reappearing() {
        let mut stage = Scryglass::default();
        stage.queue_lesson_outcome(crate::term_lookup::TestLookupOutcome::Success {
            title: "Poisson distribution",
            summary: "A discrete probability distribution.",
            source_url: "https://en.wikipedia.org/?curid=24268",
        });
        assert!(stage.begin_lesson("Poisson".to_string()));
        assert!(stage.lesson_loading());
        assert!(stage.back_overlay());
        assert!(stage.lesson().is_none());
        assert!(stage.controller.overlay().is_none());

        stage.poll_lesson(true);
        assert!(
            stage.lesson().is_none(),
            "a cancelled worker result must never reopen the world overlay"
        );
    }

    #[test]
    fn rejected_operator_route_arrival_never_arms_a_timer() {
        let mut stage = Scryglass::default();
        stage.navigate(StageRoute::Observatory);
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Smithy));

        assert_eq!(stage.controller.route(), StageRoute::Observatory);
        assert!(stage.controller.overlay().is_none());
        assert_eq!(stage.arrival(), None);
        assert!(stage.controller.back());
        assert_eq!(stage.controller.route(), StageRoute::Realm);
    }

    #[test]
    fn journey_survives_the_in_transit_none_before_arrival() {
        let mut stage = Scryglass::default();
        stage.sync_arrival(Some(Building::Keep));
        stage.begin_journey(ToolEventId("call-transit".into()), Building::Smithy, false);
        stage.sync_arrival(None);
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Journey {
                destination: Building::Smithy,
                ..
            })
        ));
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldFirstPerson
        );
    }

    #[test]
    fn map_dismisses_an_automatic_journey_to_the_realm() {
        let mut stage = Scryglass::default();
        stage.begin_journey(ToolEventId("call-map".into()), Building::Smithy, false);
        stage.toggle_world_route(Building::Smithy);
        assert_eq!(stage.controller.route(), StageRoute::Realm);
        assert!(stage.controller.overlay().is_none());
        assert_eq!(stage.world_mode(), WorldMode::Map);
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldMap
        );
    }

    #[test]
    fn map_dismisses_journey_from_manual_explore_to_the_realm() {
        let mut stage = Scryglass::default();
        stage.navigate(StageRoute::Explore(Building::Smithy));
        stage.begin_journey(
            ToolEventId("call-map-from-explore".into()),
            Building::Gatehouse,
            false,
        );
        assert_eq!(stage.world_mode(), WorldMode::FirstPerson);

        stage.toggle_world_route(Building::Gatehouse);
        assert_eq!(stage.controller.route(), StageRoute::Realm);
        assert!(stage.controller.overlay().is_none());
        assert_eq!(stage.world_mode(), WorldMode::Map);
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldMap
        );
    }

    #[test]
    fn lifecycle_ceremony_rejects_automatic_travel_but_allows_media() {
        let mut controller = StageController::default();
        assert!(controller.show_overlay(StageOverlay::Lifecycle {
            kind: CeremonyKind::LoopStart,
        }));
        assert!(!controller.show_overlay(StageOverlay::Journey {
            call_id: ToolEventId("call-during-ceremony".to_string()),
            destination: Building::Smithy,
        }));
        assert!(!controller.show_overlay(StageOverlay::Arrival {
            destination: Building::Smithy,
        }));
        assert_eq!(
            controller.resolved_scene(false, false, false),
            StageSurface::Lifecycle
        );

        assert!(controller.show_overlay(StageOverlay::Media { index: 4 }));
        assert_eq!(
            controller.resolved_scene(false, false, false),
            StageSurface::Still(4)
        );
    }

    #[test]
    fn tool_journey_does_not_displace_user_opened_media() {
        let mut stage = Scryglass::default();
        stage.reveal_media(2, true);
        stage.begin_journey(
            ToolEventId("call-during-media".to_string()),
            Building::Smithy,
            false,
        );

        assert_eq!(stage.active_media(), Some(2));
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Media { index: 2 })
        ));
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::Still(2)
        );

        stage.reveal_media(3, true);
        assert_eq!(stage.active_media(), Some(3));
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Media { index: 3 })
        ));
    }

    #[test]
    fn arrival_expires_at_exact_visible_boundary_without_redraw_reset() {
        let mut stage = Scryglass::default();
        let started = Instant::now();
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Smithy));
        stage.set_stage_visibility(true, true);

        stage.tick_visible(started, false, false);
        stage.tick_visible(started + Duration::from_millis(1_249), false, false);
        assert_eq!(stage.arrival(), Some(Building::Smithy));

        // A normal redraw reports the same arrival and must not create a fresh
        // overlay or reset its visible clock.
        stage.sync_arrival(Some(Building::Smithy));
        stage.tick_visible(started + Duration::from_millis(1_250), false, false);
        assert_eq!(stage.arrival(), None);
    }

    #[test]
    fn arrival_expiry_holds_the_destination_in_first_person_without_an_operator_pin() {
        let mut stage = Scryglass::default();
        let started = Instant::now();
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Smithy));
        stage.set_stage_visibility(true, true);

        stage.tick_visible(started, false, false);
        stage.tick_visible(started + ARRIVAL_REVEAL, false, false);

        assert_eq!(stage.arrival(), None);
        assert_eq!(
            stage.controller.route(),
            StageRoute::Explore(Building::Smithy)
        );
        assert!(stage.controller.overlay().is_none());
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldFirstPerson
        );
    }

    #[test]
    fn arrival_expiry_never_displaces_an_operator_pinned_destination() {
        let mut stage = Scryglass::default();
        let started = Instant::now();
        stage.navigate(StageRoute::Explore(Building::Gatehouse));
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Smithy));
        stage.set_stage_visibility(true, true);

        stage.tick_visible(started, false, false);
        stage.tick_visible(started + ARRIVAL_REVEAL, false, false);

        assert_eq!(stage.arrival(), None);
        assert_eq!(
            stage.controller.route(),
            StageRoute::Explore(Building::Gatehouse)
        );
        assert!(stage.controller.overlay().is_none());
        assert_eq!(
            stage.controller.resolved_scene(false, false, false),
            StageSurface::WorldFirstPerson
        );
    }

    #[test]
    fn operator_navigation_cancels_arrival_overlay_and_timer_together() {
        let mut stage = Scryglass::default();
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Smithy));
        assert_eq!(stage.arrival(), Some(Building::Smithy));
        assert!(matches!(
            stage.controller.overlay(),
            Some(StageOverlay::Arrival {
                destination: Building::Smithy
            })
        ));

        stage.navigate(StageRoute::Observatory);
        assert_eq!(stage.arrival(), None);
        assert!(stage.controller.overlay().is_none());
        stage.navigate(StageRoute::Realm);
        stage.set_stage_visibility(true, true);
        assert!(!stage.animating());
    }

    #[test]
    fn hidden_time_does_not_consume_arrival_lifetime() {
        let mut stage = Scryglass::default();
        let started = Instant::now();
        stage.sync_arrival(Some(Building::Keep));
        stage.sync_arrival(Some(Building::Gatehouse));
        stage.set_stage_visibility(true, true);
        stage.tick_visible(started, false, false);
        stage.tick_visible(started + Duration::from_millis(700), false, false);

        stage.set_stage_visibility(false, false);
        stage.tick_visible(started + Duration::from_secs(30), false, false);
        assert_eq!(stage.arrival(), Some(Building::Gatehouse));

        stage.set_stage_visibility(true, true);
        stage.tick_visible(started + Duration::from_secs(30), false, false);
        stage.tick_visible(
            started + Duration::from_secs(30) + Duration::from_millis(549),
            false,
            false,
        );
        assert_eq!(stage.arrival(), Some(Building::Gatehouse));
        stage.tick_visible(
            started + Duration::from_secs(30) + Duration::from_millis(550),
            false,
            false,
        );
        assert_eq!(stage.arrival(), None);
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn contain_fit_preserves_wide_and_tall_video_aspect() {
        assert_eq!(contain_video_cells(1920, 1080, 80, 20), (71, 20));
        assert_eq!(contain_video_cells(1080, 1920, 80, 20), (22, 20));
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn eof_is_a_parked_state_that_restart_can_resume() {
        let started = Instant::now();
        let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), started);
        playback.frame_published(Duration::from_secs(9));
        playback.mark_ended();

        assert!(playback.ended);
        assert!(playback.paused);
        assert!(!playback.is_playing());
        assert_eq!(playback.position, Duration::from_secs(10));

        let restarted = started + Duration::from_secs(30);
        playback.seek_succeeded(Duration::ZERO, true, restarted);
        assert!(!playback.ended);
        assert!(!playback.paused);
        assert!(playback.is_playing());
        assert_eq!(playback.position, Duration::ZERO);
        assert_eq!(
            playback.frame_deadline(Duration::from_millis(250)),
            restarted + Duration::from_millis(250)
        );
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn hidden_and_paused_time_are_removed_from_the_playback_anchor() {
        let started = Instant::now();
        let mut playback = VideoPlayback::new(Some(Duration::from_secs(20)), started);
        playback.frame_published(Duration::from_secs(2));

        playback.set_visible(false, started + Duration::from_secs(2));
        let shown_again = started + Duration::from_secs(62);
        playback.set_visible(true, shown_again);
        assert_eq!(
            playback.frame_deadline(Duration::from_millis(2_500)),
            shown_again + Duration::from_millis(500)
        );

        playback.frame_published(Duration::from_millis(2_500));
        playback.toggle(shown_again + Duration::from_millis(500));
        assert!(playback.paused);
        let resumed = started + Duration::from_secs(122);
        playback.toggle(resumed);
        assert!(!playback.paused);
        assert_eq!(
            playback.frame_deadline(Duration::from_secs(3)),
            resumed + Duration::from_millis(500)
        );
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn reduced_and_still_motion_sampling_keeps_source_time_pacing() {
        assert_eq!(video_sample_every(30.0, 12), 3);
        assert_eq!(video_sample_every(30.0, 6), 5);
        assert_eq!(video_sample_every(30.0, 1), 30);

        let started = Instant::now();
        let playback = VideoPlayback::new(None, started);
        assert_eq!(
            playback.frame_deadline(Duration::from_secs(1)),
            started + Duration::from_secs(1),
            "a 1 fps sampled frame must not be released after a short fixed sleep"
        );
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn seeks_clamp_to_duration_and_a_successful_seek_recovers_a_fault() {
        let started = Instant::now();
        let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), started);
        playback.frame_published(Duration::from_secs(8));
        assert_eq!(playback.seek_target(5), Duration::from_secs(10));
        assert_eq!(playback.seek_target(-20), Duration::ZERO);

        playback.mark_faulted();
        assert!(playback.faulted);
        playback.seek_succeeded(
            Duration::from_secs(4),
            false,
            started + Duration::from_secs(1),
        );
        assert!(!playback.faulted);
        assert!(!playback.ended);
        assert!(playback.paused, "an ordinary seek preserves pause state");
        assert_eq!(playback.position, Duration::from_secs(4));
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn restarting_cancels_the_end_of_reveal_countdown() {
        let mut stage = Scryglass::default();
        stage.reveal_media(0, true);
        let reveal = stage.reveal.as_mut().unwrap();
        reveal.state = RevealState::Showing;
        reveal.ending = true;
        reveal.visible_for = Duration::from_secs(1);

        stage.restart_video();

        let reveal = stage.reveal.as_ref().unwrap();
        assert!(!reveal.ending);
        assert_eq!(reveal.visible_for, Duration::ZERO);
        assert_eq!(reveal.last_visible_at, None);
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn explicit_video_recovery_unlatches_a_reveal_fault() {
        let mut stage = Scryglass::default();
        stage.reveal_media(0, true);
        stage.note_media_fault("seek failed");
        assert_eq!(stage.reveal.as_ref().unwrap().state, RevealState::Fault);

        stage.seek_video(-5);

        let reveal = stage.reveal.as_ref().unwrap();
        assert_eq!(reveal.state, RevealState::Loading);
        assert_eq!(reveal.error, None);
        assert_eq!(reveal.visible_for, Duration::ZERO);
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn show_work_video_rejects_local_network_playlist_before_opening_reference() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let path = std::env::temp_dir().join(format!(
            "angel-show-work-playlist-{}.mp4",
            std::process::id()
        ));
        std::fs::write(&path, format!("#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nhttp://{}/secret.ts\n#EXT-X-ENDLIST\n", listener.local_addr().unwrap())).unwrap();
        let result = dotmax::media::VideoPlayer::new_local(&path);
        assert!(
            result.is_err(),
            "local playlist must be rejected before network references are opened"
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn local_mp4_worker_decodes_and_restarts_after_eof_when_ffmpeg_is_available() {
        let _guard = crate::tests::env_lock();
        let path =
            std::env::temp_dir().join(format!("angel-scryglass-smoke-{}.mp4", std::process::id()));
        let generated = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=0x22cc88:s=16x16:d=0.6:r=4",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(&path)
            .status();
        if !generated.is_ok_and(|status| status.success()) {
            return;
        }
        let media = Media::Video {
            label: "smoke reel".to_string(),
            path: path.display().to_string(),
        };
        // Exercise the same visibility lifecycle as every actual compositor
        // frame. Repeated layout resets must not restart the video clock.
        let displayed_frame = |stage: &mut Scryglass| {
            stage.begin_frame();
            stage.set_stage_visibility(true, true);
            let frame = stage.video_frame(&media, 24, 10, 12, true);
            stage.finish_frame();
            frame
        };
        let mut stage = Scryglass::default();
        stage.reveal_media(0, true);
        let mut decoded = false;
        for _ in 0..100 {
            match displayed_frame(&mut stage) {
                Ok((Some(frame), _, _, _, _)) => {
                    decoded = frame.rgba.width() > 0 && frame.rgba.height() > 0;
                    break;
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("video worker failed: {error}"),
            }
        }
        assert!(decoded, "worker never published decoded RGBA pixels");

        let mut reached_eof = false;
        for _ in 0..150 {
            match displayed_frame(&mut stage) {
                Ok((_, _, _, _, true)) => {
                    reached_eof = true;
                    break;
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("video worker failed before EOF: {error}"),
            }
        }
        assert!(reached_eof, "worker never published EOF state");

        stage.restart_video();
        let mut restarted = false;
        for _ in 0..150 {
            match displayed_frame(&mut stage) {
                Ok((Some(_), position, _, _, false)) if position > Duration::ZERO => {
                    restarted = true;
                    break;
                }
                Ok(_) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => panic!("video worker failed to restart: {error}"),
            }
        }
        let _ = std::fs::remove_file(path);
        assert!(restarted, "EOF worker did not decode again after restart");
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn native_video_pixels_seek_resize_hide_reopen_and_release_the_decoder() {
        let _guard = crate::tests::env_lock();
        let root = std::env::temp_dir().join(format!(
            "angel-native-video-{}-{}",
            std::process::id(),
            NEXT_MEDIA_REQUEST.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("red-to-green.mp4");
        let generated = std::process::Command::new("ffmpeg").args([
            "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
            "color=c=red:s=32x24:r=8:d=1[r];color=c=lime:s=32x24:r=8:d=2[g];[r][g]concat=n=2:v=1:a=0",
            "-r", "8", "-pix_fmt", "yuv420p", "-y",
        ]).arg(&path).status();
        if !generated.is_ok_and(|status| status.success()) {
            eprintln!("SKIP native video fixture: ffmpeg CLI unavailable");
            return;
        }
        let media = Media::Video {
            label: "controlled red-to-green".into(),
            path: path.display().to_string(),
        };
        let mut stage = Scryglass::default();
        stage.reveal_media(0, true);
        let poll = |stage: &mut Scryglass, width, height| {
            stage.begin_frame();
            stage.set_stage_visibility(true, true);
            let result = stage.video_frame(&media, width, height, 8, false).unwrap();
            stage.finish_frame();
            result
        };
        let wait_frame = |stage: &mut Scryglass, width, height| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let result = poll(stage, width, height);
                if let Some(pixels) = result.0 {
                    break (pixels, result.1, result.3);
                }
                assert!(Instant::now() < deadline, "native pixels did not arrive");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let (red, position, paused) = wait_frame(&mut stage, 40, 12);
        assert!(paused, "motion-off must publish one frame then pause");
        assert_eq!(position, Duration::ZERO);
        assert!(
            red.rgba
                .pixels()
                .all(|p| p[0] > 200 && p[1] < 30 && p[2] < 30)
        );
        assert_eq!(red.identity.request_id, stage.media_request_id());

        stage.seek_video(1);
        assert!(
            stage.video.snapshot().frame.is_none(),
            "seek invalidates old pixels synchronously"
        );
        let (green, position, paused) = wait_frame(&mut stage, 40, 12);
        assert!(paused, "paused seek must not start playback");
        assert!(position >= Duration::from_secs(1));
        assert_ne!(red.identity.generation, green.identity.generation);
        assert!(
            green
                .rgba
                .pixels()
                .all(|p| p[1] > 200 && p[0] < 30 && p[2] < 30)
        );

        let (resized, resized_position, _) = wait_frame(&mut stage, 80, 30);
        assert_eq!(
            resized.identity, green.identity,
            "resize must retain the decoder generation"
        );
        assert_eq!(
            resized_position, position,
            "resizing must not reset or advance a paused clock"
        );

        stage.toggle_video();
        let deadline = Instant::now() + Duration::from_secs(3);
        while poll(&mut stage, 80, 30).1 <= position {
            assert!(Instant::now() < deadline, "resized decoder did not resume");
            std::thread::sleep(Duration::from_millis(5));
        }
        stage.begin_frame();
        stage.set_stage_visibility(false, false);
        stage.finish_frame();
        // Barrier: wait for the same worker to consume the visibility command.
        std::thread::sleep(Duration::from_millis(30));
        let hidden = stage.video.snapshot();
        assert!(
            !hidden.paused,
            "visibility pauses scheduling without changing play intent"
        );
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(stage.video.snapshot().position, hidden.position);
        assert_eq!(
            stage.video.snapshot().frame.unwrap().sequence,
            hidden.frame.unwrap().sequence
        );

        stage.reveal_media(0, true);
        assert!(
            stage.video.active.is_none(),
            "reopen stops the previous decoder"
        );
        let (reopened, position, _) = wait_frame(&mut stage, 40, 12);
        assert_ne!(reopened.identity.request_id, green.identity.request_id);
        assert_ne!(reopened.identity.generation, green.identity.generation);
        assert_eq!(position, Duration::ZERO);
        let active = stage.video.active.as_ref().unwrap();
        let cancelled = Arc::clone(&active.cancelled);
        // Closing must bypass an accumulated control backlog, not wait for
        // hundreds of scaler resizes before reaching the queued Stop message.
        for index in 0..1_000 {
            active
                .tx
                .send(VideoCommand::Resize(40 + index % 2, 12))
                .unwrap();
        }
        stage.return_to_world();
        assert!(cancelled.load(Ordering::Acquire));
        assert!(stage.video.active.is_none());
        let deadline = Instant::now() + Duration::from_secs(2);
        while VIDEO_RENDER_IN_FLIGHT.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !VIDEO_RENDER_IN_FLIGHT.load(Ordering::Acquire),
            "close must release the single decoder lease"
        );
        let bounded = contain_video_cells(16_384, 16_384, usize::MAX, usize::MAX);
        assert!(bounded.0 * bounded.1 * 8 <= 98_304);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(feature = "scryglass-video")]
    #[test]
    fn native_video_old_generation_cannot_publish_state_or_fault_after_seek() {
        let mailbox = Arc::new(Mutex::new(VideoSnapshot {
            generation: 2,
            position: Duration::from_secs(8),
            ..VideoSnapshot::default()
        }));
        let mut playback = VideoPlayback::new(Some(Duration::from_secs(10)), Instant::now());
        publish_video_state(&mailbox, &playback, true, 1);
        publish_video_error(&mailbox, &mut playback, "old decode failed", 1);
        let snapshot = mailbox.lock().unwrap();
        assert_eq!(snapshot.position, Duration::from_secs(8));
        assert_eq!(snapshot.error, None);
        drop(snapshot);
        publish_video_error(&mailbox, &mut playback, "current decode failed", 2);
        assert_eq!(
            mailbox.lock().unwrap().error.as_deref(),
            Some("current decode failed")
        );
    }

    /// A mid-journey world whose ride frames feed the stage paint during turns.
    fn riding_world() -> crate::world_viz::World {
        let mut world = crate::world_viz::World::new(2024);
        world.note_tool_call("write_file", "forging a plate");
        for _ in 0..8 {
            world.tick();
        }
        world
    }

    /// (distinct fg colors, SGR fg runs, estimated flush bytes) for a full
    /// repaint of `image`, mirroring what `paint_image` writes and how the
    /// crossterm backend emits it: one ~19-byte truecolor SGR per fg change
    /// along the row-major cell walk (fg state survives MoveTo), 3 UTF-8
    /// bytes per braille glyph, ~8 bytes of MoveTo per row.
    fn ink_stats(
        image: &ColoredBrailleImage,
        quant: impl Fn([u8; 3]) -> [u8; 3],
    ) -> (usize, usize, usize) {
        let mut distinct = std::collections::HashSet::new();
        let mut runs = 0usize;
        let mut last: Option<[u8; 3]> = None;
        for cell in &image.cells {
            let fg = quant(cell.fg);
            distinct.insert(fg);
            if last != Some(fg) {
                runs += 1;
                last = Some(fg);
            }
        }
        let bytes = runs * 19 + image.cells.len() * 3 + image.height * 8;
        (distinct.len(), runs, bytes)
    }

    /// (changed cells, SGR headers, estimated flush bytes) for the diff walk
    /// between two consecutive painted frames — the steady-state cost while
    /// the ride animates. MoveTo (~8B) per gap in the changed-cell sequence,
    /// one truecolor header (~19B) per fg change along it.
    fn diff_stats(
        a: &ColoredBrailleImage,
        b: &ColoredBrailleImage,
        quant: impl Fn([u8; 3]) -> [u8; 3],
    ) -> (usize, usize, usize) {
        let mut changed = 0usize;
        let mut headers = 0usize;
        let mut bytes = 0usize;
        let mut last_fg: Option<[u8; 3]> = None;
        let mut last_index: Option<usize> = None;
        for (index, (ca, cb)) in a.cells.iter().zip(&b.cells).enumerate() {
            let fg = quant(cb.fg);
            if ca.glyph == cb.glyph && quant(ca.fg) == fg {
                continue;
            }
            changed += 1;
            if last_index != Some(index.wrapping_sub(1)) {
                bytes += 8;
            }
            if last_fg != Some(fg) {
                headers += 1;
                last_fg = Some(fg);
            }
            bytes += 3;
            last_index = Some(index);
        }
        (changed, headers, bytes + headers * 19)
    }

    #[test]
    fn world_ink_quantizer_is_gentle_and_idempotent() {
        let _guard = crate::tests::env_lock();
        // Default RGB444: max channel error 8/255.
        let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
        for channel in 0..=255u8 {
            let [q, _, _] = quantize_world_ink([channel; 3]);
            assert!(
                u8::abs_diff(q, channel) <= 8,
                "channel {channel} moved to {q}; RGB444 rounding must stay within 8",
            );
            assert_eq!(
                quantize_world_ink([q; 3])[0],
                q,
                "painted values must re-quantize to themselves",
            );
        }
        // Opt-back RGB555: max channel error 4/255.
        let _rgb555 = crate::tests::TestEnvGuard::set("ANGEL_WORLD_INK", "rgb555");
        for channel in 0..=255u8 {
            let [q, _, _] = quantize_world_ink([channel; 3]);
            assert!(
                u8::abs_diff(q, channel) <= 4,
                "channel {channel} moved to {q}; RGB555 rounding must stay within 4",
            );
        }
    }

    /// SGR-flood measurement: the terminal-parse cost of a genuinely new ride
    /// frame is set by the ink's run structure, not the changed-cell count
    /// alone. Guards the run structure the world paint relies on; prints the
    /// tallies under --nocapture for perf work.
    ///
    /// Z5 pinned this to the mesh ride, which is the frame it has always
    /// measured and whose floor it states. The Dotmax world — the
    /// launch default — is measured beside it in
    /// [`dotmax_frame_ink_runs_are_measured_beside_the_ride`], which holds
    /// the monotonicity claims and reports the run structure without
    /// restating a floor the tile dither does not meet.
    #[test]
    fn ride_frame_ink_runs_bound_the_sgr_flood() {
        let _guard = crate::tests::env_lock();
        let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
        let _pin = crate::world_viz::world3d::pin_world3d();
        let mut world = riding_world();
        let first = world
            .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
            .expect("ride frame");
        for _ in 0..3 {
            world.tick();
        }
        let second = world
            .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
            .expect("ride frame");
        assert_ne!(first, second, "ticked world must render a new frame");

        let inked = first
            .cells
            .iter()
            .filter(|cell| cell.glyph != '\u{2800}')
            .count();
        assert!(inked > 400, "ride frame should carry substantial ink");

        let raw = ink_stats(&first, |fg| fg);
        let q4 = ink_stats(&first, |fg| {
            // Force the RGB444 math independent of env so this bench is stable.
            fg.map(|channel| {
                let level = (u16::from(channel) * 15 + 127) / 255;
                ((level * 255 + 7) / 15) as u8
            })
        });
        let q5 = ink_stats(&first, |fg| {
            fg.map(|channel| {
                let level = (u16::from(channel) * 31 + 127) / 255;
                ((level * 255 + 15) / 31) as u8
            })
        });
        let raw_diff = diff_stats(&first, &second, |fg| fg);
        let q4_diff = diff_stats(&first, &second, |fg| {
            fg.map(|channel| {
                let level = (u16::from(channel) * 15 + 127) / 255;
                ((level * 255 + 7) / 15) as u8
            })
        });
        let q5_diff = diff_stats(&first, &second, |fg| {
            fg.map(|channel| {
                let level = (u16::from(channel) * 31 + 127) / 255;
                ((level * 255 + 15) / 31) as u8
            })
        });
        println!(
            "stage ink: {}x{} cells={} inked={}",
            first.width,
            first.height,
            first.cells.len(),
            inked,
        );
        for (label, (distinct, runs, bytes), (changed, headers, diff_bytes)) in [
            ("raw", raw, raw_diff),
            ("rgb555", q5, q5_diff),
            ("rgb444", q4, q4_diff),
        ] {
            println!(
                "  {label}: distinct={distinct} sgr-runs={runs} mean-run={:.2} \
                 full-repaint≈{bytes}B | next-frame changed={changed} headers={headers} ≈{diff_bytes}B",
                first.cells.len() as f32 / runs.max(1) as f32,
            );
        }
        assert!(
            q4.0 <= q5.0 && q4.1 <= q5.1 && q4_diff.2 <= q5_diff.2,
            "RGB444 must never add colors, runs, or flush bytes vs RGB555",
        );
        assert!(
            q5.0 <= raw.0 && q5.1 <= raw.1 && q5_diff.2 <= raw_diff.2,
            "quantizing must never add colors, runs, or flush bytes",
        );
        // Floor the default-on RGB444 paint must hold: quantized ink keeps
        // same-color runs long enough that a full repaint stays well under
        // one truecolor header per cell.
        assert!(
            q4.1 * 2 < first.cells.len(),
            "quantized fg runs ({}) approach one-per-cell; the SGR flood is back",
            q4.1,
        );
    }

    /// The same measurement for the view Z5 made the launch default.
    ///
    /// The Dotmax world paints a true-3D scene through the
    /// braille bridge, and the two-value rock / sparse-mark grounds alternate
    /// colour from cell to cell by design — so its mean run is near 1 and it
    /// does **not** meet the ride's one-header-per-two-cells floor. That is a
    /// real cost of the flip, recorded here rather than hidden by relaxing the
    /// ride's law: the monotonicity claims (quantizing never *adds* colours,
    /// runs or flush bytes) still hold and are asserted, and the tallies print
    /// under --nocapture so a later pass can attack the run structure.
    #[test]
    fn dotmax_frame_ink_runs_are_measured_beside_the_ride() {
        let _guard = crate::tests::env_lock();
        let _mode = crate::tests::TestEnvGuard::unset("ANGEL_WORLD_INK");
        let _pin = crate::world_viz::world3d::pin(crate::world_viz::world3d::WorldView::Mesh3d);
        let mut world = riding_world();
        let first = world
            .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
            .expect("Dotmax frame");
        for _ in 0..3 {
            world.tick();
        }
        let second = world
            .scryglass_frame_paced(100, 38, false, 0.0, 0.0, 1.05)
            .expect("Dotmax frame");

        let quant4 = |fg: [u8; 3]| {
            fg.map(|channel| {
                let level = (u16::from(channel) * 15 + 127) / 255;
                ((level * 255 + 7) / 15) as u8
            })
        };
        let quant5 = |fg: [u8; 3]| {
            fg.map(|channel| {
                let level = (u16::from(channel) * 31 + 127) / 255;
                ((level * 255 + 15) / 31) as u8
            })
        };
        let raw = ink_stats(&first, |fg| fg);
        let q5 = ink_stats(&first, quant5);
        let q4 = ink_stats(&first, quant4);
        let raw_diff = diff_stats(&first, &second, |fg| fg);
        let q5_diff = diff_stats(&first, &second, quant5);
        let q4_diff = diff_stats(&first, &second, quant4);
        println!(
            "Dotmax ink: {}x{} cells={} raw-runs={} rgb555-runs={} rgb444-runs={} \
             mean-run={:.2}",
            first.width,
            first.height,
            first.cells.len(),
            raw.1,
            q5.1,
            q4.1,
            first.cells.len() as f32 / q4.1.max(1) as f32,
        );
        assert!(
            q4.0 <= q5.0 && q4.1 <= q5.1 && q4_diff.2 <= q5_diff.2,
            "RGB444 must never add colors, runs, or flush bytes vs RGB555",
        );
        assert!(
            q5.0 <= raw.0 && q5.1 <= raw.1 && q5_diff.2 <= raw_diff.2,
            "quantizing must never add colors, runs, or flush bytes",
        );
    }
}
