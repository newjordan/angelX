//! The race instruments: place, clocks, speed, boost, a live minimap of the
//! real course, the rival ladder, a rear-view cue and the pod tags. All in the
//! realm's ink on black plates, kept to the edges so the road stays open.
use super::*;
use super::camera::{project, H, W};
use super::pods::livery;
use super::state::{self, Clocks};
use rules::race::engines::{FIRE_HEAT, WARN_HEAT};

/// Everything a frame of instruments reads.
pub struct Ctx<'a> {
    pub race: &'a rules::Session,
    pub car: &'a rules::Car,
    pub seat: u32,
    pub view: &'a View3,
    /// The raster's depth for each dot (infinite where only sky shows).
    pub depth: &'a [f32],
    pub order: &'a [(u32, &'a rules::Car)],
    pub clocks: &'a Clocks,
}

pub fn text_w(text: &str, scale: i32) -> i32 { (text.chars().count() as i32 * 6 - 1) * scale }

/// Scaled 5x7 text with a one dot shadow.
pub fn shout(im: &mut Img, x: i32, y: i32, text: &str, scale: i32, ink: char) -> i32 {
    for pass in 0..2 {
        let (dx, c) = if pass == 0 { (1, 'k') } else { (0, ink) };
        for (n, ch) in text.chars().enumerate() {
            for (ry, bits) in glyph(ch).into_iter().enumerate() {
                for bx in 0..5 {
                    if bits & (1 << (4 - bx)) != 0 {
                        im.rect(x + dx + (n as i32 * 6 + bx) * scale, y + dx + ry as i32 * scale, scale, scale, c);
                    }
                }
            }
        }
    }
    x + text_w(text, scale)
}

pub fn centred(im: &mut Img, cx: i32, y: i32, text: &str, scale: i32, ink: char) {
    shout(im, cx - text_w(text, scale) / 2, y, text, scale, ink);
}

/// A black plate with a dim frame.
pub fn plate(im: &mut Img, x: i32, y: i32, w: i32, h: i32) {
    im.rect(x, y, w, h, 'k');
    im.frame(x, y, w, h, 'g');
}

fn lamp(frame: u32) -> bool { frame / 6 % 2 == 0 }

/// The key hints along the foot of the screen.
fn hints(im: &mut Img, race: &rules::Session) {
    im.rect(0, 215, 384, 9, 'k');
    im.text(7, 216, if race.finished { "E NEXT RACE                      HOLD ESC LEAVE" }
        else if race.ticks < state::GRID_TICKS { "GAS ON THE LAST LAMP: ROCKET START   TOO SOON FLOODS" }
        else { "W GAS  A/D STEER  S BRAKE  SPACE BOOST  LIFT TO COOL" }, 't');
}

pub fn draw(im: &mut Img, c: &Ctx) {
    let race = c.race;
    if race.finished {
        // The board has the screen: the race behind it is pushed back to dots.
        for y in 0..215 { for x in 0..384 { if (x + y) % 2 == 0 { im.put(x, y, 'k'); } } }
        hints(im, race);
        return;
    }
    let place = state::place_of(c.order, c.seat);
    let now = state::race_time(race.ticks);
    let count = state::laps(race);
    let (lap_n, into) = state::lap_of(race, c.car);

    place_plate(im, place, race.cars.len());
    clock_plate(im, c, now, lap_n, count);
    section(im, c, into);

    minimap(im, c, 284, 5, 94, 66);
    ladder(im, c, LADDER.0 + 2, LADDER.1 + 2);
    progress(im, c);
    speed(im, c);
    engines(im, c, ENGINES.0, ENGINES.1);
    boost(im, c);
    rear(im, c, REAR.0, REAR.1, REAR.2, REAR.3);
    tags(im, c);
    edge_arrows(im, c);
    draft_cue(im, c);
    callout(im, c);
    self_mark(im, c);
    if state::wrong_way(c.clocks, race, c.seat) { wrong_way(im, race.ticks); }
    hints(im, race);
}

/// The engine gauges and the rear cue along the foot of the screen: (x, y) and (x, y, w, h).
pub const ENGINES: (i32, i32) = (110, 184);
pub const REAR: (i32, i32, i32, i32) = (192, 192, 82, 20);

/// What the grid screen holds at the sides of the picture, as (left, top, right, bottom): the
/// lamps and the count, and the ladder. Nothing else is drawn inside them.
pub const LIGHTS: (i32, i32, i32, i32) = (26, 72, 100, 166);
pub const LADDER: (i32, i32, i32, i32) = (344, 74, 384, 134);

