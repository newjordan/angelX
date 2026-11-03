//! glass's own dialect, the expansion side: a program speaks APC
//! `glass;` sequences and reaches what the window itself can draw —
//! layers of every kind, groups replaced per frame, particles, lit
//! primitives and the showcase — more than any picture dialect can say.
//! Other terminals ignore unknown APC strings, so a program may send
//! these blind; `hello` lets it ask first.
//!
//! One framing, everywhere: `ESC _ glass ; <op> [; <key>=<value>]... [; <payload>] ESC \`
//! (BEL also ends it). Fields sit behind `;`, so a value may hold
//! commas (`at=2,1,4,2`); the payload, when the op takes one, is the
//! last field and is base64. Chunked transmissions follow the intake's
//! own rule — `m=1` on every piece but the last, the first piece's keys
//! governing — and a payload-less continuation writes its last field
//! empty (`picture;m=0;`).
//!
//! The dialect reuses the intake's machinery rather than repeating it:
//! picture uploads decode through the APC graphics adapter's loader
//! (the same caps and budget), and its state lives in [`Expansion`],
//! which the store owns beside the intake's pictures and the session
//! renders each frame beside them.

use std::sync::Arc;

use super::apc_graphics;
use super::glass_gltf::{self, ClipSel, Instance, Playback};
use super::glass_tween::{self, Ease, Prop, Repeat, Target, Timing, Track, Tween, Value};
use super::{base64, Canvas};

/// The dialect's version. v2 adds `scene` and the showcase's video
/// frames, charts, models and pages; v3 adds glTF assets (`asset`, and
/// `play` for their clips) and `tween` (docs/GLASS_DIALECT.md).
pub const VERSION: u32 = 3;

/// The ops this build takes, in the order `hello` answers them.
pub const OPS: [&str; 15] = [
    "hello", "picture", "layer", "group", "remove", "showcase", "scene", "frame", "chart",
    "model", "page", "progress", "asset", "tween", "play",
];

/// The ops whose last field is a payload.
const PAYLOAD_OPS: [&str; 8] = ["picture", "group", "scene", "frame", "chart", "model", "page", "asset"];

/// Program ids live below this; a `picture` or `layer` id the program
/// names is offset by it into the expansion's own id space, far from
/// the intake's client ids (below `MINT_BASE`) and its minted ones.
pub const ID_BASE: u32 = 0x4000_0000;

/// The most layers one `group` may name — the stage's own limit, so a
/// group that fills a frame's whole layer set settles in one go.
pub const GROUP_CAP: usize = 65_536;

/// The most bytes a `group` payload may carry once decoded; layer
/// records are short, so this caps a hostile payload before its
/// records are even split.
pub const GROUP_BYTES: usize = 1 << 20;

/// The keys each op takes; anything else is refused, so a program
/// learns its typo instead of drawing half of what it meant.
fn allowed_keys(op: &str) -> &'static [&'static str] {
    match op {
        "hello" => &["r", "m"],
        "picture" => &["id", "f", "s", "v", "o", "m", "r"],
        "layer" => &[
            "id", "k", "z", "o", "a", "h", "b", "add", "fl", "stand", "up", "at", "px",
            "anchor", "c", "p", "src", "n", "size", "life", "rise", "sway", "seed", "burst",
            "shape", "speed", "phase", "lw", "glow", "r", "m",
        ],
        "group" => &["g", "n", "r", "m"],
        "remove" => &["id", "g", "all", "asset", "r", "m"],
        "showcase" => &["id", "close", "s", "v", "fit", "events", "r", "m"],
        "scene" => &["id", "at", "px", "anchor", "s", "v", "z", "o", "r"],
        "frame" => &["id", "f", "s", "v", "o", "m", "r"],
        "chart" => &["p", "n", "fit", "r"],
        "model" => &["s", "v", "fit", "r"],
        "page" => &["cols", "rows", "r"],
        "progress" => &["t", "pause", "r"],
        "mark" => &["x", "y", "w", "h", "ax", "ay", "t"],
        "asset" => &["id", "t", "m", "r"],
        "tween" => &[
            "layer", "id", "node", "camera", "at", "px", "o", "a", "c", "size", "b", "h", "rot",
            "scale", "color", "emissive", "visible", "eye", "target", "fov", "dur", "delay", "ease",
            "repeat", "yoyo", "tid", "after", "stop", "r",
        ],
        "play" => &["id", "node", "clip", "speed", "loop", "t", "pause", "stop", "r"],
        _ => &[],
    }
}

/// Why a sequence failed: a short code for the reply, a message for
/// the program's log.
struct Bad(String, String);

/// One op's keys, `;`-separated behind the op, each field one
/// `key=value` (a value may hold commas).
struct Keys {
    op: String,
    pairs: Vec<(String, String)>,
    /// The whole key text, held for a chunked transmission's last piece.
    raw: Vec<u8>,
}

impl Keys {
    /// `op;k=v;k=v...` — no payload; a group record line is exactly
    /// this shape.
    fn parse(text: &str) -> Self {
        let mut fields = text.split(';');
        let op = fields.next().unwrap_or("").to_string();
        let mut pairs = Vec::new();
        for field in fields {
            if let Some((key, value)) = field.split_once('=') {
                pairs.push((key.to_string(), value.to_string()));
            }
        }
        Self { op, pairs, raw: text.as_bytes().to_vec() }
    }

    /// The op and its key fields, with the payload already taken off.
    fn from_parts(op: &str, key_fields: &[&str]) -> Self {
        let mut pairs = Vec::new();
        for field in key_fields {
            if let Some((key, value)) = field.split_once('=') {
                pairs.push((key.to_string(), value.to_string()));
            }
        }
        let whole = [op].iter().chain(key_fields.iter()).copied().collect::<Vec<_>>().join(";");
        Self { op: op.to_string(), pairs, raw: whole.into_bytes() }
    }

    fn check(&self) -> Result<(), Bad> {
        let allowed = allowed_keys(&self.op);
        if let Some((key, _)) = self.pairs.iter().find(|(k, _)| !allowed.contains(&k.as_str())) {
            return Err(Bad("EINVAL".into(), format!("unknown key {key}")));
        }
        Ok(())
    }

    /// A key's value, the last one it was given.
    fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn f32(&self, key: &str) -> Result<Option<f32>, Bad> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite())
                .map(Some)
                .ok_or_else(|| Bad("EINVAL".into(), format!("{key} is not a finite number"))),
        }
    }

    fn u32(&self, key: &str) -> Result<Option<u32>, Bad> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .parse::<u32>()
                .map(Some)
                .map_err(|_| Bad("EINVAL".into(), format!("{key} is not a number"))),
        }
    }

    fn i32(&self, key: &str) -> Result<Option<i32>, Bad> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .parse::<i32>()
                .map(Some)
                .map_err(|_| Bad("EINVAL".into(), format!("{key} is not a number"))),
        }
    }

    /// `0` or `1`.
    fn flag(&self, key: &str) -> Result<Option<bool>, Bad> {
        match self.get(key) {
            None => Ok(None),
            Some("0") => Ok(Some(false)),
            Some("1") => Ok(Some(true)),
            Some(_) => Err(Bad("EINVAL".into(), format!("{key} is not 0 or 1"))),
        }
    }

    /// Four numbers, `x,y,w,h` — commas live inside the one value.
    fn quad(&self, key: &str) -> Result<Option<[f32; 4]>, Bad> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => list4(Some(v), key).map(Some),
        }
    }
}

/// `x,y,w,h`, fractions allowed.
fn list4(value: Option<&str>, key: &str) -> Result<[f32; 4], Bad> {
    let Some(v) = value else { return Ok([0.0; 4]) };
    let bad = || Bad("EINVAL".into(), format!("{key}= is not four finite numbers"));
    let mut parts = v.split(',');
    let mut out = [0.0f32; 4];
    for slot in &mut out {
        *slot = parts.next().unwrap_or("").parse::<f32>().ok().filter(|n| n.is_finite()).ok_or_else(bad)?;
    }
    if parts.next().is_some() {
        return Err(bad());
    }
    Ok(out)
}

