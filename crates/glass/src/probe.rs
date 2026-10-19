//! The latency probe: the PTY input-latency harness, adapted to the window.
//!
//! `ANGEL_GLASS_PROBE=1` waits for the composer (`Write a message`), then
//! types a draft one key at a time through the same channel the window's
//! keyboard feeds, and times each key until a presented frame shows it:
//! keypress to `present()`. Compositor and scanout come after that and are
//! the same for every window on the desktop.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui_core::buffer::Cell;

use crate::{Host, Snapshot, UserEvent};

pub struct Probe {
    ready: String,
    draft: String,
    out: Option<std::path::PathBuf>,
    close: bool,
    /// `ANGEL_GLASS_PROBE_ENTER=1`: submit the draft with Enter once it
    /// is typed, so a scripted run can drive a command, not just text.
    enter: bool,
    expect: Mutex<Option<(String, Option<Instant>)>>,
    seen: Mutex<Option<Sender<Duration>>>,
    frames: Mutex<Vec<Duration>>,
}

impl Probe {
    pub fn from_env() -> Option<Arc<Self>> {
        let on = std::env::var("ANGEL_GLASS_PROBE").is_ok_and(|v| v == "1");
        on.then(|| {
            Arc::new(Self {
                ready: std::env::var("ANGEL_GLASS_PROBE_READY")
                    .unwrap_or_else(|_| "Write a message".into()),
                draft: std::env::var("ANGEL_GLASS_PROBE_TEXT")
                    .unwrap_or_else(|_| "snappy-input-0123456789-abcdef".into()),
                out: std::env::var_os("ANGEL_GLASS_PROBE_OUT").map(Into::into),
                close: std::env::var("ANGEL_GLASS_PROBE_CLOSE").is_ok_and(|v| v == "1"),
                enter: std::env::var("ANGEL_GLASS_PROBE_ENTER").is_ok_and(|v| v == "1"),
                expect: Mutex::new(None),
                seen: Mutex::new(None),
                frames: Mutex::new(Vec::new()),
            })
        })
    }

    /// Called by the render thread after every present.
    pub fn presented(&self, snapshot: &Snapshot, render_time: Duration) {
        self.frames.lock().unwrap().push(render_time);
        let mut expect = self.expect.lock().unwrap();
        let Some((text, sent)) = expect.as_ref() else {
            return;
        };
        if !contains(snapshot, text) {
            return;
        }
        let elapsed = sent.map_or(Duration::ZERO, |t| t.elapsed());
        *expect = None;
        if let Some(seen) = self.seen.lock().unwrap().as_ref() {
            let _ = seen.send(elapsed);
        }
    }

    pub fn start(self: &Arc<Self>, host: Host, keys: Sender<Event>) {
        let probe = Arc::clone(self);
        let (tx, rx) = mpsc::channel();
        *self.seen.lock().unwrap() = Some(tx);
        let _ = std::thread::Builder::new()
            .name("glass-probe".into())
            .spawn(move || probe.run(&host, &keys, &rx));
    }

    fn wait_for(
        &self,
        text: String,
        sent: Option<Instant>,
        seen: &Receiver<Duration>,
        limit: Duration,
    ) -> Option<Duration> {
        *self.expect.lock().unwrap() = Some((text, sent));
        seen.recv_timeout(limit).ok()
    }

    fn run(&self, host: &Host, keys: &Sender<Event>, seen: &Receiver<Duration>) {
        if self
            .wait_for(self.ready.clone(), None, seen, Duration::from_secs(120))
            .is_none()
        {
            eprintln!("GLASS_PROBE: never saw {:?}", self.ready);
            return;
        }
        // Let the startup work settle, as the PTY harness does.
        std::thread::sleep(Duration::from_millis(800));
        self.frames.lock().unwrap().clear();
        let mut typed = String::new();
        let mut samples = Vec::new();
        for ch in self.draft.chars() {
            typed.push(ch);
            let press =
                KeyEvent::new_with_kind(KeyCode::Char(ch), KeyModifiers::NONE, KeyEventKind::Press);
            let release = KeyEvent::new_with_kind(
                KeyCode::Char(ch),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            );
            *self.expect.lock().unwrap() = Some((typed.clone(), Some(Instant::now())));
            if keys.send(Event::Key(press)).is_err() {
                return;
            }
            let _ = keys.send(Event::Key(release));
            match seen.recv_timeout(Duration::from_secs(2)) {
                Ok(elapsed) => samples.push(elapsed),
                Err(_) => {
                    eprintln!("GLASS_PROBE: {typed:?} never appeared");
                    break;
                }
            }
        }
        let frames = std::mem::take(&mut *self.frames.lock().unwrap());
        let report = report(&samples, &frames);
        eprintln!("{report}");
        if let Some(path) = &self.out {
            let _ = std::fs::write(path, format!("{report}\n"));
        }
        // A scripted run may submit what it typed: the same press and
        // release the window's own keyboard would send.
        if self.enter {
            let press =
                KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Press);
            let release = KeyEvent::new_with_kind(
                KeyCode::Enter,
                KeyModifiers::NONE,
                KeyEventKind::Release,
            );
            let _ = keys.send(Event::Key(press));
            let _ = keys.send(Event::Key(release));
        }
        if self.close {
            host.send(UserEvent::Close);
        }
    }
}

fn contains(snapshot: &Snapshot, text: &str) -> bool {
    let cols = usize::from(snapshot.cols);
    if cols == 0 {
        return false;
    }
    snapshot.cells.chunks(cols).any(|row| {
        let line: String = row.iter().map(Cell::symbol).collect();
        line.contains(text)
    })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    sorted[(sorted.len() - 1) * p / 100]
}

fn report(samples: &[Duration], frames: &[Duration]) -> String {
    let mut keys = samples.to_vec();
    keys.sort_unstable();
    let mut renders = frames.to_vec();
    renders.sort_unstable();
    format!(
        "GLASS_INPUT_LATENCY samples={} key_to_present_p50_ms={:.3} p95_ms={:.3} worst_ms={:.3} frames={} render_p50_ms={:.3} render_p95_ms={:.3} raw_ms={:?}",
        keys.len(),
        ms(percentile(&keys, 50)),
        ms(percentile(&keys, 95)),
        ms(keys.last().copied().unwrap_or_default()),
        renders.len(),
        ms(percentile(&renders, 50)),
        ms(percentile(&renders, 95)),
        samples
            .iter()
            .map(|d| (ms(*d) * 1000.0).round() / 1000.0)
            .collect::<Vec<_>>(),
    )
}