/// The place counter, top left.
fn place_plate(im: &mut Img, place: usize, field: usize) {
    plate(im, 6, 5, 58, 46);
    shout(im, 12, 9, &place.to_string(), 4, if place == 1 { 'T' } else { 'H' });
    im.text(36, 11, state::ordinal(place), 'T');
    im.text(36, 21, &format!("/{field}"), 't');
    im.text(12, 40, "PLACE", 't');
}

/// Course, lap and the clocks, top middle. On the grid the clocks have nothing to say,
/// so the plate carries the course card instead: its laps and the lap's length.
fn clock_plate(im: &mut Img, c: &Ctx, now: f32, lap_n: u32, count: u32) {
    let race = c.race;
    plate(im, 70, 5, 208, 34);
    im.text(76, 8, state::course_name(race), 'T');
    if race.ticks < state::GRID_TICKS {
        im.text(76, 19, &format!("{} LAPS  {:.1} KM A LAP", count, state::lap_len(race) / 1000.), 'h');
        im.text(76, 29, "BEST --:--.--", 't');
        return;
    }
    let last_lap = lap_n + 1 == count && car_running(c.car);
    if last_lap && lamp(race.ticks) {
        im.text(76, 19, "FINAL LAP", 'O');
    } else {
        im.text(76, 19, &format!("LAP {}/{}", (lap_n + 1).min(count), count), 'H');
    }
    let shown = c.car.finish.unwrap_or(now.max(0.));
    im.text(150, 19, &format!("TIME {}", state::stamp(shown)), 'T');
    if let Some(s) = c.clocks.seats.get(&c.seat) {
        let this = if c.car.finish.is_some() { s.times.last().copied().unwrap_or(0.) } else { s.lap_time(now) };
        im.text(76, 29, &format!("THIS {}", state::stamp(this)), 't');
        match s.best() {
            Some(b) => { im.text(160, 29, &format!("BEST {}", state::stamp(b)), 'H'); }
            None => { im.text(160, 29, "BEST --:--.--", 't'); }
        }
    }
}

/// A small chevron over the viewer's own pod in its colour, and "YOU" for the
/// first seconds of the race, so the viewer's pod is never lost in the pack.
fn self_mark(im: &mut Img, c: &Ctx) {
    if c.race.ticks < state::GRID_TICKS + 12 || c.car.finish.is_some() { return; }
    // Only when the pack could hide it: on open road the pod at the bottom of the
    // picture is the viewer's, and a mark there would read as the pod ahead's.
    let crowded = c.order.iter().any(|&(id, o)| id != c.seat && o.finish.is_none() && (o.d - c.car.d).abs() < 16. && (o.y - c.car.y).abs() < 9.);
    if !crowded { return; }
    let p = point(c.car.d - 3., c.car.y, 3.4 + c.car.lift.max(0.));
    let Some((x, y, _)) = project(c.view, p) else { return };
    if !(10.0..W - 10.0).contains(&x) || !(52.0..H - 50.0).contains(&y) { return; }
    let (xi, yi) = (x as i32, y as i32);
    let ink = livery(c.seat).ink;
    for k in 0..5 {
        im.rect(xi - 4 + k, yi + k - 1, 9 - k * 2, 1, 'k');
        im.rect(xi - 3 + k, yi + k, 7 - k * 2, 1, if k < 3 { ink } else { 'H' });
    }
    if c.race.ticks < state::GRID_TICKS + 150 { centred(im, xi, yi - 10, "YOU", 1, 'H'); }
}

fn car_running(car: &rules::Car) -> bool { car.finish.is_none() }

/// The name of the stretch of road just entered, under the clocks for its
/// first hundred metres.
fn section(im: &mut Img, c: &Ctx, _into: f32) {
    let course = c.race.course();
    let len = state::lap_len(c.race);
    let s = c.car.d.max(0.).rem_euclid(len);
    let Some(z) = course.zones().iter().find(|z| s >= z.s0 && s < z.s1) else { return };
    if s - z.s0 > 90. || c.race.ticks < state::GRID_TICKS + 40 || c.car.finish.is_some() || state::wrong_way(c.clocks, c.race, c.seat) { return; }
    let w = text_w(z.name, 1) + 16;
    plate(im, 174 - w / 2, 42, w, 13);
    centred(im, 174, 46, z.name, 1, 'H');
}

