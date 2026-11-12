//! A Delve frame as a stage (`angel-stage`; docs/RENDER_FOUNDATION.md, "The
//! game pipeline"): the layers the window and the browser both draw from a
//! `RoomScene`, and the pictures they name, sent only when new or changed.
//!
//! In the room's own pixels, back to front: its still art scrolled by the
//! camera (an HD pack's finer drawing of it in the same rect, where it has
//! one), what is painted under the figures, each figure where it truly
//! stands (an HD drawing over any sprite a pack has one for), what is
//! painted over them, light added over all of it, motes, and last the
//! screen's own marks. A guardian's entrance leans the camera in.

use std::sync::Arc;

use angel_stage::scene::{AlphaMode, Material, MeshRef, Node, Props, Transform, World};
use angel_stage::{Frame, LAYER_PICTURES, Layer, LayerKind, Particles, Picture, Placement, Stand};

use super::*;

/// The stage's pictures: the room's still art, what is under and over the
/// figures, the figures' sheet, the screen's marks.
pub const GROUND: u32 = LAYER_PICTURES + 0x21;
pub const UNDER: u32 = LAYER_PICTURES + 0x22;
pub const SCREEN: u32 = LAYER_PICTURES + 0x23;
pub const FIGURES: u32 = LAYER_PICTURES + 0x24;
pub const OVER: u32 = LAYER_PICTURES + 0x25;
pub const SHOTS: u32 = LAYER_PICTURES + 0x26;
/// The floor rune's sign, over its ray-marched solid (`RoomScene::solids`).
const RUNE_SIGN: u32 = LAYER_PICTURES + 0x27;
/// The room-sized pictures (the ground, under, over, the screen) go in
/// square tiles, a picture a tile and `MAX_TILES` a kind from its base
/// here, so a fight that moves one part of the room re-sends that part,
/// not the whole room a frame (`send_tiled`).
const TILE_PX: i32 = 32;
const MAX_TILES: i32 = 0x400;
const GROUND_TILES: u32 = LAYER_PICTURES + 0x1000;
const UNDER_TILES: u32 = LAYER_PICTURES + 0x1400;
const OVER_TILES: u32 = LAYER_PICTURES + 0x1800;
const SCREEN_TILES: u32 = LAYER_PICTURES + 0x1c00;
/// A room in 3D (`RoomScene::world`): each figure's own picture from here
/// up, one a figure (a card shows the whole of a picture). Clear of the
/// window's other pictures (its HD art and hashed sprites sit below
/// `LAYER_PICTURES + 0x1000_1000`).
pub const WORLD_CARDS: u32 = LAYER_PICTURES + 0x2057_1000;
/// A pickup's drawing, as its card in a room in 3D (`pickup_cards`), by its
/// place on the plate sheet.
pub const PICKUP_CARDS: u32 = LAYER_PICTURES + 0x2057_2000;
/// The pickups' cards' node ids, past any figure's.
const PICKUP_NODES: u64 = 0x10_0000;
/// A turning pickup card's sides (`pickup_cards::face`, `back`): a face by
/// its drawing's place on the sheet and whether it shines, a back by
/// whether it shines.
pub const PICKUP_FACES: u32 = LAYER_PICTURES + 0x2057_3000;
const PICKUP_BACKS: u32 = PICKUP_FACES + 0x800;
/// The props' grain while a card turns among them: fine enough that its
/// drawing reads.
const CARD_GRAIN: u8 = 3;

/// The most figures a room in 3D stands up.
const MAX_CARDS: usize = 4096;

/// How far the floor tips away from the camera in a window that tilts it
/// (`angel_stage::Frame::tilt`): knights, monsters and shots stand up on
/// it, the room and its light lie on it, the screen's marks stay put.
pub const TILT: f32 = 0.3;

/// How fine the shots' shapes are drawn: this many pixels of the picture they
/// are drawn into to a pixel of the room (`angel_stage::scene::Props::grain`).
/// Fine enough that a fletching or a bead reads, coarse enough to be pixel art.
pub const SHOT_GRAIN: u8 = 2;

/// How much of a light's strength is added at its middle.
const LIGHT_GAIN: f32 = 0.3;
/// How far a fire's light wavers either way.
const LIGHT_FLICKER: f32 = 0.14;
/// A pool of light on the floor is wider than it is deep.
const LIGHT_SQUASH: f32 = 0.75;

/// What a stage was last sent: the room pictures' tiles (a tile goes again
/// only when it changed) and the figures' sheet, and the room shown, for
/// the way into the next.
#[derive(Default)]
pub struct StageMaker {
    /// The tiled room pictures as last sent, a kind each (`send_tiled`).
    ground: std::sync::Weak<Img>,
    ground_tiles: Option<TileKept>,
    under_tiles: Option<TileKept>,
    over_tiles: Option<TileKept>,
    screen_tiles: Option<TileKept>,

    sheet: Vec<u8>,
    /// Wire figures have independent pictures; one changing pose must not
    /// re-upload every other figure on the crowd sheet.
    flat_cards: Vec<Picture>,
    shot_sheet: Vec<u8>,
    room: u64,
    /// The pickups' models already sent (`item_pack`): to a window again
    /// with each new room, so one its GPU let go of comes back; a page
    /// across a wire asks for everything again when it lost one.
    meshes_sent: std::collections::BTreeSet<u32>,
    /// The Keep's stones and falsework (`world3d::works_site`) not yet sent
    /// to a page across a wire, and those that were (so a page that drew
    /// none for a while, its GPU having let them go, is sent them again).
    works_pending: Vec<(u32, Arc<angel_stage::scene::MeshData>)>,
    works_ids: std::collections::BTreeSet<u32>,
    /// The hillside works' baked meshes (`world3d::hill_mesh`) a page across a wire has been sent.
    hill_ids: std::collections::BTreeSet<u32>,
    /// Shots as flat sprites and a white-hot core, for a page that cannot
    /// draw the stage's 3D props (the legacy presenter). Otherwise a shot is
    /// shapes (`shot_meshes`) in the frame's props, and a sprite only if it
    /// is a card's own drawing. Such a page draws no room in 3D either: it
    /// lays the figures over the room's still art, as ever.
    pub flat_shots: bool,
    /// Each figure card's picture as last sent: its size and its pixels'
    /// words, compared as they are so an unchanged figure costs no copy.
    cards: Vec<(i32, i32, Vec<u32>)>,
    /// A drawer across a wire (a browser's renderer), which is sent a
    /// world by reference (a set's text, then its key) and makes its blocks
    /// itself.
    pub png_plates: bool,
    /// The kingdom in 3D: the chunks whose set text went to a page across a
    /// wire, by key, the most lately drawn last.
    chunk_texts: Vec<u64>,
    /// A lit room's pieces whose set text went, by key, the most lately
    /// drawn last (`world3d::rooms_lit`).
    lit_texts: Vec<u64>,
    /// The pickups' drawings sent, by their place on the plate sheet.
    plates_sent: Vec<u8>,
    /// The turning cards' sides sent, by picture.
    sides_sent: Vec<u32>,
    /// Which paintings over the figures were drawn last (`painted_key`).
    over_painted: u64,
    /// The figures' normal maps made lately, by their look (`look_hash`):
    /// a knight's few frames are made once, not each time it steps.
    normal_maps: std::collections::HashMap<u64, Vec<u8>>,
}

/// The most figures' normal maps a maker keeps at once.
const KEPT_NORMAL_MAPS: usize = 512;

impl StageMaker {
    /// A maker for a page that cannot draw the stage's props: shots are
    /// sprites, as they were, until it is said otherwise (`flat_shots`).
    pub fn flat() -> Self {
        Self { flat_shots: true, ..Self::default() }
    }

    /// A maker for a page across a wire (a browser): [`StageMaker::flat`],
    /// sending a world by reference (`png_plates`).
    pub fn for_wire() -> Self {
        Self { png_plates: true, ..Self::flat() }
    }

    /// Send everything again (the drawer lost its pictures).
    pub fn forget(&mut self) {
        self.ground = std::sync::Weak::new();
        self.ground_tiles = None;
        self.under_tiles = None;
        self.over_tiles = None;
        self.screen_tiles = None;
        self.sheet.clear();
        self.flat_cards.clear();
        self.shot_sheet.clear();
        self.cards.clear();
        self.chunk_texts.clear();
        self.lit_texts.clear();
        self.plates_sent.clear();
        self.sides_sent.clear();
        self.over_painted = 0;
        self.meshes_sent.clear();
        self.works_pending.clear();
        self.works_ids.clear();
        self.hill_ids.clear();
    }

    /// `scene` as a stage frame. `hd` names the picture of an HD drawing of
    /// a sprite's look, when there is one: it is drawn over the figure in
    /// the sprite's place, the sprite's own pixels taken out from under it.
    pub fn make(&mut self, scene: &Arc<RoomScene>, hd: impl FnMut(&Body) -> Option<super::hd_pack::Placed>) -> Frame {
        self.make_with(scene, hd, |_| None)
    }

