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

/// Glyph memo bounds: far past any real working set (heavy CJK text
/// draws tens of thousands of distinct chars), so the maps cannot grow
/// for the life of a session shown adversarial symbol streams.
const CHARS_MAX: usize = 65_536;
const STRINGS_MAX: usize = 16_384;

/// Picture-span guard: cells carrying any of these modifiers take the
/// per-cell path instead of the multi-column picture coalescing loop.
/// Hoisted as a plain `u16` so the hot path pays one integer `&` instead
/// of building a `Modifier` union per cell.
/// REVERSED=0x40 HIDDEN=0x80 UNDERLINED=0x08 CROSSED_OUT=0x100.
const PICTURE_GUARD_MASK: u16 = 0x40 | 0x80 | 0x08 | 0x100;

/// Insert into a glyph memo, clearing it first when full. Clearing only
/// loses memoization: glyphs re-rasterize on demand, and the atlas-full
/// path (`AtlasHealth`) still bounds the texture side.
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

/// Drop pooled picture-cell buffers whose picture is gone (evicted or
/// removed): keyed by picture id, the map otherwise keeps an entry (and
/// its retained capacity) for every picture ever drawn.
fn prune_picture_buffers(
    buffers: &mut std::collections::BTreeMap<Draw, Vec<Instance>>,
    live: impl Fn(u32) -> bool,
) {
    buffers.retain(|draw, _| match draw {
        Draw::Picture { id, .. } => live(*id),
        _ => true,
    });
}

/// The multi-char glyph cache is keyed by the symbol's hash (with the
/// symbol kept beside the entry and verified on a hit), so a lookup
/// never allocates — keying by the owned string boxed it on every hit.
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
    /// The GPU side: pipeline, pictures, stages, atlas and the layer
    /// compiler (`angel-gpu`).
    core: Core,
    transition: Option<transition::Transition>,
    handoff: Option<(Instant, u32)>,
    /// The last frame came back suboptimal: reconfigure before the next.
    reconfigure: bool,
    /// The last frame's build, surface acquire, submit and present.
    pub timing: [std::time::Duration; 4],
    /// Part lists kept for the next build.
    spare_parts: Vec<(Draw, Vec<Instance>)>,
    /// Picture-cell instances grouped by draw, kept between builds so its
    /// buffers (and map) refill instead of reallocating.
    spare_pictures: std::collections::BTreeMap<Draw, Vec<Instance>>,
    /// Direct-indexed glyph memo for printable ASCII (`0x20..=0x7E`, 4 styles, span 1).
    ascii_chars: Box<[[Option<Option<Entry>>; 95]; 4]>,
    /// Direct-indexed glyph memo for built-in geometric/braille sprites
    /// (`0x2500..=0x259F` = 160, `0x2800..=0x28FF` = 256; style-independent).
    sprite_chars: Box<[Option<Option<Entry>>; 416]>,
    chars: FastMap<(char, u8, u8), Option<Entry>>,
    /// Two-slot MRU over `chars`: solid fills hit slot 0, 2-glyph
    /// stripes (borders, checker prose) hit slot 1 after one promotion.
    /// Two tuple compares worst-case; a miss shifts 0->1 and fills 0.
    chars_mru: [Option<((char, u8, u8), Option<Entry>)>; 2],
    strings: FastMap<(u64, u8, u8), (Box<str>, Option<Entry>)>,
    indexed_lut: [Rgb; 256],
    fonts: Fonts,
    /// The card on show (`render_card.rs`), and its pictures.
    card_show: Option<CardShow>,
    card_art: CardArt,
    /// Resource identity used to prepare the card's squire sheet.
    card_squire_identity: Option<crate::squire::ShowcaseClipIdentity>,
    /// The matching module lease also supplies card idle and wave timing.
    card_squire_clip: Option<Arc<crate::ShowcaseSquireClip>>,
    font_family: String,
    palette: Palette,
    opacity: f32,
    padding: f32,
    braille: BrailleStyle,
    font_points: f32,
    base_points: f32,
    scale: f32,
    /// Layers over everything, the foreground scroll included: the
    /// showcase (`showcase.rs`), set by the window before each draw.
    pub overlay: Vec<Layer>,
    pub adapter_name: String,
    /// GPU texture side budget from [`graphics::picture_side_budget`]:
    /// 8192 on RTX/discrete (full artist exports), 4096 on integrated
    /// so a hostile upload caps at 64 MiB instead of OOMing the session.
    pub texture_budget: u32,
    pub present_mode: wgpu::PresentMode,
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

