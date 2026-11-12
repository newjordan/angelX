//! Proof for the room encounters: every game of the pool in the real
//! renderer, and a soak that plays every dealt game to its end.
//!
//!   ANGEL_ENCOUNTER_REEL=<dir> [REEL_ONLY=bannerpeak,knight_racers] cargo test -p angel-delve --lib write_encounter_reels -- --ignored --nocapture
//!   ANGEL_ENCOUNTER_SOAK=1 cargo test -p angel-delve --lib encounter_soak -- --ignored --nocapture
//!
//! The reels change no game code and write only under the directory they are
//! given: `<dir>/<game>/f0000.png ...` (seat one's own screen, 15 frames a
//! second); `make_gifs.py` beside them makes the GIFs and the contact sheet.
use super::*;
use crate::render::overworld::{Img, arena::frame_for};
use std::path::PathBuf;

type Keys = BTreeMap<u32, Input>;

/// What the bots press this tick, for whatever the hall is playing.
fn bots(run: &Run) -> Keys {
    let Some(live) = run.encounter.as_ref() else { return Keys::new() };
    if live.end.is_some() {
        return Keys::new();
    }
    match live.kind {
        Kind::WaxMaze => crate::rules::wax_maze_bot::keys(run),
        Kind::BelfryShoot => crate::rules::belfry_reel::keys(run),
        Kind::PowderRoom => crate::rules::powder_reel::keys(run),
        Kind::BookHop => crate::rules::hop_reel::keys(run),
        Kind::ShaftRun | Kind::GriffinRun | Kind::TorchlightTouge => match run.race3d.as_ref().filter(|r| !r.done) {
            Some(r) if !r.titled() => r.riders.keys().map(|&id| (id, crate::rules::race3d::tests::autopilot(run, id))).collect(),
            _ => Keys::new(),
        },
        Kind::KnightsWish => Keys::new(),
        k if k.adventure().is_some() => run.roundtable_inputs(),
        // The arcade: the rounds are the knights' own; a bot walks and taps.
        Kind::Arcade(_) => run
            .players
            .keys()
            .map(|&id| {
                let t = run.tick as i64 + i64::from(id) * 7;
                let input = Input { move_x: [1, 0, -1, 0][(t / 20 % 4) as usize], move_y: [0, 1, 0, -1][(t / 20 % 4) as usize], fire: t % 9 == 0, ..Input::default() };
                (id, input)
            })
            .collect(),
        _ => Keys::new(),
    }
}

/// A run of `seats` knights on floor two with `kind` pinned to the folly hall,
/// the party at the door and every knight with gold to stake.
fn dealt(seats: u32, kind: Kind) -> (Run, usize) {
    let mut run = Run::new(41, 1, None);
    for id in 2..=seats {
        run.join_seat(id, &format!("Knight {id}"));
    }
    run.descend_for_test();
    run.enemies.clear();
    for (id, name) in [(1, "Aldric"), (2, "Brienne"), (3, "Cedric"), (4, "Dara")] {
        if let Some(h) = run.players.get_mut(&id) {
            h.name = name.into();
        }
    }
    run.pick_folly();
    let index = run.folly_room.expect("a hall on floor two");
    run.encounter_rooms = vec![(index, kind)];
    run.dare = None;
    for h in run.players.values_mut() {
        h.carried.add(Spoil::Gold, 200);
    }
    (run, index)
}

/// Make `kind`'s game end its own way (an objective met, the line crossed, a
/// round won, a winner), so the payout shows.
fn finish(run: &mut Run, kind: Kind) {
    // A round that ended before the reel asked (a lost one) has nothing left to make end.
    if run.encounter.is_none() || (matches!(kind, Kind::Arcade(_)) && run.pastimes.heat.is_none()) {
        return;
    }
    match kind {
        Kind::Lanternward => run.lanternward.as_mut().unwrap().room = 1,
        Kind::Moonfall => run.moonfall.as_mut().unwrap().world.set(moonfall::state::INK_TAKEN),
        Kind::Bannerpeak => run.bannerpeak.as_mut().unwrap().banners = 1,
        Kind::Griffinwing => run.griffinwing.as_mut().unwrap().stage = 1,
        Kind::Lightgallop => run.lightgallop.as_mut().unwrap().posts = 1,
        Kind::KnightRacers => {
            let s = run.knight_racers.as_mut().unwrap();
            let ids: Vec<u32> = s.cars.iter().filter(|(_, c)| !c.cpu).map(|(&id, _)| id).collect();
            for (n, id) in ids.iter().enumerate() {
                s.cars.get_mut(id).unwrap().finish = Some(50.0 + n as f32 * 3.0);
            }
            s.finished = true;
        }
        Kind::KnightsWish => {
            let s = run.deathmatch.as_mut().unwrap();
            let first = *s.fighters.keys().next().unwrap();
            s.fighters.get_mut(&first).unwrap().kills = 10;
            s.winner = Some(first);
        }
        Kind::ShaftRun | Kind::GriffinRun | Kind::TorchlightTouge => {
            let race = run.race3d.as_mut().unwrap();
            let t = race.t;
            for (n, r) in race.riders.values_mut().enumerate() {
                r.finish = Some(t + n as u32 * 6);
            }
            race.done = true;
        }
        Kind::Arcade(game) => {
            let heat = run.pastimes.heat.as_ref().unwrap().clone();
            let mut done = heat.done.clone();
            for (n, id) in heat.seats.iter().enumerate() {
                done.insert(*id, pastimes::Finish { progress: game.need(), points: Some(900 - n as u16 * 120) });
            }
            let heat = pastimes::Heat { done, ..heat };
            run.pastimes.round = None;
            run.pastimes.heats.clear();
            run.pastimes.heat = None;
            run.heat_to_encounter(&heat);
        }
        Kind::WaxMaze => run.folly.as_mut().unwrap().t = folly::TITLE + folly::MAZE_TIME - 1,
        Kind::BelfryShoot => run.bats.as_mut().unwrap().t = folly::TITLE + folly::BAT_TIME - 1,
        Kind::PowderRoom => run.powder.as_mut().unwrap().t = folly::TITLE + powder::POWDER_TIME - 1,
        Kind::BookHop => run.hop.as_mut().unwrap().t = folly::TITLE + hop::HOP_TIME - 1,
    }
}

