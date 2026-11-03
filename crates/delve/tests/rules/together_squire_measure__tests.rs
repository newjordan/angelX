//! Opt-in, paired squire measurements. No host, synthetic arena, HP overrides,
//! removed waves/hazards, or alternate party AI. Artifacts live outside git.
use super::*;
use crate::render::overworld::arena;
use crate::rules::balance_sim_tests::bot_input;
use serde::Serialize;
use std::path::Path;
use std::time::Instant;

const SEEDS: [u64; 3] = [11, 29, 47];
const CAP_SECONDS: u32 = 150;
const SHOT_TICK: u32 = 2 * HZ;
const COMMAND: &str = "cargo test --manifest-path crates/Cargo.toml -p angel-delve --profile fast --lib rules::squire_company::field::measure_tests::export_squire_measurements -- --ignored --exact --nocapture";

#[derive(Default, Serialize)]
struct Mean {
    sum: f64,
    samples: u64,
}
impl Mean {
    fn add(&mut self, x: f32) {
        assert!(x.is_finite());
        self.sum += f64::from(x);
        self.samples += 1;
    }
    fn value(&self) -> Option<f64> {
        (self.samples > 0).then(|| self.sum / self.samples as f64)
    }
}

#[derive(Serialize)]
struct Shot {
    path: String,
    tick: u64,
    fight_tick: u64,
    transport_tick: u32,
    phase: String,
    active_foes: usize,
    hostile_shots: usize,
    knights_alive: usize,
    squires_alive: usize,
    knights_in_frame: usize,
    squires_in_frame: usize,
}

#[derive(Serialize)]
struct Record {
    seed: u64,
    floor: u32,
    room: usize,
    variant: &'static str,
    room_width: f32,
    room_height: f32,
    initial_foes: usize,
    initial_foe_hp: u32,
    initial_party_hp: u32,
    initial_squires: usize,
    initial_squire_stats: Vec<serde_json::Value>,
    outcome: String,
    censored: bool,
    elapsed_seconds: f64,
    simulation_seconds: f64,
    ttk_seconds: Option<f64>,
    actionable_seconds: f64,
    waiting_seconds: f64,
    hitstop_seconds: f64,
    actionable_hero_seconds: f64,
    waiting_hero_seconds: f64,
    observed_hp_loss: u64,
    observed_hp_gain: u64,
    net_hp_loss: i64,
    actual_damage: Option<u64>,
    health_cap_changed: bool,
    healing_received: u64,
    deaths: usize,
    squires_down: usize,
    foes_remaining: usize,
    hp_remaining: u32,
    engagement_distance: Mean,
    squire_owner_distance: Mean,
    step_us_mean: f64,
    step_us_p95: f64,
    shots: Vec<Shot>,
}

fn active(e: &Enemy) -> bool {
    e.hp > 0 && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH && !e.dormant
}

/// Generate with six players already present, so descent uses the real party
/// layout/scaling. Fresh intro outfits, not a saved win's health/deck bonuses.
fn scenario(seed: u64, floor: u32) -> Run {
    let mut run = Run::new(seed, 700 + seed, None);
    run.players.clear();
    for id in 1..=6 {
        run.players
            .insert(id, Hero::new(&format!("K{id}"), 24.0, 20.0));
        run.outfit(
            id,
            knights::knight(["percival", "lynette", "gareth", "galahad"][(id - 1) as usize % 4])
                .unwrap(),
        );
    }
    while run.dungeon.depth < floor {
        let before = run.dungeon.depth;
        run.descend_for_test();
        assert!(run.dungeon.depth > before, "descent stalled at {before}");
    }
    assert_eq!(run.dungeon.depth, floor);
    assert_eq!(run.party_size(), 6);
    assert!(run.tune.set("foe_shots", 50.0));
    assert!(run.tune.set("foe_shot_speed", 1.5));
    assert_eq!(run.tune.foe_shots, 50);
    assert_eq!(run.tune.foe_shot_speed, 1.5);
    let rooms: Vec<_> = run
        .dungeon
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RoomKind::Fight && !r.cleared)
        .map(|(i, _)| i)
        .collect();
    assert!(
        !rooms.is_empty(),
        "seed {seed} floor {floor}: no generated Fight room"
    );
    let room = rooms[(seed as usize * 7 + 3) % rooms.len()];
    run.enter(room, Some(2));
    // Apply the same production stakes/cap normalization as the first play
    // tick, before taking the HP baseline (not an artificial HP assignment).
    run.set_stakes();
    assert_eq!(run.room().kind, RoomKind::Fight);
    assert_eq!(run.phase, Phase::Fighting);
    assert!(!run.enemies.is_empty());
    assert!(run.enemies.iter().all(|e| e.hp > 0 && e.hp == e.max_hp));
    assert_eq!(run.players.len(), 6);
    assert!(run.players.values().all(|h| h.hp > 0 && !h.absent));
    assert!(run.squire_field.companions.is_empty());
    assert!(run.valid_snapshot(), "invalid generated input scenario");
    run
}

