//! The cockpit in angelX's own window (docs/RENDER_FOUNDATION.md).
//!
//! Built with the `glass` feature and launched with `ANGEL_GLASS=1`, the
//! cockpit draws into a GPU window instead of the terminal. The few things it
//! otherwise writes straight to the terminal (the title, the bell, the
//! clipboard, the game keyboard mode) go to the window here; each helper
//! returns false outside the window so the terminal path runs as before.

#[cfg(feature = "glass")]
static HOST: std::sync::OnceLock<angel_glass::Host> = std::sync::OnceLock::new();

/// Whether this launch asked for the window.
#[cfg(feature = "glass")]
pub(crate) fn requested() -> bool {
    std::env::var("ANGEL_GLASS").is_ok_and(|v| v == "1")
}

#[cfg(feature = "glass")]
pub(crate) fn install(host: angel_glass::Host) {
    let _ = HOST.set(host);
    // The HD art pack is read as the window opens, so the first room drawn
    // has it (`ArtPack`).
    std::sync::LazyLock::force(&ART);
    // A signal's exit must happen on the window's thread, after the GPU is
    // released: exit() from the reaper thread tears the driver down while the
    // window is still presenting (SIGSEGV in the driver).
    #[cfg(target_os = "linux")]
    crate::agent::sandbox::process_owner::route_exit(|code| {
        if let Some(host) = HOST.get() {
            host.exit_process(code);
        }
    });
}

/// The window itself, for emitters that speak layers and stages
/// directly (the mini-viz presentation surface). `None` outside it.
#[cfg(feature = "glass")]
pub(crate) fn host() -> Option<&'static angel_glass::Host> {
    HOST.get()
}

/// Whether the cockpit is drawing into the GPU window.
pub(crate) fn active() -> bool {
    #[cfg(feature = "glass")]
    return HOST.get().is_some();
    #[cfg(not(feature = "glass"))]
    false
}

/// The window's cell size in pixels, for sizing pictures. `None` outside
/// the window.
pub(crate) fn cell_pixels() -> Option<(u16, u16)> {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        let grid = host.grid_size();
        return Some((
            u16::try_from(grid.cell_width).ok()?,
            u16::try_from(grid.cell_height).ok()?,
        ));
    }
    None
}

/// Hand a picture straight to the window in place of a Kitty upload: raw
/// RGBA, scaled on the GPU into its `cols` × `rows` cells. False outside the
/// window, where the caller uploads as before.
pub(crate) fn upload_picture(
    id: u32,
    (width, height): (u32, u32),
    rgba: Vec<u8>,
    (cols, rows): (u16, u16),
) -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        host.upload_picture(angel_glass::graphics::Picture {
            id,
            width,
            height,
            rgba,
            cols,
            rows,
        });
        return true;
    }
    let _ = (id, width, height, rgba, cols, rows);
    false
}

/// The layer group the Delve's thrown things travel in.
#[cfg(feature = "glass")]
const LIFTED_GROUP: u32 = 0x4C49_4654;

/// How much higher toward the camera a thrown thing rises in the window than
/// its arc in the picture: it grows as it comes up, its shadow stays down.
#[cfg(feature = "glass")]
const LIFT: f32 = 3.0;

/// Layer groups drawn this frame, and those on screen: a group a frame no
/// longer draws (its screen has gone) is cleared by `frame_done`.
#[cfg(feature = "glass")]
#[derive(Default)]
struct Groups {
    drawn: std::collections::HashSet<u32>,
    shown: std::collections::HashSet<u32>,
}

#[cfg(feature = "glass")]
static GROUPS: std::sync::LazyLock<std::sync::Mutex<Groups>> = std::sync::LazyLock::new(Default::default);

/// Show `layers` as `group` this frame (an empty list clears it).
#[cfg(feature = "glass")]
pub(crate) fn show_group(host: &angel_glass::Host, group: u32, layers: Vec<angel_glass::Layer>) {
    let mut groups = GROUPS.lock().unwrap();
    groups.drawn.insert(group);
    let empty = layers.is_empty();
    if !empty || groups.shown.contains(&group) {
        host.set_layer_group(group, layers);
    }
    if empty {
        groups.shown.remove(&group);
    } else {
        groups.shown.insert(group);
    }
}

/// GPU geometry for the mini-viz dock. frame_done removes this group on any
/// frame where its route/pane is no longer rendered.
pub(crate) fn object_dock(dock: &crate::ui::object_dock::ObjectDock, motion: crate::ui::viz::lifecycle_viz::MotionMode) {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        show_group(host, 0x444F_434B, crate::ui::object_dock::layers(dock, motion));
    }
    let _ = (dock, motion);
}

