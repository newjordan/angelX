//! Sprite sheets: a PNG strip of up to four frames, cut into realm-ink rows.
//!
//! A card (or a phrasebook wish, or a realm wish) may name a picture instead
//! of drawing its sprite as text: `sheet <slot> <file.png>`, the file in
//! `.angel/dungeon/sheets/` (for the realm, `.angel/realm/sheets/`). When
//! the card loads, the line is replaced by the `draw <slot>` block the
//! picture makes, so the card that is checked, saved and sent to a friend is
//! plain checked text, the same as one drawn by hand.
//!
//! The picture goes through the avatar's checks (2 MiB, PNG/JPEG/WebP, at
//! most 2048 a side, a flat background flooded away, sixteen colours) and is
//! then cut: the strip into equal frames side by side, each frame cropped to
//! what all frames draw (so an animation does not jitter), shrunk by area to
//! the slot's size, and every pixel put on the nearest realm ink.
//!
//! A `paint <slot> <file.png>` line is the painted twin of a sheet: the
//! picture (a PNG on a transparent background, at most 4 MiB, 2048 a side)
//! is cut into the same rows, and also kept whole in the workspace's paint
//! store under its sha256, which the `draw` block names (`paint=<sha256>`)
//! so a client that has the picture shows it in the inks' place. It may
//! name its normal map too (`normal=<file_n.png>`, the painting's size,
//! green up): kept the same way, named `normal=<sha256>`, it is which way
//! each of the painting's pixels faces in a lit room. `pixel` says the
//! painting is pixel art (a window draws it with hard pixel edges). Rows of
//! inks right under a paint line are its inks in place of the cut, checked
//! as drawn rows are: as many frames as the painting has.

use crate::together_avatar;
use crate::together_shooter::looks::{self, MAX_FRAMES, Move, Slot};
use crate::together_shooter::script::INKS;
use image::RgbaImage;
use std::io::Read;

/// Where a workspace keeps the delve's sheets.
pub fn dir(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join(".angel/dungeon/sheets")
}

/// Where a workspace keeps the delve's paintings, each as
/// `<sha256>.png`: the host's paint store, which its friends fetch from.
pub fn paint_dir(workspace: &std::path::Path) -> std::path::PathBuf {
    workspace.join(".angel/dungeon/paint")
}

/// A painting is a PNG of at most 4 MiB.
pub const MAX_PAINT_BYTES: usize = 4 * 1024 * 1024;

/// The ink nearest a colour, over the whole realm palette.
fn nearest_ink(c: [u8; 3]) -> char {
    use angel_delve::render::overworld::ink::ink;
    let mut best = ('.', i32::MAX);
    for ch in INKS.chars() {
        let Some(p) = ink(ch) else { continue };
        let d: i32 = (0..3)
            .map(|i| (i32::from(p[i]) - i32::from(c[i])).pow(2))
            .sum();
        if d < best.1 {
            best = (ch, d);
        }
    }
    best.0
}

/// Cut a sheet into frames of ink rows, `side` pixels square. `frames`
/// counts the strip's frames; none guesses it from the picture's shape.
pub fn cut(
    bytes: &[u8],
    frames: Option<usize>,
    side: u32,
) -> Result<Vec<Vec<String>>, Vec<String>> {
    let (image, size) = together_avatar::decode(bytes)?;
    cut_image(image, size, frames, side)
}

