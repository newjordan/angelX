//! The compositor: turns a grid snapshot and the layer list into instanced
//! quads and presents them on a transparent surface. The GPU half (the
//! pipeline, pictures, stages, atlas and layer compiler) is `angel-gpu`'s;
//! this is the window's side: the surface, the fonts and glyph memo, the
//! text grid and the assembly transition.
//!
//! Draw order, back to front: the window background (default background at
//! its opacity), layers below the text, cell backgrounds, glyphs, underlines
//! and the cursor, layers above the text.

use std::sync::Arc;
use std::time::Instant;

use angel_gpu::{COLOR, Core, Draw, Entry, FastMap, Frame, Instance, MASK, PICTURE, Stage};
use bytemuck::Zeroable;
use ratatui_core::style::{Color, Modifier};
use winit::window::Window;

use crate::config::{BrailleStyle, Options, Palette, Rgb};
use crate::font::{BOLD, CellMetrics, Fonts, ITALIC, Picture};
use crate::layer::Layer;
use crate::{GridSize, Snapshot, graphics, sprites, transition};

const CHARS_MAX: usize = 65_536;
const STRINGS_MAX: usize = 16_384;

const PICTURE_GUARD_MASK: u16 = 0x40 | 0x80 | 0x08 | 0x100;

fn insert_capped<K: std::hash::Hash + Eq, V>(
    map: &mut FastMap<K, V>,
    key: K,
    value: V,
    max: usize,
) {
    if map.len() >= max && !map.contains_key(&key) {
        map.clear();
    }
    map.insert(key, value);
}

fn prune_picture_buffers(
    buffers: &mut std::collections::BTreeMap<Draw, Vec<Instance>>,
    live: impl Fn(u32) -> bool,
) {
    buffers.retain(|draw, _| match draw {
        Draw::Picture { id, .. } => live(*id),
        _ => true,
    });
}

fn symbol_hash(symbol: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = angel_gpu::FxHasher::default();
    symbol.hash(&mut hasher);
    hasher.finish()
}

pub struct Renderer {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    opaque_only: bool,
    core: Core,
    transition: Option<transition::Transition>,
    handoff: Option<(Instant, u32)>,
    reconfigure: bool,
    pub timing: [std::time::Duration; 4],
    spare_parts: Vec<(Draw, Vec<Instance>)>,
    spare_pictures: std::collections::BTreeMap<Draw, Vec<Instance>>,
    ascii_chars: Box<[[Option<Option<Entry>>; 95]; 4]>,
    sprite_chars: Box<[Option<Option<Entry>>; 416]>,
    chars: FastMap<(char, u8, u8), Option<Entry>>,
    chars_mru: [Option<((char, u8, u8), Option<Entry>)>; 2],
    strings: FastMap<(u64, u8, u8), (Box<str>, Option<Entry>)>,
    indexed_lut: [Rgb; 256],
    fonts: Fonts,
    card_show: Option<CardShow>,
    card_art: CardArt,
    card_squire_identity: Option<crate::squire::ShowcaseClipIdentity>,
    card_squire_clip: Option<Arc<crate::ShowcaseSquireClip>>,
    font_family: String,
    palette: Palette,
    opacity: f32,
    padding: f32,
    braille: BrailleStyle,
    font_points: f32,
    base_points: f32,
    scale: f32,
    pub overlay: Vec<Layer>,
    pub adapter_name: String,
    pub texture_budget: u32,
    pub present_mode: wgpu::PresentMode,
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    fn open(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<Self, String> {
        let power_preference = match std::env::var("ANGEL_GLASS_GPU").as_deref() {
            Ok("low") => wgpu::PowerPreference::LowPower,
            Ok("high") => wgpu::PowerPreference::HighPerformance,
            _ => wgpu::PowerPreference::HighPerformance,
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            force_fallback_adapter: false,
            compatible_surface: surface,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter for the window: {e}"))?;
        crate::startup::mark("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glass-device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Manual { suballocated_device_memory_block_size: (4 << 20)..(4 << 20) },
            ..Default::default()
        }))
        .map_err(|e| format!("GPU device: {e}"))?;
        crate::startup::mark("device");
        Ok(Self { instance, adapter, device, queue })
    }
}

pub struct Preparing {
    gpu: Ready<Result<Gpu, String>>,
    gpu_done: Arc<Signal>,
    fonts: Option<Ready<Result<Fonts, String>>>,
    sized: Option<(Fonts, f32)>,
    wallpaper: Ready<Option<crate::wallpaper::Wallpaper>>,
}

#[derive(Default)]
struct Signal {
    state: std::sync::Mutex<(bool, Option<Box<dyn FnOnce() + Send>>)>,
}

impl Signal {
    fn done(&self) {
        let wake = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.0 = true;
            state.1.take()
        };
        if let Some(wake) = wake {
            wake();
        }
    }

    fn when_done(&self, wake: Box<dyn FnOnce() + Send>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.0 {
            return false;
        }
        state.1 = Some(wake);
        true
    }
}

enum Ready<T> {
    Thread(std::thread::JoinHandle<Option<T>>),
    Done(Option<T>),
}

impl<T: Send + 'static> Ready<T> {
    fn start(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let slot = Arc::new(std::sync::Mutex::new(Some(work)));
        let theirs = Arc::clone(&slot);
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || theirs.lock().unwrap_or_else(|e| e.into_inner()).take().map(|work| work()));
        match spawned {
            Ok(thread) => Self::Thread(thread),
            Err(_) => Self::Done(slot.lock().unwrap_or_else(|e| e.into_inner()).take().map(|work| work())),
        }
    }

    fn wait(self) -> Option<T> {
        match self {
            Self::Thread(thread) => thread.join().ok().flatten(),
            Self::Done(value) => value,
        }
    }

    fn get(&mut self) -> Option<&T> {
        if matches!(self, Self::Thread(_)) {
            let Self::Thread(thread) = std::mem::replace(self, Self::Done(None)) else {
                return None;
            };
            *self = Self::Done(thread.join().ok().flatten());
        }
        match self {
            Self::Done(value) => value.as_ref(),
            Self::Thread(_) => None,
        }
    }
}

impl Preparing {
    pub fn start(options: &Options) -> Self {
        let gpu_done = Arc::new(Signal::default());
        let signal = Arc::clone(&gpu_done);
        let gpu = Ready::start("gpu-open", move || {
            let instance = wgpu::Instance::default();
            crate::startup::mark("instance");
            let gpu = Gpu::open(instance, None);
            signal.done();
            gpu
        });
        let (family, points) = (options.font_family.clone(), options.font_size);
        let fonts = Ready::start("font-open", move || {
            let fonts = Fonts::load(&family, points_to_px(points, 1.0), points_to_px(1.0, 1.0));
            crate::startup::mark("fonts");
            fonts
        });
        let settings = options.wallpaper();
        let wallpaper = Ready::start("wallpaper-open", move || {
            let image = settings.and_then(|settings| crate::wallpaper::load_in_budget(&settings, graphics::picture_side_budget(None)));
            crate::startup::mark("wallpaper decoded");
            image
        });
        Self { gpu, gpu_done, fonts: Some(fonts), sized: None, wallpaper }
    }

    pub fn when_gpu_ready(&self, wake: impl FnOnce() + Send + 'static) -> bool {
        self.gpu_done.when_done(Box::new(wake))
    }

    pub fn pixels_for(&mut self, options: &Options, scale: f32, cols: u16, rows: u16) -> Result<(u32, u32), String> {
        let cell = self.cell(options, scale)?;
        let pad = (options.padding * scale).round() as u32 * 2;
        Ok((u32::from(cols) * cell.width + pad, u32::from(rows) * cell.height + pad))
    }

    pub fn grid_for(&mut self, options: &Options, scale: f32, width: u32, height: u32) -> Result<GridSize, String> {
        let cell = self.cell(options, scale)?;
        let pad = (options.padding * scale).round() * 2.0;
        let cols = ((width as f32 - pad) / cell.width as f32).floor().max(1.0);
        let rows = ((height as f32 - pad) / cell.height as f32).floor().max(1.0);
        Ok(GridSize {
            cols: cols as u16,
            rows: rows as u16,
            width_px: width,
            height_px: height,
            cell_width: cell.width,
            cell_height: cell.height,
        })
    }

