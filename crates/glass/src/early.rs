//! The window's first frames, drawn on the CPU while the GPU opens.
//!
//! Opening the GPU takes most of startup (on an NVIDIA card about a tenth
//! of a second in the driver's instance and device), and a Wayland window
//! is not shown until something has been drawn into it. So once the window
//! exists, its background goes to the compositor as a plain shared-memory
//! buffer: the background colour at the window's opacity, and the
//! background image laid out and tinted as the GPU lays it out. The window
//! is on screen while the GPU is still opening, and it is drawn again at
//! each size the compositor gives it meanwhile. The program starts before
//! the GPU is open too, so what it writes meanwhile is drawn here as well:
//! the cells' backgrounds, their glyphs (the same fonts, rasterized on the
//! CPU as the atlas rasterizes them), underlines and the cursor. The GPU's
//! first frame takes its place; a few frames later the buffer is let go.
//!
//! Wayland only. Elsewhere, or if any step fails, nothing is drawn here and
//! the window shows with the GPU's first frame.

use std::collections::HashMap;

use ratatui_core::style::{Color, Modifier};
use winit::window::Window;

use crate::config::{Palette, Rgb};
use crate::font::{BOLD, Fonts, ITALIC, Picture};
use crate::render::cell_rgb;
use crate::wallpaper::{Layout, Wallpaper};
use crate::{CursorKind, Snapshot};

/// What the background looks like, as the GPU draws it under everything.
pub struct Look<'a> {
    pub background: [u8; 3],
    pub opacity: f32,
    pub wallpaper: Option<&'a Wallpaper>,
}

/// The program's text, laid out as the renderer will lay it out.
pub struct Text<'a> {
    pub snapshot: &'a Snapshot,
    pub fonts: &'a mut Fonts,
    pub palette: &'a Palette,
    /// The grid's top-left corner in window pixels.
    pub origin: (u32, u32),
    pub focused: bool,
    pub glyphs: &'a mut Glyphs,
}

/// The glyphs the early frames have drawn, kept between them: a symbol,
/// its style and the cells it spans.
#[derive(Default)]
pub struct Glyphs(HashMap<(Box<str>, u8, u32), Option<Picture>>);

/// A frame on show, kept until the GPU's frames have replaced it.
pub struct Frame {
    #[cfg(all(unix, not(target_os = "macos")))]
    _shown: wayland::Shown,
    /// GPU frames presented since.
    presents: u8,
}

impl Frame {
    /// A GPU frame was presented. True once enough have been that this
    /// buffer is off screen for good and can go.
    pub fn presented(&mut self) -> bool {
        self.presents = self.presents.saturating_add(1);
        self.presents >= 3
    }
}

/// Show `look`, and `text` over it, in `window`, at the window's size,
/// now. `None` where it can't be.
pub fn show(window: &Window, look: &Look<'_>, text: Option<Text<'_>>) -> Option<Frame> {
    let size = window.inner_size();
    if !(1..=16384).contains(&size.width) || !(1..=16384).contains(&size.height) {
        return None;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let shown = wayland::show(window, size.width, size.height, || {
            let mut pixels = paint(look, size.width, size.height);
            if let Some(text) = text {
                draw_text(&mut pixels, size.width, size.height, text);
            }
            pixels
        })?;
        Some(Frame { _shown: shown, presents: 0 })
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let _ = (window, look, text);
        None
    }
}

