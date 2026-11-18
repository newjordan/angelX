use super::Delve;
pub use angel_plug::screen::WorldPixelsState;
use angel_plug::screen::{Console, Plate, PlatePixels, Screen};
use angel_plug::viewer::{Graphics, PortalCacheKey, ViewerArt};
use base64::Engine;
use ratatui::{
    Frame,
    layout::{Rect, Size},
};
use std::fmt::Write;
use std::sync::{Arc, mpsc};

pub(crate) fn game_placement(console: &dyn Console, area: Rect, size: (u32, u32)) -> Rect {
    let (cols, rows) = game_cells(console, area, size);
    Rect::new(
        area.x + (area.width - cols) / 2,
        area.y + (area.height - rows) / 2,
        cols,
        rows,
    )
}

pub(crate) fn game_cells(console: &dyn Console, area: Rect, (fw, fh): (u32, u32)) -> (u16, u16) {
    let (cw, ch) = cell_shape(console);
    let (aw, ah) = (f32::from(area.width) * cw, f32::from(area.height) * ch);
    let k = (aw / fw.max(1) as f32).min(ah / fh.max(1) as f32);
    let cols = ((fw as f32 * k / cw).round() as u16).clamp(1, area.width.min(256));
    let rows = ((fh as f32 * k / ch).round() as u16).clamp(1, area.height.min(256));
    (cols, rows)
}

pub fn cell_shape(console: &dyn Console) -> (f32, f32) {
    static SHAPE: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *SHAPE.get_or_init(|| {
        console
            .real_cell_size()
            .map_or((10.0, 20.0), |(w, h)| (f32::from(w), f32::from(h)))
    })
}

pub(crate) fn upload_stream(
    console: &dyn Console,
    key: u64,
    size: (u32, u32),
    span: (u16, u16),
    rgba: Vec<u8>,
) -> Option<u32> {
    console.store(key, size, span, rgba, true)
}

pub(crate) fn png_upload(console: &dyn Console, png: &[u8], size: Size, id: u32) -> Option<String> {
    if size.width == 0
        || size.height == 0
        || size.width > 256
        || size.height > 256
        || id == 0
        || id > 0x00ff_ffff
        || png.len() > 3 * 1024 * 1024
    {
        return None;
    }
    let payload = base64::engine::general_purpose::STANDARD.encode(png);
    let (start, escape, end) = if console.tmux_passthrough() {
        ("\x1bPtmux;", "\x1b\x1b", "\x1b\\")
    } else {
        ("", "\x1b", "")
    };
    let mut upload = String::with_capacity(payload.len() + payload.len() / 64 + 64);
    let chunks = payload.as_bytes().chunks(4096);
    let count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        let more = u8::from(index + 1 < count);
        upload.push_str(start);
        if index == 0 {
            write!(
                upload,
                "{escape}_Gq=2,a=T,U=1,f=100,C=1,i={id},c={},r={},m={more};",
                size.width, size.height
            )
            .unwrap();
        } else {
            write!(upload, "{escape}_Gq=2,m={more};").unwrap();
        }
        upload.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        upload.push_str(escape);
        upload.push('\\');
        upload.push_str(end);
    }
    Some(upload)
}

#[derive(Default)]
pub struct State {
    pub(crate) map_enabled: bool,
    pub(crate) map_area: Option<Rect>,
    pub game_png: Option<(u64, u16, u16, u32)>,
    pub game_upload: Option<String>,
    pub(crate) game_pipeline: Option<crate::game_pipeline::Pipeline>,
    pub(crate) map_png_ready: Option<(PortalCacheKey, Arc<Vec<u8>>, u32, u32)>,
    pub(crate) map_png_pending:
        Option<(PortalCacheKey, mpsc::Receiver<Option<(Vec<u8>, u32, u32)>>)>,
}

impl State {
    pub(crate) fn new(graphics: Graphics) -> Self {
        Self {
            map_enabled: map_desired(graphics),
            ..Self::default()
        }
    }
}

impl ViewerArt for Delve {
    type Pixels = State;

    fn pixels(graphics: Graphics) -> State {
        State::new(graphics)
    }

    fn take_upload(pixels: &mut State) -> Option<String> {
        pixels.game_upload.take()
    }

