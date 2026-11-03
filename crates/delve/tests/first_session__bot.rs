//! The first session, measured: a beginner at a friend's seat from the
//! moment the link opens, played the way the live box starts a delve, and
//! what came of it (docs/FIRST-SESSION.md).
//!
//! The beginner (`Newbie`) plays slowly and imprecisely: it looks at the
//! screen a few times a second and holds its keys in between, aims an
//! eighth off as often as not, wanders, side-steps a shot only now and then,
//! and stands still to read, to look round a new place and to type a wish.
//! It knows only the keys to walk. Every other key it learns from the words
//! on its own screen (each line `Img::text` draws there, through
//! `ink::said`: a line in the room's view once it has been up long enough
//! to read, a line in the bands only at a glance now and then), or by
//! trying keys when nothing it knows works, and keeping the one that did
//! something. Every number it plays by is a knob (`Beginner`).
//!
//! `play` runs one session to its end or its time and keeps the log
//! (`Session`): when the knight first moved, fired, hit, killed, wished,
//! reached the first floor and left it; how often it fell, and where; how
//! long it stood idle, and why; every key learned and how; every line the
//! screen showed, when and for how long. `first_session_sim` prints it for
//! a set of seeds (env: FS_SEEDS, FS_SECS, FS_FILM).

use crate::ink::said;
use crate::render::overworld::arena;
use crate::rules::layout::{RoomKind, TILE_UNITS, Tile};
use crate::rules::{EnemyKind, HZ, Input, Phase, Run, TELEGRAPH};
use crate::ink::Img;
use std::collections::{BTreeMap, VecDeque};

/// How a beginner plays: every timing and every chance, in ticks and in
/// percent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beginner {
    /// Ticks between looks at the screen; the keys are held in between.
    pub think: u32,
    /// How far off an aim goes either way, in degrees.
    pub aim_slop: f32,
    /// Ticks a foe must have been awake and in sight before the beginner
    /// turns on it.
    pub notice: u32,
    /// Percent of looks spent walking some other way for a moment.
    pub wander: u32,
    /// Percent of looks a shot coming close is side-stepped.
    pub dodge: u32,
    /// Ticks a line in the room's view must be up before it is read.
    pub read: u32,
    /// Percent chance, each second a line in the bands is up, that the
    /// beginner glances at it and reads it.
    pub glance: u32,
    /// Ticks spent looking round a place on coming into it.
    pub look: u32,
    /// Ticks to type a wish once its line is open.
    pub typing: u32,
    /// Ticks spent reading the scroll's offers before choosing one.
    pub ponder: u32,
    /// Ticks between tries of a key nobody has named, while nothing known
    /// works.
    pub fumble: u32,
    /// Ticks after one wish before the next is made.
    pub between: u32,
    /// Ticks between looks at the screen's words (a frame drawn and read).
    pub every: u32,
}

impl Default for Beginner {
    fn default() -> Self {
        Beginner {
            think: 8,
            aim_slop: 35.0,
            notice: 15,
            wander: 12,
            dodge: 25,
            read: 45,
            glance: 3,
            look: 60,
            typing: 8 * HZ,
            ponder: 3 * HZ,
            fumble: 3 * HZ,
            between: 30 * HZ,
            every: 3,
        }
    }
}

/// What a key does, as a beginner thinks of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Walk,
    Fire,
    Use,
    Dash,
    Swing,
    Wish,
}

impl Key {
    pub fn name(self) -> &'static str {
        match self {
            Key::Walk => "walk (WASD)",
            Key::Fire => "fire (arrows / F)",
            Key::Use => "use (E)",
            Key::Dash => "roll (Space)",
            Key::Swing => "swing (Q)",
            Key::Wish => "wish (T)",
        }
    }
}

/// The keys a beginner tries, in turn, when nothing it knows does anything.
const FUMBLE: [Key; 5] = [Key::Dash, Key::Use, Key::Fire, Key::Swing, Key::Wish];

/// The keys a line on the screen names: a key's own token beside other
/// words ("T TO MAKE THEM", "E: BE THIS KNIGHT", "WISH 3 T", "F SHOOT").
pub fn teaches(line: &str) -> Vec<Key> {
    let up = line.to_uppercase();
    let words: Vec<&str> = up
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| matches!(c, ':' | ',' | '.' | '!' | '(' | ')' | '[' | ']')))
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < 2 {
        return Vec::new();
    }
    let mut keys = Vec::new();
    for w in &words {
        let key = match *w {
            "T" => Key::Wish,
            "E" | "P" => Key::Use,
            "F" | "ARROWS" | "ARROW" => Key::Fire,
            "Q" => Key::Swing,
            "SPACE" => Key::Dash,
            "WASD" => Key::Walk,
            _ => continue,
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// A key the HUD's tray names under its slot's picture (the weapon's F,
/// the roll's SPC): a cap alone, read at a glance as that slot's key.
pub fn keycap(line: &str) -> Option<Key> {
    match line.trim() {
        "F" => Some(Key::Fire),
        "SPC" => Some(Key::Dash),
        _ => None,
    }
}

/// Where on the screen a line was drawn: the room's view, the wish scroll
/// over it, or the bands above and below it (the party's cards, the HUD).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    View,
    Scroll,
    Band,
    /// A notice the host sent, shown by the page as a toast (`boot.js`
    /// `toast`, `TOAST_MS`).
    Toast,
}

/// How long the page shows a toast, in ticks (`TOAST_MS`, 2.5 s).
pub const TOAST_TICKS: u64 = 75;

/// Knight `id`'s screen as its seat draws it: from the run its client
/// has, the tick's tree read back through the seat's share of it
/// (`wire::publish`, `seat_tree`, `read`). That leaves out what a client
/// never hears of (the host's own empty place, the floor's tally, the
/// dice), so a picture that leans on one is caught here as in a browser.
pub fn seat_screen(run: &mut Run, id: u32, primer: &mut arena::primer::Primer) -> Screen {
    let seat = crate::wire::seat_of(id);
    let mirror = crate::wire::publish(run)
        .ok()
        .zip(seat)
        .and_then(|(tree, seat)| crate::wire::read(&crate::wire::seat_tree(&tree, Some(seat))).ok());
    match mirror {
        Some(mirror) => screen(&mirror, id, primer),
        None => screen(run, id, primer),
    }
}

/// A seat's whole screen for knight `id`, as its client draws it (the room's
/// view, the presence marks, the scroll, the bands): the picture, every line
/// on it with the layer it was written in, and the boards up over the view.
/// A line written on a picture bigger than the view (a room's whole canvas,
/// cropped to the view by the camera) counts only where the crop shows it.
pub fn screen(run: &Run, id: u32, primer: &mut arena::primer::Primer) -> Screen {
    let (w, h) = arena::seat_size();
    let (w, h) = (w as i32, h as i32);
    arena::in_seat(|| {
        let seat_view = run.view_for_seat(id);
        let run = seat_view.as_ref().unwrap_or(run);
        let focus = Some(id);
        let view_h = arena::view_rows(h);
        let (cx, cy) = arena::camera(run, focus);
        let shown = |l: &said::Line| {
            let big = l.on.0 > w || l.on.1 > view_h;
            let (x, y) = if big { (l.at.0 - cx, l.at.1 - cy) } else { l.at };
            let tw = crate::ink::text_width(&l.text);
            !big || (x + tw > 0 && x < w && y + 7 > 0 && y < view_h)
        };
        let mut lines = Vec::new();
        let (mut img, view) = said::read(|| {
            let mut img = arena::frame_for(run, w, view_h, focus);
            arena::presence_marks(&mut img, run, focus);
            crate::render::escape::paint(&mut img, run, focus);
            img
        });
        lines.extend(view.lines.iter().filter(|l| shown(l)).map(|l| (Layer::View, l.text.clone())));
        let ((), scroll) = said::read(|| crate::render::genie::paint(&mut img, run, focus));
        let scrolled = !scroll.lines.is_empty();
        lines.extend(scroll.lines.into_iter().map(|l| (Layer::Scroll, l.text)));
        // The first steps' hints, over the view as the client paints them.
        // Its keycap and its word are one plate, read together.
        let ((), hints) = said::read(|| primer.paint(&mut img, run, id));
        if !hints.lines.is_empty() {
            lines.push((Layer::View, hints.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" ")));
        }
        let (img, band) = said::read(|| arena::seat_lift_frame_with(&img, run, focus, &arena::MapView::default()));
        lines.extend(band.lines.into_iter().map(|l| (Layer::Band, l.text)));
        lines.sort();
        lines.dedup();
        let boards = view.boards.iter().chain(&hints.boards).chain(&band.boards).map(|&(w, h)| (w * h) as u32).sum();
        Screen { img, lines, boards, scroll: scrolled }
    })
}