/// The background as the GPU draws it: the background colour at the
/// window's opacity, the image over it where its layout places it (nearest
/// pixels, as the GPU samples it unless `linear` is set), then the tint
/// toward the background colour over the whole window. Premultiplied, as
/// Wayland's ARGB8888 is, in its little-endian byte order (B, G, R, A).
fn paint(look: &Look<'_>, width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let alpha = look.opacity.clamp(0.0, 1.0);
    let bg = look.background.map(|c| f32::from(c) / 255.0);
    let clear = [bg[0] * alpha, bg[1] * alpha, bg[2] * alpha, alpha];
    let tint = look.wallpaper.map_or(0.0, |image| (image.tint * alpha).clamp(0.0, 1.0));
    let finish = move |p: [f32; 4]| -> [u8; 4] {
        let p = if tint > 0.0 {
            [
                bg[0] * tint + p[0] * (1.0 - tint),
                bg[1] * tint + p[1] * (1.0 - tint),
                bg[2] * tint + p[2] * (1.0 - tint),
                tint + p[3] * (1.0 - tint),
            ]
        } else {
            p
        };
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [byte(p[2]), byte(p[1]), byte(p[0]), byte(p[3])]
    };
    let plain = finish(clear);
    let mut out = vec![0u8; w * h * 4];
    let Some(image) = look.wallpaper else {
        for pixel in out.chunks_exact_mut(4) {
            pixel.copy_from_slice(&plain);
        }
        return out;
    };
    // Where each window column and row reads the image, if it does.
    let (dst, src) = image.quad((width as f32, height as f32));
    let texels = |count: usize, at: f32, span: f32, from: f32, length: f32, size: u32| -> Vec<Option<usize>> {
        (0..count)
            .map(|i| {
                let t = (i as f32 + 0.5 - at) / span;
                if !(0.0..1.0).contains(&t) {
                    return None;
                }
                let texel = (from + t * length).floor() as i64;
                Some(address(image.layout, texel, i64::from(size)))
            })
            .collect()
    };
    let columns = texels(w, dst[0], dst[2], src[0], src[2], image.width);
    let rows = texels(h, dst[1], dst[3], src[1], src[3], image.height);
    let iw = image.width as usize;
    let fill = |(y, line): (usize, &mut [u8])| {
        for (x, pixel) in line.chunks_exact_mut(4).enumerate() {
            let value = match (rows[y], columns[x]) {
                (Some(ty), Some(tx)) => {
                    let at = (ty * iw + tx) * 4;
                    let Some(texel) = image.rgba.get(at..at + 4) else {
                        pixel.copy_from_slice(&plain);
                        continue;
                    };
                    let a = f32::from(texel[3]) / 255.0 * alpha;
                    let c = |i: usize| f32::from(texel[i]) / 255.0 * a + clear[i] * (1.0 - a);
                    finish([c(0), c(1), c(2), a + clear[3] * (1.0 - a)])
                }
                _ => plain,
            };
            pixel.copy_from_slice(&value);
        }
    };
    // Rows in bands across a few threads: a full-screen window is a few
    // million pixels, and this is on the way to the first frame.
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let band = h.div_ceil(threads).max(1) * w * 4;
    let fill = &fill;
    std::thread::scope(|scope| {
        for (n, chunk) in out.chunks_mut(band).enumerate() {
            let first = n * (band / (w * 4));
            scope.spawn(move || chunk.chunks_exact_mut(w * 4).enumerate().for_each(|(y, line)| fill((first + y, line))));
        }
    });
    out
}

