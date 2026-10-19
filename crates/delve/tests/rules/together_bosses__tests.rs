use super::*;
use crate::rules::{EnemyKind, Phase, RoomKind, Run};
use std::collections::BTreeMap;

#[test]
fn the_guardians_pass_the_checker_and_most_floors_have_two() {
    let bosses = builtin();
    assert_eq!(bosses.len(), 10);
    for (pack, guardians) in [
        (Pack::Crypt, 2),
        (Pack::Cavern, 3),
        (Pack::Archive, 2),
        (Pack::Fungal, 2),
        (Pack::Swamp, 1),
    ] {
        assert_eq!(
            bosses.iter().filter(|b| b.only_in == pack).count(),
            guardians,
            "{pack:?}"
        );
    }
    for boss in &bosses {
        let width = boss.art[0].chars().count();
        assert!(
            boss.art.iter().all(|r| r.chars().count() == width),
            "{} art rows align",
            boss.id
        );
        for stage in Stage::ALL {
            let rate = boss.per_second(stage);
            assert!(rate <= stage.caps().0 as f32 + 0.01, "{} fires {rate}/s in {stage:?}", boss.id);
        }
    }
}

#[test]
fn a_greedy_boss_is_clamped_and_slowed_and_bad_words_rejected() {
    let raw = "name Storm\nwhere mines\nhp 99999\nattack spiral shots=4 every=6\nattack ring shots=20 every=45\nart\nHH\n";
    let (boss, notes) = check("storm", raw).unwrap();
    assert_eq!(boss.hp, 900);
    for stage in Stage::ALL {
        let rate = boss.per_second(stage);
        assert!(rate <= stage.caps().0 as f32 + 0.5, "{stage:?} {rate}");
    }
    assert!(notes.iter().any(|n| n.contains("budget")));
    let errors = check("x", "name X\nwhere moon\nattack glitter\n")
        .unwrap_err()
        .errors
        .join(" | ");
    assert!(
        errors.contains("where `moon`") && errors.contains("attack `glitter`"),
        "{errors}"
    );
}

#[test]
fn the_stairs_are_guarded_until_the_boss_falls_and_it_scatters_spoils() {
    let mut run = Run::new(0, 1, None);
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Stairs)
        .unwrap();
    run.enter_for_test(stairs);
    let pack = run.dungeon.pack;
    let boss = run
        .enemies
        .iter()
        .find(|e| e.kind == EnemyKind::Boss)
        .expect("a guardian");
    let def = &run.bosses[usize::from(boss.boss.unwrap())];
    assert_eq!(def.only_in, pack);
    assert!(run.barred());
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    assert!(
        run.projectiles.iter().any(|p| p.hostile),
        "the guardian attacks"
    );
    run.calm_for_test();
    for enemy in &mut run.enemies {
        enemy.hp = 0;
    }
    run.step(&BTreeMap::new());
    assert_eq!(run.phase, Phase::Exploring);
    let spoils = run
        .room()
        .items
        .iter()
        .filter(|i| run.book.get(&i.card).is_some_and(|c| c.is_spoil()))
        .count();
    assert!(spoils >= 3, "the guardian scatters its spoils ({spoils})");
    assert!(run.found_line().is_some_and(|l| l.ends_with("falls")));
}

/// `ANGEL_ARENA_SHOTS=<dir> cargo test write_boss_shots -- --ignored`
#[test]
#[ignore]
fn write_boss_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (seed, name) in (0u64..12).map(|s| (s, s.to_string())) {
        let mut run = Run::new(seed, 1, Some("Squire"));
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        for _ in 0..140 {
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            run.step(&BTreeMap::new());
        }
        let img = crate::render::overworld::arena::frame(&run, 384, 224);
        let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
        out.extend(img.rgb_bytes());
        std::fs::write(
            dir.join(format!(
                "boss_{}_{name}.ppm",
                run.dungeon.pack.name().replace(' ', "_")
            )),
            out,
        )
        .unwrap();
    }
}

#[test]
fn a_delve_meets_one_of_its_packs_guardians_and_keeps_to_it() {
    let mut met = std::collections::BTreeSet::new();
    for raid in 0..16 {
        let mut run = Run::new(3, raid, None);
        run.begin_in(Pack::Crypt);
        let stairs = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Stairs)
            .unwrap();
        run.enter_for_test(stairs);
        let boss = run
            .enemies
            .iter()
            .find_map(|e| e.boss)
            .map(|i| run.bosses[usize::from(i)].id.clone())
            .expect("a guardian in the stairs room");
        // The same delve meets the same one again.
        let mut again = run.clone();
        again.enter_for_test(stairs);
        let twice = again
            .enemies
            .iter()
            .find_map(|e| e.boss)
            .map(|i| again.bosses[usize::from(i)].id.clone());
        assert_eq!(twice.as_deref(), Some(boss.as_str()));
        met.insert(boss);
    }
    assert_eq!(
        met.into_iter().collect::<Vec<_>>(),
        ["the-bone-choir", "waxen-warden"]
    );
}

