//! Viewport-only painting for the continuous surface.
use super::super::{kingdom as map, kit};
use super::*;

// The run's finds out in the open (declared here, not in `arena`, whose
// source the arcade cell gate fingerprints).
#[path = "finds.rs"]
mod finds;
// The signs of the ways down (`crate::rules::world::MOUTHS`).
#[path = "mouth.rs"]
mod mouth;
// The waystations beside the roads (`crate::world::waystations`).
#[path = "waystation.rs"]
mod waystation;
// The Keep's masons and hodmen, and its tally board.
#[path = "crew.rs"]
mod crew;
// The trades' tools in a knight's hand, and the work at the Keep drawn: the
// teachers, the outcrop, the gauges over a knight at a job.
#[path = "tools.rs"]
pub(super) mod tools;
#[path = "hands_art.rs"]
pub(super) mod hands_art;
// The hillside works: buildings, roads, lines, carts, people, the ghost, the flow overlay.
#[path = "hillside_art.rs"]
pub(super) mod hillside_art;
// The travellers on the roads, and the talk with one.
#[path = "wayfarers.rs"]
pub(super) mod wayfarers;
// The places of the knight's missions, beside the doors.
#[path = "missions.rs"]
pub(super) mod missions;
// The Great Shaft over the Delve's gate and its stockyard.
#[path = "shaft.rs"]
pub(super) mod shaft;
use crate::rules::kingdom::Goal;

/// The rectangles the surface's HUD lays down in a frame, by name, read back by the layout test
/// (`the_hud_never_overlaps_at_any_frame`). Nothing is kept outside a test build.
pub(super) mod hud_log {
    pub type Rect = (i32, i32, i32, i32);

