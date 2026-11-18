//! The dungeon delve drawn in the overworld's own language.
//!
//! Black paper, sixteen-pixel tiles, the kit's shaded rocks for walls and
//! sparse ground marks for floors, dusk with firelight pools — and the
//! realm's own knight in the middle of it. The Crypt is slate, the Mines are
//! the overworld's timber-brown rock, the Dragon Keep charcoal over lava.
//!
//! A room is painted once at native size (16 px per tile, 8 per arena unit,
//! 384x224) and then fitted to the frame: whole-pixel enlargement when it
//! fits, otherwise a reduction where live state and the brightest mark of
//! each footprint win, so figures and shots survive on black paper.
//!
//! Scenery uses the realm's material banks and dims at dusk. Figures, shots
//! and loot are drawn after the light at full ink; their live parts are
//! signal inks.
//!
//! Contract: pure and deterministic — the same run and size paint the same
//! pixels.

mod boss_art;
pub(crate) use boss_art::{breathe as library_breathe, breathing as library_breathing, dress as library_dress};
mod entrance_art;
#[cfg(feature = "boss-library-review")]
#[unsafe(no_mangle)]
pub extern "C" fn boss_library_review(sample: u32) {
    boss_art::library_review(sample as usize);
}
#[cfg(feature = "testing")]
pub fn boss_floor_review<T>(draw: impl FnOnce() -> T) -> T {
    boss_art::library_floor_review(draw)
}
#[cfg(feature = "testing")]
#[unsafe(no_mangle)]
pub extern "C" fn boss_effect_review(sample: u32) {
    boss_art::effect_review(sample as usize);
}
mod altar;
mod barony;
mod reclaim;
mod goods;
pub(super) mod college;
mod colosseum;
mod alehouse;
mod bazaar;
mod champions;
mod three_heroes;
mod spellfx;
mod wishstrips;
mod meleestrips;
mod muster_tent;
mod squire_rooms;
mod bomrekedan;
mod bomrekedan_works;
mod pack_heroes;
mod pack_foes;
mod demonic;
pub(super) mod ranch;
pub mod atari;
mod bones;
pub mod class_looks;
mod bounties;
mod capture;
mod camera;
pub(crate) mod down;
mod cards;
mod countryside;
mod draw_list;
mod dusk;
pub mod dungeon_map;
mod coastal;
mod woodland;
mod wetland;
mod aerie;
mod mummers;
mod hippodrome;
mod windcountry;
mod bowcountry;
mod town;
mod nooks;
mod garden;
mod freedom;
mod starfall;
mod rewards;
mod rite_wall;
mod deep_village;
mod open;
mod chronicle;
mod props;
mod eastern;
mod feel;
mod figures;
mod lane_squires;
mod floor_stats;
mod flourishes;
mod autorig;

pub(crate) fn wished_pose(rows: &[&str], tick: u64) -> Option<super::ink::Img> {
    autorig::stage_pose(rows, tick)
}

pub(crate) fn room_figure(run: &crate::rules::Run, id: u32, hero: &crate::rules::Hero) -> (super::ink::Img, Option<[i32; 2]>) {
    let (dressed, _) = figures::dressed_knight(run, id, hero);
    let mut image = dressed.image;
    let mut pivot = dressed.pivot;
    if hero.aim_x < -0.3 {
        let w = image.w;
        image = image.flip_h();
        if let Some(p) = pivot.as_mut() {
            p[0] = w - 1 - p[0];
        }
    }
    (image, pivot)
}
mod folly;
mod fx;
mod hall_voices;
mod gauntlets;
use crate::rules::Shot;
use crate::rules::wishcraft::Tint;
mod northern;
mod notices;
pub mod primer;
mod critters;
mod forest;
mod forest_road;
mod room_paint;
mod module_effects;
mod roundtable;

mod lanternward;

mod moonfall;

mod bannerpeak;

mod griffinwing;
mod ground_marks;

mod lightgallop;
mod knight_racers;
mod deathmatch;
pub(crate) mod juice;
pub fn with_deathmatch_prediction<T>(own: Option<(u32, crate::rules::deathmatch::Fighter)>, draw: impl FnOnce() -> T) -> T {
    deathmatch::with_prediction(own, draw)
}
mod fortune;
mod cauldron;
mod frogs;
mod gate;
mod hand;
mod hazards;
mod hireling;
mod squires;
mod squires_lodge;
mod squire_hall;
mod academy_squires;
mod minions;
mod squire_company;
pub(super) mod holdings;
mod holdings_folk;
mod home;
mod joust;
mod kingdom;
mod industry;
mod village_ui;
mod frontier_scene;
mod frontier_art;
mod village_art;
mod kingdom_gear;
mod claim_bosses;
mod ore_threats;
pub mod knights;
mod mirror;
pub mod quick_rig;
pub mod factory;
mod wear;
mod lessons;
mod magic;
pub(crate) mod zones;
mod ailments;
pub(crate) mod pfx;
mod ranged_events;
mod wishfx;
mod lifted;
mod marks;
mod guardian_bar;
mod signatures;
mod merchant;
pub(crate) mod party;

pub(crate) fn notice_board(lines: &[(String, char)]) -> Img {
    home::board(lines)
}

pub(crate) fn seat_colours(run: &Run, id: u32) -> &[(char, char)] {
    match run.players.get(&id).map(|h| h.colours()) {
        Some(own) if !own.is_empty() => own,
        _ => knights::seat_colours(id),
    }
}
mod pastimes;
mod teachers;
pub mod plates;
mod dungeon_art;
pub mod hd_pack;
pub mod figure_keys;
pub mod hud_art;
mod pitstop;
pub mod race3d;
mod cinder;
mod latch;
#[doc(hidden)]
pub mod slot_art;
mod brasswake;
mod rafterwake;
mod pennon;
mod presence;
mod mossbell;
mod votes;
mod team;
mod crossroads;
mod encounter;
mod rescues;
pub mod paintings;
mod room_scene;
mod shot_meshes;
pub mod dimensions;
mod pickup_cards;
mod pickup_meshes;
mod item_pack;
pub mod stage_frame;
mod runes;
mod scenery;
mod seat;
pub mod stand_back;
pub use kingdom::hillside_art::{slot_at as hill_slot_at, tile_at as hill_tile_at};

