//! The joust, at the lists: a knight in the saddle against one of the
//! realm's champions, three courses down the tilt.
//!
//! A course: the riders wait at their ends; F spurs; the horses run at
//! each other down either side of the tilt, faster all the way; they meet
//! in the middle, lances on shields (or helms). The rider aims (W high at
//! the helm, S low at the shield), strikes (F, as the lances meet) and
//! braces (Space, as his lance comes in). The rival's shield shows where he
//! guards a moment before the meeting: strike where it isn't. A knight
//! knocked off balance enough times goes over his horse's tail onto the
//! sand, and the bout is over; otherwise three courses are counted.
//!
//! The horse matters: each rides differently (`Mount::steed`), and a tended
//! one carries its rider a knock longer. Everything here is a pure function
//! of the inputs and the seed, so a mirror and a test see the same bout.

use super::*;
use crate::drive::chivalry::{Mount, Stable, TENDED_BALANCE};

/// A champion of the realm at the lists.
pub(crate) struct Rival {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    /// His colours, field and charge (timber and stone inks: never state).
    pub(crate) colours: (char, char),
    /// How many knocks put him on the sand.
    pub(crate) balance: u32,
    /// What his lance does, course after course (a bout starts further
    /// along it each time it is ridden).
    pub(crate) lance: [Hit; 5],
}

pub(crate) const RIVALS: [Rival; 3] = [
    Rival {
        id: "kay",
        name: "Sir Kay",
        colours: ('u', 'h'),
        balance: 4,
        lance: [Hit::Shield, Hit::Miss, Hit::Clean, Hit::Shield, Hit::Miss],
    },
    Rival {
        id: "palamedes",
        name: "Sir Palamedes",
        colours: ('K', 'i'),
        balance: 5,
        lance: [Hit::Clean, Hit::Shield, Hit::Clean, Hit::Broke, Hit::Shield],
    },
    Rival {
        id: "lancelot",
        name: "Sir Lancelot",
        colours: ('W', 'R'),
        balance: 6,
        lance: [Hit::Clean, Hit::Smite, Hit::Clean, Hit::Broke, Hit::Smite],
    },
];

/// The rival a bout is ridden against: the first the realm has not beaten,
/// and Lancelot once all have been.
pub(crate) fn next_rival(stable: &Stable) -> usize {
    RIVALS
        .iter()
        .position(|r| stable.wins.get(r.id).copied().unwrap_or(0) == 0)
        .unwrap_or(RIVALS.len() - 1)
}

/// What a lance did as the riders met.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Hit {
    #[default]
    Miss,
    /// On the shield: a point, a knock.
    Shield,
    /// On the shield, and the lance broke on it: two points, two knocks.
    Broke,
    /// Where the shield wasn't: two points, two knocks.
    Clean,
    /// Where the shield wasn't, at the very moment: three points, three
    /// knocks.
    Smite,
}

impl Hit {
    pub(crate) fn points(self) -> u32 {
        match self {
            Hit::Miss => 0,
            Hit::Shield => 1,
            Hit::Broke | Hit::Clean => 2,
            Hit::Smite => 3,
        }
    }
    pub(crate) fn knocks(self) -> u32 {
        self.points()
    }
    /// A braced rider takes it one grade lighter.
    fn braced(self) -> Hit {
        match self {
            Hit::Smite => Hit::Clean,
            Hit::Clean | Hit::Broke => Hit::Shield,
            _ => Hit::Miss,
        }
    }
    pub(crate) fn word(self) -> &'static str {
        match self {
            Hit::Miss => "MISSED",
            Hit::Shield => "ON THE SHIELD",
            Hit::Broke => "LANCE BROKEN",
            Hit::Clean => "CLEAN HIT",
            Hit::Smite => "A MIGHTY HIT",
        }
    }
}