    fn cell(&mut self, options: &Options, scale: f32) -> Result<CellMetrics, String> {
        let fonts = self.fonts(options, scale)?;
        let cell = fonts.cell;
        self.sized = Some((fonts, scale));
        Ok(cell)
    }

    pub fn early_parts(&mut self) -> (Option<&crate::wallpaper::Wallpaper>, Option<&mut Fonts>) {
        let wallpaper = self.wallpaper.get().and_then(Option::as_ref);
        (wallpaper, self.sized.as_mut().map(|(fonts, _)| fonts))
    }

    fn fonts(&mut self, options: &Options, scale: f32) -> Result<Fonts, String> {
        let mut fonts = match self.sized.take() {
            Some((fonts, sized)) if sized == scale => return Ok(fonts),
            Some((fonts, _)) => fonts,
            None => self
                .fonts
                .take()
                .ok_or("the fonts were already taken")?
                .wait()
                .ok_or("the font thread failed")??,
        };
        fonts.set_size(points_to_px(options.font_size, scale), points_to_px(1.0, scale));
        Ok(fonts)
    }
}

impl Renderer {
    pub fn new(window: Arc<Window>, options: &Options, mut preparing: Preparing) -> Result<Self, String> {
        let scale = window.scale_factor() as f32;
        let fonts = preparing.fonts(options, scale)?;
        let gpu = preparing.gpu.wait().ok_or("the GPU thread failed")??;
        let surface = gpu
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| format!("window surface: {e}"))?;
        crate::startup::mark("surface");
        let Gpu { instance, adapter, device, queue } = if gpu.adapter.is_surface_supported(&surface) {
            gpu
        } else {
            Gpu::open(gpu.instance.clone(), Some(&surface))?
        };
        drop(instance);
        let info = adapter.get_info();
        device.on_uncaptured_error(std::sync::Arc::new(|error: wgpu::Error| {
            eprintln!("GLASS: GPU error: {error}");
        }));

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("the GPU cannot draw to this window")?;
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        let view_format = format.remove_srgb_suffix();
        config.format = format;
        config.view_formats = if view_format == format {
            vec![]
        } else {
            vec![view_format]
        };
        let alpha_mode = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ]
        .into_iter()
        .find(|m| caps.alpha_modes.contains(m))
        .unwrap_or(caps.alpha_modes[0]);
        config.alpha_mode = alpha_mode;
        let wanted = match std::env::var("ANGEL_GLASS_PRESENT").as_deref() {
            Ok("fifo") => wgpu::PresentMode::Fifo,
            Ok("immediate") => wgpu::PresentMode::Immediate,
            _ => wgpu::PresentMode::Mailbox,
        };
        let candidates: &[wgpu::PresentMode] = if wanted == wgpu::PresentMode::Fifo {
            &[wgpu::PresentMode::Fifo]
        } else {
            &[
                wanted,
                wgpu::PresentMode::Mailbox,
                wgpu::PresentMode::Immediate,
                wgpu::PresentMode::Fifo,
            ]
        };
        config.present_mode = candidates
            .iter()
            .copied()
            .find(|mode| caps.present_modes.contains(mode))
            .unwrap_or(wgpu::PresentMode::Fifo);
        if config.present_mode != wanted {
            eprintln!(
                "GLASS: present mode {wanted:?} unavailable, using {:?}",
                config.present_mode
            );
        }
        config.desired_maximum_frame_latency = std::env::var("ANGEL_GLASS_FRAME_LATENCY")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|n| (1..=4).contains(n))
            .unwrap_or(1);
        surface.configure(&device, &config);
        crate::startup::mark("configured");
        let present_mode = config.present_mode;
        let mut core = Core::new(device.clone(), queue.clone(), view_format, options.text_gamma);
        crate::startup::mark("core");
        let early_wallpaper = preparing.wallpaper.wait().flatten();
        let side_budget = graphics::picture_side_budget(Some(&info.name));
        let wallpaper = if side_budget > graphics::picture_side_budget(None) {
            options.wallpaper().and_then(|settings| crate::wallpaper::load_in_budget(&settings, side_budget))
        } else {
            early_wallpaper
        };
        if let Some(image) = wallpaper {
            core.set_wallpaper(image);
        }
        crate::startup::mark("wallpaper");
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            view_format,
            opaque_only: alpha_mode == wgpu::CompositeAlphaMode::Opaque,
            core,
            transition: None,
            handoff: None,
            reconfigure: false,
            timing: Default::default(),
            spare_parts: Vec::new(),
            spare_pictures: Default::default(),
            ascii_chars: Box::new([[None; 95]; 4]),
            sprite_chars: Box::new([None; 416]),
            chars: FastMap::default(),
            chars_mru: [None, None],
            strings: FastMap::default(),
            indexed_lut: indexed_table(&options.palette.ansi),
            fonts,
            card_show: None,
            card_art: CardArt::default(),
            card_squire_identity: None,
            card_squire_clip: None,
            font_family: options.font_family.clone(),
            palette: options.palette.clone(),
            opacity: options.background_opacity,
            padding: options.padding,
            braille: options.braille,
            font_points: options.font_size,
            base_points: options.font_size,
            scale,
            overlay: Vec::new(),
            adapter_name: format!("{} ({:?})", info.name, info.backend),
            texture_budget: graphics::picture_side_budget(Some(&info.name)),
            present_mode,
            alpha_mode,
        })
    }

    pub fn cell(&self) -> CellMetrics {
        self.fonts.cell
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    fn padding_px(&self) -> f32 {
        (self.padding * self.scale).round()
    }

    fn origin(&self) -> (f32, f32) {
        let grid = self.grid_size();
        let pad = self.padding_px();
        let spare = |window: u32, cells: u16, size: u32| {
            ((window as f32 - 2.0 * pad - f32::from(cells) * size as f32) / 2.0)
                .max(0.0)
                .floor()
        };
        (
            pad + spare(grid.width_px, grid.cols, grid.cell_width),
            pad + spare(grid.height_px, grid.rows, grid.cell_height),
        )
    }

    pub fn grid_size(&self) -> GridSize {
        let cell = self.fonts.cell;
        let pad = self.padding_px() * 2.0;
        let cols = ((self.config.width as f32 - pad) / cell.width as f32)
            .floor()
            .max(1.0);
        let rows = ((self.config.height as f32 - pad) / cell.height as f32)
            .floor()
            .max(1.0);
        GridSize {
            cols: cols as u16,
            rows: rows as u16,
            width_px: self.config.width,
            height_px: self.config.height,
            cell_width: cell.width,
            cell_height: cell.height,
        }
    }

    pub fn cell_at(&self, x: f64, y: f64) -> (u16, u16) {
        let grid = self.grid_size();
        let (ox, oy) = self.origin();
        let col = ((x - f64::from(ox)) / f64::from(grid.cell_width))
            .floor()
            .max(0.0) as u16;
        let row = ((y - f64::from(oy)) / f64::from(grid.cell_height))
            .floor()
            .max(0.0) as u16;
        (col.min(grid.cols - 1), row.min(grid.rows - 1))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Some((_, id)) = self.handoff.take() {
            self.core.drop_frozen_frame(id);
        }
        self.config.width = width;
        self.config.height = height;
        self.reconfigure = true;
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.apply_font_size();
    }

    pub fn zoom(&mut self, steps: f32) {
        self.font_points = if steps == 0.0 {
            self.base_points
        } else {
            (self.font_points + steps).clamp(4.0, 96.0)
        };
        self.apply_font_size();
    }

    fn apply_font_size(&mut self) {
        self.fonts.set_size(
            points_to_px(self.font_points, self.scale),
            points_to_px(1.0, self.scale),
        );
        self.reset_glyphs();
    }

    fn reset_glyphs(&mut self) {
        self.ascii_chars.fill([None; 95]);
        self.sprite_chars.fill(None);
        self.chars.clear();
        self.chars_mru = [None, None];
        self.strings.clear();
        self.core.reset_atlas();
    }

    fn upload_glyph(&mut self, picture: Option<Picture>) -> Option<Entry> {
        match picture {
            Some(p) if p.width > 0 && p.height > 0 => {
                let rgba: Vec<u8> = if p.colored {
                    p.data
                } else {
                    p.data.iter().flat_map(|&a| [a, a, a, a]).collect()
                };
                match self.core.atlas_insert(p.width, p.height, &rgba) {
                    Some([x, y]) => Some(Entry {
                        uv: [x as f32, y as f32, p.width as f32, p.height as f32],
                        left: p.left as f32,
                        top: p.top as f32,
                        colored: p.colored,
                    }),
                    None => {
                        self.core.atlas_insert_failed();
                        None
                    }
                }
            }
            _ => None,
        }
    }

    #[inline]
    fn glyph(&mut self, symbol: &str, style: u8, span: u8, slen: usize, b0: u8) -> Option<Entry> {
        if slen == 1 {
            let b = b0;
            let d = b.wrapping_sub(0x20);
            debug_assert!(span == 1, "1-byte glyph always span 1");
            debug_assert!(style < 4, "style is BOLD|ITALIC only");
            if d < 0x5f {
                let slot = d as usize;
                if let Some(found) = self.ascii_chars[style as usize][slot] {
                    return found;
                }
                let picture = self.fonts.draw(symbol, style, 1);
                let before_full = self.core.atlas_full();
                let entry = self.upload_glyph(picture);
                if !self.core.atlas_full() || before_full {
                    self.ascii_chars[style as usize][slot] = Some(entry);
                }
                return entry;
            }
        } else if span == 1
            && let Some((slot, ch)) = sprite_slot(symbol)
        {
            if let Some(found) = self.sprite_chars[slot] {
                return found;
            }
            let cell = self.fonts.cell;
            let picture = sprites::draw(ch, &cell, self.braille).map(|mask| Picture {
                width: cell.width,
                height: cell.height,
                left: 0,
                top: 0,
                colored: false,
                data: mask,
            });
            let before_full = self.core.atlas_full();
            let entry = self.upload_glyph(picture);
            if !self.core.atlas_full() || before_full {
                self.sprite_chars[slot] = Some(entry);
            }
            return entry;
        }
        let mut chars = symbol.chars();
        let first = chars.next()?;
        let single = chars.next().is_none();
        if single {
            let key = (first, style, span);
            if self.chars_mru[0].is_some_and(|(k, _)| k == key) {
                return self.chars_mru[0].unwrap().1;
            }
            if self.chars_mru[1].is_some_and(|(k, _)| k == key) {
                let hit = self.chars_mru[1].unwrap();
                self.chars_mru[1] = self.chars_mru[0];
                self.chars_mru[0] = Some(hit);
                return hit.1;
            }
            if let Some(found) = self.chars.get(&key) {
                let found = *found;
                self.chars_mru[1] = self.chars_mru[0];
                self.chars_mru[0] = Some((key, found));
                return found;
            }
        } else if let Some((stored, found)) =
            self.strings.get(&(symbol_hash(symbol), style, span))
            && &**stored == symbol
        {
            return *found;
        }
        let cell = self.fonts.cell;
        let picture = single
            .then(|| sprites::draw(first, &cell, self.braille))
            .flatten()
            .map(|mask| Picture {
                width: cell.width,
                height: cell.height,
                left: 0,
                top: 0,
                colored: false,
                data: mask,
            })
            .or_else(|| self.fonts.draw(symbol, style, u32::from(span)));
        let before_full = self.core.atlas_full();
        let entry = self.upload_glyph(picture);
        if self.core.atlas_full() && !before_full {
            return None;
        }
        if single {
            insert_capped(&mut self.chars, (first, style, span), entry, CHARS_MAX);
            self.chars_mru[1] = self.chars_mru[0];
            self.chars_mru[0] = Some(((first, style, span), entry));
        } else {
            insert_capped(
                &mut self.strings,
                (symbol_hash(symbol), style, span),
                (Box::from(symbol), entry),
                STRINGS_MAX,
            );
        }
        entry
    }

    fn rgb(&self, color: Color, fallback: Rgb) -> Rgb {        let ansi = &self.palette.ansi;
        match color {
            Color::Reset => fallback,
            Color::Black => ansi[0],
            Color::Red => ansi[1],
            Color::Green => ansi[2],
            Color::Yellow => ansi[3],
            Color::Blue => ansi[4],
            Color::Magenta => ansi[5],
            Color::Cyan => ansi[6],
            Color::Gray => ansi[7],
            Color::DarkGray => ansi[8],
            Color::LightRed => ansi[9],
            Color::LightGreen => ansi[10],
            Color::LightYellow => ansi[11],
            Color::LightBlue => ansi[12],
            Color::LightMagenta => ansi[13],
            Color::LightCyan => ansi[14],
            Color::White => ansi[15],
            Color::Rgb(r, g, b) => [r, g, b],
            Color::Indexed(i) => self.indexed_lut[usize::from(i)],
        }
    }

    fn build(&mut self, snapshot: &Snapshot, layers: &[Layer], focused: bool, out: &mut Frame) {
        self.core.begin_frame();
        let now = Instant::now();
        if self.transition.is_some_and(|t| t.finished(now)) {
            self.transition = None;
        }
        let moment = self.transition.map(|t| t.time(now));
        let mut pool = self.core.take_vecs();
        let mut tiles = pool.pop().unwrap_or_default();
        let mut dust = pool.pop().unwrap_or_default();
        let mut pictures = std::mem::take(&mut self.spare_pictures);
        prune_picture_buffers(&mut pictures, |id| self.core.has_picture(id));
        let cell = self.fonts.cell;
        let (cw, ch) = (cell.width as f32, cell.height as f32);
        let (ox, oy) = self.origin();
        let cols = usize::from(snapshot.cols);
        let rows = usize::from(snapshot.rows);
        let foreground = self.palette.foreground;
        let background = self.palette.background;
        let cursor = self.palette.cursor;
        let cursor_text = self.palette.cursor_text;
        let cursor_kind = snapshot.cursor_style.kind;
        let cursor_color = snapshot.cursor_style.color.unwrap_or(cursor);
        let cursor_block_active = focused && cursor_kind == crate::CursorKind::Block;
        let cursor_thin_active = focused && cursor_kind != crate::CursorKind::Block;
        let underline_y = cell.underline_y as f32;
        let strike_y = cell.strike_y as f32;
        let underline_h = cell.underline_height as f32;
        let reversed_bit = Modifier::REVERSED.bits();
        let hidden_bit = Modifier::HIDDEN.bits();
        let dim_bit = Modifier::DIM.bits();
        let bold_bit = Modifier::BOLD.bits();
        let italic_bit = Modifier::ITALIC.bits();
        let underlined_bit = Modifier::UNDERLINED.bits();
        let crossed_bit = Modifier::CROSSED_OUT.bits();
        let mut backgrounds = pool.pop().unwrap_or_default();
        let mut glyphs = pool.pop().unwrap_or_default();
        glyphs.reserve(cols * rows / 2);
        let mut overlays = pool.pop().unwrap_or_default();
        let (cursor_col, cursor_row) = match snapshot.cursor {
            Some((c, r)) => (usize::from(c), usize::from(r)),
            None => (usize::MAX, usize::MAX),
        };
        let frame_rest = moment.is_none();
        let mut fg_memo: Option<(Color, Option<Color>, Rgb, u32, [f32; 4])> = None;
        for row in 0..rows {
            let y = oy + row as f32 * ch;
            let row_base = row * cols;
            let row_is_cursor = row == cursor_row;
            let mut run: Option<(usize, usize, Color)> = None;
            let avail = if row_base < snapshot.cells.len() {
                (snapshot.cells.len() - row_base).min(cols)
            } else {
                0
            };
            let row_cells = if avail > 0 {
                &snapshot.cells[row_base..row_base + avail]
            } else {
                &[]
            };
            let mut col = 0;
            while col < avail {
                let cell_data = unsafe { &*row_cells.as_ptr().add(col) };
                let symbol = cell_data.symbol();
                let sbytes = symbol.as_bytes();
                let slen = sbytes.len();
                let b0 = if slen == 0 { 0 } else { sbytes[0] };
                let modifier = cell_data.modifier;
                let mbits = modifier.bits();
                let at_cursor = row_is_cursor && col == cursor_col;
                let (picture, mut span) = if slen == 1 {
                    (None, 1)
                } else if slen == 0 || b0 != 0xF4 {
                    let fast_w1 = slen == 0
                        || (slen == 2
                            && matches!(b0, 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA))
                        || unsafe { slen == 3
                            && b0 == 0xE2
                            && (matches!(*sbytes.as_ptr().wrapping_add(1), 0x94 | 0x95 | 0xA0..=0xA3)
                                || (*sbytes.as_ptr().wrapping_add(1) == 0x96 && *sbytes.as_ptr().wrapping_add(2) < 0xA0)) };
                    let fast_w2 =
                        slen == 3 && matches!(b0, 0xE4..=0xE9);
                    (
                        None,
                        if fast_w1 {
                            1
                        } else if fast_w2 {
                            2.min(cols - col)
                        } else {
                            crate::backend::symbol_width(symbol).clamp(1, 2).min(cols - col)
                        },
                    )
                } else {
                    let picture =
                        graphics::read_placeholder(symbol, cell_data.fg);
                    let span = if picture.is_some() {
                        1
                    } else {
                        crate::backend::symbol_width(symbol).clamp(1, 2).min(cols - col)
                    };
                    (picture, span)
                };
                if let Some(p0) = picture
                    && frame_rest
                    && !at_cursor
                    && mbits & PICTURE_GUARD_MASK == 0
                {
                    while col + span < cols {
                        let next_col = col + span;
                        if row_is_cursor && next_col == cursor_col {
                            break;
                        }
                        if next_col >= avail {
                            break;
                        }
                        let next_cell = unsafe { &*row_cells.as_ptr().add(next_col) };
                        if next_cell.fg != cell_data.fg
                            || next_cell.bg != cell_data.bg
                            || next_cell.modifier != modifier
                        {
                            break;
                        }
                        let Some(want_col) = p0.col.checked_add(span as u16) else {
                            break;
                        };
                        if graphics::read_placeholder(next_cell.symbol(), next_cell.fg)
                            != Some(graphics::Placeholder {
                                image: p0.image,
                                row: p0.row,
                                col: want_col,
                            })
                        {
                            break;
                        }
                        span += 1;
                    }
                }
                let next_col = col + span;
                let (bg_color, fg_rev): (Option<Color>, Option<Color>) =
                    if (mbits & reversed_bit) == 0 {
                        (
                            match cell_data.bg {
                                Color::Reset => None,
                                other => Some(other),
                            },
                            None,
                        )
                    } else {
                        (Some(cell_data.fg), Some(cell_data.bg))
                    };
                let motion = if frame_rest {
                    None
                } else {
                    let t = unsafe { moment.unwrap_unchecked() };
                    let wall = !(slen == 1 && b0 == b' ')
                        && is_box_drawing(symbol);
                    let mo = transition::motion(
                        t,
                        transition::Place {
                            col: col as u16,
                            row: row as u16,
                            cols: cols as u16,
                            rows: rows as u16,
                            cell_height: ch,
                            wall,
                        },
                    );
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    let cr = [x, y, w, ch];
                    stone_tile(
                        &mut tiles,
                        &mut dust,
                        mo,
                        cr,
                        background,
                        (col as u16, row as u16),
                        wall,
                    );
                    Some(mo)
                };
                if cursor_block_active && at_cursor {
                    if let Some((s, e, k)) = run.take() {
                        let rk = match k {
                            Color::Reset => foreground,
                            _ => self.rgb(k, background),
                        };
                        backgrounds.push(Instance::solid(
                            ox + s as f32 * cw,
                            y,
                            (e - s) as f32 * cw,
                            ch,
                            premultiply(rk, 1.0),
                        ));
                    }
                    let rgb = cursor_color;
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    if frame_rest {
                        backgrounds.push(Instance::solid(x, y, w, ch, premultiply(rgb, 1.0)));
                    } else {
                        backgrounds.push(moved(
                            Instance::solid(x, y, w, ch, premultiply(rgb, 1.0)),
                            motion.unwrap_or(transition::Motion::REST),
                            [x, y, w, ch],
                        ));
                    }
                } else if frame_rest {
                    match bg_color {
                        Some(c) => {
                            match &mut run {
                                Some((_, e, k)) if *k == c => {
                                    debug_assert_eq!(*e, col);
                                    *e = next_col;
                                }
                                slot => {
                                    if let Some((s, e, k)) = *slot {
                                        let rk = match k {
                                            Color::Reset => foreground,
                                            _ => self.rgb(k, background),
                                        };
                                        backgrounds.push(Instance::solid(
                                            ox + s as f32 * cw,
                                            y,
                                            (e - s) as f32 * cw,
                                            ch,
                                            premultiply(rk, 1.0),
                                        ));
                                    }
                                    *slot = Some((col, next_col, c));
                                }
                            }
                        }
                        None => {
                            if let Some((s, e, k)) = run {
                                run = None;
                                let rk = match k {
                                    Color::Reset => foreground,
                                    _ => self.rgb(k, background),
                                };
                                backgrounds.push(Instance::solid(
                                    ox + s as f32 * cw,
                                    y,
                                    (e - s) as f32 * cw,
                                    ch,
                                    premultiply(rk, 1.0),
                                ));
                            }
                        }
                    }
                } else {
                    match bg_color {
                        Some(c) if motion.is_some() => {
                            let w = if span == 1 { cw } else { span as f32 * cw };
                            let x = ox + col as f32 * cw;
                            let rgb = match c {
                                Color::Reset => foreground,
                                _ => self.rgb(c, background),
                            };
                            backgrounds.push(moved(
                                Instance::solid(x, y, w, ch, premultiply(rgb, 1.0)),
                                motion.unwrap_or(transition::Motion::REST),
                                [x, y, w, ch],
                            ));
                        }
                        Some(c) => {
                            match &mut run {
                                Some((_, e, k)) if *k == c && *e == col => {
                                    debug_assert_eq!(*e, col);
                                    *e = next_col;
                                }
                                slot => {
                                    if let Some((s, e, k)) = *slot {
                                        let rk = match k {
                                            Color::Reset => foreground,
                                            _ => self.rgb(k, background),
                                        };
                                        backgrounds.push(Instance::solid(
                                            ox + s as f32 * cw,
                                            y,
                                            (e - s) as f32 * cw,
                                            ch,
                                            premultiply(rk, 1.0),
                                        ));
                                    }
                                    *slot = Some((col, next_col, c));
                                }
                            };
                        }
                        None => {
                            if let Some((s, e, k)) = run {
                                run = None;
                                let rk = match k {
                                    Color::Reset => foreground,
                                    _ => self.rgb(k, background),
                                };
                                backgrounds.push(Instance::solid(
                                    ox + s as f32 * cw,
                                    y,
                                    (e - s) as f32 * cw,
                                    ch,
                                    premultiply(rk, 1.0),
                                ));
                            }
                        }
                    }
                }
                if let Some(placeholder) = picture {
                    let x = ox + col as f32 * cw;
                    let pw = if span == 1 { cw } else { span as f32 * cw };
                    self.picture_span(
                        placeholder,
                        span as u16,
                        x,
                        y,
                        if (mbits & dim_bit) != 0 { 0.6 } else { 1.0 },
                        &mut pictures,
                        if frame_rest {
                            transition::Motion::REST
                        } else {
                            motion.unwrap_or(transition::Motion::REST)
                        },
                        [x, y, pw, ch],
                    );
                } else if !(if slen == 1 {
                        matches!(b0, b' ' | b'\t' | b'\n' | b'\r')
                    } else {
                        slen == 0
                            || (b0 < 0x80 && is_blank_symbol(symbol))
                    })
                    && (mbits & hidden_bit) == 0
                {
                    let abits = if (mbits & dim_bit) != 0 {
                        0x3F19_999Au32
                    } else {
                        0x3F80_0000u32
                    };
                    let style = ((mbits & bold_bit) != 0) as u8 * BOLD
                        | ((mbits & italic_bit) != 0) as u8 * ITALIC;
                    if let Some(entry) = self.glyph(symbol, style, span as u8, slen, b0) {
                        let (_, premul) = if cursor_block_active && at_cursor {
                            let v = cursor_text;
                            (v, premultiply(v, f32::from_bits(abits)))
                        } else {
                                let probe = match fg_memo.as_ref() {
                                    Some((d, r, v, pa, pv))
                                        if *d == cell_data.fg && *r == fg_rev =>
                                    {
                                        Some((*v, *pa, *pv))
                                    }
                                    _ => None,
                                };
                                match probe {
                                    Some((v, pa, pv)) if pa == abits => (v, pv),
                                    Some((v, _, _)) => {
                                        let q = premultiply(v, f32::from_bits(abits));
                                        fg_memo =
                                            Some((cell_data.fg, fg_rev, v, abits, q));
                                        (v, q)
                                    }
                                _ => {
                                    let v = match fg_rev {
                                        Some(c) => self.rgb(c, background),
                                        None => {
                                            self.rgb(cell_data.fg, foreground)
                                        }
                                    };
                                    let q = premultiply(v, f32::from_bits(abits));
                                    fg_memo =
                                        Some((cell_data.fg, fg_rev, v, abits, q));
                                    (v, q)
                                }
                            }
                        };
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    let inst = Instance {
                            rect: [x + entry.left, y + entry.top, entry.uv[2], entry.uv[3]],
                            uv: entry.uv,
                            color: premul,
                            kind: if entry.colored { COLOR } else { MASK },
                            pivot: [0.0; 2],
                            angle: 0.0,
                            ..Zeroable::zeroed()
                        };
                        if frame_rest {
                            glyphs.push(inst);
                        } else {
                            push_moved(&mut glyphs, inst, motion.unwrap_or(transition::Motion::REST), [x, y, w, ch]);
                        }
                    }
                }
                if (mbits & (underlined_bit | crossed_bit)) != 0 {
                    let abits = if (mbits & dim_bit) != 0 {
                        0x3F19_999Au32
                    } else {
                        0x3F80_0000u32
                    };
                    let dw = if span == 1 { cw } else { span as f32 * cw };
                    let dx = ox + col as f32 * cw;
                    if (mbits & underlined_bit) != 0 {
                    let fg = if cursor_block_active && at_cursor {
                        cursor_text
                    } else {
                        let probe_v = match fg_memo.as_ref() {
                            Some((d, r, v, _, _))
                                if *d == cell_data.fg && *r == fg_rev =>
                            {
                                Some(*v)
                            }
                            _ => None,
                        };
                        match probe_v {
                            Some(v) => v,
                            _ => {
                                let v = match fg_rev {
                                    Some(c) => self.rgb(c, background),
                                    None => {
                                        self.rgb(cell_data.fg, foreground)
                                    }
                                };
                                let q = premultiply(v, f32::from_bits(abits));
                                fg_memo =
                                    Some((cell_data.fg, fg_rev, v, abits, q));
                                v
                            }
                        }
                    };
                    let color = self.rgb(cell_data.underline_color, fg);
                    let uinst = Instance::solid(
                        dx,
                        y + underline_y,
                        dw,
                        underline_h,
                        premultiply(color, f32::from_bits(abits)),
                    );
                    if frame_rest {
                        overlays.push(uinst);
                    } else {
                        push_moved(&mut overlays, uinst, motion.unwrap_or(transition::Motion::REST), [dx, y, dw, ch]);
                    }
                }
                if (mbits & crossed_bit) != 0 {
                    let (_, strike_premul) = if cursor_block_active && at_cursor {
                        let v = cursor_text;
                        (v, premultiply(v, f32::from_bits(abits)))
                    } else {
                        let probe_s = match fg_memo.as_ref() {
                            Some((d, r, v, pa, pv))
                                if *d == cell_data.fg && *r == fg_rev =>
                            {
                                Some((*v, *pa, *pv))
                            }
                            _ => None,
                        };
                        match probe_s {
                            Some((v, pa, pv)) if pa == abits => (v, pv),
                            Some((v, _, _)) => {
                                let q = premultiply(v, f32::from_bits(abits));
                                fg_memo =
                                    Some((cell_data.fg, fg_rev, v, abits, q));
                                (v, q)
                            }
                            _ => {
                                let v = match fg_rev {
                                    Some(c) => self.rgb(c, background),
                                    None => {
                                        self.rgb(cell_data.fg, foreground)
                                    }
                                };
                                let q = premultiply(v, f32::from_bits(abits));
                                fg_memo =
                                    Some((cell_data.fg, fg_rev, v, abits, q));
                                (v, q)
                            }
                        }
                    };

                    let sinst = Instance::solid(
                        dx,
                        y + strike_y,
                        dw,
                        underline_h,
                        strike_premul,
                    );
                    if frame_rest {
                        overlays.push(sinst);
                    } else {
                        push_moved(&mut overlays, sinst, motion.unwrap_or(transition::Motion::REST), [dx, y, dw, ch]);
                    }
                    }
                }
                if at_cursor {
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    if cursor_thin_active {
                        let rgb = cursor_color;
                        let c = premultiply(rgb, 1.0);
                        let (ow, oh) = match cursor_kind {
                            crate::CursorKind::Underline => (w, 2.0),
                            _ => (2.0, ch),
                        };
                        overlays.push(Instance::solid(x, y + ch - oh, ow, oh, c));
                    }
                    if !focused {
                        let c = premultiply(cursor, 1.0);
                        overlays.push(Instance::solid(x, y, w, 1.0, c));
                        overlays.push(Instance::solid(x, y + ch - 1.0, w, 1.0, c));
                        overlays.push(Instance::solid(x, y, 1.0, ch, c));
                        overlays.push(Instance::solid(x + w - 1.0, y, 1.0, ch, c));
                    }
                }
                col = next_col;
            }
            if let Some((s, e, k)) = run {
                let rk = match k {
                    Color::Reset => foreground,
                    _ => self.rgb(k, background),
                };
                backgrounds.push(Instance::solid(
                    ox + s as f32 * cw,
                    y,
                    (e - s) as f32 * cw,
                    ch,
                    premultiply(rk, 1.0),
                ));
            }
        }
        let time = clock();
        let (below_layers, above_layers) = if layers.windows(2).all(|w| w[0].z <= w[1].z) {
            let split = layers.partition_point(|l| l.z < 0);
            (
                self.core.layer_parts(layers[..split].iter(), (ox, oy), cw, ch, None, time),
                self.core.layer_parts(layers[split..].iter(), (ox, oy), cw, ch, None, time),
            )
        } else {
            let mut sorted: Vec<&Layer> = layers.iter().collect();
            sorted.sort_by_key(|layer| layer.z);
            (
                self.core.layer_parts(
                    sorted.iter().copied().filter(|l| l.z < 0),
                    (ox, oy),
                    cw,
                    ch,
                    None,
                    time,
                ),
                self.core.layer_parts(
                    sorted.iter().copied().filter(|l| l.z >= 0),
                    (ox, oy),
                    cw,
                    ch,
                    None,
                    time,
                ),
            )
        };
        let mut below = tiles;
        below.extend(backgrounds);
        let mut above = glyphs;
        above.extend(overlays);
        above.extend(dust);
        let mut parts = std::mem::take(&mut self.spare_parts);
        if let Some(wallpaper) = self.core.wallpaper() {
            let window = (self.config.width as f32, self.config.height as f32);
            let (dst, src) = wallpaper.quad(window);
            let opacity = if self.opaque_only { 1.0 } else { self.opacity };
            parts.push((
                Draw::Wallpaper,
                vec![Instance {
                    rect: dst,
                    uv: src,
                    color: [1.0, 1.0, 1.0, opacity],
                    kind: PICTURE,
                    pivot: [0.0; 2],
                    angle: 0.0,
                    ..Zeroable::zeroed()
                }],
            ));
            let tint = wallpaper.tint * opacity;
            if tint > 0.0 {
                parts.push((
                    Draw::Atlas,
                    vec![Instance::solid(
                        0.0,
                        0.0,
                        window.0,
                        window.1,
                        premultiply(background, tint),
                    )],
                ));
            }
        }
        parts.extend(below_layers);
        parts.push((Draw::Atlas, below));
        for (draw, group) in pictures.iter_mut() {
            if !group.is_empty() {
                let taken = std::mem::replace(group, pool.pop().unwrap_or_default());
                parts.push((*draw, taken));
            }
        }
        self.spare_pictures = pictures;
        parts.push((Draw::Atlas, above));
        parts.extend(above_layers);
        self.card_update(snapshot.card.as_ref(), Instant::now());
        self.card_wait_for_window(Instant::now());
        if self.card_show.is_some() {
            let card = self.card_parts((ox, oy), time);
            parts.extend(card);
        }
        if !self.overlay.is_empty() {
            let overlay = std::mem::take(&mut self.overlay);
            parts.extend(self.core.layer_parts(overlay.iter(), (ox, oy), cw, ch, None, time));
            self.overlay = overlay;
        }
        if let Some((started, id)) = self.handoff {
            let alpha = transition::handoff_alpha(started.elapsed().as_secs_f32());
            if alpha > 0.0 {
                if let Some(picture) = self.core.use_picture(id) {
                    parts.push((Draw::Picture { id, linear: true }, vec![Instance {
                        rect: [0.0, 0.0, self.config.width as f32, self.config.height as f32],
                        uv: [0.0, 0.0, picture.width as f32, picture.height as f32],
                        color: [1.0, 1.0, 1.0, alpha],
                        kind: picture.kind,
                        ..Zeroable::zeroed()
                    }]));
                }
            } else {
                self.handoff = None;
                self.core.drop_frozen_frame(id);
            }
        }
        out.assemble_into(&mut parts, &mut pool);
        self.spare_parts = parts;
        self.core.keep_vecs(pool);
    }

    #[allow(clippy::too_many_arguments)]
    fn picture_span(
        &mut self,
        placeholder: graphics::Placeholder,
        cols_span: u16,
        x: f32,
        y: f32,
        alpha: f32,
        pictures: &mut std::collections::BTreeMap<Draw, Vec<Instance>>,
        motion: transition::Motion,
        cell_rect: [f32; 4],
    ) {
        let cell = (self.fonts.cell.width, self.fonts.cell.height);
        let Some(picture) = self.core.use_picture(placeholder.image) else {
            return;
        };
        push_picture_slice(
            pictures,
            placeholder.image,
            (picture.width, picture.height),
            picture.cols,
            picture.rows,
            cell,
            placeholder.row,
            placeholder.col,
            cols_span,
            x,
            y,
            alpha,
            picture.kind,
            motion,
            cell_rect,
        );
    }

    pub fn start_transition(&mut self) {
        self.transition = Some(transition::Transition::start(Instant::now()));
    }

    pub fn capture_transition(
        &mut self,
        snapshot: &Snapshot,
        layers: &[Layer],
        seconds: f32,
    ) -> (u32, u32, Vec<u8>) {
        let running = self.transition;
        self.transition = Some(transition::Transition::at(Instant::now(), seconds));
        let frame = self.capture(snapshot, layers);
        self.transition = running;
        frame
    }

    pub fn skip_transition(&mut self) {
        self.transition = None;
    }

    pub fn start_handoff(&mut self) {
        const A: u32 = 0x0fff_ffd0;
        const B: u32 = 0x0fff_ffd1;
        let previous = self.handoff.map(|(_, id)| id);
        let id = if previous == Some(A) { B } else { A };
        let alpha = if self.opaque_only { 1.0 } else { f64::from(self.opacity) };
        let background = self.palette.background;
        let channel = |c: u8| f64::from(c) / 255.0 * alpha;
        let clear = wgpu::Color { r: channel(background[0]), g: channel(background[1]), b: channel(background[2]), a: alpha };
        if self.core.freeze_frame(id, (self.config.width, self.config.height), clear) {
            self.handoff = Some((Instant::now(), id));
            self.transition = None;
            if let Some(old) = previous { self.core.drop_frozen_frame(old); }
        }
    }

    pub fn animating(&self) -> bool {
        self.handoff.is_some()
            || self.transition.is_some_and(|t| !t.finished(Instant::now()))
            || self.core.stages_animating()
            || self.core.scenes_animating()
            || self.card_animating()
    }

    pub fn apply(&mut self, pictures: Vec<graphics::Picture>) {
        let cap = self.texture_byte_cap();
        for mut picture in pictures {
            graphics::shrink_picture_to_budget(&mut picture, self.texture_budget);
            if (picture.rgba.len() as u64) > cap {
                eprintln!("GLASS: picture {} over byte cap, dropped", picture.id);
                continue;
            }
            self.core.upload(picture);
        }
    }

    pub fn free(&mut self, ids: Vec<u32>) {
        for id in ids {
            self.core.free_picture(id);
        }
    }

    pub fn texture_byte_cap(&self) -> u64 {
        graphics::texture_byte_budget(self.texture_budget)
    }

    pub fn upload_mesh(&mut self, id: u32, mesh: &angel_stage::scene::MeshData) {
        if let Err(error) = self.core.upload_mesh(id, mesh) {
            eprintln!("[glass] mesh {id} refused: {error:?}");
        }
    }

    pub fn set_stage(&mut self, id: u32, stage: Option<Stage>) {
        self.core.set_stage(id, stage);
    }

    pub fn set_scene(&mut self, id: u32, scene: Option<(angel_stage::scene::Scene, (u32, u32))>) {
        self.core.set_scene_target(id, scene);
    }

    pub fn set_gpu_picture(&mut self,id:u32,producer:Option<(Box<dyn angel_gpu::PictureProducer>,(u32,u32))>) {
        self.core.set_gpu_picture(id,producer);
    }

    fn render_stages(&mut self) {
        let cell = (self.fonts.cell.width, self.fonts.cell.height);
        let now = clock();
        self.core.render_stages(cell, now);
        self.core.set_scene_clock(now);
        self.core.render_scenes();
        self.core.render_gpu_pictures();
    }

    pub fn render(&mut self, snapshot: &Snapshot, layers: &[Layer], focused: bool) -> bool {
        let started = Instant::now();
        if std::mem::take(&mut self.reconfigure) {
            self.surface.configure(&self.device, &self.config);
        }
        if self.core.atlas_full() {
            self.reset_glyphs();
        }
        self.render_stages();
        let mut frame_data = self.core.take_frame();
        self.build(snapshot, layers, focused, &mut frame_data);
        if self.core.atlas_full() {
            self.reset_glyphs();
            self.build(snapshot, layers, focused, &mut frame_data);
            self.core.atlas_settle_after_rebuild();
        }
        let built = Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.reconfigure = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.core.keep_frame(frame_data);
                return false;
            }
            _ => {
                self.core.keep_frame(frame_data);
                return false;
            }
        };
        let acquired = Instant::now();
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        let background = self.palette.background;
        let alpha = if self.opaque_only {
            1.0
        } else {
            f64::from(self.opacity)
        };
        let channel = |c: u8| f64::from(c) / 255.0 * alpha;
        let clear = wgpu::Color {
            r: channel(background[0]),
            g: channel(background[1]),
            b: channel(background[2]),
            a: alpha,
        };
        let size = (self.config.width, self.config.height);
        self.core.draw(&view, size, clear, &frame_data, "glass-frame");
        let submitted = Instant::now();
        self.window.pre_present_notify();
        self.queue.present(frame);
        self.timing = [
            built.duration_since(started),
            acquired.duration_since(built),
            submitted.duration_since(acquired),
            submitted.elapsed(),
        ];
        self.core.keep_frame(frame_data);
        let _ = self.device.poll(wgpu::PollType::Poll);
        true
    }

    pub fn capture_later(&mut self, snapshot: &Snapshot, layers: &[Layer]) -> angel_gpu::ReadBack {
        self.capture_with(snapshot, layers, |core, texture, size| core.read_back_later(texture, size))
    }

    pub fn poll_captures(&self, wait: bool) {
        self.core.poll_read_backs(wait);
    }

    pub fn capture(&mut self, snapshot: &Snapshot, layers: &[Layer]) -> (u32, u32, Vec<u8>) {
        let (width, height) = (self.config.width, self.config.height);
        let pixels = self.capture_with(snapshot, layers, |core, texture, size| core.read_back(texture, size));
        (width, height, pixels)
    }

    fn capture_with<T>(
        &mut self,
        snapshot: &Snapshot,
        layers: &[Layer],
        read: impl FnOnce(&angel_gpu::Core, &wgpu::Texture, (u32, u32)) -> T,
    ) -> T {
        self.render_stages();
        let mut frame_data = self.core.take_frame();
        self.build(snapshot, layers, true, &mut frame_data);
        let (width, height) = (self.config.width, self.config.height);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glass-capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.view_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let background = self.palette.background;
        let alpha = f64::from(self.opacity);
        let channel = |c: u8| f64::from(c) / 255.0 * alpha;
        let clear = wgpu::Color {
            r: channel(background[0]),
            g: channel(background[1]),
            b: channel(background[2]),
            a: alpha,
        };
        self.core.draw(&view, (width, height), clear, &frame_data, "glass-capture");
        let read = read(&self.core, &texture, (width, height));
        self.core.keep_frame(frame_data);
        read
    }
}