/// Hex digits to bytes: 6 (rrggbb, opaque) or 8 (rrggbbaa).
fn hex(text: &str) -> Option<Vec<u8>> {
    if text.len() != 6 && text.len() != 8 {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// A colour, `c=rrggbb` or `c=rrggbbaa`, as straight RGBA in 0..1.
fn rgba_of(keys: &Keys) -> Result<[f32; 4], Bad> {
    let Some(v) = keys.get("c") else { return Ok([1.0, 1.0, 1.0, 1.0]) };
    let bytes =
        hex(v).ok_or_else(|| Bad("EINVAL".into(), "c= is not rrggbb or rrggbbaa".into()))?;
    let mut out = [1.0f32; 4];
    for (i, b) in bytes.iter().enumerate() {
        out[i] = f32::from(*b) / 255.0;
    }
    if bytes.len() == 3 {
        out[3] = 1.0;
    }
    Ok(out)
}

/// One picture this dialect uploaded, as the store keeps it.
struct Kept { id: u32 }

/// One layer the program asked for, as the frame draws it.
struct Wanted {
    /// The window id: the program's id offset by [`ID_BASE`].
    id: u32,
    z: i32,
    opacity: f32,
    angle: f32,
    height: f32,
    breathe: f32,
    additive: bool,
    flicker: f32,
    stand: angel_glass::Stand,
    placement: angel_glass::Placement,
    kind: angel_glass::LayerKind,
    /// Anchored to the text (`at=` without `anchor=window`): it scrolls
    /// with the text and leaves with it. `false` pins it to the window.
    text: bool,
    /// The absolute line it was anchored to, from the start of output.
    line: u64,
    /// Placed on the alternate screen (text-anchored only).
    alternate: bool,
    /// A text-anchored layer's rows below `line`, as a tween of its `at=`
    /// moves it; 0 otherwise.
    row_off: f32,
    /// The row its `at=` named, whole, as `line` took it.
    y0: f32,
}

/// The dialect's own state, owned by the store beside the intake's
/// pictures: the layers and groups the program asked for, and the
/// showcase it opened. Lifecycle operations clear it through the same
/// store calls that clear the intake's own pictures.
/// A showcase request. Frames and progress are not part of it, so a
/// stream does not reopen the panel.
#[derive(Clone, Debug, PartialEq)]
pub enum ShowcaseAsk {
    /// An uploaded picture, as version 1 opened the showcase.
    Picture { id: u32, fit: angel_glass::Fit },
    /// A video panel of this pixel size. Frames arrive separately.
    Video { width: u32, height: u32, fit: angel_glass::Fit, generation: u64 },
    /// Chart marks over a picture uploaded with `picture`. The picture's
    /// size is the chart's size. `generation` moves only when the request does.
    Chart {
        marks: Vec<angel_glass::Mark>,
        size: (u32, u32),
        fit: angel_glass::Fit,
        picture: u32,
        generation: u64,
    },
    /// A model, nodes from the scene wire.
    Model {
        nodes: Vec<angel_glass::scene::Node>,
        meshes: Vec<(u32, angel_glass::scene::MeshData)>,
        size: (u32, u32),
        fit: angel_glass::Fit,
        generation: u64,
    },
    /// Page text the window rasterises when the request changes.
    Page { text: String, cols: u16, rows: u16, generation: u64 },
}

/// One video frame waiting for the session to hand the window.
#[derive(Clone)]
pub enum PendingFrame {
    /// Raw or decoded pixels.
    Pixels { width: u32, height: u32, rgba: Vec<u8> },
    /// An already uploaded picture, by program id.
    Picture(u32),
}

/// A scene the session should draw, once it is dirty.
pub struct SceneDraw {
    pub window_id: u32,
    pub scene: angel_glass::scene::Scene,
    pub meshes: Vec<(u32, angel_glass::scene::MeshData)>,
    pub size: (u32, u32),
}

/// A placed scene: the stage scene, the picture it is drawn into, and
/// the layer that places that picture.
struct KeptScene {
    program_id: u32,
    window_id: u32,
    scene: angel_glass::scene::Scene,
    meshes: Vec<(u32, angel_glass::scene::MeshData)>,
    size: (u32, u32),
    wanted: Wanted,
    dirty: bool,
    /// Its inline meshes went up; a frame that only moves it does not
    /// send them again.
    meshes_sent: bool,
    /// The camera as the wire named it, for tweens.
    view: super::glass_scene::View,
    /// How many of `scene.nodes` the program named; an asset's nodes
    /// follow them.
    program_nodes: usize,
    /// The glTF assets hung from its nodes.
    instances: Vec<Instance>,
}

/// One glTF asset the program uploaded.
struct KeptAsset {
    id: u32,
    asset: Arc<glass_gltf::Asset>,
    /// The window ids of the pictures its materials name.
    textures: Vec<u32>,
}

/// Inline meshes the wire named, minted above the program's id space.
const MESH_BASE: u32 = 0xC000_0000;

/// Pictures an asset's materials name, minted between the program's
/// window ids and the meshes'.
const TEXTURE_BASE: u32 = 0x8000_0000;

/// Assets kept at once, and what their meshes may cost together on the
/// GPU: the window's own mesh budget, so every kept asset fits it.
pub const ASSET_COUNT: usize = 64;
pub const ASSET_BUDGET: usize = 128 << 20;

/// Nodes in one scene once its assets are hung in it.
pub const SCENE_EXPANDED_NODES: usize = 16_384;

/// The most marks one chart may name.
const CHART_CAP: usize = 4_096;

/// Page columns and rows, each side.
const PAGE_MAX: u32 = 256;

pub struct Expansion {
    /// Layers keyed by their window id, create or replace by `id`.
    layers: Vec<Wanted>,
    /// Group ids to their layers, replaced whole by `group`.
    groups: Vec<(u32, Vec<Wanted>)>,
    /// The pictures this dialect uploaded (kept through the store's own
    /// budget), so `showcase` can name one and `remove all` frees them.
    pictures: Vec<Kept>,
    /// The showcase request. Frames and progress are not part of it.
    ask: Option<ShowcaseAsk>,
    scenes: Vec<KeptScene>,
    pending_frame: Option<PendingFrame>,
    progress: Option<(Option<f32>, bool)>,
    progress_dirty: bool,
    events: bool,
    generation: u64,
    next_mesh: u32,
    assets: Vec<KeptAsset>,
    tweens: glass_tween::Tweens,
    /// The store's animation clock, milliseconds.
    clock: u64,
    /// One display frame, milliseconds: how often running motion wakes.
    frame_ms: u64,
    next_texture: u32,
    /// Asset meshes for the window, once per scene that newly shows them.
    uploads: Vec<(u32, Arc<angel_glass::scene::MeshData>)>,
}

impl Default for Expansion {
    fn default() -> Self {
        Self::new()
    }
}

impl Expansion {
    pub fn new() -> Self {
        Self { layers: Vec::new(), groups: Vec::new(), pictures: Vec::new(), ask: None, scenes: Vec::new(), pending_frame: None, progress: None, progress_dirty: false, events: false, generation: 0, next_mesh: MESH_BASE, assets: Vec::new(), tweens: glass_tween::Tweens::default(), clock: 0, frame_ms: 16, next_texture: TEXTURE_BASE, uploads: Vec::new() }
    }

    /// The frame's layers, positioned against the view: text-anchored
    /// layers move with the view's top line exactly as the intake's own
    /// showings do, and window-pinned layers never move.
    pub fn frame(&self, top: u64, rows: u16, alternate: bool) -> Vec<angel_glass::Layer> {
        let mut out = Vec::new();
        let push = |w: &Wanted, out: &mut Vec<angel_glass::Layer>| {
            let row = if w.text { w.line as f32 - top as f32 + w.row_off } else { 0.0 };
            let h = rect_h(&w.placement);
            if !w.text || (w.alternate == alternate && row < f32::from(rows) && row + h > 0.0) {
                // Only a text-anchored layer takes its row from the view; a
                // window-pinned one keeps the row it was placed at.
                let placement = if w.text { place_at(&w.placement, row) } else { w.placement.clone() };
                let mut layer = angel_glass::Layer::new(u64::from(w.id), w.z, placement, w.kind.clone());
                layer.opacity = w.opacity;
                layer.angle = w.angle;
                layer.height = w.height;
                layer.breathe = w.breathe;
                layer.additive = w.additive;
                layer.flicker = w.flicker;
                layer.stand = w.stand;
                out.push(layer);
            }
        };
        for w in &self.layers {
            push(w, &mut out);
        }
        for (_, ws) in &self.groups {
            for w in ws {
                push(w, &mut out);
            }
        }
        for scene in &self.scenes {
            push(&scene.wanted, &mut out);
        }
        out
    }

    /// The showcase's picture id, if one is open.
    #[cfg(test)]
    pub fn showcase(&self) -> Option<u32> {
        match &self.ask {
            Some(ShowcaseAsk::Picture { id, .. }) => Some(*id),
            _ => None,
        }
    }
    pub fn ask(&self) -> Option<&ShowcaseAsk> { self.ask.as_ref() }
    pub fn events_on(&self) -> bool { self.events }
    #[cfg(test)]
    pub fn progress(&self) -> Option<(Option<f32>, bool)> { self.progress }
    #[cfg(test)]
    pub fn pending_frame(&self) -> Option<&PendingFrame> { self.pending_frame.as_ref() }
    pub fn take_video_frame(&mut self) -> Option<PendingFrame> {
        if !matches!(self.ask, Some(ShowcaseAsk::Video { .. })) { return None; }
        self.pending_frame.take()
    }
    pub fn take_progress(&mut self) -> Option<(Option<f32>, bool)> {
        if !self.progress_dirty { return None; }
        self.progress_dirty = false;
        self.progress
    }
    #[cfg(test)]
    pub fn scene(&self, program_id: u32) -> Option<&angel_glass::scene::Scene> {
        self.scenes.iter().find(|s| s.program_id == program_id).map(|s| &s.scene)
    }
    #[cfg(test)]
    pub fn scene_size(&self, program_id: u32) -> Option<(u32, u32)> {
        self.scenes.iter().find(|s| s.program_id == program_id).map(|s| s.size)
    }
    pub fn scene_ids(&self) -> Vec<u32> { self.scenes.iter().map(|s| s.window_id).collect() }
    pub fn dirty_scenes(&mut self) -> Vec<SceneDraw> {
        let mut out = Vec::new();
        for scene in &mut self.scenes {
            if !scene.dirty { continue; }
            scene.dirty = false;
            let meshes = if scene.meshes_sent { Vec::new() } else { scene.meshes.clone() };
            scene.meshes_sent = true;
            out.push(SceneDraw { window_id: scene.window_id, scene: scene.scene.clone(), meshes, size: scene.size });
        }
        out
    }

    /// Asset meshes the window has not been handed yet.
    pub fn take_mesh_uploads(&mut self) -> Vec<(u32, Arc<angel_glass::scene::MeshData>)> {
        std::mem::take(&mut self.uploads)
    }

    /// Keep the store's clock, without moving anything.
    pub fn set_clock(&mut self, now: u64) {
        self.clock = self.clock.max(now);
    }

    /// One display frame, in milliseconds (4 to 100).
    pub fn set_frame_ms(&mut self, ms: u64) {
        self.frame_ms = ms.clamp(4, 100);
    }

    /// Advance every tween and every playing clip to `now`. Returns
    /// whether anything moved.
    pub fn tick(&mut self, now: u64) -> bool {
        self.clock = self.clock.max(now);
        let now = self.clock;
        let mut changed = false;
        if !self.tweens.is_empty() {
            let Expansion { tweens, layers, scenes, .. } = self;
            changed |= tweens.tick(now, &mut Bind { layers, scenes });
        }
        for scene in &mut self.scenes {
            for inst in &mut scene.instances {
                let Some(mut play) = inst.play else { continue };
                if play.since.is_none() {
                    play.since = Some(now);
                }
                let (t, _) = play.local(now, inst.asset.duration(play.clip));
                let fresh = play.shown != Some(t);
                play.shown = Some(t);
                inst.play = Some(play);
                if fresh {
                    inst.pose(&mut scene.scene.nodes, play.clip, t);
                    scene.dirty = true;
                    changed = true;
                }
            }
        }
        changed
    }

    /// When motion next needs a tick: at once for something new, at the
    /// start of a delayed tween, a display frame on while anything runs,
    /// `None` when all is still.
    pub fn deadline(&self) -> Option<u64> {
        let mut out = self.tweens.deadline(self.clock, self.frame_ms);
        let mut take = |at: u64| out = Some(out.map_or(at, |o| o.min(at)));
        for scene in &self.scenes {
            for inst in &scene.instances {
                let Some(play) = inst.play else { continue };
                if play.since.is_none() {
                    take(self.clock);
                } else if play.moving(self.clock, inst.asset.duration(play.clip)) {
                    take(self.clock.saturating_add(self.frame_ms));
                }
            }
        }
        out
    }
    #[cfg(test)]
    pub fn showcase_gen(&self) -> u64 { self.generation }
    #[cfg(test)]
    pub fn tween_count(&self) -> usize { self.tweens.len() }


    /// `ED 2` on a screen: text-anchored layers on that screen go.
    /// `scrolled` is the boundary the engine's own clear left (its
    /// lines are already in history above it), so the layers above it
    /// went with the screen and the scrollback's stay.
    pub fn clear_screen(&mut self, alternate: bool, scrolled: u64) {
        self.drop_where(|w| w.alternate == alternate && below(w, scrolled));
    }

    /// `ED 3`: the saved lines go, and the layers anchored to them.
    pub fn clear_saved(&mut self, scrolled: u64) {
        self.drop_where(|w| !w.alternate && below(w, scrolled));
    }

    /// Whether any layer or scene is anchored to the text, on either
    /// screen: only those move, crop or go with what text does.
    pub fn anchors_text(&self) -> bool {
        self.layers.iter().any(|w| w.text)
            || self.groups.iter().any(|(_, ws)| ws.iter().any(|w| w.text))
            || self.scenes.iter().any(|s| s.wanted.text)
    }

    /// The exclusive absolute line past the lowest text-anchored layer on
    /// this screen, if any. Window-pinned layers do not move with text.
    pub fn anchored_end(&self, alternate: bool) -> Option<u64> {
        let mut end = None;
        let mut note = |w: &Wanted| {
            if !w.text || w.alternate != alternate || w.line == u64::MAX {
                return;
            }
            let (_, height) = cells_size(w);
            let bottom = w.line.saturating_add((height + w.row_off.max(0.0)).max(1.0).ceil() as u64);
            end = Some(end.unwrap_or(0).max(bottom));
        };
        for w in &self.layers {
            note(w);
        }
        for (_, ws) in &self.groups {
            for w in ws {
                note(w);
            }
        }
        for scene in &self.scenes {
            note(&scene.wanted);
        }
        end
    }

    /// Text-anchored layers move and crop with a scroll region, through
    /// the same rectangle math the picture store uses.
    pub fn scroll_text(
        &mut self,
        band: &super::motion::BandScroll,
        image: impl Fn(u32) -> (u32, u32),
    ) {
        let mut born = Vec::new();
        self.map_text(|w| {
            if w.alternate != band.alternate {
                return;
            }
            if let Some(extra) = scroll_wanted(w, band, &image) {
                born.push(extra);
            }
        });
        self.layers.extend(born);
        self.drop_where(|w| w.line == u64::MAX);
    }

    /// Text-anchored layers stay on the same text through a reflow.
    pub fn reflow_text(&mut self, reflow: &super::motion::Reflow) {
        self.map_text(|w| {
            if !w.text || w.alternate {
                return;
            }
            let column = cells_col(w);
            if let Some((line, column)) = super::motion::map_anchor(reflow, w.line, column) {
                w.line = line;
                set_cells_col(w, column);
            } else {
                w.line = u64::MAX;
            }
        });
        self.drop_where(|w| w.line == u64::MAX);
    }

    fn map_text(&mut self, mut f: impl FnMut(&mut Wanted)) {
        for w in &mut self.layers {
            if w.text {
                f(w);
            }
        }
        for members in self.groups.iter_mut() {
            for w in members.1.iter_mut() {
                if w.text {
                    f(w);
                }
            }
        }
        for scene in &mut self.scenes {
            if scene.wanted.text {
                f(&mut scene.wanted);
            }
        }
    }

    /// Leaving the alternate screen: its text-anchored layers go, solo
    /// and grouped, whatever line they sit on.
    pub fn leave_alternate(&mut self) {
        self.drop_where(|w| w.alternate);
    }

    /// `RIS`, or the program's exit: everything this dialect made goes.
    /// The ids it minted keep counting, and the clock and frame stay.
    pub fn reset(&mut self) {
        let (next_mesh, next_texture, clock, frame_ms) = (self.next_mesh, self.next_texture, self.clock, self.frame_ms);
        *self = Self::new();
        self.next_mesh = next_mesh;
        self.next_texture = next_texture;
        self.clock = clock;
        self.frame_ms = frame_ms;
    }

    /// Drop the text-anchored layers `drop` names, solo and grouped;
    /// window-pinned layers are never dropped by a text rule.
    fn drop_where(&mut self, drop: impl Fn(&Wanted) -> bool) {
        self.layers.retain(|w| !w.text || !drop(w));
        for (_, ws) in &mut self.groups {
            ws.retain(|w| !w.text || !drop(w));
        }
        self.scenes.retain(|s| !s.wanted.text || !drop(&s.wanted));
    }
}

/// Whether a layer's text anchor sits below `scrolled` — the lines that
/// left for history, or were cleared away.
fn cells_col(w: &Wanted) -> f32 {
    match w.placement {
        angel_glass::Placement::Cells(c) => c[0],
        _ => 0.0,
    }
}

fn set_cells_col(w: &mut Wanted, column: f32) {
    if let angel_glass::Placement::Cells(c) = &mut w.placement {
        c[0] = column;
    }
}

fn cells_size(w: &Wanted) -> (f32, f32) {
    match w.placement {
        angel_glass::Placement::Cells(c) => (c[2], c[3]),
        _ => (1.0, 1.0),
    }
}

fn set_cells_size(w: &mut Wanted, width: f32, height: f32) {
    if let angel_glass::Placement::Cells(c) = &mut w.placement {
        c[2] = width;
        c[3] = height;
    }
}

fn picture_id(kind: &angel_glass::LayerKind) -> Option<u32> {
    match kind {
        angel_glass::LayerKind::Picture(id) | angel_glass::LayerKind::PicturePart(id, _) => Some(*id),
        _ => None,
    }
}

/// Move or cut one text-anchored layer. Returns an extra layer when the
/// scroll split it; `None` when the layer was removed.
fn scroll_wanted(
    w: &mut Wanted,
    band: &super::motion::BandScroll,
    image: &impl Fn(u32) -> (u32, u32),
) -> Option<Wanted> {
    if !w.text || w.alternate != band.alternate {
        return None;
    }
    let (width, height) = cells_size(w);
    let px = picture_id(&w.kind).map(image).unwrap_or((0, 0));
    let source = match w.kind {
        angel_glass::LayerKind::PicturePart(_, src) => Some(src),
        _ => None,
    };
    let rect = super::motion::Rect {
        line: w.line,
        column: cells_col(w),
        width,
        height,
        source,
    };
    match super::motion::scroll_rect(rect, band, px) {
        super::motion::Fate::Keep(next) => {
            write_wanted(w, &next);
            None
        }
        super::motion::Fate::Split(first, _) => {
            write_wanted(w, &first);
            None
        }
        super::motion::Fate::Gone => {
            w.line = u64::MAX;
            None
        }
    }
}

fn write_wanted(w: &mut Wanted, rect: &super::motion::Rect) {
    w.line = rect.line;
    set_cells_col(w, rect.column);
    set_cells_size(w, rect.width, rect.height);
    if let Some(src) = rect.source {
        let id = picture_id(&w.kind).unwrap_or(0);
        w.kind = angel_glass::LayerKind::PicturePart(id, src);
    }
}

fn below(w: &Wanted, scrolled: u64) -> bool {
    w.line < scrolled
}

/// A cells placement against a view row.
fn place_at(p: &angel_glass::Placement, row: f32) -> angel_glass::Placement {
    match p {
        angel_glass::Placement::Cells([c, _, w, h]) => angel_glass::Placement::Cells([*c, row, *w, *h]),
        other => other.clone(),
    }
}

/// A placement's height in cells, for the visibility test.
fn rect_h(p: &angel_glass::Placement) -> f32 {
    match p {
        angel_glass::Placement::Cells([_, _, _, h]) => *h,
        angel_glass::Placement::Pixels(_) => f32::from(u16::MAX),
    }
}

/// The dialect's entry point: one lifted `glass;` body. Replies go out
/// only when the program gave `r=`, and errors never panic.
pub fn handle(body: &[u8], canvas: &mut dyn Canvas) {
    let text = String::from_utf8_lossy(body);
    let Some(rest) = text.strip_prefix("glass;") else { return };
    // Op, key fields, then the payload as the last field for the ops
    // that take one (everything else is keys).
    let mut fields: Vec<&str> = rest.split(';').collect();
    let op = fields.first().copied().unwrap_or("").to_string();
    fields.remove(0);
    // The payload is the last field, and only when it cannot be a key:
    // a key=value field carries its '=' inside, base64 padding only at
    // the end (and a payload never travels without keys before it,
    // since every payload op needs at least id=).
    let takes_payload = PAYLOAD_OPS.contains(&op.as_str());
    let last_is_payload = match fields.last() {
        Some(last) if takes_payload && fields.len() >= 2 => {
            !last.contains('=') || last.ends_with('=')
        }
        _ => false,
    };
    let mut payload = "";
    if last_is_payload {
        payload = fields.pop().unwrap_or("");
    }
    let keys = Keys::from_parts(&op, &fields);
    // A chunked picture owns its reply: the first piece's `r` answers
    // when the last piece lands, so every other op replies here.
    let token = keys.get("r").map(str::to_string);
    let chunked_picture = matches!(op.as_str(), "picture" | "frame" | "asset") && keys.get("m").is_some_and(|v| v == "1");
    let result = dispatch(&keys, payload, canvas);
    if !chunked_picture && let Some(token) = token {
        let reply = match result {
            Ok(extra) => format!("\x1b_glass;r={token};OK;{extra}\x1b\\"),
            Err(Bad(code, message)) => format!("\x1b_glass;r={token};{code}:{message}\x1b\\"),
        };
        canvas.reply(reply.into_bytes());
    }
}

fn dispatch(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    // The op must be one this build takes before its keys are read.
    if !OPS.contains(&keys.op.as_str()) {
        return Err(Bad("EINVAL".into(), format!("unknown op {}", keys.op)));
    }
    keys.check()?;
    match keys.op.as_str() {
        "hello" => Ok(format!("v={VERSION},ops={}", OPS.join(","))),
        "picture" => picture(keys, payload, canvas),
        "layer" => layer(keys, canvas),
        "group" => group(keys, payload, canvas),
        "remove" => remove(keys, canvas),
        "showcase" => showcase(keys, canvas),
        "scene" => scene(keys, payload, canvas),
        "frame" => frame(keys, payload, canvas),
        "chart" => chart(keys, payload, canvas),
        "model" => model(keys, payload, canvas),
        "page" => page(keys, payload, canvas),
        "progress" => progress(keys, canvas),
        "asset" => asset(keys, payload, canvas),
        "tween" => tween(keys, canvas),
        "play" => play(keys, canvas),
        _ => unreachable!(),
    }
}

/// `picture`: upload a picture under a program id, decoding through the
/// APC graphics adapter's loader — `f=100` takes whatever the image crate
/// decodes (PNG, JPEG, GIF, TIFF, WebP, BMP, ICO, TGA, PNM), plus raw
/// `f=32` and `f=24`, the same compression (`o=z`) and caps. Chunked transmissions ride the store's
/// one-open-transmission rule; the first piece's keys govern, and the
/// reply waits for the last piece.
fn picture(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let data = payload.as_bytes().to_vec();
    if let Some(key) = canvas.open_chunk() {
        // A continuation: whatever keys it repeats, the first piece's
        // govern; only its `m` says whether more follow.
        let more = keys.flag("m")? == Some(true);
        let assembled = canvas.chunk(key, &data, more);
        if more {
            return Ok(String::new());
        }
        let first = Keys::parse(&String::from_utf8_lossy(&canvas.take_chunk_head(key)));
        let result = finish_payload(&first, &assembled, canvas);
        if let Some(token) = first.get("r").map(str::to_string) {
            let reply = match &result {
                Ok(extra) => format!("\x1b_glass;r={token};OK;{extra}\x1b\\"),
                Err(Bad(code, message)) => format!("\x1b_glass;r={token};{code}:{message}\x1b\\"),
            };
            canvas.reply(reply.into_bytes());
        }
        return result.map(|_| String::new());
    }
    if keys.flag("m")? == Some(true) {
        let key = canvas.begin_chunk();
        canvas.chunk_head(key, keys.raw.clone());
        canvas.chunk(key, &data, true);
        return Ok(String::new());
    }
    upload(keys, &data, canvas)
}

/// One whole picture payload, decoded and kept under the program's id.
fn upload(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let Some(id) = keys.u32("id")? else {
        return Err(Bad("EINVAL".into(), "picture needs id=".into()));
    };
    if id >= ID_BASE {
        return Err(Bad("EINVAL".into(), format!("id must be under {ID_BASE:#x}")));
    }
    // The same loader the APC graphics adapter uses, under its caps and
    // the store's budget: `f=100|32|24`, `o=z`, `s=`/`v=` for raw data.
    let key_text = format!(
        "f={},s={},v={},o={}",
        keys.get("f").unwrap_or("32"),
        keys.get("s").unwrap_or("0"),
        keys.get("v").unwrap_or("0"),
        keys.get("o").unwrap_or(""),
    );
    let (width, height, rgba) = apc_graphics::load_picture(&key_text, data)
        .map_err(|(code, message)| Bad(code, message))?;
    let window_id = ID_BASE + id;
    canvas.store(super::Picture { id: window_id, width, height, rgba, space: super::Space::Glass });
    let kept = Kept { id };
    let pictures = &mut canvas.expansion().pictures;
    if let Some(at) = pictures.iter().position(|k| k.id == id) {
        pictures[at] = kept;
    } else {
        pictures.push(kept);
    }
    Ok(String::new())
}

/// `layer`: create or replace one layer by `id`.
fn layer(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let wanted = wanted_layer(keys, canvas)?;
    let id = wanted.id;
    let state = canvas.expansion();
    // The program set every property outright: its tweens stop.
    state.tweens.stop_target(Target::Layer(id - ID_BASE));
    if let Some(at) = state.layers.iter().position(|w| w.id == id) {
        state.layers[at] = wanted;
    } else {
        state.layers.push(wanted);
    }
    Ok(String::new())
}

/// `group`: replace a whole set in one go, for per-frame animation. The
/// payload is `n` layer records, newline-separated, base64-encoded.
fn group(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let Some(group_id) = keys.u32("g")? else {
        return Err(Bad("EINVAL".into(), "group needs g=".into()));
    };
    let Some(count) = keys.u32("n")? else {
        return Err(Bad("EINVAL".into(), "group needs n=".into()));
    };
    if count as usize > GROUP_CAP {
        return Err(Bad("EINVAL".into(), format!("n is capped at {GROUP_CAP}")));
    }
    let decoded = base64(payload.as_bytes())
        .ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    if decoded.len() > GROUP_BYTES {
        return Err(Bad("EFBIG".into(), format!("payload is capped at {GROUP_BYTES} bytes")));
    }
    let text = String::from_utf8(decoded)
        .map_err(|_| Bad("EBADF".into(), "payload is not layer records".into()))?;
    let mut wanted = Vec::with_capacity(count.min(1024) as usize);
    for (i, line) in text.split('\n').enumerate() {
        if i as u32 >= count {
            break;
        }
        if line.is_empty() {
            continue;
        }
        let record = Keys::parse(line);
        if record.op != "layer" {
            return Err(Bad("EINVAL".into(), format!("record {} is not a layer record", i + 1)));
        }
        record.check()?;
        wanted.push(wanted_layer(&record, canvas)?);
    }
    if wanted.len() != count as usize {
        return Err(Bad("EINVAL".into(), format!("payload holds {} records, n= said {count}", wanted.len())));
    }
    let state = canvas.expansion();
    if let Some(at) = state.groups.iter().position(|(g, _)| *g == group_id) {
        state.groups[at].1 = wanted;
    } else {
        state.groups.push((group_id, wanted));
    }
    Ok(String::new())
}

/// `remove`: `id=` one layer (or an `a-b` span), `g=` one group,
/// `all=1` everything this dialect made.
fn remove(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    if keys.flag("all")? == Some(true) {
        let textures: Vec<u32> = canvas.expansion().assets.iter().flat_map(|a| a.textures.clone()).collect();
        canvas.expansion().reset();
        for id in textures {
            canvas.forget_glass(id);
        }
        return Ok(String::new());
    }
    if let Some(span) = keys.get("asset") {
        let (from, to) = id_span(span, "asset")?;
        let state = canvas.expansion();
        let mut gone = Vec::new();
        state.assets.retain(|a| {
            let keep = a.id < from || a.id > to;
            if !keep && Arc::strong_count(&a.asset) == 1 {
                gone.extend(a.textures.iter().copied());
            }
            keep
        });
        for id in gone {
            canvas.forget_glass(id);
        }
        return Ok(String::new());
    }
    if let Some(span) = keys.get("id") {
        let (from, to) = id_span(span, "id")?;
        let state = canvas.expansion();
        state.layers.retain(|w| w.id < ID_BASE + from || w.id > ID_BASE + to);
        state.scenes.retain(|s| s.window_id < ID_BASE + from || s.window_id > ID_BASE + to);
        return Ok(String::new());
    }
    if let Some(group_id) = keys.u32("g")? {
        canvas.expansion().groups.retain(|(g, _)| *g != group_id);
        return Ok(String::new());
    }
    Err(Bad("EINVAL".into(), "remove needs id=, g= or all=1".into()))
}

/// `a` or `a-b`, a span of program ids.
fn id_span(span: &str, key: &str) -> Result<(u32, u32), Bad> {
    let bad = || Bad("EINVAL".into(), format!("{key}= is not an id or an a-b range"));
    let (from, to) = match span.split_once('-') {
        Some((a, b)) => match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(a), Ok(b)) => (a, b),
            _ => return Err(bad()),
        },
        None => match span.parse::<u32>() {
            Ok(id) => (id, id),
            Err(_) => return Err(bad()),
        },
    };
    if to < from {
        return Err(Bad("EINVAL".into(), format!("the {key}= range ends before it starts")));
    }
    Ok((from, to))
}

