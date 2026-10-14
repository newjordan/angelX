//! The Delve's client on the engine (`docs/ENGINE.md`, "The Delve on the
//! engine"): a browser's wasm module and a joined angelX alike hold the
//! run's tree, read each tick back into a `Run`, and draw their own seat's
//! screen from it. No threads, files, environment or clock: time comes in
//! through `View`.
//!
//! - **Snapshots.** The tree is patched (`angel_net::doc`) and read back
//!   with the run's own `Deserialize` (`wire::read`); the last `KEEP` ticks
//!   are kept, each with its `Pose`.
//! - **Everyone else** is drawn at `View::render_tick`, between the two
//!   kept ticks around it (`Run::drawn_with` blends their poses in place,
//!   then puts them back).
//! - **The own knight** is the newest tick's, with the commands the server
//!   has not answered replayed through `Run::predict`, then walked
//!   `View::partial` of a tick along the hands held now: a key shows on the
//!   next display frame. In the kingdom its camera follows the same way.
//!   When the server's answer differs from what was shown, the difference
//!   is eased out over `View::correct_ms` (`angel_net::clock::CORRECT_MS`
//!   unless the `correct_ms` knob moved it).
//! - **The picture** is `Run::view_for_seat`, then `arena::frame_for_live`
//!   at the art set's own size (`size`): a torch step's lit scenery is
//!   drawn beside the painter (a stage a frame in a browser) while the step
//!   before shows, so no frame pays for a whole room.
//! - **Input.** `command` turns the hands into a `DelveCmd` as the old
//!   browser seat did: the ballot, the activity menu and a floor plate take
//!   the keys when they are up (`controls` has the keys).
//! - **Sound.** Events become `sfx` and `say` lines; the music is the drawn
//!   view's own (`Run::music`), said whenever it changes.
//!
//! The generic float motion (`motion::Motion`) is not used here: it costs a
//! whole serialize and deserialize of the run a frame. Knights, monsters,
//! shots and cameras slide between ticks; everything else a game moves
//! (riders, chariots, crabs, pigeons, critters) steps at the tick rate.

use crate::render::overworld::arena;
use crate::rules::command::DelveCmd;
use crate::rules::kingdom::follow_camera;
use crate::rules::mirror::Pose;
use crate::rules::votes::{Action, Command as Vote};
use crate::rules::{HZ, Hero, Input, Run, RoomKind};
use crate::settings::{self, Settings};
use crate::wire::{self, Event, Params};
use angel_net::client::{Frame, GameClient, View};
use angel_net::clock::bracket;
use angel_net::cmd::Command;
use angel_net::codec::{Bytes, Malformed};
use angel_net::doc::delta;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::input::{Hands, button};
use angel_net::knob::{Knob, Side};
use angel_net::proto::Welcome;
use std::collections::{BTreeMap, VecDeque};

/// Ticks kept: the interpolation delay (100 ms is three ticks) and room for
/// snapshots that bunch up on the way.
const KEEP: usize = 8;
/// A stick leaning past this much (of 127) counts as that way: the rules
/// take each axis as -1, 0 or 1.
const STICK_LEAN: i8 = 51;
/// A correction bigger than this (in run units) is a jump, not a slip: it is
/// shown at once rather than eased.
const SNAP: f32 = 4.0;
/// A correction smaller than this (run units) is no correction a player
/// could see: a panel does not count it.
const SEEN_CORRECTION: f32 = 0.05;
/// Predictions remembered per command, for measuring corrections.
const SHOWN_KEPT: usize = 64;
/// A ballot action waits at most this long for the run to take it; then it
/// is dropped (a host that does not keep the ballot).
const VOTE_WAIT_TICKS: u32 = 3 * HZ;
/// Leaving an activity, or skipping a floor plate, is asked for at most
/// this long; the run says when it has happened.
const ASK_TICKS: u32 = HZ;
/// Ballot actions waiting at most.
const VOTES_QUEUED: usize = 64;