/// The real course from the road's own samples (the forks too), fitted to
/// the plate with one scale on both axes, and a blip in each seat's colour.
fn minimap(im: &mut Img, c: &Ctx, x: i32, y: i32, w: i32, h: i32) {
    plate(im, x, y, w, h);
    let course = c.race.course();
    let len = state::lap_len(c.race);
    let main: Vec<(f32, f32)> = course.line().samples().iter().map(|p| (p.x, p.y)).collect();
    let forks: Vec<Vec<(f32, f32)>> = course.forks().iter().map(|f| f.line.samples().iter().map(|p| (p.x, p.y)).collect()).collect();
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for &(px, py) in main.iter().chain(forks.iter().flatten()) { lo = (lo.0.min(px), lo.1.min(py)); hi = (hi.0.max(px), hi.1.max(py)); }
    let (sx, sy) = ((hi.0 - lo.0).max(1.), (hi.1 - lo.1).max(1.));
    let (iw, ih) = ((w - 14) as f32, (h - 14) as f32);
    let k = (iw / sx).min(ih / sy);
    let (ox, oy) = (x as f32 + 7. + (iw - sx * k) * 0.5, y as f32 + 7. + (ih - sy * k) * 0.5);
    let at = |px: f32, py: f32| -> (i32, i32) { ((ox + (px - lo.0) * k).round() as i32, (oy + (hi.1 - py) * k).round() as i32) };
    let stride = (main.len() / 220).max(1);
    let trace = |im: &mut Img, pts: &[(f32, f32)], a: char, b: char| {
        let mut prev = None;
        for (i, &(px, py)) in pts.iter().enumerate() {
            if i % stride != 0 && i + 1 != pts.len() { continue; }
            let p = at(px, py);
            if let Some(q) = prev { let q: (i32, i32) = q; im.line(q.0, q.1, p.0, p.1, a); im.line(q.0 + 1, q.1, p.0 + 1, p.1, b); }
            prev = Some(p);
        }
    };
    for f in &forks { trace(im, f, 'j', 'g'); }
    trace(im, &main, 'G', 'j');
    // The line: a short bar across the road at the start.
    let (px, py) = at(main[0].0, main[0].1);
    im.rect(px - 2, py - 2, 5, 2, 'H');
    // Blips, the rest of the field first and the viewer last.
    for pass in 0..2 {
        for &(id, car) in c.order {
            if (id == c.seat) != (pass == 1) { continue; }
            let f = course.frame(car.route, car.d.rem_euclid(len));
            let (bx, by) = at(f.x, f.y);
            let ink = livery(id).ink;
            if id == c.seat {
                im.rect(bx - 3, by - 3, 7, 7, 'k');
                im.frame(bx - 3, by - 3, 7, 7, 'H');
                im.rect(bx - 1, by - 1, 3, 3, ink);
            } else {
                im.rect(bx - 2, by - 2, 5, 5, 'k');
                im.rect(bx - 1, by - 1, 3, 3, ink);
            }
        }
    }
}

/// Six rows of colour and name in running order: the same order as the place counter.
fn ladder(im: &mut Img, c: &Ctx, x: i32, y: i32) {
    let rows = c.order.len() as i32;
    plate(im, x, y, 34, rows * 9 + 3);
    for (n, &(id, car)) in c.order.iter().enumerate() {
        let ry = y + 3 + n as i32 * 9;
        let me = id == c.seat;
        if me { im.rect(x + 1, ry - 1, 32, 9, 'g'); }
        im.text(x + 3, ry, &(n + 1).to_string(), if me { 'H' } else { 't' });
        im.rect(x + 11, ry + 1, 4, 5, livery(id).ink);
        let name: String = livery(id).name.chars().take(3).collect();
        im.text(x + 17, ry, &name, if car.finish.is_some() { 'T' } else if me { 'H' } else { 'J' });
    }
}

/// The race in one line: where every seat is along the whole distance.
fn progress(im: &mut Img, c: &Ctx) {
    let (x0, x1, y) = (8, 376, 177);
    let total = state::course_len(c.race);
    let len = state::lap_len(c.race);
    im.rect(x0, y, x1 - x0, 1, 'j');
    for l in 0..=state::laps(c.race) {
        let x = x0 + (l as f32 * len / total * (x1 - x0) as f32) as i32;
        im.rect(x.min(x1 - 1), y - 2, 1, 5, 'h');
    }
    for pass in 0..2 {
        for &(id, car) in c.order {
            if (id == c.seat) != (pass == 1) { continue; }
            let x = x0 + (car.d.max(0.) / total * (x1 - x0) as f32).min((x1 - x0) as f32) as i32;
            if id == c.seat { im.rect(x - 2, y - 3, 5, 7, 'k'); im.rect(x - 1, y - 2, 3, 5, livery(id).ink); }
            else { im.rect(x - 1, y - 2, 3, 5, livery(id).ink); }
        }
    }
}

