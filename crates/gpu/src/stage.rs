//! Stages: a game's scene, drawn by the window into its own texture every
//! frame at the window's density, and shown wherever its picture id is
//! placed, as any picture is (docs/RENDER_FOUNDATION.md, "The game
//! pipeline"). Menus over the game, the assembly transition and resizing
//! work on it unchanged, because the cells still show it.
//!
//! A stage is layers in the game's own pixels: the room's still art scrolled
//! by the camera, what moves, the screen's marks. Their pictures go up with
//! `Host::upload_picture`; the still art only when it changes.

use angel_stage::scene::{Camera, Node, Props, Scene, stage_camera};
use angel_stage::{Layer, LayerKind, Placement, Stand, Tilt};
use bytemuck::Zeroable;

use crate::Core;
use crate::draw::bind;
use crate::scene::DepthUse;
use crate::frame::{Draw, Frame, Instance, PICTURE};
use crate::picture::GpuPicture;

/// A scene in its own pixels, drawn into picture `id`'s cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    /// The picture id its cells name.
    pub id: u32,
    /// The canvas, in the stage's own pixels (a game's native picture).
    pub size: (u32, u32),
    /// The placement box, in cells.
    pub cols: u16,
    pub rows: u16,
    /// Back to front, placed in the stage's pixels.
    pub layers: Vec<Layer>,
    /// Which scene this is (a room). When it changes, the new one slides in
    /// over the last from `enter`: `[1, 0]` from the right, `[0, -1]` from
    /// above; with no `enter` it simply replaces it.
    pub scene: u64,
    pub enter: Option<[f32; 2]>,
    /// How far the floor tips away from the camera (`angel_stage::Frame::tilt`).
    pub tilt: f32,
    /// 3D things between the layers (`angel_stage::scene::Props`): seen by
    /// the camera the tilt gives, drawn into a picture of their own at the
    /// props' grain and laid over `props.at` layers.
    pub props: Option<Props>,
    /// How hard the world was just hit, 0 to 1 (`angel_stage::Frame::
    /// impact`): the post pass splits its colours and lifts its light by it.
    pub impact: f32,
}

/// The longest side a stage texture may take: the smallest
/// `max_texture_dimension_2d` any wgpu backend guarantees (WebGPU's
/// floor), so a stage sized here is valid everywhere the compositor runs.
pub const MAX_TARGET_PX: u32 = 8192;

/// The texture a stage takes in its box: the canvas fit inside with its
/// aspect kept, in whole window pixels, as a picture of that size would be
/// placed (`graphics::cell_slice`), so the cells show it one to one. An
/// empty canvas, box or cell has no texture at all: `(0, 0)`, and the
/// caller draws nothing. The result never exceeds [`MAX_TARGET_PX`] on a
/// side, however large a box a producer names.
pub fn target_size(stage: &Stage, cell: (u32, u32)) -> (u32, u32) {
    if stage.size.0 == 0
        || stage.size.1 == 0
        || stage.cols == 0
        || stage.rows == 0
        || cell.0 == 0
        || cell.1 == 0
    {
        return (0, 0);
    }
    let (iw, ih) = (stage.size.0 as f32, stage.size.1 as f32);
    let bw = f32::from(stage.cols) * cell.0 as f32;
    let bh = f32::from(stage.rows) * cell.1 as f32;
    let fit = (bw / iw).min(bh / ih);
    let (mut w, mut h) = (
        ((iw * fit).round() as u32).max(1),
        ((ih * fit).round() as u32).max(1),
    );
    let longest = w.max(h);
    if longest > MAX_TARGET_PX {
        let scale = MAX_TARGET_PX as f32 / longest as f32;
        w = ((w as f32 * scale).floor() as u32).max(1);
        h = ((h as f32 * scale).floor() as u32).max(1);
    }
    (w, h)
}

