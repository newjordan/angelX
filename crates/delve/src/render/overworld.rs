//! The overworld — the realm as a top-down pixel map.
//!
//! The map is the world's base layer: a Zelda-1 field of 16x11-tile screens
//! drawn on black paper (black ground, sparse marks, figures laid over it)
//! in the realm master palette. There is no HUD: every pixel is the world,
//! and anything added later has to earn its place. The true-3D Dotmax world,
//! authored room plates and generated images or video are modalities layered
//! over this map; this module owns only the map itself.
//!
//! Mood: the realm sits at dusk. Colours step down their own palette bank
//! with Bayer dither, light means activity (only live places glow), and the
//! signal bank — reserved for game state — never dims.
//!
//! Contract: pure and deterministic. The same [`Scene`] renders the same
//! bytes: no clocks, no ambient randomness, no map iteration order.

#![cfg_attr(not(test), allow(dead_code))]

pub mod arena;
pub mod castles;
pub mod chrome;
pub mod college;
pub mod clerks;
pub mod deeds;
pub mod glass;
pub mod ground;
pub mod ink;
pub mod kit;
pub mod kingdom;
pub mod landmarks;
pub mod light;
pub mod material;
pub mod outcomes;
pub mod scene;
pub mod school;
pub mod sky;
pub mod wishes;

// The realm's map is the world's; the drawing reads it here.
pub use crate::world::map;

#[cfg(any(test, feature = "testing"))]
pub use glass::{GLASS_H, GLASS_W, picture_from_rgba};
pub use ink::Img;
pub use map::{MAP_H, MAP_W, SCREEN_H, SCREEN_W, TILE};
pub use scene::{Motion, Overlay, Scene, SoldierKind, SoldierState, VisitorPose};

use kit::{RockKind, Tiles};
use map::Realm;

/// A rectangle of the realm in world pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl View {
    /// Zelda screen `(sx, sy)`.
    pub fn screen(sx: i32, sy: i32) -> View {
        View {
            x: sx * SCREEN_W * TILE,
            y: sy * SCREEN_H * TILE,
            w: SCREEN_W * TILE,
            h: SCREEN_H * TILE,
        }
    }

    /// The screen holding a world point.
    pub fn screen_at(x: f32, y: f32) -> View {
        let sx = ((x as i32).div_euclid(SCREEN_W * TILE)).clamp(0, map::SCREENS_X - 1);
        let sy = ((y as i32).div_euclid(SCREEN_H * TILE)).clamp(0, map::SCREENS_Y - 1);
        View::screen(sx, sy)
    }

    /// The whole realm.
    pub fn realm() -> View {
        View {
            x: 0,
            y: 0,
            w: MAP_W * TILE,
            h: MAP_H * TILE,
        }
    }

    /// A `w`x`h` view centred on `(cx, cy)` and kept inside the realm. On
    /// an axis where the view is larger than the realm, the realm sits in the
    /// middle on black paper.
    pub fn around(cx: f32, cy: f32, w: i32, h: i32) -> View {
        let place = |c: f32, span: i32, world: i32| {
            if span >= world {
                (world - span) / 2
            } else {
                ((c - span as f32 / 2.0).round() as i32).clamp(0, world - span)
            }
        };
        let (w, h) = (w.max(1), h.max(1));
        View {
            x: place(cx, w, MAP_W * TILE),
            y: place(cy, h, MAP_H * TILE),
            w,
            h,
        }
    }

    fn touches(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        x < self.x + self.w && x + w > self.x && y < self.y + self.h && y + h > self.y
    }
}

fn rock_kind(tx: i32, ty: i32) -> RockKind {
    let (sx, sy) = (tx / SCREEN_W, ty / SCREEN_H);
    match (sx, sy) {
        (2, 0) | (3, 0) | (2, 1) | (3, 1) => RockKind::Ore,
        (4, 0) | (5, 0) | (4, 1) | (5, 1) => RockKind::Char,
        _ => RockKind::Slate,
    }
}

