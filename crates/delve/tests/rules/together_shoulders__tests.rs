use super::*;

fn party(n: u32) -> Run {
    let mut run = Run::new(7, 1, None);
    for id in 2..=n {
        run.join_seat(id, &format!("Knight {id}"));
    }
    let at = run.at;
    run.open_room_for_test(at);
    run.enemies.clear();
    run.projectiles.clear();
    run.calm_for_test();
    let y = run.room().height() / 2.0;
    for (i, hero) in run.players.values_mut().enumerate() {
        (hero.idle, hero.stone, hero.away) = (false, false, false);
        (hero.x, hero.y) = (6.0 + i as f32 * 2.2, y);
        (hero.aim_x, hero.aim_y) = (1.0, 0.0);
        hero.hp = hero.max_hp;
        hero.invulnerable = 0;
    }
    run
}

fn press(run: &mut Run, keys: &[(u32, Input)]) {
    let inputs: BTreeMap<u32, Input> = keys.iter().copied().collect();
    run.step_sim_for_test(&inputs);
}

fn ride() -> Input {
    Input { ride: true, ..Input::default() }
}

fn tap(run: &mut Run, id: u32) {
    press(run, &[(id, ride())]);
    press(run, &[]);
}

fn walk(x: i8, y: i8) -> Input {
    Input { move_x: x, move_y: y, ..Input::default() }
}

fn at(run: &Run, id: u32) -> (f32, f32) {
    let h = &run.players[&id];
    (h.x, h.y)
}

fn ground(run: &Run, (x, y): (f32, f32)) -> bool {
    let grid = Grid { room: run.room(), barred: run.barred() };
    ground_at(&grid, (x, y))
}

fn tower(n: u32) -> Run {
    let mut run = party(n);
    for id in 2..=n {
        let (x, y) = at(&run, 1);
        run.players.get_mut(&id).unwrap().x = x + 2.0;
        run.players.get_mut(&id).unwrap().y = y;
        tap(&mut run, id);
    }
    run
}

#[test]
fn a_knight_climbs_on_rides_along_and_hops_off() {
    let mut run = party(2);
    tap(&mut run, 2);
    assert_eq!(run.shoulders.riders(1), &[2]);
    assert_eq!(run.shoulders.carrier_of(2), Some(1));
    assert_eq!(at(&run, 2), at(&run, 1), "the rider sits on the carrier");
    let start = at(&run, 1);
    for _ in 0..20 {
        press(&mut run, &[(1, walk(1, 0)), (2, walk(0, 1))]);
        assert_eq!(at(&run, 2), at(&run, 1));
    }
    assert!(at(&run, 1).0 > start.0 + 5.0, "the carrier walked");
    let before = run.projectiles.len();
    press(&mut run, &[(2, Input { aim_x: 1, fire: true, ..Input::default() })]);
    assert!(run.projectiles.len() > before, "a rider fires");
    press(&mut run, &[]);
    tap(&mut run, 2);
    assert!(run.shoulders.stacks.is_empty());
    let (c, r) = (at(&run, 1), at(&run, 2));
    let apart = (c.0 - r.0).hypot(c.1 - r.1);
    assert!((1.0..=3.5).contains(&apart), "hopped down beside the carrier: {apart}");
    assert!(ground(&run, r), "onto clear ground");
    assert!(!run.players[&2].aloft);
}

#[test]
fn a_held_ride_key_climbs_once() {
    let mut run = party(2);
    for _ in 0..10 {
        press(&mut run, &[(2, ride())]);
    }
    assert_eq!(run.shoulders.riders(1), &[2], "held, the key climbed once and stayed up");
}

#[test]
fn nobody_in_reach_climbs_nothing() {
    let mut run = party(2);
    run.players.get_mut(&2).unwrap().x += 10.0;
    tap(&mut run, 2);
    assert!(run.shoulders.stacks.is_empty());
}