/// Every boss of every pack, fought to the end by one strong knight in a
/// guarded stairs room: it takes damage, stays inside its room at a finite
/// spot, falls inside a minute, and the room opens after.
/// `cargo test every_boss_can_be_fought -- --nocapture` prints each fight.
#[test]
fn every_boss_can_be_fought_to_the_end_and_the_room_opens() {
    use crate::rules::layout::Pack;
    use crate::rules::{HZ, Input};
    let mut problems = Vec::new();
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
        let count = {
            let mut run = Run::new(7, 1, None);
            let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
            run.enter_for_test(stairs);
            run.pack_bosses_for_test(pack).len()
        };
        for k in 0..count {
            let mut run = Run::new(7, 1, None);
            let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).unwrap();
            run.enter_for_test(stairs);
            let Some(&index) = run.pack_bosses_for_test(pack).get(k) else {
                continue;
            };
            run.enemies.clear();
            run.projectiles.clear();
            let (w, h) = (run.room().width(), run.room().height());
            run.spawn_boss_at_for_test(index, w * 0.7, h * 0.5);
            let name = run.bosses[index].name.clone();
            let hero = run.players.get_mut(&1).unwrap();
            (hero.x, hero.y) = (w * 0.25, h * 0.5);
            hero.invulnerable = 1_000_000;
            hero.bonus.damage = 400;
            let start_hp = run.enemies.iter().find(|e| e.kind == EnemyKind::Boss).map_or(0, |e| e.hp);
            let (mut hurt_by, mut fell_at, mut strayed) = (None, None, None);
            for t in 0..60 * HZ {
                let (hx, hy) = (run.players[&1].x, run.players[&1].y);
                let Some(boss) = run.enemies.iter().find(|e| e.kind == EnemyKind::Boss && e.hp > 0) else {
                    fell_at = Some(t);
                    break;
                };
                if !(boss.x.is_finite() && boss.y.is_finite())
                    || boss.x < -1.0 || boss.y < -1.0 || boss.x > w + 1.0 || boss.y > h + 1.0
                {
                    strayed.get_or_insert((t, boss.x, boss.y));
                }
                if hurt_by.is_none() && boss.hp < start_hp {
                    hurt_by = Some(t);
                }
                if std::env::var_os("BOSS_TRACE").is_some() && t % (5 * HZ) == 0 {
                    println!("  {name} t={t} hp={} at ({:.1},{:.1}) knight ({hx:.1},{hy:.1}) frozen={} stage={}", boss.hp, boss.x, boss.y, boss.frozen, boss.stage);
                }
                // Line up with the boss as a player would, then shoot
                // straight along the long axis (aim is eight ways).
                let sign = |d: f32| if d > 0.5 { 1 } else if d < -0.5 { -1 } else { 0 };
                let (dx, dy) = (boss.x - hx, boss.y - hy);
                let along_x = dx.abs() >= dy.abs();
                let input = Input {
                    move_x: if along_x { 0 } else { sign(dx) },
                    move_y: if along_x { sign(dy) } else { 0 },
                    aim_x: if along_x { sign(dx) } else { 0 },
                    aim_y: if along_x { 0 } else { sign(dy) },
                    fire: true,
                    ..Input::default()
                };
                run.step(&BTreeMap::from([(1, input)]));
            }
            let opened = fell_at.is_some() && {
                run.enemies.retain(|e| e.kind == EnemyKind::Boss);
                for _ in 0..HZ {
                    run.step(&BTreeMap::new());
                }
                run.phase == Phase::Exploring
            };
            println!(
                "{pack:?} {name}: hp {start_hp}, first hurt {hurt_by:?}, fell at {fell_at:?}, strayed {strayed:?}, room opens {opened}"
            );
            if hurt_by.is_none() || fell_at.is_none() || strayed.is_some() || !opened {
                problems.push(format!("{pack:?} {name}: hurt {hurt_by:?} fell {fell_at:?} strayed {strayed:?} opens {opened}"));
            }
        }
    }
    assert!(problems.is_empty(), "bosses that cannot be fought out:\n{}", problems.join("\n"));
}

/// One stretch of a guardian's fight, its health held at one share: the
/// hostile shots loosed, over how many ticks, the most in the room at once,
/// the most loosed on one tick, and the room at its densest.
struct Window {
    fired: u64,
    ticks: u32,
    peak: usize,
    volley: u64,
    densest: Option<Run>,
    /// Charge-ups begun (wind-ups, or the unruly ones' breaths and rail
    /// warnings), and the room halfway through the first.
    charges: u32,
    winding: Option<Run>,
}

impl Window {
    fn per_second(&self) -> f32 {
        self.fired as f32 * crate::rules::HZ as f32 / self.ticks.max(1) as f32
    }
}

/// Every guardian of every pack, as (pack, its place in the pack's list).
fn every_guardian() -> Vec<(Pack, usize)> {
    let mut all = Vec::new();
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
        let (mut run, _) = guardian_room(pack, 0).expect("every pack has a guardian");
        for k in 0..run.pack_bosses_for_test(pack).len() {
            all.push((pack, k));
        }
    }
    all
}

/// A stairs room with guardian `k` of `pack` alone in it, where a guardian
/// stands (the top of the room's middle), and a knight who cannot fall
/// standing below and to its left, in the same view, not firing.
fn guardian_room(pack: Pack, k: usize) -> Option<(Run, usize)> {
    guardian_room_for(pack, k, 1)
}