pub fn hill_strip_height() -> i32 {
    kingdom::hillside_art::STRIP_H
}
mod secrets;
mod side;
mod shoulders;
pub mod sprites;
mod stage;
mod swamp;
pub mod swamp_art;
pub mod siege_stage;
mod stature;

pub fn cell_signature_core(c: [u8; 3]) -> bool {
    boss_art::cell_signature_core(c)
}

pub fn cell_effect_ink(c: [u8; 3]) -> Option<[u8; 3]> {
    boss_art::cell_effect_ink(c)
}

pub fn cell_projectile_signal(c: [u8; 3]) -> bool {
    boss_art::cell_projectile_signal(c)
}

pub fn cell_projectile_core(c: [u8; 3]) -> bool {
    boss_art::cell_projectile_core(c)
}

pub fn has_boss_projectiles(run: &Run) -> bool {
    run.projectiles.iter().filter_map(crate::rules::bosses::Art::of_shot).any(boss_art::native_projectile)
}

mod foe_material;
mod strike;
mod blades;
mod melee;
mod tavern;
pub(super) mod three;
mod tide;
pub mod tray;
mod trophies;
mod ults;
mod vfx;
mod gore;
mod void;
mod volleys;
mod wanted;
mod wax_art;
mod wax_maze;
mod belfry_shoot;
mod book_hop;
mod powder_room;
#[cfg(test)]
pub(crate) fn powder_set_off(off: bool) {
    powder_room::OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(test)]
pub(crate) fn powder_profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    powder_room::profile(run, focus)
}
#[cfg(test)]
pub(crate) use powder_room::words::{Claim as PowderClaim, meets as powder_meets};
#[cfg(test)]
pub(crate) fn powder_words() -> Vec<PowderClaim> {
    powder_room::words::last()
}
#[cfg(test)]
pub(crate) fn hop_set_off(off: bool) {
    book_hop::OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(test)]
