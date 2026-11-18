//! The Keep from afar (the Works, `world::works`): the castle is bigger than
//! the 24 x 14 tile view, so a knight at the site can stand back. Holding the
//! map key (N) at the site, for that seat alone, pulls the camera out three
//! times, far enough to see the whole castle in both views; the Chronicle
//! (`rules::site::CHRONICLE_TICKS`) stands everyone at the site back for its
//! thirty seconds.
//!
//! In 3D the view is three times the native size (`ZOOM`), the world drawn at
//! the same scale into a bigger stage, so the window shows it smaller and the
//! HUD bands follow (`party::lift_with` reckons them by the stage's size). In
//! the flat view the same window of the map is shrunk to the native view.
//!
//! Also here: the progress readout (stones laid of the total, the stage's
//! name), kept short in the view's top left, under the realm's label.

use super::*;
use crate::world::works;

pub const ZOOM: i32 = 3;

std::thread_local! {
    static BACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
std::thread_local! {
    pub(crate) static WENT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

pub(crate) fn on() -> bool {
    BACK.with(|c| c.get())
}

pub(crate) fn zoom() -> i32 {
    if on() { ZOOM } else { 1 }
}

pub fn with<T>(run: &Run, focus: Option<u32>, held: bool, f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BACK.with(|c| c.set(self.0));
        }
    }
    let back = wanted(run, focus, held);
    #[cfg(test)]
    if back {
        WENT.with(|c| c.set(c.get() + 1));
    }
    let _restore = Restore(BACK.with(|c| c.replace(back)));
    f()
}

fn knight<'a>(run: &'a Run, focus: Option<u32>) -> Option<&'a crate::rules::Hero> {
    focus.and_then(|id| run.players.get(&id)).or_else(|| run.players.values().next())
}

fn near(run: &Run, focus: Option<u32>, tiles: f32) -> bool {
    let (ox, oz) = works::origin_px();
    knight(run, focus).is_some_and(|h| (h.x * UNIT - ox).hypot(h.y * UNIT - oz) <= tiles * 16.0)
}

pub fn wanted(run: &Run, focus: Option<u32>, held: bool) -> bool {
    works::on_map()
        && run.room().kind == RoomKind::Kingdom
        && run.sites.keep().is_some_and(|s| s.castle().is_some())
        && (held || run.site_replay.is_some())
        && near(run, focus, 30.0)
}

pub fn origin() -> (i32, i32) {
    let (ox, oz) = works::origin_px();
    ((ox - (NATIVE_W * ZOOM) as f32 / 2.0 + 160.0) as i32, (oz - (NATIVE_H * ZOOM) as f32 * 0.62) as i32)
}

fn shrink(im: &Img, (ox, oy): (i32, i32)) -> Img {
    let mut out = Img::new(NATIVE_W, NATIVE_H);
    let k = ZOOM;
    for y in 0..NATIVE_H {
        for x in 0..NATIVE_W {
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for j in 0..k {
                for i in 0..k {
                    if let Some(c) = im.get(ox + x * k + i, oy + y * k + j).filter(|_| im.solid(ox + x * k + i, oy + y * k + j)) {
                        for (s, v) in sum.iter_mut().zip(c) {
                            *s += u32::from(v);
                        }
                        n += 1;
                    }
                }
            }
            if n * 2 >= (k * k) as u32 {
                out.set(x, y, [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]);
            }
        }
    }
    out
}

