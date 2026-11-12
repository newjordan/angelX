//! A demo race, measured: six pods, the CPU pilots, a whole course, and what the field did.
//! `ANGEL_RACER_REPORT=/file cargo test -p angel-delve --lib race_report -- --nocapture`
//! writes the table; without the variable it only prints it. `ANGEL_RACER_CLEAN=1` puts a
//! driver with no slips in seat 1 (a good human) against five CPU rivals.
use super::course::*;
use super::race::*;
use std::collections::BTreeMap;

/// What one seat did in a race.
#[derive(Default, Clone)]
struct Seat { passes: u32, passed: u32, laps: Vec<f32>, lifted: u32, drafted: u32, boosted: u32, walls: u32, prev_impact: u32, still: u32, still_max: u32, side_slams: u32, prev_slams: u32 }

/// One course's race, measured.
pub struct Report { pub text: String, pub spread: f32, pub gap_pct: f32, pub gap_s: f32, pub passes_per_lap: f32, pub lead_changes: u32, pub blowouts: u32, pub slams: u32, pub winner: u32, pub clean_won_by: Option<f32> }

/// `rot` turns the pods round the seats: seat n drives pod n-1+rot.
pub fn report(id: CourseId, clean_seat: Option<u32>, rot: u8) -> Report {
    let mut s = Session::blank_on(id);
    s.join_ids(1..=6);
    for (n, c) in s.cars.iter_mut() { c.cpu = true; c.pod = ((n - 1) as u8 + rot) % 6; }
    let course = s.course();
    let lap = course.lap();
    let mut seats: BTreeMap<u32, Seat> = (1..=6).map(|n| (n, Seat::default())).collect();
    // A pass counts when the new order has held for two seconds: two pods running side by side
    // swap places every few ticks and that is not an overtake.
    let mut settled: BTreeMap<(u32, u32), bool> = BTreeMap::new();
    let mut pending: BTreeMap<(u32, u32), (bool, u32)> = BTreeMap::new();
    let (mut lead_changes, mut leader, mut leader_pending) = (0u32, s.order()[0], (s.order()[0], 0u32));
    let (mut spread_sum, mut spread_n, mut passes_by_lap) = (0f32, 0u32, vec![0u32; course.laps() as usize]);
    let mut lap_marks: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
    for t in 0..60_000u32 {
        let mut drives = s.bot_drives();
        if let Some(n) = clean_seat { if s.cars[&n].finish.is_none() { s.cars.get_mut(&n).unwrap().cpu = false; drives.insert(n, s.drive_clean(n)); } }
        s.step_drive(&drives);
        if s.finished { break; }
        if s.ticks <= COUNTDOWN { continue; }
        let clock = (s.ticks - COUNTDOWN) as f32 / HZ;
        // Passes: every pair whose running order changed and stayed changed.
        let order = s.order();
        for (i, &a) in order.iter().enumerate() {
            for &b in &order[i + 1..] {
                let (lo, hi) = (a.min(b), a.max(b));
                let lo_ahead = a == lo;
                let was = *settled.entry((lo, hi)).or_insert(lo_ahead);
                if lo_ahead == was { pending.remove(&(lo, hi)); continue; }
                let p = pending.entry((lo, hi)).or_insert((lo_ahead, s.ticks));
                if p.0 != lo_ahead { *p = (lo_ahead, s.ticks); }
                if s.ticks - p.1 >= 60 {
                    settled.insert((lo, hi), lo_ahead);
                    pending.remove(&(lo, hi));
                    seats.get_mut(&a).unwrap().passes += 1;
                    seats.get_mut(&b).unwrap().passed += 1;
                    let l = ((s.cars[&a].d.max(0.) / lap) as usize).min(passes_by_lap.len() - 1);
                    passes_by_lap[l] += 1;
                }
            }
        }
        if order[0] != leader_pending.0 { leader_pending = (order[0], s.ticks); }
        if order[0] != leader && s.ticks - leader_pending.1 >= 60 { lead_changes += 1; leader = order[0]; }
        for (n, c) in &s.cars {
            let marks = lap_marks.entry(*n).or_default();
            if c.finish.is_none() && (c.d / lap) as usize > marks.len() { marks.push(clock); }
            let st = seats.get_mut(n).unwrap();
            if c.finish.is_none() {
                if let Some(d) = drives.get(n) { if c.v > 20. && !d.gas && !d.brake && !c.blown() { st.lifted += 1; } }
                if c.draft > 0.3 { st.drafted += 1; }
                if c.boosting { st.boosted += 1; }
                if c.impact == 12 && st.prev_impact < 12 { st.walls += 1; }
            }
            st.prev_impact = c.impact;
            if c.v < 1. && c.finish.is_none() && !c.blown() { st.still += 1; st.still_max = st.still_max.max(st.still); } else { st.still = 0; }
            if c.slams > st.prev_slams && c.slam_side != 0 { st.side_slams += c.slams - st.prev_slams; }
            st.prev_slams = c.slams;
        }
        if t % 30 == 0 {
            let ds: Vec<f32> = s.cars.values().filter(|c| c.finish.is_none()).map(|c| c.d).collect();
            if ds.len() > 1 { spread_sum += ds.iter().cloned().fold(f32::MIN, f32::max) - ds.iter().cloned().fold(f32::MAX, f32::min); spread_n += 1; }
        }
    }
    assert!(s.finished, "{id:?} did not finish");
    for (n, marks) in &lap_marks {
        let mut prev = 0.;
        let seat = seats.get_mut(n).unwrap();
        for &m in marks { seat.laps.push(m - prev); prev = m; }
        seat.laps.push(s.cars[n].finish.unwrap() - prev);
    }
    let order = s.order();
    let win = s.cars[&order[0]].finish.unwrap();
    let mut out = format!("== {} ({} laps, {:.0} m a lap) ==\n", course.name(), course.laps(), lap);
    out += " place seat pod          finish    gap   best lap  passes made  passed  blowouts slams(side) walls  lift% draft% boost% longest stall\n";
    let race_ticks = (win * HZ).max(1.);
    for (i, n) in order.iter().enumerate() {
        let (c, st) = (&s.cars[n], &seats[n]);
        let best = st.laps.iter().cloned().fold(f32::MAX, f32::min);
        let pct = |x: u32| x as f32 / race_ticks * 100.;
        out += &format!("  {:>3}   {:>3}  {:<11} {:>7.2} {:>6.2} {:>9.2} {:>11} {:>7} {:>9} {:>3}({:>3}) {:>5} {:>6.1} {:>6.1} {:>6.1} {:>6.1}s{}\n", i + 1, n, c.spec().name, c.finish.unwrap(), c.finish.unwrap() - win, best, st.passes, st.passed, c.blowouts, c.slams, st.side_slams, st.walls, pct(st.lifted), pct(st.drafted), pct(st.boosted), st.still_max as f32 / HZ,
            if clean_seat == Some(*n) { "  <- clean driver" } else { "" });
    }
    let total: u32 = seats.values().map(|s| s.passes).sum();
    let per_lap: Vec<String> = passes_by_lap.iter().map(|n| n.to_string()).collect();
    let finish: Vec<f32> = order.iter().map(|n| s.cars[n].finish.unwrap()).collect();
    let closest = finish.windows(2).map(|w| w[1] - w[0]).fold(f32::MAX, f32::min);
    let spread = spread_sum / spread_n.max(1) as f32;
    let blowouts: u32 = s.cars.values().map(|c| c.blowouts).sum();
    let slams: u32 = s.cars.values().map(|c| c.slams).sum::<u32>() / 2;
    out += &format!("  passes {total} in the race ({:.1} a lap: {}), {lead_changes} lead changes, field spread {:.0} m on average, winner to last {:.1} s ({:.1}%), closest two finishers {:.2} s apart, {blowouts} blowouts, {slams} slams\n",
        total as f32 / course.laps() as f32, per_lap.join("+"), spread, finish[5] - finish[0], (finish[5] / finish[0] - 1.) * 100., closest);
    let clean_won_by = clean_seat.map(|n| { let t = s.cars[&n].finish.unwrap(); finish.iter().copied().filter(|f| *f != t).fold(f32::MAX, f32::min) - t });
    Report { text: out, spread, gap_pct: (finish[5] / finish[0] - 1.) * 100., gap_s: finish[5] - finish[0], passes_per_lap: total as f32 / course.laps() as f32, lead_changes, blowouts, slams, winner: order[0], clean_won_by }
}

