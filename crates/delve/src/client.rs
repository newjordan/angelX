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
//! - **The own knight's shots** leave it where it is drawn. A command the
//!   server has not answered that fires (`Run::predict_fire`, the sim's own
//!   `Hero::loose`) looses ghosts at once, on the next display frame for a
//!   key not yet sampled; they fly from the knight as drawn. The server's
//!   own shots are drawn from the newest tick flown on as far as the knight
//!   is ahead of it (`Run::flown`), not between two old ticks. When the
//!   server answers a command, its new shots take the ghosts' places (the
//!   nearest of the same kind within `HANDOVER_REACH`), and the gap between
//!   the two is eased out over `SHOT_EASE_MS` (the `shot_ease_ms` knob). A
//!   ghost the server never matches goes after `GHOST_GRACE_MS`, and one
//!   that meets stone or a monster goes there. Everyone else's shots, and
//!   every monster's, are drawn between two ticks as before. The host
//!   alone decides what a shot hits.
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
use crate::rules::{HZ, Hero, Input, Projectile, Run, RoomKind, Shot};
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

/// Ticks kept: the interpolation delay (50 ms is a tick and a half) and room for
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
/// A ghost and a new shot of the server's further apart than this (run
/// units, where each is drawn) are not one shot.
const HANDOVER_REACH: f32 = 2.5;
/// A ghost whose command the server answered with no shot to match waits
/// this long for one (the server loosed it a tick later), then goes.
const GHOST_GRACE_MS: f64 = 100.0;
/// The gap between a ghost and the server's shot that took its place is
/// eased out over this long, unless the `shot_ease_ms` knob moved it.
pub const SHOT_EASE_MS: f64 = 120.0;
/// Ghosts kept at most (a stalled link stops piling them up).
const GHOSTS_KEPT: usize = 96;

/// The keyboard, pad and sticks of the old browser seat (guest.html), and
/// the buttons a touch layout will show.
///
/// Keys: WASD walk, the arrows aim, F fires, E meets the world and takes
/// gear, Space dashes (so does Shift with a walk), Q swings, X throws a
/// bomb, Z drinks the first card, 1-4 cast, G keeps vigil, R the ultimate,
/// M , . ; ' the five skills, V the ballot (arrows, 1-5, 0, N and Esc in
/// it), N held lays the whole floor's map over the view (the minimap is
/// always in its corner). Hold Esc / pad Menu for half a second to leave a
/// mini-game; outside mini-games Esc opens the room menu and
/// Enter/Backspace leaves.
///
/// Pad: A dashes, B swings, X bombs, Y meets the world, LB is the
/// ultimate, RB and RT fire, back keeps vigil, start is Menu, L3 the ballot, R3 drinks,
/// the d-pad walks; with LT held the face buttons and RB are the skills and
/// the d-pad the four spells, and back held is the map. The right stick aims
/// and fires.
pub const CONTROLS: &str = concat!(
    r#"{"stick":"left","aim":"right","#,
    r#""text":{"key":"KeyT","kind":2,"field":"words","hint":"Your wish, then Enter"},"#,
    r#""sheet":{"key":"Tab"},"#,
    r#""buttons":["#,
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
    r#""8":"X3","9":"MENU","10":"X10","11":"X2","12":"UP","13":"DOWN","14":"LEFT","15":"RIGHT","#,
    r#""6+0":"X5","6+1":"X6","6+2":"X7","6+3":"X8","6+5":"X9","6+8":"MAP","#,
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
    /// N in the ballot: not tried yet. Held outside it, the map: the same
    /// button, since only one of the two is ever up.
    pub const UNTRIED: u32 = MAP;
    pub const MAP_KEY: u32 = MAP;
    /// Enter, Backspace: leave the game in this room.
    pub const LEAVE: u32 = OK;
}

/// One kept tick.
struct Kept {
    tick: u32,
    run: Run,
    pose: Pose,
}

/// One of our knight's shots drawn ahead of the server: loosed by command
/// `seq` as the client steps it, from where the knight was drawn, until the
/// server's own shot takes its place.
struct Ghost {
    seq: u32,
    /// As loosed: at the muzzle, before its first step.
    shot: Projectile,
    /// When the server answered its command with no shot to match yet.
    answered: Option<f64>,
}

