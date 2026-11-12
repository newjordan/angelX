//! The Deep comes up: the night's judgement, the raid, what it does and what stands against it,
//! and the bounds that keep it from ending a game.
use super::*;
use crate::kingdom::{Command, Status, Stir as Level, catalog};
use crate::rules::site::MAX_LOST;
use std::collections::BTreeMap;

fn idle() -> BTreeMap<u32, Input> {
    BTreeMap::new()
}

/// A rich hillside with the Keep staked, one knight standing in the camp who cannot be felled
/// (the raid's own bounds are tested apart from a fight).
fn hill() -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    run.walk_kingdom(None);
    run.home.industry.found_hillside(catalog(), 7);
    let k = &mut run.home.industry;
    for r in catalog().resources.keys() {
        k.stock.insert(r.clone(), 400);
    }
    k.stock.insert("fodder".into(), 100_000);
    k.stock.remove("coin");
    k.researched = catalog().all_techs().map(|(k, _)| k.clone()).collect();
    let h = run.players.get_mut(&1).unwrap();
    let camp = crate::kingdom::hillside::HILL_CAMP;
    let _ = camp;
    (h.x, h.y) = (170.5 * TILE_UNITS, 112.5 * TILE_UNITS);
    h.invulnerable = 10_000_000;
    h.stone = false;
    for _ in 0..u64::from(HZ) * 12 {
        run.step(&idle());
    }
    for b in run.home.industry.buildings.values_mut() {
        b.construction = 0;
    }
    run
}

fn raise(run: &mut Run, plan: &str, tile: (i16, i16)) -> u32 {
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Build { plan: plan.into(), tile }).unwrap_or_else(|e| panic!("{plan} at {tile:?}: {e}"));
    let id = k.at(tile).unwrap();
    k.buildings.get_mut(&id).unwrap().construction = 0;
    id
}

/// Seconds of play.
fn play(run: &mut Run, seconds: u64) {
    for _ in 0..seconds * u64::from(HZ) {
        run.step(&idle());
    }
}

/// Play up to `seconds`, a tick at a time, until `done` says so; whether it did.
fn play_until(run: &mut Run, seconds: u64, done: impl Fn(&Run) -> bool) -> bool {
    for _ in 0..seconds * u64::from(HZ) {
        run.step(&idle());
        if done(run) {
            return true;
        }
    }
    false
}

/// The sim's second set to `into` seconds into day `day`.
fn clock_at(run: &mut Run, day: u64, into: u64) {
    run.home.industry.second = day * deep_raid::DAY + into;
}

/// A hill that has a lit brazier at the works and the Deep stirred well past uneasy.
fn stirred() -> (Run, u32) {
    let mut run = hill();
    let brazier = raise(&mut run, "brazier", (152, 100));
    run.home.industry.stock.insert("timber".into(), 400);
    run.kingdom.stir.level = 80;
    // The brazier draws its fuel the next second.
    play(&mut run, 3);
    assert!(run.home.industry.buildings[&brazier].fed > 0, "lit");
    (run, brazier)
}

/// The room's tiles round `(x0, y0)..(x1, y1)` as text, raiders as R: for a failing test to show.
#[allow(dead_code)]
fn ascii(run: &Run, (x0, y0, x1, y1): (i32, i32, i32, i32)) -> String {
    let index = world::room_of(&run.dungeon, RoomKind::Kingdom).unwrap();
    let room = &run.dungeon.rooms[index];
    let mut out = String::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let here = run.enemies.iter().any(|e| e.hp > 0 && (e.x / TILE_UNITS) as i32 == x && (e.y / TILE_UNITS) as i32 == y);
            out.push(if here {
                'R'
            } else {
                match room.tile(x, y) {
                    layout::Tile::Floor => '.',
                    layout::Tile::Wall => '#',
                    layout::Tile::Hazard => '~',
                    layout::Tile::Block => 'B',
                    layout::Tile::Door => 'D',
                    layout::Tile::Stairs => '>',
                    layout::Tile::Ledge => '=',
                    layout::Tile::Bog => 'o',
                }
            });
        }
        out.push('\n');
    }
    out
}

fn raiders(run: &Run) -> Vec<&Enemy> {
    let Some(raid) = &run.kingdom.stir.watch.on else { return Vec::new() };
    run.enemies.iter().filter(|e| raid.band.contains(&e.id)).collect()
}

fn theme(run: &mut Run, t: deep_raid::Theme) {
    run.kingdom.stir.watch.raids = deep_raid::Theme::ALL.iter().position(|x| *x == t).unwrap() as u16;
}

// ---------------------------------------------------------------- the day ----

#[test]
fn the_realm_has_a_day_and_a_night_that_falls_at_dusk() {
    assert_eq!(deep_raid::clock(0), (false, deep_raid::DUSK));
    assert_eq!(deep_raid::clock(deep_raid::DUSK - 1), (false, 1));
    assert_eq!(deep_raid::clock(deep_raid::DUSK), (true, deep_raid::DAY - deep_raid::DUSK));
    assert_eq!(deep_raid::clock(deep_raid::DAY - 1), (true, 1));
    assert_eq!(deep_raid::clock(deep_raid::DAY), (false, deep_raid::DUSK), "the next day dawns");
    assert!(!deep_raid::clock(7 * deep_raid::DAY + 399).0);
    assert!(deep_raid::DAY - deep_raid::DUSK > deep_raid::LAST + deep_raid::JUDGE_AFTER, "a raid fits in a night");
}