/// The keyboard, pad and sticks of the old browser seat (guest.html), and
/// the buttons a touch layout will show.
///
/// Keys: WASD walk, the arrows aim, F fires, E meets the world and takes
/// gear, Space dashes (so does Shift with a walk), Q swings, X throws a
/// bomb, Z drinks the first card, 1-4 cast, G keeps vigil, R the ultimate,
/// M , . ; ' the five skills, V the ballot (arrows, 1-5, 0, N and Esc in
/// it), Esc pauses a game in a room and Enter or Backspace leaves it.
///
/// Pad: A dashes, B swings, X bombs, Y and back meet the world, LB is the
/// ultimate, RB and RT fire, start keeps vigil, L3 the ballot, R3 drinks,
/// the d-pad walks; with LT held the face buttons and RB are the skills and
/// the d-pad the four spells. The right stick aims and fires.
pub const CONTROLS: &str = concat!(
    r#"{"stick":"left","aim":"right","buttons":["#,
    r#"{"id":"fire","label":"F","bit":16},"#,
    r#"{"id":"use","label":"E","bit":32},"#,
    r#"{"id":"dash","label":"Dash","bit":1024},"#,
    r#"{"id":"swing","label":"Q","bit":4096},"#,
    r#"{"id":"bomb","label":"X","bit":1048576},"#,
    r#"{"id":"drink","label":"Z","bit":2097152},"#,
    r#"{"id":"ult","label":"R","bit":8388608}],"#,
    r#""keys":{"KeyW":"UP","KeyS":"DOWN","KeyA":"LEFT","KeyD":"RIGHT","#,
    r#""ArrowUp":"AIM_UP","ArrowDown":"AIM_DOWN","ArrowLeft":"AIM_LEFT","ArrowRight":"AIM_RIGHT","#,
    r#""KeyF":"FIRE","KeyE":"USE","Space":"JUMP","Escape":"MENU","KeyQ":"ALT","#,
    r#""ShiftLeft":"SHIFT","ShiftRight":"SHIFT","KeyN":"MAP","#,
    r#""Enter":"OK","NumpadEnter":"OK","Backspace":"OK","#,
    r#""Digit1":"SLOT1","Digit2":"SLOT2","Digit3":"SLOT3","Digit4":"SLOT4","#,
    r#""Numpad1":"SLOT1","Numpad2":"SLOT2","Numpad3":"SLOT3","Numpad4":"SLOT4","#,
    r#""KeyX":"X1","KeyZ":"X2","KeyG":"X3","KeyR":"X4","#,
    r#""KeyM":"X5","Comma":"X6","Period":"X7","Semicolon":"X8","Quote":"X9","#,
    r#""KeyV":"X10","Digit5":"X11","Numpad5":"X11","Digit0":"X12","Numpad0":"X12"},"#,
    r#""pad":{"0":"JUMP","1":"ALT","2":"X1","3":"USE","4":"X4","5":"FIRE","7":"FIRE","#,
    r#""8":"USE","9":"X3","10":"X10","11":"X2","12":"UP","13":"DOWN","14":"LEFT","15":"RIGHT","#,
    r#""6+0":"X5","6+1":"X6","6+2":"X7","6+3":"X8","6+5":"X9","#,
    r#""6+12":"SLOT1","6+14":"SLOT2","6+15":"SLOT3","6+13":"SLOT4"},"#,
    r#""sticks":{"move":"left","aim":"right","aim_holds":"FIRE"}}"#,
);

/// What each of the Delve's buttons is (see `CONTROLS`).
mod key {
    use angel_net::input::button::*;
    pub const WALK: [u32; 4] = [UP, DOWN, LEFT, RIGHT];
    pub const BOMB: u32 = X1;
    pub const DRINK: u32 = X2;
    pub const VIGIL: u32 = X3;
    pub const ULT: u32 = X4;
    pub const SKILLS: [u32; 5] = [X5, X6, X7, X8, X9];
    pub const SPELLS: [u32; 4] = [SLOT1, SLOT2, SLOT3, SLOT4];
    pub const BALLOT: u32 = X10;
    pub const RATE5: u32 = X11;
    pub const CLEAR: u32 = X12;
    /// N in the ballot: not tried yet.
    pub const UNTRIED: u32 = MAP;
    /// Enter, Backspace: leave the game in this room.
    pub const LEAVE: u32 = OK;
}

