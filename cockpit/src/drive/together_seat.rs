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
pub(crate) type Client = angel_delve::client::DelveClient;

/// How long a new seat waits for its welcome and the run before play
/// begins without them.
const WELCOME_WAIT: Duration = Duration::from_secs(2);

/// What the terminal's keys did, in the order it heard them.
#[derive(Clone, Copy, Debug)]
enum Key {
    /// Everything held now, all at once: the cockpit keeps which keys are
    /// down (from key releases, or a terminal's repeats when it sends none)
    /// and says so on every key and every frame.
    Held(u32),
    Release,
}

/// A seat's newest picture: `px` is `w * h` words, `0xAARRGGBB`.
#[derive(Clone)]
pub(crate) struct Picture {
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) px: Arc<Vec<u32>>,
}

impl Picture {
    /// As the renderer's image, for the terminal's picture path.
    pub(crate) fn img(&self) -> crate::stage::world_viz::overworld::Img {
        crate::stage::world_viz::overworld::Img::from_words(
            self.w as i32,
            self.h as i32,
            self.px.to_vec(),
        )
    }

    /// The pixel at (x, y), black outside.
    pub(crate) fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        if x >= self.w || y >= self.h {
            return [0, 0, 0];
        }
        let p = self.px[(y * self.w + x) as usize];
        [(p >> 16) as u8, (p >> 8) as u8, p as u8]
    }
}

struct Inner {
    link: NativeLink<Client>,
    lines: Vec<String>,
    picture: Option<Picture>,
}

/// One seat: its client and link, the keys waiting for them, and the
/// one-off actions waiting to go.
struct Core {
    inner: Mutex<Inner>,
    keys: Mutex<Vec<Key>>,
    /// Taps waiting, each its own press: one goes to the sampler once the
    /// one before has been sampled, so their order is the order they came.
    taps: Mutex<VecDeque<u32>>,
    /// Actions in the order they were made, a larger one in pieces, sent no
    /// faster than `together_gear::PIECE_EVERY`.
    outbox: Mutex<VecDeque<(u32, Vec<u8>)>>,
    sent: Mutex<Option<Instant>>,
    epoch: Instant,
}