#[test]
fn a_raid_is_small_the_first_time_and_never_more_than_the_cap() {
    assert_eq!(deep_raid::raid_size(Level::Calm, 0), None);
    assert_eq!(deep_raid::raid_size(Level::Calm, 50), None);
    // The first raid is two whatever the stir.
    for level in [Level::Uneasy, Level::Stirring, Level::Raiding] {
        assert_eq!(deep_raid::raid_size(level, 0), Some(2), "{level:?}");
    }
    assert_eq!(deep_raid::raid_size(Level::Uneasy, 1), Some(2));
    assert_eq!(deep_raid::raid_size(Level::Stirring, 1), Some(4));
    assert_eq!(deep_raid::raid_size(Level::Raiding, 1), Some(6));
    // Past that it grows a raider every third raid, and stops at the cap.
    assert_eq!(deep_raid::raid_size(Level::Raiding, 3), Some(7));
    for raids in 0..=u16::MAX {
        for level in [Level::Uneasy, Level::Stirring, Level::Raiding] {
            let n = deep_raid::raid_size(level, raids).unwrap();
            assert!((2..=deep_raid::RAID_MAX).contains(&n), "{level:?} {raids}: {n}");
        }
    }
    // The raids teach in turn.
    assert_eq!(
        (0..5).map(deep_raid::Theme::of).collect::<Vec<_>>(),
        [deep_raid::Theme::Fire, deep_raid::Theme::Thieves, deep_raid::Theme::Sappers, deep_raid::Theme::Boars, deep_raid::Theme::Fire]
    );
}

#[test]
fn the_deep_is_judged_once_at_dusk_and_only_with_something_to_come_for() {
    let (mut run, _brazier) = stirred();
    // By day nothing comes.
    clock_at(&mut run, 2, 100);
    play(&mut run, 10);
    assert!(run.kingdom.stir.watch.on.is_none() && run.kingdom.stir.watch.raids == 0);
    // At dusk the Deep answers, once.
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 3);
    let raid = run.kingdom.stir.watch.on.clone().expect("the Deep answers");
    assert_eq!((raid.theme, raid.size, run.kingdom.stir.watch.raids), (deep_raid::Theme::Fire, 2, 1), "the first raid is two imps");
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("CLIMB THE GREAT SHAFT")), "{:?}", run.found);
    assert!(run.notices.iter().any(|n| n.contains("NIGHT RAID")));
    // A calm Deep sends nothing.
    let (mut calm, _) = stirred();
    calm.kingdom.stir.level = 0;
    calm.home.industry.buildings.retain(|_, b| b.plan != "brazier");
    clock_at(&mut calm, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER);
    play(&mut calm, 3);
    assert!(calm.kingdom.stir.watch.on.is_none(), "nothing to come for");
    // With nobody on the hillside the night is not judged at all.
    let (mut empty, _) = stirred();
    empty.players.get_mut(&1).unwrap().absent = true;
    clock_at(&mut empty, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER);
    play(&mut empty, 3);
    assert!(empty.kingdom.stir.watch.on.is_none() && empty.kingdom.stir.watch.judged == 0);
    // Judged once: a raid ended is not sent again the same night.
    let judged = run.kingdom.stir.watch.judged;
    run.kingdom.stir.watch.on = None;
    play(&mut run, 3);
    assert!(run.kingdom.stir.watch.on.is_none() && run.kingdom.stir.watch.judged == judged);
    // The next night it is judged afresh.
    clock_at(&mut run, 3, deep_raid::DUSK + deep_raid::JUDGE_AFTER);
    play(&mut run, 3);
    assert_eq!(run.kingdom.stir.watch.raids, 2);
}

#[test]
fn a_castle_that_stands_calms_the_works_it_covers() {
    let (run, _brazier) = stirred();
    let tile = crate::kingdom::hillside::HILL_CAMP;
    let c = catalog();
    let k = &run.home.industry;
    let with = k.pressure_with(c, 40, Some(tile));
    assert!(with.calm > 0, "the camp below the Keep is calmed by it: {with:?}");
    // Far from the Keep the same works are at the Deep's mercy.
    let far = (tile.0 + 90, tile.1 + 40);
    let without = k.pressure_with(c, 40, Some(far));
    assert_eq!(without.calm, 0);
    assert!(without.stir > with.stir);
}

#[test]
fn the_deep_comes_for_the_works_the_castle_does_not_calm() {
    let mut run = hill();
    // One brazier in the Keep's calm, below its wall; one beyond it.
    let near = raise(&mut run, "brazier", (141, 99));
    let far = raise(&mut run, "brazier", (152, 100));
    run.home.industry.stock.insert("timber".into(), 400);
    run.kingdom.stir.level = 60;
    play(&mut run, 3);
    let c = catalog();
    assert!(run.home.industry.calm_at(c, run.home.industry.buildings[&near].tile) > 0);
    assert_eq!(run.home.industry.calm_at(c, run.home.industry.buildings[&far].tile), 0);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 3);
    let raid = run.kingdom.stir.watch.on.clone().expect("the works beyond the calm are raided");
    assert!(raid.targets.contains(&far) && !raid.targets.contains(&near), "{:?} (near {near}, far {far})", raid.targets);
    // With only the calmed works standing, the same stir sends nothing.
    let mut run = hill();
    raise(&mut run, "brazier", (141, 99));
    run.home.industry.stock.insert("timber".into(), 400);
    run.kingdom.stir.level = 60;
    play(&mut run, 3);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 3);
    assert!(run.kingdom.stir.watch.on.is_none(), "the castle's calm holds the Deep off the works it covers");
    assert_eq!(run.kingdom.stir.watch.judged, 3, "the night was judged");
}

// ---------------------------------------------------------------- the raid ----