/// Small, cached gold-coin flip frames above the cockpit. The terminal draws
/// the same material as colored dots; the window gets its full-resolution rim
/// and embossed face. At most sixteen 96x96 pictures are uploaded once.
pub(crate) fn spend_coins(coins: &[crate::ui::viz::spend_viz::CoinPose]) -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        use angel_glass::{Layer, LayerKind, Placement, Stand};
        const GROUP: u32 = 0x434F_494E;
        const BASE: u32 = angel_glass::LAYER_PICTURES + 0x0400;
        const SIDE: u32 = 96;
        let mut layers = Vec::with_capacity(coins.len());
        for (index, coin) in coins.iter().enumerate() {
            let frame = ((coin.face_scale - 0.08) / 0.92 * 15.0).round().clamp(0.0, 15.0) as u32;
            let id = BASE + frame;
            if UPLOADED.lock().unwrap().insert(id) {
                let scale = 0.08 + frame as f32 / 15.0 * 0.92;
                let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
                for y in 0..SIDE {
                    for x in 0..SIDE {
                        let color = crate::ui::viz::spend_viz::gold_pixel(
                            (x as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                            (y as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0,
                            scale,
                        );
                        rgba.extend_from_slice(&color.map_or([0; 4], |[r, g, b]| [r, g, b, 255]));
                    }
                }
                host.upload_picture(angel_glass::graphics::Picture {
                    id, width: SIDE, height: SIDE, rgba, cols: 0, rows: 0,
                });
            }
            let area = coin.area;
            let mut layer = Layer::new(u64::from(GROUP) + index as u64, 8,
                Placement::Cells([area.x as f32, area.y as f32, area.width as f32, area.height as f32]),
                LayerKind::Picture(id));
            layer.stand = Stand::Screen;
            layers.push(layer);
        }
        show_group(host, GROUP, layers);
        return true;
    }
    let _ = coins;
    false
}

/// The layer group the Old Delve's knight portrait travels in.
#[cfg(feature = "glass")]
const PORTRAIT_GROUP: u32 = 0x504F_5254;

/// The chosen knight on the Old Delve's screen, drawn by the window: square
/// pixels at the window's own density in `area`, breathing, a warm glow
/// behind and its shadow on the ground. False outside the window, where the
/// caller paints its cells.
pub(crate) fn portrait(sprite: &crate::stage::world_viz::overworld::Img, area: ratatui::layout::Rect) -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        use angel_glass::{Layer, LayerKind, Placement};
        let (w, h) = (sprite.w.max(1) as u32, sprite.h.max(1) as u32);
        // One picture per look: hash the packed sprite words first so cached
        // frames never allocate or unpack an RGBA buffer.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (w, h, sprite.words()).hash(&mut hasher);
        let id = angel_glass::LAYER_PICTURES + 0x1000 + (hasher.finish() as u32 & 0x0fff_ffff);
        if UPLOADED.lock().unwrap().insert(id) {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    match sprite.get(x, y) {
                        Some([r, g, b]) => rgba.extend_from_slice(&[r, g, b, 255]),
                        None => rgba.extend_from_slice(&[0, 0, 0, 0]),
                    }
                }
            }
            host.upload_picture(angel_glass::graphics::Picture {
                id,
                width: w,
                height: h,
                rgba,
                cols: 0,
                rows: 0,
            });
        }
        let (x, y) = (f32::from(area.x), f32::from(area.y));
        let (aw, ah) = (f32::from(area.width), f32::from(area.height));
        // A warm halo behind, wider than the knight, lower than the head.
        let mut glow = Layer::new(
            0,
            -1,
            Placement::Cells([x - aw * 0.35, y + ah * 0.05, aw * 1.7, ah * 0.95]),
            LayerKind::Glow([0.95, 0.72, 0.38, 0.16]),
        );
        glow.breathe = 0.0;
        let mut knight = Layer::new(1, 2, Placement::Cells([x, y, aw, ah]), LayerKind::Picture(id));
        knight.breathe = PORTRAIT_BREATH_PX;
        show_group(host, PORTRAIT_GROUP, vec![glow, knight]);
        return true;
    }
    let _ = (sprite, area);
    false
}

/// How far the portrait rises with each breath, in window pixels.
#[cfg(feature = "glass")]
const PORTRAIT_BREATH_PX: f32 = 10.0;

