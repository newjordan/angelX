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

/// How many times the native view the stand-back camera takes in.
pub const ZOOM: i32 = 3;

std::thread_local! {
    static BACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
std::thread_local! {
    /// How many views this thread has drawn stood back (for the tests of the page's draw path).
    pub(crate) static WENT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Whether the view being drawn stands back.
pub(crate) fn on() -> bool {
    BACK.with(|c| c.get())
}

/// How many times the native view the view being drawn takes in: [`ZOOM`]
/// standing back, 1 else.
pub(crate) fn zoom() -> i32 {
    if on() { ZOOM } else { 1 }
}

/// `f` drawing a seat's view, standing back if the seat holds the map key at
/// the site (`held`) or the Chronicle plays there.
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

/// The knight a seat looks through: its focus, or the party's first.
fn knight<'a>(run: &'a Run, focus: Option<u32>) -> Option<&'a crate::rules::Hero> {
    focus.and_then(|id| run.players.get(&id)).or_else(|| run.players.values().next())
}

/// Whether the knight is within `tiles` of the site's middle.
fn near(run: &Run, focus: Option<u32>, tiles: f32) -> bool {
    let (ox, oz) = works::origin_px();
    knight(run, focus).is_some_and(|h| (h.x * UNIT - ox).hypot(h.y * UNIT - oz) <= tiles * 16.0)
}

/// Whether this seat stands back: at the Keep, with the map key held or the
/// Chronicle playing.
pub fn wanted(run: &Run, focus: Option<u32>, held: bool) -> bool {
    works::on_map()
        && run.room().kind == RoomKind::Kingdom
        && run.sites.keep().is_some_and(|s| s.castle().is_some())
        && (held || run.site_replay.is_some())
        && near(run, focus, 30.0)
}

/// Where the stand-back window's top left lies, the kingdom's pixels: the
/// castle's middle in the window's, the keep's tall front toward the top.
pub fn origin() -> (i32, i32) {
    let (ox, oz) = works::origin_px();
    ((ox - (NATIVE_W * ZOOM) as f32 / 2.0 + 160.0) as i32, (oz - (NATIVE_H * ZOOM) as f32 * 0.62) as i32)
}

/// A window of `im` (its top left at `(ox, oy)` on `im`), `NATIVE` pixels,
/// each pixel the mean of the `ZOOM` x `ZOOM` block of the picture under it
/// that is drawn; a pixel mostly empty is empty.
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

/// The flat view stood back: the map and the castle on it as of now, the
/// knights small, the readout. As `kingdom::paint` returns a view: the
/// scenery and what lies over it.
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

/// How long the stand-back hint stays up from the first time a seat sees the Keep's readout:
/// long enough to read, short enough that it is never furniture.
pub const HINT_TICKS: u64 = 12 * crate::rules::HZ as u64;

std::thread_local! {
    /// What this seat has seen of the hint: the tick it first saw the readout, and whether it has
    /// stood back since (the hint is for the first time only, and goes once the key is used). A seat
    /// is a client, one thread; the window and the page draw one seat each.
    static HINT: std::cell::Cell<(Option<u64>, bool)> = const { std::cell::Cell::new((None, false)) };
}

/// Whether the stand-back hint is up for the seat being drawn.
fn hint_up(run: &Run) -> bool {
    HINT.with(|h| {
        let (first, used) = h.get();
        if on() {
            h.set((first.or(Some(run.tick)), true));
            return false;
        }
        // (A run begun again has a lower tick: the seat has not seen this one.)
        let first = first.filter(|&at| at <= run.tick).unwrap_or(run.tick);
        h.set((Some(first), used));
        !used && run.tick.saturating_sub(first) < HINT_TICKS
    })
}

/// Where the corner block's plate begins, the view's top left.
const CORNER: (i32, i32) = (5, 5);

/// The lines of the corner block, top to bottom: the realm's label; near the Keep, how far
/// it has risen and its stage; and, for the first seconds after coming up, how to stand back.
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
        // Three lines either way: the plate must not reach the tally board standing below it.
        lines.push((format!("{count}  {stage}"), 'h'));
        lines.push(("HOLD N TO STAND BACK".to_string(), 'j'));
    } else {
        lines.push((count, 'h'));
        lines.push((stage, 'G'));
    }
    lines
}

/// The corner block's rectangle `(x, y, w, h)` in the view's pixels: what the goal's arrow
/// and its name, and every floating mark, keep clear of.
pub(super) fn corner_rect(run: &Run, focus: Option<u32>) -> (i32, i32, i32, i32) {
    let lines = corner_lines(run, focus);
    let w = lines.iter().map(|(t, _)| super::super::ink::text_width(t)).max().unwrap_or(0) + 7;
    (CORNER.0, CORNER.1, w, lines.len() as i32 * 9 + 3)
}

/// The surface's top left, in one place: the realm's label and, near the Keep, its progress
/// readout, on one plate of the realm's black so it reads over stone and over grass; and the
/// knight's trade plate at the view's foot. Nothing else is drawn in that corner.
pub(super) fn corner(into: &mut Img, run: &Run, focus: Option<u32>) {
    // The ledger covers the view: nothing of the corner peeks out from under it.
    if super::kingdom::hillside_art::ledger_open(run, focus) {
        return;
    }
    let lines = corner_lines(run, focus);
    let rect = corner_rect(run, focus);
    // (Always on its plate: far from the Keep the label alone would lie on whatever the ground holds.)
    into.rect(rect.0, rect.1, rect.2, rect.3, 'K');
    super::kingdom::hud_log::note("corner", rect);
    for (i, (text, ink)) in lines.iter().enumerate() {
        into.text(8, 8 + i as i32 * 9, text, *ink);
    }
    if works::on_map() && near(run, focus, 40.0) {
        super::kingdom::hands_art::hud(into, run, focus);
    }
}

/// A stage's name as short as a line in the corner takes it: the great tower
/// the keep, the curtain wall and gatehouse the curtain, the parapets and
/// battlements the parapets.
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

/// The screen's marks of a view stood back: the realm's label and the
/// readout, no more (the goal's arrow and the plaques are the knight's own
/// view's, which the wider window does not keep).
pub(super) fn screen(run: &Run, focus: Option<u32>) -> Img {
    let mut screen = Img::new(NATIVE_W, NATIVE_H);
    corner(&mut screen, run, focus);
    screen
}
