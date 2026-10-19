//! The Delve drawn in wasm: a scene is set up, then stepped and framed a
//! tick at a time; the host (node, or the native timer in `main.rs`) keeps
//! the clock. Each frame answers a checksum of its pixels, so wasm and
//! native can be shown to draw the same bytes.
//!
//! The wire, too: a scene's run is stepped and its tick's patch made as the
//! server makes it (`wire_host`, not timed), then taken in as a client
//! takes it (`wire_client`: the patch read, applied to the tree, and the
//! tree read back into a run), so the client's cost a tick can be timed.

use angel_delve::render::overworld::arena;
use angel_delve::rules::Run;
use angel_delve::wire;
use angel_net::codec::Bytes;
use angel_net::doc::delta::{apply, diff};
use angel_net::doc::value::Value;
use angel_net::doc::wire::{Reader, Writer};
use std::cell::RefCell;

thread_local! {
    static RUN: RefCell<Option<Run>> = const { RefCell::new(None) };
    static WIRE: RefCell<Option<Wire>> = const { RefCell::new(None) };
    static DUMP: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Untimed diagnostic state, for locating a parity mismatch by field.
pub fn state_json() -> Vec<u8> {
    RUN.with(|r| serde_json::to_vec(r.borrow().as_ref().expect("scene first")).unwrap())
}

#[unsafe(no_mangle)]
pub extern "C" fn state_json_ptr() -> *const u8 {
    DUMP.with(|d| { *d.borrow_mut() = state_json(); d.borrow().as_ptr() })
}

#[unsafe(no_mangle)]
pub extern "C" fn state_json_len() -> u32 {
    DUMP.with(|d| d.borrow().len() as u32)
}

/// Both ends of one client's stream: the server's run, what it last sent
/// and its names; the client's tree and names; the body between them.
struct Wire {
    run: Run,
    publisher: wire::Publisher,
    sent: Value,
    writer: Writer,
    tree: Value,
    reader: Reader,
    body: Vec<u8>,
}

/// The scenes measured: a fresh run's first room, the kingdom, a busy fight.
pub const SCENES: [&str; 44] = ["first-room", "kingdom", "busy-fight", "cinder-stack", "latch-duel", "mossbell", "brasswake", "rafterwake", "pennon-rush", "cinderjaw", "the-index", "mother-of-spores", "waxen-warden", "the-bone-choir", "crypt-tithe-marshal", "crypt-tax-keeper", "crypt-relic-liege", "iron-tusk-chief", "red-pick-chief", "the-foreman", "late-fee-leviathan", "side-hall", "book-hop", "archive-bellkeeper", "fungal-rootwarden", "fungal-spore-crown", "grinwick", "hellforge-ash-castellan", "hellforge-chain-keeper", "hellforge-free-captain", "hellforge-fuel-reeve", "hellforge-slag-sovereign", "salvage-claimant", "shoggoth-heart", "the-big-ripper", "unknown-memory-warden", "unknown-silence-warden", "forest-road", "drift-course",
    // The Round Table adventures' stubs (docs/ROUNDTABLE-ADVENTURES.md): four seats each.
    "lanternward", "moonfall", "bannerpeak", "griffinwing", "lightgallop"];

/// The same preparation hook as GameClient::boot, timed outside display frames.
#[unsafe(no_mangle)]
pub extern "C" fn warm_entry(kind: u32) {
    if let Some(game) = kind.checked_sub(3).and_then(|n| arena::slot_art::GAMES.get(n as usize)) {
        arena::slot_art::warm(*game);
    } else {
        arena::slot_art::warm_dungeon();
    }
    RUN.with(|r| {
        if let Some(run) = r.borrow().as_ref() {
            arena::prepare_entry(run, Some(1));
        }
    });
}

// Keep the captain and its attack effects on screen throughout the measured ticks.
fn pennon_scene()->Run {
    let mut r=Run::pennon_review_for_test("drop-tell");
    r.pennon.as_mut().unwrap().lanes.get_mut(&1).unwrap().x=1060*angel_delve::rules::pennon::Q;
    r
}

// A real four-seat replay reaches the Feller; the fixture never invents combat state.
fn forest_scene() -> Run {
    let mut run=Run::forest_road_for_test();
    for id in 2..=4 { run.join_seat(id,&format!("Knight {id}")); }
    for (n,class) in angel_delve::rules::classes::CLASSES[..4].iter().enumerate() { run.become_class(n as u32+1,class); }
    run.start_forest_road();
    loop {
        let stage=run.forest_road.as_ref().unwrap();
        if stage.wave==7 && stage.foes[0].x-stage.knights[&1].camera<330 { return run; }
        let inputs=run.players.iter().map(|(&id,h)|(id,stage.bot_input(id,h))).collect();
        run.step(&inputs);
    }
}

// A Round Table adventure's stub with four seats at the table, each from its own fixture.
fn roundtable_scene(slug: &str) -> Run {
    let mut run = Run::roundtable_for_test(slug).expect("a Round Table adventure");
    for id in 2..=4 { run.join_seat(id, &format!("Knight {id}")); }
    run.step(&Default::default());
    run
}

// Reach a charged corner with the same four driver controllers as the host.
fn drift_scene() -> Run {
    use angel_delve::rules::race3d::{self, Course};
    let mut run = arena::race3d::testing::racing(Course::Touge, 4);
    run.floor_plates.clear();
    loop {
        let race = run.race3d.as_ref().unwrap();
        if race.riders[&1].drift.stage() >= 2 && race.riders[&1].drift.sliding { return run; }
        let inputs = race.riders.keys().map(|&id| (id, race3d::drift_bot(race, id))).collect();
        run.step(&inputs);
    }
}

/// Set up scene `kind` (an index into [`SCENES`]); `art_v2` picks the kit.
#[unsafe(no_mangle)]
pub extern "C" fn scene(kind: u32, art_v2: u32) -> u32 {
    angel_delve::settings::set(angel_delve::settings::Settings {
        art_v2: art_v2 != 0,
        feel_ledger: false,
        open_floors: false,
    });
    let run = match kind {
        0 => Run::new(7, 1, None),
        1 => arena::art_v2::testing::kingdom(),
        2 => arena::art_v2::testing::fight(),
        3 => {
            let mut r=Run::cinder_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        4 => {
            let mut r=Run::latch_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        5 => {
            let mut r=Run::mossbell_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        6 => {
            let mut r=Run::brasswake_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        7 => Run::rafterwake_review_for_test("drop-tell"),
        8 => pennon_scene(),
        9..=20 | 23..=36 => arena::art_v2::testing::boss(SCENES[kind as usize]),
        21 => arena::art_v2::testing::hall(),
        22 => arena::art_v2::testing::book_hop(),
        37 => forest_scene(),
        38 => drift_scene(),
        39..=43 => roundtable_scene(SCENES[kind as usize]),
        _ => return 0,
    };
    RUN.with(|r| *r.borrow_mut() = Some(run));
    1
}

/// The same scene with three additional seated clients, outside frame timing.
#[unsafe(no_mangle)]
pub extern "C" fn party() {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            for seat in 2..=4 { if !run.players.contains_key(&seat) { run.join_seat(seat, &format!("Knight {seat}")); } }
        }
    });
}

/// Step the scene `ticks` ticks with no one at the keys.
#[unsafe(no_mangle)]
pub extern "C" fn step(ticks: u32) {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            for _ in 0..ticks {
                if let Some(stage)=&run.forest_road {
                    let inputs=run.players.iter().map(|(&id,h)|(id,stage.bot_input(id,h))).collect();
                    run.step(&inputs);
                } else if run.roundtable_showing() {
                    let inputs=run.roundtable_inputs();
                    run.step(&inputs);
                } else if let Some(race) = run.race3d.as_ref().filter(|r| r.course == angel_delve::rules::race3d::Course::Touge) {
                    let inputs = race.riders.keys().map(|&id| (id, angel_delve::rules::race3d::drift_bot(race, id))).collect();
                    run.step(&inputs);
                } else { run.step(&Default::default()); }
            }
        }
    });
}

