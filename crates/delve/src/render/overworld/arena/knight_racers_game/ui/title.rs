//! The menus that sit over a CPU race: the title, the mode list, the classes, the courses.
use super::kit::*;
use super::*;
use crate::rules::knight_racers::career::{self, course_key};
use crate::rules::knight_racers::flow::{self, MENU};

pub fn draw(im: &mut Img, race: &rules::Session, seat: u32) -> bool {
    match race.flow.stage {
        rules::Stage::Title => title(im, race, seat),
        rules::Stage::Modes => modes(im, race, seat),
        rules::Stage::Classes => classes(im, race, seat),
        rules::Stage::Courses => courses(im, race, seat),
        _ => return false,
    }
    true
}

fn title(im: &mut Img, race: &rules::Session, seat: u32) {
    let t = race.flow.now;
    veil(im, 0, 12, 384, 118);
    im.rect(0, 12, 384, 1, 'o');
    im.rect(0, 130, 384, 1, 'o');
    im.rect(0, 14, 384, 1, 'g');
    im.rect(0, 128, 384, 1, 'g');
    centred(im, 192, 21, "KNIGHT", 5, 'T');
    centred(im, 192, 61, "RACERS", 5, 'T');
    // A pennon either side of the line under the name.
    for (x0, dir) in [(40, 1), (344, -1)] {
        for i in 0..6 { im.rect(x0 + dir * i * 4 - if dir < 0 { 3 } else { 0 }, 109 + i / 2, 4, 3 - i / 3, 'R'); }
    }
    centred(im, 192, 108, "THE CUP OF THE ROUND TABLE", 1, 'h');
    if t / 18 % 2 == 0 { centred(im, 192, 148, "PRESS ENTER", 2, 'H'); }
    centred(im, 192, 168, "E  SPACE  ENTER", 1, 't');
    // The foot: the race behind, and a record to beat.
    plate(im, 0, 186, 384, 29);
    let on = race.course();
    im.text(8, 190, "NOW SHOWING", 't');
    im.text(8, 200, on.name(), 'H');
    if let Some(sf) = race.flow.seats.get(&seat) {
        purse(im, 376, 190, sf.career.money, 'T');
        let ids = rules::cup();
        let id = ids[(t as usize / 90) % ids.len()];
        let best = sf.career.courses.get(&course_key(id)).map_or(0, |r| r.lap);
        let line = format!("{}  BEST LAP {}", rules::course(id).name().split_whitespace().next().unwrap_or(""), time_ms(best));
        im.text(376 - text_w(&line, 1), 200, &line, 'h');
    }
    hints(im, "ANY KEY STARTS   HOLD ESC LEAVES");
}

/// What the highlighted mode is, for the plate beside the list.
fn detail(race: &rules::Session, seat: u32, cur: usize) -> Vec<(String, char)> {
    let mut v: Vec<(String, char)> = Vec::new();
    let line = |s: &str, ink: char| (s.to_string(), ink);
    match cur {
        0..=3 => {
            let mode = rules::Mode::ALL[cur];
            for l in wrap(mode.blurb(), 24) { v.push((l, 'h')); }
            v.push(line("", 'h'));
            match mode {
                rules::Mode::Cup => {
                    v.push(line("3 COURSES, 2 LAPS EACH", 'H'));
                    v.push((format!("1ST PAYS {}", gold(career::purse(mode, rules::Class::Squire, 1))), 'T'));
                    v.push((format!("CUP PRIZE {}", gold(career::cup_purse(rules::Class::Squire, 1))), 'T'));
                    v.push(line("WIN IT TO OPEN POD", 't'));
                    v.push(line("AND CLASS", 't'));
                }
                rules::Mode::Single => {
                    v.push(line("ANY COURSE", 'H'));
                    v.push((format!("1ST PAYS {}", gold(career::purse(mode, rules::Class::Squire, 1))), 'T'));
                }
                rules::Mode::TimeTrial => {
                    v.push(line("YOU, ALONE", 'H'));
                    v.push((format!("A FIRST LAP PAYS {}", career::TRIAL_FIRST), 'T'));
                    v.push((format!("A NEW BEST {}", career::TRIAL_BEST), 'T'));
                }
                rules::Mode::Practice => v.push(line("NO PRIZE, NO RECORD OF PLACE", 't')),
            }
        }
        4 => {
            for l in wrap("BUY PARTS AND FIT THEM. THEY TUNE EVERY POD YOU RACE.", 24) { v.push((l, 'h')); }
        }
        _ => {
            for l in wrap("YOUR BEST LAPS, YOUR CUPS AND WHAT YOU HAVE WON.", 24) { v.push((l, 'h')); }
            if let Some(sf) = race.flow.seats.get(&seat) {
                v.push(line("", 'h'));
                v.push((format!("RACES {}  WINS {}", sf.career.races, sf.career.wins), 'H'));
                v.push((format!("CUPS {}", sf.career.cups.iter().sum::<u32>()), 'H'));
            }
        }
    }
    v
}