    #[cfg(test)]
    std::thread_local! {
        static LOG: std::cell::RefCell<Option<Vec<(&'static str, Rect)>>> = const { std::cell::RefCell::new(None) };
    }

    /// `f`, and every box the HUD noted while it ran.
    #[cfg(test)]
    pub fn read<R>(f: impl FnOnce() -> R) -> (R, Vec<(&'static str, Rect)>) {
        let was = LOG.with(|l| l.borrow_mut().replace(Vec::new()));
        let out = f();
        let boxes = LOG.with(|l| std::mem::replace(&mut *l.borrow_mut(), was)).unwrap_or_default();
        (out, boxes)
    }

    /// The HUD lays a box down: `rect` in the view's pixels.
    #[inline]
    pub fn note(_name: &'static str, _rect: Rect) {
        #[cfg(test)]
        LOG.with(|l| {
            if let Some(log) = l.borrow_mut().as_mut() {
                log.push((_name, _rect));
            }
        });
    }
}

pub(super) fn layers_with(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    paint(run, focus, legible, None)
}

/// What a window that stands the kingdom up in 3D (`world3d::kingdom`)
/// draws with its chunks: what lies on the floor and moves or marks it, as
/// `layers_with` paints it but for the land and the knights (view pixels);
/// the screen's own marks (the biome's name in its corner, the goal's
/// arrow, the boards and the door's footer), which stay put as the floor
/// tips; and each living knight with its shadow, a figure standing in the
/// world.
pub(super) fn layers_3d(run: &Run, focus: Option<u32>) -> (Img, Img, Vec<Figure>) {
    let mut three = Three { screen: Img::new(NATIVE_W, NATIVE_H), figures: Vec::new() };
    let (_, over) = paint(run, focus, true, Some(&mut three));
    // The ballot panel lies on the screen, as `arena::layers_with` lays it
    // over the view.
    votes::panel(&mut three.screen, run, focus);
    (over, three.screen, three.figures)
}

/// The Keep's crew and tally as figures over a window of `size` at `origin`
/// (the kingdom's pixels), for a view stood back (`stand_back`): `paint` draws
/// them only in the knight's own view.
pub(super) fn crew_in(figures: &mut Vec<Figure>, run: &Run, origin: (i32, i32), size: (i32, i32)) {
    crew::draw(&mut Img::new(1, 1), run, origin, size, Some(figures));
    hands_art::props(&mut Img::new(1, 1), run, origin, size, Some(figures));
    hands_art::work(&mut Img::new(1, 1), run, origin, size, Some(figures), None);
}

/// Where `paint` puts what a window in 3D draws apart.
struct Three {
    screen: Img,
    figures: Vec<Figure>,
}

/// A knight as a figure of the kingdom in 3D: its contact shadow and its
/// sprite where `paint` lays them, on a picture of their own, its feet its
/// depth.
fn knight_figure(im: &Img, hero: &crate::rules::Hero, book: &crate::rules::cards::Book, lift: f32) -> Figure {
    let (hx, hy) = (hero.x * UNIT, hero.y * UNIT);
    let (fx, fy) = (hx.floor() as i32, hy.floor() as i32);
    let (sx, sy) = (hx - knights::rig_centre(hero, book, im.w), hy - im.h as f32 + 1.0 + knights::rig_floor_offset(hero, book) as f32);
    let (left, top) = (sx.floor() as i32, sy.floor() as i32);
    let (x0, y0) = (left.min(fx - 5), top.min(fy));
    let (x1, y1) = ((left + im.w + 1).max(fx + 6), (top + im.h + 1).max(fy + 2));
    let mut img = Img::new(x1 - x0, y1 - y0);
    knights::shadow(&mut img, fx - x0, fy - y0);
    img.stamp_sub(im, sx - x0 as f32, sy - y0 as f32);
    Figure { img, x: x0, y: y0, frac: (0.0, 0.0), depth: hero.y, height: lift, additive: false, bodies: Vec::new(), foe: false, paints: Vec::new() }
}

/// The Keep's site as two pictures of the view: what lies behind the focused
/// knight's feet (laid under the knights) and what lies before them and the
/// falsework (laid over them); `None` while it is out of view.
fn site_layers(run: &Run, origin: (i32, i32), focus: Option<u32>) -> Option<(Img, Img)> {
    use super::super::keep_flat;
    if !crate::world::works::on_map() {
        return None;
    }
    let site = run.sites.keep()?;
    let castle = site.castle()?;
    if !keep_flat::in_view(castle, origin, (NATIVE_W, NATIVE_H)) {
        return None;
    }
    let hero = focus.and_then(|id| run.players.get(&id)).or_else(|| run.players.values().next());
    let feet = hero.map_or(f32::MAX, |h| h.y * UNIT);
    let knight = hero.map(|h| ((h.x * UNIT) as i32 - origin.0, (h.y * UNIT) as i32 - origin.1));
    let shown = run.site_shown(site);
    let flat = if site.bands.is_empty() { keep_flat::flat_gapped(castle, shown, &site.lost) } else { keep_flat::flat_banded(castle, shown, &site.lost, &site.bands) };
    let (back, mut front) = flat.split(origin, (NATIVE_W, NATIVE_H), feet, knight);
    let built = if run.site_replay.is_some() { shown } else { run.hands.yard.timber };
    let work = keep_flat::falsework_built(castle, shown, built, (run.crane_seconds() * crate::rules::HZ as f32) as u32);
    front.stamp(&work, flat.at.0 - origin.0, flat.at.1 - origin.1);
    Some((back, front))
}

fn paint(run: &Run, focus: Option<u32>, legible: bool, mut three: Option<&mut Three>) -> (Img, Img) {
    // Stood back at the Keep, the flat view is the map shrunk (`stand_back`).
    if three.is_none() && super::stand_back::on() {
        return super::stand_back::flat(run, focus);
    }
    let origin = camera(run, focus);
    let scenery = if three.is_some() {
        Img::new(NATIVE_W, NATIVE_H)
    } else {
        crop(&map::atlas(&run.kingdom.wishes), origin)
    };
    let mut over = Img::new(NATIVE_W, NATIVE_H);
    // (On the blank slate the label and the Keep's readout are one block, the screen's, laid
    // last over everything of the world, with the goal's arrow: `stand_back::corner`, below.
    // Elsewhere the label is laid early, as ever.)
    if !crate::world::plan::tabula() {
        over.text(8, 8, WORKS_LABEL, 'y');
    }
    super::forest_road::kingdom(&mut over, run, origin);
    super::roundtable::kingdom(&mut over, run, origin);
    let geo = map::geography(&run.kingdom.wishes);
    super::squires_lodge::town(&mut over, run, origin);
    // Region names are lettered on the ground, under everything that moves.
    let lettered = map::letter_regions(&mut over, origin);
    // So are the waystations: a knight walks in front of the counter.
    waystation::draw(&mut over, run, origin);
    // The shaft head over the Delve's gate and the stockyard beside it.
    shaft::paint(&mut over, run, origin);
    // The hillside works (`world::hillside`): in the flat view the roads and tracks, and
    // the buildings behind the knight, laid on the ground; the buildings before them
    // go over the knights (below).
    let hill_view = crate::world::hillside::of(run);
    let mut hill_front = None;
    if let Some(hv) = hill_view.as_ref().filter(|_| three.is_none()) {
        let feet = focus.and_then(|id| run.players.get(&id)).or_else(|| run.players.values().next()).map_or(f32::MAX, |h| h.y * UNIT);
        over.stamp(&hillside_art::ground(run, hv, origin, (NATIVE_W, NATIVE_H)), 0, 0);
        let (back, front) = hillside_art::buildings(run, hv, origin, (NATIVE_W, NATIVE_H), feet);
        over.stamp(&back, 0, 0);
        hill_front = Some(front);
    }
    if let Some(hv) = &hill_view {
        hillside_art::movers(&mut over, run, hv, origin, (NATIVE_W, NATIVE_H), three.as_deref_mut().map(|t| &mut t.figures));
    }
    if let Some(hv) = &hill_view {
        hillside_art::plans(&mut over, run, hv, origin, (NATIVE_W, NATIVE_H), three.is_none());
    }
    hillside_art::ghost(&mut over, run, origin, focus, three.is_none());
    let tick = run.tick as u32;
    let local = |x: f32, y: f32| {
        let (x, y) = at(x, y);
        (x - origin.0, y - origin.1)
    };
    // The blank-slate plan (`crate::world::plan`) has no chimneys, no roads for
    // walkers and no birds over its hillside: just the camps' fires and the
    // knights.
    let populated = crate::world::plan::full();
    for (i, site) in geo.sites.iter().enumerate().step_by(4).filter(|_| populated) {
        // Smoke only rises where there is a chimney.
        if site.lot.is_some() && !super::super::landmarks::chimney(site.room) {
            continue;
        }
        let (x, y) = (
            site.stand.0 * TILE - origin.0,
            site.stand.1 * TILE - origin.1,
        );
        if x < -80 || y < -80 || x > NATIVE_W + 80 || y > NATIVE_H + 80 {
            continue;
        }
        over.stamp(&kit::smoke(tick + i as u32 * 9), x + 22, y - 49);
    }
    // Twelve road walkers, each on a short dry approach, and eight birds.
    for i in (0..12).filter(|_| populated) {
        let site = &geo.sites[i * 3 % geo.sites.len()];
        let mut route = vec![site.stand];
        let mut point = site.stand;
        for _ in 0..8 {
            if let Some(next) = [
                (point.0 - 1, point.1),
                (point.0 + 1, point.1),
                (point.0, point.1 + 1),
                (point.0, point.1 - 1),
            ]
            .into_iter()
            .find(|&(x, y)| {
                x >= 0
                    && y >= 0
                    && x < super::super::MAP_W
                    && y < super::super::MAP_H
                    && geo.roads[(y * super::super::MAP_W + x) as usize]
                    && !route.contains(&(x, y))
            }) {
                route.push(next);
                point = next;
            } else {
                break;
            }
        }
        let span = route.len().saturating_sub(1).max(1) as u32 * 32;
        let phase = (tick + i as u32 * 31) % (span * 2);
        let along = if phase > span {
            span * 2 - phase
        } else {
            phase
        };
        let a = ((along / 32) as usize).min(route.len() - 1);
        let b = (a + 1).min(route.len() - 1);
        let t = (along % 32) as f32 / 32.0;
        let (x, y) = local(
            (route[a].0 as f32 + (route[b].0 - route[a].0) as f32 * t + 0.5) * TILE_UNITS,
            (route[a].1 as f32 + (route[b].1 - route[a].1) as f32 * t + 0.8) * TILE_UNITS,
        );
        if x >= -16 && y >= -16 && x < NATIVE_W + 16 && y < NATIVE_H + 16 {
            over.stamp(&kit::villager(tick / 8, false), x - 8, y - 16);
        }
    }
    for i in (0..8).filter(|_| populated) {
        let x = ((tick as i32 * 2 + i * 233).rem_euclid(super::super::MAP_W * TILE)) - origin.0;
        let y = ((i * 251 + tick as i32 / 5).rem_euclid(super::super::MAP_H * TILE)) - origin.1;
        if x >= -16 && y >= -16 && x < NATIVE_W + 16 && y < NATIVE_H + 16 {
            over.stamp(&kit::raven(tick / 8, false), x, y);
        }
    }
    let visible =
        |x: i32, y: i32, r: i32| x >= -r && y >= -r && x < NATIVE_W + r && y < NATIVE_H + r;
    for b in &run.works.blazes {
        let (x, y) = local(b.x, b.y);
        if !visible(x, y, 48) {
            continue;
        }
        over.ellipse(
            x as f32,
            y as f32,
            b.radius * UNIT,
            b.radius * UNIT * 0.5,
            'p',
        );
        over.stamp(&sprites::brazier(true, tick, 7), x - 8, y - 18);
    }
    for c in &run.works.clouds {
        let (x, y) = local(c.x, c.y);
        if visible(x, y, 48) {
            over.ellipse(
                x as f32,
                y as f32,
                16.0,
                9.0,
                if c.tint == 0 { 'E' } else { 'O' },
            );
        }
    }
    for m in &run.works.mines {
        let (x, y) = local(m.x, m.y);
        if visible(x, y, 12) {
            over.rect(x - 3, y - 2, 7, 4, 'G');
            over.put(x, y - 3, if m.armed() { '7' } else { '5' });
        }
    }
    for b in &run.works.barricades {
        let (x, y) = local(b.a.0, b.a.1);
        let (bx, by) = local(b.b.0, b.b.1);
        if visible(x, y, 80) || visible(bx, by, 80) {
            over.line(x, y, bx, by, 'B');
            over.line(x, y - 4, bx, by - 4, 'r');
        }
    }
    for sphere in &run.spheres {
        let (x, y) = local(sphere.x, sphere.y);
        if visible(x, y, 80) {
            let r = sphere.r * UNIT;
            for k in 0..64 {
                let a = k as f32 * std::f32::consts::TAU / 64.0;
                over.put(
                    x + (a.cos() * r) as i32,
                    y + (a.sin() * r * 0.8) as i32,
                    '2',
                );
            }
        }
    }
    for strike in &run.strikes {
        let (x, y) = local(strike.x, strike.y);
        if visible(x, y, 40) {
            for k in 0..24 {
                let a = k as f32 * std::f32::consts::TAU / 24.0;
                over.put(x + (a.cos() * 14.0) as i32, y + (a.sin() * 8.0) as i32, '7');
            }
        }
    }
    for phantom in &run.phantoms {
        let (x, y) = local(phantom.x, phantom.y);
        if visible(x, y, 24)
            && let Some(h) = run.players.get(&phantom.owner)
        {
            let im = ults::iced(&knights::figure_with(h, phantom.owner, false));
            over.stamp(&im, x - im.w / 2, y - im.h);
        }
    }
    // The Keep's site, in the flat view: the part of the castle behind the
    // knight's feet before the knights, the rest after (`keep_flat`).
    let site = three.is_none().then(|| site_layers(run, origin, focus)).flatten();
    if let Some((back, _)) = &site {
        over.stamp(back, 0, 0);
    }
    // The Keep's fixtures, under the knights who walk over them.
    if !super::stand_back::on() {
        hands_art::props(&mut over, run, origin, (NATIVE_W, NATIVE_H), three.as_deref_mut().map(|t| &mut t.figures));
    }
    // The road's travellers stand behind the knights who come up to them.
    let road = wayfarers::bodies(&mut over, run, origin, focus, three.as_deref_mut().map(|t| &mut t.figures));
    for (&id, hero) in &run.players {
        let (x, y) = local(hero.x, hero.y);
        if x < -24 || y < -24 || x > NATIVE_W + 24 || y > NATIVE_H + 24 {
            continue;
        }
        let bodied = hero.shape.is_some() && hero.making(&run.book).is_some_and(|m| m.parts.iter().any(|p| p == "body"));
        let im = if let Some(shape) = hero.shape {
            knights::identity_figure(hero, shape, bodied, run.shield_out(id), Default::default())
        } else if let Some(avatar) = &hero.avatar {
            knights::avatar_figure(hero, avatar.pixels_at(run.tick), run.shield_out(id))
        } else {
            knights::figure_at(hero, id, run.shield_out(id), Default::default(), false, run.tick)
        };
        // Seated, so a plain knight is dressed on their own design's sockets.
        let mut im = knights::dressed_event(&im, hero, &run.book, run.tick, 0, None, run.shield_out(id), Default::default(), Some(id)).0;
        if let Some(shape) = hero.shape.filter(|_| bodied) {
            knights::emblem(&mut im, shape);
        }
        // A look or an identity that is not the code rig carries its tool in a
        // hand of its own: held at the side, up.
        if !knights::rig_drawn(hero, id)
            && let Some((tool, load)) = tools::of(hero.tool)
        {
            let hand = (im.w as f32 * 0.72, im.h as f32 * 0.55);
            tools::draw(&mut im, tool, load, hand, (0.75, -1.0), hero.swing > 0);
        }
        // A rough block carried up from the Delve, on the shoulders and
        // sagging a pixel at each step: the picture is made taller to hold it.
        if let Some(stone) = hero.block.as_deref() {
            let block = shaft::carried_img(stone);
            let pad = block.h + 2;
            let crown = (0..im.h).find(|&row| (0..im.w).any(|col| im.get(col, row).is_some())).unwrap_or(0);
            let mut tall = Img::new(im.w.max(block.w), im.h + pad);
            tall.stamp(&im, (tall.w - im.w) / 2, pad);
            let sag = (tick / 7 % 2) as i32;
            tall.stamp(&block, (tall.w - block.w) / 2, pad + crown - block.h + 11 + sag);
            im = tall;
        }
        if hero.aim_x < -0.3 {
            im = im.flip_h();
        }
        if hero.chilled > 0 {
            im = ults::iced(&im);
        }
        if hero.frog() {
            im = sprites::frog(tick);
        }
        if hero.sitting {
            im = works::crouched(&im);
        }
        // Tomatoes on a knight in the stocks.
        for k in 0..u32::from(hero.splats.min(12)) {
            let (sx, sy) = ((k * 7 + 3) % im.w as u32, (k * 5 + 2) % (im.h as u32 / 2));
            im.put(sx as i32, sy as i32 + 2, '7');
            im.put(sx as i32 + 1, sy as i32 + 2, 'p');
        }
        if hero.stone {
            im = sprites::stone(&im);
        }
        if let Some(left) = hero.rolling() {
            im = knights::rolled_hero(&im, hero, &run.book, left);
        }
        if hero.swing > 0 {
            let a = hero.aim_y.atan2(hero.aim_x);
            for k in 0..16 {
                let angle = a - 0.7 + k as f32 / 15.0 * 1.4;
                over.put(
                    x + (angle.cos() * 18.0) as i32,
                    y - 6 + (angle.sin() * 14.0) as i32,
                    '6',
                );
            }
        }
        // Up the scaffold with a load: the knight rises as they climb.
        let lift = run.hands.seats.get(&id).map_or(0.0, |h| h.climb() * 18.0);
        if hero.hp == 0 {
            bones::draw(&mut over, hero, id, run.tick, false, x, y, true);
        } else if let Some(three) = three.as_deref_mut() {
            three.figures.push(knight_figure(&im, hero, &run.book, lift));
        } else {
            knights::shadow(&mut over, x, y);
            over.stamp_sub(
                &im,
                hero.x * UNIT - origin.0 as f32 - knights::rig_centre(hero, &run.book, im.w),
                hero.y * UNIT - origin.1 as f32 - im.h as f32 + 1.0 + knights::rig_floor_offset(hero, &run.book) as f32 - lift,
            );
        }
    }
    if let Some((_, front)) = &site {
        over.stamp(front, 0, 0);
    }
    if let Some(front) = &hill_front {
        over.stamp(front, 0, 0);
    }
    // (Stood back, the crew is drawn with the wider view: `kingdom_scene`.)
    if !super::stand_back::on() {
        crew::draw(&mut over, run, origin, (NATIVE_W, NATIVE_H), three.as_deref_mut().map(|t| &mut t.figures));
        hands_art::work(&mut over, run, origin, (NATIVE_W, NATIVE_H), three.as_deref_mut().map(|t| &mut t.figures), focus);
    }
    for shot in &run.projectiles {
        let (x, y) = local(shot.x, shot.y);
        // A mark 3 pixels across at the old size, a knight's shot's size
        // (`wishcraft`) of it, never under 1.
        let side = (3.0 * shot.scale()).round().max(1.0) as i32;
        over.rect(x - side / 2, y - side / 2, side, side, '6');
    }
    for &(ux, uy, age) in &run.blasts {
        let (x, y) = local(ux, uy);
        if visible(x, y, 48) {
            let r = (10 - u32::from(age).min(9)) as f32 * 3.0;
            for k in 0..48 {
                let a = k as f32 * std::f32::consts::TAU / 48.0;
                over.put(
                    x + (a.cos() * r) as i32,
                    y + (a.sin() * r * 0.65) as i32,
                    '6',
                );
            }
        }
    }
    super::team::over_at(&mut over,run,origin);
    super::holdings::castle_pennons(&mut over, run, origin);
    beyond(&mut over, run, origin);
    let goal = run.kingdom_goal(focus.unwrap_or(1));
    let mut taken = Vec::new();
    taken.extend(lettered);
    taken.extend(road);
    taken.extend(trinkets(&mut over, run, &geo, origin, focus, legible));
    let mouths = mouth::draw(&mut over, run, &geo, origin);
    taken.extend(mouths.iter().copied());
    if legible {
        taken.extend(waystation::marks(&mut over, run, origin, focus));
    }
    // In 3D the finds and the herald's posts stand up as blocks
    // (`world3d::lots`): only who keeps them and their keys are drawn here.
    let stood = three.is_some();
    taken.extend(finds::draw(&mut over, run, origin, focus, legible, stood));
    if legible {
        taken.extend(wayfarers::plates(&mut over, run, &geo, origin, focus));
    }
    taken.extend(super::holdings::herald(&mut over, run, &geo, origin, focus, legible, stood));
    // The screen's own marks: over the view, or apart in 3D.
    let mut apart = three.as_deref_mut().map(|t| std::mem::replace(&mut t.screen, Img::new(0, 0)));
    macro_rules! ui {
        () => {
            match apart.as_mut() {
                Some(screen) => screen,
                None => &mut over,
            }
        };
    }
    super::team::panel(ui!(),run,focus);
    // The flow overlay's marks and their words, which the goal's arrow keeps clear of.
    let mut marks: Vec<(i32, i32, i32, i32)> = Vec::new();
    // The card of what the cursor points at is up: it has the view's side, and the goal's arrow stands down.
    let mut card_up: Option<(i32, i32, i32, i32)> = None;
    if let Some(hv) = &hill_view {
        marks = hillside_art::overlay(&mut over, three.as_deref_mut().map(|t| &mut t.figures), run, hv, origin, (NATIVE_W, NATIVE_H), focus);
        hillside_art::clerk(ui!(), run, hv, focus);
        hillside_art::hud(ui!(), run, focus);
        let plate = hillside_art::goal_plate(ui!(), run, focus);
        let corner = crate::world::plan::tabula().then(|| super::stand_back::corner_rect(run, focus));
        let mut clear: Vec<_> = plate.into_iter().chain(corner).collect();
        // The night plate yields to what already stands, then the card and the goal's arrow yield to it.
        let night = hillside_art::deep_plate(ui!(), run, focus, &clear);
        clear.extend(night);
        marks.extend(plate);
        marks.extend(night);
        card_up = hillside_art::card(ui!(), run, focus, origin, &clear);
        marks.extend(card_up);
        hillside_art::ledger(ui!(), run, focus);
    }
    survey_ghost(&mut over, run, origin, focus);
    survey_panel(ui!(), run, focus);
    // The goal's arrow and name go down first; plaques make way for them.
    let mut goal_layer = Img::new(NATIVE_W, NATIVE_H);
    let mut aimed = Vec::new();
    // The ledger lies over the world, and nothing floats over it.
    let ledger_open = hillside_art::ledger_open(run, focus);
    if let Some(goal) = goal.as_ref().filter(|_| !ledger_open && card_up.is_none()) {
        // The corner block is the goal's to keep clear of, as every floating mark does.
        let mut avoid: Vec<_> = crate::world::plan::tabula().then(|| super::stand_back::corner_rect(run, focus)).into_iter().collect();
        // And the signs of the ways down.
        avoid.extend(mouths.iter().copied());
        avoid.extend(marks.iter().copied());
        // While the builder is out its strip, and the Clerk's lines over it, hold the view's foot.
        let floor = if focus.is_some_and(|id| crate::rules::hillside::building(run, id)) { (NATIVE_H - hillside_art::STRIP_H - 2 - 38 - 12) as f32 } else { NATIVE_H as f32 };
        aimed = goal_marker(&mut goal_layer, goal, origin, tick, &avoid, floor);
    }
    // The knight's own missions, each at the door where it can be done.
    let marked = if ledger_open { Vec::new() } else { missions::doors(&mut goal_layer, run, focus.unwrap_or(1), origin, tick, &aimed) };
    for &rect in &aimed {
        hud_log::note("goal", rect);
    }
    for &rect in &marked {
        hud_log::note("door", rect);
    }
    taken.extend(aimed);
    taken.extend(marks.iter().copied());
    taken.extend(marked);
    if legible {
        let hero = run.players.get(&focus.unwrap_or(1)).map(|h| (h.x / TILE_UNITS, h.y / TILE_UNITS));
        // A fingerpost's arms read as the knight passes it.
        if let Some(near) = hero {
            taken.extend(super::super::signs::arms(&mut over, origin, near));
        }
        biome_title(ui!(), run, focus.unwrap_or(1), &taken);
        plaques(&mut over, &geo, origin, goal.as_ref(), hero, taken);
    }
    // The corner block: the label, the Keep's readout and the trade plate, over everything of the
    // world, under the goal's arrow, which keeps clear of it.
    if crate::world::plan::tabula() {
        super::stand_back::corner(ui!(), run, focus);
    }
    ui!().stamp(&goal_layer, 0, 0);
    // A keeper's talk lies over the goal's arrow, as it is what the knight reads.
    waystation::talk(ui!(), run, focus);
    // The Great Shaft's ledger, over the plaques, while a knight stands at its
    // winch.
    if legible {
        let _ = shaft::ledger(ui!(), run, focus, &geo);
    }
    // The ballot covers the view: the door's footer would only peek out
    // under it.
    let ballot = run
        .playtest
        .seats
        .get(&focus.unwrap_or_else(|| run.playtest.local_seat()))
        .is_some_and(|s| s.open);
    // A seat says it in the HUD's band under its view (`party::hud_band`);
    // a terminal's frame in an opaque footer over the view's foot (apart in
    // 3D, with the screen's other marks).
    if legible
        && !ballot
        && !hud_in_band()
        && let Some(board) = footer_sign(run, &geo, focus)
    {
        // Reserve an opaque footer after every world figure and edge
        // marker. Static plaques in the atlas never share its space.
        let top = NATIVE_H - board.h - 3;
        chrome::band(ui!(), (0, top, NATIVE_W, NATIVE_H - top), chrome::rim_of('B')[0]);
        ui!().stamp(&board, (NATIVE_W - board.w) / 2, top + 2);
    }
    // A yard ledger is actionable UI, including a native-size seat whose
    // HUD bands reduce its room height. Keep it above ground plaques.
    if !ballot && let Some(panel) = super::squires_lodge::panel(run, focus.unwrap_or(1)) {
        ui!().stamp(&panel, (NATIVE_W - panel.w).max(0) / 2, 3);
    }
    // A talk with a traveller covers the foot of the view.
    if legible {
        wayfarers::panel(ui!(), run, origin, focus);
    }
    if let (Some(three), Some(screen)) = (three, apart) {
        three.screen = screen;
    }
    (scenery, over)
}

/// How long a biome's name stays in the corner after a knight comes into
/// it: two seconds, then a second fading.
const TITLE_TICKS: u64 = 3 * crate::rules::HZ as u64;

/// The name of the biome knight `id` last came into, small in the top-left
/// corner of their own view, fading out: never a banner over play.
fn biome_title(over: &mut Img, run: &Run, id: u32, taken: &[(i32, i32, i32, i32)]) {
    // The blank slate is one country with no name to call out.
    if crate::world::plan::tabula() {
        return;
    }
    let Some(&(biome, since)) = run.kingdom.entered.get(&id) else {
        return;
    };
    let age = run.tick.saturating_sub(since);
    if age >= TITLE_TICKS {
        return;
    }
    let Some(b) = crate::world::biome::Biome::ALL.get(usize::from(biome)) else {
        return;
    };
    let step = TITLE_TICKS / 6;
    let ink = match age / step {
        0..=3 => 'h',
        4 => 'G',
        _ => 'j',
    };
    let text = b.name().to_uppercase();
    let w = super::super::ink::text_width(&text);
    // Top left; the top right when the goal's arrow is in that corner.
    let clear = |x: i32| {
        !taken.iter().any(|&(tx, ty, tw, th)| x - 2 < tx + tw && tx < x + w + 2 && 4 < ty + th && ty < 18)
    };
    let x = if clear(8) { 8 } else { NATIVE_W - 8 - w };
    over.text(x, 6, &text, ink);
    over.line(x, 15, x + w.min(24), 15, if ink == 'h' { 'G' } else { 'j' });
}

/// The kingdom's door sign for the seat's knight: at a door, its name under
/// ENTER; waiting outside while the party plays on, where they are and how
/// to go back. `None` elsewhere, or by a class banner nearer than the door.
pub(super) fn footer_sign(run: &Run, geo: &map::Geography, focus: Option<u32>) -> Option<Img> {
    // With the surveyor's rod up E builds, and the panel says what.
    if run.kingdom.survey.contains_key(&focus.unwrap_or(1)) {
        return None;
    }
    let hero = focus
        .and_then(|id| run.players.get(&id))
        .or_else(|| run.players.get(&1))
        // A host who plays no knight sends none (`Hero::absent`): the
        // party's first.
        .or_else(|| run.players.values().next());
    // Waiting outside while the party plays on: no door opens for this
    // knight but the one they came out of, so the footer says so.
    if let Some(w) = run.kingdom.waiting.as_ref() {
        let at_door = hero.is_some_and(|h| (h.x - w.entry.0).hypot(h.y - w.entry.1) < 2.5);
        return Some(kit::waiting_sign(&w.place, at_door));
    }
    let hero = hero?;
    // The door E opens (`rules::kingdom::door_at`): by a class banner the
    // key changes class, so the sign keeps quiet unless the door is nearer.
    let keys = crate::rules::hud::Keys::of(focus.unwrap_or(1), focus.is_none());
    let Some(door) = crate::rules::kingdom::door_at(geo, hero.x, hero.y) else {
        // Away from any door with wishmotes banked: the kingdom is waiting.
        return (run.treasury.get(crate::realm::Spoil::Wishmote) > 0)
            .then(|| kit::survey_sign(keys.cycle));
    };
    Some(kit::door_key_sign(&geo.sites[door].name, keys.interact))
}

/// The plan on a knight's surveyor's rod (`raisings`), standing where it would
/// rise: its picture on open ground, its lot framed in what E would do.
fn survey_ghost(over: &mut Img, run: &Run, origin: (i32, i32), focus: Option<u32>) {
    let Some(target) = run.survey_target(focus.unwrap_or(1)) else {
        return;
    };
    let lot = target.lot;
    let (x, y) = (lot.tx * TILE - origin.0, lot.ty * TILE - origin.1);
    if target.level == 1 {
        if let Some(wish) = crate::rules::raisings::standing(&[crate::rules::raisings::Raised {
            plan: target.plan.id.to_owned(),
            tx: lot.tx,
            ty: lot.ty,
            level: 1,
            by: String::new(),
        }])
        .first()
        {
            over.stamp(&crate::world::wishes::art(wish), x, y);
        }
    }
    let ink = if target.refused.is_some() { '7' } else { '3' };
    over.frame(x, y, lot.tw * TILE, lot.th * TILE, ink);
}

/// The surveyor's panel: the plan, what it does, what it costs and what E does
/// about it here.
fn survey_panel(ui: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let (Some(survey), Some(target)) = (run.kingdom.survey.get(&id), run.survey_target(id)) else {
        return;
    };
    let keys = crate::rules::hud::Keys::of(id, focus.is_none());
    let name = target.plan.name.to_uppercase();
    let title = if target.level == 1 { name } else { format!("{name} LEVEL {}", target.level) };
    let price = target.price.label().to_uppercase();
    let act = match &target.refused {
        Some(why) => why.clone(),
        None => format!(
            "HOLD {} {}",
            keys.interact,
            crate::rules::home::hold_bar(survey.hold, crate::rules::home::BUY_HOLD)
        ),
    };
    let next = format!("{} NEXT PLAN", keys.cycle);
    let lines = [title.as_str(), target.plan.says, price.as_str(), act.as_str(), next.as_str()];
    let w = lines.iter().map(|l| crate::ink::text_width(l)).max().unwrap_or(0) + 8;
    let (x, y) = ((NATIVE_W - w) / 2, 8);
    chrome::panel(ui, (x, y, w, 46), chrome::Tone::FRAMED);
    for (i, line) in lines.iter().enumerate() {
        let ink = match i {
            0 => 'O',
            3 if target.refused.is_some() => '7',
            _ => 'i',
        };
        ui.text(x + 4, y + 3 + 8 * i as i32, line, ink);
    }
}

/// The kingdom's door sign for a seat's HUD band (`footer_sign`), when the
/// kingdom is on screen and its ballot is shut.
pub(super) fn band_sign(run: &Run, focus: Option<u32>) -> Option<Img> {
    if run.room().kind != RoomKind::Kingdom {
        return None;
    }
    let ballot = run
        .playtest
        .seats
        .get(&focus.unwrap_or_else(|| run.playtest.local_seat()))
        .is_some_and(|s| s.open);
    if ballot {
        return None;
    }
    footer_sign(run, &map::geography(&run.kingdom.wishes), focus)
}

fn art(rows: &[&str]) -> Img {
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
    let mut im = Img::new(w, rows.len() as i32);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            if c != '.' {
                im.put(x as i32, y as i32, c);
            }
        }
    }
    im
}