/// What a seat's screen showed (`screen`).
pub struct Screen {
    pub img: Img,
    pub lines: Vec<(Layer, String)>,
    /// The boards up over the view, their area in pixels together.
    pub boards: u32,
    /// The wish scroll is open over the view.
    pub scroll: bool,
}

/// One line the screen showed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    pub layer: Option<Layer>,
    /// Session ticks it was first and last up.
    pub first: u64,
    pub last: u64,
    /// Ticks it was up, and whether the beginner read it (and when).
    pub up: u64,
    pub read: Option<u64>,
    /// Ticks it was up in the room's view while a foe was awake: words
    /// over a fight.
    pub over_fight: u64,
}

/// What the beginner was doing while it held no keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Idle {
    /// Looking round a place just come into.
    Looking,
    /// Reading a line, or the scroll.
    Reading,
    /// Typing a wish.
    Typing,
    /// Fallen.
    Down,
    /// Nothing to do it knows of.
    Lost,
}

/// A session's log.
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub seed: u64,
    pub ticks: u64,
    /// The first time each thing happened, in session ticks.
    pub firsts: BTreeMap<&'static str, u64>,
    /// Keys learned: when, and from what (a line read, or a key tried).
    pub learned: BTreeMap<Key, (u64, String)>,
    /// Every place come into: when, and what it was.
    pub places: Vec<(u64, String)>,
    /// The first floor's chambers with a company, in the order first stood
    /// in, this run (a run begun again begins it again).
    pub chambers: Vec<usize>,
    /// Falls: when, and the chamber (its place in `chambers`, from 1) or
    /// the place it happened in.
    pub falls: Vec<(u64, String)>,
    /// Runs lost and begun again at the tavern.
    pub wipes: u32,
    pub kills: u32,
    pub wishes: Vec<(u64, String)>,
    pub idle: BTreeMap<Idle, u64>,
    pub shown: BTreeMap<String, Shown>,
    /// Ticks the knight stood with keys held and did not move: walls,
    /// corners, the crowd.
    pub stuck: u64,
    /// Ticks in a fight (a foe awake near), and of those, with a board up
    /// over the view (and their area together, pixel-ticks), and with the
    /// wish scroll open.
    pub fight: u64,
    pub boards_over_fight: u64,
    pub board_area_over_fight: u64,
    pub scroll_over_fight: u64,
}

impl Session {
    fn first(&mut self, what: &'static str, at: u64) {
        self.firsts.entry(what).or_insert(at);
    }

    /// Seconds from the link to `what`, if it happened.
    pub fn secs(&self, what: &str) -> Option<f32> {
        self.firsts.get(what).map(|&t| t as f32 / HZ as f32)
    }

    /// Falls in the first `n` chambers fought.
    pub fn falls_in_first(&self, n: usize) -> usize {
        self.falls
            .iter()
            .filter(|(_, at)| at.strip_prefix("chamber ").and_then(|k| k.parse::<usize>().ok()).is_some_and(|k| k <= n))
            .count()
    }

    /// The key was learned by `secs` seconds in.
    pub fn knows_by(&self, key: Key, secs: f32) -> bool {
        self.learned.get(&key).is_some_and(|(t, _)| *t as f32 / HZ as f32 <= secs)
    }

    pub fn idle_secs(&self) -> f32 {
        self.idle.values().sum::<u64>() as f32 / HZ as f32
    }

    /// Seconds words lay over the room's view in a fight.
    /// Seconds a board stood over the view in a fight, and of how many
    /// seconds of fighting.
    pub fn boards_over_fights(&self) -> (f32, f32) {
        (self.boards_over_fight as f32 / HZ as f32, self.fight as f32 / HZ as f32)
    }
}

/// What the beginner is about.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Going,
    /// Standing still to look round, until the tick.
    Looking(u64),
    /// The wish line is open: typing, until the tick.
    Typing(u64),
    /// The scroll is open: reading its offers, until the tick.
    Pondering(u64),
}

/// A host-side act the beginner asks for: what the page sends, which the
/// host answers (the wish line's words, offered on the scroll).
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Wish(String),
}

/// The words a beginner types into the wish line, tried in turn until one
/// is a wish the phrasebook knows.
pub const WORDS: [&str; 5] = ["fire faster", "more health", "triple shot", "faster", "bigger sword"];