pub(crate) fn hop_profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    book_hop::profile(run, focus)
}
#[cfg(test)]
pub(crate) use book_hop::words::{Claim as HopClaim, meets as hop_meets};
#[cfg(test)]
pub(crate) fn hop_words() -> Vec<HopClaim> {
    book_hop::words::last()
}
#[cfg(test)]
pub(crate) fn hop_said() -> Vec<(&'static str, String, (i32, i32, i32, i32))> {
    book_hop::words::said()
}
#[cfg(test)]
pub(crate) fn hop_tucks_tray(run: &Run) -> bool {
    book_hop::tucks_tray(run)
}
#[cfg(test)]
pub(crate) fn hop_strip_top() -> i32 {
    book_hop::strip_top()
}
#[cfg(test)]
pub(crate) fn hop_chip_xs(run: &Run) -> Vec<(u32, i32)> {
    book_hop::chip_xs(run)
}
#[cfg(test)]
pub(crate) fn belfry_set_off(off: bool) {
    belfry_shoot::OFF.store(off, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(test)]
pub(crate) fn belfry_profile(run: &Run, focus: Option<u32>) -> [(&'static str, f64); 3] {
    belfry_shoot::profile(run, focus)
}
mod works;
mod world;
mod yard;

pub use cards::card_art;
pub use dungeon_map::MapView;
pub use party::{
    BAND_H, HUD_H, band_rows, hud_band, hud_rows, lift as seat_lift, lift_frame as seat_lift_frame,
    hand_plates as seat_hand_plates, lift_frame_with as seat_lift_frame_with, lift_with as seat_lift_with, seat_size,
    view_rows,
};
pub use paintings::{Painting, Pose};
pub use room_scene::{
    Body, ClassShot, Emitter, Figure, FineGround, ForgedColour, ForgedShape, Hue, Motes, RoomScene, SceneLight, SceneShot, ShotForm,
    look_key, scene_for, scene_for_live,
};
#[cfg(any(test, feature = "testing"))]
pub use hippodrome::race_camera;

pub fn crawl_figure(who: &str) -> Option<Img> {
    let rows: &[&str] = match who {
        "merlin" => &home::MERLIN,
        "tobbin" => &home::TOBBIN,
        "blaise" => &home::BLAISE,
        "wren" => &home::WREN,
        "kay" => &trophies::KAY,
        "ector" => &lessons::ECTOR,
        "dinadan" => &tavern::DINADAN,
        "beaumains" => &hireling::BEAUMAINS,
        "maud" | "mabel" | "anselm" | "pip" => return Some(rescues::sprite_of(who)),
        _ => return None,
    };
    Some(Img::from_rows(rows))
}

pub fn crawl_enemy(kind: crate::rules::EnemyKind, tick: u32) -> Img {
    sprites::enemy(kind, tick)
}

#[cfg(any(test, feature = "testing"))]
pub fn frog_sprites(tick: u32) -> Vec<Img> {
    use crate::rules::frogs::Gift;
    let mut out = vec![
        frogs::frog(tick, false, false),
        frogs::frog(tick, true, false),
        frogs::perched(false),
    ];
    for age in [4, 10, 16] {
        out.push(frogs::transform(Gift::Princess, age, tick).1.unwrap());
    }
    for gift in [Gift::Princess, Gift::Prince, Gift::Clown] {
        out.push(frogs::transform(gift, 60, tick).0);
    }
    out
}

#[cfg(any(test, feature = "testing"))]
pub fn critter_figure(
    kind: crate::rules::EnemyKind,
    stage: u8,
    timer: u32,
    tick: u32,
) -> Img {
    critters::figure(kind, stage, timer, tick)
}

pub fn crawl_prop(what: &str, tick: u32) -> Img {
    match what {
        "tomb" => sprites::tomb(1),
        "rock" => sprites::rock(),
        "mushroom" => sprites::mushroom(tick / 40),
        "eye_pillar" => sprites::eye_pillar(1, tick / 3),
        _ => sprites::brazier(true, tick, 7),
    }
}
pub fn crawl_sign(label: &str) -> Img {
    super::kit::district_sign(label)
}

pub use knights::of_kit as knight_of;

use draw_list::{DrawList, Layer};
pub use camera::{camera, camera_exact, view_origin};
pub(crate) use camera::with_watch;
pub(super) use camera::{in_view_x, in_view_y, pointer};
use camera::crop;
pub use capture::{pickup_cards, props_meshes};
pub(super) use capture::{captured, glow, lift};
use capture::{capturing, pickup_cards_on, props_meshes_on, with_capture};
use dusk::{dusk, dusk_with, fire, light_rows, lights_out};
#[cfg(any(test, feature = "testing"))]
pub use figures::boss_display_feet;
#[cfg(test)]
use figures::{figures, foe_figure};
use figures::{figures_with, foe_figure_with};
use flourishes::{flourishes, hooks, orbits, sparks, summoned, tallow};
use folly::{bats_layer, gantry_layer, hop_floor, hop_layer, powder_layer};
use ground_marks::{flags, floor_mark};
use hazards::{foes_hazards, hazard_mark, hazards, vent_at};
pub use lifted::{
    Lifted, LiftedKind, draw_lifted_outside, lifted, lifted_outside, lifted_sprite, seat_lifted,
};
use marks::foes_marks;
pub(super) use notices::heat_board;
use notices::{
    achievement_banner, again_hint, beyond, boss_title, dare_board, field_banner, top_right_board_bottom,
    verdict, way_on_marker,
};
pub use presence::presence_marks;
use presence::{CARRIED_SKULL_GAP, fellowship_arrow, hold_line};
pub(super) use scenery::during_live;
pub use scenery::{frame_for_live, prepare_entry, wire_scenery};
use scenery::{
    baked_lights, fine_scenery_cached, fixtures_drawn_here, home_print, laid_plate, live_frame,
    room_details, scenery_cached, world3d, world_stood,
};
#[cfg(test)]
use scenery::{drawn_elsewhere, near_wall, scenery, scenery_cached_exact};
pub(crate) use seat::hud_in_band;
pub use seat::{draw_parts_outside, in_seat, parts_outside};
use volleys::{shots, tinted};
use std::sync::OnceLock;

use super::chrome;
use super::ink::{BLACK, Img, PALETTE, Rgb, bayer, hash, is_signal, rgb, vnoise};
use super::kit::{RockKind, Tiles, chest, district_sign, torch};
use super::light::{DUSK, Light, STEPS, light_at, seam, step_down, warm};
use crate::rules::{imps, looks};
use crate::rules::{
    COLS, EnemyKind, HEIGHT, Pack, Phase, ROWS, Room, RoomKind, Run, TELEGRAPH,
    TILE_UNITS, Tile, Trap, VIGIL_REACH, WIDTH, ledge::FOOT,
};

const TILE: i32 = 16;
pub const UNIT: f32 = TILE as f32 / TILE_UNITS;
pub const NATIVE_W: i32 = COLS as i32 * TILE;
pub const NATIVE_H: i32 = ROWS as i32 * TILE;

fn at(ux: f32, uy: f32) -> (i32, i32) {
    ((ux * UNIT).floor() as i32, (uy * UNIT).floor() as i32)
}

#[derive(Clone, Copy, Debug)]
struct Field {
    ox: i32,
    oy: i32,
    num: i32,
    den: i32,
}

impl Field {
    fn fit(w: i32, h: i32) -> Field {
        let (num, den) = if w >= NATIVE_W && h >= NATIVE_H {
            ((w / NATIVE_W).min(h / NATIVE_H), 1)
        } else if w * NATIVE_H <= h * NATIVE_W {
            (w, NATIVE_W)
        } else {
            (h, NATIVE_H)
        };
        let (tw, th) = (NATIVE_W * num / den, NATIVE_H * num / den);
        Field {
            ox: (w - tw) / 2,
            oy: (h - th) / 2,
            num,
            den,
        }
    }

    fn map(&self, nx: i32, ny: i32) -> (i32, i32) {
        (
            self.ox + nx * self.num / self.den,
            self.oy + ny * self.num / self.den,
        )
    }

    fn px(&self, ux: f32, uy: f32) -> (i32, i32) {
        let (nx, ny) = at(ux, uy);
        self.map(nx, ny)
    }

    fn blit(&self, scenery: &Img, figures: &Img, out: &mut Img) {
        self.blit_ranked(scenery, figures, out, None, None);
    }

    fn blit_ranked(
        &self,
        scenery: &Img,
        figures: &Img,
        out: &mut Img,
        ranked: Option<(&[u32], usize, (i32, i32))>,
        alerts: Option<&std::collections::BTreeSet<Rgb>>,
    ) {
        if self.den == 1 {
            let k = self.num;
            if k == 1
                && self.ox == 0
                && self.oy == 0
                && out.w == scenery.w
                && out.h == scenery.h
                && figures.w == scenery.w
                && figures.h == scenery.h
            {
                Img::compose_1x(figures, scenery, out);
                return;
            }
            if Img::compose_kx(figures, scenery, out, self.ox, self.oy, k) {
                return;
            }
            for y in 0..scenery.h * k {
                for x in 0..scenery.w * k {
                    if let Some(c) = figures.get(x / k, y / k).or(scenery.get(x / k, y / k)) {
                        out.set(self.ox + x, self.oy + y, c);
                    }
                }
            }
            return;
        }
        let rank = |c: Rgb| {
            let l = 299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]);
            if alerts.is_some_and(|inks| inks.contains(&c)) { l + 2_000_000 }
            else if is_signal(c) { l + 1_000_000 } else { l }
        };
        let (tw, th) = (
            NATIVE_W * self.num / self.den + 1,
            NATIVE_H * self.num / self.den + 1,
        );
        let cells = (tw * th) as usize;
        let (mut total, mut inked) = (vec![0u16; cells], vec![0u16; cells]);
        let mut best_scene: Vec<Option<Rgb>> = vec![None; cells];
        let mut best_rank = if ranked.is_some() { vec![0u32; cells] } else { Vec::new() };
        let mut best_figure: Vec<Option<Rgb>> = vec![None; cells];
        for y in 0..scenery.h {
            for x in 0..scenery.w {
                let (ox, oy) = self.map(x, y);
                let i = ((oy - self.oy) * tw + (ox - self.ox)) as usize;
                total[i] += 1;
                if let Some(c) = figures.get(x, y)
                    && best_figure[i].is_none_or(|e| rank(c) > rank(e))
                {
                    best_figure[i] = Some(c);
                }
                if let Some(c) = scenery.get(x, y).filter(|&c| c != BLACK) {
                    inked[i] += 1;
                    if let Some((ranks, stride, (ox, oy))) = ranked {
                        let value = ranks[(y + oy) as usize * stride + (x + ox) as usize];
                        if best_scene[i].is_none() || value > best_rank[i] {
                            best_scene[i] = Some(c);
                            best_rank[i] = value;
                        }
                    } else if best_scene[i].is_none_or(|e| rank(c) > rank(e)) {
                        best_scene[i] = Some(c);
                    }
                }
            }
        }
        for i in 0..cells {
            let c = best_figure[i].or(best_scene[i].filter(|_| inked[i] * 3 >= total[i]));
            if let Some(c) = c {
                out.set(self.ox + i as i32 % tw, self.oy + i as i32 / tw, c);
            }
        }
    }
}

