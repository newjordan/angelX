//! Joining a friend's delve from your own angelX: `/dungeon join <link>`.
//!
//! The host's game is the only game. This client speaks the host's small
//! protocol: it polls the host's HUD and its own seat's
//! frame, sends held controls under a short lease, and posts gear, wishes and
//! reforged cards. A background thread does the talking, so the terminal
//! never waits on the network; when the host restarts, it keeps trying.

use super::together_shooter::mirror::{Live, Pose};
use super::together_shooter::{Book, HZ, Input, Run};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the client polls and renews its held controls.
const POLL: Duration = Duration::from_millis(66);
const RENEW: Duration = Duration::from_millis(80);

/// A frame from the host: its number, RGBA pixels, width and height.
#[derive(Clone)]
pub(crate) struct Frame {
    pub(crate) seq: u64,
    /// The PNG as the host sent it: Kitty takes it as it is.
    pub(crate) png: Arc<Vec<u8>>,
    pub(crate) rgba: Arc<Vec<u8>>,
    pub(crate) w: u32,
    pub(crate) h: u32,
}

/// What arrived since the last look: replies to posts, sounds, voice lines.
#[derive(Default)]
pub(crate) struct News {
    pub(crate) replies: Vec<String>,
    pub(crate) sounds: Vec<String>,
    pub(crate) voices: Vec<serde_json::Value>,
}

#[derive(Default)]
struct Shared {
    state: Option<serde_json::Value>,
    frame: Option<Frame>,
    error: Option<String>,
    input: Input,
    /// The host's raid and the next input sequence it will take.
    raid: u64,
    next_sequence: u64,
    outbox: VecDeque<(String, &'static str, Vec<u8>)>,
    news: News,
    /// The host's book of cards, for the card screen, and the raid and card
    /// count it was fetched for.
    book: Option<Arc<Book>>,
    /// The host's delve as this angelX holds it (`GET /stream`): the run,
    /// where things stood the tick before, and when this tick arrived.
    mirror: Option<Run>,
    before: Pose,
    ticked: Option<Instant>,
    /// The round trip of a key to the host and back, smoothed.
    rtt: Option<Duration>,
}

pub(crate) struct Joined {
    /// Where the host listens, for the cockpit to name.
    pub(crate) host: String,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

/// Read an invitation: `http://host:port/#token`, or the same without the
/// scheme. The token is the 32 hex characters after `#`.
pub(crate) fn parse_link(link: &str) -> Result<(String, String), String> {
    let link = link.trim().trim_start_matches("/dungeon join").trim();
    let (base, token) = link
        .split_once('#')
        .ok_or("paste the whole invitation, including the part after #")?;
    let token = token.trim();
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("that invitation's code is not complete".into());
    }
    let base = base.trim().trim_end_matches('/');
    let base = if base.starts_with("http://") || base.starts_with("https://") {
        base.to_string()
    } else {
        format!("http://{base}")
    };
    url::Url::parse(&base).map_err(|_| "that invitation's address is not readable")?;
    Ok((base, token.to_string()))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout(Duration::from_secs(3))
        .build()
}

/// The reply's message, or the error's.
fn reply_text(result: Result<ureq::Response, ureq::Error>) -> Result<String, String> {
    let text = |response: ureq::Response| -> String {
        let body = response.into_string().unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or(body)
    };
    match result {
        Ok(response) => Ok(text(response)),
        Err(ureq::Error::Status(_, response)) => Err(text(response)),
        Err(error) => Err(format!("the host can't be reached ({error})")),
    }
}

impl Joined {
    /// Check the invitation with the host, say who is joining, and start
    /// talking in the background.
    pub(crate) fn connect(link: &str, name: &str) -> Result<Joined, String> {
        let (base, token) = parse_link(link)?;
        let agent = agent();
        let bearer = format!("Bearer {token}");
        let state = agent
            .get(&format!("{base}/state"))
            .set("Authorization", &bearer)
            .call();
        reply_text(state.map(|r| r).and_then(|r| {
            if r.status() == 200 {
                Ok(r)
            } else {
                Err(ureq::Error::Status(r.status(), r))
            }
        }))
        .map_err(|e| format!("could not join: {e}"))?;
        let hello = agent
            .post(&format!("{base}/hello"))
            .set("Authorization", &bearer)
            .set("Content-Type", "text/plain")
            .send_string(name);
        let welcome = reply_text(hello).map_err(|e| format!("could not join: {e}"))?;
        let shared = Arc::new(Mutex::new(Shared::default()));
        shared.lock().expect("fresh").news.replies.push(welcome);
        let stop = Arc::new(AtomicBool::new(false));
        // Keys go out on their own thread, so a slow picture never holds
        // them past the host's lease.
        type Job = fn(&str, &str, &Mutex<Shared>, &AtomicBool);
        let mut workers = Vec::new();
        for (name, job) in [
            ("delve-join", talk as Job),
            ("delve-keys", send_keys),
            ("delve-mirror", mirror as Job),
        ] {
            let (shared, stop) = (Arc::clone(&shared), Arc::clone(&stop));
            let (base, bearer) = (base.clone(), bearer.clone());
            workers.push(
                std::thread::Builder::new()
                    .name(name.into())
                    .spawn(move || job(&base, &bearer, &shared, &stop))
                    .map_err(|e| e.to_string())?,
            );
        }
        let host = url::Url::parse(&base)
            .ok()
            .and_then(|u| {
                u.host_str()
                    .map(|h| format!("{h}:{}", u.port().unwrap_or(80)))
            })
            .unwrap_or(base);
        Ok(Joined {
            host,
            shared,
            stop,
            workers,
        })
    }