/// High at the helm, or low at the shield.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Aim {
    High,
    #[default]
    Low,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    /// At the ends of the lists, waiting for the spur.
    #[default]
    Ready,
    /// Running at each other; they meet at `charge` ticks in, and ride on
    /// to the far end.
    Charge,
    /// The bout is over: its verdict stands for a moment.
    Done,
}

/// How a bout ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    Won,
    Lost,
    Drawn,
}

/// A bout at the lists, as it stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Joust {
    /// The knight in the saddle.
    pub(crate) knight: u32,
    pub(crate) rival: u8,
    pub(crate) mount: Mount,
    pub(crate) tended: bool,
    /// The course being run: 0, 1 or 2.
    pub(crate) course: u8,
    pub(crate) stage: Stage,
    /// Ticks into the stage.
    pub(crate) t: u32,
    pub(crate) aim: Aim,
    /// Where the rival's shield is this course.
    pub(crate) guard: Aim,
    /// When F struck and Space braced, in ticks into the charge.
    pub(crate) struck: Option<u32>,
    pub(crate) braced: Option<u32>,
    /// Keys held last tick, so a held key acts once.
    #[serde(default)]
    held: (bool, bool),
    /// Knocks each rider can still take: the knight's, the rival's.
    pub(crate) balance: (u32, u32),
    pub(crate) score: (u32, u32),
    /// This course's lances, once the riders have met: the knight's, the
    /// rival's.
    pub(crate) hits: Option<(Hit, Hit)>,
    /// Who went over: the knight, the rival.
    pub(crate) fallen: (bool, bool),
    pub(crate) verdict: Option<Verdict>,
    /// The bout's number in the realm's record (it varies the rival's lance
    /// and guard from bout to bout).
    pub(crate) bout: u32,
}

/// The lists' lanes either side of the tilt, and its ends (arena units).
pub(crate) const SOUTH_LANE: f32 = 17.4;
pub(crate) const NORTH_LANE: f32 = 12.2;
pub(crate) const WEST_END: f32 = 4.0;
pub(crate) const EAST_END: f32 = 44.0;
pub(crate) const MEETING: f32 = 24.0;
/// The plate at the lists' west end where a knight mounts (tiles).
pub(crate) const MOUNT_PLATE: (i32, i32, i32, i32) = (1, 8, 2, 1);
/// How long a bout's verdict stands before the knight dismounts.
pub(crate) const VERDICT_TICKS: u32 = 4 * HZ;
/// How long a knight holds Space at the end to withdraw from a bout.
pub(crate) const WITHDRAW_HOLD: u32 = HZ;

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

