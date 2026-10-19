//! Viewport-only painting for the continuous surface.
use super::super::{kingdom as map, kit};
use super::*;

// The run's finds out in the open (declared here, not in `arena`, whose
// source the arcade cell gate fingerprints).
#[path = "finds.rs"]
mod finds;
use crate::rules::kingdom::Goal;

pub(super) fn layers_with(run: &Run, focus: Option<u32>, legible: bool, modern: bool) -> (Img, Img) {
    paint(run, focus, legible, modern, None)
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
    let (_, over) = paint(run, focus, true, false, Some(&mut three));
    // The ballot panel lies on the screen, as `arena::layers_with` lays it
    // over the view.
    votes::panel(&mut three.screen, run, focus);
    (over, three.screen, three.figures)
}

/// Where `paint` puts what a window in 3D draws apart.
struct Three {
    screen: Img,
    figures: Vec<Figure>,
}

/// A knight as a figure of the kingdom in 3D: its contact shadow and its
/// sprite where `paint` lays them, on a picture of their own, its feet its
/// depth.
fn knight_figure(im: &Img, hero: &crate::rules::Hero) -> Figure {
    let (hx, hy) = (hero.x * UNIT, hero.y * UNIT);
    let (fx, fy) = (hx.floor() as i32, hy.floor() as i32);
    let (sx, sy) = (hx - im.w as f32 / 2.0, hy - im.h as f32 + 1.0);
    let (left, top) = (sx.floor() as i32, sy.floor() as i32);
    let (x0, y0) = (left.min(fx - 5), top.min(fy));
    let (x1, y1) = ((left + im.w + 1).max(fx + 6), (top + im.h + 1).max(fy + 2));
    let mut img = Img::new(x1 - x0, y1 - y0);
    knights::shadow(&mut img, fx - x0, fy - y0);
    img.stamp_sub(im, sx - x0 as f32, sy - y0 as f32);
    Figure { img, x: x0, y: y0, frac: (0.0, 0.0), depth: hero.y, bodies: Vec::new(), foe: false }
}