fn modes(im: &mut Img, race: &rules::Session, seat: u32) {
    let Some(sf) = race.flow.seats.get(&seat) else { return };
    let cur = sf.cursor as usize % MENU.len();
    // The list.
    plate(im, 10, 20, 176, 172);
    shout(im, 20, 26, "SELECT", 2, 'T');
    purse(im, 178, 30, sf.career.money, 'T');
    im.rect(16, 44, 164, 1, 'j');
    for (i, name) in MENU.iter().enumerate() {
        let y = 50 + i as i32 * 23;
        row(im, 14, y - 3, 168, 21, i == cur);
        let host = race.flow.host() == Some(seat);
        let shut = (i < 4 && !host) || (i == 2 && race.flow.seats.values().filter(|s| s.here).count() > 1);
        let ink = if i == cur { 'H' } else if shut { 'j' } else { 'J' };
        shout(im, 24, y, name, 2, ink);
    }
    // The plate beside it: what the highlighted entry is.
    plate(im, 194, 20, 180, 172);
    shout(im, 202, 26, MENU[cur], 2, 'T');
    im.rect(200, 44, 168, 1, 'j');
    for (i, (l, ink)) in detail(race, seat, cur).iter().enumerate() {
        im.text(202, 52 + i as i32 * 10, l, *ink);
    }
    let host = race.flow.host();
    if host != Some(seat) && cur < 4 {
        let who = host.map(|h| super::who(race, h)).unwrap_or_default();
        im.text(202, 176, &format!("{who} PICKS THE RACE"), 'O');
    } else if cur < 4 && race.flow.seats.values().filter(|s| s.here).count() > 1 {
        im.text(202, 176, "YOU PICK FOR THE PARTY", 'y');
    }
    note(im, race, seat);
    hints(im, "W S CHOOSE   ENTER OR E PICK   Q BACK");
}

fn classes(im: &mut Img, race: &rules::Session, seat: u32) {
    let Some(host) = race.flow.host() else { return };
    let (Some(h), Some(me)) = (race.flow.seats.get(&host), race.flow.seats.get(&seat)) else { return };
    let cur = h.cursor as usize % 3;
    plate(im, 24, 20, 336, 170);
    shout(im, 34, 26, "CHOOSE YOUR CLASS", 2, 'T');
    im.text(34, 44, &format!("{}   THE RIVALS RACE TO IT", race.flow.mode.name()), 't');
    purse(im, 350, 28, me.career.money, 'T');
    for (i, class) in rules::Class::ALL.iter().enumerate() {
        let y = 58 + i as i32 * 42;
        let open = h.career.class_open(*class);
        row(im, 30, y - 3, 324, 38, i == cur);
        let ink = if !open { 'j' } else if i == cur { 'H' } else { 'J' };
        shout(im, 42, y, class.name(), 2, ink);
        if open {
            for l in wrap(class.blurb(), 44).iter().take(2).enumerate() { im.text(42, y + 17 + l.0 as i32 * 9, l.1, if i == cur { 'h' } else { 'G' }); }
            // Cups won in this class, as little gold cups; and what it pays.
            for n in 0..h.career.cups[i].min(6) { trophy(im, 330 - n as i32 * 11, y + 1, 'T'); }
            let pay = format!("1ST {}", gold(career::purse(rules::Mode::Cup, *class, 1)));
            im.text(348 - text_w(&pay, 1), y + 22, &pay, if i == cur { 'T' } else { 't' });
        } else {
            lock(im, 42 + text_w(class.name(), 2) + 8, y + 3, 'j');
            im.text(42, y + 20, &format!("LOCKED  {}", class.hint()), 'O');
        }
    }
    if race.flow.host() != Some(seat) { im.text(34, 178, &format!("{} CHOOSES", super::who(race, host)), 'O'); }
    note(im, race, host);
    hints(im, "W S CHOOSE   ENTER OR E PICK   Q BACK");
}