/// The beginner at knight `id`'s keys.
pub struct Newbie {
    pub id: u32,
    pub knobs: Beginner,
    rng: u64,
    mode: Mode,
    held: Input,
    next_think: u64,
    path: Vec<(i32, i32)>,
    /// The place the knight is in: depth, room, kind.
    place: Option<(u32, usize, RoomKind)>,
    /// Ticks each foe has been awake and in sight.
    seen_foes: BTreeMap<u32, u32>,
    /// The key being tried, and when it was pressed.
    trying: Option<(Key, u64)>,
    tried: usize,
    last_try: u64,
    /// Ticks since the knight last did anything that worked.
    since_progress: u64,
    /// Health when the wish line was opened: a blow while typing lets it go.
    typing_hp: u32,
    wish_words: usize,
    /// Where the walk keys are taking the knight between looks.
    steer: Option<Goal>,
    /// Since when the knight has stood at a goal to use or take, and the
    /// things it gave up taking.
    at_goal: Option<u64>,
    given_up: Vec<(f32, f32)>,
    last_at: (f32, f32),
    pub log: Session,
    /// Ticks since the link opened.
    pub t: u64,
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// An eighth of a turn for a direction: the arrow keys' aims.
fn dir8(dx: f32, dy: f32) -> (i8, i8) {
    let a = dy.atan2(dx);
    let k = ((a / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8);
    [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)][k as usize]
}

/// Nothing but floor between two points: a foe the beginner can see.
fn sight(run: &Run, from: (f32, f32), to: (f32, f32)) -> bool {
    let room = run.room();
    let d = (to.0 - from.0).hypot(to.1 - from.1);
    let n = (d / 0.5).ceil().max(1.0) as i32;
    (1..n).all(|k| {
        let t = k as f32 / n as f32;
        let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        !matches!(room.tile((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32), Tile::Wall | Tile::Block)
    })
}

fn walkable(t: Tile, barred: bool, stairs: bool) -> bool {
    match t {
        Tile::Floor | Tile::Ledge | Tile::Bog => true,
        Tile::Door => !barred,
        Tile::Stairs => stairs,
        _ => false,
    }
}

/// The tiles from `from` to `to` over the room's walkable floor, stairs only
/// where `stairs_ok` says (a stair walked onto by chance goes somewhere).
fn path(run: &Run, from: (f32, f32), to: (f32, f32), stairs_ok: &dyn Fn(i32, i32) -> bool) -> Option<Vec<(i32, i32)>> {
    let room = run.room();
    let cell = |(x, y): (f32, f32)| ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32);
    let (start, goal) = (cell(from), cell(to));
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let inside = |(c, r): (i32, i32)| (0..cols).contains(&c) && (0..rows).contains(&r);
    if !inside(start) || !inside(goal) {
        return None;
    }
    let barred = run.barred();
    let open = |(c, r): (i32, i32)| (c, r) == goal || walkable(room.tile(c, r), barred, stairs_ok(c, r));
    let at = |(c, r): (i32, i32)| (r * cols + c) as usize;
    let mut back = vec![u32::MAX; (cols * rows) as usize];
    back[at(start)] = at(start) as u32;
    let mut queue = VecDeque::from([start]);
    while let Some(here) = queue.pop_front() {
        if here == goal {
            let mut out = vec![goal];
            let mut k = at(goal);
            while k != at(start) {
                k = back[k] as usize;
                out.push(((k as i32) % cols, (k as i32) / cols));
            }
            out.reverse();
            return Some(out);
        }
        for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (here.0 + dc, here.1 + dr);
            if inside(next) && back[at(next)] == u32::MAX && open(next) {
                back[at(next)] = at(here) as u32;
                queue.push_back(next);
            }
        }
    }
    None
}

/// Where the beginner heads in the place it stands in, and what for.
#[derive(Clone, Copy, Debug)]
struct Goal {
    to: (f32, f32),
    /// The stair tiles it may walk onto: the goal's own.
    stairs: (i32, i32, i32, i32),
    /// Press use on getting there (a door on the kingdom).
    use_there: bool,
    /// Take what lies there (a weapon).
    take: bool,
}

fn rect_goal((c, r, w, h): (i32, i32, i32, i32)) -> Goal {
    Goal {
        to: ((c as f32 + w as f32 / 2.0) * TILE_UNITS, (r as f32 + h as f32 / 2.0) * TILE_UNITS),
        stairs: (c, r, w, h),
        use_there: false,
        take: false,
    }
}

fn entrance(id: &str) -> Option<Goal> {
    crate::rules::world::ENTRANCES.iter().find(|e| e.id == id).map(|e| rect_goal(e.at))
}

impl Newbie {
    pub fn new(id: u32, seed: u64, knobs: Beginner) -> Self {
        let mut log = Session { seed, ..Session::default() };
        log.learned.insert(Key::Walk, (0, "known: every game walks on WASD".into()));
        Newbie {
            id,
            knobs,
            rng: mix(seed ^ 0x6e65_7762_6965),
            mode: Mode::Going,
            held: Input::default(),
            next_think: 0,
            path: Vec::new(),
            place: None,
            seen_foes: BTreeMap::new(),
            trying: None,
            tried: 0,
            last_try: 0,
            since_progress: 0,
            typing_hp: 0,
            wish_words: 0,
            steer: None,
            at_goal: None,
            given_up: Vec::new(),
            last_at: (0.0, 0.0),
            log,
            t: 0,
        }
    }