/// `showcase`: open on an uploaded picture, a sized video panel, events, or close.
fn showcase(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    if let Some(on) = keys.flag("events")? {
        canvas.expansion().events = on;
    }
    if keys.flag("close")? == Some(true) {
        let state = canvas.expansion();
        state.ask = None;
        state.pending_frame = None;
        return Ok(String::new());
    }
    let fit = fit_of(keys)?;
    let has_id = keys.get("id").is_some();
    let has_size = keys.get("s").is_some() || keys.get("v").is_some();
    if has_id && has_size {
        return Err(Bad("EINVAL".into(), "showcase takes a picture or a size, not both".into()));
    }
    if has_id {
        let id = keys.u32("id")?.expect("id present");
        if !canvas.expansion().pictures.iter().any(|k| k.id == id) {
            return Err(Bad("EINVAL".into(), format!("no picture {id} was uploaded")));
        }
        let state = canvas.expansion();
        state.ask = Some(ShowcaseAsk::Picture { id, fit });
        if state.progress.is_some() { state.progress_dirty = true; }
        return Ok(String::new());
    }
    if has_size {
        let (width, height) = size_pair(keys, "showcase")?;
        let state = canvas.expansion();
        if let Some(ShowcaseAsk::Video { width: w, height: h, fit: old, .. }) = &state.ask
            && *w == width && *h == height && *old == fit
        {
            return Ok(String::new());
        }
        let generation = bump(state);
        state.ask = Some(ShowcaseAsk::Video { width, height, fit, generation });
        return Ok(String::new());
    }
    if keys.get("events").is_some() { return Ok(String::new()); }
    Err(Bad("EINVAL".into(), "showcase needs id=, s= and v=, events= or close=1".into()))
}

