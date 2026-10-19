//! Gander Joust round two: one shared hall, the clack between two knights,
//! the egg, the tar's hand, the clock and podium, and offline plates.
use super::*;
use crate::rules::{
    home::Home,
    pastimes::{
        Game, GANDER_FLAP_KICK, GANDER_HATCH, GANDER_LEDGES as LEDGES,
        GANDER_NEED, GANDER_TAR_ROW as TAR_ROW,
    },
    world,
};

fn joust_hall(seats: u32) -> Run {
    let mut run = Run::at_home(53, 1, None, Home::default(), Spoils::default());
    for id in 2..=seats {
        run.join_seat(id, &format!("Seat {id}"));
    }
    run.arrive(world::room_of(&run.dungeon, RoomKind::GanderHall).unwrap(), (12.0, 11.0));
    run
}

fn put(run: &mut Run, id: u32, (x, y): (f32, f32)) {
    let h = run.players.get_mut(&id).unwrap();
    (h.x, h.y) = (x * TILE_UNITS, y * TILE_UNITS);
}

fn fire(id: u32) -> BTreeMap<u32, Input> {
    BTreeMap::from([(id, Input { fire: true, ..Default::default() })])
}

fn hall(run: &mut Run) -> &mut crate::rules::pastimes::Gander {
    run.pastimes.round.as_mut().unwrap().gander.as_mut().unwrap()
}

/// The verb: a flap kicks the goose up and gravity pulls it back; steering
/// walks it along a ledge. No formula, just the five numbers.
#[test]
fn a_flap_climbs_and_gravity_falls_back() {
    let mut run = joust_hall(1);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    let start_y = hall(&mut run).riders[0].y;
    run.step(&fire(1));
    assert_eq!(hall(&mut run).riders[0].dy, -GANDER_FLAP_KICK, "one flap kicks");
    let mut top = start_y;
    for _ in 0..30 {
        run.step(&BTreeMap::new());
        top = top.min(hall(&mut run).riders[0].y);
    }
    assert!(top < start_y - 100, "the flap climbed ({} < {})", top, start_y);
    assert_eq!(hall(&mut run).riders[0].dy, 0, "gravity settles the goose on stone");
    let settled = hall(&mut run).riders[0].clone();
    assert!(settled.resting_on().is_some() || settled.y >= TAR_ROW * 256);
    assert!(run.valid_snapshot());
}

/// The tar's hand: it rises for a low rider, takes them, and the hand
/// sinks; the rider is back on the middle ledge in well under a second.
#[test]
fn the_tar_hand_takes_a_low_rider() {
    let mut run = joust_hall(1);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    {
        let g = hall(&mut run);
        g.riders[0].y = (TAR_ROW + 1) * 256;
    }
    run.tick_pastimes(&BTreeMap::new());
    let g = hall(&mut run);
    assert!(g.hand.is_some(), "the hand rose (frame 1 of 4)");
    // The rise takes four frames, then the hand closes.
    for _ in 0..4 {
        run.tick_pastimes(&BTreeMap::new());
    }
    let g = hall(&mut run);
    assert!(g.riders[0].y < TAR_ROW * 256, "lifted out of the tar");
    assert!(run.valid_snapshot());
    // The hand sinks again once the rider is clear of it.
    for _ in 0..12 {
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(hall(&mut run).hand.is_none(), "the hand sank");
}

/// THE SHARED HALL: every knight in the heat rides the same hall, at the
/// same time — one hall on the round, not one copy per knight.
#[test]
fn one_hall_for_the_whole_heat() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2]);
    // Seat two's round is a marker: the hall itself lives once, on the
    // round, and both seats' views carry that same hall.
    assert!(run.pastimes.heats.contains_key(&2));
    let riders = hall(&mut run).riders.len();
    assert_eq!(riders, 5, "two knights plus the three rival waves");
    {
        let view = run.view_for_seat(2).unwrap();
        let v = view.pastimes.round.as_ref().unwrap().gander.as_ref().unwrap();
        let mine = run.pastimes.round.as_ref().unwrap().gander.as_ref().unwrap();
        assert_eq!(v.seats, mine.seats, "seat two sees the shared hall");
    }
    assert!(run.valid_snapshot());
}