fn guardian_room_for(pack: Pack, k: usize, knights: u8) -> Option<(Run, usize)> {
    let mut run = Run::new(7, 1, (knights > 1).then_some("B"));
    for name in ["C", "D"].into_iter().take(usize::from(knights.saturating_sub(2))) {
        run.join(name);
    }
    let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs)?;
    run.enter_for_test(stairs);
    let index = *run.pack_bosses_for_test(pack).get(k)?;
    run.enemies.clear();
    run.projectiles.clear();
    // The guardian alone: no traps or waves of the room's own.
    run.calm_for_test();
    let (w, h) = (run.room().width(), run.room().height());
    run.spawn_boss_at_for_test(index, w / 2.0, 7.0);
    for (n, hero) in run.players.values_mut().enumerate() {
        (hero.x, hero.y) = (w / 2.0 - 8.0 + 4.0 * n as f32, (h - 5.0).min(16.0));
        hero.invulnerable = 1_000_000;
    }
    Some((run, index))
}

/// Guardian `k` of `pack` fought for `ticks` with its health held at
/// `share`, after `warm` ticks at that share uncounted.
fn fight_window(pack: Pack, k: usize, knights: u8, share: f32, warm: u32, ticks: u32, keep: bool) -> (String, Window) {
    let (mut run, index) = guardian_room_for(pack, k, knights).expect("a guardian");
    let name = run.bosses[index].name.clone();
    let mut window = Window { fired: 0, ticks, peak: 0, volley: 0, densest: None, charges: 0, winding: None };
    let winding = |run: &Run| {
        run.enemies.iter().find(|e| e.kind == EnemyKind::Boss).map(|e| {
            use crate::rules::signatures::{INHALE, TELL, WIND};
            (e.stage, matches!(e.stage, WIND | INHALE | TELL), e.timer)
        })
    };
    let mut was = winding(&run);
    let mut started = None;
    for t in 0..warm + ticks {
        for boss in run.enemies.iter_mut().filter(|e| e.kind == EnemyKind::Boss) {
            boss.hp = ((boss.max_hp as f32 * share) as u32).max(1);
        }
        for hero in run.players.values_mut() {
            hero.hp = hero.max_hp;
        }
        let before = run.hostile_shots;
        // Simulation ticks: an impact's hold (`hitstop`) stops the clock, not
        // the fight.
        run.step_sim_for_test(&BTreeMap::new());
        let now = winding(&run);
        if t < warm {
            was = now;
            continue;
        }
        // A wind-up begun: a new wind stage (a breath to its rip is one).
        if let (Some((s0, w0, _)), Some((s1, true, _))) = (was, now)
            && (!w0 || (s0 != s1 && s1 != crate::rules::signatures::TELL))
        {
            window.charges += 1;
            started.get_or_insert(t);
        }
        if keep && window.winding.is_none() && started.is_some_and(|s| t == s + 14) {
            window.winding = Some(run.clone());
        }
        was = now;
        let volley = run.hostile_shots - before;
        window.fired += volley;
        window.volley = window.volley.max(volley);
        let on = run.projectiles.iter().filter(|p| p.hostile).count();
        if on > window.peak {
            window.peak = on;
            if keep {
                window.densest = Some(run.clone());
            }
        }
    }
    (name, window)
}

/// The three stretches the census and the caps look at: the opening's
/// first ten seconds at full health, ten seconds of the middle, and ten
/// seconds deep in rage.
const OPENING: (f32, u32) = (1.0, 0);
const MIDDLE: (f32, u32) = (0.6, 2 * crate::rules::HZ);
const RAGING: (f32, u32) = (0.12, 2 * crate::rules::HZ);
const STRETCH: u32 = 10 * crate::rules::HZ;

fn save_png(run: &Run, path: &std::path::Path) {
    let img = crate::render::overworld::arena::frame(run, 384, 224);
    let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
    out.extend(img.rgb_bytes());
    std::fs::write(path, out).unwrap();
}

/// Every guardian's pressure, opening, middle and rage: shots a second,
/// the most in the room at once, and the biggest single volley. With
/// `BOSS_CENSUS_FRAMES=<dir>` it also writes each one's densest frames.
/// `cargo test boss_pressure_census -- --ignored --nocapture`
#[test]
#[ignore]
fn boss_pressure_census() {
    let frames = std::env::var_os("BOSS_CENSUS_FRAMES").map(std::path::PathBuf::from);
    if let Some(dir) = &frames {
        std::fs::create_dir_all(dir).unwrap();
    }
    println!("| boss | open /s | open peak | middle /s | middle peak | rage /s | rage peak | biggest volley | charge-ups o/m/r |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for (pack, k) in every_guardian() {
        let mut row = Vec::new();
        let mut biggest = 0;
        let mut name = String::new();
        let mut charges = Vec::new();
        for (label, (share, warm)) in [("open", OPENING), ("middle", MIDDLE), ("rage", RAGING)] {
            let (n, w) = fight_window(pack, k, 1, share, warm, STRETCH, frames.is_some());
            name = n;
            biggest = biggest.max(w.volley);
            charges.push(w.charges.to_string());
            row.push(format!("{:.1} | {}", w.per_second(), w.peak));
            let slug = super::super::cards::slug(&name);
            if let (Some(dir), Some(run)) = (&frames, &w.densest) {
                save_png(run, &dir.join(format!("{slug}_{label}.ppm")));
            }
            if let (Some(dir), Some(run)) = (&frames, &w.winding) {
                save_png(run, &dir.join(format!("{slug}_{label}_windup.ppm")));
            }
        }
        println!("| {name} | {} | {biggest} | {} |", row.join(" | "), charges.join("/"));
    }
}