    fn roll(&mut self, percent: u32) -> bool {
        self.rng = mix(self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15));
        (self.rng % 100) < u64::from(percent)
    }

    /// True `n` times in `d`.
    fn odds(&mut self, n: u64, d: u64) -> bool {
        self.rng = mix(self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15));
        self.rng % d.max(1) < n
    }

    fn unit(&mut self) -> f32 {
        self.rng = mix(self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15));
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn knows(&self, key: Key) -> bool {
        self.log.learned.contains_key(&key)
    }

    fn learn(&mut self, key: Key, how: String) {
        if !self.knows(key) {
            self.log.learned.insert(key, (self.t, how));
        }
    }

    fn idle(&mut self, why: Idle) {
        *self.log.idle.entry(why).or_default() += 1;
    }

    /// The words on the screen this look: each line's time up, and what the
    /// beginner read in them.
    pub fn saw(&mut self, run: &Run, seen: &Screen, ticks: u64) {
        let fight = self.in_fight(run);
        if fight {
            self.log.fight += ticks;
            if seen.boards > 0 {
                self.log.boards_over_fight += ticks;
                self.log.board_area_over_fight += u64::from(seen.boards) * ticks;
            }
            if seen.scroll {
                self.log.scroll_over_fight += ticks;
            }
        }
        for (layer, line) in &seen.lines {
            let t = self.t;
            let shown = self.log.shown.entry(line.clone()).or_insert_with(|| Shown { layer: Some(*layer), first: t, ..Shown::default() });
            shown.last = t;
            shown.up += ticks;
            if fight && *layer == Layer::View {
                shown.over_fight += ticks;
            }
            if shown.read.is_some() {
                continue;
            }
            let long_enough = shown.up >= u64::from(self.knobs.read);
            let read = match layer {
                Layer::View | Layer::Scroll | Layer::Toast => long_enough,
                Layer::Band => {
                    // A glance now and then: `glance` percent a second.
                    let odds = u64::from(self.knobs.glance) * ticks;
                    self.odds(odds, 100 * u64::from(HZ)) && long_enough
                }
            };
            if read {
                self.log.shown.get_mut(line).unwrap().read = Some(t);
                let keys = if *layer == Layer::Band { keycap(line).into_iter().chain(teaches(line)).collect() } else { teaches(line) };
                for key in keys {
                    self.learn(key, format!("read \"{line}\""));
                }
            }
        }
    }

    /// A foe awake near the knight: a fight.
    fn in_fight(&self, run: &Run) -> bool {
        let Some(me) = run.players.get(&self.id) else { return false };
        run.enemies.iter().any(|e| {
            e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH && (e.x - me.x).hypot(e.y - me.y) < 18.0
        })
    }

    fn foes_awake(&self, run: &Run) -> bool {
        run.enemies.iter().any(|e| e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH)
    }

    /// Where to go in the place the knight is in.
    fn goal(&self, run: &Run) -> Option<Goal> {
        let me = run.players.get(&self.id)?;
        match run.room().kind {
            RoomKind::Alehouse => entrance("alehouse-out"),
            RoomKind::Bazaar => entrance("bazaar-out"),
            RoomKind::Gate => entrance("gate"),
            RoomKind::Home => {
                let (c, r, w, h) = crate::rules::home::STAIRWELL;
                Some(rect_goal((c + 1, r + 1, w - 2, h - 2)))
            }
            RoomKind::Kingdom => {
                let (sx, sy) = crate::world::kingdom::gate_stand(&run.kingdom.wishes);
                Some(Goal {
                    to: ((sx as f32 + 0.5) * TILE_UNITS, (sy as f32 + 0.7) * TILE_UNITS),
                    stairs: (0, 0, 0, 0),
                    use_there: true,
                    take: false,
                })
            }
            _ if run.dungeon.depth >= 1 => {
                let o = run.dungeon.open.as_ref()?;
                if run.at != o.map {
                    return self.door_out(run);
                }
                // Something lying near that the knight lacks, labelled
                // E TAKE: the cache's weapon, a helm or mail a kill dropped.
                // A beginner goes for what shines.
                let room = &run.dungeon.rooms[run.at];
                let wanted = |card: &str| match card {
                    "iron-helm" => !me.owned.helmet,
                    "body-armour" => !me.owned.body,
                    "kite-shield" => !me.owned.shield,
                    _ => me.unarmed() && run.book.get(card).is_some_and(|c| c.kind == crate::rules::cards::Kind::Arm),
                };
                let given_up = |i: &crate::rules::Item| self.given_up.iter().any(|&(x, y)| (x - i.x).abs() < 0.5 && (y - i.y).abs() < 0.5);
                if let Some(item) = room
                    .items
                    .iter()
                    .filter(|i| i.seen_by(Some(self.id)) && wanted(&i.card) && !given_up(i))
                    .filter(|i| (i.x - me.x).hypot(i.y - me.y) < 14.0 && sight(run, (me.x, me.y), (i.x, i.y)))
                    .min_by(|a, b| (a.x - me.x).hypot(a.y - me.y).total_cmp(&(b.x - me.x).hypot(b.y - me.y)))
                {
                    return Some(Goal { to: (item.x, item.y), stairs: (0, 0, 0, 0), use_there: false, take: true });
                }
                let g = &o.chambers[usize::from(o.guardian)];
                if g.cleared {
                    let (x, y, w, h) = g.rect;
                    let (x, y, w, h) = (i32::from(x), i32::from(y), i32::from(w), i32::from(h));
                    let room = run.room();
                    let s = (y..y + h).flat_map(|r| (x..x + w).map(move |c| (c, r))).find(|&(c, r)| room.tile(c, r) == Tile::Stairs)?;
                    return Some(rect_goal((s.0, s.1, 1, 1)));
                }
                // The nearest chamber with a company still in it, the
                // guardian's last.
                let nearest = o
                    .chambers
                    .iter()
                    .enumerate()
                    .filter(|(i, c)| !c.cleared && *i != usize::from(o.guardian) && c.kind != RoomKind::Start && run.enemies.iter().any(|e| e.home == Some(*i as u8) && e.hp > 0))
                    .min_by(|a, b| {
                        let d = |c: &crate::rules::layout::open::Chamber| {
                            let (cx, cy) = c.centre();
                            (cx - me.x).hypot(cy - me.y)
                        };
                        d(a.1).total_cmp(&d(b.1))
                    })
                    .map(|(_, c)| c);
                let target = nearest.unwrap_or(g);
                let index = o.chambers.iter().position(|c| std::ptr::eq(c, target)).unwrap_or(usize::from(o.guardian));
                // Its middle; once there, the last of its company wherever
                // they stand, as a player hunts the last of a room.
                let (cx, cy) = target.centre();
                let spot = run.open_spots(o.map, (cx, cy), 1).first().copied().unwrap_or((cx, cy));
                let inside = target.holds(me.x, me.y, 0.0);
                let last = run
                    .enemies
                    .iter()
                    .filter(|e| e.home == Some(index as u8) && e.hp > 0)
                    .min_by(|a, b| (a.x - me.x).hypot(a.y - me.y).total_cmp(&(b.x - me.x).hypot(b.y - me.y)))
                    .map(|e| (e.x, e.y));
                let to = if inside { last.unwrap_or(spot) } else { spot };
                Some(Goal { to, stairs: (0, 0, 0, 0), use_there: false, take: false })
            }
            _ => self.door_out(run),
        }
    }

    /// The nearest way out of a side place: an entrance's stair, or a doorway
    /// on the room's edge.
    fn door_out(&self, run: &Run) -> Option<Goal> {
        let me = run.players.get(&self.id)?;
        let kind = run.room().kind;
        let near = |(x, y): (f32, f32)| (x - me.x).hypot(y - me.y);
        let mut best: Option<(f32, Goal)> = None;
        for e in crate::rules::world::ENTRANCES.iter().filter(|e| e.room == kind) {
            let g = rect_goal(e.at);
            if best.is_none_or(|(d, _)| near(g.to) < d) {
                best = Some((near(g.to), g));
            }
        }
        let room = run.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        for r in 0..rows {
            for c in 0..cols {
                let edge = c == 0 || r == 0 || c == cols - 1 || r == rows - 1;
                if edge && matches!(room.tile(c, r), Tile::Door) {
                    let g = rect_goal((c, r, 1, 1));
                    if best.is_none_or(|(d, _)| near(g.to) < d) {
                        best = Some((near(g.to), g));
                    }
                }
            }
        }
        best.map(|(_, g)| g)
    }

    /// The chamber of the first floor the knight stands in, if any.
    fn chamber(&self, run: &Run) -> Option<usize> {
        let o = run.dungeon.open.as_ref().filter(|o| run.at == o.map && run.dungeon.depth == 1)?;
        let me = run.players.get(&self.id)?;
        o.chambers.iter().position(|c| c.holds(me.x, me.y, 0.0))
    }

    /// The keys for this tick, and anything asked of the host.
    pub fn input(&mut self, run: &Run) -> (Input, Option<Ask>) {
        let Some(me) = run.players.get(&self.id) else {
            return (Input::default(), None);
        };
        let me = me.clone();
        // A new place: look round it first.
        let place = (run.dungeon.depth, run.at, run.room().kind);
        if self.place != Some(place) {
            self.log.places.push((self.t, format!("{:?} (floor {}, room {})", place.2, place.0, place.1)));
            if place.0 == 1 {
                self.log.first("floor 1", self.t);
            }
            if place.0 >= 2 {
                self.log.first("floor 1 left", self.t);
            }
            self.place = Some(place);
            self.path.clear();
            if matches!(self.mode, Mode::Going) {
                self.mode = Mode::Looking(self.t + u64::from(self.knobs.look));
            }
        }
        if let Some(c) = self.chamber(run)
            && !self.log.chambers.contains(&c)
            && run.enemies.iter().any(|e| e.home == Some(c as u8))
        {
            self.log.chambers.push(c);
        }
        if me.hp == 0 {
            self.idle(Idle::Down);
            self.mode = Mode::Going;
            return (Input::default(), None);
        }
        match self.mode {
            Mode::Looking(until) => {
                if self.t < until && !self.foes_awake(run) {
                    self.idle(Idle::Looking);
                    return (Input::default(), None);
                }
                self.mode = Mode::Going;
            }
            Mode::Typing(until) => {
                // The wish line holds the keys; a blow makes the beginner
                // let it go and fight.
                if me.hp < self.typing_hp {
                    self.mode = Mode::Going;
                } else if self.t < until {
                    self.idle(Idle::Typing);
                    return (Input::default(), None);
                } else {
                    // Enter: the words go to the host, which unrolls the scroll.
                    let words = WORDS[self.wish_words % WORDS.len()].to_string();
                    self.wish_words += 1;
                    self.mode = Mode::Pondering(self.t + u64::from(self.knobs.ponder));
                    self.idle(Idle::Typing);
                    return (Input::default(), Some(Ask::Wish(words)));
                }
            }
            Mode::Pondering(until) => {
                if me.offer.is_none() {
                    // The scroll did not open (the words were not a wish it
                    // knew), or it was taken: back to the game.
                    self.mode = Mode::Going;
                } else if self.t < until {
                    self.idle(Idle::Reading);
                    return (Input::default(), None);
                } else {
                    // "1 2 3 CHOOSE": the first.
                    self.mode = Mode::Going;
                    return (Input { cast: 1, ..Input::default() }, None);
                }
            }
            Mode::Going => {}
        }
        if self.t >= self.next_think {
            self.next_think = self.t + u64::from(self.knobs.think);
            self.steer = None;
            let input = self.think(run, &me);
            if self.mode != Mode::Going {
                return (Input::default(), None);
            }
            self.held = input;
        }
        let mut input = self.held.clone();
        // On the way somewhere: the walk keys follow the path tick by tick,
        // as a hand lets go of a key at a corner.
        if let Some(goal) = self.steer {
            (input.move_x, input.move_y) = self.walk(run, &me, goal);
        }
        if input == Input::default() {
            self.idle(Idle::Lost);
        }
        (input, None)
    }

    /// The walk keys toward `goal` along the room's floor.
    fn walk(&mut self, run: &Run, me: &crate::rules::Hero, goal: Goal) -> (i8, i8) {
        let here = ((me.x / TILE_UNITS).floor() as i32, (me.y / TILE_UNITS).floor() as i32);
        let centre = |(c, r): (i32, i32)| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS);
        // A tile reached (its middle near enough) is passed.
        while self.path.len() > 1 && {
            let (x, y) = centre(self.path[0]);
            self.path[0] == here && (x - me.x).abs() < 0.45 && (y - me.y).abs() < 0.45 || self.path[1] == here
        } {
            self.path.remove(0);
        }
        // The way again when it is lost (off the path), or now and then.
        let on = self.path.first().is_some_and(|&(c, r)| (c - here.0).abs() + (r - here.1).abs() <= 1);
        if !on || self.t.is_multiple_of(u64::from(2 * HZ)) {
            let (sc, sr, sw, sh) = goal.stairs;
            self.path = path(run, (me.x, me.y), goal.to, &|c, r| c >= sc && c < sc + sw && r >= sr && r < sr + sh).unwrap_or_default();
            while self.path.len() > 1 && self.path[0] == here && self.path.get(1).is_some() {
                let (x, y) = centre(self.path[0]);
                if (x - me.x).abs() < 0.45 && (y - me.y).abs() < 0.45 {
                    self.path.remove(0);
                } else {
                    break;
                }
            }
        }
        let (wx, wy) = match self.path.as_slice() {
            [] | [_] => goal.to,
            [first, ..] => centre(*first),
        };
        let axis = |d: f32| if d > 0.3 { 1 } else if d < -0.3 { -1 } else { 0 };
        (axis(wx - me.x), axis(wy - me.y))
    }

    fn think(&mut self, run: &Run, me: &crate::rules::Hero) -> Input {
        let mut input = Input::default();
        // Foes in sight, and how long each has been noticed.
        let awake: Vec<(u32, f32, f32)> = run
            .enemies
            .iter()
            .filter(|e| e.hp > 0 && !e.dormant && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH)
            .filter(|e| (e.x - me.x).hypot(e.y - me.y) < 18.0)
            .map(|e| (e.id, e.x, e.y))
            .collect();
        let think = self.knobs.think;
        self.seen_foes.retain(|id, _| awake.iter().any(|a| a.0 == *id));
        for a in &awake {
            *self.seen_foes.entry(a.0).or_default() += think;
        }
        let noticed = |a: &&(u32, f32, f32)| self.seen_foes.get(&a.0).is_some_and(|&t| t >= self.knobs.notice);
        let nearest = |a: &&(u32, f32, f32), b: &&(u32, f32, f32)| (a.1 - me.x).hypot(a.2 - me.y).total_cmp(&(b.1 - me.x).hypot(b.2 - me.y));
        // A foe in sight is fought; one heard behind a wall is gone after.
        let target = awake.iter().filter(noticed).filter(|a| sight(run, (me.x, me.y), (a.1, a.2))).min_by(nearest).copied();
        let behind = awake.iter().filter(noticed).min_by(nearest).copied();
        // A known wish and a calm moment: the line opens (T). One wish at a
        // time: the next waits a while (`between`).
        let calm = target.is_none() && awake.is_empty();
        let rested = self.log.wishes.last().is_none_or(|(at, _)| self.t >= at + u64::from(self.knobs.between));
        if self.knows(Key::Wish) && run.can_wish(self.id) && me.offer.is_none() && rested && calm && !self.in_fight(run) {
            self.mode = Mode::Typing(self.t + u64::from(self.knobs.typing));
            self.typing_hp = me.hp;
            return input;
        }
        // A key being tried: what came of it is judged after the step.
        if let Some((key, _)) = self.trying {
            self.press(&mut input, key, target.map(|t| (t.1, t.2)), me);
            return input;
        }
        if let Some((_, tx, ty)) = target {
            let (dx, dy) = (tx - me.x, ty - me.y);
            let d = dx.hypot(dy).max(0.01);
            let ranged = !me.unarmed();
            let want = if ranged { 6.0 } else { 1.2 };
            // In: along the floor toward it; close: a beginner's shuffle about.
            if d > want + 1.0 {
                self.steer = Some(Goal { to: (tx, ty), stairs: (0, 0, 0, 0), use_there: false, take: false });
            } else if self.roll(40) {
                let side = if self.roll(50) { 1.0 } else { -1.0 };
                (input.move_x, input.move_y) = dir8(-dy * side, dx * side);
            }
            if self.knows(Key::Fire) {
                // An aim off by up to `aim_slop` either way.
                let slop = (self.unit() * 2.0 - 1.0) * self.knobs.aim_slop.to_radians();
                let a = dy.atan2(dx) + slop;
                (input.aim_x, input.aim_y) = dir8(a.cos(), a.sin());
                input.fire = self.roll(85);
            } else if self.knows(Key::Swing) && d < 2.6 {
                input.swing = true;
            } else {
                self.fumble(&mut input, Some((tx, ty)), me);
            }
            if self.knows(Key::Swing) && d < 2.4 && self.roll(30) {
                input.swing = true;
            }
            // A shot coming close: sometimes side-stepped.
            if let Some(p) = run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0).find(|p| (p.x - me.x).hypot(p.y - me.y) < 3.0)
                && self.roll(self.knobs.dodge)
            {
                self.steer = None;
                (input.move_x, input.move_y) = dir8(-p.vy, p.vx);
                input.dash = self.knows(Key::Dash) && self.roll(30);
            }
            return input;
        }
        // A fight heard but not seen: round the wall toward it.
        if let Some((_, tx, ty)) = behind {
            self.steer = Some(Goal { to: (tx, ty), stairs: (0, 0, 0, 0), use_there: false, take: false });
            return input;
        }
        let Some(goal) = self.goal(run) else {
            if self.since_progress > u64::from(self.knobs.fumble) {
                self.fumble(&mut input, None, me);
            }
            return input;
        };
        let near = (goal.to.0 - me.x).hypot(goal.to.1 - me.y);
        if near < 0.9 && (goal.use_there || goal.take) {
            // A thing that will not come after a few tries is let be.
            let at = *self.at_goal.get_or_insert(self.t);
            if goal.take && self.t - at > u64::from(4 * HZ) {
                self.given_up.push(goal.to);
                self.at_goal = None;
                return input;
            }
            if self.knows(Key::Use) {
                input.interact = self.roll(60);
            } else if self.since_progress > u64::from(self.knobs.fumble) {
                self.fumble(&mut input, None, me);
            }
            return input;
        }
        self.at_goal = None;
        // A wander now and then: a beginner looks about as it goes.
        if self.roll(self.knobs.wander) {
            let a = self.unit() * std::f32::consts::TAU;
            (input.move_x, input.move_y) = dir8(a.cos(), a.sin());
            return input;
        }
        self.steer = Some(goal);
        input
    }

    /// Try the next key nobody has named, when its turn comes.
    fn fumble(&mut self, input: &mut Input, at: Option<(f32, f32)>, me: &crate::rules::Hero) {
        if self.t.saturating_sub(self.last_try) < u64::from(self.knobs.fumble) {
            return;
        }
        let untried: Vec<Key> = FUMBLE.iter().copied().filter(|k| !self.knows(*k)).collect();
        if untried.is_empty() {
            return;
        }
        let key = untried[self.tried % untried.len()];
        self.tried += 1;
        self.last_try = self.t;
        self.trying = Some((key, self.t));
        self.press(input, key, at, me);
    }

    fn press(&mut self, input: &mut Input, key: Key, at: Option<(f32, f32)>, me: &crate::rules::Hero) {
        match key {
            Key::Fire => {
                let (dx, dy) = at.map_or((0.0, -1.0), |(x, y)| (x - me.x, y - me.y));
                (input.aim_x, input.aim_y) = dir8(dx, dy);
                input.fire = true;
            }
            Key::Use => input.interact = true,
            Key::Dash => input.dash = true,
            Key::Swing => input.swing = true,
            Key::Wish => {
                // T opens the wish line: seen at once.
                self.learn(Key::Wish, "tried T: the wish line opened".into());
                self.trying = None;
            }
            Key::Walk => {}
        }
    }

    /// What `after` compares against: the knight and its place before the
    /// step (the whole run is too dear to copy every tick).
    pub fn before(&self, run: &Run) -> Before {
        Before {
            hero: run.players.get(&self.id).cloned(),
            depth: run.dungeon.depth,
            at: run.at,
            kind: run.room().kind,
            line: run.floor_tally.knights.get(&self.id).copied().unwrap_or_default(),
            chamber: self.chamber(run),
        }
    }

    /// After the step: what the keys did.
    pub fn after(&mut self, before: &Before, run: &Run, input: &Input) {
        let (Some(was), Some(now)) = (before.hero.as_ref(), run.players.get(&self.id)) else {
            self.t += 1;
            return;
        };
        let moved = (now.x - was.x).hypot(now.y - was.y) > 0.01;
        if moved {
            self.log.first("moved", self.t);
        }
        let pressed_move = input.move_x != 0 || input.move_y != 0;
        if pressed_move && !moved && now.hp > 0 {
            self.log.stuck += 1;
        }
        let fired = now.fire_cooldown > was.fire_cooldown;
        let rolled = now.dash_cooldown > was.dash_cooldown;
        let swung = now.swing > was.swing;
        let went = (before.depth, before.at) != (run.dungeon.depth, run.at);
        let armed = was.unarmed() && !now.unarmed();
        if fired {
            self.log.first("fired", self.t);
        }
        if armed {
            self.log.first("armed", self.t);
        }
        if (now.owned.helmet && !was.owned.helmet) || (now.owned.body && !was.owned.body) {
            self.log.first("helm or mail", self.t);
        }
        let before_line = before.line;
        let line = run.floor_tally.knights.get(&self.id).copied().unwrap_or_default();
        if run.dungeon.depth == before.depth {
            if line.damage > before_line.damage {
                self.log.first("first hit", self.t);
            }
            if line.kills > before_line.kills {
                self.log.kills += line.kills - before_line.kills;
                self.log.first("first kill", self.t);
            }
        }
        if was.hp > 0 && now.hp == 0 {
            let at = match before.chamber.and_then(|c| self.log.chambers.iter().position(|&k| k == c)) {
                Some(k) => format!("chamber {}", k + 1),
                None => format!("{:?} floor {}", before.kind, before.depth),
            };
            self.log.falls.push((self.t, at));
            self.log.first("first fall", self.t);
        }
        let spent = (was.wishes + was.great_wishes).saturating_sub(now.wishes + now.great_wishes);
        if spent > 0 || now.deck.len() > was.deck.len() && was.offer.is_some() && now.offer.is_none() {
            let name = now.deck.iter().find(|c| !was.deck.contains(c)).cloned().unwrap_or_else(|| "a wish".into());
            self.log.wishes.push((self.t, name));
            self.log.first("first wish", self.t);
        }
        // What a tried key did.
        if let Some((key, at)) = self.trying {
            let worked = match key {
                Key::Fire => fired,
                Key::Dash => rolled,
                Key::Swing => swung,
                Key::Use => went || armed,
                _ => false,
            };
            if worked {
                self.learn(key, format!("tried {}: it worked", key.name()));
                self.trying = None;
            } else if self.t - at > u64::from(self.knobs.think) * 2 {
                self.trying = None;
            }
        }
        if moved || fired || went || armed || spent > 0 {
            self.since_progress = 0;
        } else {
            self.since_progress += 1;
        }
        self.last_at = (now.x, now.y);
        self.t += 1;
    }
}