/// The kingdom's little things (`trinkets`): the bell, the well, benches and
/// buckets, each doing its moment when used, and the key when a knight is
/// close.
fn trinkets(
    over: &mut Img,
    run: &Run,
    geo: &map::Geography,
    origin: (i32, i32),
    focus: Option<u32>,
    legible: bool,
) -> Vec<(i32, i32, i32, i32)> {
    use crate::rules::trinkets::{self, Thing};
    let hero = run.players.get(&focus.unwrap_or(1));
    let mut taken = Vec::new();
    let all = trinkets::things(geo);
    // Only the nearest thing in reach shows its key: the one E would use.
    let nearest = hero.and_then(|h| {
        all.iter()
            .enumerate()
            .map(|(i, (_, (tx, ty)))| {
                (i, (h.x - (*tx as f32 + 0.5) * TILE_UNITS).hypot(h.y - (*ty as f32 + 0.5) * TILE_UNITS))
            })
            .filter(|&(_, d)| d < 2.5 * TILE_UNITS)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    });
    for (i, (thing, (tx, ty))) in all.into_iter().enumerate() {
        let (x, y) = (tx * TILE + TILE / 2 - origin.0, (ty + 1) * TILE - origin.1);
        if x < -40 || y < -40 || x > NATIVE_W + 40 || y > NATIVE_H + 40 {
            continue;
        }
        let since = run
            .kingdom
            .moments
            .used
            .get(&(i as u8))
            .map(|&at| run.tick.saturating_sub(at));
        let live = since.filter(|&t| t < trinkets::MOMENT);
        match thing {
            Thing::Bell => {
                let swing = live.map_or(0, |t| [1, 0, -1, 0][(t / 3 % 4) as usize]);
                let frame = art(&[
                    "rRRRRRRRRRRr",
                    "r....h.....r",
                    "r...........",
                    "r..........r",
                    "r..........r",
                    "r..........r",
                    "r..........r",
                    "r..........r",
                    "r..........r",
                    "r..........r",
                ]);
                let bell = art(&["..oOOo..", ".oOOOOo.", ".oOOOOo.", "oOOOOOOo", "...OO..."]);
                over.stamp(&frame, x - 6, y - 10);
                over.stamp(&bell, x - 4 + swing, y - 8);
                if let Some(t) = live {
                    let r = 6.0 + t as f32 * 0.9;
                    for k in 0..32 {
                        let a = k as f32 * std::f32::consts::TAU / 32.0;
                        over.put(x + (a.cos() * r) as i32, y - 6 + (a.sin() * r * 0.6) as i32, '6');
                    }
                }
            }
            Thing::Well => {
                over.stamp(
                    &art(&[
                        "..BBBBBBBB..",
                        ".BBBBBBBBBB.",
                        "..r......r..",
                        "..r......r..",
                        ".GhhhhhhhhG.",
                        "GhhzzzzzzhhG",
                        "GhhhhhhhhhhG",
                        ".GGGGGGGGGG.",
                    ]),
                    x - 6,
                    y - 8,
                );
                if let Some(t) = live {
                    // The coin's arc in, then a ring on the water.
                    if t < 10 {
                        over.put(x - 2 + t as i32 / 2, y - 14 - (5 - (t as i32 - 5).abs()), '6');
                    } else {
                        let r = (t - 10) as f32 * 0.4;
                        for k in 0..16 {
                            let a = k as f32 * std::f32::consts::TAU / 16.0;
                            over.put(x + (a.cos() * r) as i32, y - 3 + (a.sin() * r * 0.4) as i32, 'w');
                        }
                    }
                }
            }
            Thing::Bench => {
                over.stamp(
                    &art(&["rRRRRRRRRRRRRr", "RRRRRRRRRRRRRR", ".r..........r.", ".r..........r."]),
                    x - 7,
                    y - 4,
                );
            }
            Thing::Crier => {
                // The town crier: a villager in a red hat with a hand bell
                // that swings while he cries.
                let man = kit::villager(run.tick as u32 / 8, false);
                over.stamp(&man, x - man.w / 2, y - man.h);
                over.rect(x - 3, y - man.h - 2, 7, 2, '7');
                let swing = live.map_or(0, |t| [2, 0, -2, 0][(t / 3 % 4) as usize]);
                over.rect(x + 5 + swing, y - man.h + 6, 3, 3, '6');
                over.put(x + 6 + swing, y - man.h + 9, '4');
            }
            Thing::Coach => {
                // A post with a cartwheel sign; a dust puff when one rides
                // in or out.
                over.stamp(
                    &art(&[
                        "..oOOOOOOo..",
                        ".oO.r.r.rOo.",
                        ".OOrrRRrrOO.",
                        ".oO.r.r.rOo.",
                        "..oOOOOOOo..",
                        ".....hh.....",
                        ".....hh.....",
                        ".....hh.....",
                        ".....hh.....",
                        "....GGGG....",
                    ]),
                    x - 6,
                    y - 10,
                );
                if let Some(t) = live {
                    let r = 4.0 + t as f32 * 0.5;
                    for k in 0..12 {
                        let a = k as f32 * std::f32::consts::TAU / 12.0;
                        over.put(x + (a.cos() * r) as i32, y + (a.sin() * r * 0.4) as i32, 'P');
                    }
                }
            }
            Thing::Stocks => {
                over.stamp(
                    &art(&[
                        "rRRRRRRRRRRRRRr",
                        "R..RRRR.RRRR..R",
                        "rRRRRRRRRRRRRRr",
                        ".....r...r.....",
                        ".....r...r.....",
                        ".....r...r.....",
                        "....rrr.rrr....",
                    ]),
                    x - 7,
                    y - 12,
                );
            }
            Thing::Goose => {
                if let Some((gx, gy)) = run.kingdom.moments.goose_at {
                    let (gx, gy) = (gx as f32 / 16.0, gy as f32 / 16.0);
                    let (px, py) = (gx * UNIT - origin.0 as f32, gy * UNIT - origin.1 as f32);
                    let mad = run.kingdom.moments.goose.is_some();
                    let goose = art(&[
                        "......WW....",
                        ".....WWWKo4.",
                        ".....WWW..4.",
                        "....WWW.....",
                        "WWWWWWWW....",
                        ".WWWWWWWV...",
                        "..WWWWWV....",
                        "...4..4.....",
                        "...44.44....",
                    ]);
                    let goose = if mad && run.tick / 4 % 2 == 0 { goose.flip_h() } else { goose };
                    over.stamp(&goose, px as i32 - 6, py as i32 - 9);
                    if mad {
                        chrome::label(over, (px as i32 - 10, py as i32 - 20), "HONK", 'O', chrome::Tone::SLATE);
                    }
                }
                continue;
            }
            Thing::Winch => shaft::winch(over, run, x, y, live),
            Thing::Bucket => {
                // Kicked: up and away in an arc, then back on its spot.
                let (dx, dy) = live.map_or((0, 0), |t| {
                    let t = t as i32;
                    (t * 2, -(t * (30 - t)) / 12)
                });
                over.stamp(
                    &art(&[".h....h.", "..h..h..", ".rRRRRr.", ".rRRRRr.", ".rRRRRr.", "..rrrr.."]),
                    x - 4 + dx,
                    y - 6 + dy,
                );
            }
        }
        if !legible {
            continue;
        }
        if nearest == Some(i) && !hero.is_some_and(|h| h.sitting) {
            let next = (thing == Thing::Coach)
                .then(|| trinkets::next_coach(geo, i))
                .flatten()
                .map(|(_, _, place)| format!("TO {}", place.to_uppercase()));
            let verb = if thing == Thing::Stocks && hero.is_some_and(|h| h.stocked) {
                "GET OUT"
            } else if let Some(next) = &next {
                next.as_str()
            } else {
                thing.verb()
            };
            let label = format!("E: {verb}");
            let w = super::super::ink::text_width(&label);
            let (lx, ly) = (x - w / 2, y - 24);
            taken.push(chrome::label(over, (lx, ly), &label, 'O', chrome::Tone::KEY));
        }
        taken.push((x - 8, y - 12, 16, 12));
    }
    taken
}

