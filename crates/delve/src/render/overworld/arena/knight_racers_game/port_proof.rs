//! Proof reels for what the old single circuit had and the cup's courses now carry: Moonward and
//! Sunfall by section, the leap, the pit lane, the gold pennant, a scorch, a paint trade, the splits and
//! the standings, the crowned board and the crane through the countdown. Staged on the real courses and
//! driven by the real pilots; each reel is a folder of 384x224 frames, cut into GIFs by the proof script.
//!
//!   ANGEL_PORT_PROOF=/dir cargo test -p angel-delve --profile fast --lib port_reels -- --nocapture
//!   (ANGEL_PORT_REEL=moonward|sunfall|leap|pitlane|pennant|scorch|paint|splits|standings|results|crane for one)
use super::*;
use super::law_proof::{hold_lane, Drives, Reel};
use std::path::{Path, PathBuf};

fn out_dir() -> Option<PathBuf> { std::env::var_os("ANGEL_PORT_PROOF").map(PathBuf::from) }
fn wanted(name: &str) -> bool { std::env::var("ANGEL_PORT_REEL").map_or(true, |w| w == name) }

#[test]
fn port_reels() {
    let Some(out) = out_dir() else { return };
    if wanted("crane") { crane(&out.join("crane")); }
    if wanted("moonward") { sections(&out.join("moonward"), rules::CourseId::Moonward, &[]); }
    if wanted("sunfall") { sections(&out.join("sunfall"), rules::CourseId::Sunfall, &["THE CHUTE", "THE FORGE", "MERIDIAN"]); }
    if wanted("leap") { leap(&out.join("leap")); }
    if wanted("pitlane") { pitlane(&out.join("pitlane")); }
    if wanted("pennant") { pennant(&out.join("pennant")); }
    if wanted("scorch") { scorch(&out.join("scorch")); }
    if wanted("paint") { paint(&out.join("paint")); }
    if wanted("splits") { splits(&out.join("splits")); }
    if wanted("standings") { standings(&out.join("standings")); }
    if wanted("results") { results(&out.join("results")); }
}

fn clean(reel: &Reel, seat: u32) -> Drives {
    let mut d = reel.pilots(&[seat]);
    d.insert(seat, reel.race.drive_clean(seat));
    d
}

fn drive_through(reel: &mut Reel, from: f32, until: f32, dir: &Path, every: u32, speed: f32) {
    reel.stage(&[(1, from, 0., speed), (2, from - 26., 4., speed), (3, from + 38., -4., speed), (4, from + 70., 3., speed), (5, from - 55., -4., speed), (6, from - 80., 4., speed)]);
    let mut t = 0;
    while reel.race.cars[&1].d < until && t < 1800 {
        if t % every == 0 { reel.shot(dir); }
        let d = clean(reel, 1);
        reel.step(&d);
        t += 1;
    }
}

fn sections(dir: &Path, id: rules::CourseId, only: &[&str]) {
    let course = rules::course(id);
    for z in course.zones() {
        if !only.is_empty() && !only.contains(&z.name) { continue; }
        let mut reel = Reel::on(id, 1);
        let name = z.name.to_lowercase().replace(' ', "-").replace('\'', "");
        let (from, until) = ((z.s0 - 70.).max(0.), (z.s1 + 10.).min(z.s0 + 260.));
        println!("{id:?} {} from {from:.0} to {until:.0}", z.name);
        drive_through(&mut reel, from, until, &dir.join(name), 3, 40.);
    }
}

fn staged(id: rules::CourseId, seat: u32, field: &[(u32, f32, f32, f32)]) -> Reel {
    let mut reel = Reel::on(id, seat);
    let ahead = field.iter().map(|f| f.1).fold(f32::MIN, f32::max);
    let mut all: Vec<(u32, f32, f32, f32)> = field.to_vec();
    for n in 1..=6u32 {
        if !field.iter().any(|f| f.0 == n) { all.push((n, ahead + 230. + 36. * n as f32, if n % 2 == 0 { 3. } else { -3. }, 44.)); }
    }
    reel.stage(&all);
    reel.race.ticks = rules::race::COUNTDOWN + 90;
    reel
}

