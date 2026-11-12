//! The terminal's own seats in a delve, each one a client of the delve's
//! server as every friend's is (`docs/ENGINE.md`, "The host's own screen
//! is a client too"). The host's knight and a couch partner's play on
//! in-process connections to this angelX's own server; a friend's angelX
//! plays on a socket to its host's. Either way the seat's client (the
//! Delve's own, through angel-serve's `NativeLink`) samples the keys into
//! commands, predicts its own knight, and draws the seat's own screen; the
//! terminal only shows that picture.
//!
//! Nothing here blocks the thread that draws the terminal. The terminal's
//! thread samples the seat's commands at the command rate with nothing
//! drawn (`tick`), never waiting for a frame being drawn elsewhere; the
//! seat's frames are drawn on a render worker while the room is shown
//! (`frame`), sending no commands of their own.
//! The keys the terminal hears go in at once, in order, without waiting
//! either: a tap shorter than a tick is a press of its own.

use super::together_gear;
use super::together_shooter::{Input, Run};
use angel_net::input::button;
use angel_serve::LocalConn;
use angel_serve::client::{LinkOptions, LinkStatus, NativeLink};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The Delve's client: the same one a browser runs as wasm.
pub type Client = angel_delve::client::DelveClient;

/// How long a new seat waits for its welcome and the run before play
/// begins without them.
const WELCOME_WAIT: Duration = Duration::from_secs(2);

#[path = "together_seat/keys.rs"]
mod keys;
use keys::Key;

/// A seat's newest picture: `px` is `w * h` words, `0xAARRGGBB`.
#[derive(Clone)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub px: Arc<Vec<u32>>,
}

impl Picture {
    /// As the renderer's image, for the terminal's picture path.
    pub fn img(&self) -> angel_delve::render::overworld::Img {
        angel_delve::render::overworld::Img::from_words(
            self.w as i32,
            self.h as i32,
            self.px.to_vec(),
        )
    }

    /// The pixel at (x, y), black outside.
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        if x >= self.w || y >= self.h {
            return [0, 0, 0];
        }
        let p = self.px[(y * self.w + x) as usize];
        [(p >> 16) as u8, (p >> 8) as u8, p as u8]
    }
}

struct Inner {
    link: NativeLink<Client>,
    cached_run: Option<(u32, Arc<Run>)>,
}

#[derive(Clone)]
struct CachedState {
    status: LinkStatus,
    knight: Option<u32>,
    ballot_open: Option<bool>,
    menu_open: Option<bool>,
    run: Option<Arc<Run>>,
    media_origin: Option<(String, angel_serve::client::Trust)>,
}

/// One seat: its client and link, the keys waiting for them, and the
/// one-off actions waiting to go.
struct Core {
    inner: Mutex<Inner>,
    lines: Mutex<Vec<String>>,
    picture: Mutex<Option<Picture>>,
    state: Mutex<CachedState>,
    keys: Mutex<Vec<Key>>,
    /// Taps waiting, each its own press: one goes to the sampler once the
    /// one before has been sampled, so their order is the order they came.
    taps: Mutex<VecDeque<u32>>,
    /// Actions in the order they were made, a larger one in pieces, sent no
    /// faster than `together_gear::PIECE_EVERY`.
    outbox: Mutex<VecDeque<(u32, Vec<u8>)>>,
    sent: Mutex<Option<Instant>>,
    epoch: Instant,
    /// Thrown things in the air on the newest frame, for the GPU window to
    /// lift (`ui::glass::lifted`); empty in a terminal.
    lifted: Mutex<Arc<Vec<angel_delve::render::overworld::arena::Lifted>>>,
    /// The newest frame in parts, for the GPU window to draw
    /// (`ui::glass::stage`); `None` in a terminal.
    scene: Mutex<Option<Arc<angel_delve::render::overworld::arena::RoomScene>>>,
}

