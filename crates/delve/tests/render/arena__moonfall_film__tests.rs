//! Moonfall's moments as films. The rules are a Bend module this crate's
//! tests cannot run, so a film poses the session the way the rules would
//! leave it, tick by tick (the clock, the phase, the cards and notes
//! counting down, the knights walking) and draws every tick with the real
//! stage. `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve --lib
//! write_moonfall_films -- --ignored` writes `<dir>/films/<name>/f0000.png`...
//! (`MF_FILM=a,b` picks some). What it shows is the picture, not the play.
use super::super::super::ink::Img;
use crate::rules::{
    Run,
    moonfall::{
        self as rules, clock,
        state::{Knight, Mask, Phase, SPAWN, Seat},
    },
};
use std::path::PathBuf;

pub(super) struct Reel {
    dir: PathBuf,
    n: u32,
}

impl Reel {
    pub(super) fn new(name: &str) -> Option<Reel> {
        let root = std::env::var_os("ANGEL_ARENA_SHOTS")?;
        if let Ok(only) = std::env::var("MF_FILM")
            && !only.split(',').any(|o| o == name)
        {
            return None;
        }
        // `MF_GUIDES=1` shows the guide lines the effects ride (the debug layer), in films of their own.
        let name = if std::env::var_os("MF_GUIDES").is_some() { format!("{name}-guides") } else { name.to_string() };
        let dir = PathBuf::from(root).join("films").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Some(Reel { dir, n: 0 })
    }

    pub(super) fn shot(&mut self, r: &Run, focus: u32) {
        // As the real route lays it: the effects layer over the base.
        let (mut img, fx) = super::layers(r, Some(focus));
        img.stamp(&fx, 0, 0);
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(self.dir.join(format!("f{:04}.png", self.n)))
            .unwrap();
        self.n += 1;
    }
}

pub(super) fn table(seats: u32) -> Run {
    let mut r = Run::moonfall_for_test();
    for id in 2..=seats {
        r.join_seat(id, &format!("Knight {id}"));
    }
    let s = r.moonfall.as_mut().unwrap();
    for id in 1..=seats {
        let at = SPAWN[(id as usize + 5) % 6];
        s.seats.insert(id, Seat { joined: 1, knight: Knight::at(at) });
    }
    s.peak = s.seats.len() as u32;
    s.card = 0;
    r
}

pub(super) fn s(r: &mut Run) -> &mut rules::Session {
    r.moonfall.as_mut().unwrap()
}

/// The clock at `hh:mm` of day `day` (the loop's calendar day, 1 to 3).
pub(super) fn at_hour(r: &mut Run, day: u32, hh: u32, mm: u32) {
    let s = s(r);
    let h2 = ((day - 1) * 24 + if hh < 6 { hh + 18 } else { hh - 6 }) * 2 + mm / 30;
    s.clock = clock::ticks(s.nums.day, h2) + (mm % 30) * s.nums.day / 1440;
}

pub(super) fn put(r: &mut Run, id: u32, px: (i32, i32), face: u8) {
    let k = &mut s(r).seats.get_mut(&id).unwrap().knight;
    k.x = px.0 * 256;
    k.y = px.1 * 256;
    k.face = face;
}

/// A knight walking from `a` to `b` over `n` ticks, `t` in.
pub(super) fn walk(r: &mut Run, id: u32, a: (i32, i32), b: (i32, i32), t: u32, n: u32) {
    let f = (t.min(n) as f32) / n as f32;
    let (x, y) = (a.0 as f32 + (b.0 - a.0) as f32 * f, a.1 as f32 + (b.1 - a.1) as f32 * f);
    let face = if (b.0 - a.0).abs() > (b.1 - a.1).abs() { if b.0 > a.0 { 3 } else { 2 } } else if b.1 > a.1 { 0 } else { 1 };
    let k = &mut s(r).seats.get_mut(&id).unwrap().knight;
    k.x = x as i32 * 256;
    k.y = y as i32 * 256;
    k.face = face;
    k.stride = if t < n { t + 1 } else { 0 };
}