fn leap(dir: &Path) {
    for id in [rules::CourseId::Sunfall, rules::CourseId::Moonward] {
        let crest = rules::course(id).crests()[0];
        for (seat, sub) in [(2, "from-behind"), (1, "own")] {
            let mut reel = staged(id, seat, &[(1, crest - 120., 0., 45.), (2, crest - 146., 2., 45.)]);
            let (mut high, mut air) = (0f32, 0u32);
            for t in 0..230 {
                if t % 2 == 0 { reel.shot(&dir.join(format!("{id:?}").to_lowercase()).join(sub)); }
                let mut d = reel.pilots(&[1, 2]);
                for who in [1, 2] { let mut x = reel.race.drive_clean(who); x.gas = true; d.insert(who, x); }
                reel.step(&d);
                let c = &reel.race.cars[&1];
                high = high.max(c.lift);
                if c.flying { air += 1; }
            }
            println!("{id:?} ({sub}): the leap rose {high:.1} m and held {:.2} s", air as f32 / 30.);
        }
    }
}

fn pitlane(dir: &Path) {
    for (id, name) in [(rules::CourseId::Sunfall, "sunfall"), (rules::CourseId::Moonward, "moonward"), (rules::CourseId::Emberfall, "emberfall"), (rules::CourseId::Moatwalk, "moatwalk"), (rules::CourseId::Glassway, "glassway")] {
        let course = rules::course(id);
        let Some(p) = course.pieces().iter().find(|p| p.kind == rules::PieceKind::PitLane) else { println!("{id:?} has no pit lane"); continue };
        println!("{id:?} pit lane {:.0}..{:.0} side {}", p.s0, p.s1, p.side);
        let from = p.s0 - 60.;
        let mut reel = staged(id, 1, &[(1, from, p.side * 3.5, 30.), (2, from - 30., -p.side * 2., 32.), (3, from + 34., -p.side * 5., 31.), (4, from - 70., p.side * 6., 33.)]);
        let mut t = 0;
        while reel.race.cars[&1].d < p.s0 + 140. && t < 900 {
            if t % 2 == 0 { reel.shot(&dir.join(name)); }
            let mut d = reel.pilots(&[1]);
            let c = &reel.race.cars[&1];
            d.insert(1, rules::Drive { gas: c.v < 30., steer: hold_lane(c, p.side * 3.5), ..Default::default() });
            reel.step(&d);
            t += 1;
        }
    }
}

fn pennant(dir: &Path) {
    let id = rules::CourseId::Sunfall;
    let field = [(3, 300., 0., 44.), (2, 272., 3.5, 44.), (1, 244., -3.5, 44.), (4, 216., 3., 44.), (5, 190., -3., 44.), (6, 164., 3., 44.)];
    for (seat, sub, n) in [(2, "from-second", 130), (3, "own", 90)] {
        let mut reel = staged(id, seat, &field);
        for t in 0..n {
            if t % 2 == 0 { reel.shot(&dir.join(sub)); }
            let d = clean(&reel, seat);
            reel.step(&d);
        }
    }
}

fn scorch(dir: &Path) {
    let id = rules::CourseId::Sunfall;
    for (seat, sub) in [(1, "own"), (2, "passing")] {
        let mut reel = staged(id, seat, &[(1, 235., 0., 44.), (2, 185., 4.6, 28.)]);
        reel.race.cars.get_mut(&1).unwrap().heat = [0.3, 0.997];
        for t in 0..300 {
            if t % 3 == 0 { reel.shot(&dir.join(sub)); }
            let mut d = reel.pilots(&[1, seat]);
            d.insert(1, rules::Drive { gas: true, boost: t < 25, ..Default::default() });
            if seat != 1 { let c = &reel.race.cars[&seat]; d.insert(seat, rules::Drive { gas: c.v < 27., steer: hold_lane(c, 4.6), ..Default::default() }); }
            reel.step(&d);
        }
    }
}