/// The knight and its place before a step.
pub struct Before {
    hero: Option<crate::rules::Hero>,
    depth: u32,
    at: usize,
    kind: RoomKind,
    line: crate::rules::floor_stats::Line,
    chamber: Option<usize>,
}

/// A new delve as the live box begins one (`ANGEL_DELVE_OPEN=1`,
/// `ANGEL_DELVE_HOST_KNIGHT=0`, the cockpit's `start_shooter`): the
/// Undercroft's run, a naked start, the host's own knight's place empty,
/// the party in the Sozzled Griffin; and the friend from the link seated as
/// knight 2 in the company's colours.
pub fn live_start(seed: u64, raid: u64) -> Run {
    let _open = crate::settings::scoped(crate::settings::Settings { open_floors: true, ..Default::default() });
    let mut run = Run::at_home(seed, raid, None, Default::default(), Default::default());
    run.open_floors = true;
    run.naked_start = true;
    run.identity = false;
    run.rename_host("Host");
    run.walk_alehouse();
    run.join_seat(2, "Newbie");
    let company = crate::rules::knights::COMPANY;
    run.outfit(2, &company[1 % company.len()]);
    run.set_absent(1, true);
    run
}

/// A session filmed: its seat's screen every `every` ticks, appended as raw
/// RGB to `<dir>/film.rgb` (`<dir>/film.txt` says the size and each frame's
/// tick); GIFs are cut from it afterwards (ffmpeg's rawvideo).
pub struct Film {
    pub dir: std::path::PathBuf,
    pub every: u64,
    pub out: std::cell::RefCell<Option<(std::io::BufWriter<std::fs::File>, std::fs::File)>>,
}