/// Pictures already handed to the window, by id.
#[cfg(feature = "glass")]
static UPLOADED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
    std::sync::LazyLock::new(Default::default);

/// The Delve's thrown things as layers over the game picture placed in
/// `placed` cells (a `frame` of pixels): each one at its ground point,
/// lifted along its arc, its shadow on the ground.
pub(crate) fn lifted(
    things: &[crate::stage::world_viz::overworld::arena::Lifted],
    frame: (u32, u32),
    placed: ratatui::layout::Rect,
    tilt: f32,
) {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        let Some((_, cell_h)) = cell_pixels() else {
            return;
        };
        let tilt = angel_glass::Tilt::for_stage(tilt, frame);
        let layers = lifted_layers(things, frame, placed, cell_h, tilt, |kind, variant| {
            let id = lifted_picture(host, kind, variant);
            let size = SPRITE_SIZES
                .lock()
                .unwrap()
                .get(&id)
                .copied()
                .unwrap_or((4, 4));
            (id, size)
        });
        show_group(host, LIFTED_GROUP, layers);
    }
    let _ = (things, frame, placed, tilt);
}

/// One layer per thrown thing over the game picture placed in `placed`
/// cells (a `frame` of pixels, `cell_height` pixels to a row): its sprite at
/// the picture's scale, centred on its ground point, lifted along its arc.
/// On a tilted floor (`tilt`) the ground point and the size follow the floor.
/// `sprite` names each one's picture and its size.
#[cfg(feature = "glass")]
fn lifted_layers(
    things: &[crate::stage::world_viz::overworld::arena::Lifted],
    frame: (u32, u32),
    placed: ratatui::layout::Rect,
    cell_height: u16,
    tilt: Option<angel_glass::Tilt>,
    mut sprite: impl FnMut(crate::stage::world_viz::overworld::arena::LiftedKind, u8) -> (u32, (u32, u32)),
) -> Vec<angel_glass::Layer> {
    use crate::stage::world_viz::overworld::arena::{BAND_H, HUD_H, LiftedKind, NATIVE_H, NATIVE_W};
    use angel_glass::{Layer, LayerKind, Placement};
    let (fw, fh) = (frame.0.max(1) as f32, frame.1.max(1) as f32);
    // The picture is a seat's whole screen: the party's band over the room,
    // the HUD's band under it.
    let tall = (NATIVE_H + BAND_H + HUD_H) as f32;
    // Window pixels per native picture pixel.
    let scale = f32::from(placed.height) * f32::from(cell_height) / tall;
    things
        .iter()
        .enumerate()
        .map(|(i, thing)| {
            let (id, (sw, sh)) = sprite(thing.kind, thing.variant);
            let (ground, grow) = match &tilt {
                Some(t) => (t.floor([thing.x, thing.y]), t.scale_at([thing.x, thing.y])),
                None => ([thing.x, thing.y], 1.0),
            };
            let (w, h) = (
                sw as f32 / NATIVE_W as f32 * f32::from(placed.width) * grow,
                sh as f32 / tall * f32::from(placed.height) * grow,
            );
            let x = f32::from(placed.x) + ground[0] / fw * f32::from(placed.width);
            let y = f32::from(placed.y) + ground[1] / fh * f32::from(placed.height);
            let arc = match thing.kind {
                LiftedKind::Impling => 20.0,
                LiftedKind::Bomb | LiftedKind::Glob => 26.0,
            };
            let mut layer = Layer::new(
                i as u64,
                5,
                Placement::Cells([x - w / 2.0, y - h / 2.0, w, h]),
                LayerKind::Picture(id),
            );
            layer.height = (thing.k * std::f32::consts::PI).sin() * arc * scale * grow * LIFT;
            layer
        })
        .collect()
}

#[cfg(all(test, feature = "glass"))]
mod tests {
    use super::*;
    use crate::stage::world_viz::overworld::arena::{BAND_H, HUD_H, Lifted, LiftedKind, NATIVE_H, NATIVE_W};

    /// A seat's whole screen, native pixels tall.
    const TALL: f32 = (NATIVE_H + BAND_H + HUD_H) as f32;

