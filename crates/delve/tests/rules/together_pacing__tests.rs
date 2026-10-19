//! Floor pacing: what a party meets, room by room, from the entrance to
//! the stairs. A census enters every room of a fresh floor (on a copy of
//! the run, through production `enter`) and counts what stands up.

use super::*;

/// One room as a party finds it.
#[derive(Clone, Debug)]
pub(super) struct Seen {
    pub room: usize,
    pub kind: RoomKind,
    /// Doors from the entrance.
    pub dist: usize,
    pub foes: usize,
    pub bosses: usize,
    pub elites: usize,
    pub kinds: usize,
    /// Whether the floor's guardian (its one boss) stands here.
    pub guardian: bool,
    /// Whether the Boss Key's carrier stands here: an elite, no boss.
    pub carrier: bool,
}

/// A floor's census: every room, and the walk from the entrance to the
/// stairs (room indices, entrance first).
pub(super) struct Census {
    pub depth: u32,
    pub pack: Pack,
    pub rooms: Vec<Seen>,
    pub walk: Vec<usize>,
}

fn doors_from(run: &Run, from: usize) -> (Vec<usize>, Vec<Option<(usize, usize)>>) {
    let n = run.dungeon.rooms.len();
    let mut dist = vec![usize::MAX; n];
    let mut back = vec![None; n];
    dist[from] = 0;
    let mut queue = std::collections::VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        for dir in 0..4 {
            if let Some(next) = run.dungeon.neighbour(at, dir)
                && dist[next] == usize::MAX
            {
                dist[next] = dist[at] + 1;
                back[next] = Some((at, dir));
                queue.push_back(next);
            }
        }
    }
    (dist, back)
}

/// Enter room `i` on a copy of `run` (its gate opened, so a guardian
/// stands) and count what is there.
fn look(run: &Run, i: usize, dist: usize, from: Option<usize>) -> Seen {
    let mut copy = run.clone();
    if copy
        .boss_gates
        .as_ref()
        .is_some_and(|g| g.guardian == i)
    {
        copy.clear_prerequisites_for_test();
    }
    copy.enter(i, from);
    let live = || copy.enemies.iter().filter(|e| e.hp > 0);
    let mut kinds: Vec<EnemyKind> = live().map(|e| e.kind).collect();
    kinds.sort_by_key(|k| format!("{k:?}"));
    kinds.dedup();
    Seen {
        room: i,
        kind: copy.dungeon.rooms[i].kind,
        dist,
        foes: live().count(),
        bosses: live().filter(|e| e.is_boss()).count(),
        elites: live().filter(|e| e.elite).count(),
        kinds: kinds.len(),
        guardian: copy.boss_gates.as_ref().is_some_and(|g| g.guardian == i)
            && live().any(|e| e.is_boss()),
        carrier: live().any(|e| copy.carries_key(e) && e.elite && !e.is_boss()),
    }
}

pub(super) fn census(run: &Run) -> Census {
    let (dist, back) = doors_from(run, 0);
    let stairs = run
        .dungeon
        .rooms
        .iter()
        .position(|r| {
            matches!(
                r.kind,
                RoomKind::Stairs | RoomKind::Lair | RoomKind::Threshold
            )
        })
        .expect("a floor has its stairs");
    let mut walk = vec![stairs];
    while let Some((prev, _)) = back[*walk.last().unwrap()] {
        walk.push(prev);
    }
    walk.reverse();
    let rooms = (0..run.dungeon.rooms.len())
        .filter(|&i| dist[i] != usize::MAX)
        .map(|i| {
            // The side the party walks in on: the door of this room that
            // faces the room it came from.
            let from = back[i].map(|(_, dir)| (dir + 2) % 4);
            look(run, i, dist[i], if i == 0 { None } else { from })
        })
        .collect();
    Census {
        depth: run.dungeon.depth,
        pack: run.dungeon.pack,
        rooms,
        walk,
    }
}