    pub(crate) fn set_input(&self, input: Input) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.input = input;
        }
    }

    /// The host's HUD, as last read.
    pub(crate) fn state(&self) -> Option<serde_json::Value> {
        self.shared.lock().ok().and_then(|s| s.state.clone())
    }

    /// The host's delve as held here, drawn part of the way between its
    /// last two ticks by the time since the newer one arrived; `f` gets it
    /// with the tick it shows. None until the first whole run arrives.
    pub(crate) fn with_view<T>(&self, f: impl FnOnce(&Run, u64) -> T) -> Option<T> {
        let mut shared = self.shared.lock().ok()?;
        let since = shared.ticked.map_or(0.0, |at| at.elapsed().as_secs_f32());
        let alpha = since * HZ as f32;
        // Our own knight goes where our keys take it, ahead of the host's
        // word by the time since this tick plus a round trip.
        let rtt = shared.rtt.map_or(0.0, |r| r.as_secs_f32());
        let me = shared
            .state
            .as_ref()
            .and_then(|s| s.get("actor").and_then(|a| a.as_u64()))
            .unwrap_or(2) as u32;
        let own = (me, shared.input, since + rtt);
        let Shared { mirror, before, .. } = &mut *shared;
        let run = mirror.as_mut()?;
        // Sixteenth-tick steps: a frame is new only when the view moved.
        let step = run.tick * 16 + (since * HZ as f32 * 15.0).min(15.0) as u64;
        Some(run.drawn_ahead(before, alpha, Some(own), |run| f(run, step)))
    }

    /// The host's book, once fetched.
    pub(crate) fn book(&self) -> Option<Arc<Book>> {
        self.shared.lock().ok().and_then(|s| s.book.clone())
    }

    pub(crate) fn frame(&self) -> Option<Frame> {
        self.shared.lock().ok().and_then(|s| s.frame.clone())
    }

    /// Why the host can't be reached, while it can't.
    pub(crate) fn error(&self) -> Option<String> {
        self.shared.lock().ok().and_then(|s| s.error.clone())
    }

    /// Send something to the host (a wish, a card, a reforged card).
    pub(crate) fn post(&self, path: &str, content_type: &'static str, body: Vec<u8>) {
        if let Ok(mut shared) = self.shared.lock() {
            if shared.outbox.len() < 8 {
                shared
                    .outbox
                    .push_back((path.to_string(), content_type, body));
            }
        }
    }

    pub(crate) fn news(&self) -> News {
        self.shared
            .lock()
            .map(|mut s| std::mem::take(&mut s.news))
            .unwrap_or_default()
    }

    /// The track the host's moment wants, as one of the delve's own names.
    pub(crate) fn music(&self) -> Option<&'static str> {
        let state = self.state()?;
        let track = state.get("music")?.as_str()?;
        ["crypt", "mines", "keep", "boss", "sanctuary", "menu"]
            .into_iter()
            .find(|&t| t == track)
    }

    /// This seat's knight id, from the host.
    pub(crate) fn actor(&self) -> u32 {
        self.state()
            .and_then(|s| s.get("actor").and_then(|a| a.as_u64()))
            .unwrap_or(2) as u32
    }
}