    #[test]
    fn a_bomb_mid_flight_sits_on_its_ground_point_and_is_lifted() {
        // A seat's picture at twice native size (the band over the room)
        // placed 96 × 28 cells from (10, 2).
        let placed = ratatui::layout::Rect::new(10, 2, 96, 28);
        let bomb = Lifted {
            kind: LiftedKind::Bomb,
            x: 384.0,
            y: TALL,
            k: 0.5,
            variant: 0,
        };
        let layers = lifted_layers(&[bomb], (768, 2 * TALL as u32), placed, 25, None, |_, _| (7, (4, 4)));
        assert_eq!(layers.len(), 1);
        let layer = &layers[0];
        let angel_glass::Placement::Cells([x, y, w, h]) = layer.placement else {
            panic!("cells");
        };
        // Centred on the picture's middle: column 10 + 48, row 2 + 14.
        assert!((x + w / 2.0 - 58.0).abs() < 1e-4 && (y + h / 2.0 - 16.0).abs() < 1e-4);
        assert!((w - 4.0 / NATIVE_W as f32 * 96.0).abs() < 1e-5);
        assert!((h - 4.0 / TALL * 28.0).abs() < 1e-5);
        // Top of the arc: 26 native pixels, at the window's scale, times LIFT.
        let scale = 28.0 * 25.0 / TALL;
        assert!((layer.height - 26.0 * scale * LIFT).abs() < 1e-3);
        assert_eq!(layer.kind, angel_glass::LayerKind::Picture(7));
    }

    #[test]
    fn a_landing_bomb_is_on_the_ground() {
        let placed = ratatui::layout::Rect::new(0, 0, 96, 28);
        let bomb = Lifted {
            kind: LiftedKind::Bomb,
            x: 10.0,
            y: 10.0,
            k: 1.0,
            variant: 0,
        };
        let layers = lifted_layers(&[bomb], (768, 448), placed, 25, None, |_, _| (7, (4, 4)));
        assert!(layers[0].height.abs() < 1e-3);
    }
}

/// A thrown thing's sprite as a window picture, uploaded the first time it
/// is needed.
#[cfg(feature = "glass")]
fn lifted_picture(
    host: &angel_glass::Host,
    kind: crate::stage::world_viz::overworld::arena::LiftedKind,
    variant: u8,
) -> u32 {
    use crate::stage::world_viz::overworld::arena::{LiftedKind, lifted_sprite};
    let slot = match kind {
        LiftedKind::Bomb => 0,
        LiftedKind::Impling => 1,
        LiftedKind::Glob => 2,
    };
    let id = angel_glass::LAYER_PICTURES + 0x100 + slot * 8 + u32::from(variant & 7);
    let mut sizes = SPRITE_SIZES.lock().unwrap();
    sizes.entry(id).or_insert_with(|| {
        let img = lifted_sprite(kind, variant);
        let rgba: Vec<u8> = img
            .words()
            .iter()
            .flat_map(|&w| [(w >> 16) as u8, (w >> 8) as u8, w as u8, (w >> 24) as u8])
            .collect();
        let (w, h) = (img.w.max(1) as u32, img.h.max(1) as u32);
        host.upload_picture(angel_glass::graphics::Picture {
            id,
            width: w,
            height: h,
            rgba,
            cols: 0,
            rows: 0,
        });
        (w, h)
    });
    id
}

/// The size of each thrown thing's uploaded sprite, by picture id.
#[cfg(feature = "glass")]
static SPRITE_SIZES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<u32, (u32, u32)>>> =
    std::sync::LazyLock::new(Default::default);

/// The Delve's stage: its cells name it, so its id fits the placeholder's
/// 24 bits, and is a multiple of four, which the viewer's own pictures
/// never are (`dot_image_id`).
#[cfg(feature = "glass")]
const STAGE: u32 = 0x00FA_57A4;

/// The stage's maker (what it has sent), and whether the stage was drawn
/// this frame and is up at all.
#[cfg(feature = "glass")]
#[derive(Default)]
struct StageState {
    maker: crate::stage::world_viz::overworld::arena::stage_frame::StageMaker,
    drawn: bool,
    shown: bool,
}

#[cfg(feature = "glass")]
static STAGE_STATE: std::sync::LazyLock<std::sync::Mutex<StageState>> =
    std::sync::LazyLock::new(Default::default);

