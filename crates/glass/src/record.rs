//! `ANGEL_GLASS_RECORD=path.mp4`: the window films itself, as it is drawn,
//! into a video through ffmpeg and the GPU's own encoder.
//!
//! A clock thread asks for a frame on every tick of the film's rate. The
//! window draws the frame it holds then (the same capture as a still) into
//! a texture of its own and goes on drawing while the GPU copies it back;
//! a few ticks later the pixels have arrived and go, in order, to a writer
//! thread, which feeds ffmpeg. A tick the window was too busy to answer in
//! time is not caught up late: the writer repeats the frame before it, so
//! the film keeps the wall clock's time and shows a stall as a stall. How
//! many were repeated is printed at the end.
//!
//! - `ANGEL_GLASS_RECORD_FPS`: frames a second, 1..=240 (60).
//! - `ANGEL_GLASS_RECORD_SECONDS`: how long, 1..=3600 (30).
//! - `ANGEL_GLASS_RECORD_DELAY_MS`: the wait before the first frame (2000).
//! - `ANGEL_GLASS_RECORD_CODEC`: the ffmpeg encoder (`hevc_nvenc`).

use std::collections::VecDeque;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::{Host, UserEvent};

/// What the environment asked to be recorded.
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub path: std::path::PathBuf,
    pub fps: u32,
    pub seconds: u32,
    pub delay: Duration,
    pub codec: String,
}

impl Plan {
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("ANGEL_GLASS_RECORD")?;
        let number = |key: &str, default: u64, range: std::ops::RangeInclusive<u64>| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(default)
                .clamp(*range.start(), *range.end())
        };
        Some(Self {
            path: path.into(),
            fps: number("ANGEL_GLASS_RECORD_FPS", 60, 1..=240) as u32,
            seconds: number("ANGEL_GLASS_RECORD_SECONDS", 30, 1..=3600) as u32,
            delay: Duration::from_millis(number("ANGEL_GLASS_RECORD_DELAY_MS", 2000, 0..=600_000)),
            codec: std::env::var("ANGEL_GLASS_RECORD_CODEC").unwrap_or_else(|_| "hevc_nvenc".into()),
        })
    }

    pub fn frames(&self) -> u64 {
        u64::from(self.fps) * u64::from(self.seconds)
    }

    /// The tick a moment `elapsed` into the film falls on.
    pub fn tick_at(&self, elapsed: Duration) -> u64 {
        (elapsed.as_secs_f64() * f64::from(self.fps)) as u64
    }
}

/// Start the clock: it asks the window for each frame on time, then for
/// the end.
pub(crate) fn start_clock(plan: &Plan, host: &Host) {
    let plan = plan.clone();
    let host = host.clone();
    std::thread::spawn(move || {
        std::thread::sleep(plan.delay);
        let started = Instant::now();
        host.send(UserEvent::Record(Some(started)));
        for tick in 1..plan.frames() {
            let due = Duration::from_secs_f64(tick as f64 / f64::from(plan.fps));
            if let Some(rest) = due.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
            host.send(UserEvent::Record(Some(started)));
        }
        let end = Duration::from_secs_f64(plan.frames() as f64 / f64::from(plan.fps));
        if let Some(rest) = end.checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
        host.send(UserEvent::Record(None));
    });
}

/// What the window hands the writer.
enum Piece {
    Frame(u64, Vec<u8>),
    /// The film ran its length: repeat the last frame to the end.
    End,
}

/// The window's side: the writer thread and the ticks already filmed.
pub(crate) struct Recorder {
    plan: Plan,
    frames: Option<SyncSender<Piece>>,
    writer: Option<JoinHandle<()>>,
    size: (u32, u32),
    /// The next tick not yet filmed.
    next: u64,
    /// Frames drawn but left out because the encoder was behind.
    skipped: u64,
    /// Frames on their way back from the GPU, oldest first.
    in_flight: VecDeque<(u64, angel_gpu::ReadBack)>,
}

/// Frames on their way back at once; a tick past that is left to repeat.
const IN_FLIGHT: usize = 4;

impl Recorder {
    pub fn new(plan: Plan) -> Self {
        Self { plan, frames: None, writer: None, size: (0, 0), next: 0, skipped: 0, in_flight: VecDeque::new() }
    }

    /// The tick due now, if it still wants a frame. A request that
    /// arrives after a later tick has come due is answered by that tick's,
    /// not drawn for its own. With too many frames on their way back, it
    /// waits for them and the tick repeats the one before.
    pub fn due(&mut self, started: Instant) -> Option<u64> {
        let tick = self.plan.tick_at(started.elapsed());
        if tick < self.next || tick >= self.plan.frames() {
            return None;
        }
        if self.in_flight.len() >= IN_FLIGHT {
            self.skipped += 1;
            return None;
        }
        Some(tick)
    }

    /// A frame drawn for `tick` is on its way back.
    pub fn launch(&mut self, tick: u64, read: angel_gpu::ReadBack) {
        self.in_flight.push_back((tick, read));
        self.next = tick + 1;
    }

    /// Hand the frames that have come back to the writer, in order.
    pub fn collect(&mut self) {
        while let Some((tick, read)) = self.in_flight.front() {
            let Some(pixels) = read.take() else {
                break;
            };
            let ((width, height), tick) = (read.size(), *tick);
            self.in_flight.pop_front();
            if let Some(pixels) = pixels {
                self.push(tick, width, height, pixels);
            }
        }
    }

