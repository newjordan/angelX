//! Opt-in upgrade art direction for the reviewed scenes. Simulation coordinates, hit boxes,
//! cues and authored ink materials are shared with v1. Only presentation changes.
mod combat;
pub mod figures;
mod kit;
pub mod paint;
mod race;
mod scenes;
mod ui;
use super::*;
use paint::{Canvas, Point, material};
use std::sync::{Arc, Mutex};

pub const WIDTH: i32 = 768;
pub const HEIGHT: i32 = 448;
pub fn enabled() -> bool {
    crate::render::art::enabled()
}
pub fn size() -> (u32, u32) {
    if enabled() {
        (WIDTH as u32, HEIGHT as u32)
    } else {
        (NATIVE_W as u32, NATIVE_H as u32)
    }
}
pub fn supported(run: &Run) -> bool {
    run.floor_plate(None).is_none()
        && !run.side_on()
        && run.circle() == 0
        && run.mode != crate::rules::fortune::Mode::LightsOut
        && (matches!(
            run.room().kind,
            RoomKind::Pasture
                | RoomKind::Ranch
                | RoomKind::CollegeHall
                | RoomKind::Kingdom
                | RoomKind::Sanctuary
        ) || (run.room().kind == RoomKind::Fight
            && !run.room().great()
            && matches!(run.dungeon.pack, Pack::Crypt | Pack::Archive))
            || run.chariot.is_some()
            || run.enemies.iter().any(|e| {
                e.boss
                    .and_then(|i| run.bosses.get(i as usize))
                    .is_some_and(|b| {
                        matches!(
                            b.id.as_str(),
                            crate::rules::signatures::RIPPER
                                | crate::rules::signatures::ENGINE
                        )
                    })
            }))
}
pub(super) fn shot_supported(p: &crate::rules::Projectile) -> bool {
    combat::shot_supported(p)
}
pub(super) use figures::knight_supported;
/// A figure from the arena's own drawing as this canvas paints a class.
#[cfg(any(test, feature = "testing"))]
pub fn hd_figure(im: &Img) -> Img {
    figures::lit(&figures::scale3x(im))
}
pub(super) fn foe_supported(run: &Run, e: &crate::rules::Enemy) -> bool {
    combat::supported(run, e)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    raid: u64,
    depth: u32,
    pack: Pack,
    room: usize,
    print: u64,
    barred: bool,
    origin: (i32, i32),
    home: u64,
    wishes: String,
    course: Option<(u64, usize)>,
}
fn key(run: &Run, origin: (i32, i32)) -> Key {
    Key {
        raid: run.raid_id,
        depth: run.dungeon.depth,
        pack: run.dungeon.pack,
        room: run.at,
        print: run.room().fingerprint(),
        barred: run.barred(),
        origin,
        home: home_print(run),
        course: run.chariot.as_ref().map(|r| (r.seed, r.track)),
        wishes: if run.room().kind == RoomKind::Kingdom {
            serde_json::to_string(&run.kingdom.wishes).unwrap_or_default()
        } else {
            String::new()
        },
    }
}
type Cache = Mutex<Vec<(Key, Arc<scenes::Scenery>)>>;
fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn cached(key: &Key) -> Option<Arc<scenes::Scenery>> {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, scene)| scene.clone())
}
fn insert(key: Key, scene: Arc<scenes::Scenery>) {
    let mut entries = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, old)) = entries.iter_mut().find(|(k, _)| *k == key) {
        *old = scene;
        return;
    }
    if entries.len() == 4 {
        entries.remove(0);
    }
    entries.push((key, scene));
}
/// Four bounded static regions, no tick in the key. Snapshot bakes are
/// synchronous and exact. Neither scheduling nor eviction changes their pixels.
fn scenery(run: &Run, origin: (i32, i32)) -> Arc<scenes::Scenery> {
    let key = key(run, origin);
    if let Some(scene) = cached(&key) {
        return scene;
    }
    let scene = Arc::new(scenes::bake(run, origin));
    insert(key, scene.clone());
    scene
}
#[cfg(not(target_arch = "wasm32"))]
fn pending() -> &'static Mutex<Vec<Key>> {
    static PENDING: OnceLock<Mutex<Vec<Key>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}