/// HD art for the Delve (docs/RENDER_FOUNDATION.md, step 6; docs/WORLD-TIERS.md,
/// "HD art packs"): a pack (`arena::hd_pack`), a folder of drawings and its
/// manifest, `pack.json`, each drawing a look of the game's (a sprite's, a
/// room kit's tile's: `arena::look_key`, sixteen hex digits) at its scale
/// (4 unless the pack says). The window draws a sprite's drawing in that
/// sprite's place and size, and a room whose kit the pack has tiles for is
/// composed of them, finer, in its still art's place; a look the pack has
/// no drawing of keeps its inks. Loose `<look>.png` drawings (the older
/// pack) are read too. A drawing may have its normal map beside it,
/// `<name>_n.png` (its size, green up; docs/WORLD-TIERS.md "Normal maps");
/// without one, one is made from the drawing. `ANGEL_GLASS_ART` names the
/// folder (default `~/.angelX/art/delve`), read once as the window opens.
/// `ANGEL_GLASS_ART_EXPORT` names a folder where each look the game shows
/// is saved once at its own size, listed in `looks.jsonl`, for the art
/// pipeline to redraw.
#[cfg(feature = "glass")]
#[derive(Default)]
struct ArtPack {
    /// The drawings sent up so far, by look.
    art: crate::stage::world_viz::overworld::arena::hd_pack::HdArt,
    export: Option<std::path::PathBuf>,
    exported: std::collections::HashSet<u64>,
}

#[cfg(feature = "glass")]
static ART: std::sync::LazyLock<std::sync::Mutex<ArtPack>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(ArtPack::from_environment()));

#[cfg(feature = "glass")]
impl ArtPack {
    /// The pack named by the environment, read and installed for the
    /// room's art; what it left out goes to the log.
    fn from_environment() -> Self {
        use crate::stage::world_viz::overworld::arena::hd_pack;
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let dir = std::env::var_os("ANGEL_GLASS_ART")
            .map(std::path::PathBuf::from)
            .or_else(|| home.map(|h| h.join(".angelX").join("art").join("delve")))
            .filter(|dir| dir.is_dir());
        let pack = dir.as_deref().and_then(hd_pack::Pack::open);
        if let Some(pack) = &pack {
            for warning in pack.warnings() {
                tracing::warn!(%warning, "an HD drawing was left out");
            }
            tracing::info!(drawings = pack.len(), scale = pack.scale(), "HD art pack");
        }
        hd_pack::install(pack.map(std::sync::Arc::new));
        let export = std::env::var_os("ANGEL_GLASS_ART_EXPORT").map(std::path::PathBuf::from);
        if let Some(export) = &export {
            let _ = std::fs::create_dir_all(export);
        }
        Self {
            export,
            ..Default::default()
        }
    }

    /// The window picture of `body`'s HD drawing, sent up the first time it
    /// is asked for, with its normal map when lit cards turn by them; `None`
    /// when the pack has none.
    fn hd(
        &mut self,
        host: &angel_glass::Host,
        body: &crate::stage::world_viz::overworld::arena::Body,
    ) -> Option<crate::stage::world_viz::overworld::arena::hd_pack::Placed> {
        self.art.normals = card_normals();
        let mut pictures = Vec::new();
        let id = self.art.picture(body, &mut pictures);
        for picture in pictures {
            host.upload_picture(angel_glass::graphics::Picture {
                id: picture.id,
                width: picture.width,
                height: picture.height,
                rgba: picture.rgba,
                cols: 0,
                rows: 0,
            });
        }
        id
    }

    /// Save a look the game showed, once, for the art pipeline.
    fn export(&mut self, body: &crate::stage::world_viz::overworld::arena::Body) {
        let Some(dir) = &self.export else {
            return;
        };
        if !self.exported.insert(body.key) {
            return;
        }
        let path = dir.join(format!("{:016x}.png", body.key));
        if path.exists() {
            return;
        }
        let (w, h) = (body.img.w.max(1) as u32, body.img.h.max(1) as u32);
        let rgba = crate::stage::world_viz::overworld::arena::stage_frame::rgba_of(&body.img);
        if image::save_buffer(&path, &rgba, w, h, image::ColorType::Rgba8).is_ok() {
            use std::io::Write;
            if let Ok(mut index) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("looks.jsonl"))
            {
                let _ = writeln!(index, "{{\"look\":\"{:016x}\",\"w\":{w},\"h\":{h}}}", body.key);
            }
        }
    }
}

/// Where the Delve's paintings come from (docs/DELVE.md, "Paintings"): the
/// host's own paint store, and, for a seat joined to another host, that
/// host's door (`/paint/<sha256>.png`), fetched into a cache of this
/// machine's own.
#[derive(Default, Clone)]
struct PaintSource {
    dir: Option<std::path::PathBuf>,
    host: Option<(String, angel_serve::client::Trust)>,
}

static PAINT_SOURCE: std::sync::LazyLock<std::sync::Mutex<PaintSource>> =
    std::sync::LazyLock::new(Default::default);

/// The host's paint store (`together_sheets::paint_dir`): its paintings
/// are read from there.
pub(crate) fn set_paint_dir(dir: std::path::PathBuf) {
    PAINT_SOURCE.lock().unwrap().dir = Some(dir);
}