fn paint(run: &Run, focus: Option<u32>, legible: bool, modern: bool, mut three: Option<&mut Three>) -> (Img, Img) {
    let origin = camera(run, focus);
    let scenery = if modern || three.is_some() {
        Img::new(NATIVE_W, NATIVE_H)
    } else if crate::world3d::enabled() {
        // The world in 3D, on a page that draws no scenes: each chunk's
        // plate laid down on its ground (`world3d::kingdom::lay_plates`),
        // what the run stands on its lots painted over it.
        let mut scenery = crop(&map::atlas(&run.kingdom.wishes), origin);
        let lots = crate::world3d::lots::of(run);
        crate::world3d::kingdom::lay_plates(&mut scenery, origin, lots.as_deref());
        scenery
    } else {
        crop(&map::atlas(&run.kingdom.wishes), origin)
    };
    let mut over = Img::new(NATIVE_W, NATIVE_H);
    super::forest_road::kingdom(&mut over, run, origin);
    super::roundtable::kingdom(&mut over, run, origin);
    let geo = map::geography(&run.kingdom.wishes);
    // Region names are lettered on the ground, under everything that moves.
    let lettered = map::letter_regions(&mut over, origin);
    let tick = run.tick as u32;
    let local = |x: f32, y: f32| {
        let (x, y) = at(x, y);
        (x - origin.0, y - origin.1)
    };
    for (i, site) in geo.sites.iter().enumerate().step_by(4) {
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
    for i in 0..12 {
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
    for i in 0..8 {
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
    for (&id, hero) in &run.players {
        let (x, y) = local(hero.x, hero.y);
        if x < -24 || y < -24 || x > NATIVE_W + 24 || y > NATIVE_H + 24 {
            continue;
        }
        let im = if let Some(avatar) = &hero.avatar {
            knights::avatar_figure(hero, &avatar.pixels, run.shield_out(id))
        } else {
            knights::figure_with(hero, id, run.shield_out(id))
        };
        let mut im = knights::dress(&im, &hero.worn(&run.book), run.tick);
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
            im = knights::rolled(&im, left);
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
        if hero.hp == 0 {
            bones::draw(&mut over, hero, id, run.tick, false, x, y, true);
        } else if let Some(three) = three.as_deref_mut() {
            three.figures.push(knight_figure(&im, hero));
        } else if !(modern && art_v2::knight_supported(run, hero, id)) {
            knights::shadow(&mut over, x, y);
            over.stamp_sub(
                &im,
                hero.x * UNIT - origin.0 as f32 - im.w as f32 / 2.0,
                hero.y * UNIT - origin.1 as f32 - im.h as f32 + 1.0,
            );
        }
    }
    for shot in &run.projectiles {
        let (x, y) = local(shot.x, shot.y);
        over.rect(x - 1, y - 1, 3, 3, '6');
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
    super::holdings::castle_pennons(&mut over, run, origin);
    beyond(&mut over, run, origin);
    let goal = run.kingdom_goal(focus.unwrap_or(1));
    let mut taken = class_banners(&mut over, run, &geo, origin, focus, tick, legible);
    taken.extend(lettered);
    taken.extend(trinkets(&mut over, run, &geo, origin, focus, legible));
    // In 3D the finds and the herald's posts stand up as blocks
    // (`world3d::lots`): only who keeps them and their keys are drawn here.
    let stood = three.is_some();
    taken.extend(finds::draw(&mut over, run, origin, focus, legible, stood));
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
    // The goal's arrow and name go down first; plaques make way for them.
    let mut goal_layer = Img::new(NATIVE_W, NATIVE_H);
    if let Some(goal) = &goal {
        taken.extend(goal_marker(&mut goal_layer, goal, origin, tick));
    }
    if legible {
        let hero = run.players.get(&focus.unwrap_or(1)).map(|h| (h.x / TILE_UNITS, h.y / TILE_UNITS));
        // A fingerpost's arms read as the knight passes it.
        if let Some(near) = hero {
            taken.extend(super::super::signs::arms(&mut over, origin, near));
        }
        biome_title(ui!(), run, focus.unwrap_or(1), &taken);
        plaques(&mut over, &geo, origin, goal.as_ref(), hero, taken);
    }
    ui!().stamp(&goal_layer, 0, 0);
    school_board(ui!(), run, focus, legible);
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
    let hero = focus
        .and_then(|id| run.players.get(&id))
        .or_else(|| run.players.get(&1))
        // A host who plays no knight sends none (`Hero::absent`): the
        // party's first.
        .or_else(|| run.players.values().next());
    // By a class banner the key changes class, so the door's sign keeps
    // quiet unless the door is the nearer of the two.
    let banner = |h: &crate::rules::Hero| {
        crate::rules::classes::stones(geo)
            .iter()
            .map(|&(_, p)| p)
            .chain(crate::rules::trinkets::things(geo).iter().map(|&(_, p)| p))
            .map(|(x, y)| {
                (h.x - (x as f32 + 0.5) * TILE_UNITS).hypot(h.y - (y as f32 + 0.5) * TILE_UNITS)
            })
            .fold(f32::MAX, f32::min)
    };
    // Waiting outside while the party plays on: no door opens for this
    // knight but the one they came out of, so the footer says so.
    if let Some(w) = run.kingdom.waiting.as_ref() {
        let at_door = hero.is_some_and(|h| (h.x - w.entry.0).hypot(h.y - w.entry.1) < 2.5);
        return Some(kit::waiting_sign(&w.place, at_door));
    }
    let hero = hero?;
    let site = geo.sites.iter().find(|s| {
        let x = (s.stand.0 as f32 + 0.5) * TILE_UNITS;
        let y = (s.stand.1 as f32 + 0.7) * TILE_UNITS;
        let door = (hero.x - x).hypot(hero.y - y);
        door < 4.5 && door <= banner(hero)
    })?;
    Some(kit::door_sign(&site.name, true))
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

/// The school's mission board: while a knight reads a master's board, a
/// framed scroll over the view with the class's three missions, each in
/// plain words with its progress, and the master's line beneath.
fn school_board(over: &mut Img, run: &Run, focus: Option<u32>, legible: bool) {
    use crate::rules::classes::CLASSES;
    if run.school.owner.is_none() || !legible {
        return;
    }
    let Some(i) = usize::from(run.school.class).checked_rem(CLASSES.len()) else {
        return;
    };
    let class = &CLASSES[i];
    let hero = run
        .school
        .owner
        .and_then(|id| run.players.get(&id))
        .or_else(|| focus.and_then(|id| run.players.get(&id)));
    let key = hero.map(crate::rules::home::Home::school_key);
    let standing = key
        .as_deref()
        .map(|k| run.home.school.standing(k))
        .unwrap_or_default();
    let w = 236;
    let rows = crate::rules::school::MISSIONS
        .iter()
        .filter(|m| m.class == class.name)
        .map(|m| {
            let have = standing.count(m);
            let mark = if have >= m.need { '*' } else { ' ' };
            format!("{mark} {}: {} ({have}/{})", m.step, m.asks, m.need)
        })
        .collect::<Vec<_>>();
    let lines = std::iter::once(format!("{}'S BOARD", class.name.to_uppercase()))
        .chain(rows)
        .chain(std::iter::once(String::from(
            "E again: the master hands over earned gear",
        )))
        .collect::<Vec<_>>();
    let h = 13 + lines.len() as i32 * 11;
    let (x, y) = ((NATIVE_W - w) / 2, (NATIVE_H - h) / 2 - 6);
    over.rect(x + 2, y + 2, w - 4, h - 4, 'K');
    over.frame(x, y, w, h, class.ink);
    over.frame(x + 2, y + 2, w - 4, h - 4, 'u');
    for (i, line) in lines.iter().enumerate() {
        over.text(x + 8, y + 8 + i as i32 * 11, &line, if i == 0 { 'O' } else { 'i' });
    }
}

/// The class halls' banner stones: a pole and a flag in the class's colour
/// with its emblem, the class's name, and the key when a knight is close.
fn class_banners(
    over: &mut Img,
    run: &Run,
    geo: &map::Geography,
    origin: (i32, i32),
    focus: Option<u32>,
    tick: u32,
    legible: bool,
) -> Vec<(i32, i32, i32, i32)> {
    use crate::rules::classes::{self, CLASSES};
    let hero = run.players.get(&focus.unwrap_or(1));
    let mut taken = Vec::new();
    for (i, (tx, ty)) in classes::stones(geo) {
        let class = &CLASSES[i];
        let (x, y) = (tx * TILE + TILE / 2 - origin.0, (ty + 1) * TILE - origin.1);
        if x < -24 || y < -40 || x > NATIVE_W + 24 || y > NATIVE_H + 40 {
            continue;
        }
        // Pole, base stone and a flag that flutters a pixel.
        over.rect(x - 4, y - 3, 9, 3, 'G');
        over.rect(x - 1, y - 30, 2, 28, 'h');
        let flap = i32::from((tick / 8 + i as u32) % 2 == 0);
        over.rect(x + 1, y - 29 + flap, 13, 11, class.ink);
        over.rect(x + 1, y - 29 + flap, 13, 1, 'K');
        let emblem: &[&str] = match class.name {
            "Archer" => &[".9...", "9.9..", "9..9.", "9.9..", ".9..."],
            "Shieldmaiden" => &["99999", "9...9", "9...9", ".9.9.", "..9.."],
            "Gunner" => &["...9.", "..9..", ".999.", ".999.", "..9.."],
            "Swordsman" => &["..9..", "..9..", "..9..", ".999.", "..9.."],
            "Mage" => &["..6..", ".666.", "66666", ".666.", "..6.."],
            "Healer" => &["..7..", "..7..", "77777", "..7..", "..7.."],
            _ => &["99.9.", "9999.", "99.9.", "..9..", "..9.."],
        };
        for (ey, row) in emblem.iter().enumerate() {
            for (ex, c) in row.chars().enumerate() {
                if c != '.' {
                    over.put(x + 5 + ex as i32, y - 26 + flap + ey as i32, chrome::dusk_mark(c));
                }
            }
        }
        // The class on a stand by its banner, in the onlooker's own trim:
        // what they would be, at a glance, before the E.
        if let Some(look) = super::class_looks::Look::named(class.name) {
            use super::class_looks::{Pose, figure};
            let seat = focus.unwrap_or(1);
            let im = figure(look, knights::Facing::Down, Pose::Stand, true).recolor(knights::seat_colours(seat));
            let (mx, my) = (x - 5 - im.w, y - 1);
            over.rect(mx + im.w / 2 - 4, my - 1, 9, 2, 'P');
            over.rect(mx + im.w / 2 - 3, my - 2, 7, 1, 'r');
            over.stamp(&im, mx, my - 2 - im.h);
            taken.push((mx, my - 2 - im.h, im.w, im.h + 3));
        }
        if !legible {
            continue;
        }
        let near = hero.is_some_and(|h| {
            (h.x - (tx as f32 + 0.5) * TILE_UNITS).hypot(h.y - (ty as f32 + 0.5) * TILE_UNITS)
                < 3.0 * TILE_UNITS
        });
        let mine = hero.and_then(classes::of).is_some_and(|c| c.name == class.name);
        let label = match (near, mine) {
            (_, true) => format!("{} (YOU)", class.name.to_uppercase()),
            (true, false) => format!("E: {}", class.name.to_uppercase()),
            _ => class.name.to_uppercase(),
        };
        let w = super::super::ink::text_width(&label);
        let (lx, ly) = (x - w / 2, y + 3);
        let tone = if near { chrome::Tone::KEY } else { chrome::Tone::SLATE };
        taken.push(chrome::label(over, (lx, ly), &label, 'O', tone));
        taken.push((x - 4, y - 30, 18, 30));
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
fn goal_marker(over: &mut Img, goal: &Goal, origin: (i32, i32), tick: u32) -> Vec<(i32, i32, i32, i32)> {
    let (gx, gy) = (
        goal.stand.0 * TILE + TILE / 2 - origin.0,
        goal.stand.1 * TILE - origin.1,
    );
    let inside = (24..NATIVE_W - 24).contains(&gx) && (56..NATIVE_H - 8).contains(&gy);
    let (ax, ay, ux, uy) = if inside {
        // Bobbing over the door, pointing down at it.
        let bob = [0, 1, 2, 1][(tick / 6 % 4) as usize];
        (gx as f32, (gy - 46 - bob) as f32, 0.0, 1.0)
    } else {
        let (cx, cy) = ((NATIVE_W / 2) as f32, (NATIVE_H / 2) as f32);
        let (dx, dy) = (gx as f32 - cx, gy as f32 - cy);
        // Below, it stops above the footer and the banners' names.
        let reach_y = if dy > 0.0 { NATIVE_H / 2 - 64 } else { NATIVE_H / 2 - 14 };
        let k = ((NATIVE_W / 2 - 14) as f32 / dx.abs().max(0.01))
            .min(reach_y as f32 / dy.abs().max(0.01));
        let len = dx.hypot(dy).max(0.01);
        (cx + dx * k, cy + dy * k, dx / len, dy / len)
    };
    pointer(over, (ax, ay), (ux, uy), &goal.name, !inside)
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
#[path = "../../../../tests/render/overworld__school_shots.rs"]
mod school_shots;
