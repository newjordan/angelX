//! angelface: the goddess in the window (docs/ANGELFACE.md).
//!
//! A chrome head drawn by glass itself (an `angel-gpu` scene of her rig in
//! the mesh shader's `chrome` finish), leaning out of the bottom right
//! corner of the mini-viz on the diagonal, rising out of it when called. Just her head: her regalia (rings, halo, motes) is kept in
//! `angel_face::draw` for later, off. She is a
//! presence: she breathes, blinks and looks about; she looks down at what
//! you type, sometimes, out of curiosity; she looks away to think while the
//! agent works; and she speaks lines in her own voice (Fish Audio Drama 3),
//! her face moved by NVIDIA Audio2Face from the same audio.
//!
//! She runs on her own thread, as the Orrery and the Heartglass do. The
//! cockpit tells her what happens through [`observe`], once a loop pass:
//! cheap, and nothing at all while she is away. Her model and her voice
//! come from the angelface home (`ANGEL_FACE_HOME`, `~/angelface` unless
//! set): `assets/goddess.glb`, and `voice/bank/` (`index.json`, the lines'
//! WAVs and their Audio2Face tracks).
//!
//! `ANGEL_GLASS_FACE=1` brings her in at launch (`=quiet`: without her
//! voice); `/face` calls or sends
//! her away; `/face say <occasion|line>` makes her speak; `/face mute` and
//! `/face voice` silence and free her voice.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};

/// Whether she is wanted in the window.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Whether she may speak aloud.
static VOICE: AtomicBool = AtomicBool::new(true);

/// What the cockpit tells her thread.
#[derive(Clone, Debug)]
enum Msg {
    Cue(CueKind),
    /// Speak a line: an occasion (`arrive`, `done`, ...) or a line's id.
    Say(String),
    /// Her spectacle now: burning eyes, lasers, a rainbow.
    Spectacle,
    /// Her tour of the harness: begin it (true) or end it.
    Tour(bool),
}

/// The cockpit's cues, as her thread takes them (the crate's own `Cue`
/// lives behind the window's feature).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CueKind {
    Typing,
    Submitted,
    Working,
    Finished { ok: bool },
}

static SENDER: OnceLock<Mutex<mpsc::Sender<Msg>>> = OnceLock::new();

/// A part of the cockpit she knows the place of: she sits over the world
/// (the mini-viz), and her tour points at all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Region {
    World,
    Composer,
    Transcript,
    Header,
}

/// Where each region was last drawn (cells) and when.
static REGIONS: Mutex<[Option<(ratatui::layout::Rect, std::time::Instant)>; 4]> = Mutex::new([None; 4]);

/// `region` drew at `area` this frame.
pub(crate) fn note_region(region: Region, area: ratatui::layout::Rect) {
    if !visible() {
        return;
    }
    if let Ok(mut r) = REGIONS.lock() {
        r[region as usize] = Some((area, std::time::Instant::now()));
    }
}

/// The mini-viz drew at `area` this frame.
pub(crate) fn note_panel(area: ratatui::layout::Rect) {
    note_region(Region::World, area);
}

/// Where `region` is, if it drew in the last second.
fn region(region: Region) -> Option<ratatui::layout::Rect> {
    let r = REGIONS.lock().ok()?;
    r[region as usize].and_then(|(area, at)| (at.elapsed() < std::time::Duration::from_secs(1)).then_some(area))
}

/// The mini-viz's place, if it drew in the last second.
fn panel() -> Option<ratatui::layout::Rect> {
    region(Region::World)
}

/// What she says on her tour, for the transcript (taken by `observe`).
static CAPTIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn caption(text: String) {
    if let Ok(mut c) = CAPTIONS.lock() {
        c.push(text);
    }
}

/// `/onboard`: her tour of the harness, calling her in if she is away;
/// `/onboard stop` ends it.
pub(crate) fn tour(start: bool) -> Result<(), String> {
    if !crate::ui::glass::active() {
        return Err("she lives in angelX's window (ANGEL_GLASS=1); a terminal has no room for her tour".into());
    }
    if start {
        VISIBLE.store(true, Ordering::Relaxed);
        launch();
    }
    send(Msg::Tour(start));
    Ok(())
}

/// Whether a message typed to the agent is a request for her tour: short,
/// and asking to be onboarded. Teaching is the agent's ("teach me ..." is
/// a question for it, never her tour).
pub(crate) fn onboarding_request(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    t.chars().count() <= 80
        && !t.starts_with('/')
        && t.split(|c: char| !c.is_alphanumeric()).any(|w| matches!(w, "onboard" | "onboarding" | "onboarded"))
}

/// Her thread's last word on herself, for `/face status`.
static STATUS: Mutex<String> = Mutex::new(String::new());

/// `/face status`: what her thread last said of itself.
pub(crate) fn status() -> String {
    let said = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
    let voice = if VOICE.load(Ordering::Relaxed) { "voice on" } else { "muted" };
    let wanted = if visible() { "called" } else { "away" };
    if said.is_empty() {
        format!("{wanted} · {voice} · her thread has not started (home {})", home().display())
    } else {
        format!("{wanted} · {voice} · {said} · log {}", log_path().display())
    }
}

/// Where her thread writes what happens to her, for debugging.
pub(crate) fn log_path() -> PathBuf {
    home().join("logs/face.log")
}