#[test]
fn raiders_climb_the_shaft_one_by_one_and_march_for_the_nearest_smoke() {
    let (mut run, brazier) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 2);
    assert!(run.kingdom.stir.watch.on.is_some());
    play(&mut run, 5);
    let first: Vec<(f32, f32)> = raiders(&run).iter().map(|e| (e.x, e.y)).collect();
    assert!(!first.is_empty(), "the first is up");
    let (sx, sy) = crate::world::wishes::delve_stand();
    for &(x, y) in &first {
        assert!((x / TILE_UNITS - sx as f32).abs() < 12.0 && (y / TILE_UNITS - sy as f32).abs() < 12.0, "up the shaft: {x},{y}");
    }
    play(&mut run, 6);
    assert_eq!(run.kingdom.stir.watch.on.as_ref().unwrap().came, 2, "both are up");
    // They head for the brazier's light, the nearest smoke there is.
    let at = run.home.industry.buildings[&brazier].tile;
    play(&mut run, 2);
    let goals: Vec<u32> = run.kingdom.stir.watch.on.as_ref().unwrap().goals.values().copied().collect();
    assert!(!goals.is_empty() && goals.iter().all(|x| *x == brazier), "{goals:?}");
    let dist = |run: &Run| -> f32 {
        raiders(run).iter().map(|e| ((e.x / TILE_UNITS) - f32::from(at.0)).hypot((e.y / TILE_UNITS) - f32::from(at.1))).fold(f32::MAX, f32::min)
    };
    let before = dist(&run);
    play(&mut run, 6);
    assert!(dist(&run) < before - 3.0, "they march: {before} to {}", dist(&run));
    assert!(run.valid_snapshot());
}

#[test]
fn a_raid_ends_at_dawn_and_what_is_left_slinks_back_down() {
    let (mut run, _) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 12);
    assert!(!raiders(&run).is_empty());
    clock_at(&mut run, 3, 0);
    play(&mut run, 2);
    assert!(run.kingdom.stir.watch.on.is_none(), "the raid is over");
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("DAWN")), "{:?}", run.found);
    assert!(run.enemies.iter().all(|e| e.hp == 0), "none left on the hill");
    // And no raid outlasts its time in a night.
    let (mut long, _) = stirred();
    clock_at(&mut long, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut long, 12);
    let began = long.kingdom.stir.watch.on.as_ref().unwrap().began;
    long.kingdom.stir.watch.on.as_mut().unwrap().began = began.saturating_sub(deep_raid::LAST + 1);
    play(&mut long, 2);
    assert!(long.kingdom.stir.watch.on.is_none());
}

#[test]
fn the_party_leaving_the_hillside_drops_the_raid() {
    let (mut run, _) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 8);
    assert!(run.kingdom.stir.watch.on.is_some());
    // Another room: the raid's room is not this one.
    run.kingdom.stir.watch.on.as_mut().unwrap().room += 1;
    play(&mut run, 2);
    assert!(run.kingdom.stir.watch.on.is_none());
    assert!(run.enemies.iter().all(|e| e.hp == 0), "the raiders are gone");
    // Gone below for a while and back: the hillside did not see the raid, and it is not here any more.
    let (mut run, _) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 8);
    assert!(run.kingdom.stir.watch.on.is_some());
    run.kingdom.stir.watch.on.as_mut().unwrap().seen = run.tick - 10 * u64::from(HZ);
    play(&mut run, 2);
    assert!(run.kingdom.stir.watch.on.is_none(), "a party that was away does not beat a raid by coming back");
    assert_eq!(run.kingdom.stir.watch.beaten, 0);
}

#[test]
fn a_raid_that_is_beaten_quiets_the_deep() {
    let (mut run, _) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 14);
    let before = run.kingdom.stir.level;
    assert_eq!(run.kingdom.stir.watch.on.as_ref().unwrap().came, 2);
    // The knights cut them down.
    for e in &mut run.enemies {
        e.hp = 0;
    }
    play(&mut run, 3);
    assert!(run.kingdom.stir.watch.on.is_none());
    assert_eq!(run.kingdom.stir.watch.beaten, 1);
    assert!(run.kingdom.stir.level <= before.saturating_sub(deep_raid::RELIEF) + 2, "{} from {before}", run.kingdom.stir.level);
    assert!(run.notices.iter().any(|n| n.contains("raid is broken")));
}

// ---------------------------------------------------------- what they do ----

