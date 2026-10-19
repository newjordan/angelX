//! Glass: angelX's own window. A GPU compositor where the cockpit's text is
//! one layer and anything else stacks above or below it
//! (docs/RENDER_FOUNDATION.md).
//!
//! The cockpit keeps drawing with ratatui; [`GlassBackend`] is one more
//! backend for it. [`run`] owns the main thread (winit needs it on macOS) and
//! runs the cockpit on its own thread with a [`Host`]: the backend, the
//! keyboard and mouse as terminal events, and the window's other services.

mod backend;
pub mod scroll;
mod scroll_mesh;
mod blast;
mod clipboard;
mod config;
mod cube;
mod font;
pub mod graphics;
pub mod page;
mod input;
mod layer;
pub mod monolith;
mod probe;
mod render;
mod shore;
mod showcase;
mod squire;
mod wizard;
mod sprites;
mod transition;
mod wallpaper;
mod window;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::event::Event;
use ratatui_core::buffer::Cell;
use winit::event_loop::EventLoopProxy;

pub use backend::GlassBackend;
pub use config::{BrailleStyle, Options, Palette};
pub use font::render_page;
pub use layer::{LAYER_PICTURES, Layer, LayerKind, Particles, Placement, Stand, Tilt};
pub use angel_gpu::Stage;
/// The 3D scene description (`angel-stage`'s), for `Host::set_scene`.
pub use angel_stage::scene;
pub use showcase::{Fit, Herald, Livery, MODEL_ROOT, Mark, SHOWCASE_PICTURE, Showing};
pub use window::run;

