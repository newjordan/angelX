//! The head count in the first rooms: one knight to a full table of six,
//! walking onto an open floor (as the live box lays every floor) and into
//! the first chambers off the stairs, the bots of the open-floor tests at
//! the controls.
//!
//! The 2026-10-20 play-test: five friends, near naked, walked into floor
//! two's first room and were wiped there, and quit. `party_scale_sim`
//! prints, for every party size, floor and kit, how often the party fell in
//! its first two rooms and how long the first took;
//! `five_and_six_knights_with_the_starting_kit_clear_the_first_rooms` holds
//! the full table to a fair fight.
//!
//! `cargo test -p angel-delve --lib party_scale_sim -- --ignored --nocapture`
//! (env: PS_SEEDS, PS_DEPTHS, PS_NS, PS_KITS = naked,kit,shield,roll,
//! turtle, PS_ROOMS, PS_SECS).
//!
//! `hell_scale_sim` walks the same parties into the circles below the hell
//! gate (env as above, and HS_CIRCLES; floors 1 and 2 of each circle and the
//! starting kit unless told otherwise), a line per circle, floor and size.

use super::*;

const NAMES: [&str; 6] = ["Wren", "Bors", "Ysolde", "Kay", "Tor", "Elaine"];
const INTRO: [&str; 4] = ["percival", "lynette", "gareth", "galahad"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kit {
    /// The identity start as it went live: a shape, the ball, no mail.
    Naked,
    /// The intro's knights as they are dressed: the normal start.
    Kit,
    /// The intro's knights, every one holding a kite shield.
    Shield,
    /// The intro's knights, every one with a dodge roll: no shields.
    Roll,
    /// Every one holding a kite shield, and holding it up whenever a shot
    /// is coming within `TURTLE_REACH`, as a knight who lives behind it.
    Turtle,
}

/// How near a coming shot brings a turtling knight's shield up.
const TURTLE_REACH: f32 = 6.0;

/// Whether a shot is coming at `hero` from within `TURTLE_REACH`.
fn coming(run: &Run, hero: &Hero) -> bool {
    run.projectiles.iter().filter(|p| p.hostile && p.ttl > 0 && p.traits.path.wait == 0).any(|p| {
        let (dx, dy) = (hero.x - p.x, hero.y - p.y);
        let d = dx.hypot(dy).max(0.01);
        d < TURTLE_REACH && (p.vx * dx + p.vy * dy) / (p.vx.hypot(p.vy).max(0.01) * d) > 0.5
    })
}

/// A party of `n` on open floor `depth` of hell's circle `circle` (0: above),
/// the host's seat empty while there is a seat to spare (as on the live box),
/// playing at a full table of six.
pub(crate) fn circle_run(seed: u64, depth: u32, n: usize, kit: Kit, circle: u32) -> Run {
    let mut run = Run::new(seed, 700 + seed, None);
    let naked = kit == Kit::Naked;
    run.naked_start = naked;
    run.identity = naked;
    run.restore_gear(1);
    let absent = n < MAX_PLAYERS as usize;
    let last = if absent { n as u32 + 1 } else { n as u32 };
    for id in 2..=last {
        run.join_seat(id, NAMES[(id as usize - 2) % NAMES.len()]);
    }
    if absent {
        run.set_absent(1, true);
    }
    if !naked {
        let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&i, _)| i).collect();
        for (i, id) in ids.into_iter().enumerate() {
            run.outfit(id, knights::knight(INTRO[i % INTRO.len()]).unwrap());
            let guard = match kit {
                Kit::Shield | Kit::Turtle => run.book.get("kite-shield").cloned(),
                Kit::Roll => run.book.get("dodge-roll").cloned(),
                _ => None,
            };
            if let Some(card) = guard
                && let Some(hero) = run.players.get_mut(&id)
            {
                hero.guard = card.guard.unwrap_or_default();
                hero.guard_card = Some(card.id.clone());
            }
        }
    }
    for hero in run.players.values_mut() {
        hero.invulnerable = 0;
        hero.hp = hero.max_hp;
    }
    run.spectacle.circle = circle;
    run.open_floor_for_test(depth);
    assert_eq!(run.party_len(), n);
    run
}