/// One tick, as the rules would leave it in play: the clock runs, the
/// cards and the notes count down.
pub(super) fn tick(r: &mut Run) {
    r.tick += 1;
    let s = s(r);
    s.elapsed += 1;
    if matches!(s.phase, Phase::Play) {
        s.clock += 1;
    }
    s.card = s.card.saturating_sub(1);
    for n in &mut s.news {
        n.1 = n.1.saturating_sub(1);
    }
    s.news.retain(|n| n.1 > 0);
    for seat in s.seats.values_mut() {
        if let Some((_, left)) = seat.knight.note.as_mut() {
            *left = left.saturating_sub(1);
        }
        if seat.knight.note.is_some_and(|(_, left)| left == 0) {
            seat.knight.note = None;
        }
    }
}

fn film_cast() {
    let Some(mut reel) = Reel::new("cast") else { return };
    let mut r = table(1);
    at_hour(&mut r, 1, 6, 40);
    for f in 0..200u32 {
        tick(&mut r);
        walk(&mut r, 1, (150, 250), (520, 250), f, 200);
        reel.shot(&r, 1);
    }
}

/// Two knights standing about the plaza, the first the viewer.
fn plaza(seats: u32) -> Run {
    let mut r = table(seats);
    put(&mut r, 1, (296, 262), 0);
    if seats > 1 {
        put(&mut r, 2, (330, 270), 2);
    }
    if seats > 2 {
        put(&mut r, 3, (262, 272), 3);
    }
    r
}

fn film_rewind(fell: bool) {
    let Some(mut reel) = Reel::new(if fell { "fall" } else { "rewind" }) else { return };
    let mut r = plaza(3);
    let day = s(&mut r).nums.day;
    let ctl = rules::control(&s(&mut r).nums);
    if fell {
        s(&mut r).clock = clock::end(day) - 40;
    } else {
        at_hour(&mut r, 2, 20, 0);
    }
    let from = s(&mut r).clock;
    for _ in 0..10 {
        tick(&mut r);
        reel.shot(&r, 1);
    }
    let from = from + 10;
    s(&mut r).clock = from;
    for t in 0..ctl {
        r.tick += 1;
        s(&mut r).elapsed += 1;
        s(&mut r).phase = Phase::Rewind { t, from, fell };
        reel.shot(&r, 1);
    }
    // The town lands on its first dawn, a loop older.
    let ss = s(&mut r);
    ss.phase = Phase::Play;
    ss.clock = 0;
    ss.memory.loops = 1;
    ss.card = ss.nums.rewind.saturating_sub(ctl) + 24;
    for _ in 0..90 {
        tick(&mut r);
        reel.shot(&r, 1);
    }
}

/// The bell's wait: time runs on to `to` (hours, at day `day`), then the town plays.
fn film_skip(name: &str, from_h: (u32, u32, u32), to_h: (u32, u32, u32), after: u32) {
    let Some(mut reel) = Reel::new(name) else { return };
    let mut r = plaza(2);
    at_hour(&mut r, from_h.0, from_h.1, from_h.2);
    let from = s(&mut r).clock;
    at_hour(&mut r, to_h.0, to_h.1, to_h.2);
    let to = s(&mut r).clock;
    s(&mut r).clock = from;
    s(&mut r).memory.loops = 1;
    for _ in 0..6 {
        tick(&mut r);
        s(&mut r).clock = from;
        reel.shot(&r, 1);
    }
    for t in 0..rules::SKIP_LEN {
        r.tick += 1;
        s(&mut r).elapsed += 1;
        s(&mut r).phase = Phase::Skip { t, from, to };
        reel.shot(&r, 1);
    }
    s(&mut r).phase = Phase::Play;
    s(&mut r).clock = to;
    for _ in 0..after {
        tick(&mut r);
        reel.shot(&r, 1);
    }
}

