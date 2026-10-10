//! Invited friends' windows onto the dungeon delve: one seat per invitation,
//! players 2 to 4. The terminal event loop owns the run; this listener serves
//! the host's own rendering of it as a PNG (each seat's camera on its own
//! knight), a compact HUD, and queues each seat's held controls. A friend's
//! own angelX (`/dungeon join`) draws an admitted host mirror. The browser is
//! a read-only authenticated pixel/status view; neither client awards defeats.

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
/// With the browser view on, the same link opens a read-only view of the
/// host's floor instead. Controls still come through the native client.
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
/// Simulation ticks between frames: 15 frames a second at 30 Hz.
pub(crate) const FRAME_TICKS: u64 = 2;
/// The host publishes on every terminal tick, idle ones (200 ms) included.
const HOST_SILENCE: Duration = Duration::from_secs(2);
/// Frame fetches have their own budget beside the 48 other requests a second.
const FRAMES_PER_SECOND: u32 = 20;

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
                    knight: hero
                        .knight
                        .as_deref()
                        .and_then(super::together_shooter::knights::knight)
                        .map_or_else(String::new, |k| k.name.to_string()),
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
    stream_serial: u64,
    playable: std::collections::BTreeMap<u32, bool>,
    host_seen: Instant,
    frame_seq: u64,
    /// Each seat's view of the room, its camera on its own knight.
    views: std::collections::BTreeMap<u32, Arc<Vec<u8>>>,
    last_shooter_submission: std::collections::BTreeMap<u32, ShooterSubmission>,
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
            stream_serial: 0,
            playable: Default::default(),
            host_seen: Instant::now(),
            frame_seq: 0,
            views: Default::default(),
            last_shooter_submission: Default::default(),
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
                                    let mirror = response.stream;
                                    if write_response(&mut stream, response).is_ok()
                                        && let Some(seat) = mirror
                                    {
                                        // A stream is not a request: it never
                                        // holds one of the request permits.
                                        drop(held);
                                        stream_mirror(&mut stream, seat, &state, &stop);
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
    /// most every `FRAME_TICKS` ticks and only when the run moved on, and the
    /// drawing and PNG encoding happen on the painter thread.
    pub(crate) fn publish(&self, run: &Run, paused: bool, notice: &str) {
        // Projection is owner-local even before a friend takes a seat. The
        // guest painter clones this sanitized run; serde(skip) alone would
        // not protect a pixel frame.
        let private_free;
        let run = if run.chivalry.is_some() {
            private_free = {
                let mut r = run.clone();
                r.chivalry = None;
                r
            };
            &private_free
        } else {
            run
        };
        let hud = Hud::of(run, paused, notice);
        if let Ok(mut state) = self.published.lock() {
            state.host_seen = Instant::now();
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
        let Ok(mut drawn) = self.drawn.lock() else {
            return;
        };
        let due = (*drawn).is_none_or(|(raid, tick)| {
            raid != run.raid_id
                || (tick != run.tick && (run.tick >= tick + FRAME_TICKS || paused || !run.active()))
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
pub(crate) fn frame_png(run: &Run, seat: u32) -> Vec<u8> {
    use image::ImageEncoder;
    // Each seat's camera follows its own knight.
    let rgb = arena::frame_for(run, FRAME_W as i32, FRAME_H as i32, Some(seat)).rgb_bytes();
    let mut png = Vec::with_capacity(48 * 1024);
    // Encoding a correctly sized buffer into memory cannot fail.
    let _ = image::codecs::png::PngEncoder::new(&mut png).write_image(
        &rgb,
        FRAME_W,
        FRAME_H,
        image::ExtendedColorType::Rgb8,
    );
    png
}

/// Paint whatever run is newest; frames the host outpaces are skipped.
fn paint_frames(easel: &(Mutex<Easel>, Condvar), published: &Mutex<Published>) {
    let (slot, wake) = easel;
    loop {
        let run = {
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
        // A friend's own angelX draws from its mirror: pictures are only
        // for seats without a stream (one still connecting, or on pictures).
        let seats: Vec<u32> = match published.lock() {
            Ok(state) => state
                .seats
                .iter()
                .copied()
                .filter(|seat| !state.streams.contains_key(seat))
                .collect(),
            Err(_) => return,
        };
        let frames: Vec<(u32, Arc<Vec<u8>>)> = seats
            .into_iter()
            .map(|seat| (seat, Arc::new(frame_png(&run, seat))))
            .collect();
        let Ok(mut state) = published.lock() else {
            return;
        };
        state.frame_seq += 1;
        state.views.extend(frames);
    }
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
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    /// Which frame a PNG is, so the page never fetches the same one twice.
    frame_seq: Option<u64>,
    /// `GET /stream`: after the headers the connection stays open and
    /// carries this seat's mirror of the delve (see `stream_mirror`).
    stream: Option<u32>,
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
    let frame = request.method == "GET" && route == "/frame.png";
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
    let supplied = request.authorization.strip_prefix("Bearer ").unwrap_or("");
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
            stream: Some(seat),
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
            let Some(png) = state.views.get(&seat).cloned() else {
                return Response::json(503, "the host has not drawn the room yet");
            };
            let frame_seq = state.frame_seq;
            drop(state);
            Response {
                status: 200,
                content_type: "image/png",
                body: Vec::clone(&png),
                frame_seq: Some(frame_seq),
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
            let input = submission.input;
            if !input.valid() {
                return Response::json(400, "control axes must be -1, 0, or 1");
            }
            let Some(raid_id) = state.hud.as_ref().map(|hud| hud.raid_id) else {
                return Response::json(409, "the host has not opened the dungeon yet");
            };
            if submission.raid_id != raid_id {
                return Response::json(409, "the delve changed; refresh your controls");
            }
            if state.last_shooter_submission.get(&seat) == Some(&submission) {
                return Response::json(202, "controls already received");
            }
            if submission.sequence == 0
                || submission.sequence >= (1_u64 << 53)
                || state
                    .last_shooter_submission
                    .get(&seat)
                    .is_some_and(|last| submission.sequence <= last.sequence)
            {
                return Response::json(409, "reconnect to synchronize your controls");
            }
            // A release remains valid while paused, fallen, or ending a delve.
            if !state.playable.get(&seat).copied().unwrap_or(false) && input != Input::default() {
                return Response::json(409, "wait for the host to resume the delve");
            }
            if state.shooter_inputs >= 24 * state.seats.len() as u32 {
                return Response::json(429, "please slow down");
            }
            let intent = GuestShooterIntent {
                player: seat,
                raid_id: submission.raid_id,
                input,
                received_at: Instant::now(),
            };
            if shooter_outgoing.try_send(intent).is_err() {
                return Response::json(503, "the host control queue is busy");
            }
            state.shooter_inputs += 1;
            state.last_shooter_submission.insert(seat, submission);
            Response::json(202, "controls sent to host")
        }
        _ => Response::json(404, "game route not found"),
    }
}

fn write_response(stream: &mut TcpStream, response: Response) -> std::io::Result<()> {
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
