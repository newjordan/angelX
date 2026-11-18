//! Glass: angelX's own window. A GPU compositor where the cockpit's text is
//! one layer and anything else stacks above or below it
//! (docs/RENDER_FOUNDATION.md).
//!
//! The cockpit keeps drawing with ratatui; [`GlassBackend`] is one more
//! backend for it. [`run`] owns the main thread (winit needs it on macOS) and
//! runs the cockpit on its own thread with a [`Host`]: the backend, the
//! keyboard and mouse as terminal events, and the window's other services.

mod backend;
pub mod card;
mod blast;
mod card_motion;
pub use card_motion::CardMotion;
mod clipboard;
mod config;
mod cube;
mod early;
mod font;
pub mod graphics;
pub mod page;
mod input;
mod layer;
pub mod monolith;
mod probe;
mod record;
#[cfg(feature = "remote")]
pub mod remote;
mod render;
mod shore;
mod showcase;
mod squire;
pub use squire::ShowcaseClipIdentity;
pub mod startup;
mod wizard;
mod sprites;
mod transition;
mod wallpaper;
pub use wallpaper::{load_in_budget, plan_in_budget};
mod window;
mod zones;

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
pub use layer::{LAYER_PICTURES, Layer, LayerKind, MAX_STROKE_PX, Particles, Placement, Stand, Stroke, StrokeShape, Tilt};
pub use angel_gpu::{Stage, PictureProducer};
pub use angel_gpu as gpu;
pub use wgpu;
pub use angel_stage::scene;
pub use showcase::{Fit, Herald, Livery, MODEL_ROOT, Mark, SHOWCASE_PICTURE, Showing};
pub use squire::{ClipStep as ShowcaseClipStep, ShowcaseSquireClip};
pub use window::run;
pub use zones::{PointerEvent, Zone, ZoneKind};

