//! The race as an event: the course card, the lights, the lap banners, the
//! final-lap cue, the finish and the results board. Each is a pure function
//! of the tick and the cars, so a late-joining screen shows the same thing.
use super::*;
use super::hud::{centred, plate, shout, text_w, Ctx};
use super::pods::livery;
use super::state::{self, GRID_TICKS};

pub fn draw(im: &mut Img, c: &Ctx) {
    let race = c.race;
    if race.finished {
        results(im, c);
        return;
    }
    // The course card is the clock plate's own (it holds the laps and the lap's length on the grid).
    if race.ticks < GRID_TICKS {
        lights(im, race.ticks);
    } else if race.ticks < GRID_TICKS + 30 {
        go(im, race.ticks - GRID_TICKS);
    }
    if c.car.finish.is_some() {
        finish(im, c);
        return;
    }
    banner(im, c);
    if c.car.impact >= 12 {
        im.text(162, 150, "WALL STRIKE", 'O');
        im.rect(0, 52, 2, 120, 'O'); im.rect(382, 52, 2, 120, 'O');
    } else if c.car.impact > 0 && c.car.slam == 0 && !c.car.blown() {
        im.text(166, 150, "SLAG HIT", 'O');
        im.rect(0, 52, 2, 120, 'O'); im.rect(382, 52, 2, 120, 'O');
    }
}

/// The light tree: a post of three lamps at the left of the grid, one more lit each second, and the
/// count beside it in big figures. In the last half second before the flag (the sim's rocket-start
/// window) the last lamp burns white and the figure goes with it: that is when to stand on the gas.
pub(super) fn lights(im: &mut Img, ticks: u32) {
    let lit = (1 + ticks / 30).min(3);
    let window = ticks + rules::race::LAUNCH_WINDOW >= GRID_TICKS;
    im.rect(29, 74, 30, 62, 'k');
    im.frame(29, 74, 30, 62, 'g');
    for n in 0..3u32 {
        let y = 79 + n as i32 * 18;
        let on = n < lit;
        im.rect(33, y - 1, 22, 16, 'K');
        im.rect(34, y, 20, 14, if on { 'O' } else { 'g' });
        if on {
            im.rect(36, y + 2, 16, 10, 'T');
            im.rect(40, y + 4, 8, 6, if window && n == 2 { 'W' } else { 'H' });
        } else {
            im.rect(36, y + 2, 16, 10, 'X');
        }
    }
    let n = (GRID_TICKS - ticks).div_ceil(30);
    let into = ticks % 30;
    let scale = if into < 4 { 7 } else { 6 };
    centred(im, 80, 92 - if scale == 7 { 3 } else { 0 }, &n.to_string(), scale, if window { 'W' } else { 'H' });
    centred(im, 62, 146, "READY", 1, 'T');
    centred(im, 62, 156, "ON THE GRID", 1, 'T');
}

/// The tree goes green: three lamps flare across the top of the road and the flag is down.
fn go(im: &mut Img, since: u32) {
    let scale = if since < 6 { 6 } else { 5 };
    for n in 0..3 {
        let x = 192 + (n - 1) * 20;
        im.rect(x - 6, 58, 13, 13, 'k');
        im.rect(x - 5, 59, 11, 11, 'C');
        if since < 10 { im.rect(x - 3, 61, 7, 7, 'W'); }
    }
    centred(im, 192, 78, "GO!", scale, 'y');
}

/// A new lap, the final lap, and the lap just run.
fn banner(im: &mut Img, c: &Ctx) {
    let (lap_n, into) = state::lap_of(c.race, c.car);
    let count = state::laps(c.race);
    // The wrong-way plate has that part of the screen.
    if lap_n == 0 || c.car.finish.is_some() || state::wrong_way(c.clocks, c.race, c.seat) { return; }
    let last = lap_n + 1 >= count;
    let show = if last { 100. } else { 56. };
    if into > show { return; }
    let flash = c.race.ticks / 5 % 2 == 0;
    let y = 58;
    plate(im, 96, y, 192, 42);
    if last {
        centred(im, 192, y + 4, "FINAL LAP", 3, if flash { 'O' } else { 'T' });
    } else {
        centred(im, 192, y + 4, &format!("LAP {}/{}", lap_n + 1, count), 3, 'T');
    }
    if let Some(s) = c.clocks.seats.get(&c.seat) {
        if let Some(l) = s.last() {
            let mut line = format!("LAST {}", state::stamp(l));
            if let Some(b) = s.best() {
                if s.times.len() > 1 {
                    let delta = l - b;
                    line.push_str(&format!("  {}{:.2}", if delta <= 0.005 { "BEST " } else { "+" }, delta.abs()));
                }
            }
            centred(im, 192, y + 30, &line, 1, 'h');
        }
    }
}

