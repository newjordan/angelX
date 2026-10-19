//! What a bot presses: it reads the road ahead the way a player does (a
//! bramble to clear, a snuffer to roll through, a loop to take at speed, a
//! line of foes to chain, a gate to break) and presses the same keys.
use super::level::level;
use super::math::Q;
use super::rider::{Keys, Numbers, Rider};
use super::session::Session;
use super::terrain::Kind;
use super::world::*;
use crate::rules::Input;

/// Which way through the fork a bot rides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The high rails: fast and risky.
    Upper,
    /// The valley road: safe and slower.
    Lower,
}

impl Route {
    /// The shell's bots: odd seats ride high, even seats low.
    pub fn of_seat(id: u32) -> Route {
        if id % 2 == 1 { Route::Upper } else { Route::Lower }
    }
}

/// How well a bot rides: the tuning rounds play a quick, tidy rider and a
/// first-timer who hesitates, hops late and rarely reaches for the spur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skill {
    /// Uses the spur whenever the gauge is full (and into loops and forks).
    pub spur: bool,
    /// Tucks into a roll to run a snuffer down.
    pub roll: bool,
    /// Pixels late on every hop over a bramble.
    pub lag: i32,
    /// Lifts off the pace for a few ticks every this many ticks (0: never).
    pub hesitate: u32,
}

impl Skill {
    pub const ACE: Skill = Skill { spur: true, roll: true, lag: 0, hesitate: 0 };
    pub const FIRST_RIDE: Skill = Skill { spur: false, roll: false, lag: 30, hesitate: 240 };
    /// A careful rider: a quarter of the ride spent easing off.
    pub const SLOW: Skill = Skill { spur: false, roll: false, lag: 40, hesitate: 60 };
}

impl Session {
    /// What a bot presses for knight `id`.
    pub fn bot_input(&self, id: u32, _hero: &crate::rules::Hero) -> Input {
        self.drive(id, Route::of_seat(id), &Numbers::default())
    }

    /// The keys for knight `id` taking `route` through the fork, riding well.
    pub fn drive(&self, id: u32, route: Route, n: &Numbers) -> Input {
        self.drive_as(id, route, n, Skill::ACE)
    }

    /// The same, riding as `skill` says.
    pub fn drive_as(&self, id: u32, route: Route, n: &Numbers, skill: Skill) -> Input {
        let Some(seat) = self.seats.get(&id) else { return Input::default() };
        let r = &seat.rider;
        if self.tally.is_some() || r.down > 0 || r.finish.is_some() {
            return Input::default();
        }
        let mut k = self.think(r, route, n, skill);
        // A first-timer lifts off the pace now and then.
        if skill.hesitate > 0 && (self.elapsed + id * 37) % skill.hesitate < 14 && r.grounded() {
            k.x = 0;
            k.spur = false;
        }
        Input {
            move_x: k.x as i8,
            move_y: i8::from(k.down),
            interact: k.jump,
            dash: k.spur,
            ..Input::default()
        }
    }

