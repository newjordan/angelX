//! Griffinwing's rules as they ran in Rust before they moved into Bend
//! (`cockpit/live/griffinwing-rules`): the oracle the module is held to.
//!
//! The code is the rules' own, as it stood in `crates/delve/src/rules/griffinwing`
//! (`flight`, `combat`, `script`, `boss`, `things`, `bot` and the tick of
//! `griffinwing.rs`), turned into one trait on `Session` so it can sit outside the
//! crate. Two things changed: who is at the table is the list of ids the frame
//! carries instead of the heroes themselves, and `step` and `bot_with` are
//! `step_here` and `bot_native`, since the session's own `step` and `bot_with`
//! now ask the module.
//!
//! It runs only in tests, from two places: `crates/delve`'s own tests, whose
//! engine is `answer` below (the module's two ops, answered by this code), and the
//! cockpit's parity tests, which tick the shipped module and this side by side and
//! compare every state. It is included by `#[path]` from both trees and names only
//! what a `super::*` supplies: the session's types and the accessors the rules
//! share with the pictures (`anchor`, `pressure`, `craft_s`, `latched`, `marks`,
//! `sit`, `fill_wing`, `slot`, `rider_slot`, `bit`, `medal_now`).
#![allow(clippy::all, dead_code, unused_imports)]
use super::bot::Skill;
use super::*;
use std::sync::OnceLock;

pub trait Rules {
    /// A fresh id for a foe.
    fn next_id(&mut self) -> u16;
    fn sound(&mut self, name: &'static str);
    /// A rider's short shout, shown as a portrait for a moment. The same line
    /// is not said twice in a row, and four show at most.
    fn say(&mut self, who: u8, line: Line);
    fn spark(&mut self, what: Burst, s: i32, x: i32, z: i32, n: u8);
    /// Who calls the hints: the first of the riders still flying, and when a
    /// full table leaves none (or all are lost), Aster, the wing's lead, who
    /// calls from the beacon's keep.
    fn voice(&self) -> u8;
    /// Who is at the table now (`present`), seated and stood up, and the
    /// foes' health provisioned for the seats now flying: a knight who sits
    /// down scales the beat up by its share, and a seat already provisioned
    /// does not scale twice (`Foe::seats`).
    fn muster(&mut self, here: &[u32]);
    /// One tick: the seats are whoever is here and awake, so a knight who
    /// leaves stands up and one who joins late sits down; then the beat.
    fn step_here(&mut self, inputs: &BTreeMap<u32, Input>, here: &[u32], nums: &Nums) -> Out;
    /// The calls, flashes and counters wind down.
    fn tick_lists(&mut self);
    /// The beat's end, a wipe, or the Wyrm's fall.
    fn end_beat(&mut self);
    /// The next beat flies on from where this one ends, the party mended a
    /// little and a bomb richer; what the beat made is banked.
    fn next_beat(&mut self);
    /// A beat starts over from its checkpoint: the foes the beat had are
    /// gone and its script runs again from there; what was banked stays, as
    /// do the Wyrm's lamps already out. Everyone is up, mended and unhurt for
    /// a moment.
    fn restart(&mut self);
    /// The Wyrm is down and the fly-through is over: the medal, and the
    /// tally.
    fn finish(&mut self);
    /// A craft by the id a bolt or a foe holds: a seat's, a rider's. Its
    /// place along, across and up, and whether it is flying.
    fn craft_at(&self, id: u8) -> Option<(i32, i32, i32, bool)>;
    /// One knight's tick in the air.
    fn fly(&mut self, id: u32, i: Input, n: &Nums);
    /// A tap: two bolts, side by side, both to where the reticle points.
    fn tap(&mut self, owner: u8, c: &Craft, s_c: i32);
    /// Everything the reticle sweeps over while F is held is locked: a
    /// bracket on each, snapping on. A great body (a lamp) takes up to three
    /// if the reticle dwells on it.
    fn acquire(&mut self, c: &mut Craft, s_c: i32);
    /// Letting go: a starburst for every lock, a chain of pops two ticks
    /// apart, homing on their bodies.
    fn release(&mut self, owner: u8, c: &mut Craft, s_c: i32, n: &Nums);
    fn push_bolt(&mut self, b: Bolt);
    /// The bolts' home pace (starbursts steer at it).
    fn star_speed() -> i32;
    /// The wingmen: formation flying with a little drift, a bolt now and
    /// then at what is nearest ahead.
    fn wing_fly(&mut self, _n: &Nums);
    /// After the beat's blows: a craft with no shield falls and rises again
    /// a moment later; a rider with none is lost to the beat.
    fn step_craft(&mut self, _n: &Nums);
    /// Put a foe on the rail, its health provisioned for the knights at the
    /// table; what it is worth is offered to the medal.
    fn spawn_foe(
        &mut self,
        kind: Kind,
        at: (i32, i32, i32),
        vel: (i32, i32, i32),
        st: St,
        mode: u8,
        phase: u8,
    ) -> Option<u16>;
    /// A craft's shield takes a blow. A knight is spared for a moment after
    /// one; a rider is only worn down (by what latches on). Whether it
    /// landed.
    fn hurt_craft(&mut self, id: u8, dmg: i32) -> bool;
    /// A drip from a latched kite: no spare moment, no flash.
    fn drain(&mut self, id: u8, amount: i32);
    /// Who a hunting kite goes for: a rider not yet latched, else a knight
    /// not yet latched, else anyone flying. The same hunter always finds the
    /// same victim.
    fn victim(&self, hunter: u16) -> Option<u8>;
    /// Who a turret or the Wyrm fires at: the knights flying, in turn.
    fn target(&self, turn: usize) -> Option<(i32, i32)>;
    /// A bolt from `o` that meets the formation's plane at `to` (across, up)
    /// as the rail brings the plane to it, closing at `closing` a tick.
    fn plane_shot(&self, o: (i32, i32, i32), to: (i32, i32), closing: i32) -> (i32, i32, i32);
    /// The foes' tick: flights, tells, latching, dives.
    fn step_foes(&mut self, n: &Nums);
    fn step_foe(&mut self, f: &mut Foe, anchor: i32, n: &Nums);
    /// A turret fires on its locked marks.
    fn fire_marks(&mut self, f: &Foe);
    /// A kite going for its victim: it picks one, closes, and latches onto
    /// the tail.
    fn hunt(&mut self, f: &mut Foe);
    /// A latched kite hangs on its victim's tail, draining its shield, until
    /// it is shot, or shaken off by a roll.
    fn hang(&mut self, f: &mut Foe);
    /// Dive-bombers meet craft: a blow for the knight, and the bomber is
    /// spent.
    fn bombers_ram(&mut self);
    /// A foe takes `dmg` from `by`; whether it fell.
    fn hit_foe(&mut self, fi: usize, dmg: u32, by: u8) -> bool;
    /// A foe pops into cold smoke and a spark of returned light. A kite that
    /// had latched on is a rescue; a knight's chain of pops is counted, and
    /// pays more the longer it runs.
    fn kill(&mut self, fi: usize, by: u8);
    /// A lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's
    /// tick sees it).
    fn break_lamp(&mut self, fi: usize, by: u8);
    /// Starfall: every hostile bolt is cleared, and everything small on the
    /// way is struck for what it can bear.
    fn starfall(&mut self, by: u8, s_c: i32, _x: i32, _z: i32);
    /// The bolts' tick: they fly and home, then land.
    fn step_bolts(&mut self, n: &Nums);
    /// A friendly bolt against the foes and the hostile sparks; whether it
    /// flies on.
    fn friendly_bolt(
        &mut self,
        b: &Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        dead: &mut [bool],
    ) -> bool;
    /// A friendly bolt lands on foe `i`.
    fn land(&mut self, i: usize, b: &Bolt);
    /// A hostile bolt against the knights: a roll's shield turns it round, a
    /// bare craft takes it. Whether it flies on.
    fn hostile_bolt(
        &mut self,
        b: &mut Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        n: &Nums,
    ) -> bool;
    /// A rolling craft's shield turns a bolt round: it is the roller's now,
    /// faster, and heads for what fired it (or the Wyrm's lit lamp).
    fn turn(&mut self, b: &mut Bolt, by: u32, at: (i32, i32, i32));
    /// Run the events that are due.
    fn run_script(&mut self, _n: &Nums);
    /// A centred spread: the `k`th of `count`, in half steps.
    fn spread(k: i32, count: i32) -> i32;
    fn act(&mut self, act: Act);
    /// The warm fly-through after the Wyrm: a procession of gold gates and
    /// hearth rings, lit for the way home. It pays nothing and offers
    /// nothing; it is the party's welcome.
    fn victory_gates(&mut self);
    /// The Wyrm rises out of the roost (or, after a fall, is back in its
    /// place, its lamps as they were).
    fn rise(&mut self);
    /// How long the Wyrm rests between attacks: shorter the more knights
    /// fly and the later the phase.
    fn rest(&self, phase: u8) -> u32;
    /// The Wyrm's tick.
    fn step_boss(&mut self, n: &Nums);
    /// An attack's footprints are locked and shown for the tell's ticks.
    fn begin_tell(&mut self, b: &mut Beast, n: &Nums);
    /// The Wyrm's spit: a Gloam bolt for each locked mark, from its mouth,
    /// each owing its return to the lit lamp.
    fn spit(&mut self, b: &Beast);
    /// Props and the knights who cross them.
    fn step_props(&mut self, n: &Nums);
    /// A gate or a ring flown through.
    fn take(&mut self, id: u32, p: &Prop, boosting: bool);
    /// Rock met: a blow, and the craft is spared a moment.
    fn scrape(&mut self, id: u32, s: i32, x: i32, z: i32);
    /// What knight `id` presses this tick, for the numbers `n` stand at.
    fn bot_native(&self, id: u32, n: &Nums, skill: Skill) -> Input;
    /// Whether this knight is one of the rollers: the even seats, and any
    /// knight at all when no even seat is flying.
    fn reflector(&self, id: u32) -> bool;
    /// While the lit lamp is shut, the soonest of the Wyrm's spits that is
    /// owed to it: ticks to it and where it will cross.
    fn intercept(&self, s_c: i32) -> Option<(u32, i32, i32)>;
    /// The soonest hostile bolt that will pass near this craft's plane:
    /// ticks to it, where it will cross, and whether it is a Gloam bolt.
    fn threat(&self, c: &Craft, s_c: i32) -> Option<(u32, i32, i32, bool)>;
    /// Where to go to be clear of the point `(ox, oz)`: four ways out of it
    /// (left, right, up, down), those the steer box allows, the one that ends
    /// farthest from it, so a knight at the edge goes up or down and not into
    /// the wall.
    fn room_from(c: &Craft, ox: i32, oz: i32, n: &Nums) -> (i32, i32);
    /// A footprint locked on this craft (a turret's, a bomber's lane): the
    /// place to be instead.
    fn footprint_away(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)>;
    /// The Wyrm's coil wants the gap, its roar wants the column left.
    fn boss_corridor(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)>;
}

impl Rules for Session {
    /// A fresh id for a foe.
    fn next_id(&mut self) -> u16 {
        let id = self.ids;
        self.ids = if self.ids >= 60_000 { 1 } else { self.ids + 1 };
        id
    }

    fn sound(&mut self, name: &'static str) {
        if self.snd.len() < 12 && !self.snd.contains(&name) {
            self.snd.push(name);
        }
    }

    /// A rider's short shout, shown as a portrait for a moment. The same line
    /// is not said twice in a row, and four show at most.
    fn say(&mut self, who: u8, line: Line) {
        if self.calls.iter().any(|c| c.line == line && c.ticks > 40) {
            return;
        }
        if self.calls.len() >= MAX_CALLS {
            self.calls.remove(0);
        }
        self.calls.push(Call {
            who,
            line,
            ticks: 100,
        });
    }