/// One kept tick.
struct Kept {
    tick: u32,
    run: Run,
    pose: Pose,
}

/// The ballot as this seat drives it: actions wait in order until the run
/// shows the seat took them (its serial), and the ballot shows open or shut
/// at once, before the run says so.
#[derive(Default)]
struct Ballot {
    queue: VecDeque<Vote>,
    serial: u64,
    open: bool,
    /// Ticks the front action has waited.
    waited: u32,
}

pub struct DelveClient {
    seat: Option<u32>,
    knight: Option<u32>,
    art_v2: bool,
    /// The host's art set, the `art` knob's default.
    host_art_v2: bool,
    tree: Option<Value>,
    names: Reader,
    kept: VecDeque<Kept>,
    /// Where each command first put our knight, to measure corrections.
    shown: VecDeque<(u32, f32, f32)>,
    last_ack: u32,
    offset: (f32, f32),
    offset_at: f64,
    /// Where our knight was last drawn, in run units and in frame pixels
    /// (tests and probes).
    drawn: Option<(f32, f32)>,
    drawn_px: Option<(f32, f32)>,
    ballot: Ballot,
    /// The activity menu (Esc in a room's game): the knight waits.
    menu: bool,
    /// Ticks left asking to leave the room's game, and to skip the plate.
    leaving: u32,
    skipping: u32,
    /// The drawn view's music, and the last said to the shell.
    music: Option<&'static str>,
    said_music: Option<Option<&'static str>>,
    /// The tick each sound last played on.
    sounded: BTreeMap<String, u32>,
    /// Every frame exactly as `arena::frame_for` draws it (a screenshot, a
    /// test), rather than live (`frame_for_live`).
    exact: bool,
    /// Corrections of our knight so far, and how far they moved it in all,
    /// for a developer's panel.
    corrections: u32,
    corrected: f64,
}

impl DelveClient {
    pub fn new(seat: Option<u32>, params: Params) -> DelveClient {
        // The host's art set; whatever else this process set stands (a host's
        // own seat shares its process, and its feel ledger, with the host).
        settings::set(Settings { art_v2: params.art_v2, ..settings::get() });
        DelveClient {
            seat,
            knight: seat.map(wire::knight_of),
            art_v2: params.art_v2,
            host_art_v2: params.art_v2,
            tree: None,
            names: Reader::default(),
            kept: VecDeque::with_capacity(KEEP + 1),
            shown: VecDeque::new(),
            last_ack: 0,
            offset: (0.0, 0.0),
            offset_at: 0.0,
            drawn: None,
            drawn_px: None,
            ballot: Ballot::default(),
            menu: false,
            leaving: 0,
            skipping: 0,
            music: None,
            said_music: None,
            sounded: BTreeMap::new(),
            exact: false,
            corrections: 0,
            corrected: 0.0,
        }
    }

    /// Draw every frame exactly as `arena::frame_for` does, the torch step
    /// included, rather than live.
    pub fn exact(mut self) -> DelveClient {
        self.exact = true;
        self
    }

    /// The frame's size: the art set's own (the GPU scales it).
    pub fn size(&self) -> (u32, u32) {
        let (w, h) = (arena::NATIVE_W as u32, arena::NATIVE_H as u32);
        if self.art_v2 { (2 * w, 2 * h) } else { (w, h) }
    }

    pub fn seat(&self) -> Option<u32> {
        self.seat
    }

    /// The newest run held.
    pub fn newest(&self) -> Option<&Run> {
        self.kept.back().map(|k| &k.run)
    }

    /// The newest tick held.
    pub fn newest_tick(&self) -> Option<u32> {
        self.kept.back().map(|k| k.tick)
    }

    /// Where our knight was last drawn, in run units.
    pub fn drawn_at(&self) -> Option<(f32, f32)> {
        self.drawn
    }

    /// Whether the ballot is open for this seat, as it shows.
    pub fn ballot_open(&self) -> bool {
        self.ballot.open
    }