/// Six pods round the seats, every cup course: what the field did on average.
struct Summary { spread: f32, pct: f32, ppl: f32, leads: f32, blow: f32, slam: f32, margin: f32, wins: u32, races: u32, text: String }

fn summary(clean: Option<u32>, rotations: u8) -> Summary {
    let mut all = String::new();
    let (mut spread, mut pct, mut ppl, mut leads, mut blow, mut slam, mut won, mut wins, mut n) = (0., 0., 0., 0, 0, 0, 0., 0, 0.);
    for rot in 0..rotations {
        for &id in cup() {
            let r = report(id, clean, rot);
            if rot == 0 { all += &r.text; all += "\n"; }
            spread += r.spread; pct += r.gap_pct; ppl += r.passes_per_lap; leads += r.lead_changes; blow += r.blowouts; slam += r.slams;
            won += r.clean_won_by.unwrap_or(0.); if r.clean_won_by.is_some_and(|m| m > 0.) { wins += 1; }
            n += 1.;
        }
    }
    Summary { spread: spread / n, pct: pct / n, ppl: ppl / n, leads: leads as f32 / n, blow: blow as f32 / n, slam: slam as f32 / n, margin: won / n, wins, races: n as u32, text: all }
}

/// The CPU field is a race and not a procession: it stays bunched, trades places and the lead, burns
/// an engine now and then, and bumps without demolishing itself.
#[test]
fn the_cpu_field_races_it_is_not_a_procession() {
    let s = summary(None, 6);
    println!("{} races: spread {:.0} m, last-to-first {:.1}%, {:.1} passes a lap, {:.1} lead changes, {:.1} blowouts, {:.1} slams", s.races, s.spread, s.pct, s.ppl, s.leads, s.blow, s.slam);
    assert!(s.spread < 650., "the field is strung out: {:.0} m between first and last on average (the old field: 700-830 m)", s.spread);
    assert!(s.pct < 6.5, "last is {:.1}% behind first (the old field: 9-11%)", s.pct);
    assert!(s.ppl > 12., "only {:.1} passes a lap", s.ppl);
    assert!(s.leads > 1.5, "{:.1} lead changes a race", s.leads);
    assert!(s.blow >= 0.5 && s.blow < 4., "{:.1} blowouts a race", s.blow);
    assert!(s.slam >= 2. && s.slam < 30., "{:.1} slams a race", s.slam);
}