    fn think(&self, r: &Rider, route: Route, n: &Numbers, skill: Skill) -> Keys {
        let lvl = level();
        let t = &lvl.terrain;
        let (x, y) = (r.x / Q, r.y / Q);
        let speed = (r.speed() / Q).max(2);
        let mut k = Keys { x: 1, ..Keys::default() };
        let airborne = !r.grounded();
        let on_rail = r.ground.is_some_and(|g| t.paths[g.path as usize].kind == Kind::Rail);
        let fork = lvl.terrain.paths[lvl.main].portals.iter().find(|p| p.min_speed > 0).map(|p| {
            t.paths[lvl.main].sample(p.s).x / Q
        });

        // Hold the leap while rising.
        if r.jumping && r.vy < 0 {
            k.jump = true;
        }

        // Stuck at the foot of a wall: crouch and rev a charge, then go.
        let stuck = r.grounded() && r.gsp.abs() < 160 && !r.roll;
        if r.crouch || (stuck && self.elapsed > 90 && self.elapsed % 3 == 0 && r.charge == 0 && r.coyote > 0 && x > 40 && stalled(r)) {
            k.down = r.charge < 1100;
            k.jump = r.charge < 1100 && !r.jump_key && self.elapsed % 2 == 0;
            k.x = r.dir;
            return k;
        }

        // Things ahead.
        let reach = (speed * 22 + 60).min(420);
        let mut jump_for: Option<i32> = None;
        let mut roll_at = false;
        let mut chain = false;
        let mut warden: Option<i32> = None;
        for i in Level::near(&lvl.foes, x + reach / 2, reach / 2 + 40, |f| f.x) {
            let spot = &lvl.foes[i];
            if spot.min_seats > self.provisioned || self.foes[i].hp == 0 {
                continue;
            }
            let (fx, fy, _) = foe_at(lvl, spot, self.elapsed);
            let dx = fx - x;
            if dx < -4 {
                continue;
            }
            let (_, fh) = foe_body(spot.kind);
            let same_level = (fy - y).abs() < 70 || (fy - fh - y).abs() < 70;
            match spot.kind {
                FoeKind::Bramble if same_level && dx > 0 && dx <= speed * 16 => {
                    // Clear it: take off about fifteen ticks before it.
                    if dx >= speed * 10 - 6 - skill.lag {
                        jump_for = Some(dx);
                    } else if airborne || dx > speed * 4 {
                        k.jump = k.jump || (r.jumping && r.vy < 0);
                    }
                }
                FoeKind::Snuffer if same_level && (24..=110).contains(&dx) && r.speed() > 300 => roll_at = skill.roll,
                FoeKind::Floater if dx > 0 && dx < reach => chain = true,
                FoeKind::Warden => warden = Some(dx),
                _ => {}
            }
        }
        if jump_for.is_some() && r.grounded() {
            k.jump = true;
            k.down = false;
        } else if roll_at && r.grounded() && !r.roll {
            k.down = true;
        } else if !skill.roll && r.grounded() {
            // Without the roll, hop the snuffers instead.
            let snuffer = Level::near(&lvl.foes, x + 60, 60, |f| f.x).any(|i| {
                let f = &lvl.foes[i];
                let (fx, _, _) = foe_at(lvl, f, self.elapsed);
                f.kind == FoeKind::Snuffer
                    && f.min_seats <= self.provisioned
                    && self.foes[i].hp > 0
                    && (30..=speed * 9).contains(&(fx - x))
            });
            k.jump = k.jump || snuffer;
        }

        // The warden: roll into it at speed, again and again.
        if let Some(dx) = warden {
            if dx < 190 && r.grounded() {
                k.down = r.speed() > 500;
                k.spur = r.gauge > 80 && dx > 60;
            }
        }

        // Chains: in the air, press jump afresh whenever a foe is in the cone.
        if airborne && r.lock.is_none() && r.lock_cd == 0 {
            if self.find_lock(r, n).is_some() {
                k.jump = !r.jump_key;
            }
        }
        if chain && r.grounded() && warden.is_none() {
            // Off the bank into the line of floaters.
            let near = Level::near(&lvl.foes, x + 70, 70, |f| f.x).any(|i| {
                lvl.foes[i].kind == FoeKind::Floater && self.foes[i].hp > 0
            });
            if near && r.speed() > 600 {
                k.jump = true;
            }
        }

        // The loop wants speed: spur into its mouth when the run is short.
        for p in &t.paths[lvl.main].portals {
            if p.min_speed == 0 && p.dir > 0 {
                let px = t.paths[lvl.main].sample(p.s).x / Q;
                if x < px && px - x < 220 && r.speed() < 1500 && r.gauge > 60 {
                    k.spur = true;
                }
            }
        }

        // The fork: speed to take the rails, or a hop over the mouth to stay low.
        if let Some(fx) = fork {
            match route {
                Route::Upper => {
                    if x < fx && fx - x < 260 && r.speed() < 1500 && r.gauge > 40 {
                        k.spur = true;
                    }
                }
                Route::Lower => {
                    // Ease off: the rails only take a rider at a gallop.
                    if r.grounded() && x < fx && fx - x < 70 && !on_rail {
                        k.x = if r.speed() >= 1180 { -1 } else { 0 };
                    }
                }
            }
        }
        // On a rail that ends: hop the gap.
        if let Some(g) = r.ground {
            let p = &t.paths[g.path as usize];
            if p.kind == Kind::Rail && p.next.is_none() && (p.len() - g.s) / Q < 22 {
                k.jump = true;
            }
        }
        // Springs, boost pads and posts need nothing; a spent gauge is topped up
        // by sparks as he goes. Spur on the long straights when it is full.
        if skill.spur {
            if r.gauge > 900 && !airborne && warden.is_none() && jump_for.is_none() && !roll_at {
                k.spur = true;
            }
            if r.spur && r.gauge > 120 && !airborne && jump_for.is_none() && warden.is_none() {
                k.spur = true;
            }
        }
        k
    }
}

/// Whether the rider is held up by a wall of road rather than just starting.
fn stalled(r: &Rider) -> bool {
    r.gsp.abs() < 160
}