/// The clack between two knights: a lance coming down on a friend unhorses
/// that friend and scores. This is the point of Joust.
#[test]
fn a_lance_coming_down_on_a_friend_unhorses_that_friend() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    // Park seat one above seat two — the rivals are parked far away so the
    // pass is only between the two knights.
    {
        let g = hall(&mut run);
        g.riders[0].x = 8 * 256;
        g.riders[0].y = LEDGES[0].2 * 256;
        g.riders[0].dy = 0;
        g.riders[0].stun = 0;
        g.riders[0].clash = 0;
        g.riders[1].x = 8 * 256 + 60;
        g.riders[1].y = LEDGES[0].2 * 256 + 140;
        g.riders[1].dy = 0;
        g.riders[1].stun = 0;
        g.riders[1].clash = 0;
        for i in 2..g.riders.len() {
            g.riders[i].x = (22 - i as i32) * 256;
            g.riders[i].y = 2 * 256;
            g.riders[i].down = 30 * crate::rules::HZ;
        }
    }
    run.tick_pastimes(&BTreeMap::new());
    let g = hall(&mut run);
    assert!(
        g.tally[0] >= 1,
        "seat one scored the clack (and the egg it dropped for them)"
    );
    assert_eq!(g.tally[1], 0, "seat two did not");
    assert!(g.riders[1].stun > 0, "seat two was unhorsed");
    assert!(g.eggs.len() <= 3);
    assert!(run.valid_snapshot());
}

/// The egg: an unhorsed rider drops one that rolls on the ledge; touch it
/// in time for a bonus point, leave it and it hatches into a rival.
#[test]
fn an_egg_left_on_the_ledge_hatches_into_a_rival() {
    let mut run = joust_hall(1);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    // Drop an egg on the west ledge, far from every rider.
    {
        let g = hall(&mut run);
        g.eggs.clear();
        g.eggs.push(crate::rules::pastimes::GanderEgg {
            x: 5 * 256,
            row: LEDGES[0].2,
            dir: 1,
            left: GANDER_HATCH,
        });
        g.riders[0].x = 20 * 256;
        g.riders[0].y = LEDGES[2].2 * 256;
        for i in 1..g.riders.len() {
            g.riders[i].down = 30 * crate::rules::HZ;
        }
    }
    // It rolls, untouched, until its time is up: then it hatches — a rival
    // rider is up and riding where the egg lay.
    let mut hatched = false;
    for _ in 0..(GANDER_HATCH + 30) {
        run.tick_pastimes(&BTreeMap::new());
        let g = hall(&mut run);
        if g.eggs.is_empty() && g.riders[1 + 0].down == 0 && g.riders[1].lane == LEDGES[0].2 * 256 {
            // The first rival slot is up again, riding the egg's ledge.
            hatched = true;
            break;
        }
    }
    assert!(hatched, "the egg hatched into a rival rider");
    assert!(run.valid_snapshot());
}

/// The egg bonus: touching it in time is worth a clack.
#[test]
fn touching_an_egg_in_time_pays_a_bonus() {
    let mut run = joust_hall(1);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    {
        let g = hall(&mut run);
        g.eggs.clear();
        g.eggs.push(crate::rules::pastimes::GanderEgg {
            x: 6 * 256,
            row: LEDGES[0].2,
            dir: 1,
            left: GANDER_HATCH,
        });
        g.riders[0].x = 6 * 256 + 40;
        g.riders[0].y = LEDGES[0].2 * 256;
        g.riders[0].stun = 0;
        g.riders[0].dy = 0;
        for i in 1..g.riders.len() {
            g.riders[i].down = 30 * crate::rules::HZ;
        }
    }
    run.tick_pastimes(&BTreeMap::new());
    let g = hall(&mut run);
    assert_eq!(g.tally[0], 1, "the egg was a bonus clack");
    assert!(g.eggs.is_empty(), "the egg was taken");
    assert!(run.valid_snapshot());
}