impl Drop for Joined {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

/// The background conversation with the host.
fn talk(base: &str, bearer: &str, shared: &Mutex<Shared>, stop: &AtomicBool) {
    let agent = agent();
    let mut shown_frame = 0u64;
    let mut heard_sound = 0u64;
    let mut heard_voice = 0u64;
    let mut book_for = (0u64, 0u64);
    let mut book_words = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        let state = agent
            .get(&format!("{base}/state"))
            .set("Authorization", bearer)
            .call()
            .ok()
            .and_then(|r| r.into_json::<serde_json::Value>().ok());
        let Some(state) = state else {
            if let Ok(mut s) = shared.lock() {
                s.error = Some("the host is away or rebuilding — reconnecting…".into());
            }
            std::thread::sleep(Duration::from_millis(800));
            continue;
        };
        let frame_seq = state.get("frame_seq").and_then(|v| v.as_u64()).unwrap_or(0);
        let mut frame = None;
        let mirrored = shared.lock().is_ok_and(|s| s.mirror.is_some());
        if frame_seq != shown_frame && !mirrored {
            let png = agent
                .get(&format!("{base}/frame.png?seq={frame_seq}"))
                .set("Authorization", bearer)
                .call()
                .ok()
                .and_then(|r| {
                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut r.into_reader(), &mut bytes)
                        .ok()
                        .map(|_| bytes)
                });
            if let Some((png, image)) =
                png.and_then(|png| image::load_from_memory(&png).ok().map(|i| (png, i)))
            {
                let rgba = image.to_rgba8();
                frame = Some(Frame {
                    seq: frame_seq,
                    png: Arc::new(png),
                    w: rgba.width(),
                    h: rgba.height(),
                    rgba: Arc::new(rgba.into_raw()),
                });
                shown_frame = frame_seq;
            }
        }
        // The host's phrasebook, when it learned something.
        let words_version = state
            .get("phrasebook")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if words_version != 0 && words_version != book_words {
            if let Some(text) = agent
                .get(&format!("{base}/phrasebook"))
                .set("Authorization", bearer)
                .call()
                .ok()
                .and_then(|r| r.into_string().ok())
            {
                super::together_shooter::phrasebook::take(&text, words_version);
                book_words = words_version;
            }
        }
        // The book only when it changed: a new raid, or a card added.
        let wanted = (
            state.get("raid_id").and_then(|v| v.as_u64()).unwrap_or(0),
            state
                .get("book_cards")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        );
        let mut book = None;
        if wanted != book_for && wanted.0 != 0 {
            book = agent
                .get(&format!("{base}/book"))
                .set("Authorization", bearer)
                .call()
                .ok()
                .and_then(|r| r.into_json::<Book>().ok());
            if book.is_some() {
                book_for = wanted;
            }
        }
        let outbox = {
            let Ok(mut s) = shared.lock() else {
                return;
            };
            if let Some(book) = book {
                s.book = Some(Arc::new(book));
            }
            s.error = None;
            s.raid = state.get("raid_id").and_then(|v| v.as_u64()).unwrap_or(0);
            let next = state.get("next_sequence").and_then(|v| v.as_u64());
            s.next_sequence = s.next_sequence.max(next.unwrap_or(1));
            for item in state
                .get("sounds")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                let seq = item.get(0).and_then(|v| v.as_u64()).unwrap_or(0);
                if seq > heard_sound {
                    heard_sound = seq;
                    if let Some(name) = item.get(1).and_then(|v| v.as_str()) {
                        s.news.sounds.push(name.to_string());
                    }
                }
            }
            if let Some(voice) = state.get("voice").filter(|v| !v.is_null()) {
                let seq = voice.get("seq").and_then(|v| v.as_u64()).unwrap_or(0);
                if seq > heard_voice {
                    heard_voice = seq;
                    s.news.voices.push(voice.clone());
                }
            }
            s.state = Some(state);
            if let Some(frame) = frame {
                s.frame = Some(frame);
            }
            std::mem::take(&mut s.outbox)
        };
        for (path, content_type, body) in outbox {
            let result = agent
                .post(&format!("{base}{path}"))
                .set("Authorization", bearer)
                .set("Content-Type", content_type)
                .send_bytes(&body);
            let line = match reply_text(result) {
                Ok(text) | Err(text) => text,
            };
            if let Ok(mut s) = shared.lock() {
                s.news.replies.push(line);
            }
        }
        if let Some(rest) = POLL.checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// The host's delve, streamed: one long `GET /stream`, a JSON object a