/// Stage the existing rage warning for the art benchmark, without changing rules.
#[unsafe(no_mangle)]
pub extern "C" fn force_rage() {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            for enemy in &mut run.enemies {
                enemy.hp = enemy.max_hp * 3 / 10;
                enemy.seen_hp = enemy.hp;
                enemy.dread = 1;
            }
        }
    });
}

/// Stage the collector's real cast for an active-area frame-cost sample.
#[unsafe(no_mangle)]
pub extern "C" fn force_ravage() {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            let Some(enemy) = run.enemies.first() else { return };
            let Some(boss) = enemy.boss.and_then(|b| run.bosses.get(b as usize)) else { return };
            let Some(attack) = boss.attacks.iter().find(|a| a.pattern == angel_delve::rules::bosses::Pattern::Ravage) else { return };
            let mut cast = angel_delve::rules::tide::Ravage::cast(
                (enemy.x, enemy.y), attack.shots, attack.arc, attack.speed, attack.damage, 17,
            );
            cast.boss_art = angel_delve::rules::bosses::Art::for_id(&boss.id);
            run.ravages = vec![cast];
        }
    });
}

/// Read-only parity evidence, outside every timed frame. Part zero includes
/// the wire state; the other hashes isolate actors and shot physics.
#[unsafe(no_mangle)]
pub extern "C" fn state_hash(part: u32) -> u32 {
    RUN.with(|r| {
        let r = r.borrow();
        let Some(run) = r.as_ref() else { return 0 };
        let fold = |h: u32, b: u8| (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
        if part == 9 {
            let mut plain = run.clone();
            for shot in &mut plain.projectiles { shot.look = None; }
            for cast in &mut plain.ravages { cast.boss_art = None; }
            plain.feel.impacts.clear();
            let tree = wire::Publisher::default().publish(&plain).expect("gameplay parity tree");
            let mut bytes = Vec::new();
            Writer::default().value(&mut bytes, &tree);
            return bytes.into_iter().fold(0x811c_9dc5, fold);
        }
        if matches!(part, 0 | 5 | 6 | 7 | 8) {
            // Part 6 excludes tune metadata only, for old/off byte comparisons.
            let normalized = (part == 6).then(|| { let mut r=run.clone(); r.tune=Default::default(); r });
            let run = normalized.as_ref().unwrap_or(run);
            let mut tree = wire::Publisher::default().publish(run).expect("parity tree");
            if part == 7 || part == 8 {
                // OFF compatibility excludes only the six newly added hit-stop knobs.
                assert_eq!(run.tune.hitstop, 0, "old-schema projection requires hit-stop OFF");
                assert!(run.hitstop.quiet(), "old-schema projection requires quiet hit-stop");
                if let Some(Value::Struct(fields)) = tree.field_mut("tune") {
                    const ADDED: &[&str] = &["hitstop", "hitstop_boss_kill", "hitstop_elite_kill", "hitstop_phase", "hitstop_heavy", "hitstop_cooldown"];
                    fields.retain(|(name, _)| !ADDED.contains(&name.as_ref()));
                }
                // A disabled new knob makes serde emit an otherwise-default tune.
                // Restore the old omission only after checking every old value.
                let defaults = angel_delve::rules::tune::Tune::default();
                let old_tune = angel_delve::rules::tune::Tune {
                    hitstop: defaults.hitstop,
                    hitstop_boss_kill: defaults.hitstop_boss_kill,
                    hitstop_elite_kill: defaults.hitstop_elite_kill,
                    hitstop_phase: defaults.hitstop_phase,
                    hitstop_heavy: defaults.hitstop_heavy,
                    hitstop_cooldown: defaults.hitstop_cooldown,
                    ..run.tune
                };
                if old_tune == defaults {
                    if let Value::Struct(fields) = &mut tree {
                        fields.retain(|(name, _)| name.as_ref() != "tune");
                    }
                }
            }
            if part == 5 || part == 8 {
                // Compare the pre-shelf wire schema without dropping any old field.
                // This projection is only valid for fixtures with no shelf game active.
                for name in ["cinder", "latch", "brasswake", "rafterwake", "pennon", "mossbell"] {
                    assert!(matches!(tree.field(name), Some(Value::None)));
                }
                if let Some(Value::Struct(fields)) = tree.field_mut("tune") {
                    const ADDED: &[&str] = &["cinder_fall", "cinder_slide", "cinder_lock", "cinder_tell", "cinder_attack", "pennon_gravity", "pennon_jump", "pennon_run", "pennon_coyote", "pennon_buffer", "rafter_gravity", "rafter_jump", "rafter_run", "rafter_coyote", "rafter_buffer", "brass_stride", "brass_combo", "brass_tell", "brass_shove", "brass_stop", "latch_tell", "latch_window", "latch_rest", "latch_recover", "latch_stop", "moss_launch", "moss_nudge", "moss_bounce", "moss_gravity", "moss_flipper", "roundtable"];
                    fields.retain(|(name, _)| !ADDED.contains(&name.as_ref()));
                }
                if let Value::Struct(fields) = &mut tree {
                    fields.retain(|(name, _)| !["cinder", "latch", "brasswake", "rafterwake", "pennon", "mossbell"].contains(&name.as_ref()));
                }
            }
            let mut bytes = Vec::new();
            Writer::default().value(&mut bytes, &tree);
            return bytes.into_iter().fold(0x811c_9dc5, fold);
        }
        let words: Vec<u32> = match part {
            1 => run.enemies.iter().flat_map(|e| [e.x.to_bits(), e.y.to_bits(), e.hp, e.age, e.dread]).collect(),
            2 => run.projectiles.iter().flat_map(|p| [p.x.to_bits(), p.y.to_bits(), p.vx.to_bits(), p.vy.to_bits(), p.ttl]).collect(),
            3 => run.players.values().flat_map(|h| [h.x.to_bits(), h.y.to_bits(), h.hp]).collect(),
            4 => return run.projectiles.len() as u32,
            _ => return 0,
        };
        words.into_iter().flat_map(u32::to_le_bytes).fold(0x811c_9dc5, fold)
    })
}

/// Disable room hit-stop for pre-change simulation comparisons.
#[unsafe(no_mangle)]
pub extern "C" fn hitstop_off() {
    RUN.with(|r| { if let Some(run) = r.borrow_mut().as_mut() { run.tune.hitstop = 0; } });
}

/// Appearance-free control for cross-platform simulation diagnostics.
#[unsafe(no_mangle)]
pub extern "C" fn plain_art() {
    RUN.with(|r| {
        if let Some(run) = r.borrow_mut().as_mut() {
            for boss in &mut run.bosses { boss.id = "unpainted-review".into(); }
        }
    });
}

/// Draw the scene's frame at `w`x`h` for knight 1: a checksum of its pixels.
#[unsafe(no_mangle)]
pub extern "C" fn frame(w: u32, h: u32) -> u32 {
    RUN.with(|r| {
        let r = r.borrow();
        let Some(run) = r.as_ref() else {
            return 0;
        };
        let img = arena::frame_for(run, w as i32, h as i32, Some(1));
        img.rgb_bytes().iter().fold(0x811c_9dc5u32, |h, &b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        })
    })
}

