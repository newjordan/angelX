//! Walk the monolith (`angel_glass::monolith`) in a window of its own: the
//! GPU core draws the place as the shots do, and the keys move the knight.
//!
//! cargo run -p angel-glass --example monolith_walk [-- --rows N]
//!
//! W A S D walk (Shift runs), Q E turn the eye about the knight, Z X pull it
//! in and out, R back to the start, Esc leaves.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use angel_glass::monolith::{self, Follow, Walker};
use angel_gpu::{Core, Picture};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    view_format: wgpu::TextureFormat,
    core: Core,
}

struct App {
    rows: u32,
    gpu: Option<Gpu>,
    keys: HashSet<KeyCode>,
    walker: Walker,
    follow: Follow,
    started: Instant,
    last: Instant,
}

fn start() -> Walker {
    monolith::walk(0.0)
}

impl App {
    fn step(&mut self, dt: f32) -> bool {
        let held = |k: KeyCode| self.keys.contains(&k);
        if held(KeyCode::KeyQ) {
            self.follow.orbit -= 1.2 * dt;
        }
        if held(KeyCode::KeyE) {
            self.follow.orbit += 1.2 * dt;
        }
        if held(KeyCode::KeyZ) {
            self.follow.back = (self.follow.back - 18.0 * dt).max(10.0);
            self.follow.up = (self.follow.up - 10.0 * dt).max(4.0);
        }
        if held(KeyCode::KeyX) {
            self.follow.back = (self.follow.back + 18.0 * dt).min(90.0);
            self.follow.up = (self.follow.up + 10.0 * dt).min(50.0);
        }
        // Forward is away from the eye.
        let (mut fwd, mut side) = (0.0f32, 0.0f32);
        if held(KeyCode::KeyW) || held(KeyCode::ArrowUp) {
            fwd += 1.0;
        }
        if held(KeyCode::KeyS) || held(KeyCode::ArrowDown) {
            fwd -= 1.0;
        }
        if held(KeyCode::KeyD) || held(KeyCode::ArrowRight) {
            side += 1.0;
        }
        if held(KeyCode::KeyA) || held(KeyCode::ArrowLeft) {
            side -= 1.0;
        }
        if fwd == 0.0 && side == 0.0 {
            return false;
        }
        let speed = if held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight) { 9.0 } else { monolith::WALK_SPEED };
        let len = (fwd * fwd + side * side).sqrt();
        let (s, c) = self.follow.orbit.sin_cos();
        // Away from the eye is -Z turned by the orbit; right is +X turned.
        let dx = (side * c - fwd * s) / len * speed * dt;
        let dz = (-side * s - fwd * c) / len * speed * dt;
        let heading = dx.atan2(-dz);
        let mut next = Walker { x: self.walker.x + dx, z: self.walker.z + dz, heading };
        if !monolith::may_step(self.walker, next) {
            // Slide along whichever way is open.
            next = Walker { x: self.walker.x + dx, z: self.walker.z, heading };
            if !monolith::may_step(self.walker, next) {
                next = Walker { x: self.walker.x, z: self.walker.z + dz, heading };
                if !monolith::may_step(self.walker, next) {
                    self.walker.heading = heading;
                    return true;
                }
            }
        }
        self.walker = next;
        true
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("angelX: the monolith")
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 810.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("a window"));
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(Arc::clone(&window)).expect("a surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("a GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("monolith-walk"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("a GPU device");
        device.on_uncaptured_error(Arc::new(|error: wgpu::Error| eprintln!("GPU error: {error}")));
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).expect("a surface config");
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(config.format);
        let view_format = format.remove_srgb_suffix();
        config.format = format;
        config.view_formats = if view_format == format { vec![] } else { vec![view_format] };
        config.present_mode = wgpu::PresentMode::Fifo;
        surface.configure(&device, &config);
        let mut core = Core::new(device.clone(), queue.clone(), view_format, 1.0);
        for (id, mesh) in monolith::meshes() {
            if let Err(e) = core.upload_mesh(id, &mesh) {
                eprintln!("mesh {id:#x}: {e}");
            }
        }
        for (id, width, height, rgba) in monolith::sprites() {
            core.upload(Picture { id, width, height, rgba, cols: 0, rows: 0 });
        }
        self.gpu = Some(Gpu { window, surface, config, device, queue, view_format, core });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                eprintln!("close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // By what the key says (a typed or remapped key as well as a
                // plain one), else where it is.
                let code = match &event.logical_key {
                    Key::Named(NamedKey::Escape) => Some(KeyCode::Escape),
                    Key::Named(NamedKey::Shift) => Some(KeyCode::ShiftLeft),
                    Key::Named(NamedKey::ArrowUp) => Some(KeyCode::ArrowUp),
                    Key::Named(NamedKey::ArrowDown) => Some(KeyCode::ArrowDown),
                    Key::Named(NamedKey::ArrowLeft) => Some(KeyCode::ArrowLeft),
                    Key::Named(NamedKey::ArrowRight) => Some(KeyCode::ArrowRight),
                    Key::Character(c) => match c.to_lowercase().as_str() {
                        "w" => Some(KeyCode::KeyW),
                        "a" => Some(KeyCode::KeyA),
                        "s" => Some(KeyCode::KeyS),
                        "d" => Some(KeyCode::KeyD),
                        "q" => Some(KeyCode::KeyQ),
                        "e" => Some(KeyCode::KeyE),
                        "z" => Some(KeyCode::KeyZ),
                        "x" => Some(KeyCode::KeyX),
                        "r" => Some(KeyCode::KeyR),
                        _ => None,
                    },
                    _ => None,
                }
                .or(match event.physical_key {
                    PhysicalKey::Code(code) => Some(code),
                    PhysicalKey::Unidentified(_) => None,
                });
                if std::env::var_os("MONOLITH_KEYS").is_some() {
                    eprintln!("key {:?} {:?} -> {code:?} {:?}", event.logical_key, event.physical_key, event.state);
                }
                if let Some(code) = code {
                    match event.state {
                        ElementState::Pressed => {
                            if code == KeyCode::Escape {
                                event_loop.exit();
                            }
                            if code == KeyCode::KeyR {
                                self.walker = start();
                                self.follow = Follow::default();
                            }
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32().min(0.1);
                self.last = now;
                let walking = self.step(dt);
                let t = self.started.elapsed().as_secs_f32();
                let scene = monolith::scene_stepping(self.walker, self.follow, t, self.rows, walking);
                let Some(gpu) = &mut self.gpu else { return };
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    _ => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        return;
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(gpu.view_format),
                    ..Default::default()
                });
                gpu.core.begin_frame();
                gpu.core.draw_scene(&view, (gpu.config.width, gpu.config.height), &scene, "monolith");
                gpu.window.pre_present_notify();
                gpu.queue.present(frame);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }
}

fn main() {
    let mut rows = monolith::ROWS;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--rows" {
            rows = args.next().and_then(|v| v.parse().ok()).unwrap_or(rows);
        }
    }
    let event_loop = EventLoop::new().expect("an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let now = Instant::now();
    let mut app = App { rows, gpu: None, keys: HashSet::new(), walker: start(), follow: Follow::default(), started: now, last: now };
    event_loop.run_app(&mut app).expect("the event loop");
}