    /// Whether the activity menu is up.
    pub fn menu_open(&self) -> bool {
        self.menu
    }

    fn keep(&mut self, tick: u32, run: Run) {
        let pose = run.pose();
        self.kept.push_back(Kept { tick, run, pose });
        while self.kept.len() > KEEP {
            self.kept.pop_front();
        }
    }

    /// The correction still showing at `now`.
    fn easing(&self, now: f64, over_ms: f64) -> (f32, f32) {
        let left = if over_ms > 0.0 { (1.0 - (now - self.offset_at) / over_ms).clamp(0.0, 1.0) as f32 } else { 0.0 };
        (self.offset.0 * left, self.offset.1 * left)
    }

    /// Our knight as the newest tick has them, walked on by the commands
    /// the server has not answered and `partial` of a tick of the hands held
    /// now; with its kingdom camera following, and any correction easing.
    fn own(&mut self, view: &View<DelveCmd>) -> Option<Hero> {
        let knight = self.knight?;
        let walk = self.walk(&view.hands);
        let newest = &mut self.kept.back_mut()?.run;
        // A knight its keys do not move (a ride, the privy, a Bladewind, a
        // side-on hall) is drawn from the server's ticks, between two like
        // everyone else, never held at the newest.
        if !newest.predictable(knight) {
            self.shown.clear();
            self.offset = (0.0, 0.0);
            return None;
        }
        let server = newest.players.get(&knight)?.clone();
        let kingdom = newest.room().kind == RoomKind::Kingdom && !newest.exits.outside.contains_key(&knight);
        let (w, h) = (newest.room().width(), newest.room().height());
        let follow = |run: &mut Run| {
            let ease = run.tune.camera_follow;
            if kingdom && let Some(hero) = run.players.get_mut(&knight) {
                follow_camera(hero, w, h, ease);
            }
        };
        for (_, cmd) in view.pending {
            newest.predict(knight, cmd.input());
            follow(newest);
        }
        let after = newest.players.get(&knight).map(|h| (h.x, h.y, h.camera))?;
        // Part of the coming tick, along the hands held now: held only, as
        // a press acts once, when its command is sampled. The whole tick is
        // walked, then drawn `partial` of the way, so what shows is on the
        // path the server will step (a part-tick slide can reach where a
        // whole one stops at a wall).
        if let Some(walk) = walk.filter(|_| view.partial > 0.0) {
            newest.predict(knight, walk);
            follow(newest);
            if let Some(hero) = newest.players.get_mut(&knight) {
                let t = view.partial.clamp(0.0, 1.0);
                let lerp = |a: f32, b: f32| a + (b - a) * t;
                (hero.x, hero.y) = (lerp(after.0, hero.x), lerp(after.1, hero.y));
                if let (Some((ax, ay)), Some((bx, by))) = (after.2, hero.camera) {
                    hero.camera = Some((lerp(ax, bx), lerp(ay, by)));
                }
            }
        }
        let mut hero = newest.players.get(&knight)?.clone();
        // The newest tick, as the server had it, stays as it was: its place,
        // and the roll, guard and mana the steps ahead ran.
        newest.players.insert(knight, server.clone());

        // The server answered more of our commands: compare its word with
        // what we showed for the last one it took.
        if view.ack != self.last_ack {
            self.last_ack = view.ack;
            if let Some(&(_, sx, sy)) = self.shown.iter().find(|(seq, _, _)| *seq == view.ack) {
                let (ex, ey) = (sx - server.x, sy - server.y);
                if ex != 0.0 || ey != 0.0 {
                    // What a panel counts: a correction a player could see.
                    let off = ex.hypot(ey);
                    if off > SEEN_CORRECTION {
                        self.corrections += 1;
                        self.corrected += f64::from(off);
                    }
                    let (cx, cy) = self.easing(view.now_ms, view.correct_ms);
                    let (nx, ny) = (cx + ex, cy + ey);
                    self.offset = if nx.abs() > SNAP || ny.abs() > SNAP { (0.0, 0.0) } else { (nx, ny) };
                    self.offset_at = view.now_ms;
                }
            }
            while self.shown.front().is_some_and(|(seq, _, _)| *seq <= view.ack) {
                self.shown.pop_front();
            }
        }
        if let Some(&(seq, _)) = view.pending.last()
            && self.shown.back().is_none_or(|(last, _, _)| *last < seq)
        {
            self.shown.push_back((seq, after.0, after.1));
            if self.shown.len() > SHOWN_KEPT {
                self.shown.pop_front();
            }
        }
        let (ox, oy) = self.easing(view.now_ms, view.correct_ms);
        hero.x += ox;
        hero.y += oy;
        Some(hero)
    }

