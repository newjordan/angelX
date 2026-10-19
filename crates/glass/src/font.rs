//! Font files and glyph pictures.
//!
//! swash reads outlines and bitmaps out of the font files; which file, the
//! cell size, the fallback chain and everything after the picture are ours.
//! Files are found through fontconfig (`fc-match`), the same lookup the
//! desktop's terminals use, so a family name means what it means there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use swash::FontRef;
use swash::scale::image::{Content, Image};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Angle, Format, Transform};

/// Pixel size of one cell and where things sit inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMetrics {
    pub width: u32,
    pub height: u32,
    /// Distance from the cell top to the baseline.
    pub baseline: u32,
    pub underline_y: u32,
    pub underline_height: u32,
    pub strike_y: u32,
    /// Stroke widths of light and heavy box-drawing lines: 1 and 2 points.
    pub line: u32,
    pub heavy: u32,
}

#[derive(Clone, Copy)]
struct Face {
    font: FontRef<'static>,
    synthetic_bold: bool,
    synthetic_italic: bool,
}

/// A glyph picture: an alpha mask or premultiplied RGBA, placed relative to
/// the cell's top-left corner.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub colored: bool,
    pub data: Vec<u8>,
}

pub struct Fonts {
    family: String,
    /// Regular, bold, italic, bold italic.
    faces: [Face; 4],
    fallbacks: Vec<FontRef<'static>>,
    /// Whether each fallback face is bold and italic.
    fallback_style: Vec<u8>,
    fallback_files: HashMap<(PathBuf, u32), usize>,
    fallback_for: HashMap<(char, u8), Option<usize>>,
    context: ScaleContext,
    pub size_px: f32,
    pub cell: CellMetrics,
}

pub const BOLD: u8 = 1;
pub const ITALIC: u8 = 2;

impl Fonts {
    pub fn load(family: &str, size_px: f32, point: f32) -> Result<Self, String> {
        let (regular, regular_file) = load_face(family, "")
            .ok_or_else(|| format!("no font found for {family:?} (is fontconfig installed?)"))?;
        let pick = |style: &str, bold: bool, italic: bool| {
            load_face(family, style)
                .filter(|(_, file)| *file != regular_file)
                .map(|(font, _)| Face {
                    font,
                    synthetic_bold: false,
                    synthetic_italic: false,
                })
                .unwrap_or(Face {
                    font: regular,
                    synthetic_bold: bold,
                    synthetic_italic: italic,
                })
        };
        let faces = [
            Face {
                font: regular,
                synthetic_bold: false,
                synthetic_italic: false,
            },
            pick(":weight=bold", true, false),
            pick(":slant=italic", false, true),
            pick(":weight=bold:slant=italic", true, true),
        ];
        let mut fonts = Self {
            family: family.to_string(),
            faces,
            fallbacks: Vec::new(),
            fallback_style: Vec::new(),
            fallback_files: HashMap::new(),
            fallback_for: HashMap::new(),
            context: ScaleContext::new(),
            size_px,
            cell: measure(regular, size_px, point),
        };
        fonts.set_size(size_px, point);
        Ok(fonts)
    }

    pub fn set_size(&mut self, size_px: f32, point: f32) {
        self.size_px = size_px.max(4.0);
        self.cell = measure(self.faces[0].font, self.size_px, point);
    }