#[cfg(not(target_arch = "wasm32"))]
type BakeJob = Box<dyn FnOnce() + Send>;
#[cfg(not(target_arch = "wasm32"))]
fn bake_sender() -> &'static std::sync::mpsc::SyncSender<BakeJob> {
    static SENDER: OnceLock<std::sync::mpsc::SyncSender<BakeJob>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<BakeJob>(4);
        std::thread::Builder::new()
            .name("delve-art-bake".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job();
                }
            })
            .expect("start art bake worker");
        tx
    })
}
/// At most four admitted bakes on one worker. While a room is cold, live
/// transport paints its existing v1 fallback and admits the v2 frame when ready.
#[cfg(not(target_arch = "wasm32"))]
fn scenery_live(run: &Run, origin: (i32, i32)) -> Option<Arc<scenes::Scenery>> {
    let key = key(run, origin);
    if let Some(scene) = cached(&key) {
        return Some(scene);
    }
    {
        let mut queue = pending().lock().unwrap_or_else(|e| e.into_inner());
        if queue.contains(&key) || queue.len() >= 4 {
            return None;
        }
        queue.push(key.clone());
    }
    let job_key = key.clone();
    let run = run.clone();
    if bake_sender()
        .try_send(Box::new(move || {
            let scene = Arc::new(scenes::bake(&run, origin));
            insert(job_key.clone(), scene);
            pending()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|k| *k != job_key);
        }))
        .is_err()
    {
        pending()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|k| *k != key);
    }
    None
}

/// A wasm build has no threads: there a cold room is baked at once.
#[cfg(target_arch = "wasm32")]
fn scenery_live(run: &Run, origin: (i32, i32)) -> Option<Arc<scenes::Scenery>> {
    Some(scenery(run, origin))
}

