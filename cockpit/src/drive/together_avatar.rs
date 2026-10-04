//! The avatar contract: a player's own image model draws their knight; this
//! module checks and normalises the upload. Only the normalised sprite and
//! portrait ever reach the arena or the other player. See docs/DELVE.md.

use image::{ImageFormat, ImageReader, Rgba, RgbaImage};
use std::collections::HashMap;
use std::io::Cursor;

pub(crate) const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_SIDE: u32 = 2048;
pub(crate) const SPRITE: u32 = 32;
pub(crate) const PORTRAIT: u32 = 128;
const MAX_COLOURS: usize = 16;
/// Corner-colour flood tolerance, per channel.
const BACKGROUND_TOLERANCE: u8 = 24;
/// Opaque pieces smaller than this share of the figure are stray specks.
const SPECK: f64 = 0.01;
/// One character: its largest connected piece holds at least this share.
const ONE_FIGURE: f64 = 0.60;
/// The arena and terminal need a colour that shows on a dark floor: its
/// brightest channel must reach this.
const MIN_VALUE: u8 = 110;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Avatar {
    /// 32×32 PNG for the arena.
    pub(crate) sprite: Vec<u8>,
    /// 128×128 PNG for the player cards.
    pub(crate) portrait: Vec<u8>,
    /// The sprite's most common colour, for the terminal arena.
    pub(crate) colour: [u8; 3],
    /// Decoded, normalized sprite; avoids PNG decoding on every game frame.
    pub(crate) pixels: Vec<[u8; 4]>,
}

#[derive(Debug)]
pub(crate) struct Checked {
    pub(crate) avatar: Avatar,
    pub(crate) notes: Vec<String>,
}

/// The instructions a player gives their image model.
pub(crate) fn instructions(description: &str) -> String {
    let description = description.trim();
    let description = if description.is_empty() {
        "a brave knight in any style you like"
    } else {
        description
    };
    format!(
        "Make my knight's avatar for a top-down co-op bullet hell: {description}.
One character, full body, seen from above at a slight angle, facing up.
Pixel-art style with a bold, readable silhouette and at most 16 colours.
Centre it on a transparent background, or one flat colour, with nothing else
in the image: no text, frame, scenery or ground shadow. Square, at least
256×256 pixels. Save it as a PNG."
    )
}

