//! A knight's racer: one part in every slot (`Build`), and what that adds up
//! to (`Stats`). A build is plain data, saved by the knight's name
//! (`ledger`); every field has a default, so a save written before a slot
//! existed, or naming a part the catalogue has since dropped, still loads and
//! the gap is filled from the knight's starting racer.

use super::parts::{self, BASE, NEUTRAL_WEIGHT, Part, Slot};
use serde::{Deserialize, Serialize};

pub const KIT: [&str; 4] = ["Excalibur", "Bastion", "Wraith", "Crown"];

const KIT_PARTS: [[&str; 8]; 4] = [
    ["gryphon_six", "lancer_wedge", "hover_discs", "triple_organ", "bare_crest", "bare_plate", "black_iron", "blue_glow"],
    ["bastion_block", "bastion_wagon", "hover_discs", "twin_spire", "ram_horns", "bare_plate", "banner_red", "brass_trim"],
    ["wraith_needle", "wraith_skiff", "hover_discs", "low_gutter", "gale_fins", "bare_plate", "violet_dusk", "blue_glow"],
    ["crown_cathedral", "crown_throne", "hover_discs", "grand_organ", "crown_crest", "bare_plate", "sun_gold", "silver_trim"],
];

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    pub engine: String,
    pub chassis: String,
    pub running: String,
    pub exhaust: String,
    pub crest: String,
    pub armour: String,
    pub paint: String,
    pub trim: String,
}

impl Default for Build {
    fn default() -> Build {
        Build::kit(0)
    }
}

impl Build {
    pub fn kit(i: usize) -> Build {
        let p = KIT_PARTS[i % KIT_PARTS.len()];
        let s = |k: usize| p[k].to_string();
        Build { engine: s(0), chassis: s(1), running: s(2), exhaust: s(3), crest: s(4), armour: s(5), paint: s(6), trim: s(7) }
    }

    pub fn starting(name: &str) -> Build {
        let mut h: u32 = 0x811c_9dc5;
        for b in super::ledger::key(name).bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        Build::kit((h >> 30) as usize % KIT.len())
    }

    pub fn from_ids(ids: &[String; 8]) -> Build {
        Build { engine: ids[0].clone(), chassis: ids[1].clone(), running: ids[2].clone(), exhaust: ids[3].clone(), crest: ids[4].clone(), armour: ids[5].clone(), paint: ids[6].clone(), trim: ids[7].clone() }.sound()
    }

    pub fn ids(&self) -> [String; 8] {
        Slot::ALL.map(|s| self.id(s).to_string())
    }

    pub fn id(&self, slot: Slot) -> &str {
        match slot {
            Slot::Engine => &self.engine,
            Slot::Chassis => &self.chassis,
            Slot::Running => &self.running,
            Slot::Exhaust => &self.exhaust,
            Slot::Crest => &self.crest,
            Slot::Armour => &self.armour,
            Slot::Paint => &self.paint,
            Slot::Trim => &self.trim,
        }
    }

    fn id_mut(&mut self, slot: Slot) -> &mut String {
        match slot {
            Slot::Engine => &mut self.engine,
            Slot::Chassis => &mut self.chassis,
            Slot::Running => &mut self.running,
            Slot::Exhaust => &mut self.exhaust,
            Slot::Crest => &mut self.crest,
            Slot::Armour => &mut self.armour,
            Slot::Paint => &mut self.paint,
            Slot::Trim => &mut self.trim,
        }
    }

    pub fn part(&self, slot: Slot) -> &'static Part {
        parts::part(self.id(slot))
            .filter(|p| p.slot == slot)
            .or_else(|| parts::part(Build::kit(0).id(slot)))
            .expect("the default racer wears a part in every slot")
    }

    pub fn worn(&self) -> [&'static Part; 8] {
        Slot::ALL.map(|s| self.part(s))
    }

    pub fn is_sound(&self) -> bool {
        Slot::ALL.iter().all(|&s| parts::part(self.id(s)).is_some_and(|p| p.slot == s))
    }

    pub fn sound(mut self) -> Build {
        let kit = Build::kit(0);
        for s in Slot::ALL {
            if parts::part(self.id(s)).is_none_or(|p| p.slot != s) {
                *self.id_mut(s) = kit.id(s).to_string();
            }
        }
        self
    }

    pub fn fit(&mut self, part_id: &str) -> bool {
        let Some(p) = parts::part(part_id) else { return false };
        if self.id(p.slot) == p.id {
            return false;
        }
        *self.id_mut(p.slot) = p.id.to_string();
        true
    }

    pub fn stats(&self) -> Stats {
        Stats::of(self)
    }

    pub fn differs(&self, other: &Build) -> Vec<Slot> {
        Slot::ALL.into_iter().filter(|&s| self.id(s) != other.id(s)).collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub speed: i32,
    pub accel: i32,
    pub handling: i32,
    pub armour: i32,
    pub weight: i32,
}

impl Stats {
    pub fn of(b: &Build) -> Stats {
        let mut raw = BASE;
        for p in b.worn() {
            raw.speed += p.fx.speed;
            raw.accel += p.fx.accel;
            raw.handling += p.fx.handling;
            raw.armour += p.fx.armour;
            raw.weight += p.fx.weight;
        }
        let weight = i32::from(raw.weight);
        let over = weight - NEUTRAL_WEIGHT;
        let held = |v: i32| v.clamp(0, 100);
        Stats {
            speed: held(i32::from(raw.speed) - over / 5),
            accel: held(i32::from(raw.accel) - over / 2),
            handling: held(i32::from(raw.handling) - over / 3),
            armour: held(i32::from(raw.armour)),
            weight: weight.clamp(0, 100),
        }
    }

    pub fn bars(&self) -> [(&'static str, i32); 5] {
        [("SPEED", self.speed), ("ACCEL", self.accel), ("HANDLING", self.handling), ("ARMOUR", self.armour), ("WEIGHT", self.weight)]
    }

    pub fn delta(&self, other: &Stats) -> Stats {
        Stats {
            speed: self.speed - other.speed,
            accel: self.accel - other.accel,
            handling: self.handling - other.handling,
            armour: self.armour - other.armour,
            weight: self.weight - other.weight,
        }
    }

    pub fn is_zero(&self) -> bool {
        *self == Stats::default()
    }

    pub fn unit(v: i32) -> f32 {
        v.clamp(0, 100) as f32 / 100.0
    }
}
