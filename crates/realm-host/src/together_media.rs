//! Prepared media owned by executable resources and active playback. File IO,
//! format validation and HTTP payload preparation happen before publication.
use angel_live::Asset;
use angel_serve::http::{LiveAsset, LiveAssets};
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundSpec {
    asset: String,
    #[serde(default = "default_level")]
    level: f32,
}
fn default_level() -> f32 {
    0.5
}
#[derive(Clone, Debug)]
pub struct Voice {
    pub sound: Arc<Sound>,
    pub level: u8,
}
impl Voice {
    pub fn name(&self) -> String {
        format!("{}@{}", self.sound.file.name, self.level)
    }
}
pub struct Sound {
    pub file: Arc<LiveAsset>,
    pub duration: Duration,
    path: PathBuf,
}
impl std::fmt::Debug for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sound")
            .field("file", &self.file.name)
            .field("duration", &self.duration)
            .finish()
    }
}
impl Drop for Sound {
    fn drop(&mut self) {
        if let Some(Ok(cleanup)) = CLEANUP.get() {
            let _ = cleanup.send(Retirement::Remove(self.path.clone()));
        }
    }
}
enum Retirement {
    Remove(PathBuf),
    Flush(std::sync::mpsc::SyncSender<()>),
}
pub struct RetirementGuard;
impl Drop for RetirementGuard {
    fn drop(&mut self) {
        if let Some(Ok(queue)) = CLEANUP.get() {
            let (tx, rx) = std::sync::mpsc::sync_channel(0);
            if queue.send(Retirement::Flush(tx)).is_ok() {
                let _ = rx.recv_timeout(Duration::from_secs(2));
            }
        }
    }
}
static CLEANUP: std::sync::OnceLock<Result<std::sync::mpsc::Sender<Retirement>, String>> =
    std::sync::OnceLock::new();