fn add_squires(run: &mut Run) {
    for (id, kind) in (1..=6).zip([
        Kind::Ninja,
        Kind::Healer,
        Kind::Ranger,
        Kind::Shield,
        Kind::Alchemist,
        Kind::Shield,
    ]) {
        run.home.squire_company.rosters.insert(
            run.players[&id].name.clone(),
            Roster {
                squires: vec![Squire {
                    id,
                    name: format!("S{id}"),
                    kind,
                    level: 3,
                    trained_on: None,
                }],
                active: Some(id),
                ..Default::default()
            },
        );
    }
    run.deploy_company_squires();
    assert_eq!(run.squire_field.companions.len(), 6);
    for (&owner, s) in &run.squire_field.companions {
        assert_eq!(s.squire.level, 3);
        assert_eq!(s.actor.summoner, Some(owner));
        assert!(s.actor.hp > 0);
    }
}

fn capture(
    run: &Run,
    start: u64,
    transport_tick: u32,
    seed: u64,
    floor: u32,
    variant: &str,
    dir: &Path,
) -> Shot {
    assert_eq!(
        run.phase,
        Phase::Fighting,
        "never export a cleared/wiped showcase"
    );
    assert_eq!(run.dungeon.depth, floor);
    assert_eq!(run.room().kind, RoomKind::Fight);
    assert!(
        run.enemies.iter().any(active),
        "capture must show an active fight"
    );
    assert_eq!(run.players.len(), 6);
    let image = arena::frame_for(run, 768, 448, Some(1));
    assert_eq!((image.w, image.h), (768, 448));
    let path = dir.join(format!(
        "floor{floor}-seed{seed}-{variant}-tick{}-focus1.png",
        run.tick - start
    ));
    image::RgbaImage::from_raw(768, 448, image.rgba_scaled(1))
        .unwrap()
        .save(&path)
        .unwrap();
    let in_frame = |x, y| {
        let (x, y) = arena::frame_px(run, 768, 448, Some(1), x, y);
        (0..768).contains(&x) && (0..448).contains(&y)
    };
    let alive: Vec<_> = run.players.values().filter(|h| h.hp > 0).collect();
    let squires: Vec<_> = run.squire_field
        .companions
        .values()
        .filter(|s| !s.actor.down())
        .collect();
    let shot = Shot {
        path: path.display().to_string(),
        tick: run.tick,
        fight_tick: run.tick - start,
        transport_tick,
        phase: format!("{:?}", run.phase),
        active_foes: run.enemies.iter().filter(|e| active(e)).count(),
        hostile_shots: run
            .projectiles
            .iter()
            .filter(|p| p.hostile && p.ttl > 0)
            .count(),
        knights_alive: alive.len(),
        squires_alive: squires.len(),
        knights_in_frame: alive.iter().filter(|h| in_frame(h.x, h.y)).count(),
        squires_in_frame: squires
            .iter()
            .filter(|s| in_frame(s.actor.x, s.actor.y))
            .count(),
    };
    assert_eq!(
        shot.knights_alive, 6,
        "capture early enough for all six knights"
    );
    assert_eq!(shot.knights_in_frame, shot.knights_alive);
    assert_eq!(shot.squires_in_frame, shot.squires_alive);
    assert_eq!(
        shot.squires_alive,
        run.squire_field.companions.len(),
        "capture before companion casualties"
    );
    shot
}