    /// `make`, and `paint` names the picture of a painting's frame, flipped
    /// or not, when the drawer has it (`paintings`): it is drawn in its
    /// inks' square, moved as they are, the inks taken out from under it.
    pub fn make_with(
        &mut self,
        scene: &Arc<RoomScene>,
        mut hd: impl FnMut(&Body) -> Option<super::hd_pack::Placed>,
        mut paint: impl FnMut(&Painting) -> Option<u32>,
    ) -> Frame {
        let send_tiles = if self.png_plates { send_tiled_sparse } else { send_tiled };
        let mut layers = Vec::new();
        let mut pictures = Vec::new();

        let (cx, cy) = scene.camera;
        let room_layer = |id: u32, img: &Img| {
            let (sx, sy, sw, sh) = img.span();
            Layer::new(
                u64::from(id),
                1,
                Placement::Pixels([sx as f32 - cx, sy as f32 - cy, sw as f32, sh as f32]),
                LayerKind::Picture(id),
            )
        };
        // A room in 3D stands where its still art would lie, its figures in
        // it, all drawn by the window (`Props::world`); a page that draws no
        // scenes has its still art, as ever.
        let world = scene.world.clone().filter(|_| !self.flat_shots);
        // The kingdom in 3D lies where its land would.
        let chunks = if self.flat_shots { &[][..] } else { &scene.chunks[..] };
        let (mut world_at, mut figures_at, mut cards) = (0, 0, Vec::new());
        // The kingdom's pickups that drop as models, among the shapes.
        let mut models = Vec::new();
        if !chunks.is_empty() {
            world_at = layers.len();
        } else if world.is_some() {
            world_at = layers.len();
        } else if let Some(ground) = &scene.ground {
            if let Some(fine) = &scene.fine {
                // Fine art keeps its full resolution in the room's native rect.
                if !self.ground.upgrade().is_some_and(|old| Arc::ptr_eq(&old, &fine.img)) {
                    pictures.push(picture(GROUND, &fine.img));
                    self.ground = Arc::downgrade(&fine.img);
                }
                layers.push(room_layer(GROUND, ground));
            } else {
                // The same `Arc` comes back while the ground is unchanged:
                // its tiles are last frame's without hashing a whole map.
                let tiles = match self.ground.upgrade() {
                    Some(old) if Arc::ptr_eq(&old, ground) && self.ground_tiles.is_some() => self
                        .ground_tiles
                        .as_ref()
                        .map(|k| k.layout.clone())
                        .unwrap_or_default(),
                    _ => send_tiles(&mut pictures, &mut self.ground_tiles, GROUND_TILES, GROUND, ground),
                };
                self.ground = Arc::downgrade(ground);
                room_tiles(&mut layers, ground, &tiles, (cx, cy));
            }
        }
        // The skills bar's veil: the room's still art darkened along the
        // view's foot, fixed to the screen; all that stands, moves or marks
        // the floor goes over it at full ink.
        if let Some([x, y, w, h]) = scene.veil {
            let mut veil = Layer::new(
                3,
                1,
                Placement::Pixels([x as f32, y as f32, w as f32, h as f32]),
                LayerKind::Solid([0.0, 0.0, 0.0, super::room_scene::VEIL_COVER]),
            );
            veil.stand = Stand::Screen;
            layers.push(veil);
        }
        if let Some(under) = &scene.under {
            let tiles = send_tiles(&mut pictures, &mut self.under_tiles, UNDER_TILES, UNDER, under);
            room_tiles(&mut layers, under, &tiles, (cx, cy));
        }
        for (i, solid) in scene.solids.iter().enumerate() {
            let (x, y) = (solid.x - cx, solid.y - cy);
            let mut shadow = Layer::new(
                0x3000 + i as u64,
                1,
                Placement::Pixels([x - 6.0, y + 7.0, 12.0, 3.0]),
                LayerKind::Glow([0.08, 0.07, 0.1, 0.55]),
            );
            shadow.stand = Stand::Floor;
            layers.push(shadow);
            let mut body = Layer::new(
                0x4000 + i as u64,
                2,
                Placement::Pixels([x - 10.0, y - 11.0, 20.0, 20.0]),
                LayerKind::Primitive {
                    shape: solid.shape,
                    color: [
                        f32::from(solid.material[0]) / 255.0,
                        f32::from(solid.material[1]) / 255.0,
                        f32::from(solid.material[2]) / 255.0,
                        1.0,
                    ],
                    phase: solid.phase,
                    // Simulation time owns the pose, including pause/freeze.
                    speed: 0.0,
                },
            );
            body.stand = Stand::Upright([x, y + 9.0]);
            if solid.rune == Some(crate::rules::runes::RuneKind::Invisibility) {
                body.opacity = 0.5;
            }
            layers.push(body);
            if let Some(kind) = solid.rune {
                // There is at most one floor rune. Keep its sign above the solid.
                pictures.push(picture(RUNE_SIGN, &super::runes::glyph(kind)));
                let mut sign = Layer::new(
                    0x5000 + i as u64,
                    2,
                    Placement::Pixels([x - 10.0, y - 11.0, 20.0, 20.0]),
                    LayerKind::Picture(RUNE_SIGN),
                );
                sign.stand = Stand::Upright([x, y + 9.0]);
                layers.push(sign);
            }
        }
        if world.is_some() || !chunks.is_empty() {
            // In the kingdom what lies on the floor stands up as its drawing
            // (`pickup_cards`), its pool of light laid under the figures; in
            // a room in 3D it turns as a card, as it does in 2D (below).
            let pickups = if chunks.is_empty() { Vec::new() } else { self.pickup_cards(scene, &mut pictures, &mut layers, &mut models) };
            figures_at = layers.len();
            cards = self.cards_of(scene, &mut pictures, &mut hd, &mut paint);
            cards.extend(pickups);
        } else if !scene.figures.is_empty() {
            let art: Vec<_> = scene.figures.iter().map(|f| with_art(f, &mut hd, &mut paint)).collect();
            let images: Vec<&Img> = art.iter().map(|(img, _)| img.as_ref()).collect();
            let (rects, ids) = if self.png_plates {
                let mut rects = Vec::with_capacity(images.len());
                let mut ids = Vec::with_capacity(images.len());
                for (i, img) in images.iter().enumerate() {
                    let id = LAYER_PICTURES + 0x3000 + i as u32;
                    let next = picture(id, img);
                    let same = self.flat_cards.get(i).is_some_and(|old|
                        old.width == next.width && old.height == next.height
                            && crate::render::overworld::ink::same_bytes(&old.rgba, &next.rgba));
                    if !same { pictures.push(next.clone()); }
                    if let Some(old) = self.flat_cards.get_mut(i) { *old = next; }
                    else { self.flat_cards.push(next); }
                    rects.push([0.0, 0.0, img.w as f32, img.h as f32]);
                    ids.push(id);
                }
                self.flat_cards.truncate(images.len());
                (rects, ids)
            } else {
                let ((w, h), rgba, rects) = figure_sheet(&images);
                if !crate::render::overworld::ink::same_bytes(&rgba, &self.sheet) {
                    self.sheet.clone_from(&rgba);
                    pictures.push(Picture { id: FIGURES, width: w, height: h, rgba });
                }
                (rects, vec![FIGURES; images.len()])
            };
            for (i, ((figure, rect), (_, drawings))) in scene.figures.iter().zip(rects).zip(&art).enumerate() {
                // Where it truly stands, between the room's pixels; on a
                // tilted floor it stands up on its feet.
                let (dx, dy) = (figure.frac.0 - cx, figure.frac.1 - cy);
                let foot = Stand::Upright([
                    figure.x as f32 + dx + rect[2] / 2.0,
                    (figure.depth * UNIT - cy).min(figure.y as f32 + dy + rect[3]),
                ]);
                let drawing = |d: &Drawing| {
                    let [x, y, w, h] = d.rect;
                    let mut layer = Layer::new(0x100 + i as u64, 2, Placement::Pixels([x + dx, y + dy, w, h]), LayerKind::Picture(d.id));
                    layer.angle = d.angle;
                    layer.stand = foot;
                    layer
                };
                // What lies under it (an aura's painting), then the figure,
                // then what is drawn over it.
                layers.extend(drawings.iter().filter(|d| d.under).map(drawing));
                let mut layer = Layer::new(
                    0x100 + i as u64,
                    2,
                    Placement::Pixels([figure.x as f32 + dx, figure.y as f32 + dy, rect[2], rect[3]]),
                    LayerKind::PicturePart(ids[i], rect),
                );
                layer.stand = foot;
                // A lifted figure draws a GPU drop-shadow under it (the
                // window's `layers.rs`), the sprite itself stays put.
                layer.height = figure.height;
                // A glowing figure adds light instead of covering (`expand()`
                // folds this into premultiplied `Quad.color` for the browser,
                // the window sees `additive` on the layer directly). CPU
                // compose ignores both, so parity is unaffected.
                layer.additive = figure.additive;
                layers.push(layer);
                layers.extend(drawings.iter().filter(|d| !d.under).map(drawing));
            }
        }
        if let Some(over) = &scene.over {
            // The paintings over the figures the drawer has, their inks
            // taken out of what is painted over the figures.
            let painted: Vec<(u32, &Painting)> = scene.paints.iter().filter_map(|p| Some((paint(p)?, p))).collect();
            let key = painted_key(&painted);
            // What was sent last had another set of paintings' inks in it:
            // every tile goes again.
            if key != self.over_painted {
                self.over_tiles = None;
                self.over_painted = key;
            }
            let effective: std::borrow::Cow<'_, Img> = if painted.is_empty() {
                std::borrow::Cow::Borrowed(over)
            } else {
                let mut bare = over.clone();
                for (_, p) in &painted {
                    take_inks(&mut bare, p, (0, 0));
                }
                std::borrow::Cow::Owned(bare)
            };
            let tiles = send_tiles(&mut pictures, &mut self.over_tiles, OVER_TILES, OVER, &effective);
            room_tiles(&mut layers, over, &tiles, (cx, cy));
            for (n, (id, p)) in painted.iter().enumerate() {
                let ([x, y, w, h], angle) = p.placed();
                let mut layer = Layer::new(0x6_0000 + n as u64, 1, Placement::Pixels([x - cx, y - cy, w, h]), LayerKind::Picture(*id));
                layer.angle = angle;
                layers.push(layer);
            }
        }
        // Shots that are shapes go in the props; the rest are sprites.
        let solid = !self.flat_shots;
        let sprite = |shot: &SceneShot| !solid || shot.form == ShotForm::Card;
        let mut shapes: Vec<Node> = std::mem::take(&mut models);
        if !scene.shots.is_empty() {
            // One sheet of the shots' sprites, each look once: found by its
            // pixels' hash, so a crowd of shots costs a pass each, never a
            // look against every other (a stacked knight's hundreds).
            // A painted shot the drawer has the picture of is its mark with
            // the card's inks taken out, its painting drawn in their place.
            let painted: Vec<Option<(u32, Img)>> = scene
                .shots
                .iter()
                .map(|shot| {
                    let p = shot.paint.as_ref().filter(|_| sprite(shot))?;
                    let id = paint(p)?;
                    let mut bare = shot.mark.clone();
                    clear_mask(&mut bare, &p.inks, p.at);
                    Some((id, bare))
                })
                .collect();
            let mut fallback = Vec::new();
            if self.flat_shots {
                for shot in &scene.shots {
                    if shot.mark.w == 0 {
                        if !fallback.iter().any(|(k, _)| *k == shot.form) { fallback.push((shot.form, super::volleys::ranged::mark(shot.form))); }
                    }
                }
            }
            let mut looks: Vec<&Img> = Vec::new();
            let mut seen: std::collections::HashMap<u64, Vec<usize>> = std::collections::HashMap::new();
            let which: Vec<Option<usize>> = scene
                .shots
                .iter()
                .zip(&painted)
                .map(|(shot, painted)| {
                    let mark = painted.as_ref().map_or_else(|| {
                        if shot.mark.w == 0 { fallback.iter().find(|(form,_)| *form == shot.form).map_or(&shot.mark, |(_,mark)|mark) } else { &shot.mark }
                    }, |(_, bare)| bare);
                    (sprite(shot) && mark.words().iter().any(|&w| w >> 24 != 0)).then(|| {
                        let same = seen.entry(look_hash(mark)).or_default();
                        match same.iter().copied().find(|&i| looks[i] == mark) {
                            Some(i) => i,
                            None => {
                                looks.push(mark);
                                same.push(looks.len() - 1);
                                looks.len() - 1
                            }
                        }
                    })
                })
                .collect();
            let rects = if looks.is_empty() {
                Vec::new()
            } else {
                let ((w, h), rgba, rects) = figure_sheet(&looks);
                if !crate::render::overworld::ink::same_bytes(&rgba, &self.shot_sheet) {
                    self.shot_sheet.clone_from(&rgba);
                    pictures.push(Picture { id: SHOTS, width: w, height: h, rgba });
                }
                rects
            };
            // Shots by the square of the room they are in, `CROWD_PX` a side,
            // so each counts its crowd among its neighbours only.
            let cell = |x: f32, y: f32| ((x / CROWD_PX).floor() as i32, (y / CROWD_PX).floor() as i32);
            let mut cells: std::collections::HashMap<(i32, i32), Vec<usize>> = std::collections::HashMap::new();
            for (i, shot) in scene.shots.iter().enumerate() {
                cells.entry(cell(shot.x, shot.y)).or_default().push(i);
            }
            for (i, (shot, look)) in scene.shots.iter().zip(which).enumerate() {
                // Light adds up: where shots crowd (a ring just thrown), each
                // glows less, so the crowd glows like one and never burns white.
                let (sx, sy) = cell(shot.x, shot.y);
                let crowd = (sx - 1..=sx + 1)
                    .flat_map(|x| (sy - 1..=sy + 1).map(move |y| (x, y)))
                    .filter_map(|c| cells.get(&c))
                    .flatten()
                    .filter(|&&o| {
                        let o = &scene.shots[o];
                        (o.x - shot.x).hypot(o.y - shot.y) < CROWD_PX
                    })
                    .count();
                let glow = 1.0 / (crowd as f32).sqrt();
                let flat = sprite(shot);
                let painting = painted[i].as_ref().zip(shot.paint.as_ref()).map(|((id, _), p)| (*id, p));
                shot_layers(&mut layers, i as u64, shot, (look.map(|n| rects[n]), painting), (cx, cy), glow, flat);
                if !flat {
                    shot_meshes::build(&mut shapes, i as u64, shot, (cx, cy));
                }
            }
        }
        // In the kingdom in 3D what lies on the floor is a card standing
        // (`pickup_cards`); in every room, 2D or 3D, a card turning over a
        // pool of light, a chest a chest: the deep floors keep the black
        // dungeon's language (docs/WORLD-3D.md, "Deeper floors").
        let mut grain = SHOT_GRAIN;
        if !self.flat_shots && chunks.is_empty() {
            for (i, item) in scene.pickups.iter().enumerate() {
                if item.kind == super::pickup_meshes::PickupKind::Chest {
                    let _ = super::pickup_meshes::build(&mut shapes, i as u64, item, (cx, cy));
                    continue;
                }
                let id = super::pickup_meshes::FIRST_NODE + i as u64 * super::pickup_meshes::NODES_PER;
                // What has an approved model drops onto the floor as it
                // (the operator, 2026-10-19: "the weapons should be 3d models
                // that plop on the ground"); the rest turn as cards.
                let plate = item.plate.unwrap_or_else(|| super::pickup_cards::plate_of(item.kind, ""));
                if let Some((mesh, model)) = super::item_pack::model(plate, item.pale) {
                    shapes.push(super::pickup_meshes::plop(id, item, mesh, model, plate, (cx, cy)));
                } else {
                    let (front, back) = self.card_sides(item, &mut pictures);
                    shapes.extend(super::pickup_cards::nodes(id, front, back, item, (cx, cy)));
                    grain = CARD_GRAIN;
                }
                // A room lit in 3D (the deep) lays a wider, warmer pool: the
                // dark there hides what lies in it otherwise.
                let (reach, glow) = if world.is_some() { (LOOT_GLOW.0, LOOT_GLOW.1) } else { (1.0, 1.0) };
                let (r, ry) = (super::pickup_cards::POOL * reach, super::pickup_cards::POOL * reach * LIGHT_SQUASH);
                let mut pool = Layer::new(
                    0x7_1000 + i as u64,
                    1,
                    Placement::Pixels([item.x - cx - r, item.y - cy - ry, 2.0 * r, 2.0 * ry]),
                    LayerKind::Glow(light_colour(Hue::Holy, super::pickup_cards::POOL_GLOW * glow)),
                );
                pool.additive = true;
                pool.flicker = 0.12;
                layers.push(pool);
            }
        }
        // The shapes lie over everything laid so far (the shots' light and
        // shadows, the figures) and under the room's light, motes and marks.
        let props_at = layers.len();
        // Light added over the room: warm pools that flicker round its
        // fires, and what burns and flies.
        for light in &scene.lights {
            let (r, ry) = (light.radius, light.radius * LIGHT_SQUASH);
            let seed = ((light.x as i64 as u64) << 20) ^ (light.y as i64 as u64);
            let mut layer = Layer::new(
                seed,
                4,
                Placement::Pixels([light.x - r - cx, light.y - ry - cy, 2.0 * r, 2.0 * ry]),
                LayerKind::Glow(light_colour(light.hue, light.strength * LIGHT_GAIN)),
            );
            layer.additive = true;
            if light.flicker {
                layer.flicker = LIGHT_FLICKER;
            }
            layers.push(layer);
        }
        for emitter in &scene.emitters {
            let mut layer = Layer::new(
                u64::from(emitter.seed),
                5,
                Placement::Pixels([
                    emitter.rect[0] - cx,
                    emitter.rect[1] - cy,
                    emitter.rect[2],
                    emitter.rect[3],
                ]),
                LayerKind::Particles(motes_of(emitter)),
            );
            layer.additive = true;
            layers.push(layer);
        }
        // A guardian's entrance: the camera leans in to meet it, then back.
        let mut lean = ([0.0, 0.0], 1.0);
        if let Some(([bx, by], seconds)) = scene.entrance {
            let zoom = entrance_zoom(seconds);
            if zoom > 1.0 {
                lean_in(&mut layers, [bx - cx, by - cy], zoom);
                lean_in_shapes(&mut shapes, [bx - cx, by - cy], zoom);
                lean_in_shapes(&mut cards, [bx - cx, by - cy], zoom);
                lean = ([bx - cx, by - cy], zoom);
            }
        }
        // Down into the deep: the cut held black, the camera close on the
        // party and easing back as the room's lamps come up
        // (`world3d::room::beat`).
        if let Some(([px, py], seconds)) = scene.descent {
            let zoom = crate::world3d::room::beat_zoom(seconds);
            if lean.1 == 1.0 && zoom > 1.0 {
                lean_in(&mut layers, [px - cx, py - cy], zoom);
                lean_in_shapes(&mut shapes, [px - cx, py - cy], zoom);
                lean_in_shapes(&mut cards, [px - cx, py - cy], zoom);
                lean = ([px - cx, py - cy], zoom);
            }
            let dark = crate::world3d::room::beat_dark(seconds);
            if dark > 0.0 {
                let mut veil = Layer::new(
                    4,
                    5,
                    Placement::Pixels([0.0, 0.0, scene.view.0 as f32, scene.view.1 as f32]),
                    LayerKind::Solid([0.0, 0.0, 0.0, dark]),
                );
                veil.stand = Stand::Screen;
                layers.push(veil);
            }
        }
        let screen_tiles =
            send_tiles(&mut pictures, &mut self.screen_tiles, SCREEN_TILES, SCREEN, &scene.screen);
        for (id, x, y, w, h) in screen_tiles {
            let mut marks = Layer::new(
                u64::from(id),
                6,
                Placement::Pixels([x as f32, y as f32, w as f32, h as f32]),
                LayerKind::Picture(id),
            );
            marks.stand = Stand::Screen;
            layers.push(marks);
        }
        if super::stand_back::on() {
            let (w, h) = (scene.view.0.max(1) as f32, scene.view.1.max(1) as f32);
            let mut sky = Layer::new(9_001, 40, Placement::Pixels([0.0, 0.0, w, h * 0.20]), LayerKind::Solid([0.62, 0.76, 0.86, 1.0]));
            sky.stand = Stand::Screen;
            layers.push(sky);
        }
        let kingdom = (!chunks.is_empty())
            .then(|| self.kingdom_world(scene, chunks, &mut pictures, std::mem::take(&mut cards), (world_at, figures_at)));
        // Through a door, the new room comes in the way the party went.
        let enter = scene.travelled_from(self.room);
        if self.room != scene.room && !self.png_plates {
            self.meshes_sent.clear();
        }
        self.room = scene.room;
        // A pastime's own screen: a backdrop laid under all else, no
        // ground, no room-painted over, no figures stood up yet. Its
        // pinhole render must not be tipped.
        let pastime_scene = scene.ground.is_none()
            && scene.under.is_some()
            && scene.over.is_none()
            && scene.world.is_none();
        // A model a pickup names goes up the first time it is named.
        let mut meshes = Vec::new();
        for node in &shapes {
            if let Some(angel_stage::scene::MeshRef::Uploaded(id)) = node.mesh
                && !self.meshes_sent.contains(&id)
                && let Some(mesh) = super::item_pack::mesh(id)
                && meshes.len() < angel_stage::wire::MAX_FRAME_MESHES
            {
                self.meshes_sent.insert(id);
                meshes.push((id, mesh.clone()));
            }
        }
        // The Keep's stones and falsework, new to a page across a wire.
        let room_for = angel_stage::wire::MAX_FRAME_MESHES.saturating_sub(meshes.len());
        let sent_now = self.works_pending.len().min(room_for);
        meshes.extend(self.works_pending.drain(..sent_now).map(|(id, m)| (id, (*m).clone())));
        Frame {
            size: (scene.view.0.max(1) as u32, scene.view.1.max(1) as u32),
            layers,
            pictures,
            scene: scene.room,
            enter,
            // The Atari board lies flat, as drawn (`atari`). A pastime's own
            // screen is already a pinhole render (`three.rs`): tipping the
            // frame tips it twice. Its actors stand up as Upright layers, and
            // the tilt they see is what `TILT` gives the room's figures; only
            // the flat backdrop is left level.
            tilt: if scene.board || pastime_scene { 0.0 } else if super::stand_back::on() { 0.62 } else { TILT },
            impact: scene.impact,
            props: match world {
                _ if kingdom.is_some() => Some(Props { at: props_at, grain: SHOT_GRAIN, nodes: shapes, world: kingdom }),
                Some(world) => {
                    let world = self.lit_world(scene, &world, (cx, cy), lean, cards, (world_at, figures_at));
                    Some(Props { at: props_at, grain: SHOT_GRAIN, nodes: shapes, world: Some(world) })
                }
                None => (!shapes.is_empty()).then_some(Props { at: props_at, grain, nodes: shapes, world: None }),
            },
            meshes,
        }
    }

    /// A lit room's world (`world3d::rooms_lit`, docs/REALM-MONOLITH.md
    /// "Rooms, lit"): its pieces in sight lit and toned on the stage, their
    /// flames burning as the room is fought, through the room's look; and by
    /// reference, each piece's set text the first time it is drawn, for a
    /// page that makes the same blocks itself. No pictures: no plate.
    fn lit_world(
        &mut self,
        scene: &RoomScene,
        world: &crate::world3d::room::RoomWorld,
        camera: (f32, f32),
        lean: ([f32; 2], f32),
        cards: Vec<Node>,
        (at, figures_at): (usize, usize),
    ) -> World {
        let view = (scene.view.0.max(1) as u32, scene.view.1.max(1) as u32);
        let sight = angel_stage::scene::stage_camera(TILT, view).view_projection(view.0 as f32 / view.1 as f32);
        let in_sight = |lo: [f32; 3], hi: [f32; 3]| sight.is_none_or(|m| seen(&m, lo, hi));
        // What burns in flight (embers, fireballs, blasts) lights the room
        // as lamps, the strongest first.
        let mut fires: Vec<&super::room_scene::SceneLight> = scene.lights.iter().filter(|l| l.hue == Hue::Fire).collect();
        fires.sort_by(|a, b| b.strength.total_cmp(&a.strength));
        let sparks: Vec<angel_stage::look::Lamp> = fires
            .iter()
            .map(|l| angel_stage::look::Lamp { at: [l.x, 8.0, l.y], reach: (l.radius * 2.0).max(44.0), strength: (l.strength * 1.4).clamp(0.6, 1.1) })
            .collect();
        let staged = world.lit_staged(camera, lean, (view.0 as f32, view.1 as f32), &sparks, &in_sight);
        let (scale, offset) = world.placing(camera, lean);
        let mut refs = Vec::new();
        for (key, text) in staged.sets {
            // The page's sets keep `KEPT_SETS`; what this one says went is
            // fewer, so the page has all of it.
            let had = match self.lit_texts.iter().position(|&k| k == key) {
                Some(i) => {
                    self.lit_texts.remove(i);
                    true
                }
                None => false,
            };
            self.lit_texts.push(key);
            if self.lit_texts.len() > angel_stage::set::KEPT_SETS - 8 {
                self.lit_texts.remove(0);
            }
            refs.push(angel_stage::scene::SetRef {
                key,
                text: (!had).then_some(text),
                plate: 0,
                png: None,
                scale,
                offset,
                patches: angel_stage::set::Patches::NONE,
            });
        }
        let mut refs = refs.into_iter();
        // The figures take the room's light, and the depth of field keeps
        // them sharp: each card stands on its foot (`card`).
        let mut look = staged.look;
        if matches!(world.lit().tone, crate::world3d::rooms_lit_look::Tone::Delve(_)) {
            // The play is lit where it stands (docs/WORLD-3D.md, "Deeper
            // floors"): a pool round each knight and a glow over what
            // lies on the floor, so a party sees what it walks into.
            let (_, offset) = world.placing(camera, lean);
            let zoom = lean.1;
            let pools: Vec<angel_stage::look::Lamp> = play_lamps(scene, &world.moment)
                .into_iter()
                .map(|l| angel_stage::look::Lamp { at: [l.at[0] * zoom + offset[0], l.at[1] * zoom, l.at[2] * zoom + offset[2]], reach: l.reach * zoom, ..l })
                .collect();
            look.lamps.truncate(angel_stage::look::LAMP_MAX.saturating_sub(pools.len()));
            look.lamps.extend(pools);
        }
        let feet: Vec<[f32; 3]> = cards.iter().map(|c| [c.transform.translation[0], 0.0, c.transform.translation[2]]).collect();
        crate::world3d::rooms_lit_look::with_figures(&mut look, &feet);
        World {
            floor: staged.floor,
            blocks: staged.blocks,
            extra: staged.flames,
            at,
            figures: cards,
            figures_at,
            set: refs.next(),
            more: refs.collect(),
            look: Some(look),
            meshes: staged.meshes,
        }
    }

    /// The kingdom in 3D's world (`RoomScene::chunks`), the realm lit
    /// (docs/REALM-MONOLITH.md): each chunk the view may show (`seen`) as its
    /// ground and blocks toned and lit on the stage, its ground's mesh with
    /// it; to a page across a wire by reference instead (its set's text the
    /// first time), which the page fills lit itself. What the run stands
    /// (`world3d::lots`) as lit extra blocks. The look of the view (the
    /// biomes near it as zones, the sun, the near chunks' lamps, the knight
    /// kept in sight) goes with it. `cards` are the knights; `at` where the
    /// world and the figures lie among the layers.
    fn kingdom_world(
        &mut self,
        scene: &RoomScene,
        chunks: &[Arc<crate::world3d::kingdom::Chunk>],
        _pictures: &mut Vec<Picture>,
        cards: Vec<Node>,
        (at, figures_at): (usize, usize),
    ) -> World {
        use angel_stage::set::place;
        let (cx, cy) = scene.camera;
        let offset = [-cx, 0.0, -cy];
        let in_sight = kingdom_sight(scene);
        let mut world = World { at, figures: cards, figures_at, ..World::default() };
        let (mut refs, mut lots_refs) = (Vec::new(), Vec::new());
        let mut lamps = Vec::new();
        let touch = |list: &mut Vec<u64>, key: u64, most: usize| -> bool {
            let had = match list.iter().position(|&k| k == key) {
                Some(i) => {
                    list.remove(i);
                    true
                }
                None => false,
            };
            list.push(key);
            if list.len() > most {
                list.remove(0);
            }
            had
        };
        for chunk in chunks {
            // Every near chunk's lamps: a lamp out of sight lights into it.
            lamps.extend(chunk.set.lamps.iter().copied());
            // What the run stands in it that its set has not (`world3d::lots`):
            // lit extra blocks to a window, a set of their own to a page.
            if let Some(share) = scene.lots.as_ref().and_then(|l| l.share(chunk.index)) {
                lamps.extend(share.lamps.iter().copied());
                if !share.extras.is_empty() && in_sight(share.bounds) {
                    if self.png_plates {
                        let text = !touch(&mut self.chunk_texts, share.key, angel_stage::set::KEPT_SETS - 8);
                        lots_refs.push(angel_stage::scene::SetRef {
                            key: share.key,
                            text: text.then(|| Arc::clone(&share.text)),
                            plate: 0,
                            png: None,
                            scale: 1.0,
                            offset,
                            patches: angel_stage::set::Patches::NONE,
                        });
                    } else {
                        world.extra.extend(place(&share.lit(chunk), 1.0, offset));
                    }
                }
            }
            if !in_sight(chunk.bounds) {
                continue;
            }
            if self.png_plates {
                // The page's sets keep `KEPT_SETS`; what this one says went
                // is fewer, so the page has all of it.
                let text = !touch(&mut self.chunk_texts, chunk.key, angel_stage::set::KEPT_SETS - 8);
                refs.push(angel_stage::scene::SetRef {
                    key: chunk.key,
                    text: text.then(|| Arc::clone(&chunk.text)),
                    plate: 0,
                    png: None,
                    scale: 1.0,
                    offset,
                    patches: angel_stage::set::Patches::NONE,
                });
            } else {
                let lit = chunk.lit();
                world.floor.extend(place(&lit.floor, 1.0, offset));
                world.blocks.extend(place(&lit.standing, 1.0, offset));
                world.meshes.extend(lit.meshes.iter().map(|(id, mesh)| (*id, Arc::clone(mesh))));
            }
        }
        // The Keep's site: its stones and falsework, lamps and all.
        self.works_nodes(scene, &mut world, &mut lamps, offset, &in_sight);
        // The hillside works: the production sim's buildings, roads and tracks, and the
        // ghost of the builder.
        if let Some(hill) = &scene.hill {
            lamps.extend(hill.lamps.iter().copied());
            self.hill_nodes(hill, &mut world, offset, &in_sight);
            world.extra.extend(angel_stage::set::place(&hill.ghost, 1.0, offset));
        }
        let mut refs = refs.into_iter().chain(lots_refs);
        world.set = refs.next();
        world.more = refs.collect();
        // Where the stage is now (a seat's scene is lifted round its bands
        // after the view was made), the knight and the clock as the view had them.
        let view = crate::world3d::realm_look::View {
            origin: [cx, cy],
            size: [scene.view.0 as f32, scene.view.1 as f32],
            ..scene.realm.unwrap_or(crate::world3d::realm_look::View { origin: [0.0; 2], size: [0.0; 2], knight: None, others: [None; 3], time: 0.0 })
        };
        let mut look = crate::world3d::realm_look::look(&view, &lamps);
        // Each building near the view in what it is made of (timber, stone, brick, thatch,
        // iron, whitewash), over the hillside's green; before the Keep's, which lies over them.
        if let Some(hill) = &scene.hill {
            let feet: Vec<crate::world3d::works_look::Footprint> = hill.footprints.iter().map(|f| crate::world3d::works_look::Footprint::of(&f.plan, f.rect)).collect();
            crate::world3d::works_look::dye_buildings(&mut look, &view, &feet);
        }
        if let Some(works) = &scene.works {
            if !works.kinds.iter().any(|k| *k != 0) {
                crate::world3d::works_site::dye(&mut look, &view, works);
            }
        }
        if super::stand_back::on() {
            if let Some(reveal) = look.reveal.as_mut() {
                reveal.radius = 140.0;
            }
        }
        world.look = Some(look);
        world
    }

    /// The Keep's site (`world3d::works_site`) as nodes among the world's
    /// blocks: each mesh of stone or timber in sight, toned by how wet its
    /// mortar is; its meshes to a window with the world, to a page across a
    /// wire in the frame, a few a frame (what has not gone yet is left out,
    /// not drawn half).
    fn works_nodes(
        &mut self,
        scene: &RoomScene,
        world: &mut World,
        lamps: &mut Vec<angel_stage::set::SetLamp>,
        offset: [f32; 3],
        in_sight: &impl Fn(([f32; 3], [f32; 3])) -> bool,
    ) {
        use angel_stage::scene::{Material, MeshRef, Node, Role};
        /// Clear of the chunks' and the lots' ids.
        const IDS: u64 = 0x7B00_0000_0000;
        /// The most meshes a frame adds for a page.
        const MOST_NEW: usize = 40;
        let Some(works) = scene.works.as_ref() else { return };
        lamps.extend(works.lamps.iter().copied());
        if !in_sight(works.bounds) {
            // Out of sight a while: the page's GPU may let the meshes go.
            for id in std::mem::take(&mut self.works_ids) {
                self.meshes_sent.remove(&id);
            }
            return;
        }
        let mut nodes = Vec::new();
        let parts = works.parts.iter().enumerate().map(|(i, (p, wet))| (p, *wet, works.kinds.get(i).copied().unwrap_or(0))).chain(works.falsework.iter().map(|p| (p, 0.0, 0)));
        for (k, (part, wet, kind)) in parts.enumerate() {
            if !in_sight(part.bounds) {
                continue;
            }
            if self.png_plates {
                if !self.meshes_sent.contains(&part.id) {
                    if self.works_pending.len() >= MOST_NEW {
                        continue;
                    }
                    self.works_pending.push((part.id, Arc::clone(&part.mesh)));
                    self.meshes_sent.insert(part.id);
                }
                self.works_ids.insert(part.id);
            } else {
                world.meshes.push((part.id, Arc::clone(&part.mesh)));
            }
            // Wet mortar darkens a block; it dries as the courses go on.
            let t = 1.0 - 0.42 * wet;
            let (role, tint) = if kind == 0 {
                (Role::Plain, [t, t, t, 1.0])
            } else {
                let rgb = crate::rules::site::Site::stone_rgb(kind);
                (Role::Figure, [rgb[0] * t, rgb[1] * t, rgb[2] * t, 1.0])
            };
            let mut node = Node::new(IDS + k as u64, MeshRef::Uploaded(part.id), Material { role, ..Material::lit(tint, 0.0) });
            node.transform = angel_stage::scene::Transform::IDENTITY;
            nodes.push(node);
        }
        nodes.extend(works.life.iter().cloned().map(|mut n| {
            n.id += IDS;
            n
        }));
        world.extra.extend(angel_stage::set::place(&nodes, 1.0, offset));
    }

    /// The hillside works' baked meshes (`world3d::hill_mesh`) in sight as nodes among the world's
    /// blocks: one draw a building, a carrier or a chunk of road. Their meshes go to a window with
    /// the world (once each a frame), to a page across a wire a few a frame the first time they are
    /// drawn; a page that drew none of them for a frame forgets them, so it is sent them again.
    fn hill_nodes(&mut self, hill: &crate::world3d::hillside::HillNodes, world: &mut World, offset: [f32; 3], in_sight: &impl Fn(([f32; 3], [f32; 3])) -> bool) {
        use crate::world3d::hill_mesh::PartRole;
        use angel_stage::scene::{Material, MeshRef, Node, Role};
        /// Clear of the Keep's (`works_nodes`) and the ghost's.
        const IDS: u64 = 0x7D10_0000_0000;
        /// The most meshes a frame adds for a page.
        const MOST_NEW: usize = 40;
        let mut nodes = Vec::new();
        let mut sent_to_window = std::collections::BTreeSet::new();
        for part in &hill.parts {
            if !in_sight(part.bounds()) {
                continue;
            }
            let baked = &part.baked;
            if self.png_plates {
                if !self.meshes_sent.contains(&baked.id) {
                    if self.works_pending.len() >= MOST_NEW {
                        continue;
                    }
                    self.works_pending.push((baked.id, Arc::clone(&baked.mesh)));
                    self.meshes_sent.insert(baked.id);
                }
                self.hill_ids.insert(baked.id);
            } else if sent_to_window.insert(baked.id) {
                world.meshes.push((baked.id, Arc::clone(&baked.mesh)));
            }
            let material = match baked.role {
                PartRole::Plain => Material::lit([1.0, 1.0, 1.0, 1.0], 0.0),
                PartRole::Water => Material { role: Role::Water, ..Material::lit([1.0, 1.0, 1.0, 1.0], 0.0) },
                PartRole::Accent => Material { accent: true, ..Material::color([1.0, 1.0, 1.0, 1.0]) },
            };
            let mut node = Node::new(IDS + nodes.len() as u64, MeshRef::Uploaded(baked.id), material);
            node.transform.translation = part.at;
            nodes.push(node);
        }
        if nodes.is_empty() {
            // None in sight a while: the page's GPU may let the meshes go.
            for id in std::mem::take(&mut self.hill_ids) {
                self.meshes_sent.remove(&id);
            }
            return;
        }
        world.extra.extend(angel_stage::set::place(&nodes, 1.0, offset));
    }

    /// A turning card's face and back pictures for `item`, each sent the
    /// first time it is shown.
    fn card_sides(&mut self, item: &super::pickup_meshes::ScenePickup, pictures: &mut Vec<Picture>) -> (u32, u32) {
        use super::pickup_cards as pc;
        let index = item.plate.unwrap_or_else(|| pc::plate_of(item.kind, ""));
        let shine = u32::from(item.shine);
        let front = PICKUP_FACES + u32::from(index) * 2 + shine;
        let back = PICKUP_BACKS + shine;
        let (width, height) = pc::FACE;
        if !self.sides_sent.contains(&front) {
            pictures.push(Picture { id: front, width, height, rgba: pc::face(index, item.shine) });
            self.sides_sent.push(front);
        }
        if !self.sides_sent.contains(&back) {
            pictures.push(Picture { id: back, width, height, rgba: pc::back(item.shine) });
            self.sides_sent.push(back);
        }
        (front, back)
    }

    /// What lies on the floor in a room in 3D, as cards standing up with
    /// their drawings (`pickup_cards`), each drawing sent once, over a warm
    /// pool of light laid in the room's layers.
    fn pickup_cards(&mut self, scene: &RoomScene, pictures: &mut Vec<Picture>, layers: &mut Vec<Layer>, models: &mut Vec<Node>) -> Vec<Node> {
        use super::pickup_cards as pc;
        let mut cards = Vec::new();
        for (i, item) in scene.pickups.iter().enumerate() {
            let index = item.plate.unwrap_or_else(|| pc::plate_of(item.kind, ""));
            let Some(([x, y, w, h], foot)) = pc::place(item.kind, index, (item.x, item.y), scene.camera, item.tick) else {
                continue;
            };
            // What has a model drops onto the ground as it, over its pool.
            if let Some((mesh, model)) = super::item_pack::model(index, item.pale) {
                let id = super::pickup_meshes::FIRST_NODE + i as u64 * super::pickup_meshes::NODES_PER;
                models.push(super::pickup_meshes::plop(id, item, mesh, model, index, scene.camera));
                let (r, ry) = (pc::POOL, pc::POOL * LIGHT_SQUASH);
                let (fx, fy) = (item.x - scene.camera.0, item.y - scene.camera.1);
                let mut pool = Layer::new(
                    0x7_0000 + i as u64,
                    1,
                    Placement::Pixels([fx - r, fy - ry, 2.0 * r, 2.0 * ry]),
                    LayerKind::Glow(light_colour(Hue::Holy, pc::POOL_GLOW)),
                );
                pool.flicker = 0.12;
                layers.push(pool);
                continue;
            }
            let id = PICKUP_CARDS + u32::from(index);
            if !self.plates_sent.contains(&index)
                && let Some((pw, ph, rgba)) = pc::picture(index)
            {
                pictures.push(Picture { id, width: *pw, height: *ph, rgba: rgba.clone() });
                self.plates_sent.push(index);
            }
            let mut node = card(PICKUP_NODES + i as u64, id, [x, y, w, h], [x + w / 2.0, foot]);
            node.material.sampling = angel_stage::scene::Sampling::Linear;
            cards.push(node);
            let (r, ry) = (pc::POOL, pc::POOL * LIGHT_SQUASH);
            let mut pool = Layer::new(
                0x7_0000 + i as u64,
                1,
                Placement::Pixels([x + w / 2.0 - r, foot - ry, 2.0 * r, 2.0 * ry]),
                LayerKind::Glow(light_colour(Hue::Holy, pc::POOL_GLOW)),
            );
            pool.flicker = 0.12;
            layers.push(pool);
        }
        cards
    }

    /// A room in 3D's figures as cards (`card`), each its own picture (sent
    /// when it changed): every figure standing on its foot where its layer
    /// would, and any HD drawing of its sprites at its own place on the same
    /// foot, over it.
    fn cards_of(
        &mut self,
        scene: &RoomScene,
        pictures: &mut Vec<Picture>,
        hd: &mut impl FnMut(&Body) -> Option<super::hd_pack::Placed>,
        paint: &mut impl FnMut(&Painting) -> Option<u32>,
    ) -> Vec<Node> {
        let (cx, cy) = scene.camera;
        let figures = &scene.figures[..scene.figures.len().min(MAX_CARDS)];
        self.cards.truncate(figures.len());
        // A room in 3D lights its cards (the kingdom's look does not).
        let normals = scene.world.is_some() && crate::world3d::rooms_lit_look::lit_cards_on() && crate::world3d::rooms_lit_look::card_normals_on();
        let mut cards = Vec::new();
        for (i, figure) in figures.iter().enumerate() {
            let (img, drawings) = with_art(figure, hd, paint);
            if img.w <= 0 || img.h <= 0 {
                continue;
            }
            let (dx, dy) = (figure.frac.0 - cx, figure.frac.1 - cy);
            let rect = [figure.x as f32 + dx, figure.y as f32 + dy, img.w as f32, img.h as f32];
            let foot = [rect[0] + rect[2] / 2.0, (figure.depth * UNIT - cy).min(rect[1] + rect[3])];
            let id = WORLD_CARDS + i as u32;
            let same = self.cards.get(i).is_some_and(|(w, h, words)| {
                (*w, *h) == (img.w, img.h) && crate::render::overworld::ink::same_words(words, img.words())
            });
            if !same {
                let rgba = card_rgba(&img);
                // Which way each of its pixels faces, for a look that lights
                // it pixel by pixel (`Cards::normals`), kept with it.
                if normals {
                    let key = look_hash(&img);
                    if self.normal_maps.len() >= KEPT_NORMAL_MAPS && !self.normal_maps.contains_key(&key) {
                        self.normal_maps.clear();
                    }
                    let map = self
                        .normal_maps
                        .entry(key)
                        .or_insert_with(|| angel_stage::normals::of_picture(&rgba, (img.w as u32, img.h as u32), 1.0));
                    pictures.push(Picture { id: angel_stage::normals::picture_of(id), width: img.w as u32, height: img.h as u32, rgba: map.clone() });
                }
                pictures.push(Picture { id, width: img.w as u32, height: img.h as u32, rgba });
                if i >= self.cards.len() {
                    self.cards.resize(i + 1, (0, 0, Vec::new()));
                }
                let (w, h, words) = &mut self.cards[i];
                (*w, *h) = (img.w, img.h);
                words.clear();
                words.extend_from_slice(img.words());
            }
            // A drawing stands as a card of its own, unturned: one under the
            // figure (a body alone, under what it carries) before it.
            for d in drawings.iter().filter(|d| d.under) {
                let [x, y, w, h] = d.rect;
                cards.push(card(cards.len() as u64 + 1, d.id, [x + dx, y + dy, w, h], foot));
            }
            cards.push(card(cards.len() as u64 + 1, id, rect, foot));
            for d in drawings.iter().filter(|d| !d.under) {
                let [x, y, w, h] = d.rect;
                cards.push(card(cards.len() as u64 + 1, d.id, [x + dx, y + dy, w, h], foot));
            }
        }
        cards
    }
}