/// The furthest a layer may lift, as the stage caps it.
const MAX_LIFT: f32 = 8192.0;

/// The most motes one emitter may keep in the air, as the stage caps it.
const MAX_MOTES: u16 = 4096;

/// One layer record's keys, as the frame draws it.
fn wanted_layer(keys: &Keys, canvas: &mut dyn Canvas) -> Result<Wanted, Bad> {
    let program_id = keys.u32("id")?.ok_or_else(|| Bad("EINVAL".into(), "layer needs id=".into()))?;
    if program_id >= ID_BASE {
        return Err(Bad("EINVAL".into(), format!("id must be under {ID_BASE:#x}")));
    }
    let id = ID_BASE + program_id;
    let z = keys.i32("z")?.unwrap_or(0);
    let opacity = match keys.f32("o")? {
        None => 1.0,
        Some(v) if (0.0..=1.0).contains(&v) => v,
        Some(_) => return Err(Bad("EINVAL".into(), "o= is outside 0..1".into())),
    };
    let angle = keys.f32("a")?.unwrap_or(0.0);
    let height = keys.f32("h")?.unwrap_or(0.0).clamp(0.0, MAX_LIFT);
    let breathe = keys.f32("b")?.unwrap_or(0.0).clamp(0.0, MAX_LIFT);
    let additive = keys.flag("add")?.unwrap_or(false);
    let flicker = keys.f32("fl")?.unwrap_or(0.0).clamp(0.0, 1.0);
    let stand = match keys.get("stand") {
        None | Some("floor") => angel_glass::Stand::Floor,
        Some("up") => angel_glass::Stand::Upright(foot_of(keys)?),
        Some("screen") => angel_glass::Stand::Screen,
        Some(other) => {
            return Err(Bad("EINVAL".into(), format!("stand={other} is not floor, up or screen")))
        }
    };
    let (placement, text, line, alternate, y0) = placement_of(keys, canvas)?;
    let kind = kind_of(keys, canvas)?;
    Ok(Wanted {
        id,
        z,
        opacity,
        angle,
        height,
        breathe,
        additive,
        flicker,
        stand,
        placement,
        kind,
        text,
        line,
        alternate,
        row_off: 0.0,
        y0,
    })
}

/// A `stand=up` foot: `up=x,y` in placement pixels.
fn foot_of(keys: &Keys) -> Result<[f32; 2], Bad> {
    let Some(v) = keys.get("up") else { return Ok([0.0, 0.0]) };
    let bad = || Bad("EINVAL".into(), "up= is not two numbers".into());
    let mut parts = v.split(',');
    let x = parts.next().unwrap_or("").parse::<f32>().ok().filter(|n| n.is_finite()).ok_or_else(bad)?;
    let y = parts.next().unwrap_or("").parse::<f32>().ok().filter(|n| n.is_finite()).ok_or_else(bad)?;
    if parts.next().is_some() {
        return Err(bad());
    }
    Ok([x, y])
}

/// A record's placement: `at=` cells anchored to the text by default,
/// `anchor=window` pinning it, or `px=` window pixels always pinned.
/// Text-anchored placements pin to the absolute line the cursor stands
/// on, exactly as the intake's own adapters pin theirs. The last field
/// is the whole row `at=` named, which the anchor line took.
#[allow(clippy::type_complexity)]
fn placement_of(keys: &Keys, canvas: &mut dyn Canvas) -> Result<(angel_glass::Placement, bool, u64, bool, f32), Bad> {
    if let Some(v) = keys.get("px") {
        let quad = list4(Some(v), "px")?;
        return Ok((angel_glass::Placement::Pixels(quad), false, 0, false, 0.0));
    }
    let Some(v) = keys.get("at") else {
        return Err(Bad("EINVAL".into(), "layer needs at= or px=".into()));
    };
    let quad = list4(Some(v), "at")?;
    if keys.get("anchor").is_some_and(|v| v == "window") {
        return Ok((angel_glass::Placement::Cells(quad), false, 0, false, 0.0));
    }
    let (cursor_row, _column) = canvas.cursor();
    let alternate = canvas.alternate();
    let scrolled = canvas.scrolled();
    // The anchor is the cursor's absolute line; `at=`'s row counts from
    // there, so a layer rides the text it was placed against.
    let line = scrolled.saturating_add(u64::from(cursor_row)).saturating_add(quad[1] as u64);
    let y0 = (quad[1] as u64) as f32;
    let quad = [quad[0], 0.0, quad[2], quad[3]];
    Ok((angel_glass::Placement::Cells(quad), true, line, alternate, y0))
}

/// A record's kind: every kind the stage draws.
fn kind_of(keys: &Keys, canvas: &mut dyn Canvas) -> Result<angel_glass::LayerKind, Bad> {
    Ok(match keys.get("k").unwrap_or("solid") {
        "solid" => angel_glass::LayerKind::Solid(rgba_of(keys)?),
        "picture" => {
            let id = keys.u32("p")?.ok_or_else(|| Bad("EINVAL".into(), "picture needs p=".into()))?;
            let gpu = canvas
                .gpu_of(ID_BASE + id)
                .ok_or_else(|| Bad("EINVAL".into(), format!("no picture {id} was uploaded")))?;
            angel_glass::LayerKind::Picture(gpu)
        }
        "part" => {
            let id = keys.u32("p")?.ok_or_else(|| Bad("EINVAL".into(), "part needs p=".into()))?;
            let gpu = canvas
                .gpu_of(ID_BASE + id)
                .ok_or_else(|| Bad("EINVAL".into(), format!("no picture {id} was uploaded")))?;
            let quad = keys.quad("src")?.ok_or_else(|| Bad("EINVAL".into(), "part needs src=".into()))?;
            angel_glass::LayerKind::PicturePart(gpu, quad)
        }
        "glow" => angel_glass::LayerKind::Glow(rgba_of(keys)?),
        "particles" => angel_glass::LayerKind::Particles(particles_of(keys)),
        "prim" => {
            let shape = keys.u32("shape")?.unwrap_or(0);
            if shape > 6 {
                return Err(Bad("EINVAL".into(), "shape is 0..6".into()));
            }
            let speed = keys.f32("speed")?.unwrap_or(1.0);
            let phase = keys.f32("phase")?.unwrap_or(0.0);
            angel_glass::LayerKind::Primitive { shape, color: rgba_of(keys)?, phase, speed }
        }
        kind @ ("ring" | "line") => {
            let width = keys.f32("lw")?.unwrap_or(1.5);
            let glow = keys.f32("glow")?.unwrap_or(0.0);
            if !(width.is_finite() && glow.is_finite()) {
                return Err(Bad("EINVAL".into(), "lw and glow are numbers".into()));
            }
            angel_glass::LayerKind::Stroke(angel_glass::Stroke {
                shape: if kind == "ring" { angel_glass::StrokeShape::Ring } else { angel_glass::StrokeShape::Line },
                color: rgba_of(keys)?,
                width: width.clamp(0.25, angel_glass::MAX_STROKE_PX),
                glow: glow.clamp(0.0, angel_glass::MAX_STROKE_PX),
            })
        }
        other => return Err(Bad("EINVAL".into(), format!("k={other} is not a kind"))),
    })
}

/// A particle emitter, with the stage's own caps.
fn particles_of(keys: &Keys) -> angel_glass::Particles {
    angel_glass::Particles {
        color: rgba_of(keys).unwrap_or([1.0, 1.0, 1.0, 1.0]),
        count: keys.u32("n").unwrap_or(None).unwrap_or(64).min(u32::from(MAX_MOTES)) as u16,
        size: keys.f32("size").unwrap_or(None).unwrap_or(3.0).clamp(0.5, 64.0),
        life: keys.f32("life").unwrap_or(None).unwrap_or(2.0).clamp(0.05, 600.0),
        rise: keys.f32("rise").unwrap_or(None).unwrap_or(20.0).clamp(-MAX_LIFT, MAX_LIFT),
        sway: keys.f32("sway").unwrap_or(None).unwrap_or(8.0).clamp(-MAX_LIFT, MAX_LIFT),
        seed: keys.u32("seed").unwrap_or(None).unwrap_or(0),
        burst: keys.f32("burst").unwrap_or(None),
    }
}