/// The stage's layers in texture pixels: every placement, height and breath
/// scaled from the stage's pixels to the texture's.
pub fn scaled_layers(stage: &Stage, target: (u32, u32)) -> Vec<Layer> {
    let sx = target.0 as f32 / stage.size.0.max(1) as f32;
    let sy = target.1 as f32 / stage.size.1.max(1) as f32;
    stage
        .layers
        .iter()
        .map(|layer| {
            let [x, y, w, h] = match layer.placement {
                Placement::Pixels(rect) | Placement::Cells(rect) => rect,
            };
            let mut out = layer.clone();
            out.placement = Placement::Pixels([x * sx, y * sy, w * sx, h * sy]);
            if let Stand::Upright([fx, fy]) = layer.stand {
                out.stand = Stand::Upright([fx * sx, fy * sy]);
            }
            out.height = layer.height * sy;
            out.breathe = layer.breathe * sy;
            if let LayerKind::Particles(p) = &mut out.kind {
                p.size *= sy;
                p.rise *= sy;
                p.sway *= sy;
            }
            out
        })
        .collect()
}

/// A stage's own textures beside its picture: the last scene, kept while
/// the next slides in, and the next one drawn on its own.
const STAGE_PREVIOUS: u32 = 0x4000_0000;
const STAGE_NEXT: u32 = 0x2000_0000;
/// And the picture its props are drawn into.
const STAGE_PROPS: u32 = 0x1000_0000;
/// And a world's (`Props::world`): its floor and blocks, and its figures.
const STAGE_WORLD: u32 = 0x0800_0000;
const STAGE_FIGURES: u32 = 0x0400_0000;
/// And a lit world's plain drawing, made only for its depth.
const STAGE_WORLD_DEPTH: u32 = 0x0200_0000;

/// How long a new scene takes to slide in.
const SLIDE_SECONDS: f32 = 0.42;

fn ease_in_out(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    if k < 0.5 {
        4.0 * k * k * k
    } else {
        1.0 - (-2.0 * k + 2.0).powi(3) / 2.0
    }
}

impl Core {
    /// Show a stage in the cells naming `id` (`None` takes it down).
    pub fn set_stage(&mut self, id: u32, stage: Option<Stage>) {
        match stage {
            Some(stage) => {
                self.stages.insert(id, stage);
            }
            None => {
                self.stages.remove(&id);
                // The last stage gone, the post pass's textures go with it.
                if self.stages.is_empty() && let Some(post) = self.post.as_mut() {
                    post.release();
                }
                self.stage_scenes.remove(&id);
                self.slides.remove(&id);
                for picture in [id, id ^ STAGE_PREVIOUS, id ^ STAGE_NEXT, id ^ STAGE_PROPS, id ^ STAGE_WORLD, id ^ STAGE_FIGURES, id ^ STAGE_WORLD_DEPTH] {
                    if self.pictures.get(&picture).is_some_and(|p| p.target) {
                        self.pictures.remove(&picture);
                    }
                }
            }
        }
    }

    /// The texture stage `id` draws into, `size` pixels, made when missing
    /// or when the size changed.
    fn stage_target(&mut self, id: u32, size: (u32, u32), cols: u16, rows: u16) {
        self.target_texture(id, size, cols, rows, false);
    }

    /// The texture a renderer draws into for picture `id` (a stage's, or a
    /// scene's), `size` pixels, made when missing or when the size changed.
    /// `premultiplied` says how a layer must read it: a stage is opaque, a
    /// scene may not be.
    pub(crate) fn target_texture(
        &mut self,
        id: u32,
        size: (u32, u32),
        cols: u16,
        rows: u16,
        premultiplied: bool,
    ) {
        if let Some(existing) = self.pictures.get_mut(&id)
            && existing.target
            && (existing.width, existing.height) == size
        {
            existing.cols = cols;
            existing.rows = rows;
            existing.premultiplied = premultiplied;
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-stage"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.view_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let gpu = GpuPicture {
            nearest: bind(&self.device, &self.bind_layout, &self.globals, &view, &self.nearest),
            linear: bind(&self.device, &self.bind_layout, &self.globals, &view, &self.linear),
            texture,
            view,
            width: size.0,
            height: size.1,
            cols,
            rows,
            bytes: 0,
            last_used: self.frame_number,
            target: true,
            premultiplied,
            straight: false,
            mesh_groups: [None, None],
        };
        if let Some(old) = self.pictures.insert(id, gpu) {
            self.picture_bytes -= old.bytes;
        }
    }

