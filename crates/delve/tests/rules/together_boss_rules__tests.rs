//! The guardians with a rule of their own (`furnace`, `court`), fought to the
//! end by scripted parties of one, two and four fresh knights on the floors
//! they guard: a party that plays the rule and one that ignores it, beside
//! the guardian's own file before its rule and a plain guardian of its size.
//! The knights are mended every tick, so nobody falls and the clock is the
//! party's damage alone; what they lost is counted as it went.
//! `cargo test -p angel-delve boss_rule_duels -- --ignored --nocapture`
use super::balance_sim_tests::{Party, bot_input, dir8, floor_run_in};
use super::*;
use std::f32::consts::{FRAC_PI_4, PI};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Play {
    Ignore,
    Rule,
}

#[derive(Clone, Copy, Debug, Default)]
struct Duel {
    secs: f32,
    cleared: bool,
    dmg: f32,
    stages: [f32; 3],
    open: f32,
}

const CAP: u32 = 300;
const SEEDS: u64 = 16;

fn guardian(run: &Run) -> Option<(&Enemy, &Boss)> {
    run.enemies
        .iter()
        .filter(|e| e.hp > 0)
        .find_map(|e| e.boss.and_then(|i| run.bosses.get(usize::from(i))).map(|b| (e, b)))
}

fn key(v: f32) -> i8 {
    if v > 0.38 {
        1
    } else if v < -0.38 {
        -1
    } else {
        0
    }
}

fn walk(input: &mut Input, from: (f32, f32), to: (f32, f32)) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let d = dx.hypot(dy);
    (input.move_x, input.move_y) = if d < 0.4 { (0, 0) } else { (key(dx / d), key(dy / d)) };
}

fn on_line(at: (f32, f32), angle: f32, r: f32) -> (f32, f32) {
    let a = (angle / FRAC_PI_4).round() * FRAC_PI_4;
    (at.0 + a.cos() * r, at.1 + a.sin() * r)
}

fn threatened(run: &Run, hero: &Hero) -> bool {
    run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0).any(|shot| {
        if let Some(((lx, ly), _)) = patterns::lob_flight(shot) {
            return (hero.x - lx).hypot(hero.y - ly) < HERO_RADIUS + patterns::LOB_SPLASH + 0.6;
        }
        if let Some(beam) = shot.traits.path.beam {
            let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
            let (rx, ry) = (hero.x - shot.x, hero.y - shot.y);
            return (0.0..beam.reach).contains(&(rx * ux + ry * uy)) && (rx * -uy + ry * ux).abs() < 2.5;
        }
        let (dx, dy) = (hero.x - shot.x, hero.y - shot.y);
        let d = dx.hypot(dy);
        let speed = shot.vx.hypot(shot.vy).max(0.01);
        d < 4.5 && (shot.vx * dx + shot.vy * dy) / (speed * d.max(0.01)) > 0.6
    }) || run.enemies.iter().any(|e| {
        e.stage == signatures::WIND
            && e.boss.and_then(|i| run.bosses.get(usize::from(i))).and_then(|b| signatures::winding(e, b))
                .is_some_and(|u| u.kind == bosses::Windup::Leap)
            && (e.signature.mark.0 - hero.x).hypot(e.signature.mark.1 - hero.y) < signatures::LANDING + 1.0
    })
}

fn lined_input(run: &Run, id: u32) -> Input {
    let mut input = bot_input(run, id);
    let hero = &run.players[&id];
    let Some((e, _)) = guardian(run).filter(|_| hero.hp > 0) else {
        return input;
    };
    let melee = hero.forged.as_ref().is_some_and(|w| w.melee.is_some() && w.bolt.is_none() && w.throw.is_none());
    if melee || threatened(run, hero) {
        return input;
    }
    let bearing = (hero.y - e.y).atan2(hero.x - e.x);
    let spot = on_line((e.x, e.y), bearing, 7.5);
    walk(&mut input, (hero.x, hero.y), spot);
    if input.move_x == 0 && input.move_y == 0 {
        (input.aim_x, input.aim_y) = dir8(e.x - hero.x, e.y - hero.y);
    }
    input
}