/// The socket of the host a seat has joined: the paintings this machine
/// does not keep are fetched from beside it.
pub(crate) fn set_paint_host(url: &str, trust: &angel_serve::client::Trust) {
    PAINT_SOURCE.lock().unwrap().host = Some((url.to_string(), trust.clone()));
}

/// A painting's bytes, by its hash: from the host's store, this machine's
/// cache (`~/.angelX/cache/paint`), or the host's door, kept in the cache.
/// Anything not that hash's is not it.
fn painting_bytes(sha: &str, source: &PaintSource) -> Option<Vec<u8>> {
    use crate::knowledge::cut::sha256_hex;
    if !crate::drive::together_shooter::looks::paint_name(sha) {
        return None;
    }
    let name = format!("{sha}.png");
    let limit = crate::drive::together_sheets::MAX_PAINT_BYTES;
    let read = |path: std::path::PathBuf| {
        std::fs::metadata(&path)
            .ok()
            .filter(|m| m.is_file() && m.len() as usize <= limit)
            .and_then(|_| std::fs::read(&path).ok())
            .filter(|bytes| sha256_hex(bytes) == sha)
    };
    if let Some(bytes) = source.dir.as_ref().and_then(|dir| read(dir.join(&name))) {
        return Some(bytes);
    }
    let cache = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".angelX/cache/paint"));
    if let Some(bytes) = cache.as_ref().and_then(|dir| read(dir.join(&name))) {
        return Some(bytes);
    }
    let (url, trust) = source.host.as_ref()?;
    let bytes = angel_serve::client::http_get(url, &format!("/paint/{name}"), trust, std::time::Duration::from_secs(30), limit).ok()?;
    if sha256_hex(&bytes) != sha {
        return None;
    }
    if let Some(cache) = cache {
        let _ = crate::platform::workspace_store::write_atomic(&cache.join(&name), &bytes);
    }
    Some(bytes)
}

/// A painting as the window has it: being read and cut, sent up (cut at
/// `side`, all the painting has when `whole`), or not to be had (tried
/// again after a while).
#[cfg(feature = "glass")]
enum Painted {
    Loading { slot: u32, shown: Option<(u32, usize)> },
    Ready { slot: u32, side: u32, frames: usize, whole: bool },
    Missing { slot: u32, until: std::time::Instant },
}

/// A painting read and cut off the frame: its hash, the side asked for,
/// and the frames cut (their side), if it could be, with each frame's
/// normal map when cards are lit by them (`card_normals`).
#[cfg(feature = "glass")]
type PaintCut = (String, u32, Option<(u32, Vec<Vec<u8>>)>, Option<Vec<Vec<u8>>>);

/// Whether the window's lit cards turn by their normal maps
/// (docs/WORLD-TIERS.md, "Normal maps"): a painting's and an HD drawing's
/// are made, or read, only then.
#[cfg(feature = "glass")]
fn card_normals() -> bool {
    angel_delve::world3d::rooms_lit_look::lit_cards_on() && angel_delve::world3d::rooms_lit_look::card_normals_on()
}

/// The window's paintings: each one's frames, flipped and not, sent up
/// once at about twice the size it is shown (textures have no smaller
/// copies to draw from), read and cut on a thread of its own.
#[cfg(feature = "glass")]
#[derive(Default)]
struct Paintings {
    known: std::collections::HashMap<String, Painted>,
    next: u32,
    done: std::sync::Arc<std::sync::Mutex<Vec<PaintCut>>>,
}

/// The most paintings read at once.
#[cfg(feature = "glass")]
const PAINT_LOADS: usize = 4;
/// How long a painting that could not be had waits to be tried again.
#[cfg(feature = "glass")]
const PAINT_RETRY: std::time::Duration = std::time::Duration::from_secs(15);

#[cfg(feature = "glass")]
static PAINTINGS: std::sync::LazyLock<std::sync::Mutex<Paintings>> =
    std::sync::LazyLock::new(Default::default);