fn measure(mut run: Run, seed: u64, floor: u32, variant: &'static str, dir: &Path) -> Record {
    let mut out = Record {
        seed,
        floor,
        room: run.at,
        variant,
        room_width: run.room().width(),
        room_height: run.room().height(),
        initial_foes: run.enemies.len(),
        initial_foe_hp: run.enemies.iter().map(|e| e.max_hp).sum(),
        initial_party_hp: run.players.values().map(|h| h.hp).sum(),
        initial_squires: run.squire_field.companions.len(),
        initial_squire_stats: run.squire_field
            .companions
            .iter()
            .map(|(&owner, s)| {
                serde_json::json!({
                    "owner": owner, "role": s.squire.kind.name(), "level": s.squire.level,
                    "hp": s.actor.hp, "max_hp": s.actor.max_hp, "power": s.actor.power,
                })
            })
            .collect(),
        outcome: String::new(),
        censored: true,
        elapsed_seconds: 0.0,
        simulation_seconds: 0.0,
        ttk_seconds: None,
        actionable_seconds: 0.0,
        waiting_seconds: 0.0,
        hitstop_seconds: 0.0,
        actionable_hero_seconds: 0.0,
        waiting_hero_seconds: 0.0,
        observed_hp_loss: 0,
        observed_hp_gain: 0,
        net_hp_loss: 0,
        actual_damage: None,
        health_cap_changed: false,
        healing_received: 0,
        deaths: 0,
        squires_down: 0,
        foes_remaining: 0,
        hp_remaining: 0,
        engagement_distance: Mean::default(),
        squire_owner_distance: Mean::default(),
        step_us_mean: 0.0,
        step_us_p95: 0.0,
        shots: vec![],
    };
    stakes::HEALED.with(|h| *h.borrow_mut() = [0; 4]);
    let start = run.tick;
    let mut steps = Vec::new();
    let (mut actionable, mut waiting, mut hold, mut hero_action, mut hero_wait) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut fell = std::collections::BTreeSet::new();
    let mut constant_max_hp = true;
    for tick in 0..CAP_SECONDS * HZ {
        assert_eq!(run.phase, Phase::Fighting);
        let foes: Vec<_> = run.enemies.iter().filter(|e| active(e)).collect();
        let inputs: BTreeMap<_, _> = run
            .players
            .keys()
            .map(|&id| (id, bot_input(&run, id)))
            .collect();
        assert!(
            inputs.values().all(|i| i.valid()),
            "invalid bot input must never produce a score"
        );
        let held = run.hitstop.left > 0;
        let previous_tick = run.tick;
        let before: BTreeMap<_, _> = run
            .players
            .iter()
            .map(|(&id, h)| (id, (h.hp, h.max_hp)))
            .collect();
        // Mutually exclusive room time bins. Hero time counts actionable AI
        // intent (fire/swing/cast/bomb/ult), not a claim that every shot lands.
        if run.hitstop.left > 0 {
            hold += 1;
        } else if foes.is_empty() {
            waiting += 1;
        } else {
            actionable += 1;
        }
        for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
            let nearest = foes
                .iter()
                .map(|e| (e.x - hero.x).hypot(e.y - hero.y))
                .min_by(f32::total_cmp);
            if let Some(d) = nearest {
                out.engagement_distance.add(d);
            }
            let i = inputs[&id];
            if run.hitstop.left == 0 && (i.fire || i.swing || i.cast > 0 || i.bomb || i.ult) {
                hero_action += 1;
            } else {
                hero_wait += 1;
            }
        }
        for (&id, s) in run.squire_field
            .companions
            .iter()
            .filter(|(_, s)| !s.actor.down())
        {
            let h = &run.players[&id];
            out.squire_owner_distance
                .add((s.actor.x - h.x).hypot(s.actor.y - h.y));
        }
        let now = Instant::now();
        run.step(&inputs);
        steps.push(now.elapsed().as_secs_f64() * 1e6);
        assert_eq!(
            run.tick,
            previous_tick + u64::from(!held),
            "invalid/non-advancing simulation outside hit-stop"
        );
        for (&id, hero) in &run.players {
            let (hp, max_hp) = before[&id];
            constant_max_hp &= max_hp == hero.max_hp;
            out.observed_hp_loss += u64::from(hp.saturating_sub(hero.hp));
            out.observed_hp_gain += u64::from(hero.hp.saturating_sub(hp));
            if hero.hp == 0 {
                fell.insert(id);
            }
        }
        if tick + 1 == SHOT_TICK {
            out.shots
                .push(capture(&run, start, tick + 1, seed, floor, variant, dir));
        }
        if run.phase != Phase::Fighting || run.players.values().all(|h| h.hp == 0) {
            break;
        }
    }
    let ticks = steps.len() as u64;
    out.simulation_seconds = (run.tick - start) as f64 / f64::from(HZ);
    let seconds = |n| n as f64 / f64::from(HZ);
    out.elapsed_seconds = seconds(ticks);
    out.actionable_seconds = seconds(actionable);
    out.waiting_seconds = seconds(waiting);
    out.hitstop_seconds = seconds(hold);
    out.actionable_hero_seconds = seconds(hero_action);
    out.waiting_hero_seconds = seconds(hero_wait);
    out.hp_remaining = run.players.values().map(|h| h.hp).sum();
    out.net_hp_loss = i64::from(out.initial_party_hp) - i64::from(out.hp_remaining);
    out.healing_received = stakes::HEALED.with(|h| h.borrow().iter().map(|&x| u64::from(x)).sum());
    // The existing mend counter catches healing and damage in the same tick,
    // unlike the signed per-tick HP observations. Refuse an exact damage
    // claim if a card changed a health cap during this encounter.
    out.health_cap_changed = !constant_max_hp;
    if constant_max_hp && out.net_hp_loss >= 0 {
        out.actual_damage = Some(out.net_hp_loss as u64 + out.healing_received);
    }
    out.deaths = fell.len();
    out.squires_down = run.squire_field
        .companions
        .values()
        .filter(|s| s.actor.down())
        .count();
    out.foes_remaining = run.enemies.iter().filter(|e| e.hp > 0).count();
    out.outcome = if run.phase == Phase::Wiped || run.players.values().all(|h| h.hp == 0) {
        "wipe"
    } else if run.phase != Phase::Fighting && out.foes_remaining == 0 {
        out.censored = false;
        out.ttk_seconds = Some(out.elapsed_seconds);
        "clear"
    } else {
        assert_eq!(
            ticks,
            u64::from(CAP_SECONDS * HZ),
            "unexpected non-terminal phase {:?}",
            run.phase
        );
        "timeout"
    }
    .into();
    assert_eq!(actionable + waiting + hold, ticks);
    assert_eq!(out.censored, out.ttk_seconds.is_none());
    assert!(out.elapsed_seconds > 0.0);
    assert_eq!(
        out.shots.len(),
        1,
        "fight ended before its required live capture"
    );
    assert!(out.engagement_distance.samples > 0);
    assert_eq!(
        out.squire_owner_distance.samples > 0,
        out.initial_squires > 0
    );
    out.step_us_mean = steps.iter().sum::<f64>() / steps.len() as f64;
    steps.sort_by(f64::total_cmp);
    out.step_us_p95 = steps[((steps.len() - 1) as f64 * 0.95).round() as usize];
    out
}