/// What the party's walk into its first rooms came to.
#[derive(Clone, Debug, Default)]
pub(crate) struct Outing {
    /// Seconds from the first room's company waking to its last falling.
    pub first: Option<f32>,
    /// Rooms cleared, of those walked to.
    pub cleared: usize,
    pub wiped: bool,
    /// Knights who went down at some point.
    pub fell: usize,
    /// Health lost, percent of the party's pool.
    pub lost: u32,
    pub ticks: u64,
    /// The most one knight lost in any one second, percent of its health.
    pub burst: u32,
    /// The quickest a knight went from full health to down, in seconds.
    pub to_down: Option<f32>,
    /// Hits taken and their sum, percent of the knight's health.
    pub hits: u32,
    pub hit_pct: u32,
    /// The first room's company: how many, and their health together.
    pub foes: u32,
    pub foe_hp: u32,
    /// What raised shields turned, summed over the party, and the times
    /// one was knocked aside.
    pub turned: u32,
    pub breaks: u32,
    /// Knights holding a shield.
    pub shields: u32,
}

/// The chambers a party walks into first: the fight rooms nearest the
/// stairs with a company in them.
fn first_chambers(run: &Run, rooms: usize) -> Vec<usize> {
    let o = run.dungeon.open.as_ref().expect("an open floor");
    let mut order: Vec<usize> = (0..o.chambers.len())
        .filter(|&i| {
            let c = &o.chambers[i];
            !c.cleared
                && !c.guarded()
                && c.kind != RoomKind::Start
                && run.enemies.iter().any(|e| e.home == Some(i as u8) && e.hp > 0)
        })
        .collect();
    order.sort_by_key(|&i| (o.chambers[i].dist, i));
    order.truncate(rooms);
    order
}

