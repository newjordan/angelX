//! Bounded Cockpit-owned contract and process lifecycle for the Kitty WebGPU portal.
//!
//! The Cockpit never acquires a GPU device itself. This module projects the
//! read-only [`crate::agentviz`] activity signal into a versioned packet,
//! coalesces updates behind a one-flight boundary, and invokes the isolated
//! surface-free renderer with bounded input, output, and wall time. The existing
//! text Miniviz remains authoritative whenever the renderer or Kitty adapter is
//! unavailable.

use crate::agentviz::ActivitySnapshot;
use crate::sandbox::process_owner::OwnedCommandExt;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_PACKET_BYTES: usize = 4 * 1024;
pub const MAX_STAGE_BYTES: usize = 64;
pub const MAX_SEATS: usize = 16;
pub const MAX_SEAT_LABEL_BYTES: usize = 48;

/// The first renderer format is fixed-size RGBA8 sRGB. Raw pixels avoid a PNG
/// encode/decode cycle between the renderer and `ratatui-image`; the eventual
/// frame adapter must still reject any payload that differs from these limits.
pub const FRAME_WIDTH: u32 = 320;
pub const FRAME_HEIGHT: u32 = 180;
pub const FRAME_CHANNELS: usize = 4;
pub const FRAME_BYTES: usize = FRAME_WIDTH as usize * FRAME_HEIGHT as usize * FRAME_CHANNELS;
const MAX_RECEIPT_BYTES: usize = 2 * 1024;
const MAX_RENDERER_OUTPUT_BYTES: usize = 4 + MAX_RECEIPT_BYTES + FRAME_BYTES;
const RENDER_TIMEOUT: Duration = Duration::from_secs(15);
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AngelVizPaletteV1 {
    #[default]
    Noir,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AngelVizHealthV1 {
    Nominal,
    Degraded,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AngelVizSeatV1 {
    pub slot: u8,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AngelVizStatusV1 {
    pub active_seats: u8,
    pub omitted_seats: u16,
    pub health: AngelVizHealthV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AngelVizStateV1 {
    pub schema_version: u16,
    pub sequence: u64,
    pub published_at_monotonic_ms: u64,
    pub stage: Option<String>,
    pub seats: Vec<AngelVizSeatV1>,
    pub status: AngelVizStatusV1,
    pub palette: AngelVizPaletteV1,
}

impl AngelVizStateV1 {
    /// Project the current activity into the renderer contract. All untrusted
    /// display strings are truncated at a UTF-8 boundary before serialization.
    pub fn project(
        activity: &ActivitySnapshot,
        published_at_monotonic_ms: u64,
        health: AngelVizHealthV1,
        palette: AngelVizPaletteV1,
    ) -> Self {
        let (stage, seats, omitted_seats) = match activity.stage.as_ref() {
            Some(snapshot) => {
                let seats = snapshot
                    .agents
                    .iter()
                    .take(MAX_SEATS)
                    .enumerate()
                    .map(|(slot, label)| AngelVizSeatV1 {
                        slot: slot as u8,
                        label: truncate_utf8(label, MAX_SEAT_LABEL_BYTES),
                    })
                    .collect::<Vec<_>>();
                let omitted = snapshot.agents.len().saturating_sub(seats.len());
                (
                    Some(truncate_utf8(&snapshot.name, MAX_STAGE_BYTES)),
                    seats,
                    omitted.min(u16::MAX as usize) as u16,
                )
            }
            None => (None, Vec::new(), 0),
        };
        Self {
            schema_version: SCHEMA_VERSION,
            sequence: activity.sequence,
            published_at_monotonic_ms,
            status: AngelVizStatusV1 {
                active_seats: seats.len() as u8,
                omitted_seats,
                health,
            },
            stage,
            seats,
            palette,
        }
    }

    pub fn validate(&self) -> Result<(), PacketError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(PacketError::WrongSchema {
                actual: self.schema_version,
            });
        }
        if self
            .stage
            .as_ref()
            .is_some_and(|stage| stage.len() > MAX_STAGE_BYTES)
        {
            return Err(PacketError::StageTooLong);
        }
        if self.seats.len() > MAX_SEATS {
            return Err(PacketError::TooManySeats {
                actual: self.seats.len(),
            });
        }
        for (index, seat) in self.seats.iter().enumerate() {
            if seat.slot as usize != index {
                return Err(PacketError::InvalidSeatSlot {
                    index,
                    actual: seat.slot,
                });
            }
            if seat.label.len() > MAX_SEAT_LABEL_BYTES {
                return Err(PacketError::SeatLabelTooLong { index });
            }
        }
        if self.status.active_seats as usize != self.seats.len() {
            return Err(PacketError::ActiveSeatMismatch {
                reported: self.status.active_seats,
                actual: self.seats.len(),
            });
        }
        if self.stage.is_none()
            && (!self.seats.is_empty()
                || self.status.active_seats != 0
                || self.status.omitted_seats != 0)
        {
            return Err(PacketError::NonEmptyIdleState);
        }
        let encoded = serde_json::to_vec(self)
            .map_err(|error| PacketError::Serialization(error.to_string()))?;
        if encoded.len() > MAX_PACKET_BYTES {
            return Err(PacketError::PacketTooLarge {
                actual: encoded.len(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacketError {
    WrongSchema { actual: u16 },
    StageTooLong,
    TooManySeats { actual: usize },
    InvalidSeatSlot { index: usize, actual: u8 },
    SeatLabelTooLong { index: usize },
    ActiveSeatMismatch { reported: u8, actual: usize },
    NonEmptyIdleState,
    PacketTooLarge { actual: usize },
    Serialization(String),
    StaleSequence { previous: u64, incoming: u64 },
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSchema { actual } => {
                write!(f, "unsupported portal schema {actual}")
            }
            Self::StageTooLong => write!(f, "portal stage exceeds {MAX_STAGE_BYTES} bytes"),
            Self::TooManySeats { actual } => {
                write!(
                    f,
                    "portal packet has {actual} seats; maximum is {MAX_SEATS}"
                )
            }
            Self::InvalidSeatSlot { index, actual } => {
                write!(f, "portal seat {index} has non-canonical slot {actual}")
            }
            Self::SeatLabelTooLong { index } => write!(
                f,
                "portal seat {index} label exceeds {MAX_SEAT_LABEL_BYTES} bytes"
            ),
            Self::ActiveSeatMismatch { reported, actual } => write!(
                f,
                "portal reports {reported} active seats but contains {actual}"
            ),
            Self::NonEmptyIdleState => write!(f, "idle portal packet contains active stage data"),
            Self::PacketTooLarge { actual } => write!(
                f,
                "portal packet is {actual} bytes; maximum is {MAX_PACKET_BYTES}"
            ),
            Self::Serialization(error) => write!(f, "portal packet serialization failed: {error}"),
            Self::StaleSequence { previous, incoming } => {
                write!(f, "portal sequence {incoming} is not newer than {previous}")
            }
        }
    }
}

/// Retains only the newest unpublished packet and never blocks a Cockpit
/// producer while a previous frame is being rendered.
#[derive(Debug, Default)]
pub struct LatestStateSlot {
    latest: Option<AngelVizStateV1>,
    last_sequence: Option<u64>,
    published: u64,
    coalesced: u64,
    rejected: u64,
}

impl LatestStateSlot {
    pub fn publish(&mut self, state: AngelVizStateV1) -> Result<(), PacketError> {
        if let Err(error) = state.validate() {
            self.rejected = self.rejected.saturating_add(1);
            return Err(error);
        }
        if let Some(previous) = self.last_sequence
            && state.sequence <= previous
        {
            self.rejected = self.rejected.saturating_add(1);
            return Err(PacketError::StaleSequence {
                previous,
                incoming: state.sequence,
            });
        }
        if self.latest.is_some() {
            self.coalesced = self.coalesced.saturating_add(1);
        }
        self.last_sequence = Some(state.sequence);
        self.latest = Some(state);
        self.published = self.published.saturating_add(1);
        Ok(())
    }

    pub fn take_latest(&mut self) -> Option<AngelVizStateV1> {
        self.latest.take()
    }

    #[cfg(test)]
    pub fn counts(&self) -> (u64, u64, u64) {
        (self.published, self.coalesced, self.rejected)
    }
}

#[derive(Clone, Debug)]
pub struct PortalFrame {
    pub sequence: u64,
    pub pixels: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct PortalPresentation {
    pub stage: String,
    pub active_seats: usize,
    pub omitted_seats: usize,
    /// Seats that have come back with a result (`SeatState::Returned`) —
    /// renders as "3/6 back" pips beside the seat count.
    pub returned_seats: usize,
    pub frame: Option<PortalFrame>,
}

/// The stage the portal is currently presenting, folded from the activity
/// signal. Kept separate from the renderer packet: seat-return pips update
/// here on every seq bump whether or not a frame is in flight.
#[derive(Clone, Debug)]
struct ActiveStage {
    sequence: u64,
    stage: String,
    active_seats: usize,
    omitted_seats: usize,
    returned_seats: usize,
}

struct PendingRender {
    sequence: u64,
    rx: mpsc::Receiver<Result<PortalFrame, String>>,
}

/// Non-blocking Cockpit-side lifecycle for the isolated one-request renderer.
///
/// A worker is started only for Kitty-capable interactive viewers. While it is
/// busy, [`LatestStateSlot`] replaces any unpublished intermediate state with
/// the newest sequence. Failed frames disappear back to the terminal-native
/// visualization instead of leaving a stale portal on screen.
pub struct PortalRuntime {
    renderer: Option<PathBuf>,
    started: Instant,
    latest: LatestStateSlot,
    pending: Option<PendingRender>,
    frame: Option<PortalFrame>,
    observed_sequence: Option<u64>,
    active_stage: Option<ActiveStage>,
    failed_sequence: Option<u64>,
}

impl PortalRuntime {
    pub fn discover(kitty_capable: bool) -> Self {
        if cfg!(test) {
            return Self::with_renderer(None);
        }
        let renderer = if kitty_capable && portal_enabled_from_env() {
            discover_renderer()
        } else {
            None
        };
        if let Some(path) = renderer.as_ref() {
            eprintln!("PORTAL: renderer={}", path.display());
        }
        Self::with_renderer(renderer)
    }

    fn with_renderer(renderer: Option<PathBuf>) -> Self {
        Self {
            renderer,
            started: Instant::now(),
            latest: LatestStateSlot::default(),
            pending: None,
            frame: None,
            observed_sequence: None,
            active_stage: None,
            failed_sequence: None,
        }
    }

    /// A7: when no isolated renderer is installed, the portal is inert — skip
    /// activity locks and packet projection on the UI advance hot path.
    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    pub fn advance(&mut self, activity: &ActivitySnapshot) {
        if self.renderer.is_none() {
            return;
        }
        self.poll_pending();
        self.note_activity(activity);
        self.start_latest();
    }

    /// Fold a new activity revision into the runtime: refresh the presented
    /// stage (including its returned-seat count) and queue a packet for the
    /// renderer. A seat-state update re-publishes the *same* stage under a
    /// newer sequence, so the current frame is carried across such bumps —
    /// the packet is visually identical and the portal must not flash back to
    /// the text viz while the refreshed render is in flight.
    fn note_activity(&mut self, activity: &ActivitySnapshot) {
        if self.observed_sequence == Some(activity.sequence) {
            return;
        }
        self.observed_sequence = Some(activity.sequence);
        let previous = self.active_stage.take();
        self.active_stage = activity.stage.as_ref().map(|stage| {
            let active_seats = stage.agents.len().min(MAX_SEATS);
            ActiveStage {
                sequence: activity.sequence,
                stage: truncate_utf8(&stage.name, MAX_STAGE_BYTES),
                active_seats,
                omitted_seats: stage.agents.len().saturating_sub(active_seats),
                returned_seats: stage.returned(),
            }
        });
        if let (Some(prev), Some(next)) = (previous.as_ref(), self.active_stage.as_ref())
            && prev.stage == next.stage
            && prev.active_seats == next.active_seats
            && prev.omitted_seats == next.omitted_seats
            && let Some(frame) = self.frame.as_mut()
            && frame.sequence == prev.sequence
        {
            frame.sequence = next.sequence;
        }
        self.failed_sequence = None;
        let health = if activity.stage.is_some() {
            AngelVizHealthV1::Nominal
        } else {
            AngelVizHealthV1::Unknown
        };
        let packet = AngelVizStateV1::project(
            activity,
            self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            health,
            AngelVizPaletteV1::Noir,
        );
        if let Err(error) = self.latest.publish(packet) {
            self.failed_sequence = Some(activity.sequence);
            eprintln!("PORTAL: rejected activity packet: {error}");
        }
    }

    pub fn presentation(&self) -> Option<PortalPresentation> {
        let active = self.active_stage.as_ref()?;
        if self.failed_sequence == Some(active.sequence) {
            return None;
        }
        let frame = self
            .frame
            .as_ref()
            .filter(|frame| frame.sequence == active.sequence)
            .cloned();
        Some(PortalPresentation {
            stage: active.stage.clone(),
            active_seats: active.active_seats,
            omitted_seats: active.omitted_seats,
            returned_seats: active.returned_seats,
            frame,
        })
    }

    pub fn is_rendering(&self) -> bool {
        self.pending.is_some() || self.latest.latest.is_some()
    }

    #[cfg(test)]
    pub fn force_rendering_for_test(&mut self) {
        let (_tx, rx) = mpsc::channel();
        self.pending = Some(PendingRender { sequence: 1, rx });
    }

    #[cfg(test)]
    pub fn presentation_for_test(stage: &str, active_seats: usize) -> Self {
        let mut runtime = Self::with_renderer(None);
        runtime.observed_sequence = Some(1);
        runtime.active_stage = Some(ActiveStage {
            sequence: 1,
            stage: truncate_utf8(stage, MAX_STAGE_BYTES),
            active_seats: active_seats.min(MAX_SEATS),
            omitted_seats: active_seats.saturating_sub(MAX_SEATS),
            returned_seats: 0,
        });
        runtime
    }

    fn poll_pending(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        match pending.rx.try_recv() {
            Ok(Ok(frame)) => {
                if self.observed_sequence == Some(frame.sequence) {
                    self.failed_sequence = None;
                    self.frame = Some(frame);
                }
            }
            Ok(Err(error)) => {
                if self.observed_sequence == Some(pending.sequence) {
                    self.failed_sequence = Some(pending.sequence);
                    eprintln!(
                        "PORTAL: renderer failed for sequence {}: {error}",
                        pending.sequence
                    );
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.pending = Some(pending);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if self.observed_sequence == Some(pending.sequence) {
                    self.failed_sequence = Some(pending.sequence);
                    eprintln!(
                        "PORTAL: renderer worker disconnected for sequence {}",
                        pending.sequence
                    );
                }
            }
        }
    }

    fn start_latest(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let Some(packet) = self.latest.take_latest() else {
            return;
        };
        let Some(renderer) = self.renderer.clone() else {
            return;
        };
        let sequence = packet.sequence;
        let (tx, rx) = mpsc::channel();
        match std::thread::Builder::new()
            .name("angel-webgpu-portal".to_string())
            .spawn(move || {
                let _ = tx.send(invoke_renderer(&renderer, &packet));
            }) {
            Ok(_) => {
                self.pending = Some(PendingRender { sequence, rx });
            }
            Err(error) => {
                self.failed_sequence = Some(sequence);
                eprintln!("PORTAL: could not start renderer worker: {error}");
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RendererReceipt {
    schema_version: u16,
    sequence: u64,
    width: u32,
    height: u32,
    format: String,
    frame_bytes: usize,
    production_time_us: u64,
    adapter: Option<String>,
    error: Option<RendererError>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RendererError {
    code: String,
    message: String,
}

fn portal_enabled_from_env() -> bool {
    std::env::var("ANGEL_WEBGPU_PORTAL")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "off" | "false" | "no"
            )
        })
        .unwrap_or(true)
}

fn discover_renderer() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ANGEL_PORTAL_RENDERER")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
    {
        return path.is_file().then_some(path);
    }
    let executable = std::env::current_exe().ok()?;
    let sibling = executable.parent()?.join("angel-portal-renderer");
    if sibling.is_file() {
        return Some(sibling);
    }
    let renderer_target = crate::runtime_paths::cockpit_dir()
        .join("portal-renderer")
        .join("target");
    ["debug", "release"]
        .into_iter()
        .map(|profile| renderer_target.join(profile).join("angel-portal-renderer"))
        .find(|candidate| candidate.is_file())
}

fn invoke_renderer(renderer: &Path, packet: &AngelVizStateV1) -> Result<PortalFrame, String> {
    packet.validate().map_err(|error| error.to_string())?;
    let encoded = serde_json::to_vec(packet).map_err(|error| format!("encode packet: {error}"))?;
    if encoded.is_empty() || encoded.len() > MAX_PACKET_BYTES {
        return Err(format!(
            "encoded packet length {} outside 1..={MAX_PACKET_BYTES}",
            encoded.len()
        ));
    }
    let mut child = Command::new(renderer)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn_owned()
        .map_err(|error| format!("spawn {}: {error}", renderer.display()))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("renderer stdout was not piped".to_string());
    };
    let reader = match std::thread::Builder::new()
        .name("angel-webgpu-portal-output".to_string())
        .spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take((MAX_RENDERER_OUTPUT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
                .map_err(|error| format!("read renderer output: {error}"))
        }) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("start renderer output reader: {error}"));
        }
    };

    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "renderer stdin was not piped".to_string())
        .and_then(|mut stdin| {
            stdin
                .write_all(&(encoded.len() as u32).to_be_bytes())
                .and_then(|_| stdin.write_all(&encoded))
                .and_then(|_| stdin.flush())
                .map_err(|error| format!("write renderer packet: {error}"))
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        return Err(error);
    }

    let deadline = Instant::now() + RENDER_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(CHILD_POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!(
                    "renderer exceeded {} ms",
                    RENDER_TIMEOUT.as_millis()
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!("wait for renderer: {error}"));
            }
        }
    };
    let output = reader
        .join()
        .map_err(|_| "renderer output reader panicked".to_string())??;
    if output.len() > MAX_RENDERER_OUTPUT_BYTES {
        return Err(format!(
            "renderer output exceeds {MAX_RENDERER_OUTPUT_BYTES} bytes"
        ));
    }
    if !status.success() {
        return Err(format!("renderer exited with {status}"));
    }
    parse_renderer_output(packet.sequence, &output)
}

fn parse_renderer_output(expected_sequence: u64, output: &[u8]) -> Result<PortalFrame, String> {
    let length_bytes: [u8; 4] = output
        .get(..4)
        .ok_or_else(|| "renderer output omitted receipt length".to_string())?
        .try_into()
        .map_err(|_| "renderer receipt length is invalid".to_string())?;
    let receipt_len = u32::from_be_bytes(length_bytes) as usize;
    if receipt_len == 0 || receipt_len > MAX_RECEIPT_BYTES {
        return Err(format!(
            "renderer receipt length {receipt_len} outside 1..={MAX_RECEIPT_BYTES}"
        ));
    }
    let receipt_end = 4usize
        .checked_add(receipt_len)
        .ok_or_else(|| "renderer receipt length overflow".to_string())?;
    let receipt_bytes = output
        .get(4..receipt_end)
        .ok_or_else(|| "renderer output truncated its receipt".to_string())?;
    let receipt: RendererReceipt = serde_json::from_slice(receipt_bytes)
        .map_err(|error| format!("decode renderer receipt: {error}"))?;
    if receipt.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "renderer returned schema {}, expected {SCHEMA_VERSION}",
            receipt.schema_version
        ));
    }
    if receipt.sequence != expected_sequence {
        return Err(format!(
            "renderer returned sequence {}, expected {expected_sequence}",
            receipt.sequence
        ));
    }
    if let Some(error) = receipt.error {
        if receipt.frame_bytes != 0 || output.len() != receipt_end {
            return Err("renderer error receipt included frame bytes".to_string());
        }
        return Err(format!("{}: {}", error.code, error.message));
    }
    if receipt.width != FRAME_WIDTH
        || receipt.height != FRAME_HEIGHT
        || receipt.format != "rgba8-srgb"
        || receipt.frame_bytes != FRAME_BYTES
    {
        return Err(format!(
            "renderer frame contract mismatch: {}x{} {} {} bytes",
            receipt.width, receipt.height, receipt.format, receipt.frame_bytes
        ));
    }
    let expected_output = receipt_end
        .checked_add(FRAME_BYTES)
        .ok_or_else(|| "renderer frame length overflow".to_string())?;
    if output.len() != expected_output {
        return Err(format!(
            "renderer returned {} frame bytes, expected {FRAME_BYTES}",
            output.len().saturating_sub(receipt_end)
        ));
    }
    let adapter = receipt
        .adapter
        .filter(|adapter| !adapter.is_empty())
        .ok_or_else(|| "renderer success receipt omitted adapter".to_string())?;
    let _render_metadata = (adapter, receipt.production_time_us);
    Ok(PortalFrame {
        sequence: receipt.sequence,
        pixels: Arc::from(output[receipt_end..].to_vec()),
    })
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/agentviz_portal__tests.rs"]
mod tests;