impl Joust {
    pub(crate) fn rival(&self) -> &'static Rival {
        &RIVALS[usize::from(self.rival).min(RIVALS.len() - 1)]
    }

    /// The meeting, in ticks into the charge.
    pub(crate) fn meeting(&self) -> u32 {
        self.mount.steed().charge
    }

    /// The knight rides from the west on odd courses, from the east on even
    /// ones (the riders end each course at the far end).
    pub(crate) fn from_west(&self) -> bool {
        self.course.is_multiple_of(2)
    }

    /// How long the riders hold at the meeting, lances crossed, while a
    /// late press can still land: the clash, then the impact.
    pub(crate) fn clash(&self) -> u32 {
        let steed = self.mount.steed();
        steed.strike.max(steed.brace) + 1
    }

    /// Ticks the riders have ridden on past the clash (0 until it is over).
    pub(crate) fn since_impact(&self) -> Option<u32> {
        (self.stage == Stage::Charge && self.hits.is_some())
            .then(|| self.t.saturating_sub(self.meeting() + self.clash()))
    }

    /// How far along the lists a rider is, 0 at its own end to 1 at the far
    /// end: faster and faster to the meeting (halfway), held there for the
    /// clash, then running on and slowing.
    fn along(&self) -> f32 {
        match self.stage {
            Stage::Ready => 0.0,
            Stage::Done => 1.0,
            Stage::Charge => {
                let m = self.meeting() as f32;
                let t = self.t as f32;
                let after = m + self.clash() as f32;
                if t <= m {
                    0.5 * (t / m).powf(1.5)
                } else if t <= after {
                    0.5
                } else {
                    let k = ((t - after) / (m * 0.6)).min(1.0);
                    0.5 + 0.5 * (1.0 - (1.0 - k) * (1.0 - k))
                }
            }
        }
    }

    /// The riders are holding at the meeting.
    pub(crate) fn clashing(&self) -> bool {
        self.stage == Stage::Charge
            && self.t > self.meeting()
            && self.t <= self.meeting() + self.clash()
    }

    /// Where the knight and the rival are (arena units), and which way each
    /// faces (true: east).
    pub(crate) fn riders(&self) -> ((f32, f32, bool), (f32, f32, bool)) {
        let a = self.along();
        let span = EAST_END - WEST_END;
        let west = WEST_END + span * a;
        let east = EAST_END - span * a;
        if self.from_west() {
            ((west, SOUTH_LANE, true), (east, NORTH_LANE, false))
        } else {
            ((east, SOUTH_LANE, false), (west, NORTH_LANE, true))
        }
    }

    /// The rival's guard is shown once it can be read: `None` while his
    /// shield still wavers.
    pub(crate) fn guard_shown(&self) -> Option<Aim> {
        let tell = self.mount.steed().tell;
        (self.stage == Stage::Charge && self.t + tell >= self.meeting()).then_some(self.guard)
    }

    /// The timing ring's fill toward the meeting, 0 to 1, while charging.
    pub(crate) fn closing(&self) -> Option<f32> {
        (self.stage == Stage::Charge && self.t <= self.meeting())
            .then(|| self.t as f32 / self.meeting() as f32)
    }
}

/// A rider's timing: how near the meeting a press came.
fn timing(press: Option<u32>, meeting: u32, window: u32) -> u8 {
    match press {
        Some(t) if t.abs_diff(meeting) * 3 <= window => 2,
        Some(t) if t.abs_diff(meeting) <= window => 1,
        _ => 0,
    }
}

/// The rival's guard for a course, chosen without dice from the run's
/// seed, the bout and the course.
fn guard_for(seed: u64, bout: u32, course: u8) -> Aim {
    if mix(seed ^ (u64::from(bout) << 8) ^ u64::from(course)).is_multiple_of(2) {
        Aim::High
    } else {
        Aim::Low
    }
}

impl Run {
    /// Mount up at the lists: knight `id` against the next rival, on the
    /// realm's saddled mount.
    pub(crate) fn mount_up(&mut self, id: u32) {
        let stable = &self.home.stable;
        let mount = stable.selected;
        let tended = stable.is_tended(mount);
        let rival = next_rival(stable);
        let bout = stable.bouts;
        let balance = mount.steed().balance + if tended { TENDED_BALANCE } else { 0 };
        self.joust = Some(Joust {
            knight: id,
            rival: rival as u8,
            mount,
            tended,
            course: 0,
            stage: Stage::Ready,
            t: 0,
            aim: Aim::Low,
            guard: guard_for(self.seed, bout, 0),
            struck: None,
            braced: None,
            held: (true, false),
            balance: (balance, RIVALS[rival].balance),
            score: (0, 0),
            hits: None,
            fallen: (false, false),
            verdict: None,
            bout,
        });
        *self.marks.entry("joust:start".into()).or_default() += 1;
        self.cues.push(format!("joust_start:{}", RIVALS[rival].id));
        self.sounds.push("door_open");
        self.seat_rider();
    }

    /// Keep the knight in the saddle: where their horse is.
    fn seat_rider(&mut self) {
        let Some(j) = &self.joust else {
            return;
        };
        let ((x, y, east), _) = j.riders();
        if let Some(hero) = self.players.get_mut(&j.knight) {
            (hero.x, hero.y) = (x, y);
            (hero.aim_x, hero.aim_y) = (if east { 1.0 } else { -1.0 }, 0.0);
        }
    }