fn speed(im: &mut Img, c: &Ctx) {
    plate(im, 6, 184, 100, 28);
    let kmh = (c.car.v * 12.).round() as u32;
    shout(im, 11, 188, &format!("{kmh:03}"), 3, if c.car.boosting { 'T' } else { 'H' });
    im.text(72, 190, "KM/H", 't');
    // A fine bar under the digits: cruise is the notch, boost runs past it.
    let fill = (c.car.v / 64. * 90.) as i32;
    im.rect(11, 207, 90, 2, 'g');
    im.rect(11, 207, fill.min(90), 2, if c.car.v > c.car.spec().top + 1. { 'T' } else { 'h' });
    im.rect(11 + (c.car.spec().top / 64. * 90.) as i32, 205, 1, 5, 'H');
}

fn boost(im: &mut Img, c: &Ctx) {
    let car = c.car;
    plate(im, 278, 184, 100, 28);
    let (label, tint) = if car.boost_locked { ("OVERHEAT", 'O') }
        else if car.boosting { ("BOOSTING", 'T') } else { ("BOOST", 'h') };
    let lit = if car.boost_locked && lamp(c.race.ticks) { 'k' } else { tint };
    im.text(284, 188, label, lit);
    im.text(350, 188, &format!("{:3}%", (car.boost * 100.) as u32), 'T');
    let cells = 16;
    for n in 0..cells {
        let on = (n as f32) < car.boost * cells as f32;
        // The last third runs hot: the cells cool from gold to amber to red as it empties.
        let ink = if !on { 'g' } else if car.boost_locked { 'O' } else if n < 5 { 'R' } else if n < 10 { 'O' } else { tint };
        im.rect(284 + n * 5, 199, 4, 8, ink);
    }
}

/// The sim's engine 0 is the pod's lesser-`y` side and the picture shows lesser `y` on the left
/// (a test holds the camera to it): the road's greater-`y` side is the right of the screen.
pub fn engine_label(i: usize) -> &'static str { if i == 0 { "L" } else { "R" } }
pub fn engine_word(i: usize) -> &'static str { if i == 0 { "LEFT" } else { "RIGHT" } }

/// The twin engine gauges: heat in each nacelle, the notch where it catches fire, and the scar
/// of wear the squires have yet to patch.
fn engines(im: &mut Img, c: &Ctx, x: i32, y: i32) {
    let car = c.car;
    plate(im, x, y, 78, 28);
    let blink = c.race.ticks / 4 % 2 == 0;
    let hot = car.hottest();
    let (title, tint) = if car.blown() { ("BLOWN", if blink { 'O' } else { 'R' }) }
        else if car.burning() { ("ON FIRE", if blink { 'O' } else { 'R' }) }
        else if hot >= WARN_HEAT { ("RUNNING HOT", 'O') }
        else { ("ENGINES", 't') };
    im.text(x + 5, y + 3, title, tint);
    // Left on the picture first.
    for i in 0..2usize {
        let row = i;
        let (h, wear) = (car.heat[i], car.wear[i]);
        let by = y + 13 + row as i32 * 8;
        let (bx, bw) = (x + 13, 60);
        im.text(x + 5, by - 1, engine_label(i), if h >= FIRE_HEAT && blink { 'O' } else { 't' });
        im.rect(bx, by, bw, 5, 'g');
        let fill = (h * bw as f32).round() as i32;
        let ink = if h >= FIRE_HEAT { '7' } else if h >= WARN_HEAT { '4' } else { '2' };
        im.rect(bx, by, fill, 5, ink);
        // On fire, the tip of the bar flares white-hot.
        if h >= FIRE_HEAT && fill > 2 { im.rect(bx + fill - 3, by, 3, 5, if blink { '6' } else { 'O' }); }
        im.rect(bx, by, ((wear * bw as f32).round() as i32).min(fill), 5, 'a');
        im.rect(bx + (WARN_HEAT * bw as f32) as i32, by - 1, 1, 7, 'j');
        im.rect(bx + (FIRE_HEAT * bw as f32) as i32, by - 1, 1, 7, 'H');
    }
}

