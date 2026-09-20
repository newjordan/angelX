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
#[path = "../../tests/cockpit/app/scryglass__tests.rs"]
mod tests;