    /// One tick at the lists: the mount plate, and a bout if one is on.
    pub(super) fn tick_lists(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.joust.is_none() {
            // A knight who stands at the west end and holds F mounts up.
            let mut rider = None;
            for (&id, hero) in self.players.iter_mut() {
                let fire = inputs
                    .get(&id)
                    .copied()
                    .filter(|i| i.valid())
                    .is_some_and(|i| i.fire);
                if !fire {
                    hero.buy_spent = false;
                }
                let on = hero.hp > 0 && !hero.stone && inside(MOUNT_PLATE, hero.x, hero.y);
                if on && fire && !hero.buy_spent {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= super::home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        rider = rider.or(Some(id));
                    }
                } else if on {
                    hero.buying = 0;
                }
            }
            if let Some(id) = rider {
                self.mount_up(id);
            }
            return;
        }
        let Some(mut j) = self.joust.take() else {
            return;
        };
        let input = inputs
            .get(&j.knight)
            .copied()
            .filter(|i| i.valid())
            .unwrap_or_default();
        let (fire, brace) = (input.fire, input.dash);
        let (fire_was, brace_was) = j.held;
        j.held = (fire, brace);
        let mut ended = false;
        j.t += 1;
        match j.stage {
            Stage::Ready => {
                if fire && !fire_was {
                    j.stage = Stage::Charge;
                    j.t = 0;
                    j.aim = Aim::Low;
                    j.struck = None;
                    j.braced = None;
                    j.hits = None;
                    self.cues.push("joust_spur".into());
                    self.sounds.push("descend");
                }
                // Holding Space at the end withdraws from the bout.
                if brace {
                    if let Some(hero) = self.players.get_mut(&j.knight) {
                        hero.buying = hero.buying.saturating_add(1);
                        if hero.buying >= WITHDRAW_HOLD {
                            hero.buying = 0;
                            ended = true;
                        }
                    }
                } else if let Some(hero) = self.players.get_mut(&j.knight) {
                    hero.buying = 0;
                }
            }
            Stage::Charge => {
                let meeting = j.meeting();
                let steed = j.mount.steed();
                // The lances meet once the latest window has closed (the
                // riders hold for it): a late press still counts inside it.
                let window = j.clash() - 1;
                let _ = steed;
                if j.t < meeting {
                    if input.move_y < 0 {
                        j.aim = Aim::High;
                    } else if input.move_y > 0 {
                        j.aim = Aim::Low;
                    }
                }
                if j.t <= meeting + window {
                    if fire && !fire_was && j.struck.is_none() {
                        j.struck = Some(j.t);
                    }
                    if brace && !brace_was && j.braced.is_none() {
                        j.braced = Some(j.t);
                    }
                }
                if j.t == meeting + window + 1 {
                    self.meet(&mut j);
                }
                if j.t >= meeting + meeting * 3 / 5 + window {
                    if j.fallen.0 || j.fallen.1 || j.course >= 2 {
                        j.stage = Stage::Done;
                        j.t = 0;
                        j.verdict = Some(match (j.fallen, j.score.0.cmp(&j.score.1)) {
                            ((false, true), _) => Verdict::Won,
                            ((true, false), _) => Verdict::Lost,
                            ((true, true), _) => Verdict::Drawn,
                            (_, std::cmp::Ordering::Greater) => Verdict::Won,
                            (_, std::cmp::Ordering::Less) => Verdict::Lost,
                            _ => Verdict::Drawn,
                        });
                        let rival = j.rival().id;
                        let verdict = match j.verdict {
                            Some(Verdict::Won) => "won",
                            Some(Verdict::Lost) => "lost",
                            _ => "drawn",
                        };
                        *self
                            .marks
                            .entry(format!("joust:{rival}:{verdict}"))
                            .or_default() += 1;
                        if j.fallen.1 {
                            *self.marks.entry("joust:unhorsed".into()).or_default() += 1;
                            self.notice("unhorsed");
                            if rival == "lancelot" {
                                self.notice("champion");
                            }
                        } else if j.verdict == Some(Verdict::Won) && rival == "lancelot" {
                            self.notice("champion");
                        }
                        self.cues.push(format!("joust_{verdict}:{rival}"));
                    } else {
                        j.course += 1;
                        j.stage = Stage::Ready;
                        j.t = 0;
                        j.guard = guard_for(self.seed, j.bout, j.course);
                        j.hits = None;
                    }
                }
            }
            Stage::Done => {
                if j.t >= VERDICT_TICKS {
                    ended = true;
                }
            }
        }
        if ended {
            // Down from the saddle, at the mount plate.
            let (c, r, w, h) = MOUNT_PLATE;
            if let Some(hero) = self.players.get_mut(&j.knight) {
                hero.x = (c as f32 + w as f32 / 2.0) * TILE_UNITS;
                hero.y = (r as f32 + h as f32 + 0.6) * TILE_UNITS;
                hero.buy_spent = true;
            }
            return;
        }
        self.joust = Some(j);
        self.seat_rider();
    }

    /// The riders meet: what each lance did, the knocks and the points, and
    /// who goes over.
    fn meet(&mut self, j: &mut Joust) {
        let steed = j.mount.steed();
        let meeting = j.meeting();
        // The helm is a smaller mark than the shield.
        let window = if j.aim == Aim::High {
            steed.strike * 2 / 3
        } else {
            steed.strike
        };
        let open = j.aim != j.guard;
        let mine = match (timing(j.struck, meeting, window), open) {
            (0, _) => Hit::Miss,
            (1, false) => Hit::Shield,
            (_, false) => Hit::Broke,
            (1, true) => Hit::Clean,
            (_, true) => Hit::Smite,
        };
        let lance = j.rival().lance;
        let theirs = lance[((j.bout + u32::from(j.course)) % lance.len() as u32) as usize];
        let theirs = if timing(j.braced, meeting, steed.brace) > 0 {
            theirs.braced()
        } else {
            theirs
        };
        let weight = if matches!(mine, Hit::Clean | Hit::Smite) {
            steed.weight
        } else {
            0
        };
        j.balance.1 = j.balance.1.saturating_sub(mine.knocks() + weight);
        j.balance.0 = j.balance.0.saturating_sub(theirs.knocks());
        j.score.0 += mine.points();
        j.score.1 += theirs.points();
        j.fallen = (j.balance.0 == 0, j.balance.1 == 0);
        j.hits = Some((mine, theirs));
        if mine != Hit::Miss || theirs != Hit::Miss {
            self.shake = self.shake.max(if mine == Hit::Smite { 12 } else { 6 });
            self.sounds.push("boss_hit");
        }
        self.cues.push(match (j.fallen, mine) {
            ((_, true), _) => "joust_unhorse".into(),
            ((true, _), _) => "joust_fell".into(),
            (_, Hit::Smite) => "joust_smite".into(),
            (_, Hit::Broke) => "joust_broke".into(),
            (_, Hit::Clean) => "joust_clean".into(),
            (_, Hit::Shield) => "joust_shield".into(),
            _ => "joust_miss".into(),
        });
    }

    /// The stables: a stall's plate saddles its mount, the trough's tends
    /// the saddled one.
    pub(super) fn tick_stables(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut asked: Vec<String> = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
            }
            let plate = STALL_PLATES
                .iter()
                .position(|&p| inside(p, hero.x, hero.y))
                .map(|i| format!("stable:select:{}", Mount::ALL[i].name().to_lowercase()))
                .or_else(|| inside(TEND_PLATE, hero.x, hero.y).then(|| "stable:tend".into()));
            match plate {
                Some(what) if hero.hp > 0 && !hero.stone && fire && !hero.buy_spent => {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= super::home::BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push(what);
                    }
                }
                _ => hero.buying = 0,
            }
        }
        for what in asked {
            *self.marks.entry(what).or_default() += 1;
            self.sounds.push("play_card");
        }
        // The groom has a word for whoever walks up.
        let (gx, gy) = (GROOM_AT.0 * TILE_UNITS, GROOM_AT.1 * TILE_UNITS);
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - gx).hypot(h.y - gy) < 3.6);
        if near && !self.groom_near {
            self.cues.push("npc:groom".into());
        }
        self.groom_near = near;
    }
}

