//! The things that stand on the rail: bonus gates and hearth rings to fly
//! through, an arch to thread, a ridge to clear, shutters that close on a
//! clock. Each is judged by the span of rail a craft crossed this tick and
//! where it was across and up when it crossed.
use super::flight::CRAFT_R;
use super::*;

/// A shutter's cycle, in ticks, and the narrowest its gap closes to (half).
pub const SHUTTER_PERIOD: u32 = 150;
pub const SHUTTER_MIN: i32 = dm(5);
/// What a scrape of rock takes off a shield.
const SCRAPE: i32 = 15;
/// What a hearth ring mends.
const MEND: i32 = 50;
/// A bonus gate's points, doubled for a gate flown on the spur.
pub const GATE: u32 = 100;

/// A shutter's half gap at tick `t`: open and closing and opening again over
/// its cycle, the way it shows from the far end of the rail.
pub fn shutter_gap(p: &Prop, t: u32, wx: i32) -> i32 {
    let k = tri(t + u32::from(p.phase), SHUTTER_PERIOD, 1000) + 1000;
    SHUTTER_MIN + (wx + dm(10) - SHUTTER_MIN) * k / 2000
}

impl Session {
    /// Props and the knights who cross them.
    pub(super) fn step_props(&mut self, n: &Nums) {
        let anchor = self.anchor();
        let t = self.t;
        let wx = n.wx();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let (s1, ds, x, z, flying, boosting) = {
                let c = &self.seats[&id].craft;
                (self.craft_s(c), c.ds, c.x, c.z, c.flying(), c.boost)
            };
            if !flying {
                continue;
            }
            let s0 = s1 - ds;
            for pi in 0..self.props.len() {
                let p = self.props[pi].clone();
                let over = s0 < p.s + p.len && s1 > p.s;
                match p.thing {
                    Thing::Gate | Thing::Ring => {
                        let crossed = s0 < p.s && p.s <= s1;
                        if crossed
                            && p.taken & bit(id) == 0
                            && flat(x, z, p.x, p.z) <= i64::from(p.w + CRAFT_R)
                        {
                            self.props[pi].taken |= bit(id);
                            self.take(id, &p, boosting);
                        }
                    }
                    Thing::Arch => {
                        let inside =
                            (x - p.x).abs() + CRAFT_R <= p.w && (z - p.z).abs() + CRAFT_R <= p.h;
                        if over && !inside {
                            self.scrape(id, s1, x, z);
                        }
                    }
                    Thing::Ridge => {
                        let hits =
                            (x - p.x).abs() < p.w + CRAFT_R && (z - p.z).abs() < p.h + CRAFT_R;
                        if over && hits {
                            self.scrape(id, s1, x, z);
                        }
                    }
                    Thing::Shutter => {
                        let gap = shutter_gap(&p, t, wx);
                        if over && (x - p.x).abs() > gap - CRAFT_R {
                            self.scrape(id, s1, x, z);
                        }
                    }
                }
            }
        }
        self.props.retain(|p| p.s + p.len > anchor - dm(300));
    }

    /// A gate or a ring flown through.
    fn take(&mut self, id: u32, p: &Prop, boosting: bool) {
        match p.thing {
            Thing::Gate => {
                // A victory gate (`phase` 1) is a welcome, not a score.
                if p.phase == 0 {
                    let pts = GATE * if boosting { 2 } else { 1 };
                    if let Some(s) = self.seats.get_mut(&id) {
                        s.craft.pts += pts;
                        s.craft.gates += 1;
                    }
                    self.beat.pts += pts.min(GATE);
                    self.beat.base += GATE;
                    self.beat.gates += 1;
                }
                self.sound("gold");
            }
            _ => {
                if let Some(s) = self.seats.get_mut(&id) {
                    s.craft.shield = (s.craft.shield + MEND).min(SHIELD);
                }
                self.sound("shield_up");
            }
        }
        self.spark(Burst::Gate, p.s, p.x, p.z, id as u8);
    }

    /// Rock met: a blow, and the craft is spared a moment.
    fn scrape(&mut self, id: u32, s: i32, x: i32, z: i32) {
        if self.hurt_craft(id as u8, SCRAPE) {
            self.spark(Burst::Scrape, s, x, z, id as u8);
            self.sound("rock_land");
        }
    }
}