    /// Hand over the frame drawn for `tick`. The first frame fixes the
    /// film's size; a frame of another size (the window was resized) is
    /// left out and its tick repeats the one before.
    fn push(&mut self, tick: u64, width: u32, height: u32, pixels: Vec<u8>) {
        if self.frames.is_none() && self.writer.is_none() {
            match spawn_writer(&self.plan, width, height) {
                Ok((frames, writer)) => {
                    self.frames = Some(frames);
                    self.writer = Some(writer);
                    self.size = (width, height);
                }
                Err(error) => {
                    eprintln!("GLASS: recording failed to start: {error}");
                    self.next = u64::MAX;
                    return;
                }
            }
        }
        if (width, height) != self.size {
            return;
        }
        // The window never waits on the encoder: a frame it cannot take
        // now is left out, and its tick repeats the one before.
        let Some(frames) = &self.frames else {
            return;
        };
        match frames.try_send(Piece::Frame(tick, pixels)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.skipped += 1,
            Err(TrySendError::Disconnected(_)) => self.next = u64::MAX,
        }
    }

    /// Close the film at its length: the writer repeats the last frame to
    /// the end and ffmpeg finishes the file.
    pub fn finish(&mut self) {
        if let Some(frames) = self.frames.take() {
            let _ = frames.send(Piece::End);
        }
        self.close();
    }

    /// Let the writer finish what it has. A window closed early ends its
    /// film where it was, unpadded.
    fn close(&mut self) {
        self.frames = None;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
            if self.skipped > 0 {
                eprintln!("GLASS: recording left out {} frames it was behind on", self.skipped);
            }
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.close();
    }
}

fn spawn_writer(plan: &Plan, width: u32, height: u32) -> std::io::Result<(SyncSender<Piece>, JoinHandle<()>)> {
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
        .args(["-s", &format!("{width}x{height}"), "-framerate", &plan.fps.to_string(), "-i", "-"])
        .args(["-c:v", &plan.codec])
        .args(encoder_args(&plan.codec))
        .args(["-movflags", "+faststart"])
        .arg(&plan.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;
    // A few frames of slack; the window waits on a full queue rather than
    // filming out of order.
    let (frames, received) = sync_channel(8);
    let plan = plan.clone();
    let writer = std::thread::spawn(move || write_film(&plan, &mut ffmpeg, &received));
    Ok((frames, writer))
}

/// The quality settings an encoder takes, by its family. NVENC takes the
/// RGBA as it is and converts it on the GPU (yuv420p out); p5 keeps about
/// three times a 60 fps film's pace on an RTX 3090, where p7 kept less
/// than twice it with the CPU converting.
fn encoder_args(codec: &str) -> Vec<&'static str> {
    if codec.ends_with("_nvenc") {
        vec!["-preset", "p5", "-tune", "hq", "-rc", "vbr", "-cq", "16", "-b:v", "0"]
    } else if codec == "libx264" || codec == "libx265" {
        vec!["-preset", "slow", "-crf", "16", "-pix_fmt", "yuv420p"]
    } else {
        vec!["-pix_fmt", "yuv420p"]
    }
}

fn write_film(plan: &Plan, ffmpeg: &mut Child, received: &Receiver<Piece>) {
    let Some(mut stdin) = ffmpeg.stdin.take() else {
        return;
    };
    let total = plan.frames();
    let (mut written, mut repeated, mut filmed) = (0u64, 0u64, 0u64);
    let mut last: Option<Vec<u8>> = None;
    let mut broken = false;
    let mut ended = false;
    for piece in received.iter() {
        let (tick, pixels) = match piece {
            Piece::Frame(tick, pixels) => (tick, pixels),
            Piece::End => {
                ended = true;
                break;
            }
        };
        // Ticks the window missed show the frame before them again.
        while written < tick && !broken {
            if let Some(previous) = &last {
                broken = stdin.write_all(previous).is_err();
                repeated += 1;
            }
            written += 1;
        }
        if broken {
            break;
        }
        broken = stdin.write_all(&pixels).is_err();
        written += 1;
        filmed += 1;
        last = Some(pixels);
    }
    while ended && written < total && !broken {
        if let Some(previous) = &last {
            broken = stdin.write_all(previous).is_err();
            repeated += 1;
        }
        written += 1;
    }
    drop(stdin);
    let status = ffmpeg.wait();
    eprintln!(
        "GLASS: recorded {} ({filmed} frames drawn, {repeated} repeated, {} fps, {})",
        plan.path.display(),
        plan.fps,
        match status {
            Ok(s) if s.success() && !broken => "ok".to_string(),
            Ok(s) => format!("ffmpeg {s}"),
            Err(e) => e.to_string(),
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_follow_the_clock_and_the_film_has_its_length() {
        let plan = Plan {
            path: "x.mp4".into(),
            fps: 60,
            seconds: 2,
            delay: Duration::ZERO,
            codec: "hevc_nvenc".into(),
        };
        assert_eq!(plan.frames(), 120);
        assert_eq!(plan.tick_at(Duration::ZERO), 0);
        assert_eq!(plan.tick_at(Duration::from_millis(16)), 0);
        assert_eq!(plan.tick_at(Duration::from_millis(17)), 1);
        assert_eq!(plan.tick_at(Duration::from_secs(1)), 60);
        assert!(encoder_args("h264_nvenc").contains(&"-cq"));
        assert!(encoder_args("libx264").contains(&"-crf"));
    }
}