/// The places whose names are carved into the scenery already: a plaque
/// over them would show the name twice.
const CARVED: [(&str, &str); 3] = [
    ("Artisan Quarter", "ARTISANS"),
    ("Colosseum", "COLOSSEUM"),
    ("Knights Tournament", "TOURNAMENT"),
];

/// A name plaque over every place in view, so the map can be read without
/// walking up to each door. The place the knight is in or beside goes
/// first, so its own name always shows; a plaque that would cover another
/// moves up, down or aside, and is left out only when nowhere is free (the
/// goal's own arrow names it already).
fn plaques(
    over: &mut Img,
    geo: &map::Geography,
    origin: (i32, i32),
    goal: Option<&Goal>,
    hero: Option<(f32, f32)>,
    mut placed: Vec<(i32, i32, i32, i32)>,
) {
    // The carved signs stand below their doors; keep plaques off them.
    for (name, label) in CARVED {
        if let Some(site) = geo.sites.iter().find(|s| s.name == name) {
            let w = super::super::ink::text_width(label) + 6;
            let x = site.stand.0 * TILE + 8 - w / 2 - origin.0;
            let base = if name == "Artisan Quarter" { site.stand.1 * TILE + 14 } else { (site.stand.1 + 1) * TILE + 12 };
            placed.push((x, base - 11 - origin.1, w, 11));
        }
    }
    let mut order: Vec<(usize, &map::Site)> = geo.sites.iter().enumerate().collect();
    if let Some((hx, hy)) = hero {
        order.sort_by_key(|(_, s)| {
            let (dx, dy) = (s.stand.0 as f32 + 0.5 - hx, s.stand.1 as f32 + 0.7 - hy);
            (dx.hypot(dy) * 4.0) as i32
        });
    }
    for (rank, (i, site)) in order.into_iter().enumerate() {
        if goal.is_some_and(|g| g.stand == site.stand) || CARVED.iter().any(|(name, _)| site.name == *name) {
            continue;
        }
        let top = site.lot.map_or(site.stand.1 - 3, |l| l.ty);
        let label = site.name.to_uppercase();
        let art = picture(site.room, &label);
        // The picture sits left of the name, a glyph's width and a gap.
        let lead = if art.is_some() { 9 } else { 0 };
        let w = super::super::ink::text_width(&label) + lead;
        let x = site.stand.0 * TILE + TILE / 2 - origin.0 - w / 2;
        // A pixel of black between two tags (the tags carry their own
        // pixel of air inside the rim).
        let clashes = |rect: (i32, i32, i32, i32)| {
            placed.iter().any(|&(px, py, pw, ph)| {
                rect.0 < px + pw + 1 && px < rect.0 + rect.2 + 1 && rect.1 < py + ph + 1 && py < rect.1 + rect.3 + 1
            })
        };
        // Its own line, or down onto its own roof, or one line up, if a
        // neighbour's name is there. The place the knight stands at may
        // also step aside, so its own name always shows.
        let y0 = map::roof(geo, i).unwrap_or(top * TILE) - origin.1;
        let side = if rank == 0 && hero.is_some() { w / 2 + 12 } else { 0 };
        let Some((x, y)) = [0, -side, side]
            .into_iter()
            .flat_map(|dx| [y0 - 12, y0 + 2, y0 - 26].into_iter().map(move |y| (x + dx, y)))
            .find(|&(x, y)| {
                x >= 3 && y >= 2 && x + w <= NATIVE_W - 3 && y + 9 <= NATIVE_H - 40 && !clashes((x - 3, y - 2, w + 6, 11))
            })
        else {
            continue;
        };
        let rect = (x - 3, y - 2, w + 6, 11);
        placed.push(rect);
        chrome::tag(over, rect, chrome::Tone::SLATE);
        if let Some(rows) = art {
            for (dy, row) in rows.iter().enumerate() {
                for (dx, c) in row.chars().enumerate() {
                    if c != '.' {
                        over.put(x + dx as i32, y + dy as i32, chrome::dusk_mark(c));
                    }
                }
            }
        }
        over.text(x + lead, y, &label, 'O');
    }
}