#[inline(always)]
fn push_moved(dst: &mut Vec<Instance>, instance: Instance, m: transition::Motion, cell: [f32; 4]) {
    if m == transition::Motion::REST {
        dst.push(instance);
    } else {
        dst.push(moved(instance, m, cell));
    }
}
fn moved(mut instance: Instance, m: transition::Motion, cell: [f32; 4]) -> Instance {
    if m == transition::Motion::REST {
        return instance;
    }
    let pivot = [cell[0] + cell[2] / 2.0, cell[1] + cell[3] / 2.0];
    let [x, y, w, h] = instance.rect;
    instance.rect = [
        pivot[0] + (x - pivot[0]) * m.scale + m.dx,
        pivot[1] + (y - pivot[1]) * m.scale + m.dy,
        w * m.scale,
        h * m.scale,
    ];
    instance.pivot = [pivot[0] + m.dx, pivot[1] + m.dy];
    instance.angle = m.angle;
    for channel in &mut instance.color {
        *channel *= m.alpha;
    }
    instance
}

const STONE: [f32; 3] = [0.17, 0.175, 0.19];
const WALL_STONE: [f32; 3] = [0.25, 0.255, 0.28];
const STONE_VARIATION: f32 = 0.26;
const STONE_LIGHT: f32 = 1.45;
const STONE_DARK: f32 = 0.55;
const SHADOW_PX: [f32; 2] = [14.0, 20.0];
const SHADOW_ALPHA: f32 = 0.5;

