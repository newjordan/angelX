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

mod down;
use down::Down;
mod creator;
use creator::mirror_action;

/// The knight whose client draws a host's shared screen. A local couch
/// creator takes priority over room location; P1 wins when both are open.
/// Hosted or solo screens never switch to another knight.
pub fn screen_knight(run: &Run, couch: bool) -> u32 {
    if !couch || run.players.get(&1).is_some_and(|h| h.mirror.open) {
        1
    } else if run.players.get(&2).is_some_and(|h| h.mirror.open)
        || (run.exits.outside.contains_key(&1)
            && run.players.contains_key(&2)
            && !run.exits.outside.contains_key(&2))
    {
        2
    } else {
        1
    }
}

/// Ticks kept: the interpolation delay (50 ms is a tick and a half) and room for
/// snapshots that bunch up on the way.
const KEEP: usize = 8;
/// A page's HD drawing among the pictures it fetches (`painting_wanted`):
/// `hd/<file>`, from the host's `/hd/<file>` (docs/LADDER.md, "The
/// browser").
pub const HD_PREFIX: &str = "hd/";
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
/// bomb, Z drinks the first card and P puts it at the back (the next one
/// up), 1-4 cast, G keeps vigil, R the ultimate,
/// M , . ; ' the five skills, C climbs on a friend's shoulders (and hops
/// off, or throws the top rider for a carrier), V the ballot (arrows, 1-5, 0, N and Esc in
/// it), N held lays the whole floor's map over the view (the minimap is
/// always in its corner). Hold Esc / pad Menu for half a second to leave a
/// mini-game; outside mini-games Esc opens the room menu and
/// Enter/Backspace leaves.
/// While fallen, Left/Right watch living allies, Space watches the body,
/// and Esc opens the down menu (Up/Down, Enter, or 1-4). N still holds the map.
/// The Gravecaller's guard also raises his Skeleton Wall; hold vigil for
/// six tenths of a second to spend his corpse gauge on a golem.
///
/// Pad: A dashes, B swings, X bombs, Y meets the world, LB is the
/// ultimate, RB and RT fire, back keeps vigil, start is Menu, L3 the ballot, R3 drinks,
/// the d-pad walks; with LT held the face buttons and RB are the skills and
/// the d-pad the four spells, LB the ride key, R3 the next-card key, and back held is the map. The right stick aims
/// and fires.
///
/// A page lays its windows by the rest: the wish line's title, placeholder
/// and keys (`text`), the way out of the offers (`choices`), the action a
/// card dragged onto another in the hand goes as (`drag`: `{from, to}`, onto
/// the plates the `slots` line says), and `view`, the native
/// rows the seat's bands take above and below the room's view
/// (`party::BAND_H`, `party::HUD_H`), so a line over the room keeps off them.
macro_rules! controls {
    ($title:literal, $placeholder:literal, $send:literal) => {
        concat!(
            r#"{"stick":"left","aim":"right","#,
            r#""text":{"key":"KeyT","kind":2,"field":"words","hint":"T: wish, or a short name on the mirror Name row; Enter to send","#,
            r#""title":""#, $title, r#"","placeholder":""#, $placeholder,
            r#"","send":""#, $send, r#"","cancel":"let it go"},"#,
            r#""choices":{"back":"Roll it up"},"#,
            r#""sheet":{"key":"Tab"},"#,
            r#""drag":{"kind":9},"#,
            r#""view":{"top":28,"bottom":44},"#,
            r#""buttons":["#,
            r#"{"id":"fire","label":"F","bit":16},"#,
            r#"{"id":"use","label":"E","bit":32},"#,
            r#"{"id":"dash","label":"Dash","bit":1024},"#,
            r#"{"id":"swing","label":"Q","bit":4096},"#,
            r#"{"id":"bomb","label":"X","bit":1048576},"#,
            r#"{"id":"drink","label":"Z","bit":2097152},"#,
            r#"{"id":"ult","label":"R","bit":8388608},"#,
            r#"{"id":"ride","label":"C","bit":1073741824},{"id":"team","label":"H","bit":40960}],"#,
            r#""keys":{"KeyH":"TEAM","KeyW":"UP","KeyS":"DOWN","KeyA":"LEFT","KeyD":"RIGHT","#,
            r#""ArrowUp":"AIM_UP","ArrowDown":"AIM_DOWN","ArrowLeft":"AIM_LEFT","ArrowRight":"AIM_RIGHT","#,
            r#""KeyF":"FIRE","KeyE":"USE","Space":"JUMP","Escape":"MENU","KeyQ":"ALT","#,
            r#""ShiftLeft":"SHIFT","ShiftRight":"SHIFT","KeyN":"MAP","#,
            r#""Enter":"OK","NumpadEnter":"OK","Backspace":"OK","#,
            r#""Digit1":"SLOT1","Digit2":"SLOT2","Digit3":"SLOT3","Digit4":"SLOT4","#,
            r#""Numpad1":"SLOT1","Numpad2":"SLOT2","Numpad3":"SLOT3","Numpad4":"SLOT4","#,
            r#""KeyX":"X1","KeyZ":"X2","KeyG":"X3","KeyR":"X4","#,
            r#""KeyM":"X5","Comma":"X6","Period":"X7","Semicolon":"X8","Quote":"X9","#,
            r#""KeyV":"X10","Digit5":"X11","Numpad5":"X11","Digit0":"X12","Numpad0":"X12","KeyC":"X11","KeyP":"X12"},"#,
            r#""pad":{"0":"JUMP","1":"ALT","2":"X1","3":"USE","4":"X4","5":"FIRE","7":"FIRE","#,
            r#""8":"X3","9":"MENU","10":"X10","11":"X2","12":"UP","13":"DOWN","14":"LEFT","15":"RIGHT","#,
            r#""6+0":"X5","6+1":"X6","6+2":"X7","6+3":"X8","6+5":"X9","6+8":"MAP","6+10":"TEAM","6+4":"X11","6+11":"X12","#,
            r#""6+12":"SLOT1","6+14":"SLOT2","6+15":"SLOT3","6+13":"SLOT4"},"#,
            r#""sticks":{"move":"left","aim":"right","aim_holds":"FIRE"}}"#,
        )
    };
}

