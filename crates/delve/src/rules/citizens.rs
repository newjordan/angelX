//! Citizens: the folk who live in what the realm builds.
//!
//! One tiny, shared population model for every built place — a loop's
//! settlement wing in the cockpit and a held floor's SimTower cross-section
//! in the Delve both ask the same question: given these bays, who is about,
//! and where are they at this tick?
//!
//! Light by construction: citizens are never stored. The roster is a pure
//! function of `(seed, bays)` and a citizen's place on their walk is a pure
//! function of `(seed, id, tick)`. Drawing a picture cannot mint, move or
//! lose anybody, equal worlds always show equal folk, and the whole
//! population of a place costs one bounded `Vec` per frame at most.
//! Citizens are ambience only — they haul, make, tend and rest, but the
//! model that owns the rooms (loop receipts, holdings labour) still owns
//! every cut and course. A builder stands at a rising room because the
//! room is rising, never the other way round.

/// The most folk one place ever shows at once.
pub const MAX_CITIZENS: usize = 12;
/// Standing bays a hauler serves before the place earns another.
const BAYS_PER_HAULER: usize = 3;
const MAX_HAULERS: usize = 2;

/// What one bay (a room slot, a settlement room) is right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayState {
    Empty,
    Building,
    Standing,
}

/// What a citizen is doing. Callers map their own room kinds onto the
/// standing jobs (`Make` at a bench, `Tend` a shrine or stores, `Rest` in
/// quarters, `Wander` a hall); `Build` and `Haul` are assigned here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Build,
    Haul,
    Make,
    Tend,
    Rest,
    Wander,
}

/// One bay as the roster sees it: its state, and the job worked in it once
/// it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bay {
    pub state: BayState,
    pub job: Job,
}

/// One citizen. `home` is the bay they belong to; a hauler walks between
/// `home` and `to` (everyone else has `to == home`). `hue` (0..4) lets a
/// renderer vary a tunic without inventing identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Citizen {
    pub id: u32,
    pub home: usize,
    pub to: usize,
    pub job: Job,
    pub hue: u8,
}

fn mix(seed: u64, id: u32) -> u64 {
    // splitmix64 finalizer over the seed and id: stable across runs and
    // platforms, no state, no allocation.
    let mut z = seed ^ (u64::from(id).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Who is about in a place with these bays. Standing bays get one citizen
/// working their job, a building bay one builder, and lived-in places earn
/// haulers running between the first and last standing bays. Bounded by
/// [`MAX_CITIZENS`]; builders and workers keep their places first.
pub fn roster(seed: u64, bays: &[Bay]) -> Vec<Citizen> {
    let mut folk = Vec::new();
    let next = |folk: &mut Vec<Citizen>, home: usize, to: usize, job: Job| {
        let id = folk.len() as u32;
        folk.push(Citizen {
            id,
            home,
            to,
            job,
            hue: (mix(seed, id) % 4) as u8,
        });
    };
    for (home, bay) in bays.iter().enumerate() {
        if folk.len() >= MAX_CITIZENS {
            return folk;
        }
        match bay.state {
            BayState::Empty => {}
            BayState::Building => next(&mut folk, home, home, Job::Build),
            BayState::Standing => next(&mut folk, home, home, bay.job),
        }
    }
    let standing: Vec<usize> = bays
        .iter()
        .enumerate()
        .filter(|(_, bay)| bay.state == BayState::Standing)
        .map(|(i, _)| i)
        .collect();
    if standing.len() >= 2 {
        let haulers = (standing.len() / BAYS_PER_HAULER).clamp(1, MAX_HAULERS);
        for _ in 0..haulers {
            if folk.len() >= MAX_CITIZENS {
                break;
            }
            next(&mut folk, standing[0], standing[standing.len() - 1], Job::Haul);
        }
    }
    folk
}

/// Where one citizen is on a walk of `span` steps at `tick`, taking
/// `stride` ticks per step: out to the far end, a working pause there,
/// back, a pause at home, repeat. Citizens are staggered by id so a place
/// never marches in step. A zero stride or span leaves them at home.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase {
    /// Steps from home, `0..=span`.
    pub step: u32,
    /// Heading back toward home.
    pub back: bool,
    /// Paused at an end, working (a renderer may swing a tool now).
    pub working: bool,
}

pub fn phase(seed: u64, id: u32, tick: u64, span: u32, stride: u64) -> Phase {
    if span == 0 || stride == 0 {
        return Phase {
            step: 0,
            back: false,
            working: true,
        };
    }
    let pause = (span / 3).max(2);
    let cycle = u64::from(span * 2 + pause * 2);
    let at = (tick / stride + mix(seed, id) % cycle) % cycle;
    let (span, pause) = (u64::from(span), u64::from(pause));
    if at < span {
        Phase {
            step: at as u32,
            back: false,
            working: false,
        }
    } else if at < span + pause {
        Phase {
            step: span as u32,
            back: true,
            working: true,
        }
    } else if at < span * 2 + pause {
        Phase {
            step: (span * 2 + pause - at) as u32,
            back: true,
            working: false,
        }
    } else {
        Phase {
            step: 0,
            back: false,
            working: true,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_citizens__tests.rs"]
mod tests;