fn cleanup() -> Result<&'static std::sync::mpsc::Sender<Retirement>, String> {
    CLEANUP
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<Retirement>();
            std::thread::Builder::new()
                .name("angel-sound-retire".into())
                .spawn(move || {
                    while let Ok(message) = rx.recv() {
                        match message {
                            Retirement::Remove(path) => {
                                let _ = std::fs::remove_file(path);
                            }
                            Retirement::Flush(reply) => {
                                let _ = reply.send(());
                            }
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(tx)
        })
        .as_ref()
        .map_err(Clone::clone)
}
struct Bytes(Arc<Asset>);
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0.bytes
    }
}
thread_local! {static SOUNDS:std::cell::RefCell<BTreeMap<String,Weak<Sound>>>=const{std::cell::RefCell::new(BTreeMap::new())};}
impl Sound {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn prepare(asset: Arc<Asset>) -> Result<Arc<Self>, String> {
        if let Some(sound) =
            SOUNDS.with(|pool| pool.borrow().get(&asset.id).and_then(Weak::upgrade))
        {
            return Ok(sound);
        }
        let duration = wav_duration(&asset.bytes)?;
        cleanup()?;
        let file = LiveAsset::prepare(&asset.id, "wav", "audio/wav", Bytes(Arc::clone(&asset)))
            .map_err(|e| e.to_string())?;
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "angel-live-sound-{}-{nonce}-{serial}.wav",
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&path).map_err(|e| e.to_string())?;
        let sound = Arc::new(Self {
            file,
            duration,
            path,
        });
        if let Err(error) = output.write_all(&asset.bytes) {
            drop(output);
            return Err(error.to_string());
        }
        drop(output);
        SOUNDS.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.retain(|_, sound| sound.strong_count() > 0);
            pool.insert(asset.id.clone(), Arc::downgrade(&sound));
        });
        Ok(sound)
    }
    pub fn received(name: &str, bytes: Vec<u8>) -> Result<Arc<Self>, String> {
        let id = angel_live::host::sha256_hex(&bytes);
        if name != format!("live-{}.wav", &id[..48]) {
            return Err("media content identity differs".into());
        }
        Self::prepare(Arc::new(Asset { id, bytes }))
    }
}
pub fn parse_name(name: &str) -> Option<(&str, u8)> {
    let (file, level) = name.rsplit_once('@')?;
    if level.is_empty() || level.len() > 3 || !level.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hash = file.strip_prefix("live-")?.strip_suffix(".wav")?;
    let level: u8 = level.parse().ok()?;
    (hash.len() == 48
        && hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        && level <= 100)
        .then_some((file, level))
}
pub fn prepare(
    specs: &BTreeMap<String, SoundSpec>,
    assets: &BTreeMap<String, Arc<Asset>>,
) -> Result<BTreeMap<String, Voice>, String> {
    if specs.len() > 16 {
        return Err("sound bank exceeds 16 cues".into());
    }
    specs
        .iter()
        .map(|(name, spec)| {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                || !spec.level.is_finite()
                || !(0.0..=1.0).contains(&spec.level)
            {
                return Err("invalid sound cue/level".into());
            }
            let asset = assets
                .values()
                .find(|a| a.id == spec.asset)
                .ok_or("sound asset is not captured in this generation")?;
            Ok((
                name.clone(),
                Voice {
                    sound: Sound::prepare(Arc::clone(asset))?,
                    level: (spec.level * 100.0).round() as u8,
                },
            ))
        })
        .collect()
}
fn wav_duration(bytes: &[u8]) -> Result<Duration, String> {
    let bad = || {
        "sound must be a complete PCM16 WAV, 1..2 channels, 8..48kHz, at most 10 seconds"
            .to_string()
    };
    let u16_at = |at: usize| {
        bytes
            .get(at..at + 2)
            .and_then(|v| v.try_into().ok())
            .map(u16::from_le_bytes)
            .ok_or_else(bad)
    };
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .and_then(|v| v.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(bad)
    };
    if bytes.len() < 44
        || bytes.get(..4) != Some(b"RIFF")
        || bytes.get(8..12) != Some(b"WAVE")
        || u32_at(4)? as usize + 8 != bytes.len()
    {
        return Err(bad());
    }
    let (mut at, mut format, mut data) = (12, None, None);
    while at < bytes.len() {
        let len = u32_at(at + 4)? as usize;
        let start = at + 8;
        let end = start.checked_add(len).ok_or_else(bad)?;
        if end > bytes.len() {
            return Err(bad());
        }
        match bytes.get(at..at + 4) {
            Some(b"fmt ") => {
                if format.is_some() || len < 16 {
                    return Err(bad());
                }
                let channels = u16_at(start + 2)?;
                let rate = u32_at(start + 4)?;
                let align = u16_at(start + 12)?;
                if u16_at(start)? != 1
                    || !(1..=2).contains(&channels)
                    || !(8000..=48000).contains(&rate)
                    || u16_at(start + 14)? != 16
                    || align != channels * 2
                    || u32_at(start + 8)? != rate * u32::from(align)
                {
                    return Err(bad());
                }
                format = Some((rate, align));
            }
            Some(b"data") => {
                if data.replace(len).is_some() {
                    return Err(bad());
                }
            }
            _ => {}
        }
        at = end.checked_add(len % 2).ok_or_else(bad)?;
    }
    let (rate, align) = format.ok_or_else(bad)?;
    let len = data.ok_or_else(bad)?;
    let frames = len / usize::from(align);
    if at != bytes.len() || len == 0 || len % usize::from(align) != 0 || frames > rate as usize * 10
    {
        return Err(bad());
    }
    Ok(Duration::from_secs_f64(frames as f64 / f64::from(rate)))
}

#[derive(Default)]
pub struct Media {
    pub files: LiveAssets,
    pending: VecDeque<Voice>,
    recent: BTreeMap<String, (Instant, Voice)>,
    pub emitted: u64,
}
impl Media {
    pub fn reset(&mut self) {
        self.pending.clear();
        self.recent.clear();
    }
    pub fn admit(&self, voices: &[Voice]) -> Result<(), std::io::Error> {
        self.files.admit(
            &voices
                .iter()
                .map(|v| Arc::clone(&v.sound.file))
                .collect::<Vec<_>>(),
        )
    }
    pub fn commit(&mut self, voices: Vec<Voice>) {
        let now = Instant::now();
        self.recent.retain(|_, (until, _)| *until > now);
        for voice in voices {
            self.recent
                .insert(voice.name(), (now + Duration::from_secs(3), voice.clone()));
            self.pending.push_back(voice);
            self.emitted += 1;
        }
    }
    pub fn take(&mut self) -> Vec<Voice> {
        self.pending.drain(..).collect()
    }
    pub fn voice(&mut self, name: &str) -> Option<Voice> {
        let now = Instant::now();
        self.recent.retain(|_, (until, _)| *until > now);
        self.recent.get(name).map(|(_, v)| v.clone())
    }
    pub fn retire(&mut self) {
        let now = Instant::now();
        self.recent.retain(|_, (until, _)| *until > now);
        self.files.retire(now);
    }
}