fn stone_tile(
    tiles: &mut Vec<Instance>,
    dust: &mut Vec<Instance>,
    m: transition::Motion,
    cell: [f32; 4],
    background: Rgb,
    (col, row): (u16, u16),
    wall: bool,
) {
    if m.tile <= 0.0 {
        return;
    }
    let [x, y, w, h] = cell;
    let [tx, ty, tw, th] = [x + 0.5, y + 0.5, (w - 1.0).max(1.0), (h - 1.0).max(1.0)];
    let at_rest = transition::Motion { alpha: m.tile, ..m };
    if m.lift > 0.0 {
        let shadow = transition::Motion {
            dx: m.dx + SHADOW_PX[0] * m.lift,
            dy: m.dy + SHADOW_PX[1] * m.lift,
            alpha: 1.0,
            ..m
        };
        let a = SHADOW_ALPHA * m.lift * m.tile;
        tiles.push(moved(
            Instance::solid(tx, ty, tw, th, [0.0, 0.0, 0.0, a]),
            shadow,
            cell,
        ));
    }
    let shade = 1.0 + (transition::hash(col, row, 4) - 0.5) * STONE_VARIATION;
    let tone = if wall { WALL_STONE } else { STONE };
    let base = |i: usize| ((f32::from(background[i]) / 255.0 + tone[i]) * shade).min(1.0);
    let stone = [base(0), base(1), base(2), 1.0];
    let scaled = |k: f32| [stone[0] * k, stone[1] * k, stone[2] * k, 1.0];
    let mut put = |rect: [f32; 4], color: [f32; 4]| {
        tiles.push(moved(
            Instance::solid(rect[0], rect[1], rect[2], rect[3], color),
            at_rest,
            cell,
        ));
    };
    put([tx, ty, tw, th], stone);
    put([tx, ty, tw, 1.0], scaled(STONE_LIGHT));
    put([tx, ty, 1.0, th], scaled(STONE_LIGHT));
    put([tx, ty + th - 1.0, tw, 1.0], scaled(STONE_DARK));
    put([tx + tw - 1.0, ty, 1.0, th], scaled(STONE_DARK));
    for salt in [6, 7] {
        let sx = tx + 1.0 + (tw - 3.0) * transition::hash(col, row, salt);
        let sy = ty + 1.0 + (th - 3.0) * transition::hash(col, row, salt + 10);
        put([sx.floor(), sy.floor(), 1.0, 1.0], scaled(0.7));
    }
    if m.dust > 0.0 && transition::hash(col, row, 5) < 0.35 {
        let spread = 1.0 - m.dust;
        let a = 0.5 * m.dust;
        for (i, side) in [-1.0f32, 1.0, 0.0].iter().enumerate() {
            let px = x + w / 2.0 + side * (w / 2.0 + 12.0 * spread) - 1.0;
            let py = y + h - 2.0 - (6.0 + 5.0 * i as f32) * spread;
            dust.push(Instance::solid(
                px,
                py,
                2.0,
                2.0,
                [0.62 * a, 0.58 * a, 0.52 * a, a],
            ));
        }
    }
}