/// Walk the party into its first `rooms` fight chambers in turn, each
/// fought until its company is dead, the party falls, or `cap` seconds.
pub(crate) fn first_rooms(run: &mut Run, rooms: usize, cap: u32, turtle: bool) -> Outing {
    let ids: Vec<u32> = run.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
    let pool: u32 = ids.iter().map(|i| run.players[i].max_hp).sum();
    let mut paths: BTreeMap<u32, Vec<(i32, i32)>> = BTreeMap::new();
    let mut out = Outing::default();
    let mut fell = std::collections::BTreeSet::new();
    let mut lost = 0u32;
    let start = run.tick;
    let mut window: BTreeMap<u32, std::collections::VecDeque<u32>> = BTreeMap::new();
    let mut full_at: BTreeMap<u32, u64> = ids.iter().map(|&i| (i, run.tick)).collect();
    out.shields = ids.iter().filter(|i| run.players[i].guard == cards::Guard::Shield).count() as u32;
    let down = |out: &mut Outing, at: u64, since: u64| {
        let secs = at.saturating_sub(since) as f32 / HZ as f32;
        out.to_down = Some(out.to_down.map_or(secs, |d: f32| d.min(secs)));
    };
    for (k, room) in first_chambers(run, rooms).into_iter().enumerate() {
        let mut woke: Option<u64> = None;
        if k == 0 {
            let company = run.enemies.iter().filter(|e| e.home == Some(room as u8) && e.hp > 0);
            (out.foes, out.foe_hp) = company.fold((0, 0), |(n, hp), e| (n + 1, hp + e.max_hp));
        }
        for _ in 0..cap * HZ {
            let Some(o) = run.dungeon.open.clone() else { break };
            if o.chambers[room].cleared {
                break;
            }
            if run.at != o.map {
                let door = o.doors.iter().find(|d| d.room == run.at).copied();
                run.enter(o.map, door.map(|d| usize::from(d.dir)));
                continue;
            }
            if woke.is_none() && run.enemies.iter().any(|e| e.home == Some(room as u8) && e.hp > 0 && !e.dormant) {
                woke = Some(run.tick);
            }
            // Toward the nearest of the room's company still standing, as a
            // player hunts the last of a room; the room's middle before.
            let (cx, cy) = o.chambers[room].centre();
            let lead = ids.iter().find_map(|i| run.players.get(i).filter(|h| h.hp > 0)).map_or((cx, cy), |h| (h.x, h.y));
            let goal = run
                .enemies
                .iter()
                .filter(|e| e.home == Some(room as u8) && e.hp > 0 && !e.dormant)
                .map(|e| (e.x, e.y))
                .min_by(|a, b| (a.0 - lead.0).hypot(a.1 - lead.1).total_cmp(&(b.0 - lead.0).hypot(b.1 - lead.1)))
                .unwrap_or((cx, cy));
            let inputs: BTreeMap<u32, Input> = ids
                .iter()
                .map(|&id| {
                    let mut input = depths::tests::bot(run, id, goal, paths.entry(id).or_default());
                    if turtle && let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) {
                        input.dash |= coming(run, hero);
                    }
                    (id, input)
                })
                .collect();
            let before: BTreeMap<u32, (u32, u32)> =
                ids.iter().map(|i| (*i, (run.players[i].hp, run.players[i].max_hp))).collect();
            let worn: BTreeMap<u32, shield::Wear> = ids.iter().map(|i| (*i, run.players[i].shield_wear)).collect();
            run.step_sim_for_test(&inputs);
            for (id, was) in &worn {
                let Some(h) = run.players.get(id) else { continue };
                out.turned += h.shield_wear.turned.saturating_sub(was.turned);
                out.breaks += u32::from(knocked(was, &h.shield_wear));
            }
            let wiped = run.phase == Phase::Wiped;
            for &id in &ids {
                let Some(h) = run.players.get(&id) else { continue };
                let (was, max) = before[&id];
                // A wipe dresses the party again: the last to fall fell now.
                let hp = if wiped { 0 } else { h.hp };
                let d = if h.max_hp == max { was.saturating_sub(hp) } else { 0 };
                lost += d;
                if d > 0 {
                    out.hits += 1;
                    out.hit_pct += d * 100 / max.max(1);
                }
                let w = window.entry(id).or_default();
                w.push_back(d);
                if w.len() > HZ as usize {
                    w.pop_front();
                }
                out.burst = out.burst.max(w.iter().sum::<u32>() * 100 / max.max(1));
                if was > 0 && hp == 0 {
                    down(&mut out, run.tick, full_at[&id]);
                }
                if hp >= h.max_hp {
                    full_at.insert(id, run.tick);
                }
                if hp == 0 {
                    fell.insert(id);
                }
            }
            if wiped {
                out.wiped = true;
                break;
            }
        }
        let cleared = run.dungeon.open.as_ref().is_some_and(|o| o.chambers[room].cleared);
        if cleared {
            out.cleared += 1;
            if k == 0 {
                out.first = Some((run.tick - woke.unwrap_or(run.tick)) as f32 / HZ as f32);
            }
        }
        if out.wiped || !cleared {
            break;
        }
    }
    out.fell = if out.wiped { ids.len() } else { fell.len() };
    out.ticks = run.tick - start;
    out.lost = lost * 100 / pool.max(1);
    out
}

/// Whether a shield went from `was` to `now` by being knocked aside.
fn knocked(was: &shield::Wear, now: &shield::Wear) -> bool {
    now.down > was.down
}

fn env_list(name: &str, default: &[u32]) -> Vec<u32> {
    std::env::var(name)
        .ok()
        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect::<Vec<u32>>())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_vec())
}

/// One row of the table: wipes, first clears, the first room's time.
pub(crate) struct Row {
    pub kit: Kit,
    pub circle: u32,
    pub depth: u32,
    pub n: usize,
    pub outings: Vec<Outing>,
}

impl Row {
    pub fn wipe_pct(&self) -> f32 {
        self.outings.iter().filter(|o| o.wiped).count() as f32 * 100.0 / self.outings.len().max(1) as f32
    }
    pub fn first_clear_pct(&self) -> f32 {
        self.outings.iter().filter(|o| o.first.is_some()).count() as f32 * 100.0 / self.outings.len().max(1) as f32
    }
    pub fn burst(&self) -> (f32, u32) {
        let b: Vec<u32> = self.outings.iter().map(|o| o.burst).collect();
        (b.iter().sum::<u32>() as f32 / b.len().max(1) as f32, b.iter().copied().max().unwrap_or(0))
    }
    /// The quickest full-to-down, over the outings with a knight down.
    pub fn to_down(&self) -> f32 {
        self.outings.iter().filter_map(|o| o.to_down).fold(f32::NAN, f32::min)
    }
    pub fn first_secs(&self) -> f32 {
        let t: Vec<f32> = self.outings.iter().filter_map(|o| o.first).collect();
        if t.is_empty() { f32::NAN } else { t.iter().sum::<f32>() / t.len() as f32 }
    }
}