/// Whether the engine this game's rules need is not in this build.
fn needs_engine(kind: Kind) -> bool {
    matches!(kind, Kind::KnightsWish)
}

fn save(dir: &PathBuf, n: usize, sheet: &Img) {
    image::save_buffer(dir.join(format!("f{n:04}.png")), &sheet.rgb_bytes(), sheet.w as u32, sheet.h as u32, image::ColorType::Rgb8).unwrap();
}

fn slug(kind: Kind) -> String {
    match kind {
        Kind::Arcade(g) => format!("arcade_{}", format!("{g:?}").to_lowercase()),
        other => serde_json::to_string(&other).unwrap().trim_matches('"').to_string(),
    }
}

/// One game's reel: the hall at its door, the game opening with its card, the
/// game running under the bots, the game made to end, its payout card, and the
/// hall after, cleared. Every other tick is a frame.
fn reel(root: &PathBuf, kind: Kind, seats: u32) {
    let dir = root.join(slug(kind));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (mut run, index) = dealt(seats, kind);
    let mut n = 0;
    let tick = |run: &mut Run, keys: &Keys, n: &mut usize, every_other: bool| {
        if needs_engine(kind) && run.encounter.is_some() {
            run.tick_encounter(keys);
            run.tick += 1;
        } else {
            run.step(keys);
        }
        if !every_other || run.tick % 2 == 0 {
            save(&dir, *n, &frame_for(run, 384, 224, Some(1)));
            *n += 1;
        }
    };
    // At the door: the room before the hall.
    for _ in 0..16 {
        tick(&mut run, &Keys::new(), &mut n, true);
    }
    run.enter_for_test(index);
    if kind == Kind::KnightsWish {
        // Past its own title, as the crossroads' shots do: the arena is what is drawn.
        run.deathmatch.as_mut().unwrap().ticks = deathmatch::TITLE + 1;
    }
    // The card, then the game under its bots. (An arcade round an idle table loses in a few
    // seconds is shown for two of them and then won, so its payout card shows as well.)
    let arcade = matches!(kind, Kind::Arcade(_));
    for _ in 0..(if arcade { 60 } else { encounter::TITLE_TICKS + 90 }) {
        let keys = bots(&run);
        tick(&mut run, &keys, &mut n, true);
    }
    // And on a little longer, as a table would play it. (Lanternward's rules are
    // the cockpit's engine's, which this build has not: its reel shows the moments the
    // engine recorded, one after another, in the hall.)
    let moments = ["slash3", "lamp-lit", "gallery-solved", "lever", "beacon"];
    for t in 0..(if arcade { 0 } else { 150 }) {
        if kind == Kind::Lanternward && t % 30 == 0 {
            let name = moments[(t / 30) as usize % moments.len()];
            if let Some(s) = crate::rules::lanternward::moment_session(name, 1) {
                run.lanternward = Some(s);
            }
        }
        let keys = bots(&run);
        tick(&mut run, &keys, &mut n, true);
    }
    if run.encounter.is_none() || run.encounter.is_some_and_ended() {
        println!("{kind:?}: ended on its own");
    } else {
        finish(&mut run, kind);
    }
    let mut guard = 0;
    while run.encounter.is_some() && guard < 40 * 30 {
        let keys = bots(&run);
        tick(&mut run, &keys, &mut n, true);
        guard += 1;
    }
    for _ in 0..30 {
        tick(&mut run, &Keys::new(), &mut n, true);
    }
    println!("{}: {n} frames, cleared {}", slug(kind), run.dungeon.rooms[index].cleared);
}