pub fn points_to_px(points: f32, scale: f32) -> f32 {
    let dpi = if cfg!(target_os = "macos") {
        72.0
    } else {
        96.0
    };
    points * dpi / 72.0 * scale
}

#[inline]
fn premultiply(rgb: Rgb, alpha: f32) -> [f32; 4] {
    let k = alpha * (1.0 / 255.0);
    [
        f32::from(rgb[0]) * k,
        f32::from(rgb[1]) * k,
        f32::from(rgb[2]) * k,
        alpha,
    ]
}

pub(crate) fn cell_rgb(palette: &Palette, color: Color, fallback: Rgb) -> Rgb {
    let ansi = &palette.ansi;
    match color {
        Color::Reset => fallback,
        Color::Black => ansi[0],
        Color::Red => ansi[1],
        Color::Green => ansi[2],
        Color::Yellow => ansi[3],
        Color::Blue => ansi[4],
        Color::Magenta => ansi[5],
        Color::Cyan => ansi[6],
        Color::Gray => ansi[7],
        Color::DarkGray => ansi[8],
        Color::LightRed => ansi[9],
        Color::LightGreen => ansi[10],
        Color::LightYellow => ansi[11],
        Color::LightBlue => ansi[12],
        Color::LightMagenta => ansi[13],
        Color::LightCyan => ansi[14],
        Color::White => ansi[15],
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(i) => indexed(i, ansi),
    }
}