fn rule_input(run: &Run, id: u32) -> Input {
    let mut input = lined_input(run, id);
    let hero = &run.players[&id];
    let Some((e, b)) = guardian(run).filter(|_| hero.hp > 0) else {
        return input;
    };
    let me = (hero.x, hero.y);
    let at = (e.x, e.y);
    let bearing = (me.1 - at.1).atan2(me.0 - at.0);
    let d = (me.0 - at.0).hypot(me.1 - at.1);
    let full = hero.ult_charge >= ults::ULT_FULL;
    let dodging = threatened(run, hero);
    let go = |input: &mut Input, to: (f32, f32)| {
        if !dodging {
            walk(input, me, to);
        }
        (input.aim_x, input.aim_y) = dir8(at.0 - me.0, at.1 - me.1);
        input.fire = true;
        input.swing = d <= SWORD_REACH + e.radius();
    };
    if furnace::is(b) {
        let breathing = e.stage == signatures::WIND
            && signatures::winding(e, b).is_some_and(|u| u.kind == bosses::Windup::Inhale);
        if furnace::venting(e, b) {
            go(&mut input, on_line(at, bearing, e.radius() + 1.6));
            input.ult = full;
        } else if breathing {
            go(&mut input, (me.0 + bearing.cos() * 4.0, me.1 + bearing.sin() * 4.0));
            input.ult = false;
        } else {
            input.ult = full && furnace::molten(e, b);
        }
    } else if court::is(b) {
        let back = e.dir.1.atan2(e.dir.0) + PI;
        let spent = e.stage == signatures::DAZED;
        let living: Vec<u32> = run.players.iter().filter(|(_, h)| h.hp > 0).map(|(&i, _)| i).collect();
        let holder = living.len() > 1 && living.first() == Some(&id);
        let melee = hero.forged.as_ref().is_some_and(|w| w.melee.is_some() && w.bolt.is_none() && w.throw.is_none());
        if spent {
            go(&mut input, on_line(at, back, e.radius() + 1.6));
        } else if holder {
            go(&mut input, on_line(at, bearing, e.radius() + 4.0));
        } else if living.len() > 1 && !melee {
            go(&mut input, on_line(at, back, e.radius() + 6.5));
        } else {
            let r = e.radius() + if melee { 1.2 } else { 4.0 };
            let pull = (d - r) * 0.6;
            let (vx, vy) = (-bearing.sin() - bearing.cos() * pull, bearing.cos() - bearing.sin() * pull);
            go(&mut input, (me.0 + vx, me.1 + vy));
        }
        input.ult = full && court::behind(e, me);
    }
    input
}

fn duel_room(seed: u64, depth: u32, pack: Pack, boss: &Boss, knights: usize) -> Run {
    let mut run = floor_run_in(seed, depth, Party::Fresh, knights, Some(pack));
    let stairs = run.dungeon.rooms.iter().position(|r| r.kind == RoomKind::Stairs).expect("a stairs hall");
    run.enter_for_test(stairs);
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    run.bosses.push(boss.clone());
    run.spawn_boss(run.bosses.len() - 1);
    run.phase = Phase::Fighting;
    run
}

fn duel(mut run: Run, play: Play) -> Duel {
    let trace = std::env::var_os("BOSS_DUEL_TRACE").is_some();
    let pool: u32 = run.players.values().map(|h| h.max_hp).sum();
    let mut out = Duel::default();
    let mut lost = 0u32;
    let mut open_taken = 0u32;
    let start = run.tick;
    let max_hp = guardian(&run).map_or(1, |(e, _)| e.max_hp);
    for _ in 0..CAP * HZ {
        let Some((e, b)) = guardian(&run) else {
            break;
        };
        let stage = b.stage(e.hp, e.max_hp) as usize;
        let open = furnace::venting(e, b) || (court::is(b) && e.stage == signatures::DAZED);
        let was = e.hp;
        out.stages[stage] += 1.0 / HZ as f32;
        let no_ult = std::env::var_os("NO_ULT").is_some();
        let inputs: BTreeMap<u32, Input> = run
            .players
            .keys()
            .map(|&id| {
                let mut input = if play == Play::Rule { rule_input(&run, id) } else { lined_input(&run, id) };
                input.ult &= !no_ult;
                (id, input)
            })
            .collect();
        let before: Vec<u32> = run.players.values().map(|h| h.hp).collect();
        run.step(&inputs);
        for (hero, was) in run.players.values_mut().zip(before) {
            lost += was.saturating_sub(hero.hp);
            hero.hp = hero.max_hp;
        }
        let now = guardian(&run).map_or(0, |(e, _)| e.hp);
        if open {
            open_taken += was.saturating_sub(now);
        }
        if trace && was > now {
            if let Some(e) = run.enemies.iter().find(|e| e.is_boss()) {
                let knights: Vec<String> = run.players.values().map(|h| format!("{}{:.0}/{}", if court::behind(e, (h.x, h.y)) { "B" } else { "f" }, (h.x - e.x).hypot(h.y - e.y), h.ult_charge)).collect();
                println!("  t={:>4} hp {was:>4} -{:>3} stage {} open {open} {}", run.tick - start, was - now, e.stage, knights.join(" "));
            }
        }
    }
    out.secs = (run.tick - start) as f32 / HZ as f32;
    out.cleared = guardian(&run).is_none();
    out.dmg = lost as f32 * 100.0 / pool.max(1) as f32;
    out.open = open_taken as f32 * 100.0 / max_hp as f32;
    out
}