/// What happens at a place, as a 7x7 picture on its plaque.
fn picture(kind: RoomKind, name: &str) -> Option<[&'static str; 7]> {
    const POT: [&str; 7] = [".......", ".BBBBB.", "..PPP..", ".PPPPP.", ".PoPPP.", ".PPPPP.", "..PPP.."];
    const FLAME: [&str; 7] = ["...6...", "..676..", "..777..", "...7...", "..BBB..", "..BOB..", ".BBBBB."];
    const MUSHROOM: [&str; 7] = ["..RRR..", ".R9RRR.", "RRRR9RR", "...h...", "...h...", "..hhh..", "......."];
    const WAVE: [&str; 7] = [".......", "..UU...", ".U..U..", "U....UU", ".......", "UU..UU.", "..UU..U"];
    const VALVE: [&str; 7] = ["..OOO..", ".O.O.O.", "O..O..O", "OOOOOOO", "O..O..O", ".O.O.O.", "..OOO.."];
    const BELL: [&str; 7] = ["...6...", "..666..", ".66666.", ".66666.", ".66666.", "6666666", "...6..."];
    const PIGEON: [&str; 7] = [".......", "....uu.", "...uuk@", "uuuuuu.", ".uuuuu.", "..o.o..", "......."];
    const GOOSE: [&str; 7] = ["....99.", "....9k@", "....9..", "99999..", "9999...", ".@.@...", "......."];
    const KITE: [&str; 7] = ["...O...", "..ORO..", ".ORRRO.", "..ORO..", "...O...", "...h...", "....h.."];
    const FLAG: [&str; 7] = [".hOOOO.", ".hOOOO.", ".hOOO..", ".h.....", ".h.....", ".h.....", "hhh...."];
    const WHEEL: [&str; 7] = ["..ooo..", ".o.o.o.", "o..o..o", "ooooooo", "o..o..o", ".o.o.o.", "..ooo.."];
    const SHOE: [&str; 7] = [".o...o.", ".o...o.", "o.....o", "o.....o", "o.....o", ".o...o.", "..ooo.."];
    const SWORDS: [&str; 7] = ["i.....i", ".i...i.", "..i.i..", "...i...", "..i.i..", ".o...o.", "o.....o"];
    const CROWN: [&str; 7] = [".......", "6..6..6", "66.6.66", "6666666", "6666666", ".......", "......."];
    const BOOK: [&str; 7] = [".......", "BBB.BBB", "B99B99B", "B99B99B", "B99B99B", "BBBBBBB", "......."];
    const STAIRS: [&str; 7] = ["......h", ".....hh", "....hhh", "...hhhh", "..hhhhh", ".hhhhhh", "hhhhhhh"];
    const DUCK: [&str; 7] = ["...TT..", "...Tk@@", "...TT..", "TTTTTT.", ".TTTT..", "UUUUUUU", "......."];
    const MUG: [&str; 7] = [".......", ".999...", ".TTTB..", ".TTT.B.", ".TTTB..", ".TTT...", "......."];
    const APPLE: [&str; 7] = ["...e...", "..e....", ".RRRR..", "RRRRRR.", "RR9RRR.", "RRRRRR.", ".RRRR.."];
    const STAR: [&str; 7] = ["...6...", "...6...", "6666666", ".66666.", "..666..", ".66.66.", "6.....6"];
    const DEED: [&str; 7] = ["TTTTTT.", "T9999T.", "T9hh9T.", "T9hh977", "T99997.", "TTTTTT.", "......."];
    const TARGET: [&str; 7] = ["..ooo..", ".oTTTo.", "oTRRRTo", "oTR6RTo", "oTRRRTo", ".oTTTo.", "..ooo.."];
    const ARROW: [&str; 7] = ["...V...", "..VVV..", "...o...", "...o...", "...o...", "..ToT..", ".T.o.T."];
    const STRAW: [&str; 7] = ["...o...", "..k.k..", "...o...", "ooooooo", "..TTT..", "..TRT..", "...B..."];
    const COIN: [&str; 7] = ["..666..", ".64446.", "6646466", "6644466", "6646466", ".64446.", "..666.."];
    const TUB: [&str; 7] = ["..v.v..", ".v.v.v.", ".......", "rUUUUUr", "rUUUUUr", ".rrrrr.", "..r.r.."];
    const ANVIL: [&str; 7] = [".......", "hhhhhhh", ".hhhhh.", "...h...", "..hhh..", ".hhhhh.", "......."];
    const GRIFFIN: [&str; 7] = ["r.....r", "Rr...rR", ".RrOrR.", "..OiO..", "..OO5..", ".O...O.", "......."];
    const CHEESE: [&str; 7] = [".......", "....66.", "..6666.", "666666.", "6.66.6.", "666666.", "......."];
    Some(match kind {
        RoomKind::Gate | RoomKind::MineHead => STAIRS,
        RoomKind::Stables | RoomKind::Hippodrome | RoomKind::Meadow | RoomKind::Ranch | RoomKind::Pasture => SHOE,
        RoomKind::Lists | RoomKind::Yard => SWORDS,
        RoomKind::KingsHall | RoomKind::Kingdom => CROWN,
        RoomKind::Forest | RoomKind::Crag | RoomKind::Lighthouse => FLAME,
        RoomKind::Village
        | RoomKind::CollegeGrounds
        | RoomKind::CollegeHall
        | RoomKind::CollegeClass
        | RoomKind::CollegeLibrary
        | RoomKind::WizardStudy => BOOK,
        RoomKind::Fairground => DUCK,
        RoomKind::Riverbank | RoomKind::Saltworks | RoomKind::Millpond | RoomKind::Laundry | RoomKind::Marsh => WAVE,
        RoomKind::Observatory | RoomKind::TowerStars => STAR,
        RoomKind::Windmill | RoomKind::Timberyard | RoomKind::Caravan | RoomKind::Wheelwright => WHEEL,
        RoomKind::Orchard => APPLE,
        RoomKind::Downs => CHEESE,
        RoomKind::Cliffs | RoomKind::Rookery | RoomKind::Aqueduct | RoomKind::Aviary => PIGEON,
        RoomKind::Priory | RoomKind::SunkenAbbey | RoomKind::Belfry => BELL,
        RoomKind::Strand => POT,
        RoomKind::HedgeMaze | RoomKind::Bothy => MUSHROOM,
        RoomKind::Sluice => VALVE,
        RoomKind::GooseGreen => GOOSE,
        RoomKind::Kitewright => KITE,
        RoomKind::SignalHill | RoomKind::Ruins => FLAG,
        RoomKind::Tavern => MUG,
        RoomKind::LandOffice => DEED,
        RoomKind::LongbowGreen => TARGET,
        RoomKind::Fletchery => ARROW,
        RoomKind::Scarecrow => STRAW,
        RoomKind::Market => COIN,
        RoomKind::Bathhouse => TUB,
        RoomKind::Smithy => ANVIL,
        RoomKind::Eyrie => GRIFFIN,
        // The realm's own halls go by what they are called.
        RoomKind::RealmHall => {
            let has = |w: &[&str]| w.iter().any(|w| name.contains(w));
            if has(&["CHAPEL", "PRIORY", "ABBEY"]) {
                BELL
            } else if has(&["ROUND TABLE", "COLOSSEUM", "TOURNAMENT", "GATEHOUSE", "SMITHY"]) {
                SWORDS
            } else if has(&["SCRIPTORIUM", "SCHOOL", "ATHENAEUM", "LIBRARY"]) {
                BOOK
            } else if has(&["DRAGON", "LANTERN", "LUMEN", "FORGE"]) {
                FLAME
            } else if has(&["HOMECOMING", "INN", "TAVERN"]) {
                MUG
            } else if has(&["ARTISAN", "WORKS"]) {
                WHEEL
            } else {
                CROWN
            }
        }
        _ => return None,
    })
}