    /// The input the hands walk with between two commands, or nothing while
    /// a menu, the ballot or a plate has the keys.
    fn walk(&self, hands: &Hands) -> Option<Input> {
        let run = self.newest()?;
        let knight = self.knight?;
        if self.menu || self.ballot.open || !self.ballot.queue.is_empty() || run.floor_plate(Some(knight)).is_some() {
            return None;
        }
        Some(play(hands.held, hands.held, hands))
    }

    /// Whether to draw from the tick at `render` rather than the newest: not
    /// when our knight or the party has gone through a door since.
    fn same_place(&self, at: usize) -> bool {
        let (Some(then), Some(now)) = (self.kept.get(at), self.kept.back()) else {
            return false;
        };
        let room = |run: &Run| {
            self.knight
                .and_then(|k| run.exits.outside.get(&k))
                .map_or(run.at, |e| e.at)
        };
        then.run.raid_id == now.run.raid_id
            && then.run.dungeon.depth == now.run.dungeon.depth
            && room(&then.run) == room(&now.run)
    }

    /// A ballot action, shown at once and sent until the run takes it.
    fn vote(&mut self, action: Action) {
        if self.ballot.queue.len() >= VOTES_QUEUED {
            return;
        }
        let taken = self.knight.and_then(|k| self.newest()?.playtest.seats.get(&k).map(|s| s.serial));
        self.ballot.serial = self.ballot.serial.max(taken.unwrap_or(0)) + 1;
        self.ballot.queue.push_back(Vote { serial: self.ballot.serial, action });
        match action {
            Action::Toggle => self.ballot.open = !self.ballot.open,
            Action::Close => self.ballot.open = false,
            _ => {}
        }
    }

    /// Ballot actions the run has taken are done; the ballot shows as the
    /// run has it once none wait.
    fn settle_ballot(&mut self) {
        let (Some(knight), Some(run)) = (self.knight, self.kept.back().map(|k| &k.run)) else {
            return;
        };
        let taken = run.playtest.seats.get(&knight).map_or(0, |s| s.serial);
        while self.ballot.queue.front().is_some_and(|v| v.serial <= taken) {
            self.ballot.queue.pop_front();
            self.ballot.waited = 0;
        }
        if self.ballot.queue.is_empty() {
            self.ballot.open = run.playtest.open(knight);
        } else {
            self.ballot.waited += 1;
            if self.ballot.waited > VOTE_WAIT_TICKS {
                self.ballot.queue.clear();
                self.ballot.waited = 0;
            }
        }
    }

    /// The ballot's keys, pressed this tick.
    fn ballot_keys(&mut self, pressed: u32) {
        let on = |bits: u32| pressed & bits != 0;
        if on(key::BALLOT) {
            self.vote(Action::Toggle);
        }
        if !self.ballot.open {
            return;
        }
        let actions = [
            (button::MENU | button::ALT, Action::Close),
            (button::AIM_UP | button::UP, Action::Up),
            (button::AIM_DOWN | button::DOWN, Action::Down),
            (button::AIM_LEFT | button::LEFT, Action::Left),
            (button::AIM_RIGHT | button::RIGHT, Action::Right),
            (button::SLOT1, Action::Rate(1)),
            (button::SLOT2, Action::Rate(2)),
            (button::SLOT3, Action::Rate(3)),
            (button::SLOT4, Action::Rate(4)),
            (key::RATE5, Action::Rate(5)),
            (key::CLEAR | key::BOMB, Action::Clear),
            (key::UNTRIED | button::JUMP, Action::Untried),
        ];
        for (bits, action) in actions {
            if on(bits) {
                self.vote(action);
            }
        }
    }
}