/// Every (kit, depth, size) of the table above the gate, `seeds` outings
/// each, run on as many threads as the machine has.
pub(crate) fn table(kits: &[Kit], depths: &[u32], ns: &[usize], seeds: u64, rooms: usize, cap: u32) -> Vec<Row> {
    hell_table(kits, &[0], depths, ns, seeds, rooms, cap)
}

/// The table for every circle of `circles` too (0: above the gate).
pub(crate) fn hell_table(kits: &[Kit], circles: &[u32], depths: &[u32], ns: &[usize], seeds: u64, rooms: usize, cap: u32) -> Vec<Row> {
    let cells: Vec<(Kit, u32, u32, usize)> = kits
        .iter()
        .flat_map(|&k| {
            circles
                .iter()
                .flat_map(move |&c| depths.iter().flat_map(move |&d| ns.iter().map(move |&n| (k, c, d, n))))
        })
        .collect();
    let jobs: Vec<(usize, u64)> = (0..cells.len()).flat_map(|c| (0..seeds).map(move |s| (c, s))).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(jobs.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::Mutex::new(Vec::<(usize, u64, Outing)>::new());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let j = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(&(c, seed)) = jobs.get(j) else { break };
                let (kit, circle, depth, n) = cells[c];
                let mut run = circle_run(seed, depth, n, kit, circle);
                let out = first_rooms(&mut run, rooms, cap, kit == Kit::Turtle);
                done.lock().unwrap().push((c, seed, out));
            });
        }
    });
    let mut done = done.into_inner().unwrap();
    done.sort_by_key(|d| (d.0, d.1));
    cells
        .iter()
        .enumerate()
        .map(|(c, &(kit, circle, depth, n))| Row {
            kit,
            circle,
            depth,
            n,
            outings: done.iter().filter(|d| d.0 == c).map(|d| d.2.clone()).collect(),
        })
        .collect()
}

fn env_kits(default: &str) -> Vec<Kit> {
    std::env::var("PS_KITS")
        .unwrap_or_else(|_| default.into())
        .split(',')
        .filter_map(|k| match k.trim() {
            "naked" => Some(Kit::Naked),
            "kit" => Some(Kit::Kit),
            "shield" => Some(Kit::Shield),
            "roll" => Some(Kit::Roll),
            "turtle" => Some(Kit::Turtle),
            _ => None,
        })
        .collect()
}

/// A row's numbers after its tag, as the table prints them.
fn row_line(r: &Row) -> String {
    let k = r.outings.len().max(1) as f32;
    format!(
        "{},{},{:.0},{:.0},{:.1},{:.2},{:.2},{:.0},{:.0},{},{:.1},{:.0},{:.1},{:.1},{:.0},{:.0}",
        r.n,
        r.outings.len(),
        r.wipe_pct(),
        r.first_clear_pct(),
        r.first_secs(),
        r.outings.iter().map(|o| o.cleared as f32).sum::<f32>() / k,
        r.outings.iter().map(|o| o.fell as f32).sum::<f32>() / k,
        r.outings.iter().map(|o| o.lost as f32).sum::<f32>() / k,
        r.burst().0,
        r.burst().1,
        r.to_down(),
        r.outings.iter().map(|o| o.hit_pct).sum::<u32>() as f32 / r.outings.iter().map(|o| o.hits).sum::<u32>().max(1) as f32,
        r.outings.iter().map(|o| o.hits).sum::<u32>() as f32 * 60.0 * HZ as f32
            / (r.outings.iter().map(|o| o.ticks).sum::<u64>().max(1) as f32 * r.n as f32),
        r.outings.iter().map(|o| o.foes as f32).sum::<f32>() / k,
        r.outings.iter().map(|o| o.foe_hp as f32).sum::<f32>() / k,
        r.outings.iter().filter(|o| !o.wiped && o.first.is_none()).count() as f32 * 100.0 / k,
    )
}