/// A course's road from above, fitted to a box, the forks dimmer, the line a pale stripe.
pub fn route(im: &mut Img, id: rules::CourseId, (x, y, w, h): (i32, i32, i32, i32)) {
    let c = rules::course(id);
    let main: Vec<(f32, f32)> = c.line().samples().iter().map(|p| (p.x, p.y)).collect();
    let forks: Vec<Vec<(f32, f32)>> = c.forks().iter().map(|f| f.line.samples().iter().map(|p| (p.x, p.y)).collect()).collect();
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for &(px, py) in main.iter().chain(forks.iter().flatten()) { lo = (lo.0.min(px), lo.1.min(py)); hi = (hi.0.max(px), hi.1.max(py)); }
    let (sx, sy) = ((hi.0 - lo.0).max(1.), (hi.1 - lo.1).max(1.));
    let k = ((w - 12) as f32 / sx).min((h - 12) as f32 / sy);
    let (ox, oy) = (x as f32 + 6. + ((w - 12) as f32 - sx * k) * 0.5, y as f32 + 6. + ((h - 12) as f32 - sy * k) * 0.5);
    let at = |p: (f32, f32)| ((ox + (p.0 - lo.0) * k) as i32, (oy + (hi.1 - p.1) * k) as i32);
    let stride = (main.len() / 220).max(1);
    for (pts, a, b) in forks.iter().map(|f| (f, 'j', 'g')).chain(std::iter::once((&main, 'T', 't'))) {
        let mut prev = None;
        for (i, &p) in pts.iter().enumerate() {
            if i % stride != 0 && i + 1 != pts.len() { continue; }
            let p = at(p);
            if let Some((qx, qy)) = prev { im.line(qx, qy, p.0, p.1, a); im.line(qx + 1, qy, p.0 + 1, p.1, b); }
            prev = Some(p);
        }
    }
    let s = at(main[0]);
    im.rect(s.0 - 2, s.1 - 2, 5, 3, 'H');
}

fn courses(im: &mut Img, race: &rules::Session, seat: u32) {
    let Some(host) = race.flow.host() else { return };
    let (Some(h), Some(me)) = (race.flow.seats.get(&host), race.flow.seats.get(&seat)) else { return };
    let list = flow::choices();
    let cur = h.cursor as usize % list.len();
    plate(im, 8, 8, 156, 124);
    shout(im, 14, 13, "COURSE", 2, 'T');
    for (i, &id) in list.iter().enumerate() {
        let c = rules::course(id);
        let y = 33 + i as i32 * 24;
        row(im, 12, y - 2, 148, 22, i == cur);
        let name = c.name();
        let short = name.strip_suffix(" RUN").unwrap_or(name);
        let short = short.strip_prefix("LANTERNWARD ").unwrap_or(short);
        im.text(18, y, short, if i == cur { 'H' } else { 'J' });
        let cup_n = rules::cup().iter().position(|&k| k == id).map_or("PRACTICE".to_string(), |r| format!("CUP {}", r + 1));
        im.text(18, y + 10, &format!("{} LAPS  {:.1} KM  {}", c.laps(), c.lap() / 1000., cup_n), 't');
    }
    // The road from above, in a corner.
    let id = list[cur];
    plate(im, 296, 8, 80, 72);
    route(im, id, (296, 8, 80, 72));
    // Words and records along the foot.
    plate(im, 8, 140, 368, 70);
    let c = rules::course(id);
    im.text(14, 145, c.name(), 'T');
    let tag = format!("{}{}", race.flow.mode.name(), if race.flow.mode == rules::Mode::Single { format!("  {}", race.flow.class.name()) } else { String::new() });
    im.text(370 - text_w(&tag, 1), 145, &tag, 't');
    for (i, l) in wrap(c.blurb(), 58).iter().take(2).enumerate() { im.text(14, 156 + i as i32 * 10, l, 'h'); }
    let rec = me.career.courses.get(&course_key(id)).copied().unwrap_or_default();
    im.rect(14, 180, 354, 1, 'j');
    im.text(14, 186, "BEST LAP", 't');
    im.text(78, 186, &time_ms(rec.lap), if rec.lap > 0 { 'H' } else { 'j' });
    if rec.lap > 0 { im.text(132, 186, rules::pods()[rec.lap_pod as usize % rules::pods().len()].name, 'h'); }
    im.text(14, 197, "BEST RACE", 't');
    im.text(78, 197, &time_ms(rec.race), if rec.race > 0 { 'H' } else { 'j' });
    im.text(230, 186, &format!("RUN {} TIMES", rec.runs), 't');
    im.text(230, 197, &format!("WON {}", rec.wins), 't');
    if race.flow.host() != Some(seat) { im.text(150, 13, &format!("{} CHOOSES", super::who(race, host)), 'O'); }
    note(im, race, host);
    hints(im, "W S CHOOSE   ENTER OR E PICK   Q BACK");
}