    /// Draw one grapheme (a cell's symbol) spanning `span` cells. `None` when
    /// no font has it.
    pub fn draw(&mut self, symbol: &str, style: u8, span: u32) -> Option<Picture> {
        let mut chars = symbol
            .chars()
            .filter(|c| !matches!(c, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}'));
        let base = chars.next()?;
        let mut picture = self.draw_char(base, style, span)?;
        // Combining marks, centred over the base cell. Fonts disagree on
        // where a zero-advance mark sits; real mark placement needs shaping.
        for mark in chars {
            if unicode_width::UnicodeWidthChar::width(mark) != Some(0) {
                break;
            }
            if let Some(mut over) = self.draw_char(mark, style, span) {
                over.left = (self.cell.width as i32 - over.width as i32) / 2;
                picture = composite(picture, over, 0);
            }
        }
        Some(picture)
    }

    fn draw_char(&mut self, ch: char, style: u8, span: u32) -> Option<Picture> {
        let (face, glyph) = self.lookup(ch, style)?;
        let size = self.size_px;
        let mut scaler = self
            .context
            .builder(face.font)
            .size(size)
            .hint(true)
            .build();
        let mut render = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ]);
        render.format(Format::Alpha);
        if face.synthetic_bold {
            render.embolden((size / 28.0).max(0.5));
        }
        if face.synthetic_italic {
            render.transform(Some(Transform::skew(
                Angle::from_degrees(12.0),
                Angle::from_degrees(0.0),
            )));
        }
        let image = render.render(&mut scaler, glyph)?;
        let picture = self.place(image);
        Some(self.fit_color(picture, span))
    }

    fn place(&self, image: Image) -> Picture {
        let colored = matches!(image.content, Content::Color);
        let data = match image.content {
            Content::Mask => image.data,
            Content::Color => premultiply(image.data),
            // Subpixel masks are not requested; keep their green channel.
            Content::SubpixelMask => image.data.chunks_exact(4).map(|p| p[1]).collect(),
        };
        let (width, height, data) =
            normalize(data, image.placement.width, image.placement.height, colored)
                .unwrap_or((0, 0, Vec::new()));
        Picture {
            width,
            height,
            left: image.placement.left,
            top: self.cell.baseline as i32 - image.placement.top,
            colored,
            data,
        }
    }

    /// Colour bitmaps (emoji) come at their strike's size; shrink them to the
    /// cells they cover and centre them there.
    fn fit_color(&self, picture: Picture, span: u32) -> Picture {
        if !picture.colored || picture.width == 0 || picture.height == 0 {
            return picture;
        }
        let max_w = self.cell.width * span.max(1);
        let max_h = self.cell.height;
        if picture.width <= max_w && picture.height <= max_h {
            return picture;
        }
        let scale = (max_w as f32 / picture.width as f32).min(max_h as f32 / picture.height as f32);
        let width = ((picture.width as f32 * scale).round() as u32).max(1);
        let height = ((picture.height as f32 * scale).round() as u32).max(1);
        let data = downscale_rgba(&picture.data, picture.width, picture.height, width, height);
        Picture {
            width,
            height,
            left: ((max_w - width) / 2) as i32,
            top: ((max_h - height) / 2) as i32,
            colored: true,
            data,
        }
    }

    fn lookup(&mut self, ch: char, style: u8) -> Option<(Face, u16)> {
        let face = self.faces[usize::from(style & 3)];
        let glyph = face.font.charmap().map(ch);
        if glyph != 0 {
            return Some((face, glyph));
        }
        let regular = self.faces[0];
        let glyph = regular.font.charmap().map(ch);
        if glyph != 0 {
            return Some((regular, glyph));
        }
        let index = self.fallback(ch, style & 3)?;
        let font = self.fallbacks[index];
        let glyph = font.charmap().map(ch);
        // A fallback with a real bold face draws bold itself.
        let has = self.fallback_style[index];
        (glyph != 0).then_some((
            Face {
                font,
                synthetic_bold: style & BOLD != 0 && has & BOLD == 0,
                synthetic_italic: false,
            },
            glyph,
        ))
    }

    /// A fallback face holding `ch`, in the style asked for when the system
    /// has one (fontconfig picks).
    fn fallback(&mut self, ch: char, style: u8) -> Option<usize> {
        if let Some(found) = self.fallback_for.get(&(ch, style)) {
            return *found;
        }
        let loaded = (0..self.fallbacks.len())
            .find(|&i| self.fallback_style[i] == style && self.fallbacks[i].charmap().map(ch) != 0);
        let found = loaded
            .or_else(|| self.find_fallback(ch, style))
            .or_else(|| {
                // No face in this style: the plain one, made bold by the scaler.
                (style != 0).then(|| self.fallback(ch, 0)).flatten()
            });
        self.fallback_for.insert((ch, style), found);
        found
    }

    fn find_fallback(&mut self, ch: char, style: u8) -> Option<usize> {
        // Each new face is a whole font file kept for the life of the
        // process; a stream cycling scripts must not load them all.
        if !room_for_fallback(self.fallbacks.len()) {
            return None;
        }
        let suffix = match style {
            BOLD => ":weight=bold",
            ITALIC => ":slant=italic",
            3 => ":weight=bold:slant=italic",
            _ => "",
        };
        let code = ch as u32;
        let (path, index) = fc_match(&format!("{}{suffix}:charset={code:x}", self.family))
            .or_else(|| fc_match(&format!(":charset={code:x}{suffix}")))
            .or_else(|| {
                // No fontconfig: the platform's own fallback faces, in order.
                // Already-loaded faces answer from the cache: opening here
                // would leak another copy of the file on every new char.
                static_fallbacks().into_iter().find_map(|path| {
                    if let Some(&slot) = self.fallback_files.get(&(path.clone(), 0)) {
                        return (self.fallbacks[slot].charmap().map(ch) != 0)
                            .then_some((path, 0));
                    }
                    let font = open(&path, 0)?;
                    (font.charmap().map(ch) != 0).then_some((path, 0))
                })
            })?;
        if let Some(&known) = self.fallback_files.get(&(path.clone(), index)) {
            let fits = self.fallback_style[known] == style || style == 0;
            return (fits && self.fallbacks[known].charmap().map(ch) != 0).then_some(known);
        }
        let font = open(&path, index)?;
        if font.charmap().map(ch) == 0 {
            return None;
        }
        let attributes = font.attributes();
        let mut has = 0;
        if attributes.weight().0 >= 600 {
            has |= BOLD;
        }
        if attributes.style() != swash::Style::Normal {
            has |= ITALIC;
        }
        if style != 0 && has != style {
            // fontconfig answered with another style; keep it for that style.
            self.fallbacks.push(font);
            self.fallback_style.push(has);
            let slot = self.fallbacks.len() - 1;
            self.fallback_files.insert((path, index), slot);
            return None;
        }
        self.fallbacks.push(font);
        self.fallback_style.push(has);
        let slot = self.fallbacks.len() - 1;
        self.fallback_files.insert((path, index), slot);
        Some(slot)
    }
}