#[test]
fn the_whole_party_stacks_five_high_and_each_new_rider_goes_on_top() {
    let run = tower(6);
    assert_eq!(run.shoulders.riders(1), &[2, 3, 4, 5, 6]);
    for id in 2..=6 {
        assert_eq!(run.shoulders.level(id), id as usize - 1);
        assert_eq!(at(&run, id), at(&run, 1));
        assert!(!run.players[&id].targetable(), "a rider is no monster's mark");
    }
    assert!(run.players[&1].targetable());
    assert_eq!(run.shoulders.riders(1).len(), MAX_RIDERS);
}

#[test]
fn a_rider_climbing_on_a_rider_goes_to_the_top_of_their_tower() {
    let mut run = party(3);
    tap(&mut run, 2);
    let (x, y) = at(&run, 2);
    (run.players.get_mut(&3).unwrap().x, run.players.get_mut(&3).unwrap().y) = (x + 2.0, y);
    tap(&mut run, 3);
    assert_eq!(run.shoulders.riders(1), &[2, 3]);
}

#[test]
fn a_tower_walks_slower_for_every_rider() {
    let mut lone = party(6);
    let mut stacked = tower(6);
    let (a0, b0) = (at(&lone, 1).0, at(&stacked, 1).0);
    for _ in 0..15 {
        press(&mut lone, &[(1, walk(1, 0))]);
        press(&mut stacked, &[(1, walk(1, 0))]);
    }
    let (a, b) = (at(&lone, 1).0 - a0, at(&stacked, 1).0 - b0);
    let ratio = b / a;
    let want = 1.0 - PACE_PER_RIDER * 5.0;
    assert!((ratio - want).abs() < 0.03, "a five-high tower walks at {ratio:.3} of a lone knight, want {want}");
}

#[test]
fn a_downed_carrier_throws_everyone_off_onto_clear_ground() {
    let mut run = tower(4);
    run.players.get_mut(&1).unwrap().hp = 0;
    press(&mut run, &[]);
    assert!(run.shoulders.stacks.is_empty());
    let mut spots = Vec::new();
    for id in 2..=4 {
        let hero = &run.players[&id];
        assert!(!hero.aloft && hero.targetable(), "knight {id} back on their feet");
        assert!(ground(&run, (hero.x, hero.y)));
        spots.push((hero.x, hero.y));
    }
    for (i, a) in spots.iter().enumerate() {
        for b in &spots[i + 1..] {
            assert!((a.0 - b.0).hypot(a.1 - b.1) > 0.9, "each lands on their own spot");
        }
    }
}

#[test]
fn a_felled_rider_falls_off_and_the_rest_drop_down() {
    let mut run = tower(4);
    run.players.get_mut(&3).unwrap().hp = 0;
    press(&mut run, &[]);
    assert_eq!(run.shoulders.riders(1), &[2, 4]);
    assert_eq!(run.shoulders.level(4), 2);
}

#[test]
fn a_side_on_hall_or_stone_brings_a_tower_down() {
    let mut run = tower(3);
    run.players.get_mut(&1).unwrap().stone = true;
    press(&mut run, &[]);
    assert!(run.shoulders.stacks.is_empty(), "a carrier turned to stone drops the riders");
}

#[test]
fn riders_go_through_a_doorway_with_their_carrier() {
    let mut run = tower(3);
    let next = (0..run.dungeon.rooms.len()).find(|&i| i != run.at && run.dungeon.rooms[i].kind == RoomKind::Fight).unwrap();
    run.enter_for_test(next);
    press(&mut run, &[]);
    assert_eq!(run.shoulders.riders(1), &[2, 3], "the tower stands in the new room");
    assert_eq!(at(&run, 2), at(&run, 1));
    assert_eq!(at(&run, 3), at(&run, 1));
}

fn hostile(x: f32, y: f32, vx: f32, damage: u32) -> Projectile {
    Projectile {
        x,
        y,
        vx,
        vy: 0.0,
        hostile: true,
        look: None,
        kind: Shot::Ember,
        damage,
        pierce: 0,
        last_hit: None,
        empowered: false,
        traits: Default::default(),
        ttl: 50,
    }
}