/// The game's input from the buttons: `bits` for every flag, `slots` for the
/// card, spell and skill keys, and the sticks from `hands`.
fn play(bits: u32, slots: u32, hands: &Hands) -> Input {
    let on = |b: u32| bits & b != 0;
    let axis = |v: i8| -> i8 {
        if v >= STICK_LEAN {
            1
        } else if v <= -STICK_LEAN {
            -1
        } else {
            0
        }
    };
    let first = |keys: &[u32]| keys.iter().position(|&k| slots & k != 0).map_or(0, |i| i as u8 + 1);
    let walking = key::WALK.iter().any(|&k| on(k));
    Input {
        move_x: axis(hands.x),
        move_y: axis(hands.y),
        aim_x: axis(hands.aim_x),
        aim_y: axis(hands.aim_y),
        fire: on(button::FIRE),
        dash: on(button::JUMP) || (on(button::SHIFT) && walking),
        interact: on(button::USE),
        take: on(button::USE),
        bomb: on(key::BOMB),
        swing: on(button::ALT),
        play: u8::from(slots & key::DRINK != 0),
        cast: first(&key::SPELLS),
        vigil: on(key::VIGIL),
        ult: on(key::ULT),
        skill: first(&key::SKILLS),
        ..Input::default()
    }
}

/// The input of the bits pressed since the last command: the flags and the
/// slots, no sticks (the sampler sweeps a tapped direction into the stick).
/// Shift pressed while walking is a dash pressed.
fn presses(pressed: u32, held: u32) -> Input {
    let walking = key::WALK.iter().any(|&k| held & k != 0);
    let mut input = play(pressed, pressed, &Hands::default());
    input.dash |= pressed & button::SHIFT != 0 && walking;
    input
}

/// The shortest gap between two of the same sound, in ms: rapid fire never
/// piles into noise (the host's mixer's own gaps, `together_audio`).
fn sound_gap(name: &str) -> u32 {
    match name {
        "gas_release" | "wheel_spin" | "wheel_land" => 1500,
        "spell_fire" | "skill_barrier" => 500,
        "wish_grant" => 1000,
        "spell_cast" => 250,
        "shot_bow" | "shot_crossbow" | "shot_handgonne" | "shot_bolt" | "shot_arcane" | "shot_spark" => 70,
        "hit" | "hit_blade" | "hit_axe" | "hit_arrow" | "hit_bolt" | "hit_blast" | "hit_magic" => 60,
        "step_stone" | "step_gravel" | "step_ash" | "step_wet" | "step_moss" | "step_wood" => 210,
        "knight_hit" | "knight_hit_big" | "gold" | "bomb_toss" | "shield_up" => 120,
        "boss_roar" | "boss_rage" | "boss_death" | "crack_open" | "hand_rise" | "hand_grab" | "hand_drag"
        | "ground_close" | "hell_gate_rise" | "hell_gate_enter" | "circle_toll" | "knight_fall" | "boss_rise"
        | "boss_fall" | "descend" | "door_seal" | "door_open" => 800,
        "kill" => 90,
        "swing" | "roll" | "shield_block" | "clang" | "spike" | "vent_fire" => 150,
        _ => 200,
    }
}

impl GameClient for DelveClient {
    type Cmd = DelveCmd;
    const GAME: &'static str = wire::GAME;

    fn boot(welcome: &Welcome) -> Result<Self, String> {
        let params = Params::decode(welcome.params).map_err(|_| "this page is out of date: reload it".to_string())?;
        Ok(DelveClient::new(welcome.seat, params))
    }