    #[cfg(feature = "glass")]
    fn squires() -> (Option<[[u8; 3]; 3]>, Vec<[[u8; 3]; 3]>, Vec<Vec<String>>) {
        let serving = crate::houses::serving();
        let lead = serving.lead.and_then(livery_of);
        let seated: Vec<_> = serving
            .seated
            .iter()
            .filter(|&&id| livery_of(id).is_some())
            .copied()
            .collect();
        let retinue = seated.iter().filter_map(|&id| livery_of(id)).collect();
        let names = std::iter::once(serving.lead)
            .chain(seated.iter().map(|&id| Some(id)))
            .map(|house| {
                house
                    .and_then(|id| {
                        let house = crate::houses::get(id);
                        let knight = house.knight(0)?;
                        let mut lines = vec![format!("{} of {}", knight.name, house.castle)];
                        if !knight.epithet.is_empty() {
                            lines.push(knight.epithet.clone());
                        }
                        Some(lines)
                    })
                    .unwrap_or_else(|| vec!["A squire of the Keep".to_string()])
            })
            .collect();
        (lead, retinue, names)
    }
}

#[cfg(feature = "glass")]
fn livery_of(id: crate::houses::HouseId) -> Option<[[u8; 3]; 3]> {
    use angel_delve::render::overworld::{ink::ink, light};
    let house = crate::houses::get(id);
    let field = ink(light::dusk_ink(house.field))?;
    let charge = ink(light::dusk_ink(house.charge))?;
    Some([field, light::darker(field), charge])
}

impl<S: Screen<State> + ?Sized> Pictures for S {}