const COLUMNS: &str = "n,seeds,wipe%,first-clear%,first-secs,rooms-cleared,fell/run,lost%,burst1s-avg%,burst1s-max%,fastest-down-s,mean-hit%,hits/knight/min,foes,foe-hp,timeout%";

/// What each shield came to over a row's outings, by `what`.
fn per_shield(r: &Row, what: impl Fn(&Outing) -> u32) -> f32 {
    let shields: u32 = r.outings.iter().map(|o| o.shields).sum();
    r.outings.iter().map(what).sum::<u32>() as f32 / shields.max(1) as f32
}

/// The table, a line per kit, floor and party size.
#[test]
#[ignore]
fn party_scale_sim() {
    let seeds = u64::from(env_list("PS_SEEDS", &[8])[0]);
    let depths = env_list("PS_DEPTHS", &[1, 2, 3, 4, 5, 6]);
    let ns: Vec<usize> = env_list("PS_NS", &[1, 2, 3, 4, 5, 6]).into_iter().map(|n| n as usize).collect();
    let rooms = env_list("PS_ROOMS", &[2])[0] as usize;
    let cap = env_list("PS_SECS", &[120])[0];
    let rows = table(&env_kits("naked,kit"), &depths, &ns, seeds, rooms, cap);
    println!();
    println!("PSHEAD,kit,depth,{COLUMNS},shields,turned/shield,breaks/shield");
    for r in &rows {
        let k = r.outings.len().max(1) as f32;
        println!(
            "PSROW,{:?},{},{},{:.1},{:.0},{:.2}",
            r.kit,
            r.depth,
            row_line(r),
            r.outings.iter().map(|o| o.shields as f32).sum::<f32>() / k,
            per_shield(r, |o| o.turned),
            per_shield(r, |o| o.breaks),
        );
    }
}

/// The table below the gate: a line per kit, circle, floor and party size.
#[test]
#[ignore]
fn hell_scale_sim() {
    let seeds = u64::from(env_list("PS_SEEDS", &[8])[0]);
    let circles = env_list("HS_CIRCLES", &[1, 2, 3, 4, 5]);
    let depths = env_list("PS_DEPTHS", &[1, 2]);
    let ns: Vec<usize> = env_list("PS_NS", &[2, 4, 6]).into_iter().map(|n| n as usize).collect();
    let rooms = env_list("PS_ROOMS", &[2])[0] as usize;
    let cap = env_list("PS_SECS", &[120])[0];
    let rows = hell_table(&env_kits("kit"), &circles, &depths, &ns, seeds, rooms, cap);
    println!();
    println!("HSHEAD,kit,circle,depth,{COLUMNS}");
    for r in &rows {
        println!("HSROW,{:?},{},{},{}", r.kit, r.circle, r.depth, row_line(r));
    }
}

/// Five and six knights in the normal starting kit walk into the first two
/// fight rooms of floors one and two and win them, as a full table should:
/// the first room cleared nearly always, the two seldom costing the party
/// the run. And the ward after a hit holds: in no second does a knight lose
/// more than one hit may take, and none goes from full health to down
/// faster than two hits a ward apart. On the build of the 2026-10-20
/// play-test (five friends wiped in their first room) six knights cleared
/// floor two's first room in 56% of these walks and lost the run in 44%.
#[test]
fn five_and_six_knights_with_the_starting_kit_clear_the_first_rooms() {
    let rows = table(&[Kit::Kit], &[1, 2], &[5, 6], 16, 2, 120);
    let ward = crate::rules::hero::HIT_WARD as f32 / HZ as f32;
    for r in &rows {
        let at = format!("{} knights, floor {}", r.n, r.depth);
        assert!(r.first_clear_pct() >= 80.0, "{at}: the first room cleared in {:.0}% of walks", r.first_clear_pct());
        assert!(r.wipe_pct() <= 40.0, "{at}: the party fell in {:.0}% of walks", r.wipe_pct());
        let (_, burst) = r.burst();
        assert!(burst <= crate::rules::hero::ONE_HIT_MOST, "{at}: a knight lost {burst}% in one second");
        let down = r.to_down();
        assert!(down.is_nan() || down >= ward - 0.05, "{at}: a knight went from full to down in {down:.1} s");
    }
}
