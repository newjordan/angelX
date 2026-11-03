//! The camera: where a view of the room starts, kept inside the room, and
//! what it takes to place a point in that view.

use super::*;

std::thread_local! {
    static WATCH: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// A seat's camera choice for this drawing only; its HUD keeps its owner.
pub(crate) fn with_watch<T>(watching: Option<u32>, f: impl FnOnce() -> T) -> T {
    struct Back(Option<u32>);
    impl Drop for Back {
        fn drop(&mut self) { WATCH.with(|c| c.set(self.0)); }
    }
    let _back = Back(WATCH.with(|c| c.replace(watching)));
    f()
}

/// Where the camera's view starts in a room's native pixels: centred on
/// `focus` (or between the living knights), kept inside the room. An
/// ordinary room is exactly one view, so its camera never moves.
pub fn camera(run: &Run, focus: Option<u32>) -> (i32, i32) {
    let (x, y) = camera_exact(run, focus);
    (x.floor() as i32, y.floor() as i32)
}

/// The camera between whole pixels: `camera` is its whole pixel. A window
/// that draws the room itself scrolls by this, so the room glides under a
/// knight it follows instead of stepping a pixel at a time.
pub fn camera_exact(run: &Run, focus: Option<u32>) -> (f32, f32) {
    let (pw, ph) = room_px(run.room());
    let corner = |(cx, cy): (f32, f32)| {
        (
            (cx * UNIT - NATIVE_W as f32 / 2.0).clamp(0.0, (pw - NATIVE_W).max(0) as f32),
            (cy * UNIT - NATIVE_H as f32 / 2.0).clamp(0.0, (ph - NATIVE_H).max(0) as f32),
        )
    };
    // A village of the Deep looks up at the dark while the star falls.
    if let Some(sky) = run.star_camera() {
        return corner(sky);
    }
    if let Some(hero) = WATCH.with(|c| c.get()).and_then(|id| run.players.get(&id)) {
        return corner(if run.room().kind == RoomKind::Kingdom {
            hero.camera.unwrap_or((hero.x, hero.y - 1.0))
        } else { (hero.x, hero.y) });
    }
    if run.room().kind == RoomKind::Kingdom {
        let hero = focus
            .and_then(|id| run.players.get(&id))
            .or_else(|| run.players.get(&1))
            // A host who plays no knight sends none (`Hero::absent`): the
            // party's first.
            .or_else(|| run.players.values().next());
        let centre = hero
            .map(|h| h.camera.unwrap_or((h.x, h.y - 1.0)))
            .unwrap_or((WIDTH / 2.0, HEIGHT / 2.0));
        return corner(centre);
    }
    let alive = |h: &&crate::rules::Hero| h.hp > 0;
    let watched: Vec<(f32, f32)> = match focus.and_then(|id| run.players.get(&id)).filter(alive) {
        Some(hero) => vec![(hero.x, hero.y)],
        None => run
            .players
            .values()
            .filter(alive)
            .map(|h| (h.x, h.y))
            .collect(),
    };
    // In the Wanted hall the view takes in the outlaw too.
    let mut watched = watched;
    if run.room().kind == RoomKind::Wanted
        && let Some(e) = run.enemies.iter().find(|e| e.boss.is_some() && e.hp > 0)
    {
        watched.push((e.x, e.y));
    } else if run.room().kind == RoomKind::Wanted
        && let Some(fall) = run.works.outlaw.fall
        && run.tick.saturating_sub(fall.at)
            < u64::from(crate::rules::outlaw::FALL)
    {
        // The view stays on the outlaw while it falls.
        watched.push((fall.x, fall.y));
    }
    let (sx, sy) = watched
        .iter()
        .fold((0.0, 0.0), |(ax, ay), &(x, y)| (ax + x, ay + y));
    let n = watched.len().max(1) as f32;
    if watched.is_empty() {
        return corner(((pw / 2) as f32 / UNIT, (ph / 2) as f32 / UNIT));
    }
    corner((sx / n, sy / n))
}

/// One view of `full` from `(ox, oy)`.
pub(super) fn crop(full: &Img, (ox, oy): (i32, i32)) -> Img {
    Img::from_view(full, ox, oy, NATIVE_W, NATIVE_H)
}

/// What the view cannot show: a mark on its edge toward each monster (red)
/// and each knight (their colours) out of sight in a great hall. The mark is
/// an arrowhead pointing out, so it reads as a pointer and never as a small
/// monster at the edge of the room.
/// A gold arrow at `(ax, ay)` with its tip toward `(ux, uy)`, outlined so it
/// reads on any ground, and its two- or three-word label just above or
/// below it (`edge`: pinned to the view's edge, so the label goes toward
/// the middle).
pub(in super::super) fn pointer(
    over: &mut Img,
    (ax, ay): (f32, f32),
    (ux, uy): (f32, f32),
    name: &str,
    edge: bool,
) -> Vec<(i32, i32, i32, i32)> {
    for (ink, grow) in [('K', 1.5), ('O', 0.0)] {
        for py in -10..=10 {
            for px in -10..=10 {
                let along = px as f32 * ux + py as f32 * uy;
                let across = (py as f32 * ux - px as f32 * uy).abs();
                if (-5.0 - grow..=7.0 + grow).contains(&along)
                    && across <= (7.0 + grow - along) * 0.5 + grow * 0.5
                {
                    over.put(ax.round() as i32 + px, ay.round() as i32 + py, ink);
                }
            }
        }
    }
    let label = name.to_uppercase();
    let w = super::super::ink::text_width(&label);
    let lx = (ax.round() as i32 - w / 2).clamp(4, NATIVE_W - w - 4);
    let ly = if edge && ay < (NATIVE_H / 2) as f32 || !edge && uy < -0.5 {
        ay.round() as i32 + 13
    } else {
        ay.round() as i32 - 21
    }
    .clamp(4, NATIVE_H - 13);
    let tag = chrome::label(over, (lx, ly), &label, 'O', chrome::Tone::KEY);
    vec![tag, (ax as i32 - 10, ay as i32 - 10, 21, 21)]
}

/// Where a box `w` wide over room pixel `x` starts: centred on `x`, kept
/// inside the seat's view (`view`, its top-left; the view `NATIVE_W` wide)
/// while `x` is in sight, and inside the room (`room_w` wide) always. A
/// name or a word over a figure near the view's edge stays whole on that
/// seat's screen; one over a figure out of sight stays with the figure.
pub(in super::super) fn in_view_x(x: i32, w: i32, view: (i32, i32), room_w: i32) -> i32 {
    let mut x0 = x - w / 2;
    if (view.0..view.0 + NATIVE_W).contains(&x) && w + 6 <= NATIVE_W {
        x0 = x0.clamp(view.0 + 3, view.0 + NATIVE_W - w - 3);
    }
    x0.clamp(0, (room_w - w).max(0))
}

/// As `in_view_x`, for the row a box `h` tall starts on over room pixel
/// `y`: never above the seat's view while `y` is in sight.
pub(in super::super) fn in_view_y(y0: i32, y: i32, view: (i32, i32)) -> i32 {
    if (view.1..view.1 + NATIVE_H).contains(&y) {
        y0.max(view.1 + 3)
    } else {
        y0
    }
}

/// One camera origin shared by both art pipelines, including hit shake.
pub fn view_origin(run: &Run, focus: Option<u32>) -> (i32, i32) {
    let origin = camera(run, focus);
    // A best finisher's kick rides the picture alone (`rules::gore`).
    let shake = run.shake.max(run.gore_kick());
    let (x, y) = smooth_shake(run.tick, u32::from(shake), 12, 6.0);
    (origin.0 + x, origin.1 + y)
}

/// Picture-only impact motion. The existing countdown is the trauma: small
/// blows barely move, and the last ticks settle instead of buzzing.
pub(super) fn smooth_shake(tick: u64, left: u32, full: u32, reach: f32) -> (i32, i32) {
    if crate::render::viewer::calm() { return (0, 0); }
    crate::motion::shake(tick, left, full, reach)
}