impl Film {
    pub fn new(dir: std::path::PathBuf, every: u64) -> Film {
        Film { dir, every, out: std::cell::RefCell::new(None) }
    }

    fn keep(&self, t: u64, img: &Img) {
        use std::io::Write;
        let mut out = self.out.borrow_mut();
        let (pixels, index) = out.get_or_insert_with(|| {
            let pixels = std::fs::File::create(self.dir.join("film.rgb")).expect("film.rgb");
            let mut index = std::fs::File::create(self.dir.join("film.txt")).expect("film.txt");
            let _ = writeln!(index, "{}x{} every {} at {} a second", img.w, img.h, self.every, HZ);
            (std::io::BufWriter::new(pixels), index)
        });
        let rgba = img.rgba_bytes();
        let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let _ = pixels.write_all(&rgb);
        let _ = writeln!(index, "{t}");
    }
}

/// The live box's start with the beginner as knight 1 instead (a dev host's
/// first seat: `delve-dev-host --scene live --newbie`): the same tavern, the
/// same naked start and open floors, and no empty place beside it.
pub fn live_start_first(seed: u64, raid: u64) -> Run {
    let _open = crate::settings::scoped(crate::settings::Settings { open_floors: true, ..Default::default() });
    let mut run = Run::at_home(seed, raid, None, Default::default(), Default::default());
    run.open_floors = true;
    run.naked_start = true;
    run.identity = false;
    run.rename_host("Newbie");
    let company = crate::rules::knights::COMPANY;
    run.outfit(1, &company[1 % company.len()]);
    run.walk_alehouse();
    run
}