/// The program's cells over the background, as the GPU draws them:
/// backgrounds, glyphs (dimmed, hidden, reversed as asked), underlines and
/// strikes, and the cursor. A ligature, a sprite-drawn box or braille
/// character, and the text gamma are the GPU's alone; these frames last a
/// tenth of a second.
fn draw_text(pixels: &mut [u8], width: u32, height: u32, text: Text<'_>) {
    let Text { snapshot, fonts, palette, origin, focused, glyphs } = text;
    let cell = fonts.cell;
    let (cols, rows) = (usize::from(snapshot.cols), usize::from(snapshot.rows));
    if cols == 0 || cell.width == 0 || cell.height == 0 || snapshot.cells.len() < cols * rows {
        return;
    }
    let mut canvas = Canvas { pixels, width, height };
    for row in 0..rows {
        let y = origin.1 + row as u32 * cell.height;
        let mut col = 0;
        while col < cols {
            let c = &snapshot.cells[row * cols + col];
            let symbol = c.symbol();
            let is_space = symbol.as_bytes() == b" ";
            let span = if is_space { 1 } else { crate::backend::symbol_width(symbol).clamp(1, 2).min(cols - col) };
            let x = origin.0 + col as u32 * cell.width;
            let w = span as u32 * cell.width;
            let modifier = c.modifier;
            let mut fg = cell_rgb(palette, c.fg, palette.foreground);
            let mut bg = match c.bg {
                Color::Reset => None,
                other => Some(cell_rgb(palette, other, palette.background)),
            };
            if modifier.contains(Modifier::REVERSED) {
                let swapped = bg.unwrap_or(palette.background);
                bg = Some(fg);
                fg = swapped;
            }
            let at_cursor = snapshot.cursor == Some((col as u16, row as u16));
            let block = at_cursor && focused && snapshot.cursor_style.kind == CursorKind::Block;
            let cursor = snapshot.cursor_style.color.unwrap_or(palette.cursor);
            if block {
                bg = Some(cursor);
                fg = palette.cursor_text;
            }
            if let Some(bg) = bg {
                canvas.fill(x, y, w, cell.height, bg, 1.0);
            }
            let alpha = if modifier.contains(Modifier::DIM) { 0.6 } else { 1.0 };
            if !is_space && !modifier.contains(Modifier::HIDDEN) {
                let style = if modifier.contains(Modifier::BOLD) { BOLD } else { 0 }
                    | if modifier.contains(Modifier::ITALIC) { ITALIC } else { 0 };
                let glyph = glyphs
                    .0
                    .entry((Box::from(symbol), style, span as u32))
                    .or_insert_with(|| fonts.draw(symbol, style, span as u32));
                if let Some(glyph) = glyph {
                    canvas.glyph(x as i32 + glyph.left, y as i32 + glyph.top, glyph, fg, alpha);
                }
            }
            if modifier.contains(Modifier::UNDERLINED) {
                canvas.fill(x, y + cell.underline_y, w, cell.underline_height.max(1), fg, alpha);
            }
            if modifier.contains(Modifier::CROSSED_OUT) {
                canvas.fill(x, y + cell.strike_y, w, cell.underline_height.max(1), fg, alpha);
            }
            if at_cursor && !block {
                let line = cell.line.max(1);
                match (focused, snapshot.cursor_style.kind) {
                    (true, CursorKind::Bar) => canvas.fill(x, y, line * 2, cell.height, cursor, 1.0),
                    (true, CursorKind::Underline) => canvas.fill(x, y + cell.height - line * 2, w, line * 2, cursor, 1.0),
                    _ => {
                        // Unfocused: the block's outline.
                        canvas.fill(x, y, w, line, cursor, 1.0);
                        canvas.fill(x, y + cell.height - line, w, line, cursor, 1.0);
                        canvas.fill(x, y, line, cell.height, cursor, 1.0);
                        canvas.fill(x + w - line, y, line, cell.height, cursor, 1.0);
                    }
                }
            }
            col += span;
        }
    }
}

/// Premultiplied BGRA pixels, drawn into with "over".
struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: u32,
    height: u32,
}

impl Canvas<'_> {
    /// `rgb` at `alpha` over the rectangle, clipped to the window.
    fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: Rgb, alpha: f32) {
        let (x1, y1) = ((x + w).min(self.width), (y + h).min(self.height));
        for py in y.min(y1)..y1 {
            for px in x.min(x1)..x1 {
                self.over(px, py, rgb, alpha);
            }
        }
    }

    /// A glyph with its top-left at `(x, y)`: a coverage mask in `rgb`, or
    /// its own premultiplied colours, at `alpha`.
    fn glyph(&mut self, x: i32, y: i32, glyph: &Picture, rgb: Rgb, alpha: f32) {
        for gy in 0..glyph.height {
            let py = y + gy as i32;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            for gx in 0..glyph.width {
                let px = x + gx as i32;
                if px < 0 || px >= self.width as i32 {
                    continue;
                }
                let at = (gy * glyph.width + gx) as usize;
                if glyph.colored {
                    let Some(&[r, g, b, a]) = glyph.data.get(at * 4..at * 4 + 4).and_then(|p| <&[u8; 4]>::try_from(p).ok()) else {
                        continue;
                    };
                    let i = ((py as u32 * self.width + px as u32) * 4) as usize;
                    let k = f32::from(a) / 255.0 * alpha;
                    let mix = |dst: u8, src: u8| (f32::from(src) * alpha + f32::from(dst) * (1.0 - k)).round().min(255.0) as u8;
                    let p = &mut self.pixels[i..i + 4];
                    (p[0], p[1], p[2], p[3]) = (mix(p[0], b), mix(p[1], g), mix(p[2], r), mix(p[3], a));
                } else if let Some(&coverage) = glyph.data.get(at) {
                    if coverage > 0 {
                        self.over(px as u32, py as u32, rgb, f32::from(coverage) / 255.0 * alpha);
                    }
                }
            }
        }
    }

    fn over(&mut self, x: u32, y: u32, rgb: Rgb, alpha: f32) {
        let i = ((y * self.width + x) * 4) as usize;
        let p = &mut self.pixels[i..i + 4];
        let mix = |dst: u8, src: u8| (f32::from(src) * alpha + f32::from(dst) * (1.0 - alpha)).round() as u8;
        (p[0], p[1], p[2], p[3]) = (mix(p[0], rgb[2]), mix(p[1], rgb[1]), mix(p[2], rgb[0]), mix(p[3], 255));
    }
}

