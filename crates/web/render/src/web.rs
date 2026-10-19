//! The renderer the page drives: a canvas surface around `angel_gpu::Core`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use angel_gpu::{Core, Draw, Frame, Instance, PICTURE, Picture, Stage, target_size};
use angel_stage::wire::{self, MAX_SIDE};
use angel_stage::{Shape, layers_of};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::place;

/// The picture ids the renderer keeps for itself: the stage it draws, the two
/// textures the core keeps beside it while a new scene slides in
/// (`id ^ 0x4000_0000`, `id ^ 0x2000_0000`), the one its 3D props are drawn
/// into (`id ^ 0x1000_0000`), the two a room in 3D is drawn into (its world,
/// `id ^ 0x0800_0000`, and its figures, `id ^ 0x0400_0000`), and the
/// finished frame. A game's
/// ids sit elsewhere (the Delve's from `0x8000_0000`); one that named these
/// would be dropped rather than draw over them.
const STAGE: u32 = 0x1F00_0000;
const FINISHED: u32 = 0x1E00_0000;
const RESERVED: [u32; 7] = [
    STAGE,
    STAGE ^ 0x4000_0000,
    STAGE ^ 0x2000_0000,
    STAGE ^ 0x1000_0000,
    STAGE ^ 0x0800_0000,
    STAGE ^ 0x0400_0000,
    FINISHED,
];

/// Errors the GPU reports without a scope, printed this many times and then
/// counted in silence: a broken draw repeats every frame.
const ERRORS_SHOWN: u32 = 8;

fn console(text: &str) {
    web_sys::console::error_1(&JsValue::from_str(text));
}

#[wasm_bindgen(start)]
fn start() {
    // `panic = "abort"` traps with no message; say it first.
    std::panic::set_hook(Box::new(|info| console(&format!("angel-web-render: {info}"))));
}

/// What a page draws with: one canvas, one `angel-gpu` core.
#[wasm_bindgen]
pub struct Renderer {
    core: Core,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    canvas: HtmlCanvasElement,
    backend: &'static str,
    adapter: String,
    lost: Arc<AtomicBool>,
    reconfigure: bool,
    scale: u8,
    /// The sets a room in 3D was sent (its floor and blocks are made here
    /// from its set, `angel_stage::set::Sets`).
    sets: angel_stage::set::Sets,
    /// Plates a room in 3D was sent as PNGs (`SetRef::png`), each with the
    /// picture it is to be, that the page has not taken yet: it decodes each
    /// with the browser's own decoder (`plate_wanted`, `take_plate`) and
    /// hands its pixels back (`put_picture`). Each item's plate is its own
    /// picture, kept like any other once it is here.
    plates: Vec<(u32, Vec<u8>)>,
    /// The plates the page is decoding: no frame asks for them again meanwhile.
    decoding: Vec<u32>,
}