/// Counters the session last told the program, so an event fires only
/// when one changes.
#[derive(Clone, Copy, Default)]
pub struct ShowcaseSeen {
    pub baselined: bool,
    pub steps: i32,
    pub copies: u32,
    pub dismissals: u32,
    pub clicks: u32,
}

/// The window's showcase counters, as `render` read them.
#[derive(Clone, Copy, Default)]
pub struct ShowcaseNow {
    pub seek: Option<f32>,
    pub steps: i32,
    pub copies: u32,
    pub dismissals: u32,
    pub clicks: u32,
}

/// `ESC _ glass;event;showcase;<key>=<value> ESC \`, in the order the
/// counters are read. Nothing, unless the program opted in.
pub fn showcase_event_bytes(opted_in: bool, seen: &mut ShowcaseSeen, now: ShowcaseNow) -> Vec<u8> {
    if !opted_in {
        return Vec::new();
    }
    let mut changes: Vec<(&str, String)> = Vec::new();
    if let Some(seek) = now.seek {
        changes.push(("seek", format_seek(seek)));
    }
    if seen.baselined {
        if seen.steps != now.steps {
            changes.push(("steps", now.steps.to_string()));
        }
        if seen.copies != now.copies {
            changes.push(("copies", now.copies.to_string()));
        }
        if seen.dismissals != now.dismissals {
            changes.push(("dismissals", now.dismissals.to_string()));
        }
        if seen.clicks != now.clicks {
            changes.push(("clicks", now.clicks.to_string()));
        }
        if now.dismissals > seen.dismissals {
            changes.push(("close", "1".into()));
        }
    }
    seen.steps = now.steps;
    seen.copies = now.copies;
    seen.dismissals = now.dismissals;
    seen.clicks = now.clicks;
    seen.baselined = true;
    let mut out = Vec::new();
    for (key, value) in &changes {
        out.extend(format!("\x1b_glass;event;showcase;{key}={value}\x1b\\").into_bytes());
    }
    out
}

/// A seek value, short and stable.
pub fn format_seek(v: f32) -> String {
    let text = format!("{v:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" { "0".into() } else { trimmed.to_string() }
}

fn wire_err(err: super::glass_scene::WireError) -> Bad {
    Bad(err.code.into(), err.message)
}

fn size_pair(keys: &Keys, op: &str) -> Result<(u32, u32), Bad> {
    let Some(width) = keys.u32("s")? else {
        return Err(Bad("EINVAL".into(), format!("{op} needs s=")));
    };
    let Some(height) = keys.u32("v")? else {
        return Err(Bad("EINVAL".into(), format!("{op} needs v=")));
    };
    super::glass_scene::pixel_size(width, height).map_err(wire_err)
}

fn fit_of(keys: &Keys) -> Result<angel_glass::Fit, Bad> {
    match keys.get("fit") {
        None | Some("contain") => Ok(angel_glass::Fit::Contain),
        Some("width") => Ok(angel_glass::Fit::Width),
        Some(other) => Err(Bad("EINVAL".into(), format!("fit={other} is not contain or width"))),
    }
}

fn bump(state: &mut Expansion) -> u64 {
    state.generation = state.generation.saturating_add(1);
    if state.progress.is_some() {
        state.progress_dirty = true;
    }
    state.generation
}

/// `scene`: a placed 3D scene, drawn into a picture and shown like a layer.
fn scene(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let Some(id) = keys.u32("id")? else {
        return Err(Bad("EINVAL".into(), "scene needs id=".into()));
    };
    if id >= ID_BASE {
        return Err(Bad("EINVAL".into(), format!("id must be under {ID_BASE:#x}")));
    }
    let size = size_pair(keys, "scene")?;
    let wanted = scene_wanted(keys, canvas, ID_BASE + id)?;
    let decoded = base64(payload.as_bytes())
        .ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    if decoded.len() > super::glass_scene::SCENE_BYTES {
        return Err(Bad(
            "EFBIG".into(),
            format!("scene payload is capped at {} bytes", super::glass_scene::SCENE_BYTES),
        ));
    }
    let mut wired = super::glass_scene::parse(&decoded).map_err(wire_err)?;
    show_pictures(&mut wired, canvas)?;
    let state = canvas.expansion();
    let super::glass_scene::Wired { mut scene, meshes: inline, view, gltf, .. } = wired;
    let program_nodes = scene.nodes.len();
    let meshes = assign_meshes(state, &mut scene, inline);
    let mut instances = hang_assets(state, &mut scene, &gltf)?;
    let old = state.scenes.iter().position(|s| s.program_id == id);
    if let Some(at) = old {
        // A scene sent again keeps its clips playing where they were, and
        // sends up only the assets it did not already show.
        let clock = state.clock;
        let before = &state.scenes[at];
        for inst in &mut instances {
            let same = before.instances.iter().find(|o| o.host == inst.host && o.asset_id == inst.asset_id);
            if let Some(prev) = same
                && let Some(mut play) = prev.play
                && valid_clip(&inst.asset, play.clip)
            {
                let (t, _) = play.local(clock, inst.asset.duration(play.clip));
                inst.pose(&mut scene.nodes, play.clip, t);
                play.shown = Some(t);
                inst.play = Some(play);
            }
        }
        let shown: Vec<*const glass_gltf::Asset> = before.instances.iter().map(|i| Arc::as_ptr(&i.asset)).collect();
        for inst in &instances {
            if !shown.contains(&Arc::as_ptr(&inst.asset)) {
                queue_asset(&mut state.uploads, &inst.asset);
            }
        }
    } else {
        for inst in &instances {
            queue_asset(&mut state.uploads, &inst.asset);
        }
    }
    // The program set the scene outright: its tweens stop.
    state.tweens.stop_target(Target::Layer(id));
    state.tweens.stop_scene(id);
    let kept = KeptScene {
        program_id: id,
        window_id: ID_BASE + id,
        scene,
        meshes,
        size,
        wanted,
        dirty: true,
        meshes_sent: false,
        view,
        program_nodes,
        instances,
    };
    if let Some(at) = old {
        state.scenes[at] = kept;
    } else {
        state.scenes.push(kept);
    }
    Ok(String::new())
}

/// Give a wire's inline meshes their window ids.
fn assign_meshes(
    state: &mut Expansion,
    scene: &mut angel_glass::scene::Scene,
    inline: Vec<angel_glass::scene::MeshData>,
) -> Vec<(u32, angel_glass::scene::MeshData)> {
    use angel_glass::scene::MeshRef;
    let mut meshes = Vec::new();
    for node in &mut scene.nodes {
        if let Some(MeshRef::Uploaded(index)) = node.mesh {
            let Some(mesh) = inline.get(index as usize) else {
                node.mesh = None;
                continue;
            };
            let id = state.next_mesh;
            state.next_mesh = state.next_mesh.saturating_add(1);
            node.mesh = Some(MeshRef::Uploaded(id));
            meshes.push((id, mesh.clone()));
        }
    }
    meshes
}

/// Hang each asset the wire names from its node, after the program's
/// nodes, the node's colour over every material of it.
fn hang_assets(
    state: &Expansion,
    scene: &mut angel_glass::scene::Scene,
    gltf: &[super::glass_scene::GltfRef],
) -> Result<Vec<Instance>, Bad> {
    let mut instances = Vec::with_capacity(gltf.len());
    for (k, r) in gltf.iter().enumerate() {
        let kept = state
            .assets
            .iter()
            .find(|a| a.id == r.asset)
            .ok_or_else(|| Bad("EINVAL".into(), format!("no asset {} was uploaded", r.asset)))?;
        let asset = Arc::clone(&kept.asset);
        let which = r.scene.map_or(asset.scene, |s| s as usize);
        let Some(host) = scene.nodes.get(r.node) else { continue };
        let (host_id, material) = (host.id, host.material);
        let room = SCENE_EXPANDED_NODES.saturating_sub(scene.nodes.len());
        let inst = glass_gltf::instantiate(&asset, r.asset, which, host_id, k as u32, room, &mut scene.nodes)
            .map_err(wire_err)?;
        inst.shade(&mut scene.nodes, &material);
        instances.push(inst);
    }
    Ok(instances)
}

/// An asset's meshes, to go up to the window.
fn queue_asset(uploads: &mut Vec<(u32, Arc<angel_glass::scene::MeshData>)>, asset: &glass_gltf::Asset) {
    for prim in &asset.prims {
        if !uploads.iter().any(|(id, _)| *id == prim.id) {
            uploads.push((prim.id, Arc::clone(&prim.mesh)));
        }
    }
}

fn valid_clip(asset: &glass_gltf::Asset, clip: ClipSel) -> bool {
    match clip {
        ClipSel::One(i) => i < asset.clips.len(),
        ClipSel::All => !asset.clips.is_empty(),
    }
}

fn scene_wanted(keys: &Keys, canvas: &mut dyn Canvas, window_id: u32) -> Result<Wanted, Bad> {
    let (placement, text, line, alternate, y0) = placement_of(keys, canvas).map_err(|Bad(code, message)| {
        Bad(code, message.replace("layer needs", "scene needs"))
    })?;
    let positive = match &placement {
        angel_glass::Placement::Cells([_, _, w, h]) | angel_glass::Placement::Pixels([_, _, w, h]) => {
            *w > 0.0 && *h > 0.0
        }
    };
    if !positive {
        return Err(Bad("EINVAL".into(), "scene placement must have a positive size".into()));
    }
    Ok(Wanted {
        id: window_id,
        z: keys.i32("z")?.unwrap_or(1),
        opacity: match keys.f32("o")? {
            None => 1.0,
            Some(v) if (0.0..=1.0).contains(&v) => v,
            Some(_) => return Err(Bad("EINVAL".into(), "o= is outside 0..1".into())),
        },
        angle: 0.0,
        height: 0.0,
        breathe: 0.0,
        additive: false,
        flicker: 0.0,
        stand: angel_glass::Stand::Floor,
        placement,
        kind: angel_glass::LayerKind::Picture(window_id),
        text,
        line,
        alternate,
        row_off: 0.0,
        y0,
    })
}

/// `frame`: one video frame, a picture upload or raw RGBA, chunked like
/// `picture`. `id=` names a picture already uploaded.
fn frame(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    if canvas.open_chunk().is_some() || keys.flag("m")? == Some(true) {
        return picture(keys, payload, canvas);
    }
    store_frame(keys, payload.as_bytes(), canvas)
}

fn finish_payload(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) -> Result<String, Bad> {
    match keys.op.as_str() {
        "frame" => store_frame(keys, data, canvas),
        "asset" => load_asset(keys, data, canvas),
        _ => upload(keys, data, canvas),
    }
}

fn store_frame(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) -> Result<String, Bad> {
    if let Some(id) = keys.u32("id")? {
        if !canvas.expansion().pictures.iter().any(|k| k.id == id) {
            return Err(Bad("EINVAL".into(), format!("no picture {id} was uploaded")));
        }
        canvas.expansion().pending_frame = Some(PendingFrame::Picture(id));
        return Ok(String::new());
    }
    if data.is_empty() {
        return Err(Bad("EINVAL".into(), "frame needs id= or a picture payload".into()));
    }
    if data.len() > super::glass_scene::SCENE_BYTES {
        return Err(Bad(
            "EFBIG".into(),
            format!("frame payload is capped at {} bytes", super::glass_scene::SCENE_BYTES),
        ));
    }
    let key_text = format!(
        "f={},s={},v={},o={}",
        keys.get("f").unwrap_or("32"),
        keys.get("s").unwrap_or("0"),
        keys.get("v").unwrap_or("0"),
        keys.get("o").unwrap_or(""),
    );
    let (width, height, rgba) =
        apc_graphics::load_picture(&key_text, data).map_err(|(code, message)| Bad(code, message))?;
    super::glass_scene::pixel_size(width, height).map_err(wire_err)?;
    canvas.expansion().pending_frame = Some(PendingFrame::Pixels { width, height, rgba });
    Ok(String::new())
}

/// `chart`: marks over a picture uploaded with `picture`. `p` names that
/// picture, and its size is the chart's size. The marks lie over it.
/// Without `p=` it is an error. The same request does not reopen the showcase.
fn chart(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let Some(id) = keys.u32("p")? else {
        return Err(Bad("EINVAL".into(), "chart needs p=".into()));
    };
    if id >= ID_BASE {
        return Err(Bad("EINVAL".into(), "picture id is out of range".into()));
    }
    let Some(size) = canvas.glass_size(ID_BASE + id) else {
        return Err(Bad("ENOENT".into(), "no such picture".into()));
    };
    let fit = fit_of(keys)?;
    let Some(count) = keys.u32("n")? else {
        return Err(Bad("EINVAL".into(), "chart needs n=".into()));
    };
    if count as usize > CHART_CAP {
        return Err(Bad("EINVAL".into(), format!("n is capped at {CHART_CAP}")));
    }
    let marks = chart_marks(payload, count)?;
    let state = canvas.expansion();
    if let Some(ShowcaseAsk::Chart {
        marks: old,
        size: old_size,
        fit: old_fit,
        picture,
        ..
    }) = &state.ask
        && old == &marks
        && *old_size == size
        && *old_fit == fit
        && *picture == id
    {
        return Ok(String::new());
    }
    let generation = bump(state);
    state.ask = Some(ShowcaseAsk::Chart { marks, size, fit, picture: id, generation });
    Ok(String::new())
}

fn chart_marks(payload: &str, count: u32) -> Result<Vec<angel_glass::Mark>, Bad> {
    let decoded = base64(payload.as_bytes())
        .ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    if decoded.len() > GROUP_BYTES {
        return Err(Bad("EFBIG".into(), format!("payload is capped at {GROUP_BYTES} bytes")));
    }
    let text = String::from_utf8(decoded)
        .map_err(|_| Bad("EBADF".into(), "payload is not mark records".into()))?;
    let mut marks = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        if i as u32 >= count {
            break;
        }
        if line.is_empty() {
            continue;
        }
        let record = Keys::parse(line);
        if record.op != "mark" {
            return Err(Bad("EINVAL".into(), format!("record {} is not a mark record", i + 1)));
        }
        record.check()?;
        let x = record.f32("x")?.ok_or_else(|| Bad("EINVAL".into(), format!("mark {} needs x=", i + 1)))?;
        let y = record.f32("y")?.ok_or_else(|| Bad("EINVAL".into(), format!("mark {} needs y=", i + 1)))?;
        let w = record.f32("w")?.ok_or_else(|| Bad("EINVAL".into(), format!("mark {} needs w=", i + 1)))?;
        let h = record.f32("h")?.ok_or_else(|| Bad("EINVAL".into(), format!("mark {} needs h=", i + 1)))?;
        if w <= 0.0 || h <= 0.0 {
            return Err(Bad("EINVAL".into(), format!("mark {} must have a positive size", i + 1)));
        }
        let anchor = match (record.f32("ax")?, record.f32("ay")?) {
            (Some(ax), Some(ay)) => [ax, ay],
            (None, None) => [x + w * 0.5, y + h * 0.5],
            _ => return Err(Bad("EINVAL".into(), format!("mark {} needs both ax= and ay=, or neither", i + 1))),
        };
        let text = record
            .get("t")
            .map(|t| t.split('|').map(str::to_string).collect())
            .unwrap_or_default();
        marks.push(angel_glass::Mark { rect: [x, y, w, h], anchor, text });
    }
    if marks.len() != count as usize {
        return Err(Bad("EINVAL".into(), format!("payload holds {} records, n= said {count}", marks.len())));
    }
    Ok(marks)
}