impl Census {
    pub fn seen(&self, room: usize) -> &Seen {
        self.rooms.iter().find(|s| s.room == room).unwrap()
    }
    /// Rooms that raise a boss (a guardian, a chief, the dragon, the Pit
    /// Tyrant, an outlaw), side halls included.
    pub fn boss_rooms(&self) -> Vec<&Seen> {
        self.rooms.iter().filter(|s| s.bosses > 0).collect()
    }
    /// Boss rooms on the walk from the entrance to the stairs.
    pub fn walk_bosses(&self) -> Vec<&Seen> {
        self.walk
            .iter()
            .map(|&i| self.seen(i))
            .filter(|s| s.bosses > 0)
            .collect()
    }
    /// The boss fights a party must win to go down: those on the walk, and
    /// the Boss Key's holder wherever it stands, were it a boss.
    pub fn required_bosses(&self, run: &Run) -> usize {
        let chief = run.boss_gates.as_ref().map(|g| g.leaders[0].room);
        self.walk_bosses().len()
            + chief
                .filter(|c| !self.walk.contains(c) && self.seen(*c).bosses > 0)
                .map_or(0, |_| 1)
    }
    /// Doors from the entrance to the nearest boss.
    pub fn first_boss(&self) -> Option<usize> {
        self.boss_rooms().iter().map(|s| s.dist).min()
    }
    /// The longest walk through boss rooms only, one door to the next.
    pub fn boss_run(&self, run: &Run) -> usize {
        let boss: Vec<usize> = self.boss_rooms().iter().map(|s| s.room).collect();
        fn deepest(run: &Run, boss: &[usize], at: usize, path: &mut Vec<usize>) -> usize {
            path.push(at);
            let mut best = path.len();
            for dir in 0..4 {
                if let Some(next) = run.dungeon.neighbour(at, dir)
                    && boss.contains(&next)
                    && !path.contains(&next)
                {
                    best = best.max(deepest(run, boss, next, path));
                }
            }
            path.pop();
            best
        }
        boss.iter()
            .map(|&b| deepest(run, &boss, b, &mut Vec::new()))
            .max()
            .unwrap_or(0)
    }
}

/// A run for `knights` (1 or 2) on seed `seed`, at the top of floor 1.
pub(super) fn party(seed: u64, knights: usize) -> Run {
    Run::new(seed, 17, (knights > 1).then_some("Friend"))
}

/// Floors 1 to 6 of `seed` for `knights`, each counted as it opens.
pub(super) fn delve(seed: u64, knights: usize) -> Vec<(Census, Run)> {
    let mut run = party(seed, knights);
    let mut out = Vec::new();
    for _ in 1..=DEEPEST {
        out.push((census(&run), run.clone()));
        if run.dungeon.depth >= DEEPEST {
            break;
        }
        run.descend_for_test();
    }
    out
}

fn tag(s: &Seen) -> String {
    let short = match s.kind {
        RoomKind::Start => "start",
        RoomKind::Sanctuary => "sanct",
        RoomKind::Fight => "fight",
        RoomKind::Hall => "hall",
        RoomKind::Stairs => "STAIR",
        RoomKind::Lair => "LAIR",
        RoomKind::Threshold => "THRESH",
        RoomKind::Treasure => "treas",
        _ => "other",
    };
    let mut t = format!("{short}:{}/{}k", s.foes, s.kinds);
    if s.bosses > 0 {
        t += &format!("B{}", s.bosses);
    }
    if s.elites > 0 {
        t += &format!("E{}", s.elites);
    }
    if s.carrier {
        t += "K";
    }
    t
}

/// `cargo test -p angel-delve pacing_census -- --ignored --nocapture`
#[test]
#[ignore]
fn pacing_census() {
    for knights in [1, 2] {
        let mut first = vec![Vec::new(); DEEPEST as usize];
        let mut per_floor = vec![Vec::new(); DEEPEST as usize];
        let mut runs = vec![Vec::new(); DEEPEST as usize];
        let mut walked = vec![Vec::new(); DEEPEST as usize];
        let mut required = vec![Vec::new(); DEEPEST as usize];
        let mut carried = vec![0usize; DEEPEST as usize];
        for seed in 0..24 {
            for (c, run) in delve(seed, knights) {
                let d = c.depth as usize - 1;
                let walk: Vec<String> = c.walk.iter().map(|&i| tag(c.seen(i))).collect();
                let off: Vec<String> = c
                    .boss_rooms()
                    .iter()
                    .filter(|s| !c.walk.contains(&s.room))
                    .map(|s| format!("{}@{}", tag(s), s.dist))
                    .collect();
                let fb = c.first_boss();
                let br = c.boss_rooms().len();
                let rn = c.boss_run(&run);
                println!(
                    "k{knights} s{seed:02} f{} {:?} ramps {} stairs {}: first boss @{} | boss rooms {br} | run {rn} | walk {} | off-walk bosses {}",
                    c.depth,
                    c.pack,
                    run.dungeon.ramps(),
                    run.dungeon.stairs_doors(),
                    fb.map_or("-".into(), |d| d.to_string()),
                    walk.join(" > "),
                    off.join(" ")
                );
                first[d].push(fb.unwrap_or(99));
                per_floor[d].push(br);
                runs[d].push(rn);
                walked[d].push(c.walk_bosses().len());
                required[d].push(c.required_bosses(&run));
                carried[d] += usize::from(c.rooms.iter().any(|s| s.carrier && s.bosses == 0));
            }
        }
        for d in 0..DEEPEST as usize {
            let n = first[d].len().max(1);
            let min = |v: &[usize]| v.iter().copied().min().unwrap_or(0);
            let max = |v: &[usize]| v.iter().copied().max().unwrap_or(0);
            let avg = |v: &[usize]| v.iter().sum::<usize>() as f32 / n as f32;
            println!(
                "SUMMARY k{knights} floor {}: first boss @ min {} avg {:.1} | boss rooms min {} avg {:.1} max {} | longest boss run avg {:.1} max {} | walk boss rooms min {} max {} | required boss fights min {} max {} | elite key carrier {}/{n}",
                d + 1,
                min(&first[d]),
                avg(&first[d]),
                min(&per_floor[d]),
                avg(&per_floor[d]),
                max(&per_floor[d]),
                avg(&runs[d]),
                max(&runs[d]),
                min(&walked[d]),
                max(&walked[d]),
                min(&required[d]),
                max(&required[d]),
                carried[d]
            );
        }
    }
}


