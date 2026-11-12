//! The screens a seat reads for itself: the pod select, the garage and the records. Each is a
//! plate or two over a pod on a turntable (or black), in the HUD's family.
use super::kit::*;
use super::*;
use crate::garage::parts;
use crate::rules::knight_racers::career::{self, course_key};
use crate::rules::knight_racers::flow::Local;

/// What a pod's numbers read as on a bar, 0 to 1: placed between the roster's least and most.
fn norm(v: f32, pick: impl Fn(&rules::PodSpec) -> f32) -> f32 {
    let all: Vec<f32> = rules::pods().iter().map(&pick).collect();
    let (lo, hi) = (all.iter().copied().fold(f32::MAX, f32::min), all.iter().copied().fold(f32::MIN, f32::max));
    (0.2 + 0.65 * (v - lo) / (hi - lo).max(1e-3)).clamp(0.05, 1.)
}

/// How a pod is opened.
fn pod_hint(pod: u8) -> &'static str {
    match pod { 2 => "WIN THE SQUIRE CUP", 1 => "WIN THE KNIGHT CUP", 3 => "WIN THE CHAMPION CUP", 4 => "WIN EVERY RACE OF A CUP", _ => "" }
}

/// The five bars of a pod, as the knight's parts make it, with what the parts add.
fn pod_bars(im: &mut Img, x: i32, y: i32, base: &rules::PodSpec, tuned: &rules::PodSpec) {
    let rows: [(&str, fn(&rules::PodSpec) -> f32, bool); 5] = [
        ("TOP", |p| p.top, true), ("BOOST", |p| p.boost, true), ("PUSH", |p| p.accel, true), ("GRIP", |p| p.grip, true), ("WEIGHT", |p| p.mass, true),
    ];
    for (i, (label, f, good)) in rows.iter().enumerate() {
        let b = norm(f(base), f);
        let t = norm(f(tuned), f);
        bar(im, x, y + i as i32 * 11, label, b, t - b, *good);
    }
}

/// The pod select: the six pods on a turntable, who may race which, and each seat's readiness.
pub fn pods(race: &rules::Session, seat: u32) -> Img {
    let Some(sf) = race.flow.seats.get(&seat) else { return Img::black(384, 224) };
    let look = sf.pick as usize % rules::pods().len();
    let mut im = super::super::garage::garage(look, seat, race.flow.now as f32 * 0.025 + 0.6, race.flow.now);
    plate(&mut im, 6, 8, 162, 200);
    im.text(12, 13, "CHOOSE YOUR POD", 'T');
    let sub = match race.flow.mode {
        rules::Mode::Cup => format!("{} CUP  {}", race.flow.class.name(), rules::course(rules::cup()[0]).name().split_whitespace().next().unwrap_or("")),
        rules::Mode::Practice => "PRACTICE  PROVING RUN".to_string(),
        m => format!("{}  {}", m.name(), rules::course(race.flow.course).name().split_whitespace().next().unwrap_or("")),
    };
    im.text(12, 23, &sub, 't');
    for (i, p) in rules::pods().iter().enumerate() {
        let y = 36 + i as i32 * 11;
        let open = sf.career.owns_pod(i as u8);
        row(&mut im, 9, y - 2, 156, 11, i == look);
        let ink = if !open { 'j' } else if i == look { 'H' } else { 'J' };
        im.text(16, y, p.name, ink);
        if !open { lock(&mut im, 150, y, 'j'); }
    }
    let base = rules::pods()[look];
    let tuned = base.tuned(&sf.career.tune());
    for (i, l) in wrap(pods::pod(look).blurb, 26).iter().take(2).enumerate() { im.text(12, 106 + i as i32 * 9, l, 'h'); }
    im.rect(12, 128, 150, 1, 'j');
    pod_bars(&mut im, 12, 134, &base, &tuned);
    im.rect(12, 192, 150, 1, 'j');
    if !sf.career.owns_pod(look as u8) { im.text(12, 197, &format!("LOCKED  {}", pod_hint(look as u8)), 'O'); }
    else if sf.ready { im.text(12, 197, "READY  WAITING FOR THE REST", 'y'); }
    else { im.text(12, 197, if race.flow.now / 20 % 2 == 0 { "ENTER  TO RACE" } else { "" }, 'H'); }
    // Who is ready: one chip a seat, in its colour.
    for (n, (&id, other)) in race.flow.seats.iter().filter(|(_, s)| s.here).enumerate() {
        let x = 376 - (n as i32 + 1) * 14;
        let ink = pods::livery(id).ink;
        plate(&mut im, x, 8, 12, 12);
        im.rect(x + 2, 10, 8, 8, if other.ready { ink } else { 'g' });
    }
    if race.flow.seats.values().filter(|s| s.here).count() > 1 { im.text(300 - 5 * 6, 24, "READY", 't'); }
    note_at(&mut im, race, seat, 272);
    hints(&mut im, "W S A D CHOOSE   ENTER PICKS   Q BACK");
    im
}