/// Every guardian keeps to its stage's caps, alone or with a friend: no
/// more shots a second than its stage allows (one banked volley of slack
/// over the stretch), no more in the room at once than its stage allows
/// per knight, and no volley over `VOLLEY_MOST`.
#[test]
fn every_guardian_keeps_to_its_stage_caps() {
    use crate::rules::HZ;
    let mut over = Vec::new();
    for knights in [1u8, 2] {
        for (pack, k) in every_guardian() {
            for (stage, (share, warm)) in [(Stage::Open, OPENING), (Stage::Middle, MIDDLE), (Stage::Rage, RAGING)] {
                let (name, w) = fight_window(pack, k, knights, share, warm, STRETCH, false);
                let (per_second, on_screen) = stage.caps();
                let most = u64::from(per_second * STRETCH / HZ + VOLLEY_MOST);
                let room = on_screen as usize * usize::from(knights);
                if w.fired > most || w.peak > room || w.volley > u64::from(VOLLEY_MOST) {
                    over.push(format!(
                        "{name} {stage:?} x{knights}: fired {} (most {most}), peak {} (most {room}), volley {}",
                        w.fired, w.peak, w.volley
                    ));
                }
            }
        }
    }
    assert!(over.is_empty(), "over their caps:\n{}", over.join("\n"));
}

/// The stretches the caps are measured in are the stages they name.
#[test]
fn the_census_stretches_fall_in_their_stages() {
    for (pack, k) in every_guardian() {
        let (run, index) = guardian_room(pack, k).unwrap();
        let boss = &run.bosses[index];
        for (stage, (share, _)) in [(Stage::Open, OPENING), (Stage::Middle, MIDDLE), (Stage::Rage, RAGING)] {
            assert_eq!(boss.stage((share * 1000.0) as u32, 1000), stage, "{}", boss.id);
        }
    }
}

/// Every guardian has at least one charge-up, every file's winds up for at
/// least `WIND_LEAST`, and from the middle of the fight on unless it says.
#[test]
fn every_guardian_has_a_charge_up() {
    let mut seen = std::collections::BTreeSet::new();
    for (pack, k) in every_guardian() {
        let (run, index) = guardian_room(pack, k).unwrap();
        let boss = &run.bosses[index];
        let ups = crate::rules::signatures::charge_ups(boss);
        assert!(!ups.is_empty(), "{} has no charge-up", boss.id);
        for up in &boss.windups {
            assert!(up.wind >= WIND_LEAST, "{} {:?} winds {}", boss.id, up.kind, up.wind);
            assert!(up.burst() <= VOLLEY_MOST, "{} {:?}", boss.id, up.kind);
        }
        seen.extend(ups.iter().map(|u| u.name()));
    }
    // The whole vocabulary is in use.
    assert_eq!(seen.len(), Windup::ALL.len(), "{seen:?}");
}

/// A charge-up's wind-up begun from standing (a file's, the Ripper's
/// breath, Grinwick's rail warning).
fn winds(stage: u8) -> bool {
    use crate::rules::signatures::{INHALE, TELL, WIND};
    matches!(stage, WIND | INHALE | TELL)
}

/// Every charge-up of every guardian, raging (so every one takes its
/// turn), winds up for at least 0.75 s before anything of it can harm: no
/// knight is hurt, no shot flies, no beam burns and no shockwave bursts
/// until then. Each is wound up at least once, and the blows do land.
#[test]
fn every_charge_up_winds_up_for_three_quarters_of_a_second_first() {
    use crate::rules::HZ;
    /// 0.75 s, in ticks, rounded up.
    const LEAST: u32 = (HZ * 3).div_ceil(4);
    let mut early = Vec::new();
    for (pack, k) in every_guardian() {
        let (mut run, index) = guardian_room(pack, k).unwrap();
        let def = run.bosses[index].clone();
        let mut wound = std::collections::BTreeSet::new();
        let mut landed = 0;
        let mut began: Option<(u32, String)> = None;
        let mut was = 0;
        for hero in run.players.values_mut() {
            hero.invulnerable = 0;
        }
        for t in 0..60 * HZ {
            run.enemies.retain(|e| e.kind == EnemyKind::Boss);
            let Some(boss) = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss) else {
                break;
            };
            boss.hp = boss.max_hp / 10;
            let stage = boss.stage;
            if was == 0 && winds(stage) {
                // A wind-up begins: only its own harm counts from here.
                let kind = match def.windups.get(usize::from(boss.signature.windup)) {
                    Some(up) if !crate::rules::signatures::special(&def) => up.kind.name().to_string(),
                    _ => format!("native {stage}"),
                };
                wound.insert(kind.clone());
                began = Some((t, kind));
                run.projectiles.clear();
                run.ravages.clear();
                run.works.clouds.clear();
            }
            was = stage;
            let hp: Vec<u32> = run.players.values().map(|h| h.hp).collect();
            run.step(&BTreeMap::new());
            let hurt = run.players.values().zip(&hp).any(|(h, &before)| h.hp < before);
            let flying = run.projectiles.iter().any(|p| {
                p.hostile
                    && (p.in_flight()
                        || p.traits.path.beam.is_some_and(|b| b.burning(p.traits.path.age)))
            });
            let burst = run.ravages.iter().any(|r| r.age >= r.ring(0).1);
            for hero in run.players.values_mut() {
                hero.hp = hero.max_hp;
                hero.invulnerable = 0;
            }
            if let Some((start, kind)) = &began
                && (hurt || flying || burst)
            {
                let after = t + 1 - start;
                if after < LEAST {
                    early.push(format!("{} {kind}: harm {after} ticks after its wind-up began", def.id));
                }
                landed += 1;
                began = None;
            }
        }
        let ups = crate::rules::signatures::charge_ups(&def);
        if crate::rules::signatures::special(&def) {
            assert!(!wound.is_empty(), "{} never wound up", def.id);
        } else {
            for up in &ups {
                assert!(wound.contains(up.name()), "{} never wound up its {}: {wound:?}", def.id, up.name());
            }
        }
        assert!(landed > 0, "{}: no charge-up ever landed", def.id);
    }
    assert!(early.is_empty(), "{}", early.join("\n"));
}