fn before_its_rule(boss: &Boss) -> Boss {
    let mut old = boss.clone();
    old.id = format!("{}-before", boss.id);
    if furnace::is(boss) {
        for up in old.windups.iter_mut().filter(|u| u.kind == bosses::Windup::Inhale) {
            (up.every, up.from) = (240, bosses::Stage::Rage);
        }
    }
    old
}

fn mean(duels: &[Duel], f: fn(&Duel) -> f32) -> f32 {
    duels.iter().map(f).sum::<f32>() / duels.len().max(1) as f32
}

fn line(label: &str, duels: &[Duel], against: Option<f32>) -> f32 {
    let secs = mean(duels, |d| d.secs);
    let ratio = against.map_or(String::from("     "), |a| format!("{:>4.2}x", secs / a.max(0.01)));
    let lo = duels.iter().map(|d| d.secs).fold(f32::MAX, f32::min);
    let hi = duels.iter().map(|d| d.secs).fold(0.0, f32::max);
    println!(
        "| {label:<26} | {secs:>5.1} ({lo:>4.1}-{hi:>5.1}) | {ratio} | {:>3.0}% | {:>4.0}% | {:>4.1} / {:>4.1} / {:>4.1} | {:>3.0}% |",
        duels.iter().filter(|d| d.cleared).count() as f32 * 100.0 / duels.len() as f32,
        mean(duels, |d| d.dmg),
        mean(duels, |d| d.stages[0]),
        mean(duels, |d| d.stages[1]),
        mean(duels, |d| d.stages[2]),
        mean(duels, |d| d.open),
    );
    secs
}

fn of(id: &str) -> Boss {
    bosses::builtin().into_iter().find(|b| b.id == id).or_else(|| bosses::checked(id)).expect("a guardian")
}

fn table(rule: &Boss, plain: &Boss, pack: Pack) {
    let old = before_its_rule(rule);
    println!();
    println!("{} ({} hp) beside {} ({} hp), floors 1-2", rule.name, rule.hp, plain.name, plain.hp);
    println!("| fight | secs (range) | vs old | clear | lost | open / middle / rage s | open |");
    println!("|---|---|---|---|---|---|---|");
    for knights in [1usize, 2, 4] {
        let fights = |boss: &Boss, play: Play| -> Vec<Duel> {
            [1, 2]
                .into_iter()
                .flat_map(|depth| (0..SEEDS).map(move |s| (depth, s)))
                .map(|(depth, s)| duel(duel_room(s, depth, pack, boss, knights), play))
                .collect()
        };
        let was = line(&format!("x{knights} before its rule"), &fights(&old, Play::Ignore), None);
        line(&format!("x{knights} plays the rule"), &fights(rule, Play::Rule), Some(was));
        line(&format!("x{knights} ignores it"), &fights(rule, Play::Ignore), Some(was));
        line(&format!("x{knights} {}", plain.id), &fights(plain, Play::Ignore), Some(was));
    }
}

#[test]
#[ignore]
fn boss_rule_duels() {
    let only = std::env::var("DUEL").ok();
    for (rule, plain, pack) in [
        (furnace::ID, "the-foreman", Pack::Cavern),
        (court::ID, "hellforge-slag-sovereign", Pack::Crypt),
    ] {
        if only.as_deref().is_none_or(|o| o == rule) {
            table(&of(rule), &of(plain), pack);
        }
    }
}

#[test]
fn playing_a_guardians_rule_beats_ignoring_it_and_ignoring_it_is_no_wall() {
    for (id, pack) in [(furnace::ID, Pack::Cavern), (court::ID, Pack::Crypt)] {
        let rule = of(id);
        let old = before_its_rule(&rule);
        let secs = |boss: &Boss, play: Play| -> (f32, bool) {
            let duels: Vec<Duel> = (0..4).map(|s| duel(duel_room(s, 1, pack, boss, 2), play)).collect();
            (mean(&duels, |d| d.secs), duels.iter().all(|d| d.cleared))
        };
        let (was, _) = secs(&old, Play::Ignore);
        let (played, cleared) = secs(&rule, Play::Rule);
        let (ignored, still) = secs(&rule, Play::Ignore);
        assert!(cleared && still, "{id}: every fight is won");
        assert!(played < ignored, "{id}: played {played:.1}s, ignored {ignored:.1}s");
        assert!(ignored < was * 2.5, "{id}: ignored {ignored:.1}s against {was:.1}s before");
    }
}

#[test]
#[ignore]
fn boss_rule_trace() {
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let id = var("DUEL", furnace::ID);
    let pack = if id == court::ID { Pack::Crypt } else { Pack::Cavern };
    let play = if var("PLAY", "rule") == "rule" { Play::Rule } else { Play::Ignore };
    let knights = var("KNIGHTS", "1").parse().unwrap();
    let run = duel_room(var("SEED", "0").parse().unwrap(), var("DEPTH", "1").parse().unwrap(), pack, &of(&id), knights);
    let d = duel(run, play);
    println!("{d:?}");
}