/// The garage: parts to buy and fit, slot by slot, with what each does to the racer.
pub fn garage_shop(race: &rules::Session, seat: u32) -> Img {
    let Some(sf) = race.flow.seats.get(&seat) else { return Img::black(384, 224) };
    let look = if sf.career.owns_pod(sf.pick) { sf.pick as usize } else { 0 } % rules::pods().len();
    let mut im = super::super::garage::garage_at(look, seat, race.flow.now as f32 * 0.02 + 0.9, race.flow.now, 5.);
    let slot = sf.cursor as usize % career::WORKING_SLOTS;
    // The slots.
    plate(&mut im, 4, 8, 118, 200);
    shout(&mut im, 10, 12, "GARAGE", 2, 'T');
    purse(&mut im, 118, 30, sf.career.money, 'T');
    im.rect(8, 40, 110, 1, 'j');
    for s in 0..career::WORKING_SLOTS {
        let y = 46 + s as i32 * 26;
        row(&mut im, 7, y - 3, 112, 24, s == slot);
        im.text(13, y, career::SLOT_NAMES[s], if s == slot { 'T' } else { 't' });
        let worn = parts::part(sf.career.worn(s)).map_or("", |p| p.name);
        im.text(13, y + 10, &worn.to_uppercase(), if s == slot { 'H' } else { 'J' });
    }
    // The part under the cursor.
    let list = career::wares(slot);
    let at = sf.sub as usize % list.len().max(1);
    let ware = list[at];
    let part = parts::part(ware.id);
    plate(&mut im, 246, 8, 134, 200);
    im.text(252, 13, &format!("<  {}/{}  >", at + 1, list.len()), 'h');
    im.text(252, 24, &part.map_or(ware.id.to_uppercase(), |p| p.name.to_uppercase()), 'T');
    for (i, l) in wrap(part.map_or("", |p| p.blurb), 21).iter().take(4).enumerate() { im.text(252, 36 + i as i32 * 9, &l.to_uppercase(), 'h'); }
    let worn_now = sf.career.worn(slot) == ware.id;
    let owned = sf.career.owns(ware.id);
    let (line, ink) = if worn_now { ("FITTED".to_string(), 'y') }
        else if owned { ("OWNED  ENTER FITS IT".to_string(), 'H') }
        else if sf.career.money >= ware.price { (format!("ENTER BUYS  {}", gold(ware.price)), 'T') }
        else { (format!("{}  YOU HAVE {}", gold(ware.price), gold(sf.career.money)), 'O') };
    im.text(252, 76, &line, ink);
    im.rect(252, 88, 122, 1, 'j');
    // The racer's numbers, and what this part would do to them.
    let mut with = sf.career.build.clone();
    with[slot] = ware.id.to_string();
    let (now, then) = (career::stats_of(&sf.career.build), career::stats_of(&with));
    let labels = ["SPEED", "ACCEL", "HANDLE", "ARMOUR", "WEIGHT"];
    for i in 0..5 {
        let d = if worn_now { 0. } else { (then[i] - now[i]) as f32 / 100. };
        bar_at(&mut im, 252, 252 + 40, 96 + i as i32 * 12, labels[i], now[i] as f32 / 100., d, i != 4);
    }
    // What those numbers do to a pod, in the race's own terms.
    let tune = career::tune_of(&sf.career.build);
    let after = career::tune_of(&with);
    im.rect(252, 160, 122, 1, 'j');
    let pct = |t: f32| ((t - 1.) * 100.).round() as i32;
    im.text(252, 165, "ON THE ROAD NOW", 't');
    im.text(252, 175, &format!("TOP {:+}%  PUSH {:+}%", pct(tune.top), pct(tune.accel)), 'h');
    im.text(252, 185, &format!("GRIP {:+}%  HITS {:+}%", pct(tune.grip), pct(tune.mass)), 'h');
    if !worn_now {
        let d = |a: f32, b: f32| ((a - b) * 100.).round() as i32;
        im.text(252, 196, &format!("THIS {:+} {:+} {:+} {:+}", d(after.top, tune.top), d(after.accel, tune.accel), d(after.grip, tune.grip), d(after.mass, tune.mass)), 'y');
    }
    note_at(&mut im, race, seat, 184);
    hints(&mut im, "W S SLOT   A D PART   ENTER BUY OR FIT   Q BACK");
    im
}