/// The GPU as far as it goes without a window: the instance, the adapter
/// and the device.
pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Gpu {
    /// Open an adapter and its device on `instance`. With `surface`, only
    /// an adapter that can draw to it is taken; without, the one the same
    /// preference picks first, which the window checks once it has its
    /// surface.
    fn open(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> Result<Self, String> {
        let power_preference = match std::env::var("ANGEL_GLASS_GPU").as_deref() {
            Ok("low") => wgpu::PowerPreference::LowPower,
            Ok("high") => wgpu::PowerPreference::HighPerformance,
            // Unreal-grade default: prefer the discrete/NVIDIA path (this
            // host's RTX 3090) so multi-GPU laptops do not land on the
            // integrated GPU; kitty's GL context takes what it gets.
            // `ANGEL_GLASS_GPU=low` still opts into integrated for power
            // measurements, `=high` forces discrete explicitly.
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

/// What opening the renderer waits on that needs no window — the GPU
/// (the driver's instance and device take most of startup), the fonts
/// (fontconfig) and the decoded background image — each started on a
/// thread of its own before the window is asked for, so startup waits for
/// the slowest of them rather than their sum.
pub struct Preparing {
    gpu: Ready<Result<Gpu, String>>,
    /// Said once the GPU piece is done (`when_gpu_ready`).
    gpu_done: Arc<Signal>,
    fonts: Option<Ready<Result<Fonts, String>>>,
    /// The fonts once sized for the window, with the scale they were sized for.
    sized: Option<(Fonts, f32)>,
    wallpaper: Ready<Option<crate::wallpaper::Wallpaper>>,
}

/// A piece of startup being done, and who to tell once it is: told at
/// most once, from the piece's own thread.
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

    /// Have `wake` called once the piece is done. False if it is done
    /// already, and `wake` is dropped uncalled.
    fn when_done(&self, wake: Box<dyn FnOnce() + Send>) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.0 {
            return false;
        }
        state.1 = Some(wake);
        true
    }
}

/// One piece of startup: on a thread of its own, or already done when no
/// thread could be had.
enum Ready<T> {
    Thread(std::thread::JoinHandle<Option<T>>),
    Done(Option<T>),
}

impl<T: Send + 'static> Ready<T> {
    fn start(name: &str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        // The thread takes the work out of the slot; when no thread starts,
        // the work is still there and is done here.
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

    /// The piece, once done; `None` if its thread died.
    fn wait(self) -> Option<T> {
        match self {
            Self::Thread(thread) => thread.join().ok().flatten(),
            Self::Done(value) => value,
        }
    }

    /// The piece, once done, left in place for a later `wait`.
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
    /// Start the pieces. The fonts are opened at scale 1; [`Self::fonts`]
    /// sizes them for the window's scale once there is a window.
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

    /// Have `wake` called, from the GPU's thread, once the GPU is open.
    /// False if it is open already: `wake` is not called.
    pub fn when_gpu_ready(&self, wake: impl FnOnce() + Send + 'static) -> bool {
        self.gpu_done.when_done(Box::new(wake))
    }

    /// The window size, in pixels, that holds `cols` by `rows` cells and
    /// the padding at `scale` (the renderer's `padding_px` on each side).
    /// The fonts are sized for it here and kept for the renderer.
    pub fn pixels_for(&mut self, options: &Options, scale: f32, cols: u16, rows: u16) -> Result<(u32, u32), String> {
        let cell = self.cell(options, scale)?;
        let pad = (options.padding * scale).round() as u32 * 2;
        Ok((u32::from(cols) * cell.width + pad, u32::from(rows) * cell.height + pad))
    }

    /// The grid a window `width` by `height` pixels holds at `scale`, as
    /// [`Renderer::grid_size`] will measure it.
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

    /// The cell at `scale`, the fonts sized for it and kept for the renderer.
    fn cell(&mut self, options: &Options, scale: f32) -> Result<CellMetrics, String> {
        let fonts = self.fonts(options, scale)?;
        let cell = fonts.cell;
        self.sized = Some((fonts, scale));
        Ok(cell)
    }

    /// The background image, once decoded, still here for the renderer;
    /// and the fonts, if sized already, to draw the early frames' text.
    pub fn early_parts(&mut self) -> (Option<&crate::wallpaper::Wallpaper>, Option<&mut Fonts>) {
        let wallpaper = self.wallpaper.get().and_then(Option::as_ref);
        (wallpaper, self.sized.as_mut().map(|(fonts, _)| fonts))
    }

    /// The fonts, sized for `scale`.
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
    /// The renderer for `window`, from what [`Preparing`] made: the fonts,
    /// sized here for the window's scale, the GPU and the background image.
    pub fn new(window: Arc<Window>, options: &Options, mut preparing: Preparing) -> Result<Self, String> {
        let scale = window.scale_factor() as f32;
        let fonts = preparing.fonts(options, scale)?;
        let gpu = preparing.gpu.wait().ok_or("the GPU thread failed")??;
        let surface = gpu
            .instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| format!("window surface: {e}"))?;
        crate::startup::mark("surface");
        // The adapter was picked before there was a window. One that cannot
        // draw to it (a machine with several GPUs) gives way to the one the
        // surface itself picks, as if the window had come first.
        let Gpu { instance, adapter, device, queue } = if gpu.adapter.is_surface_supported(&surface) {
            gpu
        } else {
            Gpu::open(gpu.instance.clone(), Some(&surface))?
        };
        drop(instance);
        let info = adapter.get_info();
        // A GPU validation error is a bug to log, never a reason to take the
        // operator's session down (wgpu's default handler panics).
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
        // Blend in the non-linear space terminals use, so text weight and
        // colours look as they always have.
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
        // Fifo measured ~40 ms submit-to-photon (scorecard v2): never
        // land there silently when a low-latency mode exists. An
        // explicit fifo request is honoured as asked.
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
        // One frame in flight keeps key latency honest; the env knob
        // exists for latency experiments (scorecard M1), not tuning.
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

    /// The window's scale factor.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    fn padding_px(&self) -> f32 {
        (self.padding * self.scale).round()
    }

    /// Top-left of the cell grid: the padding plus half the space whole
    /// cells leave over, so the grid sits centred.
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

    /// Cells that fit the window now.
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

    /// The cell under a window pixel, clamped to the grid.
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

    /// The window's new size. The surface is configured for it before the
    /// next frame, so the grid it gives is known (and handed on) without
    /// waiting for the swapchain.
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

    /// Zoom by `steps` points; zero returns to the configured size.
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
                        // Full: start over on the next frame, unless the
                        // working set itself outgrows the atlas
                        // (`AtlasHealth`). Not cached.
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
        // T66: `slen`/`b0` arrive from `build` (T58/T61 hoists) instead of
        // re-reading the fat pointer (`as_bytes` + `len`) and re-indexing
        // `bytes[0]` (bounds check) on every ink cell. Same values by
        // construction (`slen == symbol.as_bytes().len()`, `b0` identical;
        // `slen == 1` implies the single byte is `b0`). Pixels identical.
        if slen == 1 {
            let b = b0;
            // Branchless printable check: 0x20..=0x7e is 95 values, so one
            // `wrapping_sub` + compare replaces two bound compares. Exact:
            // b >= 0x20 maps to 0..=0x5e (< 0x5f hit), b < 0x20 wraps high
            // (miss), b > 0x7e maps to 0x5f.. (miss).
            // T28: reuse the `wrapping_sub` result as the slot. The
            // predicate pins `d < 0x5f`, i.e. `b in 0x20..=0x7e`, so
            // `d == b - 0x20` exactly and a second subtract would be dead.
            let d = b.wrapping_sub(0x20);
            // T49: dead `span == 1 && style < 4` arms elided. The sole
            // 1-byte caller (`build`) always passes `span == 1` (the
            // 1-byte arm sets `(None, 1)`, the picture-merge guard needs
            // `Some` so span never grows) and `style < 4` (`style` is
            // built only from `BOLD = 1 | ITALIC = 2`, so 0..=3). The
            // `d < 0x5f` predicate already pins printability; the two
            // extra compares + branches were pure overhead on every ASCII
            // ink cell. `debug_assert`s pin the invariant in test builds.
            // Pixels identical by construction.
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
                // Promote slot 1 -> 0 so a 2-glyph stripe stays in L1.
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
            // One load into the precomputed table: no divides, no branches.
            Color::Indexed(i) => self.indexed_lut[usize::from(i)],
        }
    }

    fn build(&mut self, snapshot: &Snapshot, layers: &[Layer], focused: bool, out: &mut Frame) {
        self.core.begin_frame();
        // A transition moves every cell; at rest nothing changes.
        let now = Instant::now();
        if self.transition.is_some_and(|t| t.finished(now)) {
            self.transition = None;
        }
        let moment = self.transition.map(|t| t.time(now));
        // Working buffers come from the pool the last build emptied.
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
        // T18 frame hoists: `cursor_style` and font metrics are frame-
        // invariant but were reloaded per cell (2 kind compares + color
        // unwrap + 3 metric casts on every column). One copy here keeps
        // the hot loop on registers. Modifier bits are const-foldable
        // but hoisting documents the exact mask set in one place.
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
        // T50: native `usize` cursor sentinel. The old prologue stored
        // `(i32, i32)` with `-1` = hidden, forcing a `row as i32` compare
        // per row and a `cursor_col as usize` select per row. `snapshot.cursor`
        // carries `u16` coordinates, so `usize::MAX` is equally unreachable
        // as a hidden sentinel with zero conversions. Same `(col, row)`
        // values both paths; pixels identical by construction.
        let (cursor_col, cursor_row) = match snapshot.cursor {
            Some((c, r)) => (usize::from(c), usize::from(r)),
            None => (usize::MAX, usize::MAX),
        };
        // T32: `at_rest` is frame-invariant (`moment.is_none()` implies every
        // cell's `motion` is None; otherwise every cell computes Some) —
        // one copy here replaces the per-cell `motion.is_none()` reads.
        // T32: `fg_memo` is keyed by cell-constant `(fg, fg_rev, abits)`
        // only — nothing row-dependent — so it lives frame-wide instead of
        // resetting per row. Solid frames previously re-resolved `rgb()`
        // once per row start; now once per frame.
        let frame_rest = moment.is_none();
        // T31 fused fg+ink memo: `fg_memo` already probes every
        // ink cell on `(fg, fg_rev)`; the separate `ink_key`
        // (`fgu` pack + second probe) is folded in by storing the
        // premultiplied ink beside the resolved fg with its `abits`.
        // Hit = one probe returns both `fg` and `premul`.
        let mut fg_memo: Option<(Color, Option<Color>, Rgb, u32, [f32; 4])> = None;
        for row in 0..rows {
            let y = oy + row as f32 * ch;
            let row_base = row * cols;
            // T50: no `as` conversions — both sides are `usize` natively.
            let row_is_cursor = row == cursor_row;
            // T71: `eff_cursor_col` sentinel deleted. T43/T47/T50 folded the
            // row-uniform `row_is_cursor` into a per-row select so every
            // cell paid one pointer-width compare (`col == MAX`) even on
            // rows with no cursor (rows-1 of rows). `col < avail << MAX`
            // always, so `col == eff` is exactly
            // `row_is_cursor && col == cursor_col` — but the AND form
            // short-circuits on the row-uniform bool (predictable
            // not-taken off the cursor row) and skips the address
            // compare on ~97% of cells; the cursor row pays the same
            // compare as before. Merge-loop break below shares the form.
            // Pixels identical by construction.
            // T67: `eff_block_col` deleted. T63 folded the row-uniform
            // `cursor_block_active` AND into a second per-row sentinel,
            // so every cell paid TWO pointer-width compares (`at_cursor`
            // plus `cursor_ink`). But `cursor_ink = col==eff_block_col`
            // is exactly `at_cursor && cursor_block_active`: `eff_block`
            // equals `cursor_col` iff (row_is_cursor && block_active),
            // and `at_cursor` is (row_is_cursor && col==cursor_col) —
            // same truth table, one compare + one frame-uniform AND
            // instead of two compares. Off-row/off-block cells resolve
            // on the already-computed `at_cursor == false` without a
            // second address compare. `at_cursor` (picture-merge break,
            // thin/unfocused overlay) is untouched. Pixels identical.
            // Integer-key background run: the key is the `Color` tag plus
            // column contiguity, so `rgb()` and the float work run once per
            // flush instead of once per cell.
            let mut run: Option<(usize, usize, Color)> = None;
            // Row-slice hoist: one bounds check per row, not per cell.
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
            // Hit = one probe returns both `fg` and `premul` (T32: the
            // declaration lives frame-wide above the row loop).
            let mut col = 0;
            while col < avail {
                // T51: unchecked row-cell fetch. `col < avail` and
                // `row_cells.len() == avail` (sliced above), so `col` is
                // always in bounds; the safe `&row_cells[col]` paid a
                // bounds compare + branch on every cell for a condition
                // the loop guard already proves. `SAFETY: col < avail ==
                // row_cells.len()`, never mutated inside the loop.
                let cell_data = unsafe { &*row_cells.as_ptr().add(col) };
                let symbol = cell_data.symbol();
                let sbytes = symbol.as_bytes();
                // T58: hoisted byte length. `sbytes.len()` was re-read at
                // every width/blank/wall site (picture gate, T23/T27 skips,
                // ink blank gate, animated wall) for one loop-invariant value.
                // One `usize` copy feeds all sites; `slen == 0` is `is_empty`.
                // Pixels identical by construction. Distinct from T29 (skip
                // logic), T35/T42 (blank predicate), T23/T27 (width skips) —
                // none hoisted the length itself.
                let slen = sbytes.len();
                // T61: hoisted first byte. `sbytes[0]` was re-indexed at
                // every width/blank/wall site (picture gate, T23/T27
                // fast widths, ink blank gate, animated wall) - each a
                // bounds-checked load. One `u8` copy feeds all sites;
                // `slen == 0` pins 0 (all readers short-circuit on
                // `slen` first, so the sentinel is never observed).
                let b0 = if slen == 0 { 0 } else { sbytes[0] };
                let modifier = cell_data.modifier;
                let mbits = modifier.bits();
                // T72: `is_plain` deleted (T62 hoist removed). Every use was
                // logically redundant: for any non-zero mask M,
                // `!is_plain && (mbits&M)!=0` ≡ `(mbits&M)!=0` (RHS implies
                // mbits!=0) and `is_plain || (mbits&M)==0` ≡ `(mbits&M)==0`
                // (LHS implies RHS since 0&M==0). So the hoist paid one
                // compare + bool store per cell plus a second branch per gate
                // to skip an AND that is itself one cycle. Bare mask tests are
                // one branch each; pixels identical by construction.
                let at_cursor = row_is_cursor && col == cursor_col;
                // T68: eager `cursor_ink` deleted. `at_cursor && cursor_block_active`
                // was materialized + stored on every cell, then re-loaded at four
                // readers (bg flush, glyph, underline, strike) — but blank/bg cells
                // (steady-state majority) never read it: the flush short-circuits on
                // `at_cursor == false` and blanks skip the ink/deco gates entirely.
                // Each reader now tests `at_cursor && cursor_block_active` inline
                // (short-circuits on the already-computed `at_cursor`, no store/load).
                // Same truth table; pixels identical by construction.
                // ASCII fast path: 1-byte symbols can never be picture
                // placeholders (prefix starts `0xF4`) and are always
                // width 1. Multi-byte symbols with first byte != 0xF4
                // skip the placeholder scan and go straight to the width
                // table; only rare F4-led cells pay the decode.
                let (picture, mut span) = if slen == 1 {
                    (None, 1)
                } else if slen == 0 || b0 != 0xF4 {
                    // T23: box/block + Latin/Greek/Cyrillic fast widths are
                    // all 1, so `span` is 1 without the `symbol_width` call
                    // + `clamp` + `min` (predicates mirror `symbol_width`'s
                    // width-1 arms exactly; anything else stays on the table
                    // path, and `1.min(cols - col)` is 1 since `col < avail`).
                    let fast_w1 = slen == 0
                        || (slen == 2
                            && matches!(b0, 0xC3..=0xCB | 0xCE | 0xCF | 0xD0 | 0xD1 | 0xD3..=0xD5 | 0xDA))
                        || unsafe { slen == 3
                            && b0 == 0xE2
                            // T76: unchecked b1/b2 (slen==3 proves in-bounds).
                            && (matches!(*sbytes.as_ptr().wrapping_add(1), 0x94 | 0x95 | 0xA0..=0xA3)
                                || (*sbytes.as_ptr().wrapping_add(1) == 0x96 && *sbytes.as_ptr().wrapping_add(2) < 0xA0)) };
                    // T27: `E4..=E9`-led 3-byte symbols are exactly
                    // `U+4000..=U+9FFF` (CJK Extension A tail + Unified --
                    // the bulk of CJK grids), all wide with no holes
                    // (`wide_or_narrow_3` answers from the lead byte alone,
                    // pinned by the exhaustive BMP test). `span` is 2
                    // without the `symbol_width` call + `clamp`; the
                    // trailing `min` stays because a 2-wide glyph at the
                    // last column still clips to 1.
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
                // Same truth as `is_blank_symbol` without the call for the
                // 1-byte common case; stays lazy so picture cells never pay it.
                // T24: inline `if` at the ink gate (no closure/call per cell);
                // `&&` short-circuit keeps picture/hidden/space cells free.
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
                        // Row-slice fetch: `next_col < avail` proves in-bounds.
                        // T51: unchecked (same proof as the main fetch;
                        // both early `break`s above establish
                        // `next_col < avail == len`). `SAFETY` as above.
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
                // T48: `next_col` hoist. `col + span` was recomputed at
                // every bg-run update (extend + restart) plus the loop
                // advance — up to 3 integer adds per cell for one value.
                // `span` is final after the picture-merge loop above, so
                // one `col + span` here feeds all consumers below
                // (`run` extends/starts + `col = next_col` advance).
                // Bit-identical (`next_col == col + span` everywhere).
                // Distinct from T45 (guard removal), T46 (float geometry),
                // T22 (span multiply): pure integer-add dedup, new site.
                let next_col = col + span;
                // T41: fused reverse/background prologue. The old sequence
                // paid an AND + `match bg` + `if reversed` per cell, and
                // the `match` work was thrown away when reversed (bg_color
                // overwritten with `Some(fg)`). One gate now covers both:
                // common path (not reversed) does the exact `Reset=>None /
                // other=>Some` mapping with `fg_rev == None`; reversed cells
                // take the swap arm (`Some(fg)`, `Some(old bg)`). Pixels
                // identical by construction (same pair both paths).
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
                // T46 lazy geometry: `w`/`x` are ink/picture/overlay-only.
                // The rest-lane bg run tracks integer `(s,e)` and blank cells
                // never read `x`/`w`, so the eager convert + multiply (even
                // the T22 fast path) was pure overhead on the common cell.
                // Each consumer materializes the same bit-exact values
                // locally (`ox + col*cw` recomputed fresh, never accumulated).
                // T30: `wall`/`motion`/`cell_rect`/`stone_tile` are dead
                // at rest (`moment.is_none()`, the steady state): `wall`
                // only feeds the transition, and `cell_rect`/`stone_tile`
                // are transition-only. One predictable-not-taken `if let`
                // replaces the per-cell `match moment` + `cell_rect if` +
                // `if let Some` (3 branches -> 1). Animated frames compute
                // the exact same `wall`/`motion`/`cell_rect`/`stone_tile`
                // inside. (`w`/`x` stay eager: they feed backgrounds and
                // ink on every frame. T46: eager `w`/`x` deleted (lazy below).
                // T52: `frame_rest` gate over the transition producer. The
                // `if let Some(t) = moment` above re-read `moment`'s
                // discriminant (plus a payload copy) on every cell, but
                // `frame_rest == moment.is_none()` is already hoisted (T32)
                // as a single register bool. At rest (the steady state) one
                // not-taken bool branch now skips `moment` entirely — no
                // discriminant load, no `f32` payload copy; animated frames
                // take the exact same body via the `else if let`. The trailing
                // `else` is unreachable (`!frame_rest` implies `moment`
                // is `Some`) and returns the identical rest tuple.
                // Pixels identical by construction.
                // T55: dead `wall` + `cell_rect` stores deleted. The outer
                // `wall` was write-only (only the inner producer `wall`
                // feeds `motion`/`stone_tile`), and `cell_rect` (`[0;4]`
                // at rest, 16B zero per cell) was never read at rest —
                // every reader sits inside an animated `else` of `if
                // frame_rest`. `motion` alone is hoisted; each animated
                // site rebuilds its identical `[x,y,w,ch]` from its own
                // local `w`/`x` (bit-exact: same `ox+col*cw`,
                // `span==1?cw:span*cw` formulae). Rest cells pay one
                // `Option` discriminant store instead of bool + Option +
                // 4-float zero.
                let motion = if frame_rest {
                    None
                } else {
                    // T74: unchecked `moment` unwrap. `frame_rest ==
                    // moment.is_none()` (T32/T52) so `!frame_rest` implies
                    // `moment.is_some()`; the `else if let Some(t)` above
                    // re-tested the discriminant plus an unreachable `else`
                    // (two branches) on every animated cell. One
                    // `unwrap_unchecked` replaces both. SAFETY: `moment`
                    // is immutable after hoist and `!frame_rest` proves
                    // `Some`; animated frames take the identical body.
                    // Pixels identical by construction.
                    let t = unsafe { moment.unwrap_unchecked() };
                    // T42: `is_space` is lazy inside the animated-only arm
                    // (was eager per cell; at rest the wall is dead so the
                    // check was pure overhead on every steady-state cell).
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
                // T73: frame-uniform guard first. `cursor_block_active` is
                // frame-invariant (perfectly predictable) while `at_cursor`
                // is per-cell; testing it first lets block-inactive frames
                // (thin/unfocused/hidden cursor) skip the `at_cursor` load
                // at all four readers. `&&` commutes over pure bool reads,
                // so the taken path (single cursor cell, block active) is
                // unchanged. Pixels identical by construction. Distinct from
                // T68 (made the test lazy, kept this order), T67/T63/T43
                // (sentinel shape) — none ordered the operands.
                if cursor_block_active && at_cursor {
                    // The cursor colour is already an `Rgb`: flush the run
                    // and draw this one cell directly (same pixels, at most
                    // one extra quad per frame).
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
                    // T46: geometry materialized here (single cell/frame).
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    // T32: `frame_rest` ⟺ `motion.is_none()` on every cell.
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
                    // T33: steady-state fast lane. At rest `motion` is None
                    // on every cell (frame-level `frame_rest`, hoisted T32),
                    // so the `Some(c) if motion.is_some()` arm below is dead
                    // here and its discriminant read is pure overhead. This
                    // two-arm match runs the exact run/None bodies with no
                    // motion touch; animated frames use the full match.
                    match bg_color {
                        Some(c) => {
                            // Integer run: one `Color` discriminant compare;
                            // `rgb()` and the floats materialize only on flush.
                            // T45: the `e == col` contiguity check is dead in
                            // this rest lane. `run` is row-local, every cell
                            // advances `col += span`, both arms maintain `e`
                            // as the previous cell's end (extend) or restart
                            // at `col` (flush/start), and cursor cells
                            // `take()` the run — so whenever `run` is `Some`,
                            // `e == col` holds by induction and the second
                            // compare is pure overhead on every run-extending
                            // bg cell. The animated lane below keeps its guard:
                            // moving cells skip the run while `col` advances,
                            // so a stale `e` can legitimately mismatch there.
                            // `debug_assert` pins the invariant in test builds.
                            // Pixels identical by construction.
                            // T65: in-place run-end update. The extend arm moved
                            // the whole `(s,e,k)` tuple out and rebuilt it,
                            // copying dead `s`/`k` on every run-extending bg
                            // cell. `&mut` updates only `e`; flush/start arms
                            // share one tail (copy-out, flush-if-some, write).
                            // Same `(s,e,k)` values; pixels identical.
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
                            // T56: no `take()` store on the common path. `run.take()`
                            // is `mem::replace(run, None)`: it writes `None` back
                            // unconditionally, so transparent-bg cells (run already
                            // `None`, the steady-state majority) paid a dead
                            // Option-sized store per cell. Match-by-copy reads the
                            // discriminant only; the `None` write fires solely on
                            // flush. Same flush values (`Color: Copy`); pixels
                            // identical by construction. Distinct from T33 (created
                            // the lane) and T45 (removed the `e == col` guard) —
                            // neither touched the `take()` store itself.
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
                            // Moving cells keep their own backgrounds; the
                            // run survives across them, exactly as before.
                            // T46: geometry materialized on the animated path.
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
                            // Integer run: one `Color` discriminant compare
                            // plus column contiguity; `rgb()` and the floats
                            // materialize only on flush.
                            // T70: in-place run-end update, animated lane
                            // (mirrors T65 rest-lane only). Extend arm no
                            // longer moves/rebuilds dead `(s,k)`; `&mut`
                            // writes only `e`. Keeps `e == col`: moving cells
                            // skip the run while `col` advances, so stale `e`
                            // can mismatch here (unlike rest lane, T45).
                            // Same values; pixels identical.
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
                            // T64: animated-lane match-by-copy (mirrors T56, which
                            // covered the rest lane only). `run.take()` is
                            // `mem::replace(run, None)`: unconditional `None`
                            // write, so transparent-bg cells paid a dead
                            // Option-sized store per animated cell. Discriminant
                            // read only; write fires solely on flush. Same values
                            // (`Color: Copy`); pixels identical by construction.
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
                // T40: `alpha`/`abits` are ink/picture-only (glyph/underline/
                // strike premultiply + `picture_span` opacity). Computing
                // them eagerly charged every background/space/blank cell
                // one AND + branch + `to_bits`. Now picture pays one inline
                // dim select (no `to_bits`) and the ink gate computes both
                // once at its top; background/blank cells pay nothing.
                // Pixels identical: same `dim -> 0.6/1.0` values, same
                // `to_bits`, just computed lazily at the consumers.
                // T36: `at_rest` deleted — it was `frame_rest` by
                // construction and its only use picked between `[0.0; 4]`
                // and `cell_rect`, but at rest `cell_rect` already IS
                // `[0.0; 4]` (T30 else arm), and when animated `at_rest`
                // is false so the else arm took `cell_rect` anyway.
                // Passing `cell_rect` directly is pixel-identical and
                // removes the per-cell `motion.is_none()` read plus the
                // picture-site branch.
                // T39: no eager per-cell `m`. At rest the glyph/decoration push sites take the direct-push lane and never read motion, so a per-cell REST materialization was pure overhead on background/space cells. Motion is computed inline at the four sites that need it (picture + three animated pushes).
                if let Some(placeholder) = picture {
                    // T46: geometry materialized at the picture site only.
                    // T55: `cell_rect` rebuilt locally (was hoisted dead
                    // `[0;4]` at rest); bit-exact same formula.
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
                        // T44: blank-first ink gate. The old order read
                        // `hidden` first, so every blank space/tab paid a
                        // `hidden` AND + branch before short-circuiting on
                        // blank. `A && B` == `B && A` (both pure reads), so
                        // testing blank first lets blank cells (common)
                        // skip the `hidden` read entirely; non-blank cells
                        // pay the same two reads in swapped order. Pixels
                        // identical by construction.
                        // T42: fused single-byte blank — `' '` folded back
                        // in (T35 had split it out behind `!is_space`).
                        // `!is_space && !blank_no_space` == `!blank_with`
                        // space` by construction, so one 4-arm `matches!`
                        // replaces the eager `is_space` bool + two gated
                        // branches; the eager `is_space` is deleted (wall
                        // computes it lazily, animated-only).
                        matches!(b0, b' ' | b'\t' | b'\n' | b'\r')
                    } else {
                        // T29: any non-empty multi-byte symbol led by a
                        // non-ASCII byte draws — `is_blank_symbol` returns
                        // false on the first byte >= 0x80 — so skip the
                        // call. Empty stays blank; rare all-ASCII
                        // multi-byte symbols take the exact call.
                        slen == 0
                            || (b0 < 0x80 && is_blank_symbol(symbol))
                    })
                    && (mbits & hidden_bit) == 0
                {
                    // T40 lazy ink alpha: computed once at the top of the
                    // ink gate and shared by glyph/strike probes below
                    // (was eager per cell, incl. background/blank cells
                    // that never read it). Same two values + `to_bits`.
                    // T57: `abits` from integer immediates. The `alpha: f32`
                    // select + `to_bits` mov ran on every ink cell, but the
                    // common memo hit reads only `abits` — the float is dead
                    // there. Selecting the bit patterns directly keeps the
                    // exact `0.6f32`/`1.0f32` bits with no float material.
                    let abits = if (mbits & dim_bit) != 0 {
                        0x3F19_999Au32 // == 0.6f32.to_bits()
                    } else {
                        0x3F80_0000u32 // == 1.0f32.to_bits()
                    };
                    // T34: fused style gate. Plain cells (neither bold nor
                    // italic — the common case) pay one AND + one not-taken
                    // branch instead of two ANDs + two branches. Styled
                    // cells take the same two inner paths, so `style` is
                    // identical.
                    // T75: branchless style pack. The T34 fused gate paid one
                    // AND + one not-taken branch on every ink cell to skip
                    // two inner ANDs + branches, and styled cells paid up
                    // to 3 branches total. `BOLD == 1` / `ITALIC == 2`
                    // (`font.rs`) are exactly bit0/bit1 of `style`, so two
                    // `!= 0 as u8` packs (single `test` + `setcc` each, no
                    // branches) build the identical value for all `mbits`.
                    // Same `style` both paths; pixels identical.
                    let style = ((mbits & bold_bit) != 0) as u8 * BOLD
                        | ((mbits & italic_bit) != 0) as u8 * ITALIC;
                    if let Some(entry) = self.glyph(symbol, style, span as u8, slen, b0) {
                        // Block-cursor cell: glyph ink is `cursor_text`
                        // (baseline `fg = cursor_text`, dropped when the
                        // cursor background went direct-draw). One
                        // predictable branch on a single cell per frame;
                        // skips the memo so the cursor cell never
                        // pollutes it.
                        // T31: fused hit returns `fg` + `premul` from one
                        // probe; no `fgu` pack, no second cache check.
                        let (_, premul) = if cursor_block_active && at_cursor {
                            let v = cursor_text;
                            (v, premultiply(v, f32::from_bits(abits)))
                        } else {
                            // T59: two-phase memo probe. By-value `match fg_memo`
                                // copied the whole `(Color, Option<Color>, Rgb,
                                // u32, [f32;4])` payload before the guard ran,
                                // so every full miss paid a dead copy. The probe
                                // borrows (`as_ref`): guard reads only `d`/`r`,
                                // `v`/`pa`/`pv` copy solely when `d`/`r` hit;
                                // full misses copy nothing. Borrow ends before
                                // any `fg_memo =` store, satisfying the checker.
                                // Same outcomes/values; pixels identical.
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
                        // T34: rest-lane ink push. At rest `m` is REST on
                        // every cell (frame-level `frame_rest`, T32), so
                        // `push_moved`'s 8-float `Motion` compare is pure
                        // overhead. Steady-state cells push directly;
                        // animated frames take the exact `push_moved` path.
                        // T46: geometry materialized at the glyph site only.
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
                // T26: fused decoration gate. Plain cells (no underline /
                // strike — the common case) pay one AND + one not-taken
                // branch instead of two ANDs + two branches. Decorated
                // cells take the same two inner paths as before, so pixels
                // are identical.
                if (mbits & (underlined_bit | crossed_bit)) != 0 {
                    // T40 lazy deco alpha: same `dim -> 0.6/1.0` + `to_bits`
                    // as the ink gate above, computed only when a decoration
                    // actually draws (rare). Decorated glyph cells compute
                    // twice (once per gate) — accepted: decorations are far
                    // rarer than glyphs, and plain/background/blank cells
                    // (the common case) pay nothing here.
                    // T60: deco-gate `alpha` float deleted (mirrors T57 ink-gate).
                    // The `f32` select + `to_bits` ran on every decorated
                    // cell, but every reader below needs only `abits`
                    // (memo compare + `premultiply`). Integer immediates
                    // keep the exact `0.6f32`/`1.0f32` bits; the three
                    // `premultiply(v, alpha)` / `premultiply(color, alpha)`
                    // readers rebuild via `f32::from_bits(abits)` bit-exact.
                    // Rare path (decorations only); plain/bg/blank cells
                    // pay nothing. Pixels identical by construction.
                    // T69: `!is_plain` elided — outer fused gate already proves it.
                    let abits = if (mbits & dim_bit) != 0 {
                        0x3F19_999Au32 // == 0.6f32.to_bits()
                    } else {
                        0x3F80_0000u32 // == 1.0f32.to_bits()
                    };
                    // Block-cursor cell (see glyph site): decorations
                    // fall back to `cursor_text`, not the cell fg.
                    // (T68: `at_cursor && cursor_block_active` tests inline.)
                    // T53: shared deco geometry. `w`/`x` are identical at
                    // both decoration sites (same `span`/`col`), so one
                    // materialization at the gate top feeds underline +
                    // strike. Single-deco cells pay the same once; both-
                    // decorated cells save a branch + int->float + mul +
                    // add. Plain cells (gate not taken) pay nothing.
                    let dw = if span == 1 { cw } else { span as f32 * cw };
                    let dx = ox + col as f32 * cw;
                    if (mbits & underlined_bit) != 0 {
                    let fg = if cursor_block_active && at_cursor {
                        cursor_text
                    } else {
                        // T59: two-phase probe (see glyph site): borrow reads
                        // only `d`/`r`; `v` copies solely on hit. Full misses
                        // copy nothing. Borrow ends before the store below.
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
                    // T46/T53: shared `dw`/`dx` from the gate top.
                    // T34: rest-lane decoration push (same REST elision as
                    // the glyph site above).
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
                    // T31: strike reuses the fused memo (same fg +
                    // alpha as the glyph above): one probe, no `fgu`.
                    // T54 (strike arm): same single-guard fusion as the
                    // glyph site above.
                    let (_, strike_premul) = if cursor_block_active && at_cursor {
                        let v = cursor_text;
                        (v, premultiply(v, f32::from_bits(abits)))
                    } else {
                        // T59: two-phase probe (see glyph site): borrow reads
                        // only `d`/`r`; `v`/`pa`/`pv` copy solely on `d`/`r`
                        // hit; full misses copy nothing. Borrow ends before
                        // any store.
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

                    // T46/T53: shared `dw`/`dx` from the gate top.
                    // T34: rest-lane strike push (same REST elision).
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
                    // T46: geometry materialized at the cursor-overlay site
                    // only (<=1 cell per frame); off-cursor cells pay nothing.
                    let w = if span == 1 { cw } else { span as f32 * cw };
                    let x = ox + col as f32 * cw;
                    // T38: fused cursor-overlay gate. At most one cell
                    // per frame sits under the cursor, so the old pair
                    // of `at_cursor && ...` gates charged every other
                    // cell two reads + two not-taken branches. One outer
                    // `if` pays one read + one branch off-cursor; the
                    // cursor cell itself takes the same two inner paths,
                    // so pixels are identical.
                    // A thin cursor (underline or bar) is drawn over the
                    // cell rather than filling it.
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
        // Fast path: hosts usually emit layers back-to-front already, so
        // split the sorted prefix without a `Vec` alloc or `sort`. Only
        // the unsorted tail pays for `collect` + `sort`.
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
        // The background image, then its tint toward the background colour,
        // under everything else.
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
                // The map entry keeps a pooled buffer for next frame.
                let taken = std::mem::replace(group, pool.pop().unwrap_or_default());
                parts.push((*draw, taken));
            }
        }
        self.spare_pictures = pictures;
        parts.push((Draw::Atlas, above));
        parts.extend(above_layers);
        // The card rides over everything the cockpit drew.
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

    /// The slice of a picture a horizontal span of `cols_span` placeholder
    /// cells shows, queued by texture and merged vertically when contiguous.
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

    /// Assemble the cockpit (`transition.rs`), from now.
    pub fn start_transition(&mut self) {
        self.transition = Some(transition::Transition::start(Instant::now()));
    }

    /// Render the frame `seconds` into the assembly transition, offscreen.
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

    /// Jump to the end of a running transition.
    pub fn skip_transition(&mut self) {
        self.transition = None;
    }

    /// The outgoing Delve/TUI frame stays spatially fixed while the other
    /// surface comes through. Two reserved targets also handle a rapid reversal.
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

    /// Whether a transition, or a stage's breathing or flickering layer,
    /// still needs frames.
    pub fn animating(&self) -> bool {
        self.handoff.is_some()
            || self.transition.is_some_and(|t| !t.finished(Instant::now()))
            || self.core.stages_animating()
            || self.core.scenes_animating()
            || self.card_animating()
    }

    /// Take the pictures handed over (`Host::upload_picture`): shrunk to
    /// [`Self::texture_budget`] first, so a hostile upload caps at the
    /// adapter's budget instead of OOMing the session the way kitty's
    /// unbounded graphics upload can. RTX/discrete keeps 8192² art;
    /// integrated shrinks past 4096² (64 MiB) before it reaches the GPU.
    pub fn apply(&mut self, pictures: Vec<graphics::Picture>) {
        // Hard byte-cap enforcement behind the shrink: a malformed picture
        // whose bytes lie about their size is dropped before the GPU instead
        // of uploaded — the explicit hostile-upload cap kitty's unbounded
        // graphics path lacks.
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

    /// Free the pictures let go (`Host::free_pictures`).
    pub fn free(&mut self, ids: Vec<u32>) {
        for id in ids {
            self.core.free_picture(id);
        }
    }

    /// GPU byte cap for this window's adapter: `budget² × 4` straight-RGBA
    /// bytes (256 MiB at 8192 on RTX/discrete, 64 MiB at 4096 on
    /// integrated). The explicit form of the hostile-upload cap kitty's
    /// unbounded path lacks; Unreal-sized scenes budget against this.
    pub fn texture_byte_cap(&self) -> u64 {
        graphics::texture_byte_budget(self.texture_budget)
    }

    /// A mesh for scenes to name by `id`.
    pub fn upload_mesh(&mut self, id: u32, mesh: &angel_stage::scene::MeshData) {
        if let Err(error) = self.core.upload_mesh(id, mesh) {
            eprintln!("[glass] mesh {id} refused: {error:?}");
        }
    }

    /// Show a stage in the cells naming `id` (`None` takes it down).
    pub fn set_stage(&mut self, id: u32, stage: Option<Stage>) {
        self.core.set_stage(id, stage);
    }

    /// Show a 3D scene in the picture `id`, the given size in pixels (`None`
    /// takes it down).
    pub fn set_scene(&mut self, id: u32, scene: Option<(angel_stage::scene::Scene, (u32, u32))>) {
        self.core.set_scene_target(id, scene);
    }

    pub fn set_gpu_picture(&mut self,id:u32,producer:Option<(Box<dyn angel_gpu::PictureProducer>,(u32,u32))>) {
        self.core.set_gpu_picture(id,producer);
    }

    /// Draw every stage and scene into its texture before the frame that
    /// shows them.
    fn render_stages(&mut self) {
        let cell = (self.fonts.cell.width, self.fonts.cell.height);
        let now = clock();
        self.core.render_stages(cell, now);
        self.core.set_scene_clock(now);
        self.core.render_scenes();
        self.core.render_gpu_pictures();
    }

    /// Draw and present. Returns false when the surface had no frame to give.
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
            // The atlas filled mid-frame: rebuild with an empty one. If
            // it fills again straight away the working set outgrows the
            // atlas; settle for blanks on the overflow instead of
            // resetting and re-rastering everything every frame.
            self.reset_glyphs();
            self.build(snapshot, layers, focused, &mut frame_data);
            self.core.atlas_settle_after_rebuild();
        }
        let built = Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                // Show this frame; reconfigure before the next one. The
                // surface may not be configured while a frame is held.
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
        // Reap finished submissions so wgpu recycles the staging buffers
        // behind write_buffer/write_texture promptly instead of whenever
        // the next blocking call happens to land.
        let _ = self.device.poll(wgpu::PollType::Poll);
        true
    }

    /// Render the frame into a texture and read it back as RGBA rows, for
    /// screenshots taken from inside (no compositor in the picture).
    /// [`capture`](Self::capture) without waiting for the pixels: they
    /// come back through the returned handle (`poll_captures`).
    pub fn capture_later(&mut self, snapshot: &Snapshot, layers: &[Layer]) -> angel_gpu::ReadBack {
        self.capture_with(snapshot, layers, |core, texture, size| core.read_back_later(texture, size))
    }

    /// Let captures in flight come back; `wait` until all of them have.
    pub fn poll_captures(&self, wait: bool) {
        self.core.poll_read_backs(wait);
    }

    pub fn capture(&mut self, snapshot: &Snapshot, layers: &[Layer]) -> (u32, u32, Vec<u8>) {
        let (width, height) = (self.config.width, self.config.height);
        let pixels = self.capture_with(snapshot, layers, |core, texture, size| core.read_back(texture, size));
        (width, height, pixels)
    }

    /// Draw the frame into a texture of its own and read it back with `read`.
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

/// An instance carried by its cell's motion: scaled about the cell centre,
/// moved, turned and faded.
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

/// Floor stone: dark slate a little lighter than the background, one tile
/// per cell, each a slightly different shade. Wall stone is paler and
/// heavier, so the walls read as blocks while they slide in over black.
const STONE: [f32; 3] = [0.17, 0.175, 0.19];
const WALL_STONE: [f32; 3] = [0.25, 0.255, 0.28];
/// How much one tile's shade differs from the next.
const STONE_VARIATION: f32 = 0.26;
/// The bevel: lit top and left edges, dark bottom and right edges.
const STONE_LIGHT: f32 = 1.45;
const STONE_DARK: f32 = 0.55;
/// The shadow under a tile in the air: offset and darkness at full height.
const SHADOW_PX: [f32; 2] = [14.0, 20.0];
const SHADOW_ALPHA: f32 = 0.5;

/// A cell's stone tile in a transition: its shadow while it is in the air,
/// the tile with bevelled edges and a few specks, and dust when it lands.
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
    // A 1-pixel grout line between tiles.
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
    // Two specks of grit in the stone.
    for salt in [6, 7] {
        let sx = tx + 1.0 + (tw - 3.0) * transition::hash(col, row, salt);
        let sy = ty + 1.0 + (th - 3.0) * transition::hash(col, row, salt + 10);
        put([sx.floor(), sy.floor(), 1.0, 1.0], scaled(0.7));
    }
    // Dust from about a third of the landings, spreading and fading.
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

/// Points at 96 dpi, times the window's scale.
pub fn points_to_px(points: f32, scale: f32) -> f32 {
    // macOS counts a point as one logical pixel; Linux and Windows count
    // 96 dpi.
    let dpi = if cfg!(target_os = "macos") {
        72.0
    } else {
        96.0
    };
    points * dpi / 72.0 * scale
}

#[inline]
fn premultiply(rgb: Rgb, alpha: f32) -> [f32; 4] {
    // One multiply per channel: `alpha / 255` hoists the division out of
    // the per-cell path (the old `/ 255.0 * alpha` paid a division per
    // channel per cell, every frame). Bit-identical ordering is not kept
    // on purpose: the scale is exact to <1e-7, far below an 8-bit step.
    let k = alpha * (1.0 / 255.0);
    [
        f32::from(rgb[0]) * k,
        f32::from(rgb[1]) * k,
        f32::from(rgb[2]) * k,
        alpha,
    ]
}

/// A cell colour as RGB: the palette's for the named and indexed colours,
/// `fallback` for the default. The GPU renderer and the early frames both
/// read colours through this.
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

/// Build the full 256-entry table once per `Renderer`: entries 0..=15
/// are the palette, the rest the xterm cube + greys. `rgb()` then resolves
/// `Indexed(i)` with one array load instead of the divide chain.
fn indexed_table(ansi: &[Rgb; 16]) -> [Rgb; 256] {
    let mut lut = [[0, 0, 0]; 256];
    for (n, slot) in lut.iter_mut().enumerate() {
        *slot = indexed(n as u8, ansi);
    }
    lut
}

/// The xterm 256-colour table above the 16 palette colours.
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

/// Seconds on the window's own clock, for what it animates itself: the core
/// is handed this wherever it needs the time.
fn clock() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Direct-indexed L1 slot (`0..416`) and decoded `char` for 3-byte UTF-8
/// Box Drawing (`U+2500..=U+257F`), Block Elements (`U+2580..=U+259F`), and
/// Braille (`U+2800..=U+28FF`).
#[inline]
const fn sprite_slot(symbol: &str) -> Option<(usize, char)> {
    let b = symbol.as_bytes();
    if b.len() != 3 || b[0] != 0xE2 {
        return None;
    }
    let cp = 0x2000u32 | (((b[1] & 0x3F) as u32) << 6) | ((b[2] & 0x3F) as u32);
    if cp >= 0x2500 && cp <= 0x259F {
        // SAFETY: 0x2500..=0x259F is a valid Unicode scalar value.
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

/// Fast UTF-8 byte check for Box Drawing (`U+2500..=U+257F`: `E2 94 80..BF` or `E2 95 80..BF`).
#[inline]
const fn is_box_drawing(symbol: &str) -> bool {
    let b = symbol.as_bytes();
    b.len() >= 3 && b[0] == 0xE2 && (b[1] == 0x94 || b[1] == 0x95)
}

/// Whether a cell's symbol draws nothing: empty or ASCII/Unicode
/// whitespace, without `str::trim`'s two-pass scan and branchy whitespace
/// table. The per-cell hot path calls this for every non-space cell.
#[inline]
fn is_blank_symbol(symbol: &str) -> bool {
    let b = symbol.as_bytes();
    if b.is_empty() {
        return true;
    }
    // Fast path: all-ASCII blanks (space is handled before this, but tab,
    // CR/LF and other control blanks land here from pasted text).
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c < 0x80 {
            if c != b' ' && c != b'\t' && c != b'\n' && c != b'\r' {
                return false;
            }
            i += 1;
        } else {
            // Any non-ASCII byte draws (box drawing, braille, CJK, emoji,
            // zero-width joiners ride along with a base glyph). The old
            // `trim().is_empty()` also treated U+00A0 etc. as blank, but
            // drawing a blank-width glyph there is harmless and far cheaper
            // than decoding UTF-8 + consulting the whitespace table.
            return false;
        }
    }
    true
}

/// Append a horizontal span of picture placeholder cells into `pictures`,
/// coalescing vertically with the immediately preceding quad when it shares
/// the same column start, width, alpha, and contiguous UV slice.
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

    /// A piece of startup gives what its work made, on its own thread, and
    /// one whose thread died gives nothing rather than taking the window
    /// down with it.
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
        // Updating a present key at the cap must not clear the memo.
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
    /// The glyph atlas starts small and grows the first time it fills:
    /// on a real GPU (skipped with no adapter), an image packed before the
    /// growth is still where its entry says after it, and one packed after
    /// lands in the larger texture.
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