struct Style {
    rock: RockKind,
    pool: (char, char),
}

fn style(pack: Pack) -> Style {
    match pack {
        Pack::Crypt => Style {
            rock: RockKind::Slate,
            pool: ('x', 'I'),
        },
        Pack::Cavern => Style {
            rock: RockKind::Ore,
            pool: ('b', 'I'),
        },
        Pack::Hellforge => Style {
            rock: RockKind::Char,
            pool: ('X', 'p'),
        },
        Pack::Archive => Style {
            rock: RockKind::Slate,
            pool: ('q', 'I'),
        },
        Pack::Fungal => Style {
            rock: RockKind::Ore,
            pool: ('e', 'N'),
        },
        Pack::Unknown => Style {
            rock: RockKind::Char,
            pool: ('0', 'q'),
        },
        Pack::Swamp => swamp_art::STYLE,
    }
}

fn doors(room: &Room) -> [(i32, i32); 4] {
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    [
        (cols / 2 - 1, 0),
        (cols - 1, rows / 2 - 1),
        (cols / 2 - 1, rows - 1),
        (0, rows / 2 - 1),
    ]
}

fn corner(room: &Room, col: i32, row: i32) -> bool {
    (col == 0 || col == room.cols as i32 - 1) && (row == 0 || row == room.rows as i32 - 1)
}

fn room_px(room: &Room) -> (i32, i32) {
    (room.cols as i32 * TILE, room.rows as i32 * TILE)
}

fn stand(cv: &mut Img, im: &Img, x: i32, y: i32) {
    note_body(im, x - im.w / 2, y - im.h + 1);
    paintings::stood((im.w, im.h), (x - im.w / 2, y - im.h + 1));
    cv.stamp(im, x - im.w / 2, y - im.h + 1);
}

fn note_body(im: &Img, x: i32, y: i32) {
    with_capture(|capture| {
        if let Some(bodies) = &mut capture.member {
            let key = room_scene::look_key(im);
            let named = match capture.pending_name.take() {
                Some((look, named)) if look == key => Some(named),
                other => {
                    capture.pending_name = other;
                    None
                }
            };
            bodies.push(room_scene::Body { key, x, y, img: im.clone(), named });
        }
    });
}

fn stand_at(cv: &mut Img, im: &Img, ux: f32, uy: f32, dy: i32) {
    let px = ux * UNIT;
    let py = uy * UNIT;
    let (x, y) = (px.floor() as i32, py.floor() as i32);
    if live_frame() && !capturing() {
        cv.stamp_sub(
            im,
            (x - im.w / 2) as f32 + (px - px.floor()),
            (y - im.h + 1 + dy) as f32 + (py - py.floor()),
        );
    } else {
        stand(cv, im, x, y + dy);
    }
}

fn pixel_fraction(ux: f32, uy: f32) -> (f32, f32) {
    let (px, py) = (ux * UNIT, uy * UNIT);
    (px - px.floor(), py - py.floor())
}

pub(super) fn near_prompt(cv: &mut Img, run: &Run, (x, y): (i32, i32), label: &str) {
    let reach = 2.5 * TILE as f32;
    let near = run.players.values().any(|h| {
        let (hx, hy) = at(h.x, h.y);
        h.hp > 0 && ((hx - x) as f32).hypot((hy - y) as f32) < reach
    });
    if !near {
        return;
    }
    let label = format!("E: {label}");
    let w = super::ink::text_width(&label);
    let (lx, ly) = ((x - w / 2).clamp(3, cv.w - w - 3), (y - 30).max(2));
    chrome::label(cv, (lx, ly), &label, 'O', chrome::Tone::KEY);
}

pub(super) fn shut_planks(cv: &mut Img, x: i32, y: i32, w: i32) {
    cv.line(x + 1, y + 1, x + w - 2, y + TILE - 3, 'B');
    cv.line(x + 1, y + TILE - 3, x + w - 2, y + 1, 'B');
    cv.line(x + 2, y + 1, x + w - 1, y + TILE - 3, 'r');
    cv.line(x + 2, y + TILE - 3, x + w - 1, y + 1, 'r');
}