#[cfg(feature = "glass")]
impl Paintings {
    /// The window picture of `p`'s frame, flipped or not, shown `scale`
    /// window pixels to a room pixel; `None` until it is read and sent up.
    fn picture(&mut self, host: &angel_glass::Host, p: &crate::stage::world_viz::overworld::arena::Painting, scale: f32) -> Option<u32> {
        use crate::stage::world_viz::overworld::arena::paintings;
        self.sent(host);
        let key = if p.subject { format!("{}:subject",p.sha) } else { p.sha.clone() };
        let subject = p.subject;
        let side = paintings::side_for(p.rect[2].max(p.rect[3]) * scale);
        let picture = |slot: u32, frames: usize| (p.frame < frames).then(|| paintings::picture(slot, p.frame, p.flip));
        let (slot, shown) = match self.known.get(&key) {
            Some(&Painted::Ready { slot, side: had, frames, whole }) => {
                // Shown bigger than it was cut for: cut again, drawn as it
                // was meanwhile.
                if whole || side <= had + had / 2 {
                    return picture(slot, frames);
                }
                (slot, Some((had, frames)))
            }
            Some(&Painted::Loading { slot, shown }) => return shown.and_then(|(_, frames)| picture(slot, frames)),
            Some(&Painted::Missing { slot, until }) if std::time::Instant::now() >= until => (slot, None),
            Some(Painted::Missing { .. }) => return None,
            None if (self.next as usize) < paintings::MAX_PAINTINGS as usize => {
                self.next += 1;
                (self.next - 1, None)
            }
            None => return None,
        };
        if self.known.values().filter(|k| matches!(k, Painted::Loading { .. })).count() >= PAINT_LOADS {
            return shown.and_then(|(_, frames)| picture(slot, frames));
        }
        self.known.insert(key.clone(), Painted::Loading { slot, shown });
        let (sha, frames, pixel, done) = (p.sha.clone(), p.frames, p.pixel, std::sync::Arc::clone(&self.done));
        let source = PAINT_SOURCE.lock().unwrap().clone();
        // Its normal map's hash, when cards turn by them; the art pixels
        // its square spans, for one made from it.
        let normal = card_normals().then(|| p.normal.clone());
        let art = p.rect[2].max(p.rect[3]).max(1.0);
        let cache_key = key.clone();
        let started = std::thread::Builder::new().name("painting".into()).spawn(move || {
            let mut normals = None;
            let cut = painting_bytes(&sha, &source).and_then(|bytes| {
                let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?.to_rgba8();
                let (w, h) = image.dimensions();
                (w <= 2048 && h <= 2048).then_some(())?;
                // Pixel art in hard blocks, anything else averaged down.
                let (side, cut) = if subject { paintings::cut_subject(image.as_raw(), (w,h), frames, side)? } else { paintings::cut_as(pixel, image.as_raw(), (w, h), frames, side)? };
                if let Some(normal) = &normal {
                    // Its own map where it names one that reads and cuts;
                    // else one made from it.
                    let authored = normal
                        .as_ref().filter(|_| !subject)
                        .and_then(|n| painting_bytes(n, &source))
                        .and_then(|bytes| image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok())
                        .map(|n| n.to_rgba8())
                        .filter(|n| n.dimensions() == (w, h))
                        .and_then(|n| paintings::cut_normals(image.as_raw(), n.as_raw(), (w, h), frames, side));
                    normals = Some(authored.unwrap_or_else(|| paintings::made_normals(&cut, side, art)));
                }
                Some((side, cut))
            });
            done.lock().unwrap().push((cache_key, side, cut, normals));
        });
        if started.is_err() {
            self.known.insert(key.clone(), Painted::Missing { slot, until: std::time::Instant::now() + PAINT_RETRY });
        }
        shown.and_then(|(_, frames)| picture(slot, frames))
    }

    /// The paintings read and cut since, sent up.
    fn sent(&mut self, host: &angel_glass::Host) {
        use crate::stage::world_viz::overworld::arena::paintings;
        let done = std::mem::take(&mut *self.done.lock().unwrap());
        for (sha, asked, cut, normals) in done {
            let Some(&Painted::Loading { slot, shown }) = self.known.get(&sha) else {
                continue;
            };
            let state = match cut {
                Some((side, frames)) => {
                    for (f, rgba) in frames.iter().enumerate() {
                        for flip in [false, true] {
                            host.upload_picture(angel_glass::graphics::Picture {
                                id: paintings::picture(slot, f, flip),
                                width: side,
                                height: side,
                                rgba: if flip { paintings::flipped(rgba, side) } else { rgba.clone() },
                                cols: 0,
                                rows: 0,
                            });
                            // Its normal map beside it, mirrored with it.
                            if let Some(map) = normals.as_ref().and_then(|n| n.get(f)) {
                                host.upload_picture(angel_glass::graphics::Picture {
                                    id: angel_stage::normals::picture_of(paintings::picture(slot, f, flip)),
                                    width: side,
                                    height: side,
                                    rgba: if flip { angel_stage::normals::mirrored(map, (side, side)) } else { map.clone() },
                                    cols: 0,
                                    rows: 0,
                                });
                            }
                        }
                    }
                    Painted::Ready { slot, side, frames: frames.len(), whole: side < asked }
                }
                // Drawn as it was, if it was.
                None => match shown {
                    Some((side, frames)) => Painted::Ready { slot, side, frames, whole: true },
                    None => Painted::Missing { slot, until: std::time::Instant::now() + PAINT_RETRY },
                },
            };
            self.known.insert(sha, state);
        }
    }
}

