//! A Delve frame as a stage (`angel-stage`; docs/RENDER_FOUNDATION.md, "The
//! game pipeline"): the layers the window and the browser both draw from a
//! `RoomScene`, and the pictures they name, sent only when new or changed.
//!
//! In the room's own pixels, back to front: its still art scrolled by the
//! camera, what is painted under the figures, each figure where it truly
//! stands (an HD drawing over any sprite a pack has one for), what is
//! painted over them, light added over all of it, motes, and last the
//! screen's own marks. A guardian's entrance leans the camera in.

use std::sync::{Arc, Weak};

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
/// A room in 3D (`RoomScene::world`): its plate, and each figure's own
/// picture from here up, one a figure (a card shows the whole of a picture).
/// Clear of the window's other pictures (its HD art and hashed sprites sit
/// below `LAYER_PICTURES + 0x1000_1000`).
pub const WORLD_PLATE: u32 = LAYER_PICTURES + 0x2057_0000;
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

/// The picture an item's plate is: each its own (`WORLD_PLATE` up), so a
/// drawer keeps every plate it was sent apart, by item.
pub fn plate_picture(item: &crate::world3d::assets::Item) -> u32 {
    WORLD_PLATE + item.slot.min(WORLD_CARDS - WORLD_PLATE - 1)
}
/// The most figures a room in 3D stands up.
const MAX_CARDS: usize = 4096;

/// An item's patches of its plate (`world3d::assets::Swatch`), for the
/// blocks a room has that its set has not: each item's own, a picture a
/// kind, from here up.
pub const WORLD_SWATCHES: u32 = LAYER_PICTURES + 0x2058_0000;

/// The kingdom in 3D (`RoomScene::chunks`): the chunks' plates from here
/// up, and their patches of their plates. A window keeps its plates in
/// [`kingdom::RESIDENT`] pictures (one a chunk it draws, the least lately
/// drawn given over to the next), so it never holds more; a page across a
/// wire, which decodes each chunk's PNG itself, one a chunk.
pub const WORLD_CHUNK_PLATES: u32 = LAYER_PICTURES + 0x2059_0000;
pub const WORLD_CHUNK_SWATCHES: u32 = LAYER_PICTURES + 0x205A_0000;

/// The picture chunk `index`'s patch of kind `kind` is.
pub fn chunk_swatch_picture(index: u32, kind: angel_stage::set::Kind) -> u32 {
    WORLD_CHUNK_SWATCHES + index.min(0xfff) * 32 + kind as u32
}

/// The picture all chunk `index`'s patches are in (`assets::swatch_atlas`),
/// for what its plate does not see: after its kinds' own.
pub fn chunk_swatch_atlas(index: u32) -> u32 {
    WORLD_CHUNK_SWATCHES + index.min(0xfff) * 32 + 31
}

/// A room with no item (`world3d::plain`): its theme's patches, a picture a
/// kind, each theme's own from here up, sent once a theme's patches.
pub const WORLD_THEME_SWATCHES: u32 = LAYER_PICTURES + 0x205B_0000;

/// The picture theme `theme`'s patch of kind `kind` is.
pub fn theme_swatch_picture(theme: crate::world3d::theme::Theme, kind: angel_stage::set::Kind) -> u32 {
    WORLD_THEME_SWATCHES + theme.slot() * 32 + kind as u32
}

/// A theme's patches as the pictures they are.
fn theme_pictures(patches: &crate::world3d::theme::Patches) -> impl Iterator<Item = Picture> + '_ {
    patches.swatches.iter().map(|s| Picture {
        id: theme_swatch_picture(patches.theme, s.kind),
        width: s.width,
        height: s.height,
        rgba: s.rgba.clone(),
    })
}

/// The picture item `item`'s patch of kind `kind` is.
pub fn swatch_picture(item: &crate::world3d::assets::Item, kind: angel_stage::set::Kind) -> u32 {
    WORLD_SWATCHES + item.slot.min(0xfff) * 32 + kind as u32
}

/// The picture all item `item`'s patches are in (`assets::swatch_atlas`),
/// for what its plate does not see: after its kinds' own.
pub fn swatch_atlas(item: &crate::world3d::assets::Item) -> u32 {
    WORLD_SWATCHES + item.slot.min(0xfff) * 32 + 31
}