/// The way to the knight's goal: over its door when it is in view, and at
/// the edge of the view, pointing, with its name, when it is not.
fn goal_marker(over: &mut Img, goal: &Goal, origin: (i32, i32), tick: u32, avoid: &[(i32, i32, i32, i32)], floor: f32) -> Vec<(i32, i32, i32, i32)> {
    let (gx, gy) = (
        goal.stand.0 * TILE + TILE / 2 - origin.0,
        goal.stand.1 * TILE - origin.1,
    );
    // The works' corner block owns the view's top left: on the blank-slate map
    // the goal's arrow and its name slide along past it rather than over it.
    pointer_with(over, (gx, gy), &goal.name, tick, crate::world::plan::tabula(), avoid, floor)
}

/// The label the surface keeps in its top left corner.
pub(super) const WORKS_LABEL: &str = "[4] KINGDOM WORKS";

/// A pointer to the view's pixel `(gx, gy)`, named `name`: bobbing over it,
/// pointing down, while it is in view, and at the edge of the view toward
/// it when it is not.
pub(super) fn pointer_toward(over: &mut Img, (gx, gy): (i32, i32), name: &str, tick: u32) -> Vec<(i32, i32, i32, i32)> {
    pointer_with(over, (gx, gy), name, tick, false, &[], NATIVE_H as f32)
}