/// line — the whole run, then each tick's changes — into the mirror. When
/// the host goes away it reconnects, and the mirror starts over.
fn mirror(base: &str, bearer: &str, shared: &Mutex<Shared>, stop: &AtomicBool) {
    use std::io::{BufRead, BufReader, Write};
    let bearer = bearer.to_string();
    let Some((host, port)) = url::Url::parse(base)
        .ok()
        .and_then(|u| Some((u.host_str()?.to_string(), u.port_or_known_default()?)))
    else {
        return;
    };
    while !stop.load(Ordering::Relaxed) {
        let connected = std::net::TcpStream::connect((host.as_str(), port)).and_then(|mut tcp| {
            tcp.set_nodelay(true)?;
            tcp.set_read_timeout(Some(Duration::from_millis(500)))?;
            write!(
                tcp,
                "GET /stream HTTP/1.1\r\nHost: {host}:{port}\r\nAuthorization: {bearer}\r\n\r\n"
            )?;
            Ok(tcp)
        });
        let Ok(tcp) = connected else {
            std::thread::sleep(Duration::from_millis(800));
            continue;
        };
        let mut reader = BufReader::with_capacity(1 << 16, tcp);
        let mut line: Vec<u8> = Vec::with_capacity(1 << 16);
        let mut in_body = false;
        let mut good = true;
        while !stop.load(Ordering::Relaxed) && good {
            // A read that times out mid-line keeps what it has: the rest
            // of the line follows.
            match reader.read_until(b'\n', &mut line) {
                Ok(0) => good = false,
                Ok(_) if !line.ends_with(b"\n") => {}
                Ok(_) => {
                    if !in_body {
                        if line.starts_with(b"HTTP/") && !line.windows(5).any(|w| w == b" 200 ") {
                            good = false;
                        }
                        in_body = line == b"\r\n";
                    } else {
                        take_line(&line, shared);
                    }
                    line.clear();
                }
                // A quiet host (paused) is not a lost one; keep reading.
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => good = false,
            }
        }
        if let Ok(mut s) = shared.lock() {
            s.mirror = None;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// One line of the host's stream into the mirror.
fn take_line(line: &[u8], shared: &Mutex<Shared>) {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "lowercase")]
    enum News {
        Whole(Box<Run>),
        Live(Box<Live>),
    }
    let Ok(news) = serde_json::from_slice::<News>(line) else {
        return;
    };
    let Ok(mut s) = shared.lock() else {
        return;
    };
    match news {
        News::Whole(run) => {
            s.before = run.pose();
            s.mirror = Some(*run);
            s.ticked = Some(Instant::now());
        }
        News::Live(live) => {
            let Shared {
                mirror,
                before,
                ticked,
                ..
            } = &mut *s;
            if let Some(run) = mirror.as_mut() {
                let pose = run.pose();
                if run.apply_live(*live) {
                    *before = pose;
                    *ticked = Some(Instant::now());
                }
            }
        }
    }
}

/// Held controls to the host: sent when they change, renewed before the
/// host's lease lapses, resynchronized when the host starts a new raid.
fn send_keys(base: &str, bearer: &str, shared: &Mutex<Shared>, stop: &AtomicBool) {
    let agent = agent();
    let mut sent: Option<(Input, u64, Instant)> = None;
    let mut sequence = 0u64;
    while !stop.load(Ordering::Relaxed) {
        let Ok((input, raid, next)) = shared.lock().map(|s| (s.input, s.raid, s.next_sequence))
        else {
            return;
        };
        sequence = sequence.max(next);
        let due = sent.is_none_or(|(last, last_raid, at)| {
            last != input || last_raid != raid || at.elapsed() >= RENEW
        });
        if !due || raid == 0 {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        }
        let body = serde_json::json!({ "sequence": sequence, "raid_id": raid, "input": input });
        let posted = Instant::now();
        let result = agent
            .post(&format!("{base}/shooter/input"))
            .set("Authorization", bearer)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string());
        sequence += 1;
        match result {
            Ok(_) => {
                sent = Some((input, raid, Instant::now()));
                let trip = posted.elapsed().min(Duration::from_millis(250));
                if let Ok(mut s) = shared.lock() {
                    s.rtt = Some(s.rtt.map_or(trip, |r| (r * 7 + trip) / 8));
                }
            }
            // A stale sequence or a new raid: take the host's next and resend.
            Err(ureq::Error::Status(409, _)) => sent = None,
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_join__tests.rs"]
mod tests;