/// The records: each course's bests, the time trials by pod, and the career's tallies.
pub fn records(race: &rules::Session, seat: u32) -> Img {
    let mut im = Img::black(384, 224);
    let Some(sf) = race.flow.seats.get(&seat) else { return im };
    let c = &sf.career;
    shout(&mut im, 12, 8, "RECORDS", 2, 'T');
    purse(&mut im, 376, 12, c.money, 'T');
    let ids = rules::courses();
    let tab = sf.tab as usize % ids.len();
    for (i, &id) in ids.iter().enumerate() {
        let x = 8 + i as i32 * 92;
        plate(&mut im, x, 28, 88, 14);
        if i == tab { im.rect(x + 1, 29, 86, 12, 'g'); im.rect(x + 1, 29, 86, 1, 'T'); }
        let n = rules::course(id).name();
        let n = n.strip_prefix("THE ").unwrap_or(n);
        let short = n.split_whitespace().next().unwrap_or(n);
        centred(&mut im, x + 44, 32, short, 1, if i == tab { 'H' } else { 'h' });
    }
    let id = ids[tab];
    let rec = c.courses.get(&course_key(id)).copied().unwrap_or_default();
    plate(&mut im, 8, 48, 250, 160);
    im.text(14, 53, rules::course(id).name(), 'T');
    im.text(14, 66, "BEST RACE", 't');
    im.text(90, 66, &time_ms(rec.race), if rec.race > 0 { 'H' } else { 'j' });
    im.text(14, 77, "BEST LAP", 't');
    im.text(90, 77, &time_ms(rec.lap), if rec.lap > 0 { 'H' } else { 'j' });
    if rec.lap > 0 { im.text(150, 77, rules::pods()[rec.lap_pod as usize % rules::pods().len()].name, 'h'); }
    im.text(14, 88, "RUN", 't');
    im.text(90, 88, &format!("{} TIMES  {} WON", rec.runs, rec.wins), 'h');
    im.rect(14, 100, 238, 1, 'j');
    im.text(14, 106, "TIME TRIAL, BEST LAP BY POD", 't');
    for (p, spec) in rules::pods().iter().enumerate() {
        let y = 120 + p as i32 * 13;
        let open = c.owns_pod(p as u8);
        let best = c.trials.get(&career::lap_key(id, p as u8)).copied().unwrap_or(0);
        im.text(20, y, spec.name, if open { 'J' } else { 'j' });
        if !open { lock(&mut im, 100, y - 1, 'j'); }
        else { im.text(120, y, &time_ms(best), if best > 0 { 'H' } else { 'j' }); }
        if best > 0 {
            // Next to each time, a bar of how close it is to the best of them.
            let top = rules::pods().iter().enumerate().filter_map(|(q, _)| c.trials.get(&career::lap_key(id, q as u8))).copied().min().unwrap_or(best);
            let k = (top as f32 / best as f32).clamp(0., 1.);
            im.rect(190, y, 60, 6, 'g');
            im.rect(190, y, (60. * k) as i32, 6, 'T');
        }
    }
    plate(&mut im, 266, 48, 110, 160);
    im.text(272, 53, "CAREER", 'T');
    let lines = [("RACES", c.races.to_string()), ("WINS", c.wins.to_string()), ("PODIUMS", c.podiums.to_string()), ("SWEEPS", c.sweeps.to_string())];
    for (i, (k, v)) in lines.iter().enumerate() {
        im.text(272, 66 + i as i32 * 11, k, 't');
        im.text(370 - text_w(v, 1), 66 + i as i32 * 11, v, 'H');
    }
    im.rect(272, 112, 98, 1, 'j');
    im.text(272, 118, "EARNED", 't');
    let e = gold(c.earned.min(u64::from(u32::MAX)) as u32);
    im.text(370 - text_w(&e, 1), 118, &e, 'T');
    im.text(272, 134, "CUPS WON", 't');
    for (i, name) in ["SQUIRE", "KNIGHT", "CHAMPION"].iter().enumerate() {
        let y = 146 + i as i32 * 12;
        trophy(&mut im, 272, y - 1, if c.cups[i] > 0 { 'T' } else { 'g' });
        im.text(286, y, name, if c.cups[i] > 0 { 'h' } else { 'j' });
        im.text(370 - text_w(&c.cups[i].to_string(), 1), y, &c.cups[i].to_string(), 'H');
    }
    hints(&mut im, "A D COURSE   Q BACK");
    let _ = Local::Records;
    im
}