/// Item `item`'s patches as the pictures they are: each kind's, and all of
/// them in one.
fn swatch_pictures(item: &crate::world3d::assets::Item) -> impl Iterator<Item = Picture> + '_ {
    let atlas = crate::world3d::assets::swatch_atlas(item.swatches())
        .map(|a| Picture { id: swatch_atlas(item), width: a.width, height: a.height, rgba: a.rgba });
    item.swatches()
        .iter()
        .map(|s| Picture { id: swatch_picture(item, s.kind), width: s.width, height: s.height, rgba: s.rgba.clone() })
        .chain(atlas)
}

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

/// What a stage was last sent: the still art, the scene before (each
/// picture goes again only when it changed) and the figures' sheet, and the
/// room shown, for the way into the next.
#[derive(Default)]
pub struct StageMaker {
    ground: Weak<Img>,
    last: Option<Arc<RoomScene>>,
    sheet: Vec<u8>,
    shot_sheet: Vec<u8>,
    room: u64,
    /// Shots as flat sprites and a white-hot core, for a page that cannot
    /// draw the stage's 3D props (the legacy presenter). Otherwise a shot is
    /// shapes (`shot_meshes`) in the frame's props, and a sprite only if it
    /// is a card's own drawing. Such a page draws no room in 3D either: it
    /// lays the figures over the room's still art, as ever.
    pub flat_shots: bool,
    /// A room in 3D: the plate sent (its item's key, 0 for none), and each
    /// figure's picture as last sent.
    plate: u64,
    /// Each figure card's picture as last sent: its size and its pixels'
    /// words, compared as they are so an unchanged figure costs no copy.
    cards: Vec<(i32, i32, Vec<u32>)>,
    /// A drawer across a wire (a browser's renderer), which is sent each
    /// item's plate as its PNG, once, in the world's set reference
    /// (`SetRef::png`), and decodes and keeps it itself; never its pixels.
    /// Otherwise the plate is a picture of its pixels, sent with each room.
    pub png_plates: bool,
    /// The items whose PNG went since everything was last sent.
    pngs_sent: Vec<u64>,
    /// The kingdom in 3D: the chunks whose set text went to a page across a
    /// wire, by key, the most lately drawn last.
    chunk_texts: Vec<u64>,
    /// The themes' patches sent (rooms with no item), by their key.
    themes_sent: Vec<u64>,
    /// A lit room's pieces whose set text went, by key, the most lately
    /// drawn last (`world3d::rooms_lit`).
    lit_texts: Vec<u64>,
    /// The pickups' drawings sent, by their place on the plate sheet.
    plates_sent: Vec<u8>,
    /// The turning cards' sides sent, by picture.
    sides_sent: Vec<u32>,
}

impl StageMaker {
    /// A maker for a page that cannot draw the stage's props: shots are
    /// sprites, as they were, until it is said otherwise (`flat_shots`).
    pub fn flat() -> Self {
        Self { flat_shots: true, ..Self::default() }
    }

    /// A maker for a page across a wire (a browser): [`StageMaker::flat`],
    /// sending a room in 3D's plate as its PNG (`png_plates`).
    pub fn for_wire() -> Self {
        Self { png_plates: true, ..Self::flat() }
    }

    /// Send everything again (the drawer lost its pictures).
    pub fn forget(&mut self) {
        self.ground = Weak::new();
        self.last = None;
        self.sheet.clear();
        self.shot_sheet.clear();
        self.plate = 0;
        self.cards.clear();
        self.pngs_sent.clear();
        self.chunk_texts.clear();
        self.themes_sent.clear();
        self.lit_texts.clear();
        self.plates_sent.clear();
        self.sides_sent.clear();
    }