fn number(n: Option<f64>) -> String {
    n.map(|x| format!("{x:.3}")).unwrap_or_default()
}

#[test]
#[ignore = "exports 24 real paired fights and PNGs; run explicitly with --ignored --exact"]
fn export_squire_measurements() {
    assert_eq!(
        crate::settings::get(),
        crate::settings::Settings::default(),
        "run with default Delve settings (unset ANGEL_DELVE_FEEL/ANGEL_DELVE_OPEN)"
    );
    let root = &std::path::PathBuf::from(std::env::var_os("DELVE_COMBAT_FEEL_OUT").expect("set DELVE_COMBAT_FEEL_OUT to the output directory"));
    let shots = root.join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    let mut records = Vec::new();
    for floor in 3..=6 {
        for seed in SEEDS {
            let base = scenario(seed, floor);
            let mut with = base.clone();
            add_squires(&mut with);
            // Companion enlistment must not alter party, foes, hazards, RNG,
            // layout or tuning. Compare the entire serialized state except
            // the two squire-owned fields.
            let strip = |run: &Run| {
                let mut v = serde_json::to_value(run).unwrap();
                v["home"]["squires"] = serde_json::Value::Null;
                v["squires"] = serde_json::Value::Null;
                v
            };
            assert_eq!(strip(&base), strip(&with), "pair differs outside squires");
            for (run, variant) in [(base, "without-squires"), (with, "with-squires")] {
                let r = measure(run, seed, floor, variant, &shots);
                println!(
                    "floor={} seed={} {} {} elapsed={:.3}s ttk={} hp_loss={} actual_damage={:?} waiting={:.3}s actionable={:.3}s distance={} leash={}",
                    r.floor,
                    r.seed,
                    r.variant,
                    r.outcome,
                    r.elapsed_seconds,
                    number(r.ttk_seconds),
                    r.net_hp_loss,
                    r.actual_damage,
                    r.waiting_seconds,
                    r.actionable_seconds,
                    number(r.engagement_distance.value()),
                    number(r.squire_owner_distance.value())
                );
                records.push(r);
            }
        }
    }
    assert_eq!(records.len(), 24);
    let mut md = String::from("# Squire combat feel: paired real-room measurements\n\n");
    md.push_str("## Method\n\nThree deterministic seeds (11, 29, 47), floors 3–6, six fresh intro-outfitted knights (Percival/Lynette/Gareth/Galahad/Percival/Lynette). Six-player generated floors via `descend_for_test`; deterministic `(seed*7+3)%Fight_rooms` selection. No foe/hero HP overrides, removed foes, calm helper, disabled waves, traps, or rocks. Both variants clone the exact same entered encounter; only roster/companions differ (asserted). Companion roles: Ninja, Healer, Ranger, Shield, Alchemist, Shield; all level 3, deployed through the production API. These are fresh knights, not a progression or whole-run balance study.\n\n");
    md.push_str("Monster bullets use `Tune::set(\"foe_shots\", 50)` and `Tune::set(\"foe_shot_speed\", 1.5)` (150% speed). All other tuning remains unchanged. Controls reuse `rules::balance_sim_tests::bot_input` without modifications. `Run::step` advances the real simulation, including hit-stop (transport calls count toward elapsed time; the separate simulation clock pauses during hit-stop). Production `set_stakes` normalizes fresh kit HP caps before the measurement baseline. 150-second cap. Invalid inputs assert rather than producing a zero score. TTK is present only on a clear; wipe/timeout rows are censored at their actual elapsed time, never counted as zero TTK.\n\n");
    md.push_str("Room time partitions into actionable (at least one live non-dormant, post-telegraph foe), waiting (no such foe), and hit-stop (separate priority bin). This measures target availability, not line-of-sight or cooldown readiness. Hero-seconds count live non-stone bot attack intent vs waiting; movement and dodging remain real AI actions. Engagement distance is living-hero nearest active foe, in world units; squire distance is standing companion to its owner, also world units. Means are sample-weighted within each fight.\n\n");
    md.push_str("Net HP loss = initial minus final party HP. Observed loss/gain sums signed per-tick changes (may hide simultaneous healing/damage). Actual damage = net loss + production `stakes::HEALED` receipts when health caps stay constant; otherwise null (see health_cap_changed). Summary damage means include only non-null rows, with their sample count shown. This is effective HP removed, not raw pre-mitigation attack damage. Deaths count distinct knights observed at zero HP. Tick costs time only `Run::step`, exclude AI/rendering, and are nondeterministic wall-clock diagnostics.\n\n");
    md.push_str("PNGs are unmodified `arena::frame_for(run, 768, 448, Some(1))` after 60 transport steps (two seconds; JSON records the simulation tick too), from the measured running simulation, not a constructed showcase. Assertions require correct floor/Fight room, phase Fighting, active enemies, six live knights, and every living knight/companion centre inside the focus-1 frame. JSON includes shot metadata and deployed squire stats; no actors are moved for art.\n\n## Results\n\n|Floor|Variant|Clears/3|Mean clear TTK s|Mean net HP loss|Mean actual damage (n/3)|Mean waiting s|Mean actionable s|Mean engagement units|Mean squire-owner units|\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for floor in 3..=6 {
        for variant in ["without-squires", "with-squires"] {
            let rows: Vec<_> = records
                .iter()
                .filter(|r| r.floor == floor && r.variant == variant)
                .collect();
            assert_eq!(rows.len(), 3);
            let mean =
                |xs: Vec<f64>| (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64);
            md.push_str(&format!(
                "|{floor}|{variant}|{}/3|{}|{}|{} ({}/3)|{}|{}|{}|{}|\n",
                rows.iter().filter(|r| !r.censored).count(),
                number(mean(rows.iter().filter_map(|r| r.ttk_seconds).collect())),
                number(mean(rows.iter().map(|r| r.net_hp_loss as f64).collect())),
                number(mean(
                    rows.iter()
                        .filter_map(|r| r.actual_damage.map(|x| x as f64))
                        .collect()
                )),
                rows.iter().filter(|r| r.actual_damage.is_some()).count(),
                number(mean(rows.iter().map(|r| r.waiting_seconds).collect())),
                number(mean(rows.iter().map(|r| r.actionable_seconds).collect())),
                number(mean(
                    rows.iter()
                        .filter_map(|r| r.engagement_distance.value())
                        .collect()
                )),
                number(mean(
                    rows.iter()
                        .filter_map(|r| r.squire_owner_distance.value())
                        .collect()
                ))
            ));
        }
    }
    md.push_str("\nClear-only means exclude censored rows; compare clear counts alongside TTK. Three seeds are a small descriptive sample, not a significance claim. Full paired observations, censoring, damage/healing, timing and shot paths follow in JSON/CSV. Baseline means WITHOUT companions; AFTER means WITH companions at the same code revision, not a historical-code comparison.\n\n## Reproduce\n\n```sh\n");
    md.push_str(COMMAND);
    md.push_str("\n```\n\n## Live captures\n\n");
    for r in &records {
        for shot in &r.shots {
            md.push_str(&format!("- Floor {} seed {} {}: [{}]({}) — tick {}, {} active foes, {}/6 knights, {}/{} standing squires.\n", r.floor, r.seed, r.variant,
                Path::new(&shot.path).file_name().unwrap().to_str().unwrap(), shot.path, shot.fight_tick,
                shot.active_foes, shot.knights_alive, shot.squires_alive, r.initial_squires));
        }
    }
    let report = serde_json::json!({
        "schema_version": 1, "command": COMMAND, "seeds": SEEDS, "floors": [3,4,5,6],
        "hz": HZ, "cap_seconds": CAP_SECONDS, "shot_tick": SHOT_TICK,
        "foe_shots": 50, "foe_shot_speed": 1.5, "roles": ["Ninja", "Healer", "Ranger", "Shield", "Alchemist", "Shield"],
        "squire_level": 3, "records": records,
    });
    std::fs::write(
        root.join("METRICS.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    let mut csv = String::from(
        "seed,floor,room,variant,initial_foes,initial_foe_hp,initial_party_hp,initial_squires,outcome,censored,elapsed_seconds,simulation_seconds,ttk_seconds,actionable_seconds,waiting_seconds,hitstop_seconds,actionable_hero_seconds,waiting_hero_seconds,net_hp_loss,observed_hp_loss,observed_hp_gain,actual_damage,health_cap_changed,healing_received,deaths,squires_down,foes_remaining,hp_remaining,engagement_distance,squire_owner_distance,step_us_mean,step_us_p95,shot_path\n",
    );
    for r in &records {
        csv.push_str(&format!("{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{}\n",
            r.seed,r.floor,r.room,r.variant,r.initial_foes,r.initial_foe_hp,r.initial_party_hp,r.initial_squires,
            r.outcome,r.censored,r.elapsed_seconds,r.simulation_seconds,number(r.ttk_seconds),r.actionable_seconds,r.waiting_seconds,r.hitstop_seconds,
            r.actionable_hero_seconds,r.waiting_hero_seconds,r.net_hp_loss,r.observed_hp_loss,r.observed_hp_gain,
            r.actual_damage.map(|x|x.to_string()).unwrap_or_default(),r.health_cap_changed,r.healing_received,r.deaths,r.squires_down,
            r.foes_remaining,r.hp_remaining,number(r.engagement_distance.value()),number(r.squire_owner_distance.value()),
            r.step_us_mean,r.step_us_p95,r.shots[0].path));
    }
    std::fs::write(root.join("METRICS.csv"), csv).unwrap();
    std::fs::write(root.join("METRICS.md"), md).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(root.join("METRICS.json")).unwrap()
        )
        .unwrap()["records"]
            .as_array()
            .unwrap()
            .len(),
        24
    );
    println!(
        "EXPORTED 24 paired records, 24 live PNGs; validity checks passed. Artifacts: {}",
        root.display()
    );
}
