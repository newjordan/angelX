//! Native dynamic audio. Fetch/decode/preparation stays off the UI thread;
//! each admitted voice retains its original bytes until its child is reaped.
use super::*;
use crate::together_media::{Sound, Voice, parse_name};
use angel_serve::client::{Trust, http_get};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, mpsc};
const LATE: Duration = Duration::from_secs(2);
struct ResultItem {
    epoch: u64,
    asked: Instant,
    name: String,
    level: u8,
    sound: Option<Arc<Sound>>,
}
pub(super) struct Playback {
    source: Option<String>,
    epoch: u64,
    inflight: usize,
    tx: mpsc::Sender<ResultItem>,
    rx: mpsc::Receiver<ResultItem>,
    cache: BTreeMap<String, Arc<Sound>>,
    order: VecDeque<String>,
    last: BTreeMap<String, Instant>,
    voices: Vec<(
        Arc<std::sync::atomic::AtomicBool>,
        std::thread::JoinHandle<()>,
    )>,
    fetches: Vec<std::thread::JoinHandle<()>>,
}
impl Default for Playback {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            source: None,
            epoch: 0,
            inflight: 0,
            tx,
            rx,
            cache: BTreeMap::new(),
            order: VecDeque::new(),
            last: BTreeMap::new(),
            voices: Vec::new(),
            fetches: Vec::new(),
        }
    }
}
impl Playback {
    pub fn clear(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.source = None;
        self.cache.clear();
        self.order.clear();
        for (stop, _) in &self.voices {
            stop.store(true, std::sync::atomic::Ordering::Release);
        }
    }
    pub fn request(
        &mut self,
        name: &str,
        origin: Option<(String, Trust)>,
        now: Instant,
    ) -> Option<Voice> {
        let (file, level) = parse_name(name)?;
        let (url, trust) = origin?;
        if self.source.as_ref() != Some(&url) {
            self.clear();
            self.source = Some(url.clone());
        }
        if let Some(sound) = self.cache.get(file) {
            return Some(Voice {
                sound: Arc::clone(sound),
                level,
            });
        }
        if self.inflight >= 4 {
            return None;
        }
        self.inflight += 1;
        let epoch = self.epoch;
        let tx = self.tx.clone();
        let file = file.to_owned();
        self.fetches.push(std::thread::spawn(move || {
            let sound = http_get(
                &url,
                &format!("/a/{file}"),
                &trust,
                LATE,
                angel_serve::http::live::MAX_FILE,
            )
            .ok()
            .and_then(|bytes| Sound::received(&file, bytes).ok());
            let _ = tx.send(ResultItem {
                epoch,
                asked: now,
                name: file,
                level,
                sound,
            });
        }));
        None
    }
    pub fn poll(&mut self, now: Instant) -> Vec<Voice> {
        self.reap();
        let mut voices = Vec::new();
        while let Ok(item) = self.rx.try_recv() {
            self.inflight = self.inflight.saturating_sub(1);
            if item.epoch != self.epoch {
                continue;
            }
            let Some(sound) = item.sound else {
                continue;
            };
            if !self.cache.contains_key(&item.name) {
                self.order.push_back(item.name.clone());
            }
            self.cache.insert(item.name, sound.clone());
            while self.cache.len() > 16 {
                if let Some(name) = self.order.pop_front() {
                    self.cache.remove(&name);
                }
            }
            if now.saturating_duration_since(item.asked) <= LATE {
                voices.push(Voice {
                    sound,
                    level: item.level,
                });
            }
        }
        voices
    }
    pub fn admit(&mut self, voice: &Voice, now: Instant) -> bool {
        self.last
            .retain(|_, at| now.saturating_duration_since(*at) < Duration::from_secs(10));
        let name = voice.name();
        if self
            .last
            .get(&name)
            .is_some_and(|at| now.saturating_duration_since(*at) < Duration::from_millis(80))
        {
            return false;
        }
        if self.last.len() >= 128 {
            return false;
        }
        self.last.insert(name, now);
        true
    }
    fn reap(&mut self) {
        let mut i = 0;
        while i < self.voices.len() {
            if self.voices[i].1.is_finished() {
                let (_, job) = self.voices.swap_remove(i);
                let _ = job.join();
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.fetches.len() {
            if self.fetches[i].is_finished() {
                let job = self.fetches.swap_remove(i);
                let _ = job.join();
            } else {
                i += 1;
            }
        }
    }
    pub fn play(&mut self, voice: Voice) {
        if cfg!(any(test, feature = "testing")) {
            return;
        }
        self.reap();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let job = std::thread::spawn(move || {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                return;
            }
            if let Some(child) = once_player(voice.sound.path(), f32::from(voice.level) / 100.0) {
                finish(voice, child, &cancelled);
            }
        });
        self.voices.push((stop, job));
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        self.clear();
        for (_, job) in self.voices.drain(..) {
            let _ = job.join();
        }
        for job in self.fetches.drain(..) {
            let _ = job.join();
        }
    }
}
fn finish(voice: Voice, mut child: Child, cancelled: &std::sync::atomic::AtomicBool) {
    let until = Instant::now() + voice.sound.duration + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            Ok(None) => {}
        }
        if Instant::now() >= until || cancelled.load(std::sync::atomic::Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(voice);
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn wave() -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/live/ability-sounds/spark.wav"),
        )
        .unwrap()
    }
    #[test]
    fn dropping_playback_cancels_and_reaps_owned_voice_jobs_then_cleanup_drains() {
        let retirement = crate::together_media::RetirementGuard;
        let bytes = wave();
        let id = angel_live::host::sha256_hex(&bytes);
        let sound = Sound::prepare(Arc::new(angel_live::Asset { id, bytes })).unwrap();
        let path = sound.path().to_owned();
        let weak = Arc::downgrade(&sound);
        let child = Command::new("sh")
            .args(["-c", "exec sleep 10"])
            .spawn()
            .unwrap();
        let mut playback = Playback::default();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let job = std::thread::spawn(move || finish(Voice { sound, level: 30 }, child, &cancelled));
        playback.voices.push((stop, job));
        drop(playback);
        assert!(weak.upgrade().is_none());
        drop(retirement);
        assert!(!path.exists());
    }
    #[test]
    fn a_playing_child_keeps_its_original_sound_file_until_it_is_reaped() {
        let bytes = wave();
        let id = angel_live::host::sha256_hex(&bytes);
        let sound = Sound::prepare(Arc::new(angel_live::Asset {
            id,
            bytes: bytes.clone(),
        }))
        .unwrap();
        let path = sound.path().to_owned();
        let heard = path.with_extension("heard");
        let weak = Arc::downgrade(&sound);
        let file = Arc::clone(&sound.file);
        let child = Command::new("sh")
            .args(["-c", "sleep 0.08; cat \"$1\" > \"$2\"", "voice"])
            .arg(&path)
            .arg(&heard)
            .spawn()
            .unwrap();
        let voice = Voice { sound, level: 30 };
        let playing = std::thread::spawn(move || {
            finish(voice, child, &std::sync::atomic::AtomicBool::new(false))
        });
        assert!(weak.upgrade().is_some());
        assert!(path.exists());
        playing.join().unwrap();
        assert_eq!(std::fs::read(&heard).unwrap(), bytes);
        assert!(weak.upgrade().is_none());
        let until = Instant::now() + Duration::from_secs(1);
        while path.exists() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!path.exists());
        assert_eq!(
            file.bytes(),
            bytes,
            "HTTP lease lost bytes when the native voice retired"
        );
        std::fs::remove_file(heard).unwrap();
    }
    fn remote(bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let thread = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < until =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("native client did not fetch its audio fixture: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            std::thread::sleep(Duration::from_millis(40));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(&bytes).unwrap();
        });
        (url, thread)
    }
    #[test]
    fn remote_fetch_is_prepared_once_and_abandoned_source_completions_do_not_play() {
        let bytes = wave();
        let id = angel_live::host::sha256_hex(&bytes);
        let file = format!("live-{}.wav", &id[..48]);
        let name = format!("{file}@25");
        let (url, server) = remote(bytes.clone());
        let mut playback = Playback::default();
        assert!(
            playback
                .request(
                    &name,
                    Some((url.clone(), Default::default())),
                    Instant::now()
                )
                .is_none()
        );
        let until = Instant::now() + Duration::from_secs(2);
        let mut ready = Vec::new();
        while playback.inflight > 0 && Instant::now() < until {
            ready.extend(playback.poll(Instant::now()));
            std::thread::sleep(Duration::from_millis(5));
        }
        server.join().unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].sound.file.bytes(), bytes);
        assert_eq!(
            playback
                .request(
                    &format!("{file}@60"),
                    Some((url, Default::default())),
                    Instant::now()
                )
                .unwrap()
                .level,
            60
        );
        let (url, server) = remote(bytes);
        playback.request(&name, Some((url, Default::default())), Instant::now());
        playback.clear();
        let until = Instant::now() + Duration::from_secs(2);
        while playback.inflight > 0 && Instant::now() < until {
            assert!(playback.poll(Instant::now()).is_empty());
            std::thread::sleep(Duration::from_millis(5));
        }
        server.join().unwrap();
        assert!(playback.cache.is_empty());
        assert_eq!(playback.inflight, 0);
    }
}
