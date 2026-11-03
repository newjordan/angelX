//! What this crate's tests still ask of a session or a rider that the product
//! no longer carries: the bot's keys, the lance-lock's reach, a mended save, a
//! rider set down on the road. Each is answered by the oracle, the rules as
//! they ran in Rust, on a copy of the product's own state.
use super::oracle::{self, Route, Skill};
use super::rider::{Numbers, Rider};
use super::session::Session;
use super::terrain::Terrain;
use crate::rules::Input;

fn through<A: serde::Serialize, B: serde::de::DeserializeOwned>(a: &A) -> B {
    serde_json::from_value(serde_json::to_value(a).expect("state")).expect("the oracle's state is the product's")
}

pub fn old(n: &Numbers) -> oracle::Numbers {
    oracle::Numbers { slope: n.slope, loop_min: n.loop_min, spur_cap: n.spur_cap, lock_reach: n.lock_reach, dimming: n.dimming }
}

/// A session, ridden as the old rules read it.
pub trait SessionOracle {
    fn drive(&self, id: u32, route: Route, n: &Numbers) -> Input;
    fn drive_as(&self, id: u32, route: Route, n: &Numbers, skill: Skill) -> Input;
    fn find_lock(&self, r: &Rider, n: &Numbers) -> Option<usize>;
    fn last_post(&self) -> Option<usize>;
    fn repair(&mut self);
}

impl SessionOracle for Session {
    fn drive(&self, id: u32, route: Route, n: &Numbers) -> Input {
        self.drive_as(id, route, n, Skill::ACE)
    }
    fn drive_as(&self, id: u32, route: Route, n: &Numbers, skill: Skill) -> Input {
        through::<_, oracle::Session>(self).drive_as(id, route, &old(n), skill)
    }
    fn find_lock(&self, r: &Rider, n: &Numbers) -> Option<usize> {
        through::<_, oracle::Session>(self).find_lock(&through::<_, oracle::rider::Rider>(r), &old(n))
    }
    fn last_post(&self) -> Option<usize> {
        through::<_, oracle::Session>(self).last_post()
    }
    fn repair(&mut self) {
        let mut old: oracle::Session = through(&*self);
        old.repair();
        *self = through(&old);
    }
}

/// A rider, set down as the old rules set one.
pub trait RiderOracle {
    fn new_on(t: &Terrain, path: usize, s: i32) -> Rider;
    fn snap_camera(&mut self, px: i32, py: i32);
}

impl RiderOracle for Rider {
    fn new_on(_: &Terrain, path: usize, s: i32) -> Rider {
        through(&oracle::rider::Rider::new_on(&oracle::level::level().terrain, path, s))
    }
    fn snap_camera(&mut self, px: i32, py: i32) {
        let mut old: oracle::rider::Rider = through(&*self);
        old.snap_camera(px, py);
        *self = through(&old);
    }
}

/// A rider of the old rules standing on the main road at column `x`.
pub fn old_rider(x: i32) -> oracle::rider::Rider {
    let lvl = oracle::level::level();
    oracle::rider::Rider::new_on(&lvl.terrain, lvl.main, lvl.terrain.paths[lvl.main].s_at_x(x))
}

/// The old rules' road.
pub fn old_terrain() -> &'static oracle::terrain::Terrain {
    &oracle::level::level().terrain
}