/// The wake: chevrons laid along the road from this pod's nose to the pod ahead whose wake it
/// rides, and a meter for how deep in it the pod sits.
fn draft_cue(im: &mut Img, c: &Ctx) {
    let car = c.car;
    if car.draft < 0.06 || car.finish.is_some() || car.blown() || c.race.ticks < state::GRID_TICKS { return; }
    let t = c.race.ticks as f32;
    if let Some(lead) = c.race.cars.get(&u32::from(car.wake)).filter(|_| car.wake != 0) {
        let lap = c.race.course().lap();
        let from = car.d + rules::race::contact::nose(car);
        let gap = (rules::race::contact::ahead(lap, car.d, lead.d) - rules::race::contact::nose(car) - rules::race::contact::tail(lead)).max(1.);
        for n in 0..5 {
            let u = ((t * 0.045 + n as f32 / 5.) % 1.).clamp(0.02, 0.98);
            let lane = car.y + (lead.y - car.y) * u;
            let Some((px, py, z)) = project(c.view, point(from + gap * u, lane, 0.5)) else { continue };
            if !(10.0..W - 10.0).contains(&px) || !(56.0..172.0).contains(&py) || z > 90. { continue; }
            let w = (7.0 / z.max(6.) * 14.).clamp(1., 6.) as i32;
            let strong = car.draft > 0.55;
            let ink = if strong { if n % 2 == 0 { 'W' } else { '3' } } else { '2' };
            let (xi, yi) = (px as i32, py as i32);
            for k in 0..2 { im.line(xi - w, yi + w / 2 + k, xi, yi - w / 2 + k, ink); im.line(xi + w, yi + w / 2 + k, xi, yi - w / 2 + k, ink); }
        }
    }
    // The meter above the pod: it fills as the draft deepens and drains as the slingshot spends it.
    let cells = 8;
    // High over the pod: the callouts own the middle of the picture (y 98 to 148), and the meter would sit under them.
    let (x0, y0) = (192 - cells * 3, 92);
    im.text(192 - text_w("DRAFT", 1) / 2, y0 - 9, "DRAFT", if car.draft > 0.55 { 'W' } else { '2' });
    for n in 0..cells {
        let on = (n as f32 + 0.5) / cells as f32 <= car.draft;
        im.rect(x0 + n * 6, y0, 5, 3, if on { if car.draft > 0.55 { '3' } else { '2' } } else { 'g' });
    }
}

/// The one thing the screen has to say about the engines and the traffic, in the middle of it, by
/// how much it matters: a blowout, a fire, a slam, the start, a pass, a warning.
fn callout(im: &mut Img, c: &Ctx) {
    let car = c.car;
    if car.finish.is_some() || c.race.ticks < state::GRID_TICKS { return; }
    let blink = c.race.ticks / 4 % 2 == 0;
    let hot = (0..2).max_by(|&a, &b| car.heat[a].total_cmp(&car.heat[b])).unwrap_or(0);
    if car.blown() {
        let which = if car.blown_side > 0 { usize::from(car.blown_side) - 1 } else { hot };
        plate(im, 80, 98, 224, 50);
        centred(im, 192, 102, "BLOWOUT", 3, if blink { 'O' } else { 'T' });
        centred(im, 192, 126, &format!("{} ENGINE  THE SQUIRES ARE ON IT", engine_word(which)), 1, 'h');
        let done = 1. - car.crash as f32 / rules::race::engines::BLOWOUT_TICKS as f32;
        im.rect(100, 137, 184, 4, 'g');
        im.rect(100, 137, (184. * done) as i32, 4, 'C');
    } else if car.burning() {
        plate(im, 76, 108, 232, 28);
        if blink { centred(im, 192, 112, &format!("{} ENGINE BURNING  {:3}%", engine_word(hot), (car.heat[hot] * 100.) as u32), 1, 'O'); }
        centred(im, 192, 123, "LIFT OFF THE GAS TO COOL IT", 1, 'T');
    } else if car.slam > 0 {
        centred(im, 192, 110, "SLAM!", 2, if blink { 'O' } else { 'T' });
        // The side it came from lights up.
        if car.slam_side != 0 {
            // slam_side is the direction of the other pod: greater `y` is the right of the picture.
            im.rect(if car.slam_side < 0 { 0 } else { 380 }, 52, 4, 120, 'O');
        }
    } else if car.launch_t > 0 && car.launch != 0 {
        if car.launch > 0 { centred(im, 192, 128, "ROCKET START!", 2, 'W'); } else { centred(im, 192, 128, "FLOODED!", 2, 'O'); }
    } else if car.cheer > 0 {
        centred(im, 192, 112, "PASSED A RIVAL!", 1, 'y');
    } else if car.hottest() >= WARN_HEAT + 0.1 && blink {
        centred(im, 192, 118, &format!("{} ENGINE HOT", engine_word(hot)), 1, 'O');
    }
}

