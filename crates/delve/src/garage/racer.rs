//! A knight's racer: one part in every slot (`Build`), and what that adds up
//! to (`Stats`). A build is plain data, saved by the knight's name
//! (`ledger`); every field has a default, so a save written before a slot
//! existed, or naming a part the catalogue has since dropped, still loads and
//! the gap is filled from the knight's starting racer.

use super::parts::{self, BASE, NEUTRAL_WEIGHT, Part, Slot};
use serde::{Deserialize, Serialize};

/// The four racers of the knight racers example, in its order.
pub const KIT: [&str; 4] = ["Excalibur", "Bastion", "Wraith", "Crown"];

/// The parts of each starting racer, in `Slot::ALL` order.
const KIT_PARTS: [[&str; 8]; 4] = [
    ["gryphon_six", "lancer_wedge", "hover_discs", "triple_organ", "bare_crest", "bare_plate", "black_iron", "blue_glow"],
    ["bastion_block", "bastion_wagon", "hover_discs", "twin_spire", "ram_horns", "bare_plate", "banner_red", "brass_trim"],
    ["wraith_needle", "wraith_skiff", "hover_discs", "low_gutter", "gale_fins", "bare_plate", "violet_dusk", "blue_glow"],
    ["crown_cathedral", "crown_throne", "hover_discs", "grand_organ", "crown_crest", "bare_plate", "sun_gold", "silver_trim"],
];

/// One part id in every slot. The ids are the catalogue's (`parts`); a field
/// that is missing or names nothing the catalogue knows is read as the
/// default racer's part (`sound`).
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
    /// Excalibur, the first of the starting kit.
    fn default() -> Build {
        Build::kit(0)
    }
}

impl Build {
    /// A starting racer: 0 Excalibur, 1 Bastion, 2 Wraith, 3 Crown.
    pub fn kit(i: usize) -> Build {
        let p = KIT_PARTS[i % KIT_PARTS.len()];
        let s = |k: usize| p[k].to_string();
        Build { engine: s(0), chassis: s(1), running: s(2), exhaust: s(3), crest: s(4), armour: s(5), paint: s(6), trim: s(7) }
    }

    /// Which starting racer a knight with this name begins with: the same one
    /// on every host and every game, and mostly a different one for each of
    /// the knights in a room.
    pub fn starting(name: &str) -> Build {
        let mut h: u32 = 0x811c_9dc5;
        for b in super::ledger::key(name).bytes() {
            h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        Build::kit((h >> 30) as usize % KIT.len())
    }

    /// A build from one id per slot, in `Slot::ALL` order (the race's career keeps it so), made
    /// sound.
    pub fn from_ids(ids: &[String; 8]) -> Build {
        Build { engine: ids[0].clone(), chassis: ids[1].clone(), running: ids[2].clone(), exhaust: ids[3].clone(), crest: ids[4].clone(), armour: ids[5].clone(), paint: ids[6].clone(), trim: ids[7].clone() }.sound()
    }

    /// One id per slot, in `Slot::ALL` order.
    pub fn ids(&self) -> [String; 8] {
        Slot::ALL.map(|s| self.id(s).to_string())
    }

    /// The id in a slot.
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

    /// The part worn in a slot. A build that has been made `sound` always has
    /// one; for any other the default racer's part stands in.
    pub fn part(&self, slot: Slot) -> &'static Part {
        parts::part(self.id(slot))
            .filter(|p| p.slot == slot)
            .or_else(|| parts::part(Build::kit(0).id(slot)))
            .expect("the default racer wears a part in every slot")
    }

    /// Every part worn, in `Slot::ALL` order.
    pub fn worn(&self) -> [&'static Part; 8] {
        Slot::ALL.map(|s| self.part(s))
    }

    /// Whether every slot holds a part of the catalogue that fits it.
    pub fn is_sound(&self) -> bool {
        Slot::ALL.iter().all(|&s| parts::part(self.id(s)).is_some_and(|p| p.slot == s))
    }

    /// This build with every slot that holds nothing the catalogue fits put
    /// back to the default racer's part.
    pub fn sound(mut self) -> Build {
        let kit = Build::kit(0);
        for s in Slot::ALL {
            if parts::part(self.id(s)).is_none_or(|p| p.slot != s) {
                *self.id_mut(s) = kit.id(s).to_string();
            }
        }
        self
    }

    /// Put `part` in its slot. False (and nothing changes) when it is
    /// already there, or is not in the catalogue.
    pub fn fit(&mut self, part_id: &str) -> bool {
        let Some(p) = parts::part(part_id) else { return false };
        if self.id(p.slot) == p.id {
            return false;
        }
        *self.id_mut(p.slot) = p.id.to_string();
        true
    }

    /// What this build adds up to.
    pub fn stats(&self) -> Stats {
        Stats::of(self)
    }

    /// Which slots differ from another build.
    pub fn differs(&self, other: &Build) -> Vec<Slot> {
        Slot::ALL.into_iter().filter(|&s| self.id(s) != other.id(s)).collect()
    }
}

/// A racer's numbers, whole and on a scale of 0 to 100 (`weight` is the load
/// it carries). The race reads these; the garage shows them as bars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    /// How fast it can go flat out.
    pub speed: i32,
    /// How quickly it gets there.
    pub accel: i32,
    /// How well it takes a corner.
    pub handling: i32,
    /// How much a hit costs it.
    pub armour: i32,
    /// What it all weighs. Heavy racers pay for it in speed, acceleration and
    /// handling: this is what armour is bought against.
    pub weight: i32,
}

impl Stats {
    /// A build's numbers: the bare racer's, plus every part's, with the
    /// weight above `NEUTRAL_WEIGHT` taken back off speed (a fifth of it),
    /// acceleration (a half) and handling (a third), and a racer lighter than
    /// neutral given the same back. Each is held to 0..=100.
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

    /// The five numbers in the order the panel shows them.
    pub fn bars(&self) -> [(&'static str, i32); 5] {
        [("SPEED", self.speed), ("ACCEL", self.accel), ("HANDLING", self.handling), ("ARMOUR", self.armour), ("WEIGHT", self.weight)]
    }

    /// `self` minus `other`, number by number: what a swap changes.
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

    /// The race's reading of a stat: 0.0 to 1.0.
    pub fn unit(v: i32) -> f32 {
        v.clamp(0, 100) as f32 / 100.0
    }
}