pub fn frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    if w < NATIVE_W || h < NATIVE_H {
        return super::frame_v1(run, w, h, focus);
    }
    compose(run, focus).map_or_else(|| super::frame_v1(run, w, h, focus), |cv| fit(&cv, w, h))
}
/// Full art layers serve native transport and the actual terminal cell painter.
pub fn terminal_layers(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    compose(run, focus).map(|cv| cv.layers())
}
pub fn terminal_layers_live(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    super::during_live(|| terminal_layers(run, focus))
}
fn compose(run: &Run, focus: Option<u32>) -> Option<Canvas> {
    if !supported(run) {
        return None;
    }
    if run.chariot.is_some() {
        return race::compose(run);
    }
    let origin = super::view_origin(run, focus);
    let anchor = if run.room().kind == RoomKind::Kingdom {
        (origin.0.div_euclid(64) * 64, origin.1.div_euclid(64) * 64)
    } else {
        (0, 0)
    };
    let scene = if super::live_frame() {
        let Some(scene) = scenery_live(run, anchor) else {
            return None;
        };
        scene
    } else {
        scenery(run, anchor)
    };
    let mut cv = if origin == anchor && scene.canvas.w == 768 && scene.canvas.h == 448 {
        scene.canvas.clone()
    } else {
        scene.canvas.view(origin)
    };
    // All dynamics use world-space fractional positions; the scene's camera
    // origin is shared with v1. A half world pixel is a real new art sample.
    combat::under(&mut cv, run);
    for e in &run.enemies {
        if foe_supported(run, e) {
            let (x, y) = (e.x * UNIT, e.y * UNIT);
            lit_figure(
                &mut cv,
                (x - 11.0, y - 22.0, 23.0, 25.0),
                &scene.lights,
                |cv| combat::foe(cv, run, e),
            );
        }
    }
    if run.room().kind == RoomKind::Pasture {
        for (i, ((x, y), east)) in crate::rules::ranch::grazers(run.tick)
            .into_iter()
            .enumerate()
        {
            figures::horse(
                &mut cv,
                x * 16.0,
                y * 16.0,
                east,
                run.tick + i as u64 * 13,
                false,
                false,
                i as u32,
            );
        }
        let ((x, y), east, gait) = crate::rules::ranch::helios(run.tick);
        figures::horse(
            &mut cv,
            x * 16.0,
            y * 16.0,
            east,
            run.tick,
            true,
            gait.is_some(),
            0,
        );
    }
    if run.room().kind == RoomKind::Ranch {
        race::ranch_horses(&mut cv, run);
    }
    for (&id, hero) in &run.players {
        if hero.hp == 0 || !knight_supported(run, hero, id) {
            continue;
        }
        let blink = hero.invulnerable > 0
            && hero.invulnerable < run.tune.hit_ward_ticks().max(1)
            && (run.tick / 2).is_multiple_of(2)
            && !strike::whiteout(hero);
        if blink {
            continue;
        }
        let (x, y) = (hero.x * UNIT, hero.y * UNIT);
        if x < (origin.0 - 24) as f32
            || x > (origin.0 + NATIVE_W + 24) as f32
            || y < (origin.1 - 24) as f32
            || y > (origin.1 + NATIVE_H + 24) as f32
        {
            continue;
        }
        lit_figure(
            &mut cv,
            // Tall enough for a Mage's hat and a raised staff.
            (x - 18.0, y - 38.0, 36.0, 40.0),
            &scene.lights,
            |cv| figures::knight(cv, x, y, hero, run.tick, id, run.shield_out(id)),
        );
        // The same shield/aim and gameplay marks are drawn by the ink overlay.
    }
    for (i, &(x, y)) in scene.fires.iter().enumerate() {
        flame(&mut cv, x, y, run.tick, i as u32);
    }
    atmosphere(&mut cv, run);

    // Bloom stays local to emissives; no convolution touches gameplay edges.
    for shot in run.projectiles.iter().take(384) {
        cv.effect_glow(Point::new(
            shot.x * UNIT,
            shot.y * UNIT,
            3.5,
            if shot.hostile { '6' } else { '3' },
            0.12,
        ));
    }
    combat::shots(&mut cv, run);
    let (_, over) = super::scene_layers_with(run, focus, true, true);
    cv.origin = (0.0, 0.0);
    ui::stamp_overlay(&mut cv, &over);
    ui::result(&mut cv, run, focus);
    ui::votes(&mut cv, run, focus);
    Some(cv)
}
/// Relight changed body samples in a small rectangle. Unchanged scenery
/// does not receive a second light pass, and cues are painted afterward.
fn lit_figure(
    cv: &mut Canvas,
    bounds: (f32, f32, f32, f32),
    lights: &[Point],
    draw: impl FnOnce(&mut Canvas),
) {
    let (x, y, w, h) = bounds;
    let x0 = ((x - cv.origin.0) * 2.0).floor().clamp(0.0, cv.w as f32) as usize;
    let y0 = ((y - cv.origin.1) * 2.0).floor().clamp(0.0, cv.h as f32) as usize;
    let x1 = ((x + w - cv.origin.0) * 2.0).ceil().clamp(0.0, cv.w as f32) as usize;
    let y1 = ((y + h - cv.origin.1) * 2.0).ceil().clamp(0.0, cv.h as f32) as usize;
    let mut before = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for yy in y0..y1 {
        before.extend_from_slice(&cv.px[yy * cv.w + x0..yy * cv.w + x1]);
    }
    draw(cv);
    let light = crate::render::art::irradiance(lights, x + w * 0.5, y + h * 0.5);
    for yy in y0..y1 {
        for xx in x0..x1 {
            let i = yy * cv.w + xx;
            if cv.px[i] != before[(yy - y0) * (x1 - x0) + xx - x0] {
                cv.px[i] = crate::render::art::grade_actor(cv.px[i], light);
            }
        }
    }
}
fn flame(cv: &mut Canvas, x: f32, y: f32, tick: u64, seed: u32) {
    let phase = (tick % 24) as f32 / 24.0 * std::f32::consts::TAU;
    let sway = (phase + seed as f32 * 1.7).sin() * 0.65;
    cv.ellipse(
        x + sway,
        y - 1.5,
        2.2,
        4.0,
        paint::Ramp::inks('@', '6', '9'),
        seed,
    );
    cv.ellipse(
        x - sway * 0.2,
        y,
        1.0,
        2.1,
        paint::Ramp::inks('6', '9', 'w'),
        seed + 1,
    );
    cv.glow(Point::new(x, y, 10.0, '6', 0.25));
}
fn atmosphere(cv: &mut Canvas, run: &Run) {
    let outside = matches!(
        run.room().kind,
        RoomKind::Pasture | RoomKind::Ranch | RoomKind::Kingdom
    );
    let (ox, oy) = cv.origin;
    let tick = (run.tick % 65536) as f32;
    if run.room().kind == RoomKind::Kingdom {
        cv.shaft(
            (ox + 335.0, oy - 10.0),
            (ox + 227.0, oy + 174.0),
            36.0,
            material('U'),
            0.05,
        );
    }
    for i in 0..32 {
        let v = hash(i, run.at as i32, 77);
        let x = (v % 768) as f32 * 0.5 + tick * if outside { 0.12 } else { 0.025 };
        let y = ((v >> 10) % 448) as f32 * 0.5 - tick * 0.035;
        let px = ox + x.rem_euclid(384.0);
        let py = oy + y.rem_euclid(224.0);
        let shimmer = (tick * 0.06 + i as f32).sin() * 0.13 + 0.3;
        cv.mist(
            px,
            py,
            if i % 7 == 0 { 0.65 } else { 0.35 },
            0.35,
            material(if outside { 'T' } else { 'U' }),
            shimmer,
        );
    }
    // Shallow translucent mist drifts across distant planes, behind live cues.
    for i in 0..4 {
        let phase = tick * 0.004 + i as f32 * 1.7;
        cv.mist(
            ox + 70.0 + i as f32 * 87.0 + phase.sin() * 7.0,
            oy + 30.0 + i as f32 * 15.0,
            63.0,
            4.0,
            material('U'),
            if outside { 0.045 } else { 0.035 },
        );
    }
}
fn fit(cv: &Canvas, w: i32, h: i32) -> Img {
    if w == WIDTH && h == HEIGHT {
        return cv.image();
    }
    let image = cv.image();
    let scale = (w as f32 / WIDTH as f32).min(h as f32 / HEIGHT as f32);
    let (tw, th) = (
        (WIDTH as f32 * scale).round() as usize,
        (HEIGHT as f32 * scale).round() as usize,
    );
    let (ox, oy) = ((w as usize - tw) / 2, (h as usize - th) / 2);
    let mut out = vec![BLACK; (w * h) as usize];
    for y in 0..th {
        for x in 0..tw {
            let sx = (x * cv.w / tw).min(cv.w - 1);
            let sy = (y * cv.h / th).min(cv.h - 1);
            out[(y + oy) * w as usize + x + ox] = image.get(sx as i32, sy as i32).unwrap_or(BLACK);
        }
    }
    Img::from_rgb(w, h, &out)
}