    fn spark(&mut self, what: Burst, s: i32, x: i32, z: i32, n: u8) {
        if self.fx.len() >= MAX_FX {
            self.fx.remove(0);
        }
        self.fx.push(Flare {
            what,
            s,
            x,
            z,
            ticks: match what {
                Burst::Starfall => 36,
                Burst::Fall => 40,
                Burst::Lock => 8,
                _ => 14,
            },
            n,
        });
    }

    /// Who calls the hints: the first of the riders still flying, and when a
    /// full table leaves none (or all are lost), Aster, the wing's lead, who
    /// calls from the beacon's keep.
    fn voice(&self) -> u8 {
        self.wing.iter().find(|w| !w.lost).map_or(7, |w| w.id)
    }

    /// Who is at the table now (`present`), seated and stood up, and the
    /// foes' health provisioned for the seats now flying: a knight who sits
    /// down scales the beat up by its share, and a seat already provisioned
    /// does not scale twice (`Foe::seats`).
    fn muster(&mut self, here: &[u32]) {
        self.seats.retain(|id, _| here.contains(id));
        for &id in here {
            if !self.seats.contains_key(&id) && (1..=MAX_PLAYERS).contains(&id) {
                self.sit(id, self.elapsed);
                if self.elapsed > 0 {
                    self.say(id as u8, Line::Join);
                    self.sound("play_card");
                }
            }
        }
        let now = self.pressure();
        for f in &mut self.foes {
            if f.seats < now {
                let was = u32::from(f.seats.max(1));
                f.hp = (u64::from(f.hp) * u64::from(now) / u64::from(was)) as u32;
                f.full = f.full * u32::from(now) / was;
                f.seats = now;
            }
        }
        self.fill_wing();
    }

    /// One tick: the seats are whoever is here and awake, so a knight who
    /// leaves stands up and one who joins late sits down; then the beat.
    fn step_here(&mut self, inputs: &BTreeMap<u32, Input>, here: &[u32], nums: &Nums) -> Out {
        self.elapsed = (self.elapsed + 1).min(LIMIT);
        self.muster(here);
        let live: Vec<u32> = self.seats.keys().copied().collect();
        let keys = |id: u32| {
            inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default()
        };
        match self.mode {
            Mode::Play => {
                self.t = (self.t + 1).min(LIMIT);
                for &id in &live {
                    self.fly(id, keys(id), nums);
                }
                self.wing_fly(nums);
                self.run_script(nums);
                self.step_foes(nums);
                self.step_boss(nums);
                self.step_bolts(nums);
                self.step_props(nums);
                self.step_craft(nums);
                self.tick_lists();
                self.end_beat();
            }
            Mode::Retry => {
                self.tick_lists();
                self.mode_t = self.mode_t.saturating_sub(1);
                if self.mode_t == 0 {
                    self.restart();
                }
            }
            Mode::Victory | Mode::Tally => {
                // The rail carries them home; at the tally the wing hovers.
                if self.mode == Mode::Victory {
                    self.t = (self.t + 1).min(LIMIT);
                }
                for &id in &live {
                    self.fly(id, keys(id), nums);
                }
                self.wing_fly(nums);
                self.step_bolts(nums);
                self.step_props(nums);
                self.step_craft(nums);
                self.tick_lists();
                self.mode_t = (self.mode_t + 1).min(LIMIT);
                if self.mode == Mode::Victory && self.mode_t >= VICTORY {
                    self.finish();
                } else if self.mode == Mode::Tally && self.mode_t > TALLY_HOLD {
                    for &id in &live {
                        let fresh = self
                            .seats
                            .get(&id)
                            .is_some_and(|s| s.craft.keys & K_SPUR == 0);
                        if keys(id).interact && fresh {
                            self.leave.push(id);
                        }
                    }
                }
            }
        }
        // The keys of this tick stand for the next one's presses.
        for (&id, seat) in &mut self.seats {
            seat.craft.keys = bits(keys(id));
        }
        Out {
            sounds: std::mem::take(&mut self.snd),
            leave: std::mem::take(&mut self.leave),
        }
    }

    /// The calls, flashes and counters wind down.
    fn tick_lists(&mut self) {
        for c in &mut self.calls {
            c.ticks = c.ticks.saturating_sub(1);
        }
        self.calls.retain(|c| c.ticks > 0);
        for f in &mut self.fx {
            f.ticks = f.ticks.saturating_sub(1);
        }
        self.fx.retain(|f| f.ticks > 0);
        for f in &mut self.foes {
            f.flash = f.flash.saturating_sub(1);
        }
    }

    /// The beat's end, a wipe, or the Wyrm's fall.
    fn end_beat(&mut self) {
        // A wipe: every knight down at once.
        if !self.seats.is_empty() && self.seats.values().all(|s| s.craft.down > 0) {
            self.mode = Mode::Retry;
            self.mode_t = RETRY;
            self.sound("knight_fall");
            return;
        }
        if self.stage < 2 && self.t >= rail::LEN[usize::from(self.stage)] {
            self.next_beat();
            return;
        }
        let dying = self
            .boss
            .as_ref()
            .is_some_and(|b| b.st == Wyrm::Dying && b.ticks == 0);
        if self.stage == 2 && dying {
            self.mode = Mode::Victory;
            self.mode_t = 0;
            self.done = Some(self.elapsed);
            self.home = self.anchor();
            self.beat.pts += Kind::Lamp.worth() * 2;
            self.beat.base += Kind::Lamp.worth() * 2;
            self.beat.offered += Kind::Lamp.worth() * 2;
            self.foes.clear();
            self.bolts.clear();
            self.say(self.voice(), Line::Home);
            self.sound("boss_death");
            self.victory_gates();
        }
    }

    /// The next beat flies on from where this one ends, the party mended a
    /// little and a bomb richer; what the beat made is banked.
    fn next_beat(&mut self) {
        self.bank.bank(&self.beat);
        self.beat = Tally::default();
        self.stage += 1;
        self.t = 0;
        self.cp = 0;
        self.next = 0;
        let line = if self.stage == 1 {
            Line::Ruins
        } else {
            Line::Roost
        };
        self.say(self.voice(), line);
        self.sound("play_card");
        for s in self.seats.values_mut() {
            s.craft.shield = (s.craft.shield + 40).min(SHIELD);
            s.craft.bombs = (s.craft.bombs + 1).min(3);
            s.craft.meter = METER;
            s.craft.spent = false;
        }
        for w in &mut self.wing {
            w.lost = false;
            w.craft.shield = (w.craft.shield + 40).min(SHIELD);
        }
    }

    /// A beat starts over from its checkpoint: the foes the beat had are
    /// gone and its script runs again from there; what was banked stays, as
    /// do the Wyrm's lamps already out. Everyone is up, mended and unhurt for
    /// a moment.
    fn restart(&mut self) {
        self.retries = self.retries.saturating_add(1);
        self.beat = Tally::default();
        self.t = self.cp;
        self.next = resume(self.stage, self.cp);
        self.props.clear();
        self.bolts.clear();
        self.fx.clear();
        self.calls.clear();
        self.foes.retain(|f| f.kind == Kind::Lamp);
        if let Some(b) = &mut self.boss {
            b.st = Wyrm::Idle;
            b.ticks = 2 * HZ;
            b.marks.clear();
        }
        for (&id, s) in &mut self.seats {
            let (x, z) = slot(id);
            let bombs = s.craft.bombs.max(2);
            let (pts, hits, saved, gates) =
                (s.craft.pts, s.craft.hits, s.craft.saved, s.craft.gates);
            s.craft = Craft::new(x, z);
            s.craft.hurt = 2 * HZ;
            s.craft.bombs = bombs;
            (s.craft.pts, s.craft.hits, s.craft.saved, s.craft.gates) = (pts, hits, saved, gates);
        }
        for w in &mut self.wing {
            w.lost = false;
            w.craft.shield = SHIELD;
            w.craft.down = 0;
        }
        self.mode = Mode::Play;
        self.mode_t = 0;
        self.say(self.voice(), Line::Hello);
    }

    /// The Wyrm is down and the fly-through is over: the medal, and the
    /// tally.
    fn finish(&mut self) {
        self.bank.bank(&self.beat);
        self.beat = Tally::default();
        self.medal = Some(self.medal_now());
        self.mode = Mode::Tally;
        self.mode_t = 0;
        self.sound("play_card");
    }

    /// A craft by the id a bolt or a foe holds: a seat's, a rider's. Its
    /// place along, across and up, and whether it is flying.
    fn craft_at(&self, id: u8) -> Option<(i32, i32, i32, bool)> {
        let c = if (1..=6).contains(&id) {
            &self.seats.get(&u32::from(id))?.craft
        } else {
            let w = self.wing.iter().find(|w| w.id == id)?;
            return Some((
                self.craft_s(&w.craft),
                w.craft.x,
                w.craft.z,
                !w.lost && w.craft.down == 0,
            ));
        };
        Some((self.craft_s(c), c.x, c.z, c.flying()))
    }

    /// One knight's tick in the air.
    fn fly(&mut self, id: u32, i: Input, n: &Nums) {
        let Some(seat) = self.seats.get(&id) else {
            return;
        };
        let mut c = seat.craft.clone();
        let old = c.keys;
        let edge = |key: u8, on: bool| on && old & key == 0;
        let (wx, wz) = (n.wx(), n.wz());
        c.hurt = c.hurt.saturating_sub(1);
        c.cool = c.cool.saturating_sub(1);
        c.rest = c.rest.saturating_sub(1);
        c.chain_ticks = c.chain_ticks.saturating_sub(1);
        if c.chain_ticks == 0 {
            c.chain = 0;
        }
        c.tap_ticks = c.tap_ticks.saturating_sub(1);
        if c.down > 0 {
            c.down -= 1;
            c.ds = PACE;
            c.held = 0;
            c.charge = 0;
            c.locks.clear();
            if c.down == 0 {
                // Back in the wing, mended a little, unhurt for a moment.
                let (x, z) = slot(id);
                c.x = x;
                c.z = z;
                c.vx = 0;
                c.vz = 0;
                c.shield = 60;
                c.hurt = 2 * HZ;
                c.roll = 0;
            }
            if let Some(s) = self.seats.get_mut(&id) {
                s.craft = c;
            }
            return;
        }
        // A second tap of a direction is a roll that way.
        let mut double = false;
        if i.move_x != 0 && c.last_x != i.move_x {
            if c.tap_dir == i.move_x && c.tap_ticks > 0 {
                double = true;
                c.tap_ticks = 0;
            } else {
                c.tap_dir = i.move_x;
                c.tap_ticks = TAP_WINDOW;
            }
        }
        c.last_x = i.move_x;
        // The roll: a spinning shield, and a throw to the side.
        if (edge(K_DASH, i.dash) || double) && c.rest == 0 && c.roll == 0 {
            c.dir = if i.move_x != 0 {
                i.move_x.signum()
            } else if c.vx.abs() > 8 {
                c.vx.signum() as i8
            } else {
                c.dir
            };
            c.roll = n.roll;
            c.roll_len = n.roll;
            c.rest = n.roll + ROLL_REST;
            c.vx = i32::from(c.dir) * ROLL_V;
            c.held = 0;
            c.charge = 0;
            c.locks.clear();
            self.sound("roll");
        }
        c.roll = c.roll.saturating_sub(1);
        // The reins.
        let (tx, tz) = (i32::from(i.move_x) * V_X, -i32::from(i.move_y) * V_Z);
        c.vx += (tx - c.vx).clamp(-ACC, ACC);
        c.vz += (tz - c.vz).clamp(-ACC, ACC);
        c.x += c.vx;
        c.z += c.vz;
        if c.x.abs() > wx {
            c.x = wx * c.x.signum();
            c.vx = 0;
        }
        if c.z.abs() > wz {
            c.z = wz * c.z.signum();
            c.vz = 0;
        }
        // The lean follows the sideways speed a little late.
        c.bank += (c.vx * Q / V_X - c.bank) / 4;
        c.lift += (c.vz * Q / V_Z - c.lift) / 4;
        // The spur on a short meter, and the brake.
        let drain = METER / n.spur.max(1) as i32;
        let refill = METER / (n.spur.max(1) as i32 * 5 / 2).max(1);
        c.boost = i.interact && !c.spent && c.meter > 0 && c.roll == 0;
        c.brake = i.swing && !c.boost;
        if c.boost {
            c.meter = (c.meter - drain).max(0);
            if c.meter == 0 {
                c.spent = true;
            }
        } else {
            c.meter = (c.meter + refill).min(METER);
            if c.spent && c.meter >= METER / 4 {
                c.spent = false;
            }
        }
        let want = if c.boost {
            BOOST_BACK
        } else if c.brake {
            -BRAKE_BACK
        } else {
            0
        };
        let back = c.back + (want - c.back).clamp(-BACK_V, BACK_V);
        c.ds = PACE + back - c.back;
        c.back = back;
        // The reticle: instant, and it stays where it is put.
        c.rx = (c.rx + i32::from(i.aim_x) * RET_V).clamp(-RX_MAX, RX_MAX);
        c.rz = (c.rz - i32::from(i.aim_y) * RET_V).clamp(-RZ_MAX, RZ_MAX);
        let s_c = self.craft_s(&c);
        // Fire: a tap is a twin bolt; a hold charges and locks; letting go
        // sends the locks.
        if c.roll == 0 && i.fire {
            if edge(K_FIRE, true) {
                c.held = 1;
                if c.cool == 0 {
                    self.tap(id as u8, &c, s_c);
                    c.cool = TAP_REST;
                }
            } else if c.held > 0 {
                c.held = (c.held + 1).min(30_000);
                if c.held > HOLD_AT {
                    c.charge = (c.charge + 1).min(n.charge);
                    self.acquire(&mut c, s_c);
                }
            }
        } else if c.held > 0 {
            if c.charge > 0 && c.roll == 0 {
                self.release(id as u8, &mut c, s_c, n);
            }
            c.held = 0;
            c.charge = 0;
            c.dwell = 0;
            c.locks.clear();
        }
        // Starfall.
        if edge(K_BOMB, i.bomb) && c.bombs > 0 && c.roll == 0 {
            c.bombs -= 1;
            self.starfall(id as u8, s_c, c.x, c.z);
        }
        if let Some(s) = self.seats.get_mut(&id) {
            s.craft = c;
        }
    }

