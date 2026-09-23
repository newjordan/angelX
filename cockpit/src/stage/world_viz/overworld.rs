//! The overworld — the realm as a top-down pixel map.
//!
//! The map is the world's base layer: a Zelda-1 field of 16x11-tile screens
//! under a HUD, drawn on black paper (black ground, sparse marks, figures
//! laid over it) in the realm master palette. The true-3D Dotmax world,
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

mod ground;
mod hud;
mod ink;
mod kit;
mod light;
mod live;
mod map;
mod scene;

pub(crate) use hud::HUD_H;
pub(crate) use ink::Img;
pub(crate) use kit::Tool;
pub(crate) use live::{Walker, soldier_state};
pub(crate) use map::{MAP_H, MAP_W, Place, SCREEN_H, SCREEN_W, TILE};
pub(crate) use scene::{
    Hud, Joust, Knight, Scene, Soldier, SoldierKind, SoldierState, Ward, Weather,
};

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
    for y in 0..view.h {
        for x in 0..view.w {
            if let Some(ch) = ground::mark(realm, view.x + x, view.y + y) {
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
    for &(x, y, w, h) in &staged.beacons {
        kit::beacon(&mut cv, (x - view.x, y - view.y, w, h), scene.tick);
    }
    cv
}

/// A frame: the HUD over one view of the map, rim vignetted.
pub(crate) fn frame_at(scene: &Scene, view: View) -> Img {
    let mut f = Img::black(view.w, view.h + HUD_H);
    hud::draw(&mut f, scene);
    f.stamp(&render_view(scene, view), 0, HUD_H);
    light::vignette(&mut f, HUD_H);
    f
}

/// A frame on the screen that holds the knight.
pub(crate) fn frame(scene: &Scene) -> Img {
    frame_at(scene, View::screen_at(scene.knight.x, scene.knight.y))
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/overworld__tests.rs"]
mod tests;