pub(super) fn open_arrow(cv: &mut Img, x: i32, y: i32) {
    for k in 0..4 {
        cv.line(x - k, y + k, x + k, y + k, '9');
    }
    cv.line(x, y + 4, x, y + 8, '9');
}

fn clay_pots(cv: &mut Img, run: &Run) {
    if atari::here(run) {
        let (x, y, w, h) = cv.span();
        let mut layer = Img::window(cv.w, cv.h, (x, y, w, h));
        clay_pots_drawn(&mut layer, run);
        cv.stamp(&atari::sprite_in(&layer, atari::GREY, (0, 0)), 0, 0);
        return;
    }
    clay_pots_drawn(cv, run);
}

fn clay_pots_drawn(cv: &mut Img, run: &Run) {
    let pot = Img::from_rows(&[
        "..roooP..",
        "...PrP...",
        ".oOORRRP.",
        "oOORRRRrP",
        "tOORRRrrP",
        "oORRRrrrP",
        ".oRRrrPP.",
        "..PrPPr..",
        "...PPP...",
        ".qqqqqqq.",
    ]);
    for (_, (x, y)) in run.pots_of(run.at) {
        let (px, py) = at(x, y);
        cv.stamp(&pot, px - 4, py - 4);
    }
    for &(x, y, left) in &run.pot_shards {
        let (px, py) = at(x, y);
        let spread = 1 + (crate::rules::pots::SHARDS - left) as i32 / 3;
        for (dx, dy, ink) in [(-2, 1, 'P'), (2, 0, 'o'), (0, 3, 'P'), (-1, -2, 'B'), (3, 3, 'o'), (-3, 3, 'B')] {
            cv.rect(px + dx * spread, py + dy + spread / 2, 2, 2, ink);
        }
    }
}

const READING_REACH: f32 = 3.5;

pub(super) fn shoppers(
    run: &Run,
    focus: Option<u32>,
) -> impl Iterator<Item = (u32, &crate::rules::Hero)> {
    run.players
        .iter()
        .filter(move |(id, _)| focus.is_none_or(|f| **id == f))
        .map(|(&id, h)| (id, h))
}

pub(super) fn knight_near(run: &Run, spot: (f32, f32), reach: f32) -> bool {
    run.players.values().any(|h| {
        h.hp > 0
            && (h.x - spot.0 * TILE_UNITS).hypot(h.y - spot.1 * TILE_UNITS) < reach * TILE_UNITS
    })
}

fn reading(run: &Run) -> bool {
    use crate::rules::{countryside, pastimes::Game, world};
    if run.pastimes.round.is_some() || coastal::board_visible(run) || woodland::board_visible(run)
        || wetland::board_visible(run)
        || aerie::board_visible(run)
        || windcountry::heist_board(run)
        || bowcountry::board_visible(run)
        || town::board_visible(run)
    {
        return true;
    }
    let kind = run.room().kind;
    if crate::rules::colosseum::is_arena(kind) {
        return true;
    }
    Game::ALL
        .iter()
        .filter(|g| g.room() == kind)
        .map(|g| g.start())
        .chain(countryside::PLOTS.iter().filter(|p| p.room == kind).map(|p| p.at))
        .chain(
            world::ENTRANCES
                .iter()
                .filter(|e| e.room == kind)
                .map(world::entrance_centre),
        )
        .any(|spot| knight_near(run, spot, READING_REACH))
}

fn layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    let (scenery, mut over) = scene_layers(run, focus, legible);
    votes::panel(&mut over, run, focus);
    if !run.roundtable_showing() {
        encounter::panel(&mut over, run, focus);
    }
    (scenery, over)
}

fn scene_layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    if industry::showing(run, focus) { return industry::layers(run, focus); }
    if industry::frontier(run) { return industry::landscape(run, focus); }
    if run.mossbell_showing() { return mossbell::layers(run, focus); }
    if run.brasswake_showing() { return brasswake::layers(run, focus); }
    if run.forest_road_showing() { return forest_road::layers(run, focus); }
    if let Some(scene) = roundtable::layers(run, focus) { return scene; }
    if run.pennon_showing() { return pennon::layers(run, focus); }
    if joust::takes_the_screen(run) { return joust::layers(run, focus); }
    if run.rafterwake_showing() { return rafterwake::layers(run, focus); }
    if run.latch_showing() { return latch::layers(run, focus); }
    if run.cinder_showing() { return cinder::layers(run, focus); }
    if run.room().kind == RoomKind::Kingdom {
        return kingdom::layers_with(run, focus, legible);
    }
    debug_assert_eq!(
        (NATIVE_W, NATIVE_H),
        at(WIDTH, HEIGHT),
        "a tile is two arena units"
    );
    if stage::takeover(run) {
        return stage::layers(run, focus, legible);
    }
    let over = room_layer(run, focus, legible);
    let origin = view_origin(run, focus);
    let mut over = crop(&over, origin);
    let mut scenery = crop(&scenery_cached(run), origin);
    if run.circle() > 0 && !run.at_home_now() {
        gate::tint(&mut scenery, run.circle());
    }
    if run.mode == crate::rules::fortune::Mode::LightsOut && !run.at_home_now() && !run.in_deep_village() {
        lights_out(&mut scenery, &mut over, run, origin);
    }
    if !atari::here(run) {
        boss_art::library_ground(&mut scenery, run, origin);
    }
    if swamp::fogged(run) {
        swamp::fog(&mut scenery, &mut over, run, origin);
    }
    if legible && !hud_in_band() && let Some(rect) = tray::bar_rect(run, focus, NATIVE_W, NATIVE_H) {
        room_scene::darken(&mut scenery, rect);
    }
    screen_marks(&mut over, run, focus, legible, origin);
    (scenery, over)
}

fn room_layer(run: &Run, focus: Option<u32>, legible: bool) -> Img {
    let (pw, ph) = room_px(run.room());
    let mut over = Img::new(pw, ph);
    let list = room_list(run, focus, legible);
    if atari::here(run) {
        let (ground, rest) = list.split(Layer::Ground);
        ground.paint(&mut over);
        atari::board(&mut over);
        rest.paint(&mut over);
    } else {
        list.paint(&mut over);
    }
    over
}