    /// A tap: two bolts, side by side, both to where the reticle points.
    fn tap(&mut self, owner: u8, c: &Craft, s_c: i32) {
        let muzzle = s_c + dm(12);
        let to = (s_c + FAR, c.x + c.rx, c.z + c.rz);
        for side in [-1, 1] {
            let from = (muzzle, c.x + side * dm(4), c.z);
            let v = scale3((to.0 - from.0, to.1 - from.1, to.2 - from.2), V_BOLT);
            self.push_bolt(Bolt {
                shot: Arm::Lance,
                owner,
                s: from.0,
                x: from.1,
                z: from.2,
                vs: PACE + v.0,
                vx: v.1,
                vz: v.2,
                dmg: LANCE,
                life: BOLT_LIFE,
                aim: 0,
                src: 0,
                delay: 0,
                full: false,
            });
        }
        self.sound("shot_bolt");
    }

    /// Everything the reticle sweeps over while F is held is locked: a
    /// bracket on each, snapping on. A great body (a lamp) takes up to three
    /// if the reticle dwells on it.
    fn acquire(&mut self, c: &mut Craft, s_c: i32) {
        let alive: Vec<(u16, Kind, i32, i32, i32, bool)> = self
            .foes
            .iter()
            .map(|f| (f.id, f.kind, f.s, f.x, f.z, f.lockable()))
            .collect();
        c.locks.retain(|l| {
            alive.iter().any(|&(id, _, s, _, _, ok)| {
                id == l.foe && ok && (LOCK_MIN..=LOCK_MAX + dm(40)).contains(&(s - s_c))
            })
        });
        let mut total: usize = c.locks.iter().map(|l| usize::from(l.n)).sum();
        let mut dwelt = false;
        for &(id, kind, s, x, z, ok) in &alive {
            let ds = s - s_c;
            if !ok || !(LOCK_MIN..=LOCK_MAX).contains(&ds) {
                continue;
            }
            // The reticle's ray at that depth, against the body.
            let ax = (i64::from(c.rx) * i64::from(ds) / i64::from(FAR)) as i32;
            let az = (i64::from(c.rz) * i64::from(ds) / i64::from(FAR)) as i32;
            if flat(x - c.x, z - c.z, ax, az) > i64::from(kind.radius() + LOCK_R) {
                continue;
            }
            match c.locks.iter_mut().find(|l| l.foe == id) {
                Some(l) => {
                    if kind == Kind::Lamp && l.n < MAX_PER_BODY && total < MAX_LOCKS {
                        dwelt = true;
                        c.dwell += 1;
                        if c.dwell >= 8 {
                            l.n += 1;
                            c.dwell = 0;
                            total += 1;
                            self.sound("spoil_pickup");
                        }
                    }
                }
                None => {
                    if total < MAX_LOCKS {
                        c.locks.push(Lock { foe: id, n: 1 });
                        total += 1;
                        c.dwell = 0;
                        self.sound("spoil_pickup");
                        self.spark(Burst::Lock, s, x, z, total as u8);
                    }
                }
            }
        }
        if !dwelt {
            c.dwell = 0;
        }
    }

    /// Letting go: a starburst for every lock, a chain of pops two ticks
    /// apart, homing on their bodies.
    fn release(&mut self, owner: u8, c: &mut Craft, s_c: i32, n: &Nums) {
        let full = c.charge >= n.charge;
        let dmg = STAR_MIN + (STAR_MAX - STAR_MIN) * c.charge.min(n.charge) / n.charge.max(1);
        let mut k = 0i32;
        let locks = c.locks.clone();
        for l in &locks {
            for _ in 0..l.n {
                let spread =
                    [(-70, 40), (70, 40), (0, 90), (-40, -40), (40, -40), (0, 0)][(k % 6) as usize];
                self.push_bolt(Bolt {
                    shot: Arm::Star,
                    owner,
                    s: s_c + dm(12),
                    x: c.x,
                    z: c.z,
                    vs: PACE + 120,
                    vx: spread.0,
                    vz: spread.1,
                    dmg: dmg as u16,
                    life: STAR_LIFE,
                    aim: l.foe,
                    src: 0,
                    delay: (k * 2) as u8,
                    full,
                });
                k += 1;
            }
        }
        if k > 0 {
            c.cool = REL_REST;
            self.sound("shot_spark");
        }
    }

    fn push_bolt(&mut self, b: Bolt) {
        if self.bolts.len() < MAX_BOLTS {
            self.bolts.push(b);
        }
    }

    /// The bolts' home pace (starbursts steer at it).
    fn star_speed() -> i32 {
        V_STAR
    }

    /// The wingmen: formation flying with a little drift, a bolt now and
    /// then at what is nearest ahead.
    fn wing_fly(&mut self, _n: &Nums) {
        let anchor = self.anchor();
        let t = self.t;
        let targets: Vec<(i32, i32, i32)> = self
            .foes
            .iter()
            .filter(|f| f.lockable() && f.kind != Kind::Lamp)
            .map(|f| (f.s, f.x, f.z))
            .collect();
        for k in 0..self.wing.len() {
            let mut w = self.wing[k].clone();
            if w.lost {
                // Falling away, out of the picture.
                w.craft.z = (w.craft.z - 20).max(-dm(110));
                self.wing[k] = w;
                continue;
            }
            let (sx, sz, back) = rider_slot(k);
            let kk = k as u32;
            let tx = sx + tri(t + 41 * kk, 130 + 16 * kk, dm(16));
            let tz = sz + tri(t + 29 * kk, 96 + 12 * kk, dm(6));
            let (ox, oz) = (w.craft.x, w.craft.z);
            w.craft.x += (tx - w.craft.x) / 8;
            w.craft.z += (tz - w.craft.z) / 8;
            w.craft.vx = w.craft.x - ox;
            w.craft.vz = w.craft.z - oz;
            w.craft.bank += (w.craft.vx * Q / V_X - w.craft.bank) / 4;
            w.craft.lift += (w.craft.vz * Q / V_Z - w.craft.lift) / 4;
            // A rider coming back up from a fall leans no further than a knight.
            w.craft.bank = w.craft.bank.clamp(-Q, Q);
            w.craft.lift = w.craft.lift.clamp(-Q, Q);
            w.craft.back = back;
            w.craft.hurt = w.craft.hurt.saturating_sub(1);
            w.fire = w.fire.saturating_sub(1);
            let me = (anchor + back, w.craft.x, w.craft.z);
            if w.fire == 0 {
                // The nearest foe ahead, within range.
                let pick = targets
                    .iter()
                    .filter(|t| (dm(60)..=dm(400)).contains(&(t.0 - me.0)))
                    .min_by_key(|t| norm((t.0 - me.0, t.1 - me.1, t.2 - me.2)));
                if let Some(&to) = pick {
                    let v = scale3((to.0 - me.0, to.1 - me.1, to.2 - me.2), V_BOLT);
                    let owner = w.id;
                    self.push_bolt(Bolt {
                        shot: Arm::Ally,
                        owner,
                        s: me.0 + dm(12),
                        x: me.1,
                        z: me.2,
                        vs: PACE + v.0,
                        vx: v.1,
                        vz: v.2,
                        dmg: 6,
                        life: BOLT_LIFE,
                        aim: 0,
                        src: 0,
                        delay: 0,
                        full: false,
                    });
                    w.fire = 14 + 3 * kk;
                } else {
                    w.fire = 6;
                }
            }
            self.wing[k] = w;
        }
    }

    /// After the beat's blows: a craft with no shield falls and rises again
    /// a moment later; a rider with none is lost to the beat.
    fn step_craft(&mut self, _n: &Nums) {
        let anchor = self.anchor();
        let ids: Vec<u32> = self.seats.keys().copied().collect();
        for id in ids {
            let (down_now, at) = {
                let c = &self.seats[&id].craft;
                (c.shield <= 0 && c.down == 0, (anchor + c.back, c.x, c.z))
            };
            if down_now {
                if let Some(s) = self.seats.get_mut(&id) {
                    s.craft.down = RISE;
                    s.craft.shield = 0;
                    s.craft.roll = 0;
                    s.craft.held = 0;
                    s.craft.charge = 0;
                    s.craft.locks.clear();
                }
                self.spark(Burst::Fall, at.0, at.1, at.2, id as u8);
                self.sound("knight_fall");
                // Whatever hung on it lets go.
                for f in &mut self.foes {
                    if f.st == St::Latched && u32::from(f.on) == id {
                        f.st = St::Flee;
                        f.ticks = 40;
                    }
                }
            }
        }
        for k in 0..self.wing.len() {
            let (gone, at) = {
                let w = &self.wing[k];
                (
                    !w.lost && w.craft.shield <= 0,
                    (anchor + w.craft.back, w.craft.x, w.craft.z),
                )
            };
            if gone {
                let id = self.wing[k].id;
                self.wing[k].lost = true;
                self.beat.lost += 1;
                self.say(id, Line::Lost);
                self.spark(Burst::Fall, at.0, at.1, at.2, id);
                self.sound("knight_fall");
                for f in &mut self.foes {
                    if f.st == St::Latched && f.on == id {
                        f.st = St::Flee;
                        f.ticks = 40;
                    }
                }
            }
        }
    }