#[wasm_bindgen]
impl Renderer {
    /// A renderer on `canvas`, which must not have a context yet. `backend` is
    /// `"webgpu"`, `"webgl"` (this build must have wgpu's WebGL2 backend), or
    /// empty for the best this build has. Rejects, with why, when there is no
    /// adapter; the canvas has then possibly been given a context, so a page
    /// that falls back to another way of drawing needs a new canvas.
    pub async fn create(canvas: HtmlCanvasElement, backend: Option<String>) -> Result<Renderer, JsError> {
        if !cfg!(feature = "webgpu") {
            return Err(JsError::new("this renderer was built with no backend (cargo features webgpu, webgl)"));
        }
        let backends = match backend.as_deref().unwrap_or("") {
            "webgpu" => wgpu::Backends::BROWSER_WEBGPU,
            "webgl" if cfg!(feature = "webgl") => wgpu::Backends::GL,
            "webgl" => return Err(JsError::new("this renderer build has no WebGL2 backend")),
            "" => wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            other => return Err(JsError::new(&format!("no backend called {other}"))),
        };
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsError::new(&format!("the canvas has no surface: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("no GPU adapter: {e}")))?;
        let info = adapter.get_info();
        let (backend, limits) = match info.backend {
            wgpu::Backend::BrowserWebGpu => ("webgpu", wgpu::Limits::default()),
            wgpu::Backend::Gl => ("webgl", wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())),
            _ => ("unknown", wgpu::Limits::default()),
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("angel-web-render"),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("no GPU device: {e}")))?;
        let shown = Arc::new(AtomicU32::new(0));
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            let n = shown.fetch_add(1, Ordering::Relaxed);
            if n < ERRORS_SHOWN {
                console(&format!("angel-web-render: GPU error: {error}"));
            }
        }));
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            flag.store(true, Ordering::Relaxed);
            console(&format!("angel-web-render: GPU device lost ({reason:?}): {message}"));
        });

        let caps = surface.get_capabilities(&adapter);
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| JsError::new("the GPU cannot draw to this canvas"))?;
        // Blend in the non-linear space the page's pixels are in: a format
        // without the sRGB curve, drawn through a view of itself.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(config.format);
        let view_format = format.remove_srgb_suffix();
        config.format = format;
        config.view_formats = if view_format == format { vec![] } else { vec![view_format] };
        config.alpha_mode = [wgpu::CompositeAlphaMode::Opaque]
            .into_iter()
            .find(|m| caps.alpha_modes.contains(m))
            .or_else(|| caps.alpha_modes.first().copied())
            .unwrap_or_default();
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);

        let core = Core::new(device.clone(), queue.clone(), view_format, 1.0);
        Ok(Renderer {
            core,
            device,
            queue,
            surface,
            config,
            view_format,
            canvas,
            backend,
            adapter: format!("{} {:?}", info.name, info.device_type).trim().to_string(),
            lost,
            reconfigure: false,
            scale: 0,
            sets: angel_stage::set::Sets::default(),
            plates: Vec::new(),
            decoding: Vec::new(),
        })
    }

    /// `"webgpu"` or `"webgl"`: the backend this renderer drew with.
    pub fn kind(&self) -> String {
        self.backend.to_string()
    }

    /// What the adapter calls itself (a browser may say nothing).
    pub fn adapter(&self) -> String {
        self.adapter.clone()
    }

    /// Whether the GPU device has been lost: nothing draws after that, and the
    /// page should go on with another way of drawing.
    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    /// The canvas now covers `width` x `height` device pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        let most = self.device.limits().max_texture_dimension_2d;
        let (width, height) = (width.clamp(1, most), height.clamp(1, most));
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.reconfigure = false;
    }

    /// The dev panel's scale knob (`place::mode`): 0 whole numbers where they
    /// fill the canvas, 1 whole numbers whenever 1x fits, 2 always fitted.
    /// Returns the mode taken.
    pub fn set_scale(&mut self, mode: u32) -> u32 {
        self.scale = place::mode(mode);
        u32::from(self.scale)
    }

    /// The post pass over the stage (`angel_gpu`'s `post.rs`: bloom,
    /// vignette, grain, a hit's split) on or off, for a weak device or the
    /// player's choice. Returns whether it is on.
    pub fn set_post(&mut self, on: bool) -> bool {
        self.core.set_post(on);
        self.core.post()
    }

    /// Black over the whole canvas.
    pub fn clear(&mut self) {
        self.show(&Frame::default());
    }

    /// Draw one stage frame (`angel_stage::wire`, the bytes the game's
    /// `angel_stage` returns) at `now_ms` on the page's clock, which a new
    /// scene's slide is timed by. Returns 0 when the bytes are not a frame
    /// (nothing is drawn; the page may draw its finished frame instead), 1
    /// when it was drawn, 2 when it was drawn but names a picture the
    /// renderer no longer has (it drops the least recently drawn when the
    /// budget is full): the page should ask the game to send them all again.
    pub fn present_stage(&mut self, bytes: &[u8], now_ms: f64) -> u32 {
        if self.lost() {
            return 0;
        }
        let Ok(wire::Decoded { mut frame, quads }) = wire::decode(bytes) else {
            return 0;
        };
        self.prepare();
        for picture in frame.pictures.drain(..) {
            if RESERVED.contains(&picture.id) {
                continue;
            }
            self.core.upload(Picture {
                id: picture.id,
                width: picture.width,
                height: picture.height,
                rgba: picture.rgba,
                cols: 0,
                rows: 0,
            });
        }
        let mut missing = quads
            .iter()
            .any(|q| q.picture != 0 && q.shape == Shape::Fill && !self.core.has_picture(q.picture));
        // A room in 3D: its floor and blocks made from its set (sent once),
        // its plate and its figures' pictures there too, or all again.
        if let Some(world) = frame.props.as_mut().and_then(|p| p.world.as_mut()) {
            missing |= !self.sets.fill(world);
            // Its plate, as a PNG for the page to decode, when it is not here
            // and not on its way.
            // (The kingdom's chunks: each set its own plate.)
            for set in world.set.iter_mut().chain(world.more.iter_mut()) {
                if let Some(png) = set.png.take()
                    && !self.core.has_picture(set.plate)
                    && !self.decoding.contains(&set.plate)
                    && !self.plates.iter().any(|(id, _)| *id == set.plate)
                {
                    self.plates.push((set.plate, png.to_vec()));
                }
            }
            let named = |n: &angel_stage::scene::Node| match n.material.source {
                angel_stage::scene::TextureSource::Picture(id) => Some(id),
                angel_stage::scene::TextureSource::None => None,
            };
            let (core, plates, decoding) = (&self.core, &self.plates, &self.decoding);
            let coming = |id: u32| decoding.contains(&id) || plates.iter().any(|(p, _)| *p == id);
            missing |= world
                .blocks
                .iter()
                .chain(&world.extra)
                .chain(&world.floor)
                .chain(&world.figures)
                .filter_map(named)
                .any(|id| !core.has_picture(id) && !coming(id));
        }
        // The stage is drawn into a texture the size of the box it is shown
        // in (a cell is one pixel here, and the box is the stage's columns
        // and rows), then that texture is shown, one to one.
        let at = place::fit((self.config.width, self.config.height), frame.size, self.scale);
        let stage = Stage {
            id: STAGE,
            size: frame.size,
            cols: at[2].min(u32::from(u16::MAX)) as u16,
            rows: at[3].min(u32::from(u16::MAX)) as u16,
            layers: layers_of(&quads),
            scene: frame.scene,
            enter: frame.enter,
            tilt: frame.tilt,
            // The frame's 3D props, laid over the quads they name (`at`
            // counts quads, which `layers_of` makes layers one for one).
            props: frame.props.take(),
            impact: frame.impact,
        };
        let size = target_size(&stage, (1, 1));
        self.core.set_stage(STAGE, Some(stage));
        self.core.render_stages((1, 1), now_ms / 1000.0);
        let (cw, ch) = (self.config.width, self.config.height);
        let at = [cw.saturating_sub(size.0) / 2, ch.saturating_sub(size.1) / 2, size.0, size.1];
        self.show_picture(STAGE, size, at);
        if missing { 2 } else { 1 }
    }

    /// The picture a plate the page has not taken yet is to be (0 for none):
    /// the page takes it (`take_plate`), decodes it and puts its pixels back
    /// (`put_picture`), or says it could not (`plate_failed`).
    pub fn plate_wanted(&self) -> u32 {
        self.plates.first().map_or(0, |(id, _)| *id)
    }

    /// The PNG of the plate `plate_wanted` names, which the page decodes;
    /// empty when there is none.
    pub fn take_plate(&mut self) -> Vec<u8> {
        if self.plates.is_empty() {
            return Vec::new();
        }
        let (id, png) = self.plates.remove(0);
        self.decoding.push(id);
        png
    }

    /// A picture's pixels, `width` x `height` straight RGBA, as the page
    /// decoded them (a plate it took). False when they are not that many
    /// bytes, or the picture is one the renderer keeps for itself.
    pub fn put_picture(&mut self, id: u32, width: u32, height: u32, rgba: &[u8]) -> bool {
        self.decoding.retain(|&d| d != id);
        let sided = |v: u32| (1..=MAX_SIDE).contains(&v);
        if self.lost() || RESERVED.contains(&id) || !sided(width) || !sided(height) || rgba.len() != width as usize * height as usize * 4 {
            return false;
        }
        self.core.upload(Picture { id, width, height, rgba: rgba.to_vec(), cols: 0, rows: 0 });
        true
    }

    /// The page could not decode the plate it took for picture `id`: the
    /// next frame that names it asks for everything again.
    pub fn plate_failed(&mut self, id: u32) {
        self.decoding.retain(|&d| d != id);
    }

    /// Draw a finished frame, `width` x `height` RGBA, scaled to the canvas as
    /// the page always has: by the largest whole number that fits, nearest,
    /// black around it. False when `rgba` is not that many bytes.
    pub fn present_frame(&mut self, width: u32, height: u32, rgba: &[u8]) -> bool {
        let sided = |v: u32| (1..=MAX_SIDE).contains(&v);
        if self.lost() || !sided(width) || !sided(height) || rgba.len() != width as usize * height as usize * 4 {
            return false;
        }
        self.prepare();
        self.core.upload(Picture { id: FINISHED, width, height, rgba: rgba.to_vec(), cols: 0, rows: 0 });
        let at = place::fit((self.config.width, self.config.height), (width, height), self.scale);
        self.show_picture(FINISHED, (width, height), at);
        true
    }
}