fn room_list(run: &Run, focus: Option<u32>, legible: bool) -> DrawList<'_> {
    let mut list = room_paint::room_list(run, focus, legible);
    list.add(Layer::Ground, move |cv| mirror::wall(cv, run));
    bomrekedan_works::add(&mut list, run);
    list
}

fn screen_marks(cv: &mut Img, run: &Run, focus: Option<u32>, legible: bool, origin: (i32, i32)) {
    let banner = legible.then(|| run.banner_now()).flatten();
    let mut screen = DrawList::default();
    let hud_here = legible && !hud_in_band();
    screen.add(Layer::Screen, |cv| {
        if hud_here {
            tray::corner(cv, run, focus);
        }
    });
    screen.add(Layer::Screen, |cv| {
        if hud_here {
            dungeon_map::minimap(cv, run, focus);
        }
    });
    screen.add(Layer::Screen, |cv| beyond(cv, run, origin));
    screen.add(Layer::Screen, |cv| way_on_marker(cv, run, origin, run.tick as u32));
    screen.add(Layer::Screen, |cv| {
        if let Some(((x, y), name)) = run.home_way() {
            let bob = [0.0, 1.0, 2.0, 1.0][(run.tick / 6 % 4) as usize];
            let (px, py) = at(x, y);
            let (sx, sy) = ((px - origin.0) as f32, (py - origin.1) as f32 - 44.0 - bob);
            pointer(cv, (sx, sy), (0.0, 1.0), name, false);
        }
    });
    screen.add(Layer::Screen, |cv| fellowship_arrow(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| kingdom::missions::spots(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| heat_board(cv, run));
    screen.add(Layer::Screen, |cv| team::panel(cv, run, focus));
    screen.add(Layer::Screen, |cv| if reading(run) { pastimes::boards(cv, run); });
    screen.add(Layer::Screen, |cv| wax_maze::screen(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| bats_layer(cv, run));
    screen.add(Layer::Screen, |cv| belfry_shoot::screen(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| gantry_layer(cv, run, origin));
    screen.add(Layer::Screen, |cv| powder_layer(cv, run, origin));
    screen.add(Layer::Screen, |cv| powder_room::screen(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| hop_layer(cv, run));
    screen.add(Layer::Screen, |cv| book_hop::screen(cv, run, focus, origin));
    screen.add(Layer::Screen, |cv| field_banner(cv, run, origin, focus));
    screen.add(Layer::Screen, |cv| verdict(cv, run));
    screen.add(Layer::Screen, |cv| if legible { rewards::screen(cv, run, origin); });
    screen.add(Layer::Screen, |cv| if legible { boss_title(cv, run); });
    screen.add(Layer::Screen, |cv| if legible { rite_wall::screen(cv, run, focus); });
    screen.add(Layer::Screen, |cv| gate::card(cv, run));
    screen.add(Layer::Screen, |cv| wanted::poster(cv, run, legible));
    screen.add(Layer::Screen, |cv| {
        if let Some(feat) = banner {
            achievement_banner(cv, run, feat);
        } else if legible {
            dare_board(cv, run);
        }
    });
    screen.add(Layer::Screen, |cv| {
        pitstop::panel(cv, run, focus, legible);
        mirror::panel(cv, run, focus);
    });
    screen.add(Layer::Screen, |cv| college::prompt(cv, run, focus, legible));
    screen.add(Layer::Screen, |cv| if legible { college::dialogue(cv, run); });
    screen.add(Layer::Screen, |cv| if legible { deep_village::screen(cv, run, focus); });
    screen.add(Layer::Screen, |cv| if legible { reclaim::screen(cv, run, focus); });

    screen.add(Layer::Screen, |cv| {
        if legible && matches!(run.room().kind, RoomKind::Alehouse | RoomKind::Bazaar | RoomKind::MusterTent) {
            alehouse::screen_boards(cv, run, focus);
        } else if legible && focus.is_some() && run.room().kind == RoomKind::Gate {
            hall_voices::title(cv, run, &mut Vec::new());
        }
    });
    screen.paint(cv);
}

const CARD_SHOT_SIDE: i32 = 8;
const CARD_FINE_SIDE: i32 = 2 * CARD_SHOT_SIDE;

fn enlarge(im: &Img, k: f32) -> Img {
    let (w, h) = (
        (im.w as f32 * k).round() as i32,
        (im.h as f32 * k).round() as i32,
    );
    let mut out = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = im.get((x as f32 / k) as i32, (y as f32 / k) as i32) {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub(super) fn resized(im: &Img, k: f32) -> Img {
    if k >= 1.0 {
        return enlarge(im, k);
    }
    let (w, h) = (((im.w as f32 * k).ceil() as i32).max(1), ((im.h as f32 * k).ceil() as i32).max(1));
    let mut out = Img::new(w, h);
    for y in 0..h {
        let y0 = ((y as f32 / k).floor() as i32).min(im.h - 1);
        let y1 = (((y + 1) as f32 / k).ceil() as i32).clamp(y0 + 1, im.h);
        for x in 0..w {
            let x0 = ((x as f32 / k).floor() as i32).min(im.w - 1);
            let x1 = (((x + 1) as f32 / k).ceil() as i32).clamp(x0 + 1, im.w);
            let (mut best, mut n) = (None::<(u32, [u8; 3])>, 0u32);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    if let Some(c) = im.get(sx, sy) {
                        n += 1;
                        let luma = 299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]);
                        if best.is_none_or(|(l, _)| luma > l) {
                            best = Some((luma, c));
                        }
                    }
                }
            }
            if let Some((_, c)) = best.filter(|_| n * 4 >= ((x1 - x0) * (y1 - y0)) as u32) {
                out.set(x, y, c);
            }
        }
    }
    out
}

#[cfg_attr(not(test), allow(dead_code))]
fn native(run: &Run) -> Img {
    let (mut cv, over) = layers(run, None, true);
    cv.stamp(&over, 0, 0);
    cv
}

pub fn frame(run: &Run, w: i32, h: i32) -> Img {
    frame_for(run, w, h, None)
}

pub fn frame_px(run: &Run, w: i32, h: i32, focus: Option<u32>, ux: f32, uy: f32) -> (i32, i32) {
    let (cx, cy) = camera(run, focus);
    let (nx, ny) = at(ux, uy);
    Field::fit(w, h).map(nx - cx, ny - cy)
}

pub fn seat_frame(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    seat_frame_with(run, w, h, focus, &MapView::default())
}

pub fn seat_frame_with(run: &Run, w: i32, h: i32, focus: Option<u32>, map: &MapView) -> Img {
    stand_back::with(run, focus, map.full, || in_seat(|| party::lift_frame_with(&frame_for(run, w, view_rows(h), focus), run, focus, map)))
}

pub fn seat_scene(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    seat_scene_with(run, w, h, focus, &MapView::default())
}

pub fn seat_scene_with(run: &Run, w: i32, h: i32, focus: Option<u32>, map: &MapView) -> RoomScene {
    stand_back::with(run, focus, map.full, || in_seat(|| party::lift_with(scene_for(run, w, view_rows(h), focus), run, focus, map)))
}

pub fn seat_px(run: &Run, w: i32, h: i32, focus: Option<u32>, ux: f32, uy: f32) -> (i32, i32) {
    let band = band_rows(h);
    let (x, y) = frame_px(run, w, view_rows(h), focus, ux, uy);
    (x, y + band)
}

pub fn racer_garage(look: usize, seat: u32, turn: f32, ticks: u32) -> Img { knight_racers::garage(look, seat, turn, ticks) }

pub fn warm_first_shown() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let run = crate::rules::Run::new(1, 1, None);
        let _wide = frame_for(&run, 384, 216, Some(1));
        let _native = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    });
}