/// `cut`, of a picture already read.
fn cut_image(
    mut image: RgbaImage,
    (width, height): (u32, u32),
    frames: Option<usize>,
    side: u32,
) -> Result<Vec<Vec<String>>, Vec<String>> {
    if !(4..=32).contains(&side) {
        return Err(vec!["a sheet's target size must be 4–32 pixels".into()]);
    }
    if !image.pixels().any(|p| p[3] < 128) && !together_avatar::remove_background(&mut image) {
        return Err(vec![
            "the sheet has no transparent or flat background; draw the frames on one flat colour"
                .into(),
        ]);
    }
    let guess = ((width as f32 / height as f32).round() as usize).max(1);
    let count = frames.unwrap_or(guess);
    if count == 0 || count > MAX_FRAMES {
        return Err(vec![format!(
            "the sheet is {count} frames; a sprite has 1 to {MAX_FRAMES}, side by side"
        )]);
    }
    let cell = width / count as u32;
    if cell == 0 {
        return Err(vec!["the sheet is narrower than its frames".into()]);
    }
    if width % count as u32 != 0 {
        return Err(vec![
            "the sheet width must divide evenly into its frames".into(),
        ]);
    }
    // What every frame draws, together: one box, so the frames line up.
    let Some(crop) = looks::crop(width, height, count, |x, y| image.get_pixel(x, y)[3] >= 128)
    else {
        return Err(vec!["the sheet has nothing drawn on it".into()]);
    };
    let (span, ox, oy) = (crop.span, crop.x, crop.y);
    let palette = together_avatar::palette(&image);
    let inks: Vec<char> = palette.iter().map(|&c| nearest_ink(c)).collect();
    let mut out = Vec::new();
    for f in 0..count as u32 {
        let mut rows = Vec::new();
        for ty in 0..side {
            let mut row = String::new();
            for tx in 0..side {
                // The source box this pixel covers.
                let x0 = ox + i64::from(tx * span / side);
                let x1 = (ox + i64::from(((tx + 1) * span).div_ceil(side))).max(x0 + 1);
                let y0 = oy + i64::from(ty * span / side);
                let y1 = (oy + i64::from(((ty + 1) * span).div_ceil(side))).max(y0 + 1);
                let (mut n, mut lit, mut sum) = (0u64, 0u64, [0u64; 3]);
                for y in y0..y1 {
                    for x in x0..x1 {
                        n += 1;
                        if x < 0 || y < 0 || x >= i64::from(cell) || y >= i64::from(height) {
                            continue;
                        }
                        let p = image.get_pixel(f * cell + x as u32, y as u32);
                        if p[3] >= 128 {
                            lit += 1;
                            for c in 0..3 {
                                sum[c] += u64::from(p[c]);
                            }
                        }
                    }
                }
                if n == 0 || lit * 100 < n * 40 {
                    row.push('.');
                    continue;
                }
                let mean = sum.map(|s| (s / lit) as u8);
                let i = palette
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, c)| {
                        (0..3)
                            .map(|k| (i32::from(c[k]) - i32::from(mean[k])).pow(2))
                            .sum::<i32>()
                    })
                    .map_or(0, |(i, _)| i);
                row.push(inks.get(i).copied().unwrap_or_else(|| nearest_ink(mean)));
            }
            rows.push(row);
        }
        if rows.iter().all(|r| r.chars().all(|c| c == '.')) {
            return Err(vec![format!(
                "frame {} is empty at {side} pixels; draw it larger",
                f + 1
            )]);
        }
        out.push(rows);
    }
    Ok(out)
}

/// A picture line's word: a sheet, cut into rows, or a painting, cut and
/// kept whole.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sheet,
    Paint,
}

/// One `sheet` line: `sheet <slot> <file> [frames=N] [size=N] [at=place]`,
/// or one `paint` line, which may add `move=<words>`.
struct Line<'a> {
    slot: &'a str,
    file: &'a str,
    frames: Option<usize>,
    size: Option<u32>,
    at: Option<&'a str>,
    /// A painting's motion words.
    moves: Option<&'a str>,
    /// A painting's normal map, a file beside it.
    normal: Option<&'a str>,
    /// A painting that is pixel art.
    pixel: bool,
    /// In a phrasebook entry: the tier the look starts at.
    tier: Option<usize>,
}