/// A lit delve's pools of light for its play, as lamps (reach in the room's
/// pixels, strength): round each knight, and over what lies on the floor.
pub const KNIGHT_POOL: (f32, f32) = (38.0, 0.45);
pub const LOOT_POOL: (f32, f32) = (26.0, 0.6);
/// How much wider and warmer the glow under a pickup lies in a lit delve
/// than in a 2D room, which keeps `pickup_cards::POOL` and `POOL_GLOW`.
const LOOT_GLOW: (f32, f32) = (1.5, 1.8);

/// The lamps a lit delve lights its play with, in the room's pixels: a soft
/// pool at each knight's feet, and a glow over what lies on the floor.
fn play_lamps(scene: &RoomScene, moment: &crate::world3d::room::Moment) -> Vec<angel_stage::look::Lamp> {
    use angel_stage::look::Lamp;
    let mut out = Vec::new();
    for [x, y] in moment.party.iter().flatten() {
        out.push(Lamp { at: [*x, 7.0, *y], reach: KNIGHT_POOL.0, strength: KNIGHT_POOL.1 });
    }
    let (cx, cy) = (scene.camera.0 + scene.view.0 as f32 / 2.0, scene.camera.1 + scene.view.1 as f32 / 2.0);
    let mut loot: Vec<&super::pickup_meshes::ScenePickup> = scene.pickups.iter().collect();
    loot.sort_by(|a, b| (a.x - cx).hypot(a.y - cy).total_cmp(&(b.x - cx).hypot(b.y - cy)));
    for item in loot.into_iter().take(4) {
        out.push(Lamp { at: [item.x, 6.0, item.y], reach: LOOT_POOL.0, strength: LOOT_POOL.1 });
    }
    out
}