/// One screen of the realm around the newest wish the party raised, or
/// around the Keep while none stands yet — what a guest page shows.
pub fn wish_view(scene: &Scene) -> Img {
    let lots = wishes::lots(&scene.wishes);
    let (cx, cy) = lots
        .iter()
        .rev()
        .flatten()
        .next()
        .map(|lot| {
            (
                ((lot.tx * 2 + lot.tw) * TILE / 2) as f32,
                ((lot.ty * 2 + lot.th) * TILE / 2) as f32,
            )
        })
        .unwrap_or_else(|| {
            let (tx, ty) = map::Place::Keep.stand_world();
            ((tx * TILE) as f32, (ty * TILE) as f32)
        });
    render_view(
        scene,
        View::around(cx, cy, SCREEN_W * TILE, SCREEN_H * TILE),
    )
}

/// Render a rectangle of the lit realm.
pub fn render_view(scene: &Scene, view: View) -> Img {
    kingdom::landscape(scene, view)
}

fn render_landscape(scene: &Scene, view: View, extra_lots: &[bool], roads: &[bool]) -> Img {
    let realm = Realm::get();
    let tiles = Tiles::get();
    let mut cv = Img::black(view.w, view.h);
    let (realm_w, realm_h) = (MAP_W * TILE, MAP_H * TILE);
    for y in 0..view.h {
        for x in 0..view.w {
            let (wx, wy) = (view.x + x, view.y + y);
            if wx < 0 || wy < 0 || wx >= realm_w || wy >= realm_h {
                continue;
            }
            let (tx, ty) = (wx / TILE, wy / TILE);
            let road = roads
                .get((ty * MAP_W + tx) as usize)
                .copied()
                .unwrap_or(false);
            let cleared = extra_lots
                .get((ty * MAP_W + tx) as usize)
                .copied()
                .unwrap_or(false);
            let mark = if road {
                if matches!(realm.at(tx, ty), b'~' | b'w' | b'f' | b'H') {
                    Some(ground::planks(wx, wy))
                } else {
                    ground::lane(wx, wy, |dx, dy| {
                        let (x, y) = (tx + dx, ty + dy);
                        matches!(realm.at(x, y), b'=' | b':' | b'H')
                            || roads.get((y * MAP_W + x) as usize).copied().unwrap_or(false)
                    })
                }
            } else if cleared {
                ground::meadow(wx, wy)
            } else {
                ground::mark(realm, wx, wy, scene.tick)
            };
            if let Some(ch) = mark {
                cv.put(x, y, ch);
            }
            if let Some(ch) = college::approach_mark(wx, wy) {
                cv.put(x, y, ch);
            }
        }
    }
    if let Some(overlay) = &scene.overlay {
        overlay.paint_ground(&mut cv, view);
    }
    let (tx0, ty0) = (
        view.x.div_euclid(TILE).max(0),
        view.y.div_euclid(TILE).max(0),
    );
    let tx1 = (view.x + view.w).div_euclid(TILE).min(MAP_W - 1);
    // Lots cleared for wishes the party paid for show bare ground.
    let lots = wishes::lots(&scene.wishes);
    let ty1 = (view.y + view.h).div_euclid(TILE).min(MAP_H - 1);
    // One tile past the view below and to each side: crowns overhang.
    for ty in ty0..=(ty1 + 1).min(MAP_H - 1) {
        for tx in (tx0 - 1).max(0)..=(tx1 + 1).min(MAP_W - 1) {
            if wishes::cleared(&lots, tx, ty)
                || college::lot().contains(tx, ty)
                || college::approach().contains(&(tx, ty))
                || roads
                    .get((ty * MAP_W + tx) as usize)
                    .copied()
                    .unwrap_or(false)
                || extra_lots
                    .get((ty * MAP_W + tx) as usize)
                    .copied()
                    .unwrap_or(false)
            {
                continue;
            }
            let v = ink::hash(tx, ty, 31);
            let sprite = match realm.at(tx, ty) {
                b'T' => tiles.tree(v),
                b'^' => tiles.rock(rock_kind(tx, ty), v),
                b'd' => &tiles.dead,
                _ => continue,
            };
            // Tree crowns stand taller and wider than their tile.
            if sprite.w > TILE {
                // Clipped to the realm: past its edge stays black paper.
                let (ox, oy) = (tx * TILE + (TILE - sprite.w) / 2, ty * TILE + TILE - sprite.h);
                for sy in 0..sprite.h {
                    for sx in 0..sprite.w {
                        let (wx, wy) = (ox + sx, oy + sy);
                        if wx < 0 || wy < 0 || wx >= MAP_W * TILE || wy >= MAP_H * TILE {
                            continue;
                        }
                        if let Some(c) = sprite.get(sx, sy) {
                            cv.set(wx - view.x, wy - view.y, c);
                        }
                    }
                }
                continue;
            }
            cv.stamp(sprite, tx * TILE - view.x, ty * TILE - view.y);
        }
    }
    let mut staged = scene::stage(scene);
    lava_glow(&mut staged.lights, view, scene.tick);
    staged.props.sort_by_key(|p| p.base);
    for p in &staged.props {
        let top = p.base - p.img.h;
        if view.touches(p.x, top, p.img.w, p.img.h) {
            cv.stamp(&p.img, p.x - view.x, top - view.y);
        }
    }
    light::dusk(
        &mut cv,
        realm,
        &staged.lights,
        (view.x, view.y),
        scene.ambient,
    );
    sky::weather(&mut cv, scene.weather, scene.tick, (view.x, view.y));
    if scene.fireworks {
        sky::fireworks(&mut cv, scene.tick, (view.x, view.y));
    }
    for &(x, y, w, h) in &staged.beacons {
        kit::beacon(&mut cv, (x - view.x, y - view.y, w, h), scene.tick);
    }
    for c in &staged.cues {
        cv.stamp(&c.img, c.x - view.x, c.base - c.img.h - view.y);
    }
    if let Some(overlay) = &scene.overlay {
        overlay.paint_top(&mut cv, scene, view);
    }
    cv
}