#[test]
fn imps_set_the_works_alight() {
    let mut run = hill();
    // A working works that smokes: a lime kiln on the camp's road.
    let kiln = raise(&mut run, "lime-kiln", (153, 100));
    let k = &mut run.home.industry;
    k.apply(catalog(), Command::Connect(kiln)).unwrap();
    k.buildings.get_mut(&kiln).unwrap().recipe = Some("burn-lime-coal".into());
    k.buildings.get_mut(&kiln).unwrap().input = Stock::from([("limestone".into(), 32), ("coal".into(), 8)]);
    run.kingdom.stir.level = 80;
    play(&mut run, 3);
    let report = run.home.industry.report(catalog());
    assert_eq!(report.status.get(&kiln), Some(&Status::Working), "the kiln works");
    assert_eq!(run.home.industry.lures(catalog(), &report).iter().map(|l| l.kind).collect::<Vec<_>>(), [crate::kingdom::LureKind::Smoke]);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 3);
    assert_eq!(run.kingdom.stir.watch.on.as_ref().map(|r| r.theme), Some(deep_raid::Theme::Fire));
    let mut lit = false;
    for _ in 0..80 {
        play(&mut run, 1);
        lit |= run.home.industry.havoc.fires.contains_key(&kiln) || run.home.industry.havoc.tally.burnt > 0;
        if lit {
            break;
        }
    }
    assert!(lit, "the imps set the kiln alight");
    assert_eq!(run.home.industry.report(catalog()).status.get(&kiln), Some(&Status::Fire));
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.starts_with("FIRE")), "{:?}", run.found);
    assert!(run.valid_snapshot());
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib deep_raid -- --include-ignored"]
fn thieves_rob_a_store_and_run_for_the_shaft_and_a_felled_thief_drops_it() {
    let (mut run, _) = stirred();
    theme(&mut run, deep_raid::Theme::Thieves);
    run.home.industry.stock.insert("ale".into(), 100);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    let taken = play_until(&mut run, 90, |r| r.kingdom.stir.watch.on.as_ref().is_some_and(|r| !r.loot.is_empty()));
    if !taken {
        let raid = run.kingdom.stir.watch.on.clone();
        let at: Vec<_> = raiders(&run).iter().map(|e| (e.id, (e.x / TILE_UNITS) as i32, (e.y / TILE_UNITS) as i32, e.age)).collect();
        eprintln!("raid {raid:?}\nraiders {at:?} tally {:?} camp {:?}\n{}", run.home.industry.havoc.tally, run.home.industry.camp(), ascii(&run, (118, 76, 150, 98)));
        let f = run.field_for(1);
        eprintln!("field to the camp: {:?}", f.as_ref().map(|f| [(135, 85), (135, 86), (135, 90), (133, 92), (133, 93), (133, 94), (137, 95), (140, 94), (140, 95)].map(|(x, y)| ((x, y), f.at(x, y)))));
    }
    assert!(taken, "a thief has something");
    let raid = run.kingdom.stir.watch.on.clone().unwrap();
    let (id, loot) = raid.loot.iter().next().map(|(a, b)| (*a, b.clone())).unwrap();
    assert_eq!(raid.goals[&id], u32::MAX, "it runs for the shaft");
    let stolen: u32 = loot.values().sum();
    assert!((1..=deep_raid::THIEF_TAKES).contains(&stolen), "{stolen}");
    // Felled before the shaft: the camp has it back.
    let camp = |run: &Run| -> u32 { run.home.industry.stock.values().sum() };
    let had = camp(&run);
    run.enemies.iter_mut().filter(|e| e.id == id).for_each(|e| e.hp = 0);
    play(&mut run, 2);
    assert!(camp(&run) >= had + stolen - 2, "the loot is back: {had} to {}", camp(&run));
    assert!(run.kingdom.stir.watch.on.as_ref().is_none_or(|r| !r.loot.contains_key(&id)));
    // A thief that reaches the shaft's mouth is away for good.
    let (mut run, _) = stirred();
    theme(&mut run, deep_raid::Theme::Thieves);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    assert!(play_until(&mut run, 90, |r| r.kingdom.stir.watch.on.as_ref().is_some_and(|r| !r.loot.is_empty())), "a thief has something");
    let id = *run.kingdom.stir.watch.on.as_ref().unwrap().loot.keys().next().unwrap();
    let (sx, sy) = crate::world::wishes::delve_stand();
    let e = run.enemies.iter_mut().find(|e| e.id == id).unwrap();
    (e.x, e.y) = ((sx as f32 + 0.5) * TILE_UNITS, (sy as f32 + 0.5) * TILE_UNITS);
    play(&mut run, 2);
    assert!(run.enemies.iter().all(|e| e.id != id), "it slipped away down the shaft");
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("SLIPS DOWN THE SHAFT")), "{:?}", run.found);
}

#[test]
fn sappers_cut_a_line_and_the_camps_hands_mend_it() {
    let (mut run, _) = stirred();
    theme(&mut run, deep_raid::Theme::Sappers);
    // A rollerway running past the brazier.
    for x in 149..156 {
        run.home.industry.apply(catalog(), Command::Roller { piece: crate::kingdom::Piece::Roller, tile: (x, 102), dir: crate::kingdom::Dir::E }).unwrap();
    }
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    let mut cut = false;
    for _ in 0..100 {
        play(&mut run, 1);
        if run.home.industry.havoc.tally.cut > 0 {
            cut = true;
            break;
        }
    }
    assert!(cut, "a line is cut");
    let k = &run.home.industry;
    assert!(!k.havoc.cuts.is_empty() && !k.plans.ghosts.is_empty());
    assert!(k.plans.ghosts.values().all(|g| g.priority == 2 && g.by == 0));
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("LINE IS CUT")), "{:?}", run.found);
    // Over, and then the hands mend it.
    run.end_raid(None);
    play(&mut run, 200);
    assert!(run.home.industry.havoc.cuts.is_empty(), "mended: {:?}", run.home.industry.havoc.cuts);
    assert_eq!(run.home.industry.rollers.cells.len(), 7, "the whole line is laid again");
    assert!(run.valid_snapshot());
}

#[test]
fn a_wall_holds_the_raid_and_a_tower_shoots_it() {
    let (mut run, brazier) = stirred();
    // A palisade ring round the brazier, a tower with arrows beside it.
    let ring: Vec<(i16, i16)> = (150..=154).flat_map(|x| [(x, 98), (x, 102)]).chain((99..=101).flat_map(|y| [(150, y), (154, y)])).collect();
    for t in &ring {
        raise(&mut run, "palisade", *t);
    }
    let tower = raise(&mut run, "watchtower", (156, 100));
    run.home.industry.apply(catalog(), Command::Connect(tower)).ok();
    run.home.industry.buildings.get_mut(&tower).unwrap().input.insert("arrow".into(), 24);
    play(&mut run, 3);
    assert_eq!(run.home.industry.report(catalog()).status[&tower], Status::Working, "manned and armed");
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 20);
    let (mut shot, mut battered) = (false, false);
    for _ in 0..100 {
        play(&mut run, 1);
        shot |= run.home.industry.buildings[&tower].input.get("arrow").copied().unwrap_or(0) < 24;
        battered |= !run.home.industry.havoc.worn.is_empty() || run.home.industry.havoc.tally.down > 0;
        if run.kingdom.stir.watch.on.is_none() {
            break;
        }
    }
    assert!(shot, "the tower looses its arrows");
    assert!(battered, "the raiders batter the palisade");
    let _ = brazier;
    assert!(run.valid_snapshot());
}

