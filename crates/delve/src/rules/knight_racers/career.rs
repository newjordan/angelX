//! A knight's career at the races: the money they have won, the pods and classes
//! they have earned, the parts they have bought and fitted, and their records.
//!
//! This is plain data with fixed numbers (no formulas): the purses, the prices, the
//! unlocks. It lives with the rules because the rules' tick pays purses and sells
//! parts; the garage's ledger (`garage::ledger`) keeps it between games, by name,
//! and the garage's `bank` carries it between the two.
//!
//! The garage's parts catalogue (`garage::parts`) is the one place a part's numbers
//! are written down for the garage; `WARES` below repeats the gameplay slots' numbers
//! (the rules cannot reach the garage, which is drawn) and a garage test fails if the
//! two ever differ.
use super::flow::{Class, Mode};
use super::tune::Tune;
use super::CourseId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const START_MONEY: u32 = 400;
pub const START_PODS: u8 = 0b10_0001;
pub const SWEEP_POD: u8 = 4;

pub const SLOTS: [&str; 8] = ["engine", "chassis", "running", "exhaust", "crest", "armour", "paint", "trim"];
pub const WORKING_SLOTS: usize = 6;
pub const SLOT_NAMES: [&str; 8] = ["ENGINES", "CHARIOTS", "RUNNING GEAR", "EXHAUST", "CRESTS", "ARMOUR", "PAINT", "TRIM"];

const BASE: [i32; 5] = [30, 30, 30, 10, 30];
const NEUTRAL_WEIGHT: i32 = 50;

#[derive(Clone, Copy, Debug)]
pub struct Ware {
    pub id: &'static str,
    pub slot: usize,
    pub price: u32,
    pub fx: [i16; 5],
}