    /// `scene` as a stage frame. `hd` names the picture of an HD drawing of
    /// a sprite's look, when there is one: it is drawn over the figure in
    /// the sprite's place, the sprite's own pixels taken out from under it.
    pub fn make(&mut self, scene: &Arc<RoomScene>, mut hd: impl FnMut(&Body) -> Option<u32>) -> Frame {
        let mut layers = Vec::new();
        let mut pictures = Vec::new();
        let last = self.last.replace(Arc::clone(scene));
        // A picture goes again only when it is not what was sent last.
        let changed = |now: &Option<Img>, then: Option<&Option<Img>>| then.is_none_or(|then| then != now);
        let (cx, cy) = scene.camera;
        // A picture of the room's pixels, where it lies on the room (a
        // window onto a great room keeps only part of it).
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
        let (mut world_at, mut figures_at, mut cards, mut new_room, mut send_png) = (0, 0, Vec::new(), false, false);
        if !chunks.is_empty() {
            world_at = layers.len();
        } else if let Some(world) = &world {
            if let Some(item) = world.item() {
                if self.plate != world.key() {
                    if !self.png_plates {
                        if let Some(plate) = item.plate() {
                            let id = plate_picture(item);
                            pictures.push(Picture { id, width: plate.width, height: plate.height, rgba: plate.rgba.clone() });
                        }
                        pictures.extend(swatch_pictures(item));
                    }
                    self.plate = world.key();
                    new_room = true;
                }
                if self.png_plates && !self.pngs_sent.contains(&item.key) {
                    self.pngs_sent.push(item.key);
                    send_png = true;
                    // Its patches go as pictures (they are small), with the plate.
                    pictures.extend(swatch_pictures(item));
                }
            } else if let Some(plain) = world.plain()
                && !self.themes_sent.contains(&plain.patches.key)
            {
                // A room with no item: its theme's patches, once a theme.
                self.themes_sent.push(plain.patches.key);
                pictures.extend(theme_pictures(&plain.patches));
            }
            world_at = layers.len();
        } else if let Some(ground) = &scene.ground {
            if !self.ground.upgrade().is_some_and(|old| Arc::ptr_eq(&old, ground)) {
                pictures.push(picture(GROUND, ground));
                self.ground = Arc::downgrade(ground);
            }
            layers.push(room_layer(GROUND, ground));
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
            if changed(&scene.under, last.as_ref().map(|l| &l.under)) {
                pictures.push(picture(UNDER, under));
            }
            layers.push(room_layer(UNDER, under));
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
            let pickups = if chunks.is_empty() { Vec::new() } else { self.pickup_cards(scene, &mut pictures, &mut layers) };
            figures_at = layers.len();
            cards = self.cards_of(scene, &mut pictures, &mut hd);
            cards.extend(pickups);
        } else if !scene.figures.is_empty() {
            let art: Vec<_> = scene.figures.iter().map(|f| with_hd(f, &mut hd)).collect();
            let images: Vec<&Img> = art.iter().map(|(img, _)| img.as_ref()).collect();
            let ((w, h), rgba, rects) = figure_sheet(&images);
            if !crate::render::overworld::ink::same_bytes(&rgba, &self.sheet) {
                self.sheet.clone_from(&rgba);
                pictures.push(Picture { id: FIGURES, width: w, height: h, rgba });
            }
            for (i, ((figure, rect), (_, drawings))) in scene.figures.iter().zip(rects).zip(&art).enumerate() {
                // Where it truly stands, between the room's pixels; on a
                // tilted floor it stands up on its feet.
                let (dx, dy) = (figure.frac.0 - cx, figure.frac.1 - cy);
                let foot = Stand::Upright([
                    figure.x as f32 + dx + rect[2] / 2.0,
                    (figure.depth * UNIT - cy).min(figure.y as f32 + dy + rect[3]),
                ]);
                let mut layer = Layer::new(
                    0x100 + i as u64,
                    2,
                    Placement::Pixels([figure.x as f32 + dx, figure.y as f32 + dy, rect[2], rect[3]]),
                    LayerKind::PicturePart(FIGURES, rect),
                );
                layer.stand = foot;
                layers.push(layer);
                for &(id, [x, y, w, h]) in drawings {
                    let mut layer = Layer::new(
                        0x100 + i as u64,
                        2,
                        Placement::Pixels([x + dx, y + dy, w, h]),
                        LayerKind::Picture(id),
                    );
                    layer.stand = foot;
                    layers.push(layer);
                }
            }
        }
        if let Some(over) = &scene.over {
            if changed(&scene.over, last.as_ref().map(|l| &l.over)) {
                pictures.push(picture(OVER, over));
            }
            layers.push(room_layer(OVER, over));
        }
        // Shots that are shapes go in the props; the rest are sprites.
        let solid = !self.flat_shots;
        let sprite = |shot: &SceneShot| !solid || shot.form == ShotForm::Card;
        let mut shapes: Vec<Node> = Vec::new();
        if !scene.shots.is_empty() {
            // One sheet of the shots' sprites, each look once: found by its
            // pixels' hash, so a crowd of shots costs a pass each, never a
            // look against every other (a stacked knight's hundreds).
            let mut looks: Vec<&Img> = Vec::new();
            let mut seen: std::collections::HashMap<u64, Vec<usize>> = std::collections::HashMap::new();
            let which: Vec<Option<usize>> = scene
                .shots
                .iter()
                .map(|shot| {
                    (sprite(shot) && shot.mark.words().iter().any(|&w| w >> 24 != 0)).then(|| {
                        let same = seen.entry(look_hash(&shot.mark)).or_default();
                        match same.iter().copied().find(|&i| looks[i] == &shot.mark) {
                            Some(i) => i,
                            None => {
                                looks.push(&shot.mark);
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
                shot_layers(&mut layers, i as u64, shot, look.map(|n| rects[n]), (cx, cy), glow, flat);
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
                let (front, back) = self.card_sides(item, &mut pictures);
                let id = super::pickup_meshes::FIRST_NODE + i as u64 * super::pickup_meshes::NODES_PER;
                shapes.extend(super::pickup_cards::nodes(id, front, back, item, (cx, cy)));
                let (r, ry) = (super::pickup_cards::POOL, super::pickup_cards::POOL * LIGHT_SQUASH);
                let mut pool = Layer::new(
                    0x7_1000 + i as u64,
                    1,
                    Placement::Pixels([item.x - cx - r, item.y - cy - ry, 2.0 * r, 2.0 * ry]),
                    LayerKind::Glow(light_colour(Hue::Holy, super::pickup_cards::POOL_GLOW)),
                );
                pool.additive = true;
                pool.flicker = 0.12;
                layers.push(pool);
                grain = CARD_GRAIN;
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
        if last.as_ref().is_none_or(|l| l.screen != scene.screen) {
            pictures.push(picture(SCREEN, &scene.screen));
        }
        let mut marks = Layer::new(
            2,
            6,
            Placement::Pixels([0.0, 0.0, scene.screen.w as f32, scene.screen.h as f32]),
            LayerKind::Picture(SCREEN),
        );
        marks.stand = Stand::Screen;
        layers.push(marks);
        let kingdom = (!chunks.is_empty())
            .then(|| self.kingdom_world(scene, chunks, &mut pictures, std::mem::take(&mut cards), (world_at, figures_at)));
        // Through a door, the new room comes in the way the party went.
        let enter = scene.travelled_from(self.room);
        self.room = scene.room;
        Frame {
            size: (scene.view.0.max(1) as u32, scene.view.1.max(1) as u32),
            layers,
            pictures,
            scene: scene.room,
            enter,
            tilt: TILT,
            impact: scene.impact,
            props: match world {
                _ if kingdom.is_some() => Some(Props { at: props_at, grain: SHOT_GRAIN, nodes: shapes, world: kingdom }),
                Some(world) => {
                    let world = match (world.item(), world.plain()) {
                        _ if world.lit().is_some() => self.lit_world(scene, &world, (cx, cy), lean, cards, (world_at, figures_at)),
                        (Some(item), _) => {
                            let plate = plate_picture(item);
                            let atlas = swatch_atlas(item);
                            let (floor, blocks) = world.nodes(plate, atlas, (cx, cy), lean);
                            // And by reference, for a page that makes them itself:
                            // the set's text with the room's first frame, the plate's
                            // PNG with the item's first (to a page that decodes it).
                            let set = world.set_ref(plate, atlas, (cx, cy), lean, new_room, send_png);
                            let extra = world.extras(&|kind| swatch_picture(item, kind), (cx, cy), lean);
                            World { floor, blocks, extra, at: world_at, figures: cards, figures_at, set, ..World::default() }
                        }
                        (None, plain) => {
                            // A room with no item: its own blocks, those of its
                            // pieces in sight, each of its theme's patches; across
                            // a wire, as nodes (no set's).
                            let theme = plain.map(|p| p.theme);
                            let swatch = |kind| theme.map_or(0, |t| theme_swatch_picture(t, kind));
                            let view = (scene.view.0.max(1) as u32, scene.view.1.max(1) as u32);
                            let sight = angel_stage::scene::stage_camera(TILT, view).view_projection(view.0 as f32 / view.1 as f32);
                            let in_sight = |lo: [f32; 3], hi: [f32; 3]| sight.is_none_or(|m| seen(&m, lo, hi));
                            let (floor, blocks) = world.plain_nodes(&swatch, (cx, cy), lean, (view.0 as f32, view.1 as f32), &in_sight);
                            let (floor, blocks, extra) = if self.png_plates {
                                (Vec::new(), Vec::new(), floor.into_iter().chain(blocks).collect())
                            } else {
                                (floor, blocks, Vec::new())
                            };
                            World { floor, blocks, extra, at: world_at, figures: cards, figures_at, set: None, ..World::default() }
                        }
                    };
                    Some(Props { at: props_at, grain: SHOT_GRAIN, nodes: shapes, world: Some(world) })
                }
                None => (!shapes.is_empty()).then_some(Props { at: props_at, grain, nodes: shapes, world: None }),
            },
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
        let Some(staged) = world.lit_staged(camera, lean, (view.0 as f32, view.1 as f32), &sparks, &in_sight) else {
            return World { at, figures: cards, figures_at, ..World::default() };
        };
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
        World {
            floor: staged.floor,
            blocks: staged.blocks,
            extra: staged.flames,
            at,
            figures: cards,
            figures_at,
            set: refs.next(),
            more: refs.collect(),
            look: Some(staged.look),
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
        world.look = Some(crate::world3d::realm_look::look(&view, &lamps));
        world
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
    fn pickup_cards(&mut self, scene: &RoomScene, pictures: &mut Vec<Picture>, layers: &mut Vec<Layer>) -> Vec<Node> {
        use super::pickup_cards as pc;
        let mut cards = Vec::new();
        for (i, item) in scene.pickups.iter().enumerate() {
            let index = item.plate.unwrap_or_else(|| pc::plate_of(item.kind, ""));
            let Some(([x, y, w, h], foot)) = pc::place(item.kind, index, (item.x, item.y), scene.camera, item.tick) else {
                continue;
            };
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
        hd: &mut impl FnMut(&Body) -> Option<u32>,
    ) -> Vec<Node> {
        let (cx, cy) = scene.camera;
        let figures = &scene.figures[..scene.figures.len().min(MAX_CARDS)];
        self.cards.truncate(figures.len());
        let mut cards = Vec::new();
        for (i, figure) in figures.iter().enumerate() {
            let (img, drawings) = with_hd(figure, hd);
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
                pictures.push(Picture { id, width: img.w as u32, height: img.h as u32, rgba: rgba_of(&img) });
                if i >= self.cards.len() {
                    self.cards.resize(i + 1, (0, 0, Vec::new()));
                }
                let (w, h, words) = &mut self.cards[i];
                (*w, *h) = (img.w, img.h);
                words.clear();
                words.extend_from_slice(img.words());
            }
            cards.push(card(cards.len() as u64 + 1, id, rect, foot));
            for &(drawing, [x, y, w, h]) in &drawings {
                cards.push(card(cards.len() as u64 + 1, drawing, [x + dx, y + dy, w, h], foot));
            }
        }
        cards
    }
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
/// stands in front of its foot.
fn card(id: u64, picture: u32, [x, y, w, h]: [f32; 4], [_, foot]: [f32; 2]) -> Node {
    let material = Material { alpha: AlphaMode::Blend, double_sided: true, ..Material::picture(picture) };
    let mut node = Node::new(id, MeshRef::Quad, material);
    node.transform = Transform {
        translation: [x + w / 2.0, foot - y - h / 2.0, foot],
        scale: [w, h, 1.0],
        ..Transform::IDENTITY
    };
    node
}

/// A picture's pixels as straight RGBA, transparent where it is.
pub fn rgba_of(img: &Img) -> Vec<u8> {
    img.words()
        .iter()
        .flat_map(|&w| [(w >> 16) as u8, (w >> 8) as u8, w as u8, (w >> 24) as u8])
        .collect()
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
fn with_hd<'a>(
    figure: &'a Figure,
    hd: &mut impl FnMut(&Body) -> Option<u32>,
) -> (std::borrow::Cow<'a, Img>, Vec<(u32, [f32; 4])>) {
    let mut img = std::borrow::Cow::Borrowed(&figure.img);
    let mut drawings = Vec::new();
    for body in &figure.bodies {
        let Some(id) = hd(body) else {
            continue;
        };
        let img = img.to_mut();
        for by in 0..body.img.h {
            for bx in 0..body.img.w {
                let Some(c) = body.img.get(bx, by) else {
                    continue;
                };
                let (fx, fy) = (body.x + bx - figure.x, body.y + by - figure.y);
                if img.get(fx, fy) == Some(c) {
                    img.clear(fx, fy);
                }
            }
        }
        drawings.push((id, [body.x as f32, body.y as f32, body.img.w as f32, body.img.h as f32]));
    }
    (img, drawings)
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
    look: Option<[f32; 4]>,
    (cx, cy): (f32, f32),
    strength: f32,
    flat: bool,
) {
    let (x, y) = (shot.x - cx, shot.y - cy - if flat { SHOT_LIFT } else { shot.dimension.lift() });
    let id = 0x5_0000 + n * 16;
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
        Placement::Pixels([shot.x - cx - 4.0, shot.y - cy - 1.2, 8.0, 2.4]),
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
            let length = (reach * 1.6).clamp(10.0, 34.0);
            layers.push(streak(id + 1, length, 5.0, [r, g, b, 0.5]));
            layers.push(streak(id + 6, length * 0.55, 2.4, [0.5 + 0.5 * r, 0.5 + 0.5 * g, 0.5 + 0.5 * b, 0.6]));
        }
        let halo = if shot.hostile { 9.0 } else { 7.5 };
        let mut layer = glow(id + 2, (x, y), halo, [r, g, b, 0.5], true);
        layer.flicker = 0.12;
        layers.push(layer);
        if shot.empowered {
            layers.push(glow(id + 3, (x, y), 11.0, [1.0, 0.82, 0.35, 0.4], true));
        }
    } else if speed > 0.0 {
        // A speed line where it just was: a thin streak of air along its
        // flight, brightest at the shot.
        let length = (reach * 1.4).clamp(8.0, 26.0);
        layers.push(streak(id + 1, length, 2.2, [0.92, 0.90, 0.82, 0.6]));
        if shot.empowered {
            layers.push(glow(id + 3, (x, y), 9.0, [1.0, 0.82, 0.35, 0.35], true));
        }
    }
    if let Some(rect) = look {
        // The sprite at twice its size, centred on the shot, as a terminal
        // paints it.
        let mut sprite = Layer::new(
            id + 4,
            3,
            Placement::Pixels([x - 24.0, y - 24.0, rect[2] * 2.0, rect[3] * 2.0]),
            LayerKind::PicturePart(SHOTS, rect),
        );
        sprite.stand = foot;
        layers.push(sprite);
    }
    if shot.bright && flat {
        let core = [0.5 + 0.5 * r, 0.5 + 0.5 * g, 0.5 + 0.5 * b, 0.95];
        layers.push(glow(id + 5, (x, y), 2.8, core, true));
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
            card: "gold".into(), x: 20.0, y: 14.0, held_off: None, took: Vec::new(),
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
            card: "gold".into(), x: 20.0, y: 14.0, held_off: None, took: Vec::new(),
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
            let item = crate::world3d::assets::item("village").unwrap();
            scene.world = Some(crate::world3d::room::RoomWorld::of(item, (NATIVE_W, NATIVE_H)));
        }
        scene
    }

    /// The kingdom with the world in 3D, two knights out in Castle Town's
    /// Village, the plates near decoded.
    fn kingdom_3d() -> Arc<RoomScene> {
        super::super::props_meshes(true);
        crate::world3d::rooms_lit::set_plates(true);
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
        assert!(scene.chunks.iter().all(|c| !c.painted()), "plates are retired for the kingdom");
        let mut maker = StageMaker::default();
        let first = maker.make(&scene, |_| None);
        let plates = |frame: &Frame| frame.pictures.iter().filter(|p| (WORLD_CHUNK_PLATES..WORLD_CHUNK_SWATCHES + 0x2_0000).contains(&p.id)).count();
        assert_eq!(plates(&first), 0, "no plates, no patches");
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
        // The page fills it lit, as the window has it.
        let mut sets = angel_stage::set::Sets::default();
        let mut made = world.clone();
        assert!(sets.fill(&mut made));
        let lit: usize = shown.iter().map(|c| c.set.shapes.len() + 1).sum();
        let stood: usize = scene.lots.iter().flat_map(|l| &l.shares).filter(|s| world.more.iter().any(|r| r.key == s.key)).map(|s| s.extras.len()).sum();
        assert_eq!(made.floor.len() + made.blocks.len(), lit + stood);
        assert_eq!(made.meshes.len(), shown.len(), "a ground a chunk");
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
        // The plate goes, the still art does not; the figures are each a picture, not a sheet.
        let plate_id = plate_picture(scene.world.clone().unwrap().item().unwrap());
        assert!((WORLD_PLATE..WORLD_CARDS).contains(&plate_id));
        assert!(ids.contains(&plate_id) && !ids.contains(&GROUND) && !ids.contains(&FIGURES), "{ids:x?}");
        let plate = frame.pictures.iter().find(|p| p.id == plate_id).unwrap();
        assert_eq!((plate.width, plate.height), (1536, 1024));
        assert_eq!(ids.iter().filter(|&&id| (WORLD_CARDS..WORLD_SWATCHES).contains(&id)).count(), scene.figures.len());
        assert!(!frame.layers.iter().any(|l| matches!(l.kind, LayerKind::Picture(GROUND) | LayerKind::PicturePart(FIGURES, _))));
        let props = frame.props.as_ref().expect("the world rides in the props");
        let world = props.world.as_ref().expect("the room in 3D");
        assert!(!world.floor.is_empty() && !world.blocks.is_empty());
        assert_eq!(world.figures.len(), scene.figures.len(), "a card a figure");
        let set = world.set.as_ref().expect("its set by reference, for a page that makes the blocks itself");
        assert!(set.text.is_some(), "with its text in the room's first frame");
        assert!(set.png.is_none(), "a window that draws it here has the plate's pixels");
        assert_eq!(set.plate, plate_id);
        // The world lies where the still art would, the figures where theirs would: over what is under them.
        assert_eq!(world.at, 0);
        let under = frame.layers[..world.figures_at].iter().position(|l| matches!(l.kind, LayerKind::Picture(UNDER)));
        assert!(under.is_some(), "what is painted under the figures lies under them");
        assert!(frame.layers[world.figures_at..].iter().any(|l| matches!(l.kind, LayerKind::Picture(OVER))));
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
        // The plate goes once, and a figure's picture again only when it changed.
        let again = maker.make(&scene, |_| None);
        let set = again.props.as_ref().and_then(|p| p.world.as_ref()).and_then(|w| w.set.as_ref()).unwrap();
        assert!(set.text.is_none(), "the set's text goes once, with its plate");
        assert!(again.pictures.iter().all(|p| p.id < WORLD_PLATE), "{:x?}", again.pictures.iter().map(|p| p.id).collect::<Vec<_>>());
        maker.forget();
        assert!(maker.make(&scene, |_| None).pictures.iter().any(|p| p.id == plate_id), "all again");
    }

    #[test]
    fn a_drawer_across_a_wire_is_sent_each_plate_as_its_png_once() {
        let scene = Arc::new(village(true));
        let item = Arc::clone(scene.world.clone().unwrap().item().unwrap());
        let mut maker = StageMaker { png_plates: true, ..StageMaker::default() };
        let set_of = |frame: &Frame| frame.props.as_ref().and_then(|p| p.world.as_ref()).and_then(|w| w.set.clone()).unwrap();
        let first = maker.make(&scene, |_| None);
        assert!(first.pictures.iter().all(|p| !(WORLD_PLATE..WORLD_CARDS).contains(&p.id)), "never its pixels");
        let set = set_of(&first);
        assert_eq!(set.png.as_deref(), Some(&item.png[..]), "the PNG as read");
        assert_eq!(set.plate, plate_picture(&item));
        assert!(set_of(&maker.make(&scene, |_| None)).png.is_none(), "once");
        // Out of the room and back: not the plate (the page keeps it by item).
        let _ = maker.make(&Arc::new(village(false)), |_| None);
        assert!(set_of(&maker.make(&scene, |_| None)).png.is_none());
        // All again: the plate too.
        maker.forget();
        assert!(set_of(&maker.make(&scene, |_| None)).png.is_some());
    }

    #[test]
    fn a_page_that_draws_no_scenes_keeps_the_rooms_still_art() {
        let frame = StageMaker::flat().make(&Arc::new(village(true)), |_| None);
        assert!(frame.props.is_none());
        assert!(frame.layers.iter().any(|l| matches!(l.kind, LayerKind::Picture(GROUND))));
        assert!(frame.layers.iter().any(|l| matches!(l.kind, LayerKind::PicturePart(FIGURES, _))));
        assert!(!frame.pictures.iter().any(|p| (WORLD_PLATE..WORLD_CARDS).contains(&p.id)));
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
        assert!(ids(&first).contains(&GROUND));
        assert!(!ids(&second).contains(&GROUND), "the room art once");
        assert!(ids(&second).is_empty(), "nothing changed, nothing sent");
        maker.forget();
        let third = maker.make(&scene, |_| None);
        assert!(ids(&third).contains(&GROUND) && ids(&third).contains(&SCREEN), "all again");
        assert_eq!(first.size, (NATIVE_W as u32, NATIVE_H as u32));
        assert!(first.layers.iter().any(|l| l.additive), "its fires' light");
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
        let drawn = |s: &SceneShot| s.mark.words().iter().any(|&w| w >> 24 != 0);
        let mut looks: Vec<&Img> = Vec::new();
        for shot in base.shots.iter().filter(|s| drawn(s)) {
            if !looks.contains(&&shot.mark) {
                looks.push(&shot.mark);
            }
        }
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
        let (img, drawings) = with_hd(figure, &mut |b: &Body| (b.key == body.key).then_some(77));
        assert_eq!(drawings[0].0, 77);
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

    #[test]
    fn a_room_laid_otherwise_than_its_item_sends_the_difference_as_extra_blocks() {
        let proof = crate::world3d::room::proof_variant("cavern-boulder-rill").expect("the Boulder Rill, walled and rolled");
        crate::world3d::rooms_lit::set_plates(true);
        crate::world3d::set_enabled(true);
        let scene = Arc::new(scene_for(&proof.run, NATIVE_W, NATIVE_H, Some(1)));
        crate::world3d::set_enabled(false);
        let world = scene.world.clone().expect("still its item");
        assert!(world.variant().is_some(), "laid otherwise");
        let item = Arc::clone(world.item().unwrap());
        assert!(!item.swatches().is_empty(), "the item's patches (swatches.png) are in the folder");
        let swatches: Vec<u32> = item.swatches().iter().map(|s| swatch_picture(&item, s.kind)).collect();
        // A window: the plate and the patches as pictures, the extra blocks with the blocks.
        let frame = StageMaker::default().make(&scene, |_| None);
        let ids: Vec<u32> = frame.pictures.iter().map(|p| p.id).collect();
        assert!(ids.contains(&plate_picture(&item)) && swatches.iter().all(|s| ids.contains(s)));
        let sent = frame.props.as_ref().and_then(|p| p.world.as_ref()).expect("the world");
        assert!(!sent.extra.is_empty(), "the walled doorway and the boulder");
        let named = |n: &Node| match n.material.source {
            angel_stage::scene::TextureSource::Picture(id) => Some(id),
            _ => None,
        };
        assert!(sent.extra.iter().filter_map(named).all(|id| swatches.contains(&id)), "each of a patch");
        // A page across a wire: the patches as pictures with the plate's PNG,
        // the extra blocks as nodes, read back as they went.
        let frame = StageMaker::for_wire().make(&scene, |_| None);
        assert!(frame.props.is_none() || frame.props.as_ref().unwrap().world.is_none(), "a flat page draws no world");
        let mut maker = StageMaker::for_wire();
        maker.flat_shots = false;
        let frame = maker.make(&scene, |_| None);
        let ids: Vec<u32> = frame.pictures.iter().map(|p| p.id).collect();
        assert!(swatches.iter().all(|s| ids.contains(s)), "the patches go once, with the plate");
        let sent = frame.props.as_ref().and_then(|p| p.world.clone()).unwrap();
        let mut out = Vec::new();
        angel_stage::wire::encode(&frame, &[], &mut out);
        let back = angel_stage::wire::decode(&out).expect("it reads").frame.props.unwrap().world.unwrap();
        assert_eq!(back.extra, sent.extra);
        assert_eq!(back.set.as_ref().map(|s| s.key), Some(world.key()));
    }
}