/// The beginner at its seat, a tick at a time: it looks at its screen,
/// presses its keys, the host answers what it asked, the run steps, and it
/// sees what came of it. A run lost begins again at the tavern (as
/// `rules::again` takes the party back), with all the beginner learned.
pub struct Driver {
    pub bot: Newbie,
    pub seed: u64,
    pub raid: u64,
    /// How a lost run begins again.
    pub start: fn(u64, u64) -> Run,
    lost_at: Option<u64>,
    toasts: Vec<(u64, String)>,
    trace: bool,
    /// The seat's first steps' hints (`arena::primer`), as its client keeps
    /// them.
    pub primer: arena::primer::Primer,
    /// A run begun again takes the environment's knobs (`knobs_from_env`).
    pub tuned: bool,
}

impl Driver {
    pub fn new(seed: u64, knobs: Beginner, knight: u32, start: fn(u64, u64) -> Run) -> Driver {
        Driver {
            bot: Newbie::new(knight, seed, knobs),
            seed,
            raid: 1,
            start,
            lost_at: None,
            toasts: Vec::new(),
            trace: std::env::var_os("FS_TRACE").is_some(),
            primer: arena::primer::Primer::default(),
            tuned: false,
        }
    }

    /// The host's notices this tick: those for this seat are the page's
    /// toasts.
    pub fn heard(&mut self, run: &Run, notices: &[crate::wire::Event]) {
        let t = self.bot.t;
        for event in notices {
            if let Some(line) = event.line(Some(self.bot.id), Some(run)).and_then(|l| l.strip_prefix("notice ").map(str::to_string)) {
                self.toasts.push((t, line));
            }
        }
        self.toasts.retain(|(at, _)| t < at + TOAST_TICKS);
    }

