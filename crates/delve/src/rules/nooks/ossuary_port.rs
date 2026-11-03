//! The Bone Dominoes' rules as a hot-loaded module: the seam between `Run`
//! and the `delve-ossuary` Bend leg (`cockpit/live/delve-ossuary`).
//!
//! `ossuary.rs` stays the native oracle and the fallback. With a brain
//! installed on this thread, `tick_nook` hands the ossuary tick to it
//! instead. The seam is one pure function: a frame (the nook's own state and
//! a row per playable knight, geometry already answered) in, the next state
//! and an ordered list of effects out. Nothing is written to the `Run` until
//! the whole answer has been checked, so a module that errors, times out or
//! lies leaves the tick to the native code with no half-applied step.
//!
//! What stays here, in Rust: the `Run`, who stands where (distances), the
//! sound names, the banner words, the monster bodies, the gold purse, the
//! laurel and the save. What the module owns: every timer, branch, score and
//! the choice of effects.
use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// The nook's working state as the module sees it: the fields the ossuary
/// rule reads and writes. `score` is ascending by knight id.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OssuaryState {
    pub running: bool,
    pub done: bool,
    pub fallen: u8,
    pub next: u32,
    pub stalled: u32,
    pub raised: u8,
    pub score: Vec<(u32, u16)>,
}