/// Lava warms what is near it: one breathing firelight per 2x2 block of
/// lava tiles touching the view.
fn lava_glow(lights: &mut Vec<light::Light>, view: View, tick: u32) {
    let realm = Realm::get();
    let margin = 3 * TILE;
    let (bx0, by0) = (
        (view.x - margin).div_euclid(TILE * 2),
        (view.y - margin).div_euclid(TILE * 2),
    );
    let (bx1, by1) = (
        (view.x + view.w + margin).div_euclid(TILE * 2),
        (view.y + view.h + margin).div_euclid(TILE * 2),
    );
    for by in by0.max(0)..=by1.min(MAP_H / 2) {
        for bx in bx0.max(0)..=bx1.min(MAP_W / 2) {
            let molten = (0..2)
                .flat_map(|dy| (0..2).map(move |dx| (bx * 2 + dx, by * 2 + dy)))
                .filter(|&(tx, ty)| {
                    tx < MAP_W && ty < MAP_H && matches!(realm.at(tx, ty), b'L' | b'F')
                })
                .count();
            if molten > 0 {
                let breath = 0.9 + 0.1 * (tick as f32 * 0.4 + (bx * 7 + by * 3) as f32).sin();
                lights.push(light::Light {
                    x: ((bx * 2 + 1) * TILE) as f32,
                    y: ((by * 2 + 1) * TILE) as f32,
                    r: 34.0,
                    s: (0.14 + 0.07 * molten as f32) * breath,
                    fire: true,
                });
            }
        }
    }
}

/// A frame: one view of the map, rim vignetted, with any glass laid over.
/// Nothing else: every pixel in the frame is the world.
pub fn frame_at(scene: &Scene, view: View) -> Img {
    if let Some(room) = scene.school_room {
        return school::render_room(
            room,
            scene.school,
            scene.tick,
            scene.outcome_motion,
            view.w,
            view.h,
        );
    }
    let mut f = if arena::art_v2::enabled() {arena::art_v2::walk_frame(scene,view)} else {render_view(scene, view)};
    light::vignette(&mut f, 0);
    if let Some(g) = &scene.glass {
        let knight = (scene.knight.x, scene.knight.y);
        glass::draw(
            &mut f,
            g,
            (view.x, view.y),
            0,
            scene.tick,
            knight,
            scene.active,
        );
    }
    f
}

/// A frame on the screen that holds the knight.
pub fn frame(scene: &Scene) -> Img {
    frame_at(scene, View::screen_at(scene.knight.x, scene.knight.y))
}