/// Whether any of the box `lo`..`hi` (stage pixels) is in sight of the
/// stage camera whose clip-space matrix is `m`: its corners' bounds in clip
/// space overlap the screen's (a corner behind the camera counts as in
/// sight).
/// Whether a box of the kingdom (a chunk's blocks', `Chunk::bounds`) is in
/// sight of `scene`'s stage camera, overlapping the screen (`seen`): a frame
/// draws only the chunks in sight,
/// whatever their plates (a set when painted, nodes when not); one near but
/// out of sight is in no frame, a window only making its plate ready.
fn kingdom_sight(scene: &RoomScene) -> impl Fn(([f32; 3], [f32; 3])) -> bool + use<> {
    let (cx, cy) = scene.camera;
    let view = (scene.view.0.max(1) as u32, scene.view.1.max(1) as u32);
    let sight = angel_stage::scene::stage_camera(TILT, view).view_projection(view.0 as f32 / view.1 as f32);
    move |(lo, hi)| {
        sight.is_none_or(|m| seen(&m, [lo[0] - cx, lo[1], lo[2] - cy], [hi[0] - cx, hi[1], hi[2] - cy]))
    }
}

fn seen(m: &angel_stage::scene::Mat4, lo: [f32; 3], hi: [f32; 3]) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for k in 0..8 {
        let p = [if k & 1 == 0 { lo[0] } else { hi[0] }, if k & 2 == 0 { lo[1] } else { hi[1] }, if k & 4 == 0 { lo[2] } else { hi[2] }];
        let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
        let w = row(3);
        if !(w.is_finite() && w > 1e-6) {
            return true;
        }
        let (x, y) = (row(0) / w, row(1) / w);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    x1 >= -1.0 && x0 <= 1.0 && y1 >= -1.0 && y0 <= 1.0
}

/// A picture standing up on the floor as a card, as its upright layer
/// would: `rect` its place in the stage's pixels, `foot` the floor point it
/// stands on. A pixel row `foot.y - h` up the stage is `h` over the floor,
/// and the whole card is as far off as its foot (`Tilt::camera`), so it
/// lands on the pixels the layer does, nearest, and hides behind what
/// stands in front of its foot. Its normal map is the picture's
/// (`normals::picture_of`), where the drawer has one: a figure's is made
/// with it (`cards_of`), a painting's or an HD drawing's by the drawer.
fn card(id: u64, picture: u32, [x, y, w, h]: [f32; 4], [_, foot]: [f32; 2]) -> Node {
    let material = Material {
        alpha: AlphaMode::Blend,
        double_sided: true,
        normals: Some(angel_stage::normals::picture_of(picture)),
        ..Material::picture(picture)
    };
    let mut node = Node::new(id, MeshRef::Quad, material);
    node.transform = Transform {
        translation: [x + w / 2.0, foot - y - h / 2.0, foot],
        scale: [w, h, 1.0],
        ..Transform::IDENTITY
    };
    node
}

/// A card's pixels as straight RGBA (`rgba_of`), its signal inks (game
/// state: a knight's mark, a foe's eyes, `ink::is_signal`) a step under
/// opaque, which a look's light leaves as drawn (`look::UNLIT_ALPHA`).
fn card_rgba(img: &Img) -> Vec<u8> {
    let mut rgba = rgba_of(img);
    for px in rgba.chunks_exact_mut(4) {
        if px[3] == 255 && crate::render::overworld::ink::is_signal([px[0], px[1], px[2]]) {
            px[3] = angel_stage::look::UNLIT_ALPHA;
        }
    }
    rgba
}

/// A picture's pixels as straight RGBA, transparent where it is.
pub fn rgba_of(img: &Img) -> Vec<u8> {
    img.words()
        .iter()
        .flat_map(|&w| [(w >> 16) as u8, (w >> 8) as u8, w as u8, (w >> 24) as u8])
        .collect()
}

/// Row `r` of the tile at `(x, y)`, `w` wide, in a kept picture `sw` wide.
fn tile_row(words: &[u32], sw: i32, x: i32, y: i32, w: i32, r: i32) -> &[u32] {
    &words[((y + r) * sw + x) as usize..((y + r) * sw + x + w) as usize]
}

fn rgba_rows(words: &[u32]) -> Vec<u8> {
    words
        .iter()
        .flat_map(|&w| [(w >> 16) as u8, (w >> 8) as u8, w as u8, (w >> 24) as u8])
        .collect()
}

/// A tiled picture as last sent: where it kept its pixels, and the words.
struct TileKept {
    span: (i32, i32, i32, i32),
    words: Vec<u32>,
    tiles: std::collections::BTreeMap<u32, Picture>,
    /// The tiles as last told for their layer: id, and where each lies in
    /// the kept pixels, replayed when the layer's pixels have not changed.
    layout: Vec<Tile>,
    /// The fingerprint of `words` when the tiles were cut.
    hash: u32,
}

impl TileKept {
    /// The fingerprint of a layer's own pixels: when it is unchanged the
    /// tiles are last frame's, however they were cut.
    fn fingerprint(words: &[u32]) -> u32 {
        let mut hash = 0xcbf29ce4u32;
        for &word in words {
            hash ^= word;
            hash = hash.wrapping_mul(0x1000193);
        }
        hash
    }

    /// True when the layer's pixels are exactly as when its tiles were cut.
    fn fresh(&self, span: (i32, i32, i32, i32), words: &[u32]) -> bool {
        self.span == span && !self.layout.is_empty() && Self::fingerprint(words) == self.hash
    }
}

/// One tile of a tiled picture for its layer: the picture, and where it
/// lies in the kept pixels.
type Tile = (u32, i32, i32, i32, i32);

/// `img` in square tiles of `TILE_PX`: the tiles whose words differ from
/// `kept` go into `pictures` (at `base`, a tile each, in reading order),
/// and every tile is told for its layer. A picture with more tiles than
/// its pictures hold (or keeping no pixels) goes whole at `whole`, as it
/// did before tiles.
fn send_tiled(
    pictures: &mut Vec<Picture>,
    kept: &mut Option<TileKept>,
    base: u32,
    whole: u32,
    img: &Img,
) -> Vec<Tile> {
    use crate::render::overworld::ink::same_words;
    let span = img.span();
    let (_, _, sw, sh) = span;
    let words = img.words();
    let (cols, tiles) = if sw > 0 && sh > 0 {
        let cols = (sw + TILE_PX - 1) / TILE_PX;
        let rows = (sh + TILE_PX - 1) / TILE_PX;
        (cols, cols * rows)
    } else {
        (0, 0)
    };
    if tiles == 0 || tiles > MAX_TILES || words.len() != (sw.max(0) * sh.max(0)) as usize {
        let same = kept.as_ref().is_some_and(|k| k.span == span && same_words(&k.words, words));
        if !same {
            pictures.push(picture(whole, img));
            let whole_tile = vec![(whole, 0, 0, sw.max(0), sh.max(0))];
            *kept = Some(TileKept { span, words: words.to_vec(), tiles: Default::default(), layout: whole_tile, hash: TileKept::fingerprint(words) });
        }
        return vec![(whole, 0, 0, sw.max(0), sh.max(0))];
    }
    let same_span = kept.as_ref().is_some_and(|k| k.span == span && k.words.len() == words.len());
    // The layer's own pixels are as they were: its tiles are as they were
    // too, however they were cut, and none need looking at again.
    if let Some(k) = kept.as_ref() {
        if k.fresh(span, words) {
            return k.layout.clone();
        }
    }
    let mut out = Vec::with_capacity(tiles as usize);
    let mut any = !same_span;
    for (i, (x, y)) in (0..tiles).map(|t| ((t % cols) * TILE_PX, (t / cols) * TILE_PX)).enumerate() {
        let (w, h) = (TILE_PX.min(sw - x), TILE_PX.min(sh - y));
        let changed = !same_span
            || (0..h).any(|r| {
                !same_words(
                    tile_row(words, sw, x, y, w, r),
                    tile_row(&kept.as_ref().expect("a kept span").words, sw, x, y, w, r),
                )
            });
        if changed {
            any = true;
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for r in 0..h {
                rgba.extend(rgba_rows(tile_row(words, sw, x, y, w, r)));
            }
            pictures.push(Picture { id: base + i as u32, width: w as u32, height: h as u32, rgba });
        }
        out.push((base + i as u32, x, y, w, h));
    }
    if any {
        *kept = Some(TileKept { span, words: words.to_vec(), tiles: Default::default(), layout: out.clone(), hash: TileKept::fingerprint(words) });
    }
    out
}