const fn w(id: &'static str, slot: usize, price: u32, fx: [i16; 5]) -> Ware { Ware { id, slot, price, fx } }

pub const WARES: [Ware; 29] = [
    w("gryphon_six", 0, 0, [16, 8, 4, 0, 12]),
    w("crown_cathedral", 0, 500, [12, 8, 0, 6, 16]),
    w("bastion_block", 0, 600, [6, 12, -2, 8, 18]),
    w("wraith_needle", 0, 1200, [22, 4, 6, -4, 6]),
    w("hellhound_twin", 0, 1500, [10, 20, -4, 2, 14]),
    w("black_sun", 0, 1700, [4, 14, 14, 0, 10]),
    w("lancer_wedge", 1, 0, [2, 0, 6, 2, 8]),
    w("wraith_skiff", 1, 400, [4, 2, 10, -2, 4]),
    w("crown_throne", 1, 400, [0, 0, 2, 8, 12]),
    w("bastion_wagon", 1, 500, [-2, 0, -4, 12, 16]),
    w("hover_discs", 2, 0, [2, 0, 4, 0, 4]),
    w("war_wheels", 2, 600, [0, 6, 2, 2, 10]),
    w("iron_treads", 2, 700, [-2, 2, -2, 6, 16]),
    w("ice_runners", 2, 800, [6, 0, -4, 0, 2]),
    w("triple_organ", 3, 0, [3, 3, 0, 0, 4]),
    w("low_gutter", 3, 300, [4, 1, 2, 0, 3]),
    w("twin_spire", 3, 300, [1, 4, 0, 0, 3]),
    w("grand_organ", 3, 500, [2, 5, -1, 0, 6]),
    w("side_dragons", 3, 900, [0, 7, 2, 0, 5]),
    w("bare_crest", 4, 0, [0, 0, 0, 0, 0]),
    w("pennon_mast", 4, 150, [0, 0, 1, 0, 1]),
    w("ram_horns", 4, 250, [0, 0, -1, 3, 3]),
    w("gale_fins", 4, 400, [3, 0, 5, 0, 2]),
    w("crown_crest", 4, 700, [2, 0, 3, 2, 5]),
    w("bare_plate", 5, 0, [0, 0, 0, 0, 0]),
    w("studded_hide", 5, 500, [0, 0, 0, 6, 4]),
    w("scale_mail", 5, 1100, [-1, -1, 0, 12, 10]),
    w("spiked_barding", 5, 1800, [-2, -2, -4, 16, 16]),
    w("bastion_plate", 5, 2200, [-3, -3, -3, 20, 20]),
];

pub fn wares(slot: usize) -> Vec<&'static Ware> { WARES.iter().filter(|p| p.slot == slot).collect() }

pub fn ware(id: &str) -> Option<&'static Ware> { WARES.iter().find(|p| p.id == id) }

pub const EXCALIBUR: [&str; 8] = ["gryphon_six", "lancer_wedge", "hover_discs", "triple_organ", "bare_crest", "bare_plate", "black_iron", "blue_glow"];

pub fn stats_of(build: &[String; 8]) -> [i32; 5] {
    let mut raw = BASE;
    for slot in 0..WORKING_SLOTS {
        let part = ware(&build[slot]).filter(|p| p.slot == slot).or_else(|| ware(EXCALIBUR[slot])).expect("Excalibur wears every working slot");
        for (r, f) in raw.iter_mut().zip(part.fx) { *r += i32::from(f); }
    }
    let over = raw[4] - NEUTRAL_WEIGHT;
    let held = |v: i32| v.clamp(0, 100);
    [held(raw[0] - over / 5), held(raw[1] - over / 2), held(raw[2] - over / 3), held(raw[3]), raw[4].clamp(0, 100)]
}

pub fn stock() -> [i32; 5] { stats_of(&excalibur()) }

pub fn excalibur() -> [String; 8] { EXCALIBUR.map(String::from) }

pub fn tune_of(build: &[String; 8]) -> Tune {
    let (s, st) = (stats_of(build), stock());
    let d = |i: usize| (s[i] - st[i]) as f32;
    Tune {
        top: 1. + 0.003 * d(0),
        boost: 1. + 0.0025 * d(0),
        accel: 1. + 0.007 * d(1),
        grip: 1. + 0.005 * d(2),
        mass: 1. + 0.012 * d(3),
    }.held()
}

pub const PURSE: [[u32; 6]; 3] = [
    [900, 600, 400, 250, 150, 80],
    [1500, 1000, 650, 400, 240, 130],
    [2400, 1600, 1050, 650, 380, 200],
];
pub const CUP_PURSE: [[u32; 6]; 3] = [
    [3000, 1800, 1000, 500, 250, 100],
    [5000, 3000, 1700, 800, 400, 150],
    [8000, 4800, 2700, 1300, 600, 250],
];
pub const TRIAL_FIRST: u32 = 400;
pub const TRIAL_BEST: u32 = 200;

pub fn purse(mode: Mode, class: Class, place: usize) -> u32 {
    let row = &PURSE[class.rank()];
    let base = row[(place.max(1) - 1).min(5)];
    match mode {
        Mode::Cup => base,
        Mode::Single => base * 2 / 3,
        Mode::TimeTrial | Mode::Practice => 0,
    }
}

pub fn cup_purse(class: Class, place: usize) -> u32 { CUP_PURSE[class.rank()][(place.max(1) - 1).min(5)] }

pub fn cup_unlock(class: Class) -> (Option<Class>, Option<u8>) {
    match class {
        Class::Squire => (Some(Class::Knight), Some(2)),
        Class::Knight => (Some(Class::Champion), Some(1)),
        Class::Champion => (None, Some(3)),
    }
}

pub fn course_key(id: CourseId) -> String { format!("{id:?}").to_lowercase() }
pub fn lap_key(id: CourseId, pod: u8) -> String { format!("{}:{pod}", course_key(id)) }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CourseRecord {
    pub race: u32,
    pub lap: u32,
    pub lap_pod: u8,
    pub runs: u32,
    pub wins: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShopError {
    Unknown,
    Poor(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bought {
    pub price: u32,
    pub fitted: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notes {
    pub new_race: bool,
    pub new_lap: bool,
    pub first_lap: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Career {
    pub money: u32,
    pub earned: u64,
    pub pods: u8,
    pub classes: u8,
    pub owned: BTreeSet<String>,
    #[serde(skip)]
    pub build: [String; 8],
    pub cups: [u32; 3],
    pub sweeps: u32,
    pub races: u32,
    pub wins: u32,
    pub podiums: u32,
    pub courses: BTreeMap<String, CourseRecord>,
    pub laps: BTreeMap<String, u32>,
    pub trials: BTreeMap<String, u32>,
}

impl Default for Career {
    fn default() -> Career {
        Career {
            money: START_MONEY, earned: 0, pods: START_PODS, classes: 1, owned: BTreeSet::new(), build: excalibur(),
            cups: [0; 3], sweeps: 0, races: 0, wins: 0, podiums: 0,
            courses: BTreeMap::new(), laps: BTreeMap::new(), trials: BTreeMap::new(),
        }
    }
}

pub fn ms(t: f32) -> u32 { (t.max(0.) * 1000.).round() as u32 }

impl Career {
    pub fn with_build(build: [String; 8]) -> Career {
        let mut c = Career { build, ..Career::default() };
        c.own_what_is_worn();
        c
    }

    pub fn own_what_is_worn(&mut self) {
        for id in self.build.clone() { if ware(&id).is_some_and(|p| p.price > 0) { self.owned.insert(id); } }
    }

    pub fn owns_pod(&self, pod: u8) -> bool { pod < 8 && self.pods & (1 << pod) != 0 }
    pub fn class_open(&self, class: Class) -> bool { (class.rank() as u8) < self.classes.clamp(1, 3) }
    pub fn open_pods(&self) -> Vec<u8> { (0..8u8).filter(|&p| self.owns_pod(p)).collect() }

    pub fn owns(&self, id: &str) -> bool { ware(id).is_some_and(|p| p.price == 0) || self.owned.contains(id) }
    pub fn worn(&self, slot: usize) -> &str { &self.build[slot] }
    pub fn stats(&self) -> [i32; 5] { stats_of(&self.build) }
    pub fn tune(&self) -> Tune { tune_of(&self.build) }

    pub fn buy_and_fit(&mut self, id: &str) -> Result<Bought, ShopError> {
        let Some(part) = ware(id) else { return Err(ShopError::Unknown) };
        let price = if self.owns(id) { 0 } else { part.price };
        if price > self.money { return Err(ShopError::Poor(price - self.money)); }
        self.money -= price;
        if price > 0 { self.owned.insert(id.to_string()); }
        let fitted = self.build[part.slot] != id;
        self.build[part.slot] = id.to_string();
        Ok(Bought { price, fitted })
    }

    pub fn pay(&mut self, amount: u32) {
        self.money = self.money.saturating_add(amount);
        self.earned = self.earned.saturating_add(u64::from(amount));
    }

    pub fn unlock_pod(&mut self, pod: u8) -> bool {
        if pod >= 8 || self.owns_pod(pod) { return false; }
        self.pods |= 1 << pod;
        true
    }

    pub fn open_class(&mut self, class: Class) -> bool {
        let n = class.rank() as u8 + 1;
        if self.classes >= n { return false; }
        self.classes = n;
        true
    }

    pub fn note_race(&mut self, course: CourseId, pod: u8, place: usize, race: f32, lap: Option<f32>) -> Notes {
        self.races += 1;
        if place == 1 { self.wins += 1; }
        if place <= 3 { self.podiums += 1; }
        let rec = self.courses.entry(course_key(course)).or_default();
        rec.runs += 1;
        if place == 1 { rec.wins += 1; }
        let mut notes = Notes::default();
        let t = ms(race);
        if t > 0 && (rec.race == 0 || t < rec.race) { notes.new_race = rec.race > 0; rec.race = t; }
        if let Some(l) = lap.map(ms).filter(|&l| l > 0) {
            if rec.lap == 0 || l < rec.lap { rec.lap = l; rec.lap_pod = pod; }
            let slot = self.laps.entry(lap_key(course, pod)).or_insert(0);
            if *slot == 0 || l < *slot { notes.new_lap = *slot > 0; notes.first_lap = *slot == 0; *slot = l; }
        }
        notes
    }

    pub fn note_trial(&mut self, course: CourseId, pod: u8, lap: f32) -> bool {
        let l = ms(lap);
        let slot = self.trials.entry(lap_key(course, pod)).or_insert(0);
        if l > 0 && (*slot == 0 || l < *slot) { *slot = l; true } else { false }
    }
}