/// One playable knight, in id order. `at_start` is within reach of the first
/// bone; `at_bone` within reach of the bone the line is about to topple
/// (bone `state.fallen`). Both are answered by Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OssuaryKnight {
    pub id: u32,
    pub pressed: bool,
    pub at_start: bool,
    pub at_bone: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OssuaryFrame {
    pub state: OssuaryState,
    pub knights: Vec<OssuaryKnight>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OssuaryCue {
    Clatter,
    Fall,
    Toll,
    Chest,
}

impl OssuaryCue {
    fn sound(self) -> &'static str {
        match self {
            Self::Clatter => "bones_clatter",
            Self::Fall => "knight_fall",
            Self::Toll => "circle_toll",
            Self::Chest => "chest_open",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OssuarySaid {
    Clack,
    Timber,
    Stops,
    Opens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OssuaryBeast {
    Skeleton,
}

/// What the module may ask of the world, in the order it asks.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase", deny_unknown_fields)]
pub enum OssuaryEffect {
    Sound { cue: OssuaryCue },
    Say { who: u32, said: OssuarySaid, #[serde(default)] gold: u32 },
    Raise { beast: OssuaryBeast, x: f32, y: f32 },
    Grant { id: u32, gold: u32 },
    Crown { id: u32 },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OssuaryStep {
    pub state: OssuaryState,
    pub effects: Vec<OssuaryEffect>,
}

/// The most effects one tick may carry, and the most coin one grant may be:
/// the host's envelope around whatever the module decides.
pub const MAX_EFFECTS: usize = 32;
pub const MAX_GRANT: u32 = 100;

/// The brain: a frame in, a step out, or why not. One per thread (tests, a
/// pinned generation) or one for the whole process (the host's setting).
pub type OssuaryBrain = Rc<dyn Fn(&OssuaryFrame) -> Result<OssuaryStep, String>>;
pub type SharedOssuaryBrain = Arc<dyn Fn(&OssuaryFrame) -> Result<OssuaryStep, String> + Send + Sync>;

/// How the installed brain has fared on this thread.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrainTally {
    /// Ticks the brain answered and the host applied.
    pub applied: u64,
    /// Ticks handed back to the native code: an error, or an answer that
    /// failed the host's check.
    pub fell_back: u64,
    pub last_error: Option<String>,
}

static SHARED: RwLock<Option<SharedOssuaryBrain>> = RwLock::new(None);
static SHARED_ARMED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static BRAIN: RefCell<Option<OssuaryBrain>> = const { RefCell::new(None) };
    static TALLY: RefCell<BrainTally> = const { RefCell::new(BrainTally { applied: 0, fell_back: 0, last_error: None }) };
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

/// Run the ossuary tick through `brain` on this thread (`None`: native
/// again); returns the brain it replaced. The tally keeps counting.
pub fn install_ossuary_brain(brain: Option<OssuaryBrain>) -> Option<OssuaryBrain> {
    ARMED.with(|armed| armed.set(brain.is_some()));
    BRAIN.with(|slot| std::mem::replace(&mut *slot.borrow_mut(), brain))
}

/// Run the ossuary tick through `brain` on every thread that has none of its
/// own (`None`: native again). This is the host's setting.
pub fn install_shared_ossuary_brain(brain: Option<SharedOssuaryBrain>) {
    SHARED_ARMED.store(brain.is_some(), Ordering::Release);
    *SHARED.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = brain;
}

/// Whether a brain, this thread's or the shared one, will answer the tick.
pub fn ossuary_brain_installed() -> bool {
    ARMED.with(Cell::get) || SHARED_ARMED.load(Ordering::Acquire)
}

enum Brain {
    Mine(OssuaryBrain),
    Shared(SharedOssuaryBrain),
}

impl Brain {
    fn ask(&self, frame: &OssuaryFrame) -> Result<OssuaryStep, String> {
        match self {
            Self::Mine(brain) => brain(frame),
            Self::Shared(brain) => brain(frame),
        }
    }
}

fn current_brain() -> Option<Brain> {
    if ARMED.with(Cell::get) {
        return BRAIN.with(|slot| slot.borrow().clone()).map(Brain::Mine);
    }
    if SHARED_ARMED.load(Ordering::Acquire) {
        return SHARED.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone().map(Brain::Shared);
    }
    None
}

pub fn ossuary_brain_tally() -> BrainTally {
    TALLY.with(|tally| tally.borrow().clone())
}

pub fn reset_ossuary_brain_tally() {
    TALLY.with(|tally| *tally.borrow_mut() = BrainTally::default());
}

impl OssuaryStep {
    /// The host's check of an answer against the frame it answered. Nothing
    /// the module returned is trusted until this passes.
    pub fn admit(&self, before: &OssuaryState, frame: &OssuaryFrame) -> Result<(), String> {
        let state = &self.state;
        if usize::from(state.fallen) > PATH.len()
            || state.raised > RAISE_CAP
            || state.next > ossuary::FALL
            || state.stalled > ossuary::STAND_AGAIN
            || state.score.len() > MAX_PLAYERS as usize
            || !state.score.windows(2).all(|pair| pair[0].0 < pair[1].0)
            || !state.score.iter().all(|(id, _)| (1..=MAX_PLAYERS).contains(id))
        {
            return Err("ossuary state outside its bounds".into());
        }
        if self.effects.len() > MAX_EFFECTS {
            return Err("ossuary effects exceed the tick budget".into());
        }
        let knight = |id: u32| frame.knights.iter().any(|k| k.id == id);
        let mut raises = 0u8;
        for effect in &self.effects {
            match *effect {
                OssuaryEffect::Sound { .. } => {}
                OssuaryEffect::Say { who, gold, .. } => {
                    if (who != 0 && !knight(who)) || gold > MAX_GRANT {
                        return Err("ossuary banner names a knight who is not here".into());
                    }
                }
                OssuaryEffect::Raise { x, y, .. } => {
                    if !(0.0..COLS as f32).contains(&x) || !(0.0..ROWS as f32).contains(&y) {
                        return Err("ossuary raises a monster outside the room".into());
                    }
                    raises += 1;
                }
                OssuaryEffect::Grant { id, gold } => {
                    if !knight(id) || gold > MAX_GRANT {
                        return Err("ossuary grants to a knight who is not here".into());
                    }
                }
                OssuaryEffect::Crown { id } => {
                    if !knight(id) && !state.score.iter().any(|(who, _)| *who == id) {
                        return Err("ossuary crowns a knight who is not here".into());
                    }
                }
            }
        }
        if state.raised < before.raised || state.raised - before.raised != raises {
            return Err("ossuary raised count and raise effects disagree".into());
        }
        Ok(())
    }
}

impl Run {
    /// The frame for this tick: the nook's state, and what each playable
    /// knight is doing and standing near. The bone in question is the one the
    /// line has reached before this tick runs.
    pub fn ossuary_frame(&self, nook: &Nook, pressed: u8) -> OssuaryFrame {
        let bone = usize::from(nook.fallen);
        let knights = self
            .nook_players()
            .into_iter()
            .map(|id| OssuaryKnight {
                id,
                pressed: has(pressed, id),
                at_start: self.nook_near(id, ossuary::at(0), REACH),
                at_bone: bone < PATH.len() && self.nook_near(id, ossuary::at(bone), 1.6),
            })
            .collect();
        OssuaryFrame {
            state: OssuaryState {
                running: nook.running,
                done: nook.done,
                fallen: nook.fallen,
                next: nook.next,
                stalled: nook.stalled,
                raised: nook.raised,
                score: nook.score.iter().map(|(&id, &n)| (id, n)).collect(),
            },
            knights,
        }
    }

    /// Tick the ossuary through the installed brain. True when the brain's
    /// answer was applied; false (nothing touched) when none is installed or
    /// the answer did not pass, and the native tick should run instead.
    pub(super) fn tick_ossuary_brain(&mut self, nook: &mut Nook, pressed: u8) -> bool {
        let Some(brain) = current_brain() else {
            return false;
        };
        let frame = self.ossuary_frame(nook, pressed);
        let answer = brain.ask(&frame).and_then(|step| step.admit(&frame.state, &frame).map(|()| step));
        match answer {
            Ok(step) => {
                self.apply_ossuary(nook, step);
                TALLY.with(|tally| tally.borrow_mut().applied += 1);
                true
            }
            Err(error) => {
                TALLY.with(|tally| {
                    let mut tally = tally.borrow_mut();
                    tally.fell_back += 1;
                    tally.last_error = Some(error);
                });
                false
            }
        }
    }

    /// Write a checked step into the nook and the world, effects in order.
    fn apply_ossuary(&mut self, nook: &mut Nook, step: OssuaryStep) {
        let OssuaryState { running, done, fallen, next, stalled, raised, score } = step.state;
        nook.running = running;
        nook.done = done;
        nook.fallen = fallen;
        nook.next = next;
        nook.stalled = stalled;
        nook.raised = raised;
        nook.score = score.into_iter().collect();
        for effect in step.effects {
            match effect {
                OssuaryEffect::Sound { cue } => self.sounds.push(cue.sound()),
                OssuaryEffect::Say { who, said, gold } => {
                    let words = match said {
                        OssuarySaid::Clack => "CLACK! FILL THE GAPS!".to_string(),
                        OssuarySaid::Timber => "TIMBER!".to_string(),
                        OssuarySaid::Stops => "THE LINE STOPS. BONES RISE!".to_string(),
                        OssuarySaid::Opens => format!("DONG! THE COFFER OPENS: {gold} GOLD EACH."),
                    };
                    self.found = Some((self.tick, who, words));
                }
                OssuaryEffect::Raise { beast: OssuaryBeast::Skeleton, x, y } => {
                    self.spawn_at(EnemyKind::Skeleton, x * TILE_UNITS, y * TILE_UNITS);
                }
                OssuaryEffect::Grant { id, gold } => {
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.carried.add(Spoil::Gold, gold);
                    }
                }
                OssuaryEffect::Crown { id } => {
                    if self.party_len() > 1 {
                        for (&who, hero) in self.players.iter_mut() {
                            hero.laurel = who == id;
                        }
                    }
                }
            }
        }
    }

    /// `tick_nook`, for hosts outside the crate that script a nook (the
    /// parity harness, the benchmark). It is the real tick: the installed
    /// brain answers it if there is one.
    pub fn tick_nook_for_test(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.tick_nook(inputs);
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/ossuary_port__tests.rs"]
mod tests;