/// `point`: pixels per point at the window's scale (lines are 1 and 2 pt).
#[allow(clippy::manual_clamp)]
fn measure(font: FontRef<'static>, size_px: f32, point: f32) -> CellMetrics {
    let metrics = font.metrics(&[]).scale(size_px);
    let glyphs = font.glyph_metrics(&[]).scale(size_px);
    let advance = ['0', 'M', ' ']
        .iter()
        .map(|&c| font.charmap().map(c))
        .find(|&g| g != 0)
        .map(|g| glyphs.advance_width(g))
        .filter(|&w| w > 0.0)
        .unwrap_or(size_px * 0.6);
    let ascent = metrics.ascent;
    let descent = metrics.descent.abs();
    // Bounded both ways with max/min (NaN resolves to a bound): every
    // later canvas and atlas entry is sized by these numbers, so a font
    // claiming a kilometre-wide cell gets a large cell, not an allocation.
    let height = (ascent + descent + metrics.leading).ceil().max(1.0).min(4096.0) as u32;
    let baseline = (ascent + metrics.leading / 2.0).round().max(0.0).min(4096.0) as u32;
    let width = advance.round().max(1.0).min(4096.0) as u32;
    let underline_height = metrics.stroke_size.round().max(1.0).min(4096.0) as u32;
    // Saturating: a font may claim a stroke thicker than the cell is tall.
    let underline_y = (baseline as f32 - metrics.underline_offset)
        .round()
        .clamp(0.0, height.saturating_sub(underline_height) as f32) as u32;
    let strike_offset = if metrics.strikeout_offset > 0.0 {
        metrics.strikeout_offset
    } else {
        metrics.x_height / 2.0
    };
    let strike_y = (baseline as f32 - strike_offset).round().max(0.0).min(4096.0) as u32;
    let line = point.ceil().max(1.0).min(1024.0) as u32;
    let heavy = (point * 2.0).ceil().max(line as f32 + 1.0).min(2048.0) as u32;
    CellMetrics {
        width,
        height,
        baseline,
        underline_y,
        underline_height,
        strike_y,
        line,
        heavy,
    }
}

/// A raster's bytes made to match the size they claim, or `None` when the
/// claim itself is absurd. Everything downstream (`composite`, the atlas)
/// indexes by width and height; a corrupt or hostile font must not turn
/// that into a panic or a giant allocation. Runs once per glyph, before
/// the atlas cache, so the check is free.
fn normalize(mut data: Vec<u8>, width: u32, height: u32, colored: bool) -> Option<(u32, u32, Vec<u8>)> {
    if width > 4096 || height > 4096 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(if colored { 4 } else { 1 })?;
    data.resize(expected, 0);
    Some((width, height, data))
}