/// One seat, shared by the terminal's thread and the render workers.
#[derive(Clone)]
pub(crate) struct Seat {
    core: Arc<Core>,
    /// The seat's own render worker, for pictures painted in half blocks
    /// (Kitty's go through the viewer's).
    worker: Arc<std::sync::OnceLock<crate::ui::game_pipeline::Pipeline>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Seat {
    fn of(link: NativeLink<Client>) -> Seat {
        Seat {
            core: Arc::new(Core {
                inner: Mutex::new(Inner {
                    link,
                    lines: Vec::new(),
                    picture: None,
                }),
                keys: Mutex::new(Vec::new()),
                taps: Mutex::new(VecDeque::new()),
                outbox: Mutex::new(VecDeque::new()),
                sent: Mutex::new(None),
                epoch: Instant::now(),
            }),
            worker: Arc::new(std::sync::OnceLock::new()),
        }
    }

    /// A seat on this angelX's own server, waiting (a moment at most) for
    /// its welcome and the run, so its first keys are its knight's.
    pub(crate) fn local(conn: LocalConn, name: &str) -> Seat {
        let seat = Seat::of(NativeLink::local(conn, name, LinkOptions::default()));
        let until = Instant::now() + WELCOME_WAIT;
        // No command goes before the run is in: the first is sampled at the
        // seat's first tick, with its knight to play.
        loop {
            let mut inner = lock(&seat.core.inner);
            let now_ms = seat.core.now_ms(Instant::now());
            inner.link.poll(now_ms);
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
    pub(crate) fn connect(link: &str, name: &str) -> Result<Seat, String> {
        NativeLink::connect(link, name, LinkOptions::default()).map(Seat::of)
    }

    fn key(&self, key: Key) {
        lock(&self.core.keys).push(key);
    }

    /// Everything held now (`angel_net::input::button`): a key held and let
    /// go between two frames is still pressed for a command.
    pub(crate) fn set_held(&self, bits: u32) {
        self.key(Key::Held(bits));
    }

    /// A press and its release at once: a key the client takes as a press
    /// (the ballot's, the menu's). Taps go one a command, in order: two in
    /// one command would be taken in the client's order, not theirs.
    pub(crate) fn tap(&self, bits: u32) {
        if bits != 0 {
            lock(&self.core.taps).push_back(bits);
        }
    }

    /// Taps of `bits` not yet given to the sampler, taken back: a menu
    /// opened and left before its tap went out never holds the knight.
    pub(crate) fn untap(&self, bits: u32) {
        lock(&self.core.taps).retain(|&tap| tap != bits);
    }

    /// Let go of everything (the terminal lost focus, a menu took the keys).
    pub(crate) fn release_all(&self) {
        self.key(Key::Release);
    }

    /// A one-off action (`together_gear`): in pieces when it is large.
    pub(crate) fn action(&self, kind: u32, body: &[u8]) {
        lock(&self.core.outbox).extend(together_gear::pieces(kind, body));
    }

    /// One display frame at `now`: the keys in, what arrived taken, and
    /// the seat's own screen drawn. The commands are `tick`'s, on the
    /// terminal's own thread.
    pub(crate) fn frame(&self, now: Instant) -> Option<Picture> {
        self.core.frame(now)
    }

    /// The seat's next frame, drawn at `now` on its own render worker; the
    /// newest finished one is `picture`. Only the newest asked for is drawn.
    pub(crate) fn request(&self, now: Instant) {
        let worker = self
            .worker
            .get_or_init(crate::ui::game_pipeline::Pipeline::new);
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

    /// The seat's next frame as the renderer's image, for the terminal's
    /// picture path to make on its own worker.
    pub(crate) fn drawer(
        &self,
    ) -> impl FnOnce(Instant) -> crate::stage::world_viz::overworld::Img + Send + 'static {
        let core = Arc::clone(&self.core);
        move |now| {
            let (w, h) = crate::stage::world_viz::overworld::arena::art_v2::size();
            core.frame(now).map_or_else(
                || crate::stage::world_viz::overworld::Img::black(w as i32, h as i32),
                |p| p.img(),
            )
        }
    }

    /// The keys in, what arrived taken, the commands and actions due sent,
    /// with nothing drawn: the seat plays at the command rate whether or
    /// not its room is on screen (its frames are drawn on a render worker
    /// when it is). A frame being drawn elsewhere is never waited for: it
    /// does the same.
    pub(crate) fn tick(&self, now: Instant) {
        let core = &self.core;
        let Ok(mut inner) = core.inner.try_lock() else {
            return;
        };
        core.feed(&mut inner, now);
        let now_ms = core.now_ms(now);
        inner.link.tick(now_ms);
        let lines = inner.link.take_lines();
        inner.lines.extend(lines);
    }

    /// The seat's player has left the game's screen: what arrived taken,
    /// pings sent and no commands (`NativeLink::idle`), so the host's server
    /// counts the seat away and keeps its knight out of harm's way.
    pub(crate) fn idle(&self, now: Instant) {
        let core = &self.core;
        let Ok(mut inner) = core.inner.try_lock() else {
            return;
        };
        core.feed(&mut inner, now);
        let now_ms = core.now_ms(now);
        inner.link.idle(now_ms);
        let lines = inner.link.take_lines();
        inner.lines.extend(lines);
    }

    /// Whether the seat's ballot is open, as its client shows it (at once,
    /// before the run says so); `None` while keys wait to be sampled, so
    /// the client has not taken them yet.
    pub(crate) fn ballot_open(&self) -> Option<bool> {
        if !lock(&self.core.keys).is_empty() || !lock(&self.core.taps).is_empty() {
            return None;
        }
        let mut inner = lock(&self.core.inner);
        if inner.link.hands().peek().pressed != 0 {
            return None;
        }
        Some(inner.link.runtime().game().is_some_and(Client::ballot_open))
    }

    /// Lines the seat's client made (`sfx <name>`, `say <name>: <words>`,
    /// `notice <words>`, `music <track>`) since the last look.
    pub(crate) fn take_lines(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.core.inner).lines)
    }

    /// The last picture drawn.
    pub(crate) fn picture(&self) -> Option<Picture> {
        lock(&self.core.inner).picture.clone()
    }

    pub(crate) fn status(&self) -> LinkStatus {
        lock(&self.core.inner).link.status().clone()
    }

    /// The knight this seat plays, once seated.
    pub(crate) fn knight(&self) -> Option<u32> {
        lock(&self.core.inner)
            .link
            .runtime()
            .seat()
            .map(angel_delve::wire::knight_of)
    }

    /// The run as this seat's client holds it: the newest tick it has.
    pub(crate) fn run(&self) -> Option<Run> {
        lock(&self.core.inner)
            .link
            .runtime()
            .game()
            .and_then(Client::newest)
            .cloned()
    }

    /// Leave on purpose: the seat is freed.
    pub(crate) fn bye(&self) {
        lock(&self.core.inner).link.bye();
    }
}

impl Core {
    fn now_ms(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.epoch).as_secs_f64() * 1000.0
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
        let frame = inner.link.draw(now_ms);
        let picture = (frame.w > 0 && frame.h > 0).then(|| Picture {
            w: frame.w,
            h: frame.h,
            px: Arc::new(frame.px.clone()),
        });
        let lines = inner.link.take_lines();
        inner.lines.extend(lines);
        if picture.is_some() {
            inner.picture.clone_from(&picture);
        }
        picture
    }
}

/// What the delve's server sends knight `knight`'s seat of `run` (its share
/// of the tick's tree, encoded as a whole snapshot), the run that seat's
/// client reads back from it, and the picture the client draws: what a
/// friend gets, for the tests of what reaches one.
#[cfg(test)]
pub(crate) fn guest_view(
    run: &Run,
    knight: u32,
) -> (Vec<u8>, Run, crate::stage::world_viz::overworld::Img) {
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
pub(crate) fn ballot_bits(action: super::together_shooter::votes::Action) -> u32 {
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
pub(crate) fn bits(input: &Input) -> u32 {
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
    on(input.fire, button::FIRE);
    on(input.dash, button::JUMP);
    // E meets the world and takes gear alike, as the client reads USE.
    on(input.interact || input.take, button::USE);
    on(input.swing, button::ALT);
    on(input.bomb, button::X1);
    on(input.play != 0, button::X2);
    on(input.vigil, button::X3);
    on(input.ult, button::X4);
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