fn parse(kind: Kind, rest: &str) -> Result<Line<'_>, String> {
    let mut parts = rest.split_whitespace();
    let (Some(slot), Some(file)) = (parts.next(), parts.next()) else {
        return Err(match kind {
            Kind::Sheet => {
                "a sheet line is `sheet <slot> <file.png> [frames=N] [size=N] [at=place]`".into()
            }
            Kind::Paint => "a paint line is `paint <slot> <file.png> [frames=N] [size=N] [at=place] [move=words] [normal=<file_n.png>] [pixel]`".into(),
        });
    };
    let word = match kind {
        Kind::Sheet => "sheet",
        Kind::Paint => "paint",
    };
    let mut line = Line {
        slot,
        file,
        frames: None,
        size: None,
        at: None,
        moves: None,
        normal: None,
        pixel: false,
        tier: None,
    };
    let mut seen = Vec::new();
    for part in parts {
        if kind == Kind::Paint && part == "pixel" {
            if line.pixel {
                return Err("`pixel` appears twice on the paint line".into());
            }
            line.pixel = true;
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or(("frames", part));
        if seen.contains(&key) {
            return Err(format!("`{key}` appears twice on the {word} line"));
        }
        seen.push(key);
        match key {
            "frames" => {
                line.frames = Some(
                    value
                        .parse()
                        .map_err(|_| format!("frames takes 1 to {MAX_FRAMES}"))?,
                )
            }
            "size" => line.size = Some(value.parse().map_err(|_| "size takes pixels".to_string())?),
            "at" => line.at = Some(value),
            "move" if kind == Kind::Paint => line.moves = Some(value),
            "normal" if kind == Kind::Paint => line.normal = Some(value),
            "tier" => {
                line.tier = Some(
                    value
                        .parse()
                        .map_err(|_| "tier takes a number".to_string())?,
                )
            }
            other if kind == Kind::Sheet => {
                return Err(format!(
                    "`{other}`: a sheet line takes frames=, size=, at= and (in the phrasebook) tier="
                ));
            }
            other => {
                return Err(format!(
                    "`{other}`: a paint line takes frames=, size=, at=, move=, normal=, pixel and (in the phrasebook) tier="
                ));
            }
        }
    }
    let plain = line
        .file
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let lower = line.file.to_ascii_lowercase();
    if kind == Kind::Paint && (!plain || line.file.starts_with('.') || !lower.ends_with(".png")) {
        return Err(format!(
            "`{}`: a painting is a plain file name ending .png (letters, digits, - and _), in the sheets folder",
            line.file
        ));
    }
    if !plain
        || line.file.starts_with('.')
        || !(lower.ends_with(".png") || lower.ends_with(".webp") || lower.ends_with(".jpg"))
    {
        return Err(format!(
            "`{}`: a sheet is a plain file name ending .png (letters, digits, - and _), in the sheets folder",
            line.file
        ));
    }
    Ok(line)
}