/// Seeds the pacing pins walk, each with one knight and with two.
const SEEDS: u64 = 24;

/// Every floor of the seeds that ramps. A floor grown again `GROW_TRIES`
/// times and still unable to ramp is the generator's last resort (the
/// deepest of its tries, `layout::floor_party`), and a few first floors are:
/// `few_floors_are_left_unramped` keeps them few.
fn every_floor(mut check: impl FnMut(&Census, &Run, usize)) {
    for knights in [1, 2] {
        for seed in 0..SEEDS {
            for (census, run) in delve(seed, knights) {
                if run.dungeon.ramps() {
                    check(&census, &run, knights);
                }
            }
        }
    }
}

#[test]
fn few_floors_are_left_unramped() {
    let (mut unramped, mut floors) = (0, 0);
    for knights in [1, 2] {
        for seed in 0..SEEDS {
            for (_, run) in delve(seed, knights) {
                floors += 1;
                unramped += usize::from(!run.dungeon.ramps());
            }
        }
    }
    assert!(unramped * 20 <= floors, "{unramped} of {floors} floors cannot ramp");
}

#[test]
fn floor_one_opens_on_rooms_without_a_boss() {
    // The entrance and every room one door from it: no boss of any kind
    // (guardian, chief, outlaw, Pit Tyrant).
    every_floor(|c, _, knights| {
        if c.depth != 1 {
            return;
        }
        for s in c.rooms.iter().filter(|s| s.dist < pacing::CHIEF_FROM) {
            assert_eq!(s.bosses, 0, "{knights} knights: a boss {} doors in: {s:?}", s.dist);
        }
        assert!(c.first_boss().unwrap() >= pacing::CHIEF_FROM);
    });
}

#[test]
fn every_floor_has_one_boss_room_and_it_holds_the_stairs() {
    every_floor(|c, run, knights| {
        let guardians: Vec<&Seen> = c.rooms.iter().filter(|s| s.guardian).collect();
        assert_eq!(guardians.len(), 1, "{knights} knights, floor {}", c.depth);
        let g = guardians[0];
        assert_eq!(Some(g.room), run.dungeon.guardian());
        assert_eq!(c.walk.last(), Some(&g.room), "the end of the walk");
        assert_eq!(g.dist, run.dungeon.stairs_doors());
        assert!(g.dist >= pacing::STAIRS_FROM, "floor {}: stairs {} doors in", c.depth, g.dist);
        // The one boss fight on the way down: the guardian, at the stairs.
        let walk: Vec<usize> = c.walk_bosses().iter().map(|s| s.room).collect();
        assert_eq!(walk, [g.room], "{knights} knights, floor {}: {:?}", c.depth, c.walk_bosses());
        assert_eq!(c.required_bosses(run), 1, "{knights} knights, floor {}", c.depth);
        // Its one chief's camp waits mid-floor, never in the guardian's
        // doorway, led by an elite with the Boss Key and no boss.
        let gates = run.boss_gates.as_ref().unwrap();
        assert_eq!(gates.leaders.len(), 1);
        let chief = c.seen(gates.leaders[0].room);
        assert_eq!(chief.bosses, 0, "{knights} knights, floor {}: {chief:?}", c.depth);
        assert!(chief.carrier && chief.elites >= 1, "{chief:?}");
        assert_eq!(c.rooms.iter().filter(|s| s.carrier).count(), 1);
        assert!(chief.dist >= pacing::CHIEF_FROM && chief.dist < g.dist, "{chief:?}");
        assert!((0..4).all(|d| run.dungeon.neighbour(chief.room, d) != Some(g.room)));
        // Every other boss is a showdown off the floor's end (the Pit, the
        // Wanted hall), off the walk behind its own door, past the chief's ring.
        for s in c.boss_rooms() {
            if s.room != g.room {
                assert!(matches!(s.kind, RoomKind::Wanted | RoomKind::Pit), "{s:?}");
                assert!(s.dist > pacing::CHIEF_FROM, "{s:?}");
                assert!(!c.walk.contains(&s.room), "{s:?}");
            }
        }
    });
}