/// `model`: scene nodes, through the scene wire, shown in the showcase.
fn model(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let size = size_pair(keys, "model")?;
    let fit = fit_of(keys)?;
    let decoded = base64(payload.as_bytes())
        .ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    if decoded.len() > super::glass_scene::SCENE_BYTES {
        return Err(Bad(
            "EFBIG".into(),
            format!("model payload is capped at {} bytes", super::glass_scene::SCENE_BYTES),
        ));
    }
    let mut wired = super::glass_scene::parse(&decoded).map_err(wire_err)?;
    if wired.scene.nodes.is_empty() {
        return Err(Bad("EINVAL".into(), "model needs at least one node".into()));
    }
    show_pictures(&mut wired, canvas)?;
    let state = canvas.expansion();
    let super::glass_scene::Wired { mut scene, meshes: inline, gltf, .. } = wired;
    let meshes = assign_meshes(state, &mut scene, inline);
    for inst in hang_assets(state, &mut scene, &gltf)? {
        queue_asset(&mut state.uploads, &inst.asset);
    }
    let nodes = scene.nodes;
    let generation = bump(state);
    state.ask = Some(ShowcaseAsk::Model { nodes, meshes, size, fit, generation });
    Ok(String::new())
}

/// Put each node's `picture` on its mesh: the program's picture id becomes
/// the window's, as a layer's `p=` does. A picture sent again under the
/// same id shows on the next scene sent (a video on a face).
fn show_pictures(wired: &mut super::glass_scene::Wired, canvas: &mut dyn Canvas) -> Result<(), Bad> {
    for &(index, id, sampling) in &wired.pictures {
        let gpu = canvas
            .gpu_of(ID_BASE + id)
            .ok_or_else(|| Bad("EINVAL".into(), format!("no picture {id} was uploaded")))?;
        if let Some(node) = wired.scene.nodes.get_mut(index) {
            node.material.source = angel_glass::scene::TextureSource::Picture(gpu);
            node.material.sampling = sampling;
        }
    }
    // A cube's faces, each its own picture or the node's material, and a
    // rounded cube's edge: refused by name, as `picture` is, for a picture
    // never uploaded.
    for (index, dress) in &wired.dress {
        let mut gpu = Vec::with_capacity(dress.faces.len());
        for face in &dress.faces {
            gpu.push(match face {
                None => None,
                Some(id) => Some(
                    canvas
                        .gpu_of(ID_BASE + id)
                        .ok_or_else(|| Bad("EINVAL".into(), format!("no picture {id} was uploaded")))?,
                ),
            });
        }
        if let Some(node) = wired.scene.nodes.get_mut(*index) {
            node.face_materials = dress.materials(&node.material, |g| gpu.get(g).copied().flatten());
        }
    }
    Ok(())
}

/// `page`: text the window rasterises as a page and shows in the showcase.
fn page(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let cols = page_dim(keys, "cols")?;
    let rows = page_dim(keys, "rows")?;
    let decoded = base64(payload.as_bytes())
        .ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    if decoded.len() > super::glass_scene::SCENE_BYTES {
        return Err(Bad(
            "EFBIG".into(),
            format!("page payload is capped at {} bytes", super::glass_scene::SCENE_BYTES),
        ));
    }
    let text = String::from_utf8(decoded)
        .map_err(|_| Bad("EBADF".into(), "page payload is not text".into()))?;
    let lines = text.lines().count();
    if lines > PAGE_MAX as usize {
        return Err(Bad("EINVAL".into(), format!("page is capped at {PAGE_MAX} lines")));
    }
    let state = canvas.expansion();
    if let Some(ShowcaseAsk::Page { text: old, cols: oc, rows: or, .. }) = &state.ask
        && old == &text && *oc == cols && *or == rows
    {
        return Ok(String::new());
    }
    let generation = bump(state);
    state.ask = Some(ShowcaseAsk::Page { text, cols, rows, generation });
    Ok(String::new())
}

fn page_dim(keys: &Keys, name: &str) -> Result<u16, Bad> {
    let Some(n) = keys.u32(name)? else {
        return Err(Bad("EINVAL".into(), format!("page needs {name}=")));
    };
    if n == 0 || n > PAGE_MAX {
        return Err(Bad("EINVAL".into(), format!("{name} must be 1..={PAGE_MAX}")));
    }
    Ok(n as u16)
}

/// `progress`: where playback is, and whether it is paused. This does
/// not reopen the showcase.
fn progress(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let at = keys.f32("t")?;
    let pause = keys.flag("pause")?;
    if at.is_none() && pause.is_none() {
        return Err(Bad("EINVAL".into(), "progress needs t= or pause=".into()));
    }
    if let Some(at) = at && !(0.0..=1.0).contains(&at) {
        return Err(Bad("EINVAL".into(), "t must be 0..=1".into()));
    }
    let state = canvas.expansion();
    state.progress = Some((at, pause.unwrap_or(false)));
    state.progress_dirty = true;
    Ok(String::new())
}

/// `asset`: a glTF 2.0 file (`.glb`, or `.gltf` with its buffers inside
/// it) under a program id, chunked like `picture`, or read from a path
/// with `t=f` under the APC file medium's rules. Its colour textures go
/// in as pictures; its meshes go up when a scene first shows it.
fn asset(keys: &Keys, payload: &str, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    if canvas.open_chunk().is_some() || keys.flag("m")? == Some(true) {
        return picture(keys, payload, canvas);
    }
    load_asset(keys, payload.as_bytes(), canvas)
}

fn load_asset(keys: &Keys, data: &[u8], canvas: &mut dyn Canvas) -> Result<String, Bad> {
    use super::glass_gltf::ASSET_BYTES;
    let Some(id) = keys.u32("id")? else {
        return Err(Bad("EINVAL".into(), "asset needs id=".into()));
    };
    if id >= ID_BASE {
        return Err(Bad("EINVAL".into(), format!("id must be under {ID_BASE:#x}")));
    }
    // Base64 is four bytes for three; whitespace between them is allowed,
    // so the text may run somewhat longer, never twice the file.
    if data.len() > ASSET_BYTES * 2 {
        return Err(Bad("EFBIG".into(), format!("an asset is capped at {ASSET_BYTES} bytes")));
    }
    let decoded = base64(data).ok_or_else(|| Bad("EBADF".into(), "payload is not base64".into()))?;
    let bytes = match keys.get("t").unwrap_or("d") {
        "d" => decoded,
        "f" => {
            let path = String::from_utf8(decoded).map_err(|_| Bad("EBADF".into(), "the path is not utf-8".into()))?;
            apc_graphics::read_file(std::path::Path::new(&path), false, None, 0).map_err(|(code, message)| Bad(code, message))?
        }
        other => return Err(Bad("EINVAL".into(), format!("t={other} is not d or f"))),
    };
    if bytes.len() > ASSET_BYTES {
        return Err(Bad("EFBIG".into(), format!("an asset is capped at {ASSET_BYTES} bytes")));
    }
    let loaded = glass_gltf::load(&bytes).map_err(wire_err)?;
    drop(bytes);
    let state = canvas.expansion();
    let old = state.assets.iter().position(|a| a.id == id);
    let others: usize = state.assets.iter().filter(|a| a.id != id).map(|a| a.asset.bytes).sum();
    if others + loaded.asset.bytes > ASSET_BUDGET {
        return Err(Bad("ENOMEM".into(), format!("assets' meshes are capped at {ASSET_BUDGET} bytes together; remove one first")));
    }
    if old.is_none() && state.assets.len() >= ASSET_COUNT {
        return Err(Bad("ENOMEM".into(), format!("at most {ASSET_COUNT} assets are kept; remove one first")));
    }
    let mut asset = loaded.asset;
    for prim in &mut asset.prims {
        prim.id = state.next_mesh;
        state.next_mesh = state.next_mesh.saturating_add(1);
    }
    let mut windows = Vec::with_capacity(loaded.images.len());
    for _ in &loaded.images {
        windows.push(state.next_texture);
        state.next_texture = if state.next_texture >= MESH_BASE - 1 { TEXTURE_BASE } else { state.next_texture + 1 };
    }
    let mut gpu = Vec::with_capacity(windows.len());
    let images = loaded.images.len();
    for (image, window) in loaded.images.into_iter().zip(&windows) {
        canvas.store(super::Picture {
            id: *window,
            width: image.width,
            height: image.height,
            rgba: image.rgba,
            space: super::Space::Glass,
        });
        gpu.push(canvas.gpu_of(*window));
    }
    for m in &mut asset.materials {
        if let Some(Some(g)) = m.image.and_then(|i| gpu.get(i)) {
            m.material.source = angel_glass::scene::TextureSource::Picture(*g);
        }
    }
    let seconds: Vec<String> = asset.clips.iter().map(|c| format_seek(c.duration)).collect();
    let mut summary = format!(
        "nodes={},meshes={},primitives={},vertices={},triangles={},materials={},textures={},clips={}",
        asset.nodes.len(),
        asset.meshes.len(),
        asset.prims.len(),
        asset.vertices,
        asset.triangles,
        asset.materials.len().saturating_sub(1),
        images,
        asset.clips.len(),
    );
    if !seconds.is_empty() {
        summary.push_str(&format!(",seconds={}", seconds.join("/")));
    }
    let skipped = asset.skipped;
    if skipped != glass_gltf::Skipped::default() {
        summary.push_str(&format!(",skipped={}/{}/{}", skipped.primitives, skipped.images, skipped.channels));
    }
    let kept = KeptAsset { id, asset: Arc::new(asset), textures: windows };
    let state = canvas.expansion();
    let gone = match old {
        Some(at) => {
            let prev = std::mem::replace(&mut state.assets[at], kept);
            if Arc::strong_count(&prev.asset) == 1 { prev.textures } else { Vec::new() }
        }
        None => {
            state.assets.push(kept);
            Vec::new()
        }
    };
    for window in gone {
        canvas.forget_glass(window);
    }
    Ok(summary)
}