/// [`frame`] memoized on the scene key for the drawing thread: a paced scene
/// changes a few times a second, the terminal redraws far more often.
pub fn frame_cached(scene: &Scene, w: i32, h: i32) -> std::rc::Rc<Img> {
    thread_local! {
        static LAST: std::cell::RefCell<Option<(u64, std::rc::Rc<Img>)>> =
            const { std::cell::RefCell::new(None) };
    }
    let key = scene.key() ^ u64::from(arena::art_v2::enabled()).rotate_left(59) ^ ((w as u64) << 40 | (h as u64) << 20).rotate_left(7);
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        if let Some((k, img)) = last.as_ref()
            && *k == key
        {
            return std::rc::Rc::clone(img);
        }
        let img = std::rc::Rc::new(frame_sized(scene, w, h));
        *last = Some((key, std::rc::Rc::clone(&img)));
        img
    })
}

/// Frame size in pixels: one Zelda screen.
pub const FRAME_W: u32 = (SCREEN_W * TILE) as u32;
pub const FRAME_H: u32 = (SCREEN_H * TILE) as u32;

/// A `w`x`h` frame around the scene's camera. The camera never zooms: a
/// larger pane shows more of the realm at the same scale.
pub fn frame_sized(scene: &Scene, w: i32, h: i32) -> Img {
    frame_at(scene, view_sized(scene, w, h))
}

/// The rectangle of the realm a `w`x`h` frame around the scene's camera
/// shows: where a click on the map lands.
pub fn view_sized(scene: &Scene, w: i32, h: i32) -> View {
    View::around(scene.camera.0, scene.camera.1, w, h)
}

/// Screen pixels per map pixel for a terminal cell height: the map keeps its
/// size relative to the text (zooming the terminal out shows more realm),
/// in whole pixels so the art stays crisp.
pub fn map_scale(cell_h: u16) -> u32 {
    ((f32::from(cell_h) / 12.0).round() as u32).max(1)
}

/// Settle a live scene onto the map's own cadence. Ambient motion — water,
/// lava, breathing fires — steps about four times a second (twice while a
/// turn runs): slow and smooth, and an idle realm re-encodes rarely. The
/// knight, the visitor and the camera move in whole pixels.
pub fn pace(scene: &mut Scene, relaxed: bool) {
    scene.tick /= if relaxed { 20 } else { 10 };
    for echo in &mut scene.outcomes {
        echo.phase = if scene.outcome_motion == Motion::Full {
            if relaxed {
                echo.phase / 2 * 2
            } else {
                echo.phase
            }
        } else {
            0
        };
    }
    scene.knight.x = scene.knight.x.round();
    scene.knight.y = scene.knight.y.round();
    if let Some(v) = scene.visitor.as_mut() {
        (v.x, v.y) = (v.x.round(), v.y.round());
    }
    for w in &mut scene.wayfarers {
        (w.x, w.y) = (w.x.round(), w.y.round());
    }
    scene.camera = (scene.camera.0.round(), scene.camera.1.round());
}

/// A frame rendered on first use — the image worker, not the draw thread,
/// pays for the pixels.
pub struct LazyFrame {
    scene: Scene,
    view: (i32, i32),
    scale: u32,
    rgba: std::sync::OnceLock<Vec<u8>>,
}

impl LazyFrame {
    /// A `view_w`x`view_h` map frame, blown up `scale` times with whole
    /// pixels so the terminal can place it without resampling.
    pub fn new(scene: Scene, view_w: i32, view_h: i32, scale: u32) -> LazyFrame {
        LazyFrame {
            scene,
            view: (view_w.max(1), view_h.max(1)),
            scale: scale.max(1),
            rgba: std::sync::OnceLock::new(),
        }
    }

    /// Pixel size of the finished frame.
    pub fn size(&self) -> (u32, u32) {
        (
            self.view.0 as u32 * self.scale,
            self.view.1 as u32 * self.scale,
        )
    }
}

impl AsRef<[u8]> for LazyFrame {
    fn as_ref(&self) -> &[u8] {
        self.rgba.get_or_init(|| {
            frame_sized(&self.scene, self.view.0, self.view.1).rgba_scaled(self.scale)
        })
    }
}
