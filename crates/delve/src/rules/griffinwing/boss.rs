//! The Gloam Wyrm: three lamps in cold-light shields, three phases. A turned
//! bolt (a roll that reflects one of its spits) opens the lit lamp's shield;
//! a charged lock breaks the lamp open; its flame goes out and the Wyrm
//! reels into the next phase. Every attack shows its danger footprint locked
//! for the tell's ticks (never under forty) before it lands, so the party
//! has time to act: the rollers reflect, the chargers lock.
use super::flight::CRAFT_R;
use super::*;

/// How far ahead of the formation the Wyrm's head holds.
pub const HEAD: i32 = dm(340);
/// How long a lamp's shield stays open after a turned bolt, in ticks.
pub const OPEN: u32 = 150;
const RISE_T: u32 = 90;
const HURT_T: u32 = 70;
/// The lamps' places about the head, across and up.
const LAMP: [(i32, i32); 3] = [(dm(-45), dm(10)), (dm(45), dm(10)), (0, dm(42))];
/// How far in front of its face the lamps hang.
const FRONT: i32 = dm(40);
/// The head's height above the rail's line, resting.
const HIGH: i32 = dm(12);
/// How wide the coils' gap and the roar's column are, half.
pub const GAP_HALF: i32 = dm(30);
pub const COL_HALF: i32 = dm(24);
const COIL_T: u32 = 22;
const ROAR_T: u32 = 26;
/// What a coil or the roar does to a craft it catches.
const BLOW: i32 = 35;

impl Session {
    /// The Wyrm rises out of the roost (or, after a fall, is back in its
    /// place, its lamps as they were).
    pub(super) fn rise(&mut self) {
        self.cp = rail::BOSS_AT;
        if let Some(b) = &mut self.boss {
            b.st = Wyrm::Rise;
            b.ticks = RISE_T;
            b.marks.clear();
            return;
        }
        let anchor = self.anchor();
        let mut lamps = [0u16; 3];
        for (k, &(dx, dz)) in LAMP.iter().enumerate() {
            let id = self.spawn_foe(
                Kind::Lamp,
                (anchor + HEAD - FRONT, dx, HIGH + dz),
                (0, 0, 0),
                St::Shut,
                k as u8,
                0,
            );
            lamps[k] = id.unwrap_or(0);
        }
        self.boss = Some(Beast {
            phase: 0,
            st: Wyrm::Rise,
            ticks: RISE_T,
            x: 0,
            z: HIGH,
            lamps,
            marks: Vec::new(),
            gap: 0,
            side: 1,
            col: 0,
            atk: 0,
            seq: 0,
        });
        let who = self.voice();
        self.say(who, Line::Wyrm);
        self.sound("boss_rise");
    }

    /// How long the Wyrm rests between attacks: shorter the more knights
    /// fly and the later the phase.
    fn rest(&self, phase: u8) -> u32 {
        let seats = u32::from(self.pressure());
        (64 - 4 * (seats - 1) - 8 * u32::from(phase)).max(34)
    }

    /// The Wyrm's tick.
    pub(super) fn step_boss(&mut self, n: &Nums) {
        let Some(mut b) = self.boss.take() else {
            return;
        };
        let anchor = self.anchor();
        let t = self.t;
        // The head holds its place ahead and sways; it rises from below.
        b.x = tri(t, 260, dm(24));
        b.z = HIGH
            - if b.st == Wyrm::Rise {
                b.ticks as i32 * dm(1) / 2
            } else {
                0
            };
        for (k, id) in b.lamps.iter().enumerate() {
            if let Some(f) = self.foes.iter_mut().find(|f| f.id == *id) {
                f.s = anchor + HEAD - FRONT;
                f.x = b.x + LAMP[k].0;
                f.z = b.z + LAMP[k].1;
                if f.st == St::Open {
                    f.ticks = f.ticks.saturating_sub(1);
                    if f.ticks == 0 {
                        f.st = St::Shut;
                    }
                }
            }
        }
        let lit = b.lamps[usize::from(b.phase.min(2))];
        let out = self
            .foes
            .iter()
            .find(|f| f.id == lit)
            .is_some_and(|f| f.st == St::Broken);
        if out && !matches!(b.st, Wyrm::Hurt | Wyrm::Dying) {
            self.bolts.retain(|x| x.owner != 0);
            b.marks.clear();
            self.sound("boss_roar");
            if b.phase >= 2 {
                b.st = Wyrm::Dying;
                b.ticks = DYING;
            } else {
                b.st = Wyrm::Hurt;
                b.ticks = HURT_T;
            }
            self.boss = Some(b);
            return;
        }
        let pat: &[u8] = match b.phase {
            0 => &[0],
            1 => &[1, 0],
            _ => &[2, 0, 1, 0],
        };
        match b.st {
            Wyrm::Rise => {
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks == 0 {
                    b.st = Wyrm::Idle;
                    b.ticks = 50;
                    self.say(self.voice(), Line::Reflect);
                }
            }
            Wyrm::Idle => {
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks == 0 {
                    b.atk = pat[usize::from(b.seq) % pat.len()];
                    b.seq = b.seq.wrapping_add(1);
                    self.begin_tell(&mut b, n);
                }
            }
            Wyrm::Tell => {
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks == 0 {
                    match b.atk {
                        0 => {
                            self.spit(&b);
                            b.st = Wyrm::Idle;
                            b.ticks = self.rest(b.phase);
                            b.marks.clear();
                        }
                        1 => {
                            b.st = Wyrm::Coil;
                            b.ticks = COIL_T;
                            self.sound("boss_rage");
                        }
                        _ => {
                            b.st = Wyrm::Roar;
                            b.ticks = ROAR_T;
                            self.sound("boss_roar");
                        }
                    }
                }
            }
            Wyrm::Coil | Wyrm::Roar => {
                let (gap, col, coil) = (b.gap, b.col, b.st == Wyrm::Coil);
                let ids: Vec<u32> = self
                    .seats
                    .iter()
                    .filter(|(_, s)| s.craft.flying())
                    .map(|(&id, _)| id)
                    .collect();
                for id in ids {
                    let x = self.seats[&id].craft.x;
                    let caught = if coil {
                        (x - gap).abs() > GAP_HALF - CRAFT_R / 2
                    } else {
                        (x - col).abs() <= COL_HALF + CRAFT_R / 2
                    };
                    if caught {
                        self.hurt_craft(id as u8, BLOW);
                    }
                }
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks == 0 {
                    b.st = Wyrm::Idle;
                    b.ticks = self.rest(b.phase);
                    b.marks.clear();
                }
            }
            Wyrm::Hurt => {
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks == 0 {
                    b.phase += 1;
                    b.st = Wyrm::Idle;
                    b.ticks = self.rest(b.phase) + 20;
                    b.seq = 0;
                    let who = self.voice();
                    self.say(
                        who,
                        if b.phase == 1 {
                            Line::Coils
                        } else {
                            Line::Roar
                        },
                    );
                    // The Wyrm calls kites to its side: they hunt the wing.
                    let hunters = 1 + i32::from(self.pressure()) / 3;
                    for k in 0..hunters {
                        let side = if k % 2 == 0 { -1 } else { 1 };
                        self.act(script::Act::Hunter { side });
                    }
                }
            }
            Wyrm::Dying => {
                b.ticks = b.ticks.saturating_sub(1);
                if b.ticks % 12 == 0 {
                    self.spark(Burst::Pop, anchor + HEAD, b.x, b.z, 0);
                }
            }
        }
        self.boss = Some(b);
    }