fn indexed_table(ansi: &[Rgb; 16]) -> [Rgb; 256] {
    let mut lut = [[0, 0, 0]; 256];
    for (n, slot) in lut.iter_mut().enumerate() {
        *slot = indexed(n as u8, ansi);
    }
    lut
}

#[inline]
fn indexed(i: u8, ansi: &[Rgb; 16]) -> Rgb {
    match i {
        0..=15 => ansi[usize::from(i)],
        16..=231 => {
            let n = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [level(n / 36), level((n / 6) % 6), level(n % 6)]
        }
        _ => {
            let v = 8 + (i - 232) * 10;
            [v, v, v]
        }
    }
}

fn clock() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

#[inline]
const fn sprite_slot(symbol: &str) -> Option<(usize, char)> {
    let b = symbol.as_bytes();
    if b.len() != 3 || b[0] != 0xE2 {
        return None;
    }
    let cp = 0x2000u32 | (((b[1] & 0x3F) as u32) << 6) | ((b[2] & 0x3F) as u32);
    if cp >= 0x2500 && cp <= 0x259F {
        let ch = match char::from_u32(cp) {
            Some(c) => c,
            None => return None,
        };
        Some(((cp - 0x2500) as usize, ch))
    } else if cp >= 0x2800 && cp <= 0x28FF {
        let ch = match char::from_u32(cp) {
            Some(c) => c,
            None => return None,
        };
        Some((160 + (cp - 0x2800) as usize, ch))
    } else {
        None
    }
}