#[test]
fn a_hit_on_a_tower_lands_on_the_carrier_and_the_riders_take_a_share() {
    let mut run = tower(3);
    run.players.get_mut(&3).unwrap().invulnerable = 20;
    let (x, y) = at(&run, 1);
    let hp: Vec<u32> = (1..=3).map(|id| run.players[&id].hp).collect();
    run.projectiles.push(hostile(x - 1.5, y, 20.0, 20));
    for _ in 0..6 {
        press(&mut run, &[]);
    }
    let lost: Vec<u32> = (1..=3).map(|id| hp[id as usize - 1] - run.players[&id].hp).collect();
    assert!(lost[0] > 0, "the carrier is struck");
    assert_eq!(lost[1], (lost[0] * RIDER_SHARE).div_ceil(100).max(1), "the rider takes their share");
    assert_eq!(lost[2], 0, "a rider in grace takes nothing");
    let ward = run.players[&2].hit_ward;
    assert!(run.players[&2].invulnerable > 0 && run.players[&2].invulnerable <= ward, "and then has their own grace");
}

fn carrier_at(x: f32, y: f32, aim: (f32, f32)) -> Run {
    let mut run = party(2);
    for (id, hero) in run.players.iter_mut() {
        (hero.x, hero.y) = (x + if *id == 2 { 2.0 } else { 0.0 }, y);
        (hero.aim_x, hero.aim_y) = aim;
    }
    tap(&mut run, 2);
    assert_eq!(run.shoulders.riders(1), &[2]);
    run
}

fn land(run: &mut Run) {
    for _ in 0..HZ {
        if run.shoulders.flights.is_empty() {
            return;
        }
        press(run, &[]);
    }
}

fn wide_room(run: &mut Run) {
    let at = run.at;
    run.dungeon.rooms[at].regrow(48, 20);
    for row in 0..20 {
        for col in 0..48 {
            let edge = row == 0 || col == 0 || row == 19 || col == 47;
            run.dungeon.rooms[at].set(col, row, if edge { Tile::Wall } else { Tile::Floor });
        }
    }
}

#[test]
fn a_throw_carries_one_hallway_along_the_aim() {
    let mut run = carrier_at(6.0, 20.0, (1.0, 0.0));
    wide_room(&mut run);
    tap(&mut run, 1);
    let flight = *run.shoulders.flight(2).expect("thrown");
    assert_eq!(flight.ticks, THROW_TICKS);
    let reach = (flight.to.0 - flight.from.0).hypot(flight.to.1 - flight.from.1);
    assert!((reach - THROW_REACH).abs() <= 0.25, "a clear throw carries {reach}, a hallway is {THROW_REACH}");
    assert!(run.shoulders.stacks.is_empty());
    for _ in 0..THROW_TICKS / 2 {
        press(&mut run, &[]);
    }
    let mid = run.shoulders.flight(2).unwrap();
    assert!(mid.height_at(mid.progress()) > THROW_PEAK * 0.8);
    land(&mut run);
    let (x, y) = at(&run, 2);
    assert!((x - flight.to.0).abs() < 1e-3 && (y - flight.to.1).abs() < 1e-3);
    assert!(run.players[&2].invulnerable > 0, "lands with a moment's grace");
    assert!(run.players[&2].targetable());
    assert!(run.shoulders.rearm.contains_key(&1));
}

#[test]
fn the_throw_is_the_open_floors_hallway_measured() {
    let mut lengths = Vec::new();
    for seed in 0..200u64 {
        let mut rng = Rng::new(seed.wrapping_mul(0x9e37_79b9) ^ 0x5eed);
        let depth = 1 + (seed % 3) as u32;
        let floor = layout::open_floor(depth, Pack::Crypt, 1 + (seed % 4) as usize, &mut rng);
        let open = floor.open.expect("an open floor");
        for &(a, b) in &open.links {
            let (ca, cb) = (&open.chambers[usize::from(a)], &open.chambers[usize::from(b)]);
            let (ca, cb) = if ca.cell < cb.cell { (ca, cb) } else { (cb, ca) };
            let gap = if ca.cell.1 == cb.cell.1 {
                i32::from(cb.rect.0) - i32::from(ca.rect.0 + ca.rect.2)
            } else {
                i32::from(cb.rect.1) - i32::from(ca.rect.1 + ca.rect.3)
            };
            lengths.push(gap + 2);
        }
    }
    lengths.sort_unstable();
    let median = lengths[lengths.len() / 2];
    let tiles = THROW_REACH / TILE_UNITS;
    println!(
        "passages: {} measured, {}..{} tiles, quartiles {} {} {}",
        lengths.len(),
        lengths[0],
        lengths[lengths.len() - 1],
        lengths[lengths.len() / 4],
        median,
        lengths[lengths.len() * 3 / 4]
    );
    assert!((median as f32 - tiles).abs() <= 1.0, "a hallway is {median} tiles, the throw {tiles}");
}