    /// An attack's footprints are locked and shown for the tell's ticks.
    fn begin_tell(&mut self, b: &mut Beast, n: &Nums) {
        let seats = usize::from(self.pressure());
        let phase = usize::from(b.phase);
        let turn = usize::from(b.seq);
        if self.target(0).is_none() {
            // Nobody to aim at: it waits.
            b.st = Wyrm::Idle;
            b.ticks = 30;
            return;
        }
        b.marks.clear();
        match b.atk {
            0 => {
                let count = (2 + (seats - 1) / 2 + phase).min(7);
                for k in 0..count {
                    if let Some((x, z)) = self.target(turn + k) {
                        // A knight asked for twice is aimed at to either side.
                        let jitter = (k as i32 / self.seats.len().max(1) as i32) * dm(16);
                        b.marks
                            .push((x + if k % 2 == 0 { jitter } else { -jitter }, z));
                    }
                }
                if b.phase == 0 && b.seq == 1 {
                    self.say(self.voice(), Line::Reflect);
                }
            }
            1 => {
                b.side = if b.seq % 2 == 0 { -1 } else { 1 };
                let gap = self.target(turn).map_or(0, |t| t.0);
                b.gap = gap.clamp(-n.wx() + GAP_HALF / 2, n.wx() - GAP_HALF / 2);
                b.marks.push((b.gap, 0));
            }
            _ => {
                b.col = self.target(turn + 1).map_or(0, |t| t.0);
                b.marks.push((b.col, 0));
            }
        }
        b.st = Wyrm::Tell;
        b.ticks = n.tell.max(40);
        self.sound("vent_fire");
    }

    /// The Wyrm's spit: a Gloam bolt for each locked mark, from its mouth,
    /// each owing its return to the lit lamp.
    fn spit(&mut self, b: &Beast) {
        let lit = b.lamps[usize::from(b.phase.min(2))];
        for &to in &b.marks {
            let from = (self.anchor() + HEAD, b.x, b.z);
            let v = self.plane_shot(from, to, dm(5));
            self.push_bolt(Bolt {
                shot: Arm::Gloam,
                owner: 0,
                s: from.0,
                x: from.1,
                z: from.2,
                vs: v.0,
                vx: v.1,
                vz: v.2,
                dmg: 25,
                life: 300,
                aim: 0,
                src: lit,
                delay: 0,
                full: false,
            });
        }
        self.sound("shot_crossbow");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tells_footprints_are_locked_for_at_least_forty_ticks_whatever_the_knob() {
        let mut s = Session::new(&BTreeMap::new());
        s.seats.insert(
            1,
            Seat {
                joined: 0,
                craft: Craft::new(0, 0),
            },
        );
        s.stage = 2;
        s.t = rail::BOSS_AT;
        s.rise();
        let n = Nums {
            tell: 5,
            ..Nums::default()
        };
        let mut b = s.boss.take().unwrap();
        b.atk = 0;
        s.begin_tell(&mut b, &n);
        assert_eq!(b.st, Wyrm::Tell);
        assert!(b.ticks >= 40 && !b.marks.is_empty());
    }
}
