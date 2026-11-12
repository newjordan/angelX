//! The cockpit in angelX's own window (docs/RENDER_FOUNDATION.md).
//!
//! Built with the `glass` feature and launched with `ANGEL_GLASS=1`, the
//! cockpit draws into a GPU window instead of the terminal. The few things it
//! otherwise writes straight to the terminal (the title, the bell, the
//! clipboard, the game keyboard mode) go to the window here; each helper
//! returns false outside the window so the terminal path runs as before.

#[cfg(feature = "glass")]
use crate::app::control::realm::GameHost;
#[cfg(feature = "glass")]
use angel_plug::glass::GlassArt;

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
    GameHost::window_opened();
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

/// Pictures already handed to the window, by id.
#[cfg(feature = "glass")]
pub(crate) static UPLOADED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u32>>> =
    std::sync::LazyLock::new(Default::default);

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
        drop(groups);
        GameHost::frame_done(&Lent);
    }
    crate::ui::present::frame_done();
}

pub(crate) struct Lent;

impl angel_plug::screen::Console for Lent {
    fn glass_active(&self) -> bool {
        active()
    }

    fn cell_pixels(&self) -> Option<(u16, u16)> {
        cell_pixels()
    }

    fn real_cell_size(&self) -> Option<(u16, u16)> {
        crate::ui::viewer::real_cell_size()
    }

    fn tmux_passthrough(&self) -> bool {
        crate::ui::dots::protocol::tmux_passthrough()
    }

    fn placeholders(
        &self,
        id: u32,
        size: ratatui::layout::Size,
        area: ratatui::layout::Rect,
        buffer: &mut ratatui::buffer::Buffer,
    ) {
        crate::ui::dots::protocol::placeholders(id, size, area, buffer);
    }

    fn store(
        &self,
        key: u64,
        size: (u32, u32),
        span: (u16, u16),
        rgba: Vec<u8>,
        stream: bool,
    ) -> Option<u32> {
        crate::ui::present::store(key, size, span, rgba, stream)
    }

    fn picture(
        &self,
        key: u64,
        size: (u32, u32),
        anchor: ratatui::layout::Rect,
        buffer: &mut ratatui::buffer::Buffer,
        pixels: &mut dyn FnMut() -> Vec<u8>,
    ) -> bool {
        // The one picture asked for here is the world map, whose content moves on every few
        // steps in the same place: it changes in its window picture rather than taking a new one.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ("console picture", anchor.x, anchor.y, anchor.width, anchor.height).hash(&mut hasher);
        crate::ui::present::picture_in_place(hasher.finish(), key, size, anchor, buffer, pixels)
    }

    fn stage_current(&self) -> fn() -> Option<angel_live::telemetry::StageSnapshot> {
        crate::ui::viz::agentviz::current
    }

    fn stage_clear(&self) -> fn() {
        crate::ui::viz::agentviz::clear
    }

    fn repo_identity(&self) -> fn(&std::path::Path) -> angel_plug::screen::Identity {
        |workspace| {
            let identity = crate::platform::workspace_store::repo_identity(workspace);
            angel_plug::screen::Identity {
                root: identity.root,
                key: identity.key,
            }
        }
    }

    #[cfg(feature = "glass")]
    fn host(&self) -> Option<&'static dyn std::any::Any> {
        host().map(|host| host as &'static dyn std::any::Any)
    }

    #[cfg(feature = "glass")]
    fn show_group(&self, group: u32, layers: &mut dyn std::any::Any) {
        if let (Some(host), Some(layers)) = (host(), layers.downcast_mut::<Vec<angel_glass::Layer>>()) {
            show_group(host, group, std::mem::take(layers));
        }
    }

    #[cfg(feature = "glass")]
    fn claim_upload(&self, id: u32) -> bool {
        UPLOADED.lock().unwrap().insert(id)
    }
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