/// The top left of an open stretch of ground `w` across and `h` deep on the hillside, below the camp.
fn open_ground(run: &Run, w: i32, h: i32) -> (i32, i32) {
    let index = world::room_of(&run.dungeon, RoomKind::Kingdom).unwrap();
    let room = &run.dungeon.rooms[index];
    for y in (100..150).step_by(2) {
        for x in (150..210).step_by(2) {
            if (x..x + w).all(|xx| (y..y + h).all(|yy| room.tile(xx, yy) == layout::Tile::Floor)) {
                return (x, y);
            }
        }
    }
    panic!("no open ground {w} by {h}");
}

/// The way a raider walks to `goal` from `from`: whether it goes through any tile of `wall`, and whether it gets to the work.
fn way(run: &mut Run, goal: u32, from: (i32, i32), wall: &[(i16, i16)]) -> (bool, bool) {
    let field = run.field_for(goal).expect("a field");
    let mut at = from;
    let mut through = false;
    for _ in 0..200 {
        let Some(next) = deep_raid::step_of(&field, at.0, at.1) else { break };
        through |= wall.contains(&(next.0 as i16, next.1 as i16));
        at = next;
        if field.at(at.0, at.1) <= 14 {
            break;
        }
    }
    (through, field.at(at.0, at.1) <= 14)
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib deep_raid -- --include-ignored"]
fn a_defence_in_the_way_is_cheaper_to_go_round_than_through_and_a_gap_is_a_way_in() {
    // The field of cheapest ways: floor costs a step, a wall costs what it will take to break.
    let mut run = hill();
    let (x0, y0) = open_ground(&run, 44, 12);
    let (x0, y0) = (x0 as i16, y0 as i16);
    // A short wall of stone with open ground round its ends, the brazier behind it.
    let wall: Vec<(i16, i16)> = (x0 + 18..=x0 + 26).map(|x| (x, y0 + 5)).collect();
    for t in &wall {
        raise(&mut run, "wall", *t);
    }
    let goal = raise(&mut run, "brazier", (x0 + 22, y0 + 8));
    run.home.industry.stock.insert("timber".into(), 400);
    play(&mut run, 3);
    let from = (i32::from(x0) + 22, i32::from(y0) + 2);
    assert_eq!(way(&mut run, goal, from, &wall), (false, true), "round the end, not through a wall of 120 hold, and so to the work");
    // A long wall with no end near: through is cheaper than round, and the way goes through it.
    let mut run = hill();
    let (x0, y0) = open_ground(&run, 44, 12);
    let (x0, y0) = (x0 as i16, y0 as i16);
    let wall: Vec<(i16, i16)> = (x0..x0 + 44).map(|x| (x, y0 + 5)).collect();
    for t in &wall {
        raise(&mut run, "palisade", *t);
    }
    let goal = raise(&mut run, "brazier", (x0 + 22, y0 + 8));
    run.home.industry.stock.insert("timber".into(), 400);
    play(&mut run, 3);
    let from = (i32::from(x0) + 22, i32::from(y0) + 2);
    let (through, got) = way(&mut run, goal, from, &wall);
    assert!(through && got, "a palisade of 40 is broken through rather than walked round for 22 tiles");
    // The same long wall with a gate in it: the gap is the way in.
    let mut run = hill();
    let (x0, y0) = open_ground(&run, 44, 12);
    let (x0, y0) = (x0 as i16, y0 as i16);
    let wall: Vec<(i16, i16)> = (x0..x0 + 44).filter(|x| !(x0 + 28..x0 + 30).contains(x)).map(|x| (x, y0 + 5)).collect();
    for t in &wall {
        raise(&mut run, "wall", *t);
    }
    raise(&mut run, "wood-gate", (x0 + 28, y0 + 5));
    let goal = raise(&mut run, "brazier", (x0 + 22, y0 + 8));
    run.home.industry.stock.insert("timber".into(), 400);
    play(&mut run, 3);
    let from = (i32::from(x0) + 22, i32::from(y0) + 2);
    assert_eq!(way(&mut run, goal, from, &wall), (false, true), "through the gate");
}

// ------------------------------------------------------------- the siege ----

/// A run with a boar at the Keep's wall and the lift in hand low enough to reach.
fn at_the_wall() -> (Run, u32, (i32, i32)) {
    let mut run = hill();
    clock_at(&mut run, 2, deep_raid::DUSK + 30);
    run.kingdom.stir.watch.raids = 3;
    run.kingdom.stir.watch.on = Some(deep_raid::Raid {
        theme: deep_raid::Theme::Boars,
        size: 2,
        came: 1,
        began: run.home.industry.second,
        room: run.at,
        seen: run.tick,
        targets: Vec::new(),
        band: Vec::new(),
        goals: BTreeMap::new(),
        loot: BTreeMap::new(),
        stones: 0,
        blows: BTreeMap::new(),
    });
    let laid = run.sites.keep().unwrap().laid;
    assert!(laid > 975, "the lift in hand is the second: {laid}");
    // A boar beside the stone laid last.
    let castle = run.sites.keep().unwrap().castle().unwrap();
    let last = castle.stones[(laid - 1) as usize].aabb().map(|v| f32::from(v) / 100.0);
    let (ox, oz) = works::origin_px();
    let (tx, ty) = (((ox + (last[0] + last[3]) / 2.0 * works::PX_PER_M) / 16.0).floor() as i32, ((oz - (last[1] + last[4]) / 2.0 * works::PX_PER_M) / 16.0).floor() as i32);
    let id = run.spawn_body(EnemyKind::Warboar, (tx as f32 + 0.5) * TILE_UNITS, (ty as f32 + 0.5) * TILE_UNITS).expect("a boar");
    run.kingdom.stir.watch.on.as_mut().unwrap().band.push(id);
    (run, id, (tx, ty))
}

#[test]
fn a_raid_knocks_stones_out_of_an_unfinished_wall_and_what_they_held_comes_down() {
    let (mut run, id, at) = at_the_wall();
    let laid = run.sites.keep().unwrap().laid;
    let first = run.sites.keep().unwrap().castle().unwrap().stages[2].first;
    assert!(run.sites.keep().unwrap().lost.is_empty());
    // Blows: a plain stone takes STONE_BLOWS to part.
    for _ in 0..deep_raid::STONE_BLOWS {
        run.siege_blow(id, at);
    }
    let site = run.sites.keep().unwrap();
    assert!(!site.lost.is_empty(), "stones are out");
    assert!(site.lost.iter().all(|&i| i >= first && i < laid), "only the lift in hand: {:?} from {first}", site.lost);
    assert_eq!(site.laid, laid, "the wall's height is what it was: holes, not a lower wall");
    assert!(site.valid());
    let raid = run.kingdom.stir.watch.on.as_ref().unwrap();
    assert_eq!(usize::from(raid.stones), site.lost.len());
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("PULL") && t.contains("STONES")), "{:?}", run.found);
    // What stood on the stone that went is gone with it, and no more: the lost are the stone and
    // what the rows say it held.
    let rows = works::rests(works::KEEP).unwrap();
    let gone: std::collections::BTreeSet<u32> = site.lost.iter().copied().collect();
    assert!(rows.falls(first, laid, &gone).is_empty(), "everything that should have fallen has");
}

