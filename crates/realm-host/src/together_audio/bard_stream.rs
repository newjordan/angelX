//! The Bard's song on the host's own speakers as one unbroken stream.
//!
//! The game names a one-beat slice (`bard_<song>_<beat>`) on every beat of held fire, and
//! `bard_rest` when the fire is let go (`rules::bard::strum`). Played as a player process a slice,
//! each starts a little early or late and the song stutters; restarted whenever the named beat
//! drifts from the player's, it breaks. Here a song's slices are joined once into one file (beats
//! 0..95), and one player per song runs from the named beat and goes round the last phrases
//! (`LOOP_FROM`..`BEATS`) by itself for as long as the fire is held: the beats named while it plays
//! only keep it going, they never move or restart it. A rest pauses it where it is; the next beat
//! named seeks it to that beat and it plays on. A player is only stopped when it has sat paused a
//! while. Until a song's file is built, and wherever `ffmpeg` or `mpv` is missing (or off unix),
//! the caller plays the slices as before.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// One beat: 0.4 s (150 BPM), the length the slices are cut to.
const BEAT: f32 = 0.4;
const BEATS: u32 = angel_delve::rules::bard::BEATS;
const LOOP_FROM: u32 = angel_delve::rules::bard::LOOP_FROM;
/// With no beat named for this long the fire has stopped without its rest reaching us (or the
/// game has stalled that long): the player is paused.
const SILENCE: Duration = Duration::from_millis(1500);
/// Paused this long, a player is stopped; the next press starts a new one.
const IDLE: Duration = Duration::from_secs(90);

struct Stream {
    child: Child,
    sock: PathBuf,
    #[cfg(unix)]
    ipc: Option<std::os::unix::net::UnixStream>,
    playing: bool,
    /// The last beat named while playing, or the pause.
    heard: Instant,
}

impl Stream {
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// One command to the player. False when it cannot be reached yet (it opens its socket a
    /// few tens of milliseconds after it starts) or any more.
    #[cfg(unix)]
    fn send(&mut self, command: &str) -> bool {
        use std::io::Write;
        if self.ipc.is_none() {
            let Ok(ipc) = std::os::unix::net::UnixStream::connect(&self.sock) else { return false };
            let _ = ipc.set_write_timeout(Some(Duration::from_millis(100)));
            // The player answers every command; nobody reads the answers but this drain.
            if let Ok(mut answers) = ipc.try_clone() {
                let _ = std::thread::Builder::new().name("delve-bard-ipc".into()).spawn(move || {
                    let _ = std::io::copy(&mut answers, &mut std::io::sink());
                });
            }
            self.ipc = Some(ipc);
        }
        let sent = self.ipc.as_mut().is_some_and(|ipc| ipc.write_all(format!("{command}\n").as_bytes()).is_ok());
        if !sent {
            self.ipc = None;
        }
        sent
    }

    #[cfg(not(unix))]
    fn send(&mut self, _command: &str) -> bool {
        false
    }

    fn pause(&mut self, now: Instant) -> bool {
        let paused = self.send(r#"{"command":["set_property","pause",true]}"#);
        if paused {
            self.playing = false;
            self.heard = now;
        }
        paused
    }

    /// Play on from `beat`.
    fn resume(&mut self, beat: u32, now: Instant) -> bool {
        let at = beat as f32 * BEAT;
        let resumed = self.send(&format!(r#"{{"command":["seek",{at:.3},"absolute+exact"]}}"#))
            && self.send(r#"{"command":["set_property","pause",false]}"#);
        if resumed {
            self.playing = true;
            self.heard = now;
        }
        resumed
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.sock);
    }
}

/// Each song's player, by its letter.
fn state() -> &'static Mutex<BTreeMap<char, Stream>> {
    static STATE: OnceLock<Mutex<BTreeMap<char, Stream>>> = OnceLock::new();
    STATE.get_or_init(|| {
        // The watch: pauses a player whose beats stopped coming, stops one paused too long.
        let _ = std::thread::Builder::new().name("delve-bard-stream".into()).spawn(|| {
            loop {
                std::thread::sleep(Duration::from_millis(60));
                let Ok(mut streams) = state().lock() else { continue };
                let now = Instant::now();
                let mut gone = Vec::new();
                for (&song, stream) in streams.iter_mut() {
                    if !stream.alive() {
                        gone.push(song);
                    } else if stream.playing && now.saturating_duration_since(stream.heard) > SILENCE {
                        if !stream.pause(now) {
                            gone.push(song);
                        }
                    } else if !stream.playing && now.saturating_duration_since(stream.heard) > IDLE {
                        gone.push(song);
                    }
                }
                for song in gone {
                    if let Some(stream) = streams.remove(&song) {
                        stream.stop();
                    }
                }
            }
        });
        Mutex::new(BTreeMap::new())
    })
}

fn cache() -> PathBuf {
    std::env::temp_dir().join(format!("angelx-bard-stream-{}", std::process::id()))
}

/// Where a song's player listens: a short path (a socket path is capped near 100 bytes, and the
/// temp dir can be long).
fn sock(song: char) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir() && d.as_os_str().len() < 64)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join(format!("angelx-bard-{}-{song}.sock", std::process::id()))
}