fn paint(dir: &Path) {
    let id = rules::CourseId::Sunfall;
    let mut reel = staged(id, 1, &[(1, 150., -1.4, 43.), (2, 150.3, 1.8, 43.), (3, 215., -4., 44.), (4, 245., 3., 44.), (5, 100., 4., 44.), (6, 80., -4., 44.)]);
    for t in 0..130 {
        reel.shot(dir);
        let mut d = reel.pilots(&[1, 2]);
        d.insert(1, rules::Drive { gas: true, steer: hold_lane(&reel.race.cars[&1], if t < 50 { -1.4 } else { -3.2 }), ..Default::default() });
        d.insert(2, rules::Drive { gas: true, steer: hold_lane(&reel.race.cars[&2], if t < 50 { 1.8 } else { 4. }), ..Default::default() });
        reel.step(&d);
    }
}

fn splits(dir: &Path) {
    let id = rules::CourseId::Sunfall;
    let lap = rules::course(id).lap();
    let mut reel = staged(id, 1, &[(1, lap - 140., 0., 44.), (2, lap - 100., 4., 42.), (3, lap - 60., -4., 44.), (4, lap - 25., 3., 44.), (5, lap - 190., -4., 44.), (6, lap - 230., 4., 44.)]);
    reel.race.ticks += 1500;
    for t in 0..260 {
        if t % 2 == 0 { reel.shot(dir); }
        let mut d = reel.pilots(&[1]);
        d.insert(1, rules::Drive { gas: true, boost: t < 70, ..Default::default() });
        reel.step(&d);
    }
}

fn standings(dir: &Path) {
    let id = rules::CourseId::Sunfall;
    let mut reel = staged(id, 1, &[(2, 140., 0., 40.), (1, 112., -4., 44.), (3, 70., 4., 44.), (4, 20., 0., 44.), (5, 10., -4., 44.), (6, 0., 4., 44.)]);
    for t in 0..230 {
        if t % 2 == 0 { reel.shot(dir); }
        let mut d = reel.pilots(&[1, 2, 3]);
        let clear = reel.race.cars[&1].d > reel.race.cars[&2].d + 16.;
        d.insert(1, rules::Drive { gas: true, boost: t < 90, steer: hold_lane(&reel.race.cars[&1], if clear { 0. } else { -4. }), ..Default::default() });
        d.insert(2, rules::Drive { gas: true, ..Default::default() });
        d.insert(3, rules::Drive { gas: true, boost: t > 70, steer: hold_lane(&reel.race.cars[&3], 4.), ..Default::default() });
        reel.step(&d);
    }
}

fn results(dir: &Path) {
    for id in [rules::CourseId::Sunfall, rules::CourseId::Moonward] {
        let mut reel = Reel::on(id, 1);
        reel.race.skip_menus();
        let sub = dir.join(format!("{id:?}").to_lowercase());
        let mut n = 0;
        while !reel.race.finished && n < 40_000 {
            let d = reel.race.bot_drives_all();
            reel.step(&d);
            n += 1;
        }
        assert!(reel.race.finished, "{id:?} was not finished");
        for k in 0..40 {
            if k % 2 == 0 { reel.shot(&sub); }
            reel.step(&Default::default());
        }
    }
}

fn crane(dir: &Path) {
    for id in [rules::CourseId::Sunfall, rules::CourseId::Moonward] {
        let mut reel = Reel::on(id, 1);
        let sub = dir.join(format!("{id:?}").to_lowercase());
        for _ in 0..120 {
            reel.shot(&sub);
            let d = reel.pilots(&[]);
            reel.step(&d);
        }
    }
}

#[cfg(test)]
mod says {
    use super::*;
    use crate::rules::Run;

    fn finished(id: rules::CourseId) -> rules::Session {
        let mut race = rules::Session::blank_on(id);
        race.join_ids(1..=6);
        let total = race.course().lap() * race.laps() as f32;
        for (i, c) in race.cars.values_mut().enumerate() { c.finish = Some(100. + i as f32); c.d = total; }
        race.finished = true;
        race
    }