/// What `/face` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Toggled {
    Shown,
    Hidden,
    /// Not in the window: nothing to float her over.
    NoWindow,
}

/// Where her model and voice live.
pub(crate) fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("ANGEL_FACE_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("angelface")
}

/// Whether she is wanted now.
pub(crate) fn visible() -> bool {
    VISIBLE.load(Ordering::Relaxed)
}

/// Bring her in if this launch asked for her (`ANGEL_GLASS_FACE=1`, or
/// `=quiet` for her without her voice until `/face voice`). No-op in a
/// terminal; twice-run safe.
pub(crate) fn launch_if_requested() {
    let wanted = std::env::var("ANGEL_GLASS_FACE").unwrap_or_default();
    if matches!(wanted.as_str(), "1" | "on" | "quiet") && crate::ui::glass::active() {
        if wanted == "quiet" {
            VOICE.store(false, Ordering::Relaxed);
        }
        VISIBLE.store(true, Ordering::Relaxed);
        launch();
    }
}

/// `/face`: call her in, or send her away.
pub(crate) fn toggle() -> Toggled {
    if !crate::ui::glass::active() {
        return Toggled::NoWindow;
    }
    let shown = !VISIBLE.fetch_xor(true, Ordering::Relaxed);
    if shown {
        launch();
        Toggled::Shown
    } else {
        Toggled::Hidden
    }
}

/// `/face mute` (false) and `/face voice` (true).
pub(crate) fn set_voice(on: bool) {
    VOICE.store(on, Ordering::Relaxed);
}

/// `/face say <occasion|line>`: what she will say, or why she cannot.
pub(crate) fn say(what: &str) -> Result<String, String> {
    if !crate::ui::glass::active() {
        return Err("she lives in angelX's window (ANGEL_GLASS=1); a terminal has no room for her".into());
    }
    if !visible() {
        return Err("she is away (/face calls her)".into());
    }
    if !VOICE.load(Ordering::Relaxed) {
        return Err("her voice is muted (/face voice frees it)".into());
    }
    let lines = bank_lines();
    let what = what.trim();
    let known = lines.iter().any(|(id, occasion, _)| id == what || occasion == what);
    if !known {
        let occasions: std::collections::BTreeSet<&str> = lines.iter().map(|(_, o, _)| o.as_str()).collect();
        return Err(format!("no line for {what:?}; occasions: {}", occasions.into_iter().collect::<Vec<_>>().join(", ")));
    }
    send(Msg::Say(what.to_string()));
    Ok(what.to_string())
}

/// `/face lasers`: her spectacle now, if she is here.
pub(crate) fn spectacle() -> Result<(), String> {
    if !crate::ui::glass::active() {
        return Err("she lives in angelX's window (ANGEL_GLASS=1)".into());
    }
    if !visible() {
        return Err("she is away (/face calls her)".into());
    }
    send(Msg::Spectacle);
    Ok(())
}