#[test]
fn true_stones_hold_longer_and_a_raid_brings_down_only_so_much() {
    let (mut run, id, at) = at_the_wall();
    // Every stone of the lift in hand set true: five blows, not two.
    let laid = run.sites.keep().unwrap().laid;
    let first = run.sites.keep().unwrap().castle().unwrap().stages[2].first;
    run.sites.keep_mut().unwrap().trues = vec![(first, laid)];
    for _ in 0..deep_raid::TRUE_BLOWS - 1 {
        run.siege_blow(id, at);
    }
    assert!(run.sites.keep().unwrap().lost.is_empty(), "a true stone is not knocked out by four blows");
    run.siege_blow(id, at);
    let site = run.sites.keep().unwrap();
    assert!(!site.lost.is_empty(), "five do it");
    // A stone knocked out is true no more: what is set in its place is a new stone.
    assert!(site.lost.iter().all(|&i| !site.is_true(i)));
    assert!(site.valid());
    // However long they batter, a raid brings down no more than its share.
    for _ in 0..800 {
        run.siege_blow(id, at);
    }
    let raid = run.kingdom.stir.watch.on.as_ref().unwrap();
    assert!(raid.stones <= deep_raid::STONE_BUDGET, "{}", raid.stones);
    assert!(run.sites.keep().unwrap().lost.len() <= deep_raid::STONE_BUDGET as usize);
    assert!(run.sites.keep().unwrap().lost.len() <= MAX_LOST);
    assert!(run.sites.keep().unwrap().valid());
    assert!(run.valid_snapshot());
}

#[test]
fn the_first_raid_that_reaches_the_wall_takes_only_a_few_stones() {
    let (mut run, id, at) = at_the_wall();
    run.kingdom.stir.watch.raids = 1;
    for _ in 0..800 {
        run.siege_blow(id, at);
    }
    let out = run.sites.keep().unwrap().lost.len();
    assert!(out > 0 && out <= deep_raid::FIRST_STONES as usize, "{out}");
}

#[test]
fn the_masons_and_the_knights_lay_the_stones_again_before_they_lay_another() {
    let mut site = crate::rules::site::Site::keep();
    site.lay(100, 0);
    let laid = site.laid;
    site.trues = vec![(laid - 50, laid - 40)];
    assert_eq!(site.lose(&[laid - 45, laid - 3, laid - 2]), 3);
    assert_eq!(site.lost, vec![laid - 45, laid - 3, laid - 2]);
    assert!(!site.stands(laid - 45) && site.stands(laid - 46));
    assert_eq!(site.true_count(), 9, "the true stone that went is not true");
    assert!(site.valid());
    assert_eq!(site.next(), laid - 45);
    // A knight's true set fills the first hole and is true again.
    site.lay_set(2, true);
    assert_eq!(site.lost, vec![laid - 3, laid - 2]);
    assert!(site.is_true(laid - 45) && site.true_count() == 10);
    assert_eq!(site.laid, laid);
    // The masons' stones go into the holes first, then on.
    site.lay(3, 0);
    assert!(site.lost.is_empty());
    assert_eq!(site.laid, laid + 1, "two filled the holes, one went on");
    assert!(site.valid());
    // Stones not laid, or already out, are not lost.
    assert_eq!(site.lose(&[site.laid + 5, 0, 0]), 1);
    assert!(site.valid());
}