    #[test]
    fn the_board_crowns_its_winner_by_name_and_the_course() {
        for id in rules::courses().iter().copied() {
            let race = finished(id);
            let order = state::standings(&race);
            let line = flow::crown_line(&race, &order).unwrap();
            assert_eq!(line, format!("KNIGHT AUREL TAKES {}", id.short()), "{id:?}");
            assert!(text_w_line(&line) < 256 - 24, "{line} must fit the board");
        }
        let mut race = finished(rules::CourseId::Sunfall);
        race.cars.get_mut(&4).unwrap().finish = Some(90.);
        let order = state::standings(&race);
        assert_eq!(flow::crown_line(&race, &order).unwrap(), "KNIGHT DUNSTAN TAKES SUNFALL");
        let mut run = Run::roundtable_for_test("knight_racers").unwrap();
        run.knight_racers = Some(race.clone());
        let board = layers(&run, Some(1)).0;
        let gold = crate::ink::ink('T').unwrap();
        let lit = (37..50).flat_map(|y| (12..250).map(move |x| (x, y))).filter(|&(x, y)| board.get(x, y) == Some(gold)).count();
        assert!(lit > 60, "the crown bar holds gold: {lit}");
    }

    fn text_w_line(s: &str) -> i32 { s.chars().count() as i32 * 6 + 24 }

    #[test]
    fn the_results_board_names_the_winners_best_lap_and_the_lap_it_was() {
        let mut race = finished(rules::CourseId::Proving);
        { let c = race.cars.get_mut(&1).unwrap(); c.marks = vec![70.]; c.finish = Some(120.); }
        let best = flow::best_lap_at(&race.cars[&1]).unwrap();
        assert_eq!(best.0, 2, "the second lap, 50 s against 70");
        assert!((best.1 - 50.).abs() < 1e-3);
        { let c = race.cars.get_mut(&1).unwrap(); c.marks = vec![40.]; }
        assert_eq!(flow::best_lap_at(&race.cars[&1]).unwrap().0, 1);
        let mut fresh = rules::Session::blank_on(rules::CourseId::Proving);
        fresh.join_ids([1]);
        assert!(flow::best_lap_at(&fresh.cars[&1]).is_none(), "a pod that has run no lap has no best");
    }

    #[test]
    fn a_split_names_the_gap_to_the_pod_first_across_the_line_or_the_lead_over_the_next() {
        let mut race = rules::Session::blank_on(rules::CourseId::Sunfall);
        race.join_ids(1..=3);
        race.ticks = state::GRID_TICKS + 400;
        for (id, t) in [(1u32, 62.40f32), (2, 61.50), (3, 63.10)] {
            let c = race.cars.get_mut(&id).unwrap();
            c.marks = vec![t]; c.split = 40; c.d = 3500.;
        }
        fn seat(race: &rules::Session, s: u32) -> Option<(String, String, char)> {
            let clocks = state::observe(race);
            let order = state::standings(race);
            let car = &race.cars[&s];
            let view = camera::view(race, car);
            let depth = vec![f32::INFINITY; 384 * 224];
            let ctx = hud::Ctx { race, car, seat: s, view: &view, depth: &depth, order: &order, clocks: &clocks };
            hud::split_text(&ctx)
        }
        let (head, line, _) = seat(&race, 1).unwrap();
        assert_eq!(head, "SPLIT 1  1:02.40");
        assert_eq!(line, "+0.90 ON BRANNOC", "seat 1 was 0.9 s behind seat 2, who was first across");
        let (_, line, _) = seat(&race, 2).unwrap();
        assert_eq!(line, "FIRST  0.90 CLEAR", "seat 2 was first across, 0.9 s clear of seat 1");
        let (_, line, _) = seat(&race, 3).unwrap();
        assert_eq!(line, "+1.60 ON BRANNOC");
        race.cars.get_mut(&1).unwrap().split = 0;
        assert!(seat(&race, 1).is_none());
        race.cars.get_mut(&2).unwrap().marks.clear();
        assert!(seat(&race, 2).is_none());
    }

    #[test]
    fn the_leader_alone_carries_the_gold_pennant() {
        let mut race = rules::Session::blank_on(rules::CourseId::Sunfall);
        race.join_ids(1..=6);
        race.ticks = state::GRID_TICKS + 300;
        for (i, c) in race.cars.values_mut().enumerate() { c.d = 800. - i as f32 * 30.; }
        assert!(pods::leads(&race, 1) && (2..=6).all(|s| !pods::leads(&race, s)));
        race.cars.get_mut(&4).unwrap().d = 900.;
        assert!(pods::leads(&race, 4) && !pods::leads(&race, 1));
        race.ticks = 10;
        assert!(!pods::leads(&race, 4));
        race.ticks = state::GRID_TICKS + 300;
        race.cars.get_mut(&4).unwrap().finish = Some(100.);
        assert!(!pods::leads(&race, 4));
        let (mut plain, mut flown) = (Mesh::new(), Mesh::new());
        let car = &race.cars[&2];
        pods::add_led(&mut plain, &race, 2, car, point(car.d - 12., car.y, 5.), false);
        pods::add_led(&mut flown, &race, 2, car, point(car.d - 12., car.y, 5.), true);
        assert!(flown.tris.len() > plain.tris.len() + 6, "{} against {}", flown.tris.len(), plain.tris.len());
    }