/// Every guardian opens gentler than it rages: fewer shots a second, fewer
/// in the room at once, and no more charge-ups.
#[test]
fn every_guardian_opens_gentler_than_it_rages() {
    let mut harsh = Vec::new();
    for (pack, k) in every_guardian() {
        let (name, open) = fight_window(pack, k, 1, OPENING.0, OPENING.1, STRETCH, false);
        let (_, rage) = fight_window(pack, k, 1, RAGING.0, RAGING.1, STRETCH, false);
        if open.per_second() >= rage.per_second() || open.peak >= rage.peak || open.charges > rage.charges {
            harsh.push(format!(
                "{name}: opening {:.1}/s peak {} charges {}, rage {:.1}/s peak {} charges {}",
                open.per_second(),
                open.peak,
                open.charges,
                rage.per_second(),
                rage.peak,
                rage.charges
            ));
        }
    }
    assert!(harsh.is_empty(), "{}", harsh.join("\n"));
}

/// Replay a saved run (`REPLAY_RUN=<run.json>`) for thirty seconds with
/// every knight walking up, and print what befalls the party each second.
#[test]
#[ignore]
fn replay_a_saved_run() {
    use crate::rules::HZ;
    let Some(path) = std::env::var_os("REPLAY_RUN") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let mut run: Run = serde_json::from_str(&text).unwrap();
    println!("loaded: valid {} at {} {:?} depth {}", run.valid_snapshot(), run.at, run.room().kind, run.dungeon.depth);
    let ids: Vec<u32> = run.players.keys().copied().collect();
    for t in 0..30 * HZ {
        let inputs: BTreeMap<u32, crate::rules::Input> = ids
            .iter()
            .map(|&id| (id, crate::rules::Input { move_y: if (t / 45) % 2 == 0 { -1 } else { 1 }, ..Default::default() }))
            .collect();
        let at = run.at;
        run.step(&inputs);
        if run.at != at || t % HZ == 0 {
            let hp: Vec<String> = run.players.values().map(|h| format!("{}:{}{}", h.name, h.hp, if h.stone { "s" } else { "" })).collect();
            println!(
                "t={t} at {} {:?} depth {} phase {:?} foes {} hostile {} | {}",
                run.at,
                run.room().kind,
                run.dungeon.depth,
                run.phase,
                run.enemies.len(),
                run.projectiles.iter().filter(|p| p.hostile).count(),
                hp.join(" ")
            );
        }
        if run.over() {
            println!("OVER at t={t}: {:?}", run.found_line());
            break;
        }
    }
}

/// Enter every room of a saved run's floor (`REPLAY_RUN=<run.json>`) and
/// stand ten seconds: print the rooms that hurt or end the party.
#[test]
#[ignore]
fn replay_every_room_of_a_saved_floor() {
    use crate::rules::HZ;
    let Some(path) = std::env::var_os("REPLAY_RUN") else {
        return;
    };
    let base: Run = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for i in 0..base.dungeon.rooms.len() {
        let mut run = base.clone();
        run.enter_for_test(i);
        let kind = run.room().kind;
        let start: u32 = run.players.values().map(|h| h.hp).sum();
        let mut ended = None;
        let mut moved = None;
        for t in 0..10 * HZ {
            let at = run.at;
            run.step(&BTreeMap::new());
            if run.at != at && moved.is_none() {
                moved = Some((t, run.at, run.room().kind));
            }
            if run.over() && ended.is_none() {
                ended = Some(t);
                break;
            }
        }
        let end: u32 = run.players.values().map(|h| h.hp).sum();
        if end < start || ended.is_some() || moved.is_some() {
            println!(
                "room {i} {kind:?}: party hp {start} -> {end}, over at {ended:?}, moved to {moved:?}, foes {}, bosses {}",
                run.enemies.len(),
                run.enemies.iter().filter(|e| e.kind == EnemyKind::Boss).count()
            );
        }
    }
}

/// The knights' most health in the charge-up census: a knight a little
/// under the cap (`Hero::hp_cap`).
const KNIGHT_HP: u32 = 150;

/// Where knights stand for the census: in the guardian's path (below it,
/// the nearest), beside it, beside it on the other side, across the room.
fn census_spots(run: &Run) -> [(f32, f32); 4] {
    let (w, h) = (run.room().width(), run.room().height());
    let (bx, by) = (w / 2.0, 7.0);
    let grid = crate::rules::Grid { room: run.room(), barred: true };
    // Each on open floor: the nearest clear spot to where it is meant.
    let floor = |(x, y): (f32, f32)| {
        (0..12)
            .flat_map(|r| (0..16).map(move |a| (r as f32 * 0.5, a as f32 * std::f32::consts::TAU / 16.0)))
            .map(|(r, a)| (x + r * a.cos(), y + r * a.sin()))
            .find(|&(x, y)| grid.clear(x, y, crate::rules::HERO_RADIUS + 0.1, crate::rules::Mover::Hero))
            .unwrap_or((x, y))
    };
    [(bx, by + 5.0), (bx + 4.5, by), (bx - 4.5, by + 1.0), (bx - 10.0, (h - 4.0).max(by + 8.0))].map(floor)
}