pub(super) fn flat(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let (ox, oy) = origin();
    let scenery = shrink(&super::super::kingdom::atlas(&run.kingdom.wishes), (ox, oy));
    let mut over = Img::new(NATIVE_W, NATIVE_H);
    if let Some(site) = run.sites.keep()
        && let Some(castle) = site.castle()
    {
        use super::super::keep_flat;
        let shown = run.site_shown(site);
        let flat = if site.bands.is_empty() { keep_flat::flat_gapped(castle, shown, &site.lost) } else { keep_flat::flat_banded(castle, shown, &site.lost, &site.bands) };
        over.stamp(&shrink(&flat.img, (ox - flat.at.0, oy - flat.at.1)), 0, 0);
        let built = if run.site_replay.is_some() { shown } else { run.hands.yard.timber };
        let work = keep_flat::falsework_built(castle, shown, built, (run.crane_seconds() * crate::rules::HZ as f32) as u32);
        over.stamp(&shrink(&work, (ox - flat.at.0, oy - flat.at.1)), 0, 0);
    }
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp > 0) {
        let (x, y) = (((hero.x * UNIT) as i32 - ox) / ZOOM, ((hero.y * UNIT) as i32 - oy) / ZOOM);
        over.rect(x - 1, y - 5, 3, 5, 'H');
        over.rect(x - 1, y - 4, 3, 3, if Some(id) == focus { 'o' } else { super::party::seat_ink(id) });
    }
    corner(&mut over, run, focus);
    (scenery, over)
}

pub const HINT_TICKS: u64 = 12 * crate::rules::HZ as u64;

std::thread_local! {
    static HINT: std::cell::Cell<(Option<u64>, bool)> = const { std::cell::Cell::new((None, false)) };
}

fn hint_up(run: &Run) -> bool {
    HINT.with(|h| {
        let (first, used) = h.get();
        if on() {
            h.set((first.or(Some(run.tick)), true));
            return false;
        }
        let first = first.filter(|&at| at <= run.tick).unwrap_or(run.tick);
        h.set((Some(first), used));
        !used && run.tick.saturating_sub(first) < HINT_TICKS
    })
}

const CORNER: (i32, i32) = (5, 5);

fn corner_lines(run: &Run, focus: Option<u32>) -> Vec<(String, char)> {
    let mut lines = vec![(super::kingdom::WORKS_LABEL.to_string(), 'y')];
    if !works::on_map() || !near(run, focus, 40.0) {
        return lines;
    }
    let Some(site) = run.sites.keep() else { return lines };
    let Some(castle) = site.castle() else { return lines };
    let shown = run.site_shown(site);
    let stage = if shown >= castle.len() {
        "THE KEEP STANDS".to_string()
    } else {
        short(&castle.stages[castle.stage_at(shown)].name)
    };
    let count = format!("KEEP {}/{}", shown, castle.len());
    if hint_up(run) {
        lines.push((format!("{count}  {stage}"), 'h'));
        lines.push(("HOLD N TO STAND BACK".to_string(), 'j'));
    } else {
        lines.push((count, 'h'));
        lines.push((stage, 'G'));
    }
    lines
}

pub(super) fn corner_rect(run: &Run, focus: Option<u32>) -> (i32, i32, i32, i32) {
    let lines = corner_lines(run, focus);
    let w = lines.iter().map(|(t, _)| super::super::ink::text_width(t)).max().unwrap_or(0) + 7;
    (CORNER.0, CORNER.1, w, lines.len() as i32 * 9 + 3)
}

pub(super) fn corner(into: &mut Img, run: &Run, focus: Option<u32>) {
    if super::kingdom::hillside_art::ledger_open(run, focus) {
        return;
    }
    let lines = corner_lines(run, focus);
    let rect = corner_rect(run, focus);
    into.rect(rect.0, rect.1, rect.2, rect.3, 'K');
    super::kingdom::hud_log::note("corner", rect);
    for (i, (text, ink)) in lines.iter().enumerate() {
        into.text(8, 8 + i as i32 * 9, text, *ink);
    }
    if works::on_map() && near(run, focus, 40.0) {
        super::kingdom::hands_art::hud(into, run, focus);
    }
}

fn short(name: &str) -> String {
    name.replace("Great tower", "Keep")
        .replace("Curtain wall and gatehouse", "Curtain")
        .replace("parapets and battlements", "parapets")
        .replace(": ", " ")
        .to_uppercase()
        .chars()
        .take(18)
        .collect()
}

pub(super) fn screen(run: &Run, focus: Option<u32>) -> Img {
    let mut screen = Img::new(NATIVE_W, NATIVE_H);
    corner(&mut screen, run, focus);
    screen
}