trait EndedExt {
    fn is_some_and_ended(&self) -> bool;
}
impl EndedExt for Option<Live> {
    fn is_some_and_ended(&self) -> bool {
        self.as_ref().is_some_and(|l| l.end.is_some())
    }
}

#[test]
#[ignore = "writes the room encounters' proof reels"]
fn write_encounter_reels() {
    let Some(root) = std::env::var_os("ANGEL_ENCOUNTER_REEL") else { return };
    let root = PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let only = std::env::var("REEL_ONLY").unwrap_or_default();
    for kind in pool() {
        if !only.is_empty() && !only.split(',').any(|x| x == slug(kind)) {
            continue;
        }
        reel(&root, kind, if kind == Kind::KnightsWish { 3 } else { 2 });
    }
}

/// Every game dealt on every floor to every party, played to its end: no
/// panic, no stuck room, health and stakes as they should be.
#[test]
#[ignore = "the encounter soak: 50 seeds, floors one to six, one to four knights"]
fn encounter_soak() {
    if std::env::var_os("ANGEL_ENCOUNTER_SOAK").is_none() {
        return;
    }
    let seeds: u64 = std::env::var("SOAK_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(50);
    let (mut played, mut paid, mut lost, mut dry, mut by_kind) = (0u32, 0u32, 0u32, 0u32, BTreeMap::<String, u32>::new());
    for seed in 1..=seeds {
        let knights = 1 + (seed % 4) as u32;
        let mut run = Run::new(seed * 7919, 1, None);
        for id in 2..=knights {
            run.join_seat(id, &format!("Knight {id}"));
        }
        run.enemies.clear();
        run.pick_folly();
        run.pick_encounters();
        for floor in 1..=6u32 {
            let halls: Vec<usize> = run.encounter_rooms.iter().map(|&(i, _)| i).collect();
            if halls.is_empty() {
                // A floor whose ordinary rooms are all camps deals no game.
                dry += 1;
            }
            for index in halls {
                for h in run.players.values_mut() {
                    h.carried.add(Spoil::Gold, 100);
                    h.hp = h.max_hp;
                }
                let hp: BTreeMap<u32, u32> = run.players.iter().map(|(&id, h)| (id, h.hp)).collect();
                let kind = run.encounter_at(index).unwrap();
                if run.at != 0 {
                    run.enter_for_test(0);
                }
                run.enter_for_test(index);
                let live = run.encounter.as_ref().unwrap_or_else(|| panic!("seed {seed} floor {floor}: {kind:?} did not open"));
                let kind = live.kind;
                assert!(run.barred() && run.valid_snapshot(), "seed {seed} floor {floor}: {kind:?}");
                let stuck = Kind::cap(kind) + encounter::PAYOUT_TICKS + 4 * 30 * 30;
                let mut n = 0;
                while run.encounter.is_some() {
                    let keys = bots(&run);
                    if needs_engine(kind) {
                        // The free-for-all's rules are the cockpit's engine's: the room's clock alone.
                        finish(&mut run, kind);
                        run.tick_encounter(&keys);
                    } else {
                        run.step(&keys);
                        // A campaign the bots cannot finish in a minute is cut at its cap.
                        if n == 1800 && kind.co_op() {
                            run.encounter.as_mut().unwrap().t = kind.cap() - 1;
                        }
                    }
                    n += 1;
                    assert!(n < stuck, "seed {seed} floor {floor}: {kind:?} is stuck after {n} ticks (phase {:?}, hp {:?}, live {:?})", run.phase, run.players.values().map(|h| h.hp).collect::<Vec<_>>(), run.encounter.as_ref().map(|e| (e.t, e.end.is_some())));
                    if n % 600 == 0 {
                        assert!(run.valid_snapshot(), "seed {seed} floor {floor}: {kind:?} invalid at {n}");
                    }
                }
                assert!(run.dungeon.rooms[index].cleared && !run.barred(), "seed {seed} floor {floor}: {kind:?} left the room shut");
                assert!(run.valid_snapshot(), "seed {seed} floor {floor}: {kind:?} after");
                assert_ne!(run.phase, Phase::Wiped);
                for (id, h) in &run.players {
                    assert!(h.hp >= 1 && h.hp == hp[id], "seed {seed} floor {floor}: {kind:?}: knight {id} hp {} was {}", h.hp, hp[id]);
                }
                played += 1;
                *by_kind.entry(slug(kind)).or_default() += 1;
                if run.bank.iter().any(|h| h.why == "encounter prize") {
                    paid += 1;
                } else {
                    lost += 1;
                }
                run.bank.clear();
            }
            if floor < 6 {
                run.descend_for_test();
            }
        }
    }
    println!("soak: {seeds} seeds, {played} games played to the end, {paid} paid, {lost} unpaid, {dry} floors dealt none");
    println!("{by_kind:?}");
}