/// The message a player carries back to their image model.
pub(crate) fn fix_it(errors: &[String]) -> String {
    format!(
        "The game could not use that avatar:\n{}\nPlease make it again following my original instructions.",
        errors
            .iter()
            .map(|error| format!("- {error}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub(crate) fn check(bytes: &[u8]) -> Result<Checked, Vec<String>> {
    if bytes.len() > MAX_BYTES {
        return Err(vec![format!(
            "the file is over {} MiB",
            MAX_BYTES / (1024 * 1024)
        )]);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| vec!["the file could not be read as an image".to_string()])?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(vec!["the file must be a PNG, JPEG or WebP image".into()]);
    }
    // The header is read before anything is decoded.
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .and_then(|reader| reader.into_dimensions().map_err(std::io::Error::other))
        .map_err(|_| vec!["the image header could not be read".to_string()])?;
    if width > MAX_SIDE || height > MAX_SIDE || width == 0 || height == 0 {
        return Err(vec![format!(
            "the image is {width}×{height}; each side must be 1–{MAX_SIDE} pixels"
        )]);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut reader = reader;
    reader.limits(limits);
    let mut image = reader
        .decode()
        .map_err(|_| vec!["the image could not be decoded".to_string()])?
        .to_rgba8();
    let mut notes = Vec::new();
    if !image.pixels().any(|pixel| pixel[3] < 128) && remove_background(&mut image) {
        notes.push("removed the flat background colour".into());
    }
    let total = u64::from(width) * u64::from(height);
    let opaque = image.pixels().filter(|pixel| pixel[3] >= 128).count() as u64;
    let share = opaque as f64 / total as f64;
    if share < 0.10 {
        return Err(vec![format!(
            "the knight covers only {:.0}% of the picture; draw it larger, filling most of the square",
            share * 100.0
        )]);
    }
    if share > 0.90 {
        return Err(vec![format!(
            "the knight and background cover {:.0}% of the picture; use a transparent or single flat-colour background with nothing else in it",
            share * 100.0
        )]);
    }
    let (specks, largest) = keep_figure(&mut image);
    if largest < ONE_FIGURE {
        return Err(vec![format!(
            "the picture has several separate things in it (the biggest is {:.0}% of the drawing); draw one character alone on a plain background",
            largest * 100.0
        )]);
    }
    if specks > 0 {
        notes.push(format!("cleared {specks} stray speck(s)"));
    }
    let square = crop_square(&image);
    let palette = palette(&square);
    if palette.len() == MAX_COLOURS {
        notes.push(format!("reduced to {MAX_COLOURS} colours"));
    }
    let sprite = reduce(&square, SPRITE, &palette);
    let portrait = reduce(&square, PORTRAIT, &palette);
    let colour = dominant(&sprite);
    Ok(Checked {
        avatar: Avatar {
            pixels: sprite.pixels().map(|pixel| pixel.0).collect(),
            sprite: png(&sprite),
            portrait: png(&portrait),
            colour,
        },
        notes,
    })
}

fn close(a: &Rgba<u8>, b: &Rgba<u8>) -> bool {
    (0..3).all(|channel| a[channel].abs_diff(b[channel]) <= BACKGROUND_TOLERANCE)
}

/// With no transparency, the colour shared by all four corners is flooded away
/// from the edges. Returns whether anything was removed.
fn remove_background(image: &mut RgbaImage) -> bool {
    let (width, height) = image.dimensions();
    let corners = [
        *image.get_pixel(0, 0),
        *image.get_pixel(width - 1, 0),
        *image.get_pixel(0, height - 1),
        *image.get_pixel(width - 1, height - 1),
    ];
    let background = corners[0];
    if !corners.iter().all(|corner| close(corner, &background)) {
        return false;
    }
    let mut seen = vec![false; (width * height) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..width {
        stack.push((x, 0));
        stack.push((x, height - 1));
    }
    for y in 0..height {
        stack.push((0, y));
        stack.push((width - 1, y));
    }
    let mut removed = false;
    while let Some((x, y)) = stack.pop() {
        let index = (y * width + x) as usize;
        if seen[index] {
            continue;
        }
        seen[index] = true;
        if !close(image.get_pixel(x, y), &background) {
            continue;
        }
        image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
        removed = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < width {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < height {
            stack.push((x, y + 1));
        }
    }
    removed
}

/// Label the 4-connected opaque pieces, clear the specks, and return how many
/// were cleared and the largest piece's share of the whole drawing.
fn keep_figure(image: &mut RgbaImage) -> (usize, f64) {
    let (width, height) = image.dimensions();
    let opaque = |image: &RgbaImage, x: u32, y: u32| image.get_pixel(x, y)[3] >= 128;
    let mut label = vec![0_u32; (width * height) as usize];
    let mut pieces: Vec<Vec<(u32, u32)>> = Vec::new();
    for start_y in 0..height {
        for start_x in 0..width {
            if label[(start_y * width + start_x) as usize] != 0 || !opaque(image, start_x, start_y)
            {
                continue;
            }
            let id = pieces.len() as u32 + 1;
            let mut piece = Vec::new();
            let mut stack = vec![(start_x, start_y)];
            label[(start_y * width + start_x) as usize] = id;
            while let Some((x, y)) = stack.pop() {
                piece.push((x, y));
                let neighbours = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                for (nx, ny) in neighbours {
                    if nx < width && ny < height {
                        let index = (ny * width + nx) as usize;
                        if label[index] == 0 && opaque(image, nx, ny) {
                            label[index] = id;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            pieces.push(piece);
        }
    }
    let total: usize = pieces.iter().map(Vec::len).sum();
    let mut specks = 0;
    for piece in &pieces {
        if (piece.len() as f64) < total as f64 * SPECK {
            specks += 1;
            for &(x, y) in piece {
                image.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    // Specks still count against the figure: a picture that crumbles into
    // fragments is not one character.
    let largest = pieces.iter().map(Vec::len).max().unwrap_or(0);
    (
        specks,
        if total == 0 {
            0.0
        } else {
            largest as f64 / total as f64
        },
    )
}

/// Crop to the opaque bounds and pad to a centred square.
fn crop_square(image: &RgbaImage) -> RgbaImage {
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] >= 128 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    let (width, height) = (right - left + 1, bottom - top + 1);
    let side = width.max(height);
    let mut square = RgbaImage::new(side, side);
    let (dx, dy) = ((side - width) / 2, (side - height) / 2);
    for y in 0..height {
        for x in 0..width {
            square.put_pixel(dx + x, dy + y, *image.get_pixel(left + x, top + y));
        }
    }
    square
}

/// The 16 most common colours among opaque pixels, after rounding each
/// channel to 16 levels. Ties break on the colour value, so it is stable.
fn palette(image: &RgbaImage) -> Vec<[u8; 3]> {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for pixel in image.pixels().filter(|pixel| pixel[3] >= 128) {
        let key = [
            pixel[0] & 0xf0 | 0x08,
            pixel[1] & 0xf0 | 0x08,
            pixel[2] & 0xf0 | 0x08,
        ];
        *counts.entry(key).or_default() += 1;
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(MAX_COLOURS)
        .map(|(colour, _)| colour)
        .collect()
}

/// Nearest-neighbour reduction, alpha cut at 50%, colours mapped to the palette.
fn reduce(square: &RgbaImage, side: u32, palette: &[[u8; 3]]) -> RgbaImage {
    let source = square.width();
    RgbaImage::from_fn(side, side, |x, y| {
        let pixel = square.get_pixel(
            ((x as u64 * source as u64 + source as u64 / 2) / side as u64).min(source as u64 - 1)
                as u32,
            ((y as u64 * source as u64 + source as u64 / 2) / side as u64).min(source as u64 - 1)
                as u32,
        );
        if pixel[3] < 128 {
            return Rgba([0, 0, 0, 0]);
        }
        let nearest = palette
            .iter()
            .min_by_key(|colour| {
                (0..3)
                    .map(|c| (i32::from(colour[c]) - i32::from(pixel[c])).pow(2))
                    .sum::<i32>()
            })
            .copied()
            .unwrap_or([pixel[0], pixel[1], pixel[2]]);
        Rgba([nearest[0], nearest[1], nearest[2], 255])
    })
}

fn value(colour: [u8; 3]) -> u8 {
    colour.into_iter().max().unwrap_or(0)
}

/// The most common colour bright enough to show on the dark arena floor; a
/// knight drawn only in dark colours gets its main colour lifted instead.
fn dominant(sprite: &RgbaImage) -> [u8; 3] {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for pixel in sprite.pixels().filter(|pixel| pixel[3] == 255) {
        *counts.entry([pixel[0], pixel[1], pixel[2]]).or_default() += 1;
    }
    let mut ranked: Vec<_> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    if let Some((colour, _)) = ranked
        .iter()
        .find(|(colour, _)| value(*colour) >= MIN_VALUE)
    {
        return *colour;
    }
    let Some(&(base, _)) = ranked.first() else {
        return [200, 200, 200];
    };
    // Mix toward white until it shows.
    let mut colour = base;
    while value(colour) < MIN_VALUE {
        colour = colour.map(|channel| channel + (255 - channel) / 4 + 1);
    }
    colour
}

fn png(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("an in-memory PNG encodes");
    bytes
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_avatar__tests.rs"]
mod tests;