/// One of our knight's shots in the newest tick, as the server sent it,
/// known again from tick to tick (`wire::SAME_SHOT`): what is left to ease
/// of the gap to the ghost it replaced.
struct Mine {
    x: f32,
    y: f32,
    kind: Shot,
    /// The ghost's place less the server shot's, when it took over.
    gap: (f32, f32),
    since: f64,
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
    /// Thrown things in the air on the last frame, for a window that lifts
    /// them toward the camera (`arena::lifted`); empty unless it asks.
    lifted: Vec<arena::Lifted>,
    /// The last frame in parts, for a window that draws it itself
    /// (`arena::draw_parts_outside`); `None` unless it asks.
    scene: Option<std::sync::Arc<arena::RoomScene>>,
    /// What a page that draws stages has been sent, and the size of each
    /// picture it holds (`GameClient::stage`).
    stage: arena::stage_frame::StageMaker,
    stage_sizes: BTreeMap<u32, (u32, u32)>,
    /// Where our knight's own shots were last drawn, in frame pixels
    /// (probes: a fire key to its shot on screen, and how far from the
    /// knight it showed).
    drawn_shots_px: Vec<(f32, f32)>,
    /// The nearest of them to the knight, in run units.
    drawn_shot_gap: Option<f32>,
    ballot: Ballot,
    /// The activity menu (Esc in a room's game): the knight waits.
    menu: bool,
    /// The map key is held, with nothing else having the keys: the whole
    /// floor's map is laid over the view (`arena::MapView`). Kept here, in
    /// the seat's own screen, and never sent or run.
    map_held: bool,
    /// Ticks left asking to leave the room's game, and to skip the plate.
    leaving: u32,
    skipping: u32,
    /// The drawn view's music, and the last said to the shell.
    music: Option<&'static str>,
    said_music: Option<Option<&'static str>>,
    /// A change in this seat's room allowance, said once by the shell.
    wish_notice: Option<&'static str>,
    /// The tick each sound last played on.
    sounded: BTreeMap<String, u32>,
    /// Every frame exactly as `arena::frame_for` draws it (a screenshot, a
    /// test), rather than live (`frame_for_live`).
    exact: bool,
    /// The duel state actually drawn when the next counter is sampled.
    latch_seen: Option<(u64, i8)>,
    moss_seen: Option<u64>,
    brass_seen: Option<u64>,
    rafter_seen: Option<u64>,
    /// Corrections of our knight so far, and how far they moved it in all,
    /// for a developer's panel.
    corrections: u32,
    corrected: f64,
    /// Our knight's shots ahead of the server, oldest first; the server's,
    /// in the order the newest tick has them, and that tick (run ticks).
    ghosts: Vec<Ghost>,
    mine: Vec<Mine>,
    mine_tick: Option<u64>,
    /// Where the ghosts fly: the party, the floor and the room (a door
    /// leaves them behind).
    ghost_place: Option<(u64, u32, usize)>,
    /// What this frame's steps ahead loosed (command, shot), the coming
    /// tick's step taken by the knight then, and the own shots drawn.
    loosed: Vec<(u32, Projectile)>,
    lead: (f32, f32),
    drawn_shots: Vec<Projectile>,
    /// The `shot_ease_ms` knob.
    shot_ease_ms: f64,
    /// For a developer's panel: ghosts handed to the server's shots and the
    /// gaps eased in all, and ghosts the server never matched.
    handovers: u32,
    /// The key of the room item the tree last carried (`wire::WORLD_ITEM`).
    world_item: u64,
    /// The key of the theme's patches the tree last carried (`wire::WORLD_THEME`).
    world_theme: u64,
    /// The keys of the kingdom's chunks the tree last carried (`wire::take_world_chunks`).
    world_chunks: Vec<u64>,
    handed: f64,
    unmatched: u32,
    /// Every shot the last frame drew: whether it was our knight's, and
    /// where (tests).
    #[cfg(test)]
    drawn_all: Vec<(bool, f32, f32)>,
}

