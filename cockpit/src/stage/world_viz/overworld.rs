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

mod glass;
mod ground;
mod ink;
mod kit;
mod light;
mod live;
mod map;
mod scene;
mod sky;

// The overworld tests read these through `use super::*`; live.rs imports glass directly.
#[cfg(test)]
pub(crate) use glass::{GLASS_H, GLASS_W, picture_from_rgba};
pub(crate) use ink::Img;
pub(crate) use live::{Duel, Walker, soldier_state};
pub(crate) use map::{MAP_H, MAP_W, SCREEN_H, SCREEN_W, TILE};
pub(crate) use scene::{Scene, SoldierKind, SoldierState};

#[cfg(test)]
use kit::Tool;
#[cfg(test)]
use map::Place;
#[cfg(test)]
use scene::{Joust, Knight, Soldier, Ward, Weather};

use kit::{RockKind, Tiles};
use map::Realm;

/// A rectangle of the realm in world pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

impl View {
    /// Zelda screen `(sx, sy)`.
    pub(crate) fn screen(sx: i32, sy: i32) -> View {
        View {
            x: sx * SCREEN_W * TILE,
            y: sy * SCREEN_H * TILE,
            w: SCREEN_W * TILE,
            h: SCREEN_H * TILE,
        }
    }

    /// The screen holding a world point.
    pub(crate) fn screen_at(x: f32, y: f32) -> View {
        let sx = ((x as i32).div_euclid(SCREEN_W * TILE)).clamp(0, map::SCREENS_X - 1);
        let sy = ((y as i32).div_euclid(SCREEN_H * TILE)).clamp(0, map::SCREENS_Y - 1);
        View::screen(sx, sy)
    }

    /// The whole realm.
    pub(crate) fn realm() -> View {
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
    pub(crate) fn around(cx: f32, cy: f32, w: i32, h: i32) -> View {
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
        (1, 0) => RockKind::Ore,
        (2, 0) => RockKind::Char,
        _ => RockKind::Slate,
    }
}

/// Render a rectangle of the lit realm.
pub(crate) fn render_view(scene: &Scene, view: View) -> Img {
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
            if let Some(ch) = ground::mark(realm, wx, wy) {
                cv.put(x, y, ch);
            }
        }
    }
    let (tx0, ty0) = (
        view.x.div_euclid(TILE).max(0),
        view.y.div_euclid(TILE).max(0),
    );
    let tx1 = (view.x + view.w).div_euclid(TILE).min(MAP_W - 1);
    let ty1 = (view.y + view.h).div_euclid(TILE).min(MAP_H - 1);
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            let v = ink::hash(tx, ty, 31);
            let sprite = match realm.at(tx, ty) {
                b'T' => tiles.tree(v),
                b'^' => tiles.rock(rock_kind(tx, ty), v),
                b'd' => &tiles.dead,
                _ => continue,
            };
            cv.stamp(sprite, tx * TILE - view.x, ty * TILE - view.y);
        }
    }
    let mut staged = scene::stage(scene);
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
    cv
}

/// A frame: one view of the map, rim vignetted, with any glass laid over.
/// Nothing else: every pixel in the frame is the world.
pub(crate) fn frame_at(scene: &Scene, view: View) -> Img {
    let mut f = render_view(scene, view);
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

/// Whether travel and arrival show their glass over the map
/// (`ANGEL_WORLD_GLASS=off` keeps the map bare).
pub(crate) fn glass_enabled() -> bool {
    std::env::var("ANGEL_WORLD_GLASS").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

/// A frame on the screen that holds the knight.
pub(crate) fn frame(scene: &Scene) -> Img {
    frame_at(scene, View::screen_at(scene.knight.x, scene.knight.y))
}

/// [`frame`] memoized on the scene key for the drawing thread: a paced scene
/// changes a few times a second, the terminal redraws far more often.
pub(crate) fn frame_cached(scene: &Scene, w: i32, h: i32) -> std::rc::Rc<Img> {
    thread_local! {
        static LAST: std::cell::RefCell<Option<(u64, std::rc::Rc<Img>)>> =
            const { std::cell::RefCell::new(None) };
    }
    let key = scene.key() ^ ((w as u64) << 40 | (h as u64) << 20).rotate_left(7);
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
pub(crate) const FRAME_W: u32 = (SCREEN_W * TILE) as u32;
pub(crate) const FRAME_H: u32 = (SCREEN_H * TILE) as u32;

/// A `w`x`h` frame around the scene's camera. The camera never zooms: a
/// larger pane shows more of the realm at the same scale.
pub(crate) fn frame_sized(scene: &Scene, w: i32, h: i32) -> Img {
    frame_at(scene, View::around(scene.camera.0, scene.camera.1, w, h))
}

/// Screen pixels per map pixel for a terminal cell height: the map keeps its
/// size relative to the text (zooming the terminal out shows more realm),
/// in whole pixels so the art stays crisp.
pub(crate) fn map_scale(cell_h: u16) -> u32 {
    ((f32::from(cell_h) / 12.0).round() as u32).max(1)
}

/// Whether the Realm route shows this map (the default) or the Dotmax 3D
/// ride as before (`ANGEL_WORLD_MAP=3d`).
pub(crate) fn map_enabled() -> bool {
    std::env::var("ANGEL_WORLD_MAP").map_or(true, |raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "3d" | "dotmax" | "off" | "0" | "false" | "no"
        )
    })
}

/// Settle a live scene onto the map's own cadence: animation steps about
/// seven times a second (half that while a turn runs) and the knight moves
/// in whole pixels, so an idle realm re-encodes rarely.
pub(crate) fn pace(scene: &mut Scene, relaxed: bool) {
    scene.tick /= if relaxed { 12 } else { 6 };
    scene.knight.x = scene.knight.x.round();
    scene.knight.y = scene.knight.y.round();
    scene.camera = (scene.camera.0.round(), scene.camera.1.round());
}

/// A frame rendered on first use — the image worker, not the draw thread,
/// pays for the pixels.
pub(crate) struct LazyFrame {
    scene: Scene,
    view: (i32, i32),
    scale: u32,
    rgba: std::sync::OnceLock<Vec<u8>>,
}

impl LazyFrame {
    /// A `view_w`x`view_h` map frame, blown up `scale` times with whole
    /// pixels so the terminal can place it without resampling.
    pub(crate) fn new(scene: Scene, view_w: i32, view_h: i32, scale: u32) -> LazyFrame {
        LazyFrame {
            scene,
            view: (view_w.max(1), view_h.max(1)),
            scale: scale.max(1),
            rgba: std::sync::OnceLock::new(),
        }
    }

    /// Pixel size of the finished frame.
    pub(crate) fn size(&self) -> (u32, u32) {
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

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__tests.rs"]
mod tests;