/// The moon on three nights, the knight pacing so its eyes follow.
fn film_moon() {
    let Some(mut reel) = Reel::new("moon") else { return };
    let mut r = plaza(2);
    s(&mut r).memory.loops = 1;
    for (day, hh, mm) in [(1, 23, 0), (2, 23, 0), (3, 22, 0), (3, 1, 30)] {
        at_hour(&mut r, day, hh, mm);
        for f in 0..44u32 {
            r.tick += 1;
            s(&mut r).elapsed += 1;
            let (a, b) = if f < 22 { ((200, 262), (380, 262)) } else { ((380, 262), (200, 262)) };
            walk(&mut r, 1, a, b, f % 22, 22);
            reel.shot(&r, 1);
        }
    }
}

fn film_mask(mask: Mask, name: &str) {
    let Some(mut reel) = Reel::new(name) else { return };
    let mut r = plaza(2);
    s(&mut r).memory.loops = 1;
    at_hour(&mut r, 2, 7, 10);
    for f in 0..96u32 {
        tick(&mut r);
        if f == 8 {
            let k = &mut s(&mut r).seats.get_mut(&1).unwrap().knight;
            k.note = Some((crate::rules::moonfall::state::Note::Mask(mask), 90));
            k.mask = mask;
        }
        reel.shot(&r, 1);
    }
}

fn film_ledger() {
    use crate::rules::moonfall::{folk::Folk, ledger::Fact, state::Note};
    let Some(mut reel) = Reel::new("ledger") else { return };
    let mut r = plaza(2);
    s(&mut r).memory.loops = 1;
    at_hour(&mut r, 1, 7, 10);
    for f in 0..110u32 {
        tick(&mut r);
        match f {
            6 => s(&mut r).news.push((Note::Learned(Fact::PellaScroll), 120)),
            52 => s(&mut r).news.push((Note::Ticked(Folk::Orin), 120)),
            _ => {}
        }
        if f == 66 {
            let k = &mut s(&mut r).seats.get_mut(&1).unwrap().knight;
            k.note = Some((Note::Got(crate::rules::moonfall::state::Item::Scroll), 60));
            k.held = crate::rules::moonfall::state::Item::Scroll;
        }
        reel.shot(&r, 1);
    }
}

fn film_finale() {
    let Some(mut reel) = Reel::new("finale") else { return };
    let mut r = plaza(3);
    let day = s(&mut r).nums.day;
    s(&mut r).memory.loops = 2;
    at_hour(&mut r, 3, 0, 20);
    let now = s(&mut r).clock;
    s(&mut r).world.lit_until = [now + 600; 3];
    for _ in 0..12 {
        tick(&mut r);
        reel.shot(&r, 1);
    }
    for t in 0..rules::WON_LEN {
        r.tick += 1;
        s(&mut r).elapsed += 1;
        s(&mut r).phase = Phase::Won { t };
        reel.shot(&r, 1);
    }
    let ss = s(&mut r);
    ss.phase = Phase::Play;
    ss.completed = Some(ss.elapsed);
    let _ = day;
    for _ in 0..120 {
        r.tick += 1;
        s(&mut r).elapsed += 1;
        reel.shot(&r, 1);
    }
}

#[test]
#[ignore = "writes Moonfall's films"]
fn write_moonfall_films() {
    super::moments::show_guides(std::env::var_os("MF_GUIDES").is_some());
    film_cast();
    film_rewind(false);
    film_rewind(true);
    film_skip("dawn", (2, 4, 30), (3, 6, 0), 70);
    film_skip("dusk", (1, 15, 30), (1, 18, 0), 70);
    film_moon();
    film_mask(Mask::Swift, "mask");
    film_mask(Mask::Stout, "mask-stout");
    film_ledger();
    film_finale();
    super::moments::show_guides(false);
    let _ = Img::new(1, 1);
}
