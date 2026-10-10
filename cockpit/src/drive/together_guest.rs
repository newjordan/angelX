//! Invited friends' windows onto the dungeon delve: one seat per invitation,
//! players 2 to 4. The terminal event loop owns the run; this listener serves
//! the host's own rendering of it as a PNG (each seat's camera on its own
//! knight), a compact HUD, and queues each seat's held controls. A friend's
//! own angelX (`/dungeon join`) draws an admitted host mirror. The browser page
//! shows the seat's frames and sends its held controls; neither client awards
//! defeats.

use super::together_shooter::mirror::Pose;
use super::together_shooter::{DEEPEST, HEIGHT, Input, Phase, Run, WIDTH};
use crate::stage::world_viz::overworld::arena;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The invitation link opened in a browser says where it goes: friends join
/// from their own angelX.
const CLIENT: &str = "This is an angelX Delve invitation.\nPaste the whole line into your angelX composer: /dungeon join <link>\n";
/// With the browser view on, the same link opens a playable page instead: the
/// seat's frames, its HUD, and held controls sent back as `/shooter/input`.
const BROWSER_VIEW: &str = include_str!("../../assets/dungeon/guest.html");
const MAX_HEADER: usize = 4096;
const MAX_BODY: usize = 512;
const MAX_CONNECTIONS: usize = 4;
/// Friends a host may invite at once: a party of four knights.
pub(crate) const MAX_SEATS: usize = 3;
/// Pixels per arena unit: the arena's native scale (16 px to a tile, as in
/// the overworld), so the guest sees the host's pixels one to one and the
/// page scales the frame up by whole device pixels.
pub(crate) const FRAME_SCALE: u32 = 8;
pub(crate) const FRAME_W: u32 = WIDTH as u32 * FRAME_SCALE;
pub(crate) const FRAME_H: u32 = HEIGHT as u32 * FRAME_SCALE;
/// Simulation ticks between frames: every tick, 30 frames a second at 30 Hz,
/// so a browser seat moves as smoothly as the host's own screen.
pub(crate) const FRAME_TICKS: u64 = 1;
/// The host publishes on every terminal tick, idle ones (200 ms) included.
const HOST_SILENCE: Duration = Duration::from_secs(2);
/// Frame fetches have their own budget beside the 48 other requests a second.
const FRAMES_PER_SECOND: u32 = 70;
/// Half a 30 Hz tick: the painter draws each tick and the halfway point
/// before it, 60 frames a second.
const HALF_TICK: Duration = Duration::from_micros(16_667);
/// Local play samples the motion three times between ticks: 120 frames a
/// second, paced across the 30 Hz tick. A far link stays at the halfway frame.
const LOCAL_STEP: Duration = Duration::from_micros(8_333);
const LOCAL_ALPHAS: [f32; 3] = [0.25, 0.5, 0.75];
/// The most tiles one patch carries. The page refuses a patch with more and
/// asks for a whole picture, so no budget here may pass it.
pub(crate) const MAX_PATCH_TILES: usize = 96;
/// A local seat's patch budget: a wider change goes as a whole fast PNG.
const LOCAL_PATCH_TILES: usize = 24;
/// A whole picture after this many patches in a row, in case a page lost
/// its picture without saying so. The page asks for one when it notices.
const HEAL_AFTER: u32 = 48;
/// A seat counts as watched this long after its last request. The page's
/// HTTP fallback polls state four times a second, a joined angelX faster.
const SEAT_PRESENCE: Duration = Duration::from_secs(2);
/// The host's lease on a seat's held controls. The painter leads a knight
/// only while its keys are this fresh, as the host only steps them so long.
pub(crate) const GUEST_LEASE: Duration = Duration::from_millis(500);

/// Set by the local one-link host. Loopback has bandwidth to spare, so the
/// painter spends its time on more frames instead of a smaller picture.
fn fast_frames() -> bool {
    std::env::var("ANGEL_DUNGEON_FAST_FRAMES").is_ok_and(|value| value == "1")
}