    /// Draw every stage into its texture, each in its own submission (the
    /// globals hold its size), before the frame that shows them. `cell` is
    /// the host's cell size in pixels, which a stage's box is measured in;
    /// `now` is the host's clock in seconds (`Core::layer_parts`).
    pub fn render_stages(&mut self, cell: (u32, u32), now: f64) {
        if self.stages.is_empty() {
            return;
        }
        let mut pool = std::mem::take(&mut self.spare_vecs);
        // Ids first, then each stage borrowed just long enough to scale
        // its layers out: no stage (and none of its layers) is cloned.
        let ids: Vec<u32> = self.stages.keys().copied().collect();
        for id in ids {
            let Some(stage) = self.stages.get(&id) else {
                continue;
            };
            let size = target_size(stage, cell);
            if size.0 == 0 || size.1 == 0 {
                continue;
            }
            let (cols, rows) = (stage.cols, stage.rows);
            let scene = stage.scene;
            let enter = stage.enter;
            let stage_tilt = stage.tilt;
            let stage_size = stage.size;
            let layers = scaled_layers(stage, size);
            self.stage_target(id, size, cols, rows);
            let tilt = Tilt::for_stage(stage_tilt, size);
            let own = |l: &&Layer| {
                !matches!(l.kind, LayerKind::Picture(p) | LayerKind::PicturePart(p, _) if p == id)
            };
            // The props (and a world, if there is one), drawn into pictures
            // of their own, which then lie among the layers like any picture
            // does.
            let laid = self.draw_props(id, stage_size, stage_tilt, size);
            // Post-processed, the screen's own marks (the trailing layers
            // that stand on the screen: the HUD, the bands) go over the
            // world after the post pass, untouched by it.
            let post = self.post_on;
            let split = if post {
                layers.len() - layers.iter().rev().take_while(|l| matches!(l.stand, Stand::Screen)).count()
            } else {
                layers.len()
            };
            let mut parts = Vec::new();
            let mut screen_parts = Vec::new();
            let mut from = 0;
            for (at, layer) in &laid {
                let at = (*at).min(layers.len()).max(from);
                parts.extend(self.layer_parts(layers[from..at.min(split).max(from)].iter().filter(own), (0.0, 0.0), 1.0, 1.0, tilt, now));
                screen_parts.extend(self.layer_parts(layers[from.max(split).min(at)..at].iter().filter(own), (0.0, 0.0), 1.0, 1.0, tilt, now));
                let into = if at <= split { &mut parts } else { &mut screen_parts };
                into.extend(self.layer_parts(std::iter::once(layer), (0.0, 0.0), 1.0, 1.0, tilt, now));
                from = at;
            }
            parts.extend(self.layer_parts(layers[from..split.max(from)].iter().filter(own), (0.0, 0.0), 1.0, 1.0, tilt, now));
            screen_parts.extend(self.layer_parts(layers[from.max(split)..].iter().filter(own), (0.0, 0.0), 1.0, 1.0, tilt, now));
            let mut frame = std::mem::take(&mut self.spare_frame);
            frame.assemble_into(&mut parts, &mut pool);
            let mut screen = std::mem::take(&mut self.spare_screen);
            screen.assemble_into(&mut screen_parts, &mut pool);
            let impact = self.stages.get(&id).map_or(0.0, |s| s.impact);
            let grid = size.0 as f32 / stage_size.0.max(1) as f32;
            // A new scene arriving from a side slides in over the last one.
            let (prev_id, next_id) = (id ^ STAGE_PREVIOUS, id ^ STAGE_NEXT);
            let last = self.stage_scenes.insert(id, scene);
            if let Some(enter) = enter
                && last.is_some_and(|prev| prev != scene)
            {
                self.stage_target(prev_id, size, 0, 0);
                let copied = self.copy_picture(id, prev_id);
                if copied {
                    self.slides.insert(id, (now, enter));
                }
            }
            let slide = self.slides.get(&id).copied().and_then(|(started, enter)| {
                let k = (now - started) as f32 / SLIDE_SECONDS;
                (k < 1.0 && self.pictures.get(&prev_id).is_some_and(|p| (p.width, p.height) == size))
                    .then_some((k, enter))
            });
            let Some((k, [ex, ey])) = slide else {
                self.slides.remove(&id);
                self.draw_stage_frame(id, size, &frame, post.then_some((&screen, grid, impact)));
                self.spare_frame = frame;
                self.spare_screen = screen;
                continue;
            };
            // The new scene into its own texture, then both into the stage's:
            // the old one leaving, the new one arriving behind it.
            self.stage_target(next_id, size, 0, 0);
            self.draw_stage_frame(next_id, size, &frame, post.then_some((&screen, grid, impact)));
            self.spare_screen = screen;
            let eased = ease_in_out(k);
            let (w, h) = (size.0 as f32, size.1 as f32);
            let at = |shift: f32| [ex * w * shift, ey * h * shift];
            let [ox, oy] = at(-eased);
            let [nx, ny] = at(1.0 - eased);
            let mut parts = Vec::new();
            for (id, (x, y)) in [(prev_id, (ox, oy)), (next_id, (nx, ny))] {
                parts.push((
                    Draw::Picture { id, linear: false },
                    vec![Instance {
                        rect: [x.round(), y.round(), w, h],
                        uv: [0.0, 0.0, w, h],
                        color: [1.0; 4],
                        kind: PICTURE,
                        pivot: [0.0; 2],
                        angle: 0.0,
                        ..Zeroable::zeroed()
                    }],
                ));
            }
            frame.assemble_into(&mut parts, &mut pool);
            self.draw_into(id, size, &frame);
            self.spare_frame = frame;
        }
        self.spare_vecs = pool;
    }