/// Guardian `k` of `pack` on floor `depth` with only its charge-ups (its
/// other attacks gone, anchored where it was put), and naked knights of
/// `KNIGHT_HP` standing at `at` (`None`: the census spots, as many as
/// `knights`). `None` for the two unruly ones, whose machines are their own.
fn charge_up_room(pack: Pack, k: usize, depth: u32, knights: u8, at: Option<&[(f32, f32)]>) -> Option<(Run, usize)> {
    let (mut run, index) = guardian_room_for(pack, k, knights)?;
    if run.bosses[index].windups.is_empty() || crate::rules::signatures::special(&run.bosses[index]) {
        return None;
    }
    run.dungeon.depth = depth;
    let def = &mut run.bosses[index];
    def.attacks.clear();
    def.phases = None;
    def.moves = Move::Anchor;
    let spots = census_spots(&run);
    let at: Vec<(f32, f32)> = at.map_or_else(|| spots[..usize::from(knights)].to_vec(), <[_]>::to_vec);
    for (hero, &(x, y)) in run.players.values_mut().zip(&at) {
        (hero.x, hero.y) = (x, y);
        hero.owned = Default::default();
        hero.armor = 0;
        hero.max_hp = KNIGHT_HP;
        hero.hp = KNIGHT_HP;
        hero.invulnerable = 0;
    }
    Some((run, index))
}

/// One charge-up seen through: what each knight lost to it, how many
/// times each was struck, and the farthest any knight was moved in a tick.
#[derive(Debug, Default)]
struct Blow {
    lost: Vec<u32>,
    struck: Vec<u32>,
    moved: f32,
}

/// Charge-up `which` of guardian `k` of `pack`, raging, wound up at once
/// and seen through to its last shot, the knights standing still at `at`.
fn one_charge_up(pack: Pack, k: usize, which: usize, depth: u32, knights: u8, at: Option<&[(f32, f32)]>) -> Option<Blow> {
    use crate::rules::HZ;
    let (mut run, index) = charge_up_room(pack, k, depth, knights, at)?;
    if which >= run.bosses[index].windups.len() {
        return None;
    }
    {
        let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss)?;
        boss.signature.cycle = which as u32;
        boss.timer = 10_000;
    }
    let mut blow = Blow { lost: vec![0; usize::from(knights)], struck: vec![0; usize::from(knights)], moved: 0.0 };
    let mut started = false;
    for _ in 0..30 * HZ {
        let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss)?;
        boss.hp = (boss.max_hp / 10).max(1);
        if boss.stage != 0 {
            started = true;
        } else if started {
            // Done with it: held still while what it threw flies out.
            boss.frozen = 1_000_000;
            if !run.projectiles.iter().any(|p| p.hostile) && run.ravages.is_empty() {
                break;
            }
        }
        let before: Vec<(u32, f32, f32)> = run.players.values().map(|h| (h.hp, h.x, h.y)).collect();
        run.step(&BTreeMap::new());
        if run.phase == Phase::Wiped {
            for (i, b) in before.iter().enumerate() {
                blow.lost[i] = blow.lost[i].max(b.0);
            }
            break;
        }
        for (i, (h, b)) in run.players.values_mut().zip(&before).enumerate() {
            let lost = b.0.saturating_sub(h.hp);
            if lost > 0 {
                blow.lost[i] += lost;
                blow.struck[i] += 1;
            }
            blow.moved = blow.moved.max((h.x - b.1).hypot(h.y - b.2));
            h.hp = h.max_hp;
        }
    }
    started.then_some(blow)
}

/// Guardian `k` of `pack` raging for `ticks` with only its charge-ups,
/// the knights standing at the census spots: what each lost in all, how
/// many charge-ups it began, and how many began while the last one's
/// shots, beam or rings were still about.
fn rage_stretch(pack: Pack, k: usize, depth: u32, knights: u8, ticks: u32) -> Option<(Vec<u32>, u32, u32)> {
    let (mut run, _) = charge_up_room(pack, k, depth, knights, None)?;
    let mut lost = vec![0; usize::from(knights)];
    let (mut began, mut stacked, mut was) = (0, 0, 0u8);
    for _ in 0..ticks {
        let lingering = run.projectiles.iter().any(|p| p.hostile) || !run.ravages.is_empty();
        let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss)?;
        boss.hp = (boss.max_hp / 10).max(1);
        let before: Vec<u32> = run.players.values().map(|h| h.hp).collect();
        run.step(&BTreeMap::new());
        let Some(boss) = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss) else { break };
        if was == 0 && boss.stage != 0 {
            began += 1;
            if lingering {
                stacked += 1;
            }
        }
        was = boss.stage;
        if run.phase == Phase::Wiped {
            for (l, b) in lost.iter_mut().zip(&before) {
                *l += *b;
            }
            break;
        }
        for (l, (h, b)) in lost.iter_mut().zip(run.players.values_mut().zip(&before)) {
            *l += b.saturating_sub(h.hp);
            h.hp = h.max_hp;
        }
    }
    Some((lost, began, stacked))
}