pub trait Pictures: Screen<State> {
    fn render_map_pixels<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> WorldPixelsState
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels_inner(frame, area, sequence, false, true, true, compose)
    }

    fn map_pixels_native(&mut self) -> bool {
        self.pixels().map_enabled
    }

    #[cfg(any(test, feature = "testing"))]
    fn render_game_img(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        size: (u32, u32),
        compose: impl FnOnce() -> crate::world_viz::overworld::Img,
    ) -> bool {
        let shape = cell_shape(self.console());
        self.place_game(frame, area, sequence, size, |cols, rows| {
            Some(scaled_png(&compose(), cols, rows, shape))
        })
    }

    fn render_game_async(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        size: (u32, u32),
        budget: std::time::Duration,
        compose: impl FnOnce() -> crate::world_viz::overworld::Img + Send + 'static,
    ) -> bool {
        if self.graphics() != Graphics::Kitty || area.width == 0 || area.height == 0 {
            return false;
        }
        let (cols, rows) = game_cells(self.console(), area, size);
        let shape = cell_shape(self.console());
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (sequence, cols, rows).hash(&mut hasher);
        let key = hasher.finish() >> 1;
        let plane = self.pixels();
        let pipeline = plane.game_pipeline.get_or_insert_with(Default::default);
        pipeline.request(
            key,
            Box::new(move || {
                let img = compose();
                Some(scaled_png(&img, cols, rows, shape))
            }),
        );
        let shown = plane
            .game_png
            .is_some_and(|(_, c, r, _)| (c, r) == (cols, rows));
        let budget = if shown {
            budget
        } else {
            std::time::Duration::from_millis(250)
        };
        let ready = match pipeline.wait(key, budget) {
            Some(png) => Some((key, png)),
            None => pipeline.latest(),
        };
        match ready {
            Some((key, png)) => self.place_game(frame, area, key, size, |_, _| Some(png.to_vec())),
            None => false,
        }
    }

    fn render_map_png<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> WorldPixelsState
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        let key = PortalCacheKey {
            sequence,
            width: area.width,
            height: area.height,
        };
        let plane = self.pixels();
        if let Some((pending, rx)) = plane.map_png_pending.as_ref() {
            match rx.try_recv() {
                Ok(Some((png, w, h))) => {
                    plane.map_png_ready = Some((pending.clone(), Arc::new(png), w, h));
                    plane.map_png_pending = None;
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => plane.map_png_pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let fresh = plane.map_png_ready.as_ref().is_some_and(|r| r.0 == key);
        if !fresh && plane.map_png_pending.is_none() {
            let (pixels, w, h) = compose();
            let (tx, rx) = mpsc::channel();
            let glass = self.console().glass_active();
            let job: Box<dyn FnOnce() + Send> = Box::new(move || {
                use image::ImageEncoder;
                if glass {
                    let _ = tx.send(Some((glass_frame(pixels.as_ref(), w, h), w, h)));
                    return;
                }
                let mut png = Vec::with_capacity(256 * 1024);
                let ok = image::codecs::png::PngEncoder::new_with_quality(
                    &mut png,
                    image::codecs::png::CompressionType::Fast,
                    image::codecs::png::FilterType::Up,
                )
                .write_image(pixels.as_ref(), w, h, image::ExtendedColorType::Rgba8)
                .is_ok();
                let _ = tx.send(ok.then_some((png, w, h)));
            });
            if self.send_job(job) {
                self.pixels().map_png_pending = Some((key, rx));
            }
        }
        match self.pixels().map_png_ready.clone() {
            Some((ready, png, w, h))
                if ready.width == area.width && ready.height == area.height =>
            {
                let seq = ready.sequence | (1 << 63);
                if self.place_game(frame, area, seq, (w, h), |_, _| Some(png.to_vec())) {
                    self.pixels().map_area = Some(area);
                    WorldPixelsState::Ready
                } else {
                    WorldPixelsState::Unavailable
                }
            }
            _ => WorldPixelsState::Pending,
        }
    }

    fn place_game(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        (fw, fh): (u32, u32),
        encode: impl FnOnce(u16, u16) -> Option<Vec<u8>>,
    ) -> bool {
        if self.graphics() != Graphics::Kitty
            || area.width == 0
            || area.height == 0
            || fw == 0
            || fh == 0
        {
            return false;
        }
        let console = self.console();
        let placed = game_placement(console, area, (fw, fh));
        let (cols, rows) = (placed.width, placed.height);
        let buffer = frame.buffer_mut();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    cell.set_symbol(" ")
                        .set_bg(ratatui::style::Color::Rgb(0, 0, 0));
                }
            }
        }
        let dot_base_id = self.dot_base_id();
        let base = (dot_base_id & 0x00ff_fffc).max(4);
        let plane = self.pixels();
        let id = match plane.game_png {
            Some((seq, c, r, id)) if (seq, c, r) == (sequence, cols, rows) => id,
            current => {
                let next = if current.is_some_and(|(_, _, _, id)| id == base) {
                    base + 1
                } else {
                    base
                };
                let size = ratatui::layout::Size::new(cols, rows);
                let upload = encode(cols, rows).and_then(|bytes| match read_glass_frame(&bytes) {
                    Some((w, h, rgba)) => upload_stream(
                        console,
                        u64::from(dot_base_id),
                        (w, h),
                        (cols, rows),
                        rgba.to_vec(),
                    )
                    .map(|id| (id, String::new()))
                    .or_else(|| png_upload(console, &bytes, size, next).map(|s| (next, s))),
                    None => png_upload(console, &bytes, size, next).map(|s| (next, s)),
                });
                match upload {
                    Some((id, upload)) => {
                        plane.game_upload = (!upload.is_empty()).then_some(upload);
                        plane.game_png = Some((sequence, cols, rows, id));
                        id
                    }
                    None => match current {
                        Some((_, c, r, id)) if (c, r) == (cols, rows) => id,
                        _ => return false,
                    },
                }
            }
        };
        console.placeholders(
            id,
            ratatui::layout::Size::new(cols, rows),
            placed,
            frame.buffer_mut(),
        );
        true
    }

    #[cfg(any(test, feature = "testing"))]
    fn render_world_pixels<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        ambient: bool,
        compose: F,
    ) -> WorldPixelsState
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels_inner(frame, area, sequence, ambient, false, false, compose)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_world_pixels_inner<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        ambient: bool,
        retain: bool,
        pre_scaled: bool,
        compose: F,
    ) -> WorldPixelsState
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        let plane = self.pixels();
        plane.map_area = None;
        let map_enabled = plane.map_enabled;
        let graphics = self.graphics();
        if pre_scaled && graphics == Graphics::Kitty && map_enabled {
            return self.render_map_png(frame, area, sequence, compose);
        }
        let supported = map_enabled || (ambient && graphics == Graphics::Halfblocks);
        let mut compose = Some(compose);
        let (state, painted) = self.plate(
            frame,
            Plate {
                area,
                sequence,
                supported,
                retain,
                pre_scaled,
            },
            &mut || {
                compose.take().map(|compose| {
                    let (pixels, width, height) = compose();
                    PlatePixels {
                        pixels: Box::new(pixels),
                        width,
                        height,
                    }
                })
            },
        );
        self.pixels().map_area = painted;
        state
    }

    #[cfg(any(test, feature = "testing"))]
    fn render_native_location<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> bool
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels(frame, area, sequence, false, compose) == WorldPixelsState::Ready
    }

    #[cfg(any(test, feature = "testing"))]
    fn render_ambient_location<P, F>(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        sequence: u64,
        compose: F,
    ) -> bool
    where
        P: AsRef<[u8]> + Send + 'static,
        F: FnOnce() -> (P, u32, u32),
    {
        self.render_world_pixels(frame, area, sequence, true, compose) == WorldPixelsState::Ready
    }
}