    /// Draw stage `id`'s props, if it has some, into their own picture, and
    /// say where it goes: the number of layers under it, and the layer that
    /// shows the picture over the stage's texture (`size`, in its own
    /// pixels). The stage keeps its nodes. A stage with none has no props
    /// picture either. Every picture laid comes back so, in order of where.
    ///
    /// With a world (`Props::world`), the world is drawn too, all at the
    /// texture's own size and sharing one depth buffer: its floor, then its
    /// blocks over it (kept in the depth), into one picture; its figures,
    /// tested against the blocks, into another; and the props, tested
    /// against the blocks, into the props' picture. Each is laid where it
    /// says, in order of where.
    fn draw_props(&mut self, id: u32, stage_size: (u32, u32), tilt: f32, size: (u32, u32)) -> Vec<(usize, Layer)> {
        let Some(props) = self.stages.get_mut(&id).and_then(|s| s.props.as_mut()) else {
            self.drop_targets(&[id ^ STAGE_PROPS, id ^ STAGE_WORLD, id ^ STAGE_FIGURES, id ^ STAGE_WORLD_DEPTH]);
            return Vec::new();
        };
        let (at, grain) = (props.at, props.grain_clamped());
        let nodes = std::mem::take(&mut props.nodes);
        let world = props.world.take();
        let camera = stage_camera(tilt, stage_size);
        let mut laid = Vec::new();
        let nodes = match world {
            None => {
                self.drop_targets(&[id ^ STAGE_WORLD, id ^ STAGE_FIGURES, id ^ STAGE_WORLD_DEPTH]);
                let fine = (
                    (stage_size.0.saturating_mul(grain)).clamp(1, MAX_TARGET_PX),
                    (stage_size.1.saturating_mul(grain)).clamp(1, MAX_TARGET_PX),
                );
                let (nodes, layer) = self.props_picture(id ^ STAGE_PROPS, fine, size, camera, nodes, DepthUse::Fresh);
                laid.extend(layer.map(|l| (at, l)));
                nodes
            }
            Some(mut world) => {
                // Its sets' grounds, uploaded the first time they are named.
                for (mesh_id, mesh) in &world.meshes {
                    if !self.has_mesh(*mesh_id) {
                        let _ = self.upload_mesh(*mesh_id, mesh);
                    }
                }
                // The floor, then the blocks over it, their depth kept for
                // what stands among them.
                let picture = id ^ STAGE_WORLD;
                let depth = self.depth_view(size);
                let standing = !world.blocks.is_empty() || !world.extra.is_empty();
                let lit = world.look.as_ref().filter(|l| l.lights()).cloned();
                if world.floor.is_empty() && !standing {
                    self.drop_targets(&[picture, id ^ STAGE_WORLD_DEPTH]);
                } else if let Some(mut look) = lit {
                    // Through its look (docs/REALM-MONOLITH.md): the floor and
                    // blocks lit and put on their places' ramps at the stage's
                    // own pixel grain, enlarged into the picture; their depth
                    // at the picture's size, what the look's reveal opens
                    // left out, for what stands among them.
                    look.pixel_rows = stage_size.1.max(16);
                    self.drop_targets(&[id ^ STAGE_WORLD_DEPTH]);
                    self.target_texture(picture, size, 0, 0, true);
                    if let Some(view) = self.pictures.get(&picture).map(|p| p.view.clone()) {
                        let (floors, blocks) = (world.floor.len(), world.blocks.len());
                        let mut nodes = std::mem::take(&mut world.floor);
                        nodes.append(&mut world.blocks);
                        nodes.append(&mut world.extra);
                        let scene = Scene { camera, nodes, look: Some(look) };
                        self.draw_scene_with_depth(&view, size, &scene, "glass-world-lit", Some(picture), DepthUse::Revealed(&depth));
                        let mut nodes = scene.nodes;
                        world.extra = nodes.split_off(floors + blocks);
                        world.blocks = nodes.split_off(floors);
                        world.floor = nodes;
                    }
                    laid.push((world.at, screen_layer(picture, size)));
                } else {
                    self.drop_targets(&[id ^ STAGE_WORLD_DEPTH]);
                    self.target_texture(picture, size, 0, 0, true);
                    if let Some(view) = self.pictures.get(&picture).map(|p| p.view.clone()) {
                        let floor = Scene { camera, nodes: std::mem::take(&mut world.floor), look: None };
                        self.draw_scene_with_depth(&view, size, &floor, "glass-world-floor", Some(picture), DepthUse::Fresh);
                        world.floor = floor.nodes;
                        // The set's blocks, and those it has not with them.
                        let mut nodes = std::mem::take(&mut world.blocks);
                        let set_blocks = nodes.len();
                        nodes.append(&mut world.extra);
                        let mut blocks = Scene { camera: Camera { clear: None, ..camera }, nodes, look: None };
                        self.draw_scene_with_depth(&view, size, &blocks, "glass-world-blocks", Some(picture), DepthUse::Keep(&depth));
                        world.extra = blocks.nodes.split_off(set_blocks);
                        world.blocks = blocks.nodes;
                    }
                    laid.push((world.at, screen_layer(picture, size)));
                }
                // An empty depth for what stands when there is nothing to
                // stand behind.
                let stand = if standing { DepthUse::Continue(&depth) } else { DepthUse::Fresh };
                let figures = std::mem::take(&mut world.figures);
                let (figures, layer) = self.props_picture(id ^ STAGE_FIGURES, size, size, camera, figures, stand);
                world.figures = figures;
                laid.extend(layer.map(|l| (world.figures_at, l)));
                let (nodes, layer) = self.props_picture(id ^ STAGE_PROPS, size, size, camera, nodes, stand);
                laid.extend(layer.map(|l| (at, l)));
                if let Some(props) = self.stages.get_mut(&id).and_then(|s| s.props.as_mut()) {
                    props.world = Some(world);
                }
                nodes
            }
        };
        if let Some(props) = self.stages.get_mut(&id).and_then(|s| s.props.as_mut()) {
            props.nodes = nodes;
        }
        // In order of where they go; one at the same place as another after it.
        laid.sort_by_key(|(at, _)| *at);
        laid
    }