#[inline]
const fn is_box_drawing(symbol: &str) -> bool {
    let b = symbol.as_bytes();
    b.len() >= 3 && b[0] == 0xE2 && (b[1] == 0x94 || b[1] == 0x95)
}

#[inline]
fn is_blank_symbol(symbol: &str) -> bool {
    let b = symbol.as_bytes();
    if b.is_empty() {
        return true;
    }
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c < 0x80 {
            if c != b' ' && c != b'\t' && c != b'\n' && c != b'\r' {
                return false;
            }
            i += 1;
        } else {
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn push_picture_slice(
    pictures: &mut std::collections::BTreeMap<Draw, Vec<Instance>>,
    image_id: u32,
    image_px: (u32, u32),
    box_cols: u16,
    box_rows: u16,
    cell: (u32, u32),
    row: u16,
    col: u16,
    cols_span: u16,
    x: f32,
    y: f32,
    alpha: f32,
    kind: u32,
    motion: transition::Motion,
    cell_rect: [f32; 4],
) {
    let boxed = graphics::box_cells(image_px, box_cols, box_rows, cell);
    let Some((dst, src)) =
        graphics::cell_span_slice(image_px, boxed, cell, row, col, cols_span)
    else {
        return;
    };
    let linear = dst[2] + 0.01 < src[2] || dst[3] + 0.01 < src[3];
    let key = Draw::Picture {
        id: image_id,
        linear,
    };
    let inst = moved(
        Instance {
            rect: [x + dst[0], y + dst[1], dst[2], dst[3]],
            uv: src,
            color: [1.0, 1.0, 1.0, alpha],
            kind,
            pivot: [0.0; 2],
            angle: 0.0,
            ..Zeroable::zeroed()
        },
        motion,
        cell_rect,
    );
    let group = pictures.entry(key).or_default();
    if motion.dx == 0.0
        && motion.dy == 0.0
        && motion.scale == 1.0
        && let Some(prev) = group.last_mut()
        && (prev.rect[0] - inst.rect[0]).abs() < 1e-3
        && (prev.rect[2] - inst.rect[2]).abs() < 1e-3
        && (prev.rect[1] + prev.rect[3] - inst.rect[1]).abs() < 1e-3
        && (prev.uv[0] - inst.uv[0]).abs() < 1e-3
        && (prev.uv[2] - inst.uv[2]).abs() < 1e-3
        && (prev.uv[1] + prev.uv[3] - inst.uv[1]).abs() < 1e-3
        && prev.color == inst.color
        && prev.angle == 0.0
    {
        prev.rect[3] += inst.rect[3];
        prev.uv[3] += inst.uv[3];
        return;
    }
    group.push(inst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(n: usize) -> Vec<Instance> {
        vec![Instance::solid(0.0, 0.0, 1.0, 1.0, [0.0; 4]); n]
    }

    #[test]
    fn a_startup_piece_gives_its_work_or_nothing() {
        let here = std::thread::current().id();
        let piece = Ready::start("test-piece", move || (std::thread::current().id() != here, 7));
        assert_eq!(piece.wait(), Some((true, 7)));
        let died: Ready<u32> = Ready::start("test-piece", || panic!("the work failed"));
        assert_eq!(died.wait(), None);
        assert_eq!(Ready::Done(Some(3)).wait(), Some(3));
    }

    #[test]
    fn capped_insert_clears_a_full_memo_but_keeps_updates() {
        let mut map: FastMap<u32, u32> = FastMap::default();
        for i in 0..8 {
            insert_capped(&mut map, i, i * 10, 4);
        }
        assert!(map.len() <= 4, "memo bounded, holds {}", map.len());
        let present = *map.keys().next().unwrap();
        let size = map.len();
        insert_capped(&mut map, present, 99, 4);
        assert_eq!(map.len(), size);
        assert_eq!(map[&present], 99);
    }

    #[test]
    fn picture_buffers_for_gone_pictures_are_pruned() {
        let mut buffers = std::collections::BTreeMap::new();
        buffers.insert(Draw::Atlas, solid(1));
        buffers.insert(Draw::Picture { id: 1, linear: false }, solid(2));
        buffers.insert(Draw::Picture { id: 2, linear: true }, solid(3));
        buffers.insert(Draw::Picture { id: 3, linear: false }, solid(4));
        prune_picture_buffers(&mut buffers, |id| id == 2);
        assert_eq!(buffers.len(), 2, "atlas and the live picture stay");
        assert!(buffers.contains_key(&Draw::Picture { id: 2, linear: true }));
    }

    #[test]
    fn xterm_cube_and_greys() {
        let ansi = Palette::default().ansi;
        assert_eq!(indexed(16, &ansi), [0, 0, 0]);
        assert_eq!(indexed(196, &ansi), [255, 0, 0]);
        assert_eq!(indexed(231, &ansi), [255, 255, 255]);
        assert_eq!(indexed(232, &ansi), [8, 8, 8]);
        assert_eq!(indexed(255, &ansi), [238, 238, 238]);
    }

    #[test]
    fn indexed_lut_agrees_with_indexed_everywhere() {
        let ansi = Palette::default().ansi;
        let lut = indexed_table(&ansi);
        for n in 0..=255u16 {
            let i = n as u8;
            assert_eq!(lut[usize::from(i)], indexed(i, &ansi), "entry {i}");
        }
    }

    #[test]
    fn sprite_slot_bijection_and_box_drawing_prefix() {
        let mut seen = [false; 416];
        for cp in 0x2500u32..=0x259F {
            let c = char::from_u32(cp).unwrap();
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            let (slot, decoded) = sprite_slot(s).unwrap();
            assert_eq!(decoded, c);
            assert!(!seen[slot]);
            seen[slot] = true;
            assert_eq!(is_box_drawing(s), (0x2500..=0x257F).contains(&cp));
        }
        for cp in 0x2800u32..=0x28FF {
            let c = char::from_u32(cp).unwrap();
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            let (slot, decoded) = sprite_slot(s).unwrap();
            assert_eq!(decoded, c);
            assert!(!seen[slot]);
            seen[slot] = true;
            assert!(!is_box_drawing(s));
        }
        assert!(seen.iter().all(|&b| b));
    }

    #[test]
    fn picture_stage_2d_coalescing_collapses_2880_cells_into_1_quad() {
        let mut pictures = std::collections::BTreeMap::new();
        let (cols, rows) = (80u16, 36u16);
        let cell = (10u32, 20u32);
        let image_px = (800u32, 720u32);
        let motion = transition::Motion::REST;
        for row in 0..rows {
            let y = f32::from(row) * 20.0;
            push_picture_slice(
                &mut pictures,
                0x00FA_57A4,
                image_px,
                cols,
                rows,
                cell,
                row,
                0,
                cols,
                0.0,
                y,
                1.0,
                PICTURE,
                motion,
                [0.0, y, 10.0, 20.0],
            );
        }
        let group = &pictures[&Draw::Picture {
            id: 0x00FA_57A4,
            linear: false,
        }];
        assert_eq!(
            group.len(),
            1,
            "80x36 (2,880 cells) coalesces horizontally + vertically into a single GPU quad"
        );
        assert_eq!(group[0].rect, [0.0, 0.0, 800.0, 720.0]);
        assert_eq!(group[0].uv, [0.0, 0.0, 800.0, 720.0]);
    }
}

include!("render_card.rs");

#[cfg(test)]
mod atlas_gpu_tests {
    #[test]
    fn the_atlas_grows_and_keeps_what_it_held() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        })) else {
            println!("no GPU adapter: skipped");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("atlas-test"),
            required_limits: adapter.limits(),
            ..Default::default()
        })) else {
            println!("no GPU device: skipped");
            return;
        };
        let mut core = angel_gpu::Core::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, 1.0);
        let image = |n: u32| -> Vec<u8> { (0..64 * 64).flat_map(|i| [n as u8, (n >> 8) as u8, i as u8, 255]).collect() };
        let first = core.atlas_insert(64, 64, &image(1)).expect("the first image fits");
        let (side, _) = core.atlas_pixels();
        assert_eq!(side, 1024, "a new atlas is small");
        let mut placed = vec![(first, 1)];
        let mut n = 2;
        while core.atlas_pixels().0 == 1024 {
            let at = core.atlas_insert(64, 64, &image(n)).expect("an image fits as the atlas grows");
            placed.push((at, n));
            n += 1;
            assert!(n < 2000, "the atlas never grew");
        }
        let at = core.atlas_insert(64, 64, &image(n)).expect("room after the growth");
        placed.push((at, n));
        let (side, pixels) = core.atlas_pixels();
        assert_eq!(side, 2048);
        for ([x, y], n) in placed {
            let want = image(n);
            for row in [0u32, 31, 63] {
                let at = ((y + row) * side + x) as usize * 4;
                let have = &pixels[at..at + 64 * 4];
                assert_eq!(have, &want[row as usize * 256..row as usize * 256 + 256], "image {n} row {row} at {x},{y}");
            }
        }
    }
}
