//! The joust, at the lists: two knights of the party down the tilt at each
//! other, best of three passes, with the rest of the party in the gallery
//! wagering real gold on them. With no friend at the lists, one of the
//! realm's champions rides against the knight.
//!
//! A pass: the riders wait at their ends; F spurs (with two knights the
//! herald drops the flag once both have, or when his patience runs out).
//! The horses run at each other down either side of the tilt, faster all
//! the way, and meet. Until the couch, a moment before the meeting, each
//! rider sets his lance (W high at the helm, S low at the chest) and his
//! shield (E: up by the helm, or down over the chest). Both show, so a late
//! switch is the whole game. Then F strikes, as the lances meet: on the
//! shield is a point, where the shield isn't is two, and at the very moment
//! one more. A knight knocked off balance enough times goes over his
//! horse's tail and the bout is over; otherwise the better lance takes each
//! pass, and two passes take the bout.
//!
//! The horse matters: each rides differently (`Mount::steed`). Its charge
//! is its speed (a faster horse meets sooner and further down the lists),
//! its balance is its heart, and a tended one carries its rider a knock
//! longer. Before the first pass each rider picks a horse at their end (A
//! and D).
//!
//! The purse is real: both entries and the Crown's share go to the winner,
//! banked under their name; a loser's entry and a lost wager are taken from
//! the realm's treasury at the save. Everything here is a pure function of
//! the inputs and the seed, so a mirror and a test see the same bout.

use super::*;
use crate::realm::chivalry::{Mount, Stable, Steed, TENDED_BALANCE};

pub mod show;

pub struct Rival {
    pub id: &'static str,
    pub name: &'static str,
    pub colours: (char, char),
    pub balance: u32,
    pub lance: [Hit; 5],
}