    /// `nodes` drawn into picture `picture`, `fine` pixels, through `camera`
    /// cleared to nothing, with depth as `depth` says; and the layer that
    /// shows it over the stage's texture (`size`). The nodes come back. No
    /// nodes, no picture.
    fn props_picture(
        &mut self,
        picture: u32,
        fine: (u32, u32),
        size: (u32, u32),
        camera: Camera,
        nodes: Vec<Node>,
        depth: DepthUse<'_>,
    ) -> (Vec<Node>, Option<Layer>) {
        if nodes.is_empty() {
            self.drop_targets(&[picture]);
            return (nodes, None);
        }
        let scene = Scene { camera, nodes, look: None };
        self.target_texture(picture, fine, 0, 0, true);
        if let Some(view) = self.pictures.get(&picture).map(|p| p.view.clone()) {
            self.draw_scene_with_depth(&view, fine, &scene, "glass-props", Some(picture), depth);
        }
        (scene.nodes, Some(screen_layer(picture, size)))
    }

    /// Take down pictures a stage drew into and no longer does.
    fn drop_targets(&mut self, pictures: &[u32]) {
        for picture in pictures {
            if self.pictures.get(picture).is_some_and(|p| p.target) {
                self.pictures.remove(picture);
            }
        }
    }

    /// Draw a stage's frame into picture `id`'s texture, `size` pixels:
    /// straight (`draw_into`), or with the post pass, when it says so, its
    /// world and its lights into the pass's own textures, the pass into
    /// `id`'s (`grid` of its pixels to a stage pixel) at `impact`, and the
    /// screen's marks over that.
    fn draw_stage_frame(&mut self, id: u32, size: (u32, u32), frame: &Frame, post: Option<(&Frame, f32, f32)>) {
        let Some((screen, grid, impact)) = post else {
            self.draw_into(id, size, frame);
            return;
        };
        let Some(view) = self.pictures.get(&id).map(|p| p.view.clone()) else {
            return;
        };
        self.draw_post(&view, size, frame, screen, grid, impact);
    }