/// The cockpit's half of the stables and the lists: what the run (or a
/// command) marked, kept in the realm's stable. Returns the lines to say,
/// with their chorus cues.
pub(crate) fn settle_stable(
    stable: &mut Stable,
    marks: &BTreeMap<String, u32>,
) -> Vec<(String, String)> {
    let mut said = Vec::new();
    for mark in marks.keys() {
        if let Some(name) = mark.strip_prefix("stable:select:") {
            if let Some(m) = Mount::parse(name)
                && m != stable.selected
            {
                stable.selected = m;
                said.push((
                    format!("stable_select:{name}"),
                    format!("{} is saddled for the lists: {}", m.name(), m.says()),
                ));
            }
        } else if mark == "stable:tend" {
            let m = stable.selected;
            if stable.is_tended(m) {
                said.push((
                    "cant_afford".into(),
                    format!("{} is tended already: one bout's worth at a time", m.name()),
                ));
            } else {
                stable.tended[m.index()] = true;
                said.push((
                    "stable_tend".into(),
                    format!(
                        "{} brushed, watered and the tack checked: a knock more of balance for the next bout",
                        m.name()
                    ),
                ));
            }
        } else if mark == "joust:start" {
            stable.tended[stable.selected.index()] = false;
        } else if mark == "joust:unhorsed" {
            stable.unhorsed += 1;
        } else if let Some(rest) = mark.strip_prefix("joust:") {
            let mut it = rest.split(':');
            let (Some(rival), Some(verdict)) = (it.next(), it.next()) else {
                continue;
            };
            let Some(r) = RIVALS.iter().find(|r| r.id == rival) else {
                continue;
            };
            stable.bouts += 1;
            let line = match verdict {
                "won" => {
                    *stable.wins.entry(rival.into()).or_default() += 1;
                    format!("You beat {} at the lists", r.name)
                }
                "lost" => format!("{} had the better of you at the lists", r.name),
                _ => format!("A draw with {} at the lists", r.name),
            };
            said.push((format!("joust_result:{verdict}"), line));
        }
    }
    said
}

/// The plates before the three stall doors (Bramble, Cinder, Mist), and
/// the trough's, where the saddled mount is tended (tiles).
pub(crate) const STALL_PLATES: [(i32, i32, i32, i32); 3] =
    [(5, 4, 2, 1), (11, 4, 2, 1), (17, 4, 2, 1)];
pub(crate) const TEND_PLATE: (i32, i32, i32, i32) = (3, 7, 2, 1);
/// Where the groom stands, by the trough, and where the saddled mount waits
/// in the yard (tiles).
pub(crate) const GROOM_AT: (f32, f32) = (6.6, 7.6);
pub(crate) const LED_AT: (f32, f32) = (9.6, 8.6);

/// What the jouster's keys do for the rest of the tick: nothing but the
/// joust (they are in the saddle).
pub(crate) fn reins(inputs: &BTreeMap<u32, Input>, rider: u32) -> BTreeMap<u32, Input> {
    let mut out = inputs.clone();
    out.insert(rider, Input::default());
    out
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_joust__tests.rs"]
mod tests;