/// The mini-viz walk shares the kingdom's actual geometry and material/light kit.
/// Cache whole world regions, excluding animation and moving actor positions.
type WalkKey = (String, (i32, i32), usize, usize);
type WalkCache = Mutex<Vec<(WalkKey, Arc<scenes::Scenery>)>>;
fn walk_cache() -> &'static WalkCache {
    static CACHE: OnceLock<WalkCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
#[cfg(not(target_arch = "wasm32"))]
fn walk_pending() -> &'static Mutex<Vec<WalkKey>> {
    static PENDING: OnceLock<Mutex<Vec<WalkKey>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}
fn walk_insert(key: WalkKey, scene: Arc<scenes::Scenery>) {
    let mut cache = walk_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, s)) = cache.iter_mut().find(|(k, _)| *k == key) {
        *s = scene;
        return;
    }
    if cache.len() == 4 {
        cache.remove(0);
    }
    cache.push((key, scene));
}
/// The walk's live bake: admitted to the baker, the frame waits for it.
#[cfg(not(target_arch = "wasm32"))]
fn walk_bake_live(
    key: WalkKey,
    wishes: Arc<Vec<crate::realm::Wish>>,
    origin: (i32, i32),
    w: usize,
    h: usize,
) -> Option<Arc<scenes::Scenery>> {
    let mut queue = walk_pending().lock().unwrap_or_else(|e| e.into_inner());
    if queue.contains(&key) || queue.len() >= 4 {
        return None;
    }
    queue.push(key.clone());
    drop(queue);
    let job_key = key.clone();
    if bake_sender()
        .try_send(Box::new(move || {
            let scene = Arc::new(scenes::bake_walk(&wishes, origin, w, h));
            walk_insert(job_key.clone(), scene);
            walk_pending()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|k| *k != job_key);
        }))
        .is_err()
    {
        walk_pending()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|k| *k != key);
    }
    None
}
/// A wasm build has no threads: there the live walk is baked at once.
#[cfg(target_arch = "wasm32")]
fn walk_bake_live(
    key: WalkKey,
    wishes: Arc<Vec<crate::realm::Wish>>,
    origin: (i32, i32),
    w: usize,
    h: usize,
) -> Option<Arc<scenes::Scenery>> {
    let s = Arc::new(scenes::bake_walk(&wishes, origin, w, h));
    walk_insert(key, s.clone());
    Some(s)
}
pub fn walk_layers(scene: &super::super::Scene, view: super::super::View) -> (Img, Img) {
    walk_layers_inner(scene, view, false).expect("exact walk bake")
}
pub fn walk_layers_live(
    scene: &super::super::Scene,
    view: super::super::View,
) -> Option<(Img, Img)> {
    walk_layers_inner(scene, view, true)
}
fn walk_layers_inner(
    scene: &super::super::Scene,
    view: super::super::View,
    live: bool,
) -> Option<(Img, Img)> {
    let origin = (view.x.div_euclid(64) * 64, view.y.div_euclid(64) * 64);
    let (w, h) = (
        ((view.w.max(1) + 127) / 64 * 64 * 2) as usize,
        ((view.h.max(1) + 127) / 64 * 64 * 2) as usize,
    );
    let key = (
        serde_json::to_string(&scene.wishes).unwrap_or_default(),
        origin,
        w,
        h,
    );
    let cached = walk_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, s)| s.clone());
    let scenery = if let Some(s) = cached {
        s
    } else if live {
        walk_bake_live(key, scene.wishes.clone(), origin, w, h)?
    } else {
        let s = Arc::new(scenes::bake_walk(&scene.wishes, origin, w, h));
        walk_insert(key, s.clone());
        s
    };
    let mut cv = Canvas::new(
        view.w.max(1) as usize * 2,
        view.h.max(1) as usize * 2,
        [0, 0, 0],
    );
    cv.origin = (view.x as f32, view.y as f32);
    let (dx, dy) = (
        (view.x - origin.0) as usize * 2,
        (view.y - origin.1) as usize * 2,
    );
    for y in 0..cv.h {
        let from = (y + dy) * scenery.canvas.w + dx;
        let to = y * cv.w;
        cv.px[to..to + cv.w].copy_from_slice(&scenery.canvas.px[from..from + cv.w]);
        cv.fx[to..to + cv.w].copy_from_slice(&scenery.canvas.fx[from..from + cv.w]);
    }
    // Reuse the party figure standard at the published feet, independent of camera.
    static HERO: OnceLock<crate::rules::Hero> = OnceLock::new();
    let mut hero = HERO
        .get_or_init(|| Run::new(0, 1, None).players[&1].clone())
        .clone();
    hero.stride = if scene.knight.walking {
        scene.tick as f32 * 0.25
    } else {
        0.0
    };
    let mut actors = vec![(scene.knight.x, scene.knight.y, 1)];
    actors.extend(
        scene
            .party
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| (x, y, i as u32 + 2)),
    );
    if let Some(v) = scene.visitor {
        hero.aim_x = if v.facing_left { -1.0 } else { 1.0 };
        actors.push((v.x, v.y, 4));
    }
    for &(x, y, id) in &actors {
        lit_figure(
            &mut cv,
            (x - 12.0, y - 22.0, 26.0, 24.0),
            &scenery.lights,
            |cv| figures::knight(cv, x, y, &hero, scene.tick as u64, id, false),
        );
    }
    // Retain published activity/status cues; never invent work or outcomes.
    let staged = super::super::scene::stage(scene);
    // Keep the live realm's traffic, work, growth and outcome props. The
    // resting architecture already has v2 surfaces; compare authored props
    // with its baseline so active states cannot silently disappear.
    static REST: OnceLock<super::super::scene::Stage> = OnceLock::new();
    let rest = REST.get_or_init(|| super::super::scene::stage(&super::super::Scene::resting()));
    for prop in &staged.props {
        if let Some(old) = rest
            .props
            .iter()
            .find(|p| p.x == prop.x && p.base == prop.base)
        {
            if old.img == prop.img {
                continue;
            }
            if prop.img.w >= 24 && prop.img.h >= 28 {
                // V2 owns the structural silhouette. Published window/work
                // changes become local emission deltas, never an old building
                // painted back over the shared architecture.
                let mut delta = Img::new(prop.img.w, prop.img.h);
                for y in 0..prop.img.h {
                    for x in 0..prop.img.w {
                        if let Some(after) = prop.img.get(x, y) {
                            let before = old.img.get(x, y).unwrap_or(BLACK);
                            let c = std::array::from_fn(|k| {
                                after[k].saturating_sub(before[k]).saturating_mul(2)
                            });
                            if c.iter().copied().max().unwrap_or(0) > 28 {
                                delta.set(x, y, c);
                            }
                        }
                    }
                }
                std::mem::swap(&mut cv.px, &mut cv.fx);
                cv.stamp(&delta, prop.x as f32, (prop.base - prop.img.h) as f32, true);
                std::mem::swap(&mut cv.px, &mut cv.fx);
                continue;
            }
        }
        if actors.iter().any(|&(x, y, _)| {
            (prop.x as f32 + prop.img.w as f32 * 0.5 - x).abs() < 12.0
                && (prop.base as f32 - y).abs() < 6.0
                && prop.img.h < 40
        }) {
            continue;
        }
        cv.stamp(
            &prop.img,
            prop.x as f32,
            (prop.base - prop.img.h) as f32,
            false,
        );
    }
    for light in &staged.lights {
        cv.glow(Point::new(
            light.x,
            light.y,
            light.r * 0.5,
            if light.fire { '6' } else { 'U' },
            light.s * 0.65,
        ));
    }
    for cue in staged.cues {
        cv.stamp(
            &cue.img,
            (cue.x - view.x) as f32 + cv.origin.0,
            (cue.base - cue.img.h - view.y) as f32 + cv.origin.1,
            true,
        );
    }
    Some(cv.layers())
}
pub fn walk_frame(scene: &super::super::Scene, view: super::super::View) -> Img {
    let (base, fx) = walk_layers(scene, view);
    let native = crate::render::art::native(&base, &fx);
    // Mini-viz's world pixel scale is unchanged; the kit's two samples become one.
    let mut out = Img::black(view.w, view.h);
    for y in 0..view.h {
        for x in 0..view.w {
            out.set(x, y, native.get(x * 2, y * 2).unwrap_or(BLACK));
        }
    }
    if let Some(overlay) = &scene.overlay {
        overlay.paint_top(&mut out, scene, view);
    }
    out
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__art_v2_tests.rs"]
mod tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../../../../tests/render/overworld__art_v2__fixtures.rs"]
pub mod testing;

#[cfg(test)]
pub(super) fn fx_shot_sample(kind: crate::rules::Shot, phase: u32) -> Img {
    let mut run = Run::new(7, 1, None);
    let shot = serde_json::from_value(serde_json::json!({
        "x":7.0,"y":7.0,"vx":1.0,"vy":phase as f32*0.2,"hostile":false,
        "kind":kind,"damage":1,"pierce":0,"ttl":100
    })).unwrap();
    run.projectiles=vec![shot]; run.tick=u64::from(phase*2);
    let mut cv = paint::Canvas::new(224,224,[0;3]);
    combat::shots(&mut cv,&run);
    cv.image()
}

#[cfg(test)]
pub(super) fn fx_boss_sample(run:&Run)->Img {
    let mut cv=paint::Canvas::new(768,448,[0;3]);
    combat::under(&mut cv,run);
    let mut im=Img::new(384,224);
    super::signatures::under_with(&mut im,run,true);
    super::works::under_with(&mut im,run,true);
    ui::stamp_overlay(&mut cv,&im);
    cv.image()
}

#[cfg(test)]
pub(super) fn fx_hit_sample(phase:u32)->Img {
    let r=Run::new(7,1,None);
    let mut hero=r.players[&1].clone();
    hero.struck=crate::rules::spectacle::STRUCK-phase*2;
    let mut cv=paint::Canvas::new(224,224,[0;3]);
    figures::knight(&mut cv,56.0,64.0,&hero,0,1,false);
    cv.image()
}

#[cfg(test)]
pub(super) fn foe_hit_sample(run: &Run) -> Img {
    let mut cv = paint::Canvas::new(320, 224, [0; 3]);
    cv.origin = (
        run.enemies[0].x * UNIT - 80.0,
        run.enemies[0].y * UNIT - 80.0,
    );
    combat::foe(&mut cv, run, &run.enemies[0]);
    cv.image()
}