fn finish(im: &mut Img, c: &Ctx) {
    let place = state::place_of(c.order, c.seat);
    let time = c.car.finish.unwrap_or(0.);
    plate(im, 82, 52, 220, 62);
    centred(im, 192, 57, "FINISH", 3, 'T');
    centred(im, 192, 80, &format!("{}{}", place, state::ordinal(place)), 3, if place == 1 { 'T' } else { 'H' });
    centred(im, 192, 104, &format!("{}   WAITING FOR THE FIELD", state::stamp(time)), 1, 'h');
}

/// The board: every seat, in order, with pod, time, the gap to the winner and
/// the best lap the screen saw; beside it the cup, race by race. The sim
/// stops its clock at the flag, so nothing here moves with the tick.
fn results(im: &mut Img, c: &Ctx) {
    let (x, y, w, h) = (6, 4, 256, 208);
    plate(im, x, y, w, h);
    shout(im, x + 10, y + 7, "RESULTS", 2, 'T');
    im.text(x + 10, y + 25, &format!("{}  {} LAPS", state::course_name(c.race), state::laps(c.race)), 't');
    let mine = c.race.flow.seats.get(&c.seat).and_then(|sf| sf.last.as_ref());
    if let Some(last) = mine {
        let flag = if last.trial { Some("NEW TRIAL RECORD") } else if last.notes.new_lap { Some("NEW BEST LAP") } else if last.notes.new_race { Some("NEW BEST RACE") } else { None };
        if let Some(f) = flag { if c.race.ticks / 6 % 3 != 0 { im.text(x + w - 10 - text_w(f, 1), y + 11, f, 'y'); } }
    }
    footer(im, c.race);
    let cols = [x + 8, x + 18, x + 28, x + 80, x + 126, x + 176];
    for (i, label) in ["", "", "KNIGHT", "TIME", "GAP", "BEST LAP"].iter().enumerate() {
        im.text(cols[i], y + 40, label, 't');
    }
    im.rect(x + 6, y + 50, w - 12, 1, 'j');
    let win = c.order.first().and_then(|(_, car)| car.finish).unwrap_or(0.);
    for (n, &(id, car)) in c.order.iter().enumerate() {
        let ry = y + 56 + n as i32 * 24;
        let me = id == c.seat;
        if me { im.rect(x + 4, ry - 3, w - 8, 22, 'g'); }
        let ink = if me { 'H' } else if n == 0 { 'T' } else { 'J' };
        im.text(cols[0], ry + 2, &(n + 1).to_string(), ink);
        im.rect(cols[1], ry, 6, 8, livery(id).ink);
        im.text(cols[2], ry, livery(id).name, ink);
        im.text(cols[2], ry + 10, &pods::name_of(car), 't');
        let time = car.finish.unwrap_or(0.);
        im.text(cols[3], ry, &state::stamp(time), ink);
        im.text(cols[4], ry, &if n == 0 { "WINNER".to_string() } else { format!("+{:.2}", time - win) }, if n == 0 { 'T' } else { 't' });
        let best = c.clocks.seats.get(&id).and_then(|s| s.best());
        im.text(cols[5], ry, &best.map_or("--".into(), state::stamp), if best.is_some() { ink } else { 't' });
    }
    if c.race.flow.mode == rules::Mode::Cup { cup(im, c, 268, 4, 110, 208); } else { prize(im, c, 268, 4, 110, 208); }
}

/// The keys at the foot of the results board: the cup goes on or to its podium; any other race
/// is run again or left.
fn footer(im: &mut Img, race: &rules::Session) {
    if race.flow.stage != rules::Stage::Race || !race.flow.settled { return; }
    im.rect(0, 215, 384, 9, 'k');
    let line = match race.flow.mode {
        rules::Mode::Cup if (race.round as usize) + 1 < rules::cup().len() => "E NEXT RACE                      HOLD ESC LEAVE",
        rules::Mode::Cup => "E THE PODIUM                     HOLD ESC LEAVE",
        _ => "E RACE AGAIN   Q MENU              HOLD ESC LEAVE",
    };
    im.text(7, 216, line, 't');
}