/// Floor's chief's camp entered by the production path; everyone but the
/// key carrier falls (the key stays kept), then the carrier: the route
/// opens and the key lies where it fell. Returns where the key lies.
fn slay_the_carrier(run: &mut Run) -> (usize, f32, f32) {
    let room = run.boss_gates.as_ref().unwrap().leaders[0].room;
    run.enter(room, Some(2));
    for hero in run.players.values_mut() {
        hero.invulnerable = 100_000;
    }
    let elite = run
        .enemies
        .iter()
        .find(|e| run.carries_key(e))
        .expect("the key carrier stands in the chief's camp")
        .clone();
    assert!(elite.elite && !elite.is_boss() && elite.boss.is_none(), "{elite:?}");
    assert!(!run.enemies.iter().any(|e| e.is_boss()), "no boss in the chief's camp");
    assert_eq!(run.works.key.hold, keyring::Hold::Kept);
    for e in &mut run.enemies {
        if e.camp_slot != elite.camp_slot {
            e.hp = 0;
        }
    }
    run.step_sim_for_test(&BTreeMap::new());
    assert_eq!(run.works.key.hold, keyring::Hold::Kept);
    assert!(!run.boss_gates.as_ref().unwrap().unlocked());
    let at = {
        let e = run
            .enemies
            .iter_mut()
            .find(|e| e.camp_slot == elite.camp_slot)
            .unwrap();
        e.hp = 0;
        (e.x, e.y)
    };
    run.step_sim_for_test(&BTreeMap::new());
    assert!(run.boss_gates.as_ref().unwrap().leaders[0].defeated);
    assert!(run.boss_gates.as_ref().unwrap().unlocked());
    assert!(run.cues.iter().any(|c| c == "key_drop"));
    let keyring::Hold::Lying { room: lies, x, y } = run.works.key.hold else {
        panic!("the key lies where the carrier fell: {:?}", run.works.key);
    };
    assert_eq!(lies, room);
    assert!((x - at.0).hypot(y - at.1) < 8.0, "key at ({x}, {y}), carrier fell at {at:?}");
    (lies, x, y)
}

#[test]
fn the_key_carrier_is_an_elite_mid_floor_and_drops_the_key() {
    for knights in [1, 2] {
        for seed in 0..SEEDS {
            let mut run = party(seed, knights);
            loop {
                let mut floor = run.clone();
                slay_the_carrier(&mut floor);
                // The camp's room still clears, and the state stands.
                for _ in 0..8 {
                    floor.clear_for_test();
                    if floor.room().cleared {
                        break;
                    }
                }
                assert!(floor.room().cleared);
                assert!(floor.valid_snapshot());
                if run.dungeon.depth >= DEEPEST {
                    break;
                }
                run.descend_for_test();
            }
        }
    }
}

#[test]
fn the_guardians_door_opens_with_the_carriers_key() {
    for seed in [3, 41, 7] {
        let mut run = party(seed, 2);
        let (_, x, y) = slay_the_carrier(&mut run);
        // A knight takes the key up.
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (x, y);
        run.step_sim_for_test(&BTreeMap::new());
        assert_eq!(run.key_bearer(), Some(1));
        // At the guardian's door it opens, and the guardian stands within.
        let guardian = run.boss_gates.as_ref().unwrap().guardian;
        let seen = boss_gates::reachable(&run.dungeon, guardian);
        let (beside, dir) = (0..run.dungeon.rooms.len())
            .filter(|&at| seen[at])
            .find_map(|at| {
                (0..4)
                    .find(|&d| run.dungeon.neighbour(at, d) == Some(guardian))
                    .map(|d| (at, d))
            })
            .expect("a door to the guardian");
        run.enter(beside, Some(2));
        assert!(run.boss_door_shut());
        let (w, h) = (run.room().width(), run.room().height());
        for hero in run.players.values_mut() {
            (hero.x, hero.y) = match dir {
                0 => (w / 2.0, 0.2),
                1 => (w - 0.2, h / 2.0),
                2 => (w / 2.0, h - 0.2),
                _ => (0.2, h / 2.0),
            };
        }
        run.travel();
        assert!(!run.boss_door_shut(), "seed {seed}: the key opens the door");
        assert_eq!(run.works.key.hold, keyring::Hold::Spent);
        assert_eq!(run.at, guardian);
        let apex = run.boss_gates.as_ref().unwrap().guardian_boss;
        assert!(
            run.enemies
                .iter()
                .any(|e| e.is_boss() && (apex.is_none() || e.boss == apex)),
            "seed {seed}: the guardian stands"
        );
    }
}