/// [`pointer_toward`], on the blank-slate map's terms when `tabula`: the arrow and its
/// name keep clear of every box in `avoid` (the HUD's corner, the signs of the ways
/// down) by sliding along, none is lower than `floor`, a door only just off the side
/// of the view is pointed at from over where it would be, not from the edge over its
/// own sign, and an arrow below holds lower.
fn pointer_with(over: &mut Img, (gx, gy): (i32, i32), name: &str, tick: u32, tabula: bool, avoid: &[(i32, i32, i32, i32)], floor: f32) -> Vec<(i32, i32, i32, i32)> {
    let gx = if tabula && (-40..NATIVE_W + 40).contains(&gx) { gx.clamp(24, NATIVE_W - 25) } else { gx };
    let inside = (24..NATIVE_W - 24).contains(&gx) && (56..NATIVE_H - 8).contains(&gy);
    let (ax, ay, ux, uy) = if inside {
        // Bobbing over the door, pointing down at it.
        let bob = [0, 1, 2, 1][(tick / 6 % 4) as usize];
        (gx as f32, (gy - 46 - bob) as f32, 0.0, 1.0)
    } else {
        let (cx, cy) = ((NATIVE_W / 2) as f32, (NATIVE_H / 2) as f32);
        let (dx, dy) = (gx as f32 - cx, gy as f32 - cy);
        // Below, it stops above the footer and the banners' names (on the
        // blank-slate map a little lower, clear of the knight at the middle,
        // whose feet its name would stand over).
        let below = if tabula { 44 } else { 64 };
        let reach_y = if dy > 0.0 { NATIVE_H / 2 - below } else { NATIVE_H / 2 - 14 };
        let k = ((NATIVE_W / 2 - 14) as f32 / dx.abs().max(0.01))
            .min(reach_y as f32 / dy.abs().max(0.01));
        let len = dx.hypot(dy).max(0.01);
        (cx + dx * k, cy + dy * k, dx / len, dy / len)
    };
    // Never lower than the HUD at the view's foot allows.
    let ay = ay.min(floor);
    // Clear of what the HUD keeps in the corner: the arrow and its name slide right along the
    // edge until neither touches it.
    let (mut ax, mut ay) = (ax, ay);
    for pass in 0..8 {
        let (tag, arrow) = super::camera::pointer_boxes((ax, ay), (ux, uy), name, !inside);
        let hit = avoid.iter().find(|r| overlaps(r, &tag) || overlaps(r, &arrow));
        let Some(&(rx, ry, rw, rh)) = hit else { break };
        if pass < 3 {
            // Past it on its right when the view has the room, else on its left.
            let half = tag.2.max(arrow.2) / 2 + 2;
            let right = rx + rw + 4 + half;
            ax = if right <= NATIVE_W - 26 { right as f32 } else { (rx - 4 - half).max(24) as f32 };
        } else {
            // Neither side is clear (two boxes share the band): below it.
            let top = tag.1.min(arrow.1);
            ay = (ay + (ry + rh + 3 - top) as f32).min(floor);
        }
    }
    pointer(over, (ax, ay), (ux, uy), name, !inside)
}