#[test]
fn the_masons_relay_what_a_raid_took_and_it_costs_the_yard() {
    let (mut run, id, at) = at_the_wall();
    for _ in 0..deep_raid::STONE_BLOWS {
        run.siege_blow(id, at);
    }
    let out = run.sites.keep().unwrap().lost.len();
    assert!(out > 0);
    run.end_raid(None);
    let laid = run.sites.keep().unwrap().laid;
    let sim = run.keep_live().expect("the Keep is staked in the sim");
    // A lodge beside the yard, its crew staffed; the yard stocked with what the stage in hand needs.
    let c = catalog();
    let k = &mut run.home.industry;
    for g in ["timber", "plank", "stone", "block"] {
        k.stock.insert(g.into(), 100);
    }
    k.apply(c, Command::Build { plan: "lodge".into(), tile: (150, 90) }).expect("a lodge");
    let lodge = k.at((150, 90)).unwrap();
    k.buildings.get_mut(&lodge).unwrap().construction = 0;
    k.apply(c, Command::Connect(lodge)).expect("a road to the lodge");
    let yard = k.sites[&sim].yard;
    let site = &k.sites[&sim];
    let needs = crate::kingdom::site::stage_needs(c, &c.castles[&site.plan].stages[usize::from(site.stage)]);
    for (r, n) in needs {
        *k.buildings.get_mut(&yard).unwrap().input.entry(r).or_default() += n.min(480);
    }
    for r in ["pole", "centring"] {
        *k.buildings.get_mut(&yard).unwrap().input.entry(r.into()).or_default() += 40;
    }
    let before = k.buildings[&yard].input.clone();
    run.hands.yard.timber = 4_000;
    let mut relaid = false;
    for _ in 0..30 {
        play(&mut run, 10);
        if run.sites.keep().unwrap().lost.len() < out {
            relaid = true;
            break;
        }
    }
    assert!(relaid, "the masons lay the stones again: still {} out of {out}", run.sites.keep().unwrap().lost.len());
    assert!(run.sites.keep().unwrap().laid >= laid, "the wall did not go lower");
    // It was paid for out of the yard.
    let after = &run.home.industry.buildings[&yard].input;
    assert!(after["ashlar"] < before["ashlar"], "{after:?} against {before:?}");
    assert!(run.sites.keep().unwrap().valid() && run.valid_snapshot());
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib deep_raid -- --include-ignored"]
fn a_breach_in_a_wall_is_a_way_in_and_laying_the_stone_closes_it() {
    // The tiles of the Keep follow the stones that stand: a tile closed by a stone that is out is open.
    let mut run = hill();
    let site = run.sites.keep().unwrap().clone();
    let castle = site.castle().unwrap();
    let (x0, y0, w, h) = castle.tiles();
    let mut breach = None;
    'find: for y in y0..y0 + h {
        for x in x0..x0 + w {
            let gate = crate::world::plan::tabula::DELVE_GATE;
            let in_gate = x >= gate.0 && y >= gate.1 && x < gate.0 + gate.2 && y < gate.1 + gate.3;
            if !in_gate && castle.blocked(x, y, site.laid) && !castle.is_ditch(x, y) && castle.need(x, y) > 0 && castle.need(x, y) <= site.laid {
                breach = Some((x, y, castle.need(x, y) - 1));
                break 'find;
            }
        }
    }
    let (x, y, stone) = breach.expect("a wall tile");
    let index = world::room_of(&run.dungeon, RoomKind::Kingdom).unwrap();
    let tile = |run: &Run| run.dungeon.rooms[index].tile(x, y);
    run.follow_site_tiles();
    assert_eq!(tile(&run), layout::Tile::Wall, "closed by the stone");
    run.sites.keep_mut().unwrap().lost = vec![stone];
    run.follow_site_tiles();
    assert_eq!(tile(&run), layout::Tile::Floor, "a breach");
    run.sites.keep_mut().unwrap().lay(1, 0);
    run.follow_site_tiles();
    assert_eq!(tile(&run), layout::Tile::Wall, "closed again");
}

// -------------------------------------------------------------- the bounds ----

#[test]
fn a_party_beaten_on_the_hillside_is_knocked_down_not_killed() {
    let (mut run, _) = stirred();
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    play(&mut run, 10);
    assert!(run.kingdom.stir.watch.on.is_some());
    let h = run.players.get_mut(&1).unwrap();
    h.invulnerable = 0;
    h.hp = 0;
    play(&mut run, 2);
    assert!(run.kingdom.stir.watch.on.is_none(), "the raiders take their due and go");
    assert_ne!(run.phase, Phase::Wiped, "not a wipe");
    assert!(run.players[&1].hp > 0, "knocked down, not killed");
    assert!(run.found.as_ref().is_some_and(|(_, _, t)| t.contains("TAKE THEIR DUE")), "{:?}", run.found);
}