/// Locking shared state that survives a panicked holder. The window's
/// shared state is plain data — a grid size, a snapshot slot, layer and
/// picture queues — rebuilt or resent by its producers every frame, so a
/// thread dying mid-update must not poison the session for everyone else:
/// recover the data and go on, the way GPU errors are logged, not fatal.
pub(crate) trait LockExt<T> {
    fn lock_recover(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_recover(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The input reader's error once the window is closed.
pub const WINDOW_CLOSED: &str = "the glass window closed";

/// The most layers one producer set may carry, window layers and a
/// stage's alike. Far beyond anything drawn today (the blast stress
/// scene peaks near twelve thousand); it exists so a buggy or hostile
/// producer cannot make every frame's work unbounded.
pub(crate) const MAX_LAYERS: usize = 65_536;

/// Whether an input error means the window was closed (a normal quit).
pub fn window_closed(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::BrokenPipe && error.to_string() == WINDOW_CLOSED
}

/// The grid that fits the window, and its cell size in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub width_px: u32,
    pub height_px: u32,
    pub cell_width: u32,
    pub cell_height: u32,
}

/// One published frame of the text grid.
#[derive(Default)]
pub(crate) struct Snapshot {
    pub cells: Vec<Cell>,
    pub cols: u16,
    pub rows: u16,
    pub cursor: Option<(u16, u16)>,
    pub scroll: Option<Arc<scroll::Page>>,
    pub revision: u64,
}

/// The latest frame; the render thread takes it when `fresh`.
#[derive(Default)]
pub(crate) struct Slot {
    pub snapshot: Snapshot,
    pub fresh: bool,
}

#[derive(Default)]
pub(crate) struct Shared {
    pub slot: Mutex<Slot>,
    pub grid: Mutex<GridSize>,
    /// The layer list, shared by reference: the window takes the `Arc`
    /// per draw (no copy, however many layers), and a producer mutating
    /// it pays the copy-on-write on its own thread instead.
    pub layers: Mutex<Arc<Vec<Layer>>>,
    pub game_keyboard: AtomicBool,
    pub scroll: Mutex<Option<Arc<scroll::Page>>>,
    pub scroll_changed: AtomicBool,
    /// Pictures handed over (`Host::upload_picture`), waiting for the
    /// render thread.
    pub pictures: Mutex<Vec<graphics::Picture>>,
    /// A picture was handed over since the last flush.
    pub pictures_waiting: AtomicBool,
    /// Stages set (`Some`) or taken down (`None`) since the render thread
    /// last looked, in order, by picture id.
    pub stages: Mutex<Vec<(u32, Option<Stage>)>>,
    /// 3D scenes set (`Some`, with the size of the picture they are drawn
    /// into) or taken down (`None`) since the render thread last looked, by
    /// picture id. Only the latest for an id is kept.
    pub scenes: Mutex<Vec<(u32, Option<(scene::Scene, (u32, u32))>)>>,
    /// The showcase's latest request, waiting for the window.
    pub showcase: Mutex<Option<showcase::Request>>,
    /// The serial of the showing up now; 0 when none is.
    pub showcase_live: std::sync::atomic::AtomicU64,
    pub showcase_serial: std::sync::atomic::AtomicU64,
    pub showcase_clicks: std::sync::atomic::AtomicU32,
    /// A playing showing's place (f32 bits, NaN for none), its pause,
    /// and a seek asked for (`Host::showcase_progress`, `showcase_seek`).
    pub showcase_progress: std::sync::atomic::AtomicU32,
    pub showcase_paused: AtomicBool,
    pub showcase_seek: std::sync::atomic::AtomicU32,
    /// A showcase frame waits to go up, its wake-up already sent.
    pub showcase_frame_waiting: AtomicBool,
    /// Presses of the copy button (`Host::showcase_copies`).
    pub showcase_copies: std::sync::atomic::AtomicU32,
    /// Steps asked for with the panel's arrows (`Host::showcase_steps`).
    pub showcase_steps: std::sync::atomic::AtomicI32,
    /// The next showing's model (`Host::showcase_model`).
    pub showcase_model: Mutex<Option<Vec<scene::Node>>>,
    /// Meshes handed over (`Host::upload_mesh`), waiting for the window.
    pub meshes: Mutex<Vec<(u32, scene::MeshData)>>,
    /// The next showing's marks (`Host::showcase_marks`).
    pub showcase_marks: Mutex<Vec<showcase::Mark>>,
}

pub(crate) enum UserEvent {
    Frame,
    /// Pictures arrived with no change to the cells.
    Pictures,
    Layers,
    /// The showcase was opened, changed or closed (`Host::showcase`).
    Showcase,
    Title(String),
    Attention,
    Capture(std::path::PathBuf),
    /// Film the assembly transition into this directory.
    Film(std::path::PathBuf),
    /// Close as if the window's close button was pressed.
    Close,
    /// The cockpit has returned; end the event loop.
    Exit,
    /// End the process with this code, from the window's thread.
    ExitProcess(i32),
}

/// What the cockpit thread holds: the window's services.
#[derive(Clone)]
pub struct Host {
    shared: Arc<Shared>,
    proxy: Arc<Mutex<EventLoopProxy<UserEvent>>>,
    input: Arc<Mutex<Option<Receiver<Event>>>>,
}

impl Host {
    pub(crate) fn send(&self, event: UserEvent) {
        let _ = self.proxy.lock_recover().send_event(event);
    }

    /// A ratatui backend drawing into this window.
    pub fn backend(&self) -> GlassBackend {
        GlassBackend::new(self.clone())
    }

    /// The window's keyboard and mouse as terminal events, for the cockpit's
    /// input thread. Once the window closes, it returns [`WINDOW_CLOSED`].
    /// There is one reader; a second call gets an error.
    pub fn input_reader(
        &self,
    ) -> io::Result<impl FnMut(Duration) -> io::Result<Option<Event>> + Send + 'static> {
        let events = self
            .input
            .lock_recover()
            .take()
            .ok_or_else(|| io::Error::other("the glass input reader is already taken"))?;
        Ok(move |wait: Duration| match events.recv_timeout(wait) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, WINDOW_CLOSED))
            }
        })
    }

    pub fn grid_size(&self) -> GridSize {
        *self.shared.grid.lock_recover()
    }

    pub fn set_title(&self, title: &str) {
        self.send(UserEvent::Title(title.to_string()));
    }

    /// The window's equivalent of the terminal bell.
    pub fn request_attention(&self) {
        self.send(UserEvent::Attention);
    }

    /// While a game holds the keyboard, modifier keys report on their own and
    /// Shift stays on every key (the terminal's game keyboard mode).
    pub fn set_game_keyboard(&self, active: bool) {
        self.shared.game_keyboard.store(active, Ordering::Relaxed);
    }

    /// Put text on the system clipboard. False when no clipboard tool ran.
    pub fn copy_to_clipboard(&self, text: &str) -> bool {
        clipboard::copy(text)
    }

    /// Queue a foreground menu surface for the next text-grid publication.
    /// No wake here: the backend publishes it atomically with the cockpit frame.
    pub fn set_scroll(&self, page: Option<scroll::Page>) {
        let mut current = self.shared.scroll.lock_recover();
        if current.is_none() && page.is_none() { return; }
        *current = page.map(Arc::new);
        self.shared.scroll_changed.store(true, std::sync::atomic::Ordering::Release);
    }

    /// Add a layer, or replace the one with the same id.
    pub fn set_layer(&self, layer: Layer) {
        {
            let mut shared = self.shared.layers.lock_recover();
            let layers = Arc::make_mut(&mut shared);
            match layers.iter().position(|l| l.id == layer.id) {
                Some(slot) => layers[slot] = layer,
                None if layers.len() < MAX_LAYERS => layers.push(layer),
                None => {}
            }
        }
        self.send(UserEvent::Layers);
    }

    /// Replace every layer of `group` (ids whose top 32 bits are `group`)
    /// with `layers`, in one go: a producer's per-frame set, such as the
    /// Delve's thrown things. An empty list clears the group.
    pub fn set_layer_group(&self, group: u32, layers: Vec<Layer>) {
        {
            let mut shared = self.shared.layers.lock_recover();
            let all = Arc::make_mut(&mut shared);
            all.retain(|l| (l.id >> 32) as u32 != group);
            all.extend(layers.into_iter().map(|mut l| {
                l.id = (u64::from(group) << 32) | (l.id & 0xffff_ffff);
                l
            }));
            all.truncate(MAX_LAYERS);
        }
        self.send(UserEvent::Layers);
    }

    pub fn remove_layer(&self, id: u64) {
        Arc::make_mut(&mut self.shared.layers.lock_recover()).retain(|l| l.id != id);
        self.send(UserEvent::Layers);
    }

    /// End the process from the window's thread once it has released the GPU
    /// and the window. Exiting from any other thread while the window
    /// presents tears the GPU driver down under it.
    pub fn exit_process(&self, code: i32) {
        self.send(UserEvent::ExitProcess(code));
    }

    /// A picture straight onto the GPU side, for a producer in this process:
    /// its pixels as they are. The picture cells naming `picture.id` show it
    /// (`graphics.rs`).
    pub fn upload_picture(&self, picture: graphics::Picture) {
        self.shared.pictures.lock_recover().push(picture);
        // The frame that shows it presents it, once; its flush sends the
        // wake-up (`GlassBackend::flush`).
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Show `stage` in the cells naming its id, drawn by the window every
    /// frame until it is replaced or taken down. Its pictures go up first
    /// (`upload_picture`); the frame that shows it presents it.
    pub fn set_stage(&self, mut stage: Stage) {
        stage.layers.truncate(MAX_LAYERS);
        self.shared
            .stages
            .lock_recover()
            .push((stage.id, Some(stage)));
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Take a stage down.
    pub fn remove_stage(&self, id: u32) {
        self.shared.stages.lock_recover().push((id, None));
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Show a 3D `scene` in the picture `id`, `size` pixels: the window
    /// draws it into its own texture every frame until it is replaced or
    /// taken down, and any layer naming the picture places it (lifted,
    /// turned, over or under the text) as it does any picture. Its pictures
    /// go up first (`upload_picture`). Set it again each frame to animate it;
    /// only the latest set for an id is drawn.
    pub fn set_scene(&self, id: u32, mut scene: scene::Scene, size: (u32, u32)) {
        scene.nodes.truncate(scene::MAX_NODES);
        let mut scenes = self.shared.scenes.lock_recover();
        scenes.retain(|(other, _)| *other != id);
        scenes.push((id, Some((scene, size))));
        drop(scenes);
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Take a scene down.
    pub fn remove_scene(&self, id: u32) {
        let mut scenes = self.shared.scenes.lock_recover();
        scenes.retain(|(other, _)| *other != id);
        scenes.push((id, None));
        drop(scenes);
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Save the next frame as a PNG, drawn offscreen (no desktop behind it).
    pub fn capture(&self, path: impl Into<std::path::PathBuf>) {
        self.send(UserEvent::Capture(path.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_poisoned_lock_recovers_its_data() {
        let shared = Arc::new(Mutex::new(41u32));
        let held = Arc::clone(&shared);
        let _ = std::thread::spawn(move || {
            let mut value = held.lock_recover();
            *value = 42;
            panic!("the holder dies mid-update");
        })
        .join();
        assert_eq!(*shared.lock_recover(), 42, "recovered, not cascaded");
    }
}
