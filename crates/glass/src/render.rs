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
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
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
    strings: FastMap<(u64, u8, u8), (Box<str>, Option<Entry>)>,
    fonts: Fonts,
    /// The `/setup` scroll's TRELLIS rollers, once loaded (`render_scroll_mesh.rs`).
    scroll_mesh: Option<ScrollMesh>,
    scroll_mesh_attempted: bool,
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
    pub present_mode: wgpu::PresentMode,
    pub alpha_mode: wgpu::CompositeAlphaMode,
}

impl Renderer {
    pub fn new(window: Arc<Window>, options: &Options) -> Result<Self, String> {
        let scale = window.scale_factor() as f32;
        let fonts = Fonts::load(
            &options.font_family,
            points_to_px(options.font_size, scale),
            points_to_px(1.0, scale),
        )?;
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| format!("window surface: {e}"))?;
        let power_preference = match std::env::var("ANGEL_GLASS_GPU").as_deref() {
            Ok("low") => wgpu::PowerPreference::LowPower,
            Ok("high") => wgpu::PowerPreference::HighPerformance,
            _ => wgpu::PowerPreference::None,
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter for the window: {e}"))?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glass-device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| format!("GPU device: {e}"))?;
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
        let present_mode = config.present_mode;
        let mut core = Core::new(device.clone(), queue.clone(), view_format, options.text_gamma);
        if let Some(image) = options
            .wallpaper()
            .and_then(|settings| crate::wallpaper::load(&settings))
        {
            core.set_wallpaper(image);
        }
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
            reconfigure: false,
            timing: Default::default(),
            spare_parts: Vec::new(),
            spare_pictures: Default::default(),
            ascii_chars: Box::new([[None; 95]; 4]),
            sprite_chars: Box::new([None; 416]),
            chars: FastMap::default(),
            strings: FastMap::default(),
            fonts,
            scroll_mesh: None,
            scroll_mesh_attempted: false,
            palette: options.palette.clone(),
            opacity: options.background_opacity,
            padding: options.padding,
            braille: options.braille,
            font_points: options.font_size,
            base_points: options.font_size,
            scale,
            overlay: Vec::new(),
            adapter_name: format!("{} ({:?})", info.name, info.backend),
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

    /// A window size in pixels holding `cols × rows` cells.
    pub fn pixels_for(&self, cols: u16, rows: u16) -> (u32, u32) {
        let cell = self.fonts.cell;
        let pad = self.padding_px() as u32 * 2;
        (
            u32::from(cols) * cell.width + pad,
            u32::from(rows) * cell.height + pad,
        )
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
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
    fn glyph(&mut self, symbol: &str, style: u8, span: u8) -> Option<Entry> {
        let bytes = symbol.as_bytes();
        if bytes.len() == 1 {
            let b = bytes[0];
            if (0x20..=0x7e).contains(&b) && span == 1 && (style as usize) < 4 {
                let slot = (b - 0x20) as usize;
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
            if let Some(found) = self.chars.get(&(first, style, span)) {
                return *found;
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

    fn rgb(&self, color: Color, fallback: Rgb) -> Rgb {
        let ansi = &self.palette.ansi;
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
        let palette = self.palette.clone();
        let mut backgrounds = pool.pop().unwrap_or_default();
        let mut glyphs = pool.pop().unwrap_or_default();
        glyphs.reserve(cols * rows / 2);
        let mut overlays = pool.pop().unwrap_or_default();
        for row in 0..rows {
            let y = oy + row as f32 * ch;
            let row_base = row * cols;
            let mut run: Option<(f32, f32, [f32; 4])> = None;
            let mut col = 0;
            while col < cols {
                let Some(cell_data) = snapshot.cells.get(row_base + col) else {
                    break;
                };
                let symbol = cell_data.symbol();
                let is_space = symbol.as_bytes() == b" ";
                let modifier = cell_data.modifier;
                let at_cursor = snapshot.cursor == Some((col as u16, row as u16));
                let picture = (!is_space && graphics::is_placeholder(symbol))
                    .then(|| graphics::read_placeholder(symbol, cell_data.fg))
                    .flatten();
                let mut span = if is_space || picture.is_some() {
                    1
                } else {
                    crate::backend::symbol_width(symbol).clamp(1, 2).min(cols - col)
                };
                if let Some(p0) = picture
                    && moment.is_none()
                    && !at_cursor
                    && !modifier.intersects(
                        Modifier::REVERSED
                            | Modifier::HIDDEN
                            | Modifier::UNDERLINED
                            | Modifier::CROSSED_OUT,
                    )
                {
                    while col + span < cols {
                        let next_col = col + span;
                        if snapshot.cursor == Some((next_col as u16, row as u16)) {
                            break;
                        }
                        let Some(next_cell) = snapshot.cells.get(row_base + next_col) else {
                            break;
                        };
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
                let x = ox + col as f32 * cw;
                let w = span as f32 * cw;
                let mut fg = self.rgb(cell_data.fg, palette.foreground);
                let mut bg = match cell_data.bg {
                    Color::Reset => None,
                    other => Some(self.rgb(other, palette.background)),
                };
                if modifier.contains(Modifier::REVERSED) {
                    let swapped = bg.unwrap_or(palette.background);
                    bg = Some(fg);
                    fg = swapped;
                }
                let (wall, motion) = match moment {
                    Some(t) => {
                        let wall = !is_space && is_box_drawing(symbol);
                        (
                            wall,
                            Some(transition::motion(
                                t,
                                transition::Place {
                                    col: col as u16,
                                    row: row as u16,
                                    cols: cols as u16,
                                    rows: rows as u16,
                                    cell_height: ch,
                                    wall,
                                },
                            )),
                        )
                    }
                    None => (false, None),
                };
                let cell_rect = [x, y, w, ch];
                if let Some(m) = motion {
                    stone_tile(
                        &mut tiles,
                        &mut dust,
                        m,
                        cell_rect,
                        palette.background,
                        (col as u16, row as u16),
                        wall,
                    );
                }
                if at_cursor && focused {
                    bg = Some(palette.cursor);
                    fg = palette.cursor_text;
                }
                match bg {
                    Some(rgb) if motion.is_some() => {
                        // Moving cells keep their own backgrounds.
                        let m = motion.unwrap_or(transition::Motion::REST);
                        backgrounds.push(moved(
                            Instance::solid(x, y, w, ch, premultiply(rgb, 1.0)),
                            m,
                            cell_rect,
                        ));
                    }
                    Some(rgb) => {
                        let color = premultiply(rgb, 1.0);
                        run = match run {
                            Some((x0, x1, c)) if c == color && (x1 - x).abs() < 0.5 => {
                                Some((x0, x + w, c))
                            }
                            Some((x0, x1, c)) => {
                                backgrounds.push(Instance::solid(x0, y, x1 - x0, ch, c));
                                Some((x, x + w, color))
                            }
                            None => Some((x, x + w, color)),
                        };
                    }
                    None => {
                        if let Some((x0, x1, c)) = run.take() {
                            backgrounds.push(Instance::solid(x0, y, x1 - x0, ch, c));
                        }
                    }
                }
                let alpha = if modifier.contains(Modifier::DIM) {
                    0.6
                } else {
                    1.0
                };
                let m = motion.unwrap_or(transition::Motion::REST);
                if let Some(placeholder) = picture {
                    self.picture_span(
                        placeholder,
                        span as u16,
                        x,
                        y,
                        alpha,
                        &mut pictures,
                        m,
                        cell_rect,
                    );
                } else if !is_space
                    && !modifier.contains(Modifier::HIDDEN)
                    && !symbol.trim().is_empty()
                {
                    let mut style = 0;
                    if modifier.contains(Modifier::BOLD) {
                        style |= BOLD;
                    }
                    if modifier.contains(Modifier::ITALIC) {
                        style |= ITALIC;
                    }
                    if let Some(entry) = self.glyph(symbol, style, span as u8) {
                        glyphs.push(moved(
                            Instance {
                                rect: [x + entry.left, y + entry.top, entry.uv[2], entry.uv[3]],
                                uv: entry.uv,
                                color: premultiply(fg, alpha),
                                kind: if entry.colored { COLOR } else { MASK },
                                pivot: [0.0; 2],
                                angle: 0.0,
                                ..Zeroable::zeroed()
                            },
                            m,
                            cell_rect,
                        ));
                    }
                }
                let line = cell.underline_height as f32;
                if modifier.contains(Modifier::UNDERLINED) {
                    let color = self.rgb(cell_data.underline_color, fg);
                    overlays.push(moved(
                        Instance::solid(
                            x,
                            y + cell.underline_y as f32,
                            w,
                            line,
                            premultiply(color, alpha),
                        ),
                        m,
                        cell_rect,
                    ));
                }
                if modifier.contains(Modifier::CROSSED_OUT) {
                    overlays.push(moved(
                        Instance::solid(
                            x,
                            y + cell.strike_y as f32,
                            w,
                            line,
                            premultiply(fg, alpha),
                        ),
                        m,
                        cell_rect,
                    ));
                }
                if at_cursor && !focused {
                    let c = premultiply(palette.cursor, 1.0);
                    overlays.push(Instance::solid(x, y, w, 1.0, c));
                    overlays.push(Instance::solid(x, y + ch - 1.0, w, 1.0, c));
                    overlays.push(Instance::solid(x, y, 1.0, ch, c));
                    overlays.push(Instance::solid(x + w - 1.0, y, 1.0, ch, c));
                }
                col += span;
            }
            if let Some((x0, x1, c)) = run {
                backgrounds.push(Instance::solid(x0, y, x1 - x0, ch, c));
            }
        }
        let mut sorted: Vec<&Layer> = layers.iter().collect();
        sorted.sort_by_key(|layer| layer.z);
        let time = clock();
        let below_layers = self.core.layer_parts(sorted.iter().copied().filter(|l| l.z < 0), (ox, oy), cw, ch, None, time);
        let above_layers = self.core.layer_parts(sorted.iter().copied().filter(|l| l.z >= 0), (ox, oy), cw, ch, None, time);
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
                        premultiply(palette.background, tint),
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
        // The foreground scroll rides over everything the cockpit drew.
        if let Some(page) = snapshot.scroll.clone() {
            parts.push((Draw::Atlas, self.scroll_instances(&page, (ox, oy))));
            if let Some(instances) = self.scroll_mesh_instances(&page, (ox, oy)) {
                parts.push((Draw::Picture { id: ROLLER_PICTURE, linear: true }, instances));
            }
        }
        if !self.overlay.is_empty() {
            let overlay = std::mem::take(&mut self.overlay);
            parts.extend(self.core.layer_parts(overlay.iter(), (ox, oy), cw, ch, None, time));
            self.overlay = overlay;
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

    /// Whether a transition, or a stage's breathing or flickering layer,
    /// still needs frames.
    pub fn animating(&self) -> bool {
        self.transition.is_some_and(|t| !t.finished(Instant::now())) || self.core.stages_animating()
    }

    /// Take the pictures handed over (`Host::upload_picture`).
    pub fn apply(&mut self, pictures: Vec<graphics::Picture>) {
        for picture in pictures {
            self.core.upload(picture);
        }
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

    /// Draw every stage and scene into its texture before the frame that
    /// shows them.
    fn render_stages(&mut self) {
        let cell = (self.fonts.cell.width, self.fonts.cell.height);
        self.core.render_stages(cell, clock());
        self.core.render_scenes();
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
    pub fn capture(&mut self, snapshot: &Snapshot, layers: &[Layer]) -> (u32, u32, Vec<u8>) {
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
        let pixels = self.core.read_back(&texture, (width, height));
        self.core.keep_frame(frame_data);
        (width, height, pixels)
    }
}

/// An instance carried by its cell's motion: scaled about the cell centre,
/// moved, turned and faded.
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

fn premultiply(rgb: Rgb, alpha: f32) -> [f32; 4] {
    [
        f32::from(rgb[0]) / 255.0 * alpha,
        f32::from(rgb[1]) / 255.0 * alpha,
        f32::from(rgb[2]) / 255.0 * alpha,
        alpha,
    ]
}

/// The xterm 256-colour table above the 16 palette colours.
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

include!("render_scroll.rs");

include!("render_scroll_mesh.rs");