#[test]
fn many_nights_unopposed_never_end_the_game_and_everything_lost_is_laid_out_again() {
    let (mut run, _brazier) = stirred();
    let _sawpit = raise(&mut run, "sawpit", (156, 104));
    let yard = raise(&mut run, "carters-yard", (143, 99));
    run.home.industry.apply(catalog(), Command::Connect(yard)).ok();
    run.home.industry.apply(catalog(), Command::Carts { yard, carts: 3 }).ok();
    let shaft = run.home.industry.buildings.iter().find(|(_, b)| catalog().plans[&b.plan].shaft > 0).map(|(id, _)| *id).unwrap();
    for night in 2..10u64 {
        run.kingdom.stir.level = 100;
        clock_at(&mut run, night, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
        play(&mut run, 100);
        clock_at(&mut run, night + 1, 0);
        play(&mut run, 2);
        let k = &run.home.industry;
        assert!(k.buildings.contains_key(&1), "the camp stands, night {night}");
        assert!(k.buildings.contains_key(&shaft), "the shaft stands, night {night}");
        assert!(run.sites.keep().is_some() && run.keep_live().is_some(), "the Keep's site stands");
        assert!(k.havoc.fires.len() <= 8 && k.plans.ghosts.len() < 64, "bounded: {} fires {} plans", k.havoc.fires.len(), k.plans.ghosts.len());
        assert!(run.kingdom.stir.watch.on.is_none(), "dawn ends the raid");
        assert!(run.valid_snapshot(), "night {night}");
        assert!(run.sites.keep().unwrap().lost.len() <= deep_raid::STONE_BUDGET as usize, "night {night}: {} stones out, {} raids", run.sites.keep().unwrap().lost.len(), run.kingdom.stir.watch.raids);
        // The camp's hands have what they need to mend, as a party that plays would.
        for (good, n) in [("timber", 120), ("plank", 60), ("stone", 60), ("nails", 60), ("block", 30)] {
            run.home.industry.stock.entry(good.into()).and_modify(|x| *x = (*x).max(n)).or_insert(n);
        }
    }
    // A day of peace after the last night: the hands set everything right.
    clock_at(&mut run, 10, 0);
    run.home.industry.havoc.fires.clear();
    play(&mut run, 300);
    let k = &run.home.industry;
    assert!(k.havoc.cuts.len() <= k.plans.ghosts.len(), "every cut has its plan");
    assert!(run.valid_snapshot());
}

// -------------------------------------------------------------- what is seen ----

#[test]
fn the_view_reads_a_fire_a_battered_wall_a_broken_line_and_the_nights_clock() {
    let mut run = hill();
    let c = catalog();
    let sawpit = raise(&mut run, "sawpit", (153, 104));
    let wall = raise(&mut run, "wall", (150, 102));
    let tower = raise(&mut run, "watchtower", (156, 100));
    run.home.industry.apply(c, Command::Road((152, 108))).unwrap();
    run.home.industry.apply(c, Command::Roller { piece: crate::kingdom::Piece::Roller, tile: (148, 106), dir: crate::kingdom::Dir::E }).unwrap();
    let k = &mut run.home.industry;
    assert!(k.ignite(c, sawpit));
    k.batter(c, wall, 50);
    k.cut_near(c, (148, 106), 0).unwrap();
    k.cut_near(c, (152, 108), 0).unwrap();
    k.apply(c, Command::Connect(tower)).ok();
    k.buildings.get_mut(&tower).unwrap().input.clear();
    play(&mut run, 3);
    let view = crate::world::hillside::of(&run).expect("the hillside is read");
    let placed = |id: u32| view.buildings.iter().find(|b| b.id == id).expect("a building");
    assert_eq!(placed(sawpit).status, Status::Fire);
    assert_eq!(placed(wall).hurt, Some((70, 120)));
    assert_eq!(placed(tower).hurt, None);
    assert!(view.marks.iter().any(|m| m.words.contains("IS ON FIRE")), "{:?}", view.marks.iter().map(|m| &m.words).collect::<Vec<_>>());
    // The tower says what it lacks: arrows, or a man to loose them.
    assert!(view.marks.iter().any(|m| m.words.starts_with("WATCHTOWER")), "{:?}", view.marks.iter().map(|m| &m.words).collect::<Vec<_>>());
    let deep: Vec<_> = view.plans.iter().filter(|p| p.deep).collect();
    assert_eq!(deep.len(), 2, "the two cuts are the Deep's plans");
    assert!(deep.iter().all(|p| p.priority == 2));
    // The clock the plate shows.
    clock_at(&mut run, 4, 100);
    assert_eq!(run.deep_clock(), (false, deep_raid::DUSK - 100));
    clock_at(&mut run, 4, deep_raid::DUSK + 5);
    assert_eq!(run.deep_clock().0, true);
    assert_eq!(run.raid_status(), None);
}

/// A ring of stone wall (hold 120 a tile) round a brazier, towers or none beside it, the fourth raid (imps) coming: how long
/// the raiders were at the wall before it first fell (if it did), and whether any raider was left at the end.
fn besiege(towers: usize) -> (Option<u64>, usize, u16) {
    let c = catalog();
    let (mut run, _brazier) = {
        let mut run = hill();
        let b = raise(&mut run, "brazier", (153, 101));
        run.home.industry.stock.insert("timber".into(), 800);
        run.kingdom.stir.level = 100;
        (run, b)
    };
    let ring: Vec<(i16, i16)> = (150..=156).flat_map(|x| [(x, 97), (x, 104)]).chain((98..=103).flat_map(|y| [(150, y), (156, y)])).collect();
    for t in &ring {
        raise(&mut run, "wall", *t);
    }
    for at in [(158, 100), (147, 100)].into_iter().take(towers) {
        let t = raise(&mut run, "watchtower", at);
        run.home.industry.apply(c, Command::Connect(t)).ok();
        run.home.industry.buildings.get_mut(&t).unwrap().input.insert("arrow".into(), 24);
    }
    run.kingdom.stir.watch.raids = 4;
    play(&mut run, 3);
    clock_at(&mut run, 2, deep_raid::DUSK + deep_raid::JUDGE_AFTER - 1);
    let mut first_fall = None;
    let mut began = None;
    let mut best_hold = 0u16;
    for s in 0..100u64 {
        play(&mut run, 1);
        let k = &run.home.industry;
        let worn_walls = ring.iter().filter_map(|t| k.at(*t)).filter_map(|id| k.defence_of(c, id)).filter(|(l, m)| l < m).count();
        if worn_walls > 0 && began.is_none() {
            began = Some(s);
        }
        best_hold = best_hold.max(ring.iter().filter_map(|t| k.at(*t)).filter_map(|id| k.defence_of(c, id)).map(|(l, m)| m - l).max().unwrap_or(0));
        if first_fall.is_none() && ring.iter().any(|t| k.at(*t).is_none()) {
            first_fall = Some(s - began.unwrap_or(s));
        }
        // The raid began (the fifth is next) and is over.
        if run.kingdom.stir.watch.raids > 4 && run.kingdom.stir.watch.on.is_none() {
            break;
        }
    }
    assert!(run.kingdom.stir.watch.raids > 4, "the raid came");
    (first_fall, raiders(&run).len(), best_hold)
}

#[test]
fn a_wall_holds_the_raid_for_a_while_and_a_tower_buys_it_the_time_to_win() {
    // No tower: the wall is battered and held for a while, and falls.
    let (fell, _, hurt) = besiege(0);
    eprintln!("no tower: the wall first fell {fell:?} s after the first blow; most it lost at once {hurt}");
    assert!(hurt > 0, "the raiders batter the wall");
    // With towers (and arrows): no wall falls, and the raiders are beaten.
    let (fell2, left, hurt2) = besiege(2);
    eprintln!("two towers: wall fell {fell2:?}; raiders left {left}; most the wall lost {hurt2}");
    assert_eq!(fell2, None, "the wall holds when the towers shoot");
    assert_eq!(left, 0, "and the raid is broken");
    // The no-tower wall either fell, or took far more of a beating than the one under the towers' guns.
    assert!(fell.is_some() || hurt > hurt2, "{fell:?} {hurt} against {hurt2}");
}