/// The Delve's frame in parts, drawn by the window into the `placed` cells
/// of `area` (docs/RENDER_FOUNDATION.md, "The game pipeline"): the Delve
/// makes the stage (`arena::stage_frame`), the window draws it.
pub(crate) fn stage(
    scene: &std::sync::Arc<crate::stage::world_viz::overworld::arena::RoomScene>,
    area: ratatui::layout::Rect,
    placed: ratatui::layout::Rect,
    buffer: &mut ratatui::buffer::Buffer,
) {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        let mut state = STAGE_STATE.lock().unwrap();
        // Window pixels to a room pixel, for the size a painting is shown.
        let grid = host.grid_size();
        let scale = f32::from(placed.width) * grid.cell_width as f32 / scene.view.0.max(1) as f32;
        let frame = state.maker.make_with(
            scene,
            |body| {
                let mut art = ART.lock().unwrap();
                art.export(body);
                art.hd(host, body)
            },
            |painting| PAINTINGS.lock().unwrap().picture(host, painting, scale),
        );
        for picture in frame.pictures {
            host.upload_picture(angel_glass::graphics::Picture {
                id: picture.id,
                width: picture.width,
                height: picture.height,
                rgba: picture.rgba,
                cols: 0,
                rows: 0,
            });
        }
        for (id, mesh) in frame.meshes {
            host.upload_mesh(id, mesh);
        }
        host.set_stage(angel_glass::Stage {
            id: STAGE,
            size: frame.size,
            cols: placed.width,
            rows: placed.height,
            layers: frame.layers,
            scene: frame.scene,
            enter: frame.enter,
            tilt: frame.tilt,
            props: frame.props,
            impact: frame.impact,
        });
        state.drawn = true;
        state.shown = true;
        drop(state);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    cell.set_symbol(" ")
                        .set_bg(ratatui::style::Color::Rgb(0, 0, 0));
                }
            }
        }
        crate::ui::dots::protocol::placeholders(
            STAGE,
            ratatui::layout::Size::new(placed.width, placed.height),
            placed,
            buffer,
        );
    }
    let _ = (scene, area, placed, buffer);
}

/// After each drawn frame: a layer group nobody drew this frame (its screen
/// has gone: the Delve, the Old Delve's portrait) leaves the window too, and
/// so does the Delve's stage.
pub(crate) fn frame_done() {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        let mut groups = GROUPS.lock().unwrap();
        let gone: Vec<u32> = groups.shown.difference(&groups.drawn).copied().collect();
        for group in gone {
            host.set_layer_group(group, Vec::new());
            groups.shown.remove(&group);
        }
        groups.drawn.clear();
        let mut stage = STAGE_STATE.lock().unwrap();
        if stage.shown && !stage.drawn {
            host.remove_stage(STAGE);
            stage.shown = false;
            stage.maker.forget();
        }
        stage.drawn = false;
    }
    crate::ui::present::frame_done();
}

pub(crate) fn set_title(title: &str) -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        host.set_title(title);
        return true;
    }
    let _ = title;
    false
}

pub(crate) fn attention() -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        host.request_attention();
        return true;
    }
    false
}

pub(crate) fn game_keyboard(active: bool) -> bool {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        host.set_game_keyboard(active);
        return true;
    }
    let _ = active;
    false
}

pub(crate) fn game_surface(active: bool) {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() { host.set_game_surface(active); }
    let _ = active;
}

/// `Some(copied)` in the window, `None` in a terminal.
pub(crate) fn copy(text: &str) -> Option<bool> {
    #[cfg(feature = "glass")]
    if let Some(host) = HOST.get() {
        return Some(host.copy_to_clipboard(text));
    }
    let _ = text;
    None
}

/// Put up the window's card (or take it down) with the next completed grid.
#[cfg(feature = "glass")]
pub(crate) fn set_card(card: Option<angel_glass::card::Card>) {
    if let Some(host) = HOST.get() {
        host.set_card(card);
    }
}