fn send_tiled_sparse(
    pictures: &mut Vec<Picture>,
    kept: &mut Option<TileKept>,
    base: u32,
    whole: u32,
    img: &Img,
) -> Vec<Tile> {
    use crate::render::overworld::ink::same_words;
    let span = img.span();
    let (_, _, sw, sh) = span;
    let words = img.words();
    let tiles = if sw > 0 && sh > 0 {
        let cols = (sw + TILE_PX - 1) / TILE_PX;
        let rows = (sh + TILE_PX - 1) / TILE_PX;
        cols * rows
    } else {
        0
    };
    if tiles == 0 || tiles > MAX_TILES || (sw + TILE_PX - 2) / TILE_PX >= 32 || (sh + TILE_PX - 2) / TILE_PX >= 32 || words.len() != (sw.max(0) * sh.max(0)) as usize {
        // A picture with no tiles to hold it (an open floor's whole map)
        // still rides the fingerprint: unchanged pixels are last frame's
        // picture, not a whole-map byte compare and send.
        if kept.as_ref().is_some_and(|k| k.fresh(span, words)) {
            return vec![(whole, 0, 0, sw.max(0), sh.max(0))];
        }
        let same = kept.as_ref().is_some_and(|k| k.span == span && same_words(&k.words, words));
        if !same {
            pictures.push(picture(whole, img));
            let whole_tile = vec![(whole, 0, 0, sw.max(0), sh.max(0))];
            *kept = Some(TileKept { span, words: words.to_vec(), tiles: Default::default(), layout: whole_tile, hash: TileKept::fingerprint(words) });
        }
        return vec![(whole, 0, 0, sw.max(0), sh.max(0))];
    }
    // Tile ids belong to room coordinates, not the current camera window.
    // A scrolling window used to invalidate every tile, even when only its
    // two edges changed. Reuse interior tiles without changing the wire.
    let (sx, sy, _, _) = span;
    let (first_x, first_y) = (sx / TILE_PX, sy / TILE_PX);
    let (last_x, last_y) = ((sx + sw - 1) / TILE_PX, (sy + sh - 1) / TILE_PX);
    let mut out = Vec::new();
    let mut sent = std::collections::BTreeMap::new();
    for ty in first_y..=last_y {
        for tx in first_x..=last_x {
            let (gx, gy) = ((tx * TILE_PX).max(sx), (ty * TILE_PX).max(sy));
            let (w, h) = (((tx + 1) * TILE_PX).min(sx + sw) - gx,
                          ((ty + 1) * TILE_PX).min(sy + sh) - gy);
            let (x, y) = (gx - sx, gy - sy);
            let id = base + (ty.rem_euclid(32) * 32 + tx.rem_euclid(32)) as u32;
            // Transparent padding is not art. Give the existing picture
            // quad just the ink's rectangle; a vanished tile needs no quad
            // or clearing upload. All three browser presenters already
            // place arbitrary picture rectangles this way.
            let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
            for r in 0..h {
                for (c, &word) in tile_row(words, sw, x, y, w, r).iter().enumerate() {
                    if word >> 24 != 0 {
                        left = left.min(c as i32);
                        top = top.min(r);
                        right = right.max(c as i32 + 1);
                        bottom = bottom.max(r + 1);
                    }
                }
            }
            if right <= left || bottom <= top { continue; }
            let (x, y, w, h) = (x + left, y + top, right - left, bottom - top);
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for r in 0..h {
                rgba.extend(tile_row(words, sw, x, y, w, r).iter().flat_map(|&word|
                    [(word >> 16) as u8, (word >> 8) as u8, word as u8, (word >> 24) as u8]));
            }
            let picture = Picture { id, width: w as u32, height: h as u32, rgba };
            let same = kept.as_ref().and_then(|k| k.tiles.get(&id)).is_some_and(|old|
                old.width == picture.width && old.height == picture.height
                    && crate::render::overworld::ink::same_bytes(&old.rgba, &picture.rgba));
            if !same { pictures.push(picture.clone()); }
            sent.insert(id, picture);
            out.push((id, x, y, w, h));
        }
    }
    *kept = Some(TileKept { span, words: Vec::new(), tiles: sent, layout: out.clone(), hash: TileKept::fingerprint(words) });
    out
}
/// A room picture's tile layers, where they lie on the room (a window onto
/// a great room keeps only part of it).
fn room_tiles(layers: &mut Vec<Layer>, img: &Img, tiles: &[Tile], (cx, cy): (f32, f32)) {
    let (sx, sy, _, _) = img.span();
    for &(id, x, y, w, h) in tiles {
        layers.push(Layer::new(
            u64::from(id),
            1,
            Placement::Pixels([(sx + x) as f32 - cx, (sy + y) as f32 - cy, w as f32, h as f32]),
            LayerKind::Picture(id),
        ));
    }
}

fn picture(id: u32, img: &Img) -> Picture {
    // The pixels it keeps (`Img::span`), which are all of them but for a
    // window onto a great room.
    let (_, _, w, h) = img.span();
    Picture {
        id,
        width: w.max(1) as u32,
        height: h.max(1) as u32,
        rgba: rgba_of(img),
    }
}

/// A figure with the sprites `hd` has drawings for taken out of it, and
/// those drawings: picture and place in the room's pixels.
/// A picture drawn over (or under) a figure in the room's pixels: an HD
/// drawing of a sprite's look, or a painting, moved and turned as it is.
struct Drawing {
    id: u32,
    rect: [f32; 4],
    angle: f32,
    under: bool,
}

/// A figure's picture with what is drawn in its sprites' places taken out
/// of it, and those drawings: an HD pack's (`hd`), and the paintings of
/// what it has on that the drawer has (`paint`).
fn with_art<'a>(
    figure: &'a Figure,
    hd: &mut impl FnMut(&Body) -> Option<super::hd_pack::Placed>,
    paint: &mut impl FnMut(&Painting) -> Option<u32>,
) -> (std::borrow::Cow<'a, Img>, Vec<Drawing>) {
    let mut img = std::borrow::Cow::Borrowed(&figure.img);
    let mut drawings = Vec::new();
    for p in &figure.paints {
        let Some(id) = paint(p) else {
            continue;
        };
        take_inks(img.to_mut(), p, (figure.x, figure.y));
        let (rect, angle) = p.placed();
        drawings.push(Drawing { id, rect, angle, under: p.under });
    }
    for body in &figure.bodies {
        let Some(super::hd_pack::Placed { id, rect, body: alone }) = hd(body) else {
            continue;
        };
        // A whole figure's drawing takes all its pixels, over it; a body's
        // alone takes those of its bare body that nothing it carries covers,
        // under it, so its arm and shield keep their inks on top.
        let bare = body.named.as_ref().and_then(|n| n.bare.as_deref()).filter(|_| alone);
        let taken = bare.unwrap_or(&body.img);
        let img = img.to_mut();
        for by in 0..body.img.h {
            for bx in 0..body.img.w {
                let Some(c) = taken.get(bx, by) else {
                    continue;
                };
                let (fx, fy) = (body.x + bx - figure.x, body.y + by - figure.y);
                if body.img.get(bx, by) == Some(c) && img.get(fx, fy) == Some(c) {
                    img.clear(fx, fy);
                }
            }
        }
        drawings.push(Drawing { id, rect, angle: 0.0, under: bare.is_some() });
    }
    (img, drawings)
}

/// A painting's inks taken out of `img`, whose top-left is at `origin` in
/// the painting's pixels: each pixel still as the inks painted it.
fn take_inks(img: &mut Img, p: &Painting, origin: (i32, i32)) {
    for by in 0..p.inks.h {
        for bx in 0..p.inks.w {
            let Some(c) = p.inks.get(bx, by) else {
                continue;
            };
            let (x, y) = (p.at.0 + bx - origin.0, p.at.1 + by - origin.1);
            if img.get(x, y) == Some(c) {
                img.clear(x, y);
            }
        }
    }
}

/// Every pixel of `img` under the inked pixels of `inks` at `at` cleared,
/// whatever colour it took after (a shot's mark is tinted whole).
fn clear_mask(img: &mut Img, inks: &Img, at: (i32, i32)) {
    for by in 0..inks.h {
        for bx in 0..inks.w {
            if inks.get(bx, by).is_some() {
                img.clear(at.0 + bx, at.1 + by);
            }
        }
    }
}

/// Which paintings over the figures were drawn in place of their inks, as
/// a number: when it changes, what is over the figures goes again.
fn painted_key(painted: &[(u32, &Painting)]) -> u64 {
    let mut parts = Vec::with_capacity(painted.len() * 3);
    for (id, p) in painted {
        parts.extend([u64::from(*id), (p.at.0 as u32 as u64) << 32 | p.at.1 as u32 as u64, look_hash(&p.inks)]);
    }
    super::paintings::phase_of(&parts) as u64 | (painted.len() as u64) << 32
}

/// The figures of a frame on one sheet, a shelf at a time with a clear
/// pixel between them: its size, its pixels, and where each one is.
fn figure_sheet(images: &[&Img]) -> ((u32, u32), Vec<u8>, Vec<[f32; 4]>) {
    let width = images.iter().map(|img| img.w + 1).max().unwrap_or(1).max(256);
    let mut spots = Vec::with_capacity(images.len());
    let (mut x, mut y, mut shelf) = (0, 0, 0);
    for img in images {
        if x + img.w > width {
            (x, y, shelf) = (0, y + shelf + 1, 0);
        }
        spots.push((x, y));
        x += img.w + 1;
        shelf = shelf.max(img.h);
    }
    let height = (y + shelf).max(1);
    let (w, h) = (width as usize, height as usize);
    let mut rgba = vec![0u8; w * h * 4];
    for (img, &(sx, sy)) in images.iter().zip(&spots) {
        for (row, chunk) in img.words().chunks_exact(img.w.max(1) as usize).enumerate() {
            let start = ((sy as usize + row) * w + sx as usize) * 4;
            for (i, &word) in chunk.iter().enumerate() {
                rgba[start + i * 4..start + i * 4 + 4]
                    .copy_from_slice(&[(word >> 16) as u8, (word >> 8) as u8, word as u8, (word >> 24) as u8]);
            }
        }
    }
    let rects = images
        .iter()
        .zip(&spots)
        .map(|(img, &(sx, sy))| [sx as f32, sy as f32, img.w as f32, img.h as f32])
        .collect();
    ((width as u32, height as u32), rgba, rects)
}

/// A shot's look as one number: its size and every pixel, mixed.
fn look_hash(img: &Img) -> u64 {
    let mut h = (img.w as u64) << 32 | img.h as u32 as u64;
    for &w in img.words() {
        h = (h.rotate_left(5) ^ u64::from(w)).wrapping_mul(0x51_7cc1_b727_220a_95);
    }
    h
}

/// How high a shot flies over its point on the floor, in the room's pixels.
const SHOT_LIFT: f32 = shot_meshes::LIFT;
/// Shots closer than this (the room's pixels) share their light.
const CROWD_PX: f32 = 8.0;
/// How far behind a shot its trail reaches, in seconds of its flight.
const TRAIL_SECONDS: f32 = 0.09;

/// A shot as the window draws it: its shadow on the floor, and lifted over
/// it a trail of light behind it, a halo, and, if it is `flat`, its sprite and
/// a white-hot core. (A shot that is not flat is shapes in the frame's props,
/// `shot_meshes`, which are its body and its heat; the light stays here, under
/// and round them.) Arrows, blades and the like keep a faint streak and no glow.
fn shot_layers(
    layers: &mut Vec<Layer>,
    n: u64,
    shot: &SceneShot,
    (look, painting): (Option<[f32; 4]>, Option<(u32, &Painting)>),
    (cx, cy): (f32, f32),
    strength: f32,
    flat: bool,
) {
    let (x, y) = (shot.x - cx, shot.y - cy - if flat { SHOT_LIFT } else { shot.dimension.lift() });
    let id = 0x5_0000 + n * 16;
    // A knight's shot drawn smaller or bigger (`SceneShot::scale`): its
    // shadow, halo, streaks and sprite follow it.
    let k = shot.scale;
    // On a tilted floor all of it stands over the shot's point on the floor.
    let foot = Stand::Upright([shot.x - cx, shot.y - cy]);
    let glow = |id: u64, (gx, gy): (f32, f32), r: f32, colour: [f32; 4], additive: bool| {
        let mut layer = Layer::new(id, 3, Placement::Pixels([gx - r, gy - r, 2.0 * r, 2.0 * r]), LayerKind::Glow(colour));
        layer.additive = additive;
        if additive {
            layer.opacity = strength;
        }
        layer.stand = foot;
        layer
    };
    // The shadow keeps the floor under it.
    let mut shadow = Layer::new(
        id,
        3,
        Placement::Pixels([shot.x - cx - 4.0 * k, shot.y - cy - 1.2 * k, 8.0 * k, 2.4 * k]),
        LayerKind::Glow([0.0, 0.0, 0.0, 0.5]),
    );
    shadow.opacity = if shot.bright { 0.7 } else { 1.0 };
    layers.push(shadow);
    let [r, g, b, _] = light_colour(shot.hue, 1.0);
    let speed = shot.vx.hypot(shot.vy);
    let (dx, dy) = if speed > 0.0 { (shot.vx / speed, shot.vy / speed) } else { (0.0, 0.0) };
    let reach = speed * TRAIL_SECONDS;
    // A streak along the flight, `length` behind the shot and `width`
    // across, brightest where the shot is.
    let streak = |id: u64, length: f32, width: f32, colour: [f32; 4]| {
        let (mx, my) = (x - dx * length * 0.42, y - dy * length * 0.42);
        let mut layer = glow(id, (mx, my), 1.0, colour, true);
        layer.placement = Placement::Pixels([mx - length / 2.0, my - width / 2.0, length, width]);
        layer.angle = dy.atan2(dx);
        layer
    };
    if shot.bright {
        // A comet's tail: a long thin streak in its colour over a short
        // wide one, tapering off behind it.
        if speed > 0.0 {
            let length = (reach * 1.6).clamp(10.0, 34.0) * k.sqrt();
            layers.push(streak(id + 1, length, 5.0 * k, [r, g, b, 0.5]));
            layers.push(streak(id + 6, length * 0.55, 2.4 * k, [0.5 + 0.5 * r, 0.5 + 0.5 * g, 0.5 + 0.5 * b, 0.6]));
        }
        let halo = if shot.hostile { 9.0 } else { 7.5 * k };
        let mut layer = glow(id + 2, (x, y), halo, [r, g, b, 0.5], true);
        layer.flicker = 0.12;
        layers.push(layer);
        if shot.empowered {
            layers.push(glow(id + 3, (x, y), 11.0 * k, [1.0, 0.82, 0.35, 0.4], true));
        }
    } else if speed > 0.0 {
        // A speed line where it just was: a thin streak of air along its
        // flight, brightest at the shot.
        let length = (reach * 1.4).clamp(8.0, 26.0) * k.sqrt();
        layers.push(streak(id + 1, length, 2.2 * k, [0.92, 0.90, 0.82, 0.6]));
        if shot.empowered {
            layers.push(glow(id + 3, (x, y), 9.0 * k, [1.0, 0.82, 0.35, 0.35], true));
        }
    }
    if let Some(rect) = look {
        // The sprite at twice its size (and the shot's), its middle pixel
        // on the shot, as a terminal paints it.
        let mut sprite = Layer::new(
            id + 4,
            3,
            Placement::Pixels([x + 1.0 - 25.0 * k, y + 1.0 - 25.0 * k, rect[2] * 2.0 * k, rect[3] * 2.0 * k]),
            LayerKind::PicturePart(SHOTS, rect),
        );
        sprite.stand = foot;
        layers.push(sprite);
    }
    if let Some((picture, p)) = painting {
        // Its painting in the card's inks' square, from the shot's point.
        let ([px, py, w, h], angle) = p.placed();
        let mut layer = Layer::new(id + 7, 3, Placement::Pixels([x + px * k, y + py * k, w * k, h * k]), LayerKind::Picture(picture));
        layer.angle = angle;
        layer.stand = foot;
        layers.push(layer);
    }
    if shot.bright && flat {
        let core = [0.5 + 0.5 * r, 0.5 + 0.5 * g, 0.5 + 0.5 * b, 0.95];
        layers.push(glow(id + 5, (x, y), 2.8 * k, core, true));
    }
}