pub fn map_desired(graphics: Graphics) -> bool {
    if graphics == Graphics::Halfblocks {
        return false;
    }
    if let Ok(raw) = std::env::var("ANGEL_BACKED_MAP").or_else(|_| std::env::var("ANGEL_TILE_MAP"))
    {
        let off = matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "off" | "false" | "no"
        );
        return !off;
    }
    matches!(graphics, Graphics::Kitty | Graphics::Iterm2)
}

pub fn scaled_png(
    img: &crate::world_viz::overworld::Img,
    cols: u16,
    rows: u16,
    (cw, ch): (f32, f32),
) -> Vec<u8> {
    let (tw, th) = (f32::from(cols) * cw, f32::from(rows) * ch);
    let k = (tw / img.w.max(1) as f32)
        .max(th / img.h.max(1) as f32)
        .ceil()
        .clamp(1.0, 3.0) as u32;
    nearest_png(img, img.w.max(1) as u32 * k, img.h.max(1) as u32 * k)
}

const GLASS_FRAME: &[u8; 4] = b"GLRW";

fn glass_frame(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + rgba.len());
    out.extend_from_slice(GLASS_FRAME);
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(rgba);
    out
}

fn read_glass_frame(bytes: &[u8]) -> Option<(u32, u32, &[u8])> {
    let rest = bytes.strip_prefix(GLASS_FRAME)?;
    let w = u32::from_le_bytes(rest.get(0..4)?.try_into().ok()?);
    let h = u32::from_le_bytes(rest.get(4..8)?.try_into().ok()?);
    let rgba = rest.get(8..)?;
    (w > 0 && h > 0 && rgba.len() == (w as usize) * (h as usize) * 4).then_some((w, h, rgba))
}

fn nearest_png(img: &crate::world_viz::overworld::Img, w: u32, h: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let (sw, sh) = (img.w.max(1) as u32, img.h.max(1) as u32);
    let src = img.rgb_bytes();
    let mut out = vec![0u8; (w * h * 3) as usize];
    let cols: Vec<usize> = (0..w)
        .map(|x| ((x * sw / w).min(sw - 1) * 3) as usize)
        .collect();
    for y in 0..h {
        let sy = (y * sh / h).min(sh - 1) as usize;
        let row = &src[sy * sw as usize * 3..(sy + 1) * sw as usize * 3];
        let dst = &mut out[(y * w * 3) as usize..((y + 1) * w * 3) as usize];
        for (x, &sx) in cols.iter().enumerate() {
            dst[x * 3..x * 3 + 3].copy_from_slice(&row[sx..sx + 3]);
        }
    }
    let mut png = Vec::with_capacity(256 * 1024);
    let _ = image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Up,
    )
    .write_image(&out, w, h, image::ExtendedColorType::Rgb8);
    png
}

#[cfg(all(test, feature = "glass"))]
mod tests {
    use super::*;

    #[test]
    fn every_house_dresses_its_squire() {
        let mut seen = std::collections::HashSet::new();
        for id in crate::houses::ids() {
            let [field, shadow, charge] = livery_of(id).expect("every house has inks");
            if let Ok(path) = std::env::var("ANGEL_SQUIRE_LIVERIES") {
                use std::io::Write as _;
                let mut out = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .unwrap();
                let key = &crate::houses::get(id).key;
                writeln!(out, "{key} {field:?} {shadow:?} {charge:?}").unwrap();
            }
            seen.insert((field, charge));
        }
        assert!(seen.len() > 8, "houses are told apart by their colours");
    }
}