#[test]
fn a_knight_in_the_air_is_untouchable() {
    let mut run = carrier_at(6.0, 20.0, (1.0, 0.0));
    wide_room(&mut run);
    tap(&mut run, 1);
    press(&mut run, &[]);
    let hp = run.players[&2].hp;
    assert!(!run.players[&2].targetable());
    let (x, y) = at(&run, 2);
    for _ in 0..THROW_TICKS {
        if run.shoulders.flight(2).is_none() {
            break;
        }
        let (x, y) = at(&run, 2);
        run.projectiles.push(hostile(x - 0.6, y, 30.0, 40));
        press(&mut run, &[]);
    }
    assert_eq!(run.players[&2].hp, hp, "nothing struck the flier");
    let _ = (x, y);
}

#[test]
fn the_landing_slams_the_monsters_round_it_and_throws_them_back() {
    let mut run = carrier_at(6.0, 20.0, (1.0, 0.0));
    wide_room(&mut run);
    let to = (6.0 + THROW_REACH, 20.0);
    for (dx, dy) in [(1.5, 0.0), (-1.5, 1.0), (0.0, -2.0), (9.0, 0.0)] {
        run.spawn_at_for_test(EnemyKind::Skeleton, to.0 + dx, to.1 + dy);
    }
    for e in &mut run.enemies {
        (e.hp, e.max_hp) = (500, 500);
        e.frozen = 10 * HZ;
    }
    let before: Vec<(f32, f32)> = run.enemies.iter().map(|e| (e.x, e.y)).collect();
    tap(&mut run, 1);
    let thrown = run.tick;
    while run.shoulders.flight(2).is_some() && run.tick < thrown + u64::from(HZ) {
        press(&mut run, &[]);
    }
    assert_eq!(run.shoulders.slams.len(), 1);
    let d = |p: (f32, f32)| (p.0 - to.0).hypot(p.1 - to.1);
    for (i, e) in run.enemies.iter().enumerate() {
        if d(before[i]) <= SLAM_REACH {
            assert!(e.hp < 500, "monster {i} in reach is struck");
            assert!(d((e.x, e.y)) > d(before[i]) + 1.0, "monster {i} is thrown back");
        } else {
            assert_eq!(e.hp, 500, "monster {i} out of reach is untouched");
        }
    }
}

#[test]
fn a_throw_at_a_wall_lands_short_of_it_on_the_floor() {
    let mut run = carrier_at(6.0, 20.0, (1.0, 0.0));
    wide_room(&mut run);
    let at_ = run.at;
    for row in 1..19 {
        run.dungeon.rooms[at_].set(10, row, Tile::Wall);
        run.dungeon.rooms[at_].set(9, row, Tile::Hazard);
    }
    tap(&mut run, 1);
    let flight = *run.shoulders.flight(2).unwrap();
    assert!(flight.to.0 + HERO_RADIUS < 9.0 * TILE_UNITS, "short of the pit and the wall: {:?}", flight.to);
    assert!(ground(&run, flight.to));
    assert!(flight.ticks < THROW_TICKS, "a short throw is a short flight");
    land(&mut run);
    assert!(ground(&run, at(&run, 2)));
}