/// Numbers, `a,b,c`, exactly `n` of them, every one finite.
fn list_n(value: &str, n: usize, key: &str) -> Result<Vec<f32>, Bad> {
    let bad = || Bad("EINVAL".into(), format!("{key}= is not {n} finite numbers"));
    let out: Vec<f32> = value
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok().filter(|v| v.is_finite()))
        .collect::<Option<Vec<f32>>>()
        .ok_or_else(bad)?;
    if out.len() != n {
        return Err(bad());
    }
    Ok(out)
}

/// A time in seconds, `0..=3600`.
fn seconds_of(keys: &Keys, key: &str, default: f64) -> Result<f64, Bad> {
    match keys.f32(key)? {
        None => Ok(default),
        Some(v) if (0.0..=glass_tween::TIME_MAX as f32).contains(&v) => Ok(f64::from(v)),
        Some(_) => Err(Bad("EINVAL".into(), format!("{key}= must be 0..={} seconds", glass_tween::TIME_MAX))),
    }
}

/// The scene a tween or a play names by `id=`.
fn scene_of(state: &Expansion, keys: &Keys) -> Result<usize, Bad> {
    let id = keys.u32("id")?.ok_or_else(|| Bad("EINVAL".into(), "needs id= of a scene".into()))?;
    state
        .scenes
        .iter()
        .position(|s| s.program_id == id)
        .ok_or_else(|| Bad("EINVAL".into(), format!("no scene {id}")))
}

/// What a `tween` names: `layer=`, or `id=` with `node=` or `camera=1`.
fn tween_target(state: &Expansion, keys: &Keys) -> Result<Option<Target>, Bad> {
    if let Some(id) = keys.u32("layer")? {
        if keys.get("id").is_some() || keys.get("node").is_some() || keys.get("camera").is_some() {
            return Err(Bad("EINVAL".into(), "a tween names layer=, or id= with node= or camera=1, not both".into()));
        }
        let known = state.layers.iter().any(|w| w.id == ID_BASE.wrapping_add(id)) || state.scenes.iter().any(|s| s.program_id == id);
        if id >= ID_BASE || !known {
            return Err(Bad("EINVAL".into(), format!("no layer {id}")));
        }
        return Ok(Some(Target::Layer(id)));
    }
    if keys.get("id").is_none() {
        return Ok(None);
    }
    let at = scene_of(state, keys)?;
    let scene = &state.scenes[at];
    let camera = keys.flag("camera")? == Some(true);
    match (keys.get("node"), camera) {
        (Some(_), true) => Err(Bad("EINVAL".into(), "a tween names node= or camera=1, not both".into())),
        (Some(node), false) => {
            let node = node.parse::<u64>().map_err(|_| Bad("EINVAL".into(), "node= is not a node id".into()))?;
            if !scene.scene.nodes[..scene.program_nodes.min(scene.scene.nodes.len())].iter().any(|n| n.id == node) {
                return Err(Bad("EINVAL".into(), format!("scene {} has no node {node}", scene.program_id)));
            }
            Ok(Some(Target::Node { scene: scene.program_id, node }))
        }
        (None, true) => Ok(Some(Target::Camera(scene.program_id))),
        (None, false) => Err(Bad("EINVAL".into(), "a scene tween needs node= or camera=1".into())),
    }
}

/// The properties a tween moves, and where to: only those its target has.
fn tween_tracks(state: &Expansion, keys: &Keys, target: Target) -> Result<Vec<Track>, Bad> {
    const LAYER: &[&str] = &["at", "px", "o", "a", "c", "size", "b", "h"];
    const NODE: &[&str] = &["at", "rot", "scale", "color", "emissive", "visible"];
    const CAMERA: &[&str] = &["eye", "target", "fov"];
    const ALL: &[&str] = &[
        "at", "px", "o", "a", "c", "size", "b", "h", "rot", "scale", "color", "emissive", "visible", "eye", "target", "fov",
    ];
    let (allowed, what) = match target {
        Target::Layer(_) => (LAYER, "a layer"),
        Target::Node { .. } => (NODE, "a scene node"),
        Target::Camera(_) => (CAMERA, "a camera"),
    };
    if let Some(key) = ALL.iter().find(|k| keys.get(k).is_some() && !allowed.contains(k)) {
        return Err(Bad("EINVAL".into(), format!("{key}= is not a property of {what}")));
    }
    let mut tracks = Vec::new();
    let unit = |key: &str| -> Result<Option<f32>, Bad> {
        match keys.f32(key)? {
            Some(v) if !(0.0..=1.0).contains(&v) => Err(Bad("EINVAL".into(), format!("{key}= is outside 0..1"))),
            other => Ok(other),
        }
    };
    match target {
        Target::Layer(id) => {
            let w = state
                .layers
                .iter()
                .find(|w| w.id == ID_BASE + id)
                .or_else(|| state.scenes.iter().find(|s| s.program_id == id).map(|s| &s.wanted))
                .ok_or_else(|| Bad("EINVAL".into(), format!("no layer {id}")))?;
            let pixels = matches!(w.placement, angel_glass::Placement::Pixels(_));
            if let Some(v) = keys.get("at") {
                if pixels {
                    return Err(Bad("EINVAL".into(), format!("layer {id} is placed with px=; tween px=")));
                }
                tracks.push(Track::new(Prop::Rect, Value::V4(list4(Some(v), "at")?)));
            }
            if let Some(v) = keys.get("px") {
                if !pixels {
                    return Err(Bad("EINVAL".into(), format!("layer {id} is placed with at=; tween at=")));
                }
                tracks.push(Track::new(Prop::Rect, Value::V4(list4(Some(v), "px")?)));
            }
            if let Some(o) = unit("o")? {
                tracks.push(Track::new(Prop::Opacity, Value::F(o)));
            }
            if let Some(a) = keys.f32("a")? {
                tracks.push(Track::new(Prop::Angle, Value::F(a)));
            }
            if keys.get("c").is_some() {
                if layer_color(&w.kind).is_none() {
                    return Err(Bad("EINVAL".into(), format!("layer {id} has no colour to tween")));
                }
                tracks.push(Track::new(Prop::Color, Value::V4(rgba_of(keys)?)));
            }
            if let Some(size) = keys.f32("size")? {
                if !matches!(w.kind, angel_glass::LayerKind::Particles(_)) {
                    return Err(Bad("EINVAL".into(), format!("size= is for particles; layer {id} is not")));
                }
                if !(0.5..=64.0).contains(&size) {
                    return Err(Bad("EINVAL".into(), "size= must be 0.5..=64".into()));
                }
                tracks.push(Track::new(Prop::Size, Value::F(size)));
            }
            for (key, prop) in [("b", Prop::Breathe), ("h", Prop::Lift)] {
                if let Some(v) = keys.f32(key)? {
                    if !(0.0..=MAX_LIFT).contains(&v) {
                        return Err(Bad("EINVAL".into(), format!("{key}= must be 0..={MAX_LIFT}")));
                    }
                    tracks.push(Track::new(prop, Value::F(v)));
                }
            }
        }
        Target::Node { .. } => {
            if let Some(v) = keys.get("at") {
                let n = list_n(v, 3, "at")?;
                tracks.push(Track::new(Prop::NodeAt, Value::V3([n[0], n[1], n[2]])));
            }
            if let Some(v) = keys.get("rot") {
                let n = list_n(v, 4, "rot")?;
                if n.iter().all(|v| *v == 0.0) {
                    return Err(Bad("EINVAL".into(), "rot= must not be zero".into()));
                }
                let q = angel_glass::scene::quat_normalize([n[0], n[1], n[2], n[3]]);
                tracks.push(Track::new(Prop::NodeRot, Value::Q(q)));
            }
            if let Some(v) = keys.get("scale") {
                let n = list_n(v, 3, "scale")?;
                tracks.push(Track::new(Prop::NodeScale, Value::V3([n[0], n[1], n[2]])));
            }
            if let Some(v) = keys.get("color") {
                let n = list_n(v, 4, "color")?;
                if n.iter().any(|c| !(0.0..=1.0).contains(c)) {
                    return Err(Bad("EINVAL".into(), "color= components must be in 0..1".into()));
                }
                tracks.push(Track::new(Prop::NodeColor, Value::V4([n[0], n[1], n[2], n[3]])));
            }
            if let Some(e) = unit("emissive")? {
                tracks.push(Track::new(Prop::NodeEmissive, Value::F(e)));
            }
            if let Some(v) = keys.flag("visible")? {
                tracks.push(Track::new(Prop::NodeVisible, Value::B(v)));
            }
        }
        Target::Camera(scene) => {
            let kept = state.scenes.iter().find(|s| s.program_id == scene);
            for (key, prop) in [("eye", Prop::Eye), ("target", Prop::LookAt)] {
                if let Some(v) = keys.get(key) {
                    let n = list_n(v, 3, key)?;
                    tracks.push(Track::new(prop, Value::V3([n[0], n[1], n[2]])));
                }
            }
            if let Some(fov) = keys.f32("fov")? {
                if kept.is_some_and(|s| s.view.fov.is_none()) {
                    return Err(Bad("EINVAL".into(), "fov= is for a perspective camera".into()));
                }
                if !(fov > 0.0 && fov < std::f32::consts::PI) {
                    return Err(Bad("EINVAL".into(), "fov= must be in (0, pi)".into()));
                }
                tracks.push(Track::new(Prop::Fov, Value::F(fov)));
            }
        }
    }
    Ok(tracks)
}

/// `tween`: move a property of a layer, a scene node or a scene's camera
/// from where it is to where the program says, eased, while glass draws
/// every frame and the program says nothing.
fn tween(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let state = canvas.expansion();
    let target = tween_target(state, keys)?;
    let tid = keys.u32("tid")?;
    if keys.flag("stop")? == Some(true) {
        match (target, tid) {
            (Some(target), _) => state.tweens.stop_target(target),
            (None, Some(tid)) => state.tweens.stop_tid(tid),
            (None, None) => return Err(Bad("EINVAL".into(), "stop=1 needs a target or tid=".into())),
        }
        return Ok(String::new());
    }
    let target = target.ok_or_else(|| Bad("EINVAL".into(), "a tween needs layer=, or id= with node= or camera=1".into()))?;
    let tracks = tween_tracks(state, keys, target)?;
    if tracks.is_empty() {
        return Err(Bad("EINVAL".into(), "a tween needs a property to move".into()));
    }
    let ease = Ease::parse(keys.get("ease").unwrap_or("linear")).map_err(|m| Bad("EINVAL".into(), m))?;
    let dur = seconds_of(keys, "dur", 0.3)?;
    let delay = seconds_of(keys, "delay", 0.0)?;
    let repeat = match keys.get("repeat") {
        None => Repeat::Count(0),
        Some("forever") => Repeat::Forever,
        Some(n) => Repeat::Count(
            n.parse::<u32>()
                .ok()
                .filter(|n| *n <= glass_tween::REPEAT_MAX)
                .ok_or_else(|| Bad("EINVAL".into(), format!("repeat= is a count up to {} or forever", glass_tween::REPEAT_MAX)))?,
        ),
    };
    let after = keys.u32("after")?;
    if after.is_some() && after == tid {
        return Err(Bad("EINVAL".into(), "a tween cannot wait on itself".into()));
    }
    let timing = Timing {
        delay_ms: (delay * 1000.0).round() as u64,
        run_ms: ease.own_duration_ms().unwrap_or(dur * 1000.0),
        repeat,
        yoyo: keys.flag("yoyo")?.unwrap_or(false),
    };
    let mut tween = Tween::new(target, tracks, ease, timing);
    tween.tid = tid;
    tween.after = after;
    state.tweens.add(tween).map_err(|m| Bad("ENOMEM".into(), m))?;
    Ok(String::new())
}