/// Held controls belong only to their seat's knight. The host must also check
/// the raid and expire this input shortly after received_at, including time
/// spent queued.
pub(crate) struct GuestShooterIntent {
    pub(crate) player: u32,
    pub(crate) raid_id: u64,
    pub(crate) input: Input,
    pub(crate) received_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShooterSubmission {
    sequence: u64,
    raid_id: u64,
    input: Input,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct Knight {
    id: u32,
    name: String,
    /// Which knight of the company they play, if one.
    knight: String,
    hp: u32,
    max_hp: u32,
    weapon: String,
    mail: u32,
    bombs: u32,
    alive: bool,
    stone: bool,
    /// Stone by this knight's own key, mending those near.
    vigil: bool,
    /// Play cards by name, in slot order (keys 1–4).
    hand: Vec<String>,
    spells: Vec<Option<String>>,
    spell_cooldowns: Vec<u32>,
    /// Held cards by name.
    deck: Vec<String>,
    /// The same by card id, with the weapon's, for a joined angelX's card
    /// screen to look up in the book.
    hand_ids: Vec<String>,
    spell_ids: Vec<Option<String>>,
    deck_ids: Vec<String>,
    arm_id: Option<String>,
    /// Spoils not yet banked.
    carrying: String,
    /// Standing in a Sanctuary with this floor's reforge unspent.
    can_reforge: bool,
    /// An Overclass window is open and this knight has a wish in it.
    can_wish: bool,
}

/// What the page shows around the frame.
#[derive(Clone, Debug, PartialEq, Serialize)]
struct Hud {
    raid_id: u64,
    paused: bool,
    phase: Phase,
    floor: u32,
    floors: u32,
    pack: &'static str,
    score: u32,
    /// Fortune's audience, in thousands of viewers.
    viewers: u32,
    players: Vec<Knight>,
    /// The party stands in a Sanctuary.
    sanctuary: bool,
    /// The party stands in a side-on hall: up jumps, down drops.
    side_on: bool,
    /// The phrasebook's version: a friend reads it again when this moves.
    phrasebook: u64,
    /// Cards in the run's book: a joined angelX fetches `/book` again when
    /// this or the raid changes.
    book_cards: usize,
    notice: String,
    boss_gates: Option<super::together_shooter::BossGateState>,
    boss_support: Option<String>,
    boss_names: Vec<String>,
    /// The track the host's moment wants.
    music: Option<&'static str>,
}

impl Hud {
    fn of(run: &Run, paused: bool, notice: &str) -> Hud {
        Hud {
            raid_id: run.raid_id,
            paused,
            phase: run.phase,
            floor: run.floor(),
            floors: DEEPEST,
            pack: run.dungeon.pack.name(),
            score: run.score,
            viewers: run.audience,
            players: run
                .players
                .iter()
                .map(|(&id, hero)| Knight {
                    id,
                    // The host's knight is "You" only on the host's screen.
                    name: if id == 1 && hero.name == "You" {
                        "Host".into()
                    } else {
                        hero.name.clone()
                    },
                    hp: hero.hp,
                    max_hp: hero.max_hp,
                    knight: hero.knight_name().unwrap_or_default(),
                    weapon: hero
                        .forged
                        .as_ref()
                        .map(|w| w.name.clone())
                        .or_else(|| {
                            hero.arm
                                .as_deref()
                                .and_then(|id| run.book.get(id))
                                .map(|c| c.name.clone())
                        })
                        .unwrap_or_else(|| hero.weapon.name().to_owned()),
                    mail: hero.armor,
                    bombs: hero.bombs,
                    alive: hero.hp > 0,
                    stone: hero.stone,
                    vigil: hero.vigil,
                    hand: names(run, &hero.hand),
                    spells: hero
                        .spells
                        .iter()
                        .map(|id| {
                            id.as_deref()
                                .and_then(|id| run.book.get(id))
                                .map(|c| c.name.clone())
                        })
                        .collect(),
                    spell_cooldowns: hero
                        .spell_cooldowns
                        .iter()
                        .map(|t| t.div_ceil(super::together_shooter::HZ))
                        .collect(),
                    deck: names(run, &hero.deck),
                    hand_ids: hero.hand.clone(),
                    spell_ids: hero.spells.to_vec(),
                    deck_ids: hero.deck.clone(),
                    arm_id: hero.arm.clone(),
                    carrying: hero.carried.label(),
                    can_reforge: run.can_reforge(id),
                    can_wish: run.can_wish(id),
                })
                .collect(),
            sanctuary: run.room().kind == super::together_shooter::RoomKind::Sanctuary,
            book_cards: run.book.cards.len(),
            side_on: run.side_on(),
            phrasebook: super::together_shooter::phrasebook::version(),
            music: run.music(),
            boss_gates: run.boss_gate_state(),
            boss_support: run.boss_support_line(),
            boss_names: run.bosses.iter().take(64).map(|b| b.name.clone()).collect(),
            notice: run
                .boss_gate_line()
                .as_deref()
                .unwrap_or(notice)
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect(),
        }
    }
}

fn names(run: &Run, ids: &[String]) -> Vec<String> {
    ids.iter()
        .map(|id| {
            run.book
                .get(id)
                .map_or_else(|| id.clone(), |c| c.name.clone())
        })
        .collect()
}

pub(crate) enum Gear {
    Weapon(super::together_forge::Weapon),
    Avatar(super::together_avatar::Avatar),
    Card(super::together_shooter::Card),
    /// A wish in the guest's own words.
    Wish(String),
    /// Raise a drafted wish the treasury can pay for.
    Grant(String),
    /// Reforge one part of the seat's kit, in their words (in a Sanctuary).
    Reforge(super::together_shooter::knights::Part, String),
    /// A reforged card the friend's own angelX drafted; checked here and
    /// again by the host before it is equipped.
    ReforgeCard(
        super::together_shooter::knights::Part,
        super::together_shooter::Card,
    ),
    /// A friend arrived (or renamed themselves): their knight joins the party.
    Hello(String),
    /// A known wish (the phrasebook's id), for the open Overclass window.
    Boon(String),
    /// A wish the phrasebook doesn't know: the host's angelX teaches it.
    Learn(String),
}

/// The treasury and the wishing stone, as the guest page shows them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct RealmHud {
    treasury: String,
    wishes: Vec<WishHud>,
    /// The wish the host's angelX is drafting right now.
    drafting: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct WishHud {
    id: String,
    name: String,
    words: String,
    by: String,
    status: super::together_realm::Status,
    price: String,
    /// What the treasury still lacks; empty when it can pay.
    need: String,
}

impl RealmHud {
    pub(crate) fn of(realm: &super::together_realm::Realm, drafting: Option<String>) -> RealmHud {
        RealmHud {
            drafting,
            treasury: realm.treasury.label(),
            wishes: realm
                .wishes
                .iter()
                .map(|w| WishHud {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    words: w.words.clone(),
                    by: w.by.clone(),
                    status: w.status,
                    price: w.price.label(),
                    need: realm.treasury.shortfall(&w.price),
                })
                .collect(),
        }
    }
}

/// One seat's newest picture as the painter left it. Readers turn it into
/// what they send: a socket a patch against the picture its page holds, or
/// a whole PNG; an HTTP route always a whole PNG.
#[derive(Clone)]
struct Picture {
    /// The frame it was painted for. A still picture keeps its number, so
    /// nobody sends it twice.
    seq: u64,
    /// When it was painted (Unix ms), for the page to tell how old it is.
    painted_ms: u64,
    /// Knight box in this picture (see `stamp_header`).
    stamp: [u8; 8],
    /// Row-major RGB.
    rgb: Arc<Vec<u8>>,
    /// The picture as a PNG, kept once a reader needed it.
    png: Option<Arc<Vec<u8>>>,
    /// The local fast path: a light PNG and a fixed tile budget. A far
    /// link packs its PNG and sends tiles only when they are smaller.
    fast: bool,
}

struct Published {
    /// Seats in play: player ids 2.. one per invitation.
    seats: Vec<u32>,
    gear: Vec<(u64, u32, Gear)>,
    realm: RealmHud,
    /// The realm screen around the newest wish, and its version.
    realm_png: Option<Arc<Vec<u8>>>,
    realm_seq: u64,
    /// The chorus line now spoken, for the page to play and subtitle.
    voice: Option<serde_json::Value>,
    /// The last sounds the delve made, numbered, for a joined angelX to play.
    sounds: std::collections::VecDeque<(u64, &'static str)>,
    sound_seq: u64,
    forge_runes: std::collections::BTreeMap<u32, String>,
    hud: Option<Hud>,
    /// The run's book as JSON, with the raid and card count it was made for.
    book: Option<(u64, usize, Arc<Vec<u8>>)>,
    /// For friends' mirrors: the whole run when its raid, floor or book
    /// changed, and each tick's changes (numbered). Seats streaming them
    /// need no pictures painted; each seat's newest stream wins.
    whole: Option<((u64, u32, usize), Arc<Vec<u8>>)>,
    live: Option<(u64, Arc<Vec<u8>>)>,
    live_from: Option<(u64, u64, bool)>,
    streams: std::collections::BTreeMap<u32, u64>,
    /// Browser seats' pushed frame streams and sockets: each seat's newest
    /// one wins.
    frame_streams: std::collections::BTreeMap<u32, u64>,
    /// The newest frame each browser seat says it has received.
    frame_acks: std::collections::BTreeMap<u32, u64>,
    stream_serial: u64,
    playable: std::collections::BTreeMap<u32, bool>,
    host_seen: Instant,
    frame_seq: u64,
    /// Each seat's newest picture, its camera on its own knight.
    pictures: std::collections::BTreeMap<u32, Picture>,
    /// When each seat last made a request: a polling page or a joined
    /// angelX is painted for `SEAT_PRESENCE` after.
    seat_seen: std::collections::BTreeMap<u32, Instant>,
    /// A seat began watching: the next host publish hands the painter its
    /// run even if the tick has not moved, so a paused delve is drawn too.
    repaint: bool,
    last_shooter_submission: std::collections::BTreeMap<u32, ShooterSubmission>,
    /// Held controls and when they came, so a seat's own knight can be
    /// painted ahead of the tick while its keys are fresh.
    held: std::collections::BTreeMap<u32, (Input, Instant)>,
    /// Round trip in milliseconds, as that seat's page measured it.
    seat_rtt_ms: std::collections::BTreeMap<u32, u32>,
    rate_start: Instant,
    requests: u32,
    frames: u32,
    shooter_inputs: u32,
    /// The invitation link opens the read-only browser view (off by default).
    browser_view: bool,
}

impl Default for Published {
    fn default() -> Self {
        Self {
            seats: vec![2],
            gear: Vec::new(),
            realm: RealmHud::default(),
            realm_png: None,
            realm_seq: 0,
            voice: None,
            sounds: std::collections::VecDeque::new(),
            sound_seq: 0,
            forge_runes: Default::default(),
            hud: None,
            book: None,
            whole: None,
            live: None,
            live_from: None,
            streams: Default::default(),
            frame_streams: Default::default(),
            frame_acks: Default::default(),
            stream_serial: 0,
            playable: Default::default(),
            host_seen: Instant::now(),
            frame_seq: 0,
            pictures: Default::default(),
            seat_seen: Default::default(),
            repaint: false,
            last_shooter_submission: Default::default(),
            held: Default::default(),
            seat_rtt_ms: Default::default(),
            rate_start: Instant::now(),
            requests: 0,
            frames: 0,
            shooter_inputs: 0,
            browser_view: false,
        }
    }
}

impl Published {
    fn snapshot(&self, seat: u32) -> Option<Vec<u8>> {
        let hud = self.hud.as_ref()?;
        Some(
            serde_json::to_vec(&serde_json::json!({
                "version": 3,
                "actor": seat,
                "raid_id": hud.raid_id,
                "next_sequence": self.last_shooter_submission.get(&seat).map_or(1, |s| s.sequence + 1),
                "frame_seq": self.frame_seq,
                "frame": { "width": FRAME_W, "height": FRAME_H },
                "host": self.host_seen.elapsed() < HOST_SILENCE,
                "playable": self.playable.get(&seat).copied().unwrap_or(false),
                "paused": hud.paused,
                "phase": hud.phase,
                "floor": hud.floor,
                "floors": hud.floors,
                "pack": hud.pack,
                "score": hud.score,
                "viewers": hud.viewers,
                "players": hud.players,
                "notice": hud.notice,
                "boss_gates": hud.boss_gates,
                "boss_support": hud.boss_support,
                "boss_names": hud.boss_names,
                "realm": self.realm,
                "realm_seq": self.realm_seq,
                "voice": self.voice,
                "music": hud.music,
                "sanctuary": hud.sanctuary,
                "side_on": hud.side_on,
                "phrasebook": hud.phrasebook,
                "book_cards": hud.book_cards,
                "sounds": self.sounds,
            }))
            .expect("guest snapshot is JSON data"),
        )
    }

    fn roll_window(&mut self) {
        if self.rate_start.elapsed() >= Duration::from_secs(1) {
            self.rate_start = Instant::now();
            self.requests = 0;
            self.frames = 0;
            self.shooter_inputs = 0;
        }
    }

    /// Budgets grow with the seats: each friend polls on their own.
    fn allow_request(&mut self) -> bool {
        self.roll_window();
        self.requests += 1;
        self.requests <= 48 * self.seats.len().max(1) as u32
    }

    fn allow_frame(&mut self) -> bool {
        self.roll_window();
        self.frames += 1;
        self.frames <= FRAMES_PER_SECOND * self.seats.len().max(1) as u32
    }

    /// Someone will look at this seat's pictures: a socket or frame stream
    /// is open, or the seat made a request a moment ago.
    fn watched(&self, seat: u32, now: Instant) -> bool {
        self.frame_streams.contains_key(&seat)
            || self
                .seat_seen
                .get(&seat)
                .is_some_and(|at| now.saturating_duration_since(*at) < SEAT_PRESENCE)
    }

    /// The seats the painter draws: watched, and not drawing for themselves
    /// from a mirror stream.
    fn picture_seats(&self, now: Instant) -> Vec<u32> {
        self.seats
            .iter()
            .copied()
            .filter(|seat| !self.streams.contains_key(seat) && self.watched(*seat, now))
            .collect()
    }

    /// A request from `seat`. A seat nobody was watching gets a fresh
    /// picture on the next host publish.
    fn saw_seat(&mut self, seat: u32, now: Instant) {
        if !self.watched(seat, now) {
            self.repaint = true;
        }
        self.seat_seen.insert(seat, now);
    }

    /// A socket or frame stream opened for `seat`; returns its serial.
    fn open_frame_stream(&mut self, seat: u32) -> u64 {
        self.stream_serial += 1;
        self.frame_streams.insert(seat, self.stream_serial);
        self.repaint = true;
        self.stream_serial
    }

    /// The seat's stream `serial` ended. If it was still the seat's newest,
    /// nothing leads that knight any more.
    fn close_frame_stream(&mut self, seat: u32, serial: u64) {
        if self.frame_streams.get(&seat) == Some(&serial) {
            self.frame_streams.remove(&seat);
            self.held.remove(&seat);
            self.seat_rtt_ms.remove(&seat);
        }
    }
}

/// The next run to draw, handed from the host's tick to the painter thread.
#[derive(Default)]
struct Easel {
    run: Option<Run>,
    stop: bool,
}

pub(crate) struct GuestServer {
    address: SocketAddr,
    public_base: Option<String>,
    /// One invitation token per seat; seat `i` plays knight `i + 2`.
    tokens: Vec<String>,
    shooter_incoming: mpsc::Receiver<GuestShooterIntent>,
    published: Arc<Mutex<Published>>,
    easel: Arc<(Mutex<Easel>, Condvar)>,
    /// The raid and tick last handed to the painter.
    drawn: Mutex<Option<(u64, u64)>>,
    stopped: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
    painter: Option<JoinHandle<()>>,
}

impl GuestServer {
    /// A new picture of the realm (RGB, `w`x`h`) for the guest page.
    pub(crate) fn set_realm_picture(&self, rgb: &[u8], w: u32, h: u32) {
        use image::ImageEncoder;
        let mut png = Vec::new();
        if image::codecs::png::PngEncoder::new(&mut png)
            .write_image(rgb, w, h, image::ExtendedColorType::Rgb8)
            .is_ok()
            && let Ok(mut state) = self.published.lock()
        {
            state.realm_png = Some(Arc::new(png));
            state.realm_seq += 1;
        }
    }

    /// A chorus line started: the page plays its recording and subtitles it.
    pub(crate) fn set_voice(&self, said: &super::together_chorus::Said) {
        let dir = super::together_chorus::voices_dir();
        let recorded = super::together_chorus::recording(dir.as_deref(), &said.line).is_some();
        if let Ok(mut state) = self.published.lock() {
            state.voice = Some(serde_json::json!({
                "seq": said.seq,
                "who": said.line.who,
                "id": said.line.id,
                "name": super::together_chorus::name(&said.line.who),
                "words": said.line.words,
                "recorded": recorded,
            }));
        }
    }

    /// Sounds the delve just made, for joined angelX clients.
    pub(crate) fn push_sounds(&self, sounds: &[&'static str]) {
        if sounds.is_empty() {
            return;
        }
        if let Ok(mut state) = self.published.lock() {
            for &sound in sounds {
                state.sound_seq += 1;
                let seq = state.sound_seq;
                state.sounds.push_back((seq, sound));
            }
            while state.sounds.len() > 24 {
                state.sounds.pop_front();
            }
        }
    }

    pub(crate) fn set_realm(&self, realm: RealmHud) {
        if let Ok(mut state) = self.published.lock() {
            state.realm = realm;
        }
    }

    pub(crate) fn drain_gear(&self) -> Vec<(u64, u32, Gear)> {
        self.published
            .lock()
            .map(|mut state| std::mem::take(&mut state.gear))
            .unwrap_or_default()
    }

    /// Where it listens and its seats' tokens (comma-separated), to resume.
    pub(crate) fn saved_invitation(&self) -> (String, String) {
        (self.address.to_string(), self.tokens.join(","))
    }

    /// The player ids of the invited seats.
    pub(crate) fn seats(&self) -> Vec<u32> {
        (0..self.tokens.len() as u32).map(|i| i + 2).collect()
    }

    /// Loopback unless the host explicitly supplies a private/Tailscale address.
    /// Wildcard/public listeners are intentionally not part of this game bridge.
    #[cfg(test)]
    pub(crate) fn start(bind: &str) -> Result<Self, String> {
        Self::start_seats(bind, 1)
    }

    /// A listener with `seats` invitations, one fresh token each.
    pub(crate) fn start_seats(bind: &str, seats: usize) -> Result<Self, String> {
        let mut tokens = Vec::new();
        for _ in 0..seats.clamp(1, MAX_SEATS) {
            let mut random = [0_u8; 16];
            SystemRandom::new()
                .fill(&mut random)
                .map_err(|_| "could not create a secure invitation token")?;
            tokens.push(
                random
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            );
        }
        Self::resume(bind, &tokens.join(","))
    }

    /// Resume a listener with saved tokens (comma-separated, one per seat).
    pub(crate) fn resume(bind: &str, tokens: &str) -> Result<Self, String> {
        let tokens: Vec<String> = tokens.split(',').map(str::to_owned).collect();
        if tokens.is_empty()
            || tokens.len() > MAX_SEATS
            || tokens
                .iter()
                .any(|t| t.len() != 32 || !t.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid saved dungeon invitation".into());
        }
        let public_base = std::env::var("ANGEL_DUNGEON_PUBLIC_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| validate_public_base(&value))
            .transpose()?;
        let address: SocketAddr = if bind.trim().is_empty() {
            "127.0.0.1:0"
        } else {
            bind.trim()
        }
        .parse()
        .map_err(|_| "use a numeric private address and port, such as 127.0.0.1:8787")?;
        if !private_address(address.ip()) {
            return Err("bind a loopback, LAN, or Tailscale address; wildcard/public listeners are not supported".into());
        }
        let listener =
            TcpListener::bind(address).map_err(|e| format!("cannot host dungeon: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let address = listener.local_addr().map_err(|e| e.to_string())?;
        let (shooter_outgoing, shooter_incoming) = mpsc::sync_channel(16);
        let published = Arc::new(Mutex::new(Published {
            seats: (0..tokens.len() as u32).map(|i| i + 2).collect(),
            browser_view: std::env::var("ANGEL_DUNGEON_BROWSER_VIEW").is_ok_and(|v| v == "1"),
            ..Published::default()
        }));
        let easel = Arc::new((Mutex::new(Easel::default()), Condvar::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let thread_state = Arc::clone(&published);
        let thread_stop = Arc::clone(&stopped);
        let thread_tokens = tokens.clone();
        let listener = thread::Builder::new()
            .name("together-guest".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                            if active.fetch_add(1, Ordering::Relaxed) >= MAX_CONNECTIONS {
                                active.fetch_sub(1, Ordering::Relaxed);
                                // Closing overload connections keeps acceptance bounded even
                                // when a peer never reads a response.
                                continue;
                            }
                            let permit = ConnectionPermit(Arc::clone(&active));
                            let state = Arc::clone(&thread_state);
                            let stop = Arc::clone(&thread_stop);
                            let tokens = thread_tokens.clone();
                            let shooter_sender = shooter_outgoing.clone();
                            let _ = thread::Builder::new().name("together-http".into()).spawn(
                                move || {
                                    let held = permit;
                                    let response = handle_request(
                                        &mut stream,
                                        &tokens,
                                        &state,
                                        &shooter_sender,
                                        &stop,
                                    );
                                    let kind = response.stream.clone();
                                    if write_response(&mut stream, response).is_ok()
                                        && let Some(kind) = kind
                                    {
                                        // A stream is not a request: it never
                                        // holds one of the request permits.
                                        drop(held);
                                        match kind {
                                            Stream::Mirror(seat) => {
                                                stream_mirror(&mut stream, seat, &state, &stop)
                                            }
                                            Stream::Frames(seat) => {
                                                stream_frames(&mut stream, seat, &state, &stop)
                                            }
                                            Stream::Socket(seat, _) => socket_session(
                                                stream,
                                                seat,
                                                &state,
                                                &shooter_sender,
                                                &stop,
                                            ),
                                        }
                                    }
                                },
                            );
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("cannot start dungeon listener: {e}"))?;
        let painter_state = Arc::clone(&published);
        let painter_easel = Arc::clone(&easel);
        let painter = thread::Builder::new()
            .name("together-frame".into())
            .spawn(move || paint_frames(&painter_easel, &painter_state));
        let painter = match painter {
            Ok(handle) => handle,
            Err(e) => {
                stopped.store(true, Ordering::Relaxed);
                let _ = listener.join();
                return Err(format!("cannot start dungeon painter: {e}"));
            }
        };
        Ok(Self {
            address,
            public_base,
            tokens,
            shooter_incoming,
            published,
            easel,
            drawn: Mutex::new(None),
            stopped,
            listener: Some(listener),
            painter: Some(painter),
        })
    }

    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }

    /// The link only opens on this machine: a loopback listener with no
    /// configured gateway in front of it.
    pub(crate) fn local_only(&self) -> bool {
        self.public_base.is_none() && self.address.ip().is_loopback()
    }

    /// The first seat's link.
    #[cfg(test)]
    pub(crate) fn invitation_url(&self) -> String {
        self.invitation_urls().remove(0)
    }

    /// Whether the invitation link opens the read-only browser view.
    pub(crate) fn set_browser_view(&self, on: bool) {
        if let Ok(mut state) = self.published.lock() {
            state.browser_view = on;
        }
    }

    pub(crate) fn browser_view(&self) -> bool {
        self.published.lock().is_ok_and(|state| state.browser_view)
    }

    /// One link per seat. Fragments never reach HTTP logs or Referer headers.
    pub(crate) fn invitation_urls(&self) -> Vec<String> {
        let base = self
            .public_base
            .clone()
            .unwrap_or_else(|| format!("http://{}/", self.address));
        self.tokens
            .iter()
            .map(|token| format!("{base}#{token}"))
            .collect()
    }

    pub(crate) fn drain_shooter_intents(&self) -> Vec<GuestShooterIntent> {
        self.shooter_incoming.try_iter().take(16).collect()
    }

    /// Called on every host tick. The HUD is small; the room is redrawn at
    /// most every `FRAME_TICKS` ticks, only when the run moved on (or a seat
    /// just began watching), and only while someone watches a seat's
    /// pictures. The drawing happens on the painter thread.
    pub(crate) fn publish(&self, run: &Run, paused: bool, notice: &str) {
        let hud = Hud::of(run, paused, notice);
        let mut watched = false;
        let mut repaint = false;
        if let Ok(mut state) = self.published.lock() {
            let now = Instant::now();
            state.host_seen = now;
            watched = !state.picture_seats(now).is_empty();
            repaint = std::mem::take(&mut state.repaint);
            for seat in state.seats.clone() {
                let hero = run.players.get(&seat);
                let runes = hero
                    .and_then(|h| h.forged.as_ref())
                    .map_or_else(String::new, |w| w.runes.clone());
                state.forge_runes.insert(seat, runes);
                let playable = !paused && run.active() && hero.is_some_and(|hero| hero.hp > 0);
                state.playable.insert(seat, playable);
            }
            if state.hud.as_ref() != Some(&hud) {
                state.hud = Some(hud);
            }
            let whole = run.mirror_key();
            if state.whole.as_ref().is_none_or(|(k, _)| *k != whole) {
                let json = serde_json::to_vec(run).unwrap_or_default();
                state.whole = Some((whole, Arc::new(json)));
            }
            let from = (run.raid_id, run.tick, paused);
            if state.live_from != Some(from) {
                let json = serde_json::to_vec(&run.live()).unwrap_or_default();
                let seq = state.live.as_ref().map_or(1, |(seq, _)| seq + 1);
                state.live = Some((seq, Arc::new(json)));
                state.live_from = Some(from);
            }
            let key = (run.raid_id, run.book.cards.len());
            if state
                .book
                .as_ref()
                .is_none_or(|(raid, n, _)| (*raid, *n) != key)
            {
                let json = serde_json::to_vec(&run.book).unwrap_or_default();
                state.book = Some((key.0, key.1, Arc::new(json)));
            }
        }
        // Nobody is looking: the painter sleeps, and `drawn` stays behind, so
        // the first publish after someone looks again hands the run over.
        if !watched {
            return;
        }
        let Ok(mut drawn) = self.drawn.lock() else {
            return;
        };
        let due = repaint
            || (*drawn).is_none_or(|(raid, tick)| {
                raid != run.raid_id
                    || (tick != run.tick
                        && (run.tick >= tick + FRAME_TICKS || paused || !run.active()))
            });
        if !due {
            return;
        }
        *drawn = Some((run.raid_id, run.tick));
        let (easel, wake) = &*self.easel;
        if let Ok(mut easel) = easel.lock() {
            easel.run = Some(run.clone());
            wake.notify_one();
        }
    }
}

impl Drop for GuestServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        let (easel, wake) = &*self.easel;
        if let Ok(mut easel) = easel.lock() {
            easel.stop = true;
        }
        wake.notify_all();
        for handle in [self.listener.take(), self.painter.take()]
            .into_iter()
            .flatten()
        {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
pub(crate) fn settlement_snapshot_for_test(run: &Run, notice: &str) -> Vec<u8> {
    let defaults = Published::default();
    let hud = Some(Hud::of(run, false, notice));
    Published { hud, ..defaults }.snapshot(2).unwrap()
}

/// The room as the guest sees it: the host's own pixels, PNG-encoded.
#[cfg(test)]
pub(crate) fn frame_png(run: &Run, seat: u32) -> Vec<u8> {
    let rgb = arena::frame_for(run, FRAME_W as i32, FRAME_H as i32, Some(seat)).rgb_bytes();
    encode_rgb(&rgb, fast_frames())
}

/// Palette PNG of a room. `fast` is the local path: little compression, so
/// the encode finishes inside a frame. A far link asks for `false` and gets
/// the smaller picture.
pub(crate) fn encode_rgb(rgb: &[u8], fast: bool) -> Vec<u8> {
    use image::ImageEncoder;
    if let Some(png) = indexed_png(rgb, fast) {
        return png;
    }
    let mut png = Vec::with_capacity(48 * 1024);
    let (compression, filter) = if fast {
        (
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::NoFilter,
        )
    } else {
        (
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        )
    };
    // Encoding a correctly sized buffer into memory cannot fail. A room has
    // a few dozen colours; the high setting packs it about seven times
    // smaller than the fast default, which matters on a friend's home link.
    let _ = image::codecs::png::PngEncoder::new_with_quality(&mut png, compression, filter)
        .write_image(rgb, FRAME_W, FRAME_H, image::ExtendedColorType::Rgb8);
    png
}

/// A room is drawn from a few dozen colours, so a palette PNG carries it in
/// about a third of the bytes of the RGB one. None when it has more than 256.
fn indexed_png(rgb: &[u8], fast: bool) -> Option<Vec<u8>> {
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut lookup = std::collections::HashMap::<[u8; 3], u8>::new();
    let mut indices = Vec::with_capacity(rgb.len() / 3);
    for pixel in rgb.chunks_exact(3) {
        let colour = [pixel[0], pixel[1], pixel[2]];
        let index = match lookup.get(&colour) {
            Some(&index) => index,
            None => {
                let index = u8::try_from(palette.len()).ok()?;
                palette.push(colour);
                lookup.insert(colour, index);
                index
            }
        };
        indices.push(index);
    }
    let mut png = Vec::with_capacity(16 * 1024);
    let mut encoder = png::Encoder::new(&mut png, FRAME_W, FRAME_H);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette.concat());
    encoder.set_compression(if fast {
        png::Compression::Fast
    } else {
        png::Compression::High
    });
    encoder.set_filter(png::Filter::NoFilter);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&indices).ok()?;
    writer.finish().ok()?;
    Some(png)
}

/// When a burst of samples began, on the clock its sleeps count from. Linux
/// sleeps to an absolute monotonic deadline (TIMER_ABSTIME), so one wake-up's
/// overshoot does not push the next 8.333 ms slot back. Elsewhere the time
/// left to the deadline is slept.
struct Burst {
    #[cfg(target_os = "linux")]
    clock: libc::timespec,
    #[cfg(not(target_os = "linux"))]
    start: Instant,
}

impl Burst {
    fn start() -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut clock = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            // CLOCK_MONOTONIC with a valid pointer does not fail.
            unsafe {
                libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut clock);
            }
            Self { clock }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                start: Instant::now(),
            }
        }
    }

    /// Sleep until `due` after the burst began.
    fn sleep_until(&self, due: Duration) {
        #[cfg(target_os = "linux")]
        {
            let nanos = self.clock.tv_nsec as i128 + due.as_nanos() as i128;
            let deadline = libc::timespec {
                tv_sec: self.clock.tv_sec + nanos.div_euclid(1_000_000_000) as libc::time_t,
                tv_nsec: nanos.rem_euclid(1_000_000_000) as libc::c_long,
            };
            // A signal ends the sleep early; sleep again to the same deadline.
            while unsafe {
                libc::clock_nanosleep(
                    libc::CLOCK_MONOTONIC,
                    libc::TIMER_ABSTIME,
                    &deadline,
                    std::ptr::null_mut(),
                )
            } == libc::EINTR
            {}
        }
        #[cfg(not(target_os = "linux"))]
        {
            let left = (self.start + due).saturating_duration_since(Instant::now());
            if !left.is_zero() {
                thread::sleep(left);
            }
        }
    }
}

/// `ANGEL_DUNGEON_FRAME_TIMING=<file>`: every 120 watched host ticks, one
/// line on how long samples took to paint, how many pictures were published
/// or matched the last one, how evenly the ticks arrived, and how many ticks
/// the painter never saw (a late host loop folds two steps into one publish).
struct PaintLog {
    file: std::fs::File,
    paints_us: Vec<u64>,
    gaps_us: Vec<u64>,
    published: u64,
    same: u64,
    folded: u64,
    last: Option<(Instant, u64, u64)>,
}

impl PaintLog {
    fn from_env() -> Option<Self> {
        let path = std::env::var_os("ANGEL_DUNGEON_FRAME_TIMING").filter(|p| !p.is_empty())?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Self {
            file,
            paints_us: Vec::new(),
            gaps_us: Vec::new(),
            published: 0,
            same: 0,
            folded: 0,
            last: None,
        })
    }

    fn arrived(&mut self, run: &Run) {
        let now = Instant::now();
        if let Some((at, raid, tick)) = self.last
            && raid == run.raid_id
            && run.tick > tick
        {
            self.gaps_us.push(micros(now.saturating_duration_since(at)));
            self.folded += run.tick - tick - 1;
        }
        self.last = Some((now, run.raid_id, run.tick));
        if self.gaps_us.len() >= 120 {
            self.flush();
        }
    }

    fn painted(&mut self, took: Duration, seats: usize, published: usize) {
        self.paints_us.push(micros(took));
        self.published += published as u64;
        self.same += seats.saturating_sub(published) as u64;
    }

    fn flush(&mut self) {
        let pct = |v: &mut Vec<u64>, p: f64| {
            v.sort_unstable();
            v.get(((v.len() as f64 * p) as usize).min(v.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0)
        };
        let (paint_p50, paint_max) = (pct(&mut self.paints_us, 0.5), pct(&mut self.paints_us, 1.0));
        let (gap_p50, gap_p99, gap_max) = (
            pct(&mut self.gaps_us, 0.5),
            pct(&mut self.gaps_us, 0.99),
            pct(&mut self.gaps_us, 1.0),
        );
        let _ = writeln!(
            self.file,
            "paint n={} p50_us={paint_p50} max_us={paint_max} published={} same={} tick_gap_us p50={gap_p50} p99={gap_p99} max={gap_max} folded={}",
            self.paints_us.len(),
            self.published,
            self.same,
            self.folded,
        );
        self.paints_us.clear();
        self.gaps_us.clear();
        self.published = 0;
        self.same = 0;
        self.folded = 0;
    }
}

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// The easel while the painter waits out one sample slot for a late tick.
enum Wait {
    Stop,
    Tick(Run),
    Late,
}

fn wait_for_tick(easel: &(Mutex<Easel>, Condvar), slot: Duration) -> Wait {
    let (easel, wake) = easel;
    let deadline = Instant::now() + slot;
    let Ok(mut easel) = easel.lock() else {
        return Wait::Stop;
    };
    loop {
        if easel.stop {
            return Wait::Stop;
        }
        if let Some(next) = easel.run.take() {
            return Wait::Tick(next);
        }
        let rest = deadline.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Wait::Late;
        }
        easel = match wake.wait_timeout(easel, rest) {
            Ok((guard, _)) => guard,
            Err(_) => return Wait::Stop,
        };
    }
}

/// Paint whatever run is newest; frames the host outpaces are skipped. Only
/// watched seats are drawn, so with nobody looking the painter just waits.
fn paint_frames(easel: &(Mutex<Easel>, Condvar), published: &Mutex<Published>) {
    // The default 50 µs timer slack turns an 8.333 ms sleep into a 9–16 ms
    // one and drops a 120 Hz sample. This thread only paints.
    #[cfg(target_os = "linux")]
    unsafe {
        libc::prctl(libc::PR_SET_TIMERSLACK, 1, 0, 0, 0);
    }
    let (slot, wake) = easel;
    let mut before: Option<Pose> = None;
    let mut seen: std::collections::HashMap<u32, u64> = std::collections::HashMap::new();
    let fast = fast_frames();
    let mut log = PaintLog::from_env();
    // A tick that arrived while the late samples below were waiting.
    let mut pending: Option<Run> = None;
    loop {
        let mut run = if let Some(run) = pending.take() {
            run
        } else {
            let Ok(mut easel) = slot.lock() else {
                return;
            };
            loop {
                if easel.stop {
                    return;
                }
                if let Some(run) = easel.run.take() {
                    break run;
                }
                easel = match wake.wait(easel) {
                    Ok(easel) => easel,
                    Err(_) => return,
                };
            }
        };
        let seats = match published.lock() {
            Ok(state) => state.picture_seats(Instant::now()),
            Err(_) => return,
        };
        if seats.is_empty() {
            before = Some(run.pose());
            if let Some(log) = log.as_mut() {
                log.last = None;
            }
            continue;
        }
        if let Some(log) = log.as_mut() {
            log.arrived(&run);
        }
        let mut sample = |run: &mut Run, pose: Option<&Pose>, alpha: f32, extra: f32| {
            let started = Instant::now();
            let Some(count) = paint_led(run, pose, alpha, extra, &seats, published, &mut seen)
            else {
                return false;
            };
            if let Some(log) = log.as_mut() {
                log.painted(started.elapsed(), seats.len(), count);
            }
            true
        };
        // Samples between the previous pose and this tick, then the tick.
        // A far link draws the halfway point (60 Hz). Local play draws three
        // samples (120 Hz). A picture whose pixels did not change is not
        // published.
        if let Some(pose) = &before {
            let steps: &[f32] = if fast { &LOCAL_ALPHAS } else { &[0.5] };
            let span = if fast { LOCAL_STEP } else { HALF_TICK };
            let burst = Burst::start();
            for (index, alpha) in steps.iter().enumerate() {
                // Blended in place: a copy of the delve per sample does not
                // fit an 8.3 ms frame.
                if !sample(&mut run, Some(pose), *alpha, 0.0) {
                    return;
                }
                burst.sleep_until(span.saturating_mul(u32::try_from(index + 1).unwrap_or(1)));
            }
        }
        if !sample(&mut run, before.as_ref(), 1.0, 0.0) {
            return;
        }
        before = Some(run.pose());
        if !fast {
            continue;
        }
        // The burst fills 25 ms of the 33 ms tick. While the next tick is
        // late, one more sample every 8.333 ms draws the same pose a frame
        // further along each seat's held keys. A wall stops that lead, so a
        // knight pressed against one gives an identical picture, which is
        // not published.
        for n in 1u32..=3 {
            match wait_for_tick(easel, LOCAL_STEP) {
                Wait::Stop => return,
                Wait::Tick(next) => {
                    pending = Some(next);
                    break;
                }
                Wait::Late => {}
            }
            if !sample(&mut run, before.as_ref(), 1.0, n as f32 / 120.0) {
                return;
            }
        }
    }
}

/// One-way delay plus one frame, never more than a quarter second.
/// No measured trip still leads by one frame, so a local key is in the
/// picture before the next 30 Hz tick.
fn paint_lead(rtt_ms: Option<u32>, fast: bool) -> f32 {
    let one_way = rtt_ms.unwrap_or(0) as f32 / 2000.0;
    let frame = if fast { 1.0 / 120.0 } else { 1.0 / 60.0 };
    (one_way + frame).clamp(0.0, 0.25)
}

/// Eight bytes after the paint clock: sprite x, y, w, h, and whether the
/// camera follows. A zero size means the page should not slide.
fn stamp_header(run: &Run, seat: u32) -> [u8; 8] {
    let Some((x, y, w, h, follows)) = arena::focus_stamp_box(run, seat) else {
        return [0; 8];
    };
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&(x.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_be_bytes());
    out[2..4].copy_from_slice(&(y.clamp(i16::MIN as i32, i16::MAX as i32) as i16).to_be_bytes());
    out[4] = w.clamp(0, 255) as u8;
    out[5] = h.clamp(0, 255) as u8;
    out[6] = u8::from(follows);
    out
}

fn note_seat_rtt(state: &mut Published, seat: u32, rtt_ms: u64) {
    let ms = u32::try_from(rtt_ms).unwrap_or(u32::MAX).min(500);
    state.seat_rtt_ms.insert(seat, ms);
}

/// Each picture seat whose fresh held keys move its knight, with how far
/// ahead to draw it. Keys older than the host's lease lead nothing.
fn seat_leads(
    published: &Mutex<Published>,
    seats: &[u32],
    fast: bool,
    extra: f32,
) -> Vec<(u32, Input, f32)> {
    let Ok(state) = published.lock() else {
        return Vec::new();
    };
    let now = Instant::now();
    seats
        .iter()
        .filter_map(|&seat| {
            let (input, at) = *state.held.get(&seat)?;
            if now.saturating_duration_since(at) >= GUEST_LEASE
                || (input.move_x == 0 && input.move_y == 0)
            {
                return None;
            }
            let rtt = state.seat_rtt_ms.get(&seat).copied();
            Some((
                seat,
                input,
                (paint_lead(rtt, fast) + extra).clamp(0.0, 0.25),
            ))
        })
        .collect()
}

/// One sample of every picture seat. A seat holding a direction is drawn
/// where those keys will have taken it, then the run is put back. A still
/// hold (no direction) is not predicted, so an identical picture stays
/// identical and is not published again. Returns how many pictures were
/// published, or None when the publish lock is gone.
fn paint_led(
    run: &mut Run,
    before: Option<&Pose>,
    alpha: f32,
    extra: f32,
    seats: &[u32],
    published: &Mutex<Published>,
    seen: &mut std::collections::HashMap<u32, u64>,
) -> Option<usize> {
    let fast = fast_frames();
    let leads = seat_leads(published, seats, fast, extra);
    let mut frames = Vec::with_capacity(seats.len());
    if leads.is_empty() {
        let mut paint = |sample: &Run| {
            for &seat in seats {
                if let Some(painted) = paint_seat(sample, seat, fast, seen) {
                    frames.push((seat, painted));
                }
            }
        };
        match before.filter(|_| alpha < 1.0) {
            Some(pose) => run.drawn_between(pose, alpha, paint),
            None => paint(run),
        }
    } else {
        for &seat in seats {
            let own = leads.iter().find(|lead| lead.0 == seat).copied();
            let painted = match before {
                Some(pose) => run.drawn_ahead(pose, alpha, own, |sample| {
                    paint_seat(sample, seat, fast, seen)
                }),
                None => paint_seat(run, seat, fast, seen),
            };
            if let Some(painted) = painted {
                frames.push((seat, painted));
            }
        }
    }
    publish_pictures(published, frames)
}

struct Painted {
    rgb: Arc<Vec<u8>>,
    stamp: [u8; 8],
    fast: bool,
}

/// One seat's picture of `run`, or None when its pixels match the picture
/// last published for that seat. Local frames keep the fractional pixel as
/// dither, so a step smaller than a pixel still changes the picture; a far
/// link draws the crisp integer picture.
fn paint_seat(
    run: &Run,
    seat: u32,
    fast: bool,
    seen: &mut std::collections::HashMap<u32, u64>,
) -> Option<Painted> {
    let img = if fast {
        arena::frame_for_live(run, FRAME_W as i32, FRAME_H as i32, Some(seat))
    } else {
        arena::frame_for(run, FRAME_W as i32, FRAME_H as i32, Some(seat))
    };
    let hash = img.content_hash();
    if seen.get(&seat) == Some(&hash) {
        return None;
    }
    seen.insert(seat, hash);
    Some(Painted {
        rgb: Arc::new(img.rgb_bytes()),
        stamp: stamp_header(run, seat),
        fast,
    })
}

/// Swap the newest pictures in under one short lock. Nothing is encoded or
/// compared while it is held; the pictures they replace are freed after.
fn publish_pictures(published: &Mutex<Published>, frames: Vec<(u32, Painted)>) -> Option<usize> {
    if frames.is_empty() {
        return Some(0);
    }
    let painted_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let count = frames.len();
    let mut replaced = Vec::with_capacity(count);
    {
        let mut state = published.lock().ok()?;
        state.frame_seq += 1;
        let seq = state.frame_seq;
        for (seat, painted) in frames {
            let picture = Picture {
                seq,
                painted_ms,
                stamp: painted.stamp,
                rgb: painted.rgb,
                png: None,
                fast: painted.fast,
            };
            replaced.push(state.pictures.insert(seat, picture));
        }
    }
    drop(replaced);
    Some(count)
}

/// The picture as a whole PNG: the one a reader already encoded, or encoded
/// now outside the lock and kept for the next reader of the same picture.
fn whole_png(published: &Mutex<Published>, seat: u32, picture: &Picture) -> Arc<Vec<u8>> {
    if let Some(png) = &picture.png {
        return Arc::clone(png);
    }
    let png = Arc::new(encode_rgb(&picture.rgb, picture.fast));
    if let Ok(mut state) = published.lock()
        && let Some(newest) = state.pictures.get_mut(&seat)
        && newest.seq == picture.seq
    {
        newest.png = Some(Arc::clone(&png));
    }
    png
}

/// What a socket sends for its seat's newest picture.
#[derive(Debug, PartialEq)]
enum Wire {
    /// The page's picture already has these pixels: nothing to send.
    Same,
    /// The changed tiles, as a whole socket message against the page's picture.
    Patch(Vec<u8>),
    /// A whole PNG.
    Whole,
}

/// A patch only against the picture the page holds (`held`: its frame and
/// pixels), never against one it may not have. A new session, a page that
/// asked for a whole picture, a change too wide for tiles, and a run of
/// `HEAL_AFTER` patches go whole. A far patch must also beat the last whole
/// PNG (`whole_len`) by its header.
fn socket_wire(
    picture: &Picture,
    held: Option<&(u64, Arc<Vec<u8>>)>,
    whole_len: usize,
    patches: u32,
) -> Wire {
    let Some((base, prev)) = held else {
        return Wire::Whole;
    };
    if patches >= HEAL_AFTER {
        return Wire::Whole;
    }
    let budget = if picture.fast {
        LOCAL_PATCH_TILES
    } else {
        far_tile_cap(whole_len)
    };
    match tile_patch(prev, &picture.rgb, budget) {
        Tiles::Same => Wire::Same,
        Tiles::Changed(tiles) if picture.fast || tiles.len() + 32 < whole_len => {
            let mut wire = Vec::with_capacity(33 + tiles.len());
            wire.push(SOCKET_PATCH);
            wire.extend_from_slice(&picture.seq.to_be_bytes());
            wire.extend_from_slice(&picture.painted_ms.to_be_bytes());
            wire.extend_from_slice(&picture.stamp);
            wire.extend_from_slice(&base.to_be_bytes());
            wire.extend_from_slice(&tiles);
            Wire::Patch(wire)
        }
        Tiles::Changed(_) | Tiles::Wide => Wire::Whole,
    }
}

/// The socket message for a whole picture.
fn whole_wire(picture: &Picture, png: &[u8]) -> Vec<u8> {
    let mut wire = Vec::with_capacity(25 + png.len());
    wire.push(SOCKET_FRAME);
    wire.extend_from_slice(&picture.seq.to_be_bytes());
    wire.extend_from_slice(&picture.painted_ms.to_be_bytes());
    wire.extend_from_slice(&picture.stamp);
    wire.extend_from_slice(png);
    wire
}

/// How many changed tiles a far link may send: what fits in the last packed
/// picture at a few hundred bytes an indexed tile, and never more than the
/// page takes. The byte check in `socket_wire` still refuses a patch that
/// grew past that picture.
fn far_tile_cap(last_png: usize) -> usize {
    // count + palette-count, a 4-colour palette, one tile header, indices.
    const TILE_BYTES: usize = 4 + 4 * 3 + 2 + 16 * 16;
    (last_png.saturating_sub(32) / TILE_BYTES).min(MAX_PATCH_TILES)
}

const TILE: usize = 16;
// A tile's column and row each travel as one byte.
const _: () = assert!(FRAME_W as usize / TILE <= 256 && FRAME_H as usize / TILE <= 256);

enum Tiles {
    /// No tile changed.
    Same,
    /// The patch body (see `tile_patch`).
    Changed(Vec<u8>),
    /// More tiles changed than the budget, the changed ink needs more than
    /// 256 colours, or the buffers are not two frames.
    Wide,
}

/// Changed 16×16 tiles of `next` against `prev`, indexed against one palette.
/// At most `max_tiles`, and never more than `MAX_PATCH_TILES`.
///
/// Body, after the socket header: `u16` count, `u16` colours, the RGB
/// palette, then each tile as `col`, `row` and 256 indices. A 4-colour tile
/// is 274 bytes. The page expands the indices back to RGBA.
fn tile_patch(prev: &[u8], next: &[u8], max_tiles: usize) -> Tiles {
    let w = FRAME_W as usize;
    let h = FRAME_H as usize;
    let max_tiles = max_tiles.min(MAX_PATCH_TILES);
    if prev.len() != w * h * 3 || next.len() != prev.len() || w % TILE != 0 || h % TILE != 0 {
        return Tiles::Wide;
    }
    let mut dirty = Vec::new();
    for row in 0..(h / TILE) {
        for col in 0..(w / TILE) {
            let changed = (0..TILE).any(|y| {
                let start = ((row * TILE + y) * w + col * TILE) * 3;
                prev[start..start + TILE * 3] != next[start..start + TILE * 3]
            });
            if changed {
                dirty.push((col, row));
                if dirty.len() > max_tiles {
                    return Tiles::Wide;
                }
            }
        }
    }
    if dirty.is_empty() {
        return Tiles::Same;
    }
    let mut lookup = std::collections::HashMap::<[u8; 3], u8>::new();
    let mut palette = Vec::<[u8; 3]>::new();
    let mut indices = Vec::with_capacity(dirty.len() * TILE * TILE);
    for &(col, row) in &dirty {
        for y in 0..TILE {
            let start = ((row * TILE + y) * w + col * TILE) * 3;
            for pixel in next[start..start + TILE * 3].chunks_exact(3) {
                let colour = [pixel[0], pixel[1], pixel[2]];
                let index = match lookup.get(&colour) {
                    Some(&index) => index,
                    None => {
                        let Ok(index) = u8::try_from(palette.len()) else {
                            return Tiles::Wide;
                        };
                        lookup.insert(colour, index);
                        palette.push(colour);
                        index
                    }
                };
                indices.push(index);
            }
        }
    }
    let mut out = Vec::with_capacity(4 + palette.len() * 3 + dirty.len() * (2 + TILE * TILE));
    out.extend_from_slice(&(dirty.len() as u16).to_be_bytes());
    out.extend_from_slice(&(palette.len() as u16).to_be_bytes());
    for colour in &palette {
        out.extend_from_slice(colour);
    }
    for (&(col, row), tile) in dirty.iter().zip(indices.chunks_exact(TILE * TILE)) {
        out.push(col as u8);
        out.push(row as u8);
        out.extend_from_slice(tile);
    }
    Tiles::Changed(out)
}

/// One seat's mirror of the delve, for as long as the friend stays: the
/// whole run, then each tick's changes, one JSON object per line
/// (`{"whole":…}` or `{"live":…}`). A newer stream for the seat ends this
/// one; so does a friend who stops reading.
fn stream_mirror(
    stream: &mut TcpStream,
    seat: u32,
    published: &Mutex<Published>,
    stop: &AtomicBool,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let serial = {
        let Ok(mut state) = published.lock() else {
            return;
        };
        state.stream_serial += 1;
        let serial = state.stream_serial;
        state.streams.insert(seat, serial);
        serial
    };
    let mut sent_whole = None;
    let mut sent_live = 0u64;
    let mut out = Vec::with_capacity(8 * 1024);
    while !stop.load(Ordering::Relaxed) {
        out.clear();
        {
            let Ok(state) = published.lock() else {
                break;
            };
            if state.streams.get(&seat) != Some(&serial) {
                return;
            }
            if let Some((key, json)) = &state.whole
                && sent_whole != Some(*key)
            {
                out.extend_from_slice(b"{\"whole\":");
                out.extend_from_slice(json);
                out.extend_from_slice(b"}\n");
                sent_whole = Some(*key);
                sent_live = 0;
            }
            if let Some((seq, json)) = &state.live
                && *seq != sent_live
            {
                out.extend_from_slice(b"{\"live\":");
                out.extend_from_slice(json);
                out.extend_from_slice(b"}\n");
                sent_live = *seq;
            }
        }
        if out.is_empty() {
            thread::sleep(Duration::from_millis(3));
            continue;
        }
        if stream
            .write_all(&out)
            .and_then(|()| stream.flush())
            .is_err()
        {
            break;
        }
    }
    if let Ok(mut state) = published.lock()
        && state.streams.get(&seat) == Some(&serial)
    {
        state.streams.remove(&seat);
    }
}

/// Frames a browser seat may have on the way before it says it got them:
/// enough to cover the round trip at the host's frame rate, fewer when the
/// receipts slow down (frames queueing on a thin link).
const FRAME_WINDOW_MIN: usize = 3;
const FRAME_WINDOW_MAX: usize = 16;
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// The frames a browser seat has on the way, and how many it may have.
/// The window follows the receipts' round trip: its floor (the path's own
/// delay, refreshed every ten seconds) sets how many frames cover it, and
/// a smoothed trip of twice the floor and more means frames are queueing,
/// so the window shrinks. Receipts through a tunnel come back unevenly, so
/// only a clear rise counts.
struct FrameWindow {
    on_the_way: std::collections::VecDeque<(u64, Instant)>,
    stalled: Instant,
    window: usize,
    floor: Option<(Duration, Instant)>,
    smoothed: Option<Duration>,
    adjusted: Instant,
}

impl FrameWindow {
    fn new() -> Self {
        Self {
            on_the_way: Default::default(),
            stalled: Instant::now(),
            window: 4,
            floor: None,
            smoothed: None,
            adjusted: Instant::now(),
        }
    }

    /// The seat has every frame up to `acked`.
    fn acked(&mut self, acked: u64) {
        let mut trip = None;
        while self.on_the_way.front().is_some_and(|&(seq, _)| seq <= acked) {
            trip = self.on_the_way.pop_front().map(|(_, at)| at.elapsed());
            self.stalled = Instant::now();
        }
        let Some(trip) = trip else {
            return;
        };
        if self
            .floor
            .is_none_or(|(least, at)| trip < least || at.elapsed() > Duration::from_secs(10))
        {
            self.floor = Some((trip, Instant::now()));
        }
        self.smoothed = Some(self.smoothed.map_or(trip, |s| (s * 7 + trip) / 8));
        if self.adjusted.elapsed() > Duration::from_millis(250)
            && let (Some((least, _)), Some(smooth)) = (self.floor, self.smoothed)
        {
            self.adjusted = Instant::now();
            let wanted = (least.as_millis() / FRAME_INTERVAL.as_millis()) as usize + 2;
            let queueing = smooth > least * 2 + Duration::from_millis(100);
            self.window = if queueing {
                self.window.saturating_sub(1)
            } else if self.window < wanted {
                self.window + 1
            } else if self.window > wanted {
                self.window - 1
            } else {
                self.window
            }
            .clamp(FRAME_WINDOW_MIN, FRAME_WINDOW_MAX);
        }
    }

    /// Room for another frame. Receipts lost for two seconds start the
    /// window over.
    fn open(&mut self) -> bool {
        if self.on_the_way.len() >= self.window && self.stalled.elapsed() > Duration::from_secs(2)
        {
            self.on_the_way.clear();
        }
        self.on_the_way.len() < self.window
    }

    fn sent(&mut self, seq: u64) {
        if self.on_the_way.is_empty() {
            self.stalled = Instant::now();
        }
        self.on_the_way.push_back((seq, Instant::now()));
    }
}

/// A browser seat's frames, pushed as the painter finishes each one: per
/// frame a 4-byte length, the 8-byte frame number (both big-endian), then
/// the PNG. Polling costs a round trip a frame, which a gateway far away
/// turns into uneven, skipped frames; a push keeps the host's cadence. The
/// page acknowledges each frame (`POST /frames/ack`), and only a window of
/// them goes unacknowledged: an unbounded push fills the buffers along a
/// slow link and the friend watches seconds-old frames.
fn stream_frames(
    stream: &mut TcpStream,
    seat: u32,
    published: &Mutex<Published>,
    stop: &AtomicBool,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let serial = {
        let Ok(mut state) = published.lock() else {
            return;
        };
        state.open_frame_stream(seat)
    };
    let mut sent = None;
    let mut window = FrameWindow::new();
    let mut out = Vec::with_capacity(16 * 1024);
    while !stop.load(Ordering::Relaxed) {
        let picture = {
            let Ok(state) = published.lock() else {
                break;
            };
            if state.frame_streams.get(&seat) != Some(&serial) {
                return;
            }
            window.acked(state.frame_acks.get(&seat).copied().unwrap_or(0));
            if window.open() {
                state
                    .pictures
                    .get(&seat)
                    .filter(|picture| sent != Some(picture.seq))
                    .cloned()
            } else {
                None
            }
        };
        let Some(picture) = picture else {
            thread::sleep(Duration::from_millis(3));
            continue;
        };
        // An HTTP page takes no tiles: every frame is a whole, current PNG.
        let png = whole_png(published, seat, &picture);
        out.clear();
        out.extend_from_slice(&(png.len() as u32).to_be_bytes());
        out.extend_from_slice(&picture.seq.to_be_bytes());
        out.extend_from_slice(&png);
        if stream
            .write_all(&out)
            .and_then(|()| stream.flush())
            .is_err()
        {
            break;
        }
        sent = Some(picture.seq);
        window.sent(picture.seq);
    }
    if let Ok(mut state) = published.lock() {
        state.close_frame_stream(seat, serial);
    }
}

struct ConnectionPermit(Arc<AtomicUsize>);
impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

fn private_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || (ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
    }
}

/// This machine's Tailscale address, if it is on a tailnet: the route to
/// Tailscale's own resolver leaves through the tailnet interface. No packet
/// is sent.
pub(crate) fn tailnet_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // Tailscale's own resolver, the same quad-100 address on every tailnet.
    socket
        .connect(SocketAddr::new(
            IpAddr::V4(std::net::Ipv4Addr::new(100, 100, 100, 100)),
            53,
        ))
        .ok()?;
    let ip = socket.local_addr().ok()?.ip();
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        .then_some(ip)
}