#[test]
fn a_throw_flies_over_a_pit_and_lands_beyond_it() {
    let mut run = carrier_at(6.0, 20.0, (1.0, 0.0));
    wide_room(&mut run);
    let at_ = run.at;
    for row in 1..19 {
        for col in 6..12 {
            run.dungeon.rooms[at_].set(col, row, Tile::Hazard);
        }
    }
    let end = ((6.0 + THROW_REACH) / TILE_UNITS) as usize;
    for row in 1..19 {
        for col in end - 1..end + 2 {
            run.dungeon.rooms[at_].set(col, row, Tile::Hazard);
        }
    }
    tap(&mut run, 1);
    let to = run.shoulders.flight(2).unwrap().to;
    assert!(to.0 > 12.0 * TILE_UNITS, "over the first pit: {to:?}");
    assert!(to.0 < (end - 1) as f32 * TILE_UNITS, "short of the second: {to:?}");
    assert!(ground(&run, to));
}

#[test]
fn a_throw_never_goes_out_through_a_doorway() {
    let mut run = carrier_at(5.0, 21.0, (-1.0, 0.0));
    wide_room(&mut run);
    let at_ = run.at;
    run.dungeon.rooms[at_].set(0, 10, Tile::Door);
    tap(&mut run, 1);
    let to = run.shoulders.flight(2).unwrap().to;
    assert!(to.0 >= TILE_UNITS + HERO_RADIUS && ground(&run, to), "landed inside the room: {to:?}");
}

#[test]
fn the_ride_key_goes_over_the_wire_and_back() {
    for held in [false, true] {
        let cmd = command::DelveCmd {
            held: Input { ride: held, move_x: 1, fire: true, ..Input::default() },
            pressed: Input { ride: true, ..Input::default() },
        };
        let mut bytes = Vec::new();
        cmd.encode(&mut bytes);
        assert_eq!(command::DelveCmd::decode(&bytes), Some(cmd));
    }
    let json = serde_json::to_string(&Input::default()).unwrap();
    assert!(!json.contains("ride"), "an unpressed key is left out for older hosts");
}

#[test]
fn towers_and_flights_go_over_the_wire_and_read_back() {
    let mut run = tower(4);
    wide_room(&mut run);
    tap(&mut run, 1);
    press(&mut run, &[]);
    assert!(!run.shoulders.flights.is_empty() && !run.shoulders.stacks.is_empty());
    let copy = run.wire_copy();
    assert_eq!(copy.shoulders, run.shoulders);
    assert!(!copy.predictable(2) && !copy.predictable(4));
    assert!(copy.predictable(1));
    let mut ahead = copy.clone();
    let mut sim = run.clone();
    for _ in 0..5 {
        ahead.predict(1, walk(1, 0));
        press(&mut sim, &[(1, walk(1, 0))]);
    }
    let (a, s) = (at(&ahead, 1), at(&sim, 1));
    assert!((a.0 - s.0).abs() < 1e-3 && (a.1 - s.1).abs() < 1e-3, "predicted {a:?}, stepped {s:?}");
    let before = sim.pose();
    let mine = ahead.players[&1].clone();
    sim.drawn_with(&before, 0.5, Some((1, &mine)), |drawn| {
        for id in [2, 3] {
            assert_eq!(at(drawn, id), (mine.x, mine.y), "rider {id} sits on the carrier as drawn");
        }
    });
    let lone = party(2).wire_copy();
    assert!(lone.shoulders.quiet());
}

fn bot(run: &Run, id: u32) -> Input {
    let hero = &run.players[&id];
    let near = run
        .enemies
        .iter()
        .filter(|e| e.hp > 0)
        .min_by(|a, b| (a.x - hero.x).hypot(a.y - hero.y).total_cmp(&(b.x - hero.x).hypot(b.y - hero.y)));
    let Some(e) = near else { return Input::default() };
    let (dx, dy) = (e.x - hero.x, e.y - hero.y);
    let d = dx.hypot(dy).max(1e-3);
    let step = |v: f32| if v > 0.38 { 1 } else if v < -0.38 { -1 } else { 0 };
    let (ax, ay) = (step(dx / d), step(dy / d));
    let mut input = Input { aim_x: ax, aim_y: ay, fire: true, ..Input::default() };
    if !run.shoulders.riding(id) {
        let (mx, my) = if d < 7.0 {
            (-dx / d, -dy / d)
        } else if d > 13.0 {
            (dx / d, dy / d)
        } else {
            (-dy / d, dx / d)
        };
        (input.move_x, input.move_y) = (step(mx), step(my));
    }
    input
}