/// The first steps' hints' timings (`arena::primer::Knobs`), in seconds, on
/// the dev panel: this viewer's own.
fn primer_knobs(k: &arena::primer::Knobs) -> Vec<Knob> {
    let d = arena::primer::Knobs::default();
    let secs = |t: u32| f64::from(t) / f64::from(crate::rules::HZ);
    let knob = |name: &'static str, label: &'static str, tip: &'static str, at: u32, default: u32, most: f64| {
        Knob::new(name, "first steps", label, tip, Side::Client).range(0.0, most, 0.5).default(secs(default)).at(secs(at))
    };
    vec![
        Knob::new("primer_on", "first steps", "First steps' hints (off / on)", "ARROWS: FIRE and T: MAKE A WISH for a knight new to the delve", Side::Client)
            .range(0.0, 1.0, 1.0).default(1.0).at(f64::from(u8::from(k.on))),
        knob("primer_fire_after", "Fire hint after a foe wakes (s)", "arena::primer::Knobs::fire_after", k.fire_after, d.fire_after, 10.0),
        knob("primer_wish_after", "Wish hint into the first floor (s)", "arena::primer::Knobs::wish_after", k.wish_after, d.wish_after, 30.0),
        knob("primer_wish_until", "Wish hint comes back until (s)", "arena::primer::Knobs::wish_until", k.wish_until, d.wish_until, 600.0),
        knob("primer_hold", "A hint stays up at least (s)", "arena::primer::Knobs::hold", k.hold, d.hold, 10.0),
    ]
}

fn set_primer_knob(k: &mut arena::primer::Knobs, name: &str, value: f64) -> Option<f64> {
    let ticks = |v: f64, most: f64| (v.clamp(0.0, most) * f64::from(crate::rules::HZ)).round() as u32;
    let secs = |t: u32| f64::from(t) / f64::from(crate::rules::HZ);
    match name {
        "primer_on" => {
            k.on = value >= 0.5;
            Some(f64::from(u8::from(k.on)))
        }
        "primer_fire_after" => {
            k.fire_after = ticks(value, 10.0);
            Some(secs(k.fire_after))
        }
        "primer_wish_after" => {
            k.wish_after = ticks(value, 30.0);
            Some(secs(k.wish_after))
        }
        "primer_wish_until" => {
            k.wish_until = ticks(value, 600.0);
            Some(secs(k.wish_until))
        }
        "primer_hold" => {
            k.hold = ticks(value, 10.0);
            Some(secs(k.hold))
        }
        _ => None,
    }
}

pub const CONTROLS: &str = controls!("Make a wish", "Wish for anything, in your words", "the genie offers how");
const NAME_CONTROLS: &str = controls!("Mirror name", "Type a short name, then Enter", "save name");
/// What each of the Delve's buttons is (see `CONTROLS`).
mod key {
    pub const TEAM: u32 = OK | SHIFT;
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
    /// C (LT+LB on a pad): climb on a friend's shoulders, hop off, or as
    /// a carrier throw the top rider (`shoulders`). The same button as 5
    /// in the ballot, since only one of the two is ever up: outside the
    /// ballot 5 is the ride key too.
    pub const RIDE: u32 = X11;
    pub const CLEAR: u32 = X12;
    /// P (LT+R3 on a pad): the first card in hand to the back, the next
    /// one up (`Input::cycle`). The same button as 0 in the ballot, since
    /// only one of the two is ever up: outside the ballot 0 is the
    /// next-card key too.
    pub const CYCLE: u32 = X12;
    /// N in the ballot: not tried yet. Held outside it, the map: the same
    /// button, since only one of the two is ever up.
    pub const UNTRIED: u32 = MAP;
    pub const MAP_KEY: u32 = MAP;
    /// Enter, Backspace: leave the game in this room.
    pub const LEAVE: u32 = OK;
}