/// The cue for what is behind: rivals within 70 m on the track at their own lane.
fn rear(im: &mut Img, c: &Ctx, x: i32, y: i32, w: i32, h: i32) {
    plate(im, x, y, w, h);
    let behind: Vec<_> = c.order.iter().filter(|(id, car)| *id != c.seat && car.finish.is_none() && c.car.d - car.d > -4. && c.car.d - car.d < 70.).collect();
    let near = behind.iter().map(|(_, car)| c.car.d - car.d).fold(f32::MAX, f32::min);
    let hot = near < 22.;
    if hot && lamp(c.race.ticks) { im.frame(x, y, w, h, 'O'); }
    im.text(x + 5, y + 4, "REAR", if hot { 'O' } else { 't' });
    let gap = if behind.is_empty() { "CLEAR".to_string() } else { format!("{:.0}M", near.max(0.)) };
    im.text(x + 5, y + 11, &gap, if hot { 'T' } else { 'h' });
    // The road behind, top to bottom: me at the top, 70 m down at the bottom.
    let (rx, rw) = (x + 37, w - 43);
    im.rect(rx, y + 4, rw, h - 8, 'K');
    im.rect(rx, y + 4, 1, h - 8, 'j'); im.rect(rx + rw - 1, y + 4, 1, h - 8, 'j');
    let lane = |yy: f32| rx + 3 + ((yy + 13.) / 26. * (rw - 7) as f32).clamp(0., (rw - 7) as f32) as i32;
    for &&(id, car) in &behind {
        let gap = (c.car.d - car.d).max(0.);
        let (px, py) = (lane(car.y), y + 6 + (gap / 70. * (h - 10) as f32) as i32);
        im.rect(px - 2, py - 1, 5, 3, 'k');
        im.rect(px - 1, py - 1, 3, 3, livery(id).ink);
    }
    let (mx, my) = (lane(c.car.y), y + 5);
    im.rect(mx - 3, my - 2, 7, 5, 'k');
    im.rect(mx - 2, my - 1, 5, 3, livery(c.seat).ink);
    im.rect(mx - 1, my - 3, 3, 1, 'H');
}

/// A pennant over every rival on screen: their colour and their place, and
/// the gap in metres for the near ones. Hidden behind a crest or a wall, and
/// stacked up when two would sit on one another.
fn tags(im: &mut Img, c: &Ctx) {
    let grid = c.race.ticks < state::GRID_TICKS + 20;
    let mut seen: Vec<(i32, i32, i32, i32)> = Vec::new();
    let hits = |taken: &[(i32, i32, i32, i32)], r: (i32, i32, i32, i32)| taken.iter().any(|t| r.0 < t.2 && r.2 > t.0 && r.1 < t.3 && r.3 > t.1);
    // Where a pennant never stands: the ladder, and on the grid the lamps and the count.
    let keep_out: &[(i32, i32, i32, i32)] = if grid { &[LIGHTS, LADDER] } else { &[LADDER] };
    // Nearest first, so the pods that matter get the clear air.
    let mut rows: Vec<_> = c.order.iter().filter(|(id, _)| *id != c.seat || grid).collect();
    rows.sort_by(|a, b| (a.1.d - c.car.d).abs().total_cmp(&(b.1.d - c.car.d).abs()));
    for &&(id, car) in &rows {
        let p = point(car.d - 2.5, car.y, 6.8 + car.lift.max(0.));
        let Some((x, y, z)) = project(c.view, p) else { continue };
        if z > 150. || !(6.0..W - 6.0).contains(&x) || !(52.0..H - 40.0).contains(&y) { continue; }
        let (xi, yi) = (x as i32, y as i32);
        let covered = c.depth.get((yi.clamp(0, 223) * 384 + xi.clamp(0, 383)) as usize).is_some_and(|&d| d < z - 5.0);
        if covered { continue; }
        let ink = livery(id).ink;
        let place = state::place_of(c.order, id);
        let label = if grid { id.to_string() } else { place.to_string() };
        let (bw, bh) = (text_w(&label, 1) + 5, 11);
        let bx = xi - bw / 2;
        let mut by = yi - bh - 3;
        // Stack up past anything already there.
        for _ in 0..4 {
            if !hits(&seen, (bx - 1, by - 1, bx + bw + 1, by + bh + 1)) { break; }
            by -= bh + 2;
        }
        if by < 50 || hits(keep_out, (bx - 2, by - 11, bx + bw + 2, by + bh + 4)) { continue; }
        seen.push((bx - 1, by - 1, bx + bw + 1, by + bh + 4));
        im.rect(bx, by, bw, bh, 'k');
        im.frame(bx, by, bw, bh, ink);
        im.text(bx + 3, by + 2, &label, if id == c.seat { 'H' } else { ink });
        // The pointer down to the pod.
        im.rect(bx + bw / 2 - 1, by + bh, 3, 1, ink);
        im.rect(bx + bw / 2, by + bh + 1, 1, 1, ink);
        if id == c.seat {
            centred(im, bx + bw / 2, by - 9, "YOU", 1, 'H');
        } else if !grid {
            let gap = car.d - c.car.d;
            let s = format!("{}{:.0}", if gap > 0. { "+" } else { "-" }, gap.abs());
            let r = (bx + bw / 2 - text_w(&s, 1) / 2, by + bh + 3, bx + bw / 2 + text_w(&s, 1) / 2 + 1, by + bh + 10);
            if gap.abs() < 99. && !hits(&seen, r) {
                seen.push(r);
                centred(im, bx + bw / 2, by + bh + 3, &s, 1, 'J');
            }
        }
    }
}