    fn snapshot(&mut self, tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        if whole {
            self.names = Reader::default();
            let mut b = Bytes::new(body);
            self.tree = Some(self.names.value(&mut b)?);
            b.end()?;
        } else if body.is_empty() {
            // Nothing changed: the newest run stands for this tick too.
            if let Some(last) = self.kept.back_mut() {
                last.tick = last.tick.max(tick);
            }
            return Ok(());
        } else {
            let tree = self.tree.as_mut().ok_or(Malformed)?;
            let mut b = Bytes::new(body);
            let patch = self.names.patch(&mut b)?;
            b.end()?;
            delta::apply(tree, patch).map_err(|_| Malformed)?;
        }
        let tree = self.tree.as_ref().ok_or(Malformed)?;
        let run = wire::read(tree).map_err(|_| Malformed)?;
        if !wire::drawable(&run) {
            return Err(Malformed);
        }
        if self.kept.back().is_some_and(|k| k.tick >= tick) {
            // A new connection's ticks start over.
            self.kept.clear();
        }
        // What the run now says ends: a game left (or no longer leavable),
        // a plate gone.
        let (can_leave, plate) = self
            .knight
            .map_or((false, false), |k| (run.can_leave_activity(k), run.floor_plate(Some(k)).is_some()));
        self.keep(tick, run);
        self.settle_ballot();
        if !can_leave {
            self.leaving = 0;
            self.menu = false;
        }
        if !plate {
            self.skipping = 0;
        }
        Ok(())
    }

    fn events(&mut self, tick: u32, events: &[&[u8]], lines: &mut Vec<String>) {
        let run = self.kept.back().map(|k| &k.run);
        for bytes in events {
            let Ok(event) = Event::decode(bytes) else { continue };
            let Some(line) = event.line(self.knight, run) else { continue };
            if let Event::Sound { name, .. } = &event {
                let gap = sound_gap(name) * HZ / 1000;
                if self.sounded.get(name).is_some_and(|&at| tick.wrapping_sub(at) < gap) {
                    continue;
                }
                self.sounded.insert(name.clone(), tick);
            }
            lines.push(line);
        }
    }

    fn command(&mut self, hands: &Hands) -> DelveCmd {
        let (Some(knight), Some(run)) = (self.knight, self.newest()) else {
            return DelveCmd::neutral();
        };
        let pressed = hands.pressed;
        let can_leave = run.can_leave_activity(knight);
        let plate = run.floor_plate(Some(knight)).is_some();
        // Only a knight standing in a live delve plays: a fallen one, or a
        // run that is won or wiped, sends nothing but its way out.
        let playable = run.active() && run.players.get(&knight).is_some_and(|h| h.hp > 0);
        let over = run.over();

        // A press while the floor's plate shows dismisses it, and nothing else.
        if plate && pressed != 0 {
            self.skipping = ASK_TICKS;
        }
        // Esc pauses the game in this room (Esc again resumes); Enter or
        // Backspace leaves it, paused or not. The ballot shuts first.
        if can_leave && !plate {
            if pressed & button::MENU != 0 && (self.menu || !self.ballot.open) {
                self.menu = !self.menu;
            } else if pressed & key::LEAVE != 0 {
                self.menu = false;
                self.leaving = ASK_TICKS;
                if self.ballot.open {
                    self.vote(Action::Close);
                }
            }
        }
        if !plate && !self.menu && self.leaving == 0 {
            self.ballot_keys(pressed);
        }

        let mut cmd = DelveCmd::neutral();
        if self.leaving > 0 {
            self.leaving -= 1;
            cmd.pressed.leave_activity = true;
        } else if self.menu {
            cmd.held.activity_menu = true;
        } else if self.skipping > 0 || plate {
            if self.skipping > 0 {
                self.skipping -= 1;
                cmd.pressed.skip_plate = true;
            }
        } else if self.ballot.open || !self.ballot.queue.is_empty() {
            cmd.pressed.vote = self.ballot.queue.front().copied();
        } else if over {
            // On the verdict's board, Enter (or R, the host's own key) asks
            // to go back to the kingdom (`Run::ask_again`).
            cmd.pressed.again = pressed & (button::OK | key::ULT) != 0;
        } else if playable {
            cmd.held = play(hands.held, hands.held, hands);
            cmd.pressed = presses(pressed, hands.held);
        }
        cmd
    }