impl Renderer {
    /// Before a frame: the surface made again if the last said it should be,
    /// and the core told a frame has begun.
    fn prepare(&mut self) {
        if self.reconfigure {
            self.surface.configure(&self.device, &self.config);
            self.reconfigure = false;
        }
        self.core.begin_frame();
    }

    /// Draw picture `id`, `size` pixels, over the box `at` of the canvas
    /// (x, y, width, height), nearest, on black.
    fn show_picture(&mut self, id: u32, size: (u32, u32), at: [u32; 4]) {
        let mut pool = self.core.take_vecs();
        let mut frame = self.core.take_frame();
        let [x, y, w, h] = at.map(|v| v as f32);
        let mut parts = vec![(
            Draw::Picture { id, linear: false },
            vec![Instance {
                rect: [x, y, w, h],
                uv: [0.0, 0.0, size.0 as f32, size.1 as f32],
                color: [1.0; 4],
                kind: PICTURE,
                pivot: [0.0; 2],
                angle: 0.0,
                tilt: [0.0; 4],
                stand: [0.0; 4],
            }],
        )];
        frame.assemble_into(&mut parts, &mut pool);
        self.show(&frame);
        self.core.keep_frame(frame);
        self.core.keep_vecs(pool);
    }

    /// Draw `frame` over black onto the canvas. A surface that is out of date
    /// is made again and tried once more; one that cannot be had this time
    /// (hidden, timed out) skips the frame.
    fn show(&mut self, frame: &Frame) {
        for _ in 0..2 {
            let texture = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                    // Show this frame; make the surface again before the next.
                    self.reconfigure = true;
                    texture
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    self.surface.configure(&self.device, &self.config);
                    continue;
                }
                _ => return,
            };
            let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.view_format),
                ..Default::default()
            });
            let size = (self.config.width, self.config.height);
            self.core.draw(&view, size, wgpu::Color::BLACK, frame, "angel-web-render");
            self.queue.present(texture);
            return;
        }
    }
}