/// The heat still crowns the most unhorsings in 75 seconds, and pays the
/// winner; the lone knight has the rival waves.
#[test]
fn the_clock_crowns_the_most_unhorsings() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    // Seat one racks up three clacks on friends and rivals alike; the
    // rivals are parked under the lance, both knights in play. A clack
    // pays one, and the egg it drops can pay one more — so count >= 3.
    // The unhorsed need their stun and respawn to clear between passes, so
    // park the next victim under the lance until three clacks have landed.
    for _ in 0..60 {
        if hall(&mut run).tally[0] >= 3 {
            break;
        }
        {
            let g = hall(&mut run);
            g.riders[0].x = 8 * 256;
            g.riders[0].y = LEDGES[0].2 * 256;
            g.riders[0].dy = 0;
            g.riders[0].stun = 0;
            g.riders[0].clash = 0;
            let v = if g.riders[1].stun == 0 && g.riders[1].clash == 0 { 1 } else { 2 };
            g.riders[v].x = 8 * 256 + 60;
            g.riders[v].y = LEDGES[0].2 * 256 + 140;
            g.riders[v].dy = 0;
            g.riders[v].stun = 0;
            g.riders[v].clash = 0;
            // Clear any egg the last clack dropped, so each pass pays one.
            g.eggs.clear();
        }
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(hall(&mut run).tally[0] >= 3, "three clacks (plus any eggs)");
    // Let the clock run out: the podium crowns seat one.
    for _ in 0..(Game::Gander.time() + 10) {
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(run.pastimes.round.is_none() && run.pastimes.heat.is_none(), "the heat ended");
    let hero1 = run.players.get(&1).unwrap();
    assert!(hero1.laurel, "seat one wears the laurel");
    let hero2 = run.players.get(&2).unwrap();
    assert_eq!(hero2.booby, Some(Game::Gander), "seat two takes the booby");
    assert!(run.valid_snapshot());
}

/// The five numbers hold — with the hatch time replacing the respawn wait
/// as number four (the round-two trade the brief asked for).
#[test]
fn the_five_numbers_hold() {
    assert_eq!(GANDER_FLAP_KICK, 52);
    assert_eq!(crate::rules::pastimes::GANDER_GRAVITY, 14);
    assert_eq!(GANDER_NEED, 8);
    assert_eq!(GANDER_HATCH, 5 * HZ);
    assert_eq!(Game::Gander.time(), 75 * HZ);
}

/// Offline plates: the start, two knights mid-heat with a clack, an egg,
/// the tar's hand, and the podium.
/// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_gander_shots -- --ignored`
#[test]
#[ignore]
fn write_gander_shots() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |run: &Run, seat: u32, name: &str| {
        let view = run.view_for_seat(seat);
        let img = crate::render::overworld::arena::frame_for(
            view.as_ref().unwrap_or(run),
            768,
            448,
            Some(seat),
        );
        image::RgbaImage::from_raw(768, 448, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let mut run = joust_hall(2);
    // Start: the start mark in the joust hall, board up.
    save(&run, 1, "gander2-01-start.png");
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    // Mid-heat: two knights in the shared hall, one clacking the other.
    for _ in 0..30 {
        run.step(&fire(1));
        run.step(&fire(2));
    }
    {
        let g = hall(&mut run);
        g.riders[0].x = 8 * 256;
        g.riders[0].y = LEDGES[0].2 * 256;
        g.riders[0].dy = 0;
        g.riders[0].stun = 0;
        g.riders[0].clash = 0;
        g.riders[1].x = 8 * 256 + 60;
        g.riders[1].y = LEDGES[0].2 * 256 + 140;
        g.riders[1].dy = 0;
        g.riders[1].stun = 0;
        g.riders[1].clash = 0;
    }
    run.step(&BTreeMap::new());
    save(&run, 1, "gander2-02-clack.png");
    save(&run, 2, "gander2-02-clack-seat2.png");
    // An egg on the ledge and the tar's hand mid-rise.
    {
        let g = hall(&mut run);
        g.eggs.clear();
        g.eggs.push(crate::rules::pastimes::GanderEgg {
            x: 6 * 256,
            row: LEDGES[0].2,
            dir: 1,
            left: GANDER_HATCH,
        });
        g.riders[0].x = 18 * 256;
        g.riders[0].y = TAR_ROW * 256 - 10;
        g.riders[0].dy = 0;
    }
    // Let the hand rise to its spread-finger frame before the capture.
    for _ in 0..2 {
        run.step(&BTreeMap::new());
    }
    save(&run, 1, "gander2-03-egg-hand.png");
    // Podium: the clock runs out, the crown and the booby prize.
    for _ in 0..(Game::Gander.time() + 10) {
        run.step(&BTreeMap::new());
    }
    save(&run, 1, "gander2-04-podium.png");
}