/// The song's joined file, or none while it is being built (started here, once a song).
fn song_file(sfx: &Path, song: char) -> Option<PathBuf> {
    let file = cache().join(format!("bard_{song}.wav"));
    if file.is_file() {
        return Some(file);
    }
    static BUILDING: OnceLock<Mutex<HashSet<char>>> = OnceLock::new();
    let first = BUILDING.get_or_init(Default::default).lock().is_ok_and(|mut b| b.insert(song));
    if first {
        let sfx = sfx.to_path_buf();
        let _ = std::thread::Builder::new().name("delve-bard-join".into()).spawn(move || {
            let _ = std::fs::create_dir_all(cache());
            let mut command = Command::new("ffmpeg");
            command.args(["-v", "error", "-y"]);
            let mut graph = String::new();
            for beat in 0..BEATS {
                command.arg("-i").arg(sfx.join(format!("bard_{song}_{beat:03}.mp3")));
                graph.push_str(&format!("[{beat}:a]atrim=0:{BEAT},asetpts=N/SR/TB[a{beat}];"));
            }
            for beat in 0..BEATS {
                graph.push_str(&format!("[a{beat}]"));
            }
            graph.push_str(&format!("concat=n={BEATS}:v=0:a=1[out]"));
            let part = cache().join(format!("bard_{song}.part.wav"));
            command.args(["-filter_complex", &graph, "-map", "[out]", "-ar", "44100", "-ac", "2"]).arg(&part);
            let built = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
            if built.is_ok_and(|s| s.success()) {
                let _ = std::fs::rename(&part, file);
            }
        });
    }
    None
}

/// Every song's file, built ahead (on a thread each), so no press ever waits on one.
pub fn warm(sfx: &Path) {
    for song in ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'] {
        let _ = song_file(sfx, song);
    }
}

/// Slice `name` was named now. True when the stream carries it (playing on through it, or picked
/// up or started on its beat); false when the caller should play the slice itself.
pub fn heard(sfx: &Path, name: &str, level: f32, now: Instant) -> bool {
    if cfg!(not(unix)) {
        return false;
    }
    let mut parts = name.split('_');
    let (Some("bard"), Some(song), Some(beat)) = (parts.next(), parts.next(), parts.next()) else { return false };
    let (Some(song), Ok(beat)) = (song.chars().next(), beat.parse::<u32>()) else { return false };
    let beat = beat.min(BEATS - 1);
    let Some(file) = song_file(sfx, song) else { return false };
    let Ok(mut streams) = state().lock() else { return false };
    if let Some(stream) = streams.get_mut(&song) {
        if stream.alive() {
            if stream.playing {
                // Held: the player is the song's clock. A beat named a little early or late
                // never moves it.
                stream.heard = now;
                return true;
            }
            if stream.resume(beat, now) {
                return true;
            }
        }
        if let Some(stream) = streams.remove(&song) {
            stream.stop();
        }
    }
    let sock = sock(song);
    let _ = std::fs::remove_file(&sock);
    let child = Command::new("mpv")
        .args(["--no-video", "--really-quiet", "--no-terminal", "--audio-display=no", "--hr-seek=yes"])
        .arg(format!("--volume={}", (level.clamp(0.0, 1.0) * 100.0) as u32))
        .arg(format!("--start={:.3}", beat as f32 * BEAT))
        .arg(format!("--ab-loop-a={:.3}", LOOP_FROM as f32 * BEAT))
        .arg(format!("--ab-loop-b={:.3}", BEATS as f32 * BEAT))
        .arg(format!("--input-ipc-server={}", sock.display()))
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(child) = child else { return false };
    streams.insert(song, Stream {
        child,
        sock,
        #[cfg(unix)]
        ipc: None,
        playing: true,
        heard: now,
    });
    true
}

/// The fire was let go (`rules::bard::REST`): every playing song pauses where it is. A player
/// that cannot be reached is stopped instead, so the music never runs on.
pub fn rest() {
    let Ok(mut streams) = state().lock() else { return };
    let now = Instant::now();
    let mut gone = Vec::new();
    for (&song, stream) in streams.iter_mut() {
        if stream.playing && !stream.pause(now) {
            gone.push(song);
        }
    }
    for song in gone {
        if let Some(stream) = streams.remove(&song) {
            stream.stop();
        }
    }
}