/// Replace every `sheet` and `paint` line of a card, wish or landmark file
/// with the sprite (or `art`) block its picture makes, reading the pictures
/// from `dir` and keeping each painting in `paint` (the workspace's
/// [`paint_dir`]). `art_side` is the face art's size (16 for a card, 32 for
/// a landmark). A paint line's own rows (right under it) go into its block
/// in place of the cut. Errors name their line.
pub fn inline(
    raw: &str,
    dir: &std::path::Path,
    paint: &std::path::Path,
    art_side: u32,
) -> Result<String, Vec<String>> {
    if !raw.lines().any(|l| sheet_line(l).is_some()) {
        return Ok(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len() * 2);
    let mut errors = Vec::new();
    let lines: Vec<&str> = raw.lines().collect();
    let name = lines.iter().find_map(|l| l.strip_prefix("name ").map(str::trim).or_else(|| l.strip_prefix("wish ").and_then(|s| s.split_once('|')).map(|(_, n)| n.trim()))).unwrap_or("");
    let default_part = looks::default_worn_part(name);
    let worn_parts: Vec<_> = lines.iter().filter_map(|line| {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.first() != Some(&"draw") || words.get(1) != Some(&"worn") { return None; }
        let tier = words.iter().skip(2).find_map(|w| w.parse::<usize>().ok()).unwrap_or(1);
        let at = words.iter().find_map(|w| w.strip_prefix("at=").or_else(|| w.strip_prefix("placed="))).unwrap_or(default_part);
        Some((tier, at))
    }).collect();
    let mut n = 0;
    while n < lines.len() {
        let line = lines[n];
        let Some((kind, rest)) = sheet_line(line) else {
            out.push_str(line);
            out.push('\n');
            n += 1;
            continue;
        };
        let start = n;
        n += 1;
        let mut rows = Vec::new();
        if kind == Kind::Paint {
            while n < lines.len() && crate::together_shooter::script::sprite_row(lines[n].trim()) {
                rows.push(lines[n].trim());
                n += 1;
            }
        }
        let inherited;
        let rest = if kind == Kind::Paint && rest.starts_with("worn ") && !rest.split_whitespace().any(|w| w.starts_with("at=")) {
            let tier = rest.split_whitespace().find_map(|w| w.strip_prefix("tier=").and_then(|t| t.parse::<usize>().ok())).unwrap_or(1);
            if let Some((_, at)) = worn_parts.iter().filter(|(t, _)| *t <= tier).max_by_key(|(t, _)| *t) {
                inherited = format!("{rest} at={at}");
                inherited.as_str()
            } else { rest }
        } else { rest };
        match block(kind, rest, (!rows.is_empty()).then_some(rows.as_slice()), dir, paint, art_side) {
            Ok(text) => {
                out.push_str(&text);
                out.push('\n');
            }
            Err(why) => errors.push(format!("line {}: {why}", start + 1)),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The word and rest of a `sheet` or `paint` line, when `line` is one.
fn sheet_line(line: &str) -> Option<(Kind, &str)> {
    let line = line.trim();
    let (word, rest) = line.split_once(char::is_whitespace)?;
    let kind = if word.eq_ignore_ascii_case("sheet") {
        Kind::Sheet
    } else if word.eq_ignore_ascii_case("paint") {
        Kind::Paint
    } else {
        return None;
    };
    Some((kind, rest.trim()))
}

fn block(
    kind: Kind,
    rest: &str,
    rows: Option<&[&str]>,
    dir: &std::path::Path,
    paint: &std::path::Path,
    art_side: u32,
) -> Result<String, String> {
    let line = parse(kind, rest)?;
    if rows.is_some() && line.size.is_some() {
        return Err("size= sizes the cut; a paint line with its own rows takes none".into());
    }
    let slot = if line.slot.eq_ignore_ascii_case("art") && kind == Kind::Sheet {
        None
    } else if kind == Kind::Paint {
        Some(Slot::parse(line.slot).ok_or_else(|| {
            format!(
                "paint slot `{}`: use worn, shot, orbit, summon or aura",
                line.slot
            )
        })?)
    } else {
        Some(Slot::parse(line.slot).ok_or_else(|| {
            format!(
                "sheet slot `{}`: use art, worn, shot, orbit, summon or aura",
                line.slot
            )
        })?)
    };
    let path = dir.join(line.file);
    let resolved = path
        .canonicalize()
        .map_err(|_| format!("no sheet {} (it belongs in {})", line.file, dir.display()))?;
    if !dir
        .canonicalize()
        .is_ok_and(|root| resolved.starts_with(root))
    {
        return Err(format!(
            "{}: a sheet must stay inside the sheets folder",
            line.file
        ));
    }
    let meta = std::fs::metadata(&path)
        .map_err(|_| format!("no sheet {} (it belongs in {})", line.file, dir.display()))?;
    let limit = match kind {
        Kind::Sheet => together_avatar::MAX_BYTES,
        Kind::Paint => MAX_PAINT_BYTES,
    };
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err(match kind {
            Kind::Sheet => format!("{}: a sheet is a picture of at most 2 MiB", line.file),
            Kind::Paint => format!("{}: a painting is a PNG of at most 4 MiB", line.file),
        });
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{}: {e}", line.file))?;
    let biggest = match slot {
        Some(slot) => slot.max_side().0.min(slot.max_side().1) as u32,
        None => art_side,
    };
    let side = line.size.unwrap_or(slot.map_or(art_side, Slot::sheet_side));
    if !(4..=biggest).contains(&side) {
        return Err(format!("size= takes 4–{biggest} pixels for {}", line.slot));
    }
    if slot.is_none()
        && (line.frames.is_some_and(|n| n != 1) || line.at.is_some() || line.tier.is_some())
    {
        return Err("sheet art is a single frame, without at= or tier=".into());
    }
    let frames = line.frames.or(slot.is_none().then_some(1));
    // A painting's own rows: checked as drawn rows (a worn piece placed
    // small is checked as one), as many frames as the painting.
    let own = match (rows, slot) {
        (Some(rows), Some(slot)) => {
            let rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
            let small = if line.at.is_some() { Slot::Worn } else { slot };
            let drawn = looks::Sprite::from_strip(small, &rows)?;
            if frames.is_some_and(|n| n != drawn.frames.len()) {
                return Err(format!(
                    "the rows under the paint line have {} frames, the painting {}",
                    drawn.frames.len(),
                    frames.unwrap_or(1)
                ));
            }
            Some(drawn.frames)
        }
        _ => None,
    };
    let frames = frames.or(own.as_ref().map(Vec::len));
    let mut painted_size = (0, 0);
    let cut = match kind {
        Kind::Sheet => cut(&bytes, frames, side),
        Kind::Paint => painting(&bytes).and_then(|(image, size)| {
            painted_size = size;
            cut_image(image, size, frames, side)
        }),
    }
    .map_err(|e| format!("{}: {}", line.file, e.join("; ")))?;
    // Its own rows stand for its inks; the cut only checked the picture.
    let cut = own.unwrap_or(cut);
    let normal = match line.normal {
        Some(file) => Some(normal_map(file, dir, painted_size)?),
        None => None,
    };
    let moves = match line.moves {
        Some(words) => Move::parse_list(words)?,
        None => Vec::new(),
    };
    let Some(slot) = slot else {
        let rows = cut.into_iter().next().unwrap_or_default();
        return Ok(format!("art\n{}\n", rows.join("\n")));
    };
    let frames: Vec<Vec<String>> = match (slot, line.at) {
        (Slot::Worn, at) => {
            crate::together_shooter::looks::place_on_body(&cut, at.unwrap_or("body"))?
        }
        (_, Some(_)) => return Err("at= places a worn sheet only".into()),
        (_, None) => cut,
    };
    // Kept only once the whole line has checked.
    let painted = match kind {
        Kind::Sheet => None,
        Kind::Paint => Some(
            keep_painting(&bytes, paint).map_err(|e| format!("{}: {}", line.file, e.join("; ")))?,
        ),
    };
    let normal = match (normal, line.normal) {
        (Some(bytes), Some(file)) => Some(
            keep_painting(&bytes, paint).map_err(|e| format!("{file}: {}", e.join("; ")))?,
        ),
        _ => None,
    };
    // A worn piece remembers the part of the knight it was put on.
    let at = (slot == Slot::Worn).then(|| looks::place(line.at.unwrap_or("body")).unwrap_or("body").to_string());
    let sprite = looks::Sprite {
        frames,
        paint: painted,
        normal,
        hd: None,
        pixel: line.pixel,
        moves,
        at,
        pal: Vec::new(),
    };
    let block = sprite.to_block(slot);
    Ok(match line.tier {
        Some(tier) => block.replacen(
            &format!("draw {}", slot.word()),
            &format!("draw {} {tier}", slot.word()),
            1,
        ),
        None => block,
    })
}

/// A painting's pixels: a PNG, on a transparent background.
fn painting(bytes: &[u8]) -> Result<(RgbaImage, (u32, u32)), Vec<String>> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(vec!["a painting must be a PNG".into()]);
    }
    let (image, size) = together_avatar::decode_within(bytes, MAX_PAINT_BYTES)?;
    if !image.pixels().any(|p| p[3] < 128) {
        return Err(vec![
            "a painting is drawn on a transparent background".into(),
        ]);
    }
    Ok((image, size))
}

/// Crop each returned frame to the shared subject square. Signed source
/// coordinates keep tall subjects near a cell edge from bleeding across
/// animation cells. No resampling is involved.
pub fn fit_picture(bytes: &[u8], frames: usize) -> Result<Vec<u8>, String> {
    let (image, (w, h)) = painting(bytes).map_err(|e| e.join("; "))?;
    if !(1..=MAX_FRAMES).contains(&frames) || w % frames as u32 != 0 {
        return Err("the painting width must divide evenly into its frames".into());
    }
    let crop = looks::crop(w, h, frames, |x, y| image.get_pixel(x, y)[3] >= 128)
        .ok_or("the painting has nothing drawn on it")?;
    if crop.x == 0 && crop.y == 0 && crop.span == crop.cell && crop.span == h {
        return Ok(bytes.to_vec());
    }
    let mut out = RgbaImage::new(crop.span * frames as u32, crop.span);
    for f in 0..frames as u32 { for y in 0..crop.span { for x in 0..crop.span {
        let (sx, sy) = (crop.x + i64::from(x), crop.y + i64::from(y));
        if sx >= 0 && sy >= 0 && sx < i64::from(crop.cell) && sy < i64::from(h) {
            *out.get_pixel_mut(f * crop.span + x, y) = *image.get_pixel(f * crop.cell + sx as u32, sy as u32);
        }
    }}}
    // A solid square can lose its entire transparent background in the
    // crop. Retain one clear pixel around each cell so the normalized PNG
    // still satisfies the painting format; the renderer crops it again.
    if out.pixels().all(|p| p[3] >= 128) {
        let side = crop.span + 2;
        let mut padded = RgbaImage::new(side * frames as u32, side);
        for f in 0..frames as u32 { for y in 0..crop.span { for x in 0..crop.span {
            *padded.get_pixel_mut(f * side + x + 1, y + 1) = *out.get_pixel(f * crop.span + x, y);
        }}}
        out = padded;
    }
    let mut bytes = std::io::Cursor::new(Vec::new());
    out.write_to(&mut bytes, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

/// A painting's normal map (`normal=<file_n.png>`): a PNG in the sheets
/// folder beside it, at most 4 MiB, exactly the painting's `size`. Its
/// bytes, to keep as they are.
fn normal_map(file: &str, dir: &std::path::Path, size: (u32, u32)) -> Result<Vec<u8>, String> {
    let plain = file
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain || file.starts_with('.') || !file.to_ascii_lowercase().ends_with(".png") {
        return Err(format!(
            "`{file}`: a normal map is a plain file name ending .png (letters, digits, - and _), in the sheets folder"
        ));
    }
    let path = dir.join(file);
    let inside = path
        .canonicalize()
        .ok()
        .zip(dir.canonicalize().ok())
        .is_some_and(|(resolved, root)| resolved.starts_with(root));
    if !inside {
        return Err(format!("no normal map {file} (it belongs in {})", dir.display()));
    }
    let meta = std::fs::metadata(&path)
        .map_err(|_| format!("no normal map {file} (it belongs in {})", dir.display()))?;
    if !meta.is_file() || meta.len() > MAX_PAINT_BYTES as u64 {
        return Err(format!("{file}: a normal map is a PNG of at most 4 MiB"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|f| f.take(MAX_PAINT_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{file}: {e}"))?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(format!("{file}: a normal map must be a PNG"));
    }
    let (_, (w, h)) = together_avatar::decode_within(&bytes, MAX_PAINT_BYTES)
        .map_err(|e| format!("{file}: {}", e.join("; ")))?;
    if (w, h) != size {
        return Err(format!(
            "{file}: a normal map is its painting's size, {}×{}, not {w}×{h}",
            size.0, size.1
        ));
    }
    Ok(bytes)
}

/// Keep a painting in the paint store under its sha256, and name it.
fn keep_painting(bytes: &[u8], paint: &std::path::Path) -> Result<String, Vec<String>> {
    let sha = angel_live::host::sha256_hex(bytes);
    let path = paint.join(format!("{sha}.png"));
    if path.is_file() {
        return Ok(sha);
    }
    // Written beside, then put in place: a reader never sees half.
    let kept = angel_live::host::workspace_store::write_atomic(&path, bytes)
        .map_err(std::io::Error::other);
    kept.map(|()| sha)
        .map_err(|e| vec![format!("the painting could not be kept in {}: {e}", paint.display())])
}

#[cfg(test)]
#[path = "../tests/together_sheets__tests.rs"]
mod tests;