    fn encode_cmd(cmd: &DelveCmd, out: &mut Vec<u8>) {
        cmd.encode(out);
    }

    fn draw(&mut self, view: &View<DelveCmd>, frame: &mut Frame) {
        let (w, h) = self.size();
        frame.resize(w, h);
        if self.kept.is_empty() {
            frame.px.fill(0xff00_0000);
            return;
        }
        let own = self.own(view);
        self.drawn = own.as_ref().map(|h| (h.x, h.y));
        let mut ticks = [0u32; KEEP];
        let n = self.kept.len();
        for (t, k) in ticks.iter_mut().zip(&self.kept) {
            *t = k.tick;
        }
        let (mut before, mut after, mut alpha) = bracket(&ticks[..n], view.render_tick).unwrap_or((n - 1, n - 1, 0.0));
        if !self.same_place(after) {
            (before, after, alpha) = (n - 1, n - 1, 0.0);
        }
        let pose = std::mem::take(&mut self.kept[before].pose);
        let knight = self.knight;
        let exact = self.exact;
        let mut music = None;
        let mut drawn_px = None;
        let img = self.kept[after].run.drawn_with(&pose, alpha, knight.zip(own.as_ref()), |run| {
            let seat_view = knight.and_then(|k| run.view_for_seat(k));
            let run = seat_view.as_ref().unwrap_or(run);
            music = run.music();
            drawn_px = own.as_ref().map(|hero| {
                let (x, y) = arena::frame_px(run, w as i32, h as i32, knight, hero.x, hero.y);
                (x as f32, y as f32)
            });
            let mut img = if exact {
                arena::frame_for(run, w as i32, h as i32, knight)
            } else {
                arena::frame_for_live(run, w as i32, h as i32, knight)
            };
            // Who is here: "AWAY" over an absent player's knight, and the
            // room's banner while it holds for the party.
            arena::presence_marks(&mut img, run, knight);
            img
        });
        self.kept[before].pose = pose;
        self.music = music;
        self.drawn_px = drawn_px;
        if img.w as u32 == w && img.h as u32 == h {
            for (out, &word) in frame.px.iter_mut().zip(img.words()) {
                // Transparent is the black paper.
                *out = if word >> 24 == 0 { 0xff00_0000 } else { word };
            }
        }
    }

    fn lines(&mut self, lines: &mut Vec<String>) {
        if self.said_music != Some(self.music) {
            self.said_music = Some(self.music);
            lines.push(match self.music {
                Some(track) => format!("music {}", wire::music_file(track)),
                None => "music".to_string(),
            });
        }
    }

    fn controls(&self) -> &'static str {
        CONTROLS
    }

    fn knobs(&self) -> Vec<Knob> {
        let host = self.host_art_v2;
        vec![
            Knob::new("art", "render", "Art kit (0 v1, 1 v2)", "ANGEL_ART (the host's environment)", Side::Client)
                .range(0.0, 1.0, 1.0)
                .default(f64::from(u8::from(host)))
                .at(f64::from(u8::from(self.art_v2))),
        ]
    }

    fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        match name {
            "art" => {
                self.art_v2 = value >= 0.5;
                settings::set(Settings { art_v2: self.art_v2, ..settings::get() });
                Some(f64::from(u8::from(self.art_v2)))
            }
            _ => None,
        }
    }

    fn dev_stats(&self, out: &mut Vec<(&'static str, f64)>) {
        out.push(("corrections", f64::from(self.corrections)));
        out.push(("corrected_units", (self.corrected * 1000.0).round() / 1000.0));
        // Where our knight was last drawn, in run units: a probe times a
        // walk by it whatever the camera does.
        if let Some((x, y)) = self.drawn {
            out.push(("own_x", (f64::from(x) * 1000.0).round() / 1000.0));
            out.push(("own_y", (f64::from(y) * 1000.0).round() / 1000.0));
        }
    }

    fn own_at(&self) -> Option<(f32, f32)> {
        self.drawn_px
    }
}

#[cfg(test)]
#[path = "../tests/client__tests.rs"]
mod tests;