    /// Put a foe on the rail, its health provisioned for the knights at the
    /// table; what it is worth is offered to the medal.
    fn spawn_foe(
        &mut self,
        kind: Kind,
        at: (i32, i32, i32),
        vel: (i32, i32, i32),
        st: St,
        mode: u8,
        phase: u8,
    ) -> Option<u16> {
        if self.foes.len() >= MAX_FOES {
            return None;
        }
        let seats = self.pressure();
        let full = kind.health() * u32::from(seats);
        let id = self.next_id();
        self.foes.push(Foe {
            id,
            kind,
            s: at.0,
            x: at.1,
            z: at.2,
            vs: vel.0,
            vx: vel.1,
            vz: vel.2,
            hp: full,
            full,
            seats,
            st,
            ticks: 0,
            fx: 0,
            fz: 0,
            on: 0,
            flash: 0,
            mode,
            phase,
        });
        self.beat.offered += kind.worth();
        Some(id)
    }

    /// A craft's shield takes a blow. A knight is spared for a moment after
    /// one; a rider is only worn down (by what latches on). Whether it
    /// landed.
    fn hurt_craft(&mut self, id: u8, dmg: i32) -> bool {
        let at = self.craft_at(id);
        if (1..=6).contains(&id) {
            let Some(seat) = self.seats.get_mut(&u32::from(id)) else {
                return false;
            };
            let c = &mut seat.craft;
            if c.hurt > 0 || c.down > 0 {
                return false;
            }
            c.shield = (c.shield - dmg).max(0);
            c.hurt = HURT;
            if let Some((s, x, z, _)) = at {
                self.spark(Burst::Hurt, s, x, z, id);
            }
            self.sound(if dmg >= 20 {
                "knight_hit_big"
            } else {
                "hero_hurt"
            });
            if self.seats[&u32::from(id)].craft.shield <= 30 {
                self.say(id, Line::Low);
            }
            true
        } else if let Some(w) = self.wing.iter_mut().find(|w| w.id == id) {
            if w.lost {
                return false;
            }
            w.craft.shield = (w.craft.shield - dmg).max(0);
            true
        } else {
            false
        }
    }

    /// A drip from a latched kite: no spare moment, no flash.
    fn drain(&mut self, id: u8, amount: i32) {
        if (1..=6).contains(&id) {
            if let Some(s) = self.seats.get_mut(&u32::from(id)) {
                if s.craft.down == 0 {
                    s.craft.shield = (s.craft.shield - amount).max(0);
                }
            }
        } else if let Some(w) = self.wing.iter_mut().find(|w| w.id == id && !w.lost) {
            w.craft.shield = (w.craft.shield - amount).max(0);
        }
    }

    /// Who a hunting kite goes for: a rider not yet latched, else a knight
    /// not yet latched, else anyone flying. The same hunter always finds the
    /// same victim.
    fn victim(&self, hunter: u16) -> Option<u8> {
        let free = |id: u8| self.craft_at(id).is_some_and(|c| c.3) && !self.latched(id);
        let riders: Vec<u8> = self
            .wing
            .iter()
            .map(|w| w.id)
            .filter(|&id| free(id))
            .collect();
        if !riders.is_empty() {
            return Some(riders[usize::from(hunter) % riders.len()]);
        }
        let knights: Vec<u8> = self
            .seats
            .keys()
            .map(|&id| id as u8)
            .filter(|&id| free(id))
            .collect();
        if !knights.is_empty() {
            return Some(knights[usize::from(hunter) % knights.len()]);
        }
        self.seats
            .keys()
            .map(|&id| id as u8)
            .find(|&id| self.craft_at(id).is_some_and(|c| c.3))
    }

    /// Who a turret or the Wyrm fires at: the knights flying, in turn.
    fn target(&self, turn: usize) -> Option<(i32, i32)> {
        let flying: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.craft.flying())
            .map(|s| (s.craft.x, s.craft.z))
            .collect();
        if flying.is_empty() {
            None
        } else {
            Some(flying[turn % flying.len()])
        }
    }

    /// A bolt from `o` that meets the formation's plane at `to` (across, up)
    /// as the rail brings the plane to it, closing at `closing` a tick.
    fn plane_shot(&self, o: (i32, i32, i32), to: (i32, i32), closing: i32) -> (i32, i32, i32) {
        let gap = (o.0 - self.anchor()).max(dm(40));
        let ticks = (gap / closing.max(1)).max(1);
        (PACE - closing, (to.0 - o.1) / ticks, (to.1 - o.2) / ticks)
    }

    /// The foes' tick: flights, tells, latching, dives.
    fn step_foes(&mut self, n: &Nums) {
        let anchor = self.anchor();
        let mut foes = std::mem::take(&mut self.foes);
        for f in &mut foes {
            self.step_foe(f, anchor, n);
        }
        self.foes = foes;
        // Those that have gone by, or fled out of sight, are gone.
        self.foes.retain(|f| {
            (f.kind == Kind::Lamp)
                || (f.hp > 0 && f.s > anchor - dm(300) && f.s < anchor + dm(1500))
        });
        // A bomber that was thrown at a craft.
        self.bombers_ram();
    }