/// Whether two rectangles `(x, y, w, h)` share a pixel.
fn overlaps(a: &(i32, i32, i32, i32), b: &(i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

pub(super) fn hall(run: &Run) -> Img {
    let mut cv = Img::black(NATIVE_W, NATIVE_H);
    for y in 0..NATIVE_H {
        for x in 0..NATIVE_W {
            if let Some(ch) = flags(x, y) {
                cv.put(x, y, ch);
            }
        }
    }
    let tiles = Tiles::get();
    for x in 0..COLS as i32 {
        cv.stamp(tiles.rock(RockKind::Slate, hash(x, 0, 5)), x * TILE, 0);
    }
    for y in 1..ROWS as i32 {
        for x in [0, COLS as i32 - 1] {
            cv.stamp(
                tiles.rock(RockKind::Slate, hash(x, y, 5)),
                x * TILE,
                y * TILE,
            );
        }
    }
    for x in 1..COLS as i32 - 1 {
        if x != 11 && x != 12 {
            cv.stamp(
                tiles.rock(RockKind::Slate, hash(x, 13, 5)),
                x * TILE,
                (ROWS as i32 - 1) * TILE,
            );
        }
    }
    cv.stamp(&kit::books(8), 52, 46);
    cv.stamp(&kit::blade_rack(5), 282, 42);
    cv.stamp(&kit::round_table(&[]), 150, 64);
    for x in [28, NATIVE_W - 34] {
        cv.stamp(&torch(run.tick as u32, x as u32), x, 34);
    }
    if let Some(back) = &run.kingdom.return_to {
        let label = district_sign(&back.name);
        cv.stamp(&label, (NATIVE_W - label.w) / 2, 18);
    }
    let exit = kit::door_sign("Kingdom", true);
    cv.stamp(&exit, (NATIVE_W - exit.w) / 2, NATIVE_H - 32);
    cv
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__survey_sign__tests.rs"]
mod survey_sign_tests;

#[cfg(test)]
#[path = "../../../../tests/render/arena__kingdom_pointer__tests.rs"]
mod pointer_tests;