fn composite(base: Picture, over: Picture, advance: i32) -> Picture {
    let colored = base.colored || over.colored;
    let over_left = over.left + advance;
    let left = base.left.min(over_left);
    let top = base.top.min(over.top);
    let right = (base.left + base.width as i32).max(over_left + over.width as i32);
    let bottom = (base.top + base.height as i32).max(over.top + over.height as i32);
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let px = if colored { 4 } else { 1 };
    let mut data = vec![0u8; (width * height) as usize * px];
    for (picture, x0) in [(&base, base.left), (&over, over_left)] {
        for y in 0..picture.height {
            for x in 0..picture.width {
                let dx = (x0 - left) as u32 + x;
                let dy = (picture.top - top) as u32 + y;
                let dst = ((dy * width + dx) as usize) * px;
                let src = (y * picture.width + x) as usize;
                if colored {
                    let rgba = if picture.colored {
                        [
                            picture.data[src * 4],
                            picture.data[src * 4 + 1],
                            picture.data[src * 4 + 2],
                            picture.data[src * 4 + 3],
                        ]
                    } else {
                        let a = picture.data[src];
                        [a, a, a, a]
                    };
                    let keep = 255 - u16::from(rgba[3]);
                    for c in 0..4 {
                        data[dst + c] =
                            (u16::from(rgba[c]) + u16::from(data[dst + c]) * keep / 255) as u8;
                    }
                } else {
                    data[dst] = data[dst].max(picture.data[src]);
                }
            }
        }
    }
    Picture {
        width,
        height,
        left,
        top,
        colored,
        data,
    }
}

fn premultiply(mut rgba: Vec<u8>) -> Vec<u8> {
    for p in rgba.chunks_exact_mut(4) {
        let a = u16::from(p[3]);
        for c in &mut p[..3] {
            *c = (u16::from(*c) * a / 255) as u8;
        }
    }
    rgba
}

/// Box filter: each target pixel averages the source pixels it covers.
fn downscale_rgba(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..dh {
        let y0 = y * sh / dh;
        let y1 = ((y + 1) * sh / dh).max(y0 + 1).min(sh);
        for x in 0..dw {
            let x0 = x * sw / dw;
            let x1 = ((x + 1) * sw / dw).max(x0 + 1).min(sw);
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * sw + sx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += u32::from(src[i + c]);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let o = ((y * dw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (sum[c] / n) as u8;
            }
        }
    }
    out
}

/// A face and the file it came from (to tell a real bold from a fallback to
/// the regular file).
fn load_face(family: &str, style: &str) -> Option<(FontRef<'static>, (PathBuf, u32))> {
    let path = Path::new(family);
    let (file, index) = if path.is_file() {
        if !style.is_empty() {
            return None;
        }
        (path.to_path_buf(), 0)
    } else {
        fc_match(&format!("{family}{style}"))
            .or_else(|| find_in_font_dirs(family, style))
            .or_else(|| platform_default(style.is_empty()))?
    };
    Some((open(&file, index)?, (file, index)))
}

/// Folders a system without fontconfig (macOS, Windows) keeps fonts in.
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join("Library/Fonts"));
        dirs.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    for dir in [
        "/Library/Fonts",
        "/System/Library/Fonts",
        "/System/Library/Fonts/Supplemental",
        "C:\\Windows\\Fonts",
    ] {
        dirs.push(PathBuf::from(dir));
    }
    dirs
}

