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
    for _ in 0..4 {
        run.tick_pastimes(&BTreeMap::new());
    }
    let g = hall(&mut run);
    assert!(g.riders[0].y < TAR_ROW * 256, "lifted out of the tar");
    assert!(run.valid_snapshot());
    for _ in 0..12 {
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(hall(&mut run).hand.is_none(), "the hand sank");
}

#[test]
fn one_hall_for_the_whole_heat() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
    let heat = run.pastimes.heat.clone().expect("a heat begins");
    assert_eq!(heat.seats, vec![1, 2]);
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

#[test]
fn a_lance_coming_down_on_a_friend_unhorses_that_friend() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
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

#[test]
fn an_egg_left_on_the_ledge_hatches_into_a_rival() {
    let mut run = joust_hall(1);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
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
    let mut hatched = false;
    for _ in 0..(GANDER_HATCH + 30) {
        run.tick_pastimes(&BTreeMap::new());
        let g = hall(&mut run);
        if g.eggs.is_empty() && g.riders[1 + 0].down == 0 && g.riders[1].lane == LEDGES[0].2 * 256 {
            hatched = true;
            break;
        }
    }
    assert!(hatched, "the egg hatched into a rival rider");
    assert!(run.valid_snapshot());
}

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

#[test]
fn the_clock_crowns_the_most_unhorsings() {
    let mut run = joust_hall(2);
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
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
            g.eggs.clear();
        }
        run.tick_pastimes(&BTreeMap::new());
    }
    assert!(hall(&mut run).tally[0] >= 3, "three clacks (plus any eggs)");
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

#[test]
fn the_five_numbers_hold() {
    assert_eq!(GANDER_FLAP_KICK, 52);
    assert_eq!(crate::rules::pastimes::GANDER_GRAVITY, 14);
    assert_eq!(GANDER_NEED, 8);
    assert_eq!(GANDER_HATCH, 5 * HZ);
    assert_eq!(Game::Gander.time(), 75 * HZ);
}

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
    save(&run, 1, "gander2-01-start.png");
    put(&mut run, 1, Game::Gander.start());
    run.step(&BTreeMap::new());
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
    for _ in 0..2 {
        run.step(&BTreeMap::new());
    }
    save(&run, 1, "gander2-03-egg-hand.png");
    for _ in 0..(Game::Gander.time() + 10) {
        run.step(&BTreeMap::new());
    }
    save(&run, 1, "gander2-04-podium.png");
}