/// The texel `texel` reads in an image `size` wide, as the GPU's sampler
/// addresses it for `layout`.
fn address(layout: Layout, texel: i64, size: i64) -> usize {
    let size = size.max(1);
    let at = match layout {
        Layout::Tiled => texel.rem_euclid(size),
        Layout::MirrorTiled => {
            let period = texel.rem_euclid(2 * size);
            if period < size { period } else { 2 * size - 1 - period }
        }
        _ => texel.clamp(0, size - 1),
    };
    at as usize
}

#[cfg(all(unix, not(target_os = "macos")))]
mod wayland {
    use std::io::Write;
    use std::os::fd::AsFd;
    use std::os::unix::fs::OpenOptionsExt;

    use wayland_backend::client::{Backend, ObjectId};
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool, wl_surface};
    use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
    use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
    use winit::window::Window;

    /// A buffer attached to the window's surface, and what it came through.
    pub struct Shown {
        conn: Connection,
        pool: wl_shm_pool::WlShmPool,
        buffer: wl_buffer::WlBuffer,
        // Dropped last: the objects above are on it.
        _queue: EventQueue<State>,
    }

    impl Drop for Shown {
        fn drop(&mut self) {
            self.buffer.destroy();
            self.pool.destroy();
            let _ = self.conn.flush();
        }
    }

    pub struct State;

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    wayland_client::delegate_noop!(State: ignore wl_shm::WlShm);
    wayland_client::delegate_noop!(State: wl_shm_pool::WlShmPool);
    wayland_client::delegate_noop!(State: ignore wl_buffer::WlBuffer);

    /// Attach `paint`'s pixels, `width` by `height`, to `window`'s surface
    /// and commit, on a connection of our own to winit's display.
    pub fn show(window: &Window, width: u32, height: u32, paint: impl FnOnce() -> Vec<u8>) -> Option<Shown> {
        let RawDisplayHandle::Wayland(display) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(surface) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: the display and the surface are winit's, live for as long
        // as `window` is, and outlive this (the window lets this go first:
        // `Glass::release`).
        let conn = Connection::from_backend(unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) });
        let (globals, queue) = registry_queue_init::<State>(&conn).ok()?;
        let qh = queue.handle();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).ok()?;
        // SAFETY: as above; the pointer is a live wl_surface proxy.
        let id = unsafe { ObjectId::from_ptr(wl_surface::WlSurface::interface(), surface.surface.as_ptr().cast()) }.ok()?;
        let surface = wl_surface::WlSurface::from_id(&conn, id).ok()?;
        let pixels = paint();
        let bytes = i32::try_from(pixels.len()).ok()?;
        let file = memory(&pixels)?;
        let pool = shm.create_pool(file.as_fd(), bytes, &qh, ());
        let (w, h) = (i32::try_from(width).ok()?, i32::try_from(height).ok()?);
        let buffer = pool.create_buffer(0, w, h, w * 4, wl_shm::Format::Argb8888, &qh, ());
        surface.attach(Some(&buffer), 0, 0);
        if surface.version() >= 4 {
            surface.damage_buffer(0, 0, w, h);
        } else {
            surface.damage(0, 0, w, h);
        }
        surface.commit();
        conn.flush().ok()?;
        Some(Shown { conn, pool, buffer, _queue: queue })
    }

    /// `pixels` in a file of shared memory: one in the runtime directory
    /// (a tmpfs), unlinked once open.
    fn memory(pixels: &[u8]) -> Option<std::fs::File> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let path = std::path::Path::new(&dir).join(format!("glass-early-{}-{nanos}", std::process::id()));
        let mut file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        file.write_all(pixels).ok()?;
        Some(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallpaper(layout: Layout, tint: f32) -> Wallpaper {
        // 2 by 1: red then green, opaque.
        Wallpaper { width: 2, height: 1, rgba: vec![255, 0, 0, 255, 0, 255, 0, 255], layout, tint, linear: false }
    }

    fn at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn the_background_alone_is_premultiplied_bgra() {
        let look = Look { background: [200, 100, 50], opacity: 0.5, wallpaper: None };
        let pixels = paint(&look, 3, 2);
        assert_eq!(pixels.len(), 3 * 2 * 4);
        assert!(pixels.chunks_exact(4).all(|p| p == [25, 50, 100, 128]));
    }

    #[test]
    fn a_centred_image_sits_in_the_middle_and_the_tint_covers_the_window() {
        let image = wallpaper(Layout::Centered, 0.5);
        let look = Look { background: [0, 0, 0], opacity: 1.0, wallpaper: Some(&image) };
        let pixels = paint(&look, 4, 1);
        // Black either side; red then green between, half toward black.
        assert_eq!(at(&pixels, 4, 0, 0), [0, 0, 0, 255]);
        assert_eq!(at(&pixels, 4, 1, 0), [0, 0, 128, 255]);
        assert_eq!(at(&pixels, 4, 2, 0), [0, 128, 0, 255]);
        assert_eq!(at(&pixels, 4, 3, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn tiles_repeat_and_mirrored_tiles_turn_back() {
        let tiled = wallpaper(Layout::Tiled, 0.0);
        let mirrored = wallpaper(Layout::MirrorTiled, 0.0);
        let row = |image: &Wallpaper| {
            let look = Look { background: [0, 0, 0], opacity: 1.0, wallpaper: Some(image) };
            let pixels = paint(&look, 6, 1);
            (0..6).map(|x| if at(&pixels, 6, x, 0)[2] == 255 { 'r' } else { 'g' }).collect::<String>()
        };
        assert_eq!(row(&tiled), "rgrgrg");
        assert_eq!(row(&mirrored), "rggrrg");
        assert_eq!(address(Layout::Clamped, -3, 2), 0);
        assert_eq!(address(Layout::Clamped, 9, 2), 1);
    }

    #[test]
    fn the_programs_cells_are_drawn_where_the_renderer_puts_them() {
        let Ok(mut fonts) = Fonts::load("monospace", 20.0, 1.0) else {
            eprintln!("no monospace font: skipped");
            return;
        };
        let cell = fonts.cell;
        // Two columns, one row: a red cell holding a white "M", then the
        // block cursor on a blank.
        let mut cells = vec![ratatui_core::buffer::Cell::default(); 2];
        cells[0].set_symbol("M").set_fg(Color::White).set_bg(Color::Rgb(200, 0, 0));
        let snapshot = Snapshot { cells, cols: 2, rows: 1, cursor: Some((1, 0)), ..Snapshot::default() };
        let palette = Palette::default();
        let (width, height) = (cell.width * 2 + 20, cell.height + 20);
        let look = Look { background: palette.background, opacity: 1.0, wallpaper: None };
        let mut pixels = paint(&look, width, height);
        let mut glyphs = Glyphs::default();
        let text = Text { snapshot: &snapshot, fonts: &mut fonts, palette: &palette, origin: (10, 10), focused: true, glyphs: &mut glyphs };
        draw_text(&mut pixels, width, height, text);
        // The red background's corner, the cursor's block, and the margin
        // left as the background.
        assert_eq!(at(&pixels, width, 10, 10), [0, 0, 200, 255]);
        let c = palette.cursor;
        assert_eq!(at(&pixels, width, 10 + cell.width, 10), [c[2], c[1], c[0], 255]);
        let b = palette.background;
        assert_eq!(at(&pixels, width, 5, 5), [b[2], b[1], b[0], 255]);
        // Somewhere in the first cell the glyph lit pixels toward white.
        let lit = (10..10 + cell.height).any(|y| (10..10 + cell.width).any(|x| at(&pixels, width, x, y)[1] > 128));
        assert!(lit, "the glyph drew nothing");
        assert_eq!(glyphs.0.len(), 1, "the glyph is kept for the next frame");
    }

    #[test]
    fn many_rows_are_painted_alike_across_the_bands() {
        let look = Look { background: [1, 2, 3], opacity: 1.0, wallpaper: None };
        let pixels = paint(&look, 7, 97);
        assert!(pixels.chunks_exact(4).all(|p| p == [3, 2, 1, 255]));
    }
}