fn cave_shake(run: &Run, im: Img) -> Img {
    if run.shake == 0 {
        return im;
    }
    let (sx, sy) = camera::smooth_shake(run.tick, run.shake, 12, 6.0);
    if sx == 0 && sy == 0 {
        return im;
    }
    let mut shifted = Img::new(im.w, im.h);
    shifted.stamp(&im, sx, sy);
    shifted
}

pub fn frame_for(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    super::cave_seat::warm_mouth(run, focus);
    if let Some(id) = focus.filter(|id| run.caves.inside(*id)) {
        return cave_shake(run, super::cave_seat::cave_seat_frame(run, id, w, h));
    }
    if industry::showing(run, focus) || industry::frontier(run) { return industry::frame(run,w,h,focus); }
    if crossroads::showing(run) { return crossroads::frame(run, w, h, focus); }
    if crossroads::pastime(run) || encounter::pastime(run) {
        let mut arena = run.clone();
        arena.dungeon.depth = 0;
        arena.dungeon.open = None;
        return frame_v1(&arena, w, h, focus);
    }
    if let Some((results, age)) = run.results_showing(focus) {
        return floor_stats::frame(run, results, age, w, h);
    }
    if run.forest_road_showing() { return forest_road::frame(run, w, h, focus); }
    if let Some(img) = roundtable::frame(run, w, h, focus) { return img; }
    frame_v1(run, w, h, focus)
}

pub fn terminal_layers(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    if industry::showing(run, focus) || industry::frontier(run) { let im=industry::frame(run,NATIVE_W,NATIVE_H,focus); return Some((im,Img::new(NATIVE_W,NATIVE_H))); }
    if crossroads::showing(run) || crossroads::pastime(run) || encounter::pastime(run) {
        let frame = frame_for(run, NATIVE_W, NATIVE_H, focus);
        return Some((frame, Img::new(NATIVE_W, NATIVE_H)));
    }
    if run.forest_road_showing() { return Some(forest_road::layers(run, focus)); }
    if let Some(scene) = roundtable::terminal_layers(run, focus) { return Some(scene); }
    if run.race3d_showing() {
        let (base, _) = race3d::layers(run, focus, false);
        let empty = Img::new(base.w, base.h);
        return Some((base, empty));
    }
    if run.mossbell_showing() { return Some(mossbell::terminal_layers(run, focus)); }
    if run.brasswake_showing() { return Some(brasswake::terminal_layers(run, focus)); }
    if run.pennon_showing() { return Some(pennon::terminal_layers(run, focus)); }
    if run.rafterwake_showing() { return Some(rafterwake::terminal_layers(run, focus)); }
    if run.latch_showing() { return Some(latch::terminal_layers(run, focus)); }
    if run.cinder_showing() { return Some(cinder::terminal_layers(run, focus)); }
    None
}
pub fn terminal_layers_live(run: &Run, focus: Option<u32>) -> Option<(Img, Img)> {
    during_live(|| terminal_layers(run, focus))
}

#[cfg(any(test, feature = "testing"))]
#[path = "../../../tests/render/arena__fixtures.rs"]
pub mod testing;