/// As `frame`, drawn live (`arena::frame_for_live`), as a client draws:
/// a torch step's scenery is drawn a stage a frame while the last shows.
#[unsafe(no_mangle)]
pub extern "C" fn frame_live(w: u32, h: u32) -> u32 {
    RUN.with(|r| {
        let r = r.borrow();
        let Some(run) = r.as_ref() else {
            return 0;
        };
        let img = arena::frame_for_live(run, w as i32, h as i32, Some(1));
        img.rgb_bytes().iter().fold(0x811c_9dc5u32, |h, &b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
    })
}

/// Set up scene `kind` on the wire: the whole first snapshot made and
/// taken. Its size in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn wire_scene(kind: u32) -> u32 {
    let run = match kind {
        0 => Run::new(7, 1, None),
        1 => arena::art_v2::testing::kingdom(),
        2 => arena::art_v2::testing::fight(),
        3 => {
            let mut r=Run::cinder_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        4 => {
            let mut r=Run::latch_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        5 => {
            let mut r=Run::mossbell_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        6 => {
            let mut r=Run::brasswake_for_test();
            for id in 2..=4 { r.join_seat(id,&format!("Seat {id}")); }
            r.step(&std::collections::BTreeMap::from([(1, angel_delve::rules::Input {interact:true,..Default::default()})]));
            r
        },
        7 => Run::rafterwake_review_for_test("drop-tell"),
        8 => pennon_scene(),
        9..=20 | 23..=36 => arena::art_v2::testing::boss(SCENES[kind as usize]),
        21 => arena::art_v2::testing::hall(),
        22 => arena::art_v2::testing::book_hop(),
        37 => forest_scene(),
        38 => drift_scene(),
        39..=43 => roundtable_scene(SCENES[kind as usize]),
        _ => return 0,
    };
    let mut publisher = wire::Publisher::default();
    let sent = publisher.publish(&run).expect("the run is a tree");
    let mut writer = Writer::default();
    let mut body = Vec::new();
    writer.value(&mut body, &sent);
    let mut reader = Reader::default();
    let tree = reader.value(&mut Bytes::new(&body)).expect("it reads");
    let size = body.len() as u32;
    WIRE.with(|w| *w.borrow_mut() = Some(Wire { run, publisher, sent, writer, tree, reader, body }));
    size
}

/// The server's half of a tick: step, publish, the patch against what was
/// sent. Its size in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn wire_host() -> u32 {
    WIRE.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return 0 };
        w.run.step(&Default::default());
        let tree = w.publisher.publish(&w.run).expect("the run is a tree");
        w.body.clear();
        if let Some(patch) = diff(&w.sent, &tree) {
            w.writer.patch(&mut w.body, &patch);
        }
        w.sent = tree;
        w.body.len() as u32
    })
}

/// The client's half: the patch read and applied, the tree read back into
/// a run. The run's tick, so nothing is optimised away.
#[unsafe(no_mangle)]
pub extern "C" fn wire_client() -> u32 {
    WIRE.with(|w| {
        let mut w = w.borrow_mut();
        let Some(w) = w.as_mut() else { return 0 };
        if !w.body.is_empty() {
            let patch = w.reader.patch(&mut Bytes::new(&w.body)).expect("the patch reads");
            apply(&mut w.tree, patch).expect("the patch fits");
        }
        wire::read(&w.tree).map_or(0, |run| run.tick as u32)
    })
}