fn tower_fight(seed: u64, secs: u32, stack: bool) -> (u32, u32, u32) {
    let mut run = Run::new(seed, 1, None);
    for id in 2..=6 {
        run.join_seat(id, &format!("Knight {id}"));
    }
    let room = (0..run.dungeon.rooms.len()).find(|&i| run.dungeon.rooms[i].kind == RoomKind::Fight).unwrap();
    run.enter_for_test(room);
    for hero in run.players.values_mut() {
        (hero.idle, hero.stone) = (false, false);
    }
    let mut seen: BTreeMap<u32, u32> = run.enemies.iter().map(|e| (e.id, e.hp)).collect();
    let mut dealt = 0;
    let mut throws = 0;
    let mut stood = 0;
    let mut key_up = true;
    for t in 0..secs * HZ {
        let mut inputs: BTreeMap<u32, Input> = run.players.keys().map(|&id| (id, bot(&run, id))).collect();
        if stack && key_up {
            for id in 2..=6 {
                let free = !run.shoulders.carried(id);
                let near = run.players.get(&id).zip(run.players.get(&1)).is_some_and(|(a, b)| (a.x - b.x).hypot(a.y - b.y) <= RIDE_REACH);
                if free && near && run.players[&id].hp > 0 {
                    inputs.get_mut(&id).unwrap().ride = true;
                }
                if free && !near {
                    let (a, b) = (&run.players[&id], &run.players[&1]);
                    let to = crate::rules::depths::open_path(&run.dungeon.rooms[run.at], (a.x, a.y), (b.x, b.y), false)
                        .and_then(|path| path.get(1).copied())
                        .map_or((b.x, b.y), |(c, r)| ((c as f32 + 0.5) * TILE_UNITS, (r as f32 + 0.5) * TILE_UNITS));
                    let (dx, dy) = (to.0 - a.x, to.1 - a.y);
                    let i = inputs.get_mut(&id).unwrap();
                    (i.move_x, i.move_y) = (dx.signum() as i8 * i8::from(dx.abs() > 0.5), dy.signum() as i8 * i8::from(dy.abs() > 0.5));
                }
            }
            if run.shoulders.riders(1).len() < MAX_RIDERS {
                let i = inputs.get_mut(&1).unwrap();
                (i.move_x, i.move_y) = (0, 0);
            }
            if t % (4 * HZ) == 2 * HZ && run.shoulders.riders(1).len() >= 2 {
                inputs.get_mut(&1).unwrap().ride = true;
                throws += 1;
            }
        }
        key_up = !inputs.values().any(|i| i.ride);
        run.step_sim_for_test(&inputs);
        let now: BTreeMap<u32, u32> = run.enemies.iter().map(|e| (e.id, e.hp)).collect();
        for (id, was) in &seen {
            dealt += was.saturating_sub(now.get(id).copied().unwrap_or(0));
        }
        seen = now;
        assert!(run.valid_snapshot(), "the run stays whole at tick {}", run.tick);
        assert!(run.shoulders.valid());
        for (carrier, riders) in &run.shoulders.stacks {
            for r in riders {
                assert_eq!(at(&run, *r), at(&run, *carrier), "rider {r} stays on {carrier}");
            }
        }
        stood += u32::from(run.shoulders.riders(1).len() == MAX_RIDERS);
        for hero in run.players.values_mut() {
            hero.hp = hero.hp.max(hero.max_hp / 2);
        }
        if run.enemies.iter().all(|e| e.hp == 0) {
            break;
        }
    }
    (dealt, throws, stood / HZ)
}

#[test]
fn six_knights_fight_as_a_tower_with_the_real_rules() {
    let (dealt, throws, stood) = tower_fight(11, 30, true);
    println!("tower sim: dealt {dealt}, throws {throws}, five-high for {stood} s");
    assert!(dealt > 0, "the tower fights");
    assert!(stood > 0, "the whole party stood in one tower");
    let (alone, _, _) = tower_fight(11, 30, false);
    println!("same room on foot: dealt {alone}");
}
