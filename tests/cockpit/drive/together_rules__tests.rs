//! The Bone Dominoes as a Bend module, against the native rules. The native
//! `ossuary.rs` is the oracle: two Runs from one seed take the same scripted
//! actions, one ticking the nook natively and one through the shipped
//! `delve-ossuary` leg, and every tick the nook, the knights, the monsters,
//! the sounds and the banner must agree to the byte (the whole Run every few
//! ticks). The measurements at the end are `--ignored`.
use super::*;
use crate::agent::harness::live_policy;
use angel_delve::rules::nooks::{
    self, BrainTally, GAPS, Nook, OssuaryBrain, PATH, domino_at, install_ossuary_brain, ossuary_brain_tally,
    reset_ossuary_brain_tally,
};
use angel_delve::rules::{Input, Run, RoomKind, TILE_UNITS};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::{OnceLock, RwLock, RwLockReadGuard};
use std::time::Instant;

type Inputs = BTreeMap<u32, Input>;

fn program() -> angel_live::Program {
    static PROGRAM: OnceLock<angel_live::Program> = OnceLock::new();
    PROGRAM.get_or_init(|| live_policy::shipped_leg(OSSUARY_LEG)).clone()
}

/// A party of `seats` in an ossuary nook, off a far cell.
fn ossuary(seed: u64, seats: u32) -> Run {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.enemies.clear();
    run.dungeon.rooms.push(nooks::room(RoomKind::Ossuary, (12, 12), 3));
    let index = run.dungeon.rooms.len() - 1;
    run.nook = Some(Nook { room: index, ..Default::default() });
    run.enter_for_test(index);
    run.enemies.clear();
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
    }
    run
}

/// What a script does to a knight before a tick: stand somewhere (arena
/// units), press the interact key, step out of play (the privy, warded stone).
#[derive(Clone, Copy, Debug)]
enum Act {
    Put(u32, f32, f32),
    Press(u32),
    Privy(u32, u32),
    Stone(u32, bool),
}

fn tile(point: (f32, f32)) -> (f32, f32) {
    (point.0 * TILE_UNITS, point.1 * TILE_UNITS)
}

fn put(id: u32, point: (f32, f32)) -> Act {
    let (x, y) = tile(point);
    Act::Put(id, x, y)
}

fn enact(run: &mut Run, acts: &[Act]) -> Inputs {
    let mut inputs = Inputs::new();
    for act in acts {
        match *act {
            Act::Put(id, x, y) => {
                if let Some(hero) = run.players.get_mut(&id) {
                    (hero.x, hero.y) = (x, y);
                }
            }
            Act::Press(id) => {
                inputs.insert(id, Input { interact: true, ..Default::default() });
            }
            Act::Privy(id, ticks) => {
                if let Some(hero) = run.players.get_mut(&id) {
                    hero.privy = ticks;
                }
            }
            Act::Stone(id, on) => {
                if let Some(hero) = run.players.get_mut(&id) {
                    hero.stone = on;
                }
            }
        }
    }
    inputs
}

fn slice(run: &Run) -> String {
    serde_json::to_string(&serde_json::json!({
        "tick": run.tick, "nook": run.nook, "players": run.players, "enemies": run.enemies,
        "sounds": run.sounds, "found": run.found, "cues": run.cues, "shake": run.shake,
    }))
    .unwrap()
}

/// What a lockstep run saw, to prove the script reached the branches it names.
#[derive(Debug, Default)]
struct Seen {
    ticks: u32,
    in_nook: u32,
    running: u32,
    stalled: u32,
    stalls: u32,
    completions: u32,
    max_raised: u8,
    skeletons: usize,
    banners: BTreeSet<String>,
    crowned: u32,
}

impl Seen {
    fn add(&mut self, other: &Seen) {
        self.ticks += other.ticks;
        self.in_nook += other.in_nook;
        self.running += other.running;
        self.stalled += other.stalled;
        self.stalls += other.stalls;
        self.completions += other.completions;
        self.max_raised = self.max_raised.max(other.max_raised);
        self.skeletons += other.skeletons;
        self.banners.extend(other.banners.iter().cloned());
        self.crowned += other.crowned;
    }
}