/// A good human run (a pilot with no slips in seat 1, the rivals' band measured from it) wins, and
/// wins clearly: the rubber band never brings it back.
#[test]
fn a_clean_run_wins_clearly_against_the_band() {
    let s = summary(Some(1), 6);
    println!("clean run: won {} of {} races by {:.1} s on average", s.wins, s.races, s.margin);
    assert!(s.wins * 10 >= s.races * 7, "the clean run won only {} of {}", s.wins, s.races);
    assert!(s.margin > 2.5, "and by only {:.1} s on average", s.margin);
}

#[test]
fn race_report() {
    let clean = std::env::var_os("ANGEL_RACER_CLEAN").map(|_| 1);
    let rotations: u8 = std::env::var("ANGEL_RACER_ROTATIONS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let s = summary(clean, rotations);
    let mut all = s.text;
    all += &format!("SUMMARY over {} races: spread {:.0} m  last-to-first {:.1}%  passes/lap {:.1}  lead changes {:.1}/race  blowouts {:.1}/race  slams {:.1}/race  clean won by {:.1} s on average, {} of {} races\n", s.races, s.spread, s.pct, s.ppl, s.leads, s.blow, s.slam, s.margin, s.wins, s.races);
    println!("{all}");
    if let Some(path) = std::env::var_os("ANGEL_RACER_REPORT") { std::fs::write(path, &all).unwrap(); }
}