    /// Draw `frame` into picture `id`'s texture (a stage's), `size`
    /// pixels, on transparency — a stage composites over whatever
    /// shows it, and a producer that wants paper paints it (the
    /// cockpit blacks the Delve's cells) — in a submission of its own
    /// (the globals hold its size).
    fn draw_into(&mut self, id: u32, size: (u32, u32), frame: &Frame) {
        let Some(target) = self.pictures.get(&id) else {
            return;
        };
        let view = target.view.clone();
        // Transparency under everything: the stage's own layers decide
        // what is opaque.
        self.draw(&view, size, wgpu::Color::TRANSPARENT, frame, "glass-stage");
    }
}

/// Picture `picture` over a whole stage texture of `size`, untouched by the
/// tilt (the camera it was drawn through has done it).
fn screen_layer(picture: u32, size: (u32, u32)) -> Layer {
    let mut layer = Layer::new(
        u64::from(picture),
        0,
        Placement::Pixels([0.0, 0.0, size.0 as f32, size.1 as f32]),
        LayerKind::Picture(picture),
    );
    layer.stand = Stand::Screen;
    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(cols: u16, rows: u16) -> Stage {
        Stage {
            id: 1,
            size: (384, 224),
            cols,
            rows,
            scene: 0,
            enter: None,
            layers: vec![Layer::new(
                0,
                0,
                Placement::Pixels([-10.5, 0.0, 768.0, 448.0]),
                LayerKind::Picture(7),
            )],
            tilt: 0.0,
            props: None,
            impact: 0.0,
        }
    }

    #[test]
    fn the_texture_fills_its_box_keeping_the_aspect() {
        // 120 × 35 cells of 12 × 25: 1440 × 875, so the width binds.
        assert_eq!(target_size(&stage(120, 35), (12, 25)), (1440, 840));
        // A tall box: the height binds.
        assert_eq!(target_size(&stage(120, 20), (12, 25)), (857, 500));
    }

    #[test]
    fn an_empty_stage_has_no_texture() {
        assert_eq!(target_size(&stage(0, 35), (12, 25)), (0, 0));
        assert_eq!(target_size(&stage(120, 0), (12, 25)), (0, 0));
        assert_eq!(target_size(&stage(120, 35), (0, 25)), (0, 0));
        let mut no_canvas = stage(120, 35);
        no_canvas.size = (0, 224);
        assert_eq!(target_size(&no_canvas, (12, 25)), (0, 0));
    }

    #[test]
    fn a_huge_box_stays_inside_the_texture_limit() {
        let (w, h) = target_size(&stage(u16::MAX, u16::MAX), (12, 25));
        assert!(w <= MAX_TARGET_PX && h <= MAX_TARGET_PX, "{w}x{h}");
        // The aspect survives the clamp (384:224, within a pixel's rounding).
        assert!((w as f32 / h as f32 - 384.0 / 224.0).abs() < 0.01);
    }

    #[test]
    fn layers_scale_to_the_texture() {
        let s = stage(120, 35);
        let layers = scaled_layers(&s, (1440, 840));
        let Placement::Pixels([x, y, w, h]) = layers[0].placement else {
            panic!("pixels");
        };
        // 3.75 texture pixels to a stage pixel, the camera's fraction kept.
        assert!((x + 39.375).abs() < 1e-3 && y == 0.0);
        assert!((w - 2880.0).abs() < 1e-3 && (h - 1680.0).abs() < 1e-3);
    }
}