#[test]
fn no_two_boss_rooms_stand_door_to_door() {
    every_floor(|c, run, knights| {
        assert!(
            c.boss_run(run) <= 1,
            "{knights} knights, floor {}: {:?}",
            c.depth,
            c.boss_rooms()
        );
    });
}

#[test]
fn a_floor_musters_more_monsters_the_deeper_its_rooms_lie() {
    every_floor(|c, _, knights| {
        // The fight rooms without a boss: none musters fewer than a room
        // nearer the entrance, along the walk and across the floor. (On the
        // first floors the key's carrier stands alone, `BOSS_ALONE_TO`: its
        // room is a duel, not a muster.)
        let plain: Vec<&Seen> = c
            .rooms
            .iter()
            .filter(|s| s.kind == RoomKind::Fight && s.bosses == 0)
            .filter(|s| !(s.carrier && c.depth <= crate::rules::BOSS_ALONE_TO))
            .collect();
        for a in &plain {
            for b in &plain {
                if a.dist < b.dist {
                    assert!(
                        a.foes <= b.foes,
                        "{knights} knights, floor {}: {a:?} then {b:?}",
                        c.depth
                    );
                }
            }
        }
        let walk: Vec<usize> = c
            .walk
            .iter()
            .map(|&i| c.seen(i))
            .filter(|s| s.kind == RoomKind::Fight && s.bosses == 0)
            .filter(|s| !(s.carrier && c.depth <= crate::rules::BOSS_ALONE_TO))
            .map(|s| s.foes)
            .collect();
        assert!(walk.windows(2).all(|w| w[0] <= w[1]), "floor {}: {walk:?}", c.depth);
        // The first room past the entrance holds the floor's fewest.
        if c.depth == 1 {
            let first = plain.iter().filter(|s| s.dist == 1).map(|s| s.foes).max();
            let fewest = usize::from(pacing::CAMP_FOES[0][0]);
            assert!(first.is_none_or(|n| n <= fewest + stakes::EXTRA_PER_KNIGHT * (knights - 1)));
        }
    });
}

/// `ANGEL_PACING_SHOTS=dir cargo test -p angel-delve write_floor_one_walk --
/// --ignored`: floor 1 for two knights, room by room from the entrance to
/// the stairs (the chief's room in its place by depth), each a moment after
/// the party walks in.
#[test]
#[ignore]
fn write_floor_one_walk() {
    let Some(dir) = std::env::var_os("ANGEL_PACING_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for seed in [3, 41] {
        let run = party(seed, 2);
        let c = census(&run);
        let chief = run.boss_gates.as_ref().unwrap().leaders[0].room;
        assert!(c.seen(chief).carrier);
        let mut rooms: Vec<usize> = c.walk.clone();
        if !rooms.contains(&chief) {
            rooms.push(chief);
        }
        rooms.sort_by_key(|&i| (c.seen(i).dist, i));
        let (_, back) = doors_from(&run, 0);
        for (n, &i) in rooms.iter().enumerate() {
            let mut copy = run.clone();
            if copy.boss_gates.as_ref().is_some_and(|g| g.guardian == i) {
                copy.clear_prerequisites_for_test();
            }
            let from = back[i].map(|(_, dir)| (dir + 2) % 4);
            copy.enter(i, if i == 0 { None } else { from });
            for _ in 0..30 {
                for hero in copy.players.values_mut() {
                    hero.invulnerable = 999;
                }
                copy.step(&BTreeMap::new());
            }
            let s = c.seen(i);
            let img = crate::render::overworld::arena::frame(&copy, 768, 448);
            let mut out = format!("P6\n{} {}\n255\n", img.w, img.h).into_bytes();
            out.extend(img.rgb_bytes());
            let name = format!(
                "s{seed:02}-{n}-door{}-{:?}-{}foes-{}boss{}.ppm",
                s.dist,
                s.kind,
                s.foes,
                s.bosses,
                if s.carrier { "-keyelite" } else { "" }
            );
            std::fs::write(dir.join(name), out).unwrap();
        }
    }
}