/// What a single race, a trial or a practice run paid, in place of a cup's points.
fn prize(im: &mut Img, c: &Ctx, x: i32, y: i32, w: i32, h: i32) {
    plate(im, x, y, w, h);
    let mode = c.race.flow.mode;
    shout(im, x + 8, y + 7, match mode { rules::Mode::TimeTrial => "TRIAL", rules::Mode::Practice => "PRACTICE", _ => "PURSE" }, 2, 'T');
    im.rect(x + 6, y + 26, w - 12, 1, 'j');
    let Some(sf) = c.race.flow.seats.get(&c.seat) else { return };
    let last = sf.last.clone().unwrap_or_default();
    let place = state::place_of(c.order, c.seat);
    let mut ry = y + 34;
    let mut line = |im: &mut Img, k: &str, v: &str, ink: char| {
        im.text(x + 8, ry, k, 't');
        im.text(x + w - 8 - text_w(v, 1), ry, v, ink);
        ry += 11;
    };
    line(im, "PLACE", &format!("{}{}", place, state::ordinal(place)), if place == 1 { 'T' } else { 'H' });
    if mode == rules::Mode::TimeTrial {
        if let Some(l) = c.clocks.seats.get(&c.seat) {
            for (n, t) in l.times.iter().enumerate() { line(im, &format!("LAP {}", n + 1), &state::stamp(*t), 'H'); }
            if let Some(b) = l.best() { line(im, "BEST", &state::stamp(b), 'T'); }
        }
        line(im, "RECORD", &sf.career.trials.get(&crate::rules::knight_racers::career::lap_key(c.race.course, c.car.pod)).map_or("--".into(), |&m| state::stamp(m as f32 / 1000.)), if last.trial { 'y' } else { 'h' });
        line(im, "PAID", &format!("+{}", last.purse), if last.purse > 0 { 'T' } else { 't' });
    } else {
        line(im, "PRIZE", &format!("+{}", last.purse), if last.purse > 0 { 'T' } else { 't' });
    }
    line(im, "GOLD", &format!("{}", sf.career.money), 'H');
    ry += 6;
    if mode == rules::Mode::Single { im.text(x + 8, ry, "THE CLASS HAS", 't'); im.text(x + 8, ry + 10, &format!("{} RIVALS", c.race.flow.class.name()), 'h'); }
}

/// Cup points so far, best first, with what this race added.
fn cup(im: &mut Img, c: &Ctx, x: i32, y: i32, w: i32, h: i32) {
    plate(im, x, y, w, h);
    shout(im, x + 8, y + 7, "CUP", 2, 'T');
    let cup = rules::cup();
    let total = cup.len().max(1);
    let tag = format!("{}/{}", (c.race.round as usize + 1).min(total), total);
    im.text(x + w - 8 - text_w(&tag, 1), y + 11, &tag, 't');
    im.rect(x + 6, y + 26, w - 12, 1, 'j');
    for (n, &(id, pts)) in c.race.standings().iter().enumerate() {
        let ry = y + 32 + n as i32 * 20;
        let me = id == c.seat;
        if me { im.rect(x + 4, ry - 3, w - 8, 19, 'g'); }
        let ink = if me { 'H' } else if n == 0 { 'T' } else { 'J' };
        im.text(x + 8, ry + 1, &(n + 1).to_string(), ink);
        im.rect(x + 18, ry, 6, 8, livery(id).ink);
        im.text(x + 28, ry, livery(id).name, ink);
        im.text(x + 28, ry + 9, &format!("{pts} PTS"), 't');
        let place = state::place_of(c.order, id);
        let won = rules::race::POINTS[(place - 1).min(rules::race::POINTS.len() - 1)];
        let tag = format!("+{won}");
        im.text(x + w - 8 - text_w(&tag, 1), ry + 9, &tag, if me { 'T' } else { 't' });
    }
    // What comes next, in words that fit the panel.
    let champion = c.race.standings().first().map_or("", |&(id, _)| livery(id).name);
    let next = cup.get(c.race.round as usize + 1).map(|&id| rules::course(id).name());
    let mut lines: Vec<String> = Vec::new();
    if let Some(last) = c.race.flow.seats.get(&c.seat).and_then(|sf| sf.last.as_ref()).filter(|_| c.race.flow.settled) {
        lines.push(format!("PURSE +{}", last.purse));
    }
    if c.race.cup_over() && c.race.flow.settled {
        lines.push("E  THE PODIUM".into());
        lines.push(format!("LEADS {champion}"));
    } else if c.race.cup_over() {
        lines.push("CUP WON BY".into());
        lines.push(champion.to_string());
    } else {
        lines.push(if next.is_some() { "E  NEXT RACE:" } else { "E  RACE AGAIN" }.into());
        if let Some(name) = next {
            let mut line = String::new();
            for word in name.split_whitespace() {
                if !line.is_empty() && line.len() + 1 + word.len() > 17 { lines.push(std::mem::take(&mut line)); }
                if !line.is_empty() { line.push(' '); }
                line.push_str(word);
            }
            lines.push(line);
        }
    }
    let at = y + 160;
    for (i, l) in lines.iter().take(5).enumerate() {
        im.text(x + 8, at + i as i32 * 10, l, if l.starts_with("PURSE") { 'T' } else if l.starts_with("E ") { 'H' } else { 'h' });
    }
}

#[cfg(test)]
#[path = "flow_proof.rs"]
mod flow_proof;