    fn step_foe(&mut self, f: &mut Foe, anchor: i32, n: &Nums) {
        let t = self.t;
        // A hit stops the foe a moment where it is, each foe by itself (the
        // shared sim never pauses): the hit lands.
        if f.flash >= 4 && matches!(f.st, St::Fly | St::Hunt) {
            return;
        }
        match (f.kind, f.st) {
            (Kind::Wisp | Kind::Kite, St::Fly) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                if f.mode == 1 && (t + u32::from(f.phase)) % 34 == 0 {
                    f.vx = -f.vx;
                }
                // A shooting wisp: a short tell, then a slow spark.
                if f.mode == 3 && f.kind == Kind::Wisp {
                    f.ticks = f.ticks.saturating_sub(1);
                    let ahead = f.s - anchor;
                    if f.ticks == 0 && (dm(200)..=dm(460)).contains(&ahead) {
                        if let Some(to) = self.target(usize::from(f.id)) {
                            f.fx = to.0;
                            f.fz = to.1;
                            f.st = St::Tell;
                            f.ticks = n.tell;
                        }
                    }
                }
            }
            (Kind::Wisp, St::Tell) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    let v = self.plane_shot((f.s, f.x, f.z), (f.fx, f.fz), dm(4));
                    self.push_bolt(Bolt {
                        shot: Arm::Spark,
                        owner: 0,
                        s: f.s,
                        x: f.x,
                        z: f.z,
                        vs: v.0,
                        vx: v.1,
                        vz: v.2,
                        dmg: SPARK as u16,
                        life: 220,
                        aim: 0,
                        src: f.id,
                        delay: 0,
                        full: false,
                    });
                    f.st = St::Fly;
                    f.ticks = 100;
                }
            }
            (Kind::Kite, St::Hunt) => self.hunt(f),
            (Kind::Kite, St::Latched) => self.hang(f),
            (_, St::Flee) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    f.hp = 0;
                }
            }
            (Kind::Turret, St::Rest) => {
                f.ticks = f.ticks.saturating_sub(1);
                let ahead = f.s - anchor;
                if f.ticks == 0 && (dm(150)..=dm(560)).contains(&ahead) {
                    if let Some(to) = self.target(usize::from(f.id) + usize::from(f.phase)) {
                        f.fx = to.0;
                        f.fz = to.1;
                        f.st = St::Tell;
                        f.ticks = n.tell;
                        self.sound("vent_fire");
                    }
                }
            }
            (Kind::Turret, St::Tell) => {
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    self.fire_marks(f);
                    f.phase = f.phase.wrapping_add(1);
                    f.st = St::Rest;
                    f.ticks = 26;
                }
            }
            (Kind::Bomber, St::Hide) => {
                if f.s - anchor <= dm(330) {
                    if let Some(to) = self.target(usize::from(f.id)) {
                        f.fx = to.0;
                        f.fz = to.1;
                        f.st = St::Tell;
                        f.ticks = n.tell;
                    }
                }
            }
            (Kind::Bomber, St::Tell) => {
                // It rises over the ridge, then dives down the locked lane.
                if f.z < f.fz.max(dm(10)) + dm(24) {
                    f.z += dm(1);
                }
                f.ticks = f.ticks.saturating_sub(1);
                if f.ticks == 0 {
                    let gap = (f.s - anchor).max(dm(60));
                    let ticks = 32;
                    let closing = gap / ticks;
                    f.vs = PACE - closing;
                    f.vx = (f.fx - f.x) / ticks;
                    f.vz = (f.fz - f.z) / ticks;
                    f.st = St::Dive;
                }
            }
            (Kind::Bomber, St::Dive) => {
                f.s += f.vs;
                f.x += f.vx;
                f.z += f.vz;
            }
            _ => {}
        }
    }

    /// A turret fires on its locked marks.
    fn fire_marks(&mut self, f: &Foe) {
        for to in Session::marks(f, self.pressure()) {
            let v = self.plane_shot((f.s, f.x, f.z + dm(12)), to, dm(5));
            self.push_bolt(Bolt {
                shot: Arm::Gloam,
                owner: 0,
                s: f.s,
                x: f.x,
                z: f.z + dm(12),
                vs: v.0,
                vx: v.1,
                vz: v.2,
                dmg: GLOAM as u16,
                life: 260,
                aim: 0,
                src: f.id,
                delay: 0,
                full: false,
            });
        }
        self.sound("shot_crossbow");
    }

    /// A kite going for its victim: it picks one, closes, and latches onto
    /// the tail.
    fn hunt(&mut self, f: &mut Foe) {
        let held = f.on != 0 && self.craft_at(f.on).is_some_and(|c| c.3);
        if !held {
            match self.victim(f.id) {
                Some(v) => f.on = v,
                None => {
                    f.st = St::Flee;
                    f.ticks = 40;
                    f.vz = 60;
                    return;
                }
            }
        }
        let Some((vs, vx, vz, _)) = self.craft_at(f.on) else {
            return;
        };
        let to = (vs - LATCH_BACK, vx, vz + dm(4));
        let d = (to.0 - f.s, to.1 - f.x, to.2 - f.z);
        if norm(d) <= i64::from(dm(9)) {
            f.st = St::Latched;
            f.ticks = 0;
            f.s = to.0;
            f.x = to.1;
            f.z = to.2;
            let who = f.on;
            self.say(who, Line::Latched);
            self.sound("clang");
        } else {
            let v = scale3(d, HUNT_V);
            f.vs = v.0;
            f.vx = v.1;
            f.vz = v.2;
            f.s += v.0;
            f.x += v.1;
            f.z += v.2;
        }
    }

    /// A latched kite hangs on its victim's tail, draining its shield, until
    /// it is shot, or shaken off by a roll.
    fn hang(&mut self, f: &mut Foe) {
        let Some((vs, vx, vz, ok)) = self.craft_at(f.on) else {
            f.st = St::Flee;
            f.ticks = 40;
            return;
        };
        let rolling = (1..=6).contains(&f.on)
            && self
                .seats
                .get(&u32::from(f.on))
                .is_some_and(|s| s.craft.roll > 0);
        if !ok || rolling {
            f.st = St::Flee;
            f.ticks = 40;
            f.vs = 0;
            f.vx = if vx >= 0 { 90 } else { -90 };
            f.vz = 70;
            return;
        }
        f.s = vs - LATCH_BACK;
        f.x = vx;
        f.z = vz + dm(4);
        f.ticks = (f.ticks + 1) % 9_996;
        if f.ticks % 12 == 0 {
            let amount = if f.on <= 6 { 2 } else { 3 };
            self.drain(f.on, amount);
        }
    }

    /// Dive-bombers meet craft: a blow for the knight, and the bomber is
    /// spent.
    fn bombers_ram(&mut self) {
        let crafts: Vec<(u8, i32, i32, i32)> = self
            .seats
            .iter()
            .filter(|(_, s)| s.craft.flying())
            .map(|(&id, s)| (id as u8, self.craft_s(&s.craft), s.craft.x, s.craft.z))
            .collect();
        let mut spent = Vec::new();
        for f in &self.foes {
            if f.kind != Kind::Bomber || f.st != St::Dive {
                continue;
            }
            let from = (f.s - f.vs, f.x - f.vx, f.z - f.vz);
            for &(id, s, x, z) in &crafts {
                if near(from, (f.s, f.x, f.z), (s, x, z), f.radius() + CRAFT_R) {
                    spent.push((f.id, id));
                    break;
                }
            }
        }
        for (fid, id) in spent {
            if self.hurt_craft(id, RAM) {
                if let Some(i) = self.foes.iter().position(|f| f.id == fid) {
                    let f = self.foes.remove(i);
                    self.spark(Burst::Pop, f.s, f.x, f.z, 0);
                    self.sound("bomb_toss");
                }
            }
        }
    }

    /// A foe takes `dmg` from `by`; whether it fell.
    fn hit_foe(&mut self, fi: usize, dmg: u32, by: u8) -> bool {
        let f = &mut self.foes[fi];
        f.hp = f.hp.saturating_sub(dmg);
        f.flash = 5;
        if f.hp > 0 {
            self.sound("hit_magic");
            return false;
        }
        if f.kind == Kind::Lamp {
            self.break_lamp(fi, by);
            return true;
        }
        self.kill(fi, by);
        true
    }

    /// A foe pops into cold smoke and a spark of returned light. A kite that
    /// had latched on is a rescue; a knight's chain of pops is counted, and
    /// pays more the longer it runs.
    fn kill(&mut self, fi: usize, by: u8) {
        let f = self.foes.remove(fi);
        let mut gain = f.kind.worth();
        self.beat.kills += 1;
        self.beat.base += gain;
        if f.st == St::Latched {
            gain += 150;
            self.beat.saved += 1;
            self.say(f.on, Line::Thanks);
            if let Some(s) = self.seats.get_mut(&u32::from(by)) {
                s.craft.saved += 1;
            }
        }
        let mut chain = 0;
        if let Some(s) = self.seats.get_mut(&u32::from(by)) {
            let c = &mut s.craft;
            c.chain += 1;
            c.chain_ticks = 75;
            c.hits += 1;
            chain = c.chain;
            if c.chain >= 3 {
                gain += 10 * (u32::from(c.chain) - 2);
            }
            c.pts += gain;
        }
        self.beat.pts += gain;
        self.spark(Burst::Pop, f.s, f.x, f.z, chain.min(99) as u8);
        self.sound("kill");
        if chain == 5 {
            self.say(by, Line::Chain);
        }
    }

    /// A lamp's flame is out: points, a flash, and the Wyrm reels (the Wyrm's
    /// tick sees it).
    fn break_lamp(&mut self, fi: usize, by: u8) {
        let f = &mut self.foes[fi];
        f.st = St::Broken;
        f.hp = 0;
        let (s, x, z) = (f.s, f.x, f.z);
        let gain = Kind::Lamp.worth();
        self.beat.pts += gain;
        self.beat.base += gain;
        self.beat.kills += 1;
        if let Some(seat) = self.seats.get_mut(&u32::from(by)) {
            seat.craft.pts += gain;
            seat.craft.hits += 1;
        }
        self.spark(Burst::Lamp, s, x, z, 2);
        self.sound("boss_rage");
    }

    /// Starfall: every hostile bolt is cleared, and everything small on the
    /// way is struck for what it can bear.
    fn starfall(&mut self, by: u8, s_c: i32, _x: i32, _z: i32) {
        self.spark(Burst::Starfall, s_c, 0, 0, by);
        self.sound("bomb");
        self.bolts
            .retain(|b| !matches!(b.shot, Arm::Spark | Arm::Gloam));
        let anchor = self.anchor();
        let mut i = 0;
        while i < self.foes.len() {
            let f = &self.foes[i];
            let reach = (-dm(50)..=dm(600)).contains(&(f.s - anchor));
            if reach && f.lockable() && f.kind != Kind::Lamp {
                if !self.hit_foe(i, STARFALL, by) {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
    }

    /// The bolts' tick: they fly and home, then land.
    fn step_bolts(&mut self, n: &Nums) {
        let anchor = self.anchor();
        let count = self.bolts.len();
        type Seg = ((i32, i32, i32), (i32, i32, i32));
        let mut seg: Vec<Option<Seg>> = vec![None; count];
        let mut dead = vec![false; count];
        for i in 0..count {
            let mut b = self.bolts[i].clone();
            if b.delay > 0 {
                // A released starburst waits at its craft's muzzle for its turn.
                match self.craft_at(b.owner) {
                    Some((s, x, z, true)) => {
                        b.s = s + dm(12);
                        b.x = x;
                        b.z = z;
                        b.delay -= 1;
                    }
                    _ => dead[i] = true,
                }
                self.bolts[i] = b;
                continue;
            }
            // Homing: a starburst and a turned bolt close on their body.
            if b.aim != 0 && matches!(b.shot, Arm::Star | Arm::Turned) {
                let pick = self
                    .foes
                    .iter()
                    .find(|f| f.id == b.aim && f.alive())
                    .map(|f| (f.s, f.x, f.z));
                match pick {
                    Some(p) => {
                        let speed = if b.shot == Arm::Turned {
                            V_TURN
                        } else {
                            Session::star_speed()
                        };
                        let want = scale3((p.0 - b.s, p.1 - b.x, p.2 - b.z), speed);
                        b.vs += (want.0 - b.vs) / 3;
                        b.vx += (want.1 - b.vx) / 3;
                        b.vz += (want.2 - b.vz) / 3;
                    }
                    None => {
                        // Its body is gone: on to the nearest ahead.
                        b.aim = self
                            .foes
                            .iter()
                            .filter(|f| f.lockable() && f.kind != Kind::Lamp)
                            .filter(|f| (dm(20)..=dm(500)).contains(&(f.s - b.s)))
                            .min_by_key(|f| norm((f.s - b.s, f.x - b.x, f.z - b.z)))
                            .map_or(0, |f| f.id);
                    }
                }
            }
            let from = (b.s, b.x, b.z);
            b.s += b.vs;
            b.x += b.vx;
            b.z += b.vz;
            b.life = b.life.saturating_sub(1);
            seg[i] = Some((from, (b.s, b.x, b.z)));
            self.bolts[i] = b;
        }
        for i in 0..count {
            let Some((from, to)) = seg[i] else { continue };
            if dead[i] {
                continue;
            }
            let b = self.bolts[i].clone();
            let flies = match b.shot {
                Arm::Spark | Arm::Gloam => {
                    let mut turned = b;
                    let flies = self.hostile_bolt(&mut turned, from, to, n);
                    self.bolts[i] = turned;
                    flies
                }
                _ => self.friendly_bolt(&b, from, to, &mut dead),
            };
            if !flies {
                dead[i] = true;
            }
        }
        let mut k = 0;
        self.bolts.retain(|b| {
            let keep = !dead[k] && b.life > 0 && b.s >= anchor - dm(80) && b.s <= anchor + dm(1200);
            k += 1;
            keep
        });
    }

    /// A friendly bolt against the foes and the hostile sparks; whether it
    /// flies on.
    fn friendly_bolt(
        &mut self,
        b: &Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        dead: &mut [bool],
    ) -> bool {
        let mut best: Option<(usize, i64)> = None;
        for (i, f) in self.foes.iter().enumerate() {
            if !f.alive() || f.st == St::Hide {
                continue;
            }
            // A released starburst pops the body it was locked on, so a chain
            // of locks is a chain of pops.
            if b.shot == Arm::Star && b.aim != 0 && f.id != b.aim {
                continue;
            }
            if near(from, to, (f.s, f.x, f.z), f.radius() + BOLT_R) {
                let d = norm((f.s - from.0, f.x - from.1, f.z - from.2));
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        if let Some((i, _)) = best {
            self.land(i, b);
            return false;
        }
        // A bolt shoots a snuffer's spark out of the air.
        let hit = self.bolts.iter().enumerate().position(|(j, h)| {
            !dead[j] && h.shot == Arm::Spark && near(from, to, (h.s, h.x, h.z), SPARK_R + BOLT_R)
        });
        if let Some(j) = hit {
            dead[j] = true;
            let h = &self.bolts[j];
            let (s, x, z) = (h.s, h.x, h.z);
            self.spark(Burst::Hit, s, x, z, 0);
            return false;
        }
        true
    }

    /// A friendly bolt lands on foe `i`.
    fn land(&mut self, i: usize, b: &Bolt) {
        let (kind, st, id) = (self.foes[i].kind, self.foes[i].st, self.foes[i].id);
        let at = (self.foes[i].s, self.foes[i].x, self.foes[i].z);
        if kind == Kind::Lamp {
            let active = self
                .boss
                .as_ref()
                .is_some_and(|boss| boss.lamps[usize::from(boss.phase.min(2))] == id);
            match (b.shot, st) {
                // A turned bolt opens the active lamp's cold-light shield.
                (Arm::Turned, St::Shut) if active => {
                    self.foes[i].st = St::Open;
                    self.foes[i].ticks = OPEN;
                    self.spark(Burst::Lamp, at.0, at.1, at.2, 1);
                    self.sound("shield_up");
                    let who = b.owner;
                    self.say(who, Line::Open);
                }
                (Arm::Turned, St::Open) => {
                    self.foes[i].ticks = OPEN;
                }
                // Only a charged starburst breaks it open.
                (Arm::Star, St::Open) if b.full && active => {
                    self.hit_foe(i, u32::from(b.dmg), b.owner);
                    self.spark(Burst::Hit, at.0, at.1, at.2, 1);
                }
                _ => {
                    self.foes[i].flash = 4;
                    self.spark(Burst::Hit, at.0, at.1, at.2, 0);
                    self.sound("clang");
                }
            }
            return;
        }
        let dmg = if b.shot == Arm::Turned {
            u32::from(TURNED)
        } else {
            u32::from(b.dmg)
        };
        self.spark(Burst::Hit, at.0, at.1, at.2, 0);
        if self.hit_foe(i, dmg, b.owner) && b.shot == Arm::Star {
            // The chain of pops counts a starburst's own kills.
        }
    }

    /// A hostile bolt against the knights: a roll's shield turns it round, a
    /// bare craft takes it. Whether it flies on.
    fn hostile_bolt(
        &mut self,
        b: &mut Bolt,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        n: &Nums,
    ) -> bool {
        let r = if b.shot == Arm::Gloam {
            GLOAM_R
        } else {
            SPARK_R
        };
        let crafts: Vec<(u32, i32, i32, i32, bool, bool)> = self
            .seats
            .iter()
            .filter(|(_, s)| s.craft.flying())
            .map(|(&id, s)| {
                (
                    id,
                    self.craft_s(&s.craft),
                    s.craft.x,
                    s.craft.z,
                    s.craft.reflecting(n.reflect()),
                    s.craft.hurt > 0,
                )
            })
            .collect();
        for (id, s, x, z, reflecting, unhurt_for_now) in crafts {
            if reflecting && near(from, to, (s, x, z), REFLECT_R + r) {
                self.turn(b, id, (s, x, z));
                return true;
            }
            if !unhurt_for_now && near(from, to, (s, x, z), CRAFT_R + r) {
                self.hurt_craft(id as u8, i32::from(b.dmg));
                self.spark(Burst::Hit, b.s, b.x, b.z, 0);
                return false;
            }
        }
        true
    }

    /// A rolling craft's shield turns a bolt round: it is the roller's now,
    /// faster, and heads for what fired it (or the Wyrm's lit lamp).
    fn turn(&mut self, b: &mut Bolt, by: u32, at: (i32, i32, i32)) {
        let target = self
            .foes
            .iter()
            .find(|f| f.id == b.src && f.alive())
            .map(|f| f.id)
            .or_else(|| {
                self.boss.as_ref().and_then(|boss| {
                    let id = boss.lamps[usize::from(boss.phase.min(2))];
                    self.foes
                        .iter()
                        .find(|f| f.id == id && f.alive())
                        .map(|f| f.id)
                })
            })
            .or_else(|| {
                self.foes
                    .iter()
                    .filter(|f| f.lockable() && f.kind != Kind::Lamp && f.s > at.0)
                    .min_by_key(|f| norm((f.s - at.0, f.x - at.1, f.z - at.2)))
                    .map(|f| f.id)
            });
        b.shot = Arm::Turned;
        b.owner = by as u8;
        b.dmg = TURNED;
        b.life = 90;
        b.aim = target.unwrap_or(0);
        let v = target
            .and_then(|id| self.foes.iter().find(|f| f.id == id))
            .map_or((V_TURN, 0, 0), |f| {
                scale3((f.s - b.s, f.x - b.x, f.z - b.z), V_TURN)
            });
        (b.vs, b.vx, b.vz) = v;
        if let Some(s) = self.seats.get_mut(&by) {
            s.craft.pts += 25;
        }
        self.beat.pts += 25;
        self.spark(Burst::Turn, b.s, b.x, b.z, by as u8);
        self.sound("shield_block");
    }

    /// Run the events that are due.
    fn run_script(&mut self, _n: &Nums) {
        let script = script(self.stage);
        while let Some(ev) = script.get(usize::from(self.next)) {
            if ev.at > self.t {
                break;
            }
            self.next += 1;
            self.act(ev.act);
        }
    }

    /// A centred spread: the `k`th of `count`, in half steps.
    fn spread(k: i32, count: i32) -> i32 {
        2 * k - (count - 1)
    }

    fn act(&mut self, act: Act) {
        let anchor = self.anchor();
        let far = anchor + LEAD;
        let seats = i32::from(self.pressure());
        let origin = rail::ORIGIN[usize::from(self.stage.min(2))];
        match act {
            Act::Wisps {
                n,
                plus,
                shape,
                x,
                z,
                mode,
            } => {
                let count = i32::from(n) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    let sp = Session::spread(k, count);
                    let (dx, dz, ds, vx) = match shape {
                        0 => (sp * dm(16), 0, 0, 0),
                        1 => (sp * dm(13), -sp.abs() * dm(3), sp.abs() * dm(10), 0),
                        2 => (0, 0, k * dm(34), 0),
                        3 => {
                            let side = if k % 2 == 0 { 1 } else { -1 };
                            (side * dm(95), (k % 3) * dm(10), k * dm(22), -side * 44)
                        }
                        _ => (sp * dm(15), (k % 2) * dm(12), k * dm(16), 0),
                    };
                    self.spawn_foe(
                        Kind::Wisp,
                        (far + ds, dm(i32::from(x)) + dx, dm(i32::from(z)) + dz),
                        (-15, vx, 0),
                        St::Fly,
                        mode,
                        (k * 7) as u8,
                    );
                }
                if mode == 3 {
                    for f in self.foes.iter_mut().filter(|f| f.mode == 3 && f.ticks == 0) {
                        f.ticks = 60;
                    }
                }
            }
            Act::Convoy { n, plus, from, z } => {
                let count = i32::from(n) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    let side = i32::from(from);
                    self.spawn_foe(
                        Kind::Kite,
                        (far + k * dm(26), side * dm(95), dm(i32::from(z))),
                        (-15, -side * 46, 0),
                        St::Fly,
                        0,
                        (k * 5) as u8,
                    );
                }
            }
            Act::Hunter { side } => {
                self.spawn_foe(
                    Kind::Kite,
                    (far - dm(40), i32::from(side) * dm(90), dm(40)),
                    (0, 0, 0),
                    St::Hunt,
                    1,
                    0,
                );
            }
            Act::Turret { x, z } => {
                if let Some(id) = self.spawn_foe(
                    Kind::Turret,
                    (far, dm(i32::from(x)), dm(i32::from(z))),
                    (0, 0, 0),
                    St::Rest,
                    0,
                    (self.next % 5) as u8,
                ) {
                    if let Some(f) = self.foes.iter_mut().find(|f| f.id == id) {
                        f.ticks = 30;
                    }
                }
            }
            Act::Ridge { x, bombers, plus } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Ridge,
                        s: far,
                        x: dm(i32::from(x)),
                        z: dm(-30),
                        w: dm(30),
                        h: dm(55),
                        len: dm(60),
                        taken: 0,
                        phase: 0,
                    });
                }
                let count = i32::from(bombers) + (seats - 1) * i32::from(plus) / 2;
                for k in 0..count {
                    self.spawn_foe(
                        Kind::Bomber,
                        (
                            far + dm(10) + k * dm(8),
                            dm(i32::from(x)) + (k % 3 - 1) * dm(14),
                            dm(-6),
                        ),
                        (0, 0, 0),
                        St::Hide,
                        0,
                        (k * 3) as u8,
                    );
                }
            }
            Act::Gate { rel, x, z, bonus } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Gate,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(20),
                        h: 0,
                        len: 0,
                        taken: 0,
                        phase: u16::from(!bonus),
                    });
                    if bonus {
                        self.beat.offered += 100 * seats as u32;
                    }
                }
            }
            Act::Ring { rel, x, z } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Ring,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(22),
                        h: 0,
                        len: 0,
                        taken: 0,
                        phase: 0,
                    });
                }
            }
            Act::Arch { rel, x, z, w, h } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Arch,
                        s: origin + rel,
                        x: dm(i32::from(x)),
                        z: dm(i32::from(z)),
                        w: dm(i32::from(w)),
                        h: dm(i32::from(h)),
                        len: dm(30),
                        taken: 0,
                        phase: 0,
                    });
                }
            }
            Act::Shutter { rel, phase } => {
                if self.props.len() < MAX_PROPS {
                    self.props.push(Prop {
                        thing: Thing::Shutter,
                        s: origin + rel,
                        x: 0,
                        z: 0,
                        w: 0,
                        h: 0,
                        len: dm(30),
                        taken: 0,
                        phase,
                    });
                }
            }
            Act::Say(line) => {
                let who = self.voice();
                self.say(who, line);
            }
            Act::Beast => self.rise(),
        }
    }

    /// The warm fly-through after the Wyrm: a procession of gold gates and
    /// hearth rings, lit for the way home. It pays nothing and offers
    /// nothing; it is the party's welcome.
    fn victory_gates(&mut self) {
        let origin = self.anchor() + dm(150);
        for k in 0..9 {
            let (x, z) = (
                tri(k as u32 * 40, 120, dm(30)),
                dm(30) + tri(k as u32 * 25, 90, dm(18)),
            );
            if self.props.len() < MAX_PROPS {
                self.props.push(Prop {
                    thing: if k % 3 == 2 { Thing::Ring } else { Thing::Gate },
                    s: origin + k * dm(75),
                    x,
                    z,
                    w: dm(24),
                    h: 0,
                    len: 0,
                    taken: 0,
                    phase: 1,
                });
            }
        }
    }

    /// The Wyrm rises out of the roost (or, after a fall, is back in its
    /// place, its lamps as they were).
    fn rise(&mut self) {
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
    fn step_boss(&mut self, n: &Nums) {
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
                        self.act(Act::Hunter { side });
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

    /// Props and the knights who cross them.
    fn step_props(&mut self, n: &Nums) {
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

    /// What knight `id` presses this tick, for the numbers `n` stand at.
    fn bot_native(&self, id: u32, n: &Nums, skill: Skill) -> Input {
        let novice = skill == Skill::Novice;
        let Some(seat) = self.seats.get(&id) else {
            return Input::default();
        };
        let c = &seat.craft;
        if self.mode == Mode::Tally {
            // A fresh press now and then leaves.
            return Input {
                interact: self.mode_t % 2 == 0,
                ..Input::default()
            };
        }
        if self.mode == Mode::Retry || c.down > 0 {
            return Input::default();
        }
        let s_c = self.craft_s(c);
        let mut input = Input::default();
        // A kite on this knight's own tail: roll it off.
        let on_me = self.latched(id as u8);

        // Where to be: round what is coming, through a gate, away from a
        // footprint, to the middle of an opening.
        let mut want: Option<(i32, i32)> = None;
        let mut rolls = on_me && c.rest == 0 && c.roll == 0;
        let mut brake = false;
        // A friend with a kite on their tail, level with this knight: drop
        // back until the chaser is in front, and shoot it.
        if let Some(chaser) = self
            .foes
            .iter()
            .find(|f| f.st == St::Latched && f.on <= 6 && u32::from(f.on) != id && f.on != 0)
        {
            if chaser.s - s_c < dm(30) {
                brake = true;
            }
        }
        let threat = self.threat(c, s_c);
        let seek = self.reflector(id).then(|| self.intercept(s_c)).flatten();
        if let Some((t_hit, px, pz)) = seek {
            // The Wyrm's lamp is shut: be where its spit will be, and roll.
            want = Some((px, pz));
            // A novice meets the Wyrm's spit half the time.
            let willing = !novice || noise(id, self.t / 60, 5, 1) >= 0;
            rolls = willing && c.rest == 0 && c.roll == 0 && t_hit <= if novice { 6 } else { 8 };
        } else if let Some((t_hit, px, pz, gloam)) = threat {
            let reflect = c.rest == 0 && c.roll == 0;
            // The even seats roll at a Gloam bolt; the odd ones dodge it
            // unless it is on them, or no even seat is there to roll.
            let eager = self.reflector(id) || t_hit <= 6;
            let slip = novice && noise(id, self.t / 45, 9, 2) == 0;
            if reflect && gloam && eager && t_hit <= if novice { 6 } else { 9 } && !slip {
                rolls = true;
            } else if rolls {
                // Already rolling a kite off.
            } else if t_hit <= if novice { 22 } else { 40 } {
                want = Some(Session::room_from(c, px, pz, &n));
            }
        }
        if want.is_none() {
            want = self.footprint_away(c, &n);
        }
        if want.is_none() {
            want = self.boss_corridor(c, &n);
        }
        if want.is_none() {
            // The next thing on the rail to thread or take.
            let ahead = |p: &Prop| (0..=dm(300)).contains(&(p.s - s_c));
            for p in self.props.iter().filter(|p| ahead(p)) {
                match p.thing {
                    Thing::Arch if (p.s - s_c) < dm(260) => {
                        want = Some((p.x, p.z));
                        break;
                    }
                    Thing::Gate | Thing::Ring
                        if p.taken & bit(id) == 0 && (p.thing == Thing::Gate || c.shield < 80) =>
                    {
                        want = Some((p.x, p.z));
                        break;
                    }
                    Thing::Shutter if (p.s - s_c) < dm(220) => {
                        want = Some((p.x, c.z));
                        let ticks = ((p.s - s_c) / PACE).max(0) as u32;
                        if shutter_gap(p, self.t + ticks, n.wx()) < dm(16) {
                            brake = true;
                        }
                        break;
                    }
                    Thing::Ridge if (p.x - c.x).abs() < p.w + dm(16) && (p.s - s_c) < dm(200) => {
                        want = Some((-p.x.signum() * dm(20), c.z));
                        break;
                    }
                    _ => {}
                }
            }
        }
        let (tx, tz) = want.unwrap_or((0, 0));
        let steer = |from: i32, to: i32, dead: i32| -> i8 {
            if (to - from).abs() <= dead {
                0
            } else {
                (to - from).signum() as i8
            }
        };
        input.move_x = steer(c.x, tx, dm(8));
        input.move_y = -steer(c.z, tz, dm(8));
        input.swing = brake;
        if rolls && on_me && threat.is_none() {
            input.dash = c.keys & K_DASH == 0;
            return input;
        }
        if rolls {
            input.dash = c.keys & K_DASH == 0;
            input.move_x = if threat.is_some_and(|t| t.1 >= c.x) {
                -1
            } else {
                1
            };
            return input;
        }

        // What to shoot: a kite on a friend's tail, a lamp that is open, a
        // flight to sweep, the nearest foe.
        let lockable: Vec<&Foe> = self
            .foes
            .iter()
            .filter(|f| {
                let near = if f.st == St::Latched { dm(15) } else { dm(40) };
                f.lockable() && (near..=dm(480)).contains(&(f.s - s_c))
            })
            .collect();
        let lamp_open = lockable.iter().find(|f| f.kind == Kind::Lamp);
        let latched = lockable
            .iter()
            .filter(|f| f.st == St::Latched)
            .min_by_key(|f| f.s);
        let fliers: Vec<&&Foe> = lockable.iter().filter(|f| f.kind != Kind::Lamp).collect();
        let cluster = fliers.len() >= 3;
        let held = c.held > HOLD_AT_BOT;
        let in_locks = |f: &Foe| c.locks.iter().any(|l| l.foe == f.id);
        // The target the reticle seeks.
        let target: Option<&Foe> = if let Some(f) = latched {
            Some(*f)
        } else if let Some(f) = lamp_open {
            Some(*f)
        } else if held {
            // Sweeping: on to the nearest one not yet locked.
            fliers
                .iter()
                .map(|f| **f)
                .filter(|f| !in_locks(f))
                .min_by_key(|f| (f.x - c.x).abs() + (f.z - c.z).abs())
        } else {
            fliers.iter().map(|f| **f).min_by_key(|f| (f.s - s_c, f.id))
        };
        if let Some(f) = target {
            let ds = (f.s - s_c).max(dm(30));
            // A novice's aim wanders a little (up to a unit and a half).
            let (jx, jz) = if novice {
                (
                    noise(id, self.t / 14, 1, dm(15)),
                    noise(id, self.t / 14, 2, dm(10)),
                )
            } else {
                (0, 0)
            };
            let rx = ((i64::from(f.x - c.x + jx) * i64::from(FAR) / i64::from(ds)) as i32)
                .clamp(-RX_MAX, RX_MAX);
            let rz = ((i64::from(f.z - c.z + jz) * i64::from(FAR) / i64::from(ds)) as i32)
                .clamp(-RZ_MAX, RZ_MAX);
            let dead = RET_V * 3 / 4;
            input.aim_x = steer(c.rx, rx, dead);
            input.aim_y = -steer(c.rz, rz, dead);
        } else {
            // Nothing to aim at: the reticle comes home.
            input.aim_x = steer(c.rx, 0, RET_V);
            input.aim_y = -steer(c.rz, 0, RET_V);
        }
        // The hands on the fire key.
        let total: usize = c.locks.iter().map(|l| usize::from(l.n)).sum();
        let want_locks = if lamp_open.is_some() && latched.is_none() {
            MAX_PER_BODY as usize
        } else {
            fliers.len().min(MAX_LOCKS)
        };
        if held {
            // Hold until the locks are in or the charge is full, then let go.
            // A lamp wants the whole charge; a flight, whatever is in hand.
            // A novice lets go early now and then, a lamp's charge unfinished.
            let early = novice && noise(id, self.t / 90, 3, 2) == 0;
            let done = if lamp_open.is_some() && latched.is_none() {
                (c.charge >= n.charge && total >= 2) || (early && c.charge >= n.charge / 2)
            } else {
                (total >= want_locks && c.charge >= n.charge / 2) || c.charge >= n.charge
            };
            input.fire = !done;
        } else if c.held > 0 {
            input.fire = true;
        } else if lamp_open.is_some() || cluster {
            input.fire = c.keys & K_FIRE == 0;
        } else if target.is_some() {
            // A tap: down on one tick and up on the next.
            input.fire = c.keys & K_FIRE == 0;
        }
        // Starfall when worn down in a crowded sky.
        let crowd = fliers.iter().filter(|f| (f.s - s_c) < dm(260)).count();
        if c.shield <= 35 && crowd >= 3 && c.bombs > 0 && c.keys & K_BOMB == 0 {
            input.bomb = true;
        }
        input
    }

    /// Whether this knight is one of the rollers: the even seats, and any
    /// knight at all when no even seat is flying.
    fn reflector(&self, id: u32) -> bool {
        id % 2 == 0
            || !self
                .seats
                .iter()
                .any(|(&k, s)| k % 2 == 0 && s.craft.flying())
    }

    /// While the lit lamp is shut, the soonest of the Wyrm's spits that is
    /// owed to it: ticks to it and where it will cross.
    fn intercept(&self, s_c: i32) -> Option<(u32, i32, i32)> {
        let boss = self.boss.as_ref()?;
        let lit = boss.lamps[usize::from(boss.phase.min(2))];
        if !self.foes.iter().any(|f| f.id == lit && f.st == St::Shut) {
            return None;
        }
        self.bolts
            .iter()
            .filter(|b| b.shot == Arm::Gloam && b.src == lit && b.s > s_c)
            .map(|b| {
                let t = ((b.s - s_c) / (PACE - b.vs).max(1)).max(0);
                (t as u32, b.x + b.vx * t, b.z + b.vz * t)
            })
            .min_by_key(|x| x.0)
    }

    /// The soonest hostile bolt that will pass near this craft's plane:
    /// ticks to it, where it will cross, and whether it is a Gloam bolt.
    fn threat(&self, c: &Craft, s_c: i32) -> Option<(u32, i32, i32, bool)> {
        let mut best: Option<(u32, i32, i32, bool)> = None;
        for b in &self.bolts {
            if !matches!(b.shot, Arm::Spark | Arm::Gloam) {
                continue;
            }
            let gap = b.s - s_c;
            let closing = (PACE - b.vs).max(1);
            if gap <= 0 || gap > dm(220) {
                continue;
            }
            let t = (gap / closing).max(0);
            let (px, pz) = (b.x + b.vx * t, b.z + b.vz * t);
            if flat(px, pz, c.x, c.z) < i64::from(dm(26)) && best.is_none_or(|x| (t as u32) < x.0) {
                best = Some((t as u32, px, pz, b.shot == Arm::Gloam));
            }
        }
        best
    }

    /// Where to go to be clear of the point `(ox, oz)`: four ways out of it
    /// (left, right, up, down), those the steer box allows, the one that ends
    /// farthest from it, so a knight at the edge goes up or down and not into
    /// the wall.
    fn room_from(c: &Craft, ox: i32, oz: i32, n: &Nums) -> (i32, i32) {
        let (wx, wz) = (n.wx(), n.wz());
        [(dm(40), 0), (dm(-40), 0), (0, dm(30)), (0, dm(-30))]
            .into_iter()
            .map(|(dx, dz)| ((c.x + dx).clamp(-wx, wx), (c.z + dz).clamp(-wz, wz)))
            .max_by_key(|&(x, z)| flat(x, z, ox, oz))
            .unwrap_or((c.x, c.z))
    }

    /// A footprint locked on this craft (a turret's, a bomber's lane): the
    /// place to be instead.
    fn footprint_away(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)> {
        for f in &self.foes {
            if f.st == St::Tell && matches!(f.kind, Kind::Turret | Kind::Bomber | Kind::Wisp) {
                for (mx, mz) in Session::marks(f, self.pressure()) {
                    if flat(c.x, c.z, mx, mz) < i64::from(dm(24)) {
                        return Some(Session::room_from(c, mx, mz, n));
                    }
                }
            }
        }
        None
    }

    /// The Wyrm's coil wants the gap, its roar wants the column left.
    fn boss_corridor(&self, c: &Craft, n: &Nums) -> Option<(i32, i32)> {
        let b = self.boss.as_ref()?;
        if !matches!(b.st, Wyrm::Tell | Wyrm::Coil | Wyrm::Roar) {
            return None;
        }
        match b.atk {
            1 => Some((b.gap, c.z)),
            2 => {
                if (c.x - b.col).abs() < COL_HALF + dm(16) {
                    let away = if c.x >= b.col { 1 } else { -1 };
                    let to = (b.col + away * (COL_HALF + dm(30))).clamp(-n.wx(), n.wx());
                    Some((to, c.z))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// The keys, a bit each, for the presses (a key held across a tick is not a
/// new press).
pub const K_FIRE: u8 = 1;

pub const K_DASH: u8 = 2;

pub const K_BOMB: u8 = 4;

pub const K_SPUR: u8 = 8;

pub fn bits(i: Input) -> u8 {
    u8::from(i.fire)
        | u8::from(i.dash) << 1
        | u8::from(i.bomb) << 2
        | u8::from(i.interact) << 3
        | u8::from(i.swing) << 4
}

/// How fast a craft steers, across and up, and how quickly it gets there
/// (units a tick, in Q).
const V_X: i32 = 62;

const V_Z: i32 = 50;

const ACC: i32 = 14;

/// The reticle's pace, and how far it ranges from its craft at the far plane.
pub const RET_V: i32 = 100;

pub const RX_MAX: i32 = dm(120);

pub const RZ_MAX: i32 = dm(80);

/// A tap's rest, how long F is held before it is a charge, the rest after a
/// release.
const TAP_REST: u32 = 6;

const HOLD_AT: u32 = 7;

const REL_REST: u32 = 12;

/// How near the reticle's ray must pass a body to lock it, beyond its size.
pub const LOCK_R: i32 = dm(16);

/// The bolts' pace along their way (relative to the rail) and how long they
/// fly.
const V_BOLT: i32 = dm(22);

const V_STAR: i32 = dm(26);

const BOLT_LIFE: u16 = 26;

const STAR_LIFE: u16 = 46;

/// What a bolt and a starburst do: a starburst's least and its most (a full
/// charge).
pub const LANCE: u16 = 10;

pub const STAR_MIN: u32 = 16;

pub const STAR_MAX: u32 = 36;

/// The spur and the brake: how far they slide a craft along the rail, and
/// how quickly.
const BOOST_BACK: i32 = dm(55);

const BRAKE_BACK: i32 = dm(45);

const BACK_V: i32 = 28;

/// A roll's rest after it ends, and how hard it throws a craft aside.
const ROLL_REST: u32 = 12;

const ROLL_V: i32 = 84;

/// The window in which a second tap is a roll.
const TAP_WINDOW: u8 = 9;

/// The craft's body, for the bolts that hit it.
pub const CRAFT_R: i32 = dm(10);

/// How far from a craft a rolling shield turns a bolt round.
pub const REFLECT_R: i32 = dm(26);

/// What Starfall does to every foe it reaches.
pub const STARFALL: u32 = 120;

/// How a kite closes on its victim, and how far behind the craft it hangs.
const HUNT_V: i32 = dm(10);

const LATCH_BACK: i32 = dm(18);

/// What a hit of each hostile bolt takes off a shield, and how big they are.
const GLOAM: i32 = 25;

const SPARK: i32 = 10;

const GLOAM_R: i32 = dm(6);

const SPARK_R: i32 = dm(5);

const BOLT_R: i32 = dm(5);

/// What a turned bolt does, and its pace home.
pub const TURNED: u16 = 60;

const V_TURN: i32 = dm(24);

/// A dive-bomber's blow.
const RAM: i32 = 35;

/// How long a shield stays down after a hit lands on it, in ticks.
pub const HURT: u32 = 36;

/// How many ticks the rail takes to carry a thing from where it appears to
/// the formation.
pub const LEAD_TICKS: u32 = (LEAD / PACE) as u32;

/// One thing the script does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// A flight of snuffers: how many, how many more per two seats, the
    /// shape (0 line, 1 vee, 2 file, 3 crossing, 4 weaving), where, and how
    /// they fly (0 drift, 1 weave, 3 shoot).
    Wisps {
        n: u8,
        plus: u8,
        shape: u8,
        x: i16,
        z: i16,
        mode: u8,
    },
    /// Kites crossing in a file from one side (-1 left, 1 right).
    Convoy {
        n: u8,
        plus: u8,
        from: i8,
        z: i16,
    },
    /// A kite that hunts a wingman and latches onto its tail.
    Hunter {
        side: i8,
    },
    /// A lantern tower on a ledge.
    Turret {
        x: i16,
        z: i16,
    },
    /// A ridge with dive-bombers waiting behind it.
    Ridge {
        x: i16,
        bombers: u8,
        plus: u8,
    },
    /// A bonus gate, a hearth ring, a rock arch, a pair of shutters: where
    /// the rail brings them (`rel` from the beat's first point).
    Gate {
        rel: i32,
        x: i16,
        z: i16,
        bonus: bool,
    },
    Ring {
        rel: i32,
        x: i16,
        z: i16,
    },
    Arch {
        rel: i32,
        x: i16,
        z: i16,
        w: i16,
        h: i16,
    },
    Shutter {
        rel: i32,
        phase: u16,
    },
    /// The wing's lead shouts.
    Say(Line),
    /// The Wyrm rises.
    Beast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ev {
    pub at: u32,
    pub act: Act,
}

fn arrive(t: u32) -> i32 {
    t as i32 * PACE
}

struct Beat(Vec<Ev>);

impl Beat {
    fn at(&mut self, at: u32, act: Act) {
        self.0.push(Ev { at, act });
    }
    /// A prop that arrives at the formation on tick `t`.
    fn prop(&mut self, t: u32, make: impl Fn(i32) -> Act) {
        self.0.push(Ev {
            at: t.saturating_sub(LEAD_TICKS),
            act: make(arrive(t)),
        });
    }
    fn gate(&mut self, t: u32, x: i16, z: i16) {
        self.prop(t, |rel| Act::Gate {
            rel,
            x,
            z,
            bonus: true,
        });
    }
    fn ring(&mut self, t: u32, x: i16, z: i16) {
        self.prop(t, |rel| Act::Ring { rel, x, z });
    }
    fn arch(&mut self, t: u32, x: i16, z: i16, w: i16, h: i16) {
        self.prop(t, |rel| Act::Arch { rel, x, z, w, h });
    }
    fn shutter(&mut self, t: u32, phase: u16) {
        self.prop(t, |rel| Act::Shutter { rel, phase });
    }
    fn say(&mut self, at: u32, line: Line) {
        self.at(at, Act::Say(line));
    }
    fn wisps(&mut self, at: u32, n: u8, plus: u8, shape: u8, x: i16, z: i16, mode: u8) {
        self.at(
            at,
            Act::Wisps {
                n,
                plus,
                shape,
                x,
                z,
                mode,
            },
        );
    }
    fn done(mut self) -> Vec<Ev> {
        self.0.sort_by_key(|e| e.at);
        self.0
    }
}

/// Thunderhead Run: aim, then hold and let go, the first gates, a rock arch,
/// and the first kite that latches on.
fn thunderhead() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Hello);
    b.wisps(110, 3, 1, 0, 0, 20, 0);
    b.gate(330, -26, 30);
    b.gate(345, 0, 38);
    b.gate(360, 26, 30);
    b.wisps(330, 5, 2, 1, 0, 25, 0);
    b.say(350, Line::Hold);
    b.ring(700, -30, 40);
    b.ring(700, 30, 40);
    b.say(560, Line::Ring);
    b.arch(860, 0, 20, 24, 15);
    b.say(700, Line::Arch);
    b.wisps(800, 4, 2, 3, 0, 15, 0);
    b.wisps(850, 4, 2, 3, 0, 30, 0);
    b.at(1010, Act::Hunter { side: -1 });
    b.wisps(1180, 6, 2, 4, 0, 20, 1);
    b.wisps(1240, 6, 2, 4, 0, 32, 1);
    b.wisps(1100, 4, 2, 0, 0, 25, 3);
    b.wisps(560, 4, 2, 2, 0, 20, 0);
    b.gate(1500, 0, 36);
    b.gate(1515, 30, 15);
    b.gate(1530, 0, -10);
    b.gate(1545, -30, 15);
    b.gate(1560, 0, 36);
    b.at(1380, Act::Hunter { side: 1 });
    b.wisps(1400, 3, 2, 0, 0, 25, 0);
    b.wisps(1560, 3, 1, 0, 0, 25, 3);
    b.at(1500, Act::Hunter { side: -1 });
    b.ring(1700, 0, 40);
    b.done()
}

/// The Ruined Causeway: turrets teach the roll, kite convoys pay off the
/// lock, dive-bombers come from behind a ridge, shutters close on a clock.
fn causeway() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Ruins);
    b.say(80, Line::Turret);
    b.at(90, Act::Turret { x: -95, z: -20 });
    b.say(150, Line::Roll);
    b.at(330, Act::Turret { x: 95, z: -10 });
    b.at(345, Act::Turret { x: -100, z: 0 });
    b.say(500, Line::Convoy);
    b.at(
        520,
        Act::Convoy {
            n: 6,
            plus: 2,
            from: 1,
            z: 25,
        },
    );
    b.gate(800, -26, 25);
    b.gate(815, 0, 36);
    b.gate(830, 26, 25);
    b.arch(900, -10, 0, 22, 14);
    b.say(750, Line::Bombers);
    b.at(
        760,
        Act::Ridge {
            x: -70,
            bombers: 2,
            plus: 2,
        },
    );
    b.say(980, Line::Shutter);
    b.shutter(1190, 0);
    b.shutter(1390, 75);
    b.at(
        1150,
        Act::Convoy {
            n: 5,
            plus: 2,
            from: -1,
            z: 15,
        },
    );
    b.at(1170, Act::Turret { x: 95, z: -20 });
    b.wisps(620, 4, 2, 0, 0, 25, 3);
    b.wisps(1300, 5, 2, 1, 0, 25, 3);
    b.at(1650, Act::Turret { x: -100, z: -10 });
    b.at(1670, Act::Turret { x: 100, z: -15 });
    b.at(
        1400,
        Act::Ridge {
            x: 70,
            bombers: 2,
            plus: 2,
        },
    );
    b.at(
        1450,
        Act::Ridge {
            x: -70,
            bombers: 1,
            plus: 2,
        },
    );
    b.ring(1750, -30, 30);
    b.ring(1750, 30, 30);
    b.at(1780, Act::Hunter { side: 1 });
    b.at(1850, Act::Hunter { side: -1 });
    b.at(1990, Act::Hunter { side: 1 });
    b.at(2050, Act::Hunter { side: -1 });
    b.wisps(1900, 4, 2, 0, 0, 28, 3);
    b.gate(2250, 0, 36);
    b.gate(2265, -30, 18);
    b.gate(2280, 0, 0);
    b.gate(2295, 30, 18);
    b.done()
}

/// The Wyrm's Roost: a short approach, then the Wyrm.
fn roost() -> Vec<Ev> {
    let mut b = Beat(Vec::new());
    b.say(30, Line::Roost);
    b.wisps(110, 4, 2, 0, 0, 25, 3);
    b.at(250, Act::Hunter { side: -1 });
    b.at(280, Act::Hunter { side: 1 });
    b.gate(520, -20, 25);
    b.ring(500, 0, 30);
    b.at(rail::BOSS_AT, Act::Beast);
    b.done()
}

/// The scripts, built once.
pub fn script(stage: u8) -> &'static [Ev] {
    static SCRIPTS: OnceLock<[Vec<Ev>; 3]> = OnceLock::new();
    &SCRIPTS.get_or_init(|| [thunderhead(), causeway(), roost()])[usize::from(stage.min(2))]
}

/// Where a beat's script stands when it comes back to the tick `cp`.
pub fn resume(stage: u8, cp: u32) -> u16 {
    script(stage).iter().take_while(|e| e.at < cp).count() as u16
}

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

/// A steady scatter in `-span..=span` from a knight, a tick and a salt.
fn noise(id: u32, t: u32, salt: u32, span: i32) -> i32 {
    let mut h =
        id.wrapping_mul(0x9E37_79B1) ^ t.wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    (h % (2 * span as u32 + 1)) as i32 - span
}

/// How long F is held before it is a charge (the flight's own number).
const HOLD_AT_BOT: u32 = 7;

/// A whole-number length of the vector `d`, rounded down.
pub fn norm(d: (i32, i32, i32)) -> i64 {
    let n = i64::from(d.0).pow(2) + i64::from(d.1).pow(2) + i64::from(d.2).pow(2);
    // A correctly rounded square root, the same on every machine.
    (n as f64).sqrt() as i64
}

/// `d` turned to a length of `len`, along the same way (straight ahead for
/// a zero vector).
pub fn scale3(d: (i32, i32, i32), len: i32) -> (i32, i32, i32) {
    let m = norm(d);
    if m == 0 {
        return (len, 0, 0);
    }
    let f = |v: i32| (i64::from(v) * i64::from(len) / m) as i32;
    (f(d.0), f(d.1), f(d.2))
}

/// A triangle wave through `-amp..=amp` over `period` ticks.
pub fn tri(t: u32, period: u32, amp: i32) -> i32 {
    let p = period.max(2);
    let k = (t % p) as i64;
    let half = i64::from(p / 2).max(1);
    let up = if k < half { k } else { i64::from(p) - k };
    ((up * 2 - half) * i64::from(amp) / half) as i32
}

/// Whether the segment from `a` to `b` passes within `r` of `p`: the swept
/// test every bolt, dive and latch is judged by (no free 3D collision, only
/// this, in rail space).
pub fn near(a: (i32, i32, i32), b: (i32, i32, i32), p: (i32, i32, i32), r: i32) -> bool {
    let d = (
        i64::from(b.0) - i64::from(a.0),
        i64::from(b.1) - i64::from(a.1),
        i64::from(b.2) - i64::from(a.2),
    );
    let w = (
        i64::from(p.0) - i64::from(a.0),
        i64::from(p.1) - i64::from(a.1),
        i64::from(p.2) - i64::from(a.2),
    );
    let len2 = d.0 * d.0 + d.1 * d.1 + d.2 * d.2;
    let t = if len2 == 0 {
        0
    } else {
        (w.0 * d.0 + w.1 * d.1 + w.2 * d.2).clamp(0, len2)
    };
    let c = |dv: i64, wv: i64| {
        if len2 == 0 { wv } else { wv - dv * t / len2 }
    };
    let (ex, ey, ez) = (c(d.0, w.0), c(d.1, w.1), c(d.2, w.2));
    ex * ex + ey * ey + ez * ez <= i64::from(r) * i64::from(r)
}

/// How far two points are apart across and up (the plane's own distance).
pub fn flat(ax: i32, az: i32, bx: i32, bz: i32) -> i64 {
    norm((0, ax - bx, az - bz))
}

/// What the module is asked, as `Session::step` and `Session::bot_with` ask it.
#[derive(serde::Deserialize)]
struct Ask {
    op: String,
    session: Session,
    #[serde(default)]
    keys: BTreeMap<u32, Input>,
    #[serde(default)]
    here: Vec<u32>,
    nums: Nums,
    #[serde(default)]
    id: u32,
    #[serde(default)]
    novice: bool,
}

/// The module's answer to `input`, from the rules above.
pub fn answer(_id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut ask: Ask = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
    match ask.op.as_str() {
        "tick" => {
            let out = ask.session.step_here(&ask.keys, &ask.here, &ask.nums);
            let sounds: Vec<usize> = out
                .sounds
                .iter()
                .filter_map(|n| SOUNDS.split(' ').position(|s| s == *n))
                .collect();
            Ok(serde_json::json!({"session": ask.session, "sounds": sounds, "leave": out.leave}))
        }
        "bot" => {
            let skill = if ask.novice {
                Skill::Novice
            } else {
                Skill::Sure
            };
            Ok(
                serde_json::to_value(ask.session.bot_native(ask.id, &ask.nums, skill))
                    .map_err(|e| e.to_string())?,
            )
        }
        op => Err(format!("griffinwing-rules: no op {op}")),
    }
}

#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/tests/rules/griffinwing_boss_bend_cases__tests.rs"]
pub mod bend_boss_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/src/rules/griffinwing/recorder/bot_bend.rs"]
pub mod bend_bot_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/tests/rules/griffinwing_combat_bend_cases__tests.rs"]
pub mod bend_combat_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/src/rules/griffinwing/recorder/flight_bend.rs"]
pub mod bend_flight_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/tests/rules/griffinwing_script_bend_cases__tests.rs"]
pub mod bend_script_tests;
#[cfg(all(test, feature = "bend-cases-games-a-griffinwing"))]
#[path = "../../../crates/delve/src/rules/griffinwing/recorder/things_bend.rs"]
pub mod bend_things_tests;