pub(crate) trait LockExt<T> {
    fn lock_recover(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn lock_recover(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub const WINDOW_CLOSED: &str = "the glass window closed";

pub(crate) const MAX_LAYERS: usize = 65_536;

pub fn window_closed(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::BrokenPipe && error.to_string() == WINDOW_CLOSED
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "remote", derive(serde::Serialize, serde::Deserialize))]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub width_px: u32,
    pub height_px: u32,
    pub cell_width: u32,
    pub cell_height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CursorStyle {
    pub kind: CursorKind,
    pub color: Option<[u8; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorKind {
    #[default]
    Block,
    Underline,
    Bar,
}

#[derive(Default)]
pub(crate) struct Snapshot {
    pub game: bool,
    pub cells: Vec<Cell>,
    pub cols: u16,
    pub rows: u16,
    pub cursor: Option<(u16, u16)>,
    pub cursor_style: CursorStyle,
    pub card: Option<Arc<card::Card>>,
    pub revision: u64,
}

#[derive(Default)]
pub(crate) struct Slot {
    pub snapshot: Snapshot,
    pub fresh: bool,
}

#[derive(Default)]
pub(crate) struct Shared {
    pub slot: Mutex<Slot>,
    pub grid: Mutex<GridSize>,
    pub layers: Mutex<Arc<Vec<Layer>>>,
    pub game_keyboard: AtomicBool,
    pub game_surface: AtomicBool,
    pub card: Mutex<Option<Arc<card::Card>>>,
    pub card_changed: AtomicBool,
    pub pictures: Mutex<Vec<graphics::Picture>>,
    pub pictures_waiting: AtomicBool,
    pub freed_pictures: Mutex<Vec<u32>>,
    pub stages: Mutex<Vec<(u32, Option<Stage>)>>,
    pub scenes: Mutex<Vec<(u32, Option<(scene::Scene, (u32, u32))>)>>,
    pub gpu_pictures: Mutex<Vec<(u32, Option<(Box<dyn PictureProducer>, (u32,u32))>)>>,
    pub showcase: Mutex<Option<showcase::Request>>,
    pub showcase_live: std::sync::atomic::AtomicU64,
    pub showcase_serial: std::sync::atomic::AtomicU64,
    pub showcase_clicks: std::sync::atomic::AtomicU32,
    pub showcase_progress: std::sync::atomic::AtomicU32,
    pub showcase_paused: AtomicBool,
    pub showcase_seek: std::sync::atomic::AtomicU32,
    pub showcase_frame_waiting: AtomicBool,
    pub showcase_copies: std::sync::atomic::AtomicU32,
    pub showcase_steps: std::sync::atomic::AtomicI32,
    pub showcase_dismissals: std::sync::atomic::AtomicU32,
    pub showcase_model: Mutex<Option<Vec<scene::Node>>>,
    pub showcase_squire_clip: Mutex<Option<(ShowcaseClipIdentity, Arc<ShowcaseSquireClip>)>>,
    pub meshes: Mutex<Vec<(u32, scene::MeshData)>>,
    pub showcase_marks: Mutex<Vec<showcase::Mark>>,
    pub zones: Mutex<Vec<(u32, Zone)>>,
    pub zone_events: Mutex<Vec<(u32, PointerEvent)>>,
    pub pointer: Mutex<Option<[f32; 2]>>,
    pub frame_nanos: std::sync::atomic::AtomicU64,
}

pub(crate) enum UserEvent {
    Frame,
    Pictures,
    Layers,
    Showcase,
    Title(String),
    Attention,
    Capture(std::path::PathBuf),
    Film(std::path::PathBuf),
    Record(Option<std::time::Instant>),
    GpuReady,
    Close,
    Exit,
    ExitProcess(i32),
    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    Clipboard(String),
}

pub(crate) enum Wake {
    Window(EventLoopProxy<UserEvent>),
    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    Channel(std::sync::mpsc::Sender<UserEvent>),
}

#[derive(Clone)]
pub struct Host {
    shared: Arc<Shared>,
    proxy: Arc<Mutex<Wake>>,
    input: Arc<Mutex<Option<Receiver<Event>>>>,
}

impl Host {
    pub(crate) fn send(&self, event: UserEvent) {
        match &*self.proxy.lock_recover() {
            Wake::Window(proxy) => {
                let _ = proxy.send_event(event);
            }
            Wake::Channel(sender) => {
                let _ = sender.send(event);
            }
        }
    }

    pub fn has_window(&self) -> bool {
        matches!(&*self.proxy.lock_recover(), Wake::Window(_))
    }

    pub fn backend(&self) -> GlassBackend {
        GlassBackend::new(self.clone())
    }

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

    pub fn request_attention(&self) {
        self.send(UserEvent::Attention);
    }

    pub fn set_game_keyboard(&self, active: bool) {
        self.shared.game_keyboard.store(active, Ordering::Relaxed);
    }

    pub fn set_game_surface(&self, active: bool) {
        self.shared.game_surface.store(active, Ordering::Relaxed);
    }

    pub fn showcase_squire_resource(&self) -> Option<(ShowcaseClipIdentity, Arc<ShowcaseSquireClip>)> {
        self.shared.showcase_squire_clip.lock_recover().clone()
    }

    pub fn showcase_squire_clip(&self, identity: Option<ShowcaseClipIdentity>, clip: Option<Arc<ShowcaseSquireClip>>) {
        let mut current = self.shared.showcase_squire_clip.lock_recover();
        let same = match (&*current, &clip) {
            (Some((old, _)), Some(_)) => Some(old) == identity.as_ref(),
            (None, None) => true,
            _ => false,
        };
        if same { return; }
        *current = clip.zip(identity).map(|(clip, identity)| (identity, clip));
        drop(current);
        self.send(UserEvent::Showcase);
    }

    pub fn install_showcase_squire_pack(
        &self,
        module: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        if module.is_empty() || name.is_empty() {
            return Err("squire pack identity must have a module and name".into());
        }
        let clip = Arc::new(ShowcaseSquireClip::from_json_bytes(bytes)?);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        bytes.hash(&mut hash);
        self.showcase_squire_clip(
            Some(ShowcaseClipIdentity {
                module: module.into(),
                name: name.into(),
                generation: 0,
                revision: 0,
                digest: format!("startup-{:016x}", hash.finish()),
            }),
            Some(clip),
        );
        Ok(())
    }

    pub fn copy_to_clipboard(&self, text: &str) -> bool {
        if self.has_window() {
            clipboard::copy(text)
        } else {
            self.send(UserEvent::Clipboard(text.to_string()));
            true
        }
    }

    pub fn set_card(&self, card: Option<card::Card>) {
        let mut current = self.shared.card.lock_recover();
        if current.is_none() && card.is_none() {
            return;
        }
        *current = card.map(Arc::new);
        self.shared.card_changed.store(true, std::sync::atomic::Ordering::Release);
    }

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

    pub fn set_layers(&self, layers: Arc<Vec<Layer>>) {
        *self.shared.layers.lock_recover() = layers;
        self.send(UserEvent::Layers);
    }

    pub fn remove_layer(&self, id: u64) {
        Arc::make_mut(&mut self.shared.layers.lock_recover()).retain(|l| l.id != id);
        self.send(UserEvent::Layers);
    }

    pub fn exit_process(&self, code: i32) {
        self.send(UserEvent::ExitProcess(code));
    }

    pub fn upload_picture(&self, picture: graphics::Picture) {
        self.shared.pictures.lock_recover().push(picture);
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    pub fn free_pictures(&self, ids: impl IntoIterator<Item = u32>) {
        self.shared.freed_pictures.lock_recover().extend(ids);
    }

    pub fn set_stage(&self, mut stage: Stage) {
        stage.layers.truncate(MAX_LAYERS);
        self.shared
            .stages
            .lock_recover()
            .push((stage.id, Some(stage)));
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    pub fn remove_stage(&self, id: u32) {
        self.shared.stages.lock_recover().push((id, None));
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    pub fn set_scene(&self, id: u32, mut scene: scene::Scene, size: (u32, u32)) {
        scene.nodes.truncate(scene::MAX_NODES);
        let mut scenes = self.shared.scenes.lock_recover();
        scenes.retain(|(other, _)| *other != id);
        scenes.push((id, Some((scene, size))));
        drop(scenes);
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    pub fn set_gpu_picture(&self, id:u32, producer: Option<(Box<dyn PictureProducer>, (u32,u32))>) {
        let mut pictures=self.shared.gpu_pictures.lock_recover();
        pictures.retain(|(other,_)|*other!=id);
        pictures.push((id,producer));
        drop(pictures);
        self.shared.pictures_waiting.store(true,Ordering::Release);
    }

    pub fn remove_scene(&self, id: u32) {
        let mut scenes = self.shared.scenes.lock_recover();
        scenes.retain(|(other, _)| *other != id);
        scenes.push((id, None));
        drop(scenes);
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    pub fn set_pointer_zones(&self, group: u32, zones: Vec<Zone>) {
        let mut all = self.shared.zones.lock_recover();
        all.retain(|(g, _)| *g != group);
        all.extend(zones.into_iter().map(|z| (group, z)));
    }

    pub fn take_pointer_events(&self, group: u32) -> Vec<PointerEvent> {
        let mut all = self.shared.zone_events.lock_recover();
        let mut mine = Vec::new();
        all.retain(|(g, e)| {
            if *g == group {
                mine.push(*e);
                false
            } else {
                true
            }
        });
        mine
    }

    pub fn pointer(&self) -> Option<[f32; 2]> {
        *self.shared.pointer.lock_recover()
    }

    pub fn frame_interval(&self) -> std::time::Duration {
        match self.shared.frame_nanos.load(Ordering::Relaxed) {
            0 => std::time::Duration::from_nanos(1_000_000_000 / 60),
            nanos => std::time::Duration::from_nanos(nanos),
        }
    }

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