/// The charge-up census: for every guardian's charge-up, on floors 1–6,
/// with 1 knight (in its path, beside it, across the room), 2 and 4
/// knights, the most one charge-up takes from a standing knight, in
/// percent of its health, and the most twenty seconds of rage take.
/// `cargo test -p angel-delve charge_up_census -- --ignored --nocapture`
#[test]
#[ignore]
fn charge_up_census() {
    use crate::rules::HZ;
    println!("| guardian | charge-up | floor | 1 path | 1 beside | 1 across | 2 | 4 | most hits | most moved | rage 20 s worst (x4) | began | stacked |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (pack, k) in every_guardian() {
        let Some((run, index)) = charge_up_room(pack, k, 1, 1, None) else { continue };
        let def = run.bosses[index].clone();
        let spots = census_spots(&run);
        for (which, up) in def.windups.iter().enumerate() {
            for depth in 1..=6 {
                let pct = |b: &Blow| b.lost.iter().max().copied().unwrap_or(0) * 100 / KNIGHT_HP;
                let mut cells = Vec::new();
                let (mut hits, mut moved) = (0, 0.0f32);
                for at in [&spots[0..1], &spots[1..2], &spots[3..4]] {
                    let b = one_charge_up(pack, k, which, depth, 1, Some(at)).unwrap_or_default();
                    cells.push(format!("{}%", pct(&b)));
                    hits = hits.max(b.struck.iter().max().copied().unwrap_or(0));
                    moved = moved.max(b.moved);
                }
                for n in [2u8, 4] {
                    let b = one_charge_up(pack, k, which, depth, n, None).unwrap_or_default();
                    cells.push(format!("{}%", pct(&b)));
                    hits = hits.max(b.struck.iter().max().copied().unwrap_or(0));
                    moved = moved.max(b.moved);
                }
                let rage = if which == 0 {
                    rage_stretch(pack, k, depth, 4, 20 * HZ)
                        .map(|(lost, began, stacked)| {
                            format!("{}% | {began} | {stacked}", lost.iter().max().copied().unwrap_or(0) * 100 / KNIGHT_HP)
                        })
                        .unwrap_or_default()
                } else {
                    "| |".into()
                };
                println!("| {} | {} | {depth} | {} | {hits} | {moved:.2} | {rage} |", def.name, up.kind.name(), cells.join(" | "));
            }
        }
    }
}


/// Every guardian's charge-ups, each on floors 1–6, with one knight in its
/// path, one across the room (by the wall) and four at the census spots,
/// all naked: what each one took.
fn every_blow() -> Vec<(String, u32, u8, Blow)> {
    let mut all = Vec::new();
    for (pack, k) in every_guardian() {
        let Some((run, index)) = charge_up_room(pack, k, 1, 1, None) else { continue };
        let def = run.bosses[index].clone();
        let spots = census_spots(&run);
        for (which, up) in def.windups.iter().enumerate() {
            for depth in 1..=6 {
                for (knights, at) in [(1u8, Some(&spots[0..1])), (1, Some(&spots[3..4])), (4, None)] {
                    let blow = one_charge_up(pack, k, which, depth, knights, at)
                        .unwrap_or_else(|| panic!("{} never wound up its {}", def.id, up.kind.name()));
                    all.push((format!("{} {}", def.id, up.kind.name()), depth, knights, blow));
                }
            }
        }
    }
    all
}

/// One undodged charge-up takes at most `BLOW_MOST` of a standing naked
/// knight's health, on every floor, alone or four strong.
#[test]
fn no_charge_up_takes_more_than_its_share_of_a_knight() {
    use crate::rules::hero::BLOW_MOST;
    let most = KNIGHT_HP * BLOW_MOST / 100;
    let mut over = Vec::new();
    let mut landed = 0;
    for (name, depth, knights, blow) in every_blow() {
        landed += blow.lost.iter().filter(|&&l| l > 0).count();
        if let Some(&lost) = blow.lost.iter().max()
            && lost > most
        {
            over.push(format!("{name} floor {depth} x{knights}: {lost} of {KNIGHT_HP} (most {most})"));
        }
    }
    assert!(landed > 0, "no charge-up ever struck");
    assert!(over.is_empty(), "{}", over.join("\n"));
}

/// A charge-up strikes a knight at most once: its dash's every tick, a
/// slam's rings, a breath's or a landing's burst and its beam together.
#[test]
fn a_charge_up_strikes_a_knight_at_most_once() {
    let mut twice = Vec::new();
    for (name, depth, knights, blow) in every_blow() {
        if let Some(&n) = blow.struck.iter().max()
            && n > 1
        {
            twice.push(format!("{name} floor {depth} x{knights}: struck {n} times"));
        }
    }
    assert!(twice.is_empty(), "{}", twice.join("\n"));
}

/// No charge-up begins while anything of the last is still about: twenty
/// seconds of every guardian's rage, four knights standing.
#[test]
fn charge_ups_never_overlap() {
    use crate::rules::HZ;
    let mut stacked = Vec::new();
    for (pack, k) in every_guardian() {
        let Some((lost, began, on)) = rage_stretch(pack, k, 4, 4, 20 * HZ) else { continue };
        let (run, index) = charge_up_room(pack, k, 4, 1, None).unwrap();
        let id = &run.bosses[index].id;
        assert!(began >= 2, "{id}: {began} charge-ups in twenty seconds of rage");
        if on > 0 {
            stacked.push(format!("{id}: {on} of {began} began over the last (lost {lost:?})"));
        }
    }
    assert!(stacked.is_empty(), "{}", stacked.join("\n"));
}