/// A light's colour, `alpha` its strength.
fn light_colour(hue: Hue, alpha: f32) -> [f32; 4] {
    let [r, g, b] = match hue {
        Hue::Fire => [1.0, 0.62, 0.30],
        Hue::Cold => [0.50, 0.68, 1.0],
        Hue::Gas => [0.56, 0.86, 0.46],
        Hue::Holy => [1.0, 0.90, 0.66],
    };
    [r, g, b, alpha.clamp(0.0, 1.0)]
}

/// How an emitter's motes look and move, in the room's pixels.
fn motes_of(emitter: &Emitter) -> Particles {
    let k = emitter.strength.clamp(0.0, 1.0);
    let (color, count, life, rise, sway) = match emitter.motes {
        Motes::Embers => ([1.0, 0.58, 0.24, 0.95], 2.0 + 6.0 * k, 1.4, 18.0, 1.5),
        Motes::Dust => ([0.95, 0.82, 0.62, 0.30], 6.0, 7.0, 6.0, 4.0),
        // A burst: thrown out at up to `rise` a second, falling at `sway`.
        Motes::Sparks => ([1.0, 0.86, 0.52, 1.0], 6.0 + 8.0 * k, 0.45, 70.0, 160.0),
    };
    Particles {
        color,
        count: count as u16,
        size: 1.0,
        life,
        rise,
        sway,
        seed: emitter.seed,
        burst: emitter.age,
    }
}

/// How far the camera has leaned in, `seconds` into a guardian's entrance:
/// in, a breath held, and back out.
fn entrance_zoom(seconds: f32) -> f32 {
    const IN: f32 = 0.5;
    const HOLD: f32 = 0.8;
    const OUT: f32 = 0.7;
    const LEAN: f32 = 0.14;
    let ease = |k: f32| {
        let k = k.clamp(0.0, 1.0);
        k * k * (3.0 - 2.0 * k)
    };
    let k = if seconds < IN {
        ease(seconds / IN)
    } else if seconds < IN + HOLD {
        1.0
    } else {
        1.0 - ease((seconds - IN - HOLD) / OUT)
    };
    1.0 + LEAN * k
}

/// The room's layers through a camera leaning in by `zoom` toward `focus`
/// (stage pixels): everything grows about it.
fn lean_in(layers: &mut [Layer], focus: [f32; 2], zoom: f32) {
    for layer in layers {
        let (Placement::Pixels(rect) | Placement::Cells(rect)) = &mut layer.placement;
        rect[0] = focus[0] + (rect[0] - focus[0]) * zoom;
        rect[1] = focus[1] + (rect[1] - focus[1]) * zoom;
        rect[2] *= zoom;
        rect[3] *= zoom;
        if let Stand::Upright(foot) = &mut layer.stand {
            foot[0] = focus[0] + (foot[0] - focus[0]) * zoom;
            foot[1] = focus[1] + (foot[1] - focus[1]) * zoom;
        }
        if let LayerKind::Particles(p) = &mut layer.kind {
            p.size *= zoom;
            p.rise *= zoom;
            p.sway *= zoom;
        }
    }
}