pub fn frame_v1(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    if let Some(plate) = run.floor_plate(focus) {
        return plates::frame(plate.title().unwrap_or(plate), w, h);
    }
    if w < 12 || h < 8 {
        return Img::black(w.max(0), h.max(0));
    }
    let legible = w >= NATIVE_W && h >= NATIVE_H;
    let (mut scenery, mut over) = boss_art::library_cells(!legible, || layers(run, focus, legible));
    let field = Field::fit(w, h);
    let snap = dungeon_art::supported(run) && laid_plate(run).is_none();
    if snap {
        dungeon_art::palette_layer(&mut scenery);
        dungeon_art::palette_layer(&mut over);
    }
    let mut out = if snap {
        dungeon_art::paper_frame(w.max(0), h.max(0))
    } else {
        Img::black(w.max(0), h.max(0))
    };
    if run.room().kind == RoomKind::Kingdom && !industry::showing(run, focus) && !industry::frontier(run) && field.den != 1 {
        let ranks = super::kingdom::atlas_ranks(&run.kingdom.wishes);
        field.blit_ranked(
            &scenery,
            &over,
            &mut out,
            Some((
                &ranks,
                super::MAP_W as usize * TILE as usize,
                camera(run, focus),
            )),
            None,
        );
    } else {
        if !legible && let Some(alerts) = boss_art::library_warning_inks(run) {
            field.blit_ranked(&scenery, &over, &mut out, None, Some(alerts));
        } else {
            field.blit(&scenery, &over, &mut out);
        }
    }
    if snap {
        return out;
    }
    if dungeon_art::supported(run) && laid_plate(run).is_none() {dungeon_art::palette_frame(out)} else {out}
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__arena_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/render/overworld__fx_sheets.rs"]
mod fx_sheets;

#[cfg(test)]
#[path = "../../../tests/render/arena__motion_sheet.rs"]
mod motion_sheet;

#[cfg(test)]
#[path = "../../../tests/render/arena__alehouse__shots.rs"]
mod alehouse_shots;
#[cfg(test)]
#[path = "../../../tests/render/arena__hall_voices__shots.rs"]
mod hall_voices_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__roads__shots.rs"]
mod roads_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__upper_world__shots.rs"]
mod upper_world_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__tabula_probe__tests.rs"]
mod tabula_probe;

#[cfg(test)]
#[path = "../../../tests/render/arena__forges__shots.rs"]
mod forges_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__holdings_folk__shots.rs"]
mod holdings_folk_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__rite__tests.rs"]
mod rite_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__champions__shots.rs"]
mod champions_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__phantom_lancer__shots.rs"]
mod phantom_lancer_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__mirage_duelist__shots.rs"]
mod mirage_duelist_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__reclaim__shots.rs"]
mod reclaim_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__melee__shots.rs"]
mod melee_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__melee_pass__shots.rs"]
mod melee_pass_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__lab__shots.rs"]
mod lab_shots;

#[cfg(test)]
#[path = "../../../tests/render/overworld__foe_art_tests.rs"]
mod foe_art_tests;
#[cfg(test)]
#[path = "../../../tests/render/arena__content__shots.rs"]
mod content_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__dyes__tests.rs"]
mod dyes_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__dyes__shots.rs"]
mod dyes_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__boss_polish__shots.rs"]
mod boss_polish_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__wayfarers__shots.rs"]
mod wayfarers_shots;

pub struct LazyFrame {
    run: Run,
    w: u32,
    h: u32,
    focus: Option<u32>,
    rgba: std::sync::OnceLock<Vec<u8>>,
}
impl LazyFrame {
    pub fn new(run: Run, w: u32, h: u32, focus: Option<u32>) -> Self {
        Self {
            run,
            w,
            h,
            focus,
            rgba: std::sync::OnceLock::new(),
        }
    }
}
impl AsRef<[u8]> for LazyFrame {
    fn as_ref(&self) -> &[u8] {
        self.rgba.get_or_init(|| {
            frame_for(&self.run, self.w as i32, self.h as i32, self.focus).rgba_bytes()
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub fn overlay_for_test(run: &Run, focus: Option<u32>) -> Img {
    layers(run, focus, true).1
}

#[cfg(any(test, feature = "testing"))]
pub fn dread_art_for_test(culture: crate::rules::deep::Culture) -> Vec<String> {
    deep_village::dread_rows(culture)
}

#[cfg(any(test, feature = "testing"))]
pub fn whole_room_for_test(run: &Run) -> Img {
    let mut cv = (*scenery_cached(run)).clone();
    cv.stamp(&room_layer(run, None, true), 0, 0);
    cv
}

#[cfg(any(test, feature = "testing"))]
pub fn privy_panel_for_test(cv: &mut Img, run: &Run, focus: Option<u32>) {
    pitstop::panel(cv, run, focus, true);
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__scenery_sheet.rs"]
mod scenery_sheet;

pub fn mossbell_labels(run:&Run,focus:Option<u32>)->Vec<(i32,i32,String)>{mossbell::labels(run,focus)}

pub fn brasswake_labels(run:&Run,focus:Option<u32>)->Vec<(i32,i32,String)>{brasswake::labels(run,focus)}

pub fn rafterwake_labels(run:&Run,focus:Option<u32>)->Vec<(i32,i32,String)>{rafterwake::labels(run,focus)}
pub fn rafterwake_contour_colour(run:&Run,focus:Option<u32>)->Option<[u8;3]>{rafterwake::contour_colour(run,focus)}

pub fn pennon_labels(run:&Run,focus:Option<u32>)->Vec<(i32,i32,String)>{pennon::labels(run,focus)}
pub fn pennon_contour_colour(run:&Run,focus:Option<u32>)->Option<[u8;3]>{pennon::contour_colour(run,focus)}

#[cfg(any(test, feature = "testing"))]
pub fn forest_road_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    forest_road::layers(run, focus)
}

#[cfg(test)]
#[path = "../../../tests/render/arena__wish_fit__tests.rs"]
mod wish_fit_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__worn_native.rs"]
mod worn_native;

#[cfg(test)]
#[path = "../../../tests/render/arena__polish_feel.rs"]
mod polish_feel_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__hud_pass.rs"]
mod hud_pass_shots;

#[cfg(test)]
#[path = "../../../tests/render/arena__quick_rig__tests.rs"]
mod quick_rig_tests;
#[path = "../../../tests/render/arena__quick_rig__auto__tests.rs"]
mod quick_rig_auto_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__autorig__tests.rs"]
mod autorig_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__factory__tests.rs"]
mod factory_tests;

#[cfg(test)]
#[path = "../../../tests/render/arena__tavern_quality.rs"]
mod tavern_quality_review;
#[cfg(test)]
#[path = "../../../tests/render/arena__squires.rs"]
mod squire_tests;