/// A family's file by its name, without fontconfig: `Berkeley Mono` with
/// `:weight=bold` finds `BerkeleyMono-Bold.otf`. Regular prefers a file
/// named Regular, or one with no style in its name.
fn find_in_font_dirs(family: &str, style: &str) -> Option<(PathBuf, u32)> {
    let squash = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let wanted = squash(family);
    if wanted.is_empty() || wanted == "monospace" {
        return None;
    }
    let bold = style.contains("bold");
    let italic = style.contains("italic");
    let mut best: Option<(u8, PathBuf)> = None;
    for dir in font_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = squash(&path.file_stem().unwrap_or_default().to_string_lossy());
            let font_file = path.extension().is_some_and(|e| {
                matches!(
                    e.to_string_lossy().to_lowercase().as_str(),
                    "ttf" | "otf" | "ttc"
                )
            });
            if !font_file || !name.starts_with(&wanted) {
                continue;
            }
            let rest = &name[wanted.len()..];
            let has_bold = rest.contains("bold");
            let has_italic = rest.contains("italic") || rest.contains("oblique");
            if has_bold != bold || has_italic != italic {
                continue;
            }
            // Regular, or the plain name, beats light/medium/condensed cuts.
            let score = match rest {
                "" | "regular" | "bold" | "italic" | "oblique" | "bolditalic" | "boldoblique" => 0,
                _ => 1,
            };
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, path));
            }
        }
    }
    best.map(|(_, path)| (path, 0))
}

/// Fallback faces on a system without fontconfig, tried in order.
fn static_fallbacks() -> Vec<PathBuf> {
    [
        "/System/Library/Fonts/Apple Color Emoji.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/Apple Symbols.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "C:\\Windows\\Fonts\\seguiemj.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\seguisym.ttf",
    ]
    .iter()
    .map(PathBuf::from)
    .filter(|p| p.is_file())
    .collect()
}

/// Where a system without fontconfig keeps a monospace font.
fn platform_default(regular: bool) -> Option<(PathBuf, u32)> {
    if !regular {
        return None;
    }
    [
        "/System/Library/Fonts/SFNSMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "C:\\Windows\\Fonts\\consola.ttf",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
    .map(|p| (p, 0))
}

fn fc_match(pattern: &str) -> Option<(PathBuf, u32)> {
    // Bounded wait: fontconfig can legitimately take seconds building a
    // cache, but a wedged one must not freeze the window (this runs on
    // the render thread, at startup and on first fallback glyphs).
    let mut child = Command::new("fc-match")
        .args(["-f", "%{file}\t%{index}", pattern])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            _ => {
                let _ = child.kill();
                break None;
            }
        }
    }?;
    if !status.success() {
        return None;
    }
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(&mut child.stdout.take()?, 1 << 16), &mut out).ok()?;
    let text = String::from_utf8(out).ok()?;
    let (file, index) = text.split_once('\t')?;
    let file = PathBuf::from(file.trim());
    file.is_file()
        .then(|| (file, index.trim().parse().unwrap_or(0)))
}

/// The largest real font collections stay well under this; anything
/// bigger named like a font is not read into memory at all.
const MAX_FONT_BYTES: u64 = 256 << 20;

/// Fallback faces kept at once. Real text mixes a handful of scripts;
/// each face is a font file retained for the life of the process.
const MAX_FALLBACK_FACES: usize = 64;

fn room_for_fallback(loaded: usize) -> bool {
    loaded < MAX_FALLBACK_FACES
}

/// Font files stay loaded for the life of the process.
fn open(path: &Path, index: u32) -> Option<FontRef<'static>> {
    if std::fs::metadata(path).ok()?.len() > MAX_FONT_BYTES {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    // Validate before leaking: a non-font file (or a bad index into a
    // collection) must not stay in memory for the life of the process.
    FontRef::from_index(&data, index as usize)?;
    let data: &'static [u8] = Box::leak(data.into_boxed_slice());
    FontRef::from_index(data, index as usize)
}

/// Cached `swash` page rasterizer for `present::document`, holding a single
/// loaded font family and a per-size glyph mask cache so 40×80 document pages
/// blit in microseconds.
struct PageFonts {
    fonts: Fonts,
    size_bits: u32,
    glyphs: HashMap<(char, u8), Option<Picture>>,
}