/// The shots' shapes through the same camera leaning in: everything grows
/// about `focus` (stage pixels, the floor's), the height with it.
fn lean_in_shapes(nodes: &mut [Node], focus: [f32; 2], zoom: f32) {
    for node in nodes {
        let t = &mut node.transform;
        t.translation[0] = focus[0] + (t.translation[0] - focus[0]) * zoom;
        t.translation[1] *= zoom;
        t.translation[2] = focus[1] + (t.translation[2] - focus[1]) * zoom;
        t.scale = t.scale.map(|v| v * zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lets a scene be made as the live drawer makes it: parts handed out,
    /// floor pickups handed over as cards turning; reset on drop.
    struct Cards;
    impl Cards {
        fn on() -> Self {
            draw_parts_outside(true);
            pickup_cards(true);
            Cards
        }
    }
    impl Drop for Cards {
        fn drop(&mut self) {
            draw_parts_outside(false);
            pickup_cards(false);
        }
    }

    /// A pickup's card in the props: its face and back quads and its edge box,
    /// none of them with a material to each face, so the wire carries all.
    fn assert_one_card(frame: &Frame) {
        let props = frame.props.as_ref().expect("the card rides in the props");
        assert_eq!(props.nodes.len(), 3, "face, back and edge");
        assert!(props.nodes.iter().all(|n| n.face_materials.is_empty()), "the wire drops per-face materials");
        assert!(matches!(props.nodes[0].mesh, Some(MeshRef::Quad)));
        assert!(matches!(props.nodes[1].mesh, Some(MeshRef::Quad)));
        assert!(matches!(props.nodes[2].mesh, Some(MeshRef::Cube)));
        let picture = |n: &Node| match n.material.source {
            angel_stage::scene::TextureSource::Picture(id) => Some(id),
            _ => None,
        };
        let (front, back) = (picture(&props.nodes[0]).unwrap(), picture(&props.nodes[1]).unwrap());
        assert_ne!(front, back);
        assert!(picture(&props.nodes[2]).is_none(), "the edge is a plain colour");
        let mut out = Vec::new();
        angel_stage::wire::encode(frame, &[], &mut out);
        let read = angel_stage::wire::decode(&out).expect("it reads");
        assert_eq!(read.frame.props.expect("props on the wire").nodes.len(), 3, "all three cross the wire");
    }

    #[test]
    fn sparse_scenery_sends_ink_not_clear_padding() {
        let mut kept = None;
        let mut pictures = Vec::new();
        let mut img = Img::new(96, 64);
        img.set(35, 17, [180, 120, 40]);
        let tiles = send_tiled_sparse(&mut pictures, &mut kept, OVER_TILES, OVER, &img);
        assert_eq!(pictures.iter().map(|p| p.rgba.len()).sum::<usize>(), 4);
        assert_eq!(tiles.len(), 1);
        assert_eq!((tiles[0].1, tiles[0].2, tiles[0].3, tiles[0].4), (35, 17, 1, 1));
        pictures.clear();
        img.clear(35, 17);
        assert!(send_tiled_sparse(&mut pictures, &mut kept, OVER_TILES, OVER, &img).is_empty());
        assert!(pictures.is_empty(), "removing a layer needs no transparent upload");
        img.set(35, 17, [180, 120, 40]);
        assert_eq!(send_tiled_sparse(&mut pictures, &mut kept, OVER_TILES, OVER, &img).len(), 1);
        assert_eq!(pictures.len(), 1, "a returning mark uploads its pixels");
    }

    #[test]
    fn sparse_tiles_reconstruct_after_camera_wrap_and_whole_picture_fallback() {
        let mut kept = None;
        let mut cache = std::collections::BTreeMap::new();
        for (left, width) in [(0, 128), (1, 128), (32, 128), (1024, 128), (0, 1056), (33, 128), (33, 128)] {
            let mut img = Img::window(1408, 96, (left, 0, width, 96));
            for y in 0..96 {
                for x in left..left + width {
                    if (x + y) % 7 != 0 { img.set(x, y, [(x % 199) as u8, y as u8, 40]); }
                }
            }
            let mut pictures = Vec::new();
            let tiles = send_tiled_sparse(&mut pictures, &mut kept, GROUND_TILES, GROUND, &img);
            for picture in pictures { cache.insert(picture.id, picture); }
            let mut restored = vec![0u32; img.words().len()];
            for (id, x, y, w, h) in tiles {
                let picture = &cache[&id];
                assert_eq!((picture.width, picture.height), (w as u32, h as u32));
                for r in 0..h {
                    for c in 0..w {
                        let i = ((r * w + c) * 4) as usize;
                        let rgba = &picture.rgba[i..i + 4];
                        restored[((y + r) * width + x + c) as usize] = u32::from_be_bytes([rgba[3], rgba[0], rgba[1], rgba[2]]);
                    }
                }
            }
            assert_eq!(restored, img.words(), "lost pixels at {left}, width {width}");
        }
    }

    /// A moving camera must not turn every unchanged room tile into a new
    /// texture. Only its entering/leaving edges need fresh pixels.
    #[test]
    fn scrolling_scenery_keeps_interior_tiles() {
        let mut kept = None;
        let mut pictures = Vec::new();
        let image = |left| {
            let mut img = Img::window(512, 256, (left, 0, 128, 96));
            for y in 0..96 {
                for x in left..left + 128 {
                    img.set(x, y, [(x % 200) as u8, y as u8, 40]);
                }
            }
            img
        };
        send_tiled_sparse(&mut pictures, &mut kept, GROUND_TILES, GROUND, &image(0));
        pictures.clear();
        send_tiled_sparse(&mut pictures, &mut kept, GROUND_TILES, GROUND, &image(1));
        assert!(pictures.len() <= 6, "camera motion re-uploaded {} tiles, not just two edges", pictures.len());
        pictures.clear();
        send_tiled_sparse(&mut pictures, &mut kept, GROUND_TILES, GROUND, &image(1));
        assert!(pictures.is_empty(), "an unchanged view sends no pixels");
    }

    #[test]
    fn pickup_3d_rune_keeps_its_sign_and_waits_until_it_spawns() {
        use crate::rules::runes::{Rune, RuneKind};
        let mut run = fight();
        run.dungeon.rooms[run.at].items.clear();
        run.rune = Some(Rune {
            kind: RuneKind::Invisibility, x: 20.0, y: 14.0, at: run.tick + 1,
        });
        let cards = Cards::on();
        let waiting = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        run.tick += 1;
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        drop(cards);
        assert!(waiting.pickups.is_empty(), "no card before the rune wells up");
        assert_eq!(scene.pickups.len(), 1);
        assert_eq!(scene.pickups[0].kind, super::super::pickup_meshes::PickupKind::Rune(RuneKind::Invisibility));
        // Its sign is its card's face: the rune's own drawing on the sheet.
        let face = scene.pickups[0].plate.unwrap_or_else(|| super::super::pickup_cards::plate_of(scene.pickups[0].kind, ""));
        assert_eq!(super::super::pickup_cards::PLATES[usize::from(face)], "rune-invisibility");
        let frame = StageMaker::default().make(&Arc::new(scene), |_| None);
        assert_one_card(&frame);
        assert!(frame.layers.iter().any(|l| matches!(l.kind, LayerKind::Glow(_)) && l.additive),
            "a pool of light under the rune");
        // Without the card pictures the rune turns as pixels over its ring.
        let terminal = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        assert!(terminal.pickups.is_empty());
        assert_eq!(terminal.compose(), frame_for(&run, NATIVE_W, NATIVE_H, Some(1)));
        run.rune = None;
        assert_ne!(terminal.compose(), scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).compose(), "the pixel rune is drawn");
    }

    #[test]
    fn pickup_3d_is_one_turning_card_and_has_a_pixel_fallback() {
        let mut run = fight();
        run.dungeon.rooms[run.at].items.clear();
        let empty_pixels = scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).compose();
        run.dungeon.rooms[run.at].items.push(crate::rules::Item {
            card: "bow".into(), x: 20.0, y: 14.0, held_off: None, took: Vec::new(), dropped: 0, owner: None,
        });
        let painted = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        assert!(painted.pickups.is_empty() && painted.solids.is_empty());
        assert_ne!(painted.compose(), empty_pixels, "terminal fallback lost the pickup");
        assert_eq!(painted.compose(), frame_for(&run, NATIVE_W, NATIVE_H, Some(1)));
        // A page that takes parts and draws shapes is handed the card, not pixels.
        let cards = Cards::on();
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        run.dungeon.rooms[run.at].items.clear();
        let empty = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        drop(cards);
        assert_eq!(scene.pickups.len(), 1);
        assert!(scene.solids.is_empty(), "no ray-marched solid any more");
        assert_eq!(scene.under, empty.under, "pickup must not also be baked into the floor");
        let frame = StageMaker::default().make(&Arc::new(scene), |_| None);
        assert_one_card(&frame);
        assert_eq!(frame.props.as_ref().unwrap().grain, CARD_GRAIN);
        assert!(frame.layers.iter().all(|l| !matches!(l.kind, LayerKind::Primitive { .. })));
    }

    #[test]
    fn a_room_with_an_item_in_2d_has_one_card_of_three_shapes_and_a_pool_of_light() {
        let mut run = fight();
        run.dungeon.rooms[run.at].items.clear();
        run.dungeon.rooms[run.at].items.push(crate::rules::Item {
            card: "bow".into(), x: 20.0, y: 14.0, held_off: None, took: Vec::new(), dropped: 0, owner: None,
        });
        let cards = Cards::on();
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        run.dungeon.rooms[run.at].items.clear();
        let bare = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        drop(cards);
        let frame = StageMaker::default().make(&Arc::new(scene), |_| None);
        let bare = StageMaker::default().make(&Arc::new(bare), |_| None);
        assert!(bare.props.is_none(), "no card, no props");
        assert_one_card(&frame);
        let props = frame.props.as_ref().unwrap();
        assert!(props.world.is_none(), "a room in 2D, its layers the room");
        let pools = |f: &Frame| f.layers.iter().filter(|l| matches!(l.kind, LayerKind::Glow(_)) && l.additive && l.flicker > 0.0).count();
        assert_eq!(pools(&frame), pools(&bare) + 1, "a pool of light under the card");
        // The face and back pictures are sent with it.
        assert_eq!(frame.pictures.iter().filter(|p| (PICKUP_FACES..PICKUP_FACES + 0x1000).contains(&p.id)).count(), 2);
    }

    #[test]
    fn a_dropped_sword_is_its_model_on_the_floor_sent_once_and_across_the_wire() {
        let mut run = fight();
        run.dungeon.rooms[run.at].items.clear();
        run.tick = 90;
        run.dungeon.rooms[run.at].items.push(crate::rules::Item::lying("knights-blade".into(), 20.0, 14.0, 90));
        let cards = Cards::on();
        let scene = Arc::new(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
        run.dungeon.rooms[run.at].items.clear();
        let bare = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        drop(cards);
        let mut maker = StageMaker::default();
        let frame = maker.make(&scene, |_| None);
        let props = frame.props.as_ref().expect("the model rides in the props");
        assert_eq!(props.nodes.len(), 1, "one node, the model");
        let Some(MeshRef::Uploaded(id)) = props.nodes[0].mesh else { panic!("a model") };
        assert_eq!(frame.meshes.iter().map(|(m, _)| *m).collect::<Vec<_>>(), vec![id], "its mesh goes with it");
        // Just let go of: up in the air, over its pool of light.
        assert!(props.nodes[0].transform.translation[1] > 6.0);
        let pools = |f: &Frame| f.layers.iter().filter(|l| matches!(l.kind, LayerKind::Glow(_)) && l.additive && l.flicker > 0.0).count();
        assert_eq!(pools(&frame), pools(&StageMaker::default().make(&Arc::new(bare), |_| None)) + 1);
        // The next frame of the same room names it and sends nothing more.
        assert!(maker.make(&scene, |_| None).meshes.is_empty());
        // Across a wire the mesh goes whole and comes back.
        let mut out = Vec::new();
        angel_stage::wire::encode(&frame, &[], &mut out);
        let read = angel_stage::wire::decode(&out).expect("it reads");
        assert_eq!(read.frame.meshes.len(), 1);
        assert_eq!(read.frame.props.unwrap().nodes[0].mesh, Some(MeshRef::Uploaded(id)));
    }

    #[test]
    fn what_a_knight_took_their_own_of_is_gone_from_their_view_and_marked_in_a_friends() {
        let mut run = fight();
        run.join("Ann");
        run.dungeon.rooms[run.at].items.clear();
        let mut item = crate::rules::Item::lying("bow".into(), 20.0, 14.0, 1);
        item.took.push(1);
        run.dungeon.rooms[run.at].items.push(item);
        let cards = Cards::on();
        let mine = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        let friends = scene_for(&run, NATIVE_W, NATIVE_H, Some(2));
        run.dungeon.rooms[run.at].items[0].took.clear();
        let unmarked = scene_for(&run, NATIVE_W, NATIVE_H, Some(2));
        drop(cards);
        assert!(mine.pickups.is_empty(), "the taker sees it gone");
        assert_eq!(friends.pickups.len(), 1, "it lies on for the friend");
        assert_ne!(friends.compose(), unmarked.compose(), "with a pip in the taker's colours");
        // Another's set-down piece is theirs alone.
        run.dungeon.rooms[run.at].items[0].owner = Some(1);
        let cards = Cards::on();
        assert_eq!(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).pickups.len(), 1);
        assert!(scene_for(&run, NATIVE_W, NATIVE_H, Some(2)).pickups.is_empty());
        drop(cards);
    }

    /// A first floor's fight, its knights at the door.
    fn fight() -> Run {
        let mut run = Run::new(11, 1, Some("Friend"));
        let index = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Fight && !r.great())
            .unwrap();
        run.enter_for_test(index);
        run
    }

    /// The Village, its party stood about the middle, as parts; with its
    /// world in 3D (`RoomScene::world`) when `world` says so.
    fn village(world: bool) -> RoomScene {
        use crate::rules::{home::Home, world as places};
        let mut run = Run::at_home(4243, 9, Some("Friend"), Home::default(), Default::default());
        run.arrive(places::room_of(&run.dungeon, RoomKind::Village).unwrap(), (12.0, 9.5));
        let mut scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        assert!(scene.world.is_none(), "rooms are not drawn in 3D unless the host says so");
        if world {
            let lit = crate::world3d::rooms_lit::lit_for(run.room(), run.dungeon.pack).expect("the Village stands up lit");
            scene.world = Some(crate::world3d::room::RoomWorld::lit_world(lit, (NATIVE_W, NATIVE_H)));
        }
        scene
    }

    /// The kingdom with the world in 3D, two knights out in Castle Town's
    /// Village.
    fn kingdom_3d() -> Arc<RoomScene> {
        super::super::props_meshes(true);
        crate::world3d::set_enabled(true);
        let run = crate::world3d::kingdom::proof_run(&[[1344.0, 1408.0], [1300.0, 1450.0]]);
        let (w, h) = super::super::seat_size();
        let scene = Arc::new(super::super::seat_scene(&run, w as i32, h as i32, Some(1)));
        crate::world3d::set_enabled(false);
        super::super::props_meshes(false);
        scene
    }

    #[test]
    fn the_kingdom_in_3d_stands_its_knights_among_its_lit_chunks() {
        let scene = kingdom_3d();
        assert!(scene.world.is_none() && !scene.chunks.is_empty(), "the kingdom by its chunks");
        assert!(!scene.is_flat());
        assert_eq!(scene.figures.len(), 2, "each knight a figure");
        let mut maker = StageMaker::default();
        let first = maker.make(&scene, |_| None);
        let world = first.props.as_ref().and_then(|p| p.world.clone()).expect("the world rides in the props");
        assert_eq!(world.figures.len(), 2, "a card a knight");
        assert!(!world.floor.is_empty() && !world.blocks.is_empty());
        // A scene draws one node an id.
        let mut ids: Vec<u64> = world.floor.iter().chain(&world.blocks).chain(&world.extra).map(|n| n.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "every chunk's nodes their own ids");
        // Lit and toned: no projector, no picture; every ground named is
        // among the meshes it carries.
        assert!(world.floor.iter().chain(&world.blocks).all(|n| n.material.projector.is_none()));
        for node in &world.floor {
            if let Some(angel_stage::scene::MeshRef::Uploaded(id)) = node.mesh {
                assert!(world.meshes.iter().any(|(m, _)| *m == id), "ground {id:#x} carried");
            }
        }
        assert!(!world.meshes.is_empty(), "the ground's height fields");
        let look = world.look.as_ref().expect("the realm's look");
        assert!(look.lights() && look.validate().is_ok() && look.reveal.is_some() == crate::world3d::realm_look::REVEAL_ON_STAGE);
        assert!(world.set.is_none(), "a window is sent nodes, not sets");
        // A page that cannot draw scenes has the kingdom flat, as ever.
        let flat = StageMaker::flat().make(&scene, |_| None);
        assert!(flat.props.is_none_or(|p| p.world.is_none()));
    }

    /// A page's frames of the kingdom: every chunk in sight in every frame
    /// by its set (its text the first time, then its key; no plate), which
    /// the page fills lit itself, the look with them; a chunk near but out
    /// of sight in none.
    fn a_page_draws_every_chunk_in_sight(scene: &Arc<RoomScene>) {
        let in_sight = kingdom_sight(scene);
        let shown: Vec<_> = scene.chunks.iter().filter(|c| in_sight(c.bounds)).collect();
        assert!(!shown.is_empty() && shown.len() <= scene.chunks.len());
        let mut keys_shown: Vec<u64> = shown.iter().map(|c| c.key).collect();
        keys_shown.sort_unstable();
        let mut maker = StageMaker { png_plates: true, ..StageMaker::default() };
        let check = |world: &World, first: bool| {
            let refs: Vec<_> = world.set.iter().chain(&world.more).collect();
            let mut keys: Vec<u64> = refs.iter().map(|r| r.key).filter(|k| scene.chunks.iter().any(|c| c.key == *k)).collect();
            keys.sort_unstable();
            assert_eq!(keys, keys_shown, "each chunk in sight by its set");
            // The rest: what the run stands, a set of its own a chunk.
            let shares: Vec<u64> = scene.lots.iter().flat_map(|l| l.shares.iter().map(|s| s.key)).collect();
            assert!(refs.iter().all(|r| keys_shown.contains(&r.key) || shares.contains(&r.key)));
            for r in &refs {
                assert_eq!((r.text.is_some(), r.png.is_some(), r.plate), (first, false, 0), "text the first time, then the key alone");
            }
            assert!(world.floor.is_empty() && world.blocks.is_empty(), "the page makes them");
            assert!(world.extra.iter().all(|n| n.id & 0xffff_ffff >= 0x7e00_0000), "only what a run stands goes as nodes");
            assert!(world.look.as_ref().is_some_and(|l| l.lights()));
        };
        let first = maker.make(scene, |_| None);
        let world = first.props.as_ref().and_then(|p| p.world.clone()).expect("the world");
        check(&world, true);
        // The page fills it lit, as the window has it, but baked.
        let mut sets = angel_stage::set::Sets::default();
        let mut made = world.clone();
        assert!(sets.fill(&mut made));
        let lit: usize = shown.iter().map(|c| c.set.shapes.len() + 1).sum();
        let stood: usize = scene.lots.iter().flat_map(|l| &l.shares).filter(|s| world.more.iter().any(|r| r.key == s.key)).map(|s| s.extras.len()).sum();
        // Baking reduces the node count: the page has far fewer nodes than the window.
        assert!(made.floor.len() + made.blocks.len() < lit + stood,
            "baking reduces nodes: page {} vs window {}",
            made.floor.len() + made.blocks.len(), lit + stood);
        // Each chunk's ground plus baked meshes.
        assert!(made.meshes.len() >= shown.len(), "a ground a chunk plus baked meshes");
        assert!(made.floor.iter().chain(&made.blocks).all(|n| n.material.projector.is_none()));
        let again = maker.make(scene, |_| None);
        check(&again.props.as_ref().and_then(|p| p.world.clone()).unwrap(), false);
        maker.forget();
        let all_again = maker.make(scene, |_| None);
        check(&all_again.props.and_then(|p| p.world).unwrap(), true);
    }

    #[test]
    fn a_page_is_sent_each_chunks_set_once_and_fills_it_lit() {
        a_page_draws_every_chunk_in_sight(&kingdom_3d());
    }

    #[test]
    fn a_room_in_3d_stands_its_figures_in_its_world() {
        let scene = Arc::new(village(true));
        assert!(!scene.figures.is_empty());
        let mut maker = StageMaker::default();
        let frame = maker.make(&scene, |_| None);
        let ids: Vec<u32> = frame.pictures.iter().map(|p| p.id).collect();
        // The still art does not go; the figures are each a picture, not a sheet.
        assert!(!ids.contains(&GROUND) && !ids.contains(&FIGURES), "{ids:x?}");
        assert_eq!(ids.iter().filter(|&&id| (WORLD_CARDS..PICKUP_CARDS).contains(&id)).count(), scene.figures.len());
        assert!(!frame.layers.iter().any(|l| matches!(l.kind, LayerKind::Picture(GROUND) | LayerKind::PicturePart(FIGURES, _))));
        let props = frame.props.as_ref().expect("the world rides in the props");
        let world = props.world.as_ref().expect("the room in 3D");
        assert!(!world.floor.is_empty() && !world.blocks.is_empty());
        assert_eq!(world.figures.len(), scene.figures.len(), "a card a figure");
        let set = world.set.as_ref().expect("its set by reference, for a page that makes the blocks itself");
        assert!(set.text.is_some(), "with its text in the room's first frame");
        assert!(set.png.is_none(), "no plate");
        // The world lies where the still art would, the figures where theirs would: over what is under them.
        assert_eq!(world.at, 0);
        let under = frame.layers[..world.figures_at]
            .iter()
            .position(|l| matches!(l.kind, LayerKind::Picture(id) if (UNDER_TILES..OVER_TILES).contains(&id)));
        assert!(under.is_some(), "what is painted under the figures lies under them");
        assert!(frame.layers[world.figures_at..]
            .iter()
            .any(|l| matches!(l.kind, LayerKind::Picture(id) if (OVER_TILES..SCREEN_TILES).contains(&id))));
        assert!(world.figures_at <= props.at);
        // Each card stands where its figure's layer would: on its foot, as far off as it.
        let flat = StageMaker::default().make(&Arc::new(village(false)), |_| None);
        let feet: Vec<[f32; 2]> = flat
            .layers
            .iter()
            .filter(|l| matches!(l.kind, LayerKind::PicturePart(FIGURES, _)))
            .map(|l| match l.stand {
                Stand::Upright(foot) => foot,
                _ => panic!("a figure stands"),
            })
            .collect();
        assert_eq!(feet.len(), world.figures.len());
        for (card, foot) in world.figures.iter().zip(&feet) {
            let t = card.transform;
            assert!((t.translation[0] - foot[0]).abs() < 1e-3 && (t.translation[2] - foot[1]).abs() < 1e-3, "{t:?} on {foot:?}");
            assert_eq!(card.material.alpha, AlphaMode::Blend);
            assert_eq!(card.material.sampling, angel_stage::scene::Sampling::Nearest);
        }
        // The set's text goes once, and a figure's picture again only when it changed.
        let again = maker.make(&scene, |_| None);
        let set = again.props.as_ref().and_then(|p| p.world.as_ref()).and_then(|w| w.set.as_ref()).unwrap();
        assert!(set.text.is_none(), "the set's text goes once");
        assert!(again.pictures.iter().all(|p| p.id < WORLD_CARDS), "{:x?}", again.pictures.iter().map(|p| p.id).collect::<Vec<_>>());
        maker.forget();
        let all = maker.make(&scene, |_| None);
        assert!(all.props.as_ref().and_then(|p| p.world.as_ref()).and_then(|w| w.set.as_ref()).is_some_and(|s| s.text.is_some()), "all again");
    }

    #[test]
    fn a_page_that_draws_no_scenes_keeps_the_rooms_still_art() {
        let frame = StageMaker::flat().make(&Arc::new(village(true)), |_| None);
        assert!(frame.props.is_none());
        assert!(frame.layers.iter().any(|l| matches!(l.kind, LayerKind::Picture(id) if (GROUND_TILES..UNDER_TILES).contains(&id))));
        assert!(frame.layers.iter().any(|l| matches!(l.kind, LayerKind::PicturePart(FIGURES, _))));
    }

    #[test]
    fn a_card_stands_where_its_upright_layer_would() {
        // A 10 x 20 picture at (30, 40), standing on row 58 (two rows above its bottom).
        let node = card(3, 77, [30.0, 40.0, 10.0, 20.0], [35.0, 58.0]);
        assert_eq!(node.id, 3);
        assert_eq!(node.material.source, angel_stage::scene::TextureSource::Picture(77));
        assert!(node.material.double_sided);
        // Centred across its place; its top 18 over the floor, its bottom 2 under; as far off as row 58.
        assert_eq!(node.transform.translation, [35.0, 8.0, 58.0]);
        assert_eq!(node.transform.scale, [10.0, 20.0, 1.0]);
    }

    #[test]
    fn figures_share_one_sheet_with_a_clear_pixel_between() {
        let mut a = Img::new(3, 2);
        a.set(2, 1, [9, 8, 7]);
        let b = Img::new(2, 4);
        let ((w, h), rgba, rects) = figure_sheet(&[&a, &b]);
        assert_eq!((w, h), (256, 4));
        assert_eq!(rects, vec![[0.0, 0.0, 3.0, 2.0], [4.0, 0.0, 2.0, 4.0]]);
        let at = (256 + 2) * 4;
        assert_eq!(&rgba[at..at + 4], &[9, 8, 7, 255]);
    }

    #[test]
    fn a_guardian_entrance_leans_in_and_back() {
        assert_eq!(entrance_zoom(0.0), 1.0);
        assert!(entrance_zoom(0.9) > 1.1);
        assert!((entrance_zoom(5.0) - 1.0).abs() < 1e-6);
        let mut layers = vec![Layer::new(0, 0, Placement::Pixels([10.0, 10.0, 4.0, 4.0]), LayerKind::Solid([1.0; 4]))];
        lean_in(&mut layers, [10.0, 10.0], 2.0);
        assert_eq!(layers[0].placement, Placement::Pixels([10.0, 10.0, 8.0, 8.0]));
    }

    #[test]
    fn a_picture_goes_only_when_it_changed() {
        let run = fight();
        let scene = Arc::new(scene_for(&run, NATIVE_W, NATIVE_H, Some(1)));
        let mut maker = StageMaker::default();
        let first = maker.make(&scene, |_| None);
        let second = maker.make(&scene, |_| None);
        let ids = |f: &Frame| f.pictures.iter().map(|p| p.id).collect::<Vec<_>>();
        let any = |f: &Frame, range: std::ops::Range<u32>| ids(f).iter().any(|id| range.contains(id));
        assert!(any(&first, GROUND_TILES..UNDER_TILES));
        assert!(!any(&second, GROUND_TILES..UNDER_TILES), "the room art once");
        assert!(ids(&second).is_empty(), "nothing changed, nothing sent");
        maker.forget();
        let third = maker.make(&scene, |_| None);
        assert!(
            any(&third, GROUND_TILES..UNDER_TILES)
                && any(&third, SCREEN_TILES..SCREEN_TILES + MAX_TILES as u32),
            "all again"
        );
        assert_eq!(first.size, (NATIVE_W as u32, NATIVE_H as u32));
        assert!(first.layers.iter().any(|l| l.additive), "its fires' light");
    }

    #[test]
    fn a_tiled_picture_goes_again_only_where_it_changed() {
        let mut img = Img::new(64, 64);
        img.set(3, 3, [200, 10, 10]);
        let (mut pictures, mut kept) = (Vec::new(), None);
        let tiles = send_tiled(&mut pictures, &mut kept, UNDER_TILES, UNDER, &img);
        assert_eq!(tiles.len(), 4, "four tiles of thirty-two square");
        assert_eq!(pictures.len(), 4, "all of it the first time");
        assert!(pictures.iter().all(|p| (p.width, p.height) == (TILE_PX as u32, TILE_PX as u32)));
        pictures.clear();
        let again = send_tiled(&mut pictures, &mut kept, UNDER_TILES, UNDER, &img);
        assert_eq!(again, tiles, "the same tiles are laid every frame");
        assert!(pictures.is_empty(), "nothing changed, nothing sent");
        img.set(40, 9, [10, 200, 10]);
        send_tiled(&mut pictures, &mut kept, UNDER_TILES, UNDER, &img);
        assert_eq!(pictures.len(), 1, "only the changed tile goes again");
        assert_eq!(pictures[0].id, UNDER_TILES + 1, "column 40 is the second tile across");
        assert_eq!(pictures[0].rgba.len(), TILE_PX as usize * TILE_PX as usize * 4);
    }

    #[test]
    fn pictures_are_told_apart_word_by_word_as_a_slice_would() {
        use crate::render::overworld::ink::{same_bytes, same_words};
        for len in [0usize, 1, 15, 16, 17, 64, 65, 1000] {
            let a: Vec<u32> = (0..len as u32).map(|i| i.wrapping_mul(2_654_435_761)).collect();
            assert!(same_words(&a, &a.clone()), "{len}");
            let bytes: Vec<u8> = a.iter().flat_map(|w| w.to_le_bytes()).collect();
            assert!(same_bytes(&bytes, &bytes.clone()), "{len}");
            for at in [0, len / 2, len.saturating_sub(1)].into_iter().filter(|&at| at < len) {
                let mut b = a.clone();
                b[at] ^= 1 << 7;
                assert!(!same_words(&a, &b), "{len} at {at}");
                let mut c = bytes.clone();
                c[at * 4 + 3] ^= 1;
                assert!(!same_bytes(&bytes, &c), "{len} at {at}");
            }
            assert!(!same_words(&a, &a[..len.saturating_sub(1)]) || len == 0);
        }
    }

    #[test]
    fn a_shaking_screen_is_an_impact_for_the_post_pass() {
        let mut run = fight();
        let impact = |run: &Run| StageMaker::default().make(&Arc::new(scene_for(run, NATIVE_W, NATIVE_H, Some(1))), |_| None).impact;
        assert_eq!(impact(&run), 0.0, "a calm room");
        run.shake = 12;
        assert_eq!(impact(&run), 1.0, "a guardian's fall");
        run.shake = 3;
        assert!((impact(&run) - 0.25).abs() < 1e-6, "dying away");
    }

    /// A fight with shots in flight, handed over whole as a window has it: an
    /// ember, a forged bolt and the Mage's own. Made once: handing frames
    /// over in parts is a switch every test in the process shares, and
    /// flipping it from many tests at once would show in the others.
    fn fight_with_shots() -> RoomScene {
        static SCENE: std::sync::OnceLock<RoomScene> = std::sync::OnceLock::new();
        SCENE
            .get_or_init(|| {
                let mut run = fight();
                let (x, y) = run.players.values().next().map(|h| (h.x, h.y)).unwrap();
                for (n, look) in [None, Some(["ember", "arrow"]), Some(["arcane", "bolt"])].into_iter().enumerate() {
                    let shot: crate::rules::Projectile = serde_json::from_value(serde_json::json!({
                        "x": x + 2.0 + 2.0 * n as f32, "y": y, "vx": 9.0, "vy": 0.0, "hostile": false,
                        "kind": "ember", "look": look, "damage": 1, "pierce": 0, "ttl": 30
                    }))
                    .unwrap();
                    run.projectiles.push(shot);
                }
                draw_parts_outside(true);
                let apart = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
                draw_parts_outside(false);
                let painted = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
                assert_eq!(apart.shots.len(), 3);
                assert!(painted.shots.is_empty(), "painted into the frame otherwise");
                apart
            })
            .clone()
    }

    /// Just the ember.
    fn fight_with_a_shot() -> RoomScene {
        let mut scene = fight_with_shots();
        scene.shots.truncate(1);
        scene
    }

    #[test]
    fn a_shot_is_said_to_be_what_it_is_for_a_window_that_builds_it() {
        let forms: Vec<ShotForm> = fight_with_shots().shots.iter().map(|s| s.form).collect();
        assert_eq!(
            forms,
            [
                ShotForm::Kind(crate::rules::Shot::Ember),
                ShotForm::Forged(ForgedColour::Ember, ForgedShape::Arrow),
                ShotForm::Class(ClassShot::Arcane),
            ]
        );
        // All three are shapes in the props, and none is a sprite.
        let frame = StageMaker::default().make(&Arc::new(fight_with_shots()), |_| None);
        let nodes = frame.props.expect("their shapes").nodes;
        let shots: std::collections::BTreeSet<u64> = nodes.iter().map(|n| (n.id - 0x5_0000_0000) / 32).collect();
        assert_eq!(shots.len(), 3);
        assert!(!frame.pictures.iter().any(|p| p.id == SHOTS));
    }

    #[test]
    fn a_flat_shot_in_flight_stands_up_lit_over_the_floor() {
        // A page that cannot draw props keeps the sprite and its white-hot core.
        let mut maker = StageMaker { flat_shots: true, ..Default::default() };
        let frame = maker.make(&Arc::new(fight_with_a_shot()), |_| None);
        assert!(frame.tilt > 0.0);
        assert!(frame.props.is_none(), "nothing for a page that cannot draw it");
        assert!(frame.pictures.iter().any(|p| p.id == SHOTS));
        let sprite = frame
            .layers
            .iter()
            .find(|l| matches!(l.kind, LayerKind::PicturePart(SHOTS, _)))
            .expect("the shot's sprite");
        // It stands on its point on the floor, lifted over it.
        let Stand::Upright([_, foot]) = sprite.stand else {
            panic!("a shot stands up");
        };
        let Placement::Pixels([_, top, _, h]) = sprite.placement else {
            unreachable!()
        };
        assert!(top + h / 2.0 < foot, "lifted over its foot");
        assert!(frame.layers.iter().any(|l| l.additive && matches!(l.kind, LayerKind::Glow(_)) && matches!(l.stand, Stand::Upright(_))));
    }

    #[test]
    fn a_solid_shot_is_shapes_in_the_props_with_its_light_under_them() {
        let scene = Arc::new(fight_with_a_shot());
        let frame = StageMaker::default().make(&scene, |_| None);
        let props = frame.props.as_ref().expect("the shot's shapes");
        assert_eq!(props.grain, SHOT_GRAIN);
        assert!(props.nodes.len() >= 2, "an ember is a veil round a core");
        // No sprite, no sprite sheet, no white-hot core: the shapes are the body and the heat.
        assert!(!frame.pictures.iter().any(|p| p.id == SHOTS));
        assert!(!frame.layers.iter().any(|l| matches!(l.kind, LayerKind::PicturePart(SHOTS, _))));
        let flat = StageMaker { flat_shots: true, ..Default::default() }.make(&scene, |_| None);
        assert_eq!(flat.layers.len(), frame.layers.len() + 2, "the sprite and the white-hot core are what the shapes replace");
        // The light stays, shadow and halo under, in the shot's layers; and the
        // shapes lie after them, before the room's own light and the marks.
        assert!(props.at > 0 && props.at < frame.layers.len());
        let under = &frame.layers[..props.at];
        assert!(under.iter().any(|l| l.additive && matches!(l.kind, LayerKind::Glow(_))), "its halo");
        assert!(under.iter().any(|l| !l.additive && matches!(l.kind, LayerKind::Glow(_))), "its shadow");
        assert!(frame.layers[props.at..].iter().any(|l| l.stand == Stand::Screen), "the screen's marks over");
        // The shapes stand where the shot does: over its point on the floor, lifted.
        let shot = &scene.shots[0];
        let (cx, cy) = scene.camera;
        for node in &props.nodes {
            let [x, y, z] = node.transform.translation;
            assert!((x - (shot.x - cx)).abs() < 12.0 && (z - (shot.y - cy)).abs() < 12.0 && y > 0.0, "{x} {y} {z}");
        }
        // A frame of nothing in flight has no props.
        let calm = Arc::new(scene_for(&fight(), NATIVE_W, NATIVE_H, Some(1)));
        assert!(StageMaker::default().make(&calm, |_| None).props.is_none());
    }

    #[test]
    fn a_cards_own_shot_stays_a_sprite_beside_the_shapes() {
        let mut scene = fight_with_a_shot();
        let mut card = scene.shots[0].clone();
        card.form = ShotForm::Card;
        card.x += 30.0;
        let mark = card.mark.clone();
        scene.shots.push(card);
        let frame = StageMaker::default().make(&Arc::new(scene), |_| None);
        let props = frame.props.as_ref().expect("the other shot's shapes");
        assert!(props.nodes.iter().all(|n| n.id < 0x5_0000_0020), "only the first shot made shapes");
        let sprites = frame.layers.iter().filter(|l| matches!(l.kind, LayerKind::PicturePart(SHOTS, _))).count();
        assert_eq!(sprites, usize::from(mark.words().iter().any(|&w| w >> 24 != 0)), "the card's drawing, and only it");
    }

    #[test]
    fn a_smaller_shot_is_a_smaller_sprite_halo_and_shadow_on_the_same_point() {
        let mut shot = fight_with_shots().shots[0].clone();
        let mut layers_at = |scale: f32| {
            shot.scale = scale;
            let mut layers = Vec::new();
            shot_layers(&mut layers, 0, &shot, (Some([0.0, 0.0, 25.0, 25.0]), None), (0.0, 0.0), 1.0, true);
            layers
        };
        let (old, small) = (layers_at(1.0), layers_at(0.5));
        assert_eq!(old.len(), small.len(), "the same layers");
        let rect = |layers: &[Layer], id: u64| match layers.iter().find(|l| l.id == id).expect("a layer").placement {
            Placement::Pixels(r) => r,
            Placement::Cells(r) => r,
        };
        // The sprite (id + 4) is half as wide and its middle pixel is still
        // on the shot's point; the halo (id + 2) and the shadow follow.
        let id = 0x5_0000;
        let (a, b) = (rect(&old, id + 4), rect(&small, id + 4));
        assert_eq!((a[2], b[2]), (50.0, 25.0));
        let mid = |r: [f32; 4]| (r[0] + 25.0 * r[2] / 50.0, r[1] + 25.0 * r[3] / 50.0);
        assert_eq!(mid(a), mid(b), "the same middle");
        assert!(rect(&small, id + 2)[2] < rect(&old, id + 2)[2], "a smaller halo");
        assert!(rect(&small, id)[2] < rect(&old, id)[2], "a smaller shadow");
        // At 1.0 it is exactly what it was: x - 24, 2 x the sprite.
        assert_eq!(a[0], shot.x - 24.0);
    }

    #[test]
    fn a_crowd_of_shots_sheets_each_look_once_and_shares_its_light() {
        // A stacked knight's crowd: three looks over and over.
        let base = fight_with_shots();
        let mut scene = base.clone();
        scene.shots.clear();
        for n in 0..300 {
            let mut shot = base.shots[n % 3].clone();
            shot.x += (n % 40) as f32 * 3.0;
            shot.y += (n / 40) as f32 * 3.0;
            scene.shots.push(shot);
        }
        let marks: Vec<Img> = base.shots.iter().map(|s| {
            if s.mark.w == 0 { super::super::volleys::ranged::mark(s.form) } else { s.mark.clone() }
        }).collect();
        let drawn = |s: &SceneShot| s.mark.w == 0 || s.mark.words().iter().any(|&w| w >> 24 != 0);
        let mut looks: Vec<&Img> = Vec::new();
        for mark in marks.iter().filter(|m| m.words().iter().any(|&w| w >> 24 != 0)) {
            if !looks.contains(&mark) { looks.push(mark); }
        }
        assert!(base.shots.iter().any(|s| s.mark.w == 0), "exercise procedural forms too");
        let frame = StageMaker::flat().make(&Arc::new(scene.clone()), |_| None);
        let sheet = frame.pictures.iter().find(|p| p.id == SHOTS).expect("the shots' sheet");
        let ((w, h), _, _) = figure_sheet(&looks);
        assert_eq!((sheet.width, sheet.height), (w, h), "each look once");
        let sprites = frame.layers.iter().filter(|l| matches!(l.kind, LayerKind::PicturePart(SHOTS, _))).count();
        assert_eq!(sprites, scene.shots.iter().filter(|s| drawn(s)).count(), "every shot its sprite");
        // Each shot's light is shared with its crowd, counted as every
        // other shot is measured against it.
        for (i, shot) in scene.shots.iter().enumerate() {
            let crowd = scene.shots.iter().filter(|o| (o.x - shot.x).hypot(o.y - shot.y) < CROWD_PX).count();
            let streak = frame.layers.iter().find(|l| l.id == 0x5_0000 + i as u64 * 16 + 1).expect("its streak");
            assert_eq!(streak.opacity, 1.0 / (crowd as f32).sqrt(), "shot {i}");
        }
    }

    #[test]
    fn the_shapes_lean_in_with_the_room() {
        let mut scene = fight_with_a_shot();
        scene.entrance = Some(([scene.shots[0].x + 20.0, scene.shots[0].y], 0.9));
        let leaning = StageMaker::default().make(&Arc::new(scene.clone()), |_| None);
        scene.entrance = None;
        let still = StageMaker::default().make(&Arc::new(scene), |_| None);
        let zoom = entrance_zoom(0.9);
        let (a, b) = (&still.props.unwrap().nodes[0], &leaning.props.unwrap().nodes[0]);
        assert!((b.transform.scale[0] / a.transform.scale[0] - zoom).abs() < 1e-5, "bigger by the zoom");
        assert!((b.transform.translation[1] / a.transform.translation[1] - zoom).abs() < 1e-5, "and higher");
        assert!(b.transform.translation[0] < a.transform.translation[0], "the focus is to its right, so it moves left of where it was");
    }

    #[test]
    fn an_hd_drawing_takes_its_sprite_out_from_under_it() {
        let run = fight();
        let scene = scene_for(&run, NATIVE_W, NATIVE_H, Some(1));
        let figure = scene.figures.iter().find(|f| !f.bodies.is_empty()).unwrap();
        let body = &figure.bodies[0];
        let (img, drawings) = with_art(
            figure,
            &mut |b: &Body| (b.key == body.key).then_some(super::hd_pack::Placed { id: 77, rect: [b.x as f32, b.y as f32, b.img.w as f32, b.img.h as f32], body: false }),
            &mut |_| None,
        );
        assert_eq!(drawings[0].id, 77);
        // A pixel of the sprite nothing was painted over.
        let at = |x: i32, y: i32| (body.x + x - figure.x, body.y + y - figure.y);
        let (bx, by) = (0..body.img.h)
            .flat_map(|y| (0..body.img.w).map(move |x| (x, y)))
            .find(|&(x, y)| {
                let (fx, fy) = at(x, y);
                body.img.get(x, y).is_some() && figure.img.get(fx, fy) == body.img.get(x, y)
            })
            .unwrap();
        let (fx, fy) = at(bx, by);
        assert_eq!(img.get(fx, fy), None);
    }
}