/// Where a hosted delve listens unless told otherwise: the tailnet, then the
/// local network, then this machine only. The invitation's token guards it.
pub(crate) fn default_bind(port: u16) -> String {
    let ip = tailnet_address()
        .or_else(lan_address)
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    SocketAddr::new(ip, port).to_string()
}

/// This machine's private address on its local network, if it has one. No
/// packet is sent: connecting a UDP socket only consults the routing table.
pub(crate) fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (private_address(ip) && !ip.is_loopback()).then_some(ip)
}

fn validate_public_base(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw.trim())
        .map_err(|_| "ANGEL_DUNGEON_PUBLIC_URL must be an http(s) root URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("ANGEL_DUNGEON_PUBLIC_URL must be an http(s) root URL without credentials, a query, or a fragment".into());
    }
    Ok(url.to_string())
}

struct Request {
    method: String,
    path: String,
    authorization: String,
    content_type: String,
    body: Vec<u8>,
    /// `Sec-WebSocket-Key` of an `Upgrade: websocket` request.
    socket_key: Option<String>,
    /// `Sec-WebSocket-Protocol`: a browser can't set a socket's
    /// Authorization header, so the page offers its token as a subprotocol.
    protocols: String,
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    /// Which frame a PNG is, so the page never fetches the same one twice.
    frame_seq: Option<u64>,
    /// After the headers the connection stays open and carries this seat's
    /// mirror (`GET /stream`, see `stream_mirror`) or its frames (`GET
    /// /frames`, see `stream_frames`).
    stream: Option<Stream>,
}