/// The furthest a clip may be sought, in seconds.
const SEEK_MAX: f32 = 3_600.0;

/// `play`: play, pause, seek, loop, speed or stop an asset's clip on the
/// glTF nodes of a scene (all of them, or the one hanging from `node=`).
fn play(keys: &Keys, canvas: &mut dyn Canvas) -> Result<String, Bad> {
    let state = canvas.expansion();
    let at = scene_of(state, keys)?;
    let clock = state.clock;
    let node = match keys.get("node") {
        None => None,
        Some(v) => Some(v.parse::<u64>().map_err(|_| Bad("EINVAL".into(), "node= is not a node id".into()))?),
    };
    let clip = match keys.get("clip") {
        None => None,
        Some("all") => Some(ClipSel::All),
        Some(v) => Some(ClipSel::One(v.parse::<usize>().map_err(|_| Bad("EINVAL".into(), "clip= is an index or all".into()))?)),
    };
    let speed = match keys.f32("speed")? {
        Some(s) if !(-16.0..=16.0).contains(&s) => return Err(Bad("EINVAL".into(), "speed= must be -16..=16".into())),
        other => other,
    };
    let seek = match keys.f32("t")? {
        Some(t) if !(0.0..=SEEK_MAX).contains(&t) => {
            return Err(Bad("EINVAL".into(), "t= must be 0..=3600 seconds".into()))
        }
        other => other,
    };
    let looping = keys.flag("loop")?;
    let pause = keys.flag("pause")?;
    let stop = keys.flag("stop")? == Some(true);
    let scene = &mut state.scenes[at];
    let picked: Vec<usize> = (0..scene.instances.len()).filter(|i| node.is_none_or(|n| scene.instances[*i].host == n)).collect();
    if picked.is_empty() {
        return Err(Bad("EINVAL".into(), match node {
            Some(n) => format!("no glTF asset hangs from node {n} of scene {}", scene.program_id),
            None => format!("scene {} shows no glTF asset", scene.program_id),
        }));
    }
    let mut longest = 0.0f32;
    for i in picked {
        let inst = &mut scene.instances[i];
        if stop {
            inst.play = None;
            inst.rest(&mut scene.scene.nodes);
            scene.dirty = true;
            continue;
        }
        if inst.asset.clips.is_empty() {
            return Err(Bad("EINVAL".into(), format!("asset {} has no animations", inst.asset_id)));
        }
        if let Some(sel) = clip
            && !valid_clip(&inst.asset, sel)
        {
            return Err(Bad("EINVAL".into(), format!("asset {} has {} clips", inst.asset_id, inst.asset.clips.len())));
        }
        let mut p = inst.play.unwrap_or_else(|| Playback::new(clip.unwrap_or(ClipSel::One(0))));
        p.rebase(clock);
        if let Some(sel) = clip
            && sel != p.clip
        {
            p.clip = sel;
            p.at = 0.0;
            inst.rest(&mut scene.scene.nodes);
        }
        if let Some(s) = speed {
            p.speed = s;
        }
        if let Some(l) = looping {
            p.looping = l;
        }
        if let Some(paused) = pause {
            p.paused = paused;
        }
        let duration = inst.asset.duration(p.clip);
        if let Some(t) = seek {
            p.at = f64::from(t);
        } else if inst.play.is_none() && p.speed < 0.0 {
            p.at = f64::from(duration);
        }
        p.shown = None;
        inst.play = Some(p);
        longest = longest.max(duration);
    }
    if stop {
        return Ok(String::new());
    }
    Ok(format!("seconds={}", format_seek(longest)))
}

/// A layer kind's colour, for the ones that have one.
fn layer_color(kind: &angel_glass::LayerKind) -> Option<[f32; 4]> {
    match kind {
        angel_glass::LayerKind::Solid(c) | angel_glass::LayerKind::Glow(c) => Some(*c),
        angel_glass::LayerKind::Primitive { color, .. } => Some(*color),
        angel_glass::LayerKind::Particles(p) => Some(p.color),
        _ => None,
    }
}

/// What tweens move, in the dialect's own state.
struct Bind<'a> {
    layers: &'a mut Vec<Wanted>,
    scenes: &'a mut Vec<KeptScene>,
}

impl Bind<'_> {
    fn layer(&mut self, id: u32) -> Option<&mut Wanted> {
        let window = ID_BASE.wrapping_add(id);
        if let Some(at) = self.layers.iter().position(|w| w.id == window) {
            return self.layers.get_mut(at);
        }
        self.scenes.iter_mut().find(|s| s.program_id == id).map(|s| &mut s.wanted)
    }

    fn scene(&mut self, id: u32) -> Option<&mut KeptScene> {
        self.scenes.iter_mut().find(|s| s.program_id == id)
    }
}

/// A program node's place in its scene.
fn node_place(scene: &KeptScene, node: u64) -> Option<usize> {
    scene.scene.nodes[..scene.program_nodes.min(scene.scene.nodes.len())].iter().position(|n| n.id == node)
}

impl glass_tween::Targets for Bind<'_> {
    fn read(&mut self, target: Target, prop: Prop) -> Option<Value> {
        match target {
            Target::Layer(id) => {
                let w = self.layer(id)?;
                Some(match prop {
                    Prop::Rect => match w.placement {
                        angel_glass::Placement::Cells(c) if w.text => Value::V4([c[0], w.y0 + w.row_off, c[2], c[3]]),
                        angel_glass::Placement::Cells(c) | angel_glass::Placement::Pixels(c) => Value::V4(c),
                    },
                    Prop::Opacity => Value::F(w.opacity),
                    Prop::Angle => Value::F(w.angle),
                    Prop::Breathe => Value::F(w.breathe),
                    Prop::Lift => Value::F(w.height),
                    Prop::Color => Value::V4(layer_color(&w.kind)?),
                    Prop::Size => match &w.kind {
                        angel_glass::LayerKind::Particles(p) => Value::F(p.size),
                        _ => return None,
                    },
                    _ => return None,
                })
            }
            Target::Node { scene, node } => {
                let s = self.scene(scene)?;
                let n = &s.scene.nodes[node_place(s, node)?];
                Some(match prop {
                    Prop::NodeAt => Value::V3(n.transform.translation),
                    Prop::NodeRot => Value::Q(n.transform.rotation),
                    Prop::NodeScale => Value::V3(n.transform.scale),
                    Prop::NodeColor => Value::V4(n.material.tint),
                    Prop::NodeEmissive => Value::F(n.material.emissive),
                    Prop::NodeVisible => Value::B(n.visible),
                    _ => return None,
                })
            }
            Target::Camera(scene) => {
                let s = self.scene(scene)?;
                Some(match prop {
                    Prop::Eye => Value::V3(s.view.eye),
                    Prop::LookAt => Value::V3(s.view.target),
                    Prop::Fov => Value::F(s.view.fov?),
                    _ => return None,
                })
            }
        }
    }

    fn write(&mut self, target: Target, prop: Prop, value: Value) -> bool {
        match target {
            Target::Layer(id) => {
                let Some(w) = self.layer(id) else { return false };
                match (prop, value) {
                    (Prop::Rect, Value::V4(r)) => match &mut w.placement {
                        angel_glass::Placement::Cells(c) if w.text => {
                            c[0] = r[0];
                            c[2] = r[2].max(0.0);
                            c[3] = r[3].max(0.0);
                            w.row_off = r[1] - w.y0;
                        }
                        angel_glass::Placement::Cells(c) | angel_glass::Placement::Pixels(c) => {
                            *c = [r[0], r[1], r[2].max(0.0), r[3].max(0.0)];
                        }
                    },
                    (Prop::Opacity, Value::F(v)) => w.opacity = v.clamp(0.0, 1.0),
                    (Prop::Angle, Value::F(v)) => w.angle = v,
                    (Prop::Breathe, Value::F(v)) => w.breathe = v.clamp(0.0, MAX_LIFT),
                    (Prop::Lift, Value::F(v)) => w.height = v.clamp(0.0, MAX_LIFT),
                    (Prop::Color, Value::V4(c)) => {
                        let c = c.map(|v| v.clamp(0.0, 1.0));
                        match &mut w.kind {
                            angel_glass::LayerKind::Solid(k) | angel_glass::LayerKind::Glow(k) => *k = c,
                            angel_glass::LayerKind::Primitive { color, .. } => *color = c,
                            angel_glass::LayerKind::Particles(p) => p.color = c,
                            _ => return false,
                        }
                    }
                    (Prop::Size, Value::F(v)) => match &mut w.kind {
                        angel_glass::LayerKind::Particles(p) => p.size = v.clamp(0.5, 64.0),
                        _ => return false,
                    },
                    _ => return false,
                }
                true
            }
            Target::Node { scene, node } => {
                let Some(s) = self.scene(scene) else { return false };
                let Some(at) = node_place(s, node) else { return false };
                let n = &mut s.scene.nodes[at];
                match (prop, value) {
                    (Prop::NodeAt, Value::V3(v)) => n.transform.translation = v,
                    (Prop::NodeRot, Value::Q(q)) => n.transform.rotation = q,
                    (Prop::NodeScale, Value::V3(v)) => n.transform.scale = v,
                    (Prop::NodeColor, Value::V4(c)) => {
                        n.material.tint = c.map(|v| v.clamp(0.0, 1.0));
                        if n.material.source == angel_glass::scene::TextureSource::None {
                            n.material.alpha = if n.material.tint[3] < 1.0 {
                                angel_glass::scene::AlphaMode::Blend
                            } else {
                                angel_glass::scene::AlphaMode::Opaque
                            };
                        }
                    }
                    (Prop::NodeEmissive, Value::F(e)) => n.material.emissive = e.clamp(0.0, 1.0),
                    (Prop::NodeVisible, Value::B(v)) => n.visible = v,
                    _ => return false,
                }
                if matches!(prop, Prop::NodeColor | Prop::NodeEmissive) {
                    let host = s.scene.nodes[at].material;
                    // Its faces (a cube's six) take its colour, as a face
                    // that is the node's own or its picture is tinted by
                    // it; a rounded cube's edge keeps its own.
                    for face in s.scene.nodes[at].face_materials.iter_mut().take(6) {
                        face.tint = host.tint;
                        face.emissive = host.emissive;
                        if face.tint[3] < 1.0 && face.alpha == angel_glass::scene::AlphaMode::Opaque {
                            face.alpha = angel_glass::scene::AlphaMode::Blend;
                        }
                    }
                    for inst in s.instances.iter().filter(|i| i.host == node) {
                        inst.shade(&mut s.scene.nodes, &host);
                    }
                }
                s.dirty = true;
                true
            }
            Target::Camera(scene) => {
                let Some(s) = self.scene(scene) else { return false };
                match (prop, value) {
                    (Prop::Eye, Value::V3(v)) => s.view.eye = v,
                    (Prop::LookAt, Value::V3(v)) => s.view.target = v,
                    (Prop::Fov, Value::F(f)) => {
                        let f = f.clamp(1e-3, std::f32::consts::PI - 1e-3);
                        s.view.fov = Some(f);
                        if let angel_glass::scene::Projection::Perspective { fov_y, .. } = &mut s.scene.camera.projection {
                            *fov_y = f;
                        }
                    }
                    _ => return false,
                }
                s.scene.camera.transform.translation = s.view.eye;
                // An eye on its target, or looking straight along `up`,
                // keeps the turn it had.
                if let Ok(rotation) = super::glass_scene::look_rotation(s.view.eye, s.view.target, s.view.up) {
                    s.scene.camera.transform.rotation = rotation;
                }
                s.dirty = true;
                true
            }
        }
    }
}