/// Our hand's plates on the screen: each card's slot in hand and its
/// plate, `(x, y, w, h)` in the frame's pixels (`arena::seat_hand_plates`).
type Plates = Vec<(usize, (i32, i32, i32, i32))>;

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
    tree: Option<Value>,
    names: Reader,
    /// The run last read, kept so a patch puts back only what it named.
    guest: wire::Guest,
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
    /// The paintings the page fetches and draws (`arena::paintings`).
    paintings: arena::paintings::PagePaintings,
    /// The HD drawings of the sprites shown, from the installed pack
    /// (`arena::hd_pack`): each sent with the first frame that names it. A
    /// browser installs none (it has no folder to read one from), so its
    /// page is sent the inks, as ever.
    hd: arena::hd_pack::HdArt,
    /// A page's HD art (`hd_manifest`): the host's manifest, each drawing's
    /// pixels fetched by the page the first time a figure on its screen
    /// asks for it (`painting_wanted`'s `hd/<file>`). None until the page
    /// asks for HD art (`?hd=1`), and always in a window, whose pack is
    /// installed from its own folder.
    hd_page: Option<arena::hd_pack::Pack>,
    /// Where our knight's own shots were last drawn, in frame pixels
    /// (probes: a fire key to its shot on screen, and how far from the
    /// knight it showed).
    drawn_shots_px: Vec<(f32, f32)>,
    /// The nearest of them to the knight, in run units.
    drawn_shot_gap: Option<f32>,
    ballot: Ballot,
    /// The activity menu (Esc in a room's game): the knight waits.
    menu: bool,
    /// A fallen seat's camera and menu, never part of a command or run.
    down: Option<Down>,
    /// A wish scroll's choice tapped (1 to 3, 0 to roll it up), for the
    /// next command.
    picked: Option<u8>,
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
    /// Our hand's plates on the drawn screen (`arena::seat_hand_plates`),
    /// the tick they were read at, and the last said to the shell
    /// (`slots`), for a page that drags a card onto another.
    plates: Plates,
    plates_tick: Option<u32>,
    said_plates: Plates,
    /// A change in this seat's room allowance, said once by the shell.
    wish_notice: Option<&'static str>,
    /// Our knight rubbed the wish altar's lamp (`rules::altar`): the shell
    /// opens its wish line, once a rub.
    ask: bool,
    /// A wish of ours that landed, said once by the shell.
    granted: Option<String>,
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
    viewer: crate::render::viewer::Preferences,
    /// The first steps' hints on this seat's own view (`arena::primer`).
    primer: arena::primer::Primer,
    /// For a developer's panel: ghosts handed to the server's shots and the
    /// gaps eased in all, and ghosts the server never matched.
    handovers: u32,
    /// The key of the lit room's set the tree last carried (`wire::WORLD_ITEM`).
    world_item: u64,
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
        // How far down rooms stand up in 3D (docs/WORLD-3D.md).
        crate::world3d::room::set_mode(params.mode());
        // And whether the first floor is the Atari board (experimental).
        crate::render::overworld::arena::atari::set(params.atari);
        DelveClient {
            seat,
            knight: seat.map(wire::knight_of),
            tree: None,
            names: Reader::default(),
            guest: wire::Guest::default(),
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
            paintings: Default::default(),
            hd: Default::default(),
            hd_page: None,
            drawn_shots_px: Vec::new(),
            drawn_shot_gap: None,
            ballot: Ballot::default(),
            menu: false,
            down: None,
            picked: None,
            map_held: false,
            leaving: 0,
            skipping: 0,
            music: None,
            said_music: None,
            plates: Vec::new(),
            plates_tick: None,
            said_plates: Vec::new(),
            wish_notice: None,
            ask: false,
            granted: None,
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
            viewer: crate::render::viewer::Preferences::default(),
            primer: arena::primer::Primer::default(),
            handovers: 0,
            world_item: 0,
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

    /// The frame's size (the GPU scales it): the party's band over the
    /// room's view and the HUD's band under it.
    pub fn size(&self) -> (u32, u32) {
        (arena::NATIVE_W as u32, (arena::NATIVE_H + arena::BAND_H + arena::HUD_H) as u32)
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
        let fallen = self.knight.filter(|id| {
            run.active() && run.crossroads.is_none() && run.players.get(id).is_some_and(|h| h.hp == 0 && !h.absent)
        });
        if let Some(id) = fallen {
            self.down.get_or_insert_with(|| Down::new(&run, id)).settle(&run, id);
            self.menu = false;
            self.leaving = 0;
        } else {
            if self.down.is_some() {
                self.map_held = false;
            }
            self.down = None;
        }
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
        if newest.players.get(&knight).is_some_and(|h| h.mirror.open) {
            self.ghosts.clear();
            self.shown.clear();
            self.offset = (0.0, 0.0);
            self.lead = (0.0, 0.0);
            return newest.players.get(&knight).cloned();
        }
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

    /// Arena movement uses the same fixed-step walk as its host. Only the
    /// own camera is predicted: the host alone awards damage and kills.
    fn deathmatch_own(&self, view: &View<DelveCmd>) -> Option<(u32, crate::rules::deathmatch::Fighter)> {
        let id = self.knight?;
        let s = self.kept.back()?.run.deathmatch.as_ref()?;
        if s.ticks <= crate::rules::deathmatch::TITLE || s.winner.is_some() { return None; }
        let mut f = s.fighters.get(&id)?.clone();
        for (_, cmd) in view.pending { f.walk_arena(cmd.input(), &s.fields); }
        if let Some(input) = self.walk(&view.hands) {
            let before = (f.hero.x, f.hero.y, f.yaw, f.bob);
            f.walk_arena(input, &s.fields);
            let t = view.partial.clamp(0., 1.);
            f.hero.x = before.0 + (f.hero.x - before.0) * t;
            f.hero.y = before.1 + (f.hero.y - before.1) * t;
            let turn = (f.yaw - before.2).sin().atan2((f.yaw - before.2).cos());
            f.yaw = before.2 + turn * t;
            f.bob = before.3 + (f.bob - before.3) * t;
        }
        Some((id, f))
    }

    /// The input the hands walk with between two commands, or nothing while
    /// a menu, the ballot or a plate has the keys.
    fn walk(&self, hands: &Hands) -> Option<Input> {
        let run = self.newest()?;
        let knight = self.knight?;
        if run.players.get(&knight).is_some_and(|h| h.mirror.open)
            || self.menu || self.ballot.open || !self.ballot.queue.is_empty() || run.floor_plate(Some(knight)).is_some() {
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
    input.dash = held & button::SHIFT != 0 && input.move_x != 0 && held & key::TEAM != key::TEAM;
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
        dash: on(button::JUMP) || (on(button::SHIFT) && walking && bits & key::TEAM != key::TEAM),
        team: bits & key::TEAM == key::TEAM,
        interact: on(button::USE),
        take: on(button::USE),
        bomb: on(key::BOMB),
        swing: on(button::ALT),
        play: u8::from(slots & key::DRINK != 0),
        cast: first(&key::SPELLS),
        vigil: on(key::VIGIL),
        ult: on(key::ULT),
        skill: first(&key::SKILLS),
        ride: on(key::RIDE),
        cycle: on(key::CYCLE),
        ..Input::default()
    }
}

/// The input of the bits pressed since the last command: the flags and the
/// slots, no sticks (the sampler sweeps a tapped direction into the stick).
/// Shift pressed while walking is a dash pressed.
fn presses(pressed: u32, held: u32) -> Input {
    let walking = key::WALK.iter().any(|&k| held & k != 0);
    let mut input = play(pressed, pressed, &Hands::default());
    input.dash |= pressed & button::SHIFT != 0 && walking && (pressed | held) & key::TEAM != key::TEAM;
    input
}

/// The shortest gap between two of the same sound, in ms: rapid fire never
/// piles into noise (the host's mixer's own gaps, `together_audio`).
pub(crate) fn sound_gap(name: &str) -> u32 {
    match name {
        "gas_release" | "wheel_spin" | "wheel_land" => 1500,
        "spell_fire" | "skill_barrier" => 500,
        "wish_grant" => 1000,
        "spell_cast" => 250,
        "shot_bow" | "shot_crossbow" | "shot_handgonne" | "shot_bolt" | "shot_arcane" | "shot_spark" => 70,
        "hit" | "hit_blade" | "hit_axe" | "hit_arrow" | "hit_bolt" | "hit_blast" | "hit_magic" | "hit_note" => 60,
        n if n.starts_with("sax_") => 70,
        "bard_kick" | "bard_wub" => 300,
        n if n.starts_with("bard_glitch") => 150,
        "step_stone" | "step_gravel" | "step_ash" | "step_wet" | "step_moss" | "step_wood" => 210,
        "knight_hit" | "knight_hit_big" | "gold" | "bomb_toss" | "shield_up" => 120,
        "boss_roar" | "boss_rage" | "boss_death" | "crack_open" | "hand_rise" | "hand_grab" | "hand_drag"
        | "ground_close" | "hell_gate_rise" | "hell_gate_enter" | "circle_toll" | "knight_fall" | "boss_rise"
        | "boss_fall" | "descend" | "door_seal" | "door_open" | "area_leave" | "bard_lullaby" | "bard_overload" => 800,
        "kill" => 90,
        "gore_kick" | "gore_thunk" | "gore_chop" | "gore_splat" | "gore_bones" | "gore_burst" => 110,
        "swing" | "roll" | "shield_block" | "clang" | "spike" | "vent_fire" => 150,
        // The blades (`rules::melee`): a whoosh by the arm, a contact by its
        // weight, a turned bullet's ting and whizz.
        "swing_sword" | "swing_axe" | "swing_maul" | "swing_spear" | "swing_cleaver" => 150,
        "hit_flesh_light" | "hit_flesh_medium" | "hit_flesh_heavy" => 70,
        "parry" => 250,
        "parry_whizz" => 150,
        "gore_behead" | "gore_head_thud" | "gore_bisect" | "gore_slump" => 110,
        // Magic by family (`rules::vfx::Voice`): a cast as `spell_cast`, a
        // spell's break a little more than a shot's hit.
        "cast_fire" | "cast_frost" | "cast_holy" | "cast_storm" | "cast_poison" | "cast_arcane" | "cast_earth" => 250,
        "impact_fire" | "impact_frost" | "impact_holy" | "impact_storm" | "impact_poison" | "impact_arcane"
        | "impact_earth" => 90,
        // The wish moments: the lamp, the genie, a landing, the fairies.
        "lamp_rub" => 600,
        "genie_appear" => 800,
        "wish_land" | "wish_land_grand" | "wish_land_mythic" => 1000,
        "fairy_wish" => 1500,
        // The steps up (`rules::rewards`).
        "level_up" | "rung_up" | "relic_find" | "deepest_yet" | "sky_lit" => 1000,
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
        // Names the patch touched, when it is a struct's. `None` is a whole
        // tree, or a patch this does not know field by field.
        let mut fields = None;
        let mut shots = wire::Shots::Rebuild;
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
            fields = wire::Guest::fields(&patch);
            shots = wire::Guest::shots_of(&patch);
            delta::apply(tree, patch).map_err(|_| Malformed)?;
        }
        let tree = self.tree.as_ref().ok_or(Malformed)?;
        let run = self.guest.read(tree, fields.as_deref(), &shots).map_err(|_| Malformed)?;
        let known = fields.as_ref();
        let touched = |name: &str| known.is_none_or(|names| names.iter().any(|n| n == name));
        // The room on show's lit set, when the host sends one (a page has no
        // asset folder for its landmark): kept, once for each room.
        if touched(wire::WORLD_ITEM)
            && let Some(Value::U(key)) = tree.field(wire::WORLD_ITEM).and_then(|i| i.field("key"))
            && self.world_item != *key
        {
            self.world_item = *key;
            wire::take_world_item(tree);
        }
        // The kingdom's chunks near this seat's view, when the world is in
        // 3D out there: kept as the tree carries them. A fight does not
        // resend them, so the key list is not built again.
        if touched(wire::WORLD_CHUNKS) {
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
        if let (Some(id), Some(previous)) = (self.knight, self.newest()) {
            // A newer rub of the lamp than we had seen: open the wish line.
            let rub = run.lamp.rubbed.get(&id);
            let ask = rub.is_some() && rub != previous.lamp.rubbed.get(&id) && previous.raid_id == run.raid_id;
            // A grant of ours landed since: say so.
            let seen = previous.lamp.landed.last().map(|l| l.tick);
            let granted = run
                .lamp
                .landed
                .iter()
                .rev()
                .find(|l| l.knight == id && seen.is_none_or(|t| l.tick > t))
                .map(|landed| format!("Your wish is granted: {}", landed.name.split(". ").next().unwrap_or(&landed.name)));
            self.ask |= ask;
            if granted.is_some() {
                self.granted = granted;
            }
        }
        if let (Some(id), Some(previous), Some(window)) = (self.knight, self.newest(), run.window.as_ref()) {
            let same_window = previous.raid_id == run.raid_id
                && previous.dungeon.depth == run.dungeon.depth
                && previous.window.as_ref().is_some_and(|w| w.opened == window.opened);
            let spent = window.wished.contains(&id);
            if same_window && spent && !previous.window.as_ref().unwrap().wished.contains(&id) {
                self.wish_notice =
                    Some(if run.can_wish(id) { "Wish spent: more held. E at the genie's lamp." } else { "Wish spent: none held." });
            } else if !same_window && run.can_wish(id) && run.players.get(&id).is_some_and(|h| h.unmade(&run.book)) {
                // A knight still a shape: the wishes are for making them.
                self.wish_notice = Some("You are a shape. Press T: wish yourself into being.");
            } else if !same_window && run.can_wish(id) {
                self.wish_notice = Some(if window.great {
                    "A super wish: two steps at once. E at the genie's lamp."
                } else {
                    "Wishes given. Rub the genie's lamp (E) and wish."
                });
            }
        }
        if let Some(id) = self.knight {
            let naming = |r: &Run| r.players.get(&id).is_some_and(|h| h.mirror.open && h.mirror.row == 9);
            if naming(&run) && self.newest().is_none_or(|old| !naming(old)) {
                self.wish_notice = Some("Mirror name: press T, type a short name, then Enter.");
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
        if (hands.pressed & key::MAP_KEY != 0 && hands.held & button::SHIFT != 0)
            || (hands.held & key::MAP_KEY != 0 && hands.pressed & button::USE != 0) {
            self.viewer.calm_screen = !self.viewer.calm_screen;
            self.wish_notice = Some(if self.viewer.calm_screen { "Calm screen on" } else { "Calm screen off" });
            return DelveCmd::neutral();
        }
        let (Some(knight), Some(run)) = (self.knight, self.newest()) else {
            return DelveCmd::neutral();
        };
        // The creator owns all controls, even if an older menu/ballot or a
        // queued leave request was still up when its snapshot arrived.
        if run.players.get(&knight).is_some_and(|h| h.mirror.open) {
            self.menu = false;
            self.leaving = 0;
            self.skipping = 0;
            self.picked = None;
            self.map_held = false;
            self.ballot.queue.clear();
            self.ballot.open = false;
            let mut cmd = DelveCmd::neutral();
            // Axes are held input, not fresh presses every tick. The sim's
            // creator fallback applies its repeat delay to these directions.
            cmd.held = play(0, 0, hands);
            cmd.pressed.mirror = mirror_action(hands);
            return cmd;
        }
        // The team chord is combat input, never an Enter in a dialog.
        let pressed = if (hands.held | hands.pressed) & key::TEAM == key::TEAM {
            hands.pressed & !key::TEAM
        } else { hands.pressed };
        let can_leave = run.can_leave_activity(knight);
        let hold_leave = run.can_hold_leave(knight);
        let plate = run.floor_plate(Some(knight)).is_some();
        // Only a knight standing in a live delve plays: a fallen one, or a
        // run that is won or wiped, sends nothing but its way out.
        let playable = run.active() && run.players.get(&knight).is_some_and(|h| h.hp > 0);
        let over = run.over();
        let side_on = run.side_on();
        let scroll_open = run.players.get(&knight).is_some_and(|h| h.offer.is_some());

        if run.crossroads_voting() {
            let selected = run.playtest.seats.get(&knight).map_or(0, |s| s.selected.min(2));
            let selected = self.ballot.queue.iter().fold(selected, |selected, v| match v.action {
                Action::Left | Action::Up => (selected + 2) % 3,
                Action::Right | Action::Down => (selected + 1) % 3,
                Action::Rate(n @ 1..=3) => usize::from(n - 1),
                _ => selected,
            });
            let picked = self.picked.take().filter(|&n| (1..=3).contains(&n))
                .or_else(|| [button::SLOT1, button::SLOT2, button::SLOT3].iter()
                    .position(|bits| pressed & bits != 0).map(|i| i as u8 + 1));
            if let Some(n) = picked {
                self.vote(Action::Rate(n));
            } else if pressed & (button::LEFT | button::AIM_LEFT) != 0 {
                self.vote(Action::Left);
            } else if pressed & (button::RIGHT | button::AIM_RIGHT) != 0 {
                self.vote(Action::Right);
            } else if pressed & (button::OK | button::FIRE) != 0 {
                self.vote(Action::Rate(selected as u8 + 1));
            }
            self.menu = false;
            let mut cmd = DelveCmd::neutral();
            cmd.pressed.vote = self.ballot.queue.front().copied();
            return cmd;
        }
        if run.crossroads.is_some() {
            self.picked = None;
            self.menu = false;
            let mut cmd = DelveCmd::neutral();
            cmd.held = play(hands.held, hands.held, hands);
            cmd.pressed = presses(hands.pressed, hands.held);
            return cmd;
        }

        if !plate && self.down.is_some() {
            let mut down = self.down.take().unwrap();
            down.keys(&self.kept.back().unwrap().run, knight, pressed, &mut self.viewer);
            self.down = Some(down);
            return DelveCmd::neutral();
        }

        // A press while the floor's plate shows dismisses it, and nothing else.
        if plate && pressed != 0 {
            self.skipping = ASK_TICKS;
        }
        if hold_leave { self.menu = false; }
        // The wish scroll's choices are open on this knight's view: the
        // menu key rolls it up and the spell keys (or a tap, `choose`) pick,
        // for the run to take (`Run::offer_inputs`); nothing pauses.
        if playable && scroll_open {
            let mut cmd = DelveCmd::neutral();
            cmd.held = play(hands.held, hands.held, hands);
            cmd.pressed = presses(hands.pressed, hands.held);
            cmd.pressed.activity_menu = pressed & button::MENU != 0;
            match self.picked.take() {
                Some(0) => cmd.pressed.activity_menu = true,
                Some(n) => cmd.pressed.cast = n,
                None => {}
            }
            self.menu = false;
            return cmd;
        }
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
            cmd.pressed = presses(hands.pressed, hands.held);
            if side_on {
                side_on_keys(&mut cmd.held, hands.held);
                // A tap of Space is a jump here, never a roll.
                cmd.pressed.dash = pressed & button::SHIFT != 0 && cmd.held.move_x != 0 && (pressed | hands.held) & key::TEAM != key::TEAM;
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
        let _viewer = crate::render::viewer::enter(self.viewer);
        let (w, h) = self.size();
        frame.resize(w, h);
        if self.kept.is_empty() {
            frame.px.fill(0xff00_0000);
            return;
        }
        let own = self.own(view);
        let deathmatch_own = self.deathmatch_own(view);
        self.drawn = own.as_ref().map(|h| (h.x, h.y));
        // The map key is held (the ballot and the activity menu have it for
        // their own: N is "not tried yet" in the ballot).
        let map_held = view.hands.held & key::MAP_KEY != 0 && view.hands.held & button::SHIFT == 0;
        self.map_held = if let Some(down) = &self.down {
            down.map || map_held && down.menu.is_none()
        } else {
            map_held
            && !self.menu
            && !self.newest().is_some_and(Run::deathmatch_showing)
            && !self.ballot.open
            && self.ballot.queue.is_empty()
        };
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
        let watching = self.down.as_ref().map(|d| d.watching);
        let down_hud = self.down.as_ref().map(|d| {
            let name = self.newest().and_then(|r| r.players.get(&d.watching)).map_or("", |h| h.name.as_str());
            arena::down::Hud { name: name.to_string(), body: watching == knight, hidden: d.hidden, menu: d.menu, map: full, calm: self.viewer.calm_screen }
        });
        let exact = self.exact;
        let mut music = None;
        let mut drawn_px = None;
        let mut lifted = Vec::new();
        let mut scene = None;
        let parts = arena::parts_outside();
        let _scenery = arena::wire_scenery(parts && self.stage.png_plates);
        // The room's view between the party's band and the HUD's.
        let room_h = arena::view_rows(h as i32) as u32;
        let mut latch_seen = None;
        let mut moss_seen = None;
        let mut brass_seen = None;
        let mut rafter_seen = None;
        let mut shots_px = std::mem::take(&mut self.drawn_shots_px);
        shots_px.clear();
        let mut shot_gap = None;
        // The hand's plates are read again once a tick, not every frame.
        let plates_due = self.plates_tick != Some(self.kept[after].tick);
        let mut plates = None;
        #[cfg(test)]
        let mut drawn_all = Vec::new();
        let ahead = own.as_ref().map(|_| shots.as_slice());
        let primer = &mut self.primer;
        // A seat's view, its HUD in the band under it (`arena::in_seat`).
        let img = arena::with_deathmatch_prediction(deathmatch_own, || arena::with_watch(watching, || arena::in_seat(|| self.kept[after].run.drawn_with_shots(&pose, alpha, knight.zip(own.as_ref()), ahead, |run| {
            let seat_view = watching.or(knight).and_then(|k| run.view_for_seat(k));
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
            // A watcher has no hand to drag.
            if plates_due {
                plates = Some(if watching.is_some() { Vec::new() } else {
                    knight.map_or_else(Vec::new, |k| arena::seat_hand_plates(run, Some(k), w as i32, h as i32))
                });
            }
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
                crate::render::genie::paint(&mut parts.screen, run, knight);
                if let Some(k) = knight { primer.paint(&mut parts.screen, run, k); }
                let mut parts = arena::seat_lift_with(parts, run, knight, &map);
                if let Some(hud) = &down_hud { hud.paint(&mut parts.screen); }
                crate::render::viewer::options(&mut parts.screen, full, self.viewer);
                if knight.is_some_and(|k| run.tripping(k)) {
                    crate::render::viewer::trip_scene(&mut parts, run.tick);
                }
                crate::render::viewer::finish_scene(&mut parts);
                scene = Some(std::sync::Arc::new(parts));
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
            crate::render::genie::paint(&mut img, run, knight);
            if let Some(k) = knight { primer.paint(&mut img, run, k); }
            let mut img = arena::seat_lift_frame_with(&img, run, knight, &map);
            if let Some(hud) = &down_hud { hud.paint(&mut img); }
            crate::render::viewer::options(&mut img, full, self.viewer);
            if knight.is_some_and(|k| run.tripping(k)) {
                // The room's view alone goes strange; the bands stay read.
                let top = arena::BAND_H * (img.w / arena::NATIVE_W).max(1);
                crate::render::viewer::trip_rows(&mut img, run.tick, top, top + room_h as i32);
            }
            Some(img)
        }))));
        self.kept[before].pose = pose;
        self.drawn_shots = shots;
        #[cfg(test)]
        {
            self.drawn_all = drawn_all;
        }
        self.music = music;
        if let Some(plates) = plates {
            self.plates = plates;
            self.plates_tick = Some(self.kept[after].tick);
        }
        self.drawn_px = drawn_px;
        self.lifted = lifted;
        self.scene = scene;
        self.latch_seen = latch_seen;
        self.moss_seen = moss_seen;
        self.brass_seen = brass_seen;
        self.rafter_seen = rafter_seen;
        self.drawn_shots_px = shots_px;
        self.drawn_shot_gap = shot_gap;
        if let Some(mut img) = img
            && img.w as u32 == w
            && img.h as u32 == h
        {
            crate::render::viewer::soften(&mut img);
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
        if let Some(granted) = self.granted.take() {
            lines.push(format!("notice {granted}"));
        }
        if std::mem::take(&mut self.ask) {
            lines.push("ask".to_string());
        }
        if self.said_music != Some(self.music) {
            self.said_music = Some(self.music);
            lines.push(match self.music {
                Some(track) => format!("music {}", wire::music_file(track)),
                None => "music".to_string(),
            });
        }
        // Where our hand's plates are, whenever they move: `slots` and
        // `[slot, x, y, w, h]` for each card, in the frame's pixels. A page
        // starts with none.
        if self.said_plates != self.plates {
            self.said_plates = self.plates.clone();
            let quads: Vec<String> = self
                .plates
                .iter()
                .map(|&(slot, (x, y, w, h))| format!("[{slot},{x},{y},{w},{h}]"))
                .collect();
            lines.push(format!("slots [{}]", quads.join(",")));
        }
    }

    fn controls(&self) -> &'static str {
        if self.knight.zip(self.newest()).is_some_and(|(id, run)|
            run.players.get(&id).is_some_and(|h| h.mirror.open && h.mirror.row == 9)) {
            NAME_CONTROLS
        } else {
            CONTROLS
        }
    }

    fn knobs(&self) -> Vec<Knob> {
        vec![
            Knob::new("calm_screen", "viewer", "Calm screen (off / on)", "This viewer only: no shake, soft flashes. Shift+N toggles.", Side::Client)
                .range(0.0, 1.0, 1.0).default(0.0).at(f64::from(self.viewer.calm_screen)),
            Knob::new("shot_ease_ms", "net", "Own shot handover eases (ms)", "SHOT_EASE_MS (angel-delve client.rs)", Side::Client)
                .range(0.0, 500.0, 10.0)
                .default(SHOT_EASE_MS)
                .at(self.shot_ease_ms),
        ]
        .into_iter()
        .chain(primer_knobs(&self.primer.knobs))
        .collect()
    }

    fn set_knob(&mut self, name: &str, value: f64) -> Option<f64> {
        match name {
            "calm_screen" => {
                self.viewer.calm_screen = value >= 0.5;
                Some(f64::from(self.viewer.calm_screen))
            }
            "shot_ease_ms" => {
                self.shot_ease_ms = value.clamp(0.0, 500.0);
                Some(self.shot_ease_ms)
            }
            _ => set_primer_knob(&mut self.primer.knobs, name, value),
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

    /// The wish scroll's choices, when it is open on our knight's view:
    /// "1 As you said: Triple Volley I", one a line; and on a summoner's,
    /// whom it is for, turned with 4 ("4 For: your dead").
    fn choices(&self) -> String {
        let (Some(knight), Some(run)) = (self.knight, self.newest()) else {
            return String::new();
        };
        if let Some(c) = run.crossroads.as_ref().filter(|c| c.voting()) {
            return c.choices.iter().enumerate().map(|(i, g)| format!("{} {}: {}", i + 1, g.name(), g.words())).collect::<Vec<_>>().join("\n");
        }
        run.players
            .get(&knight)
            .and_then(|h| h.offer.as_ref())
            .map(|offer| {
                offer
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{} {}: {}", i + 1, c.head, c.gives))
                    .chain(offer.target.map(|t| format!("4 For: {}", t.word())))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    /// Choice `n` tapped; the line after the choices on a summoner's scroll
    /// turns whom it is for (4); any past the three rolls the scroll up.
    fn choose(&mut self, n: u32) {
        let turn = self
            .knight
            .zip(self.newest())
            .and_then(|(knight, run)| run.players.get(&knight))
            .and_then(|h| h.offer.as_ref())
            .is_some_and(|offer| offer.target.is_some() && n as usize == offer.choices.len());
        self.picked = Some(if turn {
            4
        } else if n < 3 {
            n as u8 + 1
        } else {
            0
        });
    }

    /// "Upgrade what you hold" beside the wish line: each ladder our knight
    /// holds (`Run::upgrades`) as "1\tPermanent Mirrors\tI → II\tTwo mirrors
    /// circle you.\tupgrade Permanent Mirrors", one held in full as MAX with
    /// no words, and the note when none can be picked.
    fn picks(&self) -> String {
        let (Some(knight), Some(run)) = (self.knight, self.newest()) else {
            return String::new();
        };
        if run.players.get(&knight).is_some_and(|h| h.mirror.open && h.mirror.row == 9) {
            return "Mirror name\nPress T, type a short name, then Enter; Esc cancels.".into();
        }
        let list = run.upgrades(knight);
        let mut lines = vec![crate::rules::genie::UPGRADE_HEAD.to_string()];
        let mut keys = 1..=9;
        for up in &list {
            let key = if up.available { keys.next().map(|n| n.to_string()).unwrap_or_default() } else { String::new() };
            let tip = if up.next.is_some() { up.says.as_str() } else { "MAX" };
            let words = if up.available { up.words() } else { String::new() };
            lines.push(format!("{key}\t{}\t{}\t{tip}\t{words}", up.name, up.tiers()));
        }
        lines.extend(run.upgrade_note(knight, &list).map(str::to_string));
        lines.join("\n")
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
        // A painting the page has is drawn in its inks' place, and an HD
        // drawing in its sprite's, their pictures sent with the first frame
        // that names them.
        let (mut painted, mut drawn) = (Vec::new(), Vec::new());
        let calm = self.viewer.calm_screen;
        let mut frame = self.stage.make_with(
            &scene,
            |body| match (&self.hd_page, calm) {
                (_, true) => None,
                (Some(page), _) => self.hd.picture_from(page, body, &mut drawn),
                (None, _) => self.hd.picture(body, &mut drawn),
            },
            |p| if calm { None } else { self.paintings.picture(p, arena::paintings::PAGE_SCALE, &mut painted) },
        );
        frame.pictures.extend(painted);
        frame.pictures.extend(drawn);
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
        self.paintings.forget();
        self.hd.forget();
    }

    fn stage_props(&mut self, on: bool) {
        if self.stage.flat_shots == on {
            // A page that changed how it draws has none of the other way's pictures.
            self.stage.flat_shots = !on;
            self.stage.forget();
            self.stage_sizes.clear();
            self.paintings.forget();
            self.hd.forget();
        }
    }

    fn painting_wanted(&mut self) -> Option<String> {
        self.paintings.wanted().or_else(|| {
            self.hd_page.as_ref()?;
            self.hd.wanted().map(|file| format!("{HD_PREFIX}{file}"))
        })
    }

    fn put_painting(&mut self, sha: &str, width: u32, height: u32, rgba: &[u8]) -> bool {
        if let Some(file) = sha.strip_prefix(HD_PREFIX) {
            return self.hd_page.as_mut().is_some_and(|page| page.put_file(file, (width, height), rgba));
        }
        self.paintings.put(sha, (width, height), rgba)
    }

    fn painting_failed(&mut self, sha: &str) {
        if let Some(file) = sha.strip_prefix(HD_PREFIX) {
            if let Some(page) = &mut self.hd_page {
                page.file_failed(file);
            }
            return;
        }
        self.paintings.failed(sha);
    }

    fn hd_manifest(&mut self, json: &str, scale: u32, rung: u32) -> bool {
        match arena::hd_pack::Pack::from_manifest(json) {
            Ok(pack) => {
                // The page's `?ladder=`: every knight on one rung, to look.
                if let Some(rung) = crate::rules::resolution::Rung::of(rung) {
                    crate::rules::resolution::set_rule(Some(crate::rules::resolution::Rule::Fixed(rung)));
                }
                self.hd_page = Some(pack);
                // No finer than the page shows the game's pixels at: past
                // it a drawing is sampled down, and nearest, it shimmers.
                self.hd.max_scale = Some(scale.max(1));
                self.hd.forget();
                true
            }
            Err(_) => false,
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
    let _ = writeln!(out, "{}   health {}/{}", hero.name, hero.hp, hero.max_hp);
    let arm = hero.arm.as_deref().map_or_else(|| format!("{:?}", hero.weapon), name);
    let _ = writeln!(out, "Weapon  {arm}");
    if run.room().kind == crate::rules::RoomKind::Alehouse {
        let _ = writeln!(out, "FREE WEAPONS WALL: six basic arms. Hold E at a peg to equip / swap.");
    }
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
        let _ = writeln!(out, "\nHAND  (Z plays the first, P brings the next up, or drag a card onto another)");
        for id in &hero.hand {
            let _ = writeln!(out, "  {}  {}", name(id), rules(id));
        }
    }
    if hero.spells.iter().any(Option::is_some) {
        let _ = writeln!(out, "\nSPELLS");
        for (slot, spell) in hero.spells.iter().enumerate() {
            if let Some(id) = spell {
                let _ = writeln!(out, "  {}  {}  {}", slot + 1, run.book.get(id).map_or_else(|| name(id), |c| hero.spell_name(c)), rules(id));
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
    if hero.mirror.open {
        let _ = writeln!(out, "\nMIRROR  arrows/WASD move · E/Enter choose · Esc close");
        if knight == 2 {
            let _ = writeln!(out, "P2 couch: IJKL move · \\ choose · Delete close · arrows when only P2's mirror is open");
        }
        if hero.mirror.row == 9 {
            let _ = writeln!(out, "NAME  Press T, type a short name, then Enter.");
        }
    }
    if let Some(wish) = run.wish_hud(knight) {
        let _ = writeln!(out, "\nWISHES  {} held: {} (three at the start, two each floor, a super wish each boss)", wish.remaining(), wish.state());
    }
    // The identity start: what this knight has wished of themselves, and
    // what each wish bestowed, in the words of whatever granted it.
    if let Some(m) = hero.making(&run.book) {
        let shape = m.shape.map_or("shape", crate::rules::identity::Shape::name);
        let _ = writeln!(out, "\nYOUR MAKING  the {shape}{}", if m.made() { ", whole" } else { "" });
        for id in hero.deck.iter().filter(|c| c.starts_with("wish-")) {
            let Some(card) = run.book.get(id) else { continue };
            let what = match crate::rules::identity::piece_of(card) {
                Some(crate::rules::identity::Piece::Part(place)) => place,
                Some(crate::rules::identity::Piece::Weapon) => "weapon",
                Some(crate::rules::identity::Piece::Ultimate) => "ultimate",
                _ => continue,
            };
            let _ = writeln!(out, "  {what:<9} {}: {}", card.name, card.text);
        }
        if let Some(ask) = m.heading() {
            let _ = writeln!(out, "  The scroll asks: {ask}");
        }
    }
    let open = if run.room().kind == crate::rules::RoomKind::Alehouse && !run.alehouse.lamp_claimed.contains(&knight) {
        "THREE FREE WISHES: rub the home genie's lamp (E), once this run"
    } else if run.can_wish(knight) {
        "A WISH IS HELD: rub the genie's lamp (E) at home or the altar, or press T, and say it your way"
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
        out.push_str("  none yet: rub the genie's lamp (E) at the altar and wish for anything, in your words\n");
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
    // The subplot: the genie's shackle, and the party's gauge toward it.
    let _ = writeln!(
        out,
        "\nTHE GENIE  {}/{}  {}",
        run.genie.freedom.min(crate::rules::genie::FREEDOM),
        crate::rules::genie::FREEDOM,
        run.genie.hint()
    );
    let _ = writeln!(out, "\nWISHES  {open}");
    for wish in crate::rules::phrasebook::book().iter().filter(|w| !mine.contains(&w.id.as_str())) {
        let tiers = wish.tiers.len();
        let level = (0..tiers)
            .rev()
            .find(|&t| hero.deck.contains(&wish.card_id(t)))
            .map_or(0, |t| t + 1);
        // The next tier, counted as the grant counts it, so the sheet locks
        // what the grant refuses and nothing else.
        let lock = (level < tiers).then(|| run.locked(knight, wish, level)).flatten();
        let next = match &lock {
            Some(why) => format!("LOCKED: {why}"),
            None if level < tiers => format!("next: {}", wish.tiers[level].1),
            _ => "full".to_string(),
        };
        let _ = writeln!(out, "  {:<18} {level}/{tiers}  {next}", wish.name);
    }
    let _ = writeln!(out, "\nTEACHERS");
    for q in crate::rules::questline::book().lines.iter() {
        let power = q.power.clone();
        let note = run.quest_sheet(hero, &power);
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

#[cfg(test)]
#[path = "../tests/client_stream_fuzz__tests.rs"]
mod stream_fuzz_tests;


#[cfg(test)]
mod three_heroes_input_tests {
    use super::*;

    #[test]
    fn three_heroes_browser_preserves_hold_and_release_for_charged_draw() {
        let hands = Hands { held: key::SPELLS[0], ..Default::default() };
        for _ in 0..45 {
            assert_eq!(play(hands.held, hands.held, &hands).cast, 1);
        }
        assert_eq!(play(0, 0, &Hands::default()).cast, 0);
    }
}