#[derive(Clone)]
enum Stream {
    Mirror(u32),
    Frames(u32),
    /// `GET /play` upgraded to a WebSocket, with its accept key.
    Socket(u32, String),
}

impl Response {
    fn json(status: u16, message: &str) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body: serde_json::to_vec(&serde_json::json!({"message":message})).unwrap(),
            frame_seq: None,
            stream: None,
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Result<Request, Response> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::with_capacity(1024);
    let header_end = loop {
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() >= MAX_HEADER || Instant::now() >= deadline {
            return Err(Response::json(
                431,
                "request headers are too large or too slow",
            ));
        }
        let mut chunk = [0; 512];
        let count = stream
            .read(&mut chunk)
            .map_err(|_| Response::json(408, "request timed out"))?;
        if count == 0 {
            return Err(Response::json(400, "incomplete request"));
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    if header_end > MAX_HEADER {
        return Err(Response::json(431, "request headers are too large"));
    }
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| Response::json(400, "invalid headers"))?;
    let mut lines = headers.split("\r\n");
    let start: Vec<_> = lines.next().unwrap_or("").split(' ').collect();
    if start.len() != 3 || !matches!(start[2], "HTTP/1.0" | "HTTP/1.1") {
        return Err(Response::json(400, "invalid request line"));
    }
    let method = start[0].to_owned();
    let path = start[1].to_owned();
    let mut authorization = None;
    let mut content_type = None;
    let mut length = None;
    let mut upgrade = false;
    let mut socket_key = None;
    let mut protocols = String::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Response::json(400, "invalid header"))?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(Response::json(400, "transfer encoding is not supported"));
        }
        if name.eq_ignore_ascii_case("authorization")
            && authorization.replace(value.to_owned()).is_some()
        {
            return Err(Response::json(400, "duplicate authorization"));
        }
        if name.eq_ignore_ascii_case("content-type")
            && content_type.replace(value.to_owned()).is_some()
        {
            return Err(Response::json(400, "duplicate content type"));
        }
        if name.eq_ignore_ascii_case("upgrade") {
            upgrade = value.eq_ignore_ascii_case("websocket");
        }
        if name.eq_ignore_ascii_case("sec-websocket-key") {
            socket_key = Some(value.to_owned());
        }
        if name.eq_ignore_ascii_case("sec-websocket-protocol") {
            if !protocols.is_empty() {
                protocols.push(',');
            }
            protocols.push_str(value);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(Response::json(400, "duplicate content length"));
            }
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| Response::json(400, "invalid content length"))?,
            );
        }
    }
    let length = length.unwrap_or(0);
    let limit = match path.as_str() {
        "/forge" => super::together_forge::MAX_BYTES,
        "/card" => super::together_shooter::cards::MAX_BYTES,
        "/wish" | "/grant" | "/reforge" => 512,
        "/avatar" => super::together_avatar::MAX_BYTES,
        _ => MAX_BODY,
    };
    if length > limit {
        return Err(Response::json(413, "request is too large"));
    }
    while bytes.len() < header_end + length {
        if Instant::now() >= deadline {
            return Err(Response::json(408, "request timed out"));
        }
        let mut chunk = [0; 512];
        let remaining = (header_end + length - bytes.len()).min(chunk.len());
        let count = stream
            .read(&mut chunk[..remaining])
            .map_err(|_| Response::json(408, "request timed out"))?;
        if count == 0 {
            return Err(Response::json(400, "incomplete action"));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(Request {
        method,
        path,
        authorization: authorization.unwrap_or_default(),
        content_type: content_type.unwrap_or_default(),
        body: bytes[header_end..header_end + length].to_vec(),
        socket_key: socket_key.filter(|_| upgrade),
        protocols,
    })
}

fn handle_request(
    stream: &mut TcpStream,
    tokens: &[String],
    published: &Mutex<Published>,
    shooter_outgoing: &mpsc::SyncSender<GuestShooterIntent>,
    stopped: &AtomicBool,
) -> Response {
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(response) => return response,
    };
    if stopped.load(Ordering::Relaxed) {
        return Response::json(410, "host closed this invitation");
    }
    // A query only busts caches; it never selects anything.
    let route = request
        .path
        .split_once('?')
        .map_or(request.path.as_str(), |(path, _)| path);
    // Frames and their receipts share the frame budget.
    let frame = (request.method == "GET" && route == "/frame.png")
        || (request.method == "POST" && route == "/frames/ack");
    let Ok(mut state) = published.lock() else {
        return Response::json(503, "host unavailable");
    };
    if !(if frame {
        state.allow_frame()
    } else {
        state.allow_request()
    }) {
        return Response::json(429, "please slow down");
    }
    if request.method == "GET" && request.path == "/" {
        let (content_type, page) = if state.browser_view {
            ("text/html; charset=utf-8", BROWSER_VIEW)
        } else {
            ("text/plain; charset=utf-8", CLIENT)
        };
        return Response {
            status: 200,
            content_type,
            body: page.as_bytes().to_vec(),
            frame_seq: None,
            stream: None,
        };
    }
    // A socket's token rides in its subprotocols, beside `delve.v1`.
    let offered = request
        .protocols
        .split(',')
        .map(str::trim)
        .find(|p| p.len() == 32 && p.bytes().all(|b| b.is_ascii_hexdigit()))
        .filter(|_| request.socket_key.is_some())
        .unwrap_or("");
    let supplied = request
        .authorization
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .unwrap_or(offered);
    let equal = |token: &str| {
        supplied.len() == token.len()
            && supplied
                .bytes()
                .zip(token.bytes())
                .fold(0_u8, |different, (a, b)| different | (a ^ b))
                == 0
    };
    // Every token is compared in full, so timing never says which seat matched.
    let mut seat = None;
    for (i, token) in tokens.iter().enumerate() {
        if equal(token) {
            seat = Some(i as u32 + 2);
        }
    }
    let Some(seat) = seat else {
        return Response::json(401, "open the complete invitation link from your host");
    };
    state.saw_seat(seat, Instant::now());
    match (request.method.as_str(), route) {
        ("GET", "/forge/rules") => Response {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: super::together_forge::rules(
                super::together_forge::START_LOC,
                state.forge_runes.get(&seat).map_or("", String::as_str),
            )
            .into_bytes(),
            frame_seq: None,
            stream: None,
        },
        ("GET", "/stream") => Response {
            status: 200,
            content_type: "application/x-ndjson",
            body: Vec::new(),
            frame_seq: None,
            stream: Some(Stream::Mirror(seat)),
        },
        ("POST", "/frames/ack") => {
            let Some(seq) = std::str::from_utf8(&request.body)
                .ok()
                .and_then(|text| text.trim().parse::<u64>().ok())
            else {
                return Response::json(400, "send the frame number");
            };
            let acked = state.frame_acks.entry(seat).or_insert(0);
            *acked = (*acked).max(seq);
            Response::json(202, "ok")
        }
        ("GET", "/frames") => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: Vec::new(),
            frame_seq: None,
            stream: Some(Stream::Frames(seat)),
        },
        ("GET", "/book") => match &state.book {
            Some((_, _, json)) => Response {
                status: 200,
                content_type: "application/json",
                body: json.to_vec(),
                frame_seq: None,
                stream: None,
            },
            None => Response::json(409, "no active delve"),
        },
        ("GET", "/card/rules") => Response {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: super::together_shooter::cards::rules().into_bytes(),
            frame_seq: None,
            stream: None,
        },
        ("GET", "/avatar/instructions") => Response {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: super::together_avatar::instructions("").into_bytes(),
            frame_seq: None,
            stream: None,
        },
        ("GET", "/phrasebook") => Response {
            status: 200,
            content_type: "text/plain; charset=utf-8",
            body: super::together_shooter::phrasebook::text().into_bytes(),
            frame_seq: None,
            stream: None,
        },
        ("POST", "/learn") => {
            if state.gear.len() >= 4 {
                return Response::json(429, "the host is busy; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            #[derive(serde::Deserialize)]
            struct Ask {
                words: String,
            }
            let Ok(ask) = serde_json::from_slice::<Ask>(&request.body) else {
                return Response::json(400, "send {words}");
            };
            let words: String = ask
                .words
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect();
            if words.trim().chars().filter(|c| c.is_alphanumeric()).count() < 3 {
                return Response::json(400, "say the wish in a few words");
            }
            state
                .gear
                .push((raid_id, seat, Gear::Learn(words.trim().to_string())));
            Response::json(202, "The scroll is learning it; keep playing.")
        }
        ("POST", "/boon") => {
            if state.gear.len() >= 4 {
                return Response::json(429, "the host is busy; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            #[derive(serde::Deserialize)]
            struct Ask {
                wish: String,
            }
            let Ok(ask) = serde_json::from_slice::<Ask>(&request.body) else {
                return Response::json(400, "send {wish}");
            };
            if super::together_shooter::phrasebook::get(&ask.wish).is_none() {
                return Response::json(400, "the phrasebook has no such wish");
            }
            state.gear.push((raid_id, seat, Gear::Boon(ask.wish)));
            Response::json(202, "Wished.")
        }
        ("POST", "/reforge") => {
            if state.gear.len() >= 4 {
                return Response::json(429, "the host is busy; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            #[derive(serde::Deserialize)]
            struct Ask {
                part: String,
                words: String,
            }
            let Ok(ask) = serde_json::from_slice::<Ask>(&request.body) else {
                return Response::json(400, "send {part, words}");
            };
            let Some(part) = super::together_shooter::knights::Part::from_word(&ask.part) else {
                return Response::json(400, "part is offense or defense");
            };
            let words: String = ask
                .words
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect();
            if words.trim().chars().filter(|c| c.is_alphanumeric()).count() < 3 {
                return Response::json(400, "say what you want in a few words");
            }
            state
                .gear
                .push((raid_id, seat, Gear::Reforge(part, words.trim().to_string())));
            Response::json(
                202,
                "Sent to your host's angelX: watch for your reforge to land.",
            )
        }
        ("POST", "/wish" | "/grant") => {
            if state.gear.len() >= 4 {
                return Response::json(429, "the host is busy; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            let Ok(text) = std::str::from_utf8(&request.body) else {
                return Response::json(400, "send plain text");
            };
            let text: String = text
                .chars()
                .filter(|c| !c.is_control())
                .take(super::together_realm::MAX_WORDS)
                .collect();
            if text.trim().is_empty() {
                return Response::json(400, "say what you wish for");
            }
            if route == "/wish" {
                state
                    .gear
                    .push((raid_id, seat, Gear::Wish(text.trim().to_string())));
                Response::json(
                    202,
                    "Your wish is on the wishing stone. Your host's angelX will draft it; the party pays for it with spoils from the delve.",
                )
            } else {
                state
                    .gear
                    .push((raid_id, seat, Gear::Grant(text.trim().to_string())));
                Response::json(
                    202,
                    "Sent to your host: if the treasury covers it, it rises in the realm.",
                )
            }
        }
        ("POST", "/forge" | "/avatar" | "/card") => {
            if state.gear.len() >= 4 {
                return Response::json(429, "gear queue full; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            let (gear, notes) = if route == "/card" {
                let Ok(text) = std::str::from_utf8(&request.body) else {
                    return Response::json(400, "a card must be UTF-8 text");
                };
                match super::together_shooter::cards::check("", text) {
                    Ok(mut checked) => {
                        // A friend's card never replaces one of the host's.
                        checked.card.id = format!("p{seat}-{}", checked.card.id);
                        (Gear::Card(checked.card), checked.notes)
                    }
                    Err(error) => return Response::json(400, &error.fix_it()),
                }
            } else if route == "/forge" {
                let Ok(text) = std::str::from_utf8(&request.body) else {
                    return Response::json(400, "runes must be UTF-8 text");
                };
                match super::together_forge::check(text, super::together_forge::START_LOC) {
                    Ok(checked) => (Gear::Weapon(checked.weapon), checked.notes),
                    Err(error) => return Response::json(400, &error.fix_it()),
                }
            } else {
                match super::together_avatar::check(&request.body) {
                    Ok(checked) => (Gear::Avatar(checked.avatar), checked.notes),
                    Err(errors) => {
                        return Response::json(400, &super::together_avatar::fix_it(&errors));
                    }
                }
            };
            state.gear.push((raid_id, seat, gear));
            Response::json(
                202,
                &format!("Checked and queued for player {seat}. {}", notes.join("; ")),
            )
        }
        ("POST", "/hello") => {
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            let name: String = std::str::from_utf8(&request.body)
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_control())
                .take(32)
                .collect();
            let name = if name.trim().is_empty() {
                format!("Friend {}", seat - 1)
            } else {
                name.trim().to_string()
            };
            state.gear.push((raid_id, seat, Gear::Hello(name)));
            Response::json(202, &format!("Welcome — you play knight {seat}."))
        }
        ("POST", "/reforge-card") => {
            if state.gear.len() >= 8 {
                return Response::json(429, "the host is busy; try again shortly");
            }
            let Some(raid_id) = state.hud.as_ref().map(|h| h.raid_id) else {
                return Response::json(409, "no active delve");
            };
            #[derive(serde::Deserialize)]
            struct Forged {
                part: String,
                card: String,
            }
            let Ok(forged) = serde_json::from_slice::<Forged>(&request.body) else {
                return Response::json(400, "send {part, card}");
            };
            let Some(part) = super::together_shooter::knights::Part::from_word(&forged.part) else {
                return Response::json(400, "part is offense or defense");
            };
            match super::together_shooter::cards::check("", &forged.card) {
                Ok(mut checked) => {
                    checked.card.id = format!("p{seat}-{}", checked.card.id);
                    let name = checked.card.name.clone();
                    state
                        .gear
                        .push((raid_id, seat, Gear::ReforgeCard(part, checked.card)));
                    Response::json(
                        202,
                        &format!("{name} checked; your host equips it in the Sanctuary."),
                    )
                }
                Err(error) => Response::json(400, &error.fix_it()),
            }
        }
        ("GET", "/state") => match state.snapshot(seat) {
            Some(body) => Response {
                status: 200,
                content_type: "application/json; charset=utf-8",
                body,
                frame_seq: None,
                stream: None,
            },
            None => Response::json(503, "the host is preparing the dungeon"),
        },
        ("GET", voice) if voice.starts_with("/voice/") => {
            let mut parts = voice
                .trim_start_matches("/voice/")
                .trim_end_matches(".mp3")
                .splitn(2, '/');
            let line = super::together_chorus::Line {
                who: parts.next().unwrap_or("").into(),
                cue: String::new(),
                id: parts.next().unwrap_or("").into(),
                words: String::new(),
            };
            let dir = super::together_chorus::voices_dir();
            match super::together_chorus::recording(dir.as_deref(), &line)
                .and_then(|f| std::fs::read(f).ok())
            {
                Some(body) => Response {
                    status: 200,
                    content_type: "audio/mpeg",
                    body,
                    frame_seq: None,
                    stream: None,
                },
                None => Response::json(404, "no such voice"),
            }
        }
        ("GET", "/realm.png") => match state.realm_png.clone() {
            Some(png) => Response {
                status: 200,
                content_type: "image/png",
                body: Vec::clone(&png),
                frame_seq: None,
                stream: None,
            },
            None => Response::json(503, "the host has not drawn the realm yet"),
        },
        ("GET", "/frame.png") => {
            // The newest picture, encoded at most once however many polls
            // ask for it, and never while the lock is held.
            let Some(picture) = state.pictures.get(&seat).cloned() else {
                return Response::json(503, "the host has not drawn the room yet");
            };
            drop(state);
            let png = whole_png(published, seat, &picture);
            Response {
                status: 200,
                content_type: "image/png",
                body: Vec::clone(&png),
                frame_seq: Some(picture.seq),
                stream: None,
            }
        }
        ("POST", "/shooter/input") => {
            if request.content_type.split(';').next() != Some("application/json") {
                return Response::json(415, "send JSON controls");
            }
            let submission: ShooterSubmission = match serde_json::from_slice(&request.body) {
                Ok(value) => value,
                Err(_) => return Response::json(400, "invalid shooter controls"),
            };
            let (status, message) = submit_controls(&mut state, seat, submission, shooter_outgoing);
            Response::json(status, message)
        }
        ("GET", "/play") => match &request.socket_key {
            Some(key) => Response {
                status: 101,
                content_type: "",
                body: Vec::new(),
                frame_seq: None,
                stream: Some(Stream::Socket(seat, socket_accept(key))),
            },
            None => Response::json(400, "open /play as a WebSocket"),
        },
        _ => Response::json(404, "game route not found"),
    }
}

/// A seat's held controls, from `POST /shooter/input` or its socket: the
/// status and message the friend gets back.
fn submit_controls(
    state: &mut Published,
    seat: u32,
    submission: ShooterSubmission,
    shooter_outgoing: &mpsc::SyncSender<GuestShooterIntent>,
) -> (u16, &'static str) {
    state.roll_window();
    let input = submission.input;
    if !input.valid() {
        return (400, "control axes must be -1, 0, or 1");
    }
    let Some(raid_id) = state.hud.as_ref().map(|hud| hud.raid_id) else {
        return (409, "the host has not opened the dungeon yet");
    };
    if submission.raid_id != raid_id {
        return (409, "the delve changed; refresh your controls");
    }
    if state.last_shooter_submission.get(&seat) == Some(&submission) {
        return (202, "controls already received");
    }
    if submission.sequence == 0
        || submission.sequence >= (1_u64 << 53)
        || state
            .last_shooter_submission
            .get(&seat)
            .is_some_and(|last| submission.sequence <= last.sequence)
    {
        return (409, "reconnect to synchronize your controls");
    }
    // A release remains valid while paused, fallen, or ending a delve.
    if !state.playable.get(&seat).copied().unwrap_or(false) && input != Input::default() {
        return (409, "wait for the host to resume the delve");
    }
    if state.shooter_inputs >= 24 * state.seats.len() as u32 {
        return (429, "please slow down");
    }
    let intent_at = Instant::now();
    let intent = GuestShooterIntent {
        player: seat,
        raid_id: submission.raid_id,
        input,
        received_at: intent_at,
    };
    if shooter_outgoing.try_send(intent).is_err() {
        return (503, "the host control queue is busy");
    }
    state.shooter_inputs += 1;
    state.held.insert(seat, (input, intent_at));
    state.last_shooter_submission.insert(seat, submission);
    (202, "controls sent to host")
}

/// `Sec-WebSocket-Accept` for a client's key (RFC 6455).
fn socket_accept(key: &str) -> String {
    use base64::Engine;
    let digest = ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{}258EAFA5-E914-47DA-95CA-C5AB0DC85B11", key.trim()).as_bytes(),
    );
    base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
}

/// What a socket carries to the page: one byte saying which, then the rest.
const SOCKET_FRAME: u8 = 1;
const SOCKET_STATE: u8 = 2;
const SOCKET_NOTE: u8 = 3;
/// Changed 16×16 tiles against the picture the page holds (see `tile_patch`).
const SOCKET_PATCH: u8 = 4;
/// The largest message a page sends (controls, receipts, pings).
const SOCKET_MAX_IN: usize = 4096;

fn socket_write(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => out.push(n as u8),
        n if n < 65536 => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    stream.write_all(&out)
}

/// One whole message from the page (masked, as RFC 6455 has clients send).
fn socket_read(stream: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let bad = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, why.to_string());
    let mut message = Vec::new();
    let mut kind = None;
    loop {
        let mut head = [0u8; 2];
        stream.read_exact(&mut head)?;
        let (fin, opcode) = (head[0] & 0x80 != 0, head[0] & 0x0f);
        if head[1] & 0x80 == 0 {
            return Err(bad("unmasked client frame"));
        }
        let length = match head[1] & 0x7f {
            126 => {
                let mut n = [0u8; 2];
                stream.read_exact(&mut n)?;
                u16::from_be_bytes(n) as usize
            }
            127 => return Err(bad("message too large")),
            n => n as usize,
        };
        if message.len() + length > SOCKET_MAX_IN {
            return Err(bad("message too large"));
        }
        let mut mask = [0u8; 4];
        stream.read_exact(&mut mask)?;
        let mut payload = vec![0u8; length];
        stream.read_exact(&mut payload)?;
        for (i, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[i % 4];
        }
        // Control frames may arrive between a message's parts.
        if opcode >= 0x8 {
            return Ok((opcode, payload));
        }
        if opcode != 0 {
            kind = Some(opcode);
        }
        message.extend_from_slice(&payload);
        if fin {
            return Ok((kind.unwrap_or(0x1), message));
        }
    }
}

/// A browser seat's one connection, for as long as the friend stays: the
/// host sends frames (under the frame window), the seat's state ten times
/// a second, and answers; the page sends held controls, frame receipts and
/// pings. Nothing waits on a request of its own, so a far or tunnelled
/// friend pays the path's delay once, not a connection's setup per key.
fn socket_session(
    mut stream: TcpStream,
    seat: u32,
    published: &Arc<Mutex<Published>>,
    shooter_outgoing: &mpsc::SyncSender<GuestShooterIntent>,
    stop: &Arc<AtomicBool>,
) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Ok(mut reader) = stream.try_clone() else {
        return;
    };
    // The page pings twice a second; a minute of silence is a lost friend.
    let _ = reader.set_read_timeout(Some(Duration::from_secs(60)));
    let serial = {
        let Ok(mut state) = published.lock() else {
            return;
        };
        state.open_frame_stream(seat)
    };
    let closed = Arc::new(AtomicBool::new(false));
    // The page lost its picture (a hidden tab, a failed decode, a patch it
    // could not place) and asked for a whole one.
    let rekey = Arc::new(AtomicBool::new(false));
    let (notes, outbox) = mpsc::channel::<(u8, Vec<u8>)>();
    let incoming = {
        let published = Arc::clone(published);
        let closed = Arc::clone(&closed);
        let rekey = Arc::clone(&rekey);
        let outgoing = shooter_outgoing.clone();
        thread::Builder::new()
            .name("together-socket".into())
            .spawn(move || {
                let mut budget = (Instant::now(), 0u32);
                while let Ok((opcode, payload)) = socket_read(&mut reader) {
                    match opcode {
                        0x8 => break,
                        0x9 => {
                            let _ = notes.send((0xA, payload));
                            continue;
                        }
                        0x1 | 0x2 => {}
                        _ => continue,
                    }
                    if budget.0.elapsed() >= Duration::from_secs(1) {
                        budget = (Instant::now(), 0);
                    }
                    budget.1 += 1;
                    if budget.1 > 400 {
                        break;
                    }
                    let Ok(message) = serde_json::from_slice::<serde_json::Value>(&payload) else {
                        continue;
                    };
                    if let Some(seq) = message.get("ack").and_then(serde_json::Value::as_u64) {
                        if let Ok(mut state) = published.lock() {
                            let acked = state.frame_acks.entry(seat).or_insert(0);
                            *acked = (*acked).max(seq);
                        }
                    } else if message.get("keyframe").is_some() {
                        rekey.store(true, Ordering::Relaxed);
                    } else if let Some(ping) = message.get("ping") {
                        if let Some(rtt) = message.get("rtt").and_then(serde_json::Value::as_u64) {
                            if let Ok(mut state) = published.lock() {
                                note_seat_rtt(&mut state, seat, rtt);
                            }
                        }
                        let pong = serde_json::json!({ "pong": ping });
                        let mut note = vec![SOCKET_NOTE];
                        note.extend_from_slice(pong.to_string().as_bytes());
                        let _ = notes.send((0x2, note));
                    } else if message.get("input").is_some() {
                        let Ok(submission) = serde_json::from_value::<ShooterSubmission>(message)
                        else {
                            continue;
                        };
                        let Ok(mut state) = published.lock() else {
                            break;
                        };
                        let (status, why) =
                            submit_controls(&mut state, seat, submission, &outgoing);
                        if status >= 400 {
                            let next = state
                                .last_shooter_submission
                                .get(&seat)
                                .map_or(1, |s| s.sequence + 1);
                            let note = serde_json::json!({
                                "input_error": why, "status": status, "next_sequence": next,
                            });
                            let mut bytes = vec![SOCKET_NOTE];
                            bytes.extend_from_slice(note.to_string().as_bytes());
                            let _ = notes.send((0x2, bytes));
                        }
                    }
                }
                closed.store(true, Ordering::Relaxed);
            })
    };
    if incoming.is_err() {
        return;
    }
    let mut window = FrameWindow::new();
    let mut sent = None;
    // The picture the page holds, its frame and pixels. Patches are built
    // against it and nothing else, so a frame the window skipped, or one
    // the page dropped and said so, never leaves it without a base.
    let mut held: Option<(u64, Arc<Vec<u8>>)> = None;
    // The last whole picture's size, which a far patch must beat, and the
    // patches sent since it.
    let mut whole_len = 0usize;
    let mut patches = 0u32;
    let mut told: Option<Instant> = None;
    let mut out = Vec::with_capacity(16 * 1024);
    'session: while !stop.load(Ordering::Relaxed) && !closed.load(Ordering::Relaxed) {
        while let Ok((opcode, payload)) = outbox.try_recv() {
            if socket_write(&mut stream, opcode, &payload).is_err() {
                break 'session;
            }
        }
        if rekey.swap(false, Ordering::Relaxed) {
            sent = None;
            held = None;
        }
        let (picture, snapshot) = {
            let Ok(state) = published.lock() else {
                break;
            };
            if state.frame_streams.get(&seat) != Some(&serial) {
                break;
            }
            window.acked(state.frame_acks.get(&seat).copied().unwrap_or(0));
            let picture = if window.open() {
                state
                    .pictures
                    .get(&seat)
                    .filter(|picture| sent != Some(picture.seq))
                    .cloned()
            } else {
                None
            };
            let snapshot = if told.is_none_or(|at| at.elapsed() >= Duration::from_millis(100)) {
                state.snapshot(seat)
            } else {
                None
            };
            (picture, snapshot)
        };
        // The picture goes out before the state note. A full TCP window
        // should stall on the frame the friend is waiting for, not on a
        // 2 KB snapshot written ahead of it.
        let mut wrote = false;
        if let Some(picture) = picture {
            let wire = match socket_wire(&picture, held.as_ref(), whole_len, patches) {
                Wire::Same => None,
                Wire::Patch(wire) => {
                    patches += 1;
                    Some(wire)
                }
                Wire::Whole => {
                    let png = whole_png(published, seat, &picture);
                    whole_len = png.len();
                    patches = 0;
                    Some(whole_wire(&picture, &png))
                }
            };
            sent = Some(picture.seq);
            if let Some(wire) = wire {
                if socket_write(&mut stream, 0x2, &wire).is_err() {
                    break;
                }
                held = Some((picture.seq, picture.rgb));
                window.sent(picture.seq);
                wrote = true;
            }
        }
        if let Some(json) = snapshot {
            told = Some(Instant::now());
            out.clear();
            out.push(SOCKET_STATE);
            out.extend_from_slice(&json);
            if socket_write(&mut stream, 0x2, &out).is_err() {
                break;
            }
        }
        if !wrote {
            // A far link's frames are 16 ms apart, so a 2 ms poll is enough.
            // Local frames are 8 ms apart; a 2 ms poll can miss one and the
            // next publish overwrites it, which shows up as a long gap.
            thread::sleep(if fast_frames() {
                Duration::from_micros(200)
            } else {
                Duration::from_millis(2)
            });
        }
    }
    let _ = socket_write(&mut stream, 0x8, &[]);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    if let Ok(mut state) = published.lock() {
        state.close_frame_stream(seat, serial);
    }
}

fn write_response(stream: &mut TcpStream, response: Response) -> std::io::Result<()> {
    if let Some(Stream::Socket(_, accept)) = &response.stream {
        return write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\nSec-WebSocket-Protocol: delve.v1\r\n\r\n"
        );
    }
    let reason = match response.status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        _ => "Service Unavailable",
    };
    let frame = response
        .frame_seq
        .map_or_else(String::new, |seq| format!("X-Frame-Seq: {seq}\r\n"));
    // A stream has no length: it lasts until either side closes it.
    let length = if response.stream.is_some() {
        String::new()
    } else {
        format!("Content-Length: {}\r\n", response.body.len())
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\n{length}{frame}Connection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; img-src 'self' blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n\r\n",
        response.status, reason, response.content_type,
    )?;
    stream.write_all(&response.body)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_guest__tests.rs"]
mod tests;