impl DelveClient {
    pub fn new(seat: Option<u32>, params: Params) -> DelveClient {
        // The host's art set; whatever else this process set stands (a host's
        // own seat shares its process, and its feel ledger, with the host).
        settings::set(Settings { art_v2: params.art_v2, ..settings::get() });
        // And how far down rooms stand up in 3D (docs/WORLD-3D.md).
        crate::world3d::room::set_mode(params.mode());
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
            lifted: Vec::new(),
            scene: None,
            // Shots as flat sprites until the page says it draws props; a
            // room in 3D's plate as its PNG, which the page decodes itself.
            stage: arena::stage_frame::StageMaker::for_wire(),
            stage_sizes: BTreeMap::new(),
            drawn_shots_px: Vec::new(),
            drawn_shot_gap: None,
            ballot: Ballot::default(),
            menu: false,
            map_held: false,
            leaving: 0,
            skipping: 0,
            music: None,
            said_music: None,
            wish_notice: None,
            sounded: BTreeMap::new(),
            exact: false,
            latch_seen: None,
            moss_seen: None,
            brass_seen: None,
            rafter_seen: None,
            corrections: 0,
            corrected: 0.0,
            ghosts: Vec::new(),
            mine: Vec::new(),
            mine_tick: None,
            ghost_place: None,
            loosed: Vec::new(),
            lead: (0.0, 0.0),
            drawn_shots: Vec::new(),
            shot_ease_ms: SHOT_EASE_MS,
            handovers: 0,
            world_item: 0,
            world_theme: 0,
            world_chunks: Vec::new(),
            handed: 0.0,
            unmatched: 0,
            #[cfg(test)]
            drawn_all: Vec::new(),
        }
    }

    /// Draw every frame exactly as `arena::frame_for` does, the torch step
    /// included, rather than live.
    pub fn exact(mut self) -> DelveClient {
        self.exact = true;
        self
    }

    /// The frame's size: the art set's own (the GPU scales it), the party's
    /// band over the room's view and the HUD's band under it.
    pub fn size(&self) -> (u32, u32) {
        let (w, h) = (arena::NATIVE_W as u32, (arena::NATIVE_H + arena::BAND_H + arena::HUD_H) as u32);
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

    /// Thrown things in the air on the last frame drawn, in its pixels
    /// (`arena::lifted`); empty unless `arena::draw_lifted_outside` is on.
    pub fn lifted(&self) -> &[arena::Lifted] {
        &self.lifted
    }

    /// The last frame drawn, in parts (`arena::RoomScene`); `None` unless
    /// `arena::draw_parts_outside` is on, when the frame itself is not drawn.
    pub fn scene(&self) -> Option<std::sync::Arc<arena::RoomScene>> {
        self.scene.clone()
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

    /// Whether the full map is up: its key is held, and neither the ballot
    /// nor the activity menu has the keys. It changes only this seat's
    /// picture.
    pub fn map_open(&self) -> bool {
        self.map_held
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
        let coming = self.coming(&view.hands);
        self.loosed.clear();
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
        // Each command steps the knight, then its fire key as the sim's
        // would: what it looses flies ahead as ghosts (`own_shots`).
        let mut fired = Vec::new();
        let first = newest.tick + 1;
        for (i, (seq, cmd)) in view.pending.iter().enumerate() {
            let input = cmd.input();
            newest.predict(knight, input);
            newest.predict_fire(knight, input, first + i as u64, &mut fired);
            self.loosed.extend(fired.drain(..).map(|shot| (*seq, shot)));
            follow(newest);
        }
        let after = newest.players.get(&knight).map(|h| (h.x, h.y, h.camera))?;
        self.lead = (0.0, 0.0);
        // Part of the coming tick, along the hands held now: held only, as
        // a press acts once, when its command is sampled. The whole tick is
        // walked, then drawn `partial` of the way, so what shows is on the
        // path the server will step (a part-tick slide can reach where a
        // whole one stops at a wall).
        if let Some(walk) = walk.filter(|_| view.partial > 0.0) {
            newest.predict(knight, walk);
            // The coming command's fire key, the presses since the last
            // command with it: a shot shows on the next display frame.
            if let Some(coming) = coming
                && let Some(hero) = newest.players.get(&knight)
            {
                self.lead = (hero.x - after.0, hero.y - after.1);
                let last = view.pending.last().map_or(view.ack, |p| p.0);
                newest.predict_fire(knight, coming, first + view.pending.len() as u64, &mut fired);
                self.loosed.extend(fired.drain(..).map(|shot| (last.wrapping_add(1), shot)));
            }
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

    /// The coming command's input, as `command` will make it of these hands
    /// (held and pressed alike), or nothing while a menu, the ballot or a
    /// plate has the keys.
    fn coming(&self, hands: &Hands) -> Option<Input> {
        self.walk(hands)?;
        let cmd = DelveCmd { held: play(hands.held, hands.held, hands), pressed: presses(hands.pressed, hands.held) };
        Some(cmd.input())
    }

    /// Where knight `knight` is: the party, the floor and the room.
    fn place_of(run: &Run, knight: u32) -> (u64, u32, usize) {
        let at = run.exits.outside.get(&knight).map_or(run.at, |e| e.at);
        (run.raid_id, run.dungeon.depth, at)
    }

    /// Our knight's own shots for this frame, into `drawn_shots`: the
    /// server's from the newest tick flown on to where the knight is drawn,
    /// and the ghosts it has not yet answered. A command that still fires
    /// keeps the ghosts it first loosed (the coming command's are made anew
    /// each frame); the server's new shots take the places of the ghosts of
    /// the commands it answered; ghosts that went nowhere go.
    fn own_shots(&mut self, view: &View<DelveCmd>) {
        self.drawn_shots.clear();
        let (Some(knight), Some(newest)) = (self.knight, self.kept.back().map(|k| &k.run)) else {
            return;
        };
        let place = Self::place_of(newest, knight);
        if self.ghost_place != Some(place) {
            self.ghost_place = Some(place);
            self.ghosts.clear();
        }
        let now = view.now_ms;
        let last = view.pending.last().map_or(view.ack, |p| p.0);
        let coming = last.wrapping_add(1);
        let answered = |seq: u32| seq.wrapping_sub(view.ack) as i32 <= 0;
        let partial = view.partial.clamp(0.0, 1.0);
        // Flight, in ticks: a ghost's since its command played (the coming
        // command's part of a tick, from where the knight is drawn now: the
        // coming step not yet taken); the server's shots', from the newest
        // tick to where the knight is drawn.
        let ahead = view.pending.len() as f32 + partial;
        let lead = (self.lead.0 * (1.0 - partial), self.lead.1 * (1.0 - partial));
        let ghost_at = |g: &Ghost| {
            let (ticks, from) = if g.seq == coming {
                (partial, (g.shot.x - lead.0, g.shot.y - lead.1))
            } else {
                (last.wrapping_sub(g.seq) as f32 + 1.0 + partial, (g.shot.x, g.shot.y))
            };
            newest.flown(knight, &g.shot, from, ticks).map(|at| (at, ticks))
        };

        let loosed = &self.loosed;
        let fires = |seq: u32| loosed.iter().any(|(s, _)| *s == seq);
        self.ghosts.retain(|g| answered(g.seq) || (g.seq != coming && fires(g.seq)));
        let held: Vec<u32> = self.ghosts.iter().map(|g| g.seq).collect();
        for (seq, shot) in loosed.iter().filter(|(seq, _)| !held.contains(seq)) {
            self.ghosts.push(Ghost { seq: *seq, shot: shot.clone(), answered: None });
        }
        let over = self.ghosts.len().saturating_sub(GHOSTS_KEPT);
        self.ghosts.drain(..over);

        // A new tick: the server's shots known again by where last tick's
        // would be; a new one takes the place of the nearest ghost of the
        // same kind whose command the server has answered.
        if self.mine_tick != Some(newest.tick) {
            let step = 1.0 / HZ as f32;
            let dt = self.mine_tick.map_or(0.0, |t| newest.tick.wrapping_sub(t) as f32) * step;
            let mut was = std::mem::take(&mut self.mine);
            for shot in newest.projectiles.iter().filter(|p| p.loosed_by(knight)) {
                let same = was
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.kind == shot.kind)
                    .map(|(i, m)| (i, (m.x + shot.vx * dt - shot.x).hypot(m.y + shot.vy * dt - shot.y)))
                    .filter(|&(_, d)| d <= wire::SAME_SHOT)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                let (gap, since) = match same {
                    Some((i, _)) => {
                        let m = was.swap_remove(i);
                        (m.gap, m.since)
                    }
                    None => {
                        let server = (shot.x + shot.vx * step * ahead, shot.y + shot.vy * step * ahead);
                        let ghost = self
                            .ghosts
                            .iter()
                            .enumerate()
                            .filter(|(_, g)| answered(g.seq) && g.shot.kind == shot.kind)
                            .filter_map(|(i, g)| ghost_at(g).map(|((x, y), _)| (i, (x - server.0, y - server.1))))
                            .filter(|(_, (dx, dy))| dx.hypot(*dy) <= HANDOVER_REACH)
                            .min_by(|a, b| a.1.0.hypot(a.1.1).total_cmp(&b.1.0.hypot(b.1.1)));
                        match ghost {
                            Some((i, gap)) => {
                                self.ghosts.remove(i);
                                self.handovers += 1;
                                self.handed += f64::from(gap.0.hypot(gap.1));
                                (gap, now)
                            }
                            None => ((0.0, 0.0), now),
                        }
                    }
                };
                self.mine.push(Mine { x: shot.x, y: shot.y, kind: shot.kind, gap, since });
            }
            self.mine_tick = Some(newest.tick);
        }

        // A ghost the server answered with nothing to match goes after a
        // little; one that met stone or a monster goes at once.
        for g in &mut self.ghosts {
            if answered(g.seq) && g.answered.is_none() {
                g.answered = Some(now);
            }
        }
        let was = self.ghosts.len();
        self.ghosts.retain(|g| g.answered.is_none_or(|at| now - at <= GHOST_GRACE_MS));
        self.unmatched += (was - self.ghosts.len()) as u32;

        let ease = self.shot_ease_ms;
        let mine = newest.projectiles.iter().filter(|p| p.loosed_by(knight));
        for (m, shot) in self.mine.iter().zip(mine) {
            let Some((x, y)) = newest.flown(knight, shot, (shot.x, shot.y), ahead) else {
                continue;
            };
            let left = if ease > 0.0 { (1.0 - (now - m.since) / ease).clamp(0.0, 1.0) as f32 } else { 0.0 };
            let mut drawn = shot.clone();
            (drawn.x, drawn.y) = (x + m.gap.0 * left, y + m.gap.1 * left);
            drawn.ttl = drawn.ttl.saturating_sub(ahead as u32);
            self.drawn_shots.push(drawn);
        }
        let drawn = &mut self.drawn_shots;
        self.ghosts.retain(|g| {
            let Some(((x, y), ticks)) = ghost_at(g) else {
                return false;
            };
            let mut shot = g.shot.clone();
            (shot.x, shot.y) = (x, y);
            shot.ttl = shot.ttl.saturating_sub(ticks as u32);
            drawn.push(shot);
            true
        });
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
/// Side-on (a ledge hall, a Wanted showdown) the keys read as a
/// platformer's: Space jumps, up aims and shoots straight up, down drops
/// through a plank, and Shift with a direction rolls.
fn side_on_keys(input: &mut Input, held: u32) {
    let up = input.move_y < 0;
    let jump = held & button::JUMP != 0;
    input.move_y = if jump {
        -1
    } else if input.move_y > 0 {
        1
    } else {
        0
    };
    input.dash = held & button::SHIFT != 0 && input.move_x != 0;
    if up {
        (input.aim_x, input.aim_y) = (0, -1);
        input.fire = true;
    }
}

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
        let client = DelveClient::new(welcome.seat, params);
        arena::slot_art::warm_all();
        Ok(client)
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
        // The room on show's item, when the host sends one (a page has no
        // asset folder): kept, once for each item.
        if let Some(Value::U(key)) = tree.field(wire::WORLD_ITEM).and_then(|i| i.field("key"))
            && self.world_item != *key
        {
            self.world_item = *key;
            wire::take_world_item(tree);
        }
        // The theme's patches the rooms with no item are drawn with: kept,
        // once for each.
        if let Some(Value::U(key)) = tree.field(wire::WORLD_THEME).and_then(|t| t.field("key"))
            && self.world_theme != *key
        {
            self.world_theme = *key;
            wire::take_world_theme(tree);
        }
        // The kingdom's chunks near this seat's view, when the world is in
        // 3D out there: kept as the tree carries them.
        let chunk_keys: Vec<u64> = match tree.field(wire::WORLD_CHUNKS) {
            Some(Value::Map(entries)) => entries.iter().filter_map(|(_, c)| match c.field("key") {
                Some(Value::U(key)) => Some(*key),
                _ => None,
            }).collect(),
            _ => Vec::new(),
        };
        if chunk_keys != self.world_chunks {
            wire::take_world_chunks(tree);
            self.world_chunks = chunk_keys;
        }
        if !wire::drawable(&run) {
            return Err(Malformed);
        }
        if self.kept.is_empty() {
            arena::prepare_entry(&run, self.knight);
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
        if let (Some(id), Some(previous), Some(window)) = (self.knight, self.newest(), run.window.as_ref()) {
            let same_window = previous.raid_id == run.raid_id
                && previous.dungeon.depth == run.dungeon.depth
                && previous.window.as_ref().is_some_and(|w| w.opened == window.opened);
            let spent = window.wished.contains(&id);
            if same_window && spent && !previous.window.as_ref().unwrap().wished.contains(&id) {
                self.wish_notice =
                    Some(if run.can_wish(id) { "Wish spent: more held. Press T." } else { "Wish spent: none held." });
            } else if !same_window && run.can_wish(id) {
                self.wish_notice = Some(if window.great {
                    "A super wish: two steps at once. Press T."
                } else {
                    "Wishes given. Press T."
                });
            }
        }
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
        let hold_leave = run.can_hold_leave(knight);
        let plate = run.floor_plate(Some(knight)).is_some();
        // Only a knight standing in a live delve plays: a fallen one, or a
        // run that is won or wiped, sends nothing but its way out.
        let playable = run.active() && run.players.get(&knight).is_some_and(|h| h.hp > 0);
        let over = run.over();
        let side_on = run.side_on();

        // A press while the floor's plate shows dismisses it, and nothing else.
        if plate && pressed != 0 {
            self.skipping = ASK_TICKS;
        }
        if hold_leave { self.menu = false; }
        // Outside mini-games Esc pauses this room (Esc again resumes); Enter or
        // Backspace leaves it, paused or not. The ballot shuts first.
        if can_leave && !hold_leave && !plate {
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
        if hold_leave && hands.held & button::MENU != 0 {
            self.menu = false;
            cmd.held.leave_held = true;
            return cmd;
        }
        if hold_leave && pressed & button::MENU != 0 {
            return cmd;
        }
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
            if side_on {
                side_on_keys(&mut cmd.held, hands.held);
                // A tap of Space is a jump here, never a roll.
                cmd.pressed.dash = pressed & button::SHIFT != 0 && cmd.held.move_x != 0;
            }
        }
        if let Some(run)=self.newest().filter(|r|r.mossbell_showing()) {
            if pressed & (button::FIRE | button::LEFT | button::RIGHT | button::UP) != 0 {
                cmd.pressed.latch_tick=Some(self.moss_seen.unwrap_or(run.tick));
                cmd.pressed.move_x=cmd.held.move_x;
                cmd.pressed.move_y=cmd.held.move_y;
            }
        }
        if let Some(run) = self.newest().filter(|r| r.pennon_showing()) {
            let tick = self.rafter_seen.unwrap_or(run.tick);
            if cmd.held.fire || cmd.held.interact {cmd.held.latch_tick=Some(tick);}
            if cmd.pressed.fire || cmd.pressed.interact {cmd.pressed.latch_tick=Some(tick);}
        }
        if let Some(run) = self.newest().filter(|r| r.rafterwake_showing()) {
            if pressed & (button::FIRE | button::UP | button::DOWN) != 0 {
                cmd.pressed.latch_tick = Some(self.rafter_seen.unwrap_or(run.tick));
                cmd.pressed.move_y = cmd.held.move_y;
            }
        }
        if cmd.pressed.fire || cmd.pressed.interact {
            if let Some(run) = self.newest().filter(|r| r.brasswake_showing()) {
                cmd.pressed.latch_tick = Some(self.brass_seen.unwrap_or(run.tick));
            }
        }
        if cmd.pressed.fire {
            if let Some(run) = self.newest().filter(|r| r.latch_showing()) {
                if let Some(d) = run.latch.as_ref().and_then(|s| s.duels.get(&knight)) {
                    let (tick, guard) = self.latch_seen.unwrap_or((run.tick, d.guard));
                    cmd.pressed.latch_tick = Some(tick);
                    if cmd.held.move_y == 0 && cmd.pressed.move_y == 0 {
                        cmd.pressed.move_y = guard;
                    }
                }
            }
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
        // The map key is held (the ballot and the activity menu have it for
        // their own: N is "not tried yet" in the ballot).
        self.map_held = view.hands.held & key::MAP_KEY != 0
            && !self.menu
            && !self.ballot.open
            && self.ballot.queue.is_empty();
        let full = self.map_held;
        // Our own shots go with our knight: ahead of the server when it is,
        // between two ticks like everyone's when it is not.
        if own.is_some() {
            self.own_shots(view);
        } else {
            self.ghosts.clear();
            self.drawn_shots.clear();
        }
        let shots = std::mem::take(&mut self.drawn_shots);
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
        let mut lifted = Vec::new();
        let mut scene = None;
        let parts = arena::parts_outside();
        // The room's view between the party's band and the HUD's.
        let room_h = arena::view_rows(h as i32) as u32;
        let mut latch_seen = None;
        let mut moss_seen = None;
        let mut brass_seen = None;
        let mut rafter_seen = None;
        let mut shots_px = std::mem::take(&mut self.drawn_shots_px);
        shots_px.clear();
        let mut shot_gap = None;
        #[cfg(test)]
        let mut drawn_all = Vec::new();
        let ahead = own.as_ref().map(|_| shots.as_slice());
        // A seat's view, its HUD in the band under it (`arena::in_seat`).
        let img = arena::in_seat(|| self.kept[after].run.drawn_with_shots(&pose, alpha, knight.zip(own.as_ref()), ahead, |run| {
            let seat_view = knight.and_then(|k| run.view_for_seat(k));
            // The whole floor's map and the band's cards are the whole
            // party's, read from the run in full: a seat's own view leaves
            // out the knights who stand in another room.
            let party = seat_view.as_ref().filter(|v| v.players.len() != run.players.len()).map(|_| run.players.clone());
            let map = arena::MapView { full, model: full.then(|| run.floor_map(knight)), party };
            let run = seat_view.as_ref().unwrap_or(run);
            brass_seen = run.brasswake_showing().then_some(run.tick);
            rafter_seen = (run.rafterwake_showing() || run.pennon_showing()).then_some(run.tick);
            moss_seen = run.mossbell_showing().then_some(run.tick);
            latch_seen = knight.and_then(|id| run.latch.as_ref()?.duels.get(&id))
                .map(|d| (run.tick, d.guard));
            music = run.music();
            drawn_px = own.as_ref().map(|hero| {
                let (x, y) = arena::seat_px(run, w as i32, h as i32, knight, hero.x, hero.y);
                (x as f32, y as f32)
            });
            if let Some(k) = knight {
                let mine = || run.projectiles.iter().filter(|p| !p.hostile && u32::from(p.traits.owner) == k);
                shot_gap = own.as_ref().and_then(|hero| {
                    mine().map(|p| (p.x - hero.x).hypot(p.y - hero.y)).min_by(f32::total_cmp)
                });
                shots_px.extend(mine().map(|p| {
                    let (x, y) = arena::seat_px(run, w as i32, h as i32, knight, p.x, p.y);
                    (x as f32, y as f32)
                }));
            }
            #[cfg(test)]
            drawn_all.extend(run.projectiles.iter().map(|p| (knight.is_some_and(|k| p.loosed_by(k)), p.x, p.y)));
            if arena::lifted_outside() {
                lifted = arena::seat_lifted(run, w as i32, h as i32, knight, alpha);
            }
            if parts {
                // The window draws the parts; no picture is made here. Floor
                // fixtures are solid shapes only in a room that stands up
                // in 3D (the deep floors, `world3d::room::stands`); the 2D
                // world keeps its own sprites.
                arena::props_meshes(!self.stage.flat_shots && crate::world3d::room::stands_here(run));
                // What lies on the floor turns there as a card, in 2D too.
                arena::pickup_cards(!self.stage.flat_shots);
                let mut parts = if exact {
                    arena::scene_for(run, w as i32, room_h as i32, knight)
                } else {
                    arena::scene_for_live(run, w as i32, room_h as i32, knight)
                };
                // Only this scene is drawn for a page that draws props.
                arena::props_meshes(false);
                arena::pickup_cards(false);
                arena::presence_marks(&mut parts.screen, run, knight);
                crate::render::escape::paint(&mut parts.screen, run, knight);
                scene = Some(std::sync::Arc::new(arena::seat_lift_with(parts, run, knight, &map)));
                return None;
            }
            let mut img = if exact {
                arena::frame_for(run, w as i32, room_h as i32, knight)
            } else {
                arena::frame_for_live(run, w as i32, room_h as i32, knight)
            };
            // Who is here: "AWAY" over an absent player's knight, and the
            // room's banner while it holds for the party.
            arena::presence_marks(&mut img, run, knight);
            crate::render::escape::paint(&mut img, run, knight);
            Some(arena::seat_lift_frame_with(&img, run, knight, &map))
        }));
        self.kept[before].pose = pose;
        self.drawn_shots = shots;
        #[cfg(test)]
        {
            self.drawn_all = drawn_all;
        }
        self.music = music;
        self.drawn_px = drawn_px;
        self.lifted = lifted;
        self.scene = scene;
        self.latch_seen = latch_seen;
        self.moss_seen = moss_seen;
        self.brass_seen = brass_seen;
        self.rafter_seen = rafter_seen;
        self.drawn_shots_px = shots_px;
        self.drawn_shot_gap = shot_gap;
        if let Some(img) = img
            && img.w as u32 == w
            && img.h as u32 == h
        {
            for (out, &word) in frame.px.iter_mut().zip(img.words()) {
                // Transparent is the black paper.
                *out = if word >> 24 == 0 { 0xff00_0000 } else { word };
            }
        }
    }

    fn lines(&mut self, lines: &mut Vec<String>) {
        if let Some(notice) = self.wish_notice.take() {
            lines.push(format!("notice {notice}"));
        }
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
            Knob::new("shot_ease_ms", "net", "Own shot handover eases (ms)", "SHOT_EASE_MS (angel-delve client.rs)", Side::Client)
                .range(0.0, 500.0, 10.0)
                .default(SHOT_EASE_MS)
                .at(self.shot_ease_ms),
        ]
    }

    fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        match name {
            "art" => {
                self.art_v2 = value >= 0.5;
                settings::set(Settings { art_v2: self.art_v2, ..settings::get() });
                Some(f64::from(u8::from(self.art_v2)))
            }
            "shot_ease_ms" => {
                self.shot_ease_ms = value.clamp(0.0, 500.0);
                Some(self.shot_ease_ms)
            }
            _ => None,
        }
    }

    fn dev_stats(&self, out: &mut Vec<(&'static str, f64)>) {
        out.push(("corrections", f64::from(self.corrections)));
        out.push(("corrected_units", (self.corrected * 1000.0).round() / 1000.0));
        // Own shots drawn ahead: ghosts flying now, ghosts the server's
        // shots took over and the gaps eased in all, and ghosts it never
        // matched.
        out.push(("shot_ghosts", self.ghosts.len() as f64));
        out.push(("shot_handovers", f64::from(self.handovers)));
        out.push(("shot_handed_units", (self.handed * 1000.0).round() / 1000.0));
        out.push(("shot_unmatched", f64::from(self.unmatched)));
        // Where our knight was last drawn, in run units: a probe times a
        // walk by it whatever the camera does.
        if let Some((x, y)) = self.drawn {
            out.push(("own_x", (f64::from(x) * 1000.0).round() / 1000.0));
            out.push(("own_y", (f64::from(y) * 1000.0).round() / 1000.0));
        }
        // Our knight's own shots on the last frame, and the nearest one's
        // distance from the knight there, in frame pixels: a probe times a
        // fire key to its shot by them, and sees whether it left the knight.
        out.push(("own_shots", self.drawn_shots_px.len() as f64));
        if let Some((kx, ky)) = self.drawn_px
            && let Some(&(x, y)) = self.drawn_shots_px.iter().min_by(|a, b| {
                (a.0 - kx).hypot(a.1 - ky).total_cmp(&(b.0 - kx).hypot(b.1 - ky))
            })
        {
            let px = |v: f32| (f64::from(v) * 10.0).round() / 10.0;
            out.push(("own_shot_gap_px", px((x - kx).hypot(y - ky))));
            out.push(("own_shot_dx_px", px(x - kx)));
            out.push(("own_shot_dy_px", px(y - ky)));
        }
        if let Some(gap) = self.drawn_shot_gap {
            out.push(("own_shot_gap", (f64::from(gap) * 1000.0).round() / 1000.0));
        }
    }

    fn own_at(&self) -> Option<(f32, f32)> {
        self.drawn_px
    }

    fn sheet(&self) -> String {
        match (self.knight, self.kept.back()) {
            (Some(knight), Some(kept)) => sheet(&kept.run, knight),
            _ => String::new(),
        }
    }

    fn stage(&mut self, now_ms: f64, out: &mut Vec<u8>) -> bool {
        // From the next frame on, frames come in parts.
        arena::draw_parts_outside(true);
        let Some(scene) = self.scene.clone() else {
            return false;
        };
        let mut frame = self.stage.make(&scene, |_| None);
        for p in &frame.pictures {
            self.stage_sizes.insert(p.id, (p.width, p.height));
        }
        let sizes = &self.stage_sizes;
        // The props (and a room in 3D, and its figures) lie after some of the
        // layers; the wire counts quads.
        let props = frame.props.as_ref();
        let world = props.and_then(|p| p.world.as_ref());
        let marks = [props.map_or(0, |p| p.at), world.map_or(0, |w| w.at), world.map_or(0, |w| w.figures_at)];
        let (quads, under) =
            angel_stage::expand_marks(&frame.layers, &marks, (now_ms / 1000.0) as f32, |id| sizes.get(&id).copied());
        if let Some(props) = &mut frame.props {
            props.at = under[0];
            // (The world's floor and blocks go by their set's reference: the
            // page makes them itself.)
            if let Some(world) = &mut props.world {
                (world.at, world.figures_at) = (under[1], under[2]);
            }
        }
        angel_stage::wire::encode(&frame, &quads, out);
        true
    }

    fn stage_reset(&mut self) {
        self.stage.forget();
        self.stage_sizes.clear();
    }

    fn stage_props(&mut self, on: bool) {
        if self.stage.flat_shots == on {
            // A page that changed how it draws has none of the other way's pictures.
            self.stage.flat_shots = !on;
            self.stage.forget();
            self.stage_sizes.clear();
        }
    }
}

/// The card screen a page shows on Tab: the knight's arm, held cards, hand
/// and spells, what they add up to, whether a wish is open, and the wishes
/// the scroll knows, each with how far up its ladder this knight is.
pub fn sheet(run: &Run, knight: u32) -> String {
    use std::fmt::Write;
    let Some(hero) = run.players.get(&knight) else {
        return String::new();
    };
    let name = |id: &str| run.book.get(id).map_or_else(|| id.to_string(), |c| c.name.clone());
    let rules = |id: &str| {
        run.book.get(id).map_or_else(String::new, |c| {
            c.effects.iter().map(|e| e.label()).collect::<Vec<_>>().join(", ")
        })
    };
    let mut out = String::new();
    let rank_line = crate::rules::school::sheet_rank(&run.home.school, hero);
    let _ = writeln!(out, "{}   health {}/{}\n{}", hero.name, hero.hp, hero.max_hp, rank_line);
    let arm = hero.arm.as_deref().map_or_else(|| format!("{:?}", hero.weapon), name);
    let _ = writeln!(out, "Weapon  {arm}");
    let _ = writeln!(out, "\nHELD CARDS");
    if hero.deck.is_empty() {
        out.push_str("  none yet: walk over a card to take it\n");
    }
    let mut held: BTreeMap<&str, usize> = BTreeMap::new();
    for id in &hero.deck {
        *held.entry(id.as_str()).or_default() += 1;
    }
    for (id, n) in held {
        let copies = if n > 1 { format!(" x{n}") } else { String::new() };
        let _ = writeln!(out, "  {}{copies}  {}", name(id), rules(id));
    }
    if !hero.hand.is_empty() {
        let _ = writeln!(out, "\nHAND  (Z plays the first)");
        for id in &hero.hand {
            let _ = writeln!(out, "  {}  {}", name(id), rules(id));
        }
    }
    if hero.spells.iter().any(Option::is_some) {
        let _ = writeln!(out, "\nSPELLS");
        for (slot, spell) in hero.spells.iter().enumerate() {
            if let Some(id) = spell {
                let _ = writeln!(out, "  {}  {}  {}", slot + 1, name(id), rules(id));
            }
        }
    }
    let b = &hero.bonus;
    let totals: Vec<String> = [
        ("damage +%", b.damage),
        ("rate +%", b.rate),
        ("speed +%", b.speed),
        ("pierce", b.pierce),
        ("shots", b.shots),
        ("armor", b.armor),
        ("lifesteal", b.vamp),
        ("bounce", b.bounce),
        ("homing", b.homing),
        ("chain", b.chain),
        ("mend", b.mend),
        ("orbit", b.orbit),
        ("burst %", b.burst),
        ("fork", b.fork),
        ("fart", b.fart),
        ("size", b.size),
        ("knock", b.knock),
        ("burn", b.burn),
        ("chill", b.chill),
        ("poison", b.poison),
        ("boomerang", b.boomerang),
        ("velocity +%", b.velocity),
    ]
    .iter()
    .filter(|(_, v)| *v > 0)
    .map(|(k, v)| format!("{k} {v}"))
    .chain((b.volley > 0).then(|| format!("every {}th shot three wide", b.volley)))
    .collect();
    if !totals.is_empty() {
        let _ = writeln!(out, "\nALL TOGETHER  {}", totals.join(" · "));
    }
    if let Some(wish) = run.wish_hud(knight) {
        let _ = writeln!(out, "\nWISHES  {} held: {} (three at the start, two each floor, a super wish each boss)", wish.remaining(), wish.state());
    }
    let open = if run.can_wish(knight) {
        "A WISH IS HELD: press T and say it your way"
    } else {
        "No wishes held: each floor brings two, and its boss a super wish"
    };
    // The wishes this knight made in their own words, apart from the
    // scroll's: what each is now, from the run's own cards (a seat's copy
    // of the phrasebook may not know a wish the host's scroll learned).
    let mine: Vec<&str> = run
        .makers
        .iter()
        .filter(|(_, (k, who))| *k == knight || *who == hero.name)
        .map(|(id, _)| id.as_str())
        .collect();
    let _ = writeln!(out, "\nYOUR WISHES");
    if mine.is_empty() {
        out.push_str("  none yet: in a cleared room press T and wish for anything, in your words\n");
    }
    for id in &mine {
        let prefix = format!("wish-{id}-");
        let held = hero.deck.iter().rev().find(|c| c.starts_with(&prefix));
        let said = match held {
            Some(card) => format!("  {:<18} {}", name(card), rules(card)),
            None => {
                let title = crate::rules::phrasebook::get(id).map_or_else(|| id.to_string(), |w| w.name);
                format!("  {title:<18} not held this run")
            }
        };
        let _ = writeln!(out, "{said}");
    }
    let _ = writeln!(out, "\nWISHES  {open}");
    for wish in crate::rules::phrasebook::book().iter().filter(|w| !mine.contains(&w.id.as_str())) {
        let tiers = wish.tiers.len();
        let level = (0..tiers)
            .rev()
            .find(|&t| hero.deck.contains(&wish.card_id(t)))
            .map_or(0, |t| t + 1);
        let lock = crate::rules::school::wish_locked(&run.home.school, hero, &wish.id, level);
        let next = match &lock {
            Some(why) => format!("LOCKED: {why}"),
            None if level < tiers => format!("next: {}", wish.tiers[level].1),
            _ => "full".to_string(),
        };
        let _ = writeln!(out, "  {:<18} {level}/{tiers}  {next}", wish.name);
    }
    let _ = writeln!(out, "\nSCHOOL");
    for class in crate::rules::classes::CLASSES {
        let rows = crate::rules::school::next_mission(&run.home.school, hero, &class);
        let _ = writeln!(out, "  {}", rows);
    }
    let _ = writeln!(out, "\nTEACHERS");
    for q in crate::rules::questline::book().lines.iter() {
        let power = q.power.clone();
        let note = run.home.quest_sheet(hero, &power);
        let _ = match note {
            Some(step) => writeln!(out, "  {:<18} {}", q.teacher, step),
            None => writeln!(out, "  {:<18} teaches {} at {}", q.teacher, q.reward, q.place),
        };
    }
    out
}

#[cfg(test)]
#[path = "../tests/client__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/client_stacked_wishes__tests.rs"]
mod stacked_wishes_tests;