/// Her lines as `(id, occasion, text)`, from the bank's index.
pub(crate) fn bank_lines() -> Vec<(String, String, String)> {
    let path = home().join("voice/bank/index.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };
    v["lines"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|l| {
                    let s = |k: &str| l[k].as_str().unwrap_or("").to_string();
                    (s("id"), s("occasion"), s("text"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn send(msg: Msg) {
    if let Some(tx) = SENDER.get()
        && let Ok(tx) = tx.lock()
    {
        let _ = tx.send(msg);
    }
}

/// What the cockpit looked like at the last pass, to tell what changed.
#[derive(Default)]
struct Watch {
    input: (usize, u64),
    busy: bool,
}

static WATCH: Mutex<Option<Watch>> = Mutex::new(None);

/// Once a loop pass: what she should know of the cockpit. A key into the
/// composer, a turn begun, the agent at work, a turn ended well or badly.
/// Nothing while she is away.
pub(crate) fn observe(app: &mut crate::app::App) {
    if !visible() {
        return;
    }
    let said: Vec<String> = CAPTIONS.lock().map(|mut c| std::mem::take(&mut *c)).unwrap_or_default();
    for text in said {
        app.system_msg(format!("angelface · {text}"));
    }
    let input = (app.input.len(), quick_hash(app.input.as_bytes()));
    let busy = app.thinking.is_some() || app.pending_turn.is_some();
    let Ok(mut watch) = WATCH.lock() else { return };
    let Some(last) = watch.as_mut() else {
        *watch = Some(Watch { input, busy });
        return;
    };
    if input != last.input {
        // A sent composer empties: that is the turn, not typing.
        if input.0 == 0 && last.input.0 > 0 && busy {
            send(Msg::Cue(CueKind::Submitted));
        } else if input.0 > 0 {
            send(Msg::Cue(CueKind::Typing));
        }
    }
    if busy {
        send(Msg::Cue(CueKind::Working));
    } else if last.busy {
        send(Msg::Cue(CueKind::Finished { ok: app.portrait_fail_run == 0 }));
    }
    last.input = input;
    last.busy = busy;
}

fn quick_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes.iter().rev().take(64) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Start her thread, once.
fn launch() {
    #[cfg(feature = "glass")]
    {
        static LAUNCHED: OnceLock<()> = OnceLock::new();
        if let Some(host) = crate::ui::glass::host()
            && LAUNCHED.set(()).is_ok()
        {
            let (tx, rx) = mpsc::channel();
            let _ = SENDER.set(Mutex::new(tx));
            let host = host.clone();
            std::thread::Builder::new().name("angel-face".into()).spawn(move || presence::run(host, rx)).ok();
        }
    }
}

#[cfg(feature = "glass")]
mod presence {
    //! Her thread: her mind, her voice and her drawing, thirty times a
    //! second while she is here.

    use super::{CueKind, Msg, Region, STATUS, VISIBLE, VOICE, caption, home, log_path, panel, region};
    use angel_face::draw::LIMB_PICTURE;
    use angel_face::limb::LimbKind;
    use angel_glass::Placement;
    use std::io::Write as _;
    use crate::agent::service_process::ServiceChild;
    use angel_face::draw::{self, GROUP, SCENE_PICTURE};
    use angel_face::{Cue, Drawing, Goddess, Mind, Track};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// Her picture's z: over the terminal's text.
    const Z: i32 = 60;
    /// How tall she stands, as a share of the window's height.
    const HEIGHT_SHARE: f32 = 0.21;
    /// How much she grows as she speaks.
    const FORWARD_GROW: f32 = 0.14;
    /// Her frame rate.
    const FRAME: Duration = Duration::from_millis(33);
    /// The time between pressing play and the first sound, assumed.
    const AUDIO_LATENCY: Duration = Duration::from_millis(70);

    /// A line she can say.
    struct Line {
        id: String,
        occasion: String,
        wav: PathBuf,
        track: Arc<Track>,
    }

    /// A line being said.
    struct Speaking {
        track: Arc<Track>,
        started: Instant,
        _player: Option<ServiceChild>,
    }

    /// Her log: a line a happening, stamped with seconds on her clock.
    struct Log(Option<std::fs::File>);

    impl Log {
        fn open() -> Self {
            let path = log_path();
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
            Self(std::fs::OpenOptions::new().create(true).append(true).open(&path).ok())
        }

        fn line(&mut self, now: f64, text: &str) {
            if let Some(f) = self.0.as_mut() {
                let _ = writeln!(f, "{now:9.2} {text}");
            }
        }
    }

    fn set_status(text: String) {
        if let Ok(mut s) = STATUS.lock() {
            *s = text;
        }
    }

    /// What a step of her tour points at.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Aim {
        Her,
        Composer,
        Transcript,
        Header,
        World,
        Middle,
    }

    /// Her tour of the harness: each step a line of hers (the voice bank's
    /// `tour_*`, its words kept here too, for the transcript and for a
    /// tour without her voice) and what she points at while she says it.
    const TOUR: [(&str, Aim, &str); 11] = [
        ("tour_01_welcome", Aim::Her, "Hello. I have been folded into your terminal. Let me show you around, before the cube notices."),
        ("tour_02_composer", Aim::Composer, "This little box is where you speak to me. Type your wish, and press Enter. I will pretend I was not already listening."),
        ("tour_03_transcript", Aim::Transcript, "Up here, my answers appear, with every tool I touched and what it cost. Receipts. I am very tidy, for a goddess."),
        ("tour_04_model", Aim::Header, "Slash model chooses the mind that works for you. Slash think tells it how hard to think. Gently, please."),
        ("tour_05_approvals", Aim::Transcript, "Before I run a command or touch a file, I ask. Y means yes. A means yes to all of it, this turn. N means no, and I will sulk, beautifully."),
        ("tour_06_diff", Aim::Transcript, "Slash diff shows everything I changed. Nothing hidden. Not even the rainbow."),
        ("tour_07_goals", Aim::Composer, "For long work, set a goal with slash goal. Then slash loop, and I keep going until it is done, or the sun goes out."),
        ("tour_08_world", Aim::World, "This is the world. While I think, my work becomes an adventure down here. The knight does not know he is a metaphor."),
        ("tour_09_showcase", Aim::Middle, "Pictures, films and charts that I show you open over everything, on glass. Like me."),
        ("tour_10_help", Aim::Composer, "Slash help lists every command. Tab finishes them, so your fingers can rest."),
        ("tour_11_farewell", Aim::Her, "That is enough to begin. Ask me anything. I will be right here, in the corner, with my cube."),
    ];

    /// Where her tour is: the next step and when it may begin, and how many
    /// legs have pointed so far.
    struct Tour {
        step: usize,
        next_at: f64,
        legs: u32,
    }

    /// A limb pointing: what, from where it bursts in to what it points at
    /// (the grid's pixels), when it came and when it goes, how long it is
    /// stretched (meters), its picture's size, and pixels a meter.
    struct Gesture {
        kind: LimbKind,
        entry: [f32; 2],
        target: [f32; 2],
        began: f64,
        ends: f64,
        length: f32,
        size: (u32, u32),
        ppm: f32,
    }

    /// How long a limb takes to burst in, and to withdraw.
    const EXTEND: f64 = 0.55;
    const RETRACT: f64 = 0.4;
    /// How far in from its picture's end the tip sits, pixels.
    const TIP_ROOM: f32 = 10.0;

    impl Gesture {
        /// Its layer at `now`, placed in cells, or `None` once it has gone.
        fn layer(&self, now: f64, cell: (f32, f32)) -> Option<angel_glass::Layer> {
            if now >= self.ends {
                return None;
            }
            let w = self.size.0 as f32;
            let s = (now - self.began).max(0.0);
            let out = if s < EXTEND {
                rise((s / EXTEND) as f32)
            } else if now > self.ends - RETRACT {
                let k = (((self.ends - now) / RETRACT) as f32).clamp(0.0, 1.0);
                k * k
            } else {
                1.0
            };
            // Held: it taps at what it points at.
            let tap = if s >= EXTEND && now <= self.ends - RETRACT { 7.0 * ((s - EXTEND) as f32 * 9.0).sin().max(0.0) } else { 0.0 };
            let along = -(1.0 - out) * w * 1.02 + tap;
            let (r, angle) = draw::limb_rect(self.entry, self.target, self.size, TIP_ROOM, along);
            let mut layer = angel_glass::Layer::new(
                10,
                Z + 1,
                Placement::Cells([r[0] / cell.0, r[1] / cell.1, r[2] / cell.0, r[3] / cell.1]),
                angel_glass::LayerKind::Picture(LIMB_PICTURE),
            );
            layer.angle = angle;
            Some(layer)
        }
    }

    /// Where a limb bursts in from to point at `target`: a point past an
    /// edge of a `w` x `h` window (a leg likes to come down from the top),
    /// far enough for a long reach and not absurdly far.
    fn entry_for(target: [f32; 2], w: f32, h: f32, leg: bool, rng: &mut u64) -> [f32; 2] {
        let past = 80.0;
        for _ in 0..24 {
            let edge = if leg && uniform(rng) < 0.5 { 0 } else { (uniform(rng) * 4.0) as u32 };
            let k = uniform(rng);
            let p = match edge {
                0 => [w * k, -past],
                1 => [w + past, h * k],
                2 => [w * k, h + past],
                _ => [-past, h * k],
            };
            let d = ((p[0] - target[0]).powi(2) + (p[1] - target[1]).powi(2)).sqrt();
            if (360.0..1.4 * w.max(h)).contains(&d) {
                return p;
            }
        }
        [-past, if target[1] < h / 2.0 { h * 0.85 } else { h * 0.15 }]
    }

    pub(super) fn run(host: angel_glass::Host, rx: mpsc::Receiver<Msg>) {
        let mut log = Log::open();
        log.line(0.0, &format!("---- angelface starts (pid {})", std::process::id()));
        set_status("starting".into());
        if !host.has_window() {
            // A session host with no window of its own forwards what is
            // drawn over its link; her meshes change every frame she
            // speaks, so she stays out of those until the link carries
            // her as a model and weights.
            eprintln!("angelface: no window here; she appears only in a window of her own");
            log.line(0.0, "no window here: she stays out");
            set_status("no window here".into());
            return;
        }
        let home = home();
        let goddess = match std::fs::read(home.join("assets/goddess.glb")).map_err(|e| e.to_string()).and_then(|b| Goddess::from_glb(&b)) {
            Ok(g) => Arc::new(g),
            Err(why) => {
                let text = format!("no goddess at {}: {why}", home.join("assets/goddess.glb").display());
                eprintln!("angelface: {text}");
                log.line(0.0, &text);
                set_status(text);
                return;
            }
        };
        let lines = load_lines(&home.join("voice/bank"), &goddess).unwrap_or_else(|why| {
            eprintln!("angelface: no voice: {why}");
            log.line(0.0, &format!("no voice: {why}"));
            Vec::new()
        });
        log.line(0.0, &format!("her: {} parts, {} channels; {} lines in her voice", goddess.parts.len(), goddess.channels.len(), lines.len()));
        // Frame accounting for the status and the log.
        let mut frames = 0u32;
        let mut busiest = Duration::ZERO;
        let mut spent = Duration::ZERO;
        let mut sent_bytes = 0usize;
        let mut window_began = Instant::now();
        let mut last_size = (0u32, 0u32);
        let mut drawing = Drawing::new(goddess.clone());
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
        let mut mind = Mind::new(&goddess, drawing.viewer(), seed);
        let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
        let clock = Instant::now();
        let mut speaking: Option<Speaking> = None;
        let mut queued: Option<(f64, String)> = None;
        let mut spoke_at: f64 = f64::NEG_INFINITY;
        let mut said_typing_at: f64 = f64::NEG_INFINITY;
        let mut said_think_at: f64 = f64::NEG_INFINITY;
        let mut last_cue_at: f64 = 0.0;
        let mut was_working = false;
        let mut greeted = false;
        let mut arrived_at = f64::INFINITY;
        // Her spectacle: when the next is due, and where it strikes.
        let mut next_spectacle = f64::INFINITY;
        let mut strike: [f32; 2] = [0.0; 2];
        let mut pictures_sent = false;
        let mut shown = false;
        let mut forward = 0.0f32;
        let mut pointer: Option<[f32; 2]> = None;
        let mut glanced_at: f64 = f64::NEG_INFINITY;
        let mut tour: Option<Tour> = None;
        let mut gesture: Option<Gesture> = None;
        let mut limb_shown = false;
        loop {
            let frame_began = Instant::now();
            let now = clock.elapsed().as_secs_f64();
            let wanted = VISIBLE.load(Ordering::Relaxed);
            if wanted && !mind.here() {
                mind.cue(Cue::Arrive, now);
                greeted = false;
                arrived_at = now;
                next_spectacle = now + 45.0 + 60.0 * uniform(&mut rng) as f64;
                log.line(now, "arrive");
            }
            if !wanted && mind.here() {
                log.line(now, "depart");
                mind.cue(Cue::Depart, now);
                speaking = None;
                queued = None;
                tour = None;
                gesture = None;
            }
            if !wanted && mind.gone(now) {
                if shown {
                    host.set_layer_group(GROUP, Vec::new());
                    host.remove_scene(SCENE_PICTURE);
                    host.remove_scene(LIMB_PICTURE);
                    limb_shown = false;
                    shown = false;
                    log.line(now, "gone");
                    set_status("away".into());
                }
                match rx.recv_timeout(Duration::from_millis(150)) {
                    Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            if !pictures_sent {
                for (id, width, height, rgba) in drawing.pictures() {
                    host.upload_picture(angel_glass::graphics::Picture { id, width, height, rgba, cols: 0, rows: 0 });
                }
                pictures_sent = true;
            }

            // What happened in the cockpit.
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Say(what) => {
                        log.line(now, &format!("asked to say {what}"));
                        queued = Some((now, what));
                    }
                    Msg::Spectacle => next_spectacle = now,
                    Msg::Tour(true) => {
                        log.line(now, "tour: she will show them around");
                        greeted = true;
                        queued = None;
                        next_spectacle = next_spectacle.max(now + 150.0);
                        tour = Some(Tour { step: 0, next_at: now.max(arrived_at + angel_face::mind::ARRIVAL + 0.4), legs: 0 });
                    }
                    Msg::Tour(false) => {
                        if tour.take().is_some() {
                            log.line(now, "tour: ended early");
                            caption("(the tour ends)".into());
                        }
                        if let Some(g) = gesture.as_mut() {
                            g.ends = g.ends.min(now + RETRACT);
                        }
                    }
                    Msg::Cue(cue) => {
                        last_cue_at = now;
                        match cue {
                            CueKind::Typing => {
                                let was_reading = mind.reading();
                                mind.cue(Cue::Typing, now);
                                if !was_reading && mind.reading() {
                                    log.line(now, "typing: she looks down to read");
                                }
                                if !was_reading && mind.reading() && now - said_typing_at > 120.0 && chance(&mut rng, 0.10) {
                                    said_typing_at = now;
                                    queued.get_or_insert((now + 0.9, "typing".into()));
                                }
                            }
                            CueKind::Submitted => {
                                log.line(now, "submitted");
                                mind.cue(Cue::Submitted, now);
                            }
                            CueKind::Working => {
                                if !was_working {
                                    log.line(now, "the agent is working");
                                }
                                mind.cue(Cue::Working, now);
                                if !was_working && now - said_think_at > 90.0 && chance(&mut rng, 0.12) {
                                    said_think_at = now;
                                    queued.get_or_insert((now + 1.2, "think".into()));
                                }
                                was_working = true;
                            }
                            CueKind::Finished { ok } => {
                                log.line(now, &format!("turn finished ok={ok}"));
                                mind.cue(Cue::Finished { ok }, now);
                                was_working = false;
                                let (occasion, odds) = if ok { ("done", 0.45) } else { ("error", 0.7) };
                                if now - spoke_at > 20.0 && chance(&mut rng, odds) {
                                    queued = Some((now + 0.5, occasion.into()));
                                }
                            }
                        }
                    }
                }
            }
            // She greets you once she has formed; she muses if all is still.
            if !greeted && tour.is_none() && mind.here() && now > arrived_at + angel_face::mind::ARRIVAL + 0.4 {
                greeted = true;
                queued.get_or_insert((now, "arrive".into()));
            }
            if tour.is_none() && now - last_cue_at > 240.0 && now - spoke_at > 240.0 && speaking.is_none() {
                last_cue_at = now;
                if chance(&mut rng, 0.35) {
                    queued = Some((now, "idle".into()));
                }
            }
            if let Some((at, what)) = queued.clone()
                && now >= at
                && speaking.is_none()
                && !mind.in_spectacle(now)
            {
                queued = None;
                if !VOICE.load(Ordering::Relaxed) {
                    log.line(now, &format!("would say {what}, but muted"));
                } else if let Some(line) = pick(&lines, &what, &mut rng) {
                    let player = play(&line.wav);
                    log.line(now, &format!("says {} ({:.1}s){}", line.id, line.track.duration, if player.is_none() { ": no player (pw-play, ffplay) started" } else { "" }));
                    speaking = Some(Speaking { track: line.track.clone(), started: Instant::now() + AUDIO_LATENCY, _player: player });
                    spoke_at = now;
                } else {
                    log.line(now, &format!("no line for {what}"));
                }
            }

            // Her spectacle, when it is due and she is free for it.
            if now >= next_spectacle && tour.is_none() && mind.here() && !mind.in_spectacle(now) {
                if speaking.is_some() || mind.reading() || queued.is_some() {
                    next_spectacle = now + 12.0;
                } else {
                    let grid = host.grid_size();
                    let (w, h) = ((grid.cols as f32) * grid.cell_width as f32, (grid.rows as f32) * grid.cell_height as f32);
                    let roll = uniform(&mut rng);
                    strike = match host.pointer() {
                        Some(p) if roll < 0.3 => p,
                        _ if roll < 0.5 => [w * 0.42, h - grid.cell_height as f32 * 2.5],
                        _ => [w * (0.06 + 0.56 * uniform(&mut rng)), h * (0.08 + 0.64 * uniform(&mut rng))],
                    };
                    mind.cue(Cue::Spectacle, now);
                    log.line(now, &format!("spectacle: her eyes burn; she strikes at {:.0},{:.0}", strike[0], strike[1]));
                    next_spectacle = now + 90.0 + 150.0 * uniform(&mut rng) as f64;
                }
            }

            // Her speech, her mind.
            let channels = goddess.channels.len();
            let speech = speaking.as_ref().and_then(|s| {
                let t = Instant::now().saturating_duration_since(s.started).as_secs_f32();
                let early = Instant::now() < s.started;
                if early { Some((vec![0.0; channels], 0.0)) } else { s.track.sample(t, channels) }
            });
            if speech.is_none() {
                speaking = None;
            }
            let grid = host.grid_size();
            let moment = mind.moment(now, speech.as_ref().map(|(w, v)| (w.as_slice(), *v)));
            let k = 1.0 - (-(FRAME.as_secs_f32()) * if speech.is_some() { 1.6 } else { 0.8 }).exp();
            forward += ((speech.is_some() as u8 as f32) - forward) * k;

            // Where she stands, and where your words are from her eyes.
            let Some((cells, angle, size)) = placement(grid, panel(), forward, moment.presence, now) else {
                std::thread::sleep(FRAME);
                continue;
            };
            // In window pixels, near enough for where she looks (the grid's
            // own small margin aside): her eyes sit a little above her
            // picture's middle, turned with it.
            let (cw, ch) = (grid.cell_width as f32, grid.cell_height as f32);
            let rect = [cells[0] * cw, cells[1] * ch, cells[2] * cw, cells[3] * ch];
            let (sin, cos) = angle.sin_cos();
            let up = rect[3] * 0.05;
            let eyes = [rect[0] + rect[2] * 0.5 + sin * up, rect[1] + rect[3] * 0.5 - cos * up];
            let reach = rect[3] * 1.6;
            // A point on screen as yaw and pitch in her own frame: turned
            // back by her lean, since her face is leaning with the picture.
            let toward = |p: [f32; 2]| {
                let (dx, dy) = (p[0] - eyes[0], p[1] - eyes[1]);
                let (x, y) = (dx * cos + dy * sin, -dx * sin + dy * cos);
                ((x / reach).atan(), (y / reach).atan())
            };
            let composer = [grid.width_px as f32 * 0.42, grid.height_px as f32 - grid.cell_height as f32 * 2.5];
            let (yaw, pitch) = toward(composer);
            mind.set_input(yaw, pitch.clamp(-0.3, 0.65));
            if let Some(p) = host.pointer() {
                let moved = pointer.is_none_or(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 60.0);
                if moved && now - glanced_at > 5.0 && chance(&mut rng, 0.3) {
                    let (yaw, pitch) = toward(p);
                    if mind.glance(yaw, pitch, now, 1.0 + 1.5 * uniform(&mut rng) as f64) {
                        glanced_at = now;
                    }
                }
                pointer = Some(p);
            }

            // Her tour: a step when the last has been said.
            let (gw, gh) = (grid.cols as f32 * cw, grid.rows as f32 * ch);
            if let Some(t) = tour.as_mut()
                && now >= t.next_at
                && speaking.is_none()
                && !mind.in_spectacle(now)
                && mind.here()
            {
                if t.step >= TOUR.len() {
                    log.line(now, "tour: done");
                    tour = None;
                } else {
                    let (id, aim, words) = TOUR[t.step];
                    let line = lines.iter().find(|l| l.id == id);
                    let words = words.to_string();
                    let seconds = line.map_or(2.0 + words.len() as f64 * 0.06, |l| l.track.duration as f64);
                    caption(words);
                    if let Some(line) = line
                        && VOICE.load(Ordering::Relaxed)
                    {
                        let player = play(&line.wav);
                        speaking = Some(Speaking { track: line.track.clone(), started: Instant::now() + AUDIO_LATENCY, _player: player });
                        spoke_at = now;
                    }
                    // What she points at, in the grid's pixels.
                    let at_region = |r: Region, fx: f32, fy: f32| {
                        region(r).map(|a| [(f32::from(a.x) + f32::from(a.width) * fx) * cw, (f32::from(a.y) + f32::from(a.height) * fy) * ch])
                    };
                    let target = match aim {
                        Aim::Her => eyes,
                        Aim::Composer => at_region(Region::Composer, 0.3, 0.5).unwrap_or(composer),
                        Aim::Transcript => at_region(Region::Transcript, 0.45, 0.42).unwrap_or([gw * 0.35, gh * 0.4]),
                        Aim::Header => at_region(Region::Header, 0.12, 0.5).unwrap_or([gw * 0.1, ch]),
                        Aim::World => at_region(Region::World, 0.5, 0.45).unwrap_or([rect[0], rect[1]]),
                        Aim::Middle => [gw * 0.5, gh * 0.45],
                    };
                    let leg = (t.legs == 0 && t.step == 5) || chance(&mut rng, 0.25);
                    let kind = if leg { LimbKind::Leg } else { LimbKind::Arm };
                    let entry = entry_for(target, gw, gh, leg, &mut rng);
                    let ppm = rect[3] / drawing.meters_across();
                    let reach = ((target[0] - entry[0]).powi(2) + (target[1] - entry[1]).powi(2)).sqrt();
                    let radius = drawing.limb_radius(kind).unwrap_or(0.1);
                    let size = ((reach + TIP_ROOM + 40.0).round() as u32, (radius * 2.3 * ppm).round().max(8.0) as u32);
                    let length = (reach + TIP_ROOM) / ppm;
                    if let Some((mesh_id, mesh)) = drawing.limb_mesh(kind, length) {
                        host.upload_mesh(mesh_id, mesh);
                        gesture = Some(Gesture { kind, entry, target, began: now + 0.1, ends: now + seconds + 0.4, length, size, ppm });
                    }
                    if leg {
                        t.legs += 1;
                    }
                    log.line(now, &format!("tour {}/{}: {id}, a {kind:?} from {:.0},{:.0} to {:.0},{:.0} ({:.0} px)", t.step + 1, TOUR.len(), entry[0], entry[1], target[0], target[1], reach));
                    t.next_at = now + seconds + 1.1;
                    t.step += 1;
                }
            }

            // Draw her.
            for (id, mesh) in drawing.meshes(&moment) {
                sent_bytes += mesh.positions.len() * 48 + mesh.indices.len() * 4;
                host.upload_mesh(id, mesh);
            }
            let scene = drawing.scene(&moment, now);
            // Her spectacle's layers, worked in the grid's pixels (cells
            // times the cell) and placed in cells, so they land on her
            // picture exactly as glass lays it.
            let mut layers = draw::layers(Placement::Cells(cells), Z, &moment, drawing.regalia, angle);
            if moment.spectacle.is_some()
                && let Some(a) = drawing.anchors(&scene, size)
            {
                let side = rect[2];
                let centre = [rect[0] + side / 2.0, rect[1] + rect[3] / 2.0];
                let on_window = |p: [f32; 2]| {
                    let (x, y) = (p[0] / size.0 as f32 * side - side / 2.0, p[1] / size.1 as f32 * rect[3] - rect[3] / 2.0);
                    [centre[0] + x * cos - y * sin, centre[1] + x * sin + y * cos]
                };
                let middle = [grid.cols as f32 * cw / 2.0, grid.rows as f32 * ch / 2.0];
                let effects = draw::spectacle_layers(
                    &moment,
                    [on_window(a.eyes[0]), on_window(a.eyes[1])],
                    on_window(a.mouth),
                    draw::pour_toward(centre, middle),
                    strike,
                    side / 400.0,
                    now,
                    Z + 2,
                );
                layers.extend(effects.into_iter().map(|mut l| {
                    if let Placement::Pixels(r) = l.placement {
                        l.placement = Placement::Cells([r[0] / cw, r[1] / ch, r[2] / cw, r[3] / ch]);
                    }
                    l
                }));
            }
            if size != last_size {
                log.line(
                    now,
                    &format!(
                        "her picture: {}x{} px at cells {:.1},{:.1} ({:.1}x{:.1}), leaning {:.0} degrees, in a {}x{} window, {}",
                        size.0,
                        size.1,
                        cells[0],
                        cells[1],
                        cells[2],
                        cells[3],
                        angle.to_degrees(),
                        grid.width_px,
                        grid.height_px,
                        if panel().is_some() { "over the mini-viz" } else { "no mini-viz: bottom right" }
                    ),
                );
                last_size = size;
            }
            // A limb pointing, while one is.
            match gesture.as_ref().and_then(|g| g.layer(now, (cw, ch)).map(|l| (g, l))) {
                Some((g, layer)) => {
                    if let Some(limb) = drawing.limb_scene(g.kind, g.length, g.size, g.ppm, TIP_ROOM, moment.liquid.max(0.05)) {
                        host.set_scene(LIMB_PICTURE, limb, g.size);
                        layers.push(layer);
                        limb_shown = true;
                    }
                }
                None => {
                    gesture = None;
                    if limb_shown {
                        host.remove_scene(LIMB_PICTURE);
                        limb_shown = false;
                    }
                }
            }
            host.set_scene(SCENE_PICTURE, scene, size);
            host.set_layer_group(GROUP, layers);
            shown = true;
            let took = frame_began.elapsed();
            frames += 1;
            spent += took;
            busiest = busiest.max(took);
            if window_began.elapsed() >= Duration::from_secs(10) {
                let secs = window_began.elapsed().as_secs_f64();
                let text = format!(
                    "{:.1} fps · her frame {:.2} ms avg, {:.2} max · meshes {:.1} MB/s · {}x{} · presence {:.2}{}{}",
                    frames as f64 / secs,
                    spent.as_secs_f64() * 1000.0 / frames.max(1) as f64,
                    busiest.as_secs_f64() * 1000.0,
                    sent_bytes as f64 / secs / 1e6,
                    size.0,
                    size.1,
                    moment.presence,
                    if mind.reading() { " · reading" } else { "" },
                    if speaking.is_some() { " · speaking" } else { "" },
                );
                log.line(now, &text);
                set_status(text);
                frames = 0;
                spent = Duration::ZERO;
                busiest = Duration::ZERO;
                sent_bytes = 0;
                window_began = Instant::now();
            }
            if let Some(rest) = FRAME.checked_sub(took) {
                std::thread::sleep(rest);
            }
        }
    }

    /// How far her picture is turned in her corner, radians (clockwise on
    /// screen): negative leans her face up and out of the corner, her neck
    /// into it.
    const LEAN: f32 = -0.75;

    /// Her place in a window of `grid`, in cells, and her picture's turn:
    /// about a fifth of its height, her neck in the bottom right corner of
    /// the mini-viz panel (of the window, above the composer, when the
    /// mini-viz is not drawn), leaning out of it on the diagonal. She rises
    /// out of the corner along that diagonal with her presence, a little
    /// past her place and back (`presence` 0 is wholly beyond the corner),
    /// grows a little as she speaks (`forward`), and her lean drifts on her
    /// clock `now`. Returns the cells, the turn and the size her picture is
    /// drawn at (her largest, so it is only ever shown smaller).
    fn placement(
        grid: angel_glass::GridSize,
        panel: Option<ratatui::layout::Rect>,
        forward: f32,
        presence: f32,
        now: f64,
    ) -> Option<([f32; 4], f32, (u32, u32))> {
        let (w, h) = (grid.width_px as f32, grid.height_px as f32);
        let (cw, ch) = (grid.cell_width.max(1) as f32, grid.cell_height.max(1) as f32);
        if w < 200.0 || h < 200.0 {
            return None;
        }
        let side_px = (h * HEIGHT_SHARE).clamp(130.0, 560.0);
        let largest_px = (side_px * (1.0 + FORWARD_GROW)).round() as u32;
        let side = side_px * (1.0 + FORWARD_GROW * forward);
        // The corner, in pixels.
        let corner = match panel {
            Some(p) => [f32::from(p.x + p.width) * cw, f32::from(p.y + p.height) * ch],
            None => [w - 2.0 * cw, h - 4.0 * ch],
        };
        // Her lean drifts, slow and uneasy.
        let t = now as f32;
        let angle = LEAN + 0.045 * (t * 0.23).sin() + 0.02 * (t * 0.71 + 1.7).sin();
        // Where her neck's end lands from her picture's middle once turned:
        // that point sits in the corner.
        let (s, c) = angle.sin_cos();
        let neck = [-s * side * 0.5, c * side * 0.5];
        // Rising out of the corner: from a picture's length beyond it, a
        // little past her place, then home.
        let k = rise(presence);
        let out = 1.0 - k;
        let centre = [corner[0] - neck[0] * 0.9 + neck[0] * 2.2 * out, corner[1] - neck[1] * 0.9 + neck[1] * 2.2 * out];
        let cells = [(centre[0] - side / 2.0) / cw, (centre[1] - side / 2.0) / ch, side / cw, side / ch];
        Some((cells, angle, (largest_px, largest_px)))
    }

    /// Her rise through her presence, 0..1, a little past 1 before it
    /// settles (an ease out with an overshoot).
    fn rise(p: f32) -> f32 {
        let x = p.clamp(0.0, 1.0) - 1.0;
        let back = 1.9;
        1.0 + (back + 1.0) * x * x * x + back * x * x
    }

    /// The bank's lines with their tracks, the rest taken off each.
    fn load_lines(bank: &Path, goddess: &Goddess) -> Result<Vec<Line>, String> {
        let text = std::fs::read_to_string(bank.join("index.json")).map_err(|e| format!("{}: {e}", bank.join("index.json").display()))?;
        let index: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let read = |rel: &str| -> Result<Track, String> {
            let p = bank.join(rel);
            Track::from_json(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?, goddess)
        };
        let rest = index["rest"].as_str().map(read).transpose()?;
        let mut out = Vec::new();
        for l in index["lines"].as_array().map_or(&[][..], Vec::as_slice) {
            let s = |k: &str| l[k].as_str().unwrap_or("").to_string();
            let track = match read(&s("track")) {
                Ok(t) => t,
                Err(why) => {
                    eprintln!("angelface: line {}: {why}", s("id"));
                    continue;
                }
            };
            let track = match &rest {
                Some(r) => track.without_rest(r),
                None => track,
            };
            out.push(Line { id: s("id"), occasion: s("occasion"), wav: bank.join(s("wav")), track: Arc::new(track) });
        }
        Ok(out)
    }

    /// A line by id, or one at random for an occasion.
    fn pick<'a>(lines: &'a [Line], what: &str, rng: &mut u64) -> Option<&'a Line> {
        if let Some(line) = lines.iter().find(|l| l.id == what) {
            return Some(line);
        }
        let fit: Vec<&Line> = lines.iter().filter(|l| l.occasion == what).collect();
        (!fit.is_empty()).then(|| fit[(uniform(rng) * fit.len() as f32) as usize % fit.len()])
    }

    /// Play a WAV, owned: it stops when she does.
    fn play(wav: &Path) -> Option<ServiceChild> {
        let quiet = |c: &mut Command| {
            c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        };
        let mut pw = Command::new("pw-play");
        pw.arg(wav);
        quiet(&mut pw);
        if let Ok(child) = ServiceChild::spawn(&mut pw) {
            return Some(child);
        }
        let mut ff = Command::new("ffplay");
        ff.args(["-nodisp", "-autoexit", "-loglevel", "quiet"]).arg(wav);
        quiet(&mut ff);
        ServiceChild::spawn(&mut ff).ok()
    }

    fn uniform(rng: &mut u64) -> f32 {
        *rng ^= *rng >> 12;
        *rng ^= *rng << 25;
        *rng ^= *rng >> 27;
        (rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    fn chance(rng: &mut u64, odds: f32) -> bool {
        uniform(rng) < odds
    }
}