/// One seat, shared by the terminal's thread and the render workers.
#[derive(Clone)]
pub struct Seat {
    core: Arc<Core>,
    /// The seat's own render worker, for pictures painted in half blocks
    /// (Kitty's go through the viewer's).
    worker: Arc<std::sync::OnceLock<crate::game_pipeline::Pipeline>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Seat {
    pub fn media_origin(&self) -> Option<(String, angel_serve::client::Trust)> {
        if let Ok(inner) = self.core.inner.try_lock() {
            return inner
                .link
                .remote()
                .map(|(url, trust)| (url.to_owned(), trust.clone()));
        }
        lock(&self.core.state).media_origin.clone()
    }
    fn of(link: NativeLink<Client>) -> Seat {
        let status = link.status().clone();
        let media_origin = link
            .remote()
            .map(|(url, trust)| (url.to_owned(), trust.clone()));
        let seat = Seat {
            core: Arc::new(Core {
                inner: Mutex::new(Inner {
                    link,
                    cached_run: None,
                }),
                lines: Mutex::new(Vec::new()),
                picture: Mutex::new(None),
                state: Mutex::new(CachedState {
                    status,
                    knight: None,
                    ballot_open: Some(false),
                    menu_open: Some(false),
                    run: None,
                    media_origin,
                }),
                keys: Mutex::new(Vec::new()),
                taps: Mutex::new(VecDeque::new()),
                outbox: Mutex::new(VecDeque::new()),
                sent: Mutex::new(None),
                epoch: Instant::now(),
                lifted: Mutex::new(Arc::new(Vec::new())),
                scene: Mutex::new(None),
            }),
            worker: Arc::new(std::sync::OnceLock::new()),
        };
        {
            let mut inner = lock(&seat.core.inner);
            seat.core.sync_state(&mut inner);
        }
        seat
    }

    /// A seat on this angelX's own server, waiting (a moment at most) for
    /// its welcome and the run, so its first keys are its knight's.
    pub fn local(conn: LocalConn, name: &str) -> Seat {
        Seat::local_with(conn, name, LinkOptions::default())
    }

    /// A place on this angelX's own server that only watches: a host who
    /// plays no knight, whose seat is a friend's.
    pub fn watching(conn: LocalConn, name: &str) -> Seat {
        let options = LinkOptions { caps: angel_net::proto::caps::WATCH, ..LinkOptions::default() };
        Seat::local_with(conn, name, options)
    }

    fn local_with(conn: LocalConn, name: &str, options: LinkOptions) -> Seat {
        let seat = Seat::of(NativeLink::local(conn, name, options));
        let until = Instant::now() + WELCOME_WAIT;
        // No command goes before the run is in: the first is sampled at the
        // seat's first tick, with its knight to play.
        loop {
            let mut inner = lock(&seat.core.inner);
            let now_ms = seat.core.now_ms(Instant::now());
            inner.link.poll(now_ms);
            seat.core.sync_state(&mut inner);
            let ready = inner
                .link
                .runtime()
                .game()
                .is_some_and(|g| g.newest().is_some());
            drop(inner);
            if ready || Instant::now() >= until {
                return seat;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// A seat in a friend's delve, from their share link
    /// (`https://castle.ts.net/#KEY`, `http://192.168.1.5:8790/#KEY`).
    pub fn connect(link: &str, name: &str) -> Result<Seat, String> {
        let link = NativeLink::connect(link, name, LinkOptions::default())?;
        // The host's paintings are fetched from beside its socket.
        if let Some((url, trust)) = link.remote() {
            crate::services::paint_host(url, trust);
        }
        Ok(Seat::of(link))
    }

    fn key(&self, key: Key) {
        keys::push(&mut lock(&self.core.keys), key);
    }

    /// Everything held now (`angel_net::input::button`): a key held and let
    /// go between two frames is still pressed for a command.
    pub fn set_held(&self, bits: u32) {
        self.key(Key::Held(bits));
    }

    /// A press and its release at once: a key the client takes as a press
    /// (the ballot's, the menu's). Taps go one a command, in order: two in
    /// one command would be taken in the client's order, not theirs.
    pub fn tap(&self, bits: u32) {
        if bits != 0 {
            lock(&self.core.taps).push_back(bits);
        }
    }

    /// Taps of `bits` not yet given to the sampler, taken back: a menu
    /// opened and left before its tap went out never holds the knight.
    pub fn untap(&self, bits: u32) {
        lock(&self.core.taps).retain(|&tap| tap != bits);
    }

    /// Let go of everything (the terminal lost focus, a menu took the keys).
    pub fn release_all(&self) {
        self.key(Key::Release);
    }

    /// A one-off action (`together_gear`): in pieces when it is large.
    pub fn action(&self, kind: u32, body: &[u8]) {
        lock(&self.core.outbox).extend(together_gear::pieces(kind, body));
    }

    /// One display frame at `now`: the keys in, what arrived taken, and
    /// the seat's own screen drawn. The commands are `tick`'s, on the
    /// terminal's own thread.
    pub fn frame(&self, now: Instant) -> Option<Picture> {
        self.core.frame(now)
    }

    /// The seat's next frame, drawn at `now` on its own render worker; the
    /// newest finished one is `picture`. Only the newest asked for is drawn.
    pub fn request(&self, now: Instant) {
        let worker = self
            .worker
            .get_or_init(crate::game_pipeline::Pipeline::new);
        let core = Arc::clone(&self.core);
        let key = now.saturating_duration_since(self.core.epoch).as_micros() as u64;
        worker.request(
            key,
            Box::new(move || {
                core.frame(now);
                Some(Vec::new())
            }),
        );
    }

    /// The newest frame in parts (the GPU window's), drawn at `now` on the
    /// seat's own render worker, waiting at most `budget` for it; the one
    /// before stands in while the worker catches up.
    pub fn draw_scene(
        &self,
        now: Instant,
        budget: std::time::Duration,
    ) -> Option<Arc<angel_delve::render::overworld::arena::RoomScene>> {
        let worker = self
            .worker
            .get_or_init(crate::game_pipeline::Pipeline::new);
        let core = Arc::clone(&self.core);
        let key = now.saturating_duration_since(self.core.epoch).as_micros() as u64;
        worker.request(
            key,
            Box::new(move || {
                core.frame(now);
                Some(Vec::new())
            }),
        );
        let _ = worker.wait(key, budget);
        lock(&self.core.scene).clone()
    }

    /// Whether a frame in parts has been drawn yet.
    pub fn has_scene(&self) -> bool {
        lock(&self.core.scene).is_some()
    }

    /// Thrown things in the air on the newest frame drawn, in its pixels.
    pub fn lifted(&self) -> Arc<Vec<angel_delve::render::overworld::arena::Lifted>> {
        Arc::clone(&lock(&self.core.lifted))
    }

    /// The seat's next frame as the renderer's image, for the terminal's
    /// picture path to make on its own worker.
    pub fn drawer(
        &self,
    ) -> impl FnOnce(Instant) -> angel_delve::render::overworld::Img + Send + 'static {
        let core = Arc::clone(&self.core);
        move |now| {
            let (w, h) = angel_delve::render::overworld::arena::seat_size();
            core.frame(now).map_or_else(
                || angel_delve::render::overworld::Img::black(w as i32, h as i32),
                |p| p.img(),
            )
        }
    }

    /// The keys in, what arrived taken, the commands and actions due sent,
    /// with nothing drawn: the seat plays at the command rate whether or
    /// not its room is on screen (its frames are drawn on a render worker
    /// when it is). A frame being drawn elsewhere is never waited for: it
    /// does the same.
    pub fn set_knob(&self, name: &str, value: f64) -> bool {
        let Ok(mut inner) = self.core.inner.lock() else { return false };
        inner.link.set_knob(name, value).is_some()
    }

    pub fn tick(&self, now: Instant) {
        let core = &self.core;
        let Ok(mut inner) = core.inner.try_lock() else {
            return;
        };
        core.feed(&mut inner, now);
        let now_ms = core.now_ms(now);
        inner.link.tick(now_ms);
        core.sync_state(&mut inner);
    }

    /// The seat's player has left the game's screen: what arrived taken,
    /// pings sent and no commands (`NativeLink::idle`), so the host's server
    /// counts the seat away and keeps its knight out of harm's way.
    pub fn idle(&self, now: Instant) {
        let core = &self.core;
        let Ok(mut inner) = core.inner.try_lock() else {
            return;
        };
        core.feed(&mut inner, now);
        let now_ms = core.now_ms(now);
        inner.link.idle(now_ms);
        core.sync_state(&mut inner);
    }

    /// Whether the seat's ballot is open, as its client shows it (at once,
    /// before the run says so); `None` while keys wait to be sampled, so
    /// the client has not taken them yet.
    pub fn ballot_open(&self) -> Option<bool> {
        if !lock(&self.core.keys).is_empty() || !lock(&self.core.taps).is_empty() {
            return None;
        }
        if let Ok(mut inner) = self.core.inner.try_lock() {
            if inner.link.hands().peek().pressed != 0 {
                return None;
            }
            return Some(inner.link.runtime().game().is_some_and(Client::ballot_open));
        }
        lock(&self.core.state).ballot_open
    }

    /// The activity menu after this client's pending keys have been sampled.
    pub fn menu_open(&self) -> Option<bool> {
        if !lock(&self.core.keys).is_empty() || !lock(&self.core.taps).is_empty() {
            return None;
        }
        if let Ok(mut inner) = self.core.inner.try_lock() {
            if inner.link.hands().peek().pressed != 0 {
                return None;
            }
            return Some(inner.link.runtime().game().is_some_and(Client::menu_open));
        }
        lock(&self.core.state).menu_open
    }

    /// Lines the seat's client made (`sfx <name>`, `say <name>: <words>`,
    /// `notice <words>`, `music <track>`) since the last look.
    pub fn take_lines(&self) -> Vec<String> {
        if let Ok(mut inner) = self.core.inner.try_lock() {
            let lines = inner.link.take_lines();
            if !lines.is_empty() {
                lock(&self.core.lines).extend(lines);
            }
        }
        std::mem::take(&mut *lock(&self.core.lines))
    }

    /// The last picture drawn.
    pub fn picture(&self) -> Option<Picture> {
        lock(&self.core.picture).clone()
    }

    pub fn status(&self) -> LinkStatus {
        if let Ok(inner) = self.core.inner.try_lock() {
            return inner.link.status().clone();
        }
        lock(&self.core.state).status.clone()
    }

    /// The knight this seat plays, once seated.
    pub fn knight(&self) -> Option<u32> {
        if let Ok(inner) = self.core.inner.try_lock() {
            return inner
                .link
                .runtime()
                .seat()
                .map(angel_delve::wire::knight_of);
        }
        lock(&self.core.state).knight
    }

    /// The run as this seat's client holds it: the newest tick it has.
    pub fn run(&self) -> Option<Run> {
        self.run_arc().map(|run| (*run).clone())
    }

    /// Bytes this seat's socket has read and written since it joined.
    pub fn socket_bytes(&self) -> (u64, u64) {
        lock(&self.core.inner).link.bytes()
    }

    /// Shared reference to the newest run this seat's client holds, avoiding
    /// a deep clone when the snapshot tick has not changed.
    pub fn run_arc(&self) -> Option<Arc<Run>> {
        if let Ok(mut inner) = self.core.inner.try_lock() {
            self.core.sync_state(&mut inner);
            if let Some(run) = &lock(&self.core.state).run {
                return Some(Arc::clone(run));
            }
            let Inner { link, cached_run } = &mut *inner;
            return link
                .runtime()
                .game()
                .and_then(|client| {
                    let tick = client.newest_tick()?;
                    if let Some((prev_tick, cached)) = cached_run.as_ref()
                        && *prev_tick == tick
                    {
                        return Some(Arc::clone(cached));
                    }
                    let arc = Arc::new(client.newest()?.clone());
                    *cached_run = Some((tick, Arc::clone(&arc)));
                    Some(arc)
                });
        }
        lock(&self.core.state).run.clone()
    }

    /// Leave on purpose: the seat is freed.
    pub fn bye(&self) {
        lock(&self.core.inner).link.bye();
    }
}

impl Core {
    fn now_ms(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.epoch).as_secs_f64() * 1000.0
    }

    fn sync_state(&self, inner: &mut Inner) {
        let lines = inner.link.take_lines();
        if !lines.is_empty() {
            lock(&self.lines).extend(lines);
        }
        let pressed = inner.link.hands().peek().pressed != 0;
        let runtime = inner.link.runtime();
        let game = runtime.game();
        let ballot_open = (!pressed).then(|| game.is_some_and(Client::ballot_open));
        let menu_open = (!pressed).then(|| game.is_some_and(Client::menu_open));
        let status = inner.link.status().clone();
        let knight = runtime.seat().map(angel_delve::wire::knight_of);
        let remote = inner.link.remote();
        let media_origin = remote.map(|(url, trust)| (url.to_owned(), trust.clone()));
        let run = if remote.is_some() {
            game.and_then(|client| {
                let tick = client.newest_tick()?;
                if let Some((cached_tick, cached)) = &inner.cached_run
                    && *cached_tick == tick
                {
                    return Some(Arc::clone(cached));
                }
                let arc = Arc::new(client.newest()?.clone());
                inner.cached_run = Some((tick, Arc::clone(&arc)));
                Some(arc)
            })
        } else {
            None
        };
        *lock(&self.state) = CachedState {
            status,
            knight,
            ballot_open,
            menu_open,
            run,
            media_origin,
        };
    }

    /// The keys heard since the last frame, into the sampler, in order;
    /// the actions due, sent.
    fn feed(&self, inner: &mut Inner, now: Instant) {
        let keys = std::mem::take(&mut *lock(&self.keys));
        for key in keys {
            let hands = inner.link.hands();
            match key {
                Key::Held(bits) => hands.set_held(bits),
                Key::Release => {
                    // Something let go is said at once; nothing held needs no word.
                    let was = hands.peek();
                    if was.held != 0 || was.pressed != 0 {
                        let now_ms = self.now_ms(now);
                        inner.link.release_all(now_ms);
                    } else {
                        hands.release_all();
                    }
                }
            }
        }
        let hands = inner.link.hands();
        if hands.peek().pressed == 0
            && let Some(bits) = lock(&self.taps).pop_front()
        {
            hands.down(bits);
            hands.up(bits);
        }
        let mut sent = lock(&self.sent);
        if sent.is_none_or(|at| now.saturating_duration_since(at) >= together_gear::PIECE_EVERY)
            && let Some((kind, body)) = lock(&self.outbox).pop_front()
        {
            inner.link.action(kind, &body);
            *sent = Some(now);
        }
    }

    fn frame(&self, now: Instant) -> Option<Picture> {
        let mut inner = lock(&self.inner);
        self.feed(&mut inner, now);
        let now_ms = self.now_ms(now);
        inner.link.draw(now_ms);
        let lifted = inner
            .link
            .runtime()
            .game()
            .map(|client| client.lifted().to_vec())
            .unwrap_or_default();
        *lock(&self.lifted) = Arc::new(lifted);
        let has_scene = if let Some(scene) = inner.link.runtime().game().and_then(|client| client.scene()) {
            *lock(&self.scene) = Some(scene);
            true
        } else {
            false
        };
        let picture = if has_scene {
            None
        } else {
            let frame = inner.link.runtime().last_frame();
            (frame.w > 0 && frame.h > 0).then(|| Picture {
                w: frame.w,
                h: frame.h,
                px: Arc::new(frame.px.clone()),
            })
        };
        self.sync_state(&mut inner);
        if picture.is_some() {
            *lock(&self.picture) = picture.clone();
        }
        picture
    }
}

/// What the delve's server sends knight `knight`'s seat of `run` (its share
/// of the tick's tree, encoded as a whole snapshot), the run that seat's
/// client reads back from it, and the picture the client draws: what a
/// friend gets, for the tests of what reaches one.
#[cfg(any(test, feature = "testing"))]
pub fn guest_view(
    run: &Run,
    knight: u32,
) -> (Vec<u8>, Run, angel_delve::render::overworld::Img) {
    use angel_delve::wire;
    use angel_net::client::{Frame, GameClient, View};
    use angel_net::proto::{PROTOCOL, Welcome};
    let seat = wire::seat_of(knight);
    let tree = wire::publish(run).expect("a run makes a tree");
    let mut body = Vec::new();
    angel_net::doc::wire::Writer::default().value(&mut body, &wire::seat_tree(&tree, seat));
    let params = wire::Params::of(angel_delve::settings::get()).encode();
    let welcome = Welcome {
        protocol: PROTOCOL,
        game: Client::GAME,
        seat,
        seat_token: "",
        tick_hz: 30,
        snapshot_hz: 30,
        tick: 0,
        params: &params,
    };
    let mut client = Client::boot(&welcome).expect("the client boots");
    client
        .snapshot(1, true, &body)
        .expect("the client reads the tree");
    let read = client.newest().expect("a run read back").clone();
    let mut frame = Frame::default();
    let view = View {
        render_tick: 1.0,
        newest_tick: 1,
        ack: 0,
        pending: &[],
        partial: 0.0,
        hands: Default::default(),
        seat,
        now_ms: 0.0,
        correct_ms: angel_net::clock::CORRECT_MS,
    };
    client.draw(&view, &mut frame);
    let picture = Picture {
        w: frame.w,
        h: frame.h,
        px: Arc::new(frame.px),
    };
    (body, read, picture.img())
}

/// The key the Delve's client takes for a move on the ballot.
pub fn ballot_bits(action: super::together_shooter::votes::Action) -> u32 {
    use super::together_shooter::votes::Action;
    match action {
        Action::Toggle => button::X10,
        Action::Close => button::MENU,
        Action::Up => button::AIM_UP,
        Action::Down => button::AIM_DOWN,
        Action::Left => button::AIM_LEFT,
        Action::Right => button::AIM_RIGHT,
        Action::Rate(1) => button::SLOT1,
        Action::Rate(2) => button::SLOT2,
        Action::Rate(3) => button::SLOT3,
        Action::Rate(4) => button::SLOT4,
        Action::Rate(_) => button::X11,
        Action::Clear => button::X12,
        Action::Untried => button::MAP,
    }
}

/// The buttons that press `input` on a seat's client: a terminal's keys,
/// read through the Delve's controls (`angel_delve::client::CONTROLS`), the
/// same buttons a browser's keyboard presses. The menu, the ballot and a
/// floor's plate are this terminal's own to drive, so none of theirs is here.
pub fn bits(input: &Input) -> u32 {
    let mut bits = 0;
    let mut on = |yes: bool, bit: u32| {
        if yes {
            bits |= bit;
        }
    };
    on(input.move_x < 0, button::LEFT);
    on(input.move_x > 0, button::RIGHT);
    on(input.move_y < 0, button::UP);
    on(input.move_y > 0, button::DOWN);
    on(input.aim_x < 0, button::AIM_LEFT);
    on(input.aim_x > 0, button::AIM_RIGHT);
    on(input.aim_y < 0, button::AIM_UP);
    on(input.aim_y > 0, button::AIM_DOWN);
    on(input.leave_held, button::MENU);
    on(input.fire, button::FIRE);
    on(input.dash, button::JUMP);
    // E meets the world and takes gear alike, as the client reads USE.
    on(input.interact || input.take, button::USE);
    on(input.swing, button::ALT);
    on(input.bomb, button::X1);
    on(input.play != 0, button::X2);
    on(input.vigil, button::X3);
    on(input.ult, button::X4);
    // The ride key (`rules::shoulders`): C in the Delve's controls.
    on(input.ride, button::X11);
    // The next-card key (`Hero::cycle_hand`): P in the Delve's controls.
    on(input.cycle, button::X12);
    for (slot, bit) in [button::SLOT1, button::SLOT2, button::SLOT3, button::SLOT4]
        .into_iter()
        .enumerate()
    {
        on(usize::from(input.cast) == slot + 1, bit);
    }
    for (slot, bit) in [button::X5, button::X6, button::X7, button::X8, button::X9]
        .into_iter()
        .enumerate()
    {
        on(usize::from(input.skill) == slot + 1, bit);
    }
    bits
}