    #[test]
    fn a_blowout_scorch_is_drawn_where_the_pod_sat_and_the_mark_stays() {
        let mut race = rules::Session::blank_on(rules::CourseId::Emberfall);
        race.join_ids(1..=2);
        race.ticks = state::GRID_TICKS + 300;
        let c = race.cars.get_mut(&1).unwrap();
        c.d = 500.; c.y = 1.; c.scorch = Some((500., 1., 0));
        let eye = point(480., 0., 5.);
        let mut m = Mesh::new();
        pods::scorch(&mut m, &race, &race.cars[&1], eye);
        let sat = m.tris.len();
        assert!(sat > 20, "a blot of black rings: {sat}");
        let road = rules::course(rules::CourseId::Emberfall).height(500.);
        assert!(m.tris.iter().flat_map(|t| t.v).all(|p| p.z > road - 2. && p.z < road + 6.), "the scorch lies on the road");
        race.cars.get_mut(&1).unwrap().crash = 60;
        let mut blown = Mesh::new();
        pods::scorch(&mut blown, &race, &race.cars[&1], eye);
        assert!(blown.tris.len() >= sat, "coals while it sits");
        race.cars.get_mut(&1).unwrap().crash = 0;
        let mut far = Mesh::new();
        pods::scorch(&mut far, &race, &race.cars[&1], point(100., 0., 5.));
        assert!(far.tris.is_empty());
        let mut none = Mesh::new();
        pods::scorch(&mut none, &race, &race.cars[&2], eye);
        assert!(none.tris.is_empty(), "a pod that never blew leaves no mark");
    }

    #[test]
    fn the_crane_lands_on_the_chase_seat_as_the_lights_go_out_and_never_jumps() {
        for id in [rules::CourseId::Sunfall, rules::CourseId::Moonward, rules::CourseId::Emberfall] {
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            let seat = 1;
            let mut last: Option<View3> = None;
            for t in 0..=(state::GRID_TICKS + 24) {
                race.ticks = t;
                let car = &race.cars[&seat];
                let v = camera::shot(&race, car, seat).view;
                assert!(v.pos.x.is_finite() && v.pos.y.is_finite() && v.pos.z.is_finite() && v.heading_rad.is_finite() && v.pitch.is_finite(), "{id:?} tick {t}");
                if let Some(p) = last {
                    assert!((v.pos - p.pos).length() < 9., "{id:?}: the eye jumped {} m at tick {t}", (v.pos - p.pos).length());
                    let turn = (v.heading_rad - p.heading_rad + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                    assert!(turn.abs() < 0.6, "{id:?}: the heading jumped {turn} at tick {t}");
                }
                last = Some(v);
            }
            race.ticks = state::GRID_TICKS;
            let car = &race.cars[&seat];
            let (crane, chase) = (camera::shot(&race, car, seat).view, camera::chase(car, race.ticks));
            assert!((crane.pos - chase.pos).length() < 2.5, "{id:?}: lands {} m off the chase seat", (crane.pos - chase.pos).length());
            race.ticks = 0;
            let start = camera::shot(&race, &race.cars[&seat], seat).view;
            let lens = camera::Lens::new(&start);
            for c in race.cars.values() { assert!(lens.project(point(c.d, c.y, 1.)).is_some(), "{id:?}: seat {} is out of the overview", 0); }
            let front = race.cars.values().map(|c| c.d).fold(f32::MIN, f32::max);
            race.ticks = 62;
            let mid = camera::shot(&race, &race.cars[&seat], seat).view;
            let ahead = (mid.pos - point(front, 0., 0.)).length();
            assert!(ahead < 60., "{id:?}: the head-on shot is {ahead} m from the front row");
        }
    }
}
