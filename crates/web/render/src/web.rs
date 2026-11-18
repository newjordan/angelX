//! The renderer the page drives: a canvas surface around `angel_gpu::Core`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use angel_gpu::{Core, Draw, Frame, Instance, PICTURE, Picture, Stage, target_size};
use angel_stage::wire::{self, MAX_SIDE};
use angel_stage::scene::MeshRef;
use angel_stage::{Shape, layers_of};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::place;

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

const ERRORS_SHOWN: u32 = 8;

fn console(text: &str) {
    web_sys::console::error_1(&JsValue::from_str(text));
}

#[wasm_bindgen(start)]
fn start() {
    std::panic::set_hook(Box::new(|info| console(&format!("angel-web-render: {info}"))));
}

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
    sets: angel_stage::set::Sets,
    plates: Vec<(u32, Vec<u8>)>,
    decoding: Vec<u32>,
}

#[wasm_bindgen]
impl Renderer {
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

    pub fn kind(&self) -> String {
        self.backend.to_string()
    }

    pub fn adapter(&self) -> String {
        self.adapter.clone()
    }

    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

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

    pub fn set_scale(&mut self, mode: u32) -> u32 {
        self.scale = place::mode(mode);
        u32::from(self.scale)
    }

    pub fn set_post(&mut self, on: bool) -> bool {
        self.core.set_post(on);
        self.core.post()
    }

    pub fn clear(&mut self) {
        self.show(&Frame::default());
    }

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
        for (id, mesh) in frame.meshes.drain(..) {
            let _ = self.core.upload_mesh(id, &mesh);
        }
        let lost_mesh = frame.props.as_ref().is_some_and(|props| {
            props.nodes.iter().any(|n| matches!(n.mesh, Some(MeshRef::Uploaded(id)) if !self.core.has_mesh(id)))
        });
        let mut missing = lost_mesh
            || quads
            .iter()
            .any(|q| q.picture != 0 && q.shape == Shape::Fill && !self.core.has_picture(q.picture));
        if let Some(world) = frame.props.as_mut().and_then(|p| p.world.as_mut()) {
            missing |= !self.sets.fill(world);
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

    pub fn plate_wanted(&self) -> u32 {
        self.plates.first().map_or(0, |(id, _)| *id)
    }

    pub fn take_plate(&mut self) -> Vec<u8> {
        if self.plates.is_empty() {
            return Vec::new();
        }
        let (id, png) = self.plates.remove(0);
        self.decoding.push(id);
        png
    }

    pub fn put_picture(&mut self, id: u32, width: u32, height: u32, rgba: &[u8]) -> bool {
        self.decoding.retain(|&d| d != id);
        let sided = |v: u32| (1..=MAX_SIDE).contains(&v);
        if self.lost() || RESERVED.contains(&id) || !sided(width) || !sided(height) || rgba.len() != width as usize * height as usize * 4 {
            return false;
        }
        self.core.upload(Picture { id, width, height, rgba: rgba.to_vec(), cols: 0, rows: 0 });
        true
    }

    pub fn plate_failed(&mut self, id: u32) {
        self.decoding.retain(|&d| d != id);
    }

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
    fn prepare(&mut self) {
        if self.reconfigure {
            self.surface.configure(&self.device, &self.config);
            self.reconfigure = false;
        }
        self.core.begin_frame();
    }

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

    fn show(&mut self, frame: &Frame) {
        for _ in 0..2 {
            let texture = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => texture,
                wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
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