/// Two Runs from one seed, ticked in lockstep: native, and through a brain.
struct Pair {
    label: String,
    native: Run,
    bend: Run,
    brain: OssuaryBrain,
    seen: Seen,
    last_stalled: u32,
    was_done: bool,
    /// Knights are mended every tick, so the bones the line raises cannot end the script.
    mended: bool,
    /// Lockstep runs share the process with the test that arms the host-wide
    /// setting; that one waits for them all to finish.
    _unshared: RwLockReadGuard<'static, ()>,
}

static SETTING: RwLock<()> = RwLock::new(());

impl Pair {
    fn new(label: &str, seed: u64, seats: u32, brain: OssuaryBrain) -> Pair {
        Pair {
            label: format!("{label} seed {seed} seats {seats}"),
            native: ossuary(seed, seats),
            bend: ossuary(seed, seats),
            brain,
            seen: Seen::default(),
            last_stalled: 0,
            was_done: false,
            mended: true,
            _unshared: SETTING.read().unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
    }

    /// Let the raised bones hurt: dead knights are out of play.
    fn mortal(mut self) -> Pair {
        self.mended = false;
        self
    }

    fn step(&mut self, acts: &[Act]) {
        let inputs = enact(&mut self.native, acts);
        enact(&mut self.bend, acts);
        if self.mended {
            for run in [&mut self.native, &mut self.bend] {
                for hero in run.players.values_mut() {
                    hero.hp = hero.max_hp;
                }
            }
        }
        install_ossuary_brain(None);
        self.native.step(&inputs);
        install_ossuary_brain(Some(Rc::clone(&self.brain)));
        self.bend.step(&inputs);
        install_ossuary_brain(None);
        let (native, bend) = (slice(&self.native), slice(&self.bend));
        if native != bend {
            let (a, b): (serde_json::Value, serde_json::Value) =
                (serde_json::from_str(&native).unwrap(), serde_json::from_str(&bend).unwrap());
            let key = a.as_object().unwrap().keys().find(|k| a[k.as_str()] != b[k.as_str()]).cloned().unwrap_or_default();
            panic!(
                "{}: native and Bend diverge at tick {} in `{key}`\nnative: {}\n  bend: {}",
                self.label, self.native.tick, a[key.as_str()], b[key.as_str()]
            );
        }
        if self.native.tick % 20 == 0 {
            assert_eq!(
                serde_json::to_string(&self.native).unwrap(),
                serde_json::to_string(&self.bend).unwrap(),
                "{}: whole Run diverges at tick {}",
                self.label,
                self.native.tick
            );
        }
        self.note();
    }

    fn note(&mut self) {
        let seen = &mut self.seen;
        seen.ticks += 1;
        let Some(nook) = self.bend.nook.as_ref() else { return };
        if self.bend.at == nook.room {
            seen.in_nook += 1;
        }
        seen.running += u32::from(nook.running);
        seen.stalled += u32::from(nook.stalled > 0);
        seen.stalls += u32::from(nook.stalled > 0 && self.last_stalled == 0);
        self.last_stalled = nook.stalled;
        seen.completions += u32::from(nook.done && !self.was_done);
        self.was_done = nook.done;
        seen.max_raised = seen.max_raised.max(nook.raised);
        seen.skeletons = seen.skeletons.max(self.bend.enemies.len());
        if let Some((tick, _, words)) = &self.bend.found
            && *tick == self.bend.tick
        {
            seen.banners.insert(words.clone());
        }
        seen.crowned += u32::from(self.bend.players.values().any(|hero| hero.laurel));
    }

    /// The brain answered the nook's live ticks and nothing fell back to the
    /// native code. (A tick the step skips, a hit-stop or a finished nook, asks
    /// for no answer, so `applied` is at most the ticks spent in the room.)
    fn finish(self) -> Seen {
        let tally = ossuary_brain_tally();
        assert_eq!(tally.fell_back, 0, "{}: {tally:?}", self.label);
        assert!(tally.applied > 0 && tally.applied <= u64::from(self.seen.in_nook), "{}: {tally:?}", self.label);
        self.seen
    }
}

fn brain() -> OssuaryBrain {
    pinned_brain(program())
}

fn start() -> (f32, f32) {
    domino_at(0)
}

fn gap(k: usize) -> (f32, f32) {
    domino_at(GAPS[k])
}

fn lockstep(label: &str, seed: u64, seats: u32, ticks: u32, plan: impl FnMut(u32) -> Vec<Act>) -> Seen {
    run_pair(Pair::new(label, seed, seats, brain()), ticks, plan)
}

fn run_pair(mut pair: Pair, ticks: u32, mut plan: impl FnMut(u32) -> Vec<Act>) -> Seen {
    reset_ossuary_brain_tally();
    for tick in 0..ticks {
        pair.step(&plan(tick));
    }
    pair.finish()
}

#[test]
fn everyone_idle_leaves_the_line_standing() {
    for seats in 1..=4 {
        let seen = lockstep("idle", 7, seats, 400, |_| vec![]);
        assert_eq!((seen.running, seen.stalled, seen.completions), (0, 0, 0));
        assert_eq!(seen.in_nook, 400);
    }
}

#[test]
fn an_unattended_gap_stalls_the_line_and_raises_bones_to_the_cap() {
    for seed in [1, 2, 3] {
        // One knight sets the line going every 125 ticks and never stands in a gap.
        let seen = lockstep("stall", seed, 1, 1300, |tick| match tick % 125 {
            0 => vec![put(1, start())],
            1 => vec![Act::Press(1)],
            _ => vec![],
        });
        assert!(seen.stalls >= 9, "{seen:?}");
        assert_eq!(seen.max_raised, 4, "the nook raises no more than its cap: {seen:?}");
        assert!(seen.banners.contains("THE LINE STOPS. BONES RISE!"));
        assert!(seen.banners.contains("CLACK! FILL THE GAPS!"));
        assert_eq!(seen.completions, 0);
    }
}

#[test]
fn knights_in_the_gaps_carry_the_line_to_the_coffer_and_one_is_crowned() {
    // Four knights: one presses, three stand in the gaps. Three: the presser
    // walks round to the last gap while the line runs.
    for seats in [4, 3] {
        let mut standing = vec![put(seats, start())];
        standing.extend((0..2.min(seats as usize - 1)).map(|k| put(k as u32 + 1, gap(k))));
        if seats == 4 {
            standing.push(put(3, gap(2)));
        }
        let seen = lockstep("carry", 11, seats, 260, |tick| match tick {
            0 => standing.clone(),
            1 => vec![Act::Press(seats)],
            60 if seats == 3 => vec![put(3, gap(2))],
            _ => vec![],
        });
        assert_eq!(seen.completions, 1, "{seen:?}");
        assert!(seen.banners.contains("DONG! THE COFFER OPENS: 8 GOLD EACH."), "{:?}", seen.banners);
        assert!(seen.crowned > 0);
    }
}

#[test]
fn the_line_can_stall_at_any_of_its_three_gaps() {
    for empty in 0..3usize {
        reset_ossuary_brain_tally();
        let mut pair = Pair::new("stall-at", 21 + empty as u64, 4, brain());
        let mut standing = vec![put(4, start())];
        standing.extend((0..empty).map(|k| put(k as u32 + 1, gap(k))));
        pair.step(&standing);
        pair.step(&[Act::Press(4)]);
        for _ in 0..(GAPS[empty] as u32 + 2) * 4 {
            pair.step(&[]);
        }
        // The first bone to rise stands where the empty gap is, the second mid-room.
        let bones: Vec<(f32, f32)> = pair.bend.enemies.iter().map(|e| (e.x, e.y)).collect();
        assert_eq!(bones, vec![tile(gap(empty)), tile((12.0, 9.0))], "gap {empty}");
        assert_eq!(pair.bend.nook.as_ref().unwrap().fallen as usize, GAPS[empty]);
        let seen = pair.finish();
        assert_eq!(seen.stalls, 1);
    }
}

#[test]
fn simultaneous_presses_and_a_crowded_gap_go_to_the_lowest_id() {
    let mut seen = Seen::default();
    reset_ossuary_brain_tally();
    let mut pair = Pair::new("simultaneous", 5, 4, brain());
    // Everyone at the first bone, everyone presses on the same tick: knight 1 starts it.
    pair.step(&[put(1, start()), put(2, start()), put(3, start()), put(4, start())]);
    pair.step(&[Act::Press(1), Act::Press(2), Act::Press(3), Act::Press(4)]);
    assert_eq!(pair.bend.found.as_ref().map(|f| (f.1, f.2.as_str())), Some((1, "CLACK! FILL THE GAPS!")));
    // Knights 2 and 3 both stand in the first gap, 4 in the second, the third is empty.
    pair.step(&[put(1, (2.0, 2.0)), put(2, gap(0)), put(3, gap(0)), put(4, gap(1))]);
    for _ in 0..200 {
        pair.step(&[]);
    }
    let nook = pair.bend.nook.clone().unwrap();
    assert_eq!(nook.score.iter().map(|(&id, &n)| (id, n)).collect::<Vec<_>>(), vec![(2, 1), (4, 1)]);
    assert_eq!(nook.raised, 2);
    seen.add(&pair.finish());
    assert!(seen.stalls >= 1);
}

#[test]
fn knights_out_of_play_do_not_start_the_line_or_fill_a_gap() {
    reset_ossuary_brain_tally();
    let mut pair = Pair::new("unplayable", 9, 3, brain());
    // In the privy: pressing at the first bone does nothing.
    pair.step(&[put(1, start()), Act::Privy(1, 3)]);
    pair.step(&[Act::Press(1)]);
    assert!(!pair.bend.nook.as_ref().unwrap().running);
    pair.step(&[]);
    pair.step(&[]);
    pair.step(&[]);
    pair.step(&[Act::Press(1)]);
    assert!(pair.bend.nook.as_ref().unwrap().running);
    // Warded stone in the first gap, a sound knight a hand's breadth away: the line stops.
    pair.step(&[put(2, gap(0)), Act::Stone(2, true), put(3, (gap(0).0 + 2.0, gap(0).1))]);
    for _ in 0..90 {
        pair.step(&[]);
    }
    let nook = pair.bend.nook.clone().unwrap();
    assert!(nook.stalled > 0 || nook.raised > 0, "{nook:?}");
    assert!(nook.score.is_empty());
    pair.finish();
}

/// SplitMix64: a seeded script, not a source of truth.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// A random but progress-biased script: knights wander between the first
/// bone, the gaps, points a hair inside and outside the reach, and nowhere;
/// they press the key, and now and then leave play.
fn random_plan(dice: &mut Dice, seats: u32) -> Vec<Act> {
    let mut acts = vec![];
    for id in 1..=seats {
        if dice.chance(20) {
            let target = match dice.below(10) {
                0..=2 => start(),
                3..=6 => gap(dice.below(3) as usize),
                7 => {
                    // The edge of reach (1.6 arena units) from a gap: just in, just out.
                    let g = tile(gap(dice.below(3) as usize));
                    let reach = 1.6 + [-0.01, -0.0001, 0.0, 0.0001, 0.01][dice.below(5) as usize];
                    acts.push(Act::Put(id, g.0 + reach, g.1));
                    continue;
                }
                8 => domino_at(dice.below(PATH.len() as u64) as usize),
                _ => (2.0 + dice.below(20) as f32, 2.0 + dice.below(9) as f32),
            };
            acts.push(put(id, target));
        }
        if dice.chance(12) {
            acts.push(Act::Press(id));
        }
        if dice.chance(1) {
            acts.push(Act::Privy(id, 1 + dice.below(8) as u32));
        }
        if dice.chance(1) {
            acts.push(Act::Stone(id, dice.chance(50)));
        }
    }
    acts
}

#[test]
fn random_play_agrees_with_the_native_rules_for_every_seed_and_party_size() {
    let mut total = Seen::default();
    for seed in 0..5u64 {
        for seats in 1..=4u32 {
            let mut dice = Dice(seed * 1000 + u64::from(seats));
            // Odd seeds let the raised bones hurt, so knights die and drop out of play.
            let mut pair = Pair::new("random", 100 + seed, seats, brain());
            if seed % 2 == 1 {
                pair = pair.mortal();
            }
            let seen = run_pair(pair, 1200, |_| random_plan(&mut dice, seats));
            assert!(seen.in_nook > 0);
            total.add(&seen);
        }
    }
    // The sweep reached the interesting branches, not just the idle one.
    eprintln!("random sweep: {total:?}");
    assert!(total.running > 500 && total.stalls > 20 && total.completions >= 3, "{total:?}");
    assert!(total.max_raised == 4 && total.skeletons > 0 && total.crowned > 0);
    for words in ["CLACK! FILL THE GAPS!", "TIMBER!", "THE LINE STOPS. BONES RISE!", "DONG! THE COFFER OPENS: 8 GOLD EACH."] {
        assert!(total.banners.contains(words), "never saw {words}");
    }
}

#[test]
fn the_setting_arms_the_leg_for_every_thread_and_a_missing_leg_means_native() {
    let _alone = SETTING.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    let play = |armed: Option<Option<angel_live::Program>>| {
        std::thread::spawn(move || {
            match armed {
                Some(program) => arm_pinned(program),
                None => arm(false),
            }
            reset_ossuary_brain_tally();
            let mut run = ossuary(3, 2);
            for tick in 0..120 {
                let acts = match tick {
                    0 => vec![put(1, start())],
                    1 => vec![Act::Press(1)],
                    _ => vec![],
                };
                let inputs = enact(&mut run, &acts);
                run.step(&inputs);
            }
            arm(false);
            (slice(&run), ossuary_brain_tally())
        })
        .join()
        .unwrap()
    };
    let (native, off) = play(None);
    let (bend, on) = play(Some(Some(program())));
    assert_eq!(native, bend);
    assert_eq!((off.applied, off.fell_back), (0, 0));
    assert!(on.applied > 100 && on.fell_back == 0, "{on:?}");
    // The setting is on but the leg is not there: every tick is the native tick.
    let (missing, none) = play(Some(None));
    assert_eq!(native, missing);
    assert!(none.applied == 0 && none.fell_back > 100, "{none:?}");
}

#[test]
#[ignore = "needs the Bend compiler: node scripts/live/bend.mjs setup"]
fn an_edited_module_takes_over_a_running_line_and_keeps_its_state() {
    let root = std::env::temp_dir().join(format!("angel-ossuary-swap-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let module = root.join("delve-ossuary");
    std::fs::create_dir_all(&module).unwrap();
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/delve-ossuary");
    for entry in std::fs::read_dir(&shipped).unwrap().flatten() {
        std::fs::copy(entry.path(), module.join(entry.file_name())).unwrap();
    }
    let mut registry = angel_live::Registry::validated(root.clone(), crate::agent::hotload::evaluate, |_, _| Ok(()));
    registry.refresh(true);
    let generation = |registry: &angel_live::Registry| registry.snapshot().statuses.iter().find(|s| s.id == OSSUARY_LEG).unwrap().generation;
    let (before, mut run) = (generation(&registry), ossuary(3, 1));
    let tick = |registry: &angel_live::Registry, run: &mut Run, acts: &[Act]| {
        let brain = pinned_brain(registry.snapshot().policies[OSSUARY_LEG].clone());
        install_ossuary_brain(Some(brain));
        let inputs = enact(run, acts);
        run.step(&inputs);
        install_ossuary_brain(None);
    };
    tick(&registry, &mut run, &[put(1, start())]);
    tick(&registry, &mut run, &[Act::Press(1)]);
    for _ in 0..30 {
        tick(&registry, &mut run, &[]);
    }
    let fallen = run.nook.as_ref().unwrap().fallen;
    assert!(run.nook.as_ref().unwrap().running && fallen > 4);
    // Edit the rule while the line is falling: a bone every two ticks, not four.
    let source = std::fs::read_to_string(module.join("ossuary.bend")).unwrap();
    let edited = source.replacen("def fall_ticks() -> U32:\n  4", "def fall_ticks() -> U32:\n  2", 1);
    assert_ne!(source, edited);
    std::fs::write(module.join("ossuary.bend"), edited).unwrap();
    let built = std::process::Command::new("node")
        .args(["scripts/live/bend.mjs", "build"])
        .arg(&module)
        .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
        .output()
        .unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    registry.refresh(true);
    assert_eq!(generation(&registry), before + 1, "{:?}", registry.snapshot().statuses);
    // The Run kept the line where it was; the next bones come at the new pace.
    assert_eq!(run.nook.as_ref().unwrap().fallen, fallen);
    let mut gaps = vec![];
    let mut last = run.nook.as_ref().unwrap().fallen;
    for t in 0..24 {
        tick(&registry, &mut run, &[]);
        let now = run.nook.as_ref().unwrap().fallen;
        if now != last {
            gaps.push(t);
            last = now;
        }
    }
    let paces: Vec<i32> = gaps.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(paces.len() >= 4 && paces[1..].iter().all(|&p| p == 2), "{gaps:?}");
    let _ = std::fs::remove_dir_all(&root);
}

// ---- measurements ----------------------------------------------------------

fn percentile(sorted: &[u128], p: usize) -> u128 {
    sorted[(sorted.len() * p / 100).min(sorted.len() - 1)]
}

fn stats(mut samples: Vec<u128>) -> (u128, u128, u128, u128) {
    samples.sort_unstable();
    let mean = samples.iter().sum::<u128>() / samples.len() as u128;
    (percentile(&samples, 50), percentile(&samples, 95), *samples.last().unwrap(), mean)
}

/// One cycle is 180 ticks: place the party, press at the first bone, wait.
/// Even cycles leave the gaps empty (the line stalls and raises bones); odd
/// cycles with four knights fill them (the line reaches the coffer). Every
/// cycle starts from a fresh nook, as the next floor's would.
const CYCLE: u32 = 180;

fn cycle_tick(run: &mut Run, seats: u32, tick: u32) -> Inputs {
    let (cycle, at) = (tick / CYCLE, tick % CYCLE);
    let mut acts = vec![];
    if at == 0 {
        let room = run.nook.as_ref().unwrap().room;
        run.nook = Some(Nook { room, ..Default::default() });
        run.enemies.clear();
        acts.push(put(1, start()));
        for id in 2..=seats {
            let target = if cycle % 2 == 1 && id <= 4 { gap(id as usize - 2) } else { (2.0, 2.0) };
            acts.push(put(id, target));
        }
    }
    if at == 1 {
        acts.push(Act::Press(1));
    }
    run.tick += 1;
    enact(run, &acts)
}

fn time_nook_ticks(run: &mut Run, seats: u32, ticks: u32) -> Vec<u128> {
    let mut nanos = Vec::with_capacity(ticks as usize);
    for tick in 0..ticks {
        let inputs = cycle_tick(run, seats, tick);
        let before = Instant::now();
        run.tick_nook_for_test(&inputs);
        nanos.push(before.elapsed().as_nanos());
    }
    nanos
}

/// Where a Bend tick's time goes, summed over the run.
#[derive(Default)]
struct Split {
    encode: Cell<u128>,
    render: Cell<u128>,
    decode: Cell<u128>,
    bytes_in: Cell<usize>,
    bytes_out: Cell<usize>,
    calls: Cell<u64>,
    /// Calls that carried a key press, an effect or a change beyond the two
    /// countdowns: the ticks a wake-driven host could not have skipped.
    events: Cell<u64>,
}

fn measured_brain(program: angel_live::Program, split: Rc<Split>) -> OssuaryBrain {
    Rc::new(move |frame| {
        let t0 = Instant::now();
        let input = serde_json::to_value(frame).map_err(|e| e.to_string())?;
        split.bytes_in.set(split.bytes_in.get() + input.to_string().len());
        let t1 = Instant::now();
        let value = program.render(&input)?;
        let t2 = Instant::now();
        split.bytes_out.set(split.bytes_out.get() + value.to_string().len());
        let step: Result<OssuaryStep, String> = serde_json::from_value(value).map_err(|e| e.to_string());
        let t3 = Instant::now();
        if let Ok(step) = &step {
            let (a, b) = (&frame.state, &step.state);
            let moved = (a.running, a.done, a.fallen, a.raised, &a.score) != (b.running, b.done, b.fallen, b.raised, &b.score);
            if moved || !step.effects.is_empty() || frame.knights.iter().any(|k| k.pressed) {
                split.events.set(split.events.get() + 1);
            }
        }
        split.encode.set(split.encode.get() + (t1 - t0).as_nanos());
        split.render.set(split.render.get() + (t2 - t1).as_nanos());
        split.decode.set(split.decode.get() + (t3 - t2).as_nanos());
        split.calls.set(split.calls.get() + 1);
        step
    })
}

/// Shape (a), for comparison: the same compiled rule as a `system` module
/// whose line lives in the Kernel's checkpoint and is stepped as an instance.
fn system_brain(dir: &std::path::Path) -> (angel_live::Registry, OssuaryBrain) {
    std::fs::create_dir_all(dir.join("delve-ossuary-system")).unwrap();
    let module = dir.join("delve-ossuary-system");
    let live = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/delve-ossuary");
    for name in ["ossuary.compiled.js", "index.js"] {
        let target = if name == "index.js" { "policy.js" } else { name };
        std::fs::copy(live.join(name), module.join(target)).unwrap();
    }
    std::fs::write(
        module.join("index.js"),
        r#"const policy = require('./policy.js')
const fresh = () => ({ running: false, done: false, fallen: 0, next: 0, stalled: 0, raised: 0, score: [] })
module.exports = {
  apiVersion: 2,
  initialize: () => ({}),
  reduce: state => ({ state }),
  resources: () => ({ rules: { rule: 'delve-ossuary' } }),
  instantiate: () => ({ line: fresh() }),
  // The Kernel owns the line. The host restates it only after an edit of its own.
  reduceInstance: (state, event) => {
    const out = policy.render({ state: event.line ?? state.line, knights: event.knights })
    return { state: { line: out.state }, output: out }
  },
}
"#,
    )
    .unwrap();
    std::fs::write(
        module.join("module.json"),
        r#"{"api_version":2,"id":"delve-ossuary-system","kind":"system","entry":"index.js","state_schema":1,"assets":[],
"exports":[{"kind":"resource","name":"rules","contract":"delve-rules/1"}]}"#,
    )
    .unwrap();
    let mut registry = angel_live::Registry::with_resources(
        dir.to_path_buf(),
        crate::agent::hotload::evaluate,
        |_, _| Ok(()),
        crate::agent::hotload::bind_resource,
    );
    registry.refresh(true);
    let snapshot = registry.snapshot();
    let resource = snapshot
        .resources
        .get("delve-ossuary-system/rules")
        .unwrap_or_else(|| panic!("system module did not load: {:?}", snapshot.statuses))
        .clone();
    let instance = RefCell::new(resource.instantiate(&serde_json::json!({})).unwrap());
    let brain: OssuaryBrain = Rc::new(move |frame| {
        let mut instance = instance.borrow_mut();
        let line = serde_json::to_value(&frame.state).map_err(|e| e.to_string())?;
        let mut event = serde_json::json!({ "knights": frame.knights });
        if instance.checkpoint().state["line"] != line {
            event["line"] = line;
        }
        let staged = instance.stage(&event)?;
        let output = staged.output.clone();
        instance.commit(staged)?;
        serde_json::from_value(output).map_err(|e| e.to_string())
    });
    (registry, brain)
}

#[test]
fn the_system_shape_agrees_with_the_native_rules_too() {
    let dir = std::env::temp_dir().join(format!("angel-ossuary-system-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (_registry, brain) = system_brain(&dir);
    let mut total = Seen::default();
    for seed in 0..2u64 {
        for seats in [1, 4] {
            reset_ossuary_brain_tally();
            let mut pair = Pair::new("system", seed, seats, Rc::clone(&brain));
            if seed == 1 {
                pair = pair.mortal();
            }
            let mut dice = Dice(seed * 77 + u64::from(seats));
            for _ in 0..600 {
                pair.step(&random_plan(&mut dice, seats));
            }
            total.add(&pair.finish());
        }
    }
    assert!(total.running > 100, "{total:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "local measurement; run with --ignored --nocapture, on a quiet machine"]
fn ossuary_tick_cost_native_and_bend() {
    let ticks = CYCLE * 7;
    let micros = |nanos: u128| nanos as f64 / 1000.0;
    // The floor of one call through the policy boundary: a leg that does no work.
    let floor_program = live_policy::shipped_leg("swarm-route");
    let probe = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("live/swarm-route/module.json")).unwrap(),
    )
    .unwrap()["probe"]
        .clone();
    let floor: Vec<u128> = (0..2000)
        .map(|_| {
            let before = Instant::now();
            floor_program.render(&probe).unwrap();
            before.elapsed().as_nanos()
        })
        .collect();
    let (p50, p95, max, _) = stats(floor[200..].to_vec());
    eprintln!("boundary floor (swarm-route, no work): p50={:.1}us p95={:.1}us max={:.1}us", micros(p50), micros(p95), micros(max));
    let dir = std::env::temp_dir().join(format!("angel-ossuary-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (_registry, system) = system_brain(&dir);
    for seats in [1u32, 4] {
        // Warm both paths, then measure.
        let mut warm = ossuary(3, seats);
        install_ossuary_brain(Some(brain()));
        time_nook_ticks(&mut warm, seats, CYCLE * 2);
        install_ossuary_brain(None);

        let mut run = ossuary(3, seats);
        let (n50, n95, nmax, nmean) = stats(time_nook_ticks(&mut run, seats, ticks));

        let split = Rc::new(Split::default());
        let mut run = ossuary(3, seats);
        install_ossuary_brain(Some(measured_brain(program(), Rc::clone(&split))));
        reset_ossuary_brain_tally();
        let (b50, b95, bmax, bmean) = stats(time_nook_ticks(&mut run, seats, ticks));
        let tally: BrainTally = ossuary_brain_tally();
        install_ossuary_brain(None);
        assert_eq!(tally.fell_back, 0);

        let mut run = ossuary(3, seats);
        install_ossuary_brain(Some(Rc::clone(&system)));
        let (s50, s95, smax, smean) = stats(time_nook_ticks(&mut run, seats, ticks));
        install_ossuary_brain(None);

        let calls = split.calls.get().max(1) as u128;
        eprintln!(
            "seats={seats} ticks={ticks} (nook live {calls} ticks)\n  native            : p50={n50}ns p95={n95}ns max={nmax}ns mean={nmean}ns\n  bend policy leg   : p50={:.1}us p95={:.1}us max={:.1}us mean={:.1}us\n  bend system module: p50={:.1}us p95={:.1}us max={:.1}us mean={:.1}us\n  policy split/call : encode={:.1}us render={:.1}us decode={:.1}us; frame={}B step={}B per call; {} of {} calls carried a press, an effect or a state change",
            micros(b50), micros(b95), micros(bmax), micros(bmean),
            micros(s50), micros(s95), micros(smax), micros(smean),
            micros(split.encode.get() / calls), micros(split.render.get() / calls), micros(split.decode.get() / calls),
            split.bytes_in.get() / calls as usize, split.bytes_out.get() / calls as usize,
            split.events.get(), calls,
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