    /// One tick of `run`, the beginner's keys beside `others` (the other
    /// seats'), stepped here. `film` keeps a frame when one is due.
    pub fn tick(&mut self, run: &mut Run, others: &BTreeMap<u32, Input>, film: Option<&Film>) {
        let t = self.bot.t;
        if run.phase == Phase::Wiped {
            let at = *self.lost_at.get_or_insert(t);
            if t - at > u64::from(5 * HZ) {
                self.bot.log.wipes += 1;
                self.raid += 1;
                *run = (self.start)(self.seed, self.raid);
                if self.tuned {
                    knobs_from_env(run, &mut self.primer);
                }
                self.bot.place = None;
                self.bot.log.chambers.clear();
                self.bot.given_up.clear();
                self.lost_at = None;
            }
        }
        let every = u64::from(self.bot.knobs.every);
        if t.is_multiple_of(every) || film.is_some_and(|f| t.is_multiple_of(f.every)) {
            let mut seen = seat_screen(run, self.bot.id, &mut self.primer);
            seen.lines.extend(self.toasts.iter().map(|(_, l)| (Layer::Toast, l.clone())));
            if t.is_multiple_of(every) {
                self.bot.saw(run, &seen, every);
            }
            if let Some(f) = film.filter(|f| t.is_multiple_of(f.every)) {
                f.keep(t, &seen.img);
            }
        }
        let bot = &mut self.bot;
        let (input, ask) = bot.input(run);
        if self.trace && t.is_multiple_of(u64::from(HZ) / 3) {
            let me = run.players.get(&bot.id);
            eprintln!(
                "t{:>4}s {:?}#{} at {:?} goal {:?} path {} {:?} mode {:?} in ({},{}) f{} e{} hp {:?} awake {} hint {:?} wishes {:?} into {}",
                t / u64::from(HZ),
                run.room().kind,
                run.at,
                me.map(|h| (h.x as i32, h.y as i32)),
                bot.goal(run).map(|g| (g.to.0 as i32, g.to.1 as i32)),
                bot.path.len(),
                &bot.path[..bot.path.len().min(3)],
                bot.mode,
                input.move_x,
                input.move_y,
                u8::from(input.fire),
                u8::from(input.interact),
                me.map(|h| h.hp),
                run.enemies.iter().filter(|e| e.hp > 0 && !e.dormant).count(),
                self.primer.showing(),
                me.map(|h| (h.wishes, h.great_wishes, run.can_wish(bot.id))),
                run.tick.saturating_sub(run.floor_tally.began),
            );
        }
        if let Some(Ask::Wish(words)) = ask {
            let deck = run.players.get(&bot.id).map(|h| h.deck.clone()).unwrap_or_default();
            if let Some(wish) = crate::rules::phrasebook::find_for(&words, &deck) {
                let _ = run.offer_wish(bot.id, &wish.name, Some(&wish));
            }
        }
        let before = bot.before(run);
        let before_window = run.window.is_some();
        let before_shots: Vec<(f32, f32, String, u32)> = if self.trace {
            run.projectiles.iter().filter(|p| p.hostile).map(|p| (p.x, p.y, format!("{:?}", p.kind), p.damage)).collect()
        } else {
            Vec::new()
        };
        let mut inputs = others.clone();
        inputs.insert(bot.id, input.clone());
        let _open = crate::settings::scoped(crate::settings::Settings { open_floors: true, ..Default::default() });
        run.step(&inputs);
        drop(_open);
        let window_was = before_window;
        bot.after(&before, run, &input);
        if self.trace && window_was != run.window.is_some() {
            eprintln!("window t{:.2}s {} -> {} room {:?}#{} phase {:?} cues {:?}", t as f32 / HZ as f32, window_was, run.window.is_some(), run.room().kind, run.at, run.phase, run.cues);
        }
        if self.trace
            && let (Some(was), Some(now)) = (before.hero.as_ref(), run.players.get(&bot.id))
            && now.hp < was.hp
        {
            let near: Vec<String> = run
                .enemies
                .iter()
                .filter(|e| e.hp > 0 && (e.x - now.x).hypot(e.y - now.y) < 5.0)
                .map(|e| format!("{:?}@{:.1}", e.kind, (e.x - now.x).hypot(e.y - now.y)))
                .collect();
            let shots: Vec<String> = before_shots
                .iter()
                .filter(|(x, y, _, _)| (x - now.x).hypot(y - now.y) < 3.0)
                .map(|(_, _, kind, dmg)| format!("{kind}:{dmg}"))
                .collect();
            eprintln!("hit t{:.1}s -{} ({}->{}) armor {} near {near:?} shots {shots:?}", t as f32 / HZ as f32, was.hp - now.hp, was.hp, now.hp, now.armor);
        }
        if run.dungeon.depth >= 2 {
            bot.log.first("floor 1 left", bot.t);
        }
    }
}

/// The game's knobs for a measurement, from the environment: `FS_TUNE`
/// (`name=value,...`, the server's knobs, `Tune::set`) and `FS_PRIMER=0`
/// (the first steps' hints off), so one change can be measured alone.
pub fn knobs_from_env(run: &mut Run, primer: &mut arena::primer::Primer) {
    if let Ok(tune) = std::env::var("FS_TUNE") {
        for pair in tune.split(',').filter(|p| !p.is_empty()) {
            if let Some((name, value)) = pair.split_once('=')
                && let Ok(value) = value.trim().parse::<f64>()
            {
                assert!(run.tune.set(name.trim(), value), "FS_TUNE: no knob {name}");
            }
        }
    }
    if std::env::var("FS_PRIMER").is_ok_and(|v| v == "0") {
        primer.knobs.on = false;
    }
}

/// One first session: the beginner at knight 2 of `live_start(seed)`, for
/// `secs` seconds or until it has left the first floor.
pub fn play(seed: u64, knobs: Beginner, secs: u32, film: Option<&Film>) -> Session {
    play_tuned(seed, knobs, secs, film, false)
}

/// `play`, with the game's knobs from the environment when `tuned`
/// (`knobs_from_env`: the sim's FS_TUNE and FS_PRIMER).
pub fn play_tuned(seed: u64, knobs: Beginner, secs: u32, film: Option<&Film>, tuned: bool) -> Session {
    let mut run = live_start(seed, 1);
    let mut driver = Driver::new(seed, knobs, 2, live_start);
    driver.tuned = tuned;
    if tuned {
        knobs_from_env(&mut run, &mut driver.primer);
    }
    let end = u64::from(secs) * u64::from(HZ);
    let none = BTreeMap::new();
    while driver.bot.t < end && run.dungeon.depth < 2 {
        driver.tick(&mut run, &none, film);
        let notices = crate::wire::take_notices(&mut run);
        driver.heard(&run, &notices);
        let _ = crate::wire::take_sounds(&mut run);
    }
    driver.bot.log.ticks = driver.bot.t;
    driver.bot.log
}

#[cfg(test)]
#[path = "first_session__tests.rs"]
mod tests;