/// Rivals just off the edge of the picture, level with the car: an arrow on
/// that side in their colour, so a pod alongside is never invisible.
fn edge_arrows(im: &mut Img, c: &Ctx) {
    if c.race.ticks < state::GRID_TICKS { return; }
    for &(id, car) in c.order {
        if id == c.seat || car.finish.is_some() { continue; }
        let gap = c.car.d - car.d;
        if gap.abs() > 26. { continue; }
        let p = point(car.d - 2.5, car.y, 2.0);
        let on = project(c.view, p).is_some_and(|(x, _, _)| (14.0..W - 14.0).contains(&x));
        if on { continue; }
        // Which side of the lens it lies on: the sign along the camera's right.
        let (sh, ch) = c.view.heading_rad.sin_cos();
        let rel = p - c.view.pos;
        let side = if rel.x * -sh + rel.y * ch >= 0. { 1 } else { -1 };
        let ink = livery(id).ink;
        let y = 128 - (gap / 26. * 20.) as i32;
        for k in 0..6 {
            let hgt = 2 * (6 - k) - 1;
            let x = if side > 0 { 366 + k } else { 17 - k };
            im.rect(x, y - hgt / 2, 1, hgt, ink);
        }
    }
}

fn wrong_way(im: &mut Img, ticks: u32) {
    let on = lamp(ticks);
    plate(im, 112, 58, 160, 36);
    if on { im.frame(112, 58, 160, 36, 'O'); }
    centred(im, 192, 64, "WRONG WAY", 3, if on { 'O' } else { 'T' });
    centred(im, 192, 84, "TURN ABOUT", 1, 'h');
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::camera;

    /// The box of dots an element paints on a clear picture: (left, top, right, bottom), exclusive.
    fn extent(draw: impl FnOnce(&mut Img)) -> (i32, i32, i32, i32) {
        let mut im = Img::new(384, 224);
        draw(&mut im);
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..224 { for x in 0..384 { if im.get(x, y).is_some() { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1); y1 = y1.max(y + 1); } } }
        (x0, y0, x1, y1)
    }

    /// The same, ignoring the rim of the picture (columns 0..8 and 376..384).
    fn extent_inner(draw: impl FnOnce(&mut Img)) -> (i32, i32, i32, i32) {
        let mut im = Img::new(384, 224);
        draw(&mut im);
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..224 { for x in 8..376 { if im.get(x, y).is_some() { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1); y1 = y1.max(y + 1); } } }
        (x0, y0, x1, y1)
    }

    fn touches(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

    /// The engine gauge called L is on the left of the picture: the sim's engine 0 is the lesser-`y`
    /// side of the pod, and the chase camera shows lesser `y` on the left.
    #[test]
    fn the_left_gauge_is_the_engine_on_the_left_of_the_picture() {
        let mut race = rules::Session::blank_on(rules::cup()[0]);
        race.join_ids(1..=6);
        race.ticks = 200;
        let car = &race.cars[&1];
        let view = camera::view(&race, car);
        let plus = project(&view, point(car.d, car.y + 3., 1.)).unwrap().0;
        let minus = project(&view, point(car.d, car.y - 3., 1.)).unwrap().0;
        assert!(minus < plus, "+y shows at x {plus}, -y at x {minus}: greater y is the right of the picture");
    }

    /// A pod's gauges and callouts, on a clear picture, inside the picture and off the other instruments.
    #[test]
    fn the_engine_gauges_and_callouts_fit_and_say_the_right_thing() {
        let mut race = rules::Session::blank_on(rules::cup()[0]);
        race.join_ids(1..=6);
        race.ticks = state::GRID_TICKS + 400;
        let clocks = state::observe(&race);
        let order = state::standings(&race);
        let view = camera::view(&race, &race.cars[&1]);
        let depth = vec![f32::INFINITY; 384 * 224];
        let mut seen = std::collections::BTreeSet::new();
        for (heat, crash, slam, cheer, launch_t, draft) in [([0.2, 0.2], 0, 0, 0, 0, 0.), ([0.2, 0.6], 0, 0, 0, 0, 0.), ([0.8, 0.3], 0, 0, 0, 0, 0.), ([1., 0.3], 80, 0, 0, 0, 0.), ([0.2, 0.2], 0, 20, 0, 0, 0.), ([0.2, 0.2], 0, 0, 30, 0, 0.), ([0.2, 0.2], 0, 0, 0, 20, 0.), ([0.2, 0.2], 0, 0, 0, 0, 0.8)] {
            let mut car = race.cars[&1].clone();
            car.heat = heat; car.crash = crash; car.slam = slam; car.slam_side = 1; car.cheer = cheer; car.launch = 1; car.launch_t = launch_t; car.draft = draft; car.v = 40.;
            if crash > 0 { car.blown_side = 1; }
            if draft > 0. { car.wake = 2; }
            let c = Ctx { race: &race, car: &car, seat: 1, view: &view, depth: &depth, order: &order, clocks: &clocks };
            let g = extent(|im| engines(im, &c, ENGINES.0, ENGINES.1));
            assert_eq!(g, (ENGINES.0, ENGINES.1, ENGINES.0 + 78, ENGINES.1 + 28), "the gauge fills its plate and no more");
            // The edge bars (a slam's side) are the screen's rim, and run under the ladder as the wall strike's do.
            let words = extent_inner(|im| callout(im, &c));
            if words.0 != i32::MAX {
                assert!(words.0 >= 0 && words.2 <= 384 && words.1 >= 52 && words.3 <= 172, "{heat:?}: the callout leaves the open road {words:?}");
                assert!(!touches(words, (284, 5, 378, 71)) && !touches(words, (LADDER.0, LADDER.1, LADDER.2, LADDER.3)), "{heat:?}: the callout covers the map or the ladder {words:?}");
                seen.insert((words.1, words.3));
            }
            let cue = extent(|im| draft_cue(im, &c));
            assert_eq!(cue.0 != i32::MAX, draft > 0., "the draft meter shows only in a draft");
        }
        assert!(seen.len() >= 3, "different things are said in different places: {seen:?}");
    }

    /// Every instrument the grid screen holds, each painted alone, on every course: none touches another.
    #[test]
    fn the_grid_screen_holds_every_instrument_apart() {
        for &id in rules::courses() {
            let mut race = rules::Session::blank_on(id);
            race.join_ids(1..=6);
            for ticks in [0, 20, 45, 70, state::GRID_TICKS - 1] {
                race.ticks = ticks;
                let clocks = state::observe(&race);
                let order = state::standings(&race);
                let car = &race.cars[&1];
                let view = camera::view(&race, car);
                let depth = vec![f32::INFINITY; 384 * 224];
                let c = Ctx { race: &race, car, seat: 1, view: &view, depth: &depth, order: &order, clocks: &clocks };
                let place = state::place_of(&order, 1);
                let parts: Vec<(&str, (i32, i32, i32, i32))> = vec![
                    ("place", extent(|im| place_plate(im, place, 6))),
                    ("clocks", extent(|im| clock_plate(im, &c, 0., 0, state::laps(&race)))),
                    ("map", extent(|im| minimap(im, &c, 284, 5, 94, 66))),
                    ("ladder", extent(|im| ladder(im, &c, LADDER.0 + 2, LADDER.1 + 2))),
                    ("lights", extent(|im| super::flow::lights(im, ticks))),
                    ("progress", extent(|im| progress(im, &c))),
                    ("speed", extent(|im| speed(im, &c))),
                    ("rear", extent(|im| rear(im, &c, REAR.0, REAR.1, REAR.2, REAR.3))),
                    ("engines", extent(|im| engines(im, &c, ENGINES.0, ENGINES.1))),
                    ("boost", extent(|im| boost(im, &c))),
                    ("hints", extent(|im| hints(im, &race))),
                ];
                for (i, (an, a)) in parts.iter().enumerate() {
                    assert!(a.0 >= 0 && a.1 >= 0 && a.2 <= 384 && a.3 <= 224, "{an} leaves the picture on {id:?}: {a:?}");
                    for (bn, b) in &parts[i + 1..] {
                        assert!(!touches(*a, *b), "{an} {a:?} touches {bn} {b:?} on {id:?} at tick {ticks}");
                    }
                }
                // The words the clock plate carries on the grid fit inside it.
                let words = extent(|im| clock_plate(im, &c, 0., 0, state::laps(&race)));
                assert!(words.2 <= 278 && words.3 <= 39, "the course card spills out of its plate: {words:?}");
                // The fixed side boxes hold what they say they hold.
                let lights = extent(|im| super::flow::lights(im, ticks));
                assert!(lights.0 >= LIGHTS.0 && lights.2 <= LIGHTS.2 && lights.1 >= LIGHTS.1 && lights.3 <= LIGHTS.3, "the lamps leave their box: {lights:?} vs {LIGHTS:?}");
                let ladder_box = extent(|im| ladder(im, &c, LADDER.0 + 2, LADDER.1 + 2));
                assert!(ladder_box.0 >= LADDER.0 && ladder_box.2 <= LADDER.2 && ladder_box.1 >= LADDER.1 && ladder_box.3 <= LADDER.3, "the ladder leaves its box: {ladder_box:?} vs {LADDER:?}");
            }
        }
    }
}