/// A knight walking straight away from a breath gets farther every tick
/// of it: the pull is weaker than a walk.
#[test]
fn a_knight_walking_away_escapes_the_inhale() {
    use crate::rules::Input;
    let mut caught = Vec::new();
    let mut tried = 0;
    for (pack, k) in every_guardian() {
        let Some((run, index)) = charge_up_room(pack, k, 4, 1, None) else { continue };
        let def = run.bosses[index].clone();
        for (which, up) in def.windups.iter().enumerate().filter(|(_, u)| u.kind == Windup::Inhale) {
            tried += 1;
            let (mut run, _) = charge_up_room(pack, k, 4, 1, None).unwrap();
            let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss).unwrap();
            let (bx, by) = (boss.x, boss.y);
            boss.signature.cycle = which as u32;
            boss.timer = 10_000;
            let id = *run.players.keys().next().unwrap();
            // Below it, walking on down the room.
            run.players.get_mut(&id).unwrap().x = bx;
            run.players.get_mut(&id).unwrap().y = by + 4.0;
            let away = Input { move_y: 1, ..Input::default() };
            let mut far = 4.0f32;
            let mut breathing = 0;
            for _ in 0..up.wind + 10 {
                let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss).unwrap();
                boss.hp = (boss.max_hp / 10).max(1);
                run.step(&BTreeMap::from([(id, away)]));
                let boss = run.enemies.iter().find(|e| e.kind == EnemyKind::Boss).unwrap();
                let h = &run.players[&id];
                let d = (h.x - boss.x).hypot(h.y - boss.y);
                if boss.stage == crate::rules::signatures::WIND {
                    breathing += 1;
                    if d + 0.001 < far {
                        caught.push(format!("{} drew the knight from {far:.2} to {d:.2}", def.id));
                        break;
                    }
                }
                far = far.max(d);
            }
            assert!(breathing > 0, "{} never breathed in", def.id);
            assert!(far > 4.0 + crate::rules::signatures::PULL_REACH / 2.0, "{}: only {far:.2} away", def.id);
        }
    }
    assert!(tried > 0);
    assert!(caught.is_empty(), "{}", caught.join("\n"));
}

/// A frame from the middle of a charge's, a slam's and a breath's wind-up,
/// and of a beam's sweep: `ANGEL_ARENA_SHOTS=<dir> cargo test -p
/// angel-delve write_charge_up_shots -- --ignored`.
#[test]
#[ignore]
fn write_charge_up_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut done = std::collections::BTreeSet::new();
    for (pack, k) in every_guardian() {
        let Some((run, index)) = charge_up_room(pack, k, 3, 4, None) else { continue };
        let def = run.bosses[index].clone();
        for (which, up) in def.windups.iter().enumerate() {
            if !done.insert(up.kind.name()) {
                continue;
            }
            let (mut run, _) = charge_up_room(pack, k, 3, 4, None).unwrap();
            let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss).unwrap();
            boss.signature.cycle = which as u32;
            boss.timer = 10_000;
            let at = if up.kind == Windup::Beam { up.wind + u32::from(crate::rules::signatures::beam_burn(up)) / 2 } else { up.wind * 2 / 3 };
            for _ in 0..=at {
                for h in run.players.values_mut() {
                    h.hp = h.max_hp;
                }
                let boss = run.enemies.iter_mut().find(|e| e.kind == EnemyKind::Boss).unwrap();
                boss.hp = (boss.max_hp / 10).max(1);
                run.step(&BTreeMap::new());
            }
            assert!(run.enemies.iter().any(|e| e.kind == EnemyKind::Boss && e.stage != 0), "{} is not at its {}", def.id, up.kind.name());
            save_png(&run, &dir.join(format!("charge_up_{}_{}.ppm", up.kind.name(), def.id)));
        }
    }
}

/// Load a saved run (`REPLAY_RUN=<run.json>`) the way a restore does and say
/// why it is refused, if it is.
#[test]
#[ignore]
fn why_a_saved_run_is_refused() {
    let Some(path) = std::env::var_os("REPLAY_RUN") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let mut run: Run = match serde_json::from_str(&text) {
        Ok(run) => run,
        Err(e) => panic!("does not read: {e}"),
    };
    crate::rules::world::raise_world(&mut run.dungeon);
    run.migrate_kingdom();
    println!("valid_snapshot {}", run.valid_snapshot());
}

#[test]
fn a_save_from_a_build_with_fewer_seats_still_reads() {
    // A four-seat build's merchant kept four approaches; this build has more.
    let four = r#"{"approaches":[{"offer":null,"ticks":0,"spent":false,"x":0.0,"y":0.0},{"offer":null,"ticks":0,"spent":false,"x":0.0,"y":0.0},{"offer":null,"ticks":0,"spent":false,"x":0.0,"y":0.0},{"offer":null,"ticks":0,"spent":true,"x":1.0,"y":2.0}]}"#;
    let state: crate::rules::merchant::State = serde_json::from_str(four).unwrap();
    assert!(state.approaches[3].spent);
    assert!(!state.approaches[crate::rules::MAX_PLAYERS as usize - 1].spent || crate::rules::MAX_PLAYERS == 4);
}