pub const RIVALS: [Rival; 3] = [
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

pub fn next_rival(stable: &Stable) -> usize {
    RIVALS
        .iter()
        .position(|r| stable.wins.get(r.id).copied().unwrap_or(0) == 0)
        .unwrap_or(RIVALS.len() - 1)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hit {
    #[default]
    Miss,
    Shield,
    Broke,
    Clean,
    Smite,
}

impl Hit {
    pub fn points(self) -> u32 {
        match self {
            Hit::Miss => 0,
            Hit::Shield => 1,
            Hit::Broke | Hit::Clean => 2,
            Hit::Smite => 3,
        }
    }
    pub fn knocks(self) -> u32 {
        self.points()
    }
    fn timed(self) -> u8 {
        match self {
            Hit::Miss => 0,
            Hit::Shield | Hit::Clean => 1,
            Hit::Broke | Hit::Smite => 2,
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Hit::Miss => "MISSED",
            Hit::Shield => "ON THE SHIELD",
            Hit::Broke => "LANCE BROKEN",
            Hit::Clean => "CLEAN HIT",
            Hit::Smite => "A MIGHTY HIT",
        }
    }
    pub fn flash(self, aim: Aim) -> &'static str {
        match (self, aim) {
            (Hit::Miss, _) => "MISS",
            (Hit::Shield, _) => "SHIELD",
            (Hit::Broke, _) => "LANCE BROKE",
            (Hit::Clean, Aim::High) => "HELM",
            (Hit::Smite, Aim::High) => "HELM RUNG",
            (Hit::Clean, Aim::Low) => "CHEST",
            (Hit::Smite, Aim::Low) => "MIGHTY HIT",
        }
    }
}

fn grade(timed: u8, open: bool) -> Hit {
    match (timed, open) {
        (0, _) => Hit::Miss,
        (1, false) => Hit::Shield,
        (_, false) => Hit::Broke,
        (1, true) => Hit::Clean,
        (_, true) => Hit::Smite,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aim {
    High,
    #[default]
    Low,
}

impl Aim {
    fn other(self) -> Self {
        match self {
            Aim::High => Aim::Low,
            Aim::Low => Aim::High,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Ready,
    Charge,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Won,
    Lost,
    Drawn,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    fire: bool,
    interact: bool,
    side: i8,
}

impl Held {
    fn of(input: Input) -> Self {
        Held {
            fire: input.fire,
            interact: input.interact,
            side: side_key(input),
        }
    }
}

fn side_key(input: Input) -> i8 {
    if input.move_x != 0 {
        input.move_x.signum()
    } else {
        input.aim_x.signum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub id: u32,
    pub mount: Mount,
    pub tended: bool,
    pub aim: Aim,
    pub shield: Aim,
    pub struck: Option<u32>,
    pub spurred: bool,
    #[serde(default)]
    keys: Held,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wager {
    pub side: Option<u8>,
    pub stake: u32,
    pub from: (f32, f32),
    #[serde(default)]
    keys: Held,
    #[serde(default)]
    pub won: Option<bool>,
    #[serde(default)]
    pub threw: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Joust {
    pub knight: u32,
    pub rival: u8,
    pub mount: Mount,
    pub tended: bool,
    pub course: u8,
    pub stage: Stage,
    pub t: u32,
    pub aim: Aim,
    pub guard: Aim,
    pub struck: Option<u32>,
    #[serde(default)]
    pub shield: Aim,
    #[serde(default)]
    pub spurred: bool,
    #[serde(default)]
    keys: Held,
    pub balance: (u32, u32),
    pub score: (u32, u32),
    #[serde(default)]
    pub passes: (u8, u8),
    pub hits: Option<(Hit, Hit)>,
    pub fallen: (bool, bool),
    pub verdict: Option<Verdict>,
    pub bout: u32,
    #[serde(default)]
    pub foe: Option<Foe>,
    #[serde(default)]
    pub lance: Aim,
    #[serde(default)]
    pub gallery: BTreeMap<u32, Wager>,
    #[serde(default)]
    pub purse: u32,
    #[serde(default)]
    pub roar: (u8, u8),
    #[serde(default, skip_serializing_if = "show::Show::quiet")]
    pub show: show::Show,
}

pub const SOUTH_LANE: f32 = 17.4;
pub const NORTH_LANE: f32 = 12.2;
pub const WEST_END: f32 = 4.0;
pub const EAST_END: f32 = 44.0;
pub const MEETING: f32 = 24.0;
pub const MOUNT_PLATE: (i32, i32, i32, i32) = (1, 8, 2, 1);
pub const VERDICT_TICKS: u32 = 9 * HZ;
pub const WITHDRAW_HOLD: u32 = HZ;
pub const COUCH: u32 = 12;
pub const FLAG_TICKS: u32 = 5 * HZ;
pub const FIRST_FLAG_TICKS: u32 = 10 * HZ;
pub const ENTRY: u32 = 20;
pub const CROWN: u32 = 20;
pub const PURSE: u32 = 2 * ENTRY + CROWN;
pub const STAKE: u32 = 10;
pub const MAX_STAKE: u32 = 30;
pub const PAID_MARK: &str = "lists:paid";
pub const GALLERY_SEATS: [(f32, f32); 3] = [(11.0, 7.0), (37.0, 7.0), (15.0, 7.0)];

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

impl Joust {
    pub fn rival(&self) -> &'static Rival {
        &RIVALS[usize::from(self.rival).min(RIVALS.len() - 1)]
    }

    pub fn valid(&self) -> bool {
        let seat = |id: u32| (1..=MAX_PLAYERS).contains(&id);
        let foe = self.foe.as_ref().map(|f| f.id);
        self.course < 3
            && usize::from(self.rival) < RIVALS.len()
            && self.t <= 60 * HZ
            && self.passes.0 <= 3
            && self.passes.1 <= 3
            && self.balance.0 <= 16
            && self.balance.1 <= 16
            && self.roar.0 <= MAX_PLAYERS as u8
            && self.roar.1 <= MAX_PLAYERS as u8
            && self.show.valid()
            && self.purse <= PURSE
            && seat(self.knight)
            && foe.is_none_or(|id| seat(id) && id != self.knight)
            && self.gallery.len() < MAX_PLAYERS as usize
            && self.gallery.iter().all(|(&id, w)| {
                seat(id)
                    && id != self.knight
                    && Some(id) != foe
                    && w.stake <= MAX_STAKE
                    && w.side.is_none_or(|s| s < 2)
                    && w.from.0.is_finite()
                    && w.from.1.is_finite()
            })
    }

    pub fn steeds(&self) -> (Steed, Steed) {
        let mine = self.mount.steed();
        (mine, self.foe.as_ref().map_or(mine, |f| f.mount.steed()))
    }

    pub fn meeting(&self) -> u32 {
        let (a, b) = (self.steeds().0.charge, self.steeds().1.charge);
        (2 * a * b + (a + b) / 2) / (a + b)
    }

    pub fn share(&self) -> f32 {
        let (a, b) = (self.steeds().0.charge, self.steeds().1.charge);
        b as f32 / (a + b) as f32
    }

    pub fn from_west(&self) -> bool {
        self.course.is_multiple_of(2)
    }

    pub fn clash(&self) -> u32 {
        let (a, b) = self.steeds();
        a.strike.max(a.brace).max(b.strike).max(b.brace) + 1
    }

    pub fn since_impact(&self) -> Option<u32> {
        (self.stage == Stage::Charge && self.hits.is_some())
            .then(|| self.t.saturating_sub(self.meeting() + self.clash()))
    }

    fn along(&self, share: f32) -> f32 {
        match self.stage {
            Stage::Ready => 0.0,
            Stage::Done => 1.0,
            Stage::Charge => self.along_at(share, self.t),
        }
    }

    fn along_at(&self, share: f32, at: u32) -> f32 {
        let m = self.meeting() as f32;
        let t = at as f32;
        let after = m + self.clash() as f32;
        if t <= m {
            share * (t / m).powf(1.5)
        } else if t <= after {
            share
        } else {
            let k = ((t - after) / (m * 0.6)).min(1.0);
            share + (1.0 - share) * (1.0 - (1.0 - k) * (1.0 - k))
        }
    }

    pub fn clashing(&self) -> bool {
        self.stage == Stage::Charge
            && self.t > self.meeting()
            && self.t <= self.meeting() + self.clash()
    }

    pub fn meeting_x(&self) -> f32 {
        let span = EAST_END - WEST_END;
        if self.from_west() {
            WEST_END + span * self.share()
        } else {
            EAST_END - span * self.share()
        }
    }

    pub fn riders(&self) -> ((f32, f32, bool), (f32, f32, bool)) {
        let share = self.share();
        let (a, b) = (self.along(share), self.along(1.0 - share));
        let span = EAST_END - WEST_END;
        if self.from_west() {
            (
                (WEST_END + span * a, SOUTH_LANE, true),
                (EAST_END - span * b, NORTH_LANE, false),
            )
        } else {
            (
                (EAST_END - span * a, SOUTH_LANE, false),
                (WEST_END + span * b, NORTH_LANE, true),
            )
        }
    }

    pub fn riders_at(&self, at: u32) -> ((f32, f32, bool), (f32, f32, bool)) {
        let share = self.share();
        let (a, b) = (self.along_at(share, at), self.along_at(1.0 - share, at));
        let span = EAST_END - WEST_END;
        if self.from_west() {
            (
                (WEST_END + span * a, SOUTH_LANE, true),
                (EAST_END - span * b, NORTH_LANE, false),
            )
        } else {
            (
                (EAST_END - span * a, SOUTH_LANE, false),
                (WEST_END + span * b, NORTH_LANE, true),
            )
        }
    }

    pub fn unset(&self) -> bool {
        self.stage == Stage::Charge && self.t + COUCH < self.meeting()
    }

    pub fn guard_shown(&self) -> Option<Aim> {
        match &self.foe {
            Some(f) => Some(f.shield),
            None => self.told().then_some(self.guard),
        }
    }

    pub fn lance_shown(&self) -> Option<Aim> {
        match &self.foe {
            Some(f) => Some(f.aim),
            None => self.told().then_some(self.lance),
        }
    }

    fn told(&self) -> bool {
        let tell = self.mount.steed().tell;
        self.stage == Stage::Charge && self.t + tell >= self.meeting()
    }

    pub fn closing(&self) -> Option<f32> {
        (self.stage == Stage::Charge && self.t <= self.meeting())
            .then(|| self.t as f32 / self.meeting() as f32)
    }

    pub fn striking(&self) -> bool {
        let m = self.meeting();
        self.stage == Stage::Charge && self.hits.is_none() && self.t + self.steeds().0.strike >= m
    }

    pub fn rider(&self, side: u8) -> Option<u32> {
        match side {
            0 => Some(self.knight),
            _ => self.foe.as_ref().map(|f| f.id),
        }
    }

    pub fn seated(&self) -> Vec<u32> {
        std::iter::once(self.knight)
            .chain(self.foe.as_ref().map(|f| f.id))
            .chain(self.gallery.keys().copied())
            .collect()
    }

    pub fn exposure(&self) -> u32 {
        let entries = if self.purse > 0 {
            ENTRY * (1 + u32::from(self.foe.is_some()))
        } else {
            0
        };
        entries + self.gallery.values().map(|w| w.stake).sum::<u32>()
    }

    pub fn winner(&self) -> Option<u8> {
        match self.verdict {
            Some(Verdict::Won) => Some(0),
            Some(Verdict::Lost) => Some(1),
            _ => None,
        }
    }

    fn balance_on(mount: Mount, tended: bool) -> u32 {
        mount.steed().balance + if tended { TENDED_BALANCE } else { 0 }
    }
}

fn timing(press: Option<u32>, meeting: u32, window: u32) -> u8 {
    match press {
        Some(t) if t.abs_diff(meeting) * 3 <= window => 2,
        Some(t) if t.abs_diff(meeting) <= window => 1,
        _ => 0,
    }
}

fn window(steed: Steed, aim: Aim) -> u32 {
    if aim == Aim::High {
        steed.strike * 2 / 3
    } else {
        steed.strike
    }
}

fn guard_for(seed: u64, bout: u32, course: u8) -> Aim {
    if mix(seed ^ (u64::from(bout) << 8) ^ u64::from(course)).is_multiple_of(2) {
        Aim::High
    } else {
        Aim::Low
    }
}

fn lance_for(seed: u64, bout: u32, course: u8) -> Aim {
    if mix(seed ^ 0x6C61_6E63 ^ (u64::from(bout) << 8) ^ u64::from(course)).is_multiple_of(2) {
        Aim::High
    } else {
        Aim::Low
    }
}

fn next_mount(m: Mount, step: i8) -> Mount {
    let i = (m.index() as i32 + i32::from(step)).rem_euclid(Mount::ALL.len() as i32);
    Mount::ALL[i as usize]
}

impl Run {
    pub fn joust_showing(&self) -> bool {
        self.joust.is_some() && self.room().kind == RoomKind::Lists
    }

    fn at_the_lists(&self) -> Vec<u32> {
        self.players
            .iter()
            .filter(|(id, h)| h.hp > 0 && !h.stone && !self.exits.outside.contains_key(id))
            .map(|(&id, _)| id)
            .collect()
    }

    pub fn lists_free_gold(&self) -> u32 {
        let owed = self.marks.get(PAID_MARK).copied().unwrap_or(0);
        let riding = self.joust.as_ref().map_or(0, Joust::exposure);
        self.treasury
            .get(Spoil::Gold)
            .saturating_sub(owed)
            .saturating_sub(riding)
    }

    pub fn challenger(&self, id: u32) -> Option<u32> {
        let me = self.players.get(&id)?;
        self.at_the_lists()
            .into_iter()
            .filter(|&other| other != id)
            .min_by(|a, b| {
                let d = |k: &u32| {
                    let h = &self.players[k];
                    (h.x - me.x).hypot(h.y - me.y)
                };
                d(a).total_cmp(&d(b)).then(a.cmp(b))
            })
    }

    pub fn mount_up(&mut self, id: u32) {
        let challenger = self.challenger(id);
        let gallery: BTreeMap<u32, Wager> = self
            .at_the_lists()
            .into_iter()
            .filter(|&k| k != id && Some(k) != challenger)
            .filter_map(|k| {
                let h = self.players.get(&k)?;
                Some((
                    k,
                    Wager {
                        side: None,
                        stake: 0,
                        from: (h.x, h.y),
                        keys: Held {
                            fire: true,
                            interact: true,
                            side: 0,
                        },
                        won: None,
                        threw: false,
                    },
                ))
            })
            .collect();
        let stable = &self.home.stable;
        let mount = stable.selected;
        let tended = stable.is_tended(mount);
        let rival = next_rival(stable);
        let bout = stable.bouts;
        let held = Held {
            fire: true,
            interact: true,
            side: 0,
        };
        let foe = challenger.map(|fid| {
            let fm = next_mount(mount, 1);
            Foe {
                id: fid,
                mount: fm,
                tended: stable.is_tended(fm),
                aim: Aim::Low,
                shield: Aim::Low,
                struck: None,
                spurred: false,
                keys: held,
            }
        });
        let theirs = foe
            .as_ref()
            .map_or(RIVALS[rival].balance, |f| Joust::balance_on(f.mount, f.tended));
        let entries = ENTRY * (1 + u32::from(foe.is_some()));
        let purse = if self.lists_free_gold() >= entries {
            PURSE
        } else {
            0
        };
        let friendly = foe.is_some();
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
            shield: Aim::Low,
            spurred: false,
            keys: held,
            balance: (Joust::balance_on(mount, tended), theirs),
            score: (0, 0),
            passes: (0, 0),
            hits: None,
            fallen: (false, false),
            verdict: None,
            bout,
            foe,
            lance: lance_for(self.seed, bout, 0),
            gallery,
            purse,
            roar: (0, 0),
            show: show::Show::default(),
        });
        *self.marks.entry("joust:start".into()).or_default() += 1;
        self.cues.push(if friendly {
            "joust_start:friend".into()
        } else {
            format!("joust_start:{}", RIVALS[rival].id)
        });
        self.sounds.push("door_open");
        self.seat_riders();
    }

    fn seat_riders(&mut self) {
        let Some(j) = &self.joust else {
            return;
        };
        let (a, b) = j.riders();
        let mut places = vec![(j.knight, a)];
        if let Some(f) = &j.foe {
            places.push((f.id, b));
        }
        for (id, (x, y, east)) in places {
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = (x, y);
                (hero.aim_x, hero.aim_y) = (if east { 1.0 } else { -1.0 }, 0.0);
            }
        }
        for (i, &id) in j.gallery.keys().enumerate() {
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = GALLERY_SEATS[i.min(GALLERY_SEATS.len() - 1)];
                (hero.aim_x, hero.aim_y) = (0.0, 1.0);
            }
        }
    }

    pub(super) fn tick_lists(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.joust.is_none() {
            let mut rider = None;
            for (&id, hero) in self.players.iter_mut() {
                let pressing = inputs
                    .get(&id)
                    .copied()
                    .filter(|i| i.valid())
                    .is_some_and(|i| i.interact);
                if !pressing {
                    hero.buy_spent = false;
                }
                let on = hero.hp > 0 && !hero.stone && inside(MOUNT_PLATE, hero.x, hero.y);
                if on && pressing && !hero.buy_spent {
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
        if !self.players.contains_key(&j.knight)
            || j.foe.as_ref().is_some_and(|f| !self.players.contains_key(&f.id))
        {
            self.get_down(&j);
            return;
        }
        j.gallery.retain(|id, _| self.players.contains_key(id));
        let key = |id: u32| {
            inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default()
        };
        let mine = key(j.knight);
        let theirs = j.foe.as_ref().map(|f| key(f.id));
        let mut ended = false;
        j.t += 1;
        j.show.tick();
        match j.stage {
            Stage::Ready => {
                if j.course == 0 {
                    let step = side_key(mine);
                    if step != 0 && step != j.keys.side && !j.spurred {
                        j.mount = next_mount(j.mount, step);
                        j.tended = self.home.stable.is_tended(j.mount);
                        j.balance.0 = Joust::balance_on(j.mount, j.tended);
                        self.sounds.push("play_card");
                    }
                    if let (Some(f), Some(input)) = (j.foe.as_mut(), theirs) {
                        let step = side_key(input);
                        if step != 0 && step != f.keys.side && !f.spurred {
                            f.mount = next_mount(f.mount, step);
                            f.tended = self.home.stable.is_tended(f.mount);
                            j.balance.1 = Joust::balance_on(f.mount, f.tended);
                            self.sounds.push("play_card");
                        }
                    }
                }
                if mine.fire && !j.keys.fire && !j.spurred {
                    j.spurred = true;
                    j.show.spurred[0] = Some(j.show.t);
                    self.sounds.push("step_wood");
                }
                if let (Some(f), Some(input)) = (j.foe.as_mut(), theirs)
                    && input.fire
                    && !f.keys.fire
                    && !f.spurred
                {
                    f.spurred = true;
                    j.show.spurred[1] = Some(j.show.t);
                    self.sounds.push("step_wood");
                }
                let patience = if j.course == 0 {
                    FIRST_FLAG_TICKS
                } else {
                    FLAG_TICKS
                };
                let off = match &j.foe {
                    None => j.spurred,
                    Some(f) => (j.spurred && f.spurred) || j.t >= patience,
                };
                if j.course == 0 && !off {
                    self.take_wagers(&mut j, inputs);
                }
                if off {
                    j.stage = Stage::Charge;
                    j.t = 0;
                    j.aim = Aim::Low;
                    j.struck = None;
                    j.spurred = false;
                    j.hits = None;
                    if let Some(f) = j.foe.as_mut() {
                        f.aim = Aim::Low;
                        f.struck = None;
                        f.spurred = false;
                    }
                    self.cues.push("joust_spur".into());
                    self.sounds.push("descend");
                }
                let mut riders = vec![(j.knight, mine.dash, 0u8)];
                if let (Some(f), Some(input)) = (&j.foe, theirs) {
                    riders.push((f.id, input.dash, 1));
                }
                for (id, dash, side) in riders {
                    let Some(hero) = self.players.get_mut(&id) else {
                        continue;
                    };
                    if !dash || off {
                        hero.buying = 0;
                        continue;
                    }
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= WITHDRAW_HOLD && !ended && j.verdict.is_none() {
                        hero.buying = 0;
                        if j.course == 0 {
                            ended = true;
                        } else {
                            j.verdict = Some(if side == 0 {
                                Verdict::Lost
                            } else {
                                Verdict::Won
                            });
                            j.stage = Stage::Done;
                            j.t = 0;
                            self.close_bout(&mut j);
                        }
                    }
                }
            }
            Stage::Charge => {
                let meeting = j.meeting();
                let window = j.clash() - 1;
                if j.unset() {
                    let before = (j.aim, j.shield);
                    if mine.move_y < 0 {
                        j.aim = Aim::High;
                    } else if mine.move_y > 0 {
                        j.aim = Aim::Low;
                    }
                    if mine.interact && !j.keys.interact {
                        j.shield = j.shield.other();
                        self.sounds.push("shield_up");
                    }
                    if j.aim != before.0 {
                        j.show.aimed[0] = Some(j.show.t);
                    }
                    if j.shield != before.1 {
                        j.show.shielded[0] = Some(j.show.t);
                    }
                    if let (Some(f), Some(input)) = (j.foe.as_mut(), theirs) {
                        let before = (f.aim, f.shield);
                        if input.move_y < 0 {
                            f.aim = Aim::High;
                        } else if input.move_y > 0 {
                            f.aim = Aim::Low;
                        }
                        if input.interact && !f.keys.interact {
                            f.shield = f.shield.other();
                            self.sounds.push("shield_up");
                        }
                        if f.aim != before.0 {
                            j.show.aimed[1] = Some(j.show.t);
                        }
                        if f.shield != before.1 {
                            j.show.shielded[1] = Some(j.show.t);
                        }
                    }
                }
                self.cheer_from_the_gallery(&mut j, inputs);
                if let Some(f) = j.foe.as_mut() {
                    j.guard = f.shield;
                }
                if j.t <= meeting + window {
                    if mine.fire && !j.keys.fire && j.struck.is_none() {
                        j.struck = Some(j.t);
                    }
                    if let (Some(f), Some(input)) = (j.foe.as_mut(), theirs)
                        && input.fire
                        && !f.keys.fire
                        && f.struck.is_none()
                    {
                        f.struck = Some(j.t);
                    }
                }
                if j.t == meeting + window + 1 {
                    self.meet(&mut j);
                }
                if j.t >= meeting + meeting * 3 / 5 + window {
                    self.end_pass(&mut j);
                }
            }
            Stage::Done => {
                if j.t >= VERDICT_TICKS {
                    ended = true;
                }
            }
        }
        j.keys = Held::of(mine);
        if let (Some(f), Some(input)) = (j.foe.as_mut(), theirs) {
            f.keys = Held::of(input);
        }
        for (&id, w) in j.gallery.iter_mut() {
            w.keys = Held::of(key(id));
        }
        if ended {
            self.get_down(&j);
            return;
        }
        self.joust = Some(j);
        self.seat_riders();
    }

    fn cheer_from_the_gallery(&mut self, j: &mut Joust, inputs: &BTreeMap<u32, Input>) {
        if j.stage != Stage::Charge || j.t >= j.meeting() {
            return;
        }
        let ids: Vec<u32> = j.gallery.keys().copied().collect();
        for id in ids {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            let Some(w) = j.gallery.get_mut(&id) else {
                continue;
            };
            let (Some(side), true) = (w.side, fire && !w.keys.fire && !w.threw) else {
                continue;
            };
            w.threw = true;
            if side == 0 {
                j.roar.0 += 1;
            } else {
                j.roar.1 += 1;
            }
            j.show.throw(id, side);
            self.sounds.push("play_card");
        }
    }

    fn take_wagers(&mut self, j: &mut Joust, inputs: &BTreeMap<u32, Input>) {
        let ids: Vec<u32> = j.gallery.keys().copied().collect();
        for id in ids {
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            let free = self.lists_free_gold().saturating_sub(j.exposure());
            let Some(w) = j.gallery.get_mut(&id) else {
                continue;
            };
            let side = side_key(input);
            if side != 0 && side != w.keys.side {
                let pick = if side < 0 { 0 } else { 1 };
                if w.stake == 0 {
                    if free >= STAKE {
                        w.stake = STAKE;
                        w.side = Some(pick);
                        self.sounds.push("gold");
                    } else {
                        self.found = Some((self.tick, id, "PURSE SHORT".into()));
                    }
                } else if w.side != Some(pick) {
                    w.side = Some(pick);
                    self.sounds.push("play_card");
                }
            }
            if input.fire && !w.keys.fire && w.side.is_some() && w.stake < MAX_STAKE {
                if free >= STAKE {
                    w.stake += STAKE;
                    self.sounds.push("gold");
                } else {
                    self.found = Some((self.tick, id, "PURSE SHORT".into()));
                }
            }
            w.keys = Held::of(input);
        }
    }

    fn meet(&mut self, j: &mut Joust) {
        let (a, b) = j.steeds();
        let meeting = j.meeting();
        let mine = grade(
            timing(j.struck, meeting, window(a, j.aim)),
            j.aim != j.guard,
        );
        let theirs = match &j.foe {
            Some(f) => grade(
                timing(f.struck, meeting, window(b, f.aim)),
                f.aim != j.shield,
            ),
            None => {
                let lance = j.rival().lance;
                let timed = lance[((j.bout + u32::from(j.course)) % lance.len() as u32) as usize];
                grade(timed.timed(), j.lance != j.shield)
            }
        };
        let open = |h: Hit| matches!(h, Hit::Clean | Hit::Smite);
        let weight = |h: Hit, s: Steed| if open(h) { s.weight } else { 0 };
        let theirs_weight = if j.foe.is_some() { weight(theirs, b) } else { 0 };
        let (knock0, knock1) = (theirs.knocks() + theirs_weight, mine.knocks() + weight(mine, a));
        let steady = (j.roar.0 > 0 && knock0 > 0, j.roar.1 > 0 && knock1 > 0);
        j.balance.1 = j.balance.1.saturating_sub(knock1 - u32::from(steady.1));
        j.balance.0 = j.balance.0.saturating_sub(knock0 - u32::from(steady.0));
        j.score.0 += mine.points();
        j.score.1 += theirs.points();
        match mine.points().cmp(&theirs.points()) {
            std::cmp::Ordering::Greater => j.passes.0 += 1,
            std::cmp::Ordering::Less => j.passes.1 += 1,
            std::cmp::Ordering::Equal => {}
        }
        j.fallen = (j.balance.0 == 0, j.balance.1 == 0);
        j.hits = Some((mine, theirs));
        let taken = (
            show::weight_of(theirs, j.fallen.0),
            show::weight_of(mine, j.fallen.1),
        );
        let aims = (j.aim, j.foe.as_ref().map_or(j.lance, |f| f.aim));
        let at = j.show.t;
        let x = j.meeting_x();
        j.show.meet(show::Meet {
            born: at,
            course: j.course,
            hits: (mine, theirs),
            aims,
            taken,
            steady,
            down: j.fallen,
            x,
        });
        if mine != Hit::Miss || theirs != Hit::Miss {
            self.hold_for(taken.0.max(taken.1));
        }
        if mine != Hit::Miss || theirs != Hit::Miss {
            let big = mine == Hit::Smite || theirs == Hit::Smite || j.fallen.0 || j.fallen.1;
            self.fx.push(Fx::Shake(if big { 12 } else { 6 }));
            self.sounds.push("knight_hit_big");
        }
        if j.fallen.0 || j.fallen.1 {
            self.sounds.push("knight_fall");
            self.sounds.push("helmet_pop");
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

    fn end_pass(&mut self, j: &mut Joust) {
        j.roar = (0, 0);
        for w in j.gallery.values_mut() {
            w.threw = false;
        }
        let decided = j.fallen.0 || j.fallen.1 || j.passes.0 >= 2 || j.passes.1 >= 2;
        if !decided && j.course < 2 {
            j.course += 1;
            j.stage = Stage::Ready;
            j.t = 0;
            j.guard = guard_for(self.seed, j.bout, j.course);
            j.lance = lance_for(self.seed, j.bout, j.course);
            if let Some(f) = &j.foe {
                j.guard = f.shield;
            }
            j.hits = None;
            return;
        }
        use std::cmp::Ordering::{Greater, Less};
        j.verdict = Some(match (j.fallen, j.passes.0.cmp(&j.passes.1), j.score.0.cmp(&j.score.1)) {
            ((false, true), _, _) => Verdict::Won,
            ((true, false), _, _) => Verdict::Lost,
            ((true, true), _, _) => Verdict::Drawn,
            (_, Greater, _) | (_, _, Greater) => Verdict::Won,
            (_, Less, _) | (_, _, Less) => Verdict::Lost,
            _ => Verdict::Drawn,
        });
        j.stage = Stage::Done;
        j.t = 0;
        self.close_bout(j);
    }

    fn close_bout(&mut self, j: &mut Joust) {
        let verdict = match j.verdict {
            Some(Verdict::Won) => "won",
            Some(Verdict::Lost) => "lost",
            _ => "drawn",
        };
        let winner = j.winner();
        let champion = j.foe.is_none();
        if champion {
            let rival = j.rival().id;
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
            *self.marks.entry("lists:bout".into()).or_default() += 1;
            if winner.is_some() {
                self.notice("laurel_of_the_lists");
            }
            self.cues.push(if winner.is_some() {
                "joust_won:friend".into()
            } else {
                "joust_drawn:friend".into()
            });
        }
        let mut owed = 0;
        if let Some(side) = winner {
            if j.purse > 0 {
                if let Some(id) = j.rider(side) {
                    self.bank_gold(id, PURSE - ENTRY, "won the lists");
                }
                if j.rider(1 - side).is_some() {
                    owed += ENTRY;
                }
            }
            if let Some(id) = j.rider(side)
                && let Some(hero) = self.players.get_mut(&id)
            {
                hero.laurel = true;
                hero.booby = None;
            }
            if let Some(id) = j.rider(1 - side)
                && let Some(hero) = self.players.get_mut(&id)
            {
                hero.laurel = false;
                hero.booby = Some(super::pastimes::Game::Hobbyhorse);
            }
        }
        let mut paid = Vec::new();
        for (&id, w) in j.gallery.iter_mut() {
            let (Some(side), Some(win)) = (w.side, winner) else {
                continue;
            };
            if w.stake == 0 {
                continue;
            }
            let won = side == win;
            w.won = Some(won);
            if won {
                paid.push((id, w.stake));
            } else {
                owed += w.stake;
            }
        }
        for (id, gold) in paid {
            self.bank_gold(id, gold, "a wager won at the lists");
        }
        if owed > 0 {
            *self.marks.entry(PAID_MARK.into()).or_default() += owed;
        }
        if winner.is_some() {
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
    }

    fn bank_gold(&mut self, id: u32, gold: u32, why: &str) {
        let mut spoils = Spoils::default();
        spoils.add(Spoil::Gold, gold);
        self.bank.push(Haul {
            hero: id,
            spoils,
            why: why.to_string(),
        });
    }

    pub(super) fn depart_lists(&mut self, id: u32) {
        let Some(mut j) = self.joust.take() else {
            return;
        };
        if j.gallery.remove(&id).is_some() {
            self.joust = Some(j);
            return;
        }
        let side = if j.knight == id {
            0
        } else if j.foe.as_ref().is_some_and(|f| f.id == id) {
            1
        } else {
            self.joust = Some(j);
            return;
        };
        let ridden = j.course > 0 || j.stage != Stage::Ready;
        if !ridden && let Some(f) = j.foe.take() {
            if side == 0 {
                j.knight = f.id;
                j.mount = f.mount;
                j.tended = f.tended;
                j.balance.0 = Joust::balance_on(f.mount, f.tended);
                j.keys = f.keys;
                j.spurred = false;
            }
            j.balance.1 = j.rival().balance;
            j.guard = guard_for(self.seed, j.bout, 0);
            j.lance = lance_for(self.seed, j.bout, 0);
            if j.purse > 0 && self.lists_free_gold() < j.exposure() {
                j.purse = 0;
            }
            self.joust = Some(j);
            self.seat_riders();
            return;
        }
        if j.stage != Stage::Done && ridden {
            j.verdict = Some(if side == 0 {
                Verdict::Lost
            } else {
                Verdict::Won
            });
            j.stage = Stage::Done;
            j.t = 0;
            self.close_bout(&mut j);
        }
        let stays = self.players.get(&id).map(|h| (h.x, h.y));
        self.get_down(&j);
        if let (Some((x, y)), Some(hero)) = (stays, self.players.get_mut(&id)) {
            (hero.x, hero.y) = (x, y);
        }
    }

    fn get_down(&mut self, j: &Joust) {
        let (c, r, w, h) = MOUNT_PLATE;
        let foot = (
            (c as f32 + w as f32 / 2.0) * TILE_UNITS,
            (r as f32 + h as f32 + 0.6) * TILE_UNITS,
        );
        let mut downs = vec![(j.knight, foot)];
        if let Some(f) = &j.foe {
            downs.push((f.id, (foot.0 + 2.0 * TILE_UNITS, foot.1)));
        }
        downs.extend(j.gallery.iter().map(|(&id, w)| (id, w.from)));
        for (id, (x, y)) in downs {
            if let Some(hero) = self.players.get_mut(&id) {
                (hero.x, hero.y) = (x, y);
                hero.buy_spent = true;
                hero.buying = 0;
            }
        }
    }
}

pub fn settle_lists(
    treasury: &mut crate::realm::Spoils,
    marks: &BTreeMap<String, u32>,
) -> Vec<(String, String)> {
    let owed = marks.get(PAID_MARK).copied().unwrap_or(0);
    let paid = owed.min(treasury.get(Spoil::Gold));
    if paid == 0 {
        return Vec::new();
    }
    let mut cost = crate::realm::Spoils::default();
    cost.add(Spoil::Gold, paid);
    treasury.take(&cost);
    vec![(
        "joust_result:lost".into(),
        format!("The lists took {paid} gold: entries and wagers lost"),
    )]
}

impl Run {
    pub(super) fn tick_stables(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut asked: Vec<String> = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let pressing = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.interact);
            if !pressing {
                hero.buy_spent = false;
            }
            let plate = STALL_PLATES
                .iter()
                .position(|&p| inside(p, hero.x, hero.y))
                .map(|i| format!("stable:select:{}", Mount::ALL[i].name().to_lowercase()))
                .or_else(|| inside(TEND_PLATE, hero.x, hero.y).then(|| "stable:tend".into()))
                .or_else(|| {
                    MARKET_PLATES
                        .iter()
                        .position(|&p| inside(p, hero.x, hero.y))
                        .filter(|&i| market_open(&self.home.stable, self.treasury.get(Spoil::Gold), &self.marks, i).is_ok())
                        .map(market_mark)
                });
            match plate {
                Some(what) if hero.hp > 0 && !hero.stone && pressing && !hero.buy_spent => {
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

pub fn settle_stable(
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

pub const MARKET_PLATES: [(i32, i32, i32, i32); 4] =
    [(2, 12, 2, 1), (5, 12, 2, 1), (8, 12, 2, 1), (15, 12, 2, 1)];

pub fn on_market(run: &Run) -> bool {
    run.room().kind == RoomKind::Stables
        && run.players.values().any(|h| h.hp > 0 && MARKET_PLATES.iter().any(|&p| inside(p, h.x, h.y)))
}

pub fn market_mark(i: usize) -> String {
    format!("stable:buy:{i}")
}

pub fn market_open(
    stable: &Stable,
    gold: u32,
    marks: &BTreeMap<String, u32>,
    i: usize,
) -> Result<u32, &'static str> {
    let market = stable.todays_market();
    let horse = market.get(i).ok_or("NONE")?;
    if marks.contains_key(&market_mark(i)) {
        return Err("SOLD");
    }
    let promised: Vec<usize> = (0..market.len()).filter(|&k| marks.contains_key(&market_mark(k))).collect();
    if stable.herd.len() + promised.len() >= 12 {
        return Err("YARD FULL");
    }
    let owed: u32 = promised.iter().map(|&k| market[k].value(stable)).sum();
    let price = horse.value(stable);
    if gold.saturating_sub(owed) < price {
        return Err("PURSE SHORT");
    }
    Ok(price)
}

pub fn settle_market(
    stable: &mut Stable,
    treasury: &mut crate::realm::Spoils,
    marks: &BTreeMap<String, u32>,
) -> Vec<(String, String)> {
    let market = stable.todays_market();
    let mut said = Vec::new();
    for (i, horse) in market.into_iter().enumerate() {
        if !marks.contains_key(&market_mark(i)) {
            continue;
        }
        let name = horse.name.clone();
        match stable.buy(horse, treasury.get(Spoil::Gold)) {
            Some(price) => {
                let mut cost = crate::realm::Spoils::default();
                cost.add(Spoil::Gold, price);
                treasury.take(&cost);
                said.push(("stable_buy".into(), format!("{name} joins the herd for {price} gold")));
            }
            None => said.push(("cant_afford".into(), format!("{name} stays at market: the purse or the yard"))),
        }
    }
    said
}

pub const STALL_PLATES: [(i32, i32, i32, i32); 3] =
    [(5, 4, 2, 1), (11, 4, 2, 1), (17, 4, 2, 1)];
pub const TEND_PLATE: (i32, i32, i32, i32) = (3, 7, 2, 1);
pub const GROOM_AT: (f32, f32) = (6.6, 7.6);
pub const LED_AT: (f32, f32) = (9.6, 8.6);

pub fn reins(inputs: &BTreeMap<u32, Input>, rider: u32) -> BTreeMap<u32, Input> {
    reins_all(inputs, &[rider])
}

pub fn reins_all(inputs: &BTreeMap<u32, Input>, seated: &[u32]) -> BTreeMap<u32, Input> {
    let mut out = inputs.clone();
    for &id in seated {
        out.insert(id, Input::default());
    }
    out
}

#[cfg(test)]
#[path = "../../tests/rules/together_joust__tests.rs"]
mod tests;