static PAGE_FONTS: std::sync::LazyLock<std::sync::Mutex<Option<PageFonts>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Rasterize a scrolled document view (`receipt` header + wrapped `text` body
/// starting at wrapped line `scroll`) into a straight-RGBA pixel page sized to
/// `(cols * cell_px.0, rows * cell_px.1)`. Returns `None` when dimensions are
/// degenerate or no system font can be loaded.
pub fn render_page(
    receipt: &str,
    text: &str,
    scroll: u16,
    cols: u16,
    rows: u16,
    cell_px: (u16, u16),
) -> Option<(u32, u32, Vec<u8>)> {
    if cols == 0 || rows == 0 || cell_px.0 == 0 || cell_px.1 == 0 {
        return None;
    }
    let (cw, ch) = (u32::from(cell_px.0), u32::from(cell_px.1));
    let width = u32::from(cols).checked_mul(cw)?;
    let height = u32::from(rows).checked_mul(ch)?;
    if width > 4096 || height > 4096 {
        return None;
    }
    let size_px = (ch as f32 * 0.72).clamp(8.0, 64.0);
    let size_bits = size_px.to_bits();

    let mut guard = PAGE_FONTS.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let loaded = Fonts::load("monospace", size_px, 1.0).ok()?;
        *guard = Some(PageFonts {
            fonts: loaded,
            size_bits,
            glyphs: HashMap::with_capacity(256),
        });
    }
    let state = guard.as_mut()?;
    if state.size_bits != size_bits {
        state.fonts.set_size(size_px, 1.0);
        state.size_bits = size_bits;
        state.glyphs.clear();
    }

    let mut rgba = vec![0u8; (width * height * 4) as usize];
    // Obsidian page card with a 1px slate rule border and subtle header band.
    for y in 0..height {
        let is_header = y < ch;
        let bg = if y == 0 || y + 1 == height {
            [48u8, 62, 82, 255]
        } else if y == ch {
            [38u8, 50, 68, 255]
        } else if is_header {
            [18u8, 24, 34, 255]
        } else {
            [12u8, 16, 22, 255]
        };
        let row = &mut rgba[(y * width * 4) as usize..((y + 1) * width * 4) as usize];
        for px in row.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
        if width >= 2 {
            row[0..4].copy_from_slice(&[48, 62, 82, 255]);
            let last = ((width - 1) * 4) as usize;
            row[last..last + 4].copy_from_slice(&[48, 62, 82, 255]);
        }
    }

    let draw_row = |state: &mut PageFonts,
                    rgba: &mut [u8],
                    row_idx: u16,
                    line: &str,
                    fg: [u8; 3],
                    style: u8| {
        let y_origin = (u32::from(row_idx) * ch) as i32;
        for (col_idx, ch_char) in line.chars().take(usize::from(cols)).enumerate() {
            if ch_char == ' ' || ch_char == '\t' {
                continue;
            }
            if state.glyphs.len() > 2048 {
                state.glyphs.clear();
            }
            let fonts = &mut state.fonts;
            let pic = state.glyphs.entry((ch_char, style)).or_insert_with(|| {
                let mut buf = [0u8; 4];
                fonts.draw(ch_char.encode_utf8(&mut buf), style, 1)
            });
            let Some(pic) = pic.as_ref() else {
                continue;
            };
            let x_origin = (col_idx as u32 * cw) as i32;
            for py in 0..pic.height {
                let dy = y_origin + pic.top + py as i32;
                if dy < 0 || dy >= height as i32 {
                    continue;
                }
                for px in 0..pic.width {
                    let dx = x_origin + pic.left + px as i32;
                    if dx < 0 || dx >= width as i32 {
                        continue;
                    }
                    let src = (py * pic.width + px) as usize;
                    let alpha = if pic.colored {
                        pic.data[src * 4 + 3]
                    } else {
                        pic.data[src]
                    };
                    if alpha == 0 {
                        continue;
                    }
                    let dst = ((dy as u32 * width + dx as u32) * 4) as usize;
                    let a = u16::from(alpha);
                    let inv = 255 - a;
                    rgba[dst] = ((u16::from(fg[0]) * a + u16::from(rgba[dst]) * inv) / 255) as u8;
                    rgba[dst + 1] =
                        ((u16::from(fg[1]) * a + u16::from(rgba[dst + 1]) * inv) / 255) as u8;
                    rgba[dst + 2] =
                        ((u16::from(fg[2]) * a + u16::from(rgba[dst + 2]) * inv) / 255) as u8;
                    rgba[dst + 3] = 255;
                }
            }
        }
    };

    // Row 0: receipt header.
    draw_row(state, &mut rgba, 0, receipt, [138, 162, 190], 0);

    // Rows 1..rows: wrapped document lines starting at `scroll`.
    let body_rows = usize::from(rows.saturating_sub(1));
    if body_rows > 0 {
        let wrap_cols = usize::from(cols.max(1));
        let mut current_line = 0usize;
        let mut drawn = 0usize;
        'outer: for raw_line in text.split('\n') {
            let trimmed = raw_line.trim_start();
            let heading = trimmed.starts_with('#');
            let (fg, style) = if heading {
                ([232, 194, 112], BOLD)
            } else {
                ([218, 226, 236], 0)
            };
            if raw_line.is_empty() {
                if current_line >= usize::from(scroll) {
                    drawn += 1;
                    if drawn >= body_rows {
                        break;
                    }
                }
                current_line += 1;
                continue;
            }
            let mut chunk_start = 0usize;
            let mut col_count = 0usize;
            for (idx, c) in raw_line.char_indices() {
                let w = if c == '\t' { 4 } else { 1 };
                if col_count + w > wrap_cols && idx > chunk_start {
                    if current_line >= usize::from(scroll) {
                        draw_row(
                            state,
                            &mut rgba,
                            (1 + drawn) as u16,
                            &raw_line[chunk_start..idx],
                            fg,
                            style,
                        );
                        drawn += 1;
                        if drawn >= body_rows {
                            break 'outer;
                        }
                    }
                    current_line += 1;
                    chunk_start = idx;
                    col_count = 0;
                }
                col_count += w;
            }
            if chunk_start < raw_line.len() {
                if current_line >= usize::from(scroll) {
                    draw_row(
                        state,
                        &mut rgba,
                        (1 + drawn) as u16,
                        &raw_line[chunk_start..],
                        fg,
                        style,
                    );
                    drawn += 1;
                    if drawn >= body_rows {
                        break 'outer;
                    }
                }
                current_line += 1;
            }
        }
    }

    Some((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_faces_have_a_bound() {
        assert!(room_for_fallback(0));
        assert!(room_for_fallback(MAX_FALLBACK_FACES - 1));
        assert!(!room_for_fallback(MAX_FALLBACK_FACES));
        assert!(!room_for_fallback(usize::MAX));
    }

    #[test]
    fn open_rejects_non_fonts_and_oversized_files() {
        let dir = std::env::temp_dir();
        let junk = dir.join(format!("glass-font-junk-{}.ttf", std::process::id()));
        std::fs::write(&junk, b"not a font").unwrap();
        assert!(open(&junk, 0).is_none());
        // Sparse: oversized by metadata alone, so this stays instant.
        let huge = dir.join(format!("glass-font-huge-{}.ttc", std::process::id()));
        std::fs::File::create(&huge)
            .unwrap()
            .set_len(MAX_FONT_BYTES + 1)
            .unwrap();
        assert!(open(&huge, 0).is_none());
        let _ = std::fs::remove_file(&junk);
        let _ = std::fs::remove_file(&huge);
    }

    #[test]
    fn downscale_keeps_solid_colour() {
        let src = [10u8, 20, 30, 255].repeat(16);
        let out = downscale_rgba(&src, 4, 4, 2, 2);
        assert_eq!(&out[..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn rasters_are_made_to_match_their_claimed_size() {
        // Short data is padded, long data cut, absurd sizes rejected.
        let (w, h, data) = normalize(vec![7, 8], 2, 2, false).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(data, [7, 8, 0, 0]);
        let (_, _, data) = normalize(vec![1; 99], 2, 1, true).unwrap();
        assert_eq!(data.len(), 8);
        assert_eq!(normalize(vec![], 100_000, 1, false), None);
        assert_eq!(normalize(vec![], 1, 100_000, true), None);
    }

    #[test]
    fn render_page_rasterizes_receipt_and_scrolled_document_lines() {
        assert!(render_page("receipt", "hello", 0, 0, 4, (10, 20)).is_none());
        assert!(render_page("receipt", "hello", 0, 10, 4, (0, 20)).is_none());
        if Fonts::load("monospace", 14.0, 1.0).is_ok() {
            let (w, h, page0) =
                render_page("UTF-8 snapshot", "# Heading\nBody line two", 0, 20, 4, (10, 20))
                    .expect("page rasterized");
            assert_eq!((w, h), (200, 80));
            assert_eq!(page0.len(), (200 * 80 * 4) as usize);
            let (_, _, page1) =
                render_page("UTF-8 snapshot", "# Heading\nBody line two", 1, 20, 4, (10, 20))
                    .expect("scrolled page rasterized");
            assert_ne!(page0, page1, "scrolling shifts the rasterized document body");
        }
    }
}
